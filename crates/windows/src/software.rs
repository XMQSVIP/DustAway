//! Native uninstall providers. Registry data never grants a generic shell or deletion capability.
#[cfg(test)]
mod brave_acceptance;
#[cfg(test)]
mod firefox_acceptance;
#[cfg(test)]
mod firefox_rules_acceptance;
#[cfg(test)]
mod firefox_scope_acceptance;
mod icons;
mod residual_scope;
mod residual_walk;
mod store_isolation;
mod store_policy;
mod store_processes;
use crate::{fs::*, platform::known_folder, security};
use anyhow::{Context, Result, bail, ensure};
use dustaway_core::{ReasonCode, Refusal, software::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    mem::size_of,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};
use windows::{
    ApplicationModel::PackageSignatureKind,
    Management::Deployment::{PackageManager, PackageTypes},
    Win32::{
        Foundation::*,
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_ENCRYPTED,
            FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_READ_ATTRIBUTES, FILE_SHARE_READ, OPEN_EXISTING,
        },
        System::{
            ApplicationInstallationAndServicing::*,
            Com::*,
            Registry::*,
            SystemInformation::{GetSystemDirectoryW, GetTickCount64},
            Threading::*,
        },
        UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{HSTRING, PCWSTR, PWSTR, w},
};

pub struct WindowsSoftware;
fn system_installer_path() -> Result<PathBuf> {
    let mut dir = vec![0u16; 32768];
    let n = unsafe { GetSystemDirectoryW(Some(&mut dir)) } as usize;
    ensure!(n > 0 && n < dir.len(), "无法定位 Windows Installer");
    Ok(PathBuf::from(String::from_utf16(&dir[..n])?).join("msiexec.exe"))
}
/// Execution-only pin. Windows servicing hardlinks are valid for the OS-owned MSI host.
/// Its handle has no DELETE or write access; cleanup continues to use PinnedPath unchanged.
enum UninstallerPin {
    Desktop(PinnedPath),
    SystemMsi {
        _parent: PinnedPath,
        _file: Handle,
        stamp: Stamp,
    },
}
impl UninstallerPin {
    fn open(path: &Path, msi: bool) -> Result<Self> {
        if !msi {
            return Ok(Self::Desktop(PinnedPath::open(path, false, false)?));
        }
        ensure!(
            extended(path)? == extended(&system_installer_path()?)?,
            "MSI 只允许系统 Windows Installer"
        );
        is_fixed_ntfs(path)?;
        let parent = PinnedPath::open(path.parent().context("系统安装程序路径无效")?, true, false)?;
        let name = wide(path);
        let file = Handle(unsafe {
            CreateFileW(
                PCWSTR(name.as_ptr()),
                FILE_READ_ATTRIBUTES.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }?);
        let stamp = stamp(&file)?;
        let forbidden = FILE_ATTRIBUTE_DIRECTORY.0
            | FILE_ATTRIBUTE_REPARSE_POINT.0
            | FILE_ATTRIBUTE_OFFLINE.0
            | FILE_ATTRIBUTE_ENCRYPTED.0
            | 0x40000
            | 0x400000;
        ensure!(
            stamp.attributes & forbidden == 0 && stamp.links > 0,
            "系统安装程序不是可核验的本地文件"
        );
        ensure!(
            final_path(&file)?.parent() == Some(final_path(&parent.handle)?.as_path()),
            "系统安装程序实际路径发生变化"
        );
        Ok(Self::SystemMsi {
            _parent: parent,
            _file: file,
            stamp,
        })
    }
    fn stamp(&self) -> &Stamp {
        match self {
            Self::Desktop(pin) => &pin.stamp,
            Self::SystemMsi { stamp, .. } => stamp,
        }
    }
}
struct Com;
impl Com {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
fn open_key(hive: HKEY, path: &str, view: REG_SAM_FLAGS) -> Result<Option<Key>> {
    let path = text_wide(path);
    let mut key = HKEY::default();
    let status =
        unsafe { RegOpenKeyExW(hive, PCWSTR(path.as_ptr()), None, KEY_READ | view, &mut key) };
    if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
        return Ok(None);
    }
    status.ok()?;
    Ok(Some(Key(key)))
}
fn string(key: &Key, name: &str) -> Result<Option<String>> {
    let name = text_wide(name);
    let mut data = vec![0u16; 32768];
    let mut len = (data.len() * 2) as u32;
    let code = unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    code.ok()?;
    let end = data
        .iter()
        .position(|c| *c == 0)
        .context("注册表文本缺少结束标记")?;
    Ok(Some(String::from_utf16(&data[..end])?))
}
fn number(key: &Key, name: &str) -> Result<Option<u32>> {
    let name = text_wide(name);
    let mut value = 0u32;
    let mut len = 4u32;
    let code = unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut len),
        )
    };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    code.ok()?;
    Ok(Some(value))
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Registration {
    hive: String,
    view: u32,
    key: String,
    name: String,
    publisher: String,
    version: String,
    command: String,
    location: Option<String>,
    size: Option<u32>,
    date: Option<String>,
    msi: bool,
    no_remove: bool,
}
impl Registration {
    fn identity(&self) -> SoftwareIdentity {
        SoftwareIdentity {
            source: SoftwareSource::Desktop,
            key: serde_json::to_string(&(&self.hive, self.view, &self.key)).unwrap(),
            family: family(&self.name).unwrap_or("").into(),
        }
    }
    fn action(&self) -> Result<(String, String)> {
        if self.msi {
            let guid = self.key.rsplit('\\').next().unwrap_or("");
            ensure!(
                guid.len() == 38
                    && guid.starts_with('{')
                    && guid.ends_with('}')
                    && uuid::Uuid::parse_str(&guid[1..37]).is_ok(),
                "无效 MSI 产品代码"
            );
            let code = text_wide(guid);
            let state = unsafe { MsiQueryProductStateW(PCWSTR(code.as_ptr())) };
            ensure!(state == INSTALLSTATE_DEFAULT, "无法确认 MSI 产品安装状态");
            Ok((
                system_installer_path()?.to_string_lossy().into_owned(),
                format!("/x {guid} /norestart"),
            ))
        } else {
            registered_executable_command(&self.command)
        }
    }
    fn entry(&self) -> SoftwareEntry {
        let identity = self.identity();
        let action = if self.no_remove {
            Err(anyhow::anyhow!("软件禁止直接卸载"))
        } else {
            self.action().and_then(|action| {
                ensure!(
                    Path::new(&action.0).is_file(),
                    "卸载程序已缺失，请使用 Windows 应用设置"
                );
                Ok(action)
            })
        };
        let error = action.err().map(|e| e.to_string());
        let supported = !identity.family.is_empty();
        SoftwareEntry {
            software: InstalledSoftware {
                id: format!("desktop:{:x}", Sha256::digest(identity.key.as_bytes())),
                name: self.name.clone(),
                publisher: self.publisher.clone(),
                version: self.version.clone(),
                source: SoftwareSource::Desktop,
                scope: if self.hive == "user" {
                    "current_user"
                } else {
                    "machine"
                }
                .into(),
                estimated_bytes: self.size.map(|n| (n as u64 * 1024).to_string()),
                installed_on: self.date.as_ref().and_then(|s| install_date(s)),
                install_location: self.location.clone(),
                icon_data_url: icons::desktop(self),
                can_uninstall: error.is_none(),
                unavailable_reason: error,
                residual_supported: supported,
                residual_note: residual_note(supported, &identity.family),
                residual_capabilities: residual_rules::capabilities(&identity),
                missing_information: vec![],
            }
            .with_missing_information(),
            identity,
        }
    }
}
fn install_date(s: &str) -> Option<String> {
    let b = s.as_bytes();
    (b.len() == 8 && b.iter().all(u8::is_ascii_digit))
        .then(|| format!("{}-{}-{}", &s[..4], &s[4..6], &s[6..]))
}
/// Some vendors register a bare path containing spaces, without arguments or quotes.
/// Accept only the entire existing local executable, with no competing executable
/// at a whitespace boundary. ShellExecute still receives file and arguments separately.
fn registered_executable_command(value: &str) -> Result<(String, String)> {
    match executable_command(value) {
        Ok(action) => Ok(action),
        Err(original) => {
            let path = value.trim();
            if path.contains('"') || !path.to_ascii_lowercase().ends_with(".exe") {
                return Err(original);
            }
            let action = executable_command(&format!("\"{path}\""))?;
            ensure!(
                Path::new(&action.0).is_file(),
                "卸载程序路径不明确，请使用 Windows 设置"
            );
            for (index, _) in path.char_indices().filter(|(_, c)| c.is_whitespace()) {
                let prefix = &path[..index];
                for alternative in [
                    PathBuf::from(prefix),
                    PathBuf::from(format!("{prefix}.exe")),
                ] {
                    match std::fs::symlink_metadata(&alternative) {
                        Ok(meta) => ensure!(
                            meta.is_dir() && !meta.file_type().is_symlink(),
                            "卸载命令存在歧义，请使用 Windows 设置"
                        ),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => bail!("无法核对卸载路径，请使用 Windows 设置"),
                    }
                }
            }
            // Do not grant linked, remote, cloud, or replaceable paths a fallback.
            let _pin = UninstallerPin::open(Path::new(&action.0), false)?;
            Ok(action)
        }
    }
}
fn registration(hive: &str, view: u32, path: &str) -> Result<Option<Registration>> {
    let Some(key) = open_key(
        if hive == "user" {
            HKEY_CURRENT_USER
        } else {
            HKEY_LOCAL_MACHINE
        },
        path,
        REG_SAM_FLAGS(view),
    )?
    else {
        return Ok(None);
    };
    if number(&key, "SystemComponent")? == Some(1)
        || string(&key, "ParentKeyName")?.is_some()
        || string(&key, "ReleaseType")?
            .is_some_and(|s| matches!(s.as_str(), "Update" | "Hotfix" | "Security Update"))
    {
        return Ok(None);
    }
    let Some(name) = string(&key, "DisplayName")?.filter(|s| !s.trim().is_empty()) else {
        return Ok(None);
    };
    Ok(Some(Registration {
        hive: hive.into(),
        view,
        key: path.into(),
        name,
        publisher: string(&key, "Publisher")?.unwrap_or_default(),
        version: string(&key, "DisplayVersion")?.unwrap_or_default(),
        command: string(&key, "UninstallString")?.unwrap_or_default(),
        location: string(&key, "InstallLocation")?.filter(|s| !s.is_empty()),
        size: number(&key, "EstimatedSize")?,
        date: string(&key, "InstallDate")?,
        msi: number(&key, "WindowsInstaller")? == Some(1),
        no_remove: number(&key, "NoRemove")? == Some(1),
    }))
}
fn desktop_inventory() -> Result<Vec<Registration>> {
    let mut found = vec![];
    let root = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    for (label, hive) in [("user", HKEY_CURRENT_USER), ("machine", HKEY_LOCAL_MACHINE)] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let Some(key) = open_key(hive, root, view)? else {
                continue;
            };
            for index in 0..10000u32 {
                let mut name = [0u16; 256];
                let mut len = 256u32;
                let code = unsafe {
                    RegEnumKeyExW(
                        key.0,
                        index,
                        Some(PWSTR(name.as_mut_ptr())),
                        &mut len,
                        None,
                        None,
                        None,
                        None,
                    )
                };
                if code == ERROR_NO_MORE_ITEMS {
                    break;
                }
                code.ok()?;
                ensure!(index < 9999, "软件登记数量超过检查上限");
                let path = format!("{root}\\{}", String::from_utf16(&name[..len as usize])?);
                if let Some(r) = registration(label, view.0, &path)? {
                    found.push(r)
                }
            }
        }
    }
    Ok(deduplicate_registrations(found))
}
fn deduplicate_registrations(mut found: Vec<Registration>) -> Vec<Registration> {
    // HKCU's shared view may expose the very same key twice. Do not merge different registrations.
    let mut seen = HashSet::new();
    found.retain(|r| {
        seen.insert(if r.hive == "user" {
            format!("{}:{}:{}", r.hive, r.key, r.command)
        } else {
            format!("{}:{}:{}", r.hive, r.view, r.key)
        })
    });
    found
}
fn package_date(value: i64) -> Option<String> {
    let value = u64::try_from(value).ok()?;
    let filetime = FILETIME {
        dwLowDateTime: value as u32,
        dwHighDateTime: (value >> 32) as u32,
    };
    let mut time = SYSTEMTIME::default();
    unsafe { windows::Win32::System::Time::FileTimeToSystemTime(&filetime, &mut time) }.ok()?;
    Some(format!(
        "{:04}-{:02}-{:02}",
        time.wYear, time.wMonth, time.wDay
    ))
}
fn package_display_name(value: windows::core::Result<HSTRING>, fallback: &str) -> String {
    // A missing localized resource map must not hide every other installed package.
    value
        .ok()
        .map(|name| name.to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}
fn store_inventory() -> Result<SoftwareInventory> {
    store_inventory_with_policy(true, None)
}
fn store_inventory_with_policy(
    removal_policy: bool,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<SoftwareInventory> {
    let _com = Com::new()?;
    let manager = PackageManager::new()?;
    let mut entries = vec![];
    let mut warnings = vec![];
    let policy = if removal_policy {
        store_policy::read().ok()
    } else {
        None
    };
    let iterator = manager
        .FindPackagesByUserSecurityIdWithPackageTypes(&HSTRING::new(), PackageTypes::Main)?
        .First()?;
    let mut packages = Vec::new();
    while iterator.HasCurrent()? {
        check_cancel(cancel)?;
        packages.push(iterator.Current()?);
        ensure!(packages.len() <= 10000, "应用包数量超过检查上限");
        iterator.MoveNext()?;
    }
    for p in packages {
        check_cancel(cancel)?;
        if p.IsFramework()? || p.IsResourcePackage()? || p.IsBundle()? {
            continue;
        }
        let id = p.Id()?;
        let key = id.FullName()?.to_string();
        let family = id.FamilyName()?.to_string();
        let v = id.Version()?;
        let non_removable = policy.as_ref().and_then(|p| p.get(&key)).copied();
        if removal_policy && non_removable.is_none() && warnings.is_empty() {
            warnings.push("部分商店应用的移除策略未能核验，这些应用暂不允许卸载".into());
        }
        let unavailable_reason = store_policy::refusal(
            p.SignatureKind()? == PackageSignatureKind::System,
            non_removable,
        );
        let identity = SoftwareIdentity {
            source: SoftwareSource::Store,
            key,
            family,
        };
        let name = package_display_name(p.DisplayName(), &id.Name()?.to_string());
        entries.push(SoftwareEntry {
            software: InstalledSoftware {
                id: format!("store:{:x}", Sha256::digest(identity.key.as_bytes())),
                name,
                publisher: p
                    .PublisherDisplayName()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                version: format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision),
                source: SoftwareSource::Store,
                scope: "current_user".into(),
                estimated_bytes: None,
                installed_on: p
                    .InstalledDate()
                    .ok()
                    .and_then(|d| package_date(d.UniversalTime)),
                install_location: p.InstalledPath().ok().map(|p| p.to_string()),
                icon_data_url: if removal_policy {
                    icons::store(&p).ok()
                } else {
                    None
                },
                can_uninstall: unavailable_reason.is_none(),
                unavailable_reason,
                residual_supported: true,
                residual_note: residual_note(true, "store"),
                residual_capabilities: residual_rules::capabilities(&identity),
                missing_information: vec![],
            }
            .with_missing_information(),
            identity,
        });
    }
    Ok(SoftwareInventory { entries, warnings })
}
fn residual_note(supported: bool, family: &str) -> String {
    if family == "wps" && supported {
        "检查已核验的日志、默认文档备份和两份固定加载项设置；自定义备份位置、加载项代码及其他未确认内容保留。备份可能是未保存文档的唯一副本。"
    } else if supported {
        "仅检查可确认归属的固定位置，实际范围需在卸载前核验；自定义、共享或归属不明的位置保留。"
    } else {
        "暂不支持该软件的残留检查；标准卸载能力单独判断。"
    }
    .into()
}

// Private system boundary, shared by production and the opt-in missing-fixture check.
// All production callers validate current identity and removal policy before entering.
fn remove_store_package(key: &str, progress: &mut dyn FnMut(u32)) -> Result<LaunchOutcome> {
    let _com = Com::new()?;
    let op = PackageManager::new()?.RemovePackageAsync(&HSTRING::from(key))?;
    let value = Arc::new(AtomicU32::new(0));
    let update = value.clone();
    op.SetProgress(&windows_future::AsyncOperationProgressHandler::<
        windows::Management::Deployment::DeploymentResult,
        windows::Management::Deployment::DeploymentProgress,
    >::new(move |_, p| {
        update.store(p.percentage, Ordering::Release);
        Ok(())
    }))?;
    while op.Status()?.0 == 0 {
        progress(value.load(Ordering::Acquire));
        std::thread::sleep(Duration::from_millis(150));
    }
    if op.Status()?.0 == 2 {
        return Ok(LaunchOutcome::Cancelled);
    }
    let result = op.GetResults()?;
    result.ExtendedErrorCode()?.ok()?;
    Ok(LaunchOutcome::Completed)
}
pub fn family(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "google chrome" => Some("chrome"),
        "microsoft edge" => Some("edge"),
        "brave" => Some("brave"),
        "vivaldi" => Some("vivaldi"),
        "微信" | "weixin" | "wechat" => Some("weixin"),
        "qq" | "腾讯qq" => Some("qq"),
        "飞书" | "feishu" => Some("feishu"),
        "钉钉" | "dingtalk" => Some("dingtalk"),
        "百度网盘" | "baidunetdisk" => Some("baidu_netdisk"),
        "夸克网盘" | "quarkclouddrive" => Some("quark_netdisk"),
        "迅雷" | "thunder" => Some("thunder"),
        s if s.starts_with("mozilla firefox") => Some("firefox"),
        s if s == "wps office" || s.starts_with("wps office ") => Some("wps"),
        _ => None,
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Root {
    path: PathBuf,
    stamp: Stamp,
    parents: Vec<Identity>,
}
#[derive(Serialize, Deserialize)]
struct ContextData {
    rule_version: u32,
    file: Option<PathBuf>,
    args: String,
    stamp: Option<Stamp>,
    roots: Vec<Root>,
    processes: Vec<String>,
    warnings: Vec<String>,
    #[serde(default)]
    residual_scopes: Vec<residual_scope::CapturedScope>,
    #[serde(default)]
    verified_version: Option<String>,
    #[serde(default)]
    store_isolation: Option<store_isolation::Proof>,
}
#[derive(Serialize, Deserialize)]
struct ResidualStamp {
    path: PathBuf,
    root: usize,
    stamp: Stamp,
    parents: Vec<Identity>,
    #[serde(default)]
    scope: Option<usize>,
}
fn roots(entry: &SoftwareEntry) -> Result<(Vec<Root>, Vec<String>)> {
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let roaming = known_folder(&FOLDERID_RoamingAppData)?;
    roots_at(entry, &local, &roaming)
}
fn roots_at(
    entry: &SoftwareEntry,
    local: &Path,
    roaming: &Path,
) -> Result<(Vec<Root>, Vec<String>)> {
    let mut paths: Vec<PathBuf> = vec![];
    let f = entry.identity.family.as_str();
    if entry.identity.source == SoftwareSource::Store {
        ensure!(
            !f.is_empty()
                && f.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')),
            "无效应用包身份"
        );
        for leaf in [
            "LocalCache",
            "TempState",
            "LocalState",
            "RoamingState",
            "Settings",
        ] {
            paths.push(local.join("Packages").join(f).join(leaf));
        }
    } else if let Some(product) = dustaway_core::browsers::chromium(f) {
        paths.push(local.join(product.user_data));
    } else {
        match f {
            "firefox" => {
                paths.push(local.join("Mozilla/Firefox/Profiles"));
                paths.push(roaming.join("Mozilla/Firefox"));
            }
            "weixin" => paths.push(roaming.join("Tencent/xwechat")),
            "qq" => paths.push(roaming.join("Tencent/QQ/webkitex_cache")),
            "wps" => {
                for locator in [
                    residual_rules::Locator::WpsOfficeData,
                    residual_rules::Locator::WpsAddins,
                ] {
                    paths.push(
                        roaming.join(locator.fixed_roaming_base().expect("fixed WPS location")),
                    );
                }
            }
            "feishu" => paths.push(roaming.join("LarkShell/aha/users")),
            "baidu_netdisk" => paths.push(roaming.join("baidunetdisk")),
            "quark_netdisk" => paths.push(roaming.join("quark-cloud-drive")),
            "thunder" => paths.push(roaming.join("thunder")),
            "dingtalk" => {
                for e in std::fs::read_dir(local)? {
                    let e = e?;
                    let name = e.file_name().to_string_lossy().to_string();
                    if name
                        .strip_prefix("DingTalk_")
                        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                    {
                        paths.push(e.path().join("Default"));
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = vec![];
    let mut warnings = vec![];
    for path in paths {
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => warnings.push("部分已支持位置无法读取，已保留".into()),
            Ok(_) => match PinnedPath::open(&path, true, false) {
                Ok(pin) => out.push(Root {
                    path,
                    stamp: pin.stamp,
                    parents: pin.parent_ids,
                }),
                Err(_) => warnings.push("部分已支持位置被占用或包含链接，已保留".into()),
            },
        }
    }
    Ok((out, warnings))
}
fn processes(f: &str) -> Vec<String> {
    if let Some(app) = dustaway_core::applications::APPLICATIONS
        .iter()
        .find(|a| a.id == f)
    {
        return app.processes.iter().map(|s| (*s).into()).collect();
    }
    dustaway_core::browsers::chromium(f)
        .map(|p| vec![p.process.into()])
        .unwrap_or_else(|| {
            if f == "firefox" {
                vec!["firefox.exe".into()]
            } else {
                vec![]
            }
        })
}

fn ensure_no_family_installation(identity: &SoftwareIdentity) -> Result<()> {
    ensure_no_family_installation_with(identity, desktop_inventory)
}
fn ensure_no_family_installation_with(
    identity: &SoftwareIdentity,
    inventory: impl FnOnce() -> Result<Vec<Registration>>,
) -> Result<()> {
    if identity.source == SoftwareSource::Desktop && !identity.family.is_empty() {
        ensure!(
            !inventory()?
                .iter()
                .any(|registration| family(&registration.name) == Some(identity.family.as_str())),
            Refusal(ReasonCode::Protected)
        );
    }
    Ok(())
}

fn residual_owner(entry: &SoftwareEntry, file: Option<&Path>) -> (bool, Option<String>) {
    if entry.identity.source == SoftwareSource::Store {
        // This value came from the identity-checked PackageManager record, not an IPC argument.
        return (true, Some(entry.software.version.clone()));
    }
    if let Some(app) = dustaway_core::applications::application(&entry.identity.family) {
        let Ok(found) = crate::applications::identify(app) else {
            return (false, None);
        };
        let install = display_path(&found.install)
            .trim_end_matches('\\')
            .to_lowercase();
        let owns = file.is_some_and(|p| {
            display_path(p)
                .to_lowercase()
                .starts_with(&format!("{install}\\"))
                || dingtalk_root_uninstaller(app.id, &found.pin.path, p)
        }) || entry.software.install_location.as_ref().is_some_and(|p| {
            display_path(Path::new(p))
                .trim_end_matches('\\')
                .to_lowercase()
                == install
        });
        return (owns, owns.then_some(found.version));
    }
    let publisher = match entry.identity.family.as_str() {
        "chrome" => "Google LLC",
        "edge" => "Microsoft Corporation",
        "brave" => "Brave Software, Inc.",
        "vivaldi" => "Vivaldi Technologies AS",
        "firefox" => "Mozilla Corporation",
        _ => return (false, None),
    };
    let Some(file) = file else {
        return (false, None);
    };
    let Ok(pin) = PinnedPath::open(file, false, false) else {
        return (false, None);
    };
    if !crate::applications::signed_by(&pin, publisher) {
        return (false, None);
    }
    let executable = dustaway_core::browsers::chromium(&entry.identity.family)
        .map_or("firefox.exe", |product| product.process);
    // Fixed executable name within the uninstaller's bounded installation ancestry.
    // Missing signed binary/version keeps the inspection scope read-only.
    let version = file.ancestors().skip(1).take(4).find_map(|directory| {
        let main = PinnedPath::open(&directory.join(executable), false, false).ok()?;
        if !crate::applications::signed_by(&main, publisher) {
            return None;
        }
        crate::applications::executable_version(&main.path).ok()
    });
    (true, version)
}

/// DingTalk's signed main binary is nested under main/current, while its registered
/// uninstaller sits at the product root. Do not generalize to arbitrary ancestors.
fn dingtalk_root_uninstaller(family: &str, main: &Path, uninstaller: &Path) -> bool {
    if family != "dingtalk" {
        return false;
    }
    let same = |path: &Path, expected: &str| {
        path.file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(expected))
    };
    let Some(current) = main.parent() else {
        return false;
    };
    let Some(main_dir) = current.parent() else {
        return false;
    };
    let Some(product) = main_dir.parent() else {
        return false;
    };
    same(main, "DingTalk.exe")
        && same(current, "current")
        && same(main_dir, "main")
        && display_path(&product.join("uninst.exe"))
            .eq_ignore_ascii_case(&display_path(uninstaller))
}
/// Cache discovery reuses registration/package enumeration without querying uninstall policy.
/// Returned entries are descriptive only and are never passed to software removal.
pub(crate) fn recognition_inventory(
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<SoftwareInventory> {
    inventory_impl(false, cancel)
}
fn inventory_impl(
    removal_policy: bool,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<SoftwareInventory> {
    check_cancel(cancel)?;
    let mut entries = vec![];
    let mut warnings = vec![];
    match desktop_inventory() {
        Ok(items) => entries.extend(items.iter().map(Registration::entry)),
        Err(_) => warnings.push("桌面软件登记未能完整读取，请重试；缺失条目不表示已卸载".into()),
    }
    check_cancel(cancel)?;
    match store_inventory_with_policy(removal_policy, cancel) {
        Ok(items) => {
            entries.extend(items.entries);
            warnings.extend(items.warnings);
        }
        Err(e) if e.is::<ScanCancelled>() => return Err(e),
        Err(_) => warnings.push("商店应用列表读取失败，请重试或打开 Windows 应用设置".into()),
    }
    let current = std::env::current_exe()?;
    for e in &mut entries {
        if e.software
            .install_location
            .as_ref()
            .is_some_and(|p| !p.is_empty() && current.starts_with(p))
            || e.software.name == "拂尘"
            || e.software.name.eq_ignore_ascii_case("dustaway")
        {
            e.software.can_uninstall = false;
            e.software.unavailable_reason = Some("无法从正在运行的拂尘中卸载自身".into());
        }
    }
    Ok(SoftwareInventory { entries, warnings })
}
impl SoftwarePlatform for WindowsSoftware {
    fn restarted_since(&self, time: i64) -> Result<bool> {
        Ok(dustaway_core::now() - (unsafe { GetTickCount64() } / 1000) as i64 > time + 2)
    }
    fn inventory(&self) -> Result<SoftwareInventory> {
        inventory_impl(true, None)
    }
    fn prepare(&self, entry: &SoftwareEntry) -> Result<PreparedSoftware> {
        ensure!(entry.software.can_uninstall, "该软件不能直接卸载");
        let (fingerprint, file, args, stamp) = if entry.identity.source == SoftwareSource::Desktop {
            let (hive, view, key): (String, u32, String) =
                serde_json::from_str(&entry.identity.key)?;
            let raw = registration(&hive, view, &key)?.context("安装登记已变化，请刷新软件列表")?;
            ensure!(
                raw.entry().software == entry.software,
                "软件信息已变化，请刷新后重新确认"
            );
            let (file, args) = raw.action()?;
            let path = PathBuf::from(file);
            is_fixed_ntfs(&path)?;
            let pin = UninstallerPin::open(&path, raw.msi)?;
            (
                serde_json::to_string(&raw)?,
                Some(path),
                args,
                Some(pin.stamp().clone()),
            )
        } else {
            let current = store_inventory()?
                .entries
                .into_iter()
                .find(|e| e.identity == entry.identity)
                .context("应用包已变化，请刷新列表")?;
            ensure!(
                current.software.can_uninstall,
                "Windows 移除策略不允许卸载此应用"
            );
            (entry.identity.key.clone(), None, String::new(), None)
        };
        let (mut ownership, verified_version) = residual_owner(entry, file.as_deref());
        let (store_isolation, store_warning) = if entry.identity.source == SoftwareSource::Store {
            match store_isolation::capture(&entry.identity) {
                Ok(proof) => (Some(proof), None),
                Err(_) => {
                    ownership = false;
                    (None, Some(store_isolation::RETAINED.to_owned()))
                }
            }
        } else {
            (None, None)
        };
        let (roots, mut warnings) = if ownership {
            roots(entry)?
        } else {
            (
                vec![],
                if entry.software.residual_supported && store_warning.is_none() {
                    vec!["无法核对软件签名或安装归属，残留位置已保留".into()]
                } else {
                    vec![]
                },
            )
        };
        warnings.extend(store_warning);
        let residual_scopes = if ownership {
            let (scopes, issues) = residual_scope::capture(
                entry,
                &roots,
                &known_folder(&FOLDERID_LocalAppData)?,
                &known_folder(&FOLDERID_RoamingAppData)?,
            );
            warnings.extend(issues);
            if verified_version.is_none() {
                warnings.push("未能读取已核验软件的版本，残留项目仅保留".into());
            }
            scopes
        } else {
            vec![]
        };
        let context = serde_json::to_string(&ContextData {
            rule_version: RESIDUAL_RULE_VERSION,
            file,
            args,
            stamp,
            roots,
            processes: processes(&entry.identity.family),
            warnings,
            residual_scopes,
            verified_version,
            store_isolation,
        })?;
        Ok(PreparedSoftware {
            entry: entry.clone(),
            fingerprint,
            context,
        })
    }
    fn launch(&self, p: &PreparedSoftware, progress: &mut dyn FnMut(u32)) -> Result<LaunchOutcome> {
        if p.entry.identity.source == SoftwareSource::Store {
            // Recheck immediately before invoking the system removal API, not only at prepare.
            let current = store_inventory()?
                .entries
                .into_iter()
                .find(|e| e.identity == p.entry.identity)
                .context("应用包已变化，请刷新列表")?;
            ensure!(
                current.software.can_uninstall,
                "Windows 移除策略不允许卸载此应用"
            );
            let _com = Com::new()?;
            ensure!(self.installed(&p.entry.identity)?, "应用包已变化");
            return remove_store_package(&p.entry.identity.key, progress);
        }
        let (hive, view, key): (String, u32, String) = serde_json::from_str(&p.entry.identity.key)?;
        let current = registration(&hive, view, &key)?.context("卸载登记已消失")?;
        ensure!(
            serde_json::to_string(&current)? == p.fingerprint,
            "卸载登记已变化，请刷新并重新确认"
        );
        let c: ContextData = serde_json::from_str(&p.context)?;
        ensure!(
            c.rule_version == RESIDUAL_RULE_VERSION,
            "残留规则已变化，请重新检查"
        );
        let path = c.file.context("卸载程序缺失")?;
        let pin = UninstallerPin::open(&path, current.msi)?;
        ensure!(
            Some(pin.stamp()) == c.stamp.as_ref(),
            "卸载程序已变化，请重新确认"
        );
        let file = wide(&path);
        let args = text_wide(&c.args);
        let parent = wide(path.parent().context("卸载位置无效")?);
        let _com = Com::new()?;
        let mut info = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: w!("open"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(args.as_ptr()),
            lpDirectory: PCWSTR(parent.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
            if e.code() == ERROR_CANCELLED.to_hresult() {
                return Ok(LaunchOutcome::Cancelled);
            }
            if e.code() == ERROR_ELEVATION_REQUIRED.to_hresult() {
                info.lpVerb = w!("runas");
                if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
                    if e.code() == ERROR_CANCELLED.to_hresult() {
                        return Ok(LaunchOutcome::Cancelled);
                    }
                    return Err(e.into());
                }
            } else {
                return Err(e.into());
            }
        }
        drop(pin);
        if info.hProcess.is_invalid() {
            return Ok(LaunchOutcome::Untracked);
        }
        let process = Handle(info.hProcess);
        loop {
            let result = unsafe { WaitForSingleObject(process.0, 500) };
            if result == WAIT_OBJECT_0 {
                break;
            }
            ensure!(result == WAIT_TIMEOUT, "无法继续跟踪卸载进程");
        }
        let mut code = 0;
        unsafe {
            GetExitCodeProcess(process.0, &mut code)?;
        }
        if current.msi {
            match code {
                0 => {}
                1602 => return Ok(LaunchOutcome::Cancelled),
                1641 | 3010 => return Ok(LaunchOutcome::RestartRequired),
                _ => bail!("Windows Installer 返回 {code}"),
            }
        }
        // EXE exit codes are vendor-specific. The original registration is always rechecked.
        Ok(LaunchOutcome::Completed)
    }
    fn installed(&self, identity: &SoftwareIdentity) -> Result<bool> {
        if identity.source == SoftwareSource::Store {
            let _com = Com::new()?;
            let manager = PackageManager::new()?;
            let iterator = manager
                .FindPackagesByUserSecurityIdPackageFamilyName(
                    &HSTRING::new(),
                    &HSTRING::from(&identity.family),
                )?
                .First()?;
            if iterator.HasCurrent()? {
                let _ = iterator.Current()?.Id()?.FullName()?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            let (hive, view, key): (String, u32, String) = serde_json::from_str(&identity.key)?;
            Ok(open_key(
                if hive == "user" {
                    HKEY_CURRENT_USER
                } else {
                    HKEY_LOCAL_MACHINE
                },
                &key,
                REG_SAM_FLAGS(view),
            )?
            .is_some())
        }
    }
    fn scan(
        &self,
        p: &PreparedSoftware,
        exclusions: &[String],
        cancel: &AtomicBool,
    ) -> Result<ResidualScanData> {
        ensure!(
            !self.installed(&p.entry.identity)?,
            "软件仍有安装登记，暂不检查残留"
        );
        let c: ContextData = serde_json::from_str(&p.context)?;
        ensure!(
            c.rule_version == RESIDUAL_RULE_VERSION,
            "残留规则已变化，请重新检查"
        );
        store_processes::ensure_named_stopped(&c.processes, Some(cancel))?;
        ensure_no_family_installation(&p.entry.identity)?;
        let mut scan = ResidualScanData {
            records: vec![],
            warnings: c.warnings.clone(),
            supported: p.entry.software.residual_supported,
        };
        if p.entry.identity.source == SoftwareSource::Store
            && let Err(error) = store_isolation::recheck(
                c.store_isolation.as_ref(),
                &p.entry.identity,
                Some(cancel),
            )
        {
            scan.warnings.push(store_isolation::warning(&error, cancel));
            scan.warnings.sort();
            scan.warnings.dedup();
            return Ok(scan);
        }
        let classifier = residual_scope::Classifier::new(&c.residual_scopes, &mut scan.warnings);
        let mut seen = HashSet::new();
        let mut remaining = residual_walk::ENTRY_LIMIT;
        'roots: for (index, root) in c.roots.iter().enumerate() {
            if cancel.load(Ordering::Acquire) {
                scan.warnings.push("检查已停止，结果不完整".into());
                break;
            }
            if remaining == 0 {
                scan.warnings
                    .push(residual_walk::Stop::Limit.warning().into());
                break;
            }
            remaining -= 1;
            let pin = match PinnedPath::open(&root.path, true, false) {
                Ok(p) => p,
                Err(_) => {
                    if root.path.try_exists().unwrap_or(true) {
                        scan.warnings.push("部分残留位置无法读取，已保留".into());
                    }
                    continue;
                }
            };
            if pin.stamp.identity != root.stamp.identity
                || pin.stamp.created != root.stamp.created
                || pin.parent_ids != root.parents
            {
                scan.warnings.push("残留目录身份已变化，已保留".into());
                continue;
            }
            drop(pin);
            let mut pending = vec![root.path.clone()];
            while let Some(path) = pending.pop() {
                if cancel.load(Ordering::Acquire) {
                    scan.warnings
                        .push(residual_walk::Stop::Cancelled.warning().into());
                    break 'roots;
                }
                if excluded(&path, exclusions) {
                    scan.warnings.push("长期排除的位置已保留".into());
                    continue;
                }
                let meta = match std::fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(_) => {
                        scan.warnings.push("部分文件无法读取，已保留".into());
                        continue;
                    }
                };
                let pin = match PinnedPath::open(&path, meta.is_dir(), false) {
                    Ok(p) => p,
                    Err(_) => {
                        scan.warnings.push("占用、链接或受保护项目已保留".into());
                        continue;
                    }
                };
                if !seen.insert((pin.stamp.identity.volume, pin.stamp.identity.index)) {
                    continue;
                }
                let matched = classifier.classify(&path, meta.is_dir());
                let selectable = matched.as_ref().is_some_and(|(_, rule)| {
                    residual_rules::accepted(
                        rule,
                        &p.entry.identity.family,
                        c.verified_version.as_deref().unwrap_or(""),
                    )
                });
                let candidate = ResidualCandidate {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: display_path(&path),
                    category: matched
                        .as_ref()
                        .map_or("data", |(_, rule)| rule.kind.category())
                        .into(),
                    bytes: if meta.is_dir() {
                        "0".into()
                    } else {
                        pin.stamp.size.to_string()
                    },
                    directory: meta.is_dir(),
                    selectable,
                    reason: if selectable {
                        None
                    } else if matched.is_some() {
                        Some("该独立残留规则与软件版本尚未通过真实删除验收，当前只读保留".into())
                    } else {
                        Some("未匹配到卸载前已核验的独立规则，归属或范围不明，已保留".into())
                    },
                };
                let opaque = serde_json::to_string(&ResidualStamp {
                    path: path.clone(),
                    root: index,
                    stamp: pin.stamp.clone(),
                    parents: pin.parent_ids.clone(),
                    scope: matched.map(|(index, _)| index),
                })?;
                let mut stop = None;
                if meta.is_dir() {
                    match std::fs::read_dir(&path) {
                        Ok(children) => {
                            stop = residual_walk::queue_children(
                                children.map(|entry| entry.map(|entry| entry.path())),
                                &mut pending,
                                &mut remaining,
                                cancel,
                                &mut scan.warnings,
                            );
                        }
                        Err(_) => scan.warnings.push("部分目录无法读取，结果不完整".into()),
                    }
                }
                scan.records.push(ResidualRecord { candidate, opaque });
                if let Some(stop) = stop {
                    scan.warnings.push(stop.warning().into());
                    break 'roots;
                }
            }
        }
        scan.warnings.sort();
        scan.warnings.dedup();
        Ok(scan)
    }
    fn delete(
        &self,
        p: &PreparedSoftware,
        r: &ResidualRecord,
        exclusions: &[String],
    ) -> Result<()> {
        delete_residual(p, r, exclusions, residual_scope::authorized)
    }
}

fn delete_residual(
    p: &PreparedSoftware,
    r: &ResidualRecord,
    exclusions: &[String],
    authorize: impl FnOnce(&ContextData, &PreparedSoftware, &ResidualRecord) -> Result<()>,
) -> Result<()> {
    let c: ContextData = serde_json::from_str(&p.context)?;
    ensure!(
        c.rule_version == RESIDUAL_RULE_VERSION,
        "残留规则已变化，请重新检查"
    );
    authorize(&c, p, r)?;
    ensure!(
        !WindowsSoftware.installed(&p.entry.identity)?,
        Refusal(ReasonCode::Protected)
    );
    ensure_no_family_installation(&p.entry.identity)?;
    if p.entry.identity.source == SoftwareSource::Store {
        store_isolation::recheck(c.store_isolation.as_ref(), &p.entry.identity, None)?;
    }
    store_processes::ensure_named_stopped(&c.processes, None)?;
    delete_captured(&c, r, exclusions)
}

fn delete_captured(c: &ContextData, r: &ResidualRecord, exclusions: &[String]) -> Result<()> {
    let snapshot: ResidualStamp = serde_json::from_str(&r.opaque)?;
    let root = c.roots.get(snapshot.root).context("残留范围无效")?;
    ensure!(
        snapshot.path.starts_with(&root.path) && !excluded(&snapshot.path, exclusions),
        Refusal(ReasonCode::Protected)
    );
    let root_pin = PinnedPath::open(&root.path, true, false)?;
    ensure!(
        root_pin.stamp.identity == root.stamp.identity
            && root_pin.stamp.created == root.stamp.created
            && root_pin.parent_ids == root.parents,
        Refusal(ReasonCode::Changed)
    );
    drop(root_pin);
    is_fixed_ntfs(&snapshot.path)?;
    let pin = PinnedPath::open(&snapshot.path, r.candidate.directory, true)?;
    ensure!(
        pin.parent_ids == snapshot.parents,
        Refusal(ReasonCode::Changed)
    );
    if r.candidate.directory {
        ensure!(
            pin.stamp.identity == snapshot.stamp.identity
                && pin.stamp.created == snapshot.stamp.created
                && pin.stamp.attributes == snapshot.stamp.attributes,
            Refusal(ReasonCode::Changed)
        );
        pin.mark_empty_directory_delete()
    } else {
        ensure!(pin.stamp == snapshot.stamp, Refusal(ReasonCode::Changed));
        pin.mark_delete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dingtalk_uninstaller_ownership_uses_only_the_observed_product_layout() {
        let main = Path::new(r"C:\Program Files (x86)\DingDing\main\current\DingTalk.exe");
        let uninstaller = Path::new(r"C:\Program Files (x86)\DingDing\uninst.exe");
        assert!(dingtalk_root_uninstaller("dingtalk", main, uninstaller));
        assert!(dingtalk_root_uninstaller(
            "dingtalk",
            main,
            Path::new(r"c:\program files (x86)\dingding\UNINST.EXE")
        ));
        for other in [
            r"C:\Program Files (x86)\uninst.exe",
            r"C:\Program Files (x86)\DingDingOther\uninst.exe",
            r"C:\Program Files (x86)\DingDing\helper.exe",
            r"D:\DingDing\uninst.exe",
        ] {
            assert!(!dingtalk_root_uninstaller(
                "dingtalk",
                main,
                Path::new(other)
            ));
        }
        for other in [
            r"C:\Program Files (x86)\DingDing\main\current\Other.exe",
            r"C:\Program Files (x86)\DingDing\main\future\DingTalk.exe",
            r"C:\Program Files (x86)\DingDing\other\current\DingTalk.exe",
        ] {
            assert!(!dingtalk_root_uninstaller(
                "dingtalk",
                Path::new(other),
                uninstaller
            ));
        }
        assert!(!dingtalk_root_uninstaller("qq", main, uninstaller));
    }

    #[test]
    fn bare_uninstall_path_requires_an_exact_file_and_no_competing_prefix() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let folder = temp.path().join("Vendor Suite");
        std::fs::create_dir(&folder)?;
        let file = folder.join("uninst.exe");
        std::fs::write(&file, b"test-only, never executed")?;
        let value = file.to_str().context("test path")?;
        assert!(executable_command(value).is_err());
        assert_eq!(
            registered_executable_command(value)?,
            (value.into(), String::new())
        );
        assert!(registered_executable_command(&format!("{value} /S")).is_err());
        assert!(
            registered_executable_command(folder.join("missing.exe").to_str().unwrap()).is_err()
        );
        std::fs::write(temp.path().join("Vendor.exe"), b"competing, never executed")?;
        assert!(registered_executable_command(value).is_err());
        // Explicit quoting remains unambiguous even when a competing prefix exists.
        assert_eq!(
            registered_executable_command(&format!("\"{value}\" /keep-data"))?,
            (value.into(), "/keep-data".into())
        );
        Ok(())
    }

    #[test]
    fn bare_uninstall_path_does_not_enable_wrappers_or_linked_files() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let folder = temp.path().join("Vendor Suite");
        std::fs::create_dir(&folder)?;
        let wrapper = folder.join("powershell.exe");
        std::fs::write(&wrapper, b"never executed")?;
        assert!(registered_executable_command(wrapper.to_str().unwrap()).is_err());
        let file = folder.join("uninst.exe");
        std::fs::write(&file, b"never executed")?;
        std::fs::hard_link(&file, temp.path().join("alias.exe"))?;
        assert!(registered_executable_command(file.to_str().unwrap()).is_err());
        assert!(registered_executable_command("C:\\Vendor Suite\\..\\uninst.exe").is_err());
        assert!(registered_executable_command("\\\\server\\Vendor Suite\\uninst.exe").is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires the dedicated acceptance computer and an ordinary user session"]
    fn missing_fixture_store_package_returns_real_system_error() -> Result<()> {
        ensure!(
            std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
            "wrong host"
        );
        ensure!(!security::elevated()?, "ordinary token required");
        let root = std::env::current_exe()?
            .parent()
            .context("no root")?
            .to_owned();
        ensure!(
            root == Path::new(&std::env::var("LOCALAPPDATA")?)
                .join(r"DustAwayAcceptance\20260930-software"),
            "wrong root"
        );
        ensure!(
            std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
                == "DustAway.Acceptance.20260930.7c7e25e8",
            "wrong marker"
        );
        let key = "DustAway.Acceptance.Missing.7c7e25e8_1.0.0.0_x64__768b01p0s2m14";
        let identity = SoftwareIdentity {
            source: SoftwareSource::Store,
            key: key.into(),
            family: "DustAway.Acceptance.Missing.7c7e25e8_768b01p0s2m14".into(),
        };
        ensure!(
            !WindowsSoftware.installed(&identity)?,
            "fixture must NOT be installed"
        );
        let before = WindowsSoftware.inventory()?;
        ensure!(before.warnings.is_empty(), "incomplete inventory");
        let before: std::collections::BTreeSet<_> =
            before.entries.into_iter().map(|e| e.software.id).collect();
        let error = match remove_store_package(key, &mut |_| {}) {
            Err(error) => error,
            Ok(_) => bail!("missing package was reported successful"),
        };
        let code = error
            .downcast_ref::<windows::core::Error>()
            .context("missing system HRESULT")?
            .code()
            .0 as u32;
        ensure!(code == 0x80073CF1, "unexpected system error: {code:#X}");
        let after = WindowsSoftware.inventory()?;
        ensure!(
            after.warnings.is_empty(),
            "incomplete verification inventory"
        );
        let after: std::collections::BTreeSet<_> =
            after.entries.into_iter().map(|e| e.software.id).collect();
        ensure!(before == after, "installed identities changed");
        let result = serde_json::json!({"result":"passed","scope":"production API boundary; dedicated absent identity","hresult":format!("0x{code:08X}"),"existing_software_targeted":false,"installation_identities_unchanged":true,"ordinary_user":true});
        std::fs::write(
            root.join("store-api-failure-acceptance.json"),
            serde_json::to_vec_pretty(&result)?,
        )?;
        println!("{result}");
        Ok(())
    }
    #[test]
    fn msi_readonly_pin_is_limited_to_the_system_installer() {
        let path = system_installer_path().unwrap();
        let pin = UninstallerPin::open(&path, true).unwrap();
        assert!(pin.stamp().links > 0);
        let root = tempfile::tempdir().unwrap();
        let ordinary = root.path().join("msiexec.exe");
        std::fs::write(&ordinary, b"not a system installer").unwrap();
        assert!(UninstallerPin::open(&ordinary, true).is_err());
        let linked = root.path().join("linked.exe");
        std::fs::hard_link(&ordinary, &linked).unwrap();
        assert!(UninstallerPin::open(&linked, false).is_err());
        assert!(PinnedPath::open(&linked, false, true).is_err());
    }
    #[test]
    fn broken_store_display_resources_fall_back_to_package_identity() {
        let missing_resource =
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80073B1Fu32 as i32));
        assert_eq!(
            package_display_name(Err(missing_resource), "Example.Package"),
            "Example.Package"
        );
        assert_eq!(
            package_display_name(Ok(HSTRING::from("  ")), "Example.Package"),
            "Example.Package"
        );
        assert_eq!(
            package_display_name(Ok(HSTRING::from("Example App")), "Example.Package"),
            "Example App"
        );
    }
    fn registration_fixture() -> Registration {
        Registration {
            hive: "machine".into(),
            view: KEY_WOW64_64KEY.0,
            key: "fixture\\same-name".into(),
            name: "同名测试软件".into(),
            publisher: String::new(),
            version: String::new(),
            command: String::new(),
            location: None,
            size: None,
            date: None,
            msi: false,
            no_remove: false,
        }
    }
    #[test]
    fn discovery_keeps_distinct_views_and_installations_with_the_same_name() {
        let x64 = registration_fixture();
        let mut x86 = x64.clone();
        x86.view = KEY_WOW64_32KEY.0;
        let mut second = x64.clone();
        second.key.push_str("-second");
        let found = deduplicate_registrations(vec![x64.clone(), x86, second, x64]);
        assert_eq!(found.len(), 3);
        let ids: HashSet<_> = found.iter().map(|r| r.entry().software.id).collect();
        assert_eq!(ids.len(), 3);
        let mut shared = registration_fixture();
        shared.hive = "user".into();
        let mut shared32 = shared.clone();
        shared32.view = KEY_WOW64_32KEY.0;
        assert_eq!(deduplicate_registrations(vec![shared, shared32]).len(), 1);
    }
    #[test]
    fn missing_fields_and_prohibited_uninstall_remain_explicit() {
        let mut r = registration_fixture();
        let item = r.entry().software;
        assert!(item.estimated_bytes.is_none());
        assert!(item.install_location.is_none());
        assert!(!item.can_uninstall);
        assert_eq!(item.missing_information.len(), 5);
        assert!(
            item.residual_capabilities
                .iter()
                .all(|c| c.inspection == residual_rules::ResidualInspectionSupport::Unsupported)
        );
        r.name = "WPS Office".into();
        let wps = r.entry().software;
        assert!(wps.residual_supported);
        assert_eq!(
            wps.residual_capabilities[0].inspection,
            residual_rules::ResidualInspectionSupport::FixedLocations
        );
        assert_eq!(
            wps.residual_capabilities[1].inspection,
            residual_rules::ResidualInspectionSupport::FixedLocations
        );
        assert_eq!(
            wps.residual_capabilities[2].inspection,
            residual_rules::ResidualInspectionSupport::FixedLocations
        );
        assert!(!wps.can_uninstall);
        assert!(
            !wps.residual_note
                .contains("支持只读检查缓存、设置与本地资料")
        );
        r.no_remove = true;
        assert!(
            r.entry()
                .software
                .unavailable_reason
                .unwrap()
                .contains("禁止")
        );
        r.no_remove = false;
        r.msi = true;
        assert!(r.action().unwrap_err().to_string().contains("产品代码"));
    }
    fn capture(root: &std::path::Path, path: &std::path::Path) -> (ContextData, ResidualRecord) {
        let root_pin = PinnedPath::open(root, true, false).unwrap();
        let pin = PinnedPath::open(path, path.is_dir(), false).unwrap();
        let c = ContextData {
            rule_version: RESIDUAL_RULE_VERSION,
            file: None,
            args: String::new(),
            stamp: None,
            roots: vec![Root {
                path: root.into(),
                stamp: root_pin.stamp.clone(),
                parents: root_pin.parent_ids.clone(),
            }],
            processes: vec![],
            warnings: vec![],
            residual_scopes: vec![],
            verified_version: None,
            store_isolation: None,
        };
        let r = ResidualRecord {
            candidate: ResidualCandidate {
                id: "fixture-only".into(),
                path: display_path(path),
                category: "data".into(),
                bytes: pin.stamp.size.to_string(),
                directory: path.is_dir(),
                selectable: true,
                reason: None,
            },
            opaque: serde_json::to_string(&ResidualStamp {
                path: path.into(),
                root: 0,
                stamp: pin.stamp.clone(),
                parents: pin.parent_ids.clone(),
                scope: None,
            })
            .unwrap(),
        };
        (c, r)
    }
    #[test]
    fn frozen_residue_checks_changes_exclusions_and_added_children() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("owned");
        std::fs::create_dir(&root).unwrap();
        let file = root.join("settings.json");
        std::fs::write(&file, "original").unwrap();
        let (c, r) = capture(&root, &file);
        assert!(delete_captured(&c, &r, &[display_path(&root)]).is_err());
        assert!(file.exists());
        std::fs::write(&file, "new content that was not confirmed").unwrap();
        assert!(delete_captured(&c, &r, &[]).is_err());
        assert!(file.exists());
        let (c, r) = capture(&root, &file);
        delete_captured(&c, &r, &[]).unwrap();
        assert!(!file.exists());
        let (c, r) = capture(&root, &root);
        std::fs::write(root.join("new-child.txt"), "keep").unwrap();
        assert!(delete_captured(&c, &r, &[]).is_err());
        assert!(root.join("new-child.txt").exists());
    }
    #[test]
    fn replaced_root_and_hardlinks_remain_protected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("owned");
        std::fs::create_dir(&root).unwrap();
        let file = root.join("log");
        std::fs::write(&file, "keep").unwrap();
        let (c, r) = capture(&root, &file);
        std::fs::rename(&root, temp.path().join("old-root")).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::fs::write(&file, "replacement").unwrap();
        assert!(delete_captured(&c, &r, &[]).is_err());
        assert!(file.exists());
        let (c, r) = capture(&root, &file);
        std::fs::hard_link(&file, root.join("shared-link")).unwrap();
        assert!(delete_captured(&c, &r, &[]).is_err());
        assert!(file.exists());
    }
    #[test]
    fn production_acceptance_is_independent_and_closed() {
        for spec in residual_rules::catalog() {
            assert_eq!(
                residual_rules::accepted(&spec, &spec.family, "1.2.3.4"),
                residual_rules::ACCEPTANCES
                    .iter()
                    .any(|record| record.rule_id == spec.id)
            );
        }
        assert!(family("Google Chrome clone").is_none());
    }
    #[test]
    fn residual_reinstall_check_covers_another_registration_and_unreadable_inventory() {
        let r = Registration {
            hive: "machine".into(),
            view: KEY_WOW64_32KEY.0,
            key: "Software\\DustAway\\Acceptance\\DifferentInstallation".into(),
            name: "Mozilla Firefox".into(),
            publisher: "fixture".into(),
            version: "1.0".into(),
            command: String::new(),
            location: None,
            size: None,
            date: None,
            msi: false,
            no_remove: true,
        };
        let original = SoftwareIdentity {
            source: SoftwareSource::Desktop,
            key: "different-original-key".into(),
            family: "firefox".into(),
        };
        assert!(ensure_no_family_installation_with(&original, || Ok(vec![r])).is_err());
        assert!(
            ensure_no_family_installation_with(&original, || bail!("partial registry read"))
                .is_err()
        );
        assert!(ensure_no_family_installation_with(&original, || Ok(vec![])).is_ok());
        let store = SoftwareIdentity {
            source: SoftwareSource::Store,
            ..original
        };
        assert!(
            ensure_no_family_installation_with(&store, || panic!(
                "Store uses its package-family lookup"
            ))
            .is_ok()
        );
    }
}
