//! GLSL-to-SKSL transition conversion for GL Transitions.
//!
//! Reads gltransition.json embedded at compile time, parses GLSL transition
//! shaders, and converts them to Skia SKSL suitable for RuntimeEffect
//! compilation in both the engine (skia-safe) and web (CanvasKit).

use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;

// ── Data structures ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GlTransitionJsonEntry {
    name: String,
    #[serde(default, rename = "defaultParams")]
    default_params: serde_json::Map<String, Value>,
    #[serde(default, rename = "paramsTypes")]
    params_types: serde_json::Map<String, Value>,
    glsl: String,
}

#[derive(Clone)]
struct GlTransitionSource {
    glsl: String,
    default_params: serde_json::Map<String, Value>,
    params_types: serde_json::Map<String, Value>,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Returns the compiled SKSL source for a named GL transition.
///
/// The returned SKSL can be passed directly to `RuntimeEffect::make_for_shader`
/// (in engine) or `CanvasKit.RuntimeEffect.Make()` (in web). It declares
/// `uniform shader fromScene; uniform shader toScene;` as the two input
/// children plus `uniform float progress;` and `uniform float2 resolution;`
/// for the runtime parameters.
pub fn gl_transition_sksl(name: &str) -> Result<String> {
    let source = lookup_source(name)?;
    glsl_to_sksl(&source.glsl, &source.default_params, &source.params_types)
}

/// Normalise a transition name for lookup (lowercase, remove punctuation).
pub fn normalize_gltransition_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|char| *char != '-' && *char != '_' && !char.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Lists all available transition names (normalised, lowercase, no punctuation).
pub fn available_transition_names() -> Vec<&'static str> {
    let Ok(map) = sources_by_name() else {
        return Vec::new();
    };
    let mut names: Vec<&str> = map.keys().map(String::as_str).collect();
    names.sort_unstable();
    names
}

/// Returns the raw GLSL source for a transition (for debugging).
pub fn gl_transition_glsl(name: &str) -> Result<String> {
    Ok(lookup_source(name)?.glsl)
}

// ── Source lookup ────────────────────────────────────────────────────────────

fn lookup_source(name: &str) -> Result<GlTransitionSource> {
    let map = sources_by_name()
        .as_ref()
        .map_err(|error| anyhow!("failed to parse gltransition.json: {error}"))?;
    let key = normalize_gltransition_name(name);
    map.get(&key)
        .cloned()
        .ok_or_else(|| anyhow!("gltransition.json is missing `{name}`"))
}

fn sources_by_name() -> &'static Result<HashMap<String, GlTransitionSource>, String> {
    static GLTRANSITION_JSON: &str = include_str!("../../gltransition.json");
    static SOURCES_BY_NAME: OnceLock<Result<HashMap<String, GlTransitionSource>, String>> =
        OnceLock::new();

    SOURCES_BY_NAME.get_or_init(|| {
        let entries: Vec<GlTransitionJsonEntry> =
            serde_json::from_str(GLTRANSITION_JSON).map_err(|error| error.to_string())?;
        Ok(entries
            .into_iter()
            .map(|entry| {
                (
                    normalize_gltransition_name(&entry.name),
                    GlTransitionSource {
                        glsl: entry.glsl,
                        default_params: entry.default_params,
                        params_types: entry.params_types,
                    },
                )
            })
            .collect())
    })
}

// ── GLSL → SKSL conversion ──────────────────────────────────────────────────

fn glsl_to_sksl(
    glsl: &str,
    default_params: &serde_json::Map<String, Value>,
    params_types: &serde_json::Map<String, Value>,
) -> Result<String> {
    let mut source = expand_defines(glsl);
    source = strip_precision_blocks(&source);
    source = replace_transition_uniforms(&source, default_params, params_types)?;
    source = replace_glsl_types(&source);
    source = source.replace("float2(1.0).xy", "float2(1.0)");
    source = replace_swizzle(&source, "uv.xy", "uv");
    source = replace_swizzle(&source, "p.xy", "p");
    source = inline_global_initializers(&source);

    if !source.contains("transition") {
        return Err(anyhow!(
            "GLTransition source does not define transition(vec2)"
        ));
    }

    Ok(format!(
        r#"
uniform shader fromScene;
uniform shader toScene;
uniform float progress;
uniform float2 resolution;

half4 getFromColor(float2 uv) {{
    return fromScene.eval(uv * resolution);
}}

half4 getToColor(float2 uv) {{
    return toScene.eval(uv * resolution);
}}

const float ratio = 1.0;

{source}

half4 main(float2 coord) {{
    float2 uv = coord / resolution;
    return transition(uv);
}}
"#
    ))
}

