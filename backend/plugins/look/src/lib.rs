//! The look plugin: `/api/look*`, each person's background picture and the
//! widgets the owner wrote by hand. What they are is [`model`]; this file is
//! the routes and the files they live in: `look.json` and `background.img`
//! beside `state.json` (the widgets and the box's own background), and
//! `looks/` beside them (one `<name>.json` and `<name>.img` per person),
//! each a root of its own in `look.json`, the manifest.

pub mod model;

use losos_plugin::{config, fs, lock, Plugin, Request, Response};
use model::{BackgroundChoice, Invalid, Look, LookPatch, Mine, Store, Whose};

/// The files, from the manifest's config.
pub struct FileStore {
    look: String,
    background: String,
    looks: String,
}

impl FileStore {
    fn from_config() -> Self {
        FileStore {
            look: config::get_or("lookFile", "/var/lib/losos/look.json"),
            background: config::get_or("backgroundFile", "/var/lib/losos/background.img"),
            looks: config::get_or("looksDir", "/var/lib/losos/looks"),
        }
    }

    /// A person's file, `<name>.json` or `<name>.img`; the name has passed
    /// [`model::is_person_name`].
    fn person_file(&self, name: &str, ext: &str) -> String {
        fs::join(&self.looks, &format!("{name}.{ext}"))
    }

    /// Whether `looks/` is there yet: a path under a root needs the root to
    /// exist, and nobody has a look of their own on a fresh box.
    fn has_looks(&self) -> bool {
        fs::exists(&self.looks).unwrap_or(false)
    }

    /// Make `looks/` before the first person's file goes into it.
    fn make_looks(&self) -> anyhow::Result<()> {
        fs::create_dir(&self.looks, Some(0o700))?;
        Ok(())
    }

    fn picture(&self, name: Option<&str>) -> String {
        match name {
            None => self.background.clone(),
            Some(name) => self.person_file(name, "img"),
        }
    }
}

impl Store for FileStore {
    fn load_look(&mut self) -> Look {
        match fs::read_optional(&self.look) {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                losos_plugin::log::warn(&format!(
                    "unreadable look file {}; using the plain look: {e}",
                    self.look
                ));
                Look::default()
            }),
            Ok(None) => Look::default(),
            Err(e) => {
                losos_plugin::log::warn(&format!(
                    "look file {} unreadable; using the plain look: {e}",
                    self.look
                ));
                Look::default()
            }
        }
    }

    fn save_look(&mut self, look: &Look) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(look)?;
        fs::write(&self.look, &bytes, fs::WriteOptions::secret())?;
        Ok(())
    }

    fn load_person(&mut self, name: &str) -> Option<Mine> {
        if !self.has_looks() {
            return None;
        }
        let path = self.person_file(name, "json");
        match fs::read_optional(&path) {
            Ok(Some(bytes)) => Some(serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                losos_plugin::log::warn(&format!(
                    "unreadable look file {path}; using the plain look: {e}"
                ));
                Mine::default()
            })),
            Ok(None) => None,
            Err(e) => {
                losos_plugin::log::warn(&format!(
                    "look file {path} unreadable; using the plain look: {e}"
                ));
                Some(Mine::default())
            }
        }
    }

    fn save_person(&mut self, name: &str, mine: &Mine) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(mine)?;
        self.make_looks()?;
        fs::write(
            &self.person_file(name, "json"),
            &bytes,
            fs::WriteOptions::secret(),
        )?;
        Ok(())
    }

    fn write_picture(&mut self, name: Option<&str>, bytes: &[u8]) -> anyhow::Result<()> {
        if name.is_some() {
            self.make_looks()?;
        }
        fs::write(&self.picture(name), bytes, fs::WriteOptions::secret())?;
        Ok(())
    }

    fn read_picture(&mut self, name: Option<&str>) -> anyhow::Result<Option<Vec<u8>>> {
        if name.is_some() && !self.has_looks() {
            return Ok(None);
        }
        Ok(fs::read_optional(&self.picture(name))?)
    }

    fn remove_picture(&mut self, name: Option<&str>) -> anyhow::Result<()> {
        if name.is_some() && !self.has_looks() {
            return Ok(());
        }
        fs::remove_if_present(&self.picture(name))?;
        Ok(())
    }
}

