//! @omp 循环子集解析器：标注循环体与页面函数源码 → IR。
//!
//! 只收「可加速」面——数学表达式、局部声明、if/else、同形 for 头
//! （`let v = e; v < bound; v += step`，step 亦收 `++`/`--`/`-=`）、
//! 数组字面量、模板串、`Math.*` 白名单调用。任何超面构造
//! （while/break/对象字面量/箭头函数/new/…）解析即失败——上层把
//! 失败翻译成降级（原循环照跑），与「pragma 未被编译器处理」同语义。

// ── IR ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    LtE,
    Gt,
    GtE,
    /// `==`/`===` 统一入此（VM 按值+类型比较）。
    Eq,
    Neq,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    ShrS,
    ShrU,
    /// 不由源码解析产生——编译器把 `Math.imul(a, b)` 折进来的内部算子。
    Imul,
}

#[derive(Clone, Debug)]
pub enum TplPart {
    Lit(String),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Undef,
    Ident(String),
    Member(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `&&` / `||` 短路（is_and）。
    Logical(bool, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// 赋值：目标收 `Ident` / `Ident.Member` / `Ident[Index]`。
    Assign(Box<Expr>, Box<Expr>),
    /// 调用。obj = None → 裸标识符（页面函数/采集点）；
    /// obj = Some(Ident("Math")) → 数学白名单；其余 Some → 成员方法
    /// （canvas facet 面，如 `ctx.arc`）。
    Call {
        obj: Option<Box<Expr>>,
        name: String,
        args: Vec<Expr>,
    },
    ArrayLit(Vec<Expr>),
    /// 对象字面量——`{ a, b }` 简写与 `{ key: e }` 两种形态。
    Object(Vec<(String, Expr)>),
    Template(Vec<TplPart>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let(String, Expr),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    /// 裸块——扁平作用域下透明展开（块内重名 let 由编译期查重降级）。
    Block(Vec<Stmt>),
    /// 严格同形头：`for (let v = init; v < bound; v += step) body`。
    For {
        var: String,
        init: Expr,
        bound: Expr,
        allow_eq: bool,
        /// `+=` / `-=` 的表达式；`++` → Num(1)；`--` → Num(-1) 已折入符号。
        step: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
}

/// 一段页面函数源码的解析产物。
pub struct FnDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

// ── 词法 ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum TplSeg {
    Lit(String),
    ExprSrc(String),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Tpl(Vec<TplSeg>),
    Punct(&'static str),
    Eof,
}

/// 长 token 在前（最长匹配）；`++` 等非面 token 也_lex_出来、
/// 由解析器给出可读的拒绝信息。
const PUNCTS: &[&str] = &[
    ">>>", "===", "!==", "==", "!=", "<=", ">=", "&&", "||", ">>", "<<", "++", "--",
    "+=", "-=", "*=", "/=", "%=", "(", ")", "{", "}", "[", "]", ",", ";", ".", "+", "-",
    "*", "/", "%", "<", ">", "=", "!", "?", ":", "~", "^", "&", "|",
];

struct Lexer<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { b: src.as_bytes(), pos: 0 }
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.pos < self.b.len() && (self.b[self.pos] as char).is_whitespace() {
                self.pos += 1;
            }
            if self.pos + 1 < self.b.len() && self.b[self.pos] == b'/' && self.b[self.pos + 1] == b'/' {
                while self.pos < self.b.len() && self.b[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            if self.pos + 1 < self.b.len() && self.b[self.pos] == b'/' && self.b[self.pos + 1] == b'*' {
                self.pos += 2;
                while self.pos + 1 < self.b.len()
                    && !(self.b[self.pos] == b'*' && self.b[self.pos + 1] == b'/')
                {
                    self.pos += 1;
                }
                self.pos = (self.pos + 2).min(self.b.len());
                continue;
            }
            return;
        }
    }

    fn next(&mut self) -> Result<Tok, String> {
        self.skip_trivia();
        if self.pos >= self.b.len() {
            return Ok(Tok::Eof);
        }
        let c = self.b[self.pos];
        // 数字（含十六进制与指数）。
        if c.is_ascii_digit()
            || (c == b'.' && self.pos + 1 < self.b.len() && self.b[self.pos + 1].is_ascii_digit())
        {
            let start = self.pos;
            if c == b'0' && self.pos + 1 < self.b.len() && (self.b[self.pos + 1] | 32) == b'x' {
                self.pos += 2;
                let ds = self.pos;
                while self.pos < self.b.len() && self.b[self.pos].is_ascii_hexdigit() {
                    self.pos += 1;
                }
                if self.pos == ds {
                    return Err("bad hex literal".into());
                }
                let text = std::str::from_utf8(&self.b[ds..self.pos]).unwrap();
                let v = u64::from_str_radix(text, 16).map_err(|_| "hex literal overflow")? as f64;
                return Ok(Tok::Num(v));
            }
            while self.pos < self.b.len()
                && (self.b[self.pos].is_ascii_digit() || self.b[self.pos] == b'.')
            {
                self.pos += 1;
            }
            if self.pos < self.b.len() && (self.b[self.pos] | 32) == b'e' {
                self.pos += 1;
                if self.pos < self.b.len()
                    && (self.b[self.pos] == b'+' || self.b[self.pos] == b'-')
                {
                    self.pos += 1;
                }
                while self.pos < self.b.len() && self.b[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
            }
            let text = std::str::from_utf8(&self.b[start..self.pos]).unwrap();
            let v: f64 = text.parse().map_err(|_| "bad number literal")?;
            return Ok(Tok::Num(v));
        }
        // 标识符/关键字。
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let start = self.pos;
            while self.pos < self.b.len()
                && (self.b[self.pos].is_ascii_alphanumeric()
                    || self.b[self.pos] == b'_'
                    || self.b[self.pos] == b'$')
            {
                self.pos += 1;
            }
            return Ok(Tok::Ident(
                std::str::from_utf8(&self.b[start..self.pos]).unwrap().to_string(),
            ));
        }
        // 字符串。
        if c == b'\'' || c == b'"' {
            let q = c;
            self.pos += 1;
            let mut out = String::new();
            while self.pos < self.b.len() && self.b[self.pos] != q {
                if self.b[self.pos] == b'\\' {
                    self.pos += 1;
                    if self.pos >= self.b.len() {
                        return Err("unterminated string".into());
                    }
                    out.push(match self.b[self.pos] {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        other => other as char,
                    });
                } else {
                    out.push(self.b[self.pos] as char);
                }
                self.pos += 1;
            }
            if self.pos >= self.b.len() {
                return Err("unterminated string".into());
            }
            self.pos += 1;
            return Ok(Tok::Str(out));
        }
        // 模板串：整段在词法层切成 Lit / ExprSrc。
        if c == b'`' {
            self.pos += 1;
            let mut segs: Vec<TplSeg> = Vec::new();
            let mut lit = String::new();
            loop {
                if self.pos >= self.b.len() {
                    return Err("unterminated template literal".into());
                }
                let ch = self.b[self.pos];
                if ch == b'`' {
                    self.pos += 1;
                    break;
                }
                if ch == b'\\' {
                    self.pos += 1;
                    if self.pos >= self.b.len() {
                        return Err("unterminated template literal".into());
                    }
                    lit.push(match self.b[self.pos] {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        other => other as char,
                    });
                    self.pos += 1;
                    continue;
                }
                if ch == b'$' && self.pos + 1 < self.b.len() && self.b[self.pos + 1] == b'{' {
                    if !lit.is_empty() {
                        segs.push(TplSeg::Lit(std::mem::take(&mut lit)));
                    }
                    self.pos += 2;
                    let start = self.pos;
                    let mut depth = 1usize;
                    while self.pos < self.b.len() {
                        let c2 = self.b[self.pos];
                        if c2 == b'{' {
                            depth += 1;
                        } else if c2 == b'}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        } else if c2 == b'\'' || c2 == b'"' {
                            let q = c2;
                            self.pos += 1;
                            while self.pos < self.b.len() && self.b[self.pos] != q {
                                if self.b[self.pos] == b'\\' {
                                    self.pos += 1;
                                }
                                self.pos += 1;
                            }
                        } else if c2 == b'`' {
                            return Err("nested template literal not supported".into());
                        }
                        self.pos += 1;
                    }
                    if self.pos >= self.b.len() {
                        return Err("unterminated template literal".into());
                    }
                    segs.push(TplSeg::ExprSrc(
                        std::str::from_utf8(&self.b[start..self.pos]).unwrap().to_string(),
                    ));
                    self.pos += 1; // 越过 '}'
                    continue;
                }
                lit.push(ch as char);
                self.pos += 1;
            }
            if !lit.is_empty() {
                segs.push(TplSeg::Lit(lit));
            }
            return Ok(Tok::Tpl(segs));
        }
        // 标点（最长匹配）。
        for p in PUNCTS {
            let pb = p.as_bytes();
            if self.b[self.pos..].starts_with(pb) {
                self.pos += pb.len();
                return Ok(Tok::Punct(p));
            }
        }
        Err(format!("unsupported character at byte {}", self.pos))
    }
}

// ── 语法 ────────────────────────────────────────────────────────────

pub struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    pub fn new(src: &str) -> Result<Self, String> {
        let mut lex = Lexer::new(src);
        let mut toks = Vec::new();
        loop {
            let t = lex.next()?;
            let eof = t == Tok::Eof;
            toks.push(t);
            if eof {
                break;
            }
        }
        Ok(Self { toks, pos: 0 })
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        if let Tok::Punct(q) = self.peek() {
            if *q == p {
                self.bump();
                return true;
            }
        }
        false
    }

    fn expect_punct(&mut self, p: &str) -> Result<(), String> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            Err(format!("expected `{p}`, got {:?}", self.peek()))
        }
    }

