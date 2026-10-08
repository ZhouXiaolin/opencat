/**
 * three_api.js — three.js 风格 3D 场景 API（`ctx.three`）。
 *
 * 心智模型与 three.js 一致：场景图对象跨帧持有，每帧只改属性，末尾
 * `renderer.render(scene, camera)` 把整棵场景图级联成一份 Scene3 快照
 * （`opencat_core::ir::scene3`）经 `__scene3_commit` 全量重提交——与
 * canvas 命令的每帧语义一致。快照里的几何走 realm 级几何注册表
 * （`SphereGeometry` 构造时注册 / `GLTFGeometry` 宿主解码 GLB 注册），
 * 只携带 `Asset { id }`——重网格（128×128 球、prism GLB 11280 顶点）
 * 不逐帧过 JSON 桥；小内建几何（Box/Plane）仍全量内联。
 *
 * ```js
 * const T = ctx.three;
 * const scene = new T.Scene();
 * scene.background = new T.Color(0x101014);
 * const camera = new T.PerspectiveCamera(45, 16 / 9, 0.1, 100);
 * camera.position.set(2.2, 1.8, 3.0);
 * camera.lookAt(0, 0, 0);
 * const light = new T.DirectionalLight(0xffffff, 1.0);
 * light.position.set(-2, -4, -5);            // 光从 position 指向原点
 * scene.add(light);
 * const mesh = new T.Mesh(
 *     new T.BoxGeometry(1, 1, 1),
 *     new T.MeshLambertMaterial({ color: 0xe64a33 }),
 * );
 * scene.add(mesh);
     * const renderer = new T.Renderer('my-canvas-id', 1920, 1080);
 * // 每帧：
 * mesh.rotation.y = ctx.time * 1.2;
 * renderer.render(scene, camera);
 * ```
 *
 * 快照 JSON 字段与 `Scene3` 的 serde 默认命名逐字对应（snake_case、
 * enum 外部标签 `"Inline"`/`"Ambient"`/`"Directional"`、单位变体
 * `"Lambert"`/`"Unlit"`）——改 Scene3 形状时必须同步本文件。
 */
