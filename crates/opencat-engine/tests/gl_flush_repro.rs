//! 隔离复现(真流版):生产环境 dump 的 GPU 命令流 → 真执行器 → Skia flush。
//!
//! OC_DUMP_GPU_STREAM=<dir> 跑生产渲染,dump 出 gpu-frame{N}-s{K}.bin;
//! 本测试加载它,走与生产一致的 GpuStreamExecutor + GpuCompositor 路径,
//! 崩溃即与 V8/ffmpeg 主进程环境无关,可逐命令二分。

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[test]
fn mirror_nan_clearcolor_then_flush() {
    use std::num::NonZeroU32;
    use std::sync::Arc;

    use glow::HasContext;
    use opencat_gpu_native::gl as ngl;
    use skia_safe::Paint;

    let session = opencat_engine::gl_session::GlSession::offscreen().expect("offscreen session");
    session.make_current().expect("make current");
    let gl: Arc<glow::Context> = session.gl();

    let out_tex = ngl::create_texture_rgba8(&gl, 320, 240).expect("out tex");
    let out_fbo = ngl::create_framebuffer(&gl, out_tex).expect("out fbo");
    unsafe {
        gl.bind_framebuffer(
            glow::FRAMEBUFFER,
            Some(glow::NativeFramebuffer(NonZeroU32::new(out_fbo).unwrap())),
        );
        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, "#version 330 core
in vec2 aPos;
in vec3 aColor;
out vec3 vColor;
void main() { vColor = aColor; gl_Position = vec4(aPos, 0.0, 1.0); }");
        gl.compile_shader(vs);
        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, "#version 330 core
precision mediump float;
in vec3 vColor;
out vec4 fragColor;
void main() { fragColor = vec4(vColor * 0.5, 1.0); }");
        gl.compile_shader(fs);
        let prog = gl.create_program().unwrap();
        gl.attach_shader(prog, vs);
        gl.attach_shader(prog, fs);
        gl.link_program(prog);
        assert!(gl.get_program_link_status(prog));
        gl.use_program(Some(prog));
        let vbo = gl.create_buffer().unwrap();
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        // 流里录到的 usage:GLenum 0x88E4 被 JS 按 u8 截断成 0xE4(非法)。
        let verts: [f32; 15] = [
            -0.8, -0.6, 0.1, 0.3, 1.0,
             0.8, -0.6, 1.0, 0.3, 0.1,
             0.0,  0.8, 0.1, 1.0, 0.3,
        ];
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, std::slice::from_raw_parts(verts.as_ptr() as *const u8, 60), glow::STATIC_DRAW);
        let vao = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vao));
        let lp = gl.get_attrib_location(prog, "aPos").unwrap();
        gl.enable_vertex_attrib_array(lp);
        gl.vertex_attrib_pointer_f32(lp, 2, glow::FLOAT, false, 20, 0);
        let lc = gl.get_attrib_location(prog, "aColor").unwrap();
        gl.enable_vertex_attrib_array(lc);
        gl.vertex_attrib_pointer_f32(lc, 3, glow::FLOAT, false, 20, 8);
        gl.viewport(0, 0, 320, 240);
        // 流里录到的 CLEAR_COLOR:frame0 t=NaN 使 k 依赖项成 NaN。
        gl.clear_color(f32::NAN, 0.07, f32::NAN, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
            eprintln!("[repro] nan replay ok");
    }
    {
        let mut rgba = vec![0u8; 320 * 240 * 4];
        unsafe {
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(glow::NativeFramebuffer(NonZeroU32::new(out_fbo).unwrap())),
            );
            gl.read_pixels(0, 0, 320, 240, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut rgba));
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        }
        let row = 320 * 4;
        let mut fl = vec![0u8; rgba.len()];
        for (y, d) in fl.chunks_mut(row).enumerate() {
            d.copy_from_slice(&rgba[(239 - y) * row..(240 - y) * row]);
        }
        let at = |x: usize, y: usize| (fl[(y * 320 + x) * 4], fl[(y * 320 + x) * 4 + 1], fl[(y * 320 + x) * 4 + 2]);
        eprintln!("[repro] nan outtex corner={:?} center={:?} centroid={:?}", at(0, 0), at(160, 120), at(160, 137));
    }

    let target_tex = ngl::create_texture_rgba8(&gl, 320, 240).expect("target tex");
    let mut compositor = opencat_engine::gpu_compositor::GpuCompositor::new(
        opencat_engine::gpu_compositor::CompositorBackend::Gl,
        Some(gl.clone()),
        target_tex,
        320,
        240,
    )
    .expect("compositor");
    eprintln!("[repro] nan compositor ok");
    {
        let (canvas, skia) = compositor.split();
        let image = opencat_engine::gpu_compositor::import_texture(
            skia,
            opencat_gpu_native::NativeTexture::from_raw(out_tex.raw()),
            320,
            240,
        )
        .expect("import");
        canvas.clear(skia_safe::Color::from_argb(255, 26, 26, 31));
        canvas.draw_image(&image, (0.0, 0.0), Some(&Paint::default()));
    }
    eprintln!("[repro] nan composite ok");
    let rgba = compositor.finish_frame_blocking().expect("flush+readback");
    eprintln!("[repro] nan flush+readback ok, len={}", rgba.len());
    std::fs::write("out/repro-nan.rgba", &rgba).ok();
    ngl::delete_framebuffer(&gl, out_fbo);
    ngl::delete_texture(&gl, out_tex);
    ngl::delete_texture(&gl, target_tex);
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[test]
fn dumped_stream_then_skia_flush() {
    use std::num::NonZeroU32;
    use std::sync::Arc;

    use glow::HasContext;
    use opencat_core::ir::draw_types::TableRange;
    use opencat_core::ir::generated_image::GeneratedImageId;
    use opencat_core::ir::gpu_stream::{ContextAttrs, GpuBackend, GpuOpFrame, GpuSection, GpuStream};
    use opencat_core::ir::media_plan::{FrameGeneratedImage, FrameMediaPlan};
    use opencat_gpu_native::gl as ngl;

    let dump = match std::env::var("OC_GPU_STREAM_FILE") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[repro] OC_GPU_STREAM_FILE 未设,跳过");
            return;
        }
    };
    struct Cur<'a> {
        raw: &'a [u8],
        pos: usize,
    }
    impl<'a> Cur<'a> {
        fn u32(&mut self) -> u32 {
            let v = u32::from_le_bytes(self.raw[self.pos..self.pos + 4].try_into().unwrap());
            self.pos += 4;
            v
        }
        fn u64(&mut self) -> u64 {
            let v = u64::from_le_bytes(self.raw[self.pos..self.pos + 8].try_into().unwrap());
            self.pos += 8;
            v
        }
        fn chunk(&mut self) -> &[u8] {
            let n = self.u32() as usize;
            let s = &self.raw[self.pos..self.pos + n];
            self.pos += n;
            s
        }
    }
    let mut c = Cur {
        raw: &std::fs::read(&dump).expect("dump file"),
        pos: 0,
    };

    let image_id = GeneratedImageId(c.u64());
    let target_id = String::from_utf8(c.chunk().to_vec()).unwrap();
    let width = c.u32();
    let height = c.u32();
    let ops = c.chunk().to_vec();
    let bytes = c.chunk().to_vec();
    let ranges_n = c.u32() as usize;
    let mut byte_ranges = Vec::with_capacity(ranges_n);
    for _ in 0..ranges_n {
        byte_ranges.push(TableRange { start: c.u32(), len: c.u32() });
    }
    eprintln!(
        "[repro] stream {target_id} {width}x{height} ops={} bytes={} ranges={}",
        ops.len(),
        bytes.len(),
        byte_ranges.len()
    );

    let session = opencat_engine::gl_session::GlSession::offscreen().expect("offscreen session");
    session.make_current().expect("make current");
    let gl: Arc<glow::Context> = session.gl();

    let section = GpuSection {
        streams: vec![GpuStream {
            image_id,
            target_id,
            width,
            height,
            backend: GpuBackend::WebGL2,
            attrs: ContextAttrs::default(),
            ops: GpuOpFrame {
                ops,
                bytes,
                byte_ranges,
            },
        }],
    };
    let mut plan = FrameMediaPlan::default();
    plan.generated_images.push(FrameGeneratedImage {
        id: image_id,
        width,
        height,
        rgba: std::sync::Arc::from(vec![]),
        gpu: None,
    });

    let textures = std::sync::Arc::new(opencat_gpu_native::TextureRegistry::new());
    let mut executor = opencat_engine::gpu_stream_executor::GpuStreamExecutor::new();
    let outputs = executor
        .execute(&gl, &section, &mut plan, &textures, true)
        .expect("execute");
    assert_eq!(outputs.len(), 1, "一个输出纹理");
    eprintln!("[repro] execute ok");
    let out_tex = outputs[0].0.raw();

    // 直接读回 out_tex:区分"执行层没画出三角形"vs"合成层丢内容"。
    {
        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        unsafe {
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(glow::NativeFramebuffer(NonZeroU32::new(outputs[0].1).unwrap())),
            );
            gl.read_pixels(
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(&mut rgba),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        }
        let row = width as usize * 4;
        let mut flipped = vec![0u8; rgba.len()];
        for (y, dst) in flipped.chunks_mut(row).enumerate() {
            let src = (height as usize - 1 - y) * row;
            dst.copy_from_slice(&rgba[src..src + row]);
        }
        std::fs::write("F:/OpenCat/out/repro-outtex.rgba", &flipped).ok();
        let at = |x: usize, yy: usize| {
            let o = (yy * width as usize + x) * 4;
            (flipped[o], flipped[o + 1], flipped[o + 2], flipped[o + 3])
        };
        eprintln!("[repro] outtex corner={:?} center={:?} bottom={:?}", at(0, 0), at(160, 120), at(160, 200));
    }

    // 合成器(同 GpuRenderSession::new)。
    let target_tex = ngl::create_texture_rgba8(&gl, width, height).expect("target tex");
    let mut compositor = opencat_engine::gpu_compositor::GpuCompositor::new(
        opencat_engine::gpu_compositor::CompositorBackend::Gl,
        Some(gl.clone()),
        target_tex,
        width,
        height,
    )
    .expect("compositor");
    compositor.reset_backend_state();
    eprintln!("[repro] reset ok");

    {
        let (canvas, skia) = compositor.split();
        let image = opencat_engine::gpu_compositor::import_texture(
            skia,
            opencat_gpu_native::NativeTexture::from_raw(out_tex),
            width,
            height,
        )
        .expect("import");
        canvas.clear(skia_safe::Color::from_argb(255, 26, 26, 31));
        canvas.draw_image(&image, (0.0, 0.0), Some(&skia_safe::Paint::default()));
    }
    eprintln!("[repro] composite ok");

    let rgba = compositor.finish_frame_blocking().expect("flush+readback");
    eprintln!("[repro] flush+readback ok, len={}", rgba.len());
    std::fs::write("out/repro-frame.rgba", &rgba).ok();

    let out_fbo = outputs[0].1;
    ngl::delete_framebuffer(&gl, out_fbo);
    ngl::delete_texture(&gl, opencat_gpu_native::NativeTexture::from_raw(out_tex));
    ngl::delete_texture(&gl, target_tex);
}
