# Capability-profile manifest V1

Status: **offline CI contract; no profile is declared deployable here**.

The SBF feature profiles currently say which instruction tags are compiled into
an ELF. That is necessary but not sufficient for a protocol identity. A
deployable product must also say which exact semantic generations own scoring,
candidate admission and timing, streamed clearing, sources, retirement,
structured claims, and liquidity. Otherwise two ELFs can share a feature label
while disagreeing about the market they implement.

[`check_capability_profile.py`](../../programs/clutch-sbf/scripts/check_capability_profile.py)
is the offline, standard-library-only gate for that stronger identity. It reads
local JSON and returns a deterministic summary or a refusal. It never builds,
measures, contacts an RPC endpoint, signs, deploys, or declares a release.

## Required semantic owners

V1 has exactly seven slots in this canonical order. Each owner occurs exactly
once; the checker refuses missing slots, missing owners, unknown slots or
owners, duplicate slots or owners, owner/slot swaps, and reordered rows.

| Slot | Canonical semantic owner |
| --- | --- |
| `score` | `dragons-clutch/semantic-owner/score` |
| `candidate-lifecycle` | `dragons-clutch/semantic-owner/candidate-lifecycle` |
| `clear-work` | `dragons-clutch/semantic-owner/clear-work` |
| `source-plane` | `dragons-clutch/semantic-owner/source-plane` |
| `retirement` | `dragons-clutch/semantic-owner/retirement` |
| `structured-claim` | `dragons-clutch/semantic-owner/structured-claim` |
| `liquidity` | `dragons-clutch/semantic-owner/liquidity` |

Every row carries these exact fields:

- `slot` and `owner`, from the table above;
- `linkage`, exactly `linked` or `planned`;
- a nonempty canonical `semantic_version`; and
- `semantic_digest_sha256`, exactly 32 nonzero bytes encoded as lowercase hex.

`planned` does not mean unspecified. It binds a version and digest for the
frozen semantic contract being planned, but says that contract is not linked
into the measured ELF. Changing its digest, version, linkage state, or owner
changes the profile identity. A future component owner must separately document
how its semantic digest is derived; this checker validates the binding and does
not pretend that a caller-provided digest proves its own provenance.

## Canonical profile identity

The manifest schema is
`dragons-clutch/capability-profile-manifest/v1`. Unknown keys refuse, and the
JSON loader refuses duplicate object keys before normal decoding can erase
them. `release_declaration` must be `false` because this is a release-manifest
input, not a release.

The profile identity is SHA-256 of compact, key-sorted UTF-8 JSON containing
exactly:

```text
domain = dragons-clutch/capability-profile-identity/v1
profile label
the seven canonical capability rows
the three artifact-budget ceilings
```

The ceilings are `max_elf_bytes`, `max_text_bytes`, and
`max_total_loader_rent_lamports`. They are policy limits, not measurements. No
measurement value is hard-coded or copied into this document. Binding the
limits prevents a budget relaxation from retaining the old profile identity.

The measurement is excluded from this semantic identity: a reproducible build
record binds the resulting ELF back to the identity. A release manifest would
then bind both the profile identity and exact ELF digest; this checker does not
replace that release step.

## Planned, historical, and linked measurements

`artifact_budget.measurement_class` has three fail-closed states:

| Class | Meaning | Deployment eligibility |
| --- | --- | --- |
| `planned` | No measurement exists; both evidence fields must be null. | Never |
| `historical` | A schema-V1 measurement can be compared with the declared ceilings, but it predates the seven-slot identity. | Never |
| `linked` | Schema-V2 measurement evidence names the exact computed profile identity and records a clean source closure. | Possible |

The dated evidence at
`programs/clutch-sbf/audit/evidence/2026-08-22-capability-profiles.json`
remains unmodified schema-V1 historical evidence. Its exact recorded ELF,
text, and loader-rent measurements remain useful, but it cannot be relabelled
as linked: it has no seven-slot profile-identity field. The gate deliberately
refuses that substitution. This preserves the measurements without pretending
they cover components developed afterward.

For either historical or linked evidence, the checker selects exactly one
named profile and requires exactly two runs numbered one and two. ELF digest,
ELF bytes, text bytes, and total loader rent must match across the runs.
Measurements are compared directly against all three manifest ceilings; an
exact boundary passes and any excess refuses. The checker also recomputes total
loader rent from each record's loader model, ELF bytes, program-account bytes,
ProgramData metadata bytes, and per-byte rent instead of trusting the recorded
total. A linked record additionally requires:

- measurement schema
  `dragons-clutch/capability-profile-measurement/v2`;
- `manifest_input_source_clean: true`;
- `release_declaration: false`; and
- `capability_profile_identity_sha256` equal to the identity recomputed from
  the manifest.

Evidence paths must be canonical repository-relative paths. Absolute paths,
parent traversal, missing files, and symlink escapes refuse.

## Deployability rule

A structurally valid planning manifest may mix `linked` and `planned` rows so
CI and review tooling can state the residue precisely. It is never deployment
eligible.

A manifest classified `deployable` passes only when:

1. all seven semantic components are `linked`;
2. identity-linked schema-V2 measurement evidence is present and reproducible;
3. all three measured quantities are within the identity-bound ceilings; and
4. the manifest still says it is not a release declaration.

Use `--require-deployable` for a deployment-candidate CI gate. Without that
flag, the checker accepts a valid planning manifest and reports its linked and
planned slots separately.

```sh
python3 programs/clutch-sbf/scripts/check_capability_profile.py \
  path/to/profile-manifest.json

python3 programs/clutch-sbf/scripts/check_capability_profile.py \
  --require-deployable path/to/profile-manifest.json

python3 -m unittest \
  programs/clutch-sbf/scripts/test_check_capability_profile.py -v
```

The tests use explicitly synthetic fixture digests and arithmetic. They do not
create new ELF evidence or assert new program sizes. One test reads the dated
V1 evidence in place and proves that it remains comparison-only.

## Non-claims and next join

Passing this gate proves manifest consistency, canonical identity derivation,
measurement linkage, reproducibility of the selected recorded fields, and
budget comparison. It does not prove that the semantic digests were correctly
derived, that an adapter faithfully implements them, that tests exercised the
ELF, or that any program is safe, deployed, official, or released.

No concrete profile manifest is added by this lane because the current ELF
measurement schema cannot bind these seven components and several components
remain outside the active SBF adapter. The next legitimate join is to teach a
fresh measurement producer to emit schema V2 with the recomputed profile
identity, then measure a clean exact source closure twice. Historical sizes
must not be copied forward into that record.
