//! Path length measurement without skia (`getTotalLength` / `path_total_length`).
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg, Point};

const ARCLEN_ACCURACY: f64 = 0.25;

pub struct PathMeasureContour {
    pub segments: Vec<PathSeg>,
    pub cumulative: Vec<f64>,
    pub length: f32,
}

pub struct PathMeasureEntry {
    pub contours: Vec<PathMeasureContour>,
    pub total_length: f32,
}

impl PathMeasureEntry {
    pub fn from_svg(svg: &str) -> Option<Self> {
        let path = BezPath::from_svg(svg).ok()?;
        let mut contours: Vec<PathMeasureContour> = Vec::new();
        let mut current_segments: Vec<PathSeg> = Vec::new();
        let mut current_cum: Vec<f64> = Vec::new();
        let mut current_len = 0.0f64;
        let mut last_pt: Option<Point> = None;
        for el in path.elements() {
            match *el {
                kurbo::PathEl::MoveTo(p) => {
                    if !current_segments.is_empty() {
                        contours.push(PathMeasureContour {
                            segments: std::mem::take(&mut current_segments),
                            cumulative: std::mem::take(&mut current_cum),
                            length: current_len as f32,
                        });
                        current_len = 0.0;
                    }
                    last_pt = Some(p);
                }
                kurbo::PathEl::LineTo(p) => {
                    if let Some(prev) = last_pt {
                        let seg = PathSeg::Line(kurbo::Line::new(prev, p));
                        current_len += seg.arclen(ARCLEN_ACCURACY);
                        current_cum.push(current_len);
                        current_segments.push(seg);
                    }
                    last_pt = Some(p);
                }
                kurbo::PathEl::QuadTo(c, p) => {
                    if let Some(prev) = last_pt {
                        let seg = PathSeg::Quad(kurbo::QuadBez::new(prev, c, p));
                        current_len += seg.arclen(ARCLEN_ACCURACY);
                        current_cum.push(current_len);
                        current_segments.push(seg);
                    }
                    last_pt = Some(p);
                }
                kurbo::PathEl::CurveTo(c1, c2, p) => {
                    if let Some(prev) = last_pt {
                        let seg = PathSeg::Cubic(kurbo::CubicBez::new(prev, c1, c2, p));
                        current_len += seg.arclen(ARCLEN_ACCURACY);
                        current_cum.push(current_len);
                        current_segments.push(seg);
                    }
                    last_pt = Some(p);
                }
                kurbo::PathEl::ClosePath => {
                    let first = current_segments.first().map(|s| s.start());
                    if let (Some(prev), Some(first)) = (last_pt, first)
                        && (prev - first).hypot() > 1e-6
                    {
                        let seg = PathSeg::Line(kurbo::Line::new(prev, first));
                        current_len += seg.arclen(ARCLEN_ACCURACY);
                        current_cum.push(current_len);
                        current_segments.push(seg);
                    }
                    last_pt = first;
                }
            }
        }
        if !current_segments.is_empty() {
            contours.push(PathMeasureContour {
                segments: std::mem::take(&mut current_segments),
                cumulative: std::mem::take(&mut current_cum),
                length: current_len as f32,
            });
        }
        if contours.is_empty() {
            return None;
        }
        let total = contours.iter().map(|c| c.length).sum();
        Some(Self {
            contours,
            total_length: total,
        })
    }
}