// ── Name normalisation ──────────────────────────────────────────────────────

// (see pub fn normalize_gltransition_name above)

// ── #define expansion ───────────────────────────────────────────────────────

fn expand_defines(input: &str) -> String {
    let mut simple_defines: Vec<(String, String)> = Vec::new();
    let mut func_defines: Vec<(String, Vec<String>, String)> = Vec::new();
    let mut lines: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#ifdef")
            || trimmed.starts_with("#endif")
            || trimmed.starts_with("precision ")
        {
            continue;
        }
        if trimmed.starts_with("#define") {
            let rest = trimmed.trim_start_matches("#define").trim();
            if rest.is_empty() {
                continue;
            }
            let name_end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            let name = rest[..name_end].trim();
            if name.is_empty() {
                continue;
            }
            let after_name = rest[name_end..].trim_start();
            if let Some(inner) = after_name.strip_prefix('(')
                && let Some(close) = inner.find(')')
            {
                let params_str = inner[..close].trim();
                let params: Vec<String> = params_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let raw_body = inner[close + 1..].trim();
                let body = raw_body
                    .split_once("//")
                    .map(|(code, _)| code.trim())
                    .unwrap_or(raw_body)
                    .to_string();
                func_defines.push((name.to_string(), params, body));
                continue;
            }
            let value = after_name
                .split_once("//")
                .map(|(code, _)| code.trim())
                .unwrap_or(after_name)
                .to_string();
            simple_defines.push((name.to_string(), value));
            continue;
        }
        lines.push(line.to_string());
    }

    let mut result = lines.join("\n");

    for (name, params, body) in &func_defines {
        result = expand_func_macro(&result, name, params, body);
    }

    for (name, value) in &simple_defines {
        result = replace_word(&result, name, value);
    }

    result
}

fn expand_func_macro(src: &str, name: &str, params: &[String], body: &str) -> String {
    let mut result = String::new();
    let mut i = 0;
    let bytes = src.as_bytes();
    while i < src.len() {
        if let Some(pos) = src[i..].find(name) {
            let abs = i + pos;
            let after_name = abs + name.len();
            let before_ok = abs == 0 || !is_word_char(bytes[abs - 1]);
            if before_ok
                && after_name < src.len()
                && bytes[after_name] == b'('
                && let Some(close) = find_matching_paren(src, after_name)
            {
                let args_str = &src[after_name + 1..close];
                let args = split_args(args_str);
                let mut expanded = body.to_string();
                for (pi, param) in params.iter().enumerate() {
                    if let Some(arg) = args.get(pi) {
                        expanded = replace_word(&expanded, param, arg);
                    }
                }
                result.push_str(&src[i..abs]);
                result.push_str(&expanded);
                i = close + 1;
                continue;
            }
            result.push_str(&src[i..after_name]);
            i = after_name;
        } else {
            result.push_str(&src[i..]);
            break;
        }
    }
    result
}

fn find_matching_paren(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if start >= s.len() || bytes[start] != b'(' {
        return None;
    }
    let mut depth = 1i32;
    let mut i = start + 1;
    while i < s.len() && depth > 0 {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    if depth == 0 { Some(i - 1) } else { None }
}

fn split_args(s: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                args.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        args.push(cur.trim().to_string());
    }
    args
}

// ── Word-based replacement ──────────────────────────────────────────────────

