//! A little memory that outlives a call, in lososd, gone at its restart.

#[must_use]
pub fn get(key: &str) -> Option<Vec<u8>> {
    crate::wit::cache::get(key)
}

/// Keep `value` at most `ttl_secs` seconds; 0 keeps it until evicted.
pub fn set(key: &str, value: &[u8], ttl_secs: u32) {
    crate::wit::cache::set(key, value, ttl_secs);
}

pub fn remove(key: &str) {
    crate::wit::cache::remove(key);
}

/// A JSON value from the cache.
#[must_use]
pub fn get_json<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    get(key).and_then(|b| serde_json::from_slice(&b).ok())
}

/// A JSON value into the cache.
pub fn set_json<T: serde::Serialize>(key: &str, value: &T, ttl_secs: u32) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        set(key, &bytes, ttl_secs);
    }
}
