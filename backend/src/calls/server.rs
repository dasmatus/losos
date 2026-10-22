//! The SFU's HTTP side: start a call, and the signalling websocket.
//!
//! nginx serves it under `/calls/api/` behind LosOS Login, on a socket only
//! nginx can open, and names the person it signed in as `Remote-User`
//! ([`super::person`]).
//!
//! * `POST /calls/api/calls` `{room, invited, video}` → `{call}`
//! * `GET /calls/api/ws`, a websocket speaking [`super::wire`].
//! * `GET /calls/api/stream`, podcast mode's websocket ([`super::stream`]).
//! * `POST /calls/api/invites` `{call, pid}` → `{url}`: a guest
//!   link, made by the box's official edge ([`super::guests`]).
//! * `GET /calls/guest/ws`, a guest's websocket: no LosOS Login in front,
//!   the invite is the way in. nginx serves it to the internet.
//! * `GET /calls/api/health` → `ok`.

use super::admit::{CallId, ParticipantId, Refusal, UserId};
use super::person::{remote_user, Resolve};
use super::sfu::{Command, Who};
use super::wire::{ClientMsg, CreateCall, CreateInvite, ServerMsg};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::{AggregatedMessage, Session};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// A signalling message carries one SDP; a few dozen m-lines fit easily.
pub const MAX_MESSAGE: usize = 64 * 1024;
/// A socket that has not joined by now is closed.
const JOIN_WITHIN: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct State {
    pub commands: mpsc::Sender<Command>,
    pub resolve: Arc<dyn Resolve>,
    /// The ffmpeg podcast mode streams through; none turns streaming off.
    pub ffmpeg: Option<std::path::PathBuf>,
    /// The edge that makes guest links; none, and there are no guests.
    pub edge: Option<Arc<super::guests::Edge>>,
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/calls/api")
            .app_data(web::JsonConfig::default().limit(MAX_MESSAGE))
            .route("/health", web::get().to(|| async { "ok" }))
            .route("/calls", web::post().to(create))
            .route("/ws", web::get().to(ws))
            .route("/stream", web::get().to(super::stream::ws))
            .route("/invites", web::post().to(invite)),
    );
    cfg.service(web::scope("/calls/guest").route("/ws", web::get().to(guest_ws)));
}

fn refused(r: Refusal) -> HttpResponse {
    HttpResponse::build(actix_web::http::StatusCode::from_u16(r.status()).unwrap_or_default())
        .json(serde_json::json!({ "error": r.code() }))
}

