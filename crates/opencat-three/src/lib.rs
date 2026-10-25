//! opencat-three — engine 侧 3D 渲染后端（wgpu）。
//!
//! 对称于 Skia（engine 侧 2D），本 crate 光栅化 [`scene3::Scene3`]（纯数据
//! 快照，定义在 core），吐 straight-alpha RGBA8（`Rgba8Unorm`，未预乘，长度
//! `w*h*4`）——与 [`opencat_core::ir::generated_image::GeneratedImageEntry`]
//! 的契约一致，经占位 `DrawOp::Image{ImageRef::Generated}` 进合成。
//!
//! web 侧的 3D 执行器是浏览器里的真 three.js（`three-facade.ts`，钉 r181）：
//! Scene3 IR 以 three.js 为语义标准，真 three.js 是其原生执行器（GLSL 源
//! 逐字直喂，绕开 wgpu 的 naga 翻译）。wgpu wasm 化是未来可选项——本 crate
//! 不依赖任何不可移植 construct，随时可插回；届时恢复约束：**WebGL2 没有
//! compute 与 SSBO，着色器只允许 uniform buffer**。
//!
//! 确定性：同一 [`Scene3`] 在同一后端上逐字节一致（无时间、无 IO、无回调）；
//! wgpu 与真 three.js 之间是容差级一致，对齐走 k3diff 的 mae/maxd/p8 指标。

use std::borrow::Cow;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(target_os = "macos")]
use objc2::rc::Retained;

use anyhow::{Result, bail};
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3, Vec4, camera::rh::{proj, view}};
pub use opencat_core::ir::scene3::{
    self, Geometry3, Light3, Material3, Mesh3, OutputColorSpace3, Scene3, Shading3, ShaderUniform3,
    ShaderValue3, ToneMapping3,
};
use wgpu::util::DeviceExt;

use opencat_gpu_native::{NativeTexture, TextureRegistry};

mod dfg_lut;
use dfg_lut::{DFG_LUT_DATA, DFG_LUT_SIZE};
pub mod pmrem;

/// 每对象 uniform 上限的护栏：原型期一次性建缓冲，网格数多了换动态偏移。
const MAX_MESHES_PER_FRAME: usize = 256;

/// 每帧纹理槽上限。8 路视频槽（参考 js:245-247 uVideo0..7）+ 4 槽余量
/// （r7：wordmark 的 `map` 位图等静态贴图走槽 8+）。group(2) 布局里槽
/// 0..7 绑定 0..7、槽 8..11 绑定 12..15（绑定 8..11 留给共享采样器 8、
/// 透射 RT 9、Clamp 采样器 10、DFG LUT 11——既有绑定号不动，WGSL 物理
/// 路径逐位不变）；布局采样纹理总数 12+2=14 ≤ wgpu/WebGL2 片元默认
/// 预算 16（`max_sampled_textures_per_shader_stages`）。
pub const MAX_TEXTURE_SLOTS: usize = 40;

/// 纹理槽位（表下标）→ group(2) 绑定号。槽 0..7 直映射（r6 及之前的
/// 8 路视频槽），槽 8..11 跳过 8..11 号保留绑定映射到 12..15。
fn slot_binding(slot: usize) -> u32 {
    if slot < 8 {
        slot as u32
    } else {
        (slot + 4) as u32
    }
}

const GLOBALS_SIZE: u64 = 432; // mat4*3 + vec4*15
const OBJECT_SIZE: u64 = 480; // mat4*2 + vec4<f32>*11 + vec4<u32>*3 + vec4<u32>[2]*3 + vec4<f32>[3] + vec4<u32>*2

const _: () = assert!(std::mem::size_of::<GlobalsUniform>() as u64 == GLOBALS_SIZE);
const _: () = assert!(std::mem::size_of::<ObjectUniform>() as u64 == OBJECT_SIZE);

/// 一张已解码纹理（宿主解析 [`scene3::Texture3`] 来源后的像素载荷；
/// RGBA8 straight-alpha，行紧凑，长度 `width*height*4`）。
#[derive(Clone, Debug)]
pub struct TextureBitmap3 {
    pub rgba: std::sync::Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

/// PMREM 缓存键：env 位图的内容指纹。
///
/// 静态 env 逐帧字节相同 ⇒ 命中；动态 env（脚本 surface）只在真正变化时
/// 重算。用 FxHash 风格的手写折叠，避免为一次非热点调用引入 hasher。
fn env_content_key(env: &TextureBitmap3, srgb: bool) -> usize {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a 偏移基
    h ^= env.width as u64;
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
    h ^= env.height as u64;
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
    // 逐字节哈希 4 MB 太贵；按 16 B 步长跳跃采样——PMREM 只依赖低频亮度
    // 分布，跳采样足以区分内容，且成本降到 1/16。
    for b in env.rgba.iter().step_by(16) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^= srgb as u64;
    h as usize
}

/// 本帧纹理表：槽位（下标）→ (表键, 位图)。表键 =
/// [`scene3::Texture3::table_key`]；槽位由遍历规则决定（先逐 mesh 的
/// `textures` 槽表、后 `map`，场景序），宿主与渲染器按同一规则推出同序。
pub type TextureTable3 = Vec<(String, TextureBitmap3)>;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalsUniform {
    view_proj: [[f32; 4]; 4],
    /// 视图矩阵（透射折射投影 `proj × view × exit` 用；three.js
    /// transmission_pars `viewMatrix` 对应物）。
    view: [[f32; 4]; 4],
    /// 投影矩阵（同上，`projMatrix` 对应物）。
    proj: [[f32; 4]; 4],
    /// xyz = 光传播方向（已归一化），w = 0。方向光 1（场景序第一个）。
    light_dir: [f32; 4],
    /// rgb = 颜色×强度。
    light_color: [f32; 4],
    ambient: [f32; 4],
    /// HemisphereLight（场景序第一个）：rgb = 颜色×强度。
    hemi_sky: [f32; 4],
    hemi_ground: [f32; 4],
    /// 方向光 2（场景序第二个）。
    light_dir2: [f32; 4],
    light_color2: [f32; 4],
    /// 视口尺寸（px）——主面屏幕空间 UV（`gl_FragCoord/uResolution` 的
    /// GL 语义等价形式）。
    resolution: [f32; 4],
    /// xyz = 相机世界位置（viewDir = normalize(cameraPos − worldPos)，
    /// three.js `cameraPosition` uniform 对应物）。
    camera_pos: [f32; 4],
    /// 输出变换（three.js renderer 设置，⑥ js:18-20）：x = tonemap 开关
    /// （0 = NoToneMapping / 1 = ACESFilmic），y = toneMappingExposure，
    /// z = 输出 OETF（0 = Linear / 1 = sRGB）。主 pass（内建材质分支）
    /// 读它；透射 RT pass 绑另一份 globals（全 0——WebGLRenderer.js
    /// :1921-1922 RT 二遍强制 NoToneMapping、:2197 RT 输出 LinearSRGB）；
    /// ShaderMaterial 程序不读它（three.js 不向 ShaderMaterial 注入
    /// tonemapping/colorspace chunk，WebGLProgram.js:831-833）。
    output_xform: [f32; 4],
    /// 点光 1/2：xyz = 世界位置、w = 衰减指数（three.js `decay`）。
    point_pos: [[f32; 4]; 2],
    /// 点光 1/2：rgb = 颜色×强度、w = 光照半径（0 = 不衰减）。
    point_color: [[f32; 4]; 2],
    /// 环境 cubeUV 采样参数（WebGLProgram.js generateCubeUVSize 的
    /// 对应物，set_environment 按 imageHeight=RT 高推得）：x = texelWidth、
    /// y = texelHeight、z = CUBEUV_MAX_MIP、w = 场景带 environment 开关
    /// （0 = 无 env → WGSL getIBL* 归零，对应 `USE_ENVMAP` 未定义）。
    env_cubuv: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectUniform {
    model: [[f32; 4]; 4],
    /// 模型矩阵逆转置的 3x3（非均匀缩放下法线正确），升位成 mat4。
    normal_mat: [[f32; 4]; 4],
    base_color: [f32; 4],
    /// x：0 = Unlit，1 = Lambert，2 = VideoMosaic。
    flags: [u32; 4],
    /// 噪声顶点位移参数：amplitude, frequency, world_frequency, time_speed。
    disp_params: [f32; 4],
    /// x = time（脚本逐帧赋值的 noiseTime）。
    disp_time: [f32; 4],
    /// x = 单贴图槽位（0xFFFF_FFFF = 无 map）。
    tex: [u32; 4],
    /// COLOR_1 字节 id → 纹理槽位（槽 0..3 / 4..7）。
    face_to_texture: [[u32; 4]; 2],
    /// 逐槽 corner 规则的 shift a（色表下标 = (dominant + a) % 3）。
    corner_a: [[u32; 4]; 2],
    /// 逐槽 corner 规则的 invert 标志（≠0 → 取 2 − 下标）。
    corner_inv: [[u32; 4]; 2],
    /// corner 色表（3 色，按 COLOR_0 主导通道序）。
    corner_colors: [[f32; 4]; 3],
    /// x = frost 系数，y = surface UV 裁剪常数。
    mosaic: [f32; 4],
    /// PBR 透射参数 A：ior, thickness, attenuation_distance, dispersion。
    phys_a: [f32; 4],
    /// PBR 透射参数 B：transmission, roughness, metalness, specular_intensity。
    phys_b: [f32; 4],
    /// PBR 透射参数 C：clearcoat, clearcoat_roughness, 0, 0。
    phys_c: [f32; 4],
    /// rgb = Beer-Lambert 体吸收色，a = 0。
    attenuation_color: [f32; 4],
    /// PBR 透射参数 D：x = envMapIntensity（材质级 IBL 强度，envmap chunk
    /// uniform；js:172 = 1.2），y/z/w 备用。
    phys_d: [f32; 4],
    /// glTF PBR 贴图槽（Physical 分支消费）：x = baseColor，
    /// y = metallicRoughness（0xFFFF_FFFF = 无）。
    pbr_tex: [u32; 4],
    /// 法线贴图槽，0xFFFF_FFFF = 无。
    normal_tex: [u32; 4],
}

/// WGSL 是唯一着色源：engine（Vulkan/Metal/GL）与 web（WebGPU/WebGL2）
/// 都从这份源编译。只用 uniform + sampled texture —— 见模块文档的 WebGL2
/// 硬约束。位移噪声 = Ashima 3D simplex noise（与参考源 GLSL 逐字同源，
/// js:112-159，公有领域工具函数，非 liquid 专用代码）。
const WGSL: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,       // 透射折射投影 proj × view × exit（viewMatrix 对应物）
    proj: mat4x4<f32>,       // projMatrix 对应物
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
    hemi_sky: vec4<f32>,
    hemi_ground: vec4<f32>,
    light_dir2: vec4<f32>,
    light_color2: vec4<f32>,
    resolution: vec4<f32>,   // x,y = 视口 px（主面屏幕空间 UV）
    camera_pos: vec4<f32>,   // xyz = 相机世界位置（cameraPosition 对应物）
    output_xform: vec4<f32>, // x = tonemap 开关, y = exposure, z = 输出 OETF（1 = sRGB）
    point_pos: array<vec4<f32>, 2>,   // xyz = 世界位置, w = decay
    point_color: array<vec4<f32>, 2>, // rgb = 颜色×强度, w = 光照半径（0 = 不衰减）
    env_cubuv: vec4<f32>,    // x = texelWidth, y = texelHeight, z = CUBEUV_MAX_MIP, w = 带 env 开关
};
struct Object {
    model: mat4x4<f32>,
    normal_mat: mat4x4<f32>,
    base_color: vec4<f32>,
    flags: vec4<u32>,        // x: 0=Unlit 1=Lambert 2=VideoMosaic 3=Physical
    disp_params: vec4<f32>,  // amplitude, frequency, world_frequency, time_speed
    disp_time: vec4<f32>,    // x = time
    tex: vec4<u32>,          // x = 单贴图槽位（0xFFFFFFFF = 无 map）
    face_to_texture: array<vec4<u32>, 2>,  // COLOR_1 字节 id → 槽位
    corner_a: array<vec4<u32>, 2>,         // 逐槽 corner shift
    corner_inv: array<vec4<u32>, 2>,       // 逐槽 corner invert
    corner_colors: array<vec4<f32>, 3>,    // corner 色表
    mosaic: vec4<f32>,       // x = frost，y = surface UV 裁剪常数
    phys_a: vec4<f32>,       // ior, thickness, attenuation_distance, dispersion
    phys_b: vec4<f32>,       // transmission, roughness, metalness, specular_intensity
    phys_c: vec4<f32>,       // clearcoat, clearcoat_roughness
    attenuation_color: vec4<f32>,  // rgb = 体吸收色
    phys_d: vec4<f32>,             // x = envMapIntensity（材质级 IBL 强度）
    pbr_tex: vec4<u32>,            // x = baseColor 槽, y = metallicRoughness 槽
    normal_tex: vec4<u32>,         // x = 法线贴图槽
};
@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> object: Object;
// 纹理槽 0..11 + 共享采样器（Linear/Linear + Repeat/Repeat：three.js
// VideoTexture minFilter/magFilter Linear、wrapS/T Repeat 的对应固化）。
// 槽→绑定的映射见 slot_binding()（8..11 号绑定保留给下方专用资源）。
@group(2) @binding(0) var tex0: texture_2d<f32>;
@group(2) @binding(1) var tex1: texture_2d<f32>;
@group(2) @binding(2) var tex2: texture_2d<f32>;
@group(2) @binding(3) var tex3: texture_2d<f32>;
@group(2) @binding(4) var tex4: texture_2d<f32>;
@group(2) @binding(5) var tex5: texture_2d<f32>;
@group(2) @binding(6) var tex6: texture_2d<f32>;
@group(2) @binding(7) var tex7: texture_2d<f32>;
// 槽 8..31（静态贴图槽：GLB 的 baseColor / metallicRoughness / 屏幕 surface），
// 绑定 = 槽 + 4（8..11 号绑定被下方专用资源占用；16 留给环境贴图，故再跳过）。
@group(2) @binding(12) var tex8: texture_2d<f32>;
@group(2) @binding(13) var tex9: texture_2d<f32>;
@group(2) @binding(14) var tex10: texture_2d<f32>;
@group(2) @binding(15) var tex11: texture_2d<f32>;
@group(2) @binding(16) var tex12: texture_2d<f32>;
@group(2) @binding(17) var tex13: texture_2d<f32>;
@group(2) @binding(18) var tex14: texture_2d<f32>;
@group(2) @binding(19) var tex15: texture_2d<f32>;
@group(2) @binding(20) var tex16: texture_2d<f32>;
@group(2) @binding(21) var tex17: texture_2d<f32>;
@group(2) @binding(22) var tex18: texture_2d<f32>;
@group(2) @binding(23) var tex19: texture_2d<f32>;
@group(2) @binding(24) var tex20: texture_2d<f32>;
@group(2) @binding(25) var tex21: texture_2d<f32>;
@group(2) @binding(26) var tex22: texture_2d<f32>;
@group(2) @binding(27) var tex23: texture_2d<f32>;
@group(2) @binding(28) var tex24: texture_2d<f32>;
@group(2) @binding(29) var tex25: texture_2d<f32>;
@group(2) @binding(30) var tex26: texture_2d<f32>;
@group(2) @binding(31) var tex27: texture_2d<f32>;
@group(2) @binding(32) var tex28: texture_2d<f32>;
@group(2) @binding(33) var tex29: texture_2d<f32>;
@group(2) @binding(34) var tex30: texture_2d<f32>;
@group(2) @binding(35) var tex31: texture_2d<f32>;
@group(2) @binding(36) var tex32: texture_2d<f32>;
@group(2) @binding(37) var tex33: texture_2d<f32>;
@group(2) @binding(38) var tex34: texture_2d<f32>;
@group(2) @binding(39) var tex35: texture_2d<f32>;
@group(2) @binding(40) var tex36: texture_2d<f32>;
@group(2) @binding(41) var tex37: texture_2d<f32>;
@group(2) @binding(42) var tex38: texture_2d<f32>;
@group(2) @binding(43) var tex39: texture_2d<f32>;
@group(2) @binding(8) var smp: sampler;
// 透射材质：场景色缓冲（WebGLRenderer renderTransmissionPass 的 RT 对应物）
// + Clamp 采样器（RenderTarget 纹理默认 ClampToEdge，与 Repeat 的视频槽区分）
// + DFG LUT（three.js DFGLUTData.js 32×32 RG16F 逐位拷贝，EnvironmentBRDF）。
@group(2) @binding(9) var transmission_tex: texture_2d<f32>;
@group(2) @binding(10) var smp_clamp: sampler;
@group(2) @binding(11) var dfg_lut: texture_2d<f32>;
// 场景环境 PMREM cubeUV（PMREMGenerator.fromEquirectangular 的 RT 对应物，
// HalfFloat RGBA、Linear/Clamp 采样——PMREM RT 同参数，PMREMGenerator.js
// :236-243）。无 env 时绑 1×1 占位、globals.env_cubuv.w=0（getIBL* 归零，
// `USE_ENVMAP` 未定义语义）。
@group(2) @binding(44) var env_cubuv_tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(5) world: vec3<f32>,
    // 马赛克逐三角形常量（GLSL flat varying 的 WGSL 对应物）。corner 曾
    // 按 js:253 `out vec3 vCornerColor` 的字面形态（无 flat）改 smooth
    // 插值——r13 f31 成对探针实测更差（hot 13536→23783，带内出现本侧
    // 特有的逐三角形波动），回滚 flat。负结果机制未完全归因：GLB 有
    // 720 个 COLOR_0 混合三角形（纯 R dominant=0 与纯 B dominant=2 顶点
    // 混排），smooth 插值应产生跨顶点渐变，但 oracle 带上实测为纯 KC
    // 色——说明参考侧还有未建模的环节（候选：GLSL vs 与 fs 的 flat
    // int vVideoId 参与顶点级查表的组合效应、或 GLTFLoader 顶点序/共享
    // 顶点的语义差）。本侧 flat 的整面纯色与 oracle 纯色带在形态上一致，
    // 残差是**带色 ±1 错位**（同带换色、边界 ±1px 对齐），机制仍开放。
    @location(2) @interpolate(flat) is_main: u32,
    @location(3) @interpolate(flat) video_slot: u32,
    @location(4) @interpolate(flat) corner: vec3<f32>,
};

// GLSL mod(x, y) = x - y*floor(x/y)（WGSL 的 % 是截断余弦，负数语义不同）。
fn glm_mod(x: vec4<f32>, y: f32) -> vec4<f32> {
    return x - y * floor(x / y);
}

fn permute(x: vec4<f32>) -> vec4<f32> {
    return glm_mod(((x * 34.0) + 1.0) * x, 289.0);
}

fn taylor_inv_sqrt(r: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(1.79284291400159) - 0.85373472095314 * r;
}

fn snoise(v: vec3<f32>) -> f32 {
    let C = vec3<f32>(1.0 / 6.0, 1.0 / 3.0, 0.0);
    let D = vec4<f32>(0.0, 0.5, 1.0, 2.0);
    var i = floor(v + vec3<f32>(dot(v, vec3<f32>(C.y))));
    let x0 = v - i + vec3<f32>(dot(i, vec3<f32>(C.x)));
    let g = step(x0.yzx, x0.xyz);
    let l = vec3<f32>(1.0) - g;
    let i1 = min(g.xyz, l.zxy);
    let i2 = max(g.xyz, l.zxy);
    let x1 = x0 - i1 + vec3<f32>(C.x);
    let x2 = x0 - i2 + vec3<f32>(C.y);
    let x3 = x0 - vec3<f32>(D.y);
    i = glm_mod3(i, 289.0);
    var p = permute(permute(permute(
        vec4<f32>(0.0, i1.z, i2.z, 1.0) + vec4<f32>(i.z))
        + vec4<f32>(0.0, i1.y, i2.y, 1.0) + vec4<f32>(i.y))
        + vec4<f32>(0.0, i1.x, i2.x, 1.0) + vec4<f32>(i.x));
    let n_ = 0.142857142857;
    let ns = n_ * D.wyz - D.xzx;
    let j = p - 49.0 * floor(p * ns.z * ns.z);
    let x_ = floor(j * ns.z);
    let y_ = floor(j - 7.0 * x_);
    let x = x_ * ns.x + ns.yyyy;
    let y = y_ * ns.x + ns.yyyy;
    let h = 1.0 - abs(x) - abs(y);
    let b0 = vec4<f32>(x.xy, y.xy);
    let b1 = vec4<f32>(x.zw, y.zw);
    let s0 = floor(b0) * 2.0 + 1.0;
    let s1 = floor(b1) * 2.0 + 1.0;
    let sh = -step(h, vec4<f32>(0.0));
    let a0 = b0.xzyw + s0.xzyw * sh.xxyy;
    let a1 = b1.xzyw + s1.xzyw * sh.zzww;
    var p0 = vec3<f32>(a0.xy, h.x);
    var p1 = vec3<f32>(a0.zw, h.y);
    var p2 = vec3<f32>(a1.xy, h.z);
    var p3 = vec3<f32>(a1.zw, h.w);
    let norm = taylor_inv_sqrt(vec4<f32>(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3)));
    p0 = p0 * norm.x;
    p1 = p1 * norm.y;
    p2 = p2 * norm.z;
    p3 = p3 * norm.w;
    var m = max(vec4<f32>(0.6) - vec4<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), vec4<f32>(0.0));
    m = m * m;
    return 42.0 * dot(m * m, vec4<f32>(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)));
}

fn glm_mod3(x: vec3<f32>, y: f32) -> vec3<f32> {
    return x - y * floor(x / y);
}

// 位移量：snoise(p*freq + t + wp*wfreq) * amp（js:185 getDisplacement 同式）。
fn displacement_amount(p: vec3<f32>, wp: vec3<f32>, amp: f32, freq: f32, wfreq: f32, t: f32) -> f32 {
    return snoise(p * freq + vec3<f32>(t) + wp * wfreq) * amp;
}

