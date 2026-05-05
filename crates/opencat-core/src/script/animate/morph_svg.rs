//! SVG path morphing — single-contour to single-contour, closed-to-closed or open-to-open.

use std::collections::HashMap;
use kurbo::{BezPath, PathEl, PathSeg, ParamCurve, ParamCurveArclen};

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f32,
    y: f32,
}

struct MorphContour {
    from: Vec<Point>,
    to: Vec<Point>,
    closed: bool,
}

pub struct MorphSvgEntry {
    contour: MorphContour,
}

impl MorphSvgEntry {
    pub fn new(from_svg: &str, to_svg: &str, sample_count: u32) -> Option<Self> {
        let count = sample_count.clamp(8, 2048) as usize;
        let from_path = BezPath::from_svg(from_svg).ok()?;
        let to_path = BezPath::from_svg(to_svg).ok()?;
        let contour = build_contour(&from_path, &to_path, count)?;
        Some(Self { contour })
    }

    pub fn sample(&self, t: f32, tolerance: f32) -> String {
        let t = t.clamp(0.0, 1.0);
        let mut points: Vec<Point> = self
            .contour
            .from
            .iter()
            .zip(self.contour.to.iter())
            .map(|(a, b)| Point {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            })
            .collect();

        let tolerance = tolerance.max(0.0);
        if tolerance > 0.0 {
            points = if self.contour.closed {
                rdp_simplify_closed(&points, tolerance)
            } else {
                rdp_simplify_open(&points, tolerance)
            };
        }

        let min_points = if self.contour.closed { 3 } else { 2 };
        if points.len() < min_points {
            return String::new();
        }

        contour_to_svg(&points, self.contour.closed)
    }
}

fn build_contour(
    from_path: &BezPath,
    to_path: &BezPath,
    sample_count: usize,
) -> Option<MorphContour> {
    let (from_contour, from_closed) = single_contour(from_path)?;
    let (to_contour, to_closed) = single_contour(to_path)?;
    if from_closed != to_closed {
        return None;
    }

    let closed = from_closed;
    let min_points = if closed { 3 } else { 2 };
    let count = sample_count.max(min_points);
    let from_points = sample_contour(&from_contour, count, closed)?;
    let to_points = align_points(&from_points, sample_contour(&to_contour, count, closed)?, closed);

    if from_points.len() < min_points || to_points.len() < min_points {
        return None;
    }

    Some(MorphContour {
        from: from_points,
        to: to_points,
        closed,
    })
}

struct MeasuredContour {
    segments: Vec<PathSeg>,
    cumulative: Vec<f64>,
    length: f32,
}

fn single_contour(path: &BezPath) -> Option<(MeasuredContour, bool)> {
    let mut contours: Vec<MeasuredContour> = Vec::new();
    let mut segs: Vec<PathSeg> = Vec::new();
    let mut cum: Vec<f64> = Vec::new();
    let mut len: f64 = 0.0;
    let mut last: Option<kurbo::Point> = None;

    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                if !segs.is_empty() {
                    contours.push(MeasuredContour {
                        segments: std::mem::take(&mut segs),
                        cumulative: std::mem::take(&mut cum),
                        length: len as f32,
                    });
                    len = 0.0;
                }
                last = Some(p);
            }
            PathEl::LineTo(p) => {
                if let Some(prev) = last {
                    let s = PathSeg::Line(kurbo::Line::new(prev, p));
                    len += s.arclen(ARCLEN_ACCURACY);
                    cum.push(len);
                    segs.push(s);
                }
                last = Some(p);
            }
            PathEl::QuadTo(c, p) => {
                if let Some(prev) = last {
                    let s = PathSeg::Quad(kurbo::QuadBez::new(prev, c, p));
                    len += s.arclen(ARCLEN_ACCURACY);
                    cum.push(len);
                    segs.push(s);
                }
                last = Some(p);
            }
            PathEl::CurveTo(c1, c2, p) => {
                if let Some(prev) = last {
                    let s = PathSeg::Cubic(kurbo::CubicBez::new(prev, c1, c2, p));
                    len += s.arclen(ARCLEN_ACCURACY);
                    cum.push(len);
                    segs.push(s);
                }
                last = Some(p);
            }
            PathEl::ClosePath => {
                let first = segs.first().map(|s| s.start());
                if let (Some(prev), Some(first)) = (last, first) {
                    if (prev - first).hypot() > 1e-6 {
                        let s = PathSeg::Line(kurbo::Line::new(prev, first));
                        len += s.arclen(ARCLEN_ACCURACY);
                        cum.push(len);
                        segs.push(s);
                    }
                }
                last = first;
            }
        }
    }
    if !segs.is_empty() {
        contours.push(MeasuredContour {
            segments: segs,
            cumulative: cum,
            length: len as f32,
        });
    }

    if contours.len() != 1 {
        return None;
    }

    let contour = contours.pop().unwrap();
    let closed = is_closed(path);
    if !contour.length.is_finite() || contour.length <= f32::EPSILON {
        return None;
    }

    Some((MeasuredContour {
        segments: contour.segments,
        cumulative: contour.cumulative,
        length: contour.length,
    }, closed))
}

