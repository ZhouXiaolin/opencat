//! Lint pass for OpenCat markup (`.xml`) sources.
//!
//! Unlike the fail-fast parser in [`crate::parse::markup`], this module collects *all*
//! diagnostics in a single lenient traversal instead of bailing on the first problem.
//! It reuses the same building blocks (script extraction, template expansion, the
//! Tailwind recognizer and the lucide icon table) so the rules never drift from what
//! the renderer actually accepts.
//!
//! Checks performed:
//! - **XML well-formedness** — script island is stripped, templates are expanded, the
//!   remainder is parsed; parse errors are reported with their position.
//! - **id presence** — every element that the markup grammar requires an `id` on
//!   (`div`, `text`, `before`, `after`, `canvas`, `image`, `lottie`, `video`, `icon`,
//!   `path`, `caption`, `tl`, and `<audio>`) must carry a non-empty `id`.
//! - **id uniqueness** — ids must be unique across the whole document (visual + audio).
//! - **lucide icon names** — `<icon icon="…">` must resolve to a known lucide icon
//!   (with `home`/`suitcase` aliases accepted), otherwise a "did you mean …" hint is
//!   produced.
//! - **Tailwind classes** — each whitespace-delimited token in a `class` attribute is
//!   run through the same recognizer the renderer uses; unrecognized tokens are flagged.
//!   Tokens that describe behavior rather than static appearance are rejected outright:
//!   animation/transitions/timing (`animate-*`, `transition`, `transition-*`,
//!   `duration-*`, `ease-*`, `delay-*`) and interaction/scroll state (`cursor-*`,
//!   `pointer-events-*`, `select-*`, `resize*`, `touch-*`, `scroll-*`, `snap-*`,
//!   `overscroll-*`, `scrollbar-*`). OpenCat only renders *static* utilities.
//! - **attributes** — attributes a tag's grammar does not accept are flagged as
//!   ignored noise, and the reserved names the parser hard-rejects (`className`,
//!   `parentId`, `style`) are reported as errors.

use crate::parse::jsonl::tailwind;
use crate::parse::markup;
use crate::resolve::resolve::validate_lucide_icon_name;
use crate::style::NodeStyle;

/// Severity of a [`LintDiagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A hard error: the document will not render as authored.
    Error,
    /// A soft warning: the document renders, but probably not as intended.
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// A single lint finding for one document.
#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub severity: Severity,
    /// 1-based line in the source document (best effort; `None` when unknown).
    pub line: Option<u32>,
    /// 1-based column in the source document (best effort; `None` when unknown).
    pub col: Option<u32>,
    /// Human-readable description, including suggestions where applicable.
    pub message: String,
}

impl LintDiagnostic {
    fn error(line: Option<u32>, col: Option<u32>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            line,
            col,
            message: message.into(),
        }
    }

    fn warning(line: Option<u32>, col: Option<u32>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            line,
            col,
            message: message.into(),
        }
    }
}

/// Tags that the markup grammar requires to carry a non-empty `id`.
const ID_BEARING_TAGS: &[&str] = &[
    "div", "text", "before", "after", "canvas", "image", "lottie", "video", "icon", "path",
    "caption", "tl", "audio",
];

/// Tailwind exact class names OpenCat forbids. OpenCat only renders *static* utilities;
/// anything that describes behavior — animation/transitions/timing, or interaction/scroll
/// state — has no effect at render time and is routed through `<tl>`/`<transition>`/
/// `<script>` instead.
const FORBIDDEN_TAILWIND_EXACT: &[&str] = &["transition", "resize"];
/// Tailwind class name *prefixes* OpenCat forbids (same rationale as the exact names).
const FORBIDDEN_TAILWIND_PREFIXES: &[&str] = &[
    // animation / transitions / timing
    "animate-",
    "transition-",
    "duration-",
    "ease-",
    "delay-",
    // interaction / pointer state (no DOM at render time)
    "cursor-",
    "pointer-events-",
    "select-",
    "resize-",
    "touch-",
    // scroll behavior
    "scroll-",
    "snap-",
    "overscroll-",
    "scrollbar-",
];

