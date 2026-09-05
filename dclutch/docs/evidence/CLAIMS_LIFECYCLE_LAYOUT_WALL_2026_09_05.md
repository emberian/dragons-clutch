# The `claims-lifecycle` wall is the Claims link's layout — 2026-09-05

Lane SUITES-REPAIR. Measured on hbox (Linux x86_64, `cargo-build-sbf` 4.0.0,
platform-tools v1.53, rustc 1.89.0 — the host the Token-2022 v11 fixture is
canonical on), from clean `git archive` exports, each row run alone with
`--test-threads=1` and a name filter so no program log interleaves.

## The claim this replaces

`docs/evidence/SIMPLIFICATION_CONVERGENCE_2026_09_04.md` §7.3 and §8 list
`claims-lifecycle` among **the convergence's** three suite reds, with "no
control could run (its lock was stale at the base), so the column is claims
until one does". The control runs now, and it refutes the column: the row is
red at the base and red at the commit that wrote it. Nothing the convergence
did is on this path.

## The one failing row

`real_token_2022_lifecycle_refuses_ata_substitution_and_rolls_back_every_late_failure`
in `programs/dclutch-claims-sbf/program-test/rational-lifecycle/tests/lifecycle.rs`.
The honest `RetireCoordinate` that follows the nonzero-supply hostile refuses
`RationalLifecycleSbfErrorV2::Token` (0x5216) at 79,632 CU of 1,364,073, raised
inside Claims at CPI depth two with **no Token-2022 invocation in that
transaction**; the caller propagates the same code. The other four rows pass.

## The eight controls

| # | tree | change | verdict |
| ---: | --- | --- | --- |
| 1 | HEAD (23fda4edf) | the row alone, unmodified | FAILED at `assert!(accepted)` |
| 2 | 330bbfaba (main, pre-swarm) | the suite, `--locked` dropped because that workspace's lock was stale | 4 passed, 1 failed — the same row, the same code |
| 3 | d6c4dea63 (the commit that ADDED the hostile, 2026-08-27) | the suite | 1 passed, 1 failed — the same `assert!(accepted)`, then at line 1370 |
| 4 | HEAD | the whole hostile block deleted | PASS |
| 5 | HEAD | the hostile's Mint mutation and restore KEPT, its `submit` deleted | PASS |
| 6 | HEAD | no hostile, two `get_new_latest_blockhash()` before the retirement | PASS |
| 7 | HEAD | no hostile, 5,000 lamports taken off the fee payer | PASS |
| 8 | HEAD | the hostile intact, a fresh blockhash before the retirement | FAILED |

Rows 4–7 exclude the account mutation, slot drift and the fee; row 8 excludes a
duplicate transaction signature. What every failing row shares and every passing
row lacks is **one extra executed-and-refused transaction**.

## What it actually depends on

| # | tree | change | verdict |
| ---: | --- | --- | --- |
| 9 | HEAD | all eleven `RationalLifecycleSbfErrorV2::Token` sites in `rational_lifecycle_v2.rs` given a `sol_log` marker; Claims relinked | **PASS**, with only `rl-token-5` (line 1194) firing — the hostile's own refusal |
| 10 | HEAD | the instrumented source reverted, Claims relinked (`dclutch_claims_sbf.so` sha256 `e20643ec7245a2aadf4160900b9433f56f678917a75375bc2a9d1484d1d5c42b`) | FAILED again |
| 11 | HEAD | ONE `sol_log` added to line 1287 alone — a branch this transaction never takes — Claims relinked | **PASS** |

**One `sol_log` on a never-executed path flips a refusal into a commit.** The
refusal is a property of the Claims ELF's layout, not of the request, the
account frame or the test. No `cargo-build-sbf` stack-frame-overwrite
diagnostic is emitted for either link (`grep -c 'overwrites values in the
frame'` is 0 for both), so the compiler does not say it.

## What is owed, and to whom

The claims column, for cohort-17.

