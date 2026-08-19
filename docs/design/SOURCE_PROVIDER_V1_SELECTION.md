# R2 provider selection and pull-profile spec revision

Status: **PROPOSED / MODEL-ONLY** — no identity freeze, no runtime change, no
registry entry, no live reads. Written 2026-08-19 from the primary-source
comparison memo (research lane, retrieval-dated 2026-08-19; source index and
per-file SHA pins in `research/source-profile-v1/PROVENANCE.md`). The default
ELF's source registry remains empty and Endow keeps refusing
`SourceReleaseUnavailable` (`0x79`) until every gate below closes.

## 1. Selection

**Pyth pull (`PriceUpdateV2` via the receiver program) is selected as the R2
V1 provider profile to design against.** The selection is reversible until
the deployment-identity freeze in §5.

Deciding fact: `SOURCE_ADMISSION_V1` treats a transaction-timing value-
selection surface — two records qualifying for one bucket — as disqualifying
for the sole settlement source. Among the profiled candidates, only Pyth
documents a unique-record-per-instant relation
(`prev_publish_time < t <= publish_time`, `pythnet_sdk/messages.rs` @
`ec456fc`). Switchboard On-Demand's signed payload carries no timestamps, no
confidence, and no predecessor link, so several fresh quotes qualify per
bucket; Chainlink Data Streams is double access-gated (API credential plus
on-chain allowlist), documents no per-timestamp uniqueness, and its
`expiresAt` turns a missed window into a permanently unverifiable one.
Choosing either non-Pyth candidate means inventing uniqueness semantics the
provider does not document. Choosing Pyth means a bounded, enumerable spec
revision (§3) — the repo's own dossier (`research/source-profile-v1/DOSSIER.md`
§7) already lists most of it.

Retained roles for the others: Switchboard On-Demand is a candidate
*cross-check* input (fully in-transaction ed25519 verification, permissionless
submission) and never a settlement archive; Chainlink is parked, with its
mainnet verifier identity still unpinned from a fetchable primary source.

## 2. What Pyth satisfies as-is

- Unique finalized record per boundary via the documented crossing relation.
- Both legs of the implemented two-clock staleness rule: `posted_slot` age
  against `Clock.slot` and `publish_time` age against `Clock.unix_timestamp`.
- Confidence: `conf` is the aggregate 1-sigma; the existing widened
  conservative low/high interval discipline binds directly.
- Scale/orientation: `price x 10^exponent`, quote-per-base.
- Parser pinning: Anchor discriminator, 134-byte length,
  `VerificationLevel::Full`, feed-id equality — all compile-time checks
  already implemented and hostile-byte tested in `research/source-profile-v1`.
- Historical recovery: Benchmarks/Hermes payloads are signed and verified
  on-chain, so the HTTP layer is untrusted for integrity.

## 3. Required spec revision: the pull profile (SourceSpec v2 generation)

`SourceSpecV1` binds one exact immutable source data-account key. Pyth update
accounts are caller-created, ephemeral, and closable; no such key exists.
This is a new spec generation under a new domain (`dragons-clutch/feed/v2`),
not an update — old feeds fail closed by construction.

Field-level deltas from V1:

1. **Replace** the exact source data-account key **with**:
   - the receiver `Config` PDA key (stable), and
   - a canonical **config byte digest** pinned at spec creation and rechecked
     byte-exact on every append. Rationale: `Config` holds
     `valid_data_sources`, the fee, the wormhole/router address, and
     `minimum_signatures`, all mutable by governance without a program
     upgrade; account ownership plus current config is ambiguous provenance.
     A governance change is therefore a new feed generation, exactly like a
     deployment change.
2. **Add** the provider feed id (Pyth 32-byte `feed_id`), checked equal on
   every admitted update.
3. **Keep** the deployment-generation pin (program + ProgramData identity),
   rechecked on every use, now alongside the config digest.
4. **Register a new canonical selection rule** (`CROSSING_V1`) instead of the
   V1 finalized-bucket rule (see §4). Admission check 12's
   `publish_time / bucket_seconds == cursor` equation is specific to the V1
   rule and does not apply to a crossing-rule feed; the v2 check binds the
   crossing predicate instead.
5. **Update-account discipline** (no account identity to bind): an admitted
   update must be proven to originate from the pinned receiver in the same
   atomic join — via the Instructions sysvar, the immediately preceding
   instruction must be the exact pinned post instruction naming this update
   account and the pinned config, **or** the Clutch instruction CPIs the
   pinned receiver itself; plus owner, discriminator, length,
   `VerificationLevel::Full`, and feed-id equality; parse and archive before
   the caller-controlled account can change or close. Hostile SVM coverage
   must include set/post/restore config races, stale-account reuse,
   write-authority reuse, wrong-CPI, and same-slot substitution
   (`AUTHENTICATED_SOURCE_CONSTRUCTION_V1` §3.3 list).

