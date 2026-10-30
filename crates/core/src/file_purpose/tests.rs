use super::*;

fn entry(path: &str, directory: bool) -> AnalysisEntry {
    serde_json::from_value(serde_json::json!({
        "id": "example", "parent": "root", "name": "example", "path": path,
        "directory": directory, "bytes": "1000", "files": 1, "modified_unix": 0,
        "shared": false, "incomplete": false, "issue": null, "rule_ids": [], "excluded": false
    }))
    .unwrap()
}

#[test]
fn embedded_model_is_small_valid_and_cpu_predictions_use_no_filesystem() {
    assert!(EMBEDDED_MODEL.len() < 64 * 1024);
    let model = Model::load(EMBEDDED_MODEL).unwrap();
    for (path, directory, expected) in [
        (r"Z:\Nonexistent\Cache\entry.cache", false, Purpose::Cache),
        (r"Z:\Nonexistent\logs\renderer.log", false, Purpose::Logs),
        (
            r"Z:\Nonexistent\temp\session.tmp",
            false,
            Purpose::Temporary,
        ),
        (
            r"Z:\Nonexistent\Documents\contract.docx",
            false,
            Purpose::Personal,
        ),
        (
            r"Z:\Nonexistent\database\accounts.db",
            false,
            Purpose::ApplicationData,
        ),
        (r"Z:\Nonexistent\Cache", true, Purpose::Cache),
    ] {
        let hint = model
            .predict(path, directory)
            .unwrap_or_else(|| panic!("abstained: {path}"));
        assert_eq!(hint.purpose, expected, "{path}");
        assert!(
            hint.score_margin.is_finite()
                && hint.score_margin
                    >= if directory {
                        model.min_directory_margin
                    } else {
                        model.min_margin
                    }
        );
        assert!(!hint.evidence.is_empty() && hint.evidence.len() <= 3);
        assert!(hint.evidence.iter().all(|e| !e.contains(['\\', '/'])));
    }
}

#[test]
fn insufficient_context_and_protected_payloads_abstain() {
    let model = Model::load(EMBEDDED_MODEL).unwrap();
    for (path, directory) in [
        (r"D:\Unseen\opaque.qzx", false),
        (r"D:\Unseen\data.bin", false),
        (r"D:\Unseen\file.tmp", false),
        (r"D:\Unseen\mixed", true),
        (r"D:\Cache\contract.docx", false),
        (r"D:\temp\backup.zip", false),
        (r"D:\logs\messages.db", false),
        (r"D:\LocalCache\Cache\entry.cache", false),
        (r"D:\IndexedDB\Cache\entry.cache", false),
        (r"D:\Local Storage\Cache\entry.cache", false),
        (r"D:\db_storage\logs\entry.log", false),
        (r"D:\.ssh\temp\private.key", false),
    ] {
        assert!(model.predict(path, directory).is_none(), "{path}");
    }
}

#[test]
fn features_normalize_case_separators_and_reject_unbounded_or_ambiguous_input() {
    assert_eq!(
        features(r"D:\Tool\CACHE\Entry.CACHE", false),
        features("d:/tool/cache/entry.cache", false)
    );
    assert!(!features("D:/cache.dat", true).contains("ext:dat"));
    assert!(features("D:/cache/../secrets", false).is_empty());
    assert!(features("D:/cache/\0bad", false).is_empty());
    assert!(features(&"x".repeat(8193), false).is_empty());
    assert!(features("D:/账户/应用/缓存/项目.cache", false).contains("dir:缓存"));
}

#[test]
fn invalid_models_fail_without_creating_hints() {
    let valid: serde_json::Value = serde_json::from_str(EMBEDDED_MODEL).unwrap();
    for (field, value) in [
        ("schema_version", serde_json::json!(99)),
        ("feature_version", serde_json::json!(99)),
        ("min_features", serde_json::json!(0)),
        ("min_margin", serde_json::json!(0.0)),
        ("min_directory_margin", serde_json::json!(0.0)),
        ("training_sha256", serde_json::json!("invalid")),
        (
            "tokens",
            serde_json::json!({"dir:cache": [1.0, -2.0, -3.0, -4.0, -5.0]}),
        ),
    ] {
        let mut broken = valid.clone();
        broken[field] = value;
        assert!(Model::load(&broken.to_string()).is_err(), "{field}");
    }
    assert!(Model::load("{}").is_err());
}

