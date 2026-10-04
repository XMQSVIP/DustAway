//! Single-use activation and evidence collection for the dedicated packaged data fixture.
//! No package installation, uninstall, certificate trust, deletion, or vendor UI control.
use anyhow::{Context, Result, ensure};
use dustaway_windows::{
    fs::{Handle, PinnedPath, extended, text_wide},
    platform::known_folder,
    security,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows::{
    Management::Deployment::{PackageManager, PackageTypes},
    Win32::{
        Foundation::{
            ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, FILETIME, GetLastError, WAIT_OBJECT_0,
            WAIT_TIMEOUT,
        },
        Storage::Packaging::Appx::GetPackageFullName,
        System::{
            Com::{
                CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            Threading::*,
        },
        UI::Shell::{
            AO_NOERRORUI, ApplicationActivationManager, FOLDERID_LocalAppData,
            IApplicationActivationManager,
        },
    },
    core::{HSTRING, PCWSTR, PWSTR},
};

const NAME: &str = "DustAway.Acceptance.Data.20261001";
const PUBLISHER: &str = "CN=DustAway Data Acceptance 20261001";
const MARKER: &str = "DustAway.Msix.Data.20261001.7c7e25e8";
const FIXTURE_SHA: &str = "3DFBB5BA09DCF99F50FC0CB87FE29823C248505BFFDF4BA14D792091D44F5666";
const FOLDERS: [(&str, &str); 4] = [
    ("LocalCache", "persistent_local_data"),
    ("TempState", "temporary_cache"),
    ("LocalState", "local_data"),
    ("RoamingState", "roaming_data"),
];

fn arguments(args: &[String]) -> Result<(&str, bool)> {
    match args {
        [action] if action == "preflight" => Ok(("preflight", true)),
        [phase] if matches!(phase.as_str(), "seed" | "verify") => Ok((phase, false)),
        [action, phase] if action == "observe" && matches!(phase.as_str(), "seed" | "verify") => {
            Ok((phase, true))
        }
        _ => anyhow::bail!(
            "expected preflight, seed, verify, observe seed, or observe verify; no paths"
        ),
    }
}

fn write_new(path: &Path, value: &Value) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    Ok(())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let pin = PinnedPath::open(path, false, false)?;
    ensure!(pin.stamp.size <= 1_048_576, "oversized evidence file");
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == pin.stamp.size,
        "evidence changed or exceeds limit"
    );
    Ok(bytes)
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = read_bounded(path)?;
    Ok(serde_json::from_slice(
        bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes),
    )?)
}

struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct Target {
    family: String,
    full_name: String,
    image: PathBuf,
    data: PathBuf,
}

fn target(local: &Path) -> Result<Target> {
    let manager = PackageManager::new()?;
    let found = manager
        .FindPackagesByUserSecurityIdNamePublisherWithPackageTypes(
            &HSTRING::new(),
            &HSTRING::from(NAME),
            &HSTRING::from(PUBLISHER),
            PackageTypes::Main,
        )?
        .First()?;
    ensure!(
        found.HasCurrent()?,
        "dedicated data package is not installed; no activation"
    );
    let package = found.Current()?;
    ensure!(!found.MoveNext()?, "multiple fixture package versions");
    let id = package.Id()?;
    let version = id.Version()?;
    ensure!(
        id.Name()? == NAME
            && id.Publisher()? == PUBLISHER
            && [
                version.Major,
                version.Minor,
                version.Build,
                version.Revision
            ] == [1, 0, 0, 0],
        "wrong fixture identity"
    );
    ensure!(
        !package.IsFramework()? && !package.IsResourcePackage()? && !package.IsBundle()?,
        "wrong package type"
    );
    let family = id.FamilyName()?.to_string();
    ensure!(
        family.starts_with(&format!("{NAME}_"))
            && family
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_')),
        "invalid family"
    );
    let image = PathBuf::from(package.InstalledPath()?.to_string()).join("StoreDataFixture.exe");
    let _pin = PinnedPath::open(&image, false, false)?;
    ensure!(
        format!("{:x}", Sha256::digest(std::fs::read(&image)?)).eq_ignore_ascii_case(FIXTURE_SHA),
        "packaged executable differs from approved build"
    );
    let data = local.join("Packages").join(&family).join("LocalState");
    Ok(Target {
        family,
        full_name: id.FullName()?.to_string(),
        image,
        data,
    })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    phase: String,
    pid: u32,
    creation_time: u64,
    package_full_name: String,
    fixture_sha256: String,
}

