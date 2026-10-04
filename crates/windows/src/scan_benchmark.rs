//! Read-only, opt-in release benchmark; never linked into the application.
use super::*;
use dustaway_core::{AnalysisEngine, Engine, JobState, Settings};
use std::{
    io::Write,
    sync::{Arc, Mutex},
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
    fn GetProcessHandleCount(process: *mut std::ffi::c_void, count: *mut u32) -> i32;
}
#[derive(Default, serde::Serialize)]
struct Resources {
    working_set: usize,
    private_commit: usize,
    handles: u32,
}
impl Resources {
    fn sample(&mut self) {
        let mut m = Memory {
            cb: size_of::<Memory>() as u32,
            ..Default::default()
        };
        let mut handles = 0;
        unsafe {
            assert_ne!(
                GetProcessMemoryInfo(GetCurrentProcess(), &mut m, size_of::<Memory>() as u32),
                0
            );
            assert_ne!(GetProcessHandleCount(GetCurrentProcess(), &mut handles), 0);
        }
        self.working_set = self.working_set.max(m.ws);
        self.private_commit = self.private_commit.max(m.pagefile);
        self.handles = self.handles.max(handles);
    }
}
struct Measured {
    platform: WindowsPlatform,
    candidates: Mutex<Vec<(String, u64)>>,
}
impl Platform for Measured {
    fn scan(
        &self,
        r: &RuleDescriptor,
        x: &[String],
        c: &AtomicBool,
        e: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        let mut files = Vec::new();
        let result = self.platform.scan(r, x, c, &mut |event| {
            if let ScanEvent::File(f) = &event {
                files.push((f.path_display.clone(), f.bytes));
            }
            e(event);
        });
        *self.candidates.lock().unwrap() = files;
        result
    }
    fn blocked(&self, _: &RuleDescriptor) -> Result<Option<dustaway_core::BlockReason>> {
        Ok(None)
    }
    fn delete(&self, _: &RuleDescriptor, _: &FileRecord, _: &[String]) -> Result<()> {
        anyhow::bail!("benchmark never deletes")
    }
    fn free_space(&self) -> Result<u64> {
        Ok(0)
    }
}

#[test]
#[ignore = "marked fixture only; scripts/scan-performance.mjs"]
fn performance_fixture() {
    let root = PathBuf::from(std::env::var("DUSTAWAY_PERF_ROOT").unwrap());
    assert!(root.is_absolute());
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
    let mode = std::env::var("DUSTAWAY_PERF_MODE").unwrap();
    let case = std::env::var("DUSTAWAY_PERF_CASE").unwrap();
    assert!(matches!(
        case.as_str(),
        "recent" | "mixed" | "old" | "deep" | "excluded"
    ));
    let base = root.join(&case);
    let cache = base.join("local/Temp");
    if mode == "seed" {
        assert!(!cache.exists(), "never overwrite an existing fixture");
        let recent = filetime::FileTime::from_unix_time(now(), 0);
        let old = filetime::FileTime::from_unix_time(now() - 30 * 86400, 0);
        let count = if case == "deep" { 10000 } else { 100000 };
        for group in 0..count / 1000 {
            let mut dir = cache.join(format!("目录{group:03}"));
            if case == "deep" {
                for n in 0..24 {
                    dir.push(format!("层{n}"));
                }
            }
            std::fs::create_dir_all(&dir).unwrap();
            for i in 0..1000 {
                let path = dir.join(format!("文件{i:04}.tmp"));
                std::fs::File::create(&path)
                    .unwrap()
                    .write_all(b"readonly-performance-fixture")
                    .unwrap();
                let time = match case.as_str() {
                    "recent" if i % 10 != 0 => recent,
                    "mixed" if i % 2 != 0 => recent,
                    _ => old,
                };
                filetime::set_file_mtime(&path, time).unwrap();
            }
        }
        return;
    }
    let mut resources = Resources::default();
    resources.sample();
    let mut cancel_ms = None;
    let mut first_progress_ms = None;
    let began = Instant::now();
    let output;
    if mode == "analysis" {
        let engine =
            AnalysisEngine::new(Arc::new(crate::WindowsAnalysis::for_directory(&cache))).unwrap();
        let token = engine.locations()[0].token.clone();
        engine.start(&token, vec![]).unwrap();
        while engine.is_busy() {
            resources.sample();
            std::thread::sleep(Duration::from_millis(5));
        }
        let elapsed_ms = began.elapsed().as_secs_f64() * 1000.0;
        resources.sample();
        let result = engine.snapshot().unwrap();
        assert_eq!(result.state, "completed");
        output = serde_json::json!({"elapsed_ms":elapsed_ms,"result":result,"resources":resources});
    } else {
        assert!(matches!(mode.as_str(), "scan" | "cancel"));
        let platform = Arc::new(Measured {
            platform: WindowsPlatform {
                inventory: Default::default(),
                tool_roots: Default::default(),
                tool_probe: std::sync::Arc::new(crate::tool_discovery::UnavailableProbe),
                real_app_acceptance: None,
                app_test_identity: None,
                app_test_acceptance: None,
                scope: Scope::User,
                local: extended(&base.join("local")).unwrap(),
                roaming: base.join("roaming"),
                system: base.join("system"),
            },
            candidates: Mutex::new(vec![]),
        });
        let journal = tempfile::NamedTempFile::new_in(&root).unwrap();
        let engine = Engine::open(platform.clone(), Scope::User, journal.path()).unwrap();
        if case == "excluded" {
            engine
                .save_settings(Settings {
                    theme: "system".into(),
                    exclusions: (0..450)
                        .map(|i| display_path(&cache.join(format!("排除{i:04}"))))
                        .chain((0..50).map(|i| display_path(&cache.join(format!("目录{i:03}")))))
                        .collect(),
                    ..Default::default()
                })
                .unwrap();
        }
        let started = Instant::now();
        let session = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
        let mut stopped = None;
        while engine.is_busy() {
            if engine.status().scan_progress.checked > 0 && first_progress_ms.is_none() {
                first_progress_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
            }
            if mode == "cancel"
                && stopped.is_none()
                && started.elapsed() >= Duration::from_millis(150)
            {
                stopped = Some(Instant::now());
                engine.cancel();
            }
            resources.sample();
            std::thread::sleep(Duration::from_millis(5));
            assert!(started.elapsed() < Duration::from_secs(600));
        }
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        if let Some(stop) = stopped {
            cancel_ms = Some(stop.elapsed().as_secs_f64() * 1000.0);
        }
        resources.sample();
        assert_eq!(
            engine.status().state,
            if mode == "cancel" {
                JobState::Cancelled
            } else {
                JobState::Ready
            }
        );
        assert!(engine.summary().rules.iter().all(|r| !r.scan_failed));
        let mut candidates = platform.candidates.lock().unwrap();
        candidates.sort();
        std::fs::write(
            root.join("candidates.json"),
            serde_json::to_vec(&*candidates).unwrap(),
        )
        .unwrap();
        let page_ms = if mode == "scan" {
            let start = Instant::now();
            let page = engine
                .candidates(&session, "windows.user_temp", 0, 200)
                .unwrap();
            assert!(page.items.len() <= 200);
            Some(start.elapsed().as_secs_f64() * 1000.0)
        } else {
            None
        };
        output = serde_json::json!({"elapsed_ms":elapsed_ms,"first_progress_ms":first_progress_ms,"cancel_ms":cancel_ms,"page_ms":page_ms,"summary":engine.summary(),"resources":resources});
    }
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .unwrap();
}
