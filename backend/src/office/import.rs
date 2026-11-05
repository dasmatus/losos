//! Best-effort import of legacy office formats into native text/MDX content.

pub fn escape_mdx(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn extract_text(name: &str, bytes: &[u8]) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "txt" | "csv" | "md" | "markdown") {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
        let mut out = String::new();
        let targets: &[&str] = match ext.as_str() {
            "odt" | "odp" | "ods" => &["content.xml"],
            "docx" => &["word/document.xml"],
            "pptx" => &["ppt/slides/slide1.xml", "ppt/slides/slide2.xml", "ppt/slides/slide3.xml"],
            "xlsx" => &["xl/sharedStrings.xml"],
            _ => &[],
        };
        for target in targets {
            if let Ok(mut f) = zip.by_name(target) {
                let mut xml = String::new();
                if std::io::Read::read_to_string(&mut f, &mut xml).is_ok() {
                    let mut text = String::with_capacity(xml.len() / 2);
                    let mut in_tag = false;
                    for c in xml.chars() {
                        match c {
                            '<' => in_tag = true,
                            '>' => {
                                in_tag = false;
                                text.push(' ');
                            }
                            _ if !in_tag => text.push(c),
                            _ => {}
                        }
                    }
                    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !collapsed.is_empty() {
                        if !out.is_empty() {
                            out.push_str("\n\n");
                        }
                        out.push_str(&collapsed);
                    }
                }
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    format!("Imported from {name}. Formatting was not preserved.")
}

pub fn native_body(name: &str, text: &str, target: &str) -> (String, String) {
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    let safe = escape_mdx(text);
    let title = escape_mdx(stem);
    if target == "slides" {
        (
            format!("{stem}.slides"),
            format!("# {title}\n\n{safe}\n"),
        )
    } else {
        (format!("{stem}.paper"), format!("# {title}\n\n{safe}\n"))
    }
}
