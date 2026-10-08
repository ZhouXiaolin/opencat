//! 通用 glTF-BINARY（GLB）几何解码 → [`Geometry3::Inline`]。
//!
//! **解析一律走 `gltf` crate（gltf-rs 家族），不手写**：容器拆分用
//! [`gltf::Glb::from_slice`]（magic/version/chunk 遍历全部交给 crate），
//! 文档树用 [`gltf::Document`]，accessor 解码用 [`gltf::mesh::Reader`]。
//! 后续 node 层级 / 材质 / 纹理解析同样只能经该 crate（其
//! `KHR_materials_transmission` 可选特性服务棱柱透射材质轮）。
//!
//! 只做几何：POSITION / NORMAL / TEXCOORD_0 / COLOR_0 / COLOR_1 / indices。
//! 材质、纹理、节点层级不在此层（层级级联由脚本 shim 完成，IR 只收
//! 扁平世界矩阵）。这是通用资产设施，不含任何影片专用语义。
//!
//! 支持面（够用且可报错收紧，不做静默猜）：
//! - componentType：i8/u8/i16/u16/u32/f32；整型 normalized（COLOR_\*、
//!   TEXCOORD_\*）保持 crate 的 `into_rgba_f32` 直读——其对 u16 走
//!   `65535.0.recip()` 乘倒数（gltf-1.4.1 src/lib.rs:774），与参考渲染
//!   器（SwiftShader）的 unorm 顶点采样语义一致（r13 两次 f31 成对探针
//!   实证：规范除法实现 hot 13536→28864 / 36697，均更差，见解析处
//!   注释）。r12 记录的「colors1 4230 个值恰差 1 bit」即此 1 ULP，是
//!   对齐所需而非偏差。
//! - type：SCALAR / VEC2 / VEC3 / VEC4（COLOR_\* 的 VEC3 按 glTF 规范
//!   补 alpha = 1；旧手写解析器拒收 VEC3，属无必要收紧）。
//! - 取第一个 mesh 的恰好 1 个 primitive（多 primitive 报错——目前没有
//!   需要合并的资产，静默合并反而藏错）。

use anyhow::{Result, bail};

use super::scene3::Geometry3;

/// 从 GLB 字节解析第一个 mesh primitive 的几何。
pub fn parse_glb(bytes: &[u8]) -> Result<Geometry3> {
    // 头部预检只为了保住旧解析器的精确报错（测试断言其消息）；chunk 拆分
    // 与长度校验仍由 [`gltf::Glb::from_slice`] 完成。
    if bytes.len() < 12 {
        bail!("GLB 过短（{} 字节 < 12 头部）", bytes.len());
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into()?);
    let version = u32::from_le_bytes(bytes[4..8].try_into()?);
    if magic != 0x4654_6C67 {
        bail!("不是 GLB（magic 0x{magic:08x} ≠ 'glTF'）");
    }
    if version != 2 {
        bail!("仅支持 glTF 2.0，得到版本 {version}");
    }
    let glb = gltf::Glb::from_slice(bytes)
        .map_err(|e| anyhow::anyhow!("GLB 容器解析失败: {e}"))?;
    let json = gltf::Document::from_json(serde_json::from_slice(&glb.json)?)?;
    let bin: &[u8] = glb.bin.as_deref().unwrap_or(&[]);

    // GLB 的 buffer 必须内嵌 BIN chunk；Uri 源是 .gltf 文件语义，不在这层
    // 支持（避免 Reader 静默读空后报出误导性的"缺少 POSITION"）。
    for buffer in json.buffers() {
        if !matches!(buffer.source(), gltf::buffer::Source::Bin) {
            bail!("GLB buffer 必须内嵌 BIN chunk（得到 Uri 源，属 .gltf 语义）");
        }
    }

    let mesh = json
        .meshes()
        .next()
        .ok_or_else(|| anyhow::anyhow!("GLB 无 mesh"))?;
    let prims: Vec<_> = mesh.primitives().collect();
    if prims.len() != 1 {
        bail!(
            "GLB mesh 有 {} 个 primitive，本解析器只接受恰好 1 个（多 primitive 请先合并）",
            prims.len()
        );
    }
    let reader = prims[0].reader(|buffer| match buffer.source() {
        gltf::buffer::Source::Bin => Some(bin),
        gltf::buffer::Source::Uri(_) => None,
    });

    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or_else(|| anyhow::anyhow!("GLB primitive 缺少 POSITION"))?
        .collect();
    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|it| it.collect())
        .unwrap_or_default();
    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|tc| tc.into_f32().collect())
        .unwrap_or_default();
    // COLOR_0 / COLOR_1 归一化语义（r13 两次成对探针实证，勿改除法）：参考
    // 渲染器（SwiftShader GL）的 u16 unorm 顶点采样实现是**乘倒数**
    // v*(1/65535)——gltf crate 的 `into_rgba_f32` 恰用 `65535.0_f32.recip()`
    // （src/lib.rs:774），同语义。除法实现（glTF 规范 v/65535）在消费端
    // `u32(color_1.r * 255.0)` 截断下使 byte∈{1,2,4} 各加 1，f31 棱柱
    // 成对探针两次实测更差（13536→28864 含 droplets 污染、13536→36697
    // 干净复测），回滚。r13 的 videoId 调试场（`float(vVideoId)/8.0`）
    // 对该语义**无区分力**：f31 姿态下可见主面 byte∈{7,3,5,6} 不含
    // 翻转带 {1,2,4}，两假说预测相同；首读时把背景输出变换色误计为
    // videoId=7 是误报（背景 0x101014 经 toneMapping+sRGB 后 R≈231，
    // 231/255*8≈7.25）。COLOR_0 走 recip 的 1 ULP 不影响 dominant
    // argmax（角点编码通道差远大于 1 ULP）。
    let colors: Vec<[f32; 4]> = reader
        .read_colors(0)
        .map(|c| c.into_rgba_f32().collect())
        .unwrap_or_default();
    let colors1: Vec<[f32; 4]> = reader
        .read_colors(1)
        .map(|c| c.into_rgba_f32().collect())
        .unwrap_or_default();
    // provoking vertex 语义（r13 实证记录，勿重复尝试）：GL flat varying
    // 取**最后一个**顶点（GL_LAST_VERTEX_CONVENTION），Vulkan/wgpu 取
    // **第一个**。曾以索引循环 (a,b,c)→(c,a,b) 移植该语义；f31 棱柱成对
    // 探针渲染**逐位零变化**（本 GLB 棱线窄条带面的顶点属性对称，
    // provoking 顶点切换无生效面），已回滚——corner 色带差的机制不在
    // provoking（见 r13 handoff：色带差归因仍开放）。
    let indices: Vec<u32> = reader
        .read_indices()
        .ok_or_else(|| anyhow::anyhow!("GLB primitive 缺少 indices"))?
        .into_u32()
        .collect();

    if normals.len() != positions.len() {
        bail!("NORMAL 数量 {} != POSITION {}", normals.len(), positions.len());
    }
    if let Some(idx) = indices.iter().find(|i| (**i as usize) >= positions.len()) {
        bail!("索引 {idx} 越界（POSITION = {} 顶点）", positions.len());
    }

    Ok(Geometry3::Inline {
        positions,
        normals,
        indices,
        uvs,
        colors,
        colors1,
    })
}

