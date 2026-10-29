//! Notes' own syntax through the whole pipeline: the pass, the hook and
//! the allowlist, in both flavours, and `.md` notes converted to `.note`.

use losos_mdx::notes::{render_note, to_note};
use losos_mdx::{Flavour, Resolver};

/// A vault of three notes and one picture.
struct Vault;
impl Resolver for Vault {
    fn note(&self, target: &str) -> Option<String> {
        let name = target.trim_end_matches(".md").trim_end_matches(".note");
        ["Home", "Tomatoes", "Pests/Aphids"]
            .iter()
            .find(|n| n.eq_ignore_ascii_case(name) || n.rsplit('/').next() == Some(name))
            .map(|n| format!("{n}.note"))
    }
    fn file(&self, target: &str) -> Option<String> {
        (target == "pic.png" || target == "song.mp3").then(|| format!("/files/{target}"))
    }
    fn href(&self, path: &str, sub: &str) -> String {
        if sub.is_empty() {
            format!("/n/{path}")
        } else {
            format!("/n/{path}#{sub}")
        }
    }
}

/// The HTML, without the `rel` the allowlist puts on every link, and with
/// each tag's attributes in the order they were written.
fn html(source: &str, flavour: Flavour) -> String {
    let h = render_note(source, flavour, &Vault)
        .unwrap()
        .html
        .replace(" rel=\"noopener noreferrer\"", "");
    h.replace(
        r#"<li data-task="x" class="task-list-item">"#,
        r#"<li class="task-list-item" data-task="x">"#,
    )
    .replace(
        r#"<li data-task=" " class="task-list-item">"#,
        r#"<li class="task-list-item" data-task=" ">"#,
    )
    .replace(
        r#"<li data-task="/" class="task-list-item">"#,
        r#"<li class="task-list-item" data-task="/">"#,
    )
    .replace(
        r#"<aside data-callout="tip" class="callout callout-tip">"#,
        r#"<aside class="callout callout-tip" data-callout="tip">"#,
    )
}

/// No tag in the HTML carries an event handler or a script URL.
fn inert(h: &str) -> bool {
    h.split('<').skip(1).all(|t| {
        let tag = t.split('>').next().unwrap_or_default().to_lowercase();
        // Attribute names: words outside quotes, before an `=`.
        let mut names = Vec::new();
        let (mut quoted, mut word) = (false, String::new());
        for c in tag.chars() {
            match c {
                '"' => quoted = !quoted,
                '=' if !quoted => names.push(std::mem::take(&mut word)),
                c if c.is_whitespace() && !quoted => word.clear(),
                c if !quoted => word.push(c),
                _ => {}
            }
        }
        !tag.starts_with("script")
            && !tag.contains("javascript:")
            && !names.iter().any(|a| a.starts_with("on"))
    })
}

fn both(source: &str) -> String {
    let md = html(source, Flavour::Md);
    assert_eq!(md, html(source, Flavour::Mdx), "the flavours differ");
    md
}

