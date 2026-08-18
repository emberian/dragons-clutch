# Semantic vector spine: error taxonomy and canonical vector manifest

Status: **PROPOSED**. Every code, name, number, rule, path, and gate in this file
is a design proposal that has not crossed a review gate. Nothing here is
IMPLEMENTED, MODEL, or frozen. No file under `fixtures/` has been created, no
crate has been changed, and no root Cargo workspace has been created. This
document is the reviewable artifact required by
[`CODEX_HANDOFF.md`](../../CODEX_HANDOFF.md) §7 P1 packet 2 before any of that
work begins.

This document does not claim that any implementation currently agrees with any
other, that any executor exists, or that any property is proved. It proposes the
shared vocabulary in which such agreement could later be *stated and mechanically
checked*.

---

## 1. Purpose and the drift this is meant to stop

Dragon's Clutch has six Rust error surfaces, one Rocq shadow that returns
`option`, three Verus shadow files that are specification-only, and a promised
SBF program-test executor. [`docs/EVIDENCE_MATRIX.md`](../EVIDENCE_MATRIX.md) §7
requires that, for every semantic vector, five executors

```text
ordinary Rust reference
Verus-checked Eggcrate host execution
Rocq-extracted evaluator
optional Lean evaluator/checker
SBF program-test adapter
```

return "the same canonical success value or mapped error class". That sentence is
currently unimplementable: there is no canonical success value encoding, no error
class, and no mapping. Each surface invented its own vocabulary independently, so
the same fact is spelled `ArithmeticOverflow` in five enums, `Arithmetic` in a
sixth, and `None` in the Rocq shadow. Nothing detects when two of them drift
apart, because nothing states that they were ever supposed to agree.

The spine has two halves and one non-negotiable direction:

1. a **versioned error taxonomy** (§2) that names each distinct semantic fact
   once, with a stable number, independent of any language's enum; and
2. a **canonical vector manifest** (§3) whose records are language-neutral
   fixtures naming an initial state, an operation sequence, and one expected
   outcome expressed in taxonomy terms.

Dependency direction (§6): **vectors depend on nothing; implementations depend on
vectors.** An implementation never edits a vector to go green.

---

## 2. Error taxonomy v1 (PROPOSED)

### 2.1 Design rules

- **TAX-1 — one fact, one code.** A taxonomy code names one semantic fact, not
  one enum variant. The same fact raised by the kernel, the codec, and the batch
  relation shares one code.
- **TAX-2 — stable numbers, kebab names.** A code is an integer in a domain band
  plus a kebab-case name (`arith.overflow`). Both are stable forever. Numbers are
  never reused, renumbered, or reassigned; names are never repointed.
- **TAX-3 — `repr(u8)` discriminants are not taxonomy codes.** `clutch_kernel::Error`
  is `#[repr(u8)]` with implicit discriminants `0..=14` that shift whenever a
  variant is inserted. Those values must never be serialized into a vector. Each
  surface should gain an explicit `fn code(&self) -> u32` written against this
  table; that function is the only sanctioned mapping.
- **TAX-4 — result kind is orthogonal to the code.** A fact can be reported as an
  `Err` (`ModelError::NotMature`) or as a successful refusal value
  (`ResolveDecision::Refused(Refusal::NotMature)`). The code is the same; the
  vector's `result_kind` (`error` / `refusal`) differs and is compared strictly.
  An executor that errors where the manifest says `refusal` fails the gate.
- **TAX-5 — `by_design` flag.** A code carries `by_design: true` when the refusal
  exists because a gate is deliberately closed (missing evidence plane,
  information-theoretic boundary, unimplemented subset), not because the input is
  malformed. This preserves the repository's "do not weaken a refusal" rule as
  machine-readable data.
- **TAX-6 — coarsening is a declared relation, not a hierarchy.** Codes are flat.
  A surface whose single variant covers several facts declares
  `coarsens: [codes]` on that variant. The differential gate accepts executor
  output `X` for expected code `C` iff `X == C`, or `X` declares `coarsens`
  containing `C`. Never a sibling, never an unrelated code, never "close enough".
- **TAX-7 — reachability is declared.** Each `(surface, variant)` row declares
  `reachability: reachable | defensive-unreachable | dead`. Every `reachable` row
  must eventually own at least one vector; `defensive-unreachable` and `dead` rows
  must not, and their presence is a standing review item rather than a silent gap.
- **TAX-8 — no `Other`, no free text.** There is no catch-all code. An unmapped
  fact is a taxonomy change under review, not a runtime string.

### 2.2 Domains

| Band | Prefix | Domain | Fact family |
|---|---|---|---|
| 1000 | `arith` | arithmetic | checked overflow/underflow, exactness, representability |
| 2000 | `shape` | shape | structure, bounds, counts, canonicality, codec framing, indices |
| 3000 | `phase` | phase | lifecycle, ordering, adjacency, maturity, seal |
| 4000 | `auth` | authorization | signer, actor, ownership, keys, bumps, identity and binding |
| 5000 | `cons` | conservation | balances, solvency, closure, fills, settlement admissibility |
| 6000 | `evid` | evidence | required evidence absent, coverage, ambiguity |
| 7000 | `replay` | replay | sequence, duplicate settlement, idempotence |
| 8000 | `cap` | capacity | a frozen bound would be exceeded |
| 9000 | `refuse` | refusal-by-design | a well-formed request is deliberately out of scope |

Domain assignment follows the *primary fact*, never the reporting site. A refusal
whose fact is "the evidence does not exist" is `evid`, with `by_design: true`; the
`refuse` band is reserved for codes whose entire content is "this implementation
declines a well-formed request".

### 2.3 Code registry (PROPOSED v1)

`G` column: `x` = exact (names one fact), `c` = coarse (exists only to receive a
coarsening declaration).

#### 1000 arithmetic

| Code | Name | G | Fact |
|---|---|---|---|
| 1000 | `arith.checked-failure` | c | a checked operation failed, direction not distinguished |
| 1001 | `arith.overflow` | x | a checked addition/multiplication/cast exceeded its width |
| 1002 | `arith.underflow` | x | a checked subtraction would go below zero |
| 1003 | `arith.bucket-index-overflow` | x | a bucket index cannot advance to its exclusive end |
| 1004 | `arith.remainder-not-representable` | x | an exact quotient does not exist and rounding is refused |
| 1005 | `arith.ambiguous-denominator` | x | a denominator admits zero, so division is not conservative (`by_design`) |

#### 2000 shape

| Code | Name | G | Fact |
|---|---|---|---|
| 2010 | `shape.length-mismatch` | c | input length is not the exact fixed length |
| 2011 | `shape.truncated` | x | input ended before the fixed layout ended |
| 2012 | `shape.trailing-bytes` | x | input had bytes after the fixed layout |
| 2020 | `shape.non-canonical` | c | a reserved enum, flag, or padding field was invalid |
| 2021 | `shape.invalid-enum` | x | a reserved or unrecognized enum/flag value was present |
| 2022 | `shape.non-canonical-padding` | x | a padding or reserved slot was nonzero |
| 2030 | `shape.wrong-tag` | x | a discriminator was not the expected value |
| 2031 | `shape.wrong-version` | x | a layout/reference version is unsupported |
| 2040 | `shape.invalid-count` | x | a generic count is outside its frozen bound |
| 2041 | `shape.invalid-outcome-count` | x | outcome count is out of `MIN_OUTCOMES..=MAX_OUTCOMES` or disagrees with the payout set |
| 2042 | `shape.invalid-payout-count` | x | payout-vector count is zero or above `MAX_PAYOUTS` |
| 2043 | `shape.invalid-denominator` | x | a payout denominator is zero or not the common denominator |
| 2044 | `shape.invalid-payout-weights` | x | weights exceed the denominator, do not sum to it, or intrude into padding |
| 2045 | `shape.zero-quantity` | x | a semantic quantity that must be positive was zero |
| 2046 | `shape.zero-value` | x | an encoded field that must be nonzero was zero |
| 2047 | `shape.invalid-observation` | x | an observation interval is reversed or exceeds `MAX_VALUE` |
| 2048 | `shape.invalid-time-grid` | x | a bucket duration is zero or above `MAX_BUCKET_SECONDS` |
| 2049 | `shape.invalid-price-grid` | x | a price tick vector is empty, unsorted, or over-length |
| 2050 | `shape.invalid-tick` | x | an order's limit tick is outside the grid |
| 2051 | `shape.invalid-order-quantity` | x | order quantity is zero or below its own minimum fill |
| 2052 | `shape.invalid-minimum-fill` | x | an all-or-none order's minimum fill is not its quantity |
| 2053 | `shape.non-canonical-order-sequence` | x | order identities are zero or not strictly increasing |
| 2054 | `shape.malformed-summary` | x | a summary failed its internal consistency conditions |
| 2055 | `shape.invalid-bps` | x | a basis-point parameter is outside its frozen denominator |
| 2056 | `shape.invalid-owner-index` | x | an owner index is outside the modeled owner set |
| 2060 | `shape.index-out-of-range` | c | an index is out of range, kind not distinguished |
| 2061 | `shape.payout-index-out-of-range` | x | a payout-vector index is at or above the payout count |
| 2062 | `shape.outcome-index-out-of-range` | x | an outcome index is at or above the outcome count |
| 2063 | `shape.aliased-order-index` | x | two legs of one settlement name the same order slot |
| 2064 | `shape.order-index-out-of-range` | x | an order index is at or above the candidate length |
| 2065 | `shape.side-mismatch` | x | a leg's declared side is not the side its identity requires |
| 2070 | `shape.no-admissible-grid-tick` | x | tick selection completed with no admissible tick |

