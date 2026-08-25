# qedsvm path theorem for the exact-account claim executor

This directory preserves the whitespace-normalized qedsvm v0.11.0 successful-path lift of
the exact claim ELF built from dClutch commit `884fe2a`.

## Exact inputs and outputs

- qedsvm tag: `v0.11.0`
- qedsvm commit: `2356bc6865ed36a454d2a7285bd3989518ddd31f`
- qedsvm license: MIT; see `LICENSE.qedsvm`
- ELF SHA-256:
  `229f399d457d494bf5629545794edeee984a6c0437bad0293c4ff12fc4ad9569`
- ELF bytes: 1,872
- `.text` SHA-256:
  `a13251120085644b991d07c2290680d0e0b26cc46fcf4cbdcb69b27b0023aaf4`
- `.text` bytes: 984 (119 decoded SBF instructions)
- successful trace SHA-256:
  `e946482b668996008e20d365cfecb2267f857916c569d19fd52c0faf1039958f`
- checked generated theorem SHA-256:
  `94bd1bbe0f9c26b8e7cdf2285ea8a0e1a13031792e40a653503835e085507a2c`

`DClutchClaimsProofLifted.lean` embeds the exact `.text` bytes and contains
`DclutchClaimsProofSbfLifted_lifted_spec`. Lean 4.30.0 checks the raw emitted
file and the stored file, which removes trailing whitespace only, with no
`sorry`, axioms, `admit`, `external_body`, or assumed specifications. The
theorem is a raw, assumption-heavy `cuTripleWithinMem` for
one successful path with a 109-step bound from PC 0 to PC 118. It is not
whole-CFG coverage and does not yet connect the concrete bytes to
`Physical.claim_plan_refines`, controller release authenticity, or SPL custody.

## Reproduction

Build the pinned dClutch source from a clean archive:

```sh
git archive 884fe2a | tar -x -C "$ARCHIVE_DIR"
cargo build-sbf \
  --manifest-path "$ARCHIVE_DIR/programs/dclutch-claims-proof-sbf/Cargo.toml" \
  --lto --optimize-size --dump --sbf-out-dir "$ARTIFACT_DIR"
```

In qedsvm at the pinned commit, copy `qedsvm_fixture.rs` to
`qedsvm-rs/examples/dclutch_claims.rs`, then run:

```sh
QEDSVM_TRACE_OUT=success.pcs cargo run \
  --manifest-path qedsvm-rs/Cargo.toml --example dclutch_claims -- \
  "$ARTIFACT_DIR/dclutch_claims_proof_sbf.so" \
  "$DCLUTCH_DIR/formal/dclutch-semantics/vectors/direct-inline-ordinary-claims-v1.hex"

cargo run --manifest-path qedsvm-rs/Cargo.toml \
  --features qedrecover --bin qedlift -- \
  --so "$ARTIFACT_DIR/dclutch_claims_proof_sbf.so" \
  --trace success.pcs --output DClutchClaimsProofLifted.lean

lake env lean DClutchClaimsProofLifted.lean
```

Unlike the retired seven-effect target's optional wrapper, this generated file
requires no proof-term or rewrite canonicalization; the stored copy strips only
trailing whitespace.
