import * as THREE from "three";
import { GLTFLoader } from "./jsm/loaders/GLTFLoader.js";

export var renderAt = function () {};

export function initLiquid() {

const WIDTH = 1920;
const HEIGHT = 1080;
const DURATION = 6;
const POP_AT = 2.8;
const canvas = document.getElementById("liquid-pop-canvas");
const stage = document.querySelector("[data-composition-id]");
const brand = stage.dataset.brand || "heygen-webgl";
const isHeygenWebgl = brand === "heygen-webgl";
const logoAsset = stage.dataset.logoSrc || "assets/heygen-prism-official.svg";
const wordmarkAsset =
  stage.dataset.wordmarkSrc ||
  (isHeygenWebgl ? "assets/heygen-wordmark-official.svg" : "assets/hyperframes-wordmark-official.svg");

const renderer = new THREE.WebGLRenderer({
  canvas,
  alpha: false,
  antialias: true,
  preserveDrawingBuffer: true,
  powerPreference: "high-performance",
});
renderer.setSize(WIDTH, HEIGHT, false);
renderer.setPixelRatio(1);
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.08;

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(35, WIDTH / HEIGHT, 0.1, 100);
camera.position.set(0, 0, 7);
camera.lookAt(0, 0, 0);

const textureLoader = new THREE.TextureLoader();
const gltfLoader = new GLTFLoader();
const environmentUrl = "assets/equirect-garden.jpg";
const logoUrl = logoAsset.replace(/^\/+/, "");
const wordmarkUrl = wordmarkAsset.replace(/^\/+/, "");
const prismUrl = "assets/heygen-prism.glb";

return Promise.all([
  textureLoader.loadAsync(environmentUrl),
  isHeygenWebgl ? gltfLoader.loadAsync(prismUrl) : textureLoader.loadAsync(logoUrl),
  textureLoader.loadAsync(wordmarkUrl),
]).then(function (loaded) { initLoaded(loaded[0], loaded[1], loaded[2]); });

function initLoaded(gardenEnvironment, subjectAsset, wordmarkTexture) {

gardenEnvironment.mapping = THREE.EquirectangularReflectionMapping;
gardenEnvironment.colorSpace = THREE.SRGBColorSpace;
scene.environment = gardenEnvironment;
if (!isHeygenWebgl) {
  subjectAsset.colorSpace = THREE.SRGBColorSpace;
  subjectAsset.minFilter = THREE.LinearMipmapLinearFilter;
  subjectAsset.magFilter = THREE.LinearFilter;
  subjectAsset.anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
}
wordmarkTexture.colorSpace = THREE.SRGBColorSpace;
wordmarkTexture.minFilter = THREE.LinearFilter;
wordmarkTexture.magFilter = THREE.LinearFilter;
wordmarkTexture.anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());

const backgroundMaterial = new THREE.ShaderMaterial({
  depthWrite: false,
  depthTest: false,
  toneMapped: false,
  uniforms: { uFade: { value: 1 } },
  vertexShader: `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `,
  fragmentShader: `
    precision highp float;
    varying vec2 vUv;
    uniform float uFade;

    float field(vec2 uv, vec2 center, vec2 spread) {
      vec2 d = (uv - center) / spread;
      return exp(-dot(d, d) * 1.7);
    }

    void main() {
      vec3 cream = vec3(0.957, 0.949, 0.918);
      vec3 sage = vec3(0.839, 0.898, 0.761);
      vec3 teal = vec3(0.494, 0.714, 0.643);
      vec3 mauve = vec3(0.773, 0.588, 0.643);
      vec3 color = cream;
      color = mix(color, sage, field(vUv, vec2(0.18, 0.64), vec2(0.42, 0.52)) * 0.72);
      color = mix(color, teal, field(vUv, vec2(0.78, 0.72), vec2(0.36, 0.46)) * 0.58);
      color = mix(color, mauve, field(vUv, vec2(0.72, 0.22), vec2(0.38, 0.38)) * 0.54);
      color *= 0.985 + 0.035 * vUv.y;
      gl_FragColor = vec4(mix(cream, color, uFade), 1.0);
    }
  `,
});
const background = new THREE.Mesh(new THREE.PlaneGeometry(15, 8.5), backgroundMaterial);
background.position.set(0, 0, -3.5);
background.renderOrder = -10;
scene.add(background);

scene.add(new THREE.HemisphereLight(0xffffff, 0x7eb6a4, 2.1));
const keyLight = new THREE.DirectionalLight(0xffffff, 3.4);
keyLight.position.set(-4, 5, 6);
scene.add(keyLight);
const rimLight = new THREE.DirectionalLight(0xc596ff, 2.2);
rimLight.position.set(5, -2, 3);
scene.add(rimLight);

const SIMPLEX_NOISE = `
vec4 permute(vec4 x) { return mod(((x*34.0)+1.0)*x, 289.0); }
vec4 taylorInvSqrt(vec4 r) { return 1.79284291400159 - 0.85373472095314 * r; }
float snoise(vec3 v){
  const vec2 C = vec2(1.0/6.0, 1.0/3.0);
  const vec4 D = vec4(0.0, 0.5, 1.0, 2.0);
  vec3 i = floor(v + dot(v, C.yyy));
  vec3 x0 = v - i + dot(i, C.xxx);
  vec3 g = step(x0.yzx, x0.xyz);
  vec3 l = 1.0 - g;
  vec3 i1 = min(g.xyz, l.zxy);
  vec3 i2 = max(g.xyz, l.zxy);
  vec3 x1 = x0 - i1 + C.xxx;
  vec3 x2 = x0 - i2 + C.yyy;
  vec3 x3 = x0 - D.yyy;
  i = mod(i, 289.0);
  vec4 p = permute(permute(permute(
    i.z + vec4(0.0, i1.z, i2.z, 1.0))
    + i.y + vec4(0.0, i1.y, i2.y, 1.0))
    + i.x + vec4(0.0, i1.x, i2.x, 1.0));
  float n_ = 0.142857142857;
  vec3 ns = n_ * D.wyz - D.xzx;
  vec4 j = p - 49.0 * floor(p * ns.z * ns.z);
  vec4 x_ = floor(j * ns.z);
  vec4 y_ = floor(j - 7.0 * x_);
  vec4 x = x_ * ns.x + ns.yyyy;
  vec4 y = y_ * ns.x + ns.yyyy;
  vec4 h = 1.0 - abs(x) - abs(y);
  vec4 b0 = vec4(x.xy, y.xy);
  vec4 b1 = vec4(x.zw, y.zw);
  vec4 s0 = floor(b0) * 2.0 + 1.0;
  vec4 s1 = floor(b1) * 2.0 + 1.0;
  vec4 sh = -step(h, vec4(0.0));
  vec4 a0 = b0.xzyw + s0.xzyw * sh.xxyy;
  vec4 a1 = b1.xzyw + s1.xzyw * sh.zzww;
  vec3 p0 = vec3(a0.xy, h.x);
  vec3 p1 = vec3(a0.zw, h.y);
  vec3 p2 = vec3(a1.xy, h.z);
  vec3 p3 = vec3(a1.zw, h.w);
  vec4 norm = taylorInvSqrt(vec4(dot(p0,p0), dot(p1,p1), dot(p2,p2), dot(p3,p3)));
  p0 *= norm.x;
  p1 *= norm.y;
  p2 *= norm.z;
  p3 *= norm.w;
  vec4 m = max(0.6 - vec4(dot(x0,x0), dot(x1,x1), dot(x2,x2), dot(x3,x3)), 0.0);
  m *= m;
  return 42.0 * dot(m*m, vec4(dot(p0,x0), dot(p1,x1), dot(p2,x2), dot(p3,x3)));
}
`;

function makeLiquidMaterial() {
  const uniformState = { uTime: { value: 0 } };
  const material = new THREE.MeshPhysicalMaterial({
    color: 0xffffff,
    transmission: 1,
    opacity: 1,
    metalness: 0,
    roughness: 0,
    ior: 1.33,
    thickness: 1.5,
    specularIntensity: 2,
    clearcoat: 1,
    clearcoatRoughness: 0,
    dispersion: 1,
    envMapIntensity: 1.2,
    transparent: true,
    attenuationColor: new THREE.Color(0x88ce7f),
    attenuationDistance: 2,
  });

  material.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = uniformState.uTime;
    shader.vertexShader = shader.vertexShader.replace(
      "#include <common>",
      `#include <common>\n${SIMPLEX_NOISE}\nuniform float uTime;\nfloat getDisplacement(vec3 p, vec3 worldP) {\n  return snoise(p * 1.5 + uTime * 0.4 + worldP * 0.1) * 0.12;\n}`,
    );
    shader.vertexShader = shader.vertexShader.replace(
      "#include <begin_vertex>",
      `vec3 pos = position;
      vec4 worldP = modelMatrix * vec4(pos, 1.0);
      float offset = 0.01;
      vec3 tangent = cross(normal, vec3(0.0, 1.0, 0.0));
      if (length(tangent) < 0.1) tangent = cross(normal, vec3(1.0, 0.0, 0.0));
      tangent = normalize(tangent);
      vec3 bitangent = normalize(cross(normal, tangent));
      vec3 p1 = pos + tangent * offset;
      vec3 p2 = pos + bitangent * offset;
      vec4 worldP1 = modelMatrix * vec4(p1, 1.0);
      vec4 worldP2 = modelMatrix * vec4(p2, 1.0);
      float d0 = getDisplacement(pos, worldP.xyz);
      float d1 = getDisplacement(p1, worldP1.xyz);
      float d2 = getDisplacement(p2, worldP2.xyz);
      vec3 pos0 = pos + normal * d0;
      vec3 pos1 = p1 + normal * d1;
      vec3 pos2 = p2 + normal * d2;
      objectNormal = normalize(cross(pos1 - pos0, pos2 - pos0));
      vec3 transformed = pos0;`,
    );
  };
  material.customProgramCacheKey = () => "liquid-refraction-reference-v2";
  material.userData.uniformState = uniformState;
  return material;
}

