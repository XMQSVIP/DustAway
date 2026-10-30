//! Small CPU-only metadata classifier. Its output is descriptive, never deletion authority.
use crate::{AnalysisAdvice, AnalysisEntry, AnalysisPurpose};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};

pub const CLASS_COUNT: usize = 5;
pub const FEATURE_VERSION: u32 = 4;
pub const EMBEDDED_MODEL: &str = include_str!("../../../models/file-purpose/model.json");
static MODEL: LazyLock<Option<Model>> = LazyLock::new(|| Model::load(EMBEDDED_MODEL).ok());

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Cache,
    Logs,
    Temporary,
    Personal,
    ApplicationData,
}
impl Purpose {
    pub const ALL: [Self; CLASS_COUNT] = [
        Self::Cache,
        Self::Logs,
        Self::Temporary,
        Self::Personal,
        Self::ApplicationData,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hint {
    pub purpose: Purpose,
    pub model_id: String,
    /// Difference between the two highest log scores, NOT a calibrated probability.
    pub score_margin: f64,
    /// At most three learned features; no complete paths or file contents.
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Abstention {
    InvalidPath,
    SensitiveLocation,
    InsufficientFeatures,
    CloseScores,
    MissingContext,
    ConflictingContext,
    ProtectedExtension,
    ProtectedDatabaseFile,
}

#[derive(Debug, Serialize)]
pub struct Prediction {
    pub hint: Option<Hint>,
    pub abstention: Option<Abstention>,
}
impl Prediction {
    fn abstain(reason: Abstention) -> Self {
        Self {
            hint: None,
            abstention: Some(reason),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub schema_version: u32,
    pub feature_version: u32,
    pub model_id: String,
    pub provenance: String,
    pub training_sha256: String,
    pub classes: [Purpose; CLASS_COUNT],
    pub samples_per_class: [usize; CLASS_COUNT],
    pub min_features: usize,
    pub min_margin: f64,
    pub min_directory_margin: f64,
    pub log_priors: [f64; CLASS_COUNT],
    pub tokens: BTreeMap<String, [f64; CLASS_COUNT]>,
}

impl Model {
    pub fn load(json: &str) -> Result<Self> {
        ensure!(json.len() <= 512 * 1024, "model exceeds 512 KiB");
        let model: Self = serde_json::from_str(json)?;
        ensure!(
            model.schema_version == 1 && model.feature_version == FEATURE_VERSION,
            "unsupported model version"
        );
        ensure!(model.classes == Purpose::ALL, "invalid class ordering");
        ensure!(
            !model.model_id.is_empty() && !model.provenance.is_empty(),
            "missing model identity"
        );
        ensure!(
            model.training_sha256.len() == 64
                && model.training_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid training fingerprint"
        );
        ensure!(
            model.samples_per_class.iter().all(|n| *n > 0),
            "empty training class"
        );
        ensure!(
            (2..=32).contains(&model.min_features)
                && model.min_margin.is_finite()
                && model.min_margin >= 1.0,
            "invalid abstention settings"
        );
        ensure!(
            model.min_directory_margin.is_finite()
                && model.min_directory_margin >= 1.0
                && model.min_directory_margin <= model.min_margin,
            "invalid directory margin"
        );
        ensure!(
            !model.tokens.is_empty() && model.tokens.len() <= 8192,
            "invalid vocabulary size"
        );
        ensure!(
            model
                .log_priors
                .iter()
                .chain(model.tokens.values().flatten())
                .all(|n| n.is_finite() && (-1_000_000.0..=0.0).contains(n)),
            "invalid log weights"
        );
        ensure!(
            model.tokens.keys().all(|t| t.len() <= 160
                && ["dir:", "name:", "ext:", "kind:"]
                    .iter()
                    .any(|p| t.starts_with(p))),
            "invalid feature name"
        );
        Ok(model)
    }

    pub fn predict(&self, path: &str, directory: bool) -> Option<Hint> {
        self.explain(path, directory).hint
    }

    pub fn explain(&self, path: &str, directory: bool) -> Prediction {
        let Some(input) = PathFeatures::new(path, directory) else {
            return Prediction::abstain(Abstention::InvalidPath);
        };
        if input.sensitive {
            return Prediction::abstain(Abstention::SensitiveLocation);
        }
        let features = &input.tokens;
        let matched: Vec<_> = features
            .iter()
            .filter_map(|f| {
                self.tokens
                    .get(f)
                    .filter(|w| spread(w) >= 0.5)
                    .map(|w| (f, w))
            })
            .collect();
        let informative = matched
            .iter()
            .filter(|(f, w)| !f.starts_with("kind:") && spread(w) >= 0.5)
            .count();
        if informative < if directory { 1 } else { self.min_features } {
            return Prediction::abstain(Abstention::InsufficientFeatures);
        }
        let mut scores = self.log_priors;
        for (_, weights) in &matched {
            for (score, weight) in scores.iter_mut().zip(weights.iter()) {
                *score += weight;
            }
        }
        let mut ranked = [0, 1, 2, 3, 4];
        ranked.sort_by(|a, b| scores[*b].total_cmp(&scores[*a]).then(a.cmp(b)));
        let (best, second) = (ranked[0], ranked[1]);
        let margin = scores[best] - scores[second];
        if margin
            < if directory {
                self.min_directory_margin
            } else {
                self.min_margin
            }
        {
            return Prediction::abstain(Abstention::CloseScores);
        }
        // A generic extension (including a repeated name token) is not context.
        // Files require a learned directory feature supporting the winning class.
        // A directory must describe its own leaf; files need context in the
        // nearest two directories, not a remote ancestor called Cache.
        let context = if self.classes[best] == Purpose::Logs && input.log_extension {
            &input.log_context
        } else {
            &input.context
        };
        if !context.iter().any(|token| {
            self.tokens
                .get(token)
                .is_some_and(|w| w[best] - w[second] >= 1.0)
        }) {
            return Prediction::abstain(Abstention::MissingContext);
        }
        let purpose = self.classes[best];
        if matches!(purpose, Purpose::Cache | Purpose::Logs | Purpose::Temporary) {
            if input.database_file {
                return Prediction::abstain(Abstention::ProtectedDatabaseFile);
            }
            if input.protected_extension {
                return Prediction::abstain(Abstention::ProtectedExtension);
            }
            // Learned conflicting directory evidence can only suppress a hint.
            if input.ancestors.iter().any(|token| {
                self.tokens.get(token).is_some_and(|weights| {
                    weights[Purpose::Personal.index()]
                        .max(weights[Purpose::ApplicationData.index()])
                        - weights[best]
                        >= 1.0
                })
            }) {
                return Prediction::abstain(Abstention::ConflictingContext);
            }
        }
        let mut evidence: Vec<_> = matched
            .into_iter()
            .filter(|(f, w)| !f.starts_with("kind:") && w[best] > w[second])
            .collect();
        evidence.sort_by(|(fa, a), (fb, b)| {
            (b[best] - b[second])
                .total_cmp(&(a[best] - a[second]))
                .then(fa.cmp(fb))
        });
        Prediction {
            hint: Some(Hint {
                purpose,
                model_id: self.model_id.clone(),
                score_margin: margin,
                evidence: evidence
                    .into_iter()
                    .take(3)
                    .map(|(f, _)| f.clone())
                    .collect(),
            }),
            abstention: None,
        }
    }
}

fn spread(weights: &[f64; CLASS_COUNT]) -> f64 {
    weights.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - weights.iter().copied().fold(f64::INFINITY, f64::min)
}

/// Shared by training and inference. Bounded, case-insensitive, extension-aware.
/// Ignore drive letters and arbitrary ancestors; never read from the supplied path.
pub fn features(path: &str, directory: bool) -> BTreeSet<String> {
    PathFeatures::new(path, directory)
        .map(|input| input.tokens)
        .unwrap_or_default()
}

struct PathFeatures {
    tokens: BTreeSet<String>,
    context: Vec<String>,
    log_context: Vec<String>,
    log_extension: bool,
    ancestors: Vec<String>,
    sensitive: bool,
    protected_extension: bool,
    database_file: bool,
}
impl PathFeatures {
    fn new(path: &str, directory: bool) -> Option<Self> {
        if path.is_empty() || path.len() > 8192 || path.chars().any(char::is_control) {
            return None;
        }
        let normalized = path.replace('\\', "/").to_lowercase();
        let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized);
        let parts: Vec<_> = normalized.split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() > 128
            || parts.iter().enumerate().any(|(i, p)| {
                *p == "."
                    || *p == ".."
                    || p.ends_with([' ', '.'])
                    || p.contains(['*', '?', '"', '<', '>', '|'])
                    || (p.contains(':')
                        && !(i == 0
                            && p.len() == 2
                            && p.as_bytes()[0].is_ascii_alphabetic()
                            && p.ends_with(':')))
            })
        {
            return None;
        }
        let leaf = *parts.last()?;
        if leaf.ends_with(':') {
            return None;
        }
        let directories = if directory {
            &parts[..]
        } else {
            &parts[..parts.len() - 1]
        };
        let mut tokens = BTreeSet::new();
        let mut context = Vec::new();
        let mut log_context = Vec::new();
        let (stem, extension) = if directory {
            (leaf, None)
        } else {
            split_extension(leaf)
        };
        let log_extension = extension == Some("log");
        let ancestors = directories
            .iter()
            .filter(|part| part.len() <= 96 && !part.contains(':'))
            .map(|part| format!("dir:{part}"))
            .collect();
        tokens.insert(format!(
            "kind:{}",
            if directory { "directory" } else { "file" }
        ));
        for (distance, part) in directories
            .iter()
            .rev()
            .take(if log_extension { 6 } else { 4 })
            .enumerate()
        {
            if part.len() <= 96 && !part.contains(':') {
                let token = format!("dir:{part}");
                if distance < if directory { 1 } else { 2 } {
                    context.push(token.clone());
                }
                if log_extension {
                    log_context.push(token.clone());
                }
                tokens.insert(token);
            }
        }
        if !directory {
            // The extension contributes once, rather than again as a name word.
            for word in stem
                .split(|c: char| !c.is_alphabetic())
                .filter(|w| (2..=48).contains(&w.chars().count()))
                .take(6)
            {
                tokens.insert(format!("name:{word}"));
            }
            if let Some(ext) = extension {
                tokens.insert(format!("ext:{ext}"));
            }
        }
        Some(Self {
            tokens,
            context,
            log_context,
            log_extension,
            ancestors,
            sensitive: parts.iter().any(|p| sensitive_component(p)),
            protected_extension: !directory && protected_extension(leaf, extension),
            database_file: !directory && database_file(leaf, stem, extension),
        })
    }
}

fn split_extension(leaf: &str) -> (&str, Option<&str>) {
    let mut value = leaf;
    // Numeric log rotations retain their type; arbitrary numeric suffixes do not.
    if let Some((base, rotation)) = leaf.rsplit_once('.')
        && !rotation.is_empty()
        && rotation.bytes().all(|b| b.is_ascii_digit())
        && base.ends_with(".log")
    {
        value = base;
    }
    match value.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && ext.len() <= 16 => {
            (stem, Some(ext))
        }
        _ => (value, None),
    }
}

fn sensitive_component(p: &str) -> bool {
    matches!(
        p,
        "cookies"
            | "login data"
            | "bookmarks"
            | "history"
            | "sessions"
            | "local storage"
            | "session storage"
            | "indexeddb"
            | "db_storage"
            | "leveldb"
            | "localcache"
            | "wallets"
            | ".ssh"
            | ".git"
            | "source"
            | "src"
    )
}
fn database_file(leaf: &str, stem: &str, extension: Option<&str>) -> bool {
    let numeric = |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit());
    // LevelDB's numeric *.log is a write-ahead log, whereas LOG/LOG.old
    // contain diagnostics. A filename alone cannot distinguish a numeric
    // diagnostic log from database updates, so suppress disposable hints.
    // The shared extension splitter also protects numeric *.log.N rotations.
    matches!(extension, Some("ldb" | "sst" | "dbtmp"))
        || (extension == Some("log") && numeric(stem))
        || matches!(leaf, "current" | "lock")
        || leaf.strip_prefix("manifest-").is_some_and(numeric)
}
fn protected_extension(leaf: &str, extension: Option<&str>) -> bool {
    [".sqlite-wal", ".sqlite-shm", ".db-wal", ".db-shm"]
        .iter()
        .any(|suffix| leaf.ends_with(suffix))
        || extension.is_some_and(|ext| {
            matches!(
                ext,
                "doc"
                    | "docx"
                    | "xls"
                    | "xlsx"
                    | "ppt"
                    | "pptx"
                    | "pdf"
                    | "jpg"
                    | "jpeg"
                    | "png"
                    | "mp4"
                    | "mp3"
                    | "zip"
                    | "7z"
                    | "bak"
                    | "db"
                    | "sqlite"
                    | "sqlite3"
                    | "kdbx"
                    | "pem"
                    | "key"
                    | "json"
                    | "ini"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "xml"
                    | "cfg"
                    | "flac"
                    | "wav"
                    | "mov"
                    | "mkv"
                    | "webp"
                    | "heic"
                    | "gif"
                    | "blend"
                    | "psd"
                    | "dwg"
                    | "sql"
                    | "mdb"
                    | "accdb"
                    | "exe"
                    | "dll"
                    | "sys"
                    | "msi"
                    | "pfx"
                    | "p12"
                    | "rs"
                    | "py"
                    | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "c"
                    | "cpp"
                    | "h"
                    | "md"
                    | "txt"
                    | "csv"
                    | "rar"
                    | "tar"
                    | "gz"
                    | "gguf"
                    | "safetensors"
            )
        })
}

/// Fallback only. No mutation of rule IDs, manual capability, kind or selection.
pub fn hint_for(entry: &AnalysisEntry, advice: &AnalysisAdvice) -> Option<Hint> {
    if advice.recognition.is_some()
        || !entry.rule_ids.is_empty()
        || entry.excluded
        || entry.pending
        || entry.incomplete
        || entry.issue.is_some()
        || !matches!(
            advice.kind,
            AnalysisPurpose::Unknown | AnalysisPurpose::AppData
        )
    {
        return None;
    }
    MODEL.as_ref()?.predict(&entry.path, entry.directory)
}

#[cfg(test)]
mod tests;