(function () {
    'use strict';

    function num(v, d) {
        const n = Number(v);
        return Number.isFinite(n) ? n : (d === undefined ? 0 : d);
    }

    // ColorManagement.js:205-207 SRGBToLinear 逐字——three.js Color 的
    // 分量恒在线性 working color space（Color.js:30-36：hexadecimal 等
    // SRGB 输入自动转换，ColorManagement.enabled 默认 true，r152+）。
    // 提升轮（threejs-like r10）修正：此前直接回传 sRGB 分量，材质/灯光/
    // attenuation 色全部偏亮偏弱（hero/droplet 的 #88ce7f 体吸收在参考里
    // 读作饱和绿、修正前几乎透明——Beer 系数 −log(color) 对 0.211（线性）
    // vs 0.498（sRGB）差一倍以上）。
    function srgbToLinear(c) {
        return c < 0.04045 ? c * 0.0773993808 : Math.pow(c * 0.9478672986 + 0.0521327014, 2.4);
    }
    // ColorManagement.js:210-212 LinearToSRGB 逐字（背景清屏回编码用）。
    function linearToSrgb(c) {
        return c < 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 0.41666) - 0.055;
    }
    // 0xRRGGBB → 线性 [r, g, b]（0..1），three.js Color.setHex 语义
    // （setHex 默认 sourceColorSpace = SRGBColorSpace → toWorkingColorSpace）。
    function rgbFromHex(hex) {
        const h = Math.max(0, Math.min(0xffffff, Math.round(num(hex, 0xffffff)))) & 0xffffff;
        return [srgbToLinear(((h >> 16) & 255) / 255), srgbToLinear(((h >> 8) & 255) / 255), srgbToLinear((h & 255) / 255)];
    }

    /* ── 列主序 mat4 ──────────────────────────────────────────────
       与 WebGPU/glam/Scene3.matrix 同约定：m[c*4+r]，m[12..15] = 平移。
       级联只发生在本 shim，Rust 渲染器只收世界矩阵。 */

    function matIdentity() {
        return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    }

    function matMultiply(a, b) {
        const out = new Array(16);
        for (let c = 0; c < 4; c++) {
            for (let r = 0; r < 4; r++) {
                out[c * 4 + r] =
                    a[r] * b[c * 4] +
                    a[4 + r] * b[c * 4 + 1] +
                    a[8 + r] * b[c * 4 + 2] +
                    a[12 + r] * b[c * 4 + 3];
            }
        }
        return out;
    }

    function matCompose(p, r, s, order) {
        // three.js Matrix4.makeRotationFromEuler 公式直搬（列主序），保证与
        // 真实 three.js 的层级语义逐位一致。默认 'XYZ'（R = Rx·Ry·Rz）；
        // 'YXZ'（R = Ry·Rx·Rz，参考 js:339 prismMesh.rotation.order = "YXZ"）
        // 由同一定式推出。
        const a = Math.cos(r[0]), b = Math.sin(r[0]);
        const c = Math.cos(r[1]), d = Math.sin(r[1]);
        const e = Math.cos(r[2]), f = Math.sin(r[2]);
        const sx = num(s[0], 1), sy = num(s[1], 1), sz = num(s[2], 1);
        if (order === 'YXZ') {
            // R = Ry·Rx·Rz 展开（与 three.js Matrix4.js:343-357 'YXZ' 分支
            // 逐项核对，te[] 为列主序：te[0..2]=第 0 列）。
            const ce = c * e, cf = c * f, de = d * e, df = d * f;
            return [
                (ce + df * b) * sx, a * f * sx, (cf * b - de) * sx, 0,
                (de * b - cf) * sy, a * e * sy, (df + ce * b) * sy, 0,
                a * d * sz, -b * sz, a * c * sz, 0,
                p[0], p[1], p[2], 1,
            ];
        }
        // R = Rx·Ry·Rz 展开（three.js 默认序）。
        const ae = a * e, af = a * f, be = b * e, bf = b * f;
        return [
            c * e * sx, (af + be * d) * sx, (bf - ae * d) * sx, 0,
            -c * f * sy, (ae - bf * d) * sy, (be + af * d) * sy, 0,
            d * sz, -b * c * sz, a * c * sz, 0,
            p[0], p[1], p[2], 1,
        ];
    }

    /* ── 数学对象 ── */

    class Vector3 {
        constructor(x = 0, y = 0, z = 0) { this._v = [num(x), num(y), num(z)]; }
        get x() { return this._v[0]; } set x(v) { this._v[0] = num(v); }
        get y() { return this._v[1]; } set y(v) { this._v[1] = num(v); }
        get z() { return this._v[2]; } set z(v) { this._v[2] = num(v); }
        set(x, y, z) { this._v = [num(x), num(y), num(z)]; return this; }
        // three.js Vector3.setScalar（参考 js:338 prismMesh.scale.setScalar(0.72)、
        // js:488 brandMark.scale.setScalar(markScaleAtTime)）。
        setScalar(v) { var n = num(v); this._v = [n, n, n]; return this; }
        copy(v) { this._v = [num(v.x), num(v.y), num(v.z)]; return this; }
        // three.js Vector3.addScaledVector（参考 js:519 droplet 位移
        // `position.copy(base).addScaledVector(direction, …)`）。
        addScaledVector(v, s) {
            var k = num(s, 1);
            this._v[0] += num(v.x) * k;
            this._v[1] += num(v.y) * k;
            this._v[2] += num(v.z) * k;
            return this;
        }
        // three.js Vector3.normalize（参考 js:422 droplet direction 构造）。
        normalize() {
            var x = this._v[0], y = this._v[1], z = this._v[2];
            var inv = 1 / (Math.sqrt(x * x + y * y + z * z) || 1);
            this._v = [x * inv, y * inv, z * inv];
            return this;
        }
    }

    // 弧度制（three.js rotation 语义）。order 对齐 three.js Euler（默认
    // 'XYZ'；参考 js:339 prismMesh.rotation.order = "YXZ" 直接赋值即可）。
    class Euler {
        constructor(x = 0, y = 0, z = 0) {
            this._v = [num(x), num(y), num(z)];
            this.order = 'XYZ';
        }
        get x() { return this._v[0]; } set x(v) { this._v[0] = num(v); }
        get y() { return this._v[1]; } set y(v) { this._v[1] = num(v); }
        get z() { return this._v[2]; } set z(v) { this._v[2] = num(v); }
        set(x, y, z) { this._v = [num(x), num(y), num(z)]; return this; }
    }

    class Color {
        constructor(hex = 0xffffff) { this._rgb = rgbFromHex(hex); }
        set(hex) { this._rgb = rgbFromHex(hex); return this; }
    }

    /* ── 场景图 ── */

    class Object3D {
        constructor(kind) {
            this.position = new Vector3();
            this.rotation = new Euler();
            this.scale = new Vector3(1, 1, 1);
            // three.js Object3D.visible 语义：false 时跳过自身及整棵子树
            //（参考 js/liquid-pop-v2.js:467 hero.visible = time < 3.02）。
            this.visible = true;
            // three.js Object3D.renderOrder：渲染列表排序主键（背景面 -10
            // js:100、prism 1 js:340、wordmark 2 js:408、droplets 3 js:431、
            // hero 4 js:219）。渲染器按 renderOrder 升序稳定排序。
            this.renderOrder = 0;
            this.__kind = kind || 'object';
            this.__children = [];
            this.__parent = null;
        }
        add(...objs) {
            for (const o of objs) {
                if (!(o instanceof Object3D)) throw new Error('three: add() 需要 Object3D');
                if (o.__parent) o.__parent.remove(o);
                o.__parent = this;
                this.__children.push(o);
            }
            return this;
        }
        remove(o) {
            const i = this.__children.indexOf(o);
            if (i >= 0) { this.__children.splice(i, 1); o.__parent = null; }
            return this;
        }
    }

    class Scene extends Object3D {
        constructor() {
            super('scene');
            this.background = null; // Color | null（null = 透明清屏）
            // three.js Scene.environment：环境贴图（纹理对象）。render() 时
            // 经 PMREM（渲染器侧）供内建 PBR 材质做 IBL（参考 js:25-26
            // `scene.environment = gardenEnvironment`）。null = 无环境。
            this.environment = null;
        }
    }

    class Group extends Object3D {
        constructor() { super('group'); }
    }

    class Mesh extends Object3D {
        constructor(geometry, material) {
            super('mesh');
            this.geometry = geometry;
            this.material = material;
        }
    }

    class DirectionalLight extends Object3D {
        // three.js 语义：光从 position 指向 target（默认原点），
        // 即传播方向 = -position（世界系）。
        constructor(color = 0xffffff, intensity = 1) {
            super('directional');
            this.color = new Color(color);
            this.intensity = num(intensity, 1);
        }
    }

    // three.js HemisphereLight(skyColor, groundColor, intensity)：无位置语义，
    // 辐照度按世界法线 y 插值（参考 js:103）。
    class HemisphereLight extends Object3D {
        constructor(skyColor = 0xffffff, groundColor = 0x000000, intensity = 1) {
            super('hemisphere');
            this.color = new Color(skyColor);
            this.groundColor = new Color(groundColor);
            this.intensity = num(intensity, 1);
        }
    }

    class AmbientLight extends Object3D {
        constructor(color = 0xffffff, intensity = 1) {
            super('ambient');
            this.color = new Color(color);
            this.intensity = num(intensity, 1);
        }
    }

    class PerspectiveCamera extends Object3D {
        // fov = 垂直视场角（度）；aspect 仅收下保持签名兼容，渲染按输出宽高比。
        constructor(fov = 50, aspect, near = 0.1, far = 2000) {
            super('camera');
            this.fov = num(fov, 50);
            this.near = num(near, 0.1);
            this.far = num(far, 2000);
            this.__target = [0, 0, 0];
        }
        lookAt(x, y, z) {
            if (x && typeof x === 'object') this.__target = [num(x.x), num(x.y), num(x.z)];
            else this.__target = [num(x), num(y), num(z)];
            return this;
        }
    }

    /* ── 几何 / 材质 ── */

    class BoxGeometry {
        constructor(w = 1, h = 1, d = 1) {
            this.__data = boxGeometryData(num(w, 1), num(h, 1), num(d, 1));
        }
    }

    // three.js SphereGeometry 1:1（UV 球，radius/widthSegments/heightSegments，
    // phiStart 0 / phiLength 2π / thetaStart 0 / thetaLength π；极点行不重复
    // 三角化）。参考 js/liquid-pop-v2.js:217 `SphereGeometry(1.5, 128, 128)`。
    // 构造即注册 realm 几何注册表（见 registerInlineGeometry）：128×128 球
    // 16641 顶点不逐帧过 JSON 桥，快照带 `Asset { id }`。
    class SphereGeometry {
        constructor(radius = 1, widthSegments = 32, heightSegments = 16) {
            this.__kind = 'gltf-geometry';
            var r = num(radius, 1);
            var w = Math.max(2, Math.round(num(widthSegments, 32)));
            var h = Math.max(2, Math.round(num(heightSegments, 16)));
            this.__id = registerInlineGeometry(
                'sphere:' + r + ':' + w + ':' + h,
                sphereGeometryData(r, w, h));
        }
    }

    function sphereGeometryData(radius, widthSegments, heightSegments) {
        const positions = [], normals = [], indices = [];
        const grid = [];
        for (let iy = 0; iy <= heightSegments; iy++) {
            const v = iy / heightSegments;
            const row = [];
            for (let ix = 0; ix <= widthSegments; ix++) {
                const u = ix / widthSegments;
                // three.js 顶点公式（SphereGeometry.js 逐字对应）。
                const x = -radius * Math.cos(u * Math.PI * 2) * Math.sin(v * Math.PI);
                const y = radius * Math.cos(v * Math.PI);
                const z = radius * Math.sin(u * Math.PI * 2) * Math.sin(v * Math.PI);
                positions.push([x, y, z]);
                // 球心在原点：法线 = 单位化位置（three.js 同款）。
                const inv = 1 / (Math.sqrt(x * x + y * y + z * z) || 1);
                normals.push([x * inv, y * inv, z * inv]);
                row.push(positions.length - 1);
            }
            grid.push(row);
        }
        for (let iy = 0; iy < heightSegments; iy++) {
            for (let ix = 0; ix < widthSegments; ix++) {
                const a = grid[iy][ix + 1], b = grid[iy][ix];
                const c = grid[iy + 1][ix], d = grid[iy + 1][ix + 1];
                if (iy !== 0) indices.push(a, b, d);             // 顶极点行不重复
                if (iy !== heightSegments - 1) indices.push(b, c, d); // 底极点行不重复
            }
        }
        return { positions, normals, indices };
    }

    // three.js PlaneGeometry 1:1（XY 平面，法线 +z；参考 js:98 背景平面与
    // js:365/403 的 logo/wordmark 载体面）。
    class PlaneGeometry {
        constructor(w = 1, h = 1) {
            this.__data = planeGeometryData(num(w, 1), num(h, 1));
        }
    }

    // GLB 几何（three.js GLTFLoader 的 loadAsync 对应物，参考 js:36/40
    // `gltfLoader.loadAsync(prismUrl)`）：路径一次性交给宿主解码注册
    // （`scene3_geometry_register` binding，Rust 侧 gltf crate 解析
    // glTF-BINARY），快照只携带 `Asset { id }` —— 11280 顶点的 prism GLB
    // 不逐帧过 JSON 桥。同步返回：解码在宿主 native 层完成，无异步边界。
    class GLTFGeometry {
        constructor(path, id) {
            this.__kind = 'gltf-geometry';
            var resolved = resolveAssetPath(String(path));
            var cacheKey = 'glb:' + resolved + ':' + String(id);
            // 脚本逐帧重跑：同 path+id 只解码注册一次（注册表幂等覆盖等价
            // no-op，但磁盘 IO + 解析不应逐帧重付）。
            this.__id = __geometryCache[cacheKey]
                || __scene3_geometry_register(resolved, String(id));
            __geometryCache[cacheKey] = this.__id;
        }
    }

    // three.js GLTFLoader 的相对 URL 对文档 base 解析（参考 js:40
    // `new URL("../assets/heygen-prism.glb", import.meta.url)`）的 opencat
    // 对应物：宿主注入 XML 所在目录（ctx.__documentBase，engine 取
    // loader.base_dir()）。绝对路径/带 scheme 的路径原样通过。
    function resolveAssetPath(p) {
        if (p === '' || p.charAt(0) === '/' || /^[A-Za-z][A-Za-z0-9+.-]*:/.test(p)) return p;
        var base = (typeof ctx !== 'undefined' && ctx.__documentBase) || '';
        return base ? base.replace(/\/+$/, '') + '/' + p : p;
    }

    // realm 级几何缓存：程序化重网格（128×128 球 = 16641 顶点）在构造时
    // 经 `scene3_geometry_register_inline` 一次性注册，快照只带 Asset{id}
    // ——three.js 几何对象跨帧持有语义的对应物，几何体不再逐帧过 JSON 桥。
    // key = 类型+构造参数（构造确定性 ⇒ 数据确定性）；闭包内 var 随 IIFE
    // 存活整个 realm 生命周期。
    var __geometryCache = Object.create(null);

    function registerInlineGeometry(key, data) {
        if (__geometryCache[key]) return __geometryCache[key];
        var id = __scene3_geometry_register_inline(key, data.positions, data.normals, data.indices);
        __geometryCache[key] = id;
        return id;
    }

    function planeGeometryData(w, h) {
        const hw = w / 2, hh = h / 2;
        // 行序 iy=0 在 +y（three.js grid 从上往下）。
        const corners = [
            [-hw, +hh], [ +hw, +hh], // iy=0: ix=0,1
            [-hw, -hh], [ +hw, -hh], // iy=1: ix=0,1
        ];
        const positions = corners.map(([x, y]) => [x, y, 0]);
        const normals = corners.map(() => [0, 0, 1]);
        // three.js PlaneGeometry.js 顶点环序：uv = (ix/gridX, 1 - iy/gridY)
        // —— (0,1) 在左上角；索引 = (a,b,d),(b,c,d)，a=(0,0) b=(0,1)
        // c=(1,1) d=(1,0)。背景 ShaderMaterial 的 field(vUv) 依赖这套
        // UV（参考 js:80-97），背景面片法线 +z 不经背面剔除。
        const uvs = [
            [0, 1], [1, 1], // iy=0: ix=0,1
            [0, 0], [1, 0], // iy=1: ix=0,1
        ];
        const indices = [0, 2, 1, 2, 3, 1];
        return { positions, normals, indices, uvs };
    }

    function boxGeometryData(w, h, d) {
        const cross = (a, b) => [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        const positions = [], normals = [], indices = [];
        for (const n of [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]) {
            const u = Math.abs(n[0]) < 0.9 ? cross(n, [0, 1, 0]) : cross(n, [0, 0, 1]);
            const v = cross(n, u);
            const base = positions.length;
            for (const [su, sv] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
                positions.push([
                    (n[0] + u[0] * su + v[0] * sv) * 0.5 * w,
                    (n[1] + u[1] * su + v[1] * sv) * 0.5 * h,
                    (n[2] + u[2] * su + v[2] * sv) * 0.5 * d,
                ]);
                normals.push([n[0], n[1], n[2]]);
            }
            indices.push(base, base + 1, base + 2, base, base + 2, base + 3);
        }
        return { positions, normals, indices };
    }

    function material(shading) {
        return function (params) {
            params = params || {};
            this.__shading = shading;
            this.color = new Color(params.color === undefined ? 0xffffff : params.color);
            this.opacity = params.opacity === undefined ? 1 : num(params.opacity, 1);
            // three.js Material.transparent：true → 透明渲染列表 + NormalBlending
            //（js:176 makeLiquidMaterial transparent:true；WebGLRenderLists
            // 分类与 WebGLState 混合态由渲染器固化）。transmission>0 的
            // Physical 优先进透射列表（渲染器分类，同 three.js push 顺序）。
            this.__transparent = params.transparent === true;
            // 噪声顶点位移（js:185-207 onBeforeCompile 顶点段的通用化）：
            // 静态参数进构造，逐帧时间用 `material.noiseTime = t`（对应参考
            // js:525-526 的 uniformState.uTime.value 赋值）。引擎侧只位移
            // 位置——meshphysical chunk 顺序（beginnormal/defaultnormal 先于
            // begin_vertex，vendored meshphysical.glsl.js:33/37/40）使
            // js:206 的位移法线重赋对 vNormal 是死代码，见 opencat-three
            // vs() 处注释。
            const nd = params.noiseDisplacement;
            // 字段名与 Scene3 的 serde 命名（snake_case）一致，快照直通。
            this.__displacement = nd ? {
                amplitude: num(nd.amplitude),
                frequency: num(nd.frequency, 1),
                world_frequency: num(nd.worldFrequency),
                time_speed: num(nd.timeSpeed),
            } : null;
            this.noiseTime = params.noiseTime === undefined ? 0 : num(params.noiseTime);
            // 单贴图（three.js MeshBasicMaterial({ map }) / ShaderMaterial map）。
            this.__map = (params.map instanceof VideoTexture || params.map instanceof BitmapTexture)
                ? params.map : null;
            this.__textures = null;
            this.__mosaic = null;
        };
    }

    class MeshLambertMaterial { constructor(params) { material('Lambert').call(this, params); } }
    class MeshBasicMaterial { constructor(params) { material('Unlit').call(this, params); } }

    // three.js MeshPhysicalMaterial 的透射核心（参考 js:163-179
    // makeLiquidMaterial 的通用化；常量全部由脚本声明）：
    // 真二遍场景色折射 + Beer-Lambert 衰减 + 逐通道色散 + GGX/清漆高光
    // + 环境 IBL（env_map_intensity = js:172 envMapIntensity，默认 1 =
    // three.js 材质默认；场景无 env 时 IBL 项为 0——three.js 同值）。
    // transparent/opacity 走 material() 基础字段。
    class MeshPhysicalMaterial {
        constructor(params) {
            params = params || {};
            material('Physical').call(this, params);
            const att = params.attenuationColor === undefined ? null : params.attenuationColor;
            const attRgb = att instanceof Color ? att._rgb : rgbFromHex(att === null || att === undefined ? 0xffffff : att);
            // snake_case 直通 Scene3 serde（快照字段命名铁律，§21.4）。
            this.__physical = {
                ior: params.ior === undefined ? 1.5 : num(params.ior, 1.5),
                thickness: num(params.thickness),
                transmission: params.transmission === undefined ? 0 : num(params.transmission),
                roughness: params.roughness === undefined ? 1 : num(params.roughness, 1),
                metalness: params.metalness === undefined ? 0 : num(params.metalness),
                specular_intensity: params.specularIntensity === undefined ? 1 : num(params.specularIntensity, 1),
                clearcoat: num(params.clearcoat),
                clearcoat_roughness: params.clearcoatRoughness === undefined ? 0 : num(params.clearcoatRoughness),
                dispersion: num(params.dispersion),
                attenuation_color: attRgb,
                // three.js 缺省 attenuationDistance = Infinity（不衰减）；
                // 快照 JSON 无法表达 ±inf（serde f32 拒绝），按 WGSL 侧
                // volumeAttenuation 的 ≥1e29 哨兵语义发 1e30（lib.rs:519
                // 注释的既定契约；r8 前 Infinity 会直接炸快照提交）。
                attenuation_distance:
                    params.attenuationDistance === undefined || !Number.isFinite(params.attenuationDistance)
                        ? 1e30
                        : num(params.attenuationDistance, 1e30),
                env_map_intensity: params.envMapIntensity === undefined ? 1 : num(params.envMapIntensity, 1),
            };
        }
    }

    // three.js VideoTexture(video) 的对应物（参考 js:233-240）：opencat 的
    // 视频像素解码在宿主（引擎 MediaContext），脚本只声明**来源路径**——
    // 采样语义（Linear 过滤、Repeat 寻址、flipY）在渲染器 WGSL 固化，
    // 快照每帧携带 `{ VideoFrame: { path, time } }`（time = renderer.render()
    // 时的合成时间，即「视频元素当前帧」）。
    class VideoTexture {
        constructor(path) {
            this.__kind = 'video-texture';
            this.__path = resolveAssetPath(String(path));
        }
    }

    // three.js TextureLoader.loadAsync(bitmapUrl) 的对应物（参考 js:42-45/57-60
    // wordmark 贴图）：静态位图来源，宿主一次性解码；快照携带
    // `{ Bitmap: { path } }`。SVG 资产（参考 wordmark SVG）由胶水层预栅格化
    // 为 PNG（参考自身也是浏览器光栅化后进纹理）。
    class BitmapTexture {
        constructor(path) {
            this.__kind = 'bitmap-texture';
            this.__path = resolveAssetPath(String(path));
            // three.js Texture 属性：mapping/colorSpace 由脚本赋值（参考
            // js:23-24 `gardenEnvironment.mapping = EquirectangularReflectionMapping;
            // .colorSpace = SRGBColorSpace`）。渲染器按 mapping 决定采样路径：
            // equirect 环境走 PMREM cubeUV；默认 UVMapping 直采。
            this.mapping = UVMapping;
            this.colorSpace = NoColorSpace;
        }
    }

    // 视频-马赛克面片材质（参考 js:248-253 ShaderMaterial 的通用化）：
    // 主面（COLOR_0≈0）按 COLOR_1 字节 id 经 faceToTexture 选槽、屏幕空间
    // UV 采样 + frost；非主面取主导通道 corner 色。全部常量源自脚本。
    class MeshMosaicMaterial {
        constructor(params) {
            params = params || {};
            material('VideoMosaic').call(this, params);
            const texs = params.videoTextures || [];
            this.__textures = texs.map((t) => {
                if (!(t instanceof VideoTexture)) throw new Error('three: MeshMosaicMaterial.videoTextures 需要 VideoTexture');
                return t;
            });
            if (this.__textures.length > 8) throw new Error('three: videoTextures 上限 8 路（WebGL2 采样器预算）');
            const f2t = params.faceToTexture || [];
            const cc = params.cornerColors || [];
            const cr = params.cornerRule || [];
            // snake_case 直通 Scene3 serde（快照字段命名铁律，§21.4）。
            this.__mosaic = {
                face_to_texture: Array.from({ length: 8 }, (_, i) => num(f2t[i], 0) >>> 0),
                corner_colors: Array.from({ length: 3 }, (_, i) => {
                    const c = cc[i] || [1, 1, 1];
                    return [num(c[0]), num(c[1]), num(c[2])];
                }),
                corner_rule: Array.from({ length: 8 }, (_, i) => {
                    const r = cr[i] || [0, 0];
                    return [num(r[0], 0) >>> 0, num(r[1], 0) >>> 0];
                }),
                frost: num(params.frost),
                uv_crop: params.uvCrop === undefined ? 1 : num(params.uvCrop, 1),
            };
        }
    }

    /* ── 脚本着色器（three.js ShaderMaterial，参考 js:62-97）──────────
       adaptShaderMaterial 扮演 three.js WebGLProgram 的前缀拼接：用户的
       WebGL1 风格 GLSL（varying/attribute/gl_FragColor、standalone
       uniform、texture2D）改写成 450 core（naga glsl-in 的输入方言），
       并注入三方 uniform 块与显式 location。引擎侧
       （opencat-three reflect_uniform_block + 方言测试
       reference_background_glsl_compiles_through_naga）按同一条前缀契约
       反射——THREE_BUILTINS_GLSL 与 Rust 侧 canonical 表互为防漂移锚。 */

    // three.js 逐 program 前缀 uniforms 的对应物。vertex/fragment 注入
    // 同一布局（引擎用同一 buffer 绑定喂两 stage）；成员顺序/偏移必须与
    // lib.rs 的 canonical 表逐字一致（proj@0 mv@64 model@128 view@192
    // cameraPosition@256，std140 span 272）。
    const THREE_BUILTINS_GLSL = [
        'layout(std140, set = 0, binding = 0) uniform ThreeBuiltins {',
        '  mat4 projectionMatrix;',
        '  mat4 modelViewMatrix;',
        '  mat4 modelMatrix;',
        '  mat4 viewMatrix;',
        '  vec3 cameraPosition;',
        '};',
    ].join('\n');

    // gl_FragColor 以 gl_ 前缀保留字不可复用 → 统一改名（three.js 里
    // gl_FragColor 也是由 WebGL2 backend 自动改写到 out 变量的）。
    const FRAG_OUTPUT_NAME = 'fragOutput';

    function adaptShaderMaterial(vertexShader, fragmentShader, hasMap) {
        // 去 #version / precision（统一由前缀提供）。
        function strip(src) {
            return String(src)
                .replace(/^[ \t]*#version[^\n]*\n/gm, '')
                .replace(/^[ \t]*precision[ \t]+\w+[ \t]+\w+[ \t]*;[ \t]*\n?/gm, '');
        }
        // standalone uniform（float/vec2/vec3/vec4）收进 UserUniforms 块
        // （set 1）；块 uniform（名字后跟 `{`）不在匹配内，原样保留。
        function collectUniforms(src) {
            const decls = [];
            const out = src.replace(
                /^[ \t]*uniform[ \t]+(float|vec2|vec3|vec4)[ \t]+(\w+)[ \t]*;[ \t]*\n?/gm,
                (_, ty, name) => { decls.push('  ' + ty + ' ' + name + ';'); return ''; });
            return { src: out, decls };
        }
        const vs = strip(vertexShader);
        const fs = strip(fragmentShader);
        const vsU = collectUniforms(vs);
        const fsU = collectUniforms(fs);
        // varying 名单：顶点段声明序 = location 序（GLSL ES 接口匹配契约
        // 的通用化）；片元段按名字对位，缺声明即报错（fail loud）。
        const varyingNames = [];
        const varyingRe = /\bvarying[ \t]+\w+[ \t]+(\w+)[ \t]*;/g;
        let m;
        while ((m = varyingRe.exec(vs))) {
            if (varyingNames.indexOf(m[1]) < 0) varyingNames.push(m[1]);
        }
        function relink(src, dir, locations) {
            let out = src.replace(/\bvarying\b/g, dir);
            out = out.replace(/\battribute\b/g, 'in');
            out = out.replace(/\btexture2D[ \t]*\(/g, 'texture(');
            if (dir === 'in') out = out.replace(/\bgl_FragColor\b/g, FRAG_OUTPUT_NAME);
            for (const [name, loc] of locations) {
                const decl = new RegExp('\\b' + dir + '(\\s+)(\\w+)(\\s+)' + name + '(\\s*;)');
                if (!decl.test(out)) {
                    throw new Error('three: ShaderMaterial ' + (dir === 'out' ? '顶点' : '片元') + '段缺少 varying 声明: ' + name);
                }
                out = out.replace(decl, 'layout(location = ' + loc + ') ' + dir + '$1$2$3' + name + '$4');
            }
            return out;
        }
        const vsLocations = varyingNames.map((name, i) => [name, i]);
        const fsLocations = varyingNames.map((name) => {
            const declRe = new RegExp('\\bvarying[ \\t]+\\w+[ \\t]+' + name + '[ \\t]*;');
            if (!declRe.test(fs)) {
                throw new Error('three: ShaderMaterial 片元段 varying 未在顶点段声明: ' + name);
            }
            return [name, varyingNames.indexOf(name)];
        });
        const vsBody = relink(vsU.src, 'out', vsLocations);
        const fsBody = relink(fsU.src, 'in', fsLocations);
        const userBlock = (decls) => decls.length
            ? 'layout(std140, set = 1, binding = 0) uniform UserUniforms {\n' + decls.join('\n') + '\n};\n'
            : '';
        // map 采样接缝（three.js USE_MAP 注入 `map`/`vMapUv` 的对应物；
        // 契约与引擎 shader_map_seam_splits_texture_and_sampler_at_set3
        // 互为防漂移锚）：naga 30 glsl-in 无组合 sampler2D 类型，注入
        // Vulkan 拆分声明（texture2D + sampler），用户 GLSL 以组合构造
        // `texture(sampler2D(map, map_smp), uv)` 采样。绑定端（渲染器
        // group 3）= 表内 map 位图视图 + Clamp/Linear 采样器（three.js
        // Texture 默认 wrap ClampToEdge、flipY=true 的对应固化）。
        const mapBlock = hasMap
            ? 'layout(set = 3, binding = 0) uniform texture2D map;\n'
            + 'layout(set = 3, binding = 1) uniform sampler map_smp;\n'
            : '';
        // 顶点段：内建 attribute 三件套固定 location 0/1/2（引擎顶点布局
        // pos/normal/uv/color0/color1 的前三位）。
        const vertex = [
            '#version 450 core',
            'precision highp float;',
            THREE_BUILTINS_GLSL,
            'layout(location = 0) in vec3 position;',
            'layout(location = 1) in vec3 normal;',
            'layout(location = 2) in vec2 uv;',
            userBlock(vsU.decls),
            vsBody,
        ].filter(Boolean).join('\n');
        const fragment = [
            '#version 450 core',
            'precision highp float;',
            THREE_BUILTINS_GLSL,
            userBlock(fsU.decls),
            mapBlock,
            'layout(location = 0) out vec4 ' + FRAG_OUTPUT_NAME + ';',
            fsBody,
        ].filter(Boolean).join('\n');
        return { vertex, fragment };
    }

    class ShaderMaterial {
        // three.js ShaderMaterial({ uniforms, vertexShader, fragmentShader,
        // ... })（参考 js:62-71）。uniform 字典逐帧可变（js:527
        // `backgroundMaterial.uniforms.uFade.value = 1 - fade` 的对应物：
        // collectScene 每帧重读本字典）。noiseDisplacement 不支持——引擎
        // 的噪声位移在主管线顶点段，脚本着色器有自己的 vs（fail loud）。
        constructor(params) {
            params = params || {};
            material('Shader').call(this, params);
            if (params.noiseDisplacement) {
                throw new Error('three: ShaderMaterial 不支持 noiseDisplacement（噪声位移属主管线顶点段）');
            }
            if (typeof params.vertexShader !== 'string' || typeof params.fragmentShader !== 'string') {
                throw new Error('three: ShaderMaterial 需要 vertexShader/fragmentShader 字符串');
            }
            // 材质带 map → 适配器注入 set 3 采样声明（three.js USE_MAP 的
            // 对应物）；引擎侧按 map 的有无反射校验双向一致。
            const adapted = adaptShaderMaterial(
                params.vertexShader, params.fragmentShader, this.__map !== null);
            this.uniforms = params.uniforms || {};
            this.__shader = {
                vertex_shader: adapted.vertex,
                fragment_shader: adapted.fragment,
            };
        }
    }

    // uniforms 字典 → 快照载荷（three.js `{ uFade: { value } }` 容器与裸值
    // 都收；number → Float、[2..4] 数组 → VecN、Color → Vec3）。
    function shaderUniformPayload(mat) {
        return Object.keys(mat.uniforms).map((name) => {
            const holder = mat.uniforms[name];
            const value = (holder && typeof holder === 'object' && !Array.isArray(holder) && 'value' in holder)
                ? holder.value : holder;
            if (value instanceof Color) {
                return { name, value: { Vec3: value._rgb.slice() } };
            }
            if (typeof value === 'number' && Number.isFinite(value)) {
                return { name, value: { Float: value } };
            }
            if (Array.isArray(value)) {
                if (value.length >= 2 && value.length <= 4) {
                    const tag = 'Vec' + value.length;
                    return { name, value: { [tag]: value.map(num) } };
                }
            }
            throw new Error('three: 不支持的 ShaderMaterial uniform 值（' + name + '）：支持 number/[2..4] 数组/Color');
        });
    }

    /* ── 快照 ── */

    // 纹理来源 → 快照载荷。VideoFrame 的 time 取当前合成时间（three.js
    // VideoTexture「渲染时读视频元素当前帧」的对应物）；像素不过 JSON 桥，
    // 宿主光栅化时按 (path, time) 解码上传。
    function texturePayload(tex) {
        if (tex instanceof VideoTexture) {
            return { VideoFrame: { path: tex.__path, time: num((typeof ctx !== 'undefined' && ctx.time) || 0) } };
        }
        if (tex instanceof BitmapTexture) {
            return { Bitmap: { path: tex.__path } };
        }
        throw new Error('three: 不支持的纹理来源（支持 VideoTexture/BitmapTexture）');
    }

    function collectScene(scene) {
        const lights = [], meshes = [];
        (function walk(node, parent) {
            if (node.visible === false) return; // three.js：不可见节点剪掉子树
            const world = matMultiply(parent, matCompose(node.position._v, node.rotation._v, node.scale._v, node.rotation.order));
            switch (node.__kind) {
                case 'mesh': {
                    // 两种几何来源：脚本内建（__data → Inline）或注册表
                    // （GLTFGeometry → Asset{id}，重网格不逐帧过 JSON 桥）。
                    const geo = node.geometry && node.geometry.__data;
                    const geoId = node.geometry && node.geometry.__kind === 'gltf-geometry'
                        ? node.geometry.__id
                        : null;
                    if (!geo && !geoId) throw new Error('three: mesh.geometry 缺少可用几何（支持 BoxGeometry/SphereGeometry/PlaneGeometry/GLTFGeometry）');
                    const mat = node.material || {};
                    const rgb = mat.color ? mat.color._rgb : [1, 1, 1];
                    const disp = mat.__displacement
                        ? { ...mat.__displacement, time: num(mat.noiseTime) }
                        : null;
                    meshes.push({
                        geometry: geoId
                            ? { Asset: { id: geoId } }
                            // Inline 几何带 uvs（PlaneGeometry 的 vUv 面是
                            // 背景场输入；无 uv 的几何缺省 []）。
                            : { Inline: { positions: geo.positions, normals: geo.normals, indices: geo.indices, uvs: geo.uvs || [] } },
                        material: {
                            base_color: [rgb[0], rgb[1], rgb[2], num(mat.opacity, 1)],
                            // enum 外部标签：VideoMosaic/Physical/Shader 携带参数载荷（serde 默认命名）。
                            shading: mat.__mosaic ? { VideoMosaic: mat.__mosaic }
                                : mat.__physical ? { Physical: mat.__physical }
                                : mat.__shader ? {
                                    Shader: {
                                        vertex_shader: mat.__shader.vertex_shader,
                                        fragment_shader: mat.__shader.fragment_shader,
                                        uniforms: shaderUniformPayload(mat),
                                    },
                                }
                                : (mat.__shading || 'Unlit'),
                            displacement: disp,
                            map: mat.__map ? texturePayload(mat.__map) : null,
                            textures: mat.__textures ? mat.__textures.map(texturePayload) : [],
                        },
                        matrix: world,
                        // three.js 渲染列表语义（Object3D.renderOrder /
                        // Material.transparent；snake_case 直通 Scene3 serde）。
                        render_order: num(node.renderOrder, 0),
                        transparent: mat.__transparent === true,
                    });
                    break;
                }
                case 'ambient':
                    lights.push({ Ambient: { color: node.color._rgb, intensity: node.intensity } });
                    break;
                case 'hemisphere':
                    lights.push({
                        Hemisphere: {
                            sky_color: node.color._rgb,
                            ground_color: node.groundColor._rgb,
                            intensity: node.intensity,
                        },
                    });
                    break;
                case 'directional': {
                    // 世界平移 = 光位置；传播方向 = -position（target 默认原点）。
                    lights.push({
                        Directional: {
                            direction: [-world[12], -world[13], -world[14]],
                            color: node.color._rgb,
                            intensity: node.intensity,
                        },
                    });
                    break;
                }
            }
            for (const c of node.__children) walk(c, world);
        })(scene, matIdentity());
        return { lights, meshes };
    }

    // 与 canvas_api.js 的 assertCanvasTarget 同源（跨 IIFE 不可复用，保持一致）。
    function assertCanvasTarget(id, apiName) {
        var registry = (typeof ctx !== 'undefined' && ctx.__targetRegistry) || {};
        if (registry.visual && registry.visual[id]) return id;
        if (registry.nonVisual && registry.nonVisual[id]) {
            throw new Error(apiName + ": non-visual id '" + id + "' cannot be targeted");
        }
        throw new Error(apiName + ": unknown id '" + id + "'");
    }

    class Renderer {
        // three.js 对应：WebGLRenderer({ canvas }) + renderer.setSize(W, H)
        //（参考 js/liquid-pop-v2.js:17-24）。目标尺寸随快照提交（Scene3.target），
        // 宿主光栅化按此建 framebuffer —— 脚本层不读布局，尺寸是源常量。
        // 输出变换设置（js:18-20，⑥）：outputColorSpace / toneMapping /
        // toneMappingExposure——属性直挂实例（与 three.js 的 renderer 属性
        // 赋值一致，render() 时随快照提交）。默认值 = three.js WebGLRenderer
        // 构造默认 NoToneMapping / 1.0（WebGLRenderer.js:262/:270）；唯一
        // 记录偏差：outputColorSpace 默认 'Linear'（引擎既有 raw 输出契约，
        // three.js r152+ 默认 'srgb'——参考场景经 js:18 显式声明，行为一致）。
        constructor(targetId, width, height) {
            this.__targetId = assertCanvasTarget(String(targetId), 'three.Renderer');
            this.__width = Math.max(1, Math.round(num(width, 0)));
            this.__height = Math.max(1, Math.round(num(height, 0)));
            this.outputColorSpace = LinearSRGBColorSpace;
            this.toneMapping = NoToneMapping;
            this.toneMappingExposure = 1.0;
        }
        render(scene, camera) {
            if (!scene || scene.__kind !== 'scene') throw new Error('three: render() 需要 Scene');
            if (!camera || camera.__kind !== 'camera') throw new Error('three: render() 需要 PerspectiveCamera');
            if (!this.__width || !this.__height) {
                throw new Error('three: Renderer 需要显式尺寸 new Renderer(id, width, height)');
            }
            const snapshot = collectScene(scene);
            const bg = scene.background && scene.background._rgb;
            if (bg) {
                // WebGLBackground.js setClear（r181）：背景 Color 的线性 working
                // 分量经 getRGB(getUnlitUniformColorSpace(renderer)) 转到输出
                // 色彩空间后写清屏——SRGB 输出下 0x101014 清屏回写
                // 16/255,16/255,20/255（值 round-trip 恒等）。引擎清屏不走
                // 输出变换（LoadOp::Clear 原样写入），故编码在此完成。
                snapshot.clear = this.outputColorSpace === SRGBColorSpace
                    ? [linearToSrgb(bg[0]), linearToSrgb(bg[1]), linearToSrgb(bg[2]), 1]
                    : [bg[0], bg[1], bg[2], 1];
            } else {
                snapshot.clear = [0, 0, 0, 0];
            }
            snapshot.target = [this.__width, this.__height];
            snapshot.camera = {
                position: camera.position._v.slice(),
                target: camera.__target.slice(),
                fov_y_degrees: camera.fov,
                near: camera.near,
                far: camera.far,
            };
            // ⑥b 环境（Scene3 serde Option<Texture3>；js:23-26）。equirect
            // 映射是渲染器 PMREM 路径的前提（WebGLCubeUVMaps.js:31-33 只对
            // equirect/refraction 映射转换）——其它映射 fail loud 而不是
            // 静默当平面贴图采样。色彩空间：参考声明 SRGBColorSpace（js:24）
            // ——PMREM 前按 sRGB EOTF 线性化；非 Srgb 声明同样 fail loud
            // （本渲染器的 PMREM 固定按 sRGB 源实现，静默当线性源会错亮度）。
            if (scene.environment !== null) {
                if (!(scene.environment instanceof BitmapTexture)) {
                    throw new Error('three: scene.environment 需要 BitmapTexture（VideoTexture 环境不支持）');
                }
                if (scene.environment.mapping !== EquirectangularReflectionMapping) {
                    throw new Error('three: scene.environment.mapping 必须是 EquirectangularReflectionMapping（PMREM cubeUV 路径）');
                }
                if (scene.environment.colorSpace !== SRGBColorSpace) {
                    throw new Error('three: scene.environment.colorSpace 必须是 SRGBColorSpace（PMREM 按 sRGB 源线性化）');
                }
                snapshot.environment = texturePayload(scene.environment);
            }
            // ⑥ 输出变换（Scene3 serde 单元变体名拼写；上方常量即变体名）。
            snapshot.tone_mapping = this.toneMapping || NoToneMapping;
            snapshot.tone_mapping_exposure = num(this.toneMappingExposure, 1.0);
            snapshot.output_color_space = this.outputColorSpace || LinearSRGBColorSpace;
            __scene3_commit(this.__targetId, snapshot);
            return this;
        }
    }

    /* ⑥ 渲染器输出变换常量（参考 js/liquid-pop-v2.js:18-20）。值 = Scene3
       serde 单元变体名（快照直通，无转译表）。three.js 侧数字/字符串值：
       NoToneMapping=0 / ACESFilmicToneMapping=4（WebGLRenderer.js:262 区域
       常量）、SRGBColorSpace='srgb' / LinearSRGBColorSpace='linear-srgb'
       —— 仅拼写不同，语义一一对应。 */
    const NoToneMapping = 'None';
    const ACESFilmicToneMapping = 'ACESFilmic';
    const LinearSRGBColorSpace = 'Linear';
    const SRGBColorSpace = 'Srgb';
    const NoColorSpace = '';
    const UVMapping = 'UVMapping';
    // three.js constants.js：EquirectangularReflectionMapping = 302（数字
    // 值；本 shim 用语义拼写——快照不携带 mapping，只作 render() 时
    // environment 的路径校验：equirect 环境经渲染器 PMREM 转 cubeUV）。
    const EquirectangularReflectionMapping = 'EquirectangularReflectionMapping';

    ctx.three = {
        NoToneMapping,
        ACESFilmicToneMapping,
        LinearSRGBColorSpace,
        SRGBColorSpace,
        NoColorSpace,
        UVMapping,
        EquirectangularReflectionMapping,
        Scene,
        Group,
        Mesh,
        PerspectiveCamera,
        DirectionalLight,
        HemisphereLight,
        AmbientLight,
        BoxGeometry,
        SphereGeometry,
        PlaneGeometry,
        GLTFGeometry,
        MeshLambertMaterial,
        MeshBasicMaterial,
        MeshMosaicMaterial,
        MeshPhysicalMaterial,
        ShaderMaterial,
        VideoTexture,
        BitmapTexture,
        Renderer,
        Color,
        Vector3,
        Euler,
    };
})();
