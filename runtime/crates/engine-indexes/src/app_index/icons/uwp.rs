use std::path::Path;
use windows::ApplicationModel::Package;
use windows::Foundation::Size;
use windows::Storage::Streams::{DataReader, InputStreamOptions};

/// Извлечь logo из UWP Package, сохранить bytes на диск как есть.
/// Logo от GetLogo обычно уже PNG (manifest icon scaled assets).
pub(super) fn extract_logo_to_png(package: &Package, out_path: &Path) -> std::io::Result<()> {
    let map_err = |ctx: &'static str| {
        move |e: windows::core::Error| std::io::Error::other(format!("{ctx}: {e}"))
    };

    // 1) AppListEntry → DisplayInfo → GetLogo(64×64) — даёт scaled asset.
    //    Fallback на package.Logo() если AppListEntries недоступен.
    let stream_ref = entry_logo(package).or_else(|| package_logo(package));
    let stream_ref =
        stream_ref.ok_or_else(|| std::io::Error::other("no logo source on package"))?;

    let open_op = stream_ref
        .OpenReadAsync()
        .map_err(map_err("OpenReadAsync"))?;
    let stream = open_op.join().map_err(map_err("OpenReadAsync.join"))?;

    let size = stream.Size().map_err(map_err("Size"))? as u32;
    if size == 0 {
        return Err(std::io::Error::other("logo stream empty"));
    }

    let reader = DataReader::CreateDataReader(&stream).map_err(map_err("CreateDataReader"))?;
    let _ = reader.SetInputStreamOptions(InputStreamOptions::None);

    let load_op = reader.LoadAsync(size).map_err(map_err("LoadAsync"))?;
    let loaded = load_op.join().map_err(map_err("LoadAsync.join"))?;
    if loaded == 0 {
        return Err(std::io::Error::other("DataReader loaded 0 bytes"));
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
    let img = image::load_from_memory(input)
        .map_err(|e| std::io::Error::other(format!("decode png: {e}")))?;
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
        .map_err(|e| std::io::Error::other(format!("encode png: {e}")))?;
    Ok(out)
}

fn entry_logo(package: &Package) -> Option<windows::Storage::Streams::RandomAccessStreamReference> {
    let entries_op = package.GetAppListEntriesAsync().ok()?;
    let entries = entries_op.join().ok()?;
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
