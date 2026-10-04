//! Narrow user-residue rules; documented diagnostics are opt-in, real acceptance is separate.
use crate::{CleanupPolicy, DataKind, RuleDescriptor};
use std::path::Path;

pub struct ResidueRule {
    pub id: &'static str,
    pub relative: &'static str,
    pub age: u32,
    pub kind: DataKind,
}

pub const RULES: &[ResidueRule] = &[
    ResidueRule {
        id: "windows.crash_dumps",
        relative: "CrashDumps",
        age: 30,
        kind: DataKind::Diagnostics,
    },
    ResidueRule {
        id: "windows.wer_archive",
        relative: "Microsoft/Windows/WER/ReportArchive",
        age: 30,
        kind: DataKind::Diagnostics,
    },
    ResidueRule {
        id: "windows.directx_cache",
        relative: "D3DSCache",
        age: 7,
        kind: DataKind::Cache,
    },
];

pub fn rule(id: &str) -> Option<&'static ResidueRule> {
    RULES.iter().find(|r| r.id == id)
}

pub fn valid_descriptor(r: &RuleDescriptor) -> bool {
    rule(&r.id).is_some_and(|s| {
        r.root == "windows_residue"
            && r.relative == s.relative
            && r.min_age_days == s.age
            && r.data_kind == s.kind
            && r.app_id.is_none()
            && r.process.is_none()
            && !r.requires_admin
            && !r.default_selected
            && r.cleanup_policy == CleanupPolicy::Review
            && !r.cleanup_validated
            && r.version == if s.id == "windows.crash_dumps" { 2 } else { 1 }
    })
}

pub fn accepts_file(id: &str, path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match id {
        "windows.crash_dumps" => ext == "dmp",
        "windows.wer_archive" => matches!(ext.as_str(), "wer" | "dmp" | "mdmp" | "hdmp" | "cab"),
        "windows.directx_cache" => matches!(ext.as_str(), "idx" | "val"),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn narrow_extensions_and_review_only_release_gate() {
        for spec in RULES {
            let r = crate::rules::catalog()
                .unwrap()
                .into_iter()
                .find(|r| r.id == spec.id)
                .unwrap();
            assert!(valid_descriptor(&r));
            let mut invalid = r.clone();
            invalid.cleanup_validated = !r.cleanup_validated;
            assert!(!valid_descriptor(&invalid));
            invalid = r.clone();
            invalid.relative = "..".into();
            assert!(!valid_descriptor(&invalid));
            invalid = r;
            invalid.cleanup_policy = CleanupPolicy::Recommended;
            assert!(!valid_descriptor(&invalid));
            assert!(!accepts_file(spec.id, Path::new("personal.docx")));
        }
        assert!(accepts_file("windows.crash_dumps", Path::new("app.DMP")));
        assert!(!accepts_file(
            "windows.directx_cache",
            Path::new("cache.lock")
        ));
    }
}
