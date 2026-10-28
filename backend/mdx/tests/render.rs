//! What a document becomes: Markdown, the components, the result's fields.

use losos_mdx::{render, Flavour, Plain, Rendered, Resolver};

fn mdx(source: &str) -> Rendered {
    render(source, Flavour::Mdx, &Plain).unwrap()
}

#[test]
fn flavour_follows_the_file_name() {
    for name in ["a.paper", "b.slides", "c.note", "d.mdx", "E.PAPER"] {
        assert_eq!(Flavour::for_name(name), Some(Flavour::Mdx), "{name}");
    }
    assert_eq!(Flavour::for_name("f.md"), Some(Flavour::Md));
    assert_eq!(Flavour::for_name("g.sheet"), None);
    assert_eq!(Flavour::for_name("noext"), None);
}

#[test]
fn markdown_basics() {
    let r = mdx("# Plan\n\nSome **bold**, *em*, ~~gone~~ and `code`.\n\n1. one\n2. two\n\n> quoted\n\n---\n");
    assert_eq!(
        r.html,
        "<h1 id=\"plan\">Plan</h1><p>Some <strong>bold</strong>, <em>em</em>, <del>gone</del> and <code>code</code>.</p>\
         <ol><li>one</li><li>two</li></ol><blockquote><p>quoted</p></blockquote><hr>"
    );
    assert!(r.problems.is_empty());
}

#[test]
fn headings_get_unique_ids_and_lines() {
    let r = mdx("---\ntitle: x\n---\n# Žiar nad Hronom!\n\n## Notes\n\n## Notes\n");
    let ids: Vec<_> = r
        .headings
        .iter()
        .map(|h| (h.level, h.id.as_str(), h.line))
        .collect();
    assert_eq!(
        ids,
        [
            (1, "žiar-nad-hronom", 3),
            (2, "notes", 5),
            (2, "notes-1", 7)
        ]
    );
}

#[test]
fn front_matter_is_json_or_an_error() {
    let r = mdx("---\ntitle: Trip\ntags: [a, b]\n---\nx");
    assert_eq!(
        r.properties,
        Some(serde_json::json!({"title": "Trip", "tags": ["a", "b"]}))
    );
    let bad = mdx("---\n- just\n- a list\n---\nx");
    assert_eq!(bad.properties, None);
    assert_eq!(bad.properties_error.as_deref(), Some("- just\n- a list"));
    assert_eq!(mdx("---\n---\nx").properties, Some(serde_json::json!({})));
    assert_eq!(mdx("x").properties, None);
}

#[test]
fn tables_tasks_and_footnotes() {
    let r = mdx(
        "| a | b |\n|:-|-:|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n\nSee[^n].\n\n[^n]: The note.",
    );
    assert!(
        r.html
            .contains("<th align=\"left\">a</th><th align=\"right\">b</th>"),
        "{}",
        r.html
    );
    assert!(r.html.contains("<ul class=\"contains-task-list\"><li class=\"task-list-item\"><span class=\"task-box\" data-checked=\"true\">\u{2611}</span> done</li>"));
    assert!(r
        .html
        .contains("<sup><a href=\"#fn-n\" id=\"fnref-n\" class=\"footnote-ref\""));
    assert!(r
        .html
        .contains("<section class=\"footnotes\"><ol><li id=\"fn-n\"><p>The note.</p>"));
}

#[test]
fn code_stays_text() {
    let r = mdx("```Rust\nfn main() { <x> }\n```\n");
    assert!(
        r.html
            .contains("<pre><code class=\"language-rust\">fn main() { &lt;x&gt; }</code></pre>"),
        "{}",
        r.html
    );
}

#[test]
fn maths_is_mathml_with_its_tex_beside_it() {
    let r = mdx("$a<b$ and\n\n$$\n\\frac{1}{x^2}\n$$");
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    // Drawn by the browser, no script: MathML Core, inline and as a block.
    assert!(r.html.contains("<math><semantics>"), "{}", r.html);
    assert!(r.html.contains("<math display=\"block\">"), "{}", r.html);
    assert!(r.html.contains("<mfrac>"), "{}", r.html);
    assert!(r.html.contains("<mo>&lt;</mo>"), "{}", r.html);
    // The TeX rides along for Pandoc and for copying.
    assert!(
        r.html
            .contains("<annotation encoding=\"application/x-tex\">a&lt;b</annotation>"),
        "{}",
        r.html
    );
    assert!(!r.html.contains("<code"), "{}", r.html);
}

