//! 离屏 GL 底座冒烟:WGL 隐藏窗口 + glow 函数面 + clear/readback 全链。
//! 与命令流内容无关——崩溃在此测试复现 = 底座问题;不复现 = 流内容问题。

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[test]
fn offscreen_gl_session_clear_and_readback() {
    use std::sync::Arc;

    use glow::HasContext;

    let session = opencat_engine::gl_session::GlSession::offscreen().expect("offscreen session");
    session.make_current().expect("make current");
    let gl: Arc<glow::Context> = session.gl();

    unsafe {
        gl.viewport(0, 0, 4, 4);
        gl.clear_color(1.0, 0.0, 0.0, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
    }

    // 直接 FBO 路径 readback(create texture + fbo + read)。
    let tex = opencat_gpu_native::gl::create_texture_rgba8(&gl, 4, 4).expect("tex");
    let fbo = opencat_gpu_native::gl::create_framebuffer(&gl, tex).expect("fbo");
    unsafe {
        gl.bind_framebuffer(
            glow::FRAMEBUFFER,
            Some(glow::NativeFramebuffer(std::num::NonZeroU32::new(fbo).unwrap())),
        );
        gl.clear_color(0.0, 1.0, 0.0, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
    }
    let mut rgba = vec![0u8; 4 * 4 * 4];
    unsafe {
        gl.read_pixels(
            0,
            0,
            4,
            4,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(&mut rgba),
        );
    }
    assert_eq!(rgba[0], 0, "red channel of green clear");
    assert_eq!(rgba[1], 255, "green channel of green clear");
    opencat_gpu_native::gl::delete_framebuffer(&gl, fbo);
    opencat_gpu_native::gl::delete_texture(&gl, tex);

    // 完整 draw 链路:shader + VBO + VAO + attrib + drawArrays(定位
    // draw_arrays 的驱动崩溃)。
    unsafe {
        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, "#version 330 core
 in vec2 aPos; void main() { gl_Position = vec4(aPos, 0.0, 1.0); }");
        gl.compile_shader(vs);
        assert!(gl.get_shader_compile_status(vs), "vs: {}", gl.get_shader_info_log(vs));
        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, "#version 330 core
 precision mediump float; out vec4 o; void main() { o = vec4(1.0); }");
        gl.compile_shader(fs);
        assert!(gl.get_shader_compile_status(fs), "fs: {}", gl.get_shader_info_log(fs));
        let prog = gl.create_program().unwrap();
        gl.attach_shader(prog, vs);
        gl.attach_shader(prog, fs);
        gl.link_program(prog);
        assert!(gl.get_program_link_status(prog), "link: {}", gl.get_program_info_log(prog));
        gl.use_program(Some(prog));

        let vbo = gl.create_buffer().unwrap();
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &[0u8; 12], glow::STATIC_DRAW);
        let vao = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vao));
        let loc = gl.get_attrib_location(prog, "aPos").unwrap();
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_f32(loc, 2, glow::FLOAT, false, 8, 0);
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
        eprintln!("[smoke] draw_arrays ok");
    }
}
