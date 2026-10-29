//! Command history with light document snapshots and replayable raster selections.
use crate::cache::{CacheCounters, SegmentationSnapshot, StageCache};
use av_core::{
    color::{oklab, Palette, TRANSPARENT},
    edit::{EditOp, EditResult},
    segment::RegionMap,
    Control, CoreError, Params, RgbaImage, VectorDoc,
};
use std::{collections::VecDeque, sync::Arc};

/// Stable selection: original-image coordinate plus a perceptual fallback for colors.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub point: Option<[f64; 2]>,
    pub rgb: Option<[u8; 3]>,
}

/// A command bound to image content rather than transient raster-order IDs.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedEdit {
    pub op: EditOp,
    pub from: Selection,
    pub into: Option<Selection>,
    pub mask: Option<RegionMask>,
}

/// Compact, fixed region footprint in the prepared raster's original coordinate scale.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionMask {
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub runs: Vec<[u32; 2]>,
}
impl RegionMask {
    fn capture(map: &RegionMap, region: u32, scale: u32, ctl: &Control) -> Result<Self, CoreError> {
        let mut runs = Vec::new();
        let mut start = None;
        for (i, &id) in map.region_of.iter().enumerate() {
            if i % 4096 == 0 {
                ctl.check()?;
            }
            if id == region {
                start.get_or_insert(i as u32);
            } else if let Some(first) = start.take() {
                runs.push([first, i as u32 - first]);
            }
        }
        if let Some(first) = start {
            runs.push([first, map.region_of.len() as u32 - first]);
        }
        Ok(Self {
            width: map.w,
            height: map.h,
            scale,
            runs,
        })
    }
    fn rasterize(
        &self,
        map: &RegionMap,
        scale: u32,
        ctl: &Control,
    ) -> Result<Vec<bool>, CoreError> {
        let size = av_core::raster::validate_dimensions(
            self.width,
            self.height,
            av_core::raster::DEFAULT_PIXEL_LIMIT,
        )? as u64;
        if !matches!(self.scale, 1 | 2)
            || u64::from(self.width) * u64::from(scale) != u64::from(map.w) * u64::from(self.scale)
            || u64::from(self.height) * u64::from(scale) != u64::from(map.h) * u64::from(self.scale)
        {
            return Err(error("invalid project region mask dimensions"));
        }
        let mut end = 0_u64;
        for &[start, len] in &self.runs {
            let start = u64::from(start);
            let len = u64::from(len);
            if len == 0 || start < end || start + len > size {
                return Err(error("invalid project region mask run"));
            }
            end = start + len;
        }
        let mut mask = Vec::with_capacity(map.region_of.len());
        for i in 0..map.region_of.len() {
            if i % 4096 == 0 {
                ctl.check()?;
            }
            let x = i as u64 % u64::from(map.w);
            let y = i as u64 / u64::from(map.w);
            let ox = ((2 * x + 1) * u64::from(self.scale) / (2 * u64::from(scale)))
                .min(u64::from(self.width) - 1);
            let oy = ((2 * y + 1) * u64::from(self.scale) / (2 * u64::from(scale)))
                .min(u64::from(self.height) - 1);
            let index = oy * u64::from(self.width) + ox;
            let run = self
                .runs
                .partition_point(|r| u64::from(r[0]) + u64::from(r[1]) <= index);
            mask.push(self.runs.get(run).is_some_and(|r| u64::from(r[0]) <= index));
        }
        Ok(mask)
    }
}

