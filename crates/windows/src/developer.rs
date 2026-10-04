use crate::{fs::*, security};
use anyhow::Result;
use dustaway_core::{BlockReason, ReasonCode, developer::DeveloperRule};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub(crate) fn roots(
    local: &Path,
    roaming: &Path,
    home: &Path,
    spec: &DeveloperRule,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    let found = standard_roots(local, roaming, home, spec, cancel, issue)?;
    #[cfg(not(test))]
    let found = {
        let mut found = found;
        found.extend(environment_roots(
            spec.id,
            |key| std::env::var_os(key),
            issue,
        ));
        found
    };
    let mut unique = std::collections::HashSet::new();
    Ok(found
        .into_iter()
        .map(|p| {
            let leaf = match spec.id {
                "dev.pnpm" => Some("v10"),
                "dev.yarn" => Some("v6"),
                _ => None,
            };
            leaf.filter(|leaf| p.join(leaf).is_dir())
                .map_or_else(|| p.clone(), |leaf| p.join(leaf))
        })
        .filter(|p| unique.insert(p.to_string_lossy().to_lowercase()))
        .collect())
}
pub(crate) fn environment_roots(
    id: &str,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for (key, suffix, multiple) in configured_locations(id) {
        if let Some(value) = env(key).filter(|v| !v.is_empty()) {
            let paths = if multiple {
                std::env::split_paths(&value).collect::<Vec<_>>()
            } else {
                vec![PathBuf::from(value)]
            };
            for path in paths {
                if !path.is_absolute() {
                    issue(ReasonCode::Unverifiable);
                    continue;
                }
                match extended(&path.join(suffix)) {
                    Ok(path) if path.components().count() >= 3 => found.push(path),
                    _ => issue(ReasonCode::Unverifiable),
                }
            }
        }
    }
    found
}
fn configured_locations(id: &str) -> Vec<(&'static str, &'static str, bool)> {
    match id {
        "dev.cargo_archives" => vec![("CARGO_HOME", "registry/cache", false)],
        "dev.rustup_downloads" => vec![("RUSTUP_HOME", "downloads", false)],
        "dev.composer_archives" => vec![("COMPOSER_CACHE_DIR", "files", false)],
        "dev.gem_archives" => vec![("GEM_HOME", "cache", false), ("GEM_PATH", "cache", true)],
        "dev.conda_archives" => vec![("CONDA_PKGS_DIRS", "", true)],
        "dev.vcpkg_archives" => vec![("VCPKG_DEFAULT_BINARY_CACHE", "", false)],
        "dev.poetry" => vec![
            ("POETRY_CACHE_DIR", "artifacts", false),
            ("POETRY_CACHE_DIR", "cache", false),
        ],
        "dev.pdm" => ["http", "wheels", "hashes", "metadata"]
            .map(|s| ("PDM_CACHE_DIR", s, false))
            .into(),
        "dev.go_build" => vec![("GOCACHE", "", false)],
        "dev.go_modules" => vec![
            ("GOPATH", "pkg/mod/cache/download", true),
            ("GOMODCACHE", "cache/download", false),
        ],
        "dev.gradle" => vec![("GRADLE_USER_HOME", "caches", false)],
        "dev.gradle_logs" => vec![("GRADLE_USER_HOME", "daemon", false)],
        "dev.sccache" => vec![("SCCACHE_DIR", "", false)],
        "dev.ccache" => vec![("CCACHE_DIR", "", false)],
        "dev.bun" => vec![("BUN_INSTALL_CACHE_DIR", "", false)],
        "dev.yarn" => vec![("YARN_CACHE_FOLDER", "", false)],
        "dev.bundler" => vec![("BUNDLE_USER_CACHE", "", false)],
        "dev.nuget_packages" => vec![("NUGET_PACKAGES", "", false)],
        "dev.conan" => vec![("CONAN_HOME", "p", false)],
        "dev.pub" => vec![("PUB_CACHE", "hosted", false), ("PUB_CACHE", "git", false)],
        "dev.deno" => ["gen", "deps", "remote", "npm"]
            .map(|s| ("DENO_DIR", s, false))
            .into(),
        _ => vec![],
    }
}
fn standard_roots(
    local: &Path,
    roaming: &Path,
    home: &Path,
    spec: &DeveloperRule,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    check_cancel(cancel)?;
    let local_path = extended(local)?;
    let roaming_path = extended(roaming)?;
    let (local, roaming) = (local_path.as_path(), roaming_path.as_path());
    let home = extended(home)?;
    let base = if spec.home {
        &home
    } else if spec.roaming {
        roaming
    } else {
        local
    }
    .join(spec.relative);
    match spec.id {
        "dev.uv" => {
            let mut roots = vec![local.join("uv/cache")];
            match crate::uv_cache::cache_root(local, roaming) {
                Ok(root) if !roots.contains(&root) => roots.push(root),
                Ok(_) => {}
                Err(_) => issue(ReasonCode::Unverifiable),
            }
            Ok(roots)
        }
        "dev.pip_http" => Ok(["http", "http-v2"]
            .iter()
            .map(|leaf| base.join(leaf))
            .collect()),
        "dev.vscode_cache" | "dev.vscode_logs" | "dev.cursor_cache" | "dev.cursor_logs" => {
            Ok((if spec.id.starts_with("dev.cursor") {
                &["Cursor"][..]
            } else {
                &["Code", "Code - Insiders"][..]
            })
            .iter()
            .flat_map(|product| {
                let leaves: &[&str] = if spec.diagnostics {
                    &["logs"]
                } else {
                    &["Cache/Cache_Data", "Code Cache", "GPUCache"]
                };
                leaves
                    .iter()
                    .map(move |leaf| roaming.join(product).join(leaf))
            })
            .collect())
        }
        "dev.poetry" => Ok(["artifacts", "cache"]
            .iter()
            .map(|part| base.join(part))
            .collect()),
        "dev.pdm" => Ok([base, home.join(".cache/pdm")]
            .into_iter()
            .flat_map(|root| ["http", "wheels", "hashes", "metadata"].map(|leaf| root.join(leaf)))
            .collect()),
        "dev.pub" => Ok(["hosted", "git"]
            .iter()
            .map(|part| base.join(part))
            .collect()),
        "dev.deno" => Ok(["gen", "deps", "remote", "npm"]
            .iter()
            .map(|part| base.join(part))
            .collect()),
        "dev.conda_archives" => Ok([
            home.join(".conda/pkgs"),
            home.join("miniconda3/pkgs"),
            home.join("anaconda3/pkgs"),
            home.join("miniforge3/pkgs"),
            local.join("miniconda3/pkgs"),
            local.join("anaconda3/pkgs"),
            local.join("miniforge3/pkgs"),
        ]
        .into()),
        "dev.vcpkg_archives" => Ok(vec![base, roaming.join(spec.relative)]),
        "dev.gem_archives" => {
            let mut roots = vec![];
            for parent in [home.join(".gem/ruby"), home.join(".local/share/gem/ruby")] {
                for version in subdirs(&parent, cancel, issue)? {
                    if dustaway_core::developer::numeric_version(
                        &version.file_name().unwrap().to_string_lossy(),
                    ) {
                        roots.push(version.join("cache"));
                    }
                }
            }
            Ok(roots)
        }
        "dev.visual_studio" => Ok(subdirs(&base, cancel, issue)?
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .split_once('_')
                    .is_some_and(|(version, instance)| {
                        dustaway_core::developer::numeric_version(version)
                            && !instance.is_empty()
                            && instance.chars().all(|c| c.is_ascii_hexdigit())
                    })
            })
            .map(|p| p.join("ComponentModelCache"))
            .collect()),
        "dev.jetbrains_logs" | "dev.android_logs" => {
            if !base.try_exists()? {
                return Ok(vec![]);
            }
            let _pin = PinnedPath::open_cancellable(&base, true, false, cancel)?;
            let mut roots = vec![];
            for entry in std::fs::read_dir(base)? {
                check_cancel(cancel)?;
                let found = (|| -> Result<Option<PathBuf>> {
                    let entry = entry?;
                    if !entry.file_type()?.is_dir()
                        || !(if spec.id == "dev.android_logs" {
                            entry
                                .file_name()
                                .to_string_lossy()
                                .strip_prefix("AndroidStudio")
                                .is_some_and(|version| {
                                    version.starts_with("20")
                                        && dustaway_core::developer::numeric_version(version)
                                })
                        } else {
                            dustaway_core::developer::jetbrains_product(
                                &entry.file_name().to_string_lossy(),
                            )
                        })
                    {
                        return Ok(None);
                    }
                    let path = entry.path();
                    let _pin = PinnedPath::open_cancellable(&path, true, false, cancel)?;
                    Ok(Some(path.join("log")))
                })();
                match found {
                    Ok(Some(root)) => roots.push(root),
                    Ok(None) => {}
                    Err(error) if error.is::<ScanCancelled>() => return Err(error),
                    Err(error) => issue(error_reason(&error)),
                }
            }
            roots.sort();
            Ok(roots)
        }
        _ => Ok(vec![base]),
    }
}

