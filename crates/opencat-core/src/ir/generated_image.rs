//! Pipeline-owned table of core-rasterized images (color-emoji / bitmap glyphs).
//!
//! Color emoji is the one image resource core must generate itself: the font
//! database, rasterization, and glyph-width math all live in core, so the RGBA
//! cannot be re-derived by a host without re-implementing core's font pipeline.
//! The table lets those bitmaps flow through the normal frame contract
//! ([`crate::ir::draw_types::ImageRef::Generated`]) instead of being smuggled
//! out as synthetic external `glyph:*` assets.
//!
//! Lifecycle spans the whole pipeline: a fresh pipeline and the same pipeline
//! reused across frames must produce identical [`GeneratedImageId`]s for the
//! same glyph, because the id is a deterministic function of the glyph cache
//! key — never insertion order.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use anyhow::{Result, bail};

/// Stable identifier for a core-generated image.
///
/// Derived deterministically from the producing glyph's cache key (font id +
/// glyph id + size + subpixel bin), so the same glyph on a fresh vs reused
/// pipeline always yields the same id. It is deliberately NOT an insertion
/// index, which would be call-history-dependent and break the
/// render-determinism contract.
///
/// Note: unlike the table-index id newtypes in `draw_types.rs` (which are
/// `u32`), this carries a `u64` because the glyph cache key is a full hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GeneratedImageId(pub u64);

impl GeneratedImageId {
    /// Deterministic id from a frame-scoped cache key (canvas `putImageData`
    /// / surface auto-bake path). Same key ⇒ same id on fresh and reused
    /// pipelines (the [`GeneratedImageTable`] idempotency contract above).
    /// Single shared implementation — every key→id call site must route
    /// through this so engine/core can never drift apart.
    pub fn from_key(key: &str) -> Self {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        GeneratedImageId(hasher.finish())
    }
}

/// Host-side handle to a generated image that lives on the GPU.
///
/// Core never interprets this value — it only carries it from the rasterizer
/// that produced the image to the host that will consume it. The GPU pipeline
/// uses it to hand a 3D render target's texture to Skia without a CPU round
/// trip ([`Scene3Raster::render`](crate::ir::scene3::Scene3Raster::render)
/// returns it alongside the — possibly empty — CPU pixels).
///
/// Core does not know which native API produced the texture, and it does not
/// carry the texture itself. [`Self::token`] is a key into the host's shared
/// texture registry (`opencat_gpu_native::TextureRegistry`): the producer
/// publishes the texture there, the consumer resolves it, and the consumer
/// releases it once done.
///
/// The wasm path never sets it, so a `None` here always means "use `rgba`".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GpuImageHandle {
    /// Key into the host's shared texture registry. 0 means "no texture" —
    /// the producer is not on a GPU-shared backend.
    pub token: u64,
}

/// Owned RGBA bitmap for one generated image.
#[derive(Clone, Debug)]
pub struct GeneratedImageEntry {
    pub width: u32,
    pub height: u32,
    /// RGBA_8888, unpremultiplied. Length must be `width * height * 4`.
    ///
    /// Empty when [`Self::gpu`] is set (the pixels never left the GPU).
    pub rgba: Arc<[u8]>,
    /// GPU-resident pixels, when the host rasterized this image on the GPU.
    ///
    /// When set, `rgba` is empty and consumers must resolve the handle through
    /// the host instead of reading pixels. Core's length validation is skipped
    /// in that case — the host owns the invariant.
    pub gpu: Option<GpuImageHandle>,
}

