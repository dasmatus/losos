//! Named locks shared by every plugin whose manifest lists them.

pub use crate::wit::lock::Guard;

/// Wait up to 10 s for `name`; held until the guard drops.
pub fn acquire(name: &str) -> crate::Result<Guard> {
    crate::wit::lock::acquire(name, None).map_err(Into::into)
}

/// Run `f` holding `name`.
pub fn with<T>(name: &str, f: impl FnOnce() -> T) -> crate::Result<T> {
    let guard = acquire(name)?;
    let out = f();
    drop(guard);
    Ok(out)
}
