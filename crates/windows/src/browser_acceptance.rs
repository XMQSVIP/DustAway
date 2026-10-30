//! Opt-in acceptance bridge for scripts/browser-validation.mjs, absent from application builds.
use super::*;
use dustaway_core::{Engine, JobState, Selection};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn wait(engine: &Engine) {
    let until = Instant::now() + Duration::from_secs(120);
    while engine.is_busy() {
        assert!(Instant::now() < until, "browser fixture task timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "requires an isolated real-browser fixture; run scripts/browser-validation.mjs"]
fn real_browser_acceptance() {
    let root = PathBuf::from(std::env::var("DUSTAWAY_BROWSER_FIXTURE").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("dustaway-browser-fixture-")
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".dustaway-test-only")).unwrap(),
        "browser-fixture-only"
    );
    let app = std::env::var("DUSTAWAY_BROWSER_APP").unwrap();
    assert!(matches!(app.as_str(), "brave" | "vivaldi"));
    let action = std::env::var("DUSTAWAY_BROWSER_ACTION").unwrap();
    assert!(matches!(action.as_str(), "scan" | "blocked" | "clean"));
    let platform = Arc::new(WindowsPlatform {
        inventory: Default::default(),
        tool_roots: Default::default(),
        tool_probe: std::sync::Arc::new(crate::tool_discovery::UnavailableProbe),
        real_app_acceptance: None,
        log_test_acceptance: None,
        discovery_drives: vec![],
        app_test_identity: None,
        app_test_acceptance: None,
        scope: Scope::User,
        local: extended(&root.join("local")).unwrap(),
        roaming: extended(&root.join("roaming")).unwrap(),
        system: extended(&root.join("system")).unwrap(),
    });
    let engine = Engine::open(
        platform,
        Scope::User,
        &root.join(format!("{app}-{action}.sqlite")),
    )
    .unwrap();
    let ids: Vec<_> = dustaway_core::rules::catalog()
        .unwrap()
        .into_iter()
        .filter(|r| r.root == app)
        .map(|r| r.id)
        .collect();
    let start = Instant::now();
    let session = engine.start_scan(ids.clone()).unwrap();
    wait(&engine);
    assert_eq!(engine.status().state, JobState::Ready);
    let summary = engine.summary();
    let scan_ms = start.elapsed().as_millis();
    let selection = Selection {
        rule_ids: ids,
        excluded_ids: vec![],
        ..Selection::default()
    };
    if action == "blocked" {
        assert!(
            summary
                .rules
                .iter()
                .all(|r| r.availability == Some(ReasonCode::BrowserRunning))
        );
        assert!(engine.prepare(&session, selection).is_err());
    } else {
        assert!(summary.rules.iter().all(|r| !r.blocked));
        assert!(
            summary.rules.iter().all(|r| r.files > 0),
            "all three real caches must exist"
        );
        if action == "clean" {
            let plan = engine.prepare(&session, selection).unwrap();
            assert!(plan.files > 0);
            assert!(engine.execute("forged-plan").is_err());
            engine.execute(&plan.id).unwrap();
            wait(&engine);
            let report = engine.report().unwrap();
            assert_eq!(report.deleted, plan.files);
            assert_eq!(report.failed + report.skipped + report.unknown, 0);
        }
    }
    let result = serde_json::json!({ "app": app, "action": action, "scan_ms": scan_ms, "summary": summary, "report": engine.report() });
    std::fs::write(
        root.join(format!("{app}-{action}.json")),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}
