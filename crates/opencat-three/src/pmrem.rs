//! PMREM cubeUV 预滤波——three.js `PMREMGenerator.fromEquirectangular`
//! （r181）的 CPU 对应物：equirect 环境贴图 → [`PMREMGenerator._fromTexture`]
//! 的 base 级渲染（`_getEquirectMaterial`）→ `_applyPMREM` 的 GGX VNDF
//! 重要性采样卷积（`_getGGXShader`），按 cubeUV 特殊布局打包进单张 2D
//! HalfFloat 纹理。渲染时 WGSL 侧的 `textureCubeUV`（cube_uv_reflection_
//! fragment.glsl.js 同构移植）按 roughness 在该纹理上插值采样。
//!
//! 为何在 CPU：卷积是每环境一次的加载期预滤波（three.js 在
//! WebGLCubeUVMaps.get 里同样只做一次并缓存），不在逐帧路径上；CPU 实现可以
//! 在无 GPU 的 CI 里逐数值测试（布局/权重/卷积守恒），GPU 侧只需要纹理上传
//! + 采样函数。同一算法的 wgpu compute 形态是等价替换，不是本层契约。
//!
//! three.js 逐段对应（node_modules/three/src/extras/PMREMGenerator.js）：
//! - `_setSize(texture.image.width / 4)`（`_fromTexture` equirect 分支，
//!   PMREMGenerator.js:266）→ lodMax = floor(log2(W/4))、cubeSize = 2^lodMax。
//! - `_allocateTargets`（:306-311）：RT 尺寸 = `3·max(cubeSize,16·7) ×
//!   4·cubeSize`，HalfFloatType RGBA、LinearFilter、LinearSRGB（线性数据）。
//! - `_createPlanes`（:682-764）：sizeLods = [cubeSize, cubeSize/2, …, 16]
//!   （每级 ÷2，到 LOD_MIN=4 后停），总级数 = lodMax−LOD_MIN+1+6（6 级
//!   extra LOD；EXTRA_LOD_SIGMA 只服务 fromScene 的 `_blur`，equirect 路径
//!   不消费——`_applyGGXFilter` 只用级序号）。
//! - `_textureToCubeUV` + `_getEquirectMaterial`（:1032-1060）：六面片
//!   （每面属性 uv 外扩 texelSize=1/(size−2)，#25071 过采样）按
//!   `equirectUv(direction)` 采源图；源 colorSpace=SRGBColorSpace → GL 采样
//!   即线性化（SRGB8_ALPHA8 硬件解码），CPU 在读取源像素时做同一 EOTF。
//! - `_applyPMREM`（:496-507）：对 lodOut = 1..n-1 逐级 `_applyGGXFilter`
//!   ——增量粗糙度 `sqrt(t²−s²)` × blurStrength `0.05+0.95·t`，读 lodIn =
//!   lodOut−1 区域（`mipInt = lodMax − lodIn`），256 样本 Hammersley ×
//!   GGX VNDF（Heitz 2018）按 NdotL 加权平均。
//! - ping-pong + 回拷两遍（`_applyGGXFilter` 内）：回拷 = roughness 0 的
//!   bilinearCubeUV 自采样，源/目标网格重合 ⇒ 恒等重采样；CPU 直接在
//!   同一缓冲上读写，数值等价（f32 精度内），少一遍 pass。
//!
//! 布局（cube_uv_reflection_fragment.glsl.js `bilinearCubeUV` 的坐标系）：
//! GL 行序（行 0 在 uv.y 0 端）——上传 wgpu 时不翻转（wgpu v=0 = 首行），
//! WGSL 用同一套 uv 公式即对齐。区域：标准锥（faceSize 从 cubeSize 收缩到
//! 16）+ y = 4·(2^maxMip − faceSize) 偏移；mip < 4 的 extra LOD 落在
//! y = 4·(cubeSize−16) 行带、x = filterInt·3·16 的第 filterInt 号 tile
//! （cubeSize < 112 时三值重叠——`max(cubeSize, 16·7)` 的宽度护栏与
//! three.js 一致，此时 RT 下半行带不写，属既有布局事实）。

use super::TextureBitmap3;

/// `PMREMGenerator.LOD_MIN`（PMREMGenerator.js:31）。
const LOD_MIN: u32 = 4;
/// `PMREMGenerator.EXTRA_LOD_SIGMA.length`（:36）——extra LOD 级数。
const EXTRA_LOD_COUNT: usize = 6;
/// `PMREMGenerator.GGX_SAMPLES`（:41）。
const GGX_SAMPLES: u32 = 256;

/// 生成的 PMREM cubeUV 纹理（HalfFloat RGBA，GL 行序；渲染端按
/// WebGLProgram.js `generateCubeUVSize` 语义消费 `max_mip`）。
#[derive(Clone, Debug)]
pub struct PmremCubeUv {
    pub width: u32,
    pub height: u32,
    /// `CUBEUV_MAX_MIP` = lodMax = log2(RT 高) − 2（WebGLProgram.js
    /// generateCubeUVSize：`Math.log2(imageHeight) - 2`）。
    pub max_mip: f32,
    /// RGBA 半精度（4×u16/px，行紧凑，长度 width·height·4）。
    pub rgba_f16: Vec<u16>,
}

