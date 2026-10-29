//! Pure deterministic raster-to-vector engine, independent of filesystem and UI.

pub mod assemble;
pub mod boundary;
pub mod color;
pub mod detect;
pub mod edit;
pub mod error;
pub mod fit;
pub mod params;
pub mod pipeline;
pub mod preprocess;
pub mod progress;
pub mod raster;
pub mod segment;

/// Pure, individually reusable pipeline stages (session cache entry points).
pub mod stages {
    pub use crate::assemble::assemble;
    pub use crate::boundary::extract::extract_boundaries;
    pub use crate::color::antialias::resolve_aa;
    pub use crate::color::quantize::quantize;
    pub use crate::fit::fit_edges;
    pub use crate::fit::fit_edges_for_regions;
    pub use crate::preprocess::preprocess;
    pub use crate::segment::label::label;
    pub use crate::segment::merge::merge_small;
    pub use crate::segment::merge::merge_small_with_aa;
}

pub use assemble::VectorDoc;
pub use detect::detect;
pub use error::CoreError;
pub use params::{ColorCount, Mode, Params};
pub use pipeline::vectorize;
pub use progress::{Control, Stage};
pub use raster::RgbaImage;
