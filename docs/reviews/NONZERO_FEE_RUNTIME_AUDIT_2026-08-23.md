# Nonzero-fee runtime audit

Status: **PURE ARITHMETIC PARTLY COMPLETE / EVERY VALUE-MOVING ROUTE STILL
ZERO-FEE / NO RATE OR TREASURY SELECTED.** Audited at
`96d70f6a0c1c37bc45bffe870ae1548f9cc999c6` on 2026-08-23.

This is a dependency and refusal audit, not a fee proposal or promotion. It
does not select either composite rate, name a treasury, relax an SBF refusal,
or authorize real-money activation. Hoard principal is never a fee or revenue
source. Future fees are never liveness capitalization. The dealer
`FeeBudgetV1` is separately prepaid facility principal and is not projected
fee revenue.

## 1. Result

The repository has substantially more exact fee arithmetic than the phrase
"runtime zero-fee" suggests, but no executable nonzero fee path:

- `clutch-batch` verifies flat-notional and composite
  `kappa*G(a,p) + kappa'*R(a)` charges with checked `u128` arithmetic, one
  owner-level carry, payer debits, and aggregate conservation. Both streaming
  engines agree with the whole relation on fee-bearing laboratory profiles.
- `clutch-batch-policy-identity` canonically encodes and hashes every
  representable composite rate pair. The only named general fee profile still
  pins `(0, 0)`; the nonzero test rates are explicitly laboratory values.
- `RevenuePolicyV1` validates and hashes the adopted 60/0/40 split and divides
  an already-settled fee exactly: maker and executor shares floor, treasury
  receives the exact residual. Its sole named treasury is the structural
  `REVENUE_TREASURY_UNSET_V1` sentinel.
- `clutch-liveness` owns a legacy signed-intent carry, terminal ceiling,
  envelope, and treasury-service counter. This audit widened its exact
  denominator, remainder, and fragment numerator from `u64` to `u128` and
  differentially checked the arithmetic against the composite quote. That
  does not make the intent the composite carry owner: dispersion is
  owner-netted and subadditive. The successor below therefore owns carry by
  `(fee record, owner)` and allocates the resulting debit across intents only
  after the owner-level quote.
- `clutch-fee-runtime-contract` now owns an account-neutral successor join:
  selected nonzero composite record, exact owner-scoped `u128` carry,
  canonical signed-envelope debit, maker/executor/treasury allocation,
  validated-settlement-only treasury credit and close guard, fee-bearing
  settlement conservation, redemption no-rake, and explicit liveness-
  capitalization refusal. Private fields make these transition outputs
  constructor-owned rather than caller-asserted. It now also has account-
  neutral inner codecs and typed identity intents for the fee record, owner
  carry, payer allocation, candidate-wide recipient allocation, treasury
  ledger, and owner-settlement join. It has no outer SBF tag, PDA, action,
  capability, rate, treasury key, or value-moving authority.
- The bridge to `clutch-owner-settlement` projects exactly one authenticated
  row per lexicographically ordered participating owner. It uses the closed
  carry's cumulative paid atoms, recomputes the terminal payer allocation,
  proves cumulative signed-envelope debits equal the carry, makes seller-only
  rows explicitly zero, requires every positive fee to fit authenticated buy
  cash reservation, and binds the exact selected-candidate fee total before
  the one recipient split.
- The Solana reservation already has `max_fee_atoms`,
  `fee_debited_atoms: u64`, and `fee_carry_numerator: u128`, but V3 validation
  requires both persisted fee-state fields to be zero. Candidate/feed state
  has no per-owner fee/rebate vector or treasury total.
- General epoch admission recognizes only the frozen zero-fee profile and the
  zero-rate composite shape. The latter then fails closed without a
  revenue-policy record and, for the only record const, at the unset treasury
  identity. Settlement never consumes that tail.

Therefore the honest claim remains: **nonzero fee computation is model/kernel
evidence; trading, settlement, withdrawal, redemption, and retirement execute
with zero protocol trading fees.**

## 2. Exact zero-restriction map

### Frozen identities and admission

