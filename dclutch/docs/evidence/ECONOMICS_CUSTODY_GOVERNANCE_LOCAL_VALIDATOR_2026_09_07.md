# Economics Custody governance local-validator campaign — 2026-09-07

Owner: CODEX-AQUARIUM. Evidence level: bounded local-validator execution.
This is a separate immutable record for the completed Custody `Propose`,
`Apply`, and `Withdraw` extension. It does not amend or replace
[`ECONOMICS_CUSTODY_LOCAL_VALIDATOR_2026_09_07.md`](ECONOMICS_CUSTODY_LOCAL_VALIDATOR_2026_09_07.md),
which records the earlier Found and Upkeep campaign. It is neither devnet
execution evidence nor a release claim.

## Checked identities and durable artifacts

The clean diagnostic host was commit
`c3d61aa978db9a1dd71cf6d40e37fdd05c187404`. The checked runtime source was
`a628bfd39dc971b5e37f20d9482290da3f9f6160`. The runtime was authenticated by
`/tank/dregg-build/dclutch-cohort18-dealer-strict-a628bfd39-20260907/CHECKED_UPGRADE_GATE.json`,
SHA-256
`7791a357bdaa390d86a82e3a8483bc598556b6f90b5988822e7221fe79b438ee`.
`source.json` classifies this combination as
`diagnostic-host-current-runtime-checked`: the host produced a local test of
the named checked runtime; it does not claim that host source built that
runtime's ELF set.

The campaign ran at
`hbox:/tank/dregg-build/dclutch-economics-c3d61aa97-20260907/economics-run`.
After completion, the three artifacts were copied read-only from that hbox root
into the durable local evidence intake at
`/private/tmp/dclutch-economics-evidence-c3d61aa97-20260907-copy/`; their
SHA-256 values are:

| Artifact | SHA-256 |
| --- | --- |
| `source.json` | `f08b6c22daa96f3e18643e13ccaa8675badab8517f5eab5bdf9c68df6b748f97` |
| `transcript.json` | `de157693be075ebfc29f6a01e59a9010d50b9d55a44c49d855cf56595d97e734` |
| `campaign/evidence.json` | `89ee71853eff375a9f91899543a2ac64213531e407634326142d84f50b3fe93a` |

`transcript.json` declared `completed: true` with no wall. The evidence is a
fresh local validator: all slots and signatures below are local evidence, not
public devnet references.

## Transaction ledger

Every recorded transaction is listed. A `null` fee and compute-unit value is a
wrapper airdrop whose metadata was unavailable; it is not silently treated as
an on-chain program poststate. Refusals finalized with the named custom code,
and the campaign reread the protected account byte-for-byte after each one.

