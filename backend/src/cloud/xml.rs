//! A small tree for the XML bodies WebDAV clients send (PROPFIND,
//! PROPPATCH, LOCK, REPORT, SEARCH), and the writer for the answers.
//!
//! The bodies are a few hundred bytes, so a tree is simpler than a stream:
//! every element carries its namespace already resolved, which is all a
//! WebDAV request needs. Doctypes are refused, so no entity is ever
//! expanded beyond the five XML defines and character references.

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

pub const DAV: &str = "DAV:";
pub const OC: &str = "http://owncloud.org/ns";
pub const NC: &str = "http://nextcloud.org/ns";
pub const SABRE: &str = "http://sabredav.org/ns";
pub const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
pub const CARDDAV: &str = "urn:ietf:params:xml:ns:carddav";
pub const CS: &str = "http://calendarserver.org/ns/";
pub const APPLE: &str = "http://apple.com/ns/ical/";

/// A namespaced name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name {
    pub ns: String,
    pub local: String,
}

impl Name {
    pub fn new(ns: &str, local: &str) -> Name {
        Name {
            ns: ns.to_owned(),
            local: local.to_owned(),
        }
    }

    pub fn is(&self, ns: &str, local: &str) -> bool {
        self.ns == ns && self.local == local
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: Name,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// The text directly inside, concatenated.
    pub text: String,
    /// The element's own source, for dead properties stored as written.
    pub inner_xml: String,
}

impl Element {
    pub fn is(&self, ns: &str, local: &str) -> bool {
        self.name.is(ns, local)
    }

    pub fn child(&self, ns: &str, local: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.is(ns, local))
    }

