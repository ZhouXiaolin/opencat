//! GLB 场景（node 树 + 材质 + 内嵌纹理）→ [`Mesh3`] 列表。
//!
//! [`super::scene3_glb`] 只解一个 mesh 的一个 primitive 的几何；本模块是它
//! 的场景级对应物：把 glTF 的 node/mesh/material/texture 全树摊平成扁平世界
//! 矩阵的网格表（IR 只收扁平矩阵，层级级联在这里做完）。
//!
//! 材质映射（three.js `meshStandardMaterial` 语义）：
//! - `baseColorFactor` → `base_color`，`metallic`/`roughness` 因子 →
//!   [`Physical3`]；glTF 未写的 clearcoat/transmission 保持 0。
//! - `baseColorTexture` → [`Material3::base_color_map`]、
//!   `metallicRoughnessTexture` → [`Material3::metallic_roughness_map`]，
//!   由渲染器在 `fs_physical` 里按 G/B 通道乘进 roughness/metalness。
//! - 内嵌纹理（`bufferView` 源）不走文件路径：写进调用方给的缓存目录，
//!   以普通 `Texture3::Bitmap` 交给宿主解码（`image` crate 解 webp/jpeg 是
//!   engine 的既有能力）。
//!
//! 范围：三角网格 + 上述两张贴图。`EXT_texture_webp` 的 image 索引放在扩展
//! 里（`source` 字段此时指向扩展作者随手写的值），所以纹理索引直读原始
//! JSON 而不是 `gltf::Texture::source()`。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::scene3::{Geometry3, Material3, Mesh3, Physical3, Shading3, Texture3};

/// GLB 场景导入结果：内嵌纹理的落盘路径 + 摊平后的网格表。
pub struct GlbScene {
    /// 从 GLB 的 bufferView 里解出来的图片文件（webp/jpeg/png），已写盘。
    /// 调用方负责把它们登记成宿主资产（纹理来源是普通 `Bitmap` 路径）。
    pub images: Vec<String>,
    pub meshes: Vec<Mesh3>,
    /// 与 `meshes` 等长：该网格是否是屏幕（glTF 带 `emissiveTexture`）。
    pub screens: Vec<bool>,
}

/// 从 GLB 文件构造场景。
pub fn load_glb_scene(path: &str, cache_dir: &Path) -> Result<GlbScene> {
    let bytes = std::fs::read(path).map_err(|e| anyhow::anyhow!("读取 GLB '{path}' 失败: {e}"))?;
    parse_glb_scene(&bytes, cache_dir)
}

/// 同 [`load_glb_scene`]，输入是字节。
pub fn parse_glb_scene(bytes: &[u8], cache_dir: &Path) -> Result<GlbScene> {
    let glb =
        gltf::Glb::from_slice(bytes).map_err(|e| anyhow::anyhow!("GLB 容器解析失败: {e}"))?;
    let raw: serde_json::Value = serde_json::from_slice(&glb.json)?;
    // 不做 `validate()`：它会拒绝 `EXT_texture_webp`（`extensionsRequired`
    // 里的 webp 纹理源）与 Draco 之外的扩展，而本解析器不需要这些校验
    // —— 我们按 bufferView 自己解几何与图片。
    let json = gltf::Document::from_json_without_validation(serde_json::from_slice(&glb.json)?);
    let bin: &[u8] = glb.bin.as_deref().unwrap_or(&[]);

    let mut sink = TextureSink {
        json: &json,
        raw: &raw,
        bin,
        cache_dir,
        written: HashMap::new(),
    };

    let mut out = Vec::new();
    let mut screens = Vec::new();
    for scene in json.scenes() {
        for node in scene.nodes() {
            walk(&node, IDENTITY, bin, &mut sink, &mut out, &mut screens)?;
        }
    }
    if out.is_empty() {
        bail!("GLB 场景没有任何带 mesh 的节点");
    }
    let mut images: Vec<String> = sink.written.values().cloned().collect();
    images.sort();
    Ok(GlbScene {
        images,
        meshes: out,
        screens,
    })
}

