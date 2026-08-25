# dClutch Lean semantic specializer

This experimental package asks one decisive architectural question: can Lean own
the meaning of dClutch transitions and emit compact, bounded first-order data for
a very small SBF executor?

The first slice defines one inline ordinary Direct fill independently of the
Rust implementation. It currently provides:

- `ProductIR`, `FrameIR`, and typed `EffectPlan` data;
- a width-independent Direct admission predicate;
- a signed canonical Market identity and exact accepted fee rate;
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

The first Source successor specialization is also Lean-owned. It authenticates
normalized provider observations against every immutable release coordinate,
uses Product's ordered exact-rational result partition and derived final failure
outcome, advances only the next finite recovery leg, and consumes only
capability-owned prepaid work. Successful transitions emit at most five
fixed-width V2 Effect records plus one cursor-specialized 312-byte certificate.
Lean checks mapping boundedness/disjointness, ordinary/failure separation,
determinism, early-failure refusal, immediate recovery ordering, funding
conservation, exact receipt accounting, and rollback projection. Provider CPI,
Solana account/Clock authentication, physical execution, and certificate
identity hashing remain unverified adapter boundaries.

The successor physical plan now derives a four-effect replay/claim program and
two indivisible Realm-collateral transfers from that same admitted frame. Lean
checks each child plan, custody conservation, exact recomposition to the Direct
post-state, and an abstract all-or-nothing commit envelope. The 72-byte claim
and 40-byte custody vectors are generated from these definitions; they are not
handwritten parallel transaction schemas.

The next specialization layer compiles Direct admission into one canonical
568-byte `DCTV` program: 35 fixed instructions over 41 scalar and four abstract
identity registers. The program checks the admission relations and derives the
gross quote, floor fee, and successor nonces rather than accepting those outputs
as caller assertions. `dclutch-transition-vm` is the safe, `no_std`, `no_alloc`,
fixed-memory Rust interpreter for that bytecode. Lean currently checks the
program's exact encoding and proves `admitted_program_refines`: every
high-level `Admissible` frame executes successfully and derives exactly its
semantic successor nonces, gross quote, and named floor fee. Concrete admitted
and zero-fill fixtures remain regression checks. Reverse acceptance/refusal
completeness, physical-register decoding, and Rust-interpreter refinement
remain open.

Registered Direct intents now have the same semantic-to-byte path. Registration
consumes the maker nonce once; one persistent state owns the exact signed intent,
controller authority, maker, phase, remaining quantity, and registration-local
sequence. Lean proves GTC residual reuse, IOC residual cancellation, FOK
exact-fill behavior, maker cancellation, permissionless expiry, terminal
non-reuse, cumulative-fill bounds, and conservation. Its 232-byte state layout
is cursor-specialized from field data, pairwise disjoint, and has a general
hostile-decoder round-trip theorem. Lean also emits the exact 168-byte generic
VM program that derives successor remaining, sequence, and phase values. The
safe Rust codec consumes both generated artifacts and refuses unknown phases,
malformed nested intents, nonzero reserved bytes, truncation, and alternate
magic/version values. `RegisteredPhysical` executes that generated program for
both authenticated registrations, joins the results to the sole Position
balances, and proves exact claim conservation. The 20,568-byte claim-owner ELF
now dispatches this 16-byte request profile alongside inline execution; its
real-ELF campaign covers reusable and terminal fills plus hostile rollback.
The 32-byte controller request now drives that child route and real SPL custody
without repeating maker signatures. Registration account creation plus
controller register/cancel/expire dispatch remain the next physical boundary.

`CompiledPhysical.compilePhysicalPlan` then constructs the claim and custody
plans from successful program outputs instead of caller-supplied gross, fee, or
successor nonces. `admitted_compilation_refines_physical_transition` proves
that every admitted compilation selects the canonical plans, both plan
interpreters produce their named projections, and their abstract atomic join is
the one semantic Direct post-state. Lean's hostile V1 decoders now satisfy
general bounded encode/decode round-trip theorems for Effect and custody plans;
`admitted_physical_wire_round_trip` instantiates them for every admitted Direct
frame within the separately named physical `u32` outcome-coordinate profile.

Lean also owns the exact data structures and encodings for the 136-byte compact
intent and 304-byte controller instruction. Their exact lengths are theorems. A
maker key is deliberately not duplicated in the signed intent: the native
Ed25519 public key is its semantic owner. The signed identity is the canonical
Market itself, so the obsolete 136-byte execution-profile ABI has been deleted.
Lean emits three exact ABI vectors; both the safe, `no_std`, `no_alloc` Rust
codec and the frontend TypeScript codec match them byte-for-byte. The Rust codec
is shared by the controller, operator, and SVM harness. Cross-language
parser-refinement theorems remain open.

`dclutch-effect-kernel` is the first physical refinement target. It is safe
Rust, `no_std`, `no_alloc`, fixed-capacity, and transactionally applies the
Lean-emitted vector. It does not reimplement Direct admission. The
`dclutch-direct-contract` test suite executes that same vector beside the
current authenticated inline-ordinary reference transition and compares replay,
claims, gross collateral, and fee custody.

The general SDK/no-allocation measurement adapter remains a seven-effect
baseline: 1,238 CU from a 12,016-byte ELF. The active Lean-generated claim
executor assigns inline replay facts to canonical maker replay roots,
registered replay facts to canonical registration-local sequences, and all
claim balances to canonical maker/outcome Positions; it no longer uses the
cheaper combined pairwise projection. A real-SVM controller authenticates two
native Ed25519 signatures, the canonical Market and Realm, the Market's exact
capability manifest, the manifest-selected Direct semantic release, and its
finalized fee policy. It then runs the generated transition program, composes
the claim child with a real custody adapter and official SPL Token 9.0.0, and
checks transaction-wide rollback after the first Token CPI. The earlier 1,872-byte
claim target has one qedsvm v0.11.0 successful-path Hoare triple, but that
theorem does not cover the canonical-owner successor artifact. This remains
runtime evidence plus high-level Lean theorems—not whole-CFG refinement or a
checked-release proof for the controller ELF. Current hashes and boundaries are
in
`docs/evidence/COMPILED_SIGNED_DIRECT_2026_08_25.md`; the earlier
`LEAN_CLAIM_EXECUTOR_2026_08_25.md` and
`PHYSICAL_DIRECT_COMPOSITION_2026_08_25.md` reports remain historical evidence
for their named artifacts.

Run:

```sh
lake build
```

This is not a formal-verification claim for the deployed Solana program. See
`TRUST.md` for the exact boundary and `docs/decisions/0002-lean-semantic-specializer.md`
at the repository root for the succession criteria.
