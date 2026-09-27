//! f64 AST 解释器 —— "Rust 手写逐像素循环"世界的 lambda 形态。
//!
//! 语义锚点：与手写 Rust / 参考 JS 逐位对齐——所有用户算术是 f64（JS
//! Number），整数语义只在 `h01`/`imul`/`u32`/`i32`/`byte` 内建内部（与
//! `Math.imul`/`|0` 一致）。输出约定：lambda 返回 0..1 straight RGBA，
//! 量化为 `(v.clamp(0,1)*255).round()`；需要 JS 截断语义的量在 lambda 内
//! 先过 `byte()`（整数/255 的往返在 f64 与 round 下精确还原）。

use std::sync::Arc;

use rayon::prelude::*;

use super::program::{Backend, BinOp, BuiltinId as B, Expr, Program, Stmt, UnOp, Val};

/// child 采样的像素源（straight RGBA8）。
#[derive(Clone)]
pub struct ChildImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl ChildImage {
    /// 最近邻 + clamp-to-edge 采样，返回 0..1 straight 色。
    /// SKSL 侧 image child 同为 clamp tile；坐标为像素空间（像素 `i` 中心=i）。
    pub fn sample(&self, px: f64, py: f64) -> [f64; 4] {
        sample_rgba(&self.rgba, self.width, self.height, px, py)
    }
}

/// 最近邻 + clamp-to-edge 的 RGBA8 采样（行主序 straight 色，像素 `i` 中心=i）。
fn sample_rgba(buf: &[u8], width: u32, height: u32, px: f64, py: f64) -> [f64; 4] {
    let clamp_i = |v: f64, max: i64| -> usize {
        let i = v.floor() as i64;
        i.clamp(0, max) as usize
    };
    let ix = clamp_i(px, width as i64 - 1);
    let iy = clamp_i(py, height as i64 - 1);
    let o = (iy * width as usize + ix) * 4;
    [
        buf[o] as f64 / 255.0,
        buf[o + 1] as f64 / 255.0,
        buf[o + 2] as f64 / 255.0,
        buf[o + 3] as f64 / 255.0,
    ]
}

/// 一次逐像素求值的常量上下文。
pub struct InterpCtx<'a> {
    /// 按 spec 声明序展开的 uniform 值。
    pub uniforms: &'a [Val],
    /// drawRect dst 的 [x, y, w, h]。
    pub rect: [f64; 4],
    pub children: &'a [ChildImage],
    /// 仅 scan 类：`get(dx, dy)` 读取的 in-progress 缓冲与当前像素坐标。
    pub scan: Option<ScanView<'a>>,
}

/// scan pass 的采样视图：目标缓冲（being written）+ 当前像素。
#[derive(Clone, Copy)]
pub struct ScanView<'a> {
    pub buf: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub x: f64,
    pub y: f64,
}

/// scan 遍历方向。executor 拥有顺序（JS 源码里没有循环）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanDirection {
    /// 逐行自上而下、行内自左向右。
    Forward,
    /// 逐行自下而上、行内自右向左。
    Backward,
}

impl ScanDirection {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "forward" => Some(Self::Forward),
            "backward" => Some(Self::Backward),
            _ => None,
        }
    }
}

/// 渲染一帧 w×h 的 lambda 输出（straight RGBA8）。rayon 按行并行。
pub fn render(program: &Program, width: u32, height: u32, ctx: &InterpCtx) -> Vec<u8> {
    debug_assert_eq!(program.backend(), Backend::Cpu);
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; w * h * 4];
    let row_len = w * 4;
    out.par_chunks_exact_mut(row_len).enumerate().for_each(|(y, row)| {
        let py = y as f64 + 0.5;
        for (x, px_chunk) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let px = x as f64 + 0.5;
            let rgba = eval_pixel(program, px, py, ctx);
            let q = |v: f64| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
            *px_chunk = [q(rgba[0]), q(rgba[1]), q(rgba[2]), q(rgba[3])];
        }
    });
    out
}

