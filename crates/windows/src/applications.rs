//! Local application identity discovery. Never launches applications or reads user documents.
use crate::{fs::*, platform::known_folder, security};
use anyhow::{Context, Result, ensure};
use dustaway_core::{
    ApplicationRuleSupport, ApplicationSupport, BlockReason, ReasonCode, Refusal, RuleDescriptor,
    ScanBasis, applications::*, cleanup_acceptance,
};
use std::{
    collections::BTreeSet,
    mem::size_of,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use windows::{
    Win32::{
        Foundation::{ERROR_NO_MORE_ITEMS, HWND},
        Security::{Cryptography::*, WinTrust::*},
        Storage::FileSystem::*,
        System::Registry::*,
        UI::Shell::{FOLDERID_LocalAppData, FOLDERID_RoamingAppData},
    },
    core::{PCWSTR, PWSTR, w},
};

pub(crate) struct VerifiedApplication {
    pub pin: PinnedPath,
    pub version: String,
    pub install: PathBuf,
    components: Vec<(PinnedPath, String)>,
}
impl VerifiedApplication {
    pub fn fingerprint(&self) -> Result<String> {
        Ok(serde_json::to_string(&(
            native(&self.pin.path),
            &self.pin.stamp,
            &self.version,
            self.components
                .iter()
                .map(|(pin, version)| (native(&pin.path), &pin.stamp, version))
                .collect::<Vec<_>>(),
        ))?)
    }
}

fn registry_string(hive: HKEY, key: &str, name: &str) -> Option<String> {
    let key = text_wide(key);
    let name = text_wide(name);
    let mut buffer = [0u16; 32768];
    let mut bytes = (buffer.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            hive,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .ok()
        .ok()?;
    }
    let length = buffer.iter().position(|c| *c == 0)?;
    String::from_utf16(&buffer[..length]).ok()
}
fn registry_children(hive: HKEY, key: &str) -> Vec<String> {
    registry_children_cancellable(hive, key, None).unwrap_or_default()
}
fn registry_string_cancellable(
    hive: HKEY,
    key: &str,
    name: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Option<String>> {
    check_cancel(cancel)?;
    let value = registry_string(hive, key, name);
    check_cancel(cancel)?;
    Ok(value)
}
fn registry_children_cancellable(
    hive: HKEY,
    key: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<String>> {
    check_cancel(cancel)?;
    let key = text_wide(key);
    let mut handle = HKEY::default();
    if unsafe { RegOpenKeyExW(hive, PCWSTR(key.as_ptr()), None, KEY_READ, &mut handle) }.is_err() {
        check_cancel(cancel)?;
        return Ok(vec![]);
    }
    let mut result = vec![];
    for index in 0..4096 {
        // Close the handle before propagating cancellation.
        if let Err(e) = check_cancel(cancel) {
            unsafe {
                let _ = RegCloseKey(handle);
            }
            return Err(e);
        }
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                handle,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if let Err(e) = check_cancel(cancel) {
            unsafe {
                let _ = RegCloseKey(handle);
            }
            return Err(e);
        }
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status.is_ok() {
            result.push(String::from_utf16_lossy(&name[..len as usize]));
        }
    }
    unsafe {
        let _ = RegCloseKey(handle);
    }
    Ok(result)
}
/// Registry locations only add protection; they never grant deletion authority.
pub(crate) fn installed_directories() -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for root in [
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ] {
            for sub in registry_children(hive, root) {
                let key = format!(r"{root}\{sub}");
                if let Some(value) = registry_string(hive, &key, "InstallLocation") {
                    let path = PathBuf::from(value.trim_matches('"'));
                    if path.is_absolute() {
                        paths.insert(path);
                    }
                }
                if let Some(icon) =
                    registry_string(hive, &key, "DisplayIcon").and_then(|s| resource_path(&s))
                    && let Some(parent) = icon.parent()
                {
                    paths.insert(parent.to_owned());
                }
            }
        }
    }
    paths.into_iter().collect()
}
pub(crate) fn resource_path(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    let path = if let Some(rest) = value.strip_prefix('"') {
        rest.split_once('"')?.0
    } else {
        value
            .rsplit_once(',')
            .filter(|(_, n)| n.trim().parse::<i32>().is_ok())
            .map_or(value, |(p, _)| p)
    };
    let path = PathBuf::from(path);
    (path.is_absolute()
        && path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("ico")))
    .then_some(path)
}
fn icon_path(value: &str) -> Option<PathBuf> {
    resource_path(value).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")))
}
fn candidates(app: &Application) -> Vec<PathBuf> {
    candidates_cancellable(app, None).unwrap_or_default()
}
pub(crate) fn candidates_cancellable(
    app: &Application,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<PathBuf>> {
    check_cancel(cancel)?;
    let mut found = BTreeSet::new();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for key in [
            format!(
                "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{}",
                app.executable
            ),
            format!(
                "Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{}",
                app.executable
            ),
        ] {
            check_cancel(cancel)?;
            if let Some(path) =
                registry_string_cancellable(hive, &key, "", cancel)?.and_then(|s| icon_path(&s))
            {
                found.insert(path);
            }
        }
        for key in app.registry_keys {
            check_cancel(cancel)?;
            for key in [
                key.to_string(),
                key.replacen("Software\\", "Software\\WOW6432Node\\", 1),
            ] {
                for name in ["InstallPath", "Install"] {
                    if let Some(path) = registry_string_cancellable(hive, &key, name, cancel)? {
                        let root = PathBuf::from(path);
                        if root.is_absolute() {
                            found.insert(root.join(app.executable_relative));
                        }
                    }
                }
            }
        }
        for root in [
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            "Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        ] {
            for sub in registry_children_cancellable(hive, root, cancel)? {
                let key = format!("{root}\\{sub}");
                check_cancel(cancel)?;
                let name = registry_string_cancellable(hive, &key, "DisplayName", cancel)?
                    .unwrap_or_default();
                let relevant = match app.id {
                    "weixin" => name == "微信" || name == "Weixin" || name == "WeChat",
                    "qq" | "qqnt" => name == "腾讯QQ" || name == "QQ",
                    "wps" => name.starts_with("WPS Office"),
                    "feishu" => name == "飞书" || name == "Feishu",
                    "dingtalk" => name == "钉钉" || name == "DingTalk",
                    "baidu_netdisk" => name == "百度网盘" || name == "BaiduNetdisk",
                    "quark_netdisk" => name == "夸克网盘" || name == "QuarkCloudDrive",
                    "thunder" => name == "迅雷" || name == "Thunder",
                    _ => false,
                };
                if !relevant {
                    continue;
                }
                if let Some(path) =
                    registry_string_cancellable(hive, &key, "InstallLocation", cancel)?
                {
                    let root = PathBuf::from(path);
                    if root.is_absolute() {
                        found.insert(root.join(app.executable_relative));
                    }
                }
                if let Some(icon) = registry_string_cancellable(hive, &key, "DisplayIcon", cancel)?
                    .and_then(|s| resource_path(&s))
                {
                    if icon
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case(app.executable))
                    {
                        found.insert(icon.clone());
                    }
                    // Fixed relative layouts only; do not execute or interpret registry command lines.
                    if let Some(parent) = icon.parent() {
                        found.insert(parent.join(app.executable));
                        if let Some(parent) = parent.parent() {
                            found.insert(parent.join(app.executable_relative));
                        }
                    }
                }
            }
        }
    }
    if app.id == "feishu" {
        // Launcher and app/Feishu.exe belong to the same install; each is verified separately.
        let companions: Vec<_> = found
            .iter()
            .filter_map(|p| {
                let root = installation_root(app, p).ok()?;
                Some([root.join("Feishu.exe"), root.join("app/Feishu.exe")])
            })
            .flatten()
            .collect();
        found.extend(companions);
    }
    let mut existing = Vec::new();
    for path in found {
        check_cancel(cancel)?;
        if path.is_file() {
            existing.push(path);
        }
    }
    check_cancel(cancel)?;
    Ok(existing)
}