1. **`RationalLifecycleSbfErrorV2::Token` is one code over eleven sites**
   (`rational_lifecycle_v2.rs` lines 1151, 1153, 1165, 1174, 1194, 1206, 1217,
   1245, 1260, 1284, 1287), none carrying a `msg!`. Splitting the discriminant
   is the tree's own prescription and is the first step: it names which conjunct
   refuses without a diagnostic relink — and note that the relink is exactly
   what perturbs the result, so a marker build is not a safe instrument here.
2. **Then the layout dependence itself.** A route whose verdict moves with an
   unexecuted log call has a defect the split will localize but not explain.
   Until it is explained, no measurement of this route — CU, refusal code or
   acceptance — is a fact about the protocol rather than about one link.
3. `d6c4dea63` landed this suite red on 2026-08-27 and its message states the
   hostile's measured refusal without stating the suite's verdict; the
   `claims-lifecycle` row joined `tools/gate suites` later (30398e3f8), so
   nothing ran it in between.

The suite's `assert!(accepted)` was a bare boolean; it now names this document.

---

## ADDENDUM, 2026-09-05, same lane, same day: the verdict above is WITHDRAWN

**Rows 9, 10 and 11 are single draws of a coin flip, and so are rows 4 through
8.** The row is NONDETERMINISTIC. Measured after the fact, the same test binary
against the same `dclutch_claims_sbf.so`
(`e20643ec7245a2aadf4160900b9433f56f678917a75375bc2a9d1484d1d5c42b`), the same
tree, run eight times in a row one thread at a time:

| tree | draws |
| --- | --- |
| HEAD (23fda4edf) | **4 of 8 pass**, 4 fail |
| 330bbfaba (main, pre-swarm) | **3 of 8 pass**, 5 fail |

So no single-run comparison above supports anything. "One `sol_log` on a
never-taken branch flips it" is not a finding; it is one head after one tail.
The instrument was a coin and every row measured with one toss is void. What
survives is only what many draws say, and what they say is:

- **The row has been flaky since before the swarm**, at about the same rate at
  330bbfaba as at HEAD. It is not the convergence's — that part of the original
  verdict stands, and on a stronger footing than the single control it rested
  on.
- **`d6c4dea63` (2026-08-27) is still the commit that made this reachable**: it
  added the nonzero-supply hostile immediately before the honest
  `RetireCoordinate`, and its message states the hostile's measured refusal
  without stating the suite's verdict. Whether the flake is *caused* by that
  hostile is now UNKNOWN: rows 4–7 removed it and passed once each, which four
  draws at p≈0.5 barely distinguish from luck.
- **When it fails**, the honest `RetireCoordinate` refuses
  `RationalLifecycleSbfErrorV2::Token` (0x5216) at 79,632 CU with no Token-2022
  invocation in that transaction. The refusal code is stable across draws; only
  whether it happens is not.

### The hypothesis worth testing first, and how

`ProgramTest::start_with_context` mints a **fresh random payer keypair every
run**. It is the only input to this suite that changes between two runs of one
binary against one ELF, so every address, ordering or comparison downstream of
`context.payer.pubkey()` changes with it. A near-even split is what a single
bit-level comparison on a random key looks like. The probe is to record the
payer per draw and correlate it with the verdict — twenty draws will either
show the split tracking a property of that key or rule the payer out, and it
costs one `println!` and four minutes.

Until that is done, **nothing measured on this route from a single run is
evidence**: not a CU figure, not an acceptance, not a refusal code. Any lane
that touches it owes N draws and a rate, never a verdict.

### What is owed, corrected

1. Find the source of the nondeterminism (start with the payer).
2. Only then, split the eleven-site `RationalLifecycleSbfErrorV2::Token`
   discriminant so a failing draw names its conjunct. The earlier note that a
   marker relink is "not a safe instrument" was itself an artefact of the coin:
   a relink is fine, N draws are what was missing.
