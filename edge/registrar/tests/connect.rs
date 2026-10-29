//! The market's whole Stripe Connect lifecycle on the in-memory platform:
//! a seller onboards, lists, a buyer pays through a destination charge, the
//! signed webhooks land, and the ledger and the platform agree on the split.
//! No gate process and no Stripe stub: this is what `connect::Connect` is for.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use losos_registrar::connect::{Connect, MemoryConnect, Split};
use losos_registrar::market::{
    Kind, Market, MarketError, MarketOpts, NewListing, NewOrder, OrderStatus, Sharing,
    DEFAULT_FEE_BPS,
};
use losos_registrar::stripe_mode::StripeMode;
use losos_registrar::window::ComputeWindow;

const SELLER: &str = "seller-box";
const BUYER: &str = "buyer-box";
const RETURN: &str = "https://edge.example/market/return";

fn state_file(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "losos-connect-{tag}-{}-{}",
        std::process::id(),
        losos_registrar::market::now_secs()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join("market.json")
}

fn opts(state: &Path) -> MarketOpts {
    MarketOpts {
        state_file: state.to_string_lossy().into_owned(),
        gate_socket: "/nonexistent/gate.sock".to_string(),
        return_url: RETURN.to_string(),
        currency: "eur".to_string(),
        fee_bps: DEFAULT_FEE_BPS,
        storage_class: "longhorn".to_string(),
        storage_image: None,
        hardware_catalogue: None,
        vms: None,
        usdc: None,
        btc: None,
        xmr: None,
        cards: true,
        zec: None,
        coin_fee_limit: losos_registrar::market::DEFAULT_COIN_FEE_LIMIT,
        offline_refund_secs: losos_registrar::slots::DEFAULT_OFFLINE_SECS,
        direct_pay: true,
    }
}

fn sharing() -> Sharing {
    let window = ComputeWindow {
        share_compute: true,
        window_start: "00:00".to_string(),
        window_end: "23:59".to_string(),
        tz: "UTC".to_string(),
        host_vms: false,
    };
    Sharing::from_windows(&BTreeMap::from([(SELLER.to_string(), window)]))
}

async fn market(tag: &str, mode: StripeMode) -> (Market, Arc<MemoryConnect>, PathBuf) {
    let file = state_file(tag);
    let platform = Arc::new(MemoryConnect::new(mode));
    let connect: Arc<dyn Connect> = platform.clone();
    let market = Market::open_with(opts(&file), connect)
        .await
        .expect("open market");
    assert_eq!(market.check_mode().await.unwrap(), mode);
    (market, platform, file)
}

/// Onboard the seller through to ready, the way Stripe reports it.
async fn ready_seller(market: &Market, platform: &MemoryConnect) -> String {
    let first = market.onboard(SELLER, None).await.unwrap();
    assert!(!first.ready);
    assert!(first.url.is_some(), "a new seller is sent to onboarding");
    let account = platform.account_of(SELLER).expect("account made");
    platform.set_ready(&account, true).unwrap();
    let (body, sig) = platform.account_updated(&account).unwrap();
    market.webhook(&sig, &body).await.unwrap();
    assert!(market.seller(SELLER).await.unwrap().ready);
    account
}

#[tokio::test]
async fn a_seller_is_paid_the_net_of_a_destination_charge() {
    let (market, platform, _) = market("paid", StripeMode::Test).await;
    let account = ready_seller(&market, &platform).await;
    let listing = market
        .create_listing(
            SELLER,
            NewListing {
                kind: Kind::Storage,
                unit_price: 250,
                capacity: 100,
            },
            &sharing(),
        )
        .await
        .unwrap();

    let checkout = market
        .create_order(
            BUYER,
            NewOrder {
                listing_id: listing,
                quantity: 8,
                image: None,
                name: None,
                user_data: None,
            },
            &sharing(),
        )
        .await
        .unwrap();
    assert_eq!((checkout.amount, checkout.fee), (2_000, 80));
    assert!(checkout
        .checkout_url
        .as_deref()
        .is_some_and(|u| u.starts_with("https://checkout.stripe.com/")));

    let charges = platform.charges();
    assert_eq!(charges.len(), 1);
    let charge = &charges[0];
    assert_eq!(charge.destination(), account);
    assert_eq!(charge.order_id(), checkout.order_id);
    assert_eq!(
        charge.split(),
        Split::at_bps(2_000, DEFAULT_FEE_BPS).unwrap()
    );
    assert_eq!(charge.split().net(), 1_920);

    let session = platform.session_of(&checkout.order_id).unwrap();
    let (body, sig) = platform.completed(&session).unwrap();
    market.webhook(&sig, &body).await.unwrap();

    let buyer = market.account(BUYER, &sharing()).await.unwrap();
    let order = &buyer.purchases[0];
    assert_eq!(order.status, OrderStatus::Paid);
    assert_eq!(order.seller_net, charge.split().net());
    assert_eq!(buyer.entitlements.storage_gib, 8);
    let seller = market.account(SELLER, &sharing()).await.unwrap();
    assert_eq!(seller.sales[0].seller_net, 1_920);
}

