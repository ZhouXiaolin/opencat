//! 脚本侧 DOM 覆盖层：动态节点 + 静态节点的子级视图。
//!
//! 组合的树是声明式的——JSX 决定节点与父子关系，脚本不改结构。但官方
//! 插件要建节点（SplitText 的 char/word/line wrapper）并搬动子级。这里
//! 记录这些改动，渲染面按覆盖层读"这一帧的子级视图"。
//!
//! ## 两级生命周期
//!
//! **节点身份是 realm 级的**。`document.createElement` 的产物被插件当模块级
//! 草稿元素复用（TextPlugin 的 `_tempDiv` 只建一次），若每帧清空节点表，
//! 第二帧起那个 id 就指向不存在的节点，`innerHTML` 赋值静默丢失。所以节点
//! 记录（类型/tag/文本）与 id 计数器跨帧保留；id 从不复用，JS 手里的旧句柄
//! 永远指向同一个记录。
//!
//! **结构是帧级的**。父子视图与静态节点的子级覆盖每帧从声明树重建：脚本每
//! 帧重跑（`useGSAP` 语义），SplitText 每帧先 `revert()` 再重新拆分，声明树
//! 不参与，所以不存在跨帧残留。上一帧挂在树上的动态节点只是变成游离节点。
//!
//! ## 渲染面
//!
//! 静态节点没被改过时没有覆盖条目，视图按"声明文本物化为一个文本节点 +
//! 声明子级"惰性生成。改过之后覆盖条目就是权威视图。

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayNodeType {
    Element,
    Text,
}

impl OverlayNodeType {
    /// DOM `nodeType`（1 = element, 3 = text）。
    pub fn dom_node_type(self) -> u8 {
        match self {
            Self::Element => 1,
            Self::Text => 3,
        }
    }
}

/// 一个动态节点。`tag` 为小写（文本节点是 `#text`）。
#[derive(Debug, Clone)]
pub struct OverlayNode {
    pub node_type: OverlayNodeType,
    pub tag: String,
    pub text: String,
    /// `element.className`。动态节点没有 Tailwind 声明，写进来的就是权威值。
    pub class_name: String,
}

/// 静态节点的一条声明事实（由 resolve 的 target registry 提供）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredNode {
    pub id: String,
    /// 父节点 id，根为空串。
    pub parent: String,
    /// 文档序（DFS 前序的全局递增序号），同父内相对顺序即文档顺序。
    pub order: usize,
    /// 小写 tag（`div` / `text` / `path` …）。
    pub tag: String,
    /// 该节点声明的文本内容（`<text>`），无文本为空串。
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct TreeOverlay {
    /// realm 级：动态节点记录（id 从不复用）。
    nodes: HashMap<String, OverlayNode>,
    next_id: u32,
    /// realm 级：静态节点 → 物化出的文本节点 id（`<text>` 的文本子节点）。
    materialized: HashMap<String, String>,
    /// 帧级：静态节点的声明事实。
    static_children: HashMap<String, Vec<String>>,
    static_text: HashMap<String, String>,
    static_parent: HashMap<String, String>,
    static_tags: HashMap<String, String>,
    static_class: HashMap<String, String>,
    static_ids: std::collections::HashSet<String>,
    /// 帧级：子级视图覆盖（只有被脚本改过的节点才有条目）。
    overrides: HashMap<String, Vec<String>>,
    /// 帧级：子 → 父，随视图维护。
    parents: HashMap<String, String>,
}

