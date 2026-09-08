# Infrastructure floor at 88aec17e8 — 2026-09-07

Owner: CODEX-INTEGRATION. Actual local-validator execution, not devnet evidence.

The transaction campaign completed at exact host/source commit
`88aec17e86b5201ac94c8144b4332d4dcd20a534`: 209 transactions through atomic
Market founding and active Core/Resolution funding. The overall runner exited
1 because eight compute-budget rows were over their recorded budgets. All
transactions fit the chain compute limit. Of 24 witnesses, 23 passed and the
aggregate compute-budget witness failed. No budget was changed.

Clean source: `hbox:/tank/dregg-build/dclutch-floor-source-88aec17e8`.
Work: `hbox:/tank/dregg-build/dclutch-floor-88aec17e8-20260907`.
Run: `runs/20260908T012031Z-88aec17e86b5/` beneath that work directory.
The runner used `--mode full --keep-runs --rpc-port auto`, selecting port 23880,
and retained its declared deterministic loopback key derivation. This floor
loads seven infrastructure programs; it does not exercise the eighth,
separately deployed accelerator or claim a complete cohort campaign.

The native check passed before SBF compilation. Every one of the seven role
build logs contains a compilation marker. Neither frame-diagnostic shape nor
the containment-wrapper no-op marker occurs. The successor host was built by
package at the root workspace. These checks establish what ran; they do not
erase the budget failures.

## Runtime artifacts

All paths below are `elf/<role>.so` beneath the work directory.

| Role | Bytes | SHA-256 |
| --- | ---: | --- |
| Claims | 1458264 | `7656cade51da56014f0be94988330a50ba9ec75cdf8b1fb176c5d5881c48afe3` |
| Core | 1205648 | `a08769a886cf03a061376dc5741c4141c545554830a3c9b79423477b94115c97` |
| Custody | 468768 | `b936b48790ac89ff65d36b28562674fc95f4ba4e6e0f8a763122a6c4baed6422` |
| Registry | 237696 | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| Rent | 143152 | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| Resolution | 932840 | `87661a9764839945cce082feccb5095c8b7c6412fc7f65cd0114be222d84698a` |
| Trading | 2395560 | `3786b6437e66d456623a547ad6f6a26e0dafa524ca657c97cf96740bc12fb3ae` |

## Located budget failures

| Budget row | Observed CU | Budget CU | Excess CU |
| --- | ---: | ---: | ---: |
| Atomic founding | 908245 | 899068 | 9177 |
| Founding stage 4, Claims FoundingV5 | 204728 | 197589 | 7139 |
| Projected Custody Initialize | 285874 | 283115 | 2759 |
| Found37 substituted-Market refusal | 179801 | 179293 | 508 |
| Claims release activation | 763693 | 749162 | 14531 |
| Trading release activation | 1229668 | 1220411 | 9257 |
| Resolution release activation | 498643 | 466563 | 32080 |
| Core funding creation | 337686 | 333613 | 4073 |

The previous dated floor had thirteen red comparisons. This run has eight;
it is still red. Artifact hashing costs and slot-dependent PDA bump searches
are known potential causes, not a diagnosis of these particular deltas.
`tools/gauntlet/CU_BUDGETS.md` requires measurements and a stated noise band
before changing a budget. Final-source comparisons remain owed.

## Evidence and coverage boundary

| Artifact | SHA-256 |
| --- | --- |
| Run `evidence.json` | `ae0f2e1dc2acdd17de9192f474c4019d3dbdf78eda08d526334b1056ceb8f715` |
| Run `plan.json` | `e2e6837ae831ecf78db79669f9ad3a56e60c00149d1bb06ca91de3393fdc110f` |
| Run `campaign.stderr` | `7a44e21bb0e2ed73bb65ce47a977332e357ccaabe11c1a9a0920a890db2548e8` |
| Work `out/ledger.json` | `32e0ff0f2198acca45c2b89345f961866dc3d2bf7a086542968669b45075de93` |
| Work `out/inventory.json` | `16491dad146a3ce278cce7781a0373c0d615df6cf7de297a091c7cf3b69f33ba` |

This floor's census records 36 executed routes, one refused-only route and
127 routes with no execution in this ledger, from 164 enumerated routes.
Seven of 456 enumerated refusal codes are observed. These are this floor's
coverage results; they neither incorporate the other family campaigns nor
establish that every unobserved route has never run anywhere.
