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

/// Check current-corpus F5 acceptance, without claiming unrepresented categories.
pub fn quality_gate(candidate: &RunReport, reference: &RunReport) -> Result<()> {
    let refs: BTreeMap<_, _> = reference.rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut count = 0;
    let mut wins = 0;
    let mut failures = Vec::new();
    for row in &candidate.rows {
        if row.fidelity.gap_pixels != 0 {
            failures.push(format!("{}: gaps", row.id));
        }
        let threshold = match row.category.as_str() {
            "logo" | "lineart" => Some(0.95),
            "illustration" => Some(0.90),
            _ => None,
        };
        if let Some(threshold) = threshold {
            if row.fidelity.ssim < threshold || row.fidelity.delta_e_mean > 3.0 {
                failures.push(format!("{}: fidelity", row.id));
            }
        }
        if matches!(row.category.as_str(), "logo" | "illustration") {
            count += 1;
            let other = refs
                .get(row.id.as_str())
                .ok_or("missing equivalent-palette reference")?;
            if row.colors != other.colors || row.input_palette_colors != other.input_palette_colors
            {
                failures.push(format!("{}: unequal colors", row.id));
            }
            if row.nodes <= other.nodes
                && row.fidelity.ssim >= other.fidelity.ssim
                && row.fidelity.delta_e_mean <= other.fidelity.delta_e_mean
            {
                wins += 1;
            }
        }
    }
    if count < 5 || (wins as f64) / (count as f64) < 0.8 {
        failures
            .push("compactness+fidelity <80% or fewer than five logo/illustration samples".into());
    }
    println!("F5 current corpus: {wins}/{count} equivalent-color logo/illustration wins; {} images checked for gaps; human blind evaluation remains separate",candidate.rows.len());
    if !failures.is_empty() {
        return Err(failures.join("; ").into());
    }
    println!("F5 quality gate passed");
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Measure external rasterizers; this does not replace the non-AA topology gate.
pub fn render_check(reference: &Path, rendered: &Path, out: &Path, label: &str) -> Result<()> {
    safe_name(label)?;
    let load = |path: &Path| -> Result<av_core::RgbaImage> {
        let image = image::open(path)?.to_rgba8();
        Ok(av_core::RgbaImage::new(
            image.width(),
            image.height(),
            image.pixels().map(|p| p.0).collect(),
        )?)
    };
    let mut files = fs::read_dir(reference)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort();
    let mut rows = Vec::new();
    for path in files
        .into_iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "png"))
    {
        let original = load(&path)?;
        let candidate = load(&rendered.join(path.file_name().ok_or("missing filename")?))?;
        let metrics = av_metrics::compare(&original, &candidate, &candidate, 128)?;
        rows.push(serde_json::json!({"id":path.file_stem().unwrap().to_string_lossy(),"ssim":metrics.ssim,"delta_e_mean":metrics.delta_e_mean,"width":original.w,"height":original.h}));
    }
    if rows.is_empty() {
        return Err("no PNG references".into());
    }
    let minimum = rows
        .iter()
        .map(|r| r["ssim"].as_f64().unwrap())
        .fold(1.0, f64::min);
    let mean = rows
        .iter()
        .map(|r| r["ssim"].as_f64().unwrap())
        .sum::<f64>()
        / rows.len() as f64;
    fs::create_dir_all(out.join(label))?;
    fs::write(
        out.join(label).join("render-metrics.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"method":"AA export renders, white-composited; not a non-AA topology test","min_ssim":minimum,"mean_ssim":mean,"rows":rows}),
        )?,
    )?;
    println!(
        "export renders: {} images, min SSIM={minimum:.12}, mean SSIM={mean:.12}",
        rows.len()
    );
    Ok(())
}

