use dustaway_core::*;
use dustaway_windows::WindowsAnalysis;
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

fn run(root: &Path, cache: &Path) -> (Arc<AnalysisEngine>, String) {
    let engine = AnalysisEngine::with_storage(
        Arc::new(WindowsAnalysis::for_directory(root)),
        AnalysisStorage::new(cache.into()),
    )
    .unwrap();
    let session = engine.start(&engine.locations()[0].token, vec![]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while engine.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(engine.snapshot().unwrap().state, "completed");
    (engine, session)
}
fn query(session: &str) -> AnalysisReadQuery {
    AnalysisReadQuery {
        request_id: uuid::Uuid::new_v4().to_string(),
        session: session.into(),
        view: "files".into(),
        node: "0".into(),
        search: String::new(),
        metric: "allocated".into(),
        descending: true,
        discovery: None,
        cursor: None,
        offset: 0,
    }
}
#[test]
fn disk_index_queries_page_without_duplicates_and_reject_foreign_cursors() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("files");
    std::fs::create_dir(&root).unwrap();
    for i in 0..451 {
        std::fs::write(root.join(format!("文件{i:04}.dat")), vec![b'x'; i + 1024]).unwrap();
    }
    let cache = root.join("cache");
    let (engine, session) = run(&root, &cache);
    assert_eq!(
        engine.snapshot().unwrap().files,
        451,
        "the index must not scan itself"
    );
    let q = query(&session);
    let cancelled = query(&session);
    engine.cancel_query(&cancelled.request_id);
    let active = engine.begin_query(&q.request_id).unwrap();
    assert!(engine.read_page(cancelled).is_err());
    assert!(
        !active.load(std::sync::atomic::Ordering::Acquire),
        "late cancellation cannot stop a newer request"
    );
    let first = engine.read_page(q.clone()).unwrap();
    assert_eq!(first.total, Some(451));
    assert_eq!(first.items.len(), 200);
    let mut next = q.clone();
    next.request_id = uuid::Uuid::new_v4().to_string();
    next.cursor = first.next_cursor;
    next.offset = 200;
    let second = engine.read_page(next.clone()).unwrap();
    assert_eq!(second.items.len(), 200);
    assert!(
        !first
            .items
            .iter()
            .any(|a| second.items.iter().any(|b| a.id == b.id))
    );
    next.request_id = uuid::Uuid::new_v4().to_string();
    next.cursor = second.next_cursor;
    next.offset = 400;
    let last = engine.read_page(next.clone()).unwrap();
    assert_eq!(last.items.len(), 51);
    assert!(last.next_cursor.is_none());
    next.metric = "logical".into();
    next.request_id = uuid::Uuid::new_v4().to_string();
    assert!(engine.read_page(next).is_err());
    let mut search = q;
    search.search = "文件000".into();
    search.request_id = uuid::Uuid::new_v4().to_string();
    assert_eq!(engine.read_page(search).unwrap().total, Some(10));
    let mut counted = query(&session);
    counted.search = "文件".into();
    let request_id = counted.request_id.clone();
    let counted_page = engine.read_page(counted.clone()).unwrap();
    assert_eq!(counted_page.total, None);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = engine.query_count(&request_id).unwrap();
        if status.finished {
            assert_eq!(status.total, Some(451));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    // A new request on the same publication/filter reuses the completed count,
    // including when the page cursor and offset change.
    counted.request_id = uuid::Uuid::new_v4().to_string();
    counted.cursor = counted_page.next_cursor;
    counted.offset = 200;
    let counted_next = engine.read_page(counted.clone()).unwrap();
    assert_eq!(counted_next.items.len(), 200);
    assert_eq!(counted_next.total, Some(451));
    assert!(engine.query_count(&counted.request_id).unwrap().finished);

    let mut changed = counted.clone();
    changed.request_id = uuid::Uuid::new_v4().to_string();
    changed.search = "文件0".into();
    changed.cursor = None;
    changed.offset = 0;
    assert_eq!(engine.read_page(changed.clone()).unwrap().total, None);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !engine.query_count(&changed.request_id).unwrap().finished {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        engine.query_count(&changed.request_id).unwrap().total,
        Some(451)
    );

    // Updating the publication must invalidate a previously completed count.
    let session_path = std::fs::read_dir(&cache)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let writer = rusqlite::Connection::open(session_path.join("index.sqlite")).unwrap();
    writer
        .execute("UPDATE publication SET revision=revision+1", [])
        .unwrap();
    drop(writer);
    changed.request_id = uuid::Uuid::new_v4().to_string();
    changed.cursor = None;
    changed.offset = 0;
    assert_eq!(engine.read_page(changed.clone()).unwrap().total, None);
    engine.cancel_query(&changed.request_id);

    let files = engine.files(&session, "", 0).unwrap();
    assert!(
        files
            .items
            .iter()
            .all(|e| !e.path.is_empty() && e.allocated_bytes.is_some())
    );
    engine.close_session().unwrap();
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
}
#[test]
fn hardlinks_are_deduplicated_inside_each_subtree_and_at_the_scan_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("files");
    let a = root.join("a");
    let b = root.join("b");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    std::fs::write(a.join("original"), vec![5; 65536]).unwrap();
    std::fs::hard_link(a.join("original"), a.join("same-folder")).unwrap();
    std::fs::hard_link(a.join("original"), b.join("other-folder")).unwrap();
    let (engine, session) = run(&root, &temp.path().join("cache"));
    let snap = engine.snapshot().unwrap();
    assert_eq!(snap.logical_bytes, (65536 * 3).to_string());
    assert_eq!(snap.unique_bytes, "65536");
    assert_eq!(snap.allocated_bytes, "65536");
    assert_eq!(snap.allocation_unknown, 0);
    let children = engine.children(&session, "0", "", 0, true).unwrap();
    assert_eq!(children.node.allocated_bytes.as_deref(), Some("65536"));
    assert!(
        children
            .items
            .iter()
            .all(|e| e.allocated_bytes.as_deref() == Some("65536") && e.shared)
    );
    engine.close_session().unwrap();
}
#[test]
fn low_free_space_rejects_starting_an_index() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("files");
    std::fs::create_dir(&root).unwrap();
    let mut storage = AnalysisStorage::new(temp.path().join("cache"));
    storage.reserve = u64::MAX - 128 * 1024 * 1024;
    let engine =
        AnalysisEngine::with_storage(Arc::new(WindowsAnalysis::for_directory(&root)), storage)
            .unwrap();
    assert!(engine.start(&engine.locations()[0].token, vec![]).is_err());
    assert!(engine.snapshot().is_none());
}

