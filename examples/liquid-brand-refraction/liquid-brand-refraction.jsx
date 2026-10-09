// JSX frontend port of examples/liquid-brand-refraction/liquid-brand-refraction.xml.
//
// The whole threejs-like scene graph + renderAt(t) body moves into a single
// useThree callback: `build(three, canvasNode, t, ctx)`. Everything else
// (constants, scene construction, per-frame updates, renderer.render) is the
// XML <script> verbatim, so the render stays pixel-comparable.

import { useThree, useRef } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 6.0 };

export default function LiquidBrandRefraction() {
  const canvas = useRef();

  useThree(canvas, (T3, node, t, ctx) => {
        /* ════════════════════════════════════════════════════════════════════
           Per-frame procedural port of js/liquid-pop-v2.js renderAt(t).
           Constants, ease helpers and phase boundaries copied 1:1.
           （提升前此脚本上半部还有 2D 胶水段：投影 helpers FOCAL/projX/projY、
           BGFIELD effect lambda、假玻璃滴 HERO lambda、PRISM_HULL3 投影、
           DROPS 2D 表、#wordmark clip-path 段——对应 3D 实现接通后随提升轮
           退役，见文件头注释。）
           ════════════════════════════════════════════════════════════════════ */
        var DURATION = 6.0, POP_AT = 2.8, FPS = 30;

        function clamp01(v) { return Math.max(0, Math.min(1, v)); }
        function smooth(v) { var p = clamp01(v); return p * p * (3 - 2 * p); }
        function power3In(v) { return Math.pow(clamp01(v), 3); }
        function power3Out(v) { return 1 - Math.pow(1 - clamp01(v), 3); }
        function power4In(v) { return Math.pow(clamp01(v), 4); }
        function lerp(a, b, k) { return a + (b - a) * k; }
        /* js:445-449 liquidClock 逐字：着色器时钟在 melt/rupture 冻住表面
           （2.4），pop 后 2.8..3.6 以 1/4 速回走再冻结在 3.2。 */
        function liquidClock(time) {
          if (time <= 2.4) return time;
          if (time <= 2.8) return 2.4;
          if (time <= 3.6) return 2.4 + (time - 2.8);
          return 3.2;
        }

        // t (clamped to [0, DURATION]) arrives as the callback argument.
        var establish = smooth(t / 0.6);
        var pressure  = power3In((t - 1.8) / 0.6);
        var rupture   = power4In((t - POP_AT) / 0.2);
        var fade      = smooth((t - 5.4) / 0.6);
        var canvasOpacity = 1 - fade;      // reference: canvas.style.opacity = 1-fade
        /* js:528 `canvas.style.opacity = String(1 - fade)` 作用于 WebGL 画布
           本体——S4 段淡出至 root 的 cream body。 */
        ctx.getNode('threecanvas').opacity(canvasOpacity);

        /* ── brandMark f(t)（js:471-486）：建立/受压 → pop 回缩(4°) → dock
              (0.97→0.42, x→-1.15, 4°→0)。markYw（js:490）单独一式。3D 场景图
              的 brandMark Group 逐帧吃这批中间量（下方 js:488-491 段）。 ── */
        var markScale, markXw = 0, markRotationDeg = 0;
        if (t >= POP_AT && t < 3.0) {
          var recoil = smooth((t - POP_AT) / 0.2);
          markScale = lerp(1.06, 0.97, recoil);
          markRotationDeg = 4 * recoil;                 // deg(lerp(0, 4, recoil))
        } else if (t >= 3.0) {
          var dock = smooth((t - 3.0) / 0.4);
          markScale = lerp(0.97, 0.42, dock);
          markXw = lerp(0, -1.15, dock);
          markRotationDeg = 4 * (1 - dock);             // deg(lerp(4, 0, dock))
        } else {
          var markEst = lerp(0.92, 1, smooth(t / 0.6));
          markScale = t < 1.8 ? markEst : lerp(1, 1.06, pressure);
        }
        var markYw = lerp(0.02, 0.14, pressure);        // js:490

        /* ── prismMesh 自身旋转 f(t)（js:496-508）：settle 五次幂缓动 +
              t≥3.4 的 world-Y 自旋。PRISM_RX0/RY0（js:505-506）是
              rotation.x/y 的源常量基值。 ── */
        var settleP = Math.min(t, 2.4) / 2.4;
        var settle = 1 - Math.pow(1 - settleP, 5);
        var PRISM_RX0 = 0.139 * Math.PI, PRISM_RY0 = -1.17 * Math.PI;  // js:505-506
        var prismRx = PRISM_RX0 + (1 - settle) * 0.22 * Math.PI;
        var prismRy = PRISM_RY0 - (1 - settle) * 6 * Math.PI;
        if (t >= 3.4) {
          prismRx = PRISM_RX0;
          prismRy = PRISM_RY0 + 0.55 * (Math.min(t, 5.4) - 3.4);
        }

        /* ══ threejs-like 3D 场景（真重建路线 ①-⑥ 全段）。场景图与
              js/liquid-pop-v2.js 的模块级构建一一对应；每帧更新段对应
              renderAt()。几何/相机/时序数值全部引自参考源码常量（括号内为
              js 行号）。 ══ */
        var scene3d = new T3.Scene();                                   // js:30
        /* ⑥b 环境 IBL（js:37/42-46/48-50）：equirect 庭园 JPEG →
           EquirectangularReflectionMapping + SRGBColorSpace → scene.environment。
           引擎侧加载期一次性 PMREM 预滤波（PMREMGenerator.fromEquirectangular
           对应物，CPU 卷积）+ cubeUV 采样进 fs_physical 间接光；BitmapTexture
           的 mapping/colorSpace 由 Renderer.render 校验（fail-loud shim）。 */
        var gardenEnvironment3d = new T3.BitmapTexture('assets/equirect-garden.jpg'); // js:37/43
        gardenEnvironment3d.mapping = T3.EquirectangularReflectionMapping;   // js:48
        gardenEnvironment3d.colorSpace = T3.SRGBColorSpace;                  // js:49
        scene3d.environment = gardenEnvironment3d;                           // js:50
        var camera3d = new T3.PerspectiveCamera(35, 1920 / 1080, 0.1, 100); // js:31
        camera3d.position.set(0, 0, 7);                                 // js:32 初值
        camera3d.lookAt(0, 0, 0);                                       // js:33
        /* 背景 ShaderMaterial（js:62-97 GLSL 逐字；varying/gl_FragColor 的
           WebGL1 方言由 three_api.js 的 adaptShaderMaterial 改写成 450 core——
           WebGLProgram 前缀拼接的对应物）；uFade 逐帧 = 1-fade（js:527）。
           renderOrder -10（js:100）在引擎侧为真排序键（三列表分段绘制序，
           opaque 段 renderOrder 升序 → bg 最先）。depthWrite/depthTest false
           （js:63-65）仍为逐材质深度态偏差（引擎尚无逐材质深度态，handoff
           §26）：等价性依据 = bg 深度最远（z=-3.5），opaque 段内无论 bg 与
           更近物的绘制先后，深度测试都让更近物覆盖 bg —— 逐像素同果。 */
        var backgroundMaterial3d = new T3.ShaderMaterial({              // js:62
          depthWrite: false,                                            // js:63（等价实现，见上）
          depthTest: false,                                             // js:64（等价实现，见上）
          /* js:65 toneMapped:false = 文档性声明：three.js 不向 ShaderMaterial
             注入 tonemapping/colorspace chunk（WebGLProgram.js:831-839 只对内建
             材质拼 chunk），引擎 ShaderMaterial 通道恒 raw 输出——⑥a 输出变换
             （js:18-20）只作用于 Unlit/Lambert/Physical 内建分支。 */
          toneMapped: false,
          uniforms: { uFade: { value: 1 } },                            // js:66
          vertexShader: `                                               // js:72-78
            varying vec2 vUv;
            void main() {
              vUv = uv;
              gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
            }
          `,
          fragmentShader: `                                             // js:79-96
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
        var bgPlane3d = new T3.Mesh(                                    // js:98
          new T3.PlaneGeometry(15, 8.5), backgroundMaterial3d);
        bgPlane3d.position.set(0, 0, -3.5);                             // js:99
        bgPlane3d.renderOrder = -10;                                    // js:100
        scene3d.add(bgPlane3d);                                         // js:101
        var hemiLight3d = new T3.HemisphereLight(0xffffff, 0x7eb6a4, 2.1); // js:103
        scene3d.add(hemiLight3d);
        var keyLight3d = new T3.DirectionalLight(0xffffff, 3.4);        // js:104-105
        keyLight3d.position.set(-4, 5, 6);                              // js:106
        scene3d.add(keyLight3d);
        var rimLight3d = new T3.DirectionalLight(0xc596ff, 2.2);        // js:107-108
        rimLight3d.position.set(5, -2, 3);                              // js:109
        scene3d.add(rimLight3d);
        var heroMaterial3d = new T3.MeshPhysicalMaterial({             // js:163-179
          color: 0xffffff,                                              // js:164
          transmission: 1,                                              // js:165
          opacity: 1,                                                   // js:166
          metalness: 0,                                                 // js:167
          roughness: 0,                                                 // js:168
          ior: 1.33,                                                    // js:169
          thickness: 1.5,                                               // js:170
          specularIntensity: 2,                                         // js:171
          clearcoat: 1,                                                 // js:172
          clearcoatRoughness: 0,                                        // js:173
          dispersion: 1,                                                // js:174
          envMapIntensity: 1.2,                                         // js:175（⑥b：IBL，材质级乘数）
          transparent: true,                                            // js:176
          attenuationColor: 0x88ce7f,                                   // js:177
          attenuationDistance: 2,                                       // js:178
          // js:181-209 onBeforeCompile 顶点段：snoise 位移（js:185
          // getDisplacement：snoise(p*1.5 + uTime*0.4 + worldP*0.1)*0.12）。
          // 有效语义 = 只位移位置：meshphysical 顶点 main 的 chunk 顺序
          // （vendored meshphysical.glsl.js:33/37/38/40）使 defaultnormal_vertex
          // 先于 begin_vertex 消费 objectNormal，js:206 的位移法线重赋对
          // vNormal 是死代码——折射/高光用未位移法线，剪影仍波动（r8 实证）。
          noiseDisplacement: { amplitude: 0.12, frequency: 1.5, worldFrequency: 0.1, timeSpeed: 0.4 },
        });
        var heroSphere = new T3.Mesh(                                   // js:217
          new T3.SphereGeometry(1.5, 128, 128),
          heroMaterial3d);
        heroSphere.position.y = 0.1;                                    // js:218
        heroSphere.renderOrder = 4;                                     // js:219（透射段 droplets(3)→hero(4)）
        scene3d.add(heroSphere);

        /* ── prism GLB 经 brandMark Group（threejs-like ③）。参考 js:36/40
              gltfLoader.loadAsync(prismUrl)（GLTFGeometry 相对路径对 XML 文档
              目录解析——GLTFLoader 相对 URL 语义的对应物）；js:336-343 把
              GLB geometry 装进 Mesh、scale 0.72、rotation.order "YXZ"、挂到
              brandMark；js:375-378 brandMark 初始 (0, 0.02, -0.35)。prism
              材质为视频-马赛克面片（参考 ShaderMaterial js:248-331 的通用化：
              主面 COLOR_0≈0 按 COLOR_1 字节 id 选 8 路 facet 视频槽、屏幕空间
              surfaceUv 采样 + frost；非主面取主导通道 corner 色）——常量全部
              引自参考 shader 源码（括号内为 js 行号）。 ── */
        var prismGeo3d = new T3.GLTFGeometry('assets/heygen-prism.glb', 'prism');
        // js:230-241：8 路 VideoTexture（opencat 侧路径 = 参考仓库 assets/
        // 01..08.mp4 的逐字节拷贝，data-start 0 / duration 6 语义下采样合成
        // 时间 t 处的帧）。
        var facetTextures3d = [];
        for (var fi = 0; fi < 8; fi++) {
          facetTextures3d.push(new T3.VideoTexture('assets/0' + (fi + 1) + '.mp4'));
        }
        var prismMaterial3d = new T3.MeshMosaicMaterial({
          // js:267-270 modelFaceIdToVideoId（GLB COLOR_1 字节 id → 视频槽）。
          faceToTexture: [6, 3, 4, 1, 7, 2, 5, 0],
          // js:260-264 kCornerColors。
          cornerColors: [
            [1.000, 0.678, 0.984],
            [0.082, 0.824, 0.000],
            [0.000, 0.765, 1.000],
          ],
          // js:272-285 videoIdToCornerColor 的 (a, invert) 表化：
          // 下标 = (dominant + a) % 3，invert≠0 取 2 − 下标。
          cornerRule: [[2, 0], [1, 0], [1, 0], [2, 0], [2, 1], [1, 1], [1, 1], [2, 1]],
          frost: 1.5,     // js:313 distanceFromCenter² × 1.5
          uvCrop: 0.75,   // js:314 (gl_FragCoord/uResolution − 0.5)/0.75 + 0.5
          videoTextures: facetTextures3d,   // js:245-247 uVideo0..7
        });
        var prismMesh3d = new T3.Mesh(prismGeo3d, prismMaterial3d);     // js:337
        prismMesh3d.scale.setScalar(0.72);                              // js:338
        prismMesh3d.rotation.order = 'YXZ';                             // js:339
        prismMesh3d.renderOrder = 1;                                    // js:340（opaque 段 bg(-10)→prism(1)→wordmark(2)）
        var brandMark3d = new T3.Group();                               // js:366
        brandMark3d.add(prismMesh3d);                                   // js:343
        brandMark3d.position.set(0, 0.02, -0.35);                       // js:375
        brandMark3d.rotation.set(0, 0, 0);                              // js:376
        brandMark3d.scale.set(1, 1, 1);                                 // js:377
        scene3d.add(brandMark3d);                                       // js:378

        /* ── wordmark 贴图面（threejs-like ⑤）。参考 js:377-410：wordmarkPlane
              = PlaneGeometry(finalWordmarkHeight*wordmarkAspect,
              finalWordmarkHeight)，MeshBasicMaterial({map, alphaTest:0.01,
              depthWrite:true, toneMapped:false, side:DoubleSide}) +
              onBeforeCompile 注入擦除 `if (vMapUv.x > uWordmarkReveal) discard;`
              （js:391-401）。isHeygenWebgl 分支常量：finalWordmarkHeight = 0.55
              （js:382）、position (0.35, 0.14, -0.35)（js:407）、renderOrder 2
              （js:408）、visible = time>=3.0（js:494）。贴图 = 官方 wordmark SVG
              的预栅格化位图（assets/wordmark.png，3356×1000 源画布；参考
              自身也是浏览器把 SVG 光栅化进 TextureLoader 纹理，js:42-57）。
              在 opencat 的材质表达：MeshBasicMaterial 的 unlit+map 通道 = 引擎
              Unlit 分支（map_fragment vec4 语义，r7 起含 texel alpha），而
              onBeforeCompile 的脚本注入 GLSL 的通用对应物是 ShaderMaterial
              （§26 桥）——故本面声明 ShaderMaterial+map（USE_MAP 注入 map/
              采样器的对应物，three_api adaptShaderMaterial 注入 set 3
              texture2D+sampler 拆分声明），GLSL 为被注入后 three.js 实际编译
              产物：顶点 = meshbasic 顶点通道的 vMapUv+投影段；片元 =
              map_fragment → alphatest_fragment(0.01, js:386) → js:395 擦除 →
              opaque 输出 → colorspace_fragment（meshbasic.glsl.js:110 恒注入；
              js:388 toneMapped:false → 前一步 tonemapping_fragment 为
              `#if defined(TONE_MAPPING)` 空体；linearToOutputTexel = js:18
              SRGBColorSpace 的 sRGBTransferOETF，colorspace_pars_fragment.glsl.js
              逐字）→ diffuse/opacity 为内建 uniform（js:164 color 0xffffff /
              js:166 opacity 1）。js:389 side:DoubleSide = 关背面剔除；本面恒
              正对相机（+z 面朝视点），引擎 FrontSide 与其逐像素等价（§27.7-4）。 ── */
        var WM_ASPECT = 3356.0 / 1000.0;                // wordmarkAspect（官方 SVG 3356×1000）
        var WM_PLANE_H = 0.55;                          // finalWordmarkHeight（js:382）
        var wordmarkMaterial3d = new T3.ShaderMaterial({                // js:384-401 的编译产物
          depthWrite: true,                                             // js:387
          /* js:388 toneMapped:false → 编译产物无 TONE_MAPPING define
             （WebGLProgram.js:831 只在 parameters.toneMapping ≠ NoToneMapping
             时定义），tonemapping_fragment 为空；下方 GLSL 相应无 ACES 项，
             仅保留恒注入的 colorspace_fragment（WebGLProgram.js:839）。 */
          toneMapped: false,
          map: new T3.BitmapTexture('assets/wordmark.png'),    // js:385（js:42-45/57-60 TextureLoader）
          uniforms: {
            uWordmarkReveal: { value: 0 },                              // js:383/392 wordmarkReveal
            diffuse: { value: [1, 1, 1] },                              // js:164 color 0xffffff（内建 diffuse）
            opacity: { value: 1 },                                      // js:166（内建 opacity）
          },
          vertexShader: `
            varying vec2 vMapUv;
            void main() {
              vMapUv = uv;                                              // USE_MAP：vMapUv = uv（无 transform）
              gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
            }
          `,
          fragmentShader: `
            precision highp float;
            varying vec2 vMapUv;
            uniform float uWordmarkReveal;
            uniform vec3 diffuse;
            uniform float opacity;

            // colorspace_pars_fragment.glsl.js sRGBTransferOETF（内建 program
            // 前缀 linearToOutputTexel 的 SRGB 目标实现，WebGLProgram.js:839）。
            vec4 sRGBTransferOETF( in vec4 value ) {
              return vec4( mix( pow( value.rgb, vec3( 0.41666 ) ) * 1.055 - vec3( 0.055 ), value.rgb * 12.92, vec3( lessThanEqual( value.rgb, vec3( 0.0031308 ) ) ) ), value.a );
            }

            void main() {
              vec4 diffuseColor = vec4(diffuse, opacity);
              diffuseColor *= texture(sampler2D(map, map_smp), vMapUv); // map_fragment
              if (vMapUv.x > uWordmarkReveal) discard;                  // js:395（onBeforeCompile 注入，
                                                                        // 紧跟 map_fragment 之后）
              if (diffuseColor.a < 0.01) discard;                       // alphatest_fragment（js:386）
              gl_FragColor = vec4(diffuseColor.rgb, diffuseColor.a);    // opaque_fragment
              gl_FragColor = sRGBTransferOETF(gl_FragColor);            // colorspace_fragment（meshbasic.glsl.js:110）
            }
          `,
        });
        var wordmarkPlane3d = new T3.Mesh(                              // js:403-406
          new T3.PlaneGeometry(WM_PLANE_H * WM_ASPECT, WM_PLANE_H),
          wordmarkMaterial3d);
        wordmarkPlane3d.position.set(0.35, 0.14, -0.35);                // js:407（isHeygenWebgl）
        wordmarkPlane3d.renderOrder = 2;                                // js:408
        wordmarkPlane3d.visible = false;                                // js:409（初值；js:494 逐帧）
        scene3d.add(wordmarkPlane3d);                                   // js:410

        /* ── droplets 3D（threejs-like ⑤）。参考 js:412-434：15 颗
              SphereGeometry(radius, 32, 32)（radius = 0.1+0.025*(i%5)，5 档），
              共享 dropletMaterial = makeLiquidMaterial()（js:216——与 hero 同一
              构造、不同实例），137.508° 黄金角螺线布点（js:413-420），
              renderOrder 3（js:431），userData {base, direction, delay, travel}
              （js:425-430）。透射列表 + src-alpha 混合（three.js WebGLRenderLists
              push 的 transmissive 分类）；uTime 与 hero 同一 liquidClock 值
              （js:524-526）。参考对 droplet 无 visible 逐帧写——Object3D 默认
              可见（本轮提升：过渡 pin DROPS_WM_3D_PROMOTED 撤除）。 ── */
        var dropletMaterial3d = new T3.MeshPhysicalMaterial({          // js:216 = js:163-179
          color: 0xffffff,                                              // js:164
          transmission: 1,                                              // js:165
          opacity: 1,                                                   // js:166
          metalness: 0,                                                 // js:167
          roughness: 0,                                                 // js:168
          ior: 1.33,                                                    // js:169
          thickness: 1.5,                                               // js:170
          specularIntensity: 2,                                         // js:171
          clearcoat: 1,                                                 // js:172
          clearcoatRoughness: 0,                                        // js:173
          dispersion: 1,                                                // js:174
          envMapIntensity: 1.2,                                         // js:175（⑥b：IBL，材质级乘数）
          transparent: true,                                            // js:176
          attenuationColor: 0x88ce7f,                                   // js:177
          attenuationDistance: 2,                                       // js:178
          noiseDisplacement: { amplitude: 0.12, frequency: 1.5, worldFrequency: 0.1, timeSpeed: 0.4 }, // js:181-209（材质无关段；只位移位置，见 hero 处 chunk 顺序注）
        });
        var droplets3d = [];                                            // js:412
        var goldenAngle3d = (137.508 * Math.PI) / 180;                  // js:413 degToRad(137.508)
        for (var dci = 0; dci < 15; dci += 1) {                         // js:414
          var dcRadius = 0.1 + 0.025 * (dci % 5);                       // js:415
          var dcAngle = (12 * Math.PI) / 180 + dci * goldenAngle3d;     // js:416
          var dcRing = 1.72 + 0.16 * (dci % 4);                         // js:417
          var dcY = Math.sin(dcAngle * 1.37) * (1.2 + 0.07 * (dci % 3)); // js:418
          var dcX = Math.cos(dcAngle) * dcRing;                         // js:419
          var dcZ = -0.15 + 0.16 * ((dci % 5) - 2);                     // js:420
          var dcBase = new T3.Vector3(dcX, dcY + 0.1, dcZ);             // js:421
          var dcDir = new T3.Vector3(dcX, dcY * 0.88, 0.08 * ((dci % 3) - 1)).normalize(); // js:422
          var dcMesh = new T3.Mesh(                                     // js:423
            new T3.SphereGeometry(dcRadius, 32, 32), dropletMaterial3d);
          dcMesh.position.copy(dcBase);                                 // js:424
          dcMesh.userData = {                                           // js:425-430
            base: dcBase,
            direction: dcDir,
            delay: (dci % 4) / 30,
            travel: 1.65 + 0.18 * (dci % 4),
          };
          dcMesh.renderOrder = 3;                                       // js:431
          droplets3d.push(dcMesh);                                      // js:432
          scene3d.add(dcMesh);                                          // js:433
        }
        var renderer3d = new T3.Renderer('threecanvas', 1920, 1080);    // js:17-24
        /* ⑥a 输出变换（js:18-20，参考在 renderer 构造后立即声明的三个设置）：
           内建材质（Unlit/Lambert/Physical 分支）主 pass 输出链 = tonemapping
           （ACESFilmic × exposure，tonemapping_pars_fragment.glsl.js 逐字移植）
           → colorspace（sRGBTransferOETF）；ShaderMaterial 恒 raw；透射 RT
           二遍恒 NoToneMapping+Linear（WebGLRenderer.js:1921-1922/:2197）。 */
        renderer3d.outputColorSpace = T3.SRGBColorSpace;                // js:18 THREE.SRGBColorSpace
        renderer3d.toneMapping = T3.ACESFilmicToneMapping;              // js:19 THREE.ACESFilmicToneMapping
        renderer3d.toneMappingExposure = 1.08;                          // js:20

        /* renderAt() 每帧段（js:459-467）：相机推近 + hero 建立期缩放/挤压/
           爆裂。heroX/heroY 在参考里已含 heroBaseScale 乘积（js:462-463），
           此处因子分解为 heroBase·lerp(...)，乘积同值。 */
        var camZ = lerp(6.55, 7.0, establish);          // js:459
        var heroBase = lerp(1.08, 1.0, establish);      // js:461
        var heroX = lerp(1.0, 1.1, pressure);           // js:462（× heroBase 合成于 scale）
        var heroY = lerp(1.0, 0.78, pressure);          // js:463（× heroBase 合成于 scale）
        var heroPopScale = lerp(1.0, 0.02, rupture);    // js:464
        camera3d.position.z = camZ;                     // js:459 lerp(6.55,7,establish)
        camera3d.lookAt(0, 0, 0);                       // js:460
        heroSphere.scale.set(                           // js:465 hero.scale.set(...)
          heroBase * heroX * heroPopScale,
          heroBase * heroY * heroPopScale,
          heroBase * heroPopScale);
        var shaderTime = liquidClock(t);                // js:524（uTime 时钟，含 pop 后 2.4→3.2 段）
        heroMaterial3d.noiseTime = shaderTime;          // js:525
        /* js:527 uFade = 1-fade（提升提交：§26.3 过渡 pin BG_3D_PROMOTED 撤除，
           3D 背景场直接承担可见场；#bgcanvas 与 2D BGFIELD 同提交退役）。 */
        backgroundMaterial3d.uniforms.uFade.value = 1 - fade;           // js:527
        heroSphere.visible = t < 3.02;                  // js:467
        /* brandMark 逐帧段（js:488-491）：scale/position.x/position.y/rotation.z
           吃上方共享的同一批 f(t) 中间量（markScale/markXw/markYw/
           markRotationDeg）。rotation.z 参考存弧度（js:475 degToRad）。 */
        brandMark3d.scale.setScalar(markScale);                         // js:488
        brandMark3d.position.x = markXw;                                // js:489
        brandMark3d.position.y = markYw;                                // js:490
        brandMark3d.rotation.set(0, 0, markRotationDeg * Math.PI / 180); // js:491
        prismMesh3d.rotation.x = prismRx;                               // js:499-500, 504-505
        prismMesh3d.rotation.y = prismRy;                               // js:501-503, 506-507
        prismMesh3d.rotation.z = 0;                                     // js:508
        /* wordmark 逐帧段（js:492-494）：lockupReveal = smooth((time-3.0)/0.4)
           驱动左→右擦除（js:395 discard），visible 翻转在 t≥3.0（提升提交：
           §27.3 过渡 pin DROPS_WM_3D_PROMOTED 撤除）。 */
        var reveal = smooth((t - 3.0) / 0.4);           // js:492 lockupReveal
        wordmarkMaterial3d.uniforms.uWordmarkReveal.value = reveal;     // js:493
        wordmarkPlane3d.visible = t >= 3.0;                             // js:494
        /* droplets 逐帧段（js:510-522）：POP 前贴 base + 建立期缩放；POP 后
           power3Out 弹射 + settle 回落（js:518 settle 每滴同式同值）+ lerp
           收缩。 */
        var dropSettle = smooth((t - 3.6) / 0.8);       // js:518 settle
        for (var di = 0; di < droplets3d.length; di += 1) {             // js:510
          var dMesh = droplets3d[di];
          if (t < POP_AT + dMesh.userData.delay) {                      // js:512
            dMesh.position.copy(dMesh.userData.base);                   // js:513
            dMesh.scale.setScalar(lerp(0.62, 1, smooth((t - 0.6) / 1.2))); // js:514
          } else {
            var dLaunch = power3Out((t - POP_AT - dMesh.userData.delay) / 0.8); // js:517
            dMesh.position.copy(dMesh.userData.base)
              .addScaledVector(dMesh.userData.direction,               // js:519
                dMesh.userData.travel * dLaunch * (1 - 0.04 * dropSettle));
            dMesh.position.y -= 0.3 * dLaunch * dLaunch + 0.1 * dropSettle;   // js:520
            dMesh.scale.setScalar(lerp(1, 0.76, dLaunch));              // js:521
          }
        }
        dropletMaterial3d.noiseTime = shaderTime;       // js:526 uTime（与 hero 同一值）
        renderer3d.render(scene3d, camera3d);           // js:540
  
  });

  return (
    <div id="root" class="relative w-[1920px] h-[1080px] overflow-hidden bg-[#f4f2ea]">
      <canvas id="threecanvas" ref={canvas} class="absolute left-0 top-0 w-[1920px] h-[1080px]" />
    </div>
  );
}
