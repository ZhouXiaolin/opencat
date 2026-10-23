// Scene3 web executor: the SAME Scene3 IR the native engine feeds
// opencat-three (wgpu), rendered here by the browser's real three.js —
// three.js is the semantic standard, opencat-three is its portable
// implementation. Each commit carries the full scene snapshot; the raster
// lands in draw-ir's generated-image slot keyed by the commit's placeholder
// id, so the pixels composite at the exact z-position the script recorded.
//
// Alpha contract: straight (unpremultiplied) RGBA end to end —
// `WebGLRenderer({ alpha: true, premultipliedAlpha: false })` mirrors the
// native Rgba8Unorm-unpremul target and resolveImage's Unpremul MakeImage.
//
// ShaderMaterial: the IR carries the shim-adapted 450-core GLSL (varying →
// in/out, gl_FragColor → fragOutput, three builtins / user uniforms packed
// into std140 blocks). `adaptedToThreeGlsl` reverses that mechanical rewrite
// back to the WebGL1-style dialect real three.js compiles (r181 auto-upgrades
// it to `#version 300 es` on WebGL2) — the same dialect the reference page
// fed three.js, so user GLSL executes natively with no translation layer.
import * as THREE from 'three';
import { getAssetReader, getRendererOrThrow } from './wasm';
import { getDecodedFrameRgba } from './media/video-decoder';
import { provideGeneratedImageRgba } from './draw-ir';

// Captured at module load, before the script realm installs its
// `OffscreenCanvas` facade (which serves `getContext('2d')` from the
// composition surface and rejects everything else). This is host code, not
// composition code: it needs a real WebGL canvas.
const NativeOffscreenCanvas = globalThis.OffscreenCanvas;

type Vec3 = [number, number, number];

type TextureJson =
  | { VideoFrame: { path: string; time: number } }
  | { Bitmap: { path: string } };

type GeometryJson =
  | {
      Inline: {
        positions: Vec3[];
        normals: Vec3[];
        indices: number[];
        uvs?: [number, number][];
        colors?: [number, number, number, number][];
        colors1?: [number, number, number, number][];
      };
    }
  | { Asset: { id: string } };

type ShaderValueJson = { Float: number } | { Vec2: number[] } | { Vec3: number[] } | { Vec4: number[] };

type ShadingJson =
  | 'Unlit'
  | 'Lambert'
  | { VideoMosaic: unknown }
  | {
      Physical: {
        ior: number;
        thickness: number;
        transmission: number;
        roughness: number;
        metalness: number;
        specular_intensity: number;
        clearcoat: number;
        clearcoat_roughness: number;
        dispersion: number;
        attenuation_color: Vec3;
        attenuation_distance: number;
        env_map_intensity?: number;
      };
    }
  | {
      Shader: {
        vertex_shader: string;
        fragment_shader: string;
        uniforms?: { name: string; value: ShaderValueJson }[];
      };
    };

type Scene3Json = {
  clear: [number, number, number, number];
  target: [number, number];
  camera: { position: Vec3; target: Vec3; fov_y_degrees: number; near: number; far: number };
  lights: (
    | { Ambient: { color: Vec3; intensity: number } }
    | { Hemisphere: { sky_color: Vec3; ground_color: Vec3; intensity: number } }
    | { Directional: { direction: Vec3; color: Vec3; intensity: number } }
  )[];
  meshes: {
    geometry: GeometryJson;
    material: {
      base_color: [number, number, number, number];
      shading: ShadingJson;
      displacement?: unknown;
      map?: TextureJson;
      textures?: TextureJson[];
      transparent?: boolean;
    };
    matrix: number[];
    render_order?: number;
  }[];
  tone_mapping?: 'None' | 'ACESFilmic';
  tone_mapping_exposure?: number;
  output_color_space?: 'Linear' | 'Srgb';
  environment?: TextureJson;
};

type Scene3CommitJson = {
  /** Decimal string of the placeholder GeneratedImageId (u64). */
  imageId: string;
  targetId: string;
  width: number;
  height: number;
  scene: Scene3Json;
};

