# Batch policy artifact and full-width candidate identity

Status: **IMPLEMENTED OFFLINE SEMANTIC PREREQUISITE; NOT A LIVE SBF ABI,
NOT CANDIDATE-WINDOW CLOSURE, NOT SELECTION**

Implementation:
[`research/batch-policy-identity`](../../research/batch-policy-identity)

This lane closes two semantic ambiguities without changing the live program:

1. `FrozenPolicyV1` now has a canonical immutable 64-byte artifact preimage and
   a domain-separated SHA-256 identity; and
2. the relation can validate one submitted candidate against full 32-byte
   market, book, epoch, policy, and order-set identities without pretending a
   `Hash32` can be injected into a `u64`.

It deliberately does **not** close a candidate proposal window, select a
candidate, change a persisted status, freeze entitlements, or authorize
settlement. Its strongest result type is named `VerifiedSubmittedCandidateV1`.

## 1. Audit of the pre-existing seam

The three planes were individually coherent but did not join losslessly:

| plane | existing identity | exact finding |
| --- | --- | --- |
| `EpochAccount` | `Hash32` market, book, epoch, policy, and order set; separate `u64 epoch_index` | The policy was an opaque digest with no batch-policy preimage codec. |
| `relation_v1::RelationDomainV1` | `u64 market_id`, `book_id`, `epoch`, `policy_id`, and `order_set_id` | The `epoch` field is also used for expiry admission. All five values feed only the old non-cryptographic V9 digest; four are not otherwise economic inputs. No mapping from the account identities was specified. |
| `CandidateRecord` | SHA-256 candidate identity over full epoch and market identities plus free coordinates | This is a sound account identity for an immutable Epoch, but it is not the relation's policy/domain tie identity. |
| `CandidateFeedHeader` | repeats candidate, epoch, market, and order-set `Hash32`; carries `claimed_digest: u128` | Shared account coordinates are bound, but the claimed relation digest can only be the host relation's `u128` fold over its `u64` domain. |

There is no injective function from 256-bit identities into 64-bit identities.
Truncating, XOR-folding, or hashing to `u64` would merely choose a collision
policy; it would not repair the identity type mismatch.

`FrozenPolicyV1::code()` is injective over the currently registered selector
product and admitted fee range: it uses ten base-8 selector digits followed by
a 16-bit fee code. That makes it a useful internal regression tag. It is not an
immutable artifact because it has no versioned hostile-byte codec, no digest
domain, no reserved-byte rule, and no persisted account which proves that the
Epoch's opaque `Hash32` commits to that code. It also cannot repair any of the
other four `Hash32`/`u64` mismatches.

## 2. Canonical `BatchPolicyV1` artifact

The semantic owner remains `clutch_batch::relation_v1::FrozenPolicyV1`. The new
crate encodes and decodes that exact public type; it does not define a second
policy DTO.

One artifact is exactly 64 bytes:

| bytes | field | rule |
| ---: | --- | --- |
| `0..8` | magic | `DCBATP1\0` |
| `8..10` | schema | little-endian `1` |
| `10..12` | flags | exactly zero |
| `12` | allocation | price-priority marginal pro-rata / full pro-rata |
| `13` | self-cross | refuse / net at admission / pairing gate |
| `14` | AON/minimum-fill | refuse / witnessed mask / full-size counting |
| `15` | rounding boundary | exact / terminal-owner floor / receipt floor |
| `16` | residual settlement | full-pair / cumulative canonical / cumulative free / unique slices |
| `17` | transfer phase | Active only / Active or Resolved |
| `18` | portfolio lots | strict whole order / marginal pro-rata lots |
| `19` | pairing witness | recomputed constructor / explicit slices |
| `20` | dust | canonical largest remainder / reject |
| `21` | score | lexicographic dispersion V1 |
| `22` | fee base | none / flat notional |
| `23` | alignment padding | exactly zero |
| `24..28` | fee basis points | little-endian `u32`; exactly zero for no-fee; `0..=10,000` for flat notional |
| `28..64` | reserved | exactly zero |

