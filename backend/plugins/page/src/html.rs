//! Markup as values. [`Html`] is text that is already safe to put in a
//! page; [`El`] is one element under construction. Everything a person or
//! the box wrote goes in through [`Html::text`] or an attribute, which
//! escape it, so a name with `<` in it is a name, never markup.

use std::fmt::Write as _;

/// Markup that is safe to put in a page as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Html(String);

impl Html {
    #[must_use]
    pub fn new() -> Self {
        Html(String::new())
    }

    /// Markup this crate wrote itself: icons, fixed fragments. Never for
    /// anything that came from a request or the box.
    #[must_use]
    pub fn trusted(markup: impl Into<String>) -> Self {
        Html(markup.into())
    }

    /// Text, escaped.
    #[must_use]
    pub fn text(text: &str) -> Self {
        Html(escape(text))
    }

    pub fn push(&mut self, more: impl Into<Html>) {
        self.0.push_str(&more.into().0);
    }

    #[must_use]
    pub fn with(mut self, more: impl Into<Html>) -> Self {
        self.push(more);
        self
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for Html {
    fn from(text: &str) -> Self {
        Html::text(text)
    }
}

impl From<String> for Html {
    fn from(text: String) -> Self {
        Html::text(&text)
    }
}

impl From<&String> for Html {
    fn from(text: &String) -> Self {
        Html::text(text)
    }
}

impl From<El> for Html {
    fn from(el: El) -> Self {
        el.render()
    }
}

impl From<Option<Html>> for Html {
    fn from(maybe: Option<Html>) -> Self {
        maybe.unwrap_or_default()
    }
}

impl<T: Into<Html>> FromIterator<T> for Html {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut out = Html::new();
        for item in iter {
            out.push(item);
        }
        out
    }
}

/// `&`, `<`, `>`, `"` and `'` as entities: safe in text and in a quoted
/// attribute alike.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Elements with no closing tag.
const VOID: [&str; 7] = ["input", "img", "meta", "link", "br", "hr", "source"];

/// One element: `el("div").class("flex").child(...)`.
#[derive(Debug, Clone)]
pub struct El {
    tag: &'static str,
    classes: String,
    attrs: String,
    body: Html,
}

/// Start an element.
#[must_use]
pub fn el(tag: &'static str) -> El {
    El {
        tag,
        classes: String::new(),
        attrs: String::new(),
        body: Html::new(),
    }
}

impl El {
    /// Add classes. Called twice, both sets are kept.
    #[must_use]
    pub fn class(mut self, classes: &str) -> Self {
        let classes = classes.trim();
        if !classes.is_empty() {
            if !self.classes.is_empty() {
                self.classes.push(' ');
            }
            self.classes.push_str(classes);
        }
        self
    }

    /// Add classes when `on`.
    #[must_use]
    pub fn class_if(self, on: bool, classes: &str) -> Self {
        if on {
            self.class(classes)
        } else {
            self
        }
    }

    /// An attribute, its value escaped.
    #[must_use]
    pub fn attr(mut self, name: &str, value: &str) -> Self {
        let _ = write!(self.attrs, " {name}=\"{}\"", escape(value));
        self
    }

    /// An attribute when there is a value.
    #[must_use]
    pub fn attr_opt(self, name: &str, value: Option<&str>) -> Self {
        match value {
            Some(v) => self.attr(name, v),
            None => self,
        }
    }

    /// A boolean attribute (`checked`, `open`, `hidden`) when `on`.
    #[must_use]
    pub fn flag(mut self, name: &str, on: bool) -> Self {
        if on {
            self.attrs.push(' ');
            self.attrs.push_str(name);
        }
        self
    }

    #[must_use]
    pub fn id(self, id: &str) -> Self {
        self.attr("id", id)
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Html>) -> Self {
        self.body.push(child);
        self
    }

    #[must_use]
    pub fn text(self, text: &str) -> Self {
        self.child(Html::text(text))
    }

    #[must_use]
    pub fn children<T: Into<Html>>(mut self, children: impl IntoIterator<Item = T>) -> Self {
        for c in children {
            self.body.push(c);
        }
        self
    }

    /// Apply `f` when `on`, for the odd attribute or child that depends on
    /// something.
    #[must_use]
    pub fn when(self, on: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if on {
            f(self)
        } else {
            self
        }
    }

    #[must_use]
    pub fn render(self) -> Html {
        let mut out = String::with_capacity(self.body.0.len() + self.classes.len() + 32);
        out.push('<');
        out.push_str(self.tag);
        if !self.classes.is_empty() {
            let _ = write!(out, " class=\"{}\"", escape(&self.classes));
        }
        out.push_str(&self.attrs);
        out.push('>');
        if !VOID.contains(&self.tag) {
            out.push_str(&self.body.0);
            let _ = write!(out, "</{}>", self.tag);
        }
        Html(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_attributes_are_escaped() {
        let html: Html = el("a")
            .class("text-ink")
            .attr("href", "/x?a=1&b=\"2\"")
            .text("<b>Tom & 'Jerry'</b>")
            .into();
        assert_eq!(
            html.as_str(),
            "<a class=\"text-ink\" href=\"/x?a=1&amp;b=&quot;2&quot;\">&lt;b&gt;Tom &amp; &#39;Jerry&#39;&lt;/b&gt;</a>"
        );
    }

    #[test]
    fn void_elements_have_no_body() {
        let html: Html = el("input").attr("name", "q").flag("checked", true).into();
        assert_eq!(html.as_str(), "<input name=\"q\" checked>");
    }

    #[test]
    fn classes_join_and_skip_empty() {
        let html: Html = el("p")
            .class("a")
            .class("")
            .class_if(false, "b")
            .class_if(true, "c")
            .into();
        assert_eq!(html.as_str(), "<p class=\"a c\"></p>");
    }
}
