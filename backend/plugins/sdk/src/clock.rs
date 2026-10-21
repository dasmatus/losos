//! Time. `std::time::SystemTime::now` panics on wasm32; use these.

use crate::wit::clock;

/// Seconds since the Unix epoch.
#[must_use]
pub fn now() -> u64 {
    clock::now()
}

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now_ms() -> u64 {
    clock::now_ms()
}

/// Milliseconds on a clock that never goes back.
#[must_use]
pub fn monotonic_ms() -> u64 {
    clock::monotonic_ms()
}

/// Block for `ms` milliseconds, within the call's deadline.
pub fn sleep_ms(ms: u32) {
    clock::sleep_ms(ms);
}
