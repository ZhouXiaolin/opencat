//! Scene3 — 3D 场景快照（更高层的渲染 IR，与 2D DrawOp IR 并行）。
//!
//! 坐标系沿用 three.js 约定：右手系，Y 向上，相机看向 target。
//! 脚本运行时每帧产出一份纯数据快照，跨渲染边界分发（与 DrawOp 同地位）：
//! native engine 把段交给 `opencat-three`（wgpu）光栅化；web 把同一段交给
//! 浏览器里的真 three.js（`three-facade.ts`，钉 r181 —— 本 IR 以 three.js
//! 为语义标准，真 three.js 是其原生执行器；wgpu wasm 化降级为未来可选项，
//! `Scene3Raster` 契约保留）。像素输出为 straight-alpha RGBA8，经占位
//! `DrawOp::Image{ImageRef::Generated}`（内容寻址 id `scene3:...`）进入
//! 合成 —— 2D 指令集不新增概念。
//!
//! 这份结构就是「3D 的 IR」：未来若需要二进制编码，编码这份形状，
//! 而不是反过来改形状迁就编码。

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// 一帧的 3D 场景快照。纯数据、可序列化。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene3 {
    /// 清屏色（straight alpha，各分量 0..=1）。
    pub clear: [f32; 4],
    /// 渲染目标尺寸（px）。three.js `renderer.setSize(w, h)` 的对应物
    /// （参考源 js/liquid-pop-v2.js:24 `renderer.setSize(WIDTH, HEIGHT)`），
    /// 由脚本 shim 随快照提交；宿主光栅化按此出 framebuffer。
    pub target: [u32; 2],
    pub camera: Camera3,
    pub lights: Vec<Light3>,
    pub meshes: Vec<Mesh3>,
    /// three.js `renderer.toneMapping`（js:19；WebGLRenderer.js:262 默认
    /// NoToneMapping）。只作用于内建材质分支（Unlit/Lambert/Physical）的
    /// 主 pass 输出——ShaderMaterial 分支（bg/prism/wordmark）raw 输出，
    /// 与 three.js「tonemapping chunk 只注入内建 program」一致
    /// （WebGLProgram.js:831-833）。透射 RT pass 恒关（WebGLRenderer.js
    /// :1921-1922 透射二遍强制 NoToneMapping）。
    #[serde(default)]
    pub tone_mapping: ToneMapping3,
    /// three.js `renderer.toneMappingExposure`（js:20；WebGLRenderer.js:270
    /// 默认 1.0）。
    #[serde(default = "default_tone_mapping_exposure")]
    pub tone_mapping_exposure: f32,
    /// three.js `renderer.outputColorSpace`（js:18）。主 pass 内建材质的
    /// 输出 OETF；Linear = 既有 raw 输出（记录偏差：three.js r152+ 默认
    /// SRGBColorSpace——本引擎既有场景按 raw 契约成立，参考场景经 js:18
    /// 显式声明 Srgb 后行为与 three.js 一致）。RT pass 恒 Linear
    /// （WebGLRenderer.js:2197 `_currentRenderTarget === null` 分支）。
    #[serde(default)]
    pub output_color_space: OutputColorSpace3,
    /// three.js `scene.environment`（js:25-26）。None = 无环境（IBL 项全 0，
    /// 与 three.js 无 envMap 同值）。Some 时该纹理按
    /// `EquirectangularReflectionMapping` + SRGBColorSpace 语义处理
    /// （WebGLCubeUVMaps.js:31-33：equirect 映射的环境贴图经
    /// PMREMGenerator.fromEquirectangular 转 cubeUV RT 后供材质采样；
    /// 映射/色彩空间由脚本 shim 校验声明，快照只带来源）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<Texture3>,
}

/// three.js `renderer.toneMapping` 枚举（只实现参考用到的 ACESFilmic）。
/// WebGLRenderer.js 常量值 NoToneMapping=0 / ACESFilmicToneMapping=4；
/// 快照用变体名拼写（shim 侧 T3 常量同值）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneMapping3 {
    #[default]
    None,
    /// ACESFilmicToneMapping（tonemapping_pars_fragment.glsl.js：RRT+ODT
    /// 拟合，`color *= toneMappingExposure / 0.6`）。
    ACESFilmic,
}

