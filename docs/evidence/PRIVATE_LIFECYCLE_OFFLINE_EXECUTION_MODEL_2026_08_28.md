# PRIVATE lifecycle offline expected-execution model — 2026-08-28

Evidence class: offline source-contract analysis. This document is neither SBF
build evidence nor validator, simulation, devnet, or mainnet evidence. The
executable producer is
`tools/release/private-validator-lifecycle/preflight.py`; it reads no key, uses
no RPC, starts no validator, and runs no build.

## Result

The current PRIVATE completion is eight stages, not one opaque end-to-end
command:

```text
founding -> participant -> ALT -> seal -> Direct Hot
         -> resolution -> payout -> retirement
```

Three prerequisite rails precede those completion stages:

```text
checked mutable prepare -> exact local bankroll -> administration/activation
```

Pyth's eight prerequisite mutations sit between Direct and Resolution. They are
not a ninth public lifecycle stage: their terminal four-field facts are an
input to the Resolution semantic owner. Likewise market-input production and
the Direct payout-schedule projection are read-only handoff producers, not
separate completion stages.

The current source derives DCLTGMF2 as a 132-coordinate instruction whose v0
transaction locks 60 complete keys, below the devnet limit of 64. Direct Hot is
also source-pinned: four static plus 57 loaded addresses, 61 unique message
accounts, 1,159 wire bytes, and ten exact poststates. These facts refute a
present lock/packet contradiction in those two routes. They do not predict SBF
frame or compute consumption.

The model is intentionally version-derived. It reads the sole current
`GENERIC_MARKET_FOUNDING_MAGIC_Vn` and the matching fixed, physical-funding,
and complete-key constants. A successor DCLTGMF3 cut must update its Market,
durable founding journal, and runner vocabulary coherently, but the preflight
does not hard-code V3 or silently accept an incomplete rename.

## Full source-derived graph

| Step | Caller and terminal handoff | Signer/key source | Required prestate | Accepted poststate and next consumer |
|---|---|---|---|---|
| prepare | `local-mutable-prepare-v1`, then `local-mutable-plan-authenticate-v1`; `dclutch-local-mutable-prepare-report-v1` | seed-derived disposable roles; Python opens no key | clean source; exact checked gate; 13 links; 18 genesis accounts | seven mutable protocol Loader pairs plus two immutable Pyth pairs; bankroll consumes report |
| funding | Solana CLI one-shot transfer; `dclutch-private-validator-local-test-bankroll-v1`, `/status=finalized` | `core-upgrade-authority` is source and fee payer | campaign payer absent; source has more than 100 SOL plus fee; five protocol-created roles vacant | campaign payer exactly 100,000,000,000 lamports; administration/founding consume the state |
| administration | `campaign --through activation`; campaign report `/execution/completed=true` | exact `campaign_administration_keypairs` projection, solely `core-upgrade-authority` | checked mutable slots and funded payer | infrastructure initialized and release activated; market input/founding consume it |
| founding | `local-private-validator-market-v1`, then founding-only `campaign`; six finalized submission journals | exact `campaign_founding_keypairs`; founder public identity is separately bound | five rent prefundings, exact fixture supply partition, vacant Market/Custody/Claims targets | Open Market, aggregate, founder Position, admission, accepted Resolution funding; participant and Direct consume report |
| participant | `local-private-validator-user-position-admission-v1`; `dclutch-owned-loopback-user-position-admission-execution-v1`, `/phase=finalized` | participant signs owner/collateral; `core-upgrade-authority` pays | vacant Position; exact 100,000,000-atom fixture source | admitted Position plus finalized token/Custody delegation; Direct producer consumes the report |
| ALT | repeated `local-private-validator-direct-trade-v1`; Direct journals `/phase=finalized` | authenticated private-session payer | table vacant or exact durable prefix | one exact frozen, activated 57-address table; seal consumes it |
| seal | same Direct executor and journal completion | authenticated private-session payer | frozen ALT and checked public manifest | exact capability seal; Hot consumes it |
| Direct | `local-private-validator-direct-trade-produce-v1`, repeated executor, then `local-private-validator-direct-payout-schedule-v1`; finalized evidence `/status=finalized` | founding founder sells; participant buys; session owns payer | exact Market/root/Positions/tokens; replay roots vacant or authenticated | ten poststates and exhaustive K seller plus one buyer claim schedule; Pyth/Resolution run, then payout consumes schedule |
| Pyth prerequisite | eight ordered `local-private-validator-pyth-vaa-provision-v1` journals and one terminal reauthentication | payer and EncodedVaa keys; update signer remains unopened/vacant | EncodedVaa and update vacant; immutable slot-zero Receiver/Router | verified VAA journal prefix and four-field facts; Resolution alone performs Receiver PostUpdate |
| resolution | repeated `local-private-validator-flagship-resolution-v1`; checkpoint V3 `/verifiedTerminal=true` | table authority, submitter, resolver, update signer from prepared key directory by authenticated address | Pyth facts; active funding; Core Open | submit, provider-execute, Core-accept, reclaim in strictly advancing finalized slots; payouts consume terminal Market |
| payout | read-only payout-input plus repeated `local-private-validator-wallet-terminal-payout-v1`; evidence `/phase=finalized` | each claim owner; `core-upgrade-authority` pays | selected Position balance and aggregate supply positive | winner credits collateral; losers are real zero-collateral burns with byte-identical Custody/token state; retirement consumes exact supply-zero state |
| retirement | terminal sequence to `15-retirement-replay-handoff.json`, then `local-private-validator-aggregate-retirement-v1`; completion `/status=finalized` | `core-upgrade-authority` pays; program child authorities are PDAs | every claim supply zero, Core terminal, source receipt and frozen ALT exact | prepare, close-vault, close-replay, finish; exact rent/refund conservation; activity/session/receipt consumers follow |