/// 128 random bits as lowercase hex: a call id, or an invite.
fn random_hex() -> Option<String> {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 16];
    ring::rand::SystemRandom::new().fill(&mut b).ok()?;
    Some(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn new_call_id() -> Option<CallId> {
    random_hex().map(CallId)
}

/// A guest link for a call: the SFU opens the call to the invite, then
/// asks the edge for the link.
async fn invite(
    state: web::Data<State>,
    req: HttpRequest,
    body: web::Json<CreateInvite>,
) -> HttpResponse {
    let body = body.into_inner();
    let Some(edge) = state.edge.clone() else {
        return refused(Refusal::NoEdge);
    };
    let call = match CallId::parse(&body.call) {
        Ok(c) => c,
        Err(r) => return refused(r),
    };
    let user = match state.resolve.resolve(remote_user(&req)) {
        Ok(u) => u,
        Err(r) => return refused(r),
    };
    let Some(invite) = random_hex() else {
        return HttpResponse::InternalServerError().finish();
    };
    let (reply, answer) = oneshot::channel();
    let cmd = Command::Invite {
        call: call.clone(),
        pid: body.pid,
        user: user.clone(),
        invite: invite.clone(),
        reply,
    };
    if state.commands.send(cmd).await.is_err() {
        return HttpResponse::ServiceUnavailable().finish();
    }
    match answer.await {
        Ok(Ok(())) => {}
        Ok(Err(r)) => return refused(r),
        Err(_) => return HttpResponse::ServiceUnavailable().finish(),
    }
    match edge.link(&call.0, &invite).await {
        Ok(url) => {
            tracing::info!("{user} made a guest link for call {}", call.0);
            HttpResponse::Ok().json(serde_json::json!({ "url": url }))
        }
        Err(e) => {
            tracing::warn!("the edge made no guest link: {e}");
            refused(Refusal::NoEdge)
        }
    }
}

async fn create(
    state: web::Data<State>,
    req: HttpRequest,
    body: web::Json<CreateCall>,
) -> HttpResponse {
    let body = body.into_inner();
    let creator = match state.resolve.resolve(remote_user(&req)) {
        Ok(u) => u,
        Err(r) => return refused(r),
    };
    let Some(id) = new_call_id() else {
        return HttpResponse::InternalServerError().finish();
    };
    let (reply, answer) = oneshot::channel();
    let cmd = Command::Create {
        id: id.clone(),
        creator: creator.clone(),
        room: body.room,
        invited: body.invited,
        video: body.video,
        reply,
    };
    if state.commands.send(cmd).await.is_err() {
        return HttpResponse::ServiceUnavailable().finish();
    }
    match answer.await {
        Ok(Ok(())) => {
            tracing::info!("{creator} started call {}", id.0);
            HttpResponse::Ok().json(serde_json::json!({ "call": id.0 }))
        }
        Ok(Err(r)) => refused(r),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

async fn ws(
    state: web::Data<State>,
    req: HttpRequest,
    body: web::Payload,
) -> actix_web::Result<HttpResponse> {
    open(state, req, body, false)
}

async fn guest_ws(
    state: web::Data<State>,
    req: HttpRequest,
    body: web::Payload,
) -> actix_web::Result<HttpResponse> {
    open(state, req, body, true)
}

fn open(
    state: web::Data<State>,
    req: HttpRequest,
    body: web::Payload,
    guest: bool,
) -> actix_web::Result<HttpResponse> {
    // A guest's socket gets no person from nginx, and takes none.
    let user = (!guest).then(|| state.resolve.resolve(remote_user(&req)));
    let (response, session, stream) = actix_ws::handle(&req, body)?;
    let stream = stream
        .max_frame_size(MAX_MESSAGE)
        .aggregate_continuations()
        .max_continuation_size(MAX_MESSAGE);
    actix_web::rt::spawn(signal(state.into_inner(), session, stream, user));
    Ok(response)
}

async fn send(session: &mut Session, msg: &ServerMsg) -> bool {
    match serde_json::to_string(msg) {
        Ok(s) => session.text(s).await.is_ok(),
        Err(_) => false,
    }
}

async fn next_text(
    stream: &mut actix_ws::AggregatedMessageStream,
    session: &mut Session,
) -> Option<String> {
    loop {
        match stream.recv().await? {
            Ok(AggregatedMessage::Text(t)) => return Some(t.to_string()),
            Ok(AggregatedMessage::Ping(p)) => {
                if session.pong(&p).await.is_err() {
                    return None;
                }
            }
            Ok(AggregatedMessage::Pong(_)) => {}
            Ok(AggregatedMessage::Binary(_)) | Ok(AggregatedMessage::Close(_)) | Err(_) => {
                return None
            }
        }
    }
}

/// One page's signalling: a join, then SDP both ways until either side
/// goes. On a guest's socket the join is a `guest` message, and nothing
/// else gets in.
async fn signal(
    state: Arc<State>,
    mut session: Session,
    mut stream: actix_ws::AggregatedMessageStream,
    user: Option<Result<UserId, Refusal>>,
) {
    let error = |code: &str| ServerMsg::Error { code: code.into() };
    let first = tokio::time::timeout(JOIN_WITHIN, next_text(&mut stream, &mut session)).await;
    let Ok(Some(first)) = first else {
        let _ = session.close(None).await;
        return;
    };
    let joined = async {
        match serde_json::from_str::<ClientMsg>(&first) {
            Ok(ClientMsg::Join { call, device }) if user.is_some() => {
                let call = CallId::parse(&call)?;
                let user = user.unwrap_or(Err(Refusal::Unauthenticated))?;
                Ok((call, Who::Member { user, device }))
            }
            Ok(ClientMsg::Guest { call, invite, name }) if user.is_none() => {
                Ok((CallId::parse(&call)?, Who::Guest { invite, name }))
            }
            _ => Err(Refusal::NotInvited),
        }
    }
    .await;
    let (call, who) = match joined {
        Ok(x) => x,
        Err(r) => {
            send(&mut session, &error(r.code())).await;
            let _ = session.close(None).await;
            return;
        }
    };

    let who_name = match &who {
        Who::Member { user, .. } => user.to_string(),
        Who::Guest { name, .. } => format!("guest {name:?}"),
    };
    let (out_tx, mut out_rx) = mpsc::unbounded_channel();
    let (reply, answer) = oneshot::channel();
    let cmd = Command::Join {
        call,
        who,
        out: out_tx,
        reply,
    };
    if state.commands.send(cmd).await.is_err() {
        let _ = session.close(None).await;
        return;
    }
    let pid: ParticipantId = match answer.await {
        Ok(Ok(pid)) => pid,
        Ok(Err(r)) => {
            send(&mut session, &error(r.code())).await;
            let _ = session.close(None).await;
            return;
        }
        Err(_) => {
            let _ = session.close(None).await;
            return;
        }
    };
    tracing::info!("{who_name} joined as p{pid}");

    let mut leaving = false;
    loop {
        tokio::select! {
            msg = out_rx.recv() => match msg {
                Some(m) => {
                    let fatal = matches!(m, ServerMsg::Error { .. });
                    if !send(&mut session, &m).await || fatal {
                        break;
                    }
                }
                None => break,
            },
            text = next_text(&mut stream, &mut session) => {
                let Some(text) = text else { break };
                let cmd = match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(ClientMsg::Offer { sdp }) => Command::Offer { pid, sdp },
                    Ok(ClientMsg::Answer { sdp }) => Command::Answer { pid, sdp },
                    Ok(ClientMsg::Relay { to, body }) => Command::Relay { from: pid, to, body },
                    Ok(ClientMsg::Screen { on }) => Command::Screen { pid, on },
                    Ok(ClientMsg::Leave) => {
                        leaving = true;
                        break;
                    }
                    Ok(ClientMsg::Join { .. }) | Ok(ClientMsg::Guest { .. }) | Err(_) => {
                        send(&mut session, &error("badMessage")).await;
                        break;
                    }
                };
                if state.commands.send(cmd).await.is_err() {
                    break;
                }
            }
        }
    }
    if !leaving {
        let _ = state
            .commands
            .send(Command::Leave { pid, reply: None })
            .await;
        let _ = session.close(None).await;
        return;
    }
    let (reply, last) = oneshot::channel();
    let asked = state.commands.send(Command::Leave {
        pid,
        reply: Some(reply),
    });
    if asked.await.is_ok() {
        if let Ok(last) = last.await {
            send(&mut session, &ServerMsg::Left { last }).await;
        }
    }
    let _ = session.close(None).await;
}
