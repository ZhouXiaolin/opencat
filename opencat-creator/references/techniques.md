# 视觉动效技术参考

适用于 OpenCat JSONL + ctx.* API 的 9 种经生产验证的动效技术。每个 composition 至少应使用 2-3 种。

---

## 1. SVG 路径绘制

路径在画面中实时绘制自身。适用于逐步揭示图表、箭头、连接线或品牌标志。

**方法 A — morphSVG 变形：** 使用 `type: "path"` 节点，通过 `ctx.fromTo()` 的 `d` 属性（morphSVG 别名）从收缩路径变形为完整路径。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":36}
{"id":"scene1","parentId":null,"type":"div","className":"w-full h-full bg-[#0a0a0a] flex items-center justify-center","duration":36}
{"id":"draw-path","parentId":"scene1","type":"path","className":"w-[400px] h-[200px] stroke-[#c84f1c] stroke-4 fill-none","d":"M 50 100 L 200 50 L 350 100"}
{"type":"script","parentId":"scene1","src":"ctx.fromTo('draw-path',{d:'M 50 100 L 50 100 L 50 100'},{d:'M 50 100 L 200 50 L 350 100',duration:21,ease:'ease-out'});"}
```

`from` 和 `to` 必须拓扑匹配（相同数量的命令和同开/闭状态）。

**方法 B — CanvasKit 手动绘制：** 使用 `type: "canvas"` 节点，每帧通过 PathEffect.MakeDash 控制绘制进度。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":36}
{"id":"scene1","parentId":null,"type":"div","className":"w-full h-full bg-[#0a0a0a] flex items-center justify-center","duration":36}
{"id":"draw-canvas","parentId":"scene1","type":"canvas","className":"w-full h-full"}
{"type":"script","parentId":"draw-canvas","src":"var CK=ctx.CanvasKit;var canvas=ctx.getCanvas();var progress=Math.min(ctx.frame/21,1);var path=new CK.Path();path.moveTo(50,100);path.lineTo(200,50);path.lineTo(350,100);var paint=new CK.Paint();paint.setStyle(CK.PaintStyle.Stroke);paint.setColor(CK.parseColorString('#c84f1c'));paint.setStrokeWidth(4);paint.setStrokeCap(CK.StrokeCap.Round);var totalLen=280;var drawLen=totalLen*progress;paint.setPathEffect(CK.PathEffect.MakeDash([drawLen,totalLen],0));canvas.clear(CK.Color(10,10,10,1));canvas.drawPath(path,paint);"}
```

`CK.PathEffect.MakeDash` 的 `[drawLen, totalLen]` 参数组合实现了精确的路径绘制进度控制。

---

## 2. Canvas 2D 程序化艺术

动态粒子场、噪声纹理、数据可视化——每帧演变的视觉效果。使用 `type: "canvas"` 节点 + `ctx.CanvasKit` API。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"scene1","parentId":null,"type":"div","className":"w-full h-full","duration":150}
{"id":"proc-canvas","parentId":"scene1","type":"canvas","className":"w-full h-full"}
{"type":"script","parentId":"proc-canvas","src":"var CK=ctx.CanvasKit;var canvas=ctx.getCanvas();var t=ctx.frame/30;function hash(x,y){var n=x*374761393+y*668265263;n=(n^(n>>13))*1274126177;return((n^(n>>16))&0x7fffffff)/0x7fffffff;}var bg=new CK.Paint();bg.setStyle(CK.PaintStyle.Fill);bg.setColor(CK.parseColorString('#0a0a0a'));canvas.drawRect(CK.XYWHRect(0,0,1920,1080),bg);var dot=new CK.Paint();dot.setStyle(CK.PaintStyle.Fill);for(var i=0;i<200;i++){var px=hash(i,0)*1920;var py=hash(i,1)*1080;var b=Math.floor(hash(i,Math.floor(t*10))*255);dot.setColorComponents(1,1,1,b/255);canvas.drawCircle(px,py,2,dot);}"}
```

`hash()` 函数是确定性的——相同帧始终渲染相同结果。

---

## 3. CSS 3D 变换

透视旋转创造深度。适用于产品展示、卡片翻转、建筑等场景。

使用 OpenCat 的 Tailwind className 设置样式，通过 `ctx.to()` 驱动变换：

```jsonl
{"type":"composition","width":390,"height":844,"fps":30,"frames":60}
{"id":"scene1","parentId":null,"type":"div","className":"flex items-center justify-center w-full h-full bg-slate-900","duration":60}
{"id":"card-3d","parentId":"scene1","type":"div","className":"w-[200px] h-[280px] bg-white rounded-xl"}
{"type":"script","parentId":"scene1","src":"ctx.to('card-3d',{rotate:360,scaleX:0.8,scaleY:0.9,duration:36,ease:'ease-in-out'});"}
```

使用 `rotate` 实现绕 Z 轴旋转，`scaleX`/`scaleY` 模拟深度缩放。更复杂的 3D 效果（如真 3D 卡片翻转）可通过 `type: "canvas"` + CanvasKit `canvas.rotate()` / `canvas.concat()` 实现。

---

## 4. 逐词动能排版

单词逐个出现，匹配语音节奏。这是叙事驱动视频的核心技术。

使用 `ctx.splitText()` 将文本拆分为单词单位，结合 `ctx.from()` + `stagger` 实现逐词入场：

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":60}
{"id":"scene1","parentId":null,"type":"div","className":"flex items-center justify-center w-full h-full bg-white","duration":60}
{"id":"headline","parentId":"scene1","type":"text","className":"text-[60px] font-bold text-slate-900","text":"Anything a browser can render"}
{"type":"script","parentId":"scene1","src":"var words=ctx.splitText('headline',{type:'words'});var slides=[80,60,50,25,12];ctx.from(words,{opacity:0,x:function(i){return slides[i]||10;},y:14,duration:12,stagger:4,ease:'ease-out'});"}
```

