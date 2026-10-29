//! Validate completed human answers against the original shuffled key.
use crate::{runner, Result};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    pair: usize,
    choice: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evaluation {
    label: String,
    seed: u64,
    status: String,
    answers: Vec<Answer>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    pair: usize,
    image: String,
    #[serde(rename = "A")]
    a: String,
    #[serde(rename = "B")]
    b: String,
}

fn grade(
    template: &Evaluation,
    key: &[Pair],
    answers: &Evaluation,
    candidate: &str,
) -> Result<serde_json::Value> {
    if answers.label != template.label
        || answers.seed != template.seed
        || answers.status != "completed_by_human"
    {
        return Err(
            "human evaluation is incomplete or does not match the original label/seed".into(),
        );
    }
    if key.is_empty() || answers.answers.len() != key.len() || template.answers.len() != key.len() {
        return Err("evaluation pair count differs from the original template".into());
    }
    let mut choices = BTreeMap::new();
    for answer in &answers.answers {
        if choices
            .insert(answer.pair, answer.choice.as_deref())
            .is_some()
        {
            return Err("duplicate answer pair".into());
        }
    }
    let mut seen = BTreeSet::new();
    let mut wins = 0;
    let mut ties = 0;
    let mut losses = 0;
    for pair in key {
        if !seen.insert(pair.pair) || (pair.a == candidate) == (pair.b == candidate) {
            return Err("invalid key or candidate not present on exactly one side".into());
        }
        let choice = choices
            .get(&pair.pair)
            .ok_or("missing answer pair")?
            .ok_or("unanswered pair")?;
        match choice {
            "tie" => ties += 1,
            "A" | "B" => {
                let winner = if choice == "A" { &pair.a } else { &pair.b };
                if winner == candidate {
                    wins += 1;
                } else {
                    losses += 1;
                }
            }
            _ => return Err("unknown visual answer (expected A, B or tie)".into()),
        }
        if pair.image.is_empty() {
            return Err("empty image in original key".into());
        }
    }
    let passed = (wins + ties) * 5 >= key.len() * 4;
    Ok(
        serde_json::json!({"label":answers.label,"seed":answers.seed,"candidate":candidate,
        "status":"completed_by_human","pairs":key.len(),"wins":wins,"ties":ties,"losses":losses,
        "preferred_or_tied_fraction":(wins+ties) as f64/key.len() as f64,"required_fraction":0.8,"passed":passed}),
    )
}

pub fn run(root: &Path, label: &str, path: &Path, candidate: &str) -> Result<()> {
    runner::safe_name(label)?;
    runner::safe_name(candidate)?;
    let dir = root.join("visual").join(label);
    let output = dir.join("score.json");
    if path.exists() && output.exists() && path.canonicalize()? == output.canonicalize()? {
        return Err("answers cannot be the score output".into());
    }
    let template = serde_json::from_slice(&fs::read(dir.join("template.json"))?)?;
    let key: Vec<Pair> = serde_json::from_slice(&fs::read(dir.join("key.json"))?)?;
    let answers = serde_json::from_slice(&fs::read(path)?)?;
    let score = grade(&template, &key, &answers, candidate)?;
    fs::write(output, serde_json::to_vec_pretty(&score)?)?;
    println!("{score}");
    if score["passed"] != true {
        return Err("human preference/tie threshold of80% not met".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_gate_inputs_require_complete_unique_matching_human_answers() {
        // Pure synthetic gate test data, never saved to the real human evaluation.
        let template = Evaluation {
            label: "test".into(),
            seed: 42,
            status: "pending_human".into(),
            answers: vec![Answer {
                pair: 0,
                choice: None,
            }],
        };
        let key = vec![Pair {
            pair: 0,
            image: "test-image".into(),
            a: "candidate".into(),
            b: "reference".into(),
        }];
        let mut answers = Evaluation {
            label: "test".into(),
            seed: 42,
            status: "partial".into(),
            answers: vec![Answer {
                pair: 0,
                choice: None,
            }],
        };
        assert!(grade(&template, &key, &answers, "candidate").is_err());
        answers.status = "completed_by_human".into();
        assert!(grade(&template, &key, &answers, "candidate").is_err());
        answers.answers[0].choice = Some("tie".into());
        assert_eq!(
            grade(&template, &key, &answers, "candidate").unwrap()["passed"],
            true
        );
        answers.answers[0].choice = Some("B".into());
        assert_eq!(
            grade(&template, &key, &answers, "candidate").unwrap()["passed"],
            false
        );
        answers.answers[0].choice = Some("invalid".into());
        assert!(grade(&template, &key, &answers, "candidate").is_err());
        answers.answers[0].choice = Some("A".into());
        answers.seed = 0;
        assert!(grade(&template, &key, &answers, "candidate").is_err());
        answers.seed = 42;
        answers.answers[0].pair = 1;
        assert!(grade(&template, &key, &answers, "candidate").is_err());
    }
}
