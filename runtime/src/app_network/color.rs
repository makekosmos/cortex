//! `shared/electron/image-dominant-color.ts` + `image-dimensions.ts` ports —
//! 3-bit bucketed dominant color and PNG/JPEG dimension sniffing.

/// `dominantImageColor` — RGB(A) pixel buffer → CSS `rgb(R G B)`.
/// The largest chromatic bucket wins; grayscale falls back to the largest
/// bucket; alpha < 128 pixels are skipped.
pub(crate) fn dominant_image_color(pixels: &[u8], channels: usize) -> Option<String> {
    struct Bucket {
        count: usize,
        red: u64,
        green: u64,
        blue: u64,
    }
    let mut colors: std::collections::HashMap<u16, Bucket> = std::collections::HashMap::new();
    for px in pixels.chunks_exact(channels) {
        if channels >= 4 && px[3] < 128 {
            continue;
        }
        let (red, green, blue) = (px[0], px[1], px[2]);
        let key = (((red >> 5) as u16) << 6) | (((green >> 5) as u16) << 3) | (blue >> 5) as u16;
        let bucket = colors.entry(key).or_insert(Bucket {
            count: 0,
            red: 0,
            green: 0,
            blue: 0,
        });
        bucket.count += 1;
        bucket.red += red as u64;
        bucket.green += green as u64;
        bucket.blue += blue as u64;
    }
    let mut dominant: Option<&Bucket> = None;
    let mut fallback: Option<&Bucket> = None;
    for bucket in colors.values() {
        if fallback.is_none_or(|f| bucket.count > f.count) {
            fallback = Some(bucket);
        }
        let (r, g, b) = (
            bucket.red / bucket.count as u64,
            bucket.green / bucket.count as u64,
            bucket.blue / bucket.count as u64,
        );
        if r.max(g).max(b) - r.min(g).min(b) < 24 {
            continue;
        }
        if dominant.is_none_or(|d| bucket.count > d.count) {
            dominant = Some(bucket);
        }
    }
    let bucket = dominant.or(fallback)?;
    let count = bucket.count as u64;
    Some(format!(
        "rgb({} {} {})",
        (bucket.red + count / 2) / count,
        (bucket.green + count / 2) / count,
        (bucket.blue + count / 2) / count
    ))
}

fn uint16(bytes: &[u8], offset: usize) -> u32 {
    bytes[offset] as u32 * 256 + bytes[offset + 1] as u32
}
fn uint32(bytes: &[u8], offset: usize) -> u32 {
    bytes[offset] as u32 * 0x1000000
        + bytes[offset + 1] as u32 * 0x10000
        + bytes[offset + 2] as u32 * 0x100
        + bytes[offset + 3] as u32
}

/// `imageDimensions` — PNG IHDR or JPEG SOF marker scan.
pub(crate) fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() >= 24
        && bytes[..8] == *PNG_MAGIC
        && uint32(bytes, 8) == 13
        && &bytes[12..16] == b"IHDR"
    {
        let (width, height) = (uint32(bytes, 16), uint32(bytes, 20));
        return (width > 0 && height > 0).then_some((width, height));
    }
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut offset = 2usize;
    while offset < bytes.len() {
        if bytes[offset] != 0xff {
            return None;
        }
        offset += 1;
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        if offset >= bytes.len() {
            return None;
        }
        let marker = bytes[offset];
        offset += 1;
        if marker == 0xd9 || marker == 0xda || marker == 0x00 {
            return None;
        }
        if marker == 0x01 || marker == 0xd8 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if offset + 2 > bytes.len() {
            return None;
        }
        let length = uint16(bytes, offset) as usize;
        let end = offset + length;
        if length < 2 || end > bytes.len() {
            return None;
        }
        if (0xc0..=0xcf).contains(&marker) && marker != 0xc4 && marker != 0xc8 && marker != 0xcc {
            if length < 7 {
                return None;
            }
            let (height, width) = (uint16(bytes, offset + 3), uint16(bytes, offset + 5));
            return (width > 0 && height > 0).then_some((width, height));
        }
        offset = end;
    }
    None
}
