use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

/// Извлечь иконку из exe/lnk и сохранить в `out_path` как PNG.
pub(super) fn extract_to_png(exec_path: &str, out_path: &Path) -> std::io::Result<()> {
    // .lnk → resolve target, prefer icon_location если выставлен.
    let (icon_source, icon_index) = resolve_icon_source(exec_path);

    let hicon = unsafe { extract_hicon(&icon_source, icon_index) }
        .ok_or_else(|| std::io::Error::other("ExtractIconExW: no icon"))?;
    let res = unsafe { hicon_to_png(hicon, out_path) };
    unsafe {
        let _ = DestroyIcon(hicon);
    }
    res
}

/// Same as extract_to_png, но принимает .lnk path отдельно от target.
/// Стратегия:
///   1) Попробовать .lnk::icon_location() (для Squirrel apps типа Discord)
///   2) Попробовать target exe напрямую (Win32 .exe с embedded icons)
pub(super) fn extract_to_png_with_lnk(
    lnk_path: &Path,
    target_path: &str,
    out_path: &Path,
) -> std::io::Result<()> {
    // Стратегия 1: .lnk::icon_location
    if let Ok(shell_link) = lnk::ShellLink::open(lnk_path) {
        let icon_index = shell_link.header().icon_index();
        if let Some(loc) = shell_link.icon_location().as_ref() {
            if !loc.is_empty() {
                let expanded = expand_env(loc);
                if try_extract_and_save(&expanded, icon_index, out_path).is_ok() {
                    return Ok(());
                }
            }
        }
    }
    // Стратегия 2: target exe
    try_extract_and_save(target_path, 0, out_path)
}

/// Helper: extract HICON → PNG, единым шагом с cleanup.
fn try_extract_and_save(source: &str, icon_index: i32, out_path: &Path) -> std::io::Result<()> {
    let hicon = unsafe { extract_hicon(source, icon_index) }
        .ok_or_else(|| std::io::Error::other("ExtractIconExW: no icon"))?;

    // Гарантируем DestroyIcon на любом exit path.
    let res = unsafe { hicon_to_png(hicon, out_path) };
    unsafe {
        let _ = DestroyIcon(hicon);
    }
    res
}

/// Возвращает (path-to-icon-source, icon-index).
/// Для .lnk пытается icon_location → fallback к target.
fn resolve_icon_source(exec_path: &str) -> (String, i32) {
    if exec_path.to_ascii_lowercase().ends_with(".lnk") {
        if let Ok(shell_link) = lnk::ShellLink::open(exec_path) {
            let icon_index = shell_link.header().icon_index();
            if let Some(loc) = shell_link.icon_location().as_ref() {
                if !loc.is_empty() {
                    return (expand_env(loc), icon_index);
                }
            }
            // Fallback: target.exe via link_info.local_base_path()
            if let Some(info) = shell_link.link_info().as_ref() {
                if let Some(p) = info
                    .local_base_path_unicode()
                    .as_ref()
                    .or(info.local_base_path().as_ref())
                {
                    if !p.is_empty() {
                        return (expand_env(p), icon_index);
                    }
                }
            }
            // Последний fallback — relative_path относительно директории .lnk.
            if let Some(rel) = shell_link.relative_path().as_ref() {
                if !rel.is_empty() {
                    let base = Path::new(exec_path).parent().unwrap_or(Path::new("."));
                    let joined = base.join(rel);
                    return (joined.to_string_lossy().to_string(), icon_index);
                }
            }
        }
    }
    (exec_path.to_string(), 0)
}

