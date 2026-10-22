//! `losos-cloud serve --watch`: changes made to the files on disk by
//! something other than this service, brought into the table as they
//! happen.
//!
//! The box's Windows share (`modules/windows.nix`) writes into
//! `<root>/<owner>/files` directly, as the service's own user. The bytes are
//! then where every door of the cloud reads them, but the rows that give a
//! sync client its file ids and ETags, and the folder ETags above them, only
//! move when [`Store::scan_path`] looks. So every folder of every home is
//! watched with inotify, and a changed name is scanned once things go quiet
//! for a moment: a Windows copy writes a file in many pieces, and one scan
//! at the end is enough.
//!
//! The service's own writes show up here too. Each is a no-op by the time it
//! is scanned, since the store moves a file into place and updates its row
//! under the one lock the scan also takes.
//!
//! Only `files/` is watched: `uploads/` holds chunked uploads in flight and
//! `trash/` and `versions/` are the store's own. A symlink is never followed,
//! here or by the scan.

use super::store::{Store, StoreError};
use super::Cloud;
use rustix::event::{poll, PollFd, PollFlags};
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;
use std::collections::{BTreeSet, HashMap};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Scan once nothing has changed for this long.
const QUIET: Duration = Duration::from_millis(700);
/// And at the latest this long after the first change, so a long copy
/// shows up in pieces rather than only at its end.
const AT_MOST: Duration = Duration::from_secs(5);

/// What a watch descriptor is watching.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Spot {
    /// `<root>`, where homes appear.
    Root,
    /// `<root>/<owner>`, where `files/` appears.
    Home(String),
    /// A folder of a home's files, relative to `files/`.
    Files { owner: String, path: String },
}

/// The changes waiting for their scan: a name, relative to its owner's
/// `files/`, and when the first of them came.
#[derive(Default)]
struct Pending {
    names: BTreeSet<(String, String)>,
    since: Option<Instant>,
    last: Option<Instant>,
}

impl Pending {
    fn add(&mut self, owner: &str, path: String, now: Instant) {
        self.names.insert((owner.to_owned(), path));
        self.since.get_or_insert(now);
        self.last = Some(now);
    }

    /// How long to wait for more events before scanning, or `None` with
    /// nothing pending.
    fn wait(&self, now: Instant) -> Option<Duration> {
        let (since, last) = (self.since?, self.last?);
        let quiet = (last + QUIET).saturating_duration_since(now);
        let cap = (since + AT_MOST).saturating_duration_since(now);
        Some(quiet.min(cap))
    }

    /// The names to scan, without those a pending folder above them covers.
    fn take(&mut self) -> Vec<(String, String)> {
        self.since = None;
        self.last = None;
        let all = std::mem::take(&mut self.names);
        let mut out: Vec<(String, String)> = Vec::new();
        for (owner, path) in all {
            // BTreeSet order puts `a` before `a/b`, so a covering folder is
            // always already in `out`.
            let covered = out
                .iter()
                .any(|(o, p)| *o == owner && (p.is_empty() || path.starts_with(&format!("{p}/"))));
            if !covered {
                out.push((owner, path));
            }
        }
        out
    }
}

fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}/{name}")
    }
}

fn parent(path: &str) -> Option<&str> {
    if path.is_empty() {
        None
    } else {
        Some(path.rsplit_once('/').map_or("", |(p, _)| p))
    }
}

/// Bring `path` into the table, walking up to the nearest folder the table
/// already has when it is new below a new folder. The entries scanned, for
/// [`Cloud::changed`].
fn scan_up(store: &Store, owner: &str, path: &str) -> Vec<String> {
    let mut at = path.to_owned();
    loop {
        match store.scan_path(owner, &at) {
            Ok(0) => return Vec::new(),
            Ok(_) => return vec![at],
            Err(StoreError::Conflict) => match parent(&at) {
                Some(p) => at = p.to_owned(),
                None => return Vec::new(),
            },
            Err(e) => {
                tracing::warn!(owner, path = at, error = %e, "a change on disk could not be scanned");
                return Vec::new();
            }
        }
    }
}

struct Watcher {
    fd: OwnedFd,
    root: PathBuf,
    spots: HashMap<i32, Spot>,
}

const DIR: WatchFlags = WatchFlags::ONLYDIR.union(WatchFlags::DONT_FOLLOW);

