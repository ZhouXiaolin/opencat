//! Effect lambda 的自有 IR：oxc AST 解析并白名单校验后落到这份 owned 结构，
//! SKSL codegen（[`super::lower_sksl`]）与 f64 解释器（[`super::interp`]）都只认它。

pub use super::stdlib::BuiltinId;

/// 迷你静态类型系统。JS Number 即 f64——用户表达式只有浮点标量/向量/布尔；
/// 整数语义（wrapping、截断）只存在于 `h01`/`imul`/`u32`/`i32`/`byte` 内建
/// 内部，与参考 JS 的 `Math.imul`/`|0` 行为一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Float,
    Bool,
    Vec2,
    Vec3,
    Vec4,
}

impl Ty {
    pub fn vec_len(self) -> Option<usize> {
        match self {
            Ty::Vec2 => Some(2),
            Ty::Vec3 => Some(3),
            Ty::Vec4 => Some(4),
            _ => None,
        }
    }

    pub fn is_scalar_num(self) -> bool {
        matches!(self, Ty::Float)
    }

    pub fn sksl_name(self) -> &'static str {
        match self {
            Ty::Float => "float",
            Ty::Bool => "bool",
            Ty::Vec2 => "float2",
            Ty::Vec3 => "float3",
            Ty::Vec4 => "float4",
        }
    }

    /// spec hash 用稳定判别值（不跨进程持久化，仅会话内比较）。
    pub fn to_discriminant(self) -> u8 {
        match self {
            Ty::Float => 0,
            Ty::Bool => 1,
            Ty::Vec2 => 2,
            Ty::Vec3 => 3,
            Ty::Vec4 => 4,
        }
    }
}

/// 解释器的运行期值（f64 语义，与手写 Rust / 参考 JS 逐像素循环一致）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Val {
    F(f64),
    B(bool),
    V2([f64; 2]),
    V3([f64; 3]),
    V4([f64; 4]),
}

impl Val {
    pub fn ty(self) -> Ty {
        match self {
            Val::F(_) => Ty::Float,
            Val::B(_) => Ty::Bool,
            Val::V2(_) => Ty::Vec2,
            Val::V3(_) => Ty::Vec3,
            Val::V4(_) => Ty::Vec4,
        }
    }

    pub fn as_f64(self) -> f64 {
        match self {
            Val::F(v) => v,
            _ => 0.0,
        }
    }
}

/// Lambda 的参数槽。pixel 类约定：第一参数 = `uv`（rect 本地像素坐标，
/// 像素中心取 `i + 0.5`）；名为 `rect` 的参数 = `[x, y, w, h]`；名为 `u` 的
/// 参数 = uniforms 访问器（`u.<name>`）；其余参数依序为 child shader。
/// scan 类（[`ScanKind::Scan`]）约定：第一参数 = `get`（本目标 in-progress
/// 采样器，`get(dx, dy)`）；`rect`/`u` 同上；child 不可用。
#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    Uv,
    Rect,
    /// `u` 参数：uniforms 访问器，只能以 `u.<name>` 出现。
    Uniforms,
    Child { index: usize },
    /// `get` 参数（仅 scan 类）：`get(dx, dy)` 读目标 in-progress 字节。
    Get,
}

/// lambda 类。pixel = 逐像素纯函数（uv → 色）；scan = 顺序扫描 pass
/// （executor 拥有遍历顺序，JS 拥有邻居更新函数）。scan 恒走 CPU 解释器。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ScanKind {
    #[default]
    Pixel,
    Scan,
}

#[derive(Debug, Clone)]
pub struct UniformSpec {
    pub name: String,
    pub ty: Ty,
}