// 槽位 → 采样。textureSampleLevel(level 0)：马赛克槽位是 flat 逐三角形值，
// 非均匀控制流里 textureSample 过不了 uniformity 校验；视频纹理无 mipmap
// （Linear 过滤），level 0 与 textureSample 等价。
fn sample_slot(slot: u32, uv: vec2<f32>) -> vec4<f32> {
    if (slot == 0u) { return textureSampleLevel(tex0, smp, uv, 0.0); }
    if (slot == 1u) { return textureSampleLevel(tex1, smp, uv, 0.0); }
    if (slot == 2u) { return textureSampleLevel(tex2, smp, uv, 0.0); }
    if (slot == 3u) { return textureSampleLevel(tex3, smp, uv, 0.0); }
    if (slot == 4u) { return textureSampleLevel(tex4, smp, uv, 0.0); }
    if (slot == 5u) { return textureSampleLevel(tex5, smp, uv, 0.0); }
    if (slot == 6u) { return textureSampleLevel(tex6, smp, uv, 0.0); }
    if (slot == 7u) { return textureSampleLevel(tex7, smp, uv, 0.0); }
    if (slot == 8u) { return textureSampleLevel(tex8, smp, uv, 0.0); }
    if (slot == 9u) { return textureSampleLevel(tex9, smp, uv, 0.0); }
    if (slot == 10u) { return textureSampleLevel(tex10, smp, uv, 0.0); }
    if (slot == 11u) { return textureSampleLevel(tex11, smp, uv, 0.0); }
    if (slot == 12u) { return textureSampleLevel(tex12, smp, uv, 0.0); }
    if (slot == 13u) { return textureSampleLevel(tex13, smp, uv, 0.0); }
    if (slot == 14u) { return textureSampleLevel(tex14, smp, uv, 0.0); }
    if (slot == 15u) { return textureSampleLevel(tex15, smp, uv, 0.0); }
    if (slot == 16u) { return textureSampleLevel(tex16, smp, uv, 0.0); }
    if (slot == 17u) { return textureSampleLevel(tex17, smp, uv, 0.0); }
    if (slot == 18u) { return textureSampleLevel(tex18, smp, uv, 0.0); }
    if (slot == 19u) { return textureSampleLevel(tex19, smp, uv, 0.0); }
    if (slot == 20u) { return textureSampleLevel(tex20, smp, uv, 0.0); }
    if (slot == 21u) { return textureSampleLevel(tex21, smp, uv, 0.0); }
    if (slot == 22u) { return textureSampleLevel(tex22, smp, uv, 0.0); }
    if (slot == 23u) { return textureSampleLevel(tex23, smp, uv, 0.0); }
    if (slot == 24u) { return textureSampleLevel(tex24, smp, uv, 0.0); }
    if (slot == 25u) { return textureSampleLevel(tex25, smp, uv, 0.0); }
    if (slot == 26u) { return textureSampleLevel(tex26, smp, uv, 0.0); }
    if (slot == 27u) { return textureSampleLevel(tex27, smp, uv, 0.0); }
    if (slot == 28u) { return textureSampleLevel(tex28, smp, uv, 0.0); }
    if (slot == 29u) { return textureSampleLevel(tex29, smp, uv, 0.0); }
    if (slot == 30u) { return textureSampleLevel(tex30, smp, uv, 0.0); }
    if (slot == 31u) { return textureSampleLevel(tex31, smp, uv, 0.0); }
    if (slot == 32u) { return textureSampleLevel(tex32, smp, uv, 0.0); }
    if (slot == 33u) { return textureSampleLevel(tex33, smp, uv, 0.0); }
    if (slot == 34u) { return textureSampleLevel(tex34, smp, uv, 0.0); }
    if (slot == 35u) { return textureSampleLevel(tex35, smp, uv, 0.0); }
    if (slot == 36u) { return textureSampleLevel(tex36, smp, uv, 0.0); }
    if (slot == 37u) { return textureSampleLevel(tex37, smp, uv, 0.0); }
    if (slot == 38u) { return textureSampleLevel(tex38, smp, uv, 0.0); }
    if (slot == 39u) { return textureSampleLevel(tex39, smp, uv, 0.0); }
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

// COLOR_1 字节 id（0..8 截断）→ 纹理槽位（脚本声明表）。
fn face_map(byte_id: u32) -> u32 {
    let id = min(byte_id, 7u);
    let lo = object.face_to_texture[0];
    let hi = object.face_to_texture[1];
    if (id < 4u) { return lo[id]; }
    return hi[id - 4u];
}

fn corner_rule(slot: u32) -> vec2<u32> {
    let s = min(slot, 7u);
    let lo_a = object.corner_a[0];
    let hi_a = object.corner_a[1];
    let lo_i = object.corner_inv[0];
    let hi_i = object.corner_inv[1];
    if (s < 4u) { return vec2<u32>(lo_a[s], lo_i[s]); }
    return vec2<u32>(hi_a[s - 4u], hi_i[s - 4u]);
}

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color0: vec4<f32>,
    @location(4) color1: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    var p = pos;
    let n = normal;
    // 噪声顶点位移（js:185 getDisplacement 同式）：**只位移位置，法线保持
    // 原值**。依据 three.js chunk 顺序（meshphysical.glsl.js 顶点 main，
    // 本仓库 vendored 源逐行核对）：beginnormal_vertex(:33 objectNormal =
    // normal) → defaultnormal_vertex(:37 transformedNormal = objectNormal，
    // 经 normal_vertex(:38) 写入 vNormal) 先于 begin_vertex(:40) 执行。参考
    // onBeforeCompile 在 begin_vertex 替换里重赋 objectNormal（js:206）晚于
    // vNormal 消费——对片元法线是死代码。参考的有效语义 = 位移后的位置 +
    // 未位移法线（透射折射与高光都走光滑球法线，剪影仍波动）；js:190-205
    // 的切向双样本法线重建因此**不进**本管线（实装过、r8 对照证伪：带重建
    // 时 hero 折射影像被噪声法线搅成漩涡，与参考不符）。
    if (object.disp_params.x != 0.0) {
        let amp = object.disp_params.x;
        let freq = object.disp_params.y;
        let wfreq = object.disp_params.z;
        let t = object.disp_time.x * object.disp_params.w;
        let wp = (object.model * vec4<f32>(pos, 1.0)).xyz;
        let d0 = displacement_amount(pos, wp, amp, freq, wfreq, t);
        p = pos + normal * d0;
    }
    out.clip = globals.view_proj * (object.model * vec4<f32>(p, 1.0));
    out.normal = (object.normal_mat * vec4<f32>(n, 0.0)).xyz;
    // 世界坐标（worldpos_vertex：modelMatrix × 位移后顶点——透射折射与
    // viewDir 的输入）。
    out.world = (object.model * vec4<f32>(p, 1.0)).xyz;
    out.uv = uv;
    // 马赛克顶点段（js:286-292 逐行同构）：主面测试（COLOR_0 ≈ 0）、
    // COLOR_1 字节 id → 槽位、非主面 corner 色。
    let c0 = color0.rgb;
    out.is_main = select(0u, 1u, c0.r < 0.001 && c0.g < 0.001 && c0.b < 0.001);
    let slot = face_map(u32(color1.r * 255.0));
    out.video_slot = slot;
    // js:273 dominant = x>y ? (x>z ? 0 : 2) : (y>z ? 1 : 2)。
    // r14 实证（勿改回嵌套 select）：`select(select(2u,0u,x>z),
    // select(2u,1u,y>z), x>y)` 形态在此管线恒判 2——调试场三连拍钉死：
    // 同一 VS 里 c0.r=1.0/c0.b=0.0 直出为 (255,·,0) 而 dominant 打包
    // f32(dominant)/3.0 恒 170（=2），红/绿/蓝 c0 全部如此（oracle 侧
    // 同场 dom 有 0/1/2 分布）；换 if/else 同语义后 dom 正确随 c0 变化
    // （红→0 与 oracle 一致）。归因：WGSL→SPIR-V（naga，wgpu 30 同构建）
    // 对 select 实参嵌套 select 的代码生成（单层 select 如 is_main 行
    // 为正常）。全部 corner 色带因此 ±1 换色（红 c0 面渲染成蓝 dominant
    // 的查表色），是 r12-13「球内虚像 core」残差的构成之一。
    var dominant = 2u;
    if (c0.x > c0.y) {
        dominant = select(2u, 0u, c0.x > c0.z);
    } else if (c0.y > c0.z) {
        dominant = 1u;
    }
    let rule = corner_rule(slot);
    var ci = (dominant + rule.x) % 3u;
    ci = select(ci, 2u - ci, rule.y != 0u);
    out.corner = object.corner_colors[ci].rgb;
    return out;
}

// ══ PBR 透射核心（three.js r181 对齐 chunk 的 WGSL 同构移植）════════
// 来源（node_modules/three/src/renderers/shaders/ShaderChunk/）：
//   common.glsl.js（F_Schlick）、lights_physical_pars_fragment.glsl.js
//   （D_GGX/V_GGX_SmithCorrelated/BRDF_GGX/BRDF_GGX_Multiscatter/DFGApprox/
//   EnvironmentBRDF/RE_Direct_Physical）、transmission_pars_fragment.glsl.js
//   （getVolumeTransmissionRay/applyIorToRoughness/volumeAttenuation/
//   getIBLVolumeRefraction）、transmission_fragment.glsl.js、
//   lights_physical_fragment.glsl.js（材质初始化）、
//   meshphysical.glsl.js（totalDiffuse/totalSpecular 组装与清漆项）。
// 常量逐字（RECIPROCAL_PI 0.3183098861837907、EPSILON 1e-6、Schlick 系数
// -5.55473/-6.98316、色散 halfSpread (ior−1)×0.025×dispersion、粗糙度下限
// 0.0525）。已知偏差：① 透射采样链路已接通（r9 轮，原偏差解除）：RT
// 生成 mipmap（WebGLRenderer.js:1876 generateMipmaps）+ getTransmissionSample
// lod = log2(W)×roughness×clamp(ior×2−2,0,1) 的 mipped bicubic
// （transmission_pars_fragment.glsl.js:45-149 逐字移植，见下方
// get_transmission_sample）；lod 生效比例见 TRANSMISSION_LOD_SCALE 注释
// （偏差⑤：参考渲染实测有效 lod = 0）。② RT 无 4×MSAA、无 HalfFloat 的
// 偏差同步解除（r9 轮）：RT = Rgba16Float + 4×MSAA 渲染 resolve
// （WebGLRenderer.js:1875-1883 逐项）。③ 环境 IBL 已接通（⑥b）：CPU
// PMREM 预滤波（pmrem.rs）+ 本文件 cube_uv/getIBL* 函数；无 env 时
// getIBL* 归零，与 three.js `USE_ENVMAP` 未定义同值。