/// three.js `renderer.outputColorSpace` 枚举（SRGBColorSpace='srgb' /
/// LinearSRGBColorSpace='linear-srgb'；快照用变体名拼写）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputColorSpace3 {
    #[default]
    Linear,
    /// sRGBTransferOETF（colorspace_pars_fragment.glsl.js）。
    Srgb,
}

fn default_tone_mapping_exposure() -> f32 {
    1.0
}

/// 宿主注入的 3D 光栅化器（engine = `opencat-three`/wgpu，web = 同一
/// WGSL 的 WebGPU/WebGL2 后端）。core 只依赖本 trait：同一 [`Scene3`]
/// 在同一后端上必须产出逐字节一致的 straight-alpha RGBA8（行紧凑，
/// 长度 `width*height*4`）——与 [`crate::ir::GeneratedImageEntry`] 的
/// 像素契约一致。
///
/// `geometries` 是 realm 级几何注册表（[`GeometryRegistry`]）：快照里的
/// `Geometry3::Asset { id }` 只带 id，重网格（如 prism GLB 11280 顶点）
/// 不逐帧过 JSON 桥。
pub trait Scene3Raster: Send + Sync {
    fn render(
        &self,
        scene: &Scene3,
        geometries: &GeometryRegistry,
        width: u32,
        height: u32,
    ) -> Result<Rasterized3d>;
}

/// One rasterized 3D scene.
///
/// Either `rgba` carries CPU pixels (the CPU path, and every wasm host), or
/// `gpu` carries an opaque host handle to a GPU-resident render target (the
/// GPU path — no CPU round trip). `rgba` is empty when `gpu` is set.
pub struct Rasterized3d {
    pub rgba: std::sync::Arc<[u8]>,
    pub gpu: Option<crate::ir::generated_image::GpuImageHandle>,
}

impl Rasterized3d {
    /// CPU pixels only (the default path).
    pub fn cpu(rgba: std::sync::Arc<[u8]>) -> Self {
        Self { rgba, gpu: None }
    }
}

/// 几何注册表：id → 几何。realm 级（不随帧清除），由
/// `scene3_geometry_register` binding 在脚本侧一次性填充。
pub type GeometryRegistry = std::collections::BTreeMap<String, std::sync::Arc<Geometry3>>;

/// 透视相机（three.js `PerspectiveCamera` 形状：position + lookAt target）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera3 {
    pub position: [f32; 3],
    pub target: [f32; 3],
    /// 垂直视场角（度）。
    pub fov_y_degrees: f32,
    pub near: f32,
    pub far: f32,
}

/// 光源。directional 的 `direction` 是**光传播方向**（世界系，从光源指向场景）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Light3 {
    Ambient {
        color: [f32; 3],
        intensity: f32,
    },
    /// 半球光（three.js `HemisphereLight`）：天空色/地面色按世界法线 y
    /// 分量插值（`mix(ground, sky, n.y*0.5+0.5)`）贡献漫反射辐照度，
    /// 无位置/方向——three.js 的视空间 up 与本渲染器的世界 up 在未旋转
    /// 相机下等价（参考 js:103 `HemisphereLight(0xffffff, 0x7eb6a4, 2.1)`）。
    Hemisphere {
        sky_color: [f32; 3],
        ground_color: [f32; 3],
        intensity: f32,
    },
    Directional {
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
    },
}

/// 一个网格实例：几何 + 材质 + 世界矩阵。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mesh3 {
    pub geometry: Geometry3,
    pub material: Material3,
    /// 世界矩阵，列主序（WebGPU/glam 约定，`m[12..15]` = 平移）。
    /// 层级（Group 嵌套）的级联在脚本 shim / 宿主 GLB 导入器完成，
    /// IR 只收扁平的世界矩阵。
    pub matrix: [f32; 16],
    /// 绘制排序键（three.js `Object3D.renderOrder`，可负——参考背景面
    /// js:100 `background.renderOrder = -10`）。渲染器按 three.js
    /// `painterSortStable`/`reversePainterSortStable` 的主键
    /// （renderOrder 升序，再按列表方向 z 升/降序）稳定排序。
    #[serde(default)]
    pub render_order: i32,
}

