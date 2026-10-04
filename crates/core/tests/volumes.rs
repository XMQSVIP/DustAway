use anyhow::Result;
use dustaway_core::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
struct Volumes {
    calls: Mutex<std::collections::HashMap<String, u32>>,
    deleted: AtomicBool,
}
impl Platform for Volumes {
    fn scan(
        &self,
        _: &RuleDescriptor,
        _: &[String],
        _: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        for disk in ["C:", "D:", "E:"] {
            emit(ScanEvent::File(FileRecord {
                path_display: format!("{disk}\\cache\\file"),
                bytes: 10,
                modified_unix: 0,
                opaque: disk.into(),
            }));
        }
        Ok(())
    }
    fn candidate_root(&self, f: &FileRecord) -> Option<String> {
        Some(f.opaque.clone())
    }
    fn blocked(&self, _: &RuleDescriptor) -> Result<Option<dustaway_core::BlockReason>> {
        Ok(None)
    }
    fn delete(&self, _: &RuleDescriptor, _: &FileRecord, _: &[String]) -> Result<()> {
        self.deleted.store(true, Ordering::Release);
        Ok(())
    }
    fn free_space(&self) -> Result<u64> {
        panic!("legacy system-volume sampling must not be used")
    }
    fn file_volume(&self, f: &FileRecord) -> Result<VolumeTarget> {
        Ok(VolumeTarget {
            key: f.opaque.clone(),
            label: f.opaque.clone(),
            opaque: f.opaque.clone(),
        })
    }
    fn volume_available(&self, v: &VolumeTarget) -> Result<u64> {
        let mut calls = self.calls.lock().unwrap();
        let n = calls.entry(v.key.clone()).or_default();
        *n += 1;
        if v.key == "E:" && *n > 1 {
            anyhow::bail!("offline");
        }
        Ok(if *n == 1 {
            100
        } else if v.key == "C:" {
            110
        } else {
            80
        })
    }
}
#[test]
fn report_samples_plan_volumes_and_preserves_negative_or_missing_changes() {
    let root = tempfile::tempdir().unwrap();
    let fake = Arc::new(Volumes {
        calls: Mutex::default(),
        deleted: AtomicBool::new(false),
    });
    let engine = Engine::open(
        fake.clone(),
        Scope::User,
        &root.path().join("history.sqlite"),
    )
    .unwrap();
    let id = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    let wait = || {
        let end = Instant::now() + Duration::from_secs(10);
        while engine.is_busy() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    wait();
    let plan = engine
        .prepare(
            &id,
            Selection {
                rule_ids: vec!["windows.user_temp".into()],
                excluded_ids: vec![],
                ..Selection::default()
            },
        )
        .unwrap();
    engine.execute(&plan.id).unwrap();
    wait();
    let report = engine.report().unwrap();
    assert_eq!(report.deleted, 3);
    assert!(report.free_space_change.is_none());
    let changes = &report.volume_space_changes;
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].change_bytes.as_deref(), Some("10"));
    assert_eq!(changes[1].change_bytes.as_deref(), Some("-20"));
    assert_eq!(changes[2].after_bytes, None);
    assert_eq!(changes[2].error.as_deref(), Some("after_unavailable"));
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("cache"));
    assert_eq!(
        engine.history().unwrap()[0].volume_space_changes[1]
            .change_bytes
            .as_deref(),
        Some("-20")
    );
}
