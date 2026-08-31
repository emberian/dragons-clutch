# Resolution recurring deployment authentication and runtime syscall audit

Date: 2026-08-29

Status: source-bound implementation evidence plus a static follow-on audit. This
is not release admission, deployment evidence, or a mainnet execution claim.

## Result

Resolution no longer hashes a complete Loader V3 ELF or searches again for the
Loader ProgramData PDA on any of its fifteen recurring deployment-observation
call expressions. Registry first admission remains the sole owner of the full
ELF hash. Every recurring Resolution path now calls
`dclutch_registry_activation_auth_v1::cached_role_deployment_observation_v1`
through the local `cached_deployment_observation` mapping.

The change covers activation-cache roles and the Registry ArtifactRelease named
by the immutable infrastructure profile. It does not replace or weaken the
finalized record, profile, release, or activation-cache authority that selects
the release.

The focused hbox build initially found a real, independently measured SBF stack
violation in `core_effect::process_direct_funding_close_v1`: 4,160 bytes, 64
bytes beyond the 4,096-byte bound. Projecting the already-authenticated
`CoreState` down to the only two facts consumed after authentication repaired
the frame to 3,840 bytes. The final plain link and independent frame build pass.

**No caller-level CU measurement was run. This document makes no CU-margin,
transaction-margin, or under-1.4M-CU claim.** Static removal of large hashes,
PDA searches, copies, and CPIs identifies likely payoff and measurement targets;
it is not a substitute for an actual caller execution measurement.

## Exact source

The final evidence source is:

| Field | Value |
| --- | --- |
| commit | `2b390cbaf05ebf184ac1b51dde3b9e49c01e8765` |
| Git tree | `c34f03e168a816b0c32fb9f7a7f947ae5c65cabb` |
| source archive SHA-256 | `d175223895c075d346eaf66743a23ecbe83e3b8ebcfd9039cf2a713d07f52140` |
| hbox evidence root | `/tank/dregg-build/dclutch-resolution-2b390cba-vinjhq` |

Relevant commits in the source lineage are:

| Commit | Exact contribution |
| --- | --- |
| `231ad047` | Added the shared activation-auth dependency and converted the ten recurring Core-effect, pre-Market funding, and funding-abort observations. Added cache, ProgramData, release, authority, supersession, and exact caller-census tests. |
| `295c2031` | Converted the remaining five provider transport/instruction call expressions, retained finalized infrastructure ArtifactRelease authentication, added a hostile profile-release identity test, and removed the now-unreachable local full-ELF observation implementation. |
| `c756ff02` | Diagnostic attempt to isolate the shared observation call with `inline(never)`. The hbox frame remained exactly 4,160 bytes, so this marker was removed by `2b390cba`; it has no net effect in the evidence tree. |
| `2b390cba` | Replaced the post-authentication full `CoreState` copy in direct close with `DirectCloseMarketFacts { terminal_winner, product_record }`, using `request.generation` only after exact equality with authenticated Market state had already been proven. This repaired the stack frame. |

The preceding protocol-wide audit also landed `1e3e1159`, which routes the live
Fractional V2 exposure request digest through `dclutch-sha256-adapter`. It is not
part of the Resolution ELF change, but it closes the one additional reachable
software-SHA boundary found during the same audit.

## Toolchain and commands

The final build ran on hbox, a shared build node, with the complete command
under `swarm-build` as required by `AGENTS.md` and `WAVE.md`.

| Component | Exact version |
| --- | --- |
| `cargo-build-sbf` | `4.0.0` |
| platform-tools | `v1.53` |
| SBF rustc | `1.89.0` |
| memory wrapper | `swarm-build`, `SWARM_MEM_MAX=32G`, `CARGO_BUILD_JOBS=4` |

Plain link shape:

```text
swarm-build cargo build-sbf \
  --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  --sbf-out-dir <evidence-root>/deploy -- --locked
```

Independent frame-measurement shape:

```text
RUSTC_BOOTSTRAP=1 \
RUSTFLAGS='-Zemit-stack-sizes --emit=obj,link' \
CARGO_TARGET_DIR=<evidence-root>/frame-target \
swarm-build cargo build-sbf \
  --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  -- --locked
```

