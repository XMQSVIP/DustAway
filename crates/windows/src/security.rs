use crate::fs::{Handle, text_wide};
use anyhow::{Context, Result, ensure};
use std::{
    mem::size_of,
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::{ERROR_NO_MORE_FILES, HANDLE, HLOCAL, LocalFree},
        Security::{Authorization::*, *},
        System::{Diagnostics::ToolHelp::*, RemoteDesktop::ProcessIdToSessionId, Threading::*},
    },
    core::{PCWSTR, PWSTR},
};

pub fn token(process: HANDLE) -> Result<Handle> {
    let mut handle = HANDLE::default();
    unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &mut handle)?;
    }
    Ok(Handle(handle))
}
pub fn elevated_process(process: HANDLE) -> Result<bool> {
    let token = token(process)?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut len = 0;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )?;
    }
    Ok(elevation.TokenIsElevated != 0)
}
pub fn elevated() -> Result<bool> {
    elevated_process(unsafe { GetCurrentProcess() })
}
pub fn user_sid(process: HANDLE) -> Result<String> {
    let token = token(process)?;
    let mut len = 0;
    unsafe {
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut len);
        ensure!(len > 0 && len <= 65536, "无法读取用户身份");
        // usize alignment satisfies TOKEN_USER's pointer alignment.
        let mut buffer = vec![0usize; (len as usize).div_ceil(size_of::<usize>())];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            len,
            &mut len,
        )?;
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut string = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut string)?;
        let result = string.to_string()?;
        let _ = LocalFree(Some(HLOCAL(string.0.cast())));
        Ok(result)
    }
}
pub fn current_sid() -> Result<String> {
    user_sid(unsafe { GetCurrentProcess() })
}
pub fn process_image(process: HANDLE) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let mut len = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )?;
    }
    Ok(crate::fs::from_native(&buffer[..len as usize]))
}
pub fn process_session(pid: u32) -> Result<u32> {
    let mut session = 0;
    unsafe {
        ProcessIdToSessionId(pid, &mut session)?;
    }
    Ok(session)
}
pub fn process_running(name: &str) -> Result<bool> {
    let current = process_session(std::process::id())?;
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut result = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    loop {
        match result {
            Ok(()) => {}
            Err(e) if e.code() == ERROR_NO_MORE_FILES.to_hresult() => return Ok(false),
            Err(e) => return Err(e.into()),
        }
        let length = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..length]).eq_ignore_ascii_case(name) {
            // If identity cannot be queried, fail closed for this executable name.
            if process_session(entry.th32ProcessID).map_or(true, |s| s == current) {
                return Ok(true);
            }
        }
        result = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
}

/// Developer tools can run scheduled builds in another session of the same user.
/// Conservatively block a matching executable in any session, using one process snapshot.
pub(crate) fn first_running_process_id(names: &[&str]) -> Result<Option<(String, u32)>> {
    Ok(matching_processes(names)?.into_iter().next())
}

pub(crate) fn matching_processes(names: &[&str]) -> Result<Vec<(String, u32)>> {
    let mut found = vec![];
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut result = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    loop {
        match result {
            Ok(()) => {}
            Err(e) if e.code() == ERROR_NO_MORE_FILES.to_hresult() => return Ok(found),
            Err(e) => return Err(e.into()),
        }
        let length = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
        if names
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
        {
            found.push((name, entry.th32ProcessID));
        }
        result = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
}

pub fn application_processes(names: &[&str], install: &Path) -> Result<Vec<String>> {
    application_processes_with_shared(names, &[], install)
}

/// Shared helper names are not application identity. Resolve their image before
/// associating them with an installation; inaccessible images still fail closed.
pub fn application_processes_with_shared(
    names: &[&str],
    shared_names: &[&str],
    install: &Path,
) -> Result<Vec<String>> {
    let session = process_session(std::process::id())?;
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    let root = crate::fs::display_path(install)
        .trim_end_matches('\\')
        .to_lowercase()
        + "\\";
    let mut found = std::collections::BTreeSet::new();
    let mut unknown = false;
    let mut unverified = std::collections::BTreeSet::new();
    loop {
        match next {
            Ok(()) => {}
            Err(e) if e.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
            Err(e) => return Err(e.into()),
        }
        let end = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
        if shared_names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            let image = (|| -> Result<PathBuf> {
                let handle = Handle(unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    )
                }?);
                process_image(handle.0)
            })();
            match shared_component_belongs(image.as_deref().ok(), &root) {
                Some(true) => {
                    found.insert(name);
                }
                Some(false) => {}
                None => {
                    unknown = true;
                    unverified.insert(format!("{name}（PID {}）", entry.th32ProcessID));
                }
            }
            next = unsafe { Process32NextW(snapshot.0, &mut entry) };
            continue;
        }
        let relevant = names.iter().any(|n| n.eq_ignore_ascii_case(&name));
        let current_session = process_session(entry.th32ProcessID);
        if relevant && current_session.is_err() {
            anyhow::bail!(dustaway_core::Refusal(
                dustaway_core::ReasonCode::ProcessUnknown
            ));
        }
        if current_session.is_ok_and(|s| s == session) {
            if relevant {
                found.insert(name);
            } else {
                let image = (|| -> Result<PathBuf> {
                    let handle = Handle(unsafe {
                        OpenProcess(
                            PROCESS_QUERY_LIMITED_INFORMATION,
                            false,
                            entry.th32ProcessID,
                        )
                    }?);
                    process_image(handle.0)
                })();
                match image {
                    Ok(path)
                        if crate::fs::display_path(&path)
                            .to_lowercase()
                            .starts_with(&root) =>
                    {
                        found.insert(name);
                    }
                    Ok(_) => {}
                    // An inaccessible same-session image may be an unlisted background component.
                    // Do not treat it as evidence that the application has fully exited.
                    Err(_) => unknown = true,
                }
            }
        }
        next = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    let result = finish_application_processes(found.into_iter().collect(), unknown);
    if shared_names.is_empty() {
        return result;
    }
    result.with_context(|| {
        if unverified.is_empty() {
            "后台进程状态未通过核验，已保留附件".into()
        } else {
            format!(
                "无法核验共享后台进程 {} 的归属，已保留附件；请核对相关程序并正常退出后重试",
                unverified
                    .into_iter()
                    .take(8)
                    .collect::<Vec<_>>()
                    .join("、")
            )
        }
    })
}