#[test]
fn discovery_category_filter_preserves_size_order_and_unknown_category_results() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("files");
    std::fs::create_dir(&root).unwrap();
    for (name, size) in [
        ("large.rs", 8192),
        ("small.rs", 1024),
        ("unknown.dat", 16384),
    ] {
        std::fs::write(root.join(name), vec![b'x'; size]).unwrap();
    }
    let (engine, session) = run(&root, &temp.path().join("cache"));
    let mut q = query(&session);
    q.view = "discovery".into();
    q.metric = "logical".into();
    q.discovery = Some(AnalysisDiscoveryQuery {
        kind: DiscoveryKind::File,
        category: "recognized".into(),
        min_bytes: "0".into(),
        search: String::new(),
        include_nested: false,
        incomplete_only: false,
        offset: 0,
    });
    let result = engine.read_page(q.clone()).unwrap();
    assert_eq!(result.total, Some(2));
    assert_eq!(result.items[0].name, "large.rs");
    assert_eq!(result.items[1].name, "small.rs");
    q.request_id = uuid::Uuid::new_v4().to_string();
    q.descending = false;
    assert_eq!(
        engine.read_page(q.clone()).unwrap().items[0].name,
        "small.rs"
    );
    q.request_id = uuid::Uuid::new_v4().to_string();
    q.discovery.as_mut().unwrap().category = "unclassified".into();
    let result = engine.read_page(q.clone()).unwrap();
    assert_eq!(result.total, Some(1));
    assert_eq!(result.items[0].name, "unknown.dat");
    q.request_id = uuid::Uuid::new_v4().to_string();
    q.discovery.as_mut().unwrap().category = "recognized".into();
    q.discovery.as_mut().unwrap().search = "missing".into();
    assert_eq!(engine.read_page(q).unwrap().total, Some(0));
    engine.close_session().unwrap();
}

