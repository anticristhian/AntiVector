//! Shared curves must retain full coverage when all regions are opaque.

use av_core::{
    color::{oklab, LabelMap, Palette, PaletteColor},
    Control, Params,
};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x41564634),failure_persistence:Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct("tests/proptest-regressions/curve_coverage.txt"))),..ProptestConfig::default()})]
    #[test]
    fn fitted_boundaries_leave_no_background((w,h,n,bytes) in (1_u32..=24,1_u32..=24,2_u16..=6).prop_flat_map(|(w,h,n)|(Just(w),Just(h),Just(n),proptest::collection::vec(any::<u8>(),(w*h) as usize)))) {
        let labels=LabelMap {w,h,labels:bytes.into_iter().map(|b|u16::from(b)%n).collect()};
        let palette=Palette {colors:[[0,0,0],[255,255,255],[255,0,0],[0,255,0],[0,0,255],[255,128,0]].into_iter().enumerate().map(|(i,rgb)|PaletteColor {id:i as u16,rgb,oklab:oklab::from_srgb8(rgb),locked:false}).collect()};
        let ctl=Control::default();let mut p=Params::default();p.experimental.corner_recovery_radius=1.5;
        let map=av_core::stages::label(&labels,None,&ctl).unwrap();
        let graph=av_core::stages::extract_boundaries(&map,&ctl).unwrap();
        let edges=av_core::stages::fit_edges(&graph,&p,false,&ctl).unwrap();
        let doc=av_core::assemble::assemble(&map,&graph,&palette,edges,&ctl).unwrap();
        let rendered=av_metrics::render::svg(&av_export::svg::to_string(&doc).unwrap(),false).unwrap();
        prop_assert!(rendered.px.iter().all(|p|p[3]==255));
    }
}