滑动距离逐词递减（80→12px）——模拟相机逐渐稳定的效果。

结合 `ctx.timeline()` 实现更复杂的编排：

```jsonl
{"type":"script","parentId":"scene1","src":"var tl=ctx.timeline({defaults:{duration:12,ease:'ease-out'}});var words=ctx.splitText('headline',{type:'words'});var slides=[80,60,50,25,12];words.forEach(function(w,i){tl.from(w,{opacity:0,x:slides[i]||10,y:14},i*4);});"}
```

---

## 5. 视频合成

在 composition 中嵌入真实视频素材。使用 OpenCat 的 `type: "video"` JSONL 节点。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":90}
{"id":"scene1","parentId":null,"type":"div","className":"w-full h-full bg-slate-900 flex items-center justify-center","duration":90}
{"id":"video-frame","parentId":"scene1","type":"div","className":"w-[680px] h-[840px] overflow-hidden rounded-2xl"}
{"id":"footage","parentId":"video-frame","type":"video","className":"w-full h-full object-cover","path":"clip.mp4"}
{"type":"script","parentId":"scene1","src":"ctx.fromTo('video-frame',{opacity:0,scale:0.9},{opacity:1,scale:1,duration:9,ease:'ease-out'});"}
```

视频由 OpenCat 运行时自动逐帧寻道和播放。`path` 相对于 JSONL 文件路径解析。

---

## 6. 逐字打字效果

终端打字机效果。使用 `ctx.to()` 的 `text` 属性（grapheme-safe 打字机语义）：

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":60}
{"id":"scene1","parentId":null,"type":"div","className":"flex items-center justify-center w-full h-full bg-[#1e1e2e]","duration":60}
{"id":"prompt","parentId":"scene1","type":"text","className":"text-[32px] text-[#89b4fa]","text":"❯"}
{"id":"typed-text","parentId":"scene1","type":"text","className":"text-[32px] text-[#cdd6f4]","text":""}
{"id":"cursor","parentId":"scene1","type":"div","className":"w-[11px] h-[22px] bg-[#333]"}
{"type":"script","parentId":"scene1","src":"ctx.to('typed-text',{text:'npx hyperframes init',duration:27,ease:'linear'});ctx.to('cursor',{opacity:0,duration:4,repeat:20,yoyo:true,ease:'steps(1)'});"}
```

`text` 属性动画使用 grapheme 安全的分字逻辑，ZWJ emoji 和组合标记不会在簇中间截断。

---

## 7. 速度匹配过渡

前一个场景以某一速度退出，下一个场景以匹配的速度进入——产生连续运动的感知。

