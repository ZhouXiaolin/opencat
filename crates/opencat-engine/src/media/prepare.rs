use std::collections::HashMap;

use crate::executor::EnginePreparedFrameMedia;
use crate::resource::loader::EngineLoader;
use crate::resource::media::MediaContext;
use anyhow::Result;
use opencat_core::ir::draw_types::ImageRef;
use opencat_core::platform::media::{FrameMediaPlan, MediaError, PrepareMode};
use opencat_core::probe::{AssetHandle, AssetLoader};
use opencat_core::resource::asset_id::AssetId;
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, RuntimeEffect, images};

pub fn prepare_frame_with_loader(
    plan: &FrameMediaPlan,
    loader: &EngineLoader,
    media_ctx: &mut MediaContext,
) -> Result<EnginePreparedFrameMedia> {
    let mut sk_images = Vec::new();
    let mut image_index = HashMap::new();

    for image_ref in &plan.images {
        match image_ref {
            ImageRef::Static { asset_id } => {
                let aid = AssetId(asset_id.clone());
                if let Some(handle) = loader.handle(&aid) {
                    if let Some(path) = handle.local_path() {
                        if let Ok(bytes) = std::fs::read(path) {
                            if let Some(sk_image) = Image::from_encoded(Data::new_copy(&bytes)) {
                                let idx = sk_images.len();
                                sk_images.push(sk_image);
                                image_index.insert(image_ref.clone(), idx);
                            }
                        }
                    }
                }
            }
            ImageRef::VideoFrame {
                asset_id,
                frame_index,
            } => {
                let aid = AssetId(asset_id.clone());
                if let Some(handle) = loader.handle(&aid) {
                    if let Some(path) = handle.local_path() {
                        if let Ok(frame) = media_ctx.frame_rgba_by_path(path, *frame_index) {
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
                                let idx = sk_images.len();
                                sk_images.push(sk_image);
                                image_index.insert(image_ref.clone(), idx);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut runtime_effects = Vec::with_capacity(plan.runtime_effects.len());
    for effect_ref in &plan.runtime_effects {
        let effect = RuntimeEffect::make_for_shader(&effect_ref.sksl, None).map_err(|error| {
            MediaError(format!(
                "RuntimeEffect {:#x} compile failed: {}",
                effect_ref.hash, error
            ))
        })?;
        runtime_effects.push(effect);
    }

    Ok(EnginePreparedFrameMedia {
        images: sk_images,
        image_index,
        runtime_effects,
    })
}

pub fn prepare_frame(
    plan: &FrameMediaPlan,
    _mode: PrepareMode,
    asset_paths: &crate::resource::AssetPathStore,
    video: *mut MediaContext,
) -> Result<EnginePreparedFrameMedia, MediaError> {
    use opencat_core::resource::AssetPathBlobStore;
    use opencat_core::resource::asset_id::AssetId;
    use opencat_core::resource::blob_store::BlobStore;

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
            ImageRef::VideoFrame {
                asset_id,
                frame_index,
            } => {
                let video_ref = unsafe { video.as_mut() };
                if let Some(ctx) = video_ref {
                    let aid = AssetId(asset_id.clone());
                    let path = asset_paths.path(&aid).unwrap_or_else(|| std::path::Path::new(&aid.0));
                    if let Ok(frame) = ctx.frame_rgba_by_path(path, *frame_index) {
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

    let mut runtime_effects = Vec::with_capacity(plan.runtime_effects.len());
    for effect_ref in &plan.runtime_effects {
        let effect = RuntimeEffect::make_for_shader(&effect_ref.sksl, None).map_err(|error| {
            MediaError(format!(
                "RuntimeEffect {:#x} compile failed: {}",
                effect_ref.hash, error
            ))
        })?;
        runtime_effects.push(effect);
    }

    Ok(EnginePreparedFrameMedia {
        images,
        image_index,
        runtime_effects,
    })
}
