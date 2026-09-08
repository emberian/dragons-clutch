# Existing Pages explorer follow-up — 2026-09-07

Owner: CODEX-INTEGRATION. Publication evidence, not protocol execution evidence.
Observed 2026-09-07 EDT / 2026-09-08 UTC.

The existing GitHub Pages workflow deployed the explorer follow-up to the same
environment as the [first renovation](PAGES_RENOVATION_2026_09_07.md).

| Coordinate | Recorded value |
| --- | --- |
| Live source | `002169790df6c16713d39249730989d05c1a6070` |
| Live tree / published `dclutch/` tree | `d113edbe3cf8acad8397993c05742da989297240` |
| Publication commit | `9ae823e01390e2a8087d2302fedb749fc3d2f83c` |
| Workflow | `emberian/dragons-clutch`, `.github/workflows/pages.yml`, `main` |
| Run | [34182010844](https://github.com/emberian/dragons-clutch/actions/runs/34182010844), success |
| Deployment | `6319768150`, success at `2026-09-08T03:01:40Z` |
| GitHub environment URL | `https://clutch.dregg.pro/` |

## Changes and checks

The existing explorer now follows URL navigation consistently, permits retrying
the same query, and discards reads made stale by a different query or selected
deployment. Record selection also carries the deployment identity so the old
record cannot reappear after leaving or changing deployments. Focused interaction
tests exercise these races, retries and back/forward navigation.

Generated record layouts now cover aggregate checkpoints, compact lifecycle Hot,
descriptor rows, replay and resolution certificates. The explorer preserves full
integer precision and rejects truncated layouts. Four additional instruction
descriptions name their native routes. Source review corrected the certificate
description: RecoveryAdvanced and Exhausted do not establish a terminal Product
certificate accepted by Core.

The inventory reports 74 of 88 record magics and 61 of 70 instruction magics
rendered, with 14 and nine explicit exemptions respectively, and zero unaccounted
magics. Three aggregate suffixes remain exempt because their generator does not
provide full field layouts. In particular, `DCLTARF1` also identifies a different,
longer ArtifactRelease layout; its magic alone cannot select a decoder.

The integration check passed 60 tests across six focused frontend files, plus
TypeScript and targeted ESLint. A clean detached build at `7477dfb30` checked the
app, docs and renderer inputs, which remained byte-identical through the source
commit above. GitHub then rebuilt the exact publication commit. Both renderer
logs report 427 files, 174 documentation pages, 244 pages link-checked, six
indexed guides and 25 trailing-slash directory indexes.

`tools/cut.sh` checked zero credential findings and identical source/publication
tree objects before pushing the cut. GitHub repository/workflow/deployment
metadata establishes the deployed head and successful environment publication.
The workflow still mounts the existing app at the domain root and the existing
guide hierarchy at `/docs`. No public-site crawl supplied this evidence; these
checks do not establish visual or screen-reader correctness.

## Limits

This updates the existing Pages application and its documentation. It does not
establish a new devnet cohort, an active public simulator, complete decoding, or
accepted execution of every protocol family. Those remain separate deliverables.
