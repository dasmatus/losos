//! Podcast mode's way out: a member's recording of the call, piped through
//! ffmpeg to an RTMP address.
//!
//! The picture is put together in the streaming member's browser, from
//! frames that browser decrypted (`admin-ui/app/src/calls/podcast.ts`),
//! because a live stream is public and the box must not hold a key to the
//! call. Browsers cannot speak RTMP, so the browser sends its WebM
//! recording here over a websocket and ffmpeg turns it into H.264 and AAC
//! in FLV for the address. The box sees what is about to be public anyway
//! and nothing else.
//!
//! ```text
//!   page                                      SFU
//!   {call, pid, openid, rtmp}           ──►
//!                                       ◄──   {"type":"live"}
//!   WebM chunks, binary frames          ──►   ffmpeg stdin → rtmp
//!                                       ◄──   {"type":"ended","error"?}
//! ```
//!
//! The address carries the stream key, so it never goes into a log line:
//! ffmpeg's own errors are logged with the address cut out.

use super::admit::{CallId, Refusal};
use super::server::State;
use super::sfu::Command;
use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::{AggregatedMessage, Session};
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

/// One chunk of half a second at the bitrate the page records at fits in a
/// fraction of this; the first chunk carries the WebM header too.
pub const MAX_CHUNK: usize = 4 * 1024 * 1024;
const HELLO_WITHIN: Duration = Duration::from_secs(15);
/// ffmpeg gets this long to flush and close the RTMP connection.
const FLUSH_WITHIN: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Hello {
    call: String,
    pid: u64,
    openid: super::openid::OpenIdToken,
    rtmp: String,
}

/// An address ffmpeg is allowed to send to: `rtmp://` or `rtmps://`, a
/// host, a path, nothing ffmpeg would read as an option or a second URL.
pub fn valid_rtmp(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("rtmp://")
        .or_else(|| url.strip_prefix("rtmps://"))
    else {
        return false;
    };
    let Some((host, path)) = rest.split_once('/') else {
        return false;
    };
    url.len() <= 1024
        && !host.is_empty()
        && !path.is_empty()
        && url.bytes().all(|b| b.is_ascii_graphic())
        && !url.contains('|')
}

/// The ffmpeg command line: WebM from stdin, FLV to `rtmp`.
pub fn ffmpeg_args(rtmp: &str) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-fflags",
        "+genpts",
        "-thread_queue_size",
        "512",
        "-i",
        "pipe:0",
        "-map",
        "0:v:0",
        "-map",
        "0:a:0?",
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-tune",
        "zerolatency",
        "-pix_fmt",
        "yuv420p",
        "-r",
        "30",
        "-g",
        "60",
        "-b:v",
        "2500k",
        "-maxrate",
        "2500k",
        "-bufsize",
        "5000k",
        "-c:a",
        "aac",
        "-b:a",
        "128k",
        "-ar",
        "44100",
        "-ac",
        "2",
        "-f",
        "flv",
    ]
    .into_iter()
    .map(String::from)
    .chain([rtmp.to_owned()])
    .collect()
}

/// `line` with every copy of `secret` cut out.
fn redact(line: &str, secret: &str) -> String {
    line.replace(secret, "<address>")
}

pub async fn ws(
    state: web::Data<State>,
    req: HttpRequest,
    body: web::Payload,
) -> actix_web::Result<HttpResponse> {
    let (response, session, stream) = actix_ws::handle(&req, body)?;
    let stream = stream
        .max_frame_size(MAX_CHUNK)
        .aggregate_continuations()
        .max_continuation_size(MAX_CHUNK);
    actix_web::rt::spawn(serve(state.into_inner(), session, stream));
    Ok(response)
}

async fn say(session: &mut Session, v: serde_json::Value) {
    let _ = session.text(v.to_string()).await;
}

async fn end(mut session: Session, error: Option<&str>) {
    say(
        &mut session,
        serde_json::json!({ "type": "ended", "error": error }),
    )
    .await;
    let _ = session.close(None).await;
}

