//! Independent deterministic alternatives for the ADR-007 experiment.
//! Wu uses cumulative 3D histogram moments and maximum variance-reduction cuts.

use super::oklab::Oklab;

fn mean(bins: &[(Oklab, u32)], indices: &[usize]) -> Oklab {
    let mut sum = [0.0; 4];
    for &i in indices {
        let (c, n) = bins[i];
        let n = f64::from(n);
        sum[0] += c.l * n;
        sum[1] += c.a * n;
        sum[2] += c.b * n;
        sum[3] += n;
    }
    Oklab {
        l: sum[0] / sum[3],
        a: sum[1] / sum[3],
        b: sum[2] / sum[3],
    }
}

pub fn median_cut(bins: &[(Oklab, u32)], k: usize) -> Vec<Oklab> {
    let mut groups = vec![(0..bins.len()).collect::<Vec<_>>()];
    while groups.len() < k {
        let candidate = groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.len() > 1)
            .map(|(i, g)| {
                let m = mean(bins, g);
                let variance = g
                    .iter()
                    .map(|&j| bins[j].0.distance_squared(m) * f64::from(bins[j].1))
                    .sum::<f64>();
                (i, variance)
            })
            .max_by(|(ia, a), (ib, b)| a.total_cmp(b).then(ib.cmp(ia)));
        let Some((i, _)) = candidate else {
            break;
        };
        let mut group = std::mem::take(&mut groups[i]);
        let component = |c: Oklab, axis: usize| match axis {
            0 => c.l,
            1 => c.a,
            _ => c.b,
        };
        let axis = (0..3)
            .max_by(|&a, &b| {
                let span = |axis| {
                    let lo = group
                        .iter()
                        .map(|&j| component(bins[j].0, axis))
                        .fold(f64::INFINITY, f64::min);
                    let hi = group
                        .iter()
                        .map(|&j| component(bins[j].0, axis))
                        .fold(f64::NEG_INFINITY, f64::max);
                    hi - lo
                };
                span(a).total_cmp(&span(b)).then(b.cmp(&a))
            })
            .unwrap();
        group.sort_by(|&a, &b| {
            component(bins[a].0, axis)
                .total_cmp(&component(bins[b].0, axis))
                .then(a.cmp(&b))
        });
        let total: u64 = group.iter().map(|&j| u64::from(bins[j].1)).sum();
        let mut count = 0;
        let split = group
            .iter()
            .position(|&j| {
                count += u64::from(bins[j].1);
                count * 2 >= total
            })
            .unwrap_or(0)
            .saturating_add(1)
            .clamp(1, group.len() - 1);
        let other = group.split_off(split);
        groups[i] = group;
        groups.push(other);
    }
    groups.iter().map(|g| mean(bins, g)).collect()
}

type Moments = [f64; 5];
#[derive(Clone, Copy)]
struct Cube {
    lo: [usize; 3],
    hi: [usize; 3],
}
fn index(p: [usize; 3]) -> usize {
    p[0] * 33 * 33 + p[1] * 33 + p[2]
}

fn volume(table: &[Moments], cube: Cube) -> Moments {
    let mut sum = [0.0; 5];
    for mask in 0_u32..8 {
        let point = std::array::from_fn(|axis| {
            if mask & (1 << axis) != 0 {
                cube.hi[axis]
            } else {
                cube.lo[axis]
            }
        });
        let sign = if (3 - mask.count_ones()) % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        for (out, value) in sum.iter_mut().zip(table[index(point)]) {
            *out += sign * value;
        }
    }
    sum
}
fn variance(m: Moments) -> f64 {
    if m[0] > 0.0 {
        (m[4] - (m[1] * m[1] + m[2] * m[2] + m[3] * m[3]) / m[0]).max(0.0)
    } else {
        0.0
    }
}

pub fn wu(bins: &[([usize; 3], Oklab, u32)], k: usize) -> Vec<Oklab> {
    let mut table = vec![[0.0; 5]; 33 * 33 * 33];
    for &(position, c, count) in bins {
        let n = f64::from(count);
        let p = position.map(|v| v + 1);
        table[index(p)] = [
            n,
            c.l * n,
            c.a * n,
            c.b * n,
            (c.l * c.l + c.a * c.a + c.b * c.b) * n,
        ];
    }
    for axis in 0..3 {
        for r in 1..33 {
            for g in 1..33 {
                for b in 1..33 {
                    let p = [r, g, b];
                    let mut previous = p;
                    previous[axis] -= 1;
                    let sum = table[index(previous)];
                    for (value, extra) in table[index(p)].iter_mut().zip(sum) {
                        *value += extra;
                    }
                }
            }
        }
    }
    let mut cubes = vec![Cube {
        lo: [0; 3],
        hi: [32; 3],
    }];
    while cubes.len() < k {
        let mut best: Option<(f64, usize, Cube, Cube)> = None;
        for (i, &cube) in cubes.iter().enumerate() {
            let before = variance(volume(&table, cube));
            for axis in 0..3 {
                for cut in cube.lo[axis] + 1..cube.hi[axis] {
                    let mut a = cube;
                    let mut b = cube;
                    a.hi[axis] = cut;
                    b.lo[axis] = cut;
                    let va = volume(&table, a);
                    let vb = volume(&table, b);
                    if va[0] <= 0.0 || vb[0] <= 0.0 {
                        continue;
                    }
                    let gain = before - variance(va) - variance(vb);
                    if best.as_ref().is_none_or(|best| gain > best.0 + 1e-15) {
                        best = Some((gain, i, a, b));
                    }
                }
            }
        }
        let Some((_, i, a, b)) = best else {
            break;
        };
        cubes[i] = a;
        cubes.push(b);
    }
    cubes
        .into_iter()
        .filter_map(|cube| {
            let m = volume(&table, cube);
            if m[0] > 0.0 {
                Some(Oklab {
                    l: m[1] / m[0],
                    a: m[2] / m[0],
                    b: m[3] / m[0],
                })
            } else {
                None
            }
        })
        .collect()
}
