//! Narrow Win32 filesystem boundary. No recursive deletion and no path-based unlink.
//! Handles pin every directory component until final identity validation + disposition.
use anyhow::{Context, Result, ensure};
use dustaway_core::{ReasonCode, Refusal};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    mem::size_of,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Component, Path, PathBuf, Prefix},
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::*,
        System::WindowsProgramming::DRIVE_FIXED,
    },
    core::PCWSTR,
};

pub struct Handle(pub HANDLE);
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
pub fn text_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn native(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().collect()
}
pub fn from_native(path: &[u16]) -> PathBuf {
    PathBuf::from(OsString::from_wide(path))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub volume: u32,
    pub index: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stamp {
    pub identity: Identity,
    pub size: u64,
    pub created: u64,
    pub modified: u64,
    pub changed: i64,
    pub attributes: u32,
    pub links: u32,
}
impl Stamp {
    pub fn modified_unix(&self) -> i64 {
        (self.modified / 10_000_000) as i64 - 11_644_473_600
    }
    fn directory(&self) -> bool {
        self.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0
    }
    fn validate(&self) -> Result<()> {
        self.validate_for_read()?;
        if !self.directory() {
            ensure!(self.links == 1, Refusal(ReasonCode::LinkOrCloud));
            ensure!(
                self.attributes
                    & (FILE_ATTRIBUTE_READONLY.0
                        | FILE_ATTRIBUTE_SYSTEM.0
                        | FILE_ATTRIBUTE_ENCRYPTED.0)
                    == 0,
                Refusal(ReasonCode::Protected)
            );
        }
        Ok(())
    }
    fn validate_for_read(&self) -> Result<()> {
        // Recall flags also cover cloud files that are not currently marked OFFLINE.
        let forbidden =
            FILE_ATTRIBUTE_REPARSE_POINT.0 | FILE_ATTRIBUTE_OFFLINE.0 | 0x40000 | 0x400000;
        ensure!(
            self.attributes & forbidden == 0,
            Refusal(ReasonCode::LinkOrCloud)
        );
        Ok(())
    }
}

pub fn stamp(handle: &Handle) -> Result<Stamp> {
    stamp_cancellable(handle, None)
}
fn stamp_cancellable(handle: &Handle, cancel: Option<&AtomicBool>) -> Result<Stamp> {
    check_cancel(cancel)?;
    unsafe {
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        GetFileInformationByHandle(handle.0, &mut info)?;
        check_cancel(cancel)?;
        let mut basic = FILE_BASIC_INFO::default();
        GetFileInformationByHandleEx(
            handle.0,
            FileBasicInfo,
            (&mut basic as *mut FILE_BASIC_INFO).cast(),
            size_of::<FILE_BASIC_INFO>() as u32,
        )?;
        check_cancel(cancel)?;
        let join = |hi: u32, lo: u32| (u64::from(hi) << 32) | u64::from(lo);
        Ok(Stamp {
            identity: Identity {
                volume: info.dwVolumeSerialNumber,
                index: join(info.nFileIndexHigh, info.nFileIndexLow),
            },
            size: join(info.nFileSizeHigh, info.nFileSizeLow),
            created: join(
                info.ftCreationTime.dwHighDateTime,
                info.ftCreationTime.dwLowDateTime,
            ),
            modified: join(
                info.ftLastWriteTime.dwHighDateTime,
                info.ftLastWriteTime.dwLowDateTime,
            ),
            changed: basic.ChangeTime,
            attributes: info.dwFileAttributes,
            links: info.nNumberOfLinks,
        })
    }
}

pub fn extended(path: &Path) -> Result<PathBuf> {
    let mut components = path.components();
    let prefix = match components.next() {
        Some(Component::Prefix(p)) => p.kind(),
        _ => anyhow::bail!("必须使用本地绝对路径"),
    };
    let drive = match prefix {
        Prefix::Disk(d) | Prefix::VerbatimDisk(d) => d,
        _ => anyhow::bail!("仅支持本地磁盘路径"),
    };
    ensure!(
        matches!(components.next(), Some(Component::RootDir)),
        "不支持驱动器相对路径"
    );
    let mut result = PathBuf::from(format!("\\\\?\\{}:\\", char::from(drive)));
    for part in components {
        let Component::Normal(name) = part else {
            anyhow::bail!("路径包含非标准分量")
        };
        let encoded: Vec<_> = name.encode_wide().collect();
        ensure!(
            !encoded.iter().any(|c| *c == 0 || *c == b':' as u16),
            "不支持设备路径或备用数据流"
        );
        result.push(name);
    }
    Ok(result)
}

pub fn is_fixed_ntfs(path: &Path) -> Result<()> {
    is_fixed_ntfs_cancellable(path, None)
}
pub(crate) fn is_fixed_ntfs_cancellable(path: &Path, cancel: Option<&AtomicBool>) -> Result<()> {
    check_cancel(cancel)?;
    let path = extended(path)?;
    let drive: PathBuf = path
        .ancestors()
        .last()
        .context("无效磁盘根目录")?
        .to_path_buf();
    let root = wide(&drive);
    unsafe {
        ensure!(
            GetDriveTypeW(PCWSTR(root.as_ptr())) == DRIVE_FIXED,
            Refusal(ReasonCode::UnsupportedVolume)
        );
        check_cancel(cancel)?;
        let mut name = [0u16; 32];
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            None,
            None,
            None,
            None,
            Some(&mut name),
        )?;
        check_cancel(cancel)?;
        let len = name.iter().position(|c| *c == 0).unwrap_or(name.len());
        ensure!(
            String::from_utf16_lossy(&name[..len]) == "NTFS",
            Refusal(ReasonCode::UnsupportedVolume)
        );
    }
    Ok(())
}

