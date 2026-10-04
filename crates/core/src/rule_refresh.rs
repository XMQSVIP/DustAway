//! A refresh stages through the same scanner as a full scan; only its commit is different.
use super::*;

pub fn supported(id: &str) -> bool {
    matches!(id, "dev.uv" | "dev.go_build" | "dev.yarn" | "dev.ccache")
}

impl Engine {
    pub fn tool_scan(&self, session: &str, rule: &str) -> Result<RuleSummary> {
        let inner = self.inner.lock().unwrap();
        ensure!(
            !busy(&inner.status),
            ApiError::new("busy", "请等待当前任务结束")
        );
        ensure!(
            inner.status.id == session
                && !session.is_empty()
                && inner.status.state == JobState::Ready,
            "扫描会话已变化"
        );
        let row = inner
            .summary
            .iter()
            .find(|r| r.rule_id == rule)
            .context("本次未扫描该工具")?;
        ensure!(
            supported(rule) && !row.stale,
            "此项结果待更新，请重新检查这一项"
        );
        ensure!(
            row.phase == ScanPhase::Completed && row.coverage.as_ref().is_some_and(|c| c.complete),
            "缓存读取不完整，请重新检查这一项"
        );
        ensure!(
            row.tool.as_ref().is_some_and(|t| t.ready),
            "实际处理位置或工具状态未确认，请重新检查这一项"
        );
        Ok(row.clone())
    }

