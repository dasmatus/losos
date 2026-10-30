//! Pages: blocks placed by MDX, and what a page may not do.

use losos_mdx::page::{blocks_used, render_page, Registry};

const REGISTRY: &str = r#"{
  "pages": {
    "overview": {
      "title": "Overview",
      "path": "/",
      "blocks": {
        "BoxName": { "about": "The name" },
        "Apps": {
          "props": {
            "title": { "type": "text" },
            "size": { "type": "choice", "values": ["small", "large"] },
            "columns": { "type": "number" },
            "compact": { "type": "flag" },
            "more": { "type": "url" }
          }
        },
        "Section": { "children": true, "props": { "label": { "type": "text", "required": true } } }
      }
    }
  }
}"#;

fn page(source: &str) -> losos_mdx::Rendered {
    let registry = Registry::from_json(REGISTRY).unwrap();
    render_page(source, registry.page("overview").unwrap()).unwrap()
}

fn messages(r: &losos_mdx::Rendered) -> Vec<String> {
    r.problems.iter().map(|p| p.message.clone()).collect()
}

#[test]
fn a_default_page_is_its_placeholders_and_nothing_else() {
    let r = page("<BoxName />\n\n<Apps />\n");
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert_eq!(
        r.html,
        "<losos-block data-block=\"BoxName\" data-props=\"{}\"></losos-block>\
         <losos-block data-block=\"Apps\" data-props=\"{}\"></losos-block>"
    );
    assert_eq!(blocks_used(&r), ["BoxName", "Apps"]);
}

#[test]
fn text_between_blocks_is_wrapped_as_a_document() {
    let r = page(
        "# Hello\n\nSee [storage](/storage).\n\n<Apps />\n\n<Callout type=\"tip\">Hi</Callout>\n",
    );
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert!(
        r.html
            .starts_with("<div class=\"mdx-document mdx-page-text\"><h1 id=\"hello\">Hello</h1>"),
        "{}",
        r.html
    );
    assert!(
        r.html
            .contains("<a href=\"/storage\" rel=\"noopener noreferrer\">storage</a>"),
        "{}",
        r.html
    );
    assert!(
        r.html.contains("</div><losos-block data-block=\"Apps\""),
        "{}",
        r.html
    );
    assert!(
        r.html.ends_with(
            "<aside data-callout=\"tip\" class=\"callout callout-tip\">Hi</aside></div>"
        ),
        "{}",
        r.html
    );
}

#[test]
fn front_matter_alone_makes_no_wrapper() {
    let r = page("---\ntitle: x\n---\n\n<Apps />\n");
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert_eq!(
        r.html,
        "<losos-block data-block=\"Apps\" data-props=\"{}\"></losos-block>"
    );
}

#[test]
fn props_are_checked_and_carried_as_json() {
    let r = page("<Apps title=\"Mine\" size=\"large\" columns=\"4\" compact more=\"/apps\" />\n");
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert!(
        r.html.contains(
            "data-props=\"{&quot;columns&quot;:&quot;4&quot;,&quot;compact&quot;:true,&quot;more&quot;:&quot;/apps&quot;,&quot;size&quot;:&quot;large&quot;,&quot;title&quot;:&quot;Mine&quot;}\""
        ),
        "{}",
        r.html
    );
}

#[test]
fn a_bad_prop_is_dropped_with_a_problem() {
    let r = page(
        "<Apps size=\"huge\" columns=\"0\" compact=\"yes\" more=\"javascript:alert(1)\" colour=\"red\" title={x} {...y} />\n",
    );
    let m = messages(&r);
    assert!(
        m.iter().any(|m| m.contains("size is one of small, large")),
        "{m:?}"
    );
    assert!(
        m.iter().any(|m| m.contains("columns is a whole number")),
        "{m:?}"
    );
    assert!(
        m.iter()
            .any(|m| m.contains("compact is written without a value")),
        "{m:?}"
    );
    assert!(m.iter().any(|m| m.contains("more is a link")), "{m:?}");
    assert!(m.iter().any(|m| m.contains("has no prop colour")), "{m:?}");
    assert!(m.iter().any(|m| m.contains("not an expression")), "{m:?}");
    assert!(m.iter().any(|m| m.contains("spread props")), "{m:?}");
    // Drawn with nothing the checks refused.
    assert!(r.html.contains("data-props=\"{}\""), "{}", r.html);
    assert!(!r.html.contains("javascript"), "{}", r.html);
}

#[test]
fn quotes_in_a_prop_stay_inside_the_attribute() {
    let r = page("<Apps title='\"></losos-block><script>alert(1)</script>' />\n");
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert!(!r.html.contains("<script"), "{}", r.html);
    assert_eq!(r.html.matches("</losos-block>").count(), 1, "{}", r.html);
}

#[test]
fn a_block_without_its_required_prop_is_not_drawn() {
    let r = page("<Section>\n\ninside\n\n</Section>\n");
    assert!(messages(&r)
        .iter()
        .any(|m| m.contains("<Section> needs label")));
    assert!(!r.html.contains("losos-block"), "{}", r.html);
    assert!(r.html.contains("inside"), "{}", r.html);
}

