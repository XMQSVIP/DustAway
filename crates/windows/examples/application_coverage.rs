//! Read-only host audit. Writes JSON to stdout; never prepares or executes a cleanup.
use anyhow::Result;
use dustaway_core::{
    Engine, Scope,
    software::{SoftwarePlatform, SoftwareSource},
};
use dustaway_windows::{WindowsPlatform, software::WindowsSoftware};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn main() -> Result<()> {
    let inventory = WindowsSoftware.inventory()?;
    let mut audit = Vec::new();
    for e in &inventory.entries {
        let software = &e.software;
        let lower = software.name.to_lowercase();
        let new_rules: Vec<_> = dustaway_core::app_cache::catalog()
            .iter()
            .filter(|s| {
                dustaway_core::app_cache::registered_name_matches(
                    &software.name,
                    &s.aliases.iter().map(String::as_str).collect::<Vec<_>>(),
                )
            })
            .map(|s| s.id.as_str())
            .collect();
        let existing: Vec<_> = dustaway_core::applications::APPLICATIONS
            .iter()
            .filter(|a| lower.contains(&a.title.to_lowercase()))
            .map(|a| a.id)
            .collect();
        let existing_family = [
            "chrome",
            "firefox",
            "edge",
            "python",
            "go programming",
            "rustup",
            "node",
            "nvm",
            "visual studio",
            "pycharm",
            "wps",
            "微信",
            "腾讯qq",
            "钉钉",
            "飞书",
            "百度",
            "夸克",
            "迅雷",
        ]
        .iter()
        .any(|a| lower.contains(a));
        let system = software
            .unavailable_reason
            .as_deref()
            .is_some_and(|s| s.contains("系统") || s.contains("策略禁止"));
        let component = system
            || [
                "redistributable",
                "runtime",
                "driver",
                "驱动",
                "connector",
                "sdk addon",
                "languageexperiencepack",
                "framework",
                "kb500",
            ]
            .iter()
            .any(|a| lower.contains(a));
        audit.push(serde_json::json!({"name": software.name, "version": software.version, "source": software.source,
            "conclusion": if component {"系统或开发组件；没有独立垃圾许可"} else if !new_rules.is_empty() {"新增适配，按规则核对扫描结果"} else if !existing.is_empty() || existing_family {"已有规则，按实际位置核对"} else {"用途待确认；尚无固定缓存适配，不代表没有缓存"},
            "new_rules":new_rules,"existing_apps":existing,"inventory_note": software.unavailable_reason,"store": software.source == SoftwareSource::Store}));
    }
    // Inventory directory names only, never recursively search arbitrary "Cache" folders.
    let mut data_directories = Vec::new();
    for (folder, env) in [("local", "LOCALAPPDATA"), ("roaming", "APPDATA")] {
        let base = std::path::PathBuf::from(std::env::var_os(env).unwrap());
        for entry in std::fs::read_dir(base)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let ids: Vec<_> = dustaway_core::app_cache::catalog()
                .iter()
                .filter(|s| {
                    s.folder == folder
                        && s.base
                            .split('/')
                            .next()
                            .is_some_and(|p| p.eq_ignore_ascii_case(&name))
                })
                .map(|s| s.id.as_str())
                .collect();
            let existing = [
                "Microsoft",
                "Google",
                "Mozilla",
                "BraveSoftware",
                "Vivaldi",
                "Tencent",
                "DingTalk",
                "WeChat",
                "Kingsoft",
                "BaiduNetdisk",
                "Quark",
                "Thunder",
                "pip",
                "uv",
                "npm-cache",
                "pnpm",
                "go-build",
                "NuGet",
                "Yarn",
                "ccache",
                "Cursor",
                "Temp",
                "CrashDumps",
            ]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n));
            let component = [
                "Packages",
                "ConnectedDevicesPlatform",
                "Comms",
                "D3DSCache",
                "Diagnostics",
                "Publishers",
                "Programs",
            ]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n));
            data_directories.push(serde_json::json!({"folder":folder,"name":name,"new_rules":ids,
                "conclusion": if !ids.is_empty(){"新增固定位置适配；候选及读取情况见规则表"}else if existing{"已有部分固定位置规则；不代表整个目录可清理"}else if component{"系统或安装数据；不将整个目录视作垃圾"}else{"用途待确认；未开放缓存规则，保留全部内容"}}));
        }
    }
    data_directories.sort_by_key(|v| format!("{}{}", v["folder"], v["name"]));
    let platform = Arc::new(WindowsPlatform::new(Scope::User)?);
    let engine = Engine::open(platform, Scope::User, std::path::Path::new(":memory:"))?;
    let rules = dustaway_core::rules::for_scope(Scope::User)?
        .into_iter()
        .filter(|r| {
            r.root == "application_cache"
                || r.app_id.is_some()
                || r.id.starts_with("dev.vscode")
                || r.id == "dev.jetbrains_logs"
        })
        .map(|r| r.id)
        .collect();
    let session = engine.start_scan(rules)?;
    let started = Instant::now();
    while engine.is_busy() {
        if started.elapsed() > Duration::from_secs(600) {
            engine.cancel();
            anyhow::bail!("audit scan timed out; no cleanup submitted");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"session":session,"elapsed_ms":started.elapsed().as_millis(),"inventory_warnings":inventory.warnings,"software":audit,"data_directories":data_directories,"summary":engine.summary()})
        )?
    );
    Ok(())
}
