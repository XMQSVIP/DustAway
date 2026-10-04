//! Fixed user cache locations. Project trees, shared package installations and IDE state are excluded.
use crate::{CleanupPolicy, DataKind, RuleDescriptor};

pub struct DeveloperRule {
    pub id: &'static str,
    pub relative: &'static str,
    pub roaming: bool,
    pub home: bool,
    pub cleanable: bool,
    pub age: u32,
    pub diagnostics: bool,
    pub processes: &'static [&'static str],
}

pub const RULES: &[DeveloperRule] = &[
    DeveloperRule {
        id: "dev.npm_cache",
        relative: "npm-cache/_cacache",
        roaming: false,
        home: false,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["node.exe"],
    },
    DeveloperRule {
        id: "dev.npm_logs",
        relative: "npm-cache/_logs",
        roaming: false,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: true,
        processes: &["node.exe"],
    },
    DeveloperRule {
        id: "dev.pip_http",
        relative: "pip/Cache",
        roaming: false,
        home: false,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["python.exe", "pythonw.exe", "pip.exe", "pip3.exe", "uv.exe"],
    },
    DeveloperRule {
        id: "dev.pip_wheels",
        relative: "pip/Cache/wheels",
        roaming: false,
        home: false,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["python.exe", "pythonw.exe", "pip.exe", "pip3.exe", "uv.exe"],
    },
    DeveloperRule {
        id: "dev.nuget_http",
        relative: "NuGet/v3-cache",
        roaming: false,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: false,
        processes: &[
            "dotnet.exe",
            "MSBuild.exe",
            "devenv.exe",
            "NuGet.exe",
            "rider64.exe",
        ],
    },
    DeveloperRule {
        id: "dev.vscode_cache",
        relative: "Code",
        roaming: true,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: false,
        processes: &["Code.exe", "Code - Insiders.exe"],
    },
    DeveloperRule {
        id: "dev.vscode_logs",
        relative: "Code",
        roaming: true,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: true,
        processes: &["Code.exe", "Code - Insiders.exe"],
    },
    DeveloperRule {
        id: "dev.jetbrains_logs",
        relative: "JetBrains",
        roaming: false,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: true,
        processes: &[
            "idea64.exe",
            "pycharm64.exe",
            "webstorm64.exe",
            "rider64.exe",
            "clion64.exe",
            "goland64.exe",
            "phpstorm64.exe",
            "datagrip64.exe",
            "rubymine64.exe",
            "rustrover64.exe",
            "dataspell64.exe",
            "java.exe",
            "javaw.exe",
        ],
    },
    DeveloperRule {
        id: "dev.uv",
        relative: "uv/cache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["uv.exe", "python.exe", "pythonw.exe"],
    },
    DeveloperRule {
        id: "dev.poetry",
        relative: "pypoetry/Cache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["poetry.exe", "python.exe"],
    },
    DeveloperRule {
        id: "dev.pdm",
        relative: "pdm/pdm/Cache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["pdm.exe", "python.exe"],
    },
    DeveloperRule {
        id: "dev.conda_archives",
        relative: ".conda/pkgs",
        roaming: false,
        home: true,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["conda.exe", "mamba.exe", "micromamba.exe", "python.exe"],
    },
    DeveloperRule {
        id: "dev.pnpm",
        relative: "pnpm/store",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["node.exe", "pnpm.exe"],
    },
    DeveloperRule {
        id: "dev.yarn",
        relative: "Yarn/Cache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["node.exe", "yarn.exe"],
    },
    DeveloperRule {
        id: "dev.yarn_berry",
        relative: "Yarn/Berry/cache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["node.exe", "yarn.exe"],
    },
    DeveloperRule {
        id: "dev.bun",
        relative: ".bun/install/cache",
        roaming: false,
        home: true,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["bun.exe"],
    },
    DeveloperRule {
        id: "dev.deno",
        relative: "deno",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["deno.exe"],
    },
    DeveloperRule {
        id: "dev.cargo_archives",
        relative: ".cargo/registry/cache",
        roaming: false,
        home: true,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["cargo.exe", "rustc.exe", "rustup.exe", "rust-analyzer.exe"],
    },
    DeveloperRule {
        id: "dev.rustup_downloads",
        relative: ".rustup/downloads",
        roaming: false,
        home: true,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["rustup.exe", "cargo.exe", "rustc.exe"],
    },
    DeveloperRule {
        id: "dev.sccache",
        relative: "Mozilla/sccache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["sccache.exe", "rustc.exe", "cl.exe", "clang.exe"],
    },
    DeveloperRule {
        id: "dev.go_build",
        relative: "go-build",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["go.exe", "compile.exe", "link.exe", "gopls.exe"],
    },
    DeveloperRule {
        id: "dev.go_modules",
        relative: "go/pkg/mod/cache/download",
        roaming: false,
        home: true,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["go.exe", "gopls.exe"],
    },
    DeveloperRule {
        id: "dev.gradle",
        relative: ".gradle/caches",
        roaming: false,
        home: true,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["java.exe", "javaw.exe", "gradle.exe"],
    },
    DeveloperRule {
        id: "dev.gradle_logs",
        relative: ".gradle/daemon",
        roaming: false,
        home: true,
        cleanable: true,
        age: 7,
        diagnostics: true,
        processes: &["java.exe", "javaw.exe", "gradle.exe"],
    },
    DeveloperRule {
        id: "dev.maven",
        relative: ".m2/repository",
        roaming: false,
        home: true,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["java.exe", "javaw.exe", "mvn.exe"],
    },
    DeveloperRule {
        id: "dev.nuget_packages",
        relative: ".nuget/packages",
        roaming: false,
        home: true,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["dotnet.exe", "MSBuild.exe", "devenv.exe", "rider64.exe"],
    },
    DeveloperRule {
        id: "dev.composer_archives",
        relative: "Composer/files",
        roaming: false,
        home: false,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["php.exe", "composer.exe"],
    },
    DeveloperRule {
        id: "dev.gem_archives",
        relative: ".gem",
        roaming: false,
        home: true,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &["ruby.exe", "rubyw.exe", "gem.exe", "bundle.exe"],
    },
    DeveloperRule {
        id: "dev.bundler",
        relative: ".bundle/cache",
        roaming: false,
        home: true,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["ruby.exe", "rubyw.exe", "bundle.exe"],
    },
    DeveloperRule {
        id: "dev.pub",
        relative: "Pub/Cache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["dart.exe", "flutter.exe"],
    },
    DeveloperRule {
        id: "dev.vcpkg_archives",
        relative: "vcpkg/archives",
        roaming: false,
        home: false,
        cleanable: true,
        age: 30,
        diagnostics: false,
        processes: &[
            "vcpkg.exe",
            "cmake.exe",
            "ninja.exe",
            "MSBuild.exe",
            "cl.exe",
        ],
    },
    DeveloperRule {
        id: "dev.ccache",
        relative: "ccache",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["ccache.exe", "gcc.exe", "g++.exe", "clang.exe", "cl.exe"],
    },
    DeveloperRule {
        id: "dev.conan",
        relative: ".conan2/p",
        roaming: false,
        home: true,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &["conan.exe", "python.exe", "cmake.exe"],
    },
    DeveloperRule {
        id: "dev.android_logs",
        relative: "Google",
        roaming: false,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: true,
        processes: &["studio64.exe", "studio.exe", "java.exe", "javaw.exe"],
    },
    DeveloperRule {
        id: "dev.visual_studio",
        relative: "Microsoft/VisualStudio",
        roaming: false,
        home: false,
        cleanable: false,
        age: 0,
        diagnostics: false,
        processes: &[
            "devenv.exe",
            "MSBuild.exe",
            "ServiceHub.Host.dotnet.x64.exe",
        ],
    },
    DeveloperRule {
        id: "dev.cursor_cache",
        relative: "Cursor",
        roaming: true,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: false,
        processes: &["Cursor.exe"],
    },
    DeveloperRule {
        id: "dev.cursor_logs",
        relative: "Cursor",
        roaming: true,
        home: false,
        cleanable: true,
        age: 7,
        diagnostics: true,
        processes: &["Cursor.exe"],
    },
];

