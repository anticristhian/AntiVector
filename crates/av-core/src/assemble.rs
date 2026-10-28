//! Shared fitted geometry and assembly of opaque region shapes.

use crate::{
    boundary::graph::{BoundaryGraph, Loop},
    color::{Palette, TRANSPARENT},
    segment::RegionMap,
    Control, CoreError, Stage,
};

/// Vector coordinates in pixels, origin at top left.
pub type Point = [f64; 2];

/// Segment with implicit start point and explicit end point.
#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Line(Point),
    Cubic(Point, Point, Point),
}

impl Seg {
    /// Final anchor, independent of segment kind.
    pub fn end(&self) -> Point {
        match *self {
            Self::Line(p) | Self::Cubic(_, _, p) => p,
        }
    }
}

/// Geometry fitted once per shared boundary; start must remain at its junction.
#[derive(Clone, Debug, PartialEq)]
pub struct FittedEdge {
    pub id: u32,
    pub start: Point,
    pub segs: Vec<Seg>,
}

impl FittedEdge {
    /// Traverse the same geometry in either direction, preserving cubic controls.
    pub fn oriented(&self, reversed: bool) -> (Point, Vec<Seg>) {
        if !reversed {
            return (self.start, self.segs.clone());
        }
        let mut starts = Vec::with_capacity(self.segs.len());
        let mut p = self.start;
        for segment in &self.segs {
            starts.push(p);
            p = segment.end();
        }
        let segs = self
            .segs
            .iter()
            .zip(starts)
            .rev()
            .map(|(seg, start)| match *seg {
                Seg::Line(_) => Seg::Line(start),
                Seg::Cubic(c1, c2, _) => Seg::Cubic(c2, c1, start),
            })
            .collect();
        (p, segs)
    }
}

/// Region geometry as references to shared edges, including holes.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub region: u32,
    pub color: u16,
    pub loops: Vec<Loop>,
}

/// One closed path with redundant collinear anchors removed losslessly.
pub struct PathGeometry {
    pub start: Point,
    pub segs: Vec<Seg>,
}

/// Expand references while preserving the exact curve geometry and junction data.
/// SVG paths need not repeat a collinear anchor where two shared edges meet.
pub fn loop_geometry(edges: &[FittedEdge], path: &Loop) -> Result<PathGeometry, CoreError> {
    let mut start = None;
    let mut segs: Vec<Seg> = Vec::new();
    let mut current = [0.0; 2];
    let aligned = |a: Point, b: Point, c: Point| {
        let u = [b[0] - a[0], b[1] - a[1]];
        let v = [c[0] - b[0], c[1] - b[1]];
        (u[0] * v[1] - u[1] * v[0]).abs() <= 1e-10 && u[0] * v[0] + u[1] * v[1] > 0.0
    };
    for &(id, reversed) in path {
        let edge = edges
            .get(id as usize)
            .ok_or_else(|| CoreError::InvalidParams("loop edge reference".into()))?;
        let (point, pieces) = edge.oriented(reversed);
        if start.is_some() {
            if current != point {
                return Err(CoreError::Internal("disconnected shared edges".into()));
            }
        } else {
            start = Some(point);
            current = point;
        }
        for seg in pieces {
            let end = seg.end();
            if let (Some(Seg::Line(previous)), Seg::Line(_)) = (segs.last(), &seg) {
                let previous_start = if segs.len() == 1 {
                    start.unwrap()
                } else {
                    segs[segs.len() - 2].end()
                };
                if aligned(previous_start, *previous, end) {
                    *segs.last_mut().unwrap() = seg;
                    current = end;
                    continue;
                }
            }
            segs.push(seg);
            current = end;
        }
    }
    let mut start = start.ok_or_else(|| CoreError::Internal("empty region loop".into()))?;
    if current != start {
        return Err(CoreError::Internal("unclosed region loop".into()));
    }
    while segs.len() > 2 {
        let (Some(Seg::Line(first)), Some(Seg::Line(_))) = (segs.first(), segs.last()) else {
            break;
        };
        let first = *first;
        let previous = segs[segs.len() - 2].end();
        if !aligned(previous, start, first) {
            break;
        }
        segs.remove(0);
        *segs.last_mut().unwrap() = Seg::Line(first);
        start = first;
    }
    Ok(PathGeometry { start, segs })
}

/// Output complexity; timings will be added by the quality harness.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocStats {
    pub paths: usize,
    pub nodes: usize,
    pub colors: usize,
}

/// Deterministically ordered shared-edge vector document.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorDoc {
    pub width: f64,
    pub height: f64,
    pub palette: Palette,
    pub edges: Vec<FittedEdge>,
    pub shapes: Vec<Shape>,
    pub stats: DocStats,
}

/// Provisional F2 fitting: retain all integer crack vertices as line anchors.
pub fn polygon_edges(graph: &BoundaryGraph) -> Vec<FittedEdge> {
    graph
        .edges
        .iter()
        .map(|edge| FittedEdge {
            id: edge.id,
            start: [f64::from(edge.pts[0].x), f64::from(edge.pts[0].y)],
            segs: edge
                .pts
                .iter()
                .skip(1)
                .map(|p| Seg::Line([f64::from(p.x), f64::from(p.y)]))
                .collect(),
        })
        .collect()
}

/// Assemble opaque regions; frame/exterior/transparent topology stays shared.
#[tracing::instrument(skip_all)]
pub fn assemble(
    map: &RegionMap,
    graph: &BoundaryGraph,
    palette: &Palette,
    edges: Vec<FittedEdge>,
    ctl: &Control,
) -> Result<VectorDoc, CoreError> {
    map.validate()?;
    ctl.report(Stage::Assemble, 0.0)?;
    if graph.region_loops.len() != map.regions.len() || edges.len() != graph.edges.len() {
        return Err(CoreError::Internal("assembly topology mismatch".into()));
    }
    let mut order: Vec<_> = map
        .regions
        .iter()
        .filter(|r| r.color != TRANSPARENT)
        .collect();
    order.sort_by_key(|r| (std::cmp::Reverse(r.area), r.id));
    let mut shapes = Vec::with_capacity(order.len());
    let mut nodes = 0;
    for region in order {
        ctl.check()?;
        if region.color as usize >= palette.colors.len() {
            return Err(CoreError::InvalidParams(
                "shape color outside palette".into(),
            ));
        }
        let loops = graph.region_loops[region.id as usize].clone();
        for path in &loops {
            nodes += loop_geometry(&edges, path)?.segs.len();
        }
        shapes.push(Shape {
            region: region.id,
            color: region.color,
            loops,
        });
    }
    let stats = DocStats {
        paths: shapes.len(),
        nodes,
        colors: palette.colors.len(),
    };
    ctl.report(Stage::Assemble, 1.0)?;
    Ok(VectorDoc {
        width: f64::from(map.w),
        height: f64::from(map.h),
        palette: palette.clone(),
        edges,
        shapes,
        stats,
    })
}