pub(crate) fn descend(id: &str, root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let parts: Vec<_> = relative.iter().map(|s| s.to_string_lossy()).collect();
    match id {
        "dev.conda_archives"
        | "dev.rustup_downloads"
        | "dev.gem_archives"
        | "dev.android_logs"
        | "dev.jetbrains_logs" => false,
        "dev.cargo_archives" | "dev.vcpkg_archives" => parts.len() <= 1,
        "dev.composer_archives" => parts.len() <= 2,
        "dev.gradle_logs" => {
            parts.len() == 1 && dustaway_core::developer::numeric_version(&parts[0])
        }
        "dev.uv" => parts
            .first()
            .is_some_and(|part| dustaway_core::developer::uv_bucket(part)),
        _ => true,
    }
}

fn subdirs(
    base: &Path,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    check_cancel(cancel)?;
    match base.try_exists() {
        Ok(false) => return Ok(vec![]),
        Err(e) => {
            issue(error_reason(&e.into()));
            return Ok(vec![]);
        }
        _ => {}
    }
    let pin = match PinnedPath::open_cancellable(base, true, false, cancel) {
        Ok(pin) => pin,
        Err(e) if e.is::<ScanCancelled>() => return Err(e),
        Err(e) => {
            issue(error_reason(&e));
            return Ok(vec![]);
        }
    };
    let mut found = vec![];
    for entry in std::fs::read_dir(&pin.path)? {
        check_cancel(cancel)?;
        match entry {
            Ok(entry) => match entry.file_type() {
                Ok(kind) if kind.is_dir() && !kind.is_symlink() => found.push(entry.path()),
                Ok(_) => {}
                Err(e) => issue(error_reason(&e.into())),
            },
            Err(e) => issue(error_reason(&e.into())),
        }
    }
    found.sort();
    Ok(found)
}