pub fn rule(id: &str) -> Option<&'static DeveloperRule> {
    RULES.iter().find(|rule| rule.id == id)
}

pub fn valid_descriptor(r: &RuleDescriptor) -> bool {
    rule(&r.id).is_some_and(|s| {
        r.root == "developer"
            && r.relative == s.relative
            && r.version == 1
            && r.min_age_days == s.age
            && r.app_id.is_none()
            && r.process.as_deref() == s.processes.first().copied()
            && !r.requires_admin
            && !r.default_selected
            && r.cleanup_policy == CleanupPolicy::Review
            && r.cleanup_validated == s.cleanable
            && r.data_kind
                == if s.diagnostics {
                    DataKind::Diagnostics
                } else {
                    DataKind::Cache
                }
    })
}

pub fn jetbrains_product(name: &str) -> bool {
    [
        "IntelliJIdea",
        "IdeaIC",
        "PyCharm",
        "PyCharmCE",
        "WebStorm",
        "Rider",
        "CLion",
        "GoLand",
        "PhpStorm",
        "DataGrip",
        "RubyMine",
        "RustRover",
        "DataSpell",
    ]
    .iter()
    .any(|prefix| {
        name.strip_prefix(prefix).is_some_and(|suffix| {
            suffix.starts_with("20")
                && suffix.len() >= 6
                && suffix
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
        })
    })
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn log_name(name: &str) -> bool {
    name.ends_with(".log")
        || name
            .rsplit_once(".log.")
            .is_some_and(|(_, tail)| !tail.is_empty() && tail.bytes().all(|c| c.is_ascii_digit()))
}

/// Components relative to a freshly re-derived root, never an absolute user-supplied path.
pub fn accepts_file(id: &str, parts: &[&str]) -> bool {
    let Some(name) = parts.last() else {
        return false;
    };
    match id {
        "dev.npm_cache" => match parts {
            ["content-v2", "sha512", a, b, file] => hex(a, 2) && hex(b, 2) && hex(file, 124),
            ["index-v5", a, b, file] => hex(a, 2) && hex(b, 2) && hex(file, 60),
            _ => false,
        },
        "dev.npm_logs" => {
            parts.len() == 1
                && name.ends_with(".log")
                && name.as_bytes().get(4) == Some(&b'-')
                && name
                    .get(..4)
                    .is_some_and(|year| year.bytes().all(|c| c.is_ascii_digit()))
        }
        "dev.pip_http" => match parts {
            [a, b, c, d, e, file] => {
                [a, b, c, d, e].iter().all(|part| hex(part, 1))
                    && hex(file.strip_suffix(".body").unwrap_or(file), 56)
            }
            _ => false,
        },
        "dev.pip_wheels" => match parts {
            [a, b, c, key, file] => {
                [a, b, c].iter().all(|part| hex(part, 2)) && hex(key, 50) && file.ends_with(".whl")
            }
            _ => false,
        },
        "dev.nuget_http" => {
            parts.len() == 2
                && parts[0]
                    .split_once('$')
                    .is_some_and(|(hash, _)| hex(hash, 40))
                && name.ends_with(".dat")
        }
        "dev.vscode_cache" | "dev.cursor_cache" => {
            let parts = if matches!(parts.first(), Some(&"js" | &"wasm")) {
                &parts[1..]
            } else {
                parts
            };
            match parts {
                ["index-dir", "the-real-index"] => true,
                [file] => {
                    *file == "index"
                        || matches!(*file, "data_0" | "data_1" | "data_2" | "data_3")
                        || file.strip_prefix("f_").is_some_and(|hash| hex(hash, 6))
                        || file.rsplit_once('_').is_some_and(|(hash, stream)| {
                            hex(hash, 16) && matches!(stream, "0" | "1")
                        })
                }
                _ => false,
            }
        }
        "dev.vscode_logs" | "dev.cursor_logs" => {
            parts.len() >= 2
                && parts[0].len() == 15
                && parts[0].as_bytes()[8] == b'T'
                && parts[0]
                    .bytes()
                    .enumerate()
                    .all(|(i, c)| i == 8 || c.is_ascii_digit())
                && log_name(name)
        }
        "dev.jetbrains_logs" | "dev.android_logs" => parts.len() == 1 && log_name(name),
        "dev.cargo_archives" => {
            parts.len() == 2
                && parts[0]
                    .rsplit_once('-')
                    .is_some_and(|(_, hash)| hex(hash, 16))
                && name.ends_with(".crate")
        }
        "dev.rustup_downloads" => parts.len() == 1 && hex(name, 64),
        "dev.conda_archives" => {
            parts.len() == 1 && (name.ends_with(".conda") || name.ends_with(".tar.bz2"))
        }
        "dev.composer_archives" => {
            parts.len() == 3
                && name
                    .strip_suffix(".zip")
                    .is_some_and(|hash| hex(hash, 40) || hex(hash, 64))
        }
        "dev.gem_archives" => parts.len() == 1 && name.ends_with(".gem"),
        "dev.vcpkg_archives" => {
            parts.len() == 2
                && hex(parts[0], 2)
                && name
                    .strip_suffix(".zip")
                    .is_some_and(|hash| hex(hash, 40) || hex(hash, 64))
        }
        "dev.gradle_logs" => {
            parts.len() == 2
                && numeric_version(parts[0])
                && name
                    .strip_prefix("daemon-")
                    .and_then(|n| n.strip_suffix(".out.log"))
                    .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|c| c.is_ascii_digit()))
        }
        "dev.uv" => parts.len() > 1 && uv_bucket(parts[0]),
        // These records are display-only. The immutable descriptor forbids preparing
        // or deleting them through the ordinary per-file engine.
        _ => rule(id).is_some_and(|r| !r.cleanable),
    }
}