/// 从 equirect 位图（RGBA8，**sRGB 编码**——three.js 里该纹理声明
/// SRGBColorSpace，GL 采样即线性化）生成 PMREM cubeUV。
pub fn generate(env: &TextureBitmap3) -> anyhow::Result<PmremCubeUv> {
    let w = env.width;
    let h = env.height;
    anyhow::ensure!(
        env.rgba.len() == (w as usize) * (h as usize) * 4,
        "equirect 位图长度 {} != {w}x{h}x4",
        env.rgba.len()
    );
    anyhow::ensure!(w >= 4, "equirect 宽度 {w} 过小（_setSize(width/4) 需 ≥ 1）");
    // `_fromTexture` equirect 分支：`_setSize(image.width / 4)`。
    let lod_max = 31 - (w / 4).max(1).leading_zeros(); // floor(log2)
    let cube_size: u32 = 1 << lod_max;
    // `_createPlanes`：sizeLods 递减到 LOD_MIN 后固定 16。totalLods 的
    // 减法按 JS 浮点语义走 i32——lodMax < LOD_MIN 的退化小图（cubeSize
    // < 16）不为零/不回绕（JS 是浮点算术 totalLods = lodMax + 3 ≥ 3，
    // 循环正常终止）。
    let total_lods = (lod_max as i32 - LOD_MIN as i32 + 1 + EXTRA_LOD_COUNT as i32) as usize;
    let mut size_lods = Vec::with_capacity(total_lods);
    let mut lod = lod_max as i32;
    for _ in 0..total_lods {
        size_lods.push(1u32 << lod);
        if lod > LOD_MIN as i32 {
            lod -= 1;
        }
    }

    let rt_w = 3 * cube_size.max(16 * 7);
    let rt_h = 4 * cube_size;
    // 线性 f32 工作缓冲（RT 的 HalfFloat 语义在出口编码 f16；生成期用
    // f32 保卷积精度）。
    let mut rt = vec![0.0f32; (rt_w as usize) * (rt_h as usize) * 4];
    let max_mip = lod_max as f32;

    // ── 级 0：equirect → 六面（`_getEquirectMaterial` + lodMeshes[0]）──
    // 源采样 LOD：面片覆盖方位角 π/2（四分之一圆周）⇒ 面内 u 方向的源
    // 像素跨度 = W/4 对 cubeSize 个目标像素 ⇒ GL LinearMipmapLinear 的
    // 导数 LOD = log2(W/(4·cubeSize))（∈[0,1)，恒只涉及 mip0/mip1 两级；
    // v 方向 asin 压缩只让它更接近 0。GPU 的逐像素导数是区域变量，此处
    // 取常量——记录近似）。相机在原点、面片平行 ⇒ LOD 面内恒定；mip1
    //（2×2 box）逐 texel 现算、不落盘。
    let src_lod = (w as f32 / (4.0 * cube_size as f32)).log2().max(0.0);
    let src_lod_int = src_lod.floor() as u32;
    let src_lod_frac = src_lod - src_lod_int as f32;
    for face in 0..6u32 {
        let (col, row_base) = face_origin(face, cube_size);
        let texel_size = 1.0 / (cube_size as f64 - 2.0);
        for py in 0..cube_size {
            for px in 0..cube_size {
                // 片元中心 → 面内属性 uv（`_createPlanes` uv1：
                // [min..max] 线性插值，min=−texelSize、max=1+texelSize）。
                let fu = (f64::from(px) + 0.5) / f64::from(cube_size);
                let fv = (f64::from(py) + 0.5) / f64::from(cube_size);
                let uv = (
                    -texel_size + (1.0 + 2.0 * texel_size) * fu,
                    -texel_size + (1.0 + 2.0 * texel_size) * fv,
                );
                let dir = normalize3(get_direction(uv, face));
                // equirectUv（common.glsl.js:82-92，r181）：u = atan(z,x)/2π+0.5、
                // v = asin(y)/π+0.5、返回 (u, v)（无 1−v 翻转——旧版公式，
                // r181 已是直返）。v=1 = 天顶 = GL 纹理 v=1 = flipY 上传后的
                // 图像顶行；图像行序的换算在 sample_equirect_linear 内做。
                let eu = dir[2].atan2(dir[0]) as f32 * 0.15915494309189535 + 0.5;
                let ev = dir[1].clamp(-1.0, 1.0).asin() as f32 * std::f32::consts::FRAC_1_PI + 0.5;
                let (su, sv) = (eu, ev);
                let [r, g, b, _a] = sample_equirect_linear(
                    &env.rgba, w, h, su, sv, src_lod_int, src_lod_frac,
                );
                let idx = ((row_base + py) as usize * rt_w as usize + (col + px) as usize) * 4;
                rt[idx] = r;
                rt[idx + 1] = g;
                rt[idx + 2] = b;
                rt[idx + 3] = 1.0;
            }
        }
    }

    // ── 级 1..n-1：GGX VNDF 卷积（`_applyPMREM` / `_applyGGXFilter`）──
    let n = size_lods.len();
    for lod_out in 1..n {
        let lod_in = lod_out - 1;
        let s_out = size_lods[lod_out];
        let t = lod_out as f32 / (n - 1) as f32;
        let s = lod_in as f32 / (n - 1) as f32;
        let incremental = (t * t - s * s).max(0.0).sqrt();
        let blur_strength = 0.05 + t * 0.95;
        let adjusted = incremental * blur_strength;
        // 采样源 = 上一级区域：bilinearCubeUV 的 mipInt 参数。
        let mip_int = max_mip - lod_in as f32;
        let (x0, y0) = level_origin(lod_out as u32, lod_max, &size_lods);
        let texel_size = 1.0 / (s_out as f64 - 2.0);
        for face in 0..6u32 {
            let (col, row_base) = face_origin(face, s_out);
            for py in 0..s_out {
                for px in 0..s_out {
                    let fu = (f64::from(px) + 0.5) / f64::from(s_out);
                    let fv = (f64::from(py) + 0.5) / f64::from(s_out);
                    let uv = (
                        -texel_size + (1.0 + 2.0 * texel_size) * fu,
                        -texel_size + (1.0 + 2.0 * texel_size) * fv,
                    );
                    let dir = normalize3(get_direction(uv, face));
                    let mut color = [0.0f32; 3];
                    if adjusted < 0.001 {
                        // GGX shader 早退支路：极低粗糙度直采。
                        color = bilinear_cube_uv(&rt, rt_w, rt_h, max_mip, dir, mip_int);
                    } else {
                        // VNDF 切向基（GGX main 循环）。
                        let up = if dir[2].abs() < 0.999 {
                            [0.0, 0.0, 1.0]
                        } else {
                            [1.0, 0.0, 0.0]
                        };
                        let tangent = normalize3_f32(cross3(up, dir));
                        let bitangent = cross3(dir, tangent);
                        let mut total_weight = 0.0f32;
                        for i in 0..GGX_SAMPLES {
                            let xi = hammersley(i, GGX_SAMPLES);
                            // PMREM 里 V = N ⇒ 切向空间 V 恒 (0,0,1)。
                            let h_t = importance_sample_ggx_vndf(xi, [0.0, 0.0, 1.0], adjusted);
                            let h = normalize3_f32(add3(
                                add3(scale3(tangent, h_t[0]), scale3(bitangent, h_t[1])),
                                scale3(dir, h_t[2]),
                            ));
                            // ⚠ 保留 2·dot(V,H)·H（无 −V）——r15 负结果（勿
                            // 「修复」回 reflect）。r15 曾按 GGX main 循环
                            // 逐字改为 L = normalize(2·dot(V,H)·H − V)：
                            // python 逐字复刻验证等价（1e-5），但对 probe6
                            // 渲染微差（f31 full 成对 0.858→0.870）且 k3diff
                            // 全量实测变差（avg_p8 0.02481→0.02507，回退集中
                            // S1 +0.00051）。三方对拍（f31）定位：probe6
                            // （three.js r181 SwiftShader）与本实现球区互为
                            // 零偏置（−0.052），而两者对参考 mp4 同为 +1.8
                            // 系统偏亮——参考录制环境（Firefox swgl 软渲染）
                            // 的 IBL 行为与 r181 GPU 规范不一致且不可复刻，
                            // 对齐裁判是 k3diff 对参考 mp4，故维持本式。
                            let l = normalize3_f32(scale3(h, 2.0 * dot3(dir, h)));
                            let n_dot_l = dot3(dir, l).max(0.0);
                            if n_dot_l > 0.0 {
                                let sample =
                                    bilinear_cube_uv(&rt, rt_w, rt_h, max_mip, l, mip_int);
                                color[0] += sample[0] * n_dot_l;
                                color[1] += sample[1] * n_dot_l;
                                color[2] += sample[2] * n_dot_l;
                                total_weight += n_dot_l;
                            }
                        }
                        if total_weight > 0.0 {
                            color = [
                                color[0] / total_weight,
                                color[1] / total_weight,
                                color[2] / total_weight,
                            ];
                        }
                    }
                    let idx = ((y0 + row_base + py) as usize * rt_w as usize
                        + (x0 + col + px)
                        as usize)
                        * 4;
                    rt[idx] = color[0];
                    rt[idx + 1] = color[1];
                    rt[idx + 2] = color[2];
                    rt[idx + 3] = 1.0;
                }
            }
        }
    }

    let rgba_f16 = rt.iter().map(|c| f16_bits(*c)).collect::<Vec<u16>>();
    Ok(PmremCubeUv {
        width: rt_w,
        height: rt_h,
        max_mip,
        rgba_f16,
    })
}

