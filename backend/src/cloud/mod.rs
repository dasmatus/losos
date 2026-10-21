//! LosOS Cloud: the owner's files, and the people's on the box, served by
//! the `losos-cloud` binary.
//!
//! [`store`] keeps the files and the table that gives each one a stable id
//! and an ETag. The rest are ways in: WebDAV ([`dav`]) for file managers and
//! the Nextcloud desktop and phone apps, the parts of Nextcloud's own API
//! those apps need to sign in and find their way ([`nc`]), and the JSON API
//! the web app at `/cloud/` uses ([`api`]). [`server`] puts them on one
//! listener.

pub mod api;
pub mod auth;
pub mod collab;
pub mod dav;
pub mod extensions;
pub mod import;
pub mod mesh;
pub mod nc;
pub mod preview;
pub mod search;
pub mod server;
pub mod share;
pub mod store;
pub mod util;
pub mod xml;

use store::{Entry, Store, StoreError};
use tokio::sync::broadcast;

/// What the server is told at start.
#[derive(Debug, Clone)]
pub struct Config {
    /// The name the apps show for the server.
    pub product: String,
    /// Days a deleted file stays in the trash bin.
    pub trash_days: i64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            product: "LosOS Cloud".into(),
            trash_days: 30,
        }
    }
}

/// A file changed, through any door.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub owner: String,
    pub file_id: i64,
    pub etag: String,
}

pub struct Cloud {
    pub store: Store,
    pub auth: auth::Auth,
    pub locks: dav::Locks,
    pub config: Config,
    changes: broadcast::Sender<Changed>,
    /// Sharing with people on other boxes and editing together, once
    /// [`mesh::start`] has run.
    pub mesh: std::sync::OnceLock<std::sync::Arc<mesh::Mesh>>,
}

impl Cloud {
    pub fn open(root: &std::path::Path, config: Config) -> Result<Cloud, StoreError> {
        let store = Store::open(root)?;
        let auth = auth::Auth::new(&store)?;
        store.with_db(|db| Ok(db.execute_batch(dav::PROPS_SCHEMA)?))?;
        extensions::init(&store)?;
        let (changes, _) = broadcast::channel(256);
        Ok(Cloud {
            store,
            auth,
            locks: dav::Locks::default(),
            config,
            changes,
            mesh: std::sync::OnceLock::new(),
        })
    }

    /// Tell whoever listens that a file has new content.
    pub fn changed(&self, owner: &str, e: &Entry) {
        let _ = self.changes.send(Changed {
            owner: owner.to_owned(),
            file_id: e.id,
            etag: e.etag.clone(),
        });
    }

    /// Hear about every file that gets new content from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<Changed> {
        self.changes.subscribe()
    }

    /// The periodic chores: unfinished uploads, old trash.
    pub fn housekeeping(&self) {
        dav::sweep_uploads(self);
        if let Ok(owners) = self.store.owners() {
            for o in owners {
                if let Err(e) = self.store.expire_trash(&o, self.config.trash_days) {
                    tracing::warn!("cloud: trash of {o}: {e}");
                }
            }
        }
    }
}

/// Set a file's mtime and tell the listeners.
pub fn set_mtime(cloud: &Cloud, owner: &str, path: &str, mtime: i64) -> Result<Entry, StoreError> {
    let e = cloud.store.set_mtime(owner, path, mtime)?;
    cloud.changed(owner, &e);
    Ok(e)
}
