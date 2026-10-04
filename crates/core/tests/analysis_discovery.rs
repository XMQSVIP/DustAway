use anyhow::Result;
use dustaway_core::*;
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

struct Source {
    partial: bool,
    wait_for_cancel: bool,
    recognized_root: bool,
}
impl AnalysisPlatform for Source {
    fn locations(&self) -> Vec<(String, String)> {
        vec![(
            "fixture".into(),
            if self.recognized_root {
                r"C:\fixture\.venv"
            } else {
                r"C:\fixture"
            }
            .into(),
        )]
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        Ok(AnalysisRoot {
            path: path.into(),
            opaque: String::new(),
        })
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        Ok(())
    }
    fn scan(
        &self,
        root: &AnalysisRoot,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        let mut add = |id: &str,
                       parent: Option<&str>,
                       name: &str,
                       path: &str,
                       dir: bool,
                       bytes: u64,
                       key: &str|
         -> Result<bool> {
            emit(AnalysisEvent::Entry(Box::new(AnalysisRecord {
                entry: AnalysisEntry {
                    allocated_bytes: None,
                    allocation_unknown: 0,
                    verification_pending: false,
                    manual: Default::default(),
                    ancestors: vec![],
                    id: id.into(),
                    parent: parent.map(Into::into),
                    name: name.into(),
                    path: format!("{}{}", root.path, path),
                    directory: dir,
                    bytes: bytes.to_string(),
                    files: if dir { 0 } else { 1 },
                    modified_unix: 0,
                    shared: key == "shared",
                    incomplete: false,
                    pending: false,
                    issue: None,
                    rule_ids: vec![],
                    excluded: false,
                    advice: None,
                },
                file_key: (!dir).then(|| key.into()),
                opaque: vec![],
            })))
        };
        add("0", None, "fixture", "", true, 0, "")?;
        for (id, parent, name, path) in [
            ("1", "0", "node_modules", r"\node_modules"),
            ("2", "1", "内部库", r"\node_modules\内部库"),
            ("3", "2", ".git", r"\node_modules\内部库\.git"),
            ("4", "0", "未归类目录", r"\普通目录"),
            ("5", "0", "待统计", r"\待统计"),
        ] {
            if !add(id, Some(parent), name, path, true, 0, "")? {
                return Ok(());
            }
        }
        add(
            "small",
            "2".into(),
            "small.js",
            r"\node_modules\内部库\small.js",
            false,
            60,
            "shared",
        )?;
        add(
            "shared",
            "4".into(),
            "shared.bin",
            r"\普通目录\shared.bin",
            false,
            60,
            "shared",
        )?;
        add(
            "git",
            "3".into(),
            "object",
            r"\node_modules\内部库\.git\object",
            false,
            40,
            "git",
        )?;
        add(
            "huge",
            "0".into(),
            "huge.unknown",
            r"\huge.unknown",
            false,
            9_007_199_254_740_993,
            "huge",
        )?;
        add(
            "exact",
            "0".into(),
            "exact.gguf",
            r"\exact.gguf",
            false,
            104_857_600,
            "exact",
        )?;
        if self.wait_for_cancel {
            // Keep this fixture scanning after emitting known files, independent of
            // scheduling delays on a loaded host. The test observes progress first.
            let deadline = Instant::now() + Duration::from_secs(10);
            while !cancel.load(std::sync::atomic::Ordering::Acquire) {
                anyhow::ensure!(Instant::now() < deadline, "fixture cancellation timed out");
                std::thread::sleep(Duration::from_millis(1));
            }
            return Ok(());
        }
        for n in 0..205 {
            if cancel.load(std::sync::atomic::Ordering::Acquire) {
                return Ok(());
            }
            let name = format!("model-{n:03}.gguf");
            if !add(
                &name,
                Some("0"),
                &name,
                &format!("\\{name}"),
                false,
                1_000_000,
                &name,
            )? {
                return Ok(());
            }
        }
        for name in [
            "installer.msix",
            "main.rs",
            "disk.vhdx",
            "world.mcworld",
            "mail.pst",
            "photo.png",
            "office.docx",
            "db.sqlite",
            "archive.zip",
        ] {
            if !add(name, Some("0"), name, &format!("\\{name}"), false, 1, name)? {
                return Ok(());
            }
        }
        for id in ["0", "1", "2", "3", "4"] {
            emit(AnalysisEvent::DirectoryStarted {
                node: id.into(),
                path: String::new(),
            })?;
            emit(AnalysisEvent::DirectoryFinished { node: id.into() })?;
        }
        if self.partial {
            emit(AnalysisEvent::Issue {
                node: "0".into(),
                code: "entry_limit".into(),
            })?;
        } else {
            emit(AnalysisEvent::DirectoryFinished { node: "5".into() })?;
        }
        Ok(())
    }
}
fn query(kind: DiscoveryKind) -> AnalysisDiscoveryQuery {
    AnalysisDiscoveryQuery {
        kind,
        category: "all".into(),
        min_bytes: "0".into(),
        search: String::new(),
        include_nested: false,
        incomplete_only: false,
        offset: 0,
    }
}
fn wait(engine: &AnalysisEngine) {
    let start = Instant::now();
    while engine.is_busy() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn start(partial: bool, recognized_root: bool) -> (Arc<AnalysisEngine>, String) {
    let engine = AnalysisEngine::new(Arc::new(Source {
        partial,
        recognized_root,
        wait_for_cancel: false,
    }))
    .unwrap();
    let session = engine
        .start(
            &engine.locations()[0].token,
            vec![r"C:\fixture\node_modules".into()],
        )
        .unwrap();
    wait(&engine);
    (engine, session)
}
#[test]
fn directories_use_fresh_aggregates_and_fold_by_rule_not_category() {
    let (engine, session) = start(false, false);
    let mut q = query(DiscoveryKind::Directory);
    q.category = "development".into();
    let page = engine.discovery(&session, &q).unwrap();
    assert_eq!(
        page.items.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        ["1", "3"]
    );
    assert_eq!(page.items[0].bytes, "100"); // No prior children query updated size_key.
    assert!(
        page.items
            .iter()
            .all(|e| e.excluded && e.rule_ids.is_empty())
    );
    q.search = "内部库".into();
    assert_eq!(engine.discovery(&session, &q).unwrap().total, 0);
    q.include_nested = true;
    let page = engine.discovery(&session, &q).unwrap();
    assert_eq!(page.items[0].id, "2");
    q.min_bytes = "101".into();
    assert_eq!(engine.discovery(&session, &q).unwrap().total, 0);
    q = query(DiscoveryKind::Directory);
    q.category = "unclassified".into();
    let page = engine.discovery(&session, &q).unwrap();
    assert_eq!(page.total, 2);
    assert!(page.items.iter().all(|e| e.id != "0"));
    let (root, s) = start(false, true);
    q.category = "recognized".into();
    assert!(
        root.discovery(&s, &q)
            .unwrap()
            .items
            .iter()
            .any(|e| e.id == "0")
    );
}
#[test]
fn filters_cover_all_pages_categories_and_exact_u64_sizes() {
    let (engine, session) = start(false, false);
    let mut q = query(DiscoveryKind::File);
    let page = engine.discovery(&session, &q).unwrap();
    assert_eq!(page.items[0].bytes, "9007199254740993");
    q.category = "ai".into();
    let page = engine.discovery(&session, &q).unwrap();
    assert_eq!(page.total, 206);
    assert_eq!(page.items.len(), 200);
    q.offset = 200;
    let last = engine.discovery(&session, &q).unwrap();
    assert_eq!(last.items.len(), 6);
    assert!(
        last.items
            .iter()
            .all(|e| !page.items.iter().any(|p| p.id == e.id))
    );
    q.offset = 0;
    q.search = "model-204".into();
    assert_eq!(
        engine.discovery(&session, &q).unwrap().items[0].name,
        "model-204.gguf"
    );
    q.search.clear();
    q.min_bytes = "104857600".into();
    assert_eq!(engine.discovery(&session, &q).unwrap().total, 1);
    q.min_bytes = "104857601".into();
    assert_eq!(engine.discovery(&session, &q).unwrap().total, 0);
    q.category = "unclassified".into();
    q.min_bytes = "9007199254740993".into();
    assert_eq!(engine.discovery(&session, &q).unwrap().total, 1);
    q.min_bytes = "9007199254740994".into();
    assert_eq!(engine.discovery(&session, &q).unwrap().total, 0);
    q.min_bytes = "0".into();
    for category in recognition::Category::ALL {
        q.category = category.key().into();
        assert!(
            engine.discovery(&session, &q).unwrap().total > 0,
            "{}",
            category.key()
        );
    }
    for e in page.items {
        assert_eq!(
            e.advice
                .as_ref()
                .unwrap()
                .recognition
                .as_ref()
                .unwrap()
                .category,
            Some(recognition::Category::Ai)
        );
        assert!(e.rule_ids.is_empty());
    }
    assert!(
        engine
            .snapshot()
            .unwrap()
            .logical_bytes
            .parse::<u64>()
            .unwrap()
            > engine
                .snapshot()
                .unwrap()
                .unique_bytes
                .parse::<u64>()
                .unwrap()
    );
}
#[test]
fn partial_pending_and_invalid_queries_never_look_complete() {
    let (engine, session) = start(true, false);
    let mut q = query(DiscoveryKind::Directory);
    q.include_nested = true;
    q.incomplete_only = true;
    let page = engine.discovery(&session, &q).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "5");
    assert!(page.items[0].pending);
    q.min_bytes = "1".into();
    assert_eq!(engine.discovery(&session, &q).unwrap().total, 0);
    for value in ["-1", "18446744073709551616", "x", ""] {
        q.min_bytes = value.into();
        assert!(engine.discovery(&session, &q).is_err());
    }
    q.min_bytes = "0".into();
    q.category = "forged".into();
    assert!(engine.discovery(&session, &q).is_err());
    q.category = "all".into();
    assert!(engine.discovery("old-session", &q).is_err());
    q.kind = DiscoveryKind::File;
    assert!(engine.discovery(&session, &q).is_err());
}
#[test]
fn running_rejected_cancelled_and_resource_limited_results_remain_queryable() {
    let engine = AnalysisEngine::new(Arc::new(Source {
        partial: false,
        recognized_root: false,
        wait_for_cancel: true,
    }))
    .unwrap();
    let session = engine.start(&engine.locations()[0].token, vec![]).unwrap();
    let q = query(DiscoveryKind::File);
    assert!(engine.discovery(&session, &q).is_err());
    let deadline = Instant::now() + Duration::from_secs(10);
    while engine.snapshot().unwrap().files == 0 {
        assert!(Instant::now() < deadline, "fixture did not emit any files");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(engine.discovery(&session, &q).is_err());
    engine.cancel(&session).unwrap();
    wait(&engine);
    assert_eq!(engine.snapshot().unwrap().state, "cancelled");
    assert!(!engine.discovery(&session, &q).unwrap().items.is_empty());
    let limited = AnalysisEngine::with_resource_limits(
        Arc::new(Source {
            partial: false,
            recognized_root: false,
            wait_for_cancel: false,
        }),
        500_000,
        64 * 1024,
    )
    .unwrap();
    let session = limited
        .start(&limited.locations()[0].token, vec![])
        .unwrap();
    wait(&limited);
    assert_ne!(limited.snapshot().unwrap().state, "completed");
    assert_eq!(
        limited.discovery(&session, &q).unwrap().total,
        limited.snapshot().unwrap().files
    );
}
