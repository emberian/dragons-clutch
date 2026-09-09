# General PlaceOrder replay bump hints — 2026-09-09

Final same-fixture production measurement: guarded replay hints reduce whole
PlaceOrder from **1,367,992 to 1,366,545 CU**, a **1,447 CU** saving. The
first-attempt search path remains absent to avoid a measured 1,553 CU
regression. This bounded saving does not establish robust Buy/Sell or
larger-width headroom; General owns those continuing benchmarks.

## Scope and authority

The General operator and chain bundle builder share one host projection of
PlaceOrder's Custody replay bump. The input is the decoded PlaceOrder subject,
which the operator derives from authenticated `GeneralSignedOrderTermsV2.order_id()`.
The selected Effect projects that identity into all three Custody request
contexts: InitializeReplay, OpenVault, and delegated Transfer. This projection
is restricted to PlaceOrder; other actions and absent subjects leave the hint
absent. The shared family-neutral miner and all runtime validation are unchanged.

The bump travels in the existing Hot envelope's `child_relay[0]`. It does not
change the family request, Ed25519 message, child request digest, ABI, account
frame, or compute limit. The chain reproduces the supplied replay coordinate
against the actual incoming account. Custody initialization still searches its
canonical allocation bump, including when a hint supplied a matching address;
a hinted noncanonical address never becomes canonical allocation authority.

This is a host-only change. No SBF ELF was rebuilt for it. Existing frame-ratchet
debt from runtime changes remains outstanding.

## Current V2 native controls

Validation used the owned archive and its own workspace-root target:

`hbox:/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909/source`

The archive began at `7eb4d69ada25fc8386110e666a511b8c6b27fb51` and retained the
coordinated Claims/Custody work documented by the preceding evidence. The exact
two-file host patch was applied with `git apply --check` before application.
The bundle check initially exposed the archive's partially updated resolution
closure: Trading still imported removed `ProviderUpdateLifecycleV3` names.
All 30 files from the atomic committed `a28a1662c` closure were then installed
together; the corrected bundle check passed. No individual runtime import was
patched to disguise this mismatch.

All builds used `SWARM_MEM_MAX=24G CARGO_BUILD_JOBS=6 swarm-build` at that
workspace root. Every log showed actual compilation or named test execution;
none reported a preloaded scope as success.

| Control | Result | Log beside source |
| --- | --- | --- |
| `cargo check --locked -p dclutch-operator --no-default-features --lib` | passed | `check-general-replay-hints.log` |
| `general_hot_v3::tests::place_order_replay_hint_uses_signed_subject_and_preserves_family_digest` | passed, 1 test / 374 filtered | `native-general-replay-hints.log` |
| Same control with helper returning the old zero hint | failed, exit 101; Market draw 1 observed zero instead of canonical 254 | `native-general-replay-hints-red.log` |
| Exact accepted helper restored | passed | `native-general-replay-hints-restored.log` |
| `general_hot_v3::tests::the_mined_corpus_reads_this_frames_market_root_and_custody_deployment` | passed | `native-general-replay-corpus.log` |
| Initial `cargo check --locked -p dclutch-chain-bundle-builder --lib` | failed, missing old resolution lifecycle imports | `check-general-replay-bundle.log` |
| Same bundle check after complete `a28a1662c` closure | passed | `check-general-replay-bundle-v2.log` |

Each native test used `cargo test --locked -p dclutch-operator
--no-default-features --lib <exact-name> -- --exact --test-threads=1`.
The new control derives an actual signed order subject, varies 64 Market seed
sets, checks canonical replay reproduction across differing search depths,
checks the unchanged family digest, and leaves unrelated actions or missing
subjects absent. The old-zero-hint mutation was restored before subsequent
checks and was never delivered as an accepted builder.

## Real-SBF comparison

The shared General diagnostic is a Registry V1 continuation, not a current V2
release cohort. Its source, targets, production links, and logs stay under:

`hbox:/tank/dregg-build/dclutch-general-delegated-56904d11-20260908`

General owns all host execution, ELF swaps, and harness restoration. The named
continuation uses fixed participant seeds `[0x11; 32]` through `[0x15; 32]`.
Both comparison hosts must use the same production links and participant keys.
No validator was restarted and no new cohort was deployed by this lane.

The first same-production comparison held the v47 production ELF set fixed.
The only host difference was carrying the mined replay hint. `v47` is
`program-test-delegated-v47-effect-production.log`; `v48` is
`program-test-delegated-v48-production-replay-hints.log`.

| Production PlaceOrder child | Absent hint CU | Carried hint CU | Saving |
| --- | ---: | ---: | ---: |
| InitializeReplay | 43,033 | 44,588 | −1,555 |
| OpenVault | 48,098 | 46,597 | 1,501 |
| Delegated Transfer | 51,539 | 50,038 | 1,501 |
| All three Custody calls | 142,670 | 141,223 | **1,447** |

Claims remained 107,491 CU and Accelerator 55,082 CU. Both whole PlaceOrder
transactions exhausted the chain meter, so this pair proves the aggregate
Custody saving, not successful whole-transaction headroom.

A separate controlled pair used the accepted v46 **profiled Trading** link,
with the same links and keys in both runs. `v49` is
`program-test-delegated-v49-profile-unhinted.log`; `v50` is
`program-test-delegated-v50-profile-hinted.log`.

