# qedsvm proof for the exact-account Effect target

This directory preserves one qedsvm v0.11.0 successful-path lift of the exact
ELF built from dClutch commit `19692e0`.

## Exact inputs and outputs

- qedsvm tag: `v0.11.0`
- qedsvm commit: `2356bc6865ed36a454d2a7285bd3989518ddd31f`
- qedsvm license: MIT; see `LICENSE.qedsvm`
- ELF SHA-256: `552552310655e3339adace67847c4e8762d36ed861160187e2fffabfe173275b`
- ELF bytes: 2,232
- `.text` SHA-256: `8cdb7d505b7cfc9c88a1bfa663f279fc0e903ca02cc64c997d2cbe674253486b`
- `.text` bytes: 1,344 (164 decoded SBF instructions)
- successful trace SHA-256:
  `b6cadc195e62cb80b687e4951ac7635516ebdafc81893ca442f59ebefdc23c47`
- checked, path-canonicalized and wrapper-deduplicated output SHA-256:
  `4faa1fd9959fec840daf49092815cfa24683c5e73d44f91fda0ee106c7c4bcc3`

`DClutchEffectProofLifted.lean` embeds the exact `.text` bytes and contains the
path theorem `DclutchEffectProofSbfLifted_lifted_spec`. Lean 4.30.0 checks the
whole stored file without `sorry`, axioms, `admit`, `external_body`, or assumed
specifications. The theorem is a raw, assumption-heavy Hoare triple for one
successful path with a 154-step bound. It is not whole-CFG coverage and does not
connect the projection to Direct admissibility, controller authority, or SPL
custody.

## Reproduction

Build the source commit from a clean archive:

```sh
git archive 19692e0 | tar -x -C "$ARCHIVE_DIR"
cargo build-sbf \
  --manifest-path "$ARCHIVE_DIR/programs/dclutch-effect-proof-sbf/Cargo.toml" \
  --lto --optimize-size --dump --sbf-out-dir "$ARTIFACT_DIR"
```

In the pinned qedsvm checkout, copy `qedsvm_fixture.rs` to
`qedsvm-rs/examples/dclutch_effect.rs`, then:

```sh
QEDSVM_TRACE_OUT=success.pcs cargo run \
  --manifest-path qedsvm-rs/Cargo.toml --example dclutch_effect -- \
  "$ARTIFACT_DIR/dclutch_effect_proof_sbf.so" \
  "$DCLUTCH_DIR/formal/dclutch-semantics/vectors/direct-inline-ordinary-v1.hex"

cargo run --manifest-path qedsvm-rs/Cargo.toml \
  --features qedrecover --bin qedlift -- \
  --so "$ARTIFACT_DIR/dclutch_effect_proof_sbf.so" \
  --trace success.pcs --output DClutchEffectProofLifted.lean
```

qedsvm embeds the input's temporary absolute path in a comment. It also emits
duplicate `h_noovf4` and `h_noovf6` rewrite invocations in its optional
natural-number convenience wrapper. The first equal rewrite consumes every
matching occurrence, so the duplicate fails even though the machine theorem
has already been generated. Apply the canonicalizing substitutions in
`qedlift-wrapper-dedup.sed`; they normalize the temporary-path comment and
trailing whitespace and alter the convenience wrapper, not the machine theorem:

```sh
sed -i.bak -f "$DCLUTCH_DIR/formal/qedsvm-effect-proof/qedlift-wrapper-dedup.sed" \
  DClutchEffectProofLifted.lean
lake env lean DClutchEffectProofLifted.lean
```

The final check emits only two unused-variable warnings for those retained
wrapper parameters.
