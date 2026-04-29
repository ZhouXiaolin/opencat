// Path morphing by arc-length resampling and point correspondence.
//
// This keeps the runtime contract of the morph-svg plugin, but avoids the
// SDF/marching-squares reconstruction path. The tradeoff is intentional:
// output paths are cubic-only, but interpolation is deterministic, cheap, and
// preserves one coherent contour through the whole animation.

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

struct MorphContour {
    from: Vec<Point>,
    to: Vec<Point>,
    closed: bool,
}

pub(crate) struct MorphSvgEntry {
    from_svg: String,
    to_svg: String,
    contours: Vec<MorphContour>,
}

impl MorphSvgEntry {
    pub fn new(from_svg: &str, to_svg: &str, sample_count: u32) -> Option<Self> {
        let count = sample_count.clamp(8, 2048) as usize;
        let path_a = skia_safe::Path::from_svg(from_svg)?;
        let path_b = skia_safe::Path::from_svg(to_svg)?;

        let contours = build_contours(&path_a, &path_b, count)?;

        Some(Self {
            from_svg: from_svg.to_string(),
            to_svg: to_svg.to_string(),
            contours,
        })
    }

    pub fn sample(&self, t: f32, tolerance: f32) -> String {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 {
            return self.from_svg.clone();
        }
        if t >= 1.0 {
            return self.to_svg.clone();
        }

        let tolerance = tolerance.max(0.0);
        let mut sampled = Vec::with_capacity(self.contours.len());
        for contour in &self.contours {
            let mut points: Vec<Point> = contour
                .from
                .iter()
                .zip(contour.to.iter())
                .map(|(a, b)| Point {
                    x: a.x + (b.x - a.x) * t,
                    y: a.y + (b.y - a.y) * t,
                })
                .collect();

            if tolerance > 0.0 {
                points = if contour.closed {
                    rdp_simplify_closed(&points, tolerance)
                } else {
                    rdp_simplify_open(&points, tolerance)
                };
            }

            let min_points = if contour.closed { 3 } else { 2 };
            if points.len() >= min_points {
                sampled.push((points, contour.closed));
            }
        }

        if sampled.is_empty() {
            return String::new();
        }

        contours_to_svg(&sampled)
    }
}

struct MeasuredContour {
    measure: skia_safe::ContourMeasure,
    length: f32,
    closed: bool,
    centroid: Point,
    bounds: Bounds,
    area: f32,
    preview: Vec<Point>,
}

#[derive(Clone, Copy)]
struct MatchPair {
    from: Option<usize>,
    to: Option<usize>,
}