fn saturate1(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn f_schlick3(f0: vec3<f32>, f90: f32, dot_vh: f32) -> vec3<f32> {
    // common.glsl.js:100（Epic SIGGRAPH'13 优化式）。
    let fresnel = exp2((-5.55473 * dot_vh - 6.98316) * dot_vh);
    return f0 * (1.0 - fresnel) + vec3<f32>(f90 * fresnel);
}

fn f_schlick1(f0: f32, f90: f32, dot_vh: f32) -> f32 {
    let fresnel = exp2((-5.55473 * dot_vh - 6.98316) * dot_vh);
    return f0 * (1.0 - fresnel) + f90 * fresnel;
}

fn d_ggx(alpha: f32, dot_nh: f32) -> f32 {
    // lights_physical_pars_fragment.glsl.js:84。
    let a2 = alpha * alpha;
    let denom = dot_nh * dot_nh * (a2 - 1.0) + 1.0;
    return 0.3183098861837907 * a2 / (denom * denom);
}

fn v_ggx_smith_correlated(alpha: f32, dot_nl: f32, dot_nv: f32) -> f32 {
    // lights_physical_pars_fragment.glsl.js:70。
    let a2 = alpha * alpha;
    let gv = dot_nl * sqrt(a2 + (1.0 - a2) * dot_nv * dot_nv);
    let gl = dot_nv * sqrt(a2 + (1.0 - a2) * dot_nl * dot_nl);
    return 0.5 / max(gv + gl, 1e-6);
}

fn dfg_approx(dot_nv: f32, roughness: f32) -> vec2<f32> {
    // lights_physical_pars_fragment.glsl.js:379：uv=(roughness, dotNV)，
    // ClampToEdge + Linear（DataTexture flipY=false → 行 0 = v 0 端，上传
    // 行序即表行序，无需翻转）。
    return textureSampleLevel(dfg_lut, smp_clamp, vec2<f32>(roughness, dot_nv), 0.0).rg;
}

fn environment_brdf(n: vec3<f32>, v: vec3<f32>, specular_color: vec3<f32>, specular_f90: f32, roughness: f32) -> vec3<f32> {
    let fab = dfg_approx(saturate1(dot(n, v)), roughness);
    return specular_color * fab.x + vec3<f32>(specular_f90 * fab.y);
}

fn brdf_ggx(light_dir: vec3<f32>, view_dir: vec3<f32>, n: vec3<f32>, spec_color: vec3<f32>, spec_f90: f32, roughness: f32) -> vec3<f32> {
    // lights_physical_pars_fragment.glsl.js:150。
    let alpha = roughness * roughness;
    let half_dir = normalize(light_dir + view_dir);
    let dot_nl = saturate1(dot(n, light_dir));
    let dot_nv = saturate1(dot(n, view_dir));
    let dot_nh = saturate1(dot(n, half_dir));
    let dot_vh = saturate1(dot(view_dir, half_dir));
    let f = f_schlick3(spec_color, spec_f90, dot_vh);
    let v = v_ggx_smith_correlated(alpha, dot_nl, dot_nv);
    let d = d_ggx(alpha, dot_nh);
    return f * (v * d);
}

fn brdf_ggx_multiscatter(light_dir: vec3<f32>, view_dir: vec3<f32>, n: vec3<f32>, spec_color: vec3<f32>, spec_f90: f32, roughness: f32) -> vec3<f32> {
    // lights_physical_pars_fragment.glsl.js:432（直射 GGX + 多重散射补偿）。
    let single_scatter = brdf_ggx(light_dir, view_dir, n, spec_color, spec_f90, roughness);
    let dot_nl = saturate1(dot(n, light_dir));
    let dot_nv = saturate1(dot(n, view_dir));
    let dfg_v = dfg_approx(dot_nv, roughness);
    let dfg_l = dfg_approx(dot_nl, roughness);
    let fss_ess_v = spec_color * dfg_v.x + vec3<f32>(spec_f90 * dfg_v.y);
    let fss_ess_l = spec_color * dfg_l.x + vec3<f32>(spec_f90 * dfg_l.y);
    let ess_v = dfg_v.x + dfg_v.y;
    let ess_l = dfg_l.x + dfg_l.y;
    let ems_v = 1.0 - ess_v;
    let ems_l = 1.0 - ess_l;
    let favg = spec_color + (vec3<f32>(1.0) - spec_color) * 0.047619;
    let fms = fss_ess_v * fss_ess_l * favg
        / (vec3<f32>(1.0) - vec3<f32>(ems_v * ems_l) * favg * favg + vec3<f32>(1e-6));
    let compensation = ems_v * ems_l;
    return single_scatter + fms * vec3<f32>(compensation);
}

fn brdf_ggx_clearcoat(light_dir: vec3<f32>, view_dir: vec3<f32>, n: vec3<f32>, cc_roughness: f32) -> f32 {
    // lights_physical_pars_fragment.glsl.js:123（f0=0.04, f90=1）。
    let alpha = cc_roughness * cc_roughness;
    let half_dir = normalize(light_dir + view_dir);
    let dot_nl = saturate1(dot(n, light_dir));
    let dot_nv = saturate1(dot(n, view_dir));
    let dot_nh = saturate1(dot(n, half_dir));
    let dot_vh = saturate1(dot(view_dir, half_dir));
    let f = f_schlick1(0.04, 1.0, dot_vh);
    let v = v_ggx_smith_correlated(alpha, dot_nl, dot_nv);
    let d = d_ggx(alpha, dot_nh);
    return f * (v * d);
}

fn get_volume_transmission_ray(n: vec3<f32>, v: vec3<f32>, thickness: f32, ior: f32) -> vec3<f32> {
    // transmission_pars_fragment.glsl.js getVolumeTransmissionRay。
    let refraction_vector = refract(-v, normalize(n), 1.0 / ior);
    // 模型缩放（列长度；thickness 是局部空间量）。
    let model_scale = vec3<f32>(
        length(object.model[0].xyz),
        length(object.model[1].xyz),
        length(object.model[2].xyz),
    );
    return normalize(refraction_vector) * thickness * model_scale;
}

fn volume_attenuation(transmission_distance: f32, att_color: vec3<f32>, att_distance: f32) -> vec3<f32> {
    // transmission_pars_fragment.glsl.js volumeAttenuation（Beer 定律）。
    // JSON 无法表达 ±inf（serde_json 拒绝）：shim 对 Infinity 发 1e30 哨兵。
    if (att_distance >= 1.0e29) {
        return vec3<f32>(1.0, 1.0, 1.0);
    }
    let coefficient = -log(att_color) / att_distance;
    return exp(-coefficient * transmission_distance);
}

// ── Mipped bicubic 透射采样（transmission_pars_fragment.glsl.js:45-149）──
// 参考注释「Mipped Bicubic Texture Filtering by N8」的 B 样条权重；RT
// generateMipmaps + LinearMipmapLinearFilter（WebGLRenderer.js:1876-1877），
// getTransmissionSample lod = log2(transmissionSamplerSize.x) ×
// applyIorToRoughness（:145-149）。逐字移植（w0..w3/g0/g1/h0/h1/:45-84、
// bicubic/:85-111、textureBicubic/:113-127、applyIorToRoughness/:135-138、
// getTransmissionSample/:145-149）。
fn w0(a: f32) -> f32 {
    return (1.0 / 6.0) * (a * (a * (-a + 3.0) - 3.0) + 1.0);
}
fn w1(a: f32) -> f32 {
    return (1.0 / 6.0) * (a * a * (3.0 * a - 6.0) + 4.0);
}
fn w2(a: f32) -> f32 {
    return (1.0 / 6.0) * (a * (a * (-3.0 * a + 3.0) + 3.0) + 1.0);
}
fn w3(a: f32) -> f32 {
    return (1.0 / 6.0) * (a * a * a);
}
fn g0(a: f32) -> f32 {
    return w0(a) + w1(a);
}
fn g1(a: f32) -> f32 {
    return w2(a) + w3(a);
}
fn h0(a: f32) -> f32 {
    return -1.0 + w1(a) / (w0(a) + w1(a));
}
fn h1(a: f32) -> f32 {
    return 1.0 + w3(a) / (w2(a) + w3(a));
}

fn bicubic_transmission(coords: vec2<f32>, texel_size: vec4<f32>, lod: f32) -> vec4<f32> {
    // bicubic（:85-111）：uv 换 texel 坐标 → 2×2 B 样条核 → textureLod
    // 四角采样（texelSize = vec4(fLodSizeInv, fLodSize)）。
    let uv = coords * texel_size.zw + vec2<f32>(0.5);
    let iuv = floor(uv);
    let fuv = fract(uv);
    let g0x = g0(fuv.x);
    let g1x = g1(fuv.x);
    let h0x = h0(fuv.x);
    let h1x = h1(fuv.x);
    let h0y = h0(fuv.y);
    let h1y = h1(fuv.y);
    let p0 = (vec2<f32>(iuv.x + h0x, iuv.y + h0y) - vec2<f32>(0.5)) * texel_size.xy;
    let p1 = (vec2<f32>(iuv.x + h1x, iuv.y + h0y) - vec2<f32>(0.5)) * texel_size.xy;
    let p2 = (vec2<f32>(iuv.x + h0x, iuv.y + h1y) - vec2<f32>(0.5)) * texel_size.xy;
    let p3 = (vec2<f32>(iuv.x + h1x, iuv.y + h1y) - vec2<f32>(0.5)) * texel_size.xy;
    return g0(fuv.y) * (g0x * textureSampleLevel(transmission_tex, smp_clamp, p0, lod)
            + g1x * textureSampleLevel(transmission_tex, smp_clamp, p1, lod))
        + g1(fuv.y) * (g0x * textureSampleLevel(transmission_tex, smp_clamp, p2, lod)
            + g1x * textureSampleLevel(transmission_tex, smp_clamp, p3, lod));
}

fn texture_bicubic(coords: vec2<f32>, lod_in: f32) -> vec4<f32> {
    // textureBicubic（:113-127）：floor/ceil 两个 mip 级各做一次 bicubic，
    // fract(lod) 混合（GLSL textureSize → WGSL textureDimensions）。
    // 退化护栏：lod 封顶到顶层 mip（GL 对越界 textureSize 的硬件行为是
    // 返回 1×1/钳制；此处不可达除非 k=1 且宽为 2 的幂——integer lod 时
    // fract=0，cSample 权重为 0，钳到同级数值恒等）。
    let max_lod = floor(log2(max(globals.resolution.x, globals.resolution.y)));
    let lod = min(lod_in, max_lod);
    let lod_int = i32(floor(lod));
    let f_lod_size = vec2<f32>(textureDimensions(transmission_tex, lod_int));
    let c_lod_size = vec2<f32>(textureDimensions(transmission_tex, i32(ceil(lod))));
    let f_sample = bicubic_transmission(coords, vec4<f32>(1.0 / f_lod_size, f_lod_size), floor(lod));
    let c_sample = bicubic_transmission(coords, vec4<f32>(1.0 / c_lod_size, c_lod_size), ceil(lod));
    return mix(f_sample, c_sample, fract(lod));
}

// 透射 lod 公式与偏差⑤证据链见 get_transmission_sample 注释（r11 起采样
// 核 = lod0 双线性，lod 公式无生效面；TRANSMISSION_LOD_SCALE 常量随采样
// 核切换移除，翻回时按 r181 源码重建）。

fn get_transmission_sample(frag_coord: vec2<f32>, roughness: f32, ior: f32) -> vec4<f32> {
    // getTransmissionSample（:145-149）。transmissionSamplerSize = RT 尺寸
    // （WebGLMaterials.js:504 set(width,height)）；RT = 视口 ×
    // transmissionResolutionScale（默认 1.0，WebGLRenderer.js:282/:1912），
    // globals.resolution.x 即其宽度。
    //
    // lod 比例 = **0.0**（r9 轮实测，k3diff 单调：×1.0 +0.0195 / ×0.5
    // +0.0064 / ×0 最优）。r11 轮补充排除：采样核从 bicubic(lod0) 换成
    // 双线性 lod0 在 f5/f31 帧对拍不改善（PNG 域 p8 0.0627→0.0635 /
    // 0.0637→0.0645，同域公平对比）——偏差⑤的「参考 = lod0 双线性」
    // 结论**不**延伸到采样核：bicubic lod0 与双线性在参考残差尺度下无
    // 可辨差，棱柱边框错位另有根源（r11 轮定位：折射/姿态，见 handoff
    // round 11）。bicubic/mip 链/公式全链保留（翻回 r181 源码 =
    // texture_bicubic(coords, log2(W)×roughness×clamp(ior·2−2,0,1))）。
    let lod = 0.0;
    return texture_bicubic(frag_coord, lod);
}

fn project_to_refraction_coords(world_pos: vec3<f32>) -> vec2<f32> {
    // transmission_pars_fragment.glsl.js：ndc = proj × view × exit；
    // coords = ndc.xy/ndc.w × 0.5 + 0.5。wgpu 帧缓冲行序 y 向下而 NDC
    // y 向上 → v 翻转（GL 那份公式 v = +0.5·ndc.y + 0.5）。
    let ndc = globals.proj * (globals.view * vec4<f32>(world_pos, 1.0));
    return vec2<f32>(ndc.x / ndc.w * 0.5 + 0.5, 0.5 - ndc.y / ndc.w * 0.5);
}

fn get_ibl_volume_refraction(n: vec3<f32>, v: vec3<f32>, roughness: f32, diffuse: vec3<f32>, spec_color: vec3<f32>, spec_f90: f32, pos: vec3<f32>, dispersion: f32, ior: f32, thickness: f32, att_color: vec3<f32>, att_distance: f32) -> vec4<f32> {
    // transmission_pars_fragment.glsl.js getIBLVolumeRefraction。色散路径
    // 逐通道 unroll（GLSL for i 0..3 的等价展开）。
    var transmitted = vec3<f32>(0.0, 0.0, 0.0);
    var transmittance = vec3<f32>(0.0, 0.0, 0.0);
    var transmitted_a = 0.0;
    if (dispersion > 0.0) {
        let half_spread = (ior - 1.0) * 0.025 * dispersion;
        let iors = vec3<f32>(ior - half_spread, ior, ior + half_spread);
        // GLSL for i 0..3 的展开（逐通道 iors[i] 采样/衰减；WGSL 对 let 向量
        // 的动态下标不可靠，用 select 归约）。
        for (var i = 0u; i < 3u; i++) {
            let ior_i = select(iors.x, select(iors.y, iors.z, i == 2u), i == 1u);
            let ray = get_volume_transmission_ray(n, v, thickness, ior_i);
            let coords = project_to_refraction_coords(pos + ray);
            let s = get_transmission_sample(coords, roughness, ior_i);
            let attn = volume_attenuation(length(ray), att_color, att_distance);
            transmitted = vec3<f32>(
                select(transmitted.x, s.r, i == 0u),
                select(transmitted.y, s.g, i == 1u),
                select(transmitted.z, s.b, i == 2u));
            transmittance = vec3<f32>(
                select(transmittance.x, diffuse.x * attn.r, i == 0u),
                select(transmittance.y, diffuse.y * attn.g, i == 1u),
                select(transmittance.z, diffuse.z * attn.b, i == 2u));
            transmitted_a = transmitted_a + s.a;
        }
        transmitted_a = transmitted_a / 3.0;
    } else {
        let ray = get_volume_transmission_ray(n, v, thickness, ior);
        let coords = project_to_refraction_coords(pos + ray);
        let s = get_transmission_sample(coords, roughness, ior);
        transmitted = s.rgb;
        transmitted_a = s.a;
        transmittance = diffuse * volume_attenuation(length(ray), att_color, att_distance);
    }
    let attenuated_color = transmittance * transmitted;
    let f = environment_brdf(n, v, spec_color, spec_f90, roughness);
    let transmittance_factor = (transmittance.r + transmittance.g + transmittance.b) / 3.0;
    return vec4<f32>((vec3<f32>(1.0) - f) * attenuated_color, 1.0 - (1.0 - transmitted_a) * transmittance_factor);
}

// ══ 环境 IBL（⑥b）：scene.environment 的 cubeUV 采样与间接光注入 ══════
// 来源：cube_uv_reflection_fragment.glsl.js（PMREM cubeUV 布局采样，
// PMREMGenerator.js 注释「These defines must match with PMREMGenerator」）、
// envmap_physical_pars_fragment.glsl.js（getIBLIrradiance/getIBLRadiance）、
// lights_fragment_maps.glsl.js / lights_fragment_end.glsl.js（调用点）、
// lights_physical_pars_fragment.glsl.js:401-427/:546-584
// （computeMultiscattering / RE_IndirectSpecular_Physical）。
// 方差说明：GLSL 版在 view 空间 reflect 后 inverseTransformDirection 回
// 世界空间再采 cubeUV；本 WGSL 的 n/v 本就是世界空间，旋转与 reflect/
// normalize(mix()) 可交换 ⇒ 直接世界空间等价（净映射恒同）。

fn cube_uv_get_face(direction: vec3<f32>) -> f32 {
    // cube_uv_reflection_fragment.glsl.js getFace（PMREM 面序 0..5）。
    let abs_d = abs(direction);
    if (abs_d.x > abs_d.z) {
        if (abs_d.x > abs_d.y) {
            return select(3.0, 0.0, direction.x > 0.0);
        }
        return select(4.0, 1.0, direction.y > 0.0);
    }
    if (abs_d.z > abs_d.y) {
        return select(5.0, 2.0, direction.z > 0.0);
    }
    return select(4.0, 1.0, direction.y > 0.0);
}

fn cube_uv_get_uv(direction: vec3<f32>, face: f32) -> vec2<f32> {
    // getUV：RH 坐标系、PMREM 面内属性 uv（0..1）。
    var uv: vec2<f32>;
    if (face == 0.0) {
        uv = vec2<f32>(direction.z, direction.y) / abs(direction.x); // pos x
    } else if (face == 1.0) {
        uv = vec2<f32>(-direction.x, -direction.z) / abs(direction.y); // pos y
    } else if (face == 2.0) {
        uv = vec2<f32>(-direction.x, direction.y) / abs(direction.z); // pos z
    } else if (face == 3.0) {
        uv = vec2<f32>(-direction.z, direction.y) / abs(direction.x); // neg x
    } else if (face == 4.0) {
        uv = vec2<f32>(-direction.x, direction.z) / abs(direction.y); // neg y
    } else {
        uv = vec2<f32>(direction.x, direction.y) / abs(direction.z); // neg z
    }
    return 0.5 * (uv + 1.0);
}

fn cube_uv_bilinear(direction: vec3<f32>, mip_int_in: f32) -> vec3<f32> {
    // bilinearCubeUV：cubeUV_minMipLevel 4.0 / cubeUV_minTileSize 16.0；
    // #25071 过采样内缩（+1 与 faceSize−2）；mip < 4 的 extra LOD 落在
    // y = 4·(2^maxMip − faceSize) 行带、x 偏移 filterInt·3·16。
    let face = cube_uv_get_face(direction);
    let filter_int = max(4.0 - mip_int_in, 0.0);
    let mip_int = max(mip_int_in, 4.0);
    let face_size = exp2(mip_int);
    var uv = cube_uv_get_uv(direction, face) * (face_size - 2.0) + 1.0;
    var face_wrapped = face;
    if (face > 2.0) {
        uv.y += face_size;
        face_wrapped = face - 3.0;
    }
    uv.x += face_wrapped * face_size;
    uv.x += filter_int * 3.0 * 16.0;
    uv.y += 4.0 * (exp2(globals.env_cubuv.z) - face_size);
    uv.x *= globals.env_cubuv.x;
    uv.y *= globals.env_cubuv.y;
    // texture2D 线性采样（RT LinearFilter + ClampToEdge → smp_clamp lod 0）。
    return textureSampleLevel(env_cubuv_tex, smp_clamp, uv, 0.0).rgb;
}

fn cube_uv_roughness_to_mip(roughness: f32) -> f32 {
    // roughnessToMip：分段线性常量逐字（cubeUV_r0/m0..r6/m6）。
    if (roughness >= 0.8) {
        return (1.0 - roughness) * (-1.0 - (-2.0)) / (1.0 - 0.8) + (-2.0);
    } else if (roughness >= 0.4) {
        return (0.8 - roughness) * (2.0 - (-1.0)) / (0.8 - 0.4) + (-1.0);
    } else if (roughness >= 0.305) {
        return (0.4 - roughness) * (3.0 - 2.0) / (0.4 - 0.305) + 2.0;
    } else if (roughness >= 0.21) {
        return (0.305 - roughness) * (4.0 - 3.0) / (0.305 - 0.21) + 3.0;
    }
    return -2.0 * log2(1.16 * roughness); // 1.16 = 1.79^0.25
}

fn texture_cube_uv(sample_dir: vec3<f32>, roughness: f32) -> vec4<f32> {
    // textureCubeUV：mip = clamp(roughnessToMip, m0, CUBEUV_MAX_MIP)；
    // mipF == 0 单采样，否则相邻双 mip 双线性后 mix（trilinear 等价）。
    let mip = clamp(cube_uv_roughness_to_mip(roughness), -2.0, globals.env_cubuv.z);
    let mip_f = fract(mip);
    let mip_int = floor(mip);
    let color0 = cube_uv_bilinear(sample_dir, mip_int);
    if (mip_f == 0.0) {
        return vec4<f32>(color0, 1.0);
    }
    let color1 = cube_uv_bilinear(sample_dir, mip_int + 1.0);
    return vec4<f32>(mix(color0, color1, mip_f), 1.0);
}

fn get_ibl_irradiance(n: vec3<f32>) -> vec3<f32> {
    // envmap_physical_pars_fragment.glsl.js getIBLIrradiance：
    // PI·textureCubeUV(env, envMapRotation·worldNormal, 1.0).rgb·envMapIntensity。
    // environmentRotation 参考未声明（three.js 默认单位阵）→ 恒等；
    // 粗糙度 1.0 → mip 段 r0（漫反射级）。
    if (globals.env_cubuv.w < 0.5) {
        return vec3<f32>(0.0);
    }
    return vec3<f32>(3.141592653589793) * texture_cube_uv(n, 1.0).rgb * object.phys_d.x;
}

fn get_ibl_radiance(v: vec3<f32>, n: vec3<f32>, roughness: f32) -> vec3<f32> {
    // getIBLRadiance：reflect(−viewDir, normal) 后按 pow4(roughness) 向
    // 法线收拢（粗面不采切平面外的光），再 textureCubeUV·envMapIntensity。
    if (globals.env_cubuv.w < 0.5) {
        return vec3<f32>(0.0);
    }
    let reflect_vec = reflect(-v, n);
    let rr = roughness * roughness;
    let mixed = normalize(mix(reflect_vec, n, rr * rr)); // pow4(roughness)
    return texture_cube_uv(mixed, roughness).rgb * object.phys_d.x;
}

fn fs_physical(
    n_world: vec3<f32>,
    world_pos: vec3<f32>,
    geometry_roughness: f32,
    uv: vec2<f32>,
) -> vec4<f32> {
    // 法线贴图（three.js `normal_fragment_maps` 的切线空间展开）。切线由
    // 屏幕导数反推（cotangent frame）——glTF 未提供 TANGENT 时 three.js
    // 走的也是这一支，所以不依赖几何属性。
    // 导数（dpdx/dpdy）必须在一致控制流里求值——采样结果在分支外算出，
    // 用 select 遮蔽，而不是把 dpdx 放进 if。
    let map_n = select(
        vec3<f32>(0.0, 0.0, 1.0),
        sample_slot(object.normal_tex.x, uv).xyz * 2.0 - 1.0,
        object.normal_tex.x != 0xFFFFFFFFu,
    );
    let n0 = normalize(n_world);
    let dp1 = dpdx(world_pos);
    let dp2 = dpdy(world_pos);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    let dp2perp = cross(dp2, n0);
    let dp1perp = cross(n0, dp1);
    let t = dp2perp * duv1.x + dp1perp * duv2.x;
    let bt = dp2perp * duv1.y + dp1perp * duv2.y;
    // 无 normalMap 时 r147 不建切线架（normal_fragment_maps 整段在
    // `#ifdef USE_NORMALMAP` 内）。这里必须同样短路：退化 UV（常 UV 的
    // 网格 → duv=0）会让 dot(t,t)=dot(bt,bt)=0，inverseSqrt(0)=Inf，
    // 0*Inf=NaN 顺着 mat3×map_n 污染 n → 片元 alpha=0 → 该区透出背景层
    // （GLB 的 PaletteMaterial003/004 网格就是常 UV）。
    let has_normal_map = object.normal_tex.x != 0xFFFFFFFFu;
    let invmax = inverseSqrt(max(max(dot(t, t), dot(bt, bt)), 1e-12));
    let n_in = normalize(select(
        n0,
        mat3x3<f32>(t * invmax, bt * invmax, n0) * map_n,
        has_normal_map,
    ));
    // 材质初始化（lights_physical_fragment.glsl.js:4-58；js:163-179 常量
    // 经 uniform 进来）。roughness 下限 0.0525 + 屏幕空间法线梯度。
    // glTF PBR 贴图（meshStandardMaterial）：baseColor 乘进 diffuse，
    // metallicRoughness 的 G→roughness、B→metalness（纹理缺失时 1.0 无影响）。
    var base = object.base_color.rgb;
    var base_alpha = object.base_color.a;
    if (object.pbr_tex.x != 0xFFFFFFFFu) {
        // r147 map_fragment.glsl.js：`diffuseColor *= sampledDiffuseColor`
        // 是 vec4 乘，alpha 通道必须一起进——GLB 的 Palette 网格靠贴图
        // alpha 才出苹果 logo 的镂空/暗区，丢掉 alpha 会整块铺成灰面。
        let s = sample_slot(object.pbr_tex.x, uv);
        base = base * s.rgb;
        base_alpha = base_alpha * s.a;
    }
    var roughness_factor = object.phys_b.y;
    var metalness = object.phys_b.z;
    if (object.pbr_tex.y != 0xFFFFFFFFu) {
        let mr = sample_slot(object.pbr_tex.y, uv);
        roughness_factor = roughness_factor * mr.g;
        metalness = metalness * mr.b;
    }
    let diffuse = base * (1.0 - metalness);
    let roughness = min(max(roughness_factor, 0.0525) + geometry_roughness, 1.0);
    let spec_f90 = mix(object.phys_b.w, 1.0, metalness);
    let ior = object.phys_a.x;
    // specularColor = mix(min(((ior−1)/(ior+1))², 1)·specularIntensity,
    // diffuse, metalness)。
    let f0 = pow2_local((ior - 1.0) / (ior + 1.0));
    // specularColor 的金属项是**原始** diffuseColor（albedo），不是乘过
    // (1−metalness) 的 diffuse——用 diffuse 会让 metalness=1 的金属全黑。
    let spec_color = mix(min(vec3<f32>(f0), vec3<f32>(1.0)) * object.phys_b.w, base, metalness);
    let clearcoat = object.phys_c.x;
    let cc_roughness = min(max(object.phys_c.y, 0.0525) + geometry_roughness, 1.0);
    let v = normalize(globals.camera_pos.xyz - world_pos);
    let n = normalize(n_in);
    // 灯光循环（RE_Direct_Physical / RE_IndirectDiffuse_Physical）。方向光
    // 传播方向取反 = three.js directLight.direction（指向光源）。
    var direct_diffuse = vec3<f32>(0.0, 0.0, 0.0);
    var direct_specular = vec3<f32>(0.0, 0.0, 0.0);
    var clearcoat_specular = vec3<f32>(0.0, 0.0, 0.0);
    let l1 = -globals.light_dir.xyz;
    let dot_nl1 = saturate1(dot(n, l1));
    let irr1 = dot_nl1 * globals.light_color.rgb;
    direct_specular += irr1 * brdf_ggx_multiscatter(l1, v, n, spec_color, spec_f90, roughness);
    direct_diffuse += irr1 * (diffuse * 0.3183098861837907);
    clearcoat_specular += irr1 * brdf_ggx_clearcoat(l1, v, n, cc_roughness);
    let l2 = -globals.light_dir2.xyz;
    let dot_nl2 = saturate1(dot(n, l2));
    let irr2 = dot_nl2 * globals.light_color2.rgb;
    direct_specular += irr2 * brdf_ggx_multiscatter(l2, v, n, spec_color, spec_f90, roughness);
    direct_diffuse += irr2 * (diffuse * 0.3183098861837907);
    clearcoat_specular += irr2 * brdf_ggx_clearcoat(l2, v, n, cc_roughness);
    // 点光（RE_Direct_Physical 的 punctual 分支）：方向 = 位置 − 着色点，
    // 辐照度 × 距离衰减。
    for (var pi = 0u; pi < 2u; pi += 1u) {
        let to_light = globals.point_pos[pi].xyz - world_pos;
        let ld = length(to_light);
        if (ld > 0.0001) {
            let lp = to_light / ld;
            let irr = saturate1(dot(n, lp))
                * globals.point_color[pi].rgb
                * point_attenuation(ld, globals.point_color[pi].w, globals.point_pos[pi].w);
            direct_specular += irr * brdf_ggx_multiscatter(lp, v, n, spec_color, spec_f90, roughness);
            direct_diffuse += irr * (diffuse * 0.3183098861837907);
            clearcoat_specular += irr * brdf_ggx_clearcoat(lp, v, n, cc_roughness);
        }
    }
    // Hemisphere + Ambient → indirectDiffuse（irradiance × BRDF_Lambert）。
    let hemi = mix(globals.hemi_ground.rgb, globals.hemi_sky.rgb, n.y * 0.5 + 0.5);
    var indirect_diffuse = (globals.ambient.rgb + hemi) * (diffuse * 0.3183098861837907);
    // IBL 项（lights_fragment_maps.glsl.js：geometryNormal/viewDir 皆世界
    // 空间等价；USE_CLEARCOAT 时 clearcoatRadiance 用 clearcoatRoughness，
    // 清漆法线无贴图 ⇒ = 几何法线）。
    let ibl_irradiance = get_ibl_irradiance(n);
    let ibl_radiance = get_ibl_radiance(v, n, roughness);
    let cc_radiance = get_ibl_radiance(v, n, cc_roughness);
    // lights_fragment_end → RE_IndirectSpecular_Physical
    // （lights_physical_pars_fragment.glsl.js:546-584）。USE_CLEARCOAT 分支
    // （:548-552）：clearcoatSpecularIndirect 累计。
    let cc_specular_indirect = cc_radiance
        * environment_brdf(n, v, vec3<f32>(0.04), 1.0, cc_roughness);
    // :562-584。computeMultiscattering（:401-427，无 iridescence 支路）：
    // FssEss = Fr·fab.x + f90·fab.y；Ems = 1−Ess；Favg = Fr + (1−Fr)/21；
    // Fms = FssEss·Favg / (1 − Ems·Favg)。
    var single_scattering = vec3<f32>(0.0, 0.0, 0.0);
    var multi_scattering = vec3<f32>(0.0, 0.0, 0.0);
    let fab_ms = dfg_approx(saturate1(dot(n, v)), roughness);
    let fss_ess = spec_color * fab_ms.x + vec3<f32>(spec_f90 * fab_ms.y);
    let ess = fab_ms.x + fab_ms.y;
    let ems = 1.0 - ess;
    let favg = spec_color + (vec3<f32>(1.0) - spec_color) * 0.047619; // 1/21
    let fms = fss_ess * favg / (vec3<f32>(1.0) - vec3<f32>(ems) * favg);
    single_scattering = fss_ess;
    multi_scattering = fms * vec3<f32>(ems);
    // :576-582。cosineWeightedIrradiance = iblIrradiance·RECIPROCAL_PI。
    let cosine_weighted_irradiance = ibl_irradiance * 0.3183098861837907;
    let total_scattering = single_scattering + multi_scattering;
    let diffuse_ms = diffuse
        * (1.0 - max(max(total_scattering.r, total_scattering.g), total_scattering.b));
    let indirect_specular = ibl_radiance * single_scattering
        + multi_scattering * cosine_weighted_irradiance;
    indirect_diffuse += diffuse_ms * cosine_weighted_irradiance;
    // meshphysical.glsl.js totalDiffuse/totalSpecular 求和。
    var total_diffuse = direct_diffuse + indirect_diffuse;
    let total_specular = direct_specular + indirect_specular;
    // transmission_fragment.glsl.js。
    let transmitted = get_ibl_volume_refraction(
        n, v, roughness, diffuse, spec_color, spec_f90, world_pos,
        object.phys_a.w, ior, object.phys_a.y,
        object.attenuation_color.rgb, object.phys_a.z);
    let transmission_alpha = mix(1.0, transmitted.a, object.phys_b.x);
    total_diffuse = mix(total_diffuse, transmitted.rgb, object.phys_b.x);
    // outgoing（meshphysical.glsl.js）+ 清漆（USE_CLEARCOAT 分支）：
    // outgoingLight = outgoingLight·(1 − clearcoat·Fcc)
    //              + (clearcoatSpecularDirect + clearcoatSpecularIndirect)·clearcoat。
    var outgoing = total_diffuse + total_specular;
    let dot_nv_cc = saturate1(dot(n, v));
    let f_cc = f_schlick1(0.04, 1.0, dot_nv_cc);
    outgoing = outgoing * (1.0 - clearcoat * f_cc)
        + (clearcoat_specular + cc_specular_indirect) * clearcoat;
    // opaque_fragment.glsl.js。
    return vec4<f32>(outgoing, base_alpha * transmission_alpha);
}


// 点光距离衰减（three.js r181 lights_pars_begin.glsl.js 的
// getDistanceAttenuation）。
// cutoff = 光照半径（0 = 不衰减），decay = 衰减指数。
fn point_attenuation(light_distance: f32, cutoff: f32, decay: f32) -> f32 {
    // three.js r181 lights_pars_begin.glsl.js:56-73。decayExponent > 0 恒成立
    // （PointLight 缺省 decay = 2，脚本传入即原值），故不走 r147 的“非物理”
    // 窗函数分支——那是 physicallyCorrectLights=false 时代的旧写法。
    var distance_falloff = 1.0 / max(pow(light_distance, decay), 0.01);
    if (cutoff > 0.0) {
        let ratio = light_distance / cutoff;
        distance_falloff = distance_falloff * pow2_local(saturate1(1.0 - pow2_local(ratio) * pow2_local(ratio)));
    }
    return distance_falloff;
}

fn pow2_local(x: f32) -> f32 {
    return x * x;
}

// ══ 输出变换（three.js r181 tonemapping/colorspace chunk 的 WGSL 同构移植；
//    参考渲染器设置 js:18-20 经 globals.output_xform 传入）════════
// three.js 链序（meshphysical.glsl.js 片元尾部）：opaque_fragment →
// tonemapping_fragment（gl_FragColor.rgb = toneMapping(...)，仅 rgb）→
// colorspace_fragment（gl_FragColor = linearToOutputTexel(...)，rgb 变换、
// alpha 原样）。仅内建材质分支调用；透射 RT pass 的 globals 全 0 → 恒等
// （WebGLRenderer.js:1921-1922/:2197），与 three.js RT 线性语义一致。

fn rrt_and_odt_fit(v: vec3<f32>) -> vec3<f32> {
    // tonemapping_pars_fragment.glsl.js RRTAndODTFit。
    let a = v * (v + vec3<f32>(0.0245786)) - vec3<f32>(0.000090537);
    let b = v * (vec3<f32>(0.983729) * v + vec3<f32>(0.4329510)) + vec3<f32>(0.238081);
    return a / b;
}

fn aces_filmic(color_in: vec3<f32>, exposure: f32) -> vec3<f32> {
    // tonemapping_pars_fragment.glsl.js ACESFilmicToneMapping。mat3 列构造
    // 直译（GLSL mat3(a,b,c) 的 a/b/c 是列；three.js 源注释
    // 「transposed from source」即此列序）。
    let aces_input = mat3x3<f32>(
        vec3<f32>(0.59719, 0.07600, 0.02840),
        vec3<f32>(0.35458, 0.90834, 0.13383),
        vec3<f32>(0.04823, 0.01566, 0.83777));
    let aces_output = mat3x3<f32>(
        vec3<f32>(1.60475, -0.10208, -0.00327),
        vec3<f32>(-0.53108, 1.10813, -0.07276),
        vec3<f32>(-0.07367, -0.00605, 1.07602));
    var color = color_in * (exposure / 0.6);
    color = aces_input * color;
    color = rrt_and_odt_fit(color);
    color = aces_output * color;
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn srgb_oetf(v: vec3<f32>) -> vec3<f32> {
    // colorspace_pars_fragment.glsl.js sRGBTransferOETF（GLSL mix 的逐分量
    // bool 选择 = WGSL select 展开；select(f, t, cond) 的 cond=真 取**第二**
    // 参——v ≤ 0.0031308 的小值支路是 12.92·v，否则 1.055·v^0.41666−0.055；
    // three.js 字面量 0.41666/1.055/0.055/12.92/0.0031308 逐字保留）。
    let hi = pow(v, vec3<f32>(0.41666)) * vec3<f32>(1.055) - vec3<f32>(0.055);
    let lo = v * vec3<f32>(12.92);
    return vec3<f32>(
        select(hi.r, lo.r, v.r <= 0.0031308),
        select(hi.g, lo.g, v.g <= 0.0031308),
        select(hi.b, lo.b, v.b <= 0.0031308));
}

fn output_xform(c: vec4<f32>) -> vec4<f32> {
    var rgb = c.rgb;
    if (globals.output_xform.x > 0.5) {
        rgb = aces_filmic(rgb, globals.output_xform.y);
    }
    if (globals.output_xform.z > 0.5) {
        rgb = srgb_oetf(rgb);
    }
    return vec4<f32>(rgb, c.a);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // 几何粗糙度（lights_physical_fragment.glsl.js:6-7）：屏幕空间法线
    // 梯度。导数只能在 uniform 控制流取——放在全部分支之前。
    let dpn = max(abs(dpdx(in.normal)), abs(dpdy(in.normal)));
    let geometry_roughness = max(max(dpn.x, dpn.y), dpn.z);
    if (object.flags.x == 3u) {
        // 内建材质（MeshPhysicalMaterial）：输出链 = opaque_fragment →
        // tonemapping_fragment → colorspace_fragment（上方 output_xform 注释）。
        return output_xform(fs_physical(in.normal, in.world, geometry_roughness, in.uv));
    }
    if (object.flags.x == 2u) {
        // 视频-马赛克面片（js:309-329 逐行同构）。**不走 output_xform**：
        // 本分支是参考 prismMaterial（ShaderMaterial, toneMapped:false,
        // js:248-266）的通用化——three.js 不向 ShaderMaterial 注入
        // tonemapping/colorspace chunk，raw 输出。
        let d = distance(in.uv, vec2<f32>(0.5, 0.5));
        let frost = d * d * object.mosaic.x;
        // js:314 surfaceUv。GL gl_FragCoord 原点在左下，wgpu 的
        // @builtin(position) 原点在左上 → frag.y 翻转得 GL 同构 y-up 坐标；
        // flipY=true（js:239）在上传端翻行序实现（v=0=图底，见上方纹理
        // 上传循环），此处采样与 GL 完全同构；越界由 Repeat 寻址回卷
        // （js:237-238）。（内置 position 经顶点输出结构传递，fragment 不另取参。）
        let frag = vec2<f32>(in.clip.x / globals.resolution.x, 1.0 - in.clip.y / globals.resolution.y);
        let su = (frag - vec2<f32>(0.5, 0.5)) / object.mosaic.y + vec2<f32>(0.5, 0.5);
        let albedo = sample_slot(in.video_slot, su);
        let final_color = select(in.corner, albedo.rgb + vec3<f32>(frost, frost, frost), in.is_main == 1u);
        return vec4<f32>(final_color, 1.0);
    }
    if (object.flags.x == 0u) {
        // Unlit（MeshBasicMaterial 语义，toneMapped 内建 → 走输出变换）。
        // map_fragment（three.js map_fragment.glsl.js：diffuseColor *=
        // texel，vec4 含 alpha）× base color；alpha 进透明混合路径（r7
        // wordmark 贴图面依赖）。
        if (object.tex.x != 0xFFFFFFFFu) {
            let albedo = sample_slot(object.tex.x, in.uv);
            return output_xform(vec4<f32>(object.base_color.rgb * albedo.rgb, object.base_color.a * albedo.a));
        }
        return output_xform(object.base_color);
    }
    let n = normalize(in.normal);
    // HemisphereLight 辐照度（three.js getHemisphereLightIrradiance，
    // 世界法线 y 插值——本渲染器相机无滚转，与视空间 up 等价）。
    let hemi = mix(globals.hemi_ground.rgb, globals.hemi_sky.rgb, n.y * 0.5 + 0.5);
    let lambert1 = max(dot(n, -globals.light_dir.xyz), 0.0);
    let lambert2 = max(dot(n, -globals.light_dir2.xyz), 0.0);
    let rgb = object.base_color.rgb
        * (globals.ambient.rgb + hemi
            + globals.light_color.rgb * lambert1
            + globals.light_color2.rgb * lambert2);
    // Lambert（MeshLambertMaterial，toneMapped 内建 → 走输出变换）。
    return output_xform(vec4<f32>(rgb, object.base_color.a));
}
"#;

/// 3D 渲染器。构造一次，逐帧 [`Renderer3d::render`]。
/// 透射场景色缓冲 + mip 链（`_transmissionRenderTarget` 对应物）。
struct TransmissionRt {
    width: u32,
    height: u32,
    mip_count: u32,
    /// 解析后（resolve）的 mip 链纹理， HalfFloat（WebGLRenderer.js:1878
    /// `type: HalfFloatType`，WebGL2 EXT_color_buffer_float 可用时——参考
    /// 渲染当天实测走此分支）。
    texture: wgpu::Texture,
    /// 4×MSAA 渲染目标（WebGLRenderer.js:1880 `samples: 4`）；pass 里
    /// resolve 进 `texture` 的 level 0。
    msaa_texture: wgpu::Texture,
    /// 4×MSAA 深度（RT pass 专用；主 pass 深度同为 4× 但句柄分立
    /// —— three.js RT 深度是 renderTarget 自己的 depthTexture）。
    msaa_depth: wgpu::Texture,
    /// MSAA 色附件视图（pass_a 的 view；resolve_target = base_view）。
    msaa_view: wgpu::TextureView,
    /// level 0 视图（resolve 目标；wgpu 附件视图必须恰好 1 个 mip）。
    base_view: wgpu::TextureView,
    /// 全 mip 视图（主 pass binding 9 采样，get_transmission_sample 用）。
    sample_view: wgpu::TextureView,
}

pub struct Renderer3d {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    /// 透射/透明列表的 src-alpha 混合变体（同一 WGSL，仅目标混合态不同）。
    pipeline_blended: wgpu::RenderPipeline,
    /// 透射 RT pass 管线（4×MSAA + Rgba16Float 附件变体；附件采样数 =
    /// 管线 multisample.count 是 wgpu 验证约束。主 pass 管线同为 4×
    /// 但目标是 Rgba8Unorm，格式不同不能复用）。
    pipeline_rt: wgpu::RenderPipeline,
    pipeline_blended_rt: wgpu::RenderPipeline,
    object_bg_layout: wgpu::BindGroupLayout,
    texture_bg_layout: wgpu::BindGroupLayout,
    /// 脚本材质 `map` 采样布局（group 3：binding 0 贴图 + binding 1
    /// Clamp 采样器；仅带 map 的 ShaderMaterial 程序进管线布局）。
    shader_map_bgl: wgpu::BindGroupLayout,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    /// 透射 RT pass 专用 globals（内容同 globals_buf，仅 output_xform 全 0
    /// —— three.js RT 二遍 NoToneMapping + LinearSRGB 输出，WebGLRenderer.js
    /// :1921-1922/:2197。独立 buffer 免去两 pass 间的缓冲重写排序问题）。
    globals_buf_rt: wgpu::Buffer,
    globals_bg_rt: wgpu::BindGroup,
    /// 纹理槽空载占位（1×1 白）：bind group 必须完整，槽未用时绑它。
    dummy_texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Clamp 寻址采样器：透射场景色缓冲与 DFG LUT 用（RenderTarget 纹理
    /// 默认 ClampToEdge 语义；与视频槽的 Repeat 区分）。
    clamp_sampler: wgpu::Sampler,
    /// DFG LUT 纹理视图（32×32 Rg16F，three.js DFGLUTData.js 逐位拷贝）。
    dfg_view: wgpu::TextureView,
    /// 场景环境 PMREM cubeUV 纹理视图（[`Renderer3d::set_environment`]
    /// 生成；无 env 时绑 dummy 占位、globals.env_cubuv.w = 0）。
    env_view: wgpu::TextureView,
    /// env_cubuv uniform 参数（texelWidth, texelHeight, maxMip, has_env）。
    env_cubuv: [f32; 4],
    /// PMREM 结果缓存键 = `Arc::as_ptr(&bitmap.rgba)`：env 位图来自
    /// MediaContext 图像缓存的 Arc，逐帧同指针 ⇒ 预滤波只算一次
    ///（three.js PMREMGenerator 亦为 env 载入期一次性开销）。
    env_cache_key: usize,
    /// 缓存的深度缓冲（4×MSAA，MAIN_SAMPLE_COUNT；尺寸变化才重建）。
    depth: Option<(u32, u32, wgpu::TextureView)>,
    /// 主 pass 4×MSAA 颜色附件（antialias:true 的 drawing buffer 对应物；
    /// pass 里 resolve 进 1× 读回纹理，尺寸变化才重建）。
    msaa_color: Option<(u32, u32, wgpu::TextureView)>,
    /// 缓存的透射场景色缓冲（WebGLRenderer renderTransmissionPass 的
    /// `_transmissionRenderTarget` 对应物；尺寸变化才重建）。RT 带 mip 链
    ///（generateMipmaps，WebGLRenderer.js:1876），pass 附件用 base 视图
    ///（level 0，wgpu 附件视图必须恰好 1 个 mip）、binding 9 用全 mip
    /// sample 视图。
    transmission: Option<TransmissionRt>,
    /// mip 生成管线（WebGLTextures.updateRenderTargetMipmap 的
    /// glGenerateMipmap 对应物；wgpu 无内建，逐级线性降采样 blit）。
    mipgen_bgl: wgpu::BindGroupLayout,
    mipgen_pipeline: wgpu::RenderPipeline,
    /// 脚本着色器程序缓存：((vertex, fragment) 源) → 程序。按源字符串
    /// 精确匹配（GLSL 确定性编译 ⇒ 同源同程序）。
    shader_programs: Vec<((String, String), ShaderProgram)>,
    /// Linux 离线路径（[`Self::new`]）持有的 EGL 4.6 离屏上下文。
    ///
    /// external 模式下 wgpu 不管理 currency（`AdapterContext.egl = None`），
    /// 全靠这个上下文在建渲染器时 make current、此后一直不动。必须比所有
    /// wgpu 资源活得久——放最后一个字段（Rust 按声明序析构）。
    #[cfg(target_os = "linux")]
    offscreen_egl: Option<opencat_gpu_native::egl::EglContext>,
}

/// 平台首选图形后端。
///
/// Windows 上 NVIDIA 驱动的 Vulkan 路径（`nvoglv64.dll`）在 3D 场景 teardown 时
/// ACCESS_VIOLATION——`cube` 示例输出正确后仍崩、`opencat render` 的 mp4 收尾时
/// 崩（moov 缺失的坏文件）。GL 与 DX12 后端同帧逐位一致（maxdiff 1/255）且稳定，
/// 故除 macOS 外一律走 GL。macOS 原生走 Metal。
///
/// 这是**默认值**而非硬绑定：`WGPU_BACKEND` 仍可覆盖（`with_env()` 优先级更高），
/// 保留 lavapipe 软渲染等兑底路径。
fn preferred_backends() -> wgpu::Backends {
    #[cfg(target_os = "macos")]
    {
        wgpu::Backends::METAL
    }
    #[cfg(not(target_os = "macos"))]
    {
        wgpu::Backends::GL
    }
}

impl Renderer3d {
    /// 自建隐藏窗口 + 独立 GL 上下文（离线渲染路径）。
    ///
    /// Linux 上自建的是 **EGL 4.6 core 离屏上下文**（pbuffer，见
    /// [`Self::new_offscreen_egl`]）而非 wgpu 的 GL 3.3——后者实测画不出
    /// 图元；与预览的宿主上下文同一条外部适配器代码。`WGPU_BACKEND` 显式
    /// 指向非 GL 后端（vulkan 等）时保持 wgpu 自建路径。EGL 不可用（无
    /// 显示等）才回退 wgpu 自建，回退非静默。
    pub async fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            match std::env::var("WGPU_BACKEND").as_deref() {
                Ok("vulkan") | Ok("metal") | Ok("dx12") | Ok("dx11") | Ok("webgpu") => {}
                _ => match Self::new_offscreen_egl() {
                    Ok(r) => return Ok(r),
                    Err(e) => {
                        eprintln!(
                            "[opencat-three] EGL 4.6 离屏上下文不可用（{e:#}），回退 wgpu 自建 GL 上下文"
                        );
                    }
                },
            }
        }
        let (device, queue) = Self::request_device().await?;
        Self::from_device(device, queue)
    }

    /// Linux 离线专用：自建 EGL 4.6 core 离屏上下文 + 外部适配器。
    ///
    /// 上下文由返回的渲染器持有（[`Self::offscreen_egl`]），构造即 make
    /// current 且此后不动——external 模式的 wgpu 调用、Skia 合成、资源析构
    /// 都默认 currency 已在。**渲染必须全程在构造线程上**。
    #[cfg(target_os = "linux")]
    fn new_offscreen_egl() -> Result<Self> {
        let ctx = opencat_gpu_native::egl::EglContext::offscreen()?;
        let mut renderer = Self::new_external(
            |name| opencat_gpu_native::egl::load_proc(name),
            || ctx.make_current(),
        )?;
        renderer.offscreen_egl = Some(ctx);
        Ok(renderer)
    }

    /// 用**宿主自己的** GL 上下文建渲染器（预览器路径）。
    ///
    /// 与 [`Self::new`] 的区别：wgpu 不再自建隐藏窗口上下文，而是加入调用方的
    /// GL 上下文。这样两者共享同一 `HGLRC`，wgpu 渲染出的纹理名在调用方的
    /// Skia 里可直接采样——零拷贝，无需读回 CPU。
    ///
    /// # 前提（调用方保证）
    ///
    /// - 调用前 GL 上下文须 current（`make_current` 回调负责在需要时重绑）；
    /// - 此后每次与 wgpu 交互时它仍须 current——外部上下文模式下
    ///   `AdapterContext::lock()` 不再负责 `make_current`
    ///   （见 [`wgpu-hal` 的 `Adapter::new_external` 文档](wgpu::hal::gles::Adapter::new_external)）。
    ///
    /// GL 后端（Windows=WGL / Linux=EGL）可用。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn new_external(
        loader: impl FnMut(&str) -> *const std::ffi::c_void,
        make_current: impl FnOnce() -> Result<()>,
    ) -> Result<Self> {
        use wgpu::hal::api::Gles;

        // hal 的 GL 实例初始化会在结尾 `unmake_current`（它自建隐藏窗口
        // 上下文时把宿主的解绑了），所以必须在其后重绑，`new_external`
        // 才能查到宿主的扩展与函数。
        let hal_instance = unsafe {
            let desc = wgpu::hal::InstanceDescriptor {
                name: "opencat-three-external",
                flags: wgpu::InstanceFlags::empty(),
                memory_budget_thresholds: Default::default(),
                backend_options: Default::default(),
                telemetry: None,
                display: None,
            };
            <wgpu::hal::gles::Instance as wgpu::hal::Instance>::init(&desc)
                .map_err(|e| anyhow::anyhow!("hal instance 初始化失败: {e}"))?
        };
        make_current()?;
        let hal_adapter = unsafe {
            wgpu::hal::gles::Adapter::new_external(loader, wgpu::GlBackendOptions::default())
        }
        .ok_or_else(|| anyhow::anyhow!("外部 GL 上下文不可用（context 未 current？）"))?;
        let instance = unsafe { wgpu::Instance::from_hal::<Gles>(hal_instance) };
        let adapter = unsafe { instance.create_adapter_from_hal::<Gles>(hal_adapter) };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("opencat-three-external"),
            ..Default::default()
        }))
        .map_err(|e| anyhow::anyhow!("wgpu 设备创建失败: {e}"))?;
        Self::from_device(device, queue)
    }

    /// 自建上下文路径的设备请求（离屏渲染）。
    async fn request_device() -> Result<(wgpu::Device, wgpu::Queue)> {
        // 无窗口离屏渲染。先设平台首选后端，再 with_env 让 WGPU_BACKEND 覆盖。
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = preferred_backends();
        let mut desc = desc.with_env();
        // wgpu 自建的 GL 上下文写死 GL 3.3，而默认开启的间接调用校验要编译
        // compute 管线（GL 4.3+ 才有）——3.3 上失败且整个设备判死。预览路径
        // 的宿主上下文是 4.6（见 gpu-native egl.rs），能过；这里只能摘旗。
        // 校验只在 debug 下有意义，摘掉不影响渲染输出。
        desc.flags
            .remove(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL);
        let instance = wgpu::Instance::new(desc);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| anyhow::anyhow!("wgpu 适配器不可用（GPU/驱动缺失？）: {e}"))?;
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("opencat-three"),
                required_limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: MAX_TEXTURE_SLOTS as u32 + 3,
                    ..wgpu::Limits::downlevel_defaults()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| anyhow::anyhow!("wgpu 设备创建失败: {e}"))
    }

    /// 用既有设备建出渲染器（两条路径共用的尾部）。
    fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Result<Self> {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("opencat-three"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(WGSL)),
        });
        let globals_bg_layout = uniform_layout(&device, "globals");
        let object_bg_layout = uniform_layout(&device, "object");
        let texture_bg_layout = texture_layout(&device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("opencat-three"),
            bind_group_layouts: &[Some(&globals_bg_layout), Some(&object_bg_layout), Some(&texture_bg_layout)],
            immediate_size: 0,
        });
        let pipeline = build_render_pipeline(
            &device,
            "opencat-three",
            &pipeline_layout,
            &module,
            "vs",
            &module,
            "fs",
            None,
            MAIN_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba8Unorm,
            true,
        );
        let pipeline_blended = build_render_pipeline(
            &device,
            "opencat-three-blended",
            &pipeline_layout,
            &module,
            "vs",
            &module,
            "fs",
            // NormalBlending（src-alpha）：three.js 透射/透明列表的混合态
            //（WebGLState.setMaterial + Material.blending 默认 NormalBlending）。
            Some(wgpu::BlendState::ALPHA_BLENDING),
            MAIN_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba8Unorm,
            false,
        );
        // 透射 RT pass 变体：4×MSAA + HalfFloat 附件（WebGLRenderer.js
        // renderTransmissionPass 的 RT 参数，:1876-1880）——附件采样数与
        // 管线 multisample.count 必须一致，主 pass 管线不能复用。
        let pipeline_rt = build_render_pipeline(
            &device,
            "opencat-three-rt",
            &pipeline_layout,
            &module,
            "vs",
            &module,
            "fs",
            None,
            TRANSMISSION_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba16Float,
            true,
        );
        let pipeline_blended_rt = build_render_pipeline(
            &device,
            "opencat-three-rt-blended",
            &pipeline_layout,
            &module,
            "vs",
            &module,
            "fs",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            TRANSMISSION_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba16Float,
            false,
        );
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("opencat-three-globals"),
            size: GLOBALS_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("opencat-three-globals"),
            layout: &globals_bg_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });
        // 透射 RT pass 的 globals（output_xform 全 0 = NoToneMapping +
        // LinearSRGB 输出；WebGLRenderer.js:1921-1922/:2197）。同一布局，
        // 仅内容差异——逐帧随主 globals 一并写入。
        let globals_buf_rt = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("opencat-three-globals-rt-pass"),
            size: GLOBALS_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg_rt = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("opencat-three-globals-rt-pass"),
            layout: &globals_bg_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf_rt.as_entire_binding(),
            }],
        });
        let dummy = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opencat-three-dummy-tex"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let dummy_texture_view = dummy.create_view(&Default::default());
        // Linear 过滤 + Repeat 寻址（three.js VideoTexture js:235-238 固化）。
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("opencat-three-tex-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        // Linear 过滤 + Clamp 寻址：透射 RT（RenderTarget 纹理默认
        // ClampToEdge）与 DFG LUT（DataTexture ClampToEdge）。
        let clamp_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("opencat-three-clamp-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        // mip 生成管线（updateRenderTargetMipmap 的 glGenerateMipmap 对应
        // 物，wgpu 无内建）：全屏三角形 + 上一级线性采样。NDC y=−1 对应
        // 帧缓冲顶行、纹理 v=0 同为顶行 ⇒ uv = ndc·0.5+0.5 直通不翻转。
        let mipgen_module =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("opencat-three-mipgen"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(
                    r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi & 2u) * 2 - 1);
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = out.pos.xy * 0.5 + vec2<f32>(0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src, smp, in.uv, 0.0);
}
"#,
                )),
            });
        let mipgen_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("opencat-three-mipgen-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let mipgen_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("opencat-three-mipgen-layout"),
                bind_group_layouts: &[Some(&mipgen_bgl)],
                immediate_size: 0,
            });
        let mipgen_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("opencat-three-mipgen-pipeline"),
            layout: Some(&mipgen_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &mipgen_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mipgen_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        // DFG LUT：原始 u16 半精度位型直接按 Rg16Float 上传（dfg_lut.rs
        // 头注；DataTexture flipY=false 行序一致，无需翻转）。
        let dfg_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opencat-three-dfg-lut"),
            size: wgpu::Extent3d {
                width: DFG_LUT_SIZE,
                height: DFG_LUT_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        {
            let bytes: Vec<u8> = DFG_LUT_DATA.iter().flat_map(|h| h.to_le_bytes()).collect();
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &dfg_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(DFG_LUT_SIZE * 4),
                    rows_per_image: Some(DFG_LUT_SIZE),
                },
                wgpu::Extent3d {
                    width: DFG_LUT_SIZE,
                    height: DFG_LUT_SIZE,
                    depth_or_array_layers: 1,
                },
            );
        }
        let dfg_view = dfg_tex.create_view(&Default::default());
        // 脚本材质 map 采样布局（group 3）：three.js `map` uniform 的
        // 对应物——纹理（binding 0）+ Clamp/Linear 采样器（binding 1，
        // three.js Texture 默认 wrap ClampToEdge；wordmark js:57-60 的
        // Linear 过滤同款）。上传端翻转已由纹理表完成（flipY 默认 true）。
        let shader_map_bgl = device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("opencat-three-shader-map"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        Ok(Self {
            device,
            queue,
            pipeline,
            pipeline_blended,
            pipeline_rt,
            pipeline_blended_rt,
            object_bg_layout,
            texture_bg_layout,
            shader_map_bgl,
            globals_buf,
            globals_bg,
            globals_buf_rt,
            globals_bg_rt,
            dummy_texture_view: dummy_texture_view.clone(),
            sampler,
            clamp_sampler,
            dfg_view,
            env_view: dummy_texture_view,
            env_cubuv: [0.0; 4],
            env_cache_key: 0,
            depth: None,
            msaa_color: None,
            transmission: None,
            mipgen_bgl,
            mipgen_pipeline,
            shader_programs: Vec::new(),
            // 离屏 EGL 上下文只有 Linux 的 [`Self::new`] 路径持有；其余
            // 构造方（external 注入）不拥有上下文。
            #[cfg(target_os = "linux")]
            offscreen_egl: None,
        })
    }

    /// 设置场景环境（three.js `PMREMGenerator.fromEquirectangular` 的加载期
    /// 对应物，js:23-26：equirect JPEG → cubeUV RT 预滤波一次性完成）。
    /// 位图须为自然图像行序（行 0 = 顶）；PMREM 卷积在 [`pmrem`] 模块。
    /// `srgb` = three.js `scene.environment.colorSpace === SRGBColorSpace`
    /// （GL 采样即做 sRGB EOTF）；false 时字节按线性光使用。
    pub fn set_environment(&mut self, env: &TextureBitmap3, srgb: bool) -> Result<()> {
        // 缓存键不能用 Arc 指针：动态 env（脚本 surface）每帧都产出新的
        // Arc，指针必然不同，于是每帧重算一次 CPU 卷积（1024×512 的
        // equirect 约 4.5 s）。改按内容哈希——静态 env 一次命中，动态 env
        // 只在真正变化时重算。
        let key = env_content_key(env, srgb);
        if self.env_cache_key == key {
            return Ok(());
        }
        let pm = pmrem::generate(env, srgb)?;
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opencat-three-env-cubuv"),
            size: wgpu::Extent3d {
                width: pm.width,
                height: pm.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // 上传按 wgpu 256 B 行对齐补零（真实 env 的 3·cubeSize·8 恰是 256
        // 倍数，补零为退化小图路径服务；GL 无此约束，wgpu 有）。Rgba16Float
        // 每 texel 8 B ⇒ 紧凑行 = width·8。
        let row_bytes = pm.width as usize * 8;
        let bpr = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let mut bytes = vec![0u8; bpr * pm.height as usize];
        for (row, dst) in bytes.chunks_mut(bpr).enumerate() {
            let src = &pm.rgba_f16[row * row_bytes / 2..(row + 1) * row_bytes / 2];
            for (chunk, h) in dst[..row_bytes].chunks_mut(2).zip(src) {
                chunk.copy_from_slice(&h.to_le_bytes());
            }
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr as u32),
                rows_per_image: Some(pm.height),
            },
            wgpu::Extent3d {
                width: pm.width,
                height: pm.height,
                depth_or_array_layers: 1,
            },
        );
        self.env_view = tex.create_view(&Default::default());
        // generateCubeUVSize（WebGLProgram.js，envMapCubeUVHeight = RT 高）：
        // maxMip = log2(H) − 2、texelHeight = 1/H、
        // texelWidth = 1/(3·max(2^maxMip, 7·16))。
        let image_height = pm.height as f32;
        let max_mip = image_height.log2() - 2.0;
        let texel_height = 1.0 / image_height;
        let texel_width = 1.0 / (3.0 * 2.0f32.powf(max_mip).max(7.0 * 16.0));
        self.env_cubuv = [texel_width, texel_height, max_mip, 1.0];
        self.env_cache_key = key;
        Ok(())
    }

    /// 编译（或取缓存）一个脚本着色器程序，返回程序下标。三方块布局由
    /// three_api.js 的 adaptGlsl 前缀固定（set 0 = ThreeBuiltins、
    /// set 1 = UserUniforms），这里按 naga 反射核对并建 pipeline。
    fn shader_program(&mut self, material: &Material3) -> Result<usize> {
        let Shading3::Shader(s) = &material.shading else {
            bail!("shader_program 只接受 Shading3::Shader");
        };
        if let Some(idx) = self
            .shader_programs
            .iter()
            .position(|(k, _)| *k == (s.vertex_shader.clone(), s.fragment_shader.clone()))
        {
            return Ok(idx);
        }
        let (vs_module_naga, vs_wgsl) =
            compile_glsl(naga::ShaderStage::Vertex, &s.vertex_shader)?;
        let (fs_module_naga, fs_wgsl) =
            compile_glsl(naga::ShaderStage::Fragment, &s.fragment_shader)?;

        // set 0 三方块：两 stage 必须同布局（同一 buffer 绑定），成员表
        // 必须等于前缀规范（见 three_api.js adaptGlsl 的 ThreeBuiltins）。
        let canonical: Vec<(String, u32, u32)> = [
            // width = f32 分量数（mat4 → 16；cameraPosition 升 vec4 槽但
            // 分量数 3——std140 偏移 256，span 272）。
            ("projectionMatrix", 0u32, 16u32),
            ("modelViewMatrix", 64, 16),
            ("modelMatrix", 128, 16),
            ("viewMatrix", 192, 16),
            ("cameraPosition", 256, 3),
        ]
        .iter()
        .map(|(n, o, w)| (n.to_string(), *o, *w))
        .collect();
        for (stage, m) in [
            (naga::ShaderStage::Vertex, &vs_module_naga),
            (naga::ShaderStage::Fragment, &fs_module_naga),
        ] {
            match reflect_uniform_block(m, stage, 0, 0)? {
                Some((span, members)) => {
                    if span as u64 != SHADER_BUILTINS_SIZE || members != canonical {
                        bail!(
                            "ShaderMaterial {stage:?} ThreeBuiltins 布局偏离前缀规范（span {span}）"
                        );
                    }
                }
                None => bail!("ShaderMaterial {stage:?} 缺少 ThreeBuiltins 块（前缀注入失败）"),
            }
        }
        // set 1 脚本 uniforms：两 stage 一致即可，任一 stage 未声明则全无。
        let vs_user = reflect_uniform_block(&vs_module_naga, naga::ShaderStage::Vertex, 1, 0)?;
        let fs_user = reflect_uniform_block(&fs_module_naga, naga::ShaderStage::Fragment, 1, 0)?;
        let user = match (vs_user, fs_user) {
            (None, None) => None,
            (Some(a), None) | (None, Some(a)) => Some(a),
            (Some(a), Some(b)) => {
                if a == b {
                    Some(a)
                } else {
                    bail!("ShaderMaterial 两 stage 的 UserUniforms 不一致：{a:?} vs {b:?}")
                }
            }
        };

        // set 3 map 接缝：适配器（three_api.js adaptShaderMaterial）在材质
        // 带 map 时注入 `layout(set=3,binding=0) uniform texture2D map;`
        // + `layout(set=3,binding=1) uniform sampler map_smp;`。反射结果
        // 必须与材质声明双向一致（声明了 map 却没注入 → 管线布局缺 group 3
        // 会在 draw 时炸；没声明 map 却有 group 3 资源 → 无处绑定）。
        let material_has_map = material.map.is_some();
        let fs_map_resources = fs_module_naga.global_variables.iter().filter(|(_, var)| {
            var.space == naga::AddressSpace::Handle
                && matches!(var.binding, Some(rb) if rb.group == 3)
        }).count();
        let map_declared_in_shader = fs_map_resources > 0;
        match (material_has_map, map_declared_in_shader) {
            (true, false) => bail!(
                "ShaderMaterial 声明了 map 但片元段没有 set 3 采样（适配器注入失败或采样未被使用）"
            ),
            (false, true) => bail!(
                "ShaderMaterial 片元段出现 {fs_map_resources} 个 set 3 资源但材质未声明 map（组 3 是 map 专用接缝）"
            ),
            _ => {}
        }

    if std::env::var("OC_DUMP_WGSL").is_ok() {
            let dir = std::path::Path::new("out/wgsl-dump");
            let _ = std::fs::create_dir_all(dir);
            let n = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let _ = std::fs::write(dir.join(format!("vs-{n}.wgsl")), vs_wgsl.as_bytes());
            let _ = std::fs::write(dir.join(format!("fs-{n}.wgsl")), fs_wgsl.as_bytes());
        }
        let vs = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("opencat-three-shader-vs"),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(vs_wgsl.clone())),
        });
        let fs = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("opencat-three-shader-fs"),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(fs_wgsl.clone())),
        });
        let builtins_bgl = uniform_layout(&self.device, "opencat-three-shader-builtins");
        let user_bgl = user.is_some()
            .then(|| uniform_layout(&self.device, "opencat-three-shader-user"));
        let map_bgl = material_has_map.then(|| self.shader_map_bgl.clone());
        // 固定组号：set 0 builtins / set 1 user / set 3 map（set 2 是主管线
        // 纹理组，脚本材质不用）。None = 布局洞（wgpu 允许跳号）。
        let refs: Vec<Option<&wgpu::BindGroupLayout>> = vec![
            Some(&builtins_bgl),
            user_bgl.as_ref(),
            None,
            map_bgl.as_ref(),
        ];
        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("opencat-three-shader"),
            bind_group_layouts: &refs,
            immediate_size: 0,
        });
        // naga wgsl-out 的入口名 = GLSL 的 main。
        let pipeline = build_render_pipeline(
            &self.device,
            "opencat-three-shader",
            &pipeline_layout,
            &vs,
            "main",
            &fs,
            "main",
            None,
            MAIN_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba8Unorm,
            true,
        );
        let pipeline_blended = build_render_pipeline(
            &self.device,
            "opencat-three-shader-blended",
            &pipeline_layout,
            &vs,
            "main",
            &fs,
            "main",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            MAIN_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba8Unorm,
            false,
        );
        // 透射 RT pass 的 4×MSAA + Rgba16Float 变体（RT pass 也画
        // ShaderMaterial 不透明网格——背景板）。
        let pipeline_rt = build_render_pipeline(
            &self.device,
            "opencat-three-shader-rt",
            &pipeline_layout,
            &vs,
            "main",
            &fs,
            "main",
            None,
            TRANSMISSION_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba16Float,
            true,
        );
        let pipeline_blended_rt = build_render_pipeline(
            &self.device,
            "opencat-three-shader-rt-blended",
            &pipeline_layout,
            &vs,
            "main",
            &fs,
            "main",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            TRANSMISSION_SAMPLE_COUNT,
            wgpu::TextureFormat::Rgba16Float,
            false,
        );
        let idx = self.shader_programs.len();
        self.shader_programs.push((
            (s.vertex_shader.clone(), s.fragment_shader.clone()),
            ShaderProgram {
                pipeline,
                pipeline_blended,
                pipeline_rt,
                pipeline_blended_rt,
                builtins_bgl,
                user_bgl,
                map_bgl,
                // naga 的 struct span 不含 std140 尾部对齐填充（单 float
                // 块报 4）——uniform 缓冲按 16 对齐补齐（块可以小于缓冲）。
                user_size: user
                    .as_ref()
                    .map(|(span, _)| span.div_ceil(16) * 16)
                    .unwrap_or(0),
                user_members: user.map(|(_, members)| members).unwrap_or_default(),
            },
        ));
        Ok(idx)
    }

    /// 渲染一帧并读回 straight-alpha RGBA8（紧凑行序，长度 `w*h*4`）。
    ///
    /// `geometries`：[`scene3::GeometryRegistry`]——快照中
    /// `Geometry3::Asset { id }` 的解析来源（realm 级，一次性注册）。
    /// `textures`：本帧纹理表（槽位 = 表下标，见 [`TextureTable3`]）。
    pub fn render(
        &mut self,
        scene: &Scene3,
        geometries: &scene3::GeometryRegistry,
        textures: &TextureTable3,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>> {
        let tex = self.render_to_texture(scene, geometries, textures, width, height)?;
        self.read_back(&tex, width, height)
    }

    /// 渲染一帧并**保留**输出纹理在 GPU 上（不读回 CPU）。
    ///
    /// GPU 管线用：结果纹理随后交给 Skia 以零拷贝方式导入（见
    /// [`Self::output_native_texture`]），省掉 wgpu→CPU→Skia 的整帧往返。
    /// 调用方负责在不再需要时释放该纹理（drop 即可）。
    pub fn render_to_texture(
        &mut self,
        scene: &Scene3,
        geometries: &scene3::GeometryRegistry,
        textures: &TextureTable3,
        width: u32,
        height: u32,
    ) -> Result<wgpu::Texture> {
        if width == 0 || height == 0 {
            bail!("渲染尺寸必须非零，得到 {width}x{height}");
        }
        scene.validate()?;
        if scene.meshes.len() > MAX_MESHES_PER_FRAME {
            bail!(
                "mesh 数 {} 超过原型期上限 {MAX_MESHES_PER_FRAME}（换动态偏移缓冲后放开）",
                scene.meshes.len()
            );
        }

        // 相机：右手系 lookAt；directx 投影 = Z [0,1] + Y 向上，正是 wgpu/WebGPU 的 NDC。
        let aspect = width as f32 / height as f32;
        let view_m = view::look_at_mat4(
            Vec3::from(scene.camera.position),
            Vec3::from(scene.camera.target),
            Vec3::Y,
        );
        let proj_m = proj::directx::perspective(
            scene.camera.fov_y_degrees.to_radians(),
            aspect,
            scene.camera.near,
            scene.camera.far,
        );
        let mut globals = GlobalsUniform {
            view_proj: (proj_m * view_m).to_cols_array_2d(),
            view: view_m.to_cols_array_2d(),
            proj: proj_m.to_cols_array_2d(),
            light_dir: [0.0, 0.0, -1.0, 0.0],
            light_color: [0.0, 0.0, 0.0, 0.0],
            ambient: [0.0, 0.0, 0.0, 0.0],
            hemi_sky: [0.0, 0.0, 0.0, 0.0],
            hemi_ground: [0.0, 0.0, 0.0, 0.0],
            light_dir2: [0.0, 0.0, -1.0, 0.0],
            light_color2: [0.0, 0.0, 0.0, 0.0],
            resolution: [width as f32, height as f32, 0.0, 0.0],
            camera_pos: [
                scene.camera.position[0],
                scene.camera.position[1],
                scene.camera.position[2],
                0.0,
            ],
            // ⑥ 输出变换（js:18-20 声明；仅主 pass 内建材质分支读）。
            output_xform: [
                if matches!(scene.tone_mapping, ToneMapping3::ACESFilmic) { 1.0 } else { 0.0 },
                scene.tone_mapping_exposure,
                if matches!(scene.output_color_space, OutputColorSpace3::Srgb) { 1.0 } else { 0.0 },
                0.0,
            ],
            // ⑥b 环境采样参数（set_environment 预计算；w=1 语义 = 场景
            // 带 environment，对应 three.js USE_ENVMAP 定义）。RT pass 的
            // globals 同值——IBL 是场景光照内容，透射 RT 同样带 env。
            point_pos: [[0.0, 0.0, 0.0, 2.0], [0.0, 0.0, 0.0, 2.0]],
            point_color: [[0.0; 4], [0.0; 4]],
            env_cubuv: self.env_cubuv,
        };
        // 环境光求和；Hemisphere 取第一个；方向光按场景序取前两个
        // （对齐片的布光正好是 key + rim 两盏；更多是后续扩展）。
        let mut dir_lights: Vec<(Vec3, [f32; 3])> = Vec::new();
        let mut point_lights: Vec<([f32; 3], [f32; 3], f32, f32)> = Vec::new();
        let mut hemi_taken = false;
        for light in &scene.lights {
            match light {
                Light3::Ambient { color, intensity } => {
                    for i in 0..3 {
                        globals.ambient[i] += color[i] * intensity;
                    }
                }
                Light3::Hemisphere {
                    sky_color,
                    ground_color,
                    intensity,
                } => {
                    if !hemi_taken {
                        hemi_taken = true;
                        for i in 0..3 {
                            globals.hemi_sky[i] = sky_color[i] * intensity;
                            globals.hemi_ground[i] = ground_color[i] * intensity;
                        }
                    }
                }
                Light3::Point {
                    position,
                    color,
                    intensity,
                    distance,
                    decay,
                } => {
                    if point_lights.len() < 2 {
                        point_lights.push((
                            *position,
                            [
                                color[0] * intensity,
                                color[1] * intensity,
                                color[2] * intensity,
                            ],
                            *distance,
                            *decay,
                        ));
                    }
                }
                Light3::Directional {
                    direction,
                    color,
                    intensity,
                } => {
                    if dir_lights.len() < 2 {
                        dir_lights.push((
                            Vec3::from(*direction).normalize_or_zero(),
                            [
                                color[0] * intensity,
                                color[1] * intensity,
                                color[2] * intensity,
                            ],
                        ));
                    }
                }
            }
        }
        if let Some((d, c)) = dir_lights.first() {
            globals.light_dir = [d.x, d.y, d.z, 0.0];
            globals.light_color = [c[0], c[1], c[2], 0.0];
        }
        if let Some((d, c)) = dir_lights.get(1) {
            globals.light_dir2 = [d.x, d.y, d.z, 0.0];
            globals.light_color2 = [c[0], c[1], c[2], 0.0];
        }
        for (i, (pos, color, distance, decay)) in point_lights.iter().enumerate() {
            globals.point_pos[i] = [pos[0], pos[1], pos[2], *decay];
            globals.point_color[i] = [color[0], color[1], color[2], *distance];
        }
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));
        // RT pass 版本：output_xform 全 0（WebGLRenderer.js:1921-1922 透射
        // 二遍强制 NoToneMapping；:2197 RT 输出 LinearSRGB——两开关同关）。
        globals.output_xform = [0.0; 4];
        self.queue
            .write_buffer(&self.globals_buf_rt, 0, bytemuck::bytes_of(&globals));

        // 纹理槽表：槽位 = 表下标（宿主与渲染器按同一遍历规则推出同序——
        // 逐 mesh 先 textures 槽表后 map）。上传用 `queue.write_texture`
        // 逐帧直写（视频帧逐帧变，缓存无意义）；未用槽绑 1×1 占位。
        if textures.len() > MAX_TEXTURE_SLOTS {
            bail!(
                "纹理表 {} 路超过原型期上限 {MAX_TEXTURE_SLOTS}（WebGL2 采样器预算）",
                textures.len()
            );
        }
        let mut slot_of: std::collections::HashMap<&str, u32> =
            std::collections::HashMap::with_capacity(textures.len());
        for (slot, (key, _)) in textures.iter().enumerate() {
            slot_of.insert(key.as_str(), slot as u32);
        }
        let mut tex_views: Vec<wgpu::TextureView> = Vec::with_capacity(MAX_TEXTURE_SLOTS);
        for (slot, (_, bitmap)) in textures.iter().enumerate() {
            let expected = bitmap.width as usize * bitmap.height as usize * 4;
            if bitmap.rgba.len() != expected {
                bail!(
                    "纹理槽 {slot}（{key}）：rgba 长度 {len} != {w}x{h}x4",
                    key = textures[slot].0,
                    len = bitmap.rgba.len(),
                    w = bitmap.width,
                    h = bitmap.height
                );
            }
            // three.js Texture 默认 flipY=true（js:233-240，VideoTexture 亦然）：
            // GL 上传时翻转行序，纹理内存约定 v=0 = 图像底部行。着色器一律以
            // three.js UV 语义（v 向上）直接采样——马赛克 surfaceUv（js:314）与
            // Unlit map 均无需各自再翻。
            let w = bitmap.width as usize;
            let h = bitmap.height as usize;
            let mut flipped = vec![0u8; bitmap.rgba.len()];
            for row in 0..h {
                let src = &bitmap.rgba[row * w * 4..(row + 1) * w * 4];
                flipped[(h - 1 - row) * w * 4..(h - row) * w * 4].copy_from_slice(src);
            }
            let upload: &[u8] = &flipped;
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opencat-three-tex"),
                size: wgpu::Extent3d {
                    width: bitmap.width,
                    height: bitmap.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                upload,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bitmap.width * 4),
                    rows_per_image: Some(bitmap.height),
                },
                wgpu::Extent3d {
                    width: bitmap.width,
                    height: bitmap.height,
                    depth_or_array_layers: 1,
                },
            );
            tex_views.push(tex.create_view(&Default::default()));
        }
        // 透射场景色缓冲：场景里有透射网格（three.js transmissive render
        // list：`material.transmission > 0`，WebGLRenderLists.js:118-121）
        // 才建/用（WebGLRenderer.renderTransmissionPass 只在有透射对象时
        // 执行，WebGLRenderer.js:1641/1665）；绑定端永远要有视图 —— 无透
        // 射网格时绑 1×1 占位。
        let has_transmissive = scene.meshes.iter().any(|m| {
            matches!(m.material.shading, Shading3::Physical(p) if p.transmission > 0.0)
        });
        let transmission_msaa = if has_transmissive {
            Some(self.ensure_transmission(width, height))
        } else {
            None
        };
        // resolve 目标（level 0）与全 mip sample 视图（binding 9）：RT 有
        // 透射网格才带 mip 链。
        let (transmission_base, transmission_sample) = if transmission_msaa.is_some() {
            let rt = self
                .transmission
                .as_ref()
                .unwrap_or_else(|| unreachable!("ensure_transmission 刚写入了 transmission"));
            (Some(rt.base_view.clone()), Some(rt.sample_view.clone()))
        } else {
            (None, None)
        };
        // MSAA 深度视图（RT pass 专用；主 pass 深度另有一份 4×，见
        // ensure_depth）。
        let transmission_depth_view = if transmission_msaa.is_some() {
            Some(
                self.transmission
                    .as_ref()
                    .unwrap_or_else(|| unreachable!("ensure_transmission 刚写入了 transmission"))
                    .msaa_depth
                    .create_view(&Default::default()),
            )
        } else {
            None
        };

        let mut tex_entries: Vec<wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURE_SLOTS + 4);
        for slot in 0..MAX_TEXTURE_SLOTS {
            tex_entries.push(wgpu::BindGroupEntry {
                binding: slot_binding(slot),
                resource: wgpu::BindingResource::TextureView(
                    tex_views.get(slot).unwrap_or(&self.dummy_texture_view),
                ),
            });
        }
        tex_entries.push(wgpu::BindGroupEntry {
            binding: 8, // 共享 Repeat 采样器（与布局同号）
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        });
        tex_entries.push(wgpu::BindGroupEntry {
            binding: 9,
            resource: wgpu::BindingResource::TextureView(
                transmission_sample
                    .as_ref()
                    .unwrap_or(&self.dummy_texture_view),
            ),
        });
        tex_entries.push(wgpu::BindGroupEntry {
            binding: 10,
            resource: wgpu::BindingResource::Sampler(&self.clamp_sampler),
        });
        tex_entries.push(wgpu::BindGroupEntry {
            binding: 11,
            resource: wgpu::BindingResource::TextureView(&self.dfg_view),
        });
        // 场景环境 PMREM cubeUV（binding 44）：无 env 绑 dummy（w=0 时
        // WGSL 不采样）。env 纹理从不是渲染目标 ⇒ 两 pass 同视图合法。
        tex_entries.push(wgpu::BindGroupEntry {
            binding: 44,
            resource: wgpu::BindingResource::TextureView(&self.env_view),
        });
        let texture_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("opencat-three-textures"),
            layout: &self.texture_bg_layout,
            entries: &tex_entries,
        });
        // 透射 pass 自己的绑定组：binding 9（透射 RT）换 1×1 占位 —— 同一
        // 纹理不能在同一 render pass 里既当渲染目标又当采样资源（wgpu
        // usage 冲突）；不透明网格不执行 Physical 分支，占位视图不会被
        // 采样。RT 条目在 entries 数组里的下标 = 槽条目数 + 1（采样器
        // 在槽条目之后）。
        let mut tex_entries_a = tex_entries.clone();
        tex_entries_a[MAX_TEXTURE_SLOTS + 1].resource =
            wgpu::BindingResource::TextureView(&self.dummy_texture_view);
        let texture_bg_transmission = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("opencat-three-textures-transmission-pass"),
            layout: &self.texture_bg_layout,
            entries: &tex_entries_a,
        });

        self.ensure_depth(width, height);
        // 克隆出视图（深度/MSAA 色附件缓存在 self；后续 mesh 循环要
        // &mut self 编译脚本着色器程序，不能带着 &self 的借用）。
        let depth_view = match &self.depth {
            Some((_, _, view)) => view.clone(),
            None => unreachable!("ensure_depth 刚写入了 depth"),
        };
        let msaa_color_view = self.ensure_msaa_color(width, height);

        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opencat-three-out"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let tex_view = tex.create_view(&Default::default());

        let mut encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("opencat-three"),
                });
        {
            // three.js 渲染列表分类（WebGLRenderLists.js push，:118-133）：
            // transmission > 0 → transmissive；transparent → transparent；
            // 其余 → opaque。主 pass 顺序 opaque → transmissive →
            // transparent（WebGLRenderer.js renderScene，:1849-1851）；透射
            // RT pass 只画 opaque 列表（renderTransmissionPass :1933
            // renderObjects(opaqueObjects)；DoubleSide 透射物背面补画不适用
            // —— 液体材质 FrontSide，js:163-179 无 side 参数）。
            #[derive(Clone, Copy, PartialEq, Eq, Debug)]
            enum ListKind {
                Opaque,
                Transmissive,
                Transparent,
            }
            let list_of = |mesh: &Mesh3| -> ListKind {
                if matches!(mesh.material.shading, Shading3::Physical(p) if p.transmission > 0.0)
                {
                    ListKind::Transmissive
                } else if mesh.material.transparent {
                    ListKind::Transparent
                } else {
                    ListKind::Opaque
                }
            };
            // 排序键。z = 世界原点经 view_proj 的 clip z（three.js
            // projectObject 对 matrixWorld 平移分量 applyMatrix4(_projScreenMatrix)
            // 的同构——directx 投影 z 单调，远 → 大）。
            let view_proj = Mat4::from_cols_array_2d(&globals.view_proj);
            let mut order: Vec<(usize, ListKind, i32, f32)> =
                Vec::with_capacity(scene.meshes.len());
            for (i, mesh) in scene.meshes.iter().enumerate() {
                let wp = mesh.matrix;
                let clip = view_proj * Vec4::new(wp[12], wp[13], wp[14], 1.0);
                order.push((i, list_of(mesh), mesh.render_order, clip.z));
            }
            // painterSortStable / reversePainterSortStable（WebGLRenderLists.js:1-51）：
            // renderOrder 升序 → z（opaque 升序 / 其余降序=远到近）→ id 升序。
            // material.id 键本 IR 无对应概念（材质无身份），以场景序兜底；
            // 参考场景每个不透明物 renderOrder 互异，该键不参与。
            order.sort_by(|a, b| {
                a.2.cmp(&b.2)
                    .then_with(|| match a.1 {
                        ListKind::Opaque => a.3.total_cmp(&b.3),
                        _ => b.3.total_cmp(&a.3),
                    })
                    .then_with(|| a.0.cmp(&b.0))
            });
            // three.js 实际遍历的是三条列表「各自排序后」的拼接
            // （WebGLRenderer.js renderScene :1849-1851 依次 renderObjects
            // opaque → transmissive → transparent，每条列表独立 sort）。上方
            // 单一排序已给出每段的内部序（键同 painterSortStable /
            // reversePainterSortStable），稳定分段拼接即与三列表遍历逐
            // draw 一致——无条件成立，不依赖「renderOrder 值跨列表分区」
            // 的前提（r4 评审修正：此前主 pass 用单一交错序，仅当全
            // opaque renderOrder < 全 transmissive < 全 transparent 时才
            // 与 three.js 重合）。
            let mut draw_order: Vec<(usize, ListKind, i32, f32)> =
                Vec::with_capacity(order.len());
            for kind in [ListKind::Opaque, ListKind::Transmissive, ListKind::Transparent] {
                draw_order.extend(order.iter().copied().filter(|&(_, l, _, _)| l == kind));
            }

            // 先为每个网格准备 GPU 资源（顶点/索引/uniform/绑定组），两个
            // pass 复用同一批资源：透射 RT pass（不透明网格）与主 pass。
            struct MeshGpuDraw {
                vbuf: wgpu::Buffer,
                ibuf: wgpu::Buffer,
                bg: wgpu::BindGroup,
                index_count: u32,
                list: ListKind,
                /// 脚本着色器绘制资源（None = 主管线）。
                shader: Option<ShaderDraw>,
            }
            struct ShaderDraw {
                program: usize,
                builtins_bg: wgpu::BindGroup,
                user_bg: Option<wgpu::BindGroup>,
                /// 材质 map 的 group 3 绑定组（binding 0 = 表内贴图视图，
                /// binding 1 = Clamp 采样器；不引用透射 RT，两 pass 通用）。
                map_bg: Option<wgpu::BindGroup>,
            }
            let mut draws: Vec<MeshGpuDraw> = Vec::with_capacity(scene.meshes.len());
            for mesh in &scene.meshes {
                let (positions, normals, indices, uvs, colors, colors1) = match &mesh.geometry {
                    Geometry3::Inline {
                        positions,
                        normals,
                        indices,
                        uvs,
                        colors,
                        colors1,
                    } => (positions, normals, indices, uvs, colors, colors1),
                    // 注册表几何：从 realm 级注册表解析（重网格不逐帧过快照）。
                    Geometry3::Asset { id } => {
                        let geo = geometries.get(id).ok_or_else(|| {
                            anyhow::anyhow!("geometry Asset '{id}' 不在注册表中（scene3_geometry_register 未调用或 id 不符）")
                        })?;
                        match geo.as_ref() {
                            Geometry3::Inline {
                                positions,
                                normals,
                                indices,
                                uvs,
                                colors,
                                colors1,
                            } => (positions, normals, indices, uvs, colors, colors1),
                            Geometry3::Asset { id: nested } => bail!(
                                "geometry Asset '{id}' 指向另一个 Asset '{nested}'（注册表值必须是 Inline）"
                            ),
                        }
                    }
                };
                // interleaved: pos + normal + uv + color0 + color1。缺省属性
                // 填中性值（uv 原点、color0 白、color1 零）——只有马赛克材质
                // 消费 color/color1，缺省值不会误触发主面/角色逻辑。
                let count = positions.len();
                let mut verts = Vec::with_capacity(count * 16);
                for (i, (p, n)) in positions.iter().zip(normals.iter()).enumerate() {
                    verts.extend_from_slice(p);
                    verts.extend_from_slice(n);
                    match uvs.get(i) {
                        Some(uv) => verts.extend_from_slice(uv),
                        None => verts.extend_from_slice(&[0.0_f32, 0.0]),
                    }
                    match colors.get(i) {
                        Some(c) => verts.extend_from_slice(c),
                        None => verts.extend_from_slice(&[1.0_f32, 1.0, 1.0, 1.0]),
                    }
                    match colors1.get(i) {
                        Some(c) => verts.extend_from_slice(c),
                        None => verts.extend_from_slice(&[0.0_f32, 0.0, 0.0, 0.0]),
                    }
                }
                let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("opencat-three-v"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("opencat-three-i"),
                    contents: bytemuck::cast_slice(indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                // 贴图槽位：按表键解析（Unlit+map / Physical 的 glTF PBR
                // 贴图消费；其余材质忽略）。
                let resolve_slot = |tex: &opencat_core::ir::scene3::Texture3| {
                    slot_of.get(tex.table_key().as_str()).copied().ok_or_else(|| {
                        anyhow::anyhow!("材质纹理 '{:?}' 不在宿主纹理表中", tex.table_key())
                    })
                };
                let map_slot = mesh.material.map.as_ref().map(|t| resolve_slot(t)).transpose()?;
                // glTF PBR 贴图只对 Physical 有意义（其余着色模型忽略）。
                let pbr_slots = if matches!(mesh.material.shading, Shading3::Physical(_)) {
                    (
                        mesh.material
                            .base_color_map
                            .as_ref()
                            .map(|t| resolve_slot(t))
                            .transpose()?,
                        mesh.material
                            .metallic_roughness_map
                            .as_ref()
                            .map(|t| resolve_slot(t))
                            .transpose()?,
                        mesh.material
                            .normal_map
                            .as_ref()
                            .map(|t| resolve_slot(t))
                            .transpose()?,
                    )
                } else {
                    (None, None, None)
                };
                let ubuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("opencat-three-u"),
                    contents: bytemuck::bytes_of(&object_uniform(
                        &mesh.material,
                        &mesh.matrix,
                        map_slot,
                        pbr_slots,
                    )),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("opencat-three-object"),
                    layout: &self.object_bg_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubuf.as_entire_binding(),
                    }],
                });
                // 脚本着色器：三方块（proj/modelView/model/view/cameraPos）
                // + 脚本 uniforms（std140，反射偏移）。buffer 逐帧重建——
                // 与上方 vbuf/ubuf 同一批原型期分配策略。
                let mut shader = if matches!(mesh.material.shading, Shading3::Shader(_)) {
                    let program = self.shader_program(&mesh.material)?;
                    let model = Mat4::from_cols_array(&mesh.matrix);
                    let builtins = ShaderBuiltinsUniform {
                        proj: proj_m.to_cols_array_2d(),
                        model_view: (view_m * model).to_cols_array_2d(),
                        model: model.to_cols_array_2d(),
                        view: view_m.to_cols_array_2d(),
                        camera_pos: globals.camera_pos,
                    };
                    let builtins_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("opencat-three-shader-builtins"),
                                contents: bytemuck::bytes_of(&builtins),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });
                    let builtins_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("opencat-three-shader-builtins"),
                        layout: &self.shader_programs[program].1.builtins_bgl,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: builtins_buf.as_entire_binding(),
                        }],
                    });
                    let user_bg = match &self.shader_programs[program].1.user_bgl {
                        Some(layout) => {
                            let bytes = pack_user_uniforms(
                                &self.shader_programs[program].1,
                                shader_uniforms(&mesh.material),
                            )?;
                            let ubuf =
                                self.device
                                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                        label: Some("opencat-three-shader-user"),
                                        contents: &bytes,
                                        usage: wgpu::BufferUsages::UNIFORM,
                                    });
                            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("opencat-three-shader-user"),
                                layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: ubuf.as_entire_binding(),
                                }],
                            }))
                        }
                        None => None,
                    };
                    Some(ShaderDraw {
                        program,
                        builtins_bg,
                        user_bg,
                        map_bg: None,
                    })
                } else {
                    None
                };
                // 脚本材质 map → group 3 绑定组（表内贴图视图 + Clamp 采样
                // 器）。不引用透射 RT，透射/主两个 pass 通用。
                if let (Some(sd), Some(slot)) = (&mut shader, map_slot) {
                    let map_bgl = self.shader_programs[sd.program]
                        .1
                        .map_bgl
                        .as_ref()
                        .expect("带 map 的 ShaderMaterial 程序必有 map 布局（shader_program 已校验）");
                    sd.map_bg = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("opencat-three-shader-map"),
                        layout: map_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &tex_views[slot as usize],
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.clamp_sampler),
                            },
                        ],
                    }));
                }
                draws.push(MeshGpuDraw {
                    vbuf,
                    ibuf,
                    bg,
                    index_count: indices.len() as u32,
                    list: list_of(mesh),
                    shader,
                });
            }
            // 透射 pass（WebGLRenderer.renderTransmissionPass：只渲染不透明
            // 列表进 RT）。清屏 = 场景清色但强制不透明 alpha（参考 alpha:false
            // 上下文 clearAlpha 恒 1）。
            if let Some(rt_view) = &transmission_msaa {
                let base_view = transmission_base
                    .as_ref()
                    .unwrap_or_else(|| unreachable!("transmission_msaa 与 base 同源"));
                let depth_a = transmission_depth_view
                    .as_ref()
                    .unwrap_or_else(|| unreachable!("transmission_msaa 与 depth 同源"));
                let mut pass_a = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("opencat-three-transmission"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: rt_view,
                        depth_slice: None,
                        // 4×MSAA resolve → mip 链 level 0（WebGLRenderer.js
                        // :1942 updateMultisampleRenderTarget 同序）。
                        resolve_target: Some(base_view),
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: scene.clear[0] as f64,
                                g: scene.clear[1] as f64,
                                b: scene.clear[2] as f64,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_a,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                // RT pass = 不透明列表 = draw_order 的 Opaque 前段（three.js
                // renderTransmissionPass 只 renderObjects(opaqueObjects)）。
                for &(idx, list, _, _) in draw_order
                    .iter()
                    .take_while(|(_, l, ..)| *l == ListKind::Opaque)
                {
                    if list != ListKind::Opaque {
                        continue;
                    }
                    let d = &draws[idx];
                    // 每绘制切状态：脚本材质自带 pipeline 与 group0/1/3
                    // （其 layout 没有完整 group2），主管线绘制重绑 globals/
                    // object/textures —— RT pass 的 textures 版本带占位
                    // binding 9（见上方 texture_bg_transmission 注释）。
                    match &d.shader {
                        Some(sd) => {
                            let prog = &self.shader_programs[sd.program].1;
                            pass_a.set_pipeline(&prog.pipeline_rt);
                            pass_a.set_bind_group(0, &sd.builtins_bg, &[]);
                            if let Some(u) = &sd.user_bg {
                                pass_a.set_bind_group(1, u, &[]);
                            }
                            if let Some(m) = &sd.map_bg {
                                pass_a.set_bind_group(3, m, &[]);
                            }
                        }
                        None => {
                            pass_a.set_pipeline(&self.pipeline_rt);
                            // RT pass globals = output_xform 全 0 版本（three.js
                            // 透射二遍 NoToneMapping + LinearSRGB，WebGLRenderer.js
                            // :1921-1922/:2197；本 pass 现场景只含 ShaderMaterial
                            // 网格（不读该 uniform），通用场景下内建网格在此线性
                            // 输出进 RT，与参考一致）。
                            pass_a.set_bind_group(0, &self.globals_bg_rt, &[]);
                            pass_a.set_bind_group(1, &d.bg, &[]);
                            pass_a.set_bind_group(2, &texture_bg_transmission, &[]);
                        }
                    }
                    pass_a.set_vertex_buffer(0, d.vbuf.slice(..));
                    pass_a.set_index_buffer(d.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass_a.draw_indexed(0..d.index_count, 0, 0..1);
                }
            }
            // updateRenderTargetMipmap（WebGLRenderer.js:1942-1943：不透明
            // 列表画完后立即生成 mip）。
            if has_transmissive {
                self.generate_transmission_mips(&mut encoder);
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("opencat-three"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    // 4×MSAA 附件 + resolve 到 1× 读回纹理（antialias:true
                    // 的 drawing buffer 语义，MAIN_SAMPLE_COUNT 注释）。
                    view: &msaa_color_view,
                    depth_slice: None,
                    resolve_target: Some(&tex_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: scene.clear[0] as f64,
                            g: scene.clear[1] as f64,
                            b: scene.clear[2] as f64,
                            a: scene.clear[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // 主 pass：draw_order 拼接序 = opaque → transmissive →
            // transparent（renderScene 顺序，见上方分段拼接注释），后两者用
            // src-alpha 混合管线（three.js 透射物进 transparent 语义的
            // NormalBlending；WebGLState.setMaterial）。
            for &(idx, list, _, _) in &draw_order {
                let d = &draws[idx];
                let blended = list != ListKind::Opaque;
                // 每绘制切状态（与 RT pass 同构；主 pass 的 group2 是完整
                // 纹理组）。脚本材质的 layout 是 group0/1/3。
                match &d.shader {
                    Some(sd) => {
                        let prog = &self.shader_programs[sd.program].1;
                        pass.set_pipeline(if blended {
                            &prog.pipeline_blended
                        } else {
                            &prog.pipeline
                        });
                        pass.set_bind_group(0, &sd.builtins_bg, &[]);
                        if let Some(u) = &sd.user_bg {
                            pass.set_bind_group(1, u, &[]);
                        }
                        if let Some(m) = &sd.map_bg {
                            pass.set_bind_group(3, m, &[]);
                        }
                    }
                    None => {
                        pass.set_pipeline(if blended {
                            &self.pipeline_blended
                        } else {
                            &self.pipeline
                        });
                        pass.set_bind_group(0, &self.globals_bg, &[]);
                        pass.set_bind_group(1, &d.bg, &[]);
                        pass.set_bind_group(2, &texture_bg, &[]);
                    }
                }
                pass.set_vertex_buffer(0, d.vbuf.slice(..));
                pass.set_index_buffer(d.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..d.index_count, 0, 0..1);
            }
        }

        self.queue.submit([encoder.finish()]);
        Ok(tex)
    }

    /// 把输出纹理读回 CPU（straight-alpha RGBA8，紧凑行序）。
    ///
    /// 与 [`Self::render_to_texture`] 配对：CPU 路径用 `render` 一步到位；
    /// GPU 路径用 `render_to_texture` + Skia 导入，只有需要落盘时才调本方法。
    pub fn read_back(&self, tex: &wgpu::Texture, width: u32, height: u32) -> Result<Vec<u8>> {
        // 读回：行距按 256 对齐，拷回后压缩成紧凑行。
        let bpr = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buf_size = bpr as u64 * (height as u64 - 1) + width as u64 * 4;
        let rbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("opencat-three-readback"),
            size: buf_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("opencat-three-readback"),
                });
        encoder.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &rbuf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = rbuf.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| anyhow::anyhow!("等待渲染完成失败: {e}"))?;
        rx.recv()
            .map_err(|_| anyhow::anyhow!("map 回调丢失"))?
            .map_err(|e| anyhow::anyhow!("读回映射失败: {e}"))?;
        let mapped = slice
            .get_mapped_range()
            .map_err(|e| anyhow::anyhow!("读回视图失败: {e}"))?;
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * bpr) as usize;
            out.extend_from_slice(&mapped[start..start + (width * 4) as usize]);
        }
        drop(mapped);
        rbuf.unmap();
        Ok(out)
    }

    /// 输出纹理的原生句柄（GPU 管线的零拷贝导入凭据）。
    ///
    /// 仅 GL 后端有效；非 GL 后端返回 `None`。`tex` 必须是本渲染器
    /// [`Self::render_to_texture`] 的产物，且在其存活期内使用。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn output_native_texture(tex: &wgpu::Texture) -> Option<NativeTexture> {
        use wgpu::hal::api::Gles;
        unsafe {
            let hal_tex = tex.as_hal::<Gles>()?;
            match &hal_tex.inner {
                wgpu::hal::gles::TextureInner::Texture { raw, .. } => {
                    Some(NativeTexture::from_raw(raw.0.get() as u64))
                }
                _ => None,
            }
        }
    }

    /// 在 wgpu 的 GL context 已 make current 的前提下运行 `f`。
    ///
    /// Skia 共享同一 GL context：建 DirectContext、导入纹理、绘制都必须发生在
    /// 该 context current 的线程上。本方法经 wgpu 自己的 `AdapterContext`
    /// 锁进入（与 wgpu 内部操作同一把互斥锁），drop 时 unmake_current。
    ///
    /// **`f` 内不得再调用本渲染器的任何方法**——会二次加锁，1 秒后 panic。
    /// 仅 GL 后端有效；非 GL 返回 `None` 且不调用 `f`。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn with_gl_context<R>(&self, f: impl FnOnce() -> R) -> Option<R> {
        use wgpu::hal::api::Gles;
        unsafe {
            let hal_dev = self.device.as_hal::<Gles>()?;
            let _lock = hal_dev.context().lock();
            Some(f())
        }
    }

    /// 建一张可用作 Skia 合成目标的纹理（GL / Metal 后端）。
    ///
    /// 在**锁外**调用（创建纹理是 wgpu 操作）。GL 上 FBO 与读回由
    /// `opencat_gpu_native::gl::Target` 在锁内建；Metal 上 Skia 直接
    /// 包这张 MTLTexture 作渲染目标。
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub fn create_composite_target(&self, width: u32, height: u32) -> Option<wgpu::Texture> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opencat-three-composite-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Some(tex)
    }

    /// 设备引用（合成器建资源用；**不得在持锁期间调用**）。
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// 当前后端是否为 GL（GPU 纹理共享仅 GL 路径支持）。
    pub fn is_gles(&self) -> bool {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            unsafe { self.device.as_hal::<wgpu::hal::api::Gles>().is_some() }
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            false
        }
    }

    /// 当前后端是否为 Metal（macOS 的 GPU 纹理共享路径）。
    pub fn is_metal(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            unsafe { self.device.as_hal::<wgpu::hal::api::Metal>().is_some() }
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// 输出纹理的原生句柄（Metal 后端）。
    ///
    /// 返回 MTLTexture 的裸指针值；调用方须保证纹理在采样期内存活。
    #[cfg(target_os = "macos")]
    pub fn output_native_texture_metal(tex: &wgpu::Texture) -> Option<NativeTexture> {
        use wgpu::hal::api::Metal;
        unsafe {
            let hal_tex = tex.as_hal::<Metal>()?;
            let raw = hal_tex.raw_handle();
            // ProtocolObject 是 #[repr(C)] 的单个 objc_object 指针，直接取地址值。
            let ptr = raw as *const _ as *const c_void as u64;
            Some(NativeTexture::from_raw(ptr))
        }
    }

    /// 取 Metal 的 MTLDevice 裸指针（Skia Metal BackendContext 用）。
    #[cfg(target_os = "macos")]
    pub fn metal_device_raw(&self) -> Option<u64> {
        use wgpu::hal::api::Metal;
        unsafe {
            let hal_dev = self.device.as_hal::<Metal>()?;
            let raw = hal_dev.raw_device();
            Some(Retained::as_ptr(raw) as *const c_void as u64)
        }
    }

    /// 取 Metal 的 MTLCommandQueue 裸指针。
    ///
    /// wgpu 的 Queue 没有公开 raw accessor，所以这里用设备自建一条独立队列。
    /// Skia 与 wgpu 各用一条队列是安全的：两者共享同一 MTLDevice，资源
    /// 同步由 Metal 的 hazard tracking 负责。
    #[cfg(target_os = "macos")]
    pub fn metal_queue_raw(&self) -> Option<u64> {
        use wgpu::hal::api::Metal;
        unsafe {
            let hal_q = self.queue.as_hal::<Metal>()?;
            Some(hal_q.as_raw() as *const _ as *const c_void as u64)
        }
    }

    /// 从一张 wgpu 纹理读回 CPU（合成完成后落盘给编码器用）。
    pub fn read_back_texture(
        &self,
        tex: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>> {
        self.read_back(tex, width, height)
    }

    fn ensure_depth(&mut self, width: u32, height: u32) {
        let stale = self
            .depth
            .as_ref()
            .is_none_or(|(w, h, _)| *w != width || *h != height);
        if stale {
            // 主 pass 深度 4×MSAA（与主管线 multisample.count 一致是 wgpu
            // 验证约束；与透射 RT 的 msaa_depth 尺寸/格式同但句柄分立
            // —— three.js RT 深度是 renderTarget 自己的 depthTexture，
            // drawing buffer 深度是另一份）。
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opencat-three-depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: MAIN_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            self.depth = Some((width, height, view));
        }
    }

    /// 主 pass 4×MSAA 颜色附件（尺寸变化才重建）。antialias:true 的
    /// drawing buffer 对应物：draw 落 4× 附件，resolve_target 写 1× 读回
    /// 纹理（浏览器合成时 resolve 的 wgpu 等价物）。
    fn ensure_msaa_color(&mut self, width: u32, height: u32) -> wgpu::TextureView {
        let stale = self
            .msaa_color
            .as_ref()
            .is_none_or(|(w, h, _)| *w != width || *h != height);
        if stale {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opencat-three-main-msaa"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: MAIN_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            self.msaa_color = Some((width, height, view));
        }
        match &self.msaa_color {
            Some((_, _, view)) => view.clone(),
            None => unreachable!("ensure_msaa_color 刚写入了 msaa_color"),
        }
    }

    /// 透射场景色缓冲（尺寸变化才重建）。尺寸 = 视口（three.js
    /// `transmissionResolutionScale` 默认 1.0，WebGLRenderer.js:282）。
    /// RT 参数逐项对照 WebGLRenderer.js:1875-1883：`generateMipmaps: true`
    /// → 完整 mip 链；HalfFloat；`samples: 4` → MSAA 渲染 + resolve。
    /// 返回 MSAA 侧视图（pass_a 附件）；resolve 见 render() 的
    /// resolve_target。
    fn ensure_transmission(&mut self, width: u32, height: u32) -> wgpu::TextureView {
        let mip_count = (width.max(height) as f32).log2().floor() as u32 + 1;
        let stale = self
            .transmission
            .as_ref()
            .is_none_or(|rt| rt.width != width || rt.height != height);
        if stale {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opencat-three-transmission"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: mip_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opencat-three-transmission-msaa"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: TRANSMISSION_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let msaa_depth = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opencat-three-transmission-msaa-depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: TRANSMISSION_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let base_view = tex.create_view(&wgpu::TextureViewDescriptor {
                label: Some("opencat-three-transmission-base"),
                mip_level_count: Some(1),
                ..Default::default()
            });
            let sample_view = tex.create_view(&Default::default());
            let msaa_view = msaa_tex.create_view(&Default::default());
            self.transmission = Some(TransmissionRt {
                width,
                height,
                mip_count,
                texture: tex,
                msaa_texture: msaa_tex,
                msaa_depth,
                base_view,
                sample_view,
                msaa_view,
            });
        }
        self.transmission
            .as_ref()
            .map(|rt| rt.msaa_view.clone())
            .unwrap_or_else(|| unreachable!("ensure_transmission 刚写入了 transmission"))
    }

    /// mip 链生成（WebGLTextures.updateRenderTargetMipmap →
    /// glGenerateMipmap 的对应物；WebGLRenderer.js:1942 RT pass 后调用）。
    /// wgpu 无内建 mipmap 生成：逐级把 level n−1 线性降采样 blit 进
    /// level n（WebGL2 glGenerateMipmap 对任意尺寸的同构实现）。
    fn generate_transmission_mips(&self, encoder: &mut wgpu::CommandEncoder) {
        let rt = match &self.transmission {
            Some(rt) => rt,
            None => return,
        };
        for level in 1..rt.mip_count {
            let (dw, dh) = mip_dims(rt.width, rt.height, level);
            let src_view = rt.texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("opencat-three-transmission-mip-src"),
                base_mip_level: level - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let dst_view = rt.texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("opencat-three-transmission-mip-dst"),
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("opencat-three-transmission-mip-bg"),
                layout: &self.mipgen_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.clamp_sampler),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("opencat-three-transmission-mip"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(0.0, 0.0, dw as f32, dh as f32, 0.0, 1.0);
            pass.set_pipeline(&self.mipgen_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// mip 层级尺寸（WebGL Mipmap 纹理布局：max(1, dim >> level)）。
fn mip_dims(width: u32, height: u32, level: u32) -> (u32, u32) {
    (
        (width >> level).max(1),
        (height >> level).max(1),
    )
}

/// 透射 RT 的 MSAA 采样数（WebGLRenderer.js:1880 `samples: 4`）。
const TRANSMISSION_SAMPLE_COUNT: u32 = 4;
/// 主 pass（默认帧缓冲）的 MSAA 采样数。参考 renderer 构造
/// `antialias: true`（js/liquid-pop-v2.js:20）→ WebGL2 默认帧缓冲带
/// 多重采样（ANGLE/SwiftShader 实现 = 4×），three.js 主场景直接渲染进
/// 抗锯齿 drawing buffer；wgpu 对应物 = 4× 附件 + resolve 到 1× 读回
/// 纹理。棱柱 corner 色细条带 / 轮廓虹彩带 / droplets 轮廓是该语义的
/// 主要受益面。
const MAIN_SAMPLE_COUNT: u32 = 4;

fn uniform_layout(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// 纹理槽 bind group 布局：MAX_TEXTURE_SLOTS × sampled texture（槽→绑定
/// 映射见 [`slot_binding`]）+ 共享 filtering sampler（binding 8，视频槽
/// Repeat 语义）+ 透射场景色缓冲（binding 9）+ Clamp 采样器（binding 10）
/// + DFG LUT（binding 11）。
fn texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let mut entries = Vec::with_capacity(MAX_TEXTURE_SLOTS + 4);
    for i in 0..MAX_TEXTURE_SLOTS {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: slot_binding(i),
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
    }
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 8, // 共享 Repeat 采样器（WGSL @group(2) @binding(8) var smp）
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    });
    for binding in [9u32, 11u32] {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
    }
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 10,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    });
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 44, // 场景环境 PMREM cubeUV（Rgba16Float 可过滤）
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    });
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("opencat-three-textures"),
        entries: &entries,
    })
}

