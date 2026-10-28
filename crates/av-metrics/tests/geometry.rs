#[test]
fn anchors_corners_transforms_and_hausdorff() {
    let a="<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"32\" height=\"32\"><path d=\"M2 2H12V12H2Z\"/></svg>";
    let b="<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"32\" height=\"32\"><path transform=\"translate(1 0)\" d=\"M2 2H12V12H2Z\"/></svg>";
    let a = av_metrics::geometry::inspect(a).unwrap();
    let b = av_metrics::geometry::inspect(b).unwrap();
    assert_eq!(a.nodes, 4);
    assert_eq!(a.corners.len(), 4);
    assert_eq!(av_metrics::geometry::hausdorff(&a, &a), Some(0.0));
    assert!((av_metrics::geometry::hausdorff(&a, &b).unwrap() - 1.0).abs() < 1e-6);
    assert_eq!(
        av_metrics::geometry::corner_recall(&[[2.0, 2.0], [12.0, 2.0]], &a),
        1.0
    );
}
