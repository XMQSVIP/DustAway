//! Weixin 4.x attachments: recognized account layout, independent release gate.
#[cfg(test)]
#[path = "attachments_acceptance.rs"]
mod acceptance;
use crate::{
    applications,
    fs::*,
    platform::{known_folder, reveal_pinned_path},
    security,
};
use anyhow::{Context, Result, ensure};
use dustaway_core::*;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use windows::Win32::UI::Shell::FOLDERID_Documents;

pub struct WindowsAttachments;
#[path = "qq_attachments.rs"]
mod qq;
const LAYOUT: &str = "weixin4-msg-v1";
fn layout_supported(layout: &str) -> bool {
    layout == LAYOUT
}
// Evidence validates a layout/format contract, not a version allowlist.
// Installation/version changes still invalidate the captured Account identity.
fn format_released(layout: &str, root: &Path, path: &Path) -> bool {
    if !layout_supported(layout) {
        return false;
    }
    let Ok(relative) = path.strip_prefix(root.join("msg")) else {
        return false;
    };
    let parts: Vec<_> = relative.iter().filter_map(|p| p.to_str()).collect();
    if parts.len() != 3 {
        return false;
    }
    let month = parts[1].as_bytes();
    if month.len() != 7
        || month[4] != b'-'
        || !month[..4].iter().all(u8::is_ascii_digit)
        || !matches!(
            &parts[1][5..],
            "01" | "02" | "03" | "04" | "05" | "06" | "07" | "08" | "09" | "10" | "11" | "12"
        )
    {
        return false;
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        (parts[0], ext.as_str()),
        ("video", "mp4" | "jpg")
            | (
                "file",
                "zip"
                    | "pdf"
                    | "txt"
                    | "docx"
                    | "xlsx"
                    | "pptx"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "bmp"
                    | "webp"
                    | "mp4"
                    | "mov"
                    | "m4v"
                    | "mp3"
                    | "wav"
                    | "ogg"
            )
    )
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Account {
    path: Vec<u16>,
    identity: Identity,
    parents: Vec<Identity>,
    database: Identity,
    messages: Identity,
    message_db: Identity,
    owner_sid: String,
    application: String,
    version: String,
    layout: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AttachmentStamp {
    #[serde(flatten)]
    file: Snapshot,
    account: Account,
}
fn app() -> &'static dustaway_core::applications::Application {
    dustaway_core::applications::application("weixin").unwrap()
}
fn message_identity(path: &Path) -> Result<Identity> {
    use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
    // Metadata only: allow the application's database writer during browsing.
    // Parent directories remain pinned; no database contents are read.
    let name = wide(path);
    let handle = Handle(unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )?
    });
    let info = stamp(&handle)?;
    ensure!(
        info.size > 0
            && info.links == 1
            && info.attributes
                & (FILE_ATTRIBUTE_REPARSE_POINT.0
                    | FILE_ATTRIBUTE_DIRECTORY.0
                    | FILE_ATTRIBUTE_OFFLINE.0
                    | 0x40000
                    | 0x400000)
                == 0,
        "聊天消息资料布局未通过核验"
    );
    Ok(info.identity)
}
fn account(path: &Path) -> Result<(Account, applications::VerifiedApplication)> {
    is_fixed_ntfs(path)?;
    let path = extended(path)?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .context("无法读取账号目录名称")?;
    ensure!(
        name.starts_with("wxid_")
            && name.len() > 6
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "请选择 xwechat_files 下的微信账号目录"
    );
    ensure!(
        path.parent()
            .and_then(Path::file_name)
            .is_some_and(|p| p.eq_ignore_ascii_case("xwechat_files")),
        "不是受支持的微信资料结构"
    );
    let found = applications::identify(app())?;
    let root = PinnedPath::observe(&path, true, None)?;
    let database = PinnedPath::observe(&path.join("db_storage"), true, None)?;
    let messages = PinnedPath::observe(&path.join("msg"), true, None)?;
    let _message_dir = PinnedPath::observe(&path.join("db_storage/message"), true, None)?;
    let message_db = message_identity(&path.join("db_storage/message/message_0.db"))?;
    let owner_sid = security::path_owner_sid(&path)?;
    ensure!(
        owner_sid == security::current_sid()?,
        "仅可整理归属于当前 Windows 用户的微信资料"
    );
    let evidence = Account {
        path: native(&path),
        identity: root.stamp.identity.clone(),
        parents: root.parent_ids.clone(),
        database: database.stamp.identity.clone(),
        messages: messages.stamp.identity.clone(),
        message_db,
        owner_sid,
        application: found.fingerprint()?,
        version: found.version.clone(),
        layout: LAYOUT.into(),
    };
    Ok((evidence, found))
}
fn verify(item: &ManualRecord, exclusions: &[String], deleting: bool) -> Result<PinnedPath> {
    ensure!(!item.directory, Refusal(ReasonCode::Protected));
    let old: AttachmentStamp = serde_json::from_str(&item.file.opaque)?;
    let root = from_native(&old.account.path);
    let (fresh, found) = account(&root)?;
    ensure!(fresh == old.account, Refusal(ReasonCode::Changed));
    ensure!(
        layout_supported(&fresh.layout),
        Refusal(ReasonCode::ValidationPending)
    );
    ensure!(
        format_released(&fresh.layout, &root, &from_native(&old.file.path)),
        Refusal(ReasonCode::ValidationPending)
    );
    ensure!(
        security::application_processes(app().processes, &found.install)?.is_empty(),
        Refusal(ReasonCode::ApplicationRunning)
    );
    file_pin(&old, exclusions, deleting)
}
fn file_pin(old: &AttachmentStamp, exclusions: &[String], deleting: bool) -> Result<PinnedPath> {
    let path = from_native(&old.file.path);
    let root = from_native(&old.account.path);
    ensure!(
        old.file.root == old.account.path && path.starts_with(root.join("msg")),
        Refusal(ReasonCode::Protected)
    );
    ensure!(!excluded(&path, exclusions), Refusal(ReasonCode::Excluded));
    let pin = PinnedPath::open_attachment(&path, deleting)?;
    ensure!(
        pin.stamp == old.file.stamp && pin.parent_ids == old.file.parents,
        Refusal(ReasonCode::Changed)
    );
    ensure!(
        classify(&root, &path)? != AttachmentKind::Unknown,
        Refusal(ReasonCode::Protected)
    );
    Ok(pin)
}

