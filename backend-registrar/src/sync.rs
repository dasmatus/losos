//! The market's webhook sync: the one place a Stripe event changes the
//! ledger.
//!
//! A delivery is parsed once into a typed [`Delivery`], so the rules in
//! [`crate::market::apply_event`] read fields, not JSON paths. Two things
//! Stripe does not promise are handled here:
//!
//!   * **Duplicates.** Stripe may deliver one event more than once. Every
//!     event that changed the ledger is remembered by id ([`SyncLog`]), and a
//!     second delivery is dropped before any rule runs.
//!   * **Order.** Stripe does not deliver events in the order they happened.
//!     An `account.updated` carries the whole account as it was when the
//!     event was created, so an older one arriving late would undo a newer
//!     one: a seller who finished onboarding would drop off the shelf again.
//!     The sync keeps the creation time of the newest account state it
//!     applied and ignores anything older. Checkout events need no such
//!     guard: an order only moves forward (pending to paid or expired), and
//!     the rules refuse every step back.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::market::account_ready;

/// Event ids remembered. A redelivery comes within Stripe's retry horizon
/// (three days); far fewer than this many events change a ledger in that
/// time.
pub const MAX_APPLIED: usize = 1_024;

/// What a delivery says, by type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `checkout.session.completed`.
    CheckoutCompleted {
        session: Option<String>,
        /// `client_reference_id`, the order the edge created the session for.
        order: Option<String>,
        /// `payment_status` is `paid`. A delayed method (a bank debit) sends
        /// `completed` unpaid and settles later.
        paid: bool,
        amount: Option<u64>,
        currency: Option<String>,
    },
    /// `checkout.session.expired`.
    CheckoutExpired {
        session: Option<String>,
        order: Option<String>,
    },
    /// `account.updated`: a connected account and whether it can now receive
    /// transfers.
    AccountUpdated { account: String, ready: bool },
    /// Anything else, by its type. Stripe sends every type the endpoint
    /// subscribed to; none of the others changes the ledger.
    Other(String),
}

/// One webhook delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// `evt_...`, absent only on a hand-made event.
    pub id: Option<String>,
    /// When Stripe created the event, in Unix seconds.
    pub created: Option<u64>,
    pub event: Event,
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

impl Delivery {
    /// Read a verified event. Never fails: what cannot be read becomes
    /// [`Event::Other`] or an absent field, and the rules ignore it.
    #[must_use]
    pub fn parse(event: &Value) -> Self {
        let kind = event["type"].as_str().unwrap_or("");
        let object = &event["data"]["object"];
        let parsed = match kind {
            "checkout.session.completed" => Event::CheckoutCompleted {
                session: text(&object["id"]),
                order: text(&object["client_reference_id"]),
                paid: object["payment_status"].as_str() == Some("paid"),
                amount: object["amount_total"].as_u64(),
                currency: text(&object["currency"]),
            },
            "checkout.session.expired" => Event::CheckoutExpired {
                session: text(&object["id"]),
                order: text(&object["client_reference_id"]),
            },
            "account.updated" => match object["id"].as_str() {
                Some(account) => Event::AccountUpdated {
                    account: account.to_string(),
                    ready: account_ready(object),
                },
                None => Event::Other(kind.to_string()),
            },
            other => Event::Other(other.to_string()),
        };
        Self {
            id: text(&event["id"]),
            created: event["created"].as_u64(),
            event: parsed,
        }
    }
}

/// What the sync remembers between deliveries. Kept in the ledger, so a
/// restart forgets nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncLog {
    /// Ids of the events that changed the ledger, oldest first.
    #[serde(default)]
    applied: VecDeque<String>,
    /// For each connected account, the creation time of the newest state of
    /// it the ledger holds.
    #[serde(default)]
    accounts: BTreeMap<String, u64>,
}

