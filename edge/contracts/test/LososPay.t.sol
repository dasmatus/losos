// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

import {IERC20, LososPay} from "../src/LososPay.sol";
import {MockUSDC} from "../src/MockUSDC.sol";

/// Transaction tests for `LososPay`. No forge-std: the test contract is the
/// buyer, which is what a wallet is on chain.
contract LososPayTest {
    address constant TREASURY = address(0x1111111111111111111111111111111111111111);
    address constant SELLER = address(0x2222222222222222222222222222222222222222);

    MockUSDC internal token;
    LososPay internal pay;

    function setUp() public {
        token = new MockUSDC();
        pay = new LososPay(IERC20(address(token)), TREASURY);
    }

    function test_pay_splits_fee_and_seller() public {
        token.mint(address(this), 2_000_000);
        token.approve(address(pay), 2_000_000);
        bytes32 orderId = keccak256("ord_test");
        pay.pay(orderId, SELLER, 2_000_000, 80_000);
        assert(token.balanceOf(TREASURY) == 80_000);
        assert(token.balanceOf(SELLER) == 1_920_000);
        assert(token.balanceOf(address(this)) == 0);
    }

    function test_zero_fee_pays_the_seller_everything() public {
        token.mint(address(this), 500_000);
        token.approve(address(pay), 500_000);
        pay.pay(keccak256("ord_nofee"), SELLER, 500_000, 0);
        assert(token.balanceOf(TREASURY) == 0);
        assert(token.balanceOf(SELLER) == 500_000);
    }

    function test_fee_over_half_reverts() public {
        token.mint(address(this), 1_000_000);
        token.approve(address(pay), 1_000_000);
        // Half is the ceiling. One base unit over it must revert.
        (bool ok,) = address(pay).call(
            abi.encodeCall(LososPay.pay, (keccak256("ord_fee"), SELLER, 1_000_000, 500_001))
        );
        assert(!ok);
        assert(token.balanceOf(address(this)) == 1_000_000);
    }

    function test_a_machine_order_splits_half_and_half() public {
        token.mint(address(this), 1_000_000);
        token.approve(address(pay), 1_000_000);
        pay.pay(keccak256("ord_vm"), SELLER, 1_000_000, 500_000);
        assert(token.balanceOf(TREASURY) == 500_000);
        assert(token.balanceOf(SELLER) == 500_000);
    }

    function test_a_zero_token_or_treasury_cannot_be_deployed() public {
        bool ok;
        try new LososPay(IERC20(address(0)), TREASURY) {
            ok = true;
        } catch {}
        assert(!ok);
        try new LososPay(IERC20(address(token)), address(0)) {
            ok = true;
        } catch {}
        assert(!ok);
    }

    function test_zero_seller_reverts() public {
        token.mint(address(this), 100);
        token.approve(address(pay), 100);
        (bool ok,) = address(pay).call(
            abi.encodeCall(LososPay.pay, (keccak256("ord_zero"), address(0), 100, 0))
        );
        assert(!ok);
    }

    function test_short_allowance_reverts_and_moves_nothing() public {
        token.mint(address(this), 1_000_000);
        token.approve(address(pay), 10);
        (bool ok,) =
            address(pay).call(abi.encodeCall(LososPay.pay, (keccak256("ord_short"), SELLER, 1_000_000, 40_000)));
        assert(!ok);
        assert(token.balanceOf(TREASURY) == 0);
        assert(token.balanceOf(SELLER) == 0);
        assert(token.balanceOf(address(this)) == 1_000_000);
    }
}
