# Start here — 2026-09-01

You are picking up a tree that two swarms handed off in sequence. Everything
you need is written down; none of it needs archaeology. This page exists only
to put the documents in order and to carry two facts that landed after the
handoff letter was written.

## Read in this order

1. **`AGENTS.md`** — standing authority. Safety, provenance, architecture,
   refusal-code law, kernel policy, project conduct. It overrides older
   handoffs.
2. **`docs/MASTER_COMPLETION_CONTRACT.md`** — the stopping condition and rows
   C-00..C-16. Note the vocabulary clause: an ambition is *implemented with
   evidence* or *explicitly ruled out by ember, dated*. There is no third
   state, and inventing one ("deferred", "out of scope", "future work") is the
   single most common way this project gets quietly smaller.
3. **`docs/LETTER_TO_CLAUDE_2026_09_01.md`** — the live frontier. Five named
   walls, each with the file, the line and the exact command that reproduces
   it, plus a full lane registry (S0–S11) and continuation order. This is your
   queue.
4. **`docs/LETTER_TO_CODEX_2026_08_31.md`** — the epistemics the tree runs on.
   Shorter and less operational, but it is where the working method is
   argued rather than assumed.

`GOAL.md` and `WAVE.md` are deep historical ledgers. Read them for *why* a
thing is the way it is; do not treat either as a current queue.

## Two facts added after the letter

**1. Devnet is disposable (ember ruling, 2026-09-01).** The never-executed
`ProtocolInfrastructureProfileV1 -> V2` succession ceremony is *not* a
prerequisite for a checked release candidate or for the next devnet flight.
Tear down, redeploy fresh from exact current sources, abandon the cohort-8
devnet programs in place rather than migrating them; there is enough devnet
SOL to stand the new set up without tearing the old one down first. The
ceremony code stays — it is what a non-disposable deployment will need — but
it is demoted from blocker to capability, and nothing may report it as
executed until it actually runs. Recorded in `WAVE.md` under 2026-09-01.

**2. The public cut is current.** `dclutch/` in the `dragons-clutch` repo was
synced to live `7dc20ad0` and pushed to `main` as `7509c998b`. The credential
sweep ran as value tests before the push and passed. You do **not** need to
publish anything, and you do not have publication, push, tag, deploy or
mainnet authority — those require ember naming the act.

## Public CI at the cut — read before you touch anything

The cut ran the public gates and they are red. Every red below is already
diagnosed; none of them needs rediscovery, and two of them are not defects at
all. Do not treat "CI is red" as a licence to relax a gate.

**Fixed already.** `the journey campaign compiles` — the `#[path]` tripwire in
`tools/gauntlet/journey/src/main.rs` firing for the second time. The overnight
wave grew `campaign.rs` and `market.rs` call sites into `crate::chaos_fault`
and `crate::infrastructure_succession` and added a
`dclutch-source-readiness-operator` import, and the journey's linked subset
fell behind. Relinked per the file's own written protocol (link, never fork),
verified locally green, commit `673fcb3e`.

**Not a defect — an honest missing prerequisite.** The `claims` row reports
`DID NOT RUN`. It needs `cargo-build-sbf`'s own crate archive in the registry
cache to authenticate its audited Token-2022 fixture, and a runner that
installs Agave from the release tarball never has one. That is a host fact.
The row is *supposed* to say so rather than fake a verdict. Leave it.

**Not a protocol defect — CI infrastructure.** `web + SDK test suites` fails
because `lib/sourceReadinessWasmParity.test.ts` shells out to
`cargo run -p dclutch-source-readiness-operator` from a Node-only job, and
rustup cannot install toolchain 1.97.1 over the runner's preinstalled one
(`detected conflict: 'bin/cargo'`). Either provision the toolchain in that job
or give the test the same honest missing-prerequisite exit the claims suite
uses — but do **not** make it skip silently, which converts a wall into a
fake green.

**Real, and the most important thing on this page.** `SBF program-test suites`
is 2 of 7 red, and one of them built, ran, and then failed a case — which the
workflow's own guidance says to treat as real:

- `postjoin: FAILED` on `real_registry_executes_profile14_direct_hot_under_protocol_limit`
  (0 passed, 1 failed, 26 filtered out). Rerun with
  `--test registry_hot_continuation`. The name says protocol limit, and
  `SBF programs and the Direct compute margin` is red beside it, so the
  working hypothesis is that the wave grew the Direct hot route past a
  ceiling. Measure before you convict: that gate moved +6,876 CU on 08-31 and
  every named suspect was refuted before the true cause confessed.
- `dealer: FAILED` — establish first whether it ran any tests at all. A row
  that fails before its first assertion is a wiring defect, not a finding.

**Seam audit.** `repository hygiene` reports about ten new
`UNSET_GUARD_PRESENT` records — these are *positive* facts (a file does refuse
the unset pubkey, recorded so the gate fails if the last such guard is ever
deleted) and they need baseline entries with written verdicts. Never
`--write` the seam register. One finding is different and is a real question:

- `UNSET_PUBKEY_UNGUARDED` at
  `programs/dclutch-trading-sbf/src/dealer/v4_lp_accelerator_accounts.rs:196`,
  `authenticate_position` — authenticates coordinates against wire pubkeys
  with no guard refusing the all-zero one. I read it and did **not** reach a
  verdict. It may well be benign-typed, because in that function the all-zero
  pubkey is legitimately meaningful: it *is* the System program ID, which the
  code checks against deliberately. Settling it means reading the upstream LP
  admission path to see whether an attacker-chosen all-zero `lp_owner` or
  `child_root` can reach the derivation. Fix or write a verdict; do not
  assume my guess.

## The rules that bite hardest

- **The dirty tree is deliberate.** Several campaigns stopped at honest walls
  and their work is preserved uncommitted with digests recorded in the letter.
  Never `git stash`, never `git reset`, never `git clean`, never
  `git add -A`. Commit exact named paths through `tools/lane.sh`, and inspect
  the full staged list first — other lanes share this checkout.
- **The local Mac disk is full.** Route heavy builds to `hbox` and always
  through `swarm-build` (it enforces a memory cap; `taskset` alone does not,
  and this box has been OOM'd into a physical power-cycle before). hbox is
  co-tenant — keep waves small. Do **not** delete anything under `~/dev` to
  make room; that is ember's to do.
- **Never run an unfiltered `-p <crate>` test suite**, locally or on hbox.
  Filter with `-E 'test(<pattern>)'` or an explicit `--test` list, and state
  your control separately. Run the narrowest thing that could refute you.
- **Refusals are named, never numeric.** A bare `is_err()` is a test of
  nothing: it passes on whatever the transaction refuses first. Assert the
  exact discriminant derived from the enum, and prove the test red before you
  trust it green.
- **Never weaken a check to make progress.** The gate may move when the law it
  guards is re-proven on the other side; the law may not. If there is truly no
  construction that extends instead, that is a ruling, and rulings go to ember.

## What good looks like here

A green constructor is not physical completion. A fixture, a simulation and a
local-validator run are three different evidence levels, and none of them is
devnet; devnet is not mainnet. If you cannot prove a thing, land the largest
piece you *can* prove and name the remainder precisely — a named wall is the
most useful artifact in this repo, and the letter you are about to read is
made almost entirely of them.
