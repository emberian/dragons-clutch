# Protocol specification

## 1. State objects

The following names describe semantic objects, not yet byte layouts.

### Realm

Immutable collateral and implementation namespace:

- realm ID and protocol version;
- collateral mint, token program, decimals, permitted extension profile;
- realm authority policy and program-data identity;
- maximum outcomes and permitted payout denominator;
- accepted fee-policy and source-adapter versions.

The reference DREGG Realm is one configuration, not a kernel special case. It is
valid only after runtime inspection matches its frozen profile. Other Realms may
use other vetted collateral profiles. A Realm cannot change the economics of an
existing Market.

### Template and Instance

A Template content-addresses the source/window/statistic/partition compiler
program and human terms. It creates no liability. A Market Instance binds a
Template to an exact time window, Realm, capitalization, fee policy, outcome
mints, and lifecycle. A separately prepaid Series may permit permissionless
creation of bounded future Instances.

### Market

- canonical terms digest and version;
- Realm and market-local Hoard;
- outcome count and canonical outcome-mint PDAs;
- lifecycle state;
- collateral cap and issued complete-set accounting;
- FeedSpec or terminal adapter;
- observation window, repair deadline, resolution policy;
- fee policy and market-specific liveness commitments;
- immutable creation slot/time and program-version binding.

V1 should freeze `2 <= outcome_count <= 16` unless the transaction-size and
account-limit benchmark proves a different safe maximum.

### Position

Program-owned fixed-layout account keyed by Market and owner:

- owner and Market;
- fixed `[u64; MAX_OUTCOMES]` internal balances;
- Realm-collateral trading cash balance if the venue retains one;
- reserved order balances;
- generation and close-state fields.

The fixed array avoids allocation and makes all transition bounds explicit.

### Hoard

One market-local Token-2022 collateral account. It is not a realm-global vault.
This costs additional refundable rent but avoids cross-market write contention
and makes solvency local and auditable.

### SupplyLedger

A market-local fixed array holds conservative `internal_supply[i]` and
`accounted_external_supply[i]`. Issuance, bridge, merge, and redemption update it;
ordinary internal transfers and venue fills do not. This avoids scanning every
Position and keeps the shared writable account out of the trading hot path.

Only the canonical program can mint an Egg. A holder may nevertheless burn an
external Egg directly through Token-2022, making actual mint supply smaller than
`accounted_external_supply`. A permissionless donation-reconciliation instruction
may lower the accounted value to the authenticated canonical mint supply; it can
never increase it. This permits eventual close without treating a direct burn as
an invariant failure.

### FeedSpec, FeedHead, ArchivePage, WindowResult

`FeedSpec` freezes semantics and authenticated sources. `FeedHead` holds the
monotone cursor, current associative summary, coverage state, and booked work.
Archive pages retain compact summaries for the maximum horizon plus repair grace.
An immutable `WindowResult` is produced once for an exact feed/start/end/feature
tuple and may be reused by many Markets.

### EpochBook and ClearWork

An Epoch collects fixed-size single-Egg and bounded portfolio intents in densely
packed pages. After the order deadline, a candidate simplex price vector and fill
allocation is proposed. Permissionless paginated work scans every frozen page and
proves limits, conservation, complete-set conversion, fees, and score. During a
bounded proposal window a better valid candidate may replace it. The Epoch becomes
final only after all pages close consistently. Users then settle lazily into
Positions. Exact semantics are in [SIMPLEX_AUCTION.md](SIMPLEX_AUCTION.md).

## 2. Quantity model

All amounts are unsigned integer atoms. No floating-point value enters consensus.
Prices and payout weights use frozen integer scales. Intermediate products use
bounded `u128` only where the exact input bounds prove they fit.

For every outcome `i`:

```text
E_i = conservative accounted external supply for canonical outcome mint i
I_i = conservative accounted internal Position/order-escrow supply for outcome i
T_i = E_i + I_i
V   = Realm collateral atoms in the market Hoard
```

The program cannot scan all Positions during a transition. `E_i` and `I_i` are
therefore inductive SupplyLedger quantities: every instruction capable of
creating, moving, reserving, materializing, or destroying a balance is part of the
Eggcrate trusted transition surface. The Solana adapter must not expose writable
Position bytes to another program. Authenticated Token-2022 mint supply must be no
greater than `E_i`; direct burns may make it lower until donation reconciliation.

## 3. Solvency theorem

Let `P` be the finite set of payout vectors allowed by immutable Market terms.
Each vector contains nonnegative integer weights with a common denominator `D`.

```text
liability(v) = sum_i T_i * v_i / D
```

Subject to exact divisibility or explicit remainder-credit semantics, the required
invariant is:

```text
V >= max_{v in P} liability(v)
```

For ordinary categorical resolution, vectors are one-hot, so this reduces to:

