//! Who may start a call on this box's SFU, and who may join one.
//!
//! Pure bookkeeping, no sockets: the SFU loop owns one [`Calls`] and asks it
//! before it builds a peer connection. A call is started by a person of this
//! box (their Matrix id is on this box's homeserver), from a chat room, with
//! the room's members listed; only a listed person gets in. The SFU never
//! learns more than that: the media keys travel between the browsers over
//! Olm, never through here.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::time::{Duration, Instant};

/// Calls one box hosts at once. A mini PC forwards a few calls well; the cap
/// keeps a runaway client from opening thousands.
pub const MAX_CALLS: usize = 32;
/// People in one call. Every sender's frames go to every other member, so
/// the upstream of the box grows with the square of this.
pub const MAX_MEMBERS: usize = 8;
/// People a call may list as invited: a room's members.
pub const MAX_INVITED: usize = 100;
/// A call nobody joined is dropped after this.
pub const UNJOINED_TTL: Duration = Duration::from_secs(120);
/// A call everyone left is dropped after this, so a reload rejoins it.
pub const EMPTY_TTL: Duration = Duration::from_secs(30);

/// A Matrix user id, `@localpart:server`, checked for shape only.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(String);

impl UserId {
    pub fn parse(s: &str) -> Result<Self, Refusal> {
        let rest = s.strip_prefix('@').ok_or(Refusal::BadUser)?;
        let (local, server) = rest.split_once(':').ok_or(Refusal::BadUser)?;
        let local_ok = !local.is_empty()
            && local
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._=-/+".contains(&b));
        let server_ok = !server.is_empty()
            && server
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b));
        if !local_ok || !server_ok || s.len() > 255 {
            return Err(Refusal::BadUser);
        }
        Ok(UserId(s.to_owned()))
    }

    pub fn server(&self) -> &str {
        self.0.split_once(':').map(|(_, s)| s).unwrap_or_default()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 128 random bits as hex. Knowing it is not enough to join: the person
/// must also be listed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallId(pub String);

impl CallId {
    pub fn parse(s: &str) -> Result<Self, Refusal> {
        if s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            Ok(CallId(s.to_owned()))
        } else {
            Err(Refusal::NoSuchCall)
        }
    }
}

/// One connection to the SFU, numbered by the loop.
pub type ParticipantId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub user: UserId,
    /// The Matrix device the browser says it is. Unchecked: it only tells
    /// the other members which device to send their media key to, and a
    /// wrong one just gets no key.
    pub device: String,
}

#[derive(Debug)]
pub struct Call {
    pub room: String,
    pub creator: UserId,
    pub invited: BTreeSet<UserId>,
    pub video: bool,
    created: Instant,
    /// When the last member left, if the call is empty after someone joined.
    emptied: Option<Instant>,
    pub members: BTreeMap<ParticipantId, Member>,
}

/// Why the SFU said no. The code goes to the browser as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    BadUser,
    BadRoom,
    BadDevice,
    /// The person starting the call is not on this box's homeserver.
    NotLocal,
    TooManyCalls,
    TooManyInvited,
    NoSuchCall,
    NotInvited,
    Full,
    /// The OpenID token did not check out.
    Unauthenticated,
}

impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Refusal::BadUser => "badUser",
            Refusal::BadRoom => "badRoom",
            Refusal::BadDevice => "badDevice",
            Refusal::NotLocal => "notLocal",
            Refusal::TooManyCalls => "tooManyCalls",
            Refusal::TooManyInvited => "tooManyInvited",
            Refusal::NoSuchCall => "noSuchCall",
            Refusal::NotInvited => "notInvited",
            Refusal::Full => "full",
            Refusal::Unauthenticated => "unauthenticated",
        }
    }

    /// The HTTP status a refusal answers with.
    pub fn status(self) -> u16 {
        match self {
            Refusal::Unauthenticated => 401,
            Refusal::NotLocal | Refusal::NotInvited => 403,
            Refusal::NoSuchCall => 404,
            Refusal::Full | Refusal::TooManyCalls => 409,
            _ => 400,
        }
    }
}

/// A Matrix room id, `!opaque:server`.
fn room_ok(room: &str) -> bool {
    room.len() <= 255
        && room.starts_with('!')
        && room.contains(':')
        && room.bytes().all(|b| b.is_ascii_graphic())
}

/// A Matrix device id: what the homeserver hands out is short and printable.
fn device_ok(device: &str) -> bool {
    !device.is_empty()
        && device.len() <= 64
        && device.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
}

pub struct Calls {
    server_name: String,
    calls: HashMap<CallId, Call>,
}

impl Calls {
    /// `server_name` is this box's homeserver: only its people start calls.
    pub fn new(server_name: &str) -> Self {
        Calls {
            server_name: server_name.to_owned(),
            calls: HashMap::new(),
        }
    }

