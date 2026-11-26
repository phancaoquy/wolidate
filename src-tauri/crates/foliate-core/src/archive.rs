use crate::{CoreError, Result};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};

/// Metadata for one ZIP entry. The index holds only central-directory data;
/// compressed bytes stay on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMeta {
    pub name: String,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub header_offset: u64,
    pub data_offset: u64,
    pub is_dir: bool,
}

/// A ZIP central-directory index keyed by entry name.
#[derive(Debug, Clone, Default)]
pub struct ZipIndex {
    entries: BTreeMap<String, EntryMeta>,
}

impl ZipIndex {
    pub fn entries(&self) -> impl Iterator<Item = &EntryMeta> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&EntryMeta> {
        self.entries.get(name)
    }
}

/// Read the ZIP central directory through a seekable source.
///
/// This seeks to the end of the archive and reads only the central directory,
/// not the entry payloads.
pub fn index_zip(source: &mut (impl Read + Seek)) -> Result<ZipIndex> {
    let mut archive = zip::ZipArchive::new(&mut *source)?;
    let mut entries = BTreeMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let header_offset = file.header_start();
        entries.insert(
            file.name().to_owned(),
            EntryMeta {
                name: file.name().to_owned(),
                compressed_size: file.compressed_size(),
                uncompressed_size: file.size(),
                header_offset,
                data_offset: 0,
                is_dir: file.is_dir(),
            },
        );
    }
    drop(archive);

    for entry in entries.values_mut() {
        entry.data_offset = data_offset(source, entry.header_offset)?;
    }

    Ok(ZipIndex { entries })
}

fn data_offset(source: &mut (impl Read + Seek), header_offset: u64) -> Result<u64> {
    source.seek(SeekFrom::Start(header_offset))?;
    let mut header = [0_u8; 30];
    source.read_exact(&mut header)?;
    if u32::from_le_bytes(header[0..4].try_into().expect("slice length")) != 0x0403_4b50 {
        return Err(CoreError::Invalid(format!(
            "bad local ZIP header at {header_offset}"
        )));
    }
    let name_len = u16::from_le_bytes(header[26..28].try_into().expect("slice length")) as u64;
    let extra_len = u16::from_le_bytes(header[28..30].try_into().expect("slice length")) as u64;
    Ok(header_offset + 30 + name_len + extra_len)
}

/// Read and decompress one entry, enforcing an output cap so a malformed or
/// hostile archive cannot exhaust memory.
pub fn read_entry_bytes(
    source: &mut (impl Read + Seek),
    name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(&mut *source)?;
    let file = archive
        .by_name(name)
        .map_err(|_| CoreError::NotFound(format!("ZIP entry {name}")))?;
    if file.is_dir() {
        return Err(CoreError::Invalid(format!("{name} is a directory")));
    }
    if file.size() > max_bytes {
        return Err(CoreError::Invalid(format!(
            "ZIP entry {name} is {} bytes, cap is {max_bytes}",
            file.size()
        )));
    }
    let mut bytes = Vec::with_capacity(file.size().min(max_bytes) as usize);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(CoreError::Invalid(format!(
            "ZIP entry {name} exceeded the {max_bytes} byte cap"
        )));
    }
    Ok(bytes)
}

/// Stream one decompressed entry to a writer without buffering the whole entry.
pub fn stream_entry(
    source: &mut (impl Read + Seek),
    name: &str,
    writer: &mut impl Write,
    max_bytes: u64,
) -> Result<u64> {
    let mut archive = zip::ZipArchive::new(&mut *source)?;
    let file = archive
        .by_name(name)
        .map_err(|_| CoreError::NotFound(format!("ZIP entry {name}")))?;
    if file.is_dir() {
        return Err(CoreError::Invalid(format!("{name} is a directory")));
    }
    let mut limited = file.take(max_bytes.saturating_add(1));
    let copied = std::io::copy(&mut limited, writer)?;
    if copied > max_bytes {
        return Err(CoreError::Invalid(format!(
            "ZIP entry {name} exceeded the {max_bytes} byte cap"
        )));
    }
    Ok(copied)
}

/// Inclusive byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

