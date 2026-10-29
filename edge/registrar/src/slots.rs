//! Slots: a paid order is a month of what the seller's box lends, and the
//! money for it waits until the month is over.
//!
//! While a slot runs:
//!
//!   * the seller cannot stop selling the listing it was bought from
//!     ([`locked_until`]), so an offer someone has paid for stays an offer;
//!   * the payment is held, in `LososEscrow` for USDC on an EVM network or on
//!     the platform's Stripe balance for a card ([`crate::market::Card`]), and
//!     the seller is paid out only once the slot is over ([`releasable`]);
//!   * the edge watches the seller's box ([`observe`]). When it has been
//!     offline for the operator's limit in a row (a day by default), or for
//!     twice that in all during the slot, the buyer gets back the share of the
//!     slot that was not delivered and the seller keeps the rest
//!     ([`refund_offline`]).
//!
//! Neither side can run with the money. The seller is paid for what was
//! delivered and nothing before it was; the buyer cannot trigger a refund at
//! all, and cannot take a held deposit back before the edge has paid the
//! seller ([`release_due`] pays out well inside the escrow's deadline).
//!
//! Bitcoin, Monero, Zcash and Solana USDC go straight to the seller's own
//! wallet, which the edge holds no key to, so those orders lock the listing
//! but have nothing that can be held or refunded.
//!
//! Only offline time the edge saw counts. A pass that comes long after the
//! last one (the edge was down, or too busy to hear heartbeats) adds nothing,
//! so an edge outage never turns into refunds.
//!
//! Everything here is a pure function over [`MarketState`];
//! `crate::market::Market` persists it and `crate::server` says who is online.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::market::{
    delivered, usdc_deadline, EscrowStep, MarketState, Order, OrderStatus, RefundReason,
};

/// How long a seller's box may be offline in a row before its buyers are
/// refunded, unless the operator says otherwise
/// (`--market-offline-refund-after`). Twice this in all during one slot does
/// the same, so a box that comes back for a minute a day is not a way around
/// it.
pub const DEFAULT_OFFLINE_SECS: u64 = 24 * 60 * 60;

/// How long after a slot ends the edge pays the seller out by itself, when
/// the seller has not asked sooner.
pub const AUTO_PAYOUT_SECS: u64 = 24 * 60 * 60;

/// How long before an escrow deadline the edge pays out at the latest. Past
/// the deadline the buyer could take the deposit back alone, for a slot that
/// was delivered.
pub const DEADLINE_MARGIN_SECS: u64 = 2 * 24 * 60 * 60;

/// How long after payment a slot may still be waiting to be set up (a claim
/// that never bound, machines never created) before the buyer is refunded.
pub const UNDELIVERED_SECS: u64 = 23 * 60 * 60;

/// What the edge knows about one seller's box while it has a running slot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    /// The last pass that found the box online.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen_at: Option<u64>,
    /// Seconds offline in a row, as far as the edge watched.
    #[serde(default)]
    pub streak: u64,
    /// Seconds offline in all, the same way. Each order keeps the value at
    /// its payment ([`Order::offline_base`]) and counts only what came after.
    #[serde(default)]
    pub offline: u64,
}

/// Whether an order's money is held: paid, and its escrow deposit or card
/// payment still waiting for the slot to end.
#[must_use]
pub fn held(o: &Order) -> bool {
    o.status == OrderStatus::Paid && step_of(o) == Some(EscrowStep::Held)
}

/// The step of whatever holds the order's money, if anything does.
#[must_use]
pub fn step_of(o: &Order) -> Option<EscrowStep> {
    o.escrow
        .as_ref()
        .map(|e| e.step)
        .or(o.card.as_ref().map(|c| c.step))
}

/// Move the order's held money on to `step`.
fn advance(o: &mut Order, step: EscrowStep, reason: Option<RefundReason>, refund: Option<u64>) {
    if let Some(e) = o.escrow.as_mut() {
        e.step = step;
        e.reason = reason;
        e.refund = refund;
    } else if let Some(c) = o.card.as_mut() {
        c.step = step;
        c.reason = reason;
        c.refund = refund;
    }
}

