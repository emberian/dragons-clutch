# Existing GitHub Pages renovation — 2026-09-07

Owner: CODEX-INTEGRATION. Publication evidence, not protocol execution evidence.
Observed 2026-09-07 EDT / 2026-09-08 UTC.

The existing Pages workflow successfully deployed the renovated app and existing
guide hierarchy. GitHub's deployment status names `https://clutch.dregg.pro/`.

| Coordinate | Recorded value |
| --- | --- |
| Live source | `a8b4335ba280abb408d6645acb4ce2b1c388bd32` |
| Live tree / published `dclutch/` tree | `adc0f7d8ad615f5ac364b62f1fd396d318addbfb` |
| Publication commit | `a8b5b3429f8e1d5ac8fc6602577e1e299f0fd4f2` |
| Workflow | `emberian/dragons-clutch`, `.github/workflows/pages.yml`, `main` |
| Run | [34180531435](https://github.com/emberian/dragons-clutch/actions/runs/34180531435), success |
| Deployment | `6319517731`, success at `2026-09-08T02:35:31Z` |
| GitHub environment URL | `https://clutch.dregg.pro/` |

## What shipped

The existing homepage now leads to markets and the design workbench, explains
collateral and resolution, and places selected-deployment observations before
the retained key art. The existing console searches its capability catalogue
while retaining each action's prerequisites and limitations. The explorer
decodes nine generated Dealer layouts and names eight additional instruction
routes. It preserves exact integer coordinates beyond JavaScript Number's
precision. Market discovery and deployment evidence now agree with the selected
program set; cohort-17 addresses no longer carry cohort-16 provenance.

Reader, trader and operator documentation remains under `docs/guides/`, published
through its existing index. The app is mounted at the domain root and the
documentation at `/docs` by the existing renderer.

## Verification

`tools/cut.sh` found zero credential findings and checked exact tree equality
before pushing only the content cut to public `main`.

A clean detached worktree at the source above ran `npm ci`, then the same static
build environment as the workflow: `NEXT_PUBLIC_DCLUTCH_DOCS_BASE=/docs` and
`DCLUTCH_PAGES_EXPORT=1`. Both local and GitHub builds completed. Both renderer
logs report 423 files, 172 documentation pages rendered, 242 pages link-checked,
six indexed guides and 25 trailing-slash directory indexes.

Before the source commit, nine focused frontend test files passed 76 tests;
TypeScript and targeted ESLint passed. The ABI inventory matched its baseline.
The explorer survey reported 69 of 88 record magics and 57 of 70 instruction
magics rendered, with 19 and 13 explicit exemptions respectively and no
unaccounted magic. Those exemptions remain work, not evidence of decoding.
The static accessibility survey reported no new findings; its existing contrast
exemptions and 197 unresolved text/background combinations remain. These checks
do not establish visual or screen-reader correctness.

The publication facts were read through GitHub repository, workflow and
deployment metadata, with the deployed run's head SHA checked against the cut.
No public-site crawl supplied this evidence.

## Outstanding work

This publication carries development sources and the recorded cohort-17
configuration. It does not establish a fresh devnet deployment, active simulator,
or completion of the remaining local-validator family campaigns. Explorer
navigation cancellation and additional generated record mappings continue in
subsequent development commits.
