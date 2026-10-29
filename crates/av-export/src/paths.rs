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

pub(crate) fn paint(doc: &VectorDoc, overlap: bool) -> Result<Vec<Paint>, CoreError> {
    validate(doc)?;
    let regions: BTreeMap<_, _> = doc
        .shapes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.region, i))
        .collect();
    doc.shapes
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
        .collect()
}