| # | Label | Signature | Slot | Result / exact protected poststate |
| --- | --- | --- | --- | --- |
| 1 | Fund hostile governance signer | `4f8zAGkGyqw23aJ5ox6zHu8tSM4NFSqaq7BPxCiL5QF75RBwdeowUWWK4fWmHSZ4kJPL6pe47ELFqL69mewcDxFH` | 1339 | Wrapper funding; metadata unavailable. |
| 2 | Parameters Found, hostile authority | `SBqA9yTn65Qunxi8yddDTLesGZJyrEm23qQYWW7b7zXKjKpDNG91SgZ4dMZ2LD3hn3gRin5KSWorE66XZME37CE` | 1378 | Refused `ProtocolParametersSbfErrorV1::FoundingAuthority`, `Custom(24836)` / `0x6104`; no parameters record created. |
| 3 | Parameters Found | `35oKjxLDgCn2HEgkepN3weLmRsxYdZfpsABU1mgCYgY1dWRqyy4pCVCpq8qqER7zoi41zqUHE5UZ95qPACntBz9M` | 1418 | Accepted; record `Ffw8EZCEhFkFagh7kb8RY1cg2uEyy6b5GjyUGAjnL3Ah`, owner and governance authority Custody-bound; pending false, take 0, hoard moved 0. |
| 4 | Parameters Found duplicate | `3qEqccZUFgR3YxDur31h8hAFeERnXCtEsXcBaCEZyNu8du5RYRx2XtjfvU3XkwqVj5uzUF6w4wQ6fGZVhbehjX5s` | 1452 | Refused `ProtocolParametersSbfErrorV1::Record`, `Custom(24834)` / `0x6102`; record unchanged. |
| 5 | Parameters Propose, hostile authority | `4kLjaDcoXo1GZECfoyDYRZAcuKv974yTCNALpcMBGE75onLveWHBrvHVTB74TmVJGVhTRQvY74Gx15xZiwS8tH4K` | 1489 | Refused `ProtocolParametersSbfErrorV1::UnauthorizedGovernance`, `Custom(24838)` / `0x6106`; record unchanged. |
| 6 | Parameters Propose | `4UuxDDoWqzFBfBFqoiwhsiedNqcXahPegNueNGyv9Z2Y7AM2cyTWQ97uDMZV7hjeHYrLJVDXZHXEJx4q66FmPrZt` | 1527 | Accepted; active generation 0 unchanged, standing pending body cap 1, earliest apply slot 1513527, hoard moved 0. |
| 7 | Parameters Apply before maturity | `4P5ZpvRuQjzt7KvyR97cXnPt85aDVyjw6B7z2KG8f8tmC2XnuUWejkAaAwfjrUSz59nHsZ15kmv9uhwPFGMeHT2R` | 1561 | Refused `ProtocolParametersSbfErrorV1::ProposalNotMatured`, `Custom(24842)` / `0x610A`; pending proposal unchanged. |
| 8 | Parameters Apply | `NaQ9azGBZQF2vEftWSSPKR7BjpzdrHw5527HXS2FfmeY1r48Nikhwyz26Jp89Bzg7CRf77LQCGTP4u5D8aW17gg` | 1513534 | Accepted; receipt `7RoXDSe3epYP4iqofKY4h6qY56yPSxvxqv6WiWdfabPY`; generation 1, cap 1, activation slot 1513534, pending false, hoard moved 0. |
| 9 | Parameters Withdraw after Apply | `2HUyNYdf1gUd3hyK8RStuuGgrLn8hbjuuH3FxU1HNA67NqF2BiBdwRV2h9NxT88af1jesCTsc3XdQ3VuvdtJX24R` | 1513566 | Refused `ProtocolParametersSbfErrorV1::NoPendingProposal`, `Custom(24841)` / `0x6109`; active generation 1 record unchanged. |
| 10 | Parameters Propose for Withdraw | `2N6MEBUHmetyWRb1XPMGLNSHjUq1cdsHQcPrZXLTwvth7HG1p81Uf6Pgp8TuibmhLErqJ4USubKXcrnDs8R59gR` | 1513598 | Accepted; active generation 1 retained, standing cap 2, earliest apply slot 3025598, hoard moved 0. |
| 11 | Parameters Withdraw | `sepeo26ZJN64tfdBjMxnqnYsRsgwrvPDV1fViYi7cv9avSoQrx2Kj1P7t7bGhujYkoxU48rwXHV9Gg9NfE2ZhQA` | 1513630 | Accepted; generation 1 retained, pending false, hoard moved 0. |
| 12 | Fund voluntary upkeep payer | `2iXgxPwS1LHovHd8PPi9vDJeUP4eKesiE6tEBHgu3iHU41ucWfoTYHx1Qj5HZ7XnphhVkt48Cr1JM1oqdaHZEj91` | 1513662 | Wrapper funding; metadata unavailable. |
| 13 | Upkeep Found | `4PcAX7dsEcCaRVpFED7MK2DGaERCzHGFedChtY7yXdUrpwfdTP6q2298vRWW6pfaFEgJYheAgiTNZRY3PhCDhXqD` | 1513694 | Accepted; vault `7QK8qFHzmis6kQEMqZ9qCYooryegbZcvoyEaavgjSopA`, founding rent 1,781,760 lamports. |
| 14 | Upkeep Deposit Credit | `ZaZoqSrGHn3C73hSiiAAD4duvypHkL85iv5bL1wJeLtmaQZNR6EpM74xqGrc8GZUSJC2atfvi6gcPSHA2Q7Gfg6` | 1513726 | Accepted; 17,001 deposit credit, count 1, deposit/inflow 17,001, donation 0, unreceipted 0, no remainder, vault 1,798,761, hoard moved 0. |
| 15 | Upkeep Found duplicate | `3Gwo6AAjyJcueMTbLK7nziRDA8UHNKYpNcx1SGs9PJaiQpBLwg2aT49V25oPmxS8GKCFLZcwmpN8UZ3WaVkRnGqV` | 1513758 | Refused `UpkeepVaultSbfErrorV1::Vault`, `Custom(25090)` / `0x6202`; vault unchanged. |

The accepted Proposal is preserved before clock movement: the campaign waited
until the failed early-Apply transaction at slot 1561 was finalized, verified
the pending body again, then restarted the same validator ledger at the
proposal's earliest apply slot 1513527. The later accepted Apply reread that
same record and authenticated its change receipt against the prior body,
notice delay, and activated body. This proves durable same-ledger local
validator restart for this campaign.

The warp is controlled local-validator clock manipulation. It is not elapsed
devnet time, and the 90-second finalization wait is a provisional operational
cap with the source's stated lifting plan: measure a slower checked-mutable
validator root before changing it. No conclusion here asserts devnet maturity,
public availability, or a release deployment.
