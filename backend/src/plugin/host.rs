//! What a plugin's imports do: one [`HostState`] per call, holding the
//! plugin's manifest, and an implementation of every interface of
//! `plugins/wit/plugin.wit` that checks the manifest before it acts.

use super::bindings::losos::plugin as wit;
use super::bindings::losos::plugin::types::Error;
use super::{Inner, Loaded};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use wasmtime::component::{Resource, ResourceTable};

/// The state of one call: who is calling (which plugin), its deadline, and
/// the resources it holds.
pub struct HostState {
    pub(super) rt: Arc<Inner>,
    pub(super) plugin: Arc<Loaded>,
    pub(super) table: ResourceTable,
    pub(super) limits: wasmtime::StoreLimits,
    pub(super) deadline: Instant,
}

/// The `http.body` resource: an exchange whose body is still arriving.
pub struct BodyStream(pub(super) super::net::Exchange);

/// The `lock.guard` resource. Dropping it, or the call ending, releases.
pub struct LockGuard(Arc<NamedLock>);

impl Drop for LockGuard {
    fn drop(&mut self) {
        let mut held = self.0.held.lock().unwrap_or_else(|e| e.into_inner());
        *held = false;
        self.0.freed.notify_one();
    }
}

/// One named lock, shared by every plugin that lists it.
#[derive(Default)]
pub struct NamedLock {
    held: Mutex<bool>,
    freed: Condvar,
}

/// Every named lock there is.
#[derive(Default)]
pub struct Locks(Mutex<HashMap<String, Arc<NamedLock>>>);

impl Locks {
    fn get(&self, name: &str) -> Arc<NamedLock> {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.entry(name.to_string()).or_default().clone()
    }

    /// Wait until `until` for `name`.
    pub fn acquire(&self, name: &str, until: Instant) -> Result<LockGuard, Error> {
        let lock = self.get(name);
        let mut held = lock.held.lock().unwrap_or_else(|e| e.into_inner());
        while *held {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Error::TimedOut);
            }
            held = lock
                .freed
                .wait_timeout(held, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        *held = true;
        drop(held);
        Ok(LockGuard(lock))
    }
}

/// A plugin's [`wit::cache`]: entries with an expiry, the oldest written
/// going first past [`CACHE_BYTES`].
#[derive(Default)]
pub struct Cache {
    entries: HashMap<String, (Vec<u8>, Option<Instant>, u64)>,
    bytes: usize,
    seq: u64,
}

/// What one plugin's cache may hold.
pub const CACHE_BYTES: usize = 16 * 1024 * 1024;

impl Cache {
    pub fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        let expired = match self.entries.get(key) {
            None => return None,
            Some((_, Some(at), _)) => Instant::now() >= *at,
            Some(_) => false,
        };
        if expired {
            self.remove(key);
            return None;
        }
        self.entries.get(key).map(|(v, _, _)| v.clone())
    }

    pub fn set(&mut self, key: &str, value: Vec<u8>, ttl: Option<Duration>) {
        self.remove(key);
        if value.len() + key.len() > CACHE_BYTES {
            return;
        }
        while self.bytes + value.len() + key.len() > CACHE_BYTES {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, _, seq))| *seq)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.remove(&oldest);
        }
        self.seq += 1;
        self.bytes += value.len() + key.len();
        self.entries.insert(
            key.to_string(),
            (value, ttl.map(|t| Instant::now() + t), self.seq),
        );
    }

    pub fn remove(&mut self, key: &str) {
        if let Some((v, _, _)) = self.entries.remove(key) {
            self.bytes -= v.len() + key.len();
        }
    }
}

impl HostState {
    fn name(&self) -> &str {
        &self.plugin.manifest.name
    }

    /// The earlier of `ms` from now and the call's deadline.
    fn until(&self, ms: Option<u32>, default: Duration) -> Instant {
        let wanted = ms.map_or(default, |ms| Duration::from_millis(u64::from(ms)));
        (Instant::now() + wanted).min(self.deadline)
    }
}

