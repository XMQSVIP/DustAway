use super::*;

fn cases() -> Vec<(&'static str, bool, String)> {
    vec![
        (
            "dev.npm_cache",
            false,
            format!(
                "npm-cache/_cacache/content-v2/sha512/ab/cd/{}",
                "a".repeat(124)
            ),
        ),
        (
            "dev.npm_logs",
            false,
            "npm-cache/_logs/2026-01-01T01_00_00_000Z-debug-0.log".into(),
        ),
        (
            "dev.pip_http",
            false,
            format!("pip/Cache/http-v2/a/b/c/d/e/{}.body", "a".repeat(56)),
        ),
        (
            "dev.pip_wheels",
            false,
            format!(
                "pip/Cache/wheels/ab/cd/ef/{}/fixture-1.0-py3-none-any.whl",
                "a".repeat(50)
            ),
        ),
        (
            "dev.nuget_http",
            false,
            format!("NuGet/v3-cache/{}$feed/list_fixture.dat", "a".repeat(40)),
        ),
        ("dev.vscode_cache", true, "Code/GPUCache/data_0".into()),
        (
            "dev.vscode_logs",
            true,
            "Code/logs/20260101T010000/window1/renderer.log".into(),
        ),
        (
            "dev.jetbrains_logs",
            false,
            "JetBrains/PyCharm2026.2/log/idea.log.1".into(),
        ),
    ]
}

#[test]
fn developer_scan_requires_known_shapes_keeps_recent_files_and_revalidates_identity() {
    for (id, roaming, relative) in cases() {
        let (_temp, platform) = fixture();
        let base = if roaming {
            &platform.roaming
        } else {
            &platform.local
        };
        let path = base.join(relative);
        write(&path, false);
        filetime::set_file_mtime(
            &path,
            filetime::FileTime::from_unix_time(now() - 40 * 86400, 0),
        )
        .unwrap();
        let protected = path.parent().unwrap().join("personal-project.rs");
        write(&protected, true);
        let rule = rule(id);
        let collect = || {
            let mut files = vec![];
            platform
                .scan(&rule, &[], &AtomicBool::new(false), &mut |event| {
                    if let ScanEvent::File(file) = event {
                        files.push(file);
                    }
                })
                .unwrap();
            files
        };
        let files = collect();
        assert_eq!(files.len(), 1, "{id}");
        platform.validate_candidate(&rule, &files[0]).unwrap();
        std::fs::write(&path, b"changed file must not be deleted").unwrap();
        assert_eq!(
            error_reason(&platform.validate_candidate(&rule, &files[0]).unwrap_err()),
            ReasonCode::Changed,
            "{id}"
        );
        assert!(collect().is_empty(), "recent {id}");
        assert!(protected.exists());
    }
}

#[test]
fn developer_mixed_roots_preserve_valid_profiles_and_cancellation_exclusions() {
    let (_temp, platform) = fixture();
    let old = platform
        .roaming
        .join("Code - Insiders/Code Cache/js/0123456789abcdef_0");
    write(&old, true);
    for relative in [
        "Code/User/settings.json",
        "Code/Backups/untitled/backup",
        "Code/User/History/file",
        "Code - Insiders/Code Cache/project/main.rs",
    ] {
        write(&platform.roaming.join(relative), true);
    }
    let rule = rule("dev.vscode_cache");
    let mut files = vec![];
    let mut missing = false;
    platform
        .scan(
            &rule,
            &[],
            &AtomicBool::new(false),
            &mut |event| match event {
                ScanEvent::File(file) => files.push(file),
                ScanEvent::NoticeCode(ReasonCode::LocationMissing) => missing = true,
                _ => {}
            },
        )
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        !missing,
        "one absent cache must not hide the Insiders cache"
    );
    let mut excluded_files = 0;
    platform
        .scan(
            &rule,
            &[display_path(&old)],
            &AtomicBool::new(false),
            &mut |event| {
                if matches!(event, ScanEvent::File(_)) {
                    excluded_files += 1;
                }
            },
        )
        .unwrap();
    assert_eq!(excluded_files, 0);
    assert!(
        platform
            .scan(&rule, &[], &AtomicBool::new(true), &mut |_| panic!(
                "cancelled scan emitted results"
            ))
            .is_ok()
    );
}

#[test]
fn developer_roots_do_not_follow_links_or_unknown_jetbrains_folders() {
    let (temp, platform) = fixture();
    let outside = temp.path().join("protected");
    write(&outside.join("index"), true);
    std::fs::create_dir_all(platform.roaming.join("Code")).unwrap();
    let linked = platform.roaming.join("Code/GPUCache");
    let result = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(display_path(&linked).replace('/', "\\"))
        .arg(display_path(&outside).replace('/', "\\"))
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    {
        let mut files = 0;
        platform
            .scan(
                &rule("dev.vscode_cache"),
                &[],
                &AtomicBool::new(false),
                &mut |event| {
                    if matches!(event, ScanEvent::File(_)) {
                        files += 1;
                    }
                },
            )
            .unwrap();
        assert_eq!(files, 0);
        assert!(outside.join("index").exists());
    }
    write(
        &platform
            .local
            .join("JetBrains/PersonalProject/log/idea.log"),
        true,
    );
    write(
        &platform
            .local
            .join("JetBrains/PyCharm2026.2-backup/log/idea.log"),
        true,
    );
    let mut files = 0;
    platform
        .scan(
            &rule("dev.jetbrains_logs"),
            &[],
            &AtomicBool::new(false),
            &mut |event| {
                if matches!(event, ScanEvent::File(_)) {
                    files += 1;
                }
            },
        )
        .unwrap();
    assert_eq!(files, 0);
}

