//! ASCII DXF R12 (AC1009): one POLYLINE per used shared boundary, no duplicates.
use av_core::{
    assemble::{Point, Seg},
    CoreError, VectorDoc,
};
use std::{collections::BTreeSet, fmt::Write};

fn distance(p: Point, a: Point, b: Point) -> f64 {
    let v = [b[0] - a[0], b[1] - a[1]];
    let d = v[0] * v[0] + v[1] * v[1];
    let t = if d == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * v[0] + (p[1] - a[1]) * v[1]) / d).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - t * v[0]).hypot(p[1] - a[1] - t * v[1])
}

fn flatten(p: [Point; 4], depth: u8, out: &mut Vec<Point>) -> Result<(), CoreError> {
    if distance(p[1], p[0], p[3]).max(distance(p[2], p[0], p[3])) <= 0.09999 {
        out.push(p[3]);
        return Ok(());
    }
    if depth == 32 || out.len() > 16_000_000 {
        return Err(CoreError::InvalidParams("DXF subdivision limit".into()));
    }
    let mid = |a: Point, b: Point| [a[0] * 0.5 + b[0] * 0.5, a[1] * 0.5 + b[1] * 0.5];
    let a = mid(p[0], p[1]);
    let b = mid(p[1], p[2]);
    let c = mid(p[2], p[3]);
    let d = mid(a, b);
    let e = mid(b, c);
    let f = mid(d, e);
    flatten([p[0], a, d, f], depth + 1, out)?;
    flatten([f, e, c, p[3]], depth + 1, out)
}

/// Flatten one shared edge with a conservative convex-hull error bound ≤0.1px.
pub fn flatten_edge(edge: &av_core::assemble::FittedEdge) -> Result<Vec<Point>, CoreError> {
    let mut output = vec![edge.start];
    let mut current = edge.start;
    for seg in &edge.segs {
        match *seg {
            Seg::Line(p) => output.push(p),
            Seg::Cubic(a, b, p) => flatten([current, a, b, p], 0, &mut output)?,
        }
        current = seg.end();
    }
    Ok(output)
}

/// Export used shared edges in source-pixel units with CAD's upward Y axis.
/// R12 uses POLYLINE/VERTEX/SEQEND, never LWPOLYLINE or SPLINE.
pub fn to_string(doc: &VectorDoc) -> Result<String, CoreError> {
    crate::paths::validate(doc)?;
    let used: BTreeSet<_> = doc
        .shapes
        .iter()
        .flat_map(|s| s.loops.iter().flatten().map(|&(id, _)| id))
        .collect();
    let mut out=format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$EXTMIN\n10\n0\n20\n0\n30\n0\n9\n$EXTMAX\n10\n{:.8}\n20\n{:.8}\n30\n0\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",doc.width,doc.height);
    for id in used {
        let edge = doc
            .edges
            .get(id as usize)
            .ok_or_else(|| CoreError::InvalidParams("DXF edge reference".into()))?;
        let mut points = flatten_edge(edge)?;
        let closed = points.first() == points.last();
        if closed {
            points.pop();
        }
        writeln!(
            out,
            "0\nPOLYLINE\n8\n0\n66\n1\n70\n{}\n10\n0\n20\n0\n30\n0",
            u8::from(closed)
        )
        .unwrap();
        for p in points {
            writeln!(
                out,
                "0\nVERTEX\n8\n0\n10\n{:.8}\n20\n{:.8}\n30\n0\n70\n0",
                p[0],
                doc.height - p[1]
            )
            .unwrap();
        }
        out.push_str("0\nSEQEND\n8\n0\n");
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    Ok(out)
}
