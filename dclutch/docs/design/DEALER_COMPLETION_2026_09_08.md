# Dealer completion: capital, terminal claims, and authority composition

Status: implementation in progress, not execution evidence. Owner: DEALER-COMPLETION.
Scope: C-06 of `docs/MASTER_COMPLETION_CONTRACT.md`; the scoring mechanism
remains `MECHANISM_SCORING_DEALER_2026_09_04.md`. Accepted Found/Quote/Fill/partial
Withdraw evidence does not complete multi-LP ownership, General participation,
policy evolution, terminal redemption, capital exit, or retirement.

## One owner per fact

| Fact | Owner | Composition |
| --- | --- | --- |
| Scoring parameters and integer potential | `scoring_rule` and its Lean sources | Every fill and capital operation checks the same arithmetic. |
| Collateral capital, fund revision, rule commitment | `DealerFundV1` | Its one TradingPrincipal vault remains the capital destination. |
| Claims inventory and terminal payout | Claims protocol Position and terminal settlement | Position owner is the fund PDA; a wallet cannot sign for it. |
| Junior LP equity | Retained `dealer/equity.rs` exact scenario-residual kernel | Initial cash issuance, proportional later contributions and floor-rounded basket exits must compose with this same scoring fund. |
| General escrow and replay | General batch plus fund reservation | Admission must reserve present cash and inventory against concurrent RFQ fills and LP exits. |
| Native founder-bond proceeds | Native capital of the fund | Never included in collateral cash, hoard principal, or account rent. |
| Claims Position/admission rent | Their admission and Market RentCredit | Claims' existing Close moves recorded principal and donations to the authenticated credit. |
| Dealer record rent | Original funding/refund authority | Retirement must close every fund/rule/quote/LP/policy root and Custody vault/replay. |

## Terminal composition implemented by this slice

`DealerRedeem` has a Lean-owned 88-byte optimistic fund prefix, followed by
Claims' existing terminal-settlement request. Its nine-account parent frame is
followed by Claims' unchanged 36-account frame, or 39 with the founder-bond tail.
The parent is permissionless: the payer signs fees, while the program signs its
release-pinned Claims caller authority. It authenticates fund, rule, Market,
release, exact owner/context and the capital destination before invoking Claims.

Claims' semantic wire owner allocates formerly reserved byte 11 as the explicit
recipient mode. Zero preserves every existing External request byte and remains
the result of `TerminalSettlementRequestV3::new(input)`. Mode one is available
only to `CallerRole::Trading`: the recipient must be the canonical
TradingPrincipal vault under `(market, release, owner)`, and its token authority
must be the canonical Custody authority. The native founder-bond tail pays
`owner`, independently of that token authority. This is redemption of an owned
claim into its owner's capital, not a fee or funding appropriation from the hoard.

Product evaluation, exposure translation, burn amount, collateral rounding,
Custody movement and the child receipt stay with Claims. The Dealer parent joins
the exact child request and receipt, requires the vault increase to equal the
receipt payout, reads the resulting Claims inventory and increments fund cash
by that collateral payout only. The operator uses the same existing terminal
payout constructor and expected-poststate projector, with a Trading caller mode;
it does not copy the evaluator. Its executor checks exact poststate hashes of
the aggregate, Position, Claims replay, hoard and vault, plus native bond balances.

Market Terminal and Retiring both admit redemption. Retiring does not remove a
holder's exit right. Full terminal cash withdrawal already lifts the scoring
floor on chain and in the host; the earlier suspected host divergence was absent.

## Remaining convergence work

The current sponsor-only withdrawal cannot remain a parallel capital authority
once junior LP shares own the fund. Converge initial sponsor shares, later exact
basket contributions and share burns through the retained equity kernel, with
one share supply and open-LP count. For zero obligations and normalized scoring
inventory, prove and execute the kernel's zero split/merge projection; do not
introduce a merge path through the failure escrow.

General must admit and settle one schedule row using this same fund and Position.
A fund reservation must prevent another fill or exit spending its escrow. The
existing RFQ batch of two is not evidence of General participation.

Consent-safe parameter evolution needs new immutable rule versions and a delayed
fund transition with LP consent/exit protection. Overwriting a sealed rule's
bytes would violate its identity. Every rule version's funding and refund source
must remain explicit through retirement.

The single-fund policy transition follows the epoch consent rule in
`dealer-v2-scenario-collateral.md`: the sponsor of record may propose an
immutable successor rule and quiesce fills, but cannot appropriate LP principal.
Every remaining LP share, including the original sponsor's, must explicitly opt
into the exact proposal digest. New contributions stop while a proposal is
pending; proportional exits remain available. Activation requires the immutable
minimum delay, no reservation, unanimous remaining-share consent and capital
admissibility at the candidate rule. No absent LP is deemed to consent.

Cancellation or permissionless expiry clears the proposal and its consent
selection without moving capital or changing the active rule. A new proposal
has a new nonce/digest, so old consent cannot carry over. Exits during quiescence
can leave the old schedule insufficiently funded: cancellation therefore leaves
fills paused until a permissionless Resume verifies the old rule's commitment.
Exits remain available while paused. Partial capital migration into parallel
fund estates is unnecessary for this unanimous-consent model.

Native bond capital needs its own equity accounting, independent of recorded
rent and unsolicited lamport donations. Full capital exit must precede physical
close of the vault/replay, empty protocol Position/admission, LPs, quote, rule
versions and fund. All chain-facing paths require accepted current-source real
validator execution, exact refusal rollback controls and reproducible reports
before C-06 can close.

General and Dealer owners agree to one exclusive batch reservation per fund and
at most one Dealer schedule row per batch. The reservation binds the canonical
batch key, pre-reservation fund and Claims revisions, rule commitment and exact
worst-case cash. It locks the whole Position against concurrent fills, capital
operations and policy activation until exact-context settlement or cancellation.
