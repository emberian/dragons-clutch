# Existing Pages correctness checkpoint — 2026-09-08

The existing site published the corrected reader/operator guides and the
development plan's distinction between the General transport repair and the
Series recurrence repair. The configured devnet cohort remains cohort 17.

| Fact | Checked value |
| --- | --- |
| Integration source | `5024c54844af5efa6a9d2765ddce474db10b6a4d` |
| Source / published `dclutch/` tree | `54faca5fbcd0ee38f3eadd1ce71e4d79eb011e8b` |
| Publication cut | `e60d62d940b27d7930056d58bc5cdc98c87c6222` |
| Workflow | [34239568134](https://github.com/emberian/dragons-clutch/actions/runs/34239568134), success |
| Deployment | `6330091439`, success at `2026-09-08T14:39:34Z` |
| Existing environment | `https://clutch.dregg.pro/` |

Publication used `tools/cut.sh` from a clean detached checkout of the named
integration commit, verified as an ancestor of `main`. The explicit detached
checkout override pinned the reviewed snapshot while other lanes continued
committing. The credential sweep found zero findings and integration
independently checked that public `origin/main:dclutch` matched the source tree.
Native bootstrap and Journey checks passed at `c04d95a0c898b125e67ef0cbbf77a98c47aa4e36`;
the published revision differs from that checked revision only in the active
development plan.

The existing workflow built 42 app routes and assembled 439 files, including
186 rendered documentation pages and 256 link-checked pages. It retained the
six indexed guides, 25 directory indexes, app at the domain root and docs under
`/docs`. Evidence is repository metadata, workflow output and deployment
status. This publication does not establish a fresh devnet deployment,
complete protocol execution, browser visual validation or a running aquarium.
