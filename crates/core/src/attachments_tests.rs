use super::*;
use std::time::{Duration, Instant};
struct Fake {
    blocked: bool,
    partial: bool,
    count: u64,
}
impl AnalysisPlatform for Fake {
    fn locations(&self) -> Vec<(String, String)> {
        vec![("test".into(), "account".into())]
    }
    fn validate_root(&self, _: &str) -> Result<AnalysisRoot> {
        anyhow::bail!("unused")
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        _: &AtomicBool,
        _: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        anyhow::bail!("unused")
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        anyhow::bail!("unused")
    }
    fn delete_manual(&self, record: &ManualRecord, exclusions: &[String]) -> Result<()> {
        ensure!(!exclusions.contains(&record.file.path_display), "excluded");
        Ok(())
    }
}
impl AttachmentPlatform for Fake {
    fn candidate_locations(&self, _: AttachmentProvider) -> Result<Vec<String>> {
        Ok(vec!["account".into()])
    }
    fn account(&self, provider: AttachmentProvider, path: &str) -> Result<AttachmentRoot> {
        ensure!(path == "account", "invalid root");
        Ok(AttachmentRoot {
            location: AttachmentLocation {
                provider,
                token: String::new(),
                title: "test".into(),
                path: path.into(),
                version: "4.test".into(),
                cleanup_block: self.blocked.then(|| "readonly".into()),
            },
            opaque: format!("{provider:?}:{path}"),
        })
    }
    fn scan_attachments(
        &self,
        _: &AttachmentRoot,
        _: &[String],
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(Result<AttachmentRecord>) -> Result<()>,
    ) -> Result<()> {
        for i in 0..self.count {
            ensure!(!cancel.load(Ordering::Acquire), "cancelled");
            let age = if i == 0 { now() } else { now() - 100 * 86400 };
            let kind = if i % 2 == 0 {
                AttachmentKind::Image
            } else {
                AttachmentKind::File
            };
            emit(Ok(AttachmentRecord {
                entry: AttachmentEntry {
                    id: String::new(),
                    name: format!("sample-{i}"),
                    path: format!("account/{i}"),
                    kind,
                    bytes: i.to_string(),
                    modified_unix: age,
                    preview_available: kind == AttachmentKind::Image,
                    cleanup_block: self.blocked.then(|| "readonly".into()),
                    outcome: None,
                },
                record: ManualRecord {
                    file: FileRecord {
                        path_display: format!("account/{i}"),
                        bytes: i,
                        modified_unix: age,
                        opaque: String::new(),
                    },
                    directory: false,
                    identity: i.to_string(),
                },
            }))?;
        }
        if self.partial {
            emit(Err(anyhow::anyhow!("unreadable")))?;
        }
        Ok(())
    }
    fn verify_attachment(
        &self,
        _: &AttachmentRoot,
        item: &ManualRecord,
        exclusions: &[String],
    ) -> Result<()> {
        ensure!(!exclusions.contains(&item.file.path_display), "excluded");
        Ok(())
    }
    fn attachment_thumbnail(&self, _: &ManualRecord) -> Result<String> {
        Ok("data:image/png;base64,test".into())
    }
    fn reveal_attachment(&self, _: &ManualRecord) -> Result<()> {
        Ok(())
    }
}
fn scanned(blocked: bool, partial: bool) -> (Arc<AttachmentEngine>, String) {
    scanned_count(blocked, partial, 205)
}
fn scanned_count(blocked: bool, partial: bool, count: u64) -> (Arc<AttachmentEngine>, String) {
    let engine = AttachmentEngine::new(Arc::new(Fake {
        blocked,
        partial,
        count,
    }));
    let location = engine
        .discover(AttachmentProvider::Weixin)
        .locations
        .remove(0);
    let session = engine.start(&location.token, vec![]).unwrap();
    let time = Instant::now();
    while engine.is_busy() {
        assert!(time.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    (engine, session)
}
fn query(session: &str) -> AttachmentQuery {
    AttachmentQuery {
        session: session.into(),
        kind: None,
        min_age_days: 30,
        search: String::new(),
        offset: 0,
        selected_ids: None,
    }
}

#[test]
fn provider_sessions_isolate_ids_and_report_their_trusted_source() {
    let (engine, old_session) = scanned(false, false);
    let old_id = engine.page(query(&old_session)).unwrap().entries[0]
        .id
        .clone();
    engine.close().unwrap();
    for provider in [AttachmentProvider::QqClassic, AttachmentProvider::Qqnt] {
        let discovered = engine.discover(provider);
        assert_eq!(discovered.state, "ready");
        assert_eq!(discovered.locations[0].provider, provider);
        let session = engine
            .start(&discovered.locations[0].token, vec![])
            .unwrap();
        let start = Instant::now();
        while engine.is_busy() {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
        let cancel = AtomicBool::new(false);
        assert!(
            engine
                .prepare(
                    &AttachmentSelection {
                        session: session.clone(),
                        selected_ids: vec![old_id.clone()]
                    },
                    &[],
                    &cancel
                )
                .is_err()
        );
        assert!(engine.page(query(&old_session)).is_err());
        let id = engine.page(query(&session)).unwrap().entries[0].id.clone();
        let manifest = engine
            .prepare(
                &AttachmentSelection {
                    session,
                    selected_ids: vec![id],
                },
                &[],
                &cancel,
            )
            .unwrap();
        assert_eq!(manifest.summary.source.as_deref(), Some(provider.source()));
        engine.close().unwrap();
    }
}

#[test]
fn waiting_thumbnail_does_not_repopulate_a_retired_session_cache() {
    let (engine, session) = scanned(false, false);
    let id = engine
        .page(query(&session))
        .unwrap()
        .entries
        .into_iter()
        .find(|i| i.preview_available)
        .unwrap()
        .id;
    let cache = engine.thumbnails.lock().unwrap();
    let worker = engine.clone();
    let task = std::thread::spawn(move || worker.thumbnail(&session, &id));
    // Both possible schedules are safe: the query either sees the retired session,
    // or captures the old item and must fail the generation check after this lock.
    engine.generation.fetch_add(1, Ordering::AcqRel);
    *engine.session.lock().unwrap() = None;
    drop(cache);
    assert!(task.join().unwrap().is_err());
    assert!(engine.thumbnails.lock().unwrap().is_empty());
}
#[test]
fn pagination_filters_and_selection_are_explicit() {
    let (engine, id) = scanned(false, false);
    let first = engine.page(query(&id)).unwrap();
    assert_eq!(first.total, 204);
    assert_eq!(first.entries.len(), 100);
    assert_eq!(first.entries[0].bytes, "204");
    assert_eq!(first.filtered_bytes, "20910");
    assert_eq!(first.selectable_files, 204);
    assert_eq!(first.selectable_bytes, "20910");
    assert_eq!(first.categories.iter().map(|c| c.files).sum::<usize>(), 205);
    assert_eq!(
        first
            .categories
            .iter()
            .map(|c| c.bytes.parse::<u64>().unwrap())
            .sum::<u64>(),
        20910
    );
    let mut q = query(&id);
    q.offset = 100;
    let second = engine.page(q).unwrap();
    assert_eq!(second.filtered_bytes, first.filtered_bytes);
    assert_eq!(
        second.categories.iter().map(|c| c.files).sum::<usize>(),
        205
    );
    assert!(
        !first
            .entries
            .iter()
            .any(|a| second.entries.iter().any(|b| a.id == b.id))
    );
    let selection = AttachmentSelection {
        session: id.clone(),
        selected_ids: vec![first.entries[0].id.clone(), second.entries[0].id.clone()],
    };
    let manifest = engine
        .prepare(&selection, &[], &AtomicBool::new(false))
        .unwrap();
    assert_eq!(manifest.summary.files, 2);
    assert_eq!(
        manifest.summary.source.as_deref(),
        Some("weixin.attachments")
    );
    let mut q = query(&id);
    q.selected_ids = Some(selection.selected_ids);
    q.min_age_days = 0;
    assert_eq!(engine.page(q).unwrap().total, 2);
}

#[test]
fn ten_thousand_records_keep_pagination_selection_and_deleted_accounting_separate() {
    let (engine, session) = scanned_count(false, false, 10_001);
    let mut seen = std::collections::HashSet::new();
    let mut selected = Vec::new();
    for offset in (0..10_000).step_by(100) {
        let mut request = query(&session);
        request.offset = offset;
        let page = engine.page(request).unwrap();
        assert_eq!(page.total, 10_000);
        assert_eq!(page.entries.len(), 100);
        assert_eq!(page.filtered_bytes, "50005000");
        for entry in &page.entries {
            assert!(seen.insert(entry.id.clone()));
        }
        if offset == 0 || offset == 9900 {
            selected.push(page.entries[0].id.clone());
        }
    }
    assert_eq!(seen.len(), 10_000);
    let mut filtered = query(&session);
    filtered.kind = Some(AttachmentKind::File);
    filtered.search = "sample-1".into();
    assert!(engine.page(filtered).unwrap().total > 0);
    assert!(engine.snapshot().unwrap().deleted_ids.is_empty());
    let choice = AttachmentSelection {
        session: session.clone(),
        selected_ids: selected.clone(),
    };
    let manifest = engine
        .prepare(&choice, &[], &AtomicBool::new(false))
        .unwrap();
    assert_eq!(manifest.summary.files, 2);
    let data: String = manifest
        .db
        .query_row("SELECT record FROM items LIMIT 1", [], |r| r.get(0))
        .unwrap();
    manifest
        .platform
        .delete_manual(&serde_json::from_str(&data).unwrap(), &[])
        .unwrap();
    let page = engine.page(query(&session)).unwrap();
    assert_eq!(page.total, 10_000);
    assert_eq!(
        page.categories
            .iter()
            .map(|c| c.deleted_files)
            .sum::<usize>(),
        1
    );
    assert_eq!(
        page.categories
            .iter()
            .map(|c| c.retained_files)
            .sum::<usize>(),
        0
    );
    let mut chosen = query(&session);
    chosen.selected_ids = Some(selected);
    assert_eq!(engine.page(chosen).unwrap().total, 2);
    engine.close().unwrap();
    assert!(engine.page(query(&session)).is_err());
    assert!(
        engine
            .prepare(&choice, &[], &AtomicBool::new(false))
            .is_err()
    );
}
#[test]
fn readonly_incomplete_stale_and_forged_selections_cannot_prepare() {
    for (blocked, partial) in [(true, false), (false, true)] {
        let (engine, id) = scanned(blocked, partial);
        let mut page = engine.page(query(&id)).unwrap();
        assert_eq!(page.selectable_files, 0);
        assert_eq!(page.selectable_bytes, "0");
        assert_eq!(
            page.categories
                .iter()
                .map(|c| c.retained_files)
                .sum::<usize>(),
            205
        );
        let item = page.entries.remove(0);
        assert!(
            engine
                .prepare(
                    &AttachmentSelection {
                        session: id,
                        selected_ids: vec![item.id]
                    },
                    &[],
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
    let (engine, id) = scanned(false, false);
    assert!(
        engine
            .prepare(
                &AttachmentSelection {
                    session: id.clone(),
                    selected_ids: vec!["forged".into()]
                },
                &[],
                &AtomicBool::new(false)
            )
            .is_err()
    );
    assert!(engine.thumbnail("stale", "forged").is_err());
    assert!(
        engine
            .register(AttachmentProvider::Weixin, "arbitrary-directory")
            .is_err()
    );
    let item = engine.page(query(&id)).unwrap().entries.remove(0);
    let selection = AttachmentSelection {
        session: id.clone(),
        selected_ids: vec![item.id.clone()],
    };
    assert!(
        engine
            .prepare(&selection, &[item.path], &AtomicBool::new(false))
            .is_err()
    );
    assert!(
        engine
            .prepare(&selection, &[], &AtomicBool::new(true))
            .is_err()
    );
    let duplicate = AttachmentSelection {
        session: id,
        selected_ids: vec![item.id.clone(), item.id],
    };
    assert!(
        engine
            .prepare(&duplicate, &[], &AtomicBool::new(false))
            .is_err()
    );
    engine.close().unwrap();
    assert!(
        engine
            .prepare(&selection, &[], &AtomicBool::new(false))
            .is_err()
    );
}
#[test]
fn processing_results_remove_only_successful_items() {
    let (engine, id) = scanned(false, false);
    let item = engine.page(query(&id)).unwrap().entries.remove(0);
    let selection = AttachmentSelection {
        session: id.clone(),
        selected_ids: vec![item.id.clone()],
    };
    let manifest = engine
        .prepare(&selection, &[], &AtomicBool::new(false))
        .unwrap();
    let data: String = manifest
        .db
        .query_row("SELECT record FROM items", [], |row| row.get(0))
        .unwrap();
    manifest
        .platform
        .delete_manual(&serde_json::from_str(&data).unwrap(), &[])
        .unwrap();
    assert_eq!(
        engine.snapshot().unwrap().deleted_ids,
        vec![item.id.clone()]
    );
    let page = engine.page(query(&id)).unwrap();
    assert_eq!(page.filtered_bytes, "20910"); // scan snapshot, not remaining occupancy
    assert_eq!(page.selectable_files, 203);
    let category = page
        .categories
        .iter()
        .find(|c| c.kind == item.kind)
        .unwrap();
    assert_eq!(category.deleted_files, 1);
    assert_eq!(category.deleted_bytes, item.bytes);
    assert_eq!(category.retained_files, 0);
    assert_eq!(category.retained_bytes, "0");
    let other = engine
        .session
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .items
        .iter()
        .find(|entry| entry.entry.id != item.id && entry.entry.kind == item.kind)
        .unwrap()
        .record
        .identity
        .clone();
    engine
        .outcomes
        .lock()
        .unwrap()
        .insert(other, "保留：文件已变化".into());
    let page = engine.page(query(&id)).unwrap();
    let category = page
        .categories
        .iter()
        .find(|c| c.kind == item.kind)
        .unwrap();
    assert_eq!(category.deleted_files, 1);
    assert_eq!(category.retained_files, 1);
    assert!(
        engine
            .prepare(&selection, &[], &AtomicBool::new(false))
            .is_err()
    );
}