    pub fn children_named<'a>(
        &'a self,
        ns: &'a str,
        local: &'a str,
    ) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.is(ns, local))
    }

    /// The first element at this path of children.
    pub fn find(&self, path: &[(&str, &str)]) -> Option<&Element> {
        let mut cur = self;
        for (ns, local) in path {
            cur = cur.child(ns, local)?;
        }
        Some(cur)
    }

    /// All text below, trimmed.
    pub fn deep_text(&self) -> String {
        let mut s = self.text.clone();
        for c in &self.children {
            s.push_str(&c.deep_text());
        }
        s.trim().to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum XmlError {
    #[error("malformed XML: {0}")]
    Malformed(String),
    #[error("no root element")]
    Empty,
}

fn resolve_ref(name: &str) -> Option<String> {
    match name {
        "amp" => Some("&".into()),
        "lt" => Some("<".into()),
        "gt" => Some(">".into()),
        "quot" => Some("\"".into()),
        "apos" => Some("'".into()),
        n if n.starts_with("#x") => u32::from_str_radix(&n[2..], 16)
            .ok()
            .and_then(char::from_u32)
            .map(String::from),
        n if n.starts_with('#') => n[1..]
            .parse()
            .ok()
            .and_then(char::from_u32)
            .map(String::from),
        _ => None,
    }
}

/// Parse a document into its root element.
pub fn parse(body: &str) -> Result<Element, XmlError> {
    let mut reader = NsReader::from_str(body);
    let mut stack: Vec<(Element, usize)> = Vec::new();
    let mut root = None;
    loop {
        let start_pos = reader.buffer_position() as usize;
        let (res, ev) = reader
            .read_resolved_event()
            .map_err(|e| XmlError::Malformed(e.to_string()))?;
        let ns = match res {
            ResolveResult::Bound(ns) => AsRef::<str>::as_ref(&ns).to_owned(),
            _ => String::new(),
        };
        match ev {
            Event::Start(e) | Event::Empty(e) if root.is_some() => {
                let _ = e;
                return Err(XmlError::Malformed("content after the root".into()));
            }
            Event::Start(e) => {
                let el = element(&ns, &e)?;
                stack.push((el, reader.buffer_position() as usize));
            }
            Event::Empty(e) => {
                let el = element(&ns, &e)?;
                match stack.last_mut() {
                    Some((parent, _)) => parent.children.push(el),
                    None => root = Some(el),
                }
            }
            Event::End(_) => {
                let (mut el, inner_start) = stack
                    .pop()
                    .ok_or_else(|| XmlError::Malformed("unbalanced end tag".into()))?;
                el.inner_xml = body
                    .get(inner_start..start_pos)
                    .unwrap_or_default()
                    .to_owned();
                match stack.last_mut() {
                    Some((parent, _)) => parent.children.push(el),
                    None => root = Some(el),
                }
            }
            Event::Text(t) => {
                if let Some((el, _)) = stack.last_mut() {
                    el.text.push_str(&t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some((el, _)) = stack.last_mut() {
                    el.text.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                let name = r.into_inner();
                let value = resolve_ref(&name)
                    .ok_or_else(|| XmlError::Malformed(format!("unknown entity {name}")))?;
                if let Some((el, _)) = stack.last_mut() {
                    el.text.push_str(&value);
                }
            }
            Event::DocType(_) => return Err(XmlError::Malformed("doctype".into())),
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(XmlError::Malformed("unclosed element".into()));
    }
    root.ok_or(XmlError::Empty)
}

fn element(ns: &str, e: &quick_xml::events::BytesStart<'_>) -> Result<Element, XmlError> {
    let local = AsRef::<str>::as_ref(&e.local_name()).to_owned();
    let mut attrs = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|e| XmlError::Malformed(e.to_string()))?;
        let key = AsRef::<str>::as_ref(&a.key.local_name()).to_owned();
        let value = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| XmlError::Malformed(e.to_string()))?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(Element {
        name: Name::new(ns, &local),
        attrs,
        children: Vec::new(),
        text: String::new(),
        inner_xml: String::new(),
    })
}

/// The prefixes the answers use, declared once on the root.
pub fn prefix(ns: &str) -> Option<&'static str> {
    match ns {
        DAV => Some("d"),
        OC => Some("oc"),
        NC => Some("nc"),
        SABRE => Some("s"),
        CALDAV => Some("cal"),
        CARDDAV => Some("card"),
        CS => Some("cs"),
        APPLE => Some("x1"),
        _ => None,
    }
}

pub const ROOT_NAMESPACES: &str = r#"xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns" xmlns:s="http://sabredav.org/ns" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:card="urn:ietf:params:xml:ns:carddav" xmlns:cs="http://calendarserver.org/ns/" xmlns:x1="http://apple.com/ns/ical/""#;

/// `<p:local>inner</p:local>`, with the namespace declared inline when it
/// has no prefix of ours.
pub fn wrap(name: &Name, inner: &str) -> String {
    match prefix(&name.ns) {
        Some(p) if inner.is_empty() => format!("<{p}:{}/>", name.local),
        Some(p) => format!("<{p}:{l}>{inner}</{p}:{l}>", l = name.local),
        None if name.ns.is_empty() => {
            if inner.is_empty() {
                format!("<{}/>", name.local)
            } else {
                format!("<{l}>{inner}</{l}>", l = name.local)
            }
        }
        None => {
            let ns = crate::cloud::util::xml_escape(&name.ns);
            if inner.is_empty() {
                format!("<x:{} xmlns:x=\"{ns}\"/>", name.local)
            } else {
                format!("<x:{l} xmlns:x=\"{ns}\">{inner}</x:{l}>", l = name.local)
            }
        }
    }
}

/// One `<d:response>` of a multistatus: the properties found, and the ones
/// asked for that are not there.
pub struct Response {
    pub href: String,
    pub found: Vec<(Name, String)>,
    pub missing: Vec<Name>,
    /// Properties refused (PROPPATCH), with the status line to give.
    pub other: Vec<(Name, &'static str)>,
    /// A plain status instead of propstats.
    pub status: Option<&'static str>,
}

impl Response {
    pub fn new(href: String) -> Response {
        Response {
            href,
            found: Vec::new(),
            missing: Vec::new(),
            other: Vec::new(),
            status: None,
        }
    }

    pub fn write(&self, out: &mut String) {
        out.push_str("<d:response><d:href>");
        out.push_str(&crate::cloud::util::xml_escape(&self.href));
        out.push_str("</d:href>");
        if let Some(status) = self.status {
            out.push_str("<d:status>HTTP/1.1 ");
            out.push_str(status);
            out.push_str("</d:status>");
        }
        if !self.found.is_empty() {
            out.push_str("<d:propstat><d:prop>");
            for (n, v) in &self.found {
                out.push_str(&wrap(n, v));
            }
            out.push_str("</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>");
        }
        if !self.missing.is_empty() {
            out.push_str("<d:propstat><d:prop>");
            for n in &self.missing {
                out.push_str(&wrap(n, ""));
            }
            out.push_str("</d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>");
        }
        let mut statuses: Vec<&str> = self.other.iter().map(|(_, s)| *s).collect();
        statuses.dedup();
        for status in statuses {
            out.push_str("<d:propstat><d:prop>");
            for (n, _) in self.other.iter().filter(|(_, s)| *s == status) {
                out.push_str(&wrap(n, ""));
            }
            out.push_str("</d:prop><d:status>HTTP/1.1 ");
            out.push_str(status);
            out.push_str("</d:status></d:propstat>");
        }
        out.push_str("</d:response>");
    }
}

pub fn multistatus(responses: &[Response]) -> String {
    let mut out = String::with_capacity(256 + responses.len() * 512);
    out.push_str(r#"<?xml version="1.0" encoding="utf-8"?>"#);
    out.push_str("\n<d:multistatus ");
    out.push_str(ROOT_NAMESPACES);
    out.push('>');
    for r in responses {
        r.write(&mut out);
    }
    out.push_str("</d:multistatus>");
    out
}

/// A Sabre-style error body, which the Nextcloud clients show the message of.
pub fn error(exception: &str, message: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:error xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns"><s:exception>{}</s:exception><s:message>{}</s:message></d:error>"#,
        crate::cloud::util::xml_escape(exception),
        crate::cloud::util::xml_escape(message)
    )
}

/// What a PROPFIND asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropRequest {
    AllProp(Vec<Name>),
    PropName,
    Props(Vec<Name>),
}

pub fn parse_propfind(body: &str) -> Result<PropRequest, XmlError> {
    if body.trim().is_empty() {
        return Ok(PropRequest::AllProp(Vec::new()));
    }
    let root = parse(body)?;
    if !root.is(DAV, "propfind") {
        return Err(XmlError::Malformed("not a propfind".into()));
    }
    if root.child(DAV, "propname").is_some() {
        return Ok(PropRequest::PropName);
    }
    if root.child(DAV, "allprop").is_some() {
        let include = root
            .child(DAV, "include")
            .map(|i| i.children.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();
        return Ok(PropRequest::AllProp(include));
    }
    match root.child(DAV, "prop") {
        Some(p) => Ok(PropRequest::Props(
            p.children.iter().map(|c| c.name.clone()).collect(),
        )),
        None => Ok(PropRequest::AllProp(Vec::new())),
    }
}

/// One change of a PROPPATCH, in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropChange {
    Set(Element),
    Remove(Name),
}

pub fn parse_proppatch(body: &str) -> Result<Vec<PropChange>, XmlError> {
    let root = parse(body)?;
    if !root.is(DAV, "propertyupdate") {
        return Err(XmlError::Malformed("not a propertyupdate".into()));
    }
    let mut out = Vec::new();
    for op in &root.children {
        let set = op.is(DAV, "set");
        if !set && !op.is(DAV, "remove") {
            continue;
        }
        for prop in op.children_named(DAV, "prop") {
            for p in &prop.children {
                out.push(if set {
                    PropChange::Set(p.clone())
                } else {
                    PropChange::Remove(p.name.clone())
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_propfind_names_its_properties_with_namespaces() {
        let body = r#"<?xml version="1.0"?>
<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:prop><d:getetag/><oc:fileid/><x:odd xmlns:x="urn:x"/></d:prop>
</d:propfind>"#;
        let req = parse_propfind(body).unwrap();
        assert_eq!(
            req,
            PropRequest::Props(vec![
                Name::new(DAV, "getetag"),
                Name::new(OC, "fileid"),
                Name::new("urn:x", "odd"),
            ])
        );
        assert_eq!(parse_propfind("").unwrap(), PropRequest::AllProp(vec![]));
        let names = r#"<propfind xmlns="DAV:"><propname/></propfind>"#;
        assert_eq!(parse_propfind(names).unwrap(), PropRequest::PropName);
    }

    #[test]
    fn a_proppatch_keeps_values_and_order() {
        let body = r#"<d:propertyupdate xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:set><d:prop><oc:favorite>1</oc:favorite></d:prop></d:set>
  <d:remove><d:prop><oc:tags/></d:prop></d:remove>
</d:propertyupdate>"#;
        let changes = parse_proppatch(body).unwrap();
        match &changes[0] {
            PropChange::Set(e) => {
                assert!(e.is(OC, "favorite"));
                assert_eq!(e.text, "1");
            }
            _ => panic!(),
        }
        assert_eq!(changes[1], PropChange::Remove(Name::new(OC, "tags")));
    }

    #[test]
    fn entities_resolve_and_doctypes_are_refused() {
        let e = parse(r#"<a xmlns="urn:a">x &amp; y &#233;&#x41;</a>"#).unwrap();
        assert_eq!(e.text, "x & y éA");
        assert!(parse(r#"<!DOCTYPE a [<!ENTITY x "y">]><a>&x;</a>"#).is_err());
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn inner_xml_is_kept_for_dead_properties() {
        let e = parse(r#"<a xmlns="urn:a"><b>1</b><c/></a>"#).unwrap();
        assert_eq!(e.inner_xml, "<b>1</b><c/>");
    }

    #[test]
    fn multistatus_writes_found_and_missing() {
        let mut r = Response::new("/remote.php/dav/files/a/x y".into());
        r.found.push((Name::new(DAV, "getetag"), "&quot;1&quot;".into()));
        r.missing.push(Name::new("urn:x", "odd"));
        let out = multistatus(&[r]);
        assert!(out.contains("<d:href>/remote.php/dav/files/a/x y</d:href>"));
        assert!(out.contains("<d:getetag>&quot;1&quot;</d:getetag>"));
        assert!(out.contains(r#"<x:odd xmlns:x="urn:x"/>"#));
        assert!(out.contains("404 Not Found"));
        assert!(parse(&out).is_ok(), "the answer is well-formed XML");
    }
}
