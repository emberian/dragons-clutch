# dClutch successor Registry SBF adapter

This program is the sole physical writer of the 1,288-byte execution-release
activation cache.

Activation consumes one finalized `ExecutionReleaseSetV1` record and five
finalized 216-byte `ArtifactReleaseV1` records. Each record must be owned by the
Registry program, rent exempt, content-addressed at the canonical raw-record
PDA, and paired with a vacant canonical staging PDA. For each role the adapter
then authenticates the exact Loader V3 Program and ProgramData accounts, their
canonical link/PDA, deployment slot, upgrade authority, and SHA-256 of the
complete ProgramData tail beginning at fixed offset 45.

The resulting cache is created permissionlessly at exactly
`[b"dclutch:release-activation:v1", execution_release_set_id]`. Repeated
activation is idempotent only when every derived byte is identical.

The read-only reauthentication route repeats the complete current deployment
check for one selected role and returns a 144-byte receipt. A CPI consumer must
check that this Registry program produced the return data and must compare the
receipt's role, program, artifact release, semantic release, and release-set ID
to its own immutable admission context.

No route deploys or upgrades programs, creates semantic records, changes Market
authority, or accepts an instruction-provided program/release identity.

## Optimized SBF checkpoint

The local source tree was built with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-registry-sbf/Cargo.toml \
  --lto --optimize-size \
  --sbf-out-dir target/registry-deploy
```

`cargo-build-sbf 4.0.0`, platform-tools v1.53, and SBF rustc 1.89.0 produced
a verifier-clean 89,584-byte ELF. SHA-256 was
`399c7ca711f38a3cc173142a8eec268552dfcf18b15307b617f8069dd53bf5a8`.
The section audit was `.text` 77,408, `.rodata` 4,744, `.data.rel.ro` 1,200,
`.dynamic` 176, `.dynsym` 288, `.dynstr` 158, and `.rel.dyn` 4,672 bytes.

This is a local build checkpoint, not a checked release, deployed artifact, or
mainnet claim. A clean committed rebuild must pin its own digest before use.