fn shared_component_belongs(image: Option<&Path>, normalized_root: &str) -> Option<bool> {
    image.map(|path| {
        crate::fs::display_path(path)
            .to_lowercase()
            .starts_with(normalized_root)
    })
}

fn finish_application_processes(found: Vec<String>, unknown: bool) -> Result<Vec<String>> {
    ensure!(
        !found.is_empty() || !unknown,
        dustaway_core::Refusal(dustaway_core::ReasonCode::ProcessUnknown)
    );
    Ok(found)
}

pub struct Descriptor(pub PSECURITY_DESCRIPTOR);
impl Descriptor {
    pub fn from_sddl(sddl: &str) -> Result<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let text = text_wide(sddl);
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(text.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )?;
        }
        Ok(Self(descriptor))
    }
    pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0.0,
            bInheritHandle: false.into(),
        }
    }
}
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0.0)));
        }
    }
}

pub fn owner_and_dacl(path: &Path) -> Result<String> {
    let name = crate::fs::wide(path);
    let flags = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
    let mut raw = PSECURITY_DESCRIPTOR::default();
    unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(name.as_ptr()),
            SE_FILE_OBJECT,
            flags,
            None,
            None,
            None,
            None,
            &mut raw,
        )
        .ok()?;
        let descriptor = Descriptor(raw);
        let mut text = PWSTR::null();
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor.0,
            SDDL_REVISION_1,
            flags,
            &mut text,
            None,
        )?;
        let result = text.to_string()?;
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        Ok(result)
    }
}

/// Read ownership while the caller pins the path against rename/replacement.
pub(crate) fn path_owner_sid(path: &Path) -> Result<String> {
    let name = crate::fs::wide(path);
    let mut raw = PSECURITY_DESCRIPTOR::default();
    let mut owner = PSID::default();
    unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(name.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            None,
            None,
            &mut raw,
        )
        .ok()?;
        let _descriptor = Descriptor(raw);
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(owner, &mut text)?;
        let result = text.to_string();
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        Ok(result?)
    }
}

#[cfg(test)]
mod application_tests {
    use super::*;
    #[test]
    fn shared_helper_requires_install_identity_and_unknown_still_blocks() {
        let root = "c:\\apps\\qqnt\\";
        assert_eq!(
            shared_component_belongs(
                Some(Path::new(r"C:\Apps\QQNT\bin\crashpad_handler.exe")),
                root
            ),
            Some(true)
        );
        assert_eq!(
            shared_component_belongs(
                Some(Path::new(r"C:\Apps\Weixin\crashpad_handler.exe")),
                root
            ),
            Some(false)
        );
        assert_eq!(
            shared_component_belongs(
                Some(Path::new(r"C:\Apps\QQNT-other\crashpad_handler.exe")),
                root
            ),
            Some(false)
        );
        assert_eq!(shared_component_belongs(None, root), None);
        assert!(finish_application_processes(vec![], true).is_err());
    }
    #[test]
    fn background_components_and_unqueryable_processes_never_mean_exited() {
        assert_eq!(
            finish_application_processes(vec!["background.exe".into()], false).unwrap(),
            vec!["background.exe"]
        );
        assert!(finish_application_processes(vec![], true).is_err());
        assert!(
            !finish_application_processes(vec!["QQ.exe".into()], true)
                .unwrap()
                .is_empty()
        );
        assert!(
            finish_application_processes(vec![], false)
                .unwrap()
                .is_empty()
        );
    }
}
