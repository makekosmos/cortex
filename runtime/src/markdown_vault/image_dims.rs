//! Image dimension probing — port of `readImageDimensions` from
//! `shared/electron/markdown-vault.ts` (PNG/GIF/JPEG/WEBP headers only).

pub fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() >= 24 && &bytes[0..4] == b"\x89PNG" {
        return Some((be32(bytes, 16)?, be32(bytes, 20)?));
    }
    if bytes.len() >= 10 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        return Some((le16(bytes, 6)?, le16(bytes, 8)?));
    }
    if bytes.len() >= 12 && bytes[0] == 0xff && bytes[1] == 0xd8 {
        let mut offset = 2usize;
        while offset + 9 < bytes.len() {
            if bytes[offset] != 0xff {
                offset += 1;
                continue;
            }
            let marker = bytes[offset + 1];
            let size = be16(bytes, offset + 2)? as usize;
            if size < 2 {
                return None;
            }
            let is_sof = (0xc0..=0xc3).contains(&marker)
                || (0xc5..=0xc7).contains(&marker)
                || (0xc9..=0xcb).contains(&marker)
                || (0xcd..=0xcf).contains(&marker);
            if is_sof {
                let height = be16(bytes, offset + 5)? as u32;
                let width = be16(bytes, offset + 7)? as u32;
                return Some((width, height));
            }
            offset += 2 + size;
        }
    }
    if bytes.len() >= 30 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        match &bytes[12..16] {
            b"VP8X" => {
                return Some((le24(bytes, 24)? + 1, le24(bytes, 27)? + 1));
            }
            b"VP8 " => {
                return Some((le16(bytes, 26)? & 0x3fff, le16(bytes, 28)? & 0x3fff));
            }
            b"VP8L" if bytes.len() >= 25 => {
                let bits = le32(bytes, 21)?;
                return Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1));
            }
            _ => {}
        }
    }
    None
}

fn be16(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}
fn le16(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]) as u32)
}
fn be32(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}
fn le32(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn le24(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 3)
        .map(|s| s[0] as u32 + ((s[1] as u32) << 8) + ((s[2] as u32) << 16))
}