The source was an exact `git archive` of the named commit, not a dirty-tree
copy. No RPC, signing, deployment, or push occurred.

## Final artifacts

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `dclutch_resolution_proof_sbf.so` | 804,888 | `61c1548a822e4935ca10b27b88989582c1ba6c874fb808e7de3eaeacb57f6f6c` |
| plain build log | — | `92a57e203f91ab6fe10533ce560562c94577c8552d3882c19016f47aa1132e2f` |
| frame build log | — | `875d79a74ae8498fd8b34a4c8d984be33cd2fd3b0fc5da19ab8b078ebaf8b494` |
| measured Resolution object | 543,624 | `c256c65799ce7bba874d902f9c4f9e98e4248d0eaf68ce0c54d0b999401d1c17` |

The plain and frame builds emitted zero `overwrites values in the frame`
diagnostics for the final source.

## Frame evidence and repair

The first exact build of the combined cached-observation source, `295c2031`,
reported:

```text
process_direct_funding_close_v1: 4160 bytes
maximum allowed frame:           4096 bytes
overage:                           64 bytes
```

An exact `c756ff02` rebuild proved that merely adding a local non-inlining
boundary did not change the frame: it remained 4,160 bytes. That ineffective
marker was not retained.

`2b390cba` changed the lifetime and representation of authenticated Market
facts, not their authority. `authenticate_direct_close_market` still
hostile-decodes the complete `CoreState` and still checks Market owner, state digest,
identity, generation, Registry, release set, Source material, capability
manifest, phase, readiness, terminal receipt, beneficiary, and canonical Market
PDA. Only after those checks does it return:

```text
terminal_winner
product_record
```

The close step uses `request.generation` after the authenticator has proved it
equal to `state.identity.generation`. This avoids carrying the full authenticated
state through the outer function and does not establish a second generation
authority.

The final independent frame report measured 165 frames:

| Function | Frame bytes | Spare to 4,096 |
| --- | ---: | ---: |
| `core_effect::process_direct_funding_activation_v1` | 4,032 | 64 |
| `pre_market_funding_abort_v1::authenticate_release_and_caller` | 3,968 | 128 |
| `sponsored_push_v1::boxed_source_records` | 3,968 | 128 |
| `provider_transport_v3::process_submit` | 3,840 | 256 |
| `core_effect::process_direct_funding_close_v1` | 3,840 | 256 |
| `provider_v3::plan_provider_resolution_v3` | 3,712 | 384 |
| `sponsored_push_v1::process_capture` | 3,648 | 448 |
| `pre_market_funding_abort_v1::process_pre_market_funding_abort_v1` | 3,456 | 640 |
| `relay_transport_v1::process_consume` | 3,392 | 704 |
| `entrypoint` | 3,136 | 960 |
| `provider_instruction_v3::process_provider_resolution_v3` | 3,136 | 960 |

The direct-close repair is therefore a measured 4,160-to-3,840 frame change,
not an inferred source-level estimate.

## Exact fifteen-call-expression census

The permanent source test
`tests::recurring_resolution_deployment_auth_caller_census_is_exact` freezes
the following fifteen recurring call expressions. Loop expressions are counted
once in this static census even when one invocation authenticates multiple
roles.

| File and symbol | Selected deployment(s) | Static expressions |
| --- | --- | ---: |
| `core_effect.rs::authenticate_direct_close_release` | Core, Resolution | 2 |
| `core_effect.rs::authenticate_direct_activation` | Core, Resolution | 2 |
| `core_effect.rs::authenticate_activation` | Core, Resolution | 2 |
| `pre_market_funding_v1.rs::authenticate_release_and_caller` | Trading, Resolution | 2 |
| `pre_market_funding_abort_v1.rs::authenticate_release_and_caller` | Trading, Resolution through `slot_pinned_deployment_observation`, which delegates to the shared cached helper | 2 |
| `provider_transport_v3::authenticate_infrastructure` | infrastructure-profile Registry release; one loop expression authenticates Core and Resolution | 2 |
| `provider_transport_v3::authenticate_reclaim_release` | Resolution | 1 |
| `provider_instruction_v3::authenticate_market_and_infrastructure` | infrastructure-profile Registry release | 1 |
| `provider_instruction_v3::authenticate_activation_and_caller` | one loop expression for Resolution and the active caller role | 1 |
| **Total** | | **15** |

