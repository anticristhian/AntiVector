//! Four-connected region labeling and deterministic adjacency-based merging.

pub mod label;
pub mod merge;

/// Half-open pixel bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

/// Dense region in raster discovery order, including transparent components.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub id: u32,
    pub color: u16,
    pub area: u32,
    pub bbox: IRect,
    /// Sum of original linear-light color channels (for later editing).
    pub color_sum: [f64; 3],
}

/// Dense region ownership for every pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionMap {
    pub w: u32,
    pub h: u32,
    pub region_of: Vec<u32>,
    pub regions: Vec<Region>,
}

impl RegionMap {
    /// Reject malformed dimensions, IDs and empty region tables at stage entry.
    pub fn validate(&self) -> Result<(), crate::CoreError> {
        let n =
            crate::raster::validate_dimensions(self.w, self.h, crate::raster::DEFAULT_PIXEL_LIMIT)?;
        if self.region_of.len() != n
            || self
                .region_of
                .iter()
                .any(|&id| id as usize >= self.regions.len())
            || self
                .regions
                .iter()
                .enumerate()
                .any(|(i, r)| r.id as usize != i)
        {
            return Err(crate::CoreError::InvalidParams("invalid region map".into()));
        }
        Ok(())
    }
}
