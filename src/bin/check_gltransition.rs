use opencat::{Composition, RenderSession, ScriptDriver, parse_file, render_frame_rgba};

fn main() -> anyhow::Result<()> {
    let jsonl_path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("jsonl_path required"))?;

    let parsed = parse_file(&jsonl_path)?;
    let root = if let Some(script) = parsed.script.as_deref() {
        if script.trim().is_empty() {
            parsed.root
        } else {
            parsed
                .root
                .script_driver(ScriptDriver::from_source(script)?)
        }
    } else {
        parsed.root
    };

    let composition = Composition::new("check_gltransition")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .frames(parsed.frames as u32)
        .audio_sources(parsed.audio_sources.clone())
        .root(move |_ctx| root.clone())
        .build()?;

    let mut session = RenderSession::new();
    let mut frame = 0;
    while frame < composition.frames {
        let _ = render_frame_rgba(&composition, frame, &mut session)?;
        frame += 1;
    }
    Ok(())
}
