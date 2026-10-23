//! opencat-shaders — engine 侧 shader 渲染后端（wgpu）。
//!
//! 对称于 `opencat-three`（engine 侧 3D）：core 只把 **props 结构**识别成 IR
//! 派发过来，本 crate 扮演 typegpu 的角色，自己做 `props → WGSL` 烘焙，再在
//! wgpu 上跑出 straight-alpha RGBA8（`w*h*4`，未预乘）。
//!
//! 为什么这个 crate 必须自己生成 WGSL：schema.org 意义上的 shader 库
//! （`@coreroot`）把「组件 = 名词配方」编译成 WGSL。IR 若携带成品 WGSL，
//! core 就得内置一个 JS/typegpu 运行时；ir 携带 props 结构，则烘焙这件事自然
//! 落在 engine 侧的 Rust 里——与 `opencat-three` 把 Scene3 快照自己光栅化
//! 同一个形状。
//!
//! ## 三层分工
//!
//! - [`kit`]：GPU 函数库，逐字收割自真库编译产物（`tools/shader-diff/wgsl/`）。
//!   函数体是权威——改一个字符就可能让像素偏离快照。
//! - [`layout`]：单张字段表同时产出 WGSL 结构体声明与 uniform 字节，所以两者
//!   永远不会漂移（不需要 naga 反射去猜偏移）。
//! - 每个组件一个模块（[`aurora`] 等）：把真库 `packages/core/src/shaders/<C>/index.ts`
//!   的配方逐行翻译成「kit 函数调用 + 拼接文本」。
//!
//! ## 确定性
//!
//! 同一 props 在任一后端上逐字节一致：无时间、无 IO、无回调。与真库浏览器
//! 实现的差异是容差级（浮点运算顺序），对齐走 shader-diff 的 mae/p<8> 指标。

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

pub mod aurora;
pub mod colors;
pub mod filter;
pub mod kit;
pub mod vhs;
pub mod zoom_blur;
pub mod mesh_gradient;
pub mod layout;

pub use layout::{Field, Ty, Uniform, Val};

/// 一次烘焙的结果：按 pass 顺序排列的 WGSL 全集 + 共用的 uniform 字节。
///
/// `passes` 的最后一个是 final pass（写进 `Rgba8Unorm` 目标），前面的都是
/// `rtt` 中间 pass（`Rgba16Float`）。
pub struct Baked {
    pub passes: Vec<BakedPass>,
    pub uniform_bytes: Vec<u8>,
    /// final pass 读哪些 rtt（按 binding 序）。空 = 纯 generator，不读任何纹理。
    pub final_reads: Vec<String>,
    /// 第一个 rtt pass 是否要读**外层输入**（前一层/子树合成结果）。
    ///
    /// filter/warp 的真语义是「吃下面的东西」：快照烘焙时那个东西是
    /// `catalog.json` 的 `LinearGradient` 子层，所以在 `bake()` 里被画成
    /// `rtt_0`。运行时那个输入来自真实的上游，所以这里把 `rtt_0` 换成输入
    /// 纹理，`rtt_0` 那一 pass 整段跳过。
    pub consumes_input: bool,
}

pub struct BakedPass {
    /// 该 pass 的入口函数名（`final_1` / `rtt_rtt_0`）。
    pub entry: String,
    pub wgsl: String,
    /// 该 pass 里 `@group(1)` 采样纹理的名字（与 rtt key 同名）。
    pub reads: Vec<String>,
}

/// 组件的 props 值。键是 prop 名，值已经过与真库同口径的 transform
/// （颜色进 P3-linear、位置做 Y 翻转等），所以配方层只做算术。
pub type Props = BTreeMap<String, Val>;