3. `tools/gate suites` reports this row as a pass or a fail from ONE run, so
   the `claims-lifecycle` tier verdict is itself a coin flip today. That is the
   most expensive consequence and it is not fixed here.

---

## SECOND ADDENDUM, 2026-09-05, lane SUITES-2: the variable is the BLOCKHASH, and the row is fixed

Both verdicts above are now closed. The row is deterministic and green, 8 draws
of 8, and the whole suite is 5 of 5 on three draws. Measured on hbox (Linux
x86_64, `cargo-build-sbf` 4.0.0, platform-tools v1.53) from a `git archive`
export of `9fe4506f2`, each draw `--test-threads=1` with a name filter.

### The payer was not it

The addendum above named `ProgramTest`'s per-run random payer keypair as the
first suspect. It is refuted. With the payer left random, 3 of 8 draws passed;
with the payer PINNED to one fixed keypair — the same address, printed each
draw, funded by `set_account` before anything else runs — **3 of 8 draws passed
again**, the same rate. One binary, one ELF
(`e20643ec7245a2aadf4160900b9433f56f678917a75375bc2a9d1484d1d5c42b`), sixteen
draws. Nothing downstream of `context.payer.pubkey()` decides this row.

### What it is

The honest `RetireCoordinate` and the nonzero-supply hostile immediately before
it are **the same instruction** — the hostile submits `retire_coordinate.clone()`
against a Mint whose supply the test wrote, and the honest one submits
`retire_coordinate` against the Mint restored. Same instruction, same payer,
same lookup table. So on the **same recent blockhash** they compile to the same
v0 message and sign to the same signature, and the bank refuses the second as
`AlreadyProcessed`: no program invocation, no compute units, no refusal code,
and `metadata.log_messages` empty.

`ProgramTest` registers a new blockhash from a background task on a **wall-clock
timer** — `target_tick_duration` 100µs × 64 ticks per slot = 6.4 ms
(`solana-program-test-4.3.0-beta.2/src/lib.rs:1066`, `:1287`). The two
submissions land about 4 ms apart. Whether a tick fell between them is the coin.

Instrumented, printing every submission's blockhash, signature and error, 8
draws:

| draws | the honest retirement's blockhash | verdict |
| --- | --- | --- |
| 5 of 8 | **identical to the hostile's**, same signature, `err=AlreadyProcessed`, `logs=0` | FAILED |
| 3 of 8 | a later blockhash, own signature | committed |

### What the first verdict actually saw

`RationalLifecycleSbfErrorV2::Token` (0x5216) at 79,632 CU "with no Token-2022
invocation" is real, and it is **the hostile's own correct refusal** — Claims
observing a Mint whose supply is not the zero the request declared, in
`authenticate_closeable_mint`, before any CPI. It was read off the interleaved
tail of the log and attributed to the transaction that never ran. The honest
retirement's refusal has no code at all, because it never reached the program.

### Fixed at the author

`submit` in
`programs/dclutch-claims-sbf/program-test/rational-lifecycle/tests/lifecycle.rs`
now takes a blockhash strictly newer than the one visible on entry, so every
submission is its own transaction. Two of this suite's requests are identical by
construction — `replay_receipt` IS `activate_receipt.clone()` — so this also
means the replay row is now refused by Claims's own replay guard rather than
possibly by the ledger's signature dedup. The cost is
`get_new_latest_blockhash`'s 200 ms poll per submission: the row went from 0.45 s
to **2.7 s**, the five-row suite to **6.0 s**.

And the eleven-site `Token` discriminant is split, which is what would have
caught this on the first reading: the hostile's refusal now names
`MintProfile` (0x5219) rather than sharing a code with six CPI sites. See the
`RationalLifecycleSbfErrorV2` doc comments for the four new causes.

### What this leaves

Nothing owed on this row. The general lesson is the tier's, not the row's:
`tools/gate suites` drew each row once, so this row's verdict here was a coin
flip for nine days. It now draws each row three times and reports a row whose
draws disagree as NONDETERMINISTIC by name.
