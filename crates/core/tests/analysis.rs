use anyhow::Result;
use dustaway_core::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
struct Source {
    count: u64,
    slow: bool,
    fail: bool,
    issue: bool,
}

const RECOGNITION_FIXTURES: &[(&str, &str)] = &[
    ("model.gguf", "model.gguf"),
    ("main.py", "dev.source_python"),
    ("analysis.ipynb", "dev.jupyter_notebook"),
    ("uv.lock", "dev.uv_project"),
    ("types.d.ts", "dev.source_typescript"),
    ("main.go", "dev.source_go"),
    ("main.rs", "dev.source_rust"),
    ("model.weights.h5", "model.keras_weights"),
    ("experiment.h5", "data.hdf5"),
    ("system.tibx", "backup.acronis"),
    ("vm.vmem", "vm.vmware_state"),
    ("drawing.dwg", "media.cad_drawing"),
    ("photos.lrcat-data", "media.lightroom_catalog"),
    ("world.mcworld", "game.minecraft_world"),
    ("capture.pcapng", "data.packet_capture"),
    ("app.msixbundle", "installer.msix"),
    ("办公文稿.wps", "document.wps_text"),
    ("预算表.et", "document.wps_sheet"),
    ("演示文稿.dps", "document.wps_slides"),
    ("凭证.ofd", "document.ofd"),
    ("会议.wemtv.idx", "app.tencent_meeting_raw"),
    ("原型.mxgp", "media.mastergo"),
    ("场景.max", "media.max_scene"),
    ("纹理.spp", "media.substance_painter"),
    ("应用.hap", "dev.harmony_packages"),
    ("内存.hprof", "dev.java_heap"),
    (
        r"xwechat_files\account\db_storage\message\data.db",
        "app.weixin_database",
    ),
    (
        r"xwechat_files\account\msg\attach\image.dat",
        "app.weixin_attachments",
    ),
    (
        r"AppData\Roaming\LarkShell\aha\users\account\profile_main\Cache\entry",
        "app.feishu_web_cache",
    ),
    (
        r"AppData\Roaming\LarkShell\aha\users\account\profile_main\IndexedDB\entry",
        "app.feishu_profile",
    ),
    (
        r"AppData\Local\DingTalk_133\Default\Cache\entry",
        "app.dingtalk_web_cache",
    ),
    (
        r"Documents\WXWork\account\Cache\File\budget.xlsx",
        "app.wecom_received",
    ),
    (
        r"Documents\Tencent Files\account\Video\clip.mp4",
        "app.qq_data",
    ),
    (
        r"JianyingPro\User Data\Projects\com.lveditor.draft\demo\clip.mp4",
        "app.jianying_drafts",
    ),
    ("draft_content.json", "app.jianying_draft_file"),
];
struct RecognitionSource;
impl AnalysisPlatform for RecognitionSource {
    fn locations(&self) -> Vec<(String, String)> {
        vec![("识别样本".into(), r"D:\fixtures".into())]
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
        _: &AnalysisRoot,
        _: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        emit(AnalysisEvent::Entry(Box::new(entry(
            "0", None, true, 0, "",
        ))))?;
        for (name, _) in RECOGNITION_FIXTURES {
            let mut sample = entry(name, Some("0"), false, 8_000_000_000, name);
            sample.entry.name = (*name).into();
            sample.entry.path = format!(r"D:\fixtures\{name}");
            emit(AnalysisEvent::Entry(Box::new(sample)))?;
        }
        emit(AnalysisEvent::DirectoryFinished { node: "0".into() })?;
        Ok(())
    }
}
#[test]
fn offline_recognition_reaches_both_query_views_without_cleanup_rules() {
    let engine = AnalysisEngine::new(Arc::new(RecognitionSource)).unwrap();
    let session = start(&engine);
    assert_eq!(wait(&engine).state, "completed");
    let children = engine.children(&session, "0", "", 0, true).unwrap();
    let files = engine.files(&session, "", 0).unwrap();
    assert_eq!(children.items.len(), RECOGNITION_FIXTURES.len());
    assert_eq!(files.items.len(), RECOGNITION_FIXTURES.len());
    for item in children.items.iter().chain(&files.items) {
        let expected = RECOGNITION_FIXTURES
            .iter()
            .find(|(name, _)| *name == item.name)
            .unwrap()
            .1;
        assert!(item.rule_ids.is_empty());
        let advice = item.advice.as_ref().unwrap();
        assert_eq!(advice.kind, AnalysisPurpose::Unknown);
        assert_eq!(advice.recognition.as_ref().unwrap().rule_id, expected);
    }
    // The navigation node is intentionally undecorated; only result rows have advice.
    assert!(children.node.advice.is_none());
}
fn entry(id: &str, parent: Option<&str>, directory: bool, bytes: u64, key: &str) -> AnalysisRecord {
    AnalysisRecord {
        entry: AnalysisEntry {
            allocated_bytes: None,
            allocation_unknown: 0,
            verification_pending: false,
            manual: Default::default(),
            ancestors: vec![],
            id: id.into(),
            parent: parent.map(str::to_owned),
            name: format!("条目 {id}"),
            path: format!("C:\\fixture\\{id}"),
            directory,
            bytes: bytes.to_string(),
            files: if directory { 0 } else { 1 },
            modified_unix: 0,
            shared: !directory,
            incomplete: false,
            pending: false,
            issue: None,
            rule_ids: vec![],
            excluded: false,
            advice: None,
        },
        file_key: (!directory).then(|| key.into()),
        opaque: id.into(),
    }
}
impl AnalysisPlatform for Source {
    fn locations(&self) -> Vec<(String, String)> {
        vec![("当前用户文件夹".into(), "C:\\fixture".into())]
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        Ok(AnalysisRoot {
            path: path.into(),
            opaque: String::new(),
        })
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        if !emit(AnalysisEvent::Entry(Box::new(entry(
            "0", None, true, 0, "",
        ))))? {
            return Ok(());
        }
        for n in 1..=self.count {
            if cancel.load(Ordering::Acquire) {
                break;
            }
            if self.slow {
                std::thread::sleep(Duration::from_millis(1));
            }
            if !emit(AnalysisEvent::Checked)? {
                break;
            }
            if !emit(AnalysisEvent::Entry(Box::new(entry(
                &n.to_string(),
                Some("0"),
                false,
                u64::MAX / 8,
                &(n / 2).to_string(),
            ))))? {
                break;
            }
        }
        if self.issue {
            emit(AnalysisEvent::Issue {
                node: "0".into(),
                code: "directory_inaccessible".into(),
            })?;
        }
        if self.fail {
            anyhow::bail!("read error");
        }
        Ok(())
    }
    fn reveal(&self, _: &AnalysisRoot, record: &[Vec<u8>]) -> Result<()> {
        assert!(!record.is_empty());
        Ok(())
    }
}
fn setup(count: u64, slow: bool, fail: bool, issue: bool, limit: u64) -> Arc<AnalysisEngine> {
    AnalysisEngine::with_limit(
        Arc::new(Source {
            count,
            slow,
            fail,
            issue,
        }),
        limit,
    )
    .unwrap()
}
fn start(e: &Arc<AnalysisEngine>) -> String {
    e.start(&e.locations()[0].token, vec!["C:\\fixture\\2".into()])
        .unwrap()
}
fn wait(e: &AnalysisEngine) -> AnalysisSnapshot {
    let deadline = Instant::now() + Duration::from_secs(10);
    while e.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    e.snapshot().unwrap()
}
struct ChangingIdentity;
impl AnalysisPlatform for ChangingIdentity {
    fn locations(&self) -> Vec<(String, String)> {
        vec![("fixture".into(), "C:\\fixture".into())]
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
        _: &AnalysisRoot,
        _: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        emit(AnalysisEvent::Entry(Box::new(entry(
            "0", None, true, 0, "",
        ))))?;
        for (id, size) in [("1", 4096), ("2", 8192)] {
            let mut record = entry(id, Some("0"), false, size, "same-file");
            record.entry.allocated_bytes = Some(size.to_string());
            if !emit(AnalysisEvent::Entry(Box::new(record)))? {
                break;
            }
        }
        Ok(())
    }
}
#[test]
fn inconsistent_hardlink_observations_preserve_only_committed_statistics() {
    let engine = AnalysisEngine::new(Arc::new(ChangingIdentity)).unwrap();
    let session = start(&engine);
    let snapshot = wait(&engine);
    assert_eq!(snapshot.state, "partial");
    assert!(snapshot.issues.iter().any(|i| i.code == "changed"));
    assert_eq!(snapshot.files, 1);
    assert_eq!(snapshot.allocated_bytes, "4096");
    assert!(
        engine
            .children(&session, "0", "", 0, true)
            .unwrap()
            .node
            .incomplete
    );
}
#[test]
fn deduplicates_identity_preserves_large_numbers_and_exclusions_only_annotate() {
    let e = setup(3, false, false, false, 100);
    let session = start(&e);
    let s = wait(&e);
    assert_eq!(s.state, "completed");
    assert_eq!(s.logical_bytes, (3 * (u64::MAX / 8)).to_string());
    assert_eq!(s.unique_bytes, (2 * (u64::MAX / 8)).to_string());
    let page = e.children(&session, "0", "", 0, true).unwrap();
    assert_eq!(page.total, 3);
    assert!(page.items.iter().find(|e| e.id == "2").unwrap().excluded);
    assert_eq!(page.node.bytes, s.logical_bytes);
    assert_eq!(
        e.children(&session, "0", "条目 2", 0, true).unwrap().total,
        1
    );
    assert_eq!(e.children(&session, "0", "%", 0, true).unwrap().total, 0);
    assert!(e.reveal(&session, "invented").is_err());
    assert!(e.reveal("old", "1").is_err());
    e.mark_stale();
    assert!(e.snapshot().unwrap().stale);
    let next = start(&e);
    wait(&e);
    assert_ne!(session, next);
    assert!(e.children(&session, "0", "", 0, true).is_err());
}
#[test]
fn limit_failure_and_inaccessible_are_not_empty_successes() {
    let e = setup(6, false, false, false, 3);
    let id = start(&e);
    let s = wait(&e);
    assert_eq!(s.state, "partial");
    assert_eq!(s.entries, 3);
    assert_eq!(s.issues[0].code, "entry_limit");
    assert!(e.children(&id, "0", "", 0, true).unwrap().node.incomplete);
    let e = setup(0, false, true, false, 100);
    start(&e);
    assert_eq!(wait(&e).state, "failed");
    let e = setup(0, false, false, true, 100);
    let id = start(&e);
    assert_eq!(wait(&e).state, "partial");
    assert!(e.children(&id, "0", "", 0, true).unwrap().node.incomplete);
}
#[test]
fn cancellation_and_busy_are_bound_to_the_session() {
    let e = setup(6, true, false, false, 100);
    let id = start(&e);
    assert!(e.start(&e.locations()[0].token, vec![]).is_err());
    assert!(e.cancel("stale").is_err());
    e.cancel(&id).unwrap();
    assert_eq!(wait(&e).state, "cancelled");
    assert!(e.start("guessed-path", vec![]).is_err());
}