#[test]
fn an_equation_math_core_cannot_read_stays_tex() {
    let r = mdx("$\\frac{1}{$\n\n<Equation tex=\"x^{\" />");
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert!(r
        .html
        .contains("<code class=\"language-math math-inline\">\\frac{1}{</code>"));
    assert!(r
        .html
        .contains("<pre><code class=\"language-math math-display\">x^{</code></pre>"));
    // An unknown command keeps the rest of its equation.
    let r = mdx("$\\nosuchthing + x$");
    assert!(r.html.contains("<merror"), "{}", r.html);
    assert!(r.html.contains("<mi>x</mi>"), "{}", r.html);
}

#[test]
fn markdown_flavour_keeps_angle_brackets_and_braces_as_text() {
    let r = render("a < b and {c}", Flavour::Md, &Plain).unwrap();
    assert_eq!(r.html, "<p>a &lt; b and {c}</p>");
    assert!(r.problems.is_empty());
}

#[test]
fn every_component_draws() {
    let doc = r#"
<TableOfContents title="Contents" />

# One

<Callout type="tip" title="Hint">Body</Callout>

<Details summary="More" open>Hidden</Details>

<Columns>
<Column>Left</Column>
<Column>Right</Column>
</Columns>

<Tabs>
<Tab label="First">A</Tab>
</Tabs>

<Figure src="cat.png" alt="A cat" caption="Our cat" />

<Embed href="https://example.org" title="Example" />

<Slide layout="title">Hello</Slide>

<Notes>Say hello.</Notes>

<PageBreak />

<Header>Trip plan</Header>
<Footer>Page <PageNumber /></Footer>

<Table caption="Budget">
<Row header><Cell>Item</Cell><Cell align="right">EUR</Cell></Row>
<Row><Cell colSpan="2">Cabin</Cell></Row>
</Table>

Ask <Mention login="eva" /> by <Date value="2026-08-14" format="long" />, see <Comment text="Which cabin?" author="eva" date="2026-10-28">the cabin</Comment>.

<Equation tex="e = mc^2" />

<Chart src="budget.sheet" range="A1:B5" type="pie" title="Where it goes" />

Press <kbd>Ctrl</kbd>, H<sub>2</sub>O, x<sup>2</sup>, <u>under</u>, <mark>marked</mark>, <s>struck</s>, <small>small</small>, <abbr title="LosOS">L</abbr><br />.
"#;
    let r = mdx(doc);
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    for want in [
        "<nav class=\"mdx-toc\"><p class=\"mdx-toc-title\">Contents</p><ol><li><a href=\"#one\"",
        "class=\"callout callout-tip\"",
        "<p class=\"callout-title\">Hint</p>",
        "<details open=\"\" class=\"mdx-details\"><summary>More</summary>",
        "<div class=\"mdx-columns\"><div class=\"mdx-column\">",
        "<section data-label=\"First\" class=\"mdx-tab\">",
        "<figure class=\"mdx-figure\"><img src=\"cat.png\" alt=\"A cat\"><figcaption>Our cat</figcaption></figure>",
        "class=\"mdx-embed\"",
        "<section data-layout=\"title\" class=\"mdx-slide\">",
        "<aside class=\"mdx-speaker-notes\">",
        "<div class=\"page-break\"></div>",
        "<header class=\"mdx-page-header\">",
        "<span class=\"mdx-page-number\"></span>",
        "<table class=\"mdx-table\"><caption>Budget</caption><tbody><tr><th>Item</th><th align=\"right\">EUR</th></tr>",
        "<td colspan=\"2\">Cabin</td>",
        "<span class=\"mdx-mention\" data-login=\"eva\">@eva</span>",
        "<time datetime=\"2026-08-14\" data-format=\"long\" class=\"mdx-date\">14 August 2026</time>",
        "<span data-comment=\"Which cabin?\" data-author=\"eva\" data-date=\"2026-10-28\" class=\"mdx-comment\">the cabin</span>",
        "<math display=\"block\"><semantics><mrow><mi>e</mi><mo>=</mo>",
        "<figure data-src=\"budget.sheet\" data-type=\"pie\" data-range=\"A1:B5\" class=\"mdx-chart\"><figcaption>Where it goes</figcaption></figure>",
        "<kbd>Ctrl</kbd>",
        "<sub>2</sub>",
        "<u>under</u>",
        "<abbr title=\"LosOS\">L</abbr><br>",
    ] {
        assert!(r.html.contains(want), "missing {want}\nin {}", r.html);
    }
    assert!(!r.html.contains("<p></p>"), "{}", r.html);
}

