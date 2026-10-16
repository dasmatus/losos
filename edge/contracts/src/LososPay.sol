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
    /// The largest cut the market takes: half, which is the fixed split of a
    /// machine order with the box that hosts it. Storage and compute orders
    /// carry the operator's rate, capped at 20% by the registrar. The
    /// registrar checks the exact fee of every order against the `Paid` log,
    /// so this ceiling only bounds what a wrong invoice could take.
    uint256 public constant MAX_FEE_BPS = 5000;

    event Paid(
        bytes32 indexed orderId,
        address indexed payer,
        address indexed seller,
        uint256 gross,
        uint256 fee
    );

    error ZeroAddress();
    error ZeroSeller();
    error FeeTooHigh();
    error TransferFailed();

    constructor(IERC20 usdc_, address treasury_) {
        if (address(usdc_) == address(0) || treasury_ == address(0)) revert ZeroAddress();
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