/// scan pass：对 `buf`（w×h×4，straight RGBA8）**就地**做一遍顺序扫描。
/// 第 (x, y) 像素的返回值量化后立即写回，后续像素的 `get` 读到的即已更新
/// 的缓冲——顺序语义由本函数拥有（Forward/Backward），JS 只提供逐像素
/// 更新函数。单线程：扫描的邻居依赖无法按行并行（构建一次的数据，代价可接受）。
pub fn render_scan(
    program: &Program,
    width: u32,
    height: u32,
    uniforms: &[Val],
    direction: ScanDirection,
    buf: &mut [u8],
) {
    debug_assert_eq!(program.backend(), Backend::Cpu);
    debug_assert_eq!(buf.len(), width as usize * height as usize * 4);
    let w = width as usize;
    let h = height as usize;
    let q = |v: f64| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
    let write_px = |buf: &mut [u8], x: usize, y: usize, rgba: [f64; 4]| {
        let o = (y * w + x) * 4;
        buf[o] = q(rgba[0]);
        buf[o + 1] = q(rgba[1]);
        buf[o + 2] = q(rgba[2]);
        buf[o + 3] = q(rgba[3]);
    };
    match direction {
        ScanDirection::Forward => {
            for y in 0..h {
                for x in 0..w {
                    let ctx = InterpCtx {
                        uniforms,
                        rect: [0.0, 0.0, width as f64, height as f64],
                        children: &[],
                        scan: Some(ScanView {
                            buf,
                            width,
                            height,
                            x: x as f64,
                            y: y as f64,
                        }),
                    };
                    let rgba = eval_scan_pixel(program, &ctx);
                    write_px(buf, x, y, rgba);
                }
            }
        }
        ScanDirection::Backward => {
            for y in (0..h).rev() {
                for x in (0..w).rev() {
                    let ctx = InterpCtx {
                        uniforms,
                        rect: [0.0, 0.0, width as f64, height as f64],
                        children: &[],
                        scan: Some(ScanView {
                            buf,
                            width,
                            height,
                            x: x as f64,
                            y: y as f64,
                        }),
                    };
                    let rgba = eval_scan_pixel(program, &ctx);
                    write_px(buf, x, y, rgba);
                }
            }
        }
    }
}

/// scan 类的单像素求值（整型像素坐标，无 +0.5 中心偏移）。
fn eval_scan_pixel(program: &Program, ctx: &InterpCtx) -> [f64; 4] {
    let view = ctx.scan.expect("render_scan requires scan view");
    let mut locals: Vec<Val> = vec![Val::F(0.0); program.local_tys.len()];
    for stmt in &program.body {
        match eval_stmt(stmt, &mut locals, view.x, view.y, ctx) {
            Flow::Continue => {}
            Flow::Return(v) => return val_to_rgba(v),
        }
    }
    [0.0, 0.0, 0.0, 0.0]
}

/// 单像素求值，返回 0..1 straight RGBA。
pub fn eval_pixel(program: &Program, px: f64, py: f64, ctx: &InterpCtx) -> [f64; 4] {
    let mut locals: Vec<Val> = vec![Val::F(0.0); program.local_tys.len()];
    for stmt in &program.body {
        match eval_stmt(stmt, &mut locals, px, py, ctx) {
            Flow::Continue => {}
            Flow::Return(v) => return val_to_rgba(v),
        }
    }
    [0.0, 0.0, 0.0, 0.0]
}

fn val_to_rgba(v: Val) -> [f64; 4] {
    match v {
        Val::V4(c) => c,
        Val::V3(c) => [c[0], c[1], c[2], 1.0],
        Val::F(f) => [f, f, f, 1.0],
        _ => [0.0, 0.0, 0.0, 0.0],
    }
}

enum Flow {
    Continue,
    Return(Val),
}

fn eval_stmt(
    stmt: &Stmt,
    locals: &mut Vec<Val>,
    px: f64,
    py: f64,
    ctx: &InterpCtx,
) -> Flow {
    match stmt {
        Stmt::Let { slot, value } => {
            let v = eval_expr(value, locals, px, py, ctx);
            locals[*slot as usize] = v;
            Flow::Continue
        }
        Stmt::Assign { slot, value } => {
            let v = eval_expr(value, locals, px, py, ctx);
            locals[*slot as usize] = v;
            Flow::Continue
        }
        Stmt::If { cond, then, els } => {
            let c = eval_expr(cond, locals, px, py, ctx);
            let branch = if matches!(c, Val::B(true)) { then } else { els };
            for s in branch {
                match eval_stmt(s, locals, px, py, ctx) {
                    Flow::Continue => {}
                    Flow::Return(v) => return Flow::Return(v),
                }
            }
            Flow::Continue
        }
        Stmt::Return(expr) => Flow::Return(eval_expr(expr, locals, px, py, ctx)),
    }
}

