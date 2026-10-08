//! Constants and structures from libvirt 12.7.0's protocol files:
//! `src/rpc/virnetprotocol.x` (the packet header, message types, errors),
//! `src/rpc/virkeepaliveprotocol.x` (ping/pong) and
//! `src/remote/remote_protocol.x` (procedure numbers and their arguments).
//! Only the procedures the Lab uses are here; every number is the one in
//! `enum remote_procedure`, which libvirt never renumbers.

use crate::xdr::{Decoder, Encoder, XdrError};
use std::fmt;

/// `REMOTE_PROGRAM`.
pub const REMOTE_PROGRAM: u32 = 0x2000_8086;
/// `REMOTE_PROTOCOL_VERSION`.
pub const REMOTE_PROTOCOL_VERSION: u32 = 1;
/// `KEEPALIVE_PROGRAM` ("keep").
pub const KEEPALIVE_PROGRAM: u32 = 0x6b65_6570;
pub const KEEPALIVE_PROTOCOL_VERSION: u32 = 1;
pub const KEEPALIVE_PROC_PING: i32 = 1;
pub const KEEPALIVE_PROC_PONG: i32 = 2;

/// `VIR_NET_MESSAGE_MAX`: header plus payload, without the length word.
pub const MESSAGE_MAX: u32 = 33_554_432;
/// `VIR_NET_MESSAGE_LEN_MAX`: the length word, which counts itself.
pub const LEN_WORD: usize = 4;
/// `VIR_NET_MESSAGE_HEADER_XDR_LEN` words after the length: six of them.
pub const HEADER_LEN: usize = 24;
/// `VIR_NET_MESSAGE_LEGACY_PAYLOAD_MAX`, the stream chunk libvirt's own
/// client sends, so an older daemon accepts every packet we write.
pub const STREAM_CHUNK: usize = 262_120;
/// `VIR_NET_MESSAGE_STRING_MAX` and `REMOTE_STRING_MAX`.
pub const STRING_MAX: u32 = 4_194_304;
/// `REMOTE_AUTH_TYPE_LIST_MAX`.
pub const AUTH_TYPE_LIST_MAX: u32 = 20;
/// `VIR_UUID_BUFLEN`.
pub const UUID_LEN: usize = 16;

/// Procedures of `REMOTE_PROGRAM` this client calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Proc {
    ConnectOpen = 1,
    ConnectClose = 2,
    ConnectGetVersion = 4,
    DomainCreateXml = 10,
    DomainDestroy = 12,
    DomainLookupByName = 23,
    DomainResume = 28,
    AuthList = 66,
    AuthPolkit = 70,
    ConnectGetLibVersion = 157,
    DomainOpenConsole = 201,
    DomainGetState = 212,
}

impl Proc {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            1 => Proc::ConnectOpen,
            2 => Proc::ConnectClose,
            4 => Proc::ConnectGetVersion,
            10 => Proc::DomainCreateXml,
            12 => Proc::DomainDestroy,
            23 => Proc::DomainLookupByName,
            28 => Proc::DomainResume,
            66 => Proc::AuthList,
            70 => Proc::AuthPolkit,
            157 => Proc::ConnectGetLibVersion,
            201 => Proc::DomainOpenConsole,
            212 => Proc::DomainGetState,
            _ => return None,
        })
    }
}

/// `enum virNetMessageType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Call = 0,
    Reply = 1,
    Message = 2,
    Stream = 3,
    CallWithFds = 4,
    ReplyWithFds = 5,
    StreamHole = 6,
}

impl MsgType {
    fn from_u32(v: u32) -> Option<Self> {
        Some(match v {
            0 => MsgType::Call,
            1 => MsgType::Reply,
            2 => MsgType::Message,
            3 => MsgType::Stream,
            4 => MsgType::CallWithFds,
            5 => MsgType::ReplyWithFds,
            6 => MsgType::StreamHole,
            _ => return None,
        })
    }
}

/// `enum virNetMessageStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok = 0,
    Error = 1,
    Continue = 2,
}

impl Status {
    fn from_u32(v: u32) -> Option<Self> {
        Some(match v {
            0 => Status::Ok,
            1 => Status::Error,
            2 => Status::Continue,
            _ => return None,
        })
    }
}

/// `struct virNetMessageHeader`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub prog: u32,
    pub vers: u32,
    pub proc_: i32,
    pub kind: MsgType,
    pub serial: u32,
    pub status: Status,
}

impl Header {
    pub fn call(proc_: Proc, serial: u32) -> Self {
        Self {
            prog: REMOTE_PROGRAM,
            vers: REMOTE_PROTOCOL_VERSION,
            proc_: proc_ as i32,
            kind: MsgType::Call,
            serial,
            status: Status::Ok,
        }
    }

    pub fn encode(&self, e: &mut Encoder) {
        e.u32(self.prog)
            .u32(self.vers)
            .i32(self.proc_)
            .u32(self.kind as u32)
            .u32(self.serial)
            .u32(self.status as u32);
    }

    pub fn decode(d: &mut Decoder<'_>) -> Result<Self, ProtoError> {
        let prog = d.u32()?;
        let vers = d.u32()?;
        let proc_ = d.i32()?;
        let kind = d.u32()?;
        let serial = d.u32()?;
        let status = d.u32()?;
        Ok(Self {
            prog,
            vers,
            proc_,
            kind: MsgType::from_u32(kind).ok_or(ProtoError::BadType(kind))?,
            serial,
            status: Status::from_u32(status).ok_or(ProtoError::BadStatus(status))?,
        })
    }
}

