//! Calendars and contacts over CalDAV and CardDAV, the way DAVx5 and the
//! iOS accounts walk them: discovery from the DAV root, the homes, then
//! PUT, multiget, query and sync-collection, against the real server.

use actix_web::test::{call_service, init_service, read_body, TestRequest};
use actix_web::App;
use losos_ctl::cloud::{server, Cloud, Config};
use std::sync::Arc;

macro_rules! app {
    ($cloud:expr) => {
        init_service(App::new().configure(server::configure($cloud.clone(), true))).await
    };
}

fn cloud() -> (tempfile::TempDir, Arc<Cloud>) {
    let d = tempfile::tempdir().unwrap();
    let c = Arc::new(Cloud::open(d.path(), Config::default()).unwrap());
    (d, c)
}

fn req(m: &str, path: &str, who: &str) -> TestRequest {
    TestRequest::default()
        .method(actix_web::http::Method::from_bytes(m.as_bytes()).unwrap())
        .uri(path)
        .insert_header(("x-losos-cloud-user", who))
        .insert_header(("x-losos-cloud-email", "alice@example.org"))
        .insert_header(("host", "box.local"))
}

async fn text(resp: actix_web::dev::ServiceResponse) -> String {
    String::from_utf8(read_body(resp).await.to_vec()).unwrap()
}

const EVENT: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//DAVx5//EN\r\nBEGIN:VEVENT\r\nUID:dentist-1\r\nDTSTART:20261023T090000Z\r\nDTEND:20261023T100000Z\r\nSUMMARY:Dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
const OLD_EVENT: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:old-1\r\nDTSTART;VALUE=DATE:20200101\r\nSUMMARY:New year\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
const TASK: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:milk-1\r\nSUMMARY:Buy milk\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
const CARD: &str =
    "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:eva-1\r\nFN:Eva Kováč\r\nTEL:+421900000000\r\nEND:VCARD\r\n";