#[test]
fn directory_discovery_pages_both_metrics_and_counts_only_matching_directories() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("files");
    std::fs::create_dir(&root).unwrap();
    for i in 0..230 {
        let dir = root.join(format!("目录{i:03}"));
        std::fs::create_dir(&dir).unwrap();
        if i % 10 != 0 {
            std::fs::write(dir.join("payload.dat"), vec![b'x'; (i % 8 + 1) * 4096]).unwrap();
        }
    }
    let (engine, session) = run(&root, &temp.path().join("cache"));
    let mut q = query(&session);
    q.view = "discovery".into();
    q.discovery = Some(AnalysisDiscoveryQuery {
        kind: DiscoveryKind::Directory,
        category: "all".into(),
        min_bytes: "0".into(),
        search: "目录".into(),
        include_nested: false,
        incomplete_only: false,
        offset: 0,
    });
    for metric in ["logical", "allocated"] {
        for descending in [false, true] {
            q.request_id = uuid::Uuid::new_v4().to_string();
            q.metric = metric.into();
            q.descending = descending;
            q.cursor = None;
            q.offset = 0;
            let first = engine.read_page(q.clone()).unwrap();
            assert_eq!(first.items.len(), 200);
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let count = engine.query_count(&q.request_id).unwrap();
                if count.finished {
                    assert_eq!(count.total, Some(230));
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            q.request_id = uuid::Uuid::new_v4().to_string();
            q.cursor = first.next_cursor;
            q.offset = 200;
            let second = engine.read_page(q.clone()).unwrap();
            assert_eq!(second.items.len(), 30);
            assert!(second.next_cursor.is_none());
            let items: Vec<_> = first.items.into_iter().chain(second.items).collect();
            assert!(items.iter().all(|e| e.directory && e.parent.is_some()));
            assert_eq!(
                items
                    .iter()
                    .map(|e| &e.id)
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                230
            );
            let bytes = |entry: &AnalysisEntry| -> u64 {
                if metric == "allocated" {
                    entry.allocated_bytes.as_ref().unwrap().parse().unwrap()
                } else {
                    entry.bytes.parse().unwrap()
                }
            };
            assert!(items.windows(2).all(|pair| {
                let order = bytes(&pair[0]).cmp(&bytes(&pair[1]));
                let order = if descending { order.reverse() } else { order };
                order.then_with(|| pair[0].name.cmp(&pair[1].name)).is_le()
            }));
        }
    }
    q.request_id = uuid::Uuid::new_v4().to_string();
    q.cursor = None;
    q.offset = 0;
    q.metric = "logical".into();
    let filter = q.discovery.as_mut().unwrap();
    filter.category = "unclassified".into();
    filter.min_bytes = "16384".into();
    filter.search = "目录0".into();
    let selected = engine.read_page(q.clone()).unwrap();
    let expected = (0..100)
        .filter(|i| i % 10 != 0 && (i % 8 + 1) * 4096 >= 16384)
        .count();
    assert_eq!(selected.items.len(), expected);
    assert_eq!(selected.total, Some(expected as u64));
    q.request_id = uuid::Uuid::new_v4().to_string();
    q.discovery.as_mut().unwrap().incomplete_only = true;
    assert!(engine.read_page(q).unwrap().items.is_empty());
    engine.close_session().unwrap();
}

#[test]
fn disk_quota_keeps_a_queryable_committed_prefix_and_cleans_up() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("files");
    std::fs::create_dir(&root).unwrap();
    for i in 0..3000 {
        std::fs::write(root.join(format!("file-{i:04}")), b"fixture").unwrap();
    }
    let cache = temp.path().join("cache");
    let mut config = AnalysisStorage::new(cache.clone());
    config.budget = 65 * 1024 * 1024;
    let engine =
        AnalysisEngine::with_storage(Arc::new(WindowsAnalysis::for_directory(&root)), config)
            .unwrap();
    let session = engine.start(&engine.locations()[0].token, vec![]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while engine.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.state, "partial");
    assert!(snapshot.issues.iter().any(|i| i.code == "resource_limit"));
    assert!(snapshot.files > 0 && snapshot.files < 3000);
    assert!(!engine.read_page(query(&session)).unwrap().items.is_empty());
    engine.close_session().unwrap();
    assert_eq!(std::fs::read_dir(cache).unwrap().count(), 0);
}
