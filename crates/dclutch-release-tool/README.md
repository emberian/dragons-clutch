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
