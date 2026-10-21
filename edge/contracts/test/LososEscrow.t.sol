// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

import {Test, stdError} from "forge-std/Test.sol";
import {IERC20, LososEscrow} from "../src/LososEscrow.sol";
import {MockUSDC} from "../src/MockUSDC.sol";

/// Transaction tests for `LososEscrow`. The buyer, seller, treasury and edge
/// are separate addresses, and `vm.prank` makes each call come from the one
/// a wallet would send it from.
contract LososEscrowTest is Test {
    address internal constant BUYER = address(0xB0B);
    address internal constant SELLER = address(0x5E11);
    address internal constant TREASURY = address(0x7EA5);
    address internal constant EDGE = address(0xED6E);
    address internal constant STRANGER = address(0xBAD);
    address internal constant BUYER2 = address(0xB0B2);
    address internal constant SELLER2 = address(0x5E12);

    MockUSDC internal token;
    LososEscrow internal escrow;

    function setUp() public {
        // A chain's clock, not forge's 1: a tap opens `HEADSTART` back.
        vm.warp(1_800_000_000);
        token = new MockUSDC();
        escrow = new LososEscrow(IERC20(address(token)), TREASURY, EDGE);
        token.mint(BUYER, 10_000_000);
        vm.prank(BUYER);
        token.approve(address(escrow), type(uint256).max);
    }

    function _pay(uint256 gross, uint256 fee) internal returns (uint256) {
        vm.prank(BUYER);
        return escrow.pay(keccak256("ord_test"), SELLER, gross, fee, uint64(block.timestamp + 1 days));
    }

    function test_pay_holds_the_money_and_moves_nothing_else() public {
        uint256 id = _pay(2_000_000, 80_000);
        assertEq(id, 1);
        assertEq(token.balanceOf(address(escrow)), 2_000_000);
        assertEq(escrow.held(), 2_000_000);
        assertEq(token.balanceOf(SELLER), 0);
        assertEq(token.balanceOf(TREASURY), 0);
        (bytes32 orderId, address payer, address seller, uint256 gross, uint256 fee,, LososEscrow.State state) =
            escrow.deposits(id);
        assertEq(orderId, keccak256("ord_test"));
        assertEq(payer, BUYER);
        assertEq(seller, SELLER);
        assertEq(gross, 2_000_000);
        assertEq(fee, 80_000);
        assertEq(uint8(state), uint8(LososEscrow.State.Held));
    }

    function test_release_pays_treasury_and_the_recorded_seller() public {
        uint256 id = _pay(2_000_000, 80_000);
        vm.prank(EDGE);
        escrow.release(id);
        assertEq(token.balanceOf(TREASURY), 80_000);
        assertEq(token.balanceOf(SELLER), 1_920_000);
        assertEq(token.balanceOf(address(escrow)), 0);
        assertEq(escrow.held(), 0);
    }

    function test_a_machine_order_releases_half_and_half() public {
        uint256 id = _pay(1_000_000, 500_000);
        vm.prank(EDGE);
        escrow.release(id);
        assertEq(token.balanceOf(TREASURY), 500_000);
        assertEq(token.balanceOf(SELLER), 500_000);
    }

    function test_only_the_edge_releases() public {
        uint256 id = _pay(1_000_000, 0);
        address[3] memory others = [BUYER, SELLER, STRANGER];
        for (uint256 i = 0; i < others.length; i++) {
            vm.prank(others[i]);
            vm.expectRevert(LososEscrow.NotReleaser.selector);
            escrow.release(id);
        }
        // The treasury can rotate the edge's key but not release for it.
        vm.prank(TREASURY);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.release(id);
    }

    function test_a_deposit_is_released_or_refunded_once() public {
        uint256 id = _pay(1_000_000, 0);
        vm.prank(EDGE);
        escrow.release(id);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.release(id);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.refund(id);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.release(99);
    }

    function test_the_edge_refunds_the_whole_payment_at_any_time() public {
        uint256 id = _pay(2_000_000, 80_000);
        vm.prank(EDGE);
        escrow.refund(id);
        assertEq(token.balanceOf(BUYER), 10_000_000);
        assertEq(token.balanceOf(TREASURY), 0);
        assertEq(token.balanceOf(SELLER), 0);
        assertEq(escrow.held(), 0);
    }

    function test_the_payer_refunds_only_after_the_deadline() public {
        uint256 id = _pay(1_000_000, 0);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.refund(id);
        vm.warp(block.timestamp + 1 days);
        vm.prank(BUYER);
        escrow.refund(id);
        assertEq(token.balanceOf(BUYER), 10_000_000);
    }

    function test_nobody_else_refunds_even_after_the_deadline() public {
        uint256 id = _pay(1_000_000, 0);
        vm.warp(block.timestamp + 2 days);
        vm.prank(SELLER);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.refund(id);
        vm.prank(TREASURY);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.refund(id);
    }

    function test_the_edge_may_still_release_after_the_deadline_until_the_payer_refunds() public {
        uint256 id = _pay(1_000_000, 0);
        vm.warp(block.timestamp + 2 days);
        vm.prank(EDGE);
        escrow.release(id);
        assertEq(token.balanceOf(SELLER), 1_000_000);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.refund(id);
    }

    function test_paying_twice_makes_two_deposits() public {
        uint256 first = _pay(1_000_000, 0);
        uint256 second = _pay(1_000_000, 0);
        assertEq(second, first + 1);
        assertEq(escrow.held(), 2_000_000);
        vm.startPrank(EDGE);
        escrow.release(first);
        escrow.refund(second);
        vm.stopPrank();
        assertEq(token.balanceOf(SELLER), 1_000_000);
        assertEq(token.balanceOf(BUYER), 9_000_000);
    }

    function test_fee_over_half_reverts() public {
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.FeeTooHigh.selector);
        escrow.pay(keccak256("ord_fee"), SELLER, 1_000_000, 500_001, uint64(block.timestamp + 1 days));
    }

    function test_zero_seller_or_amount_reverts() public {
        vm.startPrank(BUYER);
        vm.expectRevert(LososEscrow.ZeroSeller.selector);
        escrow.pay(keccak256("ord_zero"), address(0), 100, 0, uint64(block.timestamp + 1 days));
        vm.expectRevert(LososEscrow.ZeroAmount.selector);
        escrow.pay(keccak256("ord_zero"), SELLER, 0, 0, uint64(block.timestamp + 1 days));
        vm.stopPrank();
    }

    function test_the_deadline_must_leave_an_hour_and_at_most_thirty_five_days() public {
        vm.startPrank(BUYER);
        vm.expectRevert(LososEscrow.BadDeadline.selector);
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 59 minutes));
        vm.expectRevert(LososEscrow.BadDeadline.selector);
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 35 days + 1));
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 1 hours));
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 35 days));
        vm.stopPrank();
    }

    function test_settle_refunds_the_undelivered_share_and_pays_the_rest() public {
        // A month of storage, the seller's box offline for the last quarter.
        uint256 id = _pay(2_000_000, 80_000);
        vm.prank(EDGE);
        escrow.settle(id, 500_000);
        assertEq(token.balanceOf(BUYER), 10_000_000 - 2_000_000 + 500_000);
        // The fee shrinks with the seller's share: 4% of 1.5 USDC.
        assertEq(token.balanceOf(TREASURY), 60_000);
        assertEq(token.balanceOf(SELLER), 1_440_000);
        assertEq(token.balanceOf(address(escrow)), 0);
        assertEq(escrow.held(), 0);
        (,,,,,, LososEscrow.State state) = escrow.deposits(id);
        assertEq(uint8(state), uint8(LososEscrow.State.Split));
    }

    function test_the_fee_of_a_split_rounds_down() public {
        uint256 id = _pay(1_000_001, 333_333);
        vm.prank(EDGE);
        escrow.settle(id, 1);
        // 333_333 * 1_000_000 / 1_000_001 = 333_332.67
        assertEq(token.balanceOf(TREASURY), 333_332);
        assertEq(token.balanceOf(SELLER), 1_000_000 - 333_332);
    }

    function test_only_the_edge_settles_and_only_a_real_split() public {
        uint256 id = _pay(1_000_000, 0);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.settle(id, 1);
        vm.prank(SELLER);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.settle(id, 999_999);
        vm.startPrank(EDGE);
        vm.expectRevert(LososEscrow.BadSplit.selector);
        escrow.settle(id, 0);
        vm.expectRevert(LososEscrow.BadSplit.selector);
        escrow.settle(id, 1_000_000);
        escrow.settle(id, 250_000);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.settle(id, 250_000);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.release(id);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.refund(id);
        vm.stopPrank();
    }

    function test_a_settled_deposit_cannot_be_reclaimed_after_the_deadline() public {
        uint256 id = _pay(1_000_000, 40_000);
        vm.prank(EDGE);
        escrow.settle(id, 400_000);
        vm.warp(block.timestamp + 2 days);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.refund(id);
    }

    function test_a_deposit_the_payer_reclaimed_cannot_be_settled() public {
        uint256 id = _pay(1_000_000, 40_000);
        vm.warp(block.timestamp + 1 days);
        vm.prank(BUYER);
        escrow.refund(id);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotHeld.selector);
        escrow.settle(id, 400_000);
    }

    function test_short_allowance_reverts_and_records_nothing() public {
        vm.prank(BUYER);
        token.approve(address(escrow), 10);
        vm.prank(BUYER);
        vm.expectRevert();
        escrow.pay(keccak256("ord_short"), SELLER, 1_000_000, 0, uint64(block.timestamp + 1 days));
        assertEq(escrow.depositCount(), 0);
        assertEq(escrow.held(), 0);
        assertEq(token.balanceOf(BUYER), 10_000_000);
    }

    function test_only_the_treasury_rotates_the_edge_key() public {
        uint256 id = _pay(1_000_000, 0);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotTreasury.selector);
        escrow.setReleaser(STRANGER);
        address next = address(0xED6F);
        vm.prank(TREASURY);
        escrow.setReleaser(next);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.release(id);
        vm.prank(next);
        escrow.release(id);
        assertEq(token.balanceOf(SELLER), 1_000_000);
    }

    function test_rescue_moves_only_stray_usdc() public {
        _pay(1_000_000, 0);
        // A buyer sent USDC by plain transfer instead of calling pay.
        token.mint(address(this), 300_000);
        token.transfer(address(escrow), 300_000);
        vm.prank(TREASURY);
        vm.expectRevert(LososEscrow.OwedToDeposits.selector);
        escrow.rescue(IERC20(address(token)), BUYER, 300_001);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotTreasury.selector);
        escrow.rescue(IERC20(address(token)), EDGE, 300_000);
        vm.prank(TREASURY);
        escrow.rescue(IERC20(address(token)), BUYER, 300_000);
        assertEq(token.balanceOf(BUYER), 9_000_000 + 300_000);
        assertEq(token.balanceOf(address(escrow)), 1_000_000);
        assertEq(escrow.held(), 1_000_000);
    }

    function test_rescue_moves_any_other_token() public {
        MockUSDC other = new MockUSDC();
        other.mint(address(escrow), 5);
        vm.prank(TREASURY);
        escrow.rescue(IERC20(address(other)), STRANGER, 5);
        assertEq(other.balanceOf(STRANGER), 5);
    }

    function test_zero_addresses_cannot_be_deployed() public {
        vm.expectRevert(LososEscrow.ZeroAddress.selector);
        new LososEscrow(IERC20(address(0)), TREASURY, EDGE);
        vm.expectRevert(LososEscrow.ZeroAddress.selector);
        new LososEscrow(IERC20(address(token)), address(0), EDGE);
        vm.expectRevert(LososEscrow.ZeroAddress.selector);
        new LososEscrow(IERC20(address(token)), TREASURY, address(0));
    }

    /// Whatever order of pays, releases, refunds and splits, the contract's
    /// balance is exactly what open deposits are owed.
    function testFuzz_balance_always_covers_open_deposits(uint8[16] calldata steps) public {
        uint256 open;
        for (uint256 i = 0; i < steps.length; i++) {
            uint256 count = escrow.depositCount();
            uint8 step = steps[i] % 4;
            if (step == 0 || count == 0) {
                _pay(uint256(steps[i]) * 1000 + 1, 0);
                continue;
            }
            uint256 id = uint256(steps[i]) % count + 1;
            (,,, uint256 gross,,, LososEscrow.State state) = escrow.deposits(id);
            if (state != LososEscrow.State.Held) continue;
            if (step == 3 && gross < 2) continue;
            vm.prank(EDGE);
            if (step == 1) escrow.release(id);
            else if (step == 2) escrow.refund(id);
            else escrow.settle(id, gross / 2);
        }
        for (uint256 id = 1; id <= escrow.depositCount(); id++) {
            (,,, uint256 gross,,, LososEscrow.State state) = escrow.deposits(id);
            if (state == LososEscrow.State.Held) open += gross;
        }
        assertEq(escrow.held(), open);
        assertEq(token.balanceOf(address(escrow)), open);
    }

    // Balances and taps.

    bytes32 internal constant TAP = keccak256("tap_test");
    /// One USDC an hour.
    uint128 internal constant RATE = 1_000_000;

    function _topUp(address buyer, uint256 amount) internal {
        vm.prank(buyer);
        escrow.topUp(amount);
    }

    function _openAs(address buyer, bytes32 id, address seller, uint128 rate, uint16 feeBps) internal {
        bytes32[] memory ids = new bytes32[](1);
        address[] memory sellers = new address[](1);
        uint128[] memory rates = new uint128[](1);
        ids[0] = id;
        sellers[0] = seller;
        rates[0] = rate;
        vm.prank(buyer);
        escrow.openTaps(ids, sellers, rates, feeBps);
    }

    function _open(bytes32 id, uint128 rate, uint16 feeBps) internal {
        _openAs(BUYER, id, SELLER, rate, feeBps);
    }

    function _settle(bytes32 id, uint256 amount) internal {
        vm.prank(EDGE);
        escrow.settleTap(id, amount);
    }

    function _tap(bytes32 id) internal view returns (LososEscrow.Tap memory t) {
        (t.buyer, t.seller, t.rate, t.feeBps, t.settledAt, t.closedAt) = escrow.taps(id);
    }

    function _withdrawal(address buyer) internal view returns (uint128 amount, uint64 notBefore) {
        (amount, notBefore) = escrow.withdrawals(buyer);
    }

    function _arrays(uint256 n)
        internal
        pure
        returns (bytes32[] memory ids, address[] memory sellers, uint128[] memory rates)
    {
        ids = new bytes32[](n);
        sellers = new address[](n);
        rates = new uint128[](n);
        for (uint256 i = 0; i < n; i++) {
            ids[i] = keccak256(abi.encode("tap", i));
            sellers[i] = address(uint160(0x5E1100 + i));
            rates[i] = RATE;
        }
    }

    function test_the_tap_constants() public view {
        assertEq(escrow.NOTICE(), 48 hours);
        assertEq(escrow.MAX_ACCRUAL(), 7 days);
        assertEq(escrow.HEADSTART(), 1 hours);
        assertEq(escrow.MAX_TAPS_PER_CALL(), 16);
        assertEq(escrow.MAX_FEE_BPS(), 5000);
    }

    function test_top_up_holds_the_money_on_the_buyers_balance() public {
        vm.expectEmit(true, false, false, true, address(escrow));
        emit LososEscrow.ToppedUp(BUYER, 2_000_000, 2_000_000);
        _topUp(BUYER, 2_000_000);
        vm.expectEmit(true, false, false, true, address(escrow));
        emit LososEscrow.ToppedUp(BUYER, 500_000, 2_500_000);
        _topUp(BUYER, 500_000);
        assertEq(escrow.balances(BUYER), 2_500_000);
        assertEq(escrow.held(), 2_500_000);
        assertEq(token.balanceOf(address(escrow)), 2_500_000);
        assertEq(token.balanceOf(BUYER), 7_500_000);
        assertEq(token.balanceOf(SELLER), 0);
        assertEq(token.balanceOf(TREASURY), 0);
    }

    function test_top_up_of_zero_or_past_the_allowance_reverts_and_records_nothing() public {
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.ZeroAmount.selector);
        escrow.topUp(0);
        vm.prank(BUYER);
        token.approve(address(escrow), 10);
        vm.prank(BUYER);
        vm.expectRevert();
        escrow.topUp(11);
        assertEq(escrow.balances(BUYER), 0);
        assertEq(escrow.held(), 0);
        assertEq(token.balanceOf(BUYER), 10_000_000);
    }

    function test_open_taps_records_one_tap_per_seller() public {
        bytes32[] memory ids = new bytes32[](2);
        address[] memory sellers = new address[](2);
        uint128[] memory rates = new uint128[](2);
        ids[0] = keccak256("tap_a");
        ids[1] = keccak256("tap_b");
        sellers[0] = SELLER;
        sellers[1] = SELLER2;
        rates[0] = 100;
        rates[1] = 200;
        vm.warp(1_800_000_000);
        vm.expectEmit(true, true, true, true, address(escrow));
        emit LososEscrow.TapOpened(ids[0], BUYER, SELLER, 100, 400);
        vm.expectEmit(true, true, true, true, address(escrow));
        emit LososEscrow.TapOpened(ids[1], BUYER, SELLER2, 200, 400);
        vm.prank(BUYER);
        escrow.openTaps(ids, sellers, rates, 400);
        LososEscrow.Tap memory a = _tap(ids[0]);
        assertEq(a.buyer, BUYER);
        assertEq(a.seller, SELLER);
        assertEq(a.rate, 100);
        assertEq(a.feeBps, 400);
        // Billing counts from an hour before the tap opened.
        assertEq(a.settledAt, 1_800_000_000 - 1 hours);
        assertEq(a.closedAt, 0);
        LososEscrow.Tap memory b = _tap(ids[1]);
        assertEq(b.seller, SELLER2);
        assertEq(b.rate, 200);
        // Opening a tap moves no money.
        assertEq(escrow.held(), 0);
        assertEq(token.balanceOf(address(escrow)), 0);
    }

    function test_a_tap_fee_over_half_reverts() public {
        (bytes32[] memory ids, address[] memory sellers, uint128[] memory rates) = _arrays(1);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.FeeTooHigh.selector);
        escrow.openTaps(ids, sellers, rates, 5001);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.FeeTooHigh.selector);
        escrow.openTaps(ids, sellers, rates, type(uint16).max);
        vm.prank(BUYER);
        escrow.openTaps(ids, sellers, rates, 5000);
        assertEq(_tap(ids[0]).feeBps, 5000);
    }

    function test_a_tap_to_the_zero_seller_reverts_the_whole_call() public {
        (bytes32[] memory ids, address[] memory sellers, uint128[] memory rates) = _arrays(3);
        sellers[2] = address(0);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.ZeroSeller.selector);
        escrow.openTaps(ids, sellers, rates, 400);
        assertEq(_tap(ids[0]).buyer, address(0));
        assertEq(_tap(ids[1]).buyer, address(0));
    }

    function test_a_tap_id_opens_once() public {
        _open(TAP, RATE, 400);
        // Not again by the same buyer, nor by anyone else to take it over.
        vm.expectRevert(LososEscrow.TapExists.selector);
        this.openAs(BUYER, TAP, SELLER2);
        vm.expectRevert(LososEscrow.TapExists.selector);
        this.openAs(STRANGER, TAP, STRANGER);
        // Nor after it closed.
        vm.prank(BUYER);
        escrow.closeTap(TAP);
        vm.expectRevert(LososEscrow.TapExists.selector);
        this.openAs(BUYER, TAP, SELLER);
        LososEscrow.Tap memory t = _tap(TAP);
        assertEq(t.buyer, BUYER);
        assertEq(t.seller, SELLER);
        // Nor twice in one call.
        (bytes32[] memory ids, address[] memory sellers, uint128[] memory rates) = _arrays(2);
        ids[1] = ids[0];
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.TapExists.selector);
        escrow.openTaps(ids, sellers, rates, 400);
        assertEq(_tap(ids[0]).buyer, address(0));
    }

    /// External so a test can expect the revert of a call made through it.
    function openAs(address buyer, bytes32 id, address seller) external {
        _openAs(buyer, id, seller, RATE, 400);
    }

    function test_open_taps_needs_matching_non_empty_arrays() public {
        (bytes32[] memory ids, address[] memory sellers, uint128[] memory rates) = _arrays(2);
        (bytes32[] memory one, address[] memory oneSeller, uint128[] memory oneRate) = _arrays(1);
        vm.startPrank(BUYER);
        vm.expectRevert(LososEscrow.BadTaps.selector);
        escrow.openTaps(ids, oneSeller, rates, 400);
        vm.expectRevert(LososEscrow.BadTaps.selector);
        escrow.openTaps(ids, sellers, oneRate, 400);
        vm.expectRevert(LososEscrow.BadTaps.selector);
        escrow.openTaps(one, sellers, rates, 400);
        vm.expectRevert(LososEscrow.BadTaps.selector);
        escrow.openTaps(new bytes32[](0), new address[](0), new uint128[](0), 400);
        vm.stopPrank();
    }

    function test_at_most_sixteen_taps_per_call() public {
        (bytes32[] memory ids, address[] memory sellers, uint128[] memory rates) = _arrays(17);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.TooManyTaps.selector);
        escrow.openTaps(ids, sellers, rates, 400);
        (ids, sellers, rates) = _arrays(16);
        vm.prank(BUYER);
        escrow.openTaps(ids, sellers, rates, 400);
        for (uint256 i = 0; i < 16; i++) {
            assertEq(_tap(ids[i]).seller, sellers[i]);
        }
    }

    function test_settle_tap_pays_the_treasury_and_the_taps_seller() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        vm.expectEmit(true, true, false, true, address(escrow));
        emit LososEscrow.TapSettled(TAP, SELLER, 1_000_000, 40_000, uint64(block.timestamp));
        _settle(TAP, 1_000_000);
        assertEq(token.balanceOf(TREASURY), 40_000);
        assertEq(token.balanceOf(SELLER), 960_000);
        assertEq(escrow.balances(BUYER), 4_000_000);
        assertEq(escrow.held(), 4_000_000);
        assertEq(token.balanceOf(address(escrow)), 4_000_000);
        assertEq(_tap(TAP).settledAt, block.timestamp);
    }

    function test_settle_tap_bills_less_than_the_cap_and_a_half_fee() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 5000);
        vm.warp(block.timestamp + 2 hours);
        _settle(TAP, 300_001);
        // Half of 300_001, rounded down, to the treasury.
        assertEq(token.balanceOf(TREASURY), 150_000);
        assertEq(token.balanceOf(SELLER), 150_001);
        assertEq(escrow.balances(BUYER), 5_000_000 - 300_001);
    }

    function test_the_fee_of_a_tap_slice_rounds_down() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        // 1_249 * 400 / 10_000 = 49.96
        _settle(TAP, 1_249);
        assertEq(token.balanceOf(TREASURY), 49);
        assertEq(token.balanceOf(SELLER), 1_200);
        vm.warp(block.timestamp + 1 hours);
        // 24 * 400 / 10_000 = 0.96: the whole slice goes to the seller.
        _settle(TAP, 24);
        assertEq(token.balanceOf(TREASURY), 49);
        assertEq(token.balanceOf(SELLER), 1_224);
    }

    function test_a_tap_without_a_fee_pays_the_seller_everything() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 0);
        vm.warp(block.timestamp + 1 hours);
        _settle(TAP, RATE);
        assertEq(token.balanceOf(TREASURY), 0);
        assertEq(token.balanceOf(SELLER), RATE);
    }

    function test_only_the_edge_settles_a_tap() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        address[4] memory others = [BUYER, SELLER, TREASURY, STRANGER];
        for (uint256 i = 0; i < others.length; i++) {
            vm.prank(others[i]);
            vm.expectRevert(LososEscrow.NotReleaser.selector);
            escrow.settleTap(TAP, 1);
        }
        assertEq(escrow.balances(BUYER), 5_000_000);
    }

    function test_the_edge_cannot_settle_more_than_the_rate_for_the_time_elapsed() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, 3600, 400);
        vm.warp(block.timestamp + 10);
        // The hour of headstart and ten seconds, at one a second.
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, 3611);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, type(uint256).max);
        _settle(TAP, 3610);
        assertEq(escrow.balances(BUYER), 5_000_000 - 3610);
    }

    function test_the_cap_rounds_down_to_whole_base_units() public {
        _topUp(BUYER, 5_000_000);
        // 1_000 an hour is 0.28 a second.
        _open(TAP, 1_000, 400);
        _settle(TAP, 1_000);
        vm.warp(block.timestamp + 1);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, 1);
        vm.warp(block.timestamp + 3);
        _settle(TAP, 1);
    }

    function test_a_zero_rate_tap_can_never_be_settled() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, 0, 400);
        vm.warp(block.timestamp + 30 days);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, 1);
    }

    function test_unused_cap_is_not_carried_over() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        _settle(TAP, RATE / 2);
        vm.warp(block.timestamp + 1 hours);
        // What the first settle left unbilled is gone.
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, RATE + 1);
        _settle(TAP, RATE);
        assertEq(escrow.balances(BUYER), 5_000_000 - RATE / 2 - RATE);
    }

    function test_a_tap_cannot_be_settled_twice_in_one_second() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        // The headstart can be billed in the second the tap opened, once.
        _settle(TAP, 1);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.settleTap(TAP, 1);
        vm.warp(block.timestamp + 1 hours);
        _settle(TAP, 1);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.settleTap(TAP, 1);
        vm.warp(block.timestamp + 1 seconds);
        // One second at one USDC an hour is 277 base units.
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, 278);
        _settle(TAP, 277);
    }

    function test_a_closed_tap_settles_only_up_to_its_close() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        uint64 closedAt = uint64(block.timestamp);
        vm.prank(BUYER);
        escrow.closeTap(TAP);
        vm.warp(block.timestamp + 10 days);
        // The hour it was open and the hour of headstart, no more.
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, 2 * RATE + 1);
        _settle(TAP, RATE / 4);
        LososEscrow.Tap memory t = _tap(TAP);
        assertEq(t.settledAt, closedAt);
        assertEq(t.closedAt, closedAt);
        // Billed up to the close: nothing more, however long the edge waits.
        vm.warp(block.timestamp + 10 days);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TapIsClosed.selector);
        escrow.settleTap(TAP, 1);
        assertEq(escrow.balances(BUYER), 5_000_000 - RATE / 4);
    }

    function test_the_first_hour_can_be_billed_right_after_open() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        // No time has passed, and the hour of headstart is billable.
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, RATE + 1);
        _settle(TAP, RATE);
        assertEq(_tap(TAP).settledAt, block.timestamp);
        assertEq(token.balanceOf(SELLER), 960_000);
        assertEq(token.balanceOf(TREASURY), 40_000);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.settleTap(TAP, 1);
    }

    function test_a_tap_closed_in_the_second_it_opened_bills_only_the_headstart() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.prank(BUYER);
        escrow.closeTap(TAP);
        vm.warp(block.timestamp + 1 days);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, RATE + 1);
        _settle(TAP, RATE);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TapIsClosed.selector);
        escrow.settleTap(TAP, 1);
        assertEq(escrow.balances(BUYER), 5_000_000 - RATE);
    }

    function test_accrual_is_capped_at_a_week_even_after_months() public {
        _topUp(BUYER, 10_000_000);
        _open(TAP, 1_000, 400);
        vm.warp(block.timestamp + 90 days);
        uint256 week = uint256(1_000) * 7 * 24;
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, week + 1);
        _settle(TAP, week);
        // The months before the last week are gone, not owed.
        assertEq(_tap(TAP).settledAt, block.timestamp);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.settleTap(TAP, 1);
        vm.warp(block.timestamp + 60 days);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, week + 1);
        _settle(TAP, week);
        assertEq(escrow.balances(BUYER), 10_000_000 - 2 * week);
    }

    function test_a_tap_closed_months_after_it_opened_is_still_capped_at_a_week() public {
        _topUp(BUYER, 10_000_000);
        _open(TAP, 1_000, 400);
        vm.warp(block.timestamp + 60 days);
        vm.prank(BUYER);
        escrow.closeTap(TAP);
        vm.warp(block.timestamp + 60 days);
        uint256 week = uint256(1_000) * 7 * 24;
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverCap.selector);
        escrow.settleTap(TAP, week + 1);
        _settle(TAP, week);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TapIsClosed.selector);
        escrow.settleTap(TAP, 1);
    }

    function test_the_edge_cannot_settle_more_than_the_balance() public {
        _topUp(BUYER, 500_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverBalance.selector);
        escrow.settleTap(TAP, 500_001);
        // A failed settle moves the tap's clock no further.
        assertEq(_tap(TAP).settledAt, block.timestamp - 2 hours);
        _settle(TAP, 500_000);
        assertEq(escrow.balances(BUYER), 0);
        assertEq(escrow.held(), 0);
        assertEq(token.balanceOf(address(escrow)), 0);
        vm.warp(block.timestamp + 1 hours);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverBalance.selector);
        escrow.settleTap(TAP, 1);
    }

    function test_a_tap_bills_only_its_own_buyers_balance_and_never_deposits() public {
        _topUp(BUYER, 100);
        _topUp(_fundBuyer2(), 5_000_000);
        _pay(2_000_000, 0);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        // The other buyer's balance and the open deposit could cover it, but
        // neither is this tap's to draw.
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.OverBalance.selector);
        escrow.settleTap(TAP, 101);
        assertEq(escrow.balances(BUYER2), 5_000_000);
        assertEq(escrow.held(), 7_000_100);
    }

    function _fundBuyer2() internal returns (address) {
        token.mint(BUYER2, 10_000_000);
        vm.prank(BUYER2);
        token.approve(address(escrow), type(uint256).max);
        return BUYER2;
    }

    function test_settle_needs_a_real_tap_and_a_real_amount() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NoTap.selector);
        escrow.settleTap(keccak256("tap_nope"), 1);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.ZeroAmount.selector);
        escrow.settleTap(TAP, 0);
    }

    function test_the_buyer_or_the_edge_closes_a_tap_once() public {
        bytes32 other = keccak256("tap_other");
        _open(TAP, RATE, 400);
        _open(other, RATE, 400);
        address[3] memory others = [SELLER, TREASURY, STRANGER];
        for (uint256 i = 0; i < others.length; i++) {
            vm.prank(others[i]);
            vm.expectRevert(LososEscrow.NotReleaser.selector);
            escrow.closeTap(TAP);
        }
        vm.warp(block.timestamp + 5 minutes);
        vm.expectEmit(true, true, false, true, address(escrow));
        emit LososEscrow.TapClosed(TAP, BUYER, uint64(block.timestamp));
        vm.prank(BUYER);
        escrow.closeTap(TAP);
        assertEq(_tap(TAP).closedAt, block.timestamp);
        vm.expectEmit(true, true, false, true, address(escrow));
        emit LososEscrow.TapClosed(other, BUYER, uint64(block.timestamp));
        vm.prank(EDGE);
        escrow.closeTap(other);
        vm.warp(block.timestamp + 1 hours);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.TapIsClosed.selector);
        escrow.closeTap(TAP);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.TapIsClosed.selector);
        escrow.closeTap(other);
        // Closing again did not move the close later.
        assertEq(_tap(TAP).closedAt, block.timestamp - 1 hours);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.NoTap.selector);
        escrow.closeTap(keccak256("tap_nope"));
    }

    function test_another_buyer_cannot_close_someone_elses_tap() public {
        _open(TAP, RATE, 400);
        _fundBuyer2();
        vm.prank(BUYER2);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.closeTap(TAP);
        assertEq(_tap(TAP).closedAt, 0);
    }

    function test_a_withdrawal_waits_for_the_notice() public {
        _topUp(BUYER, 2_000_000);
        vm.expectEmit(true, false, false, true, address(escrow));
        emit LososEscrow.WithdrawalAsked(BUYER, 1_500_000, uint64(block.timestamp + 48 hours));
        vm.prank(BUYER);
        escrow.askWithdrawal(1_500_000);
        (uint128 amount, uint64 notBefore) = _withdrawal(BUYER);
        assertEq(amount, 1_500_000);
        assertEq(notBefore, block.timestamp + 48 hours);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.withdraw();
        vm.warp(notBefore - 1);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.withdraw();
        vm.warp(notBefore);
        vm.expectEmit(true, false, false, true, address(escrow));
        emit LososEscrow.Withdrawn(BUYER, 1_500_000);
        vm.prank(BUYER);
        escrow.withdraw();
        assertEq(token.balanceOf(BUYER), 10_000_000 - 500_000);
        assertEq(escrow.balances(BUYER), 500_000);
        assertEq(escrow.held(), 500_000);
        assertEq(token.balanceOf(address(escrow)), 500_000);
        (amount, notBefore) = _withdrawal(BUYER);
        assertEq(amount, 0);
        assertEq(notBefore, 0);
    }

    function test_a_withdrawal_pays_once() public {
        _topUp(BUYER, 2_000_000);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.NoWithdrawal.selector);
        escrow.withdraw();
        vm.prank(BUYER);
        escrow.askWithdrawal(500_000);
        vm.warp(block.timestamp + 48 hours);
        vm.prank(BUYER);
        escrow.withdraw();
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.NoWithdrawal.selector);
        escrow.withdraw();
        assertEq(escrow.balances(BUYER), 1_500_000);
        assertEq(token.balanceOf(BUYER), 8_500_000);
    }

    function test_a_second_ask_replaces_the_first_and_restarts_the_notice() public {
        _topUp(BUYER, 2_000_000);
        vm.prank(BUYER);
        escrow.askWithdrawal(100_000);
        vm.warp(block.timestamp + 47 hours);
        vm.prank(BUYER);
        escrow.askWithdrawal(200_000);
        (uint128 amount, uint64 notBefore) = _withdrawal(BUYER);
        assertEq(amount, 200_000);
        assertEq(notBefore, block.timestamp + 48 hours);
        // The first ask's notice is over, but it no longer exists.
        vm.warp(block.timestamp + 1 hours);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.TooEarly.selector);
        escrow.withdraw();
        vm.warp(notBefore);
        vm.prank(BUYER);
        escrow.withdraw();
        assertEq(token.balanceOf(BUYER), 8_000_000 + 200_000);
        assertEq(escrow.balances(BUYER), 1_800_000);
    }

    function test_a_withdrawal_pays_what_is_left_after_the_edge_settled_during_the_notice() public {
        _topUp(BUYER, 1_000_000);
        _open(TAP, RATE, 400);
        vm.prank(BUYER);
        escrow.askWithdrawal(1_000_000);
        vm.warp(block.timestamp + 24 hours);
        // The edge bills the last slice while the notice runs.
        _settle(TAP, 300_000);
        vm.prank(BUYER);
        escrow.closeTap(TAP);
        vm.warp(block.timestamp + 24 hours);
        vm.expectEmit(true, false, false, true, address(escrow));
        emit LososEscrow.Withdrawn(BUYER, 700_000);
        vm.prank(BUYER);
        escrow.withdraw();
        assertEq(token.balanceOf(BUYER), 9_000_000 + 700_000);
        assertEq(token.balanceOf(SELLER), 288_000);
        assertEq(token.balanceOf(TREASURY), 12_000);
        assertEq(escrow.balances(BUYER), 0);
        assertEq(escrow.held(), 0);
        assertEq(token.balanceOf(address(escrow)), 0);
    }

    function test_a_withdrawal_after_the_edge_billed_everything_pays_nothing_and_clears_the_ask() public {
        _topUp(BUYER, 1_000_000);
        _open(TAP, RATE, 400);
        vm.prank(BUYER);
        escrow.askWithdrawal(1_000_000);
        vm.warp(block.timestamp + 48 hours);
        _settle(TAP, 1_000_000);
        vm.prank(BUYER);
        escrow.withdraw();
        assertEq(token.balanceOf(BUYER), 9_000_000);
        (, uint64 notBefore) = _withdrawal(BUYER);
        assertEq(notBefore, 0);
    }

    function test_asking_for_more_than_the_balance_withdraws_all_of_it() public {
        _topUp(BUYER, 1_000_000);
        vm.prank(BUYER);
        vm.expectRevert(LososEscrow.ZeroAmount.selector);
        escrow.askWithdrawal(0);
        vm.prank(BUYER);
        escrow.askWithdrawal(type(uint256).max);
        (uint128 amount,) = _withdrawal(BUYER);
        assertEq(amount, type(uint128).max);
        vm.warp(block.timestamp + 48 hours);
        vm.prank(BUYER);
        escrow.withdraw();
        assertEq(token.balanceOf(BUYER), 10_000_000);
        assertEq(escrow.balances(BUYER), 0);
    }

    function test_an_ask_just_over_uint128_is_not_truncated() public {
        _topUp(BUYER, 1_000_000);
        vm.prank(BUYER);
        escrow.askWithdrawal(uint256(type(uint128).max) + 2);
        (uint128 amount,) = _withdrawal(BUYER);
        assertEq(amount, type(uint128).max);
    }

    function test_nobody_else_withdraws_a_buyers_balance() public {
        _topUp(BUYER, 1_000_000);
        vm.prank(BUYER);
        escrow.askWithdrawal(1_000_000);
        address[4] memory others = [EDGE, SELLER, TREASURY, STRANGER];
        for (uint256 i = 0; i < others.length; i++) {
            vm.prank(others[i]);
            escrow.askWithdrawal(1_000_000);
        }
        vm.warp(block.timestamp + 48 hours);
        for (uint256 i = 0; i < others.length; i++) {
            vm.prank(others[i]);
            escrow.withdraw();
            assertEq(token.balanceOf(others[i]), 0);
        }
        assertEq(escrow.balances(BUYER), 1_000_000);
        vm.prank(BUYER);
        escrow.withdraw();
        assertEq(token.balanceOf(BUYER), 10_000_000);
    }

    function test_a_rotated_edge_key_settles_and_the_old_one_does_not() public {
        _topUp(BUYER, 5_000_000);
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        address next = address(0xED6F);
        vm.prank(TREASURY);
        escrow.setReleaser(next);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.settleTap(TAP, 1);
        vm.prank(EDGE);
        vm.expectRevert(LososEscrow.NotReleaser.selector);
        escrow.closeTap(TAP);
        vm.prank(next);
        escrow.settleTap(TAP, RATE);
        assertEq(token.balanceOf(SELLER), 960_000);
    }

    function test_rescue_cannot_move_balance_money() public {
        _topUp(BUYER, 1_000_000);
        _pay(500_000, 0);
        token.mint(address(this), 300_000);
        token.transfer(address(escrow), 300_000);
        assertEq(escrow.held(), 1_500_000);
        vm.prank(TREASURY);
        vm.expectRevert(LososEscrow.OwedToDeposits.selector);
        escrow.rescue(IERC20(address(token)), TREASURY, 300_001);
        vm.prank(TREASURY);
        escrow.rescue(IERC20(address(token)), TREASURY, 300_000);
        assertEq(token.balanceOf(address(escrow)), 1_500_000);
        // Settled and withdrawn money leaves `held` too, so nothing becomes
        // rescuable that a balance is still owed.
        _open(TAP, RATE, 400);
        vm.warp(block.timestamp + 1 hours);
        _settle(TAP, 200_000);
        vm.prank(BUYER);
        escrow.askWithdrawal(300_000);
        vm.warp(block.timestamp + 48 hours);
        vm.prank(BUYER);
        escrow.withdraw();
        assertEq(escrow.held(), 1_000_000);
        assertEq(token.balanceOf(address(escrow)), 1_000_000);
        vm.prank(TREASURY);
        vm.expectRevert(LososEscrow.OwedToDeposits.selector);
        escrow.rescue(IERC20(address(token)), TREASURY, 1);
    }

    /// Whatever order of pays, releases, refunds, top-ups, taps, settles,
    /// closes and withdrawals, by two buyers over any stretch of time,
    /// `held` is exactly what open deposits and balances are owed, the
    /// contract holds exactly that, and no token is made or lost.
    function testFuzz_held_always_covers_deposits_and_balances(uint16[32] calldata steps) public {
        _fundBuyer2();
        address[2] memory buyers = [BUYER, BUYER2];
        bytes32[] memory tapIds = new bytes32[](steps.length);
        uint256 tapCount;
        for (uint256 i = 0; i < steps.length; i++) {
            uint16 s = steps[i];
            address b = buyers[s & 1];
            uint256 op = (s >> 1) % 9;
            uint256 x = s >> 5;
            if (op == 0) {
                vm.prank(b);
                escrow.pay(keccak256("ord_fuzz"), SELLER, x * 1000 + 1, 0, uint64(block.timestamp + 1 days));
            } else if (op == 1) {
                uint256 count = escrow.depositCount();
                if (count == 0) continue;
                uint256 id = x % count + 1;
                (,,,,,, LososEscrow.State state) = escrow.deposits(id);
                if (state != LososEscrow.State.Held) continue;
                vm.prank(EDGE);
                if (x & 1 == 0) escrow.release(id);
                else escrow.refund(id);
            } else if (op == 2) {
                _topUp(b, x * 500 + 1);
            } else if (op == 3) {
                bytes32 id = keccak256(abi.encode("tap_fuzz", i));
                _openAs(b, id, x & 1 == 0 ? SELLER : SELLER2, uint128(x * 100), uint16(x % 5001));
                tapIds[tapCount++] = id;
            } else if (op == 4 || op == 5) {
                if (tapCount == 0) continue;
                bytes32 id = tapIds[x % tapCount];
                LososEscrow.Tap memory t = _tap(id);
                uint64 end = t.closedAt != 0 ? t.closedAt : uint64(block.timestamp);
                if (end <= t.settledAt) continue;
                uint256 elapsed = end - t.settledAt;
                if (elapsed > 7 days) elapsed = 7 days;
                uint256 amount = uint256(t.rate) * elapsed / 1 hours;
                uint256 balance = escrow.balances(t.buyer);
                if (amount > balance) amount = balance;
                if (op == 5) amount = amount / 2;
                if (amount == 0) continue;
                _settle(id, amount);
            } else if (op == 6) {
                if (tapCount == 0) continue;
                bytes32 id = tapIds[x % tapCount];
                LososEscrow.Tap memory t = _tap(id);
                if (t.closedAt != 0) continue;
                vm.prank(x & 1 == 0 ? t.buyer : EDGE);
                escrow.closeTap(id);
            } else if (op == 7) {
                vm.prank(b);
                escrow.askWithdrawal(x * 700 + 1);
                (, uint64 notBefore) = _withdrawal(b);
                if (x & 1 == 0) {
                    vm.warp(notBefore);
                    vm.prank(b);
                    escrow.withdraw();
                }
            } else {
                vm.warp(block.timestamp + x * 97);
                (, uint64 notBefore) = _withdrawal(b);
                if (notBefore != 0 && block.timestamp >= notBefore) {
                    vm.prank(b);
                    escrow.withdraw();
                }
            }
        }
        uint256 owed = escrow.balances(BUYER) + escrow.balances(BUYER2);
        for (uint256 id = 1; id <= escrow.depositCount(); id++) {
            (,,, uint256 gross,,, LososEscrow.State state) = escrow.deposits(id);
            if (state == LososEscrow.State.Held) owed += gross;
        }
        assertEq(escrow.held(), owed);
        assertEq(token.balanceOf(address(escrow)), owed);
        assertEq(
            token.balanceOf(BUYER) + token.balanceOf(BUYER2) + token.balanceOf(SELLER) + token.balanceOf(SELLER2)
                + token.balanceOf(TREASURY) + token.balanceOf(address(escrow)),
            20_000_000
        );
        // Only a stray transfer is ever rescuable.
        token.mint(address(escrow), 7);
        vm.prank(TREASURY);
        // With nothing owed, asking for more than the contract has
        // underflows before the check: a panic rather than OwedToDeposits.
        if (owed == 0) vm.expectRevert(stdError.arithmeticError);
        else vm.expectRevert(LososEscrow.OwedToDeposits.selector);
        escrow.rescue(IERC20(address(token)), STRANGER, 8);
        vm.prank(TREASURY);
        escrow.rescue(IERC20(address(token)), STRANGER, 7);
        assertEq(token.balanceOf(address(escrow)), owed);
    }
}