fn is_closed(path: &BezPath) -> bool {
    path.elements()
        .last()
        .map_or(false, |el| matches!(el, PathEl::ClosePath))
}

fn sample_contour(contour: &MeasuredContour, count: usize, closed: bool) -> Option<Vec<Point>> {
    let mut points = Vec::with_capacity(count);
    for i in 0..count {
        let progress = if closed {
            i as f32 / count as f32
        } else {
            i as f32 / (count.saturating_sub(1).max(1)) as f32
        };
        let (x, y, _) = sample_at_length_from_contour(contour, contour.length as f64 * progress as f64);
        points.push(Point { x, y });
    }
    Some(remove_near_duplicate_points(points))
}

fn sample_at_length_from_contour(contour: &MeasuredContour, length: f64) -> (f32, f32, f32) {
    let idx = match contour
        .cumulative
        .binary_search_by(|cum| cum.partial_cmp(&length).unwrap_or(std::cmp::Ordering::Equal))
    {
        Ok(i) => i,
        Err(i) => i.min(contour.cumulative.len().saturating_sub(1)),
    };
    let prev = if idx == 0 { 0.0 } else { contour.cumulative[idx - 1] };
    let local = length - prev;
    let seg = &contour.segments[idx];
    let seg_len = seg.arclen(ARCLEN_ACCURACY);
    let local_t = if seg_len > f64::EPSILON {
        seg.inv_arclen(local, ARCLEN_ACCURACY)
    } else {
        0.0
    };
    let p = seg.eval(local_t);
    let d = derivative(seg, local_t);
    let angle = d.y.atan2(d.x).to_degrees() as f32;
    (p.x as f32, p.y as f32, angle)
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

const ARCLEN_ACCURACY: f64 = 0.25;

fn align_points(reference: &[Point], candidate: Vec<Point>, closed: bool) -> Vec<Point> {
    if reference.is_empty() || candidate.is_empty() {
        return candidate;
    }

    let forward = if closed {
        best_cyclic_shift(reference, &candidate)
    } else {
        candidate.clone()
    };
    let mut reversed = candidate;
    reversed.reverse();
    let reverse = if closed {
        best_cyclic_shift(reference, &reversed)
    } else {
        reversed
    };

    if correspondence_error(reference, &reverse) < correspondence_error(reference, &forward) {
        reverse
    } else {
        forward
    }
}

fn best_cyclic_shift(reference: &[Point], points: &[Point]) -> Vec<Point> {
    let n = reference.len().min(points.len());
    if n == 0 {
        return Vec::new();
    }

    let mut best_shift = 0usize;
    let mut best_error = f32::INFINITY;
    for shift in 0..n {
        let mut error = 0.0f32;
        for i in 0..n {
            error += distance_sq(reference[i], points[(i + shift) % n]);
        }
        if error < best_error {
            best_error = error;
            best_shift = shift;
        }
    }

    (0..n).map(|i| points[(i + best_shift) % n]).collect()
}

fn correspondence_error(a: &[Point], b: &[Point]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return f32::INFINITY;
    }
    (0..n).map(|i| distance_sq(a[i], b[i])).sum::<f32>() / n as f32
}

fn distance_sq(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

fn remove_near_duplicate_points(points: Vec<Point>) -> Vec<Point> {
    let mut deduped: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if deduped
            .last()
            .map(|last| distance_sq(*last, point) > 1e-8)
            .unwrap_or(true)
        {
            deduped.push(point);
        }
    }

    if deduped.len() > 1 && distance_sq(deduped[0], deduped[deduped.len() - 1]) <= 1e-8 {
        deduped.pop();
    }

    deduped
}

