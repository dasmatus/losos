//! Documents written to break out. Every one must come back as inert HTML:
//! no script, no handler, no style, no frame or form, no script-scheme URL,
//! and no class the page's own layout uses.

use losos_mdx::{parse, render, Error, Flavour, Hook, Plain, Rendered, Writer};

fn html(source: &str, flavour: Flavour) -> Rendered {
    render(source, flavour, &Plain).unwrap_or_else(|e| panic!("{source:?} did not parse: {e}"))
}

/// A tag in the output: its name and its attributes, values unescaped.
struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
}

/// The output's tags. render.rs and ammonia write every attribute as
/// `name="value"` with `"` escaped, so this is enough to read them back.
fn tags(html: &str) -> Vec<Tag> {
    let mut out = Vec::new();
    for chunk in html.split('<').skip(1) {
        if chunk.starts_with('/') {
            continue;
        }
        let inside = chunk.split('>').next().unwrap_or("");
        let name: String = inside
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let mut attrs = Vec::new();
        let mut rest = &inside[name.len()..];
        loop {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            let key: String = rest
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '=')
                .collect();
            rest = &rest[key.len()..];
            let mut value = String::new();
            if let Some(r) = rest.strip_prefix("=\"") {
                let end = r.find('"').expect("a closed attribute");
                value = r[..end]
                    .replace("&quot;", "\"")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&");
                rest = &r[end + 1..];
            }
            if key.is_empty() {
                break;
            }
            attrs.push((key.to_ascii_lowercase(), value));
        }
        out.push(Tag {
            name: name.to_ascii_lowercase(),
            attrs,
        });
    }
    out
}

/// The things no output may hold, whatever went in.
fn assert_inert(r: &Rendered, source: &str) {
    const ALLOWED_TAGS: &[&str] = &[
        "a",
        "abbr",
        "aside",
        "blockquote",
        "br",
        "caption",
        "code",
        "del",
        "details",
        "div",
        "em",
        "figcaption",
        "figure",
        "footer",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "header",
        "hr",
        "img",
        "kbd",
        "li",
        "mark",
        "nav",
        "ol",
        "p",
        "pre",
        "s",
        "section",
        "small",
        "span",
        "strong",
        "sub",
        "summary",
        "sup",
        "table",
        "tbody",
        "td",
        "th",
        "thead",
        "time",
        "tr",
        "u",
        "ul",
        // MathML Core, which the equations are written in.
        "annotation",
        "math",
        "menclose",
        "merror",
        "mfrac",
        "mi",
        "mmultiscripts",
        "mn",
        "mo",
        "mover",
        "mpadded",
        "mphantom",
        "mprescripts",
        "mroot",
        "mrow",
        "ms",
        "mspace",
        "msqrt",
        "mstyle",
        "msub",
        "msubsup",
        "msup",
        "mtable",
        "mtd",
        "mtext",
        "mtr",
        "munder",
        "munderover",
        "semantics",
    ];
    for tag in tags(&r.html) {
        assert!(
            ALLOWED_TAGS.contains(&tag.name.as_str()),
            "<{}> survived in {source:?}: {}",
            tag.name,
            r.html
        );
        for (key, value) in &tag.attrs {
            assert!(
                !key.starts_with("on")
                    && !["style", "srcset", "formaction", "background"].contains(&key.as_str())
                    && !key.contains(':'),
                "{key} survived in {source:?}: {}",
                r.html
            );
            if ["href", "src", "data-src"].contains(&key.as_str()) {
                let squashed: String = value
                    .chars()
                    .filter(|c| !c.is_whitespace() && !c.is_control())
                    .collect::<String>()
                    .to_ascii_lowercase();
                for scheme in ["javascript:", "vbscript:", "data:"] {
                    assert!(
                        !squashed.starts_with(scheme),
                        "{key}={value} survived in {source:?}"
                    );
                }
            }
            if key == "class" {
                for token in value.split_whitespace() {
                    assert!(
                        [
                            "callout",
                            "contains-task-list",
                            "footnote",
                            "language-",
                            "math-",
                            "mdx-",
                            "menclose-",
                            "page-break",
                            "task-",
                            "losos-",
                            "is-"
                        ]
                        .iter()
                        .any(|p| token.starts_with(p)),
                        "class {token} survived in {source:?}"
                    );
                }
            }
        }
    }
}

