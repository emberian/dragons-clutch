# Joint clearing: every outcome of one market, one batch, one price vector

JOINT-CLEARING design lane, 2026-09-04, at `5360dff57` (`git rev-parse
--show-toplevel` = `/Users/ember/dev/dclutch`). **Design only.** No program
source moves under cohort-16; the deliverables are this note, the Lean module
`formal/dclutch-semantics/DClutchSemantics/JointClearingV1.lean` (44 theorems,
28 executable witnesses, zero `sorry`, `lake build` green at v4.30.0), and a
compute price. Every path:line below is HEAD.

## 0. What the tree already is, and what this note adds

The General family is already a joint-clearing batch with complete-set
materialisation inside the clearing. `GeneralClearing.lean:262-275` admits
exactly one aggregate liability change per batch — none, mint `q` complete
sets, or merge `q` — and `Materialize` is the action that performs it against
the Hoard (`crates/dclutch-general-adapter-contract/src/escrow_v1.rs:307-308`,
`MintCompleteSet`/`MergeCompleteSet`). Orders are portfolio orders
(`receivePerLot`, `deliverPerLot`, `GeneralClearing.lean:150-158`), prices are
an exact simplex (`PriceVector.validFor`, `:132-135`;
`RuntimeWidthErrorV2::InvalidSimplex`, `runtime_width.rs:187-188, 314-315`),
and the streamed verifier accumulates claim flow per outcome and requires a
uniform complete-set delta (`RuntimeVerifyErrorV2::ClaimImbalance`,
`runtime_verify.rs:201-202`). Nothing in this note replaces that machine.

What the tree does NOT have, and what this note supplies:

1. **A clearing rule.** The kernel verifies "best valid submitted candidate"
   (`MASTER_COMPLETION_CONTRACT.md:90`, C-05) and never says which clearing is
   right. Two valid candidates for one book can carry different price vectors
   — `Examples.jointMint` and `Examples.jointMintSkewed` in the Lean are the
   same two orders at `[50, 50]` and `[60, 40]`, both valid today — and the
   selection policy breaks the tie by filled lots, then rounding surplus, then
   candidate id (`GeneralClearing.lean:320-360`). A solver who is also a trader
   picks the price. §1 states the rule as a linear program and §2 proves that
   its KKT conditions are a *checked optimality certificate*, so a certified
   candidate may be called an optimal clearing under `AGENTS.md`'s vocabulary
   rule.
2. **The seller's floor.** `AuthenticatedOrderTermsV2` carries
   `max_quote_debit_per_lot` and nothing else (`runtime_verify.rs:220-231`);
   the verifier's only limit conjunct is `debit > debit_limit`
   (`:1242-1245`). A net seller — `deliverPerLot` nonzero, `receivePerLot`
   zero — has a derived *credit* and no limit on it. A candidate may pay a
   seller any price down to zero and the verifier admits it
   (`Examples.belowFloor`). Joint clearing needs both halves of the limit.
3. **The unfilled row.** The certificate's dual condition — no order is
   rationed strictly inside its limit — is a statement about orders the
   candidate did *not* fill, so the candidate must account for every live
   order. Today a candidate enumerates only what it fills (decision 0009 §1:
   "a batch is a window, not a ledger"); a solver may omit an inconvenient
   order. §1.4 closes it with a count the batch record already holds.

## 1. The clearing rule

### 1.1 Data

A Market with `K` outcomes, a price scale `s` (the batch's `PRICE_SCALE`,
`collection_v1.rs:114`), and a book of `N` live orders. An order `o` is

    a_o ∈ ℤ^K  = receivePerLot − deliverPerLot   (signed claim flow per lot)
    q_o ∈ ℕ    = maxLots
    ℓ_o ∈ ℤ    = maximum net quote debit per lot, in scale units. SIGNED.

A buy of outcome `i` at price ≤ `p` is `a_o = e_i, ℓ_o = p`. A sell of
outcome `i` at price ≥ `p` is `a_o = −e_i, ℓ_o = −p`. A bundle buy — "the
result lands in outcomes 3..5" — is `a_o = e_3 + e_4 + e_5`. In the Lean:
`Order`, `Order.flow`, `JointClearingV1.lean:147-153, 177-178`.

