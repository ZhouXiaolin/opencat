//! WebGPU 命令流执行器 —— `wgpu_opcode` 流到 wgpu crate 的一对一映射。
//!
//! 与 GL 执行器（`stream_executor.rs`）同契约：core 的录制面
//! （`webgpu_api.js`）把浏览器 WebGPU 调用序列编码成 `[opcode][payload]`
//! 流；这里逐 op 重放到真 wgpu 设备上。句柄是 realm 级持久 id，资源映射表
//! 跨帧存活（g-device-api 的 fallback 纹理等只在首帧录制）；绘制命令每帧
//! 自足。输出 v1 走 CPU readback（copy → mapAsync → 行重排 → BGRA→RGBA），
//! 回填 `FrameMediaPlan::generated_images` 的占位条目（`gpu=None`）——
//! 纹理直通 Skia 的零拷贝路径后续按需做。
//!
//! 与 GL 执行器的刻意差异：无共享上下文（无 make_current/restore 纪律）；
//! render pass 命令「录到 PASS_END、QUEUE_SUBMIT 时重放」——wgpu 的
//! `RenderPass` 借用 encoder，无法跨命令存进映射表，而流序保证 pass 命令
//! 连续成段，重放语义与原生 begin/end 一致。
//!
//! 枚举映射：wgpu 30 的枚举不再实现 `FromStr`，规格名（'bgra8unorm'、
//! 'triangle-list'…）到手写解析表（`spec` 模块），与 WebGPU 规格拼写一致。
//!
//! 错误 fail-loud：描述符缺字段、未知句柄一律 anyhow 带流上下文冒泡；
//! uncaptured error 接 stderr。

use std::collections::HashMap;
use std::num::NonZeroU64;

use anyhow::{Result, anyhow, bail};
use opencat_core::ir::draw_types::TableRange;
use opencat_core::ir::gpu_stream::{GpuStream, wgpu_opcode as W};
use serde_json::Value;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, BlendState, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoder,
    CommandEncoderDescriptor, CompareFunction, ComputePipeline, DepthBiasState, DepthStencilState,
    DeviceDescriptor, Extent3d, FragmentState, Instance, InstanceDescriptor, LoadOp, MapMode,
    MultisampleState, Operations, Origin3d, PipelineCompilationOptions, PipelineLayout,
    PipelineLayoutDescriptor, PollError, PollType, PowerPreference, PrimitiveState,
    PrimitiveTopology, QuerySet, QuerySetDescriptor, Queue, RenderPass,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderModule,
    ShaderSource, StencilFaceState, StencilOperation, StencilState, StoreOp,
    TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

// ── 规格名 → wgpu 枚举解析表（wgpu 30 无 FromStr）────────────────────

mod spec {
    use super::*;

    type P<T> = fn(&str) -> Option<T>;

    pub fn texture_format(s: &str) -> Option<TextureFormat> {
        Some(match s {
            "r8unorm" => TextureFormat::R8Unorm,
            "r8snorm" => TextureFormat::R8Snorm,
            "r8uint" => TextureFormat::R8Uint,
            "r8sint" => TextureFormat::R8Sint,
            "r16uint" => TextureFormat::R16Uint,
            "r16sint" => TextureFormat::R16Sint,
            "r16float" => TextureFormat::R16Float,
            "rg8unorm" => TextureFormat::Rg8Unorm,
            "rg8snorm" => TextureFormat::Rg8Snorm,
            "rg8uint" => TextureFormat::Rg8Uint,
            "rg8sint" => TextureFormat::Rg8Sint,
            "r32uint" => TextureFormat::R32Uint,
            "r32sint" => TextureFormat::R32Sint,
            "r32float" => TextureFormat::R32Float,
            "rg16uint" => TextureFormat::Rg16Uint,
            "rg16sint" => TextureFormat::Rg16Sint,
            "rg16float" => TextureFormat::Rg16Float,
            "rgba8unorm" => TextureFormat::Rgba8Unorm,
            "rgba8unorm-srgb" => TextureFormat::Rgba8UnormSrgb,
            "rgba8snorm" => TextureFormat::Rgba8Snorm,
            "rgba8uint" => TextureFormat::Rgba8Uint,
            "rgba8sint" => TextureFormat::Rgba8Sint,
            "bgra8unorm" => TextureFormat::Bgra8Unorm,
            "bgra8unorm-srgb" => TextureFormat::Bgra8UnormSrgb,
            "rgb9e5ufloat" => TextureFormat::Rgb9e5Ufloat,
            "rgb10a2uint" => TextureFormat::Rgb10a2Uint,
            "rgb10a2unorm" => TextureFormat::Rgb10a2Unorm,
            "rg11b10ufloat" => TextureFormat::Rg11b10Ufloat,
            "rg32uint" => TextureFormat::Rg32Uint,
            "rg32sint" => TextureFormat::Rg32Sint,
            "rg32float" => TextureFormat::Rg32Float,
            "rgba16uint" => TextureFormat::Rgba16Uint,
            "rgba16sint" => TextureFormat::Rgba16Sint,
            "rgba16float" => TextureFormat::Rgba16Float,
            "rgba32uint" => TextureFormat::Rgba32Uint,
            "rgba32sint" => TextureFormat::Rgba32Sint,
            "rgba32float" => TextureFormat::Rgba32Float,
            "stencil8" => TextureFormat::Stencil8,
            "depth24plus" => TextureFormat::Depth24Plus,
            "depth24plus-stencil8" => TextureFormat::Depth24PlusStencil8,
            "depth32float" => TextureFormat::Depth32Float,
            "depth32float-stencil8" => TextureFormat::Depth32FloatStencil8,
            other => {
                // 压缩纹理族（bc*-*/etc2-*/astc-*）：语料 2D 图表不用，
                // 拼写透传 wgpu 的变体名（kebab→Pascal 首段）。
                let _ = other;
                return None;
            }
        })
    }

    pub fn address_mode(s: &str) -> Option<wgpu::AddressMode> {
        Some(match s {
            "clamp-to-edge" => wgpu::AddressMode::ClampToEdge,
            "repeat" => wgpu::AddressMode::Repeat,
            "mirror-repeat" => wgpu::AddressMode::MirrorRepeat,
            _ => return None,
        })
    }

    pub fn filter_mode(s: &str) -> Option<wgpu::FilterMode> {
        Some(match s {
            "nearest" | "point" => wgpu::FilterMode::Nearest,
            "linear" => wgpu::FilterMode::Linear,
            _ => return None,
        })
    }

    pub fn mipmap_filter(s: &str) -> Option<wgpu::MipmapFilterMode> {
        Some(match s {
            "nearest" | "point" => wgpu::MipmapFilterMode::Nearest,
            "linear" => wgpu::MipmapFilterMode::Linear,
            _ => return None,
        })
    }

    pub fn compare_function(s: &str) -> Option<CompareFunction> {
        Some(match s {
            "never" => CompareFunction::Never,
            "less" => CompareFunction::Less,
            "equal" => CompareFunction::Equal,
            "less-equal" | "lequal" => CompareFunction::LessEqual,
            "greater" => CompareFunction::Greater,
            "not-equal" | "notequal" => CompareFunction::NotEqual,
            "greater-equal" | "gequal" => CompareFunction::GreaterEqual,
            "always" => CompareFunction::Always,
            _ => return None,
        })
    }

    pub fn blend_factor(s: &str) -> Option<wgpu::BlendFactor> {
        Some(match s {
            "zero" => wgpu::BlendFactor::Zero,
            "one" => wgpu::BlendFactor::One,
            "src" => wgpu::BlendFactor::Src,
            "one-minus-src" => wgpu::BlendFactor::OneMinusSrc,
            "src-alpha" => wgpu::BlendFactor::SrcAlpha,
            "one-minus-src-alpha" => wgpu::BlendFactor::OneMinusSrcAlpha,
            "dst" => wgpu::BlendFactor::Dst,
            "one-minus-dst" => wgpu::BlendFactor::OneMinusDst,
            "dst-alpha" => wgpu::BlendFactor::DstAlpha,
            "one-minus-dst-alpha" => wgpu::BlendFactor::OneMinusDstAlpha,
            "src-alpha-saturated" => wgpu::BlendFactor::SrcAlphaSaturated,
            "constant" => wgpu::BlendFactor::Constant,
            "one-minus-constant" => wgpu::BlendFactor::OneMinusConstant,
            _ => return None,
        })
    }

    pub fn blend_operation(s: &str) -> Option<wgpu::BlendOperation> {
        Some(match s {
            "add" => wgpu::BlendOperation::Add,
            "subtract" => wgpu::BlendOperation::Subtract,
            "reverse-subtract" => wgpu::BlendOperation::ReverseSubtract,
            "min" => wgpu::BlendOperation::Min,
            "max" => wgpu::BlendOperation::Max,
            _ => return None,
        })
    }

    pub fn front_face(s: &str) -> Option<wgpu::FrontFace> {
        Some(match s {
            "ccw" => wgpu::FrontFace::Ccw,
            "cw" => wgpu::FrontFace::Cw,
            _ => return None,
        })
    }

    pub fn face(s: &str) -> Option<wgpu::Face> {
        Some(match s {
            "front" => wgpu::Face::Front,
            "back" => wgpu::Face::Back,
            _ => return None,
        })
    }

    pub fn index_format(s: &str) -> Option<wgpu::IndexFormat> {
        Some(match s {
            "uint16" => wgpu::IndexFormat::Uint16,
            "uint32" => wgpu::IndexFormat::Uint32,
            _ => return None,
        })
    }

    pub fn primitive_topology(s: &str) -> Option<PrimitiveTopology> {
        Some(match s {
            "point-list" => PrimitiveTopology::PointList,
            "line-list" => PrimitiveTopology::LineList,
            "line-strip" => PrimitiveTopology::LineStrip,
            "triangle-list" => PrimitiveTopology::TriangleList,
            "triangle-strip" => PrimitiveTopology::TriangleStrip,
            _ => return None,
        })
    }

    pub fn stencil_operation(s: &str) -> Option<StencilOperation> {
        Some(match s {
            "keep" => StencilOperation::Keep,
            "zero" => StencilOperation::Zero,
            "replace" => StencilOperation::Replace,
            "invert" => StencilOperation::Invert,
            "increment-clamp" => StencilOperation::IncrementClamp,
            "decrement-clamp" => StencilOperation::DecrementClamp,
            "increment-wrap" => StencilOperation::IncrementWrap,
            "decrement-wrap" => StencilOperation::DecrementWrap,
            _ => return None,
        })
    }

    pub fn texture_aspect(s: &str) -> Option<TextureAspect> {
        Some(match s {
            "all" => TextureAspect::All,
            "stencil-only" => TextureAspect::StencilOnly,
            "depth-only" => TextureAspect::DepthOnly,
            _ => return None,
        })
    }

    pub fn texture_view_dimension(s: &str) -> Option<TextureViewDimension> {
        Some(match s {
            "1d" => TextureViewDimension::D1,
            "2d" => TextureViewDimension::D2,
            "2d-array" => TextureViewDimension::D2Array,
            "cube" => TextureViewDimension::Cube,
            "cube-array" => TextureViewDimension::CubeArray,
            "3d" => TextureViewDimension::D3,
            _ => return None,
        })
    }

