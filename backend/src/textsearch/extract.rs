//! One file's text, for the cloud index.
//!
//! What gets read, by file name:
//!
//! * plain text, markup and source code (a long list of extensions, below),
//!   and any file with another name whose first 8 KiB are UTF-8 without a NUL
//!   byte;
//! * OpenDocument text, spreadsheets, presentations and drawings, and Word,
//!   Excel and PowerPoint files in their XML formats: the XML inside the zip,
//!   with the tags taken out;
//! * PDF, through poppler's `pdftotext`, when the unit has it.
//!
//! Pictures, audio, video, archives and the old binary Office formats are
//! skipped without being opened. Every reader is capped: at most
//! [`MAX_TEXT`] bytes of text are kept per file, a zip member is read to at
//! most 32 MiB however small it claims to be, and `pdftotext` gets 20 seconds.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// Bytes of text kept per file. A search finds a word in the first megabyte
/// of a document; the rest would only grow the index.
pub const MAX_TEXT: usize = 1 << 20;
/// Bytes read from a plain text file.
const MAX_PLAIN: u64 = 4 << 20;
/// Office documents and PDFs bigger than this are not opened at all.
pub const MAX_DOCUMENT: u64 = 64 << 20;
/// Decompressed bytes read from one member of a zip.
const MAX_MEMBER: u64 = 32 << 20;
/// Slides read from one presentation.
const MAX_SLIDES: usize = 500;
const SNIFF: usize = 8192;
const PDF_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Markup,
    Odf,
    Docx,
    Xlsx,
    Pptx,
    Pdf,
    /// Not text, by its name: not opened.
    Skip,
    /// Unknown by name: read if it looks like text.
    Sniff,
}

const PLAIN: &[&str] = &[
    "txt",
    "text",
    "md",
    "markdown",
    "mdx",
    "rst",
    "org",
    "adoc",
    "asciidoc",
    "tex",
    "bib",
    "csv",
    "tsv",
    "json",
    "jsonl",
    "ndjson",
    "yaml",
    "yml",
    "toml",
    "ini",
    "cfg",
    "conf",
    "env",
    "properties",
    "log",
    "srt",
    "vtt",
    "ics",
    "vcf",
    "eml",
    "css",
    "scss",
    "sass",
    "less",
    "js",
    "mjs",
    "cjs",
    "ts",
    "mts",
    "cts",
    "tsx",
    "jsx",
    "vue",
    "svelte",
    "py",
    "rb",
    "php",
    "pl",
    "lua",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "nix",
    "rs",
    "go",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cxx",
    "java",
    "kt",
    "kts",
    "swift",
    "scala",
    "cs",
    "fs",
    "dart",
    "r",
    "jl",
    "sql",
    "graphql",
    "proto",
    "gradle",
    "cmake",
    "mk",
    "dockerfile",
    "patch",
    "diff",
];
const MARKUP: &[&str] = &[
    "html", "htm", "xhtml", "xml", "fodt", "fods", "fodp", "fodg",
];
const ODF: &[&str] = &["odt", "ods", "odp", "odg", "ott", "ots", "otp", "otg"];
const DOCX: &[&str] = &["docx", "docm", "dotx", "dotm"];
const XLSX: &[&str] = &["xlsx", "xlsm", "xltx", "xltm"];
const PPTX: &[&str] = &["pptx", "pptm", "potx", "potm"];
const SKIP: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "avif", "bmp", "tif", "tiff", "raw",
    "cr2", "cr3", "nef", "arw", "dng", "psd", "xcf", "ico", "icns", "svg", "svgz", "mp3", "m4a",
    "flac", "wav", "ogg", "oga", "opus", "aac", "wma", "mp4", "m4v", "mov", "mkv", "webm", "avi",
    "wmv", "mpg", "mpeg", "3gp", "zip", "tar", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar", "iso",
    "img", "dmg", "qcow2", "vmdk", "exe", "dll", "so", "o", "a", "bin", "class", "jar", "apk",
    "deb", "rpm", "woff", "woff2", "ttf", "otf", "eot", "sqlite", "db", "kdbx", "gpg", "pgp",
    "age", "doc", "xls", "ppt", "part", "epub", "mobi",
];