const MARKDOWN_ATTACKS: &[&str] = &[
    "<script>alert(1)</script>",
    "<SCRIPT SRC=//evil></SCRIPT>",
    "<img src=x onerror=alert(1)>",
    "<img src=x OnErRoR=alert(1)>",
    "<a href=\"javascript:alert(1)\">x</a>",
    "<a href=\"JaVaScRiPt:alert(1)\">x</a>",
    "<a href=\"jav&#x09;ascript:alert(1)\">x</a>",
    "<a href=\" javascript:alert(1)\">x</a>",
    "<a href=\"vbscript:msgbox(1)\">x</a>",
    "<a href=\"data:text/html,<script>alert(1)</script>\">x</a>",
    "[x](javascript:alert(1))",
    "[x](JAVASCRIPT:alert(1))",
    "[x]: javascript:alert(1)\n\n[y][x]",
    "![x](javascript:alert(1))",
    "![x](data:image/svg+xml,<svg onload=alert(1)>)",
    "<div style=\"position:fixed;inset:0\">cover</div>",
    "<div class=\"fixed inset-0 z-50 bg-white\">cover</div>",
    "<iframe src=\"https://evil\"></iframe>",
    "<object data=\"x\"></object><embed src=\"x\">",
    "<form action=\"https://evil\"><input name=p type=password><button>go</button></form>",
    "<svg><script>alert(1)</script></svg>",
    "<svg onload=alert(1)>",
    "<math><mi xlink:href=\"javascript:alert(1)\">x</mi></math>",
    // MathML is allowed now, so its namespace tricks are tried too.
    "<math href=\"javascript:alert(1)\"><mi>x</mi></math>",
    "<math><mi onclick=\"alert(1)\" style=\"position:fixed\">x</mi></math>",
    "<math><mtext><table><mglyph><style><img src=x onerror=alert(1)>",
    "<math><mtext><h1><a><h6></a></h6><mglyph><svg><mtext><style><a title=\"</style><img src=x onerror=alert(1)>\"></style></h1>",
    "<math><annotation-xml encoding=\"text/html\"><img src=x onerror=alert(1)></annotation-xml></math>",
    "<math><annotation encoding=\"text/html\"><script>alert(1)</script></annotation></math>",
    "<math><maction actiontype=\"statusline#http://evil\"><mi>x</mi></maction></math>",
    "<math><mi mathvariant=\"expression(alert(1))\">x</mi></math>",
    "<math><semantics><mi>x</mi><annotation encoding=\"text/html\"><iframe src=//evil></iframe></annotation></semantics></math>",
    "$\\href{javascript:alert(1)}{x}$ and $\\style{position:fixed}{x}$",
    "<img src=\"https://ok/x.png\" srcset=\"javascript:alert(1)\">",
    "<style>body{display:none}</style>",
    "<link rel=stylesheet href=//evil><meta http-equiv=refresh content=0;url=//evil><base href=//evil>",
    "<details open ontoggle=alert(1)>x</details>",
    "<a href=\"#\" data-x=\"1\" data-ON-click=\"x\">x</a>",
    "<p id=\"x\" onmouseover=\"alert(1)\">x</p>",
    "<!-- <script>alert(1)</script> -->",
    "<noscript><p title=\"</noscript><img src=x onerror=alert(1)>\"></noscript>",
    "<table><tr><td background=\"javascript:alert(1)\">x</td></tr></table>",
    "`<script>` and ```\n<script>\n```",
];

#[test]
fn raw_html_in_markdown_comes_out_inert() {
    for attack in MARKDOWN_ATTACKS {
        assert_inert(&html(attack, Flavour::Md), attack);
    }
}

#[test]
fn the_same_attacks_in_mdx_come_out_inert() {
    for attack in MARKDOWN_ATTACKS {
        // Some are not valid MDX at all; a syntax error is a refusal too.
        if let Ok(r) = render(attack, Flavour::Mdx, &Plain) {
            assert_inert(&r, attack);
        }
    }
}