#### 3000 phase

| Code | Name | G | Fact |
|---|---|---|---|
| 3001 | `phase.not-active` | x | the market is not Active where Active is required |
| 3002 | `phase.already-resolved` | x | an Active-only transition was requested after resolution |
| 3003 | `phase.not-resolved` | x | a Resolved-only transition was requested before resolution |
| 3004 | `phase.not-mature` | x | the frozen maturity horizon has not been reached |
| 3005 | `phase.not-sealed` | x | the observation window has not been sealed |
| 3006 | `phase.already-sealed` | x | the observation window is already sealed |
| 3007 | `phase.observation-after-seal` | x | an observation was offered after the seal |
| 3008 | `phase.observation-out-of-order` | x | an observation is not the next expected bucket |
| 3009 | `phase.maturity-horizon-exceeded` | x | more observations were offered than the horizon admits |
| 3010 | `phase.non-adjacent-range` | x | the right operand does not begin at the left exclusive end |

#### 4000 authorization, identity, binding

| Code | Name | G | Fact |
|---|---|---|---|
| 4001 | `auth.missing-signature` | x | the actor presented no signature assertion |
| 4002 | `auth.unauthorized-actor` | x | the signed actor is not authorized for this action |
| 4003 | `auth.policy-unavailable` | x | no authority policy exists, so the action fails closed (`by_design`) |
| 4004 | `auth.wrong-program-owner` | x | an account is not owned by the expected program identity |
| 4005 | `auth.wrong-account-key` | x | an account key did not match its trusted binding |
| 4006 | `auth.account-alias` | x | two logical account roles share one key |
| 4007 | `auth.not-writable` | x | a state account required for a transition is not writable |
| 4008 | `auth.wrong-bump` | x | a stored bump differs from the trusted expected bump |
| 4009 | `auth.zero-identity` | x | a reserved all-zero identity was supplied |
| 4010 | `auth.non-canonical-identity` | x | an identity is not the canonical domain-separated derivation |
| 4011 | `auth.mismatched-state` | c | identities, generations, phases, or immutable fields disagree |
| 4012 | `auth.mismatched-grid` | x | two operands use different semantic grids |
| 4013 | `auth.missing-pair-bindings` | x | a candidate carries no authenticated owner/side/outcome bindings |
| 4014 | `auth.order-identity-mismatch` | x | a settlement leg names an order id the ledger does not hold at that slot |
| 4015 | `auth.self-cross` | x | both settlement legs resolve to one owner |
| 4016 | `auth.outcome-binding-mismatch` | x | the legs' outcomes disagree with each other or with the receipt |
| 4017 | `auth.unknown-candidate-identity` | x | no ledger entry exists for the receipt's domain/candidate identity |

#### 5000 conservation

| Code | Name | G | Fact |
|---|---|---|---|
| 5001 | `cons.insufficient-balance` | x | a claim balance is below the requested quantity |
| 5002 | `cons.insufficient-collateral` | x | collateral is below the requested debit |
| 5003 | `cons.invariant-violation` | x | a state fails the maximum-liability/solvency invariant |
| 5004 | `cons.conservation-failure` | x | side folds do not equal the matched quantity |
| 5005 | `cons.fill-exceeds-quantity` | x | a fill exceeds its order's quantity |
| 5006 | `cons.ineligible-fill` | x | a nonzero fill was assigned to an order the clearing tick excludes |
| 5007 | `cons.minimum-fill-violation` | x | a nonzero fill is below the order's minimum fill |
| 5008 | `cons.all-or-none-violation` | x | an all-or-none order received a partial fill |
| 5009 | `cons.candidate-mismatch` | x | a candidate differs from the recomputed canonical candidate |
| 5010 | `cons.dust-rejected` | x | an allocation requires a leftover atom under a Reject dust policy (`by_design`) |
| 5011 | `cons.aggregate-closure-mismatch` | x | internal + external claims do not equal aggregate supply |
| 5012 | `cons.non-empty-initialization` | x | an initial market carries pre-existing claims, cash, or sequence |
| 5013 | `cons.insufficient-cash` | x | a payer's cash is below the exact consideration |
| 5014 | `cons.transfer-insufficient` | x | a transferring position's claims are below the transferred quantity |
| 5015 | `cons.invalid-consideration` | x | consideration is not clearing price times quantity |
| 5016 | `cons.insufficient-liveness` | x | a liveness debit exceeds the prepaid liveness pool |
| 5017 | `cons.liveness-conservation-failure` | x | liveness buckets do not sum to the funded amount |
| 5020 | `cons.inadmissible-settlement-leg` | c | a settlement leg is not admissible against the frozen candidate |
| 5021 | `cons.settlement-quantity-exceeds-fill` | x | a leg's quantity exceeds the candidate's fill for that slot |
| 5022 | `cons.settlement-cumulative-overfill` | x | cumulative settled quantity would exceed the candidate's fill |

#### 6000 evidence

| Code | Name | G | Fact |
|---|---|---|---|
| 6001 | `evid.resolution-evidence-unavailable` | x | no typed maturity/seal/source/terms/payout evidence exists (`by_design`) |
| 6002 | `evid.no-accepted-coverage` | x | no accepted observation exists for the requested statistic |
| 6003 | `evid.gapped-coverage` | x | the range contains an explicit missing bucket |
| 6004 | `evid.ambiguous-terminal-interval` | x | the terminal interval straddles a partition boundary |

#### 7000 replay

| Code | Name | G | Fact |
|---|---|---|---|
| 7001 | `replay.sequence-mismatch` | x | a request sequence is stale, skipped, or exhausted |
| 7002 | `replay.pair-already-settled` | x | this ordered leg pair was already settled for this candidate |

#### 8000 capacity

| Code | Name | G | Fact |
|---|---|---|---|
| 8001 | `cap.order-count-exceeded` | x | an order count exceeds the frozen maximum |
| 8002 | `cap.span-too-large` | x | a summary would exceed `MAX_BUCKETS` |
| 8003 | `cap.collateral-cap-exceeded` | x | the market's immutable collateral cap would be exceeded |
| 8004 | `cap.output-buffer-too-small` | x | the destination buffer is shorter than the exact encoded length |

#### 9000 refusal by design

| Code | Name | G | Fact |
|---|---|---|---|
| 9001 | `refuse.unsupported-intent` | x | the operation is outside this deliberately small subset |
| 9002 | `refuse.unsupported-statistic` | x | the statistic needs information the summary family does not carry |
| 9003 | `refuse.missing-consideration` | x | a claim-only settlement path is refused; a cash leg is mandatory |

### 2.4 Surface mapping — every existing variant

Six Rust surfaces, 104 variants, all mapped. `R` column:
`R` reachable, `D` defensive-unreachable (guarded earlier in the same function),
`X` dead (never constructed anywhere in the tree).

#### S1 `crates/clutch-kernel::Error` (15)

| Variant | Code | R | Note |
|---|---|---|---|
| `InvalidOutcomeCount` | 2041 | R | |
| `InvalidPayoutCount` | 2042 | R | |
| `InvalidPayoutIndex` | 2060 | R | **coarse**, `coarsens: [2061, 2062]` — see R1 |
| `InvalidDenominator` | 2043 | R | |
| `InvalidPayoutWeights` | 2044 | R | also raised for nonzero weights past the active prefix |
| `ZeroQuantity` | 2045 | R | |
| `ArithmeticOverflow` | 1001 | R | reachable via `split` at `Amount::MAX` |
| `ArithmeticUnderflow` | 1002 | D | every `checked_sub` is guarded by an explicit comparison first — see R6 |
| `InsufficientBalance` | 5001 | R | |
| `InsufficientCollateral` | 5002 | R | reachable only through `merge`, and only because the collateral check precedes the balance check — see R8 |
| `NotActive` | 3001 | X | declared, never constructed; `require_active` returns `AlreadyResolved` — see R6 |
| `AlreadyResolved` | 3002 | R | |
| `NotResolved` | 3003 | R | |
| `InvariantViolation` | 5003 | R | reachable only from a caller-built state (all `MarketState` fields are `pub`) |
| `RemainderRequired` | 1004 | R | |

#### S2 `crates/clutch-accumulator` (8 + 4)

