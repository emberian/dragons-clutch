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
- the Cargo profile feature, source identity, and collateral/claim release identity;
- the expected final undefined dynamic-symbol/syscall surface;
- all twelve semantic-owner rows;
- the central-registry version, digest, exact enabled intent triples, and exact
  linked account coordinates; and
- the exhaustive `dragons-clutch/wire-surface/v1` projection of legacy intent
  pairs, dedicated Direct intent pairs, outer Request actions, and Source
  generation discriminants; and
- ELF, `.text`, chosen ProgramData `max_len`, and persistent-loader-rent limits.

The twelve semantic-owner slots, in canonical order, are `relation`, `score`,
`price-measure`, `candidate-lifecycle`, `clear-work-feed`, `retirement`,
`source-plane`, `fractional-redemption`, `series-products`, `recovery`,
`structured-claim`, and `liquidity-dealer`. Every row binds an owner name,
version, digest, linkage state, required intent triples, and required account
coordinates.

Intent coordinates are `[outer_tag, version, local_action]`. Legacy two-byte
intents use local action zero; successor envelopes use their nonzero
family-local action. Account coordinates are `[tag, version]`. The manifest
pins the central registry's own semantic digest; the Python checker retains
only the decoder partition and retired Source-generation boundaries needed to
prove that this exhaustive projection does not reopen a legacy authority.

`wire_surface` has exactly five fields: `schema`, `legacy_intent_pairs`,
`dedicated_direct_intent_pairs`, `outer_request_actions`, and
`source_generation_discriminants`. The two intent-pair arrays are canonical,
sorted, disjoint projections of every enabled central-registry triple whose
local action is zero. Tags `36..=46` belong only to the dedicated Direct
decoder; all other pairs belong to the hostile current Request/Intent decoder.
The outer Request action surface is exactly `[0, 1, 2]`. Source generation
discriminants are derived from reachable legacy Source pairs: generation 1 for
tags `23..=26` and generation 2 for tags `70..=73`.

The checker returns the validated object as `wire_surface` and also computes
`wire_surface_sha256` over a canonical object domain-separated by
`dragons-clutch/wire-surface-identity/v1`. Both the exact object and this digest
are repeated in linked measurement evidence. The profile identity and the
normalized producer-input manifest digest separately bind the same object; a
client projection cannot substitute its own wire table or digest.

For all `linked` owners, required intent and account coordinates must equal the
top-level enabled/linked coverage exactly. A missing required coordinate and an
enabled coordinate with no linked semantic owner both refuse. Requirements of
a `planned` owner do not become live coverage. A profile with any planned owner
can be described for planning but cannot be measured or deployment-eligible.

The `fractional-redemption` owner is the only owner permitted to require tag
`79`. Its required subset and the central enabled subset must each be either
empty or exactly `79/v1` actions 1 through 10. A linked owner must match the
central subset exactly; a planned owner cannot enable any member. Thus a
profile cannot activate a partial Fractional lifecycle even though all ten
handlers have allocated wire coordinates.

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
| `runtime-real-pyth-release` | none | Checked `profile-successor-chain-attached-v1` identity for a separately authenticated SourceReleaseManifestV2/real-Pyth route; it compiles no fixture row. |
| `non-production-mock-source-lab` | `non-production-mock-source` | Fabricated-provider laboratory ELF. |
| `non-production-real-pyth-lab` | `non-production-real-pyth-lab` | Captured real-program/local-synthetic-observation laboratory ELF. |

The source class and exact Cargo features are in the profile identity. A mock
ELF and a real-Pyth laboratory ELF therefore cannot share a capability identity
even when their base profile feature is the same. Neither laboratory class is
production or network-price evidence.

`runtime-real-pyth-release` is admitted only with
`profile-successor-chain-attached-v1`, which reserves the `77/v2` Source V3
family but currently admits none of its actions. That profile in turn requires
this exact Source identity; `production-inert` cannot be relabeled as the
chain-attached successor. The checked-profile gate refuses both laboratory
identities as deployable; the runtime also gates legacy Source V1 tags
`23..=26` and Source V2 tags `70..=73` on those explicit non-production
features. A release-class ELF therefore has no fixture or legacy Source
fallback.

For the chain-attached successor profile the Source semantic-owner requirements
and enabled central-registry Source subset are both empty. Actions 1 through 12
remain an all-or-none lifecycle: release/request publication, ingestion,
seal/fold/evaluate/handoff, exact Failure/ResolutionV5 terminal authority,
reopen, and physical close must be reachable together before any tuple is
admitted. This prevents actions 1 through 4 from creating Head, Lineage, or Page
state that cannot reach terminal closure, and prevents action 2 from consuming
an unfounded generation request. Every release-class wire surface also has no
legacy Source pair and an empty `source_generation_discriminants` array.

## Collateral and claim release selection

`build_contract.collateral_release_identity` is independent of the Source
selector and is exactly one of:

| Identity | Additional Cargo feature | Meaning |
| --- | --- | --- |
| `production-inert` | none | No observed-positive collateral or claim release is asserted by this build identity. |
| `observed-positive-collateral-and-claim-release` | `observed-positive-collateral-release-manifest` | Select the checked-in positive-slot collateral catalog and the independently checked Token-2022 claim release. |

The observed-positive selector refuses unless the checked source manifest has
at least one collateral release, the same positive number of deployment rows,
and a nonempty independent claim release. The selected Rust module repeats
those conditions as compile-time assertions, so the empty repository template
cannot produce an apparently live ELF. Any profile enabling one Fractional
action must select this observed-positive collateral-and-claim identity; the
separate whole-family invariant then requires all ten actions.