const NO_MAP_SLOT: u32 = 0xFFFF_FFFF;

// ══ 脚本着色器（three.js ShaderMaterial）的通用编译桥 ═══════════════════
// GLSL（shim 适配后的 450 core，见 three_api.js adaptGlsl——它扮演
// three.js WebGLProgram 的前缀拼接角色）→ naga IR → WGSL 文本 → wgpu
// 模块。只允许 uniform buffer（WebGL2 硬约束）。布局由 shim 固定：
//   set 0 = ThreeBuiltins（three.js 逐 program 前缀 uniforms 的对应物，
//          两 stage 同一布局，保证同一 buffer 绑定对两 stage 都成立）
//   set 1 = UserUniforms（脚本 uniforms 字典；std140，偏移由 naga 反射）
//   set 3 = map 采样（材质带 map 时适配器注入 texture2D+sampler 拆分
//          声明，naga 30 glsl-in 无组合 sampler2D 类型——方言测试
//          shader_map_seam_splits_texture_and_sampler_at_set3 是防漂移锚）。

/// 每绘制三方 uniform：proj / modelView / model / view / cameraPosition。
/// 列主序 mat4 ×4 + vec4 1 个 = 272 B（std140：mat4 对齐 16，vec3 成员
/// 升 vec4 槽）。
const SHADER_BUILTINS_SIZE: u64 = 272;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShaderBuiltinsUniform {
    proj: [[f32; 4]; 4],
    model_view: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    camera_pos: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<ShaderBuiltinsUniform>() as u64 == SHADER_BUILTINS_SIZE);