/// Note a refusal in the journal: a denied call is a plugin bug or worse.
fn noted<T>(plugin: &str, what: &str, r: Result<T, Error>) -> Result<T, Error> {
    if let Err(Error::Denied(why)) = &r {
        tracing::warn!(plugin, what, "denied: {why}");
    }
    r
}

impl wit::types::Host for HostState {}

impl wit::log::Host for HostState {
    fn log(&mut self, level: wit::log::Level, message: String) -> wasmtime::Result<()> {
        use wit::log::Level;
        let plugin = self.name();
        match level {
            Level::Trace => tracing::trace!(plugin, "{message}"),
            Level::Debug => tracing::debug!(plugin, "{message}"),
            Level::Info => tracing::info!(plugin, "{message}"),
            Level::Warn => tracing::warn!(plugin, "{message}"),
            Level::Error => tracing::error!(plugin, "{message}"),
        }
        Ok(())
    }
}

impl wit::clock::Host for HostState {
    fn now(&mut self) -> wasmtime::Result<u64> {
        Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()))
    }

    fn now_ms(&mut self) -> wasmtime::Result<u64> {
        Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)))
    }

    fn monotonic_ms(&mut self) -> wasmtime::Result<u64> {
        Ok(u64::try_from(self.rt.started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    fn sleep_ms(&mut self, ms: u32) -> wasmtime::Result<()> {
        let until = self.until(Some(ms), Duration::ZERO);
        std::thread::sleep(until.saturating_duration_since(Instant::now()));
        Ok(())
    }
}

impl wit::random::Host for HostState {
    fn bytes(&mut self, len: u32) -> wasmtime::Result<Vec<u8>> {
        let len = (len as usize).min(64 * 1024);
        let mut out = vec![0u8; len];
        rustix::rand::getrandom(&mut out, rustix::rand::GetRandomFlags::empty())
            .map_err(|e| wasmtime::Error::msg(format!("getrandom: {e}")))?;
        Ok(out)
    }
}

impl wit::config::Host for HostState {
    fn get(&mut self, key: String) -> wasmtime::Result<Option<String>> {
        Ok(self.plugin.manifest.config.get(&key).cloned())
    }
}

impl wit::files::Host for HostState {
    fn read(&mut self, path: String) -> wasmtime::Result<Result<Vec<u8>, Error>> {
        let r = super::files::read(&self.plugin.manifest.files, &path);
        Ok(noted(self.name(), "files.read", r))
    }

    fn read_tail(&mut self, path: String, max: u32) -> wasmtime::Result<Result<Vec<u8>, Error>> {
        let r = super::files::read_tail(&self.plugin.manifest.files, &path, max);
        Ok(noted(self.name(), "files.read-tail", r))
    }

    fn write(
        &mut self,
        path: String,
        data: Vec<u8>,
        options: wit::files::WriteOptions,
    ) -> wasmtime::Result<Result<(), Error>> {
        let r = super::files::write(&self.plugin.manifest.files, &path, &data, &options);
        Ok(noted(self.name(), "files.write", r))
    }

    fn append(&mut self, path: String, data: Vec<u8>) -> wasmtime::Result<Result<(), Error>> {
        let r = super::files::append(&self.plugin.manifest.files, &path, &data);
        Ok(noted(self.name(), "files.append", r))
    }

    fn remove(&mut self, path: String) -> wasmtime::Result<Result<(), Error>> {
        let r = super::files::remove(&self.plugin.manifest.files, &path);
        Ok(noted(self.name(), "files.remove", r))
    }

    fn remove_tree(&mut self, path: String) -> wasmtime::Result<Result<(), Error>> {
        let r = super::files::remove_tree(&self.plugin.manifest.files, &path);
        Ok(noted(self.name(), "files.remove-tree", r))
    }

    fn rename(&mut self, src: String, dst: String) -> wasmtime::Result<Result<(), Error>> {
        let r = super::files::rename(&self.plugin.manifest.files, &src, &dst);
        Ok(noted(self.name(), "files.rename", r))
    }

    fn create_dir(
        &mut self,
        path: String,
        mode: Option<u32>,
    ) -> wasmtime::Result<Result<(), Error>> {
        let r = super::files::create_dir(&self.plugin.manifest.files, &path, mode);
        Ok(noted(self.name(), "files.create-dir", r))
    }

    fn read_dir(&mut self, dir: String) -> wasmtime::Result<Result<Vec<wit::files::Entry>, Error>> {
        let r = super::files::read_dir(&self.plugin.manifest.files, &dir);
        Ok(noted(self.name(), "files.read-dir", r))
    }

    fn stat(&mut self, path: String) -> wasmtime::Result<Result<wit::files::Metadata, Error>> {
        let r = super::files::stat(&self.plugin.manifest.files, &path);
        Ok(noted(self.name(), "files.stat", r))
    }
}

impl wit::exec::Host for HostState {
    fn run(
        &mut self,
        command: wit::exec::Command,
    ) -> wasmtime::Result<Result<wit::exec::Output, Error>> {
        let m = &self.plugin.manifest;
        let r = super::exec::run(&m.exec, &m.files, &command, self.deadline);
        Ok(noted(self.name(), "exec.run", r))
    }
}

impl wit::units::Host for HostState {
    fn start(&mut self, spec: wit::units::Spec) -> wasmtime::Result<Result<(), Error>> {
        let m = &self.plugin.manifest;
        let r = super::units::start(m.units.as_ref(), &m.files, &spec);
        Ok(noted(self.name(), "units.start", r))
    }

    fn state(&mut self, name: String) -> wasmtime::Result<Result<wit::units::UnitState, Error>> {
        let r = super::units::state(self.plugin.manifest.units.as_ref(), &name);
        Ok(noted(self.name(), "units.state", r))
    }

    fn stop(&mut self, name: String) -> wasmtime::Result<Result<(), Error>> {
        let r = super::units::stop(self.plugin.manifest.units.as_ref(), &name);
        Ok(noted(self.name(), "units.stop", r))
    }
}

impl wit::http::HostBody for HostState {
    fn read(
        &mut self,
        body: Resource<BodyStream>,
        max: u32,
    ) -> wasmtime::Result<Result<Option<Vec<u8>>, Error>> {
        let stream = self.table.get_mut(&body)?;
        Ok(stream.0.read(max as usize))
    }

    fn drop(&mut self, body: Resource<BodyStream>) -> wasmtime::Result<()> {
        self.table.delete(body)?;
        Ok(())
    }
}

impl wit::http::Host for HostState {
    fn fetch(
        &mut self,
        request: wit::http::Outgoing,
        max_body: u64,
    ) -> wasmtime::Result<Result<wit::http::Incoming, Error>> {
        let r = super::net::send(&self.plugin.manifest.http, &request, self.deadline).and_then(
            |mut ex| {
                let body = ex.read_all(max_body.min(super::files::MAX_READ))?;
                Ok(wit::http::Incoming {
                    status: ex.status,
                    headers: std::mem::take(&mut ex.headers),
                    body,
                })
            },
        );
        Ok(noted(self.name(), "http.fetch", r))
    }

    fn send(
        &mut self,
        request: wit::http::Outgoing,
    ) -> wasmtime::Result<Result<wit::http::Streamed, Error>> {
        let r = super::net::send(&self.plugin.manifest.http, &request, self.deadline);
        match noted(self.name(), "http.send", r) {
            Ok(mut ex) => {
                let status = ex.status;
                let headers = std::mem::take(&mut ex.headers);
                let body = self.table.push(BodyStream(ex))?;
                Ok(Ok(wit::http::Streamed {
                    status,
                    headers,
                    body,
                }))
            }
            Err(e) => Ok(Err(e)),
        }
    }
}

impl wit::lock::HostGuard for HostState {
    fn drop(&mut self, guard: Resource<LockGuard>) -> wasmtime::Result<()> {
        self.table.delete(guard)?;
        Ok(())
    }
}

impl wit::lock::Host for HostState {
    fn acquire(
        &mut self,
        name: String,
        timeout_ms: Option<u32>,
    ) -> wasmtime::Result<Result<Resource<LockGuard>, Error>> {
        if !self.plugin.manifest.locks.contains(&name) {
            return Ok(noted(
                self.name(),
                "lock.acquire",
                Err(Error::Denied(format!(
                    "{name} is not on the manifest's locks"
                ))),
            ));
        }
        let until = self.until(timeout_ms, Duration::from_secs(10));
        match self.rt.locks.acquire(&name, until) {
            Ok(guard) => Ok(Ok(self.table.push(guard)?)),
            Err(e) => Ok(Err(e)),
        }
    }
}

impl wit::cache::Host for HostState {
    fn get(&mut self, key: String) -> wasmtime::Result<Option<Vec<u8>>> {
        Ok(self.plugin.cache().get(&key))
    }

    fn set(&mut self, key: String, value: Vec<u8>, ttl_secs: u32) -> wasmtime::Result<()> {
        let ttl = (ttl_secs > 0).then(|| Duration::from_secs(u64::from(ttl_secs)));
        self.plugin.cache().set(&key, value, ttl);
        Ok(())
    }

    fn remove(&mut self, key: String) -> wasmtime::Result<()> {
        self.plugin.cache().remove(&key);
        Ok(())
    }
}

impl wit::jobs::Host for HostState {
    fn spawn(&mut self, kind: String, input: Vec<u8>) -> wasmtime::Result<Result<(), Error>> {
        let r = super::spawn_job(&self.rt, &self.plugin, kind, input);
        Ok(noted(self.name(), "jobs.spawn", r))
    }
}

impl wit::dispatch::Host for HostState {
    fn call(
        &mut self,
        request: wit::types::Request,
    ) -> wasmtime::Result<Result<wit::types::Response, Error>> {
        if !self.plugin.manifest.dispatch {
            return Ok(noted(
                self.name(),
                "dispatch.call",
                Err(Error::Denied(
                    "the manifest does not grant dispatch".to_string(),
                )),
            ));
        }
        Ok(super::dispatch(&self.rt, request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_is_held_until_its_guard_drops() {
        let locks = Arc::new(Locks::default());
        let soon = || Instant::now() + Duration::from_millis(100);
        let g = locks.acquire("state", soon()).unwrap();
        assert!(matches!(
            locks.acquire("state", soon()),
            Err(Error::TimedOut)
        ));
        // Another name is another lock.
        let other = locks.acquire("look", soon()).unwrap();
        let l2 = locks.clone();
        let waiter = std::thread::spawn(move || {
            l2.acquire("state", Instant::now() + Duration::from_secs(5))
                .is_ok()
        });
        std::thread::sleep(Duration::from_millis(50));
        drop(g);
        assert!(waiter.join().unwrap());
        drop(other);
    }

    #[test]
    fn the_cache_expires_and_evicts_the_oldest() {
        let mut c = Cache::default();
        c.set("a", vec![1], None);
        assert_eq!(c.get("a"), Some(vec![1]));
        c.set("t", vec![2], Some(Duration::from_millis(1)));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(c.get("t"), None);
        let big = vec![0u8; CACHE_BYTES / 2];
        c.set("b1", big.clone(), None);
        c.set("b2", big.clone(), None);
        assert_eq!(c.get("a"), None, "the oldest goes first");
        assert_eq!(c.get("b1"), None);
        assert!(c.get("b2").is_some());
        assert!(c.bytes <= CACHE_BYTES);
        c.set("huge", vec![0u8; CACHE_BYTES + 1], None);
        assert_eq!(c.get("huge"), None);
        c.remove("b2");
        assert_eq!(c.bytes, 0);
    }
}
