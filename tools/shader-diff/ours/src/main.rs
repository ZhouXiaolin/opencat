// ours — shader-diff 对拍的 ours 槽位渲染器。
//
// 输入：tools/shader-diff/wgsl/<Component>/（export-wgsl.test.ts 产出）
//   shader-N.wgsl  每 pass 一份完整 WGSL（final pass 恒为最后一个 -N）
//   passes.json    rtt pass 表（key/reads）+ finalReads + computeSteps
//   values.json    uniform 值（n_<id> → 字段 → 已 transform 的最终值）
//   layout.json    preset 树 + sysInitial
//
// 契约（与 oracle 逐项对齐）：
//   - canvas 512×512, rgba8unorm（oracle 是 Chrome preferred format）
//   - 黑底合成：clear black，pass 语义由导出的 WGSL 内嵌（premultiply tail 已烘焙）
//   - 帧步进：Δt = 1/30；f10/f45/f90 采样；_sys.time = ΣΔt
//   - _animTime（声明 animatedTime 的组件）每帧 += Δt × speed
//
// 用法：ours --wgsl <wgsl-dir> --out <out-dir> [--only A,B | --all]

use anyhow::{bail, Context, Result};
use naga::ir;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;

const W: u32 = 512;
const H: u32 = 512;
const FPS: f64 = 30.0;

#[derive(Deserialize)]
struct Passes {
    rtt: Vec<RttPass>,
    #[serde(rename = "finalReads")]
    final_reads: Vec<String>,
    #[serde(rename = "computeSteps")]
    compute_steps: usize,
}

#[derive(Deserialize)]
struct RttPass {
    index: usize,
    key: String,
    reads: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum FieldVal {
    Num(f64),
    Vec(Vec<f64>),
}

type Values = BTreeMap<String, BTreeMap<String, FieldVal>>;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Float,
    Int,
    Uint,
    Bool,
}

#[derive(Clone)]
struct MemberInfo {
    kind: Kind,
    components: u32,
    array_len: Option<u32>,
}

/// 二级布局：struct 名 → 字段名 → (offset, info)
type SubMap = BTreeMap<String, BTreeMap<String, (u32, MemberInfo)>>;

fn reflect(wgsl: &str) -> Result<ir::Module> {
    let mut fe = naga::front::wgsl::Frontend::new();
    let module = fe.parse(wgsl).map_err(|e| anyhow::anyhow!("wgsl parse: {e:?}"))?;
    let mut validator = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all());
    validator.validate(&module).map_err(|e| anyhow::anyhow!("wgsl validate: {e:?}"))?;
    Ok(module)
}

fn kind_of(k: ir::ScalarKind) -> Kind {
    match k {
        ir::ScalarKind::Float => Kind::Float,
        ir::ScalarKind::Sint => Kind::Int,
        ir::ScalarKind::Uint => Kind::Uint,
        ir::ScalarKind::Bool => Kind::Bool,
        other => unreachable!("unexpected scalar kind {other:?}"),
    }
}

fn member_info(module: &ir::Module, ty: naga::Handle<ir::Type>) -> Result<MemberInfo> {
    Ok(match &module.types[ty].inner {
        ir::TypeInner::Scalar(s) => MemberInfo { kind: kind_of(s.kind), components: 1, array_len: None },
        ir::TypeInner::Vector { size, scalar, .. } => MemberInfo { kind: kind_of(scalar.kind), components: *size as u32, array_len: None },
        ir::TypeInner::Array { base, size: ir::ArraySize::Constant(n), .. } => {
            let elem = member_info(module, *base)?;
            MemberInfo { kind: elem.kind, components: elem.components, array_len: Some(n.get()) }
        }
        other => bail!("unsupported member type: {other:?}"),
    })
}

/// combined 成员名 → 子 struct 类型名。
fn reflect_combined_members(module: &ir::Module) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (_h, ty) in module.types.iter() {
        if ty.name.as_deref() != Some("combined") {
            continue;
        }
        if let ir::TypeInner::Struct { members, .. } = &ty.inner {
            for m in members {
                let Some(mn) = &m.name else { continue };
                let tn = module.types[m.ty].name.clone().context("anonymous member type")?;
                out.insert(mn.clone(), tn);
            }
        }
    }
    Ok(out)
}