const heroMaterial = makeLiquidMaterial();
const dropletMaterial = makeLiquidMaterial();
const hero = new THREE.Mesh(new THREE.SphereGeometry(1.5, 128, 128), heroMaterial);
hero.position.y = 0.1;
hero.renderOrder = 4;
scene.add(hero);

const brandMark = new THREE.Group();
let logoAspect = null;
let refractedLogoPlane = null;
let revealedLogoPlane = null;
let prismMesh = null;
let prismFacetTextures = [];

if (isHeygenWebgl) {
  prismFacetTextures = Array.from({ length: 8 }, (_, index) => {
    const video = document.getElementById(`facet-video-${index}`);
    if (!video) throw new Error(`Missing HeyGen prism facet video ${index + 1}`);
    const texture = new THREE.VideoTexture(video);
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.minFilter = THREE.LinearFilter;
    texture.magFilter = THREE.LinearFilter;
    texture.wrapS = THREE.RepeatWrapping;
    texture.wrapT = THREE.RepeatWrapping;
    texture.flipY = true;
    return texture;
  });
  const prismUniforms = {
    uResolution: { value: new THREE.Vector2(WIDTH, HEIGHT) },
  };
  prismFacetTextures.forEach((texture, index) => {
    prismUniforms[`uVideo${index}`] = { value: texture };
  });
  const prismMaterial = new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    vertexColors: true,
    depthWrite: true,
    toneMapped: false,
    uniforms: prismUniforms,
    vertexShader: `
      in vec4 color_1;
      out vec2 vUv;
      flat out int vIsMainFace;
      flat out int vVideoId;
      out vec3 vCornerColor;
      const vec3 kCornerColors[3] = vec3[3](
        vec3(1.000, 0.678, 0.984),
        vec3(0.082, 0.824, 0.000),
        vec3(0.000, 0.765, 1.000)
      );
      int modelFaceIdToVideoId(int id) {
        switch (id) {
          case 0: return 6; case 1: return 3; case 2: return 4; case 3: return 1;
          case 4: return 7; case 5: return 2; case 6: return 5; case 7: return 0;
          default: return 0;
        }
      }
      vec3 videoIdToCornerColor(int videoId) {
        int dominant = color.x > color.y ? (color.x > color.z ? 0 : 2) : (color.y > color.z ? 1 : 2);
        switch (videoId) {
          case 0: return kCornerColors[(dominant + 2) % 3];
          case 1: return kCornerColors[(dominant + 1) % 3];
          case 2: return kCornerColors[(dominant + 1) % 3];
          case 3: return kCornerColors[(dominant + 2) % 3];
          case 4: return kCornerColors[2 - (dominant + 2) % 3];
          case 5: return kCornerColors[2 - (dominant + 1) % 3];
          case 6: return kCornerColors[2 - (dominant + 1) % 3];
          case 7: return kCornerColors[2 - (dominant + 2) % 3];
          default: return vec3(0.0);
        }
      }
      void main() {
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        vUv = uv;
        vIsMainFace = color.r < 0.001 && color.g < 0.001 && color.b < 0.001 ? 1 : 0;
        vVideoId = modelFaceIdToVideoId(int(color_1.r * 255.0));
        vCornerColor = videoIdToCornerColor(vVideoId);
      }
    `,
    fragmentShader: `
      uniform sampler2D uVideo0;
      uniform sampler2D uVideo1;
      uniform sampler2D uVideo2;
      uniform sampler2D uVideo3;
      uniform sampler2D uVideo4;
      uniform sampler2D uVideo5;
      uniform sampler2D uVideo6;
      uniform sampler2D uVideo7;
      uniform vec2 uResolution;
      in vec2 vUv;
      flat in int vIsMainFace;
      flat in int vVideoId;
      in vec3 vCornerColor;
      out vec4 outColor;
      void main() {
        vec3 finalColor = vCornerColor;
        if (vIsMainFace == 1) {
          float distanceFromCenter = distance(vUv, vec2(0.5));
          float frostEffect = distanceFromCenter * distanceFromCenter * 1.5;
          vec2 surfaceUv = (gl_FragCoord.xy / uResolution - 0.5) / 0.75 + 0.5;
          vec4 albedoColor = vec4(0.0);
          switch (vVideoId) {
            case 0: albedoColor = texture(uVideo0, surfaceUv); break;
            case 1: albedoColor = texture(uVideo1, surfaceUv); break;
            case 2: albedoColor = texture(uVideo2, surfaceUv); break;
            case 3: albedoColor = texture(uVideo3, surfaceUv); break;
            case 4: albedoColor = texture(uVideo4, surfaceUv); break;
            case 5: albedoColor = texture(uVideo5, surfaceUv); break;
            case 6: albedoColor = texture(uVideo6, surfaceUv); break;
            case 7: albedoColor = texture(uVideo7, surfaceUv); break;
          }
          finalColor = albedoColor.rgb + frostEffect;
        }
        outColor = vec4(finalColor, 1.0);
      }
    `,
  });
  let sourceMesh = null;
  subjectAsset.scene.traverse((object) => {
    if (!sourceMesh && object.isMesh) sourceMesh = object;
  });
  if (!sourceMesh) throw new Error("HeyGen prism GLB contains no mesh");
  prismMesh = new THREE.Mesh(sourceMesh.geometry, prismMaterial);
  prismMesh.scale.setScalar(0.72);
  prismMesh.rotation.order = "YXZ";
  prismMesh.renderOrder = 1;
  prismMesh.frustumCulled = false;
  brandMark.add(prismMesh);
} else {
  const logoTexture = subjectAsset;
  const logoWidth = logoTexture.image.naturalWidth || logoTexture.image.width || 1;
  const logoHeight = logoTexture.image.naturalHeight || logoTexture.image.height || 1;
  logoAspect = logoWidth / logoHeight;
  const maxLogoExtent = 1.54;
  const planeWidth = logoAspect >= 1 ? maxLogoExtent : maxLogoExtent * logoAspect;
  const planeHeight = logoAspect >= 1 ? maxLogoExtent / logoAspect : maxLogoExtent;
  const refractedLogoMaterial = new THREE.MeshBasicMaterial({
    map: logoTexture,
    alphaTest: 0.02,
    depthWrite: true,
    toneMapped: false,
    side: THREE.DoubleSide,
  });
  const revealedLogoMaterial = new THREE.MeshBasicMaterial({
    map: logoTexture,
    transparent: true,
    depthWrite: false,
    toneMapped: false,
    side: THREE.DoubleSide,
  });
  const logoGeometry = new THREE.PlaneGeometry(planeWidth, planeHeight);
  refractedLogoPlane = new THREE.Mesh(logoGeometry, refractedLogoMaterial);
  revealedLogoPlane = new THREE.Mesh(logoGeometry, revealedLogoMaterial);
  refractedLogoPlane.renderOrder = 1;
  revealedLogoPlane.renderOrder = 2;
  brandMark.add(refractedLogoPlane, revealedLogoPlane);
}
brandMark.position.set(0, 0.02, -0.35);
brandMark.rotation.set(0, 0, 0);
brandMark.scale.set(1, 1, 1);
scene.add(brandMark);

