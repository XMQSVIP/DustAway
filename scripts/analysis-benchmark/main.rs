use dustaway_core::AnalysisEngine;
use dustaway_windows::WindowsAnalysis;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[repr(C)]
#[derive(Default)]
struct Memory {
    cb: u32,
    faults: u32,
    peak_ws: usize,
    ws: usize,
    peak_paged: usize,
    paged: usize,
    peak_nonpaged: usize,
    nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
}
#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(process: *mut std::ffi::c_void, info: *mut Memory, size: u32) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
}
fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap());
    assert!(
        root.is_absolute()
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("dustaway-analysis-bench-")
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".dustaway-test-only")).unwrap(),
        "readonly-analysis-benchmark"
    );
    let count = std::env::args().nth(2).unwrap();
    let location = if count == "1000000" {
        root.join("3000000/first-million")
    } else {
        root.join(&count)
    };
    let comparison = std::env::args().any(|a| a == "--comparison");
    let cache = std::env::var_os("DUSTAWAY_BENCH_INDEX_ROOT")
        .map(PathBuf::from)
        .map(|path| {
            assert!(path.is_absolute());
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("dustaway-analysis-cache-")
            );
            assert_eq!(
                std::fs::read_to_string(path.join(".dustaway-test-only")).unwrap(),
                "readonly-analysis-benchmark-index"
            );
            path
        })
        .unwrap_or_else(|| root.join("index-cache"));
    let engine = AnalysisEngine::with_storage_limit(
        Arc::new(WindowsAnalysis::for_directory(&location)),
        dustaway_core::AnalysisStorage::new(cache.clone()),
        if comparison {
            500_000
        } else {
            dustaway_core::ANALYSIS_LIMIT
        },
    )
    .unwrap();
    let begin = Instant::now();
    let id = engine.start(&engine.locations()[0].token, vec![]).unwrap();
    let cancelling = std::env::args().any(|a| a == "--cancel");
    let cancel_index = std::env::args().any(|a| a == "--cancel-index");
    let query_during = std::env::args().any(|a| a == "--query-during");
    let cancel_query = std::env::args().any(|a| a == "--cancel-query");
    let mut during_query = None;
    let mut cancelled_at = None;
    let mut first = None;
    let mut peak = 0;
    let mut peak_disk = 0;
    let mut disk_sample = Instant::now();
    while engine.is_busy() {
        if engine.snapshot().unwrap().entries > 1 && first.is_none() {
            first = Some(begin.elapsed().as_millis());
        }
        let mut m = Memory {
            cb: std::mem::size_of::<Memory>() as u32,
            ..Default::default()
        };
        unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut m,
                std::mem::size_of::<Memory>() as u32,
            );
        }
        peak = peak.max(m.pagefile);
        if disk_sample.elapsed().as_millis() >= 250 {
            peak_disk = peak_disk.max(index_bytes(&cache));
            disk_sample = Instant::now();
        }
        if (query_during || cancel_query)
            && during_query.is_none()
            && engine.snapshot().unwrap().entries >= 25000
        {
            let q = read_query(&id, 0);
            let request = q.request_id.clone();
            let flag = engine.begin_query(&request).unwrap();
            let other = engine.clone();
            let cancel = cancel_query.then(|| {
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(2));
                    let begin = Instant::now();
                    other.cancel_query(&request);
                    begin
                })
            });
            let start = Instant::now();
            let page = engine.read_registered_page(q, flag);
            let latency = start.elapsed().as_micros();
            let cancelled_read_ms =
                cancel.map(|handle| handle.join().unwrap().elapsed().as_millis());
            during_query = Some(
                serde_json::json!({"elapsed_us":latency,"ok":page.is_ok(),"items":page.as_ref().map(|p|p.items.len()).ok(),"cancel_read_ms":cancelled_read_ms,"scan_still_active":engine.is_busy()}),
            );
            if cancel_query {
                cancelled_at = Some(Instant::now());
                engine.cancel(&id).unwrap();
            }
        }
        if ((cancelling && first.is_some())
            || (cancel_index && engine.snapshot().unwrap().phase == "indexing"))
            && cancelled_at.is_none()
        {
            cancelled_at = Some(Instant::now());
            engine.cancel(&id).unwrap();
        }
        assert!(begin.elapsed() < Duration::from_secs(1800));
        std::thread::sleep(Duration::from_millis(5));
    }
    let elapsed = begin.elapsed().as_millis();
    let cancel_ms = cancelled_at.map(|t| t.elapsed().as_millis());
    let s = engine.snapshot().unwrap();
    let cold_start = Instant::now();
    let cold_page = engine.read_page(read_query(&id, 0));
    let cold_query = serde_json::json!({"elapsed_us":cold_start.elapsed().as_micros(),"ok":cold_page.is_ok(),"items":cold_page.as_ref().ok().map(|r|r.items.len())});
    let mut queries = vec![];
    let mut directory_queries = vec![];
    for i in 0..10 {
        let t = Instant::now();
        let _ = engine
            .files(&id, if i % 2 == 0 { "" } else { "099" }, 0)
            .unwrap();
        queries.push(t.elapsed().as_micros());
        let t = Instant::now();
        let _ = engine.children(&id, "0", "", (i % 3) * 200, true).unwrap();
        directory_queries.push(t.elapsed().as_micros());
    }
    #[cfg(feature = "discovery")]
    let mut discovery_queries: Vec<serde_json::Value> = Vec::new();
    #[cfg(not(feature = "discovery"))]
    let discovery_queries: Vec<serde_json::Value> = Vec::new();
    if cfg!(feature = "discovery") {
        #[cfg(feature = "discovery")]
        for i in 0..12 {
            let query = dustaway_core::AnalysisDiscoveryQuery {
                kind: if i % 2 == 0 {
                    dustaway_core::DiscoveryKind::Directory
                } else {
                    dustaway_core::DiscoveryKind::File
                },
                category: if i % 3 == 0 {
                    "recognized"
                } else if i % 3 == 1 {
                    "all"
                } else {
                    "unclassified"
                }
                .into(),
                min_bytes: "0".into(),
                search: if i >= 6 { "099" } else { "" }.into(),
                include_nested: false,
                incomplete_only: false,
                offset: 0,
            };
            let began = Instant::now();
            let result = engine.discovery(&id, &query).unwrap();
            discovery_queries.push(serde_json::json!({"query":query,"elapsed_us":began.elapsed().as_micros(),"total":result.total}));
        }
    }
    let mut final_memory = Memory {
        cb: std::mem::size_of::<Memory>() as u32,
        ..Default::default()
    };
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut final_memory,
            std::mem::size_of::<Memory>() as u32,
        );
    }
    peak = peak.max(final_memory.peak_pagefile);
    let disk_bytes = index_bytes(&cache);
    peak_disk = peak_disk.max(disk_bytes);
    let mut read_queries = vec![];
    let wait_counts = std::env::args().any(|a| a == "--wait-counts");
    assert!(!wait_counts || cfg!(feature = "query-timing"));
    let mut count_queries = vec![];
    let query_rounds = std::env::args()
        .find_map(|a| {
            a.strip_prefix("--query-rounds=")
                .map(|n| n.parse::<usize>().unwrap())
        })
        .unwrap_or(1);
    assert!((1..=10).contains(&query_rounds));
    for i in 0..18 * query_rounds {
        let q = read_query(&id, i % 18);
        let t = Instant::now();
        let result = engine.read_page(q.clone());
        let needs_count = result.as_ref().is_ok_and(|page| page.total.is_none());
        read_queries.push(serde_json::json!({"case":i,"view":q.view,"metric":q.metric,"search":q.discovery.as_ref().map_or(q.search.as_str(), |d|d.search.as_str()),"elapsed_us":t.elapsed().as_micros(),"ok":result.is_ok(),"items":result.as_ref().ok().map(|r|r.items.len()),"error":result.err().map(|e|e.to_string())}));
        if wait_counts && needs_count {
            let started = Instant::now();
            loop {
                let count = engine.query_count(&q.request_id).unwrap();
                if count.finished || started.elapsed().as_secs() >= 30 {
                    count_queries.push(serde_json::json!({"case":i,"elapsed_us":started.elapsed().as_micros(),"finished":count.finished,"total":count.total,"error":count.error}));
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
    let cancel_request = uuid::Uuid::new_v4().to_string();
    let mut cancel_q = read_query(&id, 1);
    cancel_q.request_id = cancel_request.clone();
    cancel_q.search = "__no_fixture_name_matches__".into();
    let cancel_flag = engine.begin_query(&cancel_request).unwrap();
    let other = engine.clone();
    let cancel_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(5));
        let start = Instant::now();
        other.cancel_query(&cancel_request);
        start
    });
    let cancel_result = engine.read_registered_page(cancel_q, cancel_flag);
    let cancel_read_ms = cancel_thread.join().unwrap().elapsed().as_millis();
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut final_memory,
            std::mem::size_of::<Memory>() as u32,
        );
    }
    peak = peak.max(final_memory.peak_pagefile);
    println!(
        "{}",
        serde_json::json!({"count":count,"scan_ms":s.scan_ms,"write_ms":s.write_ms,"index_ms":s.index_ms,"allocated_bytes":s.allocated_bytes,"allocation_unknown":s.allocation_unknown,"index_disk_bytes":disk_bytes,"peak_index_disk_bytes":peak_disk,"cancel_read_ms":cancel_read_ms,"cancel_read_interrupted":cancel_result.is_err(),"read_queries":read_queries,"count_queries":count_queries,"wait_counts":wait_counts,"cold_query":cold_query,"elapsed_ms":elapsed,"first_entries_ms":first,"cancel_ms":cancel_ms,"peak_private_bytes":peak,"state":s.state,"files":s.files,"entries":s.entries,"logical_bytes":s.logical_bytes,"unique_bytes":s.unique_bytes,"query_us":queries,"directory_query_us":directory_queries,"during_query":during_query,"discovery_queries":discovery_queries,"issues":s.issues,"directory_query_plans":directory_query_plans(&cache)})
    );
    engine.cancel_all();
    engine.close_session().unwrap();
    #[cfg(feature = "query-timing")]
    for timing in dustaway_core::take_analysis_query_timings() {
        eprintln!("{timing}");
    }
}

