use super::*;
use crate::{ManualPreview, ManualSelection, ManualSummary};
use std::sync::{MutexGuard, TryLockError};

const BATCH: usize = 256;
const BATCH_BYTES: usize = 8 * 1024 * 1024;
const FIELDS: &str = "id,parent,name,path,directory,size_key,files,modified,shared,incomplete,pending,issue,rules,excluded,opaque,CASE WHEN file_key IS NULL THEN NULL ELSE hex(file_key) END";

fn check_cancelled(cancel: &AtomicBool) -> Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        ApiError::new("cancelled", "操作已停止，选择已保留")
    );
    Ok(())
}

/// Spans the entire preparation command, including rule plans and publication.
pub struct PreparationGuard {
    engine: Arc<AnalysisEngine>,
    id: String,
    cancel: Arc<AtomicBool>,
}
impl PreparationGuard {
    pub fn cancellation(&self) -> &AtomicBool {
        &self.cancel
    }
    pub fn check(&self) -> Result<()> {
        check_cancelled(&self.cancel)
    }
}
impl Drop for PreparationGuard {
    fn drop(&mut self) {
        let mut active = self.engine.preparation.lock().unwrap();
        if active.as_ref().is_some_and(|(id, _)| id == &self.id) {
            *active = None;
        }
    }
}

struct SelectedRecord {
    entry: AnalysisEntry,
    opaque: Vec<u8>,
    identity: Option<String>,
    chain: Vec<Vec<u8>>,
    depth: usize,
    allowed: bool,
}
impl SelectedRecord {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let bytes: String = row.get(5)?;
        let rules: String = row.get(12)?;
        Ok(Self {
            entry: AnalysisEntry {
                allocated_bytes: None,
                allocation_unknown: 0,
                verification_pending: false,
                manual: Default::default(),
                ancestors: vec![],
                id: row.get(0)?,
                parent: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                directory: row.get(4)?,
                bytes: bytes.trim_start_matches('0').into(),
                files: row.get::<_, i64>(6)? as u64,
                modified_unix: row.get(7)?,
                shared: row.get(8)?,
                incomplete: row.get(9)?,
                pending: row.get(10)?,
                issue: row.get(11)?,
                rule_ids: serde_json::from_str(&rules).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        12,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                excluded: row.get(13)?,
                advice: None,
            },
            opaque: row.get(14)?,
            identity: row.get(15)?,
            chain: vec![],
            depth: 0,
            allowed: false,
        })
    }
    fn bytes(&self) -> usize {
        512 + self.entry.id.len()
            + self.entry.parent.as_ref().map_or(0, String::len)
            + self.entry.name.len()
            + self.entry.path.len()
            + self.opaque.len()
            + self.identity.as_ref().map_or(0, String::len)
            + self.entry.issue.as_ref().map_or(0, String::len)
            + self.entry.rule_ids.iter().map(String::len).sum::<usize>()
            + self.chain.iter().map(|c| c.len() + 24).sum::<usize>()
    }
}
enum Frame {
    Node {
        id: String,
        depth: usize,
    },
    Children {
        parent: String,
        after: String,
        depth: usize,
    },
}
/// Bounded DFS frontier. Excluded subtrees are pruned, and reselected descendants
/// become separate roots. Overlapping choices are never visited twice.
struct SelectionWalk {
    overrides: HashMap<String, bool>,
    frames: Vec<Frame>,
    root: AnalysisRoot,
    visited: usize,
}
impl SelectionWalk {
    fn new(
        engine: &AnalysisEngine,
        selection: &ManualSelection,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        ensure!(
            selection.selected_ids.len() + selection.excluded_ids.len() <= 100_000,
            "自选项目数量无效"
        );
        let mut overrides = HashMap::new();
        for (ids, selected) in [
            (&selection.selected_ids, true),
            (&selection.excluded_ids, false),
        ] {
            for id in ids {
                ensure!(
                    overrides.insert(id.clone(), selected).is_none(),
                    "自选项目重复或冲突"
                );
            }
        }
        let mut frames = vec![];
        for (ids, selected) in [
            (&selection.selected_ids, true),
            (&selection.excluded_ids, false),
        ] {
            let mut offset = 0;
            while offset < ids.len() {
                let inner = engine.selection_index(cancel)?;
                AnalysisEngine::selection_session(&inner, &selection.session)?;
                let mut query = inner
                    .db
                    .prepare_cached(&format!("SELECT {FIELDS} FROM nodes WHERE id=?1"))?;
                let mut size = 0;
                let mut count = 0;
                while offset < ids.len() && count < BATCH {
                    let id = &ids[offset];
                    check_cancelled(cancel)?;
                    let mut record = query
                        .query_row([id], SelectedRecord::read)
                        .map_err(|_| ApiError::new("invalid_node", "文件不属于本次分析"))?;
                    AnalysisEngine::update_entry(&inner, &mut record.entry);
                    let bytes = record.bytes();
                    ensure!(
                        bytes <= BATCH_BYTES,
                        ApiError::new("resource_limit", "单项记录超出准备资源上限")
                    );
                    if size + bytes > BATCH_BYTES {
                        break;
                    }
                    size += bytes;
                    offset += 1;
                    count += 1;
                    if !selected {
                        continue;
                    }
                    ensure!(
                        engine
                            .platform
                            .manual_record_capability(&record.entry, &record.opaque)
                            .allowed,
                        "所选项目不能删除，请刷新结果"
                    );
                    let mut parent = record.entry.parent;
                    let mut inherited = None;
                    let mut depth = 1;
                    while let Some(id) = parent {
                        if inherited.is_none() {
                            inherited = overrides.get(&id).copied();
                        }
                        depth += 1;
                        ensure!(depth <= ANALYSIS_DEPTH + 1, "目录链无效");
                        parent = parent_of(&inner, &id)?;
                    }
                    if inherited != Some(true) {
                        frames.push(Frame::Node {
                            id: id.clone(),
                            depth,
                        });
                    }
                }
            }
        }
        let inner = engine.selection_index(cancel)?;
        AnalysisEngine::selection_session(&inner, &selection.session)?;
        Ok(Self {
            overrides,
            frames,
            root: inner
                .root
                .clone()
                .ok_or_else(|| anyhow::anyhow!("分析根目录不可用"))?,
            visited: 0,
        })
    }