fn replace_word(src: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let mut i = 0;
    let bytes = src.as_bytes();
    while i < src.len() {
        if let Some(pos) = src[i..].find(from) {
            let abs = i + pos;
            let after = abs + from.len();
            let before_ok = abs == 0 || !is_word_char(bytes[abs - 1]);
            let after_ok = after >= src.len() || !is_word_char(bytes[after]);
            if before_ok && after_ok {
                result.push_str(&src[i..abs]);
                result.push_str(to);
                i = after;
            } else {
                let next = abs + 1;
                result.push_str(&src[i..next]);
                i = next;
            }
        } else {
            result.push_str(&src[i..]);
            break;
        }
    }
    result
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ── Swizzle removal ─────────────────────────────────────────────────────────

fn replace_swizzle(src: &str, swizzle: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let mut i = 0;
    let bytes = src.as_bytes();
    while i < src.len() {
        if let Some(pos) = src[i..].find(swizzle) {
            let abs = i + pos;
            let after = abs + swizzle.len();
            let ok = after >= src.len() || !is_swizzle_char(bytes[after]);
            if ok {
                result.push_str(&src[i..abs]);
                result.push_str(replacement);
                i = after;
            } else {
                let next = abs + 1;
                result.push_str(&src[i..next]);
                i = next;
            }
        } else {
            result.push_str(&src[i..]);
            break;
        }
    }
    result
}

fn is_swizzle_char(b: u8) -> bool {
    matches!(b, b'x' | b'y' | b'z' | b'w' | b'r' | b'g' | b'b' | b'a')
}

// ── Precision block stripping ────────────────────────────────────────────────

fn strip_precision_blocks(input: &str) -> String {
    input.replace("GL_ES", "")
}

// ── Uniform → const conversion ──────────────────────────────────────────────

fn replace_transition_uniforms(
    input: &str,
    default_params: &serde_json::Map<String, Value>,
    params_types: &serde_json::Map<String, Value>,
) -> Result<String> {
    for name in default_params.keys() {
        if !params_types.contains_key(name) {
            return Err(anyhow!(
                "GLTransition default parameter `{name}` is missing from paramsTypes"
            ));
        }
    }

    let mut output = String::new();
    let mut extra_params = String::new();
    let mut emitted_params = std::collections::HashSet::new();
    for line in input.lines() {
        let (clean_line, inline_default) = extract_inline_default(line);
        let trimmed = clean_line.trim();
        if !trimmed.starts_with("uniform ") {
            output.push_str(line);
            output.push('\n');
            continue;
        }

        let declaration = trimmed.trim_end_matches(';');
        let mut parts = declaration.split_whitespace();
        let _uniform = parts.next();
        let Some(ty) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };

        if matches!(name, "progress" | "resolution") {
            continue;
        }

        let ty = params_types.get(name).and_then(Value::as_str).unwrap_or(ty);
        let value = if let Some(v) = default_params.get(name) {
            default_param_to_sksl(ty, v).ok_or_else(|| {
                anyhow!("GLTransition parameter `{name}` has unsupported default value `{v}`")
            })?
        } else if let Some(inline_val) = &inline_default {
            let glsl_to_sksl = |s: &str| -> String {
                s.replace("vec2", "float2")
                    .replace("vec3", "float3")
                    .replace("vec4", "float4")
                    .replace("ivec2", "int2")
                    .replace("ivec3", "int3")
                    .replace("ivec4", "int4")
                    .replace("bvec2", "bool2")
                    .replace("bvec3", "bool3")
                    .replace("bvec4", "bool4")
            };
            glsl_to_sksl(inline_val)
        } else {
            return Err(anyhow!(
                "GLTransition parameter `{name}` is missing a default value"
            ));
        };
        emit_const_param(&mut output, ty, name, &value);
        emitted_params.insert(name.to_string());
    }

    for (name, ty) in params_types {
        if emitted_params.contains(name) || matches!(name.as_str(), "progress" | "resolution") {
            continue;
        }
        let Some(ty) = ty.as_str() else {
            continue;
        };
        let value = default_params
            .get(name)
            .ok_or_else(|| anyhow!("GLTransition parameter `{name}` is missing a default value"))?;
        let value = default_param_to_sksl(ty, value).ok_or_else(|| {
            anyhow!("GLTransition parameter `{name}` has unsupported default value `{value}`")
        })?;
        emit_const_param(&mut extra_params, ty, name, &value);
    }

    Ok(format!("{extra_params}{output}"))
}

