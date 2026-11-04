//! LosOS Office's conversions: a document to Word, OpenDocument, PDF, a web
//! page or PowerPoint, and a Word or OpenDocument file back to a document.
//! Pandoc does the work, inside a rootless Podman container with no network
//! (modules/office-convert.nix); this one program is both sides of it.
//!
//! ```text
//! losos-convert serve --podman P --image REF --archive TAR
//!     On the box, as losos-convert: one HTTP request on stdin, the socket
//!     systemd hands over, answered on stdout. POST /export/<format> takes
//!     the JSON below; POST /import/<format>?folder=NAME takes the file.
//!     Each runs `losos-convert export|import` in a fresh container.
//!     GET /formats answers what it converts, without one.
//! losos-convert export --to FORMAT
//!     In the container: {"source", "lang"?, "pictures": {name: base64}}
//!     on stdin, the file on stdout.
//! losos-convert import --from FORMAT [--folder NAME]
//!     In the container: the file on stdin, {"source", "pictures"} on
//!     stdout, the pictures under NAME.
//! ```
//!
//! The document is written to HTML by losos-mdx (`render_for_export`), so
//! what Pandoc reads has been through the same allowlist as every page:
//! equations are MathML with their TeX beside them, and nothing in it runs.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use losos_mdx::{parse, Flavour, Resolver};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

/// The largest request: a document and its pictures, or a file to import.
const MAX_BODY: usize = 64 << 20;
/// The largest file a conversion may hand back.
const MAX_OUTPUT: usize = 128 << 20;
/// How long one conversion may take, container start included.
const DEADLINE: Duration = Duration::from_secs(120);
/// Where the container works: its only writable place, a tmpfs.
const WORK: &str = "/tmp/work";

/// What a document can be exported as, and the type it is served with.
const EXPORTS: &[(&str, &str)] = &[
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    ("odt", "application/vnd.oasis.opendocument.text"),
    ("pdf", "application/pdf"),
    ("html", "text/html; charset=utf-8"),
    (
        "pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
];

/// What can be read back into a document.
const IMPORTS: &[&str] = &["docx", "odt"];

#[derive(Deserialize)]
struct ExportRequest {
    source: String,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    pictures: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct Imported {
    source: String,
    pictures: BTreeMap<String, String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let result = match args.first().map(String::as_str) {
        Some("serve") => match (flag("--podman"), flag("--image"), flag("--archive")) {
            (Some(podman), Some(image), Some(archive)) => serve(&Podman {
                program: podman,
                image,
                archive,
            }),
            _ => Err("serve needs --podman, --image and --archive".into()),
        },
        Some("export") => match flag("--to") {
            Some(to) => export(&to),
            None => Err("export needs --to".into()),
        },
        Some("import") => match flag("--from") {
            Some(from) => import(&from, flag("--folder").as_deref().unwrap_or("pictures")),
            None => Err("import needs --from".into()),
        },
        _ => Err("usage: losos-convert serve|export|import".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("losos-convert: {e}");
            ExitCode::FAILURE
        }
    }
}

// ── In the container ──────────────────────────────────────────────────────

/// A picture's name as a document gives it: a relative path of plain
/// segments, so it lands under the work directory and nowhere else.
fn safe_path(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('/')
        && name.split('/').all(|seg| {
            !seg.is_empty()
                && seg != "."
                && seg != ".."
                && !seg.starts_with('.')
                && seg
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')'))
        })
}

/// Pictures the request carried, by the names the document uses; anything
/// else becomes nothing Pandoc could go and fetch.
struct Pictures(BTreeMap<String, String>);

impl Resolver for Pictures {
    fn file(&self, target: &str) -> Option<String> {
        Some(if self.0.contains_key(target) {
            target.to_string()
        } else {
            "#".to_string()
        })
    }
}

