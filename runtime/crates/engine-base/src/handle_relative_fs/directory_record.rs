use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedDirectoryRecord {
    pub(super) name: String,
    pub(super) directory: bool,
    pub(super) reparse_tag: u32,
    pub(super) file_id: [u8; 16],
}

pub(super) fn parse_directory_record(
    buffer: &[u8],
    used: usize,
    offset: usize,
) -> io::Result<(ParsedDirectoryRecord, Option<usize>)> {
    const HEADER: usize = 88;
    if offset > used || used - offset < HEADER {
        return Err(io::Error::other("short directory record"));
    }
    let p = &buffer[offset..used];
    let next = u32::from_ne_bytes(
        p[0..4]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory offset header"))?,
    ) as usize;
    if next != 0 && (next < HEADER || next % 8 != 0 || next > p.len()) {
        return Err(io::Error::other("invalid directory offset"));
    }
    let attrs = u32::from_ne_bytes(
        p[56..60]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory attributes"))?,
    );
    let name_len = u32::from_ne_bytes(
        p[60..64]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory name length"))?,
    ) as usize;
    let reparse_tag = u32::from_ne_bytes(
        p[68..72]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory reparse tag"))?,
    );
    let name_end = HEADER
        .checked_add(name_len)
        .ok_or_else(|| io::Error::other("filename overflow"))?;
    if !name_len.is_multiple_of(2) || name_end > p.len() {
        return Err(io::Error::other("invalid directory filename"));
    }
    let name_bytes = &p[HEADER..name_end];
    let name = String::from_utf16(
        name_bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_ne_bytes(*pair))
            .collect::<Vec<_>>()
            .as_slice(),
    )
    .map_err(|_| io::Error::other("invalid filename"))?;
    let file_id: [u8; 16] = p[72..88]
        .try_into()
        .map_err(|_| io::Error::other("invalid directory file id"))?;
    let next_offset = (next != 0).then_some(next);
    Ok((
        ParsedDirectoryRecord {
            name,
            directory: attrs & 0x10 != 0,
            reparse_tag,
            file_id,
        },
        next_offset,
    ))
}

#[cfg(test)]
mod directory_record_tests {
    use super::*;

    fn record(next: u32, attrs: u32, tag: u32, id: [u8; 16], name: &str) -> Vec<u8> {
        let words: Vec<u16> = name.encode_utf16().collect();
        let mut out = vec![0u8; 88 + words.len() * 2];
        out[0..4].copy_from_slice(&next.to_ne_bytes());
        out[56..60].copy_from_slice(&attrs.to_ne_bytes());
        out[60..64].copy_from_slice(&(words.len() as u32 * 2).to_ne_bytes());
        out[68..72].copy_from_slice(&tag.to_ne_bytes());
        out[72..88].copy_from_slice(&id);
        for (i, word) in words.iter().enumerate() {
            out[88 + i * 2..90 + i * 2].copy_from_slice(&word.to_ne_bytes());
        }
        out
    }

    #[test]
    fn parses_extended_record_and_128_bit_id() {
        let id = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let bytes = record(0, 0x10, 0, id, "файл");
        let (parsed, next) = parse_directory_record(&bytes, bytes.len(), 0).unwrap();
        assert_eq!(parsed.name, "файл");
        assert!(parsed.directory);
        assert_eq!(parsed.file_id, id);
        assert_eq!(next, None);
    }

    #[test]
    fn rejects_malformed_offsets_and_name_lengths() {
        let id = [7u8; 16];
        let mut bytes = record(0, 0, 0, id, "x");
        bytes[0..4].copy_from_slice(&87u32.to_ne_bytes());
        assert!(parse_directory_record(&bytes, bytes.len(), 0).is_err());
        let mut bytes = record(0, 0, 0, id, "x");
        bytes[60..64].copy_from_slice(&3u32.to_ne_bytes());
        assert!(parse_directory_record(&bytes, bytes.len(), 0).is_err());
        let mut bytes = record(0, 0, 0, id, "x");
        bytes[60..64].copy_from_slice(&400u32.to_ne_bytes());
        assert!(parse_directory_record(&bytes, bytes.len(), 0).is_err());
    }
}