pub(crate) fn accepts_file(id: &str, root: &Path, path: &Path) -> bool {
    if dustaway_core::developer::rule(id).is_none() {
        return true;
    }
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let parts: Option<Vec<_>> = relative
        .components()
        .map(|part| match part {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();
    parts.is_some_and(|parts| dustaway_core::developer::accepts_file(id, &parts))
}

pub(crate) fn blocked(spec: &DeveloperRule) -> Result<Option<BlockReason>> {
    blocked_with(spec, security::first_running_process_id)
}
fn blocked_with(
    spec: &DeveloperRule,
    running: impl FnOnce(&[&str]) -> Result<Option<(String, u32)>>,
) -> Result<Option<BlockReason>> {
    if let Some((process, pid)) = running(spec.processes)? {
        if [
            "node.exe",
            "python.exe",
            "pythonw.exe",
            "java.exe",
            "javaw.exe",
            "dotnet.exe",
            "php.exe",
            "ruby.exe",
            "rubyw.exe",
            "dart.exe",
        ]
        .iter()
        .any(|name| process.eq_ignore_ascii_case(name))
        {
            return Ok(Some(BlockReason::new(
                ReasonCode::ProcessUnknown,
                format!(
                    "检测到 {process}（PID {pid}），无法确认是否使用此缓存，已保留；这不代表已确认缓存被占用。请核对并正常退出相关任务后重新检查"
                ),
            )));
        }
        return Ok(Some(BlockReason::new(
            ReasonCode::ApplicationRunning,
            format!("相关工具 {process}（PID {pid}）正在运行，正常退出后可重新检查"),
        )));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_locations_use_the_production_parser_and_reject_relative_paths() {
        let mut issues = vec![];
        let found = environment_roots(
            "dev.go_build",
            |_| Some("D:\\dedicated\\go-build".into()),
            &mut |v| issues.push(v),
        );
        assert_eq!(
            found,
            vec![extended(Path::new("D:\\dedicated\\go-build")).unwrap()]
        );
        assert!(issues.is_empty());
        assert!(
            environment_roots("dev.go_build", |_| Some("relative".into()), &mut |v| issues
                .push(v))
            .is_empty()
        );
        assert_eq!(issues, vec![ReasonCode::Unverifiable]);
        let found = environment_roots(
            "dev.go_modules",
            |key| (key == "GOPATH").then(|| "C:\\first;D:\\second".into()),
            &mut |_| panic!("valid roots"),
        );
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|p| p.ends_with("pkg/mod/cache/download")));
    }
    #[test]
    fn checks_all_toolchain_processes_and_propagates_unknown_state() {
        let spec = dustaway_core::developer::rule("dev.nuget_http").unwrap();
        assert!(
            blocked_with(spec, |names| {
                assert!(names.contains(&"MSBuild.exe"));
                Ok(Some(("MSBuild.exe".into(), 123)))
            })
            .unwrap()
            .is_some()
        );
        assert!(blocked_with(spec, |_| anyhow::bail!("cannot inspect")).is_err());
        assert!(blocked_with(spec, |_| Ok(None)).unwrap().is_none());
    }

    #[test]
    fn generic_runtime_is_unknown_not_confirmed_cache_occupancy() {
        let spec = dustaway_core::developer::rule("dev.npm_cache").unwrap();
        let reason = blocked_with(spec, |_| Ok(Some(("node.exe".into(), 123))))
            .unwrap()
            .unwrap();
        assert_eq!(reason.code, ReasonCode::ProcessUnknown);
        assert!(reason.message.contains("PID 123"));
        assert!(reason.message.contains("不代表已确认缓存被占用"));
    }
}
