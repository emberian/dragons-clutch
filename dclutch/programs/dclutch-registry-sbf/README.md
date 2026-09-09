# dClutch successor Registry SBF adapter

This program is the sole physical writer of the 1,288-byte execution-release
activation cache.

Activation consumes one finalized `ExecutionReleaseSetV1` record and five
finalized 216-byte `ArtifactReleaseV2` records (`DCLTARF2`, schema 2, with the
code commitment at offset 144). Each record must be owned by the
Registry program, rent exempt, content-addressed at the canonical raw-record
PDA, and paired with a vacant canonical staging PDA. For each role the adapter
then authenticates the exact Loader V3 Program and ProgramData accounts, their
canonical link/PDA, deployment slot, and upgrade authority. It does not hash
the complete payload again: Registry finalization already verified the record's
ordered code commitment from native Loader bytes.

Artifact finalization is the one code-byte verification boundary. A 344-byte
artifact staging cursor advances over the native ProgramData payload in
canonical 1 MiB chunks, rechecking Loader continuity and total payload length
on every call. Only the final call compares the complete commitment, closes the
cursor, and refunds its rent. A 4 MiB native-loader test completes in four
calls; each full-chunk call is approximately 545k CU. The exact measurements
and evidence boundary are recorded in
[`REGISTRY_ARTIFACT_V2_CHUNKED_FINALIZATION_2026_09_09.md`](../../docs/evidence/REGISTRY_ARTIFACT_V2_CHUNKED_FINALIZATION_2026_09_09.md).
`Abort` remains available to reclaim and refund an incomplete publication
under the cursor's existing sponsor and expiry rules.

The resulting cache is created permissionlessly at exactly
`[b"dclutch:release-activation:v1", execution_release_set_id]`. Repeated
activation is idempotent only when every derived byte is identical.

The read-only reauthentication route repeats the current native Loader
continuity check for one selected role, without rehashing code, and returns a
144-byte receipt. A CPI consumer must
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
