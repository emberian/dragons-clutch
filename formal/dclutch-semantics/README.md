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

The successor physical plan now derives a four-effect replay/claim program and
two indivisible Realm-collateral transfers from that same admitted frame. Lean
checks each child plan, custody conservation, exact recomposition to the Direct
post-state, and an abstract all-or-nothing commit envelope. The 72-byte claim
and 40-byte custody vectors are generated from these definitions; they are not
handwritten parallel transaction schemas.

`dclutch-effect-kernel` is the first physical refinement target. It is safe
Rust, `no_std`, `no_alloc`, fixed-capacity, and transactionally applies the
Lean-emitted vector. It does not reimplement Direct admission. The
`dclutch-direct-contract` test suite executes that same vector beside the
current authenticated inline-ordinary reference transition and compares replay,
claims, gross collateral, and fee custody.

The general SDK/no-allocation measurement adapter remains a seven-effect
baseline: 1,238 CU from a 12,016-byte ELF. The active Lean-profile-generated
claim executor is narrower and physical: four effects, 110 CU, and a 1,872-byte
ELF. qedsvm v0.11.0 lifts its 119-instruction successful path into a
kernel-checked Lean Hoare triple without proof-term rewriting. A real-SVM
controller-PDA relay composes with that child and demonstrates runtime rollback
across both programs after a late child refusal. This remains one path theorem
plus runtime evidence—not whole-CFG, high-level refinement, release
authentication, or custody evidence.
Exact hashes and boundaries are in
`docs/evidence/LEAN_CLAIM_EXECUTOR_2026_08_25.md`.

Run:

```sh
lake build
```

This is not a formal-verification claim for the deployed Solana program. See
`TRUST.md` for the exact boundary and `docs/decisions/0002-lean-semantic-specializer.md`
at the repository root for the succession criteria.
