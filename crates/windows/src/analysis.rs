//! Attribute-only traversal. Directory handles pin the path; files permit application writers.
use crate::{
    fs::*,
    platform::{known_folder, reveal_pinned_path},
};
use anyhow::{Result, ensure};
use dustaway_core::*;
use std::{
    collections::{HashMap, VecDeque},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Storage::FileSystem::*,
        UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_LocalAppData,
            FOLDERID_Profile, FOLDERID_RoamingAppData, FOLDERID_Windows,
        },
    },
    core::PCWSTR,
};

#[path = "analysis_evidence.rs"]
mod evidence;
use evidence::*;

pub struct WindowsAnalysis {
    ignored_cache: std::sync::RwLock<Vec<PathBuf>>,
    locations: Vec<(String, String)>,
    purposes: crate::analysis_purpose::PurposeMap,
    volumes: HashMap<String, AnalysisLocationInfo>,
}
impl Default for WindowsAnalysis {
    fn default() -> Self {
        Self::new()
    }
}
impl WindowsAnalysis {
    pub fn new() -> Self {
        let mut locations: Vec<(String, String)> = [
            ("当前用户文件夹", FOLDERID_Profile),
            ("下载", FOLDERID_Downloads),
            ("文档", FOLDERID_Documents),
            ("桌面", FOLDERID_Desktop),
            ("应用数据（本地）", FOLDERID_LocalAppData),
            ("应用数据（漫游）", FOLDERID_RoamingAppData),
        ]
        .into_iter()
        .filter_map(|(title, id)| {
            known_folder(&id)
                .ok()
                .map(|p| (title.into(), display_path(&p)))
        })
        .collect();
        let system = known_folder(&FOLDERID_Windows)
            .ok()
            .and_then(|p| p.ancestors().last().map(display_path));
        let mut volumes = HashMap::new();
        let drives = unsafe { GetLogicalDrives() };
        for n in 0..26 {
            if drives & (1 << n) == 0 {
                continue;
            }
            let path = format!("{}:\\", char::from(b'A' + n));
            let native = wide(Path::new(&path));
            if unsafe { GetDriveTypeW(PCWSTR(native.as_ptr())) }
                != windows::Win32::System::WindowsProgramming::DRIVE_FIXED
            {
                continue;
            }
            let supported = is_fixed_ntfs(Path::new(&path))
                .and_then(|_| ReadPin::chain(Path::new(&path), true).map(|_| ()))
                .and_then(|_| crate::platform::volume_target(Path::new(&path)).map(|_| ()))
                .is_ok();
            let is_system = system
                .as_ref()
                .is_some_and(|p| p.eq_ignore_ascii_case(&path));
            locations.push((
                format!("{}磁盘 {}", if is_system { "系统" } else { "本地" }, path),
                path.clone(),
            ));
            volumes.insert(
                path,
                AnalysisLocationInfo {
                    kind: "volume".into(),
                    is_system,
                    supported,
                    unavailable_reason: (!supported).then(|| {
                        "此磁盘无法读取，或不是本地 NTFS 卷；可使用 Windows 存储设置".into()
                    }),
                    expected_volume: None,
                },
            );
        }
        Self {
            ignored_cache: Default::default(),
            locations,
            purposes: crate::analysis_purpose::PurposeMap::new(),
            volumes,
        }
    }
    pub fn for_directory(path: &Path) -> Self {
        Self {
            ignored_cache: Default::default(),
            locations: vec![("当前用户文件夹".into(), display_path(path))],
            purposes: crate::analysis_purpose::PurposeMap::new(),
            volumes: HashMap::new(),
        }
    }

