//! Current-user software processes, across sessions. Package checks also cover
//! unpackaged code under the captured installation location. No process control.
#[cfg(test)]
mod acceptance;
use super::*;
use windows::Win32::{
    Globalization::{CSTR_EQUAL, CompareStringOrdinal},
    Security::{Authorization::ConvertSidToStringSidW, IsValidSid, PSID},
    Storage::Packaging::Appx::GetPackageFamilyName,
    System::RemoteDesktop::{WTS_PROCESS_INFOW, WTSEnumerateProcessesW, WTSFreeMemory},
};

const LIMIT: u32 = 100_000;

struct ProcessList(*mut WTS_PROCESS_INFOW);
impl Drop for ProcessList {
    fn drop(&mut self) {
        unsafe { WTSFreeMemory(self.0.cast()) };
    }
}
fn sid_string(sid: PSID) -> Result<String> {
    ensure!(unsafe { IsValidSid(sid).as_bool() }, "无效进程所属用户");
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut text)? };
    let result = unsafe { text.to_string() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
    }
    Ok(result?)
}

struct OwnedProcess {
    pid: u32,
    #[cfg(test)]
    name: Option<String>,
    #[cfg(test)]
    session: u32,
}

fn select_owned(
    entries: &[WTS_PROCESS_INFOW],
    user_sid: &str,
    own_pid: u32,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<OwnedProcess>> {
    let mut seen = HashSet::new();
    let mut own = vec![];
    for entry in entries {
        check_cancel(cancel)?;
        // Filter by token owner, never by session: the same user may still have
        // software running in another RDP, console, or background session.
        if !entry.pUserSid.0.is_null() && sid_string(entry.pUserSid)? == user_sid {
            ensure!(
                entry.ProcessId != 0 && seen.insert(entry.ProcessId),
                "进程清单身份无效"
            );
            own.push(OwnedProcess {
                pid: entry.ProcessId,
                #[cfg(test)]
                session: entry.SessionId,
                #[cfg(test)]
                name: if entry.pProcessName.is_null() {
                    None
                } else {
                    unsafe { entry.pProcessName.to_string() }
                        .ok()
                        .filter(|s| !s.is_empty())
                },
            });
        }
    }
    ensure!(seen.contains(&own_pid), "进程清单缺少当前用户信息");
    Ok(own)
}

fn owned_processes(user_sid: &str, cancel: Option<&AtomicBool>) -> Result<Vec<OwnedProcess>> {
    let mut list = ProcessList(std::ptr::null_mut());
    let mut count = 0;
    unsafe { WTSEnumerateProcessesW(None, 0, 1, &mut list.0, &mut count)? };
    ensure!(
        count > 0 && count <= LIMIT && !list.0.is_null(),
        "进程清单不完整"
    );
    select_owned(
        unsafe { std::slice::from_raw_parts(list.0, count as usize) },
        user_sid,
        std::process::id(),
        cancel,
    )
}

fn owned_pids(user_sid: &str, cancel: Option<&AtomicBool>) -> Result<Vec<u32>> {
    Ok(owned_processes(user_sid, cancel)?
        .into_iter()
        .map(|p| p.pid)
        .collect())
}

pub(super) fn check_cancel(cancel: Option<&AtomicBool>) -> Result<()> {
    ensure!(
        !cancel.is_some_and(|value| value.load(Ordering::Acquire)),
        "检查已停止，结果不完整"
    );
    Ok(())
}

fn process_family(handle: HANDLE) -> Result<Option<String>> {
    let mut length = 0;
    let code = unsafe { GetPackageFamilyName(handle, &mut length, None) };
    if code == APPMODEL_ERROR_NO_PACKAGE {
        return Ok(None);
    }
    ensure!(
        code == ERROR_INSUFFICIENT_BUFFER && (2..=256).contains(&length),
        Refusal(ReasonCode::ProcessUnknown)
    );
    let mut text = vec![0u16; length as usize];
    unsafe { GetPackageFamilyName(handle, &mut length, Some(PWSTR(text.as_mut_ptr()))) }.ok()?;
    ensure!(
        length >= 2 && length as usize <= text.len() && text[length as usize - 1] == 0,
        Refusal(ReasonCode::ProcessUnknown)
    );
    let family = String::from_utf16(&text[..length as usize - 1])?;
    ensure!(
        store_isolation::valid_family(&family),
        Refusal(ReasonCode::ProcessUnknown)
    );
    Ok(Some(family))
}

fn under_install(image: &Path, install: &Path) -> Result<bool> {
    let image = wide(&extended(image)?);
    let mut root = wide(&extended(install)?);
    root.pop(); // remove NUL, then require the next path component boundary
    root.push(b'\\' as u16);
    if image.len() <= root.len() {
        return Ok(false);
    }
    let compared = unsafe { CompareStringOrdinal(&image[..root.len()], &root, true) };
    ensure!(compared.0 != 0, Refusal(ReasonCode::ProcessUnknown));
    Ok(compared == CSTR_EQUAL)
}

struct ObservedProcess {
    family: Option<String>,
    image: PathBuf,
}
fn observe(pid: u32, user_sid: &str) -> Result<Option<ObservedProcess>> {
    // These rights allow metadata and exit observation, never termination or memory access.
    let process = match unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
    } {
        Ok(handle) => Handle(handle),
        // A process disappearing between enumeration and opening is no longer running.
        Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    match unsafe { WaitForSingleObject(process.0, 0) } {
        WAIT_OBJECT_0 => return Ok(None),
        WAIT_TIMEOUT => {}
        _ => bail!(Refusal(ReasonCode::ProcessUnknown)),
    }
    let result = (|| -> Result<ObservedProcess> {
        // Recheck the handle's user after opening to reject stale/PID-reused metadata.
        ensure!(
            security::user_sid(process.0)? == user_sid,
            Refusal(ReasonCode::ProcessUnknown)
        );
        Ok(ObservedProcess {
            family: process_family(process.0)?,
            image: security::process_image(process.0)?,
        })
    })();
    match unsafe { WaitForSingleObject(process.0, 0) } {
        WAIT_OBJECT_0 => Ok(None),
        WAIT_TIMEOUT => result.map(Some),
        _ => bail!(Refusal(ReasonCode::ProcessUnknown)),
    }
}

fn belongs(process: &ObservedProcess, family: &str, install: &Path) -> Result<bool> {
    if process
        .family
        .as_ref()
        .is_some_and(|value| value.eq_ignore_ascii_case(family))
    {
        return Ok(true);
    }
    under_install(&process.image, install)
}

fn finish(running: bool, unknown: bool) -> Result<()> {
    ensure!(!running, Refusal(ReasonCode::ApplicationRunning));
    ensure!(!unknown, Refusal(ReasonCode::ProcessUnknown));
    Ok(())
}

pub(super) fn ensure_named_stopped(names: &[String], cancel: Option<&AtomicBool>) -> Result<()> {
    check_cancel(cancel)?;
    if names.is_empty() {
        return Ok(());
    }
    let sid = security::current_sid()?;
    let mut unknown = false;
    for pid in matching_named_pids(names, cancel)? {
        check_cancel(cancel)?;
        match named_process_belongs(pid, &sid, names) {
            Ok(true) => return finish(true, unknown),
            Ok(_) => {}
            Err(_) => unknown = true,
        }
    }
    check_cancel(cancel)?;
    finish(false, unknown)
}

fn matching_named_pids(names: &[String], cancel: Option<&AtomicBool>) -> Result<Vec<u32>> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    // WTS can omit inaccessible processes, including the same user's SSH process.
    // Toolhelp supplies the full name/PID snapshot. Never filter it by session or
    // treat failure to query a matching process's user as proof it belongs elsewhere.
    check_cancel(cancel)?;
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    let mut result = vec![];
    let mut count = 0;
    loop {
        check_cancel(cancel)?;
        match next {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
            Err(error) => return Err(error.into()),
        }
        count += 1;
        ensure!(count <= LIMIT, Refusal(ReasonCode::ProcessUnknown));
        let end = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
        if names
            .iter()
            .any(|expected| expected.eq_ignore_ascii_case(&name))
        {
            ensure!(
                entry.th32ProcessID != 0,
                Refusal(ReasonCode::ProcessUnknown)
            );
            result.push(entry.th32ProcessID);
        }
        next = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    Ok(result)
}

fn named_process_belongs(pid: u32, sid: &str, names: &[String]) -> Result<bool> {
    let process = match unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
    } {
        Ok(handle) => Handle(handle),
        Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if unsafe { WaitForSingleObject(process.0, 0) } == WAIT_OBJECT_0 {
        return Ok(false);
    }
    let observed = (|| -> Result<bool> {
        if security::user_sid(process.0)? != sid {
            return Ok(false);
        }
        let image = security::process_image(process.0)?;
        let name = image
            .file_name()
            .and_then(|s| s.to_str())
            .context(Refusal(ReasonCode::ProcessUnknown))?;
        Ok(names
            .iter()
            .any(|expected| expected.eq_ignore_ascii_case(name)))
    })();
    match unsafe { WaitForSingleObject(process.0, 0) } {
        WAIT_OBJECT_0 => Ok(false),
        WAIT_TIMEOUT => observed,
        _ => bail!(Refusal(ReasonCode::ProcessUnknown)),
    }
}

pub(super) fn ensure_stopped(
    family: &str,
    install: &Path,
    user_sid: &str,
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    check_cancel(cancel)?;
    ensure!(
        store_isolation::valid_family(family),
        Refusal(ReasonCode::ProcessUnknown)
    );
    ensure!(
        extended(install)?.components().count() > 2,
        Refusal(ReasonCode::ProcessUnknown)
    );
    let pids = owned_pids(user_sid, cancel)?;
    let mut unknown = false;
    for pid in pids {
        check_cancel(cancel)?;
        match observe(pid, user_sid).and_then(|process| {
            process
                .map(|process| belongs(&process, family, install))
                .transpose()
        }) {
            Ok(Some(true)) => return finish(true, unknown),
            Ok(_) => {}
            Err(_) => unknown = true,
        }
    }
    check_cancel(cancel)?;
    finish(false, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_snapshot_rechecks_owner_and_image_without_session_filtering() -> Result<()> {
        let pid = std::process::id();
        let exe = std::env::current_exe()?;
        let name = exe
            .file_name()
            .and_then(|s| s.to_str())
            .context("test image name")?
            .to_uppercase();
        let names = vec![name];
        assert!(matching_named_pids(&names, None)?.contains(&pid));
        assert!(named_process_belongs(
            pid,
            &security::current_sid()?,
            &names
        )?);
        assert!(!named_process_belongs(pid, "S-1-5-21-0-0-0-9999", &names)?);
        assert!(!named_process_belongs(
            pid,
            &security::current_sid()?,
            &["not-the-test-process.exe".into()]
        )?);
        assert!(
            ensure_named_stopped(&names, None)
                .unwrap_err()
                .downcast_ref::<Refusal>()
                .is_some_and(|r| r.0 == ReasonCode::ApplicationRunning)
        );
        Ok(())
    }
    #[test]
    fn package_identity_and_installation_boundary_are_independent() -> Result<()> {
        let install = Path::new("C:/WindowsApps/Fixture_1.0.0.0_x64__publisher");
        let family = "Fixture_publisher";
        let other_image = PathBuf::from("C:/Elsewhere/background.exe");
        assert!(belongs(
            &ObservedProcess {
                family: Some("fixture_PUBLISHER".into()),
                image: other_image.clone()
            },
            family,
            install
        )?);
        assert!(belongs(
            &ObservedProcess {
                family: None,
                image: install.join("nested/child.exe")
            },
            family,
            install
        )?);
        for image in [
            other_image,
            PathBuf::from("C:/WindowsApps/Fixture_1.0.0.0_x64__publisher-neighbor/app.exe"),
        ] {
            assert!(!belongs(
                &ObservedProcess {
                    family: Some("Other_publisher".into()),
                    image
                },
                family,
                install
            )?);
        }
        assert!(under_install(
            Path::new("c:/软件/ÄPP/app.exe"),
            Path::new("C:/软件/äpp")
        )?);
        assert!(finish(false, false).is_ok());
        assert!(finish(false, true).is_err());
        assert!(
            finish(true, true)
                .unwrap_err()
                .downcast_ref::<Refusal>()
                .is_some_and(|r| r.0 == ReasonCode::ApplicationRunning)
        );
        Ok(())
    }
    #[test]
    fn stopped_check_rejects_cancel_before_querying_or_accepting_empty_results() {
        let cancel = AtomicBool::new(true);
        let result = ensure_stopped(
            "Fixture_publisher",
            Path::new("C:/fixture"),
            "unused",
            Some(&cancel),
        );
        assert_eq!(result.unwrap_err().to_string(), "检查已停止，结果不完整");
        assert!(ensure_named_stopped(&[], Some(&cancel)).is_err());
    }

    #[test]
    fn user_selection_includes_other_sessions_and_excludes_other_accounts() -> Result<()> {
        use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
        struct Sid(PSID);
        impl Drop for Sid {
            fn drop(&mut self) {
                unsafe {
                    let _ = LocalFree(Some(HLOCAL(self.0.0)));
                }
            }
        }
        let mut owner = Sid(PSID::default());
        let mut other = Sid(PSID::default());
        unsafe {
            ConvertStringSidToSidW(w!("S-1-5-21-100-200-300-1001"), &mut owner.0)?;
            ConvertStringSidToSidW(w!("S-1-5-21-100-200-300-1002"), &mut other.0)?;
        }
        let mut name = text_wide("firefox.exe");
        let name_pointer = PWSTR(name.as_mut_ptr());
        let make = |pid, session, sid| WTS_PROCESS_INFOW {
            ProcessId: pid,
            SessionId: session,
            pUserSid: sid,
            pProcessName: name_pointer,
        };
        let entries = [
            make(10, 2, owner.0),
            make(20, 0, owner.0),
            make(30, 9, owner.0),
            make(40, 2, other.0),
            make(50, 2, PSID::default()),
        ];
        let selected = select_owned(&entries, &sid_string(owner.0)?, 10, None)?;
        assert_eq!(
            selected.iter().map(|p| p.pid).collect::<Vec<_>>(),
            [10, 20, 30]
        );
        assert!(
            selected
                .iter()
                .all(|p| p.name.as_deref() == Some("firefox.exe"))
        );
        assert!(select_owned(&entries, &sid_string(owner.0)?, 99, None).is_err());
        Ok(())
    }
}