struct RankedSource;
impl AnalysisPlatform for RankedSource {
    fn locations(&self) -> Vec<(String, String)> {
        vec![("test".into(), "C:\\fixture".into())]
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        Ok(AnalysisRoot {
            path: path.into(),
            opaque: "root".into(),
        })
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        _: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        emit(AnalysisEvent::Entry(Box::new(entry(
            "0", None, true, 0, "",
        ))))?;
        emit(AnalysisEvent::Entry(Box::new(entry(
            "deep",
            Some("0"),
            true,
            0,
            "",
        ))))?;
        for i in 0..205 {
            let mut record = entry(
                &format!("file-{i:03}"),
                Some("deep"),
                false,
                i,
                &i.to_string(),
            );
            record.entry.name = "同名文件.txt".into();
            emit(AnalysisEvent::Entry(Box::new(record)))?;
        }
        let mut large = entry("large", Some("deep"), false, 9_007_199_254_740_993, "large");
        large.entry.name = "深层最大文件.txt".into();
        emit(AnalysisEvent::Entry(Box::new(large)))?;
        let mut unread = entry("unread", Some("0"), false, 0, "");
        unread.entry.issue = Some("directory_inaccessible".into());
        emit(AnalysisEvent::Entry(Box::new(unread)))?;
        Ok(())
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        Ok(())
    }
    fn context(&self, _: &str) -> Result<(String, u64)> {
        Ok(("C:\\".into(), 9_007_199_254_740_993))
    }
}
#[test]
fn largest_files_rank_all_depths_stably_and_exclude_unread_nodes() {
    let e = AnalysisEngine::new(Arc::new(RankedSource)).unwrap();
    let id = e
        .start(&e.locations()[0].token, vec!["C:\\fixture\\large".into()])
        .unwrap();
    assert_eq!(wait(&e).state, "partial");
    let first = e.files(&id, "", 0).unwrap();
    assert_eq!(first.total, 206);
    assert_eq!(first.items.len(), 200);
    assert_eq!(first.items[0].id, "large");
    assert_eq!(first.items[0].bytes, "9007199254740993");
    assert!(first.items[0].excluded);
    assert!(
        first
            .items
            .iter()
            .all(|e| e.shared && e.issue.is_none() && !e.directory)
    );
    let last = e.files(&id, "", 200).unwrap();
    assert_eq!(last.items.len(), 6);
    assert_eq!(last.items.last().unwrap().bytes, "0");
    assert!(
        last.items
            .iter()
            .all(|e| !first.items.iter().any(|f| f.id == e.id))
    );
    assert_eq!(e.files(&id, "同名文件", 200).unwrap().items.len(), 5);
    assert_eq!(
        e.files(&id, "深层", 0).unwrap().items[0].parent.as_deref(),
        Some("deep")
    );
    assert_eq!(e.files(&id, "%", 0).unwrap().total, 0);
    assert!(e.files(&id, &"x".repeat(257), 0).is_err());
    assert!(e.files("stale-session", "", 0).is_err());
    assert!(e.files(&id, "", ANALYSIS_LIMIT + 1).is_err());
    let children = e.children(&id, "0", "", 0, true).unwrap();
    assert_eq!(
        children
            .items
            .iter()
            .find(|e| e.id == "unread")
            .unwrap()
            .advice
            .as_ref()
            .unwrap()
            .kind,
        AnalysisPurpose::Unknown
    );
}
#[test]
fn disk_context_uses_registered_tokens_preserves_integer_and_failed_sample_is_unknown() {
    let e = AnalysisEngine::new(Arc::new(RankedSource)).unwrap();
    assert!(e.context("C:\\fixture").is_err());
    let context = e.context(&e.locations()[0].token).unwrap();
    assert_eq!(context.available_bytes.as_deref(), Some("9007199254740993"));
    assert!(context.error.is_none() && context.sampled_at > 0);
    assert!(e.snapshot().is_none());
    let e = setup(0, false, false, false, 100);
    let context = e.context(&e.locations()[0].token).unwrap();
    assert!(
        context.available_bytes.is_none() && context.volume.is_none() && context.error.is_some()
    );
}
#[test]
fn advice_is_optional_and_partial_or_cancelled_file_rankings_keep_the_real_scope() {
    let record = entry("x", Some("0"), false, 1, "x");
    let json = serde_json::to_string(&record.entry).unwrap();
    assert!(!json.contains("advice"));
    assert!(
        serde_json::from_str::<AnalysisEntry>(&json)
            .unwrap()
            .advice
            .is_none()
    );
    let e = setup(6, false, false, false, 3);
    let id = start(&e);
    assert_eq!(wait(&e).state, "partial");
    assert_eq!(e.files(&id, "", 0).unwrap().total, 2);
    let e = setup(6, true, false, false, 100);
    let id = start(&e);
    e.cancel(&id).unwrap();
    let s = wait(&e);
    assert_eq!(s.state, "cancelled");
    assert_eq!(e.files(&id, "", 0).unwrap().total, s.files);
}

