# Custody allocation PDA reuse — 2026-09-09

## Change

Custody carries the canonical replay bump returned by its existing address
check into InitializeReplay, and the canonical vault bump into OpenVault's
System create instruction. Owner, privilege, address, rent, replay and token
checks and their refusing branches remain intact. No instruction/state ABI or
compute ceiling changes. The canonical search still occurs once; its result
is reused only within that invocation, before any account-changing CPI.

A relayed replay hint is deliberately different. The old identity check could
reproduce a valid noncanonical PDA from that hint, but InitializeReplay then
independently searched the canonical bump for its System signature. Reusing a
hint as if it were a canonical search would therefore broaden admission. The
new private witness carries `Some(bump)` only when the identity check actually
searched; hinted frames retain the original allocation-time canonical search.

The lane also adds feature-gated phase markers for InitializeReplay, OpenVault
and delegated transfers. `custody-cu-profile` remains disabled in production.
The independent caller-continuity repair is present in both final comparison
artifacts; its author owns that change and its adversarial entrypoint controls.

## Instrument first

The General lane's real-SBF diagnostic log
`program-test-delegated-v36-custody-baseline-profile.log` measured:

| Phase | CU |
| --- | ---: |
| Ordinary activation cache decode | 8,330 per invocation |
| Ordinary activation cache identity | 1,851 per invocation |
| Core Market authentication | approximately 5,000 per invocation |
| Replay address/owner authentication | approximately 4,900 per invocation |
| Realm authentication | approximately 5,950 per invocation |
| Repeated InitializeReplay bump search | 4,781 |
| OpenVault create phase, including repeated bump search and System CPI | 3,994 |
| Delegated transfer complete prestate checks | 2,308 |
| Delegated transfer complete poststate checks | 1,431 |

The first profile still contained the now-repaired calling-release omission.
It identifies work, not an accepted release. Token parsing is not the dominant
cost: even all pre/post token checks together occupy only 3,739 CU in this
sample. This lane therefore leaves token-state validation unchanged.

## Controlled comparison with deployment continuity restored

The same General host binary and Trading ELF ran
`program-test-delegated-v38-custody-continuity-baseline.log` and
`program-test-delegated-v39-custody-bumps-profile.log`. Both Custody ELFs carry
the ordinary/upkeep continuity repair committed separately in `26bdfebec`.

| Phase | Corrected baseline | Bump reuse | Saving |
| --- | ---: | ---: | ---: |
| InitializeReplay rent-to-derived checkpoint | 1,781 CU | 264 CU | 1,517 CU |
| OpenVault validated-to-created checkpoint | 3,994 CU | 2,469 CU | 1,525 CU |
| Caller deployment authentication, each of three calls | 1,947 CU | 1,947 CU | 0 CU |
| Delegated complete prestate checks | 2,308 CU | 2,308 CU | 0 CU |
| Delegated complete poststate checks | 1,431 CU | 1,431 CU | 0 CU |

The two targeted duplicate searches save **3,042 CU** in this matched phase
comparison. The baseline replay search in this key draw uses fewer attempts
than v36; the optimization removes that repeated work irrespective of draw,
while keeping one canonical search. Restoring deployment authentication costs
815 CU per call relative to v36's missing check (1,947 versus 1,132), or 2,445
CU across PlaceOrder's three Custody calls.

All three Custody CPIs and both complete PlaceOrder transactions returned
success. Whole-transaction consumption was 1,398,533 versus 1,394,677 CU;
that difference also includes key/PDA-search variation outside the targeted
phases and is not the isolated improvement. Both harness runs then failed
on the existing malformed General Order poststate (`InvalidBody` after the
Order decoder's `InvalidHeader`), so neither is a passing lifecycle. General
owns that repair and subsequent canonical Buy/Sell, width and key-draw
measurements. These narrow savings alone do not establish comfortable margin.

## Native controls

The isolated current Registry V2 source and its workspace-root target are at
`/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909/source`. This checkout
began at `7eb4d69ada25fc8386110e666a511b8c6b27fb51`; only the coordinated Claims
and Custody changes have been applied to it. Custody's package check passed.

Filtered native controls:

- `allocation_`: two tests pass. Canonical replay searches, canonical hints
  and an off-curve noncanonical hint all preserve the original canonical
  allocation bump. The returned vault bump reproduces its authenticated key.
- `tests::series_vacancy_keeps_replay_and_vault_pdas_as_commit_authorities`:
  one test passes; wrong replay/vault keys still refuse with the exact
  `CustodySbfError::Replay` and `CustodySbfError::TokenState` variants.

For the red control, hinted replay addresses were deliberately marked as
canonical in the isolated source. The allocation test failed: bump 250 was
selected instead of canonical bump 252. The accepted source was restored and
both allocation tests passed again. No mutant was built into a delivered ELF.

Logs next to that source:
`check-custody-bumps.log`, `native-custody-bumps.log`,
`native-custody-vacancy-control.log`, `native-custody-bumps-red.log`,
`native-custody-bumps-restored.log`.

## SBF diagnostic provenance

General's warm cohort remains Registry V1. Only the coordinated Custody module
changes were applied there; these measurements are SBF ProgramTest evidence,
not a current Registry V2 cohort, devnet, or mainnet claim.

Artifact/evidence root on hbox:
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/`.

ELF SHA-256:
- Original phase diagnostic (`custody-phases-baseline-elf`):
  `b6951f5a77a284958d6ccd531a5731708b51c5cd924f11dd5dde3282198e7aa8`
- Continuity-corrected baseline (`custody-continuity-baseline-elf`):
  `76ad86a2c28de49396b2302888a593ec70af117fb45c8c201bf753aac76b7cdb`
- Continuity plus bump reuse profile (`custody-bumps-profile-elf`):
  `d4acbb3e4dfa881f5bd60aaf8a7a9403ede9de8926a629846cff900ba2180296`
- Continuity plus bump reuse production (`custody-bumps-production-elf`):
  `0edb699315db76eebef1ff32f3b0e9cd855511040ed18afbfa3dcbb3d3c0d998`

Each directory contains `dclutch_custody_sbf.so`. Native checks preceded SBF
builds and the logs show actual compilations. The first corrected-baseline
check failed because the matching upkeep module had not yet been copied;
after copying that companion change, the corrected check and all three SBF
builds passed. A partial continuity source was never measured as accepted.

Root-module snapshot SHA-256:
- `custody-continuity-baseline.rs`:
  `7551337185efd84ddbc9446fe23e4d423f2b0c244975967385e5a8232d97af95`
- `custody-current-combined.rs`:
  `9ebcbbf646aef064dc06140f7cfbb30a974b7b28218dc856f2ec2ec0ef3523b4`

## Remaining cost and integration owner

Under separate public Custody entrypoints, each invocation must authenticate
its own incoming authority, Registry cache, Core Market, Realm and replay.
The shared immutable checks are already decoded once per invocation. Their
repetition across three CPIs cannot safely be removed by trusting the router
or untrusted account projections.

A bounded future option, if this route still lacks reliable compute margin,
is a Custody-owned atomic preparation operation that creates the replay and
zero-balance vault under one authenticated cache/Market/Realm frame. Another
is explicit resource preparation before the principal-moving PlaceOrder.
Both need a canonical operation/request/receipt and compiler owner; neither
means skipping authorization. They are proposals, not implemented or validated
by this lane. Existing child bump hints may reduce variance if the operator
proves all relevant replay contexts and digest preimages agree before mining.
General owns that builder/integration decision and full lifecycle evidence.

No frame baseline was captured here; this implementation leaves the frame
ratchet red for convergence.
