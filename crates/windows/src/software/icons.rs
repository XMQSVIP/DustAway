//! Display-only icon extraction. Never execute a registered icon path or expose it to the webview.
use super::{Registration, open_key, registered_executable_command, string};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{mem::size_of, path::Path};
use windows::{
    ApplicationModel::Package,
    Foundation::Size,
    Storage::Streams::DataReader,
    Win32::{
        Graphics::Gdi::*,
        System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_SAM_FLAGS},
        UI::{Shell::SHDefExtractIconW, WindowsAndMessaging::*},
    },
    core::PCWSTR,
};

const SIZE: u32 = 64;
const MAX_LOGO_BYTES: u64 = 512 * 1024;

pub(super) fn desktop(registration: &Registration) -> Option<String> {
    let registered = (|| -> Result<String> {
        let hive = if registration.hive == "user" {
            HKEY_CURRENT_USER
        } else {
            HKEY_LOCAL_MACHINE
        };
        let key = open_key(hive, &registration.key, REG_SAM_FLAGS(registration.view))?
            .context("Missing software registration")?;
        let value = string(&key, "DisplayIcon")?.context("Missing registered icon")?;
        let (path, index) = icon_location(&value).context("Invalid icon location")?;
        extract(&path, index)
    })();
    registered.ok().or_else(|| {
        // Only use an explicit executable already present in the uninstall record as a fallback.
        // MSI's shared system host is not the application's icon.
        if registration.msi {
            return None;
        }
        let (path, _) = registered_executable_command(&registration.command).ok()?;
        extract(&path, 0).ok()
    })
}

fn icon_location(value: &str) -> Option<(String, i32)> {
    let value = value.trim();
    let (path, index) = match value.rsplit_once(',') {
        Some((path, index)) if index.trim().parse::<i32>().is_ok() => {
            (path.trim(), index.trim().parse().ok()?)
        }
        _ => (value, 0),
    };
    let path = path
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(path);
    local_path(path).then(|| (path.to_owned(), index))
}

fn local_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    // Reject relative paths, UNC/device paths and URLs before accessing the file.
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
        && !path.contains(['"', '\0'])
}

struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.0);
        }
    }
}

struct Canvas {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

fn extract(path: &str, index: i32) -> Result<String> {
    ensure!(local_path(path), "Invalid local icon path");
    ensure!(Path::new(path).is_file(), "Missing icon file");
    let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut handle = HICON::default();
    unsafe {
        SHDefExtractIconW(
            PCWSTR(path.as_ptr()),
            index,
            0,
            Some(&mut handle),
            None,
            SIZE,
        )
        .ok()?;
    }
    ensure!(!handle.is_invalid(), "No icon resource");
    let icon = Icon(handle);
    // Drawing against black and white preserves both legacy AND masks and alpha icons.
    let black = render(&icon, 0)?;
    let white = render(&icon, 255)?;
    let rgba = rgba_from_backgrounds(&black, &white);
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, SIZE, SIZE);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
    }
    Ok(data_url(&bytes))
}

fn render(icon: &Icon, background: u8) -> Result<Vec<u8>> {
    unsafe {
        let dc = CreateCompatibleDC(None);
        ensure!(!dc.is_invalid(), "Cannot create icon canvas");
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE as i32,
                biHeight: -(SIZE as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => bitmap,
            Err(error) => {
                let _ = DeleteDC(dc);
                return Err(error.into());
            }
        };
        let canvas = Canvas {
            dc,
            bitmap,
            previous: SelectObject(dc, bitmap.into()),
        };
        ensure!(
            !bits.is_null() && !canvas.previous.is_invalid(),
            "Cannot select icon bitmap"
        );
        let len = (SIZE * SIZE * 4) as usize;
        std::ptr::write_bytes(bits.cast::<u8>(), background, len);
        DrawIconEx(
            dc,
            0,
            0,
            icon.0,
            SIZE as i32,
            SIZE as i32,
            0,
            None,
            DI_NORMAL,
        )?;
        GdiFlush().ok()?;
        Ok(std::slice::from_raw_parts(bits.cast::<u8>(), len).to_vec())
    }
}

fn rgba_from_backgrounds(black: &[u8], white: &[u8]) -> Vec<u8> {
    black
        .chunks_exact(4)
        .zip(white.chunks_exact(4))
        .flat_map(|(b, w)| {
            let alpha = 255 - w[0].saturating_sub(b[0]);
            let channel = |i: usize| {
                if alpha == 0 {
                    0
                } else {
                    ((u32::from(b[i]) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255)
                        as u8
                }
            };
            [channel(2), channel(1), channel(0), alpha]
        })
        .collect()
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

pub(super) fn store(package: &Package) -> Result<String> {
    let reference = package.GetLogoAsRandomAccessStreamReference(Size {
        Width: SIZE as f32,
        Height: SIZE as f32,
    })?;
    let stream = reference.OpenReadAsync()?.join()?;
    let size = stream.Size()?;
    ensure!(
        size > 0 && size <= MAX_LOGO_BYTES,
        "Package logo exceeds size limit"
    );
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    let loaded = reader.LoadAsync(size as u32)?.join()?;
    ensure!(loaded == size as u32, "Incomplete package logo");
    let mut bytes = vec![0; size as usize];
    reader.ReadBytes(&mut bytes)?;
    ensure!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Unsupported package logo format"
    );
    Ok(data_url(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_quoted_comma_paths_and_signed_resource_indices() {
        assert_eq!(
            icon_location(r#" "C:\Apps\Example, Inc\app.exe", -12 "#),
            Some((r"C:\Apps\Example, Inc\app.exe".into(), -12))
        );
        assert_eq!(
            icon_location(r"C:\Apps\app.ico"),
            Some((r"C:\Apps\app.ico".into(), 0))
        );
        for value in [
            r"\\server\app.ico",
            "https://example.com/icon.png",
            "app.exe",
            r#""C:\app.exe" --run"#,
            "",
        ] {
            assert!(icon_location(value).is_none(), "{value}");
        }
    }
    #[test]
    fn reconstructs_transparency_and_unpremultiplies_color() {
        assert_eq!(
            rgba_from_backgrounds(&[0, 0, 0, 0], &[255, 255, 255, 0]),
            [0, 0, 0, 0]
        );
        assert_eq!(
            rgba_from_backgrounds(&[30, 20, 10, 0], &[30, 20, 10, 0]),
            [10, 20, 30, 255]
        );
        assert_eq!(
            rgba_from_backgrounds(&[0, 0, 128, 0], &[127, 127, 255, 0]),
            [255, 0, 0, 128]
        );
    }
    #[test]
    fn extracts_a_real_windows_icon_as_png() -> Result<()> {
        let path = super::super::system_installer_path()?;
        let icon = extract(&path.to_string_lossy(), 0)?;
        let bytes = STANDARD.decode(icon.strip_prefix("data:image/png;base64,").unwrap())?;
        let reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info()?;
        assert_eq!((reader.info().width, reader.info().height), (SIZE, SIZE));
        Ok(())
    }
}