The immutable policy identity is

```text
SHA256("dragons-clutch/batch-policy/v1\0" || canonical_policy_bytes)
```

Decode accepts every **registered** selector, including the research-gated
`MarginalProRataLots` selector, so that registered semantics have stable
identities. Relation execution separately calls `FrozenPolicyV1::validate()`
and refuses that unimplemented variant. An unsupported policy is never silently
rewritten to a supported one.

Every policy family which can affect admission, normalization, fills, witness
shape, score, fees, residual receipts, transfer phase, or settlement is present
in this preimage. There is no default constructor and no omission-based
canonicalization.

## 3. Full-width relation domain

`FullRelationDomainV1` separates the full canonical Epoch identity from the
numeric `epoch_index` needed for expiry admission. Its exact 284-byte digest
preimage is:

| bytes | field |
| ---: | --- |
| `0..8` | `DCBRDV1\0` magic |
| `8..10` | domain schema `1` |
| `10..12` | zero flags |
| `12..16` | relation version |
| `16..48` | market `Hash32` |
| `48..80` | book `Hash32` |
| `80..112` | epoch `Hash32` |
| `112..144` | policy `Hash32` |
| `144..176` | order-set `Hash32` |
| `176..184` | epoch index |
| `184` | outcome count |
| `185` | zero padding |
| `186..188` | owner count |
| `188..196` | price scale |
| `196..204` | remainder seed |
| `204..268` | complete canonical policy bytes |
| `268..284` | zero reserved bytes |

The domain refuses zero identities and recomputes the policy digest before
relation execution. Including both the policy identity and the checked policy
bytes is deliberate: the Epoch-facing commitment is visible in the preimage,
and the relation-facing selectors cannot be substituted beneath it.

The domain identity is

```text
SHA256("dragons-clutch/full-relation-domain/v1\0" || domain_bytes)
```

No API returns a `u64` projection of an identity. The existing economic
relation is reused only through a private arithmetic projection whose four
identity tags are zero sentinels. An audit of `relation_v1` shows those tags are
read only by its obsolete V9 digest. The new verifier runs V0--V8, discards both
legacy digest fields, and returns them as explicit zero sentinels. It then
recomputes the authoritative full-width V9 score. If a future economic stage
starts reading a legacy identity field, this seam must be revised before
promotion; zero cannot become an implicit identity convention.

## 4. Candidate, feed, witness, and score identity

The account-plane candidate identity is reproduced byte-for-byte under the
existing domain:

```text
SHA256("dragons-clutch/candidate/v1"
       || epoch_hash || market_hash
       || order_len || outcome_count
       || 16 little-endian prices
       || sigma || mu || honored_aon_mask)
```

A cross-crate test constructs a real `clutch_solana_layout::CandidateRecord`
and proves exact digest equality. The new full relation-candidate identity then
commits to:

- the full relation-domain digest;
- that canonical account-candidate identity;
- all 64 canonical fills;
- the honored-AON mask; and
- a witness-presence marker and every canonical pairing slice when the policy
  selects explicit slices.

Each slice commits to typed buy and sell leg references, outcome, and exact
quantity. Thus an explicit witness cannot be swapped while retaining the V9
tie identity. The full digest is SHA-256 under
`dragons-clutch/full-relation-candidate/v1\0`.

`FullCandidateFeedBindingV1` repeats candidate, epoch, market, order set, and
the complete full-width score. `verify_submitted_candidate` checks all five
bindings, recomputes V0--V8, recomputes every score component, and compares the
full digest. It never accepts one claimed field because it agrees with another
claimed field.

Score ordering preserves the existing directions:

1. maximize dispersion-weighted direct volume;
2. maximize exact limit surplus;
3. maximize distinct participating owners;
4. minimize churn; then
5. prefer the lexicographically smaller complete 32-byte digest.

The comparison method is pure arithmetic. Calling it does not prove the
candidate set is complete and does not grant selection authority.

