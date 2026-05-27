use anyhow::{anyhow, Context as _, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use roxmltree::Document;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub title: String,
    pub chapters: Vec<Chapter>,
}

impl Book {
    #[cfg(test)]
    pub fn block_count(&self) -> usize {
        self.chapters
            .iter()
            .map(|chapter| chapter.blocks.len())
            .sum()
    }

    pub fn chapter_start_block(&self, chapter_index: usize) -> usize {
        self.chapters
            .iter()
            .take(chapter_index)
            .map(|chapter| chapter.blocks.len())
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub title: String,
    pub blocks: Vec<ReaderBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderBlock {
    pub kind: BlockKind,
    pub spans: Vec<InlineSpan>,
}

impl ReaderBlock {
    pub fn plain_text(&self) -> String {
        self.spans
            .iter()
            .map(|span| span.text.as_str())
            .collect::<String>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Heading(u8),
    Paragraph,
    ListItem,
    Blockquote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSpan {
    pub text: String,
    pub strong: bool,
    pub emphasis: bool,
}

pub fn read_epub(path: &Path) -> Result<Book> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("failed to read EPUB zip")?;
    let container = read_zip_text(&mut zip, "META-INF/container.xml")?;
    let rootfile = parse_rootfile_path(&container)?;
    let opf = read_zip_text(&mut zip, &rootfile)?;
    let package_dir = parent_dir(&rootfile);
    let package = parse_package(&opf)?;

    let mut chapters = Vec::new();
    for item_id in package.spine {
        let Some(href) = package.manifest.get(&item_id) else {
            continue;
        };
        let item_path = join_zip_path(&package_dir, href);
        let xhtml = read_zip_text(&mut zip, &item_path)
            .with_context(|| format!("failed to read spine item {item_path}"))?;
        let mut chapter = parse_xhtml_chapter(&xhtml);
        if chapter.title.is_empty() {
            chapter.title = href.rsplit('/').next().unwrap_or(href).to_string();
        }
        if !chapter.blocks.is_empty() {
            chapters.push(chapter);
        }
    }

    if chapters.is_empty() {
        return Err(anyhow!("EPUB has no readable spine content"));
    }

    Ok(Book {
        title: package.title.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled EPUB")
                .to_string()
        }),
        chapters,
    })
}

fn read_zip_text<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>, name: &str) -> Result<String> {
    let mut file = zip
        .by_name(name)
        .with_context(|| format!("missing EPUB entry {name}"))?;
    let mut out = String::new();
    file.read_to_string(&mut out)
        .with_context(|| format!("entry {name} is not UTF-8 text"))?;
    Ok(out)
}

fn parse_rootfile_path(container: &str) -> Result<String> {
    let doc = Document::parse(container).context("container.xml is invalid XML")?;
    let path = doc
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| anyhow!("container.xml has no rootfile full-path"))?;
    Ok(path.to_string())
}

struct Package {
    title: Option<String>,
    manifest: HashMap<String, String>,
    spine: Vec<String>,
}

fn parse_package(opf: &str) -> Result<Package> {
    let doc = Document::parse(opf).context("OPF package is invalid XML")?;
    let title = doc
        .descendants()
        .find(|n| n.tag_name().name() == "title")
        .and_then(|n| n.text())
        .map(normalize_ws)
        .filter(|s| !s.is_empty());

    let mut manifest = HashMap::new();
    for item in doc.descendants().filter(|n| n.has_tag_name("item")) {
        let Some(id) = item.attribute("id") else {
            continue;
        };
        let Some(href) = item.attribute("href") else {
            continue;
        };
        manifest.insert(id.to_string(), href.to_string());
    }

    let spine = doc
        .descendants()
        .filter(|n| n.has_tag_name("itemref"))
        .filter_map(|n| n.attribute("idref").map(ToOwned::to_owned))
        .collect();

    Ok(Package {
        title,
        manifest,
        spine,
    })
}