The successor's complete legacy intent projection is version 3 of tags
`2..=5`, `7`, `10`, `11`, `14..=21`, and `68`. This is the current Collateral
value plane, current Direct V4's shared tags `7` and `14`, the Realm/Profile,
artifact, and exact close paths still used by the chain-attached product. Its
dedicated Direct projection is version 3 of tags `36..=46`. No market-founding,
General value/clearing, historical Direct, legacy Source, or Dealer intent is
admitted. The checker compares both arrays to these exact constants after
proving that their union exhausts central-registry local-action-zero coverage;
adding a merely decodable historical DTO therefore cannot silently widen a
checked release.

The separately named
`profile-non-production-dealer-policy-catalog-lab` is a capability profile,
not a source identity. It enables only Dealer successor triples
`(76,1,1..=4)`, binds account coordinates `0x7d/1` and `0x7e/1`, and carries
the `production-inert` source identity because its route reads no source
account. Its local SVM evidence is not deployment eligibility; the manifest
producer remains unavailable until every required semantic-owner row is
linked.

The producer builds `profile-full` explicitly twice. Its explicit feature list
is `custom-heap,default,profile-full`: Cargo's default route enables the named
`default` marker as well as the two features to which it expands. No program
source branches on that marker, but rustc includes the complete enabled-feature
set in crate identity, so omitting it can perturb LTO ordering and produce a
different ELF. Narrow profiles continue to disable defaults and do not enable
the marker, because it would expand to `profile-full`. Only a
`production-inert` full profile is also built once through Cargo defaults, and
the default artifact must equal the explicit artifact while bound to the same
identity manifest. A laboratory full profile is a distinct identity and cannot
reuse that equivalence.

### Non-promotable size diagnostics

[`measure_capability_profile_sizes.py`](../../programs/clutch-sbf/scripts/measure_capability_profile_sizes.py)
is a separate optimization diagnostic for periods when no complete linked
manifest exists or the worktree contains unrelated changes. It accepts only
caller-named `NAME=FEATURE` selectors, exports the selected Git commit through
`git archive` into a commit-keyed deterministic local path, and performs two
fresh explicit builds from that committed tree.
It records exact ELF, section, syscall, final-frame, and exact-size loader-rent
results plus pairwise deltas. Final unstripped symbols also attribute every
`.text` address range to a crate or first-party instruction module. The
diagnostic binds the section base and exclusive end, rejects zero-sized,
out-of-range, overlapping, or gapped symbol regions, and deduplicates only
identical folded aliases. An optional two-build Cargo-default comparison
reports byte identity from the stripped ELF SHA-256 only; its stricter
equivalence gate compares every field returned by the linked producer's
`comparable_measurement`. The
dated current-HEAD diagnostic is
[`2026-08-23-current-head-capability-size-diagnostic.json`](../../programs/clutch-sbf/audit/evidence/2026-08-23-current-head-capability-size-diagnostic.json).

Its schema is deliberately not accepted by `check_capability_profile.py`. The
result is source-derived/selected-commit artifact evidence for size work and a
model-only rent calculation. It records whether the selected commit equaled
repository `HEAD` at measurement. It carries no semantic-owner/registry
linkage and is not runtime, deployment, release, public-cluster, or production
evidence.

## Source and toolchain linkage

The schema-V2 record binds:

- Git commit and tree;
- a canonical tracked-file closure, file count, and path-ordered per-file
  SHA-256 fold;
- the exact linked producer and checker paths, working-byte SHA-256 digests,
  and selected-commit Git blob object identities;
- empty tracked **and untracked** status before and after all builds; and
- version strings and binary SHA-256 digests for `cargo-build-sbf`, platform
  `rustc`, `llvm-readobj`, and `llvm-objdump`.

The identity manifest itself must be tracked and is added to the closure. The
scripts directory and the exact producer/checker files are also in the
closure, so dirty or newly imported first-party measurement code cannot coexist
with `manifest_input_source_clean: true`. The producer refuses any staged,
unstaged, or untracked path in that closure and refuses a file-list or digest
change during measurement. Ignored build output is placed in fresh temporary
targets and is not evidence input.
Commit, tree, producer-blob, and checker-blob object identities must all use the
same native Git object width: uniformly SHA-1 (40 lowercase hex characters) or
uniformly SHA-256 (64). Mixed object formats refuse.

The evidence also records a digest of the producer's canonical planning
manifest. When a checker later reads a deployable manifest, it normalizes only
the classification and evidence-pointer fields back to their planning values
and requires that digest to match. Semantic, registry, wire-surface, build, or
budget changes therefore cannot be hidden behind a new evidence pointer.

## Final artifact evidence

Each profile has two explicit fresh builds. A full production-inert profile
also has the default-equivalence build described above. A matching ELF SHA-256
is byte identity; the stricter linked gate additionally requires every
comparable audit and loader field to agree across the applicable runs. Every
run records and the checker
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

# Current committed-tree optimization diagnostic only; not linked evidence:
python3 programs/clutch-sbf/scripts/measure_capability_profile_sizes.py \
  --profile full=profile-full \
  --profile direct-v3-source-v2-point=profile-direct-v3-source-v2-point \
  --profile general-source-v2-point=profile-general-source-v2-point \
  --profile successor-chain-attached=profile-successor-chain-attached-v1 \
  --cargo-default-profile full \
  --output path/to/size-diagnostic.json

python3 programs/clutch-sbf/scripts/check_capability_profile.py \
  path/to/profile-manifest.json

python3 -m unittest \
  programs/clutch-sbf/scripts/test_check_capability_profile.py \
  programs/clutch-sbf/scripts/test_measure_capability_profiles.py \
  programs/clutch-sbf/scripts/test_measure_capability_profile_sizes.py -v
```