The source census also asserts that these files contain no additional plain
`deployment_observation(` call expression. Resolution's former local full-ELF
helper was removed after the last provider caller moved.

## Preserved authentication semantics

The recurring helper accepts only a release already selected by either the
hostile-decoded Registry activation cache or the finalized infrastructure
profile's exact ArtifactRelease record. It then preserves all of these facts:

1. the release's Program identity and supplied Program account identity;
2. Loader V3 ownership of Program and ProgramData;
3. Program executable and ProgramData non-executable dispositions;
4. exact release ProgramData identity and supplied ProgramData account identity;
5. the Loader-owned Program account's stored ProgramData link;
6. explicit three-way equality:
   `Program stored link == release ProgramData == supplied ProgramData`;
7. release Loader identity;
8. deployment slot;
9. exact upgrade policy and upgrade authority;
10. the activation-admitted ELF digest;
11. named `ReleaseSuperseded` behavior for a strictly forward slot move under
    `ExactAuthority`.

A different authority, revoked authority, lower slot, wrong owner, wrong
executable disposition, substituted ProgramData, substituted release, or
substituted Program link remains the generic Resolution deployment refusal. It
is not mislabeled as an expected upgrade.

The redundant Loader ProgramData PDA search was removed only because the
Loader-owned executable Program account already stores the link and the helper
keeps the full three-way equality above. First admission deliberately retains a
canonical Loader PDA derivation and complete ELF hash in
`programs/dclutch-registry-sbf/src/lib.rs::deployment_observation`.

Adversarial tests cover:

- a canonical release projected from an activation cache;
- a mutated activation-cache release;
- substituted ProgramData identity;
- substituted release ProgramData identity;
- substituted Loader Program link;
- exact-authority forward slot supersession;
- authority substitution at the pinned slot;
- finalized infrastructure-profile ArtifactRelease identity substitution; and
- the exact fifteen-call-expression census.

Focused native result at the evidence source: 41 passed, 0 failed.

## Remaining prioritized runtime/syscall map

Payoff labels below are qualitative static priorities. They are not CU
measurements.

### P0 — authenticate multiple roles from one decoded activation cache

Exact owner:

- `programs/dclutch-core-sbf/src/release.rs::authenticate_roles`
- Claims `affine_batch_v2::authenticate_releases`
- Claims `signed_delta_v3::authenticate_releases`
- Claims `founding_v5::authenticate_releases`
- Claims rational lifecycle/representation release helpers
- Claims market-closure, generic, and protocol-position release helpers
- Custody retirement replay handoff

Core already borrows and hostile-decodes the cache, derives its canonical
address, verifies Registry ownership, computes the cache digest, and builds the
selected projection once. It then calls `authenticate_activated_role_v1` once
per requested role; every call repeats the canonical cache PDA search and cache
decode. Several Claims routes have the same shape.

Safe plan: while one `ActivatedExecutionReleaseSetViewV1` borrow is live,
authenticate cache address, owner, flags, release-set identity, and canonical
role order once, then call
`authenticate_activated_role_in_cache_v1` for each role. Preserve the batch
mask, strict ascending role tags, duplicate-role refusal, release projection,
and every deployment fact. The existing first-role/bump and with-bump APIs are
a smaller improvement where keeping one decoded view is not yet practical.

Likely payoff: very high on multi-role routes; removes `R-1` canonical bump
searches plus repeated full cache decoding.

### P0 — replace Trading's repeated Registry reauthentication CPIs