fn pandoc(args: &[&str]) -> Result<(), String> {
    let out = Command::new("pandoc")
        .args(args)
        .current_dir(WORK)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot start pandoc: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The page size typst knows, from the front matter's `page.size`.
fn paper(properties: Option<&serde_json::Value>) -> &'static str {
    match properties
        .and_then(|p| p.pointer("/page/size"))
        .and_then(|s| s.as_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("letter") => "us-letter",
        Some("legal") => "us-legal",
        Some("a5") => "a5",
        Some("a3") => "a3",
        _ => "a4",
    }
}

fn export(to: &str) -> Result<(), String> {
    if !EXPORTS.iter().any(|(f, _)| *f == to) {
        return Err(format!("cannot export to {to}"));
    }
    let mut body = Vec::new();
    std::io::stdin()
        .take(MAX_BODY as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    let req: ExportRequest =
        serde_json::from_slice(&body).map_err(|e| format!("not an export request: {e}"))?;

    std::fs::create_dir_all(WORK).map_err(|e| e.to_string())?;
    let mut kept = BTreeMap::new();
    for (name, data) in req.pictures {
        if !safe_path(&name) {
            continue;
        }
        let Ok(bytes) = B64.decode(data.as_bytes()) else {
            continue;
        };
        let path = Path::new(WORK).join(&name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        kept.insert(name, String::new());
    }

    let mut parsed = parse(&req.source, Flavour::Mdx).map_err(|e| e.to_string())?;
    parsed.lang = req.lang.clone();
    let rendered = parsed.render_for_export(&Pictures(kept));
    let props = rendered.properties.as_ref();
    let title = props
        .and_then(|p| p.get("title"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .or_else(|| rendered.headings.first().map(|h| h.text.clone()))
        .unwrap_or_else(|| "Document".to_string());
    let lang = props
        .and_then(|p| p.get("lang"))
        .and_then(|l| l.as_str())
        .map(str::to_string)
        .or(req.lang)
        .filter(|l| l.len() <= 16 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or_else(|| "en".to_string());
    std::fs::write(
        Path::new(WORK).join("doc.html"),
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{}</body></html>",
            rendered.html
        ),
    )
    .map_err(|e| e.to_string())?;

    let out = format!("out.{to}");
    let lang_meta = format!("lang={lang}");
    let title_meta = format!("pagetitle={title}");
    let size = format!("papersize={}", paper(props));
    let mut args = vec![
        "doc.html",
        "--from=html",
        "--resource-path=.",
        "--metadata",
        &lang_meta,
        "--output",
        &out,
    ];
    match to {
        "html" => args.extend([
            "--to=html5",
            "--standalone",
            "--mathml",
            "--embed-resources",
            "--metadata",
            &title_meta,
        ]),
        "pdf" => args.extend([
            "--pdf-engine=typst",
            "--variable",
            "mainfont=Inter",
            "--variable",
            &size,
        ]),
        _ => {}
    }
    pandoc(&args)?;
    let file = std::fs::read(Path::new(WORK).join(&out)).map_err(|e| e.to_string())?;
    // A web page goes out with no script, whatever Pandoc's template does.
    if to == "html"
        && String::from_utf8_lossy(&file)
            .to_ascii_lowercase()
            .contains("<script")
    {
        return Err("the web page came out with a script in it".into());
    }
    std::io::stdout()
        .write_all(&file)
        .map_err(|e| e.to_string())
}

/// Markdown from Pandoc made safe to read as MDX: outside code, a brace is
/// text, never an expression, and so is a `<` Pandoc left bare.
fn as_mdx(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut fence: Option<String> = None;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(f) = &fence {
            if trimmed.starts_with(f.as_str()) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let mark: String = trimmed
                .chars()
                .take_while(|c| *c == '`' || *c == '~')
                .collect();
            fence = Some(mark);
            out.push_str(line);
            continue;
        }
        // Inside code (backticks) and maths (dollars) the text is the
        // construct's own, as long as the run that opened it.
        let mut inside: Option<(char, usize)> = None;
        let mut prev = '\0';
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '`' | '$' if prev != '\\' => {
                    let mut run = 1;
                    while chars.peek() == Some(&c) {
                        chars.next();
                        run += 1;
                    }
                    match inside {
                        None => inside = Some((c, run)),
                        Some(open) if open == (c, run) => inside = None,
                        Some(_) => {}
                    }
                    out.extend(std::iter::repeat_n(c, run));
                    prev = c;
                    continue;
                }
                '{' | '}' | '<' if inside.is_none() && prev != '\\' => {
                    out.push('\\');
                    out.push(c);
                }
                _ => out.push(c),
            }
            prev = c;
        }
    }
    out
}

fn import(from: &str, folder: &str) -> Result<(), String> {
    if !IMPORTS.contains(&from) {
        return Err(format!("cannot import {from}"));
    }
    if !safe_path(folder) || folder.contains(' ') {
        return Err(format!("{folder} is not a folder name to put pictures in"));
    }
    std::fs::create_dir_all(WORK).map_err(|e| e.to_string())?;
    let input = format!("in.{from}");
    let mut file = Vec::new();
    std::io::stdin()
        .take(MAX_BODY as u64 + 1)
        .read_to_end(&mut file)
        .map_err(|e| e.to_string())?;
    std::fs::write(Path::new(WORK).join(&input), file).map_err(|e| e.to_string())?;
    pandoc(&[
        &input,
        &format!("--from={from}"),
        "--to=commonmark_x-raw_html-raw_attribute-attributes-fenced_divs-bracketed_spans-subscript-superscript-smart",
        "--wrap=none",
        &format!("--extract-media={folder}"),
        "--output=out.md",
    ])?;
    let markdown =
        std::fs::read_to_string(Path::new(WORK).join("out.md")).map_err(|e| e.to_string())?;
    let mut source = as_mdx(&markdown);
    // What still does not read as MDX comes in as text, line by line, so
    // nothing is lost and nothing is markup.
    if parse(&source, Flavour::Mdx).is_err() {
        source = markdown
            .lines()
            .map(losos_mdx_escape)
            .collect::<Vec<_>>()
            .join("\n");
    }
    let mut pictures = BTreeMap::new();
    collect(&Path::new(WORK).join(folder), WORK, &mut pictures)?;
    let answer = Imported { source, pictures };
    serde_json::to_writer(std::io::stdout(), &answer).map_err(|e| e.to_string())
}

/// A line of text with every character MDX or Markdown could read as syntax
/// escaped.
fn losos_mdx_escape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn collect(dir: &Path, root: &str, into: &mut BTreeMap<String, String>) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            collect(&path, root, into)?;
        } else if kind.is_file() {
            let name = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .to_string();
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            into.insert(name, B64.encode(bytes));
        }
    }
    Ok(())
}

