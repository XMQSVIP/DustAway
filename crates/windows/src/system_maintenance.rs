//! Category-based Windows maintenance has its own estimates, plans and durable receipts.
use crate::{
    fs::*,
    windows_cleanup::{Handler, SPECS, spec},
};
use anyhow::{Context, Result, ensure};
use dustaway_core::{Scope, now};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowsAction {
    pub id: String,
    pub title: String,
    pub scope: Scope,
    pub impact: String,
    pub estimate_bytes: Option<String>,
    pub volume: Option<String>,
    pub available: bool,
    pub validated: bool,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowsPlan {
    pub id: String,
    pub session: String,
    pub actions: Vec<WindowsAction>,
    pub expires_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowsOutcome {
    pub id: String,
    pub title: String,
    pub status: String,
    pub message: String,
    pub before_bytes: Option<String>,
    pub after_bytes: Option<String>,
    pub free_before: Option<String>,
    pub free_after: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowsReport {
    pub id: String,
    pub scope: Scope,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: String,
    pub actions: Vec<WindowsOutcome>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowsSnapshot {
    pub session: String,
    pub status: String,
    pub actions: Vec<WindowsAction>,
    pub plan: Option<WindowsPlan>,
    pub report: Option<WindowsReport>,
    pub current: Option<String>,
    pub message: String,
    pub progress_bytes: Option<String>,
}
struct Prepared {
    public: WindowsPlan,
    fingerprints: Vec<String>,
    exclusions: Vec<String>,
}
pub struct WindowsMaintenance {
    scope: Scope,
    directory: PathBuf,
    state: Mutex<WindowsSnapshot>,
    plan: Mutex<Option<Prepared>>,
    busy: AtomicBool,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
}
// Each entry must link to successful real deletion and post-operation checks.
// Zero-candidate and mock runs do not enable production deletion.
pub fn accepted(id: &str) -> bool {
    #[cfg(test)]
    if tests::isolated_acceptance() {
        return true;
    }
    // docs/system-cleanup-acceptance.md: independent, positive-data Windows 11 runs.
    matches!(
        id,
        "windows.thumbnail"
            | "windows.shader"
            | "windows.delivery"
            | "windows.upgrade_logs"
            | "windows.memory_dump"
            | "windows.minidump"
            | "windows.setup_logs"
    )
}

pub fn inspect(scope: Scope) -> Vec<WindowsAction> {
    probe(
        scope,
        &[],
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicU64::new(0)),
    )
}
/// Bind both path exclusions and overlapping category exclusions to the plan.
pub fn settings_exclusions(settings: &dustaway_core::Settings) -> Vec<String> {
    let mut exclusions = settings.exclusions.clone();
    for rule in &settings.excluded_rules {
        let action = match rule.as_str() {
            "windows.directx_cache" => Some("windows.shader"),
            "windows.wer_archive" => Some("windows.error_reports"),
            _ => None,
        };
        if let Some(action) = action {
            exclusions.push(format!("@category:{action}"));
        }
    }
    exclusions
}
fn excluded(action: &str, exclusions: &[String]) -> bool {
    exclusions
        .iter()
        .any(|value| match value.strip_prefix("@category:") {
            Some(id) => id == action,
            None => true,
        })
}
fn probe(
    scope: Scope,
    exclusions: &[String],
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
) -> Vec<WindowsAction> {
    SPECS
        .iter()
        .filter(|s| s.scope == scope)
        .map(|s| {
            let mut row = WindowsAction {
                id: s.id.into(),
                title: s.title.into(),
                scope,
                impact: s.impact.into(),
                estimate_bytes: None,
                volume: None,
                available: false,
                validated: accepted(s.id),
                message: String::new(),
            };
            if cancel.load(Ordering::Acquire) {
                row.message = "检查已取消".into();
                return row;
            }
            let result = (|| -> Result<()> {
                ensure!(
                    !excluded(s.id, exclusions),
                    "已设置路径或相关类别排除；Windows 按类别维护无法遵守该排除，本项已保留"
                );
                let handler = Handler::open(s, false, cancel.clone(), progress.clone())?;
                row.volume = Some(handler.volume.clone());
                row.estimate_bytes = handler.estimate()?.map(|b| b.to_string());
                row.available =
                    row.validated && row.estimate_bytes.as_deref().is_some_and(|b| b != "0");
                row.message = if row.estimate_bytes.is_none() {
                    "Windows 未提供可靠估算"
                } else if row.estimate_bytes.as_deref() == Some("0") {
                    "Windows 未发现可清理内容"
                } else if !row.validated {
                    "当前仅支持估算，可使用 Windows 存储设置完成整理"
                } else {
                    "Windows 官方估算；需单独选择确认"
                }
                .into();
                Ok(())
            })();
            if let Err(error) = result {
                row.message = format!("{error:#}");
            }
            row
        })
        .collect()
}
impl WindowsMaintenance {
    pub fn new(scope: Scope, data: &Path) -> Result<Arc<Self>> {
        let directory = data.join("windows-maintenance-history");
        std::fs::create_dir_all(&directory)?;
        let _pin = PinnedPath::open(&directory, true, false)?;
        let this = Arc::new(Self {
            scope,
            directory,
            state: Mutex::new(WindowsSnapshot {
                session: String::new(),
                status: "idle".into(),
                actions: vec![],
                plan: None,
                report: None,
                current: None,
                message: String::new(),
                progress_bytes: None,
            }),
            plan: Mutex::new(None),
            busy: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicU64::new(0)),
        });
        if let Some(mut report) = this
            .history()?
            .into_iter()
            .find(|r| r.finished_at.is_none())
        {
            report.status = "uncertain".into();
            this.persist(&report)?;
            let mut state = this.state.lock().unwrap();
            state.status = "uncertain".into();
            state.report = Some(report);
            state.message =
                "上次 Windows 维护未确认结束，请先重新估算核对；不会自动重试删除。".into();
        }
        Ok(this)
    }
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
    pub fn snapshot(&self) -> WindowsSnapshot {
        let mut snapshot = self.state.lock().unwrap().clone();
        snapshot.progress_bytes = self
            .is_busy()
            .then(|| self.progress.load(Ordering::Acquire).to_string());
        snapshot
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        if self.is_busy() {
            self.state.lock().unwrap().status = "stopping".into();
        }
    }
    fn begin(&self) -> Result<()> {
        ensure!(
            self.busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "Windows 维护任务进行中"
        );
        self.cancel.store(false, Ordering::Release);
        self.progress.store(0, Ordering::Release);
        Ok(())
    }
    fn spawn(self: &Arc<Self>, work: impl FnOnce(&Self) -> Result<()> + Send + 'static) {
        let this = self.clone();
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&this)))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Windows 维护异常中断")));
            if let Err(error) = outcome {
                let mut state = this.state.lock().unwrap();
                state.status = if state
                    .report
                    .as_ref()
                    .is_some_and(|r| r.finished_at.is_none())
                {
                    "uncertain"
                } else if this.cancel.load(Ordering::Acquire) {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                state.message = format!("{error:#}");
                state.plan = None;
            }
            this.busy.store(false, Ordering::Release);
        });
    }
    pub fn scan(self: &Arc<Self>, exclusions: Vec<String>) -> Result<String> {
        self.begin()?;
        self.plan.lock().unwrap().take();
        let session = Uuid::new_v4().to_string();
        *self.state.lock().unwrap() = WindowsSnapshot {
            session: session.clone(),
            status: "scanning".into(),
            actions: vec![],
            plan: None,
            report: None,
            current: None,
            message: String::new(),
            progress_bytes: None,
        };
        self.spawn(move |this| {
            let actions = probe(
                this.scope,
                &exclusions,
                this.cancel.clone(),
                this.progress.clone(),
            );
            let mut state = this.state.lock().unwrap();
            state.actions = actions;
            state.status = if this.cancel.load(Ordering::Acquire) {
                "cancelled"
            } else {
                "ready"
            }
            .into();
            Ok(())
        });
        Ok(session)
    }
    pub fn prepare(
        self: &Arc<Self>,
        session: String,
        ids: Vec<String>,
        exclusions: Vec<String>,
    ) -> Result<()> {
        ensure!(
            !ids.is_empty() && ids.len() <= SPECS.len(),
            "请选择 Windows 维护项目"
        );
        ensure!(
            ids.iter().collect::<std::collections::HashSet<_>>().len() == ids.len(),
            "维护项目重复"
        );
        ensure!(
            ids.iter().all(|id| !excluded(id, &exclusions)),
            "Windows 维护不能遵守当前路径或类别排除"
        );
        let state = self.snapshot();
        ensure!(
            state.session == session && matches!(state.status.as_str(), "ready" | "prepared"),
            "请重新估算 Windows 维护项目"
        );
        for id in &ids {
            ensure!(
                spec(id)?.scope == self.scope && accepted(id),
                "此处理器尚未开放清理"
            );
            ensure!(
                state.actions.iter().any(|r| &r.id == id && r.available),
                "维护项目未扫描或不可用"
            );
        }
        self.begin()?;
        self.plan.lock().unwrap().take();
        self.state.lock().unwrap().status = "preparing".into();
        self.spawn(move |this| {
            let mut actions = vec![];
            let mut fingerprints = vec![];
            for id in ids {
                ensure!(!this.cancel.load(Ordering::Acquire), "准备已取消");
                let mut action = state.actions.iter().find(|r| r.id == id).unwrap().clone();
                let handler = Handler::open(
                    spec(&id)?,
                    false,
                    this.cancel.clone(),
                    this.progress.clone(),
                )?;
                action.estimate_bytes = Some(
                    handler
                        .estimate()?
                        .context("Windows 未提供可靠估算")?
                        .to_string(),
                );
                ensure!(
                    action.estimate_bytes.as_deref() != Some("0"),
                    "本项已没有可清理内容，请重新估算"
                );
                fingerprints.push(handler.fingerprint.clone());
                actions.push(action);
            }
            let public = WindowsPlan {
                id: Uuid::new_v4().to_string(),
                session,
                actions,
                expires_at: now() + 300,
            };
            *this.plan.lock().unwrap() = Some(Prepared {
                public: public.clone(),
                fingerprints,
                exclusions,
            });
            let mut state = this.state.lock().unwrap();
            state.plan = Some(public);
            state.status = "prepared".into();
            Ok(())
        });
        Ok(())
    }
    pub fn execute(self: &Arc<Self>, plan_id: String, exclusions: Vec<String>) -> Result<String> {
        let mut slot = self.plan.lock().unwrap();
        let prepared = slot.as_ref().context("维护计划已失效")?;
        ensure!(
            prepared.public.id == plan_id
                && prepared.public.expires_at >= now()
                && prepared.exclusions == exclusions,
            "维护计划已变化，请重新确认"
        );
        self.begin()?;
        let plan = slot.take().unwrap();
        drop(slot);
        let id = Uuid::new_v4().to_string();
        let task = id.clone();
        {
            let mut state = self.state.lock().unwrap();
            state.plan = None;
            state.status = "running".into();
        }
        self.spawn(move |this| {
            // Revalidate the entire batch before any purge. No partial execution on scope/estimate changes.
            for (action, fingerprint) in plan.public.actions.iter().zip(&plan.fingerprints) {
                let handler = Handler::open(
                    spec(&action.id)?,
                    false,
                    this.cancel.clone(),
                    this.progress.clone(),
                )?;
                ensure!(
                    &handler.fingerprint == fingerprint,
                    "处理器或范围发生变化，请重新估算"
                );
                let estimate = handler.estimate()?.context("估算未知，未执行维护")?;
                ensure!(
                    estimate
                        <= action
                            .estimate_bytes
                            .as_deref()
                            .unwrap_or("0")
                            .parse::<u64>()?,
                    "估算增加，请重新估算并确认"
                );
            }
            let mut report = WindowsReport {
                id: task,
                scope: this.scope,
                started_at: now(),
                finished_at: None,
                status: "running".into(),
                actions: vec![],
            };
            this.persist(&report)?;
            this.state.lock().unwrap().report = Some(report.clone());
            for (action, fingerprint) in plan.public.actions.iter().zip(&plan.fingerprints) {
                if this.cancel.load(Ordering::Acquire) {
                    break;
                }
                let mut row = WindowsOutcome {
                    id: action.id.clone(),
                    title: action.title.clone(),
                    status: "running".into(),
                    message: String::new(),
                    before_bytes: None,
                    after_bytes: None,
                    free_before: None,
                    free_after: None,
                };
                let outcome = (|| -> Result<()> {
                    let handler = Handler::open(
                        spec(&action.id)?,
                        true,
                        this.cancel.clone(),
                        this.progress.clone(),
                    )?;
                    ensure!(&handler.fingerprint == fingerprint, "处理器或范围发生变化");
                    let bytes = handler.estimate()?.context("估算未知")?;
                    ensure!(
                        bytes
                            <= action
                                .estimate_bytes
                                .as_deref()
                                .unwrap_or("0")
                                .parse::<u64>()?,
                        "估算增加，需重新确认"
                    );
                    row.before_bytes = Some(bytes.to_string());
                    row.free_before = free(&handler.volume);
                    report.actions.push(row.clone());
                    this.persist(&report)?;
                    {
                        let mut state = this.state.lock().unwrap();
                        state.report = Some(report.clone());
                        state.current = Some(action.title.clone());
                    }
                    let purged = handler.purge(bytes);
                    row.free_after = free(&handler.volume);
                    drop(handler);
                    row.after_bytes = Handler::open(
                        spec(&action.id)?,
                        false,
                        this.cancel.clone(),
                        this.progress.clone(),
                    )
                    .and_then(|h| h.estimate())
                    .ok()
                    .flatten()
                    .map(|b| b.to_string());
                    purged?;
                    row.status = "completed".into();
                    row.message = "Windows 已完成该项维护；估算变化不代表实际删除字节数。".into();
                    Ok(())
                })();
                if let Err(error) = outcome {
                    row.status = if this.cancel.load(Ordering::Acquire) {
                        "cancelled"
                    } else {
                        "failed"
                    }
                    .into();
                    row.message = format!("{error:#}");
                }
                if report.actions.last().is_some_and(|r| r.id == row.id) {
                    report.actions.pop();
                }
                report.actions.push(row);
                this.persist(&report)?;
                this.state.lock().unwrap().report = Some(report.clone());
            }
            report.status = if this.cancel.load(Ordering::Acquire) {
                "cancelled"
            } else if report.actions.iter().all(|r| r.status == "completed") {
                "completed"
            } else {
                "failed"
            }
            .into();
            report.finished_at = Some(now());
            this.persist(&report)?;
            let mut state = this.state.lock().unwrap();
            state.status = report.status.clone();
            state.report = Some(report);
            state.current = None;
            Ok(())
        });
        Ok(id)
    }
    pub fn validate_plan(&self, id: &str, exclusions: &[String]) -> Result<()> {
        let slot = self.plan.lock().unwrap();
        let plan = slot.as_ref().context("Windows 维护计划已失效")?;
        ensure!(
            !self.is_busy()
                && plan.public.id == id
                && plan.public.expires_at >= now()
                && plan.exclusions == exclusions,
            "Windows 维护计划已变化"
        );
        Ok(())
    }
    fn persist(&self, report: &WindowsReport) -> Result<()> {
        Uuid::parse_str(&report.id)?;
        let _root = PinnedPath::open(&self.directory, true, false)?;
        let path = self.directory.join(format!("{}.json", report.id));
        if path.exists() {
            let _verified = PinnedPath::open(&path, false, false)?;
        }
        let pending = self.directory.join(format!("{}.pending", Uuid::new_v4()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        file.write_all(&serde_json::to_vec(report)?)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&pending, &path)?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<WindowsReport>> {
        let _root = PinnedPath::open(&self.directory, true, false)?;
        let mut reports = vec![];
        for entry in std::fs::read_dir(&self.directory)? {
            let path = entry?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let _pin = PinnedPath::open(&path, false, false)?;
            let report: WindowsReport = serde_json::from_slice(&std::fs::read(&path)?)?;
            ensure!(report.scope == self.scope, "历史权限域不匹配");
            reports.push(report);
        }
        reports.sort_by_key(|r| std::cmp::Reverse(r.started_at));
        Ok(reports)
    }
}
fn free(volume: &str) -> Option<String> {
    let mut available = 0;
    unsafe {
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            windows::core::PCWSTR(text_wide(volume).as_ptr()),
            Some(&mut available),
            None,
            None,
        )
        .ok()?;
    }
    Some(available.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn isolated_acceptance() -> bool {
        std::env::var("DUSTAWAY_SYSTEM_ACCEPTANCE").as_deref()==Ok("20261003")
            && std::env::var("COMPUTERNAME").as_deref()==Ok("DESKTOP-24NJTT6")
            && std::fs::read_to_string(r"C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261003-system-cleanup\.dustaway-test").is_ok_and(|text|text=="DustAway.SystemCleanup.20261003")
    }
    fn wait(engine: &WindowsMaintenance) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1800);
        while engine.is_busy() {
            assert!(
                std::time::Instant::now() < deadline,
                "Windows operation timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    #[test]
    #[ignore = "Destructive official maintenance; explicitly authorized isolated Windows machine only"]
    fn remote_official_acceptance() {
        assert!(isolated_acceptance(), "Isolated machine marker required");
        let scope = if crate::security::elevated().unwrap() {
            Scope::System
        } else {
            Scope::User
        };
        let directory =
            PathBuf::from(r"C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261003-system-cleanup")
                .join(if scope == Scope::User {
                    "user-receipts"
                } else {
                    "system-receipts"
                });
        let engine = WindowsMaintenance::new(scope, &directory).unwrap();
        let ids = std::env::var("DUSTAWAY_ACCEPTANCE_IDS")
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for id in ids {
            assert_eq!(spec(&id).unwrap().scope, scope);
            let session = engine.scan(vec![]).unwrap();
            wait(&engine);
            let snapshot = engine.snapshot();
            println!("SCAN {}", serde_json::to_string(&snapshot).unwrap());
            assert!(
                snapshot.actions.iter().any(|a| a.id == id && a.available),
                "No positive official estimate for {id}"
            );
            engine.prepare(session, vec![id.clone()], vec![]).unwrap();
            wait(&engine);
            let plan = engine.snapshot().plan.expect("Prepared official plan");
            engine.execute(plan.id, vec![]).unwrap();
            wait(&engine);
            let result = engine.snapshot();
            println!("RECEIPT {}", serde_json::to_string(&result).unwrap());
            assert_eq!(result.status, "completed", "Official operation {id} failed");
            let report = result.report.unwrap();
            assert_eq!(report.actions.len(), 1);
            assert_eq!(report.actions[0].id, id);
        }
    }
    #[test]
    fn interrupted_intent_recovers_as_uncertain_without_execution() {
        let temp = tempfile::tempdir().unwrap();
        let engine = WindowsMaintenance::new(Scope::User, temp.path()).unwrap();
        let report = WindowsReport {
            id: Uuid::new_v4().to_string(),
            scope: Scope::User,
            started_at: now(),
            finished_at: None,
            status: "running".into(),
            actions: vec![],
        };
        engine.persist(&report).unwrap();
        drop(engine);
        let restored = WindowsMaintenance::new(Scope::User, temp.path()).unwrap();
        assert_eq!(restored.snapshot().status, "uncertain");
        assert!(!restored.is_busy());
        assert!(restored.validate_plan("fabricated", &[]).is_err());
        assert_eq!(restored.history().unwrap()[0].status, "uncertain");
    }
    #[test]
    fn exclusions_and_unknown_operations_cannot_prepare() {
        let temp = tempfile::tempdir().unwrap();
        let engine = WindowsMaintenance::new(Scope::User, temp.path()).unwrap();
        assert!(
            engine
                .prepare(String::new(), vec!["arbitrary.exe".into()], vec![])
                .is_err()
        );
        let session = engine.scan(vec![r"C:\keep".into()]).unwrap();
        wait(&engine);
        assert!(
            engine
                .snapshot()
                .actions
                .iter()
                .all(|a| !a.available && a.estimate_bytes.is_none() && a.message.contains("排除"))
        );
        assert!(
            engine
                .prepare(
                    session,
                    vec!["windows.thumbnail".into()],
                    vec![r"C:\keep".into()]
                )
                .is_err()
        );
    }
    #[test]
    fn category_exclusion_blocks_only_overlapping_handler() {
        let settings = dustaway_core::Settings {
            excluded_rules: vec!["windows.directx_cache".into()],
            ..Default::default()
        };
        let bound = settings_exclusions(&settings);
        assert!(excluded("windows.shader", &bound));
        assert!(!excluded("windows.thumbnail", &bound));
        assert!(excluded("windows.thumbnail", &[r"C:\keep".into()]));
    }
}
