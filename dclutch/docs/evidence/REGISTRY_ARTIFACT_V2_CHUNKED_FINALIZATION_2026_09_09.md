# ArtifactReleaseV2 bounded code verification — 2026-09-09

Registry admission now verifies the complete Loader code payload in consecutive
1 MiB chunks. The code commitment binds the total length, chunk profile, ordered
offsets, exact chunk lengths and native SHA-256 of each actual chunk. Flat ELF
SHA-256 remains offchain build provenance. ArtifactReleaseV2 replaces V1; it does
not preserve a parallel accepted codec.

The existing temporary staging account owns verification progress (344 bytes
for ArtifactReleaseV2, 296 bytes for ordinary records). Every Finalize rechecks
the native Loader identity, ProgramData link, deployment slot, authority and
payload length. Only complete coverage and a matching commitment close staging
and refund it. Readers must continue requiring the canonical staging account
absent before trusting an artifact as finalized.

## Actual SBF runtime control

`programs/dclutch-registry-sbf/tests/artifact_commitment_v2.rs` ran the actual
Registry ELF in Agave ProgramTest, with a real Loader deployment padded to
4,194,304 bytes. This is SBF runtime evidence, not local-validator or devnet
execution and not a complete cohort build.

| Transaction | Actual CU | Result |
| --- | ---: | --- |
| Finalize chunk 1 | 542,117 | Progress 1 MiB; raw and cursor retained |
| Finalize chunk 2 | 542,020 | Progress 2 MiB; raw and cursor retained |
| Finalize chunk 3 | 542,020 | Progress 3 MiB; raw and cursor retained |
| Finalize chunk 4 | 542,604 | Complete; raw retained; cursor closed and refunded |
| Wrong final commitment | 545,015 | Exact `RegistryError::ArtifactReleaseCodeCommitmentMismatch`; cursor rollback |
| Native Loader Extend | 2,370 | Actual extension of 10,240 bytes |
| Continue after Extend | 12,424 | Exact `RegistryError::ReleaseSuperseded`; cursor rollback |

Both hostile partial publications were subsequently aborted through the real
Registry instruction. Each raw account and cursor disappeared; the sponsor
received their combined lamports minus the actual transaction fee. No code
payload or staging state was replaced through a harness setter between chunks.
The native Loader deployment feature profile explicitly permits the fixture
ELF's SBF version; the test sets the runtime transaction limit to 1,400,000 CU.

The accepted control is four transactions for this 4 MiB payload. The current
2,735,824-byte Trading build needs three, and the Loader maximum 10,485,715-byte
payload needs ten, derived from the same chunk profile. Those latter counts are
geometry, not claims of executed campaigns. The 1 MiB profile is provisional:
changing it requires a successor commitment schema and a measured full
Finalize transaction, because chunk geometry is part of the code identity.

## Provenance and remaining execution

Recovered completed job:
`/tank/dregg-build/dclutch-structured-root-v2-work-20260908`.
Its source directory is a recorded working overlay, not a Git checkout or
committed release. Recovery compared every Registry and Registry-SBF Rust source
and Registry manifest with live tree HEAD
`4bee7cc3af1313901253ea6860809d1a088d6c1a` plus its dirty V2 migration. All
implementation files matched; the only initial difference was an additional
cursor unit test. Subsequent changes were fixture corrections, formatting and
comments. Thus the runtime supports this implementation, not a claim that all
current source was built together.

- `registry-v2-recovered-implementation-sha256.txt`: exact recovered hashes
  for 26 implementation source files, SHA-256
  `b2aaed64e0db3afbd9e180366df6023ce037eacc2e72a11565fd7e0a1edf856b`.
- `registry-v2-runtime3.log`: SHA-256
  `866816ea729017895dea8a34a8bf126508f7859381586daefaef935192a87620`;
  one test passed, zero failed, completed 2026-09-09T02:04:15Z.
- Registry ELF `target/deploy/dclutch_registry_sbf.so`: SHA-256
  `48ae0e74a7146dedf4af46c6b97a66a93425ad3d87447f6eae8d0e016fc1608f`.
- `registry-v2-check.log`: native package check completed successfully.
- `registry-v2-sbf.log`: Registry SBF link completed successfully.
- `registry-v2-cursor-final.log`: the exact qualified cursor control passed
  1/1, refusing wrong schema width, incomplete upload with verification progress,
  noncanonical initial/completed progress and an artifact tail on an ordinary
  record. An earlier unqualified exact filter ran zero tests and is discarded.

- `registry-v2-resolution-fixtures.log`: six explicitly selected SVM-harness
  constructor controls each passed 1/1, binding the actual ELF tail to the native
  commitment, distinguishing flat SHA provenance and detecting changed bytes.
  SHA-256 `d53ceef6b89bc59b0959d1d5d87ead69bf8835b4bf42746a5ea656305743cd6a`.
- `registry-v2-activation-fixture.log`: the migrated accepted mutable-slot
  activation and subsequent supersession control passed 1/1.
  SHA-256 `7885e65e61cbe4ae7a4f1a268d0ec51249fb30c57354e67787a352b02c51dda5`.

The coherent V2 migration still owes an exact committed all-eight-program build,
frame baseline capture, complete economic lifecycle execution and a fresh devnet
cohort. This measurement does not discharge those obligations.
