//! Application-bound discovery with independently documented, format-checked deletion scopes.
use crate::{fs::*, platform::known_folder, security};
use anyhow::{Context, Result, ensure};
use dustaway_core::{ReasonCode, Refusal, app_cache::CacheSpec, now};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use windows::Win32::UI::Shell::FOLDERID_LocalAppData;

#[path = "documented_cache.rs"]
mod documented;

#[cfg(test)]
#[path = "app_cache_tests.rs"]
pub(crate) mod tests;

pub(crate) struct Inventory {
    pub entries: Vec<dustaway_core::software::InstalledSoftware>,
    pub complete: bool,
}
impl Inventory {
    pub fn read(cancel: Option<&AtomicBool>) -> Result<Self> {
        match crate::software::recognition_inventory(cancel) {
            Ok(i) => Ok(Self {
                entries: i.entries.into_iter().map(|e| e.software).collect(),
                complete: i.warnings.is_empty(),
            }),
            Err(e) if e.is::<ScanCancelled>() => Err(e),
            Err(_) => Ok(Self {
                entries: vec![],
                complete: false,
            }),
        }
    }
    pub fn observation(
        &self,
        spec: &CacheSpec,
        roots: &[PathBuf],
    ) -> dustaway_core::ApplicationObservation {
        self.observe(
            &spec.title,
            &spec.aliases.iter().map(String::as_str).collect::<Vec<_>>(),
            roots,
        )
    }
    pub fn observe(
        &self,
        title: &str,
        aliases: &[&str],
        roots: &[PathBuf],
    ) -> dustaway_core::ApplicationObservation {
        let registered = self
            .entries
            .iter()
            .any(|e| dustaway_core::app_cache::registered_name_matches(&e.name, aliases));
        dustaway_core::ApplicationObservation {
            title: title.into(),
            identity_status: if registered {
                "registered"
            } else {
                "unverified_data"
            }
            .into(),
            locations: roots.iter().map(|p| display_path(p)).collect(),
            inventory_complete: self.complete,
        }
    }
}

pub(crate) fn roots(
    local: &Path,
    roaming: &Path,
    spec: &CacheSpec,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    check_cancel(cancel)?;
    let base = if spec.folder == "local" {
        local
    } else {
        roaming
    }
    .join(&spec.base);
    if !base.try_exists()? {
        return Ok(vec![]);
    }
    let _pin = PinnedPath::open_cancellable(&base, true, false, cancel)?;
    let mut profiles = vec![base.clone()];
    if spec.kind == "ziniao" {
        let instances = base.join("instances");
        if instances.try_exists()? {
            let _pin = PinnedPath::open_cancellable(&instances, true, false, cancel)?;
            for entry in std::fs::read_dir(instances)? {
                check_cancel(cancel)?;
                let result = (|| -> Result<Option<PathBuf>> {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !name
                        .strip_prefix("userdata")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                    {
                        return Ok(None);
                    }
                    let pin = PinnedPath::open_cancellable(&entry.path(), true, false, cancel)?;
                    Ok(Some(pin.path.clone()))
                })();
                match result {
                    Ok(Some(p)) => profiles.push(p),
                    Ok(None) => {}
                    Err(e) if e.is::<ScanCancelled>() => return Err(e),
                    Err(e) => issue(error_reason(&e)),
                }
            }
        }
    }
    if matches!(spec.kind.as_str(), "jetbrains" | "visual_studio") || spec.app == "pycharm" {
        profiles.clear();
        for entry in std::fs::read_dir(&base)? {
            check_cancel(cancel)?;
            let result = (|| -> Result<Option<PathBuf>> {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let valid = if spec.app == "pycharm" {
                    documented::pycharm_profile(&name)
                } else if spec.kind == "jetbrains" {
                    dustaway_core::developer::jetbrains_product(&name)
                } else {
                    name.split_once('_').is_some_and(|(v, id)| {
                        dustaway_core::developer::numeric_version(v)
                            && !id.is_empty()
                            && id.bytes().all(|c| c.is_ascii_hexdigit())
                    })
                };
                if !valid || !entry.file_type()?.is_dir() {
                    return Ok(None);
                }
                let pin = PinnedPath::open_cancellable(&entry.path(), true, false, cancel)?;
                Ok(Some(pin.path.clone()))
            })();
            match result {
                Ok(Some(p)) => profiles.push(p),
                Ok(None) => {}
                Err(e) if e.is::<ScanCancelled>() => return Err(e),
                Err(e) => issue(error_reason(&e)),
            }
        }
        if spec.kind == "visual_studio" {
            profiles.push(base);
        }
    }
    let mut result = Vec::new();
    for profile in profiles {
        for leaf in &spec.paths {
            check_cancel(cancel)?;
            let root = profile.join(leaf);
            match root.try_exists() {
                Ok(true) => match PinnedPath::open_cancellable(&root, true, false, cancel) {
                    Ok(pin) => result.push(pin.path.clone()),
                    Err(e) if e.is::<ScanCancelled>() => return Err(e),
                    Err(e) => issue(error_reason(&e)),
                },
                Ok(false) => {}
                Err(e) => issue(error_reason(&e.into())),
            }
        }
    }
    result.sort();
    result.dedup();
    Ok(result)
}

