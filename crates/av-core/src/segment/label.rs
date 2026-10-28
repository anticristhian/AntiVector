//! Two-pass union-find labeling with stable raster IDs.

use super::{IRect, Region, RegionMap};
use crate::{
    color::{oklab, LabelMap},
    Control, CoreError, RgbaImage, Stage,
};

pub(crate) fn root(parents: &mut [u32], id: u32) -> u32 {
    let mut r = id;
    while parents[r as usize] != r {
        r = parents[r as usize];
    }
    let mut p = id;
    while p != r {
        let next = parents[p as usize];
        parents[p as usize] = r;
        p = next;
    }
    r
}

/// Label all components including transparent pixels; diagonal pixels never union.
/// Optionally accumulate original linear RGB for future recoloring operations.
#[tracing::instrument(skip_all)]
pub fn label(
    labels: &LabelMap,
    original: Option<&RgbaImage>,
    ctl: &Control,
) -> Result<RegionMap, CoreError> {
    labels.validate()?;
    if let Some(image) = original {
        image.validate(crate::raster::DEFAULT_PIXEL_LIMIT)?;
        if (image.w, image.h) != (labels.w, labels.h) {
            return Err(CoreError::InvalidParams(
                "original/label dimensions differ".into(),
            ));
        }
    }
    ctl.report(Stage::Label, 0.0)?;
    let w = labels.w as usize;
    let mut parents: Vec<u32> = (0..labels.labels.len() as u32).collect();
    for i in 0..labels.labels.len() {
        if i % (w * 64) == 0 {
            ctl.check()?;
        }
        for other in [
            if i % w > 0 { Some(i - 1) } else { None },
            if i >= w { Some(i - w) } else { None },
        ]
        .into_iter()
        .flatten()
        {
            if labels.labels[i] == labels.labels[other] {
                let a = root(&mut parents, i as u32);
                let b = root(&mut parents, other as u32);
                parents[a.max(b) as usize] = a.min(b);
            }
        }
    }
    let mut ids = vec![u32::MAX; labels.labels.len()];
    let mut map = RegionMap {
        w: labels.w,
        h: labels.h,
        region_of: vec![0; labels.labels.len()],
        regions: vec![],
    };
    for i in 0..labels.labels.len() {
        if i % (w * 64) == 0 {
            ctl.check()?;
        }
        let r = root(&mut parents, i as u32) as usize;
        let x = (i % w) as u32;
        let y = (i / w) as u32;
        if ids[r] == u32::MAX {
            ids[r] = map.regions.len() as u32;
            map.regions.push(Region {
                id: ids[r],
                color: labels.labels[i],
                area: 0,
                bbox: IRect {
                    x0: x,
                    y0: y,
                    x1: x + 1,
                    y1: y + 1,
                },
                color_sum: [0.0; 3],
            });
        }
        let region = &mut map.regions[ids[r] as usize];
        region.area += 1;
        region.bbox.x0 = region.bbox.x0.min(x);
        region.bbox.y0 = region.bbox.y0.min(y);
        region.bbox.x1 = region.bbox.x1.max(x + 1);
        region.bbox.y1 = region.bbox.y1.max(y + 1);
        if let Some(image) = original {
            for ch in 0..3 {
                region.color_sum[ch] += oklab::linear_lut()[image.px[i][ch] as usize];
            }
        }
        map.region_of[i] = ids[r];
    }
    ctl.report(Stage::Label, 1.0)?;
    Ok(map)
}
