//! The forwarding loop: one str0m `Rtc` per member, every frame a member
//! sends handed to the other members of the same call.
//!
//! The loop is sans-IO str0m driven from one task: UDP datagrams from the
//! sockets, commands from the signalling sockets, and str0m's own timers
//! all arrive here, so call state needs no lock. The frames it forwards are
//! the browsers' ciphertext (see `admin-ui/app/src/calls/e2ee.ts`); the SFU
//! has no key and does not try to read them.
//!
//! The track bookkeeping follows str0m's own SFU example: a member's
//! incoming track is a strong `Arc` the member owns, every other member
//! holds a `Weak` to it and negotiates a send-only m-line for it, and stops
//! that m-line once the `Weak` is dead.

use super::admit::{CallId, Calls, ParticipantId, Refusal, UserId};
use super::tap::{Kind, Tap};
use super::wire::{stream_id, MemberInfo, ServerMsg};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use str0m::change::{SdpAnswer, SdpOffer, SdpPendingOffer};
use str0m::media::{Direction, KeyframeRequest, KeyframeRequestKind, MediaData, MediaKind, Mid};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};

pub type Reply<T> = oneshot::Sender<Result<T, Refusal>>;

/// What the signalling side asks of the loop.
#[derive(Debug)]
pub enum Command {
    Create {
        id: CallId,
        creator: UserId,
        room: String,
        invited: Vec<String>,
        video: bool,
        reply: Reply<()>,
    },
    Join {
        call: CallId,
        user: UserId,
        device: String,
        out: mpsc::UnboundedSender<ServerMsg>,
        reply: Reply<ParticipantId>,
    },
    Offer {
        pid: ParticipantId,
        sdp: String,
    },
    Answer {
        pid: ParticipantId,
        sdp: String,
    },
    Leave {
        pid: ParticipantId,
    },
}

pub struct Config {
    pub server_name: String,
    /// The local addresses the UDP sockets are bound to, which are also the
    /// host candidates the SFU offers.
    pub sockets: Vec<UdpSocket>,
    pub tap: Option<Tap>,
}

struct Datagram {
    data: Vec<u8>,
    source: SocketAddr,
    destination: SocketAddr,
}

/// Run the loop until every command sender is dropped.
pub async fn run(config: Config, mut commands: mpsc::Receiver<Command>) {
    let sockets: Vec<Arc<UdpSocket>> = config.sockets.into_iter().map(Arc::new).collect();
    let addrs: Vec<SocketAddr> = sockets.iter().filter_map(|s| s.local_addr().ok()).collect();
    let (dgram_tx, mut dgrams) = mpsc::channel::<Datagram>(4096);
    for socket in &sockets {
        let socket = socket.clone();
        let tx = dgram_tx.clone();
        tokio::spawn(async move {
            let Ok(destination) = socket.local_addr() else {
                return;
            };
            let mut buf = vec![0u8; 2000];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((n, source)) => {
                        let d = Datagram {
                            data: buf[..n].to_vec(),
                            source,
                            destination,
                        };
                        if tx.send(d).await.is_err() {
                            return;
                        }
                    }
                    Err(e) => tracing::debug!("udp recv on {destination}: {e}"),
                }
            }
        });
    }
    drop(dgram_tx);

    let mut sfu = Sfu {
        calls: Calls::new(&config.server_name),
        clients: Vec::new(),
        sockets,
        addrs,
        tap: config.tap,
        next_pid: 1,
    };

    loop {
        let timeout = sfu.drive();
        let sleep = tokio::time::sleep_until(tokio::time::Instant::from_std(timeout));
        tokio::select! {
            cmd = commands.recv() => match cmd {
                Some(cmd) => sfu.command(cmd),
                None => return,
            },
            Some(d) = dgrams.recv() => sfu.datagram(d),
            _ = sleep => {}
        }
        let now = Instant::now();
        for c in &mut sfu.clients {
            c.input(Input::Timeout(now));
        }
    }
}

struct Sfu {
    calls: Calls,
    clients: Vec<Client>,
    sockets: Vec<Arc<UdpSocket>>,
    addrs: Vec<SocketAddr>,
    tap: Option<Tap>,
    next_pid: ParticipantId,
}