/// What to do with a file, by its name alone.
pub fn kind(name: &str) -> Kind {
    let lower = name.to_ascii_lowercase();
    let ext = match lower.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext,
        // `Dockerfile`, `Makefile`, `README`: no extension, so look.
        _ => return Kind::Sniff,
    };
    let is = |list: &[&str]| list.contains(&ext);
    if is(PLAIN) {
        Kind::Plain
    } else if is(MARKUP) {
        Kind::Markup
    } else if is(ODF) {
        Kind::Odf
    } else if is(DOCX) {
        Kind::Docx
    } else if is(XLSX) {
        Kind::Xlsx
    } else if is(PPTX) {
        Kind::Pptx
    } else if ext == "pdf" {
        Kind::Pdf
    } else if is(SKIP) {
        Kind::Skip
    } else {
        Kind::Sniff
    }
}

/// The text of the file at `path`, or `None` when it has none worth keeping.
/// `pdftotext` is the binary PDFs go through; `None` skips PDFs.
pub fn extract(path: &Path, size: u64, pdftotext: Option<&Path>) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let text = match kind(name) {
        Kind::Skip => return None,
        Kind::Plain => read_plain(path)?,
        Kind::Markup => xml_text(&read_plain(path)?),
        Kind::Sniff => sniff(path)?,
        Kind::Pdf => pdf(path, size, pdftotext?)?,
        office => {
            if size > MAX_DOCUMENT {
                return None;
            }
            let mut zip = zip::ZipArchive::new(File::open(path).ok()?).ok()?;
            let members: Vec<String> = match office {
                Kind::Odf => vec!["content.xml".into()],
                Kind::Docx => vec![
                    "word/document.xml".into(),
                    "word/footnotes.xml".into(),
                    "word/endnotes.xml".into(),
                ],
                Kind::Xlsx => vec!["xl/sharedStrings.xml".into()],
                _ => slides(&zip),
            };
            let mut text = String::new();
            for member in members {
                if let Some(xml) = read_member(&mut zip, &member) {
                    text.push_str(&xml_text(&xml));
                    text.push('\n');
                }
                if text.len() >= MAX_TEXT {
                    break;
                }
            }
            text
        }
    };
    let text = clip(text);
    (!text.trim().is_empty()).then_some(text)
}

fn read_plain(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(MAX_PLAIN)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Read a file of unknown kind if it starts like UTF-8 text.
fn sniff(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut head = vec![0; SNIFF];
    let mut got = 0;
    while got < SNIFF {
        match file.read(&mut head[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(_) => return None,
        }
    }
    head.truncate(got);
    if head.contains(&0) {
        return None;
    }
    match std::str::from_utf8(&head) {
        Ok(_) => {}
        // A multi-byte character cut at the end of the window is fine.
        Err(e) if e.error_len().is_none() && got == SNIFF => {}
        Err(_) => return None,
    }
    let mut rest = Vec::new();
    file.take(MAX_PLAIN - got as u64)
        .read_to_end(&mut rest)
        .ok()?;
    head.extend_from_slice(&rest);
    Some(String::from_utf8_lossy(&head).into_owned())
}

fn pdf(path: &Path, size: u64, pdftotext: &Path) -> Option<String> {
    if size > MAX_DOCUMENT {
        return None;
    }
    let mut cmd = Command::new(pdftotext);
    cmd.args(["-q", "-enc", "UTF-8", "-nopgbrk", "-l", "500"])
        .arg(path)
        .arg("-");
    let (status, out) =
        super::run_capped(&mut cmd, MAX_TEXT * 2, Instant::now() + PDF_TIMEOUT).ok()??;
    // pdftotext exits 1..3 for a damaged or locked file; keep what it wrote.
    if !status.success() && out.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// A presentation's slides in order: slide1.xml, slide2.xml, ... slide10.xml.
fn slides(zip: &zip::ZipArchive<File>) -> Vec<String> {
    let mut found: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|n| {
            let n = n.ok()?;
            let number = n.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?;
            Some((number.parse().ok()?, n.to_string()))
        })
        .collect();
    found.sort();
    found.into_iter().take(MAX_SLIDES).map(|(_, n)| n).collect()
}

fn read_member(zip: &mut zip::ZipArchive<File>, name: &str) -> Option<String> {
    let member = zip.by_name(name).ok()?;
    let mut bytes = Vec::new();
    member.take(MAX_MEMBER).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Cut to [`MAX_TEXT`] bytes on a character boundary.
fn clip(mut text: String) -> String {
    if text.len() > MAX_TEXT {
        let mut at = MAX_TEXT;
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        text.truncate(at);
    }
    text
}

/// Elements whose start or end separates words. Everything else (a Word
/// run, a span) joins its text to its neighbour's, because documents split
/// one word across several of those.
const BREAKS: &[&str] = &[
    "p",
    "h",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "br",
    "tab",
    "s",
    "line-break",
    "tc",
    "td",
    "th",
    "tr",
    "li",
    "div",
    "si",
    "c",
    "table-cell",
    "list-item",
    "title",
    "section",
    "article",
    "header",
    "footer",
    "blockquote",
    "pre",
    "cr",
];

/// The text of an XML or HTML document: tags out, entities decoded, scripts
/// and styles dropped.
pub fn xml_text(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 3);
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        decode_into(&mut out, &rest[..lt]);
        rest = &rest[lt..];
        if let Some(body) = rest.strip_prefix("<!--") {
            rest = body.find("-->").map_or("", |end| &body[end + 3..]);
            continue;
        }
        if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body.find("]]>").unwrap_or(body.len());
            out.push_str(&body[..end]);
            rest = body.get(end + 3..).unwrap_or("");
            continue;
        }
        let Some(gt) = rest.find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        let local = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
        if !closing && (local == "script" || local == "style") {
            let close = format!("</{name}");
            rest = rest.find(&close).map_or("", |at| &rest[at..]);
            continue;
        }
        if BREAKS.contains(&local.as_str()) && !out.ends_with(char::is_whitespace) {
            out.push(if local == "p" || local == "h" {
                '\n'
            } else {
                ' '
            });
        }
    }
    decode_into(&mut out, rest);
    out
}

