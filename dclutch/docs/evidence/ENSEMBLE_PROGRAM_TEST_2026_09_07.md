# Ensemble real-ELF ProgramTest slice — 2026-09-07

Owner: CODEX-ENSEMBLE. Evidence level: real-ELF ProgramTest in an in-process
bank. This document does not claim local-validator or devnet execution.

## Source and artifacts

The direct member capture was accepted from source commits
`4de2bcbd87317088ee1e918ed971078424d17a25` and
`a628bfd39dc971b5e37f20d9482290da3f9f6160`. The fold/reclaim poststate fixture
was adopted in `624b1a90d91de5d6fd774f6a6aa897fc4ab05578`. The repository root
was `/Users/ember/dev/dclutch`; the later integration HEAD is deliberately not
claimed as the ELF source.

The fresh Linux SBF build was made on hbox with `swarm-build`, `SWARM_MEM_MAX=32G`
and `CARGO_BUILD_JOBS=6`. Its Resolution link had no occurrence of either
`Stack offset.*exceeded` or `overwrites values in the frame`; the checked build
log is `/tank/dregg-build/dclutch-codex-evidence-20260907/ensemble-direct-member-build.log`.
The ProgramTest artifacts are retained locally at
`/private/tmp/dclutch-ensemble-member-programtest-elf-20260907`.

| ELF | SHA-256 |
| --- | --- |
| `dclutch_resolution_proof_sbf.so` | `c381428fbeb8e266b2824257491c417c1acb0e57dcbd7f25542928e7c27347e5` |
| `dclutch_core_sbf.so` | `7013924151b5fbd1f3d5abf0c88fb6199adaf2a6ba7f2a91da89014c4cf086a9` |
| `dclutch_registry_sbf.so` | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| `dclutch_custody_sbf.so` | `178bb636165036ef061dc2268868c845fed47e3f9bdd1632d2c06161b242a5b7` |
| `dclutch_claims_sbf.so` | `19a3be2fcb625bf56166f8b638adba2e7d970421d1a08e9fdd1bc63d7ddaef5d` |
| `dclutch_trading_sbf.so` | `27988083edccb86cb190e1317eece27478a586e425155133a75082a48d2fdffd` |
| `dclutch_accelerator_sbf.so` | `1fd892a8ad19baa55a046b263fd8f07286beed5a8fc1e967dbfeef67155776bb` |
| `dclutch_rent_sbf.so` | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| `spl_token_2022.so` | `e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697` |

## Accepted cases

The direct production entry was run with the real Pyth Router VAA verification
and Receiver `post_update` boundary; it did not replace provider execution with
a native decoder:

```sh
SBF_OUT_DIR=/private/tmp/dclutch-ensemble-member-programtest-elf-20260907 \
cargo test -p dclutch-svm-harness --test resolution_core_v3_lifecycle \
  a_real_pyth_member_capture_writes_a_fragment_and_keeps_primary -- --test-threads=1
```

It accepted a direct Resolution caller for declared ensemble member one, wrote
that member's fragment seat, consumed the provider lifecycle update, retained
the receipt resolver, and asserted the Source remains `Primary` with
`attempt_index == 1`. Capture consumed 191,992 CU.

The existing real-ELF fold/reclaim case was executed serially:

```sh
cargo test -p dclutch-svm-harness --test relayed_mainnet_state \
  captured_ensemble_fragments_fold_then_reclaim_the_vacant_member_seat -- --test-threads=1
```

It accepted the quorum fold, decoded the durable `EnsembleFoldReceiptV1`, and
returned the never-written member seat's prepaid rent through
`ReclaimMemberSeat`. That fixture starts after member fragments exist. It is
therefore poststate evidence only; the direct-Pyth case above is the producer
evidence, and neither substitutes for the validator campaign.

## Remaining boundary

At this evidence date, a canonical founded ensemble needs two member attempts
and four selected Resolution-controller rows: both member allocations, policy,
and material. The ProgramTest producer uses a pre-founded Source fixture because
the then-live founding ledger was fixed at three rows. The validator campaign
is contingent on the bounded dynamic-ledger lift, and will record separate
validator source/runtime identities and transaction poststates.
