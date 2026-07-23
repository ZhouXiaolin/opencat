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

/// Owned RGBA bitmap for one generated image.
#[derive(Clone, Debug)]
pub struct GeneratedImageEntry {
    pub width: u32,
    pub height: u32,
    /// RGBA_8888, unpremultiplied. Length must be `width * height * 4`.
    pub rgba: Arc<[u8]>,
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
#[derive(Clone, Debug, Default)]
pub struct GeneratedImageTable {
    entries: HashMap<GeneratedImageId, GeneratedImageEntry>,
}

impl GeneratedImageTable {
    pub fn new() -> Self {
        Self::default()
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

    /// Register a generated image under `id`.
    ///
    /// - Equal content already present → no-op (`Ok(())`).
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
        if let Some(existing) = self.entries.get(&id) {
            let dims_match = existing.dimensions_match(width, height);
            let content_match = dims_match && existing.rgba.as_ref() == rgba.as_ref();
            if content_match {
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
            },
        );
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