    pub fn mark_tool_started(
        &self,
        session: &str,
        rule: &str,
        revision: u64,
        task: &str,
    ) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        ensure!(inner.status.id == session, "扫描会话已变化");
        let row = inner
            .summary
            .iter_mut()
            .find(|r| r.rule_id == rule)
            .context("未知规则")?;
        ensure!(row.revision == revision, "此项扫描已变化");
        row.stale = true;
        row.pending_tool_task = Some(task.to_owned());
        Ok(())
    }

    pub fn start_rule_refresh(self: &Arc<Self>, session: &str, rule: &str) -> Result<String> {
        ensure!(
            self.scope == Scope::User && supported(rule),
            ApiError::new("invalid_rule", "此项不支持单项复扫")
        );
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            !busy(&inner.status),
            ApiError::new("busy", "已有任务正在运行")
        );
        ensure!(
            inner.status.id == session && inner.status.state == JobState::Ready,
            ApiError::new("invalid_session", "需要完整扫描后才能检查这一项")
        );
        ensure!(
            !inner.settings.excluded_rules.iter().any(|id| id == rule),
            "此项已排除"
        );
        let row = inner
            .summary
            .iter_mut()
            .find(|r| r.rule_id == rule)
            .context("本次未扫描该工具")?;
        let revision = row.revision;
        row.stale = true;
        // Consumed at submission, including failed/cancelled refreshes. Reconnection never retries.
        row.pending_tool_task = None;
        let id = Uuid::new_v4().to_string();
        *self.refresh_cancel.lock().unwrap() = Some((session.into(), id.clone()));
        inner.plan = None;
        inner.manual = None;
        inner.db.execute("DELETE FROM plan_items", [])?;
        inner.status.rule_refresh = Some(RuleRefresh {
            id: id.clone(),
            rule_id: rule.into(),
            revision,
            state: JobState::Scanning,
            progress: ScanProgress::default(),
            error: None,
        });
        let settings = inner.settings.clone();
        let volumes = inner.scan_volumes.clone();
        self.cancelled.store(false, Ordering::Release);
        drop(inner);
        let engine = self.clone();
        let rule = rule.to_owned();
        let task = id.clone();
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.refresh_worker(&task, &rule, revision, settings, volumes)
            }));
            if let Err(error) = outcome.unwrap_or_else(|_| Err(anyhow::anyhow!("检查异常中断")))
            {
                let mut inner = engine.inner.lock().unwrap();
                if let Some(refresh) = inner.status.rule_refresh.as_mut().filter(|r| r.id == task) {
                    refresh.state = if engine.cancelled.load(Ordering::Acquire) {
                        JobState::Cancelled
                    } else {
                        JobState::Failed
                    };
                    refresh.error = Some(format!("{error:#}；旧结果已保留，请重新检查这一项"));
                }
            }
            let mut ticket = engine.refresh_cancel.lock().unwrap();
            if ticket.as_ref().is_some_and(|(_, id)| id == &task) {
                ticket.take();
            }
        });
        Ok(id)
    }

    pub fn cancel_rule_refresh(&self, session: &str, task: &str) -> Result<()> {
        let ticket = self.refresh_cancel.lock().unwrap();
        ensure!(
            ticket
                .as_ref()
                .is_some_and(|(s, id)| s == session && id == task),
            ApiError::new("invalid_task", "检查任务已变化，未停止其他任务")
        );
        self.cancel();
        Ok(())
    }

    fn refresh_worker(
        &self,
        task: &str,
        rule: &str,
        revision: u64,
        settings: Settings,
        volumes: Option<Vec<VolumeTarget>>,
    ) -> Result<()> {
        let staging = Engine::with_journal(
            self.platform.clone(),
            self.scope,
            Connection::open_in_memory()?,
        )?;
        staging.inner.lock().unwrap().settings = settings;
        staging.start_scan_with_context(vec![rule.to_owned()], volumes, false)?;
        while staging.is_busy() {
            if self.cancelled.load(Ordering::Acquire) {
                staging.cancel();
            }
            let progress = staging.status().scan_progress;
            if let Some(refresh) = self.inner.lock().unwrap().status.rule_refresh.as_mut() {
                refresh.progress = progress;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        ensure!(!self.cancelled.load(Ordering::Acquire), "检查已取消");
        let staged = staging.inner.lock().unwrap();
        let mut row = staged.summary.first().context("未取得检查结果")?.clone();
        ensure!(
            staged.status.state == JobState::Ready
                && row.phase == ScanPhase::Completed
                && row.coverage.as_ref().is_some_and(|c| c.complete),
            "缓存读取未完整完成"
        );
        // Discovery failure cannot replace verified old data, even when standard remnants scanned.
        ensure!(
            row.tool
                .as_ref()
                .is_some_and(|t| t.target.is_some() && t.identity.is_some()),
            "实际处理位置未确认"
        );
        let mut inner = self.inner.lock().unwrap();
        ensure!(
            inner
                .status
                .rule_refresh
                .as_ref()
                .is_some_and(|r| r.id == task),
            "检查任务已变化"
        );
        let index = inner
            .summary
            .iter()
            .position(|r| r.rule_id == rule && r.revision == revision)
            .context("此项扫描已变化")?;
        row.revision = revision.checked_add(1).context("扫描修订超出范围")?;
        let tx = inner.db.transaction()?;
        tx.execute("DELETE FROM candidates WHERE rule=?1", [rule])?;
        tx.execute("DELETE FROM observations WHERE rule=?1", [rule])?;
        {
            let mut source = staged
                .db
                .prepare("SELECT rule,path,size_key,record FROM observations WHERE rule=?1")?;
            let records = source.query_map([rule], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?;
            for record in records {
                let (rule, path, size, data) = record?;
                tx.execute(
                    "INSERT INTO observations VALUES(?1,?2,?3,?4)",
                    params![rule, path, size, data],
                )?;
            }
        }
        {
            let mut source = staged.db.prepare("SELECT id,rule,record,status,reason,reason_code,path,size_key,modified FROM candidates WHERE rule=?1")?;
            let mut records = source.query([rule])?;
            let mut insert = tx.prepare("INSERT INTO candidates(id,rule,record,status,reason,reason_code,path,size_key,modified) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)")?;
            while let Some(record) = records.next()? {
                ensure!(!self.cancelled.load(Ordering::Acquire), "检查已取消");
                let values = (0..9)
                    .map(|i| record.get::<_, rusqlite::types::Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                insert.execute(rusqlite::params_from_iter(values))?;
            }
        }
        ensure!(!self.cancelled.load(Ordering::Acquire), "检查已取消");
        tx.commit()?;
        inner.summary[index] = row;
        inner.status.visited = inner.summary.iter().map(|r| r.files).sum();
        let refresh = inner.status.rule_refresh.as_mut().unwrap();
        refresh.state = JobState::Finished;
        refresh.progress = staged.status.scan_progress.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Fake {
        mode: AtomicUsize,
        volumes: Mutex<Vec<Option<Vec<VolumeTarget>>>>,
    }
    fn tool() -> ToolDiscovery {
        ToolDiscovery {
            executable: Some("C:\\tool.exe".into()),
            entry: None,
            version: Some("1.0".into()),
            target: Some("C:\\cache".into()),
            identity: Some("same".into()),
            ready: true,
            reason: None,
            message: "ready".into(),
        }
    }
    impl Platform for Fake {
        fn scan(
            &self,
            rule: &RuleDescriptor,
            _: &[String],
            cancel: &AtomicBool,
            emit: &mut dyn FnMut(ScanEvent),
        ) -> Result<()> {
            let mode = if rule.id == "dev.uv" {
                self.mode.load(Ordering::Acquire)
            } else {
                1
            };
            if mode == 3 {
                while !cancel.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                return Ok(());
            }
            for n in 0..(if mode == 4 { 0 } else { 2 }) {
                emit(ScanEvent::File(FileRecord {
                    path_display: format!("C:\\{}\\{mode}-{n}", rule.id),
                    bytes: 12,
                    modified_unix: 1,
                    opaque: "{}".into(),
                }));
            }
            if mode == 2 {
                emit(ScanEvent::CoverageIssue(ReasonCode::AccessDenied));
            }
            if mode == 5 {
                anyhow::bail!("injected read failure");
            }
            Ok(())
        }
        fn scan_on_volumes(
            &self,
            rule: &RuleDescriptor,
            exclusions: &[String],
            volumes: Option<&[VolumeTarget]>,
            cancel: &AtomicBool,
            emit: &mut dyn FnMut(ScanEvent),
        ) -> Result<()> {
            self.volumes
                .lock()
                .unwrap()
                .push(volumes.map(|v| v.to_vec()));
            self.scan(rule, exclusions, cancel, emit)
        }
        fn tool_discovery(&self, id: &str) -> Option<ToolDiscovery> {
            (id == "dev.uv").then(tool)
        }
        fn blocked(&self, _: &RuleDescriptor) -> Result<Option<BlockReason>> {
            Ok(None)
        }
        fn delete(&self, _: &RuleDescriptor, _: &FileRecord, _: &[String]) -> Result<()> {
            panic!("refresh must never delete")
        }
        fn free_space(&self) -> Result<u64> {
            Ok(100)
        }
    }
    fn wait(e: &Engine) {
        let start = Instant::now();
        while e.is_busy() {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn setup() -> (Arc<Engine>, Arc<Fake>, String) {
        let platform = Arc::new(Fake {
            mode: AtomicUsize::new(1),
            volumes: Mutex::new(vec![]),
        });
        let engine = Engine::with_journal(
            platform.clone(),
            Scope::User,
            Connection::open_in_memory().unwrap(),
        )
        .unwrap();
        let session = engine
            .start_scan_on_volumes(
                vec!["windows.user_temp".into(), "dev.uv".into()],
                Some(vec![VolumeTarget {
                    key: "C".into(),
                    label: "C:\\".into(),
                    opaque: "native-C".into(),
                }]),
            )
            .unwrap();
        wait(&engine);
        (engine, platform, session)
    }
    fn ids(engine: &Engine, session: &str, rule: &str) -> Vec<String> {
        engine
            .candidates(session, rule, 0, 200)
            .unwrap()
            .items
            .into_iter()
            .map(|r| r.id)
            .collect()
    }
    #[test]
    fn atomic_refresh_preserves_other_candidates_and_consumes_old_task_once() {
        let (e, platform, session) = setup();
        let keep = ids(&e, &session, "windows.user_temp");
        let old = ids(&e, &session, "dev.uv");
        e.mark_tool_started(&session, "dev.uv", 1, "completed-tool")
            .unwrap();
        e.start_rule_refresh(&session, "dev.uv").unwrap();
        wait(&e);
        assert_eq!(ids(&e, &session, "windows.user_temp"), keep);
        assert_ne!(ids(&e, &session, "dev.uv"), old);
        let row = e.tool_scan(&session, "dev.uv").unwrap();
        assert_eq!((row.revision, row.files, row.bytes.as_str()), (2, 2, "24"));
        assert!(row.pending_tool_task.is_none());
        assert!(
            e.mark_tool_started(&session, "dev.uv", 1, "late-old-task")
                .is_err()
        );
        assert!(
            platform
                .volumes
                .lock()
                .unwrap()
                .iter()
                .all(|v| v.as_ref().unwrap()[0].key == "C")
        );
        platform.mode.store(4, Ordering::Release);
        e.start_rule_refresh(&session, "dev.uv").unwrap();
        wait(&e);
        assert_eq!(e.tool_scan(&session, "dev.uv").unwrap().files, 0);
        assert_eq!(ids(&e, &session, "windows.user_temp"), keep);
    }
    #[test]
    fn failed_partial_and_cancelled_refreshes_keep_old_data_and_require_explicit_retry() {
        let (e, platform, session) = setup();
        let old = ids(&e, &session, "dev.uv");
        for mode in [2, 5, 3] {
            platform.mode.store(mode, Ordering::Release);
            let task = e.start_rule_refresh(&session, "dev.uv").unwrap();
            if mode == 3 {
                assert!(e.start_rule_refresh(&session, "dev.uv").is_err());
                assert!(e.cancel_rule_refresh(&session, "late-cancel").is_err());
                assert!(
                    e.patch_settings(SettingsPatch {
                        exclusions: Some(vec!["C:\\keep".into()]),
                        ..Default::default()
                    })
                    .is_err()
                );
                e.cancel_rule_refresh(&session, &task).unwrap();
            }
            wait(&e);
            assert_eq!(ids(&e, &session, "dev.uv"), old);
            assert!(
                e.summary()
                    .rules
                    .iter()
                    .find(|r| r.rule_id == "dev.uv")
                    .unwrap()
                    .stale
            );
            assert!(e.tool_scan(&session, "dev.uv").is_err());
            assert_eq!(e.status().state, JobState::Ready);
        }
        platform.mode.store(1, Ordering::Release);
        e.start_rule_refresh(&session, "dev.uv").unwrap();
        wait(&e);
        assert!(e.tool_scan(&session, "dev.uv").is_ok());
        assert!(e.start_rule_refresh(&session, "dev.pnpm").is_err());
        assert!(e.start_rule_refresh("old-session", "dev.uv").is_err());
    }
    #[test]
    fn refresh_invalidates_prepared_file_plans_and_blocks_new_prepare() {
        let (e, platform, session) = setup();
        let selection = || Selection {
            rule_ids: vec!["windows.user_temp".into()],
            excluded_ids: vec![],
            included_ids: vec![],
            reviewed_rule_ids: vec![],
        };
        let plan = e.prepare(&session, selection()).unwrap();
        platform.mode.store(3, Ordering::Release);
        let task = e.start_rule_refresh(&session, "dev.uv").unwrap();
        assert!(e.prepare(&session, selection()).is_err());
        assert!(e.execute(&plan.id).is_err());
        e.cancel_rule_refresh(&session, &task).unwrap();
        wait(&e);
        assert!(e.execute(&plan.id).is_err());
        assert!(e.prepare(&session, selection()).is_ok());
    }
}
