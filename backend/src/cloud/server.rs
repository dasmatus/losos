//! One listener for every way into the cloud.
//!
//! nginx sends here `/remote.php/`, `/status.php`, `/ocs/`, `/index.php/`
//! and `/cloud/api/` (`modules/cloud.nix`). On the box the listener is a
//! Unix socket only nginx may open, which is what makes the person nginx
//! names in `X-Losos-Cloud-User` believable; a TCP listener (tests, a
//! developer's machine) believes the header only when told to.

use crate::cloud::auth::{Refusal, Who, EMAIL_HEADER, NAME_HEADER, USER_HEADER};
use crate::cloud::{api, dav, nc, Cloud};
use actix_web::http::{header, StatusCode};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use std::path::PathBuf;
use std::sync::Arc;

pub enum Listen {
    Unix(PathBuf),
    Tcp(String),
}

pub struct Options {
    pub listen: Listen,
    /// Believe `X-Losos-Cloud-User`. Always on for a Unix socket.
    pub trust_header: bool,
    pub workers: usize,
}

struct State {
    cloud: Arc<Cloud>,
    trust_header: bool,
}

fn header_value<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers().get(name).and_then(|v| v.to_str().ok())
}

/// A browser request from another site, which must not act on the person's
/// session: an unsafe method whose `Origin` is not this host.
fn cross_site(req: &HttpRequest) -> bool {
    let safe = matches!(
        req.method().as_str(),
        "GET" | "HEAD" | "OPTIONS" | "PROPFIND" | "REPORT" | "SEARCH"
    );
    if safe {
        return false;
    }
    let Some(origin) = header_value(req, "origin") else {
        return false;
    };
    let host = header_value(req, "host").unwrap_or_default();
    let origin_host = origin
        .split_once("://")
        .map(|(_, h)| h)
        .unwrap_or(origin)
        .trim_end_matches('/');
    origin == "null" || !origin_host.eq_ignore_ascii_case(host)
}

fn identify(state: &State, req: &HttpRequest) -> Result<Who, Refusal> {
    let trusted = |name| {
        if state.trust_header {
            header_value(req, name)
        } else {
            None
        }
    };
    state.cloud.auth.identify(
        &state.cloud.store,
        trusted(USER_HEADER),
        trusted(NAME_HEADER),
        trusted(EMAIL_HEADER),
        header_value(req, "authorization"),
    )
}

fn unauthorized(req: &HttpRequest, refusal: Refusal) -> HttpResponse {
    let path = req.path();
    let message = match refusal {
        Refusal::Throttled => "too many wrong passwords, wait a few minutes",
        _ => "sign in with your LosOS Login name and password, or an app password",
    };
    if path.starts_with("/ocs/") {
        let mut r = nc::ocs(req, StatusCode::UNAUTHORIZED, message, serde_json::Value::Null);
        r.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Basic realm=\"LosOS Cloud\", charset=\"UTF-8\""),
        );
        return r;
    }
    if path.starts_with("/cloud/api") {
        return api::error(StatusCode::UNAUTHORIZED, "signIn", message);
    }
    let mut r = dav::dav_error(
        StatusCode::UNAUTHORIZED,
        "Sabre\\DAV\\Exception\\NotAuthenticated",
        message,
    );
    // Only on the WebDAV paths: a browser that is shown this on a page would
    // pop its own password box over LosOS Login.
    r.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Basic realm=\"LosOS Cloud\", charset=\"UTF-8\""),
    );
    r
}

