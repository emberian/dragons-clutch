# Lean/SBF successor architecture

Status: active experiment; not an accepted replacement.

## Why this direction exists

The current integrated SBF artifact is verifier-clean, but its 9.77 MB binary
requires about 68.01 SOL of permanent Loader V3 capitalization under the local
default Rent profile. Repeated Rust control flow, width-specialized transitions,
account adapters, provider handling, and every capability are linked into one
artifact.

The general SDK/no-allocation Effect executor is 12,016 bytes, requires about
0.086 SOL of equivalent Loader V3 capitalization, and executes its seven-effect
fixture in 1,238 CU. The generated exact-account proof target is 2,232 bytes,
requires about 0.018 SOL, and executes in 155 CU. Neither is a fair
feature-for-feature controller comparison; together they show that the
deployment unit, semantic unit, and repository crate should not be assumed to
be the same object.

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
          effect microkernel
      - authenticate controller PDA signer
      - hostile-decode fixed data
      - apply program-owned state atomically
      - issue typed custody effects only
                │ typed custody plan
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
implementation. Generated ABI structures and clients should come from the same
Lean definitions or be checked against Lean-emitted canonical vectors.

## Artifact-level proof route

Two relevant primary projects were reviewed on 2026-08-25:

- [qedsvm](https://github.com/QEDGen/qedsvm) v0.11.0, pinned to commit
  `2356bc6865ed36a454d2a7285bd3989518ddd31f`, starts from compiled `.so`
  bytes and emits Lean path-scoped Hoare triples and CU bounds. Its current
  boundary is explicit: selected paths and modeled syscalls, not automatic
  whole-CFG verification. CPI lifting terminates at a checked caller envelope;
  callee behavior needs a separately composed theorem.
- [Solanalib](https://github.com/solana-foundation/leanprover-solanalib) is an
  experimental Solana Foundation spec library with account/numeric abstractions
  and a Lean port of the OOPSLA 2025 sBPF semantics. Its own roadmap still lists
  the lifted-bytecode-to-high-level-spec refinement as open.

qedsvm is therefore the first artifact bridge to exercise. Solanalib remains a
candidate source of reusable account and bounded-numeric abstractions after a
separate API, proof, and provenance review. No neighboring local project is a
dependency of this experiment.

The first SDK/Rust exercise was informative but incomplete. qedsvm executed the
exact 12,016-byte ELF and captured its successful path, then refused to lift its
overlapping mixed-width and copied memory footprints under the v0.11.0 H8 alias
model. The successor experiment generated a purpose-built exact-account,
alias-simple target. qedsvm lifts its 164-instruction successful path, and Lean
checks the emitted machine theorem after removing two duplicate rewrite calls
from an optional convenience wrapper. This closes one artifact path, not the
whole CFG or the high-level refinement chain.

The desired theorem chain is:

```text
Direct.admissible frame
  → Lean effectPlan frame refines Direct.post frame
  → canonical plan bytes decode to that effectPlan
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
- One successful Direct fixture is not property-space differential evidence.
  Generate admitted and hostile frames across quantities, prices, nonces,
  balances, fees, and arithmetic boundaries, then compare both implementations.
- The isolated projection co-locates balances that are physically distinct SPL
  accounts today. A successor must refine those fields to real custody effects,
  not silently replace token ownership with an internal ledger.
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

The first slice has passed only portions of gates 1–3, 6, 8, and 9. It has not
yet passed a complete succession gate set.
