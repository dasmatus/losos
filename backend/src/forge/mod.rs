//! LosOS Git: the box's own Git server, the `losos-git` binary.
//!
//! It replaced Forgejo. What LosOS used from Forgejo was small next to what
//! Forgejo is: Git over HTTPS (the box has no SSH), access tokens per
//! computer, a private repository for the box's own configuration, and a
//! web page per repository. The pages are the admin UI's `/git/` page
//! (`admin-ui/app/src/git/`); this server answers the Git protocol and the
//! JSON API behind them.
//!
//! The layout on disk is Forgejo's, so `losos-search`'s `git grep` and the
//! backup read it the same way: one bare repository per project at
//! `<state>/repositories/<owner>/<name>.git`, both names lower case. What
//! git does not keep (accounts, tokens, who may see what) is one SQLite file,
//! `<state>/git.db`.
//!
//! Git itself moves the data. `git upload-pack` and `git receive-pack` with
//! `--stateless-rpc` speak the protocol, and [`transport`] only pipes them to
//! HTTP, which is also what Forgejo did. There is no Rust implementation of
//! the server side of the protocol to use instead.
//!
//! Three listeners, three kinds of caller ([`server`]):
//!
//!   * `http.sock`, a Unix socket only nginx (and root) can open. nginx asks
//!     LosOS Login who is calling and names the person in
//!     `X-Losos-Cloud-User`, so that header is believed here and nowhere else.
//!   * `127.0.0.1:<port>`, for lososd's pushes of the configuration
//!     repository and anything else on the box with a token. Tokens only.
//!   * `internal.sock`, for the cloud's board service (issues on boards).
//!
//! Who may do what is in [`auth`], the names in [`names`], the records in
//! [`db`], and every call to `git` in [`gitcmd`].

pub mod api;
pub mod auth;
pub mod code;
pub mod db;
pub mod events;
pub mod gitcmd;
pub mod import;
pub mod internal;
pub mod matrix;
pub mod names;
pub mod pulls;
pub mod reviewer;
pub mod server;
pub mod statuses;
pub mod transport;
pub mod wiki;
pub mod worktrees;

use std::path::{Path, PathBuf};

/// How one `losos-git` process is set up, from its command line.
#[derive(Debug, Clone)]
pub struct Config {
    /// `/var/lib/losos-git`: the database, the repositories, the admin token.
    pub state_dir: PathBuf,
    /// The `git` binary.
    pub git: PathBuf,
    /// Nobody sees anything without signing in, public repositories included.
    pub require_sign_in: bool,
    /// What a new repository is when its maker does not say.
    pub default_private: bool,
    /// A push to a repository that does not exist yet, in the pusher's own
    /// namespace, makes it.
    pub push_create: bool,
    /// Where the pages are, below the box's address: `/git/`.
    pub base_path: String,
    /// How people from other boxes are vouched for.
    pub matrix: matrix::MatrixConfig,
}

impl Config {
    pub fn repositories(&self) -> PathBuf {
        self.state_dir.join("repositories")
    }

    pub fn database(&self) -> PathBuf {
        self.state_dir.join("git.db")
    }

    /// The token that makes a caller the site administrator, written on the
    /// first start (`server::ensure_admin_token`). lososd reads it.
    pub fn admin_token_file(&self) -> PathBuf {
        self.state_dir.join("admin-token")
    }

    /// Where one repository is on disk. Both names must have passed
    /// [`names`]' checks: that is what keeps the path inside the tree.
    pub fn repo_path(&self, owner: &str, name: &str) -> PathBuf {
        repo_path(&self.repositories(), owner, name)
    }

    /// Where a repository's wiki is: `<name>.wiki.git` beside it, as
    /// Forgejo kept them.
    pub fn wiki_path(&self, owner: &str, name: &str) -> PathBuf {
        repo_path(&self.repositories(), owner, &format!("{name}.wiki"))
    }
}

/// `<root>/<owner>/<name>.git`, lower case, Forgejo's layout.
pub fn repo_path(root: &Path, owner: &str, name: &str) -> PathBuf {
    root.join(owner.to_lowercase())
        .join(format!("{}.git", name.to_lowercase()))
}

/// The current time, RFC 3339 to the second, as every record keeps it.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
