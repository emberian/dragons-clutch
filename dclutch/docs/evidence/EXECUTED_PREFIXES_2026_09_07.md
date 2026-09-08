# Executed prefixes and current-source walls — 2026-09-07

Owner: CODEX-INTEGRATION. These are local observations, not devnet evidence.

## Artifact identity

The strict release build at `56767e555d05ecd005edec4fc939d269e8757417`
passed all eight shipped links and the diagnostic Trading link. Its checked
gate is `hbox:/tank/dregg-build/dclutch-codex-release-b-20260907/CHECKED_UPGRADE_GATE.json`,
SHA-256 `6031115cd75da111c10d248a8489d7179d8edd5d320bc59e024a5f76e7107909`.
The shipped Trading ELF is
`27988083edccb86cb190e1317eece27478a586e425155133a75082a48d2fdffd`.
The diagnostic ELF is
`0b79d41ce742e08e3f4d02b07b827ffd590bc85e4cd138d18829e66173f3e4cc`;
it is an instrument, not a deployment artifact.

The preceding release build at `cac1a0593` refused five frame-overwrite
diagnostics in Hot execution. The Series replay observation extraction at
`56767e555` removed those diagnostics. Two independent eight-link frame
captures agreed byte-for-byte; `dd6dda9b1` carries the accepted baseline.
Any later runtime change needs new captures and a fresh checked release.

## Whole-market local validator

Run directory:
`hbox:/tank/dregg-build/dclutch-codex-journey-20260907/runs/20260907T192044Z-56767e555d05-h4`.
Runtime programs came from the checked build above. The old runner overlaid
then-current Journey host files on the archived revision. This is diagnostic
execution with identified runtime artifacts, **not an exact-revision release
campaign**. `dd6dda9b1` removed that silent overlay; normal runs now use the
gate's archived sources, and host development requires explicit worktree mode.

The transcript records 252 transactions, zero conservation violations, and
these successful stages: founding through Open, four funded holders and their
collateral-transfer ring, Direct activation, two ordinary Position admissions,
a founder-to-stranger nonzero Direct trade, permissionless fee settlement,
Pyth/Wormhole source resolution, and Core terminal admission.

The first wallet payout also finalized: 300,000,000 collateral atoms, signature
`4z6mJqsTXN7vbXBc4BptF5qGX63Dsxtzk3381mY3FEsUkrK1ZSY3kfKJMaWgWKgDhrVk5oD2dh54dUKhaY9Ty2nP`.
Its shipped operator evidence contains seven observed poststates. The campaign
then refused while reading that report: it used JSON `as_u64`, while the
operator's `EvidenceV1.payout` is an exact decimal **string**. The report was
present; its representation disagreed with the reader. The repair reads the
canonical decimal string, preserves quantities beyond JavaScript's exact
integer range, and refuses malformed values and aggregate overflow.

Artifacts beneath the run directory:

| Artifact | SHA-256 |
|---|---|
| `transcript.json` | `c788c885546691befb33a7a4da39852168bd7c3d89d27f5dfabd09012aa83de5` |
| `campaign/spine/payout-evidence-buyer-claim-0.json` | `ee70d9ea2a087c565e127c8774275abc4b2627b648b320c9c2a79340d90c875f` |

The reader repair's focused native control passes with the shipped string
shape, zero, a quantity above 2^53, and distinct refusals for missing/numeric,
negative, and noncanonical values. This does not close the whole-market
campaign: remaining payouts and retirement were never entered and must run.
The old infrastructure witnesses also ask this different campaign for stages
it never declared; their failures cannot be recast as executed transactions.

## Earlier infrastructure-only validator

At `0a869a76633f2755ef3d359c2d8b63b5b6447f3b`, tier 1 completed atomic founding
and active funding, but failed thirteen compute-budget comparisons. Raw run:
`hbox:/tank/dregg-build/dclutch-codex-validator-20260907/runs/20260907T183217Z-0a869a76633f`.
No budget was widened. Its census covered 36 executed routes, one refused-only
route, and 128 never-executed routes. Compilation is not coverage.

## Remaining family walls at the checked runtime revision

- Series pre-Market expiry: four ProgramTest cases passed, three failed.
  The accepted case refused `TradingSbfError::Content`. Profiling located
  ordinary-Custody parent Market/generation checks: Series selection compared
  a domain-separated Template identity with the Registry's raw-record digest
  and consequently never selected pre-Market behavior. A native control was
  proved red before repairing that comparison. The next execution exposed an
  additional missing projected-Custody dispatch. Those repairs are ongoing.
- General `open_batch`: one ProgramTest case passed, five failed, including
  accepted Open/Close and permissionless seal creation. A filtered diagnostic
  run located the accepted failure at the lifecycle/funding-profile join,
  `ProfileMismatch`. No General accepted lifecycle is credited.
- Dealer: the new local-validator campaign at host `dd6dda9b1` completed its
  38-transaction checked substrate, then the market compiler refused absent
  fee selection. `cd88b86b1` supplies an explicit 50 bps choice and disposable
  recipient. The rerun is required before nonzero Dealer use is credited.

Raw family logs are under
`hbox:/tank/dregg-build/dclutch-codex-evidence-20260907/`: `series-premarket.log`,
`series-premarket-profiled.log`, `series-selection-red.log`,
`general-open-batch.log`, `general-profile-located.log`, and
`dealer-validator.log`. These verdicts remain failed or incomplete until a
dated follow-up names the new artifacts and accepted poststates.

## Dated addendum — 2026-09-07, later development wave

The preceding verdicts describe their named revisions, not the latest queue.
The Direct tail subsequently completed payouts and retirement on the preserved
567 runtime; its artifact hashes, signatures, refunds and five absent terminal
accounts are recorded in
`JOURNEY_RUNTIME567_DIAGNOSTIC_TAIL_2026_09_07.md`. Recovery exhaustion also
reached Core/Claims terminal admission and three ordinary-founder refunds; see
`RECOVERY_EXHAUST_LOCAL_VALIDATOR_2026_09_07.md`. Both are diagnostic execution
with stated host-provenance limits, and neither substitutes for a complete
fresh final-release campaign.

Other prefixes advanced without completing their families. Structured reached
root activation, then exposed a demo producer's invented execution-release ID
in its immutable Token behavior configuration. The producer repair
`a57a54fa3` requires a fresh Market. General has progressed beyond the old
ProfileMismatch into authenticated external-account projections; a complete
accepted nonempty lifecycle remains owed. Ensemble reached controller funding
and exposed a three-row allocation for a four-row Resolution ledger. Dealer's
canonical credit was correct, but the adapter incorrectly required its refund
beneficiary to equal the sponsor; `df6fddba9` removes that false equality while
retaining the Core-selected credit anchor. These are located defects and code
repairs, not accepted terminal execution evidence.

The current implementation queue lives in
`../design/DEVELOPMENT_WAVE_2026_09_07.md`. Reading an earlier dated failure or
`tools/gauntlet/blocked.json` alone cannot establish what has never run since.
