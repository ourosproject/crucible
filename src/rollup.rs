//! Aggregation and ranking. Enforces two honesty laws by construction:
//! §5.3 (the mean is one labeled entry in a distribution, never the headline) and
//! §5.4 (a function's signal is its percentile WITHIN the analyzed set — no absolute
//! threshold). And §5.6: `pearson` lets the report show that branching and size move
//! together, so crucible never sells one signal as two.

/// The honest shape of a primitive's spread across the analyzed set. `mean` is
/// present but is NEVER the headline (spec §5.3) — it rides inside the distribution.
#[derive(Clone, Debug, PartialEq)]
pub struct Distribution {
    pub min: u32,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: u32,
    pub mean: f64,
}

/// Distribution of a primitive's values. Empty input ⇒ all zero (an honest "nothing
/// measured," not an error).
pub fn distribution(values: &[u32]) -> Distribution {
    if values.is_empty() {
        return Distribution {
            min: 0,
            median: 0.0,
            p90: 0.0,
            p99: 0.0,
            max: 0,
            mean: 0.0,
        };
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    let mean = v.iter().map(|&x| x as f64).sum::<f64>() / v.len() as f64;
    Distribution {
        min: v[0],
        median: quantile(&v, 0.50),
        p90: quantile(&v, 0.90),
        p99: quantile(&v, 0.99),
        max: v[v.len() - 1],
        mean,
    }
}

/// Linear-interpolated quantile of an ascending slice (q in 0.0..=1.0).
fn quantile(sorted: &[u32], q: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0] as f64;
    }
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let frac = pos - lo as f64;
    sorted[lo] as f64 + frac * (sorted[hi] as f64 - sorted[lo] as f64)
}

/// Fraction of `sorted_ascending` STRICTLY LESS THAN `v` — a relative rank within
/// the analyzed set (spec §5.4). Same value ranks differently in different codebases.
pub fn percentile_rank(sorted_ascending: &[u32], v: u32) -> f64 {
    if sorted_ascending.is_empty() {
        return 0.0;
    }
    let below = sorted_ascending.partition_point(|&x| x < v);
    below as f64 / sorted_ascending.len() as f64
}

/// Pearson correlation of two equal-length columns; `None` when undefined
/// (fewer than 2 points, or either column has zero variance).
pub fn pearson(xs: &[f64], ys: &[f64]) -> Option<f64> {
    let n = xs.len();
    if n < 2 || n != ys.len() {
        return None;
    }
    let mx = xs.iter().sum::<f64>() / n as f64;
    let my = ys.iter().sum::<f64>() / n as f64;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for i in 0..n {
        let dx = xs[i] - mx;
        let dy = ys[i] - my;
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx == 0.0 || syy == 0.0 {
        return None;
    }
    Some(sxy / (sxx.sqrt() * syy.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_reports_spread_not_just_a_mean() {
        // 40 ones + one 100: the mean (~3.4) hides the monster; min/median/max don't.
        let mut v = vec![1u32; 40];
        v.push(100);
        let d = distribution(&v);
        assert_eq!(d.min, 1);
        assert_eq!(d.median, 1.0);
        assert_eq!(d.max, 100, "the outlier survives — it is the story");
        assert!(
            d.mean < 4.0,
            "and the mean, on its own, would have buried it"
        );
    }

    #[test]
    fn percentile_rank_is_relative_to_the_set() {
        // A value of 10 is near the top of a simple set, mid-pack in a complex one.
        let simple = [1, 2, 3, 4, 10]; // sorted
        let complex = [1, 5, 10, 20, 50];
        assert_eq!(percentile_rank(&simple, 10), 4.0 / 5.0); // 4 of 5 below → 0.8
        assert_eq!(percentile_rank(&complex, 10), 2.0 / 5.0); // 2 of 5 below → 0.4
    }

    #[test]
    fn pearson_sees_a_perfect_line_and_declines_the_undefined() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        let ys = [2.0, 4.0, 6.0, 8.0]; // y = 2x
        assert!((pearson(&xs, &ys).unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(
            pearson(&[5.0, 5.0, 5.0], &[1.0, 2.0, 3.0]),
            None,
            "zero variance ⇒ undefined"
        );
        assert_eq!(pearson(&[1.0], &[1.0]), None, "n < 2 ⇒ undefined");
    }
}