#[test]
fn v2_uses_one_extension_signal_and_preserves_numeric_log_rotation() {
    let tokens = features("D:/app/logs/service.log.20261025", false);
    assert!(tokens.contains("ext:log"));
    assert!(tokens.contains("name:service"));
    assert!(!tokens.contains("name:log"));
    assert!(
        !features("D:/app/Cache", true)
            .iter()
            .any(|t| t.starts_with("name:"))
    );
    assert!(features("D:/app/gpucache/data_1", false).contains("dir:gpucache"));
    assert!(features("D:/app/cache-notes/notes", false).contains("dir:cache-notes"));
    assert!(!features("D:/app/cache-notes/notes", false).contains("dir:cache"));
    assert_eq!(
        features(r"\\?\D:\app\logs\file.log", false),
        features("d:/app/logs/file.log", false)
    );
}

#[test]
fn v2_classifies_chinese_metadata_and_rotated_logs_without_gpu() {
    let model = Model::load(EMBEDDED_MODEL).unwrap();
    for (path, directory, expected) in [
        ("D:/Unseen/缓存", true, Purpose::Cache),
        ("D:/Unseen/日志", true, Purpose::Logs),
        ("D:/Unseen/临时", true, Purpose::Temporary),
        ("D:/Unseen/文档", true, Purpose::Personal),
        ("D:/Unseen/配置", true, Purpose::ApplicationData),
        ("D:/Unseen/logs/service.log.17", false, Purpose::Logs),
    ] {
        assert_eq!(
            model.predict(path, directory).map(|h| h.purpose),
            Some(expected),
            "{path}"
        );
    }
}

#[test]
fn v2_does_not_let_cache_terms_override_persistent_context_or_path_validation() {
    let model = Model::load(EMBEDDED_MODEL).unwrap();
    for path in [
        "D:/Cache/Documents/item.cache",
        "D:/Cache/config/item.cache",
        "D:/temp/Backups/item.tmp",
        "D:/logs/database/output.log",
        "D:/Backups/a/b/c/d/Cache/item.cache",
        "D:/Cache/voice.flac",
        "D:/Cache/script.py",
        "D:/Cache/engine.dll",
        "D:/Cache/secret.pfx",
        "D:/Cache/settings.json",
        "D:/Cache/item.cache:secret",
        "D:/Cache /item.cache",
        "D:/Cache/a/b/c/d/item.cache",
        "D:/Cache/not-cache-project/item.qzx",
    ] {
        let result = model.explain(path, false);
        assert!(result.hint.is_none(), "{path}: {result:?}");
        assert!(result.abstention.is_some());
    }
    assert_eq!(
        model
            .explain("D:/Cache/item.cache:secret", false)
            .abstention,
        Some(Abstention::InvalidPath)
    );
    assert_eq!(
        model
            .explain("D:/IndexedDB/Cache/item.cache", false)
            .abstention,
        Some(Abstention::SensitiveLocation)
    );
    assert!(features(&format!("D:/{}item.cache", "a/".repeat(130)), false).is_empty());
}

#[test]
fn hints_are_fallback_only_and_never_change_manual_or_rule_authority() {
    let original = entry(r"D:\Unlisted\Cache\entry.cache", false);
    let advice = AnalysisAdvice::default();
    assert!(hint_for(&original, &advice).is_some());
    assert!(!original.manual.allowed);
    assert!(original.rule_ids.is_empty());
    for kind in [
        AnalysisPurpose::Cache,
        AnalysisPurpose::Personal,
        AnalysisPurpose::BrowserData,
        AnalysisPurpose::SystemData,
        AnalysisPurpose::Incomplete,
    ] {
        assert!(
            hint_for(
                &original,
                &AnalysisAdvice {
                    kind,
                    ..Default::default()
                }
            )
            .is_none()
        );
    }
    let recognized = AnalysisAdvice {
        recognition: crate::recognition::recognize("D:/model.gguf", false),
        ..Default::default()
    };
    assert!(recognized.recognition.is_some());
    assert!(hint_for(&original, &recognized).is_none());
    for flag in ["rule", "pending", "incomplete", "issue", "excluded"] {
        let mut blocked = original.clone();
        match flag {
            "rule" => blocked.rule_ids.push("existing.cache".into()),
            "pending" => blocked.pending = true,
            "incomplete" => blocked.incomplete = true,
            "issue" => blocked.issue = Some("access_denied".into()),
            "excluded" => blocked.excluded = true,
            _ => unreachable!(),
        }
        assert!(hint_for(&blocked, &advice).is_none(), "{flag}");
    }
}

