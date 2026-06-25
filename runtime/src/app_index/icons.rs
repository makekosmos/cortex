// Icon extraction + PNG cache.
//
// Win32: ExtractIconExW (or через .lnk → target.exe) → HICON → GetIconInfo →
// GDI DIB → BGRA→RGBA swap → `image` crate PNG encode → файл в icon cache dir.
//
// UWP: Package.GetLogo() → RandomAccessStreamReference → OpenReadAsync →
// DataReader → bytes (обычно уже PNG) → файл.
//
// Storage: `<icon_cache_dir>/<sha256(exec_path)[..16]>.png`. Идемпотентно —
// если файл существует, не пересоздаём.
//
// Discovery не пишет PNG. Sources сохраняют `App.icon_source`, а `AppIndex::rescan`
// вызывает `ensure_icon` последовательным throttled loop'ом.

use crate::app_index::app::{App, AppKind, IconSource};
use crate::app_index::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub fn cached_icon_path(cache_dir: &Path, exec_path: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(exec_path.as_bytes());
    let hash = hex_short(&hasher.finalize());
    cache_dir.join(format!("{hash}.png"))
}

fn hex_short(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(32);
    for b in &bytes[..8] {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Гарантировать наличие иконки на диске. Возвращает путь.
/// Если файл уже существует — no-op (быстро).
/// При любой ошибке экстрактора — возвращает Err БЕЗ записи placeholder,
/// чтобы следующий rescan повторил попытку (UI рендерит placeholder в DOM).
pub fn ensure_icon(cache_dir: &Path, app: &App) -> Result<String> {
    let path = cached_icon_path(cache_dir, &app.exec_path);
    if path.exists() {
        return Ok(path.to_string_lossy().to_string());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match app.kind {
        AppKind::Win32 => {
            #[cfg(target_os = "windows")]
            {
                if let Some(IconSource::StartMenuLnk {
                    lnk_path,
                    target_path,
                }) = app.icon_source.as_ref()
                {
                    return ensure_icon_for_lnk(cache_dir, Path::new(lnk_path), target_path);
                }
                win32::extract_to_png(&app.exec_path, &path).map_err(|e| {
                    tracing::debug!(
                        target: "app_index::icons",
                        exec_path = %app.exec_path,
                        error = %e,
                        "win32 icon extraction failed"
                    );
                    crate::app_index::AppIndexError::Other(format!("icon extraction: {e}"))
                })?;
                Ok(path.to_string_lossy().to_string())
            }
            #[cfg(not(target_os = "windows"))]
            {
                Err(crate::app_index::AppIndexError::Other(
                    "Win32 icon extraction only on Windows".into(),
                ))
            }
        }
        AppKind::Uwp => {
            #[cfg(target_os = "windows")]
            {
                if let Some(IconSource::UwpPackage { package_full_name }) = app.icon_source.as_ref()
                {
                    return ensure_icon_for_uwp_package(
                        cache_dir,
                        &app.exec_path,
                        package_full_name,
                    );
                }
                Err(crate::app_index::AppIndexError::Other(
                    "UWP icon extraction missing package metadata".into(),
                ))
            }
            #[cfg(not(target_os = "windows"))]
            {
                Err(crate::app_index::AppIndexError::Other(
                    "UWP icon extraction only on Windows".into(),
                ))
            }
        }
        _ => Err(crate::app_index::AppIndexError::Other(format!(
            "icon extraction not implemented for kind: {:?}",
            app.kind
        ))),
    }
}

/// Win32 икон-extractor с приоритетом .lnk::icon_location над target.
/// Используется lazy из throttled icon queue — это спасает
/// Squirrel-installer apps (Discord, Slack, Teams), где target=Update.exe
/// без иконки, а icon_location в .lnk указывает на реальный exe.
#[cfg(target_os = "windows")]
pub fn ensure_icon_for_lnk(cache_dir: &Path, lnk_path: &Path, target_path: &str) -> Result<String> {
    let out = cached_icon_path(cache_dir, target_path);
    if out.exists() {
        return Ok(out.to_string_lossy().to_string());
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    win32::extract_to_png_with_lnk(lnk_path, target_path, &out)
        .map_err(|e| crate::app_index::AppIndexError::Other(format!("lnk icon extraction: {e}")))?;
    Ok(out.to_string_lossy().to_string())
}

/// UWP icon extraction для throttled queue.
/// Принимает уже открытый `Package` (избегает повторного PackageManager lookup).
/// Идемпотентно (skip if exists). При ошибке — placeholder, debug log.
#[cfg(target_os = "windows")]
pub fn ensure_icon_for_uwp(
    cache_dir: &Path,
    exec_path: &str,
    package: &windows::ApplicationModel::Package,
) -> Result<String> {
    let path = cached_icon_path(cache_dir, exec_path);
    if path.exists() {
        return Ok(path.to_string_lossy().to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match uwp::extract_logo_to_png(package, &path) {
        Ok(()) => {}
        Err(e) => {
            tracing::debug!(
                target: "app_index::icons",
                exec_path = %exec_path,
                error = %e,
                "uwp logo extraction failed, writing placeholder"
            );
            write_placeholder(&path)?;
        }
    }

    Ok(path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn ensure_icon_for_uwp_package(
    cache_dir: &Path,
    exec_path: &str,
    package_full_name: &str,
) -> Result<String> {
    use windows::core::HSTRING;
    use windows::Management::Deployment::PackageManager;

    let package_manager = PackageManager::new()
        .map_err(|e| crate::app_index::AppIndexError::Other(format!("PackageManager::new: {e}")))?;
    let empty = HSTRING::new();
    let full_name = HSTRING::from(package_full_name);
    let package = package_manager
        .FindPackageByUserSecurityIdPackageFullName(&empty, &full_name)
        .map_err(|e| {
            crate::app_index::AppIndexError::Other(format!(
                "FindPackageByUserSecurityIdPackageFullName: {e}"
            ))
        })?;

    ensure_icon_for_uwp(cache_dir, exec_path, &package)
}

/// 1×1 прозрачный PNG (67 байт). Placeholder если extractor не сработал.
#[cfg(target_os = "windows")]
const PLACEHOLDER_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

#[cfg(target_os = "windows")]
fn write_placeholder(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, PLACEHOLDER_PNG)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Win32 extraction
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod win32 {
    use std::path::Path;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::ExtractIconExW;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

    /// Извлечь иконку из exe/lnk и сохранить в `out_path` как PNG.
    pub(super) fn extract_to_png(exec_path: &str, out_path: &Path) -> std::io::Result<()> {
        // .lnk → resolve target, prefer icon_location если выставлен.
        let (icon_source, icon_index) = resolve_icon_source(exec_path);

        let hicon = unsafe { extract_hicon(&icon_source, icon_index) }.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "ExtractIconExW: no icon")
        })?;
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
        let hicon = unsafe { extract_hicon(source, icon_index) }.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "ExtractIconExW: no icon")
        })?;

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
        GetIconInfo(hicon, &mut info as *mut _).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("GetIconInfo: {e}"))
        })?;

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
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "GetObjectW(BITMAP) failed",
            ));
        }

        let width = bm.bmWidth;
        let height = bm.bmHeight;
        if width <= 0 || height <= 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "bitmap has zero dimensions",
            ));
        }

        // BITMAPINFOHEADER: top-down 32bpp BI_RGB.
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
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
        };

        let pixel_count = (width as usize) * (height as usize);
        let mut buf: Vec<u8> = vec![0u8; pixel_count * 4];

        // ScreenDC — компatible с экраном, для GetDIBits достаточно.
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "GetDC failed",
            ));
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "GetDIBits returned 0 lines",
            ));
        }

        // BGRA → RGBA swap. Если alpha-channel везде 0 (некоторые legacy icons),
        // GetDIBits не вытащит alpha — подкручиваем до 0xFF чтобы PNG не был
        // полностью прозрачным.
        let mut alpha_seen = false;
        for px in buf.chunks_exact_mut(4) {
            let b = px[0];
            let r = px[2];
            px[0] = r;
            px[2] = b;
            if px[3] != 0 {
                alpha_seen = true;
            }
        }
        if !alpha_seen {
            for px in buf.chunks_exact_mut(4) {
                px[3] = 0xFF;
            }
        }

        let image_buf =
            image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width as u32, height as u32, buf)
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::Other, "ImageBuffer::from_raw failed")
                })?;

        image_buf
            .save_with_format(out_path, image::ImageFormat::Png)
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("png save: {e}"))
            })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// UWP extraction
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod uwp {
    use std::path::Path;
    use windows::ApplicationModel::Package;
    use windows::Foundation::Size;
    use windows::Storage::Streams::{DataReader, InputStreamOptions};

    /// Извлечь logo из UWP Package, сохранить bytes на диск как есть.
    /// Logo от GetLogo обычно уже PNG (manifest icon scaled assets).
    pub(super) fn extract_logo_to_png(package: &Package, out_path: &Path) -> std::io::Result<()> {
        let map_err = |ctx: &'static str| {
            move |e: windows::core::Error| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("{ctx}: {e}"))
            }
        };

        // 1) AppListEntry → DisplayInfo → GetLogo(64×64) — даёт scaled asset.
        //    Fallback на package.Logo() если AppListEntries недоступен.
        let stream_ref = entry_logo(package).or_else(|| package_logo(package));
        let stream_ref = stream_ref.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "no logo source on package")
        })?;

        let open_op = stream_ref
            .OpenReadAsync()
            .map_err(map_err("OpenReadAsync"))?;
        let stream = open_op.get().map_err(map_err("OpenReadAsync.get"))?;

        let size = stream.Size().map_err(map_err("Size"))? as u32;
        if size == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "logo stream empty",
            ));
        }

        let reader = DataReader::CreateDataReader(&stream).map_err(map_err("CreateDataReader"))?;
        let _ = reader.SetInputStreamOptions(InputStreamOptions::None);

        let load_op = reader.LoadAsync(size).map_err(map_err("LoadAsync"))?;
        let loaded = load_op.get().map_err(map_err("LoadAsync.get"))?;
        if loaded == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "DataReader loaded 0 bytes",
            ));
        }

        let mut buf = vec![0u8; loaded as usize];
        reader.ReadBytes(&mut buf).map_err(map_err("ReadBytes"))?;

        // UWP logo приходит как 44×44 (или 64×64) PNG с transparent padding —
        // стандартный tile asset Microsoft (логотип в центре, ~12-14px padding
        // вокруг). При render в маленьком 20×20 box лого выглядит крошечным.
        // Обрезаем по bbox непрозрачных пикселей, потом записываем.
        match trim_transparent_png(&buf) {
            Ok(cropped) => std::fs::write(out_path, &cropped)?,
            Err(_) => std::fs::write(out_path, &buf)?, // fallback: пишем как есть
        }
        Ok(())
    }

    /// Обрезать прозрачные края PNG → новый PNG (тоже PNG, encoded заново).
    /// Если нет ни одного непрозрачного пикселя — возвращаем входной buf.
    fn trim_transparent_png(input: &[u8]) -> std::io::Result<Vec<u8>> {
        use image::ImageFormat;
        let img = image::load_from_memory(input).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("decode png: {e}"))
        })?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut min_x = w;
        let mut min_y = h;
        let mut max_x: u32 = 0;
        let mut max_y: u32 = 0;
        let mut any = false;
        for y in 0..h {
            for x in 0..w {
                if rgba.get_pixel(x, y)[3] > 8 {
                    any = true;
                    if x < min_x {
                        min_x = x;
                    }
                    if y < min_y {
                        min_y = y;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        }
        if !any {
            // Полностью transparent — нечего обрезать.
            return Ok(input.to_vec());
        }
        let cropped =
            image::imageops::crop_imm(&rgba, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
                .to_image();
        let mut out = Vec::new();
        let dyn_img = image::DynamicImage::ImageRgba8(cropped);
        dyn_img
            .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("encode png: {e}"))
            })?;
        Ok(out)
    }

    fn entry_logo(
        package: &Package,
    ) -> Option<windows::Storage::Streams::RandomAccessStreamReference> {
        let entries_op = package.GetAppListEntriesAsync().ok()?;
        let entries = entries_op.get().ok()?;
        for entry in entries {
            let info = match entry.DisplayInfo() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let size = Size {
                Width: 64.0,
                Height: 64.0,
            };
            if let Ok(logo) = info.GetLogo(size) {
                return Some(logo);
            }
        }
        None
    }

    fn package_logo(
        package: &Package,
    ) -> Option<windows::Storage::Streams::RandomAccessStreamReference> {
        let uri = package.Logo().ok()?;
        windows::Storage::Streams::RandomAccessStreamReference::CreateFromUri(&uri).ok()
    }
}
