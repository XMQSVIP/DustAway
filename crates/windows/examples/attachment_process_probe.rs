//! Read-only diagnostics using the same process APIs as the attachment guard.
use anyhow::Result;
use dustaway_windows::{fs::Handle, security};
use std::mem::size_of;
use windows::Win32::{
    Foundation::ERROR_NO_MORE_FILES,
    Security::{GetTokenInformation, TokenSessionId},
    System::{Diagnostics::ToolHelp::*, Threading::*},
};

fn main() -> Result<()> {
    let own = security::process_session(std::process::id())?;
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    let mut rows = vec![];
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
        let session = security::process_session(entry.th32ProcessID);
        let image = (|| -> Result<_> {
            let handle = Handle(unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    entry.th32ProcessID,
                )?
            });
            security::process_image(handle.0)
        })();
        let token_session = (|| -> Result<u32> {
            let handle = Handle(unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    entry.th32ProcessID,
                )?
            });
            let token = security::token(handle.0)?;
            let mut value = 0u32;
            let mut len = 0;
            unsafe {
                GetTokenInformation(
                    token.0,
                    TokenSessionId,
                    Some((&mut value as *mut u32).cast()),
                    4,
                    &mut len,
                )?;
            }
            Ok(value)
        })();
        let relevant = ["qq", "txplatform", "crashpad"]
            .iter()
            .any(|n| name.to_lowercase().contains(n));
        if relevant || session.as_ref().is_ok_and(|s| *s == own) && image.is_err() {
            rows.push(serde_json::json!({"pid":entry.th32ProcessID,"name":name,"session":session.as_ref().ok(),"session_error":session.err().map(|e|e.to_string()),"image":image.as_ref().ok(),"image_error":image.err().map(|e|e.to_string()),"token_session":token_session.as_ref().ok(),"token_error":token_session.err().map(|e|e.to_string())}));
        }
        next = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    let guard = security::application_processes_with_shared(
        &[
            "QQ.exe",
            "QQNT.exe",
            "QQGuild.exe",
            "QQApp.exe",
            "QQExternal.exe",
            "QQProtect.exe",
            "TXPlatform.exe",
            "QQCrashReport.exe",
            "QQUpdate.exe",
        ],
        &["crashpad_handler.exe", "crashpad_handler_extension.exe"],
        std::path::Path::new(r"C:\Program Files\Tencent\QQNT-DustAway"),
    );
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"elevated":security::elevated()?,"session":own,"rows":rows,"guard":guard.as_ref().ok(),"guard_error":guard.err().map(|e|format!("{e:#}"))})
        )?
    );
    Ok(())
}
