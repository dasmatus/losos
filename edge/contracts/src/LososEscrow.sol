// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.24;

/// @title LososEscrow
/// @notice Holds one USDC payment per deposit for as long as what it bought
/// runs. It is then released to the treasury and the seller recorded at
/// payment time, refunded to the payer, or split between the two when the
/// seller stopped delivering partway. Each of those needs two of three keys:
/// the edge's, the buyer's box's and the seller's box's, all named at
/// payment. One of them sends the transaction and the others sign the
/// decision (EIP-712), so the edge alone can neither pay a deposit out nor
/// send it back, and the two boxes can settle without the edge. Nobody can
/// change where a deposit goes. If no two agree, the payer takes the
/// deposit back after its deadline.
///
/// It also holds prepaid balances for pay-as-you-go. A buyer tops up a USDC
/// balance and opens taps, one per seller, each naming the most the edge may
/// bill per hour and the fee. The edge meters what was used and draws it
/// from the balance through the tap, never more than the tap's rate allows
/// for the time since it last did, and only to the seller and treasury the
/// tap names. The buyer takes the rest back after a notice, without the edge.
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

    /// A buyer's authorisation for the edge to bill one seller, at most
    /// `rate` USDC base units per hour, with `feeBps` of each slice going to
    /// the treasury. `settledAt` is the end of the time already billed,
    /// which starts `HEADSTART` before the tap opened; `closedAt` is zero
    /// while the tap is open.
    struct Tap {
        address buyer;
        address seller;
        uint128 rate;
        uint16 feeBps;
        uint64 settledAt;
        uint64 closedAt;
    }

    /// A buyer's request to take their balance back, payable from
    /// `notBefore` on.
    struct Withdrawal {
        uint128 amount;
        uint64 notBefore;
    }

    /// `buyerBox` and `sellerBox` are the two boxes' escrow keys: with the
    /// edge's, the three keys any two of which decide the deposit.
    struct Deposit {
        bytes32 orderId;
        address payer;
        address seller;
        uint256 gross;
        uint256 fee;
        uint64 deadline;
        State state;
        address buyerBox;
        address sellerBox;
    }

    IERC20 public immutable usdc;
    /// Receives the fee, and is the only account that can rotate the edge's
    /// key or rescue stray tokens. A hardware wallet or a multisig, never a
    /// key on a server.
    address public immutable treasury;
    /// The edge's key: one of the three that decide a deposit, and the only
    /// one that bills taps.
    address public releaser;

    /// The same ceiling as LososPay: half, the fixed split of a machine order.
    uint256 public constant MAX_FEE_BPS = 5000;
    /// A payer can take a deposit back only after its deadline, and the
    /// deadline has to leave the edge time to check the payment: at least an
    /// hour. At most 35 days: a month of storage or machines, plus the days
    /// the edge has to pay the seller once the month is over.
    uint64 public constant MIN_HOLD = 1 hours;
    uint64 public constant MAX_HOLD = 35 days;
    /// A withdrawal waits this long after it is asked for, so the edge can
    /// bill the last slice of every tap first.
    uint64 public constant NOTICE = 48 hours;
    /// The most time one `settleTap` can bill for, however long the edge
    /// waited. A stolen edge key can take at most a week at each tap's rate
    /// in one settle.
    uint64 public constant MAX_ACCRUAL = 7 days;
    /// A new tap starts one hour into its accrual, so the edge can bill the
    /// first hour of use as soon as the tap is open. In all, a tap can be
    /// billed at most its rate times the time since it opened plus this hour.
    uint64 public constant HEADSTART = 1 hours;
    /// Taps one `openTaps` call may open, so a group of listings fits in one
    /// transaction and the loop stays bounded.
    uint256 public constant MAX_TAPS_PER_CALL = 16;
    /// What the keys sign: release is `refund` 0, a whole refund is
    /// `refund` equal to the deposit's gross, anything between is a split.
    bytes32 public constant DECISION_TYPEHASH = keccak256("Decision(uint256 depositId,uint256 refund)");
    /// EIP-712 domain: "LososEscrow", version "2", this chain, this contract.
    bytes32 public immutable DOMAIN_SEPARATOR;

    uint256 public depositCount;
    /// USDC this contract owes to open deposits and to balances. Anything
    /// above it is a stray transfer, which only `rescue` can move.
    uint256 public held;
    mapping(uint256 => Deposit) public deposits;
    /// Each buyer's prepaid USDC, drawn down by `settleTap`.
    mapping(address => uint256) public balances;
    /// Each buyer's pending withdrawal, at most one.
    mapping(address => Withdrawal) public withdrawals;
    /// Taps by key, `tapKey(buyer, id)`: the same id opened by two wallets is
    /// two taps, so nobody can take a buyer's quoted ids first. A tap is
    /// never deleted, so a key cannot be opened twice.
    mapping(bytes32 => Tap) public taps;

    event Held(
        uint256 indexed depositId,
        bytes32 indexed orderId,
        address indexed payer,
        address seller,
        uint256 gross,
        uint256 fee,
        uint64 deadline,
        address buyerBox,
        address sellerBox
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
    event ToppedUp(address indexed buyer, uint256 amount, uint256 balance);
    event TapOpened(bytes32 indexed tapKey, address indexed buyer, address indexed seller, uint256 rate, uint16 feeBps);
    event TapClosed(bytes32 indexed tapKey, address indexed buyer, uint64 closedAt);
    event TapSettled(bytes32 indexed tapKey, address indexed seller, uint256 amount, uint256 fee, uint64 settledAt);
    event WithdrawalAsked(address indexed buyer, uint256 amount, uint64 notBefore);
    event Withdrawn(address indexed buyer, uint256 amount);

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
    error TapExists();
    error NoTap();
    error TapIsClosed();
    error OverCap();
    error OverBalance();
    error NoWithdrawal();
    error TooManyTaps();
    error BadTaps();
    error BadBoxes();
    error BadSignature();
    error NotApproved();

    constructor(IERC20 usdc_, address treasury_, address releaser_) {
        if (address(usdc_) == address(0) || treasury_ == address(0) || releaser_ == address(0)) {
            revert ZeroAddress();
        }
        usdc = usdc_;
        treasury = treasury_;
        releaser = releaser_;
        DOMAIN_SEPARATOR = keccak256(
            abi.encode(
                keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
                keccak256("LososEscrow"),
                keccak256("2"),
                block.chainid,
                address(this)
            )
        );
        emit ReleaserChanged(address(0), releaser_);
    }

    /// Pull `gross` from the caller and hold it. `orderId` is keccak256 of
    /// the edge's order id. `buyerBox` and `sellerBox` are the escrow keys of
    /// the buyer's and the seller's boxes: two different keys, neither of
    /// them the edge's. The edge counts a payment only when `sellerBox` is
    /// the key the seller's box gave it. Paying twice makes two deposits,
    /// and the second goes back.
    function pay(
        bytes32 orderId,
        address seller,
        uint256 gross,
        uint256 fee,
        uint64 deadline,
        address buyerBox,
        address sellerBox
    ) external returns (uint256 depositId) {
        if (seller == address(0)) revert ZeroSeller();
        if (gross == 0) revert ZeroAmount();
        if (fee > gross || fee * 10000 > gross * MAX_FEE_BPS) revert FeeTooHigh();
        if (deadline < block.timestamp + MIN_HOLD || deadline > block.timestamp + MAX_HOLD) {
            revert BadDeadline();
        }
        if (
            buyerBox == address(0) || sellerBox == address(0) || buyerBox == sellerBox || buyerBox == releaser
                || sellerBox == releaser
        ) revert BadBoxes();
        depositId = ++depositCount;
        deposits[depositId] =
            Deposit(orderId, msg.sender, seller, gross, fee, deadline, State.Held, buyerBox, sellerBox);
        held += gross;
        if (!usdc.transferFrom(msg.sender, address(this), gross)) revert TransferFailed();
        emit Held(depositId, orderId, msg.sender, seller, gross, fee, deadline, buyerBox, sellerBox);
    }

    /// The digest a key signs to agree that deposit `depositId` sends
    /// `refund` back to the payer and releases the rest.
    function decisionDigest(uint256 depositId, uint256 refund_) public view returns (bytes32) {
        return keccak256(
            abi.encodePacked("\x19\x01", DOMAIN_SEPARATOR, keccak256(abi.encode(DECISION_TYPEHASH, depositId, refund_)))
        );
    }

    /// Fee to the treasury, the rest to the seller recorded in `pay`. Needs
    /// two of the three keys: the sender, if it is one, and `sigs`.
    function release(uint256 depositId, bytes[] calldata sigs) external {
        Deposit storage d = deposits[depositId];
        if (d.state != State.Held) revert NotHeld();
        _approved(depositId, d, 0, sigs);
        d.state = State.Released;
        held -= d.gross;
        _send(treasury, d.fee);
        _send(d.seller, d.gross - d.fee);
        emit Released(depositId, d.orderId, d.seller, d.gross, d.fee);
    }

    /// Back to the payer, whole, at any time, when two of the three keys
    /// agree (the seller's address failed its check, the order was paid
    /// twice, nothing could be delivered).
    function refund(uint256 depositId, bytes[] calldata sigs) external {
        Deposit storage d = deposits[depositId];
        if (d.state != State.Held) revert NotHeld();
        _approved(depositId, d, d.gross, sigs);
        _refund(depositId, d);
    }

    /// Back to the payer, whole, by the payer alone, after the deadline:
    /// what happens when no two keys agreed in time.
    function reclaim(uint256 depositId) external {
        Deposit storage d = deposits[depositId];
        if (d.state != State.Held) revert NotHeld();
        if (msg.sender != d.payer) revert NotApproved();
        if (block.timestamp < d.deadline) revert TooEarly();
        _refund(depositId, d);
    }

    /// Part back to the payer, the rest released: the seller's box went
    /// offline partway through what was bought, so the payer gets back the
    /// share that was not delivered and the seller keeps the share that was.
    /// The fee shrinks with the seller's share, rounded down. Two of the
    /// three keys sign this exact `refundAmount`.
    function settle(uint256 depositId, uint256 refundAmount, bytes[] calldata sigs) external {
        Deposit storage d = deposits[depositId];
        if (d.state != State.Held) revert NotHeld();
        if (refundAmount == 0 || refundAmount >= d.gross) revert BadSplit();
        _approved(depositId, d, refundAmount, sigs);
        d.state = State.Split;
        held -= d.gross;
        uint256 kept = d.gross - refundAmount;
        uint256 fee = d.fee * kept / d.gross;
        _send(d.payer, refundAmount);
        _send(treasury, fee);
        _send(d.seller, kept - fee);
        emit Settled(depositId, d.orderId, d.seller, refundAmount, fee);
    }

    function _refund(uint256 depositId, Deposit storage d) internal {
        d.state = State.Refunded;
        held -= d.gross;
        _send(d.payer, d.gross);
        emit Refunded(depositId, d.orderId, d.payer, d.gross);
    }

    /// Which of the deposit's three keys `who` is, as a bit: 1 the edge, 2
    /// the buyer's box, 4 the seller's box, 0 none.
    function _party(Deposit storage d, address who) internal view returns (uint8) {
        if (who == releaser) return 1;
        if (who == d.buyerBox) return 2;
        if (who == d.sellerBox) return 4;
        return 0;
    }

    /// Revert unless the sender and `sigs` together are at least two
    /// different keys of the deposit's three, each agreeing on `refund_`.
    /// A signature from any other key reverts rather than being skipped.
    function _approved(uint256 depositId, Deposit storage d, uint256 refund_, bytes[] calldata sigs) internal view {
        if (sigs.length > 2) revert BadSignature();
        uint8 seen = _party(d, msg.sender);
        bytes32 digest = decisionDigest(depositId, refund_);
        for (uint256 i = 0; i < sigs.length; i++) {
            uint8 p = _party(d, _recover(digest, sigs[i]));
            if (p == 0) revert BadSignature();
            seen |= p;
        }
        if (seen != 3 && seen != 5 && seen != 6 && seen != 7) revert NotApproved();
    }

    /// The signer of `digest`, from a 65-byte `r ‖ s ‖ v`. High `s` and
    /// anything but `v` 27 or 28 are refused, so a signature has one form.
    function _recover(bytes32 digest, bytes calldata sig) internal pure returns (address who) {
        if (sig.length != 65) revert BadSignature();
        bytes32 r = bytes32(sig[0:32]);
        bytes32 s = bytes32(sig[32:64]);
        uint8 v = uint8(sig[64]);
        if (uint256(s) > 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0) revert BadSignature();
        if (v != 27 && v != 28) revert BadSignature();
        who = ecrecover(digest, v, r, s);
        if (who == address(0)) revert BadSignature();
    }

    /// A new edge key, for a rotation or after a compromise.
    function setReleaser(address next) external {
        if (msg.sender != treasury) revert NotTreasury();
        if (next == address(0)) revert ZeroAddress();
        emit ReleaserChanged(releaser, next);
        releaser = next;
    }

    /// Move tokens someone sent here by plain transfer instead of `pay`. For
    /// USDC only the balance above what open deposits and balances are owed
    /// can move.
    function rescue(IERC20 token, address to, uint256 amount) external {
        if (msg.sender != treasury) revert NotTreasury();
        if (to == address(0)) revert ZeroAddress();
        if (address(token) == address(usdc) && usdc.balanceOf(address(this)) - amount < held) {
            revert OwedToDeposits();
        }
        if (!token.transfer(to, amount)) revert TransferFailed();
        emit Rescued(address(token), to, amount);
    }

    /// Pull `amount` from the caller onto their balance.
    function topUp(uint256 amount) external {
        if (amount == 0) revert ZeroAmount();
        uint256 balance = balances[msg.sender] + amount;
        balances[msg.sender] = balance;
        held += amount;
        if (!usdc.transferFrom(msg.sender, address(this), amount)) revert TransferFailed();
        emit ToppedUp(msg.sender, amount, balance);
    }

    /// The key `taps` keeps `buyer`'s tap `id` under.
    function tapKey(address buyer, bytes32 id) public pure returns (bytes32) {
        return keccak256(abi.encode(buyer, id));
    }

    /// Open one tap per seller, all with the same fee, billed from the
    /// caller's balance. `ids[i]` is keccak256 of the edge's tap id, kept
    /// under `tapKey(msg.sender, ids[i])`, and
    /// `rates[i]` the most the edge may bill `sellers[i]` per hour. Billing
    /// counts from `HEADSTART` before now, so the first hour can be billed at
    /// once.
    function openTaps(bytes32[] calldata ids, address[] calldata sellers, uint128[] calldata rates, uint16 feeBps)
        external
    {
        uint256 n = ids.length;
        if (n == 0 || sellers.length != n || rates.length != n) revert BadTaps();
        if (n > MAX_TAPS_PER_CALL) revert TooManyTaps();
        if (feeBps > MAX_FEE_BPS) revert FeeTooHigh();
        uint64 start = uint64(block.timestamp) - HEADSTART;
        for (uint256 i = 0; i < n; i++) {
            if (sellers[i] == address(0)) revert ZeroSeller();
            bytes32 key = tapKey(msg.sender, ids[i]);
            if (taps[key].buyer != address(0)) revert TapExists();
            taps[key] = Tap(msg.sender, sellers[i], rates[i], feeBps, start, 0);
            emit TapOpened(key, msg.sender, sellers[i], rates[i], feeBps);
        }
    }

    /// Stop a tap. The buyer or the edge may close it, once. The edge can
    /// still bill it afterwards, but only for the time before it closed.
    function closeTap(bytes32 key) external {
        Tap storage t = taps[key];
        if (t.buyer == address(0)) revert NoTap();
        if (msg.sender != t.buyer && msg.sender != releaser) revert NotReleaser();
        if (t.closedAt != 0) revert TapIsClosed();
        uint64 closedAt = uint64(block.timestamp);
        t.closedAt = closedAt;
        emit TapClosed(key, t.buyer, closedAt);
    }

    /// The edge bills `amount` of metered use through a tap: the fee to the
    /// treasury, the rest to the tap's seller, both out of the buyer's
    /// balance. `amount` can be at most the tap's rate for the time since
    /// the last settle (or since `HEADSTART` before it opened), up to its
    /// close and at most `MAX_ACCRUAL`. Billing then restarts from that
    /// point, so cap left unused is not carried over. Over its whole life a
    /// tap pays out at most its rate times (time open + 1 hour).
    function settleTap(bytes32 key, uint256 amount) external {
        if (msg.sender != releaser) revert NotReleaser();
        Tap storage t = taps[key];
        if (t.buyer == address(0)) revert NoTap();
        if (amount == 0) revert ZeroAmount();
        uint64 end = t.closedAt != 0 ? t.closedAt : uint64(block.timestamp);
        if (end <= t.settledAt) {
            if (t.closedAt != 0) revert TapIsClosed();
            revert TooEarly();
        }
        uint256 elapsed = end - t.settledAt;
        if (elapsed > MAX_ACCRUAL) elapsed = MAX_ACCRUAL;
        if (amount > uint256(t.rate) * elapsed / 1 hours) revert OverCap();
        uint256 balance = balances[t.buyer];
        if (amount > balance) revert OverBalance();
        t.settledAt = end;
        balances[t.buyer] = balance - amount;
        held -= amount;
        uint256 fee = amount * t.feeBps / 10000;
        _send(treasury, fee);
        _send(t.seller, amount - fee);
        emit TapSettled(key, t.seller, amount, fee, end);
    }

    /// Ask to take `amount` of the caller's balance back after `NOTICE`.
    /// A new ask replaces the last one and starts the notice again. More
    /// than the balance is allowed: `withdraw` pays what is left.
    function askWithdrawal(uint256 amount) external {
        if (amount == 0) revert ZeroAmount();
        uint128 asked = amount > type(uint128).max ? type(uint128).max : uint128(amount);
        uint64 notBefore = uint64(block.timestamp) + NOTICE;
        withdrawals[msg.sender] = Withdrawal(asked, notBefore);
        emit WithdrawalAsked(msg.sender, asked, notBefore);
    }

    /// After the notice, pay the caller what they asked for, or their whole
    /// balance if the edge billed some of it in the meantime. The edge is
    /// not needed for this.
    function withdraw() external {
        Withdrawal memory w = withdrawals[msg.sender];
        if (w.notBefore == 0) revert NoWithdrawal();
        if (block.timestamp < w.notBefore) revert TooEarly();
        delete withdrawals[msg.sender];
        uint256 balance = balances[msg.sender];
        uint256 amount = w.amount < balance ? w.amount : balance;
        balances[msg.sender] = balance - amount;
        held -= amount;
        _send(msg.sender, amount);
        emit Withdrawn(msg.sender, amount);
    }

    function _send(address to, uint256 amount) internal {
        if (amount == 0) return;
        if (!usdc.transfer(to, amount)) revert TransferFailed();
    }
}
