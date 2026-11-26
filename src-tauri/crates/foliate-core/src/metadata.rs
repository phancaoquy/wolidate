use crate::{CoreError, Format, Result};
use base64::Engine as _;
use percent_encoding::percent_decode_str;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Metadata extracted from a book container.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedMetadata {
    pub title: String,
    pub author: String,
    pub language: String,
    /// Original cover image bytes. The caller renders the 300 px WebP.
    pub cover: Option<Vec<u8>>,
}

const MAX_XML_BYTES: u64 = 8 * 1024 * 1024;
const MAX_COVER_BYTES: u64 = 24 * 1024 * 1024;

/// Extract metadata for a supported format. Only EPUB, FB2, and CBZ produce
/// real metadata in v0.1; MOBI/AZW3/PDF fall back to the file name.
pub fn extract(
    source: &mut (impl Read + Seek),
    format: Format,
    fallback_title: &str,
) -> Result<ParsedMetadata> {
    source.seek(SeekFrom::Start(0))?;
    match format {
        Format::Epub => extract_epub(source, fallback_title),
        Format::Fb2 => extract_fb2(source, fallback_title),
        Format::Cbz => extract_cbz(source, fallback_title),
        Format::Mobi | Format::Azw3 | Format::Pdf => Ok(ParsedMetadata {
            title: fallback_title.to_owned(),
            ..Default::default()
        }),
    }
}

fn extract_epub(source: &mut (impl Read + Seek), fallback_title: &str) -> Result<ParsedMetadata> {
    let mut archive = zip::ZipArchive::new(&mut *source)?;
    let container = read_zip_text(&mut archive, "META-INF/container.xml", MAX_XML_BYTES)?;
    let container_xml = roxmltree::Document::parse(&container)?;
    let rootfile = container_xml
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "rootfile")
        .and_then(|node| node.attribute("full-path"))
        .ok_or_else(|| CoreError::Invalid("EPUB has no rootfile".into()))?;
    let rootfile = percent_decode_str(rootfile).decode_utf8_lossy().to_string();

    let opf = read_zip_text(&mut archive, &rootfile, MAX_XML_BYTES)?;
    let opf_xml = roxmltree::Document::parse(&opf)?;
    let opf_dir = parent_dir(&rootfile);

    let mut title = None;
    let mut creators = Vec::new();
    let mut language = None;
    for node in opf_xml.descendants().filter(|node| node.is_element()) {
        match node.tag_name().name() {
            "title" if title.is_none() => title = non_empty_text(node),
            "creator" => {
                if let Some(value) = non_empty_text(node) {
                    creators.push(value);
                }
            }
            "language" if language.is_none() => language = non_empty_text(node),
            _ => {}
        }
    }
    let mut metadata = ParsedMetadata {
        title: title
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback_title.to_owned()),
        author: creators.join(", "),
        language: language.unwrap_or_default(),
        cover: None,
    };

    let cover_id = opf_xml
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "meta"
                && node.attribute("name") == Some("cover")
        })
        .and_then(|node| node.attribute("content"));
    let cover_properties = opf_xml.descendants().find(|node| {
        node.is_element()
            && node.tag_name().name() == "item"
            && node
                .attribute("properties")
                .is_some_and(|p| p.split_whitespace().any(|value| value == "cover-image"))
    });
    let cover_href = cover_id
        .and_then(|id| {
            opf_xml.descendants().find(|node| {
                node.is_element()
                    && node.tag_name().name() == "item"
                    && node.attribute("id") == Some(id)
            })
        })
        .and_then(|node| node.attribute("href"))
        .or_else(|| cover_properties.and_then(|node| node.attribute("href")))
        .or_else(|| {
            opf_xml
                .descendants()
                .find(|node| {
                    node.is_element()
                        && node.tag_name().name() == "item"
                        && node.attribute("href").is_some_and(|href| {
                            let href = href.to_ascii_lowercase();
                            href.contains("cover")
                                && (href.ends_with(".jpg")
                                    || href.ends_with(".jpeg")
                                    || href.ends_with(".png")
                                    || href.ends_with(".webp"))
                        })
                })
                .and_then(|node| node.attribute("href"))
        });

    if let Some(href) = cover_href {
        let path = resolve_path(&opf_dir, href);
        if let Ok(bytes) = read_zip_entry(&mut archive, &path, MAX_COVER_BYTES) {
            metadata.cover = Some(bytes);
        }
    }
    Ok(metadata)
}