## Caller exposure and final evidence

The one-seed full probe needs fourteen exposed successor commands: five
founding/participant commands and nine Direct-through-retirement commands. The
twenty-seed terminal mode additionally needs six final evidence callers:

- Pyth provider closure;
- activity stage completion;
- activity manifest;
- finalized activity capture;
- private lifecycle session;
- aggregate lifecycle receipt.

For every selected mode, the offline preflight proves both sides of reachability
from source: a dispatch arm in `main.rs` and the exact command string inside the
help function that `main::usage` exposes. It separately joins sixteen runner
schema constants back to their Rust semantic-owner modules. A string merely
existing elsewhere in a module is insufficient. This is the distinction that
caught the Direct command being dispatched but hidden from accepted help.

The three other usage surfaces carrying literal patch markers were repaired in
the same cut: Pyth prerequisites, wallet payout, and aggregate retirement. The
preflight now refuses `\\n+` inside any required help function.

## Cross-stage quantities

The fixture supply has one exact partition:

```text
Token-2022 total supply = founding collateral atoms + 100,000,000 fixture atoms
```

Mint authority must be removed. Direct's owned-loopback producer fixes its fill
at 100,000,000 atoms and independently proves that the participant has enough
gross collateral plus fee; the runner never turns lamports or fee arithmetic
into token quantity.

The Direct payout schedule is deliberately exhaustive, not winner-only. It
contains every positive seller claim coordinate plus the filled buyer
coordinate. After terminal resolution the payout route burns losing claims with
zero collateral and no Custody effect. Retirement is correctly unreachable
until every aggregate supply coordinate is zero. Treating a zero payout as a
no-op would be a supply leak; treating it as an error would make retirement
unreachable.

## Predicted and observed frontier

The source read established these outcomes before another validator campaign:

- **confirmed and repaired:** Direct was dispatched but missing from help;
- **confirmed and repaired:** three other owned-loopback help strings printed
  literal leading `+` characters;
- **confirmed by the frozen one-seed probe:** after the Direct help repair, the
  next actual wall is still before validator launch: mutable preparation
  reports a checked-gate semantic-preimage mismatch;
- **refuted for the current source:** DCLTGMF2 is not over the 64-key limit; it
  is 60 complete keys;
- **refuted for the current source:** Direct Hot is not over the limit or packet
  bound; it is 61 keys and 1,159 bytes;
- **refuted as an economic contradiction:** the K+1 payout schedule is required
  because losing claims must be burned through zero-payout executions;
- **still runtime evidence owed:** Pyth prerequisite execution, the four-stage
  Resolution V7 checkpoint, every positive/zero payout, the terminal handoff,
  and four AggregateRetirement packets.

## Running the offline gate

```sh
python3 tools/release/private-validator-lifecycle/preflight.py \
  --repo /absolute/clean/dclutch \
  --through full-probe \
  --output /absolute/new/preflight.json
```

The output path is create-new and mode 0600. The report binds every source file
it relied on by SHA-256 and binds its own canonical model as `model_sha256`.

Focused adversarial gate at this cut:

```text
python3 -m unittest -v tools/release/private-validator-lifecycle/test_preflight.py
19/19 passed
```

The hostile cases delete dispatch, hide help, reintroduce patch markers, drift
schemas, zero or substitute fixture supply, add an airdrop role, raise founding
above 64 keys, split the founding magic join, reorder private stages and Pyth
actions, alter Direct geometry/vocabulary, remove zero-payout semantics, reorder
Resolution receipts, move the retirement handoff, and try to clobber output.

