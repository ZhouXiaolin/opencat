// 双侧同源的唯一事实：ref（Edge 真 WebGPU）与 engine（core 录制 → wgpu）
// 都从这份表取 preset。组件名与 props 与 react 版完全一致（官方跨框架
// 同一 props 面），只是走 shaders/js 的声明式组件数组形态。
//
// frames: 渲染帧数；anim: 每帧推进 delta=1/30（false = 每帧 delta 0 原样
// 重画，静态案）。首帧两侧协议一致：delta 0（ref 循环 i===0 ? 0 : DELTA；
// engine 侧 initialize 落定后的首绘同为 delta 0）。
export const W = 960;
export const H = 540;
export const FPS = 30;

export const CASES = {
    solid: {
        frames: 1,
        anim: false,
        components: [
            { type: 'SolidColor', id: 'c1', props: { color: '#3366cc' } },
        ],
    },
    gradient: {
        frames: 1,
        anim: false,
        components: [
            { type: 'LinearGradient', id: 'g1', props: { colorA: '#0f172a', colorB: '#7c3aed', angle: 45 } },
        ],
    },
    noise: {
        frames: 1,
        anim: false,
        components: [
            { type: 'PerlinNoise', id: 'n1', props: { scale: 2, seed: 42 } },
        ],
    },
    blend: {
        frames: 1,
        anim: false,
        components: [
            { type: 'LinearGradient', id: 'bg', props: { colorA: '#0f172a', colorB: '#7c3aed', angle: 45 } },
            { type: 'RadialGradient', id: 'fg', props: { colorA: '#f59e0b', colorB: '#ef4444cc', blendMode: 'overlay', opacity: 0.8 } },
        ],
    },
    // 动画案：30 帧噪声演化（seed 固定，speed 交由逐帧 delta 驱动）。
    'noise-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'PerlinNoise', id: 'n1', props: { scale: 3, seed: 42, speed: 1 } },
        ],
    },

    // ── 批次 S3：扩语料（compute 系出列——PixelSort/ReactionDiffusion/
    // Glass 等真用 compute 管线的 19 案不进 S3；ReactionDiffusion 单列
    // 为 S4 compute 验收案）──────────────────────────────────────────

    // Shapes 家族：填充 + 描边协议（softness 0 硬边 + 非零描边）。
    star: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Star', id: 's1', props: { color: '#f59e0b', radius: 0.35, sides: 5, innerRatio: 0.45, rotation: 18, strokeThickness: 0.02, strokeColor: '#0f172a' } },
        ],
    },
    // Textures 家族：pattern（硬边几何 + 单元格密度）。
    checkerboard: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Checkerboard', id: 'cb1', props: { colorA: '#1e293b', colorB: '#f1f5f9', cells: 8 } },
        ],
    },
    stripes: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Stripes', id: 'st1', props: { colorA: '#111827', colorB: '#f9fafb', angle: 30, density: 6 } },
        ],
    },
    truchet: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Truchet', id: 'tr1', props: { colorA: '#0b1220', colorB: '#38bdf8', cells: 10, seed: 3, thickness: 1.5 } },
        ],
    },
    // gradient 家族第二/三案：径向 + 角向（repeat > 1 覆盖平铺）。
    'radial-gradient': {
        frames: 1,
        anim: false,
        components: [
            { type: 'RadialGradient', id: 'rg1', props: { colorA: '#0ea5e9', colorB: '#f97316', repeat: 2 } },
        ],
    },
    'conic-gradient': {
        frames: 1,
        anim: false,
        components: [
            { type: 'ConicGradient', id: 'cg1', props: { colorA: '#ff0080', colorB: '#00bfff', repeat: 2, rotation: 45 } },
        ],
    },
    // 噪声家族扩员：Simplex / Worley（octaves 2）/ Voronoi（细胞+边）。
    'simplex-noise': {
        frames: 1,
        anim: false,
        components: [
            { type: 'SimplexNoise', id: 'sn1', props: { scale: 3, seed: 7 } },
        ],
    },
    'worley-noise': {
        frames: 1,
        anim: false,
        components: [
            { type: 'WorleyNoise', id: 'wn1', props: { scale: 6, seed: 3, octaves: 2, jitter: 0 } },
        ],
    },
    // voronoi 出列（2026-11-02）：点位 = fract(sin(大参数)×43758.5453)
    // 数位提取散列，无 jitter 关闭面——跨编译器（Tint/naga 的 mul+add
    // FMA 收缩差 → 1 ulp）经散列放大成全场点重排，内在不可对拍。
    // worley-noise 同散列但保留：jitter:0 关闭该路径（实测 0.0000%）。
    // child 协议案：filter（requiresChild）包 gradient——覆盖 childNode
    // /convertToTexture 子纹理链路（JSON components 的 children 递归面）。
    'invert-child': {
        frames: 1,
        anim: false,
        components: [
            {
                type: 'Invert', id: 'inv',
                children: [
                    { type: 'LinearGradient', id: 'lg', props: { colorA: '#0f172a', colorB: '#f59e0b', angle: 120 } },
                ],
            },
        ],
    },
    // 动画案第二式：Plasma 时基噪声演化（speed 1，30 帧 delta=1/30）。
    'plasma-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'Plasma', id: 'pl1', props: { density: 2, speed: 1, colorA: '#7018be', colorB: '#000000' } },
        ],
    },
    // ── S2 动画时钟全案：全部 speed 消费方（PerlinNoise/SimplexNoise/
    // Plasma/Stripes；Worley/Voronoi 的 animTime 路径=散列出列）+ 长钟案
    // （90 帧累加协议验证）───────────────────────────────────────────
    'simplex-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'SimplexNoise', id: 'sa1', props: { scale: 3, seed: 7, speed: 1 } },
        ],
    },
    'stripes-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'Stripes', id: 'st2', props: { colorA: '#111827', colorB: '#f9fafb', angle: 30, density: 6, speed: 1 } },
        ],
    },
    // 长钟：90 帧 = 3 秒钟累加，验证推钟协议无漂移/无跳步。
    'noise-anim-long': {
        frames: 90,
        anim: true,
        components: [
            { type: 'PerlinNoise', id: 'nl1', props: { scale: 3, seed: 42, speed: 1 } },
        ],
    },
    // worley-anim 出列（2026-11-02）：细胞漂移动画本身走 animTime+sin
    // 抖动路径（同 voronoi 散列），关闭动画=静态图，该案无对拍面。
    // ── S4 compute 验收案：Gray-Scott 反应扩散（TypeGPU compute 管线，
    // 无 child 无交互）。初始态由核过程生成（无 Math.random），种子核
    // 纯 IEEE 单步运算，跨编译器逐位一致。
    //
    // 单帧 + speed 32 的两个约束来源：
    // 1) 双编译器舍入差：迭代核 mul+add 链在 Dawn(Tint) 与 wgpu(naga)
    //    的 FMA 收缩决策不同 → 每次迭代引入末位差，反应扩散生长期对
    //    末位差指数放大（speed 6 × 30 帧实测 avg_p8 19.8%，分歧窗口
    //    集中在成斑期）——帧数必须压到 1；
    // 2) mp4 编码污染：更低速的态 = 稀疏 1px 点 = 4:2:0 色度最劣形态
    //    （speed 2 单帧 mp4 14.6% vs PNG 直对 0.0000%/maxd2），speed 32
    //    让点长成团斑，编码可行。32 次 dispatch 同时覆盖 ping-pong
    //    两个方向的 bind group 交替。
    'reaction-diffusion': {
        frames: 1,
        anim: false,
        components: [
            { type: 'ReactionDiffusion', id: 'rd1', props: { preset: 'coral', speed: 32 } },
        ],
    },
};