`SummaryError`:

| Variant | Code | R | Note |
|---|---|---|---|
| `InvalidGrid` | 2048 | R | *time* grid — deliberately not the same code as the batch `InvalidGrid`, see R5 |
| `InvalidObservation` | 2047 | R | |
| `BucketOverflow` | 1003 | R | |
| `SpanTooLarge` | 8002 | R | |
| `ArithmeticOverflow` | 1001 | R | |
| `MismatchedGrid` | 4012 | R | |
| `NonAdjacent` | 3010 | R | |
| `MalformedSummary` | 2054 | R | |

`StatisticError`:

| Variant | Code | R | Note |
|---|---|---|---|
| `NoAcceptedCoverage` | 6002 | R | same fact as `Refusal::NoAcceptedCoverage`, different result kind — R9 |
| `UnsupportedPredicate` | 9002 | R | same fact as `Refusal::UnsupportedStatistic`, different spelling — R9 |
| `AmbiguousDenominator` | 1005 | R | `by_design` |
| `ArithmeticOverflow` | 1001 | R | |

#### S3 `crates/clutch-batch::Error` (16)

| Variant | Code | R | Note |
|---|---|---|---|
| `InvalidGrid` | 2049 | R | *price* grid — see R5 |
| `InvalidTick` | 2050 | R | |
| `TooManyOrders` | 8001 | R | |
| `InvalidQuantity` | 2051 | R | |
| `InvalidMinimumFill` | 2052 | R | |
| `NonCanonicalOrderOrder` | 2053 | R | |
| `NonCanonicalPadding` | 2022 | R | same fact as the codec's, shared code by intent |
| `NoGridTick` | 2070 | D | `validate` already guarantees `len >= 1`, so the loop always initializes — R6 |
| `ArithmeticOverflow` | 1001 | R | also used as the "no remainder recipient selected" fallback in `allocate_side`, which is a second fact — R10 |
| `CandidateMismatch` | 5009 | R | |
| `ConservationFailure` | 5004 | R | |
| `FillExceedsQuantity` | 5005 | R | |
| `IneligibleFill` | 5006 | R | |
| `MinimumFillViolation` | 5007 | R | |
| `AllOrNoneViolation` | 5008 | R | |
| `DustRejected` | 5010 | R | `by_design` |

#### S4 `programs/solana-layout::CodecError` (12)

| Variant | Code | R | Note |
|---|---|---|---|
| `Truncated` | 2011 | R | |
| `TrailingBytes` | 2012 | R | |
| `WrongTag` | 2030 | R | |
| `WrongVersion` | 2031 | R | |
| `InvalidCount` | 2040 | R | |
| `InvalidEnum` | 2021 | R | |
| `ZeroValue` | 2046 | R | |
| `ZeroIdentity` | 4009 | R | |
| `NonCanonicalIdentity` | 4010 | R | |
| `NonCanonicalPadding` | 2022 | R | |
| `ArithmeticOverflow` | 1001 | R | |
| `OutputTooSmall` | 8004 | R | |

#### S5 `programs/solana-reference::Error` (22)

| Variant | Code | R | Note |
|---|---|---|---|
| `Layout(CodecError)` | *transparent* | R | code = the inner S4 code, `frame: "layout"` |
| `Kernel(KernelError)` | *transparent* | R | code = the inner S1 code, `frame: "kernel"` |
| `WrongLength` | 2010 | R | **coarse**, `coarsens: [2011, 2012]` |
| `WrongTag` | 2030 | R | `frame: "reference-adapter"`; distinct path from `Layout(WrongTag)` — R11 |
| `WrongVersion` | 2031 | R | `frame: "reference-adapter"` |
| `NonCanonical` | 2020 | R | **coarse**, `coarsens: [2021, 2022]` |
| `Arithmetic` | 1000 | R | **coarse**, `coarsens: [1001, 1002]` |
| `WrongProgramOwner` | 4004 | R | |
| `AccountAlias` | 4006 | R | |
| `WrongAccountKey` | 4005 | R | |
| `NotWritable` | 4007 | R | |
| `MissingSignature` | 4001 | R | |
| `UnauthorizedActor` | 4002 | R | |
| `AuthorizationUnavailable` | 4003 | R | `by_design`; only path for `CreateMarket` |
| `ResolutionEvidenceUnavailable` | 6001 | R | `by_design`; only path for `Resolve` and `RedeemInternal` |
| `WrongBump` | 4008 | R | |
| `MismatchedState` | 4011 | R | **coarse**, cross-cutting; see R3 |
| `AggregateClosureMismatch` | 5011 | R | |
| `NonEmptyInitialization` | 5012 | R | |
| `Replay` | 7001 | R | also raised for a sequence `checked_add` overflow, which is `1001` — R10 |
| `UnsupportedIntent` | 9001 | R | |
| `CollateralCap` | 8003 | R | |

#### S6 `research/vertical-model` (18 + 3 + 6)

`ModelError`:

| Variant | Code | R | Note |
|---|---|---|---|
| `Kernel(_)` | *transparent* | R | `frame: "kernel"` |
| `Summary(_)` | *transparent* | R | `frame: "accumulator"` |
| `Batch(_)` | *transparent* | R | `frame: "batch"` |
| `Accounting(_)` | *transparent* | R | `frame: "accounting"` |
| `InvalidOwner` | 2056 | R | |
| `InvalidBps` | 2055 | R | |
| `InvalidObservationOrder` | 3008 | R | |
| `ObservationAfterSeal` | 3007 | R | |
| `MaturityExceeded` | 3009 | R | |
| `NotMature` | 3004 | R | `result_kind: error` (sealing path) — R9 |
| `AlreadySealed` | 3006 | R | |
| `MissingConsideration` | 9003 | R | `by_design` |
| `MissingPairBindings` | 4013 | R | |
| `PairAlreadySettled` | 7002 | R | |
| `InvalidConsideration` | 5015 | R | also raised for a price×quantity overflow, which is `1001` — R10 |
| `InsufficientCash` | 5013 | R | also raised for a seller-cash `checked_add` overflow — R4 |
| `TransferInsufficient` | 5014 | R | also raised for a buyer-claim `checked_add` overflow — R4 |
| `InvalidFill` | 5020 | R | **coarse**, twelve facts across five domains — R2 |

`AccountingError`:

| Variant | Code | R |
|---|---|---|
| `Overflow` | 1001 | R |
| `InsufficientLiveness` | 5016 | R |
| `LivenessConservation` | 5017 | R |

`Refusal` (carried inside a successful `ResolveDecision::Refused`, `result_kind: refusal`):

| Variant | Code | R |
|---|---|---|
| `NotMature` | 3004 | R |
| `NotSealed` | 3005 | R |
| `GappedCoverage` | 6003 | R |
| `NoAcceptedCoverage` | 6002 | R |
| `AmbiguousTerminalInterval` | 6004 | R |
| `UnsupportedStatistic` | 9002 | R |

### 2.5 Review flags — genuine collisions and duplicates

These are surfaced, not papered over. Each is a decision the reviewer must make;
none is fixed by this document, and none may be closed by editing a vector.

- **R1 — `clutch_kernel::Error::InvalidPayoutIndex` names two facts.**
  `PayoutSet::get` raises it for a payout-vector index at or above `count`;
  `Position::validate_outcome` raises it for an outcome index at or above
  `outcomes`. The variant→code map is therefore not injective and a kernel vector
  cannot assert a precise code. Options: (a) split the variant into
  `InvalidPayoutIndex` / `InvalidOutcomeIndex` (preferred; codes 2061/2062 are
  reserved for exactly this), or (b) keep 2060 as a permanent coarse code and
  accept that `P-PAY-01` vectors cannot distinguish the two. Recommendation: (a),
  before any vector for `redeem`/`resolve` is frozen.
- **R2 — `ModelError::InvalidFill` names twelve facts across five domains.** It is
  raised for aliased leg indices (2063), out-of-range leg indices (2064), zero
  quantity (2045), quantity above the candidate's fill (5021), a candidate length
  above `MAX_ORDERS` (8001), an unknown candidate identity (4017), an order-id
  mismatch (4014), a side mismatch (2065), self-cross (4015), an outcome mismatch
  (4016), a `checked_add` overflow (1001), and cumulative overfill (5022). A
  cross-domain coarsening is by construction evidence of an overloaded variant.
  This one variant is the single largest anti-drift hole in the tree: three of the
  facts it hides (self-cross, cumulative overfill, outcome mismatch) are named
  P1 mechanism concerns in the handoff.
- **R3 — `solana_reference::Error::MismatchedState` is a cross-cutting coarse
  code.** It covers account-link disagreement, generation disagreement,
  lifecycle/phase disagreement, intent-to-account binding, `outcome_count` vs
  `payouts.outcomes`, and market lifecycle bounds. Acceptable for an offline
  reference; it must be split before the SVM adapter's account plane is frozen,
  because "which binding failed" is the security-relevant fact.
