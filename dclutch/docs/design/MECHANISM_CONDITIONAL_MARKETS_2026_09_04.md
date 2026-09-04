# Conditional and product markets: the combinatorial layer

CONDITIONAL design lane, 2026-09-04, read at `a1bf4ddf0` and written at
`60ae17272` (`git rev-parse --show-toplevel` = `/Users/ember/dev/dclutch`).
**Design only.** No program source moves; the deliverables are this note, the
Lean module `formal/dclutch-semantics/DClutchSemantics/ConditionalMarketV1.lean`
(50 theorems, 34 executable witnesses, zero `sorry`, zero warnings, `lake build`
green at v4.30.0 — 145 jobs), and a compute price. Every path:line below is
HEAD's. The companions this composes with: JOINT-CLEARING (the certificate the
child's clearing is verified by), BATCH-SPINE (the batch every child trades
in), SCORING-DEALER (the participant that is in every child batch), and the two
in flight at the time of writing — FOUNDER-BOND (note and Lean present,
uncommitted) and ENSEMBLE (Lean present, note not yet) — each named where a
fact of theirs is used.

**In one paragraph.** A child Market names two parent Markets at founding and
observes nothing itself: its outcomes are built from the parents' outcomes and
its certificate is a function of the parents' certificates. A **product market**
`A × B` has one ordinary cell per pair of ordinary parent outcomes, row-major
with `A` as the major axis, plus its own explicit failure coordinate. A
**conditional market** `B | A = a` has one ordinary cell per ordinary outcome of
`B`, one ordinary *off-condition* cell that pays when `A` resolves to any other
ordinary outcome, plus its own failure coordinate. Both settle to their failure
coordinate — and so to decision 0025's constant-per-claim refund walk, reused
verbatim — exactly when a parent the branch depends on resolved to *its*
failure coordinate. The conditional market is the product market's row
projection, exactly, on the condition branch; off it, it settles the moment `A`
is terminal without reading `B` at all. `P(B = b | A = a)` is a ratio of the
child's own prices. Nothing crosses between the child's Hoard and the parents';
consistency between a child's marginals and its parents' prices is closed by
arbitrage and by no conjunct, and the note says what a trader does about it.

## 0. What the tree already is, and what this note adds

Three facts the tree has make a child market nearly free:

1. **A Product is a partition of the rational line, and the joint index is a
   rational.** `ResultDomainV2` is `cut_count` strictly increasing cuts over one
   denominator with `region_count = cut_count + 1` ordinary regions and the
   failure selector at `region_count`
   (`crates/dclutch-product-runtime-v2/src/lib.rs:205-213`); `select_ordinary`
   maps a coordinate to the number of cuts at or below it (`:243-281`;
   `ProductRuntimeV2.lean:384-396`). A domain with cuts `[1, 2, …, n − 1]` over
   denominator `1` maps the integer `v ∈ [0, n)` to region `v`. So a child whose
   provider hands the Product the joint index as its observation is an ordinary
   `ResultDomainV2` record: no new domain kind, no new selector, no new
   admission. The cells are the regions; the child's failure coordinate is the
   region count, where every Product already puts it.
2. **The refund arm exists and is a theorem.** A categorical basis founded at
   `payoutScale = width − 1` refunds one collateral atom to every ordinary claim
   on the failure walk and nothing to the failure coordinate
   (`ProductBasisV3.lean:171-189`, `categoricalRefundsOnFailure`,
   `evaluateCategoricalFailure`; `categorical_terminals_partition_the_payout_scale`,
   `:205`); the kernel's `failureRefund` is `quantity · multiplier / ordinaryCount`
   with `foundingRefundExact` making the remainder impossible at founding
   (`EconomicKernel.lean:812-823`, `an_admitted_founding_makes_every_refund_exact`,
   `:1020`). This is the "constant-per-claim walk" the prompt names, and the
   child reuses it as its outage arm with nothing added.
3. **A certificate is a persisted, authenticated, kind-seeded account.**
   `ResolutionCertificateV2` is 312 bytes
   (`SourceResolutionTerminalV2Abi.lean:186`, `certificate_exact_width`),
   carries `market`, `generation`, `product_record_digest`, `selector`
   (`u32`, untruncated) and its kind (`crates/dclutch-resolution-codec/src/v2.rs:145-175`),
   lives at a PDA whose seed includes the kind so the four kinds one state can
   write never overwrite one another (`:131-142`, `kind_seed`), and Core already
   reads one exactly this way at `AdmitTerminal`
   (`programs/dclutch-core-sbf/src/resolution.rs:138`, frame index 14;
   `:1017-1049`, `authenticate_terminal_certificate`: expected key, owner is the
   Resolution program, `data_len == 312`, rent persists, decode, kind, market,
   source material, generation). A child's provider reads two of these instead
   of one price account.

What the tree does NOT have, and what this note supplies: (i) the
**parent-reference record** a child founds against, and the refusals that bind
a certificate to it; (ii) the **derived provider** — a Source provider family
whose evidence is two on-cluster certificate accounts and no signature, no
relayer and no feed; (iii) the **claim shapes** and the decision, stated in §1,
that the conditional market is the product's projection; (iv) the
**properties** as theorems and the **hostiles** as refusals by name; (v) the
one Source transition the outage arm wants and does not have (§4.3), with the
fallback that works today.

## 1. The claim shapes, exactly