/// 一次 GLSL → naga → WGSL 的编译（stage 由调用方定；错误带 stage 上下文）。
/// 返回 (naga 模块, WGSL 文本)——模块供 uniform 块反射。
fn compile_glsl(stage: naga::ShaderStage, source: &str) -> Result<(naga::Module, String)> {
    let mut frontend = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options {
        stage,
        defines: Default::default(),
    };
    let module = frontend.parse(&options, source).map_err(|e| {
        anyhow::anyhow!("ShaderMaterial {stage:?} GLSL 解析失败:\n{e}")
    })?;
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    let info = validator
        .validate(&module)
        .map_err(|e| anyhow::anyhow!("ShaderMaterial {stage:?} 校验失败:\n{e}"))?;
    let text = naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| anyhow::anyhow!("ShaderMaterial {stage:?} WGSL 输出失败: {e}"))?;
    Ok((module, text))
}

/// 共用的渲染管线构造（主管线与脚本着色器管线同一顶点布局/目标/深度态；
/// 入口名可变——主 WGSL 是 vs/fs，naga 产物是 main）。`blend`：None =
/// three.js NormalBlending 混合态（WebGLState.setMaterial，
/// premultipliedAlpha:false）= blendFuncSeparate(SRC_ALPHA,
/// ONE_MINUS_SRC_ALPHA, ONE, ONE_MINUS_SRC_ALPHA)。wgpu 30 的
/// BlendState::ALPHA_BLENDING 恰好同构：color = (SrcAlpha,
/// OneMinusSrcAlpha)，alpha 通道 = BlendComponent::OVER =
/// (One, OneMinusSrcAlpha)——alpha 的 src 因子是 ONE 不是 SrcAlpha，
/// 清屏透明的目标上产物为预乘内容（rgb = a·color），半覆盖边缘叠在
/// 已写像素上 alpha 保持 1，不会腐蚀。
/// 共用的渲染管线构造（主管线与脚本着色器管线同一顶点布局/目标/深度态；
/// 入口名可变——主 WGSL 是 vs/fs，naga 产物是 main）。`blend`：None =
/// 替换写（three.js 不透明列表 NormalBlending+transparent:false →
/// NoBlending，WebGLState.setMaterial）；Some = NormalBlending
/// src-alpha（透射/透明列表，同 setMaterial + Material.premultipliedAlpha
/// 默认 false）。
/// `sample_count`/`target_format`：主 pass 走 (1, Rgba8Unorm)，透射 RT
/// pass 走 (TRANSMISSION_SAMPLE_COUNT, Rgba16Float)——附件采样数必须与
/// 管线 multisample.count 一致（wgpu 验证）。
fn build_render_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    vs_module: &wgpu::ShaderModule,
    vs_entry: &str,
    fs_module: &wgpu::ShaderModule,
    fs_entry: &str,
    blend: Option<wgpu::BlendState>,
    sample_count: u32,
    target_format: wgpu::TextureFormat,
    depth_write: bool,
) -> wgpu::RenderPipeline {

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: vs_module,
            entry_point: Some(vs_entry),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                // interleaved: pos(3f32) + normal(3f32) + uv(2f32)
                //              + color0(4f32) + color1(4f32)
                array_stride: 64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 24,
                        shader_location: 2,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 3,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 48,
                        shader_location: 4,
                    },
                ],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: fs_module,
            entry_point: Some(fs_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        // ponytail: 不做背面剔除 —— GLB 环向未知，先靠深度测试；GLB 接入时按材质翻面
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(depth_write),
            // r147 WebGLState.setFunc 缺省 LessEqualDepth → gl.LEQUAL。
            // 用 Less 会丢弃与先画者同深度的像素：iPhone GLB 里共面/复制的
            // 网格（logo、镜头环、玻璃盖板都各自有同深度孪生面）会被挖成洞
            // ——像素上表现为背景穿透成的紫斑。
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

/// 从 naga 模块反射 uniform 块：(group, binding) → (块 span, 成员表)。
/// 成员 = (名字, 偏移, f32 分量数 1/2/3/4)；非 f32 标量/向量成员是明确
/// 错误（shim 值域只有 Float/Vec2/3/4 —— 见 ShaderValue3）。
fn reflect_uniform_block(
    module: &naga::Module,
    stage: naga::ShaderStage,
    group: u32,
    binding: u32,
) -> Result<Option<(u32, Vec<(String, u32, u32)>)>> {
    use naga::AddressSpace;
    let mut found: Option<(u32, Vec<(String, u32, u32)>)> = None;
    for (_, var) in module.global_variables.iter() {
        let rb = match (var.space, var.binding) {
            (AddressSpace::Uniform, Some(rb)) => rb,
            _ => continue,
        };
        if rb.group > 1 || rb.binding != 0 {
            bail!(
                "ShaderMaterial {stage:?} 出现未预期的 uniform 块 set={} binding={}（本渲染器只接受 set 0/1 binding 0）",
                rb.group,
                rb.binding
            );
        }
        if rb.group != group {
            // 同模块里的另一个合法块（片元段同时带 set0 前缀 + set1 脚本
            // 块）——本次查询跳过，由另一次 (group,binding) 查询覆盖。
            continue;
        }
        let ty = &module.types[var.ty];
        let (members, span) = match &ty.inner {
            naga::TypeInner::Struct { members, span } => (members, *span),
            other => bail!(
                "ShaderMaterial {stage:?} uniform 块必须是 struct，得到 {other:?}"
            ),
        };
        let mut reflected = Vec::with_capacity(members.len());
        for m in members {
            let width = match &module.types[m.ty].inner {
                naga::TypeInner::Scalar(s) if s == &naga::Scalar::F32 => 1u32,
                naga::TypeInner::Vector { size, scalar } if scalar == &naga::Scalar::F32 => {
                    *size as u32
                }
                // f32 矩阵（ThreeBuiltins 的 mat4）：width = 分量总数；打包
                // 由引擎直填（脚本值域仍是 Float/VecN——write_uniform_values
                // 的分量数检查会拒掉脚本侧 mat4 uniform）。
                naga::TypeInner::Matrix { columns, rows, scalar }
                    if scalar == &naga::Scalar::F32 =>
                {
                    *columns as u32 * *rows as u32
                }
                other => bail!(
                    "ShaderMaterial uniform 成员 {:?} 类型 {other:?} 不受支持（值域 Float/Vec2/Vec3/Vec4/mat4）",
                    m.name
                ),
            };
            reflected.push((m.name.clone().unwrap_or_default(), m.offset, width));
        }
        match &found {
            Some((prev_span, _)) if *prev_span != span => bail!(
                "ShaderMaterial 两 stage 的 uniform 块布局不一致（span {prev_span} vs {span}）"
            ),
            Some((_, prev_members)) if *prev_members != reflected => bail!(
                "ShaderMaterial 两 stage 的 uniform 块成员不一致（{prev_members:?} vs {reflected:?}）"
            ),
            _ => found = Some((span, reflected)),
        }
    }
    Ok(found)
}

/// 一个已编译的脚本着色器程序（按 (vertex, fragment) 源缓存）。
struct ShaderProgram {
    /// 主 pass 管线（4×MSAA + Rgba8Unorm，MAIN_SAMPLE_COUNT 注释）。
    pipeline: wgpu::RenderPipeline,
    /// 透射/透明列表的 src-alpha 混合变体（同一布局与模块）。
    pipeline_blended: wgpu::RenderPipeline,
    /// 透射 RT pass 的 4×MSAA + Rgba16Float 附件变体（附件采样数 =
    /// 管线 multisample.count 是 wgpu 验证约束）。
    pipeline_rt: wgpu::RenderPipeline,
    pipeline_blended_rt: wgpu::RenderPipeline,
    /// set 0 三方块布局（每绘制 272 B buffer）。
    builtins_bgl: wgpu::BindGroupLayout,
    /// set 1 脚本 uniforms 布局（脚本未声明 uniform 时为 None）。
    user_bgl: Option<wgpu::BindGroupLayout>,
    /// set 3 map 采样布局（材质带 `map` 时为 Some——naga 反射到片元段
    /// 的 group 3 资源；None 时管线布局对应槽位为洞）。
    map_bgl: Option<wgpu::BindGroupLayout>,
    user_size: u32,
    /// (名字, std140 偏移, f32 分量数)。
    user_members: Vec<(String, u32, u32)>,
}

/// 脚本材质的 uniform 值表（非 Shader 材质为空表）。
fn shader_uniforms(material: &Material3) -> &[ShaderUniform3] {
    match &material.shading {
        Shading3::Shader(s) => &s.uniforms,
        _ => &[],
    }
}

/// 脚本 uniforms → std140 字节（按反射偏移；快照未提供的名字保持 0，
/// 与 GL 默认 uniform 值 = 0 的语义一致；名字未在着色器中出现则跳过——
/// three.js 对未知 uniform 同样不设 location）。
fn pack_user_uniforms(
    program: &ShaderProgram,
    uniforms: &[ShaderUniform3],
) -> Result<Vec<u8>> {
    write_uniform_values(program.user_size, &program.user_members, uniforms)
}

/// 纯数据打包核心（pack_user_uniforms 与方言测试共用；不触 wgpu 句柄）。
fn write_uniform_values(
    user_size: u32,
    user_members: &[(String, u32, u32)],
    uniforms: &[ShaderUniform3],
) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; user_size as usize];
    for u in uniforms {
        let Some((_, offset, width)) = user_members
            .iter()
            .find(|(n, _, _)| *n == u.name)
            .cloned()
        else {
            continue;
        };
        let comps: &[f32] = match (&u.value, width) {
            (ShaderValue3::Float(v), 1) => std::slice::from_ref(v),
            (ShaderValue3::Vec2(v), 2) => v.as_slice(),
            (ShaderValue3::Vec3(v), 3) => v.as_slice(),
            (ShaderValue3::Vec4(v), 4) => v.as_slice(),
            (value, w) => bail!(
                "ShaderMaterial uniform '{}' 分量数不匹配：着色器要 {w}，快照给 {value:?}",
                u.name
            ),
        };
        let start = offset as usize;
        for (i, c) in comps.iter().enumerate() {
            buf[start + i * 4..start + i * 4 + 4].copy_from_slice(&c.to_le_bytes());
        }
    }
    Ok(buf)
}

