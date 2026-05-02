# 合成模式参考

常用 composition 模式：画中画、标题卡、幻灯片、顶层合成。时间单位为帧（fps=30，1s = 30f）。

---

## 画中画（视频嵌入框）

通过父容器控制位置/尺寸，视频填充容器。容器自身无动画属性。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":180}
{"id":"scene","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"pip-frame","parentId":"scene","type":"div","className":"absolute top-0 left-0 w-[1920px] h-[1080px] overflow-hidden z-[50] rounded-none"}
{"id":"el-video","parentId":"pip-frame","type":"video","className":"w-full h-full object-cover","path":"references/talking-head.mp4"}
{"type":"script","parentId":"scene","src":"ctx.timeline()\n  .to('pip-frame', { left:1360, top:700, width:500, height:280, borderRadius:16, duration:30 }, 300)\n  .to('pip-frame', { left:40, duration:18 }, 900);"}
```

说明：
- `pip-frame` 是 `div` 容器，`el-video` 是 `video` 子节点，视频自动填充容器。
- `borderRadius` 由脚本驱动，className 内初始为 0。
- 时间：10s = 300f（position 参数），0.6s = 18f。

---

## 标题卡淡入淡出

使用两个独立补间实现标题淡入、卡片淡出。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"scene","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"title-card","parentId":"scene","type":"div","className":"absolute inset-0 flex items-center justify-center bg-black z-60"}
{"id":"title-text","parentId":"title-card","type":"text","className":"text-white text-center text-[64px]","text":"我的视频标题"}
{"type":"script","parentId":"scene","src":"ctx.timeline()\n  .fromTo('title-text', { opacity:0 }, { opacity:1, duration:18, ease:'ease-out' }, 9)\n  .to('title-card', { opacity:0, duration:15, ease:'ease-in' }, 120);"}
```

说明：
- 标题在 9f 延迟后 18f 淡入。
- 卡片在 120f 处开始 15f 淡出，使用 `ease-in` 退场。
- 初始值由 `ctx.fromTo()` 自动写入；禁止再用 `ctx.set()` 重复设定同一 prop。

---

## 幻灯片（多场景）

使用 JSONL 的 timeline 模式实现多场景幻灯片。每个场景包含独立的标题和内容层，场景间用过渡连接。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":2385}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"slide1","parentId":"main-tl","type":"div","className":"flex flex-col items-center justify-center w-full h-full bg-slate-900","duration":900}
{"id":"title1","parentId":"slide1","type":"text","className":"text-white text-[64px] font-bold","text":"第一节"}
{"id":"slide2","parentId":"main-tl","type":"div","className":"flex flex-col items-center justify-center w-full h-full bg-slate-800","duration":750}
{"id":"title2","parentId":"slide2","type":"text","className":"text-white text-[64px] font-bold","text":"第二节"}
{"id":"slide3","parentId":"main-tl","type":"div","className":"flex flex-col items-center justify-center w-full h-full bg-slate-700","duration":600}
{"id":"title3","parentId":"slide3","type":"text","className":"text-white text-[64px] font-bold","text":"第三节"}
{"type":"script","parentId":"slide1","src":"ctx.fromTo('title1',{opacity:0,y:40},{opacity:1,y:0,duration:18,ease:'ease-out'});"}
{"type":"script","parentId":"slide2","src":"ctx.fromTo('title2',{opacity:0,y:40},{opacity:1,y:0,duration:18,ease:'ease-out'});"}
{"type":"script","parentId":"slide3","src":"ctx.fromTo('title3',{opacity:0,y:40},{opacity:1,y:0,duration:18,ease:'ease-out'});"}
{"type":"transition","parentId":"main-tl","from":"slide1","to":"slide2","effect":"slide","direction":"from_right","duration":45}
{"type":"transition","parentId":"main-tl","from":"slide2","to":"slide3","effect":"fade","duration":30}
```

说明：
- 每节场景有自己的 `duration`（900f、750f、600f）。
- 每个标题通过 `ctx.fromTo()` 入场。
- 场景间用 `transition` 连接。
- 总帧数 = 900 + 750 + 600 + 45 + 30 = 2385。

---

## 顶层合成示例

完整的视频合成，包含视频剪辑、图片、音频、脚本动画。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":1800}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"video-track","parentId":"root","type":"div","className":"absolute inset-0"}
{"id":"clip1","parentId":"video-track","type":"video","className":"w-full h-full object-cover","path":"references/clip-a.mp4"}
{"id":"clip2","parentId":"video-track","type":"video","className":"w-full h-full object-cover","path":"references/clip-b.mp4"}
{"id":"overlay-track","parentId":"root","type":"div","className":"absolute inset-0"}
{"id":"hero-img","parentId":"overlay-track","type":"image","className":"absolute top-[100px] left-[200px] w-[400px] h-[300px] object-cover rounded-lg","query":"mountain landscape"}
{"id":"bgm","parentId":"root","type":"audio","path":"references/background.mp3"}
{"type":"script","parentId":"root","src":"var tl = ctx.timeline();\ntl.fromTo('hero-img', { opacity:0, y:40 }, { opacity:1, y:0, duration:30, ease:'ease-out' }, 150);"}
```

说明：
- `type: "audio"` 的 `parentId` 为 `root`，在整个合成周期播放。
- `clip1` 和 `clip2` 在同一容器内，可由脚本控制可见性。
- 脚本通过 `ctx.timeline()` 创建动画，`hero-img` 在 150f 后入场。
