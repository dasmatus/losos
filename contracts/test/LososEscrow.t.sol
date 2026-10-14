// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {IERC20, LososEscrow} from "../src/LososEscrow.sol";

/// Transaction tests for `LososEscrow`. The buyer, seller, treasury and edge
/// are separate addresses, and `vm.prank` makes each call come from the one
/// a wallet would send it from.
contract LososEscrowTest is Test {
    address internal constant BUYER = address(0xB0B);
    address internal constant SELLER = address(0x5E11);
    address internal constant TREASURY = address(0x7EA5);
    address internal constant EDGE = address(0xED6E);
    address internal constant STRANGER = address(0xBAD);

    EscrowUsdc internal token;
    LososEscrow internal escrow;

    function setUp() public {
        token = new EscrowUsdc();
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

    function test_the_deadline_must_leave_an_hour_and_at_most_thirty_days() public {
        vm.startPrank(BUYER);
        vm.expectRevert(LososEscrow.BadDeadline.selector);
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 59 minutes));
        vm.expectRevert(LososEscrow.BadDeadline.selector);
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 30 days + 1));
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 1 hours));
        escrow.pay(keccak256("ord_d"), SELLER, 100, 0, uint64(block.timestamp + 30 days));
        vm.stopPrank();
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
        EscrowUsdc other = new EscrowUsdc();
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

    /// Whatever order of pays, releases and refunds, the contract's balance
    /// is exactly what open deposits are owed.
    function testFuzz_balance_always_covers_open_deposits(uint8[16] calldata steps) public {
        uint256 open;
        for (uint256 i = 0; i < steps.length; i++) {
            uint256 count = escrow.depositCount();
            uint8 step = steps[i] % 3;
            if (step == 0 || count == 0) {
                _pay(uint256(steps[i]) * 1000 + 1, 0);
                continue;
            }
            uint256 id = uint256(steps[i]) % count + 1;
            (,,,,,, LososEscrow.State state) = escrow.deposits(id);
            if (state != LososEscrow.State.Held) continue;
            vm.prank(EDGE);
            if (step == 1) escrow.release(id);
            else escrow.refund(id);
        }
        for (uint256 id = 1; id <= escrow.depositCount(); id++) {
            (,,, uint256 gross,,, LososEscrow.State state) = escrow.deposits(id);
            if (state == LososEscrow.State.Held) open += gross;
        }
        assertEq(escrow.held(), open);
        assertEq(token.balanceOf(address(escrow)), open);
    }
}

/// Six-decimal USDC stand-in with the `transfer` the escrow pays out with.
contract EscrowUsdc {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount, "balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        require(allowed >= amount, "allowance");
        if (allowed != type(uint256).max) {
            allowance[from][msg.sender] = allowed - amount;
        }
        require(balanceOf[from] >= amount, "balance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}
