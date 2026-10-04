//! Read-only supplement for the NonRemovable property missing from WinRT Package.
//! The command is constant, uses the OS module, and never accepts frontend input.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::Read,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_OUTPUT: u64 = 8 * 1024 * 1024;
const QUERY: &str = r#"
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:PSModulePath=[IO.Path]::Combine([Environment]::SystemDirectory,'WindowsPowerShell\v1.0\Modules')
Import-Module ([IO.Path]::Combine($env:PSModulePath,'Appx\Appx.psd1')) -ErrorAction Stop
$items=@(Appx\Get-AppxPackage -PackageTypeFilter Main -ErrorAction Stop | Microsoft.PowerShell.Utility\Select-Object PackageFullName,NonRemovable)
Microsoft.PowerShell.Utility\ConvertTo-Json -InputObject $items -Compress
"#;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Policy {
    package_full_name: String,
    non_removable: bool,
}

fn parse(bytes: &[u8]) -> Result<HashMap<String, bool>> {
    let items: Vec<Policy> = serde_json::from_slice(bytes)?;
    ensure!(items.len() <= 10000, "应用移除策略数量超过上限");
    let mut result = HashMap::new();
    for item in items {
        ensure!(
            !item.package_full_name.trim().is_empty()
                && result
                    .insert(item.package_full_name, item.non_removable)
                    .is_none(),
            "应用移除策略身份无效或重复"
        );
    }
    Ok(result)
}

pub(super) fn read() -> Result<HashMap<String, bool>> {
    let system = super::system_installer_path()?;
    let system = system.parent().context("Windows 系统目录不可用")?;
    let mut child = Command::new(system.join(r"WindowsPowerShell\v1.0\powershell.exe"))
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            QUERY,
        ])
        .current_dir(system)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW; same ordinary user token.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().context("移除策略输出不可用")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout.take(MAX_OUTPUT + 1).read_to_end(&mut bytes)?;
        Ok::<_, std::io::Error>(bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            other => {
                // Only this read-only metadata helper is stopped. Never an uninstaller.
                let _ = child.kill();
                let _ = child.wait();
                break match other {
                    Err(error) => Err(error.into()),
                    _ => Err(anyhow::anyhow!("读取应用移除策略超时")),
                };
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("移除策略读取失败"))??;
    ensure!(status?.success(), "Windows 未能读取应用移除策略");
    ensure!(bytes.len() as u64 <= MAX_OUTPUT, "应用移除策略输出超过上限");
    parse(&bytes)
}

pub(super) fn refusal(system: bool, non_removable: Option<bool>) -> Option<String> {
    if system {
        Some("Windows 系统组件由系统管理".into())
    } else {
        match non_removable {
            Some(true) => Some("Windows 策略禁止移除此应用".into()),
            Some(false) => None,
            None => Some("无法核验 Windows 移除策略，请刷新后重试".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_system_packages_require_an_explicit_removal_policy() {
        let policies = parse(br#"[{"PackageFullName":"blocked","NonRemovable":true},{"PackageFullName":"allowed","NonRemovable":false}]"#).unwrap();
        assert!(refusal(false, policies.get("blocked").copied()).is_some());
        assert!(refusal(false, policies.get("allowed").copied()).is_none());
        assert!(refusal(false, policies.get("new-version").copied()).is_some());
        assert!(refusal(true, Some(false)).is_some());
        assert!(refusal(false, None).is_some());
    }

    #[test]
    fn missing_malformed_and_conflicting_policy_never_grant_removal() {
        for input in [
            r#"[{"PackageFullName":"pkg"}]"#,
            r#"[{"PackageFullName":"pkg","NonRemovable":null}]"#,
            r#"[{"PackageFullName":"pkg","NonRemovable":"False"}]"#,
            r#"[{"PackageFullName":"pkg","NonRemovable":true},{"PackageFullName":"pkg","NonRemovable":false}]"#,
            r#"[{"PackageFullName":"","NonRemovable":false}]"#,
            "not json",
        ] {
            assert!(parse(input.as_bytes()).is_err(), "{input}");
        }
        assert!(parse(b"[]").unwrap().is_empty());
    }
}
