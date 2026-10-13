// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {IERC20, LososPay} from "../src/LososPay.sol";

/// Pays Circle's Base Sepolia USDC through `LososPay` on a fork of the
/// public chain. Skipped unless `BASE_SEPOLIA_RPC` is set.
/// `contracts/test-payment.sh` sets it when https://sepolia.base.org answers.
contract LososPayForkTest is Test {
    address internal constant USDC = 0x036CbD53842c5426634e7929541eC2318f3dCF7e;
    address internal constant TREASURY = address(0x1111111111111111111111111111111111111111);
    address internal constant SELLER = address(0x2222222222222222222222222222222222222222);

    function test_fork_pay_circle_usdc_on_base_sepolia() public {
        string memory rpc = vm.envOr("BASE_SEPOLIA_RPC", string(""));
        if (bytes(rpc).length == 0) {
            vm.skip(true);
            return;
        }
        vm.createSelectFork(rpc);
        LososPay market = new LososPay(IERC20(USDC), TREASURY);
        // Circle's token is a proxy. forge-std finds the balance slot.
        deal(USDC, address(this), 2_000_000);
        assertEq(CircleUsdc(USDC).balanceOf(address(this)), 2_000_000);
        assertTrue(CircleUsdc(USDC).approve(address(market), 2_000_000));
        uint256 treasuryBefore = CircleUsdc(USDC).balanceOf(TREASURY);
        uint256 sellerBefore = CircleUsdc(USDC).balanceOf(SELLER);
        market.pay(keccak256("ord_fork"), SELLER, 2_000_000, 80_000);
        // These addresses may already hold test USDC. The payment is the delta.
        assertEq(CircleUsdc(USDC).balanceOf(TREASURY), treasuryBefore + 80_000);
        assertEq(CircleUsdc(USDC).balanceOf(SELLER), sellerBefore + 1_920_000);
        assertEq(CircleUsdc(USDC).balanceOf(address(this)), 0);
    }
}

interface CircleUsdc {
    function approve(address spender, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}
