//! 用户函数的签名推断：候选集掩码 + 单调不动点。
//!
//! 为什么不能只靠前向推断：参考 shader 里
//!
//! ```text
//! const noise = (p) => { const i = floor(p); … hash(i) … };
//! ```
//!
//! `p` 的类型只能由 `hash(i)` 反推（`hash` 的参数被 `dot(p, [127.1, 311.7])`
//! 钉成 float2 → `i: float2` → `i = floor(p)` → `p: float2`）。这个约束**逆流
//! 穿过已声明的局部 `i`**，而 `parse.rs` 的构建器是严格前向的（`local_slot`
//! 在声明处就冻结类型）。所以签名必须由一个独立的不动点求出。
//!
//! 用**候选集掩码**而不是合一（unification）：`Ty` 只有 5 个值，候选集就是
//! 一个 `u8`。掩码只收缩不增长，迭代必然终止。
//!
//! 掩码上的每条规则都**枚举候选后调用 `parse.rs` 的真规则**（[`bin_ty_pure`]、
//! [`swizzle_comps`]、`stdlib::check_signature`）——类型规则只有一份，不存在
//! 两份实现漂移的问题。这也让本层天然**过度宽松**：只要有一条候选路径成立，
//! 掩码就不收缩。具体报错留给构建器（它有全部源码位置）。
//!
//! 本层唯一主动报的错是「签名无法唯一确定」与「调用图有环（递归）」——前者
//! 构建器无从下手（它没有候选集），后者在构建器里会表现为难懂的"未声明"。

use std::collections::HashMap;

use oxc_ast::ast::{Expression, Statement};

use super::error::{LambdaError, LambdaResult};
use super::parse::{bin_ty_pure, swizzle_comps, vec_ty};
use super::program::{BinOp, BuiltinId, Ty};
use super::stdlib;

/// 全部 5 个类型，位序固定。
const ALL_TYS: [Ty; 5] = [Ty::Float, Ty::Bool, Ty::Vec2, Ty::Vec3, Ty::Vec4];

/// 顶层预扫出的一个函数声明。
pub(crate) struct FnDecl<'a> {
    pub name: String,
    /// 名字的源码 span（报错指向声明处）。
    pub name_span: (u32, u32),
    pub params: Vec<String>,
    /// 与 `params` 一一对应的源码 span（重复声明报错指向具体形参）。
    pub param_spans: Vec<(u32, u32)>,
    pub body: FnBody<'a>,
}

pub(crate) enum FnBody<'a> {
    Block(&'a [Statement<'a>]),
    /// 简洁体 `(p) => expr`。
    Expr(&'a Expression<'a>),
}

#[derive(Clone, Debug)]
pub(crate) struct FnSig {
    pub params: Vec<Ty>,
    pub ret: Ty,
}

/// 候选类型集：bit i = [`ALL_TYS`][i] 仍可能。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Mask(u8);

impl Mask {
    const EMPTY: Self = Self(0);

    fn of(ty: Ty) -> Self {
        let i = ALL_TYS.iter().position(|t| *t == ty).expect("Ty 在 ALL_TYS 内");
        Self(1 << i)
    }

    /// 数值类型（无 Bool）。
    fn num() -> Self {
        Self::of(Ty::Float) | Self::of(Ty::Vec2) | Self::of(Ty::Vec3) | Self::of(Ty::Vec4)
    }

