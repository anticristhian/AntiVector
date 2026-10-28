//! Shared-edge polygon, corner and Bézier fitting, derived from published mathematics.
//! Selinger (2003): <https://potrace.sourceforge.net/potrace.pdf>.
//! No Potrace implementation source was used.

pub mod bezier;
pub mod corners;
pub mod optimize;
pub mod polygon;

use crate::{
    assemble::{FittedEdge, Point, Seg},
    boundary::graph::BoundaryGraph,
    Control, CoreError, Params, Stage,
};
use rayon::prelude::*;

pub(crate) fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
pub(crate) fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}
pub(crate) fn mul(a: Point, v: f64) -> Point {
    [a[0] * v, a[1] * v]
}
pub(crate) fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
pub(crate) fn cross(a: Point, b: Point) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
pub(crate) fn norm(a: Point) -> f64 {
    a[0].hypot(a[1])
}
pub(crate) fn unit(a: Point) -> Point {
    let n = norm(a);
    if n > 1e-12 {
        mul(a, 1.0 / n)
    } else {
        [1.0, 0.0]
    }
}
pub(crate) fn midpoint(a: Point, b: Point) -> Point {
    mul(add(a, b), 0.5)
}

fn lossless_lines(points: &[Point]) -> Vec<Seg> {
    let mut anchors = vec![points[0]];
    for &point in points.iter().skip(1) {
        if anchors.len() > 1 {
            let n = anchors.len();
            let a = sub(anchors[n - 1], anchors[n - 2]);
            let b = sub(point, anchors[n - 1]);
            if cross(a, b).abs() < 1e-12 && dot(a, b) > 0.0 {
                anchors[n - 1] = point;
                continue;
            }
        }
        anchors.push(point);
    }
    anchors.into_iter().skip(1).map(Seg::Line).collect()
}

fn fitted_chain(
    points: &[Point],
    params: &Params,
    pixel_factor: Option<f64>,
    ctl: &Control,
) -> Result<Vec<Seg>, CoreError> {
    let mut polygon = polygon::optimal(points, ctl)?;
    corners::recover_bridges(
        &mut polygon.points,
        &mut polygon.originals,
        &mut polygon.supports,
        params.experimental.corner_recovery_radius,
    );
    let flags = if let Some(factor) = pixel_factor {
        corners::pixel_corners(&polygon.points, polygon.closed, factor)
    } else {
        corners::classify(&polygon.points, polygon.closed, params)
    };
    let initial = bezier::initial(&polygon.points, &flags, polygon.closed);
    optimize::optimize(&initial, f64::from(params.fit_tolerance_px), ctl)
}

fn straight_aware(
    points: &[Point],
    span: f64,
    params: &Params,
    ctl: &Control,
) -> Result<Vec<Seg>, CoreError> {
    let mut anchors = vec![0, points.len() - 1];
    let mut first = 0;
    while first + 1 < points.len() {
        let direction = sub(points[first + 1], points[first]);
        let mut last = first + 1;
        while last + 1 < points.len() && sub(points[last + 1], points[last]) == direction {
            last += 1;
        }
        let length = norm(sub(points[last], points[first]));
        if length >= 16.0 && length >= span * 0.5 {
            let trim = ((last - first) / 10).max(2);
            if first + trim < last - trim {
                anchors.push(first + trim);
                anchors.push(last - trim);
            }
        }
        first = last;
    }
    anchors.sort_unstable();
    anchors.dedup();
    if anchors.len() == 2 {
        return fitted_chain(points, params, None, ctl);
    }
    let mut output = Vec::new();
    for interval in anchors.windows(2) {
        let chain = &points[interval[0]..=interval[1]];
        let chord = sub(chain[chain.len() - 1], chain[0]);
        if chain
            .iter()
            .all(|&p| cross(chord, sub(p, chain[0])).abs() < 1e-12)
        {
            output.extend(lossless_lines(chain));
        } else {
            output.extend(fitted_chain(chain, params, None, ctl)?);
        }
    }
    Ok(output)
}

fn pixel_factor(points: &[Point]) -> f64 {
    let gcd = |mut a: usize, mut b: usize| {
        while b != 0 {
            let next = a % b;
            a = b;
            b = next;
        }
        a
    };
    let mut factor = 0;
    let mut run = 1;
    for triple in points.windows(3) {
        if sub(triple[1], triple[0]) == sub(triple[2], triple[1]) {
            run += 1;
        } else {
            factor = gcd(factor, run);
            run = 1;
        }
    }
    gcd(factor, run).max(1) as f64
}

/// Apply conservative coverage guards for transparent or closed colored interfaces.
/// This avoids AA conflation in the non-overlapping SVG reference mode. F5 can
/// relax the guard only after measuring an export/AA solution with the harness.
#[tracing::instrument(skip_all)]
pub fn fit_edges_for_regions(
    graph: &BoundaryGraph,
    map: &crate::segment::RegionMap,
    palette: &crate::color::Palette,
    params: &Params,
    pixel_art: bool,
    ctl: &Control,
) -> Result<Vec<FittedEdge>, CoreError> {
    fit_edges_for_regions_at_scale(graph, map, palette, params, pixel_art, 1, ctl)
}

