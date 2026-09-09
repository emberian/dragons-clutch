# Series Prepare caller alias convergence — 2026-09-09

The complete generated Prepare account profile now keeps all five native child
caller PDAs distinct. The change removes four incorrect physical aliases;
logical frame width remains 116 and the decoded physical count changes from
60 to 64. The physical count is derived from the canonical alias table, not
an account-capacity allowance.

## Actual wall and semantic owner

The retained V1 diagnostic
`hbox:/tank/dregg-build/s691-series/shadow-diagnostic-79d.log` completed Consume
and Expire width observation, then refused Prepare hydration because projected
Initialize and Open supplied different canonical caller PDAs where the release
profile required one physical account. Full capture provenance is in
[the retained recovery evidence](RETAINED_STRUCTURED_SERIES_HOST_RECOVERY_2026_09_09.md).

`ProjectedCustodyCallerSeedsV1` owns the projected caller and includes the exact
child request digest. Normal Custody callers use Registry's
`CallerAuthoritySeedsV1`, likewise bound to the exact request digest. The
production Prepare child bank contains five different requests. Their callers
at coordinates 6, 53, 68, 81, and 97 cannot be coalesced.

The native `prepare_funding_artifacts_v5.rs` table incorrectly aliased 53, 68,
81, and 97 to 6. It now exports `SERIES_PREPARE_ROUTE_ALIASES_V5`; the host
consumes that table for physical-address and width checks. Both host literal
table copies and the separate test alias mask are removed. A constant assertion
requires every declared child route caller to remain a representative. Every
other alias and every child-route window remains as declared by its owner.

## Executed controls

The V2 checkout is
`hbox:/tank/dregg-build/dclutch-claims-sparse-a8e3b4e8c-v2-20260909/source`, with
its paired target in the same parent directory. Base source is
`a8e3b4e8c593a5a63eef94a51375f46ba5ad75d2`; checked green descendant is
`9d7991f70e7de8ad58edc4cb533bc90a277cede6`.
The unrelated existing Claims affine and sparse-chain test patches were
preserved. No sealed all-eight source, retained validator, or deployment was
modified. All commands ran through `swarm-build` and printed root/revision.

The full-profile test derives all five caller PDAs from the production
`SeriesChildBankV1` requests using their native seed owners. Other account
observations use neutral fixture bytes to isolate the complete native profile's
alias, uniqueness, width, and privilege checks. The public VM `project_atomic`
executes the actual emitted ProfileV3's complete base profile.

| Control | Result |
| --- | --- |
| Old aliases plus five canonical native callers | Exact `account_profile::v2::Error::AliasMismatch`; positive assertion red. |
| Corrected full native profile plus the same callers | `Ok(())`; printed logical 116, physical 64, callers 5. |
| Each of the four later callers replaced with the first caller key | Exact `account_profile::v2::Error::CrossItemAlias` for all four mutations. |
| Native Prepare funding/current frame constructor | One passed, 460 filtered. |
| Host geometry preserves every native representative | One passed, 969 filtered. |
| `cargo check --locked -p dclutch-trading-sbf -p dclutch-local-successor-bootstrap` | Passed. |

The full-profile control passed one test with 969 filtered; tests ran with one
thread. No unfiltered package suite ran.

Logs in the checkout's parent directory:

| File | SHA-256 |
| --- | --- |
| `series-alias-red.log` | `6444c4c9effcc022108f6c7aa2ef8e3a7d5a9b30cda33b433124855baae63d35` |
| `series-alias-green.log` | `a21202dbbaf73d8f1ca06304830ee7cdef6553935e480c7f6ec25cd728bca0d0` |
| `series-alias-native-control.log` | `aae24c75ae3d2e73108f2a3016c38cdcb08f24547679b1381a30b9a073b89de2` |

## Integration boundary

The non-Rust active-source sweep found no independent Prepare alias table in
apps, packages, formal sources, or tools. The native artifact emitter owns the
profile; generated reference convergence remains with the integration owner.
This native source change leaves the frame ratchet red pending the next exact
all-eight build and capture.

A fresh committed runtime and V2 geometry canary remain required. The prior
V1 diagnostic widths are bootstrap observations only. These controls establish
the profile correction, not an accepted Prepare or a completed recurrence.
