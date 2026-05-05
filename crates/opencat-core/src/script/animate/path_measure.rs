//! Path measuring (length + position+tangent at parameterized t) without skia.
use std::collections::HashMap;
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg, Point};

const ARCLEN_ACCURACY: f64 = 0.25;

pub struct PathMeasureContour {
    pub segments: Vec<PathSeg>,
    pub cumulative: Vec<f64>,
    pub length: f32,
}

impl PathMeasureContour {
    pub(super) fn sample_at_length(&self, length: f64) -> (f32, f32, f32) {
        let idx = match self.cumulative.binary_search_by(|cum| cum.partial_cmp(&length).unwrap_or(std::cmp::Ordering::Equal)) {
            Ok(i) => i, Err(i) => i.min(self.cumulative.len().saturating_sub(1)),
        };
        let prev = if idx == 0 { 0.0 } else { self.cumulative[idx - 1] };
        let local = length - prev;
        let seg = &self.segments[idx];
        let seg_len = seg.arclen(ARCLEN_ACCURACY);
        let local_t = if seg_len > f64::EPSILON { seg.inv_arclen(local, ARCLEN_ACCURACY) } else { 0.0 };
        let p = seg.eval(local_t);
        let d = derivative(seg, local_t);
        let angle = d.y.atan2(d.x).to_degrees() as f32;
        (p.x as f32, p.y as f32, angle)
    }
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
                    if let (Some(prev), Some(first)) = (last_pt, first) {
                        if (prev - first).hypot() > 1e-6 {
                            let seg = PathSeg::Line(kurbo::Line::new(prev, first));
                            current_len += seg.arclen(ARCLEN_ACCURACY);
                            current_cum.push(current_len);
                            current_segments.push(seg);
                        }
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
        if contours.is_empty() { return None; }
        let total = contours.iter().map(|c| c.length).sum();
        Some(Self { contours, total_length: total })
    }

    pub fn sample(&self, t: f32) -> (f32, f32, f32) {
        if self.contours.is_empty() || self.total_length <= 0.0 { return (0.0, 0.0, 0.0); }
        let target = t.clamp(0.0, 1.0) as f64 * self.total_length as f64;
        let mut accumulated = 0.0f64;
        for contour in &self.contours {
            let len = contour.length as f64;
            if accumulated + len >= target {
                let local = (target - accumulated).clamp(0.0, len);
                return contour.sample_at_length(local);
            }
            accumulated += len;
        }
        (0.0, 0.0, 0.0)
    }
}

fn derivative(seg: &PathSeg, t: f64) -> kurbo::Vec2 {
    match seg {
        PathSeg::Line(l) => l.p1 - l.p0,
        PathSeg::Quad(q) => (q.p1 - q.p0) * (2.0 * (1.0 - t)) + (q.p2 - q.p1) * (2.0 * t),
        PathSeg::Cubic(c) => {
            let one_t = 1.0 - t;
            (c.p1 - c.p0) * (3.0 * one_t * one_t)
                + (c.p2 - c.p1) * (6.0 * one_t * t)
                + (c.p3 - c.p2) * (3.0 * t * t)
        }
    }
}

#[derive(Default)]
pub struct PathMeasureState {
    pub next_id: i32,
    pub entries: HashMap<i32, PathMeasureEntry>,
}

impl PathMeasureState {
    pub fn create(&mut self, svg: &str) -> Option<i32> {
        let entry = PathMeasureEntry::from_svg(svg)?;
        let handle = self.next_id; self.next_id += 1;
        self.entries.insert(handle, entry);
        Some(handle)
    }
    pub fn length(&self, handle: i32) -> f32 { self.entries.get(&handle).map(|e| e.total_length).unwrap_or(0.0) }
    pub fn sample(&self, handle: i32, t: f32) -> (f32, f32, f32) { self.entries.get(&handle).map(|e| e.sample(t)).unwrap_or((0.0, 0.0, 0.0)) }
    pub fn dispose(&mut self, handle: i32) { self.entries.remove(&handle); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cubic_bezier_length_matches_skia_old_value() {
        let entry = PathMeasureEntry::from_svg("M100 360 C400 80 880 640 1180 360").expect("parses single cubic");
        assert!(entry.total_length > 1000.0, "len = {}", entry.total_length);
        let (x0, y0, _) = entry.sample(0.0);
        assert!((x0 - 100.0).abs() < 1.0);
        assert!((y0 - 360.0).abs() < 1.0);
        let (x1, y1, _) = entry.sample(1.0);
        assert!((x1 - 1180.0).abs() < 1.0);
        assert!((y1 - 360.0).abs() < 1.0);
    }
    #[test]
    fn multi_contour_endpoints_are_correct() {
        let entry = PathMeasureEntry::from_svg("M0 0 L100 0 M100 100 L200 100").expect("two contours");
        let (x1, y1, _) = entry.sample(1.0);
        assert!((x1 - 200.0).abs() < 1.0);
        assert!((y1 - 100.0).abs() < 1.0);
    }
}