pub fn executable_version(path: &Path) -> Result<String> {
    executable_version_cancellable(path, None)
}
pub(crate) fn executable_version_cancellable(
    path: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<String> {
    check_cancel(cancel)?;
    let name = wide(path);
    let len = unsafe { GetFileVersionInfoSizeW(PCWSTR(name.as_ptr()), None) };
    check_cancel(cancel)?;
    ensure!(len > 0 && len <= 4 * 1024 * 1024, "程序版本资源无法读取");
    let mut data = vec![0u64; (len as usize).div_ceil(8)];
    unsafe {
        GetFileVersionInfoW(PCWSTR(name.as_ptr()), None, len, data.as_mut_ptr().cast())?;
    }
    check_cancel(cancel)?;
    let mut ptr = std::ptr::null_mut();
    let mut size = 0;
    ensure!(
        unsafe { VerQueryValueW(data.as_ptr().cast(), w!("\\"), &mut ptr, &mut size) }.as_bool()
            && !ptr.is_null()
            && size as usize >= size_of::<VS_FIXEDFILEINFO>(),
        "版本资源无效"
    );
    check_cancel(cancel)?;
    let info = unsafe { std::ptr::read_unaligned(ptr.cast::<VS_FIXEDFILEINFO>()) };
    ensure!(info.dwSignature == 0xfeef04bd, "版本签名无效");
    Ok(format!(
        "{}.{}.{}.{}",
        info.dwFileVersionMS >> 16,
        info.dwFileVersionMS & 65535,
        info.dwFileVersionLS >> 16,
        info.dwFileVersionLS & 65535
    ))
}

/// Read ProductName from every declared version-resource translation; ambiguity fails closed.
pub(crate) fn executable_product_name(
    path: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<String> {
    check_cancel(cancel)?;
    let name = wide(path);
    let len = unsafe { GetFileVersionInfoSizeW(PCWSTR(name.as_ptr()), None) };
    check_cancel(cancel)?;
    ensure!(len > 0 && len <= 4 * 1024 * 1024, "程序产品资源无法读取");
    let mut data = vec![0u64; (len as usize).div_ceil(8)];
    unsafe {
        GetFileVersionInfoW(PCWSTR(name.as_ptr()), None, len, data.as_mut_ptr().cast())?;
    }
    check_cancel(cancel)?;
    let mut ptr = std::ptr::null_mut();
    let mut size = 0;
    ensure!(
        unsafe {
            VerQueryValueW(
                data.as_ptr().cast(),
                w!("\\VarFileInfo\\Translation"),
                &mut ptr,
                &mut size,
            )
        }
        .as_bool()
            && !ptr.is_null()
            && size >= 4
            && size <= len
            && size % 4 == 0,
        "产品资源翻译无效"
    );
    check_cancel(cancel)?;
    let translations =
        unsafe { std::slice::from_raw_parts(ptr.cast::<u16>(), size as usize / 2) }.to_vec();
    let mut products = BTreeSet::new();
    for pair in translations.chunks_exact(2) {
        check_cancel(cancel)?;
        let key = text_wide(&format!(
            "\\StringFileInfo\\{:04x}{:04x}\\ProductName",
            pair[0], pair[1]
        ));
        let mut value = std::ptr::null_mut();
        let mut count = 0;
        ensure!(
            unsafe {
                VerQueryValueW(
                    data.as_ptr().cast(),
                    PCWSTR(key.as_ptr()),
                    &mut value,
                    &mut count,
                )
            }
            .as_bool()
                && !value.is_null()
                && count > 1
                && count <= len / 2,
            "程序产品名称缺失"
        );
        check_cancel(cancel)?;
        let text = unsafe { std::slice::from_raw_parts(value.cast::<u16>(), count as usize) };
        ensure!(text.last() == Some(&0), "程序产品名称无效");
        products.insert(String::from_utf16(&text[..text.len() - 1])?);
    }
    ensure!(products.len() == 1, "程序产品名称不一致");
    Ok(products.into_iter().next().unwrap())
}

pub(crate) fn signed_by(pin: &PinnedPath, publisher: &str) -> bool {
    signed_by_cancellable(pin, publisher, None).unwrap_or(false)
}
pub(crate) fn signed_by_cancellable(
    pin: &PinnedPath,
    publisher: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<bool> {
    check_cancel(cancel)?;
    // The file and its ancestor directories remain pinned throughout verification/use.
    // Cache only successful checks for an unchanged file identity+stamp in this process.
    static VERIFIED: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
    let key =
        serde_json::to_string(&(native(&pin.path), &pin.stamp, publisher)).unwrap_or_default();
    let cache = VERIFIED.get_or_init(Default::default);
    if cache.lock().unwrap().contains(&key) {
        return Ok(true);
    }
    let path = wide(&pin.path);
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(path.as_ptr()),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL | WTD_REVOCATION_CHECK_NONE,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let verified = (|| -> Result<bool> {
        unsafe {
            let status = WinVerifyTrust(
                HWND(-1isize as *mut _),
                &mut action,
                (&mut data as *mut WINTRUST_DATA).cast(),
            );
            check_cancel(cancel)?;
            let mut valid = false;
            if status == 0 {
                let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
                check_cancel(cancel)?;
                if !provider.is_null() {
                    let signer = WTHelperGetProvSignerFromChain(provider, 0, false, 0);
                    check_cancel(cancel)?;
                    if !signer.is_null()
                        && (*signer).csCertChain > 0
                        && !(*signer).pasCertChain.is_null()
                    {
                        let cert = (*(*signer).pasCertChain).pCert;
                        let mut name = [0u16; 1024];
                        let len = CertGetNameStringW(
                            cert,
                            CERT_NAME_SIMPLE_DISPLAY_TYPE,
                            0,
                            None,
                            Some(&mut name),
                        );
                        check_cancel(cancel)?;
                        valid = len > 1
                            && len as usize <= name.len()
                            && String::from_utf16_lossy(&name[..len as usize - 1]) == publisher;
                    }
                }
            }
            Ok(valid)
        }
    })();
    // Releasing WinTrust state is mandatory even after cancellation.
    unsafe {
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        let _ = WinVerifyTrust(
            HWND(-1isize as *mut _),
            &mut action,
            (&mut data as *mut WINTRUST_DATA).cast(),
        );
    }
    check_cancel(cancel)?;
    let valid = verified?;
    if valid {
        let mut cache = cache.lock().unwrap();
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(key);
    }
    Ok(valid)
}

fn installation_root(app: &Application, executable: &Path) -> Result<PathBuf> {
    ensure!(
        executable
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case(app.executable)),
        Refusal(ReasonCode::ApplicationUnverified)
    );
    let parent = executable.parent().context("程序路径无效")?;
    let required_parent = match app.id {
        "qq" => Some("Bin"),
        "wps" => Some("office6"),
        "thunder" => Some("Program"),
        _ => None,
    };
    if let Some(name) = required_parent {
        ensure!(
            parent
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case(name)),
            Refusal(ReasonCode::UnsupportedLayout)
        );
        return Ok(parent.parent().context("程序目录无效")?.to_path_buf());
    }
    if app.id == "feishu"
        && parent
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("app"))
    {
        return Ok(parent.parent().context("程序目录无效")?.to_path_buf());
    }
    Ok(parent.to_path_buf())
}