    /// Expose a marked fixture directory as the test disk without enumerating host drives.
    /// Analysis still walks only this directory; cleanup uses its separate fixture provider.
    #[cfg(debug_assertions)]
    pub fn for_fixture_volume(path: &Path) -> Result<Self> {
        ensure!(path.file_name().is_some_and(|name| name == "analysis"));
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("fixture parent missing"))?;
        ensure!(std::fs::read_to_string(parent.join(".dustaway-test-only"))? == "fixture-only");
        let mut fixture = Self::for_directory(path);
        fixture.volumes.insert(
            display_path(path),
            AnalysisLocationInfo {
                kind: "volume".into(),
                ..Default::default()
            },
        );
        Ok(fixture)
    }
}
struct ReadPin {
    _parents: Vec<Handle>,
    _handle: Handle,
    path: PathBuf,
    resolved: PathBuf,
    stamp: Stamp,
    parents: Vec<Identity>,
}
impl ReadPin {
    fn single(path: &Path, directory: bool) -> Result<Self> {
        let name = wide(path);
        let sharing = if directory {
            FILE_SHARE_READ
        } else {
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        };
        let handle = Handle(
            unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    FILE_READ_ATTRIBUTES.0,
                    sharing,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                    None,
                )
            }
            .map_err(|_| Refusal(ReasonCode::DirectoryInaccessible))?,
        );
        let stamp = stamp(&handle)?;
        ensure!(
            stamp.attributes
                & (FILE_ATTRIBUTE_REPARSE_POINT.0 | FILE_ATTRIBUTE_OFFLINE.0 | 0x40000 | 0x400000)
                == 0,
            Refusal(ReasonCode::LinkOrCloud)
        );
        ensure!(
            (stamp.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0) == directory,
            Refusal(ReasonCode::Changed)
        );
        let resolved = final_path(&handle)?;
        Ok(Self {
            _parents: vec![],
            _handle: handle,
            path: path.into(),
            resolved,
            stamp,
            parents: vec![],
        })
    }
    fn chain(path: &Path, directory: bool) -> Result<Self> {
        let path = extended(path)?;
        let mut parts: Vec<_> = path.ancestors().collect();
        parts.reverse();
        ensure!(parts.len() <= 256, Refusal(ReasonCode::Unverifiable));
        let mut handles = vec![];
        let mut ids = vec![];
        let mut previous: Option<PathBuf> = None;
        for (i, part) in parts.iter().enumerate() {
            let last = i == parts.len() - 1;
            let mut pin = Self::single(part, if last { directory } else { true })?;
            if let Some(parent) = &previous {
                ensure!(
                    pin.resolved.parent() == Some(parent.as_path()),
                    Refusal(ReasonCode::Changed)
                );
            } else {
                ensure!(
                    pin.resolved.parent().is_none(),
                    Refusal(ReasonCode::UnsupportedVolume)
                );
            }
            if last {
                pin._parents = handles;
                pin.parents = ids;
                return Ok(pin);
            }
            previous = Some(pin.resolved);
            ids.push(pin.stamp.identity);
            handles.push(pin._handle);
        }
        anyhow::bail!(Refusal(ReasonCode::Unverifiable))
    }
}
struct Frame {
    pin: ReadPin,
    entries: crate::analysis_listing::Listing,
    id: String,
    depth: usize,
    deferred: VecDeque<(PathBuf, Vec<u8>, String, usize)>,
    deferred_bytes: usize,
}
fn code(error: &anyhow::Error) -> String {
    serde_json::to_value(error_reason(error))
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unverifiable".into())
}
impl AnalysisPlatform for WindowsAnalysis {
    fn ignore_analysis_cache(&self, path: &str) -> Result<()> {
        let path = extended(Path::new(path))?;
        is_fixed_ntfs(&path)?;
        let pin = ReadPin::chain(&path, true)?;
        self.ignored_cache.write().unwrap().push(pin.path);
        Ok(())
    }

