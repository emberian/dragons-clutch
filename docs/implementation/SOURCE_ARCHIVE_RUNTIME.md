# Authenticated source archive runtime slice

Status: **implemented as a callable adapter kernel and exact account codecs;
not joined to a live instruction**.

The implementation is
`programs/clutch-sbf/program/src/source_archive.rs`.  It deliberately selects
no production market-data provider.  Its only concrete provider implementation
is the deterministic mock inside
`programs/clutch-sbf/svm-tests/tests/source_archive.rs`.  Consequently this work
is evidence for the provider-neutral account and lineage machinery, not evidence
that Pyth, Switchboard, a DEX, or any other source has been authenticated.

## 1. What this closes

The earlier `source.rs` kernel already specified exact source admission:
program/key/owner, adapter and parser release, deployment generation, strictly
increasing source sequence, slot/time freshness, canonical time bucket,
finality, price scale, and confidence widening.  It did not own persisted
bytes.  In particular, the live `Resolve` route could still read an unrelated
caller buffer with the same declared domain.

This slice adds:

- an immutable, content-addressed 292-byte source-spec account;
- runtime key/owner/executable authentication for that account;
- a fixed 2,560-byte archive account containing one complete settlement window;
- exact binding to the source-spec digest, provider program and loader,
  deployment-evidence account and owner, deployment verifier release, price
  parser release, source data key/owner, generation, grid, and canonical window;
- an explicit predecessor link for source sequence, publish slot, publish time,
  and prior archive commitment;
- atomic one-record admission and append through `source::admit_price`;
- terminal sealing only after complete coverage and a witnessed authenticated
  feed cursor at the immutable maturity boundary; and
- a sealed receipt constructor that requires the exact expected archive key,
  Dragon's Clutch ownership, non-executable metadata, canonical bytes, record
  lineage, and a recomputed page commitment.

The important property is stronger than “same domain.”  Copying an otherwise
valid sealed page to another account cannot produce a receipt because
`verify_sealed_archive` checks the runtime key and owner before reading its
contents.  Changing any committed byte at the correct account causes a digest
refusal.  This is the adapter that a later `Resolve` route can join without
accepting a caller-supplied evidence buffer as an alternative path.

## 2. Source-spec account: 292 bytes

Proposed PDA preimage:

```text
PDA(clutch_program, "source-spec-v1", feed_id)
```

`feed_id` is `canonical_feed_id` over the exact 256-byte `SourceSpecV1` body.
It is the one semantic owner of the spec digest and feed identity.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | tag `0x71` |
| 1 | 1 | account version `1` |
| 2 | 32 | canonical `feed_id` / source-spec digest |
| 34 | 256 | exact canonical `SourceSpecV1` bytes |
| 290 | 1 | canonical PDA bump |
| 291 | 1 | flags, required zero |

`verify_source_spec_account` is intentionally a metadata-bearing API.  It
requires the Dragon's Clutch program id, the already-derived expected PDA, and
a runtime view carrying key, owner, executable bit, and bytes.  A raw byte
decoder is private.  Therefore the `VerifiedSourceSpecAccountV1` capability
used by archive initialization/admission cannot be obtained from an arbitrary
instruction buffer through this public module.

Decoding reconstructs `SourceSpecV1`, repeats all its structural validation,
requires its six reserved bytes to be zero, recomputes the feed digest, and
requires the stored body to be the canonical re-encoding.  The capability also
carries the authenticated account key and stored bump for the eventual PDA
join.

## 3. Archive account: 2,560 bytes

V1 deliberately admits one complete window of at most 32 records.  It does not
introduce a multi-page proof format before one is required.  Proposed PDA
preimage:

```text
PDA(clutch_program, "source-archive-v1", feed_id, window_id)
```

The SBF seam, not this provider-neutral module, must derive that PDA using the
program id and verify the stored bump.  `verify_sealed_archive` receives the
derived address as `expected_archive_key` and makes equality mandatory.

