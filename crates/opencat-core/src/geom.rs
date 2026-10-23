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

// ---------------------------------------------------------------------------
// 4×4 matrices — the CSS 3-D transform chain
// ---------------------------------------------------------------------------

/// Column-major 4×4 matrix (`m[col * 4 + row]`), the layout three.js and
/// OpenGL use. Only the operations the CSS `transform` grammar needs exist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]);

    /// `self · rhs` — a point is transformed by `rhs` first.
    pub fn mul(&self, rhs: &Mat4) -> Mat4 {
        let mut out = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += self.0[k * 4 + row] * rhs.0[col * 4 + k];
                }
                out[col * 4 + row] = sum;
            }
        }
        Mat4(out)
    }

    pub fn translate(x: f32, y: f32, z: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY.0;
        m[12] = x;
        m[13] = y;
        m[14] = z;
        Mat4(m)
    }

    /// CSS `rotateX(deg)` — positive angles tip the +y edge away from the
    /// viewer (the CSS y axis points down).
    pub fn rotate_x_deg(deg: f32) -> Mat4 {
        let (s, c) = deg.to_radians().sin_cos();
        let mut m = Mat4::IDENTITY.0;
        m[5] = c;
        m[6] = s;
        m[9] = -s;
        m[10] = c;
        Mat4(m)
    }

    /// CSS `rotateY(deg)`.
    pub fn rotate_y_deg(deg: f32) -> Mat4 {
        let (s, c) = deg.to_radians().sin_cos();
        let mut m = Mat4::IDENTITY.0;
        m[0] = c;
        m[2] = -s;
        m[8] = s;
        m[10] = c;
        Mat4(m)
    }

    /// CSS `rotate(deg)` — rotation about the view axis.
    pub fn rotate_z_deg(deg: f32) -> Mat4 {
        let (s, c) = deg.to_radians().sin_cos();
        let mut m = Mat4::IDENTITY.0;
        m[0] = c;
        m[1] = s;
        m[4] = -s;
        m[5] = c;
        Mat4(m)
    }

    pub fn scale(sx: f32, sy: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY.0;
        m[0] = sx;
        m[5] = sy;
        Mat4(m)
    }

    /// CSS `skew(ax, ay)`: `[[1, tan ax], [tan ay, 1]]`.
    pub fn skew(ax_deg: f32, ay_deg: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY.0;
        m[4] = ax_deg.to_radians().tan();
        m[1] = ay_deg.to_radians().tan();
        Mat4(m)
    }

    /// CSS `perspective(d)`: the w row gains `-1/d`, so points further along
    /// +z shrink toward the origin. `d <= 0` is the identity (no perspective).
    pub fn perspective(d: f32) -> Mat4 {
        if d <= 0.0 {
            return Mat4::IDENTITY;
        }
        let mut m = Mat4::IDENTITY.0;
        m[11] = -1.0 / d;
        Mat4(m)
    }

    /// Project onto the z=0 plane as a 3×3 homography, in the column-major
    /// layout `DrawOp::Concat` reads: `[h00, h10, h20, h01, h11, h21, h02,
    /// h12, h22]`, mapping `(x, y, 1)` to `(X, Y, W)`.
    ///
    /// A node's content is planar, so rows 0, 1 and 3 fully describe it; row 2
    /// (the z the *children* still need) is dropped here and read from the 4×4.
    pub fn to_homography3(&self) -> [f32; 9] {
        let m = &self.0;
        [m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]]
    }

    /// GSAP's folded `rotationX`/`rotationY`/`transformPerspective` triple as
    /// the CSS transform `rotateY(rotation_y) rotateX(rotation_x)`, i.e.
    /// `Ry · Rx`, with the perspective row folded into `w`.
    ///
    /// The z row matters as much as the planar one: a child sitting at
    /// `translateZ(d)` has to be scaled by `cos_x·cos_y` and to enter the
    /// perspective divisor through its own depth, otherwise a rotating form
    /// does not carry its children's depth with it.
    pub fn rotate3(rotation_x: f32, rotation_y: f32, perspective: f32) -> Mat4 {
        let k = if perspective > 0.0 {
            1.0 / perspective
        } else {
            0.0
        };
        let mut m = Mat4::rotate_y_deg(rotation_y)
            .mul(&Mat4::rotate_x_deg(rotation_x))
            .0;
        // w row: `1 - z'/perspective`, with `z'` the third row of Ry·Rx.
        m[3] = -m[2] * k;
        m[7] = -m[6] * k;
        m[11] = -m[10] * k;
        Mat4(m)
    }

    /// True when the matrix keeps the z=0 plane where it is: no z output and no
    /// z-dependent w. Those nodes need nothing beyond the ordinary 2-D ops.
    pub fn is_planar(&self) -> bool {
        let m = &self.0;
        m[2] == 0.0 && m[6] == 0.0 && m[14] == 0.0 && m[10] == 1.0 && m[11] == 0.0
    }

    /// Drop this matrix's own depth: children land in the node's plane. CSS
    /// `transform-style: flat`.
    ///
    /// shortcut: only the z output is zeroed, so a child's own `translateZ`
    /// stops mattering but an ancestor's perspective stays. Enough for flat
    /// containers that merely group 3-D children; revisit if a flat node ever
    /// sits *inside* a rotated 3-D subtree.
    pub fn flatten_depth(&self) -> Mat4 {
        let mut m = self.0;
        m[2] = 0.0;
        m[6] = 0.0;
        m[14] = 0.0;
        Mat4(m)
    }

    /// Transformed z of the point `(x, y, 0)`. Used to order siblings inside a
    /// 3-D rendering context (painter's algorithm on the transformed depth).
    pub fn z_at(&self, x: f32, y: f32) -> f32 {
        let m = &self.0;
        m[2] * x + m[6] * y + m[14]
    }
}

