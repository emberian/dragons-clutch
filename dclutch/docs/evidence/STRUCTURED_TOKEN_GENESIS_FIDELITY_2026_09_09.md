# Structured external Token genesis fidelity — 2026-09-09

**Verdict:** the explicit external Token runtime now reaches local-validator
administration with the exact pinned ELF and canonical Loader immutability.
This is a genesis-fidelity result, not a Structured receipt or full-lifecycle
result, and not public devnet execution.

The checked first-party cohort is
`691bc6ff0ab1c89db6f8ab307e6ff1d75e0557a6`, gate SHA-256
`bc83d0ab5033d4ac5c2a87fd3a40134bf43ca9d7bd7ea633edf6deecf4d5e9de`.
The separate external dependency is the already captured devnet Token-2022
observation in `tools/gauntlet/structured-claims/token-2022-devnet-20260909.json`:
711,008 ELF bytes, SHA-256
`0f3038f9271a900ffd562ffd974934ca8a9c8f8d048f67dbd39aebb35e5b83fb`.
No public RPC call or public Token mutation was needed for this experiment.

## Located defect and control

Host `4bee7cc3af1313901253ea6860809d1a088d6c1a` invoked Agave 4.0.2 with
`--upgradeable-program TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb <pinned ELF> none`.
The executable and its ProgramData had the expected Loader-v3 owner, and the
ELF hash was exact. The ProgramData header nevertheless decoded to slot zero
and `Some(Pubkey::default())`. It was not `None`. The strict post-genesis
check refused before any administration stage. The CLI help's description of
`none` therefore did not establish the loaded account fact.

The failed validator's guard stopped it. A same-ledger restart, without reset,
replacement accounts or warp, permitted two bounded finalized account reads.
The ProgramData observation was at slot 60. That read-only continuation was
then stopped gracefully; its ledger and evidence remain in place.

Commit `32991a37324a246d50d2a648efe061e3ce4e4273` authors the external Program and
ProgramData pair with `UpgradeableLoaderState`, zero deployment slot, `None`
authority, canonical metadata padding, exact ELF bytes and rent-exempt balances.
The validator receives these through two `--account` arguments. They are held
in a separate external account directory; the checked first-party genesis
account directory and its exact census remain unchanged. The strict loaded
account authority and ELF checks remain unchanged.

On the new validator, the finalized ProgramData observation at slot 371 has:

- owner `BPFLoaderUpgradeab1e11111111111111111111111`;
- 711,053 total bytes, with 45 metadata bytes;
- slot zero and authority `None`;
- the exact 711,008-byte external ELF digest above.

The checked substrate then began real Registry administration transactions.
The local-validator profile used 64 ticks per slot.

## Validation and retained evidence

The clean host checkout was
`/tank/dregg-build/structured-host-a93c5c12d-completion/source`, at the exact
32991 commit for validation and campaign launch, using its own `target/`.
The host check, named regression and release build ran through `swarm-build`:

- `cargo check --locked -p dclutch-journey-campaign`: passed;
- `substrate::tests::external_token_genesis_uses_loader_none_and_preserves_exact_elf`:
  1 passed, 932 filtered, with one test thread;
- `cargo build --locked --release -p dclutch-journey-campaign`: passed.

The regression decodes both generated accounts, checks the ProgramData
identity, exact ELF suffix and `None` authority, and distinguishes the measured
native helper's `Some(default)` header. Actual validator execution is the
separate control that the fixture reaches the loaded bank unchanged.

Retained on hbox under `/tank/dregg-build/`:

- `structured-validator-4bee-691-20260909/`: failed transcript, original
  ledger, `token-observed.{json,bin}`, `token-programdata-observed.{json,bin}`,
  same-ledger read continuation log and `token-auth-stop.json`;
- `structured-validator-32991-691-20260909/`: source pin, campaign, external
  account fixtures, `token-programdata-observed.{json,bin}` and
  `token-programdata-facts.json`;
- `structured-host-a93c5c12d-completion/`: `check-32991.log`,
  `token-genesis-test-32991.log` and `run-32991.log`.

The second campaign remained live for its authorized Structured continuation;
its later lifecycle verdict belongs to that continuation's transcript.