fn creation_time(process: &Handle) -> Result<u64> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user)?;
    }
    Ok(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
}

fn open_process(pid: u32, expected: &Target) -> Result<Handle> {
    let process = Handle(unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )?
    });
    ensure!(
        security::user_sid(process.0)? == security::current_sid()?
            && !security::elevated_process(process.0)?,
        "wrong process user or elevation"
    );
    ensure!(
        security::process_session(pid)? == security::process_session(std::process::id())?,
        "wrong activation session"
    );
    ensure!(
        extended(&security::process_image(process.0)?)? == extended(&expected.image)?,
        "activation returned a different executable"
    );
    let mut length = 0;
    ensure!(
        unsafe { GetPackageFullName(process.0, &mut length, None) } == ERROR_INSUFFICIENT_BUFFER
            && (2..=512).contains(&length),
        "activation did not return a package process"
    );
    let mut buffer = vec![0u16; length as usize];
    unsafe { GetPackageFullName(process.0, &mut length, Some(PWSTR(buffer.as_mut_ptr()))) }.ok()?;
    ensure!(
        length >= 2 && length as usize <= buffer.len() && buffer[length as usize - 1] == 0,
        "invalid process package identity"
    );
    ensure!(
        String::from_utf16(&buffer[..length as usize - 1])? == expected.full_name,
        "process package mismatch"
    );
    Ok(process)
}

fn validate_report(report: &Value, phase: &str, family: &str, pid: u32) -> Result<()> {
    for field in [
        "passed",
        "ordinary_user",
        "application_data_api_used",
        "local_settings_readback",
    ] {
        ensure!(report[field] == true, "fixture result missing {field}");
    }
    for field in [
        "shared_local_or_publisher_data_accessed",
        "uninstall_executed",
        "residual_deletion_executed",
        "production_authority_granted",
    ] {
        ensure!(report[field] == false, "unexpected fixture action {field}");
    }
    ensure!(
        report["phase"] == phase
            && report["name"] == NAME
            && report["version"] == "1.0.0.0"
            && report["family"] == family
            && report["process_id"] == pid,
        "stale or unrelated fixture report"
    );
    let folders = report["folders"]
        .as_array()
        .context("missing folder evidence")?;
    ensure!(folders.len() == FOLDERS.len(), "incomplete folder evidence");
    for (leaf, meaning) in FOLDERS {
        let rows: Vec<_> = folders.iter().filter(|row| row["folder"] == leaf).collect();
        ensure!(
            rows.len() == 1 && rows[0]["meaning"] == meaning,
            "missing/duplicate/misclassified folder"
        );
        let row = rows[0];
        if row["present"] == false && leaf == "TempState" && phase == "verify" {
            continue;
        }
        let text = format!("{MARKER}\n{leaf}\nOnly dedicated fixture content.\n");
        ensure!(
            row["present"] == true
                && row["file_bytes"]
                    .as_u64()
                    .is_some_and(|v| v >= text.len() as u64)
                && row["text_sha256"] == format!("{:x}", Sha256::digest(text.as_bytes())),
            "missing or invalid application data"
        );
    }
    Ok(())
}

