use serde_json::json;
use std::collections::HashMap;

use super::{MutationRecorder, TextUnitValues};
use crate::frame_ctx::duration_secs_to_frames;
use crate::script::ScriptTextSource;
use crate::script::mutations::{
    CanvasMutations, NodeStyleMutations, Rotate3Field, StyleMutations, TextUnitGranularity,
    TextUnitOverride, TextUnitOverrideBatch,
};
use crate::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name, flex_direction_from_name,
    inset_shadow_from_name, justify_content_from_name, object_fit_from_name, position_from_name,
    text_align_from_name,
};
use crate::style::{
    AlignItems, BorderStyle, BoxShadow, ClipPath, ColorToken, CssFilter, CssFilterKind, DropShadow,
    FlexDirection, FontWeight, InsetShadow, JustifyContent, NodeStyle, ObjectFit, Position,
    TextAlign, Transform, TransformOrigin, color_token_from_script_string,
};

/// Cached output of one G2 chart build (see `MutationStore::chart_cache`).
#[derive(Clone, Debug)]
pub struct ChartCacheEntry {
    /// The canvas node's DrawOp stream as recorded by the build.
    pub commands: Vec<crate::ir::draw_op::DrawOp>,
    /// Paragraph images the build registered (id + pixels + placement), so a
    /// cache replay can re-register them without re-rasterizing text.
    pub images: Vec<crate::ir::FrameGeneratedImage>,
    /// Fingerprint of `commands` at commit time — the probe comparison key.
    pub fingerprint: u64,
    /// Set when two builds at different probe times produced different ops —
    /// the build reads the clock, and the chart must be rebuilt every frame.
    pub dynamic: bool,
}

#[derive(Default)]
pub struct MutationStore {
    styles: HashMap<String, NodeStyleMutations>,
    canvases: HashMap<String, CanvasMutations>,
    /// three-like shim 每帧重提交的 3D 场景快照（key = 目标 canvas id）。
    scene3s: HashMap<String, crate::ir::scene3::Scene3>,
    /// realm 级 3D 几何注册表（GLB 一次性解码，快照只带 `Asset { id }`）。
    /// NOT cleared by reset_for_frame：几何与资产同寿命（prism GLB
    /// 11280 顶点 / 60096 索引 ≈ 2 MB，注册一次，全片复用）。
    geometries: crate::ir::scene3::GeometryRegistry,
    /// Base-style values keyed by (node_id, property_name), set once before
    /// scripts run. Not cleared during reset_for_frame so read_style_value
    /// always sees Tailwind/className defaults. Stores JSON values (f32, string, etc.)
    initial_styles: HashMap<String, HashMap<String, serde_json::Value>>,
    text_sources: HashMap<String, ScriptTextSource>,
    /// Script-recorded generated images (canvas putImageData / surface
    /// auto-bake path). NOT cleared by reset_for_frame: recorded during
    /// script runs, drained once per frame by the render pipeline into the
    /// pipeline `GeneratedImageTable`. Ids must be deterministic per content.
    pending_generated_images: Vec<crate::ir::FrameGeneratedImage>,
    /// Realm-level G2 chart cache: node id → the DrawOp stream and paragraph
    /// images one chart build produced. NOT cleared by reset_for_frame — a
    /// chart whose output does not depend on time replays these ops on later
    /// frames instead of rebuilding the whole G2 mark tree.
    chart_cache: HashMap<String, ChartCacheEntry>,
    /// Node ids waiting for their second probe commit. A cache entry only
    /// becomes servable after two builds at different clock times agree.
    chart_probe_armed: std::collections::HashSet<String>,
    current_frame: u32,
    /// Scene-local frame override. The JSX frontend emits ONE root driver whose
    /// store frame is the composition frame, but each `<tl>` scene has its own
    /// clock. The JSX runtime sets this around every hook and every timeline
    /// flush so tween progress is evaluated against the scene's local frame
    /// (matching XML node scripts, whose realm already runs with a
    /// `for_segment` frame). `None` = use `current_frame` as-is.
    frame_override: Option<u32>,
    /// Scene-local time override in seconds. Tween progress advances by elapsed
    /// time, and a scene window may start between frames (a `<tl>` boundary is
    /// quantized to a frame, the author's placement is not). Without this the
    /// fractional part of the scene start would be dropped, and every tween in
    /// that scene would run that much early. `None` = derive from the frame.
    time_override_secs: Option<f64>,
    fps: u32,
}

fn css_filter_from_value(value: &serde_json::Value) -> Option<CssFilter> {
    if let Some(filter) = value.as_str() {
        return Some(CssFilter::parse_css_filter_string(filter));
    }

    if let Some(ops) = value.get("ops").and_then(|ops| ops.as_array()) {
        let mut filter = CssFilter::default();
        for op in ops {
            let Some(kind) = op
                .get("kind")
                .and_then(|kind| kind.as_str())
                .and_then(CssFilterKind::from_property)
            else {
                continue;
            };
            if kind == CssFilterKind::DropShadow {
                if let Some(shadow) = op.get("shadow").and_then(|s| {
                    serde_json::from_value::<crate::style::DropShadow>(s.clone()).ok()
                }) {
                    filter.push_drop_shadow(shadow);
                }
                continue;
            }
            let Some(value) = op.get("value").and_then(|value| value.as_f64()) else {
                continue;
            };
            filter.push(kind, value as f32);
        }
        return Some(filter);
    }

    let mut filter = CssFilter::default();
    let object = value.as_object()?;
    for (name, value) in object {
        let Some(kind) = CssFilterKind::from_property(name) else {
            continue;
        };
        if kind == CssFilterKind::DropShadow {
            if let Ok(shadow) = serde_json::from_value::<crate::style::DropShadow>(value.clone()) {
                filter.push_drop_shadow(shadow);
            }
            continue;
        }
        let Some(value) = value.as_f64() else {
            continue;
        };
        filter.push(kind, value as f32);
    }
    Some(filter)
}

pub(crate) fn css_filter_to_value(filter: &CssFilter) -> serde_json::Value {
    json!({
        "ops": filter.ops.iter().map(|op| {
            let mut entry = json!({
                "kind": op.kind.property_name(),
                "value": op.value,
            });
            if let Some(shadow) = op.shadow {
                entry["shadow"] = serde_json::to_value(shadow).unwrap_or_default();
            }
            entry
        }).collect::<Vec<_>>(),
    })
}

/// Serialize the engine's transform list as the CSS matrix string CSSPlugin
/// reads back (`getComputedStyle(el).transform` → `"matrix(a, b, c, d, e, f)"`).
/// The plugin parses that to recover the element's current x/y/rotation/scale
/// so a `.to()` without explicit `from` starts where the node already is.
fn css_transform_string(transforms: &[Transform]) -> String {
    // 2×3 affine, row-major [a, b, c, d, e, f] — the CSS `matrix()` order.
    let mut m = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mul = |m: [f32; 6], n: [f32; 6]| -> [f32; 6] {
        [
            m[0] * n[0] + m[2] * n[1],
            m[1] * n[0] + m[3] * n[1],
            m[0] * n[2] + m[2] * n[3],
            m[1] * n[2] + m[3] * n[3],
            m[0] * n[4] + m[2] * n[5] + m[4],
            m[1] * n[4] + m[3] * n[5] + m[5],
        ]
    };
    for t in transforms {
        m = match *t {
            Transform::TranslateX { value } => mul(m, [1.0, 0.0, 0.0, 1.0, value, 0.0]),
            Transform::TranslateY { value } => mul(m, [1.0, 0.0, 0.0, 1.0, 0.0, value]),
            Transform::Translate { x, y } => mul(m, [1.0, 0.0, 0.0, 1.0, x, y]),
            Transform::Scale { value } => mul(m, [value, 0.0, 0.0, value, 0.0, 0.0]),
            Transform::ScaleX { value } => mul(m, [value, 0.0, 0.0, 1.0, 0.0, 0.0]),
            Transform::ScaleY { value } => mul(m, [1.0, 0.0, 0.0, value, 0.0, 0.0]),
            Transform::RotateDeg { value } => {
                let (sin, cos) = value.to_radians().sin_cos();
                mul(m, [cos, sin, -sin, cos, 0.0, 0.0])
            }
            Transform::SkewXDeg { value } => {
                mul(m, [1.0, 0.0, value.to_radians().tan(), 1.0, 0.0, 0.0])
            }
            Transform::SkewYDeg { value } => {
                mul(m, [1.0, value.to_radians().tan(), 0.0, 1.0, 0.0, 0.0])
            }
            Transform::SkewDeg { x, y } => mul(
                m,
                [
                    1.0,
                    y.to_radians().tan(),
                    x.to_radians().tan(),
                    1.0,
                    0.0,
                    0.0,
                ],
            ),
            // 3D transforms have no 2D matrix form; the 2D part of the
            // projection is what a 2D reader can act on, so nothing to add.
            Transform::Rotate3 { .. } => m,
        };
    }
    format!(
        "matrix({}, {}, {}, {}, {}, {})",
        m[0], m[1], m[2], m[3], m[4], m[5]
    )
}

