# 三维场景（`useThree`）

`ctx.three` 是 three.js 风格的 3D 场景 API，通过 `useThree` 挂每帧。

心智模型与 three.js 一致：场景图对象跨帧持有，每帧只改属性，末尾 `renderer.render(scene, camera)` 提交整棵场景图。这份文件只列 OpenCat 暴露的对象、钩子接法和限制；three.js 本身的用法不再重复。

---

## 入口

```jsx
import { useThree, useRef } from 'opencat/runtime';

function Prism() {
  const c = useRef();
  useThree(c, (T, node, t) => {
    if (!node.__scene) node.__scene = build(T);   // 场景图建一次，句柄挂 node 上跨帧复用
    const s = node.__scene;
    s.mesh.rotation.y = t * 1.2;
    s.renderer.render(s.scene, s.camera);
  });

  return <canvas id="scene3" ref={c} class="absolute inset-0 w-[1280px] h-[720px]" />;
}
```

`useThree(ref, build)` 的 `build(three, node, t)` 每帧调用：`three` 就是 `ctx.three`，`node` 是 `<canvas>` 元素，`t` 是当前场景内秒。

ref 来自 `useRef()`，必须指向 `<canvas>`。绘制逻辑必须对 `t` 纯。**`ctx.three` 在模块顶层不可读，取 `T` 要在回调内。**

---

## 命名空间

```js
const T = ctx.three;
```

| 分类 | 导出 |
| --- | --- |
| 对象 | `Scene` `Group` `Mesh` |
| 相机 | `PerspectiveCamera` |
| 灯光 | `DirectionalLight` `HemisphereLight` `AmbientLight` |
| 几何 | `BoxGeometry` `SphereGeometry` `PlaneGeometry` `GLTFGeometry` |
| 材质 | `MeshLambertMaterial` `MeshBasicMaterial` `MeshPhysicalMaterial` `MeshMosaicMaterial` `ShaderMaterial` |
| 纹理 | `VideoTexture` `BitmapTexture` |
| 其它 | `Renderer` `Color` `Vector3` `Euler` |
| 常量 | `NoToneMapping` `ACESFilmicToneMapping` `LinearSRGBColorSpace` `SRGBColorSpace` `NoColorSpace` `UVMapping` `EquirectangularReflectionMapping` |

---

## OpenCat 的差异

- `PerspectiveCamera`：`fov` 是垂直视场角（度），`aspect` 只收签名兼容，渲染按输出宽高比。
- `DirectionalLight`：光从 `position` 指向原点。
- `Scene.background` 是 `Color` 或 `null`（`null` = 透明清屏）。
- `Scene.environment` 必须是 `BitmapTexture`，`.mapping` 必须 `EquirectangularReflectionMapping`，`.colorSpace` 必须 `SRGBColorSpace`。违例直接报错。
- `SphereGeometry` 是 UV 球（phiLength 2π / thetaLength π）。`PlaneGeometry` 的 UV `(0,1)` 在左上角。
- `GLTFGeometry(path, id)` 加载 glTF 几何。
- `Renderer` 目标 id 必须是已存在的 `<canvas>` id，必须显式尺寸。`render()` 每帧调用，不提交就没有画面。
- `renderer.outputColorSpace` 默认 `Linear`；要 sRGB 需显式设 `T.SRGBColorSpace`。
- SVG 资产会被预栅格化为 PNG。

---

## 材质

### 公共字段

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `color` | `0xffffff` | hex 输入按 sRGB 转线性工作色空间 |
| `opacity` | `1` | 透明度 |
| `transparent` | `false` | 透明渲染 + NormalBlending |
| `map` | `null` | 单贴图（`VideoTexture` / `BitmapTexture`） |
| `noiseTime` | `0` | 逐帧时间，驱动顶点噪声位移 |

顶点噪声位移：

```js
const mat = new T.MeshLambertMaterial({
  color: 0x88ce7f,
  noiseDisplacement: { amplitude: 0.08, frequency: 2.4 },
});
mat.noiseTime = t;   // 每帧
```

`amplitude` / `frequency` / `worldFrequency` / `timeSpeed`。

`MeshLambertMaterial` 走光照，`MeshBasicMaterial` 是 Unlit，两者除公共字段外无额外参数。

### MeshPhysicalMaterial

折射 + Beer-Lambert 衰减 + 色散 + GGX/清漆高光 + 环境 IBL。

```js
new T.MeshPhysicalMaterial({
  color: 0x3fa9f5,
  transmission: 1.0, ior: 1.33, thickness: 1.2,
  roughness: 0.08, metalness: 0,
  attenuationColor: 0x2a6f97, attenuationDistance: 4.0,
  dispersion: 0.6,
  specularIntensity: 1.0,
  clearcoat: 1.0, clearcoatRoughness: 0.05,
  envMapIntensity: 1.0,
});
```

`attenuationDistance` 省略或非有限值 = 不衰减。

### MeshMosaicMaterial

视频马赛克面片材质。主面按 `faceToTexture` 选槽屏幕空间采样 + frost；非主面取主导通道 corner 色。

```js
new T.MeshMosaicMaterial({
  videoTextures: [new T.VideoTexture('a.mp4'), new T.VideoTexture('b.mp4')],  // 上限 8
  faceToTexture: [0, 1, 0, 1],
  cornerColors: [0x111111, 0x222222],
  cornerRule: [],
  frost: 0.2,
});
```

### ShaderMaterial

```js
new T.ShaderMaterial({
  uniforms: { uFade: { value: 0.5 }, uTint: new T.Color(0xff8800), uGrid: [4, 4] },
  vertexShader: '...',
  fragmentShader: '...',
  map: bitmapTexture,      // 可选
  transparent: true,
});
```

- `vertexShader` / `fragmentShader` 必须是非空字符串
- uniform 值支持 number、2-4 数组、`Color`；`uniforms` 字典逐帧可变
- 不支持 `noiseDisplacement`

---

## 推荐骨架

```js
function build(T) {
  const scene = new T.Scene();
  scene.background = new T.Color(0x0b0f14);
  scene.add(new T.DirectionalLight(0xffffff, 2.2));
  scene.add(new T.HemisphereLight(0x88aaff, 0x080810, 0.8));

  const camera = new T.PerspectiveCamera(45, 16 / 9, 0.1, 100);
  camera.position.set(0, 0, 4);
  camera.lookAt(0, 0, 0);

  const mesh = new T.Mesh(
    new T.SphereGeometry(1.3, 96, 96),
    new T.MeshPhysicalMaterial({
      color: 0x3fa9f5, roughness: 0.15, metalness: 0.1,
      clearcoat: 1.0, clearcoatRoughness: 0.05,
    }),
  );
  scene.add(mesh);

  return { scene, camera, mesh, renderer: new T.Renderer('scene3', 1280, 720) };
}
```

---

## 反模式

- 不要把场景图构造放 `useThree` 回调里每帧都跑——用 `node.__scene` 建一次。
- 不要漏 `renderer.render()`。
- 环境贴图只用 `BitmapTexture` + `EquirectangularReflectionMapping` + `SRGBColorSpace`。
- 顶点位移走 `material.noiseTime = t`，不要每帧重设材质。
- `ctx.three` 在模块顶层不可读，取 `T` 要在回调内。
- 暗背景配暗色球体几乎不可见；补一盏 `HemisphereLight` 或提亮 `scene.background`。