struct GatedSource {
    ready: std::sync::mpsc::Sender<()>,
    resume: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}
impl AnalysisPlatform for GatedSource {
    fn locations(&self) -> Vec<(String, String)> {
        RankedSource.locations()
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        RankedSource.validate_root(path)
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        _: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        emit(AnalysisEvent::Entry(Box::new(entry(
            "0", None, true, 0, "",
        ))))?;
        emit(AnalysisEvent::Entry(Box::new(entry(
            "first",
            Some("0"),
            false,
            1,
            "first",
        ))))?;
        self.ready.send(())?;
        self.resume.lock().unwrap().recv()?;
        emit(AnalysisEvent::Entry(Box::new(entry(
            "largest",
            Some("0"),
            false,
            999,
            "largest",
        ))))?;
        Ok(())
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        Ok(())
    }
}
#[test]
fn in_progress_file_pages_remain_readable_before_bulk_index_finalization() {
    let (ready, received) = std::sync::mpsc::channel();
    let (resume, gate) = std::sync::mpsc::channel();
    let engine = AnalysisEngine::new(Arc::new(GatedSource {
        ready,
        resume: std::sync::Mutex::new(gate),
    }))
    .unwrap();
    let id = engine.start(&engine.locations()[0].token, vec![]).unwrap();
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(engine.is_busy());
    let publication_deadline = Instant::now() + Duration::from_secs(5);
    while engine.snapshot().unwrap().files == 0 {
        assert!(Instant::now() < publication_deadline);
        std::thread::yield_now();
    }
    let page = engine.files(&id, "", 0).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, "first");
    resume.send(()).unwrap();
    assert_eq!(wait(&engine).state, "completed");
    let page = engine.files(&id, "", 0).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.items[0].id, "largest");
}

