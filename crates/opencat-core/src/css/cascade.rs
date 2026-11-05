//! 选择器匹配与层叠排序。
//!
//! 排序键按 CSS 层叠规范：`!important` > layer > specificity > 源顺序，
//! 全部升序、后者胜。specificity 直接取 selectors crate 的打包值
//! （class/attr 位与元素位不重叠，整数字典序等价于 (a,b,c) 字典序）。
//! unlayered 规则的层号是 usize::MAX，在层键里自然排到最高优先。
//!
//! 匹配语义（docs/html-align/DESIGN.md）：支持完整组合器链——后代（空格）、
//! 子（`>`）、相邻（`+`）、通用（`~`）——按右到左经典算法，需要节点在文档
//! 树里的祖先链与前部兄弟链上下文（[`SelectorContext`]）。伪元素/伪类仍
//! 在解析层被丢弃（stylesheet.rs `selector_is_supported`）。

use super::stylesheet::CssStylesheet;
use lightningcss::properties::Property;
use lightningcss::selector::{Combinator, Component, Selector};

/// 排序后的单条声明。
pub struct AppliedDeclaration {
    pub property: Property<'static>,
    pub important: bool,
    pub layer: usize,
    pub specificity: u32,
    pub order: usize,
}

/// 树位置信息：祖先/兄弟链上的一员（HTML 语义标签名 + id + class 串）。
#[derive(Clone, Copy, Debug)]
pub struct SelectorNode<'a> {
    pub tag: &'a str,
    pub id: &'a str,
    pub classes: &'a str,
}

/// 一个节点的级联匹配上下文。空 context（全部缺省）时组合器永不命中，
/// 行为退化为旧的单 compound 匹配——JSX 通道保持零回归。
#[derive(Clone, Copy, Debug, Default)]
pub struct SelectorContext<'a> {
    /// 祖先链（近→远，[0] = 父）。
    pub ancestors: &'a [SelectorNode<'a>],
    /// 前部兄弟链（近→远，[0] = 紧邻前兄）。
    pub prev_siblings: &'a [SelectorNode<'a>],
    /// tag 别名：HTML 根节点同时匹配 html/body 规则。
    pub tag_aliases: &'a [&'a str],
}

/// 对一个节点收集全部命中声明并按层叠序排列（无树上下文的旧入口）。
pub fn match_rules(
    sheet: &CssStylesheet,
    class_name: &str,
    element_id: &str,
    tag: &str,
) -> Vec<AppliedDeclaration> {
    match_rules_ctx(sheet, class_name, element_id, tag, &SelectorContext::default())
}

/// 带树上下文的命中收集。
pub fn match_rules_ctx(
    sheet: &CssStylesheet,
    class_name: &str,
    element_id: &str,
    tag: &str,
    ctx: &SelectorContext<'_>,
) -> Vec<AppliedDeclaration> {
    let classes: ahash::HashSet<&str> = class_name.split_whitespace().collect();
    let mut out: Vec<AppliedDeclaration> = Vec::new();
    for rule in &sheet.rules {
        if !rule.selectors.iter().any(|sel| {
            selector_matches(sel, &classes, element_id, tag, ctx)
        }) {
            continue;
        }
        let specificity = rule
            .selectors
            .iter()
            .map(|sel| sel.specificity())
            .max()
            .unwrap_or(0);
        for property in &rule.declarations {
            out.push(AppliedDeclaration {
                property: property.clone(),
                important: false,
                layer: rule.layer,
                specificity,
                order: rule.order,
            });
        }
        for property in &rule.important {
            out.push(AppliedDeclaration {
                property: property.clone(),
                important: true,
                layer: rule.layer,
                specificity,
                order: rule.order,
            });
        }
    }
    out.sort_by(|a, b| {
        (a.important, a.layer, a.specificity, a.order).cmp(&(
            b.important,
            b.layer,
            b.specificity,
            b.order,
        ))
    });
    sort_and_dedup(out)
}

/// 已按层叠键升序的声明做属性级去重；多张样式表（作者 + UA 兜底）合并后
/// 复用同一段排序管道。
pub fn sort_and_dedup(out: Vec<AppliedDeclaration>) -> Vec<AppliedDeclaration> {
    // 层叠去重：每个属性只保留排序后的胜者（后者胜）。width 这类单值
    // 字段重复写天然覆盖，但 box-shadow 这类列表字段是 push 语义，重复
    // 声明会把阴影叠加绘制。--tw-* 变量键不同、互不去重，仍按声明序进
    // vars；shorthand 与 longhand 是不同 id，各自去重后靠稳定排序的
    // 「先简写后长写」应用顺序保证覆盖。
    let mut kept: Vec<AppliedDeclaration> = Vec::with_capacity(out.len());
    for decl in out.into_iter().rev() {
        let pid = decl.property.property_id();
        if kept
            .iter()
            .any(|k: &AppliedDeclaration| k.property.property_id() == pid)
        {
            continue;
        }
        kept.push(decl);
    }
    kept.reverse();
    kept
}

/// 一个待匹配节点视图：subject 用（含别名与 text/image 对齐）。
struct SubjectInfo<'a> {
    tags: Vec<&'a str>,
    id: &'a str,
    classes: &'a ahash::HashSet<&'a str>,
}

fn subject_info<'a>(
    tag: &'a str,
    element_id: &'a str,
    classes: &'a ahash::HashSet<&'a str>,
    aliases: &'a [&'a str],
) -> SubjectInfo<'a> {
    let mut tags = vec![tag];
    // 引擎节点名 → HTML 标签名对齐（text 在 DOM 里由 span 承载）。
    if tag == "text" {
        tags.push("span");
    }
    if tag == "image" {
        tags.push("img");
    }
    for alias in aliases {
        tags.push(alias);
    }
    SubjectInfo {
        tags,
        id: element_id,
        classes,
    }
}