fn is_forbidden_tailwind_class(class: &str) -> bool {
    FORBIDDEN_TAILWIND_EXACT.contains(&class)
        || FORBIDDEN_TAILWIND_PREFIXES
            .iter()
            .any(|prefix| class.starts_with(prefix))
}

/// Run the lint pass against a markup source string.
///
/// The traversal is lenient: a single bad element never short-circuits the rest of
/// the document. Only an unrecoverable XML parse error stops further checks (there
/// is no tree left to walk).
pub fn lint_markup(input: &str) -> Vec<LintDiagnostic> {
    let mut diags = Vec::new();

    // 1) Strip the <script> island + expand <template>/<slot>/$var, exactly like the
    //    real parser, so positions and tag checks line up with rendering.
    let prepared = match prepare_markup(input) {
        Ok(xml) => xml,
        Err(message) => {
            diags.push(LintDiagnostic::error(
                None,
                None,
                format!("invalid XML: {message}"),
            ));
            return diags;
        }
    };

    // 2) Parse the expanded XML.
    let doc = match roxmltree::Document::parse(&prepared) {
        Ok(doc) => doc,
        Err(error) => {
            // roxmltree's Error Display already carries a position for most variants.
            diags.push(LintDiagnostic::error(
                None,
                None,
                format!("invalid XML: {error}"),
            ));
            return diags;
        }
    };

    let root = doc.root_element();
    if root.tag_name().name() != "opencat" {
        let (line, col) = line_col_of(&doc, root.range().start);
        diags.push(LintDiagnostic::error(
            line,
            col,
            format!(
                "markup document root must be <opencat>, found <{}>",
                root.tag_name().name()
            ),
        ));
        return diags;
    }

    // 3) Walk the tree collecting id / lucide / tailwind / attribute diagnostics.
    let mut seen_ids: Vec<(String, Option<(u32, u32)>)> = Vec::new();
    for node in root.descendants() {
        if !node.is_element() {
            continue;
        }
        lint_element(&doc, node, &mut seen_ids, &mut diags);
    }

    // 4) Report duplicates (every id seen more than once).
    report_duplicate_ids(&seen_ids, &mut diags);

    diags
}

/// Strip the script island and expand templates, returning the XML ready for parsing.
/// Factored out so the error path can surface a readable message.
fn prepare_markup(input: &str) -> Result<String, String> {
    let extracted = markup::extract_raw_script(input).map_err(|e| e.to_string())?;
    markup::expand_markup_templates(&extracted.xml).map_err(|e| e.to_string())
}

fn lint_element(
    doc: &roxmltree::Document<'_>,
    node: roxmltree::Node<'_, '_>,
    seen_ids: &mut Vec<(String, Option<(u32, u32)>)>,
    diags: &mut Vec<LintDiagnostic>,
) {
    let tag = node.tag_name().name();

    // id presence + uniqueness tracking.
    if ID_BEARING_TAGS.contains(&tag) {
        let (line, col) = line_col_of(doc, node.range().start);
        match node.attribute("id") {
            None => diags.push(LintDiagnostic::error(
                line,
                col,
                format!("<{tag}> is missing the required `id` attribute"),
            )),
            Some("") => diags.push(LintDiagnostic::error(
                line,
                col,
                format!("<{tag}> has an empty `id` attribute"),
            )),
            Some(id) => seen_ids.push((id.to_string(), pack_pos(line, col))),
        }
    }

    // lucide icon name validation.
    if tag == "icon"
        && let Some(icon) = node.attribute("icon")
    {
        let (line, col) = line_col_of(doc, node.range().start);
        if icon.is_empty() {
            diags.push(LintDiagnostic::error(
                line,
                col,
                "<icon> has an empty `icon` attribute",
            ));
        } else if let Err(hint) = validate_lucide_icon_name(icon) {
            diags.push(LintDiagnostic::error(
                line,
                col,
                format!("unknown lucide icon `{icon}`: {hint}"),
            ));
        }
    }

    // tailwind class validation.
    if let Some(class) = node.attribute("class")
        && !class.is_empty()
    {
        let (line, col) = line_col_of(doc, node.range().start);
        lint_tailwind_classes(class, line, col, diags);
    }

    // unknown / forbidden markup attributes.
    if let Some(allowed) = markup::allowed_attributes(tag) {
        let (line, col) = line_col_of(doc, node.range().start);
        for attr in node.attributes() {
            let name = attr.name();
            if markup::FORBIDDEN_MARKUP_ATTRS.contains(&name) {
                diags.push(LintDiagnostic::error(
                    line,
                    col,
                    format!("<{tag}>: attribute `{name}` is not allowed in markup"),
                ));
            } else if !allowed.contains(&name) {
                diags.push(LintDiagnostic::warning(
                    line,
                    col,
                    format!("<{tag}>: attribute `{name}` is not recognized and will be ignored"),
                ));
            }
        }
    }
}