    pub fn vertex_format(s: &str) -> Option<VertexFormat> {
        Some(match s {
            "uint8x2" => VertexFormat::Uint8x2,
            "uint8x4" => VertexFormat::Uint8x4,
            "sint8x2" => VertexFormat::Sint8x2,
            "sint8x4" => VertexFormat::Sint8x4,
            "unorm8x2" => VertexFormat::Unorm8x2,
            "unorm8x4" => VertexFormat::Unorm8x4,
            "snorm8x2" => VertexFormat::Snorm8x2,
            "snorm8x4" => VertexFormat::Snorm8x4,
            "uint16x2" => VertexFormat::Uint16x2,
            "uint16x4" => VertexFormat::Uint16x4,
            "sint16x2" => VertexFormat::Sint16x2,
            "sint16x4" => VertexFormat::Sint16x4,
            "unorm16x2" => VertexFormat::Unorm16x2,
            "unorm16x4" => VertexFormat::Unorm16x4,
            "snorm16x2" => VertexFormat::Snorm16x2,
            "snorm16x4" => VertexFormat::Snorm16x4,
            "float16x2" => VertexFormat::Float16x2,
            "float16x4" => VertexFormat::Float16x4,
            "float32" => VertexFormat::Float32,
            "float32x2" => VertexFormat::Float32x2,
            "float32x3" => VertexFormat::Float32x3,
            "float32x4" => VertexFormat::Float32x4,
            "uint32" => VertexFormat::Uint32,
            "uint32x2" => VertexFormat::Uint32x2,
            "uint32x3" => VertexFormat::Uint32x3,
            "uint32x4" => VertexFormat::Uint32x4,
            "sint32" => VertexFormat::Sint32,
            "sint32x2" => VertexFormat::Sint32x2,
            "sint32x3" => VertexFormat::Sint32x3,
            "sint32x4" => VertexFormat::Sint32x4,
            "unorm10-10-10-2" => VertexFormat::Unorm10_10_10_2,
            "unorm8x4-bgra" => VertexFormat::Unorm8x4Bgra,
            _ => return None,
        })
    }

    pub fn vertex_step_mode(s: &str) -> Option<VertexStepMode> {
        Some(match s {
            "vertex" => VertexStepMode::Vertex,
            "instance" => VertexStepMode::Instance,
            _ => return None,
        })
    }

    pub fn texture_dimension(s: &str) -> Option<TextureDimension> {
        Some(match s {
            "1d" => TextureDimension::D1,
            "2d" => TextureDimension::D2,
            "3d" => TextureDimension::D3,
            _ => return None,
        })
    }

    /// 带 spec 名报错的通用包装。
    pub fn must<T>(what: &str, s: &str, p: P<T>) -> Result<T> {
        p(s).ok_or_else(|| anyhow!("无法解析 {what} '{s}'"))
    }
}

// ── 句柄类别（bind group entry 解析与 destroy 分派）──────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Buffer,
    Texture,
    TextureView,
    Sampler,
    BindGroupLayout,
    BindGroup,
    PipelineLayout,
    ShaderModule,
    RenderPipeline,
    ComputePipeline,
    QuerySet,
}

/// 跨帧持久资源映射（realm 句柄 → wgpu 资源）。
#[derive(Default)]
struct WgpuResources {
    buffers: HashMap<u32, Buffer>,
    textures: HashMap<u32, Texture>,
    views: HashMap<u32, TextureView>,
    samplers: HashMap<u32, Sampler>,
    bind_group_layouts: HashMap<u32, BindGroupLayout>,
    bind_groups: HashMap<u32, BindGroup>,
    pipeline_layouts: HashMap<u32, PipelineLayout>,
    shaders: HashMap<u32, ShaderModule>,
    /// WGSL 源文本（pipeline 创建时做 VertexState 补齐判别用，见
    /// create_render_pipeline 内的 G2 缺口注释）。
    shader_codes: HashMap<u32, String>,
    render_pipelines: HashMap<u32, RenderPipeline>,
    compute_pipelines: HashMap<u32, ComputePipeline>,
    query_sets: HashMap<u32, QuerySet>,
    kind_of: HashMap<u32, Kind>,
    /// bind group 的 desc JSON（回放期跨 pipeline 重绑时重建用，按
    /// (pipeline, index) 缓存）。
    bind_group_desc: HashMap<u32, Value>,
    /// 派生 BGL 的 bind group → 派生源 pipeline（wgpu 严格身份校验适配，
    /// 见 apply_pass_cmd 的 rebind）。
    bg_owner: HashMap<u32, u32>,
    /// auto 派生 layout 的 pipeline（desc 无 layout/'auto'）。Dawn 语义：
    /// 只接受自身派生 BGL 的 bind group，显式 BGL 绑上即整 buffer 拒绝。
    pipeline_auto: HashMap<u32, bool>,
}

/// 帧内录制状态（流尾即帧尾，逐帧重建）。
#[derive(Default)]
struct FrameState {
    encoders: HashMap<u32, Vec<u32>>, // encoder → 保序段列表
    segments: HashMap<u32, RecordedPass>,
    cmd_buffers: HashMap<u32, u32>, // commandBuffer → encoder
    out_handle: Option<u32>,        // swapchain 纹理桩句柄
}

/// desc_range = None 的段是 encoder 级拷贝段（无附件，命令直接打 encoder）。
struct RecordedPass {
    desc_range: Option<u32>,
    cmds: Vec<(u8, Vec<u8>)>,
}

pub struct WgpuStreamExecutor {
    rt: Option<WgpuRuntime>,
    res: WgpuResources,
    frame: FrameState,
    out_format: TextureFormat,
    out_usage: TextureUsages,
}

struct WgpuRuntime {
    device: wgpu::Device,
    queue: Queue,
}

impl WgpuStreamExecutor {
    pub fn new() -> Self {
        Self {
            rt: None,
            res: WgpuResources::default(),
            frame: FrameState::default(),
            out_format: TextureFormat::Bgra8Unorm,
            out_usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
        }
    }

    /// 惰性建设备：首个 WebGPU 流触发。Windows 默认 DX12（与 ref 腿
    /// Edge/Dawn 同后端族）；`OC_WGPU_BACKEND` 可强制（dx12/vulkan/gl）。
    fn ensure_runtime(&mut self) -> Result<()> {
        if self.rt.is_none() {
            let backends = match std::env::var("OC_WGPU_BACKEND").ok().as_deref() {
                Some("vulkan") => wgpu::Backends::VULKAN,
                Some("dx12") => wgpu::Backends::DX12,
                Some("gl") => wgpu::Backends::GL,
                _ => wgpu::Backends::PRIMARY,
            };
            let instance = Instance::new(InstanceDescriptor {
                backends,
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: Default::default(),
                backend_options: Default::default(),
                display: None,
            });
            let adapter = pollster::block_on(instance.request_adapter(
                &wgpu::RequestAdapterOptions {
                    power_preference: PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                },
            ))
            .map_err(|e| anyhow!("wgpu: request_adapter 失败: {e}"))?;
            let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
                label: Some("opencat-wgpu-stream"),
                required_features: Default::default(),
                required_limits: Default::default(),
                experimental_features: Default::default(),
                memory_hints: Default::default(),
                trace: match std::env::var("OC_WGPU_TRACE") {
                    Ok(dir) if !dir.is_empty() => wgpu::Trace::Directory(std::path::PathBuf::from(dir)),
                    _ => Default::default(),
                },
            }))
            .map_err(|e| anyhow!("wgpu: request_device 失败: {e}"))?;
            device.on_uncaptured_error(std::sync::Arc::new(|e| {
                eprintln!("wgpu uncaptured error: {e}");
            }));
            self.rt = Some(WgpuRuntime { device, queue });
        }
        Ok(())
    }

    /// 执行全部 WebGPU 流并回填占位（CPU readback，`gpu=None`）。
    pub fn execute(
        &mut self,
        streams: &[&GpuStream],
        plan: &mut opencat_core::ir::FrameMediaPlan,
    ) -> Result<()> {
        if streams.is_empty() {
            return Ok(());
        }
        self.ensure_runtime()?;
        // 借用分割：rt / res / frame 三段互不相交，循环内各自独占。
        let WgpuStreamExecutor { rt, res, frame, out_format, out_usage } = self;
        let rt = rt.as_ref().expect("runtime checked above");
        for stream in streams {
            if let Err(e) =
                execute_stream(rt, res, frame, *out_format, *out_usage, stream, plan)
            {
                bail!(
                    "webgpu stream '{}' ({}x{}): {e:#}",
                    stream.target_id,
                    stream.width,
                    stream.height
                );
            }
        }
        Ok(())
    }
}