#[test]
fn props_are_checked() {
    for (doc, message) in [
        ("<Callout type=\"danger\">x</Callout>", "type is one of"),
        ("<Cell colSpan=\"0\">x</Cell>", "a whole number"),
        ("<Date value=\"14.8.2026\" />", "YYYY-MM-DD"),
        ("<Figure alt=\"x\" />", "needs src"),
        ("<Details open=\"yes\">x</Details>", "takes no value"),
        ("<Tab>x</Tab>", "needs label"),
    ] {
        let r = mdx(doc);
        assert!(
            r.problems.iter().any(|p| p.message.contains(message)),
            "{doc}: {:?}",
            r.problems
        );
    }
}

struct Vault;
impl Resolver for Vault {
    fn note(&self, target: &str) -> Option<String> {
        (target == "trip.note").then(|| "notes/trip.note".to_string())
    }
    fn file(&self, target: &str) -> Option<String> {
        Some(format!("/cloud/files/{target}"))
    }
    fn href(&self, path: &str, sub: &str) -> String {
        format!("/notes/{path}?at={sub}")
    }
}

#[test]
fn links_go_through_the_resolver() {
    let r = render(
        "[t](trip.note#Day%201) [m](missing.note) ![c](cat.png) [w](https://x.org) [h](#top)",
        Flavour::Mdx,
        &Vault,
    )
    .unwrap();
    assert!(
        r.html
            .contains("href=\"/notes/notes/trip.note?at=Day%201\""),
        "{}",
        r.html
    );
    assert!(r.html.contains("href=\"missing.note\""));
    assert!(r.html.contains("src=\"/cloud/files/cat.png\""));
    assert!(r.html.contains("href=\"https://x.org\""));
    assert!(r.html.contains("href=\"#top\""));
    let found: Vec<_> = r
        .links
        .iter()
        .map(|l| (l.target.as_str(), l.embed, l.external))
        .collect();
    assert_eq!(
        found,
        [
            ("trip.note#Day%201", false, false),
            ("missing.note", false, false),
            ("cat.png", true, false),
            ("https://x.org", false, true),
            ("#top", false, false)
        ]
    );
}

#[test]
fn a_pass_can_add_attributes_to_the_elements_written() {
    let mut parsed = losos_mdx::parse("Para ^abc\n\n- item", Flavour::Mdx).unwrap();
    parsed.attributes.insert(
        (0, "p".into()),
        vec![
            ("id".into(), "^abc".into()),
            ("data-line".into(), "0".into()),
        ],
    );
    // The list and its item start at the same offset; the tag tells them
    // apart.
    parsed.attributes.insert(
        (11, "li".into()),
        vec![
            ("class".into(), "is-task".into()),
            ("data-task".into(), "x".into()),
        ],
    );
    let r = parsed.render(&Plain);
    assert!(
        r.html.contains("<p id=\"^abc\" data-line=\"0\">"),
        "{}",
        r.html
    );
    assert!(
        r.html
            .contains("<ul><li data-task=\"x\" class=\"is-task\">"),
        "{}",
        r.html
    );
}

#[test]
fn reserved_names_written_by_the_author_are_refused_but_keep_their_text() {
    let r = mdx("<losos:wikilink target=\"x\">kept</losos:wikilink>");
    assert!(r.problems.iter().any(|p| p.message.contains("reserved")));
    assert!(r.html.contains("kept"));
}

#[test]
fn the_tree_is_json_with_positions() {
    let parsed =
        losos_mdx::parse("# A\n\n<Callout type=\"tip\">\nb\n</Callout>", Flavour::Mdx).unwrap();
    let json = losos_mdx::tree_json(&parsed.tree);
    assert_eq!(json["type"], "root");
    assert_eq!(json["children"][0]["type"], "heading");
    assert_eq!(json["children"][1]["type"], "mdxJsxFlowElement");
    assert_eq!(json["children"][1]["name"], "Callout");
    assert_eq!(json["children"][0]["position"]["start"]["line"], 1);
}

#[test]
fn a_date_is_words_in_the_documents_language_then_the_readers() {
    let doc = "<Date value=\"2026-10-28\" format=\"long\" />";
    assert!(mdx(doc).html.contains(">28 October 2026</time>"));
    let with_lang = format!("---\nlang: sk\n---\n{doc}");
    assert!(mdx(&with_lang).html.contains(">28. októbra 2026</time>"));
    let mut parsed = losos_mdx::parse(doc, Flavour::Mdx).unwrap();
    parsed.lang = Some("de-AT".into());
    assert!(parsed
        .render(&Plain)
        .html
        .contains(">28. Oktober 2026</time>"));
    // The document's own language wins over the reader's.
    let mut parsed = losos_mdx::parse(&with_lang, Flavour::Mdx).unwrap();
    parsed.lang = Some("de".into());
    assert!(parsed
        .render(&Plain)
        .html
        .contains(">28. októbra 2026</time>"));
}
