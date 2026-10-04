//! Attribute-only, bounded directory enumeration. Never follows a reparse point.
use crate::fs::*;
use anyhow::{Result, ensure};
use std::{
    ffi::OsString,
    mem::{offset_of, size_of},
    os::windows::ffi::OsStringExt,
    path::Path,
};
use windows::{
    Win32::{Foundation::*, Storage::FileSystem::*},
    core::PCWSTR,
};

const BUFFER: usize = 64 * 1024;
const NAME: usize = offset_of!(FILE_ID_EXTD_DIR_INFO, FileName);

#[derive(Clone, Debug)]
pub(super) struct Metadata {
    pub name: OsString,
    pub volume: u64,
    pub id: [u8; 16],
    pub attributes: u32,
    pub size: u64,
    pub allocated: Option<u64>,
    pub created: u64,
    pub modified: u64,
    pub changed: i64,
    pub links: Option<u32>,
}
pub(super) fn identity(handle: &Handle) -> Result<(u64, [u8; 16])> {
    let mut info = FILE_ID_INFO::default();
    // SAFETY: handle remains open and info is a correctly sized writable structure.
    unsafe {
        GetFileInformationByHandleEx(
            handle.0,
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )?;
    }
    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}
pub(super) fn allocated(handle: &Handle, attributes: u32) -> Result<u64> {
    if attributes & (FILE_ATTRIBUTE_COMPRESSED.0 | FILE_ATTRIBUTE_SPARSE_FILE.0) != 0 {
        let mut info = FILE_COMPRESSION_INFO::default();
        // SAFETY: the structure and information class agree, and the handle is live.
        unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                FileCompressionInfo,
                (&mut info as *mut FILE_COMPRESSION_INFO).cast(),
                size_of::<FILE_COMPRESSION_INFO>() as u32,
            )?;
        }
        Ok(u64::try_from(info.CompressedFileSize)?)
    } else {
        let mut info = FILE_STANDARD_INFO::default();
        // SAFETY: the structure and information class agree, and the handle is live.
        unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                FileStandardInfo,
                (&mut info as *mut FILE_STANDARD_INFO).cast(),
                size_of::<FILE_STANDARD_INFO>() as u32,
            )?;
        }
        Ok(u64::try_from(info.AllocationSize)?)
    }
}
pub(super) enum Listing {
    Native(Box<Native>),
    Legacy(Box<std::fs::ReadDir>),
}
pub(super) enum Entry {
    Native(Metadata),
    Legacy(Box<std::fs::DirEntry>),
}
pub(super) struct Native {
    handle: Handle,
    buffer: Vec<u64>,
    next: Option<usize>,
    done: bool,
    volume: u64,
}
impl Listing {
    pub fn open(path: &Path) -> Result<Self> {
        let path_w = wide(path);
        // SAFETY: path_w is a live nul-terminated UTF-16 string. The returned handle is owned.
        let handle = Handle(unsafe {
            CreateFileW(
                PCWSTR(path_w.as_ptr()),
                FILE_LIST_DIRECTORY.0 | FILE_READ_ATTRIBUTES.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )?
        });
        let stamp = stamp(&handle)?;
        ensure!(
            stamp.attributes
                & (FILE_ATTRIBUTE_REPARSE_POINT.0 | FILE_ATTRIBUTE_OFFLINE.0 | 0x40000 | 0x400000)
                == 0,
            "link or cloud directory"
        );
        let volume = identity(&handle)?.0;
        let mut native = Native {
            handle,
            buffer: vec![0; BUFFER / 8],
            next: None,
            done: false,
            volume,
        };
        match native.fill() {
            Ok(()) => Ok(Self::Native(Box::new(native))),
            Err(error)
                if [
                    ERROR_INVALID_FUNCTION,
                    ERROR_INVALID_LEVEL,
                    ERROR_INVALID_PARAMETER,
                    ERROR_NOT_SUPPORTED,
                ]
                .iter()
                .any(|code| error.code() == code.to_hresult()) =>
            {
                Ok(Self::Legacy(Box::new(std::fs::read_dir(path)?)))
            }
            Err(error) => Err(error.into()),
        }
    }
    pub fn next(&mut self) -> Option<Result<Entry>> {
        match self {
            Self::Legacy(entries) => entries.next().map(|entry| {
                entry
                    .map(|e| Entry::Legacy(Box::new(e)))
                    .map_err(Into::into)
            }),
            Self::Native(native) => native.next().map(|entry| entry.map(Entry::Native)),
        }
    }
}
impl Native {
    fn fill(&mut self) -> windows::core::Result<()> {
        self.buffer.fill(0);
        // SAFETY: u64 storage is sufficiently aligned and has BUFFER initialized writable bytes.
        let result = unsafe {
            GetFileInformationByHandleEx(
                self.handle.0,
                FileIdExtdDirectoryInfo,
                self.buffer.as_mut_ptr().cast(),
                BUFFER as u32,
            )
        };
        match result {
            Ok(()) => {
                self.next = Some(0);
                Ok(())
            }
            Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => {
                self.done = true;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
    fn next(&mut self) -> Option<Result<Metadata>> {
        loop {
            if self.done {
                return None;
            }
            let Some(offset) = self.next else {
                if let Err(error) = self.fill() {
                    self.done = true;
                    return Some(Err(error.into()));
                }
                continue;
            };
            // SAFETY: byte view of initialized u64 allocation, bounded to its exact byte length.
            let bytes =
                unsafe { std::slice::from_raw_parts(self.buffer.as_ptr().cast::<u8>(), BUFFER) };
            match parse(bytes, offset, self.volume) {
                Ok((entry, next)) => {
                    self.next = next;
                    if entry.name != "." && entry.name != ".." {
                        return Some(Ok(entry));
                    }
                }
                Err(error) => {
                    self.done = true;
                    return Some(Err(error));
                }
            }
        }
    }
}
fn parse(bytes: &[u8], offset: usize, volume: u64) -> Result<(Metadata, Option<usize>)> {
    let header = bytes
        .get(
            offset
                ..offset
                    .checked_add(NAME)
                    .ok_or_else(|| anyhow::anyhow!("offset overflow"))?,
        )
        .ok_or_else(|| anyhow::anyhow!("short directory header"))?;
    let u32_at = |at| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
    let i64_at = |at| i64::from_le_bytes(header[at..at + 8].try_into().unwrap());
    let length = u32_at(offset_of!(FILE_ID_EXTD_DIR_INFO, FileNameLength)) as usize;
    ensure!(
        length > 0 && length.is_multiple_of(2),
        "invalid UTF-16 name length"
    );
    let end = offset
        .checked_add(NAME)
        .and_then(|n| n.checked_add(length))
        .ok_or_else(|| anyhow::anyhow!("name overflow"))?;
    let name = bytes
        .get(offset + NAME..end)
        .ok_or_else(|| anyhow::anyhow!("truncated directory name"))?;
    let units: Vec<u16> = name
        .chunks_exact(2)
        .map(|n| u16::from_le_bytes([n[0], n[1]]))
        .collect();
    ensure!(
        !units.iter().any(|c| [0, 47, 58, 92].contains(c)),
        "invalid path component"
    );
    let step = u32_at(offset_of!(FILE_ID_EXTD_DIR_INFO, NextEntryOffset)) as usize;
    let next = if step == 0 {
        None
    } else {
        ensure!(
            step >= NAME + length && step.is_multiple_of(8),
            "invalid next record"
        );
        let next = offset
            .checked_add(step)
            .ok_or_else(|| anyhow::anyhow!("record overflow"))?;
        ensure!(next + NAME <= bytes.len(), "next record outside buffer");
        Some(next)
    };
    let id_at = offset_of!(FILE_ID_EXTD_DIR_INFO, FileId);
    Ok((
        Metadata {
            name: OsString::from_wide(&units),
            volume,
            id: header[id_at..id_at + 16].try_into()?,
            attributes: u32_at(offset_of!(FILE_ID_EXTD_DIR_INFO, FileAttributes)),
            size: u64::try_from(i64_at(offset_of!(FILE_ID_EXTD_DIR_INFO, EndOfFile)))?,
            allocated: u64::try_from(i64_at(offset_of!(FILE_ID_EXTD_DIR_INFO, AllocationSize)))
                .ok(),
            created: i64_at(offset_of!(FILE_ID_EXTD_DIR_INFO, CreationTime)) as u64,
            modified: i64_at(offset_of!(FILE_ID_EXTD_DIR_INFO, LastWriteTime)) as u64,
            changed: i64_at(offset_of!(FILE_ID_EXTD_DIR_INFO, ChangeTime)),
            links: None,
        },
        next,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_directory_records_are_rejected() {
        assert!(parse(&[0; 8], 0, 1).is_err());
        let mut buffer = vec![0; NAME + 8];
        let at = offset_of!(FILE_ID_EXTD_DIR_INFO, FileNameLength);
        buffer[at..at + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(parse(&buffer, 0, 1).is_err());
        buffer[at..at + 4].copy_from_slice(&2u32.to_le_bytes());
        buffer[NAME..NAME + 2].copy_from_slice(&(b'x' as u16).to_le_bytes());
        assert!(parse(&buffer, 0, 1).is_ok());
        buffer[0..4].copy_from_slice(&1u32.to_le_bytes());
        assert!(parse(&buffer, 0, 1).is_err());
    }
}
