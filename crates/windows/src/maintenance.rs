//! Fixed official maintenance with durable intent; no user-supplied command or path.
use crate::{
    fs::*,
    maintenance_tools::{Action, Tool},
    uv_cache::{inventory, run_bounded, validate_exclusions},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct MaintenancePlan {
    pub rule_id: String,
    pub title: String,
    pub id: String,
    pub session: String,
    pub cache: String,
    pub executable: String,
    pub expires_at: i64,
}
struct Prepared {
    public: MaintenancePlan,
    root: PathBuf,
    tool: Tool,
    entry_stamp: Option<Stamp>,
    root_identity: Identity,
    root_parents: Vec<Identity>,
    exe_stamp: Stamp,
    exclusions: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct MaintenanceReport {
    pub rule_id: String,
    pub title: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: String,
    pub cache: String,
    pub command: String,
    pub message: String,
    pub output: String,
    pub before_bytes: Option<String>,
    pub after_bytes: Option<String>,
    pub free_before: Option<String>,
    pub free_after: Option<String>,
}
pub struct Maintenance {
    directory: PathBuf,
    plan: Mutex<Option<Prepared>>,
    task: Mutex<Option<MaintenanceReport>>,
    busy: AtomicBool,
    cancel: AtomicBool,
    preparation: Mutex<Option<(String, Arc<AtomicBool>)>>,
    cancelled_preparations: Mutex<std::collections::VecDeque<String>>,
}
impl Maintenance {
    pub fn new(data: &Path) -> Self {
        Self {
            directory: data.join("tool-cache-history"),
            plan: Mutex::new(None),
            task: Mutex::new(None),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            preparation: Mutex::new(None),
            cancelled_preparations: Mutex::new(std::collections::VecDeque::new()),
        }
    }
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
    pub fn invalidate_plan(&self) {
        self.plan.lock().unwrap().take();
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub fn task(&self) -> Option<MaintenanceReport> {
        self.task.lock().unwrap().clone()
    }
    pub fn cancel_preparation(&self, id: &str) -> Result<()> {
        ensure!(uuid::Uuid::parse_str(id).is_ok(), "检查标识无效");
        let active = self.preparation.lock().unwrap();
        if let Some((current, cancel)) = active.as_ref().filter(|(current, _)| current == id) {
            let _ = current;
            cancel.store(true, Ordering::Release);
        } else {
            let mut cancelled = self.cancelled_preparations.lock().unwrap();
            if cancelled.len() >= 128 {
                cancelled.pop_front();
            }
            cancelled.push_back(id.into());
        }
        Ok(())
    }
    pub fn preparation_cancelled(&self, id: &str) -> bool {
        let active = self.preparation.lock().unwrap();
        active
            .as_ref()
            .is_some_and(|(key, cancel)| key == id && cancel.load(Ordering::Acquire))
            || self
                .cancelled_preparations
                .lock()
                .unwrap()
                .iter()
                .any(|key| key == id)
    }
    pub fn prepare_cancellable(
        &self,
        id: &str,
        session: &str,
        rule_id: &str,
        roots: &[String],
        exclusions: Vec<String>,
    ) -> Result<MaintenancePlan> {
        self.prepare_scanned_cancellable(id, session, rule_id, roots, exclusions, None)
    }
    pub fn prepare_scanned_cancellable(
        &self,
        id: &str,
        session: &str,
        rule_id: &str,
        roots: &[String],
        exclusions: Vec<String>,
        expected: Option<&dustaway_core::ToolDiscovery>,
    ) -> Result<MaintenancePlan> {
        ensure!(uuid::Uuid::parse_str(id).is_ok(), "检查标识无效");
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.preparation.lock().unwrap();
            ensure!(active.is_none(), "检查进行中");
            let mut cancelled = self.cancelled_preparations.lock().unwrap();
            if let Some(index) = cancelled.iter().position(|v| v == id) {
                cancelled.remove(index);
                anyhow::bail!("检查已取消");
            }
            *active = Some((id.into(), cancel.clone()));
        }
        let result = (|| {
            if let Some(expected) = expected {
                crate::tool_discovery::verify(rule_id, expected, &cancel)?;
            }
            let plan = self.prepare(session, rule_id, roots, exclusions, &cancel)?;
            if let Some(expected) = expected {
                ensure!(
                    expected.target.as_deref() == Some(&plan.cache)
                        && expected.executable.as_deref() == Some(&plan.executable),
                    "工具或位置已变化，请重新检查这一项"
                );
                crate::tool_discovery::verify(rule_id, expected, &cancel)?;
            }
            Ok(plan)
        })();
        if result.is_err() {
            self.invalidate_plan();
        }
        self.preparation.lock().unwrap().take();
        if cancel.load(Ordering::Acquire) {
            self.invalidate_plan();
            anyhow::bail!("检查已取消");
        }
        result
    }
    fn prepare(
        &self,
        session: &str,
        rule_id: &str,
        scanned_roots: &[String],
        exclusions: Vec<String>,
        cancel: &AtomicBool,
    ) -> Result<MaintenancePlan> {
        ensure!(!self.is_busy(), "官方整理正在进行");
        self.plan.lock().unwrap().take();
        check_cancel(Some(cancel))?;
        let action = Action::parse(rule_id)?;
        ensure!(
            dustaway_core::cleanup_evidence::documented_tool(rule_id),
            dustaway_core::ApiError::new(
                "adaptation_pending",
                "此工具操作尚未通过当前布局的保护验收，缓存保持只读"
            )
        );

        let first = scanned_roots.first().context("没有已扫描缓存位置")?;
        let initial = extended(Path::new(first))?;
        PinnedPath::open(&initial, true, false)?;
        let tool = Tool::discover(
            action,
            &crate::platform::known_folder(&windows::Win32::UI::Shell::FOLDERID_Profile)?,
            cancel,
        )?;
        crate::tool_process::guard(rule_id, Some(&tool.exe), tool.entry.as_deref(), cancel)?;
        let root = tool.target(
            &crate::platform::known_folder(&windows::Win32::UI::Shell::FOLDERID_Profile)?,
            cancel,
        )?;
        ensure!(
            scanned_roots
                .iter()
                .any(|p| extended(Path::new(p)).is_ok_and(|p| p == root)),
            dustaway_core::ApiError::new(
                "target_mismatch",
                "工具实际缓存与扫描位置不一致，请重新扫描；不会更换删除目标"
            )
        );
        validate_exclusions(&root, &exclusions)?;
        let root_pin = PinnedPath::open(&root, true, false)?;
        tool.validate_layout(&root, cancel)?;
        // Refuse partial reads/reparse/cloud layouts before offering confirmation.
        inventory(&root, cancel, Duration::from_secs(30))?;
        let exe_pin = PinnedPath::open(&tool.exe, false, false)?;
        let entry_stamp = tool
            .entry
            .as_ref()
            .map(|p| PinnedPath::open(p, false, false).map(|p| p.stamp))
            .transpose()?;
        let public = MaintenancePlan {
            rule_id: rule_id.into(),
            title: action.title().into(),
            id: uuid::Uuid::new_v4().to_string(),
            session: session.into(),
            cache: display_path(&root),
            executable: display_path(&tool.exe),
            expires_at: dustaway_core::now() + 120,
        };
        *self.plan.lock().unwrap() = Some(Prepared {
            public: public.clone(),
            root,
            tool,
            entry_stamp,
            root_identity: root_pin.stamp.identity,
            root_parents: root_pin.parent_ids,
            exe_stamp: exe_pin.stamp,
            exclusions,
        });
        Ok(public)
    }
    pub fn start(
        self: &Arc<Self>,
        id: &str,
        session: &str,
        exclusions: &[String],
    ) -> Result<String> {
        ensure!(!self.is_busy(), "官方整理正在进行");
        let prepared = self
            .plan
            .lock()
            .unwrap()
            .take()
            .context("确认已失效，请重新检查；不会续接旧操作")?;
        ensure!(
            prepared.public.id == id
                && prepared.public.session == session
                && prepared.public.expires_at >= dustaway_core::now()
                && prepared.exclusions == exclusions,
            "确认已失效，请重新检查"
        );
        let report = MaintenanceReport {
            rule_id: prepared.public.rule_id.clone(),
            title: prepared.public.title.clone(),
            session: prepared.public.session.clone(),
            reason: None,
            id: prepared.public.id.clone(),
            started_at: dustaway_core::now(),
            finished_at: None,
            status: "running".into(),
            cache: prepared.public.cache.clone(),
            command: prepared.tool.action.command_label().into(),
            message: "正在由官方工具处理缓存".into(),
            output: String::new(),
            before_bytes: None,
            after_bytes: None,
            free_before: None,
            free_after: None,
        };
        self.persist(&report, true)?; // Durable intent precedes launch. Never replayed on restart.
        *self.task.lock().unwrap() = Some(report.clone());
        self.cancel.store(false, Ordering::Release);
        self.busy.store(true, Ordering::Release);
        let this = self.clone();
        let result_id = report.id.clone();
        std::thread::spawn(move || {
            let mut report = report;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                this.run(&prepared, &mut report)
            }));
            if let Err(error) =
                result.unwrap_or_else(|_| Err(anyhow::anyhow!("整理异常中断，请核对缓存")))
            {
                report.status = if this.cancel.load(Ordering::Acquire) {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                report.reason = Some(
                    if report.status == "cancelled" {
                        "cancelled"
                    } else {
                        error
                            .downcast_ref::<dustaway_core::ApiError>()
                            .map_or("execution_failed", |e| e.code)
                    }
                    .into(),
                );
                report.message =
                    format!("{error:#}；已处理内容不会恢复，不会自动重试，可重新检查这一项核对");
            }
            report.finished_at = Some(dustaway_core::now());
            if let Err(error) = this.persist(&report, false) {
                report.message.push_str(&format!("；结果保存失败：{error}"));
            }
            let mut task = this.task.lock().unwrap();
            this.busy.store(false, Ordering::Release);
            *task = Some(report);
        });
        Ok(result_id)
    }
    fn run(&self, plan: &Prepared, report: &mut MaintenanceReport) -> Result<()> {
        check_cancel(Some(&self.cancel))?;
        validate_exclusions(&plan.root, &plan.exclusions)?;
        let root_pin = PinnedPath::open(&plan.root, true, false)?;
        ensure!(
            root_pin.stamp.identity == plan.root_identity
                && root_pin.parent_ids == plan.root_parents,
            "缓存目录已更换，未执行"
        );
        let exe_pin = PinnedPath::open(&plan.tool.exe, false, false)?;
        ensure!(exe_pin.stamp == plan.exe_stamp, "工具已更换，未执行");
        let entry_pin = plan
            .tool
            .entry
            .as_ref()
            .map(|p| PinnedPath::open(p, false, false))
            .transpose()?;
        ensure!(
            entry_pin.as_ref().map(|p| &p.stamp) == plan.entry_stamp.as_ref(),
            "工具入口已更换，未执行"
        );
        crate::tool_process::guard(
            &plan.public.rule_id,
            Some(&plan.tool.exe),
            plan.tool.entry.as_deref(),
            &self.cancel,
        )?;
        ensure!(
            plan.tool.target(
                &crate::platform::known_folder(&windows::Win32::UI::Shell::FOLDERID_Profile)?,
                &self.cancel
            )? == plan.root,
            dustaway_core::ApiError::new("target_changed", "缓存配置改变，未执行")
        );
        plan.tool.validate_layout(&plan.root, &self.cancel)?;
        let before = inventory(&plan.root, &self.cancel, Duration::from_secs(120))?;
        report.before_bytes = Some(
            before
                .values()
                .map(|v| *v as u128)
                .sum::<u128>()
                .to_string(),
        );
        report.free_before = free_bytes(&plan.root).ok().map(|n| n.to_string());
        check_cancel(Some(&self.cancel))?;
        let (ok, output) = run_bounded(
            &mut plan.tool.cleanup(&plan.root),
            &self.cancel,
            Duration::from_secs(120),
        )?;
        report.output = output;
        report.free_after = free_bytes(&plan.root).ok().map(|n| n.to_string());
        let after = inventory(&plan.root, &self.cancel, Duration::from_secs(120)).ok();
        report.after_bytes = after
            .as_ref()
            .map(|m| m.values().map(|v| *v as u128).sum::<u128>().to_string());
        ensure!(ok, "工具返回失败，请查看输出并检查这一项核对");
        report.status = "completed".into();
        report.message = if after.as_ref() == Some(&before) {
            "未观察到缓存变化；工具已完成，缓存文件清单未变化。"
        } else if after.is_none() {
            "工具已完成，整理后容量未取得，可检查这一项核对。"
        } else {
            "工具处理完成，请核对缓存逻辑大小和磁盘读数；二者不代表相同的释放量。"
        }
        .into();
        Ok(())
    }
    fn persist(&self, report: &MaintenanceReport, pending: bool) -> Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let _pin = PinnedPath::open(&self.directory, true, false)?;
        let path = self.directory.join(format!(
            "{}{}.json",
            report.id,
            if pending { ".pending" } else { "" }
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&serde_json::to_vec_pretty(report)?)?;
        file.sync_all()?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<MaintenanceReport>> {
        if !self.directory.try_exists()? {
            return Ok(vec![]);
        }
        let _pin = PinnedPath::open(&self.directory, true, false)?;
        let mut rows = BTreeMap::new();
        for entry in std::fs::read_dir(&self.directory)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let _file = PinnedPath::observe(&path, false, None)?;
            ensure!(_file.stamp.size <= 256 * 1024, "工具记录过大");
            let mut row: MaintenanceReport = serde_json::from_slice(&std::fs::read(&path)?)?;
            let pending = path.to_string_lossy().ends_with(".pending.json");
            if pending {
                if self.task().is_some_and(|t| t.id == row.id) {
                    continue;
                }
                row.status = "interrupted".into();
                row.message = "上次操作结果未确认，不会继续或重复执行；请重新扫描核对".into();
                rows.entry(row.id.clone()).or_insert(row);
            } else {
                rows.insert(row.id.clone(), row);
            }
        }
        if let Some(task) = self.task() {
            rows.insert(task.id.clone(), task);
        }
        let mut rows: Vec<_> = rows.into_values().collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.started_at));
        Ok(rows)
    }
    pub fn clear_history(&self) -> Result<()> {
        ensure!(!self.is_busy(), "官方整理进行中");
        if !self.directory.try_exists()? {
            return Ok(());
        }
        let _pin = PinnedPath::open(&self.directory, true, false)?;
        for entry in std::fs::read_dir(&self.directory)? {
            let path = entry?.path();
            let name = path.file_name().unwrap().to_string_lossy();
            let id = name
                .strip_suffix(".pending.json")
                .or_else(|| name.strip_suffix(".json"));
            if id.is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()) {
                PinnedPath::open(&path, false, true)?.mark_delete()?;
            }
        }
        self.task.lock().unwrap().take();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_before_preparation_does_not_discover_or_execute_a_tool() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Maintenance::new(temp.path());
        let id = uuid::Uuid::new_v4().to_string();
        manager.cancel_preparation(&id).unwrap();
        let error = manager
            .prepare_cancellable(&id, "scan", "dev.go_build", &[], vec![])
            .err()
            .unwrap();
        assert!(error.to_string().contains("取消"));
        assert!(manager.task().is_none());
        assert!(manager.history().unwrap().is_empty());
    }
    #[test]
    fn unfinished_intent_is_not_resumed_and_old_plan_cannot_be_replayed() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Arc::new(Maintenance::new(temp.path()));
        let report = MaintenanceReport {
            rule_id: "dev.go_build".into(),
            title: "test".into(),
            session: "old".into(),
            reason: None,
            id: uuid::Uuid::new_v4().to_string(),
            started_at: 1,
            finished_at: None,
            status: "running".into(),
            cache: "test".into(),
            command: "go clean -cache".into(),
            message: String::new(),
            output: String::new(),
            before_bytes: None,
            after_bytes: None,
            free_before: None,
            free_after: None,
        };
        manager.persist(&report, true).unwrap();
        assert_eq!(manager.history().unwrap()[0].status, "interrupted");
        assert!(manager.start(&report.id, "old", &[]).is_err());
        assert!(!manager.is_busy());
        assert!(manager.persist(&report, true).is_err());
    }
    #[test]
    fn confirmation_is_bound_to_session_expiry_exclusions_and_tool() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Arc::new(Maintenance::new(temp.path()));
        let root_pin = PinnedPath::open(temp.path(), true, false).unwrap();
        let exe = std::env::current_exe().unwrap();
        let exe_pin = PinnedPath::open(&exe, false, false).unwrap();
        *manager.plan.lock().unwrap() = Some(Prepared {
            public: MaintenancePlan {
                rule_id: "dev.go_build".into(),
                title: "test".into(),
                id: "one-use".into(),
                session: "scan".into(),
                cache: display_path(temp.path()),
                executable: display_path(&exe),
                expires_at: 0,
            },
            root: temp.path().into(),
            tool: Tool {
                action: Action::Go,
                exe,
                entry: None,
                version: "test".into(),
            },
            entry_stamp: None,
            root_identity: root_pin.stamp.identity,
            root_parents: root_pin.parent_ids,
            exe_stamp: exe_pin.stamp,
            exclusions: vec![],
        });
        assert!(manager.start("one-use", "scan", &[]).is_err());
        assert!(manager.start("one-use", "scan", &[]).is_err());
        assert!(manager.history().unwrap().is_empty());
    }
}