/// 组装一个完整 pass 的 WGSL：kit 函数 + uniform 结构体 + 全局声明 + 额外声明 + 入口。
///
/// `extra_globals` 用于 rtt pass 的 `@group(1) var rtt_0` 这类绑定。
pub fn assemble(uniform: &Uniform, kit_fns: &[&'static str], extra_globals: &str, entry_wgsl: &str) -> String {
    let mut out = String::new();
    out.push_str(&uniform.wgsl());
    out.push_str(extra_globals);
    // 闭包展开：配方只列直接调用，依赖（及依赖的依赖）由这里按拓扑序补齐。
    for name in kit::closure_for(kit_fns) {
        let body = kit::lookup(name).unwrap_or_else(|| panic!("kit has no fn {name}"));
        out.push_str(body);
        out.push('\n');
    }
    out.push_str(entry_wgsl);
    out
}

/// 全屏三角形的顶点阶段。UV 的 y 轴按真库取向（0 = 底）。
pub const VERTEX_WGSL: &str = r#"struct VSOut {
  @builtin(position) pos: vec4f,
  @location(0) uv: vec2f,
}

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

/// 渲染一帧到 RGBA8。
///
/// `width`/`height` 进 `_sys.viewportSize`，同时决定目标尺寸。
/// rtt pass 产出的纹理 key：`rtt_rtt_0` → `rtt_0`（只去掉一层前缀）。
fn rtt_key(entry: &str) -> String {
    entry.strip_prefix("rtt_").unwrap_or(entry).to_string()
}

pub fn render(baked: &Baked, width: u32, height: u32) -> Result<Vec<u8>> {
    render_with_input(baked, width, height, None)
}