// Official name artwork appears only after rupture. The carrier remains the
// existing live prism / official symbol; this plane contains name paths only.
const wordmarkWidth = wordmarkTexture.image.naturalWidth || wordmarkTexture.image.width || 1;
const wordmarkHeight = wordmarkTexture.image.naturalHeight || wordmarkTexture.image.height || 1;
const wordmarkAspect = wordmarkWidth / wordmarkHeight;
const finalWordmarkHeight = isHeygenWebgl ? 0.55 : 0.34;
const wordmarkReveal = { value: 0 };
const wordmarkMaterial = new THREE.MeshBasicMaterial({
  map: wordmarkTexture,
  alphaTest: 0.01,
  depthWrite: true,
  toneMapped: false,
  side: THREE.DoubleSide,
});
wordmarkMaterial.onBeforeCompile = (shader) => {
  shader.uniforms.uWordmarkReveal = wordmarkReveal;
  shader.fragmentShader = shader.fragmentShader.replace(
    "#include <map_fragment>",
    "#include <map_fragment>\nif (vMapUv.x > uWordmarkReveal) discard;",
  );
  shader.fragmentShader = shader.fragmentShader.replace(
    "#include <common>",
    "#include <common>\nuniform float uWordmarkReveal;",
  );
};
wordmarkMaterial.customProgramCacheKey = () => `official-wordmark-wipe-${brand}`;
const wordmarkPlane = new THREE.Mesh(
  new THREE.PlaneGeometry(finalWordmarkHeight * wordmarkAspect, finalWordmarkHeight),
  wordmarkMaterial,
);
wordmarkPlane.position.set(isHeygenWebgl ? 0.35 : 0.45, 0.14, -0.35);
wordmarkPlane.renderOrder = 2;
wordmarkPlane.visible = false;
scene.add(wordmarkPlane);

