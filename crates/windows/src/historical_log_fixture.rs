//! Raw synthetic observations backed by files inside one self-created fixture.
use super::*;
use dustaway_core::{ReasonCode, Refusal};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, Sender},
    },
};

pub(crate) const APP_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\App Paths\ToDesk.exe";
pub(crate) const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
pub(crate) type Observation<T> = std::result::Result<T, ReasonCode>;
type Values = HashMap<(Hive, String, String), Observation<Option<String>>>;
type Children = HashMap<(Hive, String), Observation<Vec<String>>>;
type Pause = (&'static str, Sender<()>, Receiver<()>);
#[derive(Clone)]
pub(crate) struct MetadataFacts {
    pub signature_valid: bool,
    pub publisher: String,
    pub product: Option<String>,
    pub version: Option<String>,
}
pub(crate) struct FixtureProbe {
    root: PathBuf,
    pub exe: PathBuf,
    pub candidates: Mutex<Vec<PathBuf>>,
    pub values: Mutex<Values>,
    pub children: Mutex<Children>,
    pub facts: Mutex<HashMap<PathBuf, MetadataFacts>>,
    pub process_state: Mutex<Processes>,
    pub process_calls: AtomicUsize,
    pub signature_calls: AtomicUsize,
    pub product_calls: AtomicUsize,
    pub version_calls: AtomicUsize,
    pub directory_errors: Mutex<HashMap<PathBuf, ReasonCode>>,
    pub entry_errors: Mutex<HashMap<PathBuf, usize>>,
    pub metadata_errors: Mutex<HashMap<PathBuf, ReasonCode>>,
    pub cancel_at: Mutex<Option<(&'static str, Arc<AtomicBool>)>>,
    pub cancel_entry: Mutex<Option<(usize, Arc<AtomicBool>)>>,
    pub pause_at: Mutex<Option<Pause>>,
}
impl FixtureProbe {
    pub fn new(root: &Path, dingtalk: bool) -> Self {
        let root = extended(root).unwrap();
        let exe = root.join("installation").join(if dingtalk {
            "DingTalk.exe"
        } else {
            "ToDesk.exe"
        });
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(
            &exe,
            b"synthetic executable metadata fixture; not a runnable application",
        )
        .unwrap();
        let facts = MetadataFacts {
            signature_valid: true,
            publisher: if dingtalk {
                "DingTalk Technology Co., Ltd."
            } else {
                "Hainan YouQu Technology Co., Ltd"
            }
            .into(),
            product: Some(if dingtalk { "DingTalk" } else { "ToDesk" }.into()),
            version: Some("0.0.0.1".into()),
        };
        Self {
            root,
            exe: exe.clone(),
            candidates: Mutex::new(vec![exe.clone()]),
            values: Default::default(),
            children: Default::default(),
            facts: Mutex::new(HashMap::from([(exe, facts)])),
            process_state: Mutex::new(Processes {
                current_session: Some(1),
                entries: vec![],
                complete: true,
            }),
            process_calls: AtomicUsize::new(0),
            signature_calls: AtomicUsize::new(0),
            product_calls: AtomicUsize::new(0),
            version_calls: AtomicUsize::new(0),
            directory_errors: Default::default(),
            entry_errors: Default::default(),
            metadata_errors: Default::default(),
            cancel_at: Default::default(),
            cancel_entry: Default::default(),
            pause_at: Default::default(),
        }
    }
    fn inside(&self, path: &Path) -> Result<()> {
        let path = extended(path)?;
        ensure!(
            path.starts_with(&self.root),
            "synthetic probe cannot inspect external data"
        );
        Ok(())
    }
    fn cancel(&self, point: &str) {
        let pause = {
            let mut pending = self.pause_at.lock().unwrap();
            if pending.as_ref().is_some_and(|(at, _, _)| *at == point) {
                pending.take()
            } else {
                None
            }
        };
        if let Some((_, sent, resume)) = pause {
            sent.send(()).unwrap();
            resume
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap();
        }
        if let Some((at, flag)) = &*self.cancel_at.lock().unwrap()
            && *at == point
        {
            flag.store(true, Ordering::Release);
        }
    }
    pub fn value(&self, hive: Hive, key: &str, name: &str, value: Observation<Option<String>>) {
        self.values
            .lock()
            .unwrap()
            .insert((hive, key.into(), name.into()), value);
    }
    pub fn facts_for(&self, pin: &PinnedPath) -> Result<MetadataFacts> {
        self.inside(&pin.path)?;
        self.facts
            .lock()
            .unwrap()
            .get(&pin.path)
            .cloned()
            .ok_or_else(|| Refusal(ReasonCode::ApplicationUnverified).into())
    }
    pub fn set_running(&self, name: &str, session: Option<u32>, image: Option<PathBuf>) {
        self.process_state.lock().unwrap().entries = vec![ProcessObservation {
            name: name.into(),
            session,
            image,
        }];
    }
    pub fn replace_program(&self) {
        let saved = self.root.join("previous-program");
        self.inside(&self.exe).unwrap();
        self.inside(&saved).unwrap();
        std::fs::rename(&self.exe, &saved).unwrap();
        std::fs::copy(&saved, &self.exe).unwrap();
    }
}
impl Probe for FixtureProbe {
    fn registry_string(&self, hive: Hive, key: &str, name: &str) -> Result<Option<String>> {
        self.cancel("registry");
        self.values
            .lock()
            .unwrap()
            .get(&(hive, key.into(), name.into()))
            .cloned()
            .unwrap_or(Ok(None))
            .map_err(|r| Refusal(r).into())
    }
    fn registry_children(
        &self,
        hive: Hive,
        key: &str,
        _: Option<&AtomicBool>,
    ) -> Result<Vec<String>> {
        self.children
            .lock()
            .unwrap()
            .get(&(hive, key.into()))
            .cloned()
            .unwrap_or(Ok(vec![]))
            .map_err(|r| Refusal(r).into())
    }
    fn standard_todesk_paths(&self, _cancel: Option<&AtomicBool>) -> Result<Vec<PathBuf>> {
        let paths = self.candidates.lock().unwrap().clone();
        for path in &paths {
            self.inside(path)?;
        }
        Ok(paths)
    }
    fn dingtalk_candidates(&self, _: Option<&AtomicBool>) -> Result<Vec<PathBuf>> {
        self.cancel("registry");
        self.standard_todesk_paths(None)
    }
    fn signed_by(
        &self,
        pin: &PinnedPath,
        publisher: &str,
        _cancel: Option<&AtomicBool>,
    ) -> Result<bool> {
        self.signature_calls.fetch_add(1, Ordering::Relaxed);
        self.cancel("signature");
        let facts = self.facts_for(pin)?;
        Ok(facts.signature_valid && facts.publisher == publisher)
    }
    fn product(&self, pin: &PinnedPath, _cancel: Option<&AtomicBool>) -> Result<String> {
        self.product_calls.fetch_add(1, Ordering::Relaxed);
        self.cancel("product");
        self.facts_for(pin)?
            .product
            .ok_or_else(|| Refusal(ReasonCode::ApplicationUnverified).into())
    }
    fn version(&self, pin: &PinnedPath, _cancel: Option<&AtomicBool>) -> Result<String> {
        self.version_calls.fetch_add(1, Ordering::Relaxed);
        self.cancel("version");
        self.facts_for(pin)?
            .version
            .ok_or_else(|| Refusal(ReasonCode::ApplicationUnverified).into())
    }
    fn processes(&self, _: Option<&AtomicBool>) -> Result<Processes> {
        self.process_calls.fetch_add(1, Ordering::Relaxed);
        self.cancel("process");
        Ok(self.process_state.lock().unwrap().clone())
    }
    fn read_dir(&self, root: &Path) -> Result<Entries> {
        self.inside(root)?;
        self.cancel("read_dir");
        if let Some(reason) = self.directory_errors.lock().unwrap().get(root) {
            return Err(Refusal(*reason).into());
        }
        let mut paths: Vec<_> = std::fs::read_dir(root)?
            .map(|e| Ok(e?.path()))
            .collect::<Result<_>>()?;
        paths.sort();
        let mut entries: Vec<Result<PathBuf>> = paths.into_iter().map(Ok).collect();
        if let Some(index) = self.entry_errors.lock().unwrap().get(root) {
            entries.insert(
                (*index).min(entries.len()),
                Err(Refusal(ReasonCode::DirectoryInaccessible).into()),
            );
        }
        let cancel = self.cancel_entry.lock().unwrap().clone();
        Ok(Box::new(entries.into_iter().enumerate().map(
            move |(index, entry)| {
                if let Some((at, flag)) = &cancel
                    && index == *at
                {
                    flag.store(true, Ordering::Release);
                }
                entry
            },
        )))
    }
    fn metadata(&self, path: &Path) -> Result<Metadata> {
        self.inside(path)?;
        self.cancel("metadata");
        if let Some(reason) = self.metadata_errors.lock().unwrap().get(path) {
            return Err(Refusal(*reason).into());
        }
        Ok(std::fs::symlink_metadata(path)?)
    }
}
