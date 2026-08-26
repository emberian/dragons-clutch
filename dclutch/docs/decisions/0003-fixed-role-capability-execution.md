# Decision 0003: fixed-role capability execution

Status: accepted as the safe V1 convergence profile on 2026-08-25 and reopened
for principled generalization on 2026-08-25. This is an architecture and wire
decision, not release or deployment evidence. It fixes canonical state/effect
authority for the current vertical; it is not a permanent claim that every
future execution strategy must be interpreted or that no separately admitted
accelerator can execute the same semantic descriptor.

The active generalization target keeps Trading as the canonical owner and
commit boundary while allowing a future Registry-authenticated, stateless AOT
accelerator to consume the same content-addressed descriptor and return the
same checked effect plan. Such an accelerator must be translation-validated
against that descriptor and may not create a second state, claim, custody, or
release authority. A genuinely state-owning sixth role still requires a new
measured release-set profile and authority decision.

## Context

`ExecutionReleaseSetV1`, the Registry activation cache, its role receipt, and
the checked multiprogram release tool all describe exactly five replaceable
execution roles: Core, Claims, Trading, Resolution, and Custody. A Market
selects one immutable release-set identity. There is therefore exactly one
current Trading `(Program, artifact release)` pair for that Market.

The experimental standalone General, Dealer, and Series SBF adapters each act
as though that one Trading slot could instead name their own Program and
ProgramData. Those Programs cannot all be authenticated by one release set.
Series is worse: its current private caller PDA proves only the unregistered
Series Program ID while its semantic release admission reauthenticates Core.
No checked authority binds the Program that owns Series, Template, and Ticket
accounts.

`CapabilityEntryV1.release_id` cannot repair this mismatch. It is already the
semantic implementation-release identity selected by the immutable capability
manifest. Reinterpreting it as a Program, artifact-release record, or mutable
Registry lookup key would give one persisted field multiple meanings.

The accepted multiprogram research decision is one data-driven Trading
controller, not one Program per venue family or historical action. No measured
result since that decision establishes a distinct syscall or canonical-state
ownership boundary for the new family Programs.

## Decision

General, Dealer, Direct, Series, and later data-defined venue families execute
inside one Registry-selected Trading Program. Their family meaning remains
content-addressed semantic data. Claims remains the sole claim/replay owner;
Custody remains the Realm-selected transfer boundary; Core remains the Market
identity/lifecycle owner.

There is no dynamic capability-Program registry and no second activation
cache. `ExecutionReleaseSetV1`, `ActivatedExecutionReleaseSetV1`,
`AuthenticatedRoleReceiptV1`, and `CheckedExecutionReleaseSetV1` remain
five-role objects. The selected executable for every data-defined capability
is always:

```text
Market.selected_release_set
  -> Registry activation cache
  -> ExecutionRoleV1::Trading
  -> exact Trading Program + checked artifact release
```

The capability manifest and the release set own disjoint facts:

| Fact | Sole semantic owner |
| --- | --- |
| Capability presence and uniqueness | authenticated capability manifest |
| Kind, semantic capability release, config, dependencies, funding | exact manifest entry |
| Core/Claims/Trading/Resolution/Custody Program and artifact | execution release set |
| Current ProgramData deployment and interpreter semantic release | Registry activation cache plus current role receipt |
| Family request and transition meaning | finalized capability-release descriptor selected by `entry.release_id` |
| Claim/replay mutation | Claims role |
| Collateral transfer | Custody role |

The Trading artifact's `ArtifactReleaseV1.semantic_release_id` identifies the
generic Trading interpreter release. It is deliberately not required to equal
a capability entry's family `release_id`. One interpreter release may execute
many capability releases; each capability release remains separately
authenticated content.

## Exact capability-selection wire

`CapabilityExecutionSelectionV1` is a 144-byte derived activation/closure
request prefix. Its sole byte-layout owner is
`DClutchSemantics/CapabilityExecutionAbi.lean`; the Lean emitter produces the
Rust width and offsets atomically, and `dclutch-release-set-contract` provides
the safe typed view:

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | magic `DCLTCER1` |
| 8 | 2 | schema version `1`, little-endian |
| 10 | 2 | artifact profile `1`, little-endian |
| 12 | 2 | canonical manifest entry index, little-endian |
| 14 | 2 | reserved zero |
| 16 | 32 | Market-selected capability-manifest content ID |
| 48 | 32 | exact entry `kind_id` |
| 80 | 32 | exact entry semantic `release_id` |
| 112 | 32 | exact entry `config_id` |