const MDX_ATTACKS: &[&str] = &[
    "{alert(1)}",
    "Text {fetch('//evil')} text",
    "import x from 'https://evil/x.js'",
    "export const a = alert(1)",
    "export default function Layout() {}",
    "<Callout {...window} />",
    "<Callout title={alert(1)} />",
    "<Callout onClick=\"alert(1)\">x</Callout>",
    "<Callout type=\"x onmouseover=alert(1)\">x</Callout>",
    "<Figure src=\"javascript:alert(1)\" />",
    "<Embed href=\"javascript:alert(1)\" />",
    "<Embed href=\"data:text/html,x\" title=\"x\" />",
    "<Chart src=\"javascript:alert(1)\" />",
    "<Mention login='\"><img src=x onerror=alert(1)>' onClick=\"x\" />",
    "<Script>alert(1)</Script>",
    "<script>alert(1)</script>",
    "<iframe src=\"https://evil\" />",
    "<div style=\"position:fixed\">x</div>",
    "<a href=\"javascript:alert(1)\">x</a>",
    "<img src=\"x\" onError=\"alert(1)\" />",
    "<u onClick=\"x\">u</u>",
    "<abbr title=\"x\" style=\"color:red\">a</abbr>",
    "<Cell colSpan=\"9999999\">x</Cell>",
    "<Date value=\"2026-01-01\\\" onload=\\\"x\" />",
    "<losos:wikilink target=\"x\" />",
    "<Tab label=\"x\" data-x=\"y\">x</Tab>",
];

#[test]
fn mdx_never_runs_and_never_lets_markup_through() {
    for attack in MDX_ATTACKS {
        match render(attack, Flavour::Mdx, &Plain) {
            Ok(r) => {
                assert_inert(&r, attack);
                assert!(
                    !r.problems.is_empty(),
                    "{attack:?} was refused silently: {}",
                    r.html
                );
            }
            Err(Error::Syntax { .. }) => {}
            Err(e) => panic!("{attack:?}: {e}"),
        }
    }
}

/// Odd but harmless: text that looks like markup, in a prop. It must come
/// out escaped, and needs no problem.
const ODD_BUT_HARMLESS: &[&str] = &[
    "<Figure src=\"x.png\" alt='\"><script>alert(1)</script>' />",
    "<Callout title='\" onmouseover=\"alert(1)'>x</Callout>",
    "<Mention login='\"><img src=x onerror=alert(1)>' />",
];

#[test]
fn markup_in_a_prop_is_text() {
    for doc in ODD_BUT_HARMLESS {
        let r = html(doc, Flavour::Mdx);
        assert_inert(&r, doc);
        assert!(r.problems.is_empty(), "{doc}: {:?}", r.problems);
    }
}

#[test]
fn a_refused_document_does_not_export() {
    let r = html("# Plan\n\n{alert(1)}", Flavour::Mdx);
    assert!(matches!(r.strict(), Err(Error::Refused(p)) if p.len() == 1));
    assert!(html("# Plan\n\nText.", Flavour::Mdx).strict().is_ok());
}

