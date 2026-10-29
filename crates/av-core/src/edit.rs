//! Pure label edits. Geometry is always rebuilt from one shared boundary graph.
use crate::{
    color::{oklab, LabelMap, Palette, TRANSPARENT},
    segment::{label::label, RegionMap},
    Control, CoreError, RgbaImage,
};
use std::collections::BTreeMap;

/// IDs refer to the current document, before this operation. Persist in order.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EditOp {
    MergePalette { from: u16, into: u16 },
    RecolorPalette { color: u16, rgb: [u8; 3] },
    DeletePalette { color: u16 },
    LockColor { color: u16, locked: bool },
    MergeRegion { region: u32, into: Option<u32> },
    RemoveRegion { region: u32 },
}

/// Dense raster-ordered regions and palette after relabeling affected pixels.
pub struct EditResult {
    pub map: RegionMap,
    pub palette: Palette,
}
fn invalid(message: &str) -> CoreError {
    CoreError::InvalidParams(message.into())
}

/// Apply transactionally: failures and cancellation never modify the inputs.
pub fn apply(
    map: &RegionMap,
    palette: &Palette,
    op: &EditOp,
    original: Option<&RgbaImage>,
    ctl: &Control,
) -> Result<EditResult, CoreError> {
    map.validate()?;
    ctl.check()?;
    for (i, color) in palette.colors.iter().enumerate() {
        if color.id as usize != i {
            return Err(invalid("non-dense edit palette"));
        }
    }
    for region in &map.regions {
        if region.color != TRANSPARENT && region.color as usize >= palette.colors.len() {
            return Err(invalid("edit region palette reference"));
        }
    }
    let mut palette = palette.clone();
    let mut labels = LabelMap {
        w: map.w,
        h: map.h,
        labels: Vec::with_capacity(map.region_of.len()),
    };
    for (i, &region) in map.region_of.iter().enumerate() {
        if i % 4096 == 0 {
            ctl.check()?;
        }
        labels.labels.push(map.regions[region as usize].color);
    }
    let color_exists = |id: u16| -> Result<usize, CoreError> {
        ((id as usize) < palette.colors.len())
            .then_some(id as usize)
            .ok_or_else(|| invalid("unknown palette color"))
    };
    let mut replace_color = None;
    let mut replace_region = None;
    match *op {
        EditOp::MergePalette { from, into } => {
            let a = color_exists(from)?;
            let b = color_exists(into)?;
            if a != b {
                palette.colors[b].locked |= palette.colors[a].locked;
                palette.colors[a].locked = false;
                replace_color = Some((from, into));
            }
        }
        EditOp::RecolorPalette { color, rgb } => {
            let i = color_exists(color)?;
            palette.colors[i].rgb = rgb;
            palette.colors[i].oklab = oklab::from_srgb8(rgb);
        }
        EditOp::DeletePalette { color } => {
            let i = color_exists(color)?;
            palette.colors[i].locked = false;
            replace_color = Some((color, TRANSPARENT));
        }
        EditOp::LockColor { color, locked } => {
            let i = color_exists(color)?;
            palette.colors[i].locked = locked;
        }
        EditOp::MergeRegion { region, into } => {
            let source = map
                .regions
                .get(region as usize)
                .ok_or_else(|| invalid("unknown source region"))?;
            if source.color == TRANSPARENT {
                return Err(invalid("cannot merge a transparent source"));
            }
            let mut neighbors = BTreeMap::<u32, u32>::new();
            let w = map.w as usize;
            for (i, &id) in map.region_of.iter().enumerate() {
                if i % 4096 == 0 {
                    ctl.check()?;
                }
                if id != region {
                    continue;
                }
                for other in [
                    i.checked_sub(w),
                    (i + w < map.region_of.len()).then_some(i + w),
                    (i % w > 0).then(|| i - 1),
                    (i % w + 1 < w).then_some(i + 1),
                ]
                .into_iter()
                .flatten()
                {
                    let target = map.region_of[other];
                    if target != region {
                        *neighbors.entry(target).or_default() += 1;
                    }
                }
            }
            let target = if let Some(into) = into {
                if !neighbors.contains_key(&into) {
                    return Err(invalid("merge target must be adjacent"));
                }
                into
            } else {
                let source_lab = palette.colors[source.color as usize].oklab;
                neighbors
                    .iter()
                    .filter(|(id, _)| map.regions[**id as usize].color != TRANSPARENT)
                    .min_by(|(a, al), (b, bl)| {
                        let cost = |id: u32, length: u32| {
                            let lab = palette.colors[map.regions[id as usize].color as usize].oklab;
                            ((source_lab.l - lab.l).powi(2)
                                + (source_lab.a - lab.a).powi(2)
                                + (source_lab.b - lab.b).powi(2))
                            .sqrt()
                                / (1.0 + f64::from(length))
                        };
                        cost(**a, **al).total_cmp(&cost(**b, **bl)).then(a.cmp(b))
                    })
                    .map(|(&id, _)| id)
                    .ok_or_else(|| invalid("no opaque neighboring region; use remove"))?
            };
            replace_region = Some((region, map.regions[target as usize].color));
        }
        EditOp::RemoveRegion { region } => {
            if region as usize >= map.regions.len() {
                return Err(invalid("unknown region"));
            }
            replace_region = Some((region, TRANSPARENT));
        }
    }
    for (i, value) in labels.labels.iter_mut().enumerate() {
        if i % 4096 == 0 {
            ctl.check()?;
        }
        if let Some((from, into)) = replace_color {
            if *value == from {
                *value = into;
            }
        }
        if let Some((region, color)) = replace_region {
            if map.region_of[i] == region {
                *value = color;
            }
        }
    }
    finish(labels, palette, original, ctl)
}

