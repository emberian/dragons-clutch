# `clutch-structured-claim`

This crate is the production-bound semantic core for Dragon's Clutch
transferable structured claims. It is safe Rust, `no_std`, allocation-free,
float-free, and fixed-capacity. Its sole production dependency is the
first-party `clutch-kernel`, which remains the one semantic owner of base total
claim supply, Hoard collateral, payout-set semantics, and global solvency.

It owns:

- exact reduced-rational to minimal integral coefficient realization;
- primitive nontrivial wrapper admission;
- complete-set cash compression, `p = k·1 + r`;
- byte-exact native-claim and deployment-bound wrapper-product preimages;
- flat associative composition over one native basis;
- supply-sensitive canonical/full wrap and unwind transitions;
- post-resolution canonical unwind and exact aggregate redemption;
- direct holder burns, beneficiary-free surplus donation, and retirement; and
- transactional refusal: every failed state transition leaves every input
  unchanged.

It does not own accounts, serialization, SHA-256, PDAs, Token-2022, CPI,
deployment authentication, signer authority, replay, reservation state, oracle
evidence, collateral-cap admission, or the live internal/external SupplyLedger
closure. `MarketLedger` joins wrapper identity to a complete
`clutch_kernel::MarketState`; full-vector wrapping/unwinding, donation, and
aggregate redemption invoke base-owned transitions rather than writing a
wrapper-local supply/Hoard projection. The adapter must still reconcile the
kernel's total supply with authenticated internal and external live ledgers and
apply collateral-cap checks. `MarketLedger` must never become persisted truth.

## Representation

For primitive coefficients `p`, the core derives:

```text
k   = min_i p_i
r_i = p_i - k

1 wrapper atom <-> k free Position cash atoms + r_i internal native Eggs
```

For any canonical resolved simplex vector `w`, `sum(w) = D`:

```text
k + dot(r, w)/D = dot(p, w)/D
```

The sole rounding boundary is terminal collateral atom divisibility. Direct
redemption computes the minimal exact lot `D / gcd(D, dot(p,w))` and refuses a
smaller inexact lot. Canonical unwind remains available after resolution, so
that refusal never traps ownership.

Composition never stores wrapper-under-wrapper edges. An allocation-free
accumulator adds exact native vectors, exposes any newly formed complete sets,
and either returns a primitive wrapper output or routes a constant vector to
cash. Negative legs remain funded orders, not bearer products.

## Adapter contract

The runtime adapter must reconstruct inputs from authenticated state and, on
every instruction:

1. bind Market and complete Terms identity to `NativeBasisIdentity`;
2. hash `NativeClaim::identity_preimage` and verify the live native claim id;
3. authenticate every Program/ProgramData/slot in `DeploymentBinding`, hash its
   product preimage, and verify the descriptor/mint derivations;
4. read actual extension-free Token-2022 mint supply into `WrapperState`;
5. prove holder and vault Position assets are free and canonically padded;
6. invoke one core transition on local copies;
7. perform exact base/token operations and check every post-delta; and
8. rely on transaction rollback if a CPI or postcondition fails.

The wrapper vault must never place orders. Direct Token-2022 burns are allowed:
they create locked surplus, not a fee or caller entitlement. `compact_donation`
only models the economic result; promotion still requires live base donation
instructions and bank tests.

The full runtime/account plan remains in
[`research/structured-claim-wrapper/ADAPTER_PLAN.md`](../../research/structured-claim-wrapper/ADAPTER_PLAN.md).

## Evidence and compatibility

`tests/kernel.rs` freezes the live native-claim digest and wrapper-product
digest, checks rational overflow/refusal, composition associativity, exhaustive
small-simplex payoff preservation, Active/Resolved race behavior, direct-burn
compaction, retirement, exact/inexact terminal lots, undercoverage, overflow,
and no-mutation-on-error. [`SBF_STACK_EVIDENCE.md`](SBF_STACK_EVIDENCE.md)
records isolated release-frame measurements and their deliberately narrow
claim boundary.

The crate targets the repository's Rust 2021 toolchain and has been checked
with `cargo test` and `cargo clippy --all-targets -- -D warnings`. Tests are not
a formal proof, SVM evidence, deployment evidence, or a mainnet claim.

All implementation in this crate was written for this repository under
AGPL-3.0-or-later. It contains no imported code from historical DREGG/JOSHI or
other excluded prototypes.
