//! Self-contained report directory with regression highlights and difference maps.

use crate::{
    runner::{load_report, safe_name, Row, RunReport},
    Result,
};
use std::{collections::BTreeMap, fmt::Write, fs, path::Path};

fn reasons(a: &Row, b: &Row) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if b.fidelity.ssim < a.fidelity.ssim - 0.005 {
        reasons.push("SSIM");
    }
    if b.fidelity.delta_e_mean > a.fidelity.delta_e_mean + 0.3 {
        reasons.push("ΔE00");
    }
    if b.nodes as f64 > a.nodes as f64 * 1.05 {
        reasons.push("nodes");
    }
    if b.fidelity.gap_pixels > 0 {
        reasons.push("gaps");
    }
    reasons
}

pub fn check(a: &RunReport, b: &RunReport) -> Result<()> {
    let rows: BTreeMap<_, _> = b.rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut failures = Vec::new();
    for before in &a.rows {
        match rows.get(before.id.as_str()) {
            Some(after) => {
                let reasons = reasons(before, after);
                if !reasons.is_empty() {
                    failures.push(format!("{}: {}", before.id, reasons.join(", ")));
                }
            }
            None => failures.push(format!("{}: missing", before.id)),
        }
    }
    if !failures.is_empty() {
        return Err(format!("quality regressions: {}", failures.join("; ")).into());
    }
    println!(
        "quality gate passed for {} baseline images (timing gate is local/nightly only)",
        a.rows.len()
    );
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Evaluate F4-specific measurements separately from the general regression gate.
pub fn curve_gate(candidate: &RunReport, reference: &RunReport) -> Result<()> {
    let synthetic: Vec<_> = candidate
        .rows
        .iter()
        .filter(|r| r.category == "synthetic")
        .collect();
    if synthetic.len() < 20 {
        return Err("F4 requires at least 20 synthetic cases".into());
    }
    let references: BTreeMap<_, _> = reference.rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut hausdorff = 0.0;
    let mut corners = 0.0;
    let mut wins = 0;
    for row in &synthetic {
        hausdorff += row
            .hausdorff
            .ok_or("missing synthetic Hausdorff measurement")?;
        corners += row
            .corner_recall
            .ok_or("missing corner recall measurement")?;
        let baseline = references
            .get(row.id.as_str())
            .ok_or("missing reference image")?;
        if row.input_palette_colors != baseline.input_palette_colors {
            return Err("F4 comparison requires equivalent input color counts".into());
        }
        if row.nodes <= baseline.nodes && row.fidelity.ssim + 1e-12 >= baseline.fidelity.ssim {
            wins += 1;
        }
    }
    hausdorff /= synthetic.len() as f64;
    corners /= synthetic.len() as f64;
    let fraction = wins as f64 / synthetic.len() as f64;
    let gaps: usize = candidate.rows.iter().map(|r| r.fidelity.gap_pixels).sum();
    println!("F4 geometry: mean Hausdorff={hausdorff:.12}, mean corner recall={corners:.12}, nodes+SSIM >= reference on {wins}/{} ({:.2}%), gaps={gaps}",synthetic.len(),fraction*100.0);
    if hausdorff > 0.5 || corners < 0.95 || fraction < 0.8 || gaps != 0 {
        return Err("F4 geometric/compactness gate failed".into());
    }
    println!("F4 geometric/compactness gate passed; general SSIM regression gate remains separate");
    Ok(())
}

pub fn compare(root: &Path, a: &str, b: &str, enforce: bool) -> Result<()> {
    safe_name(a)?;
    safe_name(b)?;
    let before = load_report(&root.join(a).join("metrics.json"))?;
    let after = load_report(&root.join(b).join("metrics.json"))?;
    let dest = root.join(format!("compare-{a}-{b}"));
    fs::create_dir_all(dest.join("assets"))?;
    let mut html=format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>AntiVector comparison</title><style>body{{font:15px system-ui;margin:32px;background:#f4f6fa;color:#18253b}}table{{border-collapse:collapse;width:100%;background:white}}td,th{{padding:10px;text-align:right;border-bottom:1px solid #ddd}}th:first-child,td:first-child{{text-align:left}}.regression{{background:#ffe4e4}}.pair{{display:flex;gap:16px;flex-wrap:wrap}}figure{{margin:12px 0}}img{{width:128px;image-rendering:pixelated;max-width:100%}}details{{padding:12px}}body.zoom img{{width:512px}}</style><h1>{} → {}</h1><p>AA fidelity; non-AA coverage; Hausdorff samples ≤0.25px. Process RSS sampled every 2ms. Timings include machine noise. Compare equivalent color counts before interpreting node rankings.</p><button onclick=\"document.body.classList.toggle('zoom')\">Toggle 100% / 400%</button><table><tr><th>Image</th><th>SSIM A → B</th><th>ΔE00 A → B</th><th>Nodes A → B</th><th>Colors A / B</th><th>Gaps B</th><th>Flags</th></tr>",escape(a),escape(b));
    let rows: BTreeMap<_, _> = after.rows.iter().map(|r| (r.id.as_str(), r)).collect();
    for original in &before.rows {
        let Some(new) = rows.get(original.id.as_str()) else {
            return Err(format!("missing image {}", original.id).into());
        };
        safe_name(&original.id)?;
        let flags = reasons(original, new);
        let class = if flags.is_empty() { "" } else { "regression" };
        writeln!(html,"<tr class=\"{class}\"><td>{}</td><td>{:.6} → {:.6}</td><td>{:.5} → {:.5}</td><td>{} → {}</td><td>{} / {}</td><td>{}</td><td>{}</td></tr>",escape(&original.id),original.fidelity.ssim,new.fidelity.ssim,original.fidelity.delta_e_mean,new.fidelity.delta_e_mean,original.nodes,new.nodes,original.colors,new.colors,new.fidelity.gap_pixels,flags.join(", "))?;
        for (prefix, label, folder) in [
            ("a", a, "renders"),
            ("b", b, "renders"),
            ("diff", b, "diffs"),
        ] {
            fs::copy(
                root.join(label)
                    .join(folder)
                    .join(format!("{}.png", original.id)),
                dest.join("assets")
                    .join(format!("{prefix}-{}.png", original.id)),
            )?;
        }
        writeln!(html,"<tr><td colspan=\"7\"><details><summary>Previews: {} (Hausdorff B {:?}, corner recall B {:?})</summary><div class=\"pair\"><figure><figcaption>A</figcaption><img alt=\"A render\" src=\"assets/a-{}.png\"></figure><figure><figcaption>B</figcaption><img alt=\"B render\" src=\"assets/b-{}.png\"></figure><figure><figcaption>B minus input (absolute)</figcaption><img alt=\"Difference map\" src=\"assets/diff-{}.png\"></figure></div></details></td></tr>",escape(&original.id),new.hausdorff,new.corner_recall,escape(&original.id),escape(&original.id),escape(&original.id))?;
    }
    html.push_str("</table></html>\n");
    fs::write(dest.join("report.html"), html)?;
    println!("report: {}", dest.join("report.html").display());
    if enforce {
        check(&before, &after)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_escapes_untrusted_text_and_validates_paths() {
        assert_eq!(escape("<x & \"y\">"), "&lt;x &amp; &quot;y&quot;&gt;");
        assert!(safe_name("../outside").is_err());
        assert!(safe_name("run-1").is_ok());
    }
}