/// Parse a CSS `transform` function list into engine transforms.
///
/// GSAP's CSSPlugin is the only writer: it emits exactly `perspective()`,
/// `translate()/translate3d()`, `rotate()/rotateX()/rotateY()`, `skew()` and
/// `scale()` in a fixed order. Anything else (matrix(), rotateZ()…) is not
/// produced by the plugin suite and is ignored.
fn parse_css_transform(text: &str) -> Vec<Transform> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().to_ascii_lowercase();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let args: Vec<f32> = rest[open + 1..open + close]
            .split(',')
            .filter_map(|part| part.trim().trim_end_matches("px").trim_end_matches("deg").parse::<f32>().ok())
            .collect();
        let first = args.first().copied().unwrap_or(0.0);
        match name.as_str() {
            "translate" => out.push(Transform::Translate {
                x: first,
                y: args.get(1).copied().unwrap_or(0.0),
            }),
            "translate3d" => {
                out.push(Transform::TranslateX { value: first });
                out.push(Transform::TranslateY {
                    value: args.get(1).copied().unwrap_or(0.0),
                });
            }
            "translatex" => out.push(Transform::TranslateX { value: first }),
            "translatey" => out.push(Transform::TranslateY { value: first }),
            "rotate" => out.push(Transform::RotateDeg { value: first }),
            "skew" => out.push(Transform::SkewDeg {
                x: first,
                y: args.get(1).copied().unwrap_or(0.0),
            }),
            "skewx" => out.push(Transform::SkewXDeg { value: first }),
            "skewy" => out.push(Transform::SkewYDeg { value: first }),
            // CSSPlugin's renderTransform emits `scale(sx)` for a uniform
            // scale and `scale(sx, sy)` for a non-uniform one — `scaleY: 0`
            // arrives as `scale(1, 0)`. Keeping only the first argument loses
            // the second axis, so the node never collapses.
            "scale" => match args.get(1).copied() {
                Some(y) => {
                    out.push(Transform::ScaleX { value: first });
                    out.push(Transform::ScaleY { value: y });
                }
                None => out.push(Transform::Scale { value: first }),
            },
            "scalex" => out.push(Transform::ScaleX { value: first }),
            "scaley" => out.push(Transform::ScaleY { value: first }),
            // perspective()/rotateX()/rotateY() fold into one Rotate3; the
            // writes land through the same path as the named properties.
            _ => {}
        }
        rest = &rest[open + close + 1..];
    }
    out
}

/// 脚本写值 → 数值。
///
/// 同一个属性有两条写入通道：自研数值通道给 JSON 数字（`{ opacity: 1 }`），
/// 原生 CSSPlugin 给 **CSS 文本**（`style.opacity = "1"`、
/// `style.width = "660px"`）。两条通道落到同一个字段，所以统一在这里转。
/// 单位只认 `px`；`%` / `auto` / `none` 没有声明式参照系可算，返回 None
/// 好过按错误基准渲染。
fn num_from_value(value: &serde_json::Value) -> Option<f32> {
    match value {
        serde_json::Value::Number(n) => n.as_f64().map(|v| v as f32),
        serde_json::Value::String(text) => text
            .trim()
            .strip_suffix("px")
            .unwrap_or(text.trim())
            .trim()
            .parse::<f32>()
            .ok(),
        _ => None,
    }
}

/// `stroke-dasharray` 的脚本侧取值 → `StrokeDash`。
///
/// 脚本写的是 **CSS 文本**（DrawSVG 每帧写 `"1200px,1400px"`，起点写
/// `"0px, 999999px"`，结束时写 `"none"`），所以字符串按 CSS 语义解析：
/// `none` 表示无虚线，单段复制成等距虚线，两段即 dash/gap。数字与数组
/// 是便利写法，与字符串同义。
fn stroke_dash_from_value(value: &serde_json::Value) -> Option<crate::style::StrokeDash> {
    use crate::style::StrokeDash;
    match value {
        serde_json::Value::String(text) => {
            let text = text.trim();
            if text.is_empty() || text.eq_ignore_ascii_case("none") {
                return None;
            }
            let lens: Vec<f32> = text
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.trim().trim_end_matches("px").parse::<f32>().ok())
                .collect();
            let dash = *lens.first()?;
            Some(StrokeDash::from_pair(dash, lens.get(1).copied()))
        }
        serde_json::Value::Number(n) => n.as_f64().map(|v| StrokeDash::from_len(v as f32)),
        serde_json::Value::Array(items) => {
            let dash = items.first().and_then(|v| v.as_f64())? as f32;
            let gap = items.get(1).and_then(|v| v.as_f64()).map(|v| v as f32);
            Some(StrokeDash::from_pair(dash, gap))
        }
        _ => None,
    }
}

fn clip_path_from_value(value: &serde_json::Value) -> Option<Option<ClipPath>> {
    let raw = value.as_str()?;
    if raw.trim().eq_ignore_ascii_case("none") {
        return Some(None);
    }
    ClipPath::parse_css(raw).map(Some)
}

impl MutationStore {
    /// Record a script-generated RGBA image (canvas `putImageData` path) and
    /// return its draw payload. The pixels go to
    /// [`Self::pending_generated_images`]; the caller records the matching
    /// `DrawOp::Image { image: ImageRef::Generated { id }, .. }` for the
    /// canvas node. Ids must be deterministic per content — re-recording the
    /// same id with different pixels is a hard error downstream.
    pub fn record_frame_generated_image(
        &mut self,
        node_id: &str,
        id: crate::ir::GeneratedImageId,
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        rgba: std::sync::Arc<[u8]>,
    ) {
        self.pending_generated_images
            .push(crate::ir::FrameGeneratedImage {
                id,
                width,
                height,
                rgba,
                gpu: None,
            });
        self.record_draw_op(
            node_id,
            crate::ir::draw_op::DrawOp::Image {
                image: crate::ir::draw_types::ImageRef::Generated { id },
                x,
                y,
                paint: None,
            },
        );
    }

    /// Record a scene3 placeholder: the draw op (z-order contract) plus a
    /// table entry carrying **no pixels** — the executing host rasterizes the
    /// frame's `RenderFrame.scene3` section (engine → opencat-three, web →
    /// real three.js) and patches the plan entry before draw execution. Same
    /// id contract as [`Self::record_frame_generated_image`].
    pub fn record_frame_generated_image_deferred(
        &mut self,
        node_id: &str,
        id: crate::ir::GeneratedImageId,
        x: f32,
        y: f32,
        width: u32,
        height: u32,
    ) {
        self.record_frame_generated_image(
            node_id,
            id,
            x,
            y,
            width,
            height,
            std::sync::Arc::from(Vec::new()),
        );
    }

    /// Record a script-generated image whose pixels live on the GPU (the
    /// `ctx.three` render-target path). Same contract as
    /// [`Self::record_frame_generated_image`], except the pixels are carried
    /// as an opaque host handle instead of a CPU buffer.
    pub fn record_frame_generated_image_gpu(
        &mut self,
        node_id: &str,
        id: crate::ir::GeneratedImageId,
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        gpu: crate::ir::generated_image::GpuImageHandle,
    ) {
        self.pending_generated_images
            .push(crate::ir::FrameGeneratedImage {
                id,
                width,
                height,
                rgba: std::sync::Arc::from(Vec::new()),
                gpu: Some(gpu),
            });
        self.record_draw_op(
            node_id,
            crate::ir::draw_op::DrawOp::Image {
                image: crate::ir::draw_types::ImageRef::Generated { id },
                x,
                y,
                paint: None,
            },
        );
    }

    /// Drain all pending generated images (called once per rendered frame by
    /// the pipeline, which inserts them into the `GeneratedImageTable`).
    pub fn take_pending_generated_images(&mut self) -> Vec<crate::ir::FrameGeneratedImage> {
        std::mem::take(&mut self.pending_generated_images)
    }

    // ── G2 chart cache ──────────────────────────────────────────────
    //
    // A chart's DrawOp stream is a pure function of its spec, not of the
    // frame time: the G2 shim freezes every animation at its virtual-clock
    // origin (progress is always 0 at the instant of build). So the stream
    // recorded on the first frame can be replayed verbatim on later frames,
    // skipping the whole G2 rebuild (layout, tween engine, three rAF drain
    // passes, paragraph rasterization). A build whose output DOES read the
    // clock declares that with `chart_cache_probe`, and the shim re-checks.

    /// Replay a cached chart build for `node_id`: re-record the cached DrawOps
    /// into the node's canvas entry and re-register the paragraph images into
    /// this frame's pending table (Arc clones, no re-rasterization). Returns
    /// `false` on a miss — the caller must run a real G2 build. An entry only
    /// serves after its two-probe comparison passed and it is not dynamic.
    pub fn chart_cache_fetch(&mut self, node_id: &str) -> bool {
        if self.chart_probe_armed.contains(node_id) {
            return false;
        }
        let Some(entry) = self.chart_cache.get(node_id) else {
            return false;
        };
        if entry.dynamic || entry.commands.is_empty() {
            return false;
        }
        let commands = entry.commands.clone();
        let images = entry.images.clone();
        for image in &images {
            self.pending_generated_images.push(image.clone());
        }
        self.canvases
            .entry(node_id.to_string())
            .or_default()
            .commands
            .extend(commands);
        true
    }