const droplets = [];
const goldenAngle = THREE.MathUtils.degToRad(137.508);
for (let i = 0; i < 15; i += 1) {
  const radius = 0.1 + 0.025 * (i % 5);
  const angle = THREE.MathUtils.degToRad(12) + i * goldenAngle;
  const ring = 1.72 + 0.16 * (i % 4);
  const y = Math.sin(angle * 1.37) * (1.2 + 0.07 * (i % 3));
  const x = Math.cos(angle) * ring;
  const z = -0.15 + 0.16 * ((i % 5) - 2);
  const base = new THREE.Vector3(x, y + 0.1, z);
  const direction = new THREE.Vector3(x, y * 0.88, 0.08 * ((i % 3) - 1)).normalize();
  const mesh = new THREE.Mesh(new THREE.SphereGeometry(radius, 32, 32), dropletMaterial);
  mesh.position.copy(base);
  mesh.userData = {
    base,
    direction,
    delay: (i % 4) / 30,
    travel: 1.65 + 0.18 * (i % 4),
  };
  mesh.renderOrder = 3;
  droplets.push(mesh);
  scene.add(mesh);
}

const clamp01 = (value) => Math.max(0, Math.min(1, value));
const smooth = (value) => {
  const p = clamp01(value);
  return p * p * (3 - 2 * p);
};
const power3In = (value) => Math.pow(clamp01(value), 3);
const power3Out = (value) => 1 - Math.pow(1 - clamp01(value), 3);
const power4In = (value) => Math.pow(clamp01(value), 4);