    fn batch(
        &mut self,
        engine: &AnalysisEngine,
        session: &str,
        cancel: &AtomicBool,
        prepare: bool,
    ) -> Result<Vec<SelectedRecord>> {
        let inner = engine.selection_index(cancel)?;
        AnalysisEngine::selection_session(&inner, session)?;
        let mut records = vec![];
        let mut size = 0;
        let mut read = 0;
        let mut cached_parent: Option<(Option<String>, Vec<Vec<u8>>)> = None;
        while read < BATCH {
            let Some(frame) = self.frames.pop() else {
                break;
            };
            let (sql, key, after, depth, children) = match frame {
                Frame::Node { id, depth } => (
                    format!("SELECT {FIELDS} FROM nodes WHERE id=?1 AND ?2='' LIMIT ?3"),
                    id,
                    String::new(),
                    depth,
                    false,
                ),
                Frame::Children {
                    parent,
                    after,
                    depth,
                } => (
                    format!(
                        "SELECT {FIELDS} FROM nodes WHERE parent=?1 AND id>?2 ORDER BY id LIMIT ?3"
                    ),
                    parent,
                    after,
                    depth,
                    true,
                ),
            };
            let mut query = inner.db.prepare_cached(&sql)?;
            let mut rows = query.query(params![key, after, (BATCH - read) as i64])?;
            let mut cursor = after;
            let mut descendants = vec![];
            let mut exhausted = false;
            let mut full = false;
            while read < BATCH {
                check_cancelled(cancel)?;
                let Some(row) = rows.next()? else {
                    exhausted = true;
                    break;
                };
                let mut record = SelectedRecord::read(row)?;
                if children && self.overrides.get(&record.entry.id) == Some(&false) {
                    cursor.clone_from(&record.entry.id);
                    read += 1;
                    self.visited += 1;
                    continue;
                }
                AnalysisEngine::update_entry(&inner, &mut record.entry);
                record.depth = depth;
                ensure!(depth <= ANALYSIS_DEPTH + 1, "目录链无效");
                record.allowed = engine
                    .platform
                    .manual_record_capability(&record.entry, &record.opaque)
                    .allowed;
                if prepare && record.allowed {
                    if cached_parent.as_ref().map(|(p, _)| p) != Some(&record.entry.parent) {
                        let chain = match &record.entry.parent {
                            Some(id) => AnalysisEngine::validation_chain(&inner, id)?,
                            None => vec![],
                        };
                        cached_parent = Some((record.entry.parent.clone(), chain));
                    }
                    record.chain = cached_parent.as_ref().unwrap().1.clone();
                    record.chain.push(record.opaque.clone());
                }
                let record_bytes = record.bytes();
                ensure!(
                    record_bytes <= BATCH_BYTES,
                    ApiError::new("resource_limit", "单项记录超出准备资源上限")
                );
                if size + record_bytes > BATCH_BYTES {
                    full = true;
                    break;
                }
                size += record_bytes;
                read += 1;
                self.visited += 1;
                cursor.clone_from(&record.entry.id);
                if record.entry.directory {
                    descendants.push(Frame::Children {
                        parent: record.entry.id.clone(),
                        after: String::new(),
                        depth: depth + 1,
                    });
                }
                records.push(record);
                if !children {
                    exhausted = true;
                    break;
                }
            }
            if !exhausted {
                self.frames.push(if children {
                    Frame::Children {
                        parent: key,
                        after: cursor,
                        depth,
                    }
                } else {
                    Frame::Node { id: key, depth }
                });
            }
            self.frames.extend(descendants.into_iter().rev());
            if full {
                break;
            }
        }
        // SQLite rows and the index lock are dropped before filesystem verification.
        Ok(records)
    }
}

