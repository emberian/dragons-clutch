# Current-tree state and rent audit — 2026-08-22

## Status

This is a source-derived review of the post-Cycle-G tree, not a rewrite of
sealed historical evidence. Historical `evidence.json` rows retain the layouts
and artifact identity they measured. The next seal needs a separate current-tree
inventory that cross-checks live Rust constructors.

Default rent in the current local runtime is:

```text
minimum_balance(bytes) = (bytes + 128) × 6,960 lamports
```

Every removed data byte therefore saves 6,960 lamports. Removing an account
also removes its 128-byte rent overhead, worth 890,880 lamports.

## Safety result: two closes must remain disabled

`CloseGeneralEpoch` previously deleted Epoch + Window + their funding ledger.
That was unsafe for two independent reasons:

1. `InitEpoch` accepts a caller-supplied epoch index whenever those PDAs are
   absent. Market has no durable used-index cursor, so deletion reopens the same
   epoch identity and child PDA namespace.
2. Window records only the verified retained top three. Sealed-unverified,
   refused, superseded, and valid-but-noncompetitive candidates and their work
   accounts are not exhaustively enumerable from it. Their close routes require
   the terminal Epoch.

The safe interim is to retain the root and close every independently
authenticated child the keeper can discover. Retained root capital is:

| Root member | Bytes | Lamports |
| --- | ---: | ---: |
| General Epoch | 329 | 3,180,720 |
| General Window | 231 | 2,498,640 |
| Funding ledger | 85 | 1,482,480 |
| **Ledgered total** | **645** | **7,161,840** |

`ClosePosition` must also remain disabled. Sell placement transfers Eggs from a
Position into an ACTIVE Reservation. A locally zero all-in seller can therefore
still own reservation-held assets. Position V1 has no outstanding-reservation
counter and the close instruction has no exhaustive reservation proof.

## Current inventory corrections

Two checked-in current-looking tables drifted from the live constructors:

- `DIRECT_EPOCH_V4_BYTES` is **673**, not 672. Rent is **5,574,960**, not
  5,568,000 lamports.
- SourceSpec V2 is **404 bytes / 3,702,720 lamports**. The older inventory
  lists only SourceSpec V1 at 292 bytes / 2,923,200 lamports.

Do not modify the historical seal to hide this. Add a constructor-derived
current overlay and make the next inventory gate enumerate every live account
constructor and version.

## Largest state costs

| Family | Bytes | Rent lamports | Geometry |
| --- | ---: | ---: | --- |
| ClearWork | 50,054 | 349,266,720 | Per candidate; current largest cold-capital object |
| CandidateFeed | 6,266 | 44,502,240 | Per candidate |
| OrderPage | 4,012 | 28,814,400 | Up to four general pages |
| SourceArchive | 2,560 | 18,708,480 | Per archive; no close route |
| Terms final | 1,656 | 12,416,640 | Persistent prerequisite |
| ResolutionWork | 1,296 | 9,911,040 | Plus 890,880 zero-byte reserve |
| Kernel | 1,255 | 9,625,680 | Per Market |
| Market | 726 | 5,943,840 | Persistent |
| Direct Epoch V4 | 673 | 5,574,960 | Durable per direct epoch |
| Reservation | 618 | 5,192,160 | Up to 64 general reservations |
| SourceSpec V2 | 404 | 3,702,720 | Persistent |
| Candidate | 337 | 3,236,400 | Per candidate |
| General Epoch | 329 | 3,180,720 | Retained replay anchor for now |
| General Window | 231 | 2,498,640 | Retained replay anchor for now |
| Position | 220 | 2,422,080 | Per owner/Market; close currently disabled |
| Receipt | 217 | 2,401,200 | Up to 416, each currently separately funded |
| Funding ledger | 85 | 1,482,480 | Per optional ledgered group |

The stated four-page / 64-reservation / three-candidate / 416-receipt general
shape locks 3,281,702,640 lamports. It is not a protocol maximum: every extra
candidate with record, feed, ClearWork, and two ledgers adds 399,970,320
lamports. Thirty-seven such candidates exceed the current audited program's
deployment rent.