/// 读文件并解析（native 渲染器路径；web 走注入的字节源，见 round 记录）。
pub fn load_glb_file(path: &str) -> Result<Geometry3> {
    let bytes =
        std::fs::read(path).map_err(|e| anyhow::anyhow!("读取 GLB '{path}' 失败: {e}"))?;
    parse_glb(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小 GLB：1 个三角形，带 POSITION(f32)、NORMAL(f32)、
    /// TEXCOORD_0(f32)、COLOR_0(u16 normalized)、COLOR_1(u16 normalized)、
    /// indices(u16)。
    fn triangle_glb() -> Vec<u8> {
        fn f32_bytes(v: &[f32]) -> Vec<u8> {
            v.iter().flat_map(|f| f.to_le_bytes()).collect()
        }
        fn u16_bytes(v: &[u16]) -> Vec<u8> {
            v.iter().flat_map(|x| x.to_le_bytes()).collect()
        }

        let pos_b = f32_bytes(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        let nrm_b = f32_bytes(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        let uv_b = f32_bytes(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        // COLOR_0：红 + alpha（u16 normalized）。
        let c0_b = u16_bytes(&[
            0xFFFF, 0, 0, 0xFFFF, // v0 红
            0xFFFF, 0, 0, 0xFFFF, // v1
            0xFFFF, 0, 0, 0xFFFF, // v2
        ]);
        // COLOR_1：video id = 2（u16=514 = 2×257 字节编码；选 2 而非 3 是
        // 因为 3 恰是乘倒数实现 trunc 安全的值，1/2/4 才会翻转——防回归
        // 锚必须落在翻转带上）。
        let c1_b = u16_bytes(&[514, 0, 0, 0, 514, 0, 0, 0, 514, 0, 0, 0]);
        let idx_b = u16_bytes(&[0, 1, 2]);

        let (off_nrm, off_uv, off_c0, off_c1, off_idx) = (
            pos_b.len(),
            pos_b.len() + nrm_b.len(),
            pos_b.len() + nrm_b.len() + uv_b.len(),
            pos_b.len() + nrm_b.len() + uv_b.len() + c0_b.len(),
            pos_b.len() + nrm_b.len() + uv_b.len() + c0_b.len() + c1_b.len(),
        );
        let bin_len = off_idx + idx_b.len();
        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "meshes": [{
                "primitives": [{
                    "attributes": {
                        "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2,
                        "COLOR_0": 3, "COLOR_1": 4,
                    },
                    "indices": 5,
                }]
            }],
            "accessors": [
                // glTF 规范要求 POSITION 带 min/max（gltf crate 的 Document
                // 校验强制；真实导出器都写，测试夹具也写）。
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                 "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0]},
                {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2"},
                {"bufferView": 3, "componentType": 5123, "count": 3, "type": "VEC4", "normalized": true},
                {"bufferView": 4, "componentType": 5123, "count": 3, "type": "VEC4", "normalized": true},
                {"bufferView": 5, "componentType": 5123, "count": 3, "type": "SCALAR"},
            ],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": pos_b.len()},
                {"buffer": 0, "byteOffset": off_nrm, "byteLength": nrm_b.len()},
                {"buffer": 0, "byteOffset": off_uv, "byteLength": uv_b.len()},
                {"buffer": 0, "byteOffset": off_c0, "byteLength": c0_b.len()},
                {"buffer": 0, "byteOffset": off_c1, "byteLength": c1_b.len()},
                {"buffer": 0, "byteOffset": off_idx, "byteLength": idx_b.len()},
            ],
            "buffers": [{"byteLength": bin_len}],
        })
        .to_string();
        let json_b = json.into_bytes();

        let mut out = Vec::new();
        out.extend_from_slice(&0x4654_6C67u32.to_le_bytes()); // 'glTF'
        out.extend_from_slice(&2u32.to_le_bytes());
        let total = 12 + 8 + json_b.len() + 8 + bin_len;
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_b.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON_SENTINEL.to_le_bytes());
        out.extend_from_slice(&json_b);
        out.extend_from_slice(&(bin_len as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN_SENTINEL.to_le_bytes());
        let mut bin = pos_b;
        bin.extend_from_slice(&nrm_b);
        bin.extend_from_slice(&uv_b);
        bin.extend_from_slice(&c0_b);
        bin.extend_from_slice(&c1_b);
        bin.extend_from_slice(&idx_b);
        out.extend_from_slice(&bin);
        out
    }

    // chunk 类型常量只服务测试 GLB 的拼装（解析侧由 gltf crate 负责）。
    const CHUNK_JSON_SENTINEL: u32 = 0x4E4F_534A; // 'JSON'
    const CHUNK_BIN_SENTINEL: u32 = 0x004E_4942; // 'BIN\0'

    #[test]
    fn parses_triangle_with_all_attributes() {
        let geo = parse_glb(&triangle_glb()).expect("合法 GLB 应解析成功");
        let Geometry3::Inline {
            positions,
            normals,
            indices,
            uvs,
            colors,
            colors1,
        } = geo
        else {
            panic!("期望 Inline 几何");
        };
        assert_eq!(positions.len(), 3);
        assert_eq!(normals.len(), 3);
        assert_eq!(uvs.len(), 3);
        assert_eq!(uvs[1], [1.0, 0.0]);
        // COLOR_0 u16 normalized：0xFFFF → 1.0。
        assert_eq!(colors[0], [1.0, 0.0, 0.0, 1.0]);
        // COLOR_1 u16 normalized：514/65535 走 crate 的 recip 路径——与
        // 参考渲染器（SwiftShader unorm 乘倒数）同语义，r13 两次成对探针
        // 实证（除法实现 f31 hot 13536→28864 / 13536→36697 更差）。
        assert!((colors1[2][0] - 514.0 / 65535.0).abs() < 2e-7);
        // videoId trunc 锚：消费端 shader 做 `u32(color_1.r * 255.0)`，
        // 参考渲染器的有效值是 1（recip 语义），不是规范除法的 2。514
        // 恰在翻转带上，作 recip 语义的防回归锚。
        assert_eq!((colors1[2][0] * 255.0) as u32, 1);
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn rejects_non_glb_and_bad_version() {
        let err = parse_glb(b"not a glb at all.....").expect_err("坏 magic 必须报错");
        assert!(err.to_string().contains("不是 GLB"), "got: {err}");
        let mut bad = triangle_glb();
        bad[4] = 3; // version = 3
        let err = parse_glb(&bad).expect_err("非 2.0 必须报错");
        assert!(err.to_string().contains("glTF 2.0"), "got: {err}");
    }

    #[test]
    fn rejects_out_of_range_index() {
        let mut glb = triangle_glb();
        // 索引在 BIN 尾部：最后一个 u16 改成 9（越界）。
        let n = glb.len();
        glb[n - 2] = 9;
        let err = parse_glb(&glb).expect_err("越界索引必须报错");
        assert!(err.to_string().contains("越界"), "got: {err}");
    }

    /// 真 GLB（prism）：gltf crate 解码形状与 engine 侧集成测试断言一致
    /// （11280 顶点 / 60096 索引 / 全五属性）。资产缺席（独立 core CI）时跳过。
    #[test]
    fn parses_real_prism_asset_when_present() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/lbr/heygen-prism.glb");
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("skip: {path} 不存在");
            return;
        };
        let Geometry3::Inline {
            positions,
            normals,
            indices,
            uvs,
            colors,
            colors1,
        } = parse_glb(&bytes).expect("prism GLB 应解析成功")
        else {
            panic!("期望 Inline 几何");
        };
        assert_eq!(positions.len(), 11280);
        assert_eq!(normals.len(), 11280);
        assert_eq!(uvs.len(), 11280);
        assert_eq!(colors.len(), 11280);
        assert_eq!(colors1.len(), 11280);
        assert_eq!(indices.len(), 60096);
        assert!(indices.iter().all(|i| (*i as usize) < positions.len()));
    }
}
