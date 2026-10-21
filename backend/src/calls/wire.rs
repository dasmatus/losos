//! The call page's messages to the SFU and back, one JSON object per
//! websocket text frame.
//!
//! ```text
//!   page                                   SFU
//!   join {call, openid, device}      ──►
//!                                    ◄──   joined {you, video, members}
//!   offer {sdp}  (audio, camera, screen) ──►
//!                                    ◄──   answer {sdp}
//!                                    ◄──   offer {sdp}  (someone's tracks)
//!   answer {sdp}                     ──►
//!                                    ◄──   members {members, live}  (on every join/leave/stream)
//!   relay {to, body}                 ──►
//!                                    ◄──   relay {from, body}  (guest key agreement)
//! ```
//!
//! A guest's socket is `/calls/guest/ws` and starts with
//! `guest {call, invite, name}` instead of `join`; the rest is the same.
//!
//! A member's tracks reach the others with the stream id `p<pid>`, so a
//! page knows whose frames it is decrypting from `members`. The second
//! video m-line a page offers is its screen, which goes out as
//! `p<pid>-screen`: the screen is a track of its own, so the camera stays
//! on while someone shares.

use super::admit::ParticipantId;
use super::openid::OpenIdToken;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ClientMsg {
    Join {
        call: String,
        openid: OpenIdToken,
        device: String,
    },
    /// A guest's first message, on `/calls/guest/ws`.
    Guest {
        call: String,
        invite: String,
        name: String,
    },
    /// A message for one other participant, passed on as it is: how a
    /// guest, who has no Olm, and the members agree on keys
    /// (`admin-ui/app/src/calls/guests.ts`).
    Relay {
        to: ParticipantId,
        body: serde_json::Value,
    },
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    Leave,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemberInfo {
    pub pid: ParticipantId,
    pub user: String,
    pub device: String,
    /// A guest's name; absent for the room's own people.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMsg {
    Joined {
        you: ParticipantId,
        video: bool,
        members: Vec<MemberInfo>,
    },
    Members {
        members: Vec<MemberInfo>,
        /// Who streams the call out in podcast mode, if anyone: every page
        /// shows that the call is public then.
        live: Option<ParticipantId>,
    },
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    Relay {
        from: ParticipantId,
        body: serde_json::Value,
    },
    Error {
        code: String,
    },
}

/// The body of `POST /calls/api/invites`: a member asks for a guest link.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateInvite {
    pub openid: OpenIdToken,
    pub call: String,
    pub pid: ParticipantId,
}

/// The body of `POST /calls/api/calls`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCall {
    pub openid: OpenIdToken,
    pub room: String,
    pub invited: Vec<String>,
    pub video: bool,
}

/// The stream id a member's tracks carry to the others.
pub fn stream_id(pid: ParticipantId) -> String {
    format!("p{pid}")
}

/// The stream id of a member's screen.
pub fn screen_stream_id(pid: ParticipantId) -> String {
    format!("p{pid}-screen")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_have_the_documented_shape() {
        let m: ClientMsg = serde_json::from_str(
            r#"{"type":"join","call":"c","openid":{"access_token":"t","matrix_server_name":"b"},"device":"D"}"#,
        )
        .unwrap();
        assert!(matches!(m, ClientMsg::Join { ref device, .. } if device == "D"));
        let m: ClientMsg = serde_json::from_str(r#"{"type":"leave"}"#).unwrap();
        assert!(matches!(m, ClientMsg::Leave));
        assert!(
            serde_json::from_str::<ClientMsg>(r#"{"type":"offer","sdp":"x","more":1}"#).is_err()
        );

        let s = serde_json::to_string(&ServerMsg::Joined {
            you: 3,
            video: true,
            members: vec![MemberInfo {
                guest: None,
                pid: 3,
                user: "@m:b".into(),
                device: "D".into(),
            }],
        })
        .unwrap();
        assert_eq!(
            s,
            r#"{"type":"joined","you":3,"video":true,"members":[{"pid":3,"user":"@m:b","device":"D"}]}"#
        );
        assert_eq!(stream_id(12), "p12");
        assert_eq!(screen_stream_id(12), "p12-screen");
    }
}