**核心规则：** 出口用加速缓动（`'ease-in'`），入口用减速缓动（`'ease-out'`）。两段曲线的最高速度在切点处相遇。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":120}
{"id":"root","parentId":null,"type":"div","className":"relative w-full h-full"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-out","parentId":"main-tl","type":"div","className":"w-full h-full bg-white flex items-center justify-center","duration":30}
{"id":"scene-in","parentId":"main-tl","type":"div","className":"w-full h-full bg-slate-900 flex items-center justify-center","duration":78}
{"type":"transition","parentId":"main-tl","from":"scene-out","to":"scene-in","effect":"fade","duration":12"}
```

Scene-out 脚本——出口加速模糊（最后 10 帧）：

```jsonl
{"type":"script","parentId":"scene-out","src":"var exitStart=ctx.sceneFrames-10;ctx.fromTo('content-out',{y:0,opacity:1},{y:-150,opacity:0,duration:10,delay:exitStart,ease:'ease-in'});"}
```

Scene-in 脚本——入口减速入场（从第 0 帧开始）：

```jsonl
{"type":"script","parentId":"scene-in","src":"ctx.fromTo('content-in',{y:150,opacity:0},{y:0,opacity:1,duration:30,ease:'ease-out'});"}
```

速度在切点处匹配——观众感知到平滑的连续相机运动。

---

## 8. 音频响应动画

从播放音频驱动任何 `ctx.*` 可动画属性。低频驱动标志脉冲、高频驱动发光、振幅驱动背景呼吸。

**使用时机：** 品牌宣传片、产品发布、混剪视频。跳过平静/教程节奏。

**工作原理：** 脚本每帧执行，通过 `ctx.frame` 采样音频频段数据：

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":300}
{"id":"scene1","parentId":null,"type":"div","className":"w-full h-full bg-[#0a0a0a] flex items-center justify-center","duration":300}
{"id":"logo","parentId":"scene1","type":"div","className":"w-[120px] h-[120px] rounded-full bg-[#00C3FF]"}
{"id":"cta","parentId":"scene1","type":"text","className":"text-[48px] font-bold text-white","text":"Launch"}
{"type":"script","parentId":"scene1","src":"var f=ctx.currentFrame;var bass=bassData[f];var treble=trebleData[f];ctx.getNode('logo').scale(1+bass*0.04);ctx.getNode('cta').opacity(0.7+treble*0.3);"}
```

频段数据需嵌入脚本或从外部文件加载。精简模板——纯 procedural 呼吸效果：

```jsonl
{"type":"script","parentId":"scene1","src":"var breathe=0.5+0.5*Math.sin(ctx.frame*0.08);ctx.getNode('logo').scale(1+breathe*0.04);ctx.getNode('cta').opacity(0.7+breathe*0.3);"}
```

保持文本/标志强度细微（≤5% 缩放、≤30% 不透明度变化）——过大读作抖动。

**禁忌：** 均衡器条、频谱分析仪、波形显示、频闪、彩虹色循环。音频提供节奏和强度；视觉词汇仍来自品牌。

---

## 9. 路径动画（Motion Path）

让元素沿任意 SVG 路径运动。适用于曲线滑动、粒子轨迹、引导揭示、环绕动画。

OpenCat 将路径动画内置于 `ctx.to()` / `ctx.from()` / `ctx.fromTo()` 的 `path` 属性中。运行时解析 SVG 路径、缓存测量器，每帧采样位置和旋转。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":120}
{"id":"scene1","parentId":null,"type":"div","className":"w-full h-full bg-[#0a0a0a]","duration":120}
{"id":"dot","parentId":"scene1","type":"div","className":"w-[20px] h-[20px] rounded-full bg-[#2a8a7c]"}
{"type":"script","parentId":"scene1","src":"ctx.to('dot',{path:'M 100 500 C 400 200 800 800 1100 400',orient:0,duration:90,ease:'ease-in-out',repeat:-1,yoyo:true});"}
```

语义：

- `path` 接受 SVG path data 字符串
- 进度 `0 → 1` 映射到从起点到终点的弧长
- 目标接收 `x`、`y`、`rotation` 采样
- `rotation` 跟随切线角度；`orient` 添加常量角度偏移
- 多个 `M` 子路径首尾相连拼接

**沿路径运动 + 入场结合：**

```jsonl
{"type":"script","parentId":"scene1","src":"ctx.fromTo('dot',{opacity:0,scale:0.5},{opacity:1,scale:1,duration:15,ease:'ease-out'});ctx.to('dot',{path:'M 100 500 C 400 200 800 800 1100 400',orient:-90,duration:90,ease:'ease-in-out'});"}
```

元素先淡入缩放入场，然后沿曲线路径飞行。`orient: -90` 使元素朝向运动方向。

---

## 何时使用什么

| 视频能量 | 技术组合 |
|----------|----------|
| 高冲击（发布、宣传） | 逐词排版 + 速度匹配过渡 + 路径动画 |
| 电影感（导览、故事） | SVG 路径绘制 + 视频合成 + 3D 变换 |
| 技术感（开发工具、API） | 逐字打字 + Canvas 2D 程序化 + 路径动画 |
| 高端（奢侈、企业） | Canvas 2D + 3D 变换 + 速度匹配过渡 |
| 数据驱动（统计、指标） | Canvas 2D 程序化 + SVG 路径绘制 |