    /// 全部 5 个类型——约束未知时的起点。
    fn all() -> Self {
        Self(0b11111)
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// 单元素时给出确定的类型。
    fn single(self) -> Option<Ty> {
        if self.0.count_ones() == 1 {
            ALL_TYS.into_iter().find(|t| Mask::of(*t) == self)
        } else {
            None
        }
    }

    /// 报错用的候选列表。
    fn candidates(self) -> String {
        let names: Vec<&str> = ALL_TYS
            .iter()
            .filter(|t| self.0 & Mask::of(**t).0 != 0)
            .map(|t| t.sksl_name())
            .collect();
        if names.is_empty() {
            "（无——存在互相矛盾的类型约束）".to_string()
        } else {
            names.join("/")
        }
    }

    fn iter(self) -> impl Iterator<Item = Ty> {
        ALL_TYS
            .into_iter()
            .enumerate()
            .filter(move |(i, _)| self.0 & (1 << i) != 0)
            .map(|(_, t)| t)
    }

    /// 二元运算的结果集：枚举两侧候选，交给 [`bin_ty_pure`]（唯一的算术规则）。
    fn bin(self, other: Self, op: BinOp) -> Self {
        let mut out = 0u8;
        for a in self.iter() {
            for b in other.iter() {
                if let Some(t) = bin_ty_pure(op, a, b) {
                    out |= Mask::of(t).0;
                }
            }
        }
        Self(out)
    }

    /// swizzle 读：基类型必须是够长的向量；用 [`swizzle_comps`]（唯一的合法性规则）。
    fn swizzle(self, prop: &str) -> Self {
        let mut out = 0u8;
        for base in self.iter() {
            let Some(n) = base.vec_len() else { continue };
            if swizzle_comps(prop, n).is_none() {
                continue;
            }
            let ty = if prop.len() == 1 { Ty::Float } else { vec_ty(prop.len()) };
            out |= Mask::of(ty).0;
        }
        Self(out)
    }

    /// 数组字面量 `[a, b, …]` 构造的向量类型（元素须标量数值，构建器校验）。
    fn vec_cons(n: usize) -> Self {
        if (2..=4).contains(&n) {
            Mask::of(vec_ty(n))
        } else {
            Mask::EMPTY
        }
    }
}

impl std::ops::BitOr for Mask {
    type Output = Self;
    fn bitor(self, o: Self) -> Self {
        Self(self.0 | o.0)
    }
}

impl std::ops::BitAnd for Mask {
    type Output = Self;
    fn bitand(self, o: Self) -> Self {
        Self(self.0 & o.0)
    }
}

/// 对每个函数体 + 主函数体做单调不动点，求出所有函数签名。
///
/// 本层对声明顺序与作用域**不敏感**（只看名字），所以主函数体里也能引用
/// 后面才声明的函数——「先声明后使用」由构建器按语句序强制。唯一例外是
/// 调用图成环（递归），那会让签名无从确定，本层直接报错。
pub(crate) fn infer_signatures<'a>(
    decls: &'a [FnDecl<'a>],
    main_stmts: &'a [Statement<'a>],
    source: &'a str,
    // `u.<name>` 的声明类型（spec 声明序）。
    uniforms: &'a [(String, Ty)],
    // 非 uniform 的 lambda 参数名 -> 类型（`uv`→float2、`rect`→float4、`get`→float4）。
    param_tys: &'a HashMap<String, Ty>,
) -> LambdaResult<Vec<FnSig>> {
    let by_name: HashMap<&str, usize> = decls
        .iter()
        .enumerate()
        .map(|(i, d)| (d.name.as_str(), i))
        .collect();

    let mut state = State {
        decls,
        by_name,
        uniforms,
        param_tys,
        params: decls.iter().map(|d| vec![Mask::all(); d.params.len()]).collect(),
        locals: vec![HashMap::new(); decls.len()],
        rets: vec![Mask::all(); decls.len()],
        edges: vec![Vec::new(); decls.len()],
        main_locals: HashMap::new(),
        cur: None,
        cur_params: Vec::new(),
        cur_locals: HashMap::new(),
    };

    // 单调不动点：掩码只收缩，界 = 5 × 绑定数，必然终止（着色器规模 2-3 轮）。
    let mut changed = true;
    while changed {
        changed = false;
        for (i, decl) in decls.iter().enumerate() {
            state.enter(i);
            match &decl.body {
                FnBody::Block(stmts) => state.walk_stmts(stmts, &mut changed),
                FnBody::Expr(e) => {
                    let m = state.walk_expr(e, &mut changed);
                    state.meet_ret(i, m, &mut changed);
                }
            }
            state.leave(i);
        }
        state.enter_main();
        state.walk_stmts(main_stmts, &mut changed);
        state.leave_main();
    }

    if let Some(v) = find_cycle(&state.edges) {
        return Err(LambdaError::at(
            format!(
                "函数 `{}` 参与递归调用：用户函数必须先声明后使用，调用图必须无环",
                decls[v].name
            ),
            decls[v].name_span,
            source,
        ));
    }

    // 收敛后逐个要求签名唯一确定。
    let mut out = Vec::with_capacity(decls.len());
    for (i, decl) in decls.iter().enumerate() {
        let mut params = Vec::with_capacity(decl.params.len());
        for (pi, name) in decl.params.iter().enumerate() {
            let m = state.params[i][pi];
            let Some(ty) = m.single() else {
                return Err(LambdaError::at(
                    format!(
                        "无法推断函数 `{}` 的参数 `{name}` 的类型：可能是 {}。\
                         请在函数体内约束它（如 `{name}.x`、`dot({name}, {name})`），\
                         或让某个调用点的实参类型明确。",
                        decl.name,
                        m.candidates()
                    ),
                    decl.name_span,
                    source,
                ));
            };
            params.push(ty);
        }
        let m = state.rets[i];
        let Some(ret) = m.single() else {
            let msg = if m.is_empty() {
                format!("函数 `{}` 缺少 return（用户函数必须有返回值）", decl.name)
            } else {
                format!(
                    "无法推断函数 `{}` 的返回类型：可能是 {}。请在函数体内约束返回值。",
                    decl.name,
                    m.candidates()
                )
            };
            return Err(LambdaError::at(msg, decl.name_span, source));
        };
        out.push(FnSig { params, ret });
    }
    Ok(out)
}

