//! A fixed uv cache-prune operation. No paths or command arguments are accepted from IPC.
use crate::{fs::*, platform::known_folder};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, FOLDERID_Profile};

pub(crate) fn cache_root(local: &Path, roaming: &Path) -> Result<PathBuf> {
    #[cfg(not(test))]
    if let Some(value) = std::env::var_os("UV_CACHE_DIR").filter(|s| !s.is_empty()) {
        return checked_cache_path(Path::new(&value));
    }
    let config = roaming.join("uv/uv.toml");
    if config.try_exists()? {
        let pin = PinnedPath::observe(&config, false, None)?;
        ensure!(
            pin.stamp.size <= 1024 * 1024,
            "uv 配置过大，无法确认缓存位置"
        );
        let value: toml::Value = toml::from_str(&std::fs::read_to_string(&config)?)?;
        if let Some(value) = value.get("cache-dir") {
            let path = value
                .as_str()
                .context("uv cache-dir 配置不是有效路径，已保留")?;
            return checked_cache_path(Path::new(path));
        }
    }
    checked_cache_path(&local.join("uv/cache"))
}
fn checked_cache_path(path: &Path) -> Result<PathBuf> {
    let path = extended(path)?;
    ensure!(
        path.components().count() >= 3,
        "uv 缓存目录过于宽泛，已保留"
    );
    Ok(path)
}

