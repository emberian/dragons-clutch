# dClutch Lean semantic specializer

This experimental package asks one decisive architectural question: can Lean own
the meaning of dClutch transitions and emit compact, bounded first-order data for
a very small SBF executor?

The first slice defines one inline ordinary Direct fill independently of the
Rust implementation. It currently provides:

- `ProductIR`, `FrameIR`, and typed `EffectPlan` data;
- a width-independent Direct admission predicate;
- exact integer quote and one named floor-fee boundary;
- a seven-effect plan;
- a total checked effect interpreter;
- machine-checked claim and collateral conservation;
- gap-free replay advancement;
- whole-state rollback on refusal;
- a cumulative-fee telescoping theorem; and
- a canonical 8-byte header and fixed 16-byte Effect encoding;
- a reproducible 120-byte Lean-emitted vector; and
- executable admitted and hostile fixtures.

`dclutch-effect-kernel` is the first physical refinement target. It is safe
Rust, `no_std`, `no_alloc`, fixed-capacity, and transactionally applies the
Lean-emitted vector. It does not reimplement Direct admission. The
`dclutch-direct-contract` test suite executes that same vector beside the
current authenticated inline-ordinary reference transition and compares replay,
claims, gross collateral, and fee custody.

The isolated SBF measurement adapter executes the plan in 1,238 CU from a
12,016-byte no-allocation, size-optimized ELF. qedsvm v0.11.0 independently
executes the exact artifact at the same CU count, but its lifter refuses the
current Rust/SDK memory-alias shape; no artifact theorem is claimed. These
results establish feasibility only. The adapter's semantic-authority,
artifact-refinement, and real-custody gaps are explicit in
`docs/evidence/LEAN_EFFECT_SBF_EXPERIMENT_2026_08_25.md`.

Run:

```sh
lake build
```

This is not a formal-verification claim for the deployed Solana program. See
`TRUST.md` for the exact boundary and `docs/decisions/0002-lean-semantic-specializer.md`
at the repository root for the succession criteria.
