//! The browser side: [`Connection`] over a WebSocket (to a byte relay in
//! front of the daemon's socket) or a Direct Sockets `TCPSocket` (only in an
//! Isolated Web App, straight to a daemon listening on TCP).
//!
//! ```js
//! import init, { VirtClient } from './pkg/losos_lab_virt.js';
//! await init();
//! // The ticket from the helper's POST virt-ticket (on a box,
//! // POST /api/lab/virt-ticket), offered as a WebSocket subprotocol.
//! const virt = await VirtClient.connect('wss://box.local/api/lab/virt',
//!                                       ['losos-lab', `ticket.${ticket}`]);
//! await virt.open('qemu:///system');
//! console.log(await virt.version());           // { hypervisor: '11.1.0', library: '12.7.0', … }
//! await virt.createDomain(xml, VirtClient.PAUSED | VirtClient.AUTODESTROY);
//! const con = await virt.console('pc1');        // con.readable: ReadableStream<Uint8Array>
//! await virt.resume('pc1');
//! for await (const chunk of con.readable) term.write(chunk);
//! con.write('uname -a\r');
//! await virt.destroy('pc1');
//! ```

use crate::client::{
    version_string, Connection, Event, Reply, CONSOLE_FORCE, START_AUTODESTROY, START_PAUSED,
};
use crate::proto::{AuthType, Domain};
use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, spawn_local, JsFuture};
use web_sys::{
    BinaryType, MessageEvent, ReadableStream, ReadableStreamDefaultController,
    ReadableStreamDefaultReader, WebSocket, WritableStream, WritableStreamDefaultWriter,
};

/// Where the bytes go.
enum Sink {
    Ws(WebSocket),
    Tcp(WritableStreamDefaultWriter),
}

#[derive(Default)]
struct Slot {
    result: Option<Result<Reply, String>>,
    waker: Option<Waker>,
}

/// Resolves when the daemon answers one call.
struct ReplyFuture(Rc<RefCell<Slot>>);

