//! Lambda 可调用的内建函数表。每个 op 带能力标记：`Sksl` 表示可 lowering
//! 到 SKSL；`CpuOnly` 表示只在解释器后端可用（整数精确语义，如溶解的
//! `h01` 哈希）——用到即整程序派发 CPU 后端。

use super::program::Ty;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Sksl,
    CpuOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinId {
    Abs,
    Sign,
    Floor,
    Ceil,
    Fract,
    Min,
    Max,
    Clamp,
    Pow,
    Sqrt,
    Exp,
    Exp2,
    Log,
    Log2,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Mix,
    Smoothstep,
    Step,
    Length,
    Distance,
    Dot,
    Cross,
    Normalize,
    /// byte(x)：JS `x|0` 截断语义（正数域），输出整数浮点。
    Byte,
    /// u32(x)：JS ToUint32 截断（正数域），结果以 f64 表示。仅 CPU 后端。
    CastU,
    /// i32(x)：JS ToInt32 截断，结果以 f64 表示。仅 CPU 后端。
    CastI,
    /// float(x)：转浮点（恒等）。
    CastF,
    /// imul(a, b)：Math.imul 语义（32 位 wrapping 乘的低 32 位，f64 进出）。
    /// 仅 CPU 后端。
    Imul,
    /// h01(a, b, c)：确定性像素哈希（参考 index.html `h01`），参数按 JS
    /// ToUint32 截断，返回 [0,1) 浮点。仅 CPU 后端。
    H01,
}

pub struct Builtin {
    pub name: &'static str,
    pub capability: Capability,
}

pub const BUILTINS: &[(&str, BuiltinId, Capability)] = &[
    ("abs", BuiltinId::Abs, Capability::Sksl),
    ("sign", BuiltinId::Sign, Capability::Sksl),
    ("floor", BuiltinId::Floor, Capability::Sksl),
    ("ceil", BuiltinId::Ceil, Capability::Sksl),
    ("fract", BuiltinId::Fract, Capability::Sksl),
    ("min", BuiltinId::Min, Capability::Sksl),
    ("max", BuiltinId::Max, Capability::Sksl),
    ("clamp", BuiltinId::Clamp, Capability::Sksl),
    ("pow", BuiltinId::Pow, Capability::Sksl),
    ("sqrt", BuiltinId::Sqrt, Capability::Sksl),
    ("exp", BuiltinId::Exp, Capability::Sksl),
    ("exp2", BuiltinId::Exp2, Capability::Sksl),
    ("log", BuiltinId::Log, Capability::Sksl),
    ("log2", BuiltinId::Log2, Capability::Sksl),
    ("sin", BuiltinId::Sin, Capability::Sksl),
    ("cos", BuiltinId::Cos, Capability::Sksl),
    ("tan", BuiltinId::Tan, Capability::Sksl),
    ("asin", BuiltinId::Asin, Capability::Sksl),
    ("acos", BuiltinId::Acos, Capability::Sksl),
    ("atan", BuiltinId::Atan, Capability::Sksl),
    ("atan2", BuiltinId::Atan2, Capability::Sksl),
    ("mix", BuiltinId::Mix, Capability::Sksl),
    ("smoothstep", BuiltinId::Smoothstep, Capability::Sksl),
    ("step", BuiltinId::Step, Capability::Sksl),
    ("length", BuiltinId::Length, Capability::Sksl),
    ("distance", BuiltinId::Distance, Capability::Sksl),
    ("dot", BuiltinId::Dot, Capability::Sksl),
    ("cross", BuiltinId::Cross, Capability::Sksl),
    ("normalize", BuiltinId::Normalize, Capability::Sksl),
    ("byte", BuiltinId::Byte, Capability::Sksl),
    ("u32", BuiltinId::CastU, Capability::CpuOnly),
    ("i32", BuiltinId::CastI, Capability::CpuOnly),
    ("float", BuiltinId::CastF, Capability::Sksl),
    ("imul", BuiltinId::Imul, Capability::CpuOnly),
    ("h01", BuiltinId::H01, Capability::CpuOnly),
];

/// `Math.sin` 等 Math 命名空间成员 -> 内建 id；不在表内即白名单拒绝。
pub fn math_member_id(name: &str) -> Option<BuiltinId> {
    let id = match name {
        "abs" => BuiltinId::Abs,
        "sign" => BuiltinId::Sign,
        "floor" => BuiltinId::Floor,
        "ceil" => BuiltinId::Ceil,
        "sqrt" => BuiltinId::Sqrt,
        "pow" => BuiltinId::Pow,
        "exp" => BuiltinId::Exp,
        "log" => BuiltinId::Log,
        "sin" => BuiltinId::Sin,
        "cos" => BuiltinId::Cos,
        "tan" => BuiltinId::Tan,
        "asin" => BuiltinId::Asin,
        "acos" => BuiltinId::Acos,
        "atan" => BuiltinId::Atan,
        "atan2" => BuiltinId::Atan2,
        "min" => BuiltinId::Min,
        "max" => BuiltinId::Max,
        "random" => return None, // 拒绝：破坏确定性
        _ => return None,
    };
    Some(id)
}

pub fn lookup(name: &str) -> Option<(BuiltinId, Capability)> {
    BUILTINS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, id, cap)| (*id, *cap))
}