## 4. CROSSING_V1 admission semantics (exact, with falsifiers)

Let `bucket_seconds = B` and bucket `k` cover `[kB, (k+1)B)`.

- **Boundary instant (PROPOSED, two variants):**
  - **(a) closing boundary — recommended:** `T(k) = (k+1)B`. Bucket `k`'s
    record is witnessed by the unique update `U` with
    `prev_publish_time(U) < T(k) <= publish_time(U)`: the source state in
    force at the moment the bucket closes. Matches "finalized bucket"
    settlement reading.
  - **(b) opening boundary:** `T(k) = kB`. Same machinery, earlier instant.
  One variant must be frozen before any parser release; both are stated so
  the choice is visible.
- **Uniqueness:** for a fixed `T`, the crossing relation admits at most one
  update by the provider's own documentation. Two distinct qualifying updates
  for one boundary is the falsifier for the whole selection — if it is ever
  exhibited, Pyth loses the deciding property of §1 and R2 reopens.
- **Degenerate update:** an update with `prev_publish_time == publish_time`
  (failed aggregation) satisfies the predicate for no `T` and can witness no
  boundary. It is skipped, never adapted into a record.
- **Absent crossing message** (documented migration/delivery gaps): no
  witness means **stall**. Nothing may manufacture a `Missing` record or
  substitute an adjacent update. Fail-closed is the product.
- **Witness reuse across consecutive boundaries:** if no update lands between
  `T(k)` and `T(k+1)`, the same `U` legitimately witnesses both. The archive
  record's source-native sequence field therefore cannot demand strict
  increase across records in the pull profile. **PROPOSED rule:** sequence
  := `publish_time(U)`, monotone **non-strict**, with equality admissible
  only when the full 64-byte record bodies (endpoints included) are
  byte-identical except the bucket field. Falsifier: two admitted records for
  different buckets with equal sequence and differing endpoints — that would
  prove the equality clause admits value drift and the rule must be replaced
  (e.g., by an explicit witness-identity hash).
- **Semantic owners, disambiguated** (dossier §7 item 5): archive
  `source publish time` := `publish_time(U)`; archive `source publish slot`
  := `posted_slot` of the admitted update account (receiver-write slot — the
  slot leg of staleness, explicitly not source-native); archive sequence :=
  as above. No field doubles as another.

## 5. Sequencing against the 2026-08-26 cutover

The Pyth DAO upgrades the receiver in place on 2026-08-26 16:00 UTC
(13-of-19 Wormhole guardian quorum -> 3-of-5 router secp256k1 quorum, same
ABI), and the SDK's own migration guide and manifest currently disagree on
the version to use (1.2.0 vs 2.0.0). Freezing identity now pins a program
seven days from an in-place mutation.

- **Freeze now (cutover-independent):** the v2 spec layout, `CROSSING_V1`
  semantics and its frozen boundary variant, the parser/adapter code against
  the pinned `ec456fc` layout (ABI is unchanged by the cutover), the hostile
  SVM test plan, and the archive semantic-owner rules.
- **Freeze only after the cutover lands:** receiver program/ProgramData
  identity bytes, the config byte digest, the SDK release pin, and the
  registry entry. A pre-cutover identity freeze is forbidden in this design.
- **Terms obligation:** the trust floor must be named in Terms — post-cutover
  that is the 3-of-5 router quorum plus the pinned config generation, and the
  failure consequence is stall-then-lapse, never substitution.

## 6. Open items that are not engineering

- Pyth ToS prohibits bulk automated extraction and does not address on-chain
  protocol usage; post-cutover historical payloads require a billed API key
  whose secret cannot live in a static frontend. Both go to the in-house
  legal analysis lane; neither blocks the §3-§4 design work.
- Retention horizon of Hermes/Benchmarks history is undocumented: the
  late-recovery horizon must be measured and stated in Terms before any
  market with a long maturity may reference a pull feed.

## 7. Falsifier summary and promotion gate

This document promotes nothing. The route to a registry entry is: v2 spec
codec + hostile-byte tests -> parser against pinned post-cutover bytes ->
the §3.5 hostile SVM campaign green on a real bank -> blank-bank
create/append/seal join (roadmap R2 items 4-5) -> only then a compiled
default-registry release. Every stage inherits the constructor pre-fund-safe
rule. The uniqueness falsifier in §4 stands permanently: one demonstrated
double-witness boundary reopens the selection.
