// S6/S8/S9 批量 preset 生成器：registry → presets.mjs 追加段。
// 名单口径（2026-11-02 拍板）：
// - S6 纯 fragment 源效果（非动画、非 filter、非媒体/交互）
// - S8 requiresChild filter（child = LinearGradient 基线底图）
// - S9 Transitions（双 child = LinearGradient + RadialGradient，progress 走默认 0.5）
// 出列：散列命中（WGSL 源 grep 43758.5453/hash33/fract(sin()——S3 内在极限②）
//       Crystal/Goo；动画族（speed/animTime）留 S7；Interactive/媒体/Utilities 不进本线。
// props 一律显式填 registry default（可复现，不依赖 lib 缺省行为）。
// 用法：bun gen-presets-batch.mjs [--emit]（--emit 写 docs/shaders-align/corpus/cases/presets.batch.mjs 片段）
import { shaderRegistry } from './node_modules/shaders/dist/registry.js';
import { CASES } from './cases/presets.mjs';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const tested = new Set();
for (const c of Object.values(CASES)) for (const comp of c.components) tested.add(comp.type);
const coreDir = './node_modules/shaders/dist/core';
const files = readdirSync(coreDir);
const isHashy = (s) => {
    const f = files.find((x) => x.startsWith(s.name + '-'));
    if (!f) return true; // chunk 找不到 = 无法筛查，保守出列
    return /43758\.5453|hash33|fract\(sin\(/.test(readFileSync(join(coreDir, f), 'utf8'));
};
const hasAnim = (s) => {
    const props = Object.keys(s.definition.props || {});
    return props.includes('speed') || JSON.stringify(s.definition).includes('animTime');
};

const defaults = (s) => {
    const out = {};
    for (const [k, v] of Object.entries(s.definition.props || {})) {
        if (v.default !== undefined) out[k] = v.default;
    }
    return out;
};
const childGradient = (id) => ({
    type: 'LinearGradient', id, props: { colorA: '#0f172a', colorB: '#7c3aed', angle: 45 },
});
const childRadial = (id) => ({
    type: 'RadialGradient', id, props: { colorA: '#0ea5e9', colorB: '#f97316', repeat: 2 },
});

const pools = { s6: [], s7: [], s8: [], s9: [], skipped: [] };
// 实测出列表（2026-11-02 批次 S6/S8/S9，goal.md 有定性）：
// - Particles：TypeGPU compute 力场（force.turbulence）→ FMA 收缩分歧内在极限①
// - Ascii：engine glyph atlas 缺口全白（引擎工程债，非协议面）
// - Halftone：亮度→点直径阈值映射放大器（eng 点径系统性大 2 倍，freq 调参不可关断）
// - IsometricCubes：线条 AA 半像素差 × 全屏密集线网（thickness 实测反向，无关闭面）
// - FilmGrain：对拍口径债——web 腿 WebCodecs mp4 vs ref ffmpeg crf0，编码器
//   不同源 × 全屏高频噪声（渲染无分歧，engine 0.0000% 背书）
const manualOut = {
    Particles: 'compute-FMA',
    Ascii: 'engine-glyph-gap',
    Halftone: 'mapping-amplifier',
    IsometricCubes: 'aa-network-amplifier',
    FilmGrain: 'web-mp4-codec-basis',
    // S3 拍板 compute 系出列（PixelSort 本体）+ S6 预筛散列案（Crystal/Goo，
    // 散列在共享 chunk——组件级 grep 不中，实测/人工判）
    PixelSort: 'compute',
    Crystal: 'hash',
    Goo: 'hash',
    // S7/S10 实测出列（2026-11-03）
    Satin: 'naga-wgsl-parse', // engine naga 前端 "failed to convert expression to a concrete type"（Dawn 接受）
    Voronoi: 'hash', // 点位散列（共享 chunk 盲区）
    FloatingParticles: 'hash', // 粒子位置=std drift 散列原语
    Hologram: 'fma-phase', // FMA 时间相位漂移×高频条纹（动画亚型）
    Boids: 'compute-FMA',
    Fog: 'fma-phase',
    ParticleFlow: 'hash',
    MagneticFilings: 'hash',
    VideoTexture: 'external-texture-channel', // registerExternalTexture 三腿 harness/引擎共同缺口
};
for (const s of shaderRegistry) {
    if (tested.has(s.name)) continue;
    // S10 拍板（2026-11-03）：Interactive 归 S7 anim 池——指针值来自
    // frameParams?.pointer ?? (.5,.5)，无输入态三腿缺省一致，无需事件注入；
    // Utilities/媒体类走 presets 手工条目（url/text 特殊 props + 资产面）。
    if (s.category === 'Utilities' ||
        ['ImageTexture', 'VideoTexture', 'WebcamTexture', 'HTMLInCanvas', 'Text'].includes(s.name)) continue;
    if (manualOut[s.name]) { pools.skipped.push(`${s.name}(${manualOut[s.name]})`); continue; }
    if (isHashy(s)) { pools.skipped.push(`${s.name}(hash)`); continue; }
    if (s.category === 'Transitions') { pools.s9.push(s); continue; }
    if (s.requiresChild) { pools.s8.push(s); continue; }
    if (hasAnim(s)) { pools.s7.push(s); continue; }
    pools.s6.push(s);
}

const fmt = (v) => JSON.stringify(v);
const entry = (s, kind, seq) => {
    const id = `${kind}-${s.name.toLowerCase()}`;
    const comp = { type: s.name, id, props: defaults(s) };
    if (kind === 's8') comp.children = [childGradient(id + '-c')];
    if (kind === 's9') comp.children = [childGradient(id + '-c1'), childRadial(id + '-c2')];
    const frames = kind === 's7' ? 30 : 1;
    const anim = kind === 's7';
    return `    ${JSON.stringify(s.name)}: {\n        frames: ${frames},\n        anim: ${anim},\n        components: [${JSON.stringify(comp, null, 8).replace(/\n/g, '\n        ')}],\n    },`;
};

const header = (title) => `    // ── ${title}（props=registry default；散列出列见 gen-presets-batch.mjs）──`;
const sections = [
    header(`批次 S6：纯 fragment 源效果 ${pools.s6.length} 案`),
    ...pools.s6.map((s) => entry(s, 's6')),
    header(`批次 S7：动画族 ${pools.s7.length} 案（30 帧 delta=1/30 推钟；props=registry default）`),
    ...pools.s7.map((s) => entry(s, 's7')),
    header(`批次 S8：filter 批量 ${pools.s8.length} 案（child=LinearGradient 基线底图）`),
    ...pools.s8.map((s) => entry(s, 's8')),
    header(`批次 S9：Transitions ${pools.s9.length} 案（双 child，progress=0.5 中间态）`),
    ...pools.s9.map((s) => entry(s, 's9')),
];
const text = sections.join('\n') + '\n';

console.log(`S6=${pools.s6.length} S7=${pools.s7.length} S8=${pools.s8.length} S9=${pools.s9.length} 排除=${pools.skipped.length}`);
if (process.argv.includes('--emit')) {
    writeFileSync('cases/presets.batch.mjs', text);
    console.log('written: cases/presets.batch.mjs');
} else {
    process.stdout.write(text);
}
