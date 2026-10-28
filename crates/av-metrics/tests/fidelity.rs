use av_core::RgbaImage;

#[test]
fn sharma_2005_all_34_reference_pairs() {
    let mut count = 0;
    let mut worst: f64 = 0.0;
    for line in include_str!("data/sharma2005.txt")
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let v: Vec<f64> = line
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let a = [v[0], v[1], v[2]];
        let b = [v[3], v[4], v[5]];
        let actual = av_metrics::delta_e::ciede2000(a, b);
        worst = worst.max((actual - v[6]).abs());
        assert!(
            (actual - v[6]).abs() < 1e-4,
            "pair {}: {actual} != {}",
            count + 1,
            v[6]
        );
        assert!((actual - av_metrics::delta_e::ciede2000(b, a)).abs() < 1e-10);
        count += 1;
    }
    assert_eq!(count, 34);
    println!("Sharma 34 pairs: maximum absolute error {worst:.12}");
}

#[test]
fn ssim_identity_degradation_and_gaps() {
    for (w, h) in [(1, 1), (3, 4), (16, 16)] {
        let a = RgbaImage::new(
            w,
            h,
            (0..w * h)
                .map(|i| [(i * 13) as u8, (i * 17) as u8, (i * 31) as u8, 255])
                .collect(),
        )
        .unwrap();
        assert!((av_metrics::ssim::ssim(&a, &a).unwrap() - 1.0).abs() < 1e-10);
        let b = RgbaImage::new(w, h, vec![[0, 0, 0, 255]; (w * h) as usize]).unwrap();
        if w > 1 {
            assert!(av_metrics::ssim::ssim(&a, &b).unwrap() < 0.5);
        }
    }
    let a = RgbaImage::new(1, 1, vec![[255, 0, 255, 255]]).unwrap();
    let empty = RgbaImage::new(1, 1, vec![[0, 0, 0, 0]]).unwrap();
    assert_eq!(av_metrics::compare(&a, &a, &a, 128).unwrap().gap_pixels, 0);
    assert_eq!(
        av_metrics::compare(&a, &a, &empty, 128).unwrap().gap_pixels,
        1
    );
}
