//! Small 2-D geometry primitives shared by scripts and effects.
//!
//! These are pure, allocation-light helpers with no store or platform
//! dependency. They exist so scripts can call robust shape analysis (used, for
//! example, to recover a 3-D mesh's projected in-plane orientation) without
//! re-deriving numerical geometry in JS.

/// Convex hull of a 2-D point set (Andrew's monotone chain).
///
/// Returns the hull vertices in counter-clockwise order (in a y-up frame),
/// de-duplicated. Collinear points on a hull edge are dropped. Returns the
/// input (or fewer) for degenerate sets of 0/1/2 points.
pub fn convex_hull(mut pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    if pts.len() <= 2 {
        return pts;
    }
    pts.sort_by(|a, b| {
        a.partial_cmp(b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    pts.dedup();
    if pts.len() <= 2 {
        return pts;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| -> f64 {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let build = |seq: &[[f64; 2]]| -> Vec<[f64; 2]> {
        let mut h: Vec<[f64; 2]> = Vec::with_capacity(seq.len());
        for &p in seq {
            while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
                h.pop();
            }
            h.push(p);
        }
        h
    };
    let mut lower = build(&pts);
    let mut rev = pts.clone();
    rev.reverse();
    let mut upper = build(&rev);
    // lower and upper each end with the terminal point; drop it from each.
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Minimum-area enclosing rectangle of a 2-D point set, by rotating calipers
/// over the convex hull.
///
/// `angle` is the orientation (radians, in the input frame) of the rectangle's
/// first axis; `width`/`height` are the side lengths. When several hull edges
/// tie on area (e.g. a near-square outline) the first encountered is kept,
/// which keeps the result a deterministic function of the input order.
/// Returns `None` for an empty set; a single point yields a degenerate box.
pub fn min_area_rect(points: &[[f64; 2]]) -> Option<(f64, f64, f64)> {
    if points.is_empty() {
        return None;
    }
    let hull = convex_hull(points.to_vec());
    if hull.len() <= 2 {
        // Degenerate: axis-aligned box around whatever we have.
        let minx = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        let maxx = points.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
        let miny = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let maxy = points.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
        return Some((0.0, maxx - minx, maxy - miny));
    }
    let n = hull.len();
    let mut best: Option<(f64, f64, f64, f64)> = None; // (area, angle, w, h)
    for i in 0..n {
        let a = hull[i];
        let b = hull[(i + 1) % n];
        let edge = [b[0] - a[0], b[1] - a[1]];
        let len = (edge[0] * edge[0] + edge[1] * edge[1]).sqrt();
        if len < 1e-12 {
            continue;
        }
        let ux = [edge[0] / len, edge[1] / len];
        // Perpendicular axis (upright in a y-up frame).
        let uy = [-ux[1], ux[0]];
        let (mut min_u, mut max_u, mut min_v, mut max_v) =
            (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for p in &hull {
            let du = p[0] * ux[0] + p[1] * ux[1];
            let dv = p[0] * uy[0] + p[1] * uy[1];
            min_u = min_u.min(du);
            max_u = max_u.max(du);
            min_v = min_v.min(dv);
            max_v = max_v.max(dv);
        }
        let (w, h) = (max_u - min_u, max_v - min_v);
        let area = w * h;
        if best.map_or(true, |(a0, _, _, _)| area < a0) {
            best = Some((area, ux[1].atan2(ux[0]), w, h));
        }
    }
    let (_, angle, w, h) = best?;
    Some((angle, w, h))
}

/// Camera view basis `(forward, right, up)` for a pinhole camera at `eye`
/// looking at `target`, matching Three.js `PerspectiveCamera.lookAt`
/// (`THREE.Matrix4.lookAt`).
///
/// `forward = normalize(target − eye)` (the view direction), `right =
/// normalize(cross(forward, up_world))`, `up = cross(right, forward)`, with
/// `up_world = (0, 1, 0)`. This is exactly the basis the chromatic-plane
/// effect builds its view ray from
/// (`D = forward + sx·tanH·aspect·right + sy·tanH·up`), so the ray becomes a
/// deterministic function of the source camera constants instead of a
/// hand-tuned sign choice. The world-up is fixed; a degenerate `eye == target`
/// yields the Three.js default view basis (`forward = (0,0,-1)`).
pub fn camera_basis(eye: [f64; 3], target: [f64; 3]) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let norm = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l < 1e-12 {
            [0.0, 0.0, 0.0]
        } else {
            [v[0] / l, v[1] / l, v[2] / l]
        }
    };
    let mut forward = norm(sub(target, eye));
    if forward == [0.0, 0.0, 0.0] {
        forward = [0.0, 0.0, -1.0];
    }
    let right = norm(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    (forward, right, up)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_basis_default_is_threejs_frame() {
        // Camera at origin looking down −z: forward=−z, right=+x, up=+y.
        let (f, r, u) = camera_basis([0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
        assert!((f[0]).abs() < 1e-12 && (f[1]).abs() < 1e-12 && (f[2] + 1.0).abs() < 1e-12);
        assert!((r[0] - 1.0).abs() < 1e-12 && r[1].abs() < 1e-12 && r[2].abs() < 1e-12);
        assert!(u[0].abs() < 1e-12 && (u[1] - 1.0).abs() < 1e-12 && u[2].abs() < 1e-12);
    }

    #[test]
    fn camera_basis_right_matches_cross_forward_up() {
        // For any forward f, right = cross(f, up_world) normalised = (−fz,0,fx).
        let (f, r, u) = camera_basis([3.2, 0.05, 2.67], [-0.12, 0.05, 0.0]);
        let want = {
            let (fx, fz) = (f[0], f[2]);
            let l = (fz * fz + fx * fx).sqrt();
            [-fz / l, 0.0, fx / l]
        };
        for i in 0..3 {
            assert!((r[i] - want[i]).abs() < 1e-12, "right[{i}]={} want {}", r[i], want[i]);
        }
        // up = cross(right, forward), and the frame is orthonormal/right-handed.
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let u2 = cross(r, f);
        for i in 0..3 {
            assert!((u[i] - u2[i]).abs() < 1e-12);
        }
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!(dot(f, r).abs() < 1e-12 && dot(f, u).abs() < 1e-12 && dot(r, u).abs() < 1e-12);
    }

    #[test]
    fn camera_basis_degenerate_is_threejs_default() {
        let (f, r, u) = camera_basis([1.0, 2.0, 3.0], [1.0, 2.0, 3.0]);
        assert_eq!(f, [0.0, 0.0, -1.0]);
        assert_eq!(r, [1.0, 0.0, 0.0]);
        assert_eq!(u, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn hull_of_square_is_four_corners() {
        let pts = vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.5, 0.5],
        ];
        let h = convex_hull(pts);
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn min_area_rect_axis_aligned_square() {
        let pts = vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 2.0],
            [0.0, 2.0],
        ];
        let (angle, w, h) = min_area_rect(&pts).unwrap();
        // angle is 0 or a multiple of 90 deg with matching side assignment.
        let deg = angle.to_degrees().rem_euclid(90.0);
        assert!(deg < 1e-6 || (deg - 90.0).abs() < 1e-6);
        let (lo, hi) = (w.min(h), w.max(h));
        assert!((lo - 2.0).abs() < 1e-6 && (hi - 2.0).abs() < 1e-6);
    }

    #[test]
    fn min_area_rect_of_rotated_rectangle_tracks_angle() {
        // A 3x1 rectangle rotated by 30 degrees.
        let th: f64 = 30.0_f64.to_radians();
        let (c, s) = (th.cos(), th.sin());
        let corners = [[0.0, 0.0], [3.0, 0.0], [3.0, 1.0], [0.0, 1.0]];
        let pts: Vec<[f64; 2]> = corners
            .iter()
            .map(|p| [p[0] * c - p[1] * s, p[0] * s + p[1] * c])
            .collect();
        let (angle, w, h) = min_area_rect(&pts).unwrap();
        let deg = angle.to_degrees().rem_euclid(180.0);
        // Either 30 deg (long axis along the rectangle's length) or 120 deg.
        let near = (deg - 30.0).abs().min((deg - 120.0).abs());
        assert!(near < 1e-6, "angle {deg} not aligned to 30/120");
        let (lo, hi) = (w.min(h), w.max(h));
        assert!((lo - 1.0).abs() < 1e-6 && (hi - 3.0).abs() < 1e-6);
    }

    #[test]
    fn min_area_rect_of_diamond_is_itself() {
        // A diamond is a 45-deg-rotated square; its minimum-area enclosing
        // rectangle is the square itself, so the angle sits at 45 mod 90 and
        // both sides are sqrt(2).
        let pts = vec![[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]];
        let (angle, w, h) = min_area_rect(&pts).unwrap();
        let deg = angle.to_degrees().rem_euclid(90.0);
        assert!((deg - 45.0).abs() < 1e-6, "angle {deg} not 45");
        let s = 2.0_f64.sqrt();
        assert!((w - s).abs() < 1e-6 && (h - s).abs() < 1e-6, "w {w} h {h}");
    }
}