#[cfg(test)]
#[path = "attachments_boundary_tests.rs"]
mod boundary_tests;
fn read_pin(item: &ManualRecord) -> Result<PinnedPath> {
    let old: AttachmentStamp = serde_json::from_str(&item.file.opaque)?;
    let pin = PinnedPath::observe(&from_native(&old.file.path), false, None)?;
    ensure!(
        pin.stamp == old.file.stamp && pin.parent_ids == old.file.parents,
        Refusal(ReasonCode::Changed)
    );
    Ok(pin)
}
fn classify(root: &Path, path: &Path) -> Result<AttachmentKind> {
    let relative = path
        .strip_prefix(root.join("msg"))
        .context("附件不在消息目录")?;
    let first = relative
        .components()
        .next()
        .and_then(|p| p.as_os_str().to_str())
        .unwrap_or("");
    if !matches!(first, "video" | "attach" | "file" | "audio") {
        return Ok(AttachmentKind::Unknown);
    }
    classify_file(path, first == "file")
}
fn classify_file(path: &Path, is_file: bool) -> Result<AttachmentKind> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "webp"
            | "mp4"
            | "mov"
            | "m4v"
            | "mp3"
            | "wav"
            | "ogg"
            | "pdf"
            | "txt"
            | "zip"
            | "docx"
            | "xlsx"
            | "pptx"
    ) {
        return Ok(AttachmentKind::Unknown);
    }
    let mut header = [0u8; 64];
    let n = std::fs::File::open(path)?.read(&mut header)?;
    let h = &header[..n];
    let kind = match ext.as_str() {
        "png" if h.starts_with(b"\x89PNG\r\n\x1a\n") => AttachmentKind::Image,
        "jpg" | "jpeg" if h.starts_with(&[0xff, 0xd8, 0xff]) => AttachmentKind::Image,
        "gif" if h.starts_with(b"GIF8") => AttachmentKind::Image,
        "bmp" if h.starts_with(b"BM") => AttachmentKind::Image,
        "webp" if h.starts_with(b"RIFF") && h.get(8..12) == Some(b"WEBP") => AttachmentKind::Image,
        "mp4" | "mov" | "m4v" if h.get(4..8) == Some(b"ftyp") => AttachmentKind::Video,
        "wav" if h.starts_with(b"RIFF") && h.get(8..12) == Some(b"WAVE") => AttachmentKind::Audio,
        "mp3"
            if h.starts_with(b"ID3")
                || h.first() == Some(&0xff) && h.get(1).is_some_and(|b| b & 0xe0 == 0xe0) =>
        {
            AttachmentKind::Audio
        }
        "ogg" if h.starts_with(b"OggS") => AttachmentKind::Audio,
        "pdf" if is_file && h.starts_with(b"%PDF-") => AttachmentKind::File,
        "zip" | "docx" | "xlsx" | "pptx" if is_file && h.starts_with(b"PK\x03\x04") => {
            AttachmentKind::File
        }
        "txt" if is_file && !h.contains(&0) && std::str::from_utf8(h).is_ok() => {
            AttachmentKind::File
        }
        _ => AttachmentKind::Unknown,
    };
    Ok(kind)
}
impl AnalysisPlatform for WindowsAttachments {
    fn locations(&self) -> Vec<(String, String)> {
        let Ok(root) = known_folder(&FOLDERID_Documents) else {
            return vec![];
        };
        let Ok(entries) = std::fs::read_dir(root.join("xwechat_files")) else {
            return vec![];
        };
        entries
            .take(64)
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("wxid_"))
            .map(|e| ("微信账号资料".into(), display_path(&e.path())))
            .collect()
    }
    fn validate_root(&self, _: &str) -> Result<AnalysisRoot> {
        anyhow::bail!("请使用附件专用扫描")
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        _: &AtomicBool,
        _: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        anyhow::bail!("请使用附件专用扫描")
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        anyhow::bail!("请使用附件专用定位")
    }
    fn delete_manual(&self, record: &ManualRecord, exclusions: &[String]) -> Result<()> {
        if qq::is_record(record) {
            return qq::delete(record, exclusions);
        }
        verify(record, exclusions, true)?.mark_attachment_delete()
    }
}
impl AttachmentPlatform for WindowsAttachments {
    fn candidate_locations(&self, provider: AttachmentProvider) -> Result<Vec<String>> {
        if provider != AttachmentProvider::Weixin {
            return qq::locations(provider);
        }
        applications::attachment_installation(app())?;
        let root = known_folder(&FOLDERID_Documents)?.join("xwechat_files");
        qq::account_directories(&root, |name| name.starts_with("wxid_"))
    }
    fn account(&self, provider: AttachmentProvider, path: &str) -> Result<AttachmentRoot> {
        if provider != AttachmentProvider::Weixin {
            return qq::register(provider, path);
        }
        let (evidence, _) = account(Path::new(path))?;
        Ok(AttachmentRoot {
            location: AttachmentLocation {
                provider,
                token: String::new(),
                path: display_path(&from_native(&evidence.path)),
                title: Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                version: evidence.version.clone(),
                cleanup_block: (!layout_supported(&evidence.layout))
                    .then(|| "此微信资料布局尚未通过兼容性核验，仅可浏览".into()),
            },
            opaque: serde_json::to_string(&evidence)?,
        })
    }
    fn scan_attachments(
        &self,
        root: &AttachmentRoot,
        exclusions: &[String],
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(Result<AttachmentRecord>) -> Result<()>,
    ) -> Result<()> {
        if root.location.provider != AttachmentProvider::Weixin {
            return qq::scan(root, exclusions, cancel, emit);
        }
        let evidence: Account = serde_json::from_str(&root.opaque)?;
        let root_path = from_native(&evidence.path);
        let fresh = account(&root_path)?.0;
        ensure!(fresh == evidence, Refusal(ReasonCode::Changed));
        let mut stack: Vec<PathBuf> = ["video", "attach", "file", "audio"]
            .iter()
            .map(|p| root_path.join("msg").join(p))
            .filter(|p| p.exists())
            .collect();
        let mut visited = 0usize;
        while let Some(dir) = stack.pop() {
            visited += 1;
            ensure!(
                visited <= 50_000 && stack.len() <= 50_000,
                "已达到本次附件目录扫描上限"
            );
            ensure!(!cancel.load(Ordering::Acquire), "扫描已停止");
            if excluded(&dir, exclusions) {
                continue;
            }
            let directory = match PinnedPath::observe(&dir, true, Some(cancel)) {
                Ok(p) => p,
                Err(e) => {
                    emit(Err(e))?;
                    continue;
                }
            };
            let entries = match std::fs::read_dir(&directory.path) {
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
                    Ok(ty) => ty,
                    Err(e) => {
                        emit(Err(e.into()))?;
                        continue;
                    }
                };
                if ty.is_dir() {
                    stack.push(path);
                    continue;
                }
                let result = (|| -> Result<AttachmentRecord> {
                    let pin = PinnedPath::observe(&path, false, Some(cancel))?;
                    let kind = classify(&root_path, &path)?;
                    let block = if kind == AttachmentKind::Unknown {
                        Some("加密或未知格式，已保留".into())
                    } else if pin.stamp.links != 1 {
                        Some("共享硬链接文件，已保留".into())
                    } else if !format_released(&evidence.layout, &root_path, &path) {
                        Some("此格式或目录布局尚未通过删除验收，仅可浏览".into())
                    } else {
                        root.location.cleanup_block.clone()
                    };
                    let identity =
                        format!("{}:{}", pin.stamp.identity.volume, pin.stamp.identity.index);
                    let snapshot = AttachmentStamp {
                        file: Snapshot {
                            path: native(&path),
                            root: evidence.path.clone(),
                            parents: pin.parent_ids.clone(),
                            stamp: pin.stamp.clone(),
                            application_identity: Some(evidence.application.clone()),
                        },
                        account: evidence.clone(),
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
                            identity,
                        },
                    })
                })();
                emit(result)?;
            }
        }
        Ok(())
    }
    fn verify_attachment(
        &self,
        root: &AttachmentRoot,
        item: &ManualRecord,
        exclusions: &[String],
    ) -> Result<()> {
        if root.location.provider != AttachmentProvider::Weixin {
            return qq::verify(root, item, exclusions);
        }
        let stamp: AttachmentStamp = serde_json::from_str(&item.file.opaque)?;
        ensure!(
            serde_json::to_string(&stamp.account)? == root.opaque,
            Refusal(ReasonCode::Changed)
        );
        verify(item, exclusions, false).map(|_| ())
    }
    fn attachment_thumbnail(&self, item: &ManualRecord) -> Result<String> {
        if qq::is_record(item) {
            return qq::preview(item);
        }
        let pin = read_pin(item)?;
        let snapshot: AttachmentStamp = serde_json::from_str(&item.file.opaque)?;
        ensure!(
            matches!(
                classify(&from_native(&snapshot.account.path), &pin.path)?,
                AttachmentKind::Image | AttachmentKind::Video
            ),
            "此格式暂不支持预览"
        );
        thumbnail(&pin.path).or_else(|error| {
            // Only a same-stem, pinned JPEG within the recognized video directory.
            if classify(&from_native(&snapshot.account.path), &pin.path)? != AttachmentKind::Video {
                return Err(error);
            }
            let cover = PinnedPath::observe(&pin.path.with_extension("jpg"), false, None)?;
            ensure!(
                classify(&from_native(&snapshot.account.path), &cover.path)?
                    == AttachmentKind::Image,
                "视频封面格式不支持"
            );
            thumbnail(&cover.path)
        })
    }
    fn reveal_attachment(&self, item: &ManualRecord) -> Result<()> {
        if qq::is_record(item) {
            return qq::reveal(item);
        }
        let pin = read_pin(item)?;
        reveal_pinned_path(&pin.path)
    }
}