impl Watcher {
    fn new(root: &Path) -> std::io::Result<Watcher> {
        let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)?;
        let mut w = Watcher {
            fd,
            root: root.to_owned(),
            spots: HashMap::new(),
        };
        w.watch_all();
        Ok(w)
    }

    fn add(&mut self, dir: &Path, flags: WatchFlags, spot: Spot) -> bool {
        match inotify::add_watch(&self.fd, dir, flags | DIR) {
            Ok(wd) => {
                self.spots.insert(wd, spot);
                true
            }
            // Gone again already, or not a folder: nothing to watch.
            Err(Errno::NOENT | Errno::NOTDIR | Errno::LOOP) => false,
            Err(e) => {
                tracing::warn!(dir = %dir.display(), error = %e, "a folder could not be watched");
                false
            }
        }
    }

    fn watch_all(&mut self) {
        let root = self.root.clone();
        self.add(&root, WatchFlags::CREATE | WatchFlags::MOVED_TO, Spot::Root);
        let Ok(homes) = std::fs::read_dir(&root) else {
            return;
        };
        for home in homes.flatten() {
            if let Some(owner) = home.file_name().to_str() {
                self.watch_home(owner);
            }
        }
    }

    fn watch_home(&mut self, owner: &str) {
        if owner.starts_with('.') {
            return;
        }
        let home = self.root.join(owner);
        if self.add(
            &home,
            WatchFlags::CREATE | WatchFlags::MOVED_TO,
            Spot::Home(owner.to_owned()),
        ) {
            self.watch_tree(owner, "");
        }
    }

    /// A folder of `owner`'s files and every folder below it.
    fn watch_tree(&mut self, owner: &str, path: &str) {
        let mut disk = self.root.join(owner).join("files");
        if !path.is_empty() {
            disk.push(path);
        }
        let flags = WatchFlags::CREATE
            | WatchFlags::CLOSE_WRITE
            | WatchFlags::ATTRIB
            | WatchFlags::DELETE
            | WatchFlags::MOVED_FROM
            | WatchFlags::MOVED_TO;
        let spot = Spot::Files {
            owner: owner.to_owned(),
            path: path.to_owned(),
        };
        if !self.add(&disk, flags, spot) {
            return;
        }
        let Ok(children) = std::fs::read_dir(&disk) else {
            return;
        };
        for child in children.flatten() {
            let is_dir = child.file_type().is_ok_and(|t| t.is_dir());
            if let (true, Some(name)) = (is_dir, child.file_name().to_str()) {
                self.watch_tree(owner, &join(path, name));
            }
        }
    }

    /// Read what has happened so far into `pending`. `true` when the
    /// kernel's queue overflowed and only a full scan can tell.
    fn read(&mut self, pending: &mut Pending) -> bool {
        let mut buf = [MaybeUninit::<u8>::uninit(); 16 * 1024];
        let mut overflowed = false;
        let mut events = Vec::new();
        {
            let mut reader = inotify::Reader::new(&self.fd, &mut buf);
            loop {
                match reader.next() {
                    Ok(e) => events.push((
                        e.wd(),
                        e.events(),
                        e.file_name()
                            .and_then(|n| n.to_str().ok())
                            .map(str::to_owned),
                    )),
                    Err(Errno::AGAIN | Errno::INTR) => break,
                    Err(e) => {
                        tracing::warn!(error = %e, "reading file changes failed");
                        break;
                    }
                }
                if reader.is_buffer_empty() {
                    break;
                }
            }
        }
        let now = Instant::now();
        for (wd, flags, name) in events {
            if flags.contains(ReadFlags::QUEUE_OVERFLOW) {
                overflowed = true;
                continue;
            }
            if flags.contains(ReadFlags::IGNORED) {
                self.spots.remove(&wd);
                continue;
            }
            let (Some(spot), Some(name)) = (self.spots.get(&wd).cloned(), name) else {
                continue;
            };
            let made = flags.intersects(ReadFlags::CREATE | ReadFlags::MOVED_TO);
            let dir = flags.contains(ReadFlags::ISDIR);
            match spot {
                // A dot name is a home still being made; it arrives by a move.
                Spot::Root if made && dir && !name.starts_with('.') => {
                    self.watch_home(&name);
                    pending.add(&name, String::new(), now);
                }
                Spot::Home(owner) if made && dir && name == "files" => {
                    self.watch_tree(&owner, "");
                    pending.add(&owner, String::new(), now);
                }
                Spot::Files { owner, path } => {
                    let child = join(&path, &name);
                    if made && dir {
                        self.watch_tree(&owner, &child);
                    }
                    pending.add(&owner, child, now);
                }
                _ => {}
            }
        }
        overflowed
    }
}

/// Watch every home under the store's root and scan what changes, on a
/// thread of its own, for as long as the process runs.
pub fn spawn(cloud: Arc<Cloud>) -> std::io::Result<()> {
    let mut watcher = Watcher::new(cloud.store.root())?;
    tracing::info!(folders = watcher.spots.len(), "watching the files on disk");
    std::thread::Builder::new()
        .name("watch".into())
        .spawn(move || run(&cloud, &mut watcher))?;
    Ok(())
}

