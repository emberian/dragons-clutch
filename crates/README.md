# Rust crates

This directory contains three offline, dependency-free prototype crates, none
of which is verified, deployed, or a released workspace:

- `clutch-kernel` — pure `no_std` collateral-generic complete-claim transition
  kernel (split/merge, materialize/dematerialize, finite resolution, exact
  redemption).
- `clutch-accumulator` — pure `no_std` interval-summary monoid (coverage,
  extrema, exact price-time integrals, TWAP); unsupported statistics refuse.
- `clutch-batch` — pure `no_std` fixed-capacity transparent relation
  (selection, deterministic pro-rata allocation, conservation checks).

Do not add another crate until its semantic owner, dependency direction, exact
toolchain compatibility, and license/provenance are recorded.

Proposed boundaries are listed in [the engineering plan](../docs/ENGINEERING_PLAN.md).
Eggcrate must remain `no_std`, `no_alloc`, safe Rust, fixed-layout, total, and free
of Solana, Token-2022, oracle, CPI, FFI, and dynamic-allocation dependencies.

The first implementation should be the smallest E1 falsifier, not a complete
workspace generated in advance of the Verus/SBF compatibility decision.
