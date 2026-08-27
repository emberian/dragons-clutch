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

## Offline Loader V3 account construction

A checked release binds *account* evidence, not only an ELF. Before a program is
deployed anywhere there is nothing to snapshot, so the account bytes have to be
constructed. `loader-accounts` does that construction once, in the same crate
that later enforces the layout, instead of leaving every caller to reimplement
Loader V3's 36-byte `Program` record, its 45-byte `ProgramData` metadata
boundary, and its ProgramData address derivation.

```text
dclutch-release-tool loader-accounts \
  --program-id <hex32> --loader-program-id <hex32> \
  --elf <program.so> --deployment-slot <u64> \
  [--upgrade-authority <hex32> | --revoked-authority <hex32>] \
  --program-out <program-account.bin> \
  --programdata-out <programdata-account.bin> \
  [--text-out <loader-accounts.txt>]
```

The authority is **three** states, not two, because Loader V3's is. The tag at
byte 12 and the key at `[13..45]` sit in a fixed 45-byte metadata region, so
`SetAuthority(Some -> None)` writes the shorter `None` over the longer `Some`
**without clearing the key**: a revoked program keeps its former authority
sitting inert behind a zero tag for the rest of its life (measured on Agave
4.0.2, `docs/design/DEVNET_DEMO_DEPLOY.md` section 2.5). So:

| flags | tag | `[13..45]` | what it is |
|---|---:|---|---|
| neither | `0` | zero | never authorized — a genesis install |
| `--upgrade-authority K` | `1` | `K` | mutable, upgradeable by `K` |
| `--revoked-authority K` | `0` | `K` | **immutable, formerly `K`** |

The third row is the only shape a real deployed-then-revoked devnet program is
ever in, and until it existed every checked manifest built offline carried a
`programdata_account_sha256` no deployed account could match — which the
frontend's byte-exact `authenticateDeployment` would refuse for every role.
A zero key is refused in both key-carrying states; the loader cannot produce one.

The text projection reports `evidence_class=predicted-loader-state-not-observed`
and names the derived ProgramData address. A run carrying `--revoked-authority`
reports `evidence_class=loader-state-carrying-an-observed-retained-authority`
instead, because nothing offline knows which key a program used to have: that
value can only come from reading the account. **These bytes are a prediction of the
account state a Loader V3 deployment—or a `solana-test-validator
--upgradeable-program ADDRESS ELF none` genesis install—would hold. They are not
an observation of any chain.** A release built from constructed accounts must
say so in its own `assumption=` lines; the tool cannot say it for you, and no
downstream manifest turns a prediction into a deployment.

## Multiprogram release sets

The successor runtime is promoted as one Registry-selected five-role set, not
as unrelated individually checked programs. `CheckedExecutionReleaseSetV1`
binds the canonical Core, Claims, Trading, Resolution, and Custody release-set
preimage to each role's compact onchain `ArtifactReleaseV1` record and complete
`CheckedReleaseV1` manifest identity.

The 336-byte release-set preimage is not independent evidence: every binding is
a pure function of the five checked manifests. `derive-set` emits exactly the
value that `create-set` will then insist on, so nobody has to hand-transcribe it.

```text
dclutch-release-tool derive-set \
  --core <core.checked> --claims <claims.checked> \
  --trading <trading.checked> --resolution <resolution.checked> \
  --custody <custody.checked> --out <execution-release-set.bin>

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

### Immutable infrastructure join

The five-role execution set is not sufficient by itself: Core selects one
immutable Registry and Rent pair through `ProtocolInfrastructureProfileV1`.
The infrastructure commands verify all five role releases again, require the
Core release in that exact set, bind the exact profile to independently checked
Registry and Rent artifacts, derive the profile PDA, and refuse any upgradeable
component.

The 144-byte profile is likewise determined by the checked Registry and Rent
manifests. `derive-infrastructure-profile` emits it and refuses an upgradeable
component at derivation time rather than letting one reach the manifest.

```text
dclutch-release-tool derive-infrastructure-profile \
  --registry <registry.checked> --rent <rent.checked> \
  --out <infrastructure-profile.bin>