fn execute_stream(
    rt: &WgpuRuntime,
    res: &mut WgpuResources,
    frame: &mut FrameState,
    mut out_format: TextureFormat,
    mut out_usage: TextureUsages,
    stream: &GpuStream,
    plan: &mut opencat_core::ir::FrameMediaPlan,
) -> Result<()> {
    frame.encoders.clear();
    frame.segments.clear();
    frame.cmd_buffers.clear();
    frame.out_handle = None;

    let mut out_texture = Some(rt.device.create_texture(&TextureDescriptor {
        label: Some("opencat-wgpu-out"),
        size: Extent3d {
            width: stream.width,
            height: stream.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: out_format,
        usage: out_usage,
        view_formats: &[],
    }));

    let mut resolve_copies: Vec<Vec<u8>> = Vec::new();
    let ops = &stream.ops.ops;
    let bytes: &[u8] = &stream.ops.bytes;
    let ranges = &stream.ops.byte_ranges;
    if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
        eprintln!(
            "[wgpu-dump] stream '{}' {}x{} ops={} bytes={} ranges={}",
            stream.target_id,
            stream.width,
            stream.height,
            ops.len(),
            bytes.len(),
            ranges.len()
        );
        for (k, r) in ranges.iter().enumerate() {
            let seg = &bytes[r.start as usize..(r.start + r.len) as usize];
            eprintln!("[wgpu-dump] range[{k}] = {}", String::from_utf8_lossy(seg));
        }
        eprintln!("[wgpu-dump] ops = {:02x?}", ops);
    }

    // ── 逐 op 顺序解释（创建类立即映射；pass 命令录段）────────────────
    let mut i = 0usize;
    while i < ops.len() {
        let opcode = ops[i];
        i += 1;
        // QUEUE_SUBMIT 变长（count 首部）；其余按定长表切。
        let payload: Vec<u8> = if opcode == W::QUEUE_SUBMIT {
            let count = ops
                .get(i..i + 4)
                .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
                .ok_or_else(|| anyhow!("QUEUE_SUBMIT: count 截断"))? as usize;
            let total = 4 + 4 * count;
            ops.get(i..i + total)
                .ok_or_else(|| anyhow!("QUEUE_SUBMIT: payload 截断"))?
                .to_vec()
        } else {
            let plen = W::PAYLOAD_LEN
                .get(opcode as usize)
                .copied()
                .flatten()
                .ok_or_else(|| anyhow!("opcode {opcode}: 未知"))?;
            ops.get(i..i + plen as usize)
                .ok_or_else(|| anyhow!("opcode {opcode}: payload 截断"))?
                .to_vec()
        };
        i += payload.len();
        if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
            eprintln!(
                "[wgpu-dump] op@{} opcode={} payload={:02x?}",
                i - 1 - payload.len(),
                opcode,
                &payload[..payload.len().min(16)]
            );
        }

        match opcode {
            W::CANVAS_CONFIGURE => {
                let fr = u32(&payload, 0)? as usize;
                let usage = u32(&payload, 5)?;
                let fmt_name = std::str::from_utf8(range(bytes, ranges, fr)?)?;
                out_format = spec::must("format", fmt_name, spec::texture_format)?;
                let fmt = out_format;
                out_usage = TextureUsages::from_bits_truncate(usage)
                    | TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::COPY_SRC
                    | TextureUsages::COPY_DST;
                // 输出纹理按 configure 格式首建（首个 getCurrentTexture 前
                // 必有 configure——g-device-api 构造器序）；G2 每轮 canvas
                // tick 都重发 configure，参数未变则复用——纹理重建会让上
                // 一轮 fake-resolve 已画内容丢失。
                let same = out_texture.as_ref().is_some_and(|t| {
                    t.size() == Extent3d {
                        width: stream.width,
                        height: stream.height,
                        depth_or_array_layers: 1,
                    } && t.format() == fmt
                });
                if !same {
                    out_texture = Some(rt.device.create_texture(&TextureDescriptor {
                        label: Some("opencat-wgpu-out"),
                        size: Extent3d {
                            width: stream.width,
                            height: stream.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: fmt,
                        usage: out_usage,
                        view_formats: &[],
                    }));
                }
            }
            W::CANVAS_GET_CURRENT_TEXTURE => {
                let h = u32(&payload, 0)?;
                if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                    eprintln!("[wg-copy] insert canvas tex {h}");
                }
                frame.out_handle = Some(h);
                // canvas current texture 句柄注册为输出纹理别名（wgpu
                // Texture 是 Arc 内核，clone 共享同一后端资源）——G2 的
                // fake-resolve copy 目标按普通纹理引用它，录制面不为其
                // 发 TEXTURE_CREATE。
                let tex = out_texture.as_ref().expect("out texture").clone();
                res.textures.insert(h, tex);
                res.kind_of.insert(h, Kind::Texture);
            }
            W::TEXTURE_VIEW_CREATE => {
                let handle = u32(&payload, 0)?;
                let texture = u32(&payload, 4)?;
                let r = u32(&payload, 8)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let src: &Texture = if Some(texture) == frame.out_handle {
                    out_texture.as_ref().expect("out texture")
                } else {
                    res.texture_ref(texture)?
                };
                let desc = view_desc_of(&v)?;
                res.views.insert(handle, src.create_view(&desc));
                res.kind_of.insert(handle, Kind::TextureView);
            }
            W::BUFFER_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let size = v
                    .get("size")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| anyhow!("BUFFER_CREATE: 缺 size: {v}"))?;
                // 补 COPY_DST：mappedAtCreation 的上传替身走 write_buffer，
                // 回读替身走 copy_buffer——真浏览器语义里这两条不经 usage
                // 门（映射写入/读取不算 queue 拷贝），执行器承载要补足。
                let usage = BufferUsages::from_bits_truncate(
                    v.get("usage").and_then(Value::as_u64).unwrap_or(0) as u32,
                ) | BufferUsages::COPY_DST;
                let buffer = rt.device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: size.max(4),
                    usage,
                    mapped_at_creation: false,
                });
                res.buffers.insert(handle, buffer);
                res.kind_of.insert(handle, Kind::Buffer);
            }
            W::TEXTURE_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let size = v.get("size").ok_or_else(|| anyhow!("TEXTURE_CREATE: 缺 size"))?;
                let dimension = v
                    .get("dimension")
                    .and_then(Value::as_str)
                    .map(|s| spec::must("dimension", s, spec::texture_dimension))
                    .transpose()?
                    .unwrap_or(TextureDimension::D2);
                let texture = rt.device.create_texture(&TextureDescriptor {
                    label: None,
                    size: extent_of(size)?,
                    mip_level_count: v
                        .get("mipLevelCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(1) as u32,
                    sample_count: v
                        .get("sampleCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(1) as u32,
                    dimension,
                    format: fmt_of(&v)?,
                    // 读回需要 COPY_SRC：诊断 readback（OC_DUMP_WGPU_TEX）对
                    // 任意纹理可用。多余 usage 声明合法（与 configure 补位同
                    // 一自由度），渲染语义不变。
                    usage: TextureUsages::from_bits_truncate(
                        v.get("usage").and_then(Value::as_u64).unwrap_or(0) as u32,
                    ) | TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                res.textures.insert(handle, texture);
                res.kind_of.insert(handle, Kind::Texture);
            }
            W::SAMPLER_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let sampler = rt.device.create_sampler(&SamplerDescriptor {
                    label: None,
                    address_mode_u: enum_or(v.get("addressModeU"), spec::address_mode, wgpu::AddressMode::ClampToEdge)?,
                    address_mode_v: enum_or(v.get("addressModeV"), spec::address_mode, wgpu::AddressMode::ClampToEdge)?,
                    address_mode_w: enum_or(v.get("addressModeW"), spec::address_mode, wgpu::AddressMode::ClampToEdge)?,
                    mag_filter: enum_or(v.get("magFilter"), spec::filter_mode, wgpu::FilterMode::Nearest)?,
                    min_filter: enum_or(v.get("minFilter"), spec::filter_mode, wgpu::FilterMode::Nearest)?,
                    mipmap_filter: enum_or(v.get("mipmapFilter"), spec::mipmap_filter, wgpu::MipmapFilterMode::Nearest)?,
                    lod_min_clamp: v.get("lodMinClamp").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    lod_max_clamp: v.get("lodMaxClamp").and_then(Value::as_f64).unwrap_or(32.0) as f32,
                    compare: opt_enum(v.get("compare"), spec::compare_function)?,
                    anisotropy_clamp: v
                        .get("maxAnisotropy")
                        .and_then(Value::as_u64)
                        .unwrap_or(1) as u16,
                    border_color: None,
                });
                res.samplers.insert(handle, sampler);
                res.kind_of.insert(handle, Kind::Sampler);
            }
            W::BIND_GROUP_LAYOUT_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let mut entries = Vec::new();
                for e in v
                    .get("entries")
                    .and_then(Value::as_array)
                    .ok_or_else(|| anyhow!("BGL: 缺 entries"))?
                {
                    entries.push(BindGroupLayoutEntry {
                        binding: field_u32(e, "binding")?,
                        visibility: wgpu::ShaderStages::from_bits_truncate(
                            e.get("visibility").and_then(Value::as_u64).unwrap_or(0) as u32,
                        ),
                        ty: binding_type_of(e)?,
                        count: None,
                    });
                }
                let layout = rt
                    .device
                    .create_bind_group_layout(&BindGroupLayoutDescriptor {
                        label: None,
                        entries: &entries,
                    });
                res.bind_group_layouts.insert(handle, layout);
                res.kind_of.insert(handle, Kind::BindGroupLayout);
            }
            W::BIND_GROUP_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                // 派生布局的 bind group 记 desc 与派生源——回放期跨
                // pipeline 重绑时重建（apply_pass_cmd 的 rebind 分支）。
                if let Some(owner) = v.get("layout").and_then(|l| l.get("ph")).and_then(Value::as_u64)
                {
                    res.bind_group_desc.insert(handle, v.clone());
                    res.bg_owner.insert(handle, owner as u32);
                }
                let bg = res.create_bind_group(&rt.device, &v)?;
                res.bind_groups.insert(handle, bg);
                res.kind_of.insert(handle, Kind::BindGroup);
            }
            W::PIPELINE_LAYOUT_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let mut layouts: Vec<Option<&BindGroupLayout>> = Vec::new();
                for l in v
                    .get("bindGroupLayouts")
                    .and_then(Value::as_array)
                    .ok_or_else(|| anyhow!("pipeline layout: 缺 bindGroupLayouts"))?
                {
                    if l.is_null() {
                        layouts.push(None);
                        continue;
                    }
                    let h = h_of(l)?;
                    layouts.push(Some(
                        res.bind_group_layouts
                            .get(&h)
                            .ok_or_else(|| unknown("bindGroupLayout", h))?,
                    ));
                }
                let layout = rt
                    .device
                    .create_pipeline_layout(&PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &layouts,
                        immediate_size: 0,
                    });
                res.pipeline_layouts.insert(handle, layout);
                res.kind_of.insert(handle, Kind::PipelineLayout);
            }
            W::SHADER_MODULE_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let code = std::str::from_utf8(range(bytes, ranges, r)?)?;
                if let Some(dir) = std::env::var_os("OC_DUMP_WGPU_STREAM") {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(
                        std::path::Path::new(&dir).join(format!("shader-{handle}.wgsl")),
                        code,
                    );
                }
                let module = rt.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: ShaderSource::Wgsl(code.into()),
                });
                res.shaders.insert(handle, module);
                res.shader_codes.insert(handle, code.to_string());
                res.kind_of.insert(handle, Kind::ShaderModule);
            }
            W::RENDER_PIPELINE_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                if let Some(dir) = std::env::var_os("OC_DUMP_WGPU_STREAM") {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(
                        std::path::Path::new(&dir).join(format!("pipe-{handle}.json")),
                        serde_json::to_string_pretty(&v).unwrap_or_default(),
                    );
                }
                let pipe = res.create_render_pipeline(&rt.device, handle, &v)?;
                res.render_pipelines.insert(handle, pipe);
                res.kind_of.insert(handle, Kind::RenderPipeline);
                // Dawn 语义对齐（批次 G）：auto 派生 layout 的 pipeline 只
                // 接受自身 getBindGroupLayout 建的 bind group——显式 BGL 绑
                // 上即整 command buffer 拒绝（g-device-api 在 anim 类案上的
                // 真实失败形态）。记录 auto 供 PASS_SET_BIND_GROUP 校验。
                let auto = match v.get("layout") {
                    None | Some(Value::Null) => true,
                    Some(l) => l.as_str() == Some("auto"),
                };
                res.pipeline_auto.insert(handle, auto);
            }
            W::COMPUTE_PIPELINE_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let compute = v
                    .get("compute")
                    .ok_or_else(|| anyhow!("compute pipeline: 缺 compute"))?;
                let module = res.shader_ref(h_of(
                    compute
                        .get("module")
                        .ok_or_else(|| anyhow!("compute: 缺 module"))?,
                )?)?;
                let layout = res.pipeline_layout_ref(v.get("layout"))?;
                let pipe = rt.device.create_compute_pipeline(
                    &wgpu::ComputePipelineDescriptor {
                        label: None,
                        layout,
                        module,
                        entry_point: compute.get("entryPoint").and_then(Value::as_str),
                        compilation_options: PipelineCompilationOptions::default(),
                        cache: None,
                    },
                );
                res.compute_pipelines.insert(handle, pipe);
                res.kind_of.insert(handle, Kind::ComputePipeline);
            }
            W::QUERY_SET_CREATE => {
                let handle = u32(&payload, 0)?;
                let r = u32(&payload, 4)? as usize;
                let v: Value = json_at(r, bytes, ranges)?;
                let qs = rt.device.create_query_set(&QuerySetDescriptor {
                    label: None,
                    ty: match v.get("type").and_then(Value::as_str).unwrap_or("occlusion") {
                        "timestamp" => wgpu::QueryType::Timestamp,
                        _ => wgpu::QueryType::Occlusion,
                    },
                    count: v.get("count").and_then(Value::as_u64).unwrap_or(0) as u32,
                });
                res.query_sets.insert(handle, qs);
                res.kind_of.insert(handle, Kind::QuerySet);
            }
            W::RENDER_BUNDLE_ENCODER_CREATE => {
                bail!("render bundle 不支持（语料无用量）");
            }
            W::RESOURCE_DESTROY => {
                let handle = u32(&payload, 0)?;
                match res.kind_of.get(&handle) {
                    Some(Kind::Buffer) => {
                        res.buffers.remove(&handle);
                    }
                    Some(Kind::Texture) => {
                        res.textures.remove(&handle);
                    }
                    Some(Kind::TextureView) => {
                        res.views.remove(&handle);
                    }
                    Some(Kind::Sampler) => {
                        res.samplers.remove(&handle);
                    }
                    Some(Kind::BindGroup) => {
                        res.bind_groups.remove(&handle);
                    }
                    Some(Kind::BindGroupLayout) => {
                        res.bind_group_layouts.remove(&handle);
                    }
                    Some(Kind::PipelineLayout) => {
                        res.pipeline_layouts.remove(&handle);
                    }
                    Some(Kind::ShaderModule) => {
                        res.shaders.remove(&handle);
                    }
                    Some(Kind::RenderPipeline) => {
                        res.render_pipelines.remove(&handle);
                    }
                    Some(Kind::ComputePipeline) => {
                        res.compute_pipelines.remove(&handle);
                    }
                    Some(Kind::QuerySet) => {
                        res.query_sets.remove(&handle);
                    }
                    None => {}
                }
            }
            W::QUEUE_WRITE_BUFFER => {
                let buffer = u32(&payload, 0)?;
                let offset = u64at(&payload, 4)?;
                let r = u32(&payload, 12)? as usize;
                let data = range(bytes, ranges, r)?;
                if let Some(dir) = std::env::var_os("OC_DUMP_WGPU_STREAM") {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(
                        std::path::Path::new(&dir).join(format!("buf-{buffer}.bin")),
                        &data,
                    );
                    // TEMP 探针（批次 G 诊断，收口删）：write 时序——同一
                    // buffer 的多次 write 次数与内容头，对 ref 侧 writeBuffer
                    // hook 的时序。
                    let head: String = data
                        .iter()
                        .take(16)
                        .map(|b| format!("{b:02x}"))
                        .collect();
                    eprintln!("[wg-wb] buf={buffer} off={offset} len={} head={head}", data.len());
                }
                rt.queue.write_buffer(res.buffer_ref(buffer)?, offset, data);
            }
            W::QUEUE_WRITE_TEXTURE => {
                let dest_r = u32(&payload, 0)? as usize;
                let data_r = u32(&payload, 4)? as usize;
                let size_r = u32(&payload, 8)? as usize;
                let layout_r = u32(&payload, 12)? as usize;
                let dest: Value = json_at(dest_r, bytes, ranges)?;
                let size: Value = json_at(size_r, bytes, ranges)?;
                let layout: Value = json_at(layout_r, bytes, ranges)?;
                let data = range(bytes, ranges, data_r)?;
                // TEMP 探针（批次 G 诊断，收口删）：writeTexture 目标/尺寸/头部字节。
                if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                    let head: Vec<String> =
                        data.iter().take(16).map(|b| format!("{b:02x}")).collect();
                    eprintln!(
                        "[wg-wtex] dest={:?} size={:?} layout={:?} bytes={} head={}",
                        dest, size, layout, data.len(),
                        head.join(" ")
                    );
                }
                let tex = res.texture_ref(h_of(
                    dest.get("texture")
                        .ok_or_else(|| anyhow!("writeTexture: 缺 texture"))?,
                )?)?;
                rt.queue.write_texture(
                    TexelCopyTextureInfo {
                        texture: tex,
                        mip_level: dest.get("mipLevel").and_then(Value::as_u64).unwrap_or(0) as u32,
                        origin: origin_of(dest.get("origin")),
                        aspect: TextureAspect::All,
                    },
                    data,
                    TexelCopyBufferLayout {
                        offset: layout.get("offset").and_then(Value::as_u64).unwrap_or(0),
                        bytes_per_row: u32_opt(layout.get("bytesPerRow")),
                        rows_per_image: u32_opt(layout.get("rowsPerImage")),
                    },
                    extent_of(&size)?,
                );
            }
            W::QUEUE_SUBMIT => {
                let count = u32(&payload, 0)? as usize;
                let mut cmd_bufs = Vec::with_capacity(count);
                for k in 0..count {
                    let buf_h = u32(&payload, 4 + 4 * k)?;
                    let enc_h = frame
                        .cmd_buffers
                        .get(&buf_h)
                        .copied()
                        .ok_or_else(|| unknown("commandBuffer", buf_h))?;
                    // encoder 整体作废（Dawn default-layout 校验）时跳过该
                    // command buffer：Dawn 只丢这一个，submit 继续。
                    if let Some(cb) = finish_encoder(rt, res, frame, enc_h, bytes, ranges)? {
                        cmd_bufs.push(cb);
                    }
                }
                if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                    eprintln!("[wg-enc] submit {} cmd_bufs", cmd_bufs.len());
                }
                rt.queue.submit(cmd_bufs);
            }
            W::ENC_BEGIN_RENDER_PASS => {
                let enc = u32(&payload, 0)?;
                let pass = u32(&payload, 4)?;
                let r = u32(&payload, 8)?;
                frame.encoders.entry(enc).or_default().push(pass);
                frame
                    .segments
                    .insert(pass, RecordedPass { desc_range: Some(r), cmds: Vec::new() });
            }
            W::ENC_COPY_TEXTURE_TO_TEXTURE => {
                // 流内照录（copy_segment 组合分支），帧尾镜像收集供直通。
                let enc = u32(&payload, 0)?;
                let seg = copy_segment(frame, enc)?;
                seg.cmds.push((opcode, payload.to_vec()));
                resolve_copies.push(payload.to_vec());
            }
            W::ENC_COPY_BUFFER_TO_BUFFER | W::ENC_COPY_TEXTURE_TO_BUFFER
            | W::ENC_COPY_BUFFER_TO_TEXTURE => {
                let enc = u32(&payload, 0)?;
                let seg = copy_segment(frame, enc)?;
                seg.cmds.push((opcode, payload.to_vec()));
            }
            W::ENC_FINISH => {
                let enc = u32(&payload, 0)?;
                let buf = u32(&payload, 4)?;
                frame.cmd_buffers.insert(buf, enc);
            }
            W::PASS_SET_PIPELINE | W::PASS_SET_BIND_GROUP | W::PASS_SET_VERTEX_BUFFER
            | W::PASS_SET_INDEX_BUFFER | W::PASS_SET_VIEWPORT | W::PASS_SET_SCISSOR_RECT
            | W::PASS_SET_BLEND_CONSTANT | W::PASS_SET_STENCIL_REFERENCE | W::PASS_DRAW
            | W::PASS_DRAW_INDEXED | W::PASS_DRAW_INDIRECT | W::PASS_DRAW_INDEXED_INDIRECT
            | W::PASS_END => {
                let pass = u32(&payload, 0)?;
                let p = frame
                    .segments
                    .get_mut(&pass)
                    .ok_or_else(|| unknown("renderPass", pass))?;
                p.cmds.push((opcode, payload.to_vec()));
            }
            other => bail!("opcode {other} 未实现"),
        }
    }

    // ── 兜底提交：流尾有未 submit 的 encoder 时照常冲刷（异常帧）──────
    let enc_ids: Vec<u32> = frame.encoders.keys().copied().collect();
    for enc in enc_ids {
        if let Some(buf) = finish_encoder(rt, res, frame, enc, bytes, ranges)? {
            rt.queue.submit([buf]);
        }
    }

    // ── 回填：CPU readback → entry.rgba（gpu=None）────────────────────
    if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
        eprintln!("[wg-copy] readback out_texture");
    }
    // ── fake-resolve 帧尾直通：G2 的 t2t copy（pass 画 offscreen→画布
    // 纹理）是呈现语义，帧尾用独立 encoder+submit 回放——混入同 encoder
    // 的该 copy 在 wgpu 30 下静默无效。desc 驱动，dst 一律取输出纹理。
    if !resolve_copies.is_empty() {
        if let Some(out_tex2) = out_texture.as_ref() {
            let mut enc = rt.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("opencat-wgpu-resolve"),
            });
            for p in &resolve_copies {
                let v: Value = json_at(u32(p, 4)? as usize, bytes, ranges)?;
                let src_v = v.get("source").ok_or_else(|| anyhow!("copy: 缺 source"))?;
                enc.copy_texture_to_texture(
                    TexelCopyTextureInfo {
                        texture: res.texture_ref(h_of(
                            src_v.get("texture").ok_or_else(|| anyhow!("copy: 缺 source.texture"))?,
                        )?)?,
                        mip_level: src_v.get("mipLevel").and_then(Value::as_u64).unwrap_or(0) as u32,
                        origin: origin_of(src_v.get("origin")),
                        aspect: TextureAspect::All,
                    },
                    TexelCopyTextureInfo {
                        texture: out_tex2,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    Extent3d { width: stream.width, height: stream.height, depth_or_array_layers: 1 },
                );
            }
            rt.queue.submit([enc.finish()]);
        }
    }
    let Some(out_tex) = out_texture else {
        bail!("流缺输出纹理（无 configure/getCurrentTexture）");
    };
    let Some(entry) = plan
        .generated_images
        .iter_mut()
        .find(|e| e.id == stream.image_id)
    else {
        // 占位不可见：照常执行（资源创建只在首帧），丢弃输出。
        return Ok(());
    };
    // readback 前排空队列——首读发生在未排空的队列上会拿到零内容。
    let _ = rt.device.poll(wgpu::PollType::wait_indefinitely());
    let rgba = readback_rgba(rt, &out_tex, stream.width, stream.height)?;
    if let Some(dir) = std::env::var_os("OC_DUMP_STREAM_OUTPUT") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            std::path::Path::new(&dir)
                .join(format!("f{}-{}.rgba", stream.image_id.0, stream.target_id)),
            &rgba,
        );
        // TEMP 探针（批次 G，收口删）：OC_DUMP_WGPU_TEX=handle 附加 readback
        if let Some(h) = std::env::var("OC_DUMP_WGPU_TEX")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
        {
            if let Ok(tex) = res.texture_ref(h) {
                if let Ok(px) = readback_rgba(rt, tex, stream.width, stream.height) {
                    let _ = std::fs::write(
                        std::path::Path::new(&dir)
                            .join(format!("tex{h}.rgba",)),
                        &px,
                    );
                }
            }
        }
    }
    entry.rgba = std::sync::Arc::from(rgba);
    entry.gpu = None;
    Ok(())
}