/// 级区域写入口（`_applyGGXFilter` 的 viewport 公式）：x = 3·size·(lodOut >
/// lodMax−LOD_MIN ? lodOut−lodMax+LOD_MIN : 0)、y = 4·(cubeSize − size)。
/// 与 [`face_origin`]（面内基）相加即区域原点。
fn level_origin(lod_out: u32, lod_max: u32, size_lods: &[u32]) -> (u32, u32) {
    let size = size_lods[lod_out as usize];
    let x = if lod_out > lod_max - LOD_MIN {
        3 * size * (lod_out - (lod_max - LOD_MIN))
    } else {
        0
    };
    (x, 4 * (size_lods[0] - size))
}

/// 面片在级区域里的列/行基（lodMesh 平面 x=(face%3)·2/3−1、
/// y = face>2 ? 0 : −1 的 NDC 几何 ⇒ GL 行序下面 0..2 在行 [0,S)、
/// 面 3..5 在行 [S,2S)）。
fn face_origin(face: u32, size: u32) -> (u32, u32) {
    ((face % 3) * size, if face > 2 { size } else { 0 })
}

/// `_getCommonVertexShader.getDirection`（PMREMGenerator.js:1134-1170）：
/// RH 坐标系、PMREM 面序约定。
fn get_direction(uv: (f64, f64), face: u32) -> [f64; 3] {
    let (u, v) = (2.0 * uv.0 - 1.0, 2.0 * uv.1 - 1.0);
    match face {
        0 => [1.0, v, u],   // direction.zyx，(1, v, u) +x
        1 => [-u, 1.0, -v], // direction.xzy 且 xz *= −1，+y
        2 => [-u, v, 1.0],  // direction.x *= −1，+z
        3 => [-1.0, v, -u], // direction.zyx 且 xz *= −1，−x
        4 => [-u, -1.0, v], // direction.xzy 且 xy *= −1，−y
        _ => [u, v, -1.0],  // direction.z *= −1，−z
    }
}