fn rdp_simplify_closed(points: &[Point], epsilon: f32) -> Vec<Point> {
    if points.len() <= 3 {
        return points.to_vec();
    }

    let anchor = leftmost_point_index(points);
    let mut opened = Vec::with_capacity(points.len() + 1);
    for i in 0..points.len() {
        opened.push(points[(anchor + i) % points.len()]);
    }
    opened.push(opened[0]);

    let mut simplified = rdp_simplify_open(&opened, epsilon);
    if simplified.len() > 1
        && distance_sq(simplified[0], simplified[simplified.len() - 1]) <= 1e-8
    {
        simplified.pop();
    }

    if simplified.len() < 3 {
        points.to_vec()
    } else {
        simplified
    }
}

fn leftmost_point_index(points: &[Point]) -> usize {
    points
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn rdp_simplify_open(points: &[Point], epsilon: f32) -> Vec<Point> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let first = points[0];
    let last = points[points.len() - 1];
    let mut max_dist = 0.0f32;
    let mut max_idx = 0usize;

    for (i, point) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let dist = perpendicular_distance(*point, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        let left = rdp_simplify_open(&points[..=max_idx], epsilon);
        let right = rdp_simplify_open(&points[max_idx..], epsilon);
        let mut result = left;
        result.pop();
        result.extend(right);
        result
    } else {
        vec![first, last]
    }
}

fn perpendicular_distance(p: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return distance_sq(p, a).sqrt();
    }

    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    let proj = Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    };
    distance_sq(p, proj).sqrt()
}

fn contour_to_svg(points: &[Point], closed: bool) -> String {
    let mut segs = Vec::new();
    append_cubic_contour(&mut segs, points, closed);
    if closed {
        segs.push("Z".to_string());
    }
    segs.join(" ")
}

fn append_cubic_contour(segs: &mut Vec<String>, points: &[Point], closed: bool) {
    segs.push(format!(
        "M {} {}",
        fmt_num(points[0].x),
        fmt_num(points[0].y)
    ));

    let segment_count = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    for i in 0..segment_count {
        let p1 = points[i];
        let p2 = points[(i + 1) % points.len()];
        let c1 = Point {
            x: p1.x + (p2.x - p1.x) / 3.0,
            y: p1.y + (p2.y - p1.y) / 3.0,
        };
        let c2 = Point {
            x: p1.x + (p2.x - p1.x) * 2.0 / 3.0,
            y: p1.y + (p2.y - p1.y) * 2.0 / 3.0,
        };

        segs.push(format!(
            "C {} {},{} {},{} {}",
            fmt_num(c1.x),
            fmt_num(c1.y),
            fmt_num(c2.x),
            fmt_num(c2.y),
            fmt_num(p2.x),
            fmt_num(p2.y)
        ));
    }
}

fn fmt_num(v: f32) -> String {
    let s = format!("{:.2}", v);
    if s.ends_with(".00") {
        s[..s.len() - 3].to_string()
    } else {
        s
    }
}

#[derive(Default)]
pub struct MorphSvgState {
    pub next_id: i32,
    pub entries: HashMap<i32, MorphSvgEntry>,
}