impl GeneratedImageEntry {
    fn dimensions_match(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}

/// Pipeline-owned store of core-rasterized images, keyed by stable
/// [`GeneratedImageId`].
///
/// Re-inserting the same id with identical `(width, height, rgba)` is an
/// idempotent no-op (the common case — the same emoji rendered on many frames).
/// Re-inserting the same id with differing content is a hard error: a stable id
/// must map to exactly one image, otherwise fresh/reused pipelines could
/// silently draw the wrong pixels.
///
/// ## Frame watermark
///
/// Entries outlive one frame: `frame = f(t)` re-evaluates the script every frame
/// and re-emits the same keys, but a host may also resolve a plan a frame later
/// (the engine caches Skia images by id and the encoder consumes frames
/// asynchronously). An entry is therefore kept until [`Self::advance_frame`]
/// retires it — that call declares "no future frame will ask for anything
/// older". Without it the table is an unbounded leak for the one legitimate
/// per-frame-key pattern (auto-bake minting a fresh key for a surface the
/// script rewrites every frame), which grows the resident set by one full RGBA
/// buffer per frame and OOM-kills a 49s composition at 1920x1080.
#[derive(Clone, Debug, Default)]
pub struct GeneratedImageTable {
    entries: HashMap<GeneratedImageId, GeneratedImageEntry>,
    /// Composition frame index that last wrote the entry.
    last_seen: HashMap<GeneratedImageId, u64>,
    /// Frame index of the most recent [`Self::advance_frame`].
    clock: u64,
    /// Entries untouched for more than this many `advance_frame` calls drop.
    max_age_frames: u32,
}

impl GeneratedImageTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retain an entry for at most `max_age_frames` advances after it was last
    /// written (see the frame-watermark note above). `usize::MAX` keeps
    /// everything, which restores the pre-watermark behaviour.
    pub fn with_max_age(max_age_frames: usize) -> Self {
        Self {
            entries: HashMap::new(),
            last_seen: HashMap::new(),
            clock: 0,
            max_age_frames: max_age_frames.min(u32::MAX as usize) as u32,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a generated image by stable id.
    pub fn get(&self, id: &GeneratedImageId) -> Option<&GeneratedImageEntry> {
        self.entries.get(id)
    }

    /// Mark that the pipeline moved on to composition frame `frame_index`, and
    /// drop entries no frame has written for more than `max_age_frames` frames.
    ///
    /// The pipeline calls this once per rendered frame. Per-frame-key authors
    /// get a bounded table instead of unbounded growth; stable-key authors are
    /// unaffected because every frame reinserts (and so refreshes) the age.
    ///
    /// Age is measured against the frame that LAST wrote the entry, not against
    /// the number of calls, so the pipeline's own out-of-order / repeat calls
    /// (preview scrubbing, `render_frame(1)` then `render_frame(0)`) never
    /// expire an entry the current frame still needs.
    pub fn advance_frame(&mut self, frame_index: u64) {
        self.clock = frame_index;
        let max_age = self.max_age_frames as u64;
        let dead: Vec<GeneratedImageId> = self
            .last_seen
            .iter()
            .filter(|(_, seen)| frame_index.saturating_sub(**seen) > max_age)
            .map(|(id, _)| *id)
            .collect();
        for id in dead {
            self.entries.remove(&id);
            self.last_seen.remove(&id);
        }
    }

    /// Mark `id` as used by the current frame without new pixels.
    ///
    /// Used by paths that reuse a previously recorded frame (the whole-frame
    /// scene snapshot) — the pixels are still being drawn, so they must not
    /// expire out from under a later frame's media plan.
    pub fn touch(&mut self, id: GeneratedImageId) {
        if self.entries.contains_key(&id) {
            self.last_seen.insert(id, self.clock);
        }
    }

    /// Drop every entry and reset all ages. Hosts call this when switching
    /// compositions so a closed composition's bitmaps are returned at once.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.last_seen.clear();
        self.clock = 0;
    }

    /// Register a generated image under `id` (CPU pixels).
    ///
    /// - Equal content already present → no-op (`Ok(())`).
    ///   A no-op still refreshes the frame watermark, so an id every frame
    ///   re-registers (the stable-key case) never expires.
    /// - Same id, different `(width, height, rgba)` → [`GeneratedImageCollision`].
    ///   This signals that two different bitmaps hashed to the same id, which
    ///   must never happen for a correct cache key and would otherwise produce
    ///   silent misrendering.
    pub fn insert(
        &mut self,
        id: GeneratedImageId,
        width: u32,
        height: u32,
        rgba: Arc<[u8]>,
    ) -> Result<()> {
        self.insert_impl(id, width, height, rgba, None)
    }

    /// Register a generated image whose pixels live on the GPU.
    ///
    /// `gpu` is an opaque host handle (see [`GpuImageHandle`]); core only
    /// carries it. `rgba` may be empty — core skips the length check because
    /// the host owns that invariant in this mode.
    pub fn insert_gpu(
        &mut self,
        id: GeneratedImageId,
        width: u32,
        height: u32,
        gpu: GpuImageHandle,
    ) -> Result<()> {
        self.insert_impl(id, width, height, Arc::from(Vec::new()), Some(gpu))
    }

    fn insert_impl(
        &mut self,
        id: GeneratedImageId,
        width: u32,
        height: u32,
        rgba: Arc<[u8]>,
        gpu: Option<GpuImageHandle>,
    ) -> Result<()> {
        // GPU-resident pixels carry no CPU copy to validate.
        if gpu.is_none() {
            let expected_len = width as usize * height as usize * 4;
            if rgba.len() != expected_len {
                bail!(
                    "generated image {id:?}: rgba length {} does not match {}x{} (expected {})",
                    rgba.len(),
                    width,
                    height,
                    expected_len
                );
            }
        }
        if let Some(existing) = self.entries.get(&id) {
            let dims_match = existing.dimensions_match(width, height);
            // Same GPU handle (or same CPU bytes) is the idempotent case.
            let content_match = dims_match
                && match (existing.gpu, gpu) {
                    (Some(a), Some(b)) => a == b,
                    (None, None) => existing.rgba.as_ref() == rgba.as_ref(),
                    _ => false,
                };
            if content_match {
                // Idempotent content, but NOT an idempotent age: a frame that
                // asks for this image again used it, so it stays alive.
                self.last_seen.insert(id, self.clock);
                return Ok(());
            }
            bail!(GeneratedImageCollision {
                id,
                existing_width: existing.width,
                existing_height: existing.height,
                new_width: width,
                new_height: height,
                // Distinguish "same size, different pixels" from "different
                // size" — both are collisions, but the cause differs.
                rgba_mismatch: dims_match,
            });
        }
        self.entries.insert(
            id,
            GeneratedImageEntry {
                width,
                height,
                rgba,
                gpu,
            },
        );
        self.last_seen.insert(id, self.clock);
        Ok(())
    }
}

/// Error raised when a stable [`GeneratedImageId`] is reinserted with
/// different RGBA content or dimensions. A correct deterministic cache key
/// never produces this; surfacing it explicitly avoids silent misrendering.
#[derive(Debug)]
pub struct GeneratedImageCollision {
    pub id: GeneratedImageId,
    pub existing_width: u32,
    pub existing_height: u32,
    pub new_width: u32,
    pub new_height: u32,
    /// `true` when dimensions matched but the RGBA bytes differed (a content
    /// collision); `false` when the dimensions themselves differed.
    pub rgba_mismatch: bool,
}

impl fmt::Display for GeneratedImageCollision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rgba_mismatch {
            write!(
                f,
                "generated image collision on {:?}: same {}x{} but RGBA content differs",
                self.id, self.existing_width, self.existing_height
            )
        } else {
            write!(
                f,
                "generated image collision on {:?}: existing {}x{} vs new {}x{}",
                self.id, self.existing_width, self.existing_height, self.new_width, self.new_height
            )
        }
    }
}