async fn handle(req: HttpRequest, body: web::Payload, state: web::Data<State>) -> HttpResponse {
    let cloud = &*state.cloud;
    let path = req.path().to_owned();
    let method = req.method().as_str().to_owned();
    if cross_site(&req) {
        return api::error(StatusCode::FORBIDDEN, "crossSite", "a request from another site");
    }

    // What answers without anyone signed in.
    match (method.as_str(), path.as_str()) {
        ("GET" | "HEAD", "/status.php") => return nc::status(cloud),
        ("GET", "/ocs/v1.php/cloud/capabilities" | "/ocs/v2.php/cloud/capabilities") => {
            return nc::ocs(&req, StatusCode::OK, "", nc::capabilities(cloud))
        }
        ("POST", "/index.php/login/v2") => return nc::flow_start(&req, cloud),
        ("POST", "/index.php/login/v2/poll") => {
            return match dav::read_body(body, 1 << 16).await {
                Ok(b) => nc::flow_poll(&req, cloud, &b),
                Err(r) => r,
            }
        }
        ("OPTIONS", p) if p.starts_with("/remote.php/") => return dav::options(),
        _ => {}
    }
    // The address the apps open the browser at: the web app's connect page,
    // which nginx puts behind LosOS Login.
    if let Some(token) = path.strip_prefix("/index.php/login/v2/flow/") {
        if token.chars().all(|c| c.is_ascii_alphanumeric()) {
            return HttpResponse::Found()
                .insert_header((header::LOCATION, format!("/cloud/connect/{token}")))
                .finish();
        }
        return HttpResponse::NotFound().finish();
    }
    // Login Flow v1, which the phone apps open in a web view: the same
    // page, which hands the app its password through an nc:// address.
    if path == "/index.php/login/flow" {
        return HttpResponse::Found()
            .insert_header((header::LOCATION, "/cloud/connect/v1"))
            .finish();
    }
    // Nextcloud's own pages, as the apps link them: the web app instead.
    if path == "/index.php/apps/files" || path.starts_with("/index.php/apps/files/") || path == "/index.php" || path == "/" {
        return HttpResponse::Found()
            .insert_header((header::LOCATION, "/cloud/"))
            .finish();
    }
    if let Some(id) = path.strip_prefix("/index.php/f/") {
        if id.chars().all(|c| c.is_ascii_digit()) {
            return HttpResponse::Found()
                .insert_header((header::LOCATION, format!("/cloud/files?fileid={id}")))
                .finish();
        }
    }
    if let Some(r) = crate::cloud::share::public(&req, cloud, &method, &path) {
        return r;
    }

    let who = match identify(&state, &req) {
        Ok(w) => w,
        Err(refusal) => return unauthorized(&req, refusal),
    };

    match (method.as_str(), path.as_str()) {
        ("GET", "/ocs/v1.php/cloud/user" | "/ocs/v2.php/cloud/user") => {
            return nc::user(&req, cloud, &who)
        }
        ("GET", p) if p.starts_with("/ocs/v1.php/cloud/users/") || p.starts_with("/ocs/v2.php/cloud/users/") => {
            let id = p.rsplit('/').next().unwrap_or_default();
            if id == who.login {
                return nc::user(&req, cloud, &who);
            }
            return nc::ocs(&req, StatusCode::NOT_FOUND, "", serde_json::Value::Null);
        }
        ("GET", "/ocs/v2.php/core/getapppassword") => {
            return nc::get_app_password(&req, cloud, &who)
        }
        ("DELETE", "/ocs/v2.php/core/apppassword") => {
            return nc::delete_app_password(&req, cloud, &who)
        }
        _ => {}
    }
    // Thumbnails as the apps ask for them: `?fileId=&x=&y=`.
    if method == "GET" && matches!(path.as_str(), "/index.php/core/preview" | "/index.php/core/preview.png") {
        let q = |k: &str| {
            req.query_string().split('&').find_map(|p| {
                p.split_once('=')
                    .filter(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_owned())
            })
        };
        let Some(id) = q("fileId").and_then(|v| v.parse::<i64>().ok()) else {
            return HttpResponse::BadRequest().finish();
        };
        let x = q("x").and_then(|v| v.parse::<u32>().ok()).unwrap_or(256);
        let y = q("y").and_then(|v| v.parse::<u32>().ok()).unwrap_or(256);
        return crate::cloud::preview::serve(&req, cloud, &who.login, id, x.max(y));
    }
    if path.starts_with("/remote.php/") {
        return dav::handle(
            dav::Ctx {
                cloud,
                who: &who,
                req: &req,
            },
            body,
        )
        .await;
    }
    if path == "/cloud/api" || path.starts_with("/cloud/api/") {
        return api::handle(&req, cloud, &who, body).await;
    }
    if path.starts_with("/ocs/") {
        if let Some(r) = crate::cloud::share::ocs(&req, cloud, &who, body).await {
            return r;
        }
        return nc::ocs(&req, StatusCode::NOT_FOUND, "no such call", serde_json::Value::Null);
    }
    HttpResponse::NotFound().finish()
}

