//! The elements a document may use: LosOS's components and a few plain
//! HTML ones. Anything else is refused. A component is markup, never code:
//! its props are strings checked here, and the HTML each one becomes is
//! written by `render`, not by the document.

/// What a prop may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    /// Any text, written out escaped.
    Text,
    /// One of these words.
    Choice(&'static [&'static str]),
    /// A link: http, https, mailto, or a path the resolver takes.
    Url,
    /// A picture or other file, resolved like a Markdown image.
    File,
    /// Present or not, with no value (`<Details open>`).
    Flag,
    /// A whole number from 1 to 1000 (a cell's span).
    Number,
    /// A date, `YYYY-MM-DD`.
    Date,
}

impl Prop {
    /// Whether `value` is one this prop takes. Flags take none.
    pub fn accepts(self, value: &str) -> bool {
        match self {
            Prop::Text | Prop::Url | Prop::File => true,
            Prop::Choice(words) => words.contains(&value),
            Prop::Flag => false,
            Prop::Number => {
                value.len() <= 4
                    && value.bytes().all(|b| b.is_ascii_digit())
                    && value.parse::<u32>().is_ok_and(|n| (1..=1000).contains(&n))
            }
            Prop::Date => {
                let b = value.as_bytes();
                b.len() == 10
                    && b[4] == b'-'
                    && b[7] == b'-'
                    && b.iter()
                        .enumerate()
                        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
            }
        }
    }
}

/// One element a document may write.
#[derive(Debug)]
pub struct Component {
    pub name: &'static str,
    pub props: &'static [(&'static str, Prop)],
    /// Props without which the element is drawn as its children alone.
    pub required: &'static [&'static str],
}

impl Component {
    pub fn prop(&self, name: &str) -> Option<Prop> {
        self.props.iter().find(|(n, _)| *n == name).map(|(_, p)| *p)
    }
}

const CALLOUT_TYPES: &[&str] = &["note", "tip", "important", "warning", "caution"];
const SLIDE_LAYOUTS: &[&str] = &["title", "content", "two-columns", "picture"];
const CHART_TYPES: &[&str] = &["bar", "line", "area", "pie"];
const ALIGN: &[&str] = &["left", "center", "right"];
const DATE_FORMATS: &[&str] = &["short", "long"];
const BUTTON_VARIANTS: &[&str] = &["primary", "secondary", "outline", "ghost", "link"];
const BUTTON_SIZES: &[&str] = &["sm", "md", "lg"];

/// LosOS's components, in README.md's order.
pub const COMPONENTS: &[Component] = &[
    Component {
        name: "Callout",
        props: &[("type", Prop::Choice(CALLOUT_TYPES)), ("title", Prop::Text)],
        required: &[],
    },
    Component {
        name: "Details",
        props: &[("summary", Prop::Text), ("open", Prop::Flag)],
        required: &[],
    },
    Component {
        name: "Columns",
        props: &[],
        required: &[],
    },
    Component {
        name: "Column",
        props: &[],
        required: &[],
    },
    Component {
        name: "Tabs",
        props: &[],
        required: &[],
    },
    Component {
        name: "Tab",
        props: &[("label", Prop::Text)],
        required: &["label"],
    },
    Component {
        name: "Figure",
        props: &[
            ("src", Prop::File),
            ("alt", Prop::Text),
            ("caption", Prop::Text),
        ],
        required: &["src"],
    },
    Component {
        name: "Embed",
        props: &[("href", Prop::Url), ("title", Prop::Text)],
        required: &["href"],
    },
    Component {
        name: "Button",
        props: &[
            ("href", Prop::Url),
            ("variant", Prop::Choice(BUTTON_VARIANTS)),
            ("size", Prop::Choice(BUTTON_SIZES)),
        ],
        required: &["href"],
    },
    Component {
        name: "Slide",
        props: &[("layout", Prop::Choice(SLIDE_LAYOUTS))],
        required: &[],
    },
    Component {
        name: "Notes",
        props: &[],
        required: &[],
    },
    Component {
        name: "PageBreak",
        props: &[],
        required: &[],
    },
    Component {
        name: "TableOfContents",
        props: &[("title", Prop::Text)],
        required: &[],
    },
    Component {
        name: "Header",
        props: &[],
        required: &[],
    },
    Component {
        name: "Footer",
        props: &[],
        required: &[],
    },
    Component {
        name: "PageNumber",
        props: &[],
        required: &[],
    },
    Component {
        name: "Table",
        props: &[("caption", Prop::Text)],
        required: &[],
    },
    Component {
        name: "Row",
        props: &[("header", Prop::Flag)],
        required: &[],
    },
    Component {
        name: "Cell",
        props: &[
            ("colSpan", Prop::Number),
            ("rowSpan", Prop::Number),
            ("align", Prop::Choice(ALIGN)),
        ],
        required: &[],
    },
    Component {
        name: "Comment",
        props: &[
            ("author", Prop::Text),
            ("date", Prop::Date),
            ("text", Prop::Text),
        ],
        required: &["text"],
    },
    Component {
        name: "Equation",
        props: &[("tex", Prop::Text), ("inline", Prop::Flag)],
        required: &["tex"],
    },
    Component {
        name: "Chart",
        props: &[
            ("src", Prop::File),
            ("range", Prop::Text),
            ("type", Prop::Choice(CHART_TYPES)),
            ("title", Prop::Text),
        ],
        required: &["src"],
    },
    Component {
        name: "Mention",
        props: &[("login", Prop::Text)],
        required: &["login"],
    },
    Component {
        name: "Date",
        props: &[
            ("value", Prop::Date),
            ("format", Prop::Choice(DATE_FORMATS)),
        ],
        required: &["value"],
    },
];

/// Plain HTML elements a document may write in MDX, as themselves. Only
/// `abbr` takes an attribute.
pub const INTRINSIC: &[Component] = &[
    Component {
        name: "abbr",
        props: &[("title", Prop::Text)],
        required: &[],
    },
    Component {
        name: "br",
        props: &[],
        required: &[],
    },
    Component {
        name: "kbd",
        props: &[],
        required: &[],
    },
    Component {
        name: "mark",
        props: &[],
        required: &[],
    },
    Component {
        name: "s",
        props: &[],
        required: &[],
    },
    Component {
        name: "small",
        props: &[],
        required: &[],
    },
    Component {
        name: "sub",
        props: &[],
        required: &[],
    },
    Component {
        name: "sup",
        props: &[],
        required: &[],
    },
    Component {
        name: "u",
        props: &[],
        required: &[],
    },
];

/// The element a JSX name stands for, if a document may use it.
pub fn find(name: &str) -> Option<&'static Component> {
    COMPONENTS.iter().chain(INTRINSIC).find(|c| c.name == name)
}