/// Run every whitespace-delimited class token through the real recognizer and flag
/// the ones that don't match any known rule. Animation/transition/timing utilities
/// are rejected outright (see [`is_forbidden_tailwind_class`]).
fn lint_tailwind_classes(
    class: &str,
    line: Option<u32>,
    col: Option<u32>,
    diags: &mut Vec<LintDiagnostic>,
) {
    for token in class.split_whitespace() {
        if is_forbidden_tailwind_class(token) {
            diags.push(LintDiagnostic::error(
                line,
                col,
                format!(
                    "forbidden Tailwind class `{token}`: OpenCat only consumes static utilities; use `<tl>`/`<transition>`/`<script>` for animation and timing"
                ),
            ));
            continue;
        }
        let mut sink = NodeStyle::default();
        if tailwind::parse_single_class(token, &mut sink) {
            continue;
        }
        diags.push(LintDiagnostic::warning(
            line,
            col,
            format!("unrecognized Tailwind class `{token}` (ignored at render time)"),
        ));
    }
}

/// Re-pack two optional line/col values into the tuple form stored alongside ids.
fn pack_pos(line: Option<u32>, col: Option<u32>) -> Option<(u32, u32)> {
    match (line, col) {
        (Some(row), Some(col)) => Some((row, col)),
        _ => None,
    }
}

/// Convert the first duplicate occurrence of each id into a diagnostic.
fn report_duplicate_ids(seen: &[(String, Option<(u32, u32)>)], diags: &mut Vec<LintDiagnostic>) {
    // Group by id preserving first-seen order for stable output.
    let mut first: std::collections::HashMap<&str, (usize, Option<(u32, u32)>)> =
        std::collections::HashMap::new();
    let mut reported: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for (idx, (id, pos)) in seen.iter().enumerate() {
        match first.get(id.as_str()) {
            None => {
                first.insert(id.as_str(), (idx, *pos));
            }
            Some((_, first_pos)) if !reported.contains(id.as_str()) => {
                reported.insert(id.as_str());
                let (line, col) = unpack_pos(*pos);
                diags.push(LintDiagnostic::error(
                    line,
                    col,
                    format!(
                        "duplicate id `{id}` (first seen at {})",
                        format_pos(*first_pos),
                    ),
                ));
            }
            Some(_) => {
                // Additional duplicates beyond the second: keep the first dup report only.
            }
        }
    }
}

/// Translate a byte offset into a pair of 1-based (row, col) options.
fn line_col_of(doc: &roxmltree::Document<'_>, offset: usize) -> (Option<u32>, Option<u32>) {
    let pos = doc.text_pos_at(offset);
    (Some(pos.row), Some(pos.col))
}

/// Split a stored (row, col) tuple back into the optional pair form used by diagnostics.
fn unpack_pos(pos: Option<(u32, u32)>) -> (Option<u32>, Option<u32>) {
    match pos {
        Some((row, col)) => (Some(row), Some(col)),
        None => (None, None),
    }
}

