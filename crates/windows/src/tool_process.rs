//! Conservative process association. Names alone are not proof of cache occupancy.
use crate::{fs::*, security, uv_cache::run_bounded};
use anyhow::Result;
use dustaway_core::{ApiError, BlockReason, ReasonCode};
use std::{
    os::windows::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    time::Duration,
};
use windows::Win32::{Foundation::FILETIME, System::Threading::*, UI::Shell::CommandLineToArgvW};

pub(crate) fn names(rule: &str) -> &'static [&'static str] {
    match rule {
        "dev.uv" => &["uv.exe", "python.exe", "pythonw.exe"],
        "dev.yarn" | "dev.pnpm" => &["node.exe", "pnpm.exe", "yarn.exe"],
        "dev.go_build" => &["go.exe", "compile.exe", "link.exe"],
        "dev.ccache" => &[
            "ccache.exe",
            "sccache.exe",
            "cl.exe",
            "clang.exe",
            "clang-cl.exe",
            "gcc.exe",
            "g++.exe",
        ],
        _ => &[],
    }
}
#[derive(Clone)]
struct Evidence {
    name: String,
    pid: u32,
    stable: bool,
    image_matches: bool,
    generic: bool,
    entry_matches: bool,
}
fn classify(rows: &[Evidence]) -> Option<BlockReason> {
    let confirmed = rows
        .iter()
        .filter(|p| p.stable && p.image_matches && (!p.generic || p.entry_matches))
        .collect::<Vec<_>>();
    let chosen = if confirmed.is_empty() {
        rows.iter().collect::<Vec<_>>()
    } else {
        confirmed
    };
    if chosen.is_empty() {
        return None;
    }
    let label = chosen
        .iter()
        .take(8)
        .map(|p| format!("{}（PID {}）", p.name, p.pid))
        .collect::<Vec<_>>()
        .join("、");
    if chosen
        .iter()
        .all(|p| p.stable && p.image_matches && (!p.generic || p.entry_matches))
    {
        Some(BlockReason::new(
            ReasonCode::ApplicationRunning,
            format!("已确认相关工具 {label} 运行中；保存工作并正常退出相关任务后重新检查"),
        ))
    } else {
        Some(BlockReason::new(
            ReasonCode::ProcessUnknown,
            format!(
                "检测到 {label}，运行状态或与工具的关联无法确认；这不代表已确认缓存被占用。请核对相关任务并重新检查"
            ),
        ))
    }
}
fn creation(handle: windows::Win32::Foundation::HANDLE) -> Result<u64> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user)?;
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}
fn entry_matches(command: &str, expected: &Path) -> bool {
    let wide = text_wide(command);
    let mut count = 0;
    let argv = unsafe { CommandLineToArgvW(windows::core::PCWSTR(wide.as_ptr()), &mut count) };
    if argv.is_null() {
        return false;
    }
    let result = unsafe { std::slice::from_raw_parts(argv, count as usize) }
        .iter()
        .skip(1)
        .any(|arg| {
            let Ok(value) = (unsafe { arg.to_string() }) else {
                return false;
            };
            Path::new(&value).is_absolute()
                && extended(Path::new(&value))
                    .is_ok_and(|p| display_path(&p).eq_ignore_ascii_case(&display_path(expected)))
        });
    unsafe {
        let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            argv.cast(),
        )));
    }
    result
}
fn command_line(pid: u32, cancel: &AtomicBool) -> Result<String> {
    // PID is from Toolhelp, never from IPC. No user text is interpolated into this fixed query.
    let script = format!(
        "$ErrorActionPreference='Stop'; $p=Get-CimInstance Win32_Process -Filter 'ProcessId={pid}'; if ($null -eq $p) {{ exit 3 }}; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); ConvertTo-Json -InputObject ([string]$p.CommandLine) -Compress"
    );
    let mut command = Command::new(
        crate::platform::windows_dir()?.join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    );
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000);
    let (ok, output) = run_bounded(&mut command, cancel, Duration::from_secs(10))?;
    anyhow::ensure!(ok, "进程关联查询未完成");
    // Never return command text as an error, log, or persisted field.
    serde_json::from_str(&output).map_err(|_| anyhow::anyhow!("进程关联查询无法读取"))
}
pub(crate) fn blocked(
    rule: &str,
    exe: Option<&Path>,
    entry: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<Option<BlockReason>> {
    let processes = security::matching_processes(names(rule))?;
    let mut evidence = vec![];
    for (name, pid) in processes {
        check_cancel(Some(cancel))?;
        let generic = ["node.exe", "python.exe", "pythonw.exe"]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n));
        let mut row = Evidence {
            name,
            pid,
            generic,
            stable: false,
            image_matches: false,
            entry_matches: false,
        };
        let inspect = (|| -> Result<()> {
            let handle = Handle(unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    false,
                    pid,
                )?
            });
            let born = creation(handle.0)?;
            let image = security::process_image(handle.0)?;
            PinnedPath::open(&image, false, false)?;
            // A compiler/linker name may belong to another toolchain. Only the
            // verified executable identity supports a positive association.
            row.image_matches = exe
                .is_some_and(|exe| display_path(&image).eq_ignore_ascii_case(&display_path(exe)));
            if generic
                && row.image_matches
                && let Some(entry) = entry
            {
                row.entry_matches =
                    command_line(pid, cancel).is_ok_and(|text| entry_matches(&text, entry));
            }
            let verify = Handle(unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    false,
                    pid,
                )?
            });
            row.stable = creation(verify.0)? == born
                && unsafe { WaitForSingleObject(handle.0, 0) }
                    == windows::Win32::Foundation::WAIT_TIMEOUT;
            Ok(())
        })();
        if inspect.is_err() {
            row.stable = false;
        }
        evidence.push(row);
    }
    check_cancel(Some(cancel))?;
    Ok(classify(&evidence))
}
pub(crate) fn guard(
    rule: &str,
    exe: Option<&Path>,
    entry: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<()> {
    match blocked(rule, exe, entry, cancel) {
        Ok(Some(reason)) => Err(ApiError::new(
            if reason.code == ReasonCode::ApplicationRunning {
                "application_running"
            } else {
                "process_unknown"
            },
            reason.message,
        )
        .into()),
        Ok(None) => Ok(()),
        Err(error) => Err(ApiError::new(
            "process_unknown",
            format!("运行状态无法确认：{error:#}；请重新检查"),
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_runtime_never_proves_occupancy_and_confirmed_match_has_priority() {
        let mut node = Evidence {
            name: "node.exe".into(),
            pid: 12,
            stable: true,
            image_matches: true,
            generic: true,
            entry_matches: false,
        };
        assert_eq!(
            classify(&[node.clone()]).unwrap().code,
            ReasonCode::ProcessUnknown
        );
        node.entry_matches = true;
        assert_eq!(
            classify(&[node.clone()]).unwrap().code,
            ReasonCode::ApplicationRunning
        );
        node.stable = false; // exited, inaccessible, or PID replaced while querying
        assert_eq!(
            classify(&[node.clone()]).unwrap().code,
            ReasonCode::ProcessUnknown
        );
        let tool = Evidence {
            name: "uv.exe".into(),
            generic: false,
            stable: true,
            ..node.clone()
        };
        let reason = classify(&[node, tool]).unwrap();
        assert_eq!(reason.code, ReasonCode::ApplicationRunning);
        assert!(reason.message.contains("PID 12"));
        assert!(!reason.message.contains("node.exe"));
        let other_linker = Evidence {
            name: "link.exe".into(),
            pid: 18,
            stable: true,
            image_matches: false,
            generic: false,
            entry_matches: false,
        };
        assert_eq!(
            classify(&[other_linker]).unwrap().code,
            ReasonCode::ProcessUnknown
        );
        assert!(classify(&[]).is_none());
    }
    #[test]
    fn entry_matching_requires_an_exact_absolute_argument() {
        let entry = Path::new(r"C:\Tools\yarn\bin\yarn.js");
        assert!(entry_matches(
            r#"node.exe "C:\Tools\yarn\bin\yarn.js" cache clean"#,
            entry
        ));
        assert!(!entry_matches(
            r#"node.exe other.js --label=C:\Tools\yarn\bin\yarn.js"#,
            entry
        ));
        assert!(!entry_matches("node.exe yarn.js", entry));
    }
}