/// 同 [`render`]，但允许把外层图像（straight-alpha RGBA8，`w*h*4`）当作第一个
/// rtt key 的输入。`consumes_input` 的组件（VHS / ZoomBlur）因为自己不再画
/// 那个 pass，必须提供；其余组件忽略这个参数。
pub fn render_with_input(
    baked: &Baked,
    width: u32,
    height: u32,
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    if width == 0 || height == 0 {
        bail!("shader render needs a non-zero canvas size, got {width}x{height}");
    }
    // 没给输入时退回到烘焙时那个内建子层（快照烘焙就是这样比的）；运行期
    // 由 [`render_target`] 拦下真正缺上游的情况。
    let input = if let (true, Some(rgba)) = (baked.consumes_input, input) {
        if rgba.len() != (width as usize) * (height as usize) * 4 {
            bail!(
                "shader 输入尺寸不匹配：期望 {}x{} ({}) 字节，拿到 {} 字节",
                width, height, (width as usize) * (height as usize) * 4, rgba.len()
            );
        }
        Some(rgba)
    } else {
        None
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .context("no wgpu adapter for shader rendering")?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("opencat-shaders"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
    }))?;

    let uniform_buf = {
        use wgpu::util::DeviceExt;
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shader uniforms"),
            contents: &baked.uniform_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    };

    let vmodule = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vertex"),
        source: wgpu::ShaderSource::Wgsl(VERTEX_WGSL.into()),
    });

    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };

    // rtt 纹理按 key 建，供后续 pass 与 final 读。
    let mut rtt_tex: BTreeMap<String, wgpu::Texture> = BTreeMap::new();
    for pass in &baked.passes[..baked.passes.len() - 1] {
        for key in &pass.reads {
            rtt_tex.entry(key.clone()).or_insert_with(|| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(key),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            });
        }
        // rtt pass 自身产出同名纹理（pass 名即 key），这里保证它存在。
    }
    for pass in &baked.passes[..baked.passes.len() - 1] {
        // rtt pass 的 entry 形如 `rtt_rtt_0`，去掉一个 `rtt_` 前缀就是它产出的 key。
        let key = rtt_key(&pass.entry);
        rtt_tex.entry(key.clone()).or_insert_with(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&key),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        });
    }

    let final_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("final"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    // 外层输入（前一层 / 子树合成结果）：装进第一个 rtt key 的槽位，
    // 这样 filter/warp 的 final pass 不必区分「快照子层」与「真实上游」。
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    if let Some(rgba) = input {
        let key = rtt_key(&baked.passes[0].entry);
        let padded_bpr = (width * 4).div_ceil(256) * 256;
        let mut padded_bytes = Vec::with_capacity((padded_bpr * height) as usize);
        for y in 0..height {
            let start = (y * width * 4) as usize;
            padded_bytes.extend_from_slice(&rgba[start..start + (width * 4) as usize]);
            padded_bytes.resize(padded_bytes.len() + (padded_bpr - width * 4) as usize, 0);
        }
        let staging = {
            use wgpu::util::DeviceExt;
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("shader input"),
                contents: &padded_bytes,
                usage: wgpu::BufferUsages::COPY_SRC,
            })
        };
        let slot = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shader input"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded_bpr), rows_per_image: Some(height) },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &slot,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            size,
        );
        // filter/warp 自身的子层 pass 被真实上游取代，slot 直接占住那个 key。
        rtt_tex.insert(key, slot);
    }

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("linearClamp"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let make_pipeline = |wgsl: &str, entry: &str, format: wgpu::TextureFormat| -> Result<wgpu::RenderPipeline> {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(entry),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        Ok(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: None,
            vertex: wgpu::VertexState {
                module: &vmodule,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        }))
    };

    let n = baked.passes.len();
    for (i, pass) in baked.passes.iter().enumerate() {
        // 输入已经占住第一个 rtt key，对应那一 pass 不再跑。
        if input.is_some() && i == 0 && n > 1 {
            continue;
        }
        let format = if i + 1 == n { wgpu::TextureFormat::Rgba8Unorm } else { wgpu::TextureFormat::Rgba16Float };
        let pipeline = make_pipeline(&pass.wgsl, &pass.entry, format).with_context(|| {
            format!("pass {} ({}) pipeline", i, pass.entry)
        })?;
        let ubg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });
        let target_view = if i + 1 == n {
            final_tex.create_view(&Default::default())
        } else {
            let key = rtt_key(&pass.entry);
            rtt_tex[&key].create_view(&Default::default())
        };
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&pass.entry),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &ubg, &[]);
        // group(1) 纹理 + group(2) 采样器：按 pass 声明的 reads 顺序绑定。
        let tex_bgs: Vec<wgpu::BindGroup> = pass
            .reads
            .iter()
            .map(|k| {
                let view = rtt_tex[k].create_view(&Default::default());
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.get_bind_group_layout(1),
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
                })
            })
            .collect();
        for (gi, bg) in tex_bgs.iter().enumerate() {
            rpass.set_bind_group(1 + gi as u32, bg, &[]);
        }
        if !pass.reads.is_empty() {
            let sbg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(1 + pass.reads.len() as u32),
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&sampler) }],
            });
            rpass.set_bind_group(1 + pass.reads.len() as u32, &sbg, &[]);
        }
        rpass.draw(0..3, 0..1);
    }

    // readback
    let bpr = width * 4;
    let padded = (bpr + 255) / 256 * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &final_tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(height) },
        },
        size,
    );
    queue.submit(Some(encoder.finish()));

    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    readback.slice(..).map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).context("wgpu poll")?;
    rx.recv()
        .context("map 回调丢失")?
        .context("读回映射失败")?;
    let data = readback.slice(..).get_mapped_range().context("读回视图失败")?;
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let start = (y * padded) as usize;
        out.extend_from_slice(&data[start..start + bpr as usize]);
    }
    drop(data);
    readback.unmap();
    Ok(out)
}