fn combine_installations(mut verified: Vec<VerifiedApplication>) -> Result<VerifiedApplication> {
    ensure!(
        !verified.is_empty(),
        Refusal(ReasonCode::ApplicationUnverified)
    );
    verified.sort_by_key(|v| native(&v.pin.path).len());
    let mut primary = verified.remove(0);
    for item in verified {
        ensure!(
            display_path(&primary.install).eq_ignore_ascii_case(&display_path(&item.install)),
            Refusal(ReasonCode::ApplicationUnverified)
        );
        if primary.pin.stamp.identity != item.pin.stamp.identity {
            primary.components.push((item.pin, item.version));
        }
    }
    Ok(primary)
}

fn qqnt_version_directory(version: &str, config: &serde_json::Value) -> Result<String> {
    ensure!(valid_file_version(version), "QQNT 文件版本无效");
    let (series, build) = version.rsplit_once('.').context("QQNT 文件版本无效")?;
    let expected = format!("{series}-{build}");
    ensure!(
        config.get("curVersion").and_then(|v| v.as_str()) == Some(expected.as_str()),
        "QQNT 启动器与当前运行组件版本不一致"
    );
    Ok(expected)
}

fn qqnt_components(
    install: &Path,
    version: &str,
    publisher: &str,
) -> Result<Vec<(PinnedPath, String)>> {
    // Observed from the signed 9.9.33.52230 installer; other layouts fail closed.
    let config = PinnedPath::open(&install.join("versions/config.json"), false, false)?;
    ensure!(config.stamp.size <= 4096, "QQNT 版本配置过大");
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&config.path)?)?;
    let directory = qqnt_version_directory(version, &value)?;
    let runtime = install.join("versions").join(&directory);
    let package = PinnedPath::open(&runtime.join("resources/app/package.json"), false, false)?;
    ensure!(package.stamp.size <= 16384, "QQNT 应用配置过大");
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&package.path)?)?;
    ensure!(
        value.get("name").and_then(|v| v.as_str()) == Some("qq-chat")
            && value.get("version").and_then(|v| v.as_str()) == Some(directory.as_str())
            && value.get("platform").and_then(|v| v.as_str()) == Some("win32"),
        "QQNT 应用组件身份不匹配"
    );
    let library = PinnedPath::open(&runtime.join("QQNT.dll"), false, false)?;
    ensure!(
        signed_by(&library, publisher),
        "QQNT 运行组件签名未通过核验"
    );
    Ok(vec![
        (config, directory.clone()),
        (package, directory.clone()),
        (library, directory),
    ])
}

