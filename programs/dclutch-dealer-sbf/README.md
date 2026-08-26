# dclutch-dealer-sbf

This crate supplies the reusable Dealer-family handler for dClutch's one
data-driven Trading controller. Its `cdylib` entrypoint is retained only as a
prototype and binary/CU measurement of that same handler. A standalone Dealer
ELF is not an accepted second Trading identity: one `ExecutionReleaseSetV1`
selects one Trading program, which must dispatch Dealer, General, Series, and
other Trading families by data.

The width-polymorphic, Lean-owned interpreter remains in
`dclutch-dealer-codec`. This adapter authenticates immutable Policy and Candidate
accounts, Dealer State, canonical sparse Core identity, exact Realm and token
profile, the transaction actor and Clock, and the current Core/Trading/Custody
deployments. Claims is authenticated only for plans that use it. It executes
the interpreter's bounded Claims and Custody children, checks each canonical
receipt and physical postcondition immediately, reinterprets against unchanged
Dealer inputs, and writes Dealer State last.

## Exact account shape

Every action has the same 23-account authenticated prefix:

1. actor, Policy, active Candidate, optional pending Candidate, optional
   proposed Candidate, Dealer State;
2. Registry activation cache and Registry program;
3. canonical Trading program and ProgramData;
4. canonical Custody program and ProgramData;
5. sparse Core Market, current Core program, and ProgramData;
6. Clock, exact Realm, collateral Mint, Realm-selected token program, and
   Custody transfer authority;
7. canonical Dealer quote, fee, and funded-liveness vaults.

All three state-bearing Dealer vaults are derived and authenticated on every
route, even when untouched. Their writable privileges are exact consequences of
the interpreted plan. The suffix then contains only resources that the plan can
use:

- Claims program, ProgramData, caller authority, aggregate, Dealer Position,
  and an actor Position only when required;
- one Custody replay account, one caller authority per transfer, and only the
  dynamic token accounts named by those transfers.

Current exact widths are 35 accounts for Fill, 33 for terminal Unwind, 30 for
Retire, 27 for ScheduleReplacement, 26 for ActivateReplacement, and 23 or 26 for
EnterTerminal depending on whether a funded pending Candidate must be refunded.
These are derived interface widths, not width-monomorphized economic routes and
not a claim that Solana imposes a mathematical 35-account limit. The parser
refuses both missing and surplus accounts.

## Semantic ownership

Core solely owns immutable Market identity, selected Realm and release set, and
lifecycle/winner. Dealer terminal entry is permissionless and accepts only the
authenticated Core Terminal/Retiring phase and exact winner; there is no
resolver signer. Claims owns its aggregate and Position derivations plus claim
phase projection. Custody owns vault derivations, replay, token transfers, and
receipts. Dealer imports those contracts and does not persist parallel copies of
their balances, authority, or lifecycle facts.

The fixed Policy (216 bytes), Candidate (4,576 bytes), and State (840 bytes)
formats and their total transitions are generated from the Lean ABI source.
The named Lean results cover totality, fragmentation-independent fee
accounting, funded liveness, replacement timing/revisions, custody conservation,
and terminal unwind at that semantic layer. This is not a claim that the SBF
adapter, Solana runtime, child programs, or canonical Trading dispatcher are
formally verified.

## Remaining executable boundary

The standalone adapter must still be wrapped by the canonical Trading
controller, and its current Registry join is intentionally isolated in
`authenticate_trading_controller_release`. Creation/initialization instructions,
operator builders, and real-SVM success/hostile/overflow/late-CPI-rollback/CU
campaigns are separate unfinished vertical work. Until controller integration,
standalone measurements are prototype evidence only and are not a deployable
release manifest.
