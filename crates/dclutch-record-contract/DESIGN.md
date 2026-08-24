# Immutable record creation contract V1

This crate owns the pure state machine for permissionless creation of large,
immutable protocol records. It is `no_std`, allocation-free, SDK-free, safe
Rust. It does not contain a hash implementation or claim to mutate an SVM
account.

## Semantic identity and raw account

One record is identified by:

- an opaque 32-byte schema/validator-release identity; and
- the nonzero 32-byte digest of the exact semantic byte sequence.

The SVM adapter derives exactly one raw PDA from
`("dclutch-raw-record-v1", schema_release_id, expected_digest)` and one staging
PDA from
`("dclutch-record-stage-v1", schema_release_id, expected_digest)`. The program
ID remains the SVM derivation boundary. Page size, sponsor, expiry, and staging
policy are deliberately absent from the raw PDA identity, so a later staging
profile can lift transaction geometry without changing the content address.

Finalized raw-account data is exactly the semantic bytes. There is no magic,
schema duplicate, digest duplicate, finality byte, bitmap, or other header in
the permanent account, so those facts do not impose duplicate rent.

An address is not an authentication receipt. Every later consumer calls
`authenticate_finalized_raw_record_v1`, whose adapter obligation requires all
of the following in the same observation:

1. derive the raw and staging PDAs from the exact seeds above;
2. require the raw account's selected program owner, exact observed data, and
   deployment-defined rent/lifecycle rules;
3. prove the canonical staging PDA is vacant;
4. hash the entire raw-account data, including every zero byte, with the digest
   policy selected by the release;
5. exact-match the result to `expected_digest`; and
6. invoke the schema-specific hostile semantic validator selected by
   `schema_release_id` over the same exact bytes.

The contract returns a move-only `AuthenticatedRawRecordV1` only after that
adapter callback succeeds. A staged record whose unwritten suffix happens to
equal the intended zero suffix still cannot authenticate because its staging
PDA is live. No transition scans raw data for zeroes to infer progress.

## State and exact encodings

All integers are little-endian. All reserved bytes must be zero.

`BeginRecordV1` is 176 bytes:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | `DCLTRIX1` |
| 8 | 2 | schema version `1` |
| 10 | 1 | action `Begin = 1` |
| 11 | 5 | reserved |
| 16 | 32 | schema/release identity |
| 48 | 32 | expected content digest |
| 80 | 8 | exact raw byte length |
| 88 | 1 | page-envelope evidence kind |
| 89 | 3 | reserved |
| 92 | 4 | maximum bytes per Append page |
| 96 | 32 | measurement-manifest or lifting-plan identity |
| 128 | 32 | authenticated staging-liveness policy identity |
| 160 | 8 | absolute expiry slot |
| 168 | 8 | separately prepaid cleanup bounty in lamports |

`StagingCursorV1` is 296 bytes:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | `DCLTRCR1` |
| 8 | 2 | schema version `1` |
| 10 | 1 | live status `Building = 1` |
| 11 | 1 | page-envelope evidence kind |
| 12 | 4 | reserved |
| 16 | 32 | schema/release identity |
| 48 | 32 | expected content digest |
| 80 | 32 | page-envelope basis identity |
| 112 | 32 | staging-liveness policy identity |
| 144 | 32 | raw-record PDA |
| 176 | 32 | staging-cursor PDA |
| 208 | 32 | immutable sponsor signer/rent refund |
| 240 | 8 | exact raw byte length |
| 248 | 4 | maximum bytes per page |
| 252 | 4 | reserved |
| 256 | 8 | checked total page count |
| 264 | 8 | next required page index |
| 272 | 8 | next required raw byte offset |
| 280 | 8 | absolute expiry slot |
| 288 | 8 | cleanup bounty in lamports |

`AppendPageV1` is a 40-byte fixed header followed by the exact semantic page:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | `DCLTRIX1` |
| 8 | 2 | schema version `1` |
| 10 | 1 | action `AppendPage = 2` |
| 11 | 5 | reserved |
| 16 | 8 | page index |
| 24 | 8 | raw-account offset |
| 32 | 4 | trailing page length |
| 36 | 4 | reserved |
| 40 | dynamic | exact page bytes |