/// Generate anonymous pairs plus a separate key and an unanswered JSON template.
pub fn visual(root: &Path, a: &str, b: &str, label: &str, seed: u64) -> Result<()> {
    for value in [a, b, label] {
        safe_name(value)?;
    }
    let first = load_report(&root.join(a).join("metrics.json"))?;
    let second = load_report(&root.join(b).join("metrics.json"))?;
    let ids: std::collections::BTreeSet<_> =
        second.rows.iter().map(|row| row.id.as_str()).collect();
    let mut rows: Vec<_> = first.rows.iter().collect();
    if rows.iter().any(|row| !ids.contains(row.id.as_str())) {
        return Err("visual pair missing image".into());
    }
    let mut state = seed;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state >> 32
    };
    for i in (1..rows.len()).rev() {
        let j = next() as usize % (i + 1);
        rows.swap(i, j);
    }
    let dir = root.join("visual").join(label);
    fs::create_dir_all(dir.join("assets"))?;
    let mut html=String::from("<!doctype html><html lang=\"pt-BR\"><meta charset=\"utf-8\"><title>Avaliação cega</title><style>body{font:16px system-ui;margin:24px}.pair{display:flex;gap:20px;flex-wrap:wrap}img{image-rendering:pixelated;max-width:100%}body.zoom img{width:calc(4 * var(--natural-width))}section{border-bottom:1px solid #ccc;padding:20px}figure{margin:0}</style><h1>Avaliação visual cega</h1><p>Escolha A, B ou empate. A ordem dos pares e os lados são embaralhados. Não consulte a chave antes de concluir.</p><button onclick=\"document.body.classList.toggle('zoom')\">100% / 400%</button><button onclick=\"save()\">Salvar respostas JSON</button>");
    let mut key = Vec::new();
    let mut answers = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        safe_name(&row.id)?;
        let swap = next() & 1 != 0;
        let engines = if swap { [b, a] } else { [a, b] };
        for (side, engine) in ["A", "B"].into_iter().zip(engines) {
            fs::copy(
                root.join(engine)
                    .join("renders")
                    .join(format!("{}.png", row.id)),
                dir.join("assets").join(format!("{i}-{side}.png")),
            )?;
        }
        writeln!(html,"<section><h2>Par {i}</h2><div class=\"pair\"><figure><figcaption>A</figcaption><img alt=\"A\" src=\"assets/{i}-A.png\"></figure><figure><figcaption>B</figcaption><img alt=\"B\" src=\"assets/{i}-B.png\"></figure></div><p><label><input type=\"radio\" name=\"p{i}\" value=\"A\">A</label> <label><input type=\"radio\" name=\"p{i}\" value=\"B\">B</label> <label><input type=\"radio\" name=\"p{i}\" value=\"tie\">Empate</label></p></section>")?;
        key.push(serde_json::json!({"pair":i,"image":row.id,"A":engines[0],"B":engines[1]}));
        answers.push(serde_json::json!({"pair":i,"choice":null}));
    }
    let template =
        serde_json::json!({"label":label,"seed":seed,"status":"pending_human","answers":answers});
    html.push_str("<script>document.querySelectorAll('img').forEach(img=>{const setup=()=>img.style.setProperty('--natural-width',img.naturalWidth+'px');if(img.complete)setup();else img.addEventListener('load',setup);});</script>");
    write!(html,"<script>const template={};function save(){{const result=structuredClone(template);result.answers.forEach(x=>x.choice=document.querySelector('input[name=p'+x.pair+']:checked')?.value??null);result.status=result.answers.every(x=>x.choice)?'completed_by_human':'partial';const link=document.createElement('a');link.href=URL.createObjectURL(new Blob([JSON.stringify(result,null,2)],{{type:'application/json'}}));link.download='{}.json';link.click();URL.revokeObjectURL(link.href);}}</script></html>",serde_json::to_string(&template)?,label)?;
    fs::write(dir.join("index.html"), html)?;
    fs::write(dir.join("key.json"), serde_json::to_vec_pretty(&key)?)?;
    fs::write(
        root.join("visual").join(format!("{label}.json")),
        serde_json::to_vec_pretty(&template)?,
    )?;
    println!(
        "blind visual evaluation: {} (pending human)",
        dir.join("index.html").display()
    );
    Ok(())
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