pub(crate) fn identify(app: &Application) -> Result<VerifiedApplication> {
    identify_candidates(
        app,
        candidates(app),
        |pin, publisher| Ok(signed_by(pin, publisher)),
        |pin| executable_version(&pin.path),
        None,
    )
}
pub(crate) fn identify_candidates(
    app: &Application,
    candidates: Vec<PathBuf>,
    signed: impl Fn(&PinnedPath, &str) -> Result<bool>,
    version: impl Fn(&PinnedPath) -> Result<String>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<VerifiedApplication> {
    let mut verified = vec![];
    let mut unsupported_layout = false;
    for candidate in candidates {
        check_cancel(cancel)?;
        let pin_result = PinnedPath::open_cancellable(&candidate, false, false, cancel);
        check_cancel(cancel)?;
        let Ok(pin) = pin_result else {
            continue;
        };
        // The official 8.2.0 installer uses this exact punctuation variant (verified
        // on the isolated computer). Do not normalize arbitrary publisher names.
        let known_dingtalk_signer =
            app.id == "dingtalk" && signed(&pin, "DingTalk Technology Co.,Ltd.")?;
        if !known_dingtalk_signer && !signed(&pin, app.publisher)? {
            continue;
        }
        let version_result = version(&pin);
        check_cancel(cancel)?;
        let version = match version_result {
            Ok(version) => version,
            Err(e) if e.is::<ScanCancelled>() => return Err(e),
            Err(_) => continue,
        };
        let install = match installation_root(app, &pin.path) {
            Ok(install) => install,
            Err(_) => {
                unsupported_layout = true;
                continue;
            }
        };
        let components = if app.id == "qqnt" {
            match qqnt_components(&install, &version, app.publisher) {
                Ok(parts) => parts,
                Err(_) => {
                    unsupported_layout = true;
                    continue;
                }
            }
        } else {
            vec![]
        };
        verified.push(VerifiedApplication {
            pin,
            version,
            install,
            components,
        });
    }
    check_cancel(cancel)?;
    if verified.is_empty() && unsupported_layout {
        return Err(Refusal(ReasonCode::UnsupportedLayout).into());
    }
    combine_installations(verified)
}

pub(crate) fn attachment_installation(app: &Application) -> Result<VerifiedApplication> {
    if candidates(app).is_empty() {
        return Err(dustaway_core::ApiError::new(
            "not_installed",
            format!("未检测到{}安装", app.title),
        )
        .into());
    }
    let mut found = identify(app)?;
    // These components were observed in the signed classic 9.7.25.29417
    // installation. Keep this requirement local to attachment capabilities;
    // ordinary QQ cache rules have their own acceptance boundary.
    if app.id == "qq" {
        for relative in ["Bin/TXPlatform.exe", "Bin/KernelUtil.dll"] {
            let pin = PinnedPath::open(&found.install.join(relative), false, false)?;
            let version = executable_version(&pin.path)?;
            ensure!(
                classic_component_matches(&found.version, &version)
                    && signed_by(&pin, app.publisher),
                Refusal(ReasonCode::ApplicationUnverified)
            );
            found.components.push((pin, version));
        }
    }
    Ok(found)
}

fn classic_component_matches(launcher: &str, component: &str) -> bool {
    valid_file_version(launcher) && launcher == component
}

fn valid_file_version(version: &str) -> bool {
    // Metadata sanity and runtime-directory construction, not a release allowlist.
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|c| c.is_ascii_digit())
                && part.parse::<u16>().is_ok()
        })
}

