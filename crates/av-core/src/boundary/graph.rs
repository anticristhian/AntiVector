//! Integer crack-edge topology. Coordinates are image pixels, y increasing down.

/// Vertex-grid coordinate. Ordering is raster (y, then x).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IPoint {
    pub y: u32,
    pub x: u32,
}

/// Real multi-region junction or the canonical start of a closed edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Junction {
    pub id: u32,
    pub point: IPoint,
}

/// Maximal boundary chain between junctions, stored exactly once.
/// Left/right are visual sides when walking start->end in y-down coordinates:
/// east has north on the left; south has east on the left.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub id: u32,
    pub start: u32,
    pub end: u32,
    pub left_region: Option<u32>,
    pub right_region: Option<u32>,
    pub pts: Vec<IPoint>,
    pub on_frame: bool,
}

/// Edge references composing a closed region boundary; bool reverses the edge.
pub type Loop = Vec<(u32, bool)>;

/// Canonical boundary topology; includes transparent regions until assembly.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryGraph {
    pub junctions: Vec<Junction>,
    pub edges: Vec<Edge>,
    pub region_loops: Vec<Vec<Loop>>,
}

/// Twice the signed area. Positive = exterior, negative = hole, in y-down space.
/// This project names positive winding CCW per 04 §7; visually it is clockwise.
pub fn signed_area2(points: &[IPoint]) -> i64 {
    points
        .windows(2)
        .map(|pair| {
            i64::from(pair[0].x) * i64::from(pair[1].y)
                - i64::from(pair[1].x) * i64::from(pair[0].y)
        })
        .sum()
}

impl BoundaryGraph {
    /// Expand loop references, including the repeated closing vertex.
    pub fn loop_points(&self, refs: &Loop) -> Vec<IPoint> {
        let mut points = Vec::new();
        for &(id, reversed) in refs {
            let edge = &self.edges[id as usize];
            let oriented: Vec<_> = if reversed {
                edge.pts.iter().rev().copied().collect()
            } else {
                edge.pts.clone()
            };
            if points.is_empty() {
                points.extend(oriented);
            } else {
                points.extend(oriented.into_iter().skip(1));
            }
        }
        points
    }
}