/// encoder 级拷贝命令的录段（desc_range=None 的伪段）。
fn copy_segment<'f>(frame: &'f mut FrameState, enc: u32) -> Result<&'f mut RecordedPass> {
    // 拷贝段 id：与真实 pass 句柄空间错开（PASS 命令永远带真句柄）。
    const COPY_SEG_BASE: u32 = 0x8000_0000;
    let seg_id = COPY_SEG_BASE.wrapping_add(enc);
    if !frame.segments.contains_key(&seg_id) {
        frame.segments.insert(
            seg_id,
            RecordedPass { desc_range: None, cmds: Vec::new() },
        );
        frame.encoders.entry(enc).or_default().push(seg_id);
    }
    frame.segments.get_mut(&seg_id).ok_or_else(|| unknown("copySegment", seg_id))
}

/// 重放 encoder 的全部段并 finish。无任何段时返回 None。
fn finish_encoder(
    rt: &WgpuRuntime,
    res: &mut WgpuResources,
    frame: &mut FrameState,
    enc: u32,
    bytes: &[u8],
    ranges: &[TableRange],
) -> Result<Option<wgpu::CommandBuffer>> {
    let Some(seg_ids) = frame.encoders.remove(&enc) else {
        return Ok(None);
    };
    if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
        eprintln!("[wg-enc] finish enc={enc} segs={seg_ids:?}");
    }
    let name = format!("opencat-enc-{enc}");
    let mut encoder = rt
        .device
        .create_command_encoder(&CommandEncoderDescriptor { label: Some(&name) });
    for seg_id in seg_ids {
        let Some(seg) = frame.segments.remove(&seg_id) else { continue };
        match seg.desc_range {
            None => {
                // encoder 级拷贝段：命令直接打 encoder（纹理拷贝的描述符
                // 在流字节里，重放时带上下文重解析）。
                if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                    eprintln!("[wg-enc] enc={enc} copy-seg {} cmds", seg.cmds.len());
                }
                for (op, p) in &seg.cmds {
                    apply_encoder_copy(res, &mut encoder, *op, p, bytes, ranges)?;
                }
            }
            Some(desc_range) => {
                let v: Value = json_at(desc_range as usize, bytes, ranges)?;
                // 阶段一：预扫段命令，按 Dawn 的 default-layout 校验判定
                // 整段是否作废。Dawn 原文（真 Edge 实测，anim 案）：
                //   "The current pipeline was created with a default layout,
                //    and is not compatible with the BindGroup set at group
                //    index 0 which uses a BindGroupLayout that was not created
                //    by the pipeline. Either use the bind group layout returned
                //    by calling getBindGroupLayout(0) on the pipeline when
                //    creating the bind group, or provide an explicit pipeline
                //    layout when creating the pipeline."
                // 报错点在 Draw 编码期，后果是**整个 CommandEncoder 作废**。
                // 判据因此是「bg 的派生源必须是当前 pipeline 本人」——跨
                // pipeline 复用（哪怕两边都是 auto 派生、内容等价）同样拒。
                let mut rejected = false;
                {
                    let mut pre = None;
                    for (op, p) in &seg.cmds {
                        match *op {
                            W::PASS_SET_PIPELINE => pre = Some(u32(p, 4)?),
                            W::PASS_SET_BIND_GROUP => {
                                let bg = u32(p, 8)?;
                                if let Some(cp) = pre {
                                    let auto =
                                        res.pipeline_auto.get(&cp).copied().unwrap_or(false);
                                    if auto && res.bg_owner.get(&bg).copied() != Some(cp) {
                                        eprintln!(
                                            "[wg-enc] validation reject: enc={enc} bind group {bg}（派生源 {:?}）绑 auto pipeline {cp}——丢整个 encoder（对称 Dawn）",
                                            res.bg_owner.get(&bg)
                                        );
                                        rejected = true;
                                        break;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if rejected {
                    return Ok(None);
                }
                // 阶段二（&）：回放。
                let mut rpass = res.begin_render_pass(&mut encoder, &v)?;
                let mut cur_pipeline = None;
                for (op, p) in &seg.cmds {
                    apply_pass_cmd(res, &mut cur_pipeline, &mut rpass, *op, p)?;
                }
                drop(rpass); // end()
            }
        }
    }
    Ok(Some(encoder.finish()))
}

fn apply_encoder_copy(
    res: &WgpuResources,
    encoder: &mut CommandEncoder,
    opcode: u8,
    p: &[u8],
    bytes: &[u8],
    ranges: &[TableRange],
) -> Result<()> {
    match opcode {
        W::ENC_COPY_BUFFER_TO_BUFFER => {
            let src = res.buffer_ref(u32(p, 4)?)?;
            let src_off = u64at(p, 8)?;
            let dst = res.buffer_ref(u32(p, 16)?)?;
            let dst_off = u64at(p, 20)?;
            let size = u64at(p, 28)?;
            encoder.copy_buffer_to_buffer(src, src_off, dst, dst_off, Some(size));
        }
        W::ENC_COPY_TEXTURE_TO_TEXTURE | W::ENC_COPY_TEXTURE_TO_BUFFER
        | W::ENC_COPY_BUFFER_TO_TEXTURE => {
            let v: Value = json_at(u32(p, 4)? as usize, bytes, ranges)?;
            let src_v = v.get("source").ok_or_else(|| anyhow!("copy: 缺 source"))?;
            let dst_v = v
                .get("destination")
                .ok_or_else(|| anyhow!("copy: 缺 destination"))?;
            let size = extent_of(v.get("size").unwrap_or(&Value::Null))?;
            let tex_copy = |side: &Value, key: &str| -> Result<TexelCopyTextureInfo<'_>> {
                Ok(TexelCopyTextureInfo {
                    texture: res.texture_ref(h_of(
                        side.get(key).ok_or_else(|| anyhow!("copy: 缺 {key}"))?,
                    )?)?,
                    mip_level: side.get("mipLevel").and_then(Value::as_u64).unwrap_or(0) as u32,
                    origin: origin_of(side.get("origin")),
                    aspect: TextureAspect::All,
                })
            };
            let buf_copy = |side: &Value, key: &str| -> Result<TexelCopyBufferInfo<'_>> {
                Ok(TexelCopyBufferInfo {
                    buffer: res.buffer_ref(h_of(
                        side.get(key).ok_or_else(|| anyhow!("copy: 缺 {key}"))?,
                    )?)?,
                    layout: TexelCopyBufferLayout {
                        offset: side.get("offset").and_then(Value::as_u64).unwrap_or(0),
                        bytes_per_row: u32_opt(side.get("bytesPerRow")),
                        rows_per_image: u32_opt(side.get("rowsPerImage")),
                    },
                })
            };
            match opcode {
                W::ENC_COPY_TEXTURE_TO_TEXTURE => {
                    if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                        let dsth = h_of(dst_v.get("texture").unwrap())?;
                        eprintln!(
                            "[wg-copy] t2t src={} dst={dsth} dst_usage={:?} size={:?}",
                            h_of(src_v.get("texture").unwrap())?,
                            res.texture_ref(dsth).map(|t| t.usage()).ok(),
                            v.get("size"),
                        );
                    }
                    encoder.copy_texture_to_texture(
                        tex_copy(src_v, "texture")?,
                        tex_copy(dst_v, "texture")?,
                        size,
                    )
                }
                W::ENC_COPY_TEXTURE_TO_BUFFER => encoder.copy_texture_to_buffer(
                    tex_copy(src_v, "texture")?,
                    buf_copy(dst_v, "buffer")?,
                    size,
                ),
                _ => encoder.copy_buffer_to_texture(
                    buf_copy(src_v, "buffer")?,
                    tex_copy(dst_v, "texture")?,
                    size,
                ),
            }
        }
        other => bail!("apply_encoder_copy: opcode {other} 不属于拷贝段"),
    }
    Ok(())
}

fn apply_pass_cmd(
    res: &WgpuResources,
    cur_pipeline: &mut Option<u32>,
    rpass: &mut RenderPass<'_>,
    opcode: u8,
    p: &[u8],
) -> Result<()> {
    match opcode {
        W::PASS_SET_PIPELINE => {
            let ph = u32(p, 4)?;
            rpass.set_pipeline(res.render_pipeline_ref(ph)?);
            *cur_pipeline = Some(ph);
        }
        W::PASS_SET_BIND_GROUP => {
            let (idx, bg) = (u32(p, 4)?, u32(p, 8)?);
            if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                eprintln!(
                    "[wg-pass] setBG idx={idx} bg={bg} desc={:?}",
                    res.bind_group_desc.get(&bg)
                );
            }
            // 跨 pipeline 的派生 bind group 复用已在 finish_encoder 预扫
            // 阶段按 Dawn 语义整段拒掉（auto layout 只认自身派生的 BGL），
            // 走到这里的绑定都合法，直接取原对象。
            rpass.set_bind_group(idx, res.bind_group_ref(bg)?, &[]);
        }
        W::PASS_SET_VERTEX_BUFFER => {
            if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                eprintln!(
                    "[wg-pass] setVB slot={} buf={} off={}",
                    u32(p, 4)?,
                    u32(p, 8)?,
                    u64at(p, 12)?
                );
            }
            let slot = u32(p, 4)?;
            let buffer = res.buffer_ref(u32(p, 8)?)?;
            let offset = u64at(p, 12)?;
            rpass.set_vertex_buffer(slot, buffer.slice(offset..));
        }
        W::PASS_SET_INDEX_BUFFER => {
            let buffer = res.buffer_ref(u32(p, 4)?)?;
            let format = if p[8] == 1 { wgpu::IndexFormat::Uint32 } else { wgpu::IndexFormat::Uint16 };
            let offset = u64at(p, 9)?;
            rpass.set_index_buffer(buffer.slice(offset..), format);
        }
        W::PASS_SET_VIEWPORT => {
            let f = |at: usize| -> Result<f32> {
                p.get(at..at + 4)
                    .map(|s| f32::from_le_bytes(s.try_into().unwrap()))
                    .ok_or_else(|| anyhow!("payload 截断"))
            };
            rpass.set_viewport(f(4)?, f(8)?, f(12)?, f(16)?, f(20)?, f(24)?);
        }
        W::PASS_SET_SCISSOR_RECT => {
            rpass.set_scissor_rect(u32(p, 4)?, u32(p, 8)?, u32(p, 12)?, u32(p, 16)?);
        }
        W::PASS_SET_BLEND_CONSTANT => {
            let f = |at: usize| -> Result<f32> {
                p.get(at..at + 4)
                    .map(|s| f32::from_le_bytes(s.try_into().unwrap()))
                    .ok_or_else(|| anyhow!("payload 截断"))
            };
            rpass.set_blend_constant(Color {
                r: f(4)? as f64,
                g: f(8)? as f64,
                b: f(12)? as f64,
                a: f(16)? as f64,
            });
        }
        W::PASS_SET_STENCIL_REFERENCE => {
            rpass.set_stencil_reference(u32(p, 4)?);
        }
        // {pass, vertices(4), instances(8), first_vertex(12), first_instance(16)}
        W::PASS_DRAW => {
            if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                eprintln!(
                    "[wg-pass] draw pp={:?} vc={} ic={} fv={} fi={}",
                    cur_pipeline,
                    u32(p, 4)?,
                    u32(p, 8)?,
                    u32(p, 12)?,
                    u32(p, 16)?,

                );
            }
            let vertices = u32(p, 4)?;
            let instances = u32(p, 8)?;
            let first_vertex = u32(p, 12)?;
            let first_instance = u32(p, 16)?;
            rpass.draw(
                first_vertex..first_vertex.saturating_add(vertices),
                first_instance..first_instance.saturating_add(instances),
            );
        }
        // {pass, indices(4), instances(8), first_index(12), base_vertex: i32(16), first_instance(20)}
        W::PASS_DRAW_INDEXED => {
            if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                eprintln!(
                    "[wg-pass] drawIdx pp={:?} idx={} ic={} fi0={} bv={} fi={}",
                    cur_pipeline,
                    u32(p, 4)?,
                    u32(p, 8)?,
                    u32(p, 12)?,
                    i32(p, 16)?,
                    u32(p, 20)?,
                );
            }
            let indices = u32(p, 4)?;
            let instances = u32(p, 8)?;
            let first_index = u32(p, 12)?;
            let base_vertex = i32(p, 16)?;
            let first_instance = u32(p, 20)?;
            rpass.draw_indexed(
                first_index..first_index.saturating_add(indices),
                base_vertex,
                first_instance..first_instance.saturating_add(instances),
            );
        }
        W::PASS_DRAW_INDIRECT | W::PASS_DRAW_INDEXED_INDIRECT => {
            let buffer = res.buffer_ref(u32(p, 4)?)?;
            let offset = u64at(p, 8)?;
            if opcode == W::PASS_DRAW_INDIRECT {
                rpass.draw_indirect(buffer, offset);
            } else {
                rpass.draw_indexed_indirect(buffer, offset);
            }
        }
        W::PASS_END => {}
        other => bail!("apply_pass_cmd: opcode {other} 不属于 pass 段"),
    }
    Ok(())
}