/// 几何数据。`Inline` 是脚本内建几何（box/sphere/plane 或宿主 GLB 解码
/// 后的完整数据）；`Asset { id }` 引用 realm 级几何注册表
/// （[`GeometryRegistry`]，重网格一次性注册，快照只带 id——11280 顶点
/// 的 prism GLB 不逐帧过 JSON 桥）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Geometry3 {
    Inline {
        positions: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        indices: Vec<u32>,
        /// TEXCOORD_0（可缺省；纹理采样属后续轮）。
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        uvs: Vec<[f32; 2]>,
        /// COLOR_0（可缺省）。
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        colors: Vec<[f32; 4]>,
        /// COLOR_1——glTF 规范外的第二顶点色（prism GLB 用它编码
        /// facet 视频 id，参考 shader `in vec4 color_1`）。通用属性位，
        /// 不含任何 liquid 语义。
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        colors1: Vec<[f32; 4]>,
    },
    Asset {
        id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material3 {
    /// straight alpha，0..=1。
    pub base_color: [f32; 4],
    pub shading: Shading3,
    /// 程序化顶点位移（噪声沿法线）。`None` = 不位移。公式与 three.js
    /// `onBeforeCompile` 顶点位移同构（参考 js:185-207）：切向双样本重算法线，
    /// 噪声输入 = `p*frequency + time*time_speed + world_p*world_frequency`，
    /// 位移量 = `snoise(...) * amplitude`。
    #[serde(default)]
    pub displacement: Option<NoiseDisplacement3>,
    /// 单贴图（three.js `MeshBasicMaterial({ map })` 语义：albedo = map ×
    /// base_color）。像素不进快照——快照只带**来源**（路径/视频+时间），
    /// 宿主在光栅化调用时解码并上传（与几何注册表同理，纹理不过 JSON 桥）。
    #[serde(default)]
    pub map: Option<Texture3>,
    /// [`Shading3::VideoMosaic`] 的纹理槽表（≤8，槽位 = 数组下标）。快照带
    /// 来源不带像素，同 `map`。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub textures: Vec<Texture3>,
    /// three.js `Material.transparent`：true 时该网格进透明渲染列表、
    /// 以 NormalBlending（src-alpha）绘制；`transmission > 0` 的
    /// [`Shading3::Physical`] 优先级更高（进透射列表，同样 src-alpha）——
    /// three.js `WebGLRenderLists.js:118-133 push()` 的分类规则。
    #[serde(default)]
    pub transparent: bool,
}

/// 纹理来源（纯数据，可序列化；解码与像素上传是宿主光栅化器的事）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Texture3 {
    /// 视频在合成时间 `time`（秒）的解码帧——three.js `VideoTexture`
    /// 「采样视频元素当前帧」语义的对应物。
    VideoFrame { path: String, time: f32 },
    /// 静态位图（路径；宿主一次性解码）。
    Bitmap { path: String },
}

impl Texture3 {
    /// 光栅化前的纹理表键（确定性；同帧内唯一即可——解码缓存在宿主）。
    pub fn table_key(&self) -> String {
        match self {
            Texture3::VideoFrame { path, time } => format!("videoframe:{path}@{time:.6}"),
            Texture3::Bitmap { path } => format!("bitmap:{path}"),
        }
    }
}