    /// Snapshot the build that just ran for `node_id`. Called after a REAL
    /// build (the frame's own ops stay in the canvas entry — they are cloned
    /// into the cache, not moved). First call arms the node; the second call
    /// compares fingerprints: identical ops at two different frame times ⇒
    /// the chart is static and servable; different ⇒ dynamic, never served.
    /// Returns the previous verdict (`Some(dynamic)`) or `None` while arming.
    pub fn chart_cache_commit(&mut self, node_id: &str) -> Option<bool> {
        let commands: Vec<crate::ir::draw_op::DrawOp> = self
            .canvases
            .get(node_id)
            .map(|canvas| canvas.commands.clone())
            .unwrap_or_default();
        let image_ids: std::collections::HashSet<crate::ir::GeneratedImageId> = commands
            .iter()
            .filter_map(|op| match op {
                crate::ir::draw_op::DrawOp::Image {
                    image: crate::ir::draw_types::ImageRef::Generated { id },
                    ..
                } => Some(*id),
                _ => None,
            })
            .collect();
        let images: Vec<crate::ir::FrameGeneratedImage> = self
            .pending_generated_images
            .iter()
            .filter(|img| image_ids.contains(&img.id))
            .cloned()
            .collect();

        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        for op in &commands {
            op.hash(&mut hasher);
        }
        let fingerprint = hasher.finish();

        if !self.chart_probe_armed.contains(node_id) {
            self.chart_probe_armed.insert(node_id.to_string());
            self.chart_cache.insert(
                node_id.to_string(),
                ChartCacheEntry {
                    commands,
                    images,
                    fingerprint,
                    dynamic: false,
                },
            );
            return None;
        }

        let dynamic = self
            .chart_cache
            .get(node_id)
            .map(|prev| prev.fingerprint != fingerprint)
            .unwrap_or(true);
        self.chart_probe_armed.remove(node_id);
        // On a static verdict the just-built frame's data is identical to the
        // stored copy, so overwriting is a no-op; on dynamic we store the
        // latest purely for diagnostics — fetch never serves a dynamic entry.
        self.chart_cache.insert(
            node_id.to_string(),
            ChartCacheEntry {
                commands,
                images,
                fingerprint,
                dynamic,
            },
        );
        Some(dynamic)
    }

    /// Drop a node's cached chart (spec changed, or the author opted out).
    pub fn chart_cache_drop(&mut self, node_id: &str) {
        self.chart_cache.remove(node_id);
    }

    /// Register a frame-scoped generated image WITHOUT recording a draw op —
    /// the pixels become available to later draws in the same frame (e.g. a
    /// lambda-effect child sampling a baked render target). Same idempotency
    /// contract as [`Self::record_frame_generated_image`]: re-recording the
    /// same id with different pixels is a downstream hard error.
    pub fn register_frame_generated_image(
        &mut self,
        id: crate::ir::GeneratedImageId,
        width: u32,
        height: u32,
        rgba: std::sync::Arc<[u8]>,
    ) {
        self.pending_generated_images
            .push(crate::ir::FrameGeneratedImage {
                id,
                width,
                height,
                rgba,
                gpu: None,
            });
    }

    /// Latest registered pixels for a generated image id (this frame).
    pub fn pending_generated_image(
        &self,
        id: &crate::ir::GeneratedImageId,
    ) -> Option<(u32, u32, &std::sync::Arc<[u8]>)> {
        self.pending_generated_images
            .iter()
            .rev()
            .find(|img| &img.id == id)
            .map(|img| (img.width, img.height, &img.rgba))
    }

    /// Drain this frame's 3D scene snapshots (three-like shim 全量重提交).
    /// Used by the realm to rasterize each snapshot into a frame generated
    /// image; callers re-commit the scenes afterwards so the mutation
    /// snapshot still carries them (`snapshot_mutations` 语义不变).
    pub fn take_scene3s(
        &mut self,
    ) -> HashMap<String, crate::ir::scene3::Scene3> {
        std::mem::take(&mut self.scene3s)
    }

    /// 消耗性帧级 mutation 转移（`std::mem::take`，零拷贝）。仅取每帧重建
    /// 的三个字段，清理面与 [`MutationStore::reset_for_frame`] 一致；
    /// geometries、initial_styles、pending_generated_images 等 realm 级
    /// 字段保持原位。调用方（realm `run_frame`、resolve `push_script_scope`）
    /// 均为"录完即取、取后即弃"，take 与原快照语义等价且无深拷贝。
    pub fn take_mutations(&mut self) -> StyleMutations {
        StyleMutations {
            mutations: std::mem::take(&mut self.styles),
            canvas_mutations: std::mem::take(&mut self.canvases),
            scene3_mutations: std::mem::take(&mut self.scene3s),
        }
    }

    /// 注册一块几何（GLB 解码产物）。同 id 重复注册 = 覆盖（确定性解码
    /// 下内容一致，等价 no-op）；不随帧清除（realm 级）。
    pub fn record_geometry(&mut self, id: &str, geometry: std::sync::Arc<crate::ir::scene3::Geometry3>) {
        self.geometries.insert(id.to_string(), geometry);
    }

    /// 几何注册表的轻量快照（`Arc` 计数 +1，无几何体拷贝）——realm 每次
    /// 光栅化传给 [`crate::ir::scene3::Scene3Raster`]。
    pub fn geometry_registry(&self) -> crate::ir::scene3::GeometryRegistry {
        self.geometries.clone()
    }

    fn entry(&mut self, id: &str) -> &mut NodeStyleMutations {
        self.styles.entry(id.to_string()).or_default()
    }

    fn canvas_entry(&mut self, id: &str) -> &mut CanvasMutations {
        self.canvases.entry(id.to_string()).or_default()
    }

    fn seconds_to_frames(&self, seconds: f32) -> u32 {
        duration_secs_to_frames(seconds as f64, self.fps.max(1))
    }

    /// Set a base-style value (from Tailwind/resolve) for later reading.
    pub fn set_initial_style(&mut self, id: &str, property: &str, value: serde_json::Value) {
        self.initial_styles
            .entry(id.to_string())
            .or_default()
            .insert(property.to_string(), value);
    }

