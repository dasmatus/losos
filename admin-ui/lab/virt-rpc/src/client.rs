//! The sans-IO connection: calls go in as method calls and come out as bytes
//! (`take_output`), bytes from the daemon go in (`feed`) and come out as
//! [`Event`]s. Nothing here reads a socket, sleeps or spawns, so the same
//! state machine runs under the native test, the wasm wrapper and the unit
//! tests' fake daemon.

use crate::proto::{
    packet, AuthType, Domain, Header, MsgType, Proc, ProtoError, RpcError, Status,
    AUTH_TYPE_LIST_MAX, HEADER_LEN, KEEPALIVE_PROC_PING, KEEPALIVE_PROC_PONG, KEEPALIVE_PROGRAM,
    KEEPALIVE_PROTOCOL_VERSION, LEN_WORD, MESSAGE_MAX, REMOTE_PROGRAM, STREAM_CHUNK,
};
use crate::xdr::{Decoder, Encoder};
use std::collections::{HashMap, HashSet};

/// `VIR_DOMAIN_START_PAUSED`: create stopped, so a console opened before
/// [`Connection::domain_resume`] sees the guest's first byte.
pub const START_PAUSED: u32 = 1 << 0;
/// `VIR_DOMAIN_START_AUTODESTROY`: the domain dies with this connection, so
/// a closed tab leaves no guest running.
pub const START_AUTODESTROY: u32 = 1 << 1;
/// `VIR_DOMAIN_CONSOLE_FORCE`: take the console from whoever holds it.
pub const CONSOLE_FORCE: u32 = 1 << 0;

/// A successful reply, decoded by the procedure it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// Procedures with no return value (open, close, destroy, open console).
    Done,
    /// `remote_connect_get_version_ret.hv_ver`: major * 1e6 + minor * 1e3 + micro.
    Version(u64),
    /// `remote_connect_get_lib_version_ret.lib_ver`, same encoding.
    LibVersion(u64),
    Domain(Domain),
    AuthList(Vec<AuthType>),
    /// `remote_auth_polkit_ret.complete`.
    Polkit(bool),
    /// `remote_domain_get_state_ret`: `virDomainState` and its reason.
    State {
        state: i32,
        reason: i32,
    },
}

/// What the daemon said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Reply {
        serial: u32,
        result: Result<Reply, RpcError>,
    },
    /// Bytes from a stream (a console's output). `stream` is the serial of
    /// the call that opened it.
    StreamData { stream: u32, data: Vec<u8> },
    /// The daemon ended the stream (end of file, or our finish/abort
    /// confirmed). No more data follows.
    StreamEnd { stream: u32 },
    /// The stream failed; the daemon has dropped it.
    StreamError { stream: u32, error: RpcError },
}

/// One connection's protocol state.
#[derive(Debug, Default)]
pub struct Connection {
    serial: u32,
    out: Vec<u8>,
    inbuf: Vec<u8>,
    pending: HashMap<u32, Proc>,
    streams: HashMap<u32, Proc>,
    closed_streams: HashSet<u32>,
}

/// `virVersion`-style number to "major.minor.micro".
pub fn version_string(v: u64) -> String {
    format!("{}.{}.{}", v / 1_000_000, (v / 1_000) % 1_000, v % 1_000)
}

