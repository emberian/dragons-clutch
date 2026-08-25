# Rust-to-SBF translation validation

Status: investigated path and one active-artifact probe; not an end-to-end
compiler proof.

Review date: 2026-08-25.

## Decision

Printing Rust constants from Lean is useful schema ownership, but it is not the
architectural endpoint and it is not translation validation. The strongest
realistic dClutch route is a two-ended proof chain:

1. extract the small canonical safe-Rust decoder/adapter functions into Lean and
   prove them equal to the Lean semantics; and
2. lift the exact linked SBF ELF and prove each admitted/refused machine path
   against the same Lean semantics.

The first end catches a wrong Rust implementation. The second end catches a
wrong compiler, linker, relocation, or handwritten adapter on every covered
path. Neither requires proving rustc or LLVM correct. The remaining gap is made
explicit as path coverage plus the Solana runtime/syscall boundary.

This favors a proof-carrying semantic IR and small stable executors, not a large
Lean-to-Rust pretty-printer. Lean should emit canonical Product/Frame/Effect
*data*. A small executor should interpret it. Source extraction and exact-ELF
lifting should establish that the executor implements the data's denotation.

## What each candidate actually establishes

| Tool | Useful dClutch role | Boundary it does not close |
| --- | --- | --- |
| [Aeneas](https://github.com/AeneasVerif/aeneas/tree/bacc7fad747cb5b1deb76fa914d9faa612f3c6f4) + [Charon](https://github.com/AeneasVerif/charon/tree/df133d3f9618641d52f49d046d91cc12eb383635) | Extract the actual safe-Rust/MIR decoder and patcher into Lean, then prove equality to `DClutchSemantics` in the same proof assistant. The official walkthrough is `charon cargo --preset=aeneas`, followed by the Aeneas Lean backend. | Trusts rustc/Charon's MIR extraction and Aeneas's functional translation; says nothing about LLVM, SBF instruction selection, linking, or runtime syscalls. |
| [Verus](https://github.com/verus-lang/verus/tree/813ec53a6fed4381b2141a6e15435dad7503c4f0) | Strong alternative for contracts over the canonical `no_std`, safe-Rust kernel. It verifies executable Rust against specifications without runtime checks. | Its HIR-to-VIR/AIR proof route is not a validator for the ordinary rustc/LLVM/SBF binary. A separate bridge to the Lean semantics would also be required. |
| [Kani](https://github.com/model-checking/kani/tree/4f7baae414d596eaa82ee90ee529f9957ca565dd) | Bit-precise exhaustive harnesses for bounded parser, arithmetic, and refusal properties; especially useful for finding counterexamples and proving small finite helpers. | Its MIR/GOTO/CBMC verification artifact is not the deployed SBF artifact. Loop unwinding and unsupported-feature boundaries must be explicit. |
| [Creusot](https://github.com/creusot-rs/creusot/tree/f753c901490403aaf4683c7e8cae069cc9feb2df) | Deductive contracts for safe Rust through Coma/Why3; another plausible kernel verifier. | Adds a second specification/prover stack and still does not connect LLVM or the emitted ELF. |
| [Alive2](https://github.com/AliveToolkit/alive2/tree/d46ff78380b582b95ad0c8ed4f843f011b8ee293) | Supplemental validation of supported LLVM IR optimization steps under the exact platform-tools LLVM fork. It is valuable compiler-bug detection. | It explicitly lacks interprocedural transformation support and does not validate rustc MIR-to-LLVM lowering, SBF instruction selection, LLD, ELF relocation, or Agave loading. It cannot be the principal bridge. |
| [qedsvm v0.12.0](https://github.com/QEDGen/qedsvm/tree/99bd5ede85374adc7fc5c835c2432ecf4e123fd1) | Starts at the exact `.so`, pins walked `.text` bytes, and emits Lean Hoare triples and CU bounds for selected paths. Its `--transition` mode can bundle a discovered finite trace family with supported transition refinements. | It is path-scoped, not automatic whole-CFG verification. Loops need bounded traces or invariants. Unsupported syscalls fail closed. CPI lifting proves the caller envelope; callee behavior needs a separate theorem. |
| [Solanalib](https://github.com/solana-foundation/leanprover-solanalib/tree/6c115ef1ef6a0cde8dbd6fd875b7dc87d60939ec) / [CertSBF](https://doi.org/10.1145/3720414) | Independent bit-precise sBPF instruction, decoder, verifier, and small-step semantics; the long-term second machine-semantics anchor. | The high-level dClutch refinement and current Agave/runtime equivalence are not supplied automatically. This is a research construction project, not the shortest route to a Direct proof. |
| [Agave](https://github.com/anza-xyz/agave/tree/0fa0bbe2c9a40b7565389181960376f5bda9d577) + [anza-xyz/sbpf](https://github.com/anza-xyz/sbpf/tree/2510663bb8d894e8e3094be351e4bb4b604f1f84) | The runtime implementation and verifier that current validators execute. Differential execution and verifier admission must pin exact versions. | Runtime tests are evidence, not a universal equivalence theorem between Agave and either Lean machine semantics. |

The source tools are complementary, not substitutes for an ELF proof. Verus,
Kani, or Creusot can prove a Rust program and rustc can still miscompile it.
Alive2 can validate many LLVM transformations and the SBF backend or linker can
still be wrong. qedsvm's exact-byte theorem is the only reviewed route that
bypasses the compiler stack for the covered behavior.

## Exact active-artifact probe

The currently built Direct controller was probed against qedsvm v0.12.0 rather
than inferred from its documentation:

- ELF: `target/deploy/dclutch_controller_proof_sbf.so`
- bytes: `79,520`
- SHA-256:
  `fc96b90929281f129d5e465f9323ea107f59d2b50e363f0b5a68779d5c6baf5f`
- ELF machine: registered `EM_SBF = 263`
- decoded logical instructions: `8,981`

Reproduction from a disposable checkout:

```sh
QEDSVM_DIR="$(mktemp -d /tmp/dclutch-qedsvm.XXXXXX)"
git clone --depth 1 --branch v0.12.0 \
  https://github.com/QEDGen/qedsvm.git "$QEDSVM_DIR"

cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  -p qedlift -- \
  --so target/deploy/dclutch_controller_proof_sbf.so \
  --coverage
```

Observed frontier:

```text
coverage dclutch_controller_proof_sbf: 0/1 lifted
    1  syscall-untraced
         <static>: modeled syscall `sol_memcpy_` ... at pc 8212 ...;
         provide a --trace to dispatch it.
```

This result is useful and narrow. qedsvm v0.12.0 parses the active `EM_SBF`
artifact and reaches a modeled syscall. The no-trace static walk is insufficient
for the branchy controller; it is not evidence that the signed Direct path is
unsupported. The binary also contains instruction forms outside qedsvm's
currently lifted set, but a concrete Direct trace must determine whether any are
reachable on the proof target before they become blockers.

The probe also exposed a local release-gate defect. The release tool admitted
only legacy `EM_BPF = 247`, although the current toolchain emits `EM_SBF = 263`
and the Solana sBPF loader admits both. The release validator and hostile tests
now admit exactly those two identifiers and refuse adjacent values. That repairs
artifact admission; it does not add semantic verification.

## The buildable proof chain

### 1. Extract the canonical Rust leaf functions

Start with `dclutch-direct-codec`, the fixed template patch primitive, and the
claim executor's hostile decoder. They are safe, `no_std`, allocation-free, and
have bounded arrays, so they are much better extraction candidates than the
Solana SDK controller.

The preferred experiment is Aeneas because the extracted function lands in the
same Lean environment as the canonical semantics:

```sh
charon cargo --preset=aeneas \
  --manifest-path crates/dclutch-direct-codec/Cargo.toml \
  --dest-file direct-codec.llbc

aeneas -backend lean direct-codec.llbc \
  -dest formal/dclutch-rust-refinement \
  -subdir /DClutchRust/Code \
  -split-files \
  -namespace DClutchRust
```

These are upstream command shapes, not yet a passing dClutch gate. The first
artifact should cover only `slice`, fixed-width integer decode, reserved-byte
refusal, `CompactIntentV1::decode`, and the bounded patch primitive. Unsupported
core-library calls must be modeled explicitly or replaced in the *canonical*
Rust with simpler equivalent loops. Do not create a verification-only Rust twin.

The acceptance theorem should have this shape:

```text
extracted_Rust_decode(bytes) = Lean_decodeCompactIntentV1(bytes)
```

for every byte list, including every refusal. A matching theorem for template
patching should quantify over every in-range value and state equality to the
canonical Lean encoder. The Charon commit, Aeneas commit, `.llbc` digest,
generated Lean digest, assumptions, and theorem names belong in release
evidence.

If Aeneas cannot translate the canonical leaf without semantic surgery, use
Verus on that same canonical function as the next experiment. Kani is the fast
counterexample/exhaustive-helper lane, not the final cross-language theorem.

### 2. Capture exact Direct machine paths

Extend the existing qedsvm Rust fixture to execute the same controller input and
account frame used by the real-SVM `physical_direct_composition` campaign. Emit
one logical-PC trace for success and one for each materially distinct refusal or
fault class. Do not claim that sampled traces exhaust the CFG.

The intended pinned flow is:

```sh
QEDSVM_TRACE_OUT=direct-success.pcs \
  cargo test --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  --test dclutch_direct -- direct_success --exact --nocapture

cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/qedrecover/Cargo.toml" -- \
  --so target/deploy/dclutch_controller_proof_sbf.so \
  --overlay formal/qedsvm-direct/direct.qedoverlay.toml \
  --trace direct-success.pcs \
  --qedmeta-out formal/qedsvm-direct/direct.qedmeta.toml

cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  -p qedlift -- \
  --so target/deploy/dclutch_controller_proof_sbf.so \
  --qedmeta formal/qedsvm-direct/direct.qedmeta.toml \
  --trace direct-success.pcs \
  --output formal/qedsvm-direct/DirectSuccessLifted.lean
```

The fixture, overlay, and output names are proposed and do not exist yet. Once
at least two justified traces and a supported descriptor exist, qedsvm's
`--transition` mode can emit per-path corollaries and a bundle:

```sh
cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  -p qedlift -- \
  --so target/deploy/dclutch_controller_proof_sbf.so \
  --transition \
  --descriptor formal/qedsvm-direct/direct-transition.toml \
  --output-dir formal/qedsvm-direct/generated
```

The bundle covers the discovered, supplied path family under its branch guards.
It does not prove that trace discovery was exhaustive. A separate coverage
argument must show that every admitted/refused semantic case selects one covered
path, or else the release must retain the uncovered cases as explicit
limitations.

### 3. Prove children separately and compose CPI envelopes

The controller's qedsvm walk terminates at `sol_invoke_signed`. That is the right
boundary, not a failure. Prove:

1. controller prefix produces the exact Lean-owned claim/custody envelope and
   authenticates its PDA signer;
2. claim executor exact ELF refines the claim projection;
3. custody adapter exact ELF presents the intended SPL instruction and checks
   postconditions; and
4. a separately pinned SPL Token/Agave assumption supplies the callee semantics
   until an artifact theorem for that callee is composed.

This is why the multiprogram architecture helps proof tractability. It does not
magically reduce aggregate rent or CU; those remain measured succession gates.

### 4. Use Alive2 only as a compiler differential

An optional platform-tools experiment should preserve pre- and post-pass LLVM
IR for the small executors and run the Alive2 plugin against the exact LLVM fork.
It is accepted only if the fork/version pairing builds and every skipped or
unsupported transform is recorded. Even a clean result is labeled “LLVM
optimization validation for supported functions,” never “Rust-to-SBF
verification.”

No reviewed tool presently validates this entire chain:

```text
Rust source -> HIR/MIR -> LLVM IR -> optimized LLVM IR
            -> SBF instruction selection -> LLD/relocations -> ELF
```

Trying to force one tool to cover it is not the path to victory. Proving the
small canonical Rust leaves and independently proving the exact machine paths
meets around that gap.

## Trust boundary at victory

A Direct vertical can be called artifact-refined only when the release names:

- the exact high-level Lean theorem and source digest;
- the exact extracted canonical-Rust functions and extraction-tool digests;
- the equality theorem between extracted Rust and high-level Lean semantics;
- the exact ELF, `.text`, toolchain, and deployment digests;
- every qedsvm path theorem and the coverage argument relating semantic cases to
  those paths;
- modeled instruction/syscall/CPI assumptions;
- the qedsvm/Solanalib machine-semantics version and its differential Agave
  evidence;
- Agave, SPL Token, loader, native Ed25519, PDA, account-memory, and atomic
  rollback boundaries that remain unproved; and
- the upgrade-authority/ProgramData identity that can invalidate the artifact
  claim.

That is substantially stronger than “Lean generated the offsets,” while still
being achievable incrementally. Full rustc or LLVM verification is not a
precondition for surpassing Dragon's Clutch. Exact source refinement plus exact
ELF path refinement of a small semantic executor is.

## Parallelizable implementation lanes

This route can be worked wide without creating independent protocol islands:

1. one owner extracts and proves the canonical Rust codec/patch leaf;
2. one owner builds the qedsvm Direct trace fixture and success-path lift;
3. one owner classifies refusal paths and proves semantic-case coverage;
4. one owner lifts the claim executor and controller CPI envelope;
5. one owner investigates the custody/SPL theorem boundary;
6. one owner runs the pinned Alive2 feasibility experiment; and
7. one integration owner binds all exact digests and theorem names in checked
   release evidence.

All lanes consume one Lean semantic owner and one canonical Rust implementation.
None is authorized to introduce a second economic model.
