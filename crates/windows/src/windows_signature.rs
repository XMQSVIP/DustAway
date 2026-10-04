//! Authenticode verification for Windows components signed through system catalogs.
use crate::fs::{Handle, PinnedPath, stamp, text_wide, wide};
use anyhow::{Result, ensure};
use windows::{
    Win32::{
        Foundation::{GENERIC_READ, HWND},
        Security::{
            Cryptography::{Catalog::*, *},
            WinTrust::*,
        },
        Storage::FileSystem::*,
    },
    core::{PCWSTR, w},
};

struct CatalogAdmin(isize);
impl Drop for CatalogAdmin {
    fn drop(&mut self) {
        unsafe {
            let _ = CryptCATAdminReleaseContext(self.0, 0);
        }
    }
}
struct Catalog {
    admin: isize,
    handle: isize,
}
impl Drop for Catalog {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                let _ = CryptCATAdminReleaseCatalogContext(self.admin, self.handle, 0);
            }
        }
    }
}

pub(crate) fn microsoft(pin: &PinnedPath) -> bool {
    const PUBLISHERS: &[&str] = &[
        "Microsoft Windows",
        "Microsoft Windows Publisher",
        "Microsoft Corporation",
    ];
    if PUBLISHERS
        .iter()
        .any(|name| crate::applications::signed_by(pin, name))
    {
        return true;
    }
    catalog_verified(pin).unwrap_or(false)
}
fn catalog_verified(pin: &PinnedPath) -> Result<bool> {
    let path = wide(&pin.path);
    // Read the same pinned file. The additional read handle also disallows writes/deletion.
    let file = Handle(unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )?
    });
    ensure!(stamp(&file)? == pin.stamp, "签名核验期间文件发生变化");
    let mut raw = 0;
    unsafe {
        CryptCATAdminAcquireContext2(&mut raw, None, w!("SHA256"), None, None)?;
    }
    let admin = CatalogAdmin(raw);
    let mut length = 0;
    unsafe {
        CryptCATAdminCalcHashFromFileHandle2(admin.0, file.0, &mut length, None, None)?;
    }
    ensure!(length > 0 && length <= 128, "目录签名摘要长度无效");
    let mut hash = vec![0; length as usize];
    unsafe {
        CryptCATAdminCalcHashFromFileHandle2(
            admin.0,
            file.0,
            &mut length,
            Some(hash.as_mut_ptr()),
            None,
        )?;
    }
    let tag = text_wide(&hash.iter().map(|v| format!("{v:02X}")).collect::<String>());
    let mut catalog = Catalog {
        admin: admin.0,
        handle: unsafe { CryptCATAdminEnumCatalogFromHash(admin.0, &hash, None, None) },
    };
    while catalog.handle != 0 {
        let mut info = CATALOG_INFO {
            cbStruct: size_of::<CATALOG_INFO>() as u32,
            ..Default::default()
        };
        unsafe {
            CryptCATCatalogInfoFromContext(catalog.handle, &mut info, 0)?;
        }
        let mut member = WINTRUST_CATALOG_INFO {
            cbStruct: size_of::<WINTRUST_CATALOG_INFO>() as u32,
            pcwszCatalogFilePath: PCWSTR(info.wszCatalogFile.as_ptr()),
            pcwszMemberTag: PCWSTR(tag.as_ptr()),
            pcwszMemberFilePath: PCWSTR(path.as_ptr()),
            hMemberFile: file.0,
            pbCalculatedFileHash: hash.as_mut_ptr(),
            cbCalculatedFileHash: length,
            hCatAdmin: admin.0,
            ..Default::default()
        };
        let mut data = WINTRUST_DATA {
            cbStruct: size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_CATALOG,
            Anonymous: WINTRUST_DATA_0 {
                pCatalog: &mut member,
            },
            dwStateAction: WTD_STATEACTION_VERIFY,
            dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL | WTD_REVOCATION_CHECK_NONE,
            ..Default::default()
        };
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let mut valid = false;
        unsafe {
            if WinVerifyTrust(
                HWND::default(),
                &mut action,
                (&mut data as *mut WINTRUST_DATA).cast(),
            ) == 0
            {
                let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
                if !provider.is_null() {
                    let signer = WTHelperGetProvSignerFromChain(provider, 0, false, 0);
                    if !signer.is_null()
                        && (*signer).csCertChain > 0
                        && !(*signer).pasCertChain.is_null()
                    {
                        let mut name = [0u16; 1024];
                        let count = CertGetNameStringW(
                            (*(*signer).pasCertChain).pCert,
                            CERT_NAME_SIMPLE_DISPLAY_TYPE,
                            0,
                            None,
                            Some(&mut name),
                        ) as usize;
                        valid = count > 1
                            && count <= name.len()
                            && matches!(
                                String::from_utf16_lossy(&name[..count - 1]).as_str(),
                                "Microsoft Windows"
                                    | "Microsoft Windows Publisher"
                                    | "Microsoft Corporation"
                            );
                    }
                }
            }
            data.dwStateAction = WTD_STATEACTION_CLOSE;
            let _ = WinVerifyTrust(
                HWND::default(),
                &mut action,
                (&mut data as *mut WINTRUST_DATA).cast(),
            );
        }
        if valid {
            return Ok(true);
        }
        catalog.handle = unsafe {
            CryptCATAdminEnumCatalogFromHash(admin.0, &hash, None, Some(&mut catalog.handle))
        };
    }
    Ok(false)
}