pub(crate) fn require_supported(rule: &RuleDescriptor) -> Result<VerifiedApplication> {
    ensure!(
        cleanup_acceptance::release_enabled(rule),
        Refusal(ReasonCode::ValidationPending)
    );
    let app = application(
        rule.app_id
            .as_deref()
            .context("application rule required")?,
    )
    .context("unknown application")?;
    let found = identify(app)?;
    if let Some(reason) = cleanup_acceptance::release_block(rule) {
        return Err(Refusal(reason).into());
    }
    Ok(found)
}
pub(crate) fn blocked(rule: &RuleDescriptor) -> Result<Option<BlockReason>> {
    let found = match require_supported(rule) {
        Ok(found) => found,
        Err(e) => {
            let code = error_reason(&e);
            return Ok(Some(BlockReason::new(code, code.message())));
        }
    };
    blocked_for_verified(rule, &found)
}

// Shared by the production gate and the isolated test-only acceptance gate.
pub(crate) fn blocked_for_verified(
    rule: &RuleDescriptor,
    found: &VerifiedApplication,
) -> Result<Option<BlockReason>> {
    let spec = dustaway_core::applications::rule(&rule.id).context("unknown application rule")?;
    let app = application(spec.app).context("unknown application")?;
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let roaming = known_folder(&FOLDERID_RoamingAppData)?;
    let layout = crate::application_layouts::inspect(&local, &roaming, spec, None, &mut |_| {})?;
    if layout.roots.is_empty() {
        let code = layout.unavailable.unwrap_or(ReasonCode::UnsupportedLayout);
        return Ok(Some(BlockReason::new(code, code.message())));
    }
    let components = security::application_processes(app.processes, &found.install)?;
    if !components.is_empty() {
        return Ok(Some(BlockReason::new(
            ReasonCode::ApplicationRunning,
            format!(
                "仍在运行：{}。请保存工作并退出，包括后台组件。",
                components.join("、")
            ),
        )));
    }
    Ok(None)
}