fn extract_cbz(source: &mut (impl Read + Seek), fallback_title: &str) -> Result<ParsedMetadata> {
    let mut archive = zip::ZipArchive::new(&mut *source)?;
    let mut cover = None;
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_owned();
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".webp")
            || lower.ends_with(".gif")
            || lower.ends_with(".avif")
        {
            drop(file);
            cover = read_zip_entry(&mut archive, &name, MAX_COVER_BYTES).ok();
            break;
        }
    }
    Ok(ParsedMetadata {
        title: fallback_title.to_owned(),
        cover,
        ..Default::default()
    })
}

fn extract_fb2(source: &mut (impl Read + Seek), fallback_title: &str) -> Result<ParsedMetadata> {
    let mut bytes = Vec::new();
    source
        .take(MAX_XML_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_XML_BYTES {
        return Err(CoreError::Invalid("FB2 metadata is too large".into()));
    }
    let text = String::from_utf8_lossy(&bytes);
    let doc = roxmltree::Document::parse(&text)?;

    let title = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "book-title")
        .and_then(non_empty_text);
    let language = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "lang")
        .and_then(non_empty_text);
    let author = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "author")
        .map(|author| {
            let first = author
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "first-name")
                .and_then(non_empty_text)
                .unwrap_or_default();
            let last = author
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "last-name")
                .and_then(non_empty_text)
                .unwrap_or_default();
            format!("{first} {last}").trim().to_owned()
        })
        .filter(|value| !value.is_empty());

    let cover = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "coverpage")
        .and_then(|node| {
            node.descendants().find(|n| {
                n.is_element() && n.tag_name().name() == "image" && n.attribute("href").is_some()
            })
        })
        .and_then(|image| image.attribute("href"))
        .and_then(|href| href.strip_prefix('#'))
        .and_then(|id| {
            doc.descendants()
                .find(|n| {
                    n.is_element()
                        && n.tag_name().name() == "binary"
                        && n.attribute("id") == Some(id)
                })
                .and_then(|binary| binary.text())
        })
        .and_then(|encoded| {
            let cleaned: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
            base64::engine::general_purpose::STANDARD
                .decode(cleaned)
                .ok()
        })
        .filter(|bytes| bytes.len() as u64 <= MAX_COVER_BYTES);

    Ok(ParsedMetadata {
        title: title.unwrap_or_else(|| fallback_title.to_owned()),
        author: author.unwrap_or_default(),
        language: language.unwrap_or_default(),
        cover,
    })
}

/// Render a cover to WebP with its longest edge at `max_dim` pixels.
pub fn write_cover_webp(image_bytes: &[u8], out: &Path, max_dim: u32) -> Result<()> {
    let image = image::load_from_memory(image_bytes)?;
    let thumbnail = image.thumbnail(max_dim, max_dim);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut writer = std::io::BufWriter::new(std::fs::File::create(out)?);
    thumbnail.write_to(&mut writer, image::ImageFormat::WebP)?;
    writer.flush()?;
    Ok(())
}

fn read_zip_text<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    cap: u64,
) -> Result<String> {
    let bytes = read_zip_entry(archive, name, cap)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_zip_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    cap: u64,
) -> Result<Vec<u8>> {
    let file = archive
        .by_name(name)
        .map_err(|_| CoreError::NotFound(format!("ZIP entry {name}")))?;
    if file.size() > cap {
        return Err(CoreError::Invalid(format!(
            "ZIP entry {name} is {} bytes, cap is {cap}",
            file.size()
        )));
    }
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > cap {
        return Err(CoreError::Invalid(format!(
            "ZIP entry {name} exceeded the {cap} byte cap"
        )));
    }
    Ok(bytes)
}