pub(crate) struct CodeIdentity {
    _pins: Vec<PinnedPath>,
    pub commit: String,
    pub fingerprint: String,
    /// Product-owned data directory, when the vendor stores it in its manifest.
    profile: Option<String>,
}
#[cfg(test)]
impl CodeIdentity {
    pub(crate) fn fixture(fingerprint: String) -> Self {
        Self {
            _pins: vec![],
            commit: "a".repeat(40),
            fingerprint,
            profile: Some("PyCharm2027.2".into()),
        }
    }
}
fn hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn same_version(executable: &str, package: &str) -> bool {
    fn parts(s: &str) -> Option<Vec<u32>> {
        let s = s.strip_suffix("-insider").unwrap_or(s);
        let mut p: Vec<u32> = s
            .split('.')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        if !(3..=4).contains(&p.len()) {
            return None;
        }
        while p.len() > 3 && p.last() == Some(&0) {
            p.pop();
        }
        Some(p)
    }
    parts(executable).is_some_and(|v| Some(v) == parts(package))
}
fn old_build_directory(modified: i64, at: i64, days: u32) -> bool {
    modified < at - i64::from(days) * 86400
}

pub(crate) fn code_identity(spec: &CacheSpec) -> Result<CodeIdentity> {
    ensure!(spec.cleanable(), Refusal(ReasonCode::ValidationPending));
    if matches!(spec.app.as_str(), "cherry" | "pycharm") {
        return documented::identity(spec);
    }
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let insiders = spec.app == "vscode_insiders";
    let dir = if insiders {
        "Microsoft VS Code Insiders"
    } else {
        "Microsoft VS Code"
    };
    let exe = if insiders {
        "Code - Insiders.exe"
    } else {
        "Code.exe"
    };
    let mut installs = crate::applications::installed_directories();
    installs.push(local.join("Programs").join(dir));
    for env in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(p) = std::env::var_os(env) {
            installs.push(PathBuf::from(p).join(dir));
        }
    }
    installs.sort();
    installs.dedup();
    let mut verified = Vec::new();
    let mut unresolved = false;
    for install in installs {
        let Ok(exe_pin) = PinnedPath::open(&install.join(exe), false, false) else {
            continue;
        };
        if !crate::applications::signed_by(&exe_pin, "Microsoft Corporation") {
            continue;
        }
        let mut app_dirs = vec![install.join("resources/app")];
        if let Ok(entries) = std::fs::read_dir(&install) {
            for entry in entries.flatten() {
                if hex(&entry.file_name().to_string_lossy(), 10) {
                    app_dirs.push(entry.path().join("resources/app"));
                }
            }
        }
        let mut matches = Vec::new();
        for app in app_dirs {
            let Ok(pin) = PinnedPath::open(&app.join("product.json"), false, false) else {
                continue;
            };
            if pin.stamp.size > 1024 * 1024 {
                continue;
            }
            let Ok(text) = std::fs::read(&pin.path) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&text) else {
                continue;
            };
            let Some(commit) = value["commit"].as_str().filter(|v| hex(v, 40)) else {
                continue;
            };
            if value["quality"] != if insiders { "insider" } else { "stable" } {
                continue;
            }
            if value["applicationName"] != if insiders { "code-insiders" } else { "code" } {
                continue;
            }
            let package_pin = match PinnedPath::open(&app.join("package.json"), false, false) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if package_pin.stamp.size > 1024 * 1024 {
                continue;
            }
            let package: serde_json::Value = match std::fs::read(&package_pin.path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
            {
                Some(v) => v,
                None => continue,
            };
            let version = crate::applications::executable_version(&exe_pin.path)?;
            let version_matches = package["version"]
                .as_str()
                .is_some_and(|v| same_version(&version, v));
            if !version_matches {
                continue;
            }
            matches.push((commit.to_string(), pin, package_pin));
        }
        // Multiple product files are ambiguous, even when their directory names look plausible.
        if matches.len() != 1 {
            unresolved = true;
            continue;
        }
        let (commit, product, package) = matches.remove(0);
        let fingerprint = serde_json::to_string(&(
            &commit,
            native(&exe_pin.path),
            &exe_pin.stamp,
            &exe_pin.parent_ids,
            &product.stamp,
            &product.parent_ids,
            &package.stamp,
        ))?;
        verified.push(CodeIdentity {
            _pins: vec![exe_pin, product, package],
            commit,
            fingerprint,
            profile: None,
        });
    }
    ensure!(
        verified.len() == 1 && !unresolved,
        Refusal(ReasonCode::ApplicationUnverified)
    );
    Ok(verified.remove(0))
}

