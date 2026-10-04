//! Only built-in, machine-registered Microsoft handlers; never merged HKCR activation.
use crate::{fs::*, platform, security};
use anyhow::{Context, Result, ensure};
use dustaway_core::Scope;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{E_ABORT, FreeLibrary, HMODULE, S_OK},
        System::{Com::*, LibraryLoader::*, Registry::*},
        UI::{LegacyWindowsEnvironmentFeatures::*, Shell::FOLDERID_LocalAppData},
    },
    core::{GUID, HRESULT, Interface, PCWSTR, PWSTR, implement, s},
};

pub(crate) struct Spec {
    pub id: &'static str,
    pub title: &'static str,
    pub key: &'static str,
    pub clsid: u128,
    pub scope: Scope,
    pub impact: &'static str,
}
const GENERIC: u128 = 0xc0e13e61_0cc6_11d1_bbb6_0060978b2ae6;
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        id: "windows.thumbnail",
        title: "缩略图缓存",
        key: "Thumbnail Cache",
        clsid: 0x889900c3_59f3_4c2f_ae21_a409ea01e605,
        scope: Scope::User,
        impact: "文件预览缩略图将在使用时重新生成。",
    },
    Spec {
        id: "windows.shader",
        title: "DirectX 着色器缓存",
        key: "D3D Shader Cache",
        clsid: 0xd8d133cd_3f26_402f_86da_90b710751c2c,
        scope: Scope::User,
        impact: "图形应用下次使用时可能重新编译着色器。",
    },
    Spec {
        id: "windows.delivery",
        title: "传递优化缓存",
        key: "Delivery Optimization Files",
        clsid: 0x4057c1ad_a51f_40bb_b960_22888ceb9812,
        scope: Scope::System,
        impact: "必要的更新内容可能重新下载，由 Windows 判断可移除范围。",
    },
    Spec {
        id: "windows.update",
        title: "Windows 更新与过期组件",
        key: "Update Cleanup",
        clsid: 0x606b3777_3051_401f_974a_e66aca82a3a3,
        scope: Scope::System,
        impact: "由 Windows 整理过期更新组件，可能需要重启；部分旧更新的回退资料会减少。",
    },
    Spec {
        id: "windows.memory_dump",
        title: "系统错误内存转储",
        key: "System error memory dump files",
        clsid: GENERIC,
        scope: Scope::System,
        impact: "删除后无法使用这些转储诊断过去的系统故障。",
    },
    Spec {
        id: "windows.minidump",
        title: "系统错误小型转储",
        key: "System error minidump files",
        clsid: GENERIC,
        scope: Scope::System,
        impact: "删除后无法使用这些转储诊断过去的系统故障。",
    },
    Spec {
        id: "windows.error_reports",
        title: "Windows 错误报告",
        key: "Windows Error Reporting Files",
        clsid: GENERIC,
        scope: Scope::System,
        impact: "移除 Windows 判断可清理的错误报告，历史排障资料会减少。",
    },
    Spec {
        id: "windows.setup_logs",
        title: "Windows 安装日志",
        key: "Setup Log Files",
        clsid: GENERIC,
        scope: Scope::System,
        impact: "移除 Windows 判断可清理的安装日志，历史排障资料会减少。",
    },
    Spec {
        id: "windows.upgrade_logs",
        title: "Windows 升级日志",
        key: "Windows Upgrade Log Files",
        clsid: 0x84e04a55_2d42_4909_86e3_62fd11483e8b,
        scope: Scope::System,
        impact: "移除可清理的升级日志，不调用旧 Windows 安装或回退清理项。",
    },
    Spec {
        id: "windows.defender_support",
        title: "Defender 支持文件",
        key: "Windows Defender",
        clsid: GENERIC,
        scope: Scope::System,
        impact: "仅由 Windows 处理 LocalCopy 与 Support；不包含扫描历史或病毒库更新备份。",
    },
];
pub(crate) fn spec(id: &str) -> Result<&'static Spec> {
    SPECS
        .iter()
        .find(|s| s.id == id)
        .context("不支持的 Windows 维护项目")
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
fn key(base: HKEY, path: &str) -> Result<Key> {
    let mut handle = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            base,
            PCWSTR(text_wide(path).as_ptr()),
            None,
            KEY_READ | KEY_WOW64_64KEY,
            &mut handle,
        )
        .ok()?;
    }
    Ok(Key(handle))
}
fn value(key: &Key, name: &str) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; 65536];
    let mut length = buffer.len() as u32;
    unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(text_wide(name).as_ptr()),
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&mut length),
        )
        .ok()?;
    }
    ensure!(length <= 65536, "注册数据过大");
    buffer.truncate(length as usize);
    Ok(buffer)
}
fn string(key: &Key, name: &str) -> Result<String> {
    let mut kind = REG_VALUE_TYPE::default();
    unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(text_wide(name).as_ptr()),
            None,
            Some(&mut kind),
            None,
            None,
        )
        .ok()?;
    }
    ensure!(
        kind == REG_SZ || kind == REG_EXPAND_SZ,
        "注册字符串类型无效"
    );
    let bytes = value(key, name)?;
    ensure!(bytes.len() % 2 == 0, "注册字符串无效");
    Ok(String::from_utf16(
        &bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|c| *c != 0)
            .collect::<Vec<_>>(),
    )?)
}
fn expanded(text: &str) -> Result<String> {
    let source = text_wide(text);
    let mut buffer = vec![0u16; 32768];
    let count = unsafe {
        windows::Win32::System::Environment::ExpandEnvironmentStringsW(
            PCWSTR(source.as_ptr()),
            Some(&mut buffer),
        )
    } as usize;
    ensure!(count > 0 && count <= buffer.len(), "环境路径无效");
    Ok(String::from_utf16(&buffer[..count - 1])?)
}
fn dword(key: &Key, name: &str) -> Result<u32> {
    let mut kind = REG_VALUE_TYPE::default();
    let mut size = 4;
    let mut number = 0u32;
    unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(text_wide(name).as_ptr()),
            None,
            Some(&mut kind),
            Some((&mut number as *mut u32).cast()),
            Some(&mut size),
        )
        .ok()?;
    }
    ensure!(kind == REG_DWORD && size == 4, "注册整数类型无效");
    Ok(number)
}
fn verify_scope(spec: &Spec, registration: &Key, windows: &std::path::Path) -> Result<()> {
    let win = display_path(windows).to_lowercase();
    let data = display_path(&platform::known_folder(
        &windows::Win32::UI::Shell::FOLDERID_ProgramData,
    )?)
    .to_lowercase();
    for command in ["CleanupString", "PreCleanupString", "PostCleanupString"] {
        ensure!(
            value(registration, command).is_err(),
            "此官方处理器配置了外部清理命令，当前不支持在应用内执行"
        );
    }
    if spec.clsid == GENERIC {
        let (folder, pattern, flags, age) = match spec.id {
            "windows.memory_dump" => (win.clone(), "*.dmp", 572, None),
            "windows.minidump" => (format!(r"{win}\minidump"), "*.dmp", 637, None),
            "windows.error_reports" => (format!(r"{data}\microsoft\windows\wer"), "*.*", 125, None),
            "windows.setup_logs" => (
                win,
                "setup*.log|setup*.old|setuplog.txt|winnt32.log",
                32,
                Some(30),
            ),
            "windows.defender_support" => (
                format!(
                    r"{data}\microsoft\windows defender\localcopy|{data}\microsoft\windows defender\support"
                ),
                "*.*",
                61,
                None,
            ),
            _ => anyhow::bail!("未核验的通用维护范围"),
        };
        ensure!(
            expanded(&string(registration, "Folder")?)?
                .trim_end_matches('\\')
                .eq_ignore_ascii_case(&folder)
                && string(registration, "FileList")?.eq_ignore_ascii_case(pattern)
                && dword(registration, "Flags")? == flags,
            "Windows 官方处理范围不在内置允许列表中"
        );
        ensure!(
            match age {
                Some(days) => dword(registration, "LastAccess")? == days,
                None => value(registration, "LastAccess").is_err(),
            },
            "官方保留期限与已核验配置不符"
        );
    } else {
        ensure!(
            ["Folder", "FileList", "LastAccess"]
                .iter()
                .all(|n| value(registration, n).is_err()),
            "处理器包含非标准文件范围配置"
        );
        if spec.id == "windows.upgrade_logs" {
            ensure!(
                string(registration, "SetupDirectories")?.eq_ignore_ascii_case(r"Windows\Panther")
                    && dword(registration, "ValidIfUpgrade")? == 1,
                "升级日志范围发生变化"
            );
        }
    }
    Ok(())
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Library(HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}
#[implement(IEmptyVolumeCacheCallBack)]
struct Callback {
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
}
impl IEmptyVolumeCacheCallBack_Impl for Callback_Impl {
    fn ScanProgress(&self, bytes: u64, _: u32, _: &PCWSTR) -> windows::core::Result<()> {
        self.progress.store(bytes, Ordering::Release);
        self.check()
    }
    fn PurgeProgress(&self, bytes: u64, _: u64, _: u32, _: &PCWSTR) -> windows::core::Result<()> {
        self.progress.store(bytes, Ordering::Release);
        self.check()
    }
}
impl Callback_Impl {
    fn check(&self) -> windows::core::Result<()> {
        if self.cancel.load(Ordering::Acquire) {
            Err(E_ABORT.into())
        } else {
            Ok(())
        }
    }
}

pub(crate) struct Handler {
    api: Option<IEmptyVolumeCache>,
    _library: Library,
    _pin: PinnedPath,
    _key: Key,
    callback: IEmptyVolumeCacheCallBack,
    pub fingerprint: String,
    pub volume: String,
    _apartment: Apartment,
}
impl Drop for Handler {
    fn drop(&mut self) {
        if let Some(api) = self.api.take() {
            unsafe {
                let _ = api.Deactivate();
            }
            drop(api);
        }
    }
}
impl Handler {
    pub fn open(
        spec: &Spec,
        consent: bool,
        cancel: Arc<AtomicBool>,
        progress: Arc<AtomicU64>,
    ) -> Result<Self> {
        ensure!(
            security::elevated()? == (spec.scope == Scope::System),
            "Windows 维护权限域不匹配"
        );
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        let apartment = Apartment;
        let registration = key(
            HKEY_LOCAL_MACHINE,
            &format!(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\VolumeCaches\{}",
                spec.key
            ),
        )?;
        let registered = string(&registration, "")?;
        let clsid = GUID::from_u128(spec.clsid);
        let guid_text = registered
            .strip_prefix('{')
            .and_then(|v| v.strip_suffix('}'))
            .unwrap_or(&registered);
        ensure!(
            GUID::try_from(guid_text).context("Windows 清理处理器 CLSID 格式无效")? == clsid,
            "Windows 清理处理器身份不匹配"
        );
        let class_path = format!(r"SOFTWARE\Classes\CLSID\{}\InprocServer32", registered);
        ensure!(
            key(HKEY_CURRENT_USER, &class_path).is_err(),
            "检测到用户级处理器覆盖"
        );
        let class = key(HKEY_LOCAL_MACHINE, &class_path)?;
        let raw_dll = string(&class, "")?;
        let windows = platform::windows_dir()?;
        let dll = PathBuf::from(expanded(&raw_dll)?);
        let full = extended(&dll)?;
        let expected_dll = match spec.id {
            "windows.thumbnail" => "thumbcache.dll",
            "windows.shader" => "d3dscache.dll",
            "windows.delivery" => "domgmt.dll",
            "windows.update" => "scavengeui.dll",
            "windows.upgrade_logs" => "setupcln.dll",
            _ if spec.clsid == GENERIC => "dataclen.dll",
            _ => anyhow::bail!("未知维护组件"),
        };
        ensure!(
            dll.file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(expected_dll)),
            "Windows 维护 DLL 身份不匹配"
        );
        let full_text = full.to_string_lossy().to_lowercase();
        ensure!(
            ["System32", "WinSxS"]
                .iter()
                .any(
                    |name| extended(&windows.join(name)).is_ok_and(|root| full_text
                        .starts_with(&format!("{}\\", root.to_string_lossy().to_lowercase())))
                ),
            "处理器不在受信任的 Windows 目录"
        );
        let pin = PinnedPath::observe(&dll, false, None)?;
        ensure!(
            crate::windows_signature::microsoft(&pin),
            "Windows 处理器签名未通过核验"
        );
        verify_scope(spec, &registration, &windows)?;
        let config = [
            "Folder",
            "FileList",
            "Flags",
            "LastAccess",
            "CleanupString",
            "PreCleanupString",
            "PostCleanupString",
            "SetupDirectories",
            "ValidIfUpgrade",
        ]
        .map(|name| (name, value(&registration, name).ok()));
        let base = if spec.scope == Scope::User {
            platform::known_folder(&FOLDERID_LocalAppData)?
        } else {
            windows
        };
        let base = display_path(&base);
        ensure!(
            base.as_bytes().get(1) == Some(&b':'),
            "维护范围必须是本地磁盘"
        );
        let volume = format!("{}\\", &base[..2]);
        let volume_identity = platform::volume_target(std::path::Path::new(&volume))?;
        let fingerprint = serde_json::to_string(&(
            spec.id,
            registered,
            raw_dll,
            &pin.stamp,
            &config,
            (&volume_identity.key, &volume_identity.opaque, &volume),
            security::current_sid()?,
        ))?;
        let library = Library(unsafe {
            LoadLibraryExW(
                PCWSTR(wide(&dll).as_ptr()),
                None,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )?
        });
        let entry = unsafe { GetProcAddress(library.0, s!("DllGetClassObject")) }
            .context("处理器缺少类工厂")?;
        type GetClass = unsafe extern "system" fn(
            *const GUID,
            *const GUID,
            *mut *mut std::ffi::c_void,
        ) -> HRESULT;
        let factory: IClassFactory = unsafe {
            let get: GetClass = std::mem::transmute(entry);
            let mut raw = std::ptr::null_mut();
            get(&clsid, &IClassFactory::IID, &mut raw).ok()?;
            IClassFactory::from_raw(raw)
        };
        let api: IEmptyVolumeCache = unsafe { factory.CreateInstance(None)? };
        let mut title = PWSTR::null();
        let mut description = PWSTR::null();
        let mut button = PWSTR::null();
        let mut flags = if consent {
            EVCF_USERCONSENTOBTAINED
        } else {
            EMPTY_VOLUME_CACHE_FLAGS(0)
        };
        let initialized = unsafe {
            if let Ok(ex) = api.cast::<IEmptyVolumeCache2>() {
                ex.InitializeEx(
                    registration.0,
                    PCWSTR(text_wide(&volume).as_ptr()),
                    PCWSTR(text_wide(spec.key).as_ptr()),
                    &mut title,
                    &mut description,
                    &mut button,
                    &mut flags,
                )
            } else {
                api.Initialize(
                    registration.0,
                    PCWSTR(text_wide(&volume).as_ptr()),
                    &mut title,
                    &mut description,
                    &mut flags,
                )
            }
        };
        for text in [title, description, button] {
            unsafe {
                CoTaskMemFree(Some(text.0.cast()));
            }
        }
        initialized?;
        ensure!(
            flags.0 & EVCF_REMOVEFROMLIST.0 == 0,
            "此 Windows 维护项目当前不可用"
        );
        let callback: IEmptyVolumeCacheCallBack = Callback { cancel, progress }.into();
        Ok(Self {
            api: Some(api),
            _library: library,
            _pin: pin,
            _key: registration,
            callback,
            fingerprint,
            volume,
            _apartment: apartment,
        })
    }
    pub fn estimate(&self) -> Result<Option<u64>> {
        let api = self.api.as_ref().unwrap();
        let mut bytes = 0;
        let result = unsafe {
            (api.vtable().GetSpaceUsed)(api.as_raw(), &mut bytes, self.callback.as_raw())
        };
        result.ok()?;
        Ok((result == S_OK && bytes != u64::MAX).then_some(bytes))
    }
    pub fn purge(&self, bytes: u64) -> Result<()> {
        let api = self.api.as_ref().unwrap();
        let result = unsafe { (api.vtable().Purge)(api.as_raw(), bytes, self.callback.as_raw()) };
        result.ok()?;
        ensure!(result == S_OK, "Windows 未确认维护成功");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callbacks_report_progress_and_request_abort_without_claiming_completion() {
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let callback: IEmptyVolumeCacheCallBack = Callback {
            cancel: cancel.clone(),
            progress: progress.clone(),
        }
        .into();
        unsafe {
            callback.ScanProgress(123, 0, PCWSTR::null()).unwrap();
        }
        assert_eq!(progress.load(Ordering::Acquire), 123);
        cancel.store(true, Ordering::Release);
        assert_eq!(
            unsafe { callback.PurgeProgress(10, 100, 0, PCWSTR::null()) }
                .unwrap_err()
                .code(),
            E_ABORT
        );
        assert_eq!(progress.load(Ordering::Acquire), 10);
    }
    #[test]
    fn operation_ids_and_permissions_are_closed() {
        assert!(spec("{C0E13E61-0CC6-11d1-BBB6-0060978B2AE6}").is_err());
        assert!(spec("C:\\Windows\\Temp").is_err());
        assert!(spec("windows.defender_history").is_err());
        for item in SPECS {
            assert_eq!(
                item.scope == Scope::User,
                matches!(item.id, "windows.thumbnail" | "windows.shader")
            );
        }
    }
}