All four identities are nonzero. The wire contains no role, Program,
ProgramData, artifact release, family tag, or caller-selected dispatch value.
Its `executor_role()` is definitionally `Trading`, and its executable binding
is definitionally `release_set.binding(Trading)`.

Core rebuilds the prefix from the hostile-decoded Market-selected manifest and
the exact indexed entry. The wire's `u16` index is not independently bounded;
the authenticated manifest's actual count is the sole bound. This avoids
duplicating the manifest profile's provisional entry limit.

For a Core-to-Trading capability activation or closure, the role-owned request
bytes are:

```text
CapabilityExecutionSelectionV1(144)
  || FundingListHeaderV1(16)
  || exact family request
```

The funding header carries only magic, version, count, and canonical reserved
bytes. The selected entry index remains solely in the selector. There is no
caller-supplied funding-key or entry-index vector and no second funding-list
record.

`CoreEffectEnvelopeV1.target_role` must be Trading for `ActivateCapability`
and `CloseCapability`. `role_request_bytes` and `role_request_digest` bind the
complete selection-plus-family request. The existing full-effect digest then
binds that exact request to release set, Market, generation, context, parent
state digest, and replay revisions. `CoreEffectAckV1` needs no duplicate family
fields: its immediate producer, Trading Program, release set, Market, context,
and full-effect digest authenticate the complete selection.

The 144-byte selector is not a prefix on hot General, Dealer, Direct, or Series
actions. Successful activation persists the exact manifest, entry, kind,
semantic release, and config selection (or its one canonical digest) in the
Trading-owned child root. A hot action authenticates that root's owner, PDA,
canonical bytes, selection, and Market/release-set join. Repeating a
caller-supplied selector would waste packet space and create a second apparent
authority for the already-persisted selection.

The authenticated `FundingStateV1` accounts are the funding list. Core reads
the first `count` standard funding accounts, hostile-decodes each account's
authoritative `entry_index`, requires those indices to be strictly increasing,
derives each PDA from Market, generation, manifest, and entry under the fixed
Trading role, validates owner, custody, status, and pairwise distinctness, and
requires membership of the selected entry. Each family request, descriptor,
and account profile must additionally bind and derive the exact funding
accounts that its semantics consume, and the Trading child rechecks the same
16-byte header. Packet tests compile the maximum-account activation as a full
versioned transaction and prove the resulting wire remains within the runtime
packet limit; they do not merely add instruction-byte widths.

## Capability descriptor and one composite root

`CapabilityEntryV1.release_id` is the SHA-256 digest of every canonical byte of
one finalized, account-resident `CapabilityProgramV1`. Activation carries the
descriptor account, not its bytes in instruction data. The raw-record owner,
PDA, schema release, finalized status, complete digest, and absent staging
cursor must all be authenticated before decoding. Descriptor schema V1's
current artifact profile 2 admits only runtime-width TransitionVM `ProgramV2`;
the former V1 fixed-bank body is not an alternate decode path. Under the
existing 1,312-byte finalized-record ceiling, the 280-byte header plus V2's
16-byte header and 24-byte instructions admits at most 42 instructions, for an
exact maximum canonical descriptor of 1,304 bytes. The current 768-byte
record-page profile publishes that maximum as two Append transactions rather
than claiming it fits one activation instruction.

Trading owns exactly one child-root account for the selected capability:

```text
CapabilityRootHeaderV1(232 immutable)
  || descriptor-defined mutable root-state tail
```

The header stores Market, generation, selected release set, and the exact
authenticated activation selector. It is an immutable projection of the
manifest selection, not a second semantic owner. The descriptor's explicit
`root_state_bytes` fixes the tail width and the whole account must be exactly
`232 + root_state_bytes` bytes. The manifest entry's `child_schema_id` equals
the descriptor's `root_schema_id` and identifies the mutable tail schema; the
common header schema is implied by the checked Trading artifact/profile and is
not a competing manifest fact. Activation creates and rents this one composite
account, family code receives only its mutable tail, hot actions authenticate
the unchanged header, and closure deletes this one account.

## Open family dispatch without a family enum

