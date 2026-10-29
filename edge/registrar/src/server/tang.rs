//! The Tang routes (`crate::tang`), on the API's guard like the rest:
//!
//! * `POST /tang/keys` `{claim}`: a key pair for a new box; `{id, thumbprint}`.
//! * `GET /tang/<id>/adv[/<kid>]`, `POST /tang/<id>/rec/<kid>`: Tang itself,
//!   which is what Clevis on the box asks at every boot.
//! * `POST /tang/<id>/owner` `{claim, lost}`: the box names the hash of its
//!   owner's lost-box secret.
//! * `GET /tang/lost`: the owner's page; `POST /tang/lost` and
//!   `POST /tang/found` `{secret}`: its two buttons.
//!
//! All answer 404 on an edge started without `--tang-dir`.

use std::sync::{Arc, LazyLock};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use serde::Deserialize;

use super::provider::Lang;
use super::AppState;
use crate::tang::{Request, Tang, TangError};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/tang/keys", post(create))
        .route("/tang/lost", get(page).post(lost))
        .route("/tang/found", post(found))
        .route("/tang/{id}/adv", get(adv))
        .route("/tang/{id}/adv/{kid}", get(adv_kid))
        .route("/tang/{id}/rec/{kid}", post(rec))
        .route("/tang/{id}/owner", post(owner))
}

impl IntoResponse for TangError {
    fn into_response(self) -> Response {
        let status = match &self {
            TangError::Unconfigured | TangError::NotFound => StatusCode::NOT_FOUND,
            TangError::Invalid(_) => StatusCode::BAD_REQUEST,
            TangError::Forbidden | TangError::Lost => StatusCode::FORBIDDEN,
            TangError::Full(_) => StatusCode::TOO_MANY_REQUESTS,
            TangError::Tangd(_) | TangError::Io(_) => {
                tracing::error!(target: "tang", "Tang request failed: {self}");
                return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
            }
        };
        (status, self.to_string()).into_response()
    }
}

fn tang(st: &AppState) -> Result<&Arc<Tang>, TangError> {
    st.tang.as_ref().ok_or(TangError::Unconfigured)
}

#[derive(Deserialize)]
struct CreateReq {
    claim: String,
}

async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreateReq>,
) -> Result<Response, TangError> {
    let (id, thumbprint) = tang(&st)?.create(&req.claim).await?;
    tracing::info!(target: "tang", "made a Tang key for box {id}");
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "thumbprint": thumbprint })),
    )
        .into_response())
}

fn answer(a: crate::tang::Answer) -> Response {
    let status = StatusCode::from_u16(a.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut resp = (status, a.body).into_response();
    if let Some(t) = a.content_type.and_then(|t| HeaderValue::from_str(&t).ok()) {
        resp.headers_mut().insert(header::CONTENT_TYPE, t);
    }
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

async fn adv(State(st): State<AppState>, Path(id): Path<String>) -> Result<Response, TangError> {
    Ok(answer(tang(&st)?.serve(&id, Request::Adv(None)).await?))
}

async fn adv_kid(
    State(st): State<AppState>,
    Path((id, kid)): Path<(String, String)>,
) -> Result<Response, TangError> {
    Ok(answer(
        tang(&st)?.serve(&id, Request::Adv(Some(kid))).await?,
    ))
}

async fn rec(
    State(st): State<AppState>,
    Path((id, kid)): Path<(String, String)>,
    body: Bytes,
) -> Result<Response, TangError> {
    Ok(answer(
        tang(&st)?
            .serve(&id, Request::Rec(kid, body.to_vec()))
            .await?,
    ))
}

#[derive(Deserialize)]
struct OwnerReq {
    claim: String,
    lost: String,
}

async fn owner(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<OwnerReq>,
) -> Result<StatusCode, TangError> {
    tang(&st)?.set_owner(&id, &req.claim, &req.lost).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SwitchReq {
    secret: String,
}

async fn lost(
    State(st): State<AppState>,
    Json(req): Json<SwitchReq>,
) -> Result<StatusCode, TangError> {
    let id = tang(&st)?.mark(&req.secret, true).await?;
    tracing::warn!(target: "tang", "box {id} reported stolen: its Tang key answers no one");
    Ok(StatusCode::NO_CONTENT)
}

async fn found(
    State(st): State<AppState>,
    Json(req): Json<SwitchReq>,
) -> Result<StatusCode, TangError> {
    let id = tang(&st)?.mark(&req.secret, false).await?;
    tracing::info!(target: "tang", "box {id} reported back with its owner");
    Ok(StatusCode::NO_CONTENT)
}

/// The page's script. The same for every language: the words come from the
/// page's `data-*` attributes, so one hash covers it in the CSP.
const SCRIPT: &str = r#"
const form = document.querySelector("form");
const say = document.getElementById("said");
async function secret(code) {
  const text = "losos-tang-lost\n" + code.trim().toLowerCase();
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(d), (b) => b.toString(16).padStart(2, "0")).join("");
}
form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const which = e.submitter && e.submitter.value === "found" ? "found" : "lost";
  const code = form.elements.code.value;
  if (!code.trim()) return;
  for (const b of form.querySelectorAll("button")) b.disabled = true;
  say.textContent = "";
  try {
    const r = await fetch("/tang/" + which, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ secret: await secret(code) }),
    });
    const key = r.ok ? which : r.status === 404 ? "unknown" : "failed";
    say.textContent = say.dataset[key];
    say.dataset.state = r.ok ? "ok" : "bad";
  } catch {
    say.textContent = say.dataset.failed;
    say.dataset.state = "bad";
  }
  for (const b of form.querySelectorAll("button")) b.disabled = false;
});
"#;

