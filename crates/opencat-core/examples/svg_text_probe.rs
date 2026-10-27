//! SVG text 侦查探针：打印 usvg 解析产物里 Text 节点的 chunk/span 结构，
//! 确认 x/y 列表、rotate、连字在解析层呈现的形态（svg_text 布局器设计输入）。
//!
//! `--flat <svg>`：对 Text 元素打印 usvg flattened() 布局的 path 定位
//! （= resvg 同源布局），与新栈（cosmic-text shape）流内 x 并排对比。

fn walk(group: &usvg::Group, depth: usize) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => walk(g, depth + 1),
            usvg::Node::Text(t) => {
                println!(
                    "TEXT chunks={} rotate={:?} wm={:?}",
                    t.chunks().len(),
                    t.rotate(),
                    t.writing_mode()
                );
                for (ci, c) in t.chunks().iter().enumerate() {
                    println!(
                        "  [chunk {ci}] x={:?} y={:?} anchor={:?} flow={:?} text={:?}",
                        c.x(),
                        c.y(),
                        c.anchor(),
                        c.text_flow(),
                        c.text()
                    );
                    for s in c.spans() {
                        println!(
                            "    [span {}..{}] font={:?} size={} ls={} ws={} fill={:?} stroke={:?} db={:?} ab={:?} shift={:?} tl={:?} deco=u:{:?} o:{:?} t:{:?}",
                            s.start(),
                            s.end(),
                            s.font().families().first(),
                            s.font_size().get(),
                            s.letter_spacing(),
                            s.word_spacing(),
                            s.fill().map(|f| match f.paint() {
                                usvg::Paint::Color(c) => {
                                    format!("#{:02x}{:02x}{:02x}", c.red, c.green, c.blue)
                                }
                                _ => "other".to_string(),
                            }),
                            s.stroke().is_some(),
                            s.dominant_baseline(),
                            s.alignment_baseline(),
                            s.baseline_shift(),
                            s.text_length(),
                            s.decoration().underline().is_some(),
                            s.decoration().overline().is_some(),
                            s.decoration().line_through().is_some(),
                        );
                    }
                }
                let flat = t.flattened();
                let mut n_paths = 0usize;
                for node in flat.children() {
                    if matches!(node, usvg::Node::Path(_)) {
                        n_paths += 1;
                    }
                }
                println!("  flattened paths={n_paths}");
            }
            _ => {}
        }
    }
}

fn walk_flat(group: &usvg::Group, prefix: &str) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => {
                let tf = g.transform();
                println!("  {prefix}group tx={} ty={} tsx={} tsy={}", tf.tx, tf.ty, tf.sx, tf.sy);
                walk_flat(g, prefix);
            }
            usvg::Node::Path(p) => {
                let b = p.data().bounds();
                println!(
                    "  {prefix}path fill={} bounds=({:.3},{:.3})-({:.3},{:.3})",
                    p.fill().map_or("none", |_| "paint"),
                    b.left(), b.top(), b.right(), b.bottom()
                );
            }
            _ => {}
        }
    }
}

fn main() {
    probe_y();
    let mut args = std::env::args().skip(1);
    let flat = args.next().as_deref() == Some("--flat");
    for path in args {
        println!("=== {path}");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                println!("  read error: {e}");
                continue;
            }
        };
        let mut opt = usvg::Options::default();
        opt.fontdb = std::sync::Arc::new({
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            db
        });
        let tree = match usvg::Tree::from_str(&text, &opt) {
            Ok(t) => t,
            Err(e) => {
                println!("  parse error: {e}");
                continue;
            }
        };
        if flat {
            for node in tree.root().children() {
                if let usvg::Node::Text(t) = node {
                    println!("TEXT flattened-paths:");
                    walk_flat(t.flattened(), "");
                }
            }
        } else {
            walk(tree.root(), 0);
        }
    }
}

// 追加：shape "Text" 打印 y 语义与 x 量化差
#[allow(dead_code)]
fn probe_y() {
    // 字体解析分歧侦查：同 "Noto Sans" @400，打 db 命中的 face 文件与实例。
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    for id in db.query(&fontdb::Query {
        families: &[
            fontdb::Family::Name("Noto Sans"),
            fontdb::Family::Serif,
        ],
        weight: fontdb::Weight::NORMAL,
        style: fontdb::Style::Normal,
        stretch: fontdb::Stretch::Normal,
    }) {
        if let Some(src) = db.face_source(id) {
            println!("fontdb hit id={id:?} source={src:?}");
        }
    }
    let style = opencat_core::style::ComputedTextStyle {
        text_px: 26.0,
        font_family: Some("Noto Sans".to_string()),
        ..Default::default()
    };
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let r = opencat_core::text::rasterize_glyphs("Text ", &style, f32::INFINITY, false, false, &db);
    for line in &r.lines {
        println!("\"Text \" line.y={} width={}", line.y, line.width);
        for p in &line.positions {
            println!(
                "  glyph x={} x_precise={}",
                p.x, p.x_precise
            );
        }
    }
}