Notation: parent `A` has `R_A` ordinary outcomes (its Product's `region_count`)
and failure selector `R_A`; parent `B` likewise with `R_B`. `unit` is the atoms
one ordinary claim draws on the refund walk; a child's payout scale is
`ordinaryCount · unit`, the shape 0025 admits. In the Lean, `ParentTerminal`,
`ParentRef`, `ParentCertificate` (`ConditionalMarketV1.lean`, §"What a parent
hands its children").

### 1.1 The product market `A × B`

- **Cells.** `R_A · R_B` ordinary cells, cell `(a, b)` at index `a · R_B + b`
  (`ProductShape.cell`), row-major with `A` as the **major axis**. Decoding is
  `row i = i / R_B`, `column i = i mod R_B` (`cell_row`, `cell_column`,
  `cell_injective`).
- **Failure coordinate** at index `R_A · R_B` (`ProductShape.failureSelector`);
  width `R_A · R_B + 1`.
- **Payout.** On an ordinary cell, the kernel's categorical one-hot at the
  child's scale; on the failure coordinate, the refund vector `[unit, …, unit, 0]`
  (`childPayoutVector`, built from `EconomicKernel.successPayoutVector` and
  `failurePayoutVector`, `EconomicKernel.lean:1049-1055`).
- **Selector.** `productSelector s ta tb = cell ta.selector tb.selector` when
  both parents are ordinary, else the failure coordinate
  (`productSelector`; `productSelector_lt_width`,
  `productSelector_ordinary_iff`, `productSelector_failure_of_parent_failure`,
  `productSelector_decodes`).
- **The Product record.** A `ResultDomainV2` with cuts `[1, …, R_A R_B − 1]`
  over denominator `1`, `source_scale_exponent = 0`; a categorical
  `ProductBasisV3` at `payoutScale = R_A · R_B` (refunding). Both are ordinary
  records the founding compiler already emits; only their *inputs* are new
  (§2.2).

A trader who wants "the joint outcome is `(a, b)`" buys one cell. A trader who
wants "`A = a` whatever `B`" buys the row bundle `[a · R_B, (a + 1) · R_B)` —
**one interval**, which is the only bundle shape the joint clearing admits
(`MECHANISM_JOINT_CLEARING_2026_09_04.md` §1.4). A trader who wants "`B = b`
whatever `A`" places `R_A` single-cell orders, because a column is not an
interval under this layout. **The founder chooses the major axis, and the
decision axis is the major axis** — for a decision market, the condition is the
row.

### 1.2 The conditional market `B | A = a`

- **Cells.** `R_B` *branch* cells at `0 … R_B − 1` (cell `b` is "`A = a` and
  `B = b`"), one **off-condition cell** at `R_B` ("`A` resolved to an ordinary
  outcome other than `a`"), the child's failure coordinate at `R_B + 1`; width
  `R_B + 2` (`ConditionalShape`).
- **Payout.** As above: one-hot at the child's scale `(R_B + 1) · unit` on any
  ordinary cell — the off-condition cell included — refund vector on failure.
- **The refund arm keyed on the parent's certificate** is the off-condition
  cell: `A`'s certificate names an ordinary selector `≠ a` ⇒ the child settles
  ordinarily to cell `R_B` and pays its holders the scale
  (`off_condition_cell_pays_the_scale`) while every branch cell pays zero
  (`branch_claims_pay_nothing_off_condition`). The child's *own* failure
  coordinate is reserved for what 0025 designed it for: an outage — `A`'s
  certificate is a failure, or `A = a` and `B`'s certificate is a failure.
- **Selector.** `conditionalSelector s ta tb?` with `B` an `Option`
  (`conditionalSelector`): `A` failed ⇒ failure; `A ≠ a` ⇒ off-condition,
  **without reading `B`** (`off_condition_settles_without_B`,
  `off_condition_ignores_B`); `A = a` ⇒ `B`'s selector if ordinary, failure if
  `B` failed, refuse `parentNotTerminal` if `B` is not yet terminal
  (`condition_branch_needs_B`).
- **The Product record.** Cuts `[1, …, R_B]` over `1` (`R_B + 1` regions), a
  categorical basis at `payoutScale = R_B + 1`.

### 1.3 Is a conditional market a projection of a product market? Decided: yes, exactly, on the condition branch

- On the condition branch with both parents ordinary,
  `productSelector = a · R_B + conditionalSelector`
  (`conditional_is_the_row_projection`): the conditional's branch cells *are*
  row `a` of the product, index for index.
- Off the condition with both ordinary, the product selector lies outside row
  `a` and the conditional settles to its off-condition cell
  (`off_condition_is_the_complement`): the off-condition cell is the
  complement of row `a`, merged into one cell.
- On the condition branch the two fail together
  (`failure_agrees_on_the_condition_branch`).
- **Where they differ, and it is the reason the conditional exists as a
  separate shape:** off the condition the conditional does not read `B`
  (`off_condition_ignores_B`), so `B`'s outage, or `B` never resolving, cannot
  reach it; the product fails there. And the width is `R_B + 2` against
  `R_A · R_B + 1`.
- **At the price level** the projection is the *conditional read*: in the
  product market `P(b | a) = p_(a,b) / Σ_b' p_(a,b')`, a ratio of the child's
  own prices (`conditionalRead`, `conditional_reads_partition_the_row`,
  `conditionalRead_le_row`); in the conditional market the same read is
  `p_b / Σ_b' p_b'` over the branch cells. Neither needs the parent's price.
- **At the claim level** a trader in the conditional market who wants Hanson's
  *voided* claim — P&L independent of `¬a` — holds branch cell `b` and the
  off-condition cell in the ratio `y = p_b / (S − p_⊥)` (so the position's
  `¬a` payoff equals its cost); its effective price is `p_b / Σ_b' p_b'`. The
  hedge is constructible from the child's own claims because the off-condition
  cell pays the whole scale (`off_condition_cell_pays_the_scale`). In the
  product market the same hedge is row `a` against the complement of row `a`.

**Recommendation:** build the product shape first; found a conditional market
when the product's width does not fit (§6) or when the founder wants the
off-condition branch to settle without waiting for `B`. A conditional market
is the product market with the off-condition rows merged; nothing else.

### 1.4 The shape that was priced and not chosen

The prompt's sentence — *refund (a complete-set return) if `A` resolves
otherwise, reusing the escrow's constant-per-claim walk* — names a third shape:
width `R_B + 1`, and `A ≠ a` routed through the child's **failure coordinate**
so that every branch claim refunds `unit`. It was priced and refused, for three
reasons, each a theorem or a line of the tree:

1. **It is not a voided claim.** A trader who bought cell `b` at `p_b` and sees
   `¬a` is paid `unit`, not `p_b`; their P&L depends on `¬a`. Hanson's voiding
   returns the *purchase price*, which a fungible-claim kernel with no
   per-trade price memory cannot do; the off-condition cell (§1.2) lets the
   trader construct voiding themselves, and the walk does not.
2. **Its read needs the parent's price.** With `q = P(A = a)`, the branch cell
   prices to `R_B · P(a, b) + (1 − q)`, so `P(b | a) = (p_b − (1 − q)) / (R_B q)`:
   the read is only as consistent as two markets' batches. §1.2's read is a
   ratio of one batch's prices.
3. **It routes an ordinary, expected event through the outage machinery.**
   `commitFailure` requires `Exhausted`, which requires the window to have
   closed (`SourceResolution.lean:1146`,
   `exhaust_requires_the_window_to_have_closed`; `:1034`,
   `failure_commit_requires_exhaustion`), so the refund could not land until the
   child's deadline; and the walker's bounty from the explicit-failure funding
   compartment (`MAINNET_STATE_RELAY.md` §12.7) would be paid for an event of
   probability `1 − q`. The escrow's walk is kept for outages, where its
   vocabulary is true.

## 2. Founding

### 2.1 The parent-reference record: one author

`ParentReferenceV1`, content-addressed and immutable, written once by the
founder and named by the child's `SourceSpec` as its `decoding_rules_id`
(the slot the relay family already uses for "every layout fact",
`MAINNET_STATE_RELAY.md` §4.1, §12.1):

| field | width | what binds it at settlement |
|---|---:|---|
| `kind` | u8 | `1` product, `2` conditional |
| `parent_a.market` | 32 | `certificate.market` (`wrongParent`) |
| `parent_a.generation` | u64 | `certificate.generation` (`parentGenerationMismatch`) |
| `parent_a.product_record_digest` | 32 | `certificate.product_record_digest` (`parentRecordMismatch`) |
| `parent_a.ordinary_count` | u32 | the referenced Product's `region_count`, re-read against the certificate's selector (`parentWidthMismatch`, `selectorOutOfRange`) |
| `parent_b.*` | same | same |
| `condition` | u32 | conditional only: `< parent_a.ordinary_count` at founding |
| `settle_by` | u64 | the child's deadline, §2.3 |

The Lean twin is `ParentRef`; `ParentRef.admit` is the six-conjunct check
(`admit_ok_binds_the_reference`), and the founding conjuncts are
`ProductShape.found?` / `ConditionalShape.found?`
(`found_product_fits_the_bank`, `found_conditional_condition_is_ordinary`):
distinct parents (`sameParent`), both with an ordinary outcome (`emptyParent`),
a condition strictly below `R_A` — the condition on `A`'s failure coordinate is
refused by its own name (`conditionOnFailure`), because a market that pays when
its parent has an outage is 0025's founder revenue one level up — and a width
the bank clears (`widthOverflow`, §6).

The parents must be **Open or Terminal** at founding, never Founding (their
Product record is not yet authenticated) and never Retired (§4.4). Their
generation and Product record digest are read from their Market state and
Registry rows at founding and frozen into the reference; the child's Market
commits the reference's identity the way it commits its Source material
(`SourceMaterialV3Abi.lean:5-15`, *"the Market commits the Source-material
identity, so a founding witness cannot substitute"*).

### 2.2 The child's own records, all existing shapes

- `ResultDomainV2`: cuts `[1, …, n − 1]`, denominator `1`, exponent `0`, with
  `n = R_A R_B` (product) or `R_B + 1` (conditional). Emitted by
  `dclutch-product-compiler` from the reference; its identity is the child's
  `resultDomainId` (`MarketCore.lean:47-59`, `Product`).
- `ProductBasisV3`: categorical, `width = n + 1`, `payoutScale = n`
  (`categoricalRefundsOnFailure`, `ProductBasisV3.lean:179-181`; the minimum
  refunding width is 3, `:174`, which every child satisfies since `n ≥ 2`).
- `SourceMaterialV3` with a `SourceSpec` whose provider family is
  **`DerivedFromParentsV1`** (§4.1), `recoveryPresent = false` — the child buys
  no alternative source because it has no source; its parents' ladders (0027)
  are what guarantee that a certificate arrives.
- The failure escrow Position and the explicit-failure funding compartment,
  exactly as any market founded under 0025/0027: the founder prepays the
  walker's bounty for the child's own deadline walk.

### 2.3 The window ordering

The child's `Leg` (`SourceResolution.lean:225-234`) is degenerate on purpose:
`windowStart = 0`, `windowEnd = acceptThrough = settle_by`,
`maximumPublicationAge = settle_by`. Its "observation" is the parents'
terminals *whenever they happened*, including before the child was founded
(a child may be founded on an already-terminal parent; nothing forbids a
market about a fact already known, and the clearing prices it at the corner).
The founding conjunct on `settle_by`, refused by name (`WindowBeforeParents`):

    settle_by ≥ deadline(A) + margin       (product and conditional)
    settle_by ≥ deadline(B) + margin       (product; conditional on the branch)

where `deadline(P)` is `P`'s last funded rung's deadline
(`SourceResolutionStateV2Abi.lean:389-412`, `Ladder.deadline?`) or, with no
policy, `end_unix_seconds + max_age_seconds` of `P`'s window
(`SourceWindowSpecV1Abi.lean`), and `margin` covers `P`'s own walk
(`MAINNET_STATE_RELAY.md` §12.7: one transition with no policy). A child that
reaches `settle_by` with a needed parent still live walks to its failure
coordinate through `exhaust`/`commitFailure` and refunds — the parents' ladders
make this a founder's mis-sizing, not a live risk, and BOND (in flight) is
what prices a founder's mis-sizing.

## 3. Clearing

A child clears in the General batch like any market: **its clearing is a
`JointClearing.Clearing` of `outcomeCount = n + 1`**
(`ProductShape.clearingFits`), verified by the eight-conjunct certificate
(`JointClearingV1.lean:218-224`, `Clearing.valid`) with complete-set minting
inside the batch, and it carries the scoring Dealer's schedule row like any
market (SCORING-DEALER §2, R0–R3 at `K = n + 1`). Nothing in the clearing knows
the market is a child.

- **Bundle shapes** (§1.1): rows are intervals; columns and arbitrary cell sets
  are not, and `PlaceOrder`'s `ShapeNotInterval` refuses them
  (JOINT-CLEARING §1.4). The row-major layout is chosen so that every
  conditional-on-`A` order is one row. Under the interval rule the
  constraint matrix stays totally unimodular and the exact certificate
  exists at the scale, unchanged.
- **Marginals** are read off the price vector by summation and nothing else:
  `rowPrice c s a = Σ_b p_(a,b)`, `columnPrice c s b = Σ_a p_(a,b)`. They
  partition the cells (`rows_partition_the_cells`,
  `columns_partition_the_cells` — exchange of summation over the rectangle,
  `sumRange_rect`, `sumRange_swap`), and with the certificate's simplex
  conjunct they sum to the scale less the failure coordinate's price
  (`marginals_sum_to_the_scale`). The failure coordinate is held by the
  escrow, nobody bids for it, and complementary slackness leaves the batch's
  residual there at price zero (`residual_worth_nothing`,
  `JointClearingV1.lean:380`) — so in practice the marginals are an exact
  simplex over the parents' ordinary outcomes.
- **The conditional read** `P(b | a) = p_(a,b) / rowPrice a`
  (`conditionalRead`) is exact, needs no parent price, and is bounded by the
  row (`conditionalRead_le_row`). The two rows of a two-outcome decision
  parent give the futarchy comparison `P(Y | X) − P(Y | ¬X)` from one price
  vector.
- **The price series** (BATCH-SPINE §2(c)) is the child's own; the marginal
  series and the conditional series are functions of it, computed by any
  reader, and become chain facts when the spine's owed persistence of the
  frozen price vector at `Close` lands.

## 4. Settlement

### 4.1 The derived provider: two certificate reads, no observation

`DerivedFromParentsV1` is a Source provider family in
`programs/dclutch-resolution-proof-sbf` beside `provider_v3.rs` and
`relay_v1.rs`: its `accept` presents the two parent certificate accounts,
authenticates each exactly as Core does at `AdmitTerminal`
(`resolution.rs:1017-1049`, the reference implementation: expected PDA from
`(parent market, generation, kind_seed)`, owner, width 312, rent, decode,
kind ∈ {`ResolutionSuccess`, `ResolutionFailure`}), admits each against the
reference (`ParentRef.admit` — six refusals, §2.1), computes the joint index,
and emits `NormalizedEvidence` with `value = index / 1`, `observationTime` and
`publicationTime` the later certificate's `observed_at`, `evidenceId =
H(cert_A ∥ cert_B)` (`SourceResolution.lean:267-281`). From there the Source
machine is untouched: `specialize` admits it under the Leg, writes the child's
`Certificate` with `kind = ResolutionSuccess`, `selector = index`, and the
child is resolved (`accept_post_is_resolved`, `:1092`; one answer,
`resolved_admits_no_second_answer`, `:1111`).

The conditional's off-condition branch presents **one** certificate (`A`'s) —
`conditionalSelector` takes `B` as an `Option` and the route takes `B`'s
account as optional; the on-condition branch presents both.

### 4.2 The child's certificate, and the terminal it drives

The child's certificate is an ordinary `ResolutionCertificateV2`; Core's
`AdmitTerminal` joins it to the child's Product width unchanged
(`MarketCore.lean:625-630`, `TerminalFrame`); the wallet payout reads it at
account 25 of the family-neutral terminal child
(`crates/dclutch-claims-svm/src/terminal_settlement_v3.rs:22-31`) unchanged.
**No route downstream of `accept` knows the market is a child.**

### 4.3 The refund arms

- **Condition failed** (`A` ordinary, `≠ a`): an ordinary terminal at the
  off-condition cell, through the honest walk, the moment `A` is terminal.
- **Outage** (`A` failed; or `A = a` and `B` failed; product: either failed):
  the child's failure coordinate, hence 0025's walk: every ordinary claim
  draws `unit` (`outage_refund_is_constant_per_claim` is
  `an_admitted_founding_makes_every_refund_exact` under its child name), the
  escrow's failure claims draw nothing, the Hoard is emptied exactly once
  (`childPayoutVector_sum`).

  **How the child gets there.** The derived provider can *see* a parent's
  failure certificate, but the Source machine reaches the failure selector
  only through `exhaust`, which refuses before `acceptThrough`
  (`exhaust_requires_the_window_to_have_closed`). Two dispositions:

  - **Fallback, works today:** wait for `settle_by`; the funded walk of
    §12.7 lands on the child's failure selector. Correct, slow (the whole
    margin), and it pays the walker's bounty for a fact the chain already
    held.
  - **Recommended, owed:** one Source transition conjunct,
    `AttestedUnobservable` — `exhaust` is admitted before `acceptThrough`
    when the active leg's provider presents evidence that the proposition
    cannot be observed. For the derived provider that evidence is a parent's
    `ResolutionFailure` certificate bound to the reference; for the relay it
    would be "the venue was upgraded" (`MAINNET_STATE_RELAY.md` §12.6 today
    folds that into the deadline walk). Lean first, in `SourceResolution.lean`
    beside `exhaust`; the theorem owed is that it cannot fire on a live parent
    (`Certified` kind is one of the two terminals), which `ParentRef.admit`
    already refuses (`admit_refuses_a_live_parent`).

- **The failure escrow's shape** is the standard one: the child's escrow
  Position holds the child's failure claims from founding (0025), and its
  explicit-failure funding compartment pays the walker. Nothing of the parents'
  escrows is touched: a parent's exhaustion refunds the *parent's* holders from
  the *parent's* Hoard, and the child's outage refunds the child's holders from
  the child's. FOUNDER-BOND (in flight) adds a lamport compartment forfeited on
  the founder's own market's exhaustion; a child founder's bond would forfeit
  on the child's exhaustion, which — the child observing nothing — can only be
  a parent's outage or a mis-sized `settle_by`. Whether a child founder should
  post a bond for a parent's oracle is BOND's question, not this note's; the
  compartments must not be shared, and L8 keeps them apart.

### 4.4 Hostiles, each against its refusal

| hostile | Lean witness | refusal | where |
|---|---|---|---|
| a child settling before a parent | `settleProduct_needs_A`, `settleProduct_needs_B`, `condition_branch_needs_B`; `Examples` rows | `parentNotTerminal` | the derived `accept`; the child stays `Primary` |
| a liveness certificate (`RecoveryAdvanced`, `Exhausted`) presented as a terminal | `admit_refuses_a_live_parent` | `parentNotTerminal` | kind check, as `authenticate_terminal_certificate` |
| a refund arm on the wrong parent outcome | `found?` refuses `condition ≥ R_A`; `conditionOnFailure` for `= R_A` | `conditionOutOfRange`, `conditionOnFailure` | founding |
| a certificate of another market | `admit_refuses_a_stranger` | `wrongParent` | the derived `accept` |
| a parent replaced after founding (same address, next generation) | `admit_refuses_a_replaced_parent` | `parentGenerationMismatch` | the derived `accept` |
| a parent whose Product record moved | `admit_refuses_a_moved_record` | `parentRecordMismatch` | the derived `accept` |
| a selector past the parent's width | `Examples` (`certificate parentA 3`) | `selectorOutOfRange` | the derived `accept` |
| a product payout vector not summing to the scale | impossible: `childPayoutVector_sum` for every selector; the terminal route's partition gate (`validate_partition`, `product_basis_terminal_v3.rs:669`) re-checks it | `InvalidPartition` | unchanged |
| a product *price* vector not summing to the scale | `Examples` off-simplex clearing | `InvalidSimplex` | JOINT-CLEARING, unchanged |
| a column bundle order | not an interval under the layout | `ShapeNotInterval` | `PlaceOrder`, JOINT-CLEARING §1.4 |
| a market crossed with itself | `Examples` | `sameParent` | founding |
| a parent retired before the child witnessed its certificate | the certificate account is closed at retirement; the derived `accept` finds no account | the child walks to failure at `settle_by` and refunds | §2.3, §4.3 |

The last row is the one physical race: retirement (`retire_v1`, L6) reclaims
the parent's accounts, and a child that has not yet cranked its `accept` loses
its evidence. The crank is permissionless and paid, so the window is the
parent's retirement delay; the honest disposition is that a child whose parent
was retired under it refunds, and the founder who founded on a parent about to
retire mis-sized `settle_by`. A parent-side reference count that blocks
retirement was considered and refused: it is a foreign write into the parent's
lifecycle by an author the parent did not name.

## 5. The properties, with proof sketches

All theorem names are in `ConditionalMarketV1.lean`; every one is proved,
none is `sorry`, and the executable witnesses are `native_decide` over the
model, not claims about SBF.

**(a) Full backing — every child claim funded by its own collateral, parents
untouched.** `childPayoutVector_sum`: for every selector — both parents
ordinary, either failed, the condition failed — the child's payout vector sums
to `ordinaryCount · unit`, the child's own scale
(`product_settlement_draws_exactly_the_child_scale`,
`conditional_settlement_draws_exactly_the_child_scale`). The payout is a
function of the selector alone (`child_payout_reads_only_the_selector`); the
parents enter through a `Nat`. Physically: the child's Hoard is its own
Custody compartment funded by its own founding and its own batches'
`Materialize`; the derived `accept` reads the parents' certificates as
read-only accounts and CPIs nothing. **Which census law:** L1 restricted to
the child's compartment; L8 with a declared delta of zero for every parent
class at every child boundary (`tools/gauntlet/journey/src/ledger.rs:1004-1012`);
L4 with zero excess on the child. The journey's new assertion is the L8 row:
*a child terminal moves no parent class*.

**(b) Consistency — marginals against the parents' prices.** Two theorems and
an honest sentence.

- *Replication off failure* (`row_bundle_replicates_the_parent_claim_off_failure`):
  whenever neither parent fails, the row bundle `(a, ·)` pays exactly `R_B`
  parent claims of `a` on every joint outcome. Sketch: both ordinary ⇒ the
  selector is a cell; the success vector is one-hot at it; summing the row's
  coordinates picks it out iff the row is `a` (`cell_injective`,
  `sumRange_indicator`); the parent's one-hot at `a` scaled by `R_B` is the
  same number (`R_A R_B unit = R_B · R_A unit`).
- *Where replication breaks* (`row_bundle_refunds_when_B_fails`): `A = a`
  ordinary and `B` failed ⇒ the row bundle refunds `R_B · unit` while `R_B`
  parent claims pay `R_B · R_A · unit`. This is the **failure premium**: a
  product price carries `P(B fails)` that the parent's does not.
- *Closed arbitrage* (`closed_arbitrage_makes_the_marginal_the_parent_read`):
  if the row bundle's clearing cost equals `R_B` parent claims' cost, then
  `rowPrice · scale_A = p^A_a · scale_C` — the two markets name one
  probability.

**Is it closed by a conjunct? No.** `A` and `A × B` are two Markets with two
books, two Hoards and two batches; no certificate of one reads the other. The
combined book — one batch clearing both Markets with the cross-market
equalities as conjuncts — was considered and refused: it needs a candidate
that mints in one Hoard against orders in another, which is exactly the
cross-class movement L8 forbids and *one Market, one batch, one complete-set
move* (BATCH-SPINE) excludes. **What a trader does:** when
`rowPrice a > R_B · p^A_a + premium`, sell the row bundle in the child (one
interval order) and buy `R_B` claims of `a` in the parent; the replication
theorem says the position pays zero in every scenario but `B`'s outage, so the
gap less the premium is riskless profit, and the two joint clearings — each
optimal for its own book (`certificate_is_optimal`) — move toward each other
by exactly the flow a trader is willing to fund. Consistency is a property of
the trader population, stated as an inequality on chain facts, and the note
does not pretend otherwise. What the *chain* does guarantee is inside one
child: rows and columns partition the same cells and sum to one simplex
(`marginals_sum_to_the_scale`), so the child cannot disagree with itself.

**(c) The decision-market property.** `P(Y | X = x)` is
`conditionalRead c s x y` — a ratio of the child's prices, exact, no parent
price — and the futarchy comparison is two rows of one vector. **The known
pathology** (Othman–Sandholm 2010, decision rules and decision markets;
Chen–Kash–Ruberry–Sundararajan on eliciting predictions for decision making;
cited from memory, the shape of the results not their constants): when the
decision `X` is *chosen by reading this market*, the conditional prices need
not be honest beliefs. Its formal root here is `off_condition_ignores_B` and
`branch_claims_pay_nothing_off_condition`: on the branch the decision does not
take, every claim pays nothing whatever it was priced at, so a trader pays
nothing in expectation for a mispricing there beyond locked capital and the
Dealer's rounding spread, and a decider who takes the branch with the higher
read can be steered by a trader who prices the *other* branch down for free.
Two bounds, then a disclosure:

- *The cost of moving a branch read.* With the scoring Dealer in every batch,
  moving the log-odds of `(x, y)` against its row by `δ` costs `b · δ` claim
  units of inventory (SCORING-DEALER §1.3, the exact slope) and the
  manipulator's expected loss is `q_x` times the mark-to-belief loss on that
  inventory, where `q_x = rowPrice x / scale` is the child's own price of the
  branch. **A branch's read is exactly as expensive to move as the branch is
  likely.** So the read on branch `x` is published beside `q_x`, and a reader
  weights it by `q_x` — a branch at `q_x → 0` is a free opinion.
- *Full support.* The literature's repair is a decision rule that takes every
  branch with positive probability and scales the score by its inverse; on
  this substrate that is the founder's choice of `X`, not the market's, and the
  market cannot enforce it.
- *The disclosure.* For the flagship (§8) the decider is not this market:
  Solana's feature activation is a validator-side act that does not read a
  devnet price. The read is then `P(Y | X)` under the traders' beliefs about a
  decision made elsewhere, and the market page says so in the same place it
  shows `q_x`. If a child is ever founded whose condition *is* decided by its
  own read, that is a different product and the page must say that too.

**(d) Settlement determinism — the child's certificate is a function of the
parents' certificates.** `settleProduct_ok_is_the_selector`: a settled value
is `productSelector` of the two admitted terminals, and `admitParent` is a
function of the certificate and the reference (`admit_ok_binds_the_reference`)
— not of the caller, the slot, the order the parents resolved in, or which
parent was cranked first. At the machine level `specialize_deterministic`
(`SourceResolution.lean:971`) and `two_admissible_observations_cannot_both_terminalize`
(`:1128`) carry it: two readers presenting the same two certificates cannot
produce two child terminals, and a second `accept` refuses. Composition with
ENSEMBLE (in flight): the child reads a *certificate*, not a source, so a
parent resolved by an ensemble median and a parent resolved by one relayed
observation are the same parent to it — which is the whole reason the child
settles from certificates and never from observations.

## 6. The compute price

**Vocabulary.** `K` is the child's outcome width (`cells + 1`), `cells` the
product's `R_A · R_B`; the prompt's `K_A · K_B ∈ {4, 9, 25}` are cells. Every
figure below is JOINT-CLEARING §3's measured or derived row re-read at the
child's width, or the terminal route's measured cost; nothing new was run.
Labels per `AGENTS.md`.

### 6.1 Walls the width meets

| wall | value | at `cells` = 4 / 9 / 25 | authority |
|---|---:|---|---|
| General bank cap `151 + 6K ≤ 512` | `K ≤ 60` | fits / fits / fits (`K` = 5 / 10 / 26); `7 × 8` fits at 57, `8 × 8` refused at 65 (`Examples`) | mathematical, `hot_v3.rs:456` via JOINT-CLEARING §3.1; `maxOutcomeCount` in the Lean |
| per-outcome heap, 384 B/outcome | `N ≈ 30` | fits / fits / fits; `6 × 6 = 37` does not until the 384 → 320 plan | measured-profile, `ITEM_OUTCOME_REGISTER_2026_09_02.md:50` |
| Structured packet on common Hot | `K ≤ 3` | not this route: a child trades native claims in the General batch; a Structured wrapper over a child inherits the wall | measured, `a621d2af6`, decision 0029 item 7 |
| chunks `⌈(2,648 + 48K) / 880⌉` | 4 / 4 / 5 | | mathematical, JOINT-CLEARING §3.1 |

### 6.2 The clearing, per transaction and per batch

Per General Hot transaction, chunked transport today ≈ `474k + chunks × ~50.2k`
plus the per-outcome slope (`~150–400 CU × outcomes × chunks`); under the D6
output page ≈ `474k + one evaluation + ~2k`. `C(N) = 2N + 2⌈N/8⌉ + 8`
transactions per batch of `N` live orders (JOINT-CLEARING §3.2).

| cells | `K` | chunks | per tx today | per tx D6 | batch `N = 2` (14 tx) | batch `N = 13` (38 tx) | `N = 13`, D6 |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 4 | 5 | 4 | ≈ 0.67 M | ≈ 0.53 M | ≈ 9.4 M | ≈ 25.5 M | ≈ 20.1 M |
| 9 | 10 | 4 | ≈ 0.67 M | ≈ 0.53 M | ≈ 9.4 M | ≈ 25.5 M | ≈ 20.1 M |
| 25 | 26 | 5 | ≈ 0.75 M | ≈ 0.53 M | ≈ 10.5 M | ≈ 28.5 M | ≈ 20.1 M |

The first two rows are JOINT-CLEARING's `K ∈ {2, 3, 5}` band to within its
stated 1 %. The third is derived: one more chunk (`+50k`) and the slope
(`5 chunks × 23 outcomes × 150–400 ≈ 17–46k`), against `OpenBatch`'s measured
K-flatness (`674,333 / 666,011 / 680,789` at `K = 2 / 13 / 258`). **No child
transaction approaches the ceiling at any width the bank clears; the price is
the transaction count, as for every market.** The Dealer's schedule row
(SCORING-DEALER §6) at the child's `K`: 92,543 measured at `K = 5`; ≈ 130k at
`K = 10` and ≈ 265k at `K = 26`, *provisional* — interpolated and
extrapolated on the measured slope of ≈ 8.2k per outcome between `K = 5` and
`K = 16`, before the shared-`Ê` and carried-potential reductions §6 names.

### 6.3 The settlement's extra reads

| step | today, measured | the child | delta |
|---|---:|---|---:|
| Resolution `accept` (the certificate's author) | Pyth `Execute` **323,836** (`GOAL.md:870`); relayed consumption **154,766** in `ProgramTest` (`MAINNET_STATE_RELAY.md:1727`) | the derived `accept`: no feed, no attestation; two 312-byte certificate reads, each one PDA derivation (`create_program_address`, ≈ 1.5k) + decode + the five comparisons of `authenticate_terminal_certificate` ≈ 3–4k each | **≈ +8k over a route with no provider decode; ≈ 150k ± 20k total**, *provisional* |
| the same, conditional off-condition branch | — | one certificate read | ≈ +4k |
| Core `AdmitTerminal` | **95,854** (cohort-13 `:1857`), **95,762** (cohort-14 `:1413`) | unchanged: reads the child's own certificate at frame index 14 | 0 |
| wallet terminal payout | **353,233** (cohort-13 `:1865`), **356,395** on both banks (`GOAL.md:3147`) | unchanged: account 25 is the child's certificate | 0 |
| provider frame accounts | 47 (Core role), 51 (Trading role) (`provider_v3.rs:37-40`) | the provider's evidence accounts leave, two certificate accounts enter: net ≈ +1, under the 64-lock wall (`PACKET_LIMIT_2026_09_01.md:197`), on the v0 transport the provider route already uses | +1 key |

Owed: the terminal payout's K-dependence at `K = 26` — the measured 353k is at
width 4, and the per-Position `SignedDeltaV3` carries `K` coordinates; bounded
above by the heap wall and unmeasured beyond `K = 5`.

## 7. What it takes to build

**Reused, unchanged.**
- The Product runtime: `ResultDomainV2` with consecutive-integer cuts,
  `select_ordinary`, the failure selector at `region_count`; the categorical
  refunding `ProductBasisV3`; the founding of both records by the compiler.
- The joint clearing (`Clearing.valid`) at the child's width; the batch
  spine's `PlaceOrder … Close`; the Dealer's schedule row.
- The escrow walk: 0025's escrow Position, `failureRefund`,
  `evaluateCategoricalFailure`, the terminal route's partition gate.
- The certificate: `ResolutionCertificateV2`, its kind-seeded PDA, and
  `authenticate_terminal_certificate` as the reference read; `AdmitTerminal`;
  the terminal child (36 accounts, certificate at 25).
- The Source machine: `specialize`, `Leg`, `accept`, the deadline walk
  `exhaust`/`commitFailure`, the explicit-failure funding compartment; the
  recovery ladder on the parents.
- The census laws L1–L8 and BATCH-SPINE's L9–L12.

**New, by file.**

| layer | change |
|---|---|
| `crates/dclutch-source-contract` | `ParentReferenceV1` record (§2.1), Lean-emitted from a `ParentReferenceV1Abi.lean` twin of `ParentRef`; `ProviderFamily::DerivedFromParentsV1` |
| `programs/dclutch-resolution-proof-sbf/src/derived_v1.rs` | the derived provider's `accept` (§4.1): two optional certificate accounts, `ParentRef.admit`'s six refusals as `DerivedProviderErrorV1` discriminants in Resolution's band, the joint index as evidence |
| `SourceResolution.lean` | the `AttestedUnobservable` conjunct on `exhaust` (§4.3) and its theorem; owed, fallback is the deadline walk |
| `crates/dclutch-product-compiler` | the child shape: `(rows, columns)` or `(branch, condition)` → the consecutive-cuts domain and the refunding basis; the `settle_by` conjunct against both parents' deadlines (`WindowBeforeParents`) |
| `programs/dclutch-core-sbf/src/found.rs` | founding reads the parents' Market state (phase Open or Terminal), generation and Product record digest, and commits the reference identity |
| `apps/dclutch-web` | `/create`: "product of" / "conditional on" pickers over two market records with the major-axis choice; the market page: row and column marginals, the conditional read as a ratio beside `q_x`, and the decision-market disclosure of §5(c) |
| journey | one L8 row per child boundary (parent classes' declared delta zero); an observation-only L13: a child certificate's `providerEvidenceId` equals `H(cert_A ∥ cert_B)` of the certificates the observer reads |
| `ConditionalMarketV1.lean` | one `import` line in `DClutchSemantics.lean`, owned by whoever next edits it (the lakefile glob already builds the module; the line is for importers and the census) |

Refusals: eleven, all named in `Refusal` in the Lean; six in the derived
provider (Resolution's band), five at founding (Core's); no new band, no new
program.

**Cohort.** No program moves under cohort-16. The child needs the joint
clearing's interval rule and the batch-keyed selection (cohort-17), and its own
records are a founding change. **Cohort-17**: the reference record and the
derived provider land Lean-first, the first child is founded on two of the
cohort's own markets (the flagship, §8), the derived `accept` is measured on
the real ELF to replace §6.3's one provisional row, and the terminal payout at
the child's width replaces the owed one.

## 8. The question for ember: the flagship child

**"If feature `X` activates by slot `S`, does mainnet's slot time move?"** —
a decision market on mainnet's own parameters, as the mainnet-state relay's
product, with both parents through the relay's four-account set and no venue
decoding at all.

- **Parent `A` — the decision.** The feature-gate account of `X` on mainnet,
  relay-attested (`MAINNET_STATE_RELAY.md` §4.1: observations, never
  interpretations). Decoding rule: the Feature program's account is
  `Option<u64> activated_at`, nine bytes — *from memory; to be pinned by the
  decoding-rules record from the feature-gate interface crate before founding*.
  Value: `activated_at` when `Some`, a sentinel above `S` when `None`, read at
  an attested mainnet `Clock` slot `≥ S` (activation is latched, so one read
  after `S` decides both cells). Cuts `[S + 1]`: two ordinary cells,
  "activated by `S`" and "not activated by `S`", plus failure. Both branches
  are *observed*, which a decision market needs and the graduation product's
  one-cell shape (§12.6) does not give.
- **Parent `B` — the metric.** Mean slot duration over a window after `S`,
  from the mainnet `Clock` sysvar — already account 4 of the relay's set (§4.2)
  — attested at two slots: `(t₂ − t₁) / (s₂ − s₁)`. Cuts at the founder's
  thresholds (e.g. `[390, 410]` ms): three cells plus failure. This is the
  relay's transport gate 1 (§6.2, *"a one-account set containing only the
  mainnet Clock sysvar"*) promoted from gate to product; its statistic is the
  scheduled median machinery's two-sample special case, or a `WindowSpec` with
  two observations.
- **The child.** `A × B`, `A` major: `2 × 3 = 6` cells, width 7; the two rows
  are `P(slot time | activated)` and `P(slot time | not activated)`, read off
  one price vector; the futarchy comparison is their difference. Or the
  conditional `B | A = activated`, width 5, which settles the moment the
  feature is seen *not* to have activated.

**What ember decides:** (i) which feature and which `S` — a real calendar the
tree cannot pick; (ii) the metric — slot time is the one that needs no venue
and demonstrates the mechanism totally, and SOL/USD through T-1 ("if `X`
activates, does SOL/USD land above `P`?") is the classic futarchy shape with
the deeper trader interest; (iii) the disclosure line of §5(c) for a decision
made by validators who do not read this market. The honest sentence about the
flagship: its economic interest is modest and its *mechanism* interest is
total — two relayed parents, one derived child, every arm of §4 exercised on
devnet, and the first conditional read that is a chain fact.

## 9. Rulings owed to ember

1. **The flagship** (§8): feature, slot, metric.
2. **`AttestedUnobservable`** (§4.3): the early exhaust on a parent's failure
   certificate, or the deadline walk alone for the first child.
3. **A child founder's bond** (§4.3): whether BOND's compartment applies to a
   founder who chose parents rather than an oracle.

## Evidence pointers

`formal/dclutch-semantics/DClutchSemantics/ConditionalMarketV1.lean` (whole);
`JointClearingV1.lean:218-224, 257, 380, 590`; `EconomicKernel.lean:812-823,
1020-1025, 1049-1055, 1080`; `ProductBasisV3.lean:165-189, 205`;
`ProductRuntimeV2.lean:384-396`; `SourceResolution.lean:225-234, 267-281,
516-545, 971, 1034, 1092, 1111, 1128, 1146`;
`SourceResolutionTerminalV2Abi.lean:186`; `SourceResolutionStateV2Abi.lean:389-412`;
`MarketCore.lean:47-59, 436-441, 625-630`; `SourceMaterialV3Abi.lean:5-15`;
`crates/dclutch-product-runtime-v2/src/lib.rs:205-213, 243-281`;
`crates/dclutch-resolution-codec/src/v2.rs:131-175`;
`crates/dclutch-resolution-codec/src/provider_v3.rs:37-40`;
`crates/dclutch-claims-svm/src/terminal_settlement_v3.rs:22-31`;
`crates/dclutch-claims-svm/src/product_basis_terminal_v3.rs:669`;
`programs/dclutch-core-sbf/src/resolution.rs:138, 1017-1049`;
`tools/gauntlet/journey/src/ledger.rs:11-57, 1004-1012`;
`docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md` §1.2, §1.4, §3, §4.4;
`docs/design/MECHANISM_BATCH_SPINE_2026_09_04.md` §2(c), §4;
`docs/design/MECHANISM_SCORING_DEALER_2026_09_04.md` §1.3, §2, §6, §7;
`docs/design/MECHANISM_FOUNDER_BOND_2026_09_04.md` (in flight) §3;
`docs/design/MAINNET_STATE_RELAY.md` §4.1-4.2, §6.2, §12.1, §12.6-12.7, :1727;
`docs/design/ITEM_OUTCOME_REGISTER_2026_09_02.md:50`;
`docs/design/PACKET_LIMIT_2026_09_01.md:197`;
`docs/decisions/0025`, `0027`, `0029` item 7; `GOAL.md:870, 3147`;
`docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md:1857, 1865`;
`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:1413`;
commit `a621d2af6`.
