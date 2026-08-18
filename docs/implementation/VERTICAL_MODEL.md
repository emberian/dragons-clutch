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