// ── On the box ────────────────────────────────────────────────────────────

struct Podman {
    program: String,
    image: String,
    archive: String,
}

struct Http {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
}

fn read_request(input: impl Read) -> Result<Http, (u16, String)> {
    let mut reader = BufReader::new(input);
    let mut head = Vec::new();
    let mut length = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        let n = (&mut reader)
            .take(8192)
            .read_line(&mut line)
            .map_err(|e| (400, e.to_string()))?;
        if n == 0 {
            return Err((400, "the request ended early".into()));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if head.len() > 64 {
            return Err((431, "too many headers".into()));
        }
        head.push(line.trim_end().to_string());
    }
    let first = head.first().ok_or((400, "no request line".to_string()))?;
    let mut parts = first.split(' ');
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    for h in &head[1..] {
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                length = v
                    .trim()
                    .parse()
                    .map_err(|_| (400, "bad length".to_string()))?;
            }
        }
    }
    if length > MAX_BODY {
        return Err((413, format!("over {MAX_BODY} bytes")));
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(|e| (400, e.to_string()))?;
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    Ok(Http {
        method,
        path: path.to_string(),
        query: query.to_string(),
        body,
    })
}

fn respond(status: u16, kind: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        431 => "Request Header Fields Too Large",
        504 => "Gateway Timeout",
        _ => "Internal Server Error",
    };
    let mut out = std::io::stdout().lock();
    write!(
        out,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    out.write_all(body)?;
    out.flush()
}

