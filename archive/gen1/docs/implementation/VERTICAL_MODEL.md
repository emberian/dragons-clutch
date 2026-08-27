# Offline vertical model

`research/vertical-model` is a deterministic host-only integration/reference
simulator for the landed `clutch-kernel`, `clutch-accumulator`, and
`clutch-batch` crates. Its `Cargo.toml` uses local path dependencies; it does
not edit or copy those crates and is intentionally outside any root workspace.

Run it with:

```text
cargo test --manifest-path research/vertical-model/Cargo.toml
cargo run --manifest-path research/vertical-model/Cargo.toml
```

The fixture composes market creation, internal complete-set split, materialize
and dematerialize boundaries, fixed-grid batch proposal/verification, interval
observation folding, deterministic resolution, internal and external redemption,
batch-fill settlement, and merge. `golden/basic.trace` is an exact event trace;
no timestamps, random values, RPC data, keys, signing, or deployment are
involved.

Resolution is explicitly gated by a frozen three-bucket maturity horizon and a
separate seal operation. A complete-looking prefix is refused as `NotMature`,
and a mature but unsealed window is refused as `NotSealed`; observations are
frozen after sealing. Batch settlement keeps a cumulative per-candidate,
per-order ledger, so partial fills such as `3 + 2 + 1` cannot exceed a verified
order fill of `5`, and replaying the same candidate remains idempotent.

Each candidate ledger is keyed by an explicit `BatchDomain` tuple containing
market, book, epoch, policy, and canonical order-set identities, plus the full
verified candidate. Bound books additionally retain each order's side, owner,
and outcome. Settlement is one atomic matched-pair receipt consuming both
canonical buy and sell order identities exactly once; reversed pairs, party
swaps, cross-outcome legs, and replay are refused. The legacy claim-only call
refuses with `MissingConsideration`, and unbound generic books refuse with
`MissingPairBindings`. The accepted path moves buyer cash to seller cash and
claims in the opposite direction, with joint cash/claim conservation checks.
Cross-book, cross-epoch, candidate-collision, wrong-order, and missing-
consideration cases are regression-tested.

Settlement and other mutating model transitions execute on a cloned staged
state and commit only after the staged conservation check succeeds. A refusal
therefore preserves the complete model state, including claims, cash, ledger,
trace, and accounting fields.

The standalone `Accounting` value applies the same copy-validate-commit rule to
its public liveness mutators, including refusal from a deliberately corrupted
prestate.

This does not supply authenticated observation authority or an adapter Resolve
authority; those remain a separate refusal boundary outside this offline model.

The model keeps three accounting domains distinct:

* `principal` mirrors kernel Hoard collateral only;
* `fee_revenue` is charged from verified batch volume and is not collateral;
* `liveness_reserved`, `liveness_paid`, and `liveness_returned` are a separately
  conserved prepaid-work bucket.

Adversarial tests cover tampered partial fills, duplicate/crashed observations
and settlement retries, out-of-order summary joins, explicit missing coverage,
ambiguous terminal intervals, unsupported path predicates, and attempted
liveness overspend. Successful transitions run aggregate claim-supply and
kernel invariant checks.

This is a fixture and reference composition, not a Solana adapter, deployment,
RPC execution, key/signing path, formal-verification result, or financial
readiness claim. The batch crate's candidate remains the best valid submitted
candidate under its frozen relation; the model makes no optimality claim.

## Coupled clearing path (added 2026-08-18)

The model now carries two clearing entry points, and they are not the same
object. The **scalar** path (`clear_batch`, `clear_batch_with_bindings`,
`settle_batch_fill_with_consideration`) drives `clutch_batch::FixedBook`, whose
relation clears one grid tick over side totals with owner and outcome erased;
there the model, not the relation, decides which buy faces which sell. It is
retained unchanged as a permanent regression lab and `golden/basic.trace` is
never rewritten. The **coupled** path (`clear_relation_v1`,
`settle_relation_receipt`) drives `clutch_batch::relation_v1`, whose relation
binds every fill to `(owner, outcome, side)` and emits a frozen
`PairingWitnessV1`. Its trace is `golden/coupled.trace`, a second trace beside
the scalar one. Both paths move claims through
`clutch_kernel::MarketState::transfer_internal` with the phase policy named at
the call site, and both stage every mutating transition on a clone committed
only after the conservation check passes.

Clearing lifts the model's `BatchDomain` into a `RelationDomainV1` through
`proposed_relation_domain`. All eleven policy families are selected at the one
construction site `proposed_relation_policy`, each with its reason stated
beside it, and every one of them is PROPOSED — this is a fixture, and no
selection here is canonized. The candidate comes from
`relation::propose_best_valid` over the bounded coordinate box
`PROPOSED_SEARCH_BOUNDS`; it is the best valid submitted candidate of that
window, never an optimum. The model then reconstructs the pairing witness with
`canonical_pairing` and re-verifies candidate and witness (`verify`,
`verify_pairing_witness`) instead of reading a claimed aggregate. Two unequal
candidates arriving under one host identity are refused as
`RelationIdentityCollision` rather than resolved; clearing an already-cleared
candidate is an idempotent replay with no second fee, no second liveness
charge, and no second trace event.

