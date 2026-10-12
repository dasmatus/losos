// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

/// @title LososPay
/// @notice Splits one USDC payment between the platform treasury and a
/// seller. The edge never holds a key: the buyer's wallet calls `pay`, and
/// the registrar later checks the `Paid` log against the order it recorded.
interface IERC20 {
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

contract LososPay {
    IERC20 public immutable usdc;
    address public immutable treasury;
    /// Same ceiling as the Stripe gate: 20%.
    uint256 public constant MAX_FEE_BPS = 2000;

    event Paid(
        bytes32 indexed orderId,
        address indexed payer,
        address indexed seller,
        uint256 gross,
        uint256 fee
    );

    error ZeroSeller();
    error FeeTooHigh();
    error TransferFailed();

    constructor(IERC20 usdc_, address treasury_) {
        usdc = usdc_;
        treasury = treasury_;
    }

    /// Pull `fee` to the treasury and the rest to `seller`. `orderId` is
    /// keccak256 of the edge's order id (`ord_...`), which is what the
    /// registrar looks for in the log.
    function pay(bytes32 orderId, address seller, uint256 gross, uint256 fee) external {
        if (seller == address(0)) revert ZeroSeller();
        if (fee > gross || fee * 10000 > gross * MAX_FEE_BPS) revert FeeTooHigh();
        _pull(treasury, fee);
        _pull(seller, gross - fee);
        emit Paid(orderId, msg.sender, seller, gross, fee);
    }

    function _pull(address to, uint256 amount) internal {
        if (amount == 0) return;
        if (!usdc.transferFrom(msg.sender, to, amount)) revert TransferFailed();
    }
}