#[test]
fn mainstream_downloads_use_narrow_shapes_age_and_identity() {
    let cases = [
        (
            "dev.cargo_archives",
            "home",
            format!(
                ".cargo/registry/cache/index.crates.io-{}/sample-1.0.crate",
                "a".repeat(16)
            ),
        ),
        (
            "dev.rustup_downloads",
            "home",
            format!(".rustup/downloads/{}", "a".repeat(64)),
        ),
        (
            "dev.conda_archives",
            "home",
            "miniconda3/pkgs/sample-1.0-0.conda".into(),
        ),
        (
            "dev.composer_archives",
            "local",
            format!("Composer/files/vendor/package/{}.zip", "a".repeat(40)),
        ),
        (
            "dev.gem_archives",
            "home",
            ".gem/ruby/3.3.0/cache/sample-1.0.gem".into(),
        ),
        (
            "dev.vcpkg_archives",
            "local",
            format!("vcpkg/archives/aa/{}.zip", "a".repeat(40)),
        ),
        (
            "dev.gradle_logs",
            "home",
            ".gradle/daemon/8.14.3/daemon-123.out.log".into(),
        ),
        (
            "dev.android_logs",
            "local",
            "Google/AndroidStudio2026.1/log/idea.log.1".into(),
        ),
        (
            "dev.cursor_cache",
            "roaming",
            "Cursor/GPUCache/data_0".into(),
        ),
        (
            "dev.cursor_logs",
            "roaming",
            "Cursor/logs/20260101T010000/window1/main.log".into(),
        ),
    ];
    for (id, base, relative) in cases {
        let (_temp, platform) = fixture();
        let base = match base {
            "home" => platform.local.parent().unwrap().join("home"),
            "roaming" => platform.roaming.clone(),
            _ => platform.local.clone(),
        };
        let path = base.join(relative);
        write(&path, true);
        filetime::set_file_mtime(
            &path,
            filetime::FileTime::from_unix_time(now() - 40 * 86400, 0),
        )
        .unwrap();
        let keep = path.parent().unwrap().join("project-private.toml");
        write(&keep, true);
        let mut files = vec![];
        let rule = rule(id);
        platform
            .scan(&rule, &[], &AtomicBool::new(false), &mut |event| {
                if let ScanEvent::File(file) = event {
                    files.push(file)
                }
            })
            .unwrap();
        assert_eq!(files.len(), 1, "{id}");
        platform.validate_candidate(&rule, &files[0]).unwrap();
        std::fs::write(&path, b"changed").unwrap();
        assert!(
            platform.validate_candidate(&rule, &files[0]).is_err(),
            "{id}"
        );
        assert!(keep.exists());
    }
}

#[test]
fn managed_caches_display_hardlinks_and_cannot_enter_file_deletion() {
    let (_temp, platform) = fixture();
    let root = platform.local.join("uv/cache");
    let path = root.join("archive-v0/test/package.py");
    write(&path, false);
    let linked = root.join("archive-v0/test/package-link.py");
    std::fs::hard_link(&path, &linked).unwrap();
    for relative in [
        "CACHEDIR.TAG",
        ".lock",
        "personal-project/main.py",
        "environments/project/Scripts/python.exe",
    ] {
        write(&root.join(relative), false);
    }
    let rule = rule("dev.uv");
    let mut files = vec![];
    platform
        .scan(&rule, &[], &AtomicBool::new(false), &mut |event| {
            if let ScanEvent::File(file) = event {
                files.push(file)
            }
        })
        .unwrap();
    assert_eq!(
        files.len(),
        1,
        "hardlink size counted once, no marker/project files"
    );
    assert!(!platform.cleanup_validated(&rule));
    assert_eq!(
        error_reason(&platform.delete(&rule, &files[0], &[]).unwrap_err()),
        ReasonCode::ValidationPending
    );
    let mut forged = rule.clone();
    forged.cleanup_validated = true;
    assert!(!platform.cleanup_validated(&forged));
    assert!(platform.delete(&forged, &files[0], &[]).is_err());
    assert!(path.exists());
}

#[test]
fn managed_python_and_ide_roots_keep_environments_history_and_unknown_instances_out() {
    let (_temp, platform) = fixture();
    for path in [
        "pypoetry/Cache/artifacts/ab/file.whl",
        "pypoetry/Cache/virtualenvs/project/Scripts/python.exe",
        "Microsoft/VisualStudio/17.0_abcdef/ComponentModelCache/data.cache",
        "Microsoft/VisualStudio/17.0_abcdef/Backups/private.txt",
        "Microsoft/VisualStudio/my-project/ComponentModelCache/private.txt",
    ] {
        write(&platform.local.join(path), false);
    }
    for id in ["dev.poetry", "dev.visual_studio"] {
        let mut files = vec![];
        platform
            .scan(&rule(id), &[], &AtomicBool::new(false), &mut |event| {
                if let ScanEvent::File(file) = event {
                    files.push(file)
                }
            })
            .unwrap();
        assert_eq!(files.len(), 1, "{id}");
    }
}
