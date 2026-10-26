//! Which issues a list shows: the filter a page sends (and a saved view
//! keeps), applied to the issues of the projects the caller may read, and
//! the order they come back in.

use super::store::{Cycle, Ticket};
use super::{priority_rank, Category};
use serde::{Deserialize, Serialize};

/// Every field narrows the list; one left out does not. Lists are any of.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filter {
    /// Project keys.
    pub project: Vec<String>,
    /// Categories: `backlog`, `unstarted`, `started`, `done`, `canceled`;
    /// `active` is the first three.
    pub category: Vec<String>,
    /// State ids.
    pub state: Vec<i64>,
    /// Logins; `me`, and `none` for nobody.
    pub assignee: Vec<String>,
    pub creator: Vec<String>,
    /// 0 to 4.
    pub priority: Vec<i64>,
    /// Label names, any of them.
    pub label: Vec<String>,
    /// Cycle ids; `current`, and `none` for no cycle.
    pub cycle: Vec<String>,
    /// A parent's key: its sub-issues. `none`: issues with no parent.
    pub parent: Option<String>,
    /// `overdue`, `week` (due in the next seven days) or `any`.
    pub due: Option<String>,
    /// Words in the title or description, or a key.
    pub q: Option<String>,
    /// `manual` (the default), `priority`, `updated`, `created`, `due`.
    pub sort: Option<String>,
    /// Issue keys: these issues only (the sidebar's pins).
    pub key: Vec<String>,
    /// At most this many, after sorting.
    pub limit: Option<usize>,
}

/// The query string's form: lists comma separated.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FilterQuery {
    pub project: Option<String>,
    pub category: Option<String>,
    pub state: Option<String>,
    pub assignee: Option<String>,
    pub creator: Option<String>,
    pub priority: Option<String>,
    pub label: Option<String>,
    pub cycle: Option<String>,
    pub parent: Option<String>,
    pub due: Option<String>,
    pub q: Option<String>,
    pub sort: Option<String>,
    pub key: Option<String>,
    pub limit: Option<String>,
}

fn list(s: &Option<String>) -> Vec<String> {
    s.as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

impl FilterQuery {
    pub fn into_filter(self) -> Result<Filter, String> {
        let ints = |s: &Option<String>, what: &str| -> Result<Vec<i64>, String> {
            list(s)
                .iter()
                .map(|v| v.parse().map_err(|_| format!("\"{v}\" is not a {what}.")))
                .collect()
        };
        Ok(Filter {
            project: list(&self.project)
                .into_iter()
                .map(|k| k.to_ascii_uppercase())
                .collect(),
            category: list(&self.category),
            state: ints(&self.state, "state")?,
            assignee: list(&self.assignee),
            creator: list(&self.creator),
            priority: ints(&self.priority, "priority")?,
            label: list(&self.label),
            cycle: list(&self.cycle),
            parent: self.parent.filter(|p| !p.is_empty()),
            due: self.due.filter(|d| !d.is_empty()),
            q: self.q.filter(|q| !q.trim().is_empty()),
            sort: self.sort.filter(|s| !s.is_empty()),
            key: list(&self.key)
                .into_iter()
                .map(|k| k.to_ascii_uppercase())
                .collect(),
            limit: match self.limit.as_deref().map(str::trim) {
                None | Some("") => None,
                Some(n) => Some(
                    n.parse::<usize>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or_else(|| format!("\"{n}\" is not a number of issues."))?,
                ),
            },
        })
    }
}

/// What a filter is matched against beside the issue.
pub struct Context<'a> {
    /// The caller's login, for `me`.
    pub me: Option<&'a str>,
    /// Today, `YYYY-MM-DD`.
    pub today: &'a str,
    /// Every cycle of the projects in the list.
    pub cycles: &'a [Cycle],
}

