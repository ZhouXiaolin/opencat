//! Aurora 配方与快照的像素对拍。
//!
//! 快照（`tools/shader-diff/ours`）是权威：它读 typegpu 编出来的 WGSL 直接跑
//! wgpu。这里读的是 `opencat-shaders` 自己烘焙的 WGSL。两边都跑 wgpu Metal，
//! 如果配方翻译正确，像素应该逐字节相同（同一后端、同一驱动、同一运算序）。
//!
//! 快照缺席时测试跳过——它是外部产物（`--out /tmp/ours-test`），不在仓库里。

use opencat_shaders::Val;

/// 烘焙器输出与快照 oracle 都走同一条 `final_1` 尾巴（`linearToSrgb(...) *
/// a`，pre-multiplied），同后端同驱动，逐字节比即可。
fn compare_premultiplied(ours: &[u8], reference: &[u8]) -> (f64, u8) {
    let mut sum = 0u64;
    let mut max = 0u8;
    for (a, b) in ours.iter().zip(reference.iter()) {
        let d = a.abs_diff(*b);
        sum += d as u64;
        max = max.max(d);
    }
    (sum as f64 / (ours.len() as f64), max)
}

/// 快照采样帧（`catalog.json` 的 `sampleFrames`）。
/// MeshGradient `speed: 1`——帧号直接换秒。
const MESH_FRAMES: &[(u32, f32)] = &[(10, 10.0 / 30.0), (45, 45.0 / 30.0), (90, 90.0 / 30.0)];

/// `bake` 收的是**秒**，`_animTime` 由 `time * speed` 得到（Aurora 默认 speed 5）。
const FRAMES: &[(u32, f32)] = &[(10, 10.0 / 30.0), (45, 45.0 / 30.0), (90, 90.0 / 30.0)];

#[test]
fn aurora_matches_snapshot() {
    let props = opencat_shaders::aurora::default_props();
    for (frame, anim_time) in FRAMES {
        let baked = opencat_shaders::aurora::bake(&props, 512.0, 512.0, *anim_time);
        let rgba = opencat_shaders::render(&baked, 512, 512).expect("render aurora");
        assert_eq!(rgba.len(), 512 * 512 * 4);

        let reference = format!("/tmp/ours-test/Aurora/default/f{frame}.png");
        let Ok(img) = image::open(&reference) else {
            eprintln!("skip: 快照 {reference} 不存在（先跑 tools/shader-diff/ours）");
            return;
        };
        let img = img.to_rgba8();

        let (mae, max) = compare_premultiplied(&rgba, img.as_raw());
        eprintln!("f{frame}: mae {mae:.6} maxd {max}");
        assert!(mae < 0.5, "f{frame}: mae {mae} 超过门禁 0.5");
    }
}

#[test]
fn aurora_props_are_complete() {
    let props = opencat_shaders::aurora::default_props();
    // 真库 13 个 prop 全在（curtainCount/colorSpace 是 compileTime，仍需参与烘焙）。
    for k in [
        "colorA", "colorB", "colorC", "colorSpace", "balance", "intensity", "curtainCount", "speed",
        "waviness", "rayDensity", "height", "center", "seed",
    ] {
        assert!(props.contains_key(k), "缺 prop {k}");
    }
    // 颜色已进 P3-linear（与真库默认色彩空间一致）。
    let a = props["colorA"].as_fs();
    assert!((a[0] - 0.3153386).abs() < 1e-5, "colorA.r = {}", a[0]);
    let _ = Val::F(0.0);
}

#[test]
fn mesh_gradient_matches_snapshot() {
    let props = opencat_shaders::mesh_gradient::default_props();
    // 真库 `animatedTime: {speed:'speed'}`，MeshGradient 的 speed 默认 1（不是 FRAMES 的 5）。
    for (frame, anim_time) in MESH_FRAMES {
        let baked = opencat_shaders::mesh_gradient::bake(&props, 512.0, 512.0, *anim_time);
        let rgba = opencat_shaders::render(&baked, 512, 512).expect("render mesh gradient");

        let reference = format!("/tmp/ours-test/MeshGradient/default/f{frame}.png");
        let Ok(img) = image::open(&reference) else {
            eprintln!("skip: 快照 {reference} 不存在");
            return;
        };
        let img = img.to_rgba8();
        let (mae, max) = compare_premultiplied(&rgba, img.as_raw());
        eprintln!("MeshGradient f{frame}: mae {mae:.6} maxd {max}");
        assert!(mae < 0.5, "f{frame}: mae {mae} 超过门禁 0.5");
    }
}

