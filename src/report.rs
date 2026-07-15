//! The human report. Deterministic ordering (spec §5, §6). It carries the honesty
//! laws in its *copy*, not just its structure: it never prints a single grade
//! (§5.5), it labels P4/P5 as unvalidated (§4), it surfaces cognitive as the
//! headline (§4), it prints the branching~size correlation (§5.6), and it states
//! that a percentile is a structural fact, not a defect risk (§5.7).

use crate::model::FnMetrics;
use crate::rollup;

const NO_DEFECT_DISCLAIMER: &str =
    "crucible measures structure, not defects — a percentile is not a bug risk.";

/// Render the full report. `top_n` bounds each worst-offenders table.
pub fn render(metrics: &[FnMetrics], top_n: usize) -> String {
    let mut out = String::new();
    out.push_str("crucible — simplicity report\n");
    out.push_str(NO_DEFECT_DISCLAIMER);
    out.push('\n');

    if metrics.is_empty() {
        out.push_str("\nno functions found.\n");
        return out;
    }

    out.push_str(&format!("\n{} functions measured.\n", metrics.len()));

    // Cognitive is the human-facing headline (derived, labeled).
    out.push_str("\n## cognitive complexity (derived: branching x nesting; the human-facing headline)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.cognitive);

    out.push_str("\n## P1 branching (cyclomatic)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.branching);
    out.push_str("\n## P2 depth (max control nesting)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.depth);
    out.push_str("\n## P3 size (statements — the baseline)\n");
    push_offenders(&mut out, metrics, top_n, |m| m.size);
    out.push_str("\n## P4 state (let bindings) [UNVALIDATED / intuition]\n");
    push_offenders(&mut out, metrics, top_n, |m| m.state);
    out.push_str("\n## P5 density (max method-chain) [UNVALIDATED / intuition]\n");
    push_offenders(&mut out, metrics, top_n, |m| m.density);

    // §5.6 — show that branching and size move together.
    let branching: Vec<f64> = metrics.iter().map(|m| m.branching as f64).collect();
    let size: Vec<f64> = metrics.iter().map(|m| m.size as f64).collect();
    out.push_str("\n## correlation\n");
    match rollup::pearson(&branching, &size) {
        Some(r) => out.push_str(&format!(
            "branching~size: r={r:.2} — much of 'branchy' is really 'big'; read them together.\n"
        )),
        None => out.push_str("branching~size: undefined (too few functions or no variance).\n"),
    }
    out
}

/// One primitive's worst offenders + its distribution line. Deterministic order:
/// by the metric descending, then by path ascending to break ties.
fn push_offenders(out: &mut String, metrics: &[FnMetrics], top_n: usize, key: impl Fn(&FnMetrics) -> u32) {
    let mut values: Vec<u32> = metrics.iter().map(&key).collect();
    values.sort_unstable();
    let dist = rollup::distribution(&values);
    out.push_str(&format!(
        "  distribution: min {} · median {:.0} · p90 {:.0} · p99 {:.0} · max {} · (mean {:.1})\n",
        dist.min, dist.median, dist.p90, dist.p99, dist.max, dist.mean
    ));

    // Total order: metric desc, then a full span tiebreak. `path` alone is NOT
    // unique — an inherent method and a trait-impl method on the same type both
    // render as `S::m` — so ties fall through to file:line:col, which is.
    let mut ranked: Vec<&FnMetrics> = metrics.iter().collect();
    ranked.sort_by(|a, b| {
        key(b)
            .cmp(&key(a))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.span.file.cmp(&b.span.file))
            .then_with(|| a.span.line.cmp(&b.span.line))
            .then_with(|| a.span.col.cmp(&b.span.col))
    });
    for m in ranked.into_iter().take(top_n) {
        let pct = rollup::percentile_rank(&values, key(m));
        out.push_str(&format!(
            "  {:>5}  p{:>3.0}  {}  {}:{}:{}\n",
            key(m),
            pct * 100.0,
            m.path,
            m.span.file,
            m.span.line,
            m.span.col + 1
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Span;

    fn fnm(path: &str, cognitive: u32, branching: u32, size: u32) -> FnMetrics {
        FnMetrics {
            path: path.into(),
            span: Span { file: "x.rs".into(), line: 1, col: 0 },
            branching,
            size,
            cognitive,
            ..Default::default()
        }
    }

    #[test]
    fn surfaces_cognitive_headline_labels_unvalidated_and_disclaims_defects() {
        let r = render(&[fnm("a", 5, 3, 10), fnm("b", 1, 1, 2)], 10);
        assert!(r.contains("cognitive complexity (derived"), "cognitive is the headline");
        assert!(r.contains("headline"));
        assert!(r.contains("[UNVALIDATED / intuition]"), "P4/P5 are tagged");
        assert!(r.contains(NO_DEFECT_DISCLAIMER), "the no-defect disclaimer ships");
        // §5.5 — never a single fused 'simplicity score'.
        assert!(!r.to_lowercase().contains("simplicity score"));
        // §5.6 — the correlation line is present.
        assert!(r.contains("branching~size: r="));
    }

    #[test]
    fn worst_offender_is_first_and_ordering_is_deterministic() {
        let r = render(&[fnm("small", 1, 1, 2), fnm("BIG", 9, 5, 40)], 10);
        let cog_section = r.split("## P1 branching").next().unwrap();
        let big_at = cog_section.find("BIG").unwrap();
        let small_at = cog_section.find("small").unwrap();
        assert!(big_at < small_at, "the worst cognitive offender is listed first");
    }

    #[test]
    fn empty_input_is_honest_not_a_crash() {
        assert!(render(&[], 10).contains("no functions found"));
    }
}