    /// Populate initial styles from a resolved NodeStyle. All animatable
    /// property extraction is centralized here — callers just pass (id, &NodeStyle).
    pub fn set_initial_style_from_node(&mut self, id: &str, style: &NodeStyle) {
        let mut props = HashMap::new();

        macro_rules! add_f32 {
            ($key:expr, $field:ident) => {
                if let Some(v) = style.$field {
                    props.insert($key.into(), json!(v));
                }
            };
        }
        macro_rules! add_color {
            ($key:expr, $field:ident) => {
                if let Some(ref c) = style.$field {
                    let (r, g, b, a) = c.rgba();
                    props.insert(
                        $key.into(),
                        json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)),
                    );
                }
            };
        }

        // f32
        add_f32!("opacity", opacity);
        add_f32!("strokeDashoffset", stroke_dashoffset);
        add_f32!("strokeWidth", stroke_width);
        add_f32!("borderWidth", border_width);
        add_f32!("textSize", text_px);
        add_f32!("letterSpacing", letter_spacing);
        add_f32!("lineHeight", line_height);
        add_f32!("width", width);
        add_f32!("height", height);
        add_f32!("gap", gap);
        add_f32!("flexGrow", flex_grow);
        // Same text form `read_style_value` returns, so a script reading the
        // CSS-declared dasharray sees the value the style sheet declared.
        if let Some(dash) = style.stroke_dasharray {
            props.insert(
                "strokeDasharray".into(),
                json!(format!("{}px,{}px", dash.dash, dash.gap)),
            );
        }
        // Inset quads — `read_style_value` exposes them as left/top/right/bottom,
        // so the initial seeding has to carry the same keys or a fresh
        // `getBoundingClientRect` would read 0 for class-declared insets.
        for (key, value) in [
            ("left", style.inset_left),
            ("top", style.inset_top),
            ("right", style.inset_right),
            ("bottom", style.inset_bottom),
        ] {
            if let Some(crate::style::LengthPercentageAuto::Length(v)) = value {
                props.insert(key.into(), json!(v));
            }
        }
        for op in &style.css_filter.ops {
            if let Some(shadow) = op.shadow {
                props.insert(
                    op.kind.property_name().into(),
                    serde_json::to_value(shadow).unwrap_or_default(),
                );
            } else {
                props.insert(op.kind.property_name().into(), json!(op.value));
            }
        }
        if !style.css_filter.is_empty() {
            props.insert("filter".into(), css_filter_to_value(&style.css_filter));
        }
        if let Some(clip_path) = style.clip_path {
            props.insert("clipPath".into(), json!(clip_path.to_css_string()));
        }
        add_f32!("backdropBlur", backdrop_blur_sigma);

        // colors
        add_color!("fillColor", fill_color);
        add_color!("strokeColor", stroke_color);
        add_color!("backgroundColor", bg_color);
        add_color!("textColor", text_color);
        add_color!("borderColor", border_color);

        // strings
        if let Some(ref s) = style.svg_path {
            props.insert("morphSVG".into(), json!(s));
        }

        if !props.is_empty() {
            self.initial_styles.insert(id.to_string(), props);
        }
    }

    /// Resolve the pixel components of a gsap `transformOrigin` string into box
    /// fractions (`'174px 0'` on an 820×1716 image → `'21.219513% 0%'`).
    ///
    /// A browser resolves px origins against the used box; the write channel
    /// has no layout, so it uses the declared box (`w-[820px] h-[1716px]`),
    /// which is the used box for every fixed-size node. An axis without a
    /// declared size keeps its token verbatim — `TransformOrigin::parse` then
    /// either accepts it (percent/keyword/zero) or leaves the previous origin,
    /// as before.
    fn origin_px_to_fractions(&self, id: &str, value: &str) -> String {
        let Some(props) = self.initial_styles.get(id) else {
            return value.to_string();
        };
        let dim = |key: &str| {
            props
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .filter(|v| *v > 0.0)
        };
        let (width, height) = (dim("width"), dim("height"));
        if width.is_none() && height.is_none() {
            return value.to_string();
        }
        // CSS accepts either `[x] [y]` or the y-first keyword order; a lone
        // token positions x (see `TransformOrigin::parse`).
        let tokens: Vec<&str> = value.split_whitespace().collect();
        let y_first = matches!(tokens.first().copied(), Some("top" | "bottom"));
        tokens
            .iter()
            .enumerate()
            .map(|(i, token)| {
                let Some(bare) = token.strip_suffix("px") else {
                    return (*token).to_string();
                };
                let Ok(px) = bare.trim().parse::<f64>() else {
                    return (*token).to_string();
                };
                let is_y = match tokens.len() {
                    1 => false,
                    _ => {
                        if y_first {
                            i == 0
                        } else {
                            i == 1
                        }
                    }
                };
                match if is_y { height } else { width } {
                    Some(size) => format!("{}%", px / size * 100.0),
                    None => (*token).to_string(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Read the current value: mutation first, then base-style, then None.
    /// Returns JSON: f32 as Number, color as String, path as String.
    pub fn read_style_value(&self, id: &str, property: &str) -> Option<serde_json::Value> {
        // 1. Check current-frame mutations
        if let Some(mutations) = self.styles.get(id) {
            let v: Option<serde_json::Value> = match property {
                // f32
                "opacity" => mutations.opacity.map(|f| json!(f)),
                // CSS 文本形式：DrawSVG 用 `getComputedStyle(...).strokeDasharray`
                // 反推当前已画出的长度（"1200px,1400px" 里逗号前是实线段）。
                "strokeDasharray" => mutations
                    .stroke_dasharray
                    .map(|d| json!(format!("{}px,{}px", d.dash, d.gap))),
                "strokeDashoffset" => mutations.stroke_dashoffset.map(|f| json!(f)),
                "strokeWidth" => mutations.stroke_width.map(|f| json!(f)),
                "borderRadius" => mutations.border_radius.map(|f| json!(f)),
                "borderWidth" => mutations.border_width.map(|f| json!(f)),
                "textSize" => mutations.text_px.map(|f| json!(f)),
                "letterSpacing" => mutations.letter_spacing.map(|f| json!(f)),
                "lineHeight" => mutations.line_height.map(|f| json!(f)),
                "width" => mutations.width.map(|f| json!(f)),
                "height" => mutations.height.map(|f| json!(f)),
                "gap" => mutations.gap.map(|f| json!(f)),
                "flexGrow" => mutations.flex_grow.map(|f| json!(f)),
                "left" => mutations.inset_left.map(|f| json!(f)),
                "top" => mutations.inset_top.map(|f| json!(f)),
                "right" => mutations.inset_right.map(|f| json!(f)),
                "bottom" => mutations.inset_bottom.map(|f| json!(f)),
                // Transform components — so a tween written without an
                // explicit `from` (e.g. `.to('obl',{x:L})`) starts from the
                // value currently set on the node, as GSAP does.
                "x" | "translateX" | "y" | "translateY" | "scale" | "scaleX" | "scaleY"
                | "rotation" | "rotate" | "skewX" | "skewY" => {
                    mutations.read_transform_value(property).map(|f| json!(f))
                }
                // CSS text, the shape a browser's computed style reports.
                // CSSPlugin hands this straight to `_applySVGOrigin`, which
                // does `origin.split(" ")` — an array or a number here throws
                // `origin.split is not a function` on every SVG tween.
                // The stored origin is a fraction of the box, so it goes back
                // out as a percentage (a browser never reports `0.5px`).
                "transformOrigin" | "svgOrigin" => {
                    let o = mutations
                        .transform_origin
                        .unwrap_or(crate::style::TransformOrigin { x: 0.5, y: 0.5 });
                    Some(json!(format!("{}% {}%", o.x * 100.0, o.y * 100.0)))
                }
                // CSS text form, the shape CSSPlugin parses on init.
                "transform" => Some(json!(css_transform_string(&mutations.transforms))),
                "cssText" => None,
                "filter" => (!mutations.css_filter.is_empty())
                    .then(|| css_filter_to_value(&mutations.css_filter)),
                "clipPath" | "clip-path" => mutations
                    .clip_path
                    .map(|clip_path| json!(clip_path.to_css_string())),
                "blur" | "blurSigma" | "brightness" | "contrast" | "grayscale" | "hueRotate"
                | "invert" | "saturate" | "sepia" => {
                    mutations.css_filter.value(property).map(|f| json!(f))
                }
                "backdropBlur" | "backdropBlurSigma" => {
                    mutations.backdrop_blur_sigma.map(|f| json!(f))
                }
                // colors (serialize as "#RRGGBBAA" or "transparent")
                "fillColor" => mutations.fill_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "strokeColor" => mutations.stroke_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "backgroundColor" | "bg" => mutations.bg_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "textColor" => mutations.text_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "borderColor" => mutations.border_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                // strings
                "morphSVG" | "d" => mutations.svg_path.as_ref().map(|s| json!(s)),
                "text" => mutations.text_content.as_ref().map(|s| json!(s)),
                "textShadow" | "textShadows" => mutations
                    .text_shadows
                    .as_ref()
                    .map(|shadows| crate::script::mutations::text_shadows_to_value(shadows)),
                _ => None,
            };
            if v.is_some() {
                return v;
            }
        }
        // 2. Fall back to initial styles (Tailwind base values)
        self.initial_styles
            .get(id)
            .and_then(|props| props.get(property).cloned())
    }

    /// Unified write: set any animatable property from (id, property_name, JSON value).
    /// Centralizes all property→field mapping in one place.
    pub fn write_style_value(&mut self, id: &str, property: &str, value: serde_json::Value) {
        // `transformOrigin: '174px 0'` is how gsap (and the browser CSSOM) names
        // a pixel anchor. The store keeps origins as box fractions, so the px
        // has to be resolved against the box BEFORE the entry borrow starts.
        let origin_px = match (property, value.as_str()) {
            ("transformOrigin" | "svgOrigin", Some(text)) => {
                Some(self.origin_px_to_fractions(id, text))
            }
            _ => None,
        };
        let entry = self.styles.entry(id.to_string()).or_default();
        match property {
            // ── f32 ──
            "opacity" => {
                entry.opacity = num_from_value(&value);
            }
            // GSAP names (`x`/`y`) accepted too: the imperative `getNode(id).x(v)`
            // proxy forwards the raw property name, while the timeline property
            // registry normalizes `x`→`translateX` before writing. Accept both so
            // imperative and tween writes share one mapping (previously `x`/`y`
            // fell through the match and were silently dropped).
            "x" | "translateX" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::TranslateX { value: v as f32 });
                }
            }
            "y" | "translateY" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::TranslateY { value: v as f32 });
                }
            }
            "scale" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::Scale { value: v as f32 });
                }
            }
            "scaleX" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::ScaleX { value: v as f32 });
                }
            }
            "scaleY" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::ScaleY { value: v as f32 });
                }
            }
            "rotate" | "rotation" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::RotateDeg { value: v as f32 });
                }
            }
            // GSAP 3D: `rotationX`/`rotationY`/`transformPerspective` fold into
            // one projective transform (see `Transform::Rotate3`).
            "rotationX" | "rotateX" => {
                if let Some(v) = num_from_value(&value) {
                    entry.write_rotate3_field(crate::script::mutations::Rotate3Field::RotationX, v as f32);
                }
            }
            "rotationY" | "rotateY" => {
                if let Some(v) = num_from_value(&value) {
                    entry.write_rotate3_field(crate::script::mutations::Rotate3Field::RotationY, v as f32);
                }
            }
            "transformPerspective" => {
                if let Some(v) = num_from_value(&value) {
                    entry.write_rotate3_field(crate::script::mutations::Rotate3Field::Perspective, v as f32);
                }
            }
            "skewX" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::SkewXDeg { value: v as f32 });
                }
            }
            "skewY" => {
                if let Some(v) = num_from_value(&value) {
                    entry.push_transform(Transform::SkewYDeg { value: v as f32 });
                }
            }
            // GSAP `transformOrigin` / `svgOrigin`. For a node whose box matches
            // the SVG viewBox (the case in OpenCat's port: one wrapper per mark)
            // svgOrigin coordinates and box-relative px coincide, so both map to
            // the same box fraction.
            "transformOrigin" | "svgOrigin" => {
                if let Some(s) = origin_px.as_deref() {
                    entry.set_transform_origin(s);
                } else if let Some(arr) = value.as_array() {
                    if let (Some(x), Some(y)) = (
                        arr.first().and_then(|v| v.as_f64()),
                        arr.get(1).and_then(|v| v.as_f64()),
                    ) {
                        entry.transform_origin = Some(TransformOrigin {
                            x: x as f32,
                            y: y as f32,
                        });
                    }
                }
            }
            // CSS `style.transform` — the write CSSPlugin actually makes
            // (`translate(100px, 50px) rotate(30deg) scale(1.2)`). A declarative
            // composition has no CSS cascade, so the string is parsed straight
            // into the engine's transform list; anything the engine cannot
            // express is dropped rather than silently mangled.
            //
            // This REPLACES the list, it does not append: `transform` is one
            // CSS property, so a write drops whatever the previous string
            // said. Appending instead keeps a stale `scale(2.6)` alive after
            // CSSPlugin writes `translate(0px, 0px)` on the way back to rest,
            // which renders a card that should be back to normal size at 2.6×.
            "transform" => {
                if let Some(s) = value.as_str() {
                    entry.transforms = parse_css_transform(s);
                }
            }
            "left" => {
                entry.inset_left = num_from_value(&value);
            }
            "top" => {
                entry.inset_top = num_from_value(&value);
            }
            "right" => {
                entry.inset_right = num_from_value(&value);
            }
            "bottom" => {
                entry.inset_bottom = num_from_value(&value);
            }
            "width" => {
                entry.width = num_from_value(&value);
            }
            "height" => {
                entry.height = num_from_value(&value);
            }
            "padding" => {
                entry.padding = num_from_value(&value);
            }
            "paddingX" => {
                entry.padding_x = num_from_value(&value);
            }
            "paddingY" => {
                entry.padding_y = num_from_value(&value);
            }
            "margin" => {
                entry.margin = num_from_value(&value);
            }
            "marginX" => {
                entry.margin_x = num_from_value(&value);
            }
            "marginY" => {
                entry.margin_y = num_from_value(&value);
            }
            "gap" => {
                entry.gap = num_from_value(&value);
            }
            "flexGrow" => {
                entry.flex_grow = num_from_value(&value);
            }
            "borderRadius" => {
                entry.border_radius = num_from_value(&value);
            }
            "borderWidth" => {
                entry.border_width = num_from_value(&value);
            }
            "borderTopWidth" => {
                entry.border_top_width = num_from_value(&value);
            }
            "borderRightWidth" => {
                entry.border_right_width = num_from_value(&value);
            }
            "borderBottomWidth" => {
                entry.border_bottom_width = num_from_value(&value);
            }
            "borderLeftWidth" => {
                entry.border_left_width = num_from_value(&value);
            }
            "strokeWidth" => {
                entry.stroke_width = num_from_value(&value).map(|v| v.max(0.0));
            }
            "strokeDasharray" => {
                // 三种写法都要收：脚本写 CSS 文本（DrawSVG 写
                // "1200px,1400px" 或 "none"，ScrambleText 之外的插件也常这样）、
                // 直接写两段数组、以及老的裸数字。
                entry.stroke_dasharray = stroke_dash_from_value(&value);
            }
            "strokeDashoffset" => {
                entry.stroke_dashoffset = num_from_value(&value);
            }
            "textSize" => {
                entry.text_px = num_from_value(&value);
            }
            "letterSpacing" => {
                entry.letter_spacing = num_from_value(&value);
            }
            "lineHeight" => {
                entry.line_height = num_from_value(&value);
            }
            "fontWeight" => {
                entry.font_weight = num_from_value(&value).map(|v| FontWeight(v as u16));
            }
            "filter" => {
                if let Some(filter) = css_filter_from_value(&value) {
                    entry.css_filter = filter;
                }
            }
            "clipPath" | "clip-path" => {
                if let Some(clip_path) = clip_path_from_value(&value) {
                    entry.clip_path = clip_path;
                }
            }
            "blur" | "blurSigma" | "brightness" | "contrast" | "grayscale" | "hueRotate"
            | "invert" | "saturate" | "sepia" => {
                if let Some(v) = num_from_value(&value) {
                    entry.css_filter.set_property(property, v as f32);
                }
            }
            "backdropBlur" | "backdropBlurSigma" => {
                entry.backdrop_blur_sigma = num_from_value(&value);
            }

            // ── colors ──
            "bg" | "backgroundColor" => {
                if let Some(s) = value.as_str() {
                    entry.bg_color = color_token_from_script_string(s);
                }
            }
            "fillColor" => {
                if let Some(s) = value.as_str() {
                    entry.fill_color = color_token_from_script_string(s);
                }
            }
            "strokeColor" => {
                if let Some(s) = value.as_str() {
                    entry.stroke_color = color_token_from_script_string(s);
                }
            }
            "textColor" => {
                if let Some(s) = value.as_str() {
                    entry.text_color = color_token_from_script_string(s);
                }
            }
            "borderColor" => {
                if let Some(s) = value.as_str() {
                    entry.border_color = color_token_from_script_string(s);
                }
            }
            "shadowColor" => {
                if let Some(s) = value.as_str() {
                    entry.box_shadow_color = color_token_from_script_string(s);
                }
            }
            "insetShadowColor" => {
                if let Some(s) = value.as_str() {
                    entry.inset_shadow_color = color_token_from_script_string(s);
                }
            }
            "dropShadowColor" => {
                if let Some(s) = value.as_str() {
                    entry.drop_shadow_color = color_token_from_script_string(s);
                }
            }

            // ── enums ──
            "position" => {
                if let Some(s) = value.as_str() {
                    entry.position = position_from_name(s);
                }
            }
            "flexDirection" => {
                if let Some(s) = value.as_str() {
                    entry.flex_direction = flex_direction_from_name(s);
                }
            }
            "justifyContent" => {
                if let Some(s) = value.as_str() {
                    entry.justify_content = justify_content_from_name(s);
                }
            }
            "alignItems" => {
                if let Some(s) = value.as_str() {
                    entry.align_items = align_items_from_name(s);
                }
            }
            "objectFit" => {
                if let Some(s) = value.as_str() {
                    entry.object_fit = object_fit_from_name(s);
                }
            }
            "borderStyle" => {
                if let Some(s) = value.as_str() {
                    entry.border_style = match s {
                        "solid" => Some(BorderStyle::Solid),
                        "dashed" => Some(BorderStyle::Dashed),
                        "dotted" => Some(BorderStyle::Dotted),
                        _ => None,
                    };
                }
            }
            "textAlign" => {
                if let Some(s) = value.as_str() {
                    entry.text_align = text_align_from_name(s);
                }
            }

            // ── strings ──
            "text" => {
                if let Some(s) = value.as_str() {
                    entry.text_content = Some(s.to_string());
                }
            }
            "morphSVG" | "d" => {
                if let Some(s) = value.as_str() {
                    entry.svg_path = Some(s.to_string());
                }
            }
            "shadow" => {
                if let Some(s) = value.as_str() {
                    if let Some(sh) = box_shadow_from_name(s) {
                        entry.box_shadow = Some(sh);
                    }
                }
            }
            "insetShadow" => {
                if let Some(s) = value.as_str() {
                    if let Some(sh) = inset_shadow_from_name(s) {
                        entry.inset_shadow = Some(sh);
                    }
                }
            }
            "dropShadow" => {
                if let Some(s) = value.as_str() {
                    if let Some(sh) = drop_shadow_from_name(s) {
                        entry.drop_shadow = Some(sh);
                    }
                }
            }
            // Overwrite semantics: the script sets the whole text-shadow stack
            // (see NodeStyleMutations::text_shadows).
            "textShadow" | "textShadows" => {
                if let Some(shadows) = crate::script::mutations::text_shadows_from_value(&value) {
                    entry.text_shadows = Some(shadows);
                }
            }

            // ── multi-arg as array ──
            "translate" => {
                if let Some(arr) = value.as_array() {
                    if let (Some(x), Some(y)) = (
                        arr.first().and_then(|v| v.as_f64()),
                        arr.get(1).and_then(|v| v.as_f64()),
                    ) {
                        entry.push_transform(Transform::TranslateX { value: x as f32 });
                        entry.push_transform(Transform::TranslateY { value: y as f32 });
                    }
                }
            }
            "skew" => {
                if let Some(arr) = value.as_array() {
                    if let (Some(x), Some(y)) = (
                        arr.first().and_then(|v| v.as_f64()),
                        arr.get(1).and_then(|v| v.as_f64()),
                    ) {
                        entry.push_transform(Transform::SkewXDeg { value: x as f32 });
                        entry.push_transform(Transform::SkewYDeg { value: y as f32 });
                    }
                }
            }

            _ => {}
        }
    }

}