#[test]
fn wikilinks_resolve_through_the_caller() {
    let h = both("[[Tomatoes#Sowing|sow]] [[Aphids]] [[Nowhere]] [[#Top]] [[pic.png]]");
    assert!(h.contains(r#"<a class="wikilink" href="/n/Tomatoes.note#Sowing" data-note-path="Tomatoes.note" data-sub="Sowing">sow</a>"#), "{h}");
    assert!(h.contains(r#"data-note-path="Pests/Aphids.note""#), "{h}");
    assert!(
        h.contains(
            r##"<a class="wikilink is-unresolved" href="#" data-note-target="Nowhere">Nowhere</a>"##
        ),
        "{h}"
    );
    assert!(
        h.contains(r##"<a class="wikilink is-internal" href="#top" data-sub="Top">Top</a>"##),
        "{h}"
    );
    assert!(
        h.contains(r#"<a class="wikilink is-file" href="/files/pic.png""#),
        "{h}"
    );
}

#[test]
fn embeds() {
    let h = both("![[pic.png|300x200]] ![[song.mp3]] ![[Tomatoes#Sowing]] ![[gone]]");
    assert!(h.contains(r#"<img class="embed-image" src="/files/pic.png" alt="pic.png" data-width="300" data-height="200">"#), "{h}");
    assert!(h.contains(r#"<span class="embed-media" data-kind="audio" data-src="/files/song.mp3">song.mp3</span>"#), "{h}");
    assert!(
        h.contains(
            r#"<span class="note-embed" data-embed-path="Tomatoes.note" data-sub="Sowing"></span>"#
        ),
        "{h}"
    );
    assert!(
        h.contains(r#"<span class="embed-missing" data-note-target="gone">gone</span>"#),
        "{h}"
    );
    let links = render_note("![[pic.png]] [[Home]]", Flavour::Md, &Vault)
        .unwrap()
        .links;
    let found: Vec<(&str, bool)> = links.iter().map(|l| (l.target.as_str(), l.embed)).collect();
    assert_eq!(found, [("pic.png", true), ("Home", false)]);
}

#[test]
fn tags_highlights_breaks_comments() {
    let h = both("A #garden/beds tag, ==bright *and* bold== %%hidden%%\nnext line");
    assert!(
        h.contains(
            r##"<a class="tag" href="#tag/garden/beds" data-tag="garden/beds">#garden/beds</a>"##
        ),
        "{h}"
    );
    assert!(h.contains("<mark>bright <em>and</em> bold</mark>"), "{h}");
    assert!(!h.contains("hidden"), "{h}");
    assert!(h.contains("<br>"), "{h}");
}

#[test]
fn callouts_fold_and_carry_titles() {
    let h = both("> [!tip]- Sow *early*\n> In March.\n\n> [!WARNING]\n> Frost.");
    assert!(h.contains(r#"<details class="callout callout-tip" data-callout="tip"><summary class="callout-title"><span class="callout-icon"></span><span class="callout-title-inner">Sow <em>early</em></span></summary><div class="callout-content"><p>In March.</p></div></details>"#), "{h}");
    assert!(h.contains(r#"<aside class="callout callout-warning" data-callout="warning"><p class="callout-title"><span class="callout-icon"></span><span class="callout-title-inner">Warning</span></p>"#), "{h}");
}

#[test]
fn tasks_carry_their_lines() {
    let src = "---\na: 1\n---\n- [ ] dig\n- [x] sow\n- [/] water";
    let r = render_note(src, Flavour::Mdx, &Vault).unwrap();
    let h = html(src, Flavour::Mdx);
    assert!(h.contains(r#"<ul class="contains-task-list">"#), "{h}");
    assert!(h.contains(r#"<li class="task-list-item" data-task=" "><span class="task-box" data-task=" " data-line="3">"#), "{h}");
    assert!(h.contains(r#"data-task="x" data-line="4""#), "{h}");
    assert!(
        h.contains(r#"<li class="task-list-item" data-task="/">"#),
        "{h}"
    );
    assert_eq!(r.properties, Some(serde_json::json!({"a": 1})));
}

#[test]
fn block_ids_name_their_blocks() {
    let h = both("Para ^p1\n\n- item ^i1\n\n| a |\n|---|\n| b |\n\n^t1\n\n> [!note]\n> x\n\n^c1");
    for want in [
        r#"<p id="^p1">"#,
        r#"<li id="^i1">"#,
        r#"<table id="^t1">"#,
        r#"data-callout="note" id="^c1""#,
    ] {
        assert!(h.contains(want), "{want} in {h}");
    }
    assert!(!h.contains("^t1</p>"), "{h}");
}

#[test]
fn headings_keep_link_text() {
    let r = render_note("## About [[Tomatoes|toms]] #veg", Flavour::Md, &Vault).unwrap();
    assert_eq!(r.headings[0].text, "About toms #veg");
}

#[test]
fn mdx_components_sit_beside_obsidian_syntax() {
    let h = html("<Callout type=\"tip\" title=\"Beds\">\n  See [[Home]] and #tag\n</Callout>\n\n<Details summary=\"More\">\n==x==\n</Details>", Flavour::Mdx);
    assert!(h.contains(r#"<aside class="callout callout-tip" data-callout="tip"><p class="callout-title">Beds</p>"#), "{h}");
    assert!(
        h.contains(r#"data-note-path="Home.note""#) && h.contains(r#"class="tag""#),
        "{h}"
    );
    assert!(
        h.contains(
            "<details class=\"mdx-details\"><summary>More</summary><p><mark>x</mark></p></details>"
        ),
        "{h}"
    );
}

#[test]
fn an_author_cannot_write_the_reserved_elements() {
    let r = render_note("<losos:wikilink link=\"javascript:alert(1)\" /> <losos:callout kind=\"x\">y</losos:callout>", Flavour::Mdx, &Vault).unwrap();
    assert!(
        !r.html.contains("wikilink") && !r.html.contains("callout"),
        "{}",
        r.html
    );
    assert_eq!(r.problems.len(), 2, "{:?}", r.problems);
}

#[test]
fn hostile_links_stay_inert() {
    for src in [
        "[[\" onclick=\"alert(1)]]",
        "![[x.png\" onerror=\"alert(1)|1]]",
        "#tag\"><script>alert(1)</script>",
        "> [!x\" onclick=\"y] t",
        "<script>alert(1)</script> [[a<b>]]",
    ] {
        for flavour in [Flavour::Md, Flavour::Mdx] {
            let Ok(r) = render_note(src, flavour, &Vault) else {
                continue;
            };
            assert!(inert(&r.html), "{src}: {}", r.html);
        }
    }
}

#[test]
fn a_md_note_converts_to_one_that_reads_the_same() {
    let md = "---\ntitle: T\n---\n# T {draft}\n\nA <b>bold</b> claim, a <br> and <https://x.org>.\n\n<!-- later -->\n\n    indented {code}\n\n[[Home]] ==mark== #tag\n\n> [!tip] Hint\n> body\n";
    let note = to_note(md).unwrap();
    assert!(
        note.contains("# T \\{draft\\}")
            && note.contains("%% later %%")
            && note.contains("```\nindented {code}\n```"),
        "{note}"
    );
    // What a reader sees stays, minus the raw HTML's markup.
    let before = render_note(md, Flavour::Md, &Vault).unwrap();
    let after = render_note(&note, Flavour::Mdx, &Vault).unwrap();
    assert!(after.problems.is_empty(), "{:?}", after.problems);
    assert_eq!(before.headings, after.headings);
    assert_eq!(before.links, after.links);
    assert_eq!(before.properties, after.properties);
}

#[test]
fn a_note_that_will_not_convert_says_where() {
    let e = to_note(&"x".repeat(losos_mdx::MAX_SOURCE + 1)).unwrap_err();
    assert!(matches!(e, losos_mdx::Error::TooLarge { .. }));
}
