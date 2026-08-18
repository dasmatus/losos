use std::fmt::Display;
#[derive(Clone, Copy)]
pub enum Action {
    Announce,
    Serve,
    BuildRuntime,
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Announce => f.write_str("[announce]"),
            Action::Serve => f.write_str("[serve]"),
            Action::BuildRuntime =>f.write_str("[build runtime]"),
        }
    }
}