/// Validate a bounded ZIP central directory without extracting, loading code or following paths.
pub(crate) fn valid_vsix(path: &Path, cancel: Option<&AtomicBool>) -> Result<bool> {
    check_cancel(cancel)?;
    let _pin = PinnedPath::open(path, false, false)?;
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    if size < 22 {
        return Ok(false);
    }
    let tail_len = size.min(65557) as usize;
    f.seek(SeekFrom::End(-(tail_len as i64)))?;
    let mut tail = vec![0; tail_len];
    f.read_exact(&mut tail)?;
    let u16at = |b: &[u8], n: usize| u16::from_le_bytes([b[n], b[n + 1]]) as usize;
    let u32at =
        |b: &[u8], n: usize| u32::from_le_bytes([b[n], b[n + 1], b[n + 2], b[n + 3]]) as u64;
    let Some(end) = (0..=tail_len - 22)
        .rev()
        .find(|&n| tail[n..n + 4] == *b"PK\x05\x06" && n + 22 + u16at(&tail, n + 20) == tail_len)
    else {
        return Ok(false);
    };
    let e = &tail[end..];
    let count = u16at(e, 10);
    let length = u32at(e, 12);
    let offset = u32at(e, 16);
    if u16at(e, 4) != 0
        || u16at(e, 6) != 0
        || count == 0
        || count > 30000
        || count != u16at(e, 8)
        || length > 16 * 1024 * 1024
        || offset + length != size - tail_len as u64 + end as u64
    {
        return Ok(false);
    }
    f.seek(SeekFrom::Start(offset))?;
    let mut directory = vec![0; length as usize];
    f.read_exact(&mut directory)?;
    let mut pos = 0;
    let mut manifest = false;
    for _ in 0..count {
        check_cancel(cancel)?;
        if pos + 46 > directory.len() || directory[pos..pos + 4] != *b"PK\x01\x02" {
            return Ok(false);
        }
        let d = &directory[pos..];
        let n = u16at(d, 28);
        let extra = u16at(d, 30);
        let comment = u16at(d, 32);
        if pos + 46 + n + extra + comment > directory.len()
            || u16at(d, 8) & 1 != 0
            || ![0, 8].contains(&u16at(d, 10))
            || u32at(d, 42) + 30 + u32at(d, 20) > offset
        {
            return Ok(false);
        }
        let name = &d[46..46 + n];
        if name.starts_with(b"/")
            || name.contains(&b'\\')
            || name.split(|&b| b == b'/').any(|p| p == b"..")
        {
            return Ok(false);
        }
        f.seek(SeekFrom::Start(u32at(d, 42)))?;
        let mut header = [0; 30];
        f.read_exact(&mut header)?;
        if header[..4] != *b"PK\x03\x04"
            || u16at(&header, 6) != u16at(d, 8)
            || u16at(&header, 8) != u16at(d, 10)
            || u16at(&header, 26) != n
            || u32at(d, 42) + 30 + n as u64 + u16at(&header, 28) as u64 + u32at(d, 20) > offset
        {
            return Ok(false);
        }
        let mut local_name = vec![0; n];
        f.read_exact(&mut local_name)?;
        if local_name != name {
            return Ok(false);
        }
        if name == b"extension/package.json" {
            if manifest {
                return Ok(false);
            }
            let packed = u32at(d, 20);
            let unpacked = u32at(d, 24);
            if packed > 1024 * 1024 || unpacked == 0 || unpacked > 1024 * 1024 {
                return Ok(false);
            }
            f.seek(SeekFrom::Current(u16at(&header, 28) as i64))?;
            let mut raw = vec![0; packed as usize];
            f.read_exact(&mut raw)?;
            let bytes = if u16at(d, 10) == 8 {
                let mut bytes = Vec::new();
                if flate2::read::DeflateDecoder::new(&raw[..])
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .is_err()
                {
                    return Ok(false);
                }
                bytes
            } else {
                raw
            };
            let mut crc = !0u32;
            for &b in &bytes {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
                }
            }
            if bytes.len() as u64 != unpacked || u64::from(!crc) != u32at(d, 16) {
                return Ok(false);
            }
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                return Ok(false);
            };
            manifest = ["name", "publisher", "version"]
                .iter()
                .all(|k| value[k].as_str().is_some_and(|s| !s.is_empty()));
            if !manifest {
                return Ok(false);
            }
            let expected = format!(
                "{}.{}-{}",
                value["publisher"].as_str().unwrap(),
                value["name"].as_str().unwrap(),
                value["version"].as_str().unwrap()
            )
            .to_lowercase();
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            // Target-platform suffixes are deliberately not adapted in this first release.
            if file_name != expected {
                return Ok(false);
            }
        }
        pos += 46 + n + extra + comment;
    }
    Ok(manifest && pos == directory.len())
}