A clearing is `(p, f, M)`: prices `p ∈ ℕ^K` with `Σ p_i = s`; fills
`f_o ∈ [0, q_o]` for every live order; and `M ∈ ℤ`, the signed number of
complete sets the batch materialises (`M > 0` mints, `M < 0` merges). Per-lot
debit of `o` at `p` is `a_o·p`; its net flow into the book at outcome `i` is
`net_i = Σ_o a_{o,i} f_o`.

### 1.2 The program and its certificate

The batch solves

    maximise   Σ_o ℓ_o f_o − s·M
    subject to net_i ≤ M           for every outcome i        (cover)
               0 ≤ f_o ≤ q_o       for every order o

This is the parimutuel call-auction LP (Lange–Economides; Peters–So–Ye's
CPCAM) with `M` free in sign so that merges are the same program as mints.
Its dual has one variable per outcome — the price — and the dual constraint
of `M` is exactly `Σ p_i = s`. The KKT conditions are the verifier
(`Clearing.valid`, `JointClearingV1.lean:210-226`):

| conjunct | statement | what it refuses |
|---|---|---|
| simplex | `Σ p_i = s`, `p ≥ 0`, `|p| = K` | a price vector off the simplex |
| bounded | `f_o ≤ q_o` | a fill past the signed quantity |
| at-or-better | `f_o > 0 ⇒ a_o·p ≤ ℓ_o` | a fill worse than its limit — buy above, sell below |
| marginal | `f_o < q_o ⇒ ℓ_o ≤ a_o·p` | an order rationed strictly inside its limit |
| cover | `net_i ≤ M` | a fill the sets cannot supply |
| slackness | `p_i > 0 ⇒ net_i = M` | **an unbacked mint**: sets claimed on an outcome nobody funded |
| distinct | order ids pairwise distinct | one order filled twice |
| complete | rows = live orders of the batch | an omitted order (§1.4) |

These are `O(N·K)` integer comparisons — one dot product per row and one
comparison per outcome — which is what the accelerator's bounded compute can
verify. The clearing *algorithm* is not on chain: solvers compute `(p, f, M)`
off chain (for a single-outcome book, the greedy in §1.6; in general, the LP)
and submit it; the chain checks the certificate. This is the same posture as
the shipping General — accelerator as second opinion on the *evaluation*
(`DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md:836`) — with the difference
that a passing certificate now *means* something: `certificate_is_optimal`
(`JointClearingV1.lean:590-598`) proves that no feasible allocation of the
same book scores higher. "Best valid submitted" becomes "optimal, and here is
the proof the chain checked", which is the sentence `AGENTS.md` withholds
until a checked certificate exists.

### 1.3 The residual, and where it goes

Complementary slackness allows `net_i < M` only where `p_i = 0`. On such an
outcome the batch mints `M` claims and hands out `net_i`; the difference is a
**residual** the batch holds (`Clearing.residual`), and
`residual_worth_nothing` (`:380-386`) proves its value at the batch's own
prices is zero, coordinate by coordinate. It arises exactly when the market
prices an outcome at zero — `Examples.zeroPriced`: buyers of outcomes 0 and 1
together value a set at 1.20, ten sets mint, eight claims of outcome 2 are
left over. Refusing residuals (the shipping `claimsBalance`, which requires
`outputs_i = inputs_i + q` on *every* outcome) would cap the mint at the thin
outcome's demand and ration the deep side strictly inside its limits —
`Examples.thinRationed` is worth 160 against 320 and is refused by the
`marginal` conjunct at every simplex price. The LP is right and the residual
is the price of it.

Three dispositions were priced:

- **Beneficiary row (recommended).** The residual is distributed to the
  configured `quote_surplus_beneficiary`
  (`crates/dclutch-general-config-contract/src/lib.rs:101, 365, 407`) as one
  more `Distribute` row, exactly as the rounding remainder is routed at
  `Close`. Every minted claim is then allocated, the shipping `claimsBalance`
  holds unchanged, no kernel operation is added, and the beneficiary's
  position is the claim-shaped twin of a remainder it already receives. Any
  participant outbids it for free: a zero-limit buy on the outcome takes the
  residual pro-rata before the beneficiary does, and the residual exists only
  because nobody bid above zero.
