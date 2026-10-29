//! Opt-in circular-loop fitting from centered normal equations.
//! Only the artificial seam of an isolated closed edge may move; real junctions
//! and non-circular contours are handled by the general shared-edge fitter.
use super::{add, mul, norm, sub};
use crate::assemble::{Point, Seg};

pub(super) fn fit(
    points: &[Point],
    tolerance: f64,
    physical_unit: f64,
    frame: Point,
) -> Option<(Point, Vec<Seg>)> {
    if points.len() < 13 || points.first() != points.last() {
        return None;
    }
    let points = &points[..points.len() - 1];
    let n = points.len() as f64;
    let mean = points
        .iter()
        .fold([0.0, 0.0], |sum, &p| add(sum, mul(p, 1.0 / n)));
    let (mut xx, mut xy, mut yy, mut xq, mut yq, mut qq) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for &p in points {
        let [x, y] = sub(p, mean);
        let q = x * x + y * y;
        xx += x * x;
        xy += x * y;
        yy += y * y;
        xq += x * q;
        yq += y * q;
        qq += q;
    }
    let det = xx * yy - xy * xy;
    if !det.is_finite() || det <= 1e-12 * xx * yy {
        return None;
    }
    let offset = [
        (xq * yy - yq * xy) / (2.0 * det),
        (yq * xx - xq * xy) / (2.0 * det),
    ];
    let center = add(mean, offset);
    let radius = (qq / n + offset[0] * offset[0] + offset[1] * offset[1]).sqrt();
    if !radius.is_finite() || !(4.0..=256.0).contains(&(radius * physical_unit)) {
        return None;
    }
    if center[0] - radius < 0.0
        || center[1] - radius < 0.0
        || center[0] + radius > frame[0]
        || center[1] + radius > frame[1]
    {
        return None;
    }
    let residual = points
        .iter()
        .map(|&p| (norm(sub(p, center)) - radius).abs())
        .fold(0.0, f64::max);
    // Reserve the quarter-cubic's conservative 300ppm radial error as well as
    // the measured source residual; never exceed the user's fitting budget.
    if residual + radius * 0.0003 > tolerance.min(0.75 / physical_unit) {
        return None;
    }
    let mut area = 0.0;
    for i in 0..points.len() {
        let a = sub(points[i], mean);
        let b = sub(points[(i + 1) % points.len()], mean);
        area += a[0] * b[1] - a[1] * b[0];
    }
    if area.abs() < 1e-9 {
        return None;
    }
    let delta = sub(points[0], center);
    let length = norm(delta);
    if length <= 1e-12 {
        return None;
    }
    let mut direction = mul(delta, 1.0 / length);
    let start = add(center, mul(direction, radius));
    let sign = area.signum();
    let k = 4.0 / 3.0 * (std::f64::consts::PI / 8.0).tan();
    let mut current = start;
    let mut segments = Vec::with_capacity(4);
    for quarter in 0..4 {
        let next = [-sign * direction[1], sign * direction[0]];
        let end = if quarter == 3 {
            start
        } else {
            add(center, mul(next, radius))
        };
        segments.push(Seg::Cubic(
            add(current, mul(next, k * radius)),
            add(end, mul(direction, k * radius)),
            end,
        ));
        current = end;
        direction = next;
    }
    Some((start, segments))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circles_close_smoothly_in_both_directions_and_ellipses_are_rejected() {
        for sign in [-1.0, 1.0] {
            let points: Vec<_> = (0..=128)
                .map(|i| {
                    if i == 128 {
                        return [80.0, 50.0];
                    }
                    let angle = sign * i as f64 * std::f64::consts::TAU / 128.0;
                    [50.0 + 30.0 * angle.cos(), 50.0 + 30.0 * angle.sin()]
                })
                .collect();
            let (start, segments) = fit(&points, 0.5, 1.0, [100.0, 100.0]).unwrap();
            assert_eq!(segments.len(), 4);
            assert_eq!(segments.last().unwrap().end(), start);
            let mut previous = start;
            for segment in segments {
                let Seg::Cubic(a, b, end) = segment else {
                    panic!("expected cubic")
                };
                for i in 0..=1000 {
                    let p =
                        super::super::bezier::evaluate([previous, a, b, end], i as f64 / 1000.0).0;
                    assert!((norm(sub(p, [50.0, 50.0])) - 30.0).abs() < 30.0 * 0.0003);
                }
                previous = end;
            }
            let ellipse: Vec<_> = points
                .iter()
                .map(|p| [50.0 + (p[0] - 50.0) * 0.5, p[1]])
                .collect();
            assert!(fit(&ellipse, 0.9, 1.0, [100.0, 100.0]).is_none());
        }
    }
}