impl Connection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bytes to write to the transport, in order. Empties the queue.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out)
    }

    pub fn has_output(&self) -> bool {
        !self.out.is_empty()
    }

    /// Calls sent and not yet answered.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    fn call(&mut self, p: Proc, args: Encoder) -> u32 {
        self.serial = self.serial.wrapping_add(1);
        let serial = self.serial;
        self.out
            .extend(packet(&Header::call(p, serial), &args.into_bytes()));
        self.pending.insert(serial, p);
        serial
    }

    /// `REMOTE_PROC_AUTH_LIST`: which auth the socket wants before open.
    pub fn auth_list(&mut self) -> u32 {
        self.call(Proc::AuthList, Encoder::new())
    }

    /// `REMOTE_PROC_AUTH_POLKIT`: ask the daemon to check its peer (the
    /// process at the other end of the unix socket) with polkit.
    pub fn auth_polkit(&mut self) -> u32 {
        self.call(Proc::AuthPolkit, Encoder::new())
    }

    /// `REMOTE_PROC_CONNECT_OPEN`. `uri` is what the daemon's driver sees,
    /// e.g. `qemu:///system` or `qemu:///session`.
    pub fn connect_open(&mut self, uri: Option<&str>, flags: u32) -> u32 {
        let mut e = Encoder::new();
        e.opt_string(uri).u32(flags);
        self.call(Proc::ConnectOpen, e)
    }

    pub fn connect_close(&mut self) -> u32 {
        self.call(Proc::ConnectClose, Encoder::new())
    }

    pub fn get_version(&mut self) -> u32 {
        self.call(Proc::ConnectGetVersion, Encoder::new())
    }

    pub fn get_lib_version(&mut self) -> u32 {
        self.call(Proc::ConnectGetLibVersion, Encoder::new())
    }

    /// `REMOTE_PROC_DOMAIN_CREATE_XML`: start a transient domain.
    pub fn domain_create_xml(&mut self, xml: &str, flags: u32) -> u32 {
        let mut e = Encoder::new();
        e.string(xml).u32(flags);
        self.call(Proc::DomainCreateXml, e)
    }

    pub fn domain_lookup_by_name(&mut self, name: &str) -> u32 {
        let mut e = Encoder::new();
        e.string(name);
        self.call(Proc::DomainLookupByName, e)
    }

    pub fn domain_destroy(&mut self, dom: &Domain) -> u32 {
        let mut e = Encoder::new();
        dom.encode(&mut e);
        self.call(Proc::DomainDestroy, e)
    }

    pub fn domain_resume(&mut self, dom: &Domain) -> u32 {
        let mut e = Encoder::new();
        dom.encode(&mut e);
        self.call(Proc::DomainResume, e)
    }

    pub fn domain_get_state(&mut self, dom: &Domain) -> u32 {
        let mut e = Encoder::new();
        dom.encode(&mut e);
        e.u32(0);
        self.call(Proc::DomainGetState, e)
    }

    /// `REMOTE_PROC_DOMAIN_OPEN_CONSOLE`. The returned serial is both the
    /// call's and the stream's: data in both directions travels as
    /// `VIR_NET_STREAM` packets carrying it. `dev` None is the first console.
    pub fn domain_open_console(&mut self, dom: &Domain, dev: Option<&str>, flags: u32) -> u32 {
        let mut e = Encoder::new();
        dom.encode(&mut e);
        e.opt_string(dev).u32(flags);
        let serial = self.call(Proc::DomainOpenConsole, e);
        self.streams.insert(serial, Proc::DomainOpenConsole);
        serial
    }

    fn stream_packet(&mut self, stream: u32, status: Status, payload: &[u8]) -> bool {
        let Some(&p) = self.streams.get(&stream) else {
            return false;
        };
        let h = Header {
            prog: REMOTE_PROGRAM,
            vers: crate::proto::REMOTE_PROTOCOL_VERSION,
            proc_: p as i32,
            kind: MsgType::Stream,
            serial: stream,
            status,
        };
        self.out.extend(packet(&h, payload));
        true
    }

    /// Queue bytes for the stream (keys typed into a console). Returns false
    /// for a stream that is not open.
    pub fn stream_send(&mut self, stream: u32, data: &[u8]) -> bool {
        if !self.streams.contains_key(&stream) || self.closed_streams.contains(&stream) {
            return false;
        }
        for chunk in data.chunks(STREAM_CHUNK) {
            self.stream_packet(stream, Status::Continue, chunk);
        }
        true
    }

    /// End our side cleanly (status OK, no payload). The daemon confirms
    /// with an empty packet, reported as [`Event::StreamEnd`].
    pub fn stream_finish(&mut self, stream: u32) -> bool {
        self.closed_streams.insert(stream) && self.stream_packet(stream, Status::Ok, &[])
    }

    /// Abort the stream (status ERROR, no payload), as `virStreamAbort` does.
    pub fn stream_abort(&mut self, stream: u32) -> bool {
        self.closed_streams.insert(stream) && self.stream_packet(stream, Status::Error, &[])
    }

    /// Bytes from the daemon, in any split. Returns every event the bytes
    /// complete; a partial packet waits for the next call.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Event>, ProtoError> {
        self.inbuf.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            if self.inbuf.len() < LEN_WORD {
                break;
            }
            let len =
                u32::from_be_bytes([self.inbuf[0], self.inbuf[1], self.inbuf[2], self.inbuf[3]]);
            if (len as usize) < LEN_WORD + HEADER_LEN || len - LEN_WORD as u32 > MESSAGE_MAX {
                return Err(ProtoError::BadLength(len));
            }
            if self.inbuf.len() < len as usize {
                break;
            }
            let pkt: Vec<u8> = self.inbuf.drain(..len as usize).collect();
            if let Some(ev) = self.packet_in(&pkt[LEN_WORD..])? {
                events.push(ev);
            }
        }
        Ok(events)
    }

    fn packet_in(&mut self, body: &[u8]) -> Result<Option<Event>, ProtoError> {
        let mut d = Decoder::new(body);
        let h = Header::decode(&mut d)?;
        if h.prog == KEEPALIVE_PROGRAM {
            // The daemon pings only clients that asked for keepalive, which
            // this one never does; answer anyway, a missed pong closes us.
            if h.proc_ == KEEPALIVE_PROC_PING {
                let pong = Header {
                    prog: KEEPALIVE_PROGRAM,
                    vers: KEEPALIVE_PROTOCOL_VERSION,
                    proc_: KEEPALIVE_PROC_PONG,
                    kind: MsgType::Message,
                    serial: 0,
                    status: Status::Ok,
                };
                self.out.extend(packet(&pong, &[]));
            }
            return Ok(None);
        }
        if h.prog != REMOTE_PROGRAM {
            return Err(ProtoError::UnknownProgram(h.prog));
        }
        match h.kind {
            MsgType::Reply | MsgType::ReplyWithFds => {
                let Some(p) = self.pending.remove(&h.serial) else {
                    return Err(ProtoError::UnexpectedReply {
                        serial: h.serial,
                        proc_: h.proc_,
                    });
                };
                let result = if h.status == Status::Error {
                    if self.streams.remove(&h.serial).is_some() {
                        self.closed_streams.remove(&h.serial);
                    }
                    Err(RpcError::decode(&mut d)?)
                } else {
                    Ok(decode_reply(p, &mut d)?)
                };
                Ok(Some(Event::Reply {
                    serial: h.serial,
                    result,
                }))
            }
            MsgType::Stream => {
                if !self.streams.contains_key(&h.serial) {
                    // Late data for a stream we already dropped.
                    return Ok(None);
                }
                match h.status {
                    Status::Continue => Ok(Some(Event::StreamData {
                        stream: h.serial,
                        data: d.remaining().to_vec(),
                    })),
                    Status::Ok => {
                        self.streams.remove(&h.serial);
                        self.closed_streams.remove(&h.serial);
                        Ok(Some(Event::StreamEnd { stream: h.serial }))
                    }
                    Status::Error => {
                        self.streams.remove(&h.serial);
                        self.closed_streams.remove(&h.serial);
                        let error = RpcError::decode(&mut d)
                            .unwrap_or_else(|_| RpcError::local("stream aborted by the daemon"));
                        Ok(Some(Event::StreamError {
                            stream: h.serial,
                            error,
                        }))
                    }
                }
            }
            // Async events (VIR_NET_MESSAGE) need a callback registration
            // this client never makes; holes only come with sparse uploads.
            MsgType::Message | MsgType::StreamHole => Ok(None),
            MsgType::Call | MsgType::CallWithFds => Err(ProtoError::BadType(h.kind as u32)),
        }
    }
}