/// 反射全部具名 struct（含嵌套），组二级布局表。
fn reflect_structs(module: &ir::Module) -> Result<SubMap> {
    let mut out = SubMap::new();
    for (_h, ty) in module.types.iter() {
        let Some(name) = &ty.name else { continue };
        if let ir::TypeInner::Struct { members, .. } = &ty.inner {
            let mut fields = BTreeMap::new();
            for m in members {
                let Some(mn) = &m.name else { continue };
                flatten_member(module, m.offset, mn, m.ty, &mut fields)?;
            }
            out.insert(name.clone(), fields);
        }
    }
    Ok(out)
}


/// 成员展开：子 struct 递归内联（字段名 = 父名.子名），叶子记 (绝对 offset, info)。
fn flatten_member(
    module: &ir::Module,
    base: u32,
    path: &str,
    ty: naga::Handle<ir::Type>,
    fields: &mut BTreeMap<String, (u32, MemberInfo)>,
) -> Result<()> {
    let t = &module.types[ty];
    if let ir::TypeInner::Struct { members, .. } = &t.inner {
        for m in members {
            let Some(mn) = &m.name else { continue };
            flatten_member(module, base + m.offset, &format!("{path}.{mn}"), m.ty, fields)?;
        }
        return Ok(());
    }
    fields.insert(path.to_string(), (base, member_info(module, ty)?));
    Ok(())
}

/// group(1) 的资源 binding 号 + 类型（从 WGSL 全局变量反射，声明序）。
struct TexBinding {
    binding: u32,
    _sampler: bool,
}

fn reflect_group1(module: &ir::Module) -> Vec<TexBinding> {    let mut out: Vec<TexBinding> = module
        .global_variables
        .iter()
        .filter(|(_, v)| v.space == ir::AddressSpace::Handle)
        .filter(|(_, v)| v.binding.map(|b| b.group) == Some(1))
        .map(|(_, v)| TexBinding { binding: v.binding.unwrap().binding, _sampler: false })
        .collect();
    out.sort_by_key(|t| t.binding);
    out
}

fn write_member(buf: &mut [u8], offset: u32, info: &MemberInfo, val: &FieldVal, what: &str) -> Result<()> {
    let flat: Vec<f64> = match val {
        FieldVal::Num(n) => vec![*n],
        FieldVal::Vec(v) => v.clone(),
    };
    let need = match info.array_len {
        Some(len) => len as usize * info.components as usize,
        None => info.components as usize,
    };
    if flat.len() != need {
        bail!("{what}: value len {} != layout len {need}", flat.len());
    }
    for (i, v) in flat.iter().enumerate() {
        let o = offset as usize + i * 4;
        match info.kind {
            Kind::Float => buf[o..o + 4].copy_from_slice(&(*v as f32).to_le_bytes()),
            Kind::Int => buf[o..o + 4].copy_from_slice(&(*v as i32).to_le_bytes()),
            Kind::Uint => buf[o..o + 4].copy_from_slice(&(*v as u32).to_le_bytes()),
            Kind::Bool => buf[o..o + 4].copy_from_slice(&((*v != 0.0) as u32).to_le_bytes()),
        }
    }
    Ok(())
}


