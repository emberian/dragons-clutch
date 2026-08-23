# Bounded quadratic liquidity facility

Status: **EXACT RESEARCH MODEL / NO LIVE AUTHORITY**.

This crate is a dependency-free, safe Rust, `no_std`, `no_alloc`, fixed-capacity
model of a fully capitalized cost-function facility over one Dragon's Clutch
native Egg basis. It changes no kernel, account layout, SBF route, mint
authority, call-auction relation, or release claim.

It contains two deliberately separate policy models:

- the original nonnegative issuance/repurchase facility, which can create
  complete sets under its exact backing recipe; and
- [`signed_dealer`](src/signed_dealer.rs), a genuinely two-sided covered dealer
  funded by an immutable LP cash-and-existing-Egg unit basket plus a separate
  sponsor cash donation. Its API distinguishes the curve-loss subsidy minimum
  from the possibly larger deposit required to finance the all-buy corner.

The signed dealer never mints. It buys with present cash and sells only actual
custodied Eggs whose backing already remains in the Market Hoard. Its complete
economic design is
[`COVERED_SIGNED_DEALER_V1.md`](../../docs/design/COVERED_SIGNED_DEALER_V1.md).

The facility is deliberately not called an AMM. On a blockchain it cannot
promise autonomous availability, and the present runtime has no adapter for
this state. The intended execution venue remains the batch call auction. A
candidate may propose one aggregate facility inventory transition, and a
future verifier could recompute the exact endpoint receipt before any asset
moves.

## Mechanism

For nonnegative facility-attributed external Egg inventory `q`, active outcome
count `n`, exact initial simplex price `pi`, total inventory `Q`, and immutable
depth `b`, the rational potential is

```text
C(q) = dot(pi,q) + (n*sum(q_i^2) - Q^2)/(2*b*n).
```

Consensus accounting uses one named rounding boundary:

```text
C_hat(q) = ceil(C(q)).
trade cash = C_hat(q_after) - C_hat(q_before).
```

Endpoint differences telescope. Splitting a trade, replaying a path, or
expressing the same native coefficient flow through a wrapper label cannot
change aggregate facility cash. The exact rational marginal price is

```text
p_i(q) = pi_i + (n*q_i - Q)/(b*n).
```

The exact integer policy binds `pi`. The admitted inventory domain requires
every resulting price numerator to remain nonnegative; the prices then sum to
one exactly. For a uniform prior this simplifies to
`Q - n*min(q) <= b`. The boundary is real capacity exhaustion, not an adapter
preference.

The global worst-case loss is

```text
max_i(q_i) - C(q) <= b/2 * max_j ||e_j-pi||^2.
```

The sponsor deposits the ceiling of that bound before the first quote. No
future fee, Hoard principal, treasury promise, or uncapitalized insurance
enters admission.

## Physical backing

The model does not treat an inequality as custody. For external inventory `q`:

```text
H = max_i(q_i)                  facility-attributed complete sets in Hoard
r_i = H - q_i                  complement Eggs retained by the facility
F = K + C_hat(q) - H           free facility cash, outside Hoard
```

Here `K` is sponsor capital. A transition explicitly reports trader cash in or
out, complete sets to split or merge, and the new retained Egg vector. It checks
for every outcome:

```text
old_r_i + bought_i + split = new_r_i + sold_i + merge.
```

Thus the facility can deliver Eggs only by splitting collateral or using Eggs
returned by traders. Hoard principal remains claimant backing.

V1 conservatively requires every facility Egg amount to be a multiple of the
full payout denominator. That makes terminal redemption exact for every
integer simplex payout vector without a second credit ledger. It is a coarse
capacity profile, not a claim that the protocol's general fractional-credit
machinery should be removed.

## Lifecycle

```text
Trading -> BuybackOnly -> Resolved -> Retired
    |             \-----------------> Retired  (only after flat unwind)
    +------------------> Resolved              (stale phase at maturity)
```

- `Trading` admits two-sided aggregate inventory changes only in the immutable
  open/close window.
- The sponsor may halt early. Anyone may close at the frozen close slot.
- `BuybackOnly` permits only componentwise inventory reduction before maturity.
- Authenticated resolution redeems retained Eggs and preserves the payout owed
  to externally held Eggs as Hoard backing. It may also close a stale `Trading`
  phase at maturity, so a missed close transaction cannot block resolution.
- Sponsor withdrawal is possible only after resolution or a complete flat
  unwind.

This proves solvency, not transaction inclusion. A live Series must separately
prepay the callers, rent, compute, and source/resolution work needed to close
and redeem.

## Verification

```sh
cargo test --manifest-path research/bounded-liquidity-facility/Cargo.toml --release
cargo clippy --manifest-path research/bounded-liquidity-facility/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path research/bounded-liquidity-facility/Cargo.toml --no-deps
```

The issuance-facility adversarial suite includes an exhaustive small
inventory/payout domain,
exact simplex prices, the global loss bound, complete-set translation, direct
versus split execution, round trips, mixed cross-outcome flow, native wrapper
decomposition, buyback-only shutdown, vertex and graded resolution, malformed
payouts, hostile padding, inventory and price boundaries, replay overflow,
largest-domain arithmetic, cached-state mutants, and refusal atomicity.

The signed-dealer suite separately covers buy-before-sale execution, exact
negative ceilings, full mixed-corner price admission, the distinction between
loss subsidy and bid financing, exhaustive signed endpoints and payouts,
state-contingent per-LP principal floors, funding cancellation/refunds,
fixed-capacity positions, exit-queue shutdown, mixed-sign unwind, terminal
Hamilton allocation, claim-order independence, cached-custody mutants, and
rollback.

The checked arguments are in [`PROOF_ARGUMENTS.md`](PROOF_ARGUMENTS.md). The
unverified runtime boundary is in [`MODEL_BOUNDARY.md`](MODEL_BOUNDARY.md). The
economic selection and alternatives are in
[`../../docs/design/BOUNDED_LIQUIDITY_FACILITY_V2.md`](../../docs/design/BOUNDED_LIQUIDITY_FACILITY_V2.md).