/// The person behind a request, for routes outside [`handle`]
/// (`extensions::scopes`): the same check, and the same 401 for nobody.
impl actix_web::FromRequest for Who {
    type Error = actix_web::Error;
    type Future = std::future::Ready<Result<Who, actix_web::Error>>;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        let result = match req.app_data::<web::Data<State>>() {
            Some(state) => identify(state, req).map_err(|refusal| {
                actix_web::error::InternalError::from_response("unauthorized", unauthorized(req, refusal)).into()
            }),
            None => Err(actix_web::error::ErrorInternalServerError("no cloud state")),
        };
        std::future::ready(result)
    }
}

/// The routes, for the real server and for tests.
pub fn configure(
    cloud: Arc<Cloud>,
    trust_header: bool,
) -> impl Fn(&mut web::ServiceConfig) + Clone + Send + 'static {
    let shared = web::Data::new(cloud.clone());
    let state = web::Data::new(State {
        cloud,
        trust_header,
    });
    move |cfg: &mut web::ServiceConfig| {
        cfg.app_data(state.clone()).app_data(shared.clone());
        crate::cloud::extensions::scopes(cfg, &state.cloud);
        cfg.default_service(web::to(handle));
    }
}

/// Serve until the process is told to stop.
pub async fn serve(cloud: Arc<Cloud>, opts: Options) -> std::io::Result<()> {
    let trust_header = opts.trust_header || matches!(opts.listen, Listen::Unix(_));
    let routes = configure(cloud.clone(), trust_header);
    // The chores, hourly.
    {
        let cloud = cloud.clone();
        std::thread::spawn(move || loop {
            cloud.housekeeping();
            std::thread::sleep(std::time::Duration::from_secs(3600));
        });
    }
    let server = HttpServer::new(move || App::new().configure(routes.clone()))
    .workers(opts.workers.max(1))
    // Sync runs keep a connection for a long time.
    .keep_alive(std::time::Duration::from_secs(75))
    .client_request_timeout(std::time::Duration::from_secs(60));
    match opts.listen {
        Listen::Unix(path) => {
            let _ = std::fs::remove_file(&path);
            let s = server.bind_uds(&path)?;
            // nginx is in the socket's group (modules/cloud.nix); nobody
            // else may connect.
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660))?;
            s.run().await
        }
        Listen::Tcp(addr) => server.bind(addr)?.run().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn cross_site_writes_are_refused_and_same_site_ones_pass() {
        let r = TestRequest::post()
            .insert_header(("host", "box.local"))
            .insert_header(("origin", "https://evil.example"))
            .to_http_request();
        assert!(cross_site(&r));
        let r = TestRequest::post()
            .insert_header(("host", "box.local"))
            .insert_header(("origin", "https://box.local"))
            .to_http_request();
        assert!(!cross_site(&r));
        let r = TestRequest::post()
            .insert_header(("host", "box.local"))
            .to_http_request();
        assert!(!cross_site(&r), "no Origin: not a browser");
        let r = TestRequest::get()
            .insert_header(("host", "box.local"))
            .insert_header(("origin", "https://evil.example"))
            .to_http_request();
        assert!(!cross_site(&r), "a GET changes nothing");
    }
}
