//! Transient systemd units, for work that outlives a call.

use crate::wit::units as w;

pub use w::UnitState as State;

/// A unit to start. `name` must carry the manifest's `units.prefix`.
#[derive(Debug, Clone, Default)]
pub struct Spec {
    pub name: String,
    pub argv: Vec<String>,
    pub properties: Vec<String>,
    pub env: Vec<(String, String)>,
    pub log: Option<String>,
}

pub fn start(spec: &Spec) -> crate::Result<()> {
    w::start(&w::Spec {
        name: spec.name.clone(),
        argv: spec.argv.clone(),
        properties: spec.properties.clone(),
        env: spec.env.clone(),
        log: spec.log.clone(),
    })
    .map_err(Into::into)
}

pub fn state(name: &str) -> crate::Result<State> {
    w::state(name).map_err(Into::into)
}

pub fn stop(name: &str) -> crate::Result<()> {
    w::stop(name).map_err(Into::into)
}