- **Strand.** Burn the residual claims without releasing collateral, leaving
  `supply_i < M` with `hoard = M = max_i supply_i`. L4 holds with zero excess,
  but it is a new kernel command (a partial burn) and the first pre-terminal
  non-uniform supply vector in the tree; every family's complete-set law
  (`EconomicKernel.lean:67-70`) would need the exception. Refused on cost.
- **Refuse residuals.** The shipping rule. Loses the LP, keeps the price
  indeterminacy, and rations orders strictly inside their limits.

**Ruling owed (ember): beneficiary row or strand.** The certificate and every
theorem are identical under both; only the physical destination of a
zero-valued claim differs.

### 1.4 Rounding, ties, and the rows a certificate must carry

*Rounding.* The Lean is exact in scale units. On chain, each order's quote is
rounded once, candidate-wide, at the one named boundary the General already
has — debit rounded up, credit rounded down (`roundedQuoteFor`,
`GeneralClearing.lean:220-230`; `runtime_verify.rs:1234-1241`). Since
`Σ_o (a_o·p) f_o = s·M` exactly (`collectedQuote_funds_sets`), the rounded
sum satisfies `Σ ceil(debits) − Σ floor(credits) ≥ M` and the surplus is the
existing `quoteSurplusPaid` at `Close`. No new rounding boundary.

*Integrality.* The verifier is exact at the scale. For a book of single-outcome
orders the dual polyhedron's vertices are integers at the scale (each vertex
sets `K−1` prices to marginal limits and the last by the simplex row), so an
exact certificate always exists. The same holds for *interval* bundles —
"the result lands in outcomes a..b" — because a consecutive-ones constraint
matrix is totally unimodular, and Product's ordered partition
(`ResultDomainV2::select_ordinary`,
`crates/dclutch-product-runtime-v2/src/lib.rs:243`) makes intervals the
natural bundle. Arbitrary bundles can have half-integer vertices, and then no
exact certificate may exist at the scale. **Rule: `PlaceOrder` admits
single-outcome and interval orders only**; an ε-certificate (dual gap bounded
by the count of marginal orders, in ticks) is the named relaxation if a wider
bundle shape is ever wanted.

*Ties.* The LP's optimal face can be a segment (`jointMint` vs
`jointMintSkewed`): every price inside both full-filled orders' limits
certifies. The rule must choose one. Recommended tie-break, applied by the
selection policy among *certified* candidates only: minimise rounding surplus
(existing `minimizeQuoteSurplus`), then minimise the price vector
lexicographically, then candidate id. Lexicographic-minimum is content-derived
and cheap to compare on the existing selection cursor; it makes the clearing a
*function* of the book rather than of the solver. Rationing among orders that
are exactly marginal at the price — the only orders that can be partially
filled — is pro-rata by lots with the remainder assigned in increasing
order-id order; the candidate's rows are already required in that order
(`NonCanonicalOrder`, `runtime_verify.rs:191-192`). **Ruling owed (ember):
the tie-break.** It changes no theorem.

*Completeness.* The batch record already counts `ORDER_COUNT` and
`CANCELLED_COUNT` (`collection_v1.rs:124, 132`). The terminal verification
row requires the certificate's distinct order count to equal
`order_count − cancelled_count`; with distinct ids and the escrowed-record
read per row, every live order appears exactly once. The Lean carries it as
`Batch.clear?`'s `orderOmitted` refusal (`:634-643`). An unfilled order costs
a row — that is the certificate's marginal price, and §3 counts it.

### 1.5 Hostiles, each against its conjunct