// Diagnostics run after all timed reads and close their connection before cleanup.
fn directory_query_plans(cache: &std::path::Path) -> Vec<String> {
    let Some(path) = std::fs::read_dir(cache).ok().and_then(|dirs| {
        dirs.flatten()
            .map(|d| d.path().join("index.sqlite"))
            .find(|p| p.is_file())
    }) else {
        return vec![];
    };
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='directory_ranking')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    if !exists {
        return vec![];
    }
    let mut plans = vec![];
    for source in ["nodes", "nodes INDEXED BY directory_ranking"] {
        let mut sql = db.prepare(&format!("EXPLAIN QUERY PLAN SELECT id,pending,allocation_inverse,name FROM {source} WHERE directory=1 AND (parent IS NOT NULL OR recognition>0) AND nested=0 ORDER BY pending,allocation_inverse,name,id LIMIT 201")).unwrap();
        for row in sql.query_map([], |r| r.get::<_, String>(3)).unwrap() {
            plans.push(format!("{source}: {}", row.unwrap()));
        }
    }
    plans
}

fn index_bytes(cache: &std::path::Path) -> u64 {
    std::fs::read_dir(cache)
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|dir| {
            std::fs::read_dir(dir.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .filter_map(|file| file.metadata().ok())
        .map(|m| m.len())
        .sum()
}
fn read_query(id: &str, i: usize) -> dustaway_core::AnalysisReadQuery {
    dustaway_core::AnalysisReadQuery {
        request_id: uuid::Uuid::new_v4().to_string(),
        session: id.into(),
        view: if i >= 12 {
            "discovery"
        } else if i % 3 == 0 {
            "directories"
        } else {
            "files"
        }
        .into(),
        node: "0".into(),
        search: if i % 3 == 2 { "099" } else { "" }.into(),
        metric: if i % 2 == 0 { "allocated" } else { "logical" }.into(),
        descending: true,
        discovery: if i >= 12 {
            Some(dustaway_core::AnalysisDiscoveryQuery {
                kind: if i % 2 == 0 {
                    dustaway_core::DiscoveryKind::Directory
                } else {
                    dustaway_core::DiscoveryKind::File
                },
                category: if i % 3 == 0 { "recognized" } else { "all" }.into(),
                min_bytes: "0".into(),
                search: String::new(),
                include_nested: false,
                incomplete_only: false,
                offset: 0,
            })
        } else {
            None
        },
        cursor: None,
        offset: 0,
    }
}
