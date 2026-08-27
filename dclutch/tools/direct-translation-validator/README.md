# Direct translation validator

This tool compares two independently executed implementations of the compiled
Direct boundary:

- Lean evaluates the typed signed-intent/controller ABI and the 35-operation
  transition program over deterministic boundary, mutation, and hostile states.
- Safe Rust independently encodes/decodes the semantic values, classifies every
  single-byte mutation in the corpus, and executes the exact emitted transition
  program with `dclutch-transition-vm`.
- The separately implemented safe-Rust Direct AOT evaluator executes every
  full Direct state in that same Lean-emitted corpus and must agree on exact
  acceptance/refusal and every accepted scalar/identity output. Refusal must
  leave its caller-owned output bank byte-for-byte unchanged.

The corpus also executes isolated Lean-encoded microprograms for GTC lifecycle
admission, checked subtraction, equality selection, and zero selection. This
keeps newly added VM opcodes inside the cross-language boundary even before a
full specialized market program consumes every opcode.

The registered-creation extension independently checks:

- fourteen exact 152-byte Lean-emitted creation requests against the public
  safe-Rust encoder and strict decoder, including the embedded intent and the
  Market, generation, nonce, replay-bump, and registration-bump projections
  used to select replay and registration coordinates;
- every single-byte mutation, every truncated width, and a padded width; and
- a safe mutable replay/registration projection against Lean `register`, with
  valid first and reused-replay states plus exact refusal rollback for an
  occupied coordinate, non-open Market, invalid time window, expiry, zero
  maximum, outcome/fee mismatch, skipped or reused nonce, and nonce overflow.

The maker identity is a signing account, not a byte in the 152-byte request.
The semantic corpus carries a finite maker coordinate to check preservation,
but neither derives nor authenticates a Solana PDA. Agreement of the exact
Market/generation/nonce and bump projections is therefore input evidence for a
PDA derivation—not proof that the SBF adapter derived, authenticated, funded,
or assigned the correct accounts.

The registered terminal extension independently checks:

- every Lean-emitted cancellation and expiry controller request against the
  public safe-Rust encoder and strict decoder;
- every Lean-emitted claim-child cancellation and expiry request against the
  public safe-Rust encoder and a validator-local strict classifier;
- every single-byte mutation, every truncated width, and a padded width for
  both wire formats; and
- a safe mutable physical projection of cancellation and expiry against the
  Lean transition result, including exact refusal rollback for stale sequence,
  sequence overflow, wrong phase, invalid state, premature expiry, and maker
  mismatch.

Maker authentication is an adapter observation rather than a field in Lean's
`CancelFrame`. The corpus therefore wraps Lean `cancel` with the same explicit
maker-equality gate evaluated by the independent Rust projection. Expiry is
permissionless and ignores that observation.

Run:

```sh
tools/direct-translation-validator/check.sh
```

The checked relation is executable three-way differential agreement over the
emitted corpus: Lean semantics, the bytecode interpreter, and the separately
implemented Direct AOT evaluator. It is not universal source refinement and
not a proof about SBF, LLVM, Agave, CPI, account authentication, or transaction
rollback. Interpreter and AOT refusals are additionally checked to leave their
respective caller-owned output banks unchanged.

The check output identifies the exact Lean semantics, emitted interpreter
program, Rust interpreter, AOT evaluator, and generated AOT register layout by
SHA-256. A result cannot be transferred to differently digested sources.

The terminal physical projection in `src/terminal.rs` is validator code, not
the on-chain implementation. The shipping safe-Rust claim API currently
exposes the encoder; its child-instruction decoder lives across the SBF adapter
boundary. Accordingly, agreement with the validator-local strict claim
classifier is wire-format evidence, not a source-refinement claim about that
adapter parser. Likewise, the transition comparison does not establish that
the SBF account mutation path refines the Lean transition; exact-ELF and
real-runtime campaigns remain separate evidence.

`src/kani_proofs.rs` also contains eight bit-precise universal proof targets
for all fixed-width intent/controller values, registered creation and terminal
controllers, every truncated intent/creation/terminal-controller width, and
the reserved intent spans. They are deliberately `cfg(kani)`-gated, following
the Kani project's integration guidance. They are **not current evidence**:
Kani is not installed on the checked macOS host, so no Kani result is claimed
until a specific release/toolchain is pinned and the harnesses actually
complete.

The identity bridge maps each Lean `Nat` identity in this corpus injectively
into the first eight little-endian bytes of a physical 32-byte Rust identity.
The Direct program observes identities only through equality and inequality.
The corpus restricts those values and all scalar inputs to `u64` because that is
the adapter's named physical domain.

## Trusted and unchecked boundaries

Trusted for this run: the pinned Lean and Rust toolchains, both compilers and
runtimes, the host operating system/hardware, this corpus encoder/parser, and
the checked-in generated transition-program include.

Unchecked: whether arbitrary safe Rust refines the Lean functions outside the
finite corpus; lowering from Rust to SBF/LLVM; Solana loader and runtime
behavior; native signature verification; PDA derivation; account creation,
funding, ownership and alias checks; and CPI or token semantics. Artifact-level
path proofs and real-SVM campaigns remain separate evidence.

## Verification route

The local host has no usable Verus, Kani, or Creusot frontend. The ordinary
Rust 1.97.1 toolchain advertises a Miri subcommand, but the matching Miri
component is unavailable for this pinned macOS toolchain. No result from any of
those tools is claimed.

The next source-level step should be deliberately split:

1. Run the checked-in Kani harnesses with a pinned release. Kani's
   [official guide](https://model-checking.github.io/kani/usage.html) describes
   `cargo kani`, `#[kani::proof]`, and `cfg(kani)` integration. This is the
   smallest bit-precise route for the fixed-size codec loops and hostile widths.
2. Extract the actual safe sequential Rust codec/interpreter with
   [Charon/Aeneas](https://aeneasverif.github.io/projects/) and prove a relation
   between that functional model and the existing Lean ABI/VM definitions.
   This avoids maintaining a second handwritten economic specification in a
   Rust annotation language.
3. Use Verus or Creusot selectively if extraction rejects an otherwise small
   primitive. Verus supports a
   [Cargo verification workflow](https://verus-lang.github.io/verus/guide/cargo_verus.html);
   Creusot translates annotated Rust to Coma/Why3 and discharges generated
   verification conditions as described in its
   [official tutorial](https://guide.creusot.rs/tutorial.html). Either can prove
   useful source contracts, but neither by itself proves equivalence to the
   independently stated Lean semantics or to the emitted SBF artifact.
4. Compose source evidence with qedsvm/exact-ELF path evidence and real-SVM
   runtime campaigns. Compiler lowering, syscalls, CPI, and Agave remain named
   boundaries until those separate arrows exist.