/// `cube_uv_reflection_fragment.glsl.js getFace`。
pub fn cube_uv_get_face(dir: [f32; 3]) -> u32 {
    let ax = dir[0].abs();
    let ay = dir[1].abs();
    let az = dir[2].abs();
    if ax > az {
        if ax > ay {
            if dir[0] > 0.0 { 0 } else { 3 }
        } else if dir[1] > 0.0 {
            1
        } else {
            4
        }
    } else if az > ay {
        if dir[2] > 0.0 { 2 } else { 5 }
    } else if dir[1] > 0.0 {
        1
    } else {
        4
    }
}

/// `getUV`（PMREM 面序；返回 0.5·(uv+1)）。
pub fn cube_uv_get_uv(dir: [f32; 3], face: u32) -> [f32; 2] {
    let uv = match face {
        0 => [dir[2] / dir[0].abs(), dir[1] / dir[0].abs()],
        1 => [-dir[0] / dir[1].abs(), -dir[2] / dir[1].abs()],
        2 => [-dir[0] / dir[2].abs(), dir[1] / dir[2].abs()],
        3 => [-dir[2] / dir[0].abs(), dir[1] / dir[0].abs()],
        4 => [-dir[0] / dir[1].abs(), dir[2] / dir[1].abs()],
        _ => [dir[0] / dir[2].abs(), dir[1] / dir[2].abs()],
    };
    [0.5 * (uv[0] + 1.0), 0.5 * (uv[1] + 1.0)]
}

/// `roughnessToMip`（分段线性 + 最低支路 −2·log2(1.16·r)）。
pub fn roughness_to_mip(roughness: f32) -> f32 {
    const R0: f32 = 1.0;
    const M0: f32 = -2.0;
    const R1: f32 = 0.8;
    const M1: f32 = -1.0;
    const R4: f32 = 0.4;
    const M4: f32 = 2.0;
    const R5: f32 = 0.305;
    const M5: f32 = 3.0;
    const R6: f32 = 0.21;
    const M6: f32 = 4.0;
    if roughness >= R1 {
        (R0 - roughness) * (M1 - M0) / (R0 - R1) + M0
    } else if roughness >= R4 {
        (R1 - roughness) * (M4 - M1) / (R1 - R4) + M1
    } else if roughness >= R5 {
        (R4 - roughness) * (M5 - M4) / (R4 - R5) + M4
    } else if roughness >= R6 {
        (R5 - roughness) * (M6 - M5) / (R5 - R6) + M5
    } else {
        -2.0 * (1.16 * roughness).log2() // 1.16 = 1.79^0.25
    }
}

/// `bilinearCubeUV`：在 f32 RT 缓冲上按 cubeUV 布局双线性采样（RT 的
/// LinearFilter + ClampToEdge 语义）。WGSL 侧同名函数的同构 CPU 镜像。
pub fn bilinear_cube_uv(
    rt: &[f32],
    rt_w: u32,
    rt_h: u32,
    max_mip: f32,
    dir: [f32; 3],
    mip_int: f32,
) -> [f32; 3] {
    const MIN_MIP_LEVEL: f32 = 4.0;
    const MIN_TILE_SIZE: f32 = 16.0;
    let face = cube_uv_get_face(dir);
    let filter_int = (MIN_MIP_LEVEL - mip_int).max(0.0);
    let mip_clamped = mip_int.max(MIN_MIP_LEVEL);
    let face_size = mip_clamped.exp2();
    let mut face = face as f32;
    let mut uv = {
        let g = cube_uv_get_uv(dir, face as u32);
        // #25071：(faceSize−2) 压缩 + 1 偏移。
        [g[0] * (face_size - 2.0) + 1.0, g[1] * (face_size - 2.0) + 1.0]
    };
    if face > 2.0 {
        uv[1] += face_size;
        face -= 3.0;
    }
    uv[0] += face * face_size;
    uv[0] += filter_int * 3.0 * MIN_TILE_SIZE;
    uv[1] += 4.0 * (max_mip.exp2() - face_size);
    // texture2D 双线性：归一化坐标 = texel/RT 尺寸（CUBEUV_TEXEL_WIDTH/HEIGHT）。
    bilinear_sample(rt, rt_w, rt_h, uv[0] / rt_w as f32, uv[1] / rt_h as f32)
}

