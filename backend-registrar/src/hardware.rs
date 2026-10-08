//! Hardware the edge's operator sells: boxes and gateways, ordered from the
//! LosOS Lab of a box that has `losos.lab.ordering.enable`.
//!
//! Unlike the market this is not a trade between tenants. The platform sells
//! its own goods, so a Checkout Session is a plain charge with no destination
//! and no fee, and Stripe collects the shipping address. Nothing is stored
//! here: the session's metadata names the box and the order, and the
//! operator ships from the Stripe dashboard.
//!
//! The catalogue is one JSON file both processes read: the registrar to show
//! it and to check a cart, the Stripe gate to price every line itself, so a
//! compromised registrar cannot charge a price the operator never set.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::market::SUPPORTED_CURRENCIES;

/// Most of one item in one order.
pub const MAX_QUANTITY: u64 = 20;
/// Most lines in one order; the catalogue is small.
pub const MAX_LINES: usize = 8;
/// Most items a catalogue may hold.
const MAX_ITEMS: usize = 16;
/// Highest unit price, in minor units: 10 000.00.
const MAX_UNIT_AMOUNT: u64 = 1_000_000;
/// Stripe's smallest charge in the supported currencies is 0.50.
const MIN_UNIT_AMOUNT: u64 = 50;
const MAX_NAME: usize = 80;
const MAX_DETAIL: usize = 200;
/// Stripe's shipping countries are ISO 3166-1 alpha-2.
const MAX_COUNTRIES: usize = 64;

/// One thing for sale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// `box` and `gateway` are what the Lab counts in a setup.
    pub sku: String,
    pub name: String,
    #[serde(default)]
    pub detail: String,
    /// Minor units of the catalogue's currency.
    pub unit_amount: u64,
}

/// What the edge sells, in which currency, shipped where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub currency: String,
    pub countries: Vec<String>,
    pub items: Vec<Item>,
}

/// One line of a cart, as a box sends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub sku: String,
    pub quantity: u64,
}

/// A cart line with the catalogue's own name and price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Priced<'a> {
    pub item: &'a Item,
    pub quantity: u64,
}

fn sku_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn text_ok(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.chars().any(char::is_control)
}

impl Catalogue {
    /// Read and check a catalogue file's contents.
    ///
    /// # Errors
    /// A sentence naming the first thing wrong with it.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let c: Self = serde_json::from_slice(bytes).map_err(|e| format!("not a catalogue: {e}"))?;
        c.check()?;
        Ok(c)
    }

    fn check(&self) -> Result<(), String> {
        if !SUPPORTED_CURRENCIES.contains(&self.currency.as_str()) {
            return Err(format!("unsupported currency {:?}", self.currency));
        }
        if self.countries.is_empty() || self.countries.len() > MAX_COUNTRIES {
            return Err("countries must name 1 to 64 shipping countries".to_string());
        }
        if let Some(bad) = self
            .countries
            .iter()
            .find(|c| c.len() != 2 || !c.bytes().all(|b| b.is_ascii_uppercase()))
        {
            return Err(format!("{bad:?} is not an ISO 3166-1 alpha-2 country code"));
        }
        if self.items.is_empty() || self.items.len() > MAX_ITEMS {
            return Err("items must hold 1 to 16 entries".to_string());
        }
        let mut seen = BTreeSet::new();
        for item in &self.items {
            if !sku_ok(&item.sku) {
                return Err(format!("bad sku {:?}", item.sku));
            }
            if !seen.insert(item.sku.as_str()) {
                return Err(format!("sku {:?} appears twice", item.sku));
            }
            if item.name.trim().is_empty() || !text_ok(&item.name, MAX_NAME) {
                return Err(format!("bad name for {:?}", item.sku));
            }
            if !text_ok(&item.detail, MAX_DETAIL) {
                return Err(format!("bad detail for {:?}", item.sku));
            }
            if !(MIN_UNIT_AMOUNT..=MAX_UNIT_AMOUNT).contains(&item.unit_amount) {
                return Err(format!("unit_amount of {:?} is out of range", item.sku));
            }
        }
        Ok(())
    }

    /// Read the catalogue at `path`.
    ///
    /// # Errors
    /// If it cannot be read or is not a valid catalogue.
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&bytes).map_err(|e| format!("{path}: {e}"))
    }

    /// Price a cart against this catalogue: every sku known, each at most
    /// once, every quantity in range.
    ///
    /// # Errors
    /// A sentence safe to show the buyer.
    pub fn price<'a>(&'a self, lines: &[Line]) -> Result<Vec<Priced<'a>>, &'static str> {
        if lines.is_empty() {
            return Err("the order is empty");
        }
        if lines.len() > MAX_LINES {
            return Err("too many lines in one order");
        }
        let mut seen = BTreeSet::new();
        let mut out = Vec::with_capacity(lines.len());
        for line in lines {
            if !seen.insert(line.sku.as_str()) {
                return Err("an item appears twice in the order");
            }
            let item = self
                .items
                .iter()
                .find(|i| i.sku == line.sku)
                .ok_or("an item in the order is not sold here")?;
            if line.quantity == 0 || line.quantity > MAX_QUANTITY {
                return Err("a quantity is out of range (1 to 20)");
            }
            out.push(Priced {
                item,
                quantity: line.quantity,
            });
        }
        Ok(out)
    }
}