fn build_contours(
    from_path: &skia_safe::Path,
    to_path: &skia_safe::Path,
    sample_count: usize,
) -> Option<Vec<MorphContour>> {
    let from = measure_contours(from_path);
    let to = measure_contours(to_path);
    if from.is_empty() || to.is_empty() {
        return None;
    }

    let pairs = match_contours(&from, &to);
    let avg_lengths: Vec<f32> = pairs
        .iter()
        .map(|pair| average_pair_length(pair, &from, &to))
        .collect();
    let total_avg_length = avg_lengths.iter().sum::<f32>().max(1.0);

    let mut result = Vec::with_capacity(pairs.len());
    for (pair, avg_length) in pairs.iter().zip(avg_lengths.iter()) {
        let from_contour = pair.from.and_then(|idx| from.get(idx));
        let to_contour = pair.to.and_then(|idx| to.get(idx));
        let closed = from_contour.map(|c| c.closed).unwrap_or(false)
            && to_contour.map(|c| c.closed).unwrap_or(false);
        let min_points = if closed { 3 } else { 2 };
        let pair_samples = ((sample_count as f32 * *avg_length / total_avg_length).round()
            as usize)
            .clamp(min_points, sample_count.max(min_points));

        let mut from_points = from_contour
            .and_then(|c| sample_contour(c, pair_samples, closed))
            .unwrap_or_default();
        let mut to_points = to_contour
            .and_then(|c| sample_contour(c, pair_samples, closed))
            .unwrap_or_default();

        if from_points.is_empty() && !to_points.is_empty() {
            from_points = collapsed_points(centroid(&to_points), to_points.len());
        }
        if to_points.is_empty() && !from_points.is_empty() {
            to_points = collapsed_points(centroid(&from_points), from_points.len());
        }
        if from_points.is_empty() || to_points.is_empty() {
            continue;
        }

        let to_points = align_points(&from_points, to_points, closed);
        result.push(MorphContour {
            from: from_points,
            to: to_points,
            closed,
        });
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn measure_contours(path: &skia_safe::Path) -> Vec<MeasuredContour> {
    skia_safe::ContourMeasureIter::new(path, false, None)
        .filter_map(|measure| {
            let length = measure.length();
            if !length.is_finite() || length <= f32::EPSILON {
                return None;
            }
            let closed = measure.is_closed();
            let preview = sample_measure(&measure, length, 32, closed)?;
            Some(MeasuredContour {
                centroid: centroid(&preview),
                bounds: bounds_of(&preview),
                area: signed_area(&preview).abs(),
                closed,
                length,
                measure,
                preview,
            })
        })
        .collect()
}

fn match_contours(from: &[MeasuredContour], to: &[MeasuredContour]) -> Vec<MatchPair> {
    if to.len() <= 18 {
        optimal_match_contours(from, to)
    } else {
        greedy_match_contours(from, to)
    }
}

fn optimal_match_contours(from: &[MeasuredContour], to: &[MeasuredContour]) -> Vec<MatchPair> {
    let states = 1usize << to.len();
    let mut dp = vec![f32::INFINITY; states];
    let mut plans = vec![Vec::<MatchPair>::new(); states];
    dp[0] = 0.0;

    for (from_idx, from_contour) in from.iter().enumerate() {
        let mut next = vec![f32::INFINITY; states];
        let mut next_plans = vec![Vec::<MatchPair>::new(); states];

        for mask in 0..states {
            if !dp[mask].is_finite() {
                continue;
            }

            let skip_cost = dp[mask] + unmatched_cost(from_contour);
            if skip_cost < next[mask] {
                next[mask] = skip_cost;
                next_plans[mask] = plans[mask].clone();
                next_plans[mask].push(MatchPair {
                    from: Some(from_idx),
                    to: None,
                });
            }

            for (to_idx, to_contour) in to.iter().enumerate() {
                let bit = 1usize << to_idx;
                if mask & bit != 0 {
                    continue;
                }
                let next_mask = mask | bit;
                let pair_cost = dp[mask] + contour_pair_cost(from_contour, to_contour);
                if pair_cost < next[next_mask] {
                    next[next_mask] = pair_cost;
                    next_plans[next_mask] = plans[mask].clone();
                    next_plans[next_mask].push(MatchPair {
                        from: Some(from_idx),
                        to: Some(to_idx),
                    });
                }
            }
        }

        dp = next;
        plans = next_plans;
    }

    let mut best_cost = f32::INFINITY;
    let mut best_plan = Vec::new();
    for mask in 0..states {
        if !dp[mask].is_finite() {
            continue;
        }
        let mut cost = dp[mask];
        let mut plan = plans[mask].clone();
        for (to_idx, to_contour) in to.iter().enumerate() {
            if mask & (1usize << to_idx) == 0 {
                cost += unmatched_cost(to_contour);
                plan.push(MatchPair {
                    from: None,
                    to: Some(to_idx),
                });
            }
        }
        if cost < best_cost {
            best_cost = cost;
            best_plan = plan;
        }
    }

    best_plan
}

fn greedy_match_contours(from: &[MeasuredContour], to: &[MeasuredContour]) -> Vec<MatchPair> {
    let mut edges = Vec::new();
    for (from_idx, from_contour) in from.iter().enumerate() {
        for (to_idx, to_contour) in to.iter().enumerate() {
            edges.push((
                contour_pair_cost(from_contour, to_contour),
                from_idx,
                to_idx,
            ));
        }
    }
    edges.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut used_from = vec![false; from.len()];
    let mut used_to = vec![false; to.len()];
    let mut pairs = Vec::new();
    for (cost, from_idx, to_idx) in edges {
        if used_from[from_idx] || used_to[to_idx] {
            continue;
        }
        if cost > unmatched_cost(&from[from_idx]) + unmatched_cost(&to[to_idx]) {
            continue;
        }
        used_from[from_idx] = true;
        used_to[to_idx] = true;
        pairs.push(MatchPair {
            from: Some(from_idx),
            to: Some(to_idx),
        });
    }

    for (idx, used) in used_from.iter().enumerate() {
        if !used {
            pairs.push(MatchPair {
                from: Some(idx),
                to: None,
            });
        }
    }
    for (idx, used) in used_to.iter().enumerate() {
        if !used {
            pairs.push(MatchPair {
                from: None,
                to: Some(idx),
            });
        }
    }
    pairs
}

fn contour_pair_cost(a: &MeasuredContour, b: &MeasuredContour) -> f32 {
    let bounds = union_bounds(a.bounds, b.bounds);
    let diag = bounds_diagonal(bounds).max(1.0);
    let centroid_cost = distance_sq(a.centroid, b.centroid).sqrt() / diag;
    let length_cost = (a.length.max(1.0) / b.length.max(1.0)).ln().abs();
    let area_cost = (a.area.max(1.0) / b.area.max(1.0)).ln().abs();
    let bounds_cost = bounds_difference(a.bounds, b.bounds) / diag;
    let shape_cost = correspondence_error(
        &a.preview,
        &align_points(&a.preview, b.preview.clone(), a.closed && b.closed),
    )
    .sqrt()
        / diag;
    let closed_cost = if a.closed == b.closed { 0.0 } else { 1.5 };

    centroid_cost * 2.0
        + shape_cost * 2.5
        + length_cost * 0.75
        + area_cost * 0.35
        + bounds_cost * 0.75
        + closed_cost
}

fn unmatched_cost(contour: &MeasuredContour) -> f32 {
    3.0 + if contour.closed { 0.35 } else { 0.0 } + contour.length.max(1.0).ln() * 0.05
}

fn average_pair_length(pair: &MatchPair, from: &[MeasuredContour], to: &[MeasuredContour]) -> f32 {
    let from_len = pair
        .from
        .and_then(|idx| from.get(idx))
        .map(|c| c.length)
        .unwrap_or(0.0);
    let to_len = pair
        .to
        .and_then(|idx| to.get(idx))
        .map(|c| c.length)
        .unwrap_or(0.0);
    ((from_len + to_len) * 0.5).max(1.0)
}

fn sample_contour(contour: &MeasuredContour, count: usize, closed: bool) -> Option<Vec<Point>> {
    sample_measure(&contour.measure, contour.length, count, closed)
}

fn sample_measure(
    measure: &skia_safe::ContourMeasure,
    length: f32,
    count: usize,
    closed: bool,
) -> Option<Vec<Point>> {
    let mut points = Vec::with_capacity(count);
    for i in 0..count {
        let progress = if closed {
            i as f32 / count as f32
        } else {
            i as f32 / (count.saturating_sub(1).max(1)) as f32
        };
        let (p, _) = measure.pos_tan(length * progress)?;
        points.push(Point { x: p.x, y: p.y });
    }

    Some(remove_near_duplicate_points(points))
}

fn align_points(reference: &[Point], candidate: Vec<Point>, closed: bool) -> Vec<Point> {
    if reference.is_empty() || candidate.is_empty() {
        return candidate;
    }

    let forward = if closed {
        best_cyclic_shift(reference, &candidate)
    } else {
        candidate.clone()
    };
    let mut reversed = candidate.clone();
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

fn collapsed_points(point: Point, count: usize) -> Vec<Point> {
    vec![point; count]
}

fn centroid(points: &[Point]) -> Point {
    let mut x = 0.0;
    let mut y = 0.0;
    for point in points {
        x += point.x;
        y += point.y;
    }
    let n = points.len().max(1) as f32;
    Point { x: x / n, y: y / n }
}

fn bounds_of(points: &[Point]) -> Bounds {
    let mut bounds = Bounds {
        left: f32::INFINITY,
        top: f32::INFINITY,
        right: f32::NEG_INFINITY,
        bottom: f32::NEG_INFINITY,
    };
    for point in points {
        bounds.left = bounds.left.min(point.x);
        bounds.top = bounds.top.min(point.y);
        bounds.right = bounds.right.max(point.x);
        bounds.bottom = bounds.bottom.max(point.y);
    }
    bounds
}

fn union_bounds(a: Bounds, b: Bounds) -> Bounds {
    Bounds {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
    }
}

fn bounds_diagonal(bounds: Bounds) -> f32 {
    let w = (bounds.right - bounds.left).max(0.0);
    let h = (bounds.bottom - bounds.top).max(0.0);
    (w * w + h * h).sqrt()
}

fn bounds_difference(a: Bounds, b: Bounds) -> f32 {
    let aw = (a.right - a.left).max(1.0);
    let ah = (a.bottom - a.top).max(1.0);
    let bw = (b.right - b.left).max(1.0);
    let bh = (b.bottom - b.top).max(1.0);
    let center_a = Point {
        x: (a.left + a.right) * 0.5,
        y: (a.top + a.bottom) * 0.5,
    };
    let center_b = Point {
        x: (b.left + b.right) * 0.5,
        y: (b.top + b.bottom) * 0.5,
    };
    distance_sq(center_a, center_b).sqrt() + (aw - bw).abs() + (ah - bh).abs()
}

fn signed_area(points: &[Point]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a.x * b.y - b.x * a.y;
    }
    area * 0.5
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
    if simplified.len() > 1 && distance_sq(simplified[0], simplified[simplified.len() - 1]) <= 1e-8
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

fn contours_to_svg(contours: &[(Vec<Point>, bool)]) -> String {
    let mut segs = Vec::new();
    for (points, closed) in contours {
        append_cubic_contour(&mut segs, points, *closed);
        if *closed {
            segs.push("Z".to_string());
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morph_triangle_at_t0_produces_valid_path() {
        let entry = MorphSvgEntry::new("M55 0 L110 95 L0 95 Z", "M55 95 L110 0 L0 0 Z", 128)
            .expect("should create entry from valid paths");
        let result = entry.sample(0.0, 0.5);
        assert!(!result.is_empty(), "sample at t=0 should not be empty");
        assert!(
            skia_safe::Path::from_svg(&result).is_some(),
            "sample output should be valid SVG path: {:?}",
            result
        );
    }

    #[test]
    fn morph_triangle_at_t1_produces_valid_path() {
        let entry = MorphSvgEntry::new("M55 0 L110 95 L0 95 Z", "M55 95 L110 0 L0 0 Z", 128)
            .expect("should create entry");
        let result = entry.sample(1.0, 0.5);
        assert!(!result.is_empty(), "sample at t=1 should not be empty");
        assert!(skia_safe::Path::from_svg(&result).is_some());
    }

    #[test]
    fn morph_at_t05_produces_valid_path() {
        let entry = MorphSvgEntry::new("M55 0 L110 95 L0 95 Z", "M55 95 L110 0 L0 0 Z", 128)
            .expect("should create entry");
        let result = entry.sample(0.5, 0.5);
        assert!(!result.is_empty(), "sample at t=0.5 should not be empty");
        assert!(skia_safe::Path::from_svg(&result).is_some());
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
    fn morph_star_to_hex_produces_single_contour() {
        let entry = MorphSvgEntry::new(
            "M55 0 L69 37 L110 37 L77 60 L88 100 L55 78 L22 100 L33 60 L0 37 L41 37 Z",
            "M55 0 L110 28 L110 82 L55 110 L0 82 L0 28 Z",
            128,
        )
        .expect("should create entry");
        let result = entry.sample(0.5, 0.5);
        assert!(!result.is_empty());
        assert!(skia_safe::Path::from_svg(&result).is_some());
        assert_eq!(
            result.matches(|c| c == 'M').count(),
            1,
            "should have exactly one contour"
        );
    }

    #[test]
    fn morph_reversed_paths_do_not_collapse() {
        let entry = MorphSvgEntry::new(
            "M0 0 L100 0 L100 100 L0 100 Z",
            "M0 0 L0 100 L100 100 L100 0 Z",
            64,
        )
        .expect("should align reversed winding");
        let result = entry.sample(0.5, 0.1);
        assert!(!result.is_empty());
        assert!(skia_safe::Path::from_svg(&result).is_some());
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
        assert!(skia_safe::Path::from_svg(&result).is_some());
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
    fn morph_multiple_subpaths_preserves_multiple_contours() {
        let entry = MorphSvgEntry::new(
            "M0 0 L40 0 L40 40 L0 40 Z M80 0 L120 0 L120 40 L80 40 Z",
            "M10 10 L50 10 L50 50 L10 50 Z M90 10 L130 10 L130 50 L90 50 Z",
            96,
        )
        .expect("should morph multiple closed subpaths");
        let result = entry.sample(0.5, 0.25);
        assert!(!result.is_empty());
        assert!(skia_safe::Path::from_svg(&result).is_some());
        assert_eq!(result.matches('M').count(), 2);
        assert_eq!(result.matches('Z').count(), 2);
    }

    #[test]
    fn morph_mismatched_subpath_counts_fades_missing_contour_from_centroid() {
        let entry = MorphSvgEntry::new(
            "M0 0 L100 0",
            "M0 0 L100 0 M40 40 L60 40 L60 60 L40 60 Z",
            96,
        )
        .expect("should tolerate mismatched contour counts");
        let result = entry.sample(0.5, 0.25);
        assert!(!result.is_empty());
        assert!(skia_safe::Path::from_svg(&result).is_some());
        assert_eq!(result.matches('M').count(), 2);
    }

    #[test]
    fn matching_uses_geometry_instead_of_subpath_order() {
        let from_path =
            skia_safe::Path::from_svg("M0 0 L30 0 L30 30 L0 30 Z M80 0 L110 0 L110 30 L80 30 Z")
                .unwrap();
        let to_path =
            skia_safe::Path::from_svg("M82 2 L112 2 L112 32 L82 32 Z M2 2 L32 2 L32 32 L2 32 Z")
                .unwrap();

        let from = measure_contours(&from_path);
        let to = measure_contours(&to_path);
        let pairs = match_contours(&from, &to);
        let matched: Vec<(usize, usize)> = pairs
            .iter()
            .filter_map(|pair| pair.from.zip(pair.to))
            .collect();

        assert!(
            matched.contains(&(0, 1)),
            "left contour should match left target"
        );
        assert!(
            matched.contains(&(1, 0)),
            "right contour should match right target"
        );
    }
}
