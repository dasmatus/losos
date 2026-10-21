//! The SFU's HTTP side: start a call, and the signalling websocket.
//!
//! nginx serves it under `/calls/api/` behind LosOS Login, on a socket only
//! nginx can open; the SFU then checks the person a second time through the
//! Matrix OpenID token, because the call is between Matrix people and the
//! homeserver is what knows them.
//!
//! * `POST /calls/api/calls` `{openid, room, invited, video}` → `{call}`
//! * `GET /calls/api/ws`, a websocket speaking [`super::wire`].
//! * `GET /calls/api/health` → `ok`.

use super::admit::{CallId, ParticipantId, Refusal};
use super::openid::Resolve;
use super::sfu::Command;
use super::wire::{ClientMsg, CreateCall, ServerMsg};
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
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/calls/api")
            .app_data(web::JsonConfig::default().limit(MAX_MESSAGE))
            .route("/health", web::get().to(|| async { "ok" }))
            .route("/calls", web::post().to(create))
            .route("/ws", web::get().to(ws)),
    );
}

fn refused(r: Refusal) -> HttpResponse {
    HttpResponse::build(actix_web::http::StatusCode::from_u16(r.status()).unwrap_or_default())
        .json(serde_json::json!({ "error": r.code() }))
}

fn new_call_id() -> Option<CallId> {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 16];
    ring::rand::SystemRandom::new().fill(&mut b).ok()?;
    Some(CallId(b.iter().map(|x| format!("{x:02x}")).collect()))
}

async fn create(state: web::Data<State>, body: web::Json<CreateCall>) -> HttpResponse {
    let body = body.into_inner();
    let resolve = state.resolve.clone();
    let token = body.openid.clone();
    let creator = match web::block(move || resolve.resolve(&token)).await {
        Ok(Ok(u)) => u,
        Ok(Err(r)) => return refused(r),
        Err(_) => return HttpResponse::InternalServerError().finish(),
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
    let (response, session, stream) = actix_ws::handle(&req, body)?;
    let stream = stream
        .max_frame_size(MAX_MESSAGE)
        .aggregate_continuations()
        .max_continuation_size(MAX_MESSAGE);
    actix_web::rt::spawn(signal(state.into_inner(), session, stream));
    Ok(response)
}

async fn send(session: &mut Session, msg: &ServerMsg) -> bool {
    match serde_json::to_string(msg) {
        Ok(s) => session.text(s).await.is_ok(),
        Err(_) => false,
    }
}

async fn next_text(stream: &mut actix_ws::AggregatedMessageStream, session: &mut Session) -> Option<String> {
    loop {
        match stream.recv().await? {
            Ok(AggregatedMessage::Text(t)) => return Some(t.to_string()),
            Ok(AggregatedMessage::Ping(p)) => {
                if session.pong(&p).await.is_err() {
                    return None;
                }
            }
            Ok(AggregatedMessage::Pong(_)) => {}
            Ok(AggregatedMessage::Binary(_)) | Ok(AggregatedMessage::Close(_)) | Err(_) => return None,
        }
    }
}

/// One page's signalling: a join, then SDP both ways until either side goes.
async fn signal(
    state: Arc<State>,
    mut session: Session,
    mut stream: actix_ws::AggregatedMessageStream,
) {
    let error = |code: &str| ServerMsg::Error { code: code.into() };
    let first = tokio::time::timeout(JOIN_WITHIN, next_text(&mut stream, &mut session)).await;
    let Ok(Some(first)) = first else {
        let _ = session.close(None).await;
        return;
    };
    let (call, openid, device) = match serde_json::from_str::<ClientMsg>(&first) {
        Ok(ClientMsg::Join {
            call,
            openid,
            device,
        }) => (call, openid, device),
        _ => {
            send(&mut session, &error("joinFirst")).await;
            let _ = session.close(None).await;
            return;
        }
    };
    let joined = async {
        let call = CallId::parse(&call)?;
        let resolve = state.resolve.clone();
        let user = web::block(move || resolve.resolve(&openid))
            .await
            .map_err(|_| Refusal::Unauthenticated)??;
        Ok::<_, Refusal>((call, user))
    }
    .await;
    let (call, user) = match joined {
        Ok(x) => x,
        Err(r) => {
            send(&mut session, &error(r.code())).await;
            let _ = session.close(None).await;
            return;
        }
    };

    let (out_tx, mut out_rx) = mpsc::unbounded_channel();
    let (reply, answer) = oneshot::channel();
    let cmd = Command::Join {
        call,
        user: user.clone(),
        device,
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
    tracing::info!("{user} joined as p{pid}");

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
                    Ok(ClientMsg::Leave) => break,
                    Ok(ClientMsg::Join { .. }) | Err(_) => {
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
    let _ = state.commands.send(Command::Leave { pid }).await;
    let _ = session.close(None).await;
}
