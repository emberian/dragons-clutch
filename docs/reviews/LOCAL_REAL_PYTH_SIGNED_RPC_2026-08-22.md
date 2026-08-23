# Local-real Pyth signed-RPC review — 2026-08-22

Status: **PASS**, scoped to **NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL
VALIDATOR ONLY / NO VALUE**.

The retained run was produced from clean tracked repository commit
`361eafd03c363cd0dc6dcd1d8ade40d4c7d7e79d`. Its public-safe transcripts are
under `docs/reviews/evidence/local-real-pyth-signed-rpc-2026-08-22/`.

The opt-in campaign executed the captured Pyth Wormhole router and receiver
programs from reconstructed exact complete Upgradeable Loader Program and
ProgramData accounts. A deterministic 13-of-19 local guardian quorum signed a
fresh synthetic nonzero-confidence observation. The router verified and
persisted the VAA in an earlier transaction. A later transaction placed the
real receiver `PostUpdate` immediately before Clutch
`AppendSourceArchiveV2`; those two instructions were atomic. The campaign then
sealed the one-record archive and categorically resolved cell 1.

The patched Agave validator and every observed child TCP listener and UDP
socket passed loopback-only probes both before and after the signed campaign.
The runner stopped the validator and removed its ledger and ephemeral keys.

## Result boundary

- Selected validator SHA-256:
  `190f7c847af303957c771a38cab97142ab2e3ab6e5047dc53af2c4d4942bb7c8`
- Test-only Clutch ELF SHA-256:
  `834562d8417bec2b62d7add17ed3a68839943bcc94c2eb3832442287e25f8e6f`
- Settled warped-Clock probe: `1787525618`
- Synthetic publish time: `1787525400`, exactly derived as
  `floor((probe_clock - 180) / 60) * 60`
- Final Clock: `1787525640`; observation age at final assertion: `240` seconds
- Price/confidence/exponent: `100000000 / 6357 / -8`
- Exact confidence interval: `[99980929, 100019071]`
- Archive: one record at the expected bucket, exact publish time/posted slot,
  uniquely selecting categorical cell 1
- Wrong Config rollback: instruction 2 refused with custom error `122`; the
  update remained absent and the archive and treasury remained byte-identical
- Wrong feed rollback: instruction 2 refused with custom error `122`; the
  update remained absent and the archive and treasury remained byte-identical

## Signed transaction sequence

| # | Step | Compute units | Result |
| ---: | --- | ---: | --- |
| 1 | Router initialize | 34,183 | accepted |
| 2 | Router initialize/write encoded VAA | 3,038 | accepted |
| 3 | Router write/verify encoded VAA | 336,652 | accepted |
| 4 | Receiver initialize | 7,931 | accepted |
| 5 | Correct SourceSpec initialize | 31,216 | accepted |
| 6 | Correct SourceArchive initialize | 30,493 | accepted |
| 7 | Wrong-feed SourceSpec initialize | 34,216 | accepted |
| 8 | Wrong-feed SourceArchive initialize | 33,493 | accepted |
| 9 | Wrong-Config PostUpdate + append | 70,673 | refused at append; atomic rollback |
| 10 | Wrong-feed PostUpdate + append | 74,729 | refused at append; atomic rollback |
| 11 | Real PostUpdate + adjacent Clutch append | 75,261 | accepted atomically |
| 12 | SealSourceArchiveV2 | 67,727 | accepted |
| 13 | Categorical resolve | 166,825 | accepted; payout cell 1 |

Each retained step also binds the returned RPC signature to the locally signed
wire bytes, records the signed-wire SHA-256 and exact program order, and checks
the confirmed status against `getTransaction`.

## Public evidence hashes

| Artifact | SHA-256 |
| --- | --- |
| `campaign.json` | `67d7b3079a76cf544e68845e9c95c8c975b2ea734b316acb273f01712880a5f8` |
| `result.json` | `65e00414c2138fb006706a83de2ae3e49c1fc16c0a5f2b54299366e057ad5ce9` |
| `probe-before.txt` | `9c3e6cc11f62b6029af702bebf812d13528ca9c98e4b05b98f30ec3f6cd3e6ea` |
| `probe-after.txt` | `9c3e6cc11f62b6029af702bebf812d13528ca9c98e4b05b98f30ec3f6cd3e6ea` |
| `probe-evidence.json` | `70a1d4d8ca63e2f114061c74a075b35d63ffc12761af981b72eb2820bcde3465` |

## What this does not establish

This is not devnet price evidence, provider availability evidence, a public-RPC
test, mainnet evidence, a production source release, or a checked deployment.
It does not establish economic quality, optimal clearing, formal verification,
production liveness, or frontend/operator readiness. The observation and
guardian keys are synthetic and local; the Clutch release identity is
unmistakably test-only. Provider loader bodies are reconstructed from captured
ELFs and transcribed headers rather than retained raw RPC account responses.
The loopback patch changes local validator network binding, and its outbound
client paths retain upstream's fixed loopback validator-client port range.
The campaign executes inside the existing 500-slot, 600-second, and 500-bps
laboratory admission envelope. It does not establish that those limits are
production-suitable, and it does not add signed-RPC negatives immediately over
the staleness or confidence boundaries.
