# General semantic-Product-ID OpenBatch — 2026-09-08

This is an accepted owned-local-validator observation. It is a **host/runtime
source-split diagnostic**, not a checked release or devnet claim. It records
one completed `OpenBatch` against the preserved a7 ledger and no later General
act.

## Inputs and provenance

| Fact | Value |
| --- | --- |
| Ledger / RPC | `/tank/dregg-build/general-a7-fresh-found-20260907`; `http://127.0.0.1:22274/` |
| Ledger genesis | `6kAr6wYNbkPUYoF3YjrARNJP2eGQJoTopewDdtTwpwL9` |
| Market / General root | `5gtcAGXAqqdRU7oEK4B9oTT7rbve7uvAxpdteoCR5a7P` / `ARKw8jbNnq8uqGbWXxixseyS7nnuzi3Cok1bsLXcNyR1` |
| Payer | `AvdDne13Q3U3ahoYeQrbqJa2sKsjjrjAeU2rHzmNuAD3` |
| Runtime gate | `/tank/dregg-build/dclutch-strict-a7f9f226-20260907/candidate/CHECKED_UPGRADE_GATE.json`; SHA-256 `6d02cebd1980ce2d84d6dc7b33533e6f40a2044a4159b5bf95334bcb6b467abe` |
| Runtime source / tree | `a7f9f226d13e9fba49d746f64f5a2f34f43d0dd4` / `675a6c6ea161cc7ca3283207f4dd97f67362a8ec793b185dd30900892069c815` |
| Host source | `ebcc5a17e82771ea2f85491e26e2580994c2dcdb` (`operator: derive General batches from product identity`) |
| Host binary | `/tank/dregg-build/dclutch-general-openproduct-ebcc5a17/target/release/dclutch-local-successor-bootstrap`; SHA-256 `77e43d8f85edb5a19c3ad401eade4186fbff44e250481ac3ed0095d7543d6ab4` |

The host source was exported from the committed tree and built on hbox through
`swarm-build`; it was not built from the shared dirty worktree. The runtime is
the sealed a7 artifact cohort above. These are intentionally named separately:
this result does not establish a single-source release.

## The repaired identity boundary

The authenticated Product graph carries two different values:

- the finalized Product-record digest, which authenticates the Registry
  coordinate; and
- the Product body's semantic `product_id`, which `AccountProfile` projects
  into `identity::SELECTION_PRODUCT` and `GeneralBatchOpeningV1` commits.

Before `ebcc5a17e`, the host fed the record digest into the OpenBatch occurrence
and Batch PDA derivation. The runtime read the semantic Product ID, so it
refused the proposed occurrence with `0x4004` before the accelerator call.
The commit keeps `product_record` for record-bound General actions and passes
`product_id` only to the OpenBatch occurrence; its loopback session uses the
same semantic ID for the primary Batch state PDA. The focused Rust control uses
deliberately distinct record (`0x94…`) and Product (`0x43…`) identities and
proves the occurrence differs. It passed:

```text
cargo test -p dclutch-operator \
  general_hot_v3::tests::open_request_derives_the_slot_independent_batch_occurrence \
  -- --exact --test-threads=1
# 1 passed
```

Both `cargo check -p dclutch-operator --locked` and
`cargo check -p dclutch-local-successor-bootstrap --locked` passed at the
committed host source.

## Capability seal and route construction

The preceding real seal remains part of the accepted frame:

| Fact | Value |
| --- | --- |
| Seal | `Ea3HWgcm9Umd3sMgEpQySNJMetjz4pc4RKKXRqpXe1Dv` |
| Seal signature / slot | `5WL6NqMx3SN3RjGj4xsxaDVygw6MBwih4AcacoNBumwv95qd7cnDx6b3Vok4WJM4Jw88uRj3oR8PPw5iM3CgATPs` / `40470` |
| Seal fee / CU | `75,000` lamports / `636,454` CU |
| Seal poststate | Trading-owned, `968` bytes, `7,628,160` lamports |
| Final OpenBatch LUT | `HfSmSpYZutJNQUdUzSJrmHc5so8vjXuaWbkhrdyews6N`, 53 exact addresses |
| Final family request digest | `593e823bb7ab7168b1c856b8a3dca6e44734d69f796ad488962fc869989122de` |

The bootstrap and exact frozen LUT runs are separate, recorded transactions;
the final route was re-emitted with the signed request digest and exact table.
No account list was hand-assembled.

## Accepted execution and read-back

The final preflight had `simulatedError: null`; submission then landed:

| Fact | Value |
| --- | --- |
| Signature | `4L77T33P5HsSevSGbYwb7ffNjyP5nq6gDhDTBhxCVsA2kuv57uBzXTPH2sXC1NYoUJb6H6paiSkBX7FHBgNrSbsH` |
| Slot | `51681` |
| Fee / CU | `5,000` lamports / `1,067,412` CU |
| Batch PDA | `qFBE4W9xtePiuntp3cFm1ns4AaQ7UkuobnmKdDXPubn` |
| Read-back | Trading-owned (`7STQupW8XtQfXWNjhoSWJETXF2uRtaKtkhGuC89AAbAc`), `424` bytes, `3,841,920` lamports, magic `DCGLST03` |

The plan recorded the primary state as vacant before execution, at logical
coordinate 5 with bump 255. The read-back is the poststate, rather than an
inference from the send.

## Immutable hbox artifacts

All paths below are immutable files on the preserved ledger directory.

| Artifact | SHA-256 |
| --- | --- |
| `general-openbatch-productid-frame-final.json` | `330044f1b6ff068194ed45bb8e035a1af86447c2f970a9b2cccc7c914f4d8d76` |
| `general-openbatch-productid-route-final.json` | `27268c4038a3bcfa28d3899fb95096bf66e7436311b426f0d115d3d35a4fd298` |
| `general-openbatch-productid-plan-final.json` | `0e4becffcf86c83f489fe4ec890dededcaea2d78f42fb2ae169c89c6c4475938` |
| `general-openbatch-productid-execute-final.json` | `fce368ae2eb9de80ec14250b884b1c4cefec68192a3cbddb252547eaafd10e25` |
| `general-openbatch-productid-lookup-bootstrap.json` | `745704c8ac8fc2944a6e588c11e8a68879c17195e898bfbb682c76951a704091` |
| `general-openbatch-productid-lookup-exact.json` | `f8a4ad69ffe6af6e5efa95cf3ca766644547cf1ec129efeb801857831e31d514` |
| `general-openbatch-capability-seal-2.json` | `b33bc8487531f9608b086c5fd60a05f86d281b159e133f27171df8e26695d2e6` |

## Controlled limit

This evidence stops after `OpenBatch`. It neither runs nor claims `PlaceOrder`,
`CancelOrder`, `CloseBatch`, or any exit. The preserved Trading artifact is
known to exhaust its compute budget on the exact nonzero PlaceOrder ProgramTest
path before accelerator invocation; the reported profile reaches
`admitted-cpi-index` at a 57,110-byte heap total and then reaches the 1,400,000
CU ceiling. The next host campaign must use the fresh Trading artifact after
the funded-seal join repair and must derive its maker signed-order terms and
actual Claims/Custody children from chain facts. The a7 ledger remains
preserved as the OpenBatch evidence source.
