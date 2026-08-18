# Planned Rust crates

This directory is reserved for greenfield first-party crates. Do not add a crate
until its semantic owner, dependency direction, exact toolchain compatibility,
and license/provenance are recorded.

Proposed boundaries are listed in [the engineering plan](../docs/ENGINEERING_PLAN.md).
Eggcrate must remain `no_std`, `no_alloc`, safe Rust, fixed-layout, total, and free
of Solana, Token-2022, oracle, CPI, FFI, and dynamic-allocation dependencies.

The first implementation should be the smallest E1 falsifier, not a complete
workspace generated in advance of the Verus/SBF compatibility decision.