impl Sfu {
    fn command(&mut self, cmd: Command) {
        let now = Instant::now();
        match cmd {
            Command::Create {
                id,
                creator,
                room,
                invited,
                video,
                reply,
            } => {
                let r = self.calls.create(id, creator, &room, &invited, video, now);
                let _ = reply.send(r);
            }
            Command::Join {
                call,
                user,
                device,
                out,
                reply,
            } => {
                let pid = self.next_pid;
                match self.calls.admit(&call, pid, user, &device, now) {
                    Ok(c) => {
                        self.next_pid += 1;
                        let video = c.video;
                        let mut rtc = Rtc::builder()
                            .set_ice_lite(true)
                            .clear_codecs()
                            .enable_opus(true, false)
                            .enable_vp8(true)
                            .build(now);
                        for addr in &self.addrs {
                            match Candidate::host(*addr, "udp") {
                                Ok(cand) => {
                                    rtc.add_local_candidate(cand);
                                }
                                Err(e) => tracing::warn!("no candidate for {addr}: {e}"),
                            }
                        }
                        let client = Client {
                            pid,
                            call: call.clone(),
                            out,
                            rtc,
                            ready: false,
                            pending: None,
                            tracks_in: Vec::new(),
                            tracks_out: Vec::new(),
                        };
                        let members = self.members(&call);
                        let _ = client.out.send(ServerMsg::Joined {
                            you: pid,
                            video,
                            members,
                        });
                        self.clients.push(client);
                        self.announce(&call);
                        let _ = reply.send(Ok(pid));
                    }
                    Err(r) => {
                        let _ = reply.send(Err(r));
                    }
                }
            }
            Command::Offer { pid, sdp } => {
                let tracks: Vec<Weak<TrackIn>> = self.tracks_of_others(pid);
                if let Some(c) = self.client(pid) {
                    c.offer(&sdp, tracks);
                }
            }
            Command::Answer { pid, sdp } => {
                if let Some(c) = self.client(pid) {
                    c.answer(&sdp);
                }
            }
            Command::Leave { pid } => {
                if let Some(c) = self.client(pid) {
                    c.rtc.disconnect();
                }
                self.reap();
            }
        }
    }

    fn client(&mut self, pid: ParticipantId) -> Option<&mut Client> {
        self.clients.iter_mut().find(|c| c.pid == pid)
    }

    /// The incoming tracks of everyone else in `pid`'s call.
    fn tracks_of_others(&self, pid: ParticipantId) -> Vec<Weak<TrackIn>> {
        let Some(call) = self.clients.iter().find(|c| c.pid == pid).map(|c| &c.call) else {
            return Vec::new();
        };
        self.clients
            .iter()
            .filter(|c| c.pid != pid && &c.call == call)
            .flat_map(|c| c.tracks_in.iter().map(Arc::downgrade))
            .collect()
    }

