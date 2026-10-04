//! A private, bounded manifest of explicit analysis selections. Never accepted over IPC.
use crate::*;
use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualSelection {
    pub session: String,
    pub selected_ids: Vec<String>,
    pub excluded_ids: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManualSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub session: String,
    pub files: u64,
    pub directories: u64,
    pub bytes: String,
    pub retained: u64,
}
/// A read-only estimate from the analysis index, never a deletion credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPreview {
    pub request_id: String,
    #[serde(flatten)]
    pub summary: ManualSummary,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectoryOutcome {
    pub total: u64,
    pub deleted: u64,
    pub skipped: u64,
    pub failed: u64,
    pub unknown: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManualCapability {
    pub allowed: bool,
    pub reason: Option<ReasonCode>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualRecord {
    pub file: FileRecord,
    pub directory: bool,
    pub identity: String,
}
pub struct ManualManifest {
    pub(crate) db: Connection,
    pub(crate) platform: Arc<dyn AnalysisPlatform>,
    pub summary: ManualSummary,
}
impl ManualManifest {
    pub(crate) fn new(platform: Arc<dyn AnalysisPlatform>, session: String) -> Result<Self> {
        let db = Connection::open_in_memory()?;
        db.execute_batch(
            "PRAGMA page_size=4096; PRAGMA max_page_count=131072; PRAGMA temp_store=MEMORY;",
        )?;
        db.execute_batch("CREATE TABLE items(id TEXT PRIMARY KEY, identity TEXT UNIQUE, directory INTEGER, depth INTEGER, record TEXT); CREATE INDEX execution_order ON items(directory,depth DESC,id)")?;
        Ok(Self {
            db,
            platform,
            summary: ManualSummary {
                session,
                bytes: "0".into(),
                ..Default::default()
            },
        })
    }
    pub(crate) fn insert(&mut self, node: &str, depth: usize, record: ManualRecord) -> Result<()> {
        let kind = if record.directory {
            "directory"
        } else {
            "file"
        };
        let count = self.db.execute(
            "INSERT OR IGNORE INTO items VALUES(?1,?2,?3,?4,?5)",
            params![
                format!("manual:{kind}:{node}"),
                record.identity,
                record.directory,
                depth as i64,
                serde_json::to_string(&record)?
            ],
        )?;
        if count > 0 {
            if record.directory {
                self.summary.directories += 1;
            } else {
                self.summary.files += 1;
                self.summary.bytes = self
                    .summary
                    .bytes
                    .parse::<u64>()?
                    .checked_add(record.file.bytes)
                    .ok_or_else(|| anyhow::anyhow!("文件大小超出范围"))?
                    .to_string();
            }
        }
        Ok(())
    }
}