| hostile | Lean witness | conjunct | refusal today | refusal needed |
|---|---|---|---|---|
| an order that would mint an unbacked set | `Examples.unbackedMint` — one buyer, one set, the other outcome priced 50: collected 50 for a 100 set | slackness | `ClaimImbalance` refuses it only because the shipping rule forbids residuals; under the LP it needs its own name | `RuntimeVerifyErrorV2::PricedResidual` |
| a price vector not summing to one | `Examples.offSimplex` | simplex | `InvalidSimplex` (`runtime_width.rs:314`) | unchanged |
| a fill worse than a buyer's limit | `Examples.worseThanLimit` | at-or-better | `QuoteLimit` (`runtime_verify.rs:1244`) | unchanged |
| a fill below a seller's floor | `Examples.belowFloor` | at-or-better | **none** — the order record has no floor | `CreditLimit`, and the record field that carries it (§4) |
| an order rationed strictly inside its limit | `Examples.thinRationed` | marginal | none — the shipping verifier does not read unfilled orders | `RationedInsideLimit` |
| a certificate that omits an order | `Examples.closedBatch.clear? soloBuy` | complete | none — a candidate enumerates only its fills | `OrderOmitted` at the terminal row |
| a batch that clears twice | `clears_once` (`:645-657`); the `.bind` example | phase | `SelectionRefusal.selectionClosed` after `Freeze`; the settlement cursor's phase | unchanged |

All four new codes are `RuntimeVerifyErrorV2` variants in the adapter
contract; they reach the log through the accelerator's `log_line()` and the
chain through Trading's family refusal, in the bands those already own.

### 1.6 The solver's algorithm, for a single-outcome book

Not on chain; stated so the LP is not mistaken for an oracle. For each
outcome sort buys by limit descending and sells ascending; cross them while
`ℓ_buy ≥ ℓ_sell` (transfers); then raise `M` one lot at a time while the sum
over outcomes of the marginal unmatched buy limit is `≥ s` (each extra set is
funded), or lower it while the sum of marginal sell floors is `≤ s` (each
merge pays for itself). Stop; `p_i` is any value between the last-filled and
first-unfilled limit on `i`, chosen to sum to `s` — the interval always
contains a simplex point by the stopping rule. `O(N log N + K·|M|)`, and the
certificate it produces is exactly §1.2's. For interval bundles, the LP.

## 2. The properties

All proven in `JointClearingV1.lean`; theorem names in brackets. The physical
half of each — that `Materialize` moves exactly `M` atoms between the
candidate's Settlement vault and the Hoard, that `Collect` draws from the
order's own escrow — is `AdapterBoundary`'s in `GeneralClearing`, unchanged.

**(a) `Σ p_i = 1` at every clearing** [`prices_sum_to_scale`,
`price_le_scale`, `price_nonneg`]. A conjunct of the verifier; the theorem is
that nothing else passes, and that every coordinate lies in `[0, s]`.

**(b) Every filled order at or better than its limit**
[`filled_at_or_better`]; and its dual, **no order rationed inside its limit**
[`partial_fill_is_marginal`]. Together they say the allocation is competitive
at `p`, not merely admissible.

**(c) Full backing** [`collectedQuote_funds_sets`,
`collectedQuote_exchange`, `sets_are_funded`]. The net quote collected at the
uniform prices equals `s·M` exactly: `Σ_o (a_o·p) f_o = Σ_i p_i net_i` by
exchange of summation, and `Σ_i p_i net_i = Σ_i p_i M = s·M` by slackness.
A mint of `M` sets is paid to the atom by the buyers it serves; a merge
releases exactly what its sellers are paid. **Which census law:** this is L1
(collateral closure) restricted to the batch's Settlement compartment, and
L8's declared per-class delta for `HoardPrincipal` — the Hoard moves by
exactly `M·unit` in one direction and nothing else the batch does touches it
(`tools/gauntlet/journey/src/ledger.rs:11-14, 1004-1012`). L3 holds because
every distributed claim is a Position row and every residual is the
beneficiary's. L4 holds with zero excess: `hoard` rises by `M` and every
outcome's supply rises by `M`. **The new invariant** is complementary
slackness itself — *residual only at zero price* [`residual_nonneg`,
`residual_worth_nothing`] — which is the exact statement that a mint is
funded; the journey should assert it per batch as the batch-local form of L4.

