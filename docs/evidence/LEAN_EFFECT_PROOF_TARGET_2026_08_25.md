# Lean-generated exact-account Effect proof target — 2026-08-25

## Scope

This evidence concerns only `dclutch-effect-proof-sbf` at source commit
`19692e0`. It is an exact-account physical projection for one Lean-owned
seven-effect shape. It is not a market, Direct lifecycle, semantic-admission
controller, or custody program.

Lean generates the loader-v1 offsets, privilege-frame words, state tag,
instruction length, and ordered effect tags. The small Rust file is an
explicitly unsafe loader-memory adapter. It assumes the loader provides a
non-null, aligned, sufficiently large ABI-v1 input buffer. It authenticates an
ordinary stored signer, not a controller PDA, and updates internal projection
balances rather than Realm-selected SPL accounts.

## Reproducible artifact

The exact committed tree was reconstructed with `git archive 19692e0` and
built with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-effect-proof-sbf/Cargo.toml \
  --lto --optimize-size --dump
```

- cargo-build-sbf: 4.0.0
- platform-tools: v1.53
- SBF rustc: 1.89.0
- default SBF architecture: v0
- verifier diagnostics: zero
- stripped ELF: 2,232 bytes
- ELF SHA-256:
  `552552310655e3339adace67847c4e8762d36ed861160187e2fffabfe173275b`
- `.text`: 1,344 bytes, 164 decoded instructions
- `.text` SHA-256:
  `8cdb7d505b7cfc9c88a1bfa663f279fc0e903ca02cc64c997d2cbe674253486b`
- read-only/data sections: none

The checked-in generated Rust constants exactly equal fresh output from Lean
4.30.0's `EmitSbfProfile.lean`. Lean checks the concrete loader offsets and
effect-shape constants with `native_decide`; that is a concrete generation
check, not a proof of the loader implementation.

Compared with the general SDK/no-allocation measurement executor, the exact
target is 81.42 percent smaller (12,016 to 2,232 bytes) and its successful
fixture is 87.48 percent cheaper (1,238 to 155 CU). This is still not a
feature-for-feature controller comparison.

## Real-SVM adversarial campaign

`effect_proof_target.rs` loaded the exact ELF through
`solana-program-test` 4.2.1 with no native processor or mock adapter.

- successful canonical plan: 155 CU and the exact expected seven-field state;
- twelve hostile cases refused: noncanonical state padding, mismatched stored
  authority, wrong owner, wrong writable privileges, short instruction,
  noncanonical plan header, outcome mismatch, claim nonconservation,
  collateral nonconservation, buyer-claim overflow, and late venue overflow;
- every refused transaction left its complete account byte-for-byte unchanged.

The campaign found and removed one real parser hazard before commit: instruction
length must be checked before reading the program ID at the length-dependent
offset.

## qedsvm path theorem

qedsvm v0.11.0 at commit
`2356bc6865ed36a454d2a7285bd3989518ddd31f` independently executed the exact ELF
at 155 CU and recorded the checked-in 155-PC successful trace. `qedlift` emitted
a 1,587-line Lean module embedding the exact 1,344 `.text` bytes and the theorem
`DclutchEffectProofSbfLifted_lifted_spec`.

The emitted raw machine theorem checks. The optional natural-number wrapper as
generated repeats two equal overflow rewrites; the second occurrence of each
rewrite fails because the first has already rewritten all matches. The stored
file canonicalizes qedsvm's temporary-path comment and removes only those two
duplicate wrapper invocations. Lean 4.30.0 checks it with two unused-variable
warnings and no `sorry`, axioms, `admit`, `external_body`, or assumed
specifications. Exact hashes and the canonicalization recipe are in
`formal/qedsvm-effect-proof/README.md`.

The theorem is a path-scoped, assumption-heavy `cuTripleWithinMem` result with a
154-step bound from entry to the selected terminal PC. It does not prove all
branches, establish loader memory assumptions, or relate the concrete
projection to the high-level Direct theorem. Runtime CU (155) and theorem path
steps (154) are therefore recorded separately.

## Loader V3 permanent capitalization

Using `Rent::default()`, a 36-byte Loader V3 Program account and 45-byte
ProgramData metadata require 0.017880240 SOL for this 2,232-byte artifact. That
is 4.81 times less than the 12,016-byte SDK executor's 0.085976880 SOL. It omits
the controller, custody adapter, registries, and every other required program,
so it is not an aggregate successor-rent estimate.

## Result and next gate

The experiment has crossed the first compiled-artifact bridge: one generated,
alias-simple successful SBF path is pinned to a kernel-checked Lean machine
theorem. The next meaningful gate is not further shaving this projection. It is
to authenticate a release-bound controller PDA, derive rather than trust the
Effect plan, and refine internal resource effects to real Realm-selected SPL
custody while preserving the small proof surface.