```text
V >= max_i T_i
```

A complete split increases `V` and every `T_i` equally. Materialization,
dematerialization, reservation, cancellation, and trading preserve each `T_i`.
Merge and redemption reduce both liability and `V` according to the frozen rule.

Direct external token burns may reduce `E_i` without notifying the program. They
are donations: they decrease possible liability and cannot violate the inequality.

## 4. Core transitions

Every public kernel operation is total over bytes/values and returns an explicit
error. No unverified caller is expected to establish a proof-only precondition.

### Create market

1. Validate Realm and collateral profile.
2. Validate canonical terms and outcome count.
3. Derive Market, Hoard, outcome mints, and feed/window identities.
4. Freeze payout, rounding, observation, failure, and fee rules.
5. Book every mandatory liveness job and rent obligation.
6. Refuse activation until all protected balances cover their bookings.

Creation funds mint/account rent and a refundable anti-spam/cleanup bond; those
funds are not Hoard principal or ordinary protocol revenue.

### Split internal

```text
input: q > 0
effect: transfer q collateral atoms to Hoard
        add q to every active internal outcome balance
```

One collateral CPI and `O(n)` fixed local stores. It must preserve all bounds before
performing any external effect.

### Materialize

```text
input: outcome i, q <= internal_balance[i]
effect: internal_balance[i] -= q
        mint q units from canonical outcome mint i to destination
```

The adapter validates the mint PDA, token program, destination, and signer seeds.

### Dematerialize

```text
input: outcome i, q external units
effect: burn q canonical units
        internal_balance[i] += q
```

### Merge internal

Debit `q` from every active internal outcome and transfer `q` collateral from
Hoard. A mixed merge may first dematerialize external outcomes. A complete
external merge is supported but necessarily performs `n` burns.

### Resolve

1. Require observation and repair windows closed.
2. Authenticate exact sealed WindowResult or terminal-state record.
3. Apply the immutable evaluator.
4. Produce exactly one payout vector or deterministic refusal.
5. Freeze the Hatch forever.

Resolution cannot depend on resolver identity, current claim holdings, fee
revenue, or a mutable client interpretation.

### Redeem

Debit/burn the submitted outcome and pay exactly the frozen weight. Ordinary
one-hot settlement has unit redemption lots. A rational fallback requires either:

- a frozen redemption lot `D / gcd(weight_i, D)`; or
- explicit persistent remainder credits.

Silent floor rounding or routing dust to treasury is prohibited.

## 5. Feed semantics

An accepted observation binds:

- exact FeedSpec and adapter version;
- source account/program/deployment identity;
- target bucket and qualifying source timestamp/slot;
- conservative price interval, confidence, quorum, and dispersion;
- evidence digest and coverage state.

Feed updates are append-only by logical boundary. A transaction may fill several
consecutive boundaries only if each has uniquely qualifying authenticated
evidence. No later update rewrites an accepted valid checkpoint.

The accumulator exposes only a frozen monoidal feature family. Candidate V1
features include coverage/gaps, first/last, conservative price intervals,
price-time integrals, extrema, sampled crossing, squared-return sums, and drawdown
summaries. An arbitrary future predicate is not derivable from a summary and must
not be advertised as such.

## 6. Simplex-auction semantics

- Orders reserve internal Realm collateral or Egg balances before becoming
  visible.
- The Epoch freezes at a deterministic slot.
- A candidate price vector must be nonnegative and sum exactly to one collateral
  unit on the frozen scale.
- A candidate has no authority until every order page is scanned.
- The verifier checks single-Egg limits, atomic portfolio dot products, per-outcome
  and collateral conservation, complete-set split/merge, and exact public score.
- Better valid candidates may replace the current candidate during the proposal
  window; selection is the best submitted score, not an unproved global optimum.
- Marginal/proportional fills follow a frozen integer remainder rule.
- Page allocations remain inactive until the Epoch atomically becomes Final.
- Settlement is lazy and idempotent.
- Cancellation cannot race Epoch finalization.
- An invalid proposal loses a bounded bond but cannot occupy the cursor
  permanently. Valid losing proposals recover theirs.

An offchain solver improves search only. It supplies no trusted fact and is not a
required service. A static client can solve small batches.

## 7. Authority boundaries

V1 programs contain no instruction for:

- changing Market terms, fee schedule, payout set, or source version;
- withdrawing Hoard principal except through a valid merge/redemption;
- freezing or seizing an Egg;
- minting an Egg without debiting internal balance or adding a complete set;
- selecting a discretionary result;
- spending booked liveness reserves;
- transferring treasury funds into a claimant accounting identity;
- signing or submitting on behalf of a user.

The strongest deployment removes program upgrade authority after a disclosed beta
and audit process. Any retained upgrade authority is a visible deployment-level
trust assumption, never concealed by the source-level proofs.