/// 二元运算。位运算不在其列——白名单直接拒绝（提示 `imul`/`h01`/`byte`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub enum Expr {
    LitF(f64),
    LitB(bool),
    Local(u32),
    Uniform(u32),
    Uv,
    Rect,
    Child(usize),
    Bin(BinOp, Box<Expr>, Box<Expr>, Ty),
    Un(UnOp, Box<Expr>, Ty),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>, Ty),
    Call { id: BuiltinId, args: Vec<Expr>, ty: Ty },
    Swizzle { base: Box<Expr>, comps: [u8; 4], len: u8, ty: Ty },
    /// 数组字面量构造向量，2-4 个标量分量。
    VecCons(Vec<Expr>, Ty),
    /// `child.eval(pos)`，恒为 Vec4（0..1 straight 色）。
    Eval { child: usize, pos: Box<Expr> },
    /// `get(dx, dy)`（仅 scan 类）：读目标 in-progress 字节，恒为 Vec4
    /// （0..1 straight 色，clamp-to-edge）。
    ScanGet { dx: Box<Expr>, dy: Box<Expr> },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { slot: u32, value: Expr },
    Assign { slot: u32, value: Expr },
    If { cond: Expr, then: Vec<Stmt>, els: Vec<Stmt> },
    Return(Expr),
}

/// 后端派发。`Auto` 依 stdlib 能力标记决定：用到 `CpuOnly` op（`h01`/`imul`，
/// 整数精确语义）→ 解释器；纯浮点 → SKSL。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Sksl,
    Cpu,
}

#[derive(Debug, Clone)]
pub struct Program {
    /// 原始 lambda 源码（报错摘录用）。
    pub source: String,
    /// 参数按声明序。
    pub params: Vec<(String, Param)>,
    pub uniforms: Vec<UniformSpec>,
    /// child 参数名，依声明序（索引即 [`Param::Child`] 的 index）。
    pub children: Vec<String>,
    pub local_tys: Vec<Ty>,
    pub body: Vec<Stmt>,
    pub return_ty: Ty,
    pub uses_cpu_only: bool,
    pub backend_override: Option<Backend>,
    pub kind: ScanKind,
}

impl Program {
    /// 实际派发的后端。scan 类恒为 CPU（顺序扫描无 SKSL 形态）。
    pub fn backend(&self) -> Backend {
        if self.kind == ScanKind::Scan {
            return Backend::Cpu;
        }
        match self.backend_override {
            Some(Backend::Cpu) => Backend::Cpu,
            Some(Backend::Sksl) => {
                if self.uses_cpu_only {
                    // mod.rs 在 compile 时已拒绝，这里是防御
                    Backend::Cpu
                } else {
                    Backend::Sksl
                }
            }
            None if self.uses_cpu_only => Backend::Cpu,
            None => Backend::Sksl,
        }
    }

    /// uniforms 打包宽度（f32 个数），spec 声明序。
    pub fn uniform_f32_len(&self) -> usize {
        self.uniforms
            .iter()
            .map(|u| u.ty.vec_len().unwrap_or(1))
            .sum()
    }

    pub fn uniform_ty(&self, idx: u32) -> Ty {
        self.uniforms[idx as usize].ty
    }

    pub fn local_ty(&self, slot: u32) -> Ty {
        self.local_tys[slot as usize]
    }

    pub fn expr_ty(&self, expr: &Expr) -> Ty {
        match expr {
            Expr::LitF(_) => Ty::Float,
            Expr::LitB(_) => Ty::Bool,
            Expr::Local(s) => self.local_ty(*s),
            Expr::Uniform(u) => self.uniform_ty(*u),
            Expr::Uv => Ty::Vec2,
            Expr::Rect => Ty::Vec4,
            Expr::Child(_) => Ty::Float, // 不可作为值出现，仅编译期占位
            Expr::Bin(_, _, _, ty) => *ty,
            Expr::Un(_, _, ty) => *ty,
            Expr::Cond(_, _, _, ty) => *ty,
            Expr::Call { ty, .. } => *ty,
            Expr::Swizzle { ty, .. } => *ty,
            Expr::VecCons(_, ty) => *ty,
            Expr::Eval { .. } => Ty::Vec4,
            Expr::ScanGet { .. } => Ty::Vec4,
        }
    }
}