fn error(status: u16, message: &str) -> std::io::Result<()> {
    let body = serde_json::json!({ "error": message }).to_string();
    respond(status, "application/json", body.as_bytes())
}

fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

/// What the converter takes, for the page to offer: `{"exports", "imports"}`.
fn formats() -> String {
    serde_json::json!({
        "exports": EXPORTS.iter().map(|(f, _)| *f).collect::<Vec<_>>(),
        "imports": IMPORTS,
    })
    .to_string()
}

fn serve(podman: &Podman) -> Result<(), String> {
    let req = match read_request(std::io::stdin().lock()) {
        Ok(r) => r,
        Err((status, message)) => return error(status, &message).map_err(|e| e.to_string()),
    };
    if req.method == "GET" && req.path == "/formats" {
        return respond(200, "application/json", formats().as_bytes()).map_err(|e| e.to_string());
    }
    if req.method != "POST" {
        return error(405, "only POST").map_err(|e| e.to_string());
    }
    let segments: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    let (args, kind): (Vec<String>, &str) = match segments.as_slice() {
        ["export", to] => match EXPORTS.iter().find(|(f, _)| f == to) {
            Some((f, kind)) => (vec!["export".into(), "--to".into(), (*f).into()], kind),
            None => return error(404, "no such format").map_err(|e| e.to_string()),
        },
        ["import", from] if IMPORTS.contains(from) => {
            let folder = query_value(&req.query, "folder").unwrap_or_else(|| "pictures".into());
            if !safe_path(&folder) || folder.contains(' ') || folder.contains('%') {
                return error(400, "a folder of plain letters").map_err(|e| e.to_string());
            }
            (
                vec![
                    "import".into(),
                    "--from".into(),
                    (*from).into(),
                    "--folder".into(),
                    folder,
                ],
                "application/json",
            )
        }
        _ => return error(404, "no such conversion").map_err(|e| e.to_string()),
    };
    match podman.run(&args, req.body) {
        Ok(out) => respond(200, kind, &out),
        Err(Failure::Timeout) => error(504, "the conversion took too long"),
        Err(Failure::Refused(why)) => error(422, &why),
        Err(Failure::Broken(why)) => {
            eprintln!("losos-convert: {why}");
            error(500, "the converter is not working")
        }
    }
    .map_err(|e| e.to_string())
}

enum Failure {
    Timeout,
    /// The document or file could not be converted; the words say why.
    Refused(String),
    /// Podman itself failed.
    Broken(String),
}

impl Podman {
    fn command(&self) -> Command {
        let mut c = Command::new(&self.program);
        c.stdin(Stdio::null());
        c
    }