## 5. Adversarial evidence

`cargo test --manifest-path research/batch-policy-identity/Cargo.toml --offline
--all-targets` currently passes 9 tests, including:

- exhaustive round trips for all **10,368** registered selector/fee-boundary
  products;
- every selector-family mutation changes the policy digest;
- every one-byte mutation in the 64-byte policy either refuses or decodes to a
  distinct policy with a distinct digest;
- unknown selector, version, flag, inactive-fee, length, and reserved-byte
  refusals;
- mutation of every byte of every full identity, including changes outside any
  possible eight-byte projection;
- exact candidate-hash parity with the layout codec;
- policy and CandidateFeed substitution refusals;
- claimed-score mutation refusal;
- explicit pairing-witness commitment and missing-witness refusal; and
- a score-order test whose only difference lies beyond the first 128 bits.

Clippy with `-D warnings` and rustdoc also pass offline. This is bounded,
executable model evidence. It is not a proof-assistant theorem, SBF execution,
or deployment evidence.

## 6. What remains before direct selection is live

These steps are dependency ordered; the semantic crate does not authorize
skipping any of them:

1. **Freeze the persisted policy owner.** Add a reviewed final BatchPolicy
   artifact account/PDA and typed upload kind, choose its immutable context,
   and make Epoch construction authenticate the final bytes and recomputed
   digest. This changes live artifact semantics and was intentionally excluded
   here.
2. **Freeze a full-width relation ABI.** Either revise Candidate/CandidateFeed
   or add one immutable verified-candidate record so the 32-byte relation
   digest and full score have one persisted owner. The present feed's `u128
   claimed_digest` cannot carry it, and relabeling that field is forbidden.
3. **Project the complete frozen book losslessly.** Authenticate every page,
   owner-tag bijection, live order count after tombstones, order-set identity,
   reservations, and exact single-Egg/portfolio record into the relation.
4. **Give streaming verification stable bytes.** Replace the opaque
   `repr(Rust)` checkpoint body with a versioned codec or another resumable
   verifier state whose partial forms always refuse.
5. **Verify submitted candidates.** Recompute fills, witness, scores, fees, and
   full relation identity; persist `VERIFIED` or `REFUSED` without treating
   submission as verification.
6. **Close the proposal window separately.** Freeze the Clock/deadline rule and
   a complete submitted-candidate set commitment. Proving one content PDA is
   unique does not prove no other valid submission exists.
7. **Select once.** Compare only the complete verified submitted set under the
   frozen full score, record the best valid submitted candidate, supersede the
   others, and move no Epoch to `CLEARED` until all entitlements are complete.
8. **Freeze direct entitlements.** Create every exact receipt/pot, bind every
   reservation and fee recipient, prove set completeness, and only then expose
   the already-landed narrow direct settlement consumer.
9. **Close lapse and terminal paths.** No-candidate lapse, unfilled refunds,
   receipt/pot consumption, and terminal sweep need once-only state machines
   and liveness funding.

## 7. Additional portfolio-selection obligations

Portfolio placement and reservation already preserve exact coefficient vectors
over native basis Eggs. The relation supports strict whole-order portfolio
lots. Making portfolio selection live additionally requires:

- a full page-to-`BookV1` projection which copies every active coefficient and
  proves zero padding, width, expiry, lot, minimum-fill, and reservation
  equality;
- continued fail-closed refusal of the registered but unimplemented marginal
  pro-rata portfolio-lot policy;
- selected-candidate entitlements which expand each filled lot to its exact Egg
  coefficient transfers without changing the native B-spline basis; and
- atomic multi-leg receipt/pot accounting, fee allocation, residual policy,
  and terminal closure across the complete selected portfolio set.

Range, tail, triangle, capped call/put, spread, Gaussian, sample, histogram, and
LP-range positions may be coefficient programs over native basis Eggs. That
composition does not turn the B-spline basis into categorical bins, and this
batch-policy identity work neither changes nor lowers native resolution
semantics.

## 8. Tier 2 general-clearing profile and the zero-sentinel argument (T2-5)

Status: **PROPOSED policy profile; HOST-TESTED verdict-identity gate.** Added
by increment T2-5 of
[TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md](../design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md);
ember's sign-off is what turns PROPOSED into frozen.

### 8.1 `GENERAL_CLEARING_POLICY_V1`

`src/general_clearing_v1.rs` pins the Tier 2 frozen policy profile v1 as a
const, sibling to `DIRECT_POLICY_V1`. Plan-pinned selectors: `fee_base: None`
(0 bps — deliberately not preempting the queued fee-base fork),
`pairing_witness: ExplicitSlices`, `portfolio_lots: StrictWholeOrder`,
`self_cross: RefuseOverlap` (two order passes). The plan's ONE pinned dust
choice is `AssignCanonical`, taken from the relation's own code and tests:
both relation test suites freeze it in their base policies, the domain's
`remainder_seed` exists solely as the largest-remainder tie-break seed, and
under `DustPolicy::Reject` a marginal pro-rata pool with any leftover atom has
no valid candidate at all (`ErrorV1::DustRejected` from the canonical
constructor) — generic on many-order books, vacuous only in the two-order
direct profile where every pool has one member and dust is structurally zero.
Selectors the plan leaves open follow `DIRECT_POLICY_V1` except
`rounding: TerminalOwnerFloor`, because `RoundingBoundaryV1::None`
(exact-or-refuse) refuses any candidate whose per-owner cash conversion leaves
a remainder — the same general-book liveness hazard as dust rejection.

The canonical artifact bytes and the digest

```text
7a9ea80b819f853d9523a5e0ed0bb8e5ab4e167ab0c2245316775955c7a2065b
```

are pinned against an independent third SHA-256 implementation by
`general_clearing_policy_identity_value_is_pinned`, and
`every_selector_mutation_moves_the_general_clearing_identity` proves every
registered alternative of every selector family (and both fee boundaries)
moves the identity.

### 8.2 Zero-sentinel soundness, as the Tier 2 program will consume it

The program-side domain construction (T2-6) is exactly

```text
RelationDomainV1 { relation_version, market_id: 0, book_id: 0,
                   epoch: epoch_index, policy_id: 0, order_set_id: 0,
                   outcome_count, owner_count, price_scale, remainder_seed,
                   policy }
