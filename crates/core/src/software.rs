//! Software removal is a separate authority from cache cleanup and manual selection.
pub mod residual_rules;
use anyhow::{Result, bail, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// New residue authority has its own version and acceptance list; existing cleanup gates do not apply.
pub const RESIDUAL_RULE_VERSION: u32 = 2;
pub const RESIDUAL_FAMILIES: &[&str] = &[
    "chrome",
    "edge",
    "firefox",
    "brave",
    "vivaldi",
    "weixin",
    "qq",
    "wps",
    "feishu",
    "dingtalk",
    "baidu_netdisk",
    "quark_netdisk",
    "thunder",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareSource {
    Desktop,
    Store,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareMissingField {
    Publisher,
    Version,
    EstimatedBytes,
    InstalledOn,
    InstallLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledSoftware {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub source: SoftwareSource,
    pub scope: String,
    pub estimated_bytes: Option<String>,
    pub installed_on: Option<String>,
    pub install_location: Option<String>,
    /// Optional display-only image. Older persisted records have no icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_data_url: Option<String>,
    pub can_uninstall: bool,
    pub unavailable_reason: Option<String>,
    pub residual_supported: bool,
    pub residual_note: String,
    /// Descriptive inventory metadata, never a grant to scan or delete files.
    #[serde(default)]
    pub residual_capabilities: Vec<residual_rules::ResidualCapability>,
    #[serde(default)]
    pub missing_information: Vec<SoftwareMissingField>,
}
impl InstalledSoftware {
    pub fn with_missing_information(mut self) -> Self {
        use SoftwareMissingField::*;
        let missing = |value: Option<&str>| value.is_none_or(|s| s.trim().is_empty());
        self.missing_information = [
            (Publisher, missing(Some(&self.publisher))),
            (Version, missing(Some(&self.version))),
            (EstimatedBytes, missing(self.estimated_bytes.as_deref())),
            (InstalledOn, missing(self.installed_on.as_deref())),
            (InstallLocation, missing(self.install_location.as_deref())),
        ]
        .into_iter()
        .filter_map(|(field, absent)| absent.then_some(field))
        .collect();
        self
    }
}

/// Recovery identifiers are never accepted from the webview. No executable command is persisted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoftwareIdentity {
    pub source: SoftwareSource,
    pub key: String,
    pub family: String,
}
#[derive(Clone)]
pub struct SoftwareEntry {
    pub software: InstalledSoftware,
    pub identity: SoftwareIdentity,
}
pub struct SoftwareInventory {
    pub entries: Vec<SoftwareEntry>,
    pub warnings: Vec<String>,
}
#[derive(Clone)]
pub struct PreparedSoftware {
    pub entry: SoftwareEntry,
    pub fingerprint: String,
    pub context: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareList {
    pub items: Vec<InstalledSoftware>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallPlan {
    pub id: String,
    pub software: InstalledSoftware,
    pub impact: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UninstallPhase {
    Uninstalling,
    Verifying,
    Removed,
    StillInstalled,
    RestartRequired,
    Cancelled,
    Failed,
    Unknown,
    ScanningResiduals,
    ResidualsReady,
    CleaningResiduals,
    Finished,
}
impl UninstallPhase {
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Uninstalling
                | Self::Verifying
                | Self::ScanningResiduals
                | Self::CleaningResiduals
        )
    }
    pub fn removed(self) -> bool {
        matches!(self, Self::Removed | Self::ResidualsReady | Self::Finished)
    }
}
/// Last standard-uninstall state. A residual failure must not erase this result.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UninstallStatus {
    Uninstalling,
    Verifying,
    Removed,
    StillInstalled,
    RestartRequired,
    Cancelled,
    Failed,
    Unknown,
}
impl UninstallStatus {
    fn from_phase(phase: UninstallPhase) -> Self {
        match phase {
            UninstallPhase::Uninstalling => Self::Uninstalling,
            UninstallPhase::Verifying => Self::Verifying,
            UninstallPhase::StillInstalled => Self::StillInstalled,
            UninstallPhase::RestartRequired => Self::RestartRequired,
            UninstallPhase::Cancelled => Self::Cancelled,
            UninstallPhase::Failed => Self::Failed,
            UninstallPhase::Unknown => Self::Unknown,
            UninstallPhase::Removed
            | UninstallPhase::ScanningResiduals
            | UninstallPhase::ResidualsReady
            | UninstallPhase::CleaningResiduals
            | UninstallPhase::Finished => Self::Removed,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResidualOutcome {
    pub deleted_files: u64,
    pub deleted_directories: u64,
    pub deleted_bytes: String,
    pub skipped: u64,
    pub failed: u64,
    pub unknown: u64,
    /// Selected entries whose deletion was never attempted (for example after Stop).
    #[serde(default)]
    pub unprocessed: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallTask {
    pub id: String,
    pub software: InstalledSoftware,
    pub phase: UninstallPhase,
    /// Optional only for journals written before independent phase results existed.
    #[serde(default)]
    pub uninstall_status: Option<UninstallStatus>,
    pub started_at: i64,
    pub updated_at: i64,
    pub progress: Option<u32>,
    pub message: String,
    pub residual: ResidualOutcome,
    /// Live capability only; recovery never recreates the prepared directory authority.
    #[serde(default)]
    pub can_scan_residuals: bool,
    /// Identifies the current in-memory manifest even if polling misses a short scan.
    #[serde(default)]
    pub residual_scan_id: Option<String>,
}
impl UninstallTask {
    fn normalize_legacy_status(&mut self) {
        self.uninstall_status
            .get_or_insert_with(|| UninstallStatus::from_phase(self.phase));
    }
    fn interrupt_standard_tracking(&mut self) {
        if matches!(
            self.phase,
            UninstallPhase::Uninstalling | UninstallPhase::Verifying
        ) {
            self.uninstall_status = Some(UninstallStatus::Unknown);
        }
    }
}
pub type UninstallReport = UninstallTask;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualCandidate {
    pub id: String,
    pub path: String,
    pub category: String,
    pub bytes: String,
    pub directory: bool,
    pub selectable: bool,
    pub reason: Option<String>,
}
#[derive(Clone)]
pub struct ResidualRecord {
    pub candidate: ResidualCandidate,
    pub opaque: String,
}
pub struct ResidualScanData {
    pub records: Vec<ResidualRecord>,
    pub warnings: Vec<String>,
    pub supported: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualScan {
    pub task_id: String,
    pub scan_id: String,
    pub total: usize,
    pub items: Vec<ResidualCandidate>,
    pub offset: usize,
    pub complete: bool,
    pub supported: bool,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualCleanupPlan {
    pub id: String,
    pub task_id: String,
    pub files: u64,
    pub directories: u64,
    pub bytes: String,
    pub includes_data: bool,
    /// A bounded preview of the exact frozen selection; no frontend paths are accepted.
    pub preview: Vec<ResidualCandidate>,
}
pub enum LaunchOutcome {
    Completed,
    Cancelled,
    RestartRequired,
    Untracked,
}

pub trait SoftwarePlatform: Send + Sync {
    fn restarted_since(&self, _time: i64) -> Result<bool> {
        Ok(false)
    }
    fn inventory(&self) -> Result<SoftwareInventory>;
    fn prepare(&self, entry: &SoftwareEntry) -> Result<PreparedSoftware>;
    fn launch(
        &self,
        prepared: &PreparedSoftware,
        progress: &mut dyn FnMut(u32),
    ) -> Result<LaunchOutcome>;
    /// Errors mean unknown, never absent. Must inspect the original source completely.
    fn installed(&self, identity: &SoftwareIdentity) -> Result<bool>;
    fn scan(
        &self,
        prepared: &PreparedSoftware,
        exclusions: &[String],
        cancel: &AtomicBool,
    ) -> Result<ResidualScanData>;
    fn delete(
        &self,
        prepared: &PreparedSoftware,
        record: &ResidualRecord,
        exclusions: &[String],
    ) -> Result<()>;
}
struct ResidualSnapshot {
    id: String,
    data: ResidualScanData,
}
/// Only aggregate progress is durable. There are no paths, candidate IDs or replay instructions.
#[derive(Clone, Serialize, Deserialize)]
struct CleanupProgress {
    remaining: u64,
    in_flight: bool,
}
#[derive(Serialize, Deserialize)]
struct JournalTask {
    #[serde(flatten)]
    task: UninstallTask,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cleanup_progress: Option<CleanupProgress>,
}
#[derive(Clone)]
struct Operation {
    id: String,
    task_id: String,
}
struct Inner {
    catalog: HashMap<String, SoftwareEntry>,
    inventory_request: Option<String>,
    plan: Option<(UninstallPlan, PreparedSoftware)>,
    task: Option<UninstallTask>,
    identity: Option<SoftwareIdentity>,
    prepared: Option<PreparedSoftware>,
    scan: Option<ResidualSnapshot>,
    cleanup: Option<(ResidualCleanupPlan, Vec<ResidualRecord>)>,
    cleanup_progress: Option<CleanupProgress>,
    operation: Option<String>,
    closing: bool,
}
impl Inner {
    fn available(&self) -> Result<()> {
        ensure!(!self.closing, "拂尘正在退出，不能启动新操作");
        ensure!(
            self.operation.is_none() && !self.task.as_ref().is_some_and(|t| t.phase.active()),
            "任务进行中"
        );
        Ok(())
    }
    fn begin(&mut self) -> Operation {
        let operation = Operation {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: self.task.as_ref().unwrap().id.clone(),
        };
        self.operation = Some(operation.id.clone());
        operation
    }
    fn owns(&self, operation: &Operation) -> bool {
        !self.closing
            && self.operation.as_ref() == Some(&operation.id)
            && self
                .task
                .as_ref()
                .is_some_and(|t| t.id == operation.task_id)
    }
    fn settle_cleanup(&mut self) {
        if let Some(progress) = self.cleanup_progress.take()
            && let Some(task) = &mut self.task
        {
            task.residual.unprocessed += progress.remaining;
            task.residual.unknown += u64::from(progress.in_flight);
        }
    }
    fn revoke_residuals(&mut self) {
        self.prepared = None;
        self.scan = None;
        self.cleanup = None;
    }
}
pub struct SoftwareEngine {
    platform: Arc<dyn SoftwarePlatform>,
    inner: Mutex<Inner>,
    journal: Mutex<Connection>,
    cancel: AtomicBool,
}
impl SoftwareEngine {
    pub fn open(platform: Arc<dyn SoftwarePlatform>, path: &Path) -> Result<Arc<Self>> {
        let journal = Connection::open(path)?;
        journal.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS uninstall_reports(id TEXT PRIMARY KEY, updated INTEGER NOT NULL, data TEXT NOT NULL, identity TEXT NOT NULL);")?;
        let mut recovered = None;
        let rows = journal
            .prepare("SELECT data,identity FROM uninstall_reports ORDER BY updated DESC")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (data, identity) in rows {
            let saved: JournalTask = serde_json::from_str(&data)?;
            let mut task = saved.task;
            task.normalize_legacy_status();
            task.can_scan_residuals = false;
            task.residual_scan_id = None;
            if task.phase.active() {
                task.interrupt_standard_tracking();
                if task.phase == UninstallPhase::CleaningResiduals {
                    if let Some(progress) = saved.cleanup_progress {
                        task.residual.unknown += u64::from(progress.in_flight);
                        task.residual.unprocessed += progress.remaining;
                    } else {
                        // Older journals lack a per-file intent; retain their uncertainty.
                        task.residual.unknown += 1;
                    }
                }
                task.phase = UninstallPhase::Unknown;
                task.message = "上次跟踪已中断，请重新核对；不会自动重试卸载或续删".into();
                task.updated_at = crate::now();
                journal.execute(
                    "UPDATE uninstall_reports SET updated=?1,data=?2 WHERE id=?3",
                    params![task.updated_at, serde_json::to_string(&task)?, task.id],
                )?;
            }
            if !task.phase.active() && task.updated_at < crate::now() - 30 * 86400 {
                journal.execute("DELETE FROM uninstall_reports WHERE id=?1", [&task.id])?;
                continue;
            }
            if recovered.is_none()
                && matches!(
                    task.phase,
                    UninstallPhase::Unknown
                        | UninstallPhase::RestartRequired
                        | UninstallPhase::StillInstalled
                )
            {
                recovered = Some((task, serde_json::from_str::<SoftwareIdentity>(&identity)?));
            }
        }
        let (task, identity) = recovered.map_or((None, None), |(t, i)| (Some(t), Some(i)));
        Ok(Arc::new(Self {
            platform,
            inner: Mutex::new(Inner {
                catalog: HashMap::new(),
                inventory_request: None,
                plan: None,
                task,
                identity,
                prepared: None,
                scan: None,
                cleanup: None,
                cleanup_progress: None,
                operation: None,
                closing: false,
            }),
            journal: Mutex::new(journal),
            cancel: AtomicBool::new(false),
        }))
    }
    fn persist(&self, inner: &Inner) -> Result<()> {
        if let (Some(task), Some(identity)) = (&inner.task, &inner.identity) {
            self.persist_state(task, identity, inner.cleanup_progress.clone())?;
        }
        Ok(())
    }
    fn persist_task(&self, task: &UninstallTask, identity: &SoftwareIdentity) -> Result<()> {
        self.persist_state(task, identity, None)
    }
    fn persist_state(
        &self,
        task: &UninstallTask,
        identity: &SoftwareIdentity,
        cleanup_progress: Option<CleanupProgress>,
    ) -> Result<()> {
        // Installation paths and residue authority belong only to the live view.
        let mut safe = task.clone();
        safe.normalize_legacy_status();
        safe.software.install_location = None;
        safe.can_scan_residuals = false;
        safe.residual_scan_id = None;
        self.journal.lock().unwrap().execute(
            "INSERT OR REPLACE INTO uninstall_reports VALUES(?1,?2,?3,?4)",
            params![
                safe.id,
                safe.updated_at,
                serde_json::to_string(&JournalTask {
                    task: safe.clone(),
                    cleanup_progress
                })?,
                serde_json::to_string(identity)?
            ],
        )?;
        Ok(())
    }
    pub fn is_busy(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.operation.is_some() || inner.task.as_ref().is_some_and(|t| t.phase.active())
    }
    pub fn snapshot(&self) -> Option<UninstallTask> {
        let inner = self.inner.lock().unwrap();
        inner.task.clone().map(|mut task| {
            task.normalize_legacy_status();
            task.can_scan_residuals = task.phase.removed() && inner.prepared.is_some();
            task.residual_scan_id = inner.scan.as_ref().map(|scan| scan.id.clone());
            task
        })
    }
    pub fn list(&self) -> Result<SoftwareList> {
        let request = uuid::Uuid::new_v4().to_string();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.available()?;
            inner.inventory_request = Some(request.clone());
            inner.plan = None;
            inner.catalog.clear();
        }
        let inventory = self.platform.inventory()?;
        let mut inner = self.inner.lock().unwrap();
        inner.available()?;
        // A newer refresh owns the catalog even if that newer read failed.
        ensure!(
            inner.inventory_request.as_ref() == Some(&request),
            "本次软件列表读取已被新的刷新取代"
        );
        inner.plan = None;
        inner.catalog = inventory
            .entries
            .into_iter()
            .map(|e| (e.software.id.clone(), e))
            .collect();
        let mut items: Vec<_> = inner.catalog.values().map(|e| e.software.clone()).collect();
        items.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(SoftwareList {
            items,
            warnings: inventory.warnings,
        })
    }
    pub fn prepare(&self, id: &str) -> Result<UninstallPlan> {
        let mut inner = self.inner.lock().unwrap();
        inner.available()?;
        // A failed replacement must not leave a previously confirmed plan executable.
        inner.plan = None;
        let entry = inner
            .catalog
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("软件列表已失效，请刷新"))?;
        ensure!(
            entry.software.can_uninstall,
            "该软件不能直接卸载，请使用 Windows 应用设置"
        );
        let prepared = self.platform.prepare(entry)?;
        let plan = UninstallPlan { id: uuid::Uuid::new_v4().to_string(), software: prepared.entry.software.clone(), impact: if entry.software.source == SoftwareSource::Store { "Windows 将移除此账户的应用及其应用数据；无法保证数据留到残留检查阶段再选择。" } else { "将打开软件自带卸载程序，可能需要管理员授权。卸载程序可能移除其管理的数据，请先保存正在编辑的内容。" }.into() };
        inner.plan = Some((plan.clone(), prepared));
        Ok(plan)
    }
    pub fn start(self: &Arc<Self>, plan_id: &str) -> Result<String> {
        let mut inner = self.inner.lock().unwrap();
        inner.available()?;
        let (plan, prepared) = inner
            .plan
            .as_ref()
            .filter(|(p, _)| p.id == plan_id)
            .ok_or_else(|| anyhow::anyhow!("卸载确认已失效，请重新选择软件"))?
            .clone();
        let id = uuid::Uuid::new_v4().to_string();
        let task = UninstallTask {
            id: id.clone(),
            software: plan.software,
            phase: UninstallPhase::Uninstalling,
            uninstall_status: Some(UninstallStatus::Uninstalling),
            started_at: crate::now(),
            updated_at: crate::now(),
            progress: None,
            message: "正在启动卸载，请完成软件卸载向导".into(),
            residual: ResidualOutcome {
                deleted_bytes: "0".into(),
                ..Default::default()
            },
            can_scan_residuals: false,
            residual_scan_id: None,
        };
        inner.plan = None;
        // Save before replacing the prior task or launching any native operation.
        self.persist_task(&task, &prepared.entry.identity)?;
        inner.task = Some(task);
        inner.identity = Some(prepared.entry.identity.clone());
        inner.prepared = Some(prepared.clone());
        inner.scan = None;
        inner.cleanup = None;
        let operation = inner.begin();
        drop(inner);
        self.spawn_worker(operation, move |engine, operation| {
            let result = engine.platform.launch(&prepared, &mut |p| {
                let mut inner = engine.inner.lock().unwrap();
                if inner.owns(operation) {
                    inner.task.as_mut().unwrap().progress = Some(p.min(100));
                }
            });
            match result {
                Ok(LaunchOutcome::Completed) => {
                    engine.update(operation, UninstallPhase::Verifying, "正在核对安装状态");
                    engine.verify_worker(operation);
                }
                Ok(LaunchOutcome::Cancelled) => engine.update(
                    operation,
                    UninstallPhase::Cancelled,
                    "卸载已取消，未开始残留清理",
                ),
                Ok(LaunchOutcome::RestartRequired) => engine.update(
                    operation,
                    UninstallPhase::RestartRequired,
                    "卸载程序要求重启；重启后请重新核对，暂不清理残留",
                ),
                Ok(LaunchOutcome::Untracked) => engine.update(
                    operation,
                    UninstallPhase::Unknown,
                    "卸载程序已启动，但无法跟踪结果，请完成向导后重新核对",
                ),
                Err(e) => engine.update(
                    operation,
                    UninstallPhase::Failed,
                    &format!("卸载失败：{e:#}"),
                ),
            }
        })?;
        Ok(id)
    }
    fn spawn_worker(
        self: &Arc<Self>,
        operation: Operation,
        work: impl FnOnce(&Self, &Operation) + Send + 'static,
    ) -> Result<()> {
        let engine = self.clone();
        let worker_operation = operation.clone();
        let spawn = std::thread::Builder::new()
            .name("software-task".into())
            .spawn(move || {
                if !engine.inner.lock().unwrap().owns(&worker_operation) {
                    return;
                }
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    work(&engine, &worker_operation);
                }))
                .is_err()
                {
                    engine.interrupted(
                        &worker_operation,
                        "后台任务异常中断，请重新核对；不会自动重试卸载或续删",
                    );
                }
            });
        if let Err(error) = spawn {
            self.interrupted(&operation, "后台任务未能启动，请重新核对；不会自动重试");
            return Err(error.into());
        }
        Ok(())
    }
    fn interrupted(&self, operation: &Operation, message: &str) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.owns(operation) {
            return;
        }
        inner.revoke_residuals();
        self.update_locked(&mut inner, UninstallPhase::Unknown, message);
    }
    fn update(&self, operation: &Operation, phase: UninstallPhase, message: &str) {
        let mut inner = self.inner.lock().unwrap();
        if inner.owns(operation) {
            self.update_locked(&mut inner, phase, message);
        }
    }
    fn update_locked(&self, inner: &mut Inner, phase: UninstallPhase, message: &str) {
        if !phase.active() {
            inner.settle_cleanup();
            inner.operation = None;
        }
        if let Some(task) = &mut inner.task {
            if matches!(
                task.phase,
                UninstallPhase::Uninstalling | UninstallPhase::Verifying
            ) {
                task.uninstall_status = Some(UninstallStatus::from_phase(phase));
            }
            task.phase = phase;
            task.message = message.into();
            task.progress = None;
            task.updated_at = crate::now();
        }
        if self.persist(inner).is_err()
            && let Some(task) = &mut inner.task
        {
            task.message.push_str("；历史记录保存失败");
        }
    }
    fn verify_worker(&self, operation: &Operation) {
        let (identity, has_context) = {
            let inner = self.inner.lock().unwrap();
            if !inner.owns(operation) {
                return;
            }
            (inner.identity.clone(), inner.prepared.is_some())
        };
        match identity.map(|i| self.platform.installed(&i)) {
            Some(Ok(false)) => self.update(
                operation,
                UninstallPhase::Removed,
                if has_context {
                    "已核对安装登记：软件已移除。可检查本次卸载残留"
                } else {
                    "已核对安装登记：软件已移除。上次卸载范围未保留，本次不能检查或清理残留"
                },
            ),
            Some(Ok(true)) => self.update(
                operation,
                UninstallPhase::StillInstalled,
                "仍有安装登记，卸载可能尚未完成。请完成向导后重新核对",
            ),
            _ => self.update(
                operation,
                UninstallPhase::Unknown,
                "安装状态未能完整读取，无法确认卸载结果，请重试核对",
            ),
        }
    }
    pub fn verify(self: &Arc<Self>, task_id: &str) -> Result<()> {
        let operation;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.available()?;
            let t = inner
                .task
                .as_mut()
                .filter(|t| t.id == task_id)
                .ok_or_else(|| anyhow::anyhow!("任务已变化"))?;
            ensure!(!t.phase.active(), "任务进行中");
            if t.phase == UninstallPhase::RestartRequired {
                ensure!(
                    self.platform.restarted_since(t.updated_at)?,
                    "请先重启 Windows，再核对卸载结果"
                );
            }
            let previous = t.clone();
            t.phase = UninstallPhase::Verifying;
            t.uninstall_status = Some(UninstallStatus::Verifying);
            t.progress = None;
            t.updated_at = crate::now();
            t.message = "正在核对安装状态".into();
            inner.cleanup = None;
            inner.scan = None;
            if let Err(error) = self.persist(&inner) {
                inner.task = Some(previous);
                return Err(error);
            }
            operation = inner.begin();
        }
        self.spawn_worker(operation, |engine, operation| {
            engine.verify_worker(operation)
        })
    }
    pub fn scan(self: &Arc<Self>, task_id: &str, exclusions: Vec<String>) -> Result<()> {
        let (prepared, operation);
        {
            let mut inner = self.inner.lock().unwrap();
            inner.available()?;
            let t = inner
                .task
                .as_ref()
                .filter(|t| t.id == task_id)
                .ok_or_else(|| anyhow::anyhow!("任务已变化"))?;
            ensure!(t.phase.removed(), "卸载结果尚未确认，不能检查残留");
            prepared = inner
                .prepared
                .clone()
                .ok_or_else(|| anyhow::anyhow!("本次卸载范围未保留，不能在重启后重建删除授权"))?;
            inner.scan = None;
            inner.cleanup = None;
            self.cancel.store(false, Ordering::Release);
            inner.task.as_mut().unwrap().phase = UninstallPhase::ScanningResiduals;
            if let Err(error) = self.persist(&inner) {
                let task = inner.task.as_mut().unwrap();
                task.phase = UninstallPhase::Removed;
                task.message = "残留检查未开始：历史记录保存失败，请稍后重新检查".into();
                task.updated_at = crate::now();
                return Err(error);
            }
            operation = inner.begin();
        }
        self.spawn_worker(operation, move |engine, operation| {
            match engine.platform.scan(&prepared, &exclusions, &engine.cancel) {
                Ok(scan) => {
                    let message = if !scan.supported {
                        "暂不支持该软件的残留检查"
                    } else if !scan.warnings.is_empty() {
                        "残留检查未完成，部分位置已保留"
                    } else if scan.records.is_empty() {
                        "没有发现已支持范围内的残留"
                    } else if scan
                        .records
                        .iter()
                        .all(|record| !record.candidate.selectable)
                    {
                        "已发现残留，当前仅支持只读检查，所有项目均已保留"
                    } else {
                        "残留检查完成，请选择需要清理的项目"
                    };
                    let message = message.to_owned();
                    let mut inner = engine.inner.lock().unwrap();
                    if !inner.owns(operation) {
                        return;
                    }
                    inner.scan = Some(ResidualSnapshot {
                        id: uuid::Uuid::new_v4().to_string(),
                        data: scan,
                    });
                    engine.update_locked(&mut inner, UninstallPhase::ResidualsReady, &message);
                }
                Err(e) => engine.update(
                    operation,
                    UninstallPhase::Removed,
                    &format!("残留检查未完成：{e:#}"),
                ),
            }
        })
    }
    pub fn residuals(&self, task: &str, offset: usize, search: &str) -> Result<ResidualScan> {
        let inner = self.inner.lock().unwrap();
        ensure!(
            inner.task.as_ref().is_some_and(|t| t.id == task),
            "任务已变化"
        );
        let snapshot = inner
            .scan
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("尚未完成残留检查"))?;
        let scan = &snapshot.data;
        // Display paths may contain both Windows separators. Normalize only
        // the search comparison, keeping frozen paths, IDs and authority intact.
        let search = search.replace('/', "\\").to_lowercase();
        let records: Vec<_> = scan
            .records
            .iter()
            .filter(|r| {
                r.candidate
                    .path
                    .replace('/', "\\")
                    .to_lowercase()
                    .contains(&search)
            })
            .collect();
        Ok(ResidualScan {
            task_id: task.into(),
            scan_id: snapshot.id.clone(),
            total: records.len(),
            items: records
                .into_iter()
                .skip(offset)
                .take(200)
                .map(|r| r.candidate.clone())
                .collect(),
            offset,
            complete: scan.warnings.is_empty(),
            supported: scan.supported,
            warnings: scan.warnings.clone(),
        })
    }
    pub fn prepare_cleanup(&self, task: &str, ids: Vec<String>) -> Result<ResidualCleanupPlan> {
        let mut inner = self.inner.lock().unwrap();
        inner.available()?;
        inner.cleanup = None;
        ensure!(
            inner
                .task
                .as_ref()
                .is_some_and(|t| t.id == task && t.phase == UninstallPhase::ResidualsReady),
            "残留检查已失效"
        );
        let scan = inner
            .scan
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("请先检查残留"))?;
        let scan = &scan.data;
        let ids: HashSet<_> = ids.into_iter().collect();
        ensure!(!ids.is_empty(), "请选择残留文件");
        let records: Vec<_> = scan
            .records
            .iter()
            .filter(|r| ids.contains(&r.candidate.id))
            .cloned()
            .collect();
        ensure!(
            records.len() == ids.len() && records.iter().all(|r| r.candidate.selectable),
            "选择包含无效或只读项目"
        );
        let bytes = records
            .iter()
            .filter(|r| !r.candidate.directory)
            .try_fold(0u64, |n, r| {
                n.checked_add(r.candidate.bytes.parse().unwrap_or(0))
                    .ok_or_else(|| anyhow::anyhow!("容量溢出"))
            })?;
        let plan = ResidualCleanupPlan {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task.into(),
            files: records.iter().filter(|r| !r.candidate.directory).count() as u64,
            directories: records.iter().filter(|r| r.candidate.directory).count() as u64,
            bytes: bytes.to_string(),
            includes_data: records.iter().any(|r| r.candidate.category == "data"),
            preview: records
                .iter()
                .take(20)
                .map(|r| r.candidate.clone())
                .collect(),
        };
        inner.cleanup = Some((plan.clone(), records));
        Ok(plan)
    }
    pub fn clean(self: &Arc<Self>, plan_id: &str, exclusions: Vec<String>) -> Result<()> {
        let (prepared, mut records, operation);
        {
            let mut inner = self.inner.lock().unwrap();
            inner.available()?;
            ensure!(
                inner
                    .task
                    .as_ref()
                    .is_some_and(|t| t.phase == UninstallPhase::ResidualsReady),
                "任务已变化"
            );
            let (_, r) = inner
                .cleanup
                .as_ref()
                .filter(|(p, _)| p.id == plan_id)
                .ok_or_else(|| anyhow::anyhow!("清理确认已失效"))?;
            records = r.clone();
            prepared = inner
                .prepared
                .clone()
                .ok_or_else(|| anyhow::anyhow!("残留范围已失效"))?;
            self.cancel.store(false, Ordering::Release);
            inner.task.as_mut().unwrap().phase = UninstallPhase::CleaningResiduals;
            inner.cleanup = None;
            inner.cleanup_progress = Some(CleanupProgress {
                remaining: records.len() as u64,
                in_flight: false,
            });
            if let Err(error) = self.persist(&inner) {
                inner.cleanup_progress = None;
                let task = inner.task.as_mut().unwrap();
                task.phase = UninstallPhase::ResidualsReady;
                task.message = "残留清理未开始：历史记录保存失败，请稍后重新确认".into();
                task.updated_at = crate::now();
                return Err(error);
            }
            operation = inner.begin();
        }
        // Files before directories; deepest directories first. Never recurse during deletion.
        records.sort_by(|a, b| {
            a.candidate
                .directory
                .cmp(&b.candidate.directory)
                .then(b.candidate.path.len().cmp(&a.candidate.path.len()))
        });
        self.spawn_worker(operation, move |engine, operation| {
            for record in &records {
                {
                    let mut inner = engine.inner.lock().unwrap();
                    if !inner.owns(operation) {
                        return;
                    }
                    if engine.cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let progress = inner.cleanup_progress.as_mut().unwrap();
                    progress.remaining -= 1;
                    progress.in_flight = true;
                    // An intent must be durable before a file operation can begin.
                    if engine.persist(&inner).is_err() {
                        let progress = inner.cleanup_progress.as_mut().unwrap();
                        progress.remaining += 1;
                        progress.in_flight = false;
                        inner.revoke_residuals();
                        engine.update_locked(
                            &mut inner,
                            UninstallPhase::Unknown,
                            "删除进度保存失败，已停止残留清理；未处理项目保留",
                        );
                        return;
                    }
                }
                let result = engine.platform.delete(&prepared, record, &exclusions);
                let mut inner = engine.inner.lock().unwrap();
                if !inner.owns(operation) {
                    return;
                }
                inner.cleanup_progress.as_mut().unwrap().in_flight = false;
                let t = inner.task.as_mut().unwrap();
                match result {
                    Ok(()) => {
                        if record.candidate.directory {
                            t.residual.deleted_directories += 1;
                        } else {
                            t.residual.deleted_files += 1;
                            let n = t.residual.deleted_bytes.parse::<u64>().unwrap_or(0);
                            t.residual.deleted_bytes =
                                (n + record.candidate.bytes.parse::<u64>().unwrap_or(0))
                                    .to_string();
                        }
                    }
                    Err(e) => {
                        if e.is::<crate::Refusal>() {
                            t.residual.skipped += 1;
                        } else {
                            t.residual.failed += 1;
                        }
                    }
                }
                if engine.persist(&inner).is_err() {
                    inner.revoke_residuals();
                    engine.update_locked(
                        &mut inner,
                        UninstallPhase::Unknown,
                        "历史写入失败，已停止残留清理，请核对结果",
                    );
                    return;
                }
            }
            let mut inner = engine.inner.lock().unwrap();
            if !inner.owns(operation) {
                return;
            }
            inner.scan = None;
            engine.update_locked(
                &mut inner,
                UninstallPhase::Finished,
                if engine.cancel.load(Ordering::Acquire) {
                    "已停止后续清理，已删除内容不会恢复；未处理项目保留"
                } else {
                    "残留处理结束；请核对已删除、保留和失败数量"
                },
            );
        })
    }
    pub fn cancel_residuals(&self, task: &str) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        let t = inner
            .task
            .as_ref()
            .filter(|t| t.id == task)
            .ok_or_else(|| anyhow::anyhow!("任务已变化"))?;
        ensure!(
            matches!(
                t.phase,
                UninstallPhase::ScanningResiduals | UninstallPhase::CleaningResiduals
            ),
            "请在软件卸载向导中取消卸载"
        );
        self.cancel.store(true, Ordering::Release);
        Ok(())
    }
    pub fn stop_tracking(&self) {
        let mut inner = self.inner.lock().unwrap();
        self.cancel.store(true, Ordering::Release);
        inner.closing = true;
        inner.operation = None;
        inner.plan = None;
        inner.catalog.clear();
        inner.revoke_residuals();
        inner.settle_cleanup();
        if let Some(task) = &mut inner.task
            && task.phase.active()
        {
            task.interrupt_standard_tracking();
            task.phase = UninstallPhase::Unknown;
            task.progress = None;
            task.message = "拂尘已停止跟踪，请重新打开后核对结果；不会自动续删".into();
            task.updated_at = crate::now();
            let _ = self.persist(&inner);
        }
    }
    pub fn history(&self) -> Result<Vec<UninstallReport>> {
        let active = self
            .snapshot()
            .filter(|t| t.phase.active())
            .map(|t| t.id)
            .unwrap_or_default();
        let journal = self.journal.lock().unwrap();
        journal.execute(
            "DELETE FROM uninstall_reports WHERE updated < ?1 AND id != ?2",
            params![crate::now() - 30 * 86400, active],
        )?;
        let rows = journal
            .prepare("SELECT data FROM uninstall_reports ORDER BY updated DESC LIMIT 100")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|r| {
                let mut task: UninstallReport = serde_json::from_str(&r)?;
                task.normalize_legacy_status();
                Ok(task)
            })
            .collect()
    }
    pub fn clear_history(&self) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        inner.available()?;
        self.journal
            .lock()
            .unwrap()
            .execute("DELETE FROM uninstall_reports", [])?;
        Ok(())
    }
}

