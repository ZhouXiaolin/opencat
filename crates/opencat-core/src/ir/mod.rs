pub mod asset_id;
pub mod cache;
pub mod composition_info;
pub mod draw_encoding;
pub mod draw_frame;
pub mod draw_op;
pub mod draw_types;
pub mod generated_image;
pub mod media_plan;
pub mod schema_gen;

pub use asset_id::*;
pub use composition_info::CompositionInfo;
pub use draw_frame::{DrawOpFrame, RenderFrame};
pub use generated_image::{
    GeneratedImageCollision, GeneratedImageEntry, GeneratedImageId, GeneratedImageTable,
};
pub use media_plan::{FrameGeneratedImage, FrameMediaPlan};

pub use draw_encoding::section as ir_section;
/// Host-facing wire entry points. Encoder intermediates stay on `draw_encoding`
/// (crate-visible for tests) and are not re-exported at the `ir` surface.
pub use draw_encoding::{
    EncodeError, IR_MAGIC, IR_VERSION, encode_ir_envelope, intern_image_strings,
};