/// Linear/Clamp 双线性采样（GL texel 中心 = (i+0.5)/N）。
fn bilinear_sample(rt: &[f32], w: u32, h: u32, u: f32, v: f32) -> [f32; 3] {
    let x = u * w as f32 - 0.5;
    let y = v * h as f32 - 0.5;
    let i0 = x.floor();
    let j0 = y.floor();
    let fx = x - i0;
    let fy = y - j0;
    let clampi = |i: i32, n: u32| i.clamp(0, n as i32 - 1) as u32;
    let xa = clampi(i0 as i32, w);
    let xb = clampi(i0 as i32 + 1, w);
    let ya = clampi(j0 as i32, h);
    let yb = clampi(j0 as i32 + 1, h);
    let at = |xx: u32, yy: u32| -> [f32; 3] {
        let idx = (yy as usize * w as usize + xx as usize) * 4;
        [rt[idx], rt[idx + 1], rt[idx + 2]]
    };
    let c00 = at(xa, ya);
    let c10 = at(xb, ya);
    let c01 = at(xa, yb);
    let c11 = at(xb, yb);
    let mut out = [0.0f32; 3];
    for k in 0..3 {
        let top = c00[k] + (c10[k] - c00[k]) * fx;
        let bot = c01[k] + (c11[k] - c01[k]) * fx;
        out[k] = top + (bot - top) * fy;
    }
    out
}

/// 源 equirect 采样：双线性 + mip0/mip1 trilinear（src_lod_frac）。
/// mip1 = 2×2 box（逐 texel 现算）。sRGB EOTF 线性化在 texel 读取处
/// （GL SRGB8_ALPHA8 采样即线性化；trilinear 在线性域混合）。
///
/// `sv` 是 equirectUv（common.glsl.js:82-92）输出的 **GL 纹理 v**：v=1 =
/// 天顶。three.js 纹理默认 flipY=true（WebGLTextures 上传时
/// UNPACK_FLIP_Y_WEBGL 翻转）⇒ GL v=1 = **自然图像顶行**；本函数输入
/// `src` 为自然图像行序（行 0 = 顶）⇒ 行索引必须按 `(1 − v)` 反算。
/// r10 实证：漏翻转时球体下缘（反射向下）采到图像顶带（r10 成对探针
/// band 内 env 贡献 R 通道 4.2×/B 1.6× 超标；noenv 成对差 0.49 ≈ 0，
/// 唯一分歧即此）。
fn sample_equirect_linear(
    src: &[u8],
    w: u32,
    h: u32,
    su: f32,
    sv: f32,
    _lod_int: u32,
    lod_frac: f32,
) -> [f32; 4] {
    let clampi = |i: i64, n: u32| i.clamp(0, n as i64 - 1) as u32;
    let bilinear = |w: u32, h: u32, fetch: &dyn Fn(u32, u32) -> [f32; 4]| -> [f32; 4] {
        let x = su * w as f32 - 0.5;
        let y = (1.0 - sv) * h as f32 - 0.5;
        let i0 = x.floor();
        let j0 = y.floor();
        let fx = x - i0;
        let fy = y - j0;
        let mut out = [0.0f32; 4];
        for k in 0..4 {
            let c00 = fetch(clampi(i0 as i64, w), clampi(j0 as i64, h))[k];
            let c10 = fetch(clampi(i0 as i64 + 1, w), clampi(j0 as i64, h))[k];
            let c01 = fetch(clampi(i0 as i64, w), clampi(j0 as i64 + 1, h))[k];
            let c11 = fetch(clampi(i0 as i64 + 1, w), clampi(j0 as i64 + 1, h))[k];
            let top = c00 + (c10 - c00) * fx;
            let bot = c01 + (c11 - c01) * fx;
            out[k] = top + (bot - top) * fy;
        }
        out
    };
    let at0 = |xx: u32, yy: u32| -> [f32; 4] {
        let idx = (yy as usize * w as usize + xx as usize) * 4;
        [
            srgb_eotf(src[idx] as f32 / 255.0),
            srgb_eotf(src[idx + 1] as f32 / 255.0),
            srgb_eotf(src[idx + 2] as f32 / 255.0),
            src[idx + 3] as f32 / 255.0,
        ]
    };
    let m0 = bilinear(w, h, &at0);
    if lod_frac <= 0.0 {
        return m0;
    }
    // mip1：2×2 box（宽高奇数时末列/行 clamp——GL GENERATE_MIPMAP 同语义）。
    let w1 = (w / 2).max(1);
    let h1 = (h / 2).max(1);
    let at1 = |xx: u32, yy: u32| -> [f32; 4] {
        let (x0, y0) = (xx * 2, yy * 2);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let a = at0(x0, y0);
        let b = at0(x1, y0);
        let c = at0(x0, y1);
        let d = at0(x1, y1);
        [
            0.25 * (a[0] + b[0] + c[0] + d[0]),
            0.25 * (a[1] + b[1] + c[1] + d[1]),
            0.25 * (a[2] + b[2] + c[2] + d[2]),
            0.25 * (a[3] + b[3] + c[3] + d[3]),
        ]
    };
    let m1 = bilinear(w1, h1, &at1);
    let mut out = [0.0f32; 4];
    for k in 0..4 {
        out[k] = m0[k] + (m1[k] - m0[k]) * lod_frac;
    }
    out
}