**CLOSED 2026-08-30 (lane CACHEREAD, `f04654a0`).** All three owners converted;
`RegistryInstructionV1` no longer appears in Trading's code at all. This item's
payoff label was "very high" and unsized; it is now **66,921 CU measured** at the
top-level Direct route's key-independent floor — 52,592 for the CPI pair and
about 14,300 for the third cache decode this section correctly named ("Registry
then searches and decodes the same cache again") and did not size. See
`docs/decisions/0017-cache-read-role-authentication.md` §9. The "preserve exact
return-receipt comparisons" instruction was followed and then some: two of the
three comparisons became constructions of a local receipt, and the cache address
is now derived from the Market's selected release set rather than the cache's
own, which is a check the CPI could not make.


Exact owners:

- `programs/dclutch-trading-sbf/src/outer.rs::reauthenticate_role`, called for
  Core and Trading in both `process_activation` and `process_close`;
- `direct_begin_retiring_v1.rs::reauthenticate_role`, called for Core and
  Trading; and
- `hot_v3.rs::reauthenticate_role`, called for Core and Trading on the
  non-continuation arm.

Each helper finds the activation cache, allocates `Instruction` account/data
vectors, invokes Registry, and reads a receipt. Registry then searches and
decodes the same cache again.

Safe plan: bind Registry identity to authenticated Market state, validate the
Registry-owned cache address and complete projection once, and authenticate the
two roles directly from one decoded view or authenticated bump. Preserve exact
return-receipt comparisons until callers consume a typed local equivalent.
Claims, Custody, Core, and Registry's own handler already establish the direct
cache-authentication semantic precedent.

Likely payoff: very high; removes two CPIs, their account/data vectors,
return-data round trips, and repeated cache searches per affected action.

### P1 — eliminate Claims full-account candidate copies

Exact owners:

- `affine_batch_v2::{build_candidates,apply_rows,resource_digest,commit_candidates}`;
- `signed_delta_v3::{build_candidates,apply_deltas,resource_digest,commit_candidates}`; and
- `sparse_native_transfer_v1::execute_authenticated_transfer` and
  `commit_candidates`.

Affine and signed-delta copy the Market plus every Position into
`Vec<u8>`/`Vec<Vec<u8>>`, mutate the copies, allocate a second vector of
`RefMut`s, and copy every byte back. Sparse native transfer clones its Market,
source Position, and destination Position before the same copyback shape.

Safe plan: finish account identity, PDA, owner, width, revision, duplicate,
arithmetic, and receipt-width preflight first. Acquire every required mutable
borrow before the first write. Retain a compact plan of touched offsets,
prestate atoms, poststate atoms, and revision changes; then mutate in place and
hash the resulting borrowed slices. Any later error must propagate so SVM
rollback preserves transaction failure atomicity. No external CPI may occur
after the first write.

Likely payoff: high and proportional to total Market/Position byte width.

The O(n^2) duplicate-key scans at `affine_batch_v2.rs:366` and
`signed_delta_v3.rs:428` are lower priority. Account counts are bounded, and a
sorted-wire adjacency check would be ABI-breaking.

### P1 — persist authenticated Position/admission bumps through a versioned layout

Exact owners:

- hot searches in `affine_batch_v2::build_candidates` and
  `signed_delta_v3::build_candidates`;
- creation searches in `protocol_position_v2::{allocate_pair,initialize_pair}`;
- V2 Position layout in
  `crates/dclutch-claims-svm/src/liability_basis_state_v2.rs`.

Creation already finds the Position and admission bumps, but later actions
search again for every Position. V2 bytes 120 through 127 are reserved and the
decoder requires them to remain zero.

Safe plan: define a V3 persisted state/migration that writes typed authenticated
bumps during creation and uses `create_program_address` later. Validate exact
seed domains and account identities during migration. Do not reinterpret V2
reserved bytes without a version transition.

Likely payoff: high for wide batches or low canonical bumps.

### P1 — thread runtime digests across one immutable snapshot

Exact first target:

- `programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs` hashes the
  same `lock_raw` in `authenticate_staged_checkpoint_v1`,
  `authenticate_request_coordinates`, and `execute_lock`.

Core retirement also hashes identical request/bundle bytes repeatedly within
several single-instruction paths in `programs/dclutch-core-sbf/src/retire_v1.rs`.

Safe plan: compute one typed digest while the exact immutable borrow or decoded
snapshot remains live and pass it through the internal call graph. Never accept
a client-supplied digest in place of hashing mutable or independently borrowed
bytes.

`solana_program::hash`/`hashv` already use the runtime SHA syscall. There is no
incremental runtime SHA syscall to substitute; the optimization is reuse of a
same-snapshot digest.

Likely payoff: medium and low-risk after exact same-snapshot caller census.

### P2 — version Clock and Rent account wires to syscall getters

Static census of `Clock::from_account_info` and `Rent::from_account_info` in the
seven permanent links:

| Link | Static decodes |
| --- | ---: |
| Registry | 3 |
| Rent | 1 |
| Core | 17 |
| Claims | 14 |
| Trading | 23 |
| Resolution | 2 |
| Custody | 11 |
| **Total** | **71** |

Safe plan: version one route family at a time to `Clock::get`/`Rent::get`,
remove the corresponding account from the exact wire, and regenerate operator,
frame, packet, and hostile tests. This is an ABI/account-profile change, not a
mechanical source replacement. The Instructions sysvar has different semantics
and is not eligible for this substitution.

Likely payoff: medium, principally account-key, packet, borrow, and decode
pressure; compute impact requires measurement.

### P2 — share the owned-CPI membrane beyond Trading Hot

Existing precedent:

- `programs/dclutch-trading-sbf/src/entrypoint_adapter.rs::invoke_signed_owned_v1`.

Remaining examples:

- Registry continuations;
- Core retirement replay handoff;
- Claims custody replay and rational terminal;
- Custody projected calls;
- Resolution pre-Market funding; and
- several non-Hot Trading composition paths.

Those paths still allocate `Vec<AccountMeta>`, `Vec<AccountInfo>`, or copied
instruction data and then pass an `Instruction` to the SDK invoke path, which
constructs its stable representation from another clone.

Safe plan: extract a separately named SBF runtime adapter/trust boundary using
the exact SDK stable layout, with native-vs-SBF borrow/alias differential tests,
privilege-escalation hostiles, and independent frame evidence. Migrate only
measured heap-critical callers. Fixed arrays alone do not remove the SDK's
internal `Instruction` clone.

Likely payoff: medium to high heap reduction; compute effect requires caller
measurement.

## Software SHA reachability conclusion

After `1e3e1159`, no software-SHA site found by this audit is reachable from a
permanent production SBF dispatch:

- the five `crates/dclutch-fractional-claim-contract/src/hot_v2.rs` sites are
  `FractionalHotCandidateV2::{validate_signed_delta_receipt,
  validate_terminal_receipt}`, private `validate_signed_delta`, private
  `validate_terminal`, and private `digestv`; only contract tests construct the
  candidate;
- old V1 digest helpers in
  `crates/dclutch-fractional-claims-kernel/src/lib.rs` are reached by operators
  and a test program, not permanent Claims dispatch; and
- other `sha2` hits are test modules or offchain compiler/operator artifact
  construction.

The SBF side of `dclutch-sha256-adapter` calls the runtime hash API; its `sha2`
implementation is the native differential implementation.

## Receipt staging boundary

The audit found no additional atomic value transition that can safely be split
without introducing new escrow, reservation, expiry, cancellation, and refund
semantics. Existing Registry publication/activation, controller funding,
provider submit/execute/reclaim, aggregate retirement, and Dealer checkpoints
are the appropriate staged seams.

Claims, Custody, and Token value changes should remain atomic unless a new
protocol-owned reservation design binds every mutable prestate and supplies a
permissionless cleanup path. A checkpoint that merely caches authentication but
does not lock or revalidate all mutable resources is not sufficient.

## Evidence limits

- Native tests and static frame measurements are not mainnet evidence.
- The hbox ELF is source-bound build evidence, not an official release artifact.
- No validator caller was executed for this change.
- No CU value or transaction margin is claimed.
- No RPC, signing, submission, deployment, tag, publication, or push occurred.
