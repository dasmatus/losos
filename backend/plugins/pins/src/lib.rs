//! The pins plugin: `/api/pins`, what the owner pinned to the admin page's
//! sidebar. What a pin is and which ones are refused is [`model`]; this
//! file is the two routes and `pins.json` beside `state.json`, the one root
//! the manifest grants.

pub mod model;

use losos_plugin::{config, fs, lock, Plugin, Request, Response};
use model::{Invalid, Pins, Store};

/// `pins.json`, from the manifest's config.
pub struct FileStore {
    path: String,
}

impl FileStore {
    fn from_config() -> Self {
        FileStore {
            path: config::get_or("pinsFile", "/var/lib/losos/pins.json"),
        }
    }
}

impl Store for FileStore {
    fn load(&mut self) -> Pins {
        match fs::read_optional(&self.path) {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                losos_plugin::log::warn(&format!(
                    "unreadable pins file {}; showing none: {e}",
                    self.path
                ));
                Pins::default()
            }),
            Ok(None) => Pins::default(),
            Err(e) => {
                losos_plugin::log::warn(&format!(
                    "pins file {} unreadable; showing none: {e}",
                    self.path
                ));
                Pins::default()
            }
        }
    }

    fn save(&mut self, pins: &Pins) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(pins)?;
        // Only lososd and the root backup read it.
        fs::write(&self.path, &bytes, fs::WriteOptions::secret())?;
        Ok(())
    }
}

pub struct PinsPlugin;

impl Plugin for PinsPlugin {
    fn handle(req: Request) -> Response {
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/pins") => Response::ok(&model::cmd_pins(&mut FileStore::from_config())),
            // The whole list, in order, so two pages saving at once each
            // write a whole list; the lock keeps the writes apart.
            ("PUT", "/api/pins") => {
                let pins = match model::parse_pins(&req.json_value()) {
                    Ok(pins) => pins,
                    Err(e) => return refused(&e),
                };
                match lock::acquire("pins") {
                    Ok(_guard) => match model::cmd_put_pins(&mut FileStore::from_config(), &pins) {
                        Ok(v) => Response::ok(&v),
                        Err(e) => refused(&e),
                    },
                    Err(e) => Response::failed(&e),
                }
            }
            _ => Response::error(404, "not found"),
        }
    }
}

/// A 400 with the sentence for an [`Invalid`], else a 500 whose detail goes
/// to the journal.
fn refused(e: &anyhow::Error) -> Response {
    match e.downcast_ref::<Invalid>() {
        Some(why) => Response::error(400, &why.to_string()),
        None => Response::failed(e),
    }
}

losos_plugin::export!(PinsPlugin);