pub(crate) fn recognized_code_file(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let parts: Vec<_> = relative
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    parts.len() >= 4
        && hex(&parts[0], 40)
        && parts[1] == "chrome"
        && matches!(parts[2].as_str(), "js" | "wasm")
        && dustaway_core::developer::accepts_file(
            "dev.vscode_cache",
            &parts[2..].iter().map(String::as_str).collect::<Vec<_>>(),
        )
}
pub(crate) fn file_reason(
    spec: &CacheSpec,
    root: &Path,
    path: &Path,
    identity: Option<&CodeIdentity>,
    cancel: Option<&AtomicBool>,
) -> Result<Option<ReasonCode>> {
    check_cancel(cancel)?;
    if !spec.cleanable() {
        // Files routed to a separately licensed rule must not be counted twice.
        // Unsupported formats remain visible in the original read-only group.
        if documented::routed_from_readonly(spec, root, path, cancel)? {
            return Ok(Some(ReasonCode::Protected));
        }
        if matches!(spec.kind.as_str(), "readonly" | "ziniao") {
            let relative = path.strip_prefix(root).context("cache root mismatch")?;
            let components: Vec<_> = relative.iter().map(|c| c.to_string_lossy()).collect();
            let parts: Vec<_> = components.iter().map(|c| c.as_ref()).collect();
            let accepted = if root.ends_with("CachedData") {
                recognized_code_file(root, path)
            } else if root.ends_with("CachedExtensionVSIXs") {
                parts.len() == 1 && !parts[0].starts_with('.') && valid_vsix(path, cancel)?
            } else {
                let parts = parts.strip_prefix(&["Cache_Data"]).unwrap_or(&parts);
                dustaway_core::developer::accepts_file("dev.vscode_cache", parts)
            };
            if !accepted {
                return Ok(Some(ReasonCode::Protected));
            }
        }
        return Ok(None);
    }
    let relative = path.strip_prefix(root).context("cache root mismatch")?;
    let parts: Vec<_> = relative
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if matches!(spec.kind.as_str(), "documented_gpu" | "documented_code") {
        if spec.app == "pycharm" {
            let Some(identity) = identity else {
                return Ok(Some(ReasonCode::ApplicationUnverified));
            };
            if !documented::profile_matches(identity.profile.as_deref(), root) {
                return Ok(Some(ReasonCode::UnsupportedLayout));
            }
        }
        return Ok(
            (!documented::accepts(spec, root, path, cancel)?).then_some(ReasonCode::Protected)
        );
    }
    if spec.kind == "vsix" {
        if parts.len() != 1
            || parts[0].starts_with('.')
            || !parts[0].contains('.')
            || !parts[0]
                .rsplit_once('-')
                .is_some_and(|(_, v)| v.as_bytes().first().is_some_and(u8::is_ascii_digit))
            || !valid_vsix(path, cancel)?
        {
            return Ok(Some(ReasonCode::Protected));
        }
    } else {
        if !recognized_code_file(root, path) {
            return Ok(Some(ReasonCode::Protected));
        }
        if !valid_code_format(path)? {
            return Ok(Some(ReasonCode::Protected));
        }
        let Some(identity) = identity else {
            return Ok(Some(ReasonCode::ApplicationUnverified));
        };
        if parts[0].eq_ignore_ascii_case(&identity.commit) {
            return Ok(Some(ReasonCode::Protected));
        }
        let commit_dir = root.join(&parts[0]);
        let pin = PinnedPath::open(&commit_dir, true, false)?;
        if !old_build_directory(pin.stamp.modified_unix(), now(), spec.age) {
            return Ok(Some(ReasonCode::Recent));
        }
        if parts[1] != "chrome"
            || !matches!(parts.get(2).map(String::as_str), Some("js" | "wasm"))
            || !dustaway_core::developer::accepts_file(
                "dev.vscode_cache",
                &parts[2..].iter().map(String::as_str).collect::<Vec<_>>(),
            )
        {
            return Ok(Some(ReasonCode::Protected));
        }
    }
    Ok(None)
}