fn eval_expr(
    expr: &Expr,
    locals: &mut Vec<Val>,
    px: f64,
    py: f64,
    ctx: &InterpCtx,
) -> Val {
    match expr {
        Expr::LitF(v) => Val::F(*v),
        Expr::LitB(b) => Val::B(*b),
        Expr::Local(slot) => locals[*slot as usize],
        Expr::Uniform(idx) => ctx.uniforms[*idx as usize],
        // 解释器直接在 rect 本地坐标系渲染（输出图即 rect 内容），
        // uv = 像素中心 (col+0.5, row+0.5)；`rect` 参数仍是 dst 的 [x,y,w,h]。
        // SKSL 侧 uv = xy - u_oc_rect.xy，两后端一致。
        Expr::Uv => Val::V2([px, py]),
        Expr::Rect => Val::V4(ctx.rect),
        // child 只能作为 .eval 接收者出现（parse 层拒绝其它用法）
        Expr::Child(_) => Val::F(0.0),
        Expr::Bin(op, l, r, _) => {
            // && / || 短路（JS 语义）
            if matches!(op, BinOp::And | BinOp::Or) {
                let lv = eval_expr(l, locals, px, py, ctx);
                let lb = matches!(lv, Val::B(true));
                let result = match op {
                    BinOp::And => {
                        if !lb {
                            Val::B(false)
                        } else {
                            Val::B(matches!(eval_expr(r, locals, px, py, ctx), Val::B(true)))
                        }
                    }
                    BinOp::Or => {
                        if lb {
                            Val::B(true)
                        } else {
                            Val::B(matches!(eval_expr(r, locals, px, py, ctx), Val::B(true)))
                        }
                    }
                    _ => unreachable!(),
                };
                return result;
            }
            let (lv, rv) = (
                eval_expr(l, locals, px, py, ctx),
                eval_expr(r, locals, px, py, ctx),
            );
            eval_bin(*op, lv, rv)
        }
        Expr::Un(op, arg, _) => {
            let a = eval_expr(arg, locals, px, py, ctx);
            match op {
                UnOp::Neg => match a {
                    Val::F(v) => Val::F(-v),
                    Val::V2(c) => Val::V2([-c[0], -c[1]]),
                    Val::V3(c) => Val::V3([-c[0], -c[1], -c[2]]),
                    Val::V4(c) => Val::V4([-c[0], -c[1], -c[2], -c[3]]),
                    _ => Val::F(0.0),
                },
                UnOp::Not => Val::B(!matches!(a, Val::B(true))),
            }
        }
        Expr::Cond(c, t, f, _) => {
            let cv = eval_expr(c, locals, px, py, ctx);
            if matches!(cv, Val::B(true)) {
                eval_expr(t, locals, px, py, ctx)
            } else {
                eval_expr(f, locals, px, py, ctx)
            }
        }
        Expr::Call { id, args, .. } => {
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(eval_expr(a, locals, px, py, ctx));
            }
            eval_builtin(*id, &vals)
        }
        Expr::Swizzle { base, comps, len, .. } => {
            let v = eval_expr(base, locals, px, py, ctx);
            let c = match v {
                Val::V2(c) => c.to_vec(),
                Val::V3(c) => c.to_vec(),
                Val::V4(c) => c.to_vec(),
                _ => Vec::new(),
            };
            if *len as usize == 1 {
                Val::F(c.get(comps[0] as usize).copied().unwrap_or(0.0))
            } else {
                let n = *len as usize;
                let mut out = [0.0f64; 4];
                for (i, comp) in comps[..n].iter().enumerate() {
                    out[i] = c.get(*comp as usize).copied().unwrap_or(0.0);
                }
                match n {
                    2 => Val::V2([out[0], out[1]]),
                    3 => Val::V3([out[0], out[1], out[2]]),
                    _ => Val::V4(out),
                }
            }
        }
        Expr::VecCons(items, _) => {
            let mut out = [0.0f64; 4];
            for (i, item) in items.iter().enumerate() {
                out[i] = eval_expr(item, locals, px, py, ctx).as_f64();
            }
            match items.len() {
                2 => Val::V2([out[0], out[1]]),
                3 => Val::V3([out[0], out[1], out[2]]),
                _ => Val::V4(out),
            }
        }
        Expr::Eval { child, pos } => {
            let p = eval_expr(pos, locals, px, py, ctx);
            let (sx, sy) = match p {
                Val::V2(c) => (c[0], c[1]),
                v => (v.as_f64(), v.as_f64()),
            };
            let color = ctx
                .children
                .get(*child)
                .map(|img| img.sample(sx, sy))
                .unwrap_or([0.0, 0.0, 0.0, 0.0]);
            Val::V4(color)
        }
        Expr::ScanGet { dx, dy } => {
            let Some(view) = &ctx.scan else {
                // parse 层保证 get 只出现在 scan 类里，解释路径必有视图
                return Val::V4([0.0, 0.0, 0.0, 0.0]);
            };
            let ddx = eval_expr(dx, locals, px, py, ctx).as_f64();
            let ddy = eval_expr(dy, locals, px, py, ctx).as_f64();
            Val::V4(sample_rgba(
                view.buf,
                view.width,
                view.height,
                view.x + ddx,
                view.y + ddy,
            ))
        }
    }
}

