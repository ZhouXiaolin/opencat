# 起手骨架

常用合成的结构骨架。每个只给关键结构，细节按需展开。格式见 `jsx-format.md`，动画接口见 `animations.md`，构图判断见 `design-principles.md`。

默认 1280×720 @ 30fps。改尺寸时同步改 `meta` 和所有 `w-[…]` / `h-[…]`。

这些骨架是起点，不是答案。真实合成的密度、层级和节奏要按 `design-principles.md` 判断，通常比骨架复杂。

---

## 1. 标题卡

单 scene。先写静态 hero frame，再动画到它。

```jsx
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1280, height: 720, fps: 30, duration: 3 };

export default function TitleCard() {
  useGSAP((ctx) => {
    ctx.fromTo('title', { opacity: 0, y: 32 }, { opacity: 1, y: 0, duration: 0.65, ease: 'power3.out' });
    ctx.fromTo('subtitle', { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.5, delay: 0.12, ease: 'power2.out' });
  });

  return (
    <div id="root" class="relative w-[1280px] h-[720px] bg-slate-950 overflow-hidden">
      <div class="absolute left-[120px] top-[240px] flex flex-col gap-[16px]">
        <text id="title" class="text-[96px] font-bold text-white leading-[1]">Hello OpenCat</text>
        <text id="subtitle" class="text-[28px] text-slate-400">Design-driven video synthesis</text>
      </div>
    </div>
  );
}
```

---

## 2. 分区信息卡

左右分区，背景结构线增强精密感。

```jsx
export default function FeaturePanel() {
  useGSAP((ctx) => {
    ctx.fromTo('grid-h', { scaleX: 0 }, { scaleX: 1, duration: 0.4, ease: 'power2.out' });
    ctx.fromTo('label', { opacity: 0, x: -20 }, { opacity: 1, x: 0, duration: 0.5, ease: 'power3.out' });
    ctx.fromTo('headline', { opacity: 0, y: 28, scale: 0.96 }, { opacity: 1, y: 0, scale: 1, duration: 0.7, ease: 'spring.gentle' });
  });

  return (
    <div id="root" class="relative w-[1280px] h-[720px] bg-[#0a0d10] overflow-hidden">
      <div id="grid-h" class="absolute left-[80px] top-[360px] w-[1120px] h-[1px] bg-[#1d2934] origin-left" />
      <div class="absolute left-[80px] top-[100px] flex flex-col gap-[16px]">
        <text id="label" class="text-[14px] font-semibold tracking-[4px] text-[#46f5b5]">FEATURE</text>
        <text id="headline" class="text-[64px] font-bold text-white leading-[1]">Real-time rendering</text>
      </div>
      <div class="absolute right-[80px] top-[100px] w-[480px] h-[520px] border-[1px] border-[#2b3a46] bg-[#111820] p-[32px]">
        <text class="text-[20px] text-slate-400">Stats, code, images…</text>
      </div>
    </div>
  );
}
```

---

## 3. 多场景 Timeline

每个 scene 一个组件，自带 `useGSAP`。**钩子自动绑到它所在的 scene**，所以 `ctx.current.time` 就是本 scene 的局部时间，从 0 开始。不用手抄起点，也不用管 transition 占了多少秒。

```jsx
function Scene01() {
  useGSAP((ctx) => {
    var t = ctx.current.time;
    if (t < 0 || t >= ctx.current.duration) return;   // 只在可见区间内跑
    ctx.fromTo('s1-title', { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' });
  });
  return (
    <div id="scene-01" class="absolute inset-0 bg-[#0b1020]" duration="4">
      <text id="s1-title" class="absolute left-[120px] top-[300px] text-[72px] font-bold text-white">Scene 1</text>
    </div>
  );
}

function Scene02() {
  useGSAP((ctx) => {
    var t = ctx.current.time;
    if (t < 0 || t >= ctx.current.duration) return;
    ctx.fromTo('s2-title', { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' });
  });
  return (
    <div id="scene-02" class="absolute inset-0 bg-[#0b1020]" duration="4">
      <text id="s2-title" class="absolute left-[120px] top-[300px] text-[72px] font-bold text-white">Scene 2</text>
    </div>
  );
}

export default function Root() {
  return (
    <div id="root" class="relative w-[1280px] h-[720px] bg-slate-950 overflow-hidden">
      <tl id="main-tl" class="absolute inset-0">
        <Scene01 />
        <transition from="scene-01" to="scene-02" effect="fade" duration="0.5" timing="ease-in-out" />
        <Scene02 />
      </tl>
    </div>
  );
}
```

`meta.duration` 必须等于 `4 + 0.5 + 4 = 8.5`。它是权威值：写小了会截断最后的 scene，写大了尾部空转。

要跨 scene 对齐（比如两个 scene 的动作落在同一条节拍上），读 `ctx.global.time`。

---

## 4. 数据展示

数字 count up + 视觉承载物。核心是让数字有重量。

```jsx
useGSAP((ctx) => {
  ctx.fromTo('metric-value', { opacity: 0, y: 24 }, { opacity: 1, y: 0, duration: 0.6, ease: 'power3.out' });
  ctx.to('metric-value', { text: '2.4M', duration: 1.4, delay: 0.3, ease: 'power2.out' });
  ctx.fromTo('bar-fill', { scaleX: 0 }, { scaleX: 1, duration: 1.4, delay: 0.3, ease: 'power2.inOut' });
});
```