pub struct PinnedPath {
    _parents: Vec<Handle>,
    pub parent_ids: Vec<Identity>,
    pub handle: Handle,
    pub stamp: Stamp,
    pub path: PathBuf,
}
pub(crate) fn final_path(handle: &Handle) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let length =
        unsafe { GetFinalPathNameByHandleW(handle.0, &mut buffer, VOLUME_NAME_GUID) } as usize;
    ensure!(
        length > 0 && length < buffer.len(),
        "无法确认文件的实际卷路径"
    );
    Ok(from_native(&buffer[..length]))
}
impl PinnedPath {
    /// Messaging attachments are commonly READONLY. This entry point is only
    /// used after the independent attachment capability/identity checks.
    pub(crate) fn open_attachment(path: &Path, deleting: bool) -> Result<Self> {
        let pin = Self::open_mode(path, false, deleting, None, true)?;
        ensure!(pin.stamp.links == 1, Refusal(ReasonCode::LinkOrCloud));
        ensure!(
            pin.stamp.attributes & (FILE_ATTRIBUTE_SYSTEM.0 | FILE_ATTRIBUTE_ENCRYPTED.0) == 0,
            Refusal(ReasonCode::Protected)
        );
        Ok(pin)
    }
    /// Does not clear READONLY, change an ACL, or use POSIX deletion semantics.
    pub(crate) fn mark_attachment_delete(self) -> Result<()> {
        ensure!(
            !self.stamp.directory() && self.stamp.links == 1,
            Refusal(ReasonCode::Protected)
        );
        if self.stamp.attributes & FILE_ATTRIBUTE_READONLY.0 == 0 {
            return self.mark_delete();
        }
        let disposition = FILE_DISPOSITION_INFO_EX {
            Flags: FILE_DISPOSITION_INFO_EX_FLAGS(
                FILE_DISPOSITION_FLAG_DELETE.0 | FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE.0,
            ),
        };
        unsafe {
            SetFileInformationByHandle(
                self.handle.0,
                FileDispositionInfoEx,
                (&disposition as *const FILE_DISPOSITION_INFO_EX).cast(),
                size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
            )
            .map_err(|e| Refusal(win32_reason(e.code().0 as u32 & 0xffff)))?;
        }
        Ok(())
    }
    pub fn open(path: &Path, directory: bool, deleting: bool) -> Result<Self> {
        Self::open_cancellable(path, directory, deleting, None)
    }
    pub(crate) fn open_cancellable(
        path: &Path,
        directory: bool,
        deleting: bool,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self> {
        Self::open_mode(path, directory, deleting, cancel, false)
    }
    /// Only for metadata observation; never yields a deletion-capable handle.
    pub(crate) fn observe(
        path: &Path,
        directory: bool,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self> {
        Self::open_mode(path, directory, false, cancel, true)
    }
    fn open_mode(
        path: &Path,
        directory: bool,
        deleting: bool,
        cancel: Option<&AtomicBool>,
        observe: bool,
    ) -> Result<Self> {
        check_cancel(cancel)?;
        let path = extended(path)?;
        let mut chain: Vec<_> = path.ancestors().map(Path::to_path_buf).collect();
        chain.reverse();
        ensure!(chain.len() <= 128, "目录层级过深，已跳过");
        let mut parents = vec![];
        let mut parent_ids = vec![];
        let mut previous: Option<PathBuf> = None;
        for (index, component) in chain.iter().enumerate() {
            check_cancel(cancel)?;
            let final_item = index + 1 == chain.len();
            let access = FILE_READ_ATTRIBUTES.0 | if final_item && deleting { DELETE.0 } else { 0 };
            // Deny write and delete sharing: reparse mutation/rename must fail while pinned.
            let name = wide(component);
            let handle = Handle(
                unsafe {
                    CreateFileW(
                        PCWSTR(name.as_ptr()),
                        access,
                        FILE_SHARE_READ,
                        None,
                        OPEN_EXISTING,
                        FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                        None,
                    )
                }
                .map_err(|e| Refusal(win32_reason(e.code().0 as u32 & 0xffff)))?,
            );
            check_cancel(cancel)?;
            let info = stamp_cancellable(&handle, cancel)?;
            if observe {
                info.validate_for_read()?;
            } else {
                info.validate()?;
            }
            check_cancel(cancel)?;
            let resolved = final_path(&handle)?;
            check_cancel(cancel)?;
            if let Some(parent) = &previous {
                ensure!(
                    resolved.parent() == Some(parent.as_path()),
                    Refusal(ReasonCode::Changed)
                );
            } else {
                ensure!(
                    resolved.parent().is_none(),
                    "不支持映射到子目录的驱动器别名"
                );
            }
            previous = Some(resolved);
            if final_item {
                ensure!(info.directory() == directory, Refusal(ReasonCode::Changed));
                return Ok(Self {
                    _parents: parents,
                    parent_ids,
                    handle,
                    stamp: info,
                    path,
                });
            }
            ensure!(info.directory(), "父路径不是目录");
            parent_ids.push(info.identity);
            parents.push(handle);
        }
        anyhow::bail!("无效路径")
    }
    pub fn mark_delete(self) -> Result<()> {
        ensure!(!self.stamp.directory(), "禁止删除目录");
        self.mark_disposition()
    }
    pub(crate) fn mark_empty_directory_delete(self) -> Result<()> {
        ensure!(self.stamp.directory(), "需要空目录");
        self.mark_disposition()
    }
    fn mark_disposition(self) -> Result<()> {
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        let result = unsafe {
            SetFileInformationByHandle(
                self.handle.0,
                FileDispositionInfo,
                (&disposition as *const FILE_DISPOSITION_INFO).cast(),
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if let Err(error) = result {
            // Access, sharing, lock and write-protection refusals remain safe skips.
            if [5, 32, 33, 19, 145]
                .iter()
                .any(|code| error.code() == windows::core::HRESULT::from_win32(*code))
            {
                return Err(Refusal(win32_reason(error.code().0 as u32 & 0xffff)).into());
            }
            return Err(dustaway_core::DeleteFailure(format!("文件删除失败：{error}")).into());
        }
        Ok(())
    }
}

pub fn win32_reason(code: u32) -> ReasonCode {
    match code {
        5 => ReasonCode::AccessDenied,
        32 | 33 => ReasonCode::InUse,
        19 => ReasonCode::Protected,
        2 | 3 => ReasonCode::Changed,
        145 => ReasonCode::Changed,
        _ => ReasonCode::Unknown,
    }
}
pub fn error_reason(error: &anyhow::Error) -> ReasonCode {
    let known = dustaway_core::reason_for(error);
    if known != ReasonCode::Unknown {
        return known;
    }
    if let Some(e) = error.downcast_ref::<std::io::Error>() {
        return e
            .raw_os_error()
            .map(|c| win32_reason(c as u32))
            .unwrap_or(ReasonCode::Unknown);
    }
    if let Some(e) = error.downcast_ref::<windows::core::Error>() {
        return win32_reason(e.code().0 as u32 & 0xffff);
    }
    ReasonCode::Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub application_identity: Option<String>,
    pub path: Vec<u16>,
    pub root: Vec<u16>,
    pub parents: Vec<Identity>,
    pub stamp: Stamp,
}

pub fn excluded(path: &Path, exclusions: &[String]) -> bool {
    let path = extended(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase();
    exclusions.iter().any(|item| {
        let Ok(item) = extended(Path::new(item)) else {
            return false;
        };
        let item = item.to_string_lossy().trim_end_matches('\\').to_lowercase();
        path == item
            || path
                .strip_prefix(&item)
                .is_some_and(|suffix| suffix.starts_with('\\'))
    })
}

pub(crate) struct CompiledExclusions(Vec<String>);
impl CompiledExclusions {
    pub fn new(exclusions: &[String]) -> Self {
        Self(
            exclusions
                .iter()
                .filter_map(|item| extended(Path::new(item)).ok())
                .map(|item| item.to_string_lossy().trim_end_matches('\\').to_lowercase())
                .collect(),
        )
    }
    pub fn contains(&self, path: &Path) -> bool {
        if self.0.is_empty() {
            return false;
        }
        let path = extended(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_lowercase();
        self.0.iter().any(|item| {
            path == *item
                || path
                    .strip_prefix(item)
                    .is_some_and(|suffix| suffix.starts_with('\\'))
        })
    }
}

#[derive(Debug)]
pub(crate) struct ScanCancelled;
impl std::fmt::Display for ScanCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("扫描已停止")
    }
}
impl std::error::Error for ScanCancelled {}
pub(crate) fn check_cancel(cancel: Option<&AtomicBool>) -> Result<()> {
    if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
        return Err(ScanCancelled.into());
    }
    Ok(())
}

pub fn free_bytes(path: &Path) -> Result<u64> {
    let name = wide(path);
    let mut available = 0;
    unsafe {
        GetDiskFreeSpaceExW(PCWSTR(name.as_ptr()), Some(&mut available), None, None)?;
    }
    Ok(available)
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_owned()
}
