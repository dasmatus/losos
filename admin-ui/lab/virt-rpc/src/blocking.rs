//! A small blocking driver for native code and the live test: the same
//! [`Connection`] over anything `Read + Write` (a `UnixStream` to
//! `virtqemud-sock`, a `TcpStream` to 16509). Set a read timeout on the
//! stream to make [`Blocking::console_until`] give up.

use crate::client::{Connection, Event, Reply};
use crate::proto::{AuthType, Domain, ProtoError, RpcError};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Proto(ProtoError),
    Rpc(RpcError),
    /// The socket wants an auth method this client does not implement.
    Auth(Vec<AuthType>),
    /// The daemon closed the connection.
    Eof,
    /// A reply of the wrong shape (a bug on one side or the other).
    Unexpected(Reply),
    Timeout,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O: {e}"),
            Error::Proto(e) => write!(f, "protocol: {e}"),
            Error::Rpc(e) => e.fmt(f),
            Error::Auth(t) => write!(f, "unsupported auth {t:?}"),
            Error::Eof => f.write_str("daemon closed the connection"),
            Error::Unexpected(r) => write!(f, "unexpected reply {r:?}"),
            Error::Timeout => f.write_str("timed out"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<ProtoError> for Error {
    fn from(e: ProtoError) -> Self {
        Error::Proto(e)
    }
}

pub struct Blocking<S> {
    conn: Connection,
    io: S,
    replies: HashMap<u32, Result<Reply, RpcError>>,
    streams: HashMap<u32, Vec<u8>>,
    ended: HashMap<u32, Option<RpcError>>,
}

impl<S: Read + Write> Blocking<S> {
    pub fn new(io: S) -> Self {
        Self {
            conn: Connection::new(),
            io,
            replies: HashMap::new(),
            streams: HashMap::new(),
            ended: HashMap::new(),
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        let out = self.conn.take_output();
        if !out.is_empty() {
            self.io.write_all(&out)?;
            self.io.flush()?;
        }
        Ok(())
    }

    /// One read; false when the read timed out.
    fn pump(&mut self) -> Result<bool, Error> {
        let mut buf = [0u8; 65536];
        let n = match self.io.read(&mut buf) {
            Ok(0) => return Err(Error::Eof),
            Ok(n) => n,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Ok(false)
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => return Ok(true),
            Err(e) => return Err(e.into()),
        };
        for ev in self.conn.feed(&buf[..n])? {
            match ev {
                Event::Reply { serial, result } => {
                    self.replies.insert(serial, result);
                }
                Event::StreamData { stream, data } => {
                    self.streams.entry(stream).or_default().extend(data);
                }
                Event::StreamEnd { stream } => {
                    self.ended.insert(stream, None);
                }
                Event::StreamError { stream, error } => {
                    self.ended.insert(stream, Some(error));
                }
            }
        }
        // A keepalive pong, if one was queued.
        self.flush()?;
        Ok(true)
    }

    fn wait(&mut self, serial: u32) -> Result<Reply, Error> {
        self.flush()?;
        loop {
            if let Some(r) = self.replies.remove(&serial) {
                return r.map_err(Error::Rpc);
            }
            self.pump()?;
        }
    }

    /// Auth (none or polkit) and `CONNECT_OPEN`.
    pub fn open(&mut self, uri: &str) -> Result<(), Error> {
        let s = self.conn.auth_list();
        let types = match self.wait(s)? {
            Reply::AuthList(t) => t,
            r => return Err(Error::Unexpected(r)),
        };
        if types.contains(&AuthType::Polkit) {
            let s = self.conn.auth_polkit();
            self.wait(s)?;
        } else if !(types.is_empty() || types.contains(&AuthType::None)) {
            return Err(Error::Auth(types));
        }
        let s = self.conn.connect_open(Some(uri), 0);
        self.wait(s).map(drop)
    }

    /// (hypervisor, library) as libvirt numbers; see [`crate::version_string`].
    pub fn version(&mut self) -> Result<(u64, u64), Error> {
        let a = self.conn.get_version();
        let b = self.conn.get_lib_version();
        let hv = match self.wait(a)? {
            Reply::Version(v) => v,
            r => return Err(Error::Unexpected(r)),
        };
        match self.wait(b)? {
            Reply::LibVersion(l) => Ok((hv, l)),
            r => Err(Error::Unexpected(r)),
        }
    }

    pub fn create(&mut self, xml: &str, flags: u32) -> Result<Domain, Error> {
        let s = self.conn.domain_create_xml(xml, flags);
        match self.wait(s)? {
            Reply::Domain(d) => Ok(d),
            r => Err(Error::Unexpected(r)),
        }
    }

    pub fn lookup(&mut self, name: &str) -> Result<Domain, Error> {
        let s = self.conn.domain_lookup_by_name(name);
        match self.wait(s)? {
            Reply::Domain(d) => Ok(d),
            r => Err(Error::Unexpected(r)),
        }
    }

    pub fn destroy(&mut self, dom: &Domain) -> Result<(), Error> {
        let s = self.conn.domain_destroy(dom);
        self.wait(s).map(drop)
    }

    pub fn resume(&mut self, dom: &Domain) -> Result<(), Error> {
        let s = self.conn.domain_resume(dom);
        self.wait(s).map(drop)
    }

    /// Opens the first console; returns the stream id.
    pub fn open_console(&mut self, dom: &Domain, flags: u32) -> Result<u32, Error> {
        let s = self.conn.domain_open_console(dom, None, flags);
        self.wait(s)?;
        Ok(s)
    }

    pub fn console_send(&mut self, stream: u32, data: &[u8]) -> Result<(), Error> {
        self.conn.stream_send(stream, data);
        self.flush()
    }

    /// Reads the console until `done` holds for everything read so far, and
    /// returns (and forgets) it. Needs a read timeout on the stream to stop
    /// at `deadline` while the guest is quiet.
    pub fn console_until(
        &mut self,
        stream: u32,
        within: Duration,
        done: impl Fn(&[u8]) -> bool,
    ) -> Result<Vec<u8>, Error> {
        let deadline = Instant::now() + within;
        loop {
            if self.streams.get(&stream).is_some_and(|b| done(b)) {
                return Ok(self.streams.remove(&stream).unwrap_or_default());
            }
            if let Some(end) = self.ended.remove(&stream) {
                return match end {
                    Some(e) => Err(Error::Rpc(e)),
                    None => Err(Error::Eof),
                };
            }
            if Instant::now() > deadline {
                return Err(Error::Timeout);
            }
            self.pump()?;
        }
    }

    /// Everything the console printed that no `console_until` took yet.
    pub fn console_take(&mut self, stream: u32) -> Vec<u8> {
        self.streams.remove(&stream).unwrap_or_default()
    }

    pub fn close(mut self) -> Result<S, Error> {
        let s = self.conn.connect_close();
        self.wait(s)?;
        Ok(self.io)
    }
}