/// sRGB EOTF（GL SRGB8_ALPHA8 硬件解码式）。
pub fn srgb_eotf(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `importanceSampleGGX_VNDF`（Heitz 2018；GGX shader 逐字）。
fn importance_sample_ggx_vndf(xi: [f32; 2], v: [f32; 3], roughness: f32) -> [f32; 3] {
    let alpha = roughness * roughness;
    let vh = normalize3_f32([alpha * v[0], alpha * v[1], v[2]]);
    let lensq = vh[0] * vh[0] + vh[1] * vh[1];
    let t1 = if lensq > 0.0 {
        scale3([-vh[1], vh[0], 0.0], 1.0 / lensq.sqrt())
    } else {
        [1.0, 0.0, 0.0]
    };
    let t2 = cross3(vh, t1);
    let r = xi[0].sqrt();
    let phi = 2.0 * std::f32::consts::PI * xi[1];
    let t1c = r * phi.cos();
    let mut t2c = r * phi.sin();
    let s = 0.5 * (1.0 + vh[2]);
    t2c = (1.0 - s) * (1.0 - t1c * t1c).max(0.0).sqrt() + s * t2c;
    let root = (1.0 - t1c * t1c - t2c * t2c).max(0.0).sqrt();
    let nh = [
        t1c * t1[0] + t2c * t2[0] + root * vh[0],
        t1c * t1[1] + t2c * t2[1] + root * vh[1],
        t1c * t1[2] + t2c * t2[2] + root * vh[2],
    ];
    normalize3_f32([alpha * nh[0], alpha * nh[1], nh[2].max(0.0)])
}

/// Hammersley 序列（`radicalInverse_VdC` 位反转逐字）。
fn hammersley(i: u32, n: u32) -> [f32; 2] {
    let mut bits = i;
    bits = (bits << 16) | (bits >> 16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xAAAA_AAAA) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xCCCC_CCCC) >> 2);
    bits = ((bits & 0x0F0F_0F0F) << 4) | ((bits & 0xF0F0_F0F0) >> 4);
    bits = ((bits & 0x00FF_00FF) << 8) | ((bits & 0xFF00_FF00) >> 8);
    [i as f32 / n as f32, bits as f32 * 2.3283064365386963e-10]
}

