# dClutch cold-start handoff — 2026-09-09

This is the single-file restart point for a new lead arriving cold. It records
the current repository, public site, retained runtime state, unfinished work,
blockers, and the dependency order to a working devnet aquarium and then
whole-protocol completion.

## Start here

Work in `/Users/ember/dev/dclutch`. Read `AGENTS.md` first. The publication
repository is `/Users/ember/dev/dragons-clutch`; publish source only through
`tools/cut.sh`. Do not edit its `dclutch/` copy directly.

Last implementation commit before this handoff:

```text
9ae8e0dab76d97184e9dc23bb2640081df080ba6
trading: admit exact reused Direct fee account; leaves frame ratchet red
```

Publication cut containing that implementation:

```text
7661ab3e408c7aa93cfd7e95a0e61c39b78367df
```

The cut gate established that `7661ab3e4:dclutch` is byte-identical to that
source revision. The live working tree still has about 190 modified or untracked paths.
They contain interrupted work and formatting noise. Never stash, reset, clean,
blanket-stage, or blanket-commit them.

Use one Sol lead and at most one bounded Terra deputy by default. Luna is for
mechanical, independently checkable work. Close one executable vertical slice
at a time instead of starting another broad audit.

## Public site

The existing GitHub Pages implementation is live at
`https://clutch.dregg.pro/`.

Workflow run `34374492416` successfully built the existing frontend, assembled
and link-checked 246 public pages, uploaded the artifact, and deployed it from
publication commit `9d9b9dfff0f01571ce16e9f2284b0fd7b09b1a73`.

Source commit `a7b9052f4` repaired a stale generator check that demanded prose
removed during the plain-language rewrite. Pages was deliberately deployed
before the fresh protocol cohort; it reads the selected existing devnet
deployment and must not present unfinished controls as usable. The later cut
`7661ab3e4` adds only the Direct protocol repair below.

## What is deployed and what is not

- Cohort 17 remains the previous public devnet deployment.
- A fresh all-eight-program cohort from current source is not deployed.
- No devnet aquarium is running.
- The prior local Direct simulator executed three nonzero fills, settled fees,
  and resumed without sending an already-submitted transaction again.
- Those fills always used the founder as seller. They do not prove a renewable,
  reciprocal aquarium.
- Older Dealer, Ensemble, Claims, retirement, and payout campaigns executed at
  their recorded revisions. They are not an integrated current-source release.

## Hbox and storage

Heavy builds, validator ledgers, simulator runs, and evidence belong under
`/tank/dregg-build` on hbox, never `/home/hbox`.

`/tank` is a writable ZFS pool. It was full, rather than globally read-only;
several old POA child datasets were read-only. The authorized old POA datasets
have been destroyed. An authorized cleanup of top-level
`/tank/dregg-build/dclutch*` roots older than three days is continuing:

```text
PID 1251893
script /tmp/dclutch-clean-old-roots.sh
log /tmp/dclutch-clean-old-roots.log
```

At the last check the pool had 27 GiB free and the cleanup was still running.
Check the log and `df -h /tank` before a build. Do not start a second cleanup.

Five retained validators are stopped with SIGSTOP:

| PID | Retained run | RPC |
| --- | --- | --- |
| 440761 | fd7 Direct simulator floor | 46024 |
| 443622 | a8 Structured runtime | 30400 |
| 751542 | a3dd Series diagnostic runtime | 31100 |
| 4106046 | older 691 Direct runtime | 43144 |
| 4166530 | older 691 Structured runtime | 28783 |

Do not resume all five. The revised Direct program cannot be inserted into an
old immutable selection. Build current source and start one fresh compatible
Direct validator for the reciprocal proof.

## Blockers in dependency order

### 1. Direct reciprocal seller setup

The first seller creates its Token-2022 PDA and the venue's shared fee PDA. A
later seller has a vacant seller PDA and that valid initialized fee PDA. The old
program and native driver required paired vacant or paired initialized states.

Commit `9ae8e0dab` repairs this:

- seller and fee accounts are classified independently;
- a system-owned zero-data PDA is vacant;
- reuse requires exact Token-2022 program owner, rent, collateral mint,
  configured owner, initialized zero-balance base bytes, and no extensions;