    fn eat_ident(&mut self, name: &str) -> bool {
        if self.peek() == &Tok::Ident(name.to_string()) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// 解析 `function name(a, b) { body }` 整段源码。
    pub fn parse_fn(src: &str) -> Result<FnDef, String> {
        let mut p = Parser::new(src)?;
        if !p.eat_ident("function") {
            return Err("expected `function`".into());
        }
        let Tok::Ident(name) = p.bump() else {
            return Err("expected function name".into());
        };
        p.expect_punct("(")?;
        let mut params = Vec::new();
        if !p.eat_punct(")") {
            loop {
                let Tok::Ident(param) = p.bump() else {
                    return Err("expected parameter name".into());
                };
                params.push(param);
                if p.eat_punct(",") {
                    continue;
                }
                p.expect_punct(")")?;
                break;
            }
        }
        let body = p.parse_stmt_or_block()?;
        if p.peek() != &Tok::Eof {
            return Err(format!("trailing tokens after function body: {:?}", p.peek()));
        }
        Ok(FnDef { name, params, body })
    }

    /// 解析循环嵌套源码——顶层必须是一条 for 语句。
    pub fn parse_loop(src: &str) -> Result<Stmt, String> {
        let mut p = Parser::new(src)?;
        let stmt = p.parse_stmt()?;
        if p.peek() != &Tok::Eof {
            return Err(format!("trailing tokens after loop: {:?}", p.peek()));
        }
        match stmt {
            f @ Stmt::For { .. } => Ok(f),
            Stmt::Block(mut body)
                if body.len() == 1 && matches!(body[0], Stmt::For { .. }) =>
            {
                Ok(body.remove(0))
            }
            _ => Err("annotated statement is not a for".into()),
        }
    }

    fn parse_stmt_or_block(&mut self) -> Result<Vec<Stmt>, String> {
        if self.eat_punct("{") {
            let mut out = Vec::new();
            loop {
                if self.eat_punct("}") {
                    return Ok(out);
                }
                if self.peek() == &Tok::Eof {
                    return Err("unterminated block".into());
                }
                out.push(self.parse_stmt()?);
            }
        }
        Ok(vec![self.parse_stmt()?])
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek().clone() {
            Tok::Punct("{") => Ok(Stmt::Block(self.parse_stmt_or_block()?)),
            Tok::Ident(kw) if kw == "let" || kw == "const" || kw == "var" => {
                self.bump();
                // 多声明符：let a = e1, b = e2; —— 扁平作用域下展开成多条。
                let mut decls: Vec<Stmt> = Vec::new();
                loop {
                    let Tok::Ident(name) = self.bump() else {
                        return Err("expected binding name".into());
                    };
                    let init = if self.eat_punct("=") {
                        self.parse_expr()?
                    } else {
                        Expr::Undef
                    };
                    decls.push(Stmt::Let(name, init));
                    if self.eat_punct(",") {
                        continue;
                    }
                    self.expect_punct(";")?;
                    break;
                }
                if decls.len() == 1 {
                    Ok(decls.remove(0))
                } else {
                    Ok(Stmt::Block(decls))
                }
            }
            Tok::Ident(kw) if kw == "if" => {
                self.bump();
                self.expect_punct("(")?;
                let cond = self.parse_expr()?;
                self.expect_punct(")")?;
                let then = self.parse_stmt_or_block()?;
                let els = if self.eat_ident("else") {
                    self.parse_stmt_or_block()?
                } else {
                    Vec::new()
                };
                Ok(Stmt::If(cond, then, els))
            }
            Tok::Ident(kw) if kw == "return" => {
                self.bump();
                if self.eat_punct(";") {
                    return Ok(Stmt::Return(None));
                }
                let e = self.parse_expr()?;
                self.expect_punct(";")?;
                Ok(Stmt::Return(Some(e)))
            }
            Tok::Ident(kw) if kw == "for" => {
                self.bump();
                self.expect_punct("(")?;
                // 严格同形头：let v = init ; v (<|<=) bound ; (v += e | v -= e | v++ | v--)
                if !self.eat_ident("let") {
                    return Err("for header must declare `let`".into());
                }
                let Tok::Ident(var) = self.bump() else {
                    return Err("expected loop variable".into());
                };
                self.expect_punct("=")?;
                let init = self.parse_expr()?;
                self.expect_punct(";")?;
                let Tok::Ident(var2) = self.bump() else {
                    return Err("expected loop variable in condition".into());
                };
                if var2 != var {
                    return Err("condition variable must match loop variable".into());
                }
                let allow_eq = if self.eat_punct("<") {
                    false
                } else if self.eat_punct("<=") {
                    true
                } else {
                    return Err("condition must be `<` or `<=`".into());
                };
                let bound = self.parse_expr()?;
                self.expect_punct(";")?;
                let Tok::Ident(var3) = self.bump() else {
                    return Err("expected loop variable in step".into());
                };
                if var3 != var {
                    return Err("step variable must match loop variable".into());
                }
                let step = if self.eat_punct("+=") {
                    self.parse_expr()?
                } else if self.eat_punct("-=") {
                    Expr::Unary(UnOp::Neg, Box::new(self.parse_expr()?))
                } else if self.eat_punct("++") {
                    Expr::Num(1.0)
                } else if self.eat_punct("--") {
                    Expr::Num(-1.0)
                } else {
                    return Err("step must be `+=`/`-=`/`++`/`--`".into());
                };
                self.expect_punct(")")?;
                let body = self.parse_stmt_or_block()?;
                Ok(Stmt::For { var, init, bound, allow_eq, step, body })
            }
            Tok::Ident(kw)
                if matches!(
                    kw.as_str(),
                    "function" | "while" | "do" | "switch" | "try" | "throw" | "break"
                        | "continue" | "class"
                ) =>
            {
                Err(format!("unsupported statement `{kw}`"))
            }
            _ => {
                let e = self.parse_expr()?;
                self.expect_punct(";")?;
                Ok(Stmt::Expr(e))
            }
        }
    }

    // 表达式（优先级爬升；层级表按 JS 实序：位运算低于比较、
    // 位移低于加减）。

    pub fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let cond = self.parse_binary(0)?;
        if self.eat_punct("?") {
            let then = self.parse_expr()?;
            self.expect_punct(":")?;
            let els = self.parse_ternary()?;
            return Ok(Expr::Ternary(Box::new(cond), Box::new(then), Box::new(els)));
        }
        // 赋值（目标面：Ident / Member / Index；编译期再收紧）。
        if self.peek() == &Tok::Punct("=") {
            if matches!(&cond, Expr::Ident(_) | Expr::Member(_, _) | Expr::Index(_, _)) {
                self.bump();
                let value = self.parse_expr()?;
                return Ok(Expr::Assign(Box::new(cond), Box::new(value)));
            }
            return Err("assignment to non-assignable target".into());
        }
        // 复合赋值脱糖：x op= e → x = x op e。
        const COMPOUND: &[(&str, BinOp)] = &[
            ("+=", BinOp::Add),
            ("-=", BinOp::Sub),
            ("*=", BinOp::Mul),
            ("/=", BinOp::Div),
            ("%=", BinOp::Mod),
        ];
        if let Tok::Punct(p) = self.peek() {
            if let Some((_, op)) = COMPOUND.iter().find(|(t, _)| t == p) {
                if matches!(&cond, Expr::Ident(_) | Expr::Member(_, _) | Expr::Index(_, _)) {
                    let op = *op;
                    self.bump();
                    let value = self.parse_expr()?;
                    return Ok(Expr::Assign(
                        Box::new(cond.clone()),
                        Box::new(Expr::Binary(op, Box::new(cond), Box::new(value))),
                    ));
                }
            }
        }
        Ok(cond)
    }

