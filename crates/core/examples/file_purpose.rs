//! Train/evaluate a metadata model with CPU and the existing Rust dependencies only.
use anyhow::{Context, Result, bail, ensure};
use dustaway_core::file_purpose::{
    CLASS_COUNT, EMBEDDED_MODEL, FEATURE_VERSION, Model, Purpose, features,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    time::Instant,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dataset {
    schema_version: u32,
    provenance: String,
    samples: Vec<Sample>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    group: String,
    split: Split,
    path: String,
    directory: bool,
    label: Option<Purpose>,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Split {
    Train,
    Test,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchInput {
    schema_version: u32,
    items: Vec<BatchItem>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchItem {
    id: String,
    path: String,
    directory: bool,
}
fn predict_batch(model: &Model, input: BatchInput) -> Result<serde_json::Value> {
    ensure!(
        input.schema_version == 1 && input.items.len() <= 50_000,
        "invalid prediction batch"
    );
    let mut seen = BTreeSet::new();
    for item in &input.items {
        ensure!(
            !item.id.is_empty() && seen.insert(&item.id),
            "duplicate or empty prediction ID"
        );
    }
    Ok(
        serde_json::json!({"schema_version":1,"model_id":model.model_id,"predictions":input.items.iter().map(|item|serde_json::json!({"id":item.id,"prediction":model.explain(&item.path,item.directory)})).collect::<Vec<_>>()}),
    )
}

fn read_dataset(path: &str) -> Result<Dataset> {
    let dataset: Dataset = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        dataset.schema_version == 1 && !dataset.provenance.trim().is_empty(),
        "invalid dataset metadata"
    );
    let mut groups = BTreeMap::new();
    let mut paths = BTreeSet::new();
    for sample in &dataset.samples {
        ensure!(
            !sample.group.trim().is_empty()
                && (sample.label.is_none() || !features(&sample.path, sample.directory).is_empty()),
            "invalid sample"
        );
        if let Some(previous) = groups.insert(&sample.group, sample.split) {
            ensure!(
                previous == sample.split,
                "group appears in both train and test: {}",
                sample.group
            );
        }
        ensure!(
            paths.insert(
                sample
                    .path
                    .replace('\\', "/")
                    .to_lowercase()
                    .trim_end_matches('/')
                    .to_owned()
            ),
            "duplicate sample path"
        );
        ensure!(
            sample.split != Split::Train || sample.label.is_some(),
            "unknown samples belong in the test set"
        );
    }
    ensure!(
        dataset.samples.iter().any(|s| s.split == Split::Test),
        "missing held-out test set"
    );
    Ok(dataset)
}

fn train(dataset: &Dataset) -> Result<Model> {
    let mut samples: Vec<_> = dataset
        .samples
        .iter()
        .filter(|s| s.split == Split::Train)
        .collect();
    // Stable output even when a labeling tool changes row order.
    samples.sort_by(|a, b| {
        (&a.group, &a.path, a.directory, a.label).cmp(&(&b.group, &b.path, b.directory, b.label))
    });
    let mut frequency: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for sample in &samples {
        for token in features(&sample.path, sample.directory) {
            frequency.entry(token).or_default().insert(&sample.group);
        }
    }
    let vocabulary: BTreeSet<_> = frequency
        .into_iter()
        .filter(|(_, groups)| groups.len() >= 2)
        .map(|(t, _)| t)
        .collect();
    // Each software group contributes at most once to a class/token pair.
    // A flood of files from one product cannot dominate learned weights.
    let mut counts: BTreeMap<String, [BTreeSet<&str>; CLASS_COUNT]> = BTreeMap::new();
    let mut totals = [0.0; CLASS_COUNT];
    let mut samples_per_class = [0; CLASS_COUNT];
    let mut groups_per_class: [BTreeSet<&str>; CLASS_COUNT] = Default::default();
    for sample in &samples {
        let class = sample.label.context("unlabeled training sample")?.index();
        samples_per_class[class] += 1;
        groups_per_class[class].insert(&sample.group);
        for token in features(&sample.path, sample.directory) {
            if vocabulary.contains(&token) {
                counts.entry(token).or_default()[class].insert(&sample.group);
            }
        }
    }
    ensure!(
        samples_per_class.iter().all(|n| *n >= 2),
        "each class needs at least two training samples"
    );
    ensure!(
        groups_per_class.iter().all(|groups| groups.len() >= 2),
        "each class needs at least two software groups"
    );
    let vocabulary_size = counts.len() as f64;
    let smoothing = 0.5;
    for groups in counts.values() {
        for i in 0..CLASS_COUNT {
            totals[i] += groups[i].len() as f64;
        }
    }
    let tokens = counts
        .into_iter()
        .map(|(token, weights)| {
            (
                token,
                std::array::from_fn(|i| {
                    ((weights[i].len() as f64 + smoothing)
                        / (totals[i] + smoothing * vocabulary_size))
                        .ln()
                }),
            )
        })
        .collect();
    let training_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&samples)?));
    let mut model = Model {
        schema_version: 1,
        feature_version: FEATURE_VERSION,
        model_id: "pending".into(),
        provenance: dataset.provenance.clone(),
        training_sha256,
        classes: Purpose::ALL,
        samples_per_class,
        min_features: 2,
        min_margin: 2.5,
        min_directory_margin: 1.0,
        // Equal priors prevent sample collection imbalance becoming a default guess.
        log_priors: [(1.0 / CLASS_COUNT as f64).ln(); CLASS_COUNT],
        tokens,
    };
    // Bind identity to learned weights, thresholds, schema and feature version,
    // not only the raw training examples.
    let model_digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&model)?));
    model.model_id = format!("purpose-nb-v{FEATURE_VERSION}-{}", &model_digest[..12]);
    Model::load(&serde_json::to_string(&model)?)
}