fn extract_inline_default(line: &str) -> (String, Option<String>) {
    let no_line = line.split_once("//").map(|(code, _)| code).unwrap_or(line);
    let mut result = String::with_capacity(no_line.len());
    let mut default = None;
    let mut chars = no_line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut content = String::new();
            loop {
                match chars.next() {
                    Some('*') if chars.peek() == Some(&'/') => {
                        chars.next();
                        break;
                    }
                    Some(c) => content.push(c),
                    None => break,
                }
            }
            if let Some(eq_pos) = content.find('=') {
                default = Some(content[eq_pos + 1..].trim().to_string());
            }
        } else {
            result.push(c);
        }
    }
    (result, default)
}

fn emit_const_param(output: &mut String, ty: &str, name: &str, value: &str) {
    output.push_str("const ");
    output.push_str(ty);
    output.push(' ');
    output.push_str(name);
    output.push_str(" = ");
    output.push_str(value);
    output.push_str(";\n");
}

// ── Default parameter serialisation ──────────────────────────────────────────

fn default_param_to_sksl(ty: &str, value: &Value) -> Option<String> {
    match value {
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => {
            let literal = number_literal_for_type(ty, value)?;
            Some(match ty {
                "vec2" | "ivec2" => format!("{ty}({literal})"),
                "vec3" => format!("{ty}({literal})"),
                "vec4" => format!("{ty}({literal})"),
                _ => literal,
            })
        }
        Value::Array(values) => {
            let args = values
                .iter()
                .map(|value| default_param_to_sksl(vector_scalar_type(ty), value))
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            Some(format!("{ty}({args})"))
        }
        Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn number_literal_for_type(ty: &str, value: &serde_json::Number) -> Option<String> {
    if matches!(ty, "int" | "ivec2" | "ivec3" | "ivec4") {
        return value
            .as_i64()
            .or_else(|| value.as_f64().map(|value| value.round() as i64))
            .map(|value| value.to_string());
    }
    value.as_f64().map(format_float_literal)
}

fn vector_scalar_type(ty: &str) -> &str {
    match ty {
        "ivec2" | "ivec3" | "ivec4" => "int",
        _ => "float",
    }
}

fn format_float_literal(value: f64) -> String {
    let mut literal = value.to_string();
    if !literal.contains('.') && !literal.contains('e') && !literal.contains('E') {
        literal.push_str(".0");
    }
    literal
}

// ── GLSL type replacement ───────────────────────────────────────────────────

fn replace_glsl_types(input: &str) -> String {
    replace_identifier_tokens(input, |token| match token {
        "ivec2" => Some("int2"),
        "ivec3" => Some("int3"),
        "ivec4" => Some("int4"),
        "vec4" => Some("half4"),
        "vec3" => Some("float3"),
        "vec2" => Some("float2"),
        "mat2" => Some("float2x2"),
        "mat3" => Some("float3x3"),
        _ => None,
    })
}

fn replace_identifier_tokens<F>(input: &str, mut replace: F) -> String
where
    F: FnMut(&str) -> Option<&'static str>,
{
    let mut output = String::with_capacity(input.len());
    let mut token = String::new();

    for char in input.chars() {
        if is_identifier_char(char) {
            token.push(char);
            continue;
        }

        if !token.is_empty() {
            if let Some(replacement) = replace(&token) {
                output.push_str(replacement);
            } else {
                output.push_str(&token);
            }
            token.clear();
        }
        output.push(char);
    }

    if !token.is_empty() {
        if let Some(replacement) = replace(&token) {
            output.push_str(replacement);
        } else {
            output.push_str(&token);
        }
    }

    output
}

fn is_identifier_char(char: char) -> bool {
    char == '_' || char.is_ascii_alphanumeric()
}

// ── Global initialiser inlining ─────────────────────────────────────────────

fn inline_global_initializers(input: &str) -> String {
    let mut hoistable: Vec<(String, String)> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    let mut brace_depth = 0_i32;

    for line in input.lines() {
        if brace_depth == 0
            && let Some(pair) = is_inlineable_global(line.trim())
        {
            hoistable.push(pair);
            brace_depth += line.chars().filter(|c| *c == '{').count() as i32;
            brace_depth -= line.chars().filter(|c| *c == '}').count() as i32;
            continue;
        }
        kept.push(line.to_string());
        brace_depth += line.chars().filter(|c| *c == '{').count() as i32;
        brace_depth -= line.chars().filter(|c| *c == '}').count() as i32;
    }

    if hoistable.is_empty() {
        return input.to_string();
    }

    let mut result = kept.join("\n");
    for (name, expr) in hoistable.into_iter().rev() {
        result = replace_word(&result, &name, &format!("({expr})"));
    }
    result
}

fn is_inlineable_global(trimmed: &str) -> Option<(String, String)> {
    if trimmed.starts_with("const ") || trimmed.starts_with("uniform ") {
        return None;
    }
    if !trimmed.ends_with(';') || !trimmed.contains('=') {
        return None;
    }
    let (left, right) = trimmed.split_once('=')?;
    if left.contains('(') {
        return None;
    }
    let type_name = left.split_whitespace().next()?;
    let valid_types: &[&str] = &[
        "float", "half", "int", "bool", "float2", "float3", "half4", "int2", "int3", "int4",
    ];
    if !valid_types.contains(&type_name) {
        return None;
    }
    let name = left.split_whitespace().nth(1)?.to_string();
    let expr = right.trim_end_matches(';').trim().to_string();
    Some((name, expr))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_removes_punctuation_and_lowercases() {
        assert_eq!(
            normalize_gltransition_name("BowTieHorizontal"),
            "bowtiehorizontal"
        );
        assert_eq!(
            normalize_gltransition_name("bow-tie_horizontal"),
            "bowtiehorizontal"
        );
        assert_eq!(normalize_gltransition_name("  BookFlip  "), "bookflip");
    }

    #[test]
    fn available_names_returns_sorted() {
        let names = available_transition_names();
        assert!(!names.is_empty());
        // Verify sorted
        for w in names.windows(2) {
            assert!(w[0] <= w[1]);
        }
    }

    #[test]
    fn nonexistent_transition_fails() {
        assert!(gl_transition_sksl("NonExistentTransition_123").is_err());
    }

    #[test]
    fn known_transition_produces_sksl() {
        let sksl = gl_transition_sksl("AdvancedMosaic").expect("AdvancedMosaic should exist");
        assert!(sksl.contains("uniform shader fromScene"));
        assert!(sksl.contains("uniform shader toScene"));
        assert!(sksl.contains("uniform float progress"));
        assert!(sksl.contains("uniform float2 resolution"));
        assert!(sksl.contains("half4 main(float2 coord)"));
        assert!(sksl.contains("transition(uv)"));
    }

    #[test]
    fn known_transition_no_redundant_precision() {
        let sksl = gl_transition_sksl("AdvancedMosaic").unwrap();
        assert!(!sksl.contains("precision "));
    }

    #[test]
    fn gl_transition_glsl_returns_raw_glsl() {
        let glsl = gl_transition_glsl("AdvancedMosaic").unwrap();
        assert!(glsl.contains("vec4 transition(vec2 uv)"));
        assert!(glsl.contains("uniform float pixelSize"));
    }

    #[test]
    fn expand_defines_handles_simple_define() {
        let input = "#define PI 3.14159\nfloat x = PI;";
        let result = expand_defines(input);
        assert!(result.contains("3.14159"));
        assert!(!result.contains("#define"));
    }

    #[test]
    fn replace_word_respects_boundaries() {
        let result = replace_word("float x = myVar;", "myVar", "newVar");
        assert_eq!(result, "float x = newVar;");
    }

    #[test]
    fn replace_swizzle_removes_xy() {
        let result = replace_swizzle("uv.xy * 2.0", "uv.xy", "uv");
        assert_eq!(result, "uv * 2.0");
    }

    #[test]
    fn format_float_literal_adds_dot_zero() {
        assert_eq!(format_float_literal(42.0), "42.0");
        assert_eq!(format_float_literal(3.14), "3.14");
    }
}