type GeometryBuffers = {
  positions: Float32Array;
  normals: Float32Array;
  uvs: Float32Array;
  indices: Uint32Array;
  colors: Float32Array;
  colors1: Float32Array;
};

// ---------------------------------------------------------------------------
// GLSL: adapted 450-core → WebGL1-style three.js dialect
// ---------------------------------------------------------------------------

// The shim strips #version/precision, packs three's per-program uniforms into
// a std140 block, rewrites the dialect — the reverse undoes exactly that,
// line-shaped, no general GLSL parsing.
const THREE_BUILTINS_BLOCK =
  /layout\(std140, set = 0, binding = 0\) uniform ThreeBuiltins \{[\s\S]*?\n\};\n?/;
const USER_UNIFORMS_BLOCK =
  /layout\(std140, set = 1, binding = 0\) uniform UserUniforms \{\n([\s\S]*?)\n\};\n?/;
const SHIM_ATTRIBUTE_LINES = [
  /layout\(location = 0\) in vec3 position;\n?/,
  /layout\(location = 1\) in vec3 normal;\n?/,
  /layout\(location = 2\) in vec2 uv;\n?/,
];
const MAP_SAMPLER_BLOCK =
  /layout\(set = 3, binding = 0\) uniform texture2D map;\nlayout\(set = 3, binding = 1\) uniform sampler map_smp;\n?/;
const FRAG_OUTPUT_DECL = /layout\(location = 0\) out vec4 fragOutput;\n?/;

// GLSL ES (1.00 + 3.00) future-use reserved words. The browser's ANGLE
// compiler rejects them as identifiers outright, while naga's glsl-in (the
// native path) accepts them — a user uniform named `half` renders on the
// engine and fails to compile the very same shader on the web. Rename them
// with a trailing underscore everywhere they appear (GLSL text + uniform
// dictionary keys); they can never legitimately appear as identifiers, so a
// word-boundary rename is safe, and both sides of the rename live in this
// module so the mapping stays consistent.
const GLSL_RESERVED = new Set([
  'asm', 'class', 'union', 'enum', 'typedef', 'template', 'this', 'packed',
  'goto', 'inline', 'noinline', 'volatile', 'public', 'static', 'extern',
  'external', 'interface', 'flat', 'long', 'short', 'double', 'half', 'fixed',
  'unsigned', 'superp', 'input', 'output', 'hvec2', 'hvec3', 'hvec4',
  'dvec2', 'dvec3', 'dvec4', 'fvec2', 'fvec3', 'fvec4',
  'sampler1D', 'sampler3D', 'sampler1DShadow', 'sampler2DShadow',
  'sampler2DRect', 'sampler3DRect', 'sampler2DRectShadow',
  'sizeof', 'cast', 'namespace', 'using',
  'coherent', 'restrict', 'readonly', 'writeonly', 'resource', 'atomic_uint',
  'noperspective', 'patch', 'sample', 'subroutine',
]);

function mapReservedIdentifiers(src: string): string {
  return src.replace(/[A-Za-z_][A-Za-z0-9_]*/g, (id) =>
    GLSL_RESERVED.has(id) ? id + '_' : id,
  );
}