fn error(message: &str) -> CoreError {
    CoreError::InvalidParams(message.into())
}
fn region_selection(
    map: &RegionMap,
    region: u32,
    scale: u32,
    ctl: &Control,
) -> Result<Selection, CoreError> {
    let r = map
        .regions
        .get(region as usize)
        .ok_or_else(|| error("unknown edit region"))?;
    let center = [
        (r.bbox.x0 + r.bbox.x1 - 1) / 2,
        (r.bbox.y0 + r.bbox.y1 - 1) / 2,
    ];
    let mut index = (center[1] * map.w + center[0]) as usize;
    if map.region_of[index] != region {
        for (i, &id) in map.region_of.iter().enumerate() {
            if i % 4096 == 0 {
                ctl.check()?;
            }
            if id == region {
                index = i;
                break;
            }
        }
    }
    Ok(Selection {
        point: Some([
            ((index % map.w as usize) as f64 + 0.5) / f64::from(scale),
            ((index / map.w as usize) as f64 + 0.5) / f64::from(scale),
        ]),
        rgb: None,
    })
}
fn color_selection(
    map: &RegionMap,
    palette: &Palette,
    color: u16,
    scale: u32,
    ctl: &Control,
) -> Result<Selection, CoreError> {
    let rgb = palette
        .colors
        .get(color as usize)
        .ok_or_else(|| error("unknown edit color"))?
        .rgb;
    let region = map
        .regions
        .iter()
        .filter(|r| r.color == color)
        .max_by_key(|r| (r.area, std::cmp::Reverse(r.id)));
    let mut selection = if let Some(region) = region {
        region_selection(map, region.id, scale, ctl)?
    } else {
        Selection {
            point: None,
            rgb: None,
        }
    };
    selection.rgb = Some(rgb);
    Ok(selection)
}
impl Selection {
    fn region(&self, map: &RegionMap, scale: u32) -> Result<Option<u32>, CoreError> {
        let Some([x, y]) = self.point else {
            return Ok(None);
        };
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x * f64::from(scale) >= f64::from(map.w)
            || y * f64::from(scale) >= f64::from(map.h)
        {
            return Err(error("project selection outside image"));
        }
        Ok(Some(
            map.region_of[(y * f64::from(scale)) as usize * map.w as usize
                + (x * f64::from(scale)) as usize],
        ))
    }
    fn color(
        &self,
        map: &RegionMap,
        palette: &Palette,
        scale: u32,
    ) -> Result<Option<u16>, CoreError> {
        if let Some(id) = self.region(map, scale)? {
            let color = map.regions[id as usize].color;
            if color != TRANSPARENT {
                return Ok(Some(color));
            }
        }
        let rgb = self.rgb.ok_or_else(|| error("missing palette selection"))?;
        let lab = oklab::from_srgb8(rgb);
        Ok(palette
            .colors
            .iter()
            .min_by(|a, b| {
                let distance = |c: &av_core::color::PaletteColor| {
                    (lab.l - c.oklab.l).powi(2)
                        + (lab.a - c.oklab.a).powi(2)
                        + (lab.b - c.oklab.b).powi(2)
                };
                distance(a).total_cmp(&distance(b)).then(a.id.cmp(&b.id))
            })
            .map(|c| c.id))
    }
}
impl RecordedEdit {
    fn bind(
        op: EditOp,
        map: &RegionMap,
        palette: &Palette,
        scale: u32,
        ctl: &Control,
        edited: &EditResult,
    ) -> Result<Self, CoreError> {
        let color = |id| color_selection(map, palette, id, scale, ctl);
        let region = |id| region_selection(map, id, scale, ctl);
        let mask = match op {
            EditOp::MergeRegion { region, .. } | EditOp::RemoveRegion { region } => {
                Some(RegionMask::capture(map, region, scale, ctl)?)
            }
            _ => None,
        };
        let (from, into) = match op {
            EditOp::MergePalette { from, into } => (color(from)?, Some(color(into)?)),
            EditOp::RecolorPalette { color: id, .. }
            | EditOp::DeletePalette { color: id }
            | EditOp::LockColor { color: id, .. } => (color(id)?, None),
            EditOp::MergeRegion { region: id, into } => {
                let index = mask
                    .as_ref()
                    .and_then(|m| m.runs.first())
                    .ok_or_else(|| error("missing source region pixels"))?[0]
                    as usize;
                let target = edited.map.regions[edited.map.region_of[index] as usize].color;
                let selection = if target == TRANSPARENT {
                    Selection {
                        point: into.map(region).transpose()?.and_then(|s| s.point),
                        rgb: None,
                    }
                } else {
                    let rgb = edited.palette.colors[target as usize].rgb;
                    let color_id = palette
                        .colors
                        .iter()
                        .find(|c| c.rgb == rgb)
                        .ok_or_else(|| error("missing merge destination color"))?
                        .id;
                    let mut selected = if let Some(id) = into {
                        region(id)?
                    } else {
                        color(color_id)?
                    };
                    selected.rgb = Some(rgb);
                    selected
                };
                (region(id)?, Some(selection))
            }
            EditOp::RemoveRegion { region: id } => (region(id)?, None),
        };
        Ok(Self {
            op,
            from,
            into,
            mask,
        })
    }
    fn resolve(
        &self,
        map: &RegionMap,
        palette: &Palette,
        scale: u32,
    ) -> Result<Option<EditOp>, CoreError> {
        let color = || self.from.color(map, palette, scale);
        let region = || {
            self.from
                .region(map, scale)?
                .ok_or_else(|| error("missing region selection"))
        };
        Ok(Some(match self.op {
            EditOp::MergePalette { .. } => {
                let (Some(from), Some(into)) = (
                    color()?,
                    self.into
                        .as_ref()
                        .ok_or_else(|| error("missing merge color"))?
                        .color(map, palette, scale)?,
                ) else {
                    return Ok(None);
                };
                EditOp::MergePalette { from, into }
            }
            EditOp::RecolorPalette { rgb, .. } => {
                let Some(color) = color()? else {
                    return Ok(None);
                };
                EditOp::RecolorPalette { color, rgb }
            }
            EditOp::DeletePalette { .. } => {
                let Some(color) = color()? else {
                    return Ok(None);
                };
                EditOp::DeletePalette { color }
            }
            EditOp::LockColor { locked, .. } => {
                let Some(color) = color()? else {
                    return Ok(None);
                };
                EditOp::LockColor { color, locked }
            }
            EditOp::RemoveRegion { .. } => EditOp::RemoveRegion { region: region()? },
            EditOp::MergeRegion { .. } => {
                let region = region()?;
                if map.regions[region as usize].color == TRANSPARENT {
                    return Ok(None);
                };
                let into = self
                    .into
                    .as_ref()
                    .map(|p| p.region(map, scale))
                    .transpose()?
                    .flatten();
                if into == Some(region) {
                    return Ok(None);
                };
                EditOp::MergeRegion { region, into }
            }
        }))
    }
}

