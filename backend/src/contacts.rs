//! The people pickers of Mail, Chat and LosOS Contacts: the caller's
//! contacts and the people on this box, best match first
//! (`GET /api/contacts/suggest`).
//!
//! The contacts themselves live in LosOS Cloud's address books
//! ([`crate::cloud::contacts`], served to phones over CardDAV and to the
//! Contacts page under `/cloud/api/contacts/`). lososd asks LosOS Cloud for
//! the book's matches over its socket, as the person
//! ([`crate::pins::CloudRequest::ContactMatches`]), and adds the people on
//! the box it knows from LosOS Login, chat and mail. A box person who is
//! also a contact (by Matrix ID) is offered once, as the contact.

use crate::cloud::contacts::{folded, rank, Matches, Suggestion, Want};
use crate::losos::Losos;

/// Someone on this box, for [`suggest`]: built by the HTTP layer from
/// LosOS Login's people, chat's server name, mail's domain and the
/// profiles.
#[derive(Debug, Clone, Default)]
pub struct Neighbour {
    pub name: String,
    pub display_name: String,
    pub email: Option<String>,
    /// Where chat runs on this box.
    pub matrix_id: Option<String>,
    pub picture: Option<String>,
}

/// The book's matches for `q`, from LosOS Cloud as `login`. An empty book
/// when LosOS Cloud does not answer: the people on the box are still
/// offered.
pub fn book_matches(l: &mut impl Losos, login: &str, q: &str, want: Want) -> Matches {
    if want == Want::Box {
        return Matches::default();
    }
    let want_word = match want {
        Want::Any => "any",
        Want::Email => "email",
        Want::Matrix => "matrix",
        Want::Mesh => "mesh",
        Want::Box => "box",
    };
    let request = crate::pins::CloudRequest::ContactMatches {
        user: login,
        query: q,
        want: want_word,
    };
    match l.cloud_request(&request) {
        Ok((200, body)) => serde_json::from_slice(&body).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "LosOS Cloud's contact matches did not read");
            Matches::default()
        }),
        Ok((status, _)) => {
            tracing::warn!(status, "LosOS Cloud refused the contact matches");
            Matches::default()
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not reach LosOS Cloud for contacts");
            Matches::default()
        }
    }
}