fn object_uniform(
    material: &Material3,
    matrix: &[f32; 16],
    map_slot: Option<u32>,
    pbr_slots: (Option<u32>, Option<u32>, Option<u32>),
) -> ObjectUniform {
    let model = Mat4::from_cols_array(matrix);
    // 逆转置：非均匀缩放下法线保持垂直于表面。
    let n3 = Mat3::from_mat4(model).inverse().transpose();
    let n = n3.to_cols_array_2d();
    let mosaic = match material.shading {
        Shading3::VideoMosaic(m) => Some(m),
        _ => None,
    };
    let physical = match material.shading {
        Shading3::Physical(p) => Some(p),
        _ => None,
    };
    let face_to_texture = mosaic.map(|m| m.face_to_texture).unwrap_or([0u32; 8]);
    let corner_rule = mosaic.map(|m| m.corner_rule).unwrap_or([[0u32; 2]; 8]);
    let corner_colors = mosaic.map(|m| m.corner_colors).unwrap_or([[0.0f32; 3]; 3]);
    let pack8 = |v: [u32; 8]| [[v[0], v[1], v[2], v[3]], [v[4], v[5], v[6], v[7]]];
    ObjectUniform {
        model: model.to_cols_array_2d(),
        normal_mat: [
            [n[0][0], n[0][1], n[0][2], 0.0],
            [n[1][0], n[1][1], n[1][2], 0.0],
            [n[2][0], n[2][1], n[2][2], 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        base_color: material.base_color,
        flags: [
            match material.shading {
                Shading3::Unlit => 0u32,
                Shading3::Lambert => 1u32,
                Shading3::VideoMosaic(_) => 2u32,
                Shading3::Physical(_) => 3u32,
                // 主管线 WGSL 不含该分支——Shader 绘制走独立 pipeline。
                Shading3::Shader(_) => 4u32,
            },
            0,
            0,
            0,
        ],
        disp_params: match &material.displacement {
            Some(d) => [d.amplitude, d.frequency, d.world_frequency, d.time_speed],
            None => [0.0; 4],
        },
        disp_time: match &material.displacement {
            Some(d) => [d.time, 0.0, 0.0, 0.0],
            None => [0.0; 4],
        },
        tex: [map_slot.unwrap_or(NO_MAP_SLOT), 0, 0, 0],
        face_to_texture: pack8(face_to_texture),
        corner_a: pack8([
            corner_rule[0][0], corner_rule[1][0], corner_rule[2][0], corner_rule[3][0],
            corner_rule[4][0], corner_rule[5][0], corner_rule[6][0], corner_rule[7][0],
        ]),
        corner_inv: pack8([
            corner_rule[0][1], corner_rule[1][1], corner_rule[2][1], corner_rule[3][1],
            corner_rule[4][1], corner_rule[5][1], corner_rule[6][1], corner_rule[7][1],
        ]),
        corner_colors: [
            [
                corner_colors[0][0],
                corner_colors[0][1],
                corner_colors[0][2],
                0.0,
            ],
            [
                corner_colors[1][0],
                corner_colors[1][1],
                corner_colors[1][2],
                0.0,
            ],
            [
                corner_colors[2][0],
                corner_colors[2][1],
                corner_colors[2][2],
                0.0,
            ],
        ],
        mosaic: mosaic.map(|m| [m.frost, m.uv_crop, 0.0, 0.0]).unwrap_or([0.0; 4]),
        phys_a: physical
            .map(|p| [p.ior, p.thickness, p.attenuation_distance, p.dispersion])
            .unwrap_or([0.0; 4]),
        phys_b: physical
            .map(|p| [p.transmission, p.roughness, p.metalness, p.specular_intensity])
            .unwrap_or([0.0; 4]),
        phys_c: physical
            .map(|p| [p.clearcoat, p.clearcoat_roughness, 0.0, 0.0])
            .unwrap_or([0.0; 4]),
        attenuation_color: physical
            .map(|p| [p.attenuation_color[0], p.attenuation_color[1], p.attenuation_color[2], 0.0])
            .unwrap_or([0.0; 4]),
        phys_d: physical
            .map(|p| [p.env_map_intensity, 0.0, 0.0, 0.0])
            .unwrap_or([0.0; 4]),
        pbr_tex: [
            pbr_slots.0.unwrap_or(NO_MAP_SLOT),
            pbr_slots.1.unwrap_or(NO_MAP_SLOT),
            0,
            0,
        ],
        normal_tex: [pbr_slots.2.unwrap_or(NO_MAP_SLOT), 0, 0, 0],
    }
}

/// [`Scene3Raster`] 的 opencat-three 实现（宿主注入 core realm 用）。
///
/// wgpu 设备**懒初始化**：首个 3D 场景出现才请求适配器，无 3D 的合成
/// 不付 GPU 初始化成本；初始化失败（无 GPU/驱动）只在该场景首次渲染时
/// 报错，不影响同进程其他管线。内部 `Mutex`：core 的渲染流单线程调用，
/// 加锁只为满足 `Scene3Raster: Send + Sync`。
pub struct LazyRenderer3d {
    inner: OnceLock<Result<Mutex<Renderer3d>, String>>,
    /// 共享纹理登记处。GPU 输出纹理的所有权交给它——本结构不再自持存活表。
    registry: Arc<TextureRegistry>,
}

impl LazyRenderer3d {
    /// 用一个共享纹理登记处建光栅化器。宿主与消费方各持一份 `Arc`。
    pub fn new(registry: Arc<TextureRegistry>) -> Self {
        Self {
            inner: OnceLock::new(),
            registry,
        }
    }

    /// 本光栅化器使用的共享纹理登记处。
    pub fn registry(&self) -> &Arc<TextureRegistry> {
        &self.registry
    }

    /// 立刻初始化 wgpu 渲染器（幂等）。
    ///
    /// GPU 合成器需要在**渲染任何帧之前**建好合成目标纹理，因此不能等
    /// 首个 3D 场景来懒初始化。无 3D 的合成也能安全调用。
    pub fn ensure_initialized(&self) -> Result<()> {
        let init = self.inner.get_or_init(|| {
            pollster::block_on(Renderer3d::new())
                .map(Mutex::new)
                .map_err(|e| format!("{e:#}"))
        });
        match init {
            Ok(_) => Ok(()),
            Err(msg) => bail!("3D 光栅化器初始化失败: {msg}"),
        }
    }

    /// 预置一个**已建好**的渲染器，取代懒初始化时自建的那个。
    ///
    /// 预览器（`opencat-see`）用它在窗口 GL 上下文就绪后注入
    /// [`Renderer3d::new_external`] 的产物——这样 wgpu 与窗口 Skia 共享
    /// 同一 `HGLRC`，纹理零拷贝。必须在首次渲染前调用（`OnceLock` 只设一次）。
    pub fn preset(&self, renderer: Renderer3d) -> Result<()> {
        self.inner
            .set(Ok(Mutex::new(renderer)))
            .map_err(|_| anyhow::anyhow!("3D 光栅化器已初始化，无法预置"))
    }

    /// GPU 模式下渲染到显存并登记到共享纹理表（不读回 CPU）。
    ///
    /// 返回的句柄是 [`scene3::Rasterized3d::gpu`] 的值。纹理的所有权交给
    /// `registry`——本渲染器不再持表。宿主消费完必须
    /// [`opencat_gpu_native::TextureRegistry::release`]。
    pub fn render_to_gpu(
        &self,
        scene: &Scene3,
        geometries: &scene3::GeometryRegistry,
        textures: &TextureTable3,
        width: u32,
        height: u32,
    ) -> Result<opencat_core::ir::generated_image::GpuImageHandle> {
        let init = self.inner.get_or_init(|| {
            pollster::block_on(Renderer3d::new())
                .map(Mutex::new)
                .map_err(|e| format!("{e:#}"))
        });
        let tex = match init {
            Ok(renderer) => {
                let mut r = renderer.lock().expect("opencat-three renderer mutex poisoned");
                r.render_to_texture(scene, geometries, textures, width, height)
                    .map_err(|e| anyhow::anyhow!("{e:#}"))?
            }
            Err(msg) => bail!("3D 光栅化器初始化失败: {msg}"),
        };
        Ok(self.publish(tex))
    }

    /// 把一张 wgpu 纹理登记到共享表，返回宿主句柄。
    ///
    /// 纹理必须活到消费方采样完成——`Renderer3d` 的 `Mutex` 释放后无人
    /// 持有它，所以把所有权凭据交给登记处保管。
    ///
    /// GL 后端：登记原生纹理名，消费方可零拷贝导入。
    /// 其他后端：登记空句柄，消费方按 `rgba` 走。
    fn publish(&self, tex: wgpu::Texture) -> opencat_core::ir::generated_image::GpuImageHandle {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let native = Renderer3d::output_native_texture(&tex).unwrap_or(NativeTexture::NONE);
        #[cfg(target_os = "macos")]
        let native = Renderer3d::output_native_texture_metal(&tex).unwrap_or(NativeTexture::NONE);
        #[cfg(not(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "macos"
        )))]
        let native = NativeTexture::NONE;

        let id = self.registry.publish(native, Box::new(tex));
        opencat_core::ir::generated_image::GpuImageHandle { token: id.raw() }
    }

    /// 建一张合成目标纹理并登记到共享表。
    ///
    /// 在**锁外**调用（创建纹理是 wgpu 操作）。纹理所有权归共享登记处。
    /// Metal 与 GL 后端都有效。
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub fn make_composite_target(
        &self,
        width: u32,
        height: u32,
    ) -> Option<opencat_core::ir::generated_image::GpuImageHandle> {
        self.with_renderer(|r| r.create_composite_target(width, height))
            .flatten()
            .map(|tex| self.publish(tex))
    }

    /// 在 wgpu 的 GL context 已 make current 的前提下运行 `f`。
    ///
    /// 仅 GL 后端有效。**`f` 内只做 Skia 操作**——任何 wgpu 调用都会二次
    /// 加锁，1 秒后 panic（实测确认）。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn with_context<R>(&self, f: impl FnOnce() -> R) -> Option<R> {
        let init = self.inner.get()?;
        let Ok(renderer) = init else { return None };
        let r = renderer.lock().expect("opencat-three renderer mutex poisoned");
        r.with_gl_context(f)
    }

    /// 只取渲染器外层锁（**不加 GL context 锁**），用于建 wgpu 资源。
    ///
    /// wgpu 的每次调用都会自己加/放 GL context 锁，所以这里只串行化对
    /// `Renderer3d` 的访问。**`f` 内不得做 Skia 操作**（context 未 make
    /// current）。
    /// 当前后端是否为 Metal（macOS 的 GPU 合成路径）。
    #[cfg(target_os = "macos")]
    pub fn is_metal(&self) -> bool {
        self.with_renderer(|r| r.is_metal()).unwrap_or(false)
    }

    pub fn with_renderer<R>(&self, f: impl FnOnce(&Renderer3d) -> R) -> Option<R> {
        let init = self.inner.get()?;
        let Ok(renderer) = init else { return None };
        let r = renderer.lock().expect("opencat-three renderer mutex poisoned");
        Some(f(&r))
    }

    /// GL 后端是否可用（合成器据此决定能否走 GPU 路径）。
    pub fn is_gles(&self) -> bool {
        self.with_renderer(|r| r.is_gles()).unwrap_or(false)
    }
}

