//! AST -> SKSL codegen。仅 [`Backend::Sksl`] 程序可达此路径。
//!
//! 约定：
//! - uniform 按 spec 声明序发射 `uniform float u_<name>;`，f32 native-endian
//!   连续打包（与 JS 侧 flat 数组一致）；末尾固定追加
//!   `u_oc_rect`（4 个标量 uniform，binding 从 drawRect dst 填充）。
//! - child 参数发射 `uniform shader u_oc_c<N>;`，`child.eval(p)` → `u_oc_c<N>.eval(p)`。
//!   child image 的 tile 空间原点在本地 (0,0)，与 `uv`（rect 本地像素坐标）
//!   天然对齐：`eval(floor(uv))` 即取该像素。
//! - lambda 返回 **straight（unpremul）** 色；SKSL 输出语义为 premul，
//!   codegen 在 return 处自动预乘，保证与解释器（straight RGBA）合成一致。
//! - `uv` 恒为 rect 本地坐标，像素中心取 `i + 0.5`（与 SKSL xy 语义一致）。

use super::error::{LambdaError, LambdaResult};
use super::program::{Backend, BinOp, Expr, Param, Program, Stmt, Ty, UnOp};

pub fn lower(program: &Program) -> LambdaResult<String> {
    debug_assert_eq!(program.backend(), Backend::Sksl);
    let mut out = String::with_capacity(1024);
    // 向量 uniform 拆成标量分量声明：Skia 的 uniform 布局有对齐规则
    // （vec2→8B、vec3/vec4→16B），连续 f32 打包会错位；全标量声明让
    // wire 上的 flat 数组天然对齐，JS 侧无需知道打包规则。
    for uniform in &program.uniforms {
        match uniform.ty.vec_len() {
            None => out.push_str(&format!("uniform float u_{};\n", uniform.name)),
            Some(n) => {
                for i in 0..n {
                    out.push_str(&format!("uniform float u_{}_{};\n", uniform.name, i));
                }
            }
        }
    }
    // u_oc_rect 同样拆标量（float4 会触发 Skia 的 16B 对齐，把 blob 顶错位），
    // 再在 main 里重组；wire 上仍是末尾 4 个 f32。
    for i in 0..4 {
        out.push_str(&format!("uniform float u_oc_rect_{i};\n"));
    }
    for (index, _) in program.children.iter().enumerate() {
        out.push_str(&format!("uniform shader u_oc_c{index};\n"));
    }
    out.push_str("half4 main(float2 xy) {\n");
    out.push_str("    float4 u_oc_rect = float4(u_oc_rect_0, u_oc_rect_1, u_oc_rect_2, u_oc_rect_3);\n");
    out.push_str("    float2 oc_uv = xy - u_oc_rect.xy;\n");
    for stmt in &program.body {
        emit_stmt(program, stmt, 1, &mut out)?;
    }
    out.push_str("    return half4(0.0);\n}\n");
    Ok(out)
}

fn indent(n: usize) -> String {
    "    ".repeat(n)
}

fn emit_stmt(program: &Program, stmt: &Stmt, depth: usize, out: &mut String) -> LambdaResult<()> {
    match stmt {
        Stmt::Let { slot, value } => {
            let ty = program.local_ty(*slot);
            out.push_str(&format!(
                "{}{} oc_l{} = {};\n",
                indent(depth),
                ty.sksl_name(),
                slot,
                emit_expr(program, value)?
            ));
        }
        Stmt::Assign { slot, value } => {
            out.push_str(&format!(
                "{}oc_l{} = {};\n",
                indent(depth),
                slot,
                emit_expr(program, value)?
            ));
        }
        Stmt::If { cond, then, els } => {
            out.push_str(&format!("{}if ({}) {{\n", indent(depth), emit_expr(program, cond)?));
            for s in then {
                emit_stmt(program, s, depth + 1, out)?;
            }
            if els.is_empty() {
                out.push_str(&format!("{}}}\n", indent(depth)));
            } else {
                out.push_str(&format!("{}}} else {{\n", indent(depth)));
                for s in els {
                    emit_stmt(program, s, depth + 1, out)?;
                }
                out.push_str(&format!("{}}}\n", indent(depth)));
            }
        }
        Stmt::Return(expr) => {
            // 返回 straight 色；SKSL 输出 premul，自动预乘。先落入临时变量，
            // 避免对复合表达式直接 swizzle。
            let value = emit_expr(program, expr)?;
            let premul = match program.expr_ty(expr) {
                Ty::Vec4 => "half4(oc_r.rgb * oc_r.a, oc_r.a)".to_string(),
                Ty::Vec3 => "half4(oc_r, 1.0)".to_string(),
                Ty::Float => "half4(oc_r, oc_r, oc_r, 1.0)".to_string(),
                other => {
                    return Err(LambdaError::msg(format!(
                        "return 类型不受支持：{other:?}（仅 float/vec3/vec4）"
                    )))
                }
            };
            let sksl_ty = match program.expr_ty(expr) {
                Ty::Float => "float",
                _ => "float4",
            };
            out.push_str(&format!(
                "{}{sksl_ty} oc_r = {value};\n{}return {premul};\n",
                indent(depth),
                indent(depth)
            ));
        }
    }
    Ok(())
}

