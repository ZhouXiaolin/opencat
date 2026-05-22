use opencat_core::platform::media::{FrameMediaPlan, MediaError, PrepareMode};
use opencat_core::platform::video::VideoFrameProvider;
use crate::executor::EnginePreparedFrameMedia;
use crate::resource::media::MediaContext;

pub fn prepare_frame(
    plan: &FrameMediaPlan,
    _mode: PrepareMode,
    asset_paths: &crate::resource::AssetPathStore,
    video: *mut MediaContext,
) -> Result<EnginePreparedFrameMedia, MediaError> {
    use std::collections::HashMap;
    use opencat_core::draw::types::ImageRef;
    use opencat_core::resource::AssetPathBlobStore;
    use opencat_core::resource::asset_id::AssetId;
    use opencat_core::resource::blob_store::BlobStore;
    use skia_safe::{images, Image, Data, AlphaType, ColorType, ImageInfo};

    let blob_store = AssetPathBlobStore::new(asset_paths);
    let mut images = Vec::new();
    let mut image_index = HashMap::new();

    for image_ref in &plan.images {
        match image_ref {
            ImageRef::Static { asset_id } => {
                let aid = AssetId(asset_id.clone());
                if let Some(bytes) = blob_store.read(&aid) {
                    if let Some(sk_image) = Image::from_encoded(Data::new_copy(&bytes)) {
                        let idx = images.len();
                        images.push(sk_image);
                        image_index.insert(image_ref.clone(), idx);
                    }
                }
            }
            ImageRef::VideoFrame { asset_id, frame_index } => {
                let video_ref = unsafe { video.as_mut() };
                if let Some(ctx) = video_ref {
                    let aid = AssetId(asset_id.clone());
                    if let Ok(frame) = ctx.frame_rgba(&aid, *frame_index) {
                        let info = ImageInfo::new(
                            (frame.width as i32, frame.height as i32),
                            ColorType::RGBA8888,
                            AlphaType::Unpremul,
                            None,
                        );
                        if let Some(sk_image) = images::raster_from_data(
                            &info,
                            Data::new_copy(&frame.data),
                            frame.width as usize * 4,
                        ) {
                            let idx = images.len();
                            images.push(sk_image);
                            image_index.insert(image_ref.clone(), idx);
                        }
                    }
                }
            }
        }
    }

    Ok(EnginePreparedFrameMedia {
        images,
        image_index,
        runtime_effects: Vec::new(),
    })
}
