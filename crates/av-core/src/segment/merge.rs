//! Priority-ordered region adjacency merging. Fine-line preservation is added in F5.

use super::{label, RegionMap};
use crate::{
    color::{LabelMap, Palette, TRANSPARENT},
    Control, CoreError, Params, RgbaImage, Stage,
};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};

/// Merge small opaque regions by perceptual distance and shared-boundary length.
/// Transparent source regions remain transparent; ties always use the smaller ID.
#[tracing::instrument(skip_all)]
pub fn merge_small(
    map: &RegionMap,
    palette: &Palette,
    params: &Params,
    original: Option<&RgbaImage>,
    ctl: &Control,
) -> Result<RegionMap, CoreError> {
    merge_impl(map, palette, params, original, None, ctl)
}

/// Merge small and thin mixed-color regions, preserving high-contrast fine lines.
#[tracing::instrument(skip_all)]
pub fn merge_small_with_aa(
    map: &RegionMap,
    palette: &Palette,
    params: &Params,
    original: Option<&RgbaImage>,
    mixture_pixels: &[bool],
    ctl: &Control,
) -> Result<RegionMap, CoreError> {
    merge_impl(map, palette, params, original, Some(mixture_pixels), ctl)
}

fn merge_impl(
    map: &RegionMap,
    palette: &Palette,
    params: &Params,
    original: Option<&RgbaImage>,
    mixture_pixels: Option<&[bool]>,
    ctl: &Control,
) -> Result<RegionMap, CoreError> {
    map.validate()?;
    params.validate()?;
    if mixture_pixels.is_some_and(|mask| mask.len() != map.region_of.len()) {
        return Err(CoreError::InvalidParams(
            "AA mixture mask dimensions".into(),
        ));
    }
    for region in &map.regions {
        if region.color != TRANSPARENT && region.color as usize >= palette.colors.len() {
            return Err(CoreError::InvalidParams(
                "region color outside palette".into(),
            ));
        }
    }
    ctl.report(Stage::MergeSmall, 0.0)?;
    let n = map.regions.len();
    let w = map.w as usize;
    let h = map.h as usize;
    let mut adjacency = vec![BTreeMap::<u32, u32>::new(); n];
    let mut perimeter = vec![0_u32; n];
    for y in 0..h {
        if y % 64 == 0 {
            ctl.check()?;
        }
        for x in 0..w {
            let a = map.region_of[y * w + x];
            for neighbor in [
                if x + 1 < w { Some(y * w + x + 1) } else { None },
                if y + 1 < h {
                    Some((y + 1) * w + x)
                } else {
                    None
                },
            ] {
                if let Some(index) = neighbor {
                    let b = map.region_of[index];
                    if a != b {
                        *adjacency[a as usize].entry(b).or_default() += 1;
                        *adjacency[b as usize].entry(a).or_default() += 1;
                        perimeter[a as usize] += 1;
                        perimeter[b as usize] += 1;
                    }
                } else {
                    perimeter[a as usize] += 1;
                }
            }
            if x == 0 {
                perimeter[a as usize] += 1;
            }
            if y == 0 {
                perimeter[a as usize] += 1;
            }
        }
    }
    let mut parents: Vec<u32> = (0..n as u32).collect();
    let mut areas: Vec<u32> = map.regions.iter().map(|r| r.area).collect();
    let colors: Vec<u16> = map.regions.iter().map(|r| r.color).collect();
    let mut mixed = vec![false; n];
    if let Some(mask) = mixture_pixels {
        for (&id, &flag) in map.region_of.iter().zip(mask) {
            mixed[id as usize] |= flag;
        }
    }
    let forced = |id: usize, areas: &[u32], perimeter: &[u32], mixed: &[bool]| {
        params.experimental.quality_v5
            && mixed[id]
            && perimeter[id] > 0
            && 2.0 * f64::from(areas[id]) / f64::from(perimeter[id]) <= 1.5
    };
    let mut heap: BinaryHeap<_> = map
        .regions
        .iter()
        .filter(|r| {
            r.color != TRANSPARENT
                && (r.area < params.min_region_area
                    || forced(r.id as usize, &areas, &perimeter, &mixed))
        })
        .map(|r| Reverse((r.area, r.id)))
        .collect();
    while let Some(Reverse((area, id))) = heap.pop() {
        ctl.check()?;
        let a = id as usize;
        if areas[a] != area || parents[a] != id {
            continue;
        }
        if params.experimental.quality_v5
            && params.detail >= 0.5
            && area >= 4
            && perimeter[a] > 0
            && f64::from(area) / f64::from(perimeter[a]).powi(2) < 0.02
            && adjacency[a].keys().all(|&b| {
                colors[b as usize] == TRANSPARENT
                    || palette.colors[colors[a] as usize]
                        .oklab
                        .distance_squared(palette.colors[colors[b as usize] as usize].oklab)
                        >= 0.15_f64.powi(2)
            })
        {
            continue;
        }
        let target = adjacency[a]
            .iter()
            .filter_map(|(&other, &shared)| {
                let b = other as usize;
                if colors[b] == TRANSPARENT && shared * 2 < perimeter[a] {
                    return None;
                }
                let distance = if colors[b] == TRANSPARENT {
                    1.0
                } else {
                    palette.colors[colors[a] as usize]
                        .oklab
                        .distance_squared(palette.colors[colors[b] as usize].oklab)
                        .sqrt()
                };
                Some((distance / (1.0 + 0.1 * f64::from(shared)), other))
            })
            .min_by(|(ca, ia), (cb, ib)| ca.total_cmp(cb).then(ia.cmp(ib)));
        let Some((_, target)) = target else {
            continue;
        };
        let b = target as usize;
        let shared = adjacency[a][&target];
        perimeter[b] = perimeter[b] + perimeter[a] - 2 * shared;
        areas[b] += areas[a];
        areas[a] = 0;
        parents[a] = target;
        mixed[b] |= mixed[a];
        let neighbors = std::mem::take(&mut adjacency[a]);
        adjacency[b].remove(&id);
        for (neighbor, length) in neighbors {
            if neighbor == target {
                continue;
            }
            adjacency[neighbor as usize].remove(&id);
            *adjacency[neighbor as usize].entry(target).or_default() += length;
            *adjacency[b].entry(neighbor).or_default() += length;
        }
        if (areas[b] < params.min_region_area || forced(b, &areas, &perimeter, &mixed))
            && colors[b] != TRANSPARENT
        {
            heap.push(Reverse((areas[b], target)));
        }
    }
    let labels = LabelMap {
        w: map.w,
        h: map.h,
        labels: map
            .region_of
            .iter()
            .map(|&id| colors[label::root(&mut parents, id) as usize])
            .collect(),
    };
    let result = label::label(&labels, original, ctl)?;
    ctl.report(Stage::MergeSmall, 1.0)?;
    Ok(result)
}