fn decode_reply(p: Proc, d: &mut Decoder<'_>) -> Result<Reply, ProtoError> {
    Ok(match p {
        Proc::ConnectOpen
        | Proc::ConnectClose
        | Proc::DomainDestroy
        | Proc::DomainResume
        | Proc::DomainOpenConsole => Reply::Done,
        Proc::ConnectGetVersion => Reply::Version(d.u64()?),
        Proc::ConnectGetLibVersion => Reply::LibVersion(d.u64()?),
        Proc::DomainCreateXml | Proc::DomainLookupByName => Reply::Domain(Domain::decode(d)?),
        Proc::AuthList => {
            let n = d.u32()?;
            if n > AUTH_TYPE_LIST_MAX {
                return Err(crate::xdr::XdrError::TooLong {
                    len: n,
                    max: AUTH_TYPE_LIST_MAX,
                }
                .into());
            }
            let mut v = Vec::with_capacity(n as usize);
            for _ in 0..n {
                v.push(AuthType::from_u32(d.u32()?));
            }
            Reply::AuthList(v)
        }
        Proc::AuthPolkit => Reply::Polkit(d.i32()? != 0),
        Proc::DomainGetState => Reply::State {
            state: d.i32()?,
            reason: d.i32()?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A daemon's packet, built the way virnetserverprogram.c builds them.
    fn server(kind: MsgType, p: i32, serial: u32, status: Status, payload: &[u8]) -> Vec<u8> {
        packet(
            &Header {
                prog: REMOTE_PROGRAM,
                vers: 1,
                proc_: p,
                kind,
                serial,
                status,
            },
            payload,
        )
    }

    #[test]
    fn connect_open_is_byte_exact() {
        let mut c = Connection::new();
        let s = c.connect_open(Some("qemu:///system"), 0);
        assert_eq!(s, 1);
        let out = c.take_output();
        let mut want = Vec::new();
        want.extend_from_slice(&56u32.to_be_bytes()); // 4 + 24 + 28
        for w in [0x2000_8086u32, 1, 1, 0, 1, 0] {
            want.extend_from_slice(&w.to_be_bytes());
        }
        want.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 14]);
        want.extend_from_slice(b"qemu:///system\0\0");
        want.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(out, want);
    }

    #[test]
    fn replies_decode_by_procedure_in_any_split() {
        let mut c = Connection::new();
        let v = c.get_version();
        let l = c.get_lib_version();
        let mut bytes = server(
            MsgType::Reply,
            4,
            v,
            Status::Ok,
            &11_001_000u64.to_be_bytes(),
        );
        bytes.extend(server(
            MsgType::Reply,
            157,
            l,
            Status::Ok,
            &12_007_000u64.to_be_bytes(),
        ));
        let mut evs = Vec::new();
        for b in bytes.chunks(5) {
            evs.extend(c.feed(b).unwrap());
        }
        assert_eq!(
            evs,
            vec![
                Event::Reply {
                    serial: v,
                    result: Ok(Reply::Version(11_001_000))
                },
                Event::Reply {
                    serial: l,
                    result: Ok(Reply::LibVersion(12_007_000))
                },
            ]
        );
        assert_eq!(version_string(12_007_000), "12.7.0");
        assert_eq!(c.pending(), 0);
    }

    #[test]
    fn errors_carry_the_daemons_message() {
        let mut c = Connection::new();
        let s = c.domain_lookup_by_name("ghost");
        let mut e = Encoder::new();
        e.i32(42)
            .i32(10)
            .opt_string(Some(
                "Domain not found: no domain with matching name 'ghost'",
            ))
            .i32(2)
            .u32(0)
            .opt_string(None)
            .opt_string(None)
            .opt_string(None)
            .i32(0)
            .i32(0)
            .u32(0);
        let evs = c
            .feed(&server(
                MsgType::Reply,
                23,
                s,
                Status::Error,
                &e.into_bytes(),
            ))
            .unwrap();
        let Event::Reply {
            result: Err(err), ..
        } = &evs[0]
        else {
            panic!("{evs:?}")
        };
        assert_eq!(err.code, 42);
        assert!(err.to_string().contains("no domain with matching name"));
    }

    #[test]
    fn console_stream_both_ways() {
        let mut c = Connection::new();
        let dom = Domain {
            name: "pc1".into(),
            uuid: [7; 16],
            id: 3,
        };
        let s = c.domain_open_console(&dom, None, 0);
        c.take_output();
        let mut bytes = server(MsgType::Reply, 201, s, Status::Ok, &[]);
        bytes.extend(server(
            MsgType::Stream,
            201,
            s,
            Status::Continue,
            b"login: ",
        ));
        bytes.extend(server(MsgType::Stream, 201, s, Status::Continue, b"/ # "));
        let evs = c.feed(&bytes).unwrap();
        assert_eq!(evs.len(), 3);
        assert_eq!(
            evs[2],
            Event::StreamData {
                stream: s,
                data: b"/ # ".to_vec()
            }
        );

        assert!(c.stream_send(s, b"uname\r"));
        let out = c.take_output();
        let mut d = Decoder::new(&out[4..]);
        let h = Header::decode(&mut d).unwrap();
        assert_eq!(
            (h.kind, h.status, h.serial, h.proc_),
            (MsgType::Stream, Status::Continue, s, 201)
        );
        assert_eq!(d.remaining(), b"uname\r");

        assert!(c.stream_finish(s));
        assert!(!c.stream_send(s, b"late"));
        let evs = c
            .feed(&server(MsgType::Stream, 201, s, Status::Ok, &[]))
            .unwrap();
        assert_eq!(evs, vec![Event::StreamEnd { stream: s }]);
    }

    #[test]
    fn big_writes_are_chunked_like_libvirt() {
        let mut c = Connection::new();
        let dom = Domain {
            name: "x".into(),
            uuid: [0; 16],
            id: 1,
        };
        let s = c.domain_open_console(&dom, None, 0);
        c.take_output();
        c.stream_send(s, &vec![b'a'; STREAM_CHUNK + 10]);
        let out = c.take_output();
        assert_eq!(out.len(), 2 * (LEN_WORD + HEADER_LEN) + STREAM_CHUNK + 10);
    }

    #[test]
    fn keepalive_ping_gets_a_pong() {
        let mut c = Connection::new();
        let ping = packet(
            &Header {
                prog: KEEPALIVE_PROGRAM,
                vers: 1,
                proc_: KEEPALIVE_PROC_PING,
                kind: MsgType::Message,
                serial: 0,
                status: Status::Ok,
            },
            &[],
        );
        assert!(c.feed(&ping).unwrap().is_empty());
        let out = c.take_output();
        assert_eq!(&out[4..8], &KEEPALIVE_PROGRAM.to_be_bytes());
        assert_eq!(&out[12..16], &KEEPALIVE_PROC_PONG.to_be_bytes());
    }

    #[test]
    fn garbage_is_refused() {
        let mut c = Connection::new();
        assert_eq!(c.feed(&[0, 0, 0, 3]), Err(ProtoError::BadLength(3)));
        let mut c = Connection::new();
        let stray = server(MsgType::Reply, 1, 99, Status::Ok, &[]);
        assert!(matches!(
            c.feed(&stray),
            Err(ProtoError::UnexpectedReply { serial: 99, .. })
        ));
    }
}