pub(crate) fn scan_basis(version: &str, observed: &str, recognized: bool) -> ScanBasis {
    if !recognized {
        ScanBasis::Unverified
    } else if version == observed {
        ScanBasis::VerifiedVersion
    } else {
        ScanBasis::RecognizedLayout
    }
}

pub fn support() -> Vec<ApplicationSupport> {
    let descriptors = dustaway_core::rules::catalog().expect("compiled rule catalog");
    let local = known_folder(&FOLDERID_LocalAppData);
    let roaming = known_folder(&FOLDERID_RoamingAppData);
    APPLICATIONS
        .iter()
        .map(|app| {
            let specs: Vec<_> = APPLICATION_RULES
                .iter()
                .filter(|r| r.app == app.id)
                .collect();
            let mut result = ApplicationSupport {
                app_id: app.id.into(),
                title: app.title.into(),
                version: None,
                supported_version: app.version.into(),
                identity_verified: false,
                scan_supported: false,
                cleanup_validated: false,
                rules: specs
                    .iter()
                    .map(|r| {
                        let descriptor = descriptors
                            .iter()
                            .find(|item| item.id == r.id)
                            .expect("registered rule");
                        let enabled = cleanup_acceptance::release_enabled(descriptor);
                        ApplicationRuleSupport {
                            rule_id: r.id.into(),
                            cleanup_validated: enabled,
                            scan_basis: ScanBasis::Unverified,
                            layout_id: None,
                            cleanup_versions: Some(cleanup_acceptance::versions(descriptor)),
                            cleanup_block: Some(if enabled {
                                ReasonCode::ApplicationUnverified
                            } else {
                                ReasonCode::ValidationPending
                            }),
                        }
                    })
                    .collect(),
                status: Some(ReasonCode::ApplicationUnverified),
                message: String::new(),
                running_components: vec![],
                locations: match (&local, &roaming) {
                    (Ok(local), Ok(roaming)) => specs
                        .iter()
                        .map(|r| crate::application_layouts::description(local, roaming, r))
                        .collect(),
                    _ => vec![],
                },
            };
            result.cleanup_validated = result.rules.iter().all(|r| r.cleanup_validated);
            match identify(app) {
                Err(e) => {
                    result.status = Some(error_reason(&e));
                    for rule in &mut result.rules {
                        if rule.cleanup_validated {
                            rule.cleanup_block = result.status;
                        }
                    }
                }
                Ok(found) => {
                    result.identity_verified = true;
                    result.version = Some(found.version.clone());
                    let mut layout_problem = None;
                    for (spec, rule) in specs.iter().zip(&mut result.rules) {
                        let descriptor = descriptors
                            .iter()
                            .find(|r| r.id == spec.id)
                            .expect("registered rule");
                        rule.cleanup_validated = cleanup_acceptance::release_enabled(descriptor);
                        rule.cleanup_block = cleanup_acceptance::release_block(descriptor);
                        let mut issues = vec![];
                        let inspection = match (&local, &roaming) {
                            (Ok(local), Ok(roaming)) => crate::application_layouts::inspect(
                                local,
                                roaming,
                                spec,
                                None,
                                &mut |code| issues.push(code),
                            ),
                            _ => Err(Refusal(ReasonCode::DirectoryInaccessible).into()),
                        };
                        match inspection {
                            Ok(layout) => {
                                let recognized = !layout.roots.is_empty();
                                rule.scan_basis =
                                    scan_basis(app.version, &found.version, recognized);
                                rule.layout_id = recognized.then(|| spec.layout_id.into());
                                result.scan_supported |= recognized;
                                layout_problem = layout_problem
                                    .or(issues.first().copied())
                                    .or(layout.unavailable);
                                // A partial read can still expose individually verified candidates.
                                if !recognized {
                                    rule.cleanup_block = rule
                                        .cleanup_block
                                        .or(issues.first().copied())
                                        .or(layout.unavailable)
                                        .or(Some(ReasonCode::UnsupportedLayout));
                                }
                            }
                            Err(e) => {
                                let reason = error_reason(&e);
                                layout_problem = layout_problem.or(Some(reason));
                                rule.cleanup_block = rule.cleanup_block.or(Some(reason));
                            }
                        }
                    }
                    result.status =
                        match security::application_processes(app.processes, &found.install) {
                            Ok(components) => {
                                result.running_components = components;
                                if !result.running_components.is_empty() {
                                    for rule in &mut result.rules {
                                        rule.cleanup_block = rule
                                            .cleanup_block
                                            .or(Some(ReasonCode::ApplicationRunning));
                                    }
                                }
                                if !result.scan_supported {
                                    layout_problem.or(Some(ReasonCode::UnsupportedLayout))
                                } else if !result.running_components.is_empty() {
                                    Some(ReasonCode::ApplicationRunning)
                                } else if !result.cleanup_validated {
                                    Some(ReasonCode::ValidationPending)
                                } else {
                                    None
                                }
                            }
                            Err(_) => {
                                for rule in &mut result.rules {
                                    rule.cleanup_block =
                                        rule.cleanup_block.or(Some(ReasonCode::ProcessUnknown));
                                }
                                Some(ReasonCode::ProcessUnknown)
                            }
                        };
                }
            }
            result.cleanup_validated = result.rules.iter().all(|r| r.cleanup_validated);
            result.message = result
                .status
                .map_or("已识别扫描位置，清理能力须逐项核对", ReasonCode::message)
                .into();
            result
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_attachment_components_require_matching_full_versions() {
        assert!(classic_component_matches("9.7.25.29417", "9.7.25.29417"));
        assert!(!classic_component_matches("9.7.25.29417", "9.7.25.29416"));
        assert!(!classic_component_matches("9.7.25.29417", "9.7.23.29394"));
        assert!(classic_component_matches("9.9.33.52230", "9.9.33.52230"));
        assert!(classic_component_matches("12.0.1.20", "12.0.1.20"));
        for malformed in ["", "9.7", "9.7.25.x", "9.7.25.65536", "../9.7.25.0"] {
            assert!(!classic_component_matches(malformed, malformed));
        }
    }
    #[test]
    fn qqnt_runtime_selection_is_bound_to_the_launcher_version() {
        assert_eq!(
            qqnt_version_directory(
                "9.9.33.52230",
                &serde_json::json!({"curVersion":"9.9.33-52230"})
            )
            .unwrap(),
            "9.9.33-52230"
        );
        for value in [
            "../9.9.33-52230",
            "C:\\Other",
            "9.9.32-52230",
            "9.9.33-52231",
            "",
        ] {
            assert!(
                qqnt_version_directory("9.9.33.52230", &serde_json::json!({"curVersion":value}))
                    .is_err()
            );
        }
        assert!(
            qqnt_version_directory(
                "9.7.25.29417",
                &serde_json::json!({"curVersion":"9.7.25-29417"})
            )
            .is_ok()
        );
        assert!(qqnt_version_directory("9.9.33.52230", &serde_json::json!({})).is_err());
        assert!(
            qqnt_version_directory("12.0.1.20", &serde_json::json!({"curVersion":"12.0.1-20"}))
                .is_ok()
        );
    }
    #[test]
    #[ignore = "read-only check of the separately downloaded and extracted official QQNT installer"]
    fn qqnt_signed_installer_components_are_pinned_without_launching() -> Result<()> {
        let root = PathBuf::from(std::env::var("DUSTAWAY_QQNT_EXTRACTED_ROOT")?);
        let launcher = PinnedPath::open(&root.join("QQ.exe"), false, false)?;
        let publisher = "Tencent Technology (Shenzhen) Company Limited";
        ensure!(
            signed_by(&launcher, publisher),
            "Launcher signature invalid"
        );
        let version = executable_version(&launcher.path)?;
        ensure!(version == "9.9.33.52230", "Wrong evidence version");
        let components = qqnt_components(&root, &version, publisher)?;
        assert_eq!(components.len(), 3);
        assert!(
            components
                .iter()
                .all(|(_, version)| version == "9.9.33-52230")
        );
        Ok(())
    }
    #[test]
    fn known_layouts_can_be_scanned_across_versions_without_cleanup_acceptance() {
        assert_eq!(
            scan_basis("1.0.0.0", "2.0.0.0", true),
            ScanBasis::RecognizedLayout
        );
        assert_eq!(
            scan_basis("1.0.0.0", "1.0.0.0", true),
            ScanBasis::VerifiedVersion
        );
        assert_eq!(
            scan_basis("1.0.0.0", "1.0.0.0", false),
            ScanBasis::Unverified
        );
        for spec in APPLICATION_RULES {
            assert_eq!(
                spec.validated(),
                matches!(spec.id, "dingtalk.http" | "dingtalk.code")
            );
            // Observed scan versions never become accepted cleanup versions.
            assert!(
                !cleanup_acceptance::versions_for_spec(spec, 1, cleanup_acceptance::ACCEPTANCES)
                    .contains(&application(spec.app).unwrap().version.to_owned())
            );
        }
    }
    #[test]
    fn classic_qq_install_layout_does_not_accept_qqnt_or_other_executables() {
        let qq = application("qq").unwrap();
        assert_eq!(
            installation_root(qq, Path::new(r"C:\Apps\QQ\Bin\QQ.exe")).unwrap(),
            PathBuf::from(r"C:\Apps\QQ")
        );
        for path in [
            r"C:\Apps\QQNT\QQ.exe",
            r"C:\Apps\QQNT\resources\app\QQ.exe",
            r"C:\Apps\QQ\Bin\other.exe",
        ] {
            assert!(installation_root(qq, Path::new(path)).is_err());
        }
    }
    #[test]
    fn feishu_launcher_and_runtime_share_an_install_and_both_affect_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Feishu");
        let runtime = root.join("app/Feishu.exe");
        std::fs::create_dir_all(runtime.parent().unwrap()).unwrap();
        std::fs::write(root.join("Feishu.exe"), b"fixture launcher").unwrap();
        std::fs::write(&runtime, b"fixture runtime").unwrap();
        let make = |path: PathBuf, version: &str| VerifiedApplication {
            install: installation_root(application("feishu").unwrap(), &path).unwrap(),
            pin: PinnedPath::open(&path, false, false).unwrap(),
            version: version.into(),
            components: vec![],
        };
        let mut combined = combine_installations(vec![
            make(runtime, "7.64.0.56"),
            make(root.join("Feishu.exe"), "7.64.6.0"),
        ])
        .unwrap();
        assert_eq!(combined.version, "7.64.6.0");
        assert_eq!(combined.components.len(), 1);
        let original = combined.fingerprint().unwrap();
        combined.components[0].1 = "7.65.0.0".into();
        assert_ne!(combined.fingerprint().unwrap(), original);
        let other = dir.path().join("Other/Feishu.exe");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::write(&other, b"fixture other install").unwrap();
        assert!(combine_installations(vec![combined, make(other, "7.64.6.0")]).is_err());
    }
    #[test]
    fn registry_icons_are_paths_never_commands() {
        assert_eq!(
            icon_path("\"D:\\WPS Office\\utility\\uninst.exe\",0").unwrap(),
            PathBuf::from("D:\\WPS Office\\utility\\uninst.exe")
        );
        assert!(icon_path("cmd.exe /c erase").is_none());
        assert!(icon_path("D:\\app.exe --delete").is_none());
        assert!(icon_path("relative.exe").is_none());
    }
    #[test]
    fn unsigned_file_cannot_establish_application_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Weixin.exe");
        std::fs::write(&path, b"not a signed executable").unwrap();
        let pin = PinnedPath::open(&path, false, false).unwrap();
        for app in APPLICATIONS {
            assert!(!signed_by(&pin, app.publisher));
        }
        assert!(executable_version(&path).is_err());
    }

    #[test]
    fn netdisk_names_install_structure_and_conflicts_are_checked() {
        for id in ["baidu_netdisk", "quark_netdisk", "thunder"] {
            let app = application(id).unwrap();
            let temp = tempfile::tempdir().unwrap();
            let make = |name: &str| {
                let path = temp.path().join(name).join(app.executable_relative);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, b"unsigned test fixture").unwrap();
                VerifiedApplication {
                    install: installation_root(app, &path).unwrap(),
                    pin: PinnedPath::open(&path, false, false).unwrap(),
                    version: "99.0.0.0".into(),
                    components: vec![],
                }
            };
            assert!(installation_root(app, &temp.path().join("wrong.exe")).is_err());
            if id == "thunder" {
                assert!(installation_root(app, &temp.path().join("Thunder.exe")).is_err());
            }
            assert!(combine_installations(vec![make("first"), make("second")]).is_err());
        }
    }
}