#[test]
fn zoom_blur_matches_snapshot() {
    let props = opencat_shaders::zoom_blur::default_props();
    // ZoomBlur 没有 animatedTime，`_sys.time` 才是驱动时钟（VHS 同理）。
    for (frame, _) in FRAMES {
        let time = *frame as f32 / 30.0;
        let baked = opencat_shaders::zoom_blur::bake(&props, 512.0, 512.0, time);
        let rgba = opencat_shaders::render(&baked, 512, 512).expect("render zoom blur");
        assert_close(&rgba, "ZoomBlur", *frame);
    }
}

#[test]
fn vhs_matches_snapshot() {
    let props = opencat_shaders::vhs::default_props();
    for (frame, _) in FRAMES {
        let time = *frame as f32 / 30.0;
        let baked = opencat_shaders::vhs::bake(&props, 512.0, 512.0, time);
        let rgba = opencat_shaders::render(&baked, 512, 512).expect("render vhs");
        assert_close(&rgba, "VHS", *frame);
    }
}

/// 逐通道比对我们出图与 `/tmp/ours-test` 的参考 PNG；参考缺席则 skip。
fn assert_close(rgba: &[u8], component: &str, frame: u32) {
    let reference = format!("/tmp/ours-test/{component}/default/f{frame}.png");
    let Ok(img) = image::open(&reference) else {
        eprintln!("skip: 快照 {reference} 不存在");
        return;
    };
    let img = img.to_rgba8();
    let (mae, max) = compare_premultiplied(rgba, img.as_raw());
    eprintln!("{component} f{frame}: mae {mae:.6} maxd {max}");
    assert!(mae < 0.5, "{component} f{frame}: mae {mae} 超过门禁 0.5");
}

/// filter/warp 吃**真实上游**：给它一个纯色输入，输出里不应再出现内建
/// LinearGradient（`#312e81`→`#f59e0b` 的横向渐变）。
///
/// 判据用像素本身，不依赖外部快照：内建子层在左边缘是深蓝、右边缘是橙黄；
/// 纯红输入经 VHS/ZoomBlur 之后整体仍偏红（r 通道显著高于 b）。
#[test]
fn filter_layers_consume_upstream_input() {
    let (w, h) = (64u32, 64u32);
    let mut red = vec![0u8; (w * h * 4) as usize];
    for px in red.chunks_exact_mut(4) {
        px[0] = 200;
        px[3] = 255;
    }

    for component in ["VHS", "ZoomBlur"] {
        let mut props = match component {
            "VHS" => opencat_shaders::vhs::default_props(),
            _ => opencat_shaders::zoom_blur::default_props(),
        };
        // 关掉会把颜色搅乱的项，只留「贴图 + 直出」这条主干。
        props.insert("wobble".into(), Val::F(0.0));
        props.insert("scanlineNoise".into(), Val::F(0.0));
        props.insert("smear".into(), Val::F(0.0));
        props.insert("intensity".into(), Val::F(0.0));
        let baked = match component {
            "VHS" => opencat_shaders::vhs::bake(&props, w as f32, h as f32, 0.0),
            _ => opencat_shaders::zoom_blur::bake(&props, w as f32, h as f32, 0.0),
        };
        assert!(baked.consumes_input, "{component} 应该声明吃上游");

        let out = opencat_shaders::render_with_input(&baked, w, h, Some(&red)).expect("render");
        let mid = ((h / 2) * w + w / 2) as usize * 4;
        let (r, b) = (out[mid] as i32, out[mid + 2] as i32);
        assert!(
            r > b + 40,
            "{component} 没有吃到红色上游：中间像素 r={r} b={b}（若是内建渐变则是蓝橙）"
        );
    }
}

/// 第一层就是 filter 时必须报错，不能拿内建渐变顶替。
#[test]
fn filter_without_upstream_is_an_error() {
    let layers = vec![opencat_shaders::ShaderLayer {
        component: "VHS".into(),
        props: serde_json::json!({}),
    }];
    let err = opencat_shaders::render_target(&layers, 32, 32, 0.0).expect_err("应当报错");
    let text = err.to_string();
    assert!(text.contains("第一层"), "错误应指出缺少上游，got: {text}");
}
