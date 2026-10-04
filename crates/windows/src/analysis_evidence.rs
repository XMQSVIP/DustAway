//! Versioned analysis evidence. It cannot authorize deletion without reopening the file.
use super::*;
use crate::analysis_listing::{self, Metadata};
const MAGIC: &[u8; 4] = b"DA02";
const HEADER: usize = 76;

pub(super) fn from_pin(pin: &ReadPin, name: std::ffi::OsString) -> Result<Metadata> {
    let (volume, id) = analysis_listing::identity(&pin._handle)?;
    let s = &pin.stamp;
    Ok(Metadata {
        name,
        volume,
        id,
        attributes: s.attributes,
        size: s.size,
        allocated: analysis_listing::allocated(&pin._handle, s.attributes).ok(),
        created: s.created,
        modified: s.modified,
        changed: s.changed,
        links: Some(s.links),
    })
}
pub(super) fn encode(m: &Metadata) -> Vec<u8> {
    let mut b = Vec::with_capacity(HEADER + m.name.len() * 2);
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(&m.volume.to_le_bytes());
    b.extend_from_slice(&m.id);
    b.extend_from_slice(&m.attributes.to_le_bytes());
    for v in [m.size, m.created, m.modified, m.changed as u64] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&m.links.unwrap_or(0).to_le_bytes());
    b.extend_from_slice(&m.allocated.unwrap_or(u64::MAX).to_le_bytes());
    for c in m.name.encode_wide() {
        b.extend_from_slice(&c.to_le_bytes());
    }
    b
}
pub(super) fn decode(b: &[u8]) -> Result<Metadata> {
    use std::os::windows::ffi::OsStringExt;
    ensure!(
        b.len() >= HEADER && b.len().is_multiple_of(2) && &b[..4] == MAGIC,
        Refusal(ReasonCode::Unverifiable)
    );
    let n = |at| u64::from_le_bytes(b[at..at + 8].try_into().unwrap());
    let links = u32::from_le_bytes(b[64..68].try_into()?);
    Ok(Metadata {
        name: std::ffi::OsString::from_wide(
            &b[HEADER..]
                .chunks_exact(2)
                .map(|v| u16::from_le_bytes([v[0], v[1]]))
                .collect::<Vec<_>>(),
        ),
        volume: n(4),
        id: b[12..28].try_into()?,
        attributes: u32::from_le_bytes(b[28..32].try_into()?),
        size: n(32),
        created: n(40),
        modified: n(48),
        changed: n(56) as i64,
        links: (links != 0).then_some(links),
        allocated: (n(68) != u64::MAX).then_some(n(68)),
    })
}
pub(super) fn matches_pin(m: &Metadata, pin: &ReadPin, full: bool) -> Result<()> {
    let (volume, id) = analysis_listing::identity(&pin._handle)?;
    ensure!(
        volume == m.volume && id == m.id,
        Refusal(ReasonCode::Changed)
    );
    if full {
        let s = &pin.stamp;
        let directory = m.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
        ensure!(
            s.attributes == m.attributes
                && s.created == m.created
                && (directory
                    || (s.size == m.size
                        && s.modified == m.modified
                        && s.changed == m.changed
                        && m.links.is_none_or(|n| n == s.links))),
            Refusal(ReasonCode::Changed)
        );
    }
    Ok(())
}
pub(super) fn record_metadata(
    m: &Metadata,
    path: &Path,
    id: String,
    parent: Option<String>,
    hints: &[(PathBuf, String)],
) -> AnalysisRecord {
    let directory = m.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
    AnalysisRecord {
        entry: AnalysisEntry {
            manual: Default::default(),
            ancestors: vec![],
            id,
            parent,
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| display_path(path)),
            path: display_path(path),
            directory,
            bytes: if directory {
                "0".into()
            } else {
                m.size.to_string()
            },
            allocated_bytes: (!directory)
                .then_some(m.allocated)
                .flatten()
                .map(|n| n.to_string()),
            allocation_unknown: u64::from(!directory && m.allocated.is_none()),
            verification_pending: !directory && m.links.is_none(),
            files: u64::from(!directory),
            modified_unix: ((m.modified / 10_000_000) as i64).saturating_sub(11_644_473_600),
            shared: !directory && m.links.is_some_and(|n| n > 1),
            incomplete: false,
            pending: false,
            issue: None,
            rule_ids: hints
                .iter()
                .filter(|(p, _)| path.starts_with(p))
                .map(|(_, id)| id.clone())
                .collect(),
            excluded: false,
            advice: None,
        },
        file_key: (!directory)
            .then(|| format!("{:016x}:{:032x}", m.volume, u128::from_le_bytes(m.id))),
        opaque: encode(m),
    }
}
pub(super) fn record(
    pin: &ReadPin,
    id: String,
    parent: Option<String>,
    hints: &[(PathBuf, String)],
) -> Result<AnalysisRecord> {
    let name = if parent.is_none() {
        Default::default()
    } else {
        pin.path
            .file_name()
            .ok_or(Refusal(ReasonCode::Unverifiable))?
            .to_owned()
    };
    Ok(record_metadata(
        &from_pin(pin, name)?,
        &pin.path,
        id,
        parent,
        hints,
    ))
}
pub(super) fn verified_node(root: &AnalysisRoot, records: &[Vec<u8>]) -> Result<ReadPin> {
    ensure!(
        !records.is_empty() && records.len() <= ANALYSIS_DEPTH + 1,
        Refusal(ReasonCode::Unverifiable)
    );
    let original: Snapshot = serde_json::from_str(&root.opaque)?;
    let mut path = from_native(&original.path);
    is_fixed_ntfs(&path)?;
    let mut current = ReadPin::chain(&path, true)?;
    ensure!(
        current.parents == original.parents && current.stamp.identity == original.stamp.identity,
        Refusal(ReasonCode::Changed)
    );
    for (index, raw) in records.iter().enumerate() {
        let m = decode(raw)?;
        if index == 0 {
            ensure!(m.name.is_empty(), Refusal(ReasonCode::Unverifiable));
        } else {
            let name: Vec<u16> = m.name.encode_wide().collect();
            ensure!(
                !name.is_empty()
                    && name != [46]
                    && name != [46, 46]
                    && !name.iter().any(|c| [0, 47, 58, 92].contains(c)),
                Refusal(ReasonCode::Unverifiable)
            );
            path.push(&m.name);
            let mut next = ReadPin::single(&path, m.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0)?;
            ensure!(
                next.resolved.parent() == Some(current.resolved.as_path()),
                Refusal(ReasonCode::Changed)
            );
            next.parents = current.parents;
            next.parents.push(current.stamp.identity);
            next._parents = current._parents;
            next._parents.push(current._handle);
            current = next;
        }
        if index + 1 < records.len() {
            ensure!(
                m.attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0,
                Refusal(ReasonCode::Changed)
            );
        }
        matches_pin(&m, &current, true)?;
    }
    Ok(current)
}