#[tokio::test]
async fn an_onboarded_seller_is_not_sent_back_to_stripe() {
    let (market, platform, _) = market("again", StripeMode::Test).await;
    let account = ready_seller(&market, &platform).await;
    let links = platform.links();
    let again = market.onboard(SELLER, None).await.unwrap();
    assert!(again.ready);
    assert!(again.url.is_none());
    assert_eq!(platform.links(), links, "no second onboarding link");
    assert_eq!(
        platform.account_of(SELLER).as_deref(),
        Some(account.as_str())
    );
}

#[tokio::test]
async fn a_missed_account_webhook_is_caught_up_on_the_next_onboard() {
    let (market, platform, _) = market("missed", StripeMode::Test).await;
    market.onboard(SELLER, None).await.unwrap();
    let account = platform.account_of(SELLER).unwrap();
    // Stripe finished onboarding but its `account.updated` never arrived.
    platform.set_ready(&account, true).unwrap();
    assert!(!market.seller(SELLER).await.unwrap().ready);
    assert!(market.onboard(SELLER, None).await.unwrap().ready);
    assert!(market.seller(SELLER).await.unwrap().ready);
}

#[tokio::test]
async fn the_box_uuid_reaches_the_account() {
    let (market, platform, _) = market("uuid", StripeMode::Test).await;
    let uuid = "0a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d";
    market.onboard(SELLER, None).await.unwrap();
    let account = platform.account_of(SELLER).unwrap();
    assert_eq!(platform.account(&account).unwrap().box_uuid, None);
    market.onboard(SELLER, Some(uuid)).await.unwrap();
    assert_eq!(
        platform.account(&account).unwrap().box_uuid.as_deref(),
        Some(uuid)
    );
}

#[tokio::test]
async fn a_seller_who_lost_onboarding_cannot_be_bought_from() {
    let (market, platform, _) = market("unready", StripeMode::Test).await;
    let account = ready_seller(&market, &platform).await;
    let listing = market
        .create_listing(
            SELLER,
            NewListing {
                kind: Kind::Storage,
                unit_price: 100,
                capacity: 10,
            },
            &sharing(),
        )
        .await
        .unwrap();
    platform.set_ready(&account, false).unwrap();
    let (body, sig) = platform.account_updated(&account).unwrap();
    market.webhook(&sig, &body).await.unwrap();
    assert!(market.browse(&sharing()).await.unwrap().is_empty());
    let refused = market
        .create_order(
            BUYER,
            NewOrder {
                listing_id: listing,
                quantity: 1,
                image: None,
                name: None,
                user_data: None,
            },
            &sharing(),
        )
        .await;
    assert!(matches!(refused, Err(MarketError::NotFound)));
    assert!(platform.charges().is_empty());
}

#[tokio::test]
async fn a_charge_the_platform_refuses_frees_the_units() {
    let (market, platform, _) = market("refused", StripeMode::Test).await;
    let account = ready_seller(&market, &platform).await;
    let listing = market
        .create_listing(
            SELLER,
            NewListing {
                kind: Kind::Storage,
                unit_price: 100,
                capacity: 10,
            },
            &sharing(),
        )
        .await
        .unwrap();
    // Stripe disabled the account's transfers after the ledger last heard.
    platform.set_ready(&account, false).unwrap();
    let order = |quantity| NewOrder {
        listing_id: listing.clone(),
        quantity,
        image: None,
        name: None,
        user_data: None,
    };
    assert!(market
        .create_order(BUYER, order(10), &sharing())
        .await
        .is_err());
    platform.set_ready(&account, true).unwrap();
    // The whole stock is orderable again: the failed order held nothing.
    market
        .create_order(BUYER, order(10), &sharing())
        .await
        .expect("units were released");
}

