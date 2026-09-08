# Existing Pages Dealer and General checkpoint — 2026-09-08

The existing Pages site now includes the accepted Dealer local-validator
campaign, General output-page integration, Ensemble deadline replay, and the
Cargo build-isolation rule. This is development publication; cohort 17 remains
the configured devnet deployment.

| Fact | Checked value |
| --- | --- |
| Integration source | `097a14049fc27ba9b89ea72fcb4dd21dc902061e` |
| Source / published `dclutch/` tree | `5461825fca5c2343ea958d86893a0642a76df2a3` |
| Publication cut | `03f261373d6097bed908ab50163ef4d3ee307c67` |
| Workflow | [34245185191](https://github.com/emberian/dragons-clutch/actions/runs/34245185191), success |
| Deployment | `6331131316`, success at `2026-09-08T15:31:31Z` |
| Existing environment | `https://clutch.dregg.pro/` |

The cut ran through `tools/cut.sh` in a clean detached checkout of the named
integration revision, verified as an ancestor of `main`. It used the explicit
checkpoint override while lanes continued in the live tree. The credential
sweep found zero findings. Integration checked public `origin/main:dclutch`
against the source tree after the push.

The existing workflow assembled 442 files: 189 rendered documentation pages,
259 link-checked pages, six indexed guides and 25 directory indexes. The app
remains at the domain root and documentation under `/docs`. Workflow output is
retained at `/private/tmp/dclutch-pages-34245185191.log`; cut output is at
`/private/tmp/dclutch-cut-097a14049-20260908.log`.

## Committed-source host control

General's session geometry test ran against this exact source on hbox through
`swarm-build`, with its own target in that source checkout. The test
`general_session::tests::output_page_session_geometry_has_one_caller_at_the_largest_action_width`
passed: one test, zero failures, 422 filtered. It verifies one OutputPage caller
at the selected large action width and the exact typed refusal for retired
General transports. This is a native host control, not a transaction success.

Integration independently compared all 3,230 exported source entries with
`git archive` of the named revision: zero mismatches. The check includes every
exported file's width and SHA-256 and every symlink's target. Source inventory
SHA-256: `e8906a9adbea3d72706db21e655356d95d2bd123d6f14862f62667021fd1738f`.
The remote verification is
`/tank/dregg-build/dclutch-general-9e87f291-20260908/root-source-097-verification.json`.
The test log is the sibling `logs/host-output-page-geometry-097.log`, SHA-256
`01b1ae17270160588610d3c3cd96f231e4c92d035ac461650a8870185deb99ca`.
It contains the actual executed test and no scheduler no-op marker.

This publication does not establish General nonempty transaction acceptance,
selected-Series execution, final-source frame convergence, a new devnet cohort,
browser visual validation, or a running aquarium.
