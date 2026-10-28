//! Lexicographic shortest polygon: segment count first, squared fitting error second.
//! Straightness uses the article's max-distance half-pixel neighborhood, not a
//! Euclidean half-pixel radius. Polygon vertices are subsequently box-constrained.

use super::{add, dot, mul, norm, sub};
use crate::{assemble::Point, Control, CoreError};

/// Adjusted open polygon, or a closed polygon with repeated first/last vertex.
pub struct Polygon {
    pub points: Vec<Point>,
    pub originals: Vec<Point>,
    pub supports: Vec<[Point; 2]>,
    pub closed: bool,
}

#[derive(Clone, Copy)]
struct Line {
    normal: Point,
    offset: f64,
    error: f64,
}

fn line(points: &[Point]) -> Line {
    let n = points.len() as f64;
    let mut sum = [0.0; 5];
    for &[x, y] in points {
        sum[0] += x;
        sum[1] += y;
        sum[2] += x * x;
        sum[3] += x * y;
        sum[4] += y * y;
    }
    let mean = [sum[0] / n, sum[1] / n];
    let a = sum[2] / n - mean[0] * mean[0];
    let b = sum[3] / n - mean[0] * mean[1];
    let c = sum[4] / n - mean[1] * mean[1];
    let angle = 0.5 * (2.0 * b).atan2(a - c);
    let normal = [-angle.sin(), angle.cos()];
    let offset = dot(normal, mean);
    let error = points
        .iter()
        .map(|&p| (dot(normal, p) - offset).powi(2))
        .sum();
    Line {
        normal,
        offset,
        error,
    }
}

fn possible(points: &[Point]) -> Option<Line> {
    if points.len() == 2 {
        return Some(line(points));
    }
    let mut directions = 0_u8;
    for pair in points.windows(2) {
        let d = sub(pair[1], pair[0]);
        directions |= if d[0] > 0.0 {
            1
        } else if d[0] < 0.0 {
            2
        } else if d[1] > 0.0 {
            4
        } else {
            8
        };
    }
    if directions == 15 {
        return None;
    }
    let fitted = line(points);
    let bound = 0.5 * (fitted.normal[0].abs() + fitted.normal[1].abs()) + 1e-9;
    let lo = points
        .iter()
        .map(|&p| dot(fitted.normal, p))
        .fold(f64::INFINITY, f64::min);
    let hi = points
        .iter()
        .map(|&p| dot(fitted.normal, p))
        .fold(f64::NEG_INFINITY, f64::max);
    if hi - lo > 2.0 * bound {
        return None;
    }
    Some(fitted)
}

fn adjust(v: Point, a: Line, b: Line, radius: f64) -> Point {
    let q00 = a.normal[0] * a.normal[0] + b.normal[0] * b.normal[0];
    let q01 = a.normal[0] * a.normal[1] + b.normal[0] * b.normal[1];
    let q11 = a.normal[1] * a.normal[1] + b.normal[1] * b.normal[1];
    let rhs = add(mul(a.normal, a.offset), mul(b.normal, b.offset));
    let det = q00 * q11 - q01 * q01;
    let mut candidates = vec![v];
    if det.abs() > 1e-12 {
        candidates.push([
            (rhs[0] * q11 - rhs[1] * q01) / det,
            (rhs[1] * q00 - rhs[0] * q01) / det,
        ]);
    }
    for x in [v[0] - radius, v[0] + radius] {
        if q11 > 1e-12 {
            candidates.push([
                x,
                ((rhs[1] - q01 * x) / q11).clamp(v[1] - radius, v[1] + radius),
            ]);
        }
    }
    for y in [v[1] - radius, v[1] + radius] {
        if q00 > 1e-12 {
            candidates.push([
                ((rhs[0] - q01 * y) / q00).clamp(v[0] - radius, v[0] + radius),
                y,
            ]);
        }
    }
    let cost =
        |p: Point| (dot(a.normal, p) - a.offset).powi(2) + (dot(b.normal, p) - b.offset).powi(2);
    candidates
        .into_iter()
        .filter(|p| (p[0] - v[0]).abs() <= radius + 1e-9 && (p[1] - v[1]).abs() <= radius + 1e-9)
        .min_by(|&a, &b| cost(a).total_cmp(&cost(b)))
        .unwrap_or(v)
}

