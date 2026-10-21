//! The host side of `plugins/wit/plugin.wit`, generated. The only module
//! of the crate allowed `unsafe`: bindgen!'s typed export calls use it.

#![allow(unsafe_code)]

wasmtime::component::bindgen!({
    world: "plugin",
    path: "plugins/wit",
    imports: { default: trappable },
    with: {
        "losos:plugin/http.body": crate::plugin::host::BodyStream,
        "losos:plugin/lock.guard": crate::plugin::host::LockGuard,
    },
});