Neither `kind_id` nor `release_id` is matched against a permanent Rust enum of
General, Dealer, or Series. The Trading interpreter authenticates the finalized
semantic preimage whose content identity is `selection.capability_release()`.
Successor-ready family semantics compile to one canonical, versioned
capability-program/descriptor schema containing the request schema, mutable
root-tail schema and exact width, account profile, derivation policy, checked
transition program, and allowed effect schema. Trading V1 hostile-decodes that
descriptor and interprets its bounded program over authenticated fixed-layout
views. The descriptor defines semantics and allowed effects independently of
execution strategy; this decision does not preclude a later measured,
Registry-authenticated and translation-validated stateless accelerator from
executing the same descriptor without acquiring Trading state/effect authority.

A family that still requires a family-specific Rust dispatcher has not crossed
the successor gate. Its standalone ELF may remain an explicitly experimental
measurement artifact, but it is not a Market-authorized successor Program.
Likewise, a generic dispatcher followed by a compiled list of family-specific
schema IDs, AccountProfile projectors, derivations, or effect handlers has not
crossed that gate merely because it removed the family enum. The initial
`SupportedContentV1` API is a fail-closed physical-profile foundation, not
proof of open-family convergence. Final admission requires finalized and
interpreted AccountProfile, derivation, and effect-projection languages—or an
equivalently certified AOT profile—so those identities select authenticated
semantic data rather than named Rust cases.

Distinct PDA domains and state schemas remain valid. They are derived under
the one Trading Program, with the manifest, kind, release, config, Market, and
family replay coordinates included wherever required to prevent cross-family
aliasing. Separate semantic modules do not imply separate executable Programs.

## Authority flow

1. Core authenticates its Market state, the selected release-set ID, and the
   capability-manifest record and content identity.
2. Core decodes the exact indexed entry and reconstructs the 144-byte selection
   prefix. It does not accept any Program coordinate from the request.
3. Core authenticates the Registry-owned cache PDA for the Market's selected
   release set and invokes `Reauthenticate(Trading)` against the exact Trading
   Program and ProgramData.
4. Core derives `CallerAuthoritySeedsV1` with caller role Core and the digest of
   the complete selection-plus-family request, then invokes only the selected
   Trading Program.
5. Trading independently reauthenticates Core before trusting the Core PDA,
   rejoins the selection to the Market and manifest entry, authenticates the
   finalized capability-release descriptor whose exact digest is the selected
   capability release, and interprets it under the V1 strategy.
6. Trading derives child requests rather than accepting plans. For Claims or
   Custody CPI it uses `CallerAuthoritySeedsV1` with caller role Trading. Each
   child independently reauthenticates Trading and consumes its own canonical
   request and receipt.
7. Trading returns `CoreEffectAckV1` immediately. Core checks the producer and
   every echo, including the full-effect digest, before commit-last state.

Series-to-Core calls use the same fixed role. Series Template/Ticket semantics
and state become Trading-owned data. The Trading Program signs the canonical
release-set caller PDA with caller role Trading; Core reauthenticates the
current Trading deployment before trusting it. The private Series caller-PDA
domain and a separately owned Series Program are superseded.

## Required refusals

The converged implementation must include adversarial coverage for:

- truncated, extended, wrong-magic, wrong-version/profile, nonzero-reserved,
  or zero-coordinate capability selections;
- an entry index outside the authenticated manifest count;
- substituted manifest, kind, capability release, or config at the same index;
- an envelope targeting anything other than Trading for generic capability
  activation or close;
- omitted selection prefixes and request length/digest substitutions;
- a caller-supplied Program, a family Program, or a Program different from the
  activated Trading binding;
- substituted release set or activation-cache PDA, wrong cache owner, stale or
  changed ProgramData deployment, wrong return-data producer, artifact release,
  or interpreter semantic release;
- equating a capability semantic release ID with an artifact-release ID or
  treating either as the other's record schema;
- malformed, noncanonical, unknown-version, or content-hash-mismatched family
  descriptors and configs;
- zero, oversized, truncated, extended, or descriptor-mismatched mutable
  root-state tails, and any attempt to rewrite the immutable root header;
- missing, duplicate, unordered, wrong-owner, wrong-PDA, wrong-entry,
  wrong-custody, wrong-status, or selected-entry-omitting FundingState accounts;
- a family identifier that would require a hard-coded dispatcher rather than
  the admitted generic descriptor schema;