impl std::error::Error for GeneratedImageCollision {}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(value: u8, w: u32, h: u32) -> Arc<[u8]> {
        Arc::from(vec![value; w as usize * h as usize * 4])
    }

    #[test]
    fn insert_then_get_roundtrips() {
        let mut table = GeneratedImageTable::new();
        let id = GeneratedImageId(42);
        table
            .insert(id, 4, 4, rgba(0xAA, 4, 4))
            .expect("first insert");
        let entry = table.get(&id).expect("present");
        assert_eq!(entry.width, 4);
        assert_eq!(entry.height, 4);
        assert_eq!(entry.rgba.as_ref(), rgba(0xAA, 4, 4).as_ref());
    }

    #[test]
    fn identical_reinsert_is_idempotent() {
        // The same emoji appears on many frames: re-inserting identical content
        // must be a silent no-op, not an error.
        let mut table = GeneratedImageTable::new();
        let id = GeneratedImageId(7);
        table.insert(id, 2, 2, rgba(1, 2, 2)).expect("first insert");
        table
            .insert(id, 2, 2, rgba(1, 2, 2))
            .expect("idempotent reinsert");
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn same_id_different_rgba_is_collision() {
        let mut table = GeneratedImageTable::new();
        let id = GeneratedImageId(99);
        table.insert(id, 2, 2, rgba(1, 2, 2)).expect("first insert");
        let err = table
            .insert(id, 2, 2, rgba(2, 2, 2))
            .expect_err("conflicting content must collide");
        let collision = err
            .downcast_ref::<GeneratedImageCollision>()
            .expect("collision error type");
        assert_eq!(collision.id, id);
        // Same dimensions, different pixels: flagged as a content (RGBA) mismatch.
        assert!(collision.rgba_mismatch);
        assert_eq!(collision.existing_width, 2);
        assert_eq!(collision.new_width, 2);
    }

    #[test]
    fn same_id_different_dimensions_is_collision() {
        let mut table = GeneratedImageTable::new();
        let id = GeneratedImageId(5);
        table.insert(id, 2, 2, rgba(1, 2, 2)).expect("first insert");
        let err = table
            .insert(id, 4, 4, rgba(1, 4, 4))
            .expect_err("different dims must collide");
        let collision = err
            .downcast_ref::<GeneratedImageCollision>()
            .expect("collision error type");
        assert!(
            !collision.rgba_mismatch,
            "dimension mismatch is not an rgba mismatch"
        );
        assert_eq!(collision.existing_width, 2);
        assert_eq!(collision.new_width, 4);
    }

    #[test]
    fn per_frame_keys_are_retired_instead_of_accumulating() {
        // The OOM regression: a surface the script rewrites every frame
        // auto-bakes a NEW id every frame. Without the watermark the table
        // grows by one full RGBA buffer per frame (1920x1080 => 8 MB), so a
        // 50s composition reaches ~1.2 GB and the process is OOM-killed.
        let mut table = GeneratedImageTable::with_max_age(2);
        for frame in 0..200u64 {
            table.advance_frame(frame);
            let key = format!("lambda_node_{frame:016x}");
            let id = GeneratedImageId::from_key(&key);
            table
                .insert(id, 1920, 1080, rgba(1, 1920, 1080))
                .expect("distinct keys never collide");
            assert!(
                table.len() <= 3,
                "frame {frame}: table kept {} entries (watermark + this frame)",
                table.len()
            );
        }
    }

    #[test]
    fn stable_keys_survive_every_frame() {
        // The emoji case: the same id is re-registered every frame, so the
        // watermark must never retire it.
        let mut table = GeneratedImageTable::with_max_age(2);
        let id = GeneratedImageId::from_key("emoji-smile");
        for frame in 0..500u64 {
            table.advance_frame(frame);
            table.insert(id, 32, 32, rgba(9, 32, 32)).expect("insert");
        }
        assert_eq!(table.len(), 1);
        assert!(table.get(&id).is_some());
    }

    #[test]
    fn entry_survives_the_frames_after_it_was_written() {
        // A host may resolve frame N's media plan while frame N+1 or N+2 runs,
        // so an entry must outlive `max_age_frames` frames past its write.
        let mut table = GeneratedImageTable::with_max_age(2);
        table.advance_frame(10);
        table.insert(GeneratedImageId::from_key("cc-frame"), 64, 64, rgba(3, 64, 64))
            .expect("insert");
        table.advance_frame(11);
        assert!(table.get(&GeneratedImageId::from_key("cc-frame")).is_some());
        table.advance_frame(12);
        assert!(table.get(&GeneratedImageId::from_key("cc-frame")).is_some());
        table.advance_frame(13);
        assert!(table.get(&GeneratedImageId::from_key("cc-frame")).is_none());
    }

    #[test]
    fn out_of_order_and_repeated_frames_keep_live_entries() {
        // Preview scrubbing renders frames out of order and repeats them. Age
        // is measured from the frame that last wrote the entry, so jumping back
        // must not expire an image the current frame still draws.
        let mut table = GeneratedImageTable::with_max_age(2);
        let id = GeneratedImageId::from_key("emoji");
        for &frame in &[0u64, 1, 2, 3, 1, 0, 0, 5, 3] {
            table.advance_frame(frame);
            table.insert(id, 16, 16, rgba(7, 16, 16)).expect("insert");
            assert!(
                table.get(&id).is_some(),
                "frame {frame} must resolve its own write"
            );
        }
    }

    #[test]
    fn touch_keeps_a_reused_frame_s_pixels_alive() {
        // The whole-frame scene snapshot redraws previously recorded generated
        // images; without a touch they would expire mid-composition.
        let mut table = GeneratedImageTable::with_max_age(2);
        let id = GeneratedImageId::from_key("emoji");
        table.advance_frame(0);
        table.insert(id, 16, 16, rgba(7, 16, 16)).expect("insert");
        for frame in 1..=4u64 {
            table.advance_frame(frame);
            table.touch(id);
            assert!(table.get(&id).is_some(), "touched on frame {frame}");
        }
    }

    #[test]
    fn unlimited_age_restores_the_previous_behaviour() {
        // `usize::MAX` keeps every entry: an explicit opt-out of the watermark.
        let mut table = GeneratedImageTable::with_max_age(usize::MAX);
        for frame in 0..64u64 {
            table.advance_frame(frame);
            table
                .insert(GeneratedImageId(frame + 1), 8, 8, rgba(1, 8, 8))
                .expect("insert");
        }
        assert_eq!(table.len(), 64);
    }

    #[test]
    fn mismatched_rgba_length_is_rejected() {
        let mut table = GeneratedImageTable::new();
        let err = table
            .insert(GeneratedImageId(1), 4, 4, rgba(0, 2, 2))
            .expect_err("length mismatch");
        assert!(
            err.to_string().contains("does not match"),
            "expected length-mismatch error, got: {err}"
        );
    }

    #[test]
    fn distinct_ids_coexist() {
        let mut table = GeneratedImageTable::new();
        table
            .insert(GeneratedImageId(1), 2, 2, rgba(1, 2, 2))
            .expect("a");
        table
            .insert(GeneratedImageId(2), 3, 3, rgba(2, 3, 3))
            .expect("b");
        assert_eq!(table.len(), 2);
        assert!(table.get(&GeneratedImageId(1)).is_some());
        assert!(table.get(&GeneratedImageId(2)).is_some());
        assert!(table.get(&GeneratedImageId(3)).is_none());
    }

    #[test]
    fn empty_table_reports_empty() {
        let table = GeneratedImageTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }
}