fn non_empty_text(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let text = node
        .text()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

fn parent_dir(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(dir, _)| dir.to_owned())
        .unwrap_or_default()
}

/// Resolve an EPUB href relative to the OPF directory.
pub fn resolve_path(base_dir: &str, href: &str) -> String {
    let href = href.split(['#', '?']).next().unwrap_or(href);
    let href = percent_decode_str(href).decode_utf8_lossy();
    let mut parts: Vec<&str> = base_dir
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in href.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    fn png_bytes() -> Vec<u8> {
        let image: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_pixel(12, 18, Rgba([200, 30, 40, 255]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn epub_bytes() -> Vec<u8> {
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
            writer
                .write_all(
                    br#"<?xml version="1.0"?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#,
                )
                .unwrap();
            writer
                .start_file("OEBPS/content.opf", SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(
                    br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>  Structural   Reasoning </dc:title>
    <dc:creator>Ada Lovelace</dc:creator>
    <dc:creator>Grace Hopper</dc:creator>
    <dc:language>en</dc:language>
    <meta name="cover" content="cover-image"/>
  </metadata>
  <manifest>
    <item id="cover-image" href="images/cover.png" media-type="image/png"/>
    <item id="chapter" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
</package>"#,
                )
                .unwrap();
            writer
                .start_file("OEBPS/images/cover.png", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&png_bytes()).unwrap();
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn extracts_epub_metadata_and_cover() {
        let bytes = epub_bytes();
        let mut source = Cursor::new(bytes);
        let metadata = extract(&mut source, Format::Epub, "fallback").unwrap();
        assert_eq!(metadata.title, "Structural Reasoning");
        assert_eq!(metadata.author, "Ada Lovelace, Grace Hopper");
        assert_eq!(metadata.language, "en");
        assert!(metadata.cover.is_some());
    }

    #[test]
    fn extracts_cbz_cover_from_first_image() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer
                .start_file("page-001.png", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&png_bytes()).unwrap();
            writer
                .start_file("page-002.png", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&png_bytes()).unwrap();
            writer.finish().unwrap();
        }
        let mut source = Cursor::new(cursor.into_inner());
        let metadata = extract(&mut source, Format::Cbz, "Comic").unwrap();
        assert_eq!(metadata.title, "Comic");
        assert!(metadata.cover.is_some());
    }

    #[test]
    fn extracts_fb2_metadata() {
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
  <description><title-info>
    <book-title>Roadside Picnic</book-title>
    <author><first-name>Arkady</first-name><last-name>Strugatsky</last-name></author>
    <lang>ru</lang>
  </title-info></description>
</FictionBook>"#;
        let mut source = Cursor::new(xml.to_vec());
        let metadata = extract(&mut source, Format::Fb2, "fallback").unwrap();
        assert_eq!(metadata.title, "Roadside Picnic");
        assert_eq!(metadata.author, "Arkady Strugatsky");
        assert_eq!(metadata.language, "ru");
    }

    #[test]
    fn writes_a_webp_cover() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("cover.webp");
        write_cover_webp(&png_bytes(), &out, 300).unwrap();
        let decoded = image::open(&out).unwrap();
        assert!(decoded.width() <= 300);
        assert!(decoded.height() <= 300);
        assert!(decoded.height() > 1);
    }

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(
            resolve_path("OEBPS", "images/cover.png"),
            "OEBPS/images/cover.png"
        );
        assert_eq!(
            resolve_path("OEBPS/text", "../images/a%20b.png"),
            "OEBPS/images/a b.png"
        );
        assert_eq!(resolve_path("", "./a/b.html#frag"), "a/b.html");
    }
}
