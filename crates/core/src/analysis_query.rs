//! Independent, cancellable WAL readers and keyset pagination.
use super::*;
use rusqlite::{OpenFlags, params_from_iter, types::Value};

// Opt-in benchmark diagnostics. The desktop build does not enable this feature.
#[cfg(feature = "analysis-query-timing")]
static QUERY_TIMINGS: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());
#[cfg(feature = "analysis-query-timing")]
pub fn take_analysis_query_timings() -> Vec<serde_json::Value> {
    std::mem::take(&mut *QUERY_TIMINGS.lock().unwrap())
}
#[cfg(feature = "analysis-query-timing")]
struct QueryTiming {
    kind: &'static str,
    started: Instant,
    last: Instant,
    phases: Vec<(&'static str, u128)>,
}
#[cfg(feature = "analysis-query-timing")]
impl QueryTiming {
    fn new(kind: &'static str) -> Self {
        Self {
            kind,
            started: Instant::now(),
            last: Instant::now(),
            phases: vec![],
        }
    }
    fn mark(&mut self, name: &'static str) {
        self.phases.push((name, self.last.elapsed().as_micros()));
        self.last = Instant::now();
    }
}
#[cfg(feature = "analysis-query-timing")]
impl Drop for QueryTiming {
    fn drop(&mut self) {
        self.mark("drop");
        let value = serde_json::json!({"query_timing":self.kind,"total_us":self.started.elapsed().as_micros(),"phases":self.phases});
        let mut records = QUERY_TIMINGS.lock().unwrap();
        if records.len() < 4096 {
            records.push(value);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReadQuery {
    pub request_id: String,
    pub session: String,
    pub view: String,
    pub node: String,
    pub search: String,
    pub metric: String,
    pub descending: bool,
    pub discovery: Option<AnalysisDiscoveryQuery>,
    pub cursor: Option<String>,
    pub offset: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReadPage {
    pub request_id: String,
    pub generated_at: i64,
    pub items: Vec<AnalysisEntry>,
    pub total: Option<u64>,
    pub offset: u64,
    pub next_cursor: Option<String>,
    pub node: Option<AnalysisEntry>,
    pub breadcrumbs: Vec<AnalysisEntry>,
    pub coverage: AnalysisCoverage,
}
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisCount {
    pub request_id: String,
    pub total: Option<u64>,
    pub finished: bool,
    pub error: Option<String>,
}
// One completed count only: bounded memory, keyed by the complete filter and
// publication revision. Never reuse a count across changing scan snapshots.
pub(super) struct CachedCount {
    key: String,
    revision: u64,
    total: u64,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    key: String,
    revision: u64,
    pending: bool,
    size: String,
    name: String,
    id: String,
}
#[derive(Clone)]
pub(super) struct ReaderDescriptor {
    pub storage: Arc<crate::analysis_storage::SessionStorage>,
    pub root: AnalysisRoot,
    pub session: String,
}
pub(super) fn persist_snapshot(inner: &Inner) -> Result<()> {
    if let Some(snapshot) = &inner.snapshot {
        inner.db.execute("INSERT INTO publication(id,snapshot,revision) VALUES(1,?1,1) ON CONFLICT(id) DO UPDATE SET snapshot=excluded.snapshot,revision=revision+1",[serde_json::to_string(snapshot)?])?;
    }
    Ok(())
}
impl AnalysisEngine {
    pub fn cancel_query(&self, id: &str) {
        if uuid::Uuid::parse_str(id).is_err() {
            return;
        }
        let active = self.query_cancelled.lock().unwrap();
        if let Some((current, cancel)) = &*active
            && current == id
        {
            cancel.store(true, Ordering::Release);
            return;
        }
        let mut pending = self.query_pending.lock().unwrap();
        if !pending.iter().any(|p| p == id) {
            if pending.len() >= 128 {
                pending.pop_front();
            }
            pending.push_back(id.into());
        }
    }
    /// Register before dispatching the potentially delayed IPC worker.
    pub fn begin_query(&self, id: &str) -> Result<Arc<AtomicBool>> {
        uuid::Uuid::parse_str(id)?;
        let mut active = self.query_cancelled.lock().unwrap();
        let mut pending = self.query_pending.lock().unwrap();
        let cancelled = if let Some(index) = pending.iter().position(|p| p == id) {
            pending.remove(index);
            true
        } else {
            false
        };
        let flag = Arc::new(AtomicBool::new(cancelled));
        if !cancelled && let Some((_, old)) = active.replace((id.into(), flag.clone())) {
            old.store(true, Ordering::Release);
        }
        Ok(flag)
    }
    pub fn query_count(&self, id: &str) -> Result<AnalysisCount> {
        self.count_result
            .lock()
            .unwrap()
            .clone()
            .filter(|r| r.request_id == id)
            .ok_or_else(|| ApiError::new("invalid_query", "查询已变化").into())
    }
    fn read_index(&self, descriptor: &ReaderDescriptor) -> Result<(Inner, u64)> {
        #[cfg(feature = "analysis-query-timing")]
        let mut timing = QueryTiming::new("reader");
        let db = Connection::open_with_flags(
            descriptor.storage.path.join("index.sqlite"),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("open");
        db.execute_batch("PRAGMA cache_size=-32768; PRAGMA temp_store=MEMORY; PRAGMA query_only=ON; BEGIN DEFERRED;")?;
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("pragmas");
        let (snapshot, revision): (String, i64) = db.query_row(
            "SELECT snapshot,revision FROM publication WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("snapshot");
        let snapshot: AnalysisSnapshot = serde_json::from_str(&snapshot)?;
        ensure!(
            snapshot.id == descriptor.session,
            ApiError::new("invalid_session", "分析会话已变化")
        );
        Ok((
            Inner {
                db,
                storage: Some(descriptor.storage.clone()),
                locations: HashMap::new(),
                root: Some(descriptor.root.clone()),
                snapshot: Some(snapshot),
                started: None,
                parents: HashMap::new(),
                totals: HashMap::new(),
                exclusions: vec![],
            },
            revision as u64,
        ))
    }
    pub fn read_page(self: &Arc<Self>, query: AnalysisReadQuery) -> Result<AnalysisReadPage> {
        let cancel = self.begin_query(&query.request_id)?;
        self.read_registered_page(query, cancel)
    }
    pub fn read_registered_page(
        self: &Arc<Self>,
        query: AnalysisReadQuery,
        cancel: Arc<AtomicBool>,
    ) -> Result<AnalysisReadPage> {
        #[cfg(feature = "analysis-query-timing")]
        let mut timing = QueryTiming::new("page");
        uuid::Uuid::parse_str(&query.request_id)?;
        ensure!(
            matches!(query.view.as_str(), "directories" | "files" | "discovery")
                && matches!(query.metric.as_str(), "logical" | "allocated")
                && query.search.chars().count() <= 256
                && query.offset <= self.limit,
            ApiError::new("invalid_query", "查询条件无效")
        );
        let _gate = self.query_gate.lock().unwrap();
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("gate");
        ensure!(
            !cancel.load(Ordering::Acquire),
            ApiError::new("cancelled", "查询已停止")
        );
        let descriptor = self
            .reader
            .lock()
            .unwrap()
            .clone()
            .filter(|r| r.session == query.session)
            .ok_or_else(|| ApiError::new("invalid_session", "分析会话已变化"))?;
        let (inner, revision) = self.read_index(&descriptor)?;
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("reader");
        let _cancellation = SqlCancellation::new(&inner.db, cancel.clone(), true)?;
        let snapshot = inner.snapshot.as_ref().unwrap();
        let metric = if query.metric == "allocated" {
            "allocation_key"
        } else {
            "size_key"
        };
        let size = match (query.metric.as_str(), query.descending) {
            ("allocated", true) => "allocation_inverse",
            ("allocated", false) => "allocation_key",
            (_, true) => "logical_inverse",
            _ => "size_key",
        };
        let mut values = vec![];
        let mut predicate = String::from("1=1");
        let mut add = |clause: &str, value: Value| {
            values.push(value);
            predicate.push_str(&format!(" AND {clause}?{}", values.len()));
        };
        let mut node = None;
        let mut breadcrumbs = vec![];
        let mut coverage = AnalysisCoverage::default();
        let mut category_index = None;
        if query.view == "directories" {
            add("parent=", Value::Text(query.node.clone()));
            let current = Self::entry(&inner, &query.node)?;
            ensure!(current.directory, "请选择目录");
            let mut p = Some(query.node.clone());
            while let Some(id) = p {
                let e = Self::entry(&inner, &id)?;
                p = e.parent.clone();
                breadcrumbs.push(e);
                ensure!(breadcrumbs.len() <= ANALYSIS_DEPTH + 1, "目录链无效");
            }
            breadcrumbs.reverse();
            node = Some(current);
            coverage=inner.db.query_row("SELECT coalesce(sum(finished=1 AND incomplete=0),0),coalesce(sum(incomplete=1 OR (started=1 AND finished=0)),0),coalesce(sum(started=0 AND incomplete=0),0) FROM nodes WHERE parent=?1 AND directory=1",[&query.node],|r|Ok(AnalysisCoverage {completed:r.get::<_,i64>(0)? as u64,incomplete:r.get::<_,i64>(1)? as u64,pending:r.get::<_,i64>(2)? as u64}))?;
        } else if query.view == "files" {
            predicate.push_str(" AND rankable=1");
        } else {
            let d = query
                .discovery
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing filters"))?;
            ensure!(
                snapshot.state != "scanning",
                ApiError::new("analysis_busy", "结束或停止分析后可按用途查找")
            );
            ensure!(d.search.chars().count() <= 256, "查询过长");
            predicate.push_str(if d.kind == DiscoveryKind::File {
                " AND rankable=1"
            } else {
                " AND directory=1 AND (parent IS NOT NULL OR recognition>0)"
            });
            if d.kind == DiscoveryKind::Directory && !d.include_nested {
                predicate.push_str(" AND nested=0");
            }
            if d.incomplete_only {
                predicate.push_str(" AND (incomplete=1 OR finished=0)");
            }
            match d.category.as_str() {
                "all" => (),
                "recognized" => predicate.push_str(" AND category>0"),
                "unclassified" => predicate.push_str(" AND category=0"),
                category => {
                    let code = crate::recognition::Category::ALL
                        .iter()
                        .find(|c| c.key() == category)
                        .ok_or_else(|| anyhow::anyhow!("invalid category"))?
                        .code();
                    values.push(Value::Integer(code.into()));
                    predicate.push_str(&format!(" AND category=?{}", values.len()));
                }
            }
            // Directory discovery must never walk the global file ranking to
            // satisfy LIMIT. Its covering partial index filters and sorts only
            // directories; materialize the selected page afterwards.
            if d.kind == DiscoveryKind::Directory {
                category_index = Some("directory_ranking");
            } else if !matches!(d.category.as_str(), "all" | "unclassified") {
                // A selective category can be empty. Avoid walking unrelated files
                // in size order, but keep that order for broad file categories.
                category_index = Some("discovery_files");
            }
            let minimum = d.min_bytes.parse::<u64>()?;
            if minimum > 0 {
                values.push(Value::Text(format!("{minimum:020}")));
                predicate.push_str(&format!(" AND {metric}>=?{}", values.len()));
                if query.metric == "allocated" {
                    predicate.push_str(" AND allocation_key!='99999999999999999999'");
                }
            }
        }
        let search = if query.view == "discovery" {
            &query.discovery.as_ref().unwrap().search
        } else {
            &query.search
        };
        if !search.is_empty() {
            values.push(Value::Text(search.clone()));
            predicate.push_str(&format!(
                " AND instr(lower(name),lower(?{}))>0",
                values.len()
            ));
        }
        let mut key_query = query.clone();
        key_query.request_id.clear();
        key_query.cursor = None;
        key_query.offset = 0;
        if let Some(d) = &mut key_query.discovery {
            d.offset = 0;
        }
        let key = serde_json::to_string(&key_query)?;
        let count_predicate = predicate.clone();
        let count_values = values.clone();
        if let Some(cursor) = &query.cursor {
            ensure!(cursor.len() <= 8192, "游标过长");
            let cursor: Cursor = serde_json::from_str(cursor)?;
            ensure!(
                cursor.key == key && cursor.revision == revision,
                ApiError::new("stale_cursor", "扫描结果已更新，请刷新列表")
            );
            let n = values.len();
            values.extend([
                Value::Integer(i64::from(cursor.pending)),
                Value::Text(cursor.size),
                Value::Text(cursor.name),
                Value::Text(cursor.id),
            ]);
            predicate.push_str(&format!(
                " AND (pending,{size},name,id)>(?{},?{},?{},?{})",
                n + 1,
                n + 2,
                n + 3,
                n + 4
            ));
        } else {
            ensure!(
                query.offset == 0,
                ApiError::new("stale_cursor", "请从第一页开始读取")
            );
        }
        let source = if let Some(index) = category_index
            && inner.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='index' AND name=?1)",
                [index],
                |r| r.get::<_, bool>(0),
            )? {
            format!("nodes INDEXED BY {index}")
        } else {
            // A cancelled index build may not have created the ranking index yet.
            "nodes".into()
        };
        let sql = format!(
            "SELECT id,pending,{size},name FROM {source} WHERE {predicate} ORDER BY pending,{size},name,id LIMIT 201"
        );
        let mut statement = inner.db.prepare(&sql)?;
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("filters_prepare");
        let mut rows = statement
            .query_map(params_from_iter(&values), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("select_ids");
        let more = rows.len() > 200;
        rows.truncate(200);
        let next_cursor = if more {
            let (id, pending, size, name) = rows.last().unwrap();
            Some(serde_json::to_string(&Cursor {
                key: key.clone(),
                revision,
                pending: *pending,
                size: size.clone(),
                name: name.clone(),
                id: id.clone(),
            })?)
        } else {
            None
        };
        let items = rows
            .iter()
            .map(|(id, _, _, _)| self.described_entry(&inner, id))
            .collect::<Result<Vec<_>>>()?;
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("describe");
        ensure!(
            !cancel.load(Ordering::Acquire),
            ApiError::new("cancelled", "查询已停止")
        );
        let cached_total = self
            .count_cache
            .lock()
            .unwrap()
            .as_ref()
            .filter(|c| c.key == key && c.revision == revision)
            .map(|c| c.total);
        let total = if query.view == "files" && search.is_empty() {
            Some(snapshot.files)
        } else if !more && query.offset == 0 {
            Some(items.len() as u64)
        } else {
            cached_total
        };
        // End this snapshot before starting the asynchronous count connection.
        // Its revision is checked again by the count worker.
        drop(_cancellation);
        drop(inner);
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("reader_close");
        *self.count_result.lock().unwrap() = Some(AnalysisCount {
            request_id: query.request_id.clone(),
            total,
            finished: total.is_some(),
            error: None,
        });
        if total.is_none() {
            let engine = self.clone();
            let request_id = query.request_id.clone();
            std::thread::spawn(move || {
                let _gate = engine.count_gate.lock().unwrap();
                if cancel.load(Ordering::Acquire) {
                    return;
                }
                let result = (|| -> Result<u64> {
                    let (inner, read_revision) = engine.read_index(&descriptor)?;
                    ensure!(read_revision == revision, "扫描结果已变化，请刷新统计");
                    let _cancel = SqlCancellation::new(&inner.db, cancel.clone(), true)?;
                    let count = inner.db.query_row(
                        &format!("SELECT count(*) FROM {source} WHERE {count_predicate}"),
                        params_from_iter(&count_values),
                        |r| r.get::<_, i64>(0),
                    )?;
                    Ok(count as u64)
                })();
                if cancel.load(Ordering::Acquire) {
                    return;
                }
                let mut count = engine.count_result.lock().unwrap();
                if count.as_ref().is_some_and(|r| r.request_id == request_id) {
                    if let Ok(total) = &result {
                        *engine.count_cache.lock().unwrap() = Some(CachedCount {
                            key,
                            revision,
                            total: *total,
                        });
                    }
                    *count = Some(AnalysisCount {
                        request_id,
                        total: result.as_ref().ok().copied(),
                        finished: true,
                        error: result.err().map(|_| "统计未完成，请刷新结果".into()),
                    });
                }
            });
        }
        #[cfg(feature = "analysis-query-timing")]
        timing.mark("count_dispatch");
        Ok(AnalysisReadPage {
            request_id: query.request_id,
            generated_at: now(),
            items,
            total,
            offset: query.offset,
            next_cursor,
            node,
            breadcrumbs,
            coverage,
        })
    }
}
