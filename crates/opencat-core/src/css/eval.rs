//! TokenList 级求值：var() 替换、calc()/color-mix() 化简、颜色归一。
//!
//! 浏览器把「替换 → 计算」做成两个独立阶段；这里同理：
//! 1. [`substitute_vars`] 递归展开 var() 引用；
//! 2. [`eval_token_list`] 把 calc()/color-mix() 等可静态求值的函数化简成
//!    字面 token，再经 `Property::parse_string` 交还 lightningcss 得到强
//!    类型声明。
//!
//! 无法化简的值（px/% 混合 calc、env()、CurrentColor 等）返回 None，
//! 上层丢弃该声明并告警——与 tailwind.rs 对未知类的既有行为一致。

use std::collections::HashMap;

use lightningcss::properties::custom::{
    Function, Token, TokenList, TokenOrValue, UnparsedProperty, Variable,
};
use lightningcss::properties::Property;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions};
use lightningcss::traits::Parse;
use lightningcss::traits::ToCss;
use lightningcss::values::color::{CssColor, HueInterpolationMethod, OKLAB, SRGB};
use lightningcss::values::length::LengthValue;

use super::EvalContext;

/// 把可能含 var()/calc() 的声明化简成强类型；typed 声明原样返回。
pub fn resolve_to_typed(
    prop: &Property<'static>,
    vars: &HashMap<String, TokenList<'static>>,
    ctx: &EvalContext,
) -> Vec<Property<'static>> {
    match prop {
        Property::Unparsed(unparsed) => resolve_unparsed(unparsed, vars, ctx),
        // typed 值内部按 lightningcss 语义不可能再含 var（含则必为
        // Unparsed），直接透传。
        other => vec![other.clone()],
    }
}

fn resolve_unparsed(
    unparsed: &UnparsedProperty<'static>,
    vars: &HashMap<String, TokenList<'static>>,
    ctx: &EvalContext,
) -> Vec<Property<'static>> {
    let Some(literal) = eval_token_list(&unparsed.value, vars, ctx, 0) else {
        log::warn!("css: {} 无法求值（var 缺失或不可化简），已丢弃", unparse_name(unparsed));
        return Vec::new();
    };
    let Some(text) = serialize_tokens(&literal) else {
        log::warn!("css: {} 含不可序列化 token，已丢弃", unparse_name(unparsed));
        return Vec::new();
    };
    // 产出是 Property<'i>，会借用输入文本（token 流内嵌 CowArcStr）；
    // 泄漏声明值文本换 'static，与样式表同策略，量级为单条声明值。
    let leaked: &'static str = Box::leak(text.clone().into_boxed_str());
    match Property::parse_string(unparsed.property_id.clone(), leaked, ParserOptions::default()) {
        Ok(typed) => vec![typed],
        Err(_) => {
            log::warn!("css: {} 值 {:?} 无法解析，已丢弃", unparse_name(unparsed), text);
            Vec::new()
        }
    }
}

fn unparse_name(unparsed: &UnparsedProperty<'static>) -> String {
    match unparsed.property_id.to_css_string(PrinterOptions::default()) {
        Ok(name) => name,
        Err(_) => "<prop>".into(),
    }
}

/// 递归展开 var() 引用；函数与其余 token 原样保留（子层照常替换）。
fn substitute_vars(
    tokens: &TokenList<'static>,
    vars: &HashMap<String, TokenList<'static>>,
    ctx: &EvalContext,
    depth: u8,
) -> Option<TokenList<'static>> {
    if depth > 32 {
        return None;
    }
    let mut out = Vec::with_capacity(tokens.0.len());
    for item in &tokens.0 {
        match item {
            TokenOrValue::Var(var) => {
                out.extend(lookup_var(var, vars, ctx, depth)?.0);
            }
            TokenOrValue::Function(func) => {
                out.push(TokenOrValue::Function(Function {
                    name: func.name.clone(),
                    arguments: substitute_vars(&func.arguments, vars, ctx, depth)?,
                }));
            }
            other => out.push(other.clone()),
        }
    }
    Some(TokenList(out))
}