    /// The image from the Nix store, loaded once per version.
    fn ensure_image(&self) -> Result<(), Failure> {
        let have = self
            .command()
            .args(["image", "exists", &self.image])
            .status()
            .map_err(|e| Failure::Broken(format!("cannot run podman: {e}")))?;
        if have.success() {
            return Ok(());
        }
        let out = self
            .command()
            .args(["load", "--quiet", "--input", &self.archive])
            .output()
            .map_err(|e| Failure::Broken(format!("cannot run podman: {e}")))?;
        if !out.status.success() {
            return Err(Failure::Broken(format!(
                "podman load: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    }

    fn run(&self, args: &[String], input: Vec<u8>) -> Result<Vec<u8>, Failure> {
        self.ensure_image()?;
        let name = format!(
            "losos-convert-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let mut child = Command::new(&self.program)
            .args([
                "run",
                "--rm",
                "--interactive",
                "--name",
                &name,
                "--pull=never",
                "--network=none",
                "--read-only",
                "--tmpfs=/tmp:rw,size=512m,mode=1777",
                "--cap-drop=all",
                "--security-opt=no-new-privileges",
                // The container stays in this service's cgroup, under its
                // memory and task limits.
                "--cgroups=disabled",
                "--log-driver=none",
                &self.image,
            ])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Failure::Broken(format!("cannot run podman: {e}")))?;
        let mut stdin = child.stdin.take().expect("piped");
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
        let mut stdout = child.stdout.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = (&mut stdout)
                .take(MAX_OUTPUT as u64 + 1)
                .read_to_end(&mut out);
            out
        });
        let mut stderr = child.stderr.take().expect("piped");
        let errors = std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = (&mut stderr).take(64 << 10).read_to_end(&mut out);
            out
        });
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if started.elapsed() > DEADLINE => {
                    let _ = self.command().args(["rm", "--force", &name]).status();
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Failure::Timeout);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(Failure::Broken(e.to_string())),
            }
        };
        let _ = writer.join();
        let out = reader.join().unwrap_or_default();
        let err = String::from_utf8_lossy(&errors.join().unwrap_or_default()).to_string();
        if status.success() && out.len() <= MAX_OUTPUT {
            return Ok(out);
        }
        // The program's own refusals start with its name; anything else is
        // Podman's.
        let mine: Vec<&str> = err
            .lines()
            .filter_map(|l| l.strip_prefix("losos-convert: "))
            .collect();
        if mine.is_empty() {
            Err(Failure::Broken(err.trim().to_string()))
        } else {
            Err(Failure::Refused(mine.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_names_stay_inside() {
        assert!(safe_path("tomato.png"));
        assert!(safe_path("Garden day files/tomato (1).png"));
        for bad in [
            "",
            "/etc/passwd",
            "../x.png",
            "a/../../x",
            ".hidden",
            "a//b",
            "a/./b",
        ] {
            assert!(!safe_path(bad), "{bad}");
        }
    }

    #[test]
    fn markdown_from_pandoc_reads_as_mdx() {
        let md = "Braces {x} and <y> stay text.\n\n```\n{code} <kept>\n```\n\nInline `{a}` too, and \\{b}, $r^{2}$ and \\$5 {c}.\n\n$$\\frac{1}{3}$$\n";
        let mdx = as_mdx(md);
        assert!(parse(&mdx, Flavour::Mdx).is_ok(), "{mdx}");
        assert!(mdx.contains("\\{x\\} and \\<y>"), "{mdx}");
        assert!(mdx.contains("{code} <kept>"), "{mdx}");
        assert!(mdx.contains("`{a}`"), "{mdx}");
        assert!(mdx.contains("\\{b\\}"), "{mdx}");
        assert!(mdx.contains("$r^{2}$"), "{mdx}");
        assert!(mdx.contains("\\$5 \\{c\\}"), "{mdx}");
        assert!(mdx.contains("$$\\frac{1}{3}$$"), "{mdx}");
    }

    #[test]
    fn a_request_is_read_with_its_body() {
        let raw = b"POST /export/docx?x=1 HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\n\r\nabcd";
        let r = read_request(&raw[..]).map_err(|e| e.1).unwrap();
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/export/docx");
        assert_eq!(r.query, "x=1");
        assert_eq!(r.body, b"abcd");
        let big = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        );
        assert_eq!(read_request(big.as_bytes()).err().map(|e| e.0), Some(413));
    }

    #[test]
    fn page_sizes_are_typst_names() {
        let letter = serde_json::json!({ "page": { "size": "Letter" } });
        assert_eq!(paper(Some(&letter)), "us-letter");
        assert_eq!(paper(None), "a4");
    }
}