fn format_pos(pos: Option<(u32, u32)>) -> String {
    match pos {
        Some((row, col)) => format!("{row}:{col}"),
        None => "unknown position".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::lint_markup;
    use crate::parse::lint::Severity;

    fn error_messages(input: &str) -> Vec<String> {
        lint_markup(input)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message)
            .collect()
    }

    fn warning_messages(input: &str) -> Vec<String> {
        lint_markup(input)
            .into_iter()
            .filter(|d| d.severity == Severity::Warning)
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn clean_markup_has_no_diagnostics() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root" class="flex items-center justify-center">
    <text id="title" class="text-[32px] text-slate-900">Hi</text>
    <icon id="play" icon="play" class="w-[24px] h-[24px]" />
  </div>
</opencat>"#;
        assert!(
            lint_markup(xml).is_empty(),
            "expected no diagnostics, got: {:?}",
            lint_markup(xml)
        );
    }

    #[test]
    fn flags_missing_id() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root"><text class="text-[12px]">x</text></div>
</opencat>"#;
        let errs = error_messages(xml);
        assert!(
            errs.iter()
                .any(|m| m.contains("missing the required `id`") && m.contains("<text>")),
            "missing-id diagnostic not found: {errs:?}",
        );
    }

    #[test]
    fn flags_empty_id() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id=""><text id="t">x</text></div>
</opencat>"#;
        let errs = error_messages(xml);
        assert!(
            errs.iter()
                .any(|m| m.contains("empty `id`") && m.contains("<div>")),
            "empty-id diagnostic not found: {errs:?}",
        );
    }

    #[test]
    fn flags_duplicate_id() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="dup"><text id="dup">x</text></div>
</opencat>"#;
        let errs = error_messages(xml);
        assert!(
            errs.iter().any(|m| m.contains("duplicate id `dup`")),
            "duplicate-id diagnostic not found: {errs:?}",
        );
    }

    #[test]
    fn flags_unknown_lucide_icon_with_suggestion() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root"><icon id="i" icon="pla" /></div>
</opencat>"#;
        let errs = error_messages(xml);
        assert!(
            errs.iter()
                .any(|m| m.contains("unknown lucide icon `pla`") && m.contains("did you mean")),
            "lucide diagnostic not found: {errs:?}",
        );
    }

    #[test]
    fn accepts_lucide_aliases() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root"><icon id="h" icon="home" /><icon id="s" icon="suitcase" /></div>
</opencat>"#;
        assert!(
            error_messages(xml).is_empty(),
            "home/suitcase aliases should be accepted, got: {:?}",
            error_messages(xml),
        );
    }

    #[test]
    fn flags_unknown_tailwind_class_as_warning() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root" class="flex not-a-real-class">
    <text id="t" class="text-[12px] bogus-token">x</text>
  </div>
</opencat>"#;
        let warns = warning_messages(xml);
        assert!(
            warns.iter().any(|m| m.contains("not-a-real-class")),
            "unknown class not flagged: {warns:?}",
        );
        assert!(
            warns.iter().any(|m| m.contains("bogus-token")),
            "unknown class not flagged: {warns:?}",
        );
        // unknown classes are warnings, not errors
        assert!(error_messages(xml).is_empty(), "unexpected errors");
    }

    #[test]
    fn flags_forbidden_tailwind_animation_classes() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root" class="animate-spin transition duration-300 ease-in delay-100" />
</opencat>"#;
        let errs = error_messages(xml);
        for forbidden in [
            "`animate-spin`",
            "`transition`",
            "`duration-300`",
            "`ease-in`",
            "`delay-100`",
        ] {
            assert!(
                errs.iter().any(|m| m.contains(forbidden)),
                "forbidden class {forbidden} not flagged: {errs:?}",
            );
        }
        assert!(
            warning_messages(xml).is_empty(),
            "forbidden classes must be errors, not warnings: {:?}",
            warning_messages(xml),
        );
    }

    #[test]
    fn flags_forbidden_interaction_and_scroll_classes() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root" class="cursor-pointer select-none touch-none scroll-smooth snap-mandatory overscroll-none scrollbar-hide" />
