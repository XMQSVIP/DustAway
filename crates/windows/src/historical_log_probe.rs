//! Read-only OS observations. Decisions and deletion permission live elsewhere.
use crate::{applications, fs::*, platform::known_folder, security};
use anyhow::{Result, ensure};
use std::{
    fs::Metadata,
    mem::size_of,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use windows::{
    Win32::{
        Foundation::{
            ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_FILES, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND,
        },
        System::{Diagnostics::ToolHelp::*, Registry::*, Threading::*},
        UI::Shell::{FOLDERID_LocalAppData, FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86},
    },
    core::{PCWSTR, PWSTR},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Hive {
    User,
    Machine,
}
impl Hive {
    fn handle(self) -> HKEY {
        match self {
            Self::User => HKEY_CURRENT_USER,
            Self::Machine => HKEY_LOCAL_MACHINE,
        }
    }
}
pub(crate) struct Discovery {
    pub paths: Vec<PathBuf>,
    // Missing standard locations are normal; a registered executable must be verifiable.
    pub registered_paths: Vec<PathBuf>,
    pub complete: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct ProcessObservation {
    pub name: String,
    pub session: Option<u32>,
    pub image: Option<PathBuf>,
}
#[derive(Clone, Debug)]
pub(crate) struct Processes {
    pub current_session: Option<u32>,
    pub entries: Vec<ProcessObservation>,
    pub complete: bool,
}
pub(crate) type Entries = Box<dyn Iterator<Item = Result<PathBuf>> + Send>;
pub(crate) trait Probe: Send + Sync {
    fn registry_string(&self, hive: Hive, key: &str, name: &str) -> Result<Option<String>>;
    fn registry_children(
        &self,
        hive: Hive,
        key: &str,
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<String>>;
    fn standard_todesk_paths(&self, cancel: Option<&AtomicBool>) -> Result<Vec<PathBuf>>;
    fn dingtalk_candidates(&self, cancel: Option<&AtomicBool>) -> Result<Vec<PathBuf>>;
    fn signed_by(
        &self,
        pin: &PinnedPath,
        publisher: &str,
        cancel: Option<&AtomicBool>,
    ) -> Result<bool>;
    fn product(&self, pin: &PinnedPath, cancel: Option<&AtomicBool>) -> Result<String>;
    fn version(&self, pin: &PinnedPath, cancel: Option<&AtomicBool>) -> Result<String>;
    fn processes(&self, cancel: Option<&AtomicBool>) -> Result<Processes>;
    fn read_dir(&self, root: &Path) -> Result<Entries> {
        Ok(Box::new(std::fs::read_dir(root)?.map(|e| Ok(e?.path()))))
    }
    fn metadata(&self, path: &Path) -> Result<Metadata> {
        Ok(std::fs::symlink_metadata(path)?)
    }
}

pub(crate) fn observed<T>(
    cancel: Option<&AtomicBool>,
    call: impl FnOnce() -> Result<T>,
) -> Result<T> {
    check_cancel(cancel)?;
    let result = call();
    check_cancel(cancel)?;
    result
}
fn collect<T>(result: Result<T>, complete: &mut bool) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.is::<ScanCancelled>() => Err(e),
        Err(_) => {
            *complete = false;
            Ok(None)
        }
    }
}
pub(crate) fn discover_todesk(probe: &dyn Probe, cancel: Option<&AtomicBool>) -> Result<Discovery> {
    let mut result = Discovery {
        paths: vec![],
        registered_paths: vec![],
        complete: true,
    };
    for hive in [Hive::User, Hive::Machine] {
        for prefix in [r"Software", r"Software\WOW6432Node"] {
            let key = format!(r"{prefix}\Microsoft\Windows\CurrentVersion\App Paths\ToDesk.exe");
            let path = collect(
                observed(cancel, || probe.registry_string(hive, &key, "")),
                &mut result.complete,
            )?
            .flatten();
            if let Some(value) = path {
                add_executable(&value, &mut result);
            }
            let root = format!(r"{prefix}\Microsoft\Windows\CurrentVersion\Uninstall");
            let children = collect(
                observed(cancel, || probe.registry_children(hive, &root, cancel)),
                &mut result.complete,
            )?
            .unwrap_or_default();
            for child in children {
                check_cancel(cancel)?;
                let key = format!(r"{root}\{child}");
                let name = collect(
                    observed(cancel, || probe.registry_string(hive, &key, "DisplayName")),
                    &mut result.complete,
                )?
                .flatten();
                if !name
                    .as_deref()
                    .is_some_and(|s| s.trim().eq_ignore_ascii_case("ToDesk"))
                {
                    continue;
                }
                let before = result.registered_paths.len();
                let location = collect(
                    observed(cancel, || {
                        probe.registry_string(hive, &key, "InstallLocation")
                    }),
                    &mut result.complete,
                )?
                .flatten();
                if let Some(location) = location.filter(|s| !s.trim().is_empty()) {
                    let path = PathBuf::from(location.trim().trim_matches('"'));
                    if path.is_absolute() {
                        let executable = path.join("ToDesk.exe");
                        result.registered_paths.push(executable.clone());
                        result.paths.push(executable);
                    } else {
                        result.complete = false;
                    }
                }
                let icon = collect(
                    observed(cancel, || probe.registry_string(hive, &key, "DisplayIcon")),
                    &mut result.complete,
                )?
                .flatten();
                if let Some(icon) = icon.filter(|s| !s.trim().is_empty()) {
                    add_executable(&icon, &mut result);
                }
                if result.registered_paths.len() == before {
                    result.complete = false;
                }
            }
        }
    }
    if let Some(paths) = collect(
        observed(cancel, || probe.standard_todesk_paths(cancel)),
        &mut result.complete,
    )? {
        result.paths.extend(paths);
    }
    result.paths.sort();
    result.paths.dedup();
    check_cancel(cancel)?;
    Ok(result)
}
fn add_executable(value: &str, found: &mut Discovery) {
    match applications::resource_path(value) {
        Some(path)
            if path
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("ToDesk.exe")) =>
        {
            found.registered_paths.push(path.clone());
            found.paths.push(path)
        }
        _ => found.complete = false,
    }
}

pub(crate) struct WindowsProbe;
#[cfg(test)]
#[path = "historical_log_fixture.rs"]
pub(crate) mod fixture;
struct RegistryKey(HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
impl Probe for WindowsProbe {
    fn registry_string(&self, hive: Hive, key: &str, name: &str) -> Result<Option<String>> {
        let key = text_wide(key);
        let name = text_wide(name);
        let mut buffer = vec![0u16; 32768];
        let mut bytes = (buffer.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                hive.handle(),
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
            return Ok(None);
        }
        status.ok()?;
        ensure!(
            bytes >= 2 && bytes as usize <= buffer.len() * 2 && bytes.is_multiple_of(2),
            "invalid registry string length"
        );
        let data = &buffer[..bytes as usize / 2];
        ensure!(
            data.last() == Some(&0) && !data[..data.len() - 1].contains(&0),
            "invalid registry string"
        );
        Ok(Some(String::from_utf16(&data[..data.len() - 1])?))
    }
    fn registry_children(
        &self,
        hive: Hive,
        key: &str,
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<String>> {
        check_cancel(cancel)?;
        let key = text_wide(key);
        let mut raw = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                hive.handle(),
                PCWSTR(key.as_ptr()),
                None,
                KEY_READ,
                &mut raw,
            )
        };
        if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
            return Ok(vec![]);
        }
        status.ok()?;
        let handle = RegistryKey(raw);
        let mut found = vec![];
        for i in 0..4096 {
            check_cancel(cancel)?;
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    handle.0,
                    i,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            check_cancel(cancel)?;
            if status == ERROR_NO_MORE_ITEMS {
                return Ok(found);
            }
            status.ok()?;
            ensure!(len as usize <= name.len(), "invalid registry child length");
            found.push(String::from_utf16(&name[..len as usize])?);
        }
        anyhow::bail!("registry enumeration limit reached")
    }
    fn standard_todesk_paths(&self, cancel: Option<&AtomicBool>) -> Result<Vec<PathBuf>> {
        [
            (&FOLDERID_LocalAppData, "Programs/ToDesk/ToDesk.exe"),
            (&FOLDERID_ProgramFiles, "ToDesk/ToDesk.exe"),
            (&FOLDERID_ProgramFilesX86, "ToDesk/ToDesk.exe"),
        ]
        .into_iter()
        .map(|(id, relative)| observed(cancel, || Ok(known_folder(id)?.join(relative))))
        .collect()
    }
    fn dingtalk_candidates(&self, cancel: Option<&AtomicBool>) -> Result<Vec<PathBuf>> {
        applications::candidates_cancellable(
            dustaway_core::applications::application("dingtalk").unwrap(),
            cancel,
        )
    }
    fn signed_by(
        &self,
        pin: &PinnedPath,
        publisher: &str,
        cancel: Option<&AtomicBool>,
    ) -> Result<bool> {
        applications::signed_by_cancellable(pin, publisher, cancel)
    }
    fn product(&self, pin: &PinnedPath, cancel: Option<&AtomicBool>) -> Result<String> {
        applications::executable_product_name(&pin.path, cancel)
    }
    fn version(&self, pin: &PinnedPath, cancel: Option<&AtomicBool>) -> Result<String> {
        applications::executable_version_cancellable(&pin.path, cancel)
    }
    fn processes(&self, cancel: Option<&AtomicBool>) -> Result<Processes> {
        check_cancel(cancel)?;
        let mut found = Processes {
            current_session: security::process_session(std::process::id()).ok(),
            entries: vec![],
            complete: true,
        };
        check_cancel(cancel)?;
        let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
        check_cancel(cancel)?;
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        loop {
            check_cancel(cancel)?;
            match next {
                Ok(()) => {}
                Err(e) if e.code() == ERROR_NO_MORE_FILES.to_hresult() => return Ok(found),
                Err(_) => {
                    found.complete = false;
                    return Ok(found);
                }
            }
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let session = security::process_session(entry.th32ProcessID).ok();
            check_cancel(cancel)?;
            let image = (|| -> Result<PathBuf> {
                let process = Handle(unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    )
                }?);
                check_cancel(cancel)?;
                security::process_image(process.0)
            })()
            .ok();
            check_cancel(cancel)?;
            found.entries.push(ProcessObservation {
                name: String::from_utf16_lossy(&entry.szExeFile[..end]),
                session,
                image,
            });
            next = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
    }
}