/// 单位矩阵（列主序）。
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// 列主序 4×4 乘法：`a * b`（先 b 后 a，与 glam 的 `a * b` 同序）。
fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

/// glTF node 的局部矩阵（`Transform::matrix()` 已是列主序列数组）。
fn local_matrix(node: &gltf::Node) -> [f32; 16] {
    let m = node.transform().matrix();
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = m[col][row];
        }
    }
    out
}

fn walk(
    node: &gltf::Node,
    parent: [f32; 16],
    bin: &[u8],
    sink: &mut TextureSink,
    out: &mut Vec<Mesh3>,
    screens: &mut Vec<bool>,
) -> Result<()> {
    let world = mul(&parent, &local_matrix(node));
    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let geometry = read_primitive(&prim, bin)?;
            let material = read_material(&prim, sink)?;
            // 屏幕判据与参考一致：带 emissiveTexture 的就是屏幕（材质名是
            // 随机串，名字不可用）。
            screens.push(prim.material().emissive_texture().is_some());
            out.push(Mesh3 {
                geometry,
                material,
                matrix: world,
                render_order: 0,
            });
        }
    }
    for child in node.children() {
        walk(&child, world, bin, sink, out, screens)?;
    }
    Ok(())
}

/// 内嵌图片的一次性落盘 + 索引（同图跨 mesh 只写一份）。
struct TextureSink<'a> {
    json: &'a gltf::Document,
    raw: &'a serde_json::Value,
    bin: &'a [u8],
    cache_dir: &'a Path,
    /// image 索引 → 落盘路径。
    written: HashMap<usize, String>,
}

impl TextureSink<'_> {
    /// glTF `texture` 索引 → `Texture3::Bitmap`。
    fn texture(&mut self, index: usize, op: &str) -> Result<Texture3> {
        let image = self.image_index(index)?;
        let path = self.materialize(image)?;
        Ok(Texture3::Bitmap { path })
    }

    /// `EXT_texture_webp` 把真正的 image 索引放在扩展里；没有扩展时读
    /// `source`。两者都是原始 JSON 的字段。
    fn image_index(&self, index: usize) -> Result<usize> {
        let tex = self
            .raw
            .get("textures")
            .and_then(|t| t.get(index))
            .ok_or_else(|| anyhow::anyhow!("texture {index} 不存在"))?;
        let webp = tex
            .get("extensions")
            .and_then(|e| e.get("EXT_texture_webp"))
            .and_then(|w| w.get("source"))
            .and_then(|s| s.as_u64());
        let src = webp.or_else(|| tex.get("source").and_then(|s| s.as_u64()));
        src.map(|s| s as usize)
            .ok_or_else(|| anyhow::anyhow!("texture {index} 没有 source"))
    }

    /// 把 image 的字节写到缓存目录，返回路径。
    fn materialize(&mut self, image: usize) -> Result<String> {
        if let Some(path) = self.written.get(&image) {
            return Ok(path.clone());
        }
        let img = self
            .json
            .images()
            .nth(image)
            .ok_or_else(|| anyhow::anyhow!("image {image} 不存在"))?;
        let path = match img.source() {
            // 外部文件：交给宿主按 GLB 同目录解析。
            gltf::image::Source::Uri { uri, .. } => self.cache_dir.join(uri).to_string_lossy().into_owned(),
            gltf::image::Source::View { view, mime_type } => {
                let (start, end) = (view.offset(), view.offset() + view.length());
                if end > self.bin.len() {
                    bail!(
                        "image {image} 的 bufferView 越界（{start}..{end} > {}）",
                        self.bin.len()
                    );
                }
                let ext = match mime_type {
                    "image/webp" => "webp",
                    "image/jpeg" => "jpg",
                    "image/png" => "png",
                    other => bail!("image {image}: 不支持的 mimeType {other}"),
                };
                std::fs::create_dir_all(self.cache_dir).with_context(|| {
                    format!("创建纹理缓存目录 {} 失败", self.cache_dir.display())
                })?;
                let out = self.cache_dir.join(format!("img{image}.{ext}"));
                std::fs::write(&out, &self.bin[start..end])
                    .with_context(|| format!("写纹理 {} 失败", out.display()))?;
                out.to_string_lossy().into_owned()
            }
        };
        self.written.insert(image, path.clone());
        Ok(path)
    }
}