/// Strict parser for a registered executable. It never invokes a command shell or searches PATH.
pub fn executable_command(value: &str) -> Result<(String, String)> {
    let s = value.trim();
    ensure!(
        !s.is_empty() && !s.contains(['\0', '\r', '\n']),
        "卸载命令无效"
    );
    let (file, args) = if let Some(s) = s.strip_prefix('"') {
        let (p, a) = s
            .split_once('"')
            .ok_or_else(|| anyhow::anyhow!("卸载命令引号不完整"))?;
        (p, a.trim())
    } else {
        let (p, a) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
        (p, a.trim())
    };
    let b = file.as_bytes();
    ensure!(
        b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'),
        "卸载程序路径不明确，请使用 Windows 设置"
    );
    ensure!(
        !file[2..].contains(':') && !file.split(['\\', '/']).any(|s| matches!(s, ".." | ".")),
        "卸载路径无效"
    );
    let name = file.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
    ensure!(name.ends_with(".exe"), "仅支持直接运行本地卸载程序");
    if matches!(
        name.as_str(),
        "cmd.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "rundll32.exe"
            | "regsvr32.exe"
            | "wscript.exe"
            | "cscript.exe"
            | "mshta.exe"
            | "msiexec.exe"
            | "explorer.exe"
    ) {
        bail!("该卸载命令需由 Windows 应用设置处理");
    }
    Ok((file.into(), args.into()))
}