fn emit_expr(program: &Program, expr: &Expr) -> LambdaResult<String> {
    match expr {
        Expr::LitF(v) => {
            if v.is_finite() {
                // Debug 格式保证带小数点（2.0 而非 2），SKSL float 字面量安全
                Ok(format!("{v:?}"))
            } else {
                Err(LambdaError::msg("浮点字面量必须是有限数"))
            }
        }
        Expr::LitB(b) => Ok(if *b { "true".into() } else { "false".into() }),
        Expr::Local(slot) => Ok(format!("oc_l{slot}")),
        Expr::Uniform(idx) => {
            let u = &program.uniforms[*idx as usize];
            match u.ty.vec_len() {
                None => Ok(format!("u_{}", u.name)),
                Some(n) => {
                    let comps: Vec<String> =
                        (0..n).map(|i| format!("u_{}_{}", u.name, i)).collect();
                    Ok(format!("float{}({})", n, comps.join(", ")))
                }
            }
        }
        Expr::Uv => Ok("oc_uv".into()),
        Expr::Rect => Ok("u_oc_rect".into()),
        Expr::Child(idx) => Err(LambdaError::msg(format!(
            "child `{}` 不能作为值使用（只能 .eval(pos)）",
            program.children[*idx]
        ))),
        // 编译期已拒绝 scan 类强制 SKSL（compile_uncached），这里仅穷尽匹配
        Expr::ScanGet { .. } => Err(LambdaError::msg(
            "scan 类 lambda 没有 SKSL 形态（顺序扫描仅 CPU 解释器）",
        )),
        Expr::Bin(op, l, r, _) => {
            let (ls, rs) = (emit_expr(program, l)?, emit_expr(program, r)?);
            Ok(match op {
                BinOp::Add => format!("({ls} + {rs})"),
                BinOp::Sub => format!("({ls} - {rs})"),
                BinOp::Mul => format!("({ls} * {rs})"),
                BinOp::Div => format!("({ls} / {rs})"),
                // JS % 是截断余数；SKSL mod() 是 floor 余数，必须展开成语义式
                BinOp::Rem => format!("({ls} - {rs} * trunc({ls} / {rs}))"),
                BinOp::Gt => format!("({ls} > {rs})"),
                BinOp::Lt => format!("({ls} < {rs})"),
                BinOp::Ge => format!("({ls} >= {rs})"),
                BinOp::Le => format!("({ls} <= {rs})"),
                BinOp::Eq => format!("({ls} == {rs})"),
                BinOp::Ne => format!("({ls} != {rs})"),
                BinOp::And => format!("({ls} && {rs})"),
                BinOp::Or => format!("({ls} || {rs})"),
            })
        }
        Expr::Un(op, arg, _) => {
            let a = emit_expr(program, arg)?;
            Ok(match op {
                UnOp::Neg => format!("(-{a})"),
                UnOp::Not => format!("(!{a})"),
            })
        }
        Expr::Cond(c, t, f, _) => Ok(format!(
            "({} ? {} : {})",
            emit_expr(program, c)?,
            emit_expr(program, t)?,
            emit_expr(program, f)?
        )),
        Expr::Call { id, args, .. } => {
            if super::stdlib::capability_of(*id) == super::stdlib::Capability::CpuOnly {
                return Err(LambdaError::msg(format!(
                    "内建 `{}` 仅 CPU 后端可用，不应到达 SKSL codegen",
                    super::stdlib::name_of(*id)
                )));
            }
            let emitted: Result<Vec<_>, _> = args.iter().map(|e| emit_expr(program, e)).collect();
            let emitted = emitted?;
            // byte() 不是 SKSL 内建：JS `x|0` 截断语义 → trunc(x)（向零取整）
            let name = match *id {
                super::stdlib::BuiltinId::Byte => "trunc".to_string(),
                super::stdlib::BuiltinId::CastF => "float".to_string(),
                _ => super::stdlib::name_of(*id).to_string(),
            };
            Ok(format!("{}({})", name, emitted.join(", ")))
        }
        Expr::Swizzle { base, comps, len, .. } => {
            let b = emit_expr(program, base)?;
            let names = ["x", "y", "z", "w"];
            let sw: String = comps[..*len as usize].iter().map(|c| names[*c as usize]).collect();
            Ok(format!("{b}.{sw}"))
        }
        Expr::VecCons(items, _) => {
            let parts: Result<Vec<_>, _> = items.iter().map(|e| emit_expr(program, e)).collect();
            Ok(format!("float{}({})", items.len(), parts?.join(", ")))
        }
        Expr::Eval { child, pos } => {
            Ok(format!("u_oc_c{}.eval({})", child, emit_expr(program, pos)?))
        }
    }
}

/// `rect` 参数是否被使用（binding 据此决定是否需要填充；v1 恒填充，防御性保留）。
pub fn uses_rect(program: &Program) -> bool {
    program.params.iter().any(|(_, p)| matches!(p, Param::Rect))
}