dclutch-release-tool create-infrastructure \
  --execution <multiprogram.checked> \
  --profile <infrastructure-profile.bin> \
  --core <core.checked> --claims <claims.checked> \
  --trading <trading.checked> --resolution <resolution.checked> \
  --custody <custody.checked> \
  --registry <registry.checked> --rent <rent.checked> \
  --out <infrastructure.checked> \
  [--text-out <infrastructure.txt>]

dclutch-release-tool verify-infrastructure \
  --manifest <infrastructure.checked> \
  --execution <multiprogram.checked> \
  --core <core.checked> --claims <claims.checked> \
  --trading <trading.checked> --resolution <resolution.checked> \
  --custody <custody.checked> \
  --registry <registry.checked> --rent <rent.checked> \
  [--text-out <infrastructure.txt>]

dclutch-release-tool inspect-infrastructure \
  --manifest <infrastructure.checked> \
  [--text-out <infrastructure.txt>]
```

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

## Capability execution evidence

Shadow and admitted capability strategies execute in separately deployed SBF
accelerators, so their executable identity is not covered merely by checking
the five-role release set. `CheckedCapabilityExecutionV1` joins one exact
`CapabilityProgramV4`, `ExecutionStrategyV2`, `AotCertificateV4`, optional
`AotAdmissionV4`, immutable accelerator `ArtifactReleaseV1`, and the complete
checked release manifest for that accelerator.

```text
dclutch-release-tool create-capability-execution \
  --descriptor <capability-v4.bin> \
  --strategy <strategy-v2.bin> \
  --certificate <certificate-v4.bin> \
  [--admission <admission-v4.bin>] \
  --accelerator <accelerator.checked> \
  --out <capability-execution.checked> \
  [--text-out <capability-execution.txt>]

dclutch-release-tool verify-capability-execution \
  --manifest <capability-execution.checked> \
  --accelerator <accelerator.checked> \
  [--text-out <capability-execution.txt>]

dclutch-release-tool inspect-capability-execution \
  --manifest <capability-execution.checked> \
  [--text-out <capability-execution.txt>]
```

Construction independently derives the interpreter tuple from the descriptor,
requires the strategy and certificate to select that exact tuple, and applies
the complete admitted-AOT validator when an admission is present. Shadow
evidence must have an all-zero admission slot. Interpreted strategies are
refused because they have no external accelerator ELF to check, and upgradeable
accelerators are refused because their executable bytes are not immutable.

This evidence does not prove the accelerator implements the declared
semantics. It binds the semantic declaration, certificate and admission to one
immutable executable artifact without adding a second runtime authority. The
translation-validation or proof obligation for that exact artifact remains a
separately named boundary.

## Direct translation-validation evidence

`tools/direct-translation-validator/check.sh <evidence-dir>` emits 21 exact
inputs under the stable labels owned by `CheckedTranslationValidationV1`.
The release tool hashes and checks that complete directory rather than parsing
human-readable digest output:

```text
dclutch-release-tool create-translation \
  --evidence-dir <direct-translation-evidence> \
  --out <direct-translation.checked> \
  [--text-out <direct-translation.txt>]

dclutch-release-tool verify-translation \
  --manifest <direct-translation.checked> \
  --evidence-dir <direct-translation-evidence> \
  [--text-out <direct-translation.txt>]

dclutch-release-tool inspect-translation \
  --manifest <direct-translation.checked> \
  [--text-out <direct-translation.txt>]
```

The directory must contain exactly named `<label>.bin` inputs for the labels
shown by `inspect-translation`. This is finite-corpus three-way differential
evidence. It is not universal Rust refinement, an SBF artifact proof, or an
Agave/runtime proof.

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

`semantic_kind` is `capability` for a capability-owned exact preimage,
`pyth-v1` for the canonical 440-byte `PythReleaseV1` preimage, or `unowned`.
For `pyth-v1` this tool delegates semantic decoding to `dclutch-pyth-svm`; it
does not reimplement or loosen the Pyth release schema.

`unowned` is a **named absence**, not a schema. Every execution role, Registry,
and Rent persists a `semantic_release_id` inside its `ArtifactReleaseV1`, but no
first-party contract in this tree owns or decodes a role-program release
preimage. Calling such a preimage `capability` would assert a decoder that does
not exist. `unowned` records the exact bytes and their SHA-256 identity while
stating plainly that naming an owner is still an open protocol obligation. The
tool does not invent that owner.