    pub fn get(&self, id: &CallId) -> Option<&Call> {
        self.calls.get(id)
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    /// Start a call. `invited` is the room's members as the caller's client
    /// sees them; the caller is added, duplicates fold.
    pub fn create(
        &mut self,
        id: CallId,
        creator: UserId,
        room: &str,
        invited: &[String],
        video: bool,
        now: Instant,
    ) -> Result<(), Refusal> {
        if creator.server() != self.server_name {
            return Err(Refusal::NotLocal);
        }
        if !room_ok(room) {
            return Err(Refusal::BadRoom);
        }
        if invited.len() > MAX_INVITED {
            return Err(Refusal::TooManyInvited);
        }
        let mut set = invited
            .iter()
            .map(|u| UserId::parse(u))
            .collect::<Result<BTreeSet<_>, _>>()?;
        set.insert(creator.clone());
        self.reap(now);
        if self.calls.len() >= MAX_CALLS {
            return Err(Refusal::TooManyCalls);
        }
        self.calls.insert(
            id,
            Call {
                room: room.to_owned(),
                creator,
                invited: set,
                video,
                created: now,
                emptied: None,
                members: BTreeMap::new(),
            },
        );
        Ok(())
    }

    /// Let `user` in as participant `pid`, or say why not.
    pub fn admit(
        &mut self,
        id: &CallId,
        pid: ParticipantId,
        user: UserId,
        device: &str,
        now: Instant,
    ) -> Result<&Call, Refusal> {
        self.reap(now);
        if !device_ok(device) {
            return Err(Refusal::BadDevice);
        }
        let call = self.calls.get_mut(id).ok_or(Refusal::NoSuchCall)?;
        if !call.invited.contains(&user) {
            return Err(Refusal::NotInvited);
        }
        if call.members.len() >= MAX_MEMBERS {
            return Err(Refusal::Full);
        }
        call.emptied = None;
        call.members.insert(
            pid,
            Member {
                user,
                device: device.to_owned(),
            },
        );
        Ok(call)
    }

    /// `pid` left. Returns the call it was in, if any.
    pub fn leave(&mut self, pid: ParticipantId, now: Instant) -> Option<CallId> {
        let (id, call) = self
            .calls
            .iter_mut()
            .find(|(_, c)| c.members.contains_key(&pid))?;
        call.members.remove(&pid);
        if call.members.is_empty() {
            call.emptied = Some(now);
        }
        Some(id.clone())
    }

    /// The call `pid` is in.
    pub fn call_of(&self, pid: ParticipantId) -> Option<(&CallId, &Call)> {
        self.calls.iter().find(|(_, c)| c.members.contains_key(&pid))
    }

    /// Drop calls nobody joined in time and calls that stayed empty.
    pub fn reap(&mut self, now: Instant) {
        self.calls.retain(|_, c| {
            if !c.members.is_empty() {
                return true;
            }
            match c.emptied {
                Some(t) => now.duration_since(t) < EMPTY_TTL,
                None => now.duration_since(c.created) < UNJOINED_TTL,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uid(s: &str) -> UserId {
        UserId::parse(s).unwrap()
    }

    fn cid(n: u8) -> CallId {
        CallId(format!("{n:032x}"))
    }

    const ROOM: &str = "!abc:mattbox.home.arpa";

    #[test]
    fn user_ids_are_checked_for_shape() {
        assert!(UserId::parse("@matus:mattbox.home.arpa").is_ok());
        assert!(UserId::parse("@eva:[::1]:8448").is_ok());
        for bad in ["matus:box", "@:box", "@matus", "@Matus:box", "@ma tus:box", "@m:"] {
            assert_eq!(UserId::parse(bad), Err(Refusal::BadUser), "{bad}");
        }
        assert_eq!(uid("@eva:kitchenbox.home.arpa").server(), "kitchenbox.home.arpa");
    }

    #[test]
    fn call_ids_are_32_lowercase_hex() {
        assert!(CallId::parse(&"a".repeat(32)).is_ok());
        assert!(CallId::parse(&"A".repeat(32)).is_err());
        assert!(CallId::parse("../../etc").is_err());
        assert!(CallId::parse(&"a".repeat(31)).is_err());
    }

    #[test]
    fn only_this_boxs_people_start_calls() {
        let mut calls = Calls::new("mattbox.home.arpa");
        let now = Instant::now();
        assert_eq!(
            calls.create(cid(1), uid("@eve:elsewhere.example"), ROOM, &[], true, now),
            Err(Refusal::NotLocal)
        );
        assert!(calls.is_empty());
        calls
            .create(cid(1), uid("@matus:mattbox.home.arpa"), ROOM, &[], true, now)
            .unwrap();
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn rooms_and_invites_are_checked() {
        let mut calls = Calls::new("b");
        let now = Instant::now();
        let me = uid("@m:b");
        assert_eq!(
            calls.create(cid(1), me.clone(), "#alias:b", &[], true, now),
            Err(Refusal::BadRoom)
        );
        assert_eq!(
            calls.create(cid(1), me.clone(), ROOM, &["not a user".into()], true, now),
            Err(Refusal::BadUser)
        );
        let many: Vec<String> = (0..=MAX_INVITED).map(|i| format!("@u{i}:b")).collect();
        assert_eq!(
            calls.create(cid(1), me, ROOM, &many, true, now),
            Err(Refusal::TooManyInvited)
        );
    }

    #[test]
    fn only_invited_people_get_in() {
        let mut calls = Calls::new("b");
        let now = Instant::now();
        calls
            .create(cid(1), uid("@m:b"), ROOM, &["@eva:b".into()], true, now)
            .unwrap();
        // The creator is invited without listing themself.
        assert!(calls.admit(&cid(1), 1, uid("@m:b"), "DEV1", now).is_ok());
        assert!(calls.admit(&cid(1), 2, uid("@eva:b"), "DEV2", now).is_ok());
        assert_eq!(
            calls.admit(&cid(1), 3, uid("@mallory:b"), "DEV3", now).err(),
            Some(Refusal::NotInvited)
        );
        assert_eq!(
            calls.admit(&cid(2), 3, uid("@eva:b"), "DEV3", now).err(),
            Some(Refusal::NoSuchCall)
        );
        assert_eq!(
            calls.admit(&cid(1), 3, uid("@eva:b"), "bad device!", now).err(),
            Some(Refusal::BadDevice)
        );
        assert_eq!(calls.get(&cid(1)).unwrap().members.len(), 2);
    }

    #[test]
    fn a_call_holds_at_most_max_members() {
        let mut calls = Calls::new("b");
        let now = Instant::now();
        calls.create(cid(1), uid("@m:b"), ROOM, &[], true, now).unwrap();
        for pid in 0..MAX_MEMBERS as u64 {
            calls.admit(&cid(1), pid, uid("@m:b"), "D", now).unwrap();
        }
        assert_eq!(
            calls.admit(&cid(1), 99, uid("@m:b"), "D", now).err(),
            Some(Refusal::Full)
        );
    }

    #[test]
    fn the_box_hosts_at_most_max_calls() {
        let mut calls = Calls::new("b");
        let now = Instant::now();
        for n in 0..MAX_CALLS as u8 {
            calls.create(cid(n), uid("@m:b"), ROOM, &[], true, now).unwrap();
        }
        assert_eq!(
            calls.create(cid(200), uid("@m:b"), ROOM, &[], true, now),
            Err(Refusal::TooManyCalls)
        );
    }

    #[test]
    fn unjoined_and_emptied_calls_are_reaped() {
        let mut calls = Calls::new("b");
        let t0 = Instant::now();
        calls.create(cid(1), uid("@m:b"), ROOM, &[], true, t0).unwrap();
        calls.create(cid(2), uid("@m:b"), ROOM, &[], true, t0).unwrap();
        calls.admit(&cid(2), 7, uid("@m:b"), "D", t0).unwrap();

        // Nobody joined call 1; call 2 is live.
        calls.reap(t0 + UNJOINED_TTL);
        assert!(calls.get(&cid(1)).is_none());
        assert!(calls.get(&cid(2)).is_some());

        // The last member leaves; a reload within EMPTY_TTL finds it.
        let t1 = t0 + UNJOINED_TTL;
        assert_eq!(calls.leave(7, t1), Some(cid(2)));
        assert_eq!(calls.leave(7, t1), None);
        calls.reap(t1 + EMPTY_TTL / 2);
        assert!(calls.get(&cid(2)).is_some());
        calls.admit(&cid(2), 8, uid("@m:b"), "D", t1 + EMPTY_TTL / 2).unwrap();
        calls.reap(t1 + EMPTY_TTL * 3);
        assert!(calls.get(&cid(2)).is_some(), "a member is back in");
        calls.leave(8, t1 + EMPTY_TTL * 3);
        calls.reap(t1 + EMPTY_TTL * 4);
        assert!(calls.get(&cid(2)).is_none());
    }

    #[test]
    fn call_of_finds_a_members_call() {
        let mut calls = Calls::new("b");
        let now = Instant::now();
        calls.create(cid(1), uid("@m:b"), ROOM, &[], false, now).unwrap();
        calls.admit(&cid(1), 4, uid("@m:b"), "D", now).unwrap();
        assert_eq!(calls.call_of(4).map(|(id, _)| id.clone()), Some(cid(1)));
        assert!(calls.call_of(5).is_none());
    }
}