/// Runs `f` on a thread with a WebAssembly-sized stack (1 MiB), or a
/// debug build's equivalent: an unoptimised build spends several times the
/// stack per frame.
fn small_stack(f: impl FnOnce() + Send + 'static) {
    let size = if cfg!(debug_assertions) {
        8 << 20
    } else {
        1 << 20
    };
    std::thread::Builder::new()
        .stack_size(size)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_deep_equation_stays_tex_instead_of_overflowing_the_stack() {
    small_stack(|| {
        for tex in [
            "{".repeat(4000) + "x" + &"}".repeat(4000),
            "\\sqrt".repeat(800) + " x",
            "\\left(".repeat(1000) + "x" + &"\\right)".repeat(1000),
            "\\begin{matrix}".repeat(500) + "x" + &"\\end{matrix}".repeat(500),
            "x^".repeat(2000) + "x",
            "x".repeat(losos_mdx::MAX_TEX + 1),
        ] {
            for source in [format!("${tex}$"), format!("$$\n{tex}\n$$")] {
                let r = html(&source, Flavour::Mdx);
                assert!(!r.html.contains("<math"), "{}", &source[..40]);
                assert!(r.html.contains("language-math"), "{}", &source[..40]);
            }
        }
        // Just under the limits still draws.
        let tex =
            "{".repeat(losos_mdx::MAX_TEX_NESTING) + "x" + &"}".repeat(losos_mdx::MAX_TEX_NESTING);
        assert!(html(&format!("${tex}$"), Flavour::Mdx)
            .html
            .contains("<math"));
    });
}

#[test]
fn deep_nesting_is_refused_before_it_can_overflow_the_stack() {
    small_stack(|| {
        let max = losos_mdx::MAX_NESTING;
        for (source, flavour) in [
            (">".repeat(20_000) + " deep", Flavour::Md),
            (">".repeat(4 * max + 1) + " deep", Flavour::Md),
            (
                "<Callout>".repeat(20_000) + "x" + &"</Callout>".repeat(20_000),
                Flavour::Mdx,
            ),
            ("*".repeat(50_000) + "x" + &"*".repeat(50_000), Flavour::Md),
            (
                (0..1_500)
                    .map(|i| format!("{}- x\n", "  ".repeat(i)))
                    .collect::<String>(),
                Flavour::Md,
            ),
            // Under the scan's limits, over the tree's.
            (">".repeat(max + 5) + " deep", Flavour::Md),
            (
                "<Callout>".repeat(max + 5) + "x" + &"</Callout>".repeat(max + 5),
                Flavour::Mdx,
            ),
        ] {
            match render(&source, flavour, &Plain) {
                Err(Error::TooDeep { .. } | Error::Syntax { .. } | Error::TooLarge { .. }) => {}
                Ok(r) => panic!(
                    "{} bytes nested deep were drawn: {} bytes",
                    source.len(),
                    r.html.len()
                ),
                Err(e) => panic!("{e}"),
            }
        }
        // Just inside the limits still draws.
        let ok = ">".repeat(max - 10) + " deep";
        assert!(render(&ok, Flavour::Md, &Plain).is_ok());
        let ok = "<Callout>".repeat(max / 2 - 5) + "x" + &"</Callout>".repeat(max / 2 - 5);
        assert!(render(&ok, Flavour::Mdx, &Plain).is_ok());
    });
}

#[test]
fn a_huge_document_is_refused_rather_than_parsed() {
    let big = "a".repeat(losos_mdx::MAX_SOURCE + 1);
    assert!(matches!(
        parse(&big, Flavour::Md),
        Err(Error::TooLarge { .. })
    ));
}

#[test]
fn unclosed_jsx_is_a_syntax_error_with_a_place() {
    match render("# Title\n\n<Callout>\nnever closed", Flavour::Mdx, &Plain) {
        Err(Error::Syntax { place, .. }) => assert!(place.line >= 3, "{place:?}"),
        other => panic!("{other:?}"),
    }
}

struct Evil;
impl Hook for Evil {
    fn element(&self, w: &mut Writer<'_>, _name: &str, _node: &losos_mdx::mdast::Node) -> bool {
        w.raw("<script>alert(1)</script><a href=\"javascript:x\" onclick=\"y\" style=\"z\">e</a>");
        true
    }
}

#[test]
fn a_hook_cannot_write_past_the_allowlist_either() {
    let mut parsed = parse("Hello", Flavour::Mdx).unwrap();
    // What a Notes pass does: put a reserved element in the tree.
    if let losos_mdx::mdast::Node::Root(root) = &mut parsed.tree {
        root.children
            .push(losos_mdx::mdast::Node::MdxJsxFlowElement(
                losos_mdx::mdast::MdxJsxFlowElement {
                    children: vec![],
                    position: None,
                    name: Some("losos:evil".into()),
                    attributes: vec![],
                },
            ));
    }
    parsed.attributes.insert(
        (0, "p".into()),
        vec![("onclick".into(), "x".into()), ("style".into(), "y".into())],
    );
    let r = parsed.render_with(&Plain, Some(&Evil));
    assert_inert(&r, "hook");
    assert!(r.html.contains(">e</a>"), "{}", r.html);
}

struct Hostile;
impl losos_mdx::Resolver for Hostile {
    fn file(&self, _: &str) -> Option<String> {
        Some("javascript:alert(1)".into())
    }
    fn note(&self, _: &str) -> Option<String> {
        Some("x".into())
    }
    fn href(&self, _: &str, _: &str) -> String {
        "\" onclick=\"alert(1)".into()
    }
}

#[test]
fn a_resolver_cannot_inject_either() {
    let r = losos_mdx::render(
        "[a](b.note) ![c](d.png) <Figure src=\"e.png\" />",
        Flavour::Mdx,
        &Hostile,
    )
    .unwrap();
    assert_inert(&r, "resolver");
}