/// Product of two column-major 3×3 homographies: `a · b`, so `b` applies first.
pub fn mul3(a: [f32; 9], b: [f32; 9]) -> [f32; 9] {
    let mut out = [0.0f32; 9];
    for col in 0..3 {
        for row in 0..3 {
            let mut sum = 0.0;
            for k in 0..3 {
                sum += a[k * 3 + row] * b[col * 3 + k];
            }
            out[col * 3 + row] = sum;
        }
    }
    out
}

/// Inverse of a column-major 3×3 homography. `None` when it is singular (a
/// degenerate parent transform) — the caller then skips the subtree.
pub fn invert3(m: [f32; 9]) -> Option<[f32; 9]> {
    // m laid out as [a, d, g, b, e, h, c, f, i] (column-major).
    let (a, b, c) = (m[0], m[3], m[6]);
    let (d, e, f) = (m[1], m[4], m[7]);
    let (g, h, i) = (m[2], m[5], m[8]);
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    Some([
        (e * i - f * h) * inv,
        (f * g - d * i) * inv,
        (d * h - e * g) * inv,
        (c * h - b * i) * inv,
        (a * i - c * g) * inv,
        (b * g - a * h) * inv,
        (b * f - c * e) * inv,
        (c * d - a * f) * inv,
        (a * e - b * d) * inv,
    ])
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

    #[test]
    fn mat4_identity_is_neutral() {
        let r = Mat4::rotate_y_deg(37.0);
        assert_eq!(Mat4::IDENTITY.mul(&r), r);
        assert_eq!(r.mul(&Mat4::IDENTITY), r);
    }

    #[test]
    fn mat4_translate_z_moves_depth_only() {
        let m = Mat4::translate(1.0, 2.0, 3.0);
        assert_eq!(m.z_at(0.0, 0.0), 3.0);
        // The projected homography of a pure translate keeps x/y linear.
        let h = m.to_homography3();
        assert_eq!([h[6], h[7], h[8]], [1.0, 2.0, 1.0]);
    }

    #[test]
    fn mat4_rotate_y_quarter_turn_maps_x_to_minus_z() {
        // rotateY(90) sends +x to -z (away from the viewer).
        let m = Mat4::rotate_y_deg(90.0);
        assert!((m.z_at(1.0, 0.0) + 1.0).abs() < 1e-6, "z={}", m.z_at(1.0, 0.0));
        assert!(m.z_at(0.0, 0.0).abs() < 1e-6);
        // rotateY(0) leaves depth alone.
        assert_eq!(Mat4::rotate_y_deg(0.0).z_at(5.0, 5.0), 0.0);
    }

    #[test]
    fn mat4_perspective_shrinks_far_points() {
        // CSS `perspective(900)` on a point pushed 400px toward the viewer:
        // w = 1 - z/d, so the point scales up by d/(d - z) = 900/500.
        let m = Mat4::perspective(900.0).mul(&Mat4::translate(100.0, 0.0, 400.0));
        let h = m.to_homography3();
        let w = h[2] * 100.0 + h[8];
        assert!((w - (1.0 - 400.0 / 900.0)).abs() < 1e-5, "w={w}");
        assert!((h[0] * 100.0 / w - 100.0 * 900.0 / 500.0).abs() < 1e-2);
        // Pushed away (z = -400) it shrinks instead.
        let far = Mat4::perspective(900.0).mul(&Mat4::translate(100.0, 0.0, -400.0));
        let hf = far.to_homography3();
        assert!(hf[0] * 100.0 / (hf[2] * 100.0 + hf[8]) < 100.0);
    }

    #[test]
    fn invert3_round_trips() {
        let m = Mat4::perspective(900.0)
            .mul(&Mat4::rotate_y_deg(-30.0))
            .mul(&Mat4::rotate_x_deg(25.0))
            .mul(&Mat4::translate(12.0, -7.0, 400.0));
        let h = m.to_homography3();
        let inv = invert3(h).expect("non-singular");
        // inv · h is the identity homography.
        let mut id = [0.0f32; 9];
        for col in 0..3 {
            for row in 0..3 {
                let mut sum = 0.0;
                for k in 0..3 {
                    sum += inv[k * 3 + row] * h[col * 3 + k];
                }
                id[col * 3 + row] = sum;
            }
        }
        let want = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        for (got, want) in id.iter().zip(want.iter()) {
            assert!((got - want).abs() < 1e-4, "id={id:?}");
        }
    }

    #[test]
    fn invert3_rejects_singular() {
        assert!(invert3([1.0, 2.0, 0.0, 2.0, 4.0, 0.0, 0.0, 0.0, 1.0]).is_none());
    }
}