**(d) No arbitrage across outcomes within a batch**
[`complement_prices_to_scale`, `perLotDebit_uniform`]. Any bundle and its
complement price to exactly `s`: assembling a complete set through the batch
and merging it, or splitting one and selling every piece, nets zero. Every
order with the same claim flow pays the same per lot, whoever placed it and
whatever else is in the book. Across batches prices move; inside one they
cannot be crossed.

**(e) Submission time is not a coordinate** [`valid_perm`,
`collectedQuote_perm`]. Reordering the fills changes neither the verdict nor
any quote; priority exists only in the pro-rata remainder rule of §1.4, and
that is content-ordered. What is and is not incentive-compatible:

- *Not strategy-proof.* A uniform-price auction admits demand reduction
  (Ausubel–Cramton): a bidder whose quantity is large relative to the margin
  can shade quantity to lower the price it pays. The gain is bounded by the
  bidder's share of the marginal lots times the price change it induces,
  which is at most the gap between the marginal limits on either side of the
  price; a one-lot bidder cannot move the price except by being the marginal
  order, in which case it pays its own limit and gains nothing by shading.
- *No speed advantage, but an information one.* Orders are on-chain records
  placed during the window and cancellable while collecting (decision 0010
  §1, `CancelOrder`). Being first buys nothing; being last lets a trader
  condition on the visible book. The batch is therefore not sealed-bid, and
  a late order can respond to early ones the way a continuous book allows —
  minus the price impact, which the uniform price removes. The fix is the one
  `INTENT.md` §1 keeps a door open for: *"the batch relation is small and
  specialized on purpose"* as an FHE/MPC target. Sealed orders are a
  transport change, not a rule change; every theorem here survives it.
- *Solver neutrality.* Because certified candidates are optimal and the
  tie-break is a function of the book, a solver cannot profit from *which*
  certificate it submits, only from being paid the work escrow for
  submitting one (decision 0010 §1, "no bond and an exact work escrow").

## 3. The compute price

**Vocabulary.** The tree's measured "N" is the *outcome* width; this section
writes `K` for outcomes and `N` for live orders, so the prompt's "OpenBatch
N=2 at 674,333 CU" is `K=2`.

### 3.1 Measured inputs