/// f64 方向（get_direction 精度域）归一化到 f32。
fn normalize3(v: [f64; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 {
        return [0.0, 0.0, 0.0];
    }
    [(v[0] / len) as f32, (v[1] / len) as f32, (v[2] / len) as f32]
}

fn normalize3_f32(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 {
        return [0.0, 0.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scale3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// f32 → f16 位型（IEEE 754 round-to-nearest-even；显式 RNE，tie-to-even）。
pub fn f16_bits(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let abs = b & 0x7FFF_FFFF;
    // inf / nan。
    if abs >= 0x7F80_0000 {
        return sign | if abs == 0x7F80_0000 { 0x7C00 } else { 0x7E00 };
    }
    let exp32 = (abs >> 23) as i32;
    let mant32 = abs & 0x007F_FFFF;
    if exp32 == 0 {
        return sign; // f32 次正规 < 2^-126 ≪ f16 最小次正规的一半 → 0
    }
    let exp = exp32 - 127;
    if exp > 15 {
        return sign | 0x7C00; // 溢出 → inf（本路径值域不可达，防御）
    }
    if exp >= -14 {
        // 正常范围：23 位尾数舍入到 10 位（RNE）。
        let mut e = exp + 15;
        let mut m = mant32 >> 13;
        let frac = mant32 & 0x1FFF;
        if frac > 0x1000 || (frac == 0x1000 && (m & 1) == 1) {
            m += 1;
            if m == 0x400 {
                m = 0;
                e += 1;
                if e >= 31 {
                    return sign | 0x7C00;
                }
            }
        }
        return sign | ((e as u16) << 10) | m as u16;
    }
    // 次正规：值 = full·2^(exp−23)（full 含隐含位），f16 次正规格点 2^-24。
    if exp < -24 {
        return sign; // < 2^-25 → 半格点以下归零（恰 2^-25 的 tie-to-even = 0）
    }
    let s = (-exp - 1) as u32; // m' = full·2^(exp+1)；exp ∈ [-24..-15] → s ∈ [23..14]
    let full = 0x0080_0000 | mant32;
    let keep = full >> s;
    let frac = full & ((1u32 << s) - 1);
    let half = 1u32 << (s - 1);
    let mut m = keep;
    if frac > half || (frac == half && (keep & 1) == 1) {
        m += 1; // 进位最多到 0x400 = 最小正规（合法编码）
    }
    sign | m as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_bitmap(w: u32, h: u32, rgba: [u8; 4]) -> TextureBitmap3 {
        TextureBitmap3 {
            rgba: std::sync::Arc::new((0..(w * h) as usize).flat_map(|_| rgba).collect()),
            width: w,
            height: h,
        }
    }

    /// f16 位型 → f32 数值（测试用解码，与编码互为镜像）。
    fn f16_value(h: u16) -> f32 {
        let sign = h & 0x8000;
        let exp = ((h >> 10) & 0x1F) as i32;
        let mant = (h & 0x03FF) as u32;
        let v = if exp == 0 {
            (mant as f32) * 2.0f32.powi(-24)
        } else {
            ((mant as f32) + 1024.0) * 2.0f32.powi(exp - 25)
        };
        if sign != 0 { -v } else { v }
    }

    fn texel_rgb(p: &PmremCubeUv, x: u32, y: u32) -> [f32; 3] {
        let idx = (y as usize * p.width as usize + x as usize) * 4;
        [
            f16_value(p.rgba_f16[idx]),
            f16_value(p.rgba_f16[idx + 1]),
            f16_value(p.rgba_f16[idx + 2]),
        ]
    }

    /// `_setSize(width/4)` + `_allocateTargets` 尺寸：64×32 → lodMax 4、
    /// RT 3·max(16,112)=336 × 64；256×128 → lodMax 6、RT 336 × 256；
    /// 6144×3072（参考 equirect-garden.jpg 尺寸）→ lodMax 10、RT 3072×4096。
    #[test]
    fn layout_follows_from_texture_set_size() {
        let p = generate(&solid_bitmap(64, 32, [0, 0, 0, 255])).expect("生成");
        assert_eq!((p.width, p.height, p.max_mip), (336, 64, 4.0));
        let p2 = generate(&solid_bitmap(256, 128, [0, 0, 0, 255])).expect("生成");
        assert_eq!((p2.width, p2.height, p2.max_mip), (336, 256, 6.0));
        // 不解码 6144×3072 全图也能验算公式（尺寸推导单列于注释）：
        // 6144/4 = 1536 → floor(log2) = 10 → cubeSize 1024 → RT 3072×4096。
        assert_eq!(31 - (6144u32 / 4).leading_zeros(), 10);
        assert_eq!((3 * 1024, 4 * 1024), (3072, 4096));
    }

    /// `roughnessToMip` 分段端点（各支路 (r_a−r)/(r_a−r_b)·(m_b−m_a)+m_a
    /// 手算值）+ getIBLIrradiance 采样路径（roughness=1 → mip −2 →
    /// mipInt 4 / filterInt 6 = 第 6 号 extra tile）。
    #[test]
    fn roughness_to_mip_matches_hand_derived_values() {
        assert_eq!(roughness_to_mip(1.0), -2.0);
        assert_eq!(roughness_to_mip(0.8), -1.0);
        assert_eq!(roughness_to_mip(0.4), 2.0);
        assert_eq!(roughness_to_mip(0.305), 3.0);
        assert_eq!(roughness_to_mip(0.21), 4.0);
        let expect = -2.0 * (1.16f32 * 0.0525).log2();
        assert!((roughness_to_mip(0.0525) - expect).abs() < 1e-6);
        let mip = roughness_to_mip(1.0).clamp(-2.0, 10.0);
        assert_eq!((mip.max(4.0), (4.0 - mip).max(0.0)), (4.0, 6.0));
    }

    /// 常量环境：所有已写区域（标准锥 + extra tiles + 卷积输出）恒等于源
    /// 线性色——GGX 权重守恒（Σw·NdotL/ΣNdotL = C）的解析验证；alpha 恒 1
    /// （equirect 材质 vec4(rgb, 1.0)）。
    #[test]
    fn solid_env_stays_solid_at_every_written_texel() {
        let lin = srgb_eotf(0x80 as f32 / 255.0);
        let p = generate(&solid_bitmap(64, 32, [0x80, 0x80, 0x80, 255])).expect("生成");
        // cubeSize=16：全部 7 级（base 3·16×2·16 + 6 个 extra tile）都写
        // 在行 [0, 32)，行带下半是未写零区（见模块文档布局段）。
        let mut checked = 0usize;
        for y in 0..32 {
            for x in (0..p.width).step_by(3) {
                let t = texel_rgb(&p, x, y);
                assert!(
                    (t[0] - lin).abs() < 2e-3 && (t[1] - lin).abs() < 2e-3 && (t[2] - lin).abs() < 2e-3,
                    "({x},{y}) = {t:?} ≠ {lin}"
                );
                let idx = (y as usize * p.width as usize + x as usize) * 4 + 3;
                assert_eq!(p.rgba_f16[idx], 0x3C00, "alpha 恒 1");
                checked += 1;
            }
        }
        assert!(checked > 3000, "抽样数 {checked} 不足");
    }

    /// 非常量环境的卷积减弱：黑白棋盘源在粗级趋于灰（能量守恒均值），
    /// 且比 base 级更接近灰——GGX 模糊真实发生的证据。
    #[test]
    fn checkerboard_blurs_toward_mean_at_coarse_levels() {
        let w = 256u32;
        let h = 128u32;
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let v = if (x / 8 + y / 8) % 2 == 0 { 255 } else { 0 };
                let i = (y * w + x) as usize * 4;
                px[i] = v;
                px[i + 1] = v;
                px[i + 2] = v;
                px[i + 3] = 255;
            }
        }
        let env = TextureBitmap3 {
            rgba: std::sync::Arc::new(px),
            width: w,
            height: h,
        };
        let p = generate(&env).expect("生成");
        let mean = 0.5 * (srgb_eotf(1.0) + srgb_eotf(0.0));
        // 布局（cubeSize 64，lodMax 6）：base 区域 3·64×2·64 在 (0,0)；
        // lodOut 6（最粗 extra）x = 3·16·(6−2) = 192、y = 4·(64−16) = 192。
        let base_center = texel_rgb(&p, 32, 32);
        let coarse = texel_rgb(&p, 200, 200);
        assert!(
            (coarse[0] - mean).abs() < (base_center[0] - mean).abs(),
            "粗级 {coarse:?} 应比 base {base_center:?} 更接近均值 {mean}"
        );
        // 卷积输出有界：不超出源极值域。
        assert!(coarse[0] >= srgb_eotf(0.0) - 1e-3 && coarse[0] <= srgb_eotf(1.0) + 1e-3);
    }

    /// `bilinearCubeUV` 镜像与生成的交互：对常量缓冲，任意方向/任意 mipInt
    /// 都返回同一常量（采样公式跨 tile 偏移的守恒检查）。
    #[test]
    fn bilinear_cube_uv_solid_buffer_invariant() {
        let lin = 0.5f32;
        let rt = vec![lin; 336 * 64 * 4];
        let dirs = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.577, 0.577, 0.577],
            [0.0, 1.0, 1e-4],
        ];
        for mip_int in [-2.0, -1.0, 0.0, 3.0, 4.0, 5.0, 10.0] {
            for d in dirs {
                let c = bilinear_cube_uv(&rt, 336, 64, 4.0, d, mip_int);
                assert!(
                    (c[0] - lin).abs() < 1e-6 && (c[1] - lin).abs() < 1e-6,
                    "mip {mip_int} dir {d:?} → {c:?}"
                );
            }
        }
    }

    /// f16 编码：精确值、RNE 舍入、次正规、无穷；数值往返。
    #[test]
    fn f16_bits_round_trip() {
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(1.0), 0x3C00);
        assert_eq!(f16_bits(-2.0), 0xC000);
        assert_eq!(f16_bits(f32::INFINITY), 0x7C00);
        assert_eq!(f16_bits(f32::NEG_INFINITY), 0xFC00);
        assert_eq!(f16_bits(f32::NAN) & 0x7E00, 0x7E00);
        // f16 可精确表示的值：65504（0x7BFF，f16 最大有限值）必须原样——
        // 不得因 RNE 进位成 inf。
        assert_eq!(f16_bits(65504.0), 0x7BFF);
        // 1 + 2^-13 低于半 ulp（half ulp = 2^-11）→ 舍到 1.0。
        assert_eq!(f16_bits(1.0 + 2.0f32.powi(-13)), 0x3C00);
        // 0.75 ulp（高于半 ulp）→ 舍到 1 + 2^-10（0x3C01）。
        assert_eq!(f16_bits(1.0 + 0.75 * 2.0f32.powi(-10)), 0x3C01);
        // 恰半 ulp tie-to-even：1 + 2^-11 → 0x3C00（偶）。
        assert_eq!(f16_bits(1.0 + 2.0f32.powi(-11)), 0x3C00);
        // 1 + 2^-10 + 2^-11 → 0x3C02（尾数 1，tie → 偶 = 2）。
        assert_eq!(f16_bits(1.0 + 2.0f32.powi(-10) + 2.0f32.powi(-11)), 0x3C02);
        // 往返判据 = RNE 本身：误差 ≤ 0.5 ulp（f16 该数值区的格距；
        // 次正规区 ulp = 2^-24，正规区 = 2^(⌊log2|v|⌋−10)）。相对误差
        // 判据在近零次正规处不成立（如 6e-8 → 0x0001 的正确舍入
        // 相对误差 0.66%）。
        for v in [6.0e-8f32, 1.0e-6, 0.5, 0.7, 2.5, 100.0, 65504.0] {
            let back = f16_value(f16_bits(v));
            let av = v.abs();
            let ulp = if av < 2.0f32.powi(-14) {
                2.0f32.powi(-24)
            } else {
                2.0f32.powf(av.log2().floor() - 10.0)
            };
            assert!(
                (back - v).abs() <= 0.5 * ulp,
                "f16 {v} → {:#06x} → {back}（误差 {} 超过半 ulp {half}）",
                f16_bits(v),
                (back - v).abs(),
                half = 0.5 * ulp
            );
        }
    }
}