fn decode_into(out: &mut String, text: &str) {
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            e => e
                .strip_prefix("#x")
                .or_else(|| e.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| e.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn kinds_by_name() {
        assert_eq!(kind("notes.MD"), Kind::Plain);
        assert_eq!(kind("Report.docx"), Kind::Docx);
        assert_eq!(kind("budget.ods"), Kind::Odf);
        assert_eq!(kind("scan.pdf"), Kind::Pdf);
        assert_eq!(kind("holiday.JPG"), Kind::Skip);
        assert_eq!(kind("Makefile"), Kind::Sniff);
        assert_eq!(kind(".bashrc"), Kind::Sniff);
        assert_eq!(kind("data.weird"), Kind::Sniff);
    }

    #[test]
    fn xml_loses_its_tags_but_keeps_words_whole() {
        let docx = r#"<w:document><w:body><w:p><w:r><w:t>Hel</w:t></w:r><w:r><w:t xml:space="preserve">lo &amp; tax</w:t></w:r></w:p><w:p><w:r><w:t>Second</w:t></w:r></w:p></w:body></w:document>"#;
        assert_eq!(xml_text(docx).trim(), "Hello & tax\nSecond");
        let html = "<html><head><style>p{}</style><script>var x = 1 < 2;</script></head><body><p>Caf&#233; &lt;3</p><td>a</td><td>b</td></body></html>";
        assert_eq!(
            xml_text(html).split_whitespace().collect::<Vec<_>>(),
            ["Café", "<3", "a", "b"]
        );
    }

    #[test]
    fn broken_entities_stay_as_written() {
        let mut out = String::new();
        decode_into(&mut out, "fish & chips &bogus; &#x41;");
        assert_eq!(out, "fish & chips &bogus; A");
    }

    #[test]
    fn office_documents_are_read_from_their_zip() {
        let dir = assert_fs::TempDir::new().unwrap();
        let path = dir.path().join("plan.odt");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("mimetype", opts).unwrap();
        zip.write_all(b"application/vnd.oasis.opendocument.text")
            .unwrap();
        zip.start_file("content.xml", opts).unwrap();
        zip.write_all(
            b"<office:text><text:p>Tax return</text:p><text:p>due in May</text:p></office:text>",
        )
        .unwrap();
        zip.finish().unwrap();
        let size = std::fs::metadata(&path).unwrap().len();
        let text = extract(&path, size, None).unwrap();
        assert_eq!(
            text.split_whitespace().collect::<Vec<_>>(),
            ["Tax", "return", "due", "in", "May"]
        );
    }

    #[test]
    fn binary_files_are_not_text() {
        let dir = assert_fs::TempDir::new().unwrap();
        let blob = dir.path().join("blob.weird");
        std::fs::write(&blob, b"\x00\x01\x02text").unwrap();
        assert_eq!(extract(&blob, 7, None), None);
        let readme = dir.path().join("README");
        std::fs::write(&readme, "Read me first").unwrap();
        assert_eq!(extract(&readme, 13, None).as_deref(), Some("Read me first"));
        let pdf = dir.path().join("scan.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        assert_eq!(extract(&pdf, 8, None), None, "no pdftotext, no PDFs");
    }

    #[test]
    fn text_is_clipped_on_a_character_boundary() {
        let text = clip("ž".repeat(MAX_TEXT));
        assert!(text.len() <= MAX_TEXT);
        assert!(text.chars().all(|c| c == 'ž'));
    }
}
