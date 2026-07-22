//! Unit tests for real-pipeline layout inspection (no browser).

use anyhow::Result;
use opencat_core::parse::primitives::{div, text};
use opencat_core::parse::composition::Composition;

use crate::inspect::collect_frame_layout_rects;

#[test]
fn collect_frame_layout_rects_uses_core_pipeline() -> Result<()> {
    let root = div()
        .id("root")
        .w_full()
        .h_full()
        .child(div().id("card").w(100.0).h(50.0).child(text("hi").id("label")));

    let composition = Composition::new("inspect-unit")
        .size(320, 180)
        .fps(30)
        .duration(1.0 / 30.0)
        .root(move |_| root.clone().into())
        .build()?;

    let rects = collect_frame_layout_rects(&composition, 0)?;
    assert!(
        rects.iter().any(|r| r.id == "card"),
        "expected card in {:?}",
        rects.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    let card = rects.iter().find(|r| r.id == "card").unwrap();
    assert!((card.width - 100.0).abs() < 0.5);
    assert!((card.height - 50.0).abs() < 0.5);
    assert!(rects.iter().any(|r| r.draw_order == 0));
    Ok(())
}