- **R4 — two model variants double as arithmetic reporters.**
  `InsufficientCash` is returned for a seller-cash `checked_add` overflow and
  `TransferInsufficient` for a buyer-claim `checked_add` overflow. An overflow is
  not an insufficiency. Either remap those sites to `AccountingError::Overflow`
  (code 1001) or declare both variants coarse over `{5013, 1001}` / `{5014, 1001}`.
- **R5 — `InvalidGrid` is a name collision, not a semantic duplicate.** The
  accumulator's is a *time* grid (bucket duration); the batch crate's is a *price*
  grid (tick vector). They get distinct codes 2048/2049. A naive "same name, same
  code" mapping would be silently wrong; this is precisely the drift the spine
  exists to catch.
- **R6 — dead and defensively unreachable variants.** `kernel::Error::NotActive`
  is declared and never constructed anywhere in the tree. `kernel::Error::ArithmeticUnderflow`
  is unreachable: every `checked_sub` is preceded by an explicit comparison.
  `batch::Error::NoGridTick` is unreachable: `validate` guarantees `len >= 1`, so
  the selection loop always initializes. Under TAX-7 these are declared and own no
  vectors; a reviewer should decide whether to delete `NotActive` outright.
- **R7 — kernel transitions are not transactional, and the contract is
  unstated.** `split` mutates `collateral`, `total_supply`, and `position.internal`
  and *then* returns `check_invariants()`. If that final check fails, the caller
  keeps a mutated `MarketState` and `Position` alongside an `Err`. The trailing
  check can fail in principle: `required_for_vector` accumulates up to 16 products
  of two `u64` values in a `u128`, which can overflow for extreme weights. The
  adapter (`no caller-provided output is mutated on error`) and the vertical model
  (clone/apply/commit staging) both promise the opposite. The manifest therefore
  needs an explicit `post_state_on_error` field per surface (§3.4, COMP-6), and
  the kernel should state its contract in rustdoc.
  *Resolved in code 2026-08-18 (commit d60ccf3), after this proposal was
  written: every kernel transition now completes every check, prospective
  invariant evaluation included, before its first write, and each rustdoc
  states the on-`Err`-unchanged contract. The `post_state_on_error` manifest
  field remains useful — it is what pins that contract per surface — but the
  flag's description of `split` no longer matches the landed kernel.*
- **R8 — `InsufficientCollateral` in `merge` is reachable only via check
  ordering.** `merge` tests `collateral < quantity` before it tests the per-outcome
  balances. Because weights sum to the denominator, any state passing the balance
  test already satisfies `collateral >= required >= quantity`. Reordering those
  two checks would silently make code 5002 unreachable from `merge` and change the
  observable error for a whole family of inputs. Vectors must therefore pin the
  documented check order (COMP-5), and a reviewer should decide whether that order
  is intentional.
- **R9 — same fact, two result kinds and two spellings.** `NotMature` is an `Err`
  in `seal_observations` and a successful `Refusal` in `resolve_from_summary`.
  `NoAcceptedCoverage` exists as both a `StatisticError` and a `Refusal`.
  `StatisticError::UnsupportedPredicate` and `Refusal::UnsupportedStatistic` are
  the same fact under two names. TAX-4 makes each expressible, but the
  inconsistency is a vocabulary defect worth an explicit ruling.
- **R10 — arithmetic facts hidden behind semantic variants.** `batch` returns
  `ArithmeticOverflow` when remainder-recipient selection finds no candidate (not
  an overflow); `solana-reference` returns `Replay` for a sequence `checked_add`
  overflow; `vertical-model` returns `InvalidConsideration` for a price×quantity
  overflow. Each is a distinct fact wearing the wrong name.
- **R11 — the adapter has two paths to one fact.** `Error::Layout(CodecError::WrongTag)`
  and `Error::WrongTag` both mean "wrong discriminator" at different framing
  layers. The `frame` field (§3.3) keeps them distinguishable without minting two
  codes; a vector must declare which frame it expects.

### 2.6 Rules for deliberate differences (EVIDENCE_MATRIX §7)

- **D1 — every code declares an analogue scope.** A code carries
  `expressible_in: [executor ids]`. The Rocq shadow has no account plane, so no
  4xxx account code is expressible there; the kernel has no byte plane, so no
  2011/2012 is expressible in it.
- **D2 — a vector outside an executor's scope declares it.** Such a vector sets
  that executor's disposition to `not-applicable` with a reason token from a
  closed list: `no-account-plane`, `no-byte-plane`, `no-cash-plane`,
  `no-statistic-family`, `refusal-only-evaluator`, `spec-only-shadow`.
- **D3 — declare once, reference thereafter.** A deliberate difference is argued
  once in the taxonomy, never re-argued per vector. A vector may reference a
  declaration; it may not create one.
- **D4 — an undeclared difference is a gate failure.** Not a warning, not a skip.
  Resolution is either an implementation fix or a reviewed taxonomy change — never
  a per-vector exception field, which this schema deliberately does not provide.
- **D5 — coarsening is directional and explicit.** Only the declaring surface may
  return the coarse code. A surface that returns a coarse code it never declared
  fails, even if the coarse code is "more correct".
- **D6 — refusal and error never substitute for each other.** See TAX-4.
- **D7 — `pending` is a first-class disposition with a named blocker.** Verus,
  Rocq, Lean, and SBF executors do not exist today (handoff §7 P0 blockers 2 and
  3). Their disposition is `pending` with `blocked_by` naming the blocker. A
  `pending` executor is not silently skipped: it is counted and reported, so the
  differential gate's coverage is always visible as a number.

### 2.7 Shadow and executor capability levels

| Executor id | Today | Capability | Notes |
|---|---|---|---|
| `rust-reference` | available | **exact** | the six landed surfaces; the only executor that can assert codes today |
| `verus-host` | `pending` (P0 blocker 2) | `spec-only-shadow` | `verus/*` are `spec fn`/`proof fn` with placeholder `ensures true` bodies; they evaluate nothing and can carry no vector until a Verus pin exists |
| `rocq-extracted` | `pending` (P0 blocker 2) | **refusal-only** | `rocq/ClutchKernel.v` transitions return `option state`; every refusal is `None` |
| `lean-checker` | `pending`, optional | refusal-only or exact | optional per EVIDENCE_MATRIX §6 |
| `sbf-program-test` | `pending` (P0 blocker 3) | exact + byte-exact | no entrypoint, no program-test harness exists |

The Rocq shadow deserves an explicit rule because it is the leading independent
mathematical shadow:

- **ROCQ-1.** `Some s'` compares against `result_kind: ok`; `None` compares
  against *any* declared error or refusal code. A Rocq run can therefore never
  fail a vector for the wrong reason — only for the wrong *disposition*.
- **ROCQ-2.** This is a real loss of resolution and is recorded as such: a Rocq
  agreement is evidence of refusal-boundary agreement, never of error-code
  agreement. No release sentence may imply otherwise.
- **ROCQ-3.** The path to exactness is an `error_of : state -> op -> code` total
  function in the Rocq model, returning a code from this table, plus an obligation
  that it agrees with the `option`. Proposed as future work, not assumed.
- **ROCQ-4.** The Rocq state record merges the kernel's `MarketState` and one
  `Position` into a single `state` with `st_internal`/`st_external`. The
  language-neutral kernel state form (§3.3) is shaped so both readings decode from
  it without transformation.
- **ROCQ-5 (pre-existing defect, unrelated to this schema but load-bearing for
  it).** `successful_transition_is_well_formed` states
  `forall s o, resolve s o = Some s -> ...`, binding the *input* state where the
  output was intended. The handoff already records this output-shape defect. A
  vector spine cannot repair a malformed obligation; it is named here because
  `P-SOLV-01` vectors will reference that obligation.

---

## 3. Canonical vector manifest schema v1 (PROPOSED)

### 3.1 Shape and file layout

One **vector** is one immutable record: an initial state, an operation sequence,
one expected outcome per step, and provenance. One **manifest** is an ordered
collection of vectors sharing a family, plus digests.

```text
fixtures/vectors/
  README.md                      provenance contract, extends fixtures/README.md
  TAXONOMY.json                  the §2 registry, machine-readable, versioned
  SCHEMA.json                    the JSON Schema below
  kernel/*.json                  one manifest per family
  accumulator/*.json
  batch/*.json
  layout/*.json
  adapter/*.json
  model/*.json
  cross-runtime/*.json           vectors that name more than one surface
  generators/                    generator programs and seed logs (no code yet)
```

### 3.2 Encoding rules

**Integers.**

- **INT-1 (exact integers are decimal strings).** Every value denoting a protocol
  quantity — atoms, weights, denominators, supplies, cash, order ids, bucket
  indices, sequence numbers, seeds, `u128` interval endpoints, integral numerators
  — is a JSON **string** of exact decimal digits: `"18446744073709551615"`. No
  sign, no leading zeros except `"0"`, no separators, no exponent.
