//! PostScript LanguageLevel 2, DSC-conforming EPS; vector paths only.
use crate::svg::SvgOptions;
use av_core::{assemble::Seg, CoreError, VectorDoc};
use std::fmt::Write;

/// Serialize EPS, using even-odd holes and a top-left pixel to point transform.
pub fn to_string(doc: &VectorDoc, options: &SvgOptions) -> Result<String, CoreError> {
    options.validate()?;
    let paints = crate::paths::paint(doc, options.overlap, options.opaque_canvas)?;
    let w = doc.width * 0.75;
    let h = doc.height * 0.75;
    let mut out=format!("%!PS-Adobe-3.0 EPSF-3.0\n%%Creator: AntiVector\n%%BoundingBox: 0 0 {:.0} {:.0}\n%%HiResBoundingBox: 0 0 {w:.6} {h:.6}\n%%LanguageLevel: 2\n%%Pages: 1\n%%EndComments\n%%Page: 1 1\ngsave\n0 {h:.6} translate\n0.75 -0.75 scale\n",w.ceil(),h.ceil());
    for paint in paints {
        writeln!(
            out,
            "{:.8} {:.8} {:.8} setrgbcolor\nnewpath",
            f64::from(paint.rgb[0]) / 255.0,
            f64::from(paint.rgb[1]) / 255.0,
            f64::from(paint.rgb[2]) / 255.0
        )
        .unwrap();
        for path in paint.paths {
            writeln!(out, "{:.8} {:.8} moveto", path.start[0], path.start[1]).unwrap();
            for seg in path.segs {
                match seg {
                    Seg::Line(p) => writeln!(out, "{:.8} {:.8} lineto", p[0], p[1]),
                    Seg::Cubic(a, b, p) => writeln!(
                        out,
                        "{:.8} {:.8} {:.8} {:.8} {:.8} {:.8} curveto",
                        a[0], a[1], b[0], b[1], p[0], p[1]
                    ),
                }
                .unwrap();
            }
            out.push_str("closepath\n");
        }
        out.push_str("eofill\n");
    }
    out.push_str("grestore\nshowpage\n%%Trailer\n%%EOF\n");
    Ok(out)
}