/// Простейшая expand %VAR% (без полной ExpandEnvironmentStringsW —
/// нам достаточно %SystemRoot% / %ProgramFiles% типового).
fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('%') {
            let var = &after[..end];
            match std::env::var(var) {
                Ok(val) => out.push_str(&val),
                Err(_) => {
                    out.push('%');
                    out.push_str(var);
                    out.push('%');
                }
            }
            rest = &after[end + 1..];
        } else {
            out.push('%');
            out.push_str(after);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

unsafe fn extract_hicon(path: &str, icon_index: i32) -> Option<HICON> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut large: HICON = HICON::default();
    let pcwstr = PCWSTR(wide.as_ptr());
    // Берём только large icon (32×32 на classic, scales by DPI).
    let n = ExtractIconExW(pcwstr, icon_index, Some(&mut large as *mut _), None, 1);
    if n == 0 || large.is_invalid() {
        return None;
    }
    Some(large)
}

unsafe fn hicon_to_png(hicon: HICON, out_path: &Path) -> std::io::Result<()> {
    let mut info = ICONINFO::default();
    GetIconInfo(hicon, &mut info as *mut _)
        .map_err(|e| std::io::Error::other(format!("GetIconInfo: {e}")))?;

    // Guard для GDI bitmap handles.
    struct BitmapGuards {
        color: windows::Win32::Graphics::Gdi::HBITMAP,
        mask: windows::Win32::Graphics::Gdi::HBITMAP,
    }
    impl Drop for BitmapGuards {
        fn drop(&mut self) {
            unsafe {
                if !self.color.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(self.color.0));
                }
                if !self.mask.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(self.mask.0));
                }
            }
        }
    }
    let _guard = BitmapGuards {
        color: info.hbmColor,
        mask: info.hbmMask,
    };

    if info.hbmColor.is_invalid() {
        // Monochrome icon — пропускаем (редко, fallback caller'а).
        return Err(std::io::Error::other(
            "icon has no color bitmap (monochrome)",
        ));
    }

    // BITMAP info — width/height.
    let mut bm = BITMAP::default();
    let got = GetObjectW(
        HGDIOBJ(info.hbmColor.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    );
    if got == 0 {
        return Err(std::io::Error::other("GetObjectW(BITMAP) failed"));
    }

    let width = bm.bmWidth;
    let height = bm.bmHeight;
    if width <= 0 || height <= 0 {
        return Err(std::io::Error::other("bitmap has zero dimensions"));
    }

    // BITMAPINFOHEADER: top-down 32bpp BI_RGB.
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // negative => top-down DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        ..Default::default()
    };

    let pixel_count = (width as usize) * (height as usize);
    let mut buf: Vec<u8> = vec![0u8; pixel_count * 4];

    // ScreenDC — компatible с экраном, для GetDIBits достаточно.
    let hdc = GetDC(None);
    if hdc.is_invalid() {
        return Err(std::io::Error::other("GetDC failed"));
    }

    let lines = GetDIBits(
        hdc,
        info.hbmColor,
        0,
        height as u32,
        Some(buf.as_mut_ptr() as *mut _),
        &mut bmi as *mut _,
        DIB_RGB_COLORS,
    );

    ReleaseDC(None, hdc);

    if lines == 0 {
        return Err(std::io::Error::other("GetDIBits returned 0 lines"));
    }

    // BGRA → RGBA swap. Если alpha-channel везде 0 (некоторые legacy icons),
    // GetDIBits не вытащит alpha — подкручиваем до 0xFF чтобы PNG не был
    // полностью прозрачным.
    let mut alpha_seen = false;
    for px in buf.as_chunks_mut::<4>().0 {
        let b = px[0];
        let r = px[2];
        px[0] = r;
        px[2] = b;
        if px[3] != 0 {
            alpha_seen = true;
        }
    }
    if !alpha_seen {
        for px in buf.as_chunks_mut::<4>().0 {
            px[3] = 0xFF;
        }
    }

    let image_buf =
        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width as u32, height as u32, buf)
            .ok_or_else(|| std::io::Error::other("ImageBuffer::from_raw failed"))?;

    image_buf
        .save_with_format(out_path, image::ImageFormat::Png)
        .map_err(|e| std::io::Error::other(format!("png save: {e}")))?;

    Ok(())
}
