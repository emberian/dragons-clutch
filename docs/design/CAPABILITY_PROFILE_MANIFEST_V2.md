# Capability-profile identity and measurement V2

Status: **offline producer/checker contract; no concrete profile is declared
deployable here**.

The program's Cargo features are build selectors, not complete protocol
identities. A measured ELF must also bind the exact semantic generations that
own its economics, the central wire registry it exposes, the persisted account
codecs those routes use, its source/laboratory class, and its artifact limits.

[`check_capability_profile.py`](../../programs/clutch-sbf/scripts/check_capability_profile.py)
validates that identity. It reads local JSON only. It never builds, contacts an
RPC endpoint, signs, deploys, or declares a release.

[`measure_capability_profiles.py`](../../programs/clutch-sbf/scripts/measure_capability_profiles.py)
is the corresponding offline producer. It has no duplicated intent-tag table.
It will build only from an explicit tracked identity manifest whose semantic
owners are all `linked` and whose central-registry coverage is exhaustive. If
no such manifest is supplied, it emits schema V2 with `availability:
unavailable`, an empty profile list, and a refusal, then exits nonzero. This is
the current expected state: historical size rows are not promoted by copying
them into a new schema.

## Identity manifest

The manifest schema is
`dragons-clutch/capability-profile-manifest/v2`. Unknown keys and duplicate
JSON keys refuse. Its canonical identity is SHA-256 of compact, key-sorted
UTF-8 JSON containing:

- the canonical profile name and label;
- the Cargo profile feature and source identity;
- the expected final undefined dynamic-symbol/syscall surface;
- all eleven semantic-owner rows;
- the central-registry version, digest, exact enabled intent triples, and exact
  linked account coordinates; and
- ELF, `.text`, chosen ProgramData `max_len`, and persistent-loader-rent limits.

The eleven semantic-owner slots, in canonical order, are `relation`, `score`,
`price-measure`, `candidate-lifecycle`, `clear-work-feed`, `retirement`,
`source-plane`, `series-products`, `recovery`, `structured-claim`, and
`liquidity-dealer`. Every row binds an owner name, version, digest, linkage
state, required intent triples, and required account coordinates.

Intent coordinates are `[outer_tag, version, local_action]`. Legacy two-byte
intents use local action zero; successor envelopes use their nonzero
family-local action. Account coordinates are `[tag, version]`. The manifest
pins the central registry's own semantic digest rather than teaching the Python
checker another copy of Rust's allocation ranges.

For all `linked` owners, required intent and account coordinates must equal the
top-level enabled/linked coverage exactly. A missing required coordinate and an
enabled coordinate with no linked semantic owner both refuse. Requirements of
a `planned` owner do not become live coverage. A profile with any planned owner
can be described for planning but cannot be measured or deployment-eligible.

This gate does not let a caller-provided digest attest to its own derivation.
Each semantic owner and the central registry still need a reviewed producer for
their declared version/digest. No such complete manifest exists today, which is
why this wave adds the unavailable/refusal state instead of a concrete live
profile.

## Mock and real-Pyth boundaries

`build_contract.source_identity` is exactly one of:

| Identity | Additional Cargo feature | Meaning |
| --- | --- | --- |
| `production-inert` | none | Ordinary artifact identity; this name does not claim a production source release. |
| `non-production-mock-source-lab` | `non-production-mock-source` | Fabricated-provider laboratory ELF. |
| `non-production-real-pyth-lab` | `non-production-real-pyth-lab` | Captured real-program/local-synthetic-observation laboratory ELF. |

The source class and exact Cargo features are in the profile identity. A mock
ELF and a real-Pyth laboratory ELF therefore cannot share a capability identity
even when their base profile feature is the same. Neither laboratory class is
production or network-price evidence.

The producer builds `profile-full` explicitly twice. Only a
`production-inert` full profile is also built once through Cargo defaults, and
the default artifact must equal the explicit artifact while bound to the same
identity manifest. A laboratory full profile is a distinct identity and cannot
reuse that equivalence.

