# `clutch-batch`

This is an executable, host-only prototype of a bounded fixed-grid frequent
batch relation. It has no Solana, Token-2022, CPI, keys, account layout, or
matching-service dependency.

Executable facts:

- the grid has at most 64 strictly increasing ticks and the book has at most 64
  orders;
- orders must be in strictly increasing canonical-order-ID order;
- the clearing tick maximizes matched quantity, then minimizes imbalance, then
  chooses the highest tick;
- each side is allocated by integer largest-remainder pro-rata using the frozen
  seed and canonical IDs;
- `DustPolicy::Reject` explicitly refuses unresolved dust, while
  `AssignCanonical` assigns every leftover atom deterministically;
- `verify` recomputes eligibility, fills, side totals, and conservation.

The implementation does not claim global optimality, fairness against order
fragmentation, privacy, settlement correctness, or formal verification. The
Verus theorem inventory and proof seam are under `verus/batch/`; those targets
remain open until a pinned Verus run checks this exact source.

## The coupled relation (`relation_v1`)

`src/relation_v1.rs` implements `BatchRelationV1` from
`docs/implementation/BATCH_RELATION_V1_DESIGN.md` beside the scalar lab, which is
retained unchanged. It is IMPLEMENTED host-model code: not verified, not the SVM
relation, and never an optimality claim. An accepted clearing is the best valid
submitted candidate of its proposal window, nothing more.

Executable facts:

- every fill is bound to `(owner, outcome, side)`; the fill vector, the price
  vector, the conversion pair, and the honored-minimum mask are the whole
  witness, and every claimed aggregate is recomputed from the frozen book;
- per-outcome conservation runs through one global virtual split/merge pair, so
  fills must carry the same net imbalance `c` on every active outcome and a
  cross-outcome "match" has no solution at all;
- V5 checks the exact integer pairing-feasibility inequality
  `part_i(O) <= F_i`, and the canonical constructor freezes the slice
  decomposition that settlement consumes;
- `FrozenPolicyV1` has no `Default`: every variant family named in the design
  (allocation A/B, self-cross N-a/b/c, all-or-none 2a/b/c, rounding R-a/b/c,
  residual settlement 1a/1b/1c, transfer phase T-a/b, portfolio lots P-a/b,
  pairing witness, dust, score, fee base) must be named at the construction site;
- the relation is `no_std`, allocation free, `forbid(unsafe_code)`, float free,
  and every accumulator is checked exact integer arithmetic.

## ScoreV2-Q core interface

`src/score_v2.rs` implements the production-quality core arithmetic for the
complete-set-quotiented successor score, without selecting it in the V1
relation or any SBF profile. It independently derives
`d_i = B_i - sigma = E_i - mu` and ranks valid submitted candidates by:

1. maximum `max(d) - min(d)`;
2. minimum directly crossed complete-set layer `min(d)`;
3. minimum virtual split/merge churn; and
4. the smaller full candidate digest.

The API accepts only an explicit owner-blind normalization contract. It names
and refuses the owner-tag-dependent V1 variants because public-key relabeling
can change their admitted flow. The exact claim is representation-neutral after
admission, never person-neutral or wash-proof. Price quality, fees, bonds, and
solver compensation remain separate policies. See
[`docs/design/SCORE_V2_Q.md`](../../docs/design/SCORE_V2_Q.md).

## Owner-blind economic relation (`relation_v2`)

`src/relation_v2.rs` is the first registry-independent RelationV2 core. Its
fixed-width order language uses one nonnegative coefficient vector for both
single-Egg and portfolio orders. No input type contains an owner, signer,
account, fee group, dealer, or settlement binding, so changing those external
labels cannot change the economic verdict.

The verifier checks canonical book/order/fill padding, expiry, exact price-unit
limits, all-or-none and minimum fills, checked coefficient expansion, one
canonical virtual split-or-merge, per-outcome conservation, and both
`d_i = B_i - sigma = E_i - mu` derivations. It SHA-256 commits every canonical
economic input using a local safe, allocation-free FIPS-180 implementation and
feeds the full digest into ScoreV2-Q.

Price coherence remains an upstream semantic precondition. RelationV2
recomputes a proof-independent semantic digest from the immutable domain,
price-policy identity, and exact canonical integer simplex. Proof and
certificate representations never enter the economic identity or rank. The
core does not copy V1b or authenticate the upstream theorem. Candidate
allocation, fees, dealer transitions, account codecs, settlement authorization,
lifecycle/SBF dispatch, and beneficial-controller identity are not implemented
by this core.

## Pure covered-dealer join (`dealer_leg_v2`)

`src/dealer_leg_v2.rs` is an additive, registry-independent relation over a
validated RelationV2 candidate. It derives the unique net dealer buy or sell in
each outcome; a candidate cannot choose gross same-outcome dealer churn. The
legacy RelationV2 verifier is unchanged as a public acceptance boundary and
continues to refuse flows that need this counterparty.