- **INT-2 (bounded structural integers may be JSON numbers).** A closed exception
  list may use JSON numbers, each with a `maximum` in the schema and each bounded
  by a frozen constant: `schema_version`, `taxonomy_version`, `code`, `step`,
  `outcomes`, `count`, `len`, `page_index`, `page_count`, `payout_index`,
  `outcome`, `limit_tick`, `clearing_tick`, order/leg indices, `owner`. All are
  ≤ 65535 and therefore exact in every conforming JSON parser.
- **INT-3 (no floats, ever).** No JSON number may have a fraction or exponent
  part. No `null` stands for a number; absence is expressed by omitting the key.
- **INT-4 (ratios are never pre-divided).** An accumulator `RatioInterval` or
  `FractionInterval` ships as its exact numerator/denominator string pairs.
  Rounding is a presentation choice a vector never makes.

Rationale for INT-1: the digest rule (§3.5) canonicalizes with RFC 8785 (JCS),
whose number serialization is ECMAScript double semantics. A `u64` above 2^53 is
not representable there. Strings make the digest sound and make every reader's
parse total.

**Bytes and enums.**

- **BYTE-1.** Byte strings are lowercase hex, no `0x`, exact length `2 × n`. A
  `Hash32` is exactly 64 hex characters.
- **ENUM-1.** Every enum-valued field is a kebab-case string from a closed list
  (`"active"`, `"resolved"`, `"buy"`, `"sell"`, `"assign-canonical"`), never a
  numeric discriminant. A vector that must inject an invalid discriminant to
  trigger 2021 uses the `raw_u8` escape hatch (ENUM-2) rather than a bare number.
- **ENUM-2.** `{"raw_u8": 7}` is legal only in a vector whose expected code is
  2021/2020 and only on fields the schema marks `escapable`.

**Fixed arrays → active-prefix lists.**

- **ARR-1.** A fixed array is encoded as a JSON array whose length equals the
  declared active count: `total_supply` has exactly `outcomes` entries, `fills`
  exactly `len` entries, `orders` exactly `len` entries. Never `MAX_OUTCOMES`
  entries, never a padded tail. This is the Rocq convention verbatim —
  `rocq/ClutchKernel.v` states that its lists "represent the active prefix of the
  kernel's fixed arrays" and `state_validb` checks the exact active length.
- **ARR-2.** The inactive tail is implicitly the type's zero value
  (`0`, `PayoutVector::ZERO`, `OrderRecord::ZERO`).
- **ARR-3 (padding override).** Because ARR-1 cannot express a nonzero tail, a
  vector targeting 2022 uses
  `"padding_override": [{"field": "total_supply", "index": 5, "value": "1"}]`.
  It is legal *only* when the expected code is 2022 or a code that coarsens it,
  and executors with no padding plane declare `not-applicable` with reason
  `no-byte-plane`.
- **ARR-4.** A reader must reject a list longer than the frozen maximum, and a
  list whose length differs from the declared active count. Length disagreement is
  a manifest defect, never a vector expectation.

