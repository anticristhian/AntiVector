//! Shared, validated paint paths; exporters never change canonical edges.
use av_core::{
    assemble::{loop_geometry, PathGeometry, Seg},
    CoreError, VectorDoc,
};
use std::collections::BTreeMap;

pub(crate) struct Paint {
    pub rgb: [u8; 3],
    pub paths: Vec<PathGeometry>,
}

pub(crate) fn validate(doc: &VectorDoc) -> Result<(), CoreError> {
    if !doc.width.is_finite()
        || !doc.height.is_finite()
        || doc.width <= 0.0
        || doc.height <= 0.0
        || doc.width > f64::from(f32::MAX)
        || doc.height > f64::from(f32::MAX)
    {
        return Err(CoreError::InvalidParams("invalid export dimensions".into()));
    }
    let point = |p: [f64; 2]| {
        p.iter()
            .all(|v| v.is_finite() && v.abs() <= f64::from(f32::MAX))
    };
    for edge in &doc.edges {
        if !point(edge.start)
            || edge.segs.iter().any(|s| match *s {
                Seg::Line(p) => !point(p),
                Seg::Cubic(a, b, p) => !point(a) || !point(b) || !point(p),
            })
        {
            return Err(CoreError::InvalidParams(
                "non-finite or out-of-range export coordinate".into(),
            ));
        }
    }
    Ok(())
}

type FrameIntervals = [Vec<(f64, f64)>; 4];

fn frame_segment(sides: &mut FrameIntervals, a: [f64; 2], b: [f64; 2], w: f64, h: f64) -> bool {
    if a == b {
        return true;
    }
    let (side, lo, hi) = if a[1] == b[1] && (a[1] == 0.0 || a[1] == h) {
        (usize::from(a[1] == h), a[0].min(b[0]), a[0].max(b[0]))
    } else if a[0] == b[0] && (a[0] == 0.0 || a[0] == w) {
        (2 + usize::from(a[0] == w), a[1].min(b[1]), a[1].max(b[1]))
    } else {
        return false;
    };
    sides[side].push((lo, hi));
    true
}

fn complete_frame(mut sides: FrameIntervals, w: f64, h: f64) -> bool {
    for (side, intervals) in sides.iter_mut().enumerate() {
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor = 0.0;
        for &(lo, hi) in intervals.iter() {
            // Demand exact, non-overlapping coverage of every frame side.
            if lo != cursor || hi <= lo {
                return false;
            }
            cursor = hi;
        }
        if cursor != if side < 2 { w } else { h } {
            return false;
        }
    }
    true
}

fn already_covers_frame(paint: &Paint, w: f64, h: f64) -> bool {
    if paint.paths.len() != 1 {
        return false;
    }
    let path = &paint.paths[0];
    let mut sides = std::array::from_fn(|_| Vec::new());
    let mut point = path.start;
    for seg in &path.segs {
        let Seg::Line(end) = seg else { return false };
        if !frame_segment(&mut sides, point, *end, w, h) {
            return false;
        }
        point = *end;
    }
    point == path.start && complete_frame(sides, w, h)
}

/// A transparent hole or exterior-connected transparent area leaves an unpaired
/// internal edge or an incomplete frame. Interior opaque interfaces cancel in
/// opposite directions. Never infer opacity from a bounding box or paint count.
fn opaque_partition(doc: &VectorDoc) -> Result<bool, CoreError> {
    let mut uses = vec![[0_usize; 2]; doc.edges.len()];
    for shape in &doc.shapes {
        for &(id, reversed) in shape.loops.iter().flatten() {
            let count = uses.get_mut(id as usize).ok_or_else(|| {
                CoreError::InvalidParams("opaque partition edge reference".into())
            })?;
            count[usize::from(reversed)] += 1;
            if count.iter().sum::<usize>() > 2 {
                return Ok(false);
            }
        }
    }
    let mut sides = std::array::from_fn(|_| Vec::new());
    for (edge, count) in doc.edges.iter().zip(uses) {
        match count {
            [0, 0] | [1, 1] => continue,
            [0, 1] | [1, 0] => (),
            _ => return Ok(false),
        }
        let mut point = edge.start;
        for seg in &edge.segs {
            let Seg::Line(end) = seg else {
                return Ok(false);
            };
            if !frame_segment(&mut sides, point, *end, doc.width, doc.height) {
                return Ok(false);
            }
            point = *end;
        }
    }
    Ok(complete_frame(sides, doc.width, doc.height))
}

pub(crate) fn paint(
    doc: &VectorDoc,
    overlap: bool,
    opaque_canvas: bool,
) -> Result<Vec<Paint>, CoreError> {
    validate(doc)?;
    let regions: BTreeMap<_, _> = doc
        .shapes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.region, i))
        .collect();
    let mut paints = doc
        .shapes
        .iter()
        .enumerate()
        .map(|(index, shape)| {
            let color = doc
                .palette
                .colors
                .get(shape.color as usize)
                .ok_or_else(|| CoreError::InvalidParams("export palette reference".into()))?;
            let paths = shape
                .loops
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    !overlap || *i == 0 || !crate::svg::omit_hole(doc, index, *i - 1, &regions)
                })
                .map(|(_, path)| loop_geometry(&doc.edges, path))
                .collect::<Result<_, _>>()?;
            Ok(Paint {
                rgb: color.rgb,
                paths,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if overlap
        && opaque_canvas
        && !paints.is_empty()
        && !already_covers_frame(&paints[0], doc.width, doc.height)
        && opaque_partition(doc)?
    {
        // Every other opaque face is painted later over this existing color.
        // Their shared interfaces still define the first region's visible shape.
        // No new color, extra paint, stroke or canonical edge is introduced.
        paints[0].paths = vec![PathGeometry {
            start: [0.0, 0.0],
            segs: vec![
                Seg::Line([doc.width, 0.0]),
                Seg::Line([doc.width, doc.height]),
                Seg::Line([0.0, doc.height]),
                Seg::Line([0.0, 0.0]),
            ],
        }];
    }
    Ok(paints)
}
