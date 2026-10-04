//! Read-only field-level probe. Logs indices/errors, never package names or paths.
use anyhow::Result;
use windows::{
    Management::Deployment::{PackageManager, PackageTypes},
    Win32::System::Com::*,
    core::HSTRING,
};
fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    let manager = PackageManager::new()?;
    let items = manager
        .FindPackagesByUserSecurityIdWithPackageTypes(&HSTRING::new(), PackageTypes::Main)?
        .First()?;
    let mut n = 0;
    while items.HasCurrent()? {
        let p = items.Current()?;
        macro_rules! probe {
            ($field:ident) => {
                if let Err(e) = p.$field() {
                    println!("index={n} field={} error={e:?}", stringify!($field));
                }
            };
        }
        probe!(IsFramework);
        probe!(IsResourcePackage);
        probe!(IsBundle);
        probe!(Id);
        probe!(SignatureKind);
        probe!(DisplayName);
        probe!(PublisherDisplayName);
        probe!(InstalledDate);
        probe!(InstalledPath);
        if let Ok(id) = p.Id() {
            if let Err(e) = id.FullName() {
                println!("index={n} full_name={e:?}");
            }
            if let Err(e) = id.FamilyName() {
                println!("index={n} family_name={e:?}");
            }
            if let Err(e) = id.Version() {
                println!("index={n} version={e:?}");
            }
        }
        n += 1;
        items.MoveNext()?;
    }
    println!("probed_packages={n}");
    unsafe {
        CoUninitialize();
    }
    Ok(())
}