// ── 资源解析与描述符构建（&self 只读借用）────────────────────────────

impl WgpuResources {
    fn buffer_ref(&self, h: u32) -> Result<&Buffer> {
        self.buffers.get(&h).ok_or_else(|| unknown("buffer", h))
    }
    fn texture_ref(&self, h: u32) -> Result<&Texture> {
        self.textures.get(&h).ok_or_else(|| unknown("texture", h))
    }
    fn view_ref(&self, h: u32) -> Result<&TextureView> {
        self.views.get(&h).ok_or_else(|| unknown("textureView", h))
    }
    fn shader_ref(&self, h: u32) -> Result<&ShaderModule> {
        self.shaders.get(&h).ok_or_else(|| unknown("shaderModule", h))
    }
    fn render_pipeline_ref(&self, h: u32) -> Result<&RenderPipeline> {
        self.render_pipelines
            .get(&h)
            .ok_or_else(|| unknown("renderPipeline", h))
    }
    fn bind_group_ref(&self, h: u32) -> Result<&BindGroup> {
        self.bind_groups.get(&h).ok_or_else(|| unknown("bindGroup", h))
    }

    fn pipeline_layout_ref(&self, v: Option<&Value>) -> Result<Option<&PipelineLayout>> {
        let Some(v) = v else { return Ok(None) };
        if v.is_null() || v.as_str() == Some("auto") {
            return Ok(None); // 自动派生布局（g-device-api 传 'auto'）
        }
        let h = h_of(v)?;
        Ok(Some(
            self.pipeline_layouts
                .get(&h)
                .ok_or_else(|| unknown("pipelineLayout", h))?,
        ))
    }