fn eval_bin(op: BinOp, l: Val, r: Val) -> Val {
    use Val::*;
    match (op, l, r) {
        (BinOp::Add, F(a), F(b)) => F(a + b),
        (BinOp::Sub, F(a), F(b)) => F(a - b),
        (BinOp::Mul, F(a), F(b)) => F(a * b),
        (BinOp::Div, F(a), F(b)) => F(a / b),
        (BinOp::Rem, F(a), F(b)) => F(js_rem(a, b)),
        (BinOp::Gt, F(a), F(b)) => B(a > b),
        (BinOp::Lt, F(a), F(b)) => B(a < b),
        (BinOp::Ge, F(a), F(b)) => B(a >= b),
        (BinOp::Le, F(a), F(b)) => B(a <= b),
        (BinOp::Eq, F(a), F(b)) => B(a == b),
        (BinOp::Ne, F(a), F(b)) => B(a != b),
        (BinOp::Eq, B(a), B(b)) => B(a == b),
        (BinOp::Ne, B(a), B(b)) => B(a != b),
        // 向量与标量/同维向量
        (op, V2(a), V2(b)) => v2_op(op, &a, &[b[0], b[1]]),
        (op, V3(a), V3(b)) => v3_op(op, &a, &[b[0], b[1], b[2]]),
        (op, V4(a), V4(b)) => v4_op(op, &a, &[b[0], b[1], b[2], b[3]]),
        (op, V2(a), F(b)) => v2_op(op, &a, &[b, b]),
        (op, F(a), V2(b)) => v2_op(op, &[a, a], &b),
        (op, V3(a), F(b)) => v3_op(op, &a, &[b, b, b]),
        (op, F(a), V3(b)) => v3_op(op, &[a, a, a], &b),
        (op, V4(a), F(b)) => v4_op(op, &a, &[b, b, b, b]),
        (op, F(a), V4(b)) => v4_op(op, &[a, a, a, a], &b),
        _ => F(0.0),
    }
}

fn js_rem(a: f64, b: f64) -> f64 {
    // JS %：截断余数 a - b*trunc(a/b)
    a - b * (a / b).trunc()
}

fn v2_op(op: BinOp, a: &[f64; 2], b: &[f64; 2]) -> Val {
    Val::V2([pair_op(op, a[0], b[0]), pair_op(op, a[1], b[1])])
}
fn v3_op(op: BinOp, a: &[f64; 3], b: &[f64; 3]) -> Val {
    Val::V3([
        pair_op(op, a[0], b[0]),
        pair_op(op, a[1], b[1]),
        pair_op(op, a[2], b[2]),
    ])
}
fn v4_op(op: BinOp, a: &[f64; 4], b: &[f64; 4]) -> Val {
    Val::V4([
        pair_op(op, a[0], b[0]),
        pair_op(op, a[1], b[1]),
        pair_op(op, a[2], b[2]),
        pair_op(op, a[3], b[3]),
    ])
}
fn pair_op(op: BinOp, a: f64, b: f64) -> f64 {
    match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => a / b,
        BinOp::Rem => js_rem(a, b),
        _ => 0.0,
    }
}

/// JS ToUint32：截断后对 2^32 取模（f64 as u32 在 Rust 是饱和而非回绕，
/// 必须显式实现；溶解域内的值都在小整数范围）。
fn to_uint32(x: f64) -> u32 {
    if !x.is_finite() {
        return 0;
    }
    let t = x.trunc();
    (t as i64).rem_euclid(1 << 32) as u32
}

/// JS ToInt32。
fn to_int32(x: f64) -> i32 {
    to_uint32(x) as i32
}

