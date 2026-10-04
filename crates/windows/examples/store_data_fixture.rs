//! Packaged application for isolated MSIX data acceptance. Never a product command.
//! Uses its own ApplicationData APIs; no arbitrary package, path, or deletion operation.
#![windows_subsystem = "windows"]
use anyhow::{Result, ensure};
use dustaway_windows::{
    fs::{Handle, PinnedPath, extended, text_wide},
    platform::known_folder,
    security,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, process::ExitCode};
use windows::{
    ApplicationModel::Package,
    Foundation::{IPropertyValue, PropertyValue},
    Storage::{ApplicationData, CreationCollisionOption, FileIO, StorageFolder},
    Win32::{
        Foundation::WAIT_OBJECT_0,
        System::Threading::{OpenEventW, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject},
        System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
        UI::Shell::FOLDERID_LocalAppData,
    },
    core::{HSTRING, Interface, PCWSTR},
};

const NAME: &str = "DustAway.Acceptance.Data.20261001";
const PUBLISHER: &str = "CN=DustAway Data Acceptance 20261001";
const MARKER: &str = "DustAway.Msix.Data.20261001.7c7e25e8";
const FILE: &str = "dustaway-fixture-note.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Seed,
    Verify,
}
impl Phase {
    fn parse(args: &[String]) -> Result<Self> {
        match args {
            [phase] if phase == "seed" => Ok(Self::Seed),
            [phase] if phase == "verify" => Ok(Self::Verify),
            _ => anyhow::bail!("exactly one seed or verify phase required; no path arguments"),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Verify => "verify",
        }
    }
}

struct Runtime;
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

fn validate_identity(name: &str, publisher: &str, version: [u16; 4]) -> Result<()> {
    ensure!(
        name == NAME && publisher == PUBLISHER && version == [1, 0, 0, 0],
        "not the exact dedicated MSIX identity"
    );
    Ok(())
}

fn write_new(folder: &StorageFolder, name: &str, text: &str) -> Result<()> {
    let file = folder
        .CreateFileAsync(&HSTRING::from(name), CreationCollisionOption::FailIfExists)?
        .join()?;
    FileIO::WriteTextAsync(&file, &HSTRING::from(text))?.join()?;
    ensure!(
        FileIO::ReadTextAsync(&file)?.join()? == text,
        "application data readback differs"
    );
    Ok(())
}

fn folder_path(folder: &StorageFolder, root: &std::path::Path, leaf: &str) -> Result<PathBuf> {
    let path = PathBuf::from(folder.Path()?.to_string());
    ensure!(
        extended(&path)? == extended(&root.join(leaf))?,
        "ApplicationData returned an unexpected location"
    );
    let _pin = PinnedPath::open(&path, true, false)?;
    Ok(path)
}

