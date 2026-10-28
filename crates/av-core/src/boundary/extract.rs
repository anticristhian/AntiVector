//! Extract maximal shared chains and right-interior region loops.

use super::graph::{signed_area2, BoundaryGraph, Edge, IPoint, Junction, Loop};
use crate::{segment::RegionMap, Control, CoreError, Stage};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct Crack {
    a: IPoint,
    b: IPoint,
    left: Option<u32>,
    right: Option<u32>,
}

fn direction(a: IPoint, b: IPoint) -> u8 {
    if b.x > a.x {
        0
    } else if b.y > a.y {
        1
    } else if b.x < a.x {
        2
    } else {
        3
    }
}

/// Extract exact pixel boundaries, including frame and transparent-region topology.
#[tracing::instrument(skip_all)]
pub fn extract_boundaries(map: &RegionMap, ctl: &Control) -> Result<BoundaryGraph, CoreError> {
    map.validate()?;
    ctl.report(Stage::ExtractBoundaries, 0.0)?;
    let mut cracks = Vec::new();
    let at = |x: u32, y: u32| Some(map.region_of[(y * map.w + x) as usize]);
    for y in 0..=map.h {
        if y % 64 == 0 {
            ctl.check()?;
        }
        for x in 0..map.w {
            let north = if y > 0 { at(x, y - 1) } else { None };
            let south = if y < map.h { at(x, y) } else { None };
            if north != south {
                cracks.push(Crack {
                    a: IPoint { x, y },
                    b: IPoint { x: x + 1, y },
                    left: north,
                    right: south,
                });
            }
        }
    }
    for y in 0..map.h {
        if y % 64 == 0 {
            ctl.check()?;
        }
        for x in 0..=map.w {
            let west = if x > 0 { at(x - 1, y) } else { None };
            let east = if x < map.w { at(x, y) } else { None };
            if west != east {
                cracks.push(Crack {
                    a: IPoint { x, y },
                    b: IPoint { x, y: y + 1 },
                    left: east,
                    right: west,
                });
            }
        }
    }
    let mut incident = BTreeMap::<IPoint, Vec<usize>>::new();
    for (i, crack) in cracks.iter().enumerate() {
        incident.entry(crack.a).or_default().push(i);
        incident.entry(crack.b).or_default().push(i);
    }
    let mut junctions: BTreeSet<IPoint> = incident
        .iter()
        .filter(|(_, edges)| edges.len() != 2)
        .map(|(&v, _)| v)
        .collect();
    // Find closed components lacking junctions, adding their smallest raster vertex.
    let mut discovered = vec![false; cracks.len()];
    for &vertex in incident.keys() {
        let first = incident[&vertex][0];
        if discovered[first] {
            continue;
        }
        let mut stack = vec![vertex];
        let mut minimum = vertex;
        let mut has_junction = false;
        while let Some(v) = stack.pop() {
            minimum = minimum.min(v);
            has_junction |= junctions.contains(&v);
            for &id in &incident[&v] {
                if discovered[id] {
                    continue;
                }
                discovered[id] = true;
                stack.push(if cracks[id].a == v {
                    cracks[id].b
                } else {
                    cracks[id].a
                });
            }
        }
        if !has_junction {
            junctions.insert(minimum);
        }
    }
    let ids: BTreeMap<_, _> = junctions
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i as u32))
        .collect();
    let mut graph = BoundaryGraph {
        junctions: junctions
            .iter()
            .map(|&point| Junction {
                id: ids[&point],
                point,
            })
            .collect(),
        edges: vec![],
        region_loops: vec![vec![]; map.regions.len()],
    };
    let mut used = vec![false; cracks.len()];
    for &start in &junctions {
        ctl.check()?;
        for &first in &incident[&start] {
            if used[first] {
                continue;
            }
            let mut pts = vec![start];
            let mut current = start;
            let mut next = first;
            let crack = cracks[first];
            let (left, right) = if crack.a == start {
                (crack.left, crack.right)
            } else {
                (crack.right, crack.left)
            };
            loop {
                if used[next] {
                    return Err(CoreError::Internal("crack visited twice".into()));
                }
                used[next] = true;
                let c = cracks[next];
                current = if c.a == current { c.b } else { c.a };
                pts.push(current);
                if junctions.contains(&current) {
                    break;
                }
                next = *incident[&current]
                    .iter()
                    .find(|&&id| !used[id])
                    .ok_or_else(|| CoreError::Internal("open crack chain".into()))?;
            }
            graph.edges.push(Edge {
                id: graph.edges.len() as u32,
                start: ids[&start],
                end: ids[&current],
                left_region: left,
                right_region: right,
                pts,
                on_frame: left.is_none() || right.is_none(),
            });
        }
    }
    // A region is on the right of its directed loop. At ambiguous junctions,
    // select the sharpest right turn to keep touching contours separate.
    let mut refs = vec![Vec::<(u32, bool)>::new(); map.regions.len()];
    for edge in &graph.edges {
        if let Some(r) = edge.right_region {
            refs[r as usize].push((edge.id, false));
        }
        if let Some(r) = edge.left_region {
            refs[r as usize].push((edge.id, true));
        }
    }
    for (region, edge_refs) in refs.iter().enumerate() {
        ctl.check()?;
        let oriented = |(id, rev): (u32, bool)| {
            let pts = &graph.edges[id as usize].pts;
            if rev {
                (pts[pts.len() - 1], pts[pts.len() - 2], pts[0], pts[1])
            } else {
                (pts[0], pts[1], pts[pts.len() - 1], pts[pts.len() - 2])
            }
        };
        let mut outgoing = BTreeMap::<IPoint, Vec<usize>>::new();
        for (i, &reference) in edge_refs.iter().enumerate() {
            outgoing.entry(oriented(reference).0).or_default().push(i);
        }
        let mut visited = vec![false; edge_refs.len()];
        let mut loops: Vec<Loop> = Vec::new();
        for i in 0..edge_refs.len() {
            if visited[i] {
                continue;
            }
            let mut path = Vec::new();
            let start = oriented(edge_refs[i]).0;
            let mut next = i;
            loop {
                if visited[next] {
                    return Err(CoreError::Internal("region loop reuses an edge".into()));
                }
                visited[next] = true;
                path.push(edge_refs[next]);
                let (_, _, end, previous) = oriented(edge_refs[next]);
                if end == start {
                    break;
                }
                let incoming = direction(previous, end);
                next = outgoing
                    .get(&end)
                    .into_iter()
                    .flatten()
                    .filter(|&&j| !visited[j])
                    .min_by_key(|&&j| {
                        let (a, b, _, _) = oriented(edge_refs[j]);
                        let turn = (direction(a, b) + 4 - incoming) % 4;
                        (
                            match turn {
                                1 => 0,
                                0 => 1,
                                3 => 2,
                                _ => 3,
                            },
                            j,
                        )
                    })
                    .copied()
                    .ok_or_else(|| CoreError::Internal("region contour is not closed".into()))?;
            }
            loops.push(path);
        }
        loops.sort_by_key(|path| std::cmp::Reverse(signed_area2(&graph.loop_points(path))));
        graph.region_loops[region] = loops;
    }
    ctl.report(Stage::ExtractBoundaries, 1.0)?;
    Ok(graph)
}
