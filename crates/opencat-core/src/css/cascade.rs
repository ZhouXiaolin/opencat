//! 选择器匹配与层叠排序。
//!
//! 排序键按 CSS 层叠规范：`!important` > layer > specificity > 源顺序，
//! 全部升序、后者胜。specificity 直接取 selectors crate 的打包值
//! （class/attr 位与元素位不重叠，整数字典序等价于 (a,b,c) 字典序）。
//! unlayered 规则的层号是 usize::MAX，在层键里自然排到最高优先。

use super::stylesheet::CssStylesheet;
use lightningcss::properties::Property;
use lightningcss::selector::{Component, Selector};

/// 排序后的单条声明。
pub struct AppliedDeclaration {
    pub property: Property<'static>,
    pub important: bool,
    pub layer: usize,
    pub specificity: u32,
    pub order: usize,
}

/// 对一个节点收集全部命中声明并按层叠序排列。
pub fn match_rules(
    sheet: &CssStylesheet,
    class_name: &str,
    element_id: &str,
    tag: &str,
) -> Vec<AppliedDeclaration> {
    let classes: ahash::HashSet<&str> = class_name.split_whitespace().collect();
    let mut out: Vec<AppliedDeclaration> = Vec::new();
    for rule in &sheet.rules {
        if !rule
            .selectors
            .iter()
            .any(|sel| selector_matches(sel, &classes, element_id, tag))
        {
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

/// 单 compound 匹配（构建期已过滤组合器与伪元素分支）。
/// 单个类名是否被样式表的任何选择器引用（含复合选择器中的一个类位）。
/// 运行时拼接类在 tailwind 静态扫描下不会产出规则，用这个判定诊断名单。
pub fn class_has_rules(sheet: &CssStylesheet, class: &str) -> bool {
    sheet.rules.iter().any(|rule| {
        rule.selectors.iter().any(|sel| {
            sel.iter_raw_match_order()
                .any(|comp| matches!(comp, Component::Class(name) if name.0.as_ref() == class))
        })
    })
}

fn selector_matches(
    selector: &Selector<'static>,
    classes: &ahash::HashSet<&str>,
    element_id: &str,
    tag: &str,
) -> bool {
    for comp in selector.iter_raw_match_order() {
        let ok = match comp {
            Component::ExplicitUniversalType
            | Component::ExplicitAnyNamespace
            | Component::ExplicitNoNamespace => true,
            Component::LocalName(name) => {
                // 引擎节点名 → HTML 标签名对齐（text 在 DOM 里由 span 承载）。
                let n: &str = name.lower_name.as_ref();
                tag == n || (tag == "text" && n == "span") || (tag == "image" && n == "img")
            }
            Component::ID(id) => element_id == id.0.as_ref(),
            Component::Class(name) => classes.contains(name.0.as_ref()),
            Component::Root => true,
            Component::Negation(inner) => !inner
                .iter()
                .any(|s| compound_matches(s, classes, element_id, tag)),
            // 其余（属性选择器/:is/:where/:nth 等）第一阶段不匹配。
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

/// :not() 内部也只支持同样的单 compound 语义。
fn compound_matches(
    selector: &Selector<'static>,
    classes: &ahash::HashSet<&str>,
    element_id: &str,
    tag: &str,
) -> bool {
    for comp in selector.iter_raw_match_order() {
        let ok = match comp {
            Component::ExplicitUniversalType => true,
            Component::LocalName(name) => tag == name.lower_name.as_ref(),
            Component::ID(id) => element_id == id.0.as_ref(),
            Component::Class(name) => classes.contains(name.0.as_ref()),
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}
