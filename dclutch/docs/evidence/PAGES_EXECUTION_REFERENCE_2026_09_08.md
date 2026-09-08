# Existing Pages execution reference — 2026-09-08

The existing Pages app and documentation now publish the corrected execution
reference, accepted Ensemble transactions, and Series native recurrence proof.
The reference distinguishes checked transactions from authored coverage claims;
historical execution does not establish acceptance on the current source.

| Fact | Checked value |
| --- | --- |
| Integration source | `ef17492898ae2e4f171ae3558c681a4fa2c48ec3` |
| Source / published `dclutch/` tree | `82b41826bf1d2682858f2f1e45f8ecdbdf78e543` |
| Publication cut | `0f6900207d5b0a51d7deb031f05f683f791d2cfa` |
| Workflow | [34252823178](https://github.com/emberian/dragons-clutch/actions/runs/34252823178), success |
| Deployment | `6332484306`, success at `2026-09-08T16:44:40Z` |
| Existing environment | `https://clutch.dregg.pro/` |

`tools/cut.sh` published committed HEAD, found zero credential findings, and
required exact tree equality. Integration checked public `origin/main` after
the push. The existing workflow rendered 42 app routes and assembled 447 files:
194 documentation pages, 264 link-checked pages, six guides and 25 directory
indexes. Its output is retained at
`/private/tmp/dclutch-pages-34252823178.log`; the cut log is
`/private/tmp/dclutch-cut-ef1749289-20260908.log`.

The reference converged from a clean detached checkout in three passes. After
committing the generated outputs, `tools/gate reference --check --converge`
passed without changes at the named source. Eight classifier controls and both
SDK route-census and phase-admission generator checks passed. The final check
log is `/private/tmp/dclutch-reference-check-ef1749289-20260908.log`.

Cohort 17 remains configured. This publication does not establish a new devnet
deployment, a running aquarium, final-source transaction acceptance or browser
visual validation.

## Addendum — fractional execution cut, 2026-09-08 17:02 UTC

The next regular cut publishes the accepted fractional validator journey and
its checked parent-route census row, Series physical-input repairs, and the
native Ensemble capital calculation. The latter is an implementation
checkpoint; physical funding integration remains open.

| Fact | Checked value |
| --- | --- |
| Integration source | `68c4b7cb8b2c025c289ffba9992343ceee0f82da` |
| Source / published `dclutch/` tree | `14e566493d6c0cd0abb1b82b3b775def545fb63f` |
| Publication cut | `7aa19480070ad74260a73f73a9ca37cbef36d8d4` |
| Workflow | [34254594473](https://github.com/emberian/dragons-clutch/actions/runs/34254594473), success |
| Deployment | `6332789265`, success at `2026-09-08T17:02:40Z` |

The cut used a clean detached checkpoint verified as an ancestor of `main`,
with the explicit checkpoint override while lanes continued in the live tree.
The credential sweep found zero findings and the published tree matched
exactly. `tools/gate reference --check --converge` passed at the committed
checkpoint with no changes on its first pass. The workflow rendered 42 app
routes and assembled 449 files: 196 documentation pages, 266 link-checked pages,
six guides and 25 directory indexes. Logs are retained at
`/private/tmp/dclutch-cut-68c4b7cb8-20260908.log`,
`/private/tmp/dclutch-reference-check-68c4b7cb8-20260908.log`, and
`/private/tmp/dclutch-pages-34254594473.log`. The runtime and validation limits
stated above remain in force.