The submitted dealer leg contains only immutable order IDs and dealer-filled
units. Receipt, residual envelopes, fee amounts and policy, facility policy,
and pre-generation live in `DealerQuotePreconditionV2`. The core recomputes its
proof-independent semantic digest and refuses any byte mutation under a fixed
digest. It does not authenticate the quote proof or facility account; the SBF
adapter must establish that trust before invoking this relation.
Successful full verification returns a private-field `VerifiedDealerLegV2`
in-memory capability. Safe downstream code cannot fabricate that capability
from the public `DealerLegVerdictV2`, whose public fields remain an explicitly
unauthenticated persistence/projection DTO.

`MinimumGrossHamiltonV1` derives per-order cash from immutable sorted order IDs,
dealer-filled units, residual buyer maxima, residual seller minima, and one net
aggregate dealer cash transition. Gross user cash and the net transition obey
`user_in + dealer_net_out = user_out + dealer_net_in`. Buyer cash is
Hamilton-allocated by residual capacity;
seller excess above exact minima is Hamilton-allocated by native Egg atoms.
Equal remainders prefer the smaller order ID. Candidate-supplied per-user cash
does not exist, and any settlement copy can be checked against recomputation.
Fees are summed and digest-bound separately and never enter dealer cash.
This relation does not establish their recipient, funding source, custody, or
transfer conservation; those are fee-transition and adapter obligations.
The pure relation admits the full 64-order RelationV2 book; any smaller runtime
chunk or LP-roster width is an adapter concern, not a market restriction.
Gross cash, fee totals, and Hamilton weights use checked `u128`, so individually
valid `u64` rows do not acquire an accidental aggregate-`u64` market cap.

The joined digest commits the RelationV2 semantic digest and the recomputed
quote semantic digest. The latter commits immutable facility, dealer-policy and
fee-policy identities, pre-generation, policy version, receipt, derived trade,
and every canonical fill and envelope row. Both exclude proof bodies and
derived allocation bytes.
This module still contains no accounts, custody, token logic, registry lookup,
authorization, lifecycle transition, or SBF dispatch.

## Account-ready exact portfolio pair (`portfolio_execution_v2`)

`src/portfolio_execution_v2.rs` replaces the old content-only atomic portfolio
seam with a narrow execution authority contract. `EconomicOrderV2` remains the
only coefficient owner. A canonical 568-byte selected-order record binds an
adapter-authenticated counted SettlementRoot, retained Feed/traversal,
OrderPage, page slot, dense RelationV2 index, owner, Position incarnation,
selected fill, and settlement witness; it does not carry a second coefficient,
quantity, limit, or policy DTO.

The private pair capability admits only one exclusive full pair with opposite
sides, distinct owners and Positions, no virtual conversion, and exact equality
of every cell in the full 16-wide RelationV2 coefficient arrays. It values the
pair as `sum(coeff_i * price_i) * filled_units` in checked `u128` and performs
one named `ExactReceiptDivisionV1` conversion by the integer price scale. A
remainder refuses instead of selecting an unnamed beneficiary.

The prepared account transition is indivisible. It authenticates the exact
counted SettlementRoot/retained-Feed traversal, both ENTITLED Reservations,
both Position V3 prestates, both purpose Replay V3 prestates, and the
counted/rent-owned 298-byte SettlementReceipt V5 through a private adapter
capability seam. It derives exact cash
debit/refund/credit and the
16-wide native-Egg debit/credit, requires both Reservations' canonical CONSUMED
postimages, keeps Position incarnation generations stable, advances each Replay
ordinal once, and commits all pre/post semantic identities into a canonical
680-byte replay-sensitive vector-transition preimage. The receipt must enter
delivery as `PortfolioPairPending` (kind `1`, zero commitment); its exact
V5-domain hash is set once as `PortfolioPairCommitted` in the SettlementReceipt
V5 postimage, so this slice creates no
uncounted account or liability. Persisted records and decoded receipt preimages
are untrusted projections; only the private prepared capability authorizes an
adapter to compose writes and CPIs.

This is account-ready pure contract code, not a routed SBF instruction or
deployment result. The remaining live work is listed in
[`PORTFOLIO_EXECUTION_V2.md`](PORTFOLIO_EXECUTION_V2.md).

Not implemented, and refused rather than guessed: portfolio marginal lot
rationing (`P-b` returns `PolicyVariantUnimplemented`), the `N-c` owner-aware
decreasing-fixed-point capping rule (infeasible candidates are refused, not
capped), and every fee base except the flat-notional and zero-fee controls.
Settlement, the kernel transfer, and the vertical-model joins live in their own
crates; this one only records their frozen selectors and freezes the slice
universe.