| Boundary | Current restriction | What must precede relaxation |
| --- | --- | --- |
| `research/batch-policy-identity/src/direct_window_v1.rs:74` | `DIRECT_POLICY_V1` pins `FeeBaseV1::None`. | A separately named successor profile and candidate/settlement ABI; never mutate this digest. |
| `research/batch-policy-identity/src/general_clearing_v1.rs:85` | `GENERAL_CLEARING_POLICY_V1` pins `None`. | Keep this profile immutable. |
| `research/batch-policy-identity/src/general_clearing_v1.rs:122-126` | `GENERAL_CLEARING_FEE_SHAPE_V1` pins composite rates `(0, 0)`. | User-selected rates, frozen arithmetic bounds, a new sibling const, pinned bytes/digest, and hostile rate tests. |
| `programs/clutch-sbf/program/src/instructions/orders_batch/general_epoch.rs:655-681` | The artifact must equal one of those two zero-rate consts. | Register only the exact new sibling; no dynamic policy. |
| `general_epoch.rs:382-445` | Fee-shaped admission requires the per-Realm record and treasury Position, but the only revenue const names UNSET. | A real immutable treasury in a sibling revenue-policy const, new-Realm election, live treasury Position, and service-ledger byte host. |

Existing Realms without a revenue record are zero-take forever. This is the
no-silent-redirect rule, not a missing migration.

### SBF trading and settlement

The older design text counted five zero gates. The current program has more
because the general walk, entitlement, direct pair, and virtual-pot paths each
re-authenticate the signed envelope. They are intentionally repeated trust
boundaries, not redundant checks to delete.

| Path | Current zero checks |
| --- | --- |
| Direct V4 placement | `orders_batch.rs:982` requires `max_fee_atoms == 0`. |
| General walk reservation | `orders_batch/clear_walk.rs:377` re-derives the reservation with a zero fee and requires the stored envelope zero. |
| General entitlement | `orders_batch/entitlement.rs:841` rebinds every entitled touch at zero; `:2084` refuses portfolio-pair settlement if either envelope is nonzero. |
| General direct submission/settlement | `orders_batch/settlement.rs:439` validates the untouched zero envelope; `:695` refuses nonzero pair/slice envelopes. |
| General virtual split/merge | `orders_batch/settlement.rs:1290`, `:1468`, and `:1581` independently refuse nonzero envelopes on split-pay, merge-deliver, and merge-pay. |
| Legacy/direct selection | `direct_selection.rs:909-910` refuses settlement unless both envelopes are zero; `:1778` validates every reservation at zero. |

Relaxing these comparisons alone would be an authorization bug. Each
fee-bearing branch must instead bind the exact rated policy, immutable Realm
revenue policy, treasury Position and service ledger, signed worst-case
envelope, persisted carry, verified candidate fee vector, and atomic
position-conservation transition.

### Layout and pure mirrors

| Boundary | Current restriction |
| --- | --- |
| `programs/solana-layout/src/reservation.rs:426` | V3 rejects nonzero `fee_debited_atoms` or `fee_carry_numerator` as noncanonical padding. |
| `programs/solana-layout/src/portfolio_settlement.rs:630,1052` | Pair preparation and reservation validation reject nonzero envelopes. The named blocker `FeeCarryAccount` is directionally right but underspecified: composite carry is owner-scoped, so neither a standalone per-intent account nor reservation-embedded carry is admissible. A fixed owner-ledger host remains to be selected and measured. |
| `programs/solana-layout/src/direct_selection_v3.rs:1325-1326` | The frozen direct body refuses fee state because its 570-byte body has no live carry semantics. |
| `research/batch-policy-identity/src/direct_lifecycle_v3.rs:2101-2102,3562-3563` | The pure Direct V3 lifecycle mirrors the zero-envelope settlement refusal. |

The mirrored refusals must change in lockstep with a versioned runtime route.
Changing only a host mirror or only SBF would destroy the differential oracle.

### Plane-L service charges

ResolutionWork lamport charges are a different denomination and policy from
collateral-atom trading fees:

- `resolution_work.rs:394-401` freezes all five V1 protocol charges to zero;
- `:826-833` rejects any nonzero schedule shape; and
- `:1021`, `:1306`, and `:1530` reject nonzero Begin/Fold/Finalize-or-Abort
  transition charges.

