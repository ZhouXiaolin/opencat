//! Draw-script paint state — tracks the current canvas drawing state
//! while interpreting draw-script commands.

use crate::canvas::paint::{PaintSpec, PaintStyle, PathEffectSpec, StrokeCap, StrokeJoin};

/// Mutable drawing state carried through a draw-script execution.
#[derive(Clone, Debug)]
pub struct DrawScriptPaintState {
    pub fill_style: PaintSpec,
    pub stroke_style: PaintSpec,
    pub line_width: f32,
    pub global_alpha: f32,
    pub anti_alias: bool,
    pub line_cap: StrokeCap,
    pub line_join: StrokeJoin,
    pub line_dash: Option<Vec<f32>>,
    pub line_dash_phase: f32,
}

impl Default for DrawScriptPaintState {
    fn default() -> Self {
        Self {
            fill_style: PaintSpec::default(),
            stroke_style: PaintSpec::default(),
            line_width: 1.0,
            global_alpha: 1.0,
            anti_alias: true,
            line_cap: StrokeCap::Butt,
            line_join: StrokeJoin::Miter,
            line_dash: None,
            line_dash_phase: 0.0,
        }
    }
}

impl DrawScriptPaintState {
    pub fn fill_paint_spec(&self) -> PaintSpec {
        let mut spec = self.fill_style.clone();
        spec.style = PaintStyle::Fill;
        spec.anti_alias = self.anti_alias;
        spec
    }

    pub fn stroke_paint_spec(&self) -> PaintSpec {
        let mut spec = self.stroke_style.clone();
        spec.style = PaintStyle::Stroke;
        spec.anti_alias = self.anti_alias;
        let mut s = spec.stroke.unwrap_or_default();
        s.width = self.line_width.max(0.0);
        s.cap = self.line_cap;
        s.join = self.line_join;
        spec.stroke = Some(s);
        if let Some(ref intervals) = self.line_dash {
            spec.path_effect = Some(PathEffectSpec::Dash {
                intervals: intervals.clone(),
                phase: self.line_dash_phase,
            });
        }
        spec
    }
}
