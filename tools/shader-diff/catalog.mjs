// catalog.mjs — 从 shaders 库 GPU-free 提取组件目录 → catalog.json。
//
// 每个条目：name / category / role / props 默认值与 ui 范围 / 是否可参与
// 对拍（exclude + reason）。filter 类组件默认给一个 LinearGradient child
// （空间变化、无外部资产、确定性），对齐 filter 数学；media 类里
// ImageTexture 用生成 PNG 的 data URL，其余（camera/video/DOM 采集）排除。
//
// 用法：node catalog.mjs [--all]   （默认全量；--all 仅为显式声明）
// 输出：catalog.json

import { writeFileSync } from 'node:fs'
// getAllShaders 在 ./registry 出口（index.js 不再导出）。
import { getAllShaders } from '/home/solaren/Projects/shaders/packages/core/dist/registry.js'

// 与 page/oracle-main.mjs 的 makeChildPreset 保持一致：filter 的默认 child。
const CHILD_PRESET = {
  type: 'LinearGradient',
  props: { colorA: '#312e81', colorB: '#f59e0b', angle: 35 },
}

// 角色判定：GpuShaderDefinition 不带 role，defineStd 的信息在降级后由
// requiresChild/ naturalSizeKey 近似回推（仅用于报告分组，不进 gate）。
function roleOf(def) {
  if (def.requiresChild) return 'filter'
  if (def.naturalSizeKey) return 'media'
  if (def.compute) return 'generator+compute'
  return 'generator'
}

// v1 排除表：需要对拍但没有确定性/无外部输入来源的组件。
const EXCLUDE = {
  WebcamTexture: '需要摄像头输入',
  VideoTexture: '需要视频源（可后续给测试 mp4）',
  HTMLInCanvas: '需要 DOM 子树采集（captureDOM 路径）',
  Text: '需要字体光栅化环境',
  Group: '纯结构容器（无自身像素）',
}

// 默认 child：所有 requiresChild 组件挂在它下面。
function entryFor(def) {
  const props = {}
  for (const [k, cfg] of Object.entries(def.props ?? {})) {
    if (cfg && 'default' in cfg) props[k] = cfg.default
  }
  const e = {
    name: def.name,
    category: def.category ?? '',
    role: roleOf(def),
    props,
    preset: { type: def.name, props },
  }
  if (def.requiresChild) {
    e.preset.children = [structuredClone(CHILD_PRESET)]
    e.childPreset = CHILD_PRESET
  }
  return e
}

const all = getAllShaders()
const entries = []
const excluded = []
for (const { definition } of all) {
  const reason = EXCLUDE[definition.name]
  if (reason) {
    excluded.push({ name: definition.name, reason })
    continue
  }
  entries.push(entryFor(definition))
}
entries.sort((a, b) => a.name.localeCompare(b.name))

const catalog = {
  generatedAt: new Date().toISOString(),
  libRepo: '/home/solaren/Projects/shaders',
  canvas: { width: 512, height: 512 },
  fps: 30,
  sampleFrames: [10, 45, 90], // t = 0.3333 / 1.5 / 3.0 s
  childPreset: CHILD_PRESET,
  components: entries,
  excluded,
}
writeFileSync(new URL('./catalog.json', import.meta.url), JSON.stringify(catalog, null, 2))
console.log(
  `catalog: ${entries.length} components ` +
  `(${entries.filter((e) => e.role === 'filter').length} filter, ` +
  `${entries.filter((e) => e.role === 'media').length} media), ` +
  `${excluded.length} excluded`,
)
