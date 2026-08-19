# R2 pull-profile promotion plan

Status: **PROPOSED PLAN / DEFAULT RELEASE STOP.** This document sequences the
promotion of the selected Pyth pull profile from research contract
(`docs/implementation/PYTH_PULL_PROFILE_R2.md`, design
`docs/design/SOURCE_PROVIDER_V1_SELECTION.md`) into a runtime route. It
changes no code. The default ELF keeps refusing `SourceReleaseUnavailable`
(`0x79`) until every gate in §5 closes; nothing here authorizes deployment,
public RPC, or live reads. Ground truth for every file:line below is the
runtime-surface map of 2026-08-19 (36 enumerated deltas; the V3 successor
branch touches none of the source-plane files).

## 1. Shape of the change

The V1 source plane is provider-neutral generic kernels
(`source.rs`, `source_archive.rs`) parameterized by two traits, with every
feature gate and the compiled release registry living in
`instructions/source_ingest.rs`. The registry is a function
(`release_registered`) plus two trait impls, not a table. Promotion therefore
has three independent axes:

1. **Spec generation v2** — new 368-byte body, new digest domain
   (`dragons-clutch/feed/v2`), new 404-byte account (new tag; V1's 0x71 and
   its offsets stay frozen). Terms must learn which spec generation it names,
   because `check_terms_binding` compares feed digests across disjoint
   domains.
2. **The crossing-rule semantics** — replacing admission check 12's
   `publish_time / bucket_seconds == cursor` with
   `prev_publish_time < T(k) <= publish_time` is the single most load-bearing
   semantic change, and it ripples: archive lineage becomes non-strict with
   the byte-identical-except-bucket equality clause, and the seal-time
   maturity witness must be restated (the same witness may legitimately cover
   consecutive boundaries).
3. **The authentication capabilities** — two have zero precedent anywhere in
   the runtime: **Instructions-sysvar decode** (for the immediate-post join)
   and **Upgradeable Loader ProgramData decode** (for the deployment-slot
   pin), plus signed `i64` Clock arithmetic (the runtime currently converts
   to `u64` at the sysvar boundary) and SHA-256 over an arbitrary-length
   external account body (the config digest).

## 2. Phase 0 — identity-independent work (now, on a branch)

Everything below is buildable before 2026-08-26 because it pins no identity
bytes. It lands on a dedicated runtime branch (next-seal-cycle content, like
the V3 successor), never on sealed main.

- **P0.1 Layout:** `SOURCE_SPEC_BODY_V2_BYTES = 368`, the
  `InitSourceSpecV2` intent (new tag; 23 is taken), encode/decode arms, and
  `canonical_feed_id_v2` under the v2 domain.
- **P0.2 Kernel port:** port `spec_v2.rs`/`crossing_v1.rs`/`auth_v2.rs` from
  `research/source-profile-v1` into the program, swapping `sha2` for
  `solana_sha256_hasher::hashv` and replacing the research crate's two pinned
  `MODEL_*` constants with the real `clutch_accumulator` `Grid`/`MAX_VALUE`
  (the research crate flags this drift itself).
- **P0.3 Trait generation:** a v2 authenticator trait returning
  `LoaderStateV1 { linked_programdata, deployment_slot }` from
  (receiver program body, ProgramData body) plus the separate config-digest
  check — the V1 two-body `deployment_generation() -> u64` shape is
  structurally wrong for pull. `PriceParserV2` returns the signed ten-field
  update (i64 price/publish/prev, i32 exponent, u64 posted_slot, write
  authority), not V1's all-unsigned `ParsedPriceV1`.
- **P0.4 New capabilities, each with its own hostile suite:**
  Instructions-sysvar decode (adjacency, post program/config/update/write
  authority; the design's CPI alternative is the fallback if the sysvar
  route proves unsound in practice); ProgramData/loader-state decode;
  signed-i64 clock comparisons; external-body config digest.
- **P0.5 Account planes:** InitSourceSpec 9 → 12 accounts (ProgramData,
  Instructions sysvar, Clock join at init for the activation check);
  Append/Seal 8 → 10. Endow's state-role table hard-pins the 292-byte V1
  spec length and must accept the 404-byte v2 account.
- **P0.6 Mock reshape:** both fixture trios (the joined-lifecycle trio and
  the parallel harness trio) become the v2 shape: receiver program owned by
  a loader key with decodable loader state, a ProgramData account, a Config
  account authenticated by digest, and a caller-created ephemeral 134-byte
  `PriceUpdateV2` account bound by the immediate-post join instead of a
  pinned key. Host-driven record rewrites become receiver-post simulations.