The V1 zero is an adopted policy. A future optional service charge requires a
V2 schedule digest and authenticated destination; it must never be folded into
the trading-fee treasury or represented as keeper capitalization.

### Redemption, withdrawal, retirement, clients, and tests

- Internal/external redemption has no fee rake. That is correct: resolved
  claimant principal is not a revenue source.
- A treasury Position would withdraw through the ordinary owner-authorized
  cash path. No special treasury sweep exists or is needed for collateral
  atoms.
- Retirement must count the treasury Position's outstanding served epochs and
  close only after its ordinary Position liabilities are zero. The pure
  `TreasuryServiceLedger` exists; its persisted successor/account join does
  not.
- Local-real-Pyth, operator builders, the SBF harness, and most SVM trade
  fixtures submit `max_fee_atoms: 0`. The one-atom prefund hostile test proves
  refusal, not nonzero support. Static-client terms and site tickets display
  zero. These projections are untrusted and must not lead a protocol change.

## 3. Pure full-width carry correction and semantic owner

Before this audit the verifier admitted the composite denominator

```text
10_000 * price_scale^2 * 10_000
```

inside `u128`, while `IntentFeeCarry` narrowed its denominator, remainder, and
fragment numerator to `u64`. At `price_scale = 1_000_000_000` the exact
denominator is `100_000_000_000_000_000_000_000_000`, larger than
`u64::MAX`, even though it remains well inside the verifier's admitted range.
That was a real cross-kernel disagreement.

The width correction in `crates/clutch-liveness/src/lib.rs`:

- stores denominator and remainder as `u128`;
- accepts exact fragment numerators as `u128`;
- uses checked addition and checked conversion of charged atoms to `u64`;
- retains `paid_atoms` and the signed envelope as `u64`, matching collateral
  account quantities; and
- rejects numerator overflow, a quotient above `u64::MAX`, and cumulative paid
  overflow without mutating the caller's copy.

The differential test drives the selected composite laboratory profile at
price scales `10_000`, `1_000_000`, and `1_000_000_000`. For each, the batch
quote's floor, remainder, terminal ceiling, and fragmented total equal the
persistent carry's outputs exactly. No nonzero rate becomes a production
constant through this test.

Width alone was insufficient. `IntentFeeCarry` authenticates an intent, while
the composite relation explicitly quotes one owner's aggregate filled payoff
vector because `G` is subadditive. Persisting independent intent carries could
therefore overcharge a fragmented owner. `OwnerFeeCarryV1` in the successor
contract makes `(selected fee record, owner)` the semantic owner, makes its
fields private, validates restored remainder state against the relation-
derived denominator, and is the only constructor for an owner assessment.
Only after that assessment does payer allocation partition the debit across
strictly ordered signed intent envelopes.

## 4. Remaining dependency order

1. Freeze the six arithmetic/admission bounds owed by `FEE_GEOMETRY.md` and
   select exact nonzero composite rates. This is an economics decision, not a
   coding default.
2. Bind a real treasury key in a new immutable `RevenuePolicyV1` sibling and a
   new Realm. Preserve 60/0/40 unless a separately reviewed sibling changes
   it.
3. Review the account-neutral codec sizes, then allocate outer SBF tags, PDA
   seeds, rent funding, close paths, action numbers, and capability profile.
   The current 2,680-byte payer and 2,640-byte recipient snapshots are exact,
   temporary representations, not a rent/compute claim. Old account versions
   must refuse under the new profile.
4. Bind adapter-proved exhaustive selected-order ownership and standing-maker
   weights into the pure projection. The pure core now requires exact owner
   rows and uses Hamilton largest remainder, but an index or partial account
   list is not evidence of exhaustiveness.
5. Wire the pure candidate-wide settlement and treasury-ledger transition into
   atomic Position updates, proving payer debits equal maker rebates plus
   treasury credit and Hoard principal/custody remain unchanged.
6. Carry the same identities through terminal close, withdrawal, rollback,
   SBF bank tests, and a local-validator campaign. Measure compute/rent before
   capability-profile admission.

Until all six complete, the correct runtime behavior is refusal, not a
placeholder sink, mock treasury, zero-rate profile relabeled as fee-bearing,
or silent fallback to the zero-fee candidate version.
