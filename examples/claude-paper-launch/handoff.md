# handoff — claude-paper-launch

> 2026-10-10 对齐工作流收工时整理。本示例在盘点阶段被跳过，未进入对齐。

## 状态：跳过（缺参考视频，无法做 k3diff 对齐）
- `F:/hyperframes-launches/claude-paper-launch/renders/` 为空（2026-10-08 建目录后无输出），没有 _swgl 参考渲染
- 组合重依赖音频：87 个 audio 标签复用根目录 3 个 mp3（voiceover / voiceover_explainer + typenew / click / toggle 音效），53.3 秒旁白讲解片

## 若要纳入对齐，先解决参考
1. 在浏览器里渲染参考组合生成基线视频（swgl 渲染管线或等价方式），存到 `renders/` 并用与其它示例一致的命名 `<name>_YYYY-MM-DD_swgl.mp4`
2. 之后按标准流程移植：入口 `F:/hyperframes-launches/claude-paper-launch/index.html`，53.3 秒 ≈ 1599 帧 @30
3. 音频轨不在 k3diff 像素对比范围内，但对齐时音频标签仍要原样编排（时长/触发点）

## 渲染与对比（在 F:/OpenCat 下，PATH 需含 C:/ffmpeg/bin）
- 直接渲染：`python -c "import os,subprocess,sys; os.environ['PATH']=r'C:/ffmpeg/bin'+os.pathsep+os.environ.get('PATH',''); sys.exit(subprocess.call(['target/release/opencat.exe']+sys.argv[1:]))" examples/claude-paper-launch/claude-paper-launch.jsx out/claude-paper-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/claude-paper-launch.mp4 --out out/k3diff-claude-paper-launch --dump <最差帧号>`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