/// 替换 + 化简：颜色归一成 hex 字面，calc/color-mix 折叠成单 token。
fn eval_token_list(
    tokens: &TokenList<'static>,
    vars: &HashMap<String, TokenList<'static>>,
    ctx: &EvalContext,
    depth: u8,
) -> Option<TokenList<'static>> {
    let substituted = substitute_vars(tokens, vars, ctx, depth)?;
    let mut out = Vec::with_capacity(substituted.0.len());
    for item in substituted.0 {
        match item {
            TokenOrValue::Function(func) => match func.name.0.as_ref() {
                "calc" => out.push(eval_calc(&func.arguments, ctx)?),
                "color-mix" => out.push(TokenOrValue::Token(eval_color_mix(&func.arguments)?)),
                _ => out.push(TokenOrValue::Function(func)),
            },
            TokenOrValue::Color(color) => {
                out.push(TokenOrValue::Token(rgba_token(css_color_to_f32(&color)?)));
            }
            other => out.push(other),
        }
    }
    Some(TokenList(out))
}

fn lookup_var(
    var: &Variable<'static>,
    vars: &HashMap<String, TokenList<'static>>,
    ctx: &EvalContext,
    depth: u8,
) -> Option<TokenList<'static>> {
    let key = var.name.ident.0.trim_start_matches("--");
    if let Some(value) = vars.get(key) {
        return substitute_vars(value, vars, ctx, depth + 1);
    }
    match &var.fallback {
        Some(fallback) => substitute_vars(fallback, vars, ctx, depth + 1),
        // 未定义且无 fallback：按 CSS 语义该声明失效（不是报错）。
        None => None,
    }
}

/// color-mix(in <space>, C1 [P1]? , C2 [P2]?) → hex 字面。
/// 混合交由 lightningcss 的规范实现（premultiplied alpha）。
fn eval_color_mix(args: &TokenList<'static>) -> Option<Token<'static>> {
    // 嵌套颜色函数（oklch()/rgb() 等）先解析成 Color 值，再剥离空白。
    let coerced: Vec<TokenOrValue<'static>> = args
        .0
        .iter()
        .filter(|t| !matches!(t, TokenOrValue::Token(Token::WhiteSpace(_))))
        .map(|t| match t {
            TokenOrValue::Function(f) if is_color_function(f.name.0.as_ref()) => {
                let inner = serialize_tokens(&f.arguments)?;
                let text = format!("{}({})", f.name.0.as_ref(), inner);
                CssColor::parse_string(&text).ok().map(TokenOrValue::Color)
            }
            other => Some(other.clone()),
        })
        .collect::<Option<Vec<_>>>()?;
    let items = &coerced;
    let space = match (items.first(), items.get(1)) {
        (Some(TokenOrValue::Token(Token::Ident(i))), Some(TokenOrValue::Token(Token::Ident(s))))
            if i.eq_ignore_ascii_case("in") =>
        {
            s.to_ascii_lowercase()
        }
        _ => return None,
    };
    // `in <space>` 后应紧跟逗号；颜色段从逗号之后开始。
    let rest = match items.get(2) {
        Some(TokenOrValue::Token(Token::Comma)) => &items[3..],
        _ => return None,
    };

    let sep = rest
        .iter()
        .position(|t| matches!(t, TokenOrValue::Token(Token::Comma)))?;
    let (c1, w1) = parse_color_with_weight(&rest[..sep])?;
    let (c2, w2) = parse_color_with_weight(&rest[sep + 1..])?;
    let (w1, w2) = if w1 + w2 == 0.0 {
        (0.5, 0.5)
    } else {
        (w1 / (w1 + w2), w2 / (w1 + w2))
    };
    let mixed = match space.as_str() {
        "oklab" => c1.interpolate::<OKLAB>(w1, &c2, w2, HueInterpolationMethod::Shorter),
        "srgb" => c1.interpolate::<SRGB>(w1, &c2, w2, HueInterpolationMethod::Shorter),
        _ => return None, // hsl/lch 等混合空间按需再扩
    }
    .ok()?;
    let rgba = mixed.to_rgb().ok()?;
    let CssColor::RGBA(rgba) = rgba else {
        return None;
    };
    Some(rgba_token([
        rgba.red as f32 / 255.0,
        rgba.green as f32 / 255.0,
        rgba.blue as f32 / 255.0,
        rgba.alpha as f32 / 255.0,
    ]))
}

fn is_color_function(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "rgb" | "rgba" | "hsl" | "hsla" | "hwb" | "lab" | "lch" | "oklab" | "oklch"
    )
}