    fn parse_binary(&mut self, min_level: usize) -> Result<Expr, String> {
        // 层级（低→高）：|| && | ^ & == 比较 位移 加减 乘除。min_level 按索引截断。
        const LEVELS: &[&[&str]] = &[
            &["||"],
            &["&&"],
            &["|"],
            &["^"],
            &["&"],
            &["==", "===", "!=", "!=="],
            &["<", "<=", ">", ">="],
            &["<<", ">>", ">>>"],
            &["+", "-"],
            &["*", "/", "%"],
        ];
        if min_level >= LEVELS.len() {
            return self.parse_unary();
        }
        let tokens = LEVELS[min_level];
        let mut lhs = self.parse_binary(min_level + 1)?;
        loop {
            let Tok::Punct(p) = self.peek() else { break };
            let p: &str = p;
            if !tokens.contains(&p) {
                break;
            }
            self.bump();
            let rhs = self.parse_binary(min_level + 1)?;
            lhs = match p {
                "||" => Expr::Logical(false, Box::new(lhs), Box::new(rhs)),
                "&&" => Expr::Logical(true, Box::new(lhs), Box::new(rhs)),
                "==" | "===" => Expr::Binary(BinOp::Eq, Box::new(lhs), Box::new(rhs)),
                "!=" | "!==" => Expr::Binary(BinOp::Neq, Box::new(lhs), Box::new(rhs)),
                "<" => Expr::Binary(BinOp::Lt, Box::new(lhs), Box::new(rhs)),
                "<=" => Expr::Binary(BinOp::LtE, Box::new(lhs), Box::new(rhs)),
                ">" => Expr::Binary(BinOp::Gt, Box::new(lhs), Box::new(rhs)),
                ">=" => Expr::Binary(BinOp::GtE, Box::new(lhs), Box::new(rhs)),
                "<<" => Expr::Binary(BinOp::Shl, Box::new(lhs), Box::new(rhs)),
                ">>" => Expr::Binary(BinOp::ShrS, Box::new(lhs), Box::new(rhs)),
                ">>>" => Expr::Binary(BinOp::ShrU, Box::new(lhs), Box::new(rhs)),
                "+" => Expr::Binary(BinOp::Add, Box::new(lhs), Box::new(rhs)),
                "-" => Expr::Binary(BinOp::Sub, Box::new(lhs), Box::new(rhs)),
                "*" => Expr::Binary(BinOp::Mul, Box::new(lhs), Box::new(rhs)),
                "/" => Expr::Binary(BinOp::Div, Box::new(lhs), Box::new(rhs)),
                "%" => Expr::Binary(BinOp::Mod, Box::new(lhs), Box::new(rhs)),
                "|" => Expr::Binary(BinOp::BitOr, Box::new(lhs), Box::new(rhs)),
                "^" => Expr::Binary(BinOp::BitXor, Box::new(lhs), Box::new(rhs)),
                "&" => Expr::Binary(BinOp::BitAnd, Box::new(lhs), Box::new(rhs)),
                _ => break,
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.eat_punct("-") {
            return Ok(Expr::Unary(UnOp::Neg, Box::new(self.parse_unary()?)));
        }
        if self.eat_punct("+") {
            return self.parse_unary();
        }
        if self.eat_punct("!") {
            return Ok(Expr::Unary(UnOp::Not, Box::new(self.parse_unary()?)));
        }
        if self.eat_punct("~") {
            return Ok(Expr::Unary(UnOp::BitNot, Box::new(self.parse_unary()?)));
        }
        if self.eat_punct("++") || self.eat_punct("--") {
            return Err("++/-- not supported".into());
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_primary()?;
        loop {
            if self.eat_punct(".") {
                let Tok::Ident(key) = self.bump() else {
                    return Err("expected member name".into());
                };
                if self.eat_punct("(") {
                    let args = self.parse_args()?;
                    e = Expr::Call { obj: Some(Box::new(e)), name: key, args };
                } else {
                    e = Expr::Member(Box::new(e), key);
                }
                continue;
            }
            if self.eat_punct("[") {
                let idx = self.parse_expr()?;
                self.expect_punct("]")?;
                e = Expr::Index(Box::new(e), Box::new(idx));
                continue;
            }
            if self.peek() == &Tok::Punct("++") || self.peek() == &Tok::Punct("--") {
                return Err("++/-- not supported".into());
            }
            break;
        }
        Ok(e)
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.eat_punct(")") {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if self.eat_punct(",") {
                continue;
            }
            self.expect_punct(")")?;
            break;
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Tok::Num(v) => Ok(Expr::Num(v)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Tpl(segs) => {
                let mut parts = Vec::with_capacity(segs.len());
                for seg in segs {
                    match seg {
                        TplSeg::Lit(s) => parts.push(TplPart::Lit(s)),
                        TplSeg::ExprSrc(src) => {
                            let mut sub = Parser::new(&src)?;
                            let e = sub.parse_expr()?;
                            if sub.peek() != &Tok::Eof {
                                return Err("unsupported expression in template literal".into());
                            }
                            parts.push(TplPart::Expr(e));
                        }
                    }
                }
                Ok(Expr::Template(parts))
            }
            Tok::Ident(id) => match id.as_str() {
                "true" => Ok(Expr::Bool(true)),
                "false" => Ok(Expr::Bool(false)),
                "null" => Ok(Expr::Null),
                "undefined" => Ok(Expr::Undef),
                "new" | "typeof" | "delete" | "await" | "yield" | "this" => {
                    Err(format!("unsupported `{id}`"))
                }
                _ => {
                    if self.peek() == &Tok::Punct("(") {
                        self.bump();
                        let args = self.parse_args()?;
                        return Ok(Expr::Call { obj: None, name: id, args });
                    }
                    Ok(Expr::Ident(id))
                }
            },
            Tok::Punct("(") => {
                let e = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            Tok::Punct("[") => {
                let mut items = Vec::new();
                if self.eat_punct("]") {
                    return Ok(Expr::ArrayLit(items));
                }
                loop {
                    items.push(self.parse_expr()?);
                    if self.eat_punct(",") {
                        if self.eat_punct("]") {
                            break;
                        }
                        continue;
                    }
                    self.expect_punct("]")?;
                    break;
                }
                Ok(Expr::ArrayLit(items))
            }
            Tok::Punct("{") => {
                // 对象字面量（表达式位置；语句位置的 `{` 已被当块吃掉）。
                let mut fields = Vec::new();
                if self.eat_punct("}") {
                    return Ok(Expr::Object(fields));
                }
                loop {
                    match self.bump() {
                        Tok::Ident(key) => {
                            if self.eat_punct(":") {
                                let v = self.parse_expr()?;
                                fields.push((key, v));
                            } else {
                                // 简写 { shape } → { shape: shape }。
                                fields.push((key.clone(), Expr::Ident(key)));
                            }
                        }
                        Tok::Str(key) => {
                            self.expect_punct(":")?;
                            let v = self.parse_expr()?;
                            fields.push((key, v));
                        }
                        other => return Err(format!("unsupported object key {other:?}")),
                    }
                    if self.eat_punct(",") {
                        if self.eat_punct("}") {
                            break;
                        }
                        continue;
                    }
                    self.expect_punct("}")?;
                    break;
                }
                Ok(Expr::Object(fields))
            }
            other => Err(format!("unsupported token {other:?}")),
        }
    }
}
