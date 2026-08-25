# Decision 0002: test a Lean-owned semantic specializer

Status: experimental; no successor accepted yet.

## Decision

dClutch will test whether protocol variability can move from width-specialized
Rust control flow into canonical first-order Product, Frame, and Effect data
whose semantics are owned and checked in Lean.

The current Rust implementation remains the executable reference and is not
deleted or described as superseded. The experiment begins with one inline
ordinary Direct fill because it combines authorization-bound facts, exact quote
arithmetic, floor fees, replay, claims, collateral, and atomic rollback without
requiring a general clearing loop.

## Intended narrow waist

1. Lean declarations define admissibility and transition meaning.
2. A checked specializer emits canonical IR bytes, layouts, clients, hostile
   vectors, proof manifests, and bounded effect plans.
3. Small SBF microprograms validate frames and apply effects. Rust, if retained,
   owns only bounded decoding, memory safety, and Solana syscall/CPI adaptation.
4. Artifact-level proofs or translation validation connect the exact executor
   ELF to the Lean semantics.

No layer may maintain a second authoritative transition implementation.

## Acceptance gates

The experiment earns architectural succession only if it demonstrates all of:

- one semantic owner for Direct-fill meaning;
- canonical, round-tripping wire data emitted from that owner;
- a `no_std`, `no_alloc`, safe, fixed-layout, total executor;
- exact agreement with the existing Direct reference over generated and hostile
  cases;
- explicit refusal with whole-state rollback;
- checked conservation, replay, exact-quote, and fee theorems;
- no width monomorphization in the executor;
- a materially smaller integrated ELF and no worse practical CU envelope;
- exact pinned source, artifact, toolchain, and trust manifests; and
- an honest path from compiled ELF bytes to the theorem statements.

Failure at a gate is evidence against succession, not a reason to weaken it.

## Evidence to date

The first semantic slice, canonical codecs, concrete differential Direct test,
and isolated real-SVM execution are implemented. The specialized claim target
is 1,872 bytes and consumes 110 CU; qedsvm v0.11.0 emits a successful-path
Hoare triple over its exact ELF and Lean checks the stored theorem. A subsequent
four-program campaign composes the claim executor, controller PDA, real custody
adapter, and official SPL Token 9.0.0 ELF. The physical example commits in
24,901 CU, and a refusal after one successful token CPI restores all earlier
claim, journal, and token mutations byte-for-byte. See
`docs/evidence/LEAN_CLAIM_EXECUTOR_2026_08_25.md` and
`docs/evidence/PHYSICAL_DIRECT_COMPOSITION_2026_08_25.md` for exact source,
artifact, toolchain, rent, rollback, theorem, and limitation details.

This evidence strongly favors continuing the experiment, but does not accept a
successor. The controller still accepts pre-derived plans rather than deriving
them from authenticated signed intents, Product, Realm, and release state. The
claim path theorem is not yet composed with the high-level Direct refinement
theorem, extended to complete path coverage, or joined to an artifact theorem
for custody. The experiment therefore does not yet satisfy the full integrated
or deployed-refinement gates.

## Bounds

The semantic `ProductIR` has no N=16 restriction. Physical account, transaction,
compute, and proof limits belong to a separately measured deployment profile.
This prevents a temporary Solana profile from becoming a false mathematical
restriction.

## Provenance

The experiment is freshly authored. Neighboring Lean repositories may inform a
provenance-reviewed design comparison, but no code or dependency is imported
without a separate explicit decision.