function liquidClock(time) {
  if (time <= 2.4) return time;
  if (time <= 2.8) return 2.4;
  if (time <= 3.6) return 2.4 + (time - 2.8);
  return 3.2;
}

renderAt = function (rawTime) {
  const time = Math.max(0, Math.min(DURATION, Number(rawTime) || 0));
  const establish = smooth(time / 0.6);
  const pressure = power3In((time - 1.8) / 0.6);
  const rupture = power4In((time - POP_AT) / 0.2);
  const fade = smooth((time - 5.4) / 0.6);

  camera.position.z = THREE.MathUtils.lerp(6.55, 7, establish);
  camera.lookAt(0, 0, 0);

  const heroBaseScale = THREE.MathUtils.lerp(1.08, 1, establish);
  const heroX = heroBaseScale * THREE.MathUtils.lerp(1, 1.1, pressure);
  const heroY = heroBaseScale * THREE.MathUtils.lerp(1, 0.78, pressure);
  const heroPopScale = THREE.MathUtils.lerp(1, 0.02, rupture);
  hero.scale.set(heroX * heroPopScale, heroY * heroPopScale, heroBaseScale * heroPopScale);
  hero.visible = time < 3.02;
  if (refractedLogoPlane && revealedLogoPlane) {
    refractedLogoPlane.visible = time < POP_AT;
    revealedLogoPlane.visible = time >= POP_AT;
  }

  const markEstablishScale = THREE.MathUtils.lerp(0.92, 1, smooth(time / 0.6));
  const markPressureScale = time < 1.8 ? markEstablishScale : THREE.MathUtils.lerp(1, 1.06, pressure);
  let markScaleAtTime = markPressureScale;
  let markRotation = 0;
  let markX = 0;
  if (time >= POP_AT && time < 3.0) {
    const recoil = smooth((time - POP_AT) / 0.2);
    markScaleAtTime = THREE.MathUtils.lerp(1.06, 0.97, recoil);
    markRotation = THREE.MathUtils.degToRad(THREE.MathUtils.lerp(0, 4, recoil));
  } else if (time >= 3.0) {
    const dock = smooth((time - 3.0) / 0.4);
    markScaleAtTime = THREE.MathUtils.lerp(0.97, 0.42, dock);
    markRotation = THREE.MathUtils.degToRad(THREE.MathUtils.lerp(4, 0, dock));
    markX = THREE.MathUtils.lerp(0, -1.15, dock);
  }
  brandMark.scale.setScalar(markScaleAtTime);
  brandMark.position.x = markX;
  brandMark.position.y = THREE.MathUtils.lerp(0.02, 0.14, pressure);
  brandMark.rotation.set(0, 0, markRotation);
  const lockupReveal = smooth((time - 3.0) / 0.4);
  wordmarkReveal.value = lockupReveal;
  wordmarkPlane.visible = time >= 3.0;
  if (prismMesh) {
    const prismTime = Math.min(time, 2.4);
    const settleProgress = clamp01(prismTime / 2.4);
    const settle = 1 - Math.pow(1 - settleProgress, 5);
    const heroX = 0.139 * Math.PI;
    const heroY = -1.17 * Math.PI;
    prismMesh.rotation.x = heroX + (1 - settle) * 0.22 * Math.PI;
    prismMesh.rotation.y = heroY - (1 - settle) * 3 * Math.PI * 2;
    if (time >= 3.4) {
      prismMesh.rotation.x = heroX;
      prismMesh.rotation.y = heroY + 0.55 * (Math.min(time, 5.4) - 3.4);
    }
    prismMesh.rotation.z = 0;
  }

  for (const droplet of droplets) {
    const { base, direction, delay, travel } = droplet.userData;
    if (time < POP_AT + delay) {
      droplet.position.copy(base);
      droplet.scale.setScalar(THREE.MathUtils.lerp(0.62, 1, smooth((time - 0.6) / 1.2)));
      continue;
    }
    const launch = power3Out((time - POP_AT - delay) / 0.8);
    const settle = smooth((time - 3.6) / 0.8);
    droplet.position.copy(base).addScaledVector(direction, travel * launch * (1 - 0.04 * settle));
    droplet.position.y -= 0.3 * launch * launch + 0.1 * settle;
    droplet.scale.setScalar(THREE.MathUtils.lerp(1, 0.76, launch));
  }

  const shaderTime = liquidClock(time);
  heroMaterial.userData.uniformState.uTime.value = shaderTime;
  dropletMaterial.userData.uniformState.uTime.value = shaderTime;
  backgroundMaterial.uniforms.uFade.value = 1 - fade;
  canvas.style.opacity = String(1 - fade);
  prismFacetTextures.forEach((texture) => {
    const video = texture.image;
    const injectedFrame = video?.nextElementSibling;
    const hasInjectedFrame =
      injectedFrame?.tagName === "IMG" &&
      injectedFrame.classList.contains("__render_frame__") &&
      injectedFrame.complete &&
      injectedFrame.naturalWidth > 0;
    if (hasInjectedFrame || video?.readyState >= 2) texture.needsUpdate = true;
  });

  renderer.render(scene, camera);
}

window.__liquidPopState = {
  ready: true,
  brand,
  logoAsset,
  wordmarkAsset,
  logoAspect,
  wordmarkAspect,
  heroSegments: [128, 128],
  satelliteCount: droplets.length,
  material: {
    transmission: heroMaterial.transmission,
    ior: heroMaterial.ior,
    thickness: heroMaterial.thickness,
    specularIntensity: heroMaterial.specularIntensity,
    clearcoat: heroMaterial.clearcoat,
    dispersion: heroMaterial.dispersion,
    envMapIntensity: heroMaterial.envMapIntensity,
  },
  orientation: { scaleSign: [1, 1, 1], eulerDegrees: [0, 0, 0] },
};

// 时间由 App 的 onFrame(T) 驱动（T = ctx.current.time），不监听 HF 播放器的
// hf-seek 事件——那是 hyperframes 私有事件，opencat 不派发。
renderAt(window.__hfThreeTime || 0);
}
}