Finalize and Abort are each the 16-byte instruction header with action `3` or
`4`; neither accepts a caller-authored digest, semantic-valid flag, finality
flag, refund account, or bounty amount.

## Checked geometry and lifting

Total byte length and page count are `u64` and use checked arithmetic. There is
no fixed maximum total artifact size in the contract. A composing SVM adapter
must checked-convert the committed byte length to its runtime account-size
domain and may refuse a runtime limit without changing the semantic contract.

Each Begin commits a positive `u32` page width and an evidence kind:

- `Measured`: the basis identity names reproducible transaction-envelope
  measurements for the selected deployment/release.
- `Provisional`: the basis identity names the required lifting plan.

The adapter authenticates the basis selected by its release. A later measured
profile or cursor schema can lift page width while retaining the same raw PDA
and semantic bytes. An in-progress V1 cursor completes or aborts under its
immutable V1 geometry.

## Transitions and atomic adapter obligations

`prepare_begin_v1` verifies distinct accounts, canonical PDA attestations,
positive checked geometry, an authenticated staging policy, a strictly future
expiry no farther than that policy's maximum lifetime, and a cleanup bounty no
smaller than policy minimum. The sponsor prepays raw rent, cursor rent, and the
separate bounty. No Hoard principal, future fees, or implicit fee revenue is a
funding source.

`prepare_append_page_v1` accepts only the exact `next_page`, exact
`next_offset`, and exact expected page length. Replays, reorderings, overlaps,
gaps, and overlong or short final pages are separate refusals. It returns one
`RawPageWriteV1` joined to the one next cursor. The adapter must run the entire
preflight before mutation, then copy the page and encode the next cursor in one
SVM instruction. Runtime rollback is the atomicity boundary; the pure function
never mutates its input cursor or caller-owned raw bytes.

`prepare_finalize_v1` requires complete cursor geometry and passes the entire
raw account to the adapter's exact hash and schema validator. Finalize is
permissionless, including before expiry. After validation, the adapter retains
the headerless raw record, returns the complete staging-account balance
(cursor rent, unused cleanup bounty, and any surplus) to the immutable sponsor,
and closes the staging account. The returned authentication receipt is valid
only in that instruction.

Before expiry, Abort requires the immutable sponsor as an SVM signer and sends
the complete balances of both raw and cursor accounts back to that sponsor. At
or after expiry, any caller may clean an incomplete or poisoned staging pair.
The exact prepaid bounty goes to the cleanup recipient; the complete raw
balance and every remaining staging lamport go to the immutable sponsor. The
contract checked-subtracts and rechecks the staging split, so cleanup cannot
redirect rent or surplus. Repeated squatting therefore repeatedly prepays and
forfeits the authenticated minimum bounty.

Finalize and Abort consume the same live writable cursor and are mutually
exclusive under SVM account locking. After either closes it, stale cursor bytes
are not an admissible live account observation; Abort-after-Finalize therefore
has no cursor authority.

## Next SBF seam

The next implementation owner should add one small record instruction adapter
with these exact account contracts:

- Begin: sponsor signer/writable, vacant canonical raw PDA, vacant canonical
  cursor PDA, authenticated clock/rent/system accounts and release-selected
  liveness/page policies.
- Append: writable raw PDA and cursor PDA; decode both before any write; apply
  the returned page write and next cursor atomically.
- Finalize: read-only complete raw PDA, writable cursor PDA, sponsor refund,
  clock if the adapter records its observation, selected hash implementation,
  and schema-release validator dispatch; return all cursor lamports and close.
- Abort: writable raw/cursor PDAs, immutable sponsor refund, authenticated
  clock, and cleanup recipient; require sponsor signature before expiry or
  apply the exact expired bounty/remainder plan afterward.
- Consumer authentication: read-only raw PDA plus an authenticated vacancy
  observation for the derived cursor PDA, followed by exact hash and semantic
  validation before minting any protocol authority.

The SBF layer must hostile-decode account owner, length, rent, signer,
writability, clock, and PDA derivations. This crate's adapter callbacks and
transition receipts specify those obligations; they are not evidence that the
operations already occurred.
