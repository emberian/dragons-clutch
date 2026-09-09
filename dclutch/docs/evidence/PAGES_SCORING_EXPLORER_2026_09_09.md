# Scoring explorer publication — 2026-09-09

The existing explorer renders Scoring Dealer redeem and close instruction
prefixes from their native generated layouts, including market, Dealer identity
and expected fund revision. It links Core retirement to its current checkpoint
handler. Focused field/route tests (21), TypeScript, ESLint and the complete
explorer coverage check passed on hbox.

- Integration source: `e2b5e3e84eb3869ae050f73ab9e044f649ae6057`.
- Publication cut: `8780a7d2120d84f958a6a387d856ba742884e5c4`.
- Exact published subtree: `e630f6152f95aef2a3fdb7a52dd8e5dbcb8f528f`;
  `tools/cut.sh` reported zero credential findings and exact tree equality.
- Existing Pages workflow [34303684535](https://github.com/emberian/dragons-clutch/actions/runs/34303684535)
  deployed that publication commit successfully at `2026-09-09T02:34:32Z`.

This deployment precedes the ArtifactReleaseV2 migration. The V2 source cut
requires a fresh compatible cohort before its browser replaces this deployment.