struct State<'a> {
    decls: &'a [FnDecl<'a>],
    by_name: HashMap<&'a str, usize>,
    uniforms: &'a [(String, Ty)],
    param_tys: &'a HashMap<String, Ty>,
    /// 每个函数的形参掩码。
    params: Vec<Vec<Mask>>,
    /// 每个函数的局部变量掩码（名字 → 掩码）。
    locals: Vec<HashMap<String, Mask>>,
    /// 每个函数的返回类型掩码。
    rets: Vec<Mask>,
    /// 调用图邻接表（去重），仅用于递归检测。
    edges: Vec<Vec<usize>>,
    /// 主函数体的局部变量掩码。
    main_locals: HashMap<String, Mask>,

    /// 当前遍历的函数（`None` = 主函数体）。
    cur: Option<usize>,
    cur_params: Vec<Mask>,
    cur_locals: HashMap<String, Mask>,
}

impl<'a> State<'a> {
    /// 进入函数 `i`，把它的持久状态搬进工作副本。
    ///
    /// 局部掩码必须**恢复**（`take`），不能清空：清空会让 `meet` 每轮都从
    /// `Mask::all()` 重新插入初值掩码，`changed` 永远为真，不动点不收敛
    /// （曾是第二个真实死循环）。掩码只收缩不增长，前提就是它跨轮次存活。
    fn enter(&mut self, i: usize) {
        self.cur = Some(i);
        self.cur_params = std::mem::take(&mut self.params[i]);
        self.cur_locals = std::mem::take(&mut self.locals[i]);
    }

    fn leave(&mut self, i: usize) {
        self.params[i] = std::mem::take(&mut self.cur_params);
        self.locals[i] = std::mem::take(&mut self.cur_locals);
    }

    fn enter_main(&mut self) {
        self.cur = None;
        self.cur_locals = std::mem::take(&mut self.main_locals);
    }

    fn leave_main(&mut self) {
        self.main_locals = std::mem::take(&mut self.cur_locals);
    }