/// 视频-马赛克面片材质（three.js `ShaderMaterial` 的「按顶点整数 id 从 N 路
/// 纹理槽采样」通用化）：COLOR_0 ≈ 0 的三角形是主面，按 COLOR_1 字节 id 经
/// `face_to_texture` 选槽、以屏幕空间 UV 采样该槽纹理并加 frost 项；非主面
/// 取「COLOR_0 主导通道 → corner 色表」的平坦色。所有常量由脚本声明，
/// WGSL 只实现机制。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoMosaic3 {
    /// COLOR_1 字节 id（0..8）→ 纹理槽位。
    pub face_to_texture: [u32; 8],
    /// 非主面 corner 色表（按 COLOR_0 主导通道序索引 3 色）。
    pub corner_colors: [[f32; 3]; 3],
    /// 逐纹理槽位的 corner 规则：`[a, invert]` → 色表下标
    /// `select((dominant + a) % 3, 2 - (dominant + a) % 3, invert != 0)`。
    pub corner_rule: [[u32; 2]; 8],
    /// 主面 frost 系数（色 += d²·frost，d = |uv − 0.5|）。
    pub frost: f32,
    /// 主面屏幕 UV 的裁剪常数：`surface_uv = (frag_ndc − 0.5)/uv_crop + 0.5`。
    pub uv_crop: f32,
}

/// 噪声顶点位移参数（纯数据；噪声实现在每个后端的顶点着色器里——
/// engine/web 同一份 WGSL）。逐帧只变 `time`，其余为脚本声明的源常量。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoiseDisplacement3 {
    pub amplitude: f32,
    pub frequency: f32,
    pub world_frequency: f32,
    pub time_speed: f32,
    pub time: f32,
}

/// PBR 透射材质参数（three.js `MeshPhysicalMaterial` 透射核心的纯数据
/// 形状；常量全部来自脚本声明）。渲染语义与 three.js r181 对应 chunk
/// 同构：真二遍场景色折射（`transmission_fragment` + WebGLRenderer
/// `renderTransmissionPass`）、Beer-Lambert 体衰减、逐通道色散折射、
/// GGX 直射高光（multiscatter 补偿）、清漆层与环境 IBL
/// （scene.environment → PMREM cubeUV，`env_map_intensity` 为乘数；
/// 场景无 env 时 IBL 项为 0，与 three.js 无 envMap 行为一致）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Physical3 {
    pub ior: f32,
    /// 折射体厚度（局部空间；与模型缩放相乘得透射路径长度）。
    pub thickness: f32,
    /// 透射率（1 = 全透射，totalDiffuse 被透射色完全替换）。
    pub transmission: f32,
    pub roughness: f32,
    pub metalness: f32,
    pub specular_intensity: f32,
    pub clearcoat: f32,
    pub clearcoat_roughness: f32,
    /// 逐通道折射率色散强度（three.js r169+ `dispersion`）。
    pub dispersion: f32,
    /// Beer-Lambert 体吸收色（straight RGB，0..=1）。
    pub attenuation_color: [f32; 3],
    /// 衰减距离（±inf = 不衰减）。
    pub attenuation_distance: f32,
    /// three.js `MeshPhysicalMaterial.envMapIntensity`（js:172；WebGLRenderer
    /// 材质默认 1.0）。环境 IBL 辐射度的乘数（envmap_physical_pars_fragment
    /// getIBLIrradiance/getIBLRadiance）。serde default 兼容旧快照。
    #[serde(default = "default_env_map_intensity")]
    pub env_map_intensity: f32,
}

fn default_env_map_intensity() -> f32 {
    1.0
}

/// 脚本着色器 uniform 值（three.js `ShaderMaterial` uniforms 字典的值域：
/// number → Float、数组 → VecN、Color → Vec3；std140 打包由引擎按编译期
/// 反射的偏移完成，这里只携带纯数据）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShaderValue3 {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

/// 一个脚本 uniform：名字 + 值（逐帧快照携带**当前值**，与 three.js
/// `material.uniforms.uFade.value` 的每帧赋值语义一致）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderUniform3 {
    pub name: String,
    pub value: ShaderValue3,
}

/// 脚本着色器材质（three.js `ShaderMaterial` 的纯数据形状）：GLSL 源 +
/// uniform 当前值。GLSL 是**适配后**的 450 core（three_api shim 扮演
/// WebGLProgram：varying→in/out、attribute→in、gl_FragColor→out、
/// 三方 uniform 块注入——与 three.js 前缀拼接同构）；引擎只做
/// GLSL→naga→WGSL 的通用编译，不携带任何场景语义。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderMaterial3 {
    pub vertex_shader: String,
    pub fragment_shader: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uniforms: Vec<ShaderUniform3>,
}