#[actix_web::test]
async fn discovery_finds_the_principal_and_both_homes() {
    let (_d, c) = cloud();
    let app = app!(c);
    let body =
        r#"<d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#;
    let r = call_service(
        &app,
        req("PROPFIND", "/remote.php/dav/", "alice")
            .insert_header(("depth", "0"))
            .set_payload(body)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 207);
    let t = text(r).await;
    assert!(
        t.contains(
            "<d:current-user-principal><d:href>/remote.php/dav/principals/users/alice/</d:href>"
        ),
        "{t}"
    );

    let body = r#"<d:propfind xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:prop><cal:calendar-home-set/><card:addressbook-home-set/><cal:calendar-user-address-set/><d:displayname/></d:prop></d:propfind>"#;
    let r = call_service(
        &app,
        req(
            "PROPFIND",
            "/remote.php/dav/principals/users/alice/",
            "alice",
        )
        .insert_header(("depth", "0"))
        .set_payload(body)
        .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(
        t.contains("<cal:calendar-home-set><d:href>/remote.php/dav/calendars/alice/</d:href>"),
        "{t}"
    );
    assert!(
        t.contains(
            "<card:addressbook-home-set><d:href>/remote.php/dav/addressbooks/users/alice/</d:href>"
        ),
        "{t}"
    );
    assert!(t.contains("mailto:alice@example.org"), "{t}");

    // Someone else's principal or home is refused.
    let r = call_service(
        &app,
        req("PROPFIND", "/remote.php/dav/calendars/bob/", "alice").to_request(),
    )
    .await;
    assert_eq!(r.status(), 403);

    // The home lists the calendar and address book everyone starts with.
    let body = r#"<d:propfind xmlns:d="DAV:" xmlns:cs="http://calendarserver.org/ns/" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:prop><d:resourcetype/><d:displayname/><cs:getctag/><cal:supported-calendar-component-set/></d:prop></d:propfind>"#;
    let r = call_service(
        &app,
        req("PROPFIND", "/remote.php/dav/calendars/alice/", "alice")
            .insert_header(("depth", "1"))
            .set_payload(body)
            .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(
        t.contains("/remote.php/dav/calendars/alice/personal/"),
        "{t}"
    );
    assert!(t.contains("<cal:calendar/>"), "{t}");
    assert!(t.contains("<cal:comp name=\"VTODO\"/>"), "{t}");
    let r = call_service(
        &app,
        req(
            "PROPFIND",
            "/remote.php/dav/addressbooks/users/alice/",
            "alice",
        )
        .insert_header(("depth", "1"))
        .set_payload(body)
        .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(
        t.contains("/remote.php/dav/addressbooks/users/alice/contacts/"),
        "{t}"
    );
    assert!(t.contains("<card:addressbook/>"), "{t}");

    let r = call_service(
        &app,
        TestRequest::default()
            .method(actix_web::http::Method::OPTIONS)
            .uri("/remote.php/dav/calendars/alice/")
            .to_request(),
    )
    .await;
    let dav = r.headers().get("dav").unwrap().to_str().unwrap();
    assert!(dav.contains("calendar-access") && dav.contains("addressbook"));
}

#[actix_web::test]
async fn events_tasks_and_cards_round_trip_and_sync() {
    let (_d, c) = cloud();
    let app = app!(c);
    let cal = "/remote.php/dav/calendars/alice/personal";

    let r = call_service(
        &app,
        req("PUT", &format!("{cal}/dentist.ics"), "alice")
            .insert_header(("if-none-match", "*"))
            .insert_header(("content-type", "text/calendar"))
            .set_payload(EVENT)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    let etag = r
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    // The same UID under another name is refused; If-None-Match guards.
    let r = call_service(
        &app,
        req("PUT", &format!("{cal}/copy.ics"), "alice")
            .set_payload(EVENT)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 409);
    assert!(text(r).await.contains("no-uid-conflict"));
    let r = call_service(
        &app,
        req("PUT", &format!("{cal}/dentist.ics"), "alice")
            .insert_header(("if-none-match", "*"))
            .set_payload(EVENT)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 412);
    let r = call_service(
        &app,
        req("PUT", &format!("{cal}/bad.ics"), "alice")
            .set_payload("hello")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 403);

    for (name, body) in [("old.ics", OLD_EVENT), ("milk.ics", TASK)] {
        let r = call_service(
            &app,
            req("PUT", &format!("{cal}/{name}"), "alice")
                .set_payload(body)
                .to_request(),
        )
        .await;
        assert_eq!(r.status(), 201, "{name}");
    }

    // GET gives the text back as sent.
    let r = call_service(
        &app,
        req("GET", &format!("{cal}/dentist.ics"), "alice").to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers().get("etag").unwrap().to_str().unwrap(), etag);
    assert!(r
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/calendar"));
    assert_eq!(text(r).await, EVENT);

    // The initial sync: everything, and a token.
    let sync = r#"<d:sync-collection xmlns:d="DAV:"><d:sync-token/><d:sync-level>1</d:sync-level><d:prop><d:getetag/></d:prop></d:sync-collection>"#;
    let r = call_service(
        &app,
        req("REPORT", &format!("{cal}/"), "alice")
            .set_payload(sync)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 207);
    let t = text(r).await;
    assert_eq!(t.matches("<d:response>").count(), 3, "{t}");
    let token = t
        .split("<d:sync-token>")
        .nth(1)
        .and_then(|s| s.split("</d:sync-token>").next())
        .unwrap()
        .to_owned();

    // Multiget with calendar-data, one known name and one not.
    let multiget = format!(
        r#"<cal:calendar-multiget xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/><cal:calendar-data/></d:prop><d:href>{cal}/dentist.ics</d:href><d:href>{cal}/gone.ics</d:href></cal:calendar-multiget>"#
    );
    let r = call_service(
        &app,
        req("REPORT", &format!("{cal}/"), "alice")
            .set_payload(multiget)
            .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(t.contains("SUMMARY:Dentist"), "{t}");
    assert!(t.contains("404 Not Found"), "{t}");

    // A query for events this month: the dentist, not the old one, not the task.
    let query = r#"<cal:calendar-query xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/></d:prop><cal:filter><cal:comp-filter name="VCALENDAR"><cal:comp-filter name="VEVENT"><cal:time-range start="20261001T000000Z" end="20261101T000000Z"/></cal:comp-filter></cal:comp-filter></cal:filter></cal:calendar-query>"#;
    let r = call_service(
        &app,
        req("REPORT", &format!("{cal}/"), "alice")
            .set_payload(query)
            .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(
        t.contains("dentist.ics") && !t.contains("old.ics") && !t.contains("milk.ics"),
        "{t}"
    );
    let tasks = query.replace("VEVENT", "VTODO").replace(
        r#"<cal:time-range start="20261001T000000Z" end="20261101T000000Z"/>"#,
        "",
    );
    let r = call_service(
        &app,
        req("REPORT", &format!("{cal}/"), "alice")
            .set_payload(tasks)
            .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(t.contains("milk.ics") && !t.contains("dentist.ics"), "{t}");

    // A delete with the wrong ETag is refused; then the delta sync shows it.
    let r = call_service(
        &app,
        req("DELETE", &format!("{cal}/old.ics"), "alice")
            .insert_header(("if-match", "\"nope\""))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 412);
    let r = call_service(
        &app,
        req("DELETE", &format!("{cal}/old.ics"), "alice").to_request(),
    )
    .await;
    assert_eq!(r.status(), 204);
    let delta = sync.replace(
        "<d:sync-token/>",
        &format!("<d:sync-token>{token}</d:sync-token>"),
    );
    let r = call_service(
        &app,
        req("REPORT", &format!("{cal}/"), "alice")
            .set_payload(delta)
            .to_request(),
    )
    .await;
    let t = text(r).await;
    assert_eq!(t.matches("<d:response>").count(), 1, "{t}");
    assert!(t.contains("old.ics") && t.contains("404 Not Found"), "{t}");
    let r = call_service(
        &app,
        req("REPORT", &format!("{cal}/"), "alice")
            .set_payload(sync.replace(
                "<d:sync-token/>",
                "<d:sync-token>http://sabre.io/ns/sync/999</d:sync-token>",
            ))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 403);
    assert!(text(r).await.contains("valid-sync-token"));

    // Contacts: a card in, an addressbook-query out.
    let book = "/remote.php/dav/addressbooks/users/alice/contacts";
    let r = call_service(
        &app,
        req("PUT", &format!("{book}/eva.vcf"), "alice")
            .set_payload(CARD)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    let q = r#"<card:addressbook-query xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:prop><d:getetag/><card:address-data/></d:prop></card:addressbook-query>"#;
    let r = call_service(
        &app,
        req("REPORT", &format!("{book}/"), "alice")
            .set_payload(q)
            .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(t.contains("FN:Eva Kováč"), "{t}");

    // Bob sees none of it.
    let r = call_service(
        &app,
        req("GET", &format!("{cal}/dentist.ics"), "bob").to_request(),
    )
    .await;
    assert_eq!(r.status(), 403);
}

#[actix_web::test]
async fn calendars_are_made_renamed_and_deleted() {
    let (_d, c) = cloud();
    let app = app!(c);
    let mk = r#"<cal:mkcalendar xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:x1="http://apple.com/ns/ical/"><d:set><d:prop><d:displayname>Work</d:displayname><x1:calendar-color>#FF0000</x1:calendar-color><cal:supported-calendar-component-set><cal:comp name="VEVENT"/></cal:supported-calendar-component-set></d:prop></d:set></cal:mkcalendar>"#;
    let r = call_service(
        &app,
        req(
            "MKCALENDAR",
            "/remote.php/dav/calendars/alice/work/",
            "alice",
        )
        .set_payload(mk)
        .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    let r = call_service(
        &app,
        req(
            "MKCALENDAR",
            "/remote.php/dav/calendars/alice/work/",
            "alice",
        )
        .set_payload(mk)
        .to_request(),
    )
    .await;
    assert_eq!(r.status(), 405);
    // An event-only calendar takes no task.
    let r = call_service(
        &app,
        req("PUT", "/remote.php/dav/calendars/alice/work/t.ics", "alice")
            .set_payload(TASK)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 403);

    let patch = r#"<d:propertyupdate xmlns:d="DAV:"><d:set><d:prop><d:displayname>Work stuff</d:displayname></d:prop></d:set></d:propertyupdate>"#;
    let r = call_service(
        &app,
        req(
            "PROPPATCH",
            "/remote.php/dav/calendars/alice/work/",
            "alice",
        )
        .set_payload(patch)
        .to_request(),
    )
    .await;
    assert_eq!(r.status(), 207);
    assert!(text(r).await.contains("200 OK"));
    let r = call_service(
        &app,
        req("PROPFIND", "/remote.php/dav/calendars/alice/work/", "alice")
            .insert_header(("depth", "0"))
            .to_request(),
    )
    .await;
    let t = text(r).await;
    assert!(t.contains("Work stuff") && t.contains("#FF0000"), "{t}");

    // Extended MKCOL makes an address book.
    let mkcol = r#"<d:mkcol xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:set><d:prop><d:resourcetype><d:collection/><card:addressbook/></d:resourcetype><d:displayname>Family</d:displayname></d:prop></d:set></d:mkcol>"#;
    let r = call_service(
        &app,
        req(
            "MKCOL",
            "/remote.php/dav/addressbooks/users/alice/family/",
            "alice",
        )
        .set_payload(mkcol)
        .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);

    let r = call_service(
        &app,
        req("DELETE", "/remote.php/dav/calendars/alice/work/", "alice").to_request(),
    )
    .await;
    assert_eq!(r.status(), 204);
    let r = call_service(
        &app,
        req("PROPFIND", "/remote.php/dav/calendars/alice/work/", "alice").to_request(),
    )
    .await;
    assert_eq!(r.status(), 404);
}