impl Future for ReplyFuture {
    type Output = Result<Reply, String>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut s = self.0.borrow_mut();
        match s.result.take() {
            Some(r) => Poll::Ready(r),
            None => {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct Inner {
    conn: Connection,
    sink: Option<Sink>,
    waiters: HashMap<u32, Rc<RefCell<Slot>>>,
    consoles: HashMap<u32, ReadableStreamDefaultController>,
    /// Why the connection is unusable, once it is.
    dead: Option<String>,
    /// The WebSocket's handlers, alive as long as the client.
    handlers: Vec<Closure<dyn FnMut(JsValue)>>,
}

type Shared = Rc<RefCell<Inner>>;

fn js_err(msg: &str) -> JsValue {
    js_sys::Error::new(msg).into()
}

fn complete(slot: &Rc<RefCell<Slot>>, r: Result<Reply, String>) {
    let waker = {
        let mut s = slot.borrow_mut();
        s.result = Some(r);
        s.waker.take()
    };
    if let Some(w) = waker {
        w.wake();
    }
}

/// Writes whatever the connection queued.
fn flush(sh: &Shared) {
    let (out, sink) = {
        let mut i = sh.borrow_mut();
        let out = i.conn.take_output();
        if out.is_empty() {
            return;
        }
        let sink = match &i.sink {
            Some(Sink::Ws(ws)) => Sink::Ws(ws.clone()),
            Some(Sink::Tcp(w)) => Sink::Tcp(w.clone()),
            None => return,
        };
        (out, sink)
    };
    let failed = match sink {
        Sink::Ws(ws) => ws.send_with_u8_array(&out).err(),
        Sink::Tcp(w) => {
            // The writer queues in order; a failed write also fails the reader.
            let _ = w.write_with_chunk(&Uint8Array::from(out.as_slice()));
            None
        }
    };
    if let Some(e) = failed {
        fail(sh, &format!("send failed: {e:?}"));
    }
}

/// The transport is gone or the stream is not libvirt: everything waiting
/// fails, every console errors.
fn fail(sh: &Shared, why: &str) {
    let (waiters, consoles) = {
        let mut i = sh.borrow_mut();
        if i.dead.is_none() {
            i.dead = Some(why.to_owned());
        }
        (
            std::mem::take(&mut i.waiters),
            std::mem::take(&mut i.consoles),
        )
    };
    for (_, w) in waiters {
        complete(&w, Err(why.to_owned()));
    }
    for (_, c) in consoles {
        c.error_with_e(&js_err(why));
    }
}

/// Bytes from the transport.
fn feed(sh: &Shared, bytes: &[u8]) {
    let events = match sh.borrow_mut().conn.feed(bytes) {
        Ok(ev) => ev,
        Err(e) => return fail(sh, &format!("libvirt protocol: {e}")),
    };
    flush(sh); // a keepalive pong
    for ev in events {
        match ev {
            Event::Reply { serial, result } => {
                let w = sh.borrow_mut().waiters.remove(&serial);
                if let Some(w) = w {
                    complete(&w, result.map_err(|e| e.to_string()));
                }
            }
            Event::StreamData { stream, data } => {
                let c = sh.borrow().consoles.get(&stream).cloned();
                if let Some(c) = c {
                    let _ = c.enqueue_with_chunk(&Uint8Array::from(data.as_slice()));
                }
            }
            Event::StreamEnd { stream } => {
                let c = sh.borrow_mut().consoles.remove(&stream);
                if let Some(c) = c {
                    let _ = c.close();
                }
            }
            Event::StreamError { stream, error } => {
                let c = sh.borrow_mut().consoles.remove(&stream);
                if let Some(c) = c {
                    c.error_with_e(&js_err(&error.to_string()));
                }
            }
        }
    }
}

/// Queues one call and waits for its reply.
async fn call(sh: &Shared, f: impl FnOnce(&mut Connection) -> u32) -> Result<Reply, JsValue> {
    let slot = Rc::new(RefCell::new(Slot::default()));
    {
        let mut i = sh.borrow_mut();
        if let Some(why) = &i.dead {
            return Err(js_err(why));
        }
        let serial = f(&mut i.conn);
        i.waiters.insert(serial, slot.clone());
    }
    flush(sh);
    ReplyFuture(slot).await.map_err(|e| js_err(&e))
}

async fn lookup(sh: &Shared, name: &str) -> Result<Domain, JsValue> {
    match call(sh, |c| c.domain_lookup_by_name(name)).await? {
        Reply::Domain(d) => Ok(d),
        r => Err(js_err(&format!("unexpected reply {r:?}"))),
    }
}

fn domain_js(d: &Domain) -> JsValue {
    let o = Object::new();
    let _ = Reflect::set(&o, &"name".into(), &d.name.as_str().into());
    let _ = Reflect::set(&o, &"uuid".into(), &d.uuid_string().into());
    let _ = Reflect::set(&o, &"id".into(), &d.id.into());
    o.into()
}

/// A libvirt connection from the page.
#[wasm_bindgen]
pub struct VirtClient {
    inner: Shared,
}

#[wasm_bindgen]
impl VirtClient {
    /// `VIR_DOMAIN_START_PAUSED`, for `createDomain`.
    #[wasm_bindgen(getter = PAUSED)]
    pub fn paused() -> u32 {
        START_PAUSED
    }

    /// `VIR_DOMAIN_START_AUTODESTROY`, for `createDomain` (its default).
    #[wasm_bindgen(getter = AUTODESTROY)]
    pub fn autodestroy() -> u32 {
        START_AUTODESTROY
    }

    /// Whether this page may use Direct Sockets (`TCPSocket`): true only in
    /// an Isolated Web App whose permissions policy allows `direct-sockets`.
    #[wasm_bindgen(js_name = directSocketsAvailable)]
    pub fn direct_sockets_available() -> bool {
        Reflect::get(&js_sys::global(), &"TCPSocket".into())
            .map(|v| v.is_function())
            .unwrap_or(false)
    }

    /// `ws://…` or `wss://…`: a WebSocket to a byte relay in front of the
    /// daemon's socket, offering `protocols` as its subprotocols
    /// (`losos-registrar lab` wants `['losos-lab', 'ticket.<hex>']`).
    /// `tcp://host:port`: a Direct Sockets TCPSocket straight to a daemon
    /// listening on TCP (Isolated Web Apps only).
    pub async fn connect(
        target: String,
        protocols: Option<Vec<String>>,
    ) -> Result<VirtClient, JsValue> {
        let inner = Rc::new(RefCell::new(Inner {
            conn: Connection::new(),
            sink: None,
            waiters: HashMap::new(),
            consoles: HashMap::new(),
            dead: None,
            handlers: Vec::new(),
        }));
        if target.starts_with("ws://") || target.starts_with("wss://") {
            connect_ws(&inner, &target, protocols.unwrap_or_default()).await?;
        } else if let Some(hp) = target.strip_prefix("tcp://") {
            connect_tcp(&inner, hp).await?;
        } else {
            return Err(js_err("target must be ws://, wss:// or tcp://host:port"));
        }
        Ok(VirtClient { inner })
    }

    /// Auth (none, or polkit for the relay's own uid) and CONNECT_OPEN.
    pub fn open(&self, uri: String) -> Promise {
        let sh = self.inner.clone();
        future_to_promise(async move {
            let types = match call(&sh, |c| c.auth_list()).await? {
                Reply::AuthList(t) => t,
                r => return Err(js_err(&format!("unexpected reply {r:?}"))),
            };
            if types.contains(&AuthType::Polkit) {
                call(&sh, |c| c.auth_polkit()).await?;
            } else if !(types.is_empty() || types.contains(&AuthType::None)) {
                return Err(js_err(&format!(
                    "the daemon wants {types:?}; this client speaks none and polkit only"
                )));
            }
            call(&sh, |c| c.connect_open(Some(&uri), 0)).await?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// `{ hypervisor, library, hypervisorNumber, libraryNumber }`.
    pub fn version(&self) -> Promise {
        let sh = self.inner.clone();
        future_to_promise(async move {
            let hv = match call(&sh, |c| c.get_version()).await? {
                Reply::Version(v) => v,
                r => return Err(js_err(&format!("unexpected reply {r:?}"))),
            };
            let lib = match call(&sh, |c| c.get_lib_version()).await? {
                Reply::LibVersion(v) => v,
                r => return Err(js_err(&format!("unexpected reply {r:?}"))),
            };
            let o = Object::new();
            let _ = Reflect::set(&o, &"hypervisor".into(), &version_string(hv).into());
            let _ = Reflect::set(&o, &"library".into(), &version_string(lib).into());
            let _ = Reflect::set(&o, &"hypervisorNumber".into(), &(hv as f64).into());
            let _ = Reflect::set(&o, &"libraryNumber".into(), &(lib as f64).into());
            Ok(o.into())
        })
    }

    /// Starts a transient domain; resolves to `{ name, uuid, id }`.
    /// `flags` defaults to AUTODESTROY: the guest dies with the connection.
    #[wasm_bindgen(js_name = createDomain)]
    pub fn create_domain(&self, xml: String, flags: Option<u32>) -> Promise {
        let sh = self.inner.clone();
        let flags = flags.unwrap_or(START_AUTODESTROY);
        future_to_promise(async move {
            match call(&sh, |c| c.domain_create_xml(&xml, flags)).await? {
                Reply::Domain(d) => Ok(domain_js(&d)),
                r => Err(js_err(&format!("unexpected reply {r:?}"))),
            }
        })
    }

    /// `{ name, uuid, id }` of a domain, or rejects (VIR_ERR_NO_DOMAIN).
    pub fn lookup(&self, name: String) -> Promise {
        let sh = self.inner.clone();
        future_to_promise(async move { Ok(domain_js(&lookup(&sh, &name).await?)) })
    }

    pub fn resume(&self, name: String) -> Promise {
        let sh = self.inner.clone();
        future_to_promise(async move {
            let d = lookup(&sh, &name).await?;
            call(&sh, |c| c.domain_resume(&d)).await?;
            Ok(JsValue::UNDEFINED)
        })
    }

    pub fn destroy(&self, name: String) -> Promise {
        let sh = self.inner.clone();
        future_to_promise(async move {
            let d = lookup(&sh, &name).await?;
            call(&sh, |c| c.domain_destroy(&d)).await?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// The domain's first console as a [`VirtConsole`]. `force` takes it
    /// from another client (virt-manager, `virsh console`) that holds it.
    pub fn console(&self, name: String, force: Option<bool>) -> Promise {
        let sh = self.inner.clone();
        let flags = if force.unwrap_or(false) {
            CONSOLE_FORCE
        } else {
            0
        };
        future_to_promise(async move {
            let d = lookup(&sh, &name).await?;
            let slot = Rc::new(RefCell::new(Slot::default()));
            let stream = {
                let mut i = sh.borrow_mut();
                if let Some(why) = &i.dead {
                    return Err(js_err(why));
                }
                let s = i.conn.domain_open_console(&d, None, flags);
                i.waiters.insert(s, slot.clone());
                s
            };
            // The stream exists before the reply does: data the daemon sends
            // right after it lands in the ReadableStream, not on the floor.
            let readable = console_stream(&sh, stream)?;
            flush(&sh);
            ReplyFuture(slot).await.map_err(|e| js_err(&e))?;
            Ok(VirtConsole {
                inner: sh,
                stream,
                readable,
            }
            .into())
        })
    }

    /// CONNECT_CLOSE, then the transport. AUTODESTROY guests die here.
    pub fn close(&self) -> Promise {
        let sh = self.inner.clone();
        future_to_promise(async move {
            let r = call(&sh, |c| c.connect_close()).await;
            let sink = sh.borrow_mut().sink.take();
            match sink {
                Some(Sink::Ws(ws)) => {
                    let _ = ws.close();
                }
                Some(Sink::Tcp(w)) => {
                    let _ = w.close();
                }
                None => {}
            }
            fail(&sh, "connection closed");
            r.map(|_| JsValue::UNDEFINED)
        })
    }
}

fn console_stream(sh: &Shared, stream: u32) -> Result<ReadableStream, JsValue> {
    let start_sh = sh.clone();
    let start = Closure::once_into_js(move |controller: ReadableStreamDefaultController| {
        start_sh.borrow_mut().consoles.insert(stream, controller);
    });
    let cancel_sh = sh.clone();
    let cancel = Closure::once_into_js(move |_reason: JsValue| {
        let sent = {
            let mut i = cancel_sh.borrow_mut();
            i.consoles.remove(&stream);
            i.conn.stream_abort(stream)
        };
        if sent {
            flush(&cancel_sh);
        }
    });
    let source = Object::new();
    Reflect::set(&source, &"start".into(), &start)?;
    Reflect::set(&source, &"cancel".into(), &cancel)?;
    ReadableStream::new_with_underlying_source(&source)
}

/// A domain's console: `readable` carries what the guest prints, `write`
/// types into it.
#[wasm_bindgen]
pub struct VirtConsole {
    inner: Shared,
    stream: u32,
    readable: ReadableStream,
}

#[wasm_bindgen]
impl VirtConsole {
    #[wasm_bindgen(getter)]
    pub fn readable(&self) -> ReadableStream {
        self.readable.clone()
    }

    /// A string (sent as UTF-8) or a Uint8Array.
    pub fn write(&self, data: JsValue) -> Result<(), JsValue> {
        let bytes = if let Some(s) = data.as_string() {
            s.into_bytes()
        } else if let Some(a) = data.dyn_ref::<Uint8Array>() {
            a.to_vec()
        } else {
            return Err(js_err("write takes a string or a Uint8Array"));
        };
        if !self
            .inner
            .borrow_mut()
            .conn
            .stream_send(self.stream, &bytes)
        {
            return Err(js_err("console is closed"));
        }
        flush(&self.inner);
        Ok(())
    }

    /// Ends the stream cleanly; the guest keeps running.
    pub fn close(&self) {
        if self.inner.borrow_mut().conn.stream_finish(self.stream) {
            flush(&self.inner);
        }
    }
}

async fn connect_ws(sh: &Shared, url: &str, protocols: Vec<String>) -> Result<(), JsValue> {
    let ws = if protocols.is_empty() {
        WebSocket::new(url)?
    } else {
        let list: Array = protocols.iter().map(|p| JsValue::from_str(p)).collect();
        WebSocket::new_with_str_sequence(url, &list)?
    };
    ws.set_binary_type(BinaryType::Arraybuffer);
    let opened = Promise::new(&mut |resolve: Function, reject: Function| {
        let on_open = Closure::once_into_js(move |_: JsValue| {
            let _ = resolve.call0(&JsValue::NULL);
        });
        let on_error = Closure::once_into_js(move |_: JsValue| {
            let _ = reject.call1(&JsValue::NULL, &js_err("WebSocket did not open"));
        });
        ws.set_onopen(Some(on_open.unchecked_ref()));
        ws.set_onerror(Some(on_error.unchecked_ref()));
    });
    JsFuture::from(opened).await?;
    ws.set_onopen(None);

    let msg_sh = sh.clone();
    let on_message = Closure::<dyn FnMut(JsValue)>::new(move |e: JsValue| {
        if let Some(e) = e.dyn_ref::<MessageEvent>() {
            let data = e.data();
            if data.is_instance_of::<js_sys::ArrayBuffer>() {
                feed(&msg_sh, &Uint8Array::new(&data).to_vec());
            }
        }
    });
    let close_sh = sh.clone();
    let on_close = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        fail(&close_sh, "the relay closed the WebSocket");
    });
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    ws.set_onerror(None);
    let mut i = sh.borrow_mut();
    i.handlers.push(on_message);
    i.handlers.push(on_close);
    i.sink = Some(Sink::Ws(ws));
    Ok(())
}

async fn connect_tcp(sh: &Shared, host_port: &str) -> Result<(), JsValue> {
    let (host, port) = host_port
        .rsplit_once(':')
        .and_then(|(h, p)| {
            p.parse::<u16>()
                .ok()
                .map(|p| (h.trim_matches(['[', ']']), p))
        })
        .ok_or_else(|| js_err("tcp:// wants host:port"))?;
    let ctor = Reflect::get(&js_sys::global(), &"TCPSocket".into())?;
    let Some(ctor) = ctor.dyn_ref::<Function>() else {
        return Err(js_err(
            "Direct Sockets (TCPSocket) is not available: it exists only in Isolated Web Apps",
        ));
    };
    let args = Array::of2(&host.into(), &port.into());
    let socket = Reflect::construct(ctor, &args)?;
    let opened: Promise = Reflect::get(&socket, &"opened".into())?.dyn_into()?;
    let info = JsFuture::from(opened).await?;
    let readable: ReadableStream = Reflect::get(&info, &"readable".into())?.dyn_into()?;
    let writable: WritableStream = Reflect::get(&info, &"writable".into())?.dyn_into()?;
    let reader: ReadableStreamDefaultReader = readable.get_reader().unchecked_into();
    let writer = writable.get_writer()?;
    sh.borrow_mut().sink = Some(Sink::Tcp(writer));

    let read_sh = sh.clone();
    spawn_local(async move {
        loop {
            let r = match JsFuture::from(reader.read()).await {
                Ok(r) => r,
                Err(e) => return fail(&read_sh, &format!("TCPSocket read failed: {e:?}")),
            };
            let done = Reflect::get(&r, &"done".into())
                .map(|d| d.is_truthy())
                .unwrap_or(true);
            if done {
                return fail(&read_sh, "the daemon closed the TCP connection");
            }
            if let Ok(v) = Reflect::get(&r, &"value".into()) {
                if let Some(a) = v.dyn_ref::<Uint8Array>() {
                    feed(&read_sh, &a.to_vec());
                }
            }
        }
    });
    Ok(())
}
