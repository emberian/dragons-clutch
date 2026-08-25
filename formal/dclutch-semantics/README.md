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
program's exact encoding, the admitted example, and a zero-fill refusal; the
general compiler-correctness and Rust-interpreter refinement theorems remain
open.

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
executor assigns replay and claim facts to two canonical replay roots and two
canonical maker/outcome Positions; it no longer uses the cheaper combined
pairwise projection. A real-SVM controller authenticates two native Ed25519
signatures, the canonical Market and Realm, the Market's exact capability
manifest, the manifest-selected Direct semantic release, and its finalized fee
policy. It then runs the generated transition program, composes the claim child
with a real custody adapter and official SPL Token 9.0.0, and checks
transaction-wide rollback after the first Token CPI. The earlier 1,872-byte
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