Settlement derives from the frozen pairing slices and from nothing else. A
`RelationReceipt` binds the full candidate identity, both canonical order
identifiers and their book indices, the outcome, the exact quantity, and the
exact cash consideration in price units, plus a `SettlementTarget` naming what
it draws on; the target kind has no default and is never inferred.
`relation_settlement_plan` computes every draw against an immutable ledger
before the first write, so a refusal cannot leave a partial write behind. The
slice universe is exactly the frozen decomposition, the receipt's own bound
coordinates must be the ones that decomposition names, and the cumulative
per-slice and per-order ledgers ceiling every draw at what the verified
candidate filled. The model keeps no pairing opinion on this path. Receipt
replay is refused with the full prestate preserved, and all six orderings of a
three-slice settlement reach the same state and the same trace multiset.

The bounded box's `max_imbalance: 0` is load bearing rather than incidental:
this host model does not carry the virtual split/merge pot of
`BATCH_RELATION_V1_DESIGN.md` §14.3, so a candidate that would create or
destroy complete sets, and any frozen slice naming a virtual split or merge
leg, is refused as `VirtualLegNotHosted` before anything is charged, never
stranded silently.

The frozen residual-pair variant is read from the policy the candidate's digest
already binds, so no call site can select a different one after the fact:

* **1a** `FullPairOnly` — a receipt names a slice and must consume it whole,
  exactly once. A short quantity refuses `PartialPairRefused` and a second
  draw refuses `PairAlreadySettled`.
* **1b-canonical** `CumulativePairCanonical` — a receipt names an executable
  pair by its own bound order indices and outcome, aggregated over every frozen
  slice carrying that pair, and draws any quantity those slices still hold.
  Over-drawing refuses `ExceedsPairRemaining`; a pair the decomposition never
  emitted refuses `UnknownPair`. The pair universe is exactly the frozen
  slices, so 1b never admits a pair the relation did not emit.
* **1c** `UniqueSliceReceipts` — a receipt names a slice and draws any
  quantity up to that slice's residue. Over-drawing refuses `SliceExceeded`,
  and an index the decomposition does not have refuses `UnknownSlice`.

Under all three, a receipt of the wrong target kind refuses
`SettlementTargetNotAdmitted`, and a receipt whose consideration is not
`quantity * prices[outcome]` refuses `InvalidConsideration`. The fourth
variant, `CumulativePairFree` (1b-free), is refused at *clear* time as
`ResidualVariantUnimplemented`, before any fee or liveness charge: its
documented strand hazard needs a terminal sweep authority this model does not
have, and clearing a batch it could not settle would be the charge-then-refuse
shape this path exists to make inexpressible.

Fees are charged only on pairable volume. `accepted_volume` is the relation's
own recomputed direct flow summed over outcomes, every atom of which the frozen
decomposition pairs to two distinct bound owners, and it is the only quantity
the model charges fee or liveness against. The relation fee base is
deliberately `FeeBaseV1::None`, so the fee stays the model's own basis-point
charge into `fee_revenue` and there is one fee owner. Every admission,
feasibility, conservation, and pairing refusal happens before the first charge,
and cleared volume always settles: the frozen slice quantities sum to
`accepted_volume` under each variant. The contrast against the scalar lab is
executed at the model boundary (adversarial review §P1-B): on a buy bound to
outcome 0 against a sell bound to outcome 1, the scalar path erases outcome,
matches volume 3, charges fee 1, and then refuses the settlement receipt for
that volume as `InvalidFill`; the coupled path has no per-outcome conservation
solution for that book, so it clears volume 0, fee 0, zero slices, and charges
nothing.

Two deviations are flagged rather than settled, and both are recorded in
`BATCH_RELATION_V1_DESIGN.md` §18:

* the landed golden trace is `golden/coupled.trace`, not the
  `golden/relation_v1.trace` named in that document's §14.3 checklist. Whether
  to accept the landed name or rename is an open decision;
* `RoundingBoundaryV1::TerminalOwnerFloor` (R-b) is the selected boundary and
  its price-unit rounding pot is carried per ledger as
  `rounding_pot_price_units`, but this model settles cash in exact price units
  and never draws on it. The price-unit to collateral-atom conversion boundary
  is therefore recorded, not exercised, on this path; the pot is kept in the
  ledger so the boundary stays visible instead of implicit.

Run the coupled trace with:

```text
cargo run --manifest-path research/vertical-model/Cargo.toml -- coupled
```

From the crate directory, `cargo run -- coupled | cmp - golden/coupled.trace`
is the exact pin check; `coupled_golden_trace_is_stable` makes the same
comparison in the suite and also re-asserts `golden/basic.trace`, which nothing
on the coupled path rewrites.

This section adds a second fixture path, not a claim upgrade. The coupled
relation's candidate is the best valid submitted candidate under its frozen
relation and the model makes no optimality claim; every policy family named
above remains PROPOSED and unpromoted; and the evidence for this path is
executed fixtures and falsifiers, not a formal-verification result.