    /// 查一个标识符当前的掩码。形参优先于局部（同一段槽区间内不可能重名）。
    ///
    /// 主函数体与函数体统一走 `cur_locals`：`cur` 只在区分「有无形参」时才用。
    fn lookup(&self, name: &str) -> Mask {
        if let Some(i) = self.cur {
            if let Some(p) = self.decls[i].params.iter().position(|n| n == name) {
                return self.cur_params[p];
            }
        } else if let Some(ty) = self.param_tys.get(name) {
            // `uv` → float2、`rect` → float4。函数体看不到它们（构建器拒绝捕获），
            // 所以只在主函数体里查。
            return Mask::of(*ty);
        }
        self.cur_locals.get(name).copied().unwrap_or(Mask::all())
    }

    /// 收缩一个绑定。掩码只会变小，所以收缩即单调。
    fn meet(&mut self, name: &str, m: Mask, changed: &mut bool) {
        let target = if let Some(i) = self.cur {
            match self.decls[i].params.iter().position(|n| n == name) {
                Some(p) => &mut self.cur_params[p],
                None => self.cur_locals.entry(name.to_string()).or_insert(Mask::all()),
            }
        } else {
            self.cur_locals.entry(name.to_string()).or_insert(Mask::all())
        };
        let after = *target & m;
        if after != *target {
            *target = after;
            *changed = true;
        }
    }

    /// 记录一个局部的掩码（`let`/`const` 的初值类型）。
    ///
    /// 必须**取交**而不是覆盖：后续的赋值会经 [`State::meet`] 收缩同一个掩码，
    /// 若这里覆盖回初值掩码，下一轮又会被收缩回去，`changed` 永远为真，不动点
    /// 不收敛（曾是一个真实死循环）。取交则只收缩不增长，与 `meet` 同构。
    fn define(&mut self, name: &str, m: Mask, changed: &mut bool) {
        self.meet(name, m, changed);
    }

    fn meet_ret(&mut self, i: usize, m: Mask, changed: &mut bool) {
        let after = self.rets[i] & m;
        if after != self.rets[i] {
            self.rets[i] = after;
            *changed = true;
        }
    }

    fn walk_stmts(&mut self, stmts: &[Statement], changed: &mut bool) {
        for stmt in stmts {
            self.walk_stmt(stmt, changed);
        }
    }