/// One lightweight assembled-stage snapshot, sharing its immutable geometry.
#[derive(Clone)]
pub struct Revision {
    pub params: Params,
    pub edits: Vec<RecordedEdit>,
    pub doc: Arc<VectorDoc>,
}
/// UI history availability; at least 100 committed commands can be undone.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct HistoryStatus {
    pub undo_depth: usize,
    pub redo_depth: usize,
}

/// Worker-owned cache and transactional history. Drafts only commit on publication.
pub struct Editor {
    cache: StageCache,
    current: Option<Revision>,
    past: VecDeque<Revision>,
    future: Vec<Revision>,
}
impl Editor {
    /// Start without a document; the source is shared by all revisions.
    pub fn new(source: Arc<RgbaImage>) -> Result<Self, CoreError> {
        Ok(Self {
            cache: StageCache::new(source)?,
            current: None,
            past: VecDeque::new(),
            future: Vec::new(),
        })
    }
    /// Number of actual base-stage executions, unaffected by history snapshots.
    pub fn counters(&self) -> CacheCounters {
        self.cache.counters()
    }
    /// Current snapshot for project serialization, without copying image buffers.
    pub fn current(&self) -> Option<Revision> {
        self.current.clone()
    }
    /// Number of committed revisions available in each direction.
    pub fn history(&self) -> HistoryStatus {
        HistoryStatus {
            undo_depth: self.past.len(),
            redo_depth: self.future.len(),
        }
    }
    fn segmentation(
        &mut self,
        p: &Params,
        edits: &[RecordedEdit],
        ctl: &Control,
    ) -> Result<(SegmentationSnapshot, EditResult, Arc<VectorDoc>), CoreError> {
        let doc = self.cache.run(p, ctl)?;
        let base = self
            .cache
            .segmentation()
            .ok_or_else(|| CoreError::Internal("missing segmentation checkpoint".into()))?;
        let mut result = EditResult {
            map: (*base.map).clone(),
            palette: (*base.palette).clone(),
        };
        for recorded in edits {
            ctl.check()?;
            if matches!(
                recorded.op,
                EditOp::MergeRegion { .. } | EditOp::RemoveRegion { .. }
            ) {
                let mask = recorded
                    .mask
                    .as_ref()
                    .ok_or_else(|| error("missing project region mask"))?;
                recorded
                    .from
                    .region(&result.map, base.scale)?
                    .ok_or_else(|| error("missing region anchor"))?;
                if let Some(into) = &recorded.into {
                    into.region(&result.map, base.scale)?;
                }
                let mask = mask.rasterize(&result.map, base.scale, ctl)?;
                let color = if matches!(recorded.op, EditOp::RemoveRegion { .. }) {
                    TRANSPARENT
                } else {
                    let into = recorded
                        .into
                        .as_ref()
                        .ok_or_else(|| error("missing merge destination"))?;
                    if into.rgb.is_none() {
                        TRANSPARENT
                    } else {
                        into.color(&result.map, &result.palette, base.scale)?
                            .unwrap_or(TRANSPARENT)
                    }
                };
                result = av_core::edit::apply_mask(
                    &result.map,
                    &result.palette,
                    &mask,
                    color,
                    Some(&base.image),
                    ctl,
                )?;
                continue;
            }
            if recorded.mask.is_some() {
                return Err(error("unexpected palette edit mask"));
            }
            let Some(op) = recorded.resolve(&result.map, &result.palette, base.scale)? else {
                continue;
            };
            result =
                av_core::edit::apply(&result.map, &result.palette, &op, Some(&base.image), ctl)?;
        }
        Ok((base, result, doc))
    }
    /// Prepare a new parameter revision without changing undo/redo or the current doc.
    pub fn draft_params(&mut self, p: &Params, ctl: &Control) -> Result<Revision, CoreError> {
        let edits = self
            .current
            .as_ref()
            .map(|r| r.edits.clone())
            .unwrap_or_default();
        self.draft_replay(p, edits, ctl)
    }
    /// Reconstruct a project or a bound command sequence deterministically.
    pub fn draft_replay(
        &mut self,
        p: &Params,
        edits: Vec<RecordedEdit>,
        ctl: &Control,
    ) -> Result<Revision, CoreError> {
        if edits.len() > 10_000 {
            return Err(error("too many project edits"));
        }
        if edits.is_empty() {
            let doc = self.cache.run(p, ctl)?;
            return Ok(Revision {
                params: p.clone(),
                edits,
                doc,
            });
        }
        let (base, result, unedited) = self.segmentation(p, &edits, ctl)?;
        let doc = if edits.is_empty() {
            unedited
        } else {
            Arc::new(base.assemble(&result.map, &result.palette, p, ctl)?)
        };
        ctl.check()?;
        Ok(Revision {
            params: p.clone(),
            edits,
            doc,
        })
    }
    /// Validate a new operation against the displayed segmentation and bind selections.
    pub fn draft_edit(&mut self, op: EditOp, ctl: &Control) -> Result<Revision, CoreError> {
        let current = self
            .current
            .clone()
            .ok_or_else(|| error("no completed document to edit"))?;
        let (base, result, _) = self.segmentation(&current.params, &current.edits, ctl)?;
        let edited =
            av_core::edit::apply(&result.map, &result.palette, &op, Some(&base.image), ctl)?;
        let recorded =
            RecordedEdit::bind(op, &result.map, &result.palette, base.scale, ctl, &edited)?;
        let mut edits = current.edits;
        edits.push(recorded);
        let mut params = current.params;
        params.palette_lock = edited
            .palette
            .colors
            .iter()
            .filter(|c| c.locked)
            .map(|c| c.rgb)
            .collect();
        // Lock/recolor changes are reflected in quantization immediately, so projects
        // reopened from final params + commands reproduce the same raster and SVG.
        self.draft_replay(&params, edits, ctl)
    }
    /// Publish an already computed revision; callers must enforce job-generation checks.
    pub fn commit(&mut self, revision: Revision) {
        let changed = self
            .current
            .as_ref()
            .is_none_or(|r| r.params != revision.params || r.edits != revision.edits);
        if changed {
            if let Some(previous) = self.current.take() {
                self.past.push_back(previous);
            }
            while self.past.len() > 128 {
                self.past.pop_front();
            }
            self.future.clear();
        }
        self.current = Some(revision);
    }
    /// Restore the exact immutable assembled snapshot, retaining redo commands.
    pub fn undo(&mut self) -> Result<Revision, CoreError> {
        let previous = self
            .past
            .pop_back()
            .ok_or_else(|| error("nothing to undo"))?;
        if let Some(current) = self.current.replace(previous.clone()) {
            self.future.push(current);
        }
        Ok(previous)
    }
    /// Reapply an undone immutable snapshot without numerical reconstruction.
    pub fn redo(&mut self) -> Result<Revision, CoreError> {
        let next = self.future.pop().ok_or_else(|| error("nothing to redo"))?;
        if let Some(current) = self.current.replace(next.clone()) {
            self.past.push_back(current);
        }
        Ok(next)
    }
}