pub(super) fn corner_in_box(
    original: Point,
    a: Point,
    b: Point,
    c: Point,
    d: Point,
    radius: f64,
) -> Point {
    adjust(original, line(&[a, b]), line(&[c, d]), radius)
}

/// Find a minimum-segment polygon with a maximum span of 200 crack vertices.
/// Endpoints (including an artificial loop junction) are retained bit-for-bit.
pub fn optimal(points: &[Point], ctl: &Control) -> Result<Polygon, CoreError> {
    if points.len() < 2 || points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(CoreError::InvalidParams(
            "polygon requires two or more finite points".into(),
        ));
    }
    let n = points.len();
    let closed = points.first() == points.last();
    let mut best = vec![(usize::MAX, f64::INFINITY); n];
    let mut previous = vec![0; n];
    best[0] = (0, 0.0);
    for end in 1..n {
        if end % 64 == 0 {
            ctl.check()?;
        }
        for start in end.saturating_sub(200)..end {
            let permitted = if end - start == 1 {
                Some(line(&points[start..=end]))
            } else {
                let mut extended = Vec::with_capacity(end - start + 3);
                if start > 0 {
                    extended.push(points[start - 1]);
                }
                extended.extend_from_slice(&points[start..=end]);
                if end + 1 < n {
                    extended.push(points[end + 1]);
                }
                possible(&extended).map(|_| line(&points[start..=end]))
            };
            if let Some(line) = permitted {
                let candidate = (best[start].0 + 1, best[start].1 + line.error);
                if candidate.0 < best[end].0
                    || candidate.0 == best[end].0 && candidate.1 < best[end].1
                {
                    best[end] = candidate;
                    previous[end] = start;
                }
            }
        }
    }
    let mut indices = vec![n - 1];
    let mut cursor = n - 1;
    while cursor > 0 {
        cursor = previous[cursor];
        indices.push(cursor);
    }
    indices.reverse();
    let lines: Vec<_> = indices
        .windows(2)
        .map(|p| line(&points[p[0]..=p[1]]))
        .collect();
    let mut adjusted: Vec<_> = indices.iter().map(|&i| points[i]).collect();
    for i in 1..indices.len() - 1 {
        adjusted[i] = adjust(points[indices[i]], lines[i - 1], lines[i], 0.5);
    }
    // Collapse numerical duplicate vertices while keeping the fixed endpoints.
    let mut output = vec![adjusted[0]];
    let mut original_points = vec![points[indices[0]]];
    for (k, &p) in adjusted.iter().enumerate().skip(1) {
        if norm(sub(p, *output.last().unwrap())) > 1e-9 {
            output.push(p);
            original_points.push(points[indices[k]]);
        }
    }
    if output.len() < 2 {
        output = vec![points[0], points[n - 1]];
        original_points = output.clone();
    }
    let area = |points: &[Point]| {
        points
            .windows(2)
            .map(|p| p[0][0] * p[1][1] - p[1][0] * p[0][1])
            .sum::<f64>()
    };
    if closed
        && (area(&output) * area(points) <= 0.0 || area(&output).abs() < 0.1 * area(points).abs())
    {
        output = points.to_vec();
        original_points = output.clone();
    }
    let supports = if output.len() == indices.len() {
        indices
            .windows(2)
            .zip(&lines)
            .map(|(pair, line)| {
                [points[pair[0]], points[pair[1]]]
                    .map(|p| sub(p, mul(line.normal, dot(line.normal, p) - line.offset)))
            })
            .collect()
    } else {
        output.windows(2).map(|p| [p[0], p[1]]).collect()
    };
    Ok(Polygon {
        points: output,
        originals: original_points,
        supports,
        closed,
    })
}