struct DenseSource {
    count: u64,
}
impl AnalysisPlatform for DenseSource {
    fn locations(&self) -> Vec<(String, String)> {
        RankedSource.locations()
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        RankedSource.validate_root(path)
    }
    fn reveal(&self, _: &AnalysisRoot, records: &[Vec<u8>]) -> Result<()> {
        assert_eq!(records.first().unwrap(), b"0");
        Ok(())
    }
    fn advice(&self, _: &AnalysisEntry) -> AnalysisAdvice {
        AnalysisAdvice {
            kind: AnalysisPurpose::SystemData,
            app_id: None,
            basis: "known_windows".into(),
            recognition: None,
        }
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        if !emit(AnalysisEvent::Entry(Box::new(entry(
            "0", None, true, 0, "",
        ))))? {
            return Ok(());
        }
        emit(AnalysisEvent::DirectoryStarted {
            node: "0".into(),
            path: "C:\\fixture".into(),
        })?;
        if !emit(AnalysisEvent::Entry(Box::new(entry(
            "dir",
            Some("0"),
            true,
            0,
            "",
        ))))? {
            return Ok(());
        }
        emit(AnalysisEvent::DirectoryStarted {
            node: "dir".into(),
            path: "C:\\fixture\\dir".into(),
        })?;
        for n in 0..self.count {
            if cancel.load(Ordering::Acquire) {
                return Ok(());
            }
            if !emit(AnalysisEvent::Entry(Box::new(entry(
                &format!("f{n}"),
                Some("dir"),
                false,
                1,
                &(n / 2).to_string(),
            ))))? {
                return Ok(());
            }
        }
        emit(AnalysisEvent::DirectoryFinished { node: "dir".into() })?;
        emit(AnalysisEvent::DirectoryFinished { node: "0".into() })?;
        Ok(())
    }
}
#[test]
fn resource_limit_preserves_atomic_counts_pages_and_trusted_purpose() {
    let e = AnalysisEngine::with_resource_limits(
        Arc::new(DenseSource { count: 10000 }),
        500000,
        128 * 1024,
    )
    .unwrap();
    let id = start(&e);
    let s = wait(&e);
    assert_eq!(s.state, "partial");
    assert!(s.issues.iter().any(|i| i.code == "resource_limit"));
    assert!(s.entries > 2 && s.entries < 10002);
    assert_eq!(s.logical_bytes, s.files.to_string());
    assert_eq!(s.unique_bytes, s.files.div_ceil(2).to_string());
    assert_eq!(e.files(&id, "", 0).unwrap().total, s.files);
    let p = e.children(&id, "0", "", 0, true).unwrap();
    assert_eq!(p.items[0].bytes, s.logical_bytes);
    assert_eq!(p.coverage.incomplete, 1);
    assert_eq!(p.coverage.pending, 0);
    assert_eq!(
        p.items[0].advice.as_ref().unwrap().kind,
        AnalysisPurpose::SystemData
    );
    assert!(p.generated_at > 0);
    e.reveal(&id, "f0").unwrap();
}
#[test]
fn indexing_can_be_observed_and_cancelled_without_losing_partial_queries() {
    let e = AnalysisEngine::new(Arc::new(DenseSource { count: 100000 })).unwrap();
    let id = start(&e);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let s = e.snapshot().unwrap();
        if s.phase == "indexing" {
            let begin = Instant::now();
            e.cancel(&id).unwrap();
            assert_eq!(wait(&e).state, "cancelled");
            assert!(begin.elapsed() < Duration::from_secs(2));
            assert_eq!(e.files(&id, "", 0).unwrap().total, 100000);
            break;
        }
        assert_eq!(s.state, "scanning", "indexing must be observable");
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

struct WideSource;
impl AnalysisPlatform for WideSource {
    fn locations(&self) -> Vec<(String, String)> {
        RankedSource.locations()
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        RankedSource.validate_root(path)
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        Ok(())
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        _: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        emit(AnalysisEvent::Entry(Box::new(entry(
            "0", None, true, 0, "",
        ))))?;
        for n in 0..10000 {
            let dir = format!("d{n}");
            if !emit(AnalysisEvent::Entry(Box::new(entry(
                &dir,
                Some("0"),
                true,
                0,
                "",
            ))))? {
                return Ok(());
            }
            emit(AnalysisEvent::DirectoryStarted {
                node: dir.clone(),
                path: dir.clone(),
            })?;
            if !emit(AnalysisEvent::Entry(Box::new(entry(
                &format!("f{n}"),
                Some(&dir),
                false,
                1,
                &n.to_string(),
            ))))? {
                return Ok(());
            }
            emit(AnalysisEvent::DirectoryFinished { node: dir })?;
        }
        Ok(())
    }
}
#[test]
fn resource_limited_wide_directory_pages_remain_readable_and_sorted() {
    for budget in [64, 128, 256] {
        let e = AnalysisEngine::with_resource_limits(Arc::new(WideSource), 500000, budget * 1024)
            .unwrap();
        let id = start(&e);
        let s = wait(&e);
        assert_eq!(s.state, "partial");
        assert!(s.issues.iter().any(|i| i.code == "resource_limit"));
        let p = e.children(&id, "0", "", 0, true).unwrap();
        assert!(p.total > 0);
        assert_eq!(p.items[0].bytes, "1");
        assert_eq!(e.files(&id, "", 0).unwrap().total, s.files);
        assert_eq!(
            p.coverage.completed + p.coverage.incomplete + p.coverage.pending,
            p.total
        );
    }
}