```

and the streaming walk runs `ClearWorkV1::begin(domain, candidate,
strict_claims = false)`. The zero sentinels are sound because the four u64
identity tags are read only by the obsolete V9 legacy digest, which under
`strict_claims = false` is never compared against anything. Authoritative
identity binding is full-width: `ClearWorkHeader{market, epoch, candidate,
order_set}` plus `FullRelationDomainV1::digest()` recomputed wherever
selection needs a total order. Score comparison uses
`FullScoreV1::total_order` over components recomputed from the streamed
`SummaryV1` — never the claimed u128 digest, which the full verifier already
discards and returns as an explicit zero sentinel.

The gate demanded by the plan is
`streaming_zero_sentinel_verdict_matches_the_full_width_verifier`
(in `src/general_clearing_v1.rs`, homed here because this crate depends on
`clutch-batch` and not the reverse): the streaming verdict under the
zero-sentinel domain equals `verify_submitted_candidate`'s V0–V8 verdict on
the same coordinates — a plain cross, a marginal pro-rata book with assigned
dust, a portfolio order clearing against singles, and a forged fill refused
with the identical relation error on both paths.
`zero_sentinel_tags_bind_nothing_and_full_width_identity_binds_everything`
states the argument executably in both directions: junk nonzero u64 tags
change nothing in the summary except the two legacy digest fields, while a
one-byte flip of one 32-byte identity is invisible to the zero-sentinel stream
but moves `FullRelationDomainV1::digest`, the full relation-candidate digest,
and the `FullScoreV1::total_order` tie. The §3 caveat stands unchanged: if a
future economic stage starts reading a legacy identity field, this seam must
be revised before promotion.
