//! What a step says back to the runner: workflow commands in its output
//! (`::add-mask::`, `::set-output`, `::group::` ...) and the files of the
//! file commands (`$GITHUB_ENV`, `$GITHUB_OUTPUT`, `$GITHUB_PATH`,
//! `$GITHUB_STATE`, `$GITHUB_STEP_SUMMARY`), in GitHub's formats.

use std::collections::BTreeMap;

/// What one line of output asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// An ordinary line, to the log as it is.
    Line(String),
    Mask(String),
    Output(String, String),
    State(String, String),
    Group(String),
    EndGroup,
    /// error, warning or notice, with its message.
    Annotation(&'static str, String),
    Debug(String),
    /// A command GitHub switched off, or one LosOS does not have.
    Ignored(String),
}

/// GitHub's escaping of command data and properties.
fn unescape(s: &str, property: bool) -> String {
    let mut out = s.replace("%0D", "\r").replace("%0A", "\n");
    if property {
        out = out.replace("%3A", ":").replace("%2C", ",");
    }
    out.replace("%25", "%")
}

/// Reads output lines one at a time, honouring `::stop-commands::`.
#[derive(Debug, Default)]
pub struct Parser {
    stopped: Option<String>,
}

impl Parser {
    pub fn line(&mut self, line: &str) -> Command {
        let trimmed = line.trim_start();
        if let Some(token) = &self.stopped {
            if trimmed == format!("::{token}::") {
                self.stopped = None;
                return Command::Ignored(String::new());
            }
            return Command::Line(line.to_string());
        }
        let Some(rest) = trimmed.strip_prefix("::") else {
            return Command::Line(line.to_string());
        };
        let Some((head, data)) = rest.split_once("::") else {
            return Command::Line(line.to_string());
        };
        let (name, props) = match head.split_once(' ') {
            Some((n, p)) => (n, p),
            None => (head, ""),
        };
        let props: BTreeMap<String, String> = props
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), unescape(v.trim(), true)))
            .collect();
        let data = unescape(data, false);
        match name {
            "add-mask" => Command::Mask(data),
            "set-output" => match props.get("name") {
                Some(n) => Command::Output(n.clone(), data),
                None => Command::Line(line.to_string()),
            },
            "save-state" => match props.get("name") {
                Some(n) => Command::State(n.clone(), data),
                None => Command::Line(line.to_string()),
            },
            "group" => Command::Group(data),
            "endgroup" => Command::EndGroup,
            "error" => Command::Annotation("error", data),
            "warning" => Command::Annotation("warning", data),
            "notice" => Command::Annotation("notice", data),
            "debug" => Command::Debug(data),
            "stop-commands" => {
                self.stopped = Some(data);
                Command::Ignored(String::new())
            }
            "set-env" | "add-path" => Command::Ignored(format!(
                "::{name}:: is switched off, as on GitHub; write to $GITHUB_{} instead",
                if name == "set-env" { "ENV" } else { "PATH" }
            )),
            "echo" => Command::Ignored(String::new()),
            _ => Command::Line(line.to_string()),
        }
    }
}

/// A `$GITHUB_ENV` or `$GITHUB_OUTPUT` file: `name=value` lines, or
/// `name<<DELIMITER` followed by lines up to the delimiter.
pub fn parse_pairs(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        let heredoc = line.split_once("<<");
        let eq = line.split_once('=');
        let use_heredoc = match (heredoc, eq) {
            (Some((h, _)), Some((e, _))) => h.len() < e.len(),
            (Some(_), None) => true,
            _ => false,
        };
        if use_heredoc {
            let (name, delim) = heredoc.unwrap_or_default();
            let mut value = Vec::new();
            let mut closed = false;
            for l in lines.by_ref() {
                if l == delim {
                    closed = true;
                    break;
                }
                value.push(l);
            }
            if !closed {
                return Err(format!("{name}: the delimiter {delim} never came"));
            }
            out.push((name.to_string(), value.join("\n")));
        } else if let Some((name, value)) = eq {
            out.push((name.to_string(), value.to_string()));
        } else {
            return Err(format!("not name=value or name<<DELIMITER: {line}"));
        }
    }
    Ok(out)
}

/// Replace every masked value in `line` by `***`.
pub fn mask(line: &str, masks: &[String]) -> String {
    let mut out = line.to_string();
    for m in masks {
        if !m.is_empty() {
            out = out.replace(m.as_str(), "***");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_commands() {
        let mut p = Parser::default();
        assert_eq!(p.line("hello"), Command::Line("hello".into()));
        assert_eq!(p.line("::add-mask::s3cret"), Command::Mask("s3cret".into()));
        assert_eq!(
            p.line("::set-output name=v::1%0A2"),
            Command::Output("v".into(), "1\n2".into())
        );
        assert_eq!(
            p.line("::error file=a.rs,line=3::bad%25"),
            Command::Annotation("error", "bad%".into())
        );
        assert_eq!(p.line("::group::Build"), Command::Group("Build".into()));
        assert!(matches!(p.line("::set-env name=A::b"), Command::Ignored(_)));
        p.line("::stop-commands::tok");
        assert_eq!(
            p.line("::add-mask::x"),
            Command::Line("::add-mask::x".into())
        );
        p.line("::tok::");
        assert_eq!(p.line("::add-mask::x"), Command::Mask("x".into()));
    }

    #[test]
    fn file_commands() {
        let text = "A=1\nB<<EOF\nline 1\nline=2\nEOF\n\nC=x=y\n";
        assert_eq!(
            parse_pairs(text).unwrap(),
            vec![
                ("A".into(), "1".into()),
                ("B".into(), "line 1\nline=2".into()),
                ("C".into(), "x=y".into()),
            ]
        );
        assert!(parse_pairs("D<<END\nnever").is_err());
        assert_eq!(mask("token abc here", &["abc".into()]), "token *** here");
    }
}