/// One whole packet: the length word, the header, the payload.
pub fn packet(h: &Header, payload: &[u8]) -> Vec<u8> {
    let mut e = Encoder::new();
    let total = LEN_WORD + HEADER_LEN + payload.len();
    e.u32(u32::try_from(total).unwrap_or(u32::MAX));
    h.encode(&mut e);
    let mut b = e.into_bytes();
    b.extend_from_slice(payload);
    b
}

/// `struct remote_nonnull_domain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domain {
    pub name: String,
    pub uuid: [u8; UUID_LEN],
    /// -1 for a domain that is not running.
    pub id: i32,
}

impl Domain {
    pub fn encode(&self, e: &mut Encoder) {
        e.string(&self.name).fixed_opaque(&self.uuid).i32(self.id);
    }

    pub fn decode(d: &mut Decoder<'_>) -> Result<Self, XdrError> {
        let name = d.string(STRING_MAX)?;
        let mut uuid = [0u8; UUID_LEN];
        uuid.copy_from_slice(d.fixed_opaque(UUID_LEN)?);
        let id = d.i32()?;
        Ok(Self { name, uuid, id })
    }

    /// The UUID in libvirt's printed form.
    pub fn uuid_string(&self) -> String {
        let h: String = self.uuid.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{}-{}-{}-{}-{}",
            &h[0..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..32]
        )
    }
}

/// `enum remote_auth_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    None,
    Sasl,
    Polkit,
    Other(u32),
}

impl AuthType {
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => AuthType::None,
            1 => AuthType::Sasl,
            2 => AuthType::Polkit,
            v => AuthType::Other(v),
        }
    }
}

/// `struct virNetMessageError`: what the daemon sends with status ERROR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    /// `virErrorNumber`, e.g. 42 VIR_ERR_NO_DOMAIN, 38 VIR_ERR_SYSTEM_ERROR.
    pub code: i32,
    /// `virErrorDomain`, the subsystem that raised it.
    pub domain: i32,
    pub message: Option<String>,
    pub level: i32,
}

impl RpcError {
    pub fn decode(d: &mut Decoder<'_>) -> Result<Self, XdrError> {
        let code = d.i32()?;
        let domain = d.i32()?;
        let message = d.opt_string(STRING_MAX)?;
        let level = d.i32()?;
        // dom (unused), str1..3, int1, int2, net (unused): read past them so
        // a malformed tail is still caught, keep only what a person reads.
        if d.bool()? {
            Domain::decode(d)?;
        }
        for _ in 0..3 {
            d.opt_string(STRING_MAX)?;
        }
        d.i32()?;
        d.i32()?;
        if d.bool()? {
            d.string(STRING_MAX)?;
            d.fixed_opaque(UUID_LEN)?;
        }
        Ok(Self {
            code,
            domain,
            message,
            level,
        })
    }

    pub(crate) fn local(message: impl Into<String>) -> Self {
        Self {
            code: -1,
            domain: 0,
            message: Some(message.into()),
            level: 2,
        }
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(m) => write!(f, "{m} (libvirt error {})", self.code),
            None => write!(f, "libvirt error {}", self.code),
        }
    }
}

impl std::error::Error for RpcError {}

/// A byte stream that is not libvirt's protocol, or that this client
/// cannot follow. The connection is unusable after one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtoError {
    Xdr(XdrError),
    BadLength(u32),
    BadType(u32),
    BadStatus(u32),
    UnknownProgram(u32),
    /// A reply to a serial this client never sent.
    UnexpectedReply {
        serial: u32,
        proc_: i32,
    },
}

impl From<XdrError> for ProtoError {
    fn from(e: XdrError) -> Self {
        ProtoError::Xdr(e)
    }
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtoError::Xdr(e) => e.fmt(f),
            ProtoError::BadLength(n) => write!(f, "packet length {n} out of range"),
            ProtoError::BadType(t) => write!(f, "unknown message type {t}"),
            ProtoError::BadStatus(s) => write!(f, "unknown message status {s}"),
            ProtoError::UnknownProgram(p) => write!(f, "unknown program {p:#x}"),
            ProtoError::UnexpectedReply { serial, proc_ } => {
                write!(f, "reply to unsent call {serial} (procedure {proc_})")
            }
        }
    }
}

impl std::error::Error for ProtoError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips() {
        let h = Header::call(Proc::DomainOpenConsole, 7);
        let p = packet(&h, &[1, 2, 3, 4]);
        assert_eq!(p.len(), 32);
        assert_eq!(&p[..4], &32u32.to_be_bytes());
        assert_eq!(&p[4..8], &[0x20, 0x00, 0x80, 0x86]);
        let mut d = Decoder::new(&p[4..]);
        assert_eq!(Header::decode(&mut d).unwrap(), h);
        assert_eq!(d.remaining(), &[1, 2, 3, 4]);
    }

    #[test]
    fn uuid_prints_like_libvirt() {
        let dom = Domain {
            name: "a".into(),
            uuid: [
                0x6f, 0x2d, 0x1e, 0xab, 0, 1, 0x40, 2, 0x80, 3, 0xde, 0xad, 0xbe, 0xef, 0, 9,
            ],
            id: 1,
        };
        assert_eq!(dom.uuid_string(), "6f2d1eab-0001-4002-8003-deadbeef0009");
    }
}
