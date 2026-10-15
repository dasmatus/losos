//! What goes wrong, as miette diagnostics: one sentence the user can act on,
//! a code that names the failure, sometimes a hint, and the exit status the
//! command ends with (1, 2 for a refused host key, 130 for a cancel).

use std::fmt;

use miette::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub message: String,
    pub code: &'static str,
    pub help: Option<String>,
    pub exit: u8,
}

impl Problem {
    /// A failure with exit status 1.
    pub fn new(code: &'static str, message: impl Into<String>) -> Problem {
        Problem {
            message: message.into(),
            code,
            help: None,
            exit: 1,
        }
    }

    pub fn help(mut self, help: impl Into<String>) -> Problem {
        self.help = Some(help.into());
        self
    }

    pub fn exit(mut self, exit: u8) -> Problem {
        self.exit = exit;
        self
    }

    /// The user stopped at a prompt.
    pub fn cancelled() -> Problem {
        Problem::new("losos::cancelled", crate::tr!("tui.login.cancelled")).exit(130)
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Problem {}

impl Diagnostic for Problem {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(self.code))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help
            .as_deref()
            .map(|h| Box::new(h) as Box<dyn fmt::Display>)
    }
}

pub type Result<T, E = Problem> = std::result::Result<T, E>;

impl From<losos_access::UserKeyError> for Problem {
    fn from(e: losos_access::UserKeyError) -> Problem {
        use losos_access::UserKeyError as E;
        let key = match e {
            E::NotArmoured => "tui.key.notArmoured",
            E::NotSecret => "tui.key.notSecret",
            E::CannotSign => "tui.key.cannotSign",
            E::NoEncryption => "tui.key.noEncryption",
            E::WrongPassphrase => "tui.key.wrongPassphrase",
        };
        Problem::new("losos::key", crate::i18n::t(key))
    }
}