fn run(phase: &str, observe: bool) -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong controller host"
    );
    ensure!(!security::elevated()?, "ordinary user required");
    ensure!(
        security::process_session(std::process::id())? != 0,
        "interactive session required"
    );
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let root = local.join("DustAwayAcceptance/20260930-software");
    let _pin = PinnedPath::open(&root, true, false)?;
    ensure!(
        extended(
            std::env::current_exe()?
                .parent()
                .context("missing exe root")?
        )? == extended(&root)?,
        "wrong controller directory"
    );
    ensure!(
        read_bounded(&root.join(".dustaway-isolated-acceptance"))?
            == b"DustAway.Acceptance.20260930.7c7e25e8",
        "wrong acceptance marker"
    );
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    let _com = Com;
    let expected = target(&local)?;
    if phase == "preflight" {
        println!(
            "{}",
            json!({"preflight":true,"package_full_name":expected.full_name,"fixture_sha256":FIXTURE_SHA,"activation_executed":false})
        );
        return Ok(());
    }
    let file = |suffix: &str| root.join(format!("msix-data-{phase}-{suffix}.json"));
    ensure!(
        !file("completed").try_exists()?,
        "phase already completed; no replay"
    );
    if phase == "verify" {
        let previous = read_json(&root.join("msix-data-seed-completed.json"))?;
        ensure!(
            previous["passed"] == true
                && previous["exit_code"] == 0
                && previous["receipt"]["package_full_name"] == expected.full_name
                && previous["receipt"]["fixture_sha256"] == FIXTURE_SHA,
            "matching completed seed required"
        );
    }
    let (receipt, process, _release) = if observe {
        // This branch has no activation or event signaling. A missing PID/receipt is
        // uncertain, never permission to relaunch. Archived exit evidence can finish collection.
        let receipt: Receipt = serde_json::from_value(read_json(&file("process"))?)?;
        ensure!(
            receipt.phase == phase
                && receipt.package_full_name == expected.full_name
                && receipt.fixture_sha256 == FIXTURE_SHA,
            "wrong stored activation identity"
        );
        let process = if file("exit").try_exists()? {
            None
        } else {
            let p = open_process(receipt.pid, &expected)?;
            ensure!(
                creation_time(&p)? == receipt.creation_time,
                "PID reused; do not observe another process"
            );
            Some(p)
        };
        (receipt, process, None)
    } else {
        ensure!(
            !expected
                .data
                .join(format!("dustaway-{phase}-started.txt"))
                .try_exists()?,
            "packaged phase already started"
        );
        // Durable intent is saved before activation. A crash cannot authorize a second launch.
        write_new(
            &file("started"),
            &json!({"phase":phase,"package_full_name":expected.full_name,"fixture_sha256":FIXTURE_SHA}),
        )?;
        let name = text_wide(&format!(
            "Local\\DustAway.Msix.Data.20261001.{phase}.Release"
        ));
        let release = Handle(unsafe { CreateEventW(None, true, false, PCWSTR(name.as_ptr()))? });
        ensure!(
            unsafe { GetLastError() } != ERROR_ALREADY_EXISTS,
            "activation event already exists; no concurrent launch"
        );
        let manager: IApplicationActivationManager =
            unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)? };
        let aumid = text_wide(&format!("{}!App", expected.family));
        let arguments = text_wide(phase);
        let pid = unsafe {
            manager.ActivateApplication(
                PCWSTR(aumid.as_ptr()),
                PCWSTR(arguments.as_ptr()),
                AO_NOERRORUI,
            )?
        };
        let process = open_process(pid, &expected)?;
        let receipt = Receipt {
            phase: phase.into(),
            pid,
            creation_time: creation_time(&process)?,
            package_full_name: expected.full_name.clone(),
            fixture_sha256: FIXTURE_SHA.into(),
        };
        write_new(&file("process"), &serde_json::to_value(&receipt)?)?;
        unsafe {
            SetEvent(release.0)?;
        }
        (receipt, Some(process), Some(release))
    };
    let exit = if let Some(process) = process {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            match unsafe { WaitForSingleObject(process.0, 1_000) } {
                WAIT_OBJECT_0 => break,
                WAIT_TIMEOUT if Instant::now() < deadline => {}
                WAIT_TIMEOUT => anyhow::bail!(
                    "same fixture still running; preserve state and observe, never relaunch"
                ),
                _ => anyhow::bail!("process observation failed; state uncertain"),
            }
        }
        let mut code = 0;
        unsafe {
            GetExitCodeProcess(process.0, &mut code)?;
        }
        let value =
            json!({"pid":receipt.pid,"creation_time":receipt.creation_time,"exit_code":code});
        write_new(&file("exit"), &value)?;
        value
    } else {
        read_json(&file("exit"))?
    };
    ensure!(
        exit["pid"] == receipt.pid
            && exit["creation_time"] == receipt.creation_time
            && exit["exit_code"] == 0,
        "unverified or unsuccessful fixture exit; keep original evidence"
    );
    let report_path = expected.data.join(format!("dustaway-{phase}-report.json"));
    let report_bytes = read_bounded(&report_path)?;
    let report: Value = serde_json::from_slice(
        report_bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(&report_bytes),
    )?;
    validate_report(&report, phase, &expected.family, receipt.pid)?;
    let completed = json!({"passed":true,"receipt":receipt,"exit_code":0,"report":report,"report_sha256":format!("{:x}",Sha256::digest(&report_bytes)),"observation_only":observe,"uninstall_executed":false,"residual_deletion_executed":false,"production_authority_granted":false});
    write_new(&file("completed"), &completed)?;
    println!("{}", serde_json::to_string_pretty(&completed)?);
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (phase, observe) = arguments(&args)?;
    run(phase, observe)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report() -> Value {
        json!({"phase":"verify","passed":true,"ordinary_user":true,"application_data_api_used":true,"local_settings_readback":true,"shared_local_or_publisher_data_accessed":false,"uninstall_executed":false,"residual_deletion_executed":false,"production_authority_granted":false,"name":NAME,"version":"1.0.0.0","family":"fixture_family","process_id":123,"folders":FOLDERS.map(|(leaf,meaning)|{let text=format!("{MARKER}\n{leaf}\nOnly dedicated fixture content.\n");json!({"folder":leaf,"meaning":meaning,"present":true,"file_bytes":text.len(),"text_sha256":format!("{:x}",Sha256::digest(text.as_bytes()))})})})
    }
    #[test]
    fn stale_and_incomplete_reports_cannot_pass() {
        let good = report();
        assert!(validate_report(&good, "verify", "fixture_family", 123).is_ok());
        assert!(validate_report(&good, "verify", "fixture_family", 124).is_err());
        assert!(validate_report(&good, "seed", "fixture_family", 123).is_err());
        let mut missing = good.clone();
        missing["folders"].as_array_mut().unwrap().pop();
        assert!(validate_report(&missing, "verify", "fixture_family", 123).is_err());
        let mut duplicate = good.clone();
        duplicate["folders"][0] = good["folders"][1].clone();
        assert!(validate_report(&duplicate, "verify", "fixture_family", 123).is_err());
        let mut lost = good;
        lost["folders"][0]["present"] = json!(false);
        assert!(validate_report(&lost, "verify", "fixture_family", 123).is_err());
    }
    #[test]
    fn temporary_loss_is_only_accepted_in_verification() {
        let mut value = report();
        value["folders"][1] =
            json!({"folder":"TempState","meaning":"temporary_cache","present":false});
        assert!(validate_report(&value, "verify", "fixture_family", 123).is_ok());
        value["phase"] = json!("seed");
        assert!(validate_report(&value, "seed", "fixture_family", 123).is_err());
    }
    #[test]
    fn intent_is_exclusive_and_cannot_overwrite_original_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("intent.json");
        write_new(&path, &json!({"attempt":1})).unwrap();
        assert!(write_new(&path, &json!({"attempt":2})).is_err());
        assert_eq!(read_json(&path).unwrap()["attempt"], 1);
    }
    #[test]
    fn arguments_never_accept_a_package_path_or_implicit_retry() {
        assert_eq!(
            arguments(&["observe".into(), "seed".into()]).unwrap(),
            ("seed", true)
        );
        for args in [
            vec![],
            vec!["retry"],
            vec!["seed", "C:\\private"],
            vec!["observe", "Another.App"],
        ] {
            assert!(arguments(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>()).is_err());
        }
    }
}