/// Whether the slot is still running at `now`.
#[must_use]
pub fn running(o: &Order, now: u64) -> bool {
    o.status == OrderStatus::Paid && o.expires_at.is_some_and(|end| now < end)
}

/// Until when the seller may not stop selling `listing_id`: the end of the
/// last slot running on it. `None` when nothing paid for is running.
#[must_use]
pub fn locked_until(state: &MarketState, listing_id: &str, now: u64) -> Option<u64> {
    state
        .orders
        .values()
        .filter(|o| o.listing_id == listing_id && running(o, now))
        .filter_map(|o| o.expires_at)
        .max()
}

/// The sellers whose boxes the edge watches: those with held money for a
/// slot that is still running.
#[must_use]
pub fn watched(state: &MarketState, now: u64) -> BTreeSet<String> {
    state
        .orders
        .values()
        .filter(|o| held(o) && running(o, now))
        .map(|o| o.seller.clone())
        .collect()
}

/// What changed in one [`observe`] pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Observed {
    /// A box went offline, or came back. Worth writing down at once: a lost
    /// reset would count an online box as offline.
    pub turned: bool,
}

/// Count the time since the last pass against every watched seller whose
/// box is not in `online`. `can_tell` is false when the edge could not have
/// heard the boxes (it shed heartbeats); such a pass, and one that comes more
/// than `max_gap` seconds after the last, counts nothing.
pub fn observe(
    state: &mut MarketState,
    online: &BTreeSet<String>,
    now: u64,
    can_tell: bool,
    max_gap: u64,
) -> Observed {
    let interval = match state.presence_tick {
        Some(tick) if can_tell && now >= tick && now - tick <= max_gap => now - tick,
        _ => 0,
    };
    state.presence_tick = Some(now);
    let watch = watched(state, now);
    state.presence.retain(|seller, _| watch.contains(seller));
    let mut out = Observed::default();
    for seller in watch {
        let p = state.presence.entry(seller.clone()).or_default();
        if online.contains(&seller) {
            out.turned |= p.streak > 0;
            p.streak = 0;
            p.seen_at = Some(now);
        } else if interval > 0 {
            out.turned |= p.streak == 0;
            p.streak = p.streak.saturating_add(interval);
            p.offline = p.offline.saturating_add(interval);
        }
    }
    out
}

/// The seller's offline seconds counted since `o` was paid.
#[must_use]
pub fn offline_in_slot(state: &MarketState, o: &Order) -> u64 {
    state
        .presence
        .get(&o.seller)
        .map_or(0, |p| p.offline.saturating_sub(o.offline_base))
}

/// What `o` gives back when the box stops delivering at `now`: the share of
/// the slot still to come plus the share it was already offline, rounded up
/// to the minor unit, never more than was paid.
#[must_use]
pub fn refund_share(o: &Order, offline: u64, now: u64) -> u64 {
    let (Some(start), Some(end)) = (o.paid_at, o.expires_at) else {
        return o.amount;
    };
    let slot = end.saturating_sub(start);
    if slot == 0 {
        return o.amount;
    }
    let unserved = end.saturating_sub(now).saturating_add(offline).min(slot);
    let share = (u128::from(o.amount) * u128::from(unserved)).div_ceil(u128::from(slot));
    u64::try_from(share).unwrap_or(o.amount).min(o.amount)
}

/// Held, running orders whose seller has been offline for `limit` seconds in
/// a row, or twice that in all since the order was paid: end the slot and
/// give the buyer back the share not delivered. Returns `(order id, refund)`.
pub fn refund_offline(state: &mut MarketState, now: u64, limit: u64) -> Vec<(String, u64)> {
    let due: Vec<(String, u64)> = state
        .orders
        .values()
        .filter(|o| held(o) && running(o, now))
        .filter_map(|o| {
            let streak = state.presence.get(&o.seller).map_or(0, |p| p.streak);
            let offline = offline_in_slot(state, o);
            (streak >= limit || offline >= limit.saturating_mul(2))
                .then(|| (o.id.clone(), refund_share(o, offline, now)))
        })
        .collect();
    for (id, refund) in &due {
        let Some(o) = state.orders.get_mut(id) else {
            continue;
        };
        if *refund >= o.amount {
            advance(
                o,
                EscrowStep::Refund,
                Some(RefundReason::SellerOffline),
                None,
            );
        } else {
            advance(
                o,
                EscrowStep::Split,
                Some(RefundReason::SellerOffline),
                Some(*refund),
            );
        }
        o.expires_at = Some(now);
    }
    due
}