    fn manual_capability(&self, entry: &AnalysisEntry) -> ManualCapability {
        crate::manual::capability(entry)
    }
    fn manual_record_capability(&self, entry: &AnalysisEntry, opaque: &[u8]) -> ManualCapability {
        let mut capability = self.manual_capability(entry);
        if capability.allowed {
            let checked = (|| -> Result<()> {
                let metadata = decode(opaque)?;
                let attributes = metadata.attributes;
                let forbidden = FILE_ATTRIBUTE_REPARSE_POINT.0
                    | FILE_ATTRIBUTE_OFFLINE.0
                    | FILE_ATTRIBUTE_SYSTEM.0
                    | FILE_ATTRIBUTE_ENCRYPTED.0
                    | 0x40000
                    | 0x400000;
                ensure!(
                    attributes & forbidden == 0
                        && (entry.directory || attributes & FILE_ATTRIBUTE_READONLY.0 == 0),
                    Refusal(ReasonCode::Protected)
                );
                ensure!(
                    entry.directory || metadata.links.is_none_or(|n| n == 1),
                    Refusal(ReasonCode::LinkOrCloud)
                );
                Ok(())
            })();
            if let Err(error) = checked {
                capability.allowed = false;
                capability.reason = Some(error_reason(&error));
            }
        }
        capability
    }
    fn manual_record(&self, root: &AnalysisRoot, records: &[Vec<u8>]) -> Result<ManualRecord> {
        let pin = verified_node(root, records)?;
        let data = records.last().ok_or(Refusal(ReasonCode::Unverifiable))?;
        let metadata = decode(data)?;
        matches_pin(&metadata, &pin, true)?;
        let original = pin.stamp.clone();
        let directory = original.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
        crate::manual::allowed_stamp(&original)?;
        ensure!(
            !crate::manual::protected(&pin.path, directory),
            Refusal(ReasonCode::Protected)
        );
        let path = pin.path.clone();
        let parents = pin.parents.clone();
        let observed = pin.stamp.clone();
        drop(pin);
        let pin = PinnedPath::open(&path, directory, true)?;
        ensure!(
            pin.parent_ids == parents && pin.stamp == observed,
            Refusal(ReasonCode::Changed)
        );
        let root_snapshot: Snapshot = serde_json::from_str(&root.opaque)?;
        ensure!(
            native(&pin.path) != root_snapshot.path,
            Refusal(ReasonCode::Protected)
        );
        Ok(ManualRecord {
            identity: format!("{}:{}", original.identity.volume, original.identity.index),
            directory,
            file: FileRecord {
                path_display: display_path(&pin.path),
                bytes: if directory { 0 } else { original.size },
                modified_unix: original.modified_unix(),
                opaque: serde_json::to_string(&Snapshot {
                    application_identity: None,
                    path: native(&pin.path),
                    root: root_snapshot.path,
                    parents: pin.parent_ids,
                    stamp: original,
                })?,
            },
        })
    }
    fn delete_manual(&self, record: &ManualRecord, exclusions: &[String]) -> Result<()> {
        crate::manual::delete(record, exclusions)
    }
    fn location_info(&self, path: &str) -> AnalysisLocationInfo {
        self.volumes
            .get(&display_path(Path::new(path)))
            .cloned()
            .unwrap_or_default()
    }
    fn volume_key(&self, path: &str) -> Result<Option<String>> {
        Ok(Some(crate::platform::volume_target(Path::new(path))?.key))
    }
    fn advice(&self, entry: &AnalysisEntry) -> AnalysisAdvice {
        self.purposes.describe(entry)
    }
    fn context(&self, path: &str) -> Result<(String, u64)> {
        let root = self.validate_root(path)?;
        let volume = crate::platform::volume_target(Path::new(&root.path))?;
        let available = free_bytes(Path::new(&volume.opaque))?;
        Ok((volume.label, available))
    }
    fn locations(&self) -> Vec<(String, String)> {
        self.locations.clone()
    }
    fn validate_root(&self, path: &str) -> Result<AnalysisRoot> {
        let path = extended(Path::new(path))?;
        ensure!(
            path.parent().is_some() || self.volumes.contains_key(&display_path(&path)),
            ApiError::new("invalid_location", "请选择后端登记的本地磁盘或目录")
        );
        is_fixed_ntfs(&path)?;
        let pin = ReadPin::chain(&path, true)?;
        let opaque = serde_json::to_string(&Snapshot {
            application_identity: None,
            path: native(&path),
            root: native(&path),
            parents: pin.parents,
            stamp: pin.stamp,
        })?;
        Ok(AnalysisRoot {
            path: display_path(&path),
            opaque,
        })
    }
    fn scan(
        &self,
        root: &AnalysisRoot,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        let original: Snapshot = serde_json::from_str(&root.opaque)?;
        let path = from_native(&original.path);
        is_fixed_ntfs(&path)?;
        let pin = ReadPin::chain(&path, true)?;
        ensure!(
            pin.parents == original.parents && pin.stamp.identity == original.stamp.identity,
            Refusal(ReasonCode::Changed)
        );
        // Chromium markers and the temporary directory require only metadata. Firefox configuration
        // parsing is left to the cleanup scan, keeping analysis strictly attribute-only.
        let mut hints = vec![];
        if let Ok(platform) = crate::WindowsPlatform::new(Scope::User) {
            let local = extended(&known_folder(&FOLDERID_LocalAppData)?)?;
            for rule in crate::rules_for_analysis()? {
                if cancel.load(Ordering::Acquire) {
                    return Ok(());
                }
                // Do not enumerate unrelated application profiles when, for example, Downloads
                // is selected. Profile-marker checks only inform guidance, never file counting.
                let base = match rule.root.as_str() {
                    "user_temp" => local.join("Temp"),
                    root => match dustaway_core::browsers::chromium(root) {
                        Some(product) => local.join(product.user_data),
                        None => continue,
                    },
                };
                if !base.starts_with(&path) && !path.starts_with(&base) {
                    continue;
                }
                if let Ok(roots) = platform.roots(&rule) {
                    for p in roots {
                        hints.push((p, rule.id.clone()));
                    }
                }
            }
        }
        if !emit(AnalysisEvent::Entry(Box::new(record(
            &pin,
            "0".into(),
            None,
            &hints,
        )?)))? {
            return Ok(());
        }
        if !emit(AnalysisEvent::DirectoryStarted {
            node: "0".into(),
            path: display_path(&path),
        })? {
            return Ok(());
        }
        let entries = match crate::analysis_listing::Listing::open(&path) {
            Ok(entries) => entries,
            Err(_) => {
                emit(AnalysisEvent::Issue {
                    node: "0".into(),
                    code: "directory_inaccessible".into(),
                })?;
                return Ok(());
            }
        };
        let mut stack = vec![Frame {
            pin,
            entries,
            id: "0".into(),
            depth: 0,
            deferred: VecDeque::new(),
            deferred_bytes: 0,
        }];
        let mut serial = 0u64;
        while !stack.is_empty() {
            if cancel.load(Ordering::Acquire) {
                return Ok(());
            }
            let frame = stack.last_mut().unwrap();
            let next = match if frame.depth == 0
                && (frame.deferred.len() >= 1024 || frame.deferred_bytes >= 2 * 1024 * 1024)
            {
                None
            } else {
                frame.entries.next()
            } {
                Some(next) => next,
                None => {
                    if let Some((path, evidence, id, depth)) = frame.deferred.pop_front() {
                        frame.deferred_bytes = frame
                            .deferred_bytes
                            .saturating_sub(evidence.len() + path.as_os_str().len() * 2 + 128);
                        let opened = (|| -> Result<Frame> {
                            let pin = ReadPin::single(&path, true)?;
                            matches_pin(&decode(&evidence)?, &pin, true)?;
                            ensure!(
                                pin.resolved.parent() == Some(frame.pin.resolved.as_path()),
                                Refusal(ReasonCode::Changed)
                            );
                            let entries = crate::analysis_listing::Listing::open(&path)?;
                            Ok(Frame {
                                pin,
                                entries,
                                id: id.clone(),
                                depth,
                                deferred: VecDeque::new(),
                                deferred_bytes: 0,
                            })
                        })();
                        match opened {
                            Ok(next) => {
                                if !emit(AnalysisEvent::DirectoryStarted {
                                    node: id,
                                    path: display_path(&path),
                                })? {
                                    return Ok(());
                                }
                                stack.push(next);
                            }
                            Err(error) => {
                                if !emit(AnalysisEvent::Issue {
                                    node: id,
                                    code: code(&error),
                                })? {
                                    return Ok(());
                                }
                            }
                        }
                        continue;
                    }
                    let finished = stack.pop().unwrap();
                    if !emit(AnalysisEvent::DirectoryFinished { node: finished.id })? {
                        return Ok(());
                    }
                    continue;
                }
            };
            if !emit(AnalysisEvent::Checked)? {
                return Ok(());
            }
            let next = match next {
                Ok(next) => next,
                Err(_) => {
                    if !emit(AnalysisEvent::Issue {
                        node: frame.id.clone(),
                        code: "directory_inaccessible".into(),
                    })? {
                        return Ok(());
                    }
                    continue;
                }
            };
            let parent_id = frame.id.clone();
            let depth = frame.depth + 1;
            if depth > ANALYSIS_DEPTH {
                if !emit(AnalysisEvent::Issue {
                    node: parent_id,
                    code: "depth_limit".into(),
                })? {
                    return Ok(());
                }
                continue;
            }
            serial += 1;
            let id = serial.to_string();
            let (name, attrs) = match &next {
                crate::analysis_listing::Entry::Native(m) => (m.name.clone(), m.attributes),
                crate::analysis_listing::Entry::Legacy(e) => (
                    e.file_name(),
                    if e.file_type().is_ok_and(|t| t.is_dir()) {
                        FILE_ATTRIBUTE_DIRECTORY.0
                    } else {
                        0
                    },
                ),
            };
            let directory = attrs & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
            let path_child = frame.pin.path.join(&name);
            if self
                .ignored_cache
                .read()
                .unwrap()
                .iter()
                .any(|p| display_path(p).eq_ignore_ascii_case(&display_path(&path_child)))
            {
                continue;
            }

            let result = (|| -> Result<(AnalysisRecord, Option<ReadPin>)> {
                ensure!(
                    attrs
                        & (FILE_ATTRIBUTE_REPARSE_POINT.0
                            | FILE_ATTRIBUTE_OFFLINE.0
                            | 0x40000
                            | 0x400000)
                        == 0,
                    Refusal(ReasonCode::LinkOrCloud)
                );
                match next {
                    crate::analysis_listing::Entry::Native(mut m) if !directory => {
                        if attrs & (FILE_ATTRIBUTE_COMPRESSED.0 | FILE_ATTRIBUTE_SPARSE_FILE.0) != 0
                        {
                            m.allocated = ReadPin::single(&path_child, false)
                                .and_then(|p| {
                                    matches_pin(&m, &p, true)?;
                                    crate::analysis_listing::allocated(&p._handle, attrs)
                                })
                                .ok();
                        }
                        Ok((
                            record_metadata(
                                &m,
                                &path_child,
                                id.clone(),
                                Some(parent_id.clone()),
                                &hints,
                            ),
                            None,
                        ))
                    }
                    entry => {
                        let pin = ReadPin::single(&path_child, directory)?;
                        ensure!(
                            pin.resolved.parent() == Some(frame.pin.resolved.as_path()),
                            Refusal(ReasonCode::Changed)
                        );
                        if let crate::analysis_listing::Entry::Native(m) = &entry {
                            matches_pin(m, &pin, true)?;
                        }
                        Ok((
                            record(&pin, id.clone(), Some(parent_id.clone()), &hints)?,
                            Some(pin),
                        ))
                    }
                }
            })();
            let (record, pin) = match result {
                Ok(value) => value,
                Err(error) => {
                    let item = AnalysisEntry {
                        allocated_bytes: None,
                        allocation_unknown: 0,
                        verification_pending: false,
                        manual: Default::default(),
                        ancestors: vec![],
                        id,
                        parent: Some(parent_id),
                        name: name.to_string_lossy().into_owned(),
                        path: display_path(&path_child),
                        directory,
                        bytes: "0".into(),
                        files: 0,
                        modified_unix: 0,
                        shared: false,
                        incomplete: true,
                        pending: false,
                        issue: Some(code(&error)),
                        rule_ids: vec![],
                        excluded: false,
                        advice: None,
                    };
                    if !emit(AnalysisEvent::Entry(Box::new(AnalysisRecord {
                        entry: item,
                        file_key: None,
                        opaque: vec![],
                    })))? {
                        return Ok(());
                    }
                    continue;
                }
            };
            let evidence = directory.then(|| record.opaque.clone());
            if !emit(AnalysisEvent::Entry(Box::new(record)))? {
                return Ok(());
            }
            if directory {
                let pin = pin.unwrap();
                if depth == 1 {
                    let evidence = evidence.unwrap();
                    let frame = stack.last_mut().unwrap();
                    frame.deferred_bytes += evidence.len() + path_child.as_os_str().len() * 2 + 128;
                    frame.deferred.push_back((path_child, evidence, id, depth));
                    continue;
                }
                match crate::analysis_listing::Listing::open(&path_child) {
                    Ok(entries) => {
                        if !emit(AnalysisEvent::DirectoryStarted {
                            node: id.clone(),
                            path: display_path(&path_child),
                        })? {
                            return Ok(());
                        }
                        stack.push(Frame {
                            pin,
                            entries,
                            id,
                            depth,
                            deferred: VecDeque::new(),
                            deferred_bytes: 0,
                        });
                    }
                    Err(_) => {
                        if !emit(AnalysisEvent::Issue {
                            node: id,
                            code: "directory_inaccessible".into(),
                        })? {
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn refine(&self, root: &AnalysisRoot, records: &[Vec<u8>]) -> Result<String> {
        let pin = verified_node(root, records)?;
        ensure!(
            pin.stamp.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0,
            Refusal(ReasonCode::Changed)
        );
        Ok(display_path(&pin.path))
    }
    fn reveal(&self, root: &AnalysisRoot, records: &[Vec<u8>]) -> Result<()> {
        let pin = verified_node(root, records)?;
        reveal_pinned_path(&pin.path)
    }
}
