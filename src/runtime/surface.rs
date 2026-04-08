use anyhow::{Result, anyhow};

#[cfg(target_os = "macos")]
use foreign_types::ForeignType;
#[cfg(target_os = "macos")]
use metal::{
    Device, MTLOrigin, MTLPixelFormat, MTLRegion, MTLSize, MTLStorageMode, MTLTextureType,
    MTLTextureUsage, Texture, TextureDescriptor,
};
#[cfg(target_os = "macos")]
use skia_safe::gpu::{self, backend_render_targets, mtl};

use crate::{
    render::{RenderSession, render_frame_to_target},
    runtime::target::{RenderFrameViewKind, RenderTargetHandle},
    scene::composition::Composition,
};

#[cfg(target_os = "macos")]
pub(crate) struct MetalEncodeBridge {
    _target: Box<MetalOffscreenTarget>,
    handle: RenderTargetHandle,
}

#[cfg(target_os = "macos")]
impl MetalEncodeBridge {
    pub(crate) fn new(width: i32, height: i32) -> Result<Self> {
        let mut target = Box::new(MetalOffscreenTarget::new(width, height)?);
        let target_ptr = target.as_mut() as *mut MetalOffscreenTarget as *mut std::ffi::c_void;
        let handle = RenderTargetHandle::new(
            RenderFrameViewKind::DrawContext2D,
            target_ptr,
            MetalOffscreenTarget::begin_frame_bridge,
            MetalOffscreenTarget::end_frame_bridge,
        )
        .with_frame_view_resolver(MetalOffscreenTarget::resolve_skia_canvas_bridge)
        .with_readback_rgba(MetalOffscreenTarget::readback_rgba_bridge);
        Ok(Self {
            _target: target,
            handle,
        })
    }

    pub(crate) fn render_frame_rgba(
        &mut self,
        composition: &Composition,
        frame_index: u32,
        session: &mut RenderSession,
    ) -> Result<Vec<u8>> {
        render_frame_to_target(composition, frame_index, session, &mut self.handle)?;
        self.handle.readback_rgba()
    }
}

#[cfg(target_os = "macos")]
struct MetalOffscreenTarget {
    device: Device,
    skia: gpu::DirectContext,
    texture: Texture,
    width: i32,
    height: i32,
    current_surface: Option<skia_safe::Surface>,
}

#[cfg(target_os = "macos")]
impl MetalOffscreenTarget {
    fn new(width: i32, height: i32) -> Result<Self> {
        let device = Device::system_default().ok_or_else(|| anyhow!("no Metal device found"))?;
        let command_queue = device.new_command_queue();
        let backend = unsafe {
            mtl::BackendContext::new(
                device.as_ptr() as mtl::Handle,
                command_queue.as_ptr() as mtl::Handle,
            )
        };
        let skia = gpu::direct_contexts::make_metal(&backend, None)
            .ok_or_else(|| anyhow!("failed to create Skia Metal direct context"))?;
        let texture = Self::create_texture(&device, width, height);
        Ok(Self {
            device,
            skia,
            texture,
            width,
            height,
            current_surface: None,
        })
    }

    fn create_texture(device: &Device, width: i32, height: i32) -> Texture {
        let descriptor = TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        descriptor.set_width(width.max(1) as u64);
        descriptor.set_height(height.max(1) as u64);
        descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        descriptor.set_storage_mode(MTLStorageMode::Shared);
        descriptor.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        device.new_texture(&descriptor)
    }

    fn ensure_size(&mut self, width: i32, height: i32) {
        if self.width == width && self.height == height {
            return;
        }
        self.texture = Self::create_texture(&self.device, width, height);
        self.width = width;
        self.height = height;
    }

    fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut std::ffi::c_void> {
        if width <= 0 || height <= 0 {
            return Err(anyhow!("invalid offscreen target size {width}x{height}"));
        }
        self.ensure_size(width, height);

        let texture_info = unsafe { mtl::TextureInfo::new(self.texture.as_ptr() as mtl::Handle) };
        let backend_render_target =
            backend_render_targets::make_mtl((self.width, self.height), &texture_info);
        let surface = gpu::surfaces::wrap_backend_render_target(
            &mut self.skia,
            &backend_render_target,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            skia_safe::ColorType::BGRA8888,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("failed to wrap metal offscreen render target"))?;
        self.current_surface = Some(surface);
        Ok(self as *mut Self as *mut std::ffi::c_void)
    }

    fn end_frame(&mut self) -> Result<()> {
        let surface = self
            .current_surface
            .take()
            .ok_or_else(|| anyhow!("offscreen end_frame called before begin_frame"))?;
        self.skia.flush_and_submit();
        drop(surface);
        Ok(())
    }

    fn readback_rgba(&self) -> Result<Vec<u8>> {
        let width = self.width.max(1) as u64;
        let height = self.height.max(1) as u64;
        let mut bgra = vec![0_u8; (width * height * 4) as usize];
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width,
                height,
                depth: 1,
            },
        };
        self.texture
            .as_ref()
            .get_bytes(bgra.as_mut_ptr().cast(), width * 4, region, 0);
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        Ok(bgra)
    }

    unsafe fn begin_frame_bridge(
        user_data: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<*mut std::ffi::c_void> {
        unsafe { &mut *(user_data as *mut Self) }.begin_frame(width, height)
    }

    unsafe fn end_frame_bridge(user_data: *mut std::ffi::c_void) -> Result<()> {
        unsafe { &mut *(user_data as *mut Self) }.end_frame()
    }

    unsafe fn resolve_skia_canvas_bridge(
        _user_data: *mut std::ffi::c_void,
        frame_surface: *mut std::ffi::c_void,
    ) -> Result<*mut std::ffi::c_void> {
        let target = unsafe { &mut *(frame_surface as *mut Self) };
        let surface = target
            .current_surface
            .as_mut()
            .ok_or_else(|| anyhow!("skia canvas requested before offscreen surface was ready"))?;
        Ok(surface.canvas() as *const _ as *mut std::ffi::c_void)
    }

    unsafe fn readback_rgba_bridge(user_data: *mut std::ffi::c_void) -> Result<Vec<u8>> {
        unsafe { &mut *(user_data as *mut Self) }.readback_rgba()
    }
}