</opencat>"#;
        let errs = error_messages(xml);
        for forbidden in [
            "`cursor-pointer`",
            "`select-none`",
            "`touch-none`",
            "`scroll-smooth`",
            "`snap-mandatory`",
            "`overscroll-none`",
            "`scrollbar-hide`",
        ] {
            assert!(
                errs.iter().any(|m| m.contains(forbidden)),
                "forbidden class {forbidden} not flagged: {errs:?}",
            );
        }
    }

    #[test]
    fn forbidden_tailwind_prefix_still_flags_arbitrary_value() {
        // `animate-[...]` and `ease-[...]` are arbitrary-value forms that the
        // recognizer would otherwise ignore; the prefix ban must still catch them.
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root" class="animate-[wiggle] ease-[cubic-bezier(0,0,1,1)]" />
</opencat>"#;
        let errs = error_messages(xml);
        assert!(
            errs.iter().any(|m| m.contains("`animate-[wiggle]`")),
            "{errs:?}",
        );
        assert!(
            errs.iter()
                .any(|m| m.contains("ease-[cubic-bezier(0,0,1,1)]")),
            "{errs:?}",
        );
    }

    #[test]
    fn flags_unknown_attribute_as_warning() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root" foo="bar" />
</opencat>"#;
        let warns = warning_messages(xml);
        assert!(
            warns
                .iter()
                .any(|m| m.contains("`foo`") && m.contains("<div>")),
            "unknown attribute not flagged: {warns:?}",
        );
        assert!(
            error_messages(xml).is_empty(),
            "unknown attributes should be warnings, got: {:?}",
            error_messages(xml),
        );
    }

    #[test]
    fn accepts_pseudo_text_attributes_and_text_shadow_classes() {
        let xml = r##"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root">
    <text id="hero" data-text="Transform" class="[text-shadow:0_0_10px_rgba(0,255,136,0.3)]">
      Transform
      <before id="hero-before" content="attr(data-text)" class="[text-shadow:-1px_0_#ff00ff]" />
      <after id="hero-after" content="attr(data-text)" class="[text-shadow:-1px_0_#00d4ff]" />
    </text>
  </div>
</opencat>"##;

        assert!(
            error_messages(xml).is_empty(),
            "pseudo text should not produce lint errors: {:?}",
            error_messages(xml)
        );
        assert!(
            warning_messages(xml).is_empty(),
            "pseudo text should not produce lint warnings: {:?}",
            warning_messages(xml)
        );
    }

    #[test]
    fn flags_forbidden_markup_attribute_as_error() {
        let xml = r#"<opencat width="320" height="240" fps="30" duration="1">
  <div id="root" className="x" />
</opencat>"#;
        let errs = error_messages(xml);
        assert!(
            errs.iter()
                .any(|m| m.contains("className") && m.contains("not allowed")),
            "forbidden markup attribute not flagged: {errs:?}",
        );
    }

    #[test]
    fn flags_malformed_xml() {
        let xml = "<opencat><div id=\"root\"></opencat>"; // unclosed <div>
        let diags = lint_markup(xml);
        assert!(
            diags.iter().any(|d| d.message.contains("invalid XML")),
            "expected an XML parse error, got: {diags:?}",
        );
    }

    #[test]
    fn flags_non_opencat_root() {
        let xml = "<div id=\"root\" />";
        let diags = lint_markup(xml);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("root must be <opencat>")),
            "expected a root tag diagnostic, got: {diags:?}",
        );
    }

    /// 端到端回归：nexus7-cyberpunk.xml 用了大量此前不被支持的任意值类
    /// （`bg-[linear-gradient(...)]`、`bg-[length:...]`、`bg-[repeating-linear-gradient(...)]`、
    /// `bg-[radial-gradient(...)]`、`[text-shadow:...]`、`h-[N%]`、多层 `shadow-[...]`）。
    /// 此前渲染会为每一个打印 "Unsupported Tailwind class" 警告；现在应全部被识别，
    /// 不再有任何 "unrecognized Tailwind class" 警告。
    #[test]
    fn nexus7_cyberpunk_has_no_unrecognized_tailwind_classes() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root");
        let xml = std::fs::read_to_string(repo.join("examples/nexus7-cyberpunk.xml"))
            .expect("nexus7-cyberpunk.xml should be readable");

        let unrecognized: Vec<String> = warning_messages(&xml)
            .into_iter()
            .filter(|m| m.contains("unrecognized Tailwind class"))
            .collect();
        assert!(
            unrecognized.is_empty(),
            "nexus7-cyberpunk.xml still has unsupported Tailwind classes:\n{}",
            unrecognized.join("\n")
        );
    }
}
