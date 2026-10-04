//! Disposable per-session indices. No scan can be resumed or used as a deletion plan.
use anyhow::{Result, ensure};
use rusqlite::Connection;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const DISK_BUDGET: u64 = 4 * 1024 * 1024 * 1024;
pub const DISK_RESERVE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct AnalysisStorage {
    pub directory: PathBuf,
    pub budget: u64,
    pub reserve: u64,
}
impl AnalysisStorage {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            budget: DISK_BUDGET,
            reserve: DISK_RESERVE,
        }
    }
    pub fn initialize(&self) -> Result<()> {
        ensure!(self.directory.is_absolute(), "索引目录必须是绝对路径");
        for parent in self.directory.ancestors().filter(|p| p.exists()) {
            ensure!(
                !linked(&fs::symlink_metadata(parent)?),
                "索引目录不能经过链接"
            );
        }
        fs::create_dir_all(&self.directory)?;
        for item in fs::read_dir(&self.directory)? {
            let item = item?;
            let path = item.path();
            if !item.file_type()?.is_dir()
                || linked(&fs::symlink_metadata(&path)?)
                || uuid::Uuid::parse_str(&item.file_name().to_string_lossy()).is_err()
            {
                continue;
            }
            let lock_path = path.join("session.lock");
            if !lock_path.is_file() || linked(&fs::symlink_metadata(&lock_path)?) {
                continue;
            }
            let mut file = OpenOptions::new().read(true).write(true).open(&lock_path)?;
            if file.try_lock().is_err() {
                continue;
            }
            let mut marker = String::new();
            (&mut file).take(128).read_to_string(&mut marker)?;
            if marker
                != format!(
                    "DustAway analysis v1\n{}",
                    item.file_name().to_string_lossy()
                )
            {
                continue;
            }
            clean_files(&path);
            drop(file);
            let _ = fs::remove_file(lock_path);
            let _ = fs::remove_dir(path);
        }
        Ok(())
    }
}
fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
pub(super) struct SessionStorage {
    pub path: PathBuf,
    lock: Option<File>,
}
impl SessionStorage {
    pub fn create(config: &AnalysisStorage, id: &str) -> Result<(Self, Connection)> {
        uuid::Uuid::parse_str(id)?;
        let path = config.directory.join(id);
        fs::create_dir(&path)?;
        let mut lock = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(path.join("session.lock"))?;
        lock.try_lock()?;
        write!(lock, "DustAway analysis v1\n{id}")?;
        let session = Self {
            path,
            lock: Some(lock),
        };
        let db = Connection::open(session.path.join("index.sqlite"))?;
        // The index is discarded after a crash and never resumes a deletion plan.
        // Avoid durability barriers for this disposable cache; cleanup journals retain
        // their existing durability settings. Larger checkpoints avoid write amplification.
        db.execute_batch("PRAGMA page_size=4096; PRAGMA journal_mode=WAL; PRAGMA synchronous=OFF; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-65536; PRAGMA wal_autocheckpoint=4096; PRAGMA journal_size_limit=16777216;")?;
        Ok((session, db))
    }
    pub fn bytes(&self) -> u64 {
        ["index.sqlite", "index.sqlite-wal", "index.sqlite-shm"]
            .iter()
            .filter_map(|name| fs::metadata(self.path.join(name)).ok())
            .map(|m| m.len())
            .sum()
    }
}
fn clean_files(path: &Path) {
    for name in ["index.sqlite-wal", "index.sqlite-shm", "index.sqlite"] {
        let file = path.join(name);
        if fs::symlink_metadata(&file).is_ok_and(|m| m.is_file() && !linked(&m)) {
            let _ = fs::remove_file(file);
        }
    }
}
impl Drop for SessionStorage {
    fn drop(&mut self) {
        clean_files(&self.path);
        self.lock.take();
        let _ = fs::remove_file(self.path.join("session.lock"));
        let _ = fs::remove_dir(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_skips_live_sessions_and_preserves_unowned_content() {
        let root = tempfile::tempdir().unwrap();
        let config = AnalysisStorage::new(root.path().join("cache"));
        config.initialize().unwrap();
        fs::write(config.directory.join("keep.txt"), "keep").unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let (session, db) = SessionStorage::create(&config, &id).unwrap();
        config.initialize().unwrap();
        assert!(session.path.join("index.sqlite").exists());
        drop(db);
        drop(session);
        assert!(!config.directory.join(id).exists());
        assert_eq!(
            fs::read_to_string(config.directory.join("keep.txt")).unwrap(),
            "keep"
        );
        let abandoned = config.directory.join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&abandoned).unwrap();
        fs::write(
            abandoned.join("session.lock"),
            format!(
                "DustAway analysis v1\n{}",
                abandoned.file_name().unwrap().to_string_lossy()
            ),
        )
        .unwrap();
        fs::write(
            abandoned.join("index.sqlite"),
            b"incomplete cache after crash",
        )
        .unwrap();
        fs::write(abandoned.join("index.sqlite-wal"), b"incomplete wal").unwrap();
        fs::write(abandoned.join("unowned.txt"), b"retain").unwrap();
        config.initialize().unwrap();
        assert!(!abandoned.join("index.sqlite").exists());
        assert!(!abandoned.join("index.sqlite-wal").exists());
        assert_eq!(fs::read(abandoned.join("unowned.txt")).unwrap(), b"retain");
    }
}
