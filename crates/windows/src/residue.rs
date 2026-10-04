use crate::fs::*;
use anyhow::{Result, ensure};
use dustaway_core::{ReasonCode, Refusal, residue::ResidueRule};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn report_marker(path: &Path, cancel: Option<&AtomicBool>) -> Result<bool> {
    let marker = path.join("Report.wer");
    if !marker.try_exists()? {
        return Ok(false);
    }
    let pin = PinnedPath::open_cancellable(&marker, false, false, cancel)?;
    ensure!(
        pin.stamp.size <= 1024 * 1024,
        Refusal(ReasonCode::UnsupportedLayout)
    );
    let bytes = std::fs::read(marker)?;
    let content = if bytes.starts_with(&[0xff, 0xfe]) {
        String::from_utf16_lossy(
            &bytes[2..]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Ok(content.lines().any(|s| s.trim() == "Version=1")
        && content
            .lines()
            .any(|s| s.starts_with("EventType=") && s.len() > 10))
}

fn cache_layout(path: &Path, cancel: Option<&AtomicBool>) -> Result<bool> {
    let mut index = HashSet::new();
    let mut values = HashSet::new();
    for entry in std::fs::read_dir(path)? {
        check_cancel(cancel)?;
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "idx" => {
                index.insert(stem);
            }
            "val" => {
                values.insert(stem);
            }
            _ => {}
        }
    }
    Ok(index.intersection(&values).next().is_some())
}

pub(crate) fn roots(
    local: &Path,
    spec: &ResidueRule,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    let base = local.join(spec.relative);
    if spec.id == "windows.crash_dumps" || !base.try_exists()? {
        return Ok(vec![base]);
    }
    let _pin = PinnedPath::open_cancellable(&base, true, false, cancel)?;
    let mut roots = vec![];
    // Only immediate report/cache directories; never walk arbitrary descendants.
    for entry in std::fs::read_dir(&base)? {
        check_cancel(cancel)?;
        let inspect = (|| -> Result<Option<PathBuf>> {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                return Ok(None);
            }
            let path = entry.path();
            let _pin = PinnedPath::open_cancellable(&path, true, false, cancel)?;
            let valid = if spec.id == "windows.wer_archive" {
                report_marker(&path, cancel)?
            } else {
                cache_layout(&path, cancel)?
            };
            Ok(valid.then_some(path))
        })();
        match inspect {
            Ok(Some(path)) => roots.push(path),
            Ok(None) => {}
            Err(e) if e.is::<ScanCancelled>() => return Err(e),
            Err(e) => issue(error_reason(&e)),
        }
    }
    roots.sort();
    Ok(roots)
}

pub(crate) fn accepts_file(id: &str, root: &Path, path: &Path) -> bool {
    if dustaway_core::residue::rule(id).is_none() {
        return true;
    }
    if path.parent() != Some(root) || !dustaway_core::residue::accepts_file(id, path) {
        return false;
    }
    if id == "windows.crash_dumps" {
        return minidump_header(path).unwrap_or(false);
    }
    if id == "windows.directx_cache" {
        let other = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("idx"))
        {
            "val"
        } else {
            "idx"
        };
        return path.with_extension(other).is_file();
    }
    true
}

// Check only the bounded format directory, never interpret or upload dump contents.
fn minidump_header(path: &Path) -> Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let pin = PinnedPath::open(path, false, false)?;
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 32];
    if file.read_exact(&mut header).is_err() || &header[..4] != b"MDMP" {
        return Ok(false);
    }
    let word = |start| u32::from_le_bytes(header[start..start + 4].try_into().unwrap());
    let count = u64::from(word(8));
    let directory = u64::from(word(12));
    if word(4) & 0xffff != 0xa793
        || !(1..=4096).contains(&count)
        || directory < 32
        || directory + count * 12 > pin.stamp.size
    {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(directory))?;
    for _ in 0..count {
        let mut stream = [0; 12];
        file.read_exact(&mut stream)?;
        let size = u64::from(u32::from_le_bytes(stream[4..8].try_into().unwrap()));
        let offset = u64::from(u32::from_le_bytes(stream[8..12].try_into().unwrap()));
        if size > 0 && (offset < 32 || offset + size > pin.stamp.size) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dump_extension_and_magic_alone_never_authorize_personal_content() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("sample.dmp");
        for data in [b"personal document".to_vec(), b"MDMP".to_vec(), vec![0; 32]] {
            std::fs::write(&file, data).unwrap();
            assert!(!minidump_header(&file).unwrap());
        }
        let mut data = vec![0; 48];
        data[..4].copy_from_slice(b"MDMP");
        data[4..8].copy_from_slice(&0xa793u32.to_le_bytes());
        data[8..12].copy_from_slice(&1u32.to_le_bytes());
        data[12..16].copy_from_slice(&32u32.to_le_bytes());
        data[36..40].copy_from_slice(&4u32.to_le_bytes());
        data[40..44].copy_from_slice(&44u32.to_le_bytes());
        std::fs::write(&file, &data).unwrap();
        assert!(minidump_header(&file).unwrap());
        data[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&file, data).unwrap();
        assert!(!minidump_header(&file).unwrap());
    }
}
