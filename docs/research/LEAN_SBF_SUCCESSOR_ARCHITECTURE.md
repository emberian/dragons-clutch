# Lean/SBF successor architecture

> The initial size measurements in this note are historical. The active signed,
> compiled Direct slice and its larger but non-fragmentable canonical state model
> are pinned in
> [`../evidence/COMPILED_SIGNED_DIRECT_2026_08_25.md`](../evidence/COMPILED_SIGNED_DIRECT_2026_08_25.md).

Status: active experiment; not an accepted replacement.

## Why this direction exists

The current integrated SBF artifact is verifier-clean, but its 9.77 MB binary
requires about 68.01 SOL of permanent Loader V3 capitalization under the local
default Rent profile. Repeated Rust control flow, width-specialized transitions,
account adapters, provider handling, and every capability are linked into one
artifact.

The general SDK/no-allocation Effect executor is 12,016 bytes, requires about
0.086 SOL of equivalent Loader V3 capitalization, and executes its seven-effect
fixture in 1,238 CU. The successor generated claim executor is 1,872 bytes,
requires about 0.015 SOL, and executes its four-effect plan in 110 CU. The
current physical experiment adds a 17,760-byte controller and 24,800-byte real
custody adapter. The three first-party ELFs total 44,432 bytes and about 0.316
SOL; composed with the official SPL Token 9.0.0 ELF, the two-transfer Direct
example commits in 24,901 CU. This is still not a fair feature-for-feature
comparison with the complete protocol because signed admission and registries
are absent. It does show that deployment, semantic, and repository units need
not be the same object.

The active successor is materially further along than those historical
measurements. Its canonical-authority controller is 79,680 bytes; together with
the 3,432-byte claim executor and 24,800-byte custody adapter, the three
first-party programs require 0.758104080 SOL of equivalent Loader V3
capitalization under the local default Rent profile. The larger controller now
authenticates two native signatures and the Market, Realm, manifest, semantic
release, finalized fee policy, mint, and custody graph. Its exact 990-byte v0
transaction executes both under ProgramTest and across a separate
`solana-test-validator` JSON-RPC process.

Lean now proves two general links that the historical experiment lacked:
`DirectProgram.admitted_program_refines` connects every high-level
`Admissible` frame to the unchanged 35-operation transition program, and
`CompiledPhysical.admitted_compilation_refines_physical_transition` connects
its successful outputs to the canonical claim/custody plans and their one
abstract atomic post-state. These are semantic and abstract-machine theorems,
not Rust, ELF, CPI, or Solana-runtime refinement claims.

The first metaprogramming layer has also landed. Typed Lean schemas now own
Direct's 41 scalar and four identity registers and generate their Rust names
and indices with canonical-order proofs. Lean also generates complete physical
claim/custody wire templates plus bounded, pairwise-disjoint dynamic patch
spans. The controller consequently owns neither output-register numbers nor
child-plan wire geometry. This change deliberately preserves the 568-byte
program, 79,680-byte controller footprint, and measured CU; its value is one
semantic ABI owner and a smaller adapter boundary, not benchmark theater.

## Proposed narrow waist

```text
Product + signed intent + chain frame
                │
                ▼
      semantic admission controller
      - authenticate exact release and frame
      - derive the only admissible EffectPlan
      - sign CPI with its controller PDA
                │ canonical Effect IR
                ▼
          claim microkernel
      - authenticate controller PDA signer
      - hostile-decode fixed data
      - apply replay/claim state atomically
                │ independently derived custody plan
                ▼
          custody micro-adapter
      - authenticate executor PDA signer
      - invoke Realm-selected Token program
      - check exact balance/supply postconditions
```

A Solana callee cannot inspect its caller directly. The controller identity must
therefore be proven by a PDA derived under the pinned controller program and
made a signer only by that controller's `invoke_signed`. Merely storing a public
key or accepting an arbitrary signer would be an authority hole.

The intended partition is by semantic responsibility, not by the historical 51
actions. A plausible first deployment contains a small number of programs:

1. immutable Realm/Product/release registry and finalized records;
2. provider/resolution controller;
3. trading admission controller consuming Product/Frame IR;
4. claims/replay Effect executor;
5. Realm-selected custody adapter; and
6. optional lifecycle capability controllers where their authority cannot be
   expressed as ordinary Product/Frame data.

That number is a hypothesis to measure, not a target to defend. Programs should
merge when cross-program authority, CU, or aggregate rent costs exceed the
semantic and verification benefit.

## Semantic ownership

Lean owns:

- Product and result-domain meaning;
- frame admissibility;
- exact quote and named rounding boundaries;
- the transition from pre-state to post-state;
- conservation, replay, boundedness, rollback, and fee theorems;
- canonical Effect IR and hostile vectors; and
- the statement that a specialized plan refines the abstract transition.

Rust or another SBF frontend may own only bounded byte decoding, account-memory
adaptation, and syscalls. It must not become a second authoritative economic
implementation; its data must be generated from Lean definitions or checked
against Lean-emitted canonical vectors.

## Artifact-level proof route

Four relevant primary projects were reviewed on 2026-08-25:

- [qedsvm](https://github.com/QEDGen/qedsvm) v0.11.0, pinned to commit
  `2356bc6865ed36a454d2a7285bd3989518ddd31f`, starts from compiled `.so`
  bytes and emits Lean path-scoped Hoare triples and CU bounds. Its current
  boundary is explicit: selected paths and modeled syscalls, not automatic
  whole-CFG verification. CPI lifting terminates at a checked caller envelope;
  callee behavior needs a separately composed theorem.
- [Solanalib](https://github.com/solana-foundation/leanprover-solanalib), pinned
  for this review to commit
  `6c115ef1ef6a0cde8dbd6fd875b7dc87d60939ec`, is an experimental Solana
  Foundation spec library with account/numeric abstractions and a bit-precise
  Lean port of all 22 instruction forms, decoder, small-step interpreter, and
  verifier from the OOPSLA 2025 sBPF semantics. Its own roadmap still lists the
  lifted-bytecode-to-high-level-spec refinement as open.
- [CertSBF](https://doi.org/10.1145/3720414) is the peer-reviewed Isabelle/HOL
  small-step semantics behind that port. Its archived
  [artifact](https://doi.org/10.5281/zenodo.14900585) validates an extracted
  executable semantics against the Solana eBPF interpreter; that validation is
  not itself a proof that Agave or a dClutch artifact refines our specification.
- [solana-lean](https://github.com/dennj/solana-lean), pinned for this review to
  commit `51ca6343c91f2bdc8a5f1ad1df68276921f82bad`, is an experimental Lean
  compiler fork with a freestanding runtime, Solana ABI/SDK, and direct SBF
  build target. It demonstrates that a post-Rust executable backend is
  technically plausible. Its compiler fork and runtime would become additional
  trusted or translation-validated components; direct compilation alone does
  not connect our theorem statements to the emitted ELF.

qedsvm is therefore the first artifact bridge to exercise. Solanalib remains a
candidate second machine semantics and source of reusable account and
bounded-numeric abstractions after a separate API, proof, license, and
provenance review. Direct Lean-to-SBF is a backend experiment, not a reason to
couple protocol meaning to a compiler fork. No neighboring local project or
newly reviewed external project is a dependency of this experiment.

The first SDK/Rust exercise was informative but incomplete. qedsvm executed the
exact 12,016-byte ELF and captured its successful path, then refused to lift its
overlapping mixed-width and copied memory footprints under the v0.11.0 H8 alias
model. The first successor experiment generated a purpose-built exact-account,
alias-simple seven-effect target. The active successor now specializes the
physical claim projection: 80-byte state, 72-byte plan, and no collateral
integers. qedsvm lifts its 119-instruction successful path, and Lean checks the
whitespace-normalized emitted theorem without proof-term rewrites. This closes
one historical artifact path, not the whole CFG or the canonical
program-to-artifact chain.

The desired theorem chain is:

```text
Direct.admissible frame
  → generated TransitionVM program derives the unique economic outputs       [landed]
  → those outputs select physical claim/custody plans and Direct.post         [landed]
  → canonical plan bytes decode to those exact physical plans                 [landed]
  → exact executor ELF path refines the concrete projection update
  → concrete projection codec denotes Direct pre/post state
  → custody envelope + separately proved callee preserve real assets
```

Every arrow is a named theorem or an explicitly unverified boundary. A test,
fixture execution, or qedsvm execution mode is never described as verification.

## Known pressure points

- The SDK executor has bounded loops for decode and application. The generated
  proof target is straight-line but still has many refusal branches. qedsvm
  proves one selected path, so the specializer must generate and cover the
  finite path family or establish a separate whole-CFG theorem.
- Even the SDK's no-allocation entrypoint reserves a generic 64-account frame
  and emits account structures and copies irrelevant to this two-account
  profile. The proof target removes that machinery with an exact serialized-
  input parser, canonical checks, and byte writes. Controller and custody
  profiles must preserve this exact-account property rather than regressing to
  a general `AccountInfo` collection.
- The general Lean compilation theorems are not property-space differential
  evidence for the Rust interpreter or SBF adapters.
  Generate admitted and hostile frames across quantities, prices, nonces,
  balances, fees, and arithmetic boundaries, then compare both implementations.
- The active claim projection no longer co-locates SPL balances. Lean's separate
  40-byte two-transfer custody plan now executes against real legacy SPL Token
  accounts under the replay-root delegate, with exact postconditions and a late
  rollback campaign. Realm selection, signed admission, semantic-release
  identity, and finalized fee policy are now inside the controller envelope;
  their concrete decoding and authority checks do not yet have artifact-level
  refinement theorems.
- Multiple programs reduce per-program rent and proof surface but add CPI CU,
  more release identities, authority PDAs, and aggregate program rent. Measure
  the complete partition before succession.
- Controller upgrade authority can invalidate an otherwise correct executor
  trust assumption. Bind controller Program/ProgramData identity and deployment
  generation exactly, or make the accepted controller immutable.

## Succession gates

No current Rust route is deleted until one complete vertical slice has:

1. one Lean-owned admissibility and transition definition;
2. generated canonical Product, Frame, Effect, account, and client codecs;
3. property-space differential agreement with the current reference;
4. real signed-intent authentication and controller-PDA binding;
5. real Realm-selected SPL custody with CPI postconditions;
6. exact qedsvm bytecode pins and checked path/refinement theorems;
7. explicit path coverage or loop-invariant evidence;
8. real-SVM success, hostile refusal, and transaction rollback evidence;
9. aggregate CU, account, ELF, and permanent-capitalization measurements; and
10. a checked release manifest binding source, toolchains, ELFs, program IDs,
    ProgramData identities, and theorem digests.

The canonical Direct slice now passes gates 1, 4, 5, 8, and 9 for its named
route. Gate 2 has advanced through generated register metadata, transition
bytecode, and physical-plan templates, but Product, Frame, account, and client
codecs remain incomplete. Gate 3 remains partial because differential
property-space coverage is incomplete. Gates 6 and 7 remain partial because
only a superseded claim executor's successful path reaches qedsvm, not the
canonical controller/custody path family. Gate 10 remains open. The controller
does derive its plans from authenticated inputs, and their abstract theorem
chain is composed; architectural succession still waits on concrete codec,
interpreter, ELF-path, coverage, and checked-release links.