### 3.3 JSON Schema (draft 2020-12, PROPOSED)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://dragons-clutch.invalid/schema/vector-manifest/v1",
  "title": "dragons-clutch/vector-manifest-v1",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema", "schema_version", "taxonomy_version", "family",
               "status", "vectors", "digests"],
  "properties": {
    "schema": { "const": "dragons-clutch/vector-manifest-v1" },
    "schema_version": { "type": "integer", "minimum": 1, "maximum": 65535 },
    "taxonomy_version": { "type": "integer", "minimum": 1, "maximum": 65535 },
    "family": {
      "enum": ["kernel", "accumulator", "batch", "layout", "adapter", "model",
               "cross-runtime"]
    },
    "status": { "enum": ["proposed", "frozen", "superseded"] },
    "notes": { "type": "string" },
    "vectors": {
      "type": "array", "minItems": 1, "items": { "$ref": "#/$defs/vector" }
    },
    "digests": {
      "type": "object", "additionalProperties": false,
      "required": ["manifest"],
      "properties": {
        "manifest": { "$ref": "#/$defs/sha256" },
        "taxonomy": { "$ref": "#/$defs/sha256" }
      }
    }
  },

  "$defs": {
    "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
    "uint": { "type": "string", "pattern": "^(0|[1-9][0-9]*)$" },
    "hex": { "type": "string", "pattern": "^([0-9a-f]{2})*$" },
    "hash32": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
    "small": { "type": "integer", "minimum": 0, "maximum": 65535 },
    "vector_id": { "type": "string", "pattern": "^[a-z0-9]+(-[a-z0-9]+)*$" },
    "property_id": { "type": "string", "pattern": "^P-[A-Z]+-[0-9]{2}$" },
    "executor_id": {
      "enum": ["rust-reference", "verus-host", "rocq-extracted",
               "lean-checker", "sbf-program-test"]
    },

    "vector": {
      "type": "object",
      "additionalProperties": false,
      "required": ["id", "primary_property_id", "property_ids", "domain",
                   "surface", "status", "initial_state", "operations",
                   "provenance", "executors", "comparison", "digests"],
      "properties": {
        "id": { "$ref": "#/$defs/vector_id" },
        "title": { "type": "string" },
        "primary_property_id": { "$ref": "#/$defs/property_id" },
        "property_ids": {
          "type": "array", "minItems": 1,
          "items": { "$ref": "#/$defs/property_id" }
        },
        "domain": {
          "enum": ["arith", "shape", "phase", "auth", "cons", "evid",
                   "replay", "cap", "refuse", "success"]
        },
        "surface": {
          "enum": ["clutch-kernel", "clutch-accumulator", "clutch-batch",
                   "solana-layout", "solana-reference", "vertical-model"]
        },
        "status": { "enum": ["proposed", "frozen", "superseded"] },
        "superseded_by": { "$ref": "#/$defs/vector_id" },
        "initial_state": { "$ref": "#/$defs/state" },
        "operations": {
          "type": "array", "minItems": 1,
          "items": { "$ref": "#/$defs/step" }
        },
        "final_state": { "$ref": "#/$defs/state" },
        "provenance": { "$ref": "#/$defs/provenance" },
        "executors": {
          "type": "object",
          "minProperties": 5,
          "additionalProperties": false,
          "patternProperties": {
            "^(rust-reference|verus-host|rocq-extracted|lean-checker|sbf-program-test)$": {
              "$ref": "#/$defs/disposition"
            }
          }
        },
        "comparison": { "$ref": "#/$defs/comparison" },
        "digests": {
          "type": "object", "additionalProperties": false,
          "required": ["vector"],
          "properties": {
            "vector": { "$ref": "#/$defs/sha256" },
            "expected_bytes": { "$ref": "#/$defs/sha256" }
          }
        }
      }
    },

    "state": {
      "type": "object",
      "required": ["form", "constructed_by", "value"],
      "additionalProperties": false,
      "properties": {
        "form": {
          "enum": ["kernel.market-position/v1", "accumulator.summary/v1",
                   "batch.book/v1", "layout.accounts/v1",
                   "adapter.reference-transition/v1", "model.vertical/v1"]
        },
        "constructed_by": {
          "description": "raw-fields is required whenever the state is not reachable through the surface's own constructors; it must be declared, never inferred.",
          "enum": ["constructor", "raw-fields", "operation-sequence"]
        },
        "value": { "type": "object" },
        "padding_override": {
          "type": "array",
          "items": {
            "type": "object", "additionalProperties": false,
            "required": ["field", "index", "value"],
            "properties": {
              "field": { "type": "string" },
              "index": { "$ref": "#/$defs/small" },
              "value": { "$ref": "#/$defs/uint" }
            }
          }
        }
      }
    },

    "step": {
      "type": "object",
      "additionalProperties": false,
      "required": ["step", "op", "expect"],
      "properties": {
        "step": { "$ref": "#/$defs/small" },
        "op": { "type": "string", "pattern": "^[a-z0-9]+(_[a-z0-9]+)*$" },
        "args": { "type": "object" },
        "expect": { "$ref": "#/$defs/outcome" },
        "post_state": { "$ref": "#/$defs/state" }
      }
    },

    "outcome": {
      "oneOf": [
        {
          "type": "object", "additionalProperties": false,
          "required": ["result_kind"],
          "properties": {
            "result_kind": { "const": "ok" },
            "value": {
              "description": "The canonical success value, if the operation returns one (a redeemed payout, a verified candidate). Absent means unit."
            }
          }
        },
        {
          "type": "object", "additionalProperties": false,
          "required": ["result_kind", "code", "name"],
          "properties": {
            "result_kind": { "enum": ["error", "refusal"] },
            "code": { "type": "integer", "minimum": 1000, "maximum": 9999 },
            "name": { "type": "string", "pattern": "^[a-z]+\\.[a-z0-9-]+$" },
            "frame": {
              "enum": ["kernel", "accumulator", "batch", "accounting",
                       "layout", "reference-adapter", "vertical-model"]
            },
            "by_design": { "type": "boolean" }
          }
        }
      ]
    },

    "provenance": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind"],
      "oneOf": [
        {
          "properties": {
            "kind": { "const": "handwritten" },
            "source": { "type": "string" },
            "rationale": { "type": "string" }
          },
          "required": ["kind", "source"]
        },
        {
          "properties": {
            "kind": { "const": "generated" },
            "generator": { "type": "string" },
            "generator_version": { "type": "string" },
            "seed": { "type": "string", "pattern": "^[0-9a-f]{16}$" },
            "reproduction_command": { "type": "string" },
            "minimized_from": { "$ref": "#/$defs/vector_id" }
          },
          "required": ["kind", "generator", "generator_version", "seed",
                       "reproduction_command"]
        }
      ]
    },

    "disposition": {
      "type": "object",
      "additionalProperties": false,
      "required": ["mode"],
      "properties": {
        "mode": {
          "enum": ["exact", "coarsened", "refusal-only", "not-applicable",
                   "pending"]
        },
        "coarsens_to": { "type": "integer", "minimum": 1000, "maximum": 9999 },
        "reason": {
          "enum": ["no-account-plane", "no-byte-plane", "no-cash-plane",
                   "no-statistic-family", "refusal-only-evaluator",
                   "spec-only-shadow"]
        },
        "blocked_by": { "type": "string" }
      }
    },

    "comparison": {
      "type": "object",
      "additionalProperties": false,
      "required": ["semantic", "byte_exact", "post_state_on_error"],
      "properties": {
        "semantic": { "const": true },
        "byte_exact": { "enum": ["required", "optional", "not-applicable"] },
        "byte_artifacts": {
          "type": "array",
          "items": {
            "type": "object", "additionalProperties": false,
            "required": ["role", "digest"],
            "properties": {
              "role": { "type": "string" },
              "length": { "$ref": "#/$defs/small" },
              "digest": { "$ref": "#/$defs/sha256" },
              "bytes": { "$ref": "#/$defs/hex" }
            }
          }
        },
        "post_state_on_error": {
          "enum": ["unchanged", "as-declared", "unspecified"]
        },
        "single_fault": { "type": "boolean" },
        "precedence_note": { "type": "string" }
      }
    }
  }
}
```

### 3.4 Comparison rules

- **COMP-1 — semantic comparison is always required.** Two runs agree when their
  decoded active-prefix values and their outcome (`result_kind` + `code` under
  TAX-6) are equal. In-memory layout, field order, padding bytes, and enum
  discriminants are never compared semantically.
- **COMP-2 — byte-exact comparison is narrow and named.** It applies only where a
  codec owns the bytes: `solana-layout` `encode` outputs, `solana-reference`
  `TransitionOutput` account bytes, and the vertical model's golden trace lines.
  Each byte artifact is named by `role` with a length and a SHA-256; inline `bytes`
  are optional and are provenance, not the comparison key.
- **COMP-3 — executors without a byte plane declare `not-applicable`.** Never
  "skipped". A vector requiring byte-exactness of an executor that has no bytes is
  a manifest defect.
- **COMP-4 — every executor has a disposition.** `executors` requires all five
  keys. Coverage is therefore always a printable ratio (today: 1 exact, 4
  pending), and "the gate passed" can never mean "one executor ran".
- **COMP-5 — single-fault discipline.** A refusal vector must be constructed so
  exactly one fault applies. When two faults necessarily coexist, the vector sets
  `single_fault: false` and must give a `precedence_note` citing the owning
  implementation's documented check order. R8 is the motivating case: the
  observable code depends on the order of two checks inside `merge`.
- **COMP-6 — post-state on error is declared, not assumed.** `unchanged` for the
  reference adapter and the vertical model (both stage and commit); `as-declared`
  with an explicit `post_state` for `clutch-kernel`, which mutates before its final
  invariant check (R7); `unspecified` is permitted only with a review note and is
  a standing defect, not a resting state.
- **COMP-7 — the success value is part of the vector.** `redeem_internal` returns
  a payout; `verify` returns the candidate. Agreeing on "Ok" is not agreement.
- **COMP-8 — refusal ≠ error (TAX-4), and coarsening is one-directional (TAX-6).**

### 3.5 Digests and provenance

- **DIG-1.** `digests.vector` = SHA-256 over the RFC 8785 (JCS) canonical JSON of
  the vector object with its own `digests` member removed.
- **DIG-2.** `digests.manifest` = SHA-256 over the JCS canonical JSON of the
  manifest with its own `digests` member removed. This is the value that
  `EVIDENCE_MATRIX.md` §3 calls `vector_manifest_digest` in the artifact ledger.
- **DIG-3.** `digests.taxonomy` binds the manifest to an exact `TAXONOMY.json`
  byte image. A manifest whose taxonomy digest does not match the checked-out
  taxonomy fails before any executor runs.
- **DIG-4.** Generated vectors must be reproducible from
  `generator@generator_version --seed <seed>` alone, per EVIDENCE_MATRIX §7.
  Minimized adversarial failures become permanent named fixtures carrying
  `minimized_from`.
- **DIG-5.** Placeholder digests (`pending-*`, empty strings) are legal only in
  this PROPOSED document. The review gate (§7) rejects any manifest containing one.

### 3.6 Versioning and migration

- **VER-1.** `taxonomy_version` and `schema_version` are independent monotone
  integers. A manifest may not mix taxonomy versions.
- **VER-2.** Adding a code is a taxonomy minor change. Changing the *meaning* of
  an existing code is **forbidden**, without exception.
- **VER-3.** Retiring a code sets `status: retired` and `superseded_by`; the
  number is never reused.
- **VER-4.** Splitting a coarse code (R1's `2060`, R2's `5020`) mints new codes
  and marks the old one `status: coarse-retired` with `split_into: [codes]`.
  Existing vectors keep resolving through `split_into` until they are re-frozen.
- **VER-5.** Vectors are append-only and immutable once `frozen`. Changing an
  expectation means minting a new `id` and marking the old one `superseded` with
  `superseded_by`. A frozen vector's bytes never change, so its digest never
  changes, so an artifact-ledger record stays meaningful forever.
- **VER-6.** A schema change that could reject an existing frozen manifest
  requires a `schema_version` bump plus a migration note enumerating the affected
  vector ids.
- **VER-7.** Migration is data, not code archaeology: `TAXONOMY.json` carries the
  full retirement/split history so an old manifest is always interpretable.
- **VER-8.** The taxonomy and this schema are frozen together or not at all. A
  vector that references a code the pinned taxonomy does not define fails to load.

---

## 4. Worked examples (PROPOSED, hand-written)

All three are consistent with the current implementations. Digest fields are
`pending-generator` placeholders per DIG-5; a real manifest must carry computed
values.

### 4.1 `kernel-binary-split-resolve-redeem-exact`

Kernel success: a binary one-hot market, split 11 atoms, resolve to payout 1,
redeem 10 internal claims of outcome 1 for exactly 10 atoms. Follows
`clutch-kernel`'s `resolution_is_finite_and_redemption_is_exact` semantics.

```json
{
  "id": "kernel-binary-split-resolve-redeem-exact",
  "title": "complete split, finite resolution, exact one-hot redemption",
  "primary_property_id": "P-SOLV-01",
  "property_ids": ["P-SOLV-01", "P-PAY-01", "P-SUP-01"],
  "domain": "success",
  "surface": "clutch-kernel",
  "status": "proposed",
  "initial_state": {
    "form": "kernel.market-position/v1",
    "constructed_by": "constructor",
    "value": {
      "outcomes": 2,
      "phase": "active",
      "resolved_payout": 0,
      "collateral": "0",
      "total_supply": ["0", "0"],
      "payouts": {
        "count": 2,
        "outcomes": 2,
        "vectors": [
          { "denominator": "1", "weights": ["1", "0"] },
          { "denominator": "1", "weights": ["0", "1"] }
        ]
      },
      "position": { "internal": ["0", "0"], "external": ["0", "0"] }
    }
  },
  "operations": [
    {
      "step": 0,
      "op": "split",
      "args": { "quantity": "11" },
      "expect": { "result_kind": "ok" },
      "post_state": {
        "form": "kernel.market-position/v1",
        "constructed_by": "operation-sequence",
        "value": {
          "outcomes": 2, "phase": "active", "resolved_payout": 0,
          "collateral": "11",
          "total_supply": ["11", "11"],
          "payouts": {
            "count": 2, "outcomes": 2,
            "vectors": [
              { "denominator": "1", "weights": ["1", "0"] },
              { "denominator": "1", "weights": ["0", "1"] }
            ]
          },
          "position": { "internal": ["11", "11"], "external": ["0", "0"] }
        }
      }
    },
    {
      "step": 1,
      "op": "resolve",
      "args": { "payout_index": 1 },
      "expect": { "result_kind": "ok" }
    },
    {
      "step": 2,
      "op": "redeem_internal",
      "args": { "outcome": 1, "quantity": "10" },
      "expect": { "result_kind": "ok", "value": { "payout": "10" } }
    }
  ],
  "final_state": {
    "form": "kernel.market-position/v1",
    "constructed_by": "operation-sequence",
    "value": {
      "outcomes": 2,
      "phase": "resolved",
      "resolved_payout": 1,
      "collateral": "1",
      "total_supply": ["11", "1"],
      "payouts": {
        "count": 2, "outcomes": 2,
        "vectors": [
          { "denominator": "1", "weights": ["1", "0"] },
          { "denominator": "1", "weights": ["0", "1"] }
        ]
      },
      "position": { "internal": ["11", "1"], "external": ["0", "0"] }
    }
  },
  "provenance": {
    "kind": "handwritten",
    "source": "docs/implementation/VECTOR_SPINE_PROPOSAL.md#41",
    "rationale": "Smallest trace that exercises complete-set split, finite resolution, and an exact integral redemption whose maximum-liability requirement drops from 11 to 1."
  },
  "executors": {
    "rust-reference": { "mode": "exact" },
    "verus-host": { "mode": "pending", "reason": "spec-only-shadow",
                    "blocked_by": "CODEX_HANDOFF.md#7-P0-2" },
    "rocq-extracted": { "mode": "refusal-only",
                        "reason": "refusal-only-evaluator",
                        "blocked_by": "CODEX_HANDOFF.md#7-P0-2" },
    "lean-checker": { "mode": "pending", "blocked_by": "optional per EVIDENCE_MATRIX.md#6" },
    "sbf-program-test": { "mode": "pending", "blocked_by": "CODEX_HANDOFF.md#7-P0-3" }
  },
  "comparison": {
    "semantic": true,
    "byte_exact": "not-applicable",
    "post_state_on_error": "unchanged",
    "single_fault": true
  },
  "digests": { "vector": "pending-generator" }
}
```

Notes carried by this vector:

- `total_supply[0]` stays at `11` after redemption because outcome 0 carries
  weight 0 under the resolved vector; the maximum-liability requirement is
  `ceil((11·0 + 1·1)/1) = 1`, which the remaining collateral of `1` exactly meets.
  A vector that "tidied" `total_supply[0]` to `0` would encode a supply leak.
- The Rocq shadow decodes this state directly: `st_internal`/`st_external` are the
  `position` lists, and every list is already an active prefix of length 2
  (ARR-1/ROCQ-4). Its disposition is `refusal-only`, so it checks `Some`, not the
  payout — the payout equality is `rust-reference`'s to assert until ROCQ-3 exists.

### 4.2 `batch-ineligible-fill-below-clearing-tick`

Batch refusal: a forged candidate moves a fill onto a buy order whose limit tick
is below the clearing tick. `clutch-batch::verify` refuses with `IneligibleFill`
(code 5006). Grid `[10, 20, 30]`, `remainder_seed = 7`, dust `assign-canonical`.
At tick 2 the eligible buy total is 10 (order 1 only) and the sell total is 10, so
the canonical candidate is `fills = [10, 0, 10]`.

```json
{
  "id": "batch-ineligible-fill-below-clearing-tick",
  "title": "a nonzero fill on an order the clearing tick excludes is refused",
  "primary_property_id": "P-BATCH-01",
  "property_ids": ["P-BATCH-01", "P-BATCH-02"],
  "domain": "cons",
  "surface": "clutch-batch",
  "status": "proposed",
  "initial_state": {
    "form": "batch.book/v1",
    "constructed_by": "constructor",
    "value": {
      "policy": {
        "grid": { "ticks": ["10", "20", "30"], "len": 3 },
        "tie_rule": "max-volume-min-imbalance-highest-tick",
        "dust_policy": "assign-canonical",
        "remainder_seed": "7"
      },
      "len": 3,
      "orders": [
        { "canonical_order_id": "1", "side": "buy",  "limit_tick": 2,
          "quantity": "10", "minimum_fill": "1", "partial_policy": "allow" },
        { "canonical_order_id": "2", "side": "buy",  "limit_tick": 0,
          "quantity": "10", "minimum_fill": "1", "partial_policy": "allow" },
        { "canonical_order_id": "3", "side": "sell", "limit_tick": 2,
          "quantity": "10", "minimum_fill": "1", "partial_policy": "allow" }
      ]
    }
  },
  "operations": [
    {
      "step": 0,
      "op": "propose",
      "expect": {
        "result_kind": "ok",
        "value": { "clearing_tick": 2, "len": 3,
                   "matched": "10", "fills": ["10", "0", "10"] }
      }
    },
    {
      "step": 1,
      "op": "verify",
      "args": {
        "candidate": { "clearing_tick": 2, "len": 3,
                       "matched": "10", "fills": ["0", "10", "10"] }
      },
      "expect": {
        "result_kind": "error",
        "code": 5006,
        "name": "cons.ineligible-fill",
        "frame": "batch"
      }
    }
  ],
  "provenance": {
    "kind": "handwritten",
    "source": "docs/implementation/VECTOR_SPINE_PROPOSAL.md#42",
    "rationale": "Aggregate side totals are preserved by the forgery, so conservation alone cannot catch it; eligibility must be checked per order. Single-fault: the forged vector satisfies matched, length, tick, minimum-fill, and all-or-none checks."
  },
  "executors": {
    "rust-reference": { "mode": "exact" },
    "verus-host": { "mode": "pending", "reason": "spec-only-shadow",
                    "blocked_by": "CODEX_HANDOFF.md#7-P0-2" },
    "rocq-extracted": { "mode": "not-applicable",
                        "reason": "no-statistic-family",
                        "blocked_by": "no Rocq batch model exists" },
    "lean-checker": { "mode": "pending",
                      "blocked_by": "EVIDENCE_MATRIX.md#6 names BatchRelationV1 as the first Lean seam" },
    "sbf-program-test": { "mode": "not-applicable", "reason": "no-account-plane" }
  },
  "comparison": {
    "semantic": true,
    "byte_exact": "not-applicable",
    "post_state_on_error": "unchanged",
    "single_fault": true,
    "precedence_note": "verify checks candidate shape, matched, then per-order fills; the eligibility test precedes the canonical-allocation equality test, so this input yields 5006 rather than 5009."
  },
  "digests": { "vector": "pending-generator" }
}
```

Note the precedence declaration is load-bearing: the same forged candidate would
also fail the later canonical-allocation equality check with `CandidateMismatch`
(5009). Under COMP-5 the vector must say which check owns the input. If the
implementation ever reorders those checks, this vector fails — which is the
intended behavior, not a flake.

### 4.3 `adapter-materialize-breaks-aggregate-closure`

Adapter refusal: a forged position claims one internal atom of outcome 0 that the
aggregate supply does not carry. `apply` runs `validate_aggregate_closure` before
the transition and refuses with `AggregateClosureMismatch` (code 5011). No caller
output is mutated.

Identities are the real domain-separated derivations for
`profile = canonical_profile_hash(b"fixture-profile")`, `realm_nonce = 7`,
`market_nonce = 9`:

| Role | Value |
|---|---|
| profile | `f5b58a12ef8eefe67a7db5413d376667adad488910378310d52230cf2981af8c` |
| realm | `936d10778b3872af23a19ec4658739bac2fdce0093c0c1f43f6bd7071e9647cc` |
| market | `86a37c886dd8df637fc379977bd9851f278d36104859a7873e884b8e30327c9a` |
| outcome 0 | `344579950d28abe5f30f9ead60f8309d164893a11365ac4065aac599a99da306` |
| outcome 1 | `267a59df3d33b31132b8c6c47bc2e8ff7eadae2e7be5214c94d52d25d43aff20` |
| owner | `1f1f…1f` (32 × `0x1f`) |

```json
{
  "id": "adapter-materialize-breaks-aggregate-closure",
  "title": "a forged position cannot materialize claims absent from aggregate supply",
  "primary_property_id": "P-SUP-01",
  "property_ids": ["P-SUP-01", "P-SOLV-01", "P-POOL-01"],
  "domain": "cons",
  "surface": "solana-reference",
  "status": "proposed",
  "initial_state": {
    "form": "adapter.reference-transition/v1",
    "constructed_by": "raw-fields",
    "value": {
      "bindings": {
        "program_id": "3232323232323232323232323232323232323232323232323232323232323232",
        "market":   "3333333333333333333333333333333333333333333333333333333333333333",
        "hoard":    "3434343434343434343434343434343434343434343434343434343434343434",
        "position": "3535353535353535353535353535353535353535353535353535353535353535",
        "kernel":   "3636363636363636363636363636363636363636363636363636363636363636",
        "external": "3737373737373737373737373737373737373737373737373737373737373737",
        "replay":   "3838383838383838383838383838383838383838383838383838383838383838",
        "market_bump": 3, "hoard_bump": 4, "position_bump": 5,
        "external_bump": 6, "replay_bump": 7
      },
      "metadata": {
        "all_accounts": { "owner_program": "3232…32", "writable": true },
        "actor": {
          "key": "1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f",
          "signer": true
        }
      },
      "accounts": {
        "market": {
          "market": "86a37c886dd8df637fc379977bd9851f278d36104859a7873e884b8e30327c9a",
          "realm":  "936d10778b3872af23a19ec4658739bac2fdce0093c0c1f43f6bd7071e9647cc",
          "profile":"f5b58a12ef8eefe67a7db5413d376667adad488910378310d52230cf2981af8c",
          "terms":  "0808080808080808080808080808080808080808080808080808080808080808",
          "outcome_count": 2,
          "lifecycle": "active",
          "stored_bump": 3,
          "hoard_bump": 4,
          "outcomes": [
            "344579950d28abe5f30f9ead60f8309d164893a11365ac4065aac599a99da306",
            "267a59df3d33b31132b8c6c47bc2e8ff7eadae2e7be5214c94d52d25d43aff20"
          ],
          "feed": "0909090909090909090909090909090909090909090909090909090909090909",
          "collateral_cap": "1000",
          "created_slot": "55"
        },
        "hoard": { "collateral_atoms": "0", "stored_bump": 4, "flags": 0 },
        "position": {
          "owner": "1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f",
          "generation": "2",
          "internal": ["1", "0"],
          "cash_atoms": "100",
          "reserved_cash_atoms": "7",
          "stored_bump": 5,
          "close_state": "open"
        },
        "kernel": {
          "phase": "active",
          "resolved_payout": 0,
          "payouts": {
            "count": 2, "outcomes": 2,
            "vectors": [
              { "denominator": "1", "weights": ["1", "0"] },
              { "denominator": "1", "weights": ["0", "1"] }
            ]
          },
          "total_supply": ["0", "0"]
        },
        "external": {
          "position_generation": "2",
          "balances": ["0", "0"],
          "stored_bump": 6,
          "flags": 0
        },
        "replay": {
          "position_generation": "2",
          "sequence": "0",
          "stored_bump": 7,
          "flags": 0
        }
      }
    }
  },
  "operations": [
    {
      "step": 0,
      "op": "apply",
      "args": {
        "request": {
          "sequence": "0",
          "action": "layout",
          "intent": {
            "kind": "materialize",
            "market": "86a37c886dd8df637fc379977bd9851f278d36104859a7873e884b8e30327c9a",
            "owner":  "1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f",
            "destination": "3737373737373737373737373737373737373737373737373737373737373737",
            "outcome": 0,
            "quantity": "1"
          }
        }
      },
      "expect": {
        "result_kind": "error",
        "code": 5011,
        "name": "cons.aggregate-closure-mismatch",
        "frame": "reference-adapter"
      }
    }
  ],
  "provenance": {
    "kind": "handwritten",
    "source": "docs/implementation/VECTOR_SPINE_PROPOSAL.md#43",
    "rationale": "The closed single-position reference model must refuse before the kernel runs: internal[0] + external[0] = 1 but aggregate total_supply[0] = 0. Metadata, bindings, bumps, links, padding, replay sequence, and signature are all valid, so closure is the only fault."
  },
  "executors": {
    "rust-reference": { "mode": "exact" },
    "verus-host": { "mode": "not-applicable", "reason": "no-account-plane" },
    "rocq-extracted": { "mode": "not-applicable", "reason": "no-account-plane" },
    "lean-checker": { "mode": "not-applicable", "reason": "no-account-plane" },
    "sbf-program-test": { "mode": "pending",
                          "blocked_by": "CODEX_HANDOFF.md#7-P0-3" }
  },
  "comparison": {
    "semantic": true,
    "byte_exact": "not-applicable",
    "post_state_on_error": "unchanged",
    "single_fault": true,
    "precedence_note": "validate_metadata, then Request::decode, then account decode, then validate_links, validate_padding, validate_aggregate_closure, then the replay sequence check. Closure precedes replay, so a stale sequence would not mask this fault."
  },
  "digests": { "vector": "pending-generator" }
}
```

This vector is the canonical illustration of D1/D2: code 5011 is expressible only
in the account plane, so three of five executors are `not-applicable` **by
declaration**, and the coverage report reads "1 exact, 1 pending, 3 not-applicable"
rather than "passed". A successful run of this vector proves nothing about Rocq or
Verus, and the manifest says so in machine-readable form.

---

## 5. What this spine does not do

- It is not a proof, a refinement, or a translation validation. Agreement across
  executors on a finite vector set is agreement on that finite set. It says
  nothing about all inputs.
- It cannot repair an overloaded variant (R1–R4), a malformed proof obligation
  (ROCQ-5), or a missing executor (P0 blockers 2 and 3). It makes each visible and
  countable.
- A green differential gate with four `pending` executors is one Rust
  implementation agreeing with itself. The `executors` map exists so that this
  fact can never be rounded up in a summary.

---

## 6. Ownership and dependency direction

- **Owner.** The semantic-vector owner owns `fixtures/vectors/**` and
  `fixtures/generators/**` exclusively. No other packet may write there. This
  mirrors the handoff's "one persisted fact, one semantic owner" rule: the
  expected outcome of a named scenario is a persisted fact, and it is owned here
  rather than duplicated in six test modules.
- **Location.** `fixtures/vectors/` as laid out in §3.1, under the existing
  [`fixtures/README.md`](../../fixtures/README.md) contract: synthetic by default,
  canonical, reproducible, with a provenance manifest, and no wallet material,
  secrets, credentials, or unlicensed copied inputs.
- **Direction — vectors depend on nothing.** `fixtures/vectors/` contains data
  only. It has no Cargo manifest, no build script, and no dependency on any crate.
  Every implementation depends on the vectors; no vector depends on an
  implementation. If reading a vector required building a crate, the vector would
  no longer be language-neutral and the Rocq/Lean/SBF executors could not use it.
- **Readers are test-only and separate.** A proposed `tools/clutch-vectors/`
  host-only, `std`, dev-dependency crate parses manifests and drives the
  `rust-reference` executor. The four semantic crates remain `no_std`,
  allocator-free, and dependency-free; they gain at most a `[dev-dependencies]`
  edge, never a library-target edge. Rocq/Lean/SBF readers are separate programs
  reading the same JSON.
- **Anti-drift rule (non-negotiable).** An implementation may never edit a vector
  to make a test pass. This is the same rule as AGENTS.md's "Do not weaken a
  refusal to make an integration test pass." A disagreement is triaged as either
  an implementation defect or a vector defect; a vector defect mints a **new**
  vector id and records the review (VER-5). Vectors are never silently corrected.
- **Ledger binding.** `digests.manifest` is the `vector_manifest_digest` field of
  the EVIDENCE_MATRIX §3 artifact ledger record. A property's ledger row without a
  manifest digest is incomplete.

---

## 7. Review gate before any root Cargo workspace exists

The handoff's P1 packet 2 says a root workspace may not be created until this
schema and its dependency direction are reviewed. Proposed gate items — all human
decisions, none of which this document may make for the reviewer:

- **G1 — taxonomy shape.** Accept or amend the nine domains, the code bands, and
  the flat-codes-plus-coarsening-relation design (TAX-6). A hierarchy was
  considered and rejected because `ModelError::InvalidFill` coarsens across five
  domains; a tree could not express it without lying.
- **G2 — rulings on R1–R11.** In particular: split `InvalidPayoutIndex` (R1),
  decide the fate of `InvalidFill` (R2), delete or keep `NotActive` (R6), and
  state the kernel's post-error mutation contract (R7). Each has a decided default
  proposed above; none is applied.
- **G3 — encoding rules.** Accept INT-1 (exact integers as decimal strings) and
  the closed INT-2 exception list, the active-prefix convention (ARR-1) and its
  `padding_override` escape hatch (ARR-3), and JCS+SHA-256 as the digest rule.
- **G4 — comparison and disposition rules.** Accept COMP-1…COMP-8, the five
  executor ids, and the requirement that all five carry a disposition on every
  vector (COMP-4). This is what keeps "the gate passed" from meaning "one
  executor ran".
- **G5 — ownership and direction.** Accept `fixtures/vectors/` as data-only with
  no Cargo manifest, and the test-only reader crate. Confirm no semantic crate
  gains a library-target dependency.
- **G6 — workspace decision, separately.** Only after G1–G5: decide whether a root
  workspace is created at all. The handoff's current stance (independent manifests,
  offline, locked) is a deliberate isolation property, and a shared lockfile is a
  change to that property, not a convenience. If a workspace is created, the
  vector data directory stays outside it.
- **G7 — bootstrap scope.** Approve the first vector set before it is written:
  one success and one refusal per reachable code, kernel family first, and no
  frozen manifest containing a placeholder digest (DIG-5).

Until G1–G7 are decided, this document is the artifact; there are no files under
`fixtures/vectors/`, no reader crate, and no root workspace.