impl MorphSvgState {
    pub fn create(&mut self, from_svg: &str, to_svg: &str, grid: u32) -> Option<i32> {
        let entry = MorphSvgEntry::new(from_svg, to_svg, grid)?;
        let handle = self.next_id;
        self.next_id += 1;
        self.entries.insert(handle, entry);
        Some(handle)
    }
    pub fn sample(&self, handle: i32, t: f32, tolerance: f32) -> String {
        self.entries
            .get(&handle)
            .map(|e| e.sample(t, tolerance))
            .unwrap_or_default()
    }
    pub fn dispose(&mut self, handle: i32) {
        self.entries.remove(&handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morph_triangle_at_t0_produces_valid_path() {
        let entry =
            MorphSvgEntry::new("M55 0 L110 95 L0 95 Z", "M55 95 L110 0 L0 0 Z", 128)
                .expect("should create entry from valid paths");
        let result = entry.sample(0.0, 0.5);
        assert_ne!(result, "M55 0 L110 95 L0 95 Z");
        assert!(kurbo::BezPath::from_svg(&result).is_ok());
    }

    #[test]
    fn morph_triangle_at_t1_produces_valid_path() {
        let entry =
            MorphSvgEntry::new("M55 0 L110 95 L0 95 Z", "M55 95 L110 0 L0 0 Z", 128)
                .expect("should create entry");
        let result = entry.sample(1.0, 0.5);
        assert_ne!(result, "M55 95 L110 0 L0 0 Z");
        assert!(kurbo::BezPath::from_svg(&result).is_ok());
    }

    #[test]
    fn morph_endpoint_stays_on_resampled_track() {
        let to_svg = "M55 0 L110 28 L110 82 L55 110 L0 82 L0 28 Z";
        let entry = MorphSvgEntry::new(
            "M55 0 L69 37 L110 37 L77 60 L88 100 L55 78 L22 100 L33 60 L0 37 L41 37 Z",
            to_svg,
            128,
        )
        .expect("should create entry");

        let at_end = entry.sample(1.0, 0.0);
        let near_end = entry.sample(0.9999, 0.0);

        assert_ne!(at_end, to_svg);
        assert!(kurbo::BezPath::from_svg(&at_end).is_ok());
        assert!(kurbo::BezPath::from_svg(&near_end).is_ok());
        assert_eq!(at_end.matches('M').count(), 1);
        assert_eq!(at_end.matches('Z').count(), 1);
        assert_eq!(
            at_end.matches('C').count(),
            near_end.matches('C').count()
        );
    }

    #[test]
    fn morph_at_t05_produces_valid_path() {
        let entry =
            MorphSvgEntry::new("M55 0 L110 95 L0 95 Z", "M55 95 L110 0 L0 0 Z", 128)
                .expect("should create entry");
        let result = entry.sample(0.5, 0.5);
        assert!(!result.is_empty(), "sample at t=0.5 should not be empty");
        assert!(kurbo::BezPath::from_svg(&result).is_ok());
        assert!(
            result.contains('C'),
            "intermediate morphs should use cubic output: {result}"
        );
        assert!(
            !result.contains('L'),
            "intermediate morphs should not emit line commands: {result}"
        );
    }

    #[test]
    fn morph_reversed_closed_paths_do_not_collapse() {
        let entry = MorphSvgEntry::new(
            "M0 0 L100 0 L100 100 L0 100 Z",
            "M0 0 L0 100 L100 100 L100 0 Z",
            64,
        )
        .expect("should align reversed winding");
        let result = entry.sample(0.5, 0.1);
        assert!(!result.is_empty());
        assert!(kurbo::BezPath::from_svg(&result).is_ok());
        assert_eq!(result.matches('M').count(), 1);
        assert_eq!(result.matches('Z').count(), 1);
    }

    #[test]
    fn morph_open_path_stays_open() {
        let entry = MorphSvgEntry::new(
            "M0 0 C30 60 70 -60 100 0",
            "M0 50 C30 -10 70 110 100 50",
            64,
        )
        .expect("should morph open contours");
        let result = entry.sample(0.5, 0.25);
        assert!(!result.is_empty());
        assert!(kurbo::BezPath::from_svg(&result).is_ok());
        assert!(
            !result.contains('Z'),
            "open path should not be forcibly closed: {result}"
        );
        assert!(
            result.contains('C'),
            "open intermediate morphs should use cubic output: {result}"
        );
    }

    #[test]
    fn rejects_multiple_subpaths() {
        assert!(
            MorphSvgEntry::new(
                "M0 0 L40 0 L40 40 L0 40 Z M80 0 L120 0 L120 40 L80 40 Z",
                "M10 10 L50 10 L50 50 L10 50 Z M90 10 L130 10 L130 50 L90 50 Z",
                96,
            )
            .is_none()
        );
    }

    #[test]
    fn rejects_mismatched_subpath_counts() {
        assert!(
            MorphSvgEntry::new(
                "M0 0 L100 0",
                "M0 0 L100 0 M40 40 L60 40 L60 60 L40 60 Z",
                96,
            )
            .is_none()
        );
    }

    #[test]
    fn rejects_open_to_closed() {
        assert!(
            MorphSvgEntry::new(
                "M0 0 C30 60 70 -60 100 0",
                "M0 0 L100 0 L50 80 Z",
                64,
            )
            .is_none()
        );
    }
}