pub fn eval_builtin(id: B, args: &[Val]) -> Val {
    // 逐分量（带标量广播）的算术内建：与 GLSL/SKSL 语义一致。
    // lane 闭包收到已按分量展开的标量参数。
    fn elementwise(args: &[Val], lanes: usize, f: impl Fn(&[f64]) -> f64) -> Val {
        let lane_arg = |lane: usize, v: &Val| match v {
            Val::V2(c) => c[lane.min(1)],
            Val::V3(c) => c[lane.min(2)],
            Val::V4(c) => c[lane.min(3)],
            other => other.as_f64(),
        };
        let mut out = [0.0f64; 4];
        // 内建最多 3 参；宽于 3 的调用在 signature 检查就被拒绝
        let mut scalars = [0.0f64; 3];
        for (lane, slot) in out.iter_mut().enumerate().take(lanes) {
            for (i, v) in args.iter().enumerate().take(3) {
                scalars[i] = lane_arg(lane, v);
            }
            *slot = f(&scalars[..args.len().min(3)]);
        }
        match lanes {
            1 => Val::F(out[0]),
            2 => Val::V2([out[0], out[1]]),
            3 => Val::V3([out[0], out[1], out[2]]),
            _ => Val::V4(out),
        }
    }
    // 确定输出向量宽度：各向量参数维度一致（signature 已查），标量广播
    let lanes_of = |args: &[Val]| -> usize {
        args.iter()
            .filter_map(|v| v.ty().vec_len())
            .next()
            .unwrap_or(1)
    };
    let math = |args: &[Val], f: &dyn Fn(&[f64]) -> f64| -> Val {
        elementwise(args, lanes_of(args), |s| f(s))
    };
    match id {
        B::Abs => math(args, &|s| s[0].abs()),
        B::Sign => math(args, &|s| {
            // JS Math.sign：±0 保持符号（与 GLSL sign 的 0 归并不同）
            if s[0] > 0.0 {
                1.0
            } else if s[0] < 0.0 {
                -1.0
            } else {
                s[0]
            }
        }),
        B::Floor => math(args, &|s| s[0].floor()),
        B::Ceil => math(args, &|s| s[0].ceil()),
        B::Fract => math(args, &|s| s[0] - s[0].floor()),
        B::Sqrt => math(args, &|s| s[0].sqrt()),
        B::Exp => math(args, &|s| s[0].exp()),
        B::Exp2 => math(args, &|s| s[0].exp2()),
        B::Log => math(args, &|s| s[0].ln()),
        B::Log2 => math(args, &|s| s[0].log2()),
        B::Sin => math(args, &|s| s[0].sin()),
        B::Cos => math(args, &|s| s[0].cos()),
        B::Tan => math(args, &|s| s[0].tan()),
        B::Asin => math(args, &|s| s[0].asin()),
        B::Acos => math(args, &|s| s[0].acos()),
        B::Atan => math(args, &|s| s[0].atan()),
        B::Atan2 => math(args, &|s| s[0].atan2(s[1])),
        B::Pow => math(args, &|s| s[0].powf(s[1])),
        B::Min => math(args, &|s| s[0].min(s[1])),
        B::Max => math(args, &|s| s[0].max(s[1])),
        B::Clamp => math(args, &|s| s[1].max(s[0]).min(s[2])), // min(max(x, lo), hi)
        B::Step => math(args, &|s| if s[1] < s[0] { 0.0 } else { 1.0 }), // step(edge, x)
        B::Mix => math(args, &|s| s[0] + (s[1] - s[0]) * s[2]),
        B::Smoothstep => math(args, &|s| {
            let t = ((s[2] - s[0]) / (s[1] - s[0])).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }),
        B::Length => match args.first() {
            Some(Val::V2(c)) => Val::F((c[0] * c[0] + c[1] * c[1]).sqrt()),
            Some(Val::V3(c)) => Val::F((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()),
            Some(Val::V4(c)) => {
                Val::F((c[0] * c[0] + c[1] * c[1] + c[2] * c[2] + c[3] * c[3]).sqrt())
            }
            _ => Val::F(args.first().map(|v| v.as_f64()).unwrap_or(0.0).abs()),
        },
        B::Distance => {
            let (a, b) = (
                args.first().copied().unwrap_or(Val::F(0.0)),
                args.get(1).copied().unwrap_or(Val::F(0.0)),
            );
            let d = eval_bin(BinOp::Sub, a, b);
            eval_builtin(B::Length, &[d])
        }
        B::Dot => {
            let (a, b) = (
                args.first().copied().unwrap_or(Val::F(0.0)),
                args.get(1).copied().unwrap_or(Val::F(0.0)),
            );
            match (a, b) {
                (Val::V2(x), Val::V2(y)) => Val::F(x[0] * y[0] + x[1] * y[1]),
                (Val::V3(x), Val::V3(y)) => Val::F(x[0] * y[0] + x[1] * y[1] + x[2] * y[2]),
                (Val::V4(x), Val::V4(y)) => {
                    Val::F(x[0] * y[0] + x[1] * y[1] + x[2] * y[2] + x[3] * y[3])
                }
                (Val::F(x), Val::F(y)) => Val::F(x * y),
                _ => Val::F(0.0),
            }
        }
        B::Cross => match (args.first(), args.get(1)) {
            (Some(Val::V3(a)), Some(Val::V3(b))) => Val::V3([
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]),
            _ => Val::V3([0.0; 3]),
        },
        B::Normalize => match args.first() {
            Some(Val::V2(c)) => {
                let l = (c[0] * c[0] + c[1] * c[1]).sqrt();
                Val::V2([c[0] / l, c[1] / l])
            }
            Some(Val::V3(c)) => {
                let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
                Val::V3([c[0] / l, c[1] / l, c[2] / l])
            }
            Some(Val::V4(c)) => {
                let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2] + c[3] * c[3]).sqrt();
                Val::V4([c[0] / l, c[1] / l, c[2] / l, c[3] / l])
            }
            other => Val::F(other.map(|v| v.as_f64()).unwrap_or(1.0).signum()),
        },
        B::Byte => Val::F(to_int32(f64_arg(args, 0)) as f64),
        B::CastU => Val::F(to_uint32(f64_arg(args, 0)) as f64),
        B::CastI => Val::F(to_int32(f64_arg(args, 0)) as f64),
        B::CastF => Val::F(f64_arg(args, 0)),
        B::Imul => Val::F(
            to_int32(f64_arg(args, 0)).wrapping_mul(to_int32(f64_arg(args, 1))) as f64,
        ),
        B::H01 => {
            let h = h01(
                to_uint32(f64_arg(args, 0)),
                to_uint32(f64_arg(args, 1)),
                to_uint32(f64_arg(args, 2)),
            );
            Val::F(h)
        }
    }
}

