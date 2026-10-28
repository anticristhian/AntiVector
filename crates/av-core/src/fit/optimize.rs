//! Globally minimize smooth-run node count with fixed tangent least squares.

use super::{
    add,
    bezier::{evaluate, Piece},
    cross, dot, mul, norm, sub, unit,
};
use crate::{
    assemble::{Point, Seg},
    Control, CoreError,
};

fn cubic(piece: &Piece) -> Option<[Point; 4]> {
    match piece.seg {
        Seg::Cubic(a, b, end) => Some([piece.start, a, b, end]),
        _ => None,
    }
}

fn solve(
    samples: &[Point],
    parameters: &[f64],
    start: Point,
    end: Point,
    t0: Point,
    t1: Point,
) -> Option<[Point; 4]> {
    let mut a = 0.0;
    let mut b = 0.0;
    let mut c = 0.0;
    let mut d = 0.0;
    let mut e = 0.0;
    for (&point, &t) in samples.iter().zip(parameters) {
        let u = 1.0 - t;
        let b0 = u.powi(3);
        let b1 = 3.0 * u * u * t;
        let b2 = 3.0 * u * t * t;
        let b3 = t.powi(3);
        let residual = sub(point, add(mul(start, b0 + b1), mul(end, b2 + b3)));
        let v0 = mul(t0, b1);
        let v1 = mul(t1, -b2);
        a += dot(v0, v0);
        b += dot(v0, v1);
        c += dot(v1, v1);
        d += dot(v0, residual);
        e += dot(v1, residual);
    }
    let det = a * c - b * b;
    if det.abs() < 1e-12 {
        return None;
    }
    let u = (d * c - e * b) / det;
    let v = (e * a - d * b) / det;
    if u <= 0.0 || v <= 0.0 || !u.is_finite() || !v.is_finite() {
        return None;
    }
    Some([start, add(start, mul(t0, u)), sub(end, mul(t1, v)), end])
}

fn error(curve: [Point; 4], samples: &[Point], ts: &[f64]) -> (f64, f64) {
    let mut maximum: f64 = 0.0;
    let mut sum = 0.0;
    for (&p, &initial) in samples.iter().zip(ts) {
        let mut t = initial;
        for _ in 0..5 {
            let (q, d, dd) = evaluate(curve, t);
            let residual = sub(q, p);
            let denominator = dot(d, d) + dot(residual, dd);
            if denominator.abs() < 1e-12 {
                break;
            }
            t = (t - dot(residual, d) / denominator).clamp(0.0, 1.0);
        }
        let distance = norm(sub(evaluate(curve, t).0, p));
        maximum = maximum.max(distance);
        sum += distance * distance;
    }
    (maximum, sum)
}

fn candidate(pieces: &[Piece]) -> Option<(Seg, f64, f64)> {
    let first = cubic(&pieces[0])?;
    let last = cubic(pieces.last()?)?;
    let start = first[0];
    let end = last[3];
    let chord = sub(end, start);
    let length = norm(chord);
    if length < 1e-9 {
        return None;
    }
    let t0 = unit(sub(first[1], start));
    let t1 = unit(sub(end, last[2]));
    let mut samples = vec![start];
    let mut total_turn = 0.0;
    let mut sign = 0;
    for piece in pieces {
        let curve = cubic(piece)?;
        let mut previous = unit(sub(curve[1], curve[0]));
        for step in 1..=8 {
            let (p, tangent, second) = evaluate(curve, step as f64 / 8.0);
            let tangent = unit(tangent);
            let turn = cross(previous, tangent).atan2(dot(previous, tangent));
            total_turn += turn.abs();
            previous = tangent;
            let curvature = cross(tangent, second);
            if curvature.abs() > 1e-8 {
                let current = if curvature > 0.0 { 1 } else { -1 };
                if sign != 0 && sign != current {
                    return None;
                }
                sign = current;
            }
            samples.push(p);
        }
    }
    // Large near-semicircle replacements are ill-conditioned with fixed endpoint
    // tangents; cap turning to 120° rather than spending the whole error budget
    // on one arc. This preserves the requested tolerance as an upper bound.
    if total_turn > 120_f64.to_radians() {
        return None;
    }
    let normal = [-chord[1] / length, chord[0] / length];
    let deviation = samples
        .iter()
        .map(|&p| dot(normal, sub(p, start)).abs())
        .fold(0.0, f64::max);
    if length >= 3.0
        && deviation <= 0.35
        && dot(unit(chord), t0) > 0.9999
        && dot(unit(chord), t1) > 0.9999
    {
        return Some((Seg::Line(end), deviation, deviation * deviation));
    }
    let mut ts = vec![0.0; samples.len()];
    for i in 1..samples.len() {
        ts[i] = ts[i - 1] + norm(sub(samples[i], samples[i - 1]));
    }
    let total = *ts.last().unwrap();
    for t in &mut ts {
        *t /= total;
    }
    let mut curve = solve(&samples, &ts, start, end, t0, t1).unwrap_or_else(|| {
        // Bounded 16x16 grid fallback for singular or non-positive LS solutions.
        let mut best = [
            start,
            add(start, mul(t0, length / 3.0)),
            sub(end, mul(t1, length / 3.0)),
            end,
        ];
        let mut penalty = f64::INFINITY;
        for i in 1..=16 {
            for j in 1..=16 {
                let candidate = [
                    start,
                    add(start, mul(t0, length * i as f64 / 8.0)),
                    sub(end, mul(t1, length * j as f64 / 8.0)),
                    end,
                ];
                let e = error(candidate, &samples, &ts).1;
                if e < penalty {
                    penalty = e;
                    best = candidate;
                }
            }
        }
        best
    });
    for _ in 0..3 {
        let mut previous = 0.0;
        for i in 1..ts.len() - 1 {
            let (p, d, dd) = evaluate(curve, ts[i]);
            let r = sub(p, samples[i]);
            let denominator = dot(d, d) + dot(r, dd);
            if denominator.abs() > 1e-12 {
                ts[i] = (ts[i] - dot(r, d) / denominator).clamp(previous, 1.0);
            }
            previous = ts[i];
        }
        if let Some(next) = solve(&samples, &ts, start, end, t0, t1) {
            curve = next;
        }
    }
    // A convex source run must not acquire an inflection in its replacement.
    for step in 0..=16 {
        let (_, d, dd) = evaluate(curve, step as f64 / 16.0);
        let curvature = cross(d, dd);
        if curvature.abs() > 1e-8 && sign != 0 && (if curvature > 0.0 { 1 } else { -1 }) != sign {
            return None;
        }
    }
    let (maximum, penalty) = error(curve, &samples, &ts);
    Some((Seg::Cubic(curve[1], curve[2], end), maximum, penalty))
}