impl ByteRange {
    pub fn len(self) -> u64 {
        self.end.saturating_sub(self.start).saturating_add(1)
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// Parse a single-part `Range` header against a known file size.
///
/// Returns `Ok(None)` when the header is absent or not a byte-range header.
pub fn parse_range_header(value: &str, size: u64) -> Result<Option<ByteRange>> {
    let Some(spec) = value.trim().strip_prefix("bytes=") else {
        return Ok(None);
    };
    if size == 0 {
        return Err(CoreError::Invalid("range on an empty file".into()));
    }
    if spec.contains(',') {
        return Err(CoreError::Invalid(
            "multiple ranges are not supported".into(),
        ));
    }
    let (start_raw, end_raw) = spec
        .split_once('-')
        .ok_or_else(|| CoreError::Invalid(format!("bad range header: {value}")))?;

    if start_raw.is_empty() {
        let suffix = end_raw
            .parse::<u64>()
            .map_err(|_| CoreError::Invalid(format!("bad range suffix: {value}")))?;
        if suffix == 0 {
            return Err(CoreError::Invalid("zero-length range suffix".into()));
        }
        return Ok(Some(ByteRange {
            start: size.saturating_sub(suffix),
            end: size - 1,
        }));
    }

    let start = start_raw
        .parse::<u64>()
        .map_err(|_| CoreError::Invalid(format!("bad range start: {value}")))?;
    if start >= size {
        return Err(CoreError::Invalid(format!(
            "range starts at {start}, file is {size} bytes"
        )));
    }
    let end = if end_raw.is_empty() {
        size - 1
    } else {
        end_raw
            .parse::<u64>()
            .map_err(|_| CoreError::Invalid(format!("bad range end: {value}")))?
            .min(size - 1)
    };
    if end < start {
        return Err(CoreError::Invalid(format!("inverted range: {value}")));
    }
    Ok(Some(ByteRange { start, end }))
}

/// Read an inclusive range into memory. Callers must bound `range.len()`
/// before calling this; the protocol layer uses an 8 MiB cap.
pub fn read_range(source: &mut (impl Read + Seek), range: ByteRange) -> Result<Vec<u8>> {
    let len = usize::try_from(range.len())
        .map_err(|_| CoreError::Invalid("range is too large for this platform".into()))?;
    source.seek(SeekFrom::Start(range.start))?;
    let mut bytes = vec![0_u8; len];
    source.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Basic content type for the custom protocol. EPUB resources are usually
/// XHTML/HTML, but images and fonts must keep their real types.
pub fn content_type(name: &str) -> &'static str {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "xhtml" | "xht" => "application/xhtml+xml",
        "html" | "htm" => "text/html",
        "xml" | "opf" | "ncx" => "application/xml",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;

    fn sample_zip() -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer
                .start_file(
                    "mimetype",
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(b"application/epub+zip").unwrap();

            writer
                .start_file("META-INF/container.xml", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"<container/>").unwrap();

            writer
                .start_file("OEBPS/chapter1.xhtml", SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(b"<html><body>Hello archive</body></html>")
                .unwrap();
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn indexes_central_directory_without_reading_entries() {
        let bytes = sample_zip();
        let mut source = Cursor::new(bytes);
        let index = index_zip(&mut source).unwrap();
        assert_eq!(index.len(), 3);
        let chapter = index.get("OEBPS/chapter1.xhtml").unwrap();
        assert_eq!(chapter.uncompressed_size, 39);
        assert!(!chapter.is_dir);
        assert!(chapter.data_offset > chapter.header_offset);
    }

    #[test]
    fn reads_an_entry_by_name() {
        let bytes = sample_zip();
        let mut source = Cursor::new(bytes);
        let text = read_entry_bytes(&mut source, "OEBPS/chapter1.xhtml", 1024).unwrap();
        assert_eq!(text, b"<html><body>Hello archive</body></html>");
    }

    #[test]
    fn rejects_entry_over_output_cap() {
        let bytes = sample_zip();
        let mut source = Cursor::new(bytes);
        let err = read_entry_bytes(&mut source, "OEBPS/chapter1.xhtml", 8).unwrap_err();
        assert!(matches!(err, CoreError::Invalid(_)));
    }

    #[test]
    fn streams_an_entry() {
        let bytes = sample_zip();
        let mut source = Cursor::new(bytes);
        let mut out = Vec::new();
        let copied = stream_entry(&mut source, "OEBPS/chapter1.xhtml", &mut out, 1024).unwrap();
        assert_eq!(copied, 39);
        assert_eq!(out, b"<html><body>Hello archive</body></html>");
    }

    #[test]
    fn parses_range_headers() {
        assert_eq!(
            parse_range_header("bytes=0-99", 1000).unwrap(),
            Some(ByteRange { start: 0, end: 99 })
        );
        assert_eq!(
            parse_range_header("bytes=900-", 1000).unwrap(),
            Some(ByteRange {
                start: 900,
                end: 999
            })
        );
        assert_eq!(
            parse_range_header("bytes=-50", 1000).unwrap(),
            Some(ByteRange {
                start: 950,
                end: 999
            })
        );
        assert_eq!(
            parse_range_header("bytes=900-9999", 1000).unwrap(),
            Some(ByteRange {
                start: 900,
                end: 999
            })
        );
        assert_eq!(parse_range_header("items=0-1", 1000).unwrap(), None);
    }

    #[test]
    fn rejects_invalid_ranges() {
        assert!(parse_range_header("bytes=1000-", 1000).is_err());
        assert!(parse_range_header("bytes=5-4", 1000).is_err());
        assert!(parse_range_header("bytes=0-1,4-5", 1000).is_err());
        assert!(parse_range_header("bytes=0-", 0).is_err());
    }

    #[test]
    fn reads_a_range() {
        let bytes = sample_zip();
        let mut source = Cursor::new(bytes);
        let slice = read_range(&mut source, ByteRange { start: 2, end: 5 }).unwrap();
        assert_eq!(slice.len(), 4);
    }

    #[test]
    fn maps_content_types() {
        assert_eq!(content_type("a/chapter.xhtml"), "application/xhtml+xml");
        assert_eq!(content_type("a/cover.WEBP"), "image/webp");
        assert_eq!(content_type("a/unknown.bin"), "application/octet-stream");
    }
}