#[test]
fn older_analysis_payloads_remain_compatible() {
    let old = r#"{"kind":"unknown","app_id":null,"basis":"unrecognized_location"}"#;
    let advice: AnalysisAdvice = serde_json::from_str(old).unwrap();
    assert!(advice.model_hint.is_none());
    assert!(
        !serde_json::to_string(&advice)
            .unwrap()
            .contains("model_hint")
    );
}

#[test]
fn nested_log_context_is_bounded_and_does_not_relax_other_file_types() {
    let model = Model::load(EMBEDDED_MODEL).unwrap();
    for path in [
        "D:/Editor/logs/20261029T120000/window1/exthost/renderer.log",
        "D:/Editor/logs/20261029T120000/window1/exthost/output_20261029/extension.log",
        "D:/Editor/logs/20261029T120000/window1/exthost/output_20261029/service.log.17",
        "D:/Editor/日志/会话/窗口/扩展/runtime.log",
    ] {
        assert_eq!(
            model.predict(path, false).map(|h| h.purpose),
            Some(Purpose::Logs),
            "{path}"
        );
    }
    for path in [
        "D:/Editor/logs/a/b/c/d/e/f/renderer.log",
        "D:/Editor/logs/20261029/window1/exthost/contract.pdf",
        "D:/Editor/logs/20261029/window1/exthost/data.bin",
        "D:/Editor/Cache/20261029/window1/exthost/item.cache",
        "D:/Documents/logs/20261029/window1/exthost/service.log",
        "D:/Editor/logs/20261029/config/service.log",
        "D:/Editor/logs/20261029/IndexedDB/service.log",
        "D:/Editor/logs/20261029/window1/notes.txt",
    ] {
        assert!(model.predict(path, false).is_none(), "{path}");
    }
}

#[test]
fn database_files_never_receive_disposable_hints_even_under_log_or_cache_names() {
    let model = Model::load(EMBEDDED_MODEL).unwrap();
    for directory in ["logs", "Cache", "temp", "日志", "缓存", "临时"] {
        for leaf in [
            "000003.log",
            "000003.LOG.17",
            "20261029.log",
            "cache.ldb",
            "log.sst",
            "temp.dbtmp",
            "CURRENT",
            "LOCK",
            "MANIFEST-000001",
        ] {
            let path = format!("D:/Fictional/{directory}/{leaf}");
            let result = model.explain(&path, false);
            assert!(
                result.hint.is_none_or(|hint| !matches!(
                    hint.purpose,
                    Purpose::Cache | Purpose::Logs | Purpose::Temporary
                )),
                "{path}"
            );
        }
    }
    assert_eq!(
        model.explain("D:/logs/000003.log", false).abstention,
        Some(Abstention::ProtectedDatabaseFile)
    );
    for path in [
        "D:/LevelDB/logs/service.log",
        "D:/leveldb/a/b/c/d/Cache/entry.cache",
        "D:/LEVELDB/temp/session.tmp",
    ] {
        assert_eq!(
            model.explain(path, false).abstention,
            Some(Abstention::SensitiveLocation)
        );
    }
    // This is a narrowly matched guard, not a ban on every name containing digits
    // or every directory containing the word leveldb.
    for path in [
        "D:/logs/service123.log",
        "D:/logs/20261029-startup.log",
        "D:/logs/service.log.17",
        "D:/leveldb-notes/logs/service.log",
    ] {
        assert_eq!(
            model.predict(path, false).map(|h| h.purpose),
            Some(Purpose::Logs)
        );
    }
    assert!(!database_file("log", "log", None));
    assert!(!database_file("log.old", "log", Some("old")));
    assert!(!database_file("manifest-notes", "manifest-notes", None));
}