/// A command's answer: its JSON, a 400 with the sentence for an
/// [`Invalid`], or a 500 whose detail goes to the journal.
fn answer(result: anyhow::Result<serde_json::Value>) -> Response {
    match result {
        Ok(v) => Response::ok(&v),
        Err(e) => match e.downcast_ref::<Invalid>() {
            Some(why) => Response::error(400, &why.to_string()),
            None => Response::failed(&e),
        },
    }
}

/// Run `f` on the store with the look's lock held, so two changes at once
/// cannot lose one of them.
fn locked(f: impl FnOnce(&mut FileStore) -> anyhow::Result<serde_json::Value>) -> Response {
    match lock::acquire("look") {
        Ok(_guard) => answer(f(&mut FileStore::from_config())),
        Err(e) => Response::failed(&e),
    }
}

pub struct LookPlugin;

impl Plugin for LookPlugin {
    fn handle(req: Request) -> Response {
        let whose = match Whose::of(&req.caller.principal) {
            Ok(whose) => whose,
            Err(why) => return Response::error(400, &why.to_string()),
        };
        match (req.method.as_str(), req.route.as_str()) {
            // Under the lock: the owner's first look may copy the box's.
            ("GET", "/api/look") => locked(|s| model::cmd_look(s, &whose)),
            ("POST", "/api/look") => match model::parse_patch(&req.json_value()) {
                Some(patch) => locked(|s| model::cmd_patch_look(s, &whose, &patch)),
                None => Response::error(
                    400,
                    r#"body must be JSON: {"background": {"kind": "none|shipped", "name": "..."}, "veil": 20-90}"#,
                ),
            },
            // The type is read off the bytes, never off the request; the
            // size cap is the route's maxBody plus the command's own check.
            ("PUT", "/api/look/background") => {
                locked(|s| model::cmd_upload_background(s, &whose, &req.body))
            }
            ("DELETE", "/api/look/background") => {
                let patch = LookPatch {
                    background: Some(BackgroundChoice::None),
                    veil: None,
                };
                locked(|s| model::cmd_patch_look(s, &whose, &patch))
            }
            // The box's own, with no token, and a person's, to them alone:
            // someone else's name is a picture that is not there.
            ("GET", "/api/look/background") => picture(None),
            ("GET", "/api/look/background/{name}") => {
                let name = req.param("name").unwrap_or_default();
                if whose.name_is(name) {
                    picture(Some(name))
                } else {
                    Response::error(404, "no picture")
                }
            }
            ("POST", "/api/look/widgets") => match model::parse_draft(&req.json_value()) {
                Some(draft) => locked(|s| model::cmd_put_widget(s, &draft)),
                None => Response::error(
                    400,
                    r#"body must be JSON: {"id": "..." (optional), "name": "...", "span": "half|full", "source": "..."}"#,
                ),
            },
            ("DELETE", "/api/look/widgets/{id}") => {
                let id = req.param("id").unwrap_or_default().to_string();
                locked(|s| model::cmd_delete_widget(s, &id))
            }
            _ => Response::error(404, "not found"),
        }
    }
}

/// An uploaded picture, cached hard: a replacement is a new `?v=`, so a new
/// URL.
fn picture(name: Option<&str>) -> Response {
    match model::cmd_read_background(&mut FileStore::from_config(), name) {
        Ok(Some((content_type, bytes))) => Response::bytes(200, &content_type, bytes)
            .header("cache-control", "private, max-age=31536000, immutable")
            .header("x-content-type-options", "nosniff"),
        Ok(None) => Response::error(404, "no picture"),
        Err(e) => Response::failed(&e),
    }
}

losos_plugin::export!(LookPlugin);
