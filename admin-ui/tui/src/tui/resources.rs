//! The reads a pane needs, fetched when the pane opens and again on `r`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Resource {
    Loading,
    Ok(Value),
    Error {
        message: String,
        status: Option<u16>,
    },
}

impl Resource {
    pub fn data(&self) -> Option<&Value> {
        match self {
            Resource::Ok(v) => Some(v),
            _ => None,
        }
    }
}

static LOADING: Resource = Resource::Loading;

#[derive(Debug, Default)]
pub struct Resources {
    store: HashMap<String, Resource>,
    asked: HashSet<String>,
}

impl Resources {
    pub fn get(&self, path: &str) -> &Resource {
        self.store.get(path).unwrap_or(&LOADING)
    }

    /// The paths of `paths` not fetched yet, marked as asked for.
    pub fn take_unasked(&mut self, paths: &[&str]) -> Vec<String> {
        paths
            .iter()
            .filter(|p| self.asked.insert((**p).to_string()))
            .map(|p| (*p).to_string())
            .collect()
    }

    pub fn mark_asked(&mut self, path: &str) {
        self.asked.insert(path.to_string());
    }

    pub fn put(&mut self, path: String, resource: Resource) {
        self.store.insert(path, resource);
    }
}
