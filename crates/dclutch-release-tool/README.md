# dclutch-release-tool

This is an offline evidence compiler for one checked dClutch SBF release. It
does not deploy, sign, read RPC state, or declare an address official.

It keeps two identities separate:

- the **semantic release ID** is SHA-256 of an exact semantic preimage already
  owned by the relevant capability or Pyth release contract; and
- the **checked release ID** is SHA-256 of this tool's canonical manifest,
  which binds that semantic identity to one exact SBF ELF, Loader V3 account
  snapshots, source and lockfile digests, toolchain strings, build command,
  target, and explicit assumptions.

The ProgramData snapshot must contain the supplied ELF at Loader V3's fixed
45-byte program-data offset. Any remaining allocation bytes must be zero. The
Program account must be the exact 36-byte Loader V3 Program state and must name
the supplied ProgramData address.

## Commands

```text
dclutch-release-tool create \
  --elf <program.so> \
  --semantic-preimage <release.bin> \
  --metadata <metadata.txt> \
  --program-account-data <program-account.bin> \
  --programdata-account-data <programdata-account.bin> \
  --out <checked-release.bin> \
  [--text-out <checked-release.txt>]

dclutch-release-tool verify <same evidence flags> \
  --manifest <checked-release.bin> \
  [--text-out <checked-release.txt>]

dclutch-release-tool inspect --manifest <checked-release.bin> \
  [--text-out <checked-release.txt>]
```

Without `--text-out`, the canonical text projection is written to stdout.
Verification is offline and exits unsuccessfully for any noncanonical
manifest, evidence mismatch, ELF change, semantic-preimage change, Loader
linkage change, ProgramData change, metadata change, or nonzero allocation
padding.

## Multiprogram release sets

The successor runtime is promoted as one Registry-selected five-role set, not
as unrelated individually checked programs. `CheckedExecutionReleaseSetV1`
binds the canonical Core, Claims, Trading, Resolution, and Custody release-set
preimage to each role's compact onchain `ArtifactReleaseV1` record and complete
`CheckedReleaseV1` manifest identity.

```text
dclutch-release-tool create-set \
  --release-set <execution-release-set.bin> \
  --core <core.checked> --claims <claims.checked> \
  --trading <trading.checked> --resolution <resolution.checked> \
  --custody <custody.checked> --out <multiprogram.checked> \
  [--text-out <multiprogram.txt>]

dclutch-release-tool verify-set \
  --manifest <multiprogram.checked> \
  --core <core.checked> --claims <claims.checked> \
  --trading <trading.checked> --resolution <resolution.checked> \
  --custody <custody.checked> [--text-out <multiprogram.txt>]

dclutch-release-tool inspect-set --manifest <multiprogram.checked> \
  [--text-out <multiprogram.txt>]
```

Construction refuses program or artifact-release substitution. Verification
re-decodes all five checked manifests, derives their compact Registry records,
and requires the entire set to rebuild byte-for-byte. The manifest is offline
reproducibility evidence: the Registry activation cache remains the sole
runtime authority, and this is neither deployment nor public-network evidence.

### From checked evidence to Registry activation

The host-only operator join accepts the verified
`CheckedExecutionReleaseSetV1`, all five complete `CheckedReleaseV1` values,
and one finalized snapshot of the canonical release-set record, artifact
records, Loader V3 Program accounts, and ProgramData accounts. It rebuilds the
checked set, delegates chain authentication to the existing Registry operator,
and requires the resulting activated releases to equal the checked artifacts
exactly before compiling the existing unsigned activation packet.

Its deterministic text projection includes the checked-set identities, the
finalized observation, activation-cache address and mode, complete ELF bytes
hashed, packet geometry, compute budget, and a digest of the exact unsigned
message. This projection is evidence, not another release DTO or runtime
authority. The builder performs no RPC, signing, submission, deployment, or
account mutation; execution by the Registry is still what creates the
authoritative cache.

## Metadata V1

The metadata input is canonical UTF-8 text. Lines must occur in this exact
order, hexadecimal values must be 64 lowercase digits, booleans must be exact,
and at least one strictly sorted unique `assumption=` line is required.

```text
dclutch-release-metadata-v1
semantic_kind=capability
program_id=<hex32>
programdata_id=<hex32>
loader_program_id=<hex32>
program_owner=<same loader hex32>
program_executable=true
programdata_owner=<same loader hex32>
programdata_executable=false
source_digest=<hex32>
cargo_lock_digest=<hex32>
source_revision=<printable one-line value>
rustc_version=<printable one-line value>
solana_version=<printable one-line value>
cargo_build_sbf_version=<printable one-line value>
target_triple=<printable one-line value>
build_command=<printable one-line value>
assumption=<printable one-line value>
```

`semantic_kind` is `capability` for a capability-owned exact preimage or
`pyth-v1` for the canonical 440-byte `PythReleaseV1` preimage. In the latter
case this tool delegates semantic decoding to `dclutch-pyth-svm`; it does not
reimplement or loosen the Pyth release schema.
