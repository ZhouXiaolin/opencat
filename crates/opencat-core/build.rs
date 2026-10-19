use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const LUCIDE_ICONS_DIR: &str = "lucide";

fn main() {
    if let Err(error) = try_main() {
        panic!("failed to generate lucide icon code: {error}");
    }
}

fn try_main() -> Result<(), String> {
    println!("cargo:rerun-if-changed={LUCIDE_ICONS_DIR}");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").map_err(|error| error.to_string())?);

    generate_lucide_icons(&out_dir)?;

    Ok(())
}

// ── Lucide icon codegen ──────────────────────────────────────────────────────

struct LucideIcon {
    name: String,
    paths: Vec<String>,
}

fn generate_lucide_icons(out_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(LUCIDE_ICONS_DIR)
        .map_err(|error| format!("failed to read lucide dir: {error}"))?;
    let mut icons: Vec<LucideIcon> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read dir entry: {error}"))?;
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "svg") {
            continue;
        }
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let paths = extract_svg_paths(&content, &name)?;
        icons.push(LucideIcon { name, paths });
    }
    icons.sort_by(|a, b| a.name.cmp(&b.name));

    let mut output = String::new();
    output.push_str("/// Auto-generated Lucide icon path data.\n");
    output.push_str("/// Each icon maps to a list of SVG path data strings.\n");
    output.push_str("pub fn lucide_icon_paths(name: &str) -> Option<&'static [&'static str]> {\n");
    output.push_str("    match name {\n");
    for icon in &icons {
        let paths_literal = icon
            .paths
            .iter()
            .map(|p| format!("\"{}\"", p.escape_default()))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            output,
            "        \"{}\" => Some(&[{}]),",
            icon.name, paths_literal
        );
    }
    output.push_str("        _ => None,\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
    output.push_str("pub fn lucide_icon_names() -> &'static [&'static str] {\n");
    output.push_str("    &[\n");
    for icon in &icons {
        let _ = writeln!(output, "        \"{}\",", icon.name);
    }
    output.push_str("    ]\n");
    output.push_str("}\n");

    fs::write(out_dir.join("lucide_icons.rs"), output)
        .map_err(|error| format!("failed to write lucide_icons.rs: {error}"))?;
    Ok(())
}

fn extract_svg_paths(content: &str, file_name: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('<') && !line.starts_with("</") && !line.starts_with("<svg") {
            if let Some(d) = extract_attr(line, "d") {
                paths.push(d.to_string());
            } else if line.starts_with("<circle") {
                let cx: f32 = extract_attr(line, "cx")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid cx: {e}"))?;
                let cy: f32 = extract_attr(line, "cy")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid cy: {e}"))?;
                let r: f32 = extract_attr(line, "r")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid r: {e}"))?;
                paths.push(format!(
                    "M{cx},{cy} m{neg_r},0 a{r},{r} 0 1,0 {d},0 a{r},{r} 0 1,0 {neg_d},0",
                    d = r * 2.0,
                    neg_d = -r * 2.0,
                    neg_r = -r
                ));
            } else if line.starts_with("<line") {
                let x1: f32 = extract_attr(line, "x1")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid x1: {e}"))?;
                let y1: f32 = extract_attr(line, "y1")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid y1: {e}"))?;
                let x2: f32 = extract_attr(line, "x2")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid x2: {e}"))?;
                let y2: f32 = extract_attr(line, "y2")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid y2: {e}"))?;
                paths.push(format!("M{x1},{y1} L{x2},{y2}"));
            } else if line.starts_with("<rect") {
                let x: f32 = extract_attr(line, "x")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid x: {e}"))?;
                let y: f32 = extract_attr(line, "y")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid y: {e}"))?;
                let w: f32 = extract_attr(line, "width")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid width: {e}"))?;
                let h: f32 = extract_attr(line, "height")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid height: {e}"))?;
                paths.push(format!(
                    "M{x},{y} L{x},{y2} L{x2},{y2} L{x2},{y} Z",
                    x2 = x + w,
                    y2 = y + h
                ));
            } else if line.starts_with("<polyline") || line.starts_with("<polygon") {
                let points_str = extract_attr(line, "points").unwrap_or("");
                let points: Vec<&str> = points_str.split_whitespace().collect();
                if points.len() >= 2 {
                    let mut d = format!("M{}", points[0]);
                    for p in &points[1..] {
                        let _ = write!(d, " L{p}");
                    }
                    if line.starts_with("<polygon") {
                        d.push_str(" Z");
                    }
                    paths.push(d);
                }
            }
        }
    }
    Ok(paths)
}

fn extract_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let prefix = format!("{attr}=\"");
    let start = tag.find(&prefix)?;
    let rest = &tag[start + prefix.len()..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}