- the SBF handler skips creation only for that exact initialized account;
- the native journal planner accepts vacant seller plus exact existing fee.

Touched files:

```text
crates/dclutch-trading/src/token_setup_v1.rs
programs/dclutch-trading-sbf/src/direct_token_setup_v1.rs
tools/local-validator/bootstrap/successor/src/direct_trade.rs
```

Locked metadata, both touched library checks, and the focused classifier test
passed. The classifier rejects wrong owner, rent, mint bytes, frozen state, and
nonzero amount.

The focused SBF test binary and successor driver currently fail to compile
because unrelated dirty Structured code imports private
`dclutch_release_tool::infrastructure::CheckedInfrastructureV1`. The Direct
repair is not runtime-proven and its frame capture is owed.

Smallest closure: isolate or finish that Structured visibility change, run the
focused SBF acceptance/refusal cases, build the changed Trading ELF on hbox,
then execute seller A setup, seller B mixed-state setup, A-to-B and B-to-A
trades, fee settlement, hostile fee-account rollback, and restart on one fresh
validator.

### 2. Aquarium runner

Dirty `tools/load-simulator/{aquarium,simulator}.py`, their tests, and
`direct_trade_producer.rs` already add per-role participant evidence,
rotation, alternate seller destinations, and replenishment. Close this after
the runtime proof. It must show two identities trading both directions,
preserved fees and inventory, finalized poststates, and restart without
duplicate sends.

### 3. Direct public clients

The dirty TypeScript CLI `buy` path has a durable exact-packet journal,
submitted-transaction recovery, finalizer, and buyer-taker flow. It is untested
and uncommitted. Main paths:

```text
packages/dclutch-cli/src/commands/trade.ts
packages/dclutch-cli/src/directTradeJournalFile.ts
packages/dclutch-sdk/lib/directWalletPreparationV1.ts
packages/dclutch-sdk/lib/directTradeJournal.ts
packages/dclutch-sdk/lib/clientOperationJournal.ts
```

Immediate terminal `sell` is a separate limitation: wallet preparation assumes
a portable seller offer and an authenticated buyer taker. Aquarium rotation
does not generalize that SDK path. C-04/C-12 ultimately require it.

### 4. General matched settlement

- Buy Place/Submit executed on the actual runtime.
- Sell passed the first normalized-affine composition check.
- Another reader still decoded raw zero-transfer rows.
- Verify reached Result rent: actual 2,227,200 versus projected zero. Current
  Rent scalar 147 and the Credit beneficiary repair are correct; the intended
  terminal Result output is dirty.
- Untracked `programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch/placed_order.rs`
  is the positive matched-order helper.

Close one accepted Buy plus Sell match through the real handler and
Verify/Effect poststates, with hostile normalized-row and rent controls.

The width-258 wall is a measured program scalar-bank/profile bound. It is not a
client compute-budget setting and cannot be fixed by requesting more
transaction CU. Split or redesign that profile if the width is required.

### 5. Structured lifecycle

Fresh market `CPbaUQsiedRBCr5CpkzqPaoquqF2beAoy5ajoisrbC5d` on the retained a8
validator opened and activated coordinates 0 and 1 at 544,206 and 556,206 CU.
The next operation used one address as both a readonly protocol account and
transaction fee payer. Use a distinct fee payer.

Dirty work spans the four-action planner, new WASM crate, native
acquisition/driver, browser panel, review model, and lookup-table provisioning.
Close ActivateReceipt, ActivateCoordinate, RetireCoordinate, and RetireReceipt
on one current-source chain with exact poststates and wrong-meta control.

### 6. Dealer multi-provider liquidity

Dealer is a large integration slice: about 1,267 dirty lines of discovery,
1,127 of projection, runtime equity-profile changes, SDK recovery, and the
existing workspace. Older Dealer execution does not prove this V4 topology.

First close native discovery/projection against an accepted corpus and execute
one multi-provider lifecycle adding and removing distinct LP tranches. Generate
WASM and exercise SDK/browser wallet flow after that runtime seam passes.

