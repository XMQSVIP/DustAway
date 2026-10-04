//! Fixed installed-tool adapters. Never invoke a shell, Corepack, or project scripts.
use crate::{fs::*, platform::known_folder, uv_cache::run_bounded};
use anyhow::{Context, Result, bail, ensure};
use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    time::Duration,
};
use windows::Win32::UI::Shell::{FOLDERID_Profile, FOLDERID_RoamingAppData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Pnpm,
    Go,
    Yarn,
    Ccache,
}
impl Action {
    pub fn parse(id: &str) -> Result<Self> {
        match id {
            "dev.pnpm" => Ok(Self::Pnpm),
            "dev.go_build" => Ok(Self::Go),
            "dev.yarn" => Ok(Self::Yarn),
            "dev.ccache" => Ok(Self::Ccache),
            _ => bail!("不支持的整理操作"),
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Pnpm => "整理 pnpm 闲置缓存",
            Self::Go => "清空 Go 构建缓存",
            Self::Yarn => "清空 Yarn Classic 下载缓存",
            Self::Ccache => "整理 ccache 旧编译缓存",
        }
    }
    pub fn command_label(self) -> &'static str {
        match self {
            Self::Pnpm => "pnpm store prune",
            Self::Go => "go clean -cache",
            Self::Yarn => "yarn cache clean",
            Self::Ccache => "ccache --evict-older-than 30d",
        }
    }
}
pub(crate) struct Tool {
    pub action: Action,
    pub exe: PathBuf,
    pub entry: Option<PathBuf>,
    pub version: String,
}
fn search_exe(name: &str) -> Result<PathBuf> {
    let mut dirs: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|p| p.is_absolute())
        .collect();
    dirs.push(known_folder(&FOLDERID_Profile)?.join(".local/bin"));
    let cwd = std::env::current_dir()?;
    for dir in dirs {
        if dir == cwd {
            continue;
        }
        let path = dir.join(name);
        if path.is_file() {
            let pin = PinnedPath::open(&path, false, false)?;
            ensure!(pin.stamp.size > 1024, "工具文件过小，无法核验");
            return Ok(path);
        }
    }
    bail!(dustaway_core::ApiError::new(
        "tool_missing",
        format!("未找到已安装的 {name}，保留缓存；不会自动下载或安装")
    ))
}
pub(crate) fn supported(action: Action, version: &str) -> bool {
    let version = version.trim();
    let numeric = match action {
        Action::Go => version
            .strip_prefix("go version go")
            .and_then(|s| s.split_whitespace().next()),
        Action::Ccache => version
            .lines()
            .next()
            .and_then(|s| s.strip_prefix("ccache version ")),
        _ => Some(version),
    };
    let Some(v) = numeric else { return false };
    let parts: Vec<_> = v.split('.').collect();
    if parts.len() < 2
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    true
}
fn command_supported(action: Action, help: &str) -> bool {
    // Query help, never trial-run a destructive command to detect support.
    // Layout validation and fixed command construction remain independent gates.
    let words: Vec<_> = help
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .filter(|word| !word.is_empty())
        .collect();
    let required: &[&str] = match action {
        Action::Pnpm => &["store", "prune", "path"],
        Action::Yarn => &[
            "cache",
            "clean",
            "dir",
            "--cache-folder",
            "--offline",
            "--ignore-scripts",
            "--non-interactive",
        ],
        Action::Go => &["clean", "-cache", "build", "cache"],
        Action::Ccache => &["--evict-older-than", "--get-config"],
    };
    required.iter().all(|word| words.contains(word))
}
impl Tool {
    pub fn discover(action: Action, cwd: &Path, cancel: &AtomicBool) -> Result<Self> {
        let (exe, entry) = match action {
            Action::Go => (search_exe("go.exe")?, None),
            Action::Ccache => (search_exe("ccache.exe")?, None),
            Action::Pnpm | Action::Yarn => {
                let name = if action == Action::Pnpm {
                    "pnpm"
                } else {
                    "yarn"
                };
                // A real native pnpm installation is supported; shell/Corepack shims are not.
                if action == Action::Pnpm
                    && let Ok(exe) = search_exe("pnpm.exe")
                {
                    (exe, None)
                } else {
                    let node = search_exe("node.exe")?;
                    let relative = if action == Action::Pnpm {
                        "pnpm/bin/pnpm.cjs"
                    } else {
                        "yarn/bin/yarn.js"
                    };
                    let roots = [
                        node.parent().context("工具位置无效")?.join("node_modules"),
                        known_folder(&FOLDERID_RoamingAppData)?.join("npm/node_modules"),
                    ];
                    let entry = roots
                        .iter()
                        .map(|r| r.join(relative))
                        .find(|p| p.is_file())
                        .ok_or_else(|| {
                            dustaway_core::ApiError::new(
                                "tool_missing",
                                "未找到真实工具入口；不执行 Corepack 或命令包装脚本",
                            )
                        })?;
                    let package = entry
                        .parent()
                        .and_then(Path::parent)
                        .context("工具包位置无效")?
                        .join("package.json");
                    let pin = PinnedPath::open(&package, false, false)?;
                    ensure!(pin.stamp.size < 1024 * 1024, "工具包元数据过大");
                    let metadata: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&package)?)?;
                    ensure!(metadata["name"].as_str() == Some(name), "工具包身份不匹配");
                    PinnedPath::open(&entry, false, false)?;
                    (node, Some(entry))
                }
            }
        };
        let mut tool = Self {
            action,
            exe,
            entry,
            version: String::new(),
        };
        let args = if action == Action::Go {
            vec!["version"]
        } else {
            vec!["--version"]
        };
        tool.version = tool.probe(cwd, &args, cancel)?;
        ensure!(
            supported(action, &tool.version),
            dustaway_core::ApiError::new(
                "application_unverified",
                "无法核验工具版本信息，已保留缓存"
            )
        );
        let help_args: &[&str] = match action {
            Action::Pnpm => &["help", "store"],
            Action::Yarn => &["--ignore-scripts", "--non-interactive", "help", "cache"],
            Action::Go => &["help", "clean"],
            Action::Ccache => &["--help"],
        };
        let help = tool.probe(cwd, help_args, cancel)?;
        ensure!(
            command_supported(action, &help),
            dustaway_core::ApiError::new(
                "unsupported_layout",
                "当前工具未能核实所需命令及参数，已保留缓存"
            )
        );
        Ok(tool)
    }
    pub fn base(&self, cwd: &Path) -> Command {
        let mut command = Command::new(&self.exe);
        if let Some(entry) = &self.entry {
            command.arg(entry);
        }
        command
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH")
            .env("COREPACK_ENABLE_NETWORK", "0")
            .env("npm_config_manage_package_manager_versions", "false")
            .env("npm_config_ignore_pnpmfile", "true")
            .env("npm_config_ignore_scripts", "true")
            .env("npm_config_update_notifier", "false")
            .env("YARN_IGNORE_PATH", "1")
            .env("GOTOOLCHAIN", "local")
            .env("GOWORK", "off")
            .env("GOFLAGS", "")
            .env("GOCACHEPROG", "");
        command
    }
    fn probe(&self, cwd: &Path, args: &[&str], cancel: &AtomicBool) -> Result<String> {
        let identity = crate::tool_discovery::identity(&self.exe, self.entry.as_deref())?;
        let (ok, output) = run_bounded(self.base(cwd).args(args), cancel, Duration::from_secs(10))?;
        ensure!(
            identity == crate::tool_discovery::identity(&self.exe, self.entry.as_deref())?,
            "工具在查询期间变化"
        );
        ensure!(ok, "工具路径查询失败：{output}");
        Ok(output.trim().to_owned())
    }
    pub fn target(&self, cwd: &Path, cancel: &AtomicBool) -> Result<PathBuf> {
        let args: &[&str] = match self.action {
            Action::Pnpm => &["store", "path"],
            Action::Go => &["env", "GOCACHE"],
            Action::Yarn => &["--ignore-scripts", "--non-interactive", "cache", "dir"],
            Action::Ccache => &["--get-config", "cache_dir"],
        };
        let output = self.probe(cwd, args, cancel)?;
        // Banners, relative paths and multi-line output are not a target authorization.
        ensure!(
            output.lines().count() == 1 && Path::new(&output).is_absolute(),
            "工具未返回唯一绝对缓存位置"
        );
        let root = extended(Path::new(&output))?;
        ensure!(root.components().count() >= 4, "缓存位置过于宽泛");
        Ok(root)
    }
    pub fn validate_layout(&self, root: &Path, cancel: &AtomicBool) -> Result<()> {
        check_cancel(Some(cancel))?;
        is_fixed_ntfs(root)?;
        PinnedPath::open(root, true, false)?;
        match self.action {
            Action::Pnpm => {
                ensure!(
                    root.file_name().is_some_and(|s| s == "v10")
                        && root.join("files").is_dir()
                        && root.join("index").is_dir(),
                    "仅支持 pnpm 常规 v10 存储布局"
                );
                ensure!(
                    !root.join("links").exists() && !root.join("projects").exists(),
                    "全局虚拟存储尚未适配，已保留"
                );
            }
            Action::Go => {
                let marker = root.join("README");
                let pin = PinnedPath::open(&marker, false, false)?;
                ensure!(
                    pin.stamp.size < 8192 && std::fs::read_to_string(marker)?.contains("Go build"),
                    "缺少 Go 构建缓存标记"
                );
            }
            Action::Yarn => {
                ensure!(
                    root.file_name().is_some_and(|n| n == "v6"),
                    "仅支持 Yarn Classic v6 全局缓存"
                );
                for entry in std::fs::read_dir(root)? {
                    check_cancel(Some(cancel))?;
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    ensure!(
                        name == ".tmp"
                            || (name.starts_with("npm-")
                                && entry.path().join("node_modules").is_dir()),
                        "缓存中有未识别的 Yarn 布局"
                    );
                }
            }
            Action::Ccache => {
                ensure!(
                    root.join("ccache.conf").is_file() || root.join("0/stats").is_file(),
                    "缺少可核验的 ccache 布局"
                );
            }
        };
        Ok(())
    }
    pub fn cleanup(&self, root: &Path) -> Command {
        // Some native tools do not support Win32 extended path syntax in their own parsers.
        // Identity is pinned using the extended path; only its display spelling goes to the tool.
        let display_root = PathBuf::from(display_path(root));
        let root = display_root.as_path();
        let cwd = known_folder(&FOLDERID_Profile).expect("current user profile");
        let mut command = self.base(&cwd);
        match self.action {
            Action::Pnpm => {
                command
                    .args(["store", "prune", "--store-dir"])
                    .arg(root.parent().unwrap())
                    .env("npm_config_offline", "true");
            }
            Action::Go => {
                command
                    .args(["clean", "-cache"])
                    .env("GOCACHE", root)
                    .env("GOENV", "off")
                    .env("GOPROXY", "off");
            }
            Action::Yarn => {
                command
                    .args([
                        "--offline",
                        "--ignore-scripts",
                        "--non-interactive",
                        "--cache-folder",
                    ])
                    .arg(root.parent().unwrap())
                    .args(["cache", "clean"]);
            }
            Action::Ccache => {
                command
                    .args(["--evict-older-than", "30d"])
                    .env("CCACHE_DIR", root);
            }
        };
        command
    }
}

