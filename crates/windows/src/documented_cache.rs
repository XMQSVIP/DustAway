//! Vendor identity and supported cache formats authorize these narrow scopes. We only
//! accept recognized GPU blockfile formats and Simple Cache code entries;
//! cookies, databases and browser storage are never deleted.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

// Extracted without executing the official, SHA-256-verified 1.8.1 Windows release.
const CHERRY_EXE: &str = "a866793f98aace8f554fe714def1c51d9d1e857a0bad2457c6b88135391ffdbe";
const CHERRY_ASAR: &str = "94936572c26f420780ca2f5ffae9de58f1e83ef783eec247278881a2328788d9";

fn exact_hash(pin: &PinnedPath, expected: &str) -> Result<bool> {
    // Include NTFS change time, file identity and the pinned parent chain. A
    // changed/replaced executable cannot reuse a previously verified digest.
    static VERIFIED: OnceLock<Mutex<BTreeMap<String, bool>>> = OnceLock::new();
    let key = serde_json::to_string(&(native(&pin.path), &pin.stamp, &pin.parent_ids, expected))?;
    let cache = VERIFIED.get_or_init(Default::default);
    if let Some(verified) = cache.lock().unwrap().get(&key) {
        return Ok(*verified);
    }
    ensure!(
        pin.stamp.size <= 512 * 1024 * 1024,
        Refusal(ReasonCode::ApplicationUnverified)
    );
    let mut file = File::open(&pin.path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    let verified = format!("{:x}", digest.finalize()) == expected;
    let mut cache = cache.lock().unwrap();
    if cache.len() >= 32 {
        cache.clear();
    }
    cache.insert(key, verified);
    Ok(verified)
}

fn pycharm_product(value: &serde_json::Value) -> bool {
    value["name"] == "PyCharm"
        && value["version"]
            .as_str()
            .is_some_and(dustaway_core::developer::numeric_version)
        && value["buildNumber"]
            .as_str()
            .is_some_and(dustaway_core::developer::numeric_version)
        && value["productCode"] == "PY"
        && value["productVendor"] == "JetBrains"
        && value["dataDirectoryName"].as_str().is_some_and(|profile| {
            pycharm_profile(profile)
                && value["version"].as_str().is_some_and(|version| {
                    let suffix = &profile["PyCharm".len()..];
                    version == suffix
                        || version
                            .strip_prefix(suffix)
                            .is_some_and(|rest| rest.starts_with('.'))
                })
        })
        && value["launch"].as_array().is_some_and(|entries| {
            entries.iter().any(|v| {
                v["os"] == "Windows"
                    && v["arch"] == "amd64"
                    && v["launcherPath"] == "bin/pycharm64.exe"
            })
        })
}

pub(super) fn pycharm_profile(name: &str) -> bool {
    name.strip_prefix("PyCharm").is_some_and(|version| {
        version.split('.').count() == 2 && dustaway_core::developer::numeric_version(version)
    })
}

pub(super) fn profile_matches(profile: Option<&str>, root: &Path) -> bool {
    profile.is_some_and(|profile| {
        pycharm_profile(profile)
            && root.ancestors().any(|p| {
                p.file_name().is_some_and(|name| name == profile)
                    && p.parent()
                        .is_some_and(|parent| parent.ends_with("JetBrains"))
            })
    })
}

fn pycharm_build_matches(value: &serde_json::Value, build: &str) -> bool {
    pycharm_product(value)
        && value["buildNumber"]
            .as_str()
            .is_some_and(|number| build.trim() == format!("PY-{number}"))
}

fn jcef_matches(release: &str, dll_version: &str) -> bool {
    let versions: Vec<_> = release
        .lines()
        .filter_map(|line| line.trim().strip_prefix("JCEF_VERSION_DETAILED="))
        .collect();
    versions.len() == 1
        && versions[0]
            .trim_matches('"')
            .split_once("-g")
            .is_some_and(|(version, rest)| !rest.is_empty() && same_version(dll_version, version))
}

pub(super) fn identity(spec: &CacheSpec) -> Result<CodeIdentity> {
    let mut installs = crate::applications::installed_directories();
    installs.sort();
    installs.dedup();
    let mut verified = Vec::new();
    let mut unsupported = false;
    for install in installs {
        let name = match spec.app.as_str() {
            "cherry" => "Cherry Studio.exe",
            "pycharm" => "bin/pycharm64.exe",
            _ => continue,
        };
        let Ok(exe) = PinnedPath::open(&install.join(name), false, false) else {
            continue;
        };
        let mut pins = vec![];
        let mut profile = None;
        if spec.app == "cherry" {
            let Ok(package) = PinnedPath::open(&install.join("resources/app.asar"), false, false)
            else {
                unsupported = true;
                continue;
            };
            // This official executable is unsigned. Digests establish provenance,
            // not a version allowlist; an unknown unsigned binary is unverified.
            if !exact_hash(&exe, CHERRY_EXE)? || !exact_hash(&package, CHERRY_ASAR)? {
                return Err(Refusal(ReasonCode::ApplicationUnverified).into());
            }
            pins.push(package);
        } else {
            if !crate::applications::signed_by(&exe, "JetBrains s.r.o.") {
                continue;
            }
            let Ok(product) = PinnedPath::open(&install.join("product-info.json"), false, false)
            else {
                unsupported = true;
                continue;
            };
            let Ok(build) = PinnedPath::open(&install.join("build.txt"), false, false) else {
                unsupported = true;
                continue;
            };
            if product.stamp.size > 1024 * 1024 || build.stamp.size > 100 {
                unsupported = true;
                continue;
            }
            let value =
                serde_json::from_slice::<serde_json::Value>(&std::fs::read(&product.path)?)?;
            if !pycharm_build_matches(&value, &std::fs::read_to_string(&build.path)?) {
                unsupported = true;
                continue;
            }
            let runtime = PinnedPath::open(&install.join("jbr/bin/libcef.dll"), false, false)?;
            let release = PinnedPath::open(&install.join("jbr/release"), false, false)?;
            ensure!(
                release.stamp.size <= 65536,
                Refusal(ReasonCode::UnsupportedVersion)
            );
            if !jcef_matches(
                &std::fs::read_to_string(&release.path)?,
                &crate::applications::executable_version(&runtime.path)?,
            ) {
                unsupported = true;
                continue;
            }
            profile = value["dataDirectoryName"].as_str().map(str::to_owned);
            pins.extend([product, build, runtime, release]);
        }
        pins.push(exe);
        let fingerprint = serde_json::to_string(
            &pins
                .iter()
                .map(|p| (native(&p.path), &p.stamp, &p.parent_ids))
                .collect::<Vec<_>>(),
        )?;
        verified.push(CodeIdentity {
            _pins: pins,
            commit: String::new(),
            fingerprint,
            profile,
        });
    }
    ensure!(!unsupported, Refusal(ReasonCode::UnsupportedVersion));
    ensure!(
        verified.len() == 1,
        Refusal(ReasonCode::ApplicationUnverified)
    );
    Ok(verified.remove(0))
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// GPU-only rebuildable shader storage. Never use this recognizer for HTTP
/// caches: deleting a single HTTP backing file can break the next page load.
pub(super) fn gpu_file(root: &Path, path: &Path) -> Result<bool> {
    if path.parent() != Some(root) {
        return Ok(false);
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return Ok(false);
    };
    let expected = match name {
        "data_0" => (0, 36),
        "data_1" => (1, 256),
        "data_2" => (2, 1024),
        "data_3" => (3, 4096),
        "index" => (4, 0),
        _ => return Ok(false),
    };
    let pin = PinnedPath::open(path, false, false)?;
    if pin.stamp.size < 32 {
        return Ok(false);
    }
    let mut header = [0; 32];
    File::open(path)?.read_exact(&mut header)?;
    if name == "index" {
        let entries = word(&header, 28);
        let entries = if entries == 0 { 65536 } else { entries };
        return Ok(word(&header, 0) == 0xc103cac3
            && word(&header, 4) == 0x30000
            && entries.is_power_of_two()
            && entries <= 1_048_576
            && pin.stamp.size == 368 + u64::from(entries) * 4);
    }
    Ok(word(&header, 0) == 0xc104cac3
        && word(&header, 4) == 0x20000
        && word(&header, 8) & 65535 == expected.0
        && word(&header, 12) == expected.1
        && pin.stamp.size >= 8192
        && pin.stamp.size <= 8192 + 65536 * u64::from(expected.1)
        && (pin.stamp.size - 8192).is_multiple_of(u64::from(expected.1)))
}

pub(super) fn accepts(
    spec: &CacheSpec,
    root: &Path,
    path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    check_cancel(cancel)?;
    let Ok(relative) = path.strip_prefix(root) else {
        return Ok(false);
    };
    let parts: Vec<_> = relative.iter().map(|c| c.to_string_lossy()).collect();
    if spec.kind == "documented_gpu" {
        return gpu_file(root, path);
    }
    if spec.kind == "documented_code"
        && parts.len() == 2
        && matches!(parts[0].as_ref(), "js" | "wasm")
    {
        let name = parts[1].as_ref();
        // Do not delete the shared index. Its recognized marker is a second
        // layout check; an arbitrary file with cache magic is insufficient.
        if name.strip_suffix("_0").is_some_and(|n| hex(n, 16)) && valid_code_format(path)? {
            let marker = path.parent().unwrap().join("index");
            return if marker.try_exists()? {
                valid_code_format(&marker)
            } else {
                Ok(false)
            };
        }
    }
    Ok(false)
}

pub(super) fn routed_from_readonly(
    spec: &CacheSpec,
    root: &Path,
    path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    let (app, base) = match spec.id.as_str() {
        "appcache.cherry" => (
            "cherry",
            root.ancestors().find(|p| p.ends_with("CherryStudio")),
        ),
        "appcache.jetbrains" => (
            "pycharm",
            root.ancestors().find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(pycharm_profile)
            }),
        ),
        _ => return Ok(false),
    };
    let Some(base) = base else { return Ok(false) };
    for rule in dustaway_core::app_cache::catalog()
        .iter()
        .filter(|r| r.app == app && r.cleanable())
    {
        for leaf in &rule.paths {
            let target = base.join(leaf);
            if path.starts_with(&target) {
                match accepts(rule, &target, path, cancel) {
                    Ok(true) => return Ok(true),
                    Err(error) if error.is::<ScanCancelled>() => return Err(error),
                    _ => {} // Original read-only coverage survives unsupported layouts.
                }
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "documented_cache_tests.rs"]
mod tests;
