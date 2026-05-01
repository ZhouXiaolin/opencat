# design/*.md 格式规范

YAML front matter 存机器可读 token，Markdown body 存人类可读设计描述。对齐 [Google DESIGN.md](https://github.com/google-labs-code/design.md) 结构，颜色值使用 Tailwind token 以适配 OpenCat className 体系。

## 文件结构

```markdown
---
version: alpha
name: <kebab-case>
description: <一句话，用于匹配>
mood:
  words: [情绪词]
  temperature: warm | cool | neutral
  energy: calm | moderate | dynamic | intense
keywords: [k1, k2, k3]
colors:
  <role>: <tailwind-token>       # 如 primary: amber-400
typography:
  <level>:
    fontSize: <px>
    fontWeight: <400|500|600|700>
rounded:
  <scale>: <px>                  # 如 sm: 8px  → className rounded-[8px]
spacing:
  <scale>: <px>                  # 如 md: 16px → className gap-[16px]
components:
  <component-name>:
    <property>: <value>           # 支持 {colors.primary} token 引用
composition:
  preferred: [模式]
  depth: [层级]
iconography:
  style: outline | filled | dual   # 可选
  stroke: <px>                      # 可选
motion:
  ease: <opencat预设>
  flow: center-out | bottom-up | left-right | radial
  rhythm: <模式>
  entrance: [模式]
  emphasis: [模式]
timing:
  default-duration: <"Xs">
  entrance-ratio: <"N%">
  breathe-frames: <"N-N">
---

## Overview
## Colors
## Typography
## Composition
## Motion
## Timing
## Do's and Don'ts
```

## Token Schema

### Meta
| 字段 | 类型 | 必填 |
|------|------|------|
| name | string | 是 |
| description | string | 是 |
| keywords | string[] | 是 |

### mood
| 字段 | 类型 | 可选值 |
|------|------|--------|
| words | string[] | — |
| temperature | enum | warm / cool / neutral |
| energy | enum | calm / moderate / dynamic / intense |

### colors
`map<string, string>` — key 为语义角色，value 为 Tailwind 色 token。支持 `{colors.role}` 引用。

推荐角色名：`primary`, `accent`, `accent-secondary`, `success`, `warning`, `surface-primary`, `surface-secondary`, `text-primary`, `text-secondary`, `text-tertiary`, `text-on-primary`, `border`

### typography
`map<string, object>` — key 为语义层级名，value 包含：

| 字段 | 类型 | 示例 |
|------|------|------|
| fontSize | number+px | 16px |
| fontWeight | number | 400 / 500 / 600 / 700 |

推荐层级名：`hero-title`, `section-title`, `card-title`, `label`, `promo`, `meta`, `sub-label`

### rounded / spacing
`map<string, string>` — key 为尺度名，value 为 px 值。OpenCat 转换为 `rounded-[Npx]` / `gap-[Npx]` / `p-[Npx]`。

### composition
| 字段 | 类型 | 可选值 |
|------|------|--------|
| preferred | enum[] | Hero Center / Split Screen / Card Grid / Full Bleed + Overlay / Stack & Reveal |
| depth | string[] | background / atmosphere / stage / decorations；OpenCat 层数定义 |

### iconography
可选。`map<string, string>` — 图标设计决策。

| 字段 | 类型 | 说明 |
|------|------|------|
| style | enum | outline / filled / dual；图标风格 |
| stroke | string | 描边宽度，如 1.5px |

### motion
| 字段 | 类型 | 说明 |
|------|------|------|
| ease | string | opencat.md §5.1 easing preset |
| flow | enum | center-out / bottom-up / left-right / radial; 入场方向 |
| rhythm | string | 节奏模式，→ 连接，如 build-up → hit → settle |
| entrance | enum[] | Stagger Reveal / Focus Pull / Linked Motion / Typewriter |
| emphasis | enum[] | Punch In / Focus Pull |

### timing
| 字段 | 类型 | 说明 |
|------|------|------|
| default-duration | string | 默认总时长，如 6s |
| entrance-ratio | string | 入场占场景比，如 30% |
| breathe-frames | string | 静止帧数范围，如 4-6 |

### components
`map<string, map<string, string>>` — key 为组件名 + 可选 variant 后缀，value 为属性键值对。值支持 token 引用。

```yaml
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.text-on-primary}"
    rounded: "{rounded.md}"
```

## 兼容规则

| 标准 DESIGN.md | 我们的适配 |
|---------------|-----------|
| colors value: hex `"#FFD100"` | Tailwind token `amber-400` |
| typography 含 lineHeight/letterSpacing/fontFamily | 仅 fontSize + fontWeight（系统字体） |
| 无 mood / keywords / composition / motion / timing | 添加为 OpenCat 领域扩展 |

## 完整示例

```markdown
---
version: alpha
name: app-clean
description: App 首页干净现代风格
mood:
  words: [清晰, 亲和]
  temperature: cool
  energy: moderate
keywords: [app, clean, modern]
colors:
  primary: slate-900
  accent: indigo-500
  surface-primary: slate-50
  surface-secondary: white
typography:
  display: {fontSize: 36px, fontWeight: 700}
  title: {fontSize: 20px, fontWeight: 600}
  body: {fontSize: 16px, fontWeight: 500}
rounded:
  sm: 8px
  md: 12px
spacing:
  md: 16px
  lg: 24px
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: white
    rounded: "{rounded.md}"
composition:
  preferred: [Hero Center, Card Grid]
  depth: [background, stage, decorations]
motion:
  ease: spring.gentle
  flow: center-out
  rhythm: steady
  entrance: [Stagger Reveal]
  emphasis: [Focus Pull]
timing:
  default-duration: 4s
  entrance-ratio: 25%
  breathe-frames: 10-14
---
```
