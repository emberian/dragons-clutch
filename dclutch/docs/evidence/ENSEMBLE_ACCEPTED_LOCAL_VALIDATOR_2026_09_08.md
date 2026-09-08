# Ensemble producer, three captures, fold and reclaim — accepted local validator

Date: 2026-09-08. Evidence level: retained local-validator execution against
real SBF ELFs. This is neither devnet nor mainnet evidence.

## Verdict

The retained four-member, quorum-three Ensemble campaign now has the complete
vertical it was built to prove. Three independently posted and verified Pyth
updates produced three Resolution-owned member certificates. The unchanged
Resolution ELF then accepted one quorum Fold and one reclaim of the uncaptured
fourth member's prepaid seat.

The original fold refusal was an operator preparation defect, not a Source
transition defect. Immediately before the accepted retry, the canonical success
certificate, fold receipt and fourth member seat were all absent. The fold
receipt is created and funded by the fold itself. The success certificate must
already carry 3,062,400 lamports, because `initialize_certificate_at_kind`
requires the System-owned coordinate to meet the 312-byte rent minimum before
allocating it. The fourth member seat must likewise be prepaid so the terminal
can reclaim its rent. Commit `ed2c420b88e7d8ba5cf050bf12850918b7db45fe`
made that preparation explicit, retained the exact before/after posture in the
transcript, and applied the same rule to both fresh and resumed campaigns.

This explains the earlier `ResolutionError::OutputState (0x8002)`: the
certificate initializer is the first output action in `commit_ensemble_fold`,
and the observed absent certificate could not meet its rent predicate. The same
deployed ELF accepted once the two canonical coordinates were funded. No
refusal was weakened and no program was upgraded.

## Source and retained substrate

The program cohort is the sealed exact-551 candidate:

- source revision: `551ffc1b99abdf01478e7e963a1ac20a99e6c0f6`
- source-tree manifest SHA-256:
  `63c3915387c7a36458d3aa7d18a42fd2e7237c315f92a6154ec42861a0d7e843`
- checked gate:
  `hbox:/home/hbox/dclutch-strict-551ffc1b-ensemble-20260908-retry/candidate/CHECKED_UPGRADE_GATE.json`
- checked-gate SHA-256:
  `8c6357f8e2fa257d574fde4913cd67ca54b873584360b1b6d2f630f49760f621`
- Resolution program: `7YHYtFvQG65mAxqGvn5oeTX5SMVVNoyk3165DHc5rh4R`
- live Resolution ELF SHA-256:
  `aa1efaa57dd24d1e27725df37d32132088fa972f32697e868e3063f416ba129c`

The ledger remains at
`hbox:/home/hbox/dclutch-ensemble-four-member-q3-551ffc1b-host4d-port43320-20260908/work/substrate/ledger`.
The untouched pre-resume copy is
`hbox:/home/hbox/dclutch-ensemble-four-member-q3-551ffc1b-host4d-port43320-pre-resume-backup-20260908/substrate`.

The retry host was built from clean revision
`f749f3e2bdee79caa04da37b9e1b1763245a1b62`, which contains `ed2c420b8`, at
`hbox:/tank/dregg-build/dclutch-ensemble-terminal-ed2c420b8/source`. Its
checkout-specific Cargo target is beside that source root. Both
`cargo check --locked -p dclutch-ladder-campaign` and
`cargo build --release --locked -p dclutch-ladder-campaign` ran through
`swarm-build`; the logs contain neither the scheduler no-op marker nor an SBF
stack diagnostic. The host binary SHA-256 is
`ae011716cf3028143ca1e5068b42ec2bfeb862db631c1f8ad5e810e44786c025`.

## Accepted actions

| action | slot | CU | signature |
|---|---:|---:|---|
| prepay canonical success certificate and member-3 seat | 20,765 | 600 | `5sCnNcPCfbe9XvENyDCb4PdgRXrRsVSZbSX9ikjRxiLNP758323k8J6oejZi2Y6DGxjGAAF1G4xmY6G35hxzyXKG` |
| fold members 0, 1 and 2 | 20,927 | 383,676 | `ukYyq4xpZHk3N59NQFBhtVtuQE71Wsjw4JYQwtZRpZa1yezAyU774p9FuNh8BEBshWQnJH4RHACjiVVmv9RZeTt` |
| reclaim member 3 | 21,057 | 41,986 | `RQkogBYrJ2oGTmECtZvorAM1rVUg88sMr5uRHrNhSYAXYWycE2tuKWsmUmNZy2chGSDrZDGNeZTd2njYENMAGLN` |

All three transactions are finalized with `err = null`. The transcript also
retains the accepted address-table creation, extension and freeze transactions
used by each wide instruction.

The three producer captures were already finalized in the original campaign:

| member | capture slot | certificate | certificate SHA-256 | reading | captor Fold delta |
|---:|---:|---|---|---:|---:|
| 0 | 8,946 | `2DZqH4Nj2sPD8QRQkZuJPAx1eBBBGoNzVnxATDgAfsk6` | `313438e34ba29474753cd2402cf83d2f0595724e35444fdf81aca4fd05b21b93` | 100,000,000 / 1 | 0 |
| 1 | 9,526 | `8o3ADy6aXCDBuTPbTTTPFPzM6PaAFHG7Vr4TYtWzqXkv` | `f2166d2f4ed6798c0434c6a1cc317ea1cfdf01b7ce8fedbfe48e5c506d1be09a` | 100,000,000 / 1 | +1 lamport |
| 2 | 10,105 | `5qYy6QyJhBvuCLmybwnokvCkWRcFhsiPk398PqqHJhhy` | `69ae304fbfa94e26e7034e002902cb02b987bacfb40860a8cd5cc43a075e74b1` | 100,000,000 / 1 | +1 lamport |

Each member certificate is 312 bytes, owned by Resolution, generation 2 and
kind `ResolutionSuccess`. Their distinct provider-evidence digests remain in
the retained accounts.

## Actual poststates

The Source account
`8VY4eSZZoH3Fupik3YcY36eX35mcGsYMoHypFu2Ld13i` is Resolution-owned, 224
bytes, generation 2, phase `Resolved`, selector 0 and terminal sequence 1. Its
resolution-evidence digest is
`360770082d962c484b5bb6c685dd84144a16c3be671cabe2b98d82f98b8ddb5c`;
its complete account-data SHA-256 is
`da43e5f92051c7247d2fd7e10e461366c360d883fa5ed2ee73db9d47a2818533`.

The terminal certificate
`DKNuQGdiJndyDYSzJWbc7UArBG5qEqf2aLyxXJuAEY7U` is Resolution-owned, 312
bytes and `ResolutionSuccess`. It records generation 2, selector 0, result
100,000,000 / 1, terminal observation time 1,788,856,402 and the same evidence
digest as the Source. Its account-data SHA-256 is
`d293d7f7de72332b20e0b8379718f9267f133d1cbec25ae4c23a3a7d0bbca7ff`.

The fold receipt
`XqAmrNYBgCdGyVKx6TERzQn3ZNkA3VJnupqBJzkayfE` is Resolution-owned and 280
bytes. It records members 4, quorum 3, consumed count 3, bitmap `0b0111`,
median 100,000,000, selector 0 and terminal sequence 1. Its first three
fragment digests equal the three member-certificate SHA-256 values above; its
fourth digest is zero. Its account-data SHA-256 is
`1175b538fc7e0b7fed737dface0b8e264d87a5145ee8e8477987a5568f431851`.

The funding ledger
`9ai7t2jtU559u4ExD5CkvfUCZhpiTZe9BZXHU6JEDXZz` moved from 3,730,565 to
3,730,563 lamports in the Fold transaction. Exactly two secondary captors each
received one lamport; the primary captor received zero, matching the member
bounty plan. The poststate is 408 bytes and has SHA-256
`0ef41bd8323bb4d992def369c3118c59325dad72b04ebe03459685a6e7826c26`.

The member-3 seat
`855oe1ChEk6yxdr8qCxKDSfamNGJVbtj9zexkbJkdPn5` entered Reclaim with
3,062,400 lamports and ended absent with zero. Source's persisted rent
beneficiary `89wBRYK6YuJWFLoNEg1hidjHBL4XeYTBAQAc7RFxeNWB` moved from 18,075,130
to 21,137,530 lamports in the same transaction: an exact 3,062,400-lamport
credit.

## Retained evidence and controls

- accepted transcript:
  `hbox:/home/hbox/dclutch-ensemble-four-member-q3-551ffc1b-host4d-port43320-20260908/ensemble-resume-fold-reclaim-accepted-v3-20260908.json`,
  SHA-256 `fc050eedfdeb5be367994b39edd50c470a124f3715c99c26aa6df816b2019559`
- accepted host log:
  `hbox:/home/hbox/dclutch-ensemble-four-member-q3-551ffc1b-host4d-port43320-20260908/ensemble-resume-fold-reclaim-v3-20260908.log`,
  SHA-256 `583748d6d3e560f37d5061a75ab6f1daf017470c9e70cebaaf928f56e91fc7d1`
- founding evidence SHA-256:
  `a8ec5739d67ea8eb50d6351eea4626c04fe36c62cd2cbccbd953eed7f71e79c0`
- exact retained-account native replay SHA-256:
  `b212774155d6baff81c4b2e39a081877c5bfafca4e1b1a5e076654ab5ba07f9c`
- native-check log SHA-256:
  `28e69eb4cb537db577ff1f139d7eb6d7b609f395c9090aa28187a3e1af243391`
- release-build log SHA-256:
  `212f56c6c88f96675f2661864264af3d361d1709ea1bb3d95cc7f5e66c905460`

The focused exact-account replay
`retained_rpc43320_fold_replays_deadline_refusal_then_accepts_after_deadline`
passed on hbox with one test run, zero failures and 469 filtered tests. It
replays the original bank Clock refusal as `DeadlineNotReached` and accepts
the same three certificates at the authenticated deadline plus one.
