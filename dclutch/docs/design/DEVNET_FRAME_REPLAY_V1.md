# Replaying a devnet frame: how to convict a coarse refusal in an hour

Owner: lane PROGRAMS-17D, written 2026-09-06 at `/Users/ember/dev/dclutch`.
This is a recipe, not evidence. The run it was written from is the cohort-16.1
`OpenBatch` refusal, whose two walls are recorded in the dated addendum to
`docs/evidence/COHORT161_UPGRADED_SEALED_2026_09_05.md`.

## The gap this closes

`TradingSbfError::Content` is one wire code over ~1,700 sites. The tree has an
instrument for exactly that — `hot_cu_checkpoint!`, `hot_cu_reason!` and the
`dclutch-hot-why:` loggers, all behind `--features hot-cu-profile` — and until
now it could not be pointed at a devnet refusal, because **a release pins the
ELF digest and the deployment slot, so an instrumented Trading refuses
`Release` before it can print a checkpoint.** Cohort-16.1 recorded "could not be
localized" for that reason.

It is only a wall on a chain. A hot route **never hashes an ELF**:
`crates/dclutch-trading/src/shadow_accelerator_auth/deployment.rs:97` takes the
activation-bound arm, and `slot_pinned_release_elf_digest_v1`
(`crates/dclutch-registry/src/immutable_registry.rs:439`) compares the
ProgramData header's **deployment slot** and **upgrade authority** and then
returns the release's own recorded digest. The pins therefore live entirely in
the 45-byte Loader V3 header, and the ELF behind it is free to be the profiled
build.

## The instrument

`programs/dclutch-trading-sbf/program-test/devnet-replay` — one binary,
`dclutch-devnet-frame-replay`, that loads a capture into `ProgramTest` and calls
`simulate_transaction`, which is what a chain preflight does: no signature, no
blockhash age, no fee. It replays the transaction's own wire bytes.

    dclutch-devnet-frame-replay --capture CAPTURE_JSON \
        [--programdata-elf KEY=PROFILED_SO] [--programdata-slot KEY=N] \
        [--set-account KEY=FILE] [--expect-units N] [--expect-error TEXT]

## The recipe

**1. Capture.** From the driver's own evidence, not by hand: the executed plan
carries `transactionBase64`, which is the exact unsigned packet. Resolve its
address lookup tables, read every account it names at `finalized`, and write a
`dclutch-devnet-frame-capture-v1` document: `transactionBase64`, `warpSlot`
(the chain slot the route observed), and a `state` map of
`{lamports, owner, executable, rentEpoch, dataBase64}` per address, plus the
lookup tables themselves. Absent accounts are simply absent.

**2. Reproduce the code and the units before believing anything.** Run the
replay against the **deployed** ELFs first. It must reproduce the chain's
refusal code. The compute units will be close but not equal — the harness bank
and the cluster do not price every syscall alike — so treat the code and the
phase as the reproduction and the units as corroboration.

**3. Two harness facts that will otherwise waste an hour.**

- *Every loaded program's deployment slot must be one number.*
  `ProgramCache::extract` admits a program only when its deployment slot is at
  or below the cache root or the fork graph calls it an ancestor, and
  `BankForks::relationship` answers `Unknown` for anything below the root.
  `ProgramTestContext::warp_to_slot(w)` roots the fork at `w - 1`. The two
  together force one deployment slot D and a bank at D + 1. When they disagree
  the transaction dies `ProgramCacheHitMaxLimit` with **zero logs and zero
  units**, which reads exactly like a program that refused nothing.
  Equalize with `--programdata-slot`, and move each release's own pin to match
  (`--set-account` on the Registry activation cache, whose per-role
  `ArtifactReleaseV1` bodies carry `deployment_slot` at a fixed stride).
- *An address lookup table's `last_extended_slot` must be at or below the bank
  slot*, or address resolution refuses before the program is reached and the
  result carries no simulation details at all. `--set-account` the table with
  that field zeroed.

The chain slot the program reads is restored on top, as the **Clock sysvar**
(`set_sysvar`, not a bare account write: `Clock::get()` is a syscall over the
bank's sysvar cache). So the bank sits where the loader needs it and the
program reads the slot it read on chain.

**4. Then instrument.** `--programdata-elf TRADING=<profiled .so>` replaces the
ELF tail and keeps the 45-byte header, so every release pin still holds. Read
the checkpoint that precedes the refusal, add checkpoints inside that span,
repeat. Three passes took the cohort-16.1 refusal from "1,686 sites" to one
line.

**5. Change one input and watch the refusal move.** That is the red proof. The
cohort-16.1 conviction was finished by swapping one address in the lookup table
and seeing the same frame walk 65,000 units further into a different wall.

## Keeping the diagnostics in the tree

A statement-form `hot_cu_checkpoint!("x")` costs the shipped link nothing —
`macro_rules!` never expands the arguments of an empty arm. A
`.map_err(|_| { log(); Coarse })` does **not** have that property: the closure
is real code whether or not the log is. Use the wrapping forms
(`hot_cu_watch_lifecycle!`, `hot_cu_watch_reason!` in
`programs/dclutch-trading-sbf/src/hot_v3.rs`), which expand to the wrapped
expression itself without the feature — so a localization can be left in the
tree permanently instead of being applied and reverted every time a coarse code
has to be convicted.

Adding lines still moves the ELF: SBF release builds carry `file:line` panic
locations, so any source edit to a linked crate changes the digest. That is the
frames ratchet's business, not a reason to keep the instrument out of the tree.

## What the recipe cannot do

- It cannot execute where the frame refuses; it reproduces refusals, not
  successes past them.
- It substitutes deployment slots, so a `ReleaseSuperseded` or
  `DeploymentSlotMismatch` after `--programdata-slot` is the instrument
  talking. `authenticate_deployment_v2`'s `_ => Content` arm folds
  `DeploymentSlotMismatch` into `Content`
  (`crates/dclutch-trading/src/shadow_accelerator_auth/deployment.rs:103-108`)
  even though both enums carry that discriminant — so a substituted slot can
  wear the very code you are chasing. Restore the true slot before believing a
  `Content` inside a deployment authentication.
- Program-test enables its own feature set. Do not publish a CU figure from it.
