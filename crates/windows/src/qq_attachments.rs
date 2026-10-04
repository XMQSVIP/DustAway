//! QQ families have independent identity/layout checks and release evidence.
//! Compatibility follows signed components and validated layouts, not a version allowlist.
use super::*;
use dustaway_core::applications::Application;

const QQNT: Application = Application {
    id: "qqnt",
    title: "QQNT",
    version: "",
    executable: "QQ.exe",
    processes: &[
        "QQ.exe",
        "QQApp.exe",
        "QQExternal.exe",
        "QQProtect.exe",
        "TXPlatform.exe",
        "QQCrashReport.exe",
        "QQNT.exe",
        "QQUpdate.exe",
    ],
    publisher: "Tencent Technology (Shenzhen) Company Limited",
    registry_keys: &["Software\\Tencent\\QQNT", "Software\\Tencent\\QQ"],
    executable_relative: "QQ.exe",
};
fn application(provider: AttachmentProvider) -> Result<&'static Application> {
    match provider {
        AttachmentProvider::QqClassic => {
            Ok(dustaway_core::applications::application("qq").unwrap())
        }
        AttachmentProvider::Qqnt => Ok(&QQNT),
        _ => anyhow::bail!("不是 QQ 附件来源"),
    }
}
fn process_names(provider: AttachmentProvider) -> Result<Vec<&'static str>> {
    let mut names = application(provider)?.processes.to_vec();
    // Classic QQ starts QQGuild outside its installation directory. Its account
    // data is protected, but it must also be exited before attachment deletion.
    names.push("QQGuild.exe");
    Ok(names)
}
fn ensure_exited(provider: AttachmentProvider, install: &Path) -> Result<()> {
    ensure!(
        running_processes(provider, install)?.is_empty(),
        Refusal(ReasonCode::ApplicationRunning)
    );
    Ok(())
}
fn running_processes(provider: AttachmentProvider, install: &Path) -> Result<Vec<String>> {
    let shared: &[&str] = if provider == AttachmentProvider::Qqnt {
        &["crashpad_handler.exe", "crashpad_handler_extension.exe"]
    } else {
        &[]
    };
    security::application_processes_with_shared(&process_names(provider)?, shared, install)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct QqAccount {
    provider: AttachmentProvider,
    path: Vec<u16>,
    identity: Identity,
    parents: Vec<Identity>,
    markers: Vec<(String, Identity)>,
    owner_sid: String,
    application: String,
    version: String,
    layout: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct QqStamp {
    provider: AttachmentProvider,
    // Keep the common snapshot at the top level for volume accounting, as with
    // Weixin. The trusted QQ account identity remains separately bound below.
    #[serde(flatten)]
    file: Snapshot,
    account: QqAccount,
}
pub(super) fn is_record(item: &ManualRecord) -> bool {
    serde_json::from_str::<serde_json::Value>(&item.file.opaque)
        .ok()
        .is_some_and(|v| v.get("provider").is_some())
}
fn account_name(name: &str) -> bool {
    (5..=12).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_digit())
}
pub(super) fn account_directories(
    root: &Path,
    accept: impl Fn(&str) -> bool,
) -> Result<Vec<String>> {
    if !root.try_exists()? {
        return Ok(vec![]);
    }
    let pin = PinnedPath::observe(root, true, None)?;
    let mut paths = vec![];
    for entry in std::fs::read_dir(&pin.path)? {
        let entry = entry?;
        if accept(&entry.file_name().to_string_lossy()) {
            ensure!(paths.len() < 64, "账号位置过多，请使用选择其他位置");
            paths.push(display_path(&entry.path()));
        }
    }
    Ok(paths)
}
pub(super) fn locations(provider: AttachmentProvider) -> Result<Vec<String>> {
    applications::attachment_installation(application(provider)?)?;
    let documents = known_folder(&FOLDERID_Documents)?;
    let mut paths = account_directories(&documents.join("Tencent Files"), account_name)?;
    paths.extend(account_directories(
        &documents.join("TencentFiles"),
        account_name,
    )?);
    // Numeric directories from the other QQ family are not accounts of this
    // provider. Partial matching layouts still reach inspect() and show errors.
    paths
        .into_iter()
        .filter_map(|path| match family_hint(provider, Path::new(&path)) {
            Ok(true) => Some(Ok(path)),
            Ok(false) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}
fn family_hint(provider: AttachmentProvider, path: &Path) -> Result<bool> {
    Ok(match provider {
        AttachmentProvider::QqClassic => {
            path.join("Registry2.0.db").try_exists()? || path.join("FileInfo.db").try_exists()?
        }
        AttachmentProvider::Qqnt => path.join("nt_qq").try_exists()?,
        _ => false,
    })
}
fn layout(
    provider: AttachmentProvider,
) -> Result<(
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
)> {
    match provider {
        AttachmentProvider::QqClassic => Ok((
            "qq-classic-account-v1",
            &["Registry2.0.db", "FileInfo.db"],
            &["Image", "Video", "Audio", "FileRecv"],
        )),
        AttachmentProvider::Qqnt => Ok((
            "qqnt-account-v1",
            &[
                "nt_qq/nt_db",
                "nt_qq/nt_db/nt_msg.db",
                "nt_qq/nt_db/profile_info.db",
            ],
            &[
                "nt_qq/nt_data/Pic",
                "nt_qq/nt_data/Video",
                "nt_qq/nt_data/Audio",
                "nt_qq/nt_data/File",
            ],
        )),
        _ => anyhow::bail!("未知 QQ 系列"),
    }
}
fn inspect(
    provider: AttachmentProvider,
    path: &Path,
) -> Result<(QqAccount, applications::VerifiedApplication)> {
    is_fixed_ntfs(path)?;
    let path = extended(path)?;
    ensure!(
        path.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(account_name),
        "请选择 QQ 数字账号资料目录"
    );
    ensure!(
        path.parent()
            .and_then(Path::file_name)
            .is_some_and(|p| p.eq_ignore_ascii_case("Tencent Files")
                || p.eq_ignore_ascii_case("TencentFiles")),
        "QQ 账号父目录布局未通过核验"
    );
    let found = applications::attachment_installation(application(provider)?)?;
    let pin = PinnedPath::observe(&path, true, None)?;
    let owner_sid = security::path_owner_sid(&path)?;
    ensure!(
        owner_sid == security::current_sid()?,
        "仅可整理归属于当前 Windows 用户的 QQ 资料"
    );
    let (layout, required, _) = layout(provider)?;
    let mut markers = vec![];
    let mut marker_directories = vec![];
    for name in required {
        let marker = path.join(name);
        let identity = if provider == AttachmentProvider::Qqnt && *name == "nt_qq/nt_db" {
            let directory = PinnedPath::observe(&marker, true, None)?;
            let identity = directory.stamp.identity.clone();
            marker_directories.push(directory);
            identity
        } else {
            super::message_identity(&marker)?
        };
        markers.push(((*name).into(), identity));
    }
    Ok((
        QqAccount {
            provider,
            path: native(&path),
            identity: pin.stamp.identity.clone(),
            parents: pin.parent_ids.clone(),
            markers,
            owner_sid,
            application: found.fingerprint()?,
            version: found.version.clone(),
            layout: layout.into(),
        },
        found,
    ))
}
// Real-app evidence validates the layout/format contract. Version is retained
// in QqAccount and the installation fingerprint to invalidate stale scans.
fn account_released(account: &QqAccount) -> bool {
    #[cfg(test)]
    if acceptance::permitted(account, None) {
        return true;
    }
    #[cfg(test)]
    if nt_acceptance::permitted(account, None) {
        return true;
    }
    matches!(
        (account.provider, account.layout.as_str()),
        (AttachmentProvider::QqClassic, "qq-classic-account-v1")
            | (AttachmentProvider::Qqnt, "qqnt-account-v1")
    )
}
fn released(account: &QqAccount, path: &Path) -> bool {
    #[cfg(test)]
    if acceptance::permitted(account, Some(path)) {
        return true;
    }
    #[cfg(test)]
    if nt_acceptance::permitted(account, Some(path)) {
        return true;
    }
    if !account_released(account) {
        return false;
    }
    // Evidence: docs/verification-qq-attachments.md. Receiving media as a file
    // does not validate other media directories or nested/shared index areas.
    let root = from_native(&account.path);
    let Ok(relative) = path.strip_prefix(&root) else {
        return false;
    };
    let parts: Vec<_> = relative.iter().filter_map(|p| p.to_str()).collect();
    let area_matches = match account.provider {
        AttachmentProvider::QqClassic => {
            parts.len() == 3
                && parts[0].eq_ignore_ascii_case("FileRecv")
                && parts[1].eq_ignore_ascii_case("MobileFile")
        }
        AttachmentProvider::Qqnt => {
            parts.len() == 4
                && parts[0].eq_ignore_ascii_case("nt_qq")
                && parts[1].eq_ignore_ascii_case("nt_data")
                && parts[2].eq_ignore_ascii_case("File")
        }
        _ => false,
    };
    area_matches
        && path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| {
                ["jpg", "mp4", "zip"]
                    .iter()
                    .any(|e| ext.eq_ignore_ascii_case(e))
            })
}
pub(super) fn register(provider: AttachmentProvider, path: &str) -> Result<AttachmentRoot> {
    let (account, _) = inspect(provider, Path::new(path))?;
    Ok(AttachmentRoot {
        location: AttachmentLocation {
            provider,
            token: String::new(),
            path: display_path(&from_native(&account.path)),
            title: format!(
                "{} · {}",
                provider.title(),
                Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
            version: account.version.clone(),
            cleanup_block: (!account_released(&account))
                .then(|| "此系列的资料布局尚未通过兼容性核验，仅可浏览".into()),
        },
        opaque: serde_json::to_string(&account)?,
    })
}
fn attachment_area(account: &QqAccount, path: &Path) -> Result<&'static str> {
    let root = from_native(&account.path);
    let relative = path.strip_prefix(&root).context("附件不在此账号内")?;
    ensure!(
        !relative
            .components()
            .any(|c| protected_directory(c.as_os_str())),
        "备份、迁移与账号资料已保留"
    );
    let (_, _, areas) = layout(account.provider)?;
    areas
        .iter()
        .copied()
        .find(|area| path.starts_with(root.join(area)) && path != root.join(area))
        .context("文件不在已识别的 QQ 附件目录")
}
fn protected_directory(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_string_lossy().to_ascii_lowercase().as_str(),
        "backup"
            | "backups"
            | "migration"
            | "migrate"
            | "mycollection"
            | "groupcollection"
            | "qqguild"
            | "nt_db"
            | "login"
            | "config"
    )
}
fn protected_name(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    name.ends_with(".db")
        || name.contains(".db-")
        || name.ends_with(".sqlite")
        || name.ends_with(".index")
        || name.ends_with(".idx")
        || name.ends_with(".ini")
        || name.ends_with(".json")
}
fn kind(account: &QqAccount, path: &Path) -> Result<AttachmentKind> {
    let area = attachment_area(account, path)?;
    if protected_name(path) {
        return Ok(AttachmentKind::Unknown);
    }
    let is_file = area.ends_with("FileRecv") || area.ends_with("/File");
    super::classify_file(path, is_file)
}
pub(super) fn scan(
    root: &AttachmentRoot,
    exclusions: &[String],
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(Result<AttachmentRecord>) -> Result<()>,
) -> Result<()> {
    let account: QqAccount = serde_json::from_str(&root.opaque)?;
    ensure!(
        account.provider == root.location.provider,
        Refusal(ReasonCode::Changed)
    );
    let root_path = from_native(&account.path);
    ensure!(
        inspect(account.provider, &root_path)?.0 == account,
        Refusal(ReasonCode::Changed)
    );
    let mut stack = Vec::new();
    for area in layout(account.provider)?.2 {
        let path = root_path.join(area);
        match path.try_exists() {
            Ok(true) => stack.push(path),
            Ok(false) => (),
            Err(e) => emit(Err(e.into()))?,
        }
    }
    let mut visited = 0;
    while let Some(dir) = stack.pop() {
        visited += 1;
        ensure!(
            visited <= ATTACHMENT_LIMIT && stack.len() <= ATTACHMENT_LIMIT,
            "已达到本次附件目录扫描上限"
        );
        ensure!(!cancel.load(Ordering::Acquire), "扫描已停止");
        if excluded(&dir, exclusions) {
            continue;
        }
        let pin = match PinnedPath::observe(&dir, true, Some(cancel)) {
            Ok(p) => p,
            Err(e) => {
                emit(Err(e))?;
                continue;
            }
        };
        let entries = match std::fs::read_dir(&pin.path) {
            Ok(e) => e,
            Err(e) => {
                emit(Err(e.into()))?;
                continue;
            }
        };
        for entry in entries {
            ensure!(!cancel.load(Ordering::Acquire), "扫描已停止");
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    emit(Err(e.into()))?;
                    continue;
                }
            };
            let path = entry.path();
            if excluded(&path, exclusions) {
                continue;
            }
            let ty = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    emit(Err(e.into()))?;
                    continue;
                }
            };
            if ty.is_dir() {
                if protected_directory(&entry.file_name()) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            // Database/index files are protected even when found inside media directories.
            if protected_name(&path) {
                continue;
            }
            emit((|| {
                let pin = PinnedPath::observe(&path, false, Some(cancel))?;
                let kind = kind(&account, &path)?;
                let block = if kind == AttachmentKind::Unknown {
                    Some("加密或未知格式，已保留".into())
                } else if pin.stamp.links != 1 {
                    Some("共享硬链接文件，已保留".into())
                } else if !released(&account, &path) {
                    Some("此格式或目录布局尚未通过删除验收，仅可浏览".into())
                } else {
                    root.location.cleanup_block.clone()
                };
                let snapshot = QqStamp {
                    provider: account.provider,
                    account: account.clone(),
                    file: Snapshot {
                        path: native(&path),
                        root: account.path.clone(),
                        parents: pin.parent_ids.clone(),
                        stamp: pin.stamp.clone(),
                        application_identity: Some(account.application.clone()),
                    },
                };
                Ok(AttachmentRecord {
                    entry: AttachmentEntry {
                        id: String::new(),
                        name: entry.file_name().to_string_lossy().into(),
                        path: display_path(&path),
                        kind,
                        bytes: pin.stamp.size.to_string(),
                        modified_unix: pin.stamp.modified_unix(),
                        preview_available: matches!(
                            kind,
                            AttachmentKind::Image | AttachmentKind::Video
                        ),
                        cleanup_block: block,
                        outcome: None,
                    },
                    record: ManualRecord {
                        file: FileRecord {
                            path_display: display_path(&path),
                            bytes: pin.stamp.size,
                            modified_unix: pin.stamp.modified_unix(),
                            opaque: serde_json::to_string(&snapshot)?,
                        },
                        directory: false,
                        identity: format!(
                            "{}:{}",
                            pin.stamp.identity.volume, pin.stamp.identity.index
                        ),
                    },
                })
            })())?;
        }
    }
    Ok(())
}
fn pin_record(
    item: &ManualRecord,
    exclusions: &[String],
    deleting: bool,
) -> Result<(QqStamp, PinnedPath)> {
    ensure!(!item.directory, Refusal(ReasonCode::Protected));
    let old: QqStamp = serde_json::from_str(&item.file.opaque)?;
    ensure!(
        old.provider == old.account.provider && old.file.root == old.account.path,
        Refusal(ReasonCode::Changed)
    );
    let path = from_native(&old.file.path);
    attachment_area(&old.account, &path)?;
    ensure!(
        !protected_name(&path) && !excluded(&path, exclusions),
        Refusal(ReasonCode::Protected)
    );
    let pin = if deleting {
        PinnedPath::open_attachment(&path, true)?
    } else {
        PinnedPath::observe(&path, false, None)?
    };
    ensure!(
        pin.stamp == old.file.stamp && pin.parent_ids == old.file.parents,
        Refusal(ReasonCode::Changed)
    );
    Ok((old, pin))
}
fn check_delete(item: &ManualRecord, exclusions: &[String], deleting: bool) -> Result<PinnedPath> {
    let (old, pin) = pin_record(item, exclusions, deleting)?;
    let (fresh, found) = inspect(old.provider, &from_native(&old.account.path))?;
    ensure!(fresh == old.account, Refusal(ReasonCode::Changed));
    ensure_exited(fresh.provider, &found.install)?;
    ensure!(
        released(&fresh, &pin.path),
        Refusal(ReasonCode::ValidationPending)
    );
    ensure!(
        pin.stamp.links == 1 && kind(&fresh, &pin.path)? != AttachmentKind::Unknown,
        Refusal(ReasonCode::Protected)
    );
    Ok(pin)
}
pub(super) fn verify(
    root: &AttachmentRoot,
    item: &ManualRecord,
    exclusions: &[String],
) -> Result<()> {
    let stamp: QqStamp = serde_json::from_str(&item.file.opaque)?;
    ensure!(
        stamp.provider == root.location.provider
            && serde_json::to_string(&stamp.account)? == root.opaque,
        Refusal(ReasonCode::Changed)
    );
    check_delete(item, exclusions, false).map(|_| ())
}
pub(super) fn delete(item: &ManualRecord, exclusions: &[String]) -> Result<()> {
    check_delete(item, exclusions, true)?.mark_attachment_delete()
}
pub(super) fn preview(item: &ManualRecord) -> Result<String> {
    let (stamp, pin) = pin_record(item, &[], false)?;
    ensure!(
        matches!(
            kind(&stamp.account, &pin.path)?,
            AttachmentKind::Image | AttachmentKind::Video
        ),
        "此格式暂不支持预览"
    );
    super::thumbnail(&pin.path)
}
pub(super) fn reveal(item: &ManualRecord) -> Result<()> {
    let (_, pin) = pin_record(item, &[], false)?;
    reveal_pinned_path(&pin.path)
}

#[cfg(test)]
#[path = "qq_attachments_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "qq_attachments_acceptance.rs"]
mod acceptance;

#[cfg(test)]
#[path = "qqnt_attachments_acceptance.rs"]
mod nt_acceptance;