#[tokio::test]
async fn a_forged_or_cross_mode_webhook_changes_nothing() {
    let (market, platform, _) = market("forged", StripeMode::Test).await;
    market.onboard(SELLER, None).await.unwrap();
    let account = platform.account_of(SELLER).unwrap();
    platform.set_ready(&account, true).unwrap();
    let (body, _) = platform.account_updated(&account).unwrap();
    let forged = market.webhook("t=1,v1=00", &body).await;
    assert!(matches!(forged, Err(MarketError::BadSignature)));

    let live = MemoryConnect::new(StripeMode::Live);
    live.create_account(SELLER, None).await.unwrap();
    let live_account = live.account_of(SELLER).unwrap();
    live.set_ready(&live_account, true).unwrap();
    let (live_body, live_sig) = live.account_updated(&live_account).unwrap();
    assert!(market.webhook(&live_sig, &live_body).await.is_err());
    assert!(!market.seller(SELLER).await.unwrap().ready);
}

#[tokio::test]
async fn test_and_live_keep_separate_ledgers() {
    let (test_market, platform, file) = market("modes", StripeMode::Test).await;
    ready_seller(&test_market, &platform).await;
    assert!(file.with_file_name("market-test.json").exists());
    assert!(!file.exists(), "nothing written to the live ledger");

    let live = Market::open_with(opts(&file), Arc::new(MemoryConnect::new(StripeMode::Live)))
        .await
        .unwrap();
    assert_eq!(live.check_mode().await.unwrap(), StripeMode::Live);
    assert!(
        live.seller(SELLER).await.is_none(),
        "a test seller is not a live one"
    );
}

#[tokio::test]
async fn no_key_means_no_market() {
    let file = state_file("nokey");
    let platform = Arc::new(MemoryConnect::new(StripeMode::Test));
    platform.set_configured(false);
    let market = Market::open_with(opts(&file), platform).await.unwrap();
    assert!(matches!(
        market.check_mode().await,
        Err(MarketError::Unconfigured)
    ));
    assert!(matches!(
        market.onboard(SELLER, None).await,
        Err(MarketError::Unconfigured)
    ));
}

#[tokio::test]
async fn a_late_account_event_does_not_undo_a_newer_one() {
    let (market, platform, _) = market("late", StripeMode::Test).await;
    market.onboard(SELLER, None).await.unwrap();
    let account = platform.account_of(SELLER).unwrap();
    // Stripe made an "onboarding still pending" event a minute ago, then the
    // seller finished. The newer event arrives first.
    let (stale, stale_sig) = platform
        .account_updated_at(&account, losos_registrar::market::now_secs() - 60)
        .unwrap();
    platform.set_ready(&account, true).unwrap();
    let (fresh, fresh_sig) = platform.account_updated(&account).unwrap();
    market.webhook(&fresh_sig, &fresh).await.unwrap();
    assert!(market.seller(SELLER).await.unwrap().ready);
    market.webhook(&stale_sig, &stale).await.unwrap();
    assert!(
        market.seller(SELLER).await.unwrap().ready,
        "the older pending state did not take the seller off the shelf"
    );
}

#[tokio::test]
async fn an_account_read_on_onboarding_outranks_older_events() {
    let (market, platform, _) = market("read", StripeMode::Test).await;
    market.onboard(SELLER, None).await.unwrap();
    let account = platform.account_of(SELLER).unwrap();
    let (stale, stale_sig) = platform
        .account_updated_at(&account, losos_registrar::market::now_secs() - 60)
        .unwrap();
    // The seller finished onboarding; the next onboard reads it from Stripe.
    platform.set_ready(&account, true).unwrap();
    assert!(market.onboard(SELLER, None).await.unwrap().ready);
    market.webhook(&stale_sig, &stale).await.unwrap();
    assert!(market.seller(SELLER).await.unwrap().ready);
}

#[tokio::test]
async fn a_newer_account_event_still_takes_a_seller_off_the_shelf() {
    let (market, platform, _) = market("newer", StripeMode::Test).await;
    let account = ready_seller(&market, &platform).await;
    platform.set_ready(&account, false).unwrap();
    let (body, sig) = platform
        .account_updated_at(&account, losos_registrar::market::now_secs() + 1)
        .unwrap();
    market.webhook(&sig, &body).await.unwrap();
    assert!(!market.seller(SELLER).await.unwrap().ready);
}