pub fn uv_bucket(name: &str) -> bool {
    [
        "archive",
        "wheels",
        "sdists",
        "built-wheels",
        "git",
        "interpreter",
        "flat-index",
        "simple",
        "environments",
        "environments-ci",
    ]
    .iter()
    .any(|prefix| {
        name.strip_prefix(prefix)
            .and_then(|n| n.strip_prefix("-v"))
            .is_some_and(|version| {
                !version.is_empty() && version.bytes().all(|c| c.is_ascii_digit())
            })
    })
}

pub fn numeric_version(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn developer_descriptors_cannot_expand_roots_or_enable_default_selection() {
        for spec in RULES {
            let rule = crate::rules::catalog()
                .unwrap()
                .into_iter()
                .find(|r| r.id == spec.id)
                .unwrap();
            assert!(valid_descriptor(&rule));
            for field in [
                "root", "relative", "process", "age", "default", "policy", "version", "app",
            ] {
                let mut bad = rule.clone();
                match field {
                    "root" => bad.root = "user_temp".into(),
                    "relative" => bad.relative = "..".into(),
                    "process" => bad.process = None,
                    "age" => bad.min_age_days += 1,
                    "default" => bad.default_selected = true,
                    "policy" => bad.cleanup_policy = CleanupPolicy::Recommended,
                    "version" => bad.version += 1,
                    _ => bad.app_id = Some("npm".into()),
                }
                assert!(
                    crate::rules::validate(&bad).is_err(),
                    "{}: {field}",
                    spec.id
                );
            }
        }
    }
    #[test]
    fn shapes_exclude_configuration_projects_and_unknown_versions() {
        assert!(accepts_file(
            "dev.npm_cache",
            &["content-v2", "sha512", "ab", "cd", &"a".repeat(124)]
        ));
        assert!(!accepts_file(
            "dev.npm_cache",
            &["content-v9", "sha512", "ab", "cd", &"a".repeat(124)]
        ));
        for id in RULES.iter().filter(|s| s.cleanable).map(|s| s.id) {
            for parts in [
                &["User", "settings.json"][..],
                &["node_modules", "package.json"],
                &[".venv", "python.exe"],
                &["project", "main.rs"],
            ] {
                assert!(!accepts_file(id, parts), "{id}");
            }
        }
        assert!(jetbrains_product("PyCharm2026.2"));
        assert!(!jetbrains_product("PyCharm2026.2-backup"));
        assert!(!jetbrains_product("personal-project"));
    }
}