fn thumbnail(path: &Path) -> Result<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use windows::{
        Win32::{Foundation::SIZE, Graphics::Gdi::*, System::Com::*, UI::Shell::*},
        core::PCWSTR,
    };
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    struct Bitmap(HBITMAP);
    impl Drop for Bitmap {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let _com = Com;
        let shell_path = display_path(path);
        let name = wide(Path::new(&shell_path));
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(name.as_ptr()), None)
                .context("无法取得 Windows 文件预览接口")?;
        let bitmap = Bitmap(
            factory
                .GetImage(SIZE { cx: 512, cy: 512 }, SIIGBF_THUMBNAILONLY)
                .context("Windows 未能生成此文件的缩略图")?,
        );
        let mut info = BITMAP::default();
        ensure!(
            GetObjectW(
                bitmap.0.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some((&mut info as *mut BITMAP).cast())
            ) != 0,
            "缩略图不可用"
        );
        let (w, h) = (info.bmWidth, info.bmHeight.abs());
        ensure!(w > 0 && h > 0 && w <= 512 && h <= 512, "缩略图大小不支持");
        let dc = CreateCompatibleDC(None);
        ensure!(!dc.is_invalid(), "无法创建预览画布");
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        let copied = GetDIBits(
            dc,
            bitmap.0,
            0,
            h as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        ensure!(copied == h, "无法读取缩略图");
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let mut output = vec![];
        {
            let mut png = png::Encoder::new(&mut output, w as u32, h as u32);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?.write_image_data(&pixels)?;
        }
        Ok(format!("data:image/png;base64,{}", STANDARD.encode(output)))
    }
}
