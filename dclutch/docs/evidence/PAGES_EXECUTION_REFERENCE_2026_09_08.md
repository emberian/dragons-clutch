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

## Addendum — selector and runtime checkpoint, 2026-09-08 17:29 UTC

Publication cut `813caaf1566efc4bff0450916ac071884d856ebf` carries source
`f173bb8b9ae6230342ee48232899d584e077b88d`, including source-derived Claims
selectors, authenticated Series runtime wiring, and sealed artifact validation
reuse. Source and published tree are
`44ff73ca7d8f4eae936cb38939ee5281b5254d20`. The cut found zero credential
findings and checked exact tree equality. The committed reference check
converged without changes.

[Workflow 34257345585](https://github.com/emberian/dragons-clutch/actions/runs/34257345585)
and deployment `6333274546` succeeded; the latter records
`2026-09-08T17:29:47Z` and the existing `https://clutch.dregg.pro/` environment.
The workflow rendered 42 app routes and assembled 449 files: 196 documentation
pages, 266 link-checked pages, six guides and 25 directory indexes. Logs:
`/private/tmp/dclutch-cut-f173bb8b9-20260908.log`,
`/private/tmp/dclutch-reference-check-f173bb8b9-20260908.log`, and
`/private/tmp/dclutch-pages-34257345585.log`. The runtime and validation limits
above still apply.

## Addendum — checkpoint retirement and CPI repair, 2026-09-08 17:45 UTC

Cut `60b9372c0dd46ac843bfd57889c2a97c433a87e6` publishes source
`6c7a9885888d03565f64dea0d8427fae11fd4baf`, tree
`1d6dc6c48d14f9fe27837b75a3dc7fd5fab94f0e`. It carries checkpoint retirement
convergence, borrowed-source CPI preparation, and the WholeUnwrap evidence
reference. The credential sweep found zero findings, the published tree
matched, and the committed reference check reached its fixed point unchanged.

[Workflow 34258862103](https://github.com/emberian/dragons-clutch/actions/runs/34258862103)
and deployment `6333552804` succeeded at `2026-09-08T17:45:02Z` in the existing
Pages environment. The workflow rendered 42 app routes and assembled 450 files:
197 documentation pages, 267 link-checked pages, six guides and 25 directory
indexes. Logs: `/private/tmp/dclutch-cut-6c7a98858-20260908.log`,
`/private/tmp/dclutch-reference-check-6c7a98858-20260908.log`, and
`/private/tmp/dclutch-pages-34258862103.log`. Historical execution and current
implementation checkpoints still do not establish the fresh cohort or a
completed current-source lifecycle.

## Addendum — native founding and route payer checkpoint, 2026-09-08 18:18 UTC

Cut `23fd1168193a66a1ec91b7e71d86144c9cf7b9e8` publishes source
`fe02f0813036efa7c2345250dac5b6701c24fee7`, tree
`582594bef25c79d744c0f0559fe6cdff389a2df8`. It carries native Series founding
convergence, checkpoint retirement repairs, and the Direct route payer flow.
The credential sweep found zero findings, the published tree matched exactly,
and the committed reference check converged unchanged on its first pass.

[Workflow 34262138884](https://github.com/emberian/dragons-clutch/actions/runs/34262138884)
and deployment `6334144311` succeeded at `2026-09-08T18:18:14Z` in the existing
Pages environment. The workflow rendered 42 app routes and assembled 450 files:
197 documentation pages, 267 link-checked pages, six guides and 25 directory
indexes. Logs: `/private/tmp/dclutch-cut-fe02f0813-20260908.log`,
`/private/tmp/dclutch-reference-check-fe02f0813-20260908.log`, and
`/private/tmp/dclutch-pages-34262138884.log`. This remains a source and site
checkpoint; it does not establish a fresh devnet cohort or completed
current-source execution.

## Addendum — Bearer console delivery, 2026-09-08 22:22 UTC

Cut `e1bcbb5e4e1b861195b60b6a74625b4bf5744fbb` publishes source
`73a2503d65ee75608f2ade4ded3b9e6ec1d0f9cb`, tree
`2fe5bf0a037a0373ace2466325915e7ade37fa8b`. It adds the existing representation
workspace to the Console at `/representation`, independent transfer-authority
and payer signatures, durable operation recovery, exact finalized Token-2022
balance checks, and the trader guide's wallet instructions. It also carries
the durable Pyth recovery producer and checkpoint packet extent repairs.
The credential sweep found zero findings, published and source trees matched,
and the committed reference check converged unchanged.

[Workflow 34285462083](https://github.com/emberian/dragons-clutch/actions/runs/34285462083)
and deployment `6338327616` succeeded at `2026-09-08T22:22:08Z`, still in the
existing Pages environment. The workflow rendered 43 app routes and assembled
462 files: 197 documentation pages, 269 link-checked pages, six guides and 26
directory indexes. Logs: `/private/tmp/dclutch-cut-73a2503d6-20260908.log`,
`/private/tmp/dclutch-reference-check-73a2503d6-20260908.log`, and
`/private/tmp/dclutch-pages-34285462083.log`. The new console's build and
targeted tests are source evidence; a current-cohort wallet execution and
browser visual review remain separate outstanding checks.


## Addendum — claim-check and retirement reference, 2026-09-08 22:41 UTC

Cut `7951a6bdc3762189433fb92b1f75436339af6ef8` publishes source
`dd756b79e7d3f82194c2852e1c74955e42378a0e`, tree
`6f6d7de2b719fc4c81947ab95bf0dbfa7b4c2499`. The credential sweep found zero
findings and the source and published trees matched exactly. The committed
reference check converged unchanged.

[Workflow 34287110785](https://github.com/emberian/dragons-clutch/actions/runs/34287110785)
and existing Pages deployment `6338614401` succeeded at
`2026-09-08T22:41:51Z`. Logs are
`/private/tmp/dclutch-cut-dd756b79e-20260908.log` and
`/private/tmp/dclutch-reference-check-dd756b79e-20260908.log`.
This publishes the implementation checkpoint; the fresh devnet cohort and
complete current-source lifecycles remain outstanding.
