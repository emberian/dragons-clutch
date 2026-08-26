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
`CompiledPhysical.admitted_compilation_refines_physical_transition` proves that
those outputs select the canonical claim/custody plans, both
abstract child interpreters reach their named projections, and their atomic
join equals the high-level Direct post-state.
The canonical Effect and custody-plan decoders have general bounded
encode/decode round-trip theorems and concrete hostile refusal theorems.
`CompiledPhysical.admitted_physical_wire_round_trip` applies them to the plans
selected by any admitted frame whose outcome coordinate fits the physical V1
`u32` field.
Lean owns the compact intent and controller-instruction data structures and
proves their encodings are exactly 136 and 304 bytes.
For registered Direct intents, Lean checks registration, GTC/IOC/FOK residual
semantics, cancellation, expiry, terminal non-reuse, replay advancement, and
claim/collateral conservation. The persisted state schema is data-derived,
pairwise disjoint, and exactly 232 bytes; its hostile decoder has a general
encode/decode round-trip theorem. Lean checks that the 168-byte, ten-operation
residual program derives the exact successor remaining quantity, local replay
sequence, and phase for every semantically admitted fill.

For the Source successor model, Lean checks that the Product-owned ordered
rational domain exhausts the ordinary line, assigns each result one selector,
and derives a distinct final failure selector. The pure transition bind-checks
normalized observations to the selected provider release, admits only the next
recovery leg, refuses failure before explicit exhaustion, preserves the exact
capability-owned work-capital partition, and emits a receipt agreeing with the
post-payment funding state. Source effects retain the shared eight-byte header
and sixteen-byte record geometry under a disjoint V2 role/resource profile;
their exact decoder round trip and five-effect transition bound are checked.
The certificate schema is data-derived, pairwise disjoint, and exactly 312
bytes.

`cumulativeFee_monotone` proves monotonicity of the concrete floor-fee function,
and `cumulative_floor_fee_fragmentation_independent` combines it with the
telescoping subtraction theorem. Matcher-selected fragmentation therefore
cannot change a resting order's final cumulative fee in the semantic model.

## Not yet connected

- machine-checked refinement theorems for the safe Rust and TypeScript codecs
  (both have exact cross-language vector, round-trip, and hostile-parser tests);
- a refinement theorem from the Lean Effect/custody decoders to the independent
  safe Rust parsers and exact-account SBF parser;
- a reverse theorem that transition-program acceptance implies the semantic
  `Admissible` predicate;
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
- a proof that the Source provider adapter actually performed the selected CPI,
  Program/ProgramData, account, parser, Clock, and normalized-evidence checks;
- a refinement from the Source V2 Effect profile and 312-byte certificate to a
  physical shared executor, account-role frame, hash, or SBF artifact;
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
- global replay-root retirement at Market teardown (terminal registration
  retirement now returns rent to the persisted maker and removes buyer SPL
  delegation; prepaid creation, fill, cancellation, and expiry also have
  adversarial real-ELF execution evidence); and
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