```jsx
<text id="metric-label" class="absolute left-[140px] top-[180px] text-[18px] font-semibold tracking-[2px] text-sky-400">MONTHLY ACTIVE USERS</text>
<text id="metric-value" class="absolute left-[140px] top-[220px] text-[120px] font-bold text-white leading-[1]">0</text>
<div class="absolute left-[140px] top-[400px] w-[600px] h-[8px] bg-slate-800 rounded-full overflow-hidden">
  <div id="bar-fill" class="w-full h-full bg-sky-400 rounded-full origin-left" />
</div>
```

---

## 5. 组件复用

把复用结构抽成组件，用 props 派生 id。多个实例不撞 id。

```jsx
function Panel({ id, num, title, accent }) {
  return (
    <div id={id} class="relative w-[340px] h-[300px] border-[2px] border-[#2a2a3a] bg-[#12121a] p-[28px]">
      <div id={`${id}-corner`} class={`absolute left-[-1px] top-[-1px] w-[22px] h-[22px] border-l-[3px] border-t-[3px] border-[${accent}]`} />
      <text id={`${id}-num`} class="text-[20px] tracking-[4px] text-slate-500">{num}</text>
      <text id={`${id}-title`} class="mt-[20px] text-[44px] font-bold text-white">{title}</text>
    </div>
  );
}
```

> prop 名别用 `h`（JSX transform 在组件体内生成 `h(...)`，`h` 会遮蔽元素创建函数）。用 `ht` 之类。

---

## 6. Canvas 程序化视觉

粒子 / 噪声 / 网格等背景层。表用模块级常量，保证每帧确定。

```jsx
import { useCanvas, useRef } from 'opencat/runtime';

const P = [];
for (let i = 0; i < 48; i++) {
  P.push({ a: (i * 137.508 * Math.PI) / 180, r: 2 + ((i * 7 + 3) % 5) * 0.5, s: 120 + ((i * 13 + 7) % 80) });
}

function Field() {
  const c = useRef();
  useCanvas(c, (canvas, t) => {
    const CK = canvas.CK;
    canvas.clear(CK.parseColorString('#0b0f14'));
    const p = canvas.paint({ color: [0.29, 0.87, 0.5, 0.35], style: 'fill' });
    for (const q of P) {
      canvas.drawCircle(640 + Math.cos(q.a) * q.s * t, 360 + Math.sin(q.a) * q.s * t, q.r, p);
    }
  });
  return <canvas id="field" ref={c} class="absolute inset-0 w-[1280px] h-[720px]" />;
}
```

---

## 7. 效果内核（`ly.pixel`）

全屏像素效果：扫描线、噪点、色散、溶解。

```jsx
useCanvas(c, (canvas) => {
  canvas.clear();

  const kernel = ly.pixel(
    (uv, u) => {
      const band = step(fract(uv.y / u.period), 0.5);
      const v = 0.5 + 0.5 * band;
      return [v * 0.2, v * 0.9, v * 0.6, 1];
    },
    { uniforms: { period: 'float' } }
  );

  ly.draw(canvas, kernel, {
    rect: [0, 0, 1280, 720],
    uniforms: { period: 6.0 },
  });
});
```

---

## 8. 3D 场景

场景图在 `useThree` 回调里建一次，句柄挂到 canvas 节点上跨帧复用。

```jsx
useThree(c, (T, node, t) => {
  if (!node.__scene) {
    const scene = new T.Scene();
    scene.background = new T.Color(0x0b0f14);
    scene.add(new T.DirectionalLight(0xffffff, 2.2));
    scene.add(new T.HemisphereLight(0x88aaff, 0x080810, 0.8));

    const camera = new T.PerspectiveCamera(45, 16 / 9, 0.1, 100);
    camera.position.set(0, 0, 4);
    camera.lookAt(0, 0, 0);

    const mesh = new T.Mesh(
      new T.SphereGeometry(1.3, 96, 96),
      new T.MeshPhysicalMaterial({ color: 0x3fa9f5, roughness: 0.15, metalness: 0.1 }),
    );
    scene.add(mesh);

    node.__scene = { scene, camera, mesh, renderer: new T.Renderer('scene3', 1280, 720) };
  }
  const s = node.__scene;
  s.mesh.rotation.y = t * 0.5;
  s.renderer.render(s.scene, s.camera);
});
```

---

## 9. 音频

声明池 + 在节点上挂 `audioId`。

```jsx
export const audio = [
  { id: 'bgm', path: 'assets/bgm.mp3' },
  { id: 'whoosh', path: 'assets/whoosh.mp3', duration: 0.4 },
];
```

```jsx
<div id="scene-01" class="absolute inset-0 bg-[#0b1020]" duration="3" audioId="whoosh">…</div>
<tl id="main-tl" class="absolute inset-0" audioId="bgm">…</tl>
```

---

## 反模式

- 每个 scene 都是居中标题 + 副标题
- 同尺寸卡片网格铺满
- 背景空到像没加载
- 紫蓝渐变、cyan-on-dark、渐变文字
- 所有元素从下方 fade up、所有 scene 都 crossfade、所有元素同一个 delay 间隔
- 每帧都重建场景图 / 昂贵的表