/// Fit in original pixel units after preprocessing enlarged the segmentation.
/// Return coordinates in the graph's units, preserving junctions exactly.
pub fn fit_edges_for_regions_at_scale(
    graph: &BoundaryGraph,
    map: &crate::segment::RegionMap,
    palette: &crate::color::Palette,
    params: &Params,
    pixel_art: bool,
    scale: u32,
    ctl: &Control,
) -> Result<Vec<FittedEdge>, CoreError> {
    use crate::color::TRANSPARENT;
    map.validate()?;
    let mut preserve = vec![false; graph.edges.len()];
    for (i, edge) in graph.edges.iter().enumerate() {
        if let (Some(left), Some(right)) = (edge.left_region, edge.right_region) {
            let left = map
                .regions
                .get(left as usize)
                .ok_or_else(|| CoreError::InvalidParams("edge region reference".into()))?
                .color;
            let right = map
                .regions
                .get(right as usize)
                .ok_or_else(|| CoreError::InvalidParams("edge region reference".into()))?
                .color;
            if left == TRANSPARENT || right == TRANSPARENT {
                preserve[i] = true;
                continue;
            }
            let a = palette
                .colors
                .get(left as usize)
                .ok_or_else(|| CoreError::InvalidParams("edge palette reference".into()))?
                .rgb;
            let b = palette
                .colors
                .get(right as usize)
                .ok_or_else(|| CoreError::InvalidParams("edge palette reference".into()))?
                .rgb;
            preserve[i] = !params.experimental.quality_v5
                && edge.start == edge.end
                && !a.iter().all(|&v| v >= 240)
                && !b.iter().all(|&v| v >= 240);
        }
    }
    // In a nested colored component even its outer boundary participates in the
    // same compositing artifact. Keep closed loops conservative as a group.
    if preserve.iter().any(|&fixed| fixed) {
        for (fixed, edge) in preserve.iter_mut().zip(&graph.edges) {
            *fixed |= edge.start == edge.end;
        }
    }
    fit_impl(graph, params, pixel_art, &preserve, scale, ctl)
}

/// Fit every boundary exactly once; return geometry in dense edge-ID order.
/// Pixel-art retains grid corners with only lossless collinear simplification.
#[tracing::instrument(skip_all)]
pub fn fit_edges(
    graph: &BoundaryGraph,
    params: &Params,
    pixel_art: bool,
    ctl: &Control,
) -> Result<Vec<FittedEdge>, CoreError> {
    fit_impl(graph, params, pixel_art, &[], 1, ctl)
}

fn fit_impl(
    graph: &BoundaryGraph,
    params: &Params,
    pixel_art: bool,
    preserve: &[bool],
    scale: u32,
    ctl: &Control,
) -> Result<Vec<FittedEdge>, CoreError> {
    params.validate()?;
    if scale == 0 {
        return Err(CoreError::InvalidParams("zero fitting scale".into()));
    }
    let scale = f64::from(scale);
    ctl.report(Stage::FitEdges, 0.0)?;
    for (i, edge) in graph.edges.iter().enumerate() {
        if edge.id as usize != i
            || edge.pts.len() < 2
            || edge.start as usize >= graph.junctions.len()
            || edge.end as usize >= graph.junctions.len()
        {
            return Err(CoreError::InvalidParams("malformed boundary graph".into()));
        }
        if edge.pts[0] != graph.junctions[edge.start as usize].point
            || *edge.pts.last().unwrap() != graph.junctions[edge.end as usize].point
        {
            return Err(CoreError::InvalidParams(
                "boundary endpoints do not match junctions".into(),
            ));
        }
    }
    let mut order: Vec<_> = graph.edges.iter().collect();
    order.sort_by_key(|edge| (std::cmp::Reverse(edge.pts.len()), edge.id));
    let mut fitted: Vec<FittedEdge> = order
        .par_iter()
        .map(|edge| {
            ctl.check()?;
            // Keep the finer sampling for details that fit in one polygon
            // search window; normalize longer chains to source pixel units.
            let scale = if edge.pts.len() <= 200 { 1.0 } else { scale };
            let mut normalized = params.clone();
            normalized.fit_tolerance_px /= scale as f32;
            normalized.experimental.corner_recovery_radius /= scale;
            let params = &normalized;
            let points: Vec<_> = edge
                .pts
                .iter()
                .map(|p| [f64::from(p.x) / scale, f64::from(p.y) / scale])
                .collect();
            let span = |axis: usize| {
                points
                    .iter()
                    .map(|p| p[axis])
                    .fold(f64::NEG_INFINITY, f64::max)
                    - points.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min)
            };
            let thin = span(0).min(span(1)) <= 2.0;
            let mut segs = if edge.on_frame
                || pixel_art
                    && params.experimental.pixel_art_strategy
                        == crate::params::PixelArtStrategy::PreserveGrid
                || thin
                || preserve.get(edge.id as usize).copied().unwrap_or(false)
            {
                lossless_lines(&points)
            } else if pixel_art {
                fitted_chain(&points, params, Some(pixel_factor(&points)), ctl)?
            } else {
                straight_aware(&points, span(0).min(span(1)), params, ctl)?
            };
            let rescale = |p: &mut Point| {
                p[0] *= scale;
                p[1] *= scale;
            };
            for seg in &mut segs {
                match seg {
                    Seg::Line(p) => rescale(p),
                    Seg::Cubic(a, b, p) => {
                        rescale(a);
                        rescale(b);
                        rescale(p);
                    }
                }
            }
            Ok(FittedEdge {
                id: edge.id,
                start: [points[0][0] * scale, points[0][1] * scale],
                segs,
            })
        })
        .collect::<Result<_, CoreError>>()?;
    fitted.sort_by_key(|edge| edge.id);
    ctl.report(Stage::FitEdges, 1.0)?;
    Ok(fitted)
}
