//! LosOS Lab's model and simulator for the browser. `LabCore` is the whole
//! state as plain Rust (what the tests drive); `Lab` is its wasm-bindgen face,
//! strings in and JSON text out. README.md is the contract.

#![forbid(unsafe_code)]

mod catalog;
mod console;
mod files;
mod js;
mod lab;
mod net;
mod omap;
mod pages;
mod rng;
mod scenarios;
mod sim;
mod url;
mod world;

pub use lab::LabCore;

use serde_json::Value;
use wasm_bindgen::prelude::*;

fn out(v: Value) -> String {
    v.to_string()
}

/// The Lab: world, evaluation, simulator, clock, consoles and pages.
#[wasm_bindgen]
pub struct Lab {
    core: LabCore,
}

#[wasm_bindgen]
impl Lab {
    /// `seed` feeds the PRNG (MAC suffixes, nonces, ping times); `now_ms` is
    /// `Date.now()`; `tz_offset_min` is `-new Date().getTimezoneOffset()`.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: f64, now_ms: f64, tz_offset_min: f64) -> Lab {
        Lab {
            core: LabCore::new(seed, now_ms, tz_offset_min),
        }
    }

    pub fn catalog(&self) -> String {
        out(self.core.catalog())
    }
    pub fn set_plate(&mut self, url: &str) {
        self.core.set_plate(url)
    }

    pub fn load_scenario(&mut self, key: &str) -> String {
        out(self.core.load_scenario(key))
    }
    pub fn load_this_box(&mut self, settings: &str, edge: &str) -> String {
        out(self.core.load_this_box(settings, edge))
    }
    pub fn export_setup(&self) -> String {
        out(self.core.export_setup())
    }
    pub fn import_setup(&mut self, text: &str, file_name: &str) -> String {
        out(self.core.import_setup(text, file_name))
    }
    pub fn open_last(&mut self, text: &str) -> String {
        out(self.core.open_last(text))
    }
    pub fn last_setup(&mut self) -> Option<String> {
        self.core.last_setup()
    }
    pub fn peek_setup(&self, text: &str) -> String {
        out(self.core.peek_setup(text))
    }

    pub fn add_device(&mut self, kind: &str, x: f64, y: f64, opts: &str) -> String {
        out(self.core.add_device(kind, x, y, opts))
    }
    pub fn connect(
        &mut self,
        a: &str,
        b: &str,
        kind: &str,
        a_port: Option<String>,
        b_port: Option<String>,
    ) -> String {
        out(self
            .core
            .connect(a, b, kind, a_port.as_deref(), b_port.as_deref()))
    }
    pub fn remove_device(&mut self, id: &str) -> String {
        out(self.core.remove_device(id))
    }
    pub fn remove_link(&mut self, id: &str) -> String {
        out(self.core.remove_link(id))
    }
    pub fn set_power(&mut self, id: &str, on: bool) -> String {
        out(self.core.set_power(id, on))
    }
    pub fn set_cfg(&mut self, id: &str, key: &str, value: &str) -> String {
        out(self.core.set_cfg(id, key, value))
    }
    pub fn set_name(&mut self, id: &str, name: &str) -> String {
        out(self.core.set_name(id, name))
    }
    pub fn set_site(&mut self, id: &str, site: &str) -> String {
        out(self.core.set_site(id, site))
    }
    pub fn move_device(&mut self, id: &str, pos: &str) -> String {
        out(self.core.move_device(id, pos))
    }
    pub fn phys_pos(&mut self, id: &str) -> String {
        out(self.core.phys_pos(id))
    }

    pub fn snapshot(&self) -> String {
        out(self.core.snapshot())
    }
    pub fn suggest_urls(&self, id: &str) -> String {
        out(self.core.suggest_urls(id))
    }

    pub fn tick(&mut self, elapsed_ms: f64) -> String {
        out(self.core.tick(elapsed_ms))
    }
    pub fn step(&mut self) -> String {
        out(self.core.step())
    }
    pub fn set_mode(&mut self, mode: &str) -> String {
        out(self.core.set_mode(mode))
    }
    pub fn set_playing(&mut self, on: bool) {
        self.core.set_playing(on)
    }
    pub fn set_sim_speed(&mut self, speed: f64) {
        self.core.set_sim_speed(speed)
    }
    pub fn set_clock_speed(&mut self, speed: f64) -> String {
        out(self.core.set_clock_speed(speed))
    }
    pub fn skip_to_next_timer(&mut self) -> String {
        out(self.core.skip_to_next_timer())
    }
    pub fn set_filter(&mut self, proto: &str, on: bool) -> String {
        out(self.core.set_filter(proto, on))
    }
    pub fn clear(&mut self) {
        self.core.clear()
    }
    pub fn scan(&mut self, id: &str) -> String {
        out(self.core.scan(id))
    }
    pub fn start_flow(&mut self, stages: &str, title: &str) -> String {
        out(self.core.start_flow_json(stages, title))
    }

    pub fn console_info(&self, id: &str) -> String {
        out(self.core.console_info(id))
    }
    pub fn exec(&mut self, id: &str, line: &str) -> String {
        out(self.core.exec(id, line))
    }
    pub fn http(&mut self, id: &str, url: &str) -> String {
        out(self.core.http(id, url))
    }
    pub fn error_page(&self, error: &str, host: &str) -> String {
        self.core.error_page(error, host)
    }
}
