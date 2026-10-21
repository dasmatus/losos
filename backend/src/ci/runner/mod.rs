//! The runner: `losos-ci runner`, as the `shared` user. It asks the
//! daemon for work over the runner socket, unpacks the commit into a job
//! directory, runs the steps ([`job`]) in a rootless Podman pod
//! ([`sandbox`]) and reports the log and the result as it goes.

pub mod commands;
pub mod job;
pub mod sandbox;
