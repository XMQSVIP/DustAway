//! Batch-local directory caches; the database is the authoritative state between batches.
use super::*;
use rusqlite::OptionalExtension;
pub(super) fn storage_key(key: &str) -> rusqlite::types::Value {
    if key.len() == 49
        && key.as_bytes()[16] == b':'
        && let (Ok(volume), Ok(id)) = (
            u64::from_str_radix(&key[..16], 16),
            u128::from_str_radix(&key[17..], 16),
        )
    {
        let mut bytes = volume.to_le_bytes().to_vec();
        bytes.extend_from_slice(&id.to_le_bytes());
        return bytes.into();
    }
    key.to_owned().into()
}

pub(super) fn parent_of(inner: &Inner, id: &str) -> Result<Option<String>> {
    if let Some(parent) = inner.parents.get(id) {
        return Ok(parent.clone());
    }
    Ok(inner
        .db
        .prepare_cached("SELECT parent FROM nodes WHERE id=?1")?
        .query_row([id], |r| r.get(0))?)
}
pub(super) fn aggregate(inner: &Inner, id: &str) -> Result<Option<Aggregate>> {
    if let Some(total) = inner.totals.get(id) {
        return Ok(Some(total.clone()));
    }
    let row = inner.db.prepare_cached("SELECT size_key,files,shared,incomplete,started,finished,recognition,allocation_key,allocation_unknown FROM nodes WHERE id=?1 AND directory=1")?.query_row([id],|r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)? as u64,r.get::<_,bool>(2)?,r.get::<_,bool>(3)?,r.get::<_,bool>(4)?,r.get::<_,bool>(5)?,r.get::<_,u32>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,i64>(8)? as u64))
    }).optional()?;
    row.map(
        |(
            bytes,
            files,
            shared,
            incomplete,
            started,
            finished,
            recognition,
            allocated,
            allocation_unknown,
        )| {
            Ok(Aggregate {
                bytes: bytes.parse()?,
                files,
                shared,
                incomplete,
                started,
                finished,
                recognition,
                allocated: allocated.map(|v| v.parse()).transpose()?.unwrap_or(0),
                allocation_unknown,
                dirty: Default::default(),
            })
        },
    )
    .transpose()
}
pub(super) fn cached_total<'a>(inner: &'a mut Inner, id: &str) -> Result<&'a mut Aggregate> {
    if !inner.totals.contains_key(id) {
        let total = aggregate(inner, id)?.ok_or_else(|| anyhow::anyhow!("directory missing"))?;
        inner.totals.insert(id.into(), total);
    }
    Ok(inner.totals.get_mut(id).unwrap())
}
pub(super) fn flush(inner: &mut Inner) -> Result<()> {
    let mut update=inner.db.prepare_cached("UPDATE nodes SET size_key=?2,files=?3,shared=?4,incomplete=?5,pending=?6,started=?7,finished=?8,allocation_key=?9,allocation_unknown=?10,logical_inverse=?11,allocation_inverse=?12 WHERE id=?1")?;
    for (id, t) in &inner.totals {
        update.execute(params![
            id,
            format!("{:020}", t.bytes),
            t.files as i64,
            t.shared,
            t.incomplete,
            !t.started && !t.incomplete,
            t.started,
            t.finished,
            format!("{:020}", t.allocated),
            t.allocation_unknown as i64,
            format!("{:020}", u64::MAX - t.bytes),
            format!("{:020}", u64::MAX - t.allocated)
        ])?;
    }
    inner.totals.clear();
    inner.parents.clear();
    Ok(())
}
pub(super) fn allocate(inner: &mut Inner, record: &AnalysisRecord, unique: bool) -> Result<()> {
    let e = &record.entry;
    let amount = e
        .allocated_bytes
        .as_ref()
        .map(|n| n.parse::<u64>())
        .transpose()?;
    let unknown = u64::from(amount.is_none());
    let Some(key) = &record.file_key else {
        return Ok(());
    };
    let stored_key = storage_key(key);
    let key = &stored_key;
    if !unique {
        // Only hardlinked identities need a directory membership set. Ordinary files do not.
        let initialized = inner
            .db
            .query_row(
                "SELECT 1 FROM allocation_seen WHERE identity=?1 AND directory=''",
                [key],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !initialized {
            inner
                .db
                .execute("INSERT INTO allocation_seen VALUES(?1,'')", [key])?;
            let first: String = inner.db.query_row(
                "SELECT first_node FROM identities WHERE id=?1",
                [key],
                |r| r.get(0),
            )?;
            let mut parent = parent_of(inner, &first)?;
            while let Some(id) = parent {
                inner.db.execute(
                    "INSERT OR IGNORE INTO allocation_seen VALUES(?1,?2)",
                    params![key, id],
                )?;
                cached_total(inner, &id)?.shared = true;
                parent = parent_of(inner, &id)?;
            }
        }
    }
    let mut parent = e.parent.clone();
    while let Some(id) = parent {
        let include = unique
            || inner.db.execute(
                "INSERT OR IGNORE INTO allocation_seen VALUES(?1,?2)",
                params![key, id],
            )? == 1;
        let t = cached_total(inner, &id)?;
        t.shared |= !unique;
        if include {
            t.allocated = t
                .allocated
                .checked_add(amount.unwrap_or(0))
                .ok_or_else(|| anyhow::anyhow!("allocation overflow"))?;
            t.allocation_unknown += unknown;
        }
        parent = parent_of(inner, &id)?;
    }
    if unique {
        let snapshot = inner.snapshot.as_mut().unwrap();
        snapshot.allocated_bytes = snapshot
            .allocated_bytes
            .parse::<u64>()?
            .checked_add(amount.unwrap_or(0))
            .ok_or_else(|| anyhow::anyhow!("allocation overflow"))?
            .to_string();
        snapshot.allocation_unknown += unknown;
    }
    Ok(())
}

impl AnalysisEngine {
    pub(super) fn work_batched(&self, root: &AnalysisRoot) -> Result<()> {
        let scanned_ms = std::sync::atomic::AtomicU64::new(0);
        std::thread::scope(|scope| -> Result<()> {
            // Eight 2 MiB batches bound queued producer data at 16 MiB.
            let (sender, receiver) = std::sync::mpsc::sync_channel::<Vec<AnalysisEvent>>(8);
            let scanned_clock = &scanned_ms;
            let producer = scope.spawn(move || {
                let scan_begin = Instant::now();
                let mut events = Vec::new();
                let mut bytes = 0;
                let mut entries = 0;
                let mut seen = 0;
                let mut began = Instant::now();
                let result = self.platform.scan(root, &self.cancelled, &mut |event| {
                    if self.cancelled.load(Ordering::Acquire) {
                        return Ok(false);
                    }
                    bytes += match &event {
                        AnalysisEvent::Entry(r) => {
                            entries += 1;
                            seen += 1;
                            512 + r.opaque.len() + r.entry.path.len() + r.entry.name.len()
                        }
                        _ => 128,
                    };
                    events.push(event);
                    if entries >= 512
                        || (entries > 0 && (seen <= 8 || self.index_bytes < 1024 * 1024))
                        || bytes >= 2 * 1024 * 1024
                        || began.elapsed().as_millis() >= 50
                    {
                        if sender.send(std::mem::take(&mut events)).is_err() {
                            return Ok(false);
                        }
                        bytes = 0;
                        entries = 0;
                        began = Instant::now();
                    }
                    Ok(true)
                });
                if !events.is_empty() {
                    let _ = sender.send(events);
                }
                scanned_clock.store(scan_begin.elapsed().as_millis() as u64, Ordering::Release);
                result
            });
            let mut outcome = Ok(());
            let mut write_time = std::time::Duration::ZERO;
            let mut space_check = Instant::now() - std::time::Duration::from_secs(2);
            for events in &receiver {
                if self.cancelled.load(Ordering::Acquire) {
                    break;
                }
                let write_begin = Instant::now();
                let mut inner = self.inner.lock().unwrap();
                let config = self.storage_config.lock().unwrap().clone();
                if let (Some(storage), Some(config)) = (&inner.storage, &config) {
                    let full = storage.bytes() + 8 * 1024 * 1024 >= config.budget;
                    let free = if space_check.elapsed().as_secs() >= 1 {
                        space_check = Instant::now();
                        self.platform
                            .context(&config.directory.to_string_lossy())
                            .map(|(_, free)| free)
                    } else {
                        Ok(u64::MAX)
                    };
                    if full
                        || free.is_err()
                        || free.is_ok_and(|free| free <= config.reserve + 8 * 1024 * 1024)
                    {
                        Self::issue(&mut inner, "0", "resource_limit")?;
                        flush(&mut inner)?;
                        break;
                    }
                }
                let before = inner.snapshot.clone();
                inner.db.execute_batch("BEGIN IMMEDIATE")?;
                let mut stop = false;
                let result = (|| -> Result<()> {
                    for event in events {
                        if self.cancelled.load(Ordering::Acquire) {
                            stop = true;
                            break;
                        }
                        match event {
                            AnalysisEvent::DirectoryStarted { node, path } => {
                                cached_total(&mut inner, &node)?.started = true;
                                let s = inner.snapshot.as_mut().unwrap();
                                s.current_directory = Some(path);
                                s.phase = if node == "0" {
                                    "enumerating"
                                } else {
                                    "walking"
                                }
                                .into();
                            }
                            AnalysisEvent::DirectoryFinished { node } => {
                                cached_total(&mut inner, &node)?.finished = true
                            }
                            AnalysisEvent::Checked => inner.snapshot.as_mut().unwrap().checked += 1,
                            AnalysisEvent::Issue { node, code } => {
                                Self::issue(&mut inner, &node, &code)?
                            }
                            AnalysisEvent::Entry(mut record) => {
                                if inner.snapshot.as_ref().unwrap().entries >= self.limit {
                                    Self::issue(&mut inner, "0", "entry_limit")?;
                                    stop = true;
                                    break;
                                }
                                let name = record.entry.path.replace('/', "\\").to_lowercase();
                                record.entry.excluded = inner.exclusions.iter().any(|p| {
                                    let p =
                                        p.replace('/', "\\").trim_end_matches('\\').to_lowercase();
                                    name == p
                                        || name
                                            .strip_prefix(&p)
                                            .is_some_and(|s| s.starts_with('\\'))
                                });
                                Self::insert(&mut inner, &record)?;
                                if let Some(code) = &record.entry.issue {
                                    Self::issue(&mut inner, &record.entry.id, code)?;
                                }
                            }
                        }
                    }
                    flush(&mut inner)?;
                    persist_snapshot(&inner)?;
                    inner.db.execute_batch("COMMIT")?;
                    Ok(())
                })();
                write_time += write_begin.elapsed();
                if let Err(error) = result {
                    let _ = inner.db.execute_batch("ROLLBACK");
                    inner.snapshot = before;
                    inner.totals.clear();
                    inner.parents.clear();
                    if error
                        .downcast_ref::<ApiError>()
                        .is_some_and(|e| e.code == "analysis_changed")
                    {
                        Self::issue(&mut inner, "0", "changed")?;
                        let _ = flush(&mut inner);
                    } else if resource_error(&error) {
                        Self::issue(&mut inner, "0", "resource_limit")?;
                        let _ = flush(&mut inner);
                    } else {
                        outcome = Err(error);
                    }
                    break;
                }
                // Publish only committed totals. Status readers must not wait
                // behind the next SQLite write or a blocking volume-space call.
                *self.progress_snapshot.lock().unwrap() = inner
                    .snapshot
                    .clone()
                    .map(|snapshot| (snapshot, inner.started.unwrap()));
                if stop {
                    break;
                }
            }
            drop(receiver);
            let scanned = producer
                .join()
                .map_err(|_| anyhow::anyhow!("scanner panicked"))?;
            if let Some(snapshot) = &mut self.inner.lock().unwrap().snapshot {
                snapshot.scan_ms = scanned_ms.load(Ordering::Acquire);
                snapshot.write_ms = write_time.as_millis() as u64;
            }
            outcome.and(scanned)
        })
    }
}