impl Filter {
    pub fn matches(&self, t: &Ticket, project_key: &str, cx: &Context<'_>) -> bool {
        if !self.project.is_empty() && !self.project.iter().any(|k| k == project_key) {
            return false;
        }
        if !self.key.is_empty() && !self.key.iter().any(|k| k.eq_ignore_ascii_case(&t.key)) {
            return false;
        }
        if !self.category.is_empty() {
            let c = t.state.category;
            let hit = self.category.iter().any(|want| match want.as_str() {
                "active" => !c.closed(),
                "open" => !c.closed() && c != Category::Backlog,
                other => Category::parse(other) == Some(c),
            });
            if !hit {
                return false;
            }
        }
        if !self.state.is_empty() && !self.state.contains(&t.state.id) {
            return false;
        }
        let person = |want: &[String], who: Option<&str>| {
            want.is_empty()
                || want.iter().any(|w| match w.as_str() {
                    "none" => who.is_none(),
                    "me" => who.is_some() && who == cx.me,
                    login => who.is_some_and(|x| x.eq_ignore_ascii_case(login)),
                })
        };
        if !person(&self.assignee, t.assignee.as_deref()) {
            return false;
        }
        if !person(&self.creator, Some(t.creator.as_str())) {
            return false;
        }
        if !self.priority.is_empty() && !self.priority.contains(&t.priority) {
            return false;
        }
        if !self.label.is_empty()
            && !self
                .label
                .iter()
                .any(|l| t.labels.iter().any(|x| x.name.eq_ignore_ascii_case(l)))
        {
            return false;
        }
        if !self.cycle.is_empty() {
            let hit = self.cycle.iter().any(|c| match c.as_str() {
                "none" => t.cycle_id.is_none(),
                "current" => t.cycle_id.is_some_and(|id| {
                    cx.cycles.iter().any(|cy| {
                        cy.id == id
                            && cy.starts.as_str() <= cx.today
                            && cx.today <= cy.ends.as_str()
                    })
                }),
                id => id.parse::<i64>().ok() == t.cycle_id,
            });
            if !hit {
                return false;
            }
        }
        let parent_ok = match self.parent.as_deref() {
            None => true,
            Some("none") => t.parent_id.is_none(),
            Some(key) => t
                .parent_key
                .as_deref()
                .is_some_and(|k| k.eq_ignore_ascii_case(key)),
        };
        if !parent_ok {
            return false;
        }
        match self.due.as_deref() {
            None | Some("") => {}
            Some("any") => {
                if t.due.is_none() {
                    return false;
                }
            }
            Some("overdue") => {
                if !t.due.as_deref().is_some_and(|d| d < cx.today) || t.state.category.closed() {
                    return false;
                }
            }
            Some("week") => {
                let end = chrono::NaiveDate::parse_from_str(cx.today, "%Y-%m-%d")
                    .map(|d| {
                        (d + chrono::Duration::days(7))
                            .format("%Y-%m-%d")
                            .to_string()
                    })
                    .unwrap_or_default();
                if !t
                    .due
                    .as_deref()
                    .is_some_and(|d| d >= cx.today && d <= end.as_str())
                {
                    return false;
                }
            }
            Some(_) => {}
        }
        if let Some(q) = &self.q {
            let q = q.trim();
            if !(t.key.eq_ignore_ascii_case(q) || words_in(q, t)) {
                return false;
            }
        }
        true
    }

    /// Put `list` in the asked order. The manual order is the board's and
    /// the backlog's; the others break ties by the manual order too.
    pub fn sort(&self, list: &mut [Ticket]) {
        let manual = |a: &Ticket, b: &Ticket| {
            a.sort
                .partial_cmp(&b.sort)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.id.cmp(&a.id))
        };
        match self.sort.as_deref() {
            Some("priority") => list.sort_by(|a, b| {
                priority_rank(a.priority)
                    .cmp(&priority_rank(b.priority))
                    .then_with(|| manual(a, b))
            }),
            Some("updated") => {
                list.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| manual(a, b)))
            }
            Some("created") => {
                list.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| manual(a, b)))
            }
            Some("due") => list.sort_by(|a, b| match (&a.due, &b.due) {
                (Some(x), Some(y)) => x.cmp(y).then_with(|| manual(a, b)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => manual(a, b),
            }),
            _ => list.sort_by(manual),
        }
    }
}

