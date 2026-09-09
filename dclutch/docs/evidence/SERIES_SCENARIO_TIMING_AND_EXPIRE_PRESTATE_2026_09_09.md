# Series scenario timing and Expire prestate — 2026-09-09

This is host construction and fixture evidence. It is not a completed Series
recurrence, fresh V2 validator measurement, or release-width authorization.

## Operational schedule input

The local Series entrance now requires three explicit decimal slot inputs:
`--schedule-lead-slots`, `--schedule-retry-slots`, and
`--schedule-period-slots`. There is no implicit fixture schedule. These are
operator liveness allowances, not protocol bounds or capitalization facts.
The authoring observation occurs after reusable M0 immutable publication;
only then does the canonical Template receive its first slot. Both occurrence
retry ends are checked for overflow before publication. The command logs the
inputs and authoring slot before schedule-bound publication and includes them,
the first/second starts and retry ends, and the pre-Prepare finalized slot in
successful execution output. Prepare can run before the occurrence start; the
driver adds no wait or clock warp.

The measured profile `retained-691-post-template-publication-20260909` is
504–505 slots from Template authoring to completed diagnostic publication
(7a8: 98498 to 99003; 072: 102264 to 102768). This is a provisional operational
measurement of a retained V1 cohort, not a protocol minimum. It excludes later
parent publication, activation, sealing and routing. The previous +8 lead and
16 retry allowance expired during publication. A full V2 campaign must choose
explicit lead/retry headroom for its measured remaining publication and choose
period spacing for its intended Consume/Expire recurrence. The fixture's
600/256/320 timing inputs test exact propagation and overflow only; they are
not certified full-campaign defaults. Lift the provisional profile by recording
the actual V2 authoring/publication/Prepare slots, not by increasing a universal
constant.

## Expire account source

Expire uses the same explicit Prepare-prediction versus observed-Prepare phase
as Consume. Predictions require the canonical account to be absent. Observed
Prepare requires all five resources: Ticket state, normal Custody replay,
SeriesEscrow vault, projected Custody state, and projected Hoard vault. Native
decoders authenticate exact expected Ticket/replay/projected poststates from
the Prepare projection and token mint, authority, amount and option/state
fields. Program ownership and the non-executable bit are checked separately.
Missing finalized resources never fall back to predicted widths. Parent-root
prediction/finality remains independent of this resource phase.

The fixture control runs the full canonical Expire role constructor, preserves
alias keys, checks all five distinct resources, and refuses an existing account
in prediction mode, an absent account in observed mode, wrong owner, corrupted
native state and a valid token account with a different amount. These are host
fixtures; no account is seeded in a validator.

## Fresh V2 Prepare authority

The driver compiles the routed Prepare transaction including fee payer,
ComputeBudget instructions and lookup-loaded keys. It prints actual key count,
serialized packet length and SHA-256, simulates that exact signed packet,
then submits those same bytes only after successful simulation. Native profile
width is not substituted for transaction account count. Output retains packet
geometry, simulation (including CU), and finalized transaction CU/poststates.
The canonical message operator enforces the packet ceiling; the validator
simulation is the authority for its account-lock and CU limits. If the packet
expires before confirmation the exact-packet RPC path refuses rather than
claiming submission success.

The next inclusive all-eight V2 cohort must rederive the selected Series source
manifest and check actual Prepare account count/packet/CU. The retained V1
DCLTSSM2 diagnostic remains unpublishable and must not supply release widths.

## Validation

Heavy validation used the idle Claims V2 checkout and its paired target under
`/tank/dregg-build/dclutch-claims-sparse-a8e3b4e8c-v2-20260909/`.
The checkout base was `9d7991f70e7de8ad58edc4cb533bc90a277cede6`; only these two
host files were changed, preserving Claims' two unrelated test patches. No
sealed all-eight source, retained validator, or local Mac target was changed.

`cargo check --locked -p dclutch-local-successor-bootstrap` passed. Each test
ran alone with `--test-threads=1`: the explicit timing control passed (1 test,
970 filtered), and the complete Expire role constructor control passed (1 test,
970 filtered). Reinstating the old prediction-only behavior for the observed
phase made the latter red: zero authenticated prepared resources rather than
five. Restoring the final implementation returned it green. An earlier control
also refused the unnormalized in-memory projected state; the final expected
state uses native encode/decode normalization, preserving exactly persisted
facts rather than comparing transient request fields.

| Artifact | SHA-256 |
|---|---|
| `series_found_prepare_driver.rs` tested source | `deaaea910abbc43166eb80d203473de4a3e3693dda789f2d7faef815c76bd085` |
| `series_expire_geometry.rs` tested source | `a75b7611b54cf653624904a5a1f170eccfaffb81955edfdfb6bf6479856b0201` |
| `series-timing-expire-check.log` | `eaa10ae6f030dfb8c00cd13148adb727cb1f3d20dad3e9d2bef88c6b83ecbb12` |
| `series-timing-test.log` | `f48d13178bb34027122d0baf7fe9d2f19a8104ffe38bf01a0352561bd781db54` |
| `series-expire-regression-red.log` | `32d2d298f5bd28df8d2b272b18218e952074d498b895cf2ceb79444cac0080e9` |
| `series-expire-regression-green.log` | `0db032b5ee74ffdbca34e83ffafd489467ed95d3cd2ea85db2173604da73b947` |

Logs are under the paired checkout's parent directory. Successful fixtures do
not resolve the pending fresh V2 packet and recurrence execution boundary.