impl MutationRecorder for MutationStore {
    fn record_opacity(&mut self, id: &str, v: f32) {
        self.entry(id).opacity = Some(v);
    }
    fn record_translate(&mut self, id: &str, x: f32, y: f32) {
        self.entry(id).push_transform(Transform::Translate { x, y });
    }
    fn record_translate_x(&mut self, id: &str, v: f32) {
        self.entry(id).push_transform(Transform::TranslateX { value: v });
    }
    fn record_translate_y(&mut self, id: &str, v: f32) {
        self.entry(id).push_transform(Transform::TranslateY { value: v });
    }
    fn record_scale(&mut self, id: &str, v: f32) {
        self.entry(id).push_transform(Transform::Scale { value: v });
    }
    fn record_scale_x(&mut self, id: &str, v: f32) {
        self.entry(id).push_transform(Transform::ScaleX { value: v });
    }
    fn record_scale_y(&mut self, id: &str, v: f32) {
        self.entry(id).push_transform(Transform::ScaleY { value: v });
    }
    fn record_rotate(&mut self, id: &str, deg: f32) {
        self.entry(id).push_transform(Transform::RotateDeg { value: deg });
    }
    fn record_rotate3(&mut self, id: &str, rotation_x: f32, rotation_y: f32, perspective: f32) {
        self.entry(id).write_rotate3_field(Rotate3Field::RotationX, rotation_x);
        self.entry(id).write_rotate3_field(Rotate3Field::RotationY, rotation_y);
        self.entry(id).write_rotate3_field(Rotate3Field::Perspective, perspective);
    }
    fn record_skew_x(&mut self, id: &str, deg: f32) {
        self.entry(id).push_transform(Transform::SkewXDeg { value: deg });
    }
    fn record_skew_y(&mut self, id: &str, deg: f32) {
        self.entry(id).push_transform(Transform::SkewYDeg { value: deg });
    }
    fn record_skew(&mut self, id: &str, x_deg: f32, y_deg: f32) {
        self.entry(id)
            .push_transform(Transform::SkewDeg { x: x_deg, y: y_deg });
    }
    fn record_left(&mut self, id: &str, v: f32) {
        self.entry(id).inset_left = Some(v);
    }
    fn record_top(&mut self, id: &str, v: f32) {
        self.entry(id).inset_top = Some(v);
    }
    fn record_right(&mut self, id: &str, v: f32) {
        self.entry(id).inset_right = Some(v);
    }
    fn record_bottom(&mut self, id: &str, v: f32) {
        self.entry(id).inset_bottom = Some(v);
    }
    fn record_width(&mut self, id: &str, v: f32) {
        self.entry(id).width = Some(v);
    }
    fn record_height(&mut self, id: &str, v: f32) {
        self.entry(id).height = Some(v);
    }
    fn record_padding(&mut self, id: &str, v: f32) {
        self.entry(id).padding = Some(v);
    }
    fn record_padding_x(&mut self, id: &str, v: f32) {
        self.entry(id).padding_x = Some(v);
    }
    fn record_padding_y(&mut self, id: &str, v: f32) {
        self.entry(id).padding_y = Some(v);
    }
    fn record_margin(&mut self, id: &str, v: f32) {
        self.entry(id).margin = Some(v);
    }
    fn record_margin_x(&mut self, id: &str, v: f32) {
        self.entry(id).margin_x = Some(v);
    }
    fn record_margin_y(&mut self, id: &str, v: f32) {
        self.entry(id).margin_y = Some(v);
    }
    fn record_gap(&mut self, id: &str, v: f32) {
        self.entry(id).gap = Some(v);
    }
    fn record_flex_grow(&mut self, id: &str, v: f32) {
        self.entry(id).flex_grow = Some(v);
    }
    fn record_border_radius(&mut self, id: &str, v: f32) {
        self.entry(id).border_radius = Some(v);
    }
    fn record_border_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_width = Some(v);
    }
    fn record_border_top_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_top_width = Some(v);
    }
    fn record_border_right_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_right_width = Some(v);
    }
    fn record_border_bottom_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_bottom_width = Some(v);
    }
    fn record_border_left_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_left_width = Some(v);
    }
    fn record_stroke_width(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_width = Some(v.max(0.0));
    }
    fn record_stroke_dasharray(&mut self, id: &str, v: crate::style::StrokeDash) {
        self.entry(id).stroke_dasharray = Some(crate::style::StrokeDash {
            dash: v.dash.max(0.0),
            gap: v.gap.max(0.0),
        });
    }
    fn record_stroke_dashoffset(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_dashoffset = Some(v);
    }
    fn record_text_size(&mut self, id: &str, v: f32) {
        self.entry(id).text_px = Some(v);
    }
    fn record_letter_spacing(&mut self, id: &str, v: f32) {
        self.entry(id).letter_spacing = Some(v);
    }
    fn record_line_height(&mut self, id: &str, v: f32) {
        self.entry(id).line_height = Some(v);
    }

    fn record_position(&mut self, id: &str, pos: Position) {
        self.entry(id).position = Some(pos);
    }
    fn record_flex_direction(&mut self, id: &str, fd: FlexDirection) {
        self.entry(id).flex_direction = Some(fd);
    }
    fn record_justify_content(&mut self, id: &str, jc: JustifyContent) {
        self.entry(id).justify_content = Some(jc);
    }
    fn record_align_items(&mut self, id: &str, ai: AlignItems) {
        self.entry(id).align_items = Some(ai);
    }
    fn record_object_fit(&mut self, id: &str, of: ObjectFit) {
        self.entry(id).object_fit = Some(of);
    }
    fn record_text_align(&mut self, id: &str, ta: TextAlign) {
        self.entry(id).text_align = Some(ta);
    }
    fn record_border_style(&mut self, id: &str, bs: BorderStyle) {
        self.entry(id).border_style = Some(bs);
    }
    fn record_font_weight(&mut self, id: &str, w: FontWeight) {
        self.entry(id).font_weight = Some(w);
    }
    fn record_box_shadow(&mut self, id: &str, sh: BoxShadow) {
        self.entry(id).box_shadow = Some(sh);
    }
    fn record_inset_shadow(&mut self, id: &str, sh: InsetShadow) {
        self.entry(id).inset_shadow = Some(sh);
    }
    fn record_drop_shadow(&mut self, id: &str, sh: DropShadow) {
        self.entry(id).drop_shadow = Some(sh);
    }

    fn record_bg_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).bg_color = Some(color);
    }
    fn record_fill_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).fill_color = Some(color);
    }
    fn record_stroke_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).stroke_color = Some(color);
    }
    fn record_border_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).border_color = Some(color);
    }
    fn record_text_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).text_color = Some(color);
    }
    fn record_box_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).box_shadow_color = Some(color);
    }
    fn record_inset_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).inset_shadow_color = Some(color);
    }
    fn record_drop_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).drop_shadow_color = Some(color);
    }

    fn record_transform(&mut self, id: &str, t: Transform) {
        self.entry(id).push_transform(t);
    }

    fn record_transform_origin(&mut self, id: &str, origin: TransformOrigin) {
        self.entry(id).transform_origin = Some(origin);
    }

    fn record_text_content(&mut self, id: &str, text: String) {
        self.entry(id).text_content = Some(text.clone());
        self.text_sources.insert(
            id.to_string(),
            ScriptTextSource {
                text,
                kind: crate::script::ScriptTextSourceKind::TextNode,
            },
        );
    }

    fn record_text_unit_override(
        &mut self,
        id: &str,
        granularity: TextUnitGranularity,
        index: usize,
        values: TextUnitValues,
    ) {
        let mutations = self.entry(id);
        match &mut mutations.text_unit_overrides {
            Some(batch) => {
                if batch.granularity != granularity {
                    return;
                }
                if index >= batch.overrides.len() {
                    batch
                        .overrides
                        .resize_with(index + 1, TextUnitOverride::default);
                }
            }
            None => {
                let mut batch = TextUnitOverrideBatch {
                    granularity,
                    overrides: Vec::new(),
                };
                batch
                    .overrides
                    .resize_with(index + 1, TextUnitOverride::default);
                mutations.text_unit_overrides = Some(batch);
            }
        }
        let entry = &mut mutations.text_unit_overrides.as_mut().unwrap().overrides[index];
        if let Some(v) = values.opacity {
            entry.opacity = Some(v);
        }
        if let Some(v) = values.translate_x {
            entry.translate_x = Some(v);
        }
        if let Some(v) = values.translate_y {
            entry.translate_y = Some(v);
        }
        if let Some(v) = values.scale {
            entry.scale = Some(v);
        }
        if let Some(v) = values.rotation_deg {
            entry.rotation_deg = Some(v);
        }
        if let Some(c) = values.color {
            entry.color = Some(c);
        }
    }

    fn record_svg_path(&mut self, id: &str, data: String) {
        self.entry(id).svg_path = Some(data);
    }

    fn record_draw_op(&mut self, id: &str, cmd: crate::ir::draw_op::DrawOp) {
        self.canvas_entry(id).commands.push(cmd);
    }

    fn chart_cache_fetch(&mut self, node_id: &str) -> bool {
        MutationStore::chart_cache_fetch(self, node_id)
    }
    fn chart_cache_commit(&mut self, node_id: &str) -> Option<bool> {
        MutationStore::chart_cache_commit(self, node_id)
    }
    fn chart_cache_drop(&mut self, node_id: &str) {
        MutationStore::chart_cache_drop(self, node_id)
    }

    fn record_scene3(&mut self, id: &str, scene: crate::ir::scene3::Scene3) {
        self.scene3s.insert(id.to_string(), scene);
    }

    fn record_draw_picture(&mut self, target_id: &str, owner_id: &str, x: f32, y: f32) {
        self.record_draw_op(
            target_id,
            crate::ir::draw_op::DrawOp::DrawSubtreePicture {
                owner_id: owner_id.to_string(),
                x,
                y,
            },
        );
    }

    fn record_canvas_runtime_effect(
        &mut self,
        id: &str,
        sksl: String,
        uniforms_bytes: Vec<u8>,
        children: Vec<crate::ir::draw_types::ScriptRuntimeEffectChild>,
        dst: crate::ir::draw_op::Rect4,
    ) {
        self.record_draw_op(
            id,
            crate::ir::draw_op::DrawOp::ScriptRuntimeEffect {
                sksl,
                uniforms_bytes,
                children,
                dst,
            },
        );
    }

    fn reset_for_frame(&mut self, current_frame: u32, fps: u32) {
        self.styles.clear();
        self.canvases.clear();
        self.scene3s.clear();
        self.current_frame = current_frame;
        self.frame_override = None;
        self.time_override_secs = None;
        self.fps = fps.max(1);
    }

    fn snapshot_mutations(&self) -> StyleMutations {
        StyleMutations {
            mutations: self.styles.clone(),
            canvas_mutations: self.canvases.clone(),
            scene3_mutations: self.scene3s.clone(),
        }
    }

    fn read_style_value(&self, id: &str, property: &str) -> Option<serde_json::Value> {
        self.read_style_value(id, property)
    }

    fn write_style_value(&mut self, id: &str, property: &str, value: serde_json::Value) {
        self.write_style_value(id, property, value)
    }

    fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        self.text_sources.insert(id.to_string(), source);
    }
    fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }
    fn get_text_source(&self, id: &str) -> Option<&ScriptTextSource> {
        self.text_sources.get(id)
    }
    fn current_frame(&self) -> u32 {
        self.current_frame
    }
    fn script_frame(&self) -> u32 {
        self.frame_override.unwrap_or(self.current_frame)
    }
    fn set_frame_override(&mut self, frame: Option<u32>) {
        self.frame_override = frame;
    }
    /// Scene-local time the tween evaluator should use, in seconds.
    fn script_time_secs(&self) -> f64 {
        self.time_override_secs
            .unwrap_or_else(|| self.script_frame() as f64 / self.fps.max(1) as f64)
    }
    fn set_time_override_secs(&mut self, secs: Option<f64>) {
        self.time_override_secs = secs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::ColorToken;

    #[test]
    fn record_opacity_writes_into_styles() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.7);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.opacity, Some(0.7));
    }

    #[test]
    fn lone_translate_stays_atomic() {
        // No collision → the write stays as a single `Translate`, so the
        // rendered matrix path is unchanged from before this fix.
        let mut store = MutationStore::default();
        store.record_translate("node-a", 12.0, -8.0);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.transforms, vec![Transform::Translate { x: 12.0, y: -8.0 }]);
    }

    #[test]
    fn translate_then_translate_x_replaces_only_x() {
        // A later x-write supersedes the x component of the earlier translate
        // while leaving y intact.
        let mut store = MutationStore::default();
        store.record_translate("node-a", 12.0, -8.0);
        store.record_translate_x("node-a", 5.0);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(
            entry.transforms,
            vec![
                Transform::TranslateY { value: -8.0 },
                Transform::TranslateX { value: 5.0 },
            ]
        );
    }

    #[test]
    fn repeated_translate_x_replaces_not_accumulates() {
        // GSAP's `x` is a single writable property: two writes in one frame are
        // last-write-wins. The old append behaviour summed them and collapsed
        // the k3 OPEN-card braces onto the text.
        let mut store = MutationStore::default();
        store.record_translate_x("node-a", 67.5225);
        store.record_translate_x("node-a", 325.5715);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.transforms, vec![Transform::TranslateX { value: 325.5715 }]);
    }

    #[test]
    fn distinct_transform_slots_coexist() {
        // Writing x then y then x again keeps both slots, with x last-write-wins.
        let mut store = MutationStore::default();
        store.record_translate_x("node-a", 1.0);
        store.record_translate_y("node-a", 2.0);
        store.record_translate_x("node-a", 3.0);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(
            entry.transforms,
            vec![
                Transform::TranslateY { value: 2.0 },
                Transform::TranslateX { value: 3.0 },
            ]
        );
    }

    #[test]
    fn transform_origin_parses_gsap_forms() {
        // Bare `0px 0px` (GSAP's favourite ink-anchor spelling) normalises to the
        // element's top-left; percentages/kewords pass through as box fractions.
        let mut store = MutationStore::default();
        store.write_style_value("node-a", "transformOrigin", json!("0px 0px"));
        let snap = store.snapshot_mutations();
        let o = snap
            .mutations
            .get("node-a")
            .expect("node-a recorded")
            .transform_origin
            .expect("origin recorded");
        assert_eq!((o.x, o.y), (0.0, 0.0));

        store.write_style_value("node-b", "svgOrigin", json!("50% 100%"));
        let snap = store.snapshot_mutations();
        let o = snap
            .mutations
            .get("node-b")
            .expect("node-b recorded")
            .transform_origin
            .expect("origin recorded");
        assert_eq!((o.x, o.y), (0.5, 1.0));
    }

    #[test]
    fn transform_origin_is_last_write_wins_and_reads_back() {
        let mut store = MutationStore::default();
        store.write_style_value("node-a", "transformOrigin", json!("0% 0%"));
        store.write_style_value("node-a", "transformOrigin", json!("100% 50%"));
        let o = store.read_style_value("node-a", "transformOrigin").unwrap();
        assert_eq!(o, json!("100% 50%"));
        // A touched node that never set an origin falls back to the CSS centre.
        store.record_opacity("node-z", 0.5);
        assert_eq!(
            store.read_style_value("node-z", "transformOrigin").unwrap(),
            json!("50% 50%")
        );
    }

    #[test]
    fn record_bg_color_uses_color_token() {
        let mut store = MutationStore::default();
        store.record_bg_color("node-a", ColorToken::Custom(255, 0, 0, 255));
        let snap = store.snapshot_mutations();
        assert_eq!(
            snap.mutations.get("node-a").unwrap().bg_color,
            Some(ColorToken::Custom(255, 0, 0, 255))
        );
    }

    #[test]
    fn snapshot_does_not_clear_styles() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.5);
        let _ = store.snapshot_mutations();
        let snap = store.snapshot_mutations();
        assert!(snap.mutations.contains_key("node-a"));
    }

    #[test]
    fn reset_for_frame_clears_styles_and_canvases() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.5);
        store.reset_for_frame(7, 30);
        let snap = store.snapshot_mutations();
        assert!(snap.mutations.is_empty());
    }

    #[test]
    fn script_frame_prefers_override_and_resets_each_frame() {
        let mut store = MutationStore::default();
        store.reset_for_frame(90, 30);
        assert_eq!(store.script_frame(), 90);
        store.set_frame_override(Some(5));
        assert_eq!(store.script_frame(), 5);
        // A new frame drops the override: a scene-local frame must not leak
        // into the next frame's evaluation.
        store.reset_for_frame(91, 30);
        assert_eq!(store.script_frame(), 91);
    }

    #[test]
    fn scene3_commit_overwrites_and_resets_per_frame() {
        let mut store = MutationStore::default();
        let scene = |matrix: [f32; 16]| -> crate::ir::scene3::Scene3 {
            crate::ir::scene3::Scene3 {
                clear: [0.0, 0.0, 0.0, 0.0],
                target: [64, 64],
                camera: crate::ir::scene3::Camera3 {
                    position: [0.0, 0.0, 3.0],
                    target: [0.0; 3],
                    fov_y_degrees: 45.0,
                    near: 0.1,
                    far: 10.0,
                },
                lights: vec![],
                tone_mapping: crate::ir::scene3::ToneMapping3::None,
                tone_mapping_exposure: 1.0,
                output_color_space: crate::ir::scene3::OutputColorSpace3::Linear,
                environment: None,
                meshes: vec![crate::ir::scene3::Mesh3 {
                    geometry: crate::ir::scene3::Geometry3::Inline {
                        positions: vec![[0.0; 3]; 3],
                        normals: vec![[0.0, 0.0, 1.0]; 3],
                        indices: vec![0, 1, 2],
                        uvs: vec![],
                        colors: vec![],
                        colors1: vec![],
                    },
                    material: crate::ir::scene3::Material3 {
                        base_color: [1.0; 4],
                        shading: crate::ir::scene3::Shading3::Lambert,
                        displacement: None,
                        map: None,
                        textures: vec![],
                        transparent: false,
                    },
                    matrix,
                    render_order: 0,
                }],
            }
        };
        store.record_scene3("c3d", scene([1.0; 16]));
        // 同 id 重复提交 → 覆盖（最后一次赢），不是追加。
        store.record_scene3("c3d", scene([2.0; 16]));
        assert_eq!(store.scene3s.len(), 1);
        assert_eq!(store.scene3s["c3d"].meshes[0].matrix, [2.0; 16]);

        store.reset_for_frame(1, 30);
        assert!(store.scene3s.is_empty());
        assert!(store.snapshot_mutations().scene3_mutations.is_empty());
    }

    #[test]
    fn geometry_registry_survives_frame_reset() {
        use crate::ir::scene3::Geometry3;
        let mut store = MutationStore::default();
        let geo = std::sync::Arc::new(Geometry3::Inline {
            positions: vec![[0.0; 3]; 4],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            uvs: vec![],
            colors: vec![],
            colors1: vec![],
        });
        store.record_geometry("prism", geo.clone());
        store.record_geometry("prism", geo.clone()); // 幂等覆盖
        store.reset_for_frame(7, 30);
        let registry = store.geometry_registry();
        assert_eq!(registry.len(), 1, "注册表不随帧清除");
        assert_eq!(registry["prism"].as_ref(), geo.as_ref());
    }

    #[test]
    fn text_shadow_write_applies_overwrite_semantics_and_replays() {
        let mut store = MutationStore::default();

        let payload = json!([
            [12.0, 9.0, 74.25, "rgba(0,0,0,1)"],
            [8.0, 6.0, 47.25, "rgba(0,0,0,0.9)"],
            [0.0, 0.0, 60.75, "rgba(0,0,0,0.7)"]
        ]);
        store.write_style_value("hl", "textShadow", payload);
        store.write_style_value("hl", "textShadow", json!([[4.0, 3.0, 20.0, "#000000"]]));

        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("hl").expect("hl recorded");
        let shadows = entry.text_shadows.as_ref().expect("text shadows set");
        // Overwrite semantics: the second write replaces the whole stack.
        assert_eq!(shadows.len(), 1);
        assert_eq!(shadows[0].offset_x, 4.0);
        assert_eq!(shadows[0].offset_y, 3.0);
        assert_eq!(shadows[0].blur_sigma, 20.0);

        // Applied to a NodeStyle, the stack fully replaces the base shadows.
        let mut style = crate::style::NodeStyle::default();
        style.text_shadows = vec![crate::style::TextShadow {
            offset_x: 99.0,
            offset_y: 99.0,
            blur_sigma: 99.0,
            color: crate::style::ColorToken::Custom(1, 2, 3, 255),
        }];
        entry.apply_to(&mut style);
        assert_eq!(style.text_shadows.len(), 1);
        assert_eq!(style.text_shadows[0].offset_x, 4.0);

        // Snapshot replay keeps the same stack (precomputed host path).
        let mut replay = MutationStore::default();
        use super::MutationRecorder as _;
        snap.apply_to_recorder(&mut replay);
        let replayed = replay
            .read_style_value("hl", "textShadow")
            .expect("replayed textShadow");
        let parsed = crate::script::mutations::text_shadows_from_value(&replayed)
            .expect("replayed payload parses");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].blur_sigma, 20.0);
    }

    #[test]
    fn text_shadow_write_rejects_malformed_payload() {
        let mut store = MutationStore::default();
        // Missing color entry.
        store.write_style_value("hl", "textShadow", json!([[1.0, 2.0, 3.0]]));
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("hl").expect("entry exists");
        assert!(entry.text_shadows.is_none());

        // Not an array at all.
        store.write_style_value("hl2", "textShadow", json!("12px 9px 74px black"));
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("hl2").expect("entry exists");
        assert!(entry.text_shadows.is_none());
    }

    #[test]
    fn css_filter_values_round_trip_through_value_api() {
        let mut store = MutationStore::default();

        store.write_style_value("node-a", "brightness", json!(0.5));
        store.write_style_value("node-a", "hueRotate", json!(45.0));
        store.write_style_value("node-a", "blur", json!(6.0));

        assert_eq!(
            store.read_style_value("node-a", "brightness"),
            Some(json!(0.5))
        );
        assert_eq!(
            store.read_style_value("node-a", "hueRotate"),
            Some(json!(45.0))
        );
        assert_eq!(store.read_style_value("node-a", "blur"), Some(json!(6.0)));

        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.css_filter.value("brightness"), Some(0.5));
        assert_eq!(entry.css_filter.value("hueRotate"), Some(45.0));
        assert_eq!(entry.css_filter.value("blur"), Some(6.0));
    }

    #[test]
    fn css_filter_string_preserves_operation_order() {
        let mut store = MutationStore::default();

        store.write_style_value("node-a", "filter", json!("brightness(0.5) contrast(2)"));
        store.write_style_value("node-b", "filter", json!("contrast(2) brightness(0.5)"));

        let snap = store.snapshot_mutations();
        let a = &snap.mutations.get("node-a").unwrap().css_filter;
        let b = &snap.mutations.get("node-b").unwrap().css_filter;
        assert_ne!(a, b, "CSS filter order is semantically significant");
        assert_eq!(
            a.to_css_string(),
            "brightness(0.5) contrast(2)",
            "filter string order must round-trip"
        );
        assert_eq!(
            b.to_css_string(),
            "contrast(2) brightness(0.5)",
            "filter string order must round-trip"
        );
    }

    #[test]
    fn css_filter_ordered_value_preserves_operation_order() {
        let mut store = MutationStore::default();

        store.write_style_value(
            "node-a",
            "filter",
            json!({
                "ops": [
                    { "kind": "brightness", "value": 0.5 },
                    { "kind": "contrast", "value": 2.0 },
                    { "kind": "brightness", "value": 0.75 }
                ]
            }),
        );

        assert_eq!(
            store.read_style_value("node-a", "filter"),
            Some(json!({
                "ops": [
                    { "kind": "brightness", "value": 0.5 },
                    { "kind": "contrast", "value": 2.0 },
                    { "kind": "brightness", "value": 0.75 }
                ]
            }))
        );
        let snap = store.snapshot_mutations();
        let filter = &snap.mutations.get("node-a").unwrap().css_filter;
        assert_eq!(
            filter.to_css_string(),
            "brightness(0.5) contrast(2) brightness(0.75)"
        );
        assert_eq!(filter.value("brightness"), Some(0.75));
    }

    #[test]
    fn record_draw_picture_stores_subtree_reference() {
        let mut store = MutationStore::default();
        store.record_draw_picture("stage", "stage", 4.0, 5.0);

        let snap = store.snapshot_mutations();
        let commands = &snap.canvas_mutations.get("stage").unwrap().commands;
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            crate::ir::draw_op::DrawOp::DrawSubtreePicture { owner_id, x, y } => {
                assert_eq!(owner_id, "stage");
                assert_eq!(*x, 4.0);
                assert_eq!(*y, 5.0);
            }
            other => panic!("expected DrawSubtreePicture, got {:?}", other),
        }
    }

    #[test]
    fn record_canvas_runtime_effect_pushes_script_effect() {
        use crate::ir::draw_op::Rect4;
        use crate::ir::draw_types::{ImageRef, ScriptRuntimeEffectChild};
        let mut store = MutationStore::default();
        store.record_canvas_runtime_effect(
            "s1-canvas",
            "half4 main(float2 p){return half4(1);}".to_string(),
            vec![1u8, 2, 3, 4, 5, 6, 7, 8],
            vec![ScriptRuntimeEffectChild::Image(ImageRef::Static {
                asset_id: "decor".into(),
            })],
            Rect4 {
                x: 0.0,
                y: 0.0,
                width: 360.0,
                height: 480.0,
            },
        );

        let snap = store.snapshot_mutations();
        let commands = &snap.canvas_mutations.get("s1-canvas").unwrap().commands;
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            crate::ir::draw_op::DrawOp::ScriptRuntimeEffect {
                sksl,
                uniforms_bytes,
                children,
                dst,
            } => {
                assert!(sksl.contains("half4"));
                assert_eq!(uniforms_bytes.len(), 8);
                assert_eq!(children.len(), 1);
                assert_eq!(dst.width, 360.0);
            }
            other => panic!("expected ScriptRuntimeEffect, got {:?}", other),
        }
    }

    #[test]
    fn dispatch_canvas_runtime_effect_draw_records_script_effect() {
        use crate::script::dispatch::dispatch_binding;
        use serde_json::json;
        let mut store = MutationStore::default();
        let args = vec![
            json!("s1-canvas"),
            json!("half4 main(float2 p){return half4(1);}"),
            json!([0.5, 1.0, 0.0, 0.0]),
            json!(
                r#"[{"__opencatShader":"image","assetId":"decor","tileX":"clamp","tileY":"clamp"}]"#
            ),
            json!(0.0),
            json!(0.0),
            json!(360.0),
            json!(480.0),
        ];
        dispatch_binding(&mut store, "canvas_runtime_effect_draw", &args)
            .expect("binding should dispatch");
        let snap = store.snapshot_mutations();
        let commands = &snap.canvas_mutations.get("s1-canvas").unwrap().commands;
        assert_eq!(commands.len(), 1);
        matches!(
            commands[0],
            crate::ir::draw_op::DrawOp::ScriptRuntimeEffect { .. }
        );
    }
}