fn run(phase: Phase) -> Result<()> {
    // These checks happen before WinRT activation or writes, including error reports.
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong fixture host"
    );
    ensure!(!security::elevated()?, "ordinary user required");
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let control = local.join("DustAwayAcceptance/20260930-software");
    let _control_pin = PinnedPath::open(&control, true, false)?;
    ensure!(
        std::fs::read_to_string(control.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "isolated acceptance marker missing"
    );
    unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
    let _runtime = Runtime;
    let package = Package::Current()?;
    let identity = package.Id()?;
    let version = identity.Version()?;
    validate_identity(
        &identity.Name()?.to_string(),
        &identity.Publisher()?.to_string(),
        [
            version.Major,
            version.Minor,
            version.Build,
            version.Revision,
        ],
    )?;
    let install = PathBuf::from(package.InstalledPath()?.to_string());
    ensure!(
        extended(&std::env::current_exe()?)? == extended(&install.join("StoreDataFixture.exe"))?,
        "executable does not belong to the installed fixture"
    );
    let family = identity.FamilyName()?.to_string();
    ensure!(
        family.starts_with(&format!("{NAME}_"))
            && family
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_')),
        "invalid fixture package family"
    );
    let root = local.join("Packages").join(&family);
    // The controller retains the activation PID before releasing this event. Without
    // this handshake a short-lived successful app could disappear before OpenProcess.
    // The fixture never creates/signals the event and cannot control another process.
    let release_name = text_wide(&format!(
        "Local\\DustAway.Msix.Data.20261001.{}.Release",
        phase.name()
    ));
    let release = Handle(unsafe {
        OpenEventW(
            SYNCHRONIZATION_SYNCHRONIZE,
            false,
            PCWSTR(release_name.as_ptr()),
        )?
    });
    ensure!(
        unsafe { WaitForSingleObject(release.0, 120_000) } == WAIT_OBJECT_0,
        "controller did not acknowledge this process; data generation not started"
    );
    let data = ApplicationData::Current()?;
    let local_folder = data.LocalFolder()?;
    folder_path(&local_folder, &root, "LocalState")?;
    // The report lives in this package's data; the controller must archive it before uninstall.
    // Never disable filesystem virtualization merely to write an external report.
    write_new(
        &local_folder,
        &format!("dustaway-{}-started.txt", phase.name()),
        MARKER,
    )?;
    let result = (|| -> Result<serde_json::Value> {
        let settings = data.LocalSettings()?.Values()?;
        let key = HSTRING::from("DustAwayFixtureOwner");
        let value = HSTRING::from(MARKER);
        if phase == Phase::Seed {
            ensure!(
                !settings.HasKey(&key)?,
                "fixture settings already exist; no replay"
            );
            ensure!(
                !settings.Insert(&key, &PropertyValue::CreateString(&value)?)?,
                "settings replaced"
            );
        }
        ensure!(
            settings
                .Lookup(&key)?
                .cast::<IPropertyValue>()?
                .GetString()?
                == MARKER,
            "application settings did not persist"
        );
        let mut folders = vec![];
        for (leaf, folder, meaning) in [
            (
                "LocalCache",
                data.LocalCacheFolder()?,
                "persistent_local_data",
            ),
            ("TempState", data.TemporaryFolder()?, "temporary_cache"),
            ("LocalState", local_folder.clone(), "local_data"),
            ("RoamingState", data.RoamingFolder()?, "roaming_data"),
        ] {
            let path = folder_path(&folder, &root, leaf)?;
            let text = format!("{MARKER}\n{leaf}\nOnly dedicated fixture content.\n");
            if phase == Phase::Seed {
                write_new(&folder, FILE, &text)?;
            }
            // Windows may discard temporary files between launches. Report that absence;
            // do not recreate it during verification or count it as deletion evidence.
            if phase == Phase::Verify && leaf == "TempState" && !path.join(FILE).try_exists()? {
                folders.push(json!({"folder":leaf, "meaning":meaning, "present":false}));
                continue;
            }
            let file = folder.GetFileAsync(&HSTRING::from(FILE))?.join()?;
            ensure!(
                FileIO::ReadTextAsync(&file)?.join()? == text,
                "fixture content changed"
            );
            folders.push(json!({
                "folder":leaf, "meaning":meaning, "present":true,
                "file_bytes":file.GetBasicPropertiesAsync()?.join()?.Size()?,
                "text_sha256":format!("{:x}", Sha256::digest(text.as_bytes())),
            }));
        }
        Ok(json!({
            "phase":phase.name(), "passed":true, "ordinary_user":true,
            "name":NAME, "version":"1.0.0.0", "family":family,
            "process_id":std::process::id(), "application_data_api_used":true,
            "local_settings_readback":true, "folders":folders,
            "shared_local_or_publisher_data_accessed":false,
            "uninstall_executed":false, "residual_deletion_executed":false,
            "production_authority_granted":false,
        }))
    })();
    match result {
        Ok(report) => write_new(
            &local_folder,
            &format!("dustaway-{}-report.json", phase.name()),
            &serde_json::to_string_pretty(&report)?,
        )?,
        Err(error) => {
            let _ = write_new(
                &local_folder,
                &format!("dustaway-{}-failure.json", phase.name()),
                &json!({"phase":phase.name(),"passed":false,"error":error.to_string()}).to_string(),
            );
            return Err(error);
        }
    }
    data.Close()?;
    Ok(())
}

fn main() -> ExitCode {
    let result = Phase::parse(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(run);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("MSIX fixture refused or failed: {error:#}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_or_updated_packages_cannot_use_the_data_fixture() {
        assert!(validate_identity(NAME, PUBLISHER, [1, 0, 0, 0]).is_ok());
        assert!(validate_identity("Another.App", PUBLISHER, [1, 0, 0, 0]).is_err());
        assert!(validate_identity(NAME, "CN=Another Publisher", [1, 0, 0, 0]).is_err());
        assert!(validate_identity(NAME, PUBLISHER, [1, 0, 0, 1]).is_err());
    }
    #[test]
    fn activation_cannot_supply_a_path_package_or_extra_phase() {
        for args in [
            vec![],
            vec!["clean"],
            vec!["seed", "C:\\private"],
            vec!["seed", "verify"],
        ] {
            assert!(Phase::parse(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>()).is_err());
        }
        assert_eq!(Phase::parse(&["seed".into()]).unwrap(), Phase::Seed);
        assert_eq!(Phase::parse(&["verify".into()]).unwrap(), Phase::Verify);
    }
}
