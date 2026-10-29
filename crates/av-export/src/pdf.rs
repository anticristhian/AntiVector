//! Editable vector PDF, 96 CSS pixels/inch converted to 72 points/inch.
use crate::svg::SvgOptions;
use av_core::{assemble::Seg, CoreError, VectorDoc};
use pdf_writer::{Content, Finish, Pdf, Rect, Ref};

/// Serialize one vector page. Holes use even-odd filling; no image is embedded.
pub fn to_bytes(doc: &VectorDoc, options: &SvgOptions) -> Result<Vec<u8>, CoreError> {
    options.validate()?;
    let paints = crate::paths::paint(doc, options.overlap)?;
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let pages = Ref::new(2);
    let page = Ref::new(3);
    let stream = Ref::new(4);
    pdf.catalog(catalog).pages(pages);
    pdf.pages(pages).kids([page]).count(1);
    let mut writer = pdf.page(page);
    writer
        .parent(pages)
        .media_box(Rect::new(
            0.0,
            0.0,
            doc.width as f32 * 0.75,
            doc.height as f32 * 0.75,
        ))
        .contents(stream);
    writer.resources();
    writer.finish();
    let mut content = Content::new();
    content.transform([0.75, 0.0, 0.0, -0.75, 0.0, doc.height as f32 * 0.75]);
    for paint in paints {
        content.set_fill_rgb(
            f32::from(paint.rgb[0]) / 255.0,
            f32::from(paint.rgb[1]) / 255.0,
            f32::from(paint.rgb[2]) / 255.0,
        );
        for path in paint.paths {
            content.move_to(path.start[0] as f32, path.start[1] as f32);
            for seg in path.segs {
                match seg {
                    Seg::Line(p) => {
                        content.line_to(p[0] as f32, p[1] as f32);
                    }
                    Seg::Cubic(a, b, p) => {
                        content.cubic_to(
                            a[0] as f32,
                            a[1] as f32,
                            b[0] as f32,
                            b[1] as f32,
                            p[0] as f32,
                            p[1] as f32,
                        );
                    }
                }
            }
            content.close_path();
        }
        content.fill_even_odd();
    }
    pdf.stream(stream, &content.finish());
    Ok(pdf.finish())
}