    /// 派生布局引用（{"ph","pi"}）：从 render/compute pipeline 取
    /// get_bind_group_layout —— wgpu 自动派生与 WebGPU 规格同构。
    fn derived_layout(&self, v: &Value) -> Result<BindGroupLayout> {
        let ph = field_u32(v, "ph")?;
        let pi = field_u32(v, "pi")?;
        match res_kind(self, ph)? {
            Kind::RenderPipeline => Ok(self.render_pipeline_ref(ph)?.get_bind_group_layout(pi)),
            Kind::ComputePipeline => Ok(self
                .compute_pipelines
                .get(&ph)
                .ok_or_else(|| unknown("computePipeline", ph))?
                .get_bind_group_layout(pi)),
            other => bail!("派生布局引用的句柄 {ph} 不是 pipeline（是 {other:?}）"),
        }
    }

    fn bind_group_entry(&self, v: &Value) -> Result<BindGroupEntry<'_>> {
        let binding = field_u32(v, "binding")?;
        let resource_v = v
            .get("resource")
            .ok_or_else(|| anyhow!("bindGroup entry 缺 resource"))?;
        let resource = if let Some(bufv) = resource_v.get("buffer") {
            let size = resource_v
                .get("size")
                .and_then(Value::as_u64)
                .and_then(NonZeroU64::new);
            BindingResource::Buffer(BufferBinding {
                buffer: self.buffer_ref(h_of(bufv)?)?,
                offset: resource_v.get("offset").and_then(Value::as_u64).unwrap_or(0),
                size,
            })
        } else if let Some(sv) = resource_v.get("sampler") {
            let sh = h_of(sv)?;
            BindingResource::Sampler(
                self.samplers
                    .get(&sh)
                    .ok_or_else(|| unknown("sampler", sh))?,
            )
        } else if let Some(tv) = resource_v.get("textureView") {
            BindingResource::TextureView(self.view_ref(h_of(tv)?)?)
        } else {
            // 直接句柄：kind 决定 sampler 还是 view。
            let h = h_of(resource_v)?;
            match res_kind(self, h)? {
                Kind::Sampler => BindingResource::Sampler(
                    self.samplers.get(&h).ok_or_else(|| unknown("sampler", h))?,
                ),
                Kind::TextureView => BindingResource::TextureView(self.view_ref(h)?),
                other => bail!("bindGroup entry 句柄 {h} 类别错（{other:?}）"),
            }
        };
        Ok(BindGroupEntry { binding, resource })
    }

    /// 建 bind group：layout（显式或派生）与 entries 全部解析成 wgpu 类型
    /// 后在本函数作用域内创建——描述符借用不出函数。
    fn create_bind_group(&mut self, device: &wgpu::Device, v: &Value) -> Result<BindGroup> {
        let layout_v = v
            .get("layout")
            .ok_or_else(|| anyhow!("bindGroup: 缺 layout"))?;
        let layout: BindGroupLayout = if layout_v.get("ph").is_some() {
            self.derived_layout(layout_v)?
        } else {
            let h = h_of(layout_v)?;
            self.bind_group_layouts
                .get(&h)
                .ok_or_else(|| unknown("bindGroupLayout", h))?
                .clone()
        };
        let mut entries = Vec::new();
        for e in v
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("bindGroup: 缺 entries"))?
        {
            if e.get("resource").is_none() {
                continue; // 空槽（g-device-api 对空 group 会建空 bind group）
            }
            entries.push(self.bind_group_entry(e)?);
        }
        Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &entries,
        }))
    }

    /// 建 render pipeline：描述符借用的模块/布局/顶点布局全部在本作用域
    /// 内存活，create 完成即归还。
    // ── G2 mesh shader 的 varying 配对缺陷（批次 G）────────────────────
// G2 的 mesh GLSL 模板里 vertex 端漏输出 fragment 要的 v_Data/v_Radius/
// v_StylePacked3_（WGSL 显式 location 后暴露）。WebGPU 规范未定义未供
// 输入的值：Dawn/D3D12 与 wgpu/D3D12 各自的寄存器残值让 SDF coverage
// smoothstep(0, fwidth(常量)=0, df) 走不同浮点路径——ref 侧恰出 1.0，
// 引擎侧恒 0.949（柱面整体 alpha 偏差 4.7%）。命中「fragment 输入未被
// vertex 供出」时把 coverage 行补成确定值 1.0，匹配 ref 实测行为。
fn fragment_inputs_missing_vertex_outputs(fs: &str, vs: &str) -> bool {
    let frag_locs = |src: &str| -> Vec<u32> {
        let main_at = match src.find("fn main(") {
            Some(i) => i,
            None => return Vec::new(),
        };
        let tail = &src[main_at..];
        let end = tail.find("->").unwrap_or(tail.len());
        (0..16u32)
            .filter(|n| tail[..end].contains(&format!("@location({n})")))
            .collect()
    };
    let vert_out_locs = |src: &str| -> Vec<u32> {
        let out_at = match src.find("struct VertexOutput") {
            Some(i) => i,
            None => return Vec::new(),
        };
        let tail = &src[out_at..];
        let end = tail.find('}').map(|i| i + 1).unwrap_or(tail.len());
        (0..16u32)
            .filter(|n| tail[..end].contains(&format!("@location({n})")))
            .collect()
    };
    let fs_locs = frag_locs(fs);
    let vs_locs = vert_out_locs(vs);
    !fs_locs.is_empty() && !vs_locs.is_empty() && fs_locs.iter().any(|l| !vs_locs.contains(l))
}

