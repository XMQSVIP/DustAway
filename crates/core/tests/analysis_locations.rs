use anyhow::Result;
use dustaway_core::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

struct Volume {
    supported: bool,
    identity: AtomicUsize,
}
fn record(id: &str, parent: Option<&str>) -> AnalysisRecord {
    AnalysisRecord {
        entry: serde_json::from_value(serde_json::json!({
            "id":id,"parent":parent,"name":id,"path":format!("C:\\{id}"),"directory":true,
            "bytes":"0","files":0,"modified_unix":0,"shared":false,"incomplete":false,
            "issue":null,"rule_ids":[],"excluded":false
        }))
        .unwrap(),
        file_key: None,
        opaque: id.into(),
    }
}
impl AnalysisPlatform for Volume {
    fn locations(&self) -> Vec<(String, String)> {
        vec![
            ("系统磁盘 C:".into(), "C:\\".into()),
            ("磁盘 D:".into(), "D:\\".into()),
        ]
    }
    fn location_info(&self, path: &str) -> AnalysisLocationInfo {
        AnalysisLocationInfo {
            kind: if path.len() == 3 {
                "volume"
            } else {
                "directory"
            }
            .into(),
            is_system: path.starts_with('C'),
            supported: self.supported,
            unavailable_reason: (!self.supported).then(|| "不是 NTFS".into()),
            ..Default::default()
        }
    }
    fn volume_key(&self, path: &str) -> Result<Option<String>> {
        let n = self.identity.load(Ordering::Acquire);
        anyhow::ensure!(n != 2, "volume disconnected");
        Ok(Some(if path.starts_with('D') {
            format!("data-volume-{n}")
        } else {
            format!("volume-{n}")
        }))
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        Ok(AnalysisRoot {
            path: path.into(),
            opaque: "root".into(),
        })
    }
    fn context(&self, _: &str) -> Result<(String, u64)> {
        Ok(("C:\\".into(), 0))
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        Ok(())
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        emit(AnalysisEvent::Entry(Box::new(record("0", None))))?;
        emit(AnalysisEvent::DirectoryStarted {
            node: "0".into(),
            path: "C:\\".into(),
        })?;
        emit(AnalysisEvent::Entry(Box::new(record("pending", Some("0")))))?;
        while !cancel.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
}
#[test]
fn cleanup_scope_accepts_one_or_many_volume_tokens_but_never_paths_or_folder_tokens() {
    let source = Arc::new(Volume {
        supported: true,
        identity: AtomicUsize::new(0),
    });
    let e = AnalysisEngine::new(source.clone()).unwrap();
    let mut locations = e.locations();
    locations.sort_by(|a, b| a.path.cmp(&b.path));
    let tokens: Vec<_> = locations.iter().map(|l| l.token.clone()).collect();
    let single = e.scan_volumes(&tokens[..1]).unwrap();
    assert_eq!(single[0].label, "C:\\");
    assert_eq!(e.scan_volumes(&tokens).unwrap().len(), 2);
    assert!(e.scan_volumes(&[]).is_err());
    assert!(e.scan_volumes(&["C:\\".into()]).is_err());
    assert!(
        e.scan_volumes(&[tokens[0].clone(), tokens[0].clone()])
            .is_err()
    );
    let folder = e.register("C:\\folder").unwrap();
    assert!(e.scan_volumes(&[folder.token]).is_err());
    source.identity.store(1, Ordering::Release);
    assert!(e.scan_volumes(&tokens).is_err());
    source.identity.store(2, Ordering::Release);
    assert!(e.scan_volumes(&tokens).is_err());
    assert!(e.snapshot().is_none());
}
#[test]
fn volume_tokens_reject_remapped_and_missing_volumes_and_do_not_invent_zero_space() {
    let source = Arc::new(Volume {
        supported: true,
        identity: AtomicUsize::new(0),
    });
    let e = AnalysisEngine::new(source.clone()).unwrap();
    let location = e
        .locations()
        .into_iter()
        .find(|l| l.path == "C:\\")
        .unwrap();
    assert_eq!(location.info.kind, "volume");
    assert!(location.info.is_system);
    let serialized = serde_json::to_value(&location).unwrap();
    assert!(serialized.get("expected_volume").is_none());
    let c = e.context(&location.token).unwrap();
    assert_eq!(c.available_bytes.as_deref(), Some("0"));
    assert_eq!(c.volume_key.as_deref(), Some("volume-0"));
    source.identity.store(1, Ordering::Release);
    assert!(e.start(&location.token, vec![]).is_err());
    assert!(
        e.scan_volumes(std::slice::from_ref(&location.token))
            .is_err()
    );
    assert!(e.snapshot().is_none());
    let c = e.context(&location.token).unwrap();
    assert!(c.available_bytes.is_none() && c.volume_key.is_none() && c.error.is_some());
    source.identity.store(2, Ordering::Release);
    assert!(e.start(&location.token, vec![]).is_err());
    assert!(e.context("C:\\").is_err());
}
#[test]
fn unsupported_volume_has_a_reason_and_cannot_start() {
    let e = AnalysisEngine::new(Arc::new(Volume {
        supported: false,
        identity: AtomicUsize::new(0),
    }))
    .unwrap();
    let location = &e.locations()[0];
    assert!(!location.info.supported);
    assert_eq!(
        location.info.unavailable_reason.as_deref(),
        Some("不是 NTFS")
    );
    assert!(e.start(&location.token, vec![]).is_err());
    assert!(
        e.context(&location.token)
            .unwrap()
            .available_bytes
            .is_none()
    );
}
#[test]
fn unread_root_children_stay_pending_after_cancellation_and_identifiers_are_scoped() {
    let e = AnalysisEngine::new(Arc::new(Volume {
        supported: true,
        identity: AtomicUsize::new(0),
    }))
    .unwrap();
    let id = e.start(&e.locations()[0].token, vec![]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while e.snapshot().unwrap().entries < 2 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let page = e.children(&id, "0", "", 0, true).unwrap();
    assert!(page.items[0].pending && page.items[0].incomplete);
    assert!(!page.node.pending && page.node.incomplete);
    assert_eq!(e.snapshot().unwrap().phase, "enumerating");
    assert!(e.refine("wrong", "pending").is_err());
    assert!(e.refine(&id, "invented").is_err());
    e.cancel(&id).unwrap();
    while e.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(e.snapshot().unwrap().state, "cancelled");
    assert!(e.children(&id, "0", "", 0, true).unwrap().items[0].pending);
    // A rejected start must not replace a readable previous session.
    assert!(e.start("expired-location", vec![]).is_err());
    assert_eq!(e.snapshot().unwrap().id, id);
    assert!(e.children(&id, "0", "", 0, true).unwrap().items[0].pending);
}