async fn serve(
    state: Arc<State>,
    mut session: Session,
    mut stream: actix_ws::AggregatedMessageStream,
) {
    let Some(ffmpeg) = state.ffmpeg.clone() else {
        return end(session, Some("noStreaming")).await;
    };
    let first = tokio::time::timeout(HELLO_WITHIN, stream.recv()).await;
    let Ok(Some(Ok(AggregatedMessage::Text(first)))) = first else {
        let _ = session.close(None).await;
        return;
    };
    let Ok(hello) = serde_json::from_str::<Hello>(&first) else {
        return end(session, Some("badMessage")).await;
    };
    if !valid_rtmp(&hello.rtmp) {
        return end(session, Some("badAddress")).await;
    }
    let checked = async {
        let call = CallId::parse(&hello.call)?;
        let resolve = state.resolve.clone();
        let openid = hello.openid.clone();
        let user = web::block(move || resolve.resolve(&openid))
            .await
            .map_err(|_| Refusal::Unauthenticated)??;
        Ok::<_, Refusal>((call, user))
    }
    .await;
    let (call, user) = match checked {
        Ok(x) => x,
        Err(r) => return end(session, Some(r.code())).await,
    };
    let broadcast = |on: bool| {
        let (reply, answer) = oneshot::channel();
        let cmd = Command::Broadcast {
            call: call.clone(),
            pid: hello.pid,
            user: user.clone(),
            on,
            reply,
        };
        let commands = state.commands.clone();
        async move {
            commands.send(cmd).await.map_err(|_| Refusal::NoSuchCall)?;
            answer.await.map_err(|_| Refusal::NoSuchCall)?
        }
    };
    if let Err(r) = broadcast(true).await {
        return end(session, Some(r.code())).await;
    }
    let error = pipe(&ffmpeg, &hello.rtmp, &mut session, &mut stream).await;
    let _ = broadcast(false).await;
    tracing::info!(
        "p{} stopped streaming call {}{}",
        hello.pid,
        call.0,
        error.map(|e| format!(": {e}")).unwrap_or_default()
    );
    end(session, error).await;
}

/// Run ffmpeg and feed it the page's chunks until either side stops.
/// Returns why it ended, if not because the page said so.
async fn pipe(
    ffmpeg: &Path,
    rtmp: &str,
    session: &mut Session,
    stream: &mut actix_ws::AggregatedMessageStream,
) -> Option<&'static str> {
    let child = tokio::process::Command::new(ffmpeg)
        .args(ffmpeg_args(rtmp))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("cannot start ffmpeg: {e}");
            return Some("noStreaming");
        }
    };
    let (Some(mut stdin), Some(stderr)) = (child.stdin.take(), child.stderr.take()) else {
        return Some("noStreaming");
    };
    let secret = rtmp.to_owned();
    actix_web::rt::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::warn!("ffmpeg: {}", redact(&line, &secret));
        }
    });
    say(session, serde_json::json!({ "type": "live" })).await;

    let error = loop {
        tokio::select! {
            status = child.wait() => {
                break match status {
                    Ok(s) => {
                        tracing::warn!("ffmpeg ended early: {s}");
                        Some("streamFailed")
                    }
                    Err(_) => Some("streamFailed"),
                };
            }
            msg = stream.recv() => match msg {
                Some(Ok(AggregatedMessage::Binary(chunk))) => {
                    if stdin.write_all(&chunk).await.is_err() {
                        break Some("streamFailed");
                    }
                }
                Some(Ok(AggregatedMessage::Ping(p))) => {
                    if session.pong(&p).await.is_err() {
                        break None;
                    }
                }
                Some(Ok(AggregatedMessage::Pong(_))) => {}
                // A text frame is the page saying stop; a close or a
                // dropped socket ends the stream the same way.
                Some(Ok(AggregatedMessage::Text(_)))
                | Some(Ok(AggregatedMessage::Close(_)))
                | Some(Err(_))
                | None => break None,
            },
        }
    };
    // End of input: ffmpeg writes what it has and closes the connection.
    drop(stdin);
    if tokio::time::timeout(FLUSH_WITHIN, child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_rtmp_addresses_are_streamed_to() {
        assert!(valid_rtmp("rtmp://a.rtmp.youtube.com/live2/abcd-efgh"));
        assert!(valid_rtmp("rtmps://live.example:443/app/key?x=1"));
        assert!(!valid_rtmp("rtmp://host"));
        assert!(!valid_rtmp("rtmp:///key"));
        assert!(!valid_rtmp("http://host/live"));
        assert!(!valid_rtmp("file:///etc/passwd"));
        assert!(!valid_rtmp("rtmp://host/live key"));
        assert!(!valid_rtmp("rtmp://host/a|tee"));
        assert!(!valid_rtmp("-i rtmp://host/a"));
        assert!(!valid_rtmp(&format!("rtmp://host/{}", "k".repeat(1100))));
    }

    #[test]
    fn the_address_is_the_last_argument_and_cut_from_logs() {
        let args = ffmpeg_args("rtmp://h/live/secret");
        assert_eq!(
            args.last().map(String::as_str),
            Some("rtmp://h/live/secret")
        );
        assert_eq!(args.iter().filter(|a| a.contains("secret")).count(), 1);
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "pipe:0"));
        assert_eq!(
            redact(
                "rtmp://h/live/secret: Connection refused",
                "rtmp://h/live/secret"
            ),
            "<address>: Connection refused"
        );
    }
}