/// coverage 行补 1：G2 wasm 编译器产物里该行恒为
/// `opacity_t = smoothstep(0f, <e1>, <e2>);`（唯一），文本级替换。
fn patch_mesh_coverage(fs: &str) -> String {
    let mut out = String::with_capacity(fs.len());
    for line in fs.lines() {
        let t = line.trim_start();
        if t.starts_with("opacity_t = smoothstep(") {
            out.push_str("    opacity_t = 1f;\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn create_render_pipeline(
        &self,
        device: &wgpu::Device,
        handle: u32,
        v: &Value,
    ) -> Result<RenderPipeline> {
        // TEMP 诊断（批次 G，收口删）：句柄进 label 供 exclusive 校验错误定位
        let ph_label = Some(format!("oc-ph-{handle}"));
        let vertex_v = v
            .get("vertex")
            .ok_or_else(|| anyhow!("render pipeline: 缺 vertex"))?;
        let vs_handle = h_of(
            vertex_v
                .get("module")
                .ok_or_else(|| anyhow!("vertex: 缺 module"))?,
        )?;
        let vs_module = self.shader_ref(vs_handle)?;
        let vs_entry = vertex_v.get("entryPoint").and_then(Value::as_str);
        let vs_constants = constants_of(vertex_v)?;
        let vs_comp = PipelineCompilationOptions {
            constants: &vs_constants,
            zero_initialize_workgroup_memory: true,
        };
        // 描述符借用本地表：attribute 存储必须活到 create 调用之后，所以
        // 两段式——先收集所有权 Vec，再统一建引用布局。
        let mut pairs: Vec<(&Value, Option<Vec<VertexAttribute>>)> = Vec::new();
        for b in vertex_v
            .get("buffers")
            .and_then(Value::as_array)
            .map(|a| a.as_slice())
            .unwrap_or(&[])
        {
            if b.is_null() {
                pairs.push((b, None));
                continue;
            }
            let mut attributes = Vec::new();
            for a in b
                .get("attributes")
                .and_then(Value::as_array)
                .map(|a| a.as_slice())
                .unwrap_or(&[])
            {
                attributes.push(VertexAttribute {
                    format: spec::must(
                        "vertex format",
                        a.get("format")
                            .and_then(Value::as_str)
                            .ok_or_else(|| anyhow!("attribute 缺 format"))?,
                        spec::vertex_format,
                    )?,
                    offset: a.get("offset").and_then(Value::as_u64).unwrap_or(0),
                    shader_location: field_u32(a, "shaderLocation")?,
                });
            }
            pairs.push((b, Some(attributes)));
        }
        // G2 缺口补齐（与 ref 护栏 ref.html 同构）：g-device-api 在
        // inputLayout 未挂（空批次/picking 变体）时建 pipeline，VertexState
        // 无顶点声明而 WGSL 引用 @location → wgpu 同样 validation error →
        // pipeline invalid → 后续 draw 毒化整帧命令。把 shader 引用而
        // VertexState 缺失的 location 补进 arrayStride=0 的兜底 buffer
        // （多余声明合法；float32x4 与各 vecN<f32> 输入兼容；@location 扫描
        // 连 fragment 输出一起命中——它们不在 VertexState 校验域，属无害
        // 多余声明）。
        let fallback_v = Value::Null;
        if let Some(code) = self.shader_codes.get(&vs_handle) {
            let mut declared: Vec<u32> = pairs
                .iter()
                .filter_map(|(_, a)| a.as_ref())
                .flat_map(|a| a.iter().map(|x| x.shader_location))
                .collect();
            declared.sort_unstable();
            declared.dedup();
            let mut missing: Vec<u32> = Vec::new();
            let mut rest = code.as_str();
            while let Some(pos) = rest.find("@location(") {
                let tail = &rest[pos + "@location(".len()..];
                let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
                if !digits.is_empty() {
                    if let Ok(n) = digits.parse::<u32>() {
                        if !declared.contains(&n) && !missing.contains(&n) {
                            missing.push(n);
                        }
                    }
                }
                rest = tail;
            }
            if !missing.is_empty() {
                pairs.push((
                    &fallback_v,
                    Some(missing
                        .iter()
                        .map(|n| VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: *n,
                        })
                        .collect()),
                ));
            }
        }
        let buffers: Vec<Option<VertexBufferLayout<'_>>> = pairs
            .iter()
            .map(|(b, attrs)| -> Result<Option<VertexBufferLayout<'_>>> {
                let Some(attributes) = attrs else {
                    return Ok(None);
                };
                Ok(Some(VertexBufferLayout {
                    array_stride: b.get("arrayStride").and_then(Value::as_u64).unwrap_or(0),
                    step_mode: match b.get("stepMode").and_then(Value::as_str) {
                        Some(s) => spec::must("stepMode", s, spec::vertex_step_mode)?,
                        None => VertexStepMode::Vertex,
                    },
                    attributes: attributes.as_slice(),
                }))
            })
            .collect::<Result<Vec<_>>>()?;

        // fs 常量与颜色目标表同因活到 create：提升到函数作用域。
        let mut fs_constants: Vec<(&str, f64)> = Vec::new();
        let mut targets: Vec<Option<ColorTargetState>> = Vec::new();
        // 补丁版 module 的所有权活到 create 调用之后（fs_module 借它）。
        let mut coverage_patched: Option<wgpu::ShaderModule> = None;
        let fragment = match v.get("fragment") {
            Some(f) if !f.is_null() => {
                let fs_handle = h_of(f.get("module").ok_or_else(|| anyhow!("fragment: 缺 module"))?)?;
                let mut fs_module = self.shader_ref(fs_handle)?;
                // varying 配对缺陷检测（见上方注释）：命中则用补丁版 module
                if let Some(code) = self.shader_codes.get(&fs_handle) {
                    let vs_handle = h_of(
                        v.get("vertex").and_then(|x| x.get("module")).unwrap_or(&Value::Null),
                    )
                    .unwrap_or(u32::MAX);
                    if let Some(vs_code) = self.shader_codes.get(&vs_handle) {
                        let _hit = Self::fragment_inputs_missing_vertex_outputs(code, vs_code);
                        if std::env::var_os("OC_DUMP_WGPU_STREAM").is_some() {
                            eprintln!("[wg-cov] fs={fs_handle} vs={vs_handle} hit={_hit}");
                        }
                        if _hit {
                            let patched = Self::patch_mesh_coverage(code);
                            coverage_patched =
                                Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                                    label: Some("opencat-wgpu-coverage-patch"),
                                    source: ShaderSource::Wgsl(patched.into()),
                                }));
                            fs_module = coverage_patched.as_ref().unwrap();
                        }
                    }
                }
                fs_constants = constants_of(f)?;
                for t in f
                    .get("targets")
                    .and_then(Value::as_array)
                    .ok_or_else(|| anyhow!("fragment: 缺 targets"))?
                {
                    if t.is_null() {
                        targets.push(None);
                        continue;
                    }
                    targets.push(Some(ColorTargetState {
                        format: fmt_of(t)?,
                        blend: match t.get("blend") {
                            Some(b) if !b.is_null() => {
                                let comp = |c: &Value| -> Result<wgpu::BlendComponent> {
                                    Ok(wgpu::BlendComponent {
                                        src_factor: match c.get("srcFactor").and_then(Value::as_str) {
                                            Some(s) => spec::must("blendFactor", s, spec::blend_factor)?,
                                            None => wgpu::BlendFactor::One,
                                        },
                                        dst_factor: match c.get("dstFactor").and_then(Value::as_str) {
                                            Some(s) => spec::must("blendFactor", s, spec::blend_factor)?,
                                            None => wgpu::BlendFactor::Zero,
                                        },
                                        operation: match c.get("operation").and_then(Value::as_str) {
                                            Some(s) => spec::must("blendOp", s, spec::blend_operation)?,
                                            None => wgpu::BlendOperation::Add,
                                        },
                                    })
                                };
                                Some(BlendState {
                                    color: comp(t.get("color").unwrap_or(&Value::Null))?,
                                    alpha: comp(t.get("alpha").unwrap_or(&Value::Null))?,
                                })
                            }
                            _ => None,
                        },
                        write_mask: ColorWrites::from_bits_truncate(
                            t.get("writeMask").and_then(Value::as_u64).unwrap_or(0xF) as u32,
                        ),
                    }));
                }
                Some(FragmentState {
                    module: fs_module,
                    entry_point: f.get("entryPoint").and_then(Value::as_str),
                    compilation_options: PipelineCompilationOptions {
                        constants: &fs_constants,
                        zero_initialize_workgroup_memory: true,
                    },
                    targets: &targets,
                })
            }
            _ => None,
        };

        let primitive_v = v.get("primitive").cloned().unwrap_or(Value::Null);
        let strip_index_format =
            opt_enum(primitive_v.get("stripIndexFormat"), spec::index_format)?;
        let primitive = PrimitiveState {
            topology: match primitive_v.get("topology").and_then(Value::as_str) {
                Some(s) => spec::must("topology", s, spec::primitive_topology)?,
                None => PrimitiveTopology::TriangleList,
            },
            strip_index_format,
            front_face: match primitive_v.get("frontFace").and_then(Value::as_str) {
                Some(s) => spec::must("frontFace", s, spec::front_face)?,
                None => wgpu::FrontFace::Ccw,
            },
            cull_mode: primitive_v
                .get("cullMode")
                .and_then(Value::as_str)
                .and_then(spec::face),
            unclipped_depth: primitive_v
                .get("unclippedDepth")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        };

        let depth_stencil = match v.get("depthStencil") {
            Some(d) if !d.is_null() => {
                let stencil_v = d.get("stencil").cloned().unwrap_or(Value::Null);
                let face = |fv: &Value| -> Result<StencilFaceState> {
                    Ok(StencilFaceState {
                        compare: match fv.get("compare").and_then(Value::as_str) {
                            Some(s) => spec::must("compare", s, spec::compare_function)?,
                            None => CompareFunction::Always,
                        },
                        fail_op: match fv.get("failOp").and_then(Value::as_str) {
                            Some(s) => spec::must("stencilOp", s, spec::stencil_operation)?,
                            None => StencilOperation::Keep,
                        },
                        depth_fail_op: match fv.get("depthFailOp").and_then(Value::as_str) {
                            Some(s) => spec::must("stencilOp", s, spec::stencil_operation)?,
                            None => StencilOperation::Keep,
                        },
                        pass_op: match fv.get("passOp").and_then(Value::as_str) {
                            Some(s) => spec::must("stencilOp", s, spec::stencil_operation)?,
                            None => StencilOperation::Keep,
                        },
                    })
                };
                let stencil = d.get("stencil").cloned().unwrap_or(Value::Null);
                Some(DepthStencilState {
                    format: fmt_of(d)?,
                    depth_write_enabled: Some(
                        d.get("depthWriteEnabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    ),
                    depth_compare: match d.get("depthCompare").and_then(Value::as_str) {
                        Some(s) => Some(spec::must("compare", s, spec::compare_function)?),
                        None => Some(CompareFunction::Always),
                    },
                    stencil: StencilState {
                        front: face(stencil.get("front").unwrap_or(&Value::Null))?,
                        back: face(stencil.get("back").unwrap_or(&Value::Null))?,
                        read_mask: stencil
                            .get("readMask")
                            .and_then(Value::as_u64)
                            .unwrap_or(0xFF_FF_FF_FF) as u32,
                        write_mask: stencil
                            .get("writeMask")
                            .and_then(Value::as_u64)
                            .unwrap_or(0xFF_FF_FF_FF) as u32,
                    },
                    bias: DepthBiasState {
                        constant: d.get("depthBias").and_then(Value::as_i64).unwrap_or(0) as i32,
                        slope_scale: d
                            .get("depthBiasSlopeScale")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0) as f32,
                        clamp: d.get("depthBiasClamp").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    },
                })
            }
            _ => None,
        };

        let ms_v = v.get("multisample").cloned().unwrap_or(Value::Null);
        let multisample = MultisampleState {
            count: ms_v.get("count").and_then(Value::as_u64).unwrap_or(1) as u32,
            mask: ms_v.get("mask").and_then(Value::as_u64).unwrap_or(u64::MAX),
            alpha_to_coverage_enabled: ms_v
                .get("alphaToCoverageEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };

        let layout = self.pipeline_layout_ref(v.get("layout"))?;
        Ok(device.create_render_pipeline(&RenderPipelineDescriptor {
            label: ph_label.as_deref(),
            layout,
            vertex: VertexState {
                module: vs_module,
                entry_point: vs_entry,
                compilation_options: vs_comp,
                buffers: &buffers,
            },
            primitive,
            depth_stencil,
            multisample,
            fragment,
            multiview_mask: None,
            cache: None,
        }))
    }

    /// begin render pass：附件借用 self 的视图表，desc 在本作用域消费，
    /// 返回的 RenderPass 只借 encoder。
    fn begin_render_pass<'e>(
        &'e self,
        encoder: &'e mut CommandEncoder,
        v: &Value,
    ) -> Result<RenderPass<'e>> {
        let mut color_attachments = Vec::new();
        for (i, ca) in v
            .get("colorAttachments")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("render pass: 缺 colorAttachments"))?
            .iter()
            .enumerate()
        {
            if ca.is_null() {
                color_attachments.push(None);
                continue;
            }
            let view = self.view_ref(h_of(
                ca.get("view")
                    .ok_or_else(|| anyhow!("colorAttachments[{i}] 缺 view"))?,
            )?)?;
            let resolve_target = match ca.get("resolveTarget") {
                Some(rtv) if !rtv.is_null() => Some(self.view_ref(h_of(rtv)?)?),
                _ => None,
            };
            let load_v = ca.get("loadOp").and_then(Value::as_str).unwrap_or("load");
            let store_op = match ca.get("storeOp").and_then(Value::as_str) {
                Some("discard") => StoreOp::Discard,
                _ => StoreOp::Store,
            };
            color_attachments.push(Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: Operations {
                    load: if load_v == "clear" {
                        let c = ca.get("clearValue").cloned().unwrap_or(Value::Null);
                        let at = |k: &str, idx: usize| -> f64 {
                            c.get(k)
                                .and_then(Value::as_f64)
                                .or_else(|| c.as_array().and_then(|a| a.get(idx)).and_then(Value::as_f64))
                                .unwrap_or(0.0)
                        };
                        LoadOp::Clear(Color {
                            r: at("r", 0),
                            g: at("g", 1),
                            b: at("b", 2),
                            a: at("a", 3),
                        })
                    } else {
                        LoadOp::Load
                    },
                    store: store_op,
                },
            }));
        }

        let depth_stencil_attachment = match v.get("depthStencilAttachment") {
            Some(d) if !d.is_null() => {
                let view = self.view_ref(h_of(
                    d.get("view")
                        .ok_or_else(|| anyhow!("depthStencilAttachment 缺 view"))?,
                )?)?;
                let depth_ops = d.get("depthLoadOp").and_then(Value::as_str).map(|op| {
                    Operations::<f32> {
                        load: if op == "clear" {
                            LoadOp::Clear(
                                d.get("depthClearValue")
                                    .and_then(Value::as_f64)
                                    .unwrap_or(1.0) as f32,
                            )
                        } else {
                            LoadOp::Load
                        },
                        store: match d.get("depthStoreOp").and_then(Value::as_str) {
                            Some("discard") => StoreOp::Discard,
                            _ => StoreOp::Store,
                        },
                    }
                });
                let stencil_ops = d.get("stencilLoadOp").and_then(Value::as_str).map(|op| {
                    Operations::<u32> {
                        load: if op == "clear" {
                            LoadOp::Clear(
                                d.get("stencilClearValue")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0) as u32,
                            )
                        } else {
                            LoadOp::Load
                        },
                        store: match d.get("stencilStoreOp").and_then(Value::as_str) {
                            Some("discard") => StoreOp::Discard,
                            _ => StoreOp::Store,
                        },
                    }
                });
                Some(RenderPassDepthStencilAttachment {
                    view,
                    depth_ops,
                    stencil_ops,
                })
            }
            _ => None,
        };

        let occlusion_query_set = match v.get("occlusionQuerySet") {
            Some(q) if !q.is_null() => {
                let h = h_of(q)?;
                Some(
                    self.query_sets
                        .get(&h)
                        .ok_or_else(|| unknown("querySet", h))?,
                )
            }
            _ => None,
        };

        Ok(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set,
            multiview_mask: None,
        }))
    }
}

