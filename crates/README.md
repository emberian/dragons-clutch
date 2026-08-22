# Rust crates

This directory contains six offline, dependency-free `no_std` crates
(updated 2026-08-22; none is a deployment or release claim —
`CURRENT_TRUTH.md` supersedes status language here):

- `clutch-kernel` — pure `no_std` collateral-generic complete-claim transition
  kernel (split/merge, materialize/dematerialize, finite resolution, exact
  redemption).
- `clutch-accumulator` — pure `no_std` interval-summary monoid (coverage,
  extrema, exact price-time integrals, TWAP); unsupported statistics refuse.
- `clutch-batch` — pure `no_std` fixed-capacity transparent relation
  (selection, deterministic pro-rata allocation, conservation checks).
- `clutch-bspline` — pure `no_std` exact degree-zero through degree-three
  open-clamped payout-basis evaluator. It owns basis evaluation only; evidence
  authentication and account/runtime binding remain adapter obligations.
- `clutch-bspline-accumulator` — joins the basis evaluator to the interval
  accumulator for windowed smooth-claim evidence.
- `clutch-liveness` — the host-side liveness/fee-carry kernels
  (`IntentFeeCarry`, `TreasuryServiceLedger`) backing the liveness policy
  profile and the revenue seams.

New crates record their semantic owner, dependency direction, toolchain
compatibility, and license/provenance at introduction (each README does).

Proposed boundaries are listed in [the engineering plan](../docs/ENGINEERING_PLAN.md).
Eggcrate must remain `no_std`, `no_alloc`, safe Rust, fixed-layout, total, and free
of Solana, Token-2022, oracle, CPI, FFI, and dynamic-allocation dependencies.

The first implementation should be the smallest E1 falsifier, not a complete
workspace generated in advance of the Verus/SBF compatibility decision.
