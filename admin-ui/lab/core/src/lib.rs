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
// For a renderer linked into the same module (admin-ui/lab/render): the
// world and the evaluation it draws from, read in place, never serialised.
pub use net::{Addr, BoxSt, Net, SpokeSt};
pub use omap::OMap;
pub use world::{Device, End, Link, World};

use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

fn out(v: Value) -> String {
    v.to_string()
}

/// The Lab: world, evaluation, simulator, clock, consoles and pages.
///
/// The state sits behind `Rc<RefCell<..>>` so that a renderer compiled into
/// the same module can hold the very instance the page drives (`shared`).
/// Everything here runs on the page's one thread and no borrow outlives a
/// call, so the cell never sees two borrows at once.
#[wasm_bindgen]
pub struct Lab {
    core: Rc<RefCell<LabCore>>,
}

impl Lab {
    /// The state this `Lab` drives, for a renderer in the same module. Not
    /// exported to JavaScript.
    pub fn shared(&self) -> Rc<RefCell<LabCore>> {
        Rc::clone(&self.core)
    }
}

#[wasm_bindgen]
impl Lab {
    /// `seed` feeds the PRNG (MAC suffixes, nonces, ping times); `now_ms` is
    /// `Date.now()`; `tz_offset_min` is `-new Date().getTimezoneOffset()`.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: f64, now_ms: f64, tz_offset_min: f64) -> Lab {
        Lab {
            core: Rc::new(RefCell::new(LabCore::new(seed, now_ms, tz_offset_min))),
        }
    }

    pub fn catalog(&self) -> String {
        out(self.core.borrow().catalog())
    }
    pub fn set_plate(&mut self, url: &str) {
        self.core.borrow_mut().set_plate(url)
    }

    pub fn load_scenario(&mut self, key: &str) -> String {
        out(self.core.borrow_mut().load_scenario(key))
    }
    pub fn load_this_box(&mut self, settings: &str, edge: &str) -> String {
        out(self.core.borrow_mut().load_this_box(settings, edge))
    }
    pub fn export_setup(&self) -> String {
        out(self.core.borrow().export_setup())
    }
    pub fn import_setup(&mut self, text: &str, file_name: &str) -> String {
        out(self.core.borrow_mut().import_setup(text, file_name))
    }
    pub fn open_last(&mut self, text: &str) -> String {
        out(self.core.borrow_mut().open_last(text))
    }
    pub fn last_setup(&mut self) -> Option<String> {
        self.core.borrow_mut().last_setup()
    }
    pub fn peek_setup(&self, text: &str) -> String {
        out(self.core.borrow().peek_setup(text))
    }

    pub fn add_device(&mut self, kind: &str, x: f64, y: f64, opts: &str) -> String {
        out(self.core.borrow_mut().add_device(kind, x, y, opts))
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
            .borrow_mut()
            .connect(a, b, kind, a_port.as_deref(), b_port.as_deref()))
    }
    pub fn remove_device(&mut self, id: &str) -> String {
        out(self.core.borrow_mut().remove_device(id))
    }
    pub fn remove_link(&mut self, id: &str) -> String {
        out(self.core.borrow_mut().remove_link(id))
    }
    pub fn set_power(&mut self, id: &str, on: bool) -> String {
        out(self.core.borrow_mut().set_power(id, on))
    }
    pub fn set_cfg(&mut self, id: &str, key: &str, value: &str) -> String {
        out(self.core.borrow_mut().set_cfg(id, key, value))
    }
    pub fn set_name(&mut self, id: &str, name: &str) -> String {
        out(self.core.borrow_mut().set_name(id, name))
    }
    pub fn set_site(&mut self, id: &str, site: &str) -> String {
        out(self.core.borrow_mut().set_site(id, site))
    }
    pub fn move_device(&mut self, id: &str, pos: &str) -> String {
        out(self.core.borrow_mut().move_device(id, pos))
    }
    pub fn phys_pos(&mut self, id: &str) -> String {
        out(self.core.borrow_mut().phys_pos(id))
    }

    pub fn snapshot(&self) -> String {
        out(self.core.borrow().snapshot())
    }
    pub fn suggest_urls(&self, id: &str) -> String {
        out(self.core.borrow().suggest_urls(id))
    }

    pub fn tick(&mut self, elapsed_ms: f64) -> String {
        out(self.core.borrow_mut().tick(elapsed_ms))
    }
    pub fn step(&mut self) -> String {
        out(self.core.borrow_mut().step())
    }
    pub fn set_mode(&mut self, mode: &str) -> String {
        out(self.core.borrow_mut().set_mode(mode))
    }
    pub fn set_playing(&mut self, on: bool) {
        self.core.borrow_mut().set_playing(on)
    }
    pub fn set_sim_speed(&mut self, speed: f64) {
        self.core.borrow_mut().set_sim_speed(speed)
    }
    pub fn set_clock_speed(&mut self, speed: f64) -> String {
        out(self.core.borrow_mut().set_clock_speed(speed))
    }
    pub fn skip_to_next_timer(&mut self) -> String {
        out(self.core.borrow_mut().skip_to_next_timer())
    }
    pub fn set_filter(&mut self, proto: &str, on: bool) -> String {
        out(self.core.borrow_mut().set_filter(proto, on))
    }
    pub fn clear(&mut self) {
        self.core.borrow_mut().clear()
    }
    pub fn scan(&mut self, id: &str) -> String {
        out(self.core.borrow_mut().scan(id))
    }
    pub fn start_flow(&mut self, stages: &str, title: &str) -> String {
        out(self.core.borrow_mut().start_flow_json(stages, title))
    }

    pub fn console_info(&self, id: &str) -> String {
        out(self.core.borrow().console_info(id))
    }
    pub fn exec(&mut self, id: &str, line: &str) -> String {
        out(self.core.borrow_mut().exec(id, line))
    }
    pub fn http(&mut self, id: &str, url: &str) -> String {
        out(self.core.borrow_mut().http(id, url))
    }
    pub fn error_page(&self, error: &str, host: &str) -> String {
        self.core.borrow().error_page(error, host)
    }
}
