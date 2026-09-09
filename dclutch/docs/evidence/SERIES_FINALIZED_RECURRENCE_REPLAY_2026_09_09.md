# Series finalized recurrence replay — 2026-09-09

The Found/Prepare input adapter now takes an explicit occurrence index and
selects that occurrence's authored Ticket and inclusion proof. For a finalized
parent root it reads and decodes the actual replay state; only the explicitly
predicted activation variant constructs initial state. Prepare requires the
requested index to equal the replay's next occurrence and refuses an already
prepared current Ticket. The existing first-occurrence entrance supplies zero
explicitly; later-occurrence campaign attachment is still outstanding.

The shared operator owns `decode_series_root_replay_v1`, used by both finalized
corpus acquisition and authoring. It authenticates finalized Trading ownership,
non-executable state, the canonical root PDA, header Template/release identity
and exact replay width before invoking the native `SeriesStateV3` decoder.
Authoring also refuses a root whose observed width or balance has changed since
the supplied finalized input fact. No account is seeded and no failed decode
falls back to initial state.

The filtered second-occurrence control first ran with an intentional
initial-state substitution in the paired hbox checkout. It failed at the exact
state equality assertion (`RED_EXIT=101`); restoring the observed decoder
passed (`GREEN_EXIT=0`). The same control rejects another account owner by its
exact located refusal. Locked operator/bootstrap checks passed afterward.
Actual operator compilation is present in the logs; source files were copied
without preserving stale modification times.

Evidence is under
`/tank/dregg-build/dclutch-claims-sparse-a8e3b4e8c-v2-20260909`:
`series-replay-reset-red.log`, `series-replay-preserve-green.log`, and
`series-replay-check.log`. Driver:
`/tank/dregg-build/series-replay-control-driver.log`. The paired checkout's HEAD
was `da35e8e70e7fb662a73efe954342709a9c7e6eba` with these exact scoped source
patches, and its original Claims test changes were preserved. This is a host
fixture/check level, not selected-runtime recurrence execution. No SBF build,
clock warp, validator restart or public submission accompanied this change.