/// 一个 primitive 的几何（读法与 [`super::scene3_glb::parse_glb`] 一致，
/// 但允许多 mesh / 多 primitive）。
fn read_primitive(prim: &gltf::Primitive, bin: &[u8]) -> Result<Geometry3> {
    let reader = prim.reader(|buffer| match buffer.source() {
        gltf::buffer::Source::Bin => Some(bin),
        gltf::buffer::Source::Uri(_) => None,
    });
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or_else(|| anyhow::anyhow!("primitive 缺少 POSITION"))?
        .collect();
    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|it| it.collect())
        .unwrap_or_default();
    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|tc| tc.into_f32().collect())
        .unwrap_or_default();
    let indices: Vec<u32> = reader
        .read_indices()
        .ok_or_else(|| anyhow::anyhow!("primitive 缺少 indices"))?
        .into_u32()
        .collect();
    if normals.len() != positions.len() {
        bail!(
            "NORMAL 数量 {} != POSITION {}",
            normals.len(),
            positions.len()
        );
    }
    if let Some(idx) = indices.iter().find(|i| (**i as usize) >= positions.len()) {
        bail!("索引 {idx} 越界（POSITION = {} 顶点）", positions.len());
    }
    Ok(Geometry3::Inline {
        positions,
        normals,
        indices,
        uvs,
        colors: Vec::new(),
        colors1: Vec::new(),
    })
}

/// glTF 材质 → IR 材质。
fn read_material(prim: &gltf::Primitive, sink: &mut TextureSink) -> Result<Material3> {
    let mat = prim.material();
    let pbr = mat.pbr_metallic_roughness();
    let base_color_map = pbr
        .base_color_texture()
        .map(|info| sink.texture(info.texture().index(), "baseColorTexture"))
        .transpose()?;
    let metallic_roughness_map = pbr
        .metallic_roughness_texture()
        .map(|info| sink.texture(info.texture().index(), "metallicRoughnessTexture"))
        .transpose()?;
    let normal_map = mat
        .normal_texture()
        .map(|info| sink.texture(info.texture().index(), "normalTexture"))
        .transpose()?;
    Ok(Material3 {
        base_color: pbr.base_color_factor(),
        shading: Shading3::Physical(Physical3 {
            ior: 1.5,
            transmission: 0.0,
            // glTF 没写 roughness/metallic 时的规范缺省是 1.0。
            roughness: pbr.roughness_factor(),
            metalness: pbr.metallic_factor(),
            specular_intensity: 1.0,
            // 快照走 JSON：`f32::INFINITY` 会序列化成 null，用一个大有限值
            // （与 three_api.js 的 MeshPhysicalMaterial 缺省 1e30 一致）。
            attenuation_distance: 1e30,
            env_map_intensity: 1.0,
            ..Physical3::default()
        }),
        displacement: None,
        map: None,
        textures: Vec::new(),
        transparent: !matches!(mat.alpha_mode(), gltf::material::AlphaMode::Opaque),
        base_color_map,
        metallic_roughness_map,
        normal_map,
    })
}

/// 快照里的一颗 GLB 网格：几何走 realm 注册表（只带 id），材质是 IR
/// 载荷（脚本侧原样塞回快照，不经 shim 的材质对象）。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GlbSceneMesh {
    pub geometry_id: String,
    pub material: Material3,
    /// 节点世界矩阵（列主序）。
    pub matrix: [f32; 16],
    /// glTF 带 `emissiveTexture` 的网格 = 屏幕（参考 index.html:804-816 的
    /// 判据：材质名是随机串，只有 emissiveMap 稳定可用）。
    pub screen: bool,
}

/// GLB 内嵌纹理的默认落盘目录（按 GLB 路径区分，进程内复用）。
pub fn texture_cache_dir(glb_path: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    glb_path.hash(&mut h);
    std::env::temp_dir()
        .join("opencat-glb-textures")
        .join(format!("{:016x}", h.finish()))
}