| Profiled PlaceOrder measurement | Absent hint CU | Carried hint CU | Saving |
| --- | ---: | ---: | ---: |
| InitializeReplay | 41,533 | 43,088 | −1,555 |
| OpenVault | 51,098 | 51,097 | 1 |
| Delegated Transfer | 51,539 | 51,538 | 1 |
| Whole PlaceOrder transaction | 1,368,318 | 1,369,871 | **−1,553** |

This uncensored whole-transaction regression is why the final helper leaves a
first-attempt canonical bump (255) absent. The original pair did not print the
bump: its first-attempt classification is inferred from the measured search
cost signature, not an observed byte. The later native branch control directly
covers canonical 255 and the final runtime pair prints canonical 254. It carries deeper canonical bumps
and preserves zero's existing absent-hint representation. This is a measured
cost policy, not a protocol-validity bound; it must be remeasured if the chain's
syscall cost profile changes. Initialization's canonical search is preserved.

## Final cost-policy controls

The final 64-seed test explicitly requires coverage of first-attempt,
two-attempt, and deeper search draws. Removing the cost guard makes Market
draw 2 emit 255 instead of the expected absent byte 0: exit 101. Restoring the
accepted guard passes, one test with 376 filtered out. The final bundle lib
check passes in 8.17 seconds. Logs beside the V2 source:

- `native-general-replay-crossover-red.log`
- `native-general-replay-crossover-restored.log`
- `check-general-replay-crossover-bundle.log`

These final checks also include the complete eight-file committed
`8c44cbff8` Initialize evidence closure, applied only after its full patch
passed `git apply --check`. An intermediate archive-to-live diff had accidentally
included its operator hunk without companion files and correctly failed on
missing `decode_owned`/test-module definitions. That intermediate patch was
withdrawn before General used it; the corrected runtime-comparison patch has
only the helper and seed-control hunks. The complete Initialize closure was
then applied to the V2 archive for the final native checks above.

## Link identities

The exact production cohort list is `production-v47-sha256.txt` in the General
base. Production Trading is
`5e2c8849a513a582fb2faa29ea8c6331ad34d14e5666b7a3808bf698aab3dc71`;
the separately measured profiled Trading ELF is
`2836d116d352f4ba6bfb088ef3630bd2c85f5ff1264be076929bb7b888c0bde4`.
Custody is unchanged across every hint comparison:
`0edb699315db76eebef1ff32f3b0e9cd855511040ed18afbfa3dcbb3d3c0d998`.
These are diagnostic link identities, not a declaration that this older warm
cohort is built from current V2 sources.


## Final same-fixture guarded policy and hostile control

After General's committed `a3dd6962f` Verify evidence correction, its complete
artifact catalog changed the founded manifest and therefore the Market seed
corpus. The final measurement does **not** compare that new host with a saved
older binary. One newly compiled host supplies a temporary toggle that changes
only the existing envelope replay byte to zero before submission; with the
toggle absent it submits the guarded builder's byte. Both arms retain identical
Market/release-set bytes, fixed participant keys, instruction geometry, and
production ELF set. The exact identities are printed in each log.

| Same current fixture | Absent hint CU | Guarded hint CU | Saving |
| --- | ---: | ---: | ---: |
| Production whole PlaceOrder (`v54` / `v55`) | 1,367,992 | 1,366,545 | **1,447** |
| Production headroom | 32,008 | 33,455 | 1,447 more |
| Profiled Trading whole PlaceOrder (`v56` / `v57`) | 1,381,860 | 1,380,413 | **1,447** |

Both current corpora print mined bump 254; the baseline carries zero and the
treatment carries 254. All three Custody calls total 144,170 versus 142,723 CU
in both pairs: InitializeReplay increases 1,555 CU and the other two calls each
save 1,501 CU. Both PlaceOrder executions and their real poststate checks pass;
the full named campaign subsequently stops while constructing Verify at
`Lifecycle("disabled-plan")`. No row is presented as a passing complete lifecycle.

Runtime logs in the General base:

- `program-test-delegated-v54-final-production-absent.log`
- `program-test-delegated-v55-final-production-hinted.log`
- `program-test-delegated-v56-final-profile-absent.log`
- `program-test-delegated-v57-final-profile-hinted.log`

A separate temporary public-entrypoint control changes only the replay hint
from the current canonical 254 to 1. Production `v51` and profiled Trading
`v52` both refuse exact `CustodySbfError::Replay`, derived from the registered
Custody band, and preserve all **13 writable account poststates**. The fee
payer is excluded because a refused transaction still pays its transaction fee.
Deleting only this hostile-byte assignment proves the control red: `v53`
honestly commits PlaceOrder at 1,380,413 CU and the expected-refusal assertion
fails. The actual hint checker was not bypassed or weakened.

- `program-test-delegated-v51-replay-hostile-production.log`: passed
- `program-test-delegated-v52-replay-hostile-profile.log`: passed
- `program-test-delegated-v53-replay-hostile-red.log`: expected failure, exit 101

Host build logs are `host-replay-hint-hostile.log`, `host-replay-hint-red.log`,
`host-replay-hint-final-measure.log`, and `host-replay-hint-final-restored.log`.
All executions use the exact filter
`one_founded_market_opens_and_then_closes_its_batch_in_one_bank --exact
--nocapture --test-threads=1` through `swarm-build`.

The original parent harness was restored byte-for-byte with `cmp` confirmation;
production Trading was restored by writing a distinct file and renaming it,
then its SHA-256 was rechecked. The accepted parent host was rebuilt after
removing every temporary probe. Only the guarded helper/builder source remains
in the warm source. General received the shared host/ELF mutex back before its
next runtime build. No temporary probe, mutant, measurement switch, or ELF is
part of this host-only commit.