## Market cold capital

`CreateMarket` alone locks:

| Shape | 2 outcomes | 8 outcomes | 16 outcomes |
| --- | ---: | ---: | ---: |
| Categorical V2 | 31,354,800 | 40,124,400 | 51,817,200 |
| Native point V3 | 32,426,640 | 41,196,240 | 52,889,040 |
| Occupation V4 | 32,872,080 | 41,641,680 | 53,334,480 |

Each additional owner costs Position + Replay = 3,897,600 lamports, excluding
the owner's external Token-2022 account.

## Ranked format successors

These are design estimates, not implemented or measured runtime claims.

1. **ReceiptPageV1.** Sixteen existing 217-byte receipts plus one header and
   embedded funding identity fit in about 3,632 bytes. Twenty-six pages cover
   416 receipts for about 680,409,600 lamports, versus 1,615,610,880 for 416
   individual receipts plus ledgers: a projected saving of 935,201,280. Measure
   the extra write contention and CU before adopting it.
2. **Active-width ClearWork.** The current body spends 2,384 bytes on each of
   sixteen padded outcomes. A binary checkpoint can be 16,678 bytes instead of
   50,054, saving 232,296,960 lamports per candidate and 696,890,880 for three.
   Active counts must be immutable and omitted padding must reconstruct
   canonically.
3. **Embedded mandatory FundingTailV1.** A 56-byte payer/principal/donation/
   generation tail inside the governed account removes a separate 85-byte
   ledger and its account overhead, saving 1,092,720 lamports per group. No
   creation path may omit funding identity afterward.
4. **Dynamic CandidateFeed.** A versioned exact length
   `218 + 8×outcomes + 8×orders + 13×slices` reduces a minimal binary feed to
   263 bytes / 2,721,360 lamports, saving 41,780,880 per candidate.
5. **Specialized OrderPages.** A binary portfolio page projects to 2,220 bytes
   and a single-order-only page to 1,964, saving 12,472,320–14,254,080 per page.
   Keep the conceptual 16-outcome ceiling; specialize a named capacity profile.
6. **Active SourceArchives.** `512 + 64×record_count` makes an eight-record
   archive 1,024 bytes and a one-record archive 576 bytes. This needs a genuine
   retention/refcount/close policy; variable sizing alone does not solve
   permanent history growth.
7. **Binary Market profile.** Removing duplicated/derivable Market, Kernel,
   Hoard, Supply, and padded Position facts projects a 15,506,880-lamport
   reduction in two-outcome CreateMarket rent. Every removed fact needs one
   remaining semantic owner and account-meta/CU measurements.

## Terminal successor

Prefer a Market-owned monotone epoch generation if concurrent creation permits
it: roughly eight persistent bytes per Market are cheaper than a tombstone for
every epoch. If an inspectable tombstone is required, an 84-byte record containing
tag/version, Market, epoch identity, index, terminal generation, phase, and bump
costs 1,475,520 lamports. Replacing the current ledgered general root group with
that tombstone would refund about 5,686,320 per epoch.

Before root retirement can return:

- epoch identity must never become reusable;
- every child family must increment/decrement an exhaustive persisted count;
- candidate close must require its canonical full or growing ClearWork absent;
- the zero-count transition and tombstone write must be atomic;
- funding semantics must state who pays permanent tombstone rent; and
- old layouts must remain distinct rather than silently changing meaning.

## Deployment comparison

The audited current ELF is 2,105,728 bytes with SHA-256
`a56c7ce158dc0667fabbc6b9736699adf5e3495350cf8b56b7616bf56868e272`.
It locks 14.6582124 SOL persistently. Its exact loader buffer is recycled during
deployment rather than adding another permanent 14.657 SOL. The program costs
roughly as much rent as 467 binary categorical Market creations or 275
maximum-width V4 Market creations. Program capability profiles and state
capacity profiles are therefore both material; optimizing only one leaves the
other capital bottleneck intact.