| quantity | value | authority |
|---|---|---|
| General `OpenBatch` through Hot, family policy, `K = 2 / 13 / 258` | 674,333 / 666,011 / 680,789 CU | measured, `ae026955d` (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4563`); K-flat because `OpenBatch`'s item stride is 0 (`hot_candidate_v3.rs:73-113`) |
| the same, one accelerator CPI under the output page vs four chunks | 51,404 vs 4 × ~50,201 | measured (`docs/decisions/0003:421-424`, `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:3006`) |
| Trading's own work per Hot transaction, derived | ≈ 474,000 | 674,333 − 4 × 50,201; K-flat to within the ladder's 15 k |
| accelerator `Consider`, standalone, `K = 1 → 258` | 36,113 → 74,877 (≈ 151 CU/outcome) | measured (`PACKET_LIMIT_2026_09_01.md:329`) |
| accelerator `InitializeSettlement`, `K = 1 → 258` | 61,753 → 164,970 (≈ 402 CU/outcome) | measured (same) |
| accelerator `Collect`, `K = 258`, one page | 146,909 / 147,336 / 148,130 | measured (decision 0010 §2a) |
| `VerifyCandidateRow` per row through Hot | **unmeasured**; taken as 45,000 + 400·K per invocation | provisional, extrapolated from the two slopes above; PROGRAMS-16C's four-action run replaces it |
| bank width, per-outcome actions | `8·(151 + 6K) + 32·45 = 2,648 + 48K` bytes | mathematical (`hot_candidate_v3.rs:61-65`, `v2.rs:944-957`) |
| chunks | `⌈(2,648 + 48K) / 880⌉` = 4 for `K ≤ 13`, 5 at `K = 30`, 7 at `K = 60` | mathematical |
| Hot bank cap | `151 + 6K ≤ 512 ⇒ K ≤ 60` | mathematical (`hot_v3.rs:456, 4072-4076`, `UnsupportedContent`) |
| settlement page width `R` (rows per `Collect`/`Distribute`) | 8 | provisional; the 64-lock wall (`PACKET_LIMIT:197`) against ~50 fixed accounts; the Lean profile says 32 |
| ceiling | 1,399,700 | chain-derived |

### 3.2 The per-transaction and per-batch formulas

Per General Hot transaction today: `≈ 474k + chunks(K) × eval(action, K)`;
under the output page (D6): `≈ 474k + eval(action, K) + ~2k`. At `K ≤ 13`
that is **≈ 0.67 M today and ≈ 0.53 M with the page, for every action** —
the ladder's own numbers, and the verification row's estimate lands in the
same band. At `K = 60` the chunk loop takes it to ≈ 0.9 M today and leaves it
at ≈ 0.53 M with the page. **No single transaction approaches the ceiling at
any admissible K; the price is the transaction count.**

Per batch of `N` live orders, of which all are placed, verified and settled:

    critical path  C(N) = 2N + 2⌈N/R⌉ + 8
                          (OpenBatch, N PlaceOrder, CloseBatch, SubmitCandidate,
                           N VerifyCandidateRow, Consider, Freeze,
                           InitializeSettlement, ⌈N/R⌉ Collect, Materialize,
                           ⌈N/R⌉ Distribute, Close)
    with releases  T(N) = C(N) + N + 1     (N ReleaseOrder, CloseCandidate)

The `N` verification rows are the certificate's price: an unfilled order
costs a row because `marginal` must see it. The `2N` place/release
transactions are the escrow's (decision 0010 §2, per-order vaults).

### 3.3 The table

CU are per batch, critical path `C(N)`, at 0.67 M per transaction today and
0.53 M under the output page; `R = 8`. `K` moves each transaction by
`chunks × ~150–400 CU` per extra outcome — under 1 % across the three
columns — so the table is written once and the K-dependence is stated below
it.

| N | C(N) tx | today, K ∈ {2, 3, 5} | with D6 output page | T(N) tx incl. releases | today | D6 |
|---:|---:|---:|---:|---:|---:|---:|
| 2 | 14 | ≈ 9.4 M | ≈ 7.4 M | 17 | ≈ 11.4 M | ≈ 9.0 M |
| 13 | 38 | ≈ 25.5 M | ≈ 20.1 M | 52 | ≈ 34.8 M | ≈ 27.6 M |
| 258 | 590 | ≈ 395 M | ≈ 313 M | 849 | ≈ 569 M | ≈ 450 M |

K-dependence: from `K = 2` to `K = 5`, +4 chunks × 3 outcomes × ~150–400 CU
≈ +2–5 k per transaction (< 1 %); the first K that changes the shape is
`K = 14` (five chunks under the chunked transport — nothing under the page),
and `K = 61` is refused by the bank cap before any compute is spent.

### 3.4 What fits today, and what D6 changes

- **K.** Every `K ≤ 60` fits per transaction (mathematical, the 512-scalar
  cap); `K = 258` fits only the two stride-0 actions. On a Structured market
  the packet caps `K` at 3 (decision 0029 item 7). `K ∈ {2, 3, 5}` are all
  inside every wall with the same per-transaction cost.
- **N.** No per-transaction wall. A batch is `≈ 0.67 M × (2N + 2⌈N/8⌉ + 8)`
  CU on its critical path and that many transactions; at one crank per slot
  (the root's write lock serialises them in practice), `N = 13` clears in
  ≈ 38 slots (≈ 15 s) and `N = 258` in ≈ 590 slots (≈ 4 min). The batch's
  `SETTLEMENT_CLOSE_SLOT` (`collection_v1.rs:120`) is what bounds `N`, and
  `max_orders_per_candidate` (`general-config-contract/src/v3.rs:95`) is the
  declared cap.
- **The 880-byte channel.** It never carries an order: the certificate is
  streamed one row per transaction and each row's bank is the fixed
  `2,648 + 48K` bytes. The channel bounds the *chunk count*, hence the
  per-transaction multiplier — four re-evaluations of the same bank at every
  action with `K ≤ 13`.
- **D6 (decision 0028, output page).** Removes `chunks − 1` re-evaluations
  per transaction: −22 % at `K ≤ 13`, −40 % at `K = 60`, and it flattens the
  K-dependence to one evaluation. It changes nothing about the count.
- **The lever that changes the count** is rows per verification transaction.
  Each row reads its escrowed order record, so rows per transaction are
  lock-bounded at ~10 today; a batch accumulator (Merkle root of admitted
  order ids, maintained by `PlaceOrder`/`CancelOrder`) would replace the
  account read with a `32·⌈log₂ N⌉`-byte proof inside the bank, making rows
  bank-bounded at ≈ 16 (`N = 258`) to ≈ 30 (`N = 13`) per transaction. The
  settlement pages cannot be Merkle'd — atoms move — so `C(N)` stays `O(N)`
  with a smaller constant: the floor is three transactions per order (place,
  collect, distribute) under per-order escrow. Named as the follow-on, not
  designed here.

## 4. What it takes to build

### 4.1 Reused as-is

- The General bank and register model, the family lifecycle policy
  (`ae026955d`), the fifteen-action Hot dispatch, the accelerator as second
  opinion (`programs/dclutch-general-accelerator-sbf/src/lib.rs:1050-1090`),
  both transports.
- The collection half — `OpenBatch`, `PlaceOrder` with worst-case escrow
  (`collection_v1.rs:1416-1429`), `CancelOrder`, `CloseBatch`,
  `ReleaseOrder` — and the batch counters.
- The candidate half — `SubmitCandidate` with the work escrow,
  `VerifyCandidateRow` streaming one row per transaction into the runtime
  verifier, `Consider`/`Freeze` with the selection cursor.
- The settlement machine — `InitializeSettlement`, `Collect`, `Materialize`
  (the in-batch mint/merge as the sole complete-set move, with the Hoard
  patched at runtime per `escrow_v1`), `Distribute`, `Close` with the
  surplus beneficiary.
- Split and merge as user acts (decision 0029 item 5, docket D7) — outside the
  batch, so a holder can turn a complete set into collateral or back without
  waiting for one; inside the batch `Materialize` is the same kernel command
  applied to the candidate's own inventory.
- `GeneralClearing.lean`'s settlement machine and rounding boundary,
  unchanged; `JointClearingV1.lean` is the owner of the *rule* and imports
  nothing from it on purpose.

### 4.2 New, by file

| layer | change |
|---|---|
| `collection_v1.rs` order record | a second signed-terms field, `min_quote_credit_per_lot: u64` (the seller's floor), beside `max_quote_debit_per_lot`; `quote_reserve` unchanged (a seller escrows claims, not quote); `GeneralSignedOrderTermsV1`, its identity preimage and its Lean twin move |
| `runtime_verify.rs` `AuthenticatedOrderTermsV2` | carries the floor; the row step gains `CreditLimit` (`credit < floor × lots`) and `RationedInsideLimit` (`lots < max_lots` and the derived per-lot value strictly inside the limit) |
| `runtime_verify.rs` terminal step (`balance_from_cursor`) | `ClaimImbalance` becomes: `net_i ≤ M` everywhere, `net_i = M` where `p_i > 0`, else `PricedResidual`; `OrderOmitted` when the distinct order count differs from the batch's live count |
| `runtime_settlement.rs` / `escrow_v1.rs` | the beneficiary residual row on `Distribute` (or the strand command, per the ruling) |
| `GeneralClearing.lean` selection policy | a first criterion `requireCertificate` (only certified candidates compete) and `minimizePriceVector` before `minimizeCandidateId`; emitted through `EmitGeneralSelectionDecisionCorpusRust.lean` |
| `PlaceOrder` admission | the bundle-shape rule of §1.4: single outcome or interval, `ShapeNotInterval` otherwise |
| `JointClearingV1.lean` | one `import` line in `DClutchSemantics.lean`, owned by whoever next edits it (the lakefile glob already builds the module; the import is for `AbiCoverage` and the README census) |
| journey ledger | a per-batch assertion of §2(c)'s new invariant, residual-only-at-zero-price, evaluated at the `Materialize` boundary |

Refusals: four new `RuntimeVerifyErrorV2` variants and one `PlaceOrder` shape
refusal, in the General adapter's existing bands; no new band, no new
program. Every General artifact digest moves (the order layout is in the
signed terms), so this is a cohort boundary.

### 4.3 The families it subsumes, and the ones it does not

- **Direct is not a batch of one.** A Direct fill is two orders whose limits
  cross, cleared at the *maker's* limit — a priority rule the batch
  deliberately lacks. In the batch the same two orders certify at every price
  between the two limits and the tie-break picks one; the maker-price rule is
  a different tie-break, not a different mechanism. Direct stays as the
  no-solver, one-transaction path; joint clearing subsumes its economics and
  not its latency.
- **The Dealer becomes a participant, not a venue.** Its sealed rules produce
  a schedule of limit orders from its own capital at `OpenBatch`; placing them
  is `PlaceOrder` from the Dealer's custody, and its worst-case escrow is the
  order escrow. Nothing in the kernel changes; the Dealer's accepted-transition
  contract keeps it from quoting, which is exactly what a batch participant
  does not do.
- **Structured and Fractional shards** are a representation of the same
  claims; a shard holder reconstitutes to native before the batch and
  denominates after. Orthogonal to clearing; the batch trades native claims.
- **Product** supplies the outcomes (`select_ordinary`) and, through its
  ordered partition, the interval bundles §1.4 admits.
- **Series** is a schedule of batches, one per occurrence; unchanged.

### 4.4 The cohort

Cohort-16 is in flight with PROGRAMS-16C's four-action run and the founding
changes of decisions 0025/0027; it must not carry an order-layout change
under it. **Cohort-17** is the earliest: the order record and verifier
conjuncts land together with their Lean emission, the selection policy gains
its two criteria, and the four-action run is re-measured on the certified
shape — which is also the measurement that replaces §3.1's one provisional
row. D6's output page, if ember rules it in, rides the same boundary and
takes the per-transaction figure from 0.67 M to 0.53 M.

## 5. Rulings owed to ember

1. **Residual disposition** (§1.3): beneficiary row (recommended) or strand.
2. **Tie-break** (§1.4): lexicographic-minimum price vector after minimum
   rounding surplus (recommended), or another content-derived rule.
3. **Sealed or visible** (§2e): the batch is visible-bid by construction of
   on-chain order records; whether the FHE horizon is reopened for it is a
   product question, not one this note can settle.

## Evidence pointers

`formal/dclutch-semantics/DClutchSemantics/JointClearingV1.lean` (whole);
`formal/dclutch-semantics/DClutchSemantics/GeneralClearing.lean:132-158,
220-230, 262-297, 320-360`;
`crates/dclutch-general-adapter-contract/src/runtime_verify.rs:191-208,
220-231, 1234-1245`; `runtime_width.rs:187-188, 314-315`;
`collection_v1.rs:114-132, 1416-1429`; `hot_candidate_v3.rs:61-113,
2485-2506`; `escrow_v1.rs:307-308`;
`crates/dclutch-general-config-contract/src/lib.rs:101, 365, 407`, `v3.rs:95`;
`programs/dclutch-trading-sbf/src/hot_v3.rs:456-457, 4072-4076`;
`programs/dclutch-general-accelerator-sbf/src/lib.rs:544-597, 1050-1090`;
`tools/gauntlet/journey/src/ledger.rs:11-57`;
`docs/decisions/0003:421-424`, `0009` §1, `0010` §1-2, `0028`, `0029` items
5 and 7; `docs/design/ACCELERATOR_OUTPUT_CHANNEL_2026_09_02.md:28-45`;
`docs/design/GENERAL_INPUT_TRANSPORT_2026_09_02.md`;
`docs/design/PACKET_LIMIT_2026_09_01.md:155-156, 197, 286`;
`docs/INTENT.md` §1, §4; `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:3006, 4563`; commit `ae026955d`.
