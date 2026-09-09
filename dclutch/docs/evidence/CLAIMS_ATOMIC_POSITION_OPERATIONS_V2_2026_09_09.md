# Claims atomic Position operations on the V2 runtime — 2026-09-09

## Boundary

This is real-ELF `solana-program-test` evidence, not a local-validator or
devnet lifecycle. Both campaigns install an already compiled Product/LBV2
graph. The sparse chain also installs a previously admitted source Position
and admission record, then executes one real atomic destination Admit → sparse
transfer → source Close. It is not evidence of funding, trading, or resolution.

## Runtime and build identity

- production source: `a8e3b4e8c593a5a63eef94a51375f46ba5ad75d2`
- candidate: `/tank/dregg-build/dclutch-strict-a8e3b4e8c-v2-native-completion-20260909`
- Claims ELF: `ed5cc2b3b97d927564774bc1887f6689b2d90aeedd5fcd4089bb7c90d2ace183`
- Registry ELF: `48ae0e74a7146dedf4af46c6b97a66a93425ad3d87447f6eae8d0e016fc1608f`
- Core ELF: `d242da365eac4e7f6cea239b2649d2dd9ab6d358bf38be55974ccb09f30a492d`
- Rent ELF: `6d00dfde5c764c7039e58df784ec91a88aa216e513a9f5fb79815a10cc358ac7`
- checked-upgrade gate: `46af94470e79e86d7ceeb456574baddf1ea4c8bc705f944273b34e2df91dc90a`
- reproducible release gate: `75d07a91b44103883eb87b9102b0692bd96c37d379b55a50cefc98c34ba364b3`
- successor campaign pack: `0d2442e660b7ff4ad27b7e27e1db58bf4ed992777745ad47e358bf76ad19920d`

The build owner temporarily removed referenced frame objects during cleanup.
The production ELF and gate bytes did not change. All eight exact frame objects
were restored from the same source and toolchain; descriptor checks, the
release-gate verifier, and the campaign-pack verifier then passed again with
the hashes above.

The isolated exact-source checkout and Cargo target were
`/tank/dregg-build/dclutch-claims-sparse-a8e3b4e8c-v2-20260909/{source,target}`.
Only the required auxiliary callers were linked under `swarm-build`:

- sparse-chain caller: 31,392 bytes,
  `d09cde96b099426e838cd21d87bfa864e79977d5b9a980de8bc6cdb716687869`
- affine-batch caller: 41,680 bytes,
  `2b8ecb4d26d0c2810b6e9aa69f567ab992787b0f2335331dbd29ad2edc75a7f9`

Their build logs contain zero stack-frame diagnostics and zero
`Unit run-u<N>.scope was already loaded` scheduler no-op markers.

## Sparse Admit → transfer → Close

The filtered invocation executed only
`real_sbf_admit_transfer_close_chains_by_exact_receipt`, with `--exact` and
`--test-threads=1`: **1 passed, 0 failed, 0 filtered**.

The first real run refused Admit at exact Claims `0x5146` (`Rent`): the shared
fixture had left the first Trading Position owner in Core's provisional
`rent_beneficiary`. Commit `da8215e90` decodes Core state, changes only that
field to the derived lifecycle RentCredit, and preserves both Position owners.
The same commit pins every hostile runtime refusal to its registry-derived
code and checks all touched accounts byte-for-byte after refusal.

| control | result | transaction CU |
| --- | --- | ---: |
| zero transfer quantity after Admit | exact Claims `0x5260` (`Instruction`); aggregate, both Positions, both admissions and RentCredit unchanged | 192,494 |
| substituted admission receipt owner | exact Claims `0x5264` (`ClaimsState`); the same six accounts unchanged | 236,322 |
| caller refuses after all three Claims stages return | exact caller `0x105003`; the same six accounts unchanged | 388,151 |
| canonical Admit → transfer → Close | accepted; destination holds 9,001, aggregate supply is unchanged, source Position and admission are reclaimed, and RentCredit receives exactly their two rent principals | 388,146 |

## Affine group operation

The filtered invocation executed only
`real_sbf_affine_batch_is_runtime_width_exact_and_atomic`, with `--exact` and
`--test-threads=1`: **1 passed, 0 failed, 1 filtered**. Commit `c19063090`
replaces the runtime cases' bare acceptance checks with exact, registry-derived
refusal assertions while retaining byte-identical rollback checks.

| control | result | transaction CU |
| --- | --- | ---: |
| aliased Position account | exact Claims `0x5161` (`Accounts`); Claims state unchanged | 37,198 |
| substituted Product record | exact Claims `0x5163` (`ProductBasis`); Claims state unchanged | 114,102 |
| substituted linked-basis record | exact Claims `0x5163` (`ProductBasis`); Claims state unchanged | 155,058 |
| caller refuses after Claims returns success | exact caller `0x100003`; aggregate and both Positions unchanged | 177,062 |
| canonical two-row affine batch | accepted; aggregate and both Position revisions become 1, outcome 0 becomes aggregate/destination 7, and outcome 257 becomes aggregate/destination `u64::MAX` | 177,056 |
| replay of the now-stale batch | exact Claims `0x5164` (`ClaimsState`); accepted poststate unchanged | 107,646 |

Logs are under
`/tank/dregg-build/dclutch-claims-sparse-a8e3b4e8c-v2-20260909/logs/`:
`sparse-chain-original.log`, `sparse-chain-exact-refusals.log`,
`affine-batch-original.log`, and `affine-batch-exact-refusals-fixed.log`.