- **P0.7 Hostile SVM campaign** (the design §3.5 list, against the reshaped
  mock): set/post/restore config races, stale-account reuse, write-authority
  reuse, wrong-CPI/wrong-adjacency, same-slot substitution, plus crossing
  falsifiers (double witness refuses, degenerate prev==publish witnesses
  nothing, absent witness stalls, witness reuse admits with byte-identity).
- **P0.8 Error granularity decision:** the 27-variant research
  `AuthV2Error` currently collapses into one `SourceAdmissionFailed` code
  under `source_refusal()`. Decide the runtime error surface before tests
  freeze on it.
- **P0.9 Registry mechanism decision:** `release_registered` must become a
  two-generation predicate, and the six dispatch sites' hard-coded
  `::<MockParser, MockDeployment>` turbofish cannot express even two
  registered releases. The V1 principle "an inert compiled registry, no
  runtime negotiation" should survive: one compiled (parser, authenticator,
  spec-predicate) triple per release, selected by spec generation + exact
  predicate match, never by caller data.

## 3. Phase 1 — identity freeze (only after 2026-08-26 16:00 UTC)

Checklist, in order, all from primary sources with retrieval dates:

1. Confirm the DAO cutover executed; record the post-cutover receiver
   program bytes' identity, its ProgramData key, and the decoded deployment
   slot.
2. Pin the post-cutover receiver `Config` full-body SHA-256 (the digest is
   the governance-generation pin; any later governance change is a new feed
   generation by construction).
3. Pin the SDK/source release that matches the deployed program (the 1.2.0
   guide vs 2.0.0 manifest inconsistency must have resolved; if it has not,
   record both and STOP).
4. Set `activation_unix_timestamp` at or after the cutover instant.
5. Re-verify the 134-byte `PriceUpdateV2` layout and discriminator against
   the deployed post-cutover program (expected unchanged: same ABI).
6. Write the release dossier: every §3.3-style identity, checksum, URL, and
   retrieval date, into `research/source-profile-v1/PROVENANCE.md`'s
   successor section.

## 4. Phase 2 — registry entry and reseal

1. Compile the production release triple into `source_ingest.rs`; the
   default ELF's `release_registered` accepts exactly that one v2 spec.
2. Flip the harness/runner/docs surface that asserts the empty registry
   (the map enumerates all of it: `expect_default_source_refusals`, the
   joined-lifecycle default campaign, prefund/collateral cfg branches, both
   runner scripts, and ten prose sites). The default campaign's assertion
   changes from "Endow refuses 0x79 always" to "Endow refuses 0x79 for
   every spec except the exact registered release" — the refusal boundary
   narrows, it never disappears.
3. Full runtime reseal cycle: this is a new ELF identity by construction —
   final-LTO/stack audit, liveness profile re-measurement, 100+-gate
   emission, manifest commit, post-commit check, fresh Persvati portable
   attestation, hbox rebuild. Budget a full cycle; nothing may quote the old
   digests as current.
4. Only then: blank-bank joined lifecycle per degree WITHOUT the mock-ELF
   split for source construction — the named injections drop from four
   toward one (the evidence buffer, unless its public constructor lands in
   the same cycle).

## 5. Promotion gates (all must hold; any red stops the flip)

- Every Phase-0 hostile suite green on the reshaped mock, both ELF profiles.
- The crossing-rule kernel's falsifiers green in-runtime (not only in the
  research crate).
- Phase-1 checklist complete with primary-source pins; the §4 design
  falsifier stands permanently (one demonstrated double-witness boundary
  reopens the provider selection).
- The V3 successor branch coordination resolved: `genesis.rs` and
  `seeds.rs` are shared-edit files (the successor adds direct-V3 handlers
  and seeds there; the seeds distinctness test array is a guaranteed
  textual merge point). One runtime seal cycle should carry either R2 or V3
  first — not both mid-merge; the second rebases onto the first's sealed
  base.
- Ember's explicit go for the registry flip itself: compiling a production
  release into the default ELF is the protocol's first value-admission
  authority and is not covered by standing swarm authorization.

## 6. What this plan deliberately does not do

No identity bytes now; no weakening of any refusal to make a campaign pass;
no multi-page archives, repair generations, cross-Realm feed reuse, or
second providers (each remains a separately designed successor); no claim
that a green mock-reshaped campaign is production-provider evidence — the
mock stays labeled non-production even in v2 shape.