/// Only the observed Simple Cache v5 entries and v9 marker are adapted. Other
/// index/sparse/legacy formats remain untouched, even beneath an old build.
pub(crate) fn valid_code_format(path: &Path) -> Result<bool> {
    let _pin = PinnedPath::open(path, false, false)?;
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if size < 24 {
        return Ok(false);
    }
    let mut header = [0; 24];
    file.read_exact(&mut header)?;
    if u64::from_le_bytes(header[..8].try_into().unwrap()) != 0xfcfb6d1ba7725c30 {
        return Ok(false);
    }
    let version = u32::from_le_bytes(header[8..12].try_into().unwrap());
    if path.file_name().is_some_and(|n| n == "index") {
        return Ok(size == 24 && version == 9);
    }
    if !path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with("_0"))
        || version != 5
        || size < 48
    {
        return Ok(false);
    }
    let key_length = u32::from_le_bytes(header[12..16].try_into().unwrap());
    if key_length == 0 || u64::from(key_length) + 48 > size {
        return Ok(false);
    }
    file.seek(SeekFrom::End(-24))?;
    let mut footer = [0; 24];
    file.read_exact(&mut footer)?;
    Ok(u64::from_le_bytes(footer[..8].try_into().unwrap()) == 0xf4fa6f45970d41d8)
}

pub(crate) fn valid_gpu_file(root: &Path, path: &Path) -> Result<bool> {
    documented::gpu_file(root, path)
}

pub(crate) fn blocked(spec: &CacheSpec) -> Result<Option<dustaway_core::BlockReason>> {
    let reason = |code, text: &str| Some(dustaway_core::BlockReason::new(code, text));
    if !spec.cleanable() {
        return Ok(reason(ReasonCode::ValidationPending, &spec.impact));
    }
    if let Err(error) = code_identity(spec) {
        if error_reason(&error) == ReasonCode::UnsupportedVersion {
            return Ok(reason(
                ReasonCode::UnsupportedVersion,
                &format!("{}的当前组件或数据格式尚未适配，暂不清理。", spec.title),
            ));
        }
        return Ok(reason(
            ReasonCode::ApplicationUnverified,
            &format!(
                "无法核验{}的安装与构建；缓存仍可查看，暂不清理。",
                spec.title
            ),
        ));
    }
    for name in &spec.processes {
        match security::process_running(name) {
            Ok(true) => {
                return Ok(reason(
                    ReasonCode::ApplicationRunning,
                    "应用或相关组件仍在运行，请保存工作并正常退出后重新检查。",
                ));
            }
            Err(_) => {
                return Ok(reason(
                    ReasonCode::ProcessUnknown,
                    "无法确认应用运行状态，请重新检查。",
                ));
            }
            Ok(false) => {}
        }
    }
    if spec.app == "pycharm" {
        for name in ["java.exe", "javaw.exe"] {
            if !matches!(security::process_running(name), Ok(false)) {
                return Ok(reason(
                    ReasonCode::ProcessUnknown,
                    "存在 Java 运行时或无法读取其状态，尚不能排除 IDE 组件；请正常退出相关程序后重新检查。",
                ));
            }
        }
    }
    Ok(None)
}
