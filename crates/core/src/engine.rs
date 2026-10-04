use crate::{model::*, rules};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::HashSet,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
#[path = "rule_refresh.rs"]
mod rule_refresh;

struct Inner {
    scan_volumes: Option<Vec<VolumeTarget>>,
    scan_started: Option<Instant>,
    rule_started: Option<Instant>,
    db: Connection,
    status: JobStatus,
    summary: Vec<RuleSummary>,
    plan: Option<CleanupPlan>,
    manual: Option<crate::ManualManifest>,
    settings: Settings,
    report: Option<CleanupReport>,
}

pub struct Engine {
    refresh_cancel: Mutex<Option<(String, String)>>,
    platform: Arc<dyn Platform>,
    scope: Scope,
    inner: Mutex<Inner>,
    journal: Mutex<Connection>,
    cancelled: AtomicBool,
}

impl Engine {
    pub fn open(
        platform: Arc<dyn Platform>,
        scope: Scope,
        journal_path: &Path,
    ) -> Result<Arc<Self>> {
        Self::with_journal(platform, scope, Connection::open(journal_path)?)
    }
    fn with_journal(
        platform: Arc<dyn Platform>,
        scope: Scope,
        journal: Connection,
    ) -> Result<Arc<Self>> {
        let db = Connection::open_in_memory()?;
        db.execute_batch("CREATE TABLE candidates(id TEXT PRIMARY KEY, rule TEXT NOT NULL, record TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'candidate', reason TEXT, reason_code TEXT, path TEXT NOT NULL, size_key TEXT NOT NULL, modified INTEGER NOT NULL);
            CREATE INDEX by_rule ON candidates(rule, id);
            CREATE INDEX by_size ON candidates(rule, size_key, id);
            CREATE TABLE observations(rule TEXT NOT NULL, path TEXT NOT NULL, size_key TEXT, record TEXT NOT NULL, PRIMARY KEY(rule,path));
            CREATE INDEX observation_size ON observations(rule,size_key,path);
            CREATE TABLE plan_items(id TEXT PRIMARY KEY);")?;
        journal.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS reports(id TEXT PRIMARY KEY, started INTEGER NOT NULL, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS intents(job TEXT NOT NULL, item TEXT NOT NULL, outcome TEXT NOT NULL, PRIMARY KEY(job,item));
            CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS preferences(key TEXT PRIMARY KEY, data TEXT NOT NULL);")?;
        let has_rule = journal
            .prepare("PRAGMA table_info(intents)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .iter()
            .any(|c| c == "rule");
        if !has_rule {
            journal.execute(
                "ALTER TABLE intents ADD COLUMN rule TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        let stored: Option<String> = journal
            .query_row("SELECT data FROM settings WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let settings = stored
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Settings {
                theme: "system".into(),
                ..Default::default()
            });
        let engine = Arc::new(Self {
            refresh_cancel: Mutex::new(None),
            platform,
            scope,
            inner: Mutex::new(Inner {
                scan_volumes: None,
                scan_started: None,
                rule_started: None,
                db,
                status: JobStatus::default(),
                summary: vec![],
                plan: None,
                manual: None,
                settings,
                report: None,
            }),
            journal: Mutex::new(journal),
            cancelled: AtomicBool::new(false),
        });
        engine.recover()?;
        Ok(engine)
    }

    pub fn bootstrap(&self) -> Result<Bootstrap> {
        Ok(Bootstrap {
            preferences: self.preferences()?,
            rules: rules::for_scope(self.scope)?,
            settings: self.settings(),
            version: env!("CARGO_PKG_VERSION").into(),
            capabilities: Capabilities {
                system_cleanup: self.scope == Scope::System,
            },
        })
    }
    pub fn settings(&self) -> Settings {
        self.inner.lock().unwrap().settings.clone()
    }
    pub fn save_settings(&self, settings: Settings) -> Result<SaveSettingsResult> {
        self.save_settings_locked(&mut self.inner.lock().unwrap(), settings)
    }
    pub fn patch_settings(&self, patch: SettingsPatch) -> Result<SaveSettingsResult> {
        let mut inner = self.inner.lock().unwrap();
        let settings = patch.apply(inner.settings.clone());
        self.save_settings_locked(&mut inner, settings)
    }
    fn save_settings_locked(
        &self,
        inner: &mut Inner,
        settings: Settings,
    ) -> Result<SaveSettingsResult> {
        ensure!(
            matches!(settings.theme.as_str(), "system" | "light" | "dark"),
            "无效主题"
        );
        ensure!(settings.exclusions.len() <= 500, "最多添加 500 个排除项");
        ensure!(
            settings
                .exclusions
                .iter()
                .all(|s| !s.is_empty() && s.len() < 32768 && !s.contains('\0')),
            "排除路径无效"
        );
        let known: HashSet<_> = rules::catalog()?.into_iter().map(|r| r.id).collect();
        ensure!(
            settings.excluded_rules.iter().all(|id| known.contains(id)),
            "排除规则无效"
        );
        let invalidate = settings.exclusions != inner.settings.exclusions
            || settings.excluded_rules != inner.settings.excluded_rules;
        ensure!(
            !invalidate || !busy(&inner.status),
            ApiError::new("busy", "任务进行中，请先取消")
        );
        self.journal.lock().unwrap().execute(
            "INSERT OR REPLACE INTO settings VALUES(1,?1)",
            [serde_json::to_string(&settings)?],
        )?;
        inner.settings = settings;
        if !invalidate {
            return Ok(SaveSettingsResult {
                scan_invalidated: false,
                settings: inner.settings.clone(),
            });
        }
        inner.plan = None;
        inner.manual = None;
        inner.db.execute("DELETE FROM plan_items", [])?;
        // A settings change invalidates the entire scan, including cached selections.
        inner.status = JobStatus::default();
        inner.summary.clear();
        self.platform.invalidate_tool_discovery(None);
        inner.report = None;
        inner.db.execute("DELETE FROM candidates", [])?;
        Ok(SaveSettingsResult {
            scan_invalidated: true,
            settings: inner.settings.clone(),
        })
    }
    pub fn preferences(&self) -> Result<Preferences> {
        let data: Option<String> = self
            .journal
            .lock()
            .unwrap()
            .query_row("SELECT data FROM preferences WHERE key='scan'", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(data
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }
    pub fn remember_scan(&self, session: &str) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        ensure!(
            inner.status.id == session && !session.is_empty(),
            ApiError::new("invalid_session", "扫描会话已失效")
        );
        let preferences = Preferences {
            last_scan_rule_ids: Some(inner.summary.iter().map(|r| r.rule_id.clone()).collect()),
        };
        self.journal.lock().unwrap().execute(
            "INSERT OR REPLACE INTO preferences VALUES('scan',?1)",
            [serde_json::to_string(&preferences)?],
        )?;
        Ok(())
    }
    pub fn runtime_snapshot(&self) -> RuntimeSnapshot {
        let inner = self.inner.lock().unwrap();
        RuntimeSnapshot {
            uninstall: None,
            analysis: None,
            control: None,
            status: Self::timed_status(&inner),
            summary: ScanSummary {
                session_id: inner.status.id.clone(),
                state: inner.status.state.clone(),
                rules: Self::timed_summary(&inner),
            },
            report: inner.report.clone(),
        }
    }
    fn candidate_record(
        inner: &Inner,
        session: &str,
        id: &str,
    ) -> Result<(RuleDescriptor, FileRecord)> {
        ensure!(
            inner.status.id == session && !session.is_empty(),
            ApiError::new("invalid_session", "扫描会话已失效")
        );
        ensure!(
            !busy(&inner.status),
            ApiError::new("busy", "请等待当前任务结束")
        );
        let (rule, record): (String, String) = inner
            .db
            .query_row(
                "SELECT rule,record FROM candidates WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| ApiError::new("invalid_candidate", "候选文件不存在，请重新扫描"))?;
        let rule = rules::catalog()?
            .into_iter()
            .find(|r| r.id == rule)
            .context("未知规则")?;
        Ok((rule, serde_json::from_str(&record)?))
    }
    pub fn reveal_candidate(&self, session: &str, id: &str) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        let (rule, file) = Self::candidate_record(&inner, session, id)?;
        self.platform
            .candidate_action(&rule, &file, ExclusionKind::File, true)?;
        Ok(())
    }
    pub fn add_candidate_exclusions(
        &self,
        session: &str,
        items: Vec<CandidateExclusion>,
    ) -> Result<SaveSettingsResult> {
        ensure!(
            !items.is_empty() && items.len() <= 500,
            ApiError::new("invalid_request", "一次最多添加 500 个排除项")
        );
        let mut inner = self.inner.lock().unwrap();
        let mut settings = inner.settings.clone();
        let mut seen: HashSet<String> = settings
            .exclusions
            .iter()
            .map(|s| s.trim_end_matches('\\').to_lowercase())
            .collect();
        for item in items {
            let (rule, file) = Self::candidate_record(&inner, session, &item.candidate_id)?;
            let path = self
                .platform
                .candidate_action(&rule, &file, item.kind, false)?;
            if seen.insert(path.trim_end_matches('\\').to_lowercase()) {
                settings.exclusions.push(path);
            }
        }
        self.save_settings_locked(&mut inner, settings)
    }
    fn timed_status(inner: &Inner) -> JobStatus {
        let mut status = inner.status.clone();
        if status.state == JobState::Scanning {
            status.scan_progress.elapsed_ms = inner
                .scan_started
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or(0);
        }
        status
    }
    fn timed_summary(inner: &Inner) -> Vec<RuleSummary> {
        let mut rows = inner.summary.clone();
        if inner.status.state == JobState::Scanning {
            for row in &mut rows {
                if row.phase == ScanPhase::Scanning {
                    row.progress.elapsed_ms = inner
                        .rule_started
                        .map(|t| t.elapsed().as_millis() as u64)
                        .unwrap_or(0);
                }
            }
        }
        rows
    }
    pub fn status(&self) -> JobStatus {
        Self::timed_status(&self.inner.lock().unwrap())
    }
    fn publish_progress(
        &self,
        rule: &str,
        checked: u64,
        last_progress_ms: u64,
        skipped: u64,
        reasons: &[ReasonCount],
    ) {
        let mut inner = self.inner.lock().unwrap();
        let elapsed = inner
            .rule_started
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let total_elapsed = inner
            .scan_started
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let row = inner
            .summary
            .iter_mut()
            .find(|r| r.rule_id == rule)
            .unwrap();
        let delta = checked.saturating_sub(row.progress.checked);
        row.progress = ScanProgress {
            checked,
            elapsed_ms: elapsed,
            last_progress_ms,
        };
        row.skipped = skipped;
        row.reasons = reasons.to_vec();
        inner.status.scan_progress.checked += delta;
        inner.status.scan_progress.elapsed_ms = total_elapsed;
        if delta > 0 {
            inner.status.scan_progress.last_progress_ms =
                total_elapsed.saturating_sub(elapsed.saturating_sub(last_progress_ms));
        }
    }
    pub fn is_busy(&self) -> bool {
        busy(&self.status())
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn start_scan(self: &Arc<Self>, rule_ids: Vec<String>) -> Result<String> {
        self.start_scan_on_volumes(rule_ids, None)
    }
    pub fn start_scan_on_volumes(
        self: &Arc<Self>,
        rule_ids: Vec<String>,
        volumes: Option<Vec<VolumeTarget>>,
    ) -> Result<String> {
        self.start_scan_with_context(rule_ids, volumes, true)
    }
    fn start_scan_with_context(
        self: &Arc<Self>,
        rule_ids: Vec<String>,
        volumes: Option<Vec<VolumeTarget>>,
        reset_discovery: bool,
    ) -> Result<String> {
        if let Some(volumes) = &volumes {
            ensure!(
                !volumes.is_empty() && volumes.len() <= 26,
                "请选择至少一个有效磁盘"
            );
            let keys: HashSet<_> = volumes.iter().map(|v| &v.key).collect();
            ensure!(
                keys.len() == volumes.len() && volumes.iter().all(|v| !v.key.is_empty()),
                "磁盘选择重复或无效"
            );
        }
        let available = rules::for_scope(self.scope)?;
        ensure!(!rule_ids.is_empty(), "请选择至少一个清理类别");
        ensure!(
            rule_ids
                .iter()
                .all(|id| available.iter().any(|r| &r.id == id)),
            "包含不允许的规则"
        );
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            !busy(&inner.status),
            ApiError::new("busy", "已有任务正在运行")
        );
        let selected: Vec<_> = available
            .into_iter()
            .filter(|r| rule_ids.contains(&r.id) && !inner.settings.excluded_rules.contains(&r.id))
            .collect();
        ensure!(!selected.is_empty(), "所选规则均已排除");
        let session = Uuid::new_v4().to_string();
        self.refresh_cancel.lock().unwrap().take();
        inner.db.execute_batch(
            "DELETE FROM candidates; DELETE FROM plan_items; DELETE FROM observations;",
        )?;
        inner.plan = None;
        inner.manual = None;
        inner.report = None;
        inner.summary = selected
            .iter()
            .map(|r| RuleSummary {
                observation: None,
                revision: 1,
                stale: false,
                pending_tool_task: None,
                tool: None,
                coverage: Some(ScanCoverage::default()),
                progress: ScanProgress::default(),
                phase: ScanPhase::Pending,
                rule_id: r.id.clone(),
                files: 0,
                bytes: "0".into(),
                skipped: 0,
                blocked: false,
                note: None,
                scan_failed: false,
                scan_issue: None,
                availability: None,
                reasons: vec![],
            })
            .collect();
        inner.scan_started = Some(Instant::now());
        inner.scan_volumes = volumes.clone();
        if reset_discovery {
            self.platform.invalidate_tool_discovery(None);
        }
        for rule in &selected {
            self.platform.invalidate_tool_discovery(Some(&rule.id));
        }
        inner.rule_started = None;
        inner.status = JobStatus {
            scan_volumes: volumes
                .as_ref()
                .map(|v| v.iter().map(|v| v.label.clone()).collect()),
            id: session.clone(),
            state: JobState::Scanning,
            ..Default::default()
        };
        self.cancelled.store(false, Ordering::Release);
        drop(inner);
        let engine = self.clone();
        std::thread::spawn(move || engine.guard_worker(|| engine.scan_worker(selected, volumes)));
        Ok(session)
    }
    fn guard_worker(&self, work: impl FnOnce() -> Result<()>) {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        let error = match outcome {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(format!("{e:#}")),
            Err(_) => Some("后台任务异常中断，请重新扫描".into()),
        };
        if let Some(error) = error {
            if let Ok(mut inner) = self.inner.lock() {
                inner.status.error = Some(error);
            }
            // A failed journal write may follow a successful deletion. Preserve uncertainty.
            if self.recover().is_ok()
                && let Ok(history) = self.history()
            {
                let mut inner = self.inner.lock().unwrap();
                if let Some(current) = &inner.report
                    && let Some(report) = history.into_iter().find(|r| r.id == current.id)
                {
                    inner.report = Some(report);
                }
            }
            if let Ok(mut inner) = self.inner.lock() {
                inner.status = Self::timed_status(&inner);
                inner.summary = Self::timed_summary(&inner);
                for row in &mut inner.summary {
                    if row.phase == ScanPhase::Pending {
                        row.phase = ScanPhase::Interrupted;
                    }
                    if row.phase == ScanPhase::Scanning {
                        row.phase = ScanPhase::Failed;
                        row.scan_failed = true;
                        row.blocked = true;
                    }
                }
                inner.status.state = JobState::Failed;
            }
        }
    }
    fn scan_worker(
        &self,
        selected: Vec<RuleDescriptor>,
        volumes: Option<Vec<VolumeTarget>>,
    ) -> Result<()> {
        let exclusions = self.settings().exclusions;
        let scanned_ids = selected.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        for rule in selected {
            if self.cancelled.load(Ordering::Acquire) {
                break;
            }
            {
                let mut inner = self.inner.lock().unwrap();
                inner.rule_started = Some(Instant::now());
                inner.status.current_rule = rule.id.clone();
                inner
                    .summary
                    .iter_mut()
                    .find(|s| s.rule_id == rule.id)
                    .unwrap()
                    .phase = ScanPhase::Scanning;
            }
            let rule_started = Instant::now();
            let mut last_publish = Instant::now();
            let mut checked = 0;
            let mut last_progress_ms = 0;
            let mut pending = Vec::with_capacity(128);
            let mut skipped = 0u64;
            let mut note = None;
            let mut db_error = None;
            let mut reasons = vec![];
            let mut scan_issue = None;
            let mut coverage = ScanCoverage::default();
            let mut observation = CacheObservation::default();
            let outcome = self.platform.scan_on_volumes(
                &rule,
                &exclusions,
                volumes.as_deref(),
                &self.cancelled,
                &mut |event| {
                    match event {
                        ScanEvent::Checked => {
                            checked += 1;
                            last_progress_ms = rule_started.elapsed().as_millis() as u64;
                        }
                        ScanEvent::File(file) => pending.push(file),
                        ScanEvent::Observation(file) => {
                            let result = (|| -> Result<()> {
                                let inner = self.inner.lock().unwrap();
                                let size = file
                                    .bytes
                                    .as_deref()
                                    .and_then(|b| b.parse::<u64>().ok())
                                    .map(|b| format!("{b:020}"));
                                inner.db.execute(
                                    "INSERT OR REPLACE INTO observations VALUES(?1,?2,?3,?4)",
                                    params![
                                        rule.id,
                                        file.path,
                                        size,
                                        serde_json::to_string(&file)?
                                    ],
                                )?;
                                Ok(())
                            })();
                            if let Err(e) = result {
                                db_error = Some(e);
                                self.cancel();
                            }
                        }
                        ScanEvent::Skipped(reason) => {
                            skipped += 1;
                            count_reason(&mut reasons, ReasonCode::Unknown);
                            if note.is_none() {
                                note = Some(reason);
                            }
                        }
                        ScanEvent::Notice(reason) => note = Some(reason),
                        ScanEvent::ApplicationObserved(app) => observation.application = Some(app),
                        ScanEvent::RetainedFile { reason, bytes } => {
                            skipped += 1;
                            count_reason(&mut reasons, reason);
                            observation.retained_files += 1;
                            if let Some(item) =
                                observation.reasons.iter_mut().find(|r| r.code == reason)
                            {
                                item.files += 1;
                                item.bytes =
                                    (item.bytes.parse::<u64>().unwrap_or(0).saturating_add(bytes))
                                        .to_string();
                            } else {
                                observation.reasons.push(ObservedReason {
                                    code: reason,
                                    files: 1,
                                    bytes: bytes.to_string(),
                                });
                            }
                            observation.retained_bytes = (observation
                                .retained_bytes
                                .parse::<u64>()
                                .unwrap_or(0)
                                .saturating_add(bytes))
                            .to_string();
                            if reason == ReasonCode::Recent {
                                observation.recent_files += 1;
                                observation.recent_bytes = (observation
                                    .recent_bytes
                                    .parse::<u64>()
                                    .unwrap_or(0)
                                    .saturating_add(bytes))
                                .to_string();
                            }
                        }
                        ScanEvent::Retained(code) => {
                            skipped += 1;
                            count_reason(&mut reasons, code);
                        }
                        ScanEvent::NoticeCode(code) => {
                            scan_issue = Some(code);
                            note = Some(code.message().into());
                        }
                        ScanEvent::CoverageIssue(code) => count_reason(&mut coverage.issues, code),
                    }
                    let publish = last_publish.elapsed() >= Duration::from_millis(100);
                    if (pending.len() >= 128 || publish)
                        && let Err(e) = self.insert_batch(&rule.id, &mut pending)
                    {
                        db_error = Some(e);
                        self.cancel();
                    }
                    if publish {
                        self.publish_progress(
                            &rule.id,
                            checked,
                            last_progress_ms,
                            skipped,
                            &reasons,
                        );
                        last_publish = Instant::now();
                        self.inner
                            .lock()
                            .unwrap()
                            .summary
                            .iter_mut()
                            .find(|s| s.rule_id == rule.id)
                            .unwrap()
                            .coverage = Some(coverage.clone());
                    }
                },
            );
            self.publish_progress(&rule.id, checked, last_progress_ms, skipped, &reasons);
            if let Some(error) = db_error {
                return Err(error);
            }
            self.insert_batch(&rule.id, &mut pending)?;
            let (availability, availability_note) = match self.platform.blocked(&rule) {
                Ok(Some(reason)) => (Some(reason.code), Some(reason.message)),
                Ok(None) => (None, None),
                Err(_) => (
                    Some(ReasonCode::ProcessUnknown),
                    Some(ReasonCode::ProcessUnknown.message().into()),
                ),
            };
            let mut inner = self.inner.lock().unwrap();
            let summary = inner
                .summary
                .iter_mut()
                .find(|s| s.rule_id == rule.id)
                .unwrap();
            summary.progress.elapsed_ms = rule_started.elapsed().as_millis() as u64;
            summary.tool = self.platform.tool_discovery(&rule.id);
            summary.observation = Some(observation);
            summary.skipped = skipped;
            summary.reasons = reasons;
            summary.availability = availability;
            summary.scan_issue = scan_issue;
            summary.scan_failed = matches!(
                scan_issue,
                Some(ReasonCode::DirectoryInaccessible | ReasonCode::UnsupportedVolume)
            ) || outcome.is_err();
            if let Err(error) = outcome {
                summary.scan_issue = Some(match reason_for(&error) {
                    ReasonCode::Unknown => ReasonCode::ScanFailed,
                    code => code,
                });
            }
            summary.blocked = summary.scan_failed || availability.is_some();
            summary.phase = if summary.scan_failed {
                ScanPhase::Failed
            } else if self.cancelled.load(Ordering::Acquire) {
                ScanPhase::Interrupted
            } else if !coverage.issues.is_empty() {
                ScanPhase::Partial
            } else {
                ScanPhase::Completed
            };
            coverage.complete = summary.phase == ScanPhase::Completed;
            summary.coverage = Some(coverage);
            summary.note = summary
                .scan_issue
                .map(|c| c.message().to_owned())
                .or(availability_note)
                .or(note);
        }
        let mut inner = self.inner.lock().unwrap();
        inner.status = Self::timed_status(&inner);
        inner.status.current_rule.clear();
        for row in &mut inner.summary {
            if matches!(row.phase, ScanPhase::Pending | ScanPhase::Scanning) {
                row.phase = ScanPhase::Interrupted;
            }
        }
        inner.status.state = if self.cancelled.load(Ordering::Acquire) {
            for id in scanned_ids {
                self.platform.invalidate_tool_discovery(Some(&id));
            }
            JobState::Cancelled
        } else {
            JobState::Ready
        };
        Ok(())
    }
    fn insert_batch(&self, rule: &str, batch: &mut Vec<FileRecord>) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let mut inner = self.inner.lock().unwrap();
        let mut bytes = 0u64;
        let count = batch.len() as u64;
        let tx = inner.db.transaction()?;
        {
            let mut insert =
                tx.prepare_cached("INSERT INTO candidates(id,rule,record,path,size_key,modified) VALUES(?1,?2,?3,?4,?5,?6)")?;
            for file in batch.drain(..) {
                bytes = bytes
                    .checked_add(file.bytes)
                    .context("文件大小超出可表示范围")?;
                insert.execute(params![
                    Uuid::new_v4().to_string(),
                    rule,
                    serde_json::to_string(&file)?,
                    file.path_display.to_lowercase(),
                    format!("{:020}", file.bytes),
                    file.modified_unix
                ])?;
            }
        }
        tx.commit()?;
        inner.status.visited += count;
        let summary = inner
            .summary
            .iter_mut()
            .find(|s| s.rule_id == rule)
            .unwrap();
        summary.files += count;
        summary.bytes = (summary.bytes.parse::<u64>()? + bytes).to_string();
        Ok(())
    }
    pub fn summary(&self) -> ScanSummary {
        let inner = self.inner.lock().unwrap();
        ScanSummary {
            session_id: inner.status.id.clone(),
            state: inner.status.state.clone(),
            rules: Self::timed_summary(&inner),
        }
    }
    pub fn candidates(
        &self,
        session: &str,
        rule: &str,
        offset: u64,
        limit: u32,
    ) -> Result<CandidatePage> {
        self.query_candidates(session, rule, offset, limit, CandidateQuery::default())
    }
    pub fn query_candidates(
        &self,
        session: &str,
        rule: &str,
        offset: u64,
        limit: u32,
        query: CandidateQuery,
    ) -> Result<CandidatePage> {
        let inner = self.inner.lock().unwrap();
        ensure!(
            session == inner.status.id && !session.is_empty(),
            ApiError::new("invalid_session", "扫描会话已失效")
        );
        ensure!(
            inner.summary.iter().any(|r| r.rule_id == rule),
            ApiError::new("invalid_request", "该类别未扫描")
        );
        ensure!(
            query.search.chars().count() <= 256,
            ApiError::new("invalid_request", "搜索最多 256 个字符")
        );
        let column = match query.sort.as_str() {
            "size" => "c.size_key",
            "modified" => "c.modified",
            "path" => "c.path",
            _ => return Err(ApiError::new("invalid_request", "无效排序").into()),
        };
        let direction = match query.direction.as_str() {
            "asc" => "ASC",
            "desc" => "DESC",
            _ => return Err(ApiError::new("invalid_request", "无效排序方向").into()),
        };
        ensure!(
            matches!(query.view.as_str(), "scan" | "cleanup" | "unplanned"),
            ApiError::new("invalid_request", "无效明细视图")
        );
        ensure!(
            query.view == "scan" || inner.report.is_some(),
            ApiError::new("invalid_plan", "当前没有已执行计划")
        );
        ensure!(
            query.status.as_deref().is_none_or(|s| matches!(
                s,
                "candidate" | "deleted" | "skipped" | "failed" | "unknown"
            )),
            ApiError::new("invalid_request", "无效结果筛选")
        );
        let limit = limit.clamp(1, 200);
        ensure!(offset <= i64::MAX as u64, "分页位置无效");
        let join = if query.view == "cleanup" {
            "JOIN plan_items p ON c.id=p.id"
        } else {
            ""
        };
        let unplanned = if query.view == "unplanned" {
            "AND NOT EXISTS (SELECT 1 FROM plan_items p WHERE p.id=c.id)"
        } else {
            ""
        };
        let filter = format!(
            "FROM candidates c {join} WHERE c.rule=?1 {unplanned} AND instr(c.path,?2)>0 AND (?3 IS NULL OR c.status=?3)"
        );
        let search = query.search.to_lowercase();
        let total = inner.db.query_row(
            &format!("SELECT count(*) {filter}"),
            params![rule, search, query.status],
            |r| r.get::<_, i64>(0),
        )? as u64;
        let mut statement = inner.db.prepare(&format!("SELECT c.id,c.rule,c.record,c.status,c.reason,c.reason_code {filter} ORDER BY {column} {direction},c.id ASC LIMIT ?4 OFFSET ?5"))?;
        let items = statement
            .query_map(
                params![rule, search, query.status, limit, offset as i64],
                |row| {
                    let data: String = row.get(2)?;
                    let file: FileRecord = serde_json::from_str(&data).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Ok(Candidate {
                        root: self.platform.candidate_root(&file),
                        id: row.get(0)?,
                        rule_id: row.get(1)?,
                        path: file.path_display,
                        bytes: file.bytes.to_string(),
                        modified_unix: file.modified_unix,
                        status: row.get(3)?,
                        reason: row.get(4)?,
                        reason_code: row
                            .get::<_, Option<String>>(5)?
                            .and_then(|s| serde_json::from_str(&s).ok()),
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(CandidatePage {
            items,
            total,
            offset,
        })
    }
    pub fn observations(
        &self,
        session: &str,
        rule: &str,
        offset: u64,
        limit: u32,
    ) -> Result<ObservationPage> {
        let inner = self.inner.lock().unwrap();
        ensure!(
            !session.is_empty() && session == inner.status.id,
            "扫描会话已失效"
        );
        ensure!(
            inner.summary.iter().any(|r| r.rule_id == rule),
            "该类别未扫描"
        );
        ensure!(offset <= i64::MAX as u64, "分页位置无效");
        let total = inner.db.query_row(
            "SELECT count(*) FROM observations WHERE rule=?1",
            [rule],
            |r| r.get::<_, i64>(0),
        )? as u64;
        let mut query = inner.db.prepare("SELECT record FROM observations WHERE rule=?1 ORDER BY size_key DESC,path ASC LIMIT ?2 OFFSET ?3")?;
        let records = query
            .query_map(params![rule, limit.clamp(1, 200), offset as i64], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let items = records
            .iter()
            .map(|r| serde_json::from_str(r))
            .collect::<serde_json::Result<Vec<_>>>()?;
        Ok(ObservationPage {
            items,
            total,
            offset,
        })
    }

    pub fn refresh_availability(&self, session: &str) -> Result<ScanSummary> {
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            session == inner.status.id
                && inner.status.state == JobState::Ready
                && !busy(&inner.status),
            ApiError::new("invalid_session", "请完成一次新的扫描")
        );
        let rules = rules::for_scope(self.scope)?;
        inner.plan = None;
        inner.manual = None;
        inner.db.execute("DELETE FROM plan_items", [])?;
        for row in &mut inner.summary {
            let rule = rules
                .iter()
                .find(|r| r.id == row.rule_id)
                .context("无效规则")?;
            let (availability, note) = match self.platform.blocked(rule) {
                Ok(None) => (None, None),
                Ok(Some(reason)) => (Some(reason.code), Some(reason.message)),
                Err(_) => (
                    Some(ReasonCode::ProcessUnknown),
                    Some(ReasonCode::ProcessUnknown.message().into()),
                ),
            };
            row.availability = availability;
            row.tool = self.platform.tool_discovery(&rule.id);
            row.blocked = row.scan_failed || row.availability.is_some();
            row.note = row.scan_issue.map(|c| c.message().into()).or(note);
        }
        Ok(ScanSummary {
            session_id: inner.status.id.clone(),
            state: inner.status.state.clone(),
            rules: Self::timed_summary(&inner),
        })
    }
    pub fn prepare(&self, session: &str, selection: Selection) -> Result<CleanupPlan> {
        let rules = rules::for_scope(self.scope)?;
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            inner.status.state == JobState::Ready
                && !busy(&inner.status)
                && session == inner.status.id,
            ApiError::new("invalid_session", "请完成一次新的扫描")
        );
        // Invalidate any previous plan even if preparing this one fails.
        inner.plan = None;
        inner.manual = None;
        inner.db.execute("DELETE FROM plan_items", [])?;
        ensure!(
            !selection.rule_ids.is_empty() || !selection.included_ids.is_empty(),
            "请选择清理文件"
        );
        ensure!(
            selection.excluded_ids.len() <= 100_000 && selection.included_ids.len() <= 100_000,
            "文件选择过多，请改用类别选择或目录排除"
        );
        let all_ids: HashSet<_> = selection.rule_ids.into_iter().collect();
        let mut ids = all_ids.clone();
        let reviewed: HashSet<_> = selection.reviewed_rule_ids.into_iter().collect();
        for id in &selection.included_ids {
            let rule: String = inner
                .db
                .query_row(
                    "SELECT rule FROM candidates WHERE id=?1 AND status='candidate'",
                    [id],
                    |r| r.get(0),
                )
                .context("所选文件不属于当前扫描候选")?;
            ids.insert(rule);
        }
        for id in &ids {
            let rule = rules.iter().find(|r| &r.id == id).context("无效规则")?;
            ensure!(
                self.platform.cleanup_validated(rule),
                Refusal(ReasonCode::ValidationPending)
            );
            ensure!(
                rule.cleanup_policy != CleanupPolicy::Review || reviewed.contains(id),
                "需确认项目尚未明确选择"
            );
            ensure!(
                inner.summary.iter().any(|s| &s.rule_id == id
                    && !s.blocked
                    && matches!(s.phase, ScanPhase::Completed | ScanPhase::Partial)),
                "该类别未扫描或目前无法清理"
            );
            if let Some(reason) = self.platform.blocked(rule)? {
                bail!(Refusal(reason.code));
            }
        }
        let tx = inner.db.transaction()?;
        for id in &all_ids {
            tx.execute("INSERT INTO plan_items SELECT id FROM candidates WHERE rule=?1 AND status='candidate'", [id])?;
        }
        for id in selection.included_ids {
            tx.execute("INSERT OR IGNORE INTO plan_items SELECT id FROM candidates WHERE id=?1 AND status='candidate'", [id])?;
        }
        for id in selection.excluded_ids {
            ensure!(
                tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM candidates WHERE id=?1)",
                    [&id],
                    |r| r.get::<_, bool>(0)
                )?,
                "排除文件不属于当前扫描"
            );
            tx.execute("DELETE FROM plan_items WHERE id=?1", [&id])?;
        }
        let files = tx.query_row("SELECT count(*) FROM plan_items", [], |r| {
            r.get::<_, i64>(0)
        })? as u64;
        ensure!(files > 0, "没有可清理的文件");
        let mut bytes = 0u64;
        let mut categories: Vec<PlanCategory> = vec![];
        {
            let mut q =
                tx.prepare("SELECT rule,record FROM candidates JOIN plan_items USING(id)")?;
            for row in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
                let (rule_id, data) = row?;
                let file = serde_json::from_str::<FileRecord>(&data)?;
                let rule = rules.iter().find(|r| r.id == rule_id).context("无效规则")?;
                self.platform.validate_candidate(rule, &file)?;
                let size = file.bytes;
                bytes = bytes.checked_add(size).context("文件大小超出范围")?;
                if let Some(item) = categories.iter_mut().find(|c| c.rule_id == rule_id) {
                    item.files += 1;
                    item.bytes = (item.bytes.parse::<u64>()? + size).to_string();
                } else {
                    categories.push(PlanCategory {
                        coverage: None,
                        rule_id,
                        files: 1,
                        bytes: size.to_string(),
                    });
                }
            }
        }
        tx.commit()?;
        for category in &mut categories {
            category.coverage = inner
                .summary
                .iter()
                .find(|s| s.rule_id == category.rule_id)
                .and_then(|s| s.coverage.clone());
        }
        let mut rule_ids: Vec<_> = categories.iter().map(|c| c.rule_id.clone()).collect();
        rule_ids.sort();
        let plan = CleanupPlan {
            manual: None,
            id: Uuid::new_v4().to_string(),
            session_id: session.into(),
            files,
            bytes: bytes.to_string(),
            impacts: rules
                .iter()
                .filter(|r| rule_ids.contains(&r.id))
                .map(|r| r.impact.clone())
                .collect(),
            rule_ids,
            categories,
        };
        inner.plan = Some(plan.clone());
        Ok(plan)
    }
    pub fn execute(self: &Arc<Self>, plan_id: &str) -> Result<String> {
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            inner.status.state == JobState::Ready && !busy(&inner.status),
            ApiError::new("invalid_plan", "任务状态不允许清理")
        );
        let plan = inner
            .plan
            .as_ref()
            .ok_or_else(|| ApiError::new("invalid_plan", "清理计划已失效，请重新确认"))?;
        ensure!(
            plan.id == plan_id && plan.session_id == inner.status.id,
            ApiError::new("invalid_plan", "清理计划不匹配")
        );
        let plan = inner.plan.take().unwrap();
        let report = CleanupReport {
            manual: plan.manual.as_ref().map(|m| RuleOutcome {
                rule_id: m.source.clone().unwrap_or_default(),
                total: m.files,
                deleted_bytes: "0".into(),
                ..Default::default()
            }),
            directories: crate::DirectoryOutcome {
                total: plan.manual.as_ref().map_or(0, |m| m.directories),
                ..Default::default()
            },
            volume_space_changes: vec![],
            id: plan.id.clone(),
            started_at: now(),
            finished_at: None,
            status: "running".into(),
            total: plan.files,
            deleted: 0,
            skipped: 0,
            failed: 0,
            unknown: 0,
            candidate_bytes: plan.bytes,
            deleted_bytes: "0".into(),
            free_space_change: None,
            scope: self.scope,
            categories: plan
                .categories
                .iter()
                .map(|c| RuleOutcome {
                    coverage: c.coverage.clone(),
                    rule_id: c.rule_id.clone(),
                    total: c.files,
                    deleted_bytes: "0".into(),
                    retained: inner
                        .summary
                        .iter()
                        .find(|s| s.rule_id == c.rule_id)
                        .map(|s| s.reasons.clone())
                        .unwrap_or_default(),
                    ..Default::default()
                })
                .collect(),
        };
        self.persist_report(&report)?;
        inner.status.state = JobState::Cleaning;
        inner.status.total = plan.files + plan.manual.as_ref().map_or(0, |m| m.directories);
        inner.status.completed = 0;
        inner.report = Some(report);
        self.cancelled.store(false, Ordering::Release);
        drop(inner);
        let engine = self.clone();
        std::thread::spawn(move || engine.guard_worker(|| engine.clean_worker()));
        Ok(plan.id)
    }
    pub fn invalidate_for_software(&self) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        ensure!(!busy(&inner.status), "已有清理任务");
        inner.plan = None;
        inner.manual = None;
        inner.summary.clear();
        inner.status = JobStatus::default();
        inner.report = None;
        inner
            .db
            .execute_batch("DELETE FROM candidates; DELETE FROM plan_items;")?;
        Ok(())
    }
    pub fn invalidate_plan(&self) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        ensure!(!busy(&inner.status), ApiError::new("busy", "任务进行中"));
        inner.plan = None;
        inner.manual = None;
        inner.db.execute("DELETE FROM plan_items", [])?;
        Ok(())
    }
    /// Attach a backend-only manifest to an optional rule plan. No synthetic rule IDs.
    pub fn attach_manual(&self, manifest: crate::ManualManifest) -> Result<CleanupPlan> {
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            self.scope == Scope::User && !busy(&inner.status),
            "当前不能准备自选清理"
        );
        if inner.status.id.is_empty() {
            inner.status.id = Uuid::new_v4().to_string();
        }
        let mut plan = inner.plan.take().unwrap_or_else(|| CleanupPlan {
            manual: None,
            id: Uuid::new_v4().to_string(),
            session_id: inner.status.id.clone(),
            files: 0,
            bytes: "0".into(),
            rule_ids: vec![],
            impacts: vec![],
            categories: vec![],
        });
        ensure!(plan.manual.is_none(), "自选清单已附加");
        plan.files += manifest.summary.files;
        plan.bytes = plan
            .bytes
            .parse::<u64>()?
            .checked_add(manifest.summary.bytes.parse()?)
            .context("文件大小超出范围")?
            .to_string();
        plan.manual = Some(manifest.summary.clone());
        inner.manual = Some(manifest);
        inner.status.state = JobState::Ready;
        inner.plan = Some(plan.clone());
        Ok(plan)
    }
    fn clean_manual(
        &self,
        manifest: crate::ManualManifest,
        exclusions: &[String],
        report: &mut CleanupReport,
    ) -> Result<()> {
        let mut query = manifest
            .db
            .prepare("SELECT id,record FROM items ORDER BY directory,depth DESC,id")?;
        for row in query.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            if self.cancelled.load(Ordering::Acquire) {
                break;
            }
            let (id, data) = row?;
            let record: crate::ManualRecord = serde_json::from_str(&data)?;
            self.inner.lock().unwrap().status.current_rule = "manual".into();
            self.journal.lock().unwrap().execute(
                "INSERT INTO intents(job,item,outcome,rule) VALUES(?1,?2,'pending','')",
                params![report.id, id],
            )?;
            let result = manifest.platform.delete_manual(&record, exclusions);
            let reason = result.as_ref().err().map(reason_for);
            let status = match &result {
                Ok(()) => "deleted",
                Err(e) if e.downcast_ref::<DeleteFailure>().is_some() => "failed",
                Err(_) => "skipped",
            };
            if record.directory {
                match status {
                    "deleted" => report.directories.deleted += 1,
                    "failed" => report.directories.failed += 1,
                    _ => report.directories.skipped += 1,
                }
            } else {
                let manual = report.manual.as_mut().context("缺少自选报告")?;
                match status {
                    "deleted" => {
                        report.deleted += 1;
                        manual.deleted += 1;
                        report.deleted_bytes =
                            (report.deleted_bytes.parse::<u64>()? + record.file.bytes).to_string();
                        manual.deleted_bytes =
                            (manual.deleted_bytes.parse::<u64>()? + record.file.bytes).to_string();
                    }
                    "failed" => {
                        report.failed += 1;
                        manual.failed += 1;
                    }
                    _ => {
                        report.skipped += 1;
                        manual.skipped += 1;
                    }
                }
                if let Some(reason) = reason {
                    count_reason(&mut manual.reasons, reason);
                }
            }
            let mut journal = self.journal.lock().unwrap();
            let tx = journal.transaction()?;
            tx.execute(
                "UPDATE intents SET outcome=?3 WHERE job=?1 AND item=?2",
                params![report.id, id, status],
            )?;
            tx.execute(
                "UPDATE reports SET data=?2 WHERE id=?1",
                params![report.id, serde_json::to_string(report)?],
            )?;
            tx.commit()?;
            drop(journal);
            let mut inner = self.inner.lock().unwrap();
            inner.status.completed += 1;
            inner.report = Some(report.clone());
        }
        Ok(())
    }
    fn clean_worker(&self) -> Result<()> {
        let rules = rules::for_scope(self.scope)?;
        let exclusions = self.settings().exclusions;
        let mut report = self.inner.lock().unwrap().report.clone().unwrap();
        // Resolve only backend plan records, once per allowed root. No frontend paths are sampled.
        let representatives = {
            let inner = self.inner.lock().unwrap();
            let mut roots = std::collections::HashSet::new();
            let mut files = vec![];
            let mut query = inner
                .db
                .prepare("SELECT c.record FROM candidates c JOIN plan_items p ON c.id=p.id")?;
            for row in query.query_map([], |r| r.get::<_, String>(0))? {
                if self.cancelled.load(Ordering::Acquire) {
                    break;
                }
                let file: FileRecord = serde_json::from_str(&row?)?;
                let root = self
                    .platform
                    .candidate_root(&file)
                    .unwrap_or_else(|| file.path_display.clone());
                if roots.insert(root) {
                    files.push(file);
                }
            }
            if let Some(manifest) = &inner.manual {
                let mut q = manifest.db.prepare("SELECT record FROM items")?;
                for row in q.query_map([], |r| r.get::<_, String>(0))? {
                    if self.cancelled.load(Ordering::Acquire) {
                        break;
                    }
                    let item: crate::ManualRecord = serde_json::from_str(&row?)?;
                    let root = self
                        .platform
                        .candidate_root(&item.file)
                        .unwrap_or_else(|| item.file.path_display.clone());
                    if roots.insert(root) {
                        files.push(item.file);
                    }
                }
            }
            files
        };
        let mut volumes = std::collections::BTreeMap::new();
        let mut unresolved = false;
        for file in representatives {
            match self.platform.file_volume(&file) {
                Ok(v) => {
                    volumes.entry(v.key.clone()).or_insert(v);
                }
                Err(_) => unresolved = true,
            }
        }
        let volumes: Vec<_> = volumes.into_values().collect();
        for volume in &volumes {
            let before = self.platform.volume_available(volume).ok();
            report.volume_space_changes.push(VolumeSpaceChange {
                volume: volume.label.clone(),
                before_bytes: before.map(|n| n.to_string()),
                after_bytes: None,
                change_bytes: None,
                error: Some(
                    if before.is_some() {
                        "after_pending"
                    } else {
                        "before_unavailable"
                    }
                    .into(),
                ),
            });
        }
        if unresolved {
            report.volume_space_changes.push(VolumeSpaceChange {
                volume: "未能识别的目标磁盘".into(),
                before_bytes: None,
                after_bytes: None,
                change_bytes: None,
                error: Some("volume_unavailable".into()),
            });
        }
        self.persist_report(&report)?;
        self.inner.lock().unwrap().report = Some(report.clone());
        let mut last_id = String::new();
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                break;
            }
            let next: Option<(String, String, String)> = {
                let inner = self.inner.lock().unwrap();
                inner.db.query_row("SELECT c.id,c.rule,c.record FROM candidates c JOIN plan_items p ON c.id=p.id WHERE c.id>?1 ORDER BY c.id LIMIT 1", [&last_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?
            };
            let Some((id, rule_id, data)) = next else {
                break;
            };
            last_id = id.clone();
            let rule = rules
                .iter()
                .find(|r| r.id == rule_id)
                .context("规则已失效")?;
            let file: FileRecord = serde_json::from_str(&data)?;
            self.inner.lock().unwrap().status.current_rule = rule_id.clone();
            // Durable intent precedes each destructive action. A crash leaves 'pending'.
            self.journal.lock().unwrap().execute(
                "INSERT INTO intents(job,item,outcome,rule) VALUES(?1,?2,'pending',?3)",
                params![report.id, id, rule_id],
            )?;
            let result = match if self.platform.cleanup_validated(rule) {
                self.platform.blocked(rule)
            } else {
                Ok(Some(BlockReason::new(
                    ReasonCode::ValidationPending,
                    ReasonCode::ValidationPending.message(),
                )))
            } {
                Ok(Some(reason)) => Err(Refusal(reason.code).into()),
                Ok(None) => self.platform.delete(rule, &file, &exclusions),
                Err(_) => Err(Refusal(ReasonCode::ProcessUnknown).into()),
            };
            let reason_code = result.as_ref().err().map(reason_for);
            let (status, reason) = match result {
                Ok(()) => {
                    report.deleted += 1;
                    report.deleted_bytes =
                        (report.deleted_bytes.parse::<u64>()? + file.bytes).to_string();
                    ("deleted", None)
                }
                Err(error) if error.downcast_ref::<DeleteFailure>().is_some() => {
                    report.failed += 1;
                    ("failed", Some(format!("{error:#}")))
                }
                Err(error) => {
                    report.skipped += 1;
                    ("skipped", Some(format!("{error:#}")))
                }
            };
            if let Some(category) = report.categories.iter_mut().find(|c| c.rule_id == rule_id) {
                match status {
                    "deleted" => {
                        category.deleted += 1;
                        category.deleted_bytes =
                            (category.deleted_bytes.parse::<u64>()? + file.bytes).to_string();
                    }
                    "skipped" => category.skipped += 1,
                    "failed" => category.failed += 1,
                    _ => (),
                }
                if let Some(code) = reason_code {
                    count_reason(&mut category.reasons, code);
                }
            }
            {
                let mut journal = self.journal.lock().unwrap();
                let tx = journal.transaction()?;
                tx.execute(
                    "UPDATE intents SET outcome=?3 WHERE job=?1 AND item=?2",
                    params![report.id, id, status],
                )?;
                tx.execute(
                    "UPDATE reports SET data=?2 WHERE id=?1",
                    params![report.id, serde_json::to_string(&report)?],
                )?;
                tx.commit()?;
            }
            let mut inner = self.inner.lock().unwrap();
            inner.db.execute(
                "UPDATE candidates SET status=?2,reason=?3,reason_code=?4 WHERE id=?1",
                params![
                    id,
                    status,
                    reason,
                    reason_code.map(|c| serde_json::to_string(&c)).transpose()?
                ],
            )?;
            inner.status.completed += 1;
            inner.report = Some(report.clone());
        }
        let manual = self.inner.lock().unwrap().manual.take();
        if let Some(manifest) = manual {
            self.clean_manual(manifest, &exclusions, &mut report)?;
        }
        let cancelled = self.cancelled.load(Ordering::Acquire);
        report.status = if cancelled { "cancelled" } else { "finished" }.into();
        report.finished_at = Some(now());
        for (volume, change) in volumes.iter().zip(report.volume_space_changes.iter_mut()) {
            let after = self.platform.volume_available(volume).ok();
            let before = change
                .before_bytes
                .as_ref()
                .and_then(|n| n.parse::<u64>().ok());
            change.after_bytes = after.map(|n| n.to_string());
            change.change_bytes = before
                .zip(after)
                .map(|(b, a)| (i128::from(a) - i128::from(b)).to_string());
            change.error = match (before, after) {
                (Some(_), Some(_)) => None,
                (None, None) => Some("both_unavailable"),
                (None, _) => Some("before_unavailable"),
                (_, None) => Some("after_unavailable"),
            }
            .map(str::to_owned);
        }
        self.persist_report(&report)?;
        let mut inner = self.inner.lock().unwrap();
        inner.report = Some(report);
        inner.status.state = if cancelled {
            JobState::Cancelled
        } else {
            JobState::Finished
        };
        inner.status.current_rule.clear();
        Ok(())
    }
    fn persist_report(&self, report: &CleanupReport) -> Result<()> {
        self.journal.lock().unwrap().execute(
            "INSERT OR REPLACE INTO reports VALUES(?1,?2,?3)",
            params![report.id, report.started_at, serde_json::to_string(report)?],
        )?;
        Ok(())
    }
    pub fn report(&self) -> Option<CleanupReport> {
        self.inner.lock().unwrap().report.clone()
    }
    pub fn validate_plan(&self, plan_id: &str) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        ensure!(
            inner.status.state == JobState::Ready && !busy(&inner.status),
            ApiError::new("invalid_plan", "需要重新扫描")
        );
        ensure!(
            inner
                .plan
                .as_ref()
                .is_some_and(|p| p.id == plan_id && p.session_id == inner.status.id),
            ApiError::new("invalid_plan", "清理计划已失效")
        );
        Ok(())
    }
    pub fn record_external_report(&self, report: &CleanupReport) -> Result<()> {
        ensure!(
            self.scope == Scope::User && report.scope == Scope::System,
            "报告来源不匹配"
        );
        let cutoff: Option<i64> = self
            .journal
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM metadata WHERE key='history_cleared_at'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if cutoff.is_some_and(|cutoff| report.started_at <= cutoff) {
            return Ok(());
        }
        self.persist_report(report)
    }
    pub fn history(&self) -> Result<Vec<CleanupReport>> {
        self.prune_history()?;
        let journal = self.journal.lock().unwrap();
        let mut q = journal.prepare("SELECT data FROM reports ORDER BY started DESC LIMIT 100")?;
        let mut reports = vec![];
        for row in q.query_map([], |r| r.get::<_, String>(0))? {
            reports.push(serde_json::from_str(&row?)?);
        }
        Ok(reports)
    }
    pub fn clear_history(&self) -> Result<()> {
        ensure!(!self.is_busy(), "任务进行中");
        let mut journal = self.journal.lock().unwrap();
        let tx = journal.transaction()?;
        tx.execute_batch("DELETE FROM intents; DELETE FROM reports;")?;
        tx.execute(
            "INSERT OR REPLACE INTO metadata VALUES('history_cleared_at',?1)",
            [now()],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn recover(&self) -> Result<()> {
        let interrupted: Vec<String> = {
            let journal = self.journal.lock().unwrap();
            journal
                .prepare("SELECT data FROM reports WHERE json_extract(data,'$.status')='running'")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for data in interrupted {
            let mut report: CleanupReport = serde_json::from_str(&data)?;
            if report.status == "running" {
                report.status = "interrupted".into();
                report.finished_at = Some(now());
                report.unknown = self.journal.lock().unwrap().query_row(
                    "SELECT count(*) FROM intents WHERE job=?1 AND outcome='pending' AND item NOT LIKE 'manual:directory:%'",
                    [&report.id],
                    |r| r.get::<_, i64>(0),
                )? as u64;
                report.directories.unknown = self.journal.lock().unwrap().query_row("SELECT count(*) FROM intents WHERE job=?1 AND outcome='pending' AND item LIKE 'manual:directory:%'", [&report.id], |r| r.get::<_, i64>(0))? as u64;
                if let Some(manual) = &mut report.manual {
                    manual.unknown = self.journal.lock().unwrap().query_row("SELECT count(*) FROM intents WHERE job=?1 AND outcome='pending' AND item LIKE 'manual:file:%'", [&report.id], |r| r.get::<_, i64>(0))? as u64;
                }
                for category in &mut report.categories {
                    category.unknown = self.journal.lock().unwrap().query_row("SELECT count(*) FROM intents WHERE job=?1 AND rule=?2 AND outcome='pending'", params![report.id, category.rule_id], |r| r.get::<_, i64>(0))? as u64;
                    if category.unknown > 0 {
                        category.reasons.push(ReasonCount {
                            code: ReasonCode::Unknown,
                            count: category.unknown,
                        });
                    }
                }
                self.inner.lock().unwrap().db.execute("UPDATE candidates SET status='unknown',reason_code=?1,reason=?2 WHERE id IN (SELECT id FROM plan_items) AND status='candidate' AND id IN (SELECT value FROM json_each(?3))", params![serde_json::to_string(&ReasonCode::Unknown)?, ReasonCode::Unknown.message(), {
                    let journal = self.journal.lock().unwrap();
                    let ids: Vec<String> = journal.prepare("SELECT item FROM intents WHERE job=?1 AND outcome='pending'")?.query_map([&report.id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
                    serde_json::to_string(&ids)?
                }])?;
                self.persist_report(&report)?;
            }
        }
        self.prune_history()
    }
    fn prune_history(&self) -> Result<()> {
        let mut journal = self.journal.lock().unwrap();
        let tx = journal.transaction()?;
        tx.execute(
            "DELETE FROM reports WHERE started<?1 AND json_extract(data,'$.status')<>'running'",
            [now() - 30 * 86400],
        )?;
        tx.execute(
            "DELETE FROM intents WHERE job NOT IN (SELECT id FROM reports)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}

fn busy(status: &JobStatus) -> bool {
    matches!(status.state, JobState::Scanning | JobState::Cleaning)
        || status
            .rule_refresh
            .as_ref()
            .is_some_and(|r| r.state == JobState::Scanning)
}

#[cfg(test)]
mod startup_storage_tests {
    use super::*;
    struct Untouched;
    impl Platform for Untouched {
        fn scan(
            &self,
            _: &RuleDescriptor,
            _: &[String],
            _: &AtomicBool,
            _: &mut dyn FnMut(ScanEvent),
        ) -> Result<()> {
            panic!("must not scan before durable startup succeeds")
        }
        fn blocked(&self, _: &RuleDescriptor) -> Result<Option<BlockReason>> {
            panic!("must not check deletion")
        }
        fn delete(&self, _: &RuleDescriptor, _: &FileRecord, _: &[String]) -> Result<()> {
            panic!("must never delete")
        }
        fn free_space(&self) -> Result<u64> {
            Ok(0)
        }
    }
    #[test]
    fn sqlite_full_and_readonly_startup_do_not_create_a_cleanup_engine() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA max_page_count=1").unwrap();
        let error = Engine::with_journal(Arc::new(Untouched), Scope::User, db)
            .err()
            .expect("full journal must fail");
        let sqlite = error.downcast_ref::<rusqlite::Error>().unwrap();
        assert_eq!(
            sqlite.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DiskFull)
        );
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA query_only=ON").unwrap();
        let error = Engine::with_journal(Arc::new(Untouched), Scope::User, db)
            .err()
            .expect("readonly journal must fail");
        assert_eq!(
            error
                .downcast_ref::<rusqlite::Error>()
                .unwrap()
                .sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        );
    }
}