fn run(cloud: &Cloud, watcher: &mut Watcher) {
    let mut pending = Pending::default();
    loop {
        let wait = pending.wait(Instant::now());
        let timeout = wait.map(|d| rustix::event::Timespec {
            tv_sec: d.as_secs() as i64,
            tv_nsec: d.subsec_nanos() as i64,
        });
        let mut fds = [PollFd::new(&watcher.fd, PollFlags::IN)];
        match poll(&mut fds, timeout.as_ref()) {
            Ok(0) => {}
            Ok(_) => {
                if watcher.read(&mut pending) {
                    tracing::warn!("too many changes at once; scanning every home");
                    watcher.spots.clear();
                    watcher.watch_all();
                    if let Ok(owners) = cloud.store.owners() {
                        for owner in owners {
                            pending.add(&owner, String::new(), Instant::now());
                        }
                    }
                }
                if !pending.wait(Instant::now()).is_some_and(|d| d.is_zero()) {
                    continue;
                }
            }
            Err(Errno::INTR) => continue,
            Err(e) => {
                tracing::error!(error = %e, "waiting for file changes failed; no longer watching");
                return;
            }
        }
        for (owner, path) in pending.take() {
            for scanned in scan_up(&cloud.store, &owner, &path) {
                if let Ok(entry) = cloud.store.stat(&owner, &scanned) {
                    if !entry.dir {
                        cloud.changed(&owner, &entry);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::Config;
    use std::fs;

    fn cloud() -> (assert_fs::TempDir, Arc<Cloud>) {
        let dir = assert_fs::TempDir::new().unwrap();
        let cloud = Arc::new(Cloud::open(dir.path(), Config::default()).unwrap());
        cloud.store.ensure_home("matus").unwrap();
        (dir, cloud)
    }

    /// Wait until `check` holds, for at most ten seconds.
    fn eventually(mut check: impl FnMut() -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            if check() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn a_folder_covers_the_names_below_it() {
        let mut p = Pending::default();
        let now = Instant::now();
        p.add("a", "x/y".into(), now);
        p.add("a", "x".into(), now);
        p.add("a", "xy".into(), now);
        p.add("b", "x/y".into(), now);
        assert_eq!(
            p.take(),
            vec![
                ("a".into(), "x".into()),
                ("a".into(), "xy".into()),
                ("b".into(), "x/y".into())
            ]
        );
        assert!(p.wait(now).is_none());
    }

    #[test]
    fn waiting_ends_when_quiet_or_after_the_cap() {
        let mut p = Pending::default();
        let t0 = Instant::now();
        p.add("a", "f".into(), t0);
        assert_eq!(p.wait(t0), Some(QUIET));
        // Events every half second keep pushing the quiet point out, but
        // never past the cap.
        let late = t0 + AT_MOST - Duration::from_millis(100);
        p.add("a", "f".into(), late);
        assert_eq!(p.wait(late), Some(Duration::from_millis(100)));
    }

    #[test]
    fn files_written_behind_the_stores_back_get_rows_and_etags() {
        let (dir, cloud) = cloud();
        let root_etag = cloud.store.stat("matus", "").unwrap().etag;
        spawn(cloud.clone()).unwrap();
        let files = dir.path().join("matus/files");

        // A new folder with a file in it, the way a Windows copy makes them.
        fs::create_dir(files.join("Trip")).unwrap();
        fs::create_dir(files.join("Trip/Day 1")).unwrap();
        fs::write(files.join("Trip/Day 1/notes.txt"), b"hello").unwrap();
        assert!(eventually(|| cloud
            .store
            .stat("matus", "Trip/Day 1/notes.txt")
            .is_ok_and(|e| e.size == 5)));
        assert_ne!(cloud.store.stat("matus", "").unwrap().etag, root_etag);

        // Rewritten in place: a new ETag, and the listeners hear of it.
        let mut changes = cloud.subscribe();
        let before = cloud.store.stat("matus", "Trip/Day 1/notes.txt").unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(files.join("Trip/Day 1/notes.txt"), b"hello, again").unwrap();
        assert!(eventually(|| cloud
            .store
            .stat("matus", "Trip/Day 1/notes.txt")
            .is_ok_and(|e| e.size == 12 && e.etag != before.etag)));
        let heard = changes.try_recv().unwrap();
        assert_eq!((heard.owner.as_str(), heard.file_id), ("matus", before.id));

        // Renamed keeps nothing behind; deleted is gone from the table.
        fs::rename(files.join("Trip/Day 1"), files.join("Trip/First day")).unwrap();
        assert!(eventually(|| cloud
            .store
            .stat("matus", "Trip/First day/notes.txt")
            .is_ok()
            && cloud.store.stat("matus", "Trip/Day 1").is_err()));
        fs::remove_file(files.join("Trip/First day/notes.txt")).unwrap();
        assert!(eventually(|| cloud
            .store
            .stat("matus", "Trip/First day/notes.txt")
            .is_err()));
    }

    #[test]
    fn a_new_home_is_watched_too() {
        let (dir, cloud) = cloud();
        spawn(cloud.clone()).unwrap();
        fs::create_dir_all(dir.path().join("eva/files/Docs")).unwrap();
        // The folders appear in one go; give the watch on `eva/` a moment
        // before writing below it.
        std::thread::sleep(Duration::from_millis(200));
        fs::write(dir.path().join("eva/files/Docs/a.txt"), b"a").unwrap();
        assert!(eventually(|| cloud.store.stat("eva", "Docs/a.txt").is_ok()));
    }
}