fn tags_match(tags: &[&str], name: &str) -> bool {
    tags.iter().any(|t| *t == name)
}

/// 把 selector 的 match order 切成 compound 序列与连接组合器序列。
/// `iter_raw_match_order` 右到左：compounds[0] 是 subject。
fn split_selector<'a>(
    selector: &'a Selector<'static>,
) -> Option<(Vec<Vec<&'a Component<'static>>>, Vec<Combinator>)> {
    let mut compounds: Vec<Vec<&'a Component<'static>>> = Vec::new();
    let mut combinators: Vec<Combinator> = Vec::new();
    let mut current: Vec<&'a Component<'static>> = Vec::new();
    for comp in selector.iter_raw_match_order() {
        match comp {
            // 伪元素派生的内部组合器（::before 等在解析层已被丢弃）不算。
            Component::Combinator(c)
                if !matches!(
                    c,
                    Combinator::PseudoElement | Combinator::SlotAssignment | Combinator::Part
                ) =>
            {
                compounds.push(std::mem::take(&mut current));
                combinators.push(*c);
            }
            other => current.push(other),
        }
    }
    compounds.push(current);
    Some((compounds, combinators))
}

fn selector_matches(
    selector: &Selector<'static>,
    classes: &ahash::HashSet<&str>,
    element_id: &str,
    tag: &str,
    ctx: &SelectorContext<'_>,
) -> bool {
    let Some((compounds, combinators)) = split_selector(selector) else {
        return false;
    };
    let subject = subject_info(tag, element_id, classes, ctx.tag_aliases);
    match_chain(&compounds, &combinators, 0, 0, &subject, ctx)
}

/// 右到左递归。`depth` = 当前 compound 的目标距 subject 的层数：
/// 0 = subject 本身，d ≥ 1 = ancestors[d-1]。后代组合器在祖先链上回溯，
/// 子组合器严格走父层；兄弟组合器只在 subject 层生效（祖先级兄弟链不
/// 追踪——语料近似，见 DESIGN.md）。
fn match_chain(
    compounds: &[Vec<&Component<'static>>],
    combinators: &[Combinator],
    idx: usize,
    depth: usize,
    subject: &SubjectInfo<'_>,
    ctx: &SelectorContext<'_>,
) -> bool {
    let matched = if depth == 0 {
        compound_matches_parts(&compounds[idx], subject)
    } else {
        ctx.ancestors
            .get(depth - 1)
            .is_some_and(|node| compound_matches_node(&compounds[idx], node))
    };
    if !matched {
        return false;
    }
    if idx == compounds.len() - 1 {
        return true;
    }
    match combinators[idx] {
        Combinator::Child => match_chain(compounds, combinators, idx + 1, depth + 1, subject, ctx),
        Combinator::Descendant => (depth + 1..=ctx.ancestors.len())
            .any(|d| match_chain(compounds, combinators, idx + 1, d, subject, ctx)),
        Combinator::NextSibling => {
            depth == 0
                && ctx.prev_siblings.first().is_some_and(|prev| {
                    let classes: ahash::HashSet<&str> = prev.classes.split_whitespace().collect();
                    let info = SubjectInfo {
                        tags: vec![prev.tag],
                        id: prev.id,
                        classes: &classes,
                    };
                    compound_matches_parts(&compounds[idx + 1], &info)
                        && idx + 1 == compounds.len() - 1
                })
        }
        Combinator::LaterSibling => {
            depth == 0
                && ctx.prev_siblings.iter().any(|prev| {
                    let classes: ahash::HashSet<&str> = prev.classes.split_whitespace().collect();
                    let info = SubjectInfo {
                        tags: vec![prev.tag],
                        id: prev.id,
                        classes: &classes,
                    };
                    compound_matches_parts(&compounds[idx + 1], &info)
                        && idx + 1 == compounds.len() - 1
                })
        }
        _ => false,
    }
}

fn compound_matches_parts(parts: &[&Component<'static>], info: &SubjectInfo<'_>) -> bool {
    parts.iter().all(|comp| match comp {
        Component::ExplicitUniversalType
        | Component::ExplicitAnyNamespace
        | Component::ExplicitNoNamespace => true,
        Component::LocalName(name) => {
            let n: &str = name.lower_name.as_ref();
            info.tags.iter().any(|t| *t == n)
        }
        Component::ID(id) => info.id == id.0.as_ref(),
        Component::Class(name) => info.classes.contains(name.0.as_ref()),
        Component::Root => true,
        Component::Negation(inner) => !inner
            .iter()
            .any(|s| compound_matches(s, info)),
        // 其余（属性选择器/:is/:where/:nth 等）第一阶段不匹配。
        _ => false,
    })
}

fn compound_matches_node(parts: &[&Component<'static>], node: &SelectorNode<'_>) -> bool {
    let classes: ahash::HashSet<&str> = node.classes.split_whitespace().collect();
    let info = SubjectInfo {
        tags: vec![node.tag],
        id: node.id,
        classes: &classes,
    };
    compound_matches_parts(parts, &info)
}

/// :not() 内部也只支持同样的单 compound 语义。
fn compound_matches(selector: &Selector<'static>, info: &SubjectInfo<'_>) -> bool {
    selector
        .iter_raw_match_order()
        .all(|comp| match comp {
            Component::ExplicitUniversalType => true,
            Component::LocalName(name) => {
                let n: &str = name.lower_name.as_ref();
                info.tags.iter().any(|t| *t == n)
            }
            Component::ID(id) => info.id == id.0.as_ref(),
            Component::Class(name) => info.classes.contains(name.0.as_ref()),
            _ => false,
        })
}