/// The total of a priced cart, in minor units. Cannot overflow: at most
/// [`MAX_LINES`] × [`MAX_QUANTITY`] × [`MAX_UNIT_AMOUNT`].
#[must_use]
pub fn total(lines: &[Priced<'_>]) -> u64 {
    lines.iter().map(|l| l.item.unit_amount * l.quantity).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Catalogue {
        Catalogue::parse(
            br#"{"currency":"eur","countries":["SK","DE"],"items":[
                {"sku":"box","name":"LosOS box","detail":"x86_64 mini PC, 16 GB, 1 TB","unit_amount":44900},
                {"sku":"gateway","name":"LosOS edge gateway","unit_amount":44900}]}"#,
        )
        .unwrap()
    }

    fn line(sku: &str, quantity: u64) -> Line {
        Line {
            sku: sku.to_string(),
            quantity,
        }
    }

    #[test]
    fn a_cart_is_priced_from_the_catalogue_not_from_the_request() {
        let c = catalogue();
        let priced = c.price(&[line("box", 3), line("gateway", 1)]).unwrap();
        assert_eq!(priced[0].item.name, "LosOS box");
        assert_eq!(total(&priced), 4 * 44_900);
    }

    #[test]
    fn carts_outside_the_rules_are_refused() {
        let c = catalogue();
        assert!(c.price(&[]).is_err());
        assert!(c.price(&[line("box", 0)]).is_err());
        assert!(c.price(&[line("box", MAX_QUANTITY + 1)]).is_err());
        assert!(c.price(&[line("router", 1)]).is_err());
        assert!(c.price(&[line("box", 1), line("box", 1)]).is_err());
        let many: Vec<Line> = (0..=MAX_LINES).map(|_| line("box", 1)).collect();
        assert!(c.price(&many).is_err());
        assert!(c.price(&[line("box", MAX_QUANTITY)]).is_ok());
    }

    #[test]
    fn a_catalogue_with_anything_odd_does_not_load() {
        let bad = [
            r#"{"currency":"czk","countries":["SK"],"items":[{"sku":"box","name":"b","unit_amount":100}]}"#,
            r#"{"currency":"eur","countries":[],"items":[{"sku":"box","name":"b","unit_amount":100}]}"#,
            r#"{"currency":"eur","countries":["sk"],"items":[{"sku":"box","name":"b","unit_amount":100}]}"#,
            r#"{"currency":"eur","countries":["SK"],"items":[]}"#,
            r#"{"currency":"eur","countries":["SK"],"items":[{"sku":"Box","name":"b","unit_amount":100}]}"#,
            r#"{"currency":"eur","countries":["SK"],"items":[{"sku":"box","name":" ","unit_amount":100}]}"#,
            r#"{"currency":"eur","countries":["SK"],"items":[{"sku":"box","name":"b","unit_amount":10}]}"#,
            r#"{"currency":"eur","countries":["SK"],"items":[{"sku":"box","name":"b\n","unit_amount":100}]}"#,
            r#"{"currency":"eur","countries":["SK"],"items":[{"sku":"box","name":"b","unit_amount":100},{"sku":"box","name":"c","unit_amount":100}]}"#,
            r#"{"currency":"eur","countries":["SK"],"items":[{"sku":"box","name":"b","unit_amount":100,"destination":"acct_1"}]}"#,
        ];
        for raw in bad {
            assert!(Catalogue::parse(raw.as_bytes()).is_err(), "{raw}");
        }
        assert_eq!(catalogue().items.len(), 2);
    }
}
