//! Opt-in measurement against the same marked 100k fixture as the paired benchmark.
use dustaway_core::AnalysisEngine;
use dustaway_windows::WindowsAnalysis;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires the marked performance fixture and serialized release measurements"]
fn largest_file_query_performance() {
    let root = PathBuf::from(std::env::var("DUSTAWAY_PERF_ROOT").unwrap())
        .canonicalize()
        .unwrap();
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("dustaway-perf-")
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".dustaway-test-only")).unwrap(),
        "performance-fixture-only"
    );
    let engine = AnalysisEngine::new(Arc::new(WindowsAnalysis::for_directory(
        &root.join("old/local/Temp"),
    )))
    .unwrap();
    let token = engine.locations()[0].token.clone();
    let id = engine.start(&token, vec![]).unwrap();
    let wait = || {
        let deadline = Instant::now() + Duration::from_secs(300);
        while engine.is_busy() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    wait();
    assert_eq!(engine.snapshot().unwrap().files, 100_000);
    let mut cases = vec![];
    for (name, search, offset, total) in [
        ("first", "", 0, 100_000),
        ("last", "", 99_800, 100_000),
        ("search-first", "文件00", 0, 10_000),
        ("search-last", "文件00", 9_800, 10_000),
        ("search-empty", "不存在", 0, 0),
    ] {
        let warmup = engine.files(&id, search, offset).unwrap();
        assert_eq!(warmup.total, total);
        let mut samples = vec![];
        for _ in 0..50 {
            let began = Instant::now();
            let page = engine.files(&id, search, offset).unwrap();
            samples.push(began.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(page.total, total);
            assert_eq!(page.items.len(), if total == 0 { 0 } else { 200 });
        }
        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);
        cases.push(serde_json::json!({"case":name,"search":search,"offset":offset,"samples_ms":samples,"p95_ms":sorted[47],"passed":sorted[47]<=200.0}));
    }
    let mut cancellations = vec![];
    for _ in 0..20 {
        let id = engine.start(&token, vec![]).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        assert!(engine.is_busy());
        let began = Instant::now();
        engine.cancel(&id).unwrap();
        wait();
        cancellations.push(began.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(engine.snapshot().unwrap().state, "cancelled");
    }
    let mut sorted = cancellations.clone();
    sorted.sort_by(f64::total_cmp);
    let report = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"fixture":root,"queries":cases,"analysis_cancel_ms":cancellations,"analysis_cancel_p95_ms":sorted[18],"analysis_cancel_passed":sorted[18]<=1000.0});
    // Output stays inside the explicitly marked fixture; never accepts an arbitrary write target.
    std::fs::write(
        root.join("analysis-query-0.8.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