fn evaluate(model: &Model, dataset: &Dataset) -> serde_json::Value {
    let mut total = 0usize;
    let mut predicted = 0usize;
    let mut correct = 0usize;
    let mut correct_abstentions = 0usize;
    let mut false_disposable = 0usize;
    let mut confusion: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut outcomes = Vec::new();
    let mut abstentions = BTreeMap::<String, usize>::new();
    let mut groups = BTreeSet::new();
    for (index, sample) in dataset
        .samples
        .iter()
        .filter(|s| s.split == Split::Test)
        .enumerate()
    {
        total += 1;
        groups.insert(&sample.group);
        let result = model.explain(&sample.path, sample.directory);
        let prediction = result.hint.as_ref().map(|h| h.purpose);
        if let Some(reason) = result.abstention {
            *abstentions
                .entry(
                    serde_json::to_value(reason)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                )
                .or_default() += 1;
        }
        let outcome = serde_json::json!({"sample_index": index, "group": sample.group, "expected": sample.label, "predicted": prediction, "abstention": result.abstention});
        outcomes.push(outcome.clone());
        predicted += usize::from(prediction.is_some());
        correct += usize::from(prediction.is_some() && prediction == sample.label);
        correct_abstentions += usize::from(prediction.is_none() && sample.label.is_none());
        false_disposable += usize::from(
            matches!(
                prediction,
                Some(Purpose::Cache | Purpose::Logs | Purpose::Temporary)
            ) && prediction != sample.label,
        );
        let key = |label: Option<Purpose>| {
            label.map_or_else(
                || "unknown".into(),
                |p| {
                    serde_json::to_value(p)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                },
            )
        };
        *confusion
            .entry(key(sample.label))
            .or_default()
            .entry(key(prediction))
            .or_default() += 1;
        if prediction != sample.label {
            failures.push(outcome);
        }
    }
    let classes: Vec<_> = Purpose::ALL.iter().map(|class| {
        let label = serde_json::to_value(class).unwrap().as_str().unwrap().to_owned();
        let support: usize = confusion.get(&label).map(|row| row.values().sum()).unwrap_or(0);
        let class_predicted: usize = confusion.values().map(|row| row.get(&label).copied().unwrap_or(0)).sum();
        let tp = confusion.get(&label).and_then(|row| row.get(&label)).copied().unwrap_or(0);
        serde_json::json!({"class": class, "support": support, "precision": ratio(tp, class_predicted), "recall": ratio(tp, support)})
    }).collect();
    serde_json::json!({
        "model_id": model.model_id, "provenance": dataset.provenance,
        "evaluation_scope": "Agreement with supplied evaluation labels; consult provenance for review method and independence. This evaluator does not verify ground truth or deletion safety.",
        "test_samples": total, "test_groups": groups.len(), "predicted": predicted,
        "abstained": total - predicted, "correct_predictions": correct, "correct_unknown_abstentions": correct_abstentions,
        "coverage": ratio(predicted, total), "precision_when_predicted": ratio(correct, predicted),
        "incorrect_cache_log_temp_predictions": false_disposable,
        "classes": classes, "confusion": confusion, "failures": failures,
        "abstention_reasons": abstentions, "outcomes": outcomes
    })
}
fn ratio(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}
fn pretty(value: &impl Serialize) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}