/// Paint a fixed raster selection without enlarging it to a newly merged region.
pub fn apply_mask(
    map: &RegionMap,
    palette: &Palette,
    mask: &[bool],
    color: u16,
    original: Option<&RgbaImage>,
    ctl: &Control,
) -> Result<EditResult, CoreError> {
    map.validate()?;
    if mask.len() != map.region_of.len()
        || color != TRANSPARENT && color as usize >= palette.colors.len()
    {
        return Err(invalid("invalid edit mask/color"));
    }
    let mut labels = LabelMap {
        w: map.w,
        h: map.h,
        labels: Vec::with_capacity(mask.len()),
    };
    for (i, &region) in map.region_of.iter().enumerate() {
        if i % 4096 == 0 {
            ctl.check()?;
        }
        let old = map.regions[region as usize].color;
        if old != TRANSPARENT && old as usize >= palette.colors.len() {
            return Err(invalid("mask region palette reference"));
        }
        labels.labels.push(if mask[i] { color } else { old });
    }
    finish(labels, palette.clone(), original, ctl)
}
fn finish(
    mut labels: LabelMap,
    mut palette: Palette,
    original: Option<&RgbaImage>,
    ctl: &Control,
) -> Result<EditResult, CoreError> {
    // Equal RGB entries must share a label, otherwise artificial internal borders remain.
    let mut aliases = BTreeMap::new();
    let mut remap = Vec::with_capacity(palette.colors.len());
    for i in 0..palette.colors.len() {
        let id = *aliases.entry(palette.colors[i].rgb).or_insert(i as u16);
        remap.push(id);
        if id as usize != i {
            palette.colors[id as usize].locked |= palette.colors[i].locked;
            palette.colors[i].locked = false;
        }
    }
    for (i, value) in labels.labels.iter_mut().enumerate() {
        if i % 4096 == 0 {
            ctl.check()?;
        }
        if *value != TRANSPARENT {
            *value = remap[*value as usize];
        }
    }
    let palette = crate::color::compact_palette(&palette, &mut labels)?;
    let map = label(&labels, original, ctl)?;
    Ok(EditResult { map, palette })
}