### 3.1 Header: 512 bytes

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | tag `0x72` |
| 1 | 1 | account version `1` |
| 2 | 1 | flags: `0` open, `1` sealed |
| 3 | 1 | populated record count, `0..=32` |
| 4 | 32 | source-spec digest / feed id |
| 36 | 32 | source-adapter implementation id |
| 68 | 4 | source-adapter version |
| 72 | 2 | closed price-parser id |
| 74 | 2 | price-parser version |
| 76 | 32 | exact provider program key |
| 108 | 32 | exact provider program owner/loader |
| 140 | 32 | exact deployment-evidence account key |
| 172 | 32 | exact deployment-evidence account owner |
| 204 | 32 | deployment-verifier implementation id |
| 236 | 4 | deployment-verifier version |
| 240 | 32 | exact source data-account key |
| 272 | 32 | exact source data-account owner (= provider program) |
| 304 | 8 | pinned deployment generation |
| 312 | 4 | grid family id |
| 316 | 2 | grid version |
| 318 | 2 | zero padding |
| 320 | 8 | bucket seconds |
| 328 | 32 | canonical window-domain id |
| 360 | 8 | inclusive window start bucket |
| 368 | 8 | exclusive window end bucket |
| 376 | 8 | maturity bucket, exclusive |
| 384 | 8 | repair generation |
| 392 | 8 | page index, exactly zero in V1 |
| 400 | 8 | first bucket, equal to window start |
| 408 | 8 | authenticated feed cursor at seal, zero while open |
| 416 | 8 | predecessor source sequence |
| 424 | 8 | predecessor publish slot |
| 432 | 8 | predecessor publish time |
| 440 | 32 | predecessor sealed-archive commitment, or zero at genesis |
| 472 | 32 | this page's commitment |
| 504 | 1 | canonical PDA bump |
| 505 | 7 | required zero |

The `window_id` is SHA-256 over
`WINDOW_DOMAIN_TAG || WindowDomain::encode_canonical()`.  Initialization checks
that the domain uses complete-required coverage, has one through 32 buckets,
and names the exact source adapter id/version, feed-spec digest, and grid from
the authenticated source spec.  The evaluator version, maturity boundary,
repair generation, and full range remain committed by the canonical window
bytes rather than being inferred later.

### 3.2 Records: 32 × 64 bytes

| Record offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | canonical bucket |
| 8 | 16 | conservative low endpoint |
| 24 | 16 | conservative high endpoint |
| 40 | 8 | source-native sequence |
| 48 | 8 | source publish slot |
| 56 | 8 | source publish time |

Populated records are contiguous from slot zero.  Bucket `i` must equal
`window_start + i`; source sequence increases strictly; publish slot and time
never move backwards.  The first record continues the three predecessor fields
in the header, while each later record continues the prior record.  Every
unused 64-byte slot must be all zero.

The page commitment is SHA-256 over the domain string
`dragons-clutch/source-archive/v1` and every account byte except the commitment
field itself.  It therefore covers open/sealed state, count, every frozen
identity, predecessor, seal cursor, bump, reserved bytes, populated records,
and zero padding.  The program's safe native SHA-256 wrapper is used on SBF;
the same API supplies host equivalence.

## 4. Provider-neutral authentication seam

`DeploymentAuthenticatorV1` is a compile-time trait with constants for:

- reviewed verifier id and version;
- provider program key and its exact owner/loader; and
- deployment-evidence account key and exact owner.

Its only method receives bytes from those already metadata-authenticated
runtime accounts and returns a deployment generation.  `append_authenticated`
then requires that generation to equal the immutable source spec.  The price
side independently selects `PriceParserV1`, whose id/version/adapter release
must equal both source spec and archive header before `source::admit_price`
runs.

This is a closed-registry seam, not plugin permissionlessness.  A production
instruction must choose both generic implementations in its own match over
registered constants.  Letting instruction data or a caller select an
implementation would allow an authenticator that simply lies and is therefore
outside the relation.

Each append authenticates:

1. source-spec account key, owner, executable bit, body, and digest;
2. current page tag/version/canonical padding and prior commitment;
3. exact source, parser, deployment-verifier, grid, and window bindings;
4. exact provider program key/owner/executable bit;
5. exact deployment key/owner/non-executable bit and pinned generation;
6. exact source data key/owner/non-executable bit through `admit_price`;
7. source sequence and previous/current publish linkage;
8. cluster-slot and source-time freshness;
9. finalized canonical bucket and `publish_time / bucket_seconds` equality;
10. normalized positive price and absolute/relative confidence bounds; and
11. the next record's exact bucket and fixed page capacity.

No archive byte is changed until all those checks pass.  The final record write,
count increment, and commitment stamp have no fallible operation between them.
The hostile tests snapshot the entire 2,560 bytes and establish exact rollback
for wrong source account, wrong provider owner, stale data, excessive
confidence, wrong bucket, and premature seal.

## 5. Seal and resolution receipt

`seal_archive` accepts only a complete page and an authenticated feed cursor at
or beyond the frozen maturity bucket.  It sets the terminal flag and seal
cursor and recomputes the commitment.  Any later append returns
`AlreadySealed` without changing a byte.

`verify_sealed_archive` takes:

- the Dragon's Clutch program id;
- the canonical archive PDA already derived by the SBF seam;
- archive runtime key, owner, executable bit, and bytes;
- a metadata-authenticated source-spec capability; and
- the immutable `WindowDomain` reconstructed from market terms.

Only after every check passes does it return `SealedArchiveReceiptV1`.  The
receipt has private fields and exposes the archive key, feed/spec digest,
window id, page commitment, deployment generation, range, sealed feed cursor,
and terminal source lineage.  It can project into `source.rs`'s narrower
`AuthenticatedArchiveV1` only after this stronger account verifier has run.

The later `Resolve` join should accept the receipt (or invoke this verifier
immediately before folding records).  It must remove the current
caller-supplied evidence alternative rather than retain it as a compatibility
path.

## 6. Exact evidence and remaining STOPs

Executed locally on 2026-08-19:

```text
cargo check --manifest-path programs/clutch-sbf/program/Cargo.toml --offline
  PASS (three pre-existing dead-code warnings in order settlement)

cargo test --manifest-path programs/clutch-sbf/svm-tests/Cargo.toml \
  --test source_archive --offline
  4 passed; 0 failed

cargo-build-sbf --manifest-path Cargo.toml --offline
  produced the release ELF; emitted the repository's pre-existing oversized
  frame diagnostics in host-only buffered layout/reference functions, and no
  source-archive frame diagnostic
```

The tests pin 292 and 2,560 bytes and cover a complete three-record archive,
source-spec content addressing, parser/deployment releases, predecessor
lineage, maturity seal, receipt fields, content tampering, terminality, full
rollback, and the same-domain/different-account substitution attack.

This is host execution of the exact `clutch-sbf` library in the SVM test
workspace.  It is **not** bank execution and reports no compute units because
no routed instruction owns this ABI yet.  Recording host timing as SBF CU would
be misleading.

The following remain explicit STOP conditions:

1. No production `DeploymentAuthenticatorV1` exists.  A Pyth implementation
   requires a pinned official loader/receiver interface, exact program and
   deployment/config accounts, upgrade/generation semantics, and hostile
   fixtures.
2. No production `PriceParserV1` exists.  In particular, a current-value price
   account does not establish a unique finalized record for a historical
   bucket merely because it carries a timestamp.
3. The live `FeedAdvance` route does not create/authenticate these accounts or
   read the canonical Clock sysvar for this relation.
4. The live `Resolve` route does not yet require
   `SealedArchiveReceiptV1`; its separate caller buffer remains
   non-qualifying.
5. The proposed two PDA preimages and bump checks must be integrated into the
   shared seed/dispatch ABI and tested against the real program ELF.
6. `authenticated_feed_cursor` at seal must come from the program-owned
   authenticated feed head.  The pure function's integer argument is not on its
   own feed authentication.
7. Multi-page windows, missing-record proofs, repair replacements, and source
   migration are unsupported.  V1 fails closed rather than inventing them.

Until items 1–5 close, the honest description is: **fixed authenticated-source
archive semantics and codecs, with a mock adapter and a callable sealed receipt;
not a source-authenticated deployed resolution path.**