/// 把一个 `<Shader>` 层栈光栅化成一张 straight-alpha RGBA8（`w*h*4`）。
///
/// `layers` 按 z 序排列（IR 里就是声明顺序）：
/// - generator 层（Aurora / MeshGradient）整帧重画，覆盖底下的内容；
/// - filter / warp 层（VHS / ZoomBlur）吃前一层的结果，所以必须有一个输入。
///
/// 时间基准：IR 只带 `time`（该层的 f(t) 值），`animatedTime` 类的累积时钟
/// （`_animTime`）在这里算——真库按帧推进 `Δt × speed`，同一次编译内多个层
/// 各自累积。
pub fn render_target(
    layers: &[ShaderLayer],
    width: u32,
    height: u32,
    time: f32,
) -> Result<Vec<u8>> {
    if layers.is_empty() {
        bail!("shader target 至少要有一层");
    }
    let (w, h) = (width as f32, height as f32);
    let mut rgba: Option<Vec<u8>> = None;
    for layer in layers {
        let props = props_of(&layer.component, &layer.props)?;
        // 帧时钟：`shaderProps({ time })` 可以覆盖它（动画加速/暂停），
        // 未覆盖时就是 hook 的时钟。
        let t = props.get("time").map(|v| v.as_f32()).unwrap_or(time);
        let baked = match layer.component.as_str() {
            "Aurora" => aurora::bake(&props, w, h, t),
            "MeshGradient" => mesh_gradient::bake(&props, w, h, t),
            "VHS" => vhs::bake(&props, w, h, t),
            "ZoomBlur" => zoom_blur::bake(&props, w, h, t),
            other => bail!("shader 组件 {other} 还没有烘焙器（已支持 Aurora / MeshGradient / VHS / ZoomBlur）"),
        };
        // filter/warp 的真语义是「吃下面的东西」：它的输入就是上一层的成品。
        if baked.consumes_input && rgba.is_none() {
            bail!(
                "shader 层 {} 要吃上游图像，但它是这个 <Shader> 的第一层——在它前面放一个 generator（Aurora / MeshGradient）或一块 <img>",
                layer.component
            );
        }
        rgba = Some(render_with_input(&baked, width, height, rgba.as_deref())?);
    }
    Ok(rgba.expect("layers 非空"))
}

/// JSON props → 该组件的 [`Props`]。缺的键从组件默认值补，所以示例只写想改的。
fn props_of(component: &str, json: &serde_json::Value) -> Result<Props> {
    let mut props = match component {
        "Aurora" => aurora::default_props(),
        "MeshGradient" => mesh_gradient::default_props(),
        "VHS" => vhs::default_props(),
        "ZoomBlur" => zoom_blur::default_props(),
        other => bail!("shader 组件 {other} 还没有烘焙器"),
    };
    if let serde_json::Value::Object(map) = json {
        for (k, v) in map {
            match v {
                serde_json::Value::Number(n) => {
                    props.insert(k.clone(), Val::F(n.as_f64().unwrap_or(0.0) as f32));
                }
                serde_json::Value::Array(a) => {
                    let vals: Vec<f32> = a
                        .iter()
                        .map(|x| x.as_f64().unwrap_or(0.0) as f32)
                        .collect();
                    props.insert(k.clone(), Val::Fs(vals));
                }
                // 颜色字符串（`#a533f8`）进 P3-linear，与真库 transform 同口径。
                serde_json::Value::String(s) => {
                    if let Some(rgba) = parse_hex_color(s) {
                        props.insert(k.clone(), Val::Fs(rgba));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(props)
}

/// `<Shader>` 的一层：shaders 库组件名 + 该层的 props（JSON，未 transform）。
///
/// 与 `opencat_core::ir::shader::ShaderLayer` 同形；本 crate 不依赖 core，
/// 由 engine 在调用点把 IR 结构搬过来。
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderLayer {
    pub component: String,
    pub props: serde_json::Value,
}

impl From<(String, serde_json::Value)> for ShaderLayer {
    fn from((component, props): (String, serde_json::Value)) -> Self {
        ShaderLayer { component, props }
    }
}

fn parse_hex_color(s: &str) -> Option<Vec<f32>> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let rgb = [
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ];
    let alpha = if hex.len() == 8 {
        u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0
    } else {
        1.0
    };
    Some(colors::p3_linear_rgba(rgb, alpha))
}
