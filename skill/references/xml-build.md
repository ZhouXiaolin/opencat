# XML Build

目标：把 `STORYBOARD.md` 转成一个 OpenCat XML 文件，默认 `index.xml`。

## 构建前读取

- `design.md` — 精确 token
- `expanded-prompt.md` — 场景视觉细节
- `STORYBOARD.md` — beat timing、技法、资产、转场
- `SCRIPT.md` — 仅当有旁白/字幕
- `references/opencat.md` — XML 硬规则
- `references/transitions.md` — 转场参数
- 按需读取专项 references

## XML 架构

### 单场景

```xml
<opencat width="1920" height="1080" fps="30" duration="4">
  <script>
    var tl = ctx.timeline();
    tl.fromTo('title', { opacity: 0, y: 60 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' }, 0.2);
  </script>
  <div id="root" class="relative flex w-full h-full overflow-hidden bg-[#000000]">
    ...
  </div>
</opencat>
```

### 多场景

用一个可视 root 包住 `<tl>`。不要创建多个顶层可视元素。

```xml
<opencat width="1920" height="1080" fps="30" duration="12.6">
  <soundtrack>
    <audio id="music" path="assets/music.mp3" attach="main-tl" duration="12.6" />
  </soundtrack>

  <script>
    var tl = ctx.timeline();
    tl.fromTo('b1-title', { opacity: 0, y: 80 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' }, 0.2);
    tl.fromTo('b2-title', { opacity: 0, x: -80 }, { opacity: 1, x: 0, duration: 0.6, ease: 'expo.out' }, 4.1);
  </script>

  <div id="root" class="relative w-full h-full overflow-hidden bg-[#000000]">
    <tl id="main-tl" class="absolute inset-0">
      <div id="beat-1" class="relative w-full h-full overflow-hidden bg-[#000000]" duration="3.8">
        ...
      </div>
      <transition from="beat-1" to="beat-2" effect="slide" direction="from_right" duration="0.3" timing="ease-out" />
      <div id="beat-2" class="relative w-full h-full overflow-hidden bg-[#000000]" duration="4.0">
        ...
      </div>
      <transition from="beat-2" to="beat-3" effect="fade" duration="0.5" timing="ease-in-out" />
      <div id="beat-3" class="relative w-full h-full overflow-hidden bg-[#000000]" duration="4.0">
        ...
      </div>
    </tl>
  </div>
</opencat>
```

`<opencat duration>` = sum(scene durations) + sum(transition durations)。

## Timing model

OpenCat timeline scenes are sequential. Script times are global seconds in the composition timeline.

For each beat:

- `beatStart = sum(previous scene durations + previous transition durations)`
- Scene entrance starts at `beatStart + 0.1` to `beatStart + 0.3`
- Transition starts after the outgoing scene duration; do not fade outgoing elements before it
- Final scene may fade to black or resolve

Keep a small timing table in comments only when useful; do not over-comment obvious tweens.

## Layout before animation

1. Write the scene as it should look at its most visible moment.
2. Use flex/grid/padding/gap for content layout.
3. Use absolute only for:
   - full-frame layers: `absolute inset-0`
   - corner labels/metadata: explicit `top/left/right/bottom`
   - Canvas-aligned annotations with calculated pixel coordinates
4. Add `ctx.fromTo()` entrances from offscreen/hidden to the CSS position.
5. Add ambient/camera motion on wrappers, not on the same element already using transform entrance when it could conflict.

## Animation rules

- Use one `<script>` directly under `<opencat>`.
- Prefer `ctx.timeline()` and explicit positions.
- Vary ease, duration, direction, and stagger within every scene.
- Avoid using the same transform property on the same element from competing tweens at the same time.
- Do not use CSS animation/transform classes.
- Use seeded randomness if randomness is needed; never rely on wall-clock time.
- For ambient loops, calculate finite duration/repeat behavior or animate over the scene duration.

## Text

- Text lives inside `<text>`; do not nest spans inside `<text>`.
- Use real UTF-8 characters, not `\uXXXX` escapes.
- Use `ctx.splitText()` or patterns from `text-animations.md` for per-word/per-char animation.
- Avoid forced manual line breaks unless the layout intentionally depends on display lines.
- Font sizes should be video-scale: headline 64-120px, body 28-42px, label 18-24px.

## Media and assets

- Use `path` for local assets and `url` for remote assets.
- Asset paths are relative to the XML file's directory.
- `<audio>` must live in `<soundtrack>` and attach to `main-tl` or a scene id.
- `<video>` can have children, but animate a wrapper when changing transform/opacity around media.
- If a captured image is used, it needs motion treatment: perspective, slow zoom, scroll reveal, device frame, parallax, or shader/canvas treatment.

## Captions

Only add `<caption>` or scripted subtitle text when there is real caption data or a written script. Do not create empty caption scaffolding.

## Common failure modes

- Multiple visual roots under `<opencat>`
- `className`, `parentId`, or `style` copied from JSONL/HTML habits
- `<script type="...">` or nested `<script>`
- `<tl>` scenes not adjacent to their `<transition>`
- `<transition>` missing between scenes
- Root `duration` stale after timing edits
- Absolute elements without coordinates
- Tiny web-size text
- Product screenshot pasted full-frame with no authored motion or composition
