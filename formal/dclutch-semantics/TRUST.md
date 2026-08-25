# Trust boundary

## Machine-checked in this package

For an `Admissible` inline ordinary Direct frame, Lean 4 checks that:

- the emitted plan has seven effects;
- the small effect interpreter produces the specified post-state;
- selected-outcome claims are conserved;
- collateral across buyer, seller, and venue is conserved;
- both replay nonces advance exactly once;
- every post-state field remains in `u64` range;
- quote equality is exact at the Product-owned scale;
- the fee uses the named floor boundary; and
- a refused frame exposes the unchanged pre-state.

Lean also checks the general encoded length equations for V1 headers, effect
records, and plans, plus the exact 120-byte semantic-plan and 72-byte physical
claim-plan encodings of the Direct example. It checks the concrete account
offsets, privilege words, replay/Position state tags, instruction length, and four effect
tags emitted for the exact-account claim executor. That concrete check assumes
the named loader-v1 serialization formula.
Lean also checks that the multiprogram physical plan's four replay/claim effects
and two indivisible custody transfers execute to projections that join to the
same Direct post-state, and that the custody projection conserves collateral.
The semantic intent binds the canonical Market and exact maker-accepted fee
rate; the authenticated Market/manifest graph owns semantic-release and
fee-policy selection. Lean checks the transition program's 35-instruction
shape, 568-byte encoding, and zero-fill refusal. For every high-level
`Admissible` frame,
`DirectProgram.admitted_program_refines` proves that the abstract transition VM
accepts and derives exactly the semantic successor nonces, gross quote, and
named floor fee.
Lean owns the compact intent and controller-instruction data structures and
proves their encodings are exactly 136 and 304 bytes.

`cumulativeFee_monotone` proves monotonicity of the concrete floor-fee function,
and `cumulative_floor_fee_fragmentation_independent` combines it with the
telescoping subtraction theorem. Matcher-selected fragmentation therefore
cannot change a resting order's final cumulative fee in the semantic model.

## Not yet connected

- machine-checked refinement theorems for the safe Rust and TypeScript codecs
  (both have exact cross-language vector, round-trip, and hostile-parser tests);
- a reverse theorem that transition-program acceptance implies the semantic
  `Admissible` predicate, plus composition from the derived output registers to
  the unique physical claim and custody plans;
- a machine-checked refinement from Lean's transition VM to the safe Rust
  `dclutch-transition-vm` interpreter (cross-language exact-vector, hostile-
  bytecode, hostile-frame, rollback, and integer-boundary tests exist);
- a proof of native Ed25519 instruction authenticity (the real-SVM campaign
  separately executes a two-signature native batch and a tampered-message
  refusal);
- a proof of Solana account ownership, signer/writable flags, PDA derivation,
  CPI, sysvars, Token/Token-2022 semantics, rent, and transaction rollback
  (the exact claim, controller, custody, and official SPL Token ELFs now provide
  adversarial real-SVM evidence for the first account/PDA/CPI/rollback slice);
- a machine-checked refinement from the Lean effect interpreter to the Rust
  microkernel (`dclutch-effect-kernel` currently supplies cross-language vector,
  round-trip, execution, hostile-parser, late-rollback, and one concrete
  differential Direct-reference test only);
- a composition theorem from Direct's high-level `effectPlan_refines_transition`
  through the projection codec to the exact machine theorem;
- an implementation-level proof that real Solana CPI sequencing has the
  abstract `atomicCommit` behavior (transaction rollback is still a separately
  tested runtime property);
- whole-CFG artifact coverage (qedsvm v0.11.0 checks one successful path of the
  superseded combined-projection claim artifact; the canonical four-account
  owner model and the general Rust/SDK executor remain outside its alias model);
- compute-unit, stack, ELF-size, and rent measurements for a complete successor
  (claim, signed experimental controller, custody, and official SPL Token are
  measured together; Realm selection and release-authentication costs remain
  absent);
- all Direct routes other than inline ordinary execution; and
- all other protocol families.

The package uses Lean 4.30.0. No theorem contains `sorry`, an axiom, an
`external_body`, or an assumed specification. Lean's kernel, toolchain, native
code used by `native_decide`, hardware, and operating system remain trusted for
the build. `native_decide` is used only for concrete regression examples, not
for the general conservation and refinement theorems.

## Provenance

The implementation is freshly authored for dClutch from the protocol invariants
recorded in this repository. It does not import, copy, or depend on leanuweave,
minidregg, breadstuffs, Dragon's Clutch, or another historical DREGG codebase.
