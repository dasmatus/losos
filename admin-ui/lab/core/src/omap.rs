//! A string-keyed map that remembers insertion order, like a JS `Map`, and
//! keeps a key's first position when it is set again.

use serde::ser::{Serialize, SerializeMap, Serializer};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct OMap<V> {
    keys: Vec<String>,
    vals: Vec<V>,
    idx: HashMap<String, usize>,
}

impl<V> Default for OMap<V> {
    fn default() -> Self {
        OMap {
            keys: Vec::new(),
            vals: Vec::new(),
            idx: HashMap::new(),
        }
    }
}

impl<V> OMap<V> {
    pub fn get(&self, k: &str) -> Option<&V> {
        self.idx.get(k).map(|&i| &self.vals[i])
    }
    pub fn get_mut(&mut self, k: &str) -> Option<&mut V> {
        match self.idx.get(k) {
            Some(&i) => Some(&mut self.vals[i]),
            None => None,
        }
    }
    pub fn contains(&self, k: &str) -> bool {
        self.idx.contains_key(k)
    }
    pub fn insert(&mut self, k: String, v: V) {
        if let Some(&i) = self.idx.get(&k) {
            self.vals[i] = v;
        } else {
            self.idx.insert(k.clone(), self.keys.len());
            self.keys.push(k);
            self.vals.push(v);
        }
    }
    pub fn get_or_insert_with(&mut self, k: &str, f: impl FnOnce() -> V) -> &mut V {
        let i = match self.idx.get(k) {
            Some(&i) => i,
            None => {
                self.idx.insert(k.to_string(), self.keys.len());
                self.keys.push(k.to_string());
                self.vals.push(f());
                self.vals.len() - 1
            }
        };
        &mut self.vals[i]
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.keys.iter().zip(self.vals.iter())
    }
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.vals.iter()
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.vals.iter_mut()
    }
}

impl<V: Serialize> Serialize for OMap<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.keys.len()))?;
        for (k, v) in self.iter() {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}