/// Every word of `q` is in the title or the description.
fn words_in(q: &str, t: &Ticket) -> bool {
    let hay = format!("{} {}", t.title, t.body).to_lowercase();
    q.to_lowercase().split_whitespace().all(|w| hay.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::tracker::store::{Label, WorkflowState};

    fn ticket(id: i64, cat: Category, prio: i64, assignee: Option<&str>) -> Ticket {
        Ticket {
            id,
            project_id: 1,
            key: format!("LOS-{id}"),
            number: id,
            title: format!("Issue {id} about backups"),
            body: String::new(),
            state: WorkflowState {
                id: cat as i64,
                project_id: 1,
                name: cat.as_str().into(),
                category: cat,
                color: String::new(),
                position: 0,
            },
            priority: prio,
            estimate: None,
            assignee: assignee.map(str::to_string),
            creator: "matus".into(),
            cycle_id: None,
            parent_id: None,
            parent_key: None,
            due: None,
            sort: -(id as f64),
            repo: None,
            issue_id: None,
            labels: vec![Label {
                id: 1,
                name: "bug".into(),
                color: String::new(),
                description: String::new(),
            }],
            comments: 0,
            children: 0,
            children_closed: 0,
            created: format!("2026-10-{id:02}"),
            updated: format!("2026-10-{id:02}"),
            started_at: None,
            completed_at: None,
            canceled_at: None,
        }
    }

    fn cx() -> Context<'static> {
        Context {
            me: Some("eva"),
            today: "2026-10-22",
            cycles: &[],
        }
    }

    #[test]
    fn my_active_issues() {
        let f = FilterQuery {
            assignee: Some("me".into()),
            category: Some("active".into()),
            ..Default::default()
        }
        .into_filter()
        .unwrap();
        assert!(f.matches(&ticket(1, Category::Started, 0, Some("eva")), "LOS", &cx()));
        assert!(!f.matches(&ticket(2, Category::Done, 0, Some("eva")), "LOS", &cx()));
        assert!(!f.matches(
            &ticket(3, Category::Started, 0, Some("matus")),
            "LOS",
            &cx()
        ));
        assert!(!f.matches(&ticket(4, Category::Started, 0, None), "LOS", &cx()));
    }

    #[test]
    fn words_keys_and_labels() {
        let f = Filter {
            q: Some("LOS-3".into()),
            ..Default::default()
        };
        assert!(f.matches(&ticket(3, Category::Backlog, 0, None), "LOS", &cx()));
        assert!(!f.matches(&ticket(4, Category::Backlog, 0, None), "LOS", &cx()));
        let f = Filter {
            q: Some("about BACKUPS".into()),
            label: vec!["Bug".into()],
            ..Default::default()
        };
        assert!(f.matches(&ticket(4, Category::Backlog, 0, None), "LOS", &cx()));
        assert!(FilterQuery {
            priority: Some("x".into()),
            ..Default::default()
        }
        .into_filter()
        .is_err());
    }

    #[test]
    fn a_list_of_keys_picks_those_issues_and_a_limit_is_a_number() {
        let f = FilterQuery {
            key: Some("los-3, LOS-5".into()),
            limit: Some("2".into()),
            ..Default::default()
        }
        .into_filter()
        .unwrap();
        assert_eq!(f.limit, Some(2));
        assert!(f.matches(&ticket(3, Category::Done, 0, None), "LOS", &cx()));
        assert!(f.matches(&ticket(5, Category::Backlog, 0, None), "LOS", &cx()));
        assert!(!f.matches(&ticket(4, Category::Backlog, 0, None), "LOS", &cx()));
        for bad in ["0", "-1", "many"] {
            assert!(FilterQuery {
                limit: Some(bad.into()),
                ..Default::default()
            }
            .into_filter()
            .is_err());
        }
    }

    #[test]
    fn priority_order_puts_urgent_first_and_none_last() {
        let mut l = vec![
            ticket(1, Category::Started, 0, None),
            ticket(2, Category::Started, 4, None),
            ticket(3, Category::Started, 1, None),
        ];
        Filter {
            sort: Some("priority".into()),
            ..Default::default()
        }
        .sort(&mut l);
        assert_eq!(l.iter().map(|t| t.id).collect::<Vec<_>>(), [3, 2, 1]);
    }

    #[test]
    fn overdue_is_open_and_past() {
        let f = Filter {
            due: Some("overdue".into()),
            ..Default::default()
        };
        let mut t = ticket(1, Category::Started, 0, None);
        t.due = Some("2026-10-20".into());
        assert!(f.matches(&t, "LOS", &cx()));
        t.state.category = Category::Done;
        assert!(!f.matches(&t, "LOS", &cx()));
    }
}
