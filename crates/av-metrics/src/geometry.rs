//! SVG complexity and contour metrics, with samples spaced at most 0.25 px.

use av_core::CoreError;
use resvg::{
    tiny_skia::{PathSegment, Point},
    usvg::{Group, Node},
};

type P = [f64; 2];

/// Original SVG anchors, sharp tangent changes, and dense contour samples.
pub struct Geometry {
    pub paths: usize,
    pub nodes: usize,
    pub anchors: Vec<P>,
    pub corners: Vec<P>,
    pub samples: Vec<P>,
}

#[derive(Clone, Copy)]
struct Curve {
    p: [P; 4],
}

fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn line(a: P, b: P) -> Curve {
    Curve {
        p: [
            a,
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) / 3.0),
            std::array::from_fn(|i| a[i] + 2.0 * (b[i] - a[i]) / 3.0),
            b,
        ],
    }
}

fn finish(curves: &mut Vec<Curve>, geometry: &mut Geometry) {
    if curves.is_empty() {
        return;
    }
    geometry.nodes += curves.len();
    for (i, curve) in curves.iter().enumerate() {
        geometry.anchors.push(curve.p[0]);
        let previous = curves[(i + curves.len() - 1) % curves.len()];
        let prev = previous.p[..3]
            .iter()
            .rev()
            .find(|&&p| dist(p, previous.p[3]) > 1e-9)
            .copied()
            .unwrap_or(previous.p[0]);
        let next = curve.p[1..]
            .iter()
            .find(|&&p| dist(p, curve.p[0]) > 1e-9)
            .copied()
            .unwrap_or(curve.p[3]);
        let a = [curve.p[0][0] - prev[0], curve.p[0][1] - prev[1]];
        let b = [next[0] - curve.p[0][0], next[1] - curve.p[0][1]];
        let length = a[0].hypot(a[1]) * b[0].hypot(b[1]);
        if length > 0.0 && (a[0] * b[0] + a[1] * b[1]) / length < 30_f64.to_radians().cos() {
            geometry.corners.push(curve.p[0]);
        }
        let length = curve.p.windows(2).map(|p| dist(p[0], p[1])).sum::<f64>();
        // The derivative magnitude is bounded by 3 times the control polygon
        // length; this conservative subdivision guarantees <=0.25px spacing.
        let steps = (3.0 * length / 0.25).ceil().max(1.0) as usize;
        for step in 0..steps {
            let t = step as f64 / steps as f64;
            let u = 1.0 - t;
            geometry.samples.push(std::array::from_fn(|ch| {
                u.powi(3) * curve.p[0][ch]
                    + 3.0 * u * u * t * curve.p[1][ch]
                    + 3.0 * u * t * t * curve.p[2][ch]
                    + t.powi(3) * curve.p[3][ch]
            }));
        }
    }
    curves.clear();
}

fn visit(group: &Group, geometry: &mut Geometry) {
    for node in group.children() {
        match node {
            Node::Group(child) => visit(child, geometry),
            Node::Path(path) => {
                geometry.paths += 1;
                let transform = path.abs_transform();
                let map = |mut p: Point| {
                    transform.map_point(&mut p);
                    [f64::from(p.x), f64::from(p.y)]
                };
                let mut start = [0.0; 2];
                let mut current = start;
                let mut curves = Vec::new();
                for segment in path.data().segments() {
                    match segment {
                        PathSegment::MoveTo(p) => {
                            finish(&mut curves, geometry);
                            start = map(p);
                            current = start;
                        }
                        PathSegment::LineTo(p) => {
                            let end = map(p);
                            curves.push(line(current, end));
                            current = end;
                        }
                        PathSegment::QuadTo(c, p) => {
                            let c = map(c);
                            let end = map(p);
                            curves.push(Curve {
                                p: [
                                    current,
                                    std::array::from_fn(|i| {
                                        current[i] + 2.0 / 3.0 * (c[i] - current[i])
                                    }),
                                    std::array::from_fn(|i| end[i] + 2.0 / 3.0 * (c[i] - end[i])),
                                    end,
                                ],
                            });
                            current = end;
                        }
                        PathSegment::CubicTo(a, b, p) => {
                            let end = map(p);
                            curves.push(Curve {
                                p: [current, map(a), map(b), end],
                            });
                            current = end;
                        }
                        PathSegment::Close => {
                            if dist(current, start) > 1e-9 {
                                curves.push(line(current, start));
                            }
                            finish(&mut curves, geometry);
                            current = start;
                        }
                    }
                }
                finish(&mut curves, geometry);
            }
            _ => {}
        }
    }
}

/// Parse all SVG paths and transforms, counting unique anchors per closed subpath.
pub fn inspect(svg: &str) -> Result<Geometry, CoreError> {
    let tree = resvg::usvg::Tree::from_str(svg, &Default::default())
        .map_err(|e| CoreError::InvalidParams(e.to_string()))?;
    let mut geometry = Geometry {
        paths: 0,
        nodes: 0,
        anchors: vec![],
        corners: vec![],
        samples: vec![],
    };
    visit(tree.root(), &mut geometry);
    Ok(geometry)
}

struct Kd {
    point: P,
    axis: usize,
    left: Option<Box<Kd>>,
    right: Option<Box<Kd>>,
}
impl Kd {
    fn build(points: &mut [P], depth: usize) -> Option<Box<Self>> {
        if points.is_empty() {
            return None;
        }
        let axis = depth % 2;
        let middle = points.len() / 2;
        points.select_nth_unstable_by(middle, |a, b| a[axis].total_cmp(&b[axis]));
        let (left, rest) = points.split_at_mut(middle);
        let (point, right) = rest.split_first_mut().unwrap();
        Some(Box::new(Self {
            point: *point,
            axis,
            left: Self::build(left, depth + 1),
            right: Self::build(right, depth + 1),
        }))
    }
    fn nearest(&self, p: P, best: &mut f64) {
        *best = best.min(dist(p, self.point));
        let delta = p[self.axis] - self.point[self.axis];
        let (near, far) = if delta < 0.0 {
            (&self.left, &self.right)
        } else {
            (&self.right, &self.left)
        };
        if let Some(node) = near {
            node.nearest(p, best);
        }
        if delta.abs() < *best {
            if let Some(node) = far {
                node.nearest(p, best);
            }
        }
    }
}

/// Symmetric Hausdorff distance between 0.25px contour samples.
/// Both empty returns zero; only one empty returns None rather than a fabricated value.
pub fn hausdorff(a: &Geometry, b: &Geometry) -> Option<f64> {
    if a.samples.is_empty() && b.samples.is_empty() {
        return Some(0.0);
    }
    if a.samples.is_empty() || b.samples.is_empty() {
        return None;
    }
    let directed = |source: &[P], target: &[P]| {
        let mut points = target.to_vec();
        let tree = Kd::build(&mut points, 0).unwrap();
        source
            .iter()
            .map(|&p| {
                let mut nearest = f64::INFINITY;
                tree.nearest(p, &mut nearest);
                nearest
            })
            .fold(0.0, f64::max)
    };
    Some(directed(&a.samples, &b.samples).max(directed(&b.samples, &a.samples)))
}

/// Fraction of ground-truth sharp corners matched within 1px by sharp vector anchors.
pub fn corner_recall(expected: &[P], actual: &Geometry) -> f64 {
    if expected.is_empty() {
        return 1.0;
    }
    expected
        .iter()
        .filter(|&&p| actual.corners.iter().any(|&q| dist(p, q) <= 1.0))
        .count() as f64
        / expected.len() as f64
}