    fn walk_stmt(&mut self, stmt: &Statement, changed: &mut bool) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                for d in &decl.declarations {
                    let Some(init) = &d.init else { continue };
                    let m = self.walk_expr(init, changed);
                    if let Some(name) = binding_name(&d.id) {
                        self.define(name, m, changed);
                    }
                }
            }
            Statement::ExpressionStatement(es) => {
                self.walk_expr(&es.expression, changed);
            }
            Statement::ReturnStatement(ret) => {
                let m = match &ret.argument {
                    Some(arg) => self.walk_expr(arg, changed),
                    None => Mask::EMPTY,
                };
                if let Some(i) = self.cur {
                    self.meet_ret(i, m, changed);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.walk_expr(&if_stmt.test, changed);
                self.walk_stmt(&if_stmt.consequent, changed);
                if let Some(alt) = &if_stmt.alternate {
                    self.walk_stmt(alt, changed);
                }
            }
            Statement::ForStatement(f) => {
                // `for (let i = …)`：计数变量恒为 Float（构建器强制）。
                if let Some(oxc_ast::ast::ForStatementInit::VariableDeclaration(vd)) = &f.init
                    && let Some(name) = vd.declarations.first().and_then(|d| binding_name(&d.id))
                {
                    self.define(name, Mask::of(Ty::Float), changed);
                }
                if let Some(test) = &f.test {
                    self.walk_expr(test, changed);
                }
                if let Some(update) = &f.update {
                    self.walk_expr(update, changed);
                }
                self.walk_stmt(&f.body, changed);
            }
            Statement::BlockStatement(b) => self.walk_stmts(&b.body, changed),
            _ => {}
        }
    }

    /// 走一个表达式，返回它的类型掩码。
    fn walk_expr(&mut self, expr: &Expression, changed: &mut bool) -> Mask {
        match expr {
            Expression::NumericLiteral(_) => Mask::of(Ty::Float),
            Expression::BooleanLiteral(_) => Mask::of(Ty::Bool),
            Expression::Identifier(id) => self.lookup(id.name.as_str()),
            Expression::BinaryExpression(bin) => {
                let l = self.walk_expr(&bin.left, changed);
                let r = self.walk_expr(&bin.right, changed);
                let Some(op) = bin_op(bin.operator) else {
                    // 位运算等：构建器报错，这里给个中性结果。
                    return Mask::all();
                };
                l.bin(r, op)
            }
            Expression::LogicalExpression(log) => {
                self.walk_expr(&log.left, changed);
                self.walk_expr(&log.right, changed);
                Mask::of(Ty::Bool)
            }
            Expression::UnaryExpression(un) => {
                let a = self.walk_expr(&un.argument, changed);
                match un.operator {
                    oxc_ast::ast::UnaryOperator::LogicalNot => Mask::of(Ty::Bool),
                    // 取负要求数值，其余一元运算构建器拒绝。
                    _ => a & Mask::num(),
                }
            }
            Expression::ConditionalExpression(c) => {
                self.walk_expr(&c.test, changed);
                let t = self.walk_expr(&c.consequent, changed);
                let f = self.walk_expr(&c.alternate, changed);
                t & f & (Mask::num() | Mask::of(Ty::Bool))
            }
            Expression::ParenthesizedExpression(p) => self.walk_expr(&p.expression, changed),
            Expression::ArrayExpression(arr) => {
                for el in &arr.elements {
                    if let Some(e) = el.as_expression() {
                        self.walk_expr(e, changed);
                    }
                }
                Mask::vec_cons(arr.elements.len())
            }
            Expression::StaticMemberExpression(member) => {
                let prop = member.property.name.as_str();
                // `u.<name>`：类型取 spec 声明（未声明则由构建器报错，这里给中性值）。
                if let Expression::Identifier(obj) = &member.object
                    && obj.name == "u"
                {
                    return self
                        .uniforms
                        .iter()
                        .find(|(n, _)| n == prop)
                        .map(|(_, ty)| Mask::of(*ty))
                        .unwrap_or(Mask::all());
                }
                let base = self.walk_expr(&member.object, changed);
                base.swizzle(prop)
            }
            Expression::CallExpression(call) => self.walk_call(call, changed),
            Expression::AssignmentExpression(assign) => {
                let r = self.walk_expr(&assign.right, changed);
                if let Some(name) = assign_target_name(&assign.left) {
                    self.meet(name, r, changed);
                }
                r
            }
            Expression::UpdateExpression(_) => Mask::of(Ty::Float),
            _ => Mask::all(),
        }
    }

    fn walk_call(&mut self, call: &oxc_ast::ast::CallExpression, changed: &mut bool) -> Mask {
        // 实参先走一遍——它们的类型会反向约束被调函数的形参。
        let arg_masks: Vec<Mask> = call
            .arguments
            .iter()
            .filter_map(|a| a.as_expression())
            .map(|a| self.walk_expr(a, changed))
            .collect();

        // 用户函数
        if let Expression::Identifier(id) = &call.callee
            && let Some(&fi) = self.by_name.get(id.name.as_str())
        {
            if let Some(cur) = self.cur
                && !self.edges[cur].contains(&fi)
            {
                self.edges[cur].push(fi);
            }
            for (k, arg) in arg_masks.iter().enumerate() {
                if k >= self.params[fi].len() {
                    break;
                }
                let after = self.params[fi][k] & *arg;
                if after != self.params[fi][k] {
                    self.params[fi][k] = after;
                    *changed = true;
                }
            }
            return self.rets[fi];
        }

        // `child.eval(pos)` / `get(dx, dy)` 恒为 float4。
        if let Expression::StaticMemberExpression(m) = &call.callee
            && m.property.name == "eval"
        {
            return Mask::of(Ty::Vec4);
        }
        if let Expression::Identifier(id) = &call.callee
            && id.name == "get"
        {
            return Mask::of(Ty::Vec4);
        }

        // 内建：把实参候选做笛卡尔积，用唯一的签名规则求返回类型。
        let id = match &call.callee {
            Expression::Identifier(id) => stdlib::lookup(id.name.as_str()).map(|(b, _)| b),
            Expression::StaticMemberExpression(m) => {
                if let Expression::Identifier(obj) = &m.object
                    && obj.name == "Math"
                {
                    stdlib::math_member_id(m.property.name.as_str())
                } else {
                    None
                }
            }
            _ => None,
        };
        match id {
            Some(id) => builtin_ret(id, &arg_masks),
            None => Mask::all(),
        }
    }
}