fn cross_validate(dataset: &Dataset) -> Result<serde_json::Value> {
    let groups: BTreeSet<_> = dataset
        .samples
        .iter()
        .filter(|s| s.split == Split::Train)
        .map(|s| s.group.clone())
        .collect();
    ensure!(
        groups.len() >= 3,
        "group evaluation needs at least three training groups"
    );
    let mut folds = Vec::new();
    for group in groups {
        let fold = Dataset {
            schema_version: dataset.schema_version,
            provenance: dataset.provenance.clone(),
            samples: dataset
                .samples
                .iter()
                .filter(|s| s.split == Split::Train)
                .cloned()
                .map(|mut sample| {
                    if sample.group == group {
                        sample.split = Split::Test;
                    }
                    sample
                })
                .collect(),
        };
        let model = train(&fold)?;
        folds.push(
            serde_json::json!({"held_out_group": group, "evaluation": evaluate(&model, &fold)}),
        );
    }
    let sum = |field: &str| {
        folds
            .iter()
            .map(|f| f["evaluation"][field].as_u64().unwrap())
            .sum::<u64>()
    };
    let total = sum("test_samples");
    let predicted = sum("predicted");
    let correct = sum("correct_predictions");
    Ok(
        serde_json::json!({"scope": "Leave-one-software-group-out on synthetic training groups; the original test split is excluded. Development metric, not real-machine accuracy.", "fold_count": folds.len(), "samples": total, "predicted": predicted, "correct_predictions": correct, "abstained": total-predicted, "precision_when_predicted": ratio(correct as usize,predicted as usize), "coverage": ratio(predicted as usize,total as usize), "incorrect_cache_log_temp_predictions": sum("incorrect_cache_log_temp_predictions"), "folds": folds}),
    )
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("train" | "check") if args.len() == 4 => {
            let dataset = read_dataset(&args[1])?;
            let started = Instant::now();
            let model = train(&dataset)?;
            let training_ms = started.elapsed().as_secs_f64() * 1000.0;
            let model_json = pretty(&model)?;
            let report_json = pretty(&evaluate(&model, &dataset))?;
            if args[0] == "check" {
                ensure!(
                    fs::read_to_string(&args[2])?.replace("\r\n", "\n") == model_json,
                    "model is stale; rerun train"
                );
                ensure!(
                    fs::read_to_string(&args[3])?.replace("\r\n", "\n") == report_json,
                    "evaluation is stale; rerun train"
                );
            } else {
                ensure!(
                    args[1] != args[2] && args[1] != args[3] && args[2] != args[3],
                    "input/output paths must differ"
                );
                fs::write(&args[2], &model_json)?;
                fs::write(&args[3], &report_json)?;
            }
            println!(
                "{}: {} bytes, {} features, {} training samples, {:.2} ms CPU training",
                args[0],
                model_json.len(),
                model.tokens.len(),
                model.samples_per_class.iter().sum::<usize>(),
                training_ms
            );
            println!("{report_json}");
        }
        Some("predict" | "explain")
            if args.len() == 2 || (args.len() == 3 && args[2] == "--directory") =>
        {
            let model = Model::load(EMBEDDED_MODEL)?;
            if args[0] == "explain" {
                println!("{}", pretty(&model.explain(&args[1], args.len() == 3))?);
            } else {
                println!("{}", pretty(&model.predict(&args[1], args.len() == 3))?);
            }
        }
        Some("evaluate") if args.len() == 2 || args.len() == 3 => {
            let dataset = read_dataset(&args[1])?;
            let model_json = if args.len() == 3 {
                fs::read_to_string(&args[2])?
            } else {
                EMBEDDED_MODEL.into()
            };
            let model = Model::load(&model_json)?;
            println!("{}", pretty(&evaluate(&model, &dataset))?);
        }
        Some("predict-batch") if args.len() == 2 => {
            ensure!(
                fs::metadata(&args[1])?.len() <= 32 * 1024 * 1024,
                "prediction input exceeds 32 MiB"
            );
            let input: BatchInput = serde_json::from_slice(&fs::read(&args[1])?)?;
            println!(
                "{}",
                pretty(&predict_batch(&Model::load(EMBEDDED_MODEL)?, input)?)?
            );
        }
        Some("cross-validate") if args.len() == 2 => {
            println!("{}", pretty(&cross_validate(&read_dataset(&args[1])?)?)?);
        }
        Some("bench") if args.len() == 2 => {
            let dataset = read_dataset(&args[1])?;
            let model = Model::load(EMBEDDED_MODEL)?;
            let started = Instant::now();
            for sample in dataset.samples.iter().cycle().take(100_000) {
                std::hint::black_box(
                    model.predict(std::hint::black_box(&sample.path), sample.directory),
                );
            }
            println!(
                "100000 predictions: {:.2} ms (CPU, metadata only; excludes disk scanning)",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        _ => bail!(
            "Usage: file_purpose train|check <dataset.json> <model.json> <evaluation.json>\n       file_purpose predict|explain <path> [--directory]\n       file_purpose predict-batch <unlabeled-paths.json>\n       file_purpose evaluate <dataset.json> [model.json]\n       file_purpose cross-validate <dataset.json>\n       file_purpose bench <dataset.json>"
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seed() -> Dataset {
        serde_json::from_str(include_str!("../../../models/file-purpose/seed.json")).unwrap()
    }
    #[test]
    fn test_rows_cannot_influence_weights_or_training_fingerprint() {
        let mut dataset = seed();
        let first = pretty(&train(&dataset).unwrap()).unwrap();
        assert_eq!(first, pretty(&train(&dataset).unwrap()).unwrap());
        for sample in dataset
            .samples
            .iter_mut()
            .filter(|s| s.split == Split::Test)
        {
            sample.path = "D:/completely/different/opaque.ext".into();
            sample.label = Some(Purpose::Logs);
        }
        assert_eq!(first, pretty(&train(&dataset).unwrap()).unwrap());
    }
    #[test]
    fn training_rejects_group_leakage_and_case_insensitive_duplicate_paths() {
        for duplicate in [false, true] {
            let mut dataset = seed();
            let first_group = dataset.samples[0].group.clone();
            let first_path = dataset.samples[0].path.to_uppercase().replace('/', "\\");
            let first_directory = dataset.samples[0].directory;
            let test = dataset
                .samples
                .iter_mut()
                .find(|s| s.split == Split::Test)
                .unwrap();
            if duplicate {
                test.path = first_path;
                test.directory = first_directory;
            } else {
                test.group = first_group;
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("dataset.json");
            fs::write(&path, pretty(&dataset).unwrap()).unwrap();
            assert!(read_dataset(path.to_str().unwrap()).is_err());
        }
    }

    #[test]
    fn row_order_does_not_change_model_and_one_product_cannot_flood_weights() {
        let mut dataset = seed();
        let first = train(&dataset).unwrap();
        dataset.samples.reverse();
        assert_eq!(
            pretty(&first).unwrap(),
            pretty(&train(&dataset).unwrap()).unwrap()
        );
        let duplicate = dataset
            .samples
            .iter()
            .find(|s| s.split == Split::Train && !s.directory)
            .unwrap()
            .clone();
        for n in 0..1000 {
            // An invented token in a single product must never enter the vocabulary.
            let mut sample = duplicate.clone();
            sample.path = format!("D:/singleproduct/singleproducttoken/item{n}.uniqueextension");
            dataset.samples.push(sample);
        }
        let second = train(&dataset).unwrap();
        assert!(
            !second
                .tokens
                .keys()
                .any(|token| token.contains("singleproduct"))
        );
        // Repeated same-group sample features have identical effect to one sample.
        dataset.samples.truncate(dataset.samples.len() - 999);
        assert_eq!(second.tokens, train(&dataset).unwrap().tokens);
    }

    #[test]
    fn group_validation_never_uses_original_test_rows() {
        let mut dataset = seed();
        let first = cross_validate(&dataset).unwrap();
        dataset.samples.retain(|s| s.split == Split::Train);
        assert_eq!(first, cross_validate(&dataset).unwrap());
    }

    #[test]
    fn one_group_per_class_cannot_train_an_apparently_valid_model() {
        let mut dataset = seed();
        for sample in dataset
            .samples
            .iter_mut()
            .filter(|s| s.split == Split::Train)
        {
            sample.group = "single-machine".into();
        }
        assert!(train(&dataset).is_err());
    }

    #[test]
    fn unlabeled_batch_is_inference_only_and_rejects_duplicate_ids() {
        let model = Model::load(EMBEDDED_MODEL).unwrap();
        let input = BatchInput {
            schema_version: 1,
            items: vec![BatchItem {
                id: "one".into(),
                path: "D:/Unlisted/logs/20261029/window1/exthost/service.log".into(),
                directory: false,
            }],
        };
        let output = predict_batch(&model, input).unwrap();
        assert_eq!(
            output["predictions"][0]["prediction"]["hint"]["purpose"],
            "logs"
        );
        assert!(output.get("accuracy").is_none());
        let duplicate = BatchInput {
            schema_version: 1,
            items: (0..2)
                .map(|_| BatchItem {
                    id: "one".into(),
                    path: "D:/unknown".into(),
                    directory: false,
                })
                .collect(),
        };
        assert!(predict_batch(&model, duplicate).is_err());
    }
}