#[cfg(test)]
#[path = "maintenance_acceptance.rs"]
mod acceptance;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_are_observed_while_actions_and_command_capabilities_are_checked() {
        for (id, good, other) in [
            ("dev.pnpm", "10.30.3", "11.0.0"),
            ("dev.yarn", "1.22.22", "4.0.0"),
            (
                "dev.go_build",
                "go version go1.24.2 windows/amd64",
                "go version go1.20.0 windows/amd64",
            ),
            (
                "dev.ccache",
                "ccache version 4.14.0",
                "ccache version 4.6.0",
            ),
        ] {
            let action = Action::parse(id).unwrap();
            assert!(supported(action, good));
            assert!(supported(action, other));
            assert!(!supported(action, "malformed"));
        }
        assert!(Action::parse("dev.bun").is_err());
        assert!(Action::parse("cmd.exe").is_err());
        for (action, help) in [
            (Action::Pnpm, "pnpm store: path, prune"),
            (
                Action::Yarn,
                "yarn cache [ls|list|clean|dir] --cache-folder --offline --ignore-scripts --non-interactive",
            ),
            (Action::Go, "go clean: -cache removes the build cache"),
            (Action::Ccache, "--evict-older-than AGE --get-config KEY"),
        ] {
            assert!(command_supported(action, help));
            assert!(!command_supported(action, "unknown command"));
            assert!(!command_supported(
                action,
                "--evict-older-than-missing --get-config"
            ));
        }
    }
    #[test]
    fn fixed_commands_never_include_project_or_package_removal() {
        let root = Path::new("C:\\fixture\\cache\\v10");
        for action in [Action::Pnpm, Action::Go, Action::Yarn, Action::Ccache] {
            let tool = Tool {
                action,
                exe: "C:\\tools\\tool.exe".into(),
                entry: None,
                version: String::new(),
            };
            let command = tool.cleanup(root);
            let args: Vec<_> = command
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            assert!(!args.iter().any(|a| matches!(
                a.as_str(),
                "-modcache" | "--clear" | "--force" | "install" | "remove"
            )));
            match action {
                Action::Go => assert_eq!(args, ["clean", "-cache"]),
                Action::Ccache => assert_eq!(args, ["--evict-older-than", "30d"]),
                Action::Pnpm => assert_eq!(&args[..3], ["store", "prune", "--store-dir"]),
                Action::Yarn => assert_eq!(&args[args.len() - 2..], ["cache", "clean"]),
            }
        }
    }
}