/// 内建返回类型掩码：实参候选的笛卡尔积（≤ 5^4）逐组过 [`stdlib::check_signature`]。
fn builtin_ret(id: BuiltinId, arg_masks: &[Mask]) -> Mask {
    let lists: Vec<Vec<Ty>> = arg_masks.iter().map(|m| m.iter().collect()).collect();
    let mut out = 0u8;
    let mut cur = Vec::with_capacity(lists.len());
    cartesian(id, &lists, 0, &mut cur, &mut out);
    Mask(out)
}

fn cartesian(id: BuiltinId, lists: &[Vec<Ty>], idx: usize, cur: &mut Vec<Ty>, out: &mut u8) {
    if idx == lists.len() {
        let mut sink = |_: String| {};
        if let Some(ret) = stdlib::check_signature(id, cur, &mut sink) {
            *out |= Mask::of(ret).0;
        }
        return;
    }
    for t in &lists[idx] {
        cur.push(*t);
        cartesian(id, lists, idx + 1, cur, out);
        cur.pop();
    }
}

/// 调用图找环（DFS 三色）。返回环上任一节点。
fn find_cycle(edges: &[Vec<usize>]) -> Option<usize> {
    fn dfs(v: usize, edges: &[Vec<usize>], color: &mut [u8]) -> Option<usize> {
        color[v] = 1;
        for &w in &edges[v] {
            if color[w] == 1 {
                return Some(w);
            }
            if color[w] == 0
                && let Some(c) = dfs(w, edges, color)
            {
                return Some(c);
            }
        }
        color[v] = 2;
        None
    }
    let mut color = vec![0u8; edges.len()];
    for v in 0..edges.len() {
        if color[v] == 0
            && let Some(c) = dfs(v, edges, &mut color)
        {
            return Some(c);
        }
    }
    None
}

fn bin_op(op: oxc_ast::ast::BinaryOperator) -> Option<BinOp> {
    use oxc_ast::ast::BinaryOperator as O;
    Some(match op {
        O::Addition => BinOp::Add,
        O::Subtraction => BinOp::Sub,
        O::Multiplication => BinOp::Mul,
        O::Division => BinOp::Div,
        O::Remainder => BinOp::Rem,
        O::GreaterThan => BinOp::Gt,
        O::LessThan => BinOp::Lt,
        O::GreaterEqualThan => BinOp::Ge,
        O::LessEqualThan => BinOp::Le,
        O::Equality | O::StrictEquality => BinOp::Eq,
        O::Inequality | O::StrictInequality => BinOp::Ne,
        _ => return None,
    })
}

fn binding_name<'a>(p: &'a oxc_ast::ast::BindingPattern<'a>) -> Option<&'a str> {
    match p {
        oxc_ast::ast::BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

fn assign_target_name<'a>(t: &'a oxc_ast::ast::AssignmentTarget<'a>) -> Option<&'a str> {
    match t {
        oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}
