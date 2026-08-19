# Source-authenticated observation plane V1

Status: **typed admission kernel implemented; runtime join not implemented**.
The current `FeedAdvance` and `Resolve` paths are explicitly non-qualifying.

This document defines the narrowest realistic path from the current observation
scaffold to a fully onchain, permissionless, objective crypto-price profile. It
does not select Pyth, Switchboard, an AMM, or any other external ABI. Selecting
one requires a primary-source layout/deployment dossier, an immutable or
generation-pinned program check, an audited parser, and SVM fixtures. The typed
policy and admission code live in
`programs/clutch-sbf/program/src/source.rs`.

## 1. Finding: the current path authenticates no source lineage

Today `FeedAdvance` accepts exactly three accounts: any signer, a writable
program-owned `FeedAccount`, and a read-only program-owned observation buffer.
The buffer itself declares the feed, grid, buckets, and intervals. The program
checks its codec and accumulator fold, but it never reads an external source
account, source program, deployment identity, Clock sysvar, price parser,
freshness fact, confidence rule, or canonical record-selection rule.

The cursor therefore proves only: “these caller-supplied records were folded in
this order.” `Intent::FeedAdvance.evidence` is copied into `FeedAccount.summary`
without recomputation, and the current 124-byte feed layout does not persist the
grid, source generation, last source sequence, publish slot, or publish time.

Resolution has a separate and stronger substitution gap. `Resolve` reads the
feed cursor but folds observations from an unrelated caller-supplied evidence
buffer. A caller can:

1. advance the feed with observation set A;
2. construct observation set B with the same domain and a maturity-compatible
   cursor;
3. resolve from B.

Nothing binds B to A, the feed summary, an archive account, or a source. The
typed source kernel names this exact case
`SourceError::UnauthenticatedResolutionEvidence`, and
`an_unrelated_caller_buffer_cannot_substitute_for_the_feed_archive` is the
regression test. The live instruction does not yet call that check, so this is
a design/test obligation, not a claim that the deployed ABI refuses it.

Consequently:

- current local-validator `FeedAdvance` success is SBF/codec/accumulator
  evidence only;
- current local-validator `Resolve` success is evidence-gate differential
  evidence only;
- neither is evidence that an observation came from a market data source; and
- no current build should describe the observation or resolution plane as
  source-authenticated, oracle-secure, or trustless.

## 2. Minimal admissible profile

The first qualifying profile should be deliberately narrower than the generic
accumulator and terms layouts:

- one exact crypto base asset and quote asset;
- quote-units per one base-unit orientation;
- one positive fixed-point integer scale, at most 18 decimals;
- one exact source data account owned by one exact source program;
- one audited parser release selected by a closed onchain registry;
- an immutable source program, or an upgradeable program pinned to a deployment
  generation and checked against its loader deployment account on every use;
- one unique source-native finalized record per bucket;
- complete coverage only: callers may not manufacture `Missing` records;
- maximum 32 buckets, wholly contained in one canonical archive page;
- exact slot and time freshness bounds;
- exact confidence widening and both absolute and relative confidence caps;
- no discretionary proposer, resolver, challenge vote, governance override, or
  alternate-source fallback; and
- fail-closed behavior after source upgrades, stale data, gaps, ambiguity, or
  unsupported source states.

“Latest price with a timestamp” is insufficient. If two records can qualify for
one bucket, transaction timing lets a caller select the settlement value. A
source parser dossier must establish why its `canonical_bucket` record is
unique and terminal. An external oracle's current-value account qualifies only
if its retained state or proof format establishes that property. Otherwise the
profile needs a source-native historical update/proof account or cannot launch.

## 3. Canonical `SourceSpecV1`

`SourceSpecV1::encode_canonical` produces exactly 256 bytes. The existing
`canonical_feed_id` function hashes this preimage under
`dragons-clutch/feed/v1`; the result is the single feed identity already stored
in `TermsAccount.feed` and `MarketAccount.feed`. There is no separately mutable
feed-name truth.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | magic `DCSRCV1\0` |
| 8 | 2 | source-spec schema = 1 |
| 10 | 32 | audited `source_adapter_id` |
| 42 | 4 | source adapter version |
| 46 | 2 | closed parser id |
| 48 | 2 | parser version |
| 50 | 32 | source program key |
| 82 | 32 | exact source data-account key |
| 114 | 8 | deployment generation |
| 122 | 32 | base-asset identity |
| 154 | 32 | quote-asset identity |
| 186 | 1 | orientation = quote per base |
| 187 | 1 | normalized decimals |
| 188 | 4 | grid family id |
| 192 | 2 | grid version |
| 194 | 8 | bucket seconds |
| 202 | 8 | maximum staleness slots |
| 210 | 8 | maximum staleness seconds |
| 218 | 8 | maximum future-time skew seconds |
| 226 | 16 | maximum widened confidence atoms |
| 242 | 4 | maximum widened confidence basis points |
| 246 | 2 | integer confidence multiplier |
| 248 | 2 | canonical selection rule id |
| 250 | 6 | zero reserved |