fn f64_arg(args: &[Val], i: usize) -> f64 {
    args.get(i).map(|v| v.as_f64()).unwrap_or(0.0)
}

/// 确定性像素哈希（参考 index.html:1316-1321 的 mulberry 风格 scramble）：
/// u32 wrapping 乘、XOR、逻辑右移。
pub fn h01(a: u32, b: u32, c: u32) -> f64 {
    let mut h = (a.wrapping_add(374_761)).wrapping_mul(0x9E37_79B1)
        ^ (b.wrapping_add(668_265)).wrapping_mul(0x85EB_CA6B)
        ^ (c.wrapping_add(951_274)).wrapping_mul(0xC2B2_AE35);
    h = (h ^ (h >> 16)).wrapping_mul(0x27D4_EB2F);
    (h ^ (h >> 15)) as f64 / 4294967296.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// h01 与参考实现的 oracle 锚点逐位一致。
    #[test]
    fn h01_matches_oracle_anchors() {
        let cases: [(u32, u32, u32, f64); 4] = [
            (0, 0, 0, 0.92743052844889462),
            (1, 2, 3, 0.7796348906122148),
            (77, 144, 8, 0.5401346527505666),
            (5759, 1028, 60, 0.56664541969075799),
        ];
        for (a, b, c, want) in cases {
            assert_eq!(h01(a, b, c), want, "h01({a},{b},{c})");
        }
    }

    #[test]
    fn byte_and_uint_semantics() {
        assert_eq!(eval_builtin(B::Byte, &[Val::F(128.6)]), Val::F(128.0));
        assert_eq!(eval_builtin(B::Byte, &[Val::F(255.0)]), Val::F(255.0));
        assert_eq!(eval_builtin(B::CastU, &[Val::F(-1.0)]), Val::F(4294967295.0));
        assert_eq!(eval_builtin(B::CastI, &[Val::F(-1.0)]), Val::F(-1.0));
    }

    #[test]
    fn js_remainder_semantics() {
        assert_eq!(js_rem(-5.0, 3.0), -2.0);
        assert_eq!(js_rem(5.0, 3.0), 2.0);
        assert_eq!(js_rem(7.5, 2.0), 1.5);
    }
}