fn parse_xhtml_chapter(xhtml: &str) -> Chapter {
    let mut reader = Reader::from_str(xhtml);
    reader.config_mut().trim_text(true);

    let mut blocks = Vec::new();
    let mut current: Option<BlockBuilder> = None;
    let mut strong_depth = 0usize;
    let mut emphasis_depth = 0usize;
    let mut bridgehead_depth = 0usize;
    let mut span_bridgehead_stack = Vec::new();
    let mut title = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if let Some(kind) = block_kind(&tag) {
                    if let Some(block) = current.take().and_then(BlockBuilder::finish) {
                        blocks.push(block);
                    }
                    current = Some(BlockBuilder::new(tag, kind));
                } else if tag == "span" {
                    let is_bridgehead = is_bridgehead_span(&e);
                    span_bridgehead_stack.push(is_bridgehead);
                    if is_bridgehead {
                        bridgehead_depth += 1;
                    }
                } else if is_strong_tag(&tag) {
                    strong_depth += 1;
                } else if is_emphasis_tag(&tag) {
                    emphasis_depth += 1;
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(block) = current.as_mut() {
                    let text = String::from_utf8_lossy(e.as_ref());
                    block.push_text(
                        &text,
                        strong_depth > 0 || bridgehead_depth > 0,
                        emphasis_depth > 0,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if is_strong_tag(&tag) {
                    strong_depth = strong_depth.saturating_sub(1);
                } else if tag == "span" {
                    if span_bridgehead_stack.pop().unwrap_or(false) {
                        bridgehead_depth = bridgehead_depth.saturating_sub(1);
                    }
                } else if is_emphasis_tag(&tag) {
                    emphasis_depth = emphasis_depth.saturating_sub(1);
                } else if current.as_ref().is_some_and(|block| block.tag == tag) {
                    if let Some(block) = current.take().and_then(BlockBuilder::finish) {
                        if title.is_empty() && matches!(block.kind, BlockKind::Heading(_)) {
                            title = block.plain_text();
                        }
                        blocks.push(block);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    if let Some(block) = current.and_then(BlockBuilder::finish) {
        if title.is_empty() && matches!(block.kind, BlockKind::Heading(_)) {
            title = block.plain_text();
        }
        blocks.push(block);
    }

    Chapter { title, blocks }
}

struct BlockBuilder {
    tag: String,
    kind: BlockKind,
    spans: Vec<InlineSpan>,
}

impl BlockBuilder {
    fn new(tag: String, kind: BlockKind) -> Self {
        Self {
            tag,
            kind,
            spans: Vec::new(),
        }
    }

    fn push_text(&mut self, raw: &str, strong: bool, emphasis: bool) {
        let mut text = normalize_ws(raw);
        if text.is_empty() {
            return;
        }

        if let Some(previous) = self.spans.last_mut() {
            let needs_space = should_insert_space(&previous.text, &text);
            if needs_space && (previous.strong != strong || previous.emphasis != emphasis) {
                previous.text.push(' ');
            } else if needs_space {
                text.insert(0, ' ');
            }
            if previous.strong == strong && previous.emphasis == emphasis {
                previous.text.push_str(&text);
                return;
            }
        }

        self.spans.push(InlineSpan {
            text,
            strong,
            emphasis,
        });
    }

    fn finish(mut self) -> Option<ReaderBlock> {
        trim_spans(&mut self.spans);
        if self.spans.is_empty() {
            return None;
        }
        Some(ReaderBlock {
            kind: self.kind,
            spans: self.spans,
        })
    }
}

fn block_kind(tag: &str) -> Option<BlockKind> {
    match tag {
        "h1" => Some(BlockKind::Heading(1)),
        "h2" => Some(BlockKind::Heading(2)),
        "h3" => Some(BlockKind::Heading(3)),
        "h4" => Some(BlockKind::Heading(4)),
        "h5" => Some(BlockKind::Heading(5)),
        "h6" => Some(BlockKind::Heading(6)),
        "p" => Some(BlockKind::Paragraph),
        "li" => Some(BlockKind::ListItem),
        "blockquote" => Some(BlockKind::Blockquote),
        _ => None,
    }
}

fn is_strong_tag(tag: &str) -> bool {
    matches!(tag, "strong" | "b")
}

fn is_emphasis_tag(tag: &str) -> bool {
    matches!(tag, "em" | "i" | "cite")
}

fn is_bridgehead_span(start: &quick_xml::events::BytesStart<'_>) -> bool {
    start.attributes().flatten().any(|attr| {
        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
        let value = String::from_utf8_lossy(attr.value.as_ref());
        key == "type" && value.split_whitespace().any(|part| part == "bridgehead")
    })
}

fn should_insert_space(previous: &str, next: &str) -> bool {
    let Some(previous_char) = previous.chars().next_back() else {
        return false;
    };
    let Some(next_char) = next.chars().next() else {
        return false;
    };
    !previous_char.is_whitespace() && !matches!(next_char, '.' | ',' | ';' | ':' | '!' | '?' | ')')
}

fn trim_spans(spans: &mut Vec<InlineSpan>) {
    while spans
        .first()
        .is_some_and(|span| span.text.trim().is_empty())
    {
        spans.remove(0);
    }
    while spans.last().is_some_and(|span| span.text.trim().is_empty()) {
        spans.pop();
    }
    if let Some(first) = spans.first_mut() {
        first.text = first.text.trim_start().to_string();
    }
    if let Some(last) = spans.last_mut() {
        last.text = last.text.trim_end().to_string();
    }
}

fn normalize_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parent_dir(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn join_zip_path(base: &str, href: &str) -> String {
    if base.is_empty() {
        href.to_string()
    } else {
        format!("{base}/{href}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn extracts_spine_blocks_in_order() {
        let epub_path = fixture_epub(&[
            (
                "c1",
                "chapter1.xhtml",
                r#"<html><body><h1>One</h1><p>First paragraph.</p></body></html>"#,
            ),
            (
                "c2",
                "chapter2.xhtml",
                r#"<html><body><h1>Two</h1><p>Second paragraph.</p></body></html>"#,
            ),
        ]);

        let book = read_epub(&epub_path).expect("fixture EPUB should parse");
        assert_eq!(book.title, "Fixture Book");
        assert_eq!(book.chapters[0].title, "One");
        assert_eq!(book.chapters[0].blocks[0].kind, BlockKind::Heading(1));
        assert_eq!(book.chapters[0].blocks[0].plain_text(), "One");
        assert_eq!(book.chapters[0].blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(book.chapters[0].blocks[1].plain_text(), "First paragraph.");
        assert_eq!(book.chapters[1].title, "Two");
        assert_eq!(book.chapters[1].blocks[0].plain_text(), "Two");
        assert_eq!(book.chapters[1].blocks[1].plain_text(), "Second paragraph.");
        assert_eq!(book.block_count(), 4);
        assert_eq!(book.chapter_start_block(1), 2);
    }

    #[test]
    fn preserves_reader_structure_and_inline_emphasis() {
        let epub_path = fixture_epub(&[(
            "c1",
            "chapter1.xhtml",
            r#"<html><body><h1>One</h1><h2>Scene</h2><p>Plain <strong>bold</strong> and <em>italic</em>.</p></body></html>"#,
        )]);

        let book = read_epub(&epub_path).expect("fixture EPUB should parse");
        let blocks = &book.chapters[0].blocks;
        assert_eq!(blocks[0].kind, BlockKind::Heading(1));
        assert_eq!(blocks[1].kind, BlockKind::Heading(2));
        assert_eq!(blocks[2].kind, BlockKind::Paragraph);
        assert_eq!(blocks[2].plain_text(), "Plain bold and italic.");
        assert!(blocks[2]
            .spans
            .iter()
            .any(|span| span.strong && span.text.trim() == "bold"));
        assert!(blocks[2]
            .spans
            .iter()
            .any(|span| span.emphasis && span.text.trim() == "italic"));
    }

    #[test]
    fn preserves_epub_bridgehead_and_cite_semantics() {
        // Regression: 2026-05-27. Real EPUBs often encode bold lead-ins as
        // epub:type="bridgehead" spans and book titles as cite tags.
        let epub_path = fixture_epub(&[(
            "c1",
            "chapter1.xhtml",
            r#"<html xmlns:epub="http://www.idpf.org/2007/ops"><body><p><span epub:type="bridgehead">The modern fairy story.</span> Read <cite>Alice</cite>.</p></body></html>"#,
        )]);

        let book = read_epub(&epub_path).expect("fixture EPUB should parse");
        let spans = &book.chapters[0].blocks[0].spans;
        assert!(spans
            .iter()
            .any(|span| span.strong && span.text.contains("The modern fairy story")));
        assert!(spans
            .iter()
            .any(|span| span.emphasis && span.text.contains("Alice")));
    }

    fn fixture_epub(chapters: &[(&str, &str, &str)]) -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let epub_path = dir.keep().join("sample.epub");
        let file = File::create(&epub_path).expect("fixture EPUB file should be created");
        let mut zip = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default();

        zip.start_file("META-INF/container.xml", opts)
            .expect("container entry should start");
        zip.write_all(
            br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="OPS/content.opf"/></rootfiles></container>"#,
        )
        .expect("container entry should be written");

        let manifest = chapters
            .iter()
            .map(|(id, href, _)| format!(r#"<item id="{id}" href="{href}"/>"#))
            .collect::<String>();
        let spine = chapters
            .iter()
            .map(|(id, _, _)| format!(r#"<itemref idref="{id}"/>"#))
            .collect::<String>();
        zip.start_file("OPS/content.opf", opts)
            .expect("OPF entry should start");
        zip.write_all(
            format!(
                r#"<package><metadata><dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">Fixture Book</dc:title></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
            )
            .as_bytes(),
        )
        .expect("OPF entry should be written");

        for (_, href, body) in chapters {
            zip.start_file(format!("OPS/{href}"), opts)
                .expect("chapter entry should start");
            zip.write_all(body.as_bytes())
                .expect("chapter entry should be written");
        }

        zip.finish().expect("fixture EPUB zip should finish");
        epub_path
    }
}
