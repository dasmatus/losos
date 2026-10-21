//! The look plugin: `/api/look*`, a background picture and the widgets the
//! owner wrote by hand. What they are is [`model`]; this file is the routes
//! and the two files they live in, `look.json` and `background.img` beside
//! `state.json`, each a root of their own in `look.json`, the manifest.

pub mod model;

use losos_plugin::{config, fs, lock, Plugin, Request, Response};
use model::{BackgroundChoice, Invalid, Look, LookPatch, Store};

/// The two files, from the manifest's config.
pub struct FileStore {
    look: String,
    background: String,
}

impl FileStore {
    fn from_config() -> Self {
        FileStore {
            look: config::get_or("lookFile", "/var/lib/losos/look.json"),
            background: config::get_or("backgroundFile", "/var/lib/losos/background.img"),
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

    fn write_background(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        fs::write(&self.background, bytes, fs::WriteOptions::secret())?;
        Ok(())
    }

    fn read_background(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(fs::read_optional(&self.background)?)
    }

    fn remove_background(&mut self) -> anyhow::Result<()> {
        fs::remove_if_present(&self.background)?;
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
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/look") => answer(model::cmd_look(&mut FileStore::from_config())),
            ("POST", "/api/look") => match model::parse_patch(&req.json_value()) {
                Some(patch) => locked(|s| model::cmd_patch_look(s, &patch)),
                None => Response::error(
                    400,
                    r#"body must be JSON: {"background": {"kind": "none|shipped", "name": "..."}, "veil": 20-90}"#,
                ),
            },
            // The type is read off the bytes, never off the request; the
            // size cap is the route's maxBody plus the command's own check.
            ("PUT", "/api/look/background") => {
                locked(|s| model::cmd_upload_background(s, &req.body))
            }
            ("DELETE", "/api/look/background") => {
                let patch = LookPatch {
                    background: Some(BackgroundChoice::None),
                    veil: None,
                };
                locked(|s| model::cmd_patch_look(s, &patch))
            }
            // Cached hard: a replacement is a new `?v=`, so a new URL.
            ("GET", "/api/look/background") => {
                match model::cmd_read_background(&mut FileStore::from_config()) {
                    Ok(Some((content_type, bytes))) => Response::bytes(200, &content_type, bytes)
                        .header("cache-control", "private, max-age=31536000, immutable")
                        .header("x-content-type-options", "nosniff"),
                    Ok(None) => Response::error(404, "no picture"),
                    Err(e) => Response::failed(&e),
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

losos_plugin::export!(LookPlugin);
