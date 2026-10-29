#![no_main]
use av_core::{ColorCount, Control, Params, RgbaImage};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&kind, payload)) = data.split_first() else { return; };
    if kind % 3 == 2 {
        if let Ok(project) = av_session::project::open_bytes(payload) {
            if project.image.px.len() <= 65_536 {
                let session = av_session::Session::new(project.image).unwrap();
                // Invalid bound edits are recoverable; reconstruction must not panic.
                let _ = session.restore(&project.params, project.edits, &Control::default());
            }
        }
        return;
    }
    let image = if kind % 3 == 0 {
        let Ok(image) = av_session::decode::load_bytes(payload, 65_536) else { return; };
        image
    } else {
        if payload.len() < 8 { return; }
        let w = 1 + u32::from(payload[0] % 32);
        let h = 1 + u32::from(payload[1] % 32);
        let px = (0..w*h).map(|i| {
            let index = (i as usize * 4 + 2) % payload.len();
            std::array::from_fn(|c| if c == 3 && kind & 0x40 != 0 { 255 } else { payload[(index+c)%payload.len()] })
        }).collect();
        RgbaImage { w, h, px }
    };
    let presets = ["logo", "illustration", "photo", "lineart", "pixelart"];
    let mut params = Params::preset(presets[usize::from(kind / 3) % presets.len()]).unwrap();
    params.experimental.circular_loops = kind & 0x20 != 0;
    if let Some(&value) = payload.first() {
        params.colors = ColorCount::Fixed(2 + u16::from(value % 7));
        params.detail = f32::from(value) / 255.0;
        params.smoothness = 1.0 - params.detail;
        params.fit_tolerance_px = 0.1 + params.smoothness * 2.9;
        params.seed = u64::from(value);
    }
    params.min_region_area = av_core::params::default_min_region_area(params.detail, image.w, image.h);
    let doc = av_core::vectorize(&image, &params, &Control::default()).expect("valid raster/params must vectorize");
    assert_eq!((doc.width, doc.height), (f64::from(image.w), f64::from(image.h)));
    let options = av_export::svg::SvgOptions { opaque_canvas: kind & 0x80 != 0, ..Default::default() };
    let svg = av_export::svg::to_string_with_options(&doc, &options).unwrap();
    assert!(!svg.contains("NaN") && !svg.contains("inf"));
    av_export::png::to_bytes(&doc, &options).expect("valid vector must rasterize");
});