impl scene3::Scene3Raster for LazyRenderer3d {
    fn render(
        &self,
        scene: &Scene3,
        geometries: &scene3::GeometryRegistry,
        width: u32,
        height: u32,
    ) -> Result<scene3::Rasterized3d> {
        Ok(scene3::Rasterized3d::cpu(self.render_with_textures(
            scene,
            geometries,
            &TextureTable3::new(),
            width,
            height,
        )?))
    }
}

impl LazyRenderer3d {
    /// 场景环境预计算入口（锁内转发 [`Renderer3d::set_environment`]）。
    /// 逐帧调用幂等：env 位图来自宿主图像缓存的同一 Arc ⇒ 同指针直接跳过。
    pub fn set_environment(&self, env: &TextureBitmap3, srgb: bool) -> Result<(), String> {
        let init = self.inner.get_or_init(|| {
            pollster::block_on(Renderer3d::new())
                .map(Mutex::new)
                .map_err(|e| format!("{e:#}"))
        });
        match init {
            Ok(renderer) => renderer
                .lock()
                .expect("opencat-three renderer mutex poisoned")
                .set_environment(env, srgb)
                .map_err(|e| format!("{e:#}")),
            Err(msg) => Err(msg.clone()),
        }
    }

    /// 带纹理表的渲染（宿主解析 [`scene3::Texture3`] 来源后调用；无纹理的
    /// 宿主走 [`scene3::Scene3Raster`] 即可）。与 trait 同语义，只是显式
    /// 携带本帧纹理表。
    pub fn render_with_textures(
        &self,
        scene: &Scene3,
        geometries: &scene3::GeometryRegistry,
        textures: &TextureTable3,
        width: u32,
        height: u32,
    ) -> Result<std::sync::Arc<[u8]>> {
        let init = self.inner.get_or_init(|| {
            pollster::block_on(Renderer3d::new())
                .map(Mutex::new)
                .map_err(|e| format!("{e:#}"))
        });
        match init {
            Ok(renderer) => {
                let rgba = renderer
                    .lock()
                    .expect("opencat-three renderer mutex poisoned")
                    .render(scene, geometries, textures, width, height)?;
                Ok(std::sync::Arc::from(rgba))
            }
            Err(msg) => bail!("3D 光栅化器初始化失败: {msg}"),
        }
    }
}

