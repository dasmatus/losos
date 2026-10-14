// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {IERC20, LososEscrow} from "../src/LososEscrow.sol";

/// Holds, releases and refunds Circle's Base Sepolia USDC through
/// `LososEscrow` on a fork of the public chain. Skipped unless
/// `BASE_SEPOLIA_RPC` is set, as in `LososPayFork.t.sol`.
contract LososEscrowForkTest is Test {
    address internal constant USDC = 0x036CbD53842c5426634e7929541eC2318f3dCF7e;
    address internal constant TREASURY = address(0x1111111111111111111111111111111111111111);
    address internal constant SELLER = address(0x2222222222222222222222222222222222222222);
    address internal constant EDGE = address(0x3333333333333333333333333333333333333333);

    function test_fork_hold_release_and_refund_circle_usdc_on_base_sepolia() public {
        string memory rpc = vm.envOr("BASE_SEPOLIA_RPC", string(""));
        if (bytes(rpc).length == 0) {
            vm.skip(true);
            return;
        }
        vm.createSelectFork(rpc);
        IERC20 usdc = IERC20(USDC);
        LososEscrow escrow = new LososEscrow(usdc, TREASURY, EDGE);
        // Circle's token is a proxy. forge-std finds the balance slot.
        deal(USDC, address(this), 3_000_000);
        assertTrue(CircleUsdc(USDC).approve(address(escrow), 3_000_000));
        uint256 treasuryBefore = usdc.balanceOf(TREASURY);
        uint256 sellerBefore = usdc.balanceOf(SELLER);
        uint64 deadline = uint64(block.timestamp + 1 days);

        uint256 kept = escrow.pay(keccak256("ord_fork"), SELLER, 2_000_000, 80_000, deadline);
        uint256 returned = escrow.pay(keccak256("ord_fork"), SELLER, 1_000_000, 40_000, deadline);
        assertEq(usdc.balanceOf(address(escrow)), 3_000_000);

        vm.startPrank(EDGE);
        escrow.release(kept);
        escrow.refund(returned);
        vm.stopPrank();
        // These addresses may already hold test USDC. The payment is the delta.
        assertEq(usdc.balanceOf(TREASURY), treasuryBefore + 80_000);
        assertEq(usdc.balanceOf(SELLER), sellerBefore + 1_920_000);
        assertEq(usdc.balanceOf(address(this)), 1_000_000);
        assertEq(usdc.balanceOf(address(escrow)), 0);
    }
}

interface CircleUsdc {
    function approve(address spender, uint256 amount) external returns (bool);
}