fn json_to_val(v: &serde_json::Value) -> FieldVal {
    match v {
        serde_json::Value::Number(n) => FieldVal::Num(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::Array(a) => FieldVal::Vec(a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()),
        _ => FieldVal::Num(0.0),
    }
}

const VERTEX_WGSL: &str = r#"
struct VSOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VSOut {
    var pos = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    var uv = array<vec2f, 3>(vec2f(0.0, 1.0), vec2f(2.0, 1.0), vec2f(0.0, -1.0));
    var out: VSOut;
    out.pos = vec4f(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}
"#;

struct PassPlan {
    key: String,
    pipeline: wgpu::RenderPipeline,
    reads: Vec<String>,
}

fn main() -> Result<()> {
    let mut wgsl_dir = PathBuf::from("wgsl");
    let mut out_dir = PathBuf::from("out-ours");
    let mut only: Option<Vec<String>> = None;
    let mut all = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--wgsl" => wgsl_dir = PathBuf::from(args.next().context("--wgsl needs a value")?),
            "--out" => out_dir = PathBuf::from(args.next().context("--out needs a value")?),
            "--only" => {
                let v = args.next().context("--only needs a value")?;
                only = Some(v.split(',').map(str::to_string).collect());
            }
            "--all" => all = true,
            other => bail!("unknown arg: {other}"),
        }
    }

    // lavapipe：与 oracle 同一套软件 Vulkan（NVIDIA ICD OOM）。
    if std::env::var("VK_ICD_FILENAMES").is_err() && Path::new("/usr/share/vulkan/icd.d/lvp_icd.json").exists() {
        // SAFETY: 单线程启动期设置。
        unsafe { std::env::set_var("VK_ICD_FILENAMES", "/usr/share/vulkan/icd.d/lvp_icd.json") };
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .context("no vulkan adapter")?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("ours"),
            required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        },
    ))?;
    let info = adapter.get_info();
    println!("adapter: {} {} {:?}", info.name, info.vendor, info.backend);

    let mut components: Vec<String> = Vec::new();
    if all {
        for e in std::fs::read_dir(&wgsl_dir)? {
            let p = e?.path();
            if p.is_dir() && p.join("passes.json").exists() {
                components.push(p.file_name().unwrap().to_string_lossy().into_owned());
            }
        }
        components.sort();
    } else if let Some(list) = &only {
        components = list.clone();
    } else {
        bail!("give --all or --only A,B")
    }

    let mut skipped = 0usize;
    for name in &components {
        let dir = wgsl_dir.join(name);
        let passes: Passes = serde_json::from_str(&std::fs::read_to_string(dir.join("passes.json"))?)?;
        let has_compute_textures = passes.final_reads.iter().any(|k| k.starts_with("compute"))
            || passes.rtt.iter().any(|rp| rp.reads.iter().any(|k| k.starts_with("compute")));
        if passes.compute_steps > 0 || has_compute_textures {
            skipped += 1;
            continue;
        }
        match render_component(&device, &queue, &wgsl_dir, name, &out_dir) {
            Ok(()) => println!("✓ {name}"),
            Err(e) => println!("✗ {name}: {e:#}"),
        }
    }
    println!("done, skipped (compute/texture): {skipped}");
    Ok(())
}