/// 着色模型。unlit / lambert / 视频-马赛克面片 / PBR 透射 / 脚本着色器。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shading3 {
    Unlit,
    Lambert,
    VideoMosaic(VideoMosaic3),
    Physical(Physical3),
    Shader(ShaderMaterial3),
}

impl Scene3 {
    /// 结构校验：索引越界、法线缺失这类错误要在进渲染器之前报出来，
    /// 而不是等 GPU 吐出黑块。
    pub fn validate(&self) -> Result<()> {
        if self.target[0] == 0 || self.target[1] == 0 {
            bail!(
                "target 尺寸必须非零，得到 {}x{}",
                self.target[0],
                self.target[1]
            );
        }
        for (i, c) in self.clear.iter().enumerate() {
            if !(0.0..=1.0).contains(c) {
                bail!("clear[{i}] 超出 0..=1: {c}");
            }
        }
        for (i, mesh) in self.meshes.iter().enumerate() {
            match &mesh.geometry {
                Geometry3::Asset { id } => {
                    if id.is_empty() {
                        bail!("mesh #{i}: Asset id 为空");
                    }
                }
                Geometry3::Inline {
                    positions,
                    normals,
                    indices,
                    uvs,
                    colors,
                    colors1,
                } => {
                    if normals.len() != positions.len() {
                        bail!(
                            "mesh #{i}: normals 数量 {} != positions 数量 {}",
                            normals.len(),
                            positions.len()
                        );
                    }
                    for (name, count) in [
                        ("uvs", uvs.len()),
                        ("colors", colors.len()),
                        ("colors1", colors1.len()),
                    ] {
                        if count != 0 && count != positions.len() {
                            bail!(
                                "mesh #{i}: {name} 数量 {count} != positions 数量 {}",
                                positions.len()
                            );
                        }
                    }
                    if indices.len() % 3 != 0 {
                        bail!("mesh #{i}: indices 数量 {} 不是 3 的倍数", indices.len());
                    }
                    if let Some((j, idx)) = indices
                        .iter()
                        .enumerate()
                        .find(|(_, idx)| (**idx as usize) >= positions.len())
                    {
                        bail!(
                            "mesh #{i}: indices[{j}] = {idx} 越界（positions = {}）",
                            positions.len()
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit_cube_scene(indices: Vec<u32>, normals: Vec<[f32; 3]>) -> Scene3 {
        Scene3 {
            clear: [0.0, 0.0, 0.0, 0.0],
            target: [64, 64],
            camera: Camera3 {
                position: [0.0, 0.0, 3.0],
                target: [0.0, 0.0, 0.0],
                fov_y_degrees: 45.0,
                near: 0.1,
                far: 10.0,
            },
            lights: vec![Light3::Ambient {
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            }],
            tone_mapping: ToneMapping3::None,
            tone_mapping_exposure: 1.0,
            output_color_space: OutputColorSpace3::Linear,
            environment: None,
            meshes: vec![Mesh3 {
                geometry: Geometry3::Inline {
                    positions: vec![[0.0; 3]; 3],
                    normals,
                    indices,
                    uvs: vec![],
                    colors: vec![],
                    colors1: vec![],
                },
                material: Material3 {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    shading: Shading3::Lambert,
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
            }],
        }
    }

    #[test]
    fn valid_inline_geometry_passes() {
        let scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 3]);
        scene.validate().expect("合法场景应通过校验");
    }

    #[test]
    fn out_of_range_index_is_rejected() {
        let scene = lit_cube_scene(vec![0, 1, 3], vec![[0.0, 0.0, 1.0]; 3]);
        let err = scene.validate().expect_err("越界索引必须报错");
        assert!(err.to_string().contains("越界"), "got: {err}");
    }

    #[test]
    fn normal_count_mismatch_is_rejected() {
        let scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 2]);
        let err = scene.validate().expect_err("法线数量不匹配必须报错");
        assert!(err.to_string().contains("normals"), "got: {err}");
    }

    #[test]
    fn zero_sized_target_is_rejected() {
        let mut scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 3]);
        scene.target = [0, 64];
        let err = scene.validate().expect_err("零尺寸渲染目标必须报错");
        assert!(err.to_string().contains("target"), "got: {err}");
    }

    // Shading3::Physical 经快照 JSON（serde 外部标签，snake_case 字段）往返
    // 保真——three_api.js 的快照载荷字段命名铁律（§21.4）。
    #[test]
    fn physical_shading_round_trips_through_snapshot_json() {
        let mut scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 3]);
        scene.meshes[0].material.shading = Shading3::Physical(Physical3 {
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
        let json = serde_json::to_string(&scene).expect("快照序列化");
        assert!(
            json.contains("\"Physical\""),
            "外部标签必须是 Physical: {json}"
        );
        assert!(json.contains("\"attenuation_color\""), "snake_case: {json}");
        assert!(json.contains("\"specular_intensity\""), "snake_case: {json}");
        let back: Scene3 = serde_json::from_str(&json).expect("快照反序列化");
        assert_eq!(back, scene);
    }

    // Shading3::Shader 同样经快照 JSON 往返保真：GLSL 源逐字 + uniforms
    // 外部标签值（{"Float": …}/{"Vec3": […]}）。
    #[test]
    fn shader_shading_round_trips_through_snapshot_json() {
        let mut scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 3]);
        scene.meshes[0].material.shading = Shading3::Shader(ShaderMaterial3 {
            vertex_shader: "#version 450 core\nvoid main() {}".to_string(),
            fragment_shader: "#version 450 core\nlayout(location = 0) out vec4 fragOutput;\nvoid main() {}".to_string(),
            uniforms: vec![
                ShaderUniform3 {
                    name: "uFade".to_string(),
                    value: ShaderValue3::Float(0.25),
                },
                ShaderUniform3 {
                    name: "uTint".to_string(),
                    value: ShaderValue3::Vec3([0.957, 0.949, 0.918]),
                },
            ],
        });
        let json = serde_json::to_string(&scene).expect("快照序列化");
        assert!(json.contains("\"Shader\""), "外部标签必须是 Shader: {json}");
        assert!(json.contains("\"vertex_shader\""), "snake_case: {json}");
        assert!(json.contains("\"Float\""), "uniform 值外部标签: {json}");
        let back: Scene3 = serde_json::from_str(&json).expect("快照反序列化");
        assert_eq!(back, scene);
    }

    // render_order / transparent（threejs-like r7：⑤ 段 droplets 需要
    // renderOrder + transparent 列表语义）经快照 JSON 往返保真，且**旧快照**
    // （缺这两字段）靠 serde default 解码——快照只增不改的兼容契约。
    #[test]
    fn render_order_and_transparent_round_trip_and_backcompat() {
        let mut scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 3]);
        scene.meshes[0].render_order = -10;
        scene.meshes[0].material.transparent = true;
        let json = serde_json::to_string(&scene).expect("快照序列化");
        assert!(json.contains("\"render_order\":-10"), "snake_case render_order: {json}");
        assert!(json.contains("\"transparent\":true"), "transparent 字段: {json}");
        let back: Scene3 = serde_json::from_str(&json).expect("快照反序列化");
        assert_eq!(back, scene);
        // 旧快照（r6 及之前不含 render_order/transparent）解码不炸：剥掉
        // 这两个尾巴字段（两者都是各自对象的末字段，前面带逗号）。
        let stripped = json
            .replace(",\"render_order\":-10}", "}")
            .replace(",\"transparent\":true}", "}");
        let old: Scene3 =
            serde_json::from_str(&stripped).expect("旧快照（无新字段）应靠 default 解码");
        assert_eq!(old.meshes[0].render_order, 0);
        assert!(!old.meshes[0].material.transparent);
    }

    // tone_mapping / tone_mapping_exposure / output_color_space（threejs-like
    // r8：⑥ 段 renderer 输出变换 js:18-20）经快照 JSON 往返保真；**旧快照**
    // （缺这三个字段）靠 serde default 解码为 NoToneMapping/1.0/Linear——
    // 既有场景输出逐字节不变。
    #[test]
    fn tone_mapping_settings_round_trip_and_backcompat() {
        let mut scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 3]);
        scene.tone_mapping = ToneMapping3::ACESFilmic;
        scene.tone_mapping_exposure = 1.08;
        scene.output_color_space = OutputColorSpace3::Srgb;
        let json = serde_json::to_string(&scene).expect("快照序列化");
        assert!(
            json.contains("\"tone_mapping\":\"ACESFilmic\""),
            "单元变体按变体名拼写: {json}"
        );
        assert!(json.contains("\"tone_mapping_exposure\":1.08"), "exposure: {json}");
        assert!(
            json.contains("\"output_color_space\":\"Srgb\""),
            "unit 变体按变体名拼写: {json}"
        );
        let back: Scene3 = serde_json::from_str(&json).expect("快照反序列化");
        assert_eq!(back, scene);
        // 旧快照（r7 及之前）解码为引擎中性默认：无 tonemap / exposure 1 /
        // Linear 输出——与 three.js 默认 NoToneMapping/1.0 同值（Linear 是
        // 本引擎既有 raw 契约，见字段注释）。
        let stripped = json
            .replace(",\"tone_mapping\":\"ACESFilmic\"", "")
            .replace(",\"tone_mapping_exposure\":1.08", "")
            .replace(",\"output_color_space\":\"Srgb\"", "");
        let old: Scene3 =
            serde_json::from_str(&stripped).expect("旧快照（无新字段）应靠 default 解码");
        assert_eq!(old.tone_mapping, ToneMapping3::None);
        assert_eq!(old.tone_mapping_exposure, 1.0);
        assert_eq!(old.output_color_space, OutputColorSpace3::Linear);
    }

    // scene.environment + Physical3.env_map_intensity（threejs-like r9：⑥b
    // 段 env IBL js:23-26/163-179）经快照 JSON 往返保真；**旧快照**（缺
    // environment 字段 / env_map_intensity 字段）靠 serde default 解码为
    // None / 1.0（three.js 材质默认 envMapIntensity=1）——既有场景输出
    // 逐字节不变。
    #[test]
    fn environment_and_env_map_intensity_round_trip_and_backcompat() {
        let mut scene = lit_cube_scene(vec![0, 1, 2], vec![[0.0, 0.0, 1.0]; 3]);
        scene.environment = Some(Texture3::Bitmap {
            path: "../assets/equirect-garden.jpg".to_string(),
        });
        scene.meshes[0].material.shading = Shading3::Physical(Physical3 {
            ior: 1.33,
            thickness: 1.5,
            transmission: 1.0,
            roughness: 0.1,
            metalness: 0.0,
            specular_intensity: 1.0,
            clearcoat: 0.4,
            clearcoat_roughness: 0.1,
            dispersion: 0.0,
            attenuation_color: [1.0, 1.0, 1.0],
            attenuation_distance: 1e30,
            env_map_intensity: 1.2,
        });
        let json = serde_json::to_string(&scene).expect("快照序列化");
        assert!(
            json.contains("\"environment\":{\"Bitmap\":{\"path\":"),
            "外部标签 Texture3: {json}"
        );
        assert!(json.contains("\"env_map_intensity\":1.2"), "snake_case: {json}");
        let back: Scene3 = serde_json::from_str(&json).expect("快照反序列化");
        assert_eq!(back, scene);
        // 旧快照：剥掉 environment（Scene3 尾字段）与 env_map_intensity
        // （Physical 载荷尾字段）→ None / 1.0。
        let stripped = json
            .replace(
                ",\"environment\":{\"Bitmap\":{\"path\":\"../assets/equirect-garden.jpg\"}}",
                "",
            )
            .replace(",\"env_map_intensity\":1.2}", "}");
        let old: Scene3 =
            serde_json::from_str(&stripped).expect("旧快照（无新字段）应靠 default 解码");
        assert!(old.environment.is_none());
        match &old.meshes[0].material.shading {
            Shading3::Physical(p) => assert_eq!(p.env_map_intensity, 1.0),
            other => panic!("期望 Physical: {other:?}"),
        }
    }
}