impl TreeOverlay {
    /// 开始新的一帧：清掉帧级结构，装入本帧声明树。
    pub fn begin_frame(&mut self, declared: &[DeclaredNode]) {
        self.static_children.clear();
        self.static_text.clear();
        self.static_parent.clear();
        self.static_tags.clear();
        self.static_class.clear();
        self.static_ids.clear();
        self.overrides.clear();
        self.parents.clear();

        let mut ordered: Vec<&DeclaredNode> = declared.iter().collect();
        ordered.sort_by_key(|n| n.order);
        for node in ordered {
            self.static_ids.insert(node.id.clone());
            self.static_tags.insert(node.id.clone(), node.tag.clone());
            if !node.parent.is_empty() {
                self.static_parent
                    .insert(node.id.clone(), node.parent.clone());
                self.static_children
                    .entry(node.parent.clone())
                    .or_default()
                    .push(node.id.clone());
            }
            if !node.text.is_empty() {
                self.static_text.insert(node.id.clone(), node.text.clone());
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    pub fn knows(&self, id: &str) -> bool {
        self.nodes.contains_key(id) || self.static_ids.contains(id)
    }

    pub fn is_static(&self, id: &str) -> bool {
        self.static_ids.contains(id)
    }

    pub fn is_dynamic(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    /// 节点 tag：动态节点查自身，静态节点查声明。
    pub fn tag_of(&self, id: &str) -> Option<&str> {
        if let Some(node) = self.nodes.get(id) {
            return Some(node.tag.as_str());
        }
        self.static_tags.get(id).map(String::as_str)
    }

    pub fn node_type_of(&self, id: &str) -> OverlayNodeType {
        self.nodes
            .get(id)
            .map(|n| n.node_type)
            .unwrap_or(OverlayNodeType::Element)
    }

    fn alloc_id(&mut self) -> String {
        let id = format!("oc-el-{}", self.next_id);
        self.next_id += 1;
        id
    }

    fn alloc_text(&mut self, text: &str) -> String {
        let id = self.alloc_id();
        self.nodes.insert(
            id.clone(),
            OverlayNode {
                node_type: OverlayNodeType::Text,
                tag: "#text".to_string(),
                text: text.to_string(),
                class_name: String::new(),
            },
        );
        id
    }

    pub fn create_element(&mut self, tag: &str) -> String {
        let id = self.alloc_id();
        self.nodes.insert(
            id.clone(),
            OverlayNode {
                node_type: OverlayNodeType::Element,
                tag: tag.to_ascii_lowercase(),
                text: String::new(),
                class_name: String::new(),
            },
        );
        id
    }

    /// `element.className`：动态节点读自身记录，静态节点读本帧写入（未写为空）。
    pub fn class_name_of(&self, id: &str) -> String {
        if let Some(node) = self.nodes.get(id) {
            return node.class_name.clone();
        }
        self.static_class.get(id).cloned().unwrap_or_default()
    }

    pub fn set_class_name(&mut self, id: &str, class_name: &str) -> bool {
        if let Some(node) = self.nodes.get_mut(id) {
            node.class_name = class_name.to_string();
            return true;
        }
        if self.static_ids.contains(id) {
            self.static_class.insert(id.to_string(), class_name.to_string());
            return true;
        }
        false
    }

    pub fn create_text_node(&mut self, text: &str) -> String {
        self.alloc_text(text)
    }

    /// 默认视图：声明文本（物化为文本节点）+ 声明子级。
    ///
    /// 物化出的文本节点按静态节点 id 缓存（realm 级），一帧里反复读
    /// `childNodes` 不会重复分配；跨帧复用同一个 id，文本变了就地更新。
    fn ensure_view(&mut self, id: &str) {
        if self.overrides.contains_key(id) || !self.static_ids.contains(id) {
            return;
        }
        let mut view: Vec<String> = Vec::new();
        if let Some(text) = self.static_text.get(id).cloned() {
            let child = match self.materialized.get(id) {
                Some(existing) => {
                    if let Some(node) = self.nodes.get_mut(existing) {
                        node.text = text;
                    }
                    existing.clone()
                }
                None => {
                    let created = self.alloc_text(&text);
                    self.materialized.insert(id.to_string(), created.clone());
                    created
                }
            };
            self.parents.insert(child.clone(), id.to_string());
            view.push(child);
        }
        if let Some(children) = self.static_children.get(id).cloned() {
            for child in children {
                self.parents.insert(child.clone(), id.to_string());
                view.push(child);
            }
        }
        self.overrides.insert(id.to_string(), view);
    }

    fn view_mut(&mut self, id: &str) -> &mut Vec<String> {
        self.ensure_view(id);
        self.overrides.entry(id.to_string()).or_default()
    }

    /// 子级视图，按文档序：`(id, nodeType)`。
    pub fn children(&mut self, id: &str) -> Vec<(String, u8)> {
        let view = self.view_mut(id).clone();
        view.into_iter()
            .map(|child| {
                let node_type = self.node_type_of(&child).dom_node_type();
                (child, node_type)
            })
            .collect()
    }

    pub fn parent_of(&self, id: &str) -> Option<String> {
        self.parents
            .get(id)
            .cloned()
            .or_else(|| self.static_parent.get(id).cloned())
    }

    /// `(previousSibling, nextSibling)`。
    pub fn siblings(&mut self, id: &str) -> (Option<String>, Option<String>) {
        let Some(parent) = self.parent_of(id) else {
            return (None, None);
        };
        let view = self.view_mut(&parent).clone();
        let Some(index) = view.iter().position(|child| child == id) else {
            return (None, None);
        };
        let prev = index.checked_sub(1).map(|i| view[i].clone());
        let next = view.get(index + 1).cloned();
        (prev, next)
    }

    /// 文本内容：文本节点读自身，元素读子级拼接。
    pub fn text_of(&mut self, id: &str) -> String {
        if let Some(node) = self.nodes.get(id)
            && node.node_type == OverlayNodeType::Text
        {
            return node.text.clone();
        }
        let mut out = String::new();
        for child in self.view_mut(id).clone() {
            out.push_str(&self.text_of(&child));
        }
        out
    }

    fn unlink(&mut self, id: &str) {
        if let Some(old) = self.parents.remove(id) {
            let view = self.view_mut(&old);
            view.retain(|c| c != id);
        }
    }

    /// `parent.appendChild(child)`。child 会先从原父级摘除。
    pub fn append_child(&mut self, parent: &str, child: &str) -> bool {
        if parent == child || !self.knows(parent) || !self.knows(child) {
            return false;
        }
        self.unlink(child);
        self.parents.insert(child.to_string(), parent.to_string());
        self.view_mut(parent).push(child.to_string());
        true
    }

    /// `parent.insertBefore(child, reference)`；`reference == None` 等价 append。
    pub fn insert_before(&mut self, parent: &str, child: &str, reference: Option<&str>) -> bool {
        if parent == child || !self.knows(parent) || !self.knows(child) {
            return false;
        }
        let Some(reference) = reference else {
            return self.append_child(parent, child);
        };
        if reference == child {
            return true;
        }
        self.unlink(child);
        self.parents.insert(child.to_string(), parent.to_string());
        let view = self.view_mut(parent);
        match view.iter().position(|c| c == reference) {
            Some(index) => view.insert(index, child.to_string()),
            // 参考节点不在此父级下：与浏览器一致，按 append 处理。
            None => view.push(child.to_string()),
        }
        true
    }

    pub fn remove_child(&mut self, parent: &str, child: &str) -> bool {
        let view = self.view_mut(parent);
        let before = view.len();
        view.retain(|c| c != child);
        if view.len() == before {
            return false;
        }
        self.parents.remove(child);
        true
    }

    /// `old.replaceWith(...new_children)`（DOM 语义）。
    pub fn replace_with(&mut self, old_child: &str, new_children: &[String]) -> bool {
        let Some(parent) = self.parent_of(old_child) else {
            return false;
        };
        for child in new_children {
            if child != old_child {
                self.unlink(child);
            }
        }
        let view = self.view_mut(&parent);
        let Some(index) = view.iter().position(|c| c == old_child) else {
            return false;
        };
        view.splice(index..index + 1, new_children.iter().cloned());
        self.parents.remove(old_child);
        for child in new_children {
            self.parents.insert(child.clone(), parent.clone());
        }
        true
    }

    pub fn replace_child(&mut self, parent: &str, new_child: &str, old_child: &str) -> bool {
        if self.parent_of(old_child).as_deref() != Some(parent) {
            return false;
        }
        self.replace_with(old_child, &[new_child.to_string()])
    }

    /// `node.remove()`。
    pub fn remove(&mut self, id: &str) -> bool {
        let Some(parent) = self.parent_of(id) else {
            return false;
        };
        self.remove_child(&parent, id)
    }

    /// `element.textContent = text`：清空子级并插入一个文本节点。
    pub fn set_text(&mut self, id: &str, text: &str) -> bool {
        if let Some(node) = self.nodes.get_mut(id)
            && node.node_type == OverlayNodeType::Text
        {
            node.text = text.to_string();
            return true;
        }
        if !self.knows(id) {
            return false;
        }
        for child in std::mem::take(self.view_mut(id)) {
            self.parents.remove(&child);
        }
        if !text.is_empty() {
            let child = self.alloc_text(text);
            self.parents.insert(child.clone(), id.to_string());
            self.view_mut(id).push(child);
        }
        true
    }

    /// `element.normalize()`：合并相邻文本节点、删除空文本节点。
    pub fn normalize(&mut self, id: &str) -> bool {
        if !self.knows(id) {
            return false;
        }
        let view = self.view_mut(id).clone();
        let mut merged: Vec<String> = Vec::with_capacity(view.len());
        for child in view {
            let is_text = self.node_type_of(&child) == OverlayNodeType::Text;
            if is_text {
                let text = self.text_of(&child);
                if text.is_empty() {
                    self.parents.remove(&child);
                    continue;
                }
                if let Some(last) = merged.last()
                    && self.node_type_of(last) == OverlayNodeType::Text
                {
                    let last = last.clone();
                    let combined = format!("{}{}", self.text_of(&last), text);
                    self.set_text(&last, &combined);
                    self.parents.remove(&child);
                    continue;
                }
            }
            merged.push(child);
        }
        self.overrides.insert(id.to_string(), merged);
        true
    }

    /// `cloneNode(deep)`。静态节点以声明事实为模板建动态副本。
    pub fn clone_node(&mut self, id: &str, deep: bool) -> Option<String> {
        if self.node_type_of(id) == OverlayNodeType::Text {
            let text = self.text_of(id);
            return Some(self.alloc_text(&text));
        }
        let tag = self.tag_of(id)?.to_string();
        let new_id = self.create_element(&tag);
        if deep {
            for (child, _) in self.children(id) {
                if let Some(cloned) = self.clone_node(&child, true) {
                    self.append_child(&new_id, &cloned);
                }
            }
        }
        Some(new_id)
    }
}
