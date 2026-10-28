//! Initial cubic construction with continuous midpoint tangents.

use super::{add, corners, midpoint, mul, norm, sub, unit};
use crate::assemble::{Point, Seg};

/// Segment with explicit start, used internally by the global optimizer.
#[derive(Clone, Debug)]
pub struct Piece {
    pub start: Point,
    pub seg: Seg,
}

fn push(pieces: &mut Vec<Piece>, current: &mut Point, seg: Seg) {
    pieces.push(Piece {
        start: *current,
        seg: seg.clone(),
    });
    *current = seg.end();
}

/// Convert the polygon into midpoint curves and straight corner pieces.
pub fn initial(points: &[Point], sharp: &[bool], closed: bool) -> Vec<Piece> {
    if points.is_empty() || sharp.len() != points.len() {
        return Vec::new();
    }
    if points.len() < 3 {
        return vec![Piece {
            start: points[0],
            seg: Seg::Line(points[points.len() - 1]),
        }];
    }
    let last = points.len() - 1;
    let mut result = Vec::new();
    let mut current = points[0];
    let first_mid = midpoint(points[0], points[1]);
    let tangent = if closed {
        unit(sub(points[1], points[last - 1]))
    } else {
        unit(sub(points[1], points[0]))
    };
    if closed && !sharp[0] {
        let length = norm(sub(first_mid, current)) / 3.0;
        let c1 = add(current, mul(tangent, length));
        let c2 = sub(first_mid, mul(unit(sub(points[1], points[0])), length));
        push(&mut result, &mut current, Seg::Cubic(c1, c2, first_mid));
    } else {
        push(&mut result, &mut current, Seg::Line(first_mid));
    }
    for i in 1..last {
        let v = points[i];
        let after = midpoint(v, points[i + 1]);
        if sharp[i] {
            push(&mut result, &mut current, Seg::Line(v));
            push(&mut result, &mut current, Seg::Line(after));
        } else {
            let alpha = corners::alpha(current, v, after).clamp(0.55, 1.0);
            let c1 = add(current, mul(sub(v, current), alpha));
            let c2 = add(after, mul(sub(v, after), alpha));
            push(&mut result, &mut current, Seg::Cubic(c1, c2, after));
        }
    }
    if closed && !sharp[0] {
        let length = norm(sub(points[last], current)) / 3.0;
        let c1 = add(
            current,
            mul(unit(sub(points[last], points[last - 1])), length),
        );
        let c2 = sub(points[last], mul(tangent, length));
        push(&mut result, &mut current, Seg::Cubic(c1, c2, points[last]));
    } else {
        push(&mut result, &mut current, Seg::Line(points[last]));
    }
    result
}

/// Evaluate a cubic and its first two derivatives.
pub fn evaluate(p: [Point; 4], t: f64) -> (Point, Point, Point) {
    let u = 1.0 - t;
    let point = add(
        add(mul(p[0], u.powi(3)), mul(p[1], 3.0 * u * u * t)),
        add(mul(p[2], 3.0 * u * t * t), mul(p[3], t.powi(3))),
    );
    let derivative = add(
        add(
            mul(sub(p[1], p[0]), 3.0 * u * u),
            mul(sub(p[2], p[1]), 6.0 * u * t),
        ),
        mul(sub(p[3], p[2]), 3.0 * t * t),
    );
    let second = add(
        mul(add(sub(p[2], mul(p[1], 2.0)), p[0]), 6.0 * u),
        mul(add(sub(p[3], mul(p[2], 2.0)), p[1]), 6.0 * t),
    );
    (point, derivative, second)
}