/// 颜色 token 流（可能尾随权重百分比）→ (CssColor, weight)。
fn parse_color_with_weight(tokens: &[TokenOrValue<'static>]) -> Option<(CssColor, f32)> {
    let mut color: Option<CssColor> = None;
    let mut weight = 1.0;
    for t in tokens {
        match t {
            TokenOrValue::Color(c) => color = Some(c.clone()),
            // `transparent` 即 rgba(0,0,0,0)。
            TokenOrValue::Token(Token::Ident(i)) if i.eq_ignore_ascii_case("transparent") => {
                color = Some(CssColor::RGBA(lightningcss::values::color::RGBA {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 0,
                }))
            }
            TokenOrValue::Token(Token::Percentage { unit_value, .. }) => weight = *unit_value,
            TokenOrValue::Token(Token::WhiteSpace(_)) => {}
            _ => return None,
        }
    }
    Some((color?, weight))
}

/// calc/数学表达式的操作数类型。
#[derive(Debug, Clone, Copy, PartialEq)]
enum CV {
    Num(f32),
    Px(f32),
    Pct(f32),
    Deg(f32),
}

impl CV {
    fn kind(self) -> u8 {
        match self {
            CV::Num(_) => 0,
            CV::Px(_) => 1,
            CV::Pct(_) => 2,
            CV::Deg(_) => 3,
        }
    }
}

/// 小型 Pratt 解析器：+ - * / 与括号，`*` `/` 右操作数必须为纯数。
fn eval_calc(tokens: &TokenList<'static>, ctx: &EvalContext) -> Option<TokenOrValue<'static>> {
    // 自定义属性原样保留空白，运算解析只认操作数与分隔符。
    let items_vec: Vec<TokenOrValue<'static>> = tokens
        .0
        .iter()
        .filter(|t| !matches!(t, TokenOrValue::Token(Token::WhiteSpace(_))))
        .cloned()
        .collect();
    let items = &items_vec;
    let mut pos = 0usize;

    fn parse_operand(
        items: &[TokenOrValue<'static>],
        pos: &mut usize,
        ctx: &EvalContext,
    ) -> Option<CV> {
        let item = items.get(*pos)?;
        *pos += 1;
        match item {
            TokenOrValue::Token(Token::Number { value, .. }) => Some(CV::Num(*value)),
            TokenOrValue::Token(Token::Percentage { unit_value, .. }) => Some(CV::Pct(*unit_value)),
            TokenOrValue::Token(Token::Dimension { value, unit, .. }) => {
                let px = dimension_to_px(*value, unit, ctx)?;
                Some(CV::Px(px))
            }
            TokenOrValue::Length(len) => Some(CV::Px(length_to_px(len.clone(), ctx)?)),
            TokenOrValue::Token(Token::ParenthesisBlock) => {
                // 括号子表达式：递归到 CloseParenthesis。
                let inner_end = items[*pos..]
                    .iter()
                    .position(|t| matches!(t, TokenOrValue::Token(Token::CloseParenthesis)))?;
                let inner = TokenList(items[*pos..*pos + inner_end].to_vec());
                *pos += inner_end + 1;
                match eval_calc(&inner, ctx)? {
                    TokenOrValue::Token(Token::Number { value, .. }) => Some(CV::Num(value)),
                    TokenOrValue::Token(Token::Percentage { unit_value, .. }) => {
                        Some(CV::Pct(unit_value))
                    }
                    TokenOrValue::Length(l) => Some(CV::Px(length_to_px(l, ctx)?)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn apply(op: char, a: CV, b: CV) -> Option<CV> {
        match op {
            '+' | '-' => {
                if a.kind() != b.kind() {
                    return None; // px/% 混合运算不化简（NodeStyle 无处安放）
                }
                let sign = if op == '+' { 1.0 } else { -1.0 };
                Some(match (a, b) {
                    (CV::Num(x), CV::Num(y)) => CV::Num(x + sign * y),
                    (CV::Px(x), CV::Px(y)) => CV::Px(x + sign * y),
                    (CV::Pct(x), CV::Pct(y)) => CV::Pct(x + sign * y),
                    (CV::Deg(x), CV::Deg(y)) => CV::Deg(x + sign * y),
                    _ => None?,
                })
            }
            '*' => {
                let (v, k) = match (a, b) {
                    (CV::Num(k), other) => (other, k),
                    (other, CV::Num(k)) => (other, k),
                    _ => return None,
                };
                Some(scale(v, k))
            }
            '/' => {
                let CV::Num(d) = b else { return None };
                if d == 0.0 {
                    return None;
                }
                Some(scale(a, 1.0 / d))
            }
            _ => None,
        }
    }

    fn scale(v: CV, k: f32) -> CV {
        match v {
            CV::Num(x) => CV::Num(x * k),
            CV::Px(x) => CV::Px(x * k),
            CV::Pct(x) => CV::Pct(x * k),
            CV::Deg(x) => CV::Deg(x * k),
        }
    }

    let mut lhs = parse_operand(items, &mut pos, ctx)?;
    while pos < items.len() {
        let Some(TokenOrValue::Token(Token::Delim(op))) = items.get(pos) else {
            return None;
        };
        let op = *op;
        pos += 1;
        let rhs = parse_operand(items, &mut pos, ctx)?;
        // 乘除优先：先折叠右侧高优先级段。
        let mut pending_rhs = rhs;
        while pos < items.len() {
            let Some(TokenOrValue::Token(Token::Delim(next_op))) = items.get(pos) else {
                break;
            };
            if !matches!(next_op, '*' | '/') {
                break;
            }
            let nop = *next_op;
            pos += 1;
            let next_rhs = parse_operand(items, &mut pos, ctx)?;
            pending_rhs = apply(nop, pending_rhs, next_rhs)?;
        }
        lhs = apply(op, lhs, pending_rhs)?;
    }

    Some(match lhs {
        CV::Num(v) => TokenOrValue::Token(Token::Number { has_sign: v < 0.0, value: v, int_value: None }),
        CV::Px(v) => TokenOrValue::Length(LengthValue::Px(v)),
        CV::Pct(v) => TokenOrValue::Token(Token::Percentage { has_sign: false, unit_value: v, int_value: None }),
        CV::Deg(v) => TokenOrValue::Token(Token::Dimension {
            has_sign: false,
            value: v,
            int_value: None,
            unit: "deg".into(),
        }),
    })
}

fn dimension_to_px(value: f32, unit: &str, ctx: &EvalContext) -> Option<f32> {
    Some(match unit.to_ascii_lowercase().as_str() {
        "px" => value,
        "rem" => value * ctx.rem_base,
        "em" => value * ctx.rem_base, // 父字号传播为后续增强
        "pt" => value * 96.0 / 72.0,
        "pc" => value * 16.0,
        "in" => value * 96.0,
        "cm" => value * 96.0 / 2.54,
        "mm" => value * 96.0 / 25.4,
        "vw" => value * ctx.viewport.0 / 100.0,
        "vh" => value * ctx.viewport.1 / 100.0,
        "vmin" => value * ctx.viewport.0.min(ctx.viewport.1) / 100.0,
        "vmax" => value * ctx.viewport.0.max(ctx.viewport.1) / 100.0,
        _ => return None,
    })
}

fn length_to_px(len: LengthValue, ctx: &EvalContext) -> Option<f32> {
    Some(match len {
        LengthValue::Px(v) => v,
        LengthValue::Rem(v) => v * ctx.rem_base,
        LengthValue::Em(v) => v * ctx.rem_base,
        LengthValue::Vw(v) => v * ctx.viewport.0 / 100.0,
        LengthValue::Vh(v) => v * ctx.viewport.1 / 100.0,
        LengthValue::Vmin(v) => v * ctx.viewport.0.min(ctx.viewport.1) / 100.0,
        LengthValue::Vmax(v) => v * ctx.viewport.0.max(ctx.viewport.1) / 100.0,
        _ => return None,
    })
}

/// CssColor → sRGB f32。LAB 系/display-p3 的转换全部交给 lightningcss。
pub(super) fn css_color_to_f32(color: &CssColor) -> Option<[f32; 4]> {
    let rgb = color.to_rgb().ok()?;
    let CssColor::RGBA(rgba) = rgb else {
        return None;
    };
    Some([
        rgba.red as f32 / 255.0,
        rgba.green as f32 / 255.0,
        rgba.blue as f32 / 255.0,
        rgba.alpha as f32 / 255.0,
    ])
}

/// 字面颜色输出为 #rrggbbaa：Ident 形态会把括号转义、Function 形态要拼
/// token 树，hex 最稳。
fn rgba_token(rgba: [f32; 4]) -> Token<'static> {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Token::IDHash(format!(
        "{:02x}{:02x}{:02x}{:02x}",
        ch(rgba[0]),
        ch(rgba[1]),
        ch(rgba[2]),
        ch(rgba[3])
    )
    .into())
}

/// TokenList → CSS 文本。TokenList 本身没有 ToCss 实现；这里只覆盖
/// eval 阶段可能产出的字面 token 面（数字/单位/颜色/函数/分隔符）。
pub(crate) fn serialize_tokens(tokens: &TokenList<'static>) -> Option<String> {
    let mut out = String::new();
    serialize_into(&tokens.0, &mut out)?;
    Some(out)
}

fn serialize_into(items: &[TokenOrValue<'static>], out: &mut String) -> Option<()> {
    for item in items {
        match item {
            TokenOrValue::Token(token) => serialize_token(token, out)?,
            TokenOrValue::Length(LengthValue::Px(v)) => {
                out.push_str(&format!("{v}px"));
            }
            TokenOrValue::Length(_) => return None,
            TokenOrValue::Color(_) => return None, // eval 已把颜色归一成 hex 字面
            TokenOrValue::Function(func) => {
                out.push_str(func.name.0.as_ref());
                out.push('(');
                serialize_into(&func.arguments.0, out)?;
                out.push(')');
            }
            TokenOrValue::Var(_) | TokenOrValue::Env(_) => return None,
            TokenOrValue::DashedIdent(_) | TokenOrValue::Url(_) | TokenOrValue::AnimationName(_) => {
                return None
            }
            TokenOrValue::Angle(a) => out.push_str(&format!("{}deg", a.to_degrees())),
            TokenOrValue::Time(_) | TokenOrValue::Resolution(_) => return None,
            TokenOrValue::UnresolvedColor(_) => return None,
        }
    }
    Some(())
}

fn serialize_token(token: &Token<'static>, out: &mut String) -> Option<()> {
    match token {
        Token::Ident(s) => out.push_str(s),
        Token::IDHash(s) => {
            out.push('#');
            out.push_str(s);
        }
        Token::Number { value, .. } => out.push_str(&format_css_number(*value)),
        Token::Percentage { unit_value, .. } => {
            out.push_str(&format_css_number(*unit_value));
            out.push('%');
        }
        Token::Dimension { value, unit, .. } => {
            out.push_str(&format_css_number(*value));
            out.push_str(unit);
        }
        Token::Delim(op) => {
            out.push(' ');
            out.push(*op);
            out.push(' ');
        }
        Token::Comma => out.push(','),
        Token::Function(name) => {
            out.push_str(name);
            out.push('(');
        }
        Token::CloseParenthesis => out.push(')'),
        Token::WhiteSpace(_) => out.push(' '),
        _ => return None,
    }
    Some(())
}

fn format_css_number(v: f32) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 {
        format!("{}", v as i64)
    } else {
        format!("{v:.4}")
    }
}
