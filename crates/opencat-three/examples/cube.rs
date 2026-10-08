//! 冒烟：`Scene3` → wgpu → straight-alpha RGBA。
//!
//! ```bash
//! cargo run -p opencat-three --example cube
//! ```
//!
//! 检查四件事：同场景逐字节一致（确定性）、背景透明、画面中心命中立方体、
//! 旋转改变着色。产物写 `/tmp/opencat-three-cube-f{0,1}.png`。

use glam::Vec3;
use opencat_three::Renderer3d;
use opencat_three::scene3::{Camera3, Geometry3, Light3, Material3, Mesh3, Scene3, Shading3};

fn cube() -> Geometry3 {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for axis in 0..3 {
        for sign in [-1.0f32, 1.0] {
            let mut n = [0.0f32; 3];
            n[axis] = sign;
            let nv = Vec3::from(n);
            let u = if nv.x.abs() < 0.9 { nv.cross(Vec3::Y) } else { nv.cross(Vec3::Z) };
            let v = nv.cross(u);
            let base = positions.len() as u32;
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let p = nv * 0.5 + u * (0.5 * su) + v * (0.5 * sv);
                positions.push([p.x, p.y, p.z]);
                normals.push(n);
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    Geometry3::Inline { positions, normals, indices, uvs: vec![], colors: vec![], colors1: vec![] }
}

fn world_matrix_rot_y(deg: f32) -> [f32; 16] {
    // 列主序绕 Y 旋转（example 自足，不引 glam 矩阵依赖）。
    let (s, c) = deg.to_radians().sin_cos();
    [
        c, 0.0, -s, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        s, 0.0, c, 0.0, //
        0.0, 0.0, 0.0, 1.0, //
    ]
}

fn scene(rot_y_deg: f32) -> Scene3 {
    Scene3 {
        clear: [0.0, 0.0, 0.0, 0.0],
        target: [640, 480],
        camera: Camera3 {
            position: [2.2, 1.8, 3.0],
            target: [0.0, 0.0, 0.0],
            fov_y_degrees: 45.0,
            near: 0.1,
            far: 100.0,
        },
        lights: vec![
            Light3::Ambient { color: [1.0, 1.0, 1.0], intensity: 0.25 },
            Light3::Hemisphere {
                sky_color: [0.4, 0.5, 0.6],
                ground_color: [0.5, 0.4, 0.3],
                intensity: 1.0,
            },
            Light3::Directional {
                // 光传播方向（从光源指向场景）：前表面 +z 被照亮。
                direction: [-0.4, -0.8, -1.0],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
        ],
        // ⑥a 输出变换默认（three.js NoToneMapping/1.0；Linear = 引擎 raw
        // 契约）——不改动本例既有断言的逐字节输出。
        tone_mapping: opencat_three::scene3::ToneMapping3::None,
        tone_mapping_exposure: 1.0,
        output_color_space: opencat_three::scene3::OutputColorSpace3::Linear,
        environment: None,
        meshes: vec![Mesh3 {
            geometry: cube(),
            material: Material3 {
                base_color: [0.9, 0.3, 0.2, 1.0],
                shading: Shading3::Lambert,
                displacement: None,
                map: None,
                textures: vec![],
                transparent: false,
            },
            matrix: world_matrix_rot_y(rot_y_deg),
            render_order: 0,
        }],
    }
}

// 透射冒烟场景：全屏 Unlit 背景板（条纹色 z=-1）+ Physical 立方体。
// 立方体 transmission=1：其颜色应来自「场景色缓冲中的背景板」经折射采样
// + 衰减/菲涅耳，而不是自发光——与背景板直视色必不相同，且不透明。
fn scene_transmission() -> Scene3 {
    let mut s = scene(0.0);
    let mut board = s.meshes[0].clone();
    board.material = Material3 {
        base_color: [1.0, 0.0, 0.0, 1.0],
        shading: Shading3::Unlit,
        displacement: None,
        map: None,
        textures: vec![],
        transparent: false,
    };
    board.matrix = [
        20.0, 0.0, 0.0, 0.0, //
        0.0, 20.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, -1.0, 1.0, //
    ];
    s.meshes[0].material.shading = Shading3::Physical(opencat_three::scene3::Physical3 {
        ior: 1.33,
        thickness: 1.5,
        transmission: 1.0,
        roughness: 0.0,
        metalness: 0.0,
        specular_intensity: 2.0,
        clearcoat: 1.0,
        clearcoat_roughness: 0.0,
        dispersion: 1.0,
        attenuation_color: [0.533, 0.808, 0.498],
        attenuation_distance: 2.0,
        env_map_intensity: 1.2,
    });
    s.meshes.push(board);
    s
}

fn main() -> anyhow::Result<()> {
    let mut r = pollster::block_on(Renderer3d::new())?;
    let (w, h) = (480u32, 360u32);
    let no_assets = opencat_three::scene3::GeometryRegistry::new();
    let no_textures = Vec::new();
    let f0 = r.render(&scene(0.0), &no_assets, &no_textures, w, h)?;
    let f0_again = r.render(&scene(0.0), &no_assets, &no_textures, w, h)?;
    let f1 = r.render(&scene(30.0), &no_assets, &no_textures, w, h)?;

    assert_eq!(f0.len(), (w * h * 4) as usize, "输出必须是紧凑 w*h*4");
    assert_eq!(f0, f0_again, "同场景必须逐字节一致（渲染确定性）");

    assert_eq!(&f0[0..4], &[0, 0, 0, 0], "角落必须是透明背景");

    let center = (((h / 2) * w + w / 2) * 4) as usize;
    let px0 = &f0[center..center + 4];
    assert_eq!(px0[3], 255, "画面中心应有立方体（alpha=255），实际 {px0:?}");

    let px1 = &f1[center..center + 4];
    assert_ne!(px0, px1, "旋转 30° 必须改变中心着色");

    for (name, data) in [("cube-f0", &f0), ("cube-f1", &f1)] {
        let path = std::env::temp_dir().join(format!("opencat-three-{name}.png"));
        image::save_buffer(&path, data, w, h, image::ColorType::Rgba8)?;
        println!("written {}", path.display());
    }
    // 透射冒烟：Physical 立方体前面的红背景板经场景色缓冲折射进像素。
    let t = r.render(&scene_transmission(), &no_assets, &no_textures, w, h)?;
    let pxt = &t[center..center + 4];
    assert_eq!(pxt[3], 255, "透射体必须不透明（RT 全不透明）: {pxt:?}");
    let refracted_channel = pxt[0]; // R：背景板红色经衰减/菲涅耳后应显著存在
    assert!(
        refracted_channel > 40,
        "透射体中心应采样到背景板红色（折射链路通），实际 {pxt:?}"
    );
    let path = std::env::temp_dir().join("opencat-three-cube-transmission.png");
    image::save_buffer(&path, &t, w, h, image::ColorType::Rgba8)?;
    println!("written {}", path.display());
    println!("ok: center f0={px0:?} f1={px1:?} transmission={pxt:?}");

    // ══ ⑥a 输出变换冒烟（js:18-20）：ACESFilmic(×exposure) + sRGB OETF，
    //    CPU 镜像逐位比对。Unlit 全屏板（MeshBasicMaterial 语义 = toneMapped
    //    内建，走 output_xform；VideoMosaic/ShaderMaterial 不走——后者已有
    //    上方 map/wipe 冒烟锁定 raw 值）。 ══
    let use_fs_quad = |mut s: Scene3, c: [f32; 4]| {
        s.meshes.clear();
        s.meshes.push(Mesh3 {
            geometry: Geometry3::Inline {
                positions: vec![[-8.0, 4.5, 0.0], [8.0, 4.5, 0.0], [-8.0, -4.5, 0.0], [8.0, -4.5, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 4],
                indices: vec![0, 2, 1, 2, 3, 1],
                uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                colors: vec![],
                colors1: vec![],
            },
            material: Material3 {
                base_color: c,
                shading: Shading3::Unlit,
                displacement: None,
                map: None,
                textures: vec![],
                transparent: false,
            },
            matrix: world_matrix_rot_y(0.0),
            render_order: 0,
        });
        s
    };
    // CPU 镜像（与 lib.rs WGSL / three.js tonemapping_pars_fragment +
    // colorspace_pars_fragment 同源公式；mat3 列构造 × 向量）。
    fn mv3(cols: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
        let mut o = [0f32; 3];
        for col in 0..3 {
            for row in 0..3 {
                o[row] += cols[col][row] * v[col];
            }
        }
        o
    }
    fn aces_filmic_cpu(color_in: [f32; 3], exposure: f32) -> [f32; 3] {
        let input_cols = [
            [0.59719, 0.07600, 0.02840],
            [0.35458, 0.90834, 0.13383],
            [0.04823, 0.01566, 0.83777],
        ];
        let output_cols = [
            [1.60475, -0.10208, -0.00327],
            [-0.53108, 1.10813, -0.07276],
            [-0.07367, -0.00605, 1.07602],
        ];
        let mut c = color_in.map(|x| x * (exposure / 0.6));
        c = mv3(input_cols, c);
        c = c.map(|x| {
            let a = x * (x + 0.0245786) - 0.000090537;
            let b = x * (0.983729 * x + 0.4329510) + 0.238081;
            a / b
        });
        c = mv3(output_cols, c);
        c.map(|x| x.clamp(0.0, 1.0))
    }
    fn srgb_oetf_cpu(v: f32) -> f32 {
        if v <= 0.0031308 {
            v * 12.92
        } else {
            v.powf(0.41666) * 1.055 - 0.055
        }
    }
    let board_color = [0.2f32, 0.4, 0.8, 1.0];
    // ① 默认（tonemap 关）：raw 输出 = round(base×255)。
    let raw = r.render(&use_fs_quad(scene(0.0), board_color), &no_assets, &no_textures, w, h)?;
    let praw = &raw[center..center + 4];
    for (i, want) in [51u8, 102, 204, 255].iter().enumerate() {
        assert!(
            (praw[i] as i32 - *want as i32).abs() <= 1,
            "tonemap 关 = raw 输出，通道 {i}: {praw:?} ≈ {want}"
        );
    }
    // ② 开（js:18-20）：ACES(×1.08) → sRGB，CPU 镜像 ±1。
    let mut toned = scene(0.0);
    toned.tone_mapping = opencat_three::scene3::ToneMapping3::ACESFilmic;
    toned.tone_mapping_exposure = 1.08;
    toned.output_color_space = opencat_three::scene3::OutputColorSpace3::Srgb;
    let out = r.render(&use_fs_quad(toned, board_color), &no_assets, &no_textures, w, h)?;
    let ptoned = &out[center..center + 4];
    let linear = aces_filmic_cpu([0.2, 0.4, 0.8], 1.08);
    for (i, &lin) in linear.iter().enumerate() {
        let want = (srgb_oetf_cpu(lin) * 255.0).round() as i32;
        assert!(
            (ptoned[i] as i32 - want).abs() <= 1,
            "ACES+sRGB 通道 {i}: gpu {ptoned:?} vs cpu {want}（linear {lin}）"
        );
    }
    assert_ne!(praw[..3], ptoned[..3], "输出变换必须实际改变像素");
    // ③ 透射 RT 二遍恒线性（WebGLRenderer.js:1921-1922/:2197）：tonemap 开
    //    的透射场景照常成立（RT 里的 Unlit 板不被预变换，主 pass 才变换）。
    let mut t_on = scene_transmission();
    t_on.tone_mapping = opencat_three::scene3::ToneMapping3::ACESFilmic;
    t_on.tone_mapping_exposure = 1.08;
    t_on.output_color_space = opencat_three::scene3::OutputColorSpace3::Srgb;
    let t2 = r.render(&t_on, &no_assets, &no_textures, w, h)?;
    let pxt2 = &t2[center..center + 4];
    assert!(
        pxt2[0] > 40,
        "tonemap 开的透射链路仍通（RT 线性采样 + 主 pass 变换）: {pxt2:?}"
    );
    assert_ne!(pxt[..3], pxt2[..3], "主 pass 输出变换必须作用到透射体");
    println!(
        "ok: xform off={praw:?} on={ptoned:?} (cpu {:?}) transmission-on={pxt2:?}",
        linear.map(|l| (srgb_oetf_cpu(l) * 255.0).round() as u8)
    );

    // 脚本着色器冒烟（GLSL→naga→WGSL 全链路在真 GPU 上）：全屏单四边形
    // ShaderMaterial，vUv 驱动红绿渐变 + uFade 调亮度。左下角 uv=(0,0)、
    // 右上角 uv=(1,1)（three.js PlaneGeometry uv 语义）。
    let mut s = scene(0.0);
    s.meshes.clear();
    s.meshes.push(Mesh3 {
        geometry: Geometry3::Inline {
            positions: vec![[-8.0, 4.5, 0.0], [8.0, 4.5, 0.0], [-8.0, -4.5, 0.0], [8.0, -4.5, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 2, 1, 2, 3, 1],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
            colors: vec![],
            colors1: vec![],
        },
        material: Material3 {
            base_color: [1.0, 1.0, 1.0, 1.0],
            shading: Shading3::Shader(opencat_three::scene3::ShaderMaterial3 {
                vertex_shader: r"#version 450 core
precision highp float;
layout(std140, set = 0, binding = 0) uniform ThreeBuiltins {
  mat4 projectionMatrix;
  mat4 modelViewMatrix;
  mat4 modelMatrix;
  mat4 viewMatrix;
  vec3 cameraPosition;
};
layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 0) out vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
"
                .to_string(),
                fragment_shader: r#"#version 450 core
precision highp float;
layout(std140, set = 0, binding = 0) uniform ThreeBuiltins {
  mat4 projectionMatrix;
  mat4 modelViewMatrix;
  mat4 modelMatrix;
  mat4 viewMatrix;
  vec3 cameraPosition;
};
layout(std140, set = 1, binding = 0) uniform UserUniforms {
  float uFade;
};
layout(location = 0) in vec2 vUv;
layout(location = 0) out vec4 fragOutput;
void main() {
  fragOutput = vec4(vUv.x, vUv.y, 0.25, 1.0) * uFade;
}
"#
                .to_string(),
                uniforms: vec![
                    opencat_three::scene3::ShaderUniform3 {
                        name: "uFade".to_string(),
                        value: opencat_three::scene3::ShaderValue3::Float(1.0),
                    },
                ],
            }),
            displacement: None,
            map: None,
            textures: vec![],
            transparent: false,
        },
        matrix: [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, //
        ],
        render_order: 0,
    });
    let sh = r.render(&s, &no_assets, &no_textures, w, h)?;
    // 相机斜视 ⇒ 屏幕角点的 uv 不是端点、且极角可能落出四边形——在
    // 25%/75% 位置采样，断言用方向性（x→R 增、y→G 增）而非端点值。
    let px_at = |buf: &[u8], x: u32, y: u32| -> [u8; 4] {
        let o = ((y * w + x) * 4) as usize;
        buf[o..o + 4].try_into().unwrap()
    };
    let tl = px_at(&sh, w / 4, h / 4);
    let br = px_at(&sh, 3 * w / 4, 3 * h / 4);
    let bl = px_at(&sh, w / 4, 3 * h / 4);
    assert_eq!(tl[3], 255, "脚本着色器应覆盖采样点: {tl:?}");
    // B = 0.25·uFade·255 = 64（uniform 装载 + 常量项，逐像素精确）。
    assert_eq!(tl[3], 255, "脚本着色器应铺满全屏: {tl:?}");
    assert_eq!(tl[2], 64, "B = 0.25·uFade·255: {tl:?}");
    // vUv.x 沿 +x 增 → 右侧 R > 左侧 R；vUv.y 沿 +y 增 → 上侧 G > 下侧 G。
    assert!(
        br[0] > tl[0] && tl[1] > bl[1],
        "uv 方向性（x→R 增、y→G 增）: tl={tl:?} bl={bl:?} br={br:?}"
    );
    // uFade=0.5 → B 减半到 32、G 减半（uniform 反射打包 + 逐帧生效链路）。
    if let Shading3::Shader(shm) = &mut s.meshes[0].material.shading {
        shm.uniforms[0].value = opencat_three::scene3::ShaderValue3::Float(0.5);
    }
    let sh2 = r.render(&s, &no_assets, &no_textures, w, h)?;
    let tl2 = px_at(&sh2, w / 4, h / 4);
    assert_eq!(tl2[2], 32, "uFade=0.5 → B=32: {tl2:?}");
    assert!(
        (i32::from(tl[1]) - i32::from(tl2[1])).abs() > 40,
        "uFade 减半应显著调暗: {tl:?} vs {tl2:?}"
    );
    let path = std::env::temp_dir().join("opencat-three-cube-shader.png");
    image::save_buffer(&path, &sh, w, h, image::ColorType::Rgba8)?;
    println!("written {}", path.display());
    println!("ok: shader tl={tl:?} br={br:?} fade-half tl2={tl2:?}");

    // ── 透明混合冒烟（r7 ⑤ 段 droplets 依赖）：transparent=true 的 Unlit
    // 半透明面片必须以 NormalBlending（src-alpha）落在不透明红板之后：
    // out.rgb = src·a + dst·(1−a)（WebGLState.setMaterial NormalBlending）。
    // a=0.5、绿面片/红板 → 中心 ≈ (127.5, 127.5, 0)，alpha 恢复 255；同一
    // 场景 transparent=false（不透明列表、blend None）→ 纯绿。两者显著
    // 不同 = 混合管线单变量证明。
    let blend_scene = |transparent: bool| {
        let mut s = scene(0.0);
        s.camera = Camera3 {
            position: [0.0, 0.0, 5.0],
            target: [0.0, 0.0, 0.0],
            fov_y_degrees: 45.0,
            near: 0.1,
            far: 100.0,
        };
        s.meshes.clear();
        let plane = |w: f32, h: f32, z: f32| Geometry3::Inline {
            positions: vec![[-w / 2.0, h / 2.0, z], [w / 2.0, h / 2.0, z], [-w / 2.0, -h / 2.0, z], [w / 2.0, -h / 2.0, z]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 2, 1, 2, 3, 1],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
            colors: vec![],
            colors1: vec![],
        };
        let mut board = Mesh3 {
            geometry: plane(20.0, 20.0, -1.0),
            material: Material3 {
                base_color: [1.0, 0.0, 0.0, 1.0],
                shading: Shading3::Unlit,
                displacement: None,
                map: None,
                textures: vec![],
                transparent: false,
            },
            matrix: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
            render_order: 0,
        };
        board.matrix[12..15].copy_from_slice(&[0.0, 0.0, 0.0]);
        let quad = Mesh3 {
            geometry: plane(2.0, 2.0, 0.0),
            material: Material3 {
                base_color: [0.0, 1.0, 0.0, 0.5],
                shading: Shading3::Unlit,
                displacement: None,
                map: None,
                textures: vec![],
                transparent,
            },
            matrix: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
            render_order: 1,
        };
        s.meshes.push(board);
        s.meshes.push(quad);
        s
    };
    let on = r.render(&blend_scene(true), &no_assets, &no_textures, w, h)?;
    let off = r.render(&blend_scene(false), &no_assets, &no_textures, w, h)?;
    let pon = px_at(&on, w / 2, h / 2);
    let poff = px_at(&off, w / 2, h / 2);
    let near = |v: u8, t: i32| (i32::from(v) - t).abs() <= 2;
    assert!(near(pon[0], 128) && near(pon[1], 128) && pon[2] <= 2 && pon[3] == 255,
        "src-alpha 混合 = 半绿半红: {pon:?}");
    // blend-off：无混合 → rgb 原样写（alpha 也原样 0.5——three.js alpha:false
    // 上下文里画布不透，rgb 才是可见语义）。
    assert_eq!(&poff[0..3], &[0, 255, 0], "blend-off（不透明列表）应为纯绿: {poff:?}");
    let path = std::env::temp_dir().join("opencat-three-cube-blend.png");
    image::save_buffer(&path, &on, w, h, image::ColorType::Rgba8)?;
    println!("written {}", path.display());
    println!("ok: blend on={pon:?} off={poff:?}");

    // ── ShaderMaterial map 接缝冒烟（r7 wordmark 贴图面依赖）：材质带
    // `map` 位图，片元用 set 3 拆分声明采样 + uReveal 擦除 discard。
    // 位图 2×1（左红右绿）：uReveal=1 → 左半红/右半绿；uReveal=0.5 →
    // uv.x>0.5 被 discard（露出透明清屏）= wordmarkReveal 语义链路。
    let map_key_path = "cube-example-map.png";
    let table = vec![(
        format!("bitmap:{map_key_path}"),
        opencat_three::TextureBitmap3 {
            rgba: std::sync::Arc::new(vec![255, 0, 0, 255, 0, 255, 0, 255]),
            width: 2,
            height: 1,
        },
    )];
    let map_shader = |reveal: f32| {
        let mut s = scene(0.0);
        // 正对相机 + 面片恰好铺满视口：uv 与屏幕坐标一一对应（z=0 处可视
        // 高 = 2·d·tan(fov/2)、宽 ×aspect；超大面片下屏幕 25% ≠ uv 0.25），
        // 断言才能用确定值。
        s.camera = Camera3 {
            position: [0.0, 0.0, 5.0],
            target: [0.0, 0.0, 0.0],
            fov_y_degrees: 45.0,
            near: 0.1,
            far: 100.0,
        };
        s.meshes.clear();
        let vis_h = 2.0 * 5.0 * (45.0f32.to_radians() / 2.0).tan();
        let vis_w = vis_h * (w as f32 / h as f32);
        s.meshes.push(Mesh3 {
            geometry: Geometry3::Inline {
                positions: vec![
                    [-vis_w / 2.0, vis_h / 2.0, 0.0],
                    [vis_w / 2.0, vis_h / 2.0, 0.0],
                    [-vis_w / 2.0, -vis_h / 2.0, 0.0],
                    [vis_w / 2.0, -vis_h / 2.0, 0.0],
                ],
                normals: vec![[0.0, 0.0, 1.0]; 4],
                indices: vec![0, 2, 1, 2, 3, 1],
                uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                colors: vec![],
                colors1: vec![],
            },
            material: Material3 {
                base_color: [1.0, 1.0, 1.0, 1.0],
                shading: Shading3::Shader(opencat_three::scene3::ShaderMaterial3 {
                    vertex_shader: r"#version 450 core
precision highp float;
layout(std140, set = 0, binding = 0) uniform ThreeBuiltins {
  mat4 projectionMatrix;
  mat4 modelViewMatrix;
  mat4 modelMatrix;
  mat4 viewMatrix;
  vec3 cameraPosition;
};
layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 0) out vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
"
                    .to_string(),
                    fragment_shader: r#"#version 450 core
precision highp float;
layout(std140, set = 0, binding = 0) uniform ThreeBuiltins {
  mat4 projectionMatrix;
  mat4 modelViewMatrix;
  mat4 modelMatrix;
  mat4 viewMatrix;
  vec3 cameraPosition;
};
layout(std140, set = 1, binding = 0) uniform UserUniforms {
  float uReveal;
};
layout(set = 3, binding = 0) uniform texture2D map;
layout(set = 3, binding = 1) uniform sampler map_smp;
layout(location = 0) in vec2 vUv;
layout(location = 0) out vec4 fragOutput;
void main() {
  if (vUv.x > uReveal) discard;
  fragOutput = texture(sampler2D(map, map_smp), vUv);
}
"#
                    .to_string(),
                    uniforms: vec![opencat_three::scene3::ShaderUniform3 {
                        name: "uReveal".to_string(),
                        value: opencat_three::scene3::ShaderValue3::Float(reveal),
                    }],
                }),
                displacement: None,
                map: Some(opencat_three::scene3::Texture3::Bitmap {
                    path: map_key_path.to_string(),
                }),
                textures: vec![],
                transparent: false,
            },
            matrix: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
            render_order: 0,
        });
        s
    };
    let full = r.render(&map_shader(1.0), &no_assets, &table, w, h)?;
    let pfl = px_at(&full, w / 4, h / 2);
    let pfr = px_at(&full, 3 * w / 4, h / 2);
    assert!(pfl[0] > 200 && pfl[1] < 60, "uv.x≈0.25 应采到左半红色: {pfl:?}");
    assert!(pfr[1] > 200 && pfr[0] < 60, "uv.x≈0.75 应采到右半绿色: {pfr:?}");
    let wiped = r.render(&map_shader(0.5), &no_assets, &table, w, h)?;
    let pwl = px_at(&wiped, w / 4, h / 2);
    let pwr = px_at(&wiped, 3 * w / 4, h / 2);
    assert!(pwl[0] > 200, "reveal=0.5 左半仍在: {pwl:?}");
    assert_eq!(pwr, [0, 0, 0, 0], "reveal=0.5 右半被 discard（透出清屏）: {pwr:?}");
    let path = std::env::temp_dir().join("opencat-three-cube-mapwipe.png");
    image::save_buffer(&path, &wiped, w, h, image::ColorType::Rgba8)?;
    println!("written {}", path.display());
    println!("ok: map full l={pfl:?} r={pfr:?} wipe-half l={pwl:?} r={pwr:?}");

    // ── 环境 IBL 冒烟（⑥b）：解析封闭解 ────────────────────────────
    // 纯色 env E + 白色漫反射 Physical（specular_intensity=0、clearcoat=0、
    // transmission=0、无灯光、NoToneMapping/Linear raw 输出）：
    //   getIBLIrradiance = π·E·intensity → RE_IndirectSpecular 里
    //   cosineWeightedIrradiance = E → indirectDiffuse = 1·E；
    //   radiance/multiscatter 项全部为 0（spec_color = 0）⇒ 输出 = E·intensity
    //   逐通道精确成立。这同时打到 cubeUV extra-LOD 寻址（roughness=1 →
    //   mip −2 → filterInt 6 tile）与 getIBLRadiance 通路（× single=0）。
    let env_rgb = [200u8, 120, 60];
    // 256×128（cubeSize 64，texelSize 有限——8×4 的退化小图 cubeSize=2 会
    // 触发 1/(size−2)=inf，three.js 同样未定义，真实 env ≥ 256 不会走到）。
    let solid_env = opencat_three::TextureBitmap3 {
        rgba: std::sync::Arc::new(
            (0..256 * 128)
                .flat_map(|_| [env_rgb[0], env_rgb[1], env_rgb[2], 255u8])
                .collect(),
        ),
        width: 256,
        height: 128,
    };
    let e_lin: [f32; 3] = env_rgb.map(|c| opencat_three::pmrem::srgb_eotf(c as f32 / 255.0));
    let ibl_scene = |intensity: f32| -> Scene3 {
        let mut s = scene(0.0);
        s.lights.clear(); // 关掉例子的三点布光 → 直接项为 0
        s.meshes[0].material.base_color = [1.0, 1.0, 1.0, 1.0]; // 漫反射 = 1 ⇒ 输出 = E·intensity
        s.tone_mapping = opencat_three::scene3::ToneMapping3::None;
        s.output_color_space = opencat_three::scene3::OutputColorSpace3::Linear;
        s.meshes[0].material.shading = Shading3::Physical(opencat_three::scene3::Physical3 {
            ior: 1.5,
            thickness: 0.0,
            transmission: 0.0,
            roughness: 0.5,
            metalness: 0.0,
            specular_intensity: 0.0,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
            dispersion: 0.0,
            attenuation_color: [1.0, 1.0, 1.0],
            attenuation_distance: 1e30,
            env_map_intensity: intensity,
        });
        s
    };
    r.set_environment(&solid_env)
        .expect("纯色 env 的 PMREM 生成不应失败");
    for (tag, intensity) in [("1.0", 1.0f32), ("0.7", 0.7)] {
        let f = r.render(&ibl_scene(intensity), &no_assets, &no_textures, w, h)?;
        let p = px_at(&f, w / 2, h / 2);
        for k in 0..3 {
            let want = e_lin[k] * intensity;
            let got = p[k] as f32 / 255.0;
            assert!(
                (got - want).abs() <= 2.0 / 255.0,
                "IBL 解析解 {tag} 通道 {k}: 期望 {want}（E·intensity），实际 {got}（{p:?}）"
            );
        }
        let dbgpath = std::env::temp_dir().join(format!("opencat-three-ibl-{tag}.png"));
        image::save_buffer(&dbgpath, &f, w, h, image::ColorType::Rgba8)?;
        println!("ok: ibl intensity={tag} px={p:?}（= E·intensity 解析解）written {dbgpath:?}");
    }
    // env 关（未 set_environment 的独立渲染器）+ 同 Physical 材质 →
    // getIBL* 归零 → 纯黑（USE_ENVMAP 未定义语义；逐字节中性）。
    let mut r2 = pollster::block_on(Renderer3d::new())?;
    let f_noenv = r2.render(&ibl_scene(1.0), &no_assets, &no_textures, w, h)?;
    assert_eq!(
        &f_noenv[center..center + 4],
        &[0, 0, 0, 255],
        "无 env 的 Physical 应零 IBL 输出"
    );
    println!("ok: ibl env-off px=[0,0,0,255]（getIBL* 归零）");
    Ok(())
}