#[cfg(test)]
mod mip_dims_tests {
    //! mip 层级尺寸（generate_transmission_mips 的视口/采样尺寸来源）。
    #[test]
    fn mip_dims_halve_with_floor_at_one() {
        use super::mip_dims;
        // 1920×1080：mip_count = floor(log2(1920))+1 = 11；逐级 max(1,>>)。
        assert_eq!(mip_dims(1920, 1080, 0), (1920, 1080));
        assert_eq!(mip_dims(1920, 1080, 1), (960, 540));
        assert_eq!(mip_dims(1920, 1080, 10), (1, 1));
        // 奇数尺寸下取整到 1 不回弹。
        assert_eq!(mip_dims(15, 7, 3), (1, 1));
        assert_eq!(mip_dims(15, 7, 0), (15, 7));
    }
}

#[cfg(test)]
mod shader_glsl_tests {
    //! GLSL→naga→WGSL 桥的方言测试（无 GPU；GPU 侧烟测在 examples/cube.rs）。
    //! 源 = three_api.js adaptGlsl 对参考背景 ShaderMaterial（js:62-97）
    //! 的产物形状：450 core、precision、TwoBlock 前缀、location 显式化。

    use super::*;

    // three_api.js adaptGlsl 注入的固定前缀（与 JS 逐字同源——改动必须
    // 两边同步，这里就是防漂移锚）。
    const THREE_BUILTINS_BLOCK: &str = r#"
layout(std140, set = 0, binding = 0) uniform ThreeBuiltins {
  mat4 projectionMatrix;
  mat4 modelViewMatrix;
  mat4 modelMatrix;
  mat4 viewMatrix;
  vec3 cameraPosition;
};"#;

    fn vertex_source() -> String {
        format!(
            "#version 450 core\nprecision highp float;\n{THREE_BUILTINS_BLOCK}\n\
             layout(location = 0) in vec3 position;\n\
             layout(location = 1) in vec3 normal;\n\
             layout(location = 2) in vec2 uv;\n\
             layout(location = 0) out vec2 vUv;\n\
             void main() {{\n  vUv = uv;\n  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);\n}}\n"
        )
    }

    // 参考片元 shader（js:80-97 逐字，gl_FragColor→fragOutput）。
    fn fragment_source() -> String {
        format!(
            "#version 450 core\nprecision highp float;\n{THREE_BUILTINS_BLOCK}\n\
             layout(std140, set = 1, binding = 0) uniform UserUniforms {{\n  float uFade;\n}};\n\
             layout(location = 0) in vec2 vUv;\n\
             layout(location = 0) out vec4 fragOutput;\n\n\
             float field(vec2 uv, vec2 center, vec2 spread) {{\n  vec2 d = (uv - center) / spread;\n  return exp(-dot(d, d) * 1.7);\n}}\n\n\
             void main() {{\n  vec3 cream = vec3(0.957, 0.949, 0.918);\n  vec3 sage = vec3(0.839, 0.898, 0.761);\n  vec3 teal = vec3(0.494, 0.714, 0.643);\n  vec3 mauve = vec3(0.773, 0.588, 0.643);\n  vec3 color = cream;\n  color = mix(color, sage, field(vUv, vec2(0.18, 0.64), vec2(0.42, 0.52)) * 0.72);\n  color = mix(color, teal, field(vUv, vec2(0.78, 0.72), vec2(0.36, 0.46)) * 0.58);\n  color = mix(color, mauve, field(vUv, vec2(0.72, 0.22), vec2(0.38, 0.38)) * 0.54);\n  color *= 0.985 + 0.035 * vUv.y;\n  fragOutput = vec4(mix(cream, color, uFade), 1.0);\n}}\n"
        )
    }

    #[test]
    fn reference_background_glsl_compiles_through_naga() {
        let (vs_module, vs_wgsl) =
            compile_glsl(naga::ShaderStage::Vertex, &vertex_source())
                .expect("顶点段应通过 naga glsl-in");
        let (fs_module, fs_wgsl) =
            compile_glsl(naga::ShaderStage::Fragment, &fragment_source())
                .expect("片元段应通过 naga glsl-in");
        assert!(vs_wgsl.contains("main"), "WGSL 保留 main 入口");
        assert!(fs_wgsl.contains("uFade"), "uniform 成员名保留");
        // set 0 三方块：两 stage 同布局 = 前缀规范（272 B、五成员）。
        let canonical: Vec<(String, u32, u32)> = [
            // width = f32 分量数（mat4 → 16；cameraPosition 升 vec4 槽但
            // 分量数 3——std140 偏移 256，span 272）。
            ("projectionMatrix", 0u32, 16u32),
            ("modelViewMatrix", 64, 16),
            ("modelMatrix", 128, 16),
            ("viewMatrix", 192, 16),
            ("cameraPosition", 256, 3),
        ]
        .iter()
        .map(|(n, o, w)| (n.to_string(), *o, *w))
        .collect();
        for (stage, m) in [
            (naga::ShaderStage::Vertex, &vs_module),
            (naga::ShaderStage::Fragment, &fs_module),
        ] {
            let (span, members) = reflect_uniform_block(m, stage, 0, 0)
                .expect("反射不出错")
                .expect("ThreeBuiltins 块存在");
            assert_eq!(span as u64, SHADER_BUILTINS_SIZE, "std140 span");
            assert_eq!(members, canonical, "前缀成员表/偏移");
        }
        // set 1：uFade @0，单 float 成员。naga 的 span 不含 std140 尾部
        // 填充（报 4）；引擎侧按 16 对齐补齐（shader_program 的 div_ceil）。
        let (span, members) = reflect_uniform_block(&fs_module, naga::ShaderStage::Fragment, 1, 0)
            .expect("反射不出错")
            .expect("UserUniforms 块存在");
        assert_eq!(span, 4, "naga span = 成员字节，不含尾部填充");
        assert_eq!(members, vec![("uFade".to_string(), 0u32, 1u32)]);
        // 顶点段没有 UserUniforms → None（引擎按 union 建布局）。
        assert!(reflect_uniform_block(&vs_module, naga::ShaderStage::Vertex, 1, 0)
            .expect("反射不出错")
            .is_none());
    }

    #[test]
    fn packed_user_uniforms_write_at_reflected_offsets() {
        let bytes = write_uniform_values(
            32,
            &[
                ("uFade".to_string(), 0, 1),
                ("uTint".to_string(), 16, 3),
            ],
            &[
                ShaderUniform3 {
                    name: "uFade".to_string(),
                    value: ShaderValue3::Float(0.25),
                },
                ShaderUniform3 {
                    name: "uTint".to_string(),
                    value: ShaderValue3::Vec3([0.5, 0.25, 0.125]),
                },
                ShaderUniform3 {
                    name: "uMissing".to_string(), // 不在块里 → 跳过
                    value: ShaderValue3::Float(9.0),
                },
            ],
        )
        .expect("打包成功");
        assert_eq!(bytes.len(), 32);
        assert_eq!(f32::from_le_bytes(bytes[0..4].try_into().unwrap()), 0.25);
        assert_eq!(f32::from_le_bytes(bytes[16..20].try_into().unwrap()), 0.5);
        assert_eq!(f32::from_le_bytes(bytes[24..28].try_into().unwrap()), 0.125);
        assert_eq!(&bytes[28..32], &[0, 0, 0, 0], "vec3 填充位保持 0");
    }

    #[test]
    fn uniform_arity_mismatch_is_rejected() {
        let err = write_uniform_values(
            16,
            &[("uFade".to_string(), 0, 1)],
            &[ShaderUniform3 {
                name: "uFade".to_string(),
                value: ShaderValue3::Vec3([1.0, 2.0, 3.0]),
            }],
        )
        .expect_err("分量数不匹配必须报错");
        assert!(err.to_string().contains("分量数不匹配"), "got: {err}");
    }

    // map 采样接缝的方言锚（r7）：naga 30 glsl-in 没有 `sampler2D` 组合
    // 类型——standalone 采样走 Vulkan 拆分形态 `texture2D` + `sampler`
    // （组合类型名会被当未知 Identifier 报 NotImplemented("variable
    // qualifier")），采样调用是组合构造
    // `texture(sampler2D(tex, smp), uv)`。适配器（three_api.js）按同一条
    // 契约在材质带 map 时注入 set 3 的两个声明；本测试把绑定号钉死。
    #[test]
    fn shader_map_seam_splits_texture_and_sampler_at_set3() {
        let src = "#version 450 core\nprecision highp float;\n\
layout(set = 3, binding = 0) uniform texture2D map;\n\
layout(set = 3, binding = 1) uniform sampler map_smp;\n\
layout(location = 0) in vec2 vUv;\n\
layout(location = 0) out vec4 fragOutput;\n\
void main() { fragOutput = texture(sampler2D(map, map_smp), vUv); }\n";
        let (module, wgsl) =
            compile_glsl(naga::ShaderStage::Fragment, src).expect("编译应通过");
        let bindings: Vec<(u32, u32)> = module
            .global_variables
            .iter()
            .filter_map(|(_, var)| {
                if var.space == naga::AddressSpace::Handle {
                    var.binding.map(|rb| (rb.group, rb.binding))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            bindings,
            vec![(3, 0), (3, 1)],
            "map 纹理/采样器必须钉在 set 3 binding 0/1"
        );
        assert!(wgsl.contains("textureSample(map, map_smp"), "WGSL 采样形态: {wgsl}");
    }

    // ⑥a：wordmark 片元的 colorspace_fragment（sRGBTransferOETF，mix 的
    // vec3(bool) 选择子是 three.js chunk 原形态）必须过 naga glsl-in。
    // 该面在参考片里恒 visible=false（js:494 + 提升前 pin）→ 整片渲染
    // 永不触发编译——本测试就是它的编译时锚；GLSL 与 XML wordmark
    // fragmentShader 逐字同步（改动必须两边同步）。
    #[test]
    fn wordmark_colorspace_chunk_compiles_through_naga() {
        let src = "#version 450 core
precision highp float;
layout(std140, set = 1, binding = 0) uniform UserUniforms {
  float uWordmarkReveal;
  vec3 diffuse;
  float opacity;
};
layout(set = 3, binding = 0) uniform texture2D map;
layout(set = 3, binding = 1) uniform sampler map_smp;
layout(location = 0) in vec2 vMapUv;
layout(location = 0) out vec4 fragOutput;
vec4 sRGBTransferOETF( in vec4 value ) {
  return vec4( mix( pow( value.rgb, vec3( 0.41666 ) ) * 1.055 - vec3( 0.055 ), value.rgb * 12.92, vec3( lessThanEqual( value.rgb, vec3( 0.0031308 ) ) ) ), value.a );
}
void main() {
  vec4 diffuseColor = vec4(diffuse, opacity);
  diffuseColor *= texture(sampler2D(map, map_smp), vMapUv);
  if (vMapUv.x > uWordmarkReveal) discard;
  if (diffuseColor.a < 0.01) discard;
  fragOutput = vec4(diffuseColor.rgb, diffuseColor.a);
  fragOutput = sRGBTransferOETF(fragOutput);
}
";
        let (_, wgsl) = compile_glsl(naga::ShaderStage::Fragment, src)
            .expect("wordmark colorspace chunk 应通过 naga glsl-in（mix vec3(bool) 选择子）");
        assert!(wgsl.contains("0.0031308"), "OETF 字面量保留: {wgsl}");
    }
}