fn render_component(device: &wgpu::Device, queue: &wgpu::Queue, wgsl_dir: &Path, name: &str, out_dir: &Path) -> Result<()> {
    let dir = wgsl_dir.join(name);
    let passes: Passes = serde_json::from_str(&std::fs::read_to_string(dir.join("passes.json"))?)?;
    let values: Values = serde_json::from_str(&std::fs::read_to_string(dir.join("values.json"))?)?;

    let first_src = std::fs::read_to_string(dir.join("shader-0.wgsl"))?;
    let first_module = reflect(&first_src)?;
    let structs = reflect_structs(&first_module)?;
    let combined_member_structs = reflect_combined_members(&first_module)?;

    // uniform buffer 初值
    let sys_struct = combined_member_structs.get("_sys").context("no _sys member")?.clone();
    let sys = structs.get(&sys_struct).context("no _sys struct")?;
    let uniform_size = {
        // combined 的 size：取所有成员 offset+size 的最大者。直接从 struct "combined" 拿 span。
        // 导出的 uniform struct 名固定是 combined（typegpu 产出）。
        let c = structs.get("combined").context("no combined struct")?;
        let mut end = 0u32;
        for (off, info) in c.values() {
            let sz = match info.array_len {
                Some(n) => n * info.components,
                None => info.components,
            };
            end = end.max(off + sz * 4);
        }
        // 对齐 16
        (end + 15) / 16 * 16
    };
    let mut ubuf = vec![0u8; uniform_size as usize];

    // combined 的展平叶子表：key 形如 "n_n0.colorA"，offset 是 combined 内绝对偏移。
    // values.json 的键（n_root/n_n0/_sys）是 combined 成员名，子 struct 内字段相对其基址。
    let combined_flat = structs.get("combined").context("no combined struct")?.clone();
    let member_base = |member: &str| -> Result<u32> {
        // 成员基址 = 该成员任一叶子的绝对 offset 减去其子 struct 内相对 offset；
        // 直接取 "member." 前缀叶子中最小 offset 再减 0 不稳，改从原始 struct 布局算：
        // 简化：combined 展平叶子的绝对 offset 本身就带基址，直接拼 key 查即可。
        let _ = member;
        bail!("unused")
    };
    let _ = member_base;

    let write_group = |ubuf: &mut Vec<u8>, prefix: &str, fields: &BTreeMap<String, FieldVal>| -> Result<()> {
        for (fname, val) in fields {
            let key = format!("{prefix}.{fname}");
            let (off, info) = combined_flat
                .get(&key)
                .with_context(|| format!("{key} not in combined flat layout"))?;
            write_member(ubuf, *off, info, val, &key).with_context(|| format!("{name} uniform write"))?;
        }
        Ok(())
    };

    // sysInitial 先写底（viewportSize/aspect/pointer 等不在 values.json 里）。
    let layout: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("layout.json"))?)?;
    if let Some(sys_init) = layout.get("sysInitial") {
        let mut sys_fields = BTreeMap::new();
        if let Some(obj) = sys_init.as_object() {
            for (fname, val) in obj {
                sys_fields.insert(fname.clone(), json_to_val(val));
            }
        }
        write_group(&mut ubuf, "_sys", &sys_fields)?;
    }

    // _sys 不从 values.json 写：导出器的 defineSystem 默认初值是 [1,1]（uniformStore.ts
    // 的占位），会覆盖上面 sysInitial 的真实 512×512。_sys 的动态字段只有 time，帧循环写。
    for (struct_key, fields) in &values {
        if struct_key == "_sys" {
            continue;
        }
        write_group(&mut ubuf, struct_key, fields)?;
    }
    if std::env::var("OURS_DUMPBIN").is_ok() {
        std::fs::write(std::env::var("OURS_DUMPBIN").unwrap(), &ubuf)?;
    }
    let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("uniforms"),
        contents: &ubuf,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // BGL0/uniform_bg 延后到 pipeline 建好后用其自动 BGL 创建（见 uniform_bg）。

    let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("linearClamp"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    // RTT 纹理（rgba16float 对齐库 rttFormat）
    let rtt_textures: BTreeMap<String, wgpu::Texture> = passes
        .rtt
        .iter()
        .map(|rp| {
            (
                rp.key.clone(),
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&rp.key),
                    size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                }),
            )
        })
        .collect();

    let final_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("final"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let final_view = final_tex.create_view(&Default::default());

    let vmodule = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(VERTEX_WGSL.into()),
    });

    let make_pipeline = |module: &wgpu::ShaderModule, entry: &str, color: wgpu::TextureFormat| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &vmodule,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };

    // rtt passes
    let mut rtt_plans: Vec<PassPlan> = Vec::new();
    for rp in &passes.rtt {
        let src = std::fs::read_to_string(dir.join(format!("shader-{}.wgsl", rp.index)))?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let entry = format!("rtt_{}", rp.key);
        let pipeline = make_pipeline(&module, &entry, wgpu::TextureFormat::Rgba16Float);
        rtt_plans.push(PassPlan {
            key: rp.key.clone(),
            pipeline,
            reads: rp.reads.clone(),
        });
    }
    // final pass
    let final_entry_src = {
        let n = passes.rtt.len();
        std::fs::read_to_string(dir.join(format!("shader-{n}.wgsl")))?
    };
    let final_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(final_entry_src.clone().into()),
    });
    let final_pipeline = make_pipeline(&final_module, "final_1", wgpu::TextureFormat::Rgba8Unorm);

    // group1 texture bind groups（binding 号来自各 pass module 的反射）
    let make_tex_bg = |src_module: &ir::Module, pipeline: &wgpu::RenderPipeline, reads: &[String]| -> Result<Option<wgpu::BindGroup>> {
        let bindings = reflect_group1(src_module);
        if bindings.is_empty() {
            return Ok(None);
        }
        if bindings.len() != reads.len() {
            bail!("group1 bindings {} != reads {}", bindings.len(), reads.len());
        }
        let views: Vec<wgpu::TextureView> = bindings
            .iter()
            .zip(reads)
            .map(|(_, key)| {
                let t = rtt_textures.get(key).with_context(|| format!("texture key {key}"))?;
                Ok(t.create_view(&Default::default()))
            })
            .collect::<Result<Vec<_>>>()?;
        let entries: Vec<wgpu::BindGroupEntry> = bindings
            .iter()
            .zip(reads)
            .enumerate()
            .map(|(i, (b, _))| wgpu::BindGroupEntry {
                binding: b.binding,
                resource: wgpu::BindingResource::TextureView(&views[i]),
            })
            .collect();
        let bgl = pipeline.get_bind_group_layout(1);
        Ok(Some(device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &bgl, entries: &entries })))
    };

    // group2 binding 号 + 名字从 naga 反射（全部同一 sampler 资源）。
    fn reflect_group2_named(src: &str) -> Result<Vec<(u32, String)>> {
        let mut fe = naga::front::wgsl::Frontend::new();
        let module = fe.parse(src).map_err(|e| anyhow::anyhow!("wgsl re-parse: {e:?}"))?;
        let mut out = Vec::new();
        for (_, v) in module.global_variables.iter() {
            if v.space == ir::AddressSpace::Handle {
                if let Some(b) = v.binding {
                    if b.group == 2 {
                        out.push((b.binding, v.name.clone().unwrap_or_default()));
                    }
                }
            }
        }
        out.sort();
        Ok(out)
    }

    // rtt 源码重解析（为 group1/group2 反射各留 naga module）
    let rtt_naga: Vec<ir::Module> = passes
        .rtt
        .iter()
        .map(|rp| reflect(&std::fs::read_to_string(dir.join(format!("shader-{}.wgsl", rp.index)))?))
        .collect::<Result<Vec<_>>>()?;
    let final_naga = reflect(&final_entry_src)?;

    let rtt_tex_bgs = rtt_plans
        .iter()
        .zip(&rtt_naga)
        .map(|(plan, m)| make_tex_bg(m, &plan.pipeline, &plan.reads))
        .collect::<Result<Vec<_>>>()?;
    let final_tex_bg = make_tex_bg(&final_naga, &final_pipeline, &passes.final_reads)?;

    let make_samp_bg_from = |names: &[(u32, String)], pipeline: &wgpu::RenderPipeline| -> Option<wgpu::BindGroup> {
        if names.is_empty() {
            return None;
        }
        let bes: Vec<wgpu::BindGroupEntry> = names
            .iter()
            .map(|(b, _)| wgpu::BindGroupEntry { binding: *b, resource: wgpu::BindingResource::Sampler(&linear_sampler) })
            .collect();
        let bgl = pipeline.get_bind_group_layout(2);
        Some(device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &bgl, entries: &bes }))
    };
    let rtt_samp_bgs: Vec<Option<wgpu::BindGroup>> = passes
        .rtt
        .iter()
        .enumerate()
        .map(|(i, rp)| {
            let src = std::fs::read_to_string(dir.join(format!("shader-{}.wgsl", rp.index)))?;
            Ok(make_samp_bg_from(&reflect_group2_named(&src)?, &rtt_plans[i].pipeline))
        })
        .collect::<Result<Vec<_>>>()?;
    let final_samp_bg = make_samp_bg_from(&reflect_group2_named(&final_entry_src)?, &final_pipeline);



    // 各 pass 用自己 pipeline 的 group0 BGL 建 uniform bind group（自动布局按 pass 可能
    // 有 stage/min_binding_size 差异；跨 pipeline 共享一个 bg 会触发 Exclusive pipelines 校验）。
    let uniform_bg_for = |pipeline: &wgpu::RenderPipeline| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        })
    };
    let rtt_uniform_bgs: Vec<wgpu::BindGroup> = rtt_plans.iter().map(|p| uniform_bg_for(&p.pipeline)).collect();
    let final_uniform_bg = uniform_bg_for(&final_pipeline);

    // —— 帧循环
    let max_frame = 90usize;
    let sample_frames = [10usize, 45, 90];
    let out_cdir = out_dir.join(name).join("default");
    std::fs::create_dir_all(&out_cdir)?;

    let speed = values
        .get("n_n0")
        .and_then(|m| m.get("speed"))
        .and_then(|v| match v {
            FieldVal::Num(n) => Some(*n as f32),
            _ => None,
        })
        .unwrap_or(0.0);
    let n0_struct = combined_member_structs.get("n_n0").context("no n_n0 member")?.clone();
    let n0 = structs.get(&n0_struct).context("no n_n0 struct")?;
    let has_anim_time = n0.contains_key("_animTime");
    let t_off = combined_flat.get("_sys.time").map(|(o, _)| *o as u64);
    let at_off = combined_flat.get("n_n0._animTime").map(|(o, _)| *o as u64);
    println!("t_off={t_off:?} at_off={at_off:?} has_anim={has_anim_time} speed={speed}");
    if std::env::var("OURS_DUMP").is_ok() {
        let o = |k: &str| combined_flat.get(k).map(|(o, _)| *o).unwrap_or(999);
        let f32at = |k: &str| {
            let off = o(k) as usize;
            f32::from_le_bytes(ubuf[off..off + 4].try_into().unwrap())
        };
        for k in ["_sys.time", "_sys.viewportSize", "_sys.aspect", "n_n0.colorA", "n_n0.center", "n_n0._animTime", "n_root._opacity", "n_n0.meshAnchors", "n_n0.ambPosA", "n_n0._f3p0"] {
            let off = o(k) as usize;
            let n = combined_flat.get(k).map(|(_, i)| i.components as usize).unwrap_or(0);
            let vals: Vec<f32> = (0..n).map(|i| f32::from_le_bytes(ubuf[off + i * 4..off + i * 4 + 4].try_into().unwrap())).collect();
            println!("{k} @ {off}: {vals:?}");
        }
    }

    let mut anim_time = 0.0f32;
    let mut time_acc = 0.0f32;
    // 时间偏移实验：OURS_T0 秒（可负）加到 animTime 上，用于对齐oracle 相位。
    let t0: f32 = std::env::var("OURS_T0").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    anim_time = t0;

    // 采样帧的 extraField CPU 写入（导出器 extra.json）：真 renderer 在 onBeforeRender 里经
    // setExtraField 写 uniform（scatteredAnchors/conv*/Form3D 等），key 形如 "n0\u0000meshAnchors"。
    let extra_frames: std::collections::BTreeMap<u32, std::collections::BTreeMap<String, serde_json::Value>> =
        std::fs::read_to_string(dir.join("extra.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

    for f in 1..=max_frame {
        time_acc += (1.0 / FPS) as f32;
        if has_anim_time {
            anim_time += (1.0 / FPS) as f32 * speed;
        }
        if let Some(o) = t_off {
            queue.write_buffer(&uniform_buf, o, &time_acc.to_le_bytes());
        }
        if let (Some(o), true) = (at_off, has_anim_time) {
            queue.write_buffer(&uniform_buf, o, &anim_time.to_le_bytes());
        }
        if let Some(fields) = extra_frames.get(&(f as u32)) {
            // 整块写 ubuf 前先把当前帧 time/_animTime 同步进去（这两个平时直接写 GPU buffer，
            // ubuf 里还是 initial 值，整块写会把 GPU 上的时间覆盖回 0）。
            if let Some(o) = t_off {
                ubuf[o as usize..o as usize + 4].copy_from_slice(&time_acc.to_le_bytes());
            }
            if let (Some(o), true) = (at_off, has_anim_time) {
                ubuf[o as usize..o as usize + 4].copy_from_slice(&anim_time.to_le_bytes());
            }
            for (k, v) in fields {
                let (nid, fname) = k.split_once('\u{0}').context("extra key missing NUL")?;
                let (off, info) = combined_flat
                    .get(&format!("n_{nid}.{fname}"))
                    .with_context(|| format!("extra {k} not in combined layout"))?;
                let fv = json_to_val(v);
                write_member(&mut ubuf, *off, info, &fv, k)
                    .with_context(|| format!("extra write {k}"))?;
            }
            queue.write_buffer(&uniform_buf, 0, &ubuf);
            if std::env::var("OURS_DUMP").is_ok() {
                for (k, _v) in fields {
                    let (nid, fname) = k.split_once('\u{0}').context("extra key missing NUL")?;
                    let (off, info) = combined_flat.get(&format!("n_{nid}.{fname}")).unwrap();
                    let o = *off as usize;
                    let vals: Vec<f32> = (0..info.components as usize * info.array_len.unwrap_or(1) as usize)
                        .map(|i| f32::from_le_bytes(ubuf[o + i * 4..o + i * 4 + 4].try_into().unwrap()))
                        .collect();
                    println!("extra f{f} {k} @ {off}: {vals:?}");
                }
            }
        }

        let mut enc = device.create_command_encoder(&Default::default());
        for (i, plan) in rtt_plans.iter().enumerate() {
            let tex = rtt_textures.get(&plan.key).unwrap();
            let view = tex.create_view(&Default::default());
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rpass.set_pipeline(&plan.pipeline);
            rpass.set_bind_group(0, &rtt_uniform_bgs[i], &[]);
            if let Some(b) = &rtt_tex_bgs[i] {
                rpass.set_bind_group(1, b, &[]);
            }
            if let Some(b) = &rtt_samp_bgs[i] {
                rpass.set_bind_group(2, b, &[]);
            }
            rpass.draw(0..3, 0..1);
        }
        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &final_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rpass.set_pipeline(&final_pipeline);
            rpass.set_bind_group(0, &final_uniform_bg, &[]);
            if let Some(b) = &final_tex_bg {
                rpass.set_bind_group(1, b, &[]);
            }
            if let Some(b) = &final_samp_bg {
                rpass.set_bind_group(2, b, &[]);
            }
            rpass.draw(0..3, 0..1);
        }

        if sample_frames.contains(&f) {
            let bpr = (W * 4 + 255) / 256 * 256;
            let rbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (bpr * H) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: &final_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo {
                    buffer: &rbuf,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(H) },
                },
                wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
            );
            queue.submit([enc.finish()]);
            let slice = rbuf.slice(..);
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            slice.map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res);
            });
            device.poll(wgpu::PollType::wait_indefinitely())?;
            rx.recv().map_err(|_| anyhow::anyhow!("map callback lost"))?.map_err(|e| anyhow::anyhow!("map: {e:?}"))?;
            let data = slice.get_mapped_range().map_err(|e| anyhow::anyhow!("view: {e:?}"))?;
            let mut rgba = Vec::with_capacity((W * H * 4) as usize);
            for y in 0..H {
                let s = (y * bpr) as usize;
                rgba.extend_from_slice(&data[s..s + (W * 4) as usize]);
            }
            drop(data);
            rbuf.unmap();
            image::RgbaImage::from_raw(W, H, rgba)
                .context("image size")?
                .save(out_cdir.join(format!("f{f}.png")))?;
        } else {
            queue.submit([enc.finish()]);
        }
    }

    std::fs::write(
        out_cdir.join("manifest.json"),
        serde_json::json!({
            "name": name,
            "adapter": "lavapipe-wgpu",
            "format": "rgba8unorm",
            "fps": FPS,
            "sampleFrames": sample_frames,
        })
        .to_string(),
    )?;
    Ok(())
}