pub fn name_of(id: BuiltinId) -> &'static str {
    BUILTINS
        .iter()
        .find(|(_, bid, _)| *bid == id)
        .map(|(n, _, _)| *n)
        .unwrap_or("?")
}

pub fn capability_of(id: BuiltinId) -> Capability {
    BUILTINS
        .iter()
        .find(|(_, bid, _)| *bid == id)
        .map(|(_, _, c)| *c)
        .unwrap_or(Capability::Sksl)
}

/// 内建函数签名检查：`args` 已推断的类型进、返回类型出；不匹配给可读错误。
pub fn check_signature(
    id: BuiltinId,
    args: &[Ty],
    err: &mut dyn FnMut(String),
) -> Option<Ty> {
    use Ty::*;
    /// 一组数值参数：全标量 -> Float 域；全同长向量 -> 该向量；标量与向量混
    /// 出现时只允许向量 + 标量（广播）。返回统一后的"主体类型"。
    fn unify_num(args: &[Ty], what: &str, err: &mut dyn FnMut(String)) -> Option<Ty> {
        let mut vec_len: Option<usize> = None;
        for (i, ty) in args.iter().enumerate() {
            if !ty.is_scalar_num() && ty.vec_len().is_none() {
                err(format!("{what}: 参数 {i} 需要数值类型，得到 {ty:?}"));
                return None;
            }
            if let Some(n) = ty.vec_len() {
                match vec_len {
                    None => vec_len = Some(n),
                    Some(m) if m != n => {
                        err(format!("{what}: 向量维度不一致（{m} vs {n}）"));
                        return None;
                    }
                    _ => {}
                }
            }
        }
        Some(match vec_len {
            None => Float,
            Some(2) => Vec2,
            Some(3) => Vec3,
            Some(4) => Vec4,
            _ => Float,
        })
    }
    match id {
        BuiltinId::Abs | BuiltinId::Sign | BuiltinId::Floor | BuiltinId::Ceil
        | BuiltinId::Fract | BuiltinId::Sqrt | BuiltinId::Exp | BuiltinId::Exp2
        | BuiltinId::Log | BuiltinId::Log2 | BuiltinId::Sin | BuiltinId::Cos
        | BuiltinId::Tan | BuiltinId::Asin | BuiltinId::Acos | BuiltinId::Atan
        | BuiltinId::Normalize => {
            if args.len() != 1 {
                err(format!("{}: 期望 1 个参数，得到 {}", name_of(id), args.len()));
                return None;
            }
            unify_num(args, name_of(id), err)
        }
        BuiltinId::Pow | BuiltinId::Atan2 | BuiltinId::Step | BuiltinId::Min
        | BuiltinId::Max => {
            if args.len() != 2 {
                err(format!("{}: 期望 2 个参数，得到 {}", name_of(id), args.len()));
                return None;
            }
            unify_num(args, name_of(id), err)
        }
        // dot/distance 始终返回标量
        BuiltinId::Distance | BuiltinId::Dot => {
            if args.len() != 2 {
                err(format!("{}: 期望 2 个参数，得到 {}", name_of(id), args.len()));
                return None;
            }
            unify_num(args, name_of(id), err).map(|_| Float)
        }
        BuiltinId::Mix => {
            if args.len() != 3 {
                err("mix: 期望 3 个参数".into());
                return None;
            }
            unify_num(args, "mix", err)
        }
        BuiltinId::Smoothstep | BuiltinId::Clamp => {
            if args.len() != 3 {
                err(format!("{}: 期望 3 个参数，得到 {}", name_of(id), args.len()));
                return None;
            }
            unify_num(args, name_of(id), err)
        }
        BuiltinId::Cross => {
            if args.len() == 2 && args.iter().all(|t| *t == Vec3) {
                Some(Vec3)
            } else {
                err("cross: 期望两个 vec3".into());
                None
            }
        }
        BuiltinId::Length => {
            if args.len() == 1 && (args[0].vec_len().is_some() || args[0] == Float) {
                Some(Float)
            } else {
                err("length: 期望 1 个数值或向量".into());
                None
            }
        }
        BuiltinId::Byte => {
            if args.len() == 1 && args[0].is_scalar_num() {
                Some(Float)
            } else {
                err("byte: 期望 1 个标量数值".into());
                None
            }
        }
        BuiltinId::CastU => {
            if args.len() == 1 && args[0].is_scalar_num() {
                Some(Float)
            } else {
                err("u32: 期望 1 个标量数值".into());
                None
            }
        }
        BuiltinId::CastI => {
            if args.len() == 1 && args[0].is_scalar_num() {
                Some(Float)
            } else {
                err("i32: 期望 1 个标量数值".into());
                None
            }
        }
        BuiltinId::CastF => {
            if args.len() == 1 && args[0].is_scalar_num() {
                Some(Float)
            } else {
                err("float: 期望 1 个标量数值".into());
                None
            }
        }
        BuiltinId::Imul => {
            if args.len() == 2 && args.iter().all(|t| t.is_scalar_num()) {
                Some(Float)
            } else {
                err("imul: 期望两个标量数值".into());
                None
            }
        }
        BuiltinId::H01 => {
            if args.len() == 3 && args.iter().all(|t| t.is_scalar_num()) {
                Some(Float)
            } else {
                err("h01: 期望三个标量数值".into());
                None
            }
        }
    }
}