static PAGE_CSP: LazyLock<String> = LazyLock::new(|| {
    let d = ring::digest::digest(&ring::digest::SHA256, SCRIPT.as_bytes());
    format!(
        "default-src 'none'; style-src 'unsafe-inline'; script-src 'sha256-{}'; connect-src 'self'; form-action 'none'; base-uri 'none'; frame-ancestors 'none'",
        base64::engine::general_purpose::STANDARD.encode(d.as_ref())
    )
});

struct Words {
    title: &'static str,
    lead: &'static str,
    why: &'static str,
    label: &'static str,
    lost: &'static str,
    found: &'static str,
    said_lost: &'static str,
    said_found: &'static str,
    said_unknown: &'static str,
    said_failed: &'static str,
}

fn words(lang: Lang) -> Words {
    match lang {
        Lang::En => Words {
            title: "Report a stolen box",
            lead: "If your LosOS box was taken, this edge can stop helping it unlock its disk. Type the box's recovery code. It stays in this browser: the page sends the edge only a value made from it.",
            why: "This works for a box installed to unlock with this edge. Until you take the report back, the box does not open, at home included. The recovery code still opens it at the box's own screen.",
            label: "Recovery code",
            lost: "My box was stolen",
            found: "I have it back",
            said_lost: "Done. This edge will not unlock that box until you take the report back.",
            said_found: "Done. The box unlocks with this edge again.",
            said_unknown: "No box on this edge uses that recovery code.",
            said_failed: "That did not work. Try again in a minute.",
        },
        Lang::Sk => Words {
            title: "Nahlásiť ukradnuté zariadenie",
            lead: "Ak vám niekto zobral zariadenie LosOS, tento edge mu môže prestať pomáhať pri odomknutí disku. Zadajte kód na obnovu zariadenia. Zostane v tomto prehliadači: stránka pošle edgeu iba hodnotu, ktorá sa z neho vypočíta.",
            why: "Funguje to pre zariadenie nainštalované tak, aby sa odomykalo cez tento edge. Kým hlásenie nezrušíte, zariadenie sa neotvorí, ani doma. Na vlastnej obrazovke zariadenia ho kód na obnovu stále otvorí.",
            label: "Kód na obnovu",
            lost: "Zariadenie mi ukradli",
            found: "Zariadenie mám späť",
            said_lost: "Hotovo. Tento edge zariadenie neodomkne, kým hlásenie nezrušíte.",
            said_found: "Hotovo. Zariadenie sa cez tento edge znova odomyká.",
            said_unknown: "Žiadne zariadenie na tomto edgei nepoužíva tento kód na obnovu.",
            said_failed: "Nepodarilo sa. Skúste to znova o minútu.",
        },
        Lang::De => Words {
            title: "Gestohlene Box melden",
            lead: "Wurde deine LosOS-Box mitgenommen, kann diese Edge aufhören, ihr beim Entsperren der Festplatte zu helfen. Gib den Wiederherstellungscode der Box ein. Er bleibt in diesem Browser: Die Seite schickt der Edge nur einen Wert, der daraus berechnet wird.",
            why: "Das gilt für eine Box, die so installiert wurde, dass sie mit dieser Edge entsperrt. Bis du die Meldung zurücknimmst, öffnet sich die Box nicht, auch nicht zu Hause. Am eigenen Bildschirm der Box öffnet sie der Wiederherstellungscode weiterhin.",
            label: "Wiederherstellungscode",
            lost: "Meine Box wurde gestohlen",
            found: "Ich habe sie zurück",
            said_lost: "Erledigt. Diese Edge entsperrt die Box nicht, bis du die Meldung zurücknimmst.",
            said_found: "Erledigt. Die Box entsperrt wieder mit dieser Edge.",
            said_unknown: "Keine Box auf dieser Edge nutzt diesen Wiederherstellungscode.",
            said_failed: "Das hat nicht geklappt. Versuch es in einer Minute noch einmal.",
        },
    }
}