All identities, versions, parser ids, and the deployment generation are
nonzero. Base and quote differ. V1 registers only quote-per-base orientation and
the finalized-bucket-record selection rule. Staleness bounds are finite and
positive. Confidence bps are in `1..=10_000`, confidence atoms fit the
accumulator's `MAX_VALUE`, and the widening multiplier is in `1..=32`.

`TermsAccount` already carries the three fields needed to bind this artifact:
`feed`, `source_adapter_id`, and `source_version`. `check_terms_binding` checks
all three. `InitTerms` must additionally require that the `SourceSpecAccount`
exists at its canonical PDA and that the terms grid equals the spec grid; this
join is not present today.

## 4. Admission relation

The program, not instruction data, selects a compile-time `PriceParserV1` from a
closed parser registry. Before parsing, `admit_price` checks:

1. source account key equals the source spec;
2. source account owner equals the frozen source program;
3. source data account is non-executable;
4. compiled adapter id/version and parser id/version equal the spec;
5. persisted feed head equals the canonical feed, grid, and deployment
   generation;
6. parser-observed deployment generation equals the spec;
7. source-native sequence increases strictly;
8. publish slot/time do not move backwards;
9. publish slot is not future and is within the slot-age bound;
10. publish time is within the past/future time bounds;
11. the parser established a finalized canonical bucket record;
12. both the parser's bucket and `publish_time / bucket_seconds` equal the feed
    cursor;
13. normalized price is positive and at most `MAX_VALUE`;
14. widened confidence is within the absolute and price-relative caps; and
15. interval endpoints and `cursor + 1` fit exactly.

The output is one `Observation::Accepted` and the exact source progress fields
to persist. No API returns `Observation::Missing`. Failure to present a source
record is not evidence that no record exists.

The two-clock rule is intentional. Slot freshness catches an old account even
when its wall time is malformed; source-time freshness catches a semantically
old record published in a recent transaction. Clock slot and Unix time must be
read from the canonical Clock sysvar by the SBF seam, never from the request.

## 5. Required persisted layouts

These are proposed exact V1 layouts. They are not implemented codecs and must
not be treated as frozen until the external parser dossier and transaction-size
measurements close.

### 5.1 `SourceSpecAccountV1` — 292 bytes

PDA: `source-spec-v1 := PDA(program, "source-spec", feed_id)`.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | account tag |
| 1 | 1 | account version = 1 |
| 2 | 32 | `feed_id = canonical_feed_id(spec_bytes)` |
| 34 | 256 | canonical `SourceSpecV1` bytes |
| 290 | 1 | stored PDA bump |
| 291 | 1 | flags, zero in V1 |

It is created permissionlessly after the runtime verifies the source program,
deployment account, source data account, parser registry entry, and feed digest.
There is no update instruction. A source upgrade creates a different spec/feed
generation; existing feeds fail closed.

### 5.2 `FeedAccountV2` — 202 bytes

PDA remains derived from the canonical feed identity. This is a schema revision,
not an in-place reinterpretation of the current 124-byte account.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | account tag |
| 1 | 1 | account version = 2 |
| 2 | 32 | canonical feed identity |
| 34 | 32 | Realm identity |
| 66 | 32 | SourceSpec account identity (= feed id in V1) |
| 98 | 8 | next accepted bucket cursor |
| 106 | 8 | next logical boundary |
| 114 | 8 | sealed archive-page count |
| 122 | 8 | last source-native sequence |
| 130 | 8 | last publish slot |
| 138 | 8 | last publish time |
| 146 | 8 | source deployment generation |
| 154 | 4 | grid family id |
| 158 | 2 | grid version |
| 160 | 8 | bucket seconds |
| 168 | 32 | recomputed accumulator/archive-chain digest |
| 200 | 1 | stored PDA bump |
| 201 | 1 | flags, zero in V1 |

The current caller-declared `Intent::FeedAdvance.evidence` cannot remain the
summary authority. The program recomputes the new digest from the prior digest,
canonical archive identity, and admitted record bytes with a domain-separated
hash selected before ABI freeze.

### 5.3 `FeedArchivePageV1` — 2,188 bytes

V1 uses 32 records per page. PDA:
`PDA(program, "feed-archive", feed_id, deployment_generation, page_index)`.

Header (140 bytes):

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | account tag |
| 1 | 1 | account version = 1 |
| 2 | 32 | feed id |
| 34 | 32 | SourceSpec identity |
| 66 | 8 | source deployment generation |
| 74 | 4 | grid family id |
| 78 | 2 | grid version |
| 80 | 8 | bucket seconds |
| 88 | 8 | page index |
| 96 | 8 | first bucket |
| 104 | 1 | populated record count |
| 105 | 1 | sealed flag |
| 106 | 32 | digest of header identity + populated records |
| 138 | 1 | stored PDA bump |
| 139 | 1 | flags, zero in V1 |