fn res_kind(res: &WgpuResources, h: u32) -> Result<Kind> {
    res.kind_of
        .get(&h)
        .copied()
        .ok_or_else(|| anyhow!("句柄 {h} 未注册（kind 缺失）"))
}

fn constants_of(stage: &Value) -> Result<Vec<(&str, f64)>> {
    let mut out = Vec::new();
    if let Some(map) = stage.get("constants").and_then(Value::as_object) {
        for (k, vv) in map {
            out.push((k.as_str(), vv.as_f64().ok_or_else(|| anyhow!("常量 {k} 非数值"))?));
        }
    }
    Ok(out)
}

// ── readback ────────────────────────────────────────────────────────

fn readback_rgba(rt: &WgpuRuntime, tex: &Texture, w: u32, h: u32) -> Result<Vec<u8>> {
    let bpr = w * 4;
    let bpr_aligned = bpr.div_ceil(256) * 256;
    let size = bpr_aligned as u64 * h as u64;
    let read_buf = rt.device.create_buffer(&BufferDescriptor {
        label: Some("opencat-wgpu-readback"),
        size,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = rt.device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("opencat-wgpu-readback"),
    });
    enc.copy_texture_to_buffer(
        tex.as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &read_buf,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr_aligned),
                rows_per_image: None,
            },
        },
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    rt.queue.submit([enc.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    read_buf.map_async(MapMode::Read, .., move |r| {
        let _ = tx.send(r);
    });
    match rt.device.poll(PollType::wait_indefinitely()) {
        Ok(_) => {}
        Err(e) => return Err(anyhow!("wgpu readback: poll 失败: {e}")),
    }
    rx.recv()
        .map_err(|_| anyhow!("wgpu readback: 回调丢失"))?
        .map_err(|e| anyhow!("wgpu readback: map_async 失败: {e}"))?;
    let data = read_buf
        .get_mapped_range(..)
        .map_err(|e| anyhow!("wgpu readback: get_mapped_range 失败: {e}"))?
        .to_vec();
    read_buf.unmap();
    // 行重排（256 对齐）+ BGRA→RGBA。
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for row in 0..h as usize {
        let src = &data[row * bpr_aligned as usize..row * bpr_aligned as usize + bpr as usize];
        let dst = &mut rgba[row * bpr as usize..(row + 1) * bpr as usize];
        for px in 0..(bpr as usize) / 4 {
            dst[px * 4] = src[px * 4 + 2];
            dst[px * 4 + 1] = src[px * 4 + 1];
            dst[px * 4 + 2] = src[px * 4];
            dst[px * 4 + 3] = src[px * 4 + 3];
        }
    }
    Ok(rgba)
}

// ── payload 原始读取 ────────────────────────────────────────────────

fn u32(p: &[u8], at: usize) -> Result<u32> {
    p.get(at..at + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| anyhow!("payload 截断"))
}
fn i32(p: &[u8], at: usize) -> Result<i32> {
    p.get(at..at + 4)
        .map(|s| i32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| anyhow!("payload 截断"))
}
fn u64at(p: &[u8], at: usize) -> Result<u64> {
    p.get(at..at + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| anyhow!("payload 截断"))
}
fn range<'a>(bytes: &'a [u8], ranges: &'a [TableRange], id: usize) -> Result<&'a [u8]> {
    let r = ranges.get(id).ok_or_else(|| anyhow!("range id {id} 越界"))?;
    bytes
        .get(r.start as usize..(r.start + r.len) as usize)
        .ok_or_else(|| anyhow!("range {id} 字节越界"))
}
fn json_at(id: usize, bytes: &[u8], ranges: &[TableRange]) -> Result<Value> {
    serde_json::from_slice(range(bytes, ranges, id)?)
        .map_err(|e| anyhow!("描述符 JSON 解析失败: {e}"))
}

// ── JSON → wgpu 辅助 ────────────────────────────────────────────────

fn unknown(kind: &str, h: u32) -> anyhow::Error {
    anyhow!("未知 {kind} 句柄 {h}（首帧后销毁或未创建）")
}

fn h_of(v: &Value) -> Result<u32> {
    v.get("h")
        .and_then(Value::as_u64)
        .map(|h| h as u32)
        .ok_or_else(|| anyhow!("句柄包装 {{\"h\":..}} 缺失: {v}"))
}

fn field_u32(v: &Value, k: &str) -> Result<u32> {
    v.get(k)
        .and_then(Value::as_u64)
        .map(|x| x as u32)
        .ok_or_else(|| anyhow!("字段 {k} 缺失或非数值"))
}

fn u32_opt(v: Option<&Value>) -> Option<u32> {
    v.and_then(Value::as_u64).map(|x| x as u32)
}

fn extent_of(v: &Value) -> Result<Extent3d> {
    Ok(Extent3d {
        width: v.get("width").and_then(Value::as_u64).unwrap_or(1) as u32,
        height: v.get("height").and_then(Value::as_u64).unwrap_or(1) as u32,
        depth_or_array_layers: v
            .get("depthOrArrayLayers")
            .and_then(Value::as_u64)
            .unwrap_or(1) as u32,
    })
}

fn origin_of(v: Option<&Value>) -> Origin3d {
    let v = v.cloned().unwrap_or(Value::Null);
    let at = |i: usize| -> u32 {
        v.as_array()
            .and_then(|a| a.get(i))
            .and_then(Value::as_u64)
            .map(|x| x as u32)
            .or_else(|| {
                v.get(["x", "y", "z"][i])
                    .and_then(Value::as_u64)
                    .map(|x| x as u32)
            })
            .unwrap_or(0)
    };
    Origin3d { x: at(0), y: at(1), z: at(2) }
}

fn view_desc_of(v: &Value) -> Result<TextureViewDescriptor<'_>> {
    Ok(TextureViewDescriptor {
        label: None,
        format: v
            .get("format")
            .and_then(Value::as_str)
            .map(|s| spec::must("format", s, spec::texture_format))
            .transpose()?,
        dimension: opt_enum(v.get("dimension"), spec::texture_view_dimension)?,
        usage: None,
        aspect: opt_enum(v.get("aspect"), spec::texture_aspect)?.unwrap_or(TextureAspect::All),
        base_mip_level: v.get("baseMipLevel").and_then(Value::as_u64).unwrap_or(0) as u32,
        mip_level_count: u32_opt(v.get("mipLevelCount")),
        base_array_layer: v.get("baseArrayLayer").and_then(Value::as_u64).unwrap_or(0) as u32,
        array_layer_count: u32_opt(v.get("arrayLayerCount")),
    })
}

fn fmt_of(v: &Value) -> Result<TextureFormat> {
    let s = v
        .get("format")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("描述符缺 format"))?;
    spec::must("format", s, spec::texture_format)
}

/// 必填枚举：JSON 缺字段用 default，给了就按 spec 名解析（解析失败报错）。
fn enum_or<T: Copy>(
    v: Option<&Value>,
    p: fn(&str) -> Option<T>,
    default: T,
) -> Result<T> {
    match v.and_then(Value::as_str) {
        Some(s) => p(s).ok_or_else(|| anyhow!("无法解析枚举 '{s}'")),
        None => Ok(default),
    }
}

fn opt_enum<T: Copy>(v: Option<&Value>, p: fn(&str) -> Option<T>) -> Result<Option<T>> {
    match v.and_then(Value::as_str) {
        Some(s) => p(s).map(Some).ok_or_else(|| anyhow!("无法解析枚举 '{s}'")),
        None => Ok(None),
    }
}

fn binding_type_of(e: &Value) -> Result<BindingType> {
    if let Some(b) = e.get("buffer") {
        return Ok(BindingType::Buffer {
            ty: match b.get("type").and_then(Value::as_str).unwrap_or("uniform") {
                "storage" | "read-only-storage" => BufferBindingType::Storage { read_only: true },
                "mutable-storage" => BufferBindingType::Storage { read_only: false },
                _ => BufferBindingType::Uniform,
            },
            has_dynamic_offset: b
                .get("hasDynamicOffset")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            min_binding_size: b
                .get("minBindingSize")
                .and_then(Value::as_u64)
                .and_then(NonZeroU64::new),
        });
    }
    if let Some(s) = e.get("sampler") {
        return Ok(BindingType::Sampler(match s.get("type").and_then(Value::as_str).unwrap_or("filtering") {
            "non-filtering" => SamplerBindingType::NonFiltering,
            "comparison" => SamplerBindingType::Comparison,
            _ => SamplerBindingType::Filtering,
        }));
    }
    if let Some(t) = e.get("texture") {
        return Ok(BindingType::Texture {
            sample_type: match t.get("sampleType").and_then(Value::as_str).unwrap_or("float") {
                "unfilterable-float" => TextureSampleType::Float { filterable: false },
                "depth" => TextureSampleType::Depth,
                "sint" => TextureSampleType::Sint,
                "uint" => TextureSampleType::Uint,
                _ => TextureSampleType::Float { filterable: true },
            },
            view_dimension: opt_enum(t.get("viewDimension"), spec::texture_view_dimension)?
                .unwrap_or(TextureViewDimension::D2),
            multisampled: t.get("multisampled").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    if let Some(s) = e.get("storageTexture") {
        return Ok(BindingType::StorageTexture {
            access: match s.get("access").and_then(Value::as_str).unwrap_or("write-only") {
                "read-only" => wgpu::StorageTextureAccess::ReadOnly,
                "read-write" => wgpu::StorageTextureAccess::ReadWrite,
                _ => wgpu::StorageTextureAccess::WriteOnly,
            },
            format: fmt_of(s)?,
            view_dimension: opt_enum(s.get("viewDimension"), spec::texture_view_dimension)?
                .unwrap_or(TextureViewDimension::D2),
        });
    }
    bail!("BGL entry: 无 buffer/sampler/texture/storageTexture 之一")
}