- arbitrary signers, PDAs derived under an unregistered family Program, wrong
  caller role, wrong request digest, or wrong context;
- child effects outside the descriptor's allowed effect schema, substituted
  Claims/Custody Programs, stale replay revisions, overwritten return data, or
  acknowledgment digest substitution;
- legacy Series-owned Template, Series, or Ticket accounts after Trading-owned
  derivations become canonical; and
- any refusal after an earlier write or CPI, with transaction-wide rollback
  checked byte-for-byte.

## Convergence file plan

The accepted implementation sequence is:

1. Keep and extend
   `crates/dclutch-release-set-contract/src/lib.rs` and its README with the
   fixed Trading selection projection. Own its layout in
   `formal/dclutch-semantics/DClutchSemantics/CapabilityExecutionAbi.lean`,
   emit it with `formal/dclutch-semantics/EmitCapabilityExecutionAbiRust.lean`,
   and consume only the generated constants in
   `crates/dclutch-release-set-contract/src/generated_capability_execution.rs`.
   Keep the five-role release-set wire.
2. Keep `crates/dclutch-registry-contract/src/activation.rs`,
   `programs/dclutch-registry-sbf`,
   `crates/dclutch-release-tool/src/multiprogram.rs`, and
   `crates/dclutch-operator/src/release_activation.rs` five-role. Add hostile
   tests and documentation proving no capability Program can be injected.
3. Change
   `formal/dclutch-semantics/DClutchSemantics/MarketCorePhysicalAbi.lean`,
   `crates/dclutch-market-core-codec/src/physical.rs`, and their generated
   constants/tests so generic capability activation and close target Trading
   only and hash the complete selection prefix plus family request.
4. Implement `programs/dclutch-core-sbf/src/capability.rs` to reconstruct the
   selector from the actual Market-selected manifest entry, reauthenticate
   Trading, invoke it, and validate its exact acknowledgment.
5. Create one `programs/dclutch-trading-sbf` with data-driven descriptor
   admission and interpreter dispatch. Move General, Dealer, Direct, and Series
   adapter modules there only after each no longer dispatches on a closed
   family or outcome-count enum.
6. Retain the family semantic contracts, Lean definitions, generated codecs,
   hostile corpora, and exact state schemas as semantic data owners. Program
   convergence is not semantic deletion.
7. Replace `SeriesCoreCallerSeedsV1` and
   `SERIES_CORE_CALLER_AUTHORITY_PDA_DOMAIN_V1` in
   `crates/dclutch-market-core-codec/src/physical.rs` and the owning Lean ABI
   with the release-set-owned Trading caller authority. Core must reauthenticate
   Trading on this incoming route.
8. Delete `programs/dclutch-general-sbf`,
   `programs/dclutch-dealer-sbf`, and `programs/dclutch-series-sbf` in the same
   convergence cycle that lands the complete Trading vertical. Remove their
   workspace members, standalone lockfiles, build scripts, validator entries,
   and any README language calling them selected Trading Programs.
9. Update `tools/local-validator/bootstrap/successor` to build and activate one
   Trading artifact and to emit capability descriptors separately. Do not
   synthesize per-family Registry bindings.
10. Delete or rewrite any checked-release/operator fixture that supplies more
    than one Program as `ExecutionRoleV1::Trading`. The one remaining Trading
    checked release binds the interpreter artifact; manifest entries bind the
    separate family semantic releases.

No standalone family route is deleted before the one Trading vertical owns its
state, authenticates the release/manifest/config chain, executes its canonical
child effects, reports honest operator status, and passes hostile rollback and
footprint measurement. Once that vertical is accepted, the superseded Program
authority path is deleted rather than retained as a fallback.

## Rejected alternative

A canonical dynamic map from capability release to arbitrary Program would
make every capability family a new deployment authority class, enlarge the
activation cache and checked release set with a provisional dynamic bound, and
duplicate the manifest's release selection. It would also preserve the current
family-specific Rust dispatch and one-Program-per-family drift without evidence
of a distinct syscall or canonical-state boundary. This repository therefore
does not add that map.

If a future capability proves it must own state under a distinct Program—for
example because the runtime gives that Program an exclusive syscall or token
authority—the result requires a new measured architecture decision and a new
release-set profile. It is not smuggled into `CapabilityEntryV1.release_id` or
the V1 Registry cache.