function adaptedToThreeGlsl(stage: 'vertex' | 'fragment', src: string): string {
  let out = String(src)
    .replace(/^[ \t]*#version[^\n]*\n/gm, '')
    .replace(/^[ \t]*precision[ \t]+\w+[ \t]+\w+[ \t]*;[ \t]*\n?/gm, '')
    .replace(THREE_BUILTINS_BLOCK, '');
  for (const attr of SHIM_ATTRIBUTE_LINES) out = out.replace(attr, '');
  // User std140 block back to standalone uniforms (three resolves them by
  // name from ShaderMaterial.uniforms).
  out = out.replace(USER_UNIFORMS_BLOCK, (_, members: string) =>
    members
      .split('\n')
      .map((line) => 'uniform ' + line.trim() + '\n')
      .join(''),
  );
  if (stage === 'fragment') {
    out = out.replace(MAP_SAMPLER_BLOCK, 'uniform sampler2D map;\n');
    // Combined naga-style sample → GLSL1 texture2D (three #defines it to
    // `texture` on WebGL2).
    out = out.replace(/texture\(\s*sampler2D\(\s*map\s*,\s*map_smp\s*\)\s*,/g, 'texture2D(map,');
    out = out.replace(FRAG_OUTPUT_DECL, '');
    out = out.replace(/\bfragOutput\b/g, 'gl_FragColor');
    out = out.replace(/layout\(location = \d+\) in /g, 'varying ');
  } else {
    out = out.replace(/layout\(location = \d+\) out /g, 'varying ');
    // Remaining bare `in` are user-declared attributes (e.g. `in vec4 color_1`).
    out = out.replace(/\bin\b/g, 'attribute');
  }
  return mapReservedIdentifiers(out);
}

function shaderUniformValue(v: ShaderValueJson): number | THREE.Vector2 | THREE.Vector3 | THREE.Vector4 {
  if ('Float' in v) return v.Float;
  if ('Vec2' in v) return new THREE.Vector2(v.Vec2[0], v.Vec2[1]);
  if ('Vec3' in v) return new THREE.Vector3(v.Vec3[0], v.Vec3[1], v.Vec3[2]);
  return new THREE.Vector4(v.Vec4[0], v.Vec4[1], v.Vec4[2], v.Vec4[3]);
}

// IR colors are linear working-space straight components — feed them without
// a second sRGB decode (three's ColorManagement decodes only what declares
// itself sRGB-encoded).
function linearColor(rgb: readonly number[]): THREE.Color {
  const c = new THREE.Color();
  c.setRGB(rgb[0], rgb[1], rgb[2], THREE.LinearSRGBColorSpace);
  return c;
}

/** Native's ≥1e29 sentinel means three.js `attenuationDistance = Infinity`. */
function attenuationDistance(d: number): number {
  return d >= 1e29 ? Infinity : d;
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

export class Scene3WebExecutor {
  /** One offscreen renderer per Scene3 target (`Renderer(id, w, h)`). */
  private renderers = new Map<string, THREE.WebGLRenderer>();
  /** Realm-level geometry registry mirrors: id → typed buffers (never clears). */
  private geometries = new Map<string, GeometryBuffers>();
  /** Decoded texture sources by table key (`bitmap:path` / `videoframe:path@t`). */
  private textures = new Map<string, THREE.Texture>();
  /** PMREM cubeUV outputs keyed by the equirect source's table key. */
  private environments = new Map<string, THREE.Texture>();

  /** Decode every out-of-band resource this frame's commits reference:
      registry geometries (wasm), bitmap textures (asset reader), video-frame
      textures (decode cache), equirect environments (PMREM). */
  async prepare(commits: Scene3CommitJson[]): Promise<void> {
    const wasm = getRendererOrThrow();

    const geometryIds = new Set<string>();
    const textureKeys = new Map<string, TextureJson>();
    const envs = new Map<string, { tex: TextureJson; targetId: string; w: number; h: number }>();
    for (const commit of commits) {
      for (const mesh of commit.scene.meshes) {
        if ('Asset' in mesh.geometry) geometryIds.add(mesh.geometry.Asset.id);
        const mat = mesh.material;
        for (const tex of [...(mat.textures ?? []), ...(mat.map ? [mat.map] : [])]) {
          textureKeys.set(textureKey(tex), tex);
        }
      }
      const env = commit.scene.environment;
      if (env && !this.environments.has(textureKey(env))) {
        envs.set(textureKey(env), { tex: env, targetId: commit.targetId, w: commit.width, h: commit.height });
      }
    }

    for (const id of geometryIds) {
      if (!this.geometries.has(id)) this.geometries.set(id, wasm.get_scene3_geometry(id));
    }
    const reader = getAssetReader();
    for (const [key, tex] of textureKeys) {
      if (!this.textures.has(key)) this.textures.set(key, await this.decodeTexture(tex, reader, key));
    }
    for (const [key, env] of envs) {
      const source = await this.decodeTexture(env.tex, reader, key);
      source.colorSpace = THREE.SRGBColorSpace;
      source.mapping = THREE.EquirectangularReflectionMapping;
      const renderer = this.acquireRenderer(env.targetId, env.w, env.h);
      const pmrem = new THREE.PMREMGenerator(renderer);
      this.environments.set(key, pmrem.fromEquirectangular(source).texture);
      pmrem.dispose();
    }
  }

  /** Render every commit and publish its pixels into draw-ir's generated
      slot (`resolveImage('generated')` reads it at the placeholder z-site). */
  renderFrame(commits: Scene3CommitJson[]): void {
    for (const commit of commits) {
      const { scene } = commit;
      const [w, h] = commit.width && commit.height ? [commit.width, commit.height] : scene.target;
      const renderer = this.acquireRenderer(commit.targetId, w, h);
      renderer.outputColorSpace =
        scene.output_color_space === 'Srgb' ? THREE.SRGBColorSpace : THREE.LinearSRGBColorSpace;
      renderer.toneMapping =
        scene.tone_mapping === 'ACESFilmic' ? THREE.ACESFilmicToneMapping : THREE.NoToneMapping;
      renderer.toneMappingExposure = scene.tone_mapping_exposure ?? 1;
      // IR clear is already output-space encoded (shim mirrors WebGLBackground
      // setClear); declaring the same space to setRGB makes three's
      // working→output round-trip identity.
      const clearSpace =
        scene.output_color_space === 'Srgb' ? THREE.SRGBColorSpace : THREE.LinearSRGBColorSpace;
      const clear = new THREE.Color();
      clear.setRGB(scene.clear[0], scene.clear[1], scene.clear[2], clearSpace);
      renderer.setClearColor(clear, scene.clear[3]);

      const threeScene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(
        scene.camera.fov_y_degrees,
        w / h,
        scene.camera.near,
        scene.camera.far,
      );
      camera.position.set(...scene.camera.position);
      camera.lookAt(...scene.camera.target);

      for (const light of scene.lights) {
        if ('Ambient' in light) {
          threeScene.add(
            new THREE.AmbientLight(linearColor(light.Ambient.color), light.Ambient.intensity),
          );
        } else if ('Hemisphere' in light) {
          threeScene.add(
            new THREE.HemisphereLight(
              linearColor(light.Hemisphere.sky_color),
              linearColor(light.Hemisphere.ground_color),
              light.Hemisphere.intensity,
            ),
          );
        } else {
          // IR direction = light propagation (light → scene); three shines
          // position → target(origin), so position = -direction.
          const dir = new THREE.DirectionalLight(
            linearColor(light.Directional.color),
            light.Directional.intensity,
          );
          dir.position.set(-light.Directional.direction[0], -light.Directional.direction[1], -light.Directional.direction[2]);
          threeScene.add(dir);
        }
      }

      for (const mesh of scene.meshes) {
        const material = this.buildMaterial(mesh.material);
        const object = new THREE.Mesh(this.buildGeometry(mesh.geometry), material);
        object.matrixAutoUpdate = false;
        object.matrix.fromArray(mesh.matrix);
        object.matrixWorldNeedsUpdate = true;
        object.renderOrder = mesh.render_order ?? 0;
        threeScene.add(object);
      }

      if (scene.environment) {
        const env = this.environments.get(textureKey(scene.environment));
        if (!env) throw new Error(`scene3: environment ${textureKey(scene.environment)} not prepared`);
        threeScene.environment = env;
      }

      renderer.render(threeScene, camera);

      // GL default-framebuffer origin is bottom-left; flip to top-down rows.
      const gl = renderer.getContext() as WebGL2RenderingContext;
      const pixels = new Uint8Array(w * h * 4);
      gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      const row = w * 4;
      const out = new Uint8Array(w * h * 4);
      for (let y = 0; y < h; y++) {
        out.set(pixels.subarray((h - 1 - y) * row, (h - y) * row), y * row);
      }
      provideGeneratedImageRgba(BigInt(commit.imageId), w, h, out);
    }
  }

  private acquireRenderer(targetId: string, w: number, h: number): THREE.WebGLRenderer {
    let renderer = this.renderers.get(targetId);
    if (!renderer) {
      // Straight-alpha contract (module doc); 4× MSAA mirrors the native
      // main pass; offscreen canvas — pixels leave via readPixels only.
      // Offscreen canvas, not `document.createElement`: the script realm
      // replaces `document` with the composition overlay, so a DOM canvas
      // there is an overlay handle, not a drawable surface.
      const canvas = new NativeOffscreenCanvas(w, h);
      renderer = new THREE.WebGLRenderer({
        canvas,
        alpha: true,
        premultipliedAlpha: false,
        antialias: true,
      });
      this.renderers.set(targetId, renderer);
    }
    renderer.setSize(w, h, false);
    return renderer;
  }

  private buildGeometry(geometry: GeometryJson): THREE.BufferGeometry {
    const buffers =
      'Inline' in geometry
        ? {
            positions: flat3(geometry.Inline.positions),
            normals: flat3(geometry.Inline.normals),
            uvs: flat2(geometry.Inline.uvs ?? []),
            indices: new Uint32Array(geometry.Inline.indices),
            colors: flat4(geometry.Inline.colors ?? []),
            colors1: flat4(geometry.Inline.colors1 ?? []),
          }
        : this.geometries.get(geometry.Asset.id) ??
          (() => {
            throw new Error(`scene3: geometry ${geometry.Asset.id} not prepared`);
          })();
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(buffers.positions, 3));
    geo.setAttribute('normal', new THREE.BufferAttribute(buffers.normals, 3));
    if (buffers.uvs.length) geo.setAttribute('uv', new THREE.BufferAttribute(buffers.uvs, 2));
    if (buffers.colors.length) geo.setAttribute('color', new THREE.BufferAttribute(buffers.colors, 4));
    if (buffers.colors1.length) geo.setAttribute('color_1', new THREE.BufferAttribute(buffers.colors1, 4));
    geo.setIndex(new THREE.BufferAttribute(buffers.indices, 1));
    return geo;
  }

  private buildMaterial(material: Scene3Json['meshes'][number]['material']): THREE.Material {
    const [r, g, b, a] = material.base_color;
    const transparent = material.transparent ?? false;
    // opencat-three never backface-culls (no side concept in the shim/IR);
    // three.js defaults FrontSide, which drops half of a ring of planes.
    const side = THREE.DoubleSide;
    if (material.displacement) {
      throw new Error('scene3: noiseDisplacement is not supported by the web executor yet');
    }
    const shading = material.shading;
    if (shading === 'Unlit') {
      const mat = new THREE.MeshBasicMaterial({ opacity: a, transparent, side });
      mat.color.setRGB(r, g, b, THREE.LinearSRGBColorSpace);
      return this.applyMap(mat, material.map);
    }
    if (shading === 'Lambert') {
      const mat = new THREE.MeshLambertMaterial({ opacity: a, transparent, side });
      mat.color.setRGB(r, g, b, THREE.LinearSRGBColorSpace);
      return this.applyMap(mat, material.map);
    }
    if ('VideoMosaic' in shading) {
      throw new Error('scene3: VideoMosaic is not supported by the web executor');
    }
    if ('Physical' in shading) {
      const p = shading.Physical;
      const mat = new THREE.MeshPhysicalMaterial({
        opacity: a,
        transparent,
        side,
        ior: p.ior,
        thickness: p.thickness,
        transmission: p.transmission,
        roughness: p.roughness,
        metalness: p.metalness,
        specularIntensity: p.specular_intensity,
        clearcoat: p.clearcoat,
        clearcoatRoughness: p.clearcoat_roughness,
        dispersion: p.dispersion,
        attenuationColor: linearColor(p.attenuation_color),
        attenuationDistance: attenuationDistance(p.attenuation_distance),
        envMapIntensity: p.env_map_intensity ?? 1,
      });
      mat.color.setRGB(r, g, b, THREE.LinearSRGBColorSpace);
      return this.applyMap(mat, material.map);
    }
    // ShaderMaterial: reverse the shim's mechanical rewrite so real three.js
    // compiles the WebGL1-style source it was written for (module doc).
    const s = shading.Shader;
    const uniforms: Record<string, { value: number | THREE.Vector2 | THREE.Vector3 | THREE.Vector4 }> = {};
    // Same reserved-word rename as the GLSL text (mapReservedIdentifiers).
    for (const u of s.uniforms ?? []) uniforms[mapReservedIdentifiers(u.name)] = { value: shaderUniformValue(u.value) };
    return new THREE.ShaderMaterial({
      uniforms,
      vertexShader: adaptedToThreeGlsl('vertex', s.vertex_shader),
      fragmentShader: adaptedToThreeGlsl('fragment', s.fragment_shader),
      transparent,
      side,
    });
  }

  private applyMap<M extends THREE.Material & { map: THREE.Texture | null }>(mat: M, map: TextureJson | undefined): M {
    if (!map) return mat;
    const tex = this.textures.get(textureKey(map));
    if (!tex) throw new Error(`scene3: texture ${textureKey(map)} not prepared`);
    mat.map = tex;
    return mat;
  }

  private async decodeTexture(
    tex: TextureJson,
    reader: ((path: string) => Uint8Array | ArrayBuffer | number[] | Promise<Uint8Array | ArrayBuffer | number[]>) | null,
    key: string,
  ): Promise<THREE.Texture> {
    if ('Bitmap' in tex) {
      if (!reader) throw new Error(`scene3: no asset reader for ${key}`);
      const bytes = await reader(tex.Bitmap.path);
      const blob = new Blob([bytes as BlobPart], { type: 'image/png' });
      const bitmap = await createImageBitmap(blob);
      const texture = new THREE.Texture(bitmap);
      // Color maps decode sRGB → linear in the sampler (three convention for
      // albedo maps; albedo = map × base_color in the IR contract).
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.needsUpdate = true;
      return texture;
    }
    if ('VideoFrame' in tex) {
      const decoded = await getDecodedFrameRgba(tex.VideoFrame.path, tex.VideoFrame.time, 'exact');
      if (!decoded) throw new Error(`scene3: video frame unavailable for ${key}`);
      const data = new THREE.DataTexture(
        new Uint8Array(decoded.rgba),
        decoded.width,
        decoded.height,
        THREE.RGBAFormat,
        THREE.UnsignedByteType,
      );
      // Memory rows are top-down; flipY=true matches three's image-texture
      // convention (and the native sampler's flipY contract).
      data.flipY = true;
      data.needsUpdate = true;
      return data;
    }
    throw new Error(`scene3: unknown texture source ${key}`);
  }
}

function textureKey(tex: TextureJson): string {
  if ('Bitmap' in tex) return `bitmap:${tex.Bitmap.path}`;
  return `videoframe:${tex.VideoFrame.path}@${tex.VideoFrame.time.toFixed(6)}`;
}

function flat3(v: readonly (readonly number[])[]): Float32Array {
  const out = new Float32Array(v.length * 3);
  for (let i = 0; i < v.length; i++) {
    out[i * 3] = v[i][0];
    out[i * 3 + 1] = v[i][1];
    out[i * 3 + 2] = v[i][2];
  }
  return out;
}

function flat2(v: readonly (readonly number[])[]): Float32Array {
  const out = new Float32Array(v.length * 2);
  for (let i = 0; i < v.length; i++) {
    out[i * 2] = v[i][0];
    out[i * 2 + 1] = v[i][1];
  }
  return out;
}

function flat4(v: readonly (readonly number[])[]): Float32Array {
  const out = new Float32Array(v.length * 4);
  for (let i = 0; i < v.length; i++) {
    out[i * 4] = v[i][0];
    out[i * 4 + 1] = v[i][1];
    out[i * 4 + 2] = v[i][2];
    out[i * 4 + 3] = v[i][3];
  }
  return out;
}

/** Page-level singleton: one executor per document (renderers are expensive,
    geometry/texture caches are realm-level by contract). */
export const scene3Executor = new Scene3WebExecutor();