async fn page(headers: HeaderMap) -> Response {
    let lang = Lang::from_headers(&headers);
    let w = words(lang);
    let html = format!(
        r#"<!doctype html>
<html lang="{code}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; --ground: #f3f5f7; --surface: #ffffff; --ink: #18212a; --muted: #5c6a77; --line: #dce1e6; --accent: #0e6e7d; --accent-ink: #ffffff; --bad: #a3352b; }}
@media (prefers-color-scheme: dark) {{ :root {{ --ground: #0a0e12; --surface: #141b22; --ink: #e2e8ee; --muted: #94a2b0; --line: #212c36; --accent: #48b3c0; --accent-ink: #0a0e12; --bad: #f08a7e; }} }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 16px; background: var(--ground); color: var(--ink); font: 16px/1.55 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }}
main {{ width: 100%; max-width: 30rem; background: var(--surface); border: 1px solid var(--line); border-radius: 14px; padding: 32px 28px; }}
svg {{ display: block; margin-bottom: 20px; color: var(--accent); }}
h1 {{ font-size: 1.3rem; line-height: 1.3; font-weight: 600; margin: 0 0 12px; }}
p {{ margin: 0 0 12px; }}
.why {{ color: var(--muted); }}
label {{ display: block; margin: 20px 0 6px; font-weight: 500; }}
input {{ width: 100%; padding: 9px 12px; border: 1px solid var(--line); border-radius: 9px; background: var(--ground); color: var(--ink); font: 15px ui-monospace, SFMono-Regular, Menlo, monospace; }}
.row {{ display: flex; flex-wrap: wrap; gap: 8px; margin-top: 14px; }}
button {{ padding: 9px 16px; border-radius: 9px; border: 1px solid var(--accent); background: var(--accent); color: var(--accent-ink); font: inherit; font-weight: 500; cursor: pointer; }}
button[value="found"] {{ background: transparent; color: var(--accent); }}
button:disabled {{ opacity: 0.6; cursor: default; }}
:focus-visible {{ outline: 2px solid var(--accent); outline-offset: 3px; }}
#said {{ min-height: 1.55em; margin-top: 14px; }}
#said[data-state="bad"] {{ color: var(--bad); }}
footer {{ margin-top: 24px; font-size: 0.8rem; color: var(--muted); }}
</style>
</head>
<body>
<main>
<svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/><path d="M12 15v2"/></svg>
<h1>{title}</h1>
<p>{lead}</p>
<p class="why">{why}</p>
<form>
<label for="code">{label}</label>
<input id="code" name="code" autocomplete="off" spellcheck="false" placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" required>
<div class="row">
<button type="submit" value="lost">{lost}</button>
<button type="submit" value="found">{found}</button>
</div>
</form>
<p id="said" role="status" data-lost="{said_lost}" data-found="{said_found}" data-unknown="{said_unknown}" data-failed="{said_failed}"></p>
<footer>LosOS edge</footer>
</main>
<script>{script}</script>
</body>
</html>
"#,
        code = lang.code(),
        title = w.title,
        lead = w.lead,
        why = w.why,
        label = w.label,
        lost = w.lost,
        found = w.found,
        said_lost = w.said_lost,
        said_found = w.said_found,
        said_unknown = w.said_unknown,
        said_failed = w.said_failed,
        script = SCRIPT,
    );
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_str(&PAGE_CSP).expect("ASCII policy"),
            ),
            (
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            ),
        ],
        html,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_names_the_script_and_nothing_else_runs() {
        let script = PAGE_CSP
            .split("; ")
            .find(|d| d.starts_with("script-src "))
            .unwrap();
        assert!(script.starts_with("script-src 'sha256-"), "{script}");
        assert!(!script.contains("unsafe"), "{script}");
        assert!(PAGE_CSP.contains("connect-src 'self'"));
    }

    #[test]
    fn no_words_break_out_of_their_attribute() {
        for lang in [Lang::En, Lang::Sk, Lang::De] {
            let w = words(lang);
            for s in [
                w.said_lost,
                w.said_found,
                w.said_unknown,
                w.said_failed,
                w.title,
                w.lead,
                w.why,
            ] {
                assert!(
                    !s.contains('"') && !s.contains('<') && !s.contains('&'),
                    "{s}"
                );
            }
        }
    }
}
