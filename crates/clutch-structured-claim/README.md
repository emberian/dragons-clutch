# `clutch-structured-claim`

This crate is the production-bound semantic core for Dragon's Clutch
transferable structured claims. It is safe Rust, `no_std`, allocation-free,
float-free, and fixed-capacity. It owns product algebra only; authoritative
Market supply, Hoard collateral, payout-set semantics, and global solvency
remain in their current base-account owners.

It owns:

- exact reduced-rational to minimal integral coefficient realization;
- primitive nontrivial wrapper admission;
- complete-set cash compression, `p = k·1 + r`;
- byte-exact native-claim and deployment-bound wrapper-product preimages;
- flat associative composition over one native basis;
- exact backing quantities consumed by the current account-owned lifecycle; and
- transactional refusal for coefficient and composition operations.

It does not own accounts, serialization, SHA-256, PDAs, Token-2022, CPI,
deployment authentication, signer authority, replay, reservation state, oracle
evidence, collateral-cap admission, or the live internal/external ClaimLedger
closure. The withdrawn `MarketLedger`/`StructuredClaimMachine` model has been
deleted: no wrapper-local object can act as parallel Market, supply, Hoard, or
resolution authority.

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
4. read actual extension-free Token-2022 mint supply into the current mint
   observation;
5. prove holder and vault Position assets are free and canonically padded;
6. invoke the current HoardV2/ClaimLedgerV3/PositionV3 lifecycle contract;
7. perform exact base/token operations and check every post-delta; and
8. rely on transaction rollback if a CPI or postcondition fails.

The wrapper vault must never place orders. Direct Token-2022 burns create
locked surplus, not a fee or caller entitlement. Compaction is owned by the
current base adapter's exact Hoard-to-neutral disposition, not this algebra
crate.

Current runtime and account ownership is documented by the
[runtime contract](../clutch-structured-claim-runtime-contract/README.md),
[successor adapter](../../programs/structured-claim-adapter/README.md), and
[SBF wrapper](../../programs/structured-claim-sbf/README.md). The former
research [`ADAPTER_PLAN.md`](../../research/structured-claim-wrapper/ADAPTER_PLAN.md)
is explicitly withdrawn design history.

## Evidence and compatibility

`tests/algebra.rs` freezes the native-claim identity join, checks rational
overflow/refusal, composition associativity, deployment-locus binding, and
no-mutation-on-error. [`SBF_STACK_EVIDENCE.md`](SBF_STACK_EVIDENCE.md)
records isolated release-frame measurements and their deliberately narrow
claim boundary.

The crate targets the repository's Rust 2021 toolchain and has been checked
with `cargo test` and `cargo clippy --all-targets -- -D warnings`. Tests are not
a formal proof, SVM evidence, deployment evidence, or a mainnet claim.

All implementation in this crate was written for this repository under
AGPL-3.0-or-later. It contains no imported code from historical DREGG/JOSHI or
other excluded prototypes.
