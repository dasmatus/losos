//! The command line, with clap's derive. The words of `--help` come from the
//! catalogue, so the help reads in the language the rest does: clap builds
//! the command from the types below, and [`localized`] puts the catalogue's
//! sentences on it before it parses anything.

use clap::{ArgAction, Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::i18n::t;
use crate::tui::panes::VERSION;

#[derive(Debug, Parser)]
#[command(
    name = "losos",
    version = VERSION,
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true,
    max_term_width = 100
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// The host to open, as `losos open` takes it.
    #[arg(value_name = "URL")]
    pub url: Option<String>,

    #[command(flatten)]
    pub global: Global,
}

#[derive(Debug, Clone, Args)]
pub struct Global {
    #[arg(long, global = true, value_name = "URL")]
    pub host: Option<String>,

    #[arg(long, global = true, value_name = "en|sk|de")]
    pub lang: Option<String>,

    #[arg(long, global = true, value_name = "dark|light", value_parser = ["dark", "light"], hide_possible_values = true)]
    pub theme: Option<String>,

    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    #[arg(short = 'h', long, global = true, action = ArgAction::Help)]
    help: Option<bool>,

    #[arg(short = 'V', long, action = ArgAction::Version)]
    version: Option<bool>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Open {
        #[arg(value_name = "URL")]
        url: Option<String>,
    },
    Login {
        #[arg(value_name = "URL")]
        url: String,
        #[arg(long, value_name = "FINGERPRINT")]
        key: Option<String>,
        #[arg(long, value_name = "FPR")]
        fingerprint: Option<String>,
    },
    Logout {
        #[arg(value_name = "URL")]
        url: String,
    },
    Hosts,
    Key {
        #[command(subcommand)]
        command: KeyCommand,
    },
    Call {
        #[arg(value_name = "METHOD")]
        method: String,
        #[arg(value_name = "PATH")]
        path: String,
        #[arg(value_name = "JSON")]
        json: Option<String>,
    },
    Edge {
        #[command(subcommand)]
        command: EdgeCommand,
    },
    Ci {
        #[command(subcommand)]
        command: CiCommand,
    },
    Cloud {
        #[command(subcommand)]
        command: CloudCommand,
    },
    Git {
        #[command(subcommand)]
        command: GitCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum CloudCommand {
    Setup {
        #[arg(long, value_name = "NAME")]
        device: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum GitCommand {
    Setup {
        #[arg(long, value_name = "NAME")]
        device: Option<String>,
    },
    /// git's credential helper protocol, for the helper `git setup` names.
    #[command(hide = true)]
    Credential {
        #[arg(value_name = "ACTION")]
        action: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum KeyCommand {
    Import {
        #[arg(value_name = "FILE", required_unless_present = "gpg")]
        file: Option<String>,
        #[arg(long, value_name = "ID", conflicts_with = "file")]
        gpg: Option<String>,
    },
    List,
    Remove {
        #[arg(value_name = "FINGERPRINT")]
        fingerprint: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum EdgeCommand {
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
    Reconcile,
    Escrow {
        #[command(subcommand)]
        command: EscrowCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SecretCommand {
    List,
    Set {
        #[arg(value_name = "NAME")]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum EscrowCommand {
    Releaser {
        #[arg(long = "new", conflicts_with = "replace")]
        make: bool,
        #[arg(long)]
        replace: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum CiCommand {
    Setup {
        #[arg(long, value_name = "URL")]
        edge: String,
        #[arg(long, value_name = "OWNER/NAME", default_value = "dasmatus/losos")]
        repo: String,
        #[arg(long)]
        print: bool,
    },
}

/// Every subcommand's path and the catalogue key of its one-line help.
const ABOUT: &[(&[&str], &str)] = &[
    (&["open"], "tui.help.cmd.open"),
    (&["login"], "tui.help.cmd.login"),
    (&["logout"], "tui.help.cmd.logout"),
    (&["hosts"], "tui.help.cmd.hosts"),
    (&["key"], "tui.help.cmd.key"),
    (&["key", "import"], "tui.help.cmd.keyImport"),
    (&["key", "list"], "tui.help.cmd.keyList"),
    (&["key", "remove"], "tui.help.cmd.keyRemove"),
    (&["call"], "tui.help.cmd.call"),
    (&["edge"], "tui.help.cmd.edge"),
    (&["edge", "secret"], "tui.help.cmd.secret"),
    (&["edge", "secret", "list"], "tui.help.cmd.secretList"),
    (&["edge", "secret", "set"], "tui.help.cmd.secretSet"),
    (&["edge", "reconcile"], "tui.help.cmd.reconcile"),
    (&["edge", "escrow"], "tui.help.cmd.escrow"),
    (&["edge", "escrow", "releaser"], "tui.help.cmd.releaser"),
    (&["ci"], "tui.help.cmd.ci"),
    (&["ci", "setup"], "tui.help.cmd.ciSetup"),
    (&["cloud"], "tui.help.cmd.cloud"),
    (&["cloud", "setup"], "tui.help.cmd.cloudSetup"),
    (&["git"], "tui.help.cmd.git"),
    (&["git", "setup"], "tui.help.cmd.gitSetup"),
];

/// Every argument's id and the catalogue key of its help.
const ARGS: &[(&str, &str)] = &[
    ("host", "tui.help.opt.host"),
    ("lang", "tui.help.opt.lang"),
    ("theme", "tui.help.opt.theme"),
    ("verbose", "tui.help.opt.verbose"),
    ("help", "tui.help.opt.help"),
    ("version", "tui.help.opt.version"),
    ("url", "tui.help.arg.url"),
    ("key", "tui.help.opt.key"),
    ("fingerprint", "tui.help.opt.fingerprint"),
    ("file", "tui.help.arg.file"),
    ("gpg", "tui.help.cmd.keyImportGpg"),
    ("method", "tui.help.arg.method"),
    ("path", "tui.help.arg.path"),
    ("json", "tui.help.arg.json"),
    ("name", "tui.help.arg.name"),
    ("make", "tui.help.opt.newReleaser"),
    ("replace", "tui.help.opt.replaceReleaser"),
    ("edge", "tui.help.arg.edge"),
    ("repo", "tui.help.opt.repo"),
    ("print", "tui.help.opt.print"),
    ("device", "tui.help.opt.device"),
];

/// An argument whose help differs in one command from the table above.
const ARGS_AT: &[(&[&str], &str, &str)] = &[(
    &["key", "remove"],
    "fingerprint",
    "tui.help.arg.keyFingerprint",
)];

fn arg_help(path: &[&str], id: &str) -> Option<&'static str> {
    ARGS_AT
        .iter()
        .find(|(p, a, _)| *p == path && *a == id)
        .map(|(_, _, key)| *key)
        .or_else(|| ARGS.iter().find(|(a, _)| *a == id).map(|(_, key)| *key))
}

fn localize(mut cmd: clap::Command, path: &[&str]) -> clap::Command {
    if let Some((_, key)) = ABOUT.iter().find(|(p, _)| *p == path) {
        cmd = cmd.about(t(key));
    }
    let ids: Vec<String> = cmd
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();
    for id in ids {
        if let Some(key) = arg_help(path, &id) {
            let options = t("tui.help.options");
            cmd = cmd.mut_arg(id, |a| {
                let a = a.help(t(key));
                if a.is_positional() {
                    a.help_heading(t("tui.help.arguments"))
                } else {
                    a.help_heading(options)
                }
            });
        }
    }
    let names: Vec<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    for name in names {
        let mut sub_path = path.to_vec();
        sub_path.push(&name);
        let sub_path: Vec<&str> = sub_path.to_vec();
        cmd = cmd.mut_subcommand(&name, |s| localize(s, &sub_path));
    }
    cmd.subcommand_help_heading(t("tui.help.commands"))
        .help_template(format!(
            "{{about-with-newline}}\n{}: {{usage}}\n\n{{all-args}}{{after-help}}",
            t("tui.help.usage")
        ))
}

/// The command with the catalogue's words on it.
pub fn localized() -> clap::Command {
    // The second line of the root's usage lines up under the first, after
    // the heading in whichever language it is.
    with_usage_heading(t("tui.help.usage").chars().count())
}

/// The command, with the root's second usage line indented for a heading
/// `heading` characters wide. Help uses the catalogue's heading; clap's
/// error messages always say "Usage:".
fn with_usage_heading(heading: usize) -> clap::Command {
    let indent = " ".repeat(heading + 2);
    localize(Cli::command(), &[])
        .override_usage(format!("losos [OPTIONS] [URL]\n{indent}losos <COMMAND>"))
        .about(t("tui.help.what"))
        .after_help(format!("{}\n{}", t("tui.help.keychain"), t("tui.help.ci")))
}

/// Parse `args` (the program's name first) with the localized command.
pub fn parse(args: &[String]) -> Result<Cli, clap::Error> {
    use clap::error::ErrorKind;
    let for_errors = || with_usage_heading("Usage".len());
    let matches = match localized().try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            return Err(e)
        }
        Err(e) => {
            // Again, so the usage under the error lines up under "Usage:".
            return Err(for_errors().try_get_matches_from(args).err().unwrap_or(e));
        }
    };
    let cli = Cli::from_arg_matches(&matches)?;
    // `losos <url>` is short for `losos open <url>`; with a command after it
    // the URL would be dropped without a word. Global options may come
    // first, so clap's own conflict rule cannot say this.
    if let (Some(url), Some(_)) = (&cli.url, &cli.command) {
        return Err(for_errors().error(
            ErrorKind::ArgumentConflict,
            crate::tr!("tui.help.urlBeforeCommand", url = url),
        ));
    }
    Ok(cli)
}

/// `--lang` before clap runs, so clap's own help is in that language too.
pub fn lang_flag(args: &[String]) -> Option<String> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--" {
            break;
        }
        if let Some(v) = arg.strip_prefix("--lang=") {
            return Some(v.to_string());
        }
        if arg == "--lang" {
            return iter.next().cloned();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        std::iter::once("losos")
            .chain(list.iter().copied())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn the_command_is_consistent() {
        localized().debug_assert();
    }

    #[test]
    fn the_old_command_lines_still_parse() {
        let cli = parse(&args(&[
            "login",
            "https://box.local",
            "--fingerprint",
            "AB CD",
        ]))
        .unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Login {
                fingerprint: Some(_),
                ..
            })
        ));
        let cli = parse(&args(&["call", "GET", "/api/state", "--host", "box.local"])).unwrap();
        assert_eq!(cli.global.host.as_deref(), Some("box.local"));
        let cli = parse(&args(&["edge", "escrow", "releaser", "--new"])).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Edge {
                command: EdgeCommand::Escrow {
                    command: EscrowCommand::Releaser {
                        make: true,
                        replace: false
                    }
                }
            })
        ));
        let cli = parse(&args(&["key", "import", "--gpg", "me@example.org"])).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Key {
                command: KeyCommand::Import {
                    gpg: Some(_),
                    file: None
                }
            })
        ));
        let cli = parse(&args(&["https://box.local"])).unwrap();
        assert_eq!(cli.url.as_deref(), Some("https://box.local"));
        assert!(cli.command.is_none());
        let cli = parse(&args(&["ci", "setup", "--edge", "e.example"])).unwrap();
        assert!(
            matches!(cli.command, Some(Command::Ci { command: CiCommand::Setup { ref repo, .. } }) if repo == "dasmatus/losos")
        );
        let cli = parse(&args(&["cloud", "setup", "--device", "work pc"])).unwrap();
        assert!(
            matches!(cli.command, Some(Command::Cloud { command: CloudCommand::Setup { device: Some(ref d) } }) if d == "work pc")
        );
        let cli = parse(&args(&["git", "credential", "get"])).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Git {
                command: GitCommand::Credential { .. }
            })
        ));
        assert!(parse(&args(&["edge", "escrow", "releaser", "--new", "--replace"])).is_err());
        assert!(parse(&args(&["key", "import"])).is_err());
    }

    #[test]
    fn help_is_in_the_language_asked_for() {
        assert_eq!(
            lang_flag(&args(&["--lang", "sk", "hosts"])).as_deref(),
            Some("sk")
        );
        assert_eq!(
            lang_flag(&args(&["hosts", "--lang=de"])).as_deref(),
            Some("de")
        );
        let help = crate::i18n::with_locale(crate::i18n::Locale::De, || {
            localized().render_help().to_string()
        });
        assert!(help.contains(&crate::i18n::template(
            "tui.help.cmd.hosts",
            &[],
            crate::i18n::Locale::De
        )));
    }
}