impl SyncLog {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.applied.is_empty() && self.accounts.is_empty()
    }

    /// Whether the event `id` was already applied.
    #[must_use]
    pub fn applied(&self, id: &str) -> bool {
        self.applied.iter().any(|a| a == id)
    }

    /// Remember that the event `id` was applied.
    pub fn record(&mut self, id: &str) {
        if self.applied(id) {
            return;
        }
        if self.applied.len() >= MAX_APPLIED {
            self.applied.pop_front();
        }
        self.applied.push_back(id.to_string());
    }

    /// Whether a state of `account` created at `created` is older than the
    /// one the ledger holds. Two states in the same second are both applied,
    /// in the order they arrive.
    #[must_use]
    pub fn stale(&self, account: &str, created: u64) -> bool {
        self.accounts
            .get(account)
            .is_some_and(|&held| created < held)
    }

    /// Note that the ledger now holds the state of `account` as of `at`.
    /// Returns whether that moved the clock forward.
    pub fn saw_account(&mut self, account: &str, at: u64) -> bool {
        match self.accounts.get(account) {
            Some(&held) if held >= at => false,
            _ => {
                self.accounts.insert(account.to_string(), at);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_completed_session_is_read_field_by_field() {
        let d = Delivery::parse(&json!({
            "id": "evt_1",
            "created": 1_700_000_000,
            "type": "checkout.session.completed",
            "data": { "object": {
                "id": "cs_test_1",
                "client_reference_id": "ord_1",
                "payment_status": "paid",
                "amount_total": 2_000,
                "currency": "eur",
            }},
        }));
        assert_eq!(d.id.as_deref(), Some("evt_1"));
        assert_eq!(d.created, Some(1_700_000_000));
        assert_eq!(
            d.event,
            Event::CheckoutCompleted {
                session: Some("cs_test_1".into()),
                order: Some("ord_1".into()),
                paid: true,
                amount: Some(2_000),
                currency: Some("eur".into()),
            }
        );
    }

    #[test]
    fn an_account_update_carries_its_readiness() {
        let ready = Delivery::parse(&json!({
            "type": "account.updated",
            "data": { "object": {
                "id": "acct_1",
                "details_submitted": true,
                "payouts_enabled": true,
                "capabilities": { "transfers": "active" },
            }},
        }));
        assert_eq!(
            ready.event,
            Event::AccountUpdated {
                account: "acct_1".into(),
                ready: true
            }
        );
        let pending = Delivery::parse(&json!({
            "type": "account.updated",
            "data": { "object": { "id": "acct_1", "details_submitted": true }},
        }));
        assert_eq!(
            pending.event,
            Event::AccountUpdated {
                account: "acct_1".into(),
                ready: false
            }
        );
    }

    #[test]
    fn what_cannot_be_read_is_ignored_not_refused() {
        for body in [
            json!({}),
            json!({ "type": "charge.refunded", "data": { "object": {} } }),
            json!({ "type": "account.updated", "data": { "object": {} } }),
        ] {
            assert!(matches!(Delivery::parse(&body).event, Event::Other(_)));
        }
    }

    #[test]
    fn the_log_remembers_a_bounded_number_of_events() {
        let mut log = SyncLog::default();
        for i in 0..=MAX_APPLIED {
            log.record(&format!("evt_{i}"));
        }
        assert!(!log.applied("evt_0"), "the oldest is dropped");
        assert!(log.applied("evt_1"));
        assert!(log.applied(&format!("evt_{MAX_APPLIED}")));
        log.record("evt_1");
        assert_eq!(log.applied.len(), MAX_APPLIED, "no duplicate entry");
    }

    #[test]
    fn an_older_account_state_is_stale() {
        let mut log = SyncLog::default();
        assert!(!log.stale("acct_1", 5), "nothing held yet");
        assert!(log.saw_account("acct_1", 10));
        assert!(log.stale("acct_1", 9));
        assert!(!log.stale("acct_1", 10), "same second");
        assert!(!log.stale("acct_2", 1), "per account");
        assert!(!log.saw_account("acct_1", 7), "the clock never goes back");
        assert!(log.stale("acct_1", 9));
    }
}