pub(crate) fn discover(
    home: &Path,
    local: &Path,
    cancel: &AtomicBool,
) -> Result<(PathBuf, String, PathBuf)> {
    let exe = [
        home.join(".local/bin/uv.exe"),
        home.join(".cargo/bin/uv.exe"),
        local.join("Programs/uv/uv.exe"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .ok_or_else(|| {
        dustaway_core::ApiError::new("tool_missing", "未找到标准位置的 uv.exe；不会自动安装工具")
    })?;
    let identity = crate::tool_discovery::identity(&exe, None)?;
    let probe = |args: &[&str]| -> Result<String> {
        let mut command = Command::new(&exe);
        command
            .args(args)
            .current_dir(home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000);
        let (ok, output) = run_bounded(&mut command, cancel, Duration::from_secs(10))?;
        ensure!(ok, "uv 只读查询失败：{output}");
        Ok(output.trim().to_owned())
    };
    let version = probe(&["--version"])?;
    ensure!(
        version.starts_with("uv ")
            && version.split_whitespace().nth(1).is_some_and(|v| v
                .split('.')
                .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))),
        "无法确认 uv 版本"
    );
    let output = probe(&["cache", "dir", "--offline"])?;
    ensure!(
        output.lines().count() == 1 && Path::new(&output).is_absolute(),
        "uv 未返回唯一绝对缓存位置"
    );
    let root = checked_cache_path(Path::new(&output))?;
    ensure!(
        identity == crate::tool_discovery::identity(&exe, None)?,
        "uv 程序在查询期间变化"
    );
    Ok((exe, version, root))
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UvPlan {
    pub id: String,
    pub session: String,
    pub cache: String,
    pub executable: String,
    pub expires_at: i64,
}
struct Prepared {
    public: UvPlan,
    root: PathBuf,
    exe: PathBuf,
    root_identity: Identity,
    root_parents: Vec<Identity>,
    exe_stamp: Stamp,
    exclusions: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct UvReport {
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
pub struct UvCache {
    directory: PathBuf,
    plan: Mutex<Option<Prepared>>,
    task: Mutex<Option<UvReport>>,
    task_session: Mutex<String>,
    busy: AtomicBool,
    cancel: AtomicBool,
    preparation: Mutex<Option<(String, Arc<AtomicBool>)>>,
    cancelled_preparations: Mutex<std::collections::VecDeque<String>>,
}
impl UvCache {
    pub fn new(data: &Path) -> Self {
        Self {
            directory: data.join("uv-cache-history"),
            plan: Mutex::new(None),
            task: Mutex::new(None),
            task_session: Mutex::new(String::new()),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            preparation: Mutex::new(None),
            cancelled_preparations: Mutex::new(Default::default()),
        }
    }
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub fn task_session(&self) -> String {
        self.task_session.lock().unwrap().clone()
    }
    pub fn task(&self) -> Option<UvReport> {
        self.task.lock().unwrap().clone()
    }
    pub fn invalidate_plan(&self) {
        self.plan.lock().unwrap().take();
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
    pub fn prepare_scanned_cancellable(
        &self,
        id: &str,
        session: &str,
        scanned_root: &str,
        exclusions: Vec<String>,
        expected: &dustaway_core::ToolDiscovery,
    ) -> Result<UvPlan> {
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
            crate::tool_discovery::verify("dev.uv", expected, &cancel)?;
            let plan = self.prepare_with_cancel(session, scanned_root, exclusions, &cancel)?;
            ensure!(
                expected.target.as_deref() == Some(&plan.cache)
                    && expected.executable.as_deref() == Some(&plan.executable),
                "工具或位置已变化，请重新检查这一项"
            );
            crate::tool_discovery::verify("dev.uv", expected, &cancel)?;
            Ok(plan)
        })();
        self.preparation.lock().unwrap().take();
        if cancel.load(Ordering::Acquire) || result.is_err() {
            self.invalidate_plan();
        }
        check_cancel(Some(&cancel))?;
        result
    }
    pub fn prepare(
        &self,
        session: &str,
        scanned_root: &str,
        exclusions: Vec<String>,
    ) -> Result<UvPlan> {
        self.prepare_with_cancel(session, scanned_root, exclusions, &AtomicBool::new(false))
    }
    fn prepare_with_cancel(
        &self,
        session: &str,
        scanned_root: &str,
        exclusions: Vec<String>,
        cancel: &AtomicBool,
    ) -> Result<UvPlan> {
        ensure!(!self.is_busy(), "uv 整理正在进行");
        self.plan.lock().unwrap().take();
        let local = known_folder(&FOLDERID_LocalAppData)?;
        let home = known_folder(&FOLDERID_Profile)?;
        let (exe, _, root) = discover(&home, &local, cancel)?;
        ensure!(
            extended(Path::new(scanned_root))? == root,
            "uv 缓存位置已经变化，请重新扫描"
        );
        validate_exclusions(&root, &exclusions)?;
        let root_pin = validate_root(&root)?;
        let exe_pin = PinnedPath::open(&exe, false, false)?;
        ensure!(exe_pin.stamp.size > 1024, "uv 程序无法确认");
        crate::tool_process::guard("dev.uv", Some(&exe), None, cancel)?;
        let public = UvPlan {
            id: uuid::Uuid::new_v4().to_string(),
            session: session.into(),
            cache: display_path(&root),
            executable: display_path(&exe),
            expires_at: dustaway_core::now() + 120,
        };
        *self.plan.lock().unwrap() = Some(Prepared {
            public: public.clone(),
            root,
            exe,
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
        ensure!(!self.is_busy(), "uv 整理正在进行");
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
        let report = UvReport {
            id: prepared.public.id.clone(),
            started_at: dustaway_core::now(),
            finished_at: None,
            status: "running".into(),
            cache: prepared.public.cache.clone(),
            command: "uv cache prune --offline --no-config --cache-dir <已确认缓存>".into(),
            message: "正在由 uv 整理闲置缓存".into(),
            output: String::new(),
            before_bytes: None,
            after_bytes: None,
            free_before: None,
            free_after: None,
        };
        self.persist(&report, true)?; // Durable intent precedes launch. Never replayed on restart.
        *self.task_session.lock().unwrap() = prepared.public.session.clone();
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
    fn run(&self, plan: &Prepared, report: &mut UvReport) -> Result<()> {
        check_cancel(Some(&self.cancel))?;
        let (current_exe, _, current) = discover(
            &known_folder(&FOLDERID_Profile)?,
            &known_folder(&FOLDERID_LocalAppData)?,
            &self.cancel,
        )?;
        ensure!(current_exe == plan.exe, "uv 工具位置已变化");
        ensure!(current == plan.root, "缓存配置已改变，未执行整理");
        validate_exclusions(&plan.root, &plan.exclusions)?;
        let root_pin = validate_root(&plan.root)?;
        ensure!(
            root_pin.stamp.identity == plan.root_identity
                && root_pin.parent_ids == plan.root_parents,
            "缓存目录已更换，未执行整理"
        );
        let exe_pin = PinnedPath::open(&plan.exe, false, false)?;
        ensure!(exe_pin.stamp == plan.exe_stamp, "uv 程序已更换，未执行整理");
        crate::tool_process::guard("dev.uv", Some(&plan.exe), None, &self.cancel)?;
        let before = inventory(&plan.root, &self.cancel, Duration::from_secs(120))?;
        report.before_bytes = Some(
            before
                .values()
                .map(|n| *n as u128)
                .sum::<u128>()
                .to_string(),
        );
        report.free_before = free_bytes(&plan.root).ok().map(|n| n.to_string());
        check_cancel(Some(&self.cancel))?;
        // No shell, force, --ci, package uninstall, project discovery or inherited configuration.
        let mut command = Command::new(&plan.exe);
        command
            .args([
                "cache",
                "prune",
                "--offline",
                "--no-config",
                "--no-progress",
                "--cache-dir",
            ])
            .arg(&plan.root)
            .current_dir(&plan.root)
            .env("UV_LOCK_TIMEOUT", "1")
            .env("UV_NO_CONFIG", "1")
            .env("UV_OFFLINE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000);
        let (status, output) = run_bounded(&mut command, &self.cancel, Duration::from_secs(120))?;
        report.output = output;
        report.free_after = free_bytes(&plan.root).ok().map(|n| n.to_string());
        // A fresh read is an observed logical size, never claimed as physical reclaimed bytes.
        let after = inventory(&plan.root, &self.cancel, Duration::from_secs(120)).ok();
        report.after_bytes = after
            .as_ref()
            .map(|files| files.values().map(|n| *n as u128).sum::<u128>().to_string());
        ensure!(status, "uv 返回失败，请查看工具输出");
        report.status = "completed".into();
        report.message = completion_message(&before, after.as_ref()).into();
        Ok(())
    }
    fn persist(&self, report: &UvReport, pending: bool) -> Result<()> {
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
    pub fn history(&self) -> Result<Vec<UvReport>> {
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
            ensure!(_file.stamp.size <= 256 * 1024, "uv 记录过大");
            let mut row: UvReport = serde_json::from_slice(&std::fs::read(&path)?)?;
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
        ensure!(!self.is_busy(), "uv 整理进行中");
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
pub(crate) fn validate_root(root: &Path) -> Result<PinnedPath> {
    is_fixed_ntfs(root)?;
    let pin = PinnedPath::open(root, true, false)?;
    // An existing but empty cache has nothing to prune. A missing marker is not
    // evidence that a previously observed file changed: discovery has no snapshot yet.
    if std::fs::read_dir(root)?.next().transpose()?.is_none() {
        anyhow::bail!(dustaway_core::ApiError::new(
            "cache_missing",
            "uv 缓存目录为空，无需整理"
        ));
    }
    let marker = root.join("CACHEDIR.TAG");
    ensure!(
        marker.try_exists()?,
        dustaway_core::ApiError::new(
            "unsupported_layout",
            "uv 缓存目录中有文件，但缺少 CACHEDIR.TAG 缓存标记，无法核验目录结构，暂不整理"
        )
    );
    let _marker =
        PinnedPath::observe(&marker, false, None).context("无法核验 uv 缓存标记 CACHEDIR.TAG")?;
    ensure!(
        _marker.stamp.size < 4096
            && std::fs::read_to_string(&marker)?
                .starts_with("Signature: 8a477f597d28d172789f06886806bc55"),
        dustaway_core::ApiError::new(
            "unsupported_layout",
            "uv 缓存标记 CACHEDIR.TAG 无效，无法核验目录结构，暂不整理"
        )
    );
    Ok(pin)
}
pub(crate) fn validate_exclusions(root: &Path, exclusions: &[String]) -> Result<()> {
    ensure!(
        !excluded(root, exclusions)
            && !exclusions
                .iter()
                .any(|p| extended(Path::new(p)).is_ok_and(|p| excluded(&p, &[display_path(root)]))),
        "uv 缓存与保留项重叠，不能进行整体整理"
    );
    Ok(())
}
fn completion_message(
    before: &BTreeMap<String, u64>,
    after: Option<&BTreeMap<String, u64>>,
) -> &'static str {
    if after.is_some_and(|after| after == before) {
        "未观察到缓存变化。uv 已完成检查，缓存文件清单未变化。"
    } else {
        "uv 整理完成。仅移除工具判断闲置的缓存；完成后的单项检查用于核对当前占用。"
    }
}

pub(crate) fn inventory(
    root: &Path,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<BTreeMap<String, u64>> {
    let deadline = Instant::now() + timeout;
    let mut pending = vec![root.to_path_buf()];
    let mut files = BTreeMap::new();
    while let Some(dir) = pending.pop() {
        check_cancel(Some(cancel))?;
        ensure!(Instant::now() < deadline, "缓存核对超时，未继续");
        let _pin = PinnedPath::observe(&dir, true, Some(cancel))?;
        for entry in std::fs::read_dir(&dir)? {
            check_cancel(Some(cancel))?;
            ensure!(Instant::now() < deadline, "缓存核对超时，未继续");
            let entry = entry?;
            let metadata = std::fs::symlink_metadata(entry.path())?;
            ensure!(
                metadata.file_attributes() & (0x400 | 0x1000 | 0x40000 | 0x400000) == 0,
                "缓存内有链接或云文件，整体整理已阻止"
            );
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                files.insert(entry.path().to_string_lossy().into_owned(), metadata.len());
            }
        }
    }
    Ok(files)
}
pub(crate) fn run_bounded(
    command: &mut Command,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<(bool, String)> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
    // Closing the app (including an abnormal exit) terminates only this owned
    // tool process and its children. No user's existing process is stopped.
    let job = Handle(unsafe { CreateJobObjectW(None, None)? });
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )?;
    }
    let mut child = command.spawn()?;
    if let Err(error) = unsafe { AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())) } {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    fn drain(mut pipe: impl Read) -> String {
        let mut kept = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(n) = pipe.read(&mut buffer) {
            if n == 0 {
                break;
            }
            let take = n.min(32768usize.saturating_sub(kept.len()));
            kept.extend_from_slice(&buffer[..take]);
        }
        String::from_utf8_lossy(&kept).into_owned()
    }
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || drain(stdout));
    let err = std::thread::spawn(move || drain(stderr));
    let deadline = Instant::now() + timeout;
    let result = loop {
        if let Some(status) = child.try_wait()? {
            break Ok(status.success());
        }
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!("工具操作已停止或超时"));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    // Close the job before joining pipe readers: an inherited pipe held by a
    // descendant must not keep cancellation or shutdown waiting indefinitely.
    drop(job);
    let output = format!(
        "{}\n{}",
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default()
    );
    match result {
        Ok(status) => Ok((status, output)),
        Err(error) => Err(anyhow::anyhow!("{error}; 工具输出：{output}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_cache_and_unmarked_content_are_not_reported_as_changed() {
        let temp = tempfile::tempdir().unwrap();
        let error = validate_root(temp.path()).err().unwrap();
        assert_eq!(
            error
                .downcast_ref::<dustaway_core::ApiError>()
                .unwrap()
                .code,
            "cache_missing"
        );
        assert!(error.to_string().contains("为空，无需整理"));
        assert!(!error.to_string().contains("已变化"));

        let payload = temp.path().join("keep.bin");
        std::fs::write(&payload, b"preserve unverified content").unwrap();
        let error = validate_root(temp.path()).err().unwrap();
        assert_eq!(
            error
                .downcast_ref::<dustaway_core::ApiError>()
                .unwrap()
                .code,
            "unsupported_layout"
        );
        assert!(error.to_string().contains("缺少 CACHEDIR.TAG"));
        assert!(!error.to_string().contains("重新扫描"));
        assert_eq!(
            std::fs::read(&payload).unwrap(),
            b"preserve unverified content"
        );

        let marker = temp.path().join("CACHEDIR.TAG");
        std::fs::write(&marker, "invalid marker").unwrap();
        assert!(
            validate_root(temp.path())
                .err()
                .unwrap()
                .to_string()
                .contains("标记 CACHEDIR.TAG 无效")
        );
        std::fs::write(&marker, "Signature: 8a477f597d28d172789f06886806bc55\n").unwrap();
        assert!(validate_root(temp.path()).is_ok());
    }
    #[test]
    fn zero_yield_requires_an_unchanged_inventory_not_just_equal_size() {
        let before = BTreeMap::from([("old".into(), 42)]);
        let replaced = BTreeMap::from([("new".into(), 42)]);
        assert!(completion_message(&before, Some(&before)).starts_with("未观察到"));
        assert!(!completion_message(&before, Some(&replaced)).starts_with("未观察到"));
        assert!(!completion_message(&before, None).starts_with("未观察到"));
    }
    #[test]
    fn rejects_broad_paths_and_overlapping_exclusions() {
        assert!(checked_cache_path(Path::new("C:\\")).is_err());
        assert!(checked_cache_path(Path::new("relative")).is_err());
        let root = Path::new("C:\\Users\\test\\cache");
        for path in ["C:\\Users", "C:\\Users\\test\\cache\\keep"] {
            assert!(validate_exclusions(root, &[path.into()]).is_err());
        }
        assert!(validate_exclusions(root, &["C:\\Users\\other".into()]).is_ok());
    }
    #[test]
    fn invalid_user_configuration_cannot_silently_fall_back_to_another_cache() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let roaming = temp.path().join("roaming");
        std::fs::create_dir_all(roaming.join("uv")).unwrap();
        std::fs::write(roaming.join("uv/uv.toml"), "cache-dir = 12").unwrap();
        assert!(cache_root(&local, &roaming).is_err());
        std::fs::write(roaming.join("uv/uv.toml"), "cache-dir = 'relative'").unwrap();
        assert!(cache_root(&local, &roaming).is_err());
        std::fs::write(roaming.join("uv/uv.toml"), "cache-dir = 'D:\\uv-cache'").unwrap();
        assert_eq!(
            cache_root(&local, &roaming).unwrap(),
            extended(Path::new("D:\\uv-cache")).unwrap()
        );
    }
    #[test]
    fn incomplete_intent_is_reported_and_never_resumed() {
        let temp = tempfile::tempdir().unwrap();
        let manager = UvCache::new(temp.path());
        let report = UvReport {
            id: "test".into(),
            started_at: 1,
            finished_at: None,
            status: "running".into(),
            cache: "test".into(),
            command: "uv cache prune".into(),
            message: String::new(),
            output: String::new(),
            before_bytes: None,
            after_bytes: None,
            free_before: None,
            free_after: None,
        };
        manager.persist(&report, true).unwrap();
        assert_eq!(manager.history().unwrap()[0].status, "interrupted");
        assert!(!manager.is_busy());
        assert!(manager.persist(&report, true).is_err());
    }
    #[test]
    fn expired_changed_and_replayed_confirmation_never_launches() {
        let temp = tempfile::tempdir().unwrap();
        let root_pin = PinnedPath::open(temp.path(), true, false).unwrap();
        let exe = std::env::current_exe().unwrap();
        let exe_pin = PinnedPath::open(&exe, false, false).unwrap();
        for kind in ["expired", "session", "exclusions", "token"] {
            let manager = Arc::new(UvCache::new(temp.path()));
            *manager.plan.lock().unwrap() = Some(Prepared {
                public: UvPlan {
                    id: "expected".into(),
                    session: "scan".into(),
                    cache: display_path(temp.path()),
                    executable: display_path(&exe),
                    expires_at: if kind == "expired" {
                        0
                    } else {
                        dustaway_core::now() + 120
                    },
                },
                root: temp.path().to_path_buf(),
                exe: exe.clone(),
                root_identity: root_pin.stamp.identity.clone(),
                root_parents: root_pin.parent_ids.clone(),
                exe_stamp: exe_pin.stamp.clone(),
                exclusions: vec![],
            });
            let changed = if kind == "exclusions" {
                vec!["C:\\keep".into()]
            } else {
                vec![]
            };
            assert!(
                manager
                    .start(
                        if kind == "token" { "wrong" } else { "expected" },
                        if kind == "session" { "new" } else { "scan" },
                        &changed
                    )
                    .is_err()
            );
            assert!(manager.start("expected", "scan", &[]).is_err());
            assert!(!manager.is_busy());
            assert!(manager.history().unwrap().is_empty());
        }
    }
    #[test]
    fn owned_process_times_out_and_never_continues_in_background() {
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 15",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000);
        let start = Instant::now();
        assert!(
            run_bounded(
                &mut command,
                &AtomicBool::new(false),
                Duration::from_millis(100)
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(5));
    }
    #[test]
    #[ignore = "requires an existing uv executable; only creates and prunes a dedicated temporary cache"]
    fn official_uv_prunes_only_dedicated_orphan_cache_and_preserves_hardlink() {
        let exe = known_folder(&FOLDERID_Profile)
            .unwrap()
            .join(".local/bin/uv.exe");
        assert!(exe.is_file());
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("uv/cache");
        let orphan = cache.join("archive-v0/dustaway-orphan/module.py");
        std::fs::create_dir_all(orphan.parent().unwrap()).unwrap();
        std::fs::write(&orphan, b"dedicated acceptance sample").unwrap();
        std::fs::write(
            cache.join("CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n",
        )
        .unwrap();
        std::fs::write(cache.join(".gitignore"), "*").unwrap();
        let protected = temp.path().join("protected-project-module.py");
        std::fs::hard_link(&orphan, &protected).unwrap();
        let pin = validate_root(&cache).unwrap();
        let mut command = Command::new(&exe);
        command
            .args([
                "cache",
                "prune",
                "--offline",
                "--no-config",
                "--no-progress",
                "--cache-dir",
            ])
            .arg(&cache)
            .current_dir(&cache)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000);
        let (success, output) = run_bounded(
            &mut command,
            &AtomicBool::new(false),
            Duration::from_secs(15),
        )
        .unwrap();
        assert!(success, "{output}");
        assert!(!orphan.exists(), "{output}");
        assert_eq!(
            std::fs::read(&protected).unwrap(),
            b"dedicated acceptance sample"
        );
        drop(pin);
    }
}