/// Held orders whose slot never got set up within [`UNDELIVERED_SECS`] of
/// payment: give the whole payment back and end the order.
pub fn refund_undelivered(state: &mut MarketState, now: u64) -> Vec<String> {
    let mut refunded = Vec::new();
    for o in state.orders.values_mut() {
        let late = o
            .paid_at
            .is_some_and(|paid| now >= paid.saturating_add(UNDELIVERED_SECS));
        if !held(o) || delivered(o) || !late {
            continue;
        }
        advance(
            o,
            EscrowStep::Refund,
            Some(RefundReason::NotDelivered),
            None,
        );
        o.expires_at = Some(o.expires_at.map_or(now, |at| at.min(now)));
        refunded.push(o.id.clone());
    }
    refunded
}

/// Whether the seller may be paid for `o` now: its money is held, the slot
/// is over and what it bought was set up.
#[must_use]
pub fn releasable(o: &Order, now: u64) -> bool {
    held(o) && o.expires_at.is_some_and(|end| now >= end) && delivered(o)
}

/// Held orders the edge pays out by itself: a day after the slot ended, or,
/// for an escrow deposit, [`DEADLINE_MARGIN_SECS`] before the buyer could
/// take it back alone, whichever comes first.
pub fn release_due(state: &mut MarketState, now: u64) -> Vec<String> {
    let mut released = Vec::new();
    for o in state.orders.values_mut() {
        let after_slot = releasable(o, now)
            && o.expires_at
                .is_some_and(|end| now >= end.saturating_add(AUTO_PAYOUT_SECS));
        let before_deadline = held(o)
            && delivered(o)
            && o.escrow.is_some()
            && now.saturating_add(DEADLINE_MARGIN_SECS) >= usdc_deadline(o);
        if after_slot || before_deadline {
            advance(o, EscrowStep::Release, None, None);
            released.push(o.id.clone());
        }
    }
    released
}

/// The seller asked to be paid: every sale of theirs that is
/// [`releasable`]. Returns the ids and what they pay the seller.
pub fn release_for(state: &mut MarketState, seller: &str, now: u64) -> (Vec<String>, u64) {
    let mut ids = Vec::new();
    let mut amount = 0u64;
    for o in state
        .orders
        .values_mut()
        .filter(|o| o.seller == seller && releasable(o, now))
    {
        advance(o, EscrowStep::Release, None, None);
        amount = amount.saturating_add(seller_net(o));
        ids.push(o.id.clone());
    }
    (ids, amount)
}

/// While `o`'s money is held for a slot that will be paid out: when the edge
/// pays the seller by itself.
#[must_use]
pub fn payout_at(o: &Order) -> Option<u64> {
    if !held(o) {
        return None;
    }
    let after_slot = o.expires_at?.saturating_add(AUTO_PAYOUT_SECS);
    if o.escrow.is_some() {
        Some(after_slot.min(usdc_deadline(o).saturating_sub(DEADLINE_MARGIN_SECS)))
    } else {
        Some(after_slot)
    }
}

/// What a sale pays its seller in full.
#[must_use]
pub fn seller_net(o: &Order) -> u64 {
    o.amount.saturating_sub(o.fee).saturating_sub(o.fee_offset)
}

/// What the seller of a split order receives: the share kept, less the fee
/// scaled the same way, rounded down as `LososEscrow.settle` and the gate's
/// transfer do it.
#[must_use]
pub fn split_net(o: &Order, refund: u64) -> u64 {
    let kept = o.amount.saturating_sub(refund);
    if o.amount == 0 {
        return 0;
    }
    let fee = o.fee.saturating_add(o.fee_offset);
    let fee_kept = u128::from(fee) * u128::from(kept) / u128::from(o.amount);
    kept.saturating_sub(u64::try_from(fee_kept).unwrap_or(kept))
}

