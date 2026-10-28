//! Geometric corner analysis at polygon scale, independent of individual stairs.

use super::{add, cross, dot, midpoint, mul, norm, sub};
use crate::{assemble::Point, Params};

/// Recover one sharp intersection when a raster corner appears as a short bridge
/// between two much longer sides. The retained vertex stays inside its original
/// configured box (0.5px by default); junction endpoints cannot move.
pub fn recover_bridges(
    points: &mut Vec<Point>,
    originals: &mut Vec<Point>,
    supports: &mut Vec<[Point; 2]>,
    radius: f64,
) {
    if points.len() != originals.len() || supports.len() + 1 != points.len() {
        return;
    }
    loop {
        let mut changed = false;
        for i in 1..points.len().saturating_sub(2) {
            let [a, b] = supports[i - 1];
            let [c, d] = supports[i + 1];
            let incoming = sub(b, a);
            let bridge = sub(points[i + 1], points[i]);
            let outgoing = sub(d, c);
            let short = norm(bridge);
            if norm(incoming) < 4.0 * short || norm(outgoing) < 4.0 * short {
                continue;
            }
            let det = cross(incoming, outgoing);
            if det.abs() < 0.3 * norm(incoming) * norm(outgoing) {
                continue;
            }
            let intersection = add(a, mul(incoming, cross(sub(c, a), outgoing) / det));
            let retained = [i, i + 1]
                .into_iter()
                .map(|j| {
                    let candidate = super::polygon::corner_in_box(originals[j], a, b, c, d, radius);
                    (j, candidate, norm(sub(candidate, intersection)))
                })
                .filter(|(_, _, distance)| *distance <= 1.5)
                .min_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((j, candidate, _)) = retained {
                let original = originals[j];
                points[i] = candidate;
                originals[i] = original;
                points.remove(i + 1);
                originals.remove(i + 1);
                supports.remove(i);
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    // A closed contour may begin just before a clipped tip. Keep that junction
    // fixed and recover the nearby geometric corner in the next vertex's box.
    if points.len() >= 5 && points.first() == points.last() {
        let last = points.len() - 1;
        let [a, b] = supports[last - 1];
        let [c, d] = supports[1];
        let short = norm(sub(points[1], points[0]));
        let u = sub(b, a);
        let v = sub(d, c);
        if short < radius + 1.5
            && norm(u) >= 4.0 * short
            && norm(v) >= 4.0 * short
            && cross(u, v).abs() > 0.3 * norm(u) * norm(v)
        {
            points[1] = super::polygon::corner_in_box(originals[1], a, b, c, d, radius);
        }
    }
}

/// Alpha from the half-pixel square between a vertex and its midpoint chord.
pub fn alpha(a: Point, v: Point, b: Point) -> f64 {
    let chord = sub(b, a);
    let len = norm(chord);
    if len < 1e-12 {
        return 4.0 / 3.0;
    }
    let normal = [-chord[1] / len, chord[0] / len];
    let distance = dot(normal, sub(v, a)).abs();
    let radius = 0.5 * (normal[0].abs() + normal[1].abs());
    if distance <= radius {
        0.0
    } else {
        4.0 / 3.0 * (1.0 - radius / distance)
    }
}

/// Mark sharp polygon vertices; closed artificial endpoints remain fixed anchors,
/// but can have continuous tangents when they are not geometric corners.
pub fn classify(points: &[Point], closed: bool, p: &Params) -> Vec<bool> {
    if points.len() < 3 {
        return vec![true; points.len()];
    }
    let mut corners = vec![false; points.len()];
    let last = points.len() - 1;
    for (i, corner) in corners.iter_mut().enumerate() {
        if !closed && (i == 0 || i == last) {
            *corner = true;
            continue;
        }
        let index = if i == last { 0 } else { i };
        let prev = if index == 0 { last - 1 } else { index - 1 };
        let next = index + 1;
        let mut a = points[prev];
        let v = points[index];
        let mut b = points[next];
        if closed {
            if index == 0 && (norm(sub(a, v)) < 2.0 || norm(sub(b, v)) < 2.0) {
                *corner = false;
                continue;
            }
            let mut at = prev;
            while norm(sub(a, v)) < 2.0 {
                at = (at + last - 1) % last;
                if at == index {
                    break;
                }
                a = points[at];
            }
            let mut at = next % last;
            while norm(sub(b, v)) < 2.0 {
                at = (at + 1) % last;
                if at == index {
                    break;
                }
                b = points[at];
            }
        }
        let left = sub(a, v);
        let right = sub(b, v);
        let lengths = norm(left) * norm(right);
        let angle = if lengths > 1e-12 {
            (dot(left, right) / lengths)
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        } else {
            0.0
        };
        let curvature = alpha(midpoint(a, v), v, midpoint(v, b));
        *corner = curvature > 0.55 + 0.78 * f64::from(p.smoothness)
            || angle < f64::from(p.corner_threshold_deg) && norm(left) >= 2.0 && norm(right) >= 2.0;
    }
    corners
}

/// Simplified Kopf-inspired experiment: retain only scale-supported right angles.
/// It does not change connectivity; compared against exact grid preservation in F5.
pub fn pixel_corners(points: &[Point], closed: bool, factor: f64) -> Vec<bool> {
    if points.len() < 3 {
        return vec![true; points.len()];
    }
    let last = points.len() - 1;
    points
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            if !closed && (i == 0 || i == last) {
                return true;
            }
            let i = if i == last { 0 } else { i };
            let previous = if i == 0 { last - 1 } else { i - 1 };
            let next = i + 1;
            let a = sub(points[previous], v);
            let b = sub(points[next], v);
            let lengths = norm(a) * norm(b);
            lengths > 0.0
                && norm(a) + 0.5 >= factor
                && norm(b) + 0.5 >= factor
                && ((dot(a, b) / lengths).clamp(-1.0, 1.0).acos().to_degrees() - 90.0).abs() <= 10.0
        })
        .collect()
}
