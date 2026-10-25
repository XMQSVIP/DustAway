//! Captures the same FileRecords delivered to Engine, without another discovery pass.
use anyhow::{Context, Result, ensure};
use dustaway_core::*;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

pub struct CapturedPlatform {
    platform: Arc<dyn Platform>,
    files: Mutex<Vec<FileRecord>>,
}
impl CapturedPlatform {
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self {
            platform,
            files: Mutex::new(Vec::new()),
        }
    }
    pub fn selected(&self, candidates: &[Candidate]) -> Result<Vec<FileRecord>> {
        let records = self.files.lock().unwrap();
        candidates
            .iter()
            .map(|candidate| {
                let matches: Vec<_> = records
                    .iter()
                    .filter(|f| f.path_display == candidate.path)
                    .collect();
                ensure!(matches.len() == 1, "Ambiguous original candidate");
                let record = matches.first().context("Missing original candidate")?;
                ensure!(
                    record.bytes.to_string() == candidate.bytes
                        && record.modified_unix == candidate.modified_unix,
                    "Candidate metadata differs"
                );
                Ok((**record).clone())
            })
            .collect()
    }
}
impl Platform for CapturedPlatform {
    fn cleanup_validated(&self, r: &RuleDescriptor) -> bool {
        self.platform.cleanup_validated(r)
    }
    fn scan(
        &self,
        r: &RuleDescriptor,
        e: &[String],
        c: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        self.files.lock().unwrap().clear();
        self.platform.scan(r, e, c, &mut |event| {
            if let ScanEvent::File(file) = &event {
                self.files.lock().unwrap().push(file.clone());
            }
            emit(event);
        })
    }
    fn validate_candidate(&self, r: &RuleDescriptor, f: &FileRecord) -> Result<()> {
        self.platform.validate_candidate(r, f)
    }
    fn blocked(&self, r: &RuleDescriptor) -> Result<Option<BlockReason>> {
        self.platform.blocked(r)
    }
    fn delete(&self, r: &RuleDescriptor, f: &FileRecord, e: &[String]) -> Result<()> {
        self.platform.delete(r, f, e)
    }
    fn free_space(&self) -> Result<u64> {
        self.platform.free_space()
    }
}