/// A seller's held money, as the Market pane shows it next to Pay out.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct PayoutView {
    /// Held for slots still running, minor units.
    pub held: u64,
    /// Slots that are over and can be paid out now.
    pub ready: u64,
    /// On its way to the seller.
    pub paying: u64,
    /// When the next running slot ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_at: Option<u64>,
    /// Whether the edge pays out by itself a day after a slot ends.
    pub auto_after_secs: u64,
}

#[must_use]
pub fn payout_view(state: &MarketState, seller: &str, now: u64) -> PayoutView {
    let mut view = PayoutView {
        auto_after_secs: AUTO_PAYOUT_SECS,
        ..PayoutView::default()
    };
    for o in state.orders.values().filter(|o| o.seller == seller) {
        if releasable(o, now) {
            view.ready = view.ready.saturating_add(seller_net(o));
        } else if held(o) {
            view.held = view.held.saturating_add(seller_net(o));
            if let Some(end) = o.expires_at {
                view.next_at = Some(view.next_at.map_or(end, |n| n.min(end)));
            }
        } else if o.status == OrderStatus::Paid && step_of(o) == Some(EscrowStep::Release) {
            view.paying = view.paying.saturating_add(seller_net(o));
        }
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::{Card, Escrow, Kind, Rail, ENTITLEMENT_SECS};

    const PAID: u64 = 1_000_000;
    const DAY: u64 = 24 * 60 * 60;

    fn order(id: &str, seller: &str, amount: u64) -> Order {
        Order {
            id: id.into(),
            listing_id: "lst_1".into(),
            buyer: "buyer".into(),
            seller: seller.into(),
            kind: Kind::Compute,
            quantity: 1,
            unit_price: amount,
            amount,
            fee: amount / 25,
            currency: "usd".into(),
            status: OrderStatus::Paid,
            session_id: None,
            created_at: PAID,
            paid_at: Some(PAID),
            expires_at: Some(PAID + ENTITLEMENT_SECS),
            volume: None,
            claim: None,
            vm: None,
            rail: Rail::Usdc,
            tx_hash: None,
            pay_to: None,
            pay_to_solana: None,
            paid_network: None,
            coin: None,
            escrow: Some(Escrow {
                network: "base".into(),
                deposit: 1,
                account: None,
                deadline: None,
                step: EscrowStep::Held,
                reason: None,
                tx: None,
                sent_at: None,
                refund: None,
                keys: None,
                approval: None,
            }),
            fee_offset: 0,
            offset_collected: false,
            card: None,
            offline_base: 0,
            boxes: None,
        }
    }

    fn card(id: &str, seller: &str, amount: u64) -> Order {
        Order {
            rail: Rail::Stripe,
            escrow: None,
            card: Some(Card {
                payment_intent: format!("pi_{id}"),
                step: EscrowStep::Held,
                reason: None,
                refund: None,
                tried_at: None,
            }),
            ..order(id, seller, amount)
        }
    }

    fn state_of(orders: Vec<Order>) -> MarketState {
        let mut state = MarketState::default();
        for o in orders {
            state.orders.insert(o.id.clone(), o);
        }
        state
    }

    fn online(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| (*s).to_string()).collect()
    }

    /// Run passes every `step` seconds from `from` to `to` with `seller`
    /// offline, and return the state.
    fn offline_for(state: &mut MarketState, from: u64, to: u64, step: u64) {
        let mut t = from;
        while t <= to {
            observe(state, &online(&[]), t, true, 600);
            t += step;
        }
    }

    #[test]
    fn a_paid_slot_locks_its_listing_until_it_ends() {
        let state = state_of(vec![order("a", "s", 1_000)]);
        assert_eq!(
            locked_until(&state, "lst_1", PAID + 1),
            Some(PAID + ENTITLEMENT_SECS)
        );
        assert_eq!(locked_until(&state, "lst_2", PAID + 1), None);
        assert_eq!(locked_until(&state, "lst_1", PAID + ENTITLEMENT_SECS), None);
        // An unpaid or expired order locks nothing.
        let mut pending = order("p", "s", 1_000);
        pending.status = OrderStatus::Pending;
        let mut expired = order("e", "s", 1_000);
        expired.status = OrderStatus::Expired;
        let state = state_of(vec![pending, expired]);
        assert_eq!(locked_until(&state, "lst_1", PAID + 1), None);
    }

    #[test]
    fn a_coin_sale_locks_its_listing_though_nothing_is_held() {
        let mut coin = order("c", "s", 1_000);
        coin.escrow = None;
        let state = state_of(vec![coin]);
        assert!(locked_until(&state, "lst_1", PAID + 1).is_some());
        assert!(watched(&state, PAID + 1).is_empty());
    }

    #[test]
    fn only_offline_time_the_edge_watched_counts() {
        let mut state = state_of(vec![order("a", "s", 1_000)]);
        observe(&mut state, &online(&["s"]), PAID, true, 600);
        // Offline for ten minutes in 60 s passes.
        offline_for(&mut state, PAID + 60, PAID + 600, 60);
        assert_eq!(state.presence["s"].streak, 600);
        // The edge was down for a week: none of it counts.
        observe(&mut state, &online(&[]), PAID + 7 * DAY, true, 600);
        assert_eq!(state.presence["s"].streak, 600);
        // Nor does a pass while it was shedding heartbeats.
        observe(&mut state, &online(&[]), PAID + 7 * DAY + 60, false, 600);
        assert_eq!(state.presence["s"].streak, 600);
        // Back online resets the streak and keeps the total.
        let seen = observe(&mut state, &online(&["s"]), PAID + 7 * DAY + 120, true, 600);
        assert!(seen.turned);
        assert_eq!(state.presence["s"].streak, 0);
        assert_eq!(state.presence["s"].offline, 600);
    }

    #[test]
    fn a_day_offline_refunds_the_share_not_delivered() {
        let mut state = state_of(vec![order("a", "s", 3_000)]);
        observe(&mut state, &online(&["s"]), PAID, true, 600);
        // Ten days in, the box goes away for a day.
        let gone = PAID + 10 * DAY;
        observe(&mut state, &online(&["s"]), gone, true, 600);
        offline_for(&mut state, gone + 300, gone + DAY - 300, 300);
        assert!(refund_offline(&mut state, gone + DAY - 300, DAY).is_empty());
        observe(&mut state, &online(&[]), gone + DAY, true, 600);
        let now = gone + DAY;
        let due = refund_offline(&mut state, now, DAY);
        // 30 days paid for, 10 delivered: 20 back, rounded up.
        assert_eq!(due, [("a".to_string(), 2_000)]);
        let o = &state.orders["a"];
        let e = o.escrow.as_ref().unwrap();
        assert_eq!(e.step, EscrowStep::Split);
        assert_eq!(e.refund, Some(2_000));
        assert_eq!(e.reason, Some(RefundReason::SellerOffline));
        assert_eq!(o.expires_at, Some(now));
        // Once.
        assert!(refund_offline(&mut state, now + 60, DAY).is_empty());
        assert!(locked_until(&state, "lst_1", now + 60).is_none());
    }

    #[test]
    fn a_box_offline_from_the_start_refunds_everything() {
        let mut state = state_of(vec![card("a", "s", 3_000)]);
        offline_for(&mut state, PAID, PAID + DAY, 600);
        assert_eq!(
            refund_offline(&mut state, PAID + DAY, DAY),
            [("a".to_string(), 3_000)]
        );
        let c = state.orders["a"].card.as_ref().unwrap();
        assert_eq!(c.step, EscrowStep::Refund);
        assert_eq!(c.refund, None);
    }

    #[test]
    fn a_box_that_blinks_back_once_a_day_is_refunded_too() {
        // Online for one pass a day, offline the rest: never a day in a row.
        let mut state = state_of(vec![order("a", "s", 3_000)]);
        let mut t = PAID;
        let mut refunded = Vec::new();
        while refunded.is_empty() && t < PAID + ENTITLEMENT_SECS {
            let up = (t - PAID).is_multiple_of(DAY);
            observe(
                &mut state,
                &online(if up { &["s"] } else { &[] }),
                t,
                true,
                600,
            );
            refunded = refund_offline(&mut state, t, DAY);
            t += 600;
        }
        assert!(state.presence.get("s").is_none_or(|p| p.streak < DAY));
        // Two days of offline time in all, a little more than two days in.
        let (id, refund) = &refunded[0];
        assert_eq!(id, "a");
        assert!(t - PAID < 3 * DAY, "{}", t - PAID);
        // Everything after it went away, and the time it was away before.
        assert!(*refund > 2_700 && *refund < 3_000, "{refund}");
    }

    #[test]
    fn the_nightly_reboot_never_refunds_anyone() {
        // Ten minutes offline every night for the whole month.
        let mut state = state_of(vec![order("a", "s", 3_000)]);
        let mut t = PAID;
        while t < PAID + ENTITLEMENT_SECS {
            let rebooting = (t - PAID) % DAY < 600;
            observe(
                &mut state,
                &online(if rebooting { &[] } else { &["s"] }),
                t,
                true,
                600,
            );
            assert!(
                refund_offline(&mut state, t, DAY).is_empty(),
                "at {}",
                t - PAID
            );
            t += 60;
        }
        assert!(state.presence["s"].offline < 2 * DAY);
    }

    #[test]
    fn offline_time_before_a_purchase_does_not_count_against_it() {
        let mut first = order("a", "s", 3_000);
        first.paid_at = Some(PAID - 5 * DAY);
        first.expires_at = Some(PAID - 5 * DAY + ENTITLEMENT_SECS);
        let mut state = state_of(vec![first]);
        offline_for(&mut state, PAID - 5 * DAY, PAID - 5 * DAY + DAY - 600, 600);
        observe(&mut state, &online(&["s"]), PAID - 3 * DAY, true, 600);
        offline_for(&mut state, PAID - 2 * DAY, PAID - 2 * DAY + DAY / 2, 600);
        observe(&mut state, &online(&["s"]), PAID, true, 600);
        let offline = state.presence["s"].offline;
        assert!(offline > DAY);
        // A second order paid now starts from what the first already saw.
        let mut second = order("b", "s", 3_000);
        second.offline_base = offline;
        state.orders.insert("b".into(), second);
        assert_eq!(offline_in_slot(&state, &state.orders["b"]), 0);
        let end = PAID + DAY / 2 + 3_600;
        offline_for(&mut state, PAID + 60, end, 600);
        // The first has its two days in all; the second does not.
        let due = refund_offline(&mut state, end, DAY);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, "a");
    }

    #[test]
    fn a_coin_or_solana_sale_is_not_watched_or_refunded() {
        let mut coin = order("c", "s", 3_000);
        coin.escrow = None;
        let mut state = state_of(vec![coin]);
        offline_for(&mut state, PAID, PAID + 3 * DAY, 600);
        assert!(state.presence.is_empty());
        assert!(refund_offline(&mut state, PAID + 3 * DAY, DAY).is_empty());
    }

    #[test]
    fn the_seller_is_paid_only_once_the_slot_is_over() {
        let mut state = state_of(vec![order("a", "s", 1_000), order("b", "s", 2_000)]);
        state.orders.get_mut("b").unwrap().expires_at = Some(PAID + 40 * DAY);
        let end = PAID + ENTITLEMENT_SECS;
        assert_eq!(release_for(&mut state, "s", end - 1), (vec![], 0));
        assert_eq!(
            payout_view(&state, "s", end - 1),
            PayoutView {
                held: 960 + 1_920,
                ready: 0,
                paying: 0,
                next_at: Some(end),
                auto_after_secs: AUTO_PAYOUT_SECS,
            }
        );
        assert_eq!(payout_view(&state, "s", end).ready, 960);
        // Another seller cannot ask for this one's money.
        assert_eq!(release_for(&mut state, "t", end), (vec![], 0));
        assert_eq!(
            release_for(&mut state, "s", end),
            (vec!["a".to_string()], 960)
        );
        assert_eq!(step_of(&state.orders["a"]), Some(EscrowStep::Release));
        assert_eq!(step_of(&state.orders["b"]), Some(EscrowStep::Held));
        assert_eq!(payout_view(&state, "s", end).paying, 960);
        // Asking again releases nothing more.
        assert_eq!(release_for(&mut state, "s", end), (vec![], 0));
    }

    #[test]
    fn nothing_undelivered_is_paid_out() {
        let mut o = order("a", "s", 1_000);
        o.kind = Kind::Storage;
        let mut state = state_of(vec![o]);
        let end = PAID + ENTITLEMENT_SECS;
        assert!(!releasable(&state.orders["a"], end));
        assert!(release_due(&mut state, end + AUTO_PAYOUT_SECS).is_empty());
        state.orders.get_mut("a").unwrap().volume = Some("market-b/ord-a".into());
        assert!(releasable(&state.orders["a"], end));
    }

    #[test]
    fn the_edge_pays_out_by_itself_a_day_after_the_slot() {
        let mut state = state_of(vec![card("a", "s", 1_000)]);
        let end = PAID + ENTITLEMENT_SECS;
        assert!(release_due(&mut state, end + AUTO_PAYOUT_SECS - 1).is_empty());
        assert_eq!(release_due(&mut state, end + AUTO_PAYOUT_SECS), ["a"]);
        assert!(release_due(&mut state, end + AUTO_PAYOUT_SECS).is_empty());
    }

    #[test]
    fn an_escrow_is_paid_out_before_the_buyer_could_take_it_back() {
        // Paid late, so the slot outlasts the deadline's margin.
        let mut o = order("a", "s", 1_000);
        o.created_at = PAID - 3 * DAY;
        let mut state = state_of(vec![o]);
        let deadline = usdc_deadline(&state.orders["a"]);
        assert!(deadline > PAID + ENTITLEMENT_SECS - 3 * DAY);
        assert!(release_due(&mut state, deadline - DEADLINE_MARGIN_SECS - 1).is_empty());
        assert_eq!(
            release_due(&mut state, deadline - DEADLINE_MARGIN_SECS),
            ["a"]
        );
    }

    #[test]
    fn the_deadline_leaves_the_margin_after_a_slot_paid_on_time() {
        let o = order("a", "s", 1_000);
        let end = o.expires_at.unwrap();
        assert!(usdc_deadline(&o) >= end + AUTO_PAYOUT_SECS + DEADLINE_MARGIN_SECS);
        assert!(usdc_deadline(&o) - o.created_at <= 35 * DAY);
    }

    #[test]
    fn a_slot_never_set_up_goes_back_whole() {
        let mut o = order("a", "s", 1_000);
        o.kind = Kind::Storage;
        let mut state = state_of(vec![o, card("b", "s", 500)]);
        assert!(refund_undelivered(&mut state, PAID + UNDELIVERED_SECS - 1).is_empty());
        assert_eq!(
            refund_undelivered(&mut state, PAID + UNDELIVERED_SECS),
            ["a"]
        );
        let e = state.orders["a"].escrow.as_ref().unwrap();
        assert_eq!(e.step, EscrowStep::Refund);
        assert_eq!(e.reason, Some(RefundReason::NotDelivered));
        assert_eq!(step_of(&state.orders["b"]), Some(EscrowStep::Held));
    }

    #[test]
    fn refunds_are_rounded_up_and_bounded() {
        let o = order("a", "s", 1_000);
        let end = o.expires_at.unwrap();
        assert_eq!(refund_share(&o, 0, end), 0);
        assert_eq!(refund_share(&o, 0, end - 1), 1);
        assert_eq!(refund_share(&o, 0, PAID), 1_000);
        assert_eq!(refund_share(&o, 40 * DAY, end - DAY), 1_000);
        assert_eq!(refund_share(&o, DAY, end - DAY), 67);
    }

    #[test]
    fn a_split_pays_the_seller_the_kept_share_less_the_scaled_fee() {
        let o = order("a", "s", 1_000);
        assert_eq!(seller_net(&o), 960);
        assert_eq!(split_net(&o, 250), 750 - 30);
        assert_eq!(split_net(&o, 1_000), 0);
        assert_eq!(split_net(&o, 0), 960);
    }
}
