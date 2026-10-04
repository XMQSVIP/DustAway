//! Read-only analysis has its own index and identifiers, never cleanup candidates.
use crate::{ApiError, now};
use anyhow::{Result, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub const ANALYSIS_LIMIT: u64 = 5_000_000;
pub const ANALYSIS_DEPTH: usize = 128;
pub const ANALYSIS_INDEX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisLocation {
    pub token: String,
    pub title: String,
    pub path: String,
    #[serde(flatten)]
    pub info: AnalysisLocationInfo,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisLocationInfo {
    pub kind: String,
    pub is_system: bool,
    pub supported: bool,
    pub unavailable_reason: Option<String>,
    #[serde(skip)]
    pub expected_volume: Option<String>,
}
impl Default for AnalysisLocationInfo {
    fn default() -> Self {
        Self {
            kind: "directory".into(),
            is_system: false,
            supported: true,
            unavailable_reason: None,
            expected_volume: None,
        }
    }
}
#[derive(Debug, Clone)]
pub struct AnalysisRoot {
    pub path: String,
    pub opaque: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisPurpose {
    Cache,
    BrowserData,
    AppData,
    SystemData,
    Personal,
    Unknown,
    Incomplete,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisAdvice {
    pub kind: AnalysisPurpose,
    pub app_id: Option<String>,
    pub basis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recognition: Option<crate::recognition::Recognition>,
}
impl Default for AnalysisAdvice {
    fn default() -> Self {
        Self {
            kind: AnalysisPurpose::Unknown,
            app_id: None,
            basis: "unrecognized_location".into(),
            recognition: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisContext {
    pub location_token: String,
    pub volume: Option<String>,
    pub available_bytes: Option<String>,
    pub sampled_at: i64,
    pub error: Option<String>,
    pub volume_key: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisEntry {
    #[serde(default)]
    pub allocated_bytes: Option<String>,
    #[serde(default)]
    pub allocation_unknown: u64,
    #[serde(default)]
    pub verification_pending: bool,
    #[serde(default)]
    pub manual: crate::ManualCapability,
    #[serde(default)]
    pub ancestors: Vec<String>,
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub path: String,
    pub directory: bool,
    pub bytes: String,
    pub files: u64,
    pub modified_unix: i64,
    pub shared: bool,
    pub incomplete: bool,
    #[serde(default)]
    pub pending: bool,
    pub issue: Option<String>,
    pub rule_ids: Vec<String>,
    pub excluded: bool,
    /// Query-time explanation, never persisted in the index or a cleanup plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advice: Option<Box<AnalysisAdvice>>,
}
#[derive(Debug, Clone)]
pub struct AnalysisRecord {
    pub entry: AnalysisEntry,
    pub file_key: Option<String>,
    pub opaque: Vec<u8>,
}
pub enum AnalysisEvent {
    Entry(Box<AnalysisRecord>),
    Checked,
    Issue { node: String, code: String },
    DirectoryStarted { node: String, path: String },
    DirectoryFinished { node: String },
}
pub trait AnalysisPlatform: Send + Sync + 'static {
    fn ignore_analysis_cache(&self, _path: &str) -> Result<()> {
        Ok(())
    }
    fn manual_capability(&self, _entry: &AnalysisEntry) -> crate::ManualCapability {
        crate::ManualCapability {
            allowed: false,
            reason: Some(crate::ReasonCode::Unverifiable),
        }
    }
    fn manual_record_capability(
        &self,
        entry: &AnalysisEntry,
        _opaque: &[u8],
    ) -> crate::ManualCapability {
        self.manual_capability(entry)
    }
    fn manual_record(
        &self,
        _root: &AnalysisRoot,
        _records: &[Vec<u8>],
    ) -> Result<crate::ManualRecord> {
        anyhow::bail!(crate::Refusal(crate::ReasonCode::Unverifiable))
    }
    fn delete_manual(&self, _record: &crate::ManualRecord, _exclusions: &[String]) -> Result<()> {
        anyhow::bail!(crate::Refusal(crate::ReasonCode::Unverifiable))
    }
    fn locations(&self) -> Vec<(String, String)>;
    fn location_info(&self, _path: &str) -> AnalysisLocationInfo {
        AnalysisLocationInfo::default()
    }
    fn volume_key(&self, _path: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot>;
    fn scan(
        &self,
        root: &AnalysisRoot,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()>;
    fn reveal(&self, root: &AnalysisRoot, records: &[Vec<u8>]) -> Result<()>;
    fn refine(&self, _root: &AnalysisRoot, _records: &[Vec<u8>]) -> Result<String> {
        anyhow::bail!("此位置不能单独分析，请使用目录选择器")
    }
    fn advice(&self, _entry: &AnalysisEntry) -> AnalysisAdvice {
        AnalysisAdvice::default()
    }
    fn context(&self, _path: &str) -> Result<(String, u64)> {
        anyhow::bail!("磁盘信息不可用")
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisIssue {
    pub code: String,
    pub count: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSnapshot {
    pub id: String,
    #[serde(default)]
    pub query_api_version: u8,
    #[serde(default)]
    pub scan_ms: u64,
    #[serde(default)]
    pub write_ms: u64,
    #[serde(default)]
    pub index_ms: u64,
    pub location: AnalysisLocation,
    pub state: String,
    pub started_at: i64,
    pub elapsed_ms: u64,
    pub checked: u64,
    pub entries: u64,
    pub files: u64,
    pub logical_bytes: String,
    pub unique_bytes: String,
    #[serde(default)]
    pub allocated_bytes: String,
    #[serde(default)]
    pub allocation_unknown: u64,
    pub issues: Vec<AnalysisIssue>,
    pub stale: bool,
    pub error: Option<String>,
    pub phase: String,
    pub current_directory: Option<String>,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AnalysisCoverage {
    pub completed: u64,
    pub incomplete: u64,
    pub pending: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisPage {
    pub generated_at: i64,
    pub coverage: AnalysisCoverage,
    pub node: AnalysisEntry,
    pub breadcrumbs: Vec<AnalysisEntry>,
    pub items: Vec<AnalysisEntry>,
    pub total: u64,
    pub offset: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisFilesPage {
    pub generated_at: i64,
    pub items: Vec<AnalysisEntry>,
    pub total: u64,
    pub offset: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryKind {
    Directory,
    File,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisDiscoveryQuery {
    pub kind: DiscoveryKind,
    /// all, recognized, unclassified, or a recognition category key.
    pub category: String,
    pub min_bytes: String,
    pub search: String,
    pub include_nested: bool,
    pub incomplete_only: bool,
    pub offset: u64,
}
#[derive(Default, Clone)]
struct Aggregate {
    bytes: u64,
    files: u64,
    shared: bool,
    incomplete: bool,
    started: bool,
    finished: bool,
    dirty: std::cell::Cell<bool>,
    recognition: u32,
    allocated: u64,
    allocation_unknown: u64,
}
struct Inner {
    db: Connection,
    storage: Option<Arc<crate::analysis_storage::SessionStorage>>,
    locations: HashMap<String, AnalysisLocation>,
    root: Option<AnalysisRoot>,
    snapshot: Option<AnalysisSnapshot>,
    started: Option<Instant>,
    parents: HashMap<String, Option<String>>,
    totals: HashMap<String, Aggregate>,
    exclusions: Vec<String>,
}
pub struct AnalysisEngine {
    reader: Mutex<Option<ReaderDescriptor>>,
    query_cancelled: Mutex<Option<(String, Arc<AtomicBool>)>>,
    query_pending: Mutex<std::collections::VecDeque<String>>,
    query_gate: Mutex<()>,
    count_gate: Mutex<()>,
    count_result: Mutex<Option<AnalysisCount>>,
    count_cache: Mutex<Option<query::CachedCount>>,
    storage_config: Mutex<Option<crate::AnalysisStorage>>,
    platform: Arc<dyn AnalysisPlatform>,
    inner: Mutex<Inner>,
    cancelled: Arc<AtomicBool>,
    index_bytes: u64,
    indexing_snapshot: Mutex<Option<AnalysisSnapshot>>,
    limit: u64,
    current_session: Mutex<Option<String>>,
    preview_cancelled: Mutex<Option<(String, Arc<AtomicBool>)>>,
    preparation: Mutex<Option<(String, Arc<AtomicBool>)>>,
    preparation_pending: Mutex<std::collections::HashSet<String>>,
}
// Cancellation only applies during a live task. Finished partial results remain queryable.
struct SqlCancellation<'a>(&'a Connection);
impl<'a> SqlCancellation<'a> {
    fn new(db: &'a Connection, cancel: Arc<AtomicBool>, enabled: bool) -> Result<Self> {
        db.progress_handler(
            1000,
            Some(move || enabled && cancel.load(Ordering::Acquire)),
        )?;
        Ok(Self(db))
    }
}
impl Drop for SqlCancellation<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}
fn resource_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<rusqlite::Error>().is_some_and(|e| {
        matches!(
            e.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DiskFull | rusqlite::ErrorCode::OutOfMemory)
        )
    })
}
fn database(budget: u64) -> Result<Connection> {
    schema(Connection::open_in_memory()?, budget)
}
fn schema(db: Connection, budget: u64) -> Result<Connection> {
    db.execute_batch("PRAGMA temp_store=MEMORY; PRAGMA page_size=4096;")?;
    db.pragma_update(
        None,
        "max_page_count",
        (budget / 4096).clamp(16, 0xffff_fffe) as i64,
    )?;
    db.execute_batch("CREATE TABLE nodes(id TEXT PRIMARY KEY,parent TEXT,name TEXT,path TEXT,directory INTEGER,size_key TEXT,files INTEGER,modified INTEGER,shared INTEGER,incomplete INTEGER,pending INTEGER,issue TEXT,rules TEXT,excluded INTEGER,opaque BLOB,rankable INTEGER,recognition INTEGER,category INTEGER,nested INTEGER,file_key TEXT,allocation_key TEXT,allocation_unknown INTEGER DEFAULT 0,verification_pending INTEGER DEFAULT 0,started INTEGER DEFAULT 0,finished INTEGER DEFAULT 0,logical_inverse TEXT,allocation_inverse TEXT);
        CREATE INDEX children ON nodes(parent,size_key DESC,name,id);
        CREATE INDEX manual_children ON nodes(parent,id); CREATE INDEX file_identity ON nodes(file_key) WHERE file_key IS NOT NULL;
        CREATE TABLE identities(id TEXT PRIMARY KEY, first_node TEXT, size_key TEXT, allocation_key TEXT) WITHOUT ROWID; CREATE TABLE allocation_seen(identity TEXT, directory TEXT, PRIMARY KEY(identity,directory)) WITHOUT ROWID; CREATE TABLE publication(id INTEGER PRIMARY KEY,snapshot TEXT,revision INTEGER);")?;
    Ok(db)
}
#[path = "analysis_index.rs"]
mod index;
use index::*;
#[path = "analysis_query.rs"]
mod query;
#[cfg(feature = "analysis-query-timing")]
pub use query::take_analysis_query_timings;
pub use query::{AnalysisCount, AnalysisReadPage, AnalysisReadQuery};
use query::{ReaderDescriptor, persist_snapshot};

impl AnalysisEngine {
    pub fn with_storage(
        platform: Arc<dyn AnalysisPlatform>,
        storage: crate::AnalysisStorage,
    ) -> Result<Arc<Self>> {
        Self::with_storage_limit(platform, storage, ANALYSIS_LIMIT)
    }
    pub fn with_storage_limit(
        platform: Arc<dyn AnalysisPlatform>,
        storage: crate::AnalysisStorage,
        limit: u64,
    ) -> Result<Arc<Self>> {
        storage.initialize()?;
        platform.ignore_analysis_cache(&storage.directory.to_string_lossy())?;
        let engine = Self::with_limit(platform, limit)?;
        *engine.storage_config.lock().unwrap() = Some(storage);
        Ok(engine)
    }
    pub fn storage_location(&self) -> Option<String> {
        self.storage_config
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.directory.to_string_lossy().into_owned())
    }
    pub fn configure_storage(&self, storage: crate::AnalysisStorage) -> Result<()> {
        ensure!(!self.is_busy(), "分析尚未停止");
        storage.initialize()?;
        self.platform
            .ignore_analysis_cache(&storage.directory.to_string_lossy())?;
        self.close_session()?;
        *self.storage_config.lock().unwrap() = Some(storage);
        Ok(())
    }
    pub fn close_session(&self) -> Result<()> {
        ensure!(!self.is_busy(), "分析尚未停止");
        if let Some((_, cancel)) = &*self.query_cancelled.lock().unwrap() {
            cancel.store(true, Ordering::Release);
        }
        let _page = self.query_gate.lock().unwrap();
        let _count = self.count_gate.lock().unwrap();
        let mut inner = self.inner.lock().unwrap();
        inner.db = database(self.index_bytes)?;
        self.reader.lock().unwrap().take();
        self.count_cache.lock().unwrap().take();
        inner.storage = None;
        inner.snapshot = None;
        inner.parents.clear();
        inner.totals.clear();
        Ok(())
    }

    pub fn new(platform: Arc<dyn AnalysisPlatform>) -> Result<Arc<Self>> {
        Self::with_limit(platform, ANALYSIS_LIMIT)
    }
    pub fn with_limit(platform: Arc<dyn AnalysisPlatform>, limit: u64) -> Result<Arc<Self>> {
        Self::with_resource_limits(platform, limit, ANALYSIS_INDEX_BYTES)
    }
    pub fn with_resource_limits(
        platform: Arc<dyn AnalysisPlatform>,
        limit: u64,
        index_bytes: u64,
    ) -> Result<Arc<Self>> {
        let locations = platform
            .locations()
            .into_iter()
            .map(|(title, path)| {
                let token = uuid::Uuid::new_v4().to_string();
                let mut info = platform.location_info(&path);
                info.expected_volume = platform.volume_key(&path).ok().flatten();
                if info.kind == "volume" && info.supported && info.expected_volume.is_none() {
                    info.supported = false;
                    info.unavailable_reason = Some("无法确认磁盘身份，请重新启动后选择位置".into());
                }
                (
                    token.clone(),
                    AnalysisLocation {
                        token,
                        title,
                        path,
                        info,
                    },
                )
            })
            .collect();
        Ok(Arc::new(Self {
            reader: Mutex::new(None),
            query_cancelled: Mutex::new(None),
            query_gate: Mutex::new(()),
            count_gate: Mutex::new(()),
            count_result: Mutex::new(None),
            count_cache: Mutex::new(None),
            storage_config: Mutex::new(None),
            platform,
            inner: Mutex::new(Inner {
                db: database(index_bytes)?,
                storage: None,
                locations,
                root: None,
                snapshot: None,
                started: None,
                parents: HashMap::new(),
                totals: HashMap::new(),
                exclusions: vec![],
            }),
            cancelled: Arc::new(AtomicBool::new(false)),
            index_bytes,
            indexing_snapshot: Mutex::new(None),
            limit,
            current_session: Mutex::new(None),
            preview_cancelled: Mutex::new(None),
            preparation: Mutex::new(None),
            preparation_pending: Mutex::new(Default::default()),
            query_pending: Mutex::new(Default::default()),
        }))
    }
    pub fn locations(&self) -> Vec<AnalysisLocation> {
        let mut list: Vec<_> = self
            .inner
            .lock()
            .unwrap()
            .locations
            .values()
            .cloned()
            .collect();
        list.sort_by(|a, b| {
            b.info
                .is_system
                .cmp(&a.info.is_system)
                .then_with(|| (b.info.kind == "volume").cmp(&(a.info.kind == "volume")))
                .then_with(|| a.title.cmp(&b.title))
        });
        list
    }
    fn validate_location(&self, location: &AnalysisLocation) -> Result<AnalysisRoot> {
        ensure!(
            location.info.supported,
            ApiError::new(
                "unsupported_volume",
                location
                    .info
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("此位置不可用")
            )
        );
        let root = self.platform.validate_root(&location.path)?;
        if let Some(expected) = &location.info.expected_volume {
            ensure!(
                self.platform.volume_key(&location.path)?.as_ref() == Some(expected),
                ApiError::new(
                    "location_changed",
                    "磁盘已断开或身份变化，请重新启动后选择位置"
                )
            );
        }
        Ok(root)
    }
    /// Resolve only registered volume tokens. Directory tokens and client paths cannot
    /// establish a garbage-scan scope; expected identities are checked on every request.
    pub fn scan_volumes(&self, tokens: &[String]) -> Result<Vec<crate::VolumeTarget>> {
        ensure!(
            !tokens.is_empty() && tokens.len() <= 26,
            "请选择至少一个磁盘"
        );
        let locations = self.locations();
        let mut result: Vec<crate::VolumeTarget> = Vec::new();
        for token in tokens {
            let location = locations
                .iter()
                .find(|l| &l.token == token)
                .ok_or_else(|| ApiError::new("invalid_location", "磁盘选择已失效，请重新选择"))?;
            ensure!(
                location.info.kind == "volume",
                ApiError::new("invalid_location", "请选择磁盘而不是文件夹")
            );
            self.validate_location(location)?;
            let key = location
                .info
                .expected_volume
                .clone()
                .ok_or_else(|| ApiError::new("location_changed", "无法确认磁盘身份"))?;
            ensure!(!result.iter().any(|v| v.key == key), "磁盘选择重复");
            result.push(crate::VolumeTarget {
                key,
                label: location.path.clone(),
                opaque: String::new(),
            });
        }
        Ok(result)
    }
    pub fn context(&self, token: &str) -> Result<AnalysisContext> {
        let location = self
            .inner
            .lock()
            .unwrap()
            .locations
            .get(token)
            .cloned()
            .ok_or_else(|| ApiError::new("invalid_location", "目录选择已失效，请重新选择"))?;
        // Sampling never holds the index lock or starts a scan.
        let result = (|| -> Result<_> {
            self.validate_location(&location)?;
            let key = self.platform.volume_key(&location.path)?;
            let value = self.platform.context(&location.path)?;
            ensure!(
                self.platform.volume_key(&location.path)? == key,
                "磁盘身份变化"
            );
            Ok((value, key))
        })();
        let (volume, available_bytes, volume_key, error) = match result {
            Ok(((volume, bytes), key)) => (Some(volume), Some(bytes.to_string()), key, None),
            Err(e) => (None, None, None, Some(format!("无法查询当前可用空间：{e}"))),
        };
        Ok(AnalysisContext {
            location_token: token.into(),
            volume,
            available_bytes,
            sampled_at: now(),
            error,
            volume_key,
        })
    }
    /// Only called with an OS picker result, never an IPC-supplied path.
    pub fn register(&self, path: &str) -> Result<AnalysisLocation> {
        let root = self.platform.validate_root(path)?;
        let mut inner = self.inner.lock().unwrap();
        if let Some(location) = inner.locations.values().find(|l| l.path == root.path) {
            return Ok(location.clone());
        }
        ensure!(
            inner.locations.len() < 32,
            ApiError::new("location_limit", "本次已选择较多目录，请重新启动后再选择")
        );
        let location = AnalysisLocation {
            token: uuid::Uuid::new_v4().to_string(),
            title: "自选目录".into(),
            path: root.path,
            info: AnalysisLocationInfo {
                expected_volume: self.platform.volume_key(path)?,
                ..self.platform.location_info(path)
            },
        };
        inner
            .locations
            .insert(location.token.clone(), location.clone());
        Ok(location)
    }
    pub fn is_busy(&self) -> bool {
        self.snapshot().is_some_and(|s| s.state == "scanning")
    }
    pub fn snapshot(&self) -> Option<AnalysisSnapshot> {
        loop {
            if let Some(mut s) = self.indexing_snapshot.lock().unwrap().clone() {
                s.elapsed_ms = ((now() - s.started_at).max(0) as u64) * 1000;
                return Some(s);
            }
            // Indexing can begin after the check above. Do not block on the DB
            // mutex until it finishes: keep checking its independently readable
            // snapshot so progress and cancellation stay available.
            match self.inner.try_lock() {
                Ok(inner) => {
                    let mut s = inner.snapshot.clone()?;
                    if s.state == "scanning" {
                        s.elapsed_ms = inner.started.map_or(0, |t| t.elapsed().as_millis() as u64);
                    }
                    return Some(s);
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(error) => panic!("analysis state mutex poisoned: {error}"),
            }
        }
    }
    pub fn start(self: &Arc<Self>, token: &str, exclusions: Vec<String>) -> Result<String> {
        if let Some((_, flag)) = &*self.query_cancelled.lock().unwrap() {
            flag.store(true, Ordering::Release);
        }
        let _page = self.query_gate.lock().unwrap();
        let _count = self.count_gate.lock().unwrap();
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            !inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state == "scanning"),
            ApiError::new("busy", "已有空间分析任务")
        );
        let location = inner
            .locations
            .get(token)
            .cloned()
            .ok_or_else(|| ApiError::new("invalid_location", "目录选择已失效，请重新选择"))?;
        let root = self.validate_location(&location)?;
        let id = uuid::Uuid::new_v4().to_string();
        let config = self.storage_config.lock().unwrap().clone();
        let (db, storage) = if let Some(config) = &config {
            let free = self
                .platform
                .context(&config.directory.to_string_lossy())?
                .1;
            ensure!(
                free > config.reserve + 64 * 1024 * 1024,
                ApiError::new("index_space", "临时索引位置空间不足，请选择其他本地磁盘")
            );
            let (storage, db) = crate::analysis_storage::SessionStorage::create(config, &id)?;
            (
                schema(db, config.budget.saturating_sub(64 * 1024 * 1024))?,
                Some(Arc::new(storage)),
            )
        } else {
            (database(self.index_bytes)?, None)
        };
        inner.db = db;
        self.count_cache.lock().unwrap().take();
        inner.storage = storage;
        inner.root = Some(root.clone());
        inner.parents.clear();
        inner.totals.clear();
        inner.exclusions = exclusions;
        inner.snapshot = Some(AnalysisSnapshot {
            query_api_version: if inner.storage.is_some() { 2 } else { 1 },
            scan_ms: 0,
            write_ms: 0,
            index_ms: 0,
            id: id.clone(),
            location,
            state: "scanning".into(),
            started_at: now(),
            elapsed_ms: 0,
            checked: 0,
            entries: 0,
            files: 0,
            logical_bytes: "0".into(),
            unique_bytes: "0".into(),
            allocated_bytes: "0".into(),
            allocation_unknown: 0,
            issues: vec![],
            stale: false,
            error: None,
            phase: "enumerating".into(),
            current_directory: None,
        });
        persist_snapshot(&inner)?;
        *self.reader.lock().unwrap() = inner.storage.as_ref().map(|storage| ReaderDescriptor {
            storage: storage.clone(),
            root: root.clone(),
            session: id.clone(),
        });
        inner.started = Some(Instant::now());
        {
            let mut current = self.current_session.lock().unwrap();
            *current = Some(id.clone());
            self.cancelled.store(false, Ordering::Release);
        }
        drop(inner);
        let engine = self.clone();
        std::thread::spawn(move || {
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| engine.work(&root)));
            let mut inner = engine.inner.lock().unwrap_or_else(|e| e.into_inner());
            engine.inner.clear_poison();
            // Bulk-build the rank index once instead of maintaining a second sorted
            // tree on every scanned file. In-progress pages still query these same
            // records; SQLite sorts that partial result without reading the disk.
            // Keep the task busy until indexing really finishes, including on cancel.
            if let Some(s) = &mut inner.snapshot {
                s.phase = "indexing".into();
            }
            *engine.indexing_snapshot.lock().unwrap() = inner.snapshot.clone();
            drop(inner);
            let mut inner = engine.inner.lock().unwrap_or_else(|e| e.into_inner());
            let index_begin = Instant::now();
            let index_resource = Arc::new(AtomicBool::new(false));
            let indexed = if engine.cancelled.load(Ordering::Acquire) {
                Ok(())
            } else {
                (|| -> Result<()> {
                    let _cancel = SqlCancellation::new(&inner.db, engine.cancelled.clone(), true)?;
                    if let (Some(storage), Some(config)) = (
                        inner.storage.clone(),
                        engine.storage_config.lock().unwrap().clone(),
                    ) {
                        let platform = engine.platform.clone();
                        let flag = index_resource.clone();
                        let cancel = engine.cancelled.clone();
                        let mut checked = Instant::now() - std::time::Duration::from_secs(1);
                        inner.db.progress_handler(
                            1000,
                            Some(move || {
                                if cancel.load(Ordering::Acquire) {
                                    return true;
                                }
                                if checked.elapsed().as_millis() >= 250 {
                                    checked = Instant::now();
                                    let exhausted = storage.bytes() + 64 * 1024 * 1024
                                        >= config.budget
                                        || platform
                                            .context(&config.directory.to_string_lossy())
                                            .map_or(true, |(_, free)| {
                                                free <= config.reserve + 64 * 1024 * 1024
                                            });
                                    if exhausted {
                                        flag.store(true, Ordering::Release);
                                        return true;
                                    }
                                }
                                false
                            }),
                        )?;
                    }
                    inner.db.execute_batch("CREATE INDEX largest_files ON nodes(size_key DESC,name,id) WHERE rankable=1")?;
                    inner.db.execute_batch("CREATE INDEX discovery_files ON nodes(category,size_key DESC,name,id) WHERE rankable=1")?;
                    inner.db.execute_batch("CREATE INDEX largest_allocation ON nodes(allocation_key DESC,name,id) WHERE rankable=1; CREATE INDEX directory_ranking ON nodes(category,pending,size_key DESC,name,id,logical_inverse,allocation_key,allocation_inverse,nested,parent,recognition,incomplete,finished) WHERE directory=1; CREATE INDEX directory_allocation ON nodes(category,pending,allocation_key DESC,name,id) WHERE directory=1")?;
                    for column in [
                        "size_key",
                        "allocation_key",
                        "logical_inverse",
                        "allocation_inverse",
                    ] {
                        inner.db.execute_batch(&format!("CREATE INDEX page_{column} ON nodes(pending,{column},name,id); CREATE INDEX child_{column} ON nodes(parent,pending,{column},name,id);"))?;
                    }
                    Ok(())
                })()
            };
            let limited = index_resource.load(Ordering::Acquire)
                || indexed.as_ref().err().is_some_and(resource_error);
            if limited {
                let _ = Self::issue(&mut inner, "0", "resource_limit");
            }
            let elapsed = inner.started.unwrap().elapsed().as_millis() as u64;
            if let Some(s) = &mut inner.snapshot {
                s.elapsed_ms = elapsed;
                s.index_ms = index_begin.elapsed().as_millis() as u64;
                let error = if engine.cancelled.load(Ordering::Acquire) || limited {
                    None
                } else if indexed.is_err() {
                    Some("排行索引未完成，已保留部分统计，请重新分析")
                } else {
                    match outcome {
                        Ok(Ok(())) => None,
                        Ok(Err(_)) => Some("读取中断，已保留部分统计，请重新分析"),
                        Err(_) => Some("分析异常中断，已保留部分统计"),
                    }
                };
                s.state = if error.is_some() {
                    "failed"
                } else if engine.cancelled.load(Ordering::Acquire) {
                    "cancelled"
                } else if !s.issues.is_empty() {
                    "partial"
                } else {
                    "completed"
                }
                .into();
                s.error = error.map(str::to_owned);
                s.phase = "finished".into();
                s.current_directory = None;
            }
            if inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state == "completed")
            {
                let _ = inner.db.execute(
                    "UPDATE nodes SET pending=0,started=1,finished=1 WHERE directory=1",
                    [],
                );
            }
            let _ = flush(&mut inner);
            let _ = persist_snapshot(&inner);
            *engine.indexing_snapshot.lock().unwrap() = None;
        });
        Ok(id)
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let current = self.current_session.lock().unwrap();
        ensure!(
            current.as_deref() == Some(id),
            ApiError::new("invalid_session", "分析任务已变化")
        );
        self.cancelled.store(true, Ordering::Release);
        Ok(())
    }
    pub fn cancel_all(&self) {
        if let Some((_, cancel)) = &*self.query_cancelled.lock().unwrap() {
            cancel.store(true, Ordering::Release);
        }

        self.cancelled.store(true, Ordering::Release);
        for operation in [&self.preview_cancelled, &self.preparation] {
            if let Some((_, flag)) = &*operation.lock().unwrap() {
                flag.store(true, Ordering::Release);
            }
        }
    }
    pub fn mark_stale(&self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(s) = &mut inner.snapshot {
            s.stale = true;
        }
        let _ = persist_snapshot(&inner);
    }
    fn work(&self, root: &AnalysisRoot) -> Result<()> {
        self.work_batched(root)
    }
    fn insert(inner: &mut Inner, record: &AnalysisRecord) -> Result<()> {
        let e = &record.entry;
        let bytes = e.bytes.parse::<u64>()?;
        let rankable = !e.directory && e.issue.is_none();
        let classification = crate::recognition::classify(&e.path, e.directory);
        let recognition = classification.map_or(0, |c| c.rule);
        let category = classification.map_or(0, |c| c.category.code());
        let mut ancestor = e.parent.clone();
        let mut nested = false;
        if e.directory && recognition != 0 {
            while let Some(id) = ancestor {
                if aggregate(inner, &id)?.is_some_and(|t| t.recognition == recognition) {
                    nested = true;
                    break;
                }
                ancestor = parent_of(inner, &id)?;
            }
        }
        let mut updates = Vec::new();
        let mut parent = e.parent.clone();
        // Validate aggregate changes before committing a node or a deduplication identity.
        if rankable {
            while let Some(id) = parent {
                let total = aggregate(inner, &id)?
                    .ok_or_else(|| ApiError::new("invalid_analysis", "目录结构无效"))?;
                let bytes = total
                    .bytes
                    .checked_add(bytes)
                    .ok_or_else(|| ApiError::new("overflow", "大小超出统计范围"))?;
                updates.push((id.clone(), bytes));
                parent = parent_of(inner, &id)?;
            }
        }
        let tx = rusqlite::Savepoint::with_name(&mut inner.db, "analysis_item")?;
        let unique = if rankable {
            if let Some(key) = &record.file_key {
                tx.prepare_cached("INSERT OR IGNORE INTO identities VALUES(?1,?2,?3,?4)")?
                    .execute(params![
                        storage_key(key),
                        e.id,
                        format!("{bytes:020}"),
                        e.allocated_bytes
                            .as_ref()
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(|n| format!("{n:020}"))
                    ])?
                    == 1
            } else {
                false
            }
        } else {
            false
        };
        if rankable
            && !unique
            && let Some(key) = &record.file_key
        {
            let (old_size, old_allocation, old_modified): (String, Option<String>, i64) = tx.query_row(
                "SELECT identities.size_key,identities.allocation_key,nodes.modified FROM identities JOIN nodes ON nodes.id=identities.first_node WHERE identities.id=?1",
                [storage_key(key)], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            let allocation_changed = match (old_allocation, e.allocated_bytes.as_ref()) {
                (Some(old), Some(new)) => old.parse::<u64>()? != new.parse::<u64>()?,
                _ => false,
            };
            ensure!(
                old_size.parse::<u64>()? == bytes
                    && old_modified == e.modified_unix
                    && !allocation_changed,
                ApiError::new("analysis_changed", "同一文件在分析期间发生变化")
            );
        }
        let snapshot = inner.snapshot.as_ref().unwrap();
        let logical = snapshot
            .logical_bytes
            .parse::<u64>()?
            .checked_add(if rankable { bytes } else { 0 })
            .ok_or_else(|| ApiError::new("overflow", "大小超出统计范围"))?;
        let unique_bytes = snapshot
            .unique_bytes
            .parse::<u64>()?
            .checked_add(if unique { bytes } else { 0 })
            .ok_or_else(|| ApiError::new("overflow", "大小超出统计范围"))?;
        let compact_path = if !e.directory
            && let Some(parent) = &e.parent
        {
            let parent_path: Option<String> = tx
                .query_row("SELECT path FROM nodes WHERE id=?1", [parent], |r| r.get(0))
                .ok();
            parent_path
                .is_some_and(|p| format!("{}\\{}", p.trim_end_matches('\\'), e.name) == e.path)
        } else {
            false
        };
        tx.prepare_cached(
            "INSERT INTO nodes VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,0,0,?24,?25)",
        )?
        .execute(params![
            e.id,
            e.parent,
            e.name,
            if compact_path {""} else {&e.path},
            e.directory,
            format!("{bytes:020}"),
            i64::try_from(e.files)?,
            e.modified_unix,
            e.shared,
            e.incomplete,
            e.pending,
            e.issue,
            serde_json::to_string(&e.rule_ids)?,
            e.excluded,
            record.opaque,
            rankable,
            recognition,
            category,
            nested,
            record.file_key.as_ref().map(|k|storage_key(k)),
            e.allocated_bytes.as_ref().and_then(|s|s.parse::<u64>().ok()).map(|n|format!("{n:020}")).unwrap_or_else(||"99999999999999999999".into()),
            e.allocation_unknown as i64,e.verification_pending,
            format!("{:020}",u64::MAX-bytes),
            e.allocated_bytes.as_ref().and_then(|s|s.parse::<u64>().ok()).map(|n|format!("{:020}",u64::MAX-n)).unwrap_or_else(||"99999999999999999999".into())
        ])?;
        tx.commit()?;
        if e.directory {
            inner.parents.insert(e.id.clone(), e.parent.clone());
            inner.totals.insert(
                e.id.clone(),
                Aggregate {
                    recognition,
                    ..Aggregate::default()
                },
            );
        }
        for (id, bytes) in updates {
            let total = cached_total(inner, &id)?;
            total.bytes = bytes;
            total.dirty.set(true);
            total.files += 1;
            total.shared |= e.shared;
        }
        if rankable {
            allocate(inner, record, unique)?;
        }
        let s = inner.snapshot.as_mut().unwrap();
        s.entries += 1;
        if rankable {
            s.files += 1;
        }
        s.logical_bytes = logical.to_string();
        s.unique_bytes = unique_bytes.to_string();
        Ok(())
    }
    fn issue(inner: &mut Inner, node: &str, code: &str) -> Result<()> {
        let s = inner.snapshot.as_mut().unwrap();
        if let Some(issue) = s.issues.iter_mut().find(|i| i.code == code) {
            issue.count += 1;
        } else {
            s.issues.push(AnalysisIssue {
                code: code.into(),
                count: 1,
            });
        }
        let mut current = Some(node.to_owned());
        // Files are not in parents; read their parent once.
        if aggregate(inner, node)?.is_none() {
            current = inner
                .db
                .query_row("SELECT parent FROM nodes WHERE id=?1", [node], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .ok()
                .flatten();
        }
        while let Some(id) = current {
            if let Ok(t) = cached_total(inner, &id) {
                t.incomplete = true;
            }
            current = parent_of(inner, &id)?;
        }
        Ok(())
    }
    fn entry(inner: &Inner, id: &str) -> Result<AnalysisEntry> {
        let mut e = inner.db.prepare_cached(
            "SELECT id,parent,name,path,directory,size_key,files,modified,shared,incomplete,pending,issue,rules,excluded FROM nodes WHERE id=?1")?.query_row([id], |r| {
                let bytes: String = r.get(5)?;
                let rules: String = r.get(12)?;
                Ok(AnalysisEntry { allocated_bytes: None, allocation_unknown: 0, verification_pending: false, manual: Default::default(), ancestors: vec![], id:r.get(0)?,parent:r.get(1)?,name:r.get(2)?,path:r.get(3)?,directory:r.get(4)?,bytes:bytes.trim_start_matches('0').to_owned(),files:r.get::<_,i64>(6)? as u64,modified_unix:r.get(7)?,shared:r.get(8)?,incomplete:r.get(9)?,pending:r.get(10)?,issue:r.get(11)?,rule_ids:serde_json::from_str(&rules).map_err(|e| rusqlite::Error::FromSqlConversionFailure(12,rusqlite::types::Type::Text,Box::new(e)))?,excluded:r.get(13)?,advice:None })
            }).map_err(|_| ApiError::new("invalid_node", "目录或文件不属于本次分析"))?;
        Self::update_entry(inner, &mut e);
        Ok(e)
    }
    fn update_entry(inner: &Inner, e: &mut AnalysisEntry) {
        if e.path.is_empty()
            && let Some(parent) = &e.parent
            && let Ok(path) = inner
                .db
                .prepare_cached("SELECT path FROM nodes WHERE id=?1")
                .and_then(|mut statement| statement.query_row([parent], |r| r.get::<_, String>(0)))
        {
            e.path = format!("{}\\{}", path.trim_end_matches('\\'), e.name);
        }
        if e.bytes.is_empty() {
            e.bytes = "0".into();
        }
        if !e.directory
            && let Ok((allocated, unknown, pending, duplicates)) = inner.db.prepare_cached(
                "SELECT allocation_key,allocation_unknown,verification_pending,EXISTS(SELECT 1 FROM nodes peers WHERE peers.file_key=nodes.file_key LIMIT 1 OFFSET 1) FROM nodes WHERE id=?1").and_then(|mut statement| statement.query_row([&e.id],
                |r| Ok((r.get::<_,Option<String>>(0)?,r.get::<_,i64>(1)? as u64,r.get::<_,bool>(2)?,r.get::<_,bool>(3)?)))) {
                e.allocated_bytes = allocated.filter(|n|n!="99999999999999999999").map(|n| { let n=n.trim_start_matches('0'); if n.is_empty(){"0".into()}else{n.into()} });
                e.allocation_unknown=unknown; e.verification_pending=pending; e.shared |= duplicates;
            }
        if e.directory
            && let Some(t) = aggregate(inner, &e.id).ok().flatten()
        {
            e.bytes = t.bytes.to_string();
            e.allocated_bytes = Some(t.allocated.to_string());
            e.allocation_unknown = t.allocation_unknown;
            e.files = t.files;
            e.shared = t.shared;
            e.incomplete |= t.incomplete;
            if inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state != "completed")
            {
                e.pending = !t.started && !t.incomplete && e.issue.is_none();
                e.incomplete |= !t.finished;
            }
        }
    }
    fn described_entry(&self, inner: &Inner, id: &str) -> Result<AnalysisEntry> {
        let mut entry = Self::entry(inner, id)?;
        entry.manual = self.manual_capability(inner, &entry)?;
        let mut parent = entry.parent.clone();
        while let Some(id) = parent {
            entry.ancestors.push(id.clone());
            parent = parent_of(inner, &id)?;
        }
        let mut advice = self.platform.advice(&entry);
        advice.recognition = crate::recognition::recognize(&entry.path, entry.directory);
        entry.advice = Some(Box::new(advice));
        Ok(entry)
    }
    fn manual_capability(
        &self,
        inner: &Inner,
        entry: &AnalysisEntry,
    ) -> Result<crate::ManualCapability> {
        let opaque: Vec<u8> = inner
            .db
            .prepare_cached("SELECT opaque FROM nodes WHERE id=?1")?
            .query_row([&entry.id], |r| r.get(0))?;
        Ok(self.platform.manual_record_capability(entry, &opaque))
    }
    pub fn discovery(
        &self,
        session: &str,
        query: &AnalysisDiscoveryQuery,
    ) -> Result<AnalysisFilesPage> {
        let invalid = || ApiError::new("invalid_query", "用途筛选条件无效");
        ensure!(
            query.search.chars().count() <= 256
                && query.offset <= self.limit
                && !query.min_bytes.is_empty()
                && query.min_bytes.len() <= 20
                && query.min_bytes.bytes().all(|b| b.is_ascii_digit()),
            invalid()
        );
        let minimum = query.min_bytes.parse::<u64>().map_err(|_| invalid())?;
        ensure!(
            query.kind == DiscoveryKind::Directory || !query.incomplete_only,
            invalid()
        );
        // Only these static fragments enter SQL; user input remains bound parameters.
        let (category_clause, category) = match query.category.as_str() {
            "all" => ("1=1", 0),
            "recognized" => ("category>0", 0),
            "unclassified" => ("category=0", 0),
            key => (
                "category=?1",
                crate::recognition::Category::ALL
                    .iter()
                    .find(|c| c.key() == key)
                    .ok_or_else(invalid)?
                    .code(),
            ),
        };
        ensure!(
            !self.is_busy(),
            ApiError::new("analysis_busy", "结束或停止分析后可按用途查找")
        );
        let inner = self.inner.lock().unwrap();
        Self::check_session(&inner, session)?;
        ensure!(
            inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state != "scanning"),
            ApiError::new("analysis_busy", "结束或停止分析后可按用途查找")
        );
        let (total, ids) = if query.kind == DiscoveryKind::File {
            let predicate = format!(
                "rankable=1 AND {category_clause} AND size_key>=?2 AND instr(lower(name),lower(?3))>0 AND ?1>=0"
            );
            let size = format!("{minimum:020}");
            let total = inner.db.query_row(
                &format!("SELECT count(*) FROM nodes WHERE {predicate}"),
                params![category, size, query.search],
                |r| r.get::<_, i64>(0),
            )? as u64;
            let mut sql = inner.db.prepare(&format!("SELECT id FROM nodes WHERE {predicate} ORDER BY size_key DESC,name,id LIMIT 200 OFFSET ?4"))?;
            let ids = sql
                .query_map(
                    params![category, size, query.search, query.offset as i64],
                    |r| r.get::<_, String>(0),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (total, ids)
        } else {
            let predicate = format!(
                "directory=1 AND (parent IS NOT NULL OR recognition>0) AND {category_clause} AND (?2 OR nested=0) AND instr(lower(name),lower(?3))>0 AND size_key>=?4 AND (?5=0 OR incomplete=1 OR finished=0) AND ?1>=0"
            );
            let size = format!("{minimum:020}");
            let total = inner.db.query_row(
                &format!("SELECT count(*) FROM nodes WHERE {predicate}"),
                params![
                    category,
                    query.include_nested,
                    query.search,
                    size,
                    query.incomplete_only
                ],
                |r| r.get::<_, i64>(0),
            )? as u64;
            let mut sql = inner.db.prepare(&format!("SELECT id FROM nodes WHERE {predicate} ORDER BY pending,size_key DESC,name,id LIMIT 200 OFFSET ?6"))?;
            let ids = sql
                .query_map(
                    params![
                        category,
                        query.include_nested,
                        query.search,
                        size,
                        query.incomplete_only,
                        query.offset as i64
                    ],
                    |r| r.get::<_, String>(0),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (total, ids)
        };
        let items = ids
            .iter()
            .map(|id| self.described_entry(&inner, id))
            .collect::<Result<Vec<_>>>()?;
        Ok(AnalysisFilesPage {
            generated_at: now(),
            items,
            total,
            offset: query.offset,
        })
    }
    pub fn files(&self, session: &str, search: &str, offset: u64) -> Result<AnalysisFilesPage> {
        ensure!(
            search.chars().count() <= 256 && offset <= self.limit,
            ApiError::new("invalid_query", "查询范围无效")
        );
        let inner = self.inner.lock().unwrap();
        Self::check_session(&inner, session)?;
        let _cancel = SqlCancellation::new(
            &inner.db,
            self.cancelled.clone(),
            inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state == "scanning"),
        )?;
        let total = inner.db.query_row(
            "SELECT count(*) FROM nodes WHERE rankable=1 AND instr(lower(name),lower(?1))>0",
            [search],
            |r| r.get::<_, i64>(0),
        )? as u64;
        let mut query = inner.db.prepare("SELECT id FROM nodes WHERE rankable=1 AND instr(lower(name),lower(?1))>0 ORDER BY size_key DESC,name,id LIMIT 200 OFFSET ?2")?;
        let ids = query
            .query_map(params![search, offset as i64], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let items = ids
            .iter()
            .map(|id| self.described_entry(&inner, id))
            .collect::<Result<Vec<_>>>()?;
        Ok(AnalysisFilesPage {
            generated_at: now(),
            items,
            total,
            offset,
        })
    }
    pub fn children(
        &self,
        session: &str,
        node: &str,
        search: &str,
        offset: u64,
        descending: bool,
    ) -> Result<AnalysisPage> {
        ensure!(
            search.chars().count() <= 256 && offset <= self.limit,
            ApiError::new("invalid_query", "查询范围无效")
        );
        let inner = self.inner.lock().unwrap();
        Self::check_session(&inner, session)?;
        let current = Self::entry(&inner, node)?;
        ensure!(
            current.directory,
            ApiError::new("invalid_node", "请选择目录")
        );
        let _cancel = SqlCancellation::new(
            &inner.db,
            self.cancelled.clone(),
            inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state == "scanning"),
        )?;
        let coverage = inner.db.query_row("SELECT coalesce(sum(finished=1 AND incomplete=0),0),coalesce(sum(incomplete=1 OR (started=1 AND finished=0)),0),coalesce(sum(started=0 AND incomplete=0),0) FROM nodes WHERE parent=?1 AND directory=1",[node], |r| Ok(AnalysisCoverage { completed:r.get::<_,i64>(0)? as u64,incomplete:r.get::<_,i64>(1)? as u64,pending:r.get::<_,i64>(2)? as u64 }))?;
        let total = inner.db.query_row(
            "SELECT count(*) FROM nodes WHERE parent=?1 AND instr(lower(name),lower(?2))>0",
            params![node, search],
            |r| r.get::<_, i64>(0),
        )? as u64;
        let direction = if descending { "DESC" } else { "ASC" };
        let mut q=inner.db.prepare(&format!("SELECT id FROM nodes WHERE parent=?1 AND instr(lower(name),lower(?2))>0 ORDER BY size_key {direction},name,id LIMIT 200 OFFSET ?3"))?;
        let ids = q
            .query_map(params![node, search, offset as i64], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let items = ids
            .iter()
            .map(|id| self.described_entry(&inner, id))
            .collect::<Result<Vec<_>>>()?;
        let mut breadcrumbs = vec![current.clone()];
        let mut parent = current.parent.clone();
        while let Some(id) = parent {
            let e = Self::entry(&inner, &id)?;
            parent = e.parent.clone();
            breadcrumbs.push(e);
        }
        breadcrumbs.reverse();
        Ok(AnalysisPage {
            generated_at: now(),
            coverage,
            node: current,
            breadcrumbs,
            items,
            total,
            offset,
        })
    }
    fn check_session(inner: &Inner, session: &str) -> Result<()> {
        ensure!(
            inner.snapshot.as_ref().is_some_and(|s| s.id == session),
            ApiError::new("invalid_session", "空间分析会话已失效")
        );
        Ok(())
    }
    fn validation_chain(inner: &Inner, node: &str) -> Result<Vec<Vec<u8>>> {
        let mut current = Some(node.to_owned());
        let mut chain = Vec::new();
        while let Some(id) = current {
            ensure!(
                chain.len() <= ANALYSIS_DEPTH,
                ApiError::new("invalid_node", "目录链无效")
            );
            let (parent, opaque): (Option<String>, Vec<u8>) = inner
                .db
                .query_row("SELECT parent,opaque FROM nodes WHERE id=?1", [&id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .map_err(|_| ApiError::new("invalid_node", "文件不属于本次分析"))?;
            chain.push(opaque);
            current = parent;
        }
        chain.reverse();
        Ok(chain)
    }
    pub fn reveal(&self, session: &str, node: &str) -> Result<()> {
        let (root, records) = {
            let inner = self.inner.lock().unwrap();
            Self::check_session(&inner, session)?;
            (
                inner.root.clone().unwrap(),
                Self::validation_chain(&inner, node)?,
            )
        };
        self.platform.reveal(&root, &records)
    }
    /// Synchronous callers get a private token; IPC uses an identified preparation guard.
    pub fn prepare_manual(
        &self,
        selection: &crate::ManualSelection,
    ) -> Result<crate::ManualManifest> {
        self.prepare_manual_cancellable(selection, &AtomicBool::new(false))
    }
    pub fn manual_exclusion(&self, session: &str, node: &str) -> Result<String> {
        let inner = self.inner.lock().unwrap();
        Self::check_session(&inner, session)?;
        ensure!(
            inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state != "scanning" && !s.stale),
            "分析结果已变化"
        );
        let records = Self::validation_chain(&inner, node)?;
        Ok(self
            .platform
            .manual_record(inner.root.as_ref().unwrap(), &records)?
            .file
            .path_display)
    }
    pub fn refine(&self, session: &str, node: &str) -> Result<AnalysisLocation> {
        let (root, records) = {
            let inner = self.inner.lock().unwrap();
            Self::check_session(&inner, session)?;
            ensure!(
                Self::entry(&inner, node)?.directory,
                ApiError::new("invalid_node", "请选择目录")
            );
            (
                inner.root.clone().unwrap(),
                Self::validation_chain(&inner, node)?,
            )
        };
        let path = self.platform.refine(&root, &records)?;
        self.register(&path)
    }
}

#[cfg(test)]
mod sql_cancellation_tests {
    use super::*;
    struct UnusedPlatform;
    impl AnalysisPlatform for UnusedPlatform {
        fn locations(&self) -> Vec<(String, String)> {
            vec![]
        }
        fn validate_root(&self, _: &str) -> Result<AnalysisRoot> {
            unreachable!()
        }
        fn scan(
            &self,
            _: &AnalysisRoot,
            _: &AtomicBool,
            _: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
        ) -> Result<()> {
            unreachable!()
        }
        fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
            unreachable!()
        }
    }
    #[test]
    fn progress_can_switch_to_indexing_while_database_is_locked() {
        let engine = AnalysisEngine::new(Arc::new(UnusedPlatform)).unwrap();
        let guard = engine.inner.lock().unwrap();
        let other = engine.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let snapshot = other.snapshot();
            let query = AnalysisDiscoveryQuery {
                kind: DiscoveryKind::Directory,
                category: "all".into(),
                min_bytes: "0".into(),
                search: String::new(),
                include_nested: false,
                incomplete_only: false,
                offset: 0,
            };
            assert!(other.discovery("indexing", &query).is_err());
            tx.send(snapshot).unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        *engine.indexing_snapshot.lock().unwrap() = Some(AnalysisSnapshot {
            id: "indexing".into(),
            query_api_version: 1,
            scan_ms: 0,
            write_ms: 0,
            index_ms: 0,
            location: AnalysisLocation {
                token: "fixture".into(),
                title: "fixture".into(),
                path: "fixture".into(),
                info: AnalysisLocationInfo::default(),
            },
            state: "scanning".into(),
            phase: "indexing".into(),
            started_at: now(),
            elapsed_ms: 0,
            checked: 0,
            entries: 0,
            files: 0,
            logical_bytes: "0".into(),
            unique_bytes: "0".into(),
            allocated_bytes: "0".into(),
            allocation_unknown: 0,
            issues: vec![],
            stale: false,
            error: None,
            current_directory: None,
        });
        // This must return before the DB lock is released; otherwise progress
        // can remain blocked through the entire indexing operation.
        let result = rx.recv_timeout(std::time::Duration::from_secs(2));
        drop(guard);
        worker.join().unwrap();
        assert_eq!(result.unwrap().unwrap().phase, "indexing");
    }
    #[test]
    fn interrupts_running_sql_and_removes_the_handler_before_reading_partial_results() {
        let db = database(ANALYSIS_INDEX_BYTES).unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let other = flag.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            other.store(true, Ordering::Release);
        });
        let began = Instant::now();
        {
            let _guard = SqlCancellation::new(&db, flag.clone(), true).unwrap();
            let result=db.query_row("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<100000000) SELECT sum(x) FROM n", [], |r| r.get::<_,i64>(0));
            assert_eq!(
                result.unwrap_err().sqlite_error_code(),
                Some(rusqlite::ErrorCode::OperationInterrupted)
            );
        }
        worker.join().unwrap();
        assert!(began.elapsed() < std::time::Duration::from_secs(2));
        let _guard = SqlCancellation::new(&db, flag, false).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM nodes", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

#[cfg(test)]
mod manual_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    pub(super) struct Source {
        count: usize,
        partial: bool,
        benchmark: bool,
        prepared: AtomicUsize,
        hold: Mutex<Option<Arc<std::sync::Barrier>>>,
    }
    impl AnalysisPlatform for Source {
        fn locations(&self) -> Vec<(String, String)> {
            vec![("fixture".into(), "G:\\fixture".into())]
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
            for n in 0..=self.count + 1 {
                let dir = n < 2;
                let entry = serde_json::from_value(
                    serde_json::json!({"id":n.to_string(),"parent": if n==0 {None} else if n==1 {Some("0")} else {Some("1")},"name":n.to_string(),"path":format!("G:\\fixture\\{n}"),"directory":dir,"bytes":if dir {"0"} else if self.count > 10 {"1"} else {"9007199254740993"},"files":if dir {0} else {1},"modified_unix":0,"shared":false,"incomplete":false,"issue":null,"rule_ids":[],"excluded":false}),
                )?;
                if !emit(AnalysisEvent::Entry(Box::new(AnalysisRecord {
                    entry,
                    file_key: Some(n.to_string()),
                    opaque: n.to_string().into_bytes(),
                })))? {
                    break;
                }
            }
            if self.partial {
                emit(AnalysisEvent::Issue {
                    node: "1".into(),
                    code: "directory_inaccessible".into(),
                })?;
            }
            emit(AnalysisEvent::DirectoryFinished { node: "1".into() })?;
            emit(AnalysisEvent::DirectoryFinished { node: "0".into() })?;
            Ok(())
        }
        fn manual_capability(&self, entry: &AnalysisEntry) -> crate::ManualCapability {
            crate::ManualCapability {
                allowed: !entry.incomplete
                    && !entry.pending
                    && !entry.excluded
                    && entry.rule_ids.is_empty(),
                reason: None,
            }
        }
        fn manual_record(
            &self,
            _: &AnalysisRoot,
            chain: &[Vec<u8>],
        ) -> Result<crate::ManualRecord> {
            self.prepared.fetch_add(1, Ordering::Release);
            if !self.benchmark
                && let Some(gate) = self.hold.lock().unwrap().take()
            {
                gate.wait();
            }
            if self.count > 10 && !self.benchmark {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let id = String::from_utf8(chain.last().unwrap().clone())?;
            Ok(crate::ManualRecord {
                directory: id == "1",
                identity: id.clone(),
                file: crate::FileRecord {
                    path_display: id.clone(),
                    opaque: id,
                    bytes: if chain.len() == 2 {
                        0
                    } else if self.benchmark {
                        1
                    } else {
                        9_007_199_254_740_993
                    },
                    modified_unix: 0,
                },
            })
        }
    }
    pub(super) fn setup(count: usize, partial: bool) -> (Arc<Source>, Arc<AnalysisEngine>, String) {
        setup_mode(count, partial, false)
    }
    fn setup_mode(
        count: usize,
        partial: bool,
        benchmark: bool,
    ) -> (Arc<Source>, Arc<AnalysisEngine>, String) {
        let source = Arc::new(Source {
            count,
            partial,
            benchmark,
            prepared: AtomicUsize::new(0),
            hold: Mutex::new(None),
        });
        let engine = AnalysisEngine::new(source.clone()).unwrap();
        let id = engine.start(&engine.locations()[0].token, vec![]).unwrap();
        let deadline =
            Instant::now() + std::time::Duration::from_secs(if benchmark { 240 } else { 60 });
        while engine.is_busy() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        (source, engine, id)
    }

    fn preview(
        engine: &AnalysisEngine,
        session: &str,
        selected: &[&str],
        excluded: &[&str],
    ) -> Result<crate::ManualPreview> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let cancel = engine.begin_manual_preview(&request_id)?;
        engine.preview_manual(
            &crate::ManualSelection {
                session: session.into(),
                selected_ids: selected.iter().map(|s| (*s).into()).collect(),
                excluded_ids: excluded.iter().map(|s| (*s).into()).collect(),
            },
            request_id,
            cancel,
        )
    }
    #[test]
    fn preview_is_readonly_exact_and_supports_empty_folders() {
        let (source, engine, session) = setup(2, false);
        let result = preview(&engine, &session, &["1", "2"], &["3"]).unwrap();
        assert_eq!(result.summary.files, 1);
        assert_eq!(result.summary.directories, 1);
        assert_eq!(result.summary.bytes, "9007199254740993");
        assert_eq!(
            source.prepared.load(Ordering::Acquire),
            0,
            "preview must not freeze or open deletion handles"
        );
        let frozen = engine
            .prepare_manual(&crate::ManualSelection {
                session: session.clone(),
                selected_ids: vec!["1".into(), "2".into()],
                excluded_ids: vec!["3".into()],
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(result.summary).unwrap(),
            serde_json::to_value(frozen.summary).unwrap()
        );
        assert_eq!(
            preview(&engine, &session, &[], &[]).unwrap().summary.files,
            0
        );
        let (_, empty, session) = setup(0, false);
        let result = preview(&empty, &session, &["1"], &[]).unwrap();
        assert_eq!(
            (
                result.summary.files,
                result.summary.directories,
                result.summary.bytes.as_str()
            ),
            (0, 1, "0")
        );
    }
    #[test]
    fn preview_deduplicates_identity_and_retains_exclusions_and_rule_content() {
        let (source, engine, session) = setup(4, false);
        {
            let inner = engine.inner.lock().unwrap();
            inner
                .db
                .execute("UPDATE nodes SET file_key='same' WHERE id IN ('2','3')", [])
                .unwrap();
            inner
                .db
                .execute("UPDATE nodes SET excluded=1 WHERE id='4'", [])
                .unwrap();
            inner
                .db
                .execute(
                    "UPDATE nodes SET rules='[\"weixin.logs\"]' WHERE id='5'",
                    [],
                )
                .unwrap();
        }
        let result = preview(&engine, &session, &["1"], &[]).unwrap().summary;
        assert_eq!(
            (result.files, result.directories, result.retained),
            (1, 1, 2)
        );
        assert_eq!(result.bytes, "9007199254740993");
        assert_eq!(source.prepared.load(Ordering::Acquire), 0);
        assert!(preview(&engine, &session, &["4"], &[]).is_err());
        assert!(preview(&engine, &session, &["5"], &[]).is_err());
    }
    #[test]
    fn preview_validates_sessions_nodes_and_partial_directories() {
        let (_, engine, session) = setup(2, true);
        assert!(preview(&engine, &session, &["1"], &[]).is_err());
        assert_eq!(
            preview(&engine, &session, &["2"], &[])
                .unwrap()
                .summary
                .files,
            1
        );
        assert!(preview(&engine, "other", &["2"], &[]).is_err());
        assert!(preview(&engine, &session, &["unknown"], &[]).is_err());
        assert!(preview(&engine, &session, &["2", "2"], &[]).is_err());
        assert!(preview(&engine, &session, &["2"], &["2"]).is_err());
        engine
            .inner
            .lock()
            .unwrap()
            .snapshot
            .as_mut()
            .unwrap()
            .stale = true;
        assert!(preview(&engine, &session, &["2"], &[]).is_err());
    }
    #[test]
    fn preview_nearest_override_reselects_inside_an_excluded_subtree() {
        let (_, engine, session) = setup(3, false);
        {
            let mut inner = engine.inner.lock().unwrap();
            inner
                .db
                .execute(
                    "UPDATE nodes SET directory=1,size_key='00000000000000000000' WHERE id='2'",
                    [],
                )
                .unwrap();
            inner
                .db
                .execute("UPDATE nodes SET parent='2' WHERE id='3'", [])
                .unwrap();
            inner.parents.insert("3".into(), Some("2".into()));
            inner.parents.insert("2".into(), Some("1".into()));
        }
        let result = preview(&engine, &session, &["1", "3"], &["2"])
            .unwrap()
            .summary;
        assert_eq!((result.files, result.directories), (2, 1));
        assert_eq!(result.bytes, "18014398509481986");
    }
    #[test]
    fn preview_cancellation_is_isolated_and_does_not_wait_for_locked_index() {
        let (_, engine, session) = setup(2, false);
        let request = uuid::Uuid::new_v4().to_string();
        let old = engine.begin_manual_preview(&request).unwrap();
        let newer = uuid::Uuid::new_v4().to_string();
        let flag = engine.begin_manual_preview(&newer).unwrap();
        assert!(old.load(Ordering::Acquire));
        engine.cancel_manual_preview(&request);
        assert!(!flag.load(Ordering::Acquire));
        let locked = engine.inner.lock().unwrap();
        let task = engine.clone();
        let worker_session = session.clone();
        let id = newer.clone();
        let worker = std::thread::spawn(move || {
            task.preview_manual(
                &crate::ManualSelection {
                    session: worker_session,
                    selected_ids: vec!["1".into()],
                    excluded_ids: vec![],
                },
                id,
                flag,
            )
        });
        let start = Instant::now();
        engine.cancel_manual_preview(&newer);
        assert!(worker.join().unwrap().is_err());
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
        assert!(!engine.cancelled.load(Ordering::Acquire));
        drop(locked);
        assert_eq!(
            preview(&engine, &session, &["2"], &[])
                .unwrap()
                .summary
                .files,
            1
        );
    }
    #[test]
    #[ignore = "explicit three-run synthetic performance benchmark"]
    fn manual_selection_performance() {
        for run in 1..=3 {
            let started = Instant::now();
            let (_, engine, session) = setup_mode(499_998, false, true);
            let index_ms = started.elapsed().as_millis();
            let started = Instant::now();
            let all = preview(&engine, &session, &["1"], &[]).unwrap();
            let preview_ms = started.elapsed().as_millis();
            assert_eq!(
                (
                    all.summary.files,
                    all.summary.directories,
                    all.summary.bytes.as_str()
                ),
                (499_998, 1, "499998")
            );
            let started = Instant::now();
            assert_eq!(
                preview(&engine, &session, &["2"], &[])
                    .unwrap()
                    .summary
                    .files,
                1
            );
            let one_ms = started.elapsed().as_millis();
            let ids: Vec<_> = (2..202).map(|id| id.to_string()).collect();
            let refs: Vec<_> = ids.iter().map(String::as_str).collect();
            let started = Instant::now();
            assert_eq!(
                preview(&engine, &session, &refs, &[])
                    .unwrap()
                    .summary
                    .files,
                200
            );
            let sparse_ms = started.elapsed().as_millis();
            let started = Instant::now();
            let manifest = engine
                .prepare_manual(&crate::ManualSelection {
                    session: session.clone(),
                    selected_ids: vec!["1".into()],
                    excluded_ids: vec![],
                })
                .unwrap();
            let prepare_ms = started.elapsed().as_millis();
            assert_eq!(
                (
                    manifest.summary.files,
                    manifest.summary.directories,
                    manifest.summary.bytes.as_str()
                ),
                (499_998, 1, "499998")
            );
            eprintln!(
                "PERF run={run} index_ms={index_ms} preview_ms={preview_ms} one_ms={one_ms} sparse_ms={sparse_ms} prepare_ms={prepare_ms}"
            );
        }
    }
    #[test]
    #[ignore = "explicit 500000-record resource acceptance"]
    fn manual_preview_half_million_records() {
        let (source, engine, session) = setup(499_998, false);
        assert_eq!(engine.snapshot().unwrap().entries, 500_000);
        let start = Instant::now();
        let result = preview(&engine, &session, &["1"], &[]).unwrap();
        eprintln!(
            "manual_preview_records=500000 elapsed_ms={}",
            start.elapsed().as_millis()
        );
        assert_eq!(result.summary.files, 499_998);
        assert_eq!(result.summary.bytes, "499998");
        assert_eq!(source.prepared.load(Ordering::Acquire), 0);
        let request = uuid::Uuid::new_v4().to_string();
        let flag = engine.begin_manual_preview(&request).unwrap();
        let task = engine.clone();
        let worker_session = session.clone();
        let id = request.clone();
        let worker = std::thread::spawn(move || {
            task.preview_manual(
                &crate::ManualSelection {
                    session: worker_session,
                    selected_ids: vec!["1".into()],
                    excluded_ids: vec![],
                },
                id,
                flag,
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(30));
        let start = Instant::now();
        // UI paging can acquire the index between batches while summary is still running.
        assert!(engine.children(&session, "1", "", 0, true).is_ok());
        let paging_ms = start.elapsed().as_millis();
        let start = Instant::now();
        engine.cancel_manual_preview(&request);
        assert!(worker.join().unwrap().is_err());
        eprintln!(
            "manual_preview_cancel_ms={} paging_ms={paging_ms}",
            start.elapsed().as_millis()
        );
        assert!(paging_ms < 1000);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        let operation_id = uuid::Uuid::new_v4().to_string();
        let operation = engine.begin_manual_preparation(&operation_id).unwrap();
        let task = engine.clone();
        let selected_session = session.clone();
        let worker = std::thread::spawn(move || {
            task.prepare_manual_cancellable(
                &crate::ManualSelection {
                    session: selected_session,
                    selected_ids: vec!["1".into()],
                    excluded_ids: vec![],
                },
                operation.cancellation(),
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(30));
        let paging = Instant::now();
        engine.children(&session, "1", "", 0, true).unwrap();
        let paging_ms = paging.elapsed().as_millis();
        let start = Instant::now();
        engine.cancel_manual_preparation(&operation_id).unwrap();
        assert!(worker.join().unwrap().is_err());
        eprintln!(
            "manual_preparation_cancel_ms={} paging_ms={paging_ms}",
            start.elapsed().as_millis()
        );
        assert!(paging_ms < 1000);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }
    #[test]
    fn nearest_override_and_large_integer_manifest_are_exact() {
        let (_, engine, session) = setup(2, false);
        let manifest = engine
            .prepare_manual(&crate::ManualSelection {
                session,
                selected_ids: vec!["1".into(), "2".into()],
                excluded_ids: vec!["3".into()],
            })
            .unwrap();
        assert_eq!(manifest.summary.files, 1);
        assert_eq!(manifest.summary.directories, 1);
        assert_eq!(manifest.summary.bytes, "9007199254740993");
    }
    #[test]
    fn partial_folder_refuses_bulk_selection_but_complete_file_can_be_chosen() {
        let (_, engine, session) = setup(2, true);
        assert!(
            engine
                .prepare_manual(&crate::ManualSelection {
                    session: session.clone(),
                    selected_ids: vec!["1".into()],
                    excluded_ids: vec![]
                })
                .is_err()
        );
        assert_eq!(
            engine
                .prepare_manual(&crate::ManualSelection {
                    session,
                    selected_ids: vec!["2".into()],
                    excluded_ids: vec![]
                })
                .unwrap()
                .summary
                .files,
            1
        );
    }
    #[test]
    fn blocking_verification_keeps_operation_active_and_discards_stale_or_cancelled_manifest() {
        for cancelled in [false, true] {
            let (source, engine, session) = setup(2, false);
            let gate = Arc::new(std::sync::Barrier::new(2));
            *source.hold.lock().unwrap() = Some(gate.clone());
            let operation_id = uuid::Uuid::new_v4().to_string();
            let guard = engine.begin_manual_preparation(&operation_id).unwrap();
            let task = engine.clone();
            let selected_session = session.clone();
            let worker = std::thread::spawn(move || {
                task.prepare_manual_cancellable(
                    &crate::ManualSelection {
                        session: selected_session,
                        selected_ids: vec!["1".into()],
                        excluded_ids: vec![],
                    },
                    guard.cancellation(),
                )
            });
            let deadline = Instant::now() + std::time::Duration::from_secs(5);
            while source.prepared.load(Ordering::Acquire) == 0 {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            engine.children(&session, "1", "", 0, true).unwrap();
            if cancelled {
                engine.cancel_manual_preparation(&operation_id).unwrap();
            } else {
                engine.mark_stale();
            }
            assert!(!worker.is_finished());
            assert!(
                engine
                    .begin_manual_preparation(&uuid::Uuid::new_v4().to_string())
                    .is_err()
            );
            gate.wait();
            assert!(worker.join().unwrap().is_err());
        }
    }
    #[test]
    fn preparation_is_cancelled_without_waiting_for_the_index_mutex() {
        let (source, engine, session) = setup(100_000, false);
        assert_eq!(engine.snapshot().unwrap().files, 100_000);
        let task = engine.clone();
        let id = session.clone();
        let operation_id = uuid::Uuid::new_v4().to_string();
        let operation = engine.begin_manual_preparation(&operation_id).unwrap();
        let worker = std::thread::spawn(move || {
            task.prepare_manual_cancellable(
                &crate::ManualSelection {
                    session: id,
                    selected_ids: vec!["1".into()],
                    excluded_ids: vec![],
                },
                operation.cancellation(),
            )
        });
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while source.prepared.load(Ordering::Acquire) < 2 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let cancellation = Instant::now();
        let paging = Instant::now();
        engine.children(&session, "1", "", 0, true).unwrap();
        let paging_ms = paging.elapsed().as_millis();
        engine.cancel_manual_preparation(&operation_id).unwrap();
        assert!(worker.join().unwrap().is_err());
        eprintln!(
            "preparation_cancel_and_paging_ms={} paging_ms={paging_ms}",
            cancellation.elapsed().as_millis()
        );
        assert!(cancellation.elapsed() < std::time::Duration::from_secs(2));
        assert!(source.prepared.load(Ordering::Acquire) < 1000);
        assert!(cancellation.elapsed() < std::time::Duration::from_secs(1));
    }
}

#[path = "analysis_preview.rs"]
mod preview;
