//! Explicit, bounded, metadata-only observation using production Windows traversal.
//! Raw observations stay in a new local directory; no labels or training are inferred.
use anyhow::{Result, ensure};
use dustaway_core::{
    AnalysisEvent, AnalysisPlatform,
    file_purpose::{EMBEDDED_MODEL, Model},
};
use dustaway_windows::WindowsAnalysis;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    machine: String,
    roots: Vec<Root>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Root {
    group: String,
    partition: Partition,
    path: String,
    reference: String,
    reference_basis: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Holdout,
}

fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 64
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn validate(manifest: &Manifest) -> Result<()> {
    ensure!(
        manifest.schema_version == 1 && valid_tag(&manifest.machine),
        "invalid manifest identity"
    );
    ensure!(
        !manifest.roots.is_empty() && manifest.roots.len() <= 100,
        "expected 1..100 explicit roots"
    );
    let mut groups = BTreeMap::new();
    let mut paths = BTreeSet::new();
    for root in &manifest.roots {
        ensure!(
            valid_tag(&root.group) && !root.reference_basis.trim().is_empty(),
            "missing group or reference provenance"
        );
        ensure!(
            [
                "cache",
                "logs",
                "temporary",
                "application_data",
                "personal",
                "unknown"
            ]
            .contains(&root.reference.as_str()),
            "invalid reference purpose"
        );
        if let Some(partition) = groups.insert(&root.group, root.partition) {
            ensure!(
                partition == root.partition,
                "one product group cannot cross partitions"
            );
        }
        let path = Path::new(&root.path);
        ensure!(
            path.is_absolute()
                && path.components().count() >= 3
                && !path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir)),
            "use an explicit application subtree, not an entire drive"
        );
        let normalized = root
            .path
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase();
        ensure!(paths.insert(normalized), "duplicate root");
    }
    for a in &paths {
        for b in &paths {
            ensure!(
                a == b || !b.starts_with(&format!("{a}/")),
                "overlapping roots"
            );
        }
    }
    Ok(())
}
fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string_pretty(value)?)?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 2,
        "Usage: file_purpose_observe <private-manifest.json> <new-output-directory>"
    );
    let manifest: Manifest = serde_json::from_slice(&fs::read(&args[0])?)?;
    validate(&manifest)?;
    let output = Path::new(&args[1]);
    ensure!(
        !output.exists(),
        "output exists; use a new directory to preserve observations"
    );
    fs::create_dir(output)?;
    let platform = WindowsAnalysis::new();
    let model = Model::load(EMBEDDED_MODEL)?;
    let batch = uuid::Uuid::new_v4().to_string();
    let mut observations = Vec::new();
    let mut predictions = Vec::new();
    let mut summaries = Vec::new();
    let mut seen = BTreeSet::new();
    for (scope, root) in manifest.roots.iter().enumerate() {
        let started = Instant::now();
        let mut count = 0usize;
        let mut checked = 0usize;
        let mut limited = false;
        let mut issues = BTreeMap::<String, usize>::new();
        let result = platform.validate_root(&root.path).and_then(|validated| {
            platform.scan(&validated, &AtomicBool::new(false), &mut |event| {
                if count >= 300 || checked >= 3000 || started.elapsed() > Duration::from_secs(15) { limited = true; return Ok(false); }
                match event {
                    AnalysisEvent::Checked => checked += 1,
                    AnalysisEvent::Issue {code,..} => *issues.entry(code).or_default() += 1,
                    AnalysisEvent::Entry(record) => {
                        let entry = record.entry;
                        // Parent directories with aggregate sizes are not a file-size sample.
                        if !seen.insert(entry.path.replace('\\', "/").to_lowercase()) { return Ok(true); }
                        let id = format!("{:x}", Sha256::digest(format!("{batch}|{}", entry.path)));
                        let prediction = (root.partition == Partition::Development).then(|| model.explain(&entry.path, entry.directory));
                        let reviewed = dustaway_core::recognition::recognize(&entry.path, entry.directory);
                        // Save predictions separately, keeping annotation rows blind.
                        predictions.push(serde_json::json!({"id":id,"partition":root.partition,"prediction":prediction,"withheld":root.partition == Partition::Holdout}));
                        observations.push(serde_json::json!({"id":id,"group":root.group,"partition":root.partition,"scope":scope,"path":entry.path,"directory":entry.directory,"bytes":entry.bytes,"modified_unix":entry.modified_unix,"reference":root.reference,"reference_basis":root.reference_basis,"recognized_rule":reviewed.map(|r|r.rule_id),"review":{"status":"pending","label":null,"basis":""}}));
                        count += 1;
                    }
                    _ => {}
                }
                Ok(true)
            })
        });
        summaries.push(serde_json::json!({"scope":scope,"group":root.group,"partition":root.partition,"reference":root.reference,"observations":count,"checked":checked,"limited":limited,"issues":issues,"completed":result.is_ok()&&!limited,"error":result.err().map(|_|"root_unavailable_or_scan_failed"),"elapsed_ms":started.elapsed().as_millis()}));
    }
    let inventory = serde_json::json!({"schema_version":1,"machine":manifest.machine,"batch":batch,"scope":"Explicit application subtrees, single machine, bounded traversal; directory-order sample, not random or representative.","observations":observations});
    let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&inventory)?));
    write_json(output.join("observations.private.json"), &inventory)?;
    write_json(
        output.join("predictions.private.json"),
        &serde_json::json!({"schema_version":1,"model_id":model.model_id,"inventory_sha256":fingerprint,"predictions":predictions}),
    )?;
    write_json(
        output.join("collection.json"),
        &serde_json::json!({"schema_version":1,"batch":batch,"machine":manifest.machine,"model_id":model.model_id,"inventory_sha256":fingerprint,"observations":observations.len(),"roots":summaries,"file_contents_read":false,"deletion_executed":false,"labels_inferred":false}),
    )?;
    println!(
        "Saved {} metadata observations from {} explicit roots. All labels remain pending; raw paths are private.",
        observations.len(),
        summaries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scopes_cannot_overlap_or_split_one_product_between_training_and_holdout() {
        let base = serde_json::json!({"schema_version":1,"machine":"local-01","roots":[{"group":"app","partition":"development","path":"C:/AppData/App/Cache","reference":"cache","reference_basis":"fixture only"}]});
        assert!(validate(&serde_json::from_value(base.clone()).unwrap()).is_ok());
        for path in [
            "C:/",
            "C:/AppData/App/Cache/subtree",
            "C:/AppData/App/../Cache",
        ] {
            let mut invalid = base.clone();
            let mut second = invalid["roots"][0].clone();
            second["path"] = path.into();
            invalid["roots"].as_array_mut().unwrap().push(second);
            assert!(validate(&serde_json::from_value(invalid).unwrap()).is_err());
        }
        let mut invalid = base.clone();
        let mut second = invalid["roots"][0].clone();
        second["path"] = "C:/AppData/App/logs".into();
        second["partition"] = "holdout".into();
        invalid["roots"].as_array_mut().unwrap().push(second);
        assert!(validate(&serde_json::from_value(invalid).unwrap()).is_err());
    }
}
