// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

/// @title LososEscrow
/// @notice Holds one USDC payment per deposit for as long as what it bought
/// runs. The edge then releases it to the treasury and the seller recorded
/// at payment time, refunds it to the payer, or splits it between the two
/// when the seller stopped delivering partway. The edge's key can do nothing
/// else: it cannot change where a deposit goes. If the edge never answers,
/// the payer takes the deposit back after its deadline.
interface IERC20 {
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

contract LososEscrow {
    enum State {
        None,
        Held,
        Released,
        Refunded,
        Split
    }

    struct Deposit {
        bytes32 orderId;
        address payer;
        address seller;
        uint256 gross;
        uint256 fee;
        uint64 deadline;
        State state;
    }

    IERC20 public immutable usdc;
    /// Receives the fee, and is the only account that can rotate the edge's
    /// key or rescue stray tokens. A hardware wallet or a multisig, never a
    /// key on a server.
    address public immutable treasury;
    /// The edge's key: releases and refunds deposits.
    address public releaser;

    /// The same ceiling as LososPay: half, the fixed split of a machine order.
    uint256 public constant MAX_FEE_BPS = 5000;
    /// A payer can take a deposit back only after its deadline, and the
    /// deadline has to leave the edge time to check the payment: at least an
    /// hour. At most 35 days: a month of storage or machines, plus the days
    /// the edge has to pay the seller once the month is over.
    uint64 public constant MIN_HOLD = 1 hours;
    uint64 public constant MAX_HOLD = 35 days;

    uint256 public depositCount;
    /// USDC this contract owes to open deposits. Anything above it is a
    /// stray transfer, which only `rescue` can move.
    uint256 public held;
    mapping(uint256 => Deposit) public deposits;

    event Held(
        uint256 indexed depositId,
        bytes32 indexed orderId,
        address indexed payer,
        address seller,
        uint256 gross,
        uint256 fee,
        uint64 deadline
    );
    event Released(
        uint256 indexed depositId, bytes32 indexed orderId, address indexed seller, uint256 gross, uint256 fee
    );
    event Refunded(uint256 indexed depositId, bytes32 indexed orderId, address indexed payer, uint256 gross);
    event Settled(
        uint256 indexed depositId, bytes32 indexed orderId, address indexed seller, uint256 refunded, uint256 fee
    );
    event ReleaserChanged(address indexed previous, address indexed next);
    event Rescued(address indexed token, address indexed to, uint256 amount);

    error ZeroAddress();
    error ZeroSeller();
    error ZeroAmount();
    error FeeTooHigh();
    error BadDeadline();
    error NotReleaser();
    error NotTreasury();
    error NotHeld();
    error TooEarly();
    error BadSplit();
    error OwedToDeposits();
    error TransferFailed();

    constructor(IERC20 usdc_, address treasury_, address releaser_) {
        if (address(usdc_) == address(0) || treasury_ == address(0) || releaser_ == address(0)) {
            revert ZeroAddress();
        }
        usdc = usdc_;
        treasury = treasury_;
        releaser = releaser_;
        emit ReleaserChanged(address(0), releaser_);
    }

    /// Pull `gross` from the caller and hold it. `orderId` is keccak256 of
    /// the edge's order id. Paying twice makes two deposits, and the edge
    /// refunds the second.
    function pay(bytes32 orderId, address seller, uint256 gross, uint256 fee, uint64 deadline)
        external
        returns (uint256 depositId)
    {
        if (seller == address(0)) revert ZeroSeller();
        if (gross == 0) revert ZeroAmount();
        if (fee > gross || fee * 10000 > gross * MAX_FEE_BPS) revert FeeTooHigh();
        if (deadline < block.timestamp + MIN_HOLD || deadline > block.timestamp + MAX_HOLD) {
            revert BadDeadline();
        }
        depositId = ++depositCount;
        deposits[depositId] = Deposit(orderId, msg.sender, seller, gross, fee, deadline, State.Held);
        held += gross;
        if (!usdc.transferFrom(msg.sender, address(this), gross)) revert TransferFailed();
        emit Held(depositId, orderId, msg.sender, seller, gross, fee, deadline);
    }

    /// The edge has checked the order and the seller's address: fee to the
    /// treasury, the rest to the seller recorded in `pay`.
    function release(uint256 depositId) external {
        if (msg.sender != releaser) revert NotReleaser();
        Deposit storage d = deposits[depositId];
        if (d.state != State.Held) revert NotHeld();
        d.state = State.Released;
        held -= d.gross;
        _send(treasury, d.fee);
        _send(d.seller, d.gross - d.fee);
        emit Released(depositId, d.orderId, d.seller, d.gross, d.fee);
    }

    /// Back to the payer, whole. The edge may refund at any time (the seller's
    /// address failed its check, the order was paid twice, nothing could be
    /// delivered). The payer may refund only after the deadline.
    function refund(uint256 depositId) external {
        Deposit storage d = deposits[depositId];
        if (d.state != State.Held) revert NotHeld();
        if (msg.sender != releaser) {
            if (msg.sender != d.payer) revert NotReleaser();
            if (block.timestamp < d.deadline) revert TooEarly();
        }
        d.state = State.Refunded;
        held -= d.gross;
        _send(d.payer, d.gross);
        emit Refunded(depositId, d.orderId, d.payer, d.gross);
    }

    /// Part back to the payer, the rest released: the seller's box went
    /// offline partway through what was bought, so the payer gets back the
    /// share that was not delivered and the seller keeps the share that was.
    /// The fee shrinks with the seller's share, rounded down. For all or
    /// nothing, the edge calls `refund` or `release`.
    function settle(uint256 depositId, uint256 refundAmount) external {
        if (msg.sender != releaser) revert NotReleaser();
        Deposit storage d = deposits[depositId];
        if (d.state != State.Held) revert NotHeld();
        if (refundAmount == 0 || refundAmount >= d.gross) revert BadSplit();
        d.state = State.Split;
        held -= d.gross;
        uint256 kept = d.gross - refundAmount;
        uint256 fee = d.fee * kept / d.gross;
        _send(d.payer, refundAmount);
        _send(treasury, fee);
        _send(d.seller, kept - fee);
        emit Settled(depositId, d.orderId, d.seller, refundAmount, fee);
    }

    /// A new edge key, for a rotation or after a compromise.
    function setReleaser(address next) external {
        if (msg.sender != treasury) revert NotTreasury();
        if (next == address(0)) revert ZeroAddress();
        emit ReleaserChanged(releaser, next);
        releaser = next;
    }

    /// Move tokens someone sent here by plain transfer instead of `pay`. For
    /// USDC only the balance above what open deposits are owed can move.
    function rescue(IERC20 token, address to, uint256 amount) external {
        if (msg.sender != treasury) revert NotTreasury();
        if (to == address(0)) revert ZeroAddress();
        if (address(token) == address(usdc) && usdc.balanceOf(address(this)) - amount < held) {
            revert OwedToDeposits();
        }
        if (!token.transfer(to, amount)) revert TransferFailed();
        emit Rescued(address(token), to, amount);
    }

    function _send(address to, uint256 amount) internal {
        if (amount == 0) return;
        if (!usdc.transfer(to, amount)) revert TransferFailed();
    }
}