    fn members(&self, call: &CallId) -> Vec<MemberInfo> {
        self.calls
            .get(call)
            .map(|c| {
                c.members
                    .iter()
                    .map(|(pid, m)| MemberInfo {
                        pid: *pid,
                        user: m.user.to_string(),
                        device: m.device.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Tell everyone in `call` who is in it now.
    fn announce(&self, call: &CallId) {
        let members = self.members(call);
        for c in self.clients.iter().filter(|c| &c.call == call) {
            let _ = c.out.send(ServerMsg::Members {
                members: members.clone(),
            });
        }
    }

    fn datagram(&mut self, d: Datagram) {
        let Ok(contents) = d.data.as_slice().try_into() else {
            return;
        };
        let input = Input::Receive(
            Instant::now(),
            Receive {
                proto: Protocol::Udp,
                source: d.source,
                destination: d.destination,
                contents,
            },
        );
        if let Some(c) = self.clients.iter_mut().find(|c| c.rtc.accepts(&input)) {
            c.input(input);
        }
    }

    /// Drop members whose connection died or whose page left.
    fn reap(&mut self) {
        let now = Instant::now();
        let mut left = Vec::new();
        self.clients.retain(|c| {
            let alive = c.rtc.is_alive() && !c.out.is_closed();
            if !alive {
                left.push(c.pid);
            }
            alive
        });
        let mut calls = Vec::new();
        for pid in left {
            tracing::info!("p{pid} left");
            if let Some(call) = self.calls.leave(pid, now) {
                calls.push(call);
            }
        }
        for call in calls {
            self.announce(&call);
        }
    }

    /// Poll every member until str0m has nothing more to say, pass on what
    /// came out, and return when the loop must next wake up.
    fn drive(&mut self) -> Instant {
        let mut queue = VecDeque::new();
        loop {
            self.reap();
            let mut timeout = Instant::now() + Duration::from_millis(100);
            for c in &mut self.clients {
                let t = c.poll(&self.sockets, &mut queue);
                timeout = timeout.min(t);
            }
            if queue.is_empty() {
                return timeout;
            }
            while let Some(p) = queue.pop_front() {
                self.propagate(p);
            }
        }
    }

    fn propagate(&mut self, p: Propagated) {
        let Some(origin) = self.clients.iter().find(|c| c.pid == p.origin()).map(|c| c.call.clone())
        else {
            return;
        };
        if let (Propagated::MediaData(pid, data), Some(tap)) = (&p, self.tap.as_mut()) {
            let kind = match self
                .clients
                .iter()
                .find(|c| c.pid == *pid)
                .and_then(|c| c.tracks_in.iter().find(|t| t.mid == data.mid))
                .map(|t| t.kind)
            {
                Some(MediaKind::Video) => Kind::Video,
                _ => Kind::Audio,
            };
            tap.record(*pid, &data.mid.to_string(), kind, data.time.numer(), &data.data);
        }
        for c in self.clients.iter_mut().filter(|c| c.call == origin && c.pid != p.origin()) {
            match &p {
                Propagated::TrackOpen(_, track) => c.track_open(track.clone()),
                Propagated::MediaData(from, data) => c.media_out(*from, data),
                Propagated::KeyframeRequest(_, req, source, mid) => {
                    if c.pid == *source {
                        c.keyframe_request(*req, *mid);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct TrackIn {
    origin: ParticipantId,
    mid: Mid,
    kind: MediaKind,
}

#[derive(Debug)]
struct TrackOut {
    track: Weak<TrackIn>,
    state: OutState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutState {
    ToOpen,
    Negotiating(Mid),
    Open(Mid),
    ToStop(Mid),
    NegotiatingStop(Mid),
}

impl TrackOut {
    fn mid(&self) -> Option<Mid> {
        match self.state {
            OutState::ToOpen => None,
            OutState::Negotiating(m)
            | OutState::Open(m)
            | OutState::ToStop(m)
            | OutState::NegotiatingStop(m) => Some(m),
        }
    }
}

enum Propagated {
    TrackOpen(ParticipantId, Weak<TrackIn>),
    MediaData(ParticipantId, Box<MediaData>),
    /// From, the request, the member whose track it is, that track's mid.
    KeyframeRequest(ParticipantId, KeyframeRequest, ParticipantId, Mid),
}

impl Propagated {
    fn origin(&self) -> ParticipantId {
        match self {
            Propagated::TrackOpen(p, _)
            | Propagated::MediaData(p, _)
            | Propagated::KeyframeRequest(p, _, _, _) => *p,
        }
    }
}

struct Client {
    pid: ParticipantId,
    call: CallId,
    out: mpsc::UnboundedSender<ServerMsg>,
    rtc: Rtc,
    /// The page's first offer is answered; the SFU may offer from now on.
    ready: bool,
    pending: Option<SdpPendingOffer>,
    tracks_in: Vec<Arc<TrackIn>>,
    tracks_out: Vec<TrackOut>,
}

impl Client {
    fn input(&mut self, input: Input) {
        if !self.rtc.is_alive() {
            return;
        }
        if let Err(e) = self.rtc.handle_input(input) {
            tracing::info!("p{}: {e}", self.pid);
            self.rtc.disconnect();
        }
    }

    fn fail(&mut self, code: &str) {
        let _ = self.out.send(ServerMsg::Error { code: code.into() });
        self.rtc.disconnect();
    }

    /// The page's offer. Only one is taken: the page sends its audio and
    /// video once and mutes by replacing tracks, so every later change is
    /// the SFU's to offer and there is no glare to resolve.
    fn offer(&mut self, sdp: &str, others: Vec<Weak<TrackIn>>) {
        if self.ready {
            return self.fail("renegotiation");
        }
        let Ok(offer) = SdpOffer::from_sdp_string(sdp) else {
            return self.fail("badSdp");
        };
        match self.rtc.sdp_api().accept_offer(offer) {
            Ok(answer) => {
                let _ = self.out.send(ServerMsg::Answer {
                    sdp: answer.to_sdp_string(),
                });
                self.ready = true;
                for t in others {
                    self.track_open(t);
                }
            }
            Err(e) => {
                tracing::info!("p{}: offer refused: {e}", self.pid);
                self.fail("badSdp");
            }
        }
    }

    fn answer(&mut self, sdp: &str) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let Ok(answer) = SdpAnswer::from_sdp_string(sdp) else {
            return self.fail("badSdp");
        };
        if let Err(e) = self.rtc.sdp_api().accept_answer(pending, answer) {
            tracing::info!("p{}: answer refused: {e}", self.pid);
            return self.fail("badSdp");
        }
        for t in &mut self.tracks_out {
            if let OutState::Negotiating(m) = t.state {
                t.state = OutState::Open(m);
            }
        }
        self.tracks_out
            .retain(|t| !matches!(t.state, OutState::NegotiatingStop(_)));
    }

    /// Offer the page the m-lines it is missing, if any and none is pending.
    fn negotiate(&mut self) {
        if !self.ready || self.pending.is_some() {
            return;
        }
        for t in &mut self.tracks_out {
            if let OutState::Open(m) = t.state {
                if t.track.upgrade().is_none() {
                    t.state = OutState::ToStop(m);
                }
            }
        }
        let mut change = self.rtc.sdp_api();
        for t in &mut self.tracks_out {
            match t.state {
                OutState::ToOpen => {
                    if let Some(track) = t.track.upgrade() {
                        let mid = change.add_media(
                            track.kind,
                            Direction::SendOnly,
                            Some(stream_id(track.origin)),
                            None,
                            None,
                        );
                        t.state = OutState::Negotiating(mid);
                    }
                }
                OutState::ToStop(mid) => {
                    change.stop_media(mid);
                    t.state = OutState::NegotiatingStop(mid);
                }
                _ => {}
            }
        }
        if !change.has_changes() {
            return;
        }
        if let Some((offer, pending)) = change.apply() {
            let _ = self.out.send(ServerMsg::Offer {
                sdp: offer.to_sdp_string(),
            });
            self.pending = Some(pending);
        }
    }

    fn poll(&mut self, sockets: &[Arc<UdpSocket>], queue: &mut VecDeque<Propagated>) -> Instant {
        self.negotiate();
        loop {
            if !self.rtc.is_alive() {
                return Instant::now();
            }
            let out = match self.rtc.poll_output() {
                Ok(o) => o,
                Err(e) => {
                    tracing::info!("p{}: {e}", self.pid);
                    self.rtc.disconnect();
                    return Instant::now();
                }
            };
            match out {
                Output::Timeout(t) => return t,
                Output::Transmit(t) => {
                    if let Some(s) = sockets.iter().find(|s| s.local_addr().ok() == Some(t.source)) {
                        // A full send buffer drops the datagram, as the
                        // network would.
                        let _ = s.try_send_to(&t.contents, t.destination);
                    }
                }
                Output::Event(e) => {
                    if let Some(p) = self.event(e) {
                        queue.push_back(p);
                    }
                }
            }
        }
    }

    fn event(&mut self, e: Event) -> Option<Propagated> {
        match e {
            Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                self.rtc.disconnect();
                None
            }
            Event::Connected => {
                tracing::info!("p{} connected", self.pid);
                None
            }
            Event::MediaAdded(m) => {
                // Only what the page sends comes in; the SFU's own send-only
                // m-lines are added through negotiate().
                if m.direction == Direction::RecvOnly || m.direction == Direction::SendRecv {
                    let t = Arc::new(TrackIn {
                        origin: self.pid,
                        mid: m.mid,
                        kind: m.kind,
                    });
                    let weak = Arc::downgrade(&t);
                    self.tracks_in.push(t);
                    Some(Propagated::TrackOpen(self.pid, weak))
                } else {
                    None
                }
            }
            Event::MediaData(data) => {
                if !data.contiguous {
                    if let Some(mut w) = self.rtc.writer(data.mid) {
                        let _ = w.request_keyframe(data.rid, KeyframeRequestKind::Pli);
                    }
                }
                Some(Propagated::MediaData(self.pid, Box::new(data)))
            }
            Event::KeyframeRequest(req) => {
                let out = self.tracks_out.iter().find(|t| t.mid() == Some(req.mid))?;
                let track = out.track.upgrade()?;
                Some(Propagated::KeyframeRequest(self.pid, req, track.origin, track.mid))
            }
            _ => None,
        }
    }

    fn track_open(&mut self, track: Weak<TrackIn>) {
        self.tracks_out.push(TrackOut {
            track,
            state: OutState::ToOpen,
        });
    }

    fn media_out(&mut self, origin: ParticipantId, data: &MediaData) {
        let Some(mid) = self
            .tracks_out
            .iter()
            .find(|o| {
                o.track
                    .upgrade()
                    .is_some_and(|t| t.origin == origin && t.mid == data.mid)
            })
            .and_then(|o| match o.state {
                OutState::Open(m) => Some(m),
                _ => None,
            })
        else {
            return;
        };
        let Some(w) = self.rtc.writer(mid) else {
            return;
        };
        let Some(pt) = w.match_params(data.params) else {
            return;
        };
        if let Err(e) = w.write(pt, data.network_time, data.time, data.data.clone()) {
            tracing::info!("p{}: write: {e}", self.pid);
            self.rtc.disconnect();
        }
    }

    fn keyframe_request(&mut self, req: KeyframeRequest, mid: Mid) {
        if let Some(mut w) = self.rtc.writer(mid) {
            let _ = w.request_keyframe(req.rid, req.kind);
        }
    }
}