/// The people a picker offers for `q`: `book`'s matches and the people on
/// this box but `me`, best match first, at most `limit` (1 to 50).
pub fn suggest(
    book: Matches,
    neighbours: &[Neighbour],
    me: &str,
    q: &str,
    want: Want,
    limit: usize,
) -> Vec<Suggestion> {
    let q = folded(q.trim());
    let known: std::collections::HashSet<&str> =
        book.known_matrix_ids.iter().map(String::as_str).collect();
    let mut ranked: Vec<(u8, Suggestion)> = book
        .matches
        .iter()
        .map(|r| (r.rank, r.suggestion.clone()))
        .collect();
    for n in neighbours {
        if n.name == me
            || (want != Want::Box && n.matrix_id.as_deref().is_some_and(|m| known.contains(m)))
        {
            continue;
        }
        let fits = match want {
            Want::Any | Want::Box => true,
            Want::Matrix => n.matrix_id.is_some(),
            Want::Email => n.email.is_some(),
            // Sharing with someone on the same box is LosOS Cloud's own
            // share, not a mesh address.
            Want::Mesh => false,
        };
        if !fits {
            continue;
        }
        let shown = if n.display_name.trim().is_empty() {
            n.name.clone()
        } else {
            n.display_name.trim().to_string()
        };
        let email = n.email.clone().unwrap_or_default();
        let matrix = n.matrix_id.clone().unwrap_or_default();
        let Some(r) = rank(&q, &[&shown, &n.name, &email, &matrix]) else {
            continue;
        };
        ranked.push((
            r,
            Suggestion {
                key: format!("box:{}", n.name),
                source: "box".into(),
                name: shown,
                picture: n.picture.clone(),
                emails: n.email.iter().cloned().collect(),
                matrix_id: n.matrix_id.clone(),
                mesh_address: None,
                contact_id: None,
            },
        ));
    }
    ranked.sort_by(|(ra, a), (rb, b)| {
        ra.cmp(rb)
            .then_with(|| folded(&a.name).cmp(&folded(&b.name)))
            .then_with(|| a.key.cmp(&b.key))
    });
    ranked
        .into_iter()
        .take(limit.clamp(1, 50))
        .map(|(_, s)| s)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::contacts::Ranked;

    fn contact(id: &str, name: &str, rank: u8, matrix: Option<&str>) -> Ranked {
        Ranked {
            rank,
            suggestion: Suggestion {
                key: format!("contact:{id}"),
                source: "contact".into(),
                name: name.into(),
                picture: None,
                emails: vec![],
                matrix_id: matrix.map(str::to_string),
                mesh_address: None,
                contact_id: Some(id.into()),
            },
        }
    }

    fn neighbours() -> Vec<Neighbour> {
        vec![
            Neighbour {
                name: "eva".into(),
                matrix_id: Some("@eva:box".into()),
                ..Neighbour::default()
            },
            Neighbour {
                name: "jan".into(),
                display_name: "Jan".into(),
                matrix_id: Some("@jan:box".into()),
                email: Some("jan@box.home.arpa".into()),
                ..Neighbour::default()
            },
            Neighbour {
                name: "stano".into(),
                display_name: "Stano".into(),
                matrix_id: Some("@stano:box".into()),
                email: Some("stano@box.home.arpa".into()),
                picture: Some("/p".into()),
            },
        ]
    }

    fn names(book: Matches, q: &str, want: Want) -> Vec<String> {
        suggest(book, &neighbours(), "eva", q, want, 10)
            .into_iter()
            .map(|s| s.name)
            .collect()
    }

    #[test]
    fn the_book_and_the_box_are_ranked_together_and_a_contact_shows_once() {
        let book = Matches {
            matches: vec![
                contact("1", "Ján Štefan", 1, Some("@jan:box")),
                contact("2", "Stefánia", 0, None),
            ],
            known_matrix_ids: vec!["@jan:box".into()],
        };
        // "st": Stefánia and Stano start with it, Ján Štefan has a word that does.
        assert_eq!(
            names(book.clone(), "st", Want::Any),
            ["Stano", "Stefánia", "Ján Štefan"]
        );
        // Jan on the box is a contact already: offered once, as the contact,
        // even when the book did not match the query.
        let none = Matches {
            matches: vec![],
            known_matrix_ids: vec!["@jan:box".into()],
        };
        assert_eq!(names(none.clone(), "", Want::Matrix), ["Stano"]);
        // The book's "On this box" view: everyone else on the box.
        assert_eq!(names(none, "", Want::Box), ["Jan", "Stano"]);
        assert!(names(Matches::default(), "", Want::Mesh).is_empty());
        let s = &suggest(
            Matches::default(),
            &neighbours(),
            "eva",
            "stano",
            Want::Any,
            10,
        )[0];
        assert_eq!(s.source, "box");
        assert_eq!(s.key, "box:stano");
        assert_eq!(s.emails, ["stano@box.home.arpa"]);
    }

    #[test]
    fn without_chat_the_box_has_no_matrix_ids_to_offer() {
        let mut people = neighbours();
        for p in &mut people {
            p.matrix_id = None;
        }
        let picked = |want| -> Vec<String> {
            suggest(Matches::default(), &people, "eva", "", want, 10)
                .into_iter()
                .map(|s| s.name)
                .collect()
        };
        assert!(picked(Want::Matrix).is_empty());
        assert_eq!(picked(Want::Email), ["Jan", "Stano"]);
    }

    #[test]
    fn the_book_is_asked_as_the_person_and_a_silent_cloud_is_an_empty_book() {
        let mut fake = crate::fake::FakeLosos::default();
        fake.cloud_answers.push_back((
            200,
            serde_json::to_vec(&Matches {
                matches: vec![contact("7", "Eva Nováková", 0, None)],
                known_matrix_ids: vec![],
            })
            .unwrap(),
        ));
        let got = book_matches(&mut fake, "notshared", "eva nov", Want::Email);
        assert_eq!(got.matches.len(), 1);
        assert_eq!(fake.cloud_calls, ["contacts notshared email eva nov"]);
        assert_eq!(
            book_matches(&mut fake, "notshared", "x", Want::Any),
            Matches::default()
        );
        // "On this box" needs no book.
        assert_eq!(
            book_matches(&mut fake, "notshared", "", Want::Box),
            Matches::default()
        );
        assert_eq!(fake.cloud_calls.len(), 2);
    }
}