#[test]
fn a_block_may_hold_text_when_it_takes_children() {
    let r = page("<Section label=\"A\">\n\nSome **text**.\n\n<Apps />\n\n</Section>\n");
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert!(r.html.contains("<losos-block data-block=\"Section\" data-props=\"{&quot;label&quot;:&quot;A&quot;}\"><p>Some <strong>text</strong>.</p><losos-block data-block=\"Apps\""), "{}", r.html);
}

#[test]
fn a_block_that_takes_nothing_drops_what_is_inside() {
    let r = page("<Apps>\n\nhidden\n\n</Apps>\n");
    assert!(messages(&r)
        .iter()
        .any(|m| m.contains("takes nothing inside it")));
    assert!(!r.html.contains("hidden"), "{}", r.html);
}

#[test]
fn a_block_inside_a_sentence_is_refused() {
    let r = page("Look: <Apps /> here\n");
    assert!(messages(&r)
        .iter()
        .any(|m| m.contains("goes on a line of its own")));
    assert!(!r.html.contains("losos-block"), "{}", r.html);
}

#[test]
fn blocks_go_inside_document_components() {
    let r = page(
        "<Columns>\n<Column>\n<Apps />\n</Column>\n<Column>\n<BoxName />\n</Column>\n</Columns>\n",
    );
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert!(
        r.html.contains(
            "<div class=\"mdx-columns\"><div class=\"mdx-column\"><losos-block data-block=\"Apps\""
        ),
        "{}",
        r.html
    );
}

#[test]
fn an_author_cannot_write_the_placeholder_or_an_unknown_element() {
    let r = page(
        "<losos-block data-block=\"Apps\">x</losos-block>\n\n<Widget />\n\n<losos:block block=\"Apps\" />\n\n<script>alert(1)</script>\n",
    );
    let m = messages(&r);
    assert!(
        m.iter().any(|m| m.contains("<losos-block> is not a block")),
        "{m:?}"
    );
    assert!(
        m.iter().any(|m| m.contains("<Widget> is not a block")),
        "{m:?}"
    );
    assert!(m.iter().any(|m| m.contains("reserved for LosOS")), "{m:?}");
    assert!(!r.html.contains("losos-block"), "{}", r.html);
    assert!(!r.html.contains("<script"), "{}", r.html);
}

#[test]
fn imports_exports_and_expressions_are_refused() {
    let r = page("import x from \"y\"\n\nexport const a = 1\n\n{alert(1)}\n\n<Apps />\n");
    let m = messages(&r);
    assert!(
        m.iter().filter(|m| m.contains("import and export")).count() >= 2,
        "{m:?}"
    );
    assert!(
        m.iter().any(|m| m.contains("expressions in braces")),
        "{m:?}"
    );
}

#[test]
fn the_registry_refuses_names_that_are_not_plain() {
    assert!(Registry::from_json(r#"{"pages":{"Bad Page":{"title":"x"}}}"#).is_err());
    assert!(
        Registry::from_json(r#"{"pages":{"a":{"title":"x","blocks":{"Callout":{}}}}}"#)
            .unwrap_err()
            .contains("document component")
    );
    assert!(Registry::from_json(r#"{"pages":{"a":{"title":"x","blocks":{"lower":{}}}}}"#).is_err());
    assert!(Registry::from_json(r#"{"pages":{"settings.storage":{"title":"x"}}}"#).is_ok());
    assert!(Registry::from_json(r#"{"pages":{"settings/storage":{"title":"x"}}}"#).is_err());
    assert!(Registry::from_json(r#"{"pages":{"a..b":{"title":"x"}}}"#).is_err());
}

#[test]
fn a_button_is_a_plain_link_with_our_classes() {
    let r = page(
        "<Button href=\"/storage\" variant=\"secondary\">Open storage</Button>\n\nOr <Button href=\"https://example.com\" size=\"sm\">a site</Button> inline.\n\n<Button href=\"javascript:alert(1)\">x</Button>\n",
    );
    assert!(r.html.contains("<p><a href=\"/storage\" class=\"mdx-button mdx-button-secondary mdx-button-md\" rel=\"noopener noreferrer\">Open storage</a></p>"), "{}", r.html);
    assert!(r.html.contains("<a href=\"https://example.com\" class=\"mdx-button mdx-button-primary mdx-button-sm\" rel=\"noopener noreferrer\">a site</a> inline."), "{}", r.html);
    assert!(!r.html.contains("javascript"), "{}", r.html);
    assert!(messages(&r)
        .iter()
        .any(|m| m.contains("not a link a page may follow")));
    let r = page("<Button href=\"/x\" variant=\"destructive\" onClick=\"x\">y</Button>\n");
    let m = messages(&r);
    assert!(m.iter().any(|m| m.contains("variant is one of")), "{m:?}");
    assert!(m.iter().any(|m| m.contains("has no prop onClick")), "{m:?}");
    assert!(!r.html.contains("onclick"), "{}", r.html);
}