impl AnalysisEngine {
    pub fn begin_manual_preparation(self: &Arc<Self>, id: &str) -> Result<PreparationGuard> {
        ensure!(uuid::Uuid::parse_str(id).is_ok(), "准备操作标识无效");
        let mut active = self.preparation.lock().unwrap();
        ensure!(
            active.is_none(),
            ApiError::new("busy", "清单仍在准备或停止中")
        );
        let cancelled = self.preparation_pending.lock().unwrap().remove(id);
        let cancel = Arc::new(AtomicBool::new(cancelled));
        *active = Some((id.into(), cancel.clone()));
        Ok(PreparationGuard {
            engine: self.clone(),
            id: id.into(),
            cancel,
        })
    }
    pub fn cancel_manual_preparation(&self, id: &str) -> Result<()> {
        ensure!(
            uuid::Uuid::parse_str(id).is_ok(),
            ApiError::new("invalid_operation", "准备操作标识无效")
        );
        let active = self.preparation.lock().unwrap();
        let Some((current, cancel)) = &*active else {
            // IPC delivery can overtake preparation registration. Remember only this
            // UUID, with a hard bound; never cancel a different active operation.
            let mut pending = self.preparation_pending.lock().unwrap();
            ensure!(
                pending.contains(id) || pending.len() < 128,
                ApiError::new("resource_limit", "等待受理的停止请求过多")
            );
            pending.insert(id.to_owned());
            return Ok(());
        };
        ensure!(
            current == id,
            ApiError::new("invalid_operation", "准备操作标识不匹配")
        );
        cancel.store(true, Ordering::Release);
        Ok(())
    }
    pub fn begin_manual_preview(&self, request_id: &str) -> Result<Arc<AtomicBool>> {
        ensure!(
            uuid::Uuid::parse_str(request_id).is_ok(),
            "统计请求标识无效"
        );
        let flag = Arc::new(AtomicBool::new(false));
        if let Some((_, old)) = self
            .preview_cancelled
            .lock()
            .unwrap()
            .replace((request_id.into(), flag.clone()))
        {
            old.store(true, Ordering::Release);
        }
        Ok(flag)
    }
    pub fn cancel_manual_preview(&self, request_id: &str) {
        if let Some((id, cancel)) = &*self.preview_cancelled.lock().unwrap()
            && id == request_id
        {
            cancel.store(true, Ordering::Release);
        }
    }
    fn selection_index(&self, cancel: &AtomicBool) -> Result<MutexGuard<'_, Inner>> {
        loop {
            check_cancelled(cancel)?;
            match self.inner.try_lock() {
                Ok(inner) => return Ok(inner),
                Err(TryLockError::Poisoned(error)) => return Err(anyhow::anyhow!("{error}")),
                Err(TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(2))
                }
            }
        }
    }
    fn selection_session(inner: &Inner, session: &str) -> Result<()> {
        Self::check_session(inner, session)?;
        ensure!(
            inner
                .snapshot
                .as_ref()
                .is_some_and(|s| s.state != "scanning" && !s.stale),
            ApiError::new("invalid_session", "分析结果已变化，请重新选择")
        );
        Ok(())
    }
    pub fn validate_manual_session(&self, session: &str) -> Result<()> {
        Self::selection_session(&self.inner.lock().unwrap(), session)
    }
    pub fn preview_manual(
        &self,
        selection: &ManualSelection,
        request_id: String,
        cancel: Arc<AtomicBool>,
    ) -> Result<ManualPreview> {
        let mut walk = SelectionWalk::new(self, selection, &cancel)?;
        let mut summary = ManualSummary {
            session: selection.session.clone(),
            bytes: "0".into(),
            ..Default::default()
        };
        let identities = Connection::open_in_memory()?;
        identities.execute_batch("PRAGMA page_size=4096; PRAGMA max_page_count=16384; PRAGMA temp_store=MEMORY; CREATE TABLE identities(id TEXT PRIMARY KEY) WITHOUT ROWID;")?;
        let mut bytes = 0u64;
        while !walk.frames.is_empty() {
            for record in walk.batch(self, &selection.session, &cancel, false)? {
                check_cancelled(&cancel)?;
                if !record.allowed {
                    summary.retained += 1;
                    continue;
                }
                if record.entry.directory {
                    summary.directories += 1;
                    continue;
                }
                let Some(key) = record.identity else {
                    summary.retained += 1;
                    continue;
                };
                if identities
                    .prepare_cached("INSERT OR IGNORE INTO identities VALUES(?1)")?
                    .execute([key])?
                    == 1
                {
                    bytes = bytes
                        .checked_add(record.entry.bytes.parse::<u64>()?)
                        .ok_or_else(|| anyhow::anyhow!("文件大小超出范围"))?;
                    summary.files += 1;
                }
            }
            std::thread::yield_now();
        }
        Self::selection_session(&*self.selection_index(&cancel)?, &selection.session)?;
        check_cancelled(&cancel)?;
        summary.bytes = bytes.to_string();
        Ok(ManualPreview {
            request_id,
            summary,
        })
    }
    pub fn prepare_manual_cancellable(
        &self,
        selection: &ManualSelection,
        cancel: &AtomicBool,
    ) -> Result<crate::ManualManifest> {
        ensure!(!selection.selected_ids.is_empty(), "自选项目数量无效");
        let mut walk = SelectionWalk::new(self, selection, cancel)?;
        let mut manifest =
            crate::ManualManifest::new(self.platform.clone(), selection.session.clone())?;
        while !walk.frames.is_empty() {
            for record in walk.batch(self, &selection.session, cancel, true)? {
                check_cancelled(cancel)?;
                if !record.allowed {
                    manifest.summary.retained += 1;
                    continue;
                }
                let verified = self.platform.manual_record(&walk.root, &record.chain);
                check_cancelled(cancel)?;
                match verified {
                    Ok(item) => manifest.insert(&record.entry.id, record.depth, item)?,
                    Err(_) => manifest.summary.retained += 1,
                }
            }
            std::thread::yield_now();
        }
        Self::selection_session(&*self.selection_index(cancel)?, &selection.session)?;
        check_cancelled(cancel)?;
        ensure!(
            manifest.summary.files + manifest.summary.directories > 0,
            "所选内容已变化或均需保留，请重新扫描"
        );
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::super::manual_tests::setup;
    use super::*;
    #[test]
    fn sparse_choices_visit_only_selected_records_and_batch_bytes_are_bounded() {
        let (_, engine, session) = setup(1000, false);
        for count in [1, 200] {
            let selection = ManualSelection {
                session: session.clone(),
                selected_ids: (2..2 + count).map(|n| n.to_string()).collect(),
                excluded_ids: vec![],
            };
            let cancel = AtomicBool::new(false);
            let mut walk = SelectionWalk::new(&engine, &selection, &cancel).unwrap();
            while !walk.frames.is_empty() {
                walk.batch(&engine, &session, &cancel, false).unwrap();
            }
            assert_eq!(walk.visited, count);
        }
        engine
            .inner
            .lock()
            .unwrap()
            .db
            .execute(
                "UPDATE nodes SET opaque=zeroblob(3000000) WHERE id IN ('2','3','4')",
                [],
            )
            .unwrap();
        let selection = ManualSelection {
            session: session.clone(),
            selected_ids: vec!["2".into(), "3".into(), "4".into()],
            excluded_ids: vec![],
        };
        let cancel = AtomicBool::new(false);
        let mut walk = SelectionWalk::new(&engine, &selection, &cancel).unwrap();
        assert_eq!(
            walk.batch(&engine, &session, &cancel, false).unwrap().len(),
            2
        );
        assert_eq!(
            walk.batch(&engine, &session, &cancel, false).unwrap().len(),
            1
        );
        assert_eq!(walk.visited, 3);
    }
    #[test]
    fn preparation_tokens_isolate_late_cancellation_and_session_changes() {
        let (_, engine, session) = setup(2, false);
        let old = uuid::Uuid::new_v4().to_string();
        let current = uuid::Uuid::new_v4().to_string();
        let guard = engine.begin_manual_preparation(&old).unwrap();
        assert!(engine.begin_manual_preparation(&current).is_err());
        assert!(engine.cancel_manual_preparation("").is_err());
        guard.check().unwrap();
        engine.cancel_manual_preparation(&old).unwrap();
        assert!(guard.check().is_err());
        drop(guard);
        let guard = engine.begin_manual_preparation(&current).unwrap();
        assert!(engine.cancel_manual_preparation(&old).is_err());
        engine.cancel(&session).unwrap(); // scan cancellation is isolated
        guard.check().unwrap();
        let selection = ManualSelection {
            session: session.clone(),
            selected_ids: vec!["1".into()],
            excluded_ids: vec![],
        };
        let mut walk = SelectionWalk::new(&engine, &selection, guard.cancellation()).unwrap();
        engine.mark_stale();
        assert!(
            walk.batch(&engine, &session, guard.cancellation(), true)
                .is_err()
        );
    }
    #[test]
    fn stop_before_registration_is_remembered_only_for_its_operation() {
        let (_, engine, _) = setup(2, false);
        let stopped = uuid::Uuid::new_v4().to_string();
        engine.cancel_manual_preparation(&stopped).unwrap();
        let other = engine
            .begin_manual_preparation(&uuid::Uuid::new_v4().to_string())
            .unwrap();
        other.check().unwrap();
        drop(other);
        let operation = engine.begin_manual_preparation(&stopped).unwrap();
        assert!(operation.check().is_err());
        assert!(engine.preparation_pending.lock().unwrap().is_empty());
        drop(operation);
        for _ in 0..128 {
            engine
                .cancel_manual_preparation(&uuid::Uuid::new_v4().to_string())
                .unwrap();
        }
        assert!(
            engine
                .cancel_manual_preparation(&uuid::Uuid::new_v4().to_string())
                .is_err()
        );
        let operation = engine
            .begin_manual_preparation(&uuid::Uuid::new_v4().to_string())
            .unwrap();
        operation.check().unwrap();
    }
}