The header is followed by 32 fixed 64-byte records:

| Record offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | bucket |
| 8 | 16 | conservative low endpoint |
| 24 | 16 | conservative high endpoint |
| 40 | 8 | source-native sequence |
| 48 | 8 | source publish slot |
| 56 | 8 | source publish time |

Unused records are zero. Populated records are contiguous, strictly increasing
by bucket and source sequence, and individually arise only from `admit_price`.
A sealed page is immutable. The summary/page digest is recomputed, not supplied.

## 6. Required instruction joins

### 6.1 Source-spec/feed initialization

A new permissionless initialization family creates the `SourceSpecAccountV1`
and `FeedAccountV2` PDAs. Its account list must include payer/signature, both new
PDAs, System program, source program account, source deployment-evidence account,
and source data account. Initialization verifies the concrete loader state:

- source program is executable and owns the source data account;
- its ProgramData/deployment account is the one named by the program account;
- immutable programs have no upgrade authority; or
- generation-pinned programs match the exact deployment generation, with every
  later advance repeating that check.

The exact loader parser must use a pinned official Solana interface. This report
does not invent its bytes.

### 6.2 Authenticated `FeedAdvance`

The qualifying instruction replaces, rather than supplements, the caller page:

| Index | Account | Access |
| ---: | --- | --- |
| 0 | permissionless transaction signer/payer | signer, read-only unless page creation pays rent |
| 1 | `FeedAccountV2` PDA | writable |
| 2 | `SourceSpecAccountV1` PDA | read-only |
| 3 | source program | executable, read-only |
| 4 | source deployment-evidence account | read-only |
| 5 | exact source data account | read-only |
| 6 | current `FeedArchivePageV1` PDA | writable |
| 7 | Clock sysvar | read-only |

If archive pages are created lazily, a separate permissionless page-init
instruction is preferable to adding conditional System-program accounts to the
hot path. `FeedAdvance` admits exactly one bucket; batching is deferred until a
selected external source can prove multiple unique records in one account.

### 6.3 Authenticated `Resolve`

The live caller-supplied evidence buffer must not survive in the qualifying
path. For the minimal profile, terms require a complete window of at most 32
buckets contained in one canonical page. `Resolve` receives that page read-only,
derives its PDA from feed/generation/page index, verifies program ownership,
codec, seal, digest, exact range, and terms/source bindings, then folds those
authenticated records into `WindowResult`.

`check_resolution_archive` makes the provenance rule explicit: a
`CallerSuppliedBuffer` is always
`SourceError::UnauthenticatedResolutionEvidence`, even if its domain and cursor
match. Later multi-page windows may stream several canonical page accounts or a
checked inclusion proof, but must preserve the same lineage rule.

## 7. Permissionlessness and absence of resolver authority

Anyone may pay to initialize an eligible canonical source spec, create an
archive page, submit the next unique finalized source record, or resolve a
mature complete market. No submitter chooses a value, source, bucket, confidence
rule, fallback, or payout. Every such fact is either frozen in terms/spec state,
parsed from an authenticated source account by compiled code, or derived by the
accumulator/payout relation.

This removes discretionary resolver authority; it does not remove the selected
source's trust and manipulation surface. A qualifying parser dossier must state
the source publisher/validator assumptions, upgrade controls, price construction,
retention, common-mode failure, and maximum economically plausible manipulation.
If those assumptions are unacceptable, the parser is not admitted.

## 8. Gates before a parser can become load-bearing

1. Select one candidate source from primary official specifications.
2. Pin program ids, loader/deployment identity, crate/source revision, account
   layout, and every normalization rule.
3. Prove or decisively test unique finalized-bucket selection; reject a
   transaction-timing selection surface.
4. Implement the concrete `PriceParserV1` without allocation, floats, unchecked
   casts, or panics on hostile bytes.
5. Add adversarial fixtures for wrong owner/key/deployment, upgrade substitution,
   signed exponent boundaries, stale/future time, wide confidence, malformed
   layout/TLV, replays, skipped buckets, and ambiguous records.
6. Implement and test the three codecs and canonical PDAs above.
7. Replace the live caller-buffer `FeedAdvance` and `Resolve` paths; do not offer
   an unauthenticated compatibility mode for value-bearing markets.
8. Run host differential tests, `solana-program-test`, local-validator replay and
   atomic rollback tests, SBF stack analysis, compute/account/transaction-size
   measurements, and same-machine reproducible ELF evidence.
9. Add the authenticated source/archive gates to the baseline manifest.
10. Keep the result labelled adapter-verified, not formally verified, until the
    exact parser and account/runtime refinement obligations are proved.

Until all ten close, the correct public statement remains: the repository has a
tested fail-closed source-admission relation and a proposed authenticated archive
ABI, but no source-authenticated onchain observation or resolution path.