## Source and toolchain linkage

The schema-V2 record binds:

- Git commit and tree;
- a canonical tracked-file closure, file count, and path-ordered per-file
  SHA-256 fold;
- empty tracked **and untracked** status before and after all builds; and
- version strings and binary SHA-256 digests for `cargo-build-sbf`, platform
  `rustc`, `llvm-readobj`, and `llvm-objdump`.

The identity manifest itself must be tracked and is added to the closure. The
producer refuses any staged, unstaged, or untracked path in that closure and
refuses a file-list or digest change during measurement. Ignored build output
is placed in fresh temporary targets and is not evidence input.

The evidence also records a digest of the producer's canonical planning
manifest. When a checker later reads a deployable manifest, it normalizes only
the classification and evidence-pointer fields back to their planning values
and requires that digest to match. Semantic, registry, build, or budget changes
therefore cannot be hidden behind a new evidence pointer.

## Final artifact evidence

Each profile has two explicit fresh builds. A full production-inert profile
also has the default-equivalence build described above. Byte identity is
required across the applicable runs. Every run records and the checker
recomputes or compares:

- final ELF SHA-256 and exact current ELF length;
- exact `.text` and `.rodata` sizes;
- the sorted exact undefined dynamic-symbol/syscall surface;
- backend stack-diagnostic counts and zero diagnosed symbols surviving final
  LTO; and
- a final unstripped-symbol/disassembly audit of every direct `r10` reference,
  with the 4,096-byte frame boundary and complete text-function disassembly.

This is an artifact-shape and direct-frame audit, not a proof of control-flow
reachability, runtime correctness, or compute-unit cost.

## Loader-rent vocabulary

Schema V2 does not hide account overhead inside names such as “account bytes.”
For the selected loader-v3 model it records separately:

- Program account data length: exactly `36` bytes;
- ProgramData data length: `45 + chosen max_len` bytes;
- transient Buffer data length: `37 + chosen max_len` bytes; and
- rent storage overhead: exactly `128` billable bytes **per account**.

Each account row records data length, overhead, billable bytes, and rent-exempt
lamports. The current ELF length and chosen ProgramData `max_len` are distinct;
`exact_size_allocation` is true only when they match. Persistent deployment
principal is Program plus ProgramData. Buffer principal is separately labeled
`transient-recyclable` and is never added to the persistent figure. Fees are
outside both quantities.

The manifest budget limits persistent Program+ProgramData rent. Transient
Buffer liquidity is still reported, but it is not silently turned into a
persistent-rent ceiling.

## Historical evidence and deployment eligibility

The dated schema-V1 evidence at
`programs/clutch-sbf/audit/evidence/2026-08-22-capability-profiles.json` remains
comparison-only. The checker understands its older combined rent vocabulary so
existing rows remain readable, but rejects it as linked evidence because it has
no V2 identity, registry, owner, cleanliness, final-frame, lab-class, or
chosen-`max_len` binding.

A manifest classified `deployable` passes only when every semantic owner is
linked, an available clean schema-V2 measurement repeats the exact identity
manifest, all registry coverage is complete, the final artifacts reproduce,
and all budgets pass. Passing is a manifest-consistency result, not a release,
deployment authorization, security proof, formal-verification claim, devnet
result, or mainnet evidence.

```sh
# Expected to refuse with an explicit unavailable document until a concrete
# fully linked identity manifest is reviewed and checked in:
python3 programs/clutch-sbf/scripts/measure_capability_profiles.py

python3 programs/clutch-sbf/scripts/measure_capability_profiles.py \
  --identity-manifest path/to/tracked-profile-manifest.json \
  --output path/to/new-measurement.json

python3 programs/clutch-sbf/scripts/check_capability_profile.py \
  path/to/profile-manifest.json

python3 -m unittest \
  programs/clutch-sbf/scripts/test_check_capability_profile.py \
  programs/clutch-sbf/scripts/test_measure_capability_profiles.py -v
```