### 7. Series recurrence

Commits `393f260aa` and `ae6c56170` land shared corpus acquisition and explicit
occurrence/finalized replay authoring. Remaining: the Shadow Certificate
producer and a current-source campaign with two occurrences, consume and expiry,
once-only funding redistribution, both ticket retirements, terminal close, and
safe replay. C-07 names six required routes across Core, Claims, and Trading.

### 8. Complete protocol test coverage

Several program tests compile but have never executed an accepted route. A
green refusal suite may exercise only the first failing conjunct.

Derive a campaign matrix from actual entry points. Every intended chain-facing
route needs:

1. an accepted case reaching and checking its poststate;
2. a named exact refusal control proven red before repair;
3. rollback and conservation checks where value mutates;
4. a production CLI, web, or operator driver when users can invoke it.

Fixtures, native simulations, ProgramTest, local validators, and devnet are
different evidence levels. Record which one actually ran.

### 9. Frozen build and devnet release

1. Commit a coherent revision.
2. Run locked metadata and touched checks.
3. Generate reference artifacts with convergence from a detached clean tree.
4. Capture frames for every changed SBF dependency closure.
5. Build all eight production ELFs from that commit on hbox.
6. Record hashes and requote deploy rent.
7. Deploy all eight with fresh identities, abandoning cohort 17.
8. Record every program id, signature, hash, and poststate.
9. Found the public market and start the reciprocal simulator.
10. Update bindings, cut, and redeploy the existing Pages app.

Standing authority in `AGENTS.md` covers the conditioned fresh full-cohort
devnet deployment and publication cuts. It does not cover mainnet, tags,
releases, force pushes, or arbitrary branches.

The last recorded deployer balance was 26.572399090 devnet SOL. An older
eight-ELF rent quote was about 41 SOL before fees and market funding. Requote
the final ELFs. A prior faucet request was unanswered; no faucet action occurred.

## Dirty-tree ownership

| Slice | Main unfinished paths |
| --- | --- |
| Aquarium | simulator/aquarium, tests, Direct producer |
| Direct CLI | trade command, filesystem journal, shared SDK finalizer |
| General runtime | Claims composition, Trading Hot, Result rent, matched helper |
| General clients | WASM, discovery/operator, Rust CLI, browser and corpus |
| Structured | planner, WASM, driver/acquisition, browser, ALT provisioning |
| Dealer | discovery/projection, Trading equity, SDK recovery, workspace |
| Series | later producers and current-source recurrence |
| Formatting | many small unrelated Rust diffs; classify byte-for-byte |

`Cargo.toml` and `Cargo.lock` are dirty because unfinished Structured WASM
and Dealer work add dependencies. Treat them as semantic changes.

## Traps

- Build through `swarm-build` on hbox, never bare `taskset`.
- One checkout gets one Cargo target; never share across source trees.
- `rsync -a` once preserved stale mtimes and caused Cargo to reuse stale
  libraries. Sync changed files by checksum without preserving changed mtimes.
- `swarm-build` can exit zero after doing nothing when a scope name is loaded.
- Filter ProgramTests and use `--test-threads=1` for readable logs.
- Solana transaction CU, protocol frame bounds, scalar-bank widths, and
  measured ratchets are separate constraints. Measure before changing one.
- Generate references only from committed clean source.
- Commit exact paths through `tools/lane.sh`; cut via `tools/cut.sh`.

## Recommended sessions

Session 1: let cleanup finish; isolate the Structured compile interference;
build and execute Direct mixed-prestate cases; run fresh reciprocal A/B trades.

Session 2: finish aquarium rotation/replenishment/restart and Direct CLI buy;
commit/cut each slice and capture the Trading frame.

Session 3: close General's accepted matched chain, then Structured's four
actions. Keep client work behind runtime proof.

Later: Dealer multi-provider, Series recurrence, the accepted-route matrix,
all-eight freeze/build/frames, funding, fresh devnet, aquarium, bindings, and
final Pages publication.

The next executable action is the focused Direct SBF test and fresh reciprocal
validator run from `9ae8e0dab`, after isolating the dirty Structured compile
interference.
