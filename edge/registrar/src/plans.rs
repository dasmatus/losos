//! What the market sells, as typed plans.
//!
//! Each [`Kind`] has one [`Plan`]: the unit it is priced in, the name the
//! buyer sees on Stripe's page, how the platform's cut is taken, how long a
//! paid order lasts, and what it grants. Pricing, the gate's checks, the
//! Checkout line item and the entitlement all read the plan, so a rule
//! changed here changes everywhere at once and cannot drift between the
//! registrar and the gate.
//!
//! Sellers still set their own price per unit on a listing; a plan is the
//! shape of the product, not its price.

use crate::connect::Split;
use crate::market::{fee_for, Kind, MarketError, MAX_FEE_BPS};

/// How the platform's cut of a sale is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeRule {
    /// The operator's rate (`--market-fee-bps`), never above [`MAX_FEE_BPS`].
    Operator,
    /// A fixed rate, whatever the operator set.
    Fixed(u32),
}

/// What a paid order of a plan grants its buyer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    /// GiB of the mesh's Longhorn pool, as a volume in the buyer's namespace.
    StorageGib,
    /// vCPU-hours on the seller's node inside its compute window. A ledger
    /// credit only.
    ComputeVcpuHours,
    /// Replicas of one virtual machine on the seller's node.
    MachineReplicas,
}

/// One product of the market.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub kind: Kind,
    /// The unit a listing is priced and sized in.
    pub unit: &'static str,
    /// The product's name on the Checkout page.
    pub name: &'static str,
    pub fee: FeeRule,
    /// How long a paid order lasts, from the moment it is paid.
    pub term_secs: u64,
    pub grants: Grant,
}

const MONTH_SECS: u64 = 30 * 24 * 3600;

/// Every plan, one per [`Kind`].
pub const PLANS: [Plan; 3] = [
    Plan {
        kind: Kind::Storage,
        unit: "GiB-month",
        name: "Storage",
        fee: FeeRule::Operator,
        term_secs: MONTH_SECS,
        grants: Grant::StorageGib,
    },
    Plan {
        kind: Kind::Compute,
        unit: "vCPU-hour",
        name: "Compute",
        fee: FeeRule::Operator,
        // Priced per vCPU-hour but sold as a one-month rental: the units are
        // the size of the grant, the month its lifetime.
        term_secs: MONTH_SECS,
        grants: Grant::ComputeVcpuHours,
    },
    Plan {
        kind: Kind::Vm,
        unit: "replica-month",
        name: "Virtual machine",
        // Split half and half with the hosting box's owner.
        fee: FeeRule::Fixed(crate::vms::VM_FEE_BPS),
        term_secs: MONTH_SECS,
        grants: Grant::MachineReplicas,
    },
];

/// The plan of `kind`.
#[must_use]
pub const fn plan(kind: Kind) -> &'static Plan {
    match kind {
        Kind::Storage => &PLANS[0],
        Kind::Compute => &PLANS[1],
        Kind::Vm => &PLANS[2],
    }
}

impl Plan {
    /// The platform's cut in basis points when the operator's rate is
    /// `operator_bps`.
    #[must_use]
    pub const fn fee_bps(&self, operator_bps: u32) -> u32 {
        match self.fee {
            FeeRule::Operator => {
                if operator_bps > MAX_FEE_BPS {
                    MAX_FEE_BPS
                } else {
                    operator_bps
                }
            }
            FeeRule::Fixed(bps) => bps,
        }
    }

    /// How a sale of `gross` splits under this plan.
    ///
    /// # Errors
    /// As [`Split::at_bps`].
    pub fn split(&self, gross: u64, operator_bps: u32) -> Result<Split, MarketError> {
        Split::at_bps(gross, self.fee_bps(operator_bps))
    }

    /// Whether `fee` is the cut this plan takes of `gross`. With no operator
    /// rate known (`None`), an operator-rate plan accepts anything up to the
    /// ceiling; a fixed-rate plan never needs one.
    #[must_use]
    pub fn fee_ok(&self, gross: u64, fee: u64, operator_bps: Option<u32>) -> bool {
        match (self.fee, operator_bps) {
            (FeeRule::Fixed(bps), _) => fee == fee_for(gross, bps),
            (FeeRule::Operator, Some(bps)) => fee == fee_for(gross, self.fee_bps(bps)),
            (FeeRule::Operator, None) => fee <= fee_for(gross, MAX_FEE_BPS),
        }
    }

    /// When an order paid at `paid_at` lapses.
    #[must_use]
    pub const fn expiry(&self, paid_at: u64) -> u64 {
        paid_at + self.term_secs
    }

    /// The Checkout line item's name.
    #[must_use]
    pub fn line_item(&self) -> String {
        format!("{} ({}) - losos market", self.name, self.unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_its_own_plan() {
        for kind in [Kind::Storage, Kind::Compute, Kind::Vm] {
            assert_eq!(plan(kind).kind, kind);
        }
        assert_eq!(PLANS.len(), 3);
    }

    #[test]
    fn the_operator_rate_applies_except_to_machines() {
        assert_eq!(plan(Kind::Storage).fee_bps(400), 400);
        assert_eq!(plan(Kind::Compute).fee_bps(250), 250);
        assert_eq!(plan(Kind::Vm).fee_bps(400), 5_000);
        assert_eq!(plan(Kind::Vm).fee_bps(0), 5_000);
    }

    #[test]
    fn the_operator_rate_never_passes_the_ceiling() {
        assert_eq!(plan(Kind::Storage).fee_bps(9_000), MAX_FEE_BPS);
        let s = plan(Kind::Storage).split(10_000, 9_000).unwrap();
        assert_eq!(s.fee(), fee_for(10_000, MAX_FEE_BPS));
    }

    #[test]
    fn a_fee_is_checked_against_the_plan() {
        let storage = plan(Kind::Storage);
        assert!(storage.fee_ok(2_000, 80, Some(400)));
        assert!(!storage.fee_ok(2_000, 81, Some(400)));
        assert!(storage.fee_ok(2_000, 400, None), "at the ceiling");
        assert!(!storage.fee_ok(2_000, 401, None), "over the ceiling");
        let vm = plan(Kind::Vm);
        assert!(vm.fee_ok(1_001, fee_for(1_001, 5_000), None));
        assert!(
            !vm.fee_ok(1_001, 80, Some(400)),
            "a machine ignores the operator"
        );
    }

    #[test]
    fn a_paid_order_lasts_a_month() {
        for p in &PLANS {
            assert_eq!(p.expiry(1_000), 1_000 + 30 * 24 * 3600);
        }
    }

    #[test]
    fn the_line_item_names_the_product_and_its_unit() {
        assert_eq!(
            plan(Kind::Storage).line_item(),
            "Storage (GiB-month) - losos market"
        );
        assert_eq!(
            plan(Kind::Vm).line_item(),
            "Virtual machine (replica-month) - losos market"
        );
    }
}
