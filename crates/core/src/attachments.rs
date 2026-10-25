//! Explicit, session-scoped messaging attachments. No path-based IPC deletion.
#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;
use crate::*;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use uuid::Uuid;

pub const ATTACHMENT_LIMIT: usize = 50_000;
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentProvider {
    Weixin,
    QqClassic,
    Qqnt,
}
impl AttachmentProvider {
    pub fn source(self) -> &'static str {
        match self {
            Self::Weixin => "weixin.attachments",
            Self::QqClassic => "qq.attachments",
            Self::Qqnt => "qqnt.attachments",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Weixin => "微信",
            Self::QqClassic => "经典 QQ",
            Self::Qqnt => "QQNT",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentDiscovery {
    pub provider: AttachmentProvider,
    pub locations: Vec<AttachmentLocation>,
    pub state: String,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    Video,
    Audio,
    File,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentLocation {
    pub provider: AttachmentProvider,
    pub token: String,
    pub path: String,
    pub title: String,
    pub version: String,
    pub cleanup_block: Option<String>,
}
#[derive(Debug, Clone)]
pub struct AttachmentRoot {
    pub location: AttachmentLocation,
    pub opaque: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: AttachmentKind,
    pub bytes: String,
    pub modified_unix: i64,
    pub preview_available: bool,
    pub cleanup_block: Option<String>,
    pub outcome: Option<String>,
}
#[derive(Debug, Clone)]
pub struct AttachmentRecord {
    pub entry: AttachmentEntry,
    pub record: ManualRecord,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentSnapshot {
    pub session: String,
    pub location: AttachmentLocation,
    pub state: String,
    pub files: usize,
    pub bytes: String,
    pub issues: u64,
    pub error: Option<String>,
    pub deleted_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentSelection {
    pub session: String,
    pub selected_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentQuery {
    pub session: String,
    pub kind: Option<AttachmentKind>,
    pub min_age_days: u32,
    pub search: String,
    pub offset: usize,
    pub selected_ids: Option<Vec<String>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentPage {
    pub entries: Vec<AttachmentEntry>,
    pub total: usize,
    pub filtered_bytes: String,
    pub selectable_files: usize,
    pub selectable_bytes: String,
    pub categories: Vec<AttachmentCategory>,
    pub snapshot: AttachmentSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentCategory {
    pub kind: AttachmentKind,
    pub files: usize,
    pub bytes: String,
    pub retained_files: usize,
    pub retained_bytes: String,
    pub deleted_files: usize,
    pub deleted_bytes: String,
}

pub trait AttachmentPlatform: AnalysisPlatform {
    fn candidate_locations(&self, provider: AttachmentProvider) -> Result<Vec<String>>;
    fn account(&self, provider: AttachmentProvider, path: &str) -> Result<AttachmentRoot>;
    fn scan_attachments(
        &self,
        root: &AttachmentRoot,
        exclusions: &[String],
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(Result<AttachmentRecord>) -> Result<()>,
    ) -> Result<()>;
    fn verify_attachment(
        &self,
        root: &AttachmentRoot,
        item: &ManualRecord,
        exclusions: &[String],
    ) -> Result<()>;
    fn attachment_thumbnail(&self, item: &ManualRecord) -> Result<String>;
    fn reveal_attachment(&self, item: &ManualRecord) -> Result<()>;
}
struct Session {
    root: AttachmentRoot,
    snapshot: AttachmentSnapshot,
    items: Vec<AttachmentRecord>,
    identities: HashSet<String>,
    memory: usize,
}
type Outcomes = Arc<Mutex<HashMap<String, String>>>;
pub struct AttachmentEngine {
    platform: Arc<dyn AttachmentPlatform>,
    locations: Mutex<HashMap<String, AttachmentRoot>>,
    session: Mutex<Option<Session>>,
    cancelled: AtomicBool,
    generation: AtomicU64,
    outcomes: Outcomes,
    // A bounded session-only LRU. No media is written to the journal.
    thumbnails: Mutex<Vec<(String, String)>>,
}
impl AttachmentEngine {
    pub fn new(platform: Arc<dyn AttachmentPlatform>) -> Arc<Self> {
        Arc::new(Self {
            platform,
            locations: Mutex::new(HashMap::new()),
            session: Mutex::new(None),
            cancelled: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            outcomes: Arc::new(Mutex::new(HashMap::new())),
            thumbnails: Mutex::new(vec![]),
        })
    }
    pub fn discover(&self, provider: AttachmentProvider) -> AttachmentDiscovery {
        let mut result = AttachmentDiscovery {
            provider,
            locations: vec![],
            state: "no_accounts".into(),
            issues: vec![],
        };
        let paths = match self.platform.candidate_locations(provider) {
            Ok(paths) => paths,
            Err(error) => {
                result.state = error
                    .downcast_ref::<ApiError>()
                    .map_or("unverified_installation", |e| e.code)
                    .into();
                result.issues.push(error.to_string());
                return result;
            }
        };
        for path in paths {
            match self.register(provider, &path) {
                Ok(location) => {
                    if !result.locations.iter().any(|l| l.token == location.token) {
                        result.locations.push(location);
                    }
                }
                Err(error) => result.issues.push(format!("{path}：{error}")),
            }
        }
        result.state = if !result.locations.is_empty() {
            "ready"
        } else if !result.issues.is_empty() {
            "unsupported_location"
        } else {
            "no_accounts"
        }
        .into();
        result
    }
    pub fn register(&self, provider: AttachmentProvider, path: &str) -> Result<AttachmentLocation> {
        let mut root = self.platform.account(provider, path)?;
        ensure!(root.location.provider == provider, "账号资料来源不匹配");
        let mut locations = self.locations.lock().unwrap();
        if let Some(existing) = locations
            .values()
            .find(|r| r.location.provider == provider && r.location.path == root.location.path)
        {
            root.location.token = existing.location.token.clone();
        } else {
            ensure!(locations.len() < 64, "资料位置过多，请重新打开程序");
            root.location.token = Uuid::new_v4().to_string();
        }
        let location = root.location.clone();
        locations.insert(location.token.clone(), root);
        Ok(location)
    }
    pub fn is_busy(&self) -> bool {
        self.session
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|s| s.snapshot.state == "scanning")
    }
    pub fn start(self: &Arc<Self>, token: &str, exclusions: Vec<String>) -> Result<String> {
        let root = self
            .locations
            .lock()
            .unwrap()
            .get(token)
            .cloned()
            .ok_or_else(|| ApiError::new("invalid_location", "请重新选择账号资料位置"))?;
        let fresh = self
            .platform
            .account(root.location.provider, &root.location.path)?;
        ensure!(
            fresh.opaque == root.opaque && fresh.location.provider == root.location.provider,
            "资料位置或应用安装已变化，请重新选择位置"
        );
        let mut session = self.session.lock().unwrap();
        ensure!(
            !session
                .as_ref()
                .is_some_and(|s| s.snapshot.state == "scanning"),
            "附件扫描进行中"
        );
        let id = Uuid::new_v4().to_string();
        self.generation.fetch_add(1, Ordering::AcqRel);
        *session = Some(Session {
            root: root.clone(),
            snapshot: AttachmentSnapshot {
                session: id.clone(),
                location: root.location.clone(),
                state: "scanning".into(),
                files: 0,
                bytes: "0".into(),
                issues: 0,
                error: None,
                deleted_ids: vec![],
            },
            items: vec![],
            identities: HashSet::new(),
            memory: 0,
        });
        self.outcomes.lock().unwrap().clear();
        self.thumbnails.lock().unwrap().clear();
        self.cancelled.store(false, Ordering::Release);
        drop(session);
        let engine = self.clone();
        std::thread::spawn(move || {
            let result = engine.platform.scan_attachments(
                &root,
                &exclusions,
                &engine.cancelled,
                &mut |item| {
                    ensure!(!engine.cancelled.load(Ordering::Acquire), "扫描已停止");
                    let mut guard = engine.session.lock().unwrap();
                    let s = guard.as_mut().unwrap();
                    match item {
                        Err(_) => {
                            s.snapshot.issues += 1;
                        }
                        Ok(mut item) => {
                            let memory =
                                item.record.file.opaque.len() + item.entry.path.len() * 2 + 512;
                            ensure!(
                                s.items.len() < ATTACHMENT_LIMIT
                                    && s.memory + memory <= MEMORY_LIMIT,
                                "已达到本次附件扫描上限，请选择更小的资料范围"
                            );
                            if s.identities.insert(item.record.identity.clone()) {
                                item.entry.id = Uuid::new_v4().to_string();
                                s.snapshot.bytes = (s.snapshot.bytes.parse::<u64>()?
                                    + item.record.file.bytes)
                                    .to_string();
                                s.memory += memory;
                                s.items.push(item);
                                s.snapshot.files = s.items.len();
                            }
                        }
                    }
                    Ok(())
                },
            );
            let mut guard = engine.session.lock().unwrap();
            let s = guard.as_mut().unwrap();
            s.items.sort_by(|a, b| {
                b.record
                    .file
                    .bytes
                    .cmp(&a.record.file.bytes)
                    .then(a.entry.id.cmp(&b.entry.id))
            });
            s.snapshot.state = if engine.cancelled.load(Ordering::Acquire) {
                "cancelled"
            } else if result.is_err() || s.snapshot.issues > 0 {
                "incomplete"
            } else {
                "complete"
            }
            .into();
            s.snapshot.error = result.err().map(|e| e.to_string());
        });
        Ok(id)
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn snapshot(&self) -> Option<AttachmentSnapshot> {
        self.session.lock().unwrap().as_ref().map(|s| {
            let mut snapshot = s.snapshot.clone();
            let outcomes = self.outcomes.lock().unwrap();
            snapshot.deleted_ids = s
                .items
                .iter()
                .filter(|r| {
                    outcomes
                        .get(&r.record.identity)
                        .is_some_and(|v| v == "deleted")
                })
                .map(|r| r.entry.id.clone())
                .collect();
            snapshot
        })
    }
    pub fn close(&self) -> Result<()> {
        ensure!(!self.is_busy(), "请先停止附件扫描");
        self.generation.fetch_add(1, Ordering::AcqRel);
        *self.session.lock().unwrap() = None;
        self.thumbnails.lock().unwrap().clear();
        self.outcomes.lock().unwrap().clear();
        Ok(())
    }
    pub fn page(&self, q: AttachmentQuery) -> Result<AttachmentPage> {
        ensure!(
            [0, 30, 90, 180].contains(&q.min_age_days) && q.search.len() <= 512,
            "筛选条件无效"
        );
        ensure!(
            q.selected_ids
                .as_ref()
                .is_none_or(|ids| ids.len() <= ATTACHMENT_LIMIT),
            "选择过多"
        );
        let guard = self.session.lock().unwrap();
        let s = guard
            .as_ref()
            .filter(|s| s.snapshot.session == q.session)
            .ok_or_else(|| ApiError::new("invalid_session", "附件会话已失效，请重新扫描"))?;
        let selected: Option<HashSet<_>> = q.selected_ids.map(|ids| ids.into_iter().collect());
        let needle = q.search.to_lowercase();
        let outcomes = self.outcomes.lock().unwrap();
        let mut total = 0;
        let mut filtered_bytes = 0u64;
        let mut selectable_files = 0;
        let mut selectable_bytes = 0u64;
        let mut categories: Vec<AttachmentCategory> = [
            AttachmentKind::Image,
            AttachmentKind::Video,
            AttachmentKind::Audio,
            AttachmentKind::File,
            AttachmentKind::Unknown,
        ]
        .into_iter()
        .map(|kind| AttachmentCategory {
            kind,
            files: 0,
            bytes: "0".into(),
            retained_files: 0,
            retained_bytes: "0".into(),
            deleted_files: 0,
            deleted_bytes: "0".into(),
        })
        .collect();
        let mut entries = vec![];
        for item in &s.items {
            let processed = outcomes.contains_key(&item.record.identity);
            let deleted = outcomes
                .get(&item.record.identity)
                .is_some_and(|v| v == "deleted");
            let blocked =
                item.entry.cleanup_block.is_some() || s.snapshot.state != "complete" || processed;
            let category = categories
                .iter_mut()
                .find(|c| c.kind == item.entry.kind)
                .unwrap();
            category.files += 1;
            category.bytes = (category.bytes.parse::<u64>()? + item.record.file.bytes).to_string();
            if deleted {
                category.deleted_files += 1;
                category.deleted_bytes =
                    (category.deleted_bytes.parse::<u64>()? + item.record.file.bytes).to_string();
            } else if blocked {
                category.retained_files += 1;
                category.retained_bytes =
                    (category.retained_bytes.parse::<u64>()? + item.record.file.bytes).to_string();
            }
            if selected
                .as_ref()
                .is_some_and(|ids| !ids.contains(&item.entry.id))
                || q.kind.is_some_and(|kind| kind != item.entry.kind)
                || !item.entry.name.to_lowercase().contains(&needle)
                || (q.min_age_days > 0
                    && (item.entry.modified_unix <= 0
                        || now().saturating_sub(item.entry.modified_unix)
                            < i64::from(q.min_age_days) * 86400))
            {
                continue;
            }
            filtered_bytes += item.record.file.bytes;
            if !blocked {
                selectable_files += 1;
                selectable_bytes += item.record.file.bytes;
            }
            if total >= q.offset && entries.len() < 100 {
                let mut entry = item.entry.clone();
                entry.outcome = outcomes.get(&item.record.identity).cloned();
                if s.snapshot.state != "complete" {
                    entry.cleanup_block = Some("本次扫描未完整结束，已保留".into());
                }
                if entry.outcome.is_some() {
                    entry.cleanup_block = Some("本次已处理，请重新扫描后再选择".into());
                }
                entries.push(entry);
            }
            total += 1;
        }
        Ok(AttachmentPage {
            entries,
            total,
            filtered_bytes: filtered_bytes.to_string(),
            selectable_files,
            selectable_bytes: selectable_bytes.to_string(),
            categories,
            snapshot: s.snapshot.clone(),
        })
    }
    fn item(&self, session: &str, id: &str) -> Result<AttachmentRecord> {
        let guard = self.session.lock().unwrap();
        let s = guard
            .as_ref()
            .filter(|s| s.snapshot.session == session)
            .ok_or_else(|| ApiError::new("invalid_session", "附件会话已失效"))?;
        s.items
            .iter()
            .find(|r| r.entry.id == id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("附件不存在"))
    }
    pub fn thumbnail(&self, session: &str, id: &str) -> Result<String> {
        let generation = self.generation.load(Ordering::Acquire);
        let item = self.item(session, id)?;
        ensure!(item.entry.preview_available, "此格式暂不支持预览");
        // Serialize extraction: at most one Shell decoder and 32 cached images per session.
        let mut cache = self.thumbnails.lock().unwrap();
        ensure!(
            generation == self.generation.load(Ordering::Acquire),
            "附件会话已失效"
        );
        let key = format!("{session}:{id}");
        if let Some(index) = cache.iter().position(|(k, _)| k == &key) {
            let value = cache.remove(index);
            let result = value.1.clone();
            cache.push(value);
            return Ok(result);
        }
        let data = self.platform.attachment_thumbnail(&item.record)?;
        ensure!(
            generation == self.generation.load(Ordering::Acquire),
            "附件会话已失效"
        );
        ensure!(data.len() <= 2 * 1024 * 1024, "预览图片过大");
        if cache.len() == 32 {
            cache.remove(0);
        }
        cache.push((key, data.clone()));
        Ok(data)
    }
    pub fn reveal(&self, session: &str, id: &str) -> Result<()> {
        self.platform
            .reveal_attachment(&self.item(session, id)?.record)
    }
    pub fn prepare(
        &self,
        selection: &AttachmentSelection,
        exclusions: &[String],
        cancel: &AtomicBool,
    ) -> Result<ManualManifest> {
        ensure!(
            !selection.selected_ids.is_empty() && selection.selected_ids.len() <= ATTACHMENT_LIMIT,
            "请选择附件"
        );
        let guard = self.session.lock().unwrap();
        let s = guard
            .as_ref()
            .filter(|s| s.snapshot.session == selection.session)
            .ok_or_else(|| ApiError::new("invalid_session", "附件会话已失效"))?;
        ensure!(s.snapshot.state == "complete", "扫描未完整结束，请重新扫描");
        ensure!(
            s.root.location.cleanup_block.is_none(),
            "当前账号资料尚未通过附件清理兼容性核验"
        );
        let ids: HashSet<_> = selection.selected_ids.iter().collect();
        ensure!(ids.len() == selection.selected_ids.len(), "附件选择重复");
        let items: Vec<_> = s
            .items
            .iter()
            .filter(|r| ids.contains(&r.entry.id))
            .cloned()
            .collect();
        ensure!(items.len() == ids.len(), "附件选择已失效");
        let executor = Arc::new(AttachmentExecutor {
            platform: self.platform.clone(),
            outcomes: self.outcomes.clone(),
        });
        let mut manifest = ManualManifest::new(executor, selection.session.clone())?;
        manifest.summary.source = Some(s.root.location.provider.source().into());
        for item in items {
            ensure!(!cancel.load(Ordering::Acquire), "准备已取消");
            ensure!(
                item.entry.cleanup_block.is_none()
                    && !self
                        .outcomes
                        .lock()
                        .unwrap()
                        .contains_key(&item.record.identity),
                "附件已保留或已经处理"
            );
            self.platform
                .verify_attachment(&s.root, &item.record, exclusions)?;
            manifest.insert(&item.entry.id, 0, item.record)?;
        }
        Ok(manifest)
    }
}
struct AttachmentExecutor {
    platform: Arc<dyn AttachmentPlatform>,
    outcomes: Outcomes,
}
impl AnalysisPlatform for AttachmentExecutor {
    fn locations(&self) -> Vec<(String, String)> {
        vec![]
    }
    fn validate_root(&self, _: &str) -> Result<AnalysisRoot> {
        anyhow::bail!("不支持通用分析")
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        _: &AtomicBool,
        _: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        anyhow::bail!("不支持通用分析")
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        anyhow::bail!("不支持通用分析")
    }
    fn delete_manual(&self, record: &ManualRecord, exclusions: &[String]) -> Result<()> {
        let result = self.platform.delete_manual(record, exclusions);
        self.outcomes.lock().unwrap().insert(
            record.identity.clone(),
            match &result {
                Ok(()) => "deleted".into(),
                Err(e) => format!("保留：{e}"),
            },
        );
        result
    }
}