fn smooth_run(pieces: &[Piece], tolerance: f64, ctl: &Control) -> Result<Vec<Seg>, CoreError> {
    let n = pieces.len();
    let mut best = vec![(usize::MAX, f64::INFINITY); n + 1];
    let mut previous = vec![0; n + 1];
    let mut selected: Vec<Option<Seg>> = vec![None; n + 1];
    best[0] = (0, 0.0);
    for end in 1..=n {
        ctl.check()?;
        for start in end.saturating_sub(32)..end {
            let fit = if end - start == 1 {
                Some((pieces[start].seg.clone(), 0.0, 0.0))
            } else {
                candidate(&pieces[start..end])
            };
            if let Some((seg, error, penalty)) = fit {
                if error > tolerance {
                    continue;
                }
                let cost = (best[start].0 + 1, best[start].1 + penalty);
                if cost.0 < best[end].0 || cost.0 == best[end].0 && cost.1 < best[end].1 {
                    best[end] = cost;
                    previous[end] = start;
                    selected[end] = Some(seg);
                }
            }
        }
    }
    let mut result = Vec::new();
    let mut cursor = n;
    while cursor > 0 {
        result.push(
            selected[cursor]
                .take()
                .ok_or_else(|| CoreError::Internal("empty curve DP path".into()))?,
        );
        cursor = previous[cursor];
    }
    result.reverse();
    Ok(result)
}

/// Optimize each smooth run; corners and junction endpoints are never removed.
pub fn optimize(pieces: &[Piece], tolerance: f64, ctl: &Control) -> Result<Vec<Seg>, CoreError> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < pieces.len() {
        if matches!(pieces[i].seg, Seg::Cubic(..)) {
            let start = i;
            while i < pieces.len() && matches!(pieces[i].seg, Seg::Cubic(..)) {
                i += 1;
            }
            result.extend(smooth_run(&pieces[start..i], tolerance, ctl)?);
        } else {
            result.push(pieces[i].seg.clone());
            i += 1;
        }
    }
    // Remove midpoint anchors on straight runs without changing geometry.
    let mut output = Vec::new();
    let mut start = pieces.first().map_or([0.0; 2], |p| p.start);
    for seg in result {
        if let (Some(Seg::Line(previous)), Seg::Line(end)) = (output.last_mut(), &seg) {
            let a = sub(*previous, start);
            let b = sub(*end, *previous);
            if cross(a, b).abs() <= 1e-9 * norm(a).max(1.0) * norm(b).max(1.0) && dot(a, b) > 0.0 {
                *previous = *end;
                continue;
            }
        }
        if let Some(previous) = output.last() {
            start = previous.end();
        }
        output.push(seg);
    }
    Ok(output)
}
