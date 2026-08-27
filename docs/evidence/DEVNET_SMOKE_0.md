# DEVNET SMOKE-0 — the first real devnet run, and the four walls it found first

**Date: 2026-08-27. Lane: SMOKE-0. Status: COMPLETE — run closed, every
reclaimable lamport recovered. Every number in this file is measured, not
predicted, unless it says otherwise.**

**The one-line verdict: the tires hold — the wheels aren't attached yet.**
Deploy mechanics, byte verification, observed slots, the recycle window, the
budget arithmetic, and the fail-closed rails all held on the real cluster, at
a total cost of 0.0096 SOL. What does not exist yet is the road from a
deployed program set to a living market: the life requires revocation
(permanent ~31.7 SOL), the devnet Pyth release row is unminted, and no driver
in the tree can speak to a non-loopback cluster (§2). Those are SMOKE-1's
preconditions, each with an owner (§6). Two fell the same evening: ember
ruled W1 live (iteration over full trustlessness — decision 0012 carries the
design), and the devnet `PythReleaseV1` row was minted under deputization
(`crates/dclutch-pyth-svm/src/devnet.rs`, commit `11f249ff`).

Charter: the scope-limited first phase of `WAVE.md`'s DEVNET-SMOKE — a
recyclable single-market tire-kick on devnet, authorized by ember for exactly
this scope. Repo commit at execution: `b6e28707829dc41330ac12051e9408b1b44b791e`.

The verdict structure of this document: [§1](#1-preflight) is the preflight
that every later decision rests on; [§2](#2-the-four-walls) is the finding
that reshaped the run — four walls, all measured **before any lamport moved**;
[§3](#3-the-mutable-dress-rehearsal) is what was executed on devnet;
[§4](#4-wallet-arithmetic) is the money; [§5](#5-frictions) the frictions;
[§6](#6-smoke-1-deltas) the go/no-go deltas for SMOKE-1.

---

## 1. Preflight

### 1.1 The wallet

| fact | value |
|---|---|
| deployer keypair | `~/jobs/dragons-clutch-devnet-20260819/keys/deployer.json` |
| address | `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` |
| balance at start | **65,000,000,000 lamports (65 SOL exactly)** |
| cluster | devnet, genesis `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` |
| RPC | `https://api.devnet.solana.com` (public; rate-limit observations in §5) |
| local CLI | solana-cli 4.0.2 (Agave), devnet solana-core 4.2.1 |

The charter said ~55 SOL; the measured balance is 65. The gen-1 Track-C
programs from the 2026-08-19 job read **fully absent** on devnet (both id and
ProgramData accounts gone, 0 lamports) — `collect-sol.sh` had already
reclaimed them, and notably **no 36-byte Program residue survived there**,
which §3 re-measures for the current CLI (`devnet-recycle.sh` counts
0.00114144 SOL per closed program as stranded; the canary checks whether that
is still true).

### 1.2 Devnet Pyth — reproduced exactly, plus a live watch

`tools/release/devnet-observe.sh --cadence` (12 bounded read-only calls,
logged): **every fact pinned by `fixtures/pyth/upgraded-2026-08-26/
PROVENANCE.md` still reproduces** — receiver `rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp`
(ProgramData `3UV7w2yT…`, slot 460,336,311), router `HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL`
(slot 460,336,290), push oracle `pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou`,
all three still under upgrade authority `upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr`
(the 3-of-5 Pyth multisig; guardian set cardinality 5, `minimum_signatures` 3,
five key bodies unchanged). SOL/USD `PriceUpdateV2` at
`7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE`, `verification_level = Full`,
price 108.906774 at read, 312 s old.

Devnet rent re-read live from the cluster: `min_balance(n) = 890,880 + 6,960·n`,
affine, exact at 10⁶ — unchanged from the runbook.

**Cadence** (the §12.3 input the charter asked for):

| source | window | p50 | p90 | p99 | max |
|---|---|---:|---:|---:|---:|
| historical page (999 gaps) | 86.0 h | **313 s** | 321 s | 325 s | **4,784 s** |
| live watch (this run) | 11 min | two gaps observed, **313 s and 313 s** | | | |

The live watch (20-second polls of the `PriceUpdateV2` account,
21:43–21:54Z) caught three publications: 21:40:24 → 21:45:37 → 21:50:50Z.
An earlier watch attempt also caught a 314 s gap before deploy traffic
starved its polls (§5 friction 1). Three live-confirmed gaps at 313–314 s
against the 86-hour page's p50 of 313 s: the charter's ~10-minute
measurement agrees with the archival profile exactly.

### 1.3 Window width from the measured cadence

`MAINNET_STATE_RELAY.md` §12.3 (TWIN): under the stated-as-approximate Poisson
model with mean ~313 s, `P(≥1 publication in W) = 1 − exp(−W/313)`:

| width | cadences | coverage |
|---|---|---|
| 313 s | one | ~62% |
| **1,252 s (~21 min)** | **four** | **~98%** |
| 1,800 s (30 min) | ~5.75 | ~99.7% |

Guidance followed: **≥ four cadences**; 30 minutes for a market that should
not fail for provider reasons. `max_age_seconds` is a separate submission-
latency budget, and the 86-hour page reconfirms the known 4,784 s outage
(2026-08-25), so a bound below that either tolerates refusal-for-79-minutes or
leans on the funded failure path — exactly as the runbook §5.3 records.

### 1.4 The checked-release candidate — **the gate is RED at HEAD**

`tools/release/checked-release-candidate.sh --commit b6e28707` **REFUSED**:

```text
BUILD DIAGNOSTIC: dealer-accelerator emitted 82 SBF stack-frame overwrite reports
... _ZN19dclutch_trading_sbf6hot_v323execute_child_routes_v3... overwrites values
    in the frame ... may cause undefined behavior during execution.
refusing: 82 SBF build diagnostics
```

All 82 are the **dealer-accelerator's** monomorphization of Trading's
`hot_v3::execute_child_routes_v3` — the same diagnostic class the
2026-08-26 candidate evidence recorded as *fixed* (0 diagnostics at
`3b0c5883`). It has **regressed** under the W2-era hot_v3 churn. The seven
role builds emitted zero diagnostics; their ELFs are what §3 deploys.
**Deploy-day checklist item 1 (`sbf_build_diagnostics_total = 0`) is red at
HEAD** and owned by whoever owns the accelerator/hot_v3 seam.

Role artifacts at `b6e28707` (all smaller than the runbook's `3b0c5883` table
except custody and resolution):

| role | ELF bytes | sha256 (prefix) | predicted ProgramData rent |
|---|---:|---|---:|
| registry | 206,880 | `e7076e4e` | 1.441088880 |
| rent | 137,392 | `e63e6e80` | 0.957452400 |
| custody | 360,128 | `b80ece53` | 2.507694960 |
| resolution | 588,176 | `cab90b3f` | 4.094909040 |
| claims | 1,009,032 | `e0786c79` | 7.024066800 |
| trading | 1,324,688 | `d862d3fd` | 9.221032560 |
| core | 933,328 | `fa1873ce` | 6.497166960 |
| **seven roles** | **4,559,624** | | **31.743411600** |

Prioritization fees read from the cluster immediately before deploying: **all
zero across the recent-fee page** — no priority fee paid anywhere in this run
(re-checked in §5 if any transaction stalled).

---

## 2. The four walls

All four were found by reading the code at HEAD against the charter — before
any transaction was signed. Per the charter's own rail: a tire-kick that
surfaces a devnet-specific wall and stops cleanly is a success.

### W1 — the life and the recycle are mutually exclusive, by protocol design

The charter requires both "THE LIFE at N=4 … RESOLUTION … redemption …
retirement" and "the run must END with programs closed and rent recovered
(recycle is NOT optional)". At HEAD these cannot both happen:

- The market life runs at 87–91% of the 1,400,000 CU ceiling **only because
  of the immutable fast path**: `cached_role_deployment_observation_v1`
  (`crates/dclutch-registry-activation-auth-v1/src/lib.rs:243`) reuses the
  activation-bound ELF digest **only** for `Immutable` releases; an
  `ExactAuthority` release re-hashes its full ELF at **every** authenticated
  action (measured hashing cost 0.53–0.66 CU/byte, runbook §3.3 dry run —
  Trading alone ≈ 700k CU). A mutable deployment cannot execute
  `Found31`/`DCLTGMF1` under the ceiling.
- Core infrastructure init hard-requires `Immutable` for Registry and Rent
  (`programs/dclutch-core-sbf/src/infrastructure.rs:281`), and
  `immutable_release_elf_digest_v1` (`crates/dclutch-registry-contract/src/
  immutable_registry.rs:384`) refuses any observed upgrade authority.
- The run-spec producer mints **only** `Immutable` releases
  (`tools/local-validator/bootstrap/successor/src/plan.rs:760`) and refuses
  at plan time an observed ProgramData that still carries an authority.
- And an immutable Loader V3 program **can never be closed** — measured by
  the runbook (§2.5, §4.3) and enforced by `devnet-recycle.sh`.

So: the life requires revocation; revocation makes ~31.7 SOL permanently
unrecoverable; the recycle rail forbids that. **This is the runbook §4.3
sentence — "the protocol's correctness condition is the same event as the
loss of the money" — meeting a charter that asked for both.** Which half wins
is ember's call (principal + deploy + scope), yielded as a question in §6
with a recommended answer.

### W2 — the devnet PythReleaseV1 row still does not exist

`dclutch_pyth_svm::release::PRODUCTION_RELEASES` is `[PythReleaseV1; 0]` at
HEAD, and the producer imports only `local_validator_release_v1`
(`plan.rs:11`). Runbook §8's checklist says this precondition is open and
owned by the Pyth adapter lane. Every fact the row needs is pinned and was
re-verified live in §1.2. Without it, a devnet market **cannot resolve
against the real receiver** — stage 3/4 of the charter has no release row to
bind, independent of W1.

### W3 — there is no devnet driver for the campaign

The successor supervisor refuses any RPC origin that is not `127.0.0.1`
(`runtime.rs::rpc_origin`, :1196–1210) and *launches its own guarded localhost
validator* — it cannot be pointed at devnet. The charter's "per-role
`--keypair` flows" **do not exist**: the only keypair affordance is
`--keypair-seed`, TEST-ONLY and loopback-gated (`seed.rs:139`). The journey
runner — the producer of the N=4 life and the L1–L7 conservation ledger — is
loopback-bound the same way and *by design*: its founder key is ephemeral and
never persisted, so the life exists only in-process against the validator the
runner itself launches (`tools/gauntlet/journey/run-journey.sh:27`, README
"Not a fast lane"). Registry publication, Core init, activation, the 116-tx
founding ladder, the life, the conservation ledger — none of it has an
executable devnet path today. Only the Loader-level deploy mechanics (runbook
§2, `solana program …`) are drivable, and that is exactly what §3 executes.

### W4 — the deploy-day checklist is red beyond W1–W3

`sbf_build_diagnostics_total = 82` at HEAD (§1.4); `DCLTGMF1` has not been
re-measured at this commit (the gauntlet dry-run gate was last green at
`90d7688d`); and GIT-SCAN item 9's stale-blocker-C runbook correction is
still outstanding. Day-of items, but red today.

---

## 3. The mutable dress rehearsal

The maximal charter slice consistent with the inviolable rail: runbook §2
mechanics, **mutable** (upgrade authority = deployer, never revoked), canary
first — then `devnet-recycle.sh --execute` closes everything and the wallet
arithmetic closes the loop. Program ids are rehearsal-scoped (fresh keypairs,
stored with the job at `~/jobs/dclutch-smoke0-20260827/keys/`), **not**
durable protocol addresses.

**Scope revision after the first measurement** (recorded before executing
it): the canary showed the public endpoint moves buffer bytes at a pace that
prices the full seven-role ladder (~4.56 MB, ~4,500 writes) in **hours**, with
`Max retries exceeded` failures needing manual resume cycles. The remaining
stage-2 questions are answered by the two endpoints of the size range instead:
**rent (137 KB, smallest, `--use-rpc` path) end-to-end including close**, then
**trading (1.32 MB, largest, TPU path — an A/B against the RPC path)**, then
recycle. Deploying the middle five mutably would re-measure arithmetic the
cluster already confirmed affine, at ~2 more hours of straw-throughput and
~0.02 SOL of fees, and was cut for that reason. (Retrospective: the TPU
discovery in §3.1 post-dates this cut and would have made the five cheap in
*time* — but not in information, and not in transient exposure, so the cut
stands on those grounds.)

### 3.1 The canary (rent, 137,392 B, `--use-rpc`)

The buffer took **three passes**:

| pass | path | outcome |
|---|---|---|
| 1 | `--use-rpc` | 276 s wall, wrote ~71% (97,560 B), then `Error: Data writes to account failed: Custom error: Max retries exceeded` — the CLI exhausted its default 5 sign-attempts against expiring blockhashes while throttled |
| 2 | `--use-rpc` | resumed into the same buffer (`--max-sign-attempts 100`), killed externally mid-run (session interruption, not devnet) — on-chain state read back cleanly as 71% written both times, so a killed writer strands nothing |
| 3 | `--use-rpc` | resumed the remaining 29%; ground for **15+ minutes without finishing ~40 writes** (~3 tx/min effective); killed deliberately to convert it into the A/B below |
| 4 | **TPU** (no `--use-rpc`) | the same remaining 29%: **9 seconds, rc=0** |

**The A/B verdict inverts the runbook §2.2 note**: on today's devnet, TPU
submission is ~two orders of magnitude faster than `--use-rpc` through the
public endpoint, whose per-IP throttle is the binding constraint. (`--use-rpc`
still has its stated role when TPU egress is blocked — but it should be the
fallback, not the default, and §3.2 stress-tests TPU at 1.32 MB.)

The buffer account was fully allocated (137,429 B, 0.957396720 SOL rent) in
pass 1's first seconds — allocation is one transaction; the writes are the
slow part. Reading the partial buffer back and diffing against the ELF is a
reliable progress meter and costs one `getAccountInfo` — when the IP isn't
starved (§5 friction 1).

**Canary deploy → verify → close (all timestamps 2026-08-27T21:40Z):**

```text
buffer-verified  e63e6e80…3641172   (byte-exact after 4 passes)
deploy           rc=0  wall=5s   tx 2kGQhNE9VtsGDemNRMpAuuFWT8Sn2kkPEBLc3
                                    HzdqvvvQKkp1ae3Bs2LSgfbFyi4RNwVrhgizSQo7wwYHXGvGnKX
deployed-verified sha256(dump) == ELF, 137,392 B
deploy-delta     1,207,120 lamports   (36-byte Program rent 1,141,440 + fees;
                                       the buffer DRAINS into ProgramData in the
                                       same instruction — runbook §2.1 confirmed)
close            rc=0  wall=2s   returned 957,447,400 lamports
                                    (ProgramData rent 957,452,400 − 5,000 fee)
residue          Program account SURVIVES: 1,141,440 lamports, space=36,
                 executable=true  — devnet-recycle.sh's stranded-residue row
                 CONFIRMED on devnet
```

(§1.1 noted the gen-1 Track-C ids read *fully* absent — no residue. That
absence is now the anomaly, not the rule: this run measured the residue
surviving twice with solana-cli 4.0.2. The gen-1 case is left unexplained —
a different CLI era or a devnet ledger event — and the current measurement
is the operative fact.)

The program id `H29j7pBvETiuVEwwPBZBASUjxjLjyVFpXLyW8HNpDEAU` briefly held the
first successor-generation dClutch bytes ever executed-able on a public
cluster, rehearsal-scoped, and is now closed (the id is consumed — a closed
program id can never be redeployed; rehearsal ids are throwaway by design).
One tooling nit: `solana program show --output json` failed to parse in the
script (its output was not JSON at that moment); the deployment slot for the
canary was therefore not captured before the close. §3.2 captures trading's.
(Explained: `program show` demands a *default signer* even for a read — it
errored `No default signer found` because this machine's CLI config points at
a nonexistent local-validator payer. Pass `--keypair` to `show`/`dump` too.)

### 3.2 The stress case (trading, 1,324,688 B, TPU path)

```text
2026-08-27T21:40:36Z  write-buffer (TPU, no --use-rpc)
2026-08-27T21:40:59Z  rc=0  wall=23s        <- ~1,310 writes, one pass, no retries
buffer-verified  d862d3fd…dd2eb2 byte-exact, single pass
deploy           rc=0  wall=7s   tx 3XDgfY76Xppx6nPu2nzxhH2gF2siseAhEB45jfL9
                                    4LhcNxH1rdj4x8hGtkeRRcHYdcyipRYYLvcZfQ18LRWfr2rh
show             programdata AAgBqSEXGbTB2NuiLMhSszpdwGztzhh8F2XKTRzeKLCK
                 lastDeploySlot 489,051,220        <- a REAL observed devnet slot
                 authority 4zrxtw…vEwP (deployer; MUTABLE, kept)
                 lamports 9,221,032,560            <- §1.4's prediction TO THE LAMPORT
dump-verified    sha256 == ELF, 1,324,688 B
```

**The largest role artifact reached devnet, verified byte-exact, in 30 seconds
of transactions.** The §1.4 rent prediction matched exactly, the deployment
slot is nonzero and real (the input class blocker A's fix exists for), and the
ProgramData header hostile-parses as `tag=3 slot=489051220 authtag=1` — the
exact mid-deploy mutable shape the runbook §2.5 describes.

### 3.3 The observed-programdata rail, fed real devnet bytes

The full 1,324,733-byte ProgramData account body was read off devnet and
handed to `prepare --trading-observed-programdata` — the 993a9ec path that
mints a release from a cluster observation. Two findings:

- **The prebuilt release binary predates the flag** (`unknown prepare
  argument`) — the c5d791e-proven path had never been *built* on this machine;
  rebuilt from the tree for the exercise.
- Rebuilt and re-run with the full seven-role argument set and the real
  1,324,733-byte account body: **refused at plan time, exactly as designed** —
  `Error("Trading ProgramData account upgrade authority is not the one this
  plan authenticates against")`. The producer's fail-closed rail has now been
  exercised against genuine devnet bytes: **a mutable deployment cannot be
  described by the record producer at all**, which is W1 confirmed by
  execution rather than by reading.

### 3.4 Recycle

`devnet-recycle.sh` plan mode read the exact expected state (trading MUTABLE
9.221032560 SOL reclaimable; the canary's closed id an orphan Program residue;
**zero orphan buffers** — both buffers were drained into their deploys, as
§2.1's geometry says). `--execute` under the charter's own authorization text
then failed on a real defect — **the generated `close` commands named
`--authority` but no fee-payer `--keypair`, so on a machine with no default
CLI signer they die with `No default signer found`** — fixed in
`tools/release/devnet-recycle.sh` (committed with this dossier) and re-run:

```text
Closed Program Id CWGoUjgwhDpHLbgd4jFPoBWdoRfQCH1xrH14Pef5yUyR, 9.22103256 SOL reclaimed
buffers: none
post-state (plan re-run): both ids orphan-program 0.001141440 each;
                          still reclaimable 0.000000000 — NOTHING LEFT OPEN
```

---

### 3.5 The permanent footprint

The first dClutch transactions on a public cluster, finalized and permanent
(the ~1,450 buffer-write signatures are omitted; these are the landmarks):

| act | slot | signature |
|---|---:|---|
| rent deploy | 489,050,747 | `2kGQhNE9VtsGDemNRMpAuuFWT8Sn2kkPEBLc3HzdqvvvQKkp1ae3Bs2LSgfbFyi4RNwVrhgizSQo7wwYHXGvGnKX` |
| rent close | 489,050,781 | `5CdGef4a78NjjrLnhtZUbP2aSA4YQJ9r2X4ECM7359EdZ8LF7Va3RSHkNPbQJJTV14HKMLATuhDf7iK9omaiKebf` |
| trading deploy | 489,051,220 | `3XDgfY76Xppx6nPu2nzxhH2gF2siseAhEB45jfL94LhcNxH1rdj4x8hGtkeRRcHYdcyipRYYLvcZfQ18LRWfr2rh` |
| trading close | 489,052,335 | `58SAGDc9bMADP8gyS5zsLeeqNqaLyeurVZpS1JjXEcUSgpG9a9UFptaMQhWoeN2tcSdrCEUQvziRz7zLfPq2gSbe` |

(The canary's deployment slot was recoverable after all — the deploy
transaction's own slot, 489,050,747 — correcting §3.1's note that it was
lost.)

## 4. Wallet arithmetic

| checkpoint | lamports | SOL |
|---|---:|---:|
| start | 65,000,000,000 | 65.000000000 |
| canary closed (rent deployed + closed) | 64,998,123,560 | 64.998123560 |
| trading deployed (peak exposure) | 55,769,384,560 | 55.769384560 |
| **after recycle (final)** | **64,990,412,120** | **64.990412120** |

**Total cost of the entire run: 9,587,880 lamports = 0.009587880 SOL**, of
which 2,282,880 is the two 36-byte Program residues (stranded by Loader
design, counted burned) and 7,305,000 is transaction fees (~1,461 signatures
at 5,000 — the RPC-path retry passes are most of the overage above the
~1,450-write minimum). Peak principal at risk: ~9.23 SOL, all inside an open
recycle window at every moment. Every reclaimable lamport came back.

---

## 5. Frictions

*(per-role wall times in §3; measured behaviors below)*

1. **One `write-buffer` saturates the whole per-IP RPC budget.** During the
   canary (rent, 137,392 B ≈ 136 writes, `--use-rpc`), every other request
   from the same IP — including a 1-per-20-s account poll — was refused with
   `429 Connection rate limits exceeded`, for the entire duration. Deploy-day
   consequences: (a) nothing else can share the deploy machine's IP while a
   buffer is writing — the live Pyth cadence watch, balance checks, and any
   frontend all collide; (b) buffer wall time through the public RPC is
   minutes per hundred KB, not seconds — which would price the ~4.6 MB
   seven-role ladder in hours. Friction 4 dissolves (b): the TPU path moves
   the same bytes ~100× faster. (a) still stands for every RPC read.
2. **The CLI is silent while it works** when stderr is not a TTY — progress
   goes to an in-place progress bar, so a scripted deploy sees nothing until
   completion. Watch the chain, not the process; except (1) means you cannot
   even do that from the same IP.
3. Prioritization fees: the recent-fee page read all zeros immediately before
   the ladder; no priority fee was attached to any transaction in this run,
   and none was needed — **on the TPU path nothing ever stalled**.
4. **TPU vs `--use-rpc` is the whole ballgame** (§3.1's A/B, §3.2's stress
   case): `--use-rpc` moved ~350 B/s and failed with `Max retries exceeded`;
   TPU moved the 1.32 MB Trading artifact in 23 s, single pass, zero retries.
   The runbook §2.2's "`--use-rpc` avoids TPU submission drops" advice should
   invert to: TPU by default, `--use-rpc` as the fallback.
5. **Two tool defects found by first execution**, both now understood:
   `solana program show`/`dump` demand a resolvable default signer even for
   reads (pass `--keypair`); `devnet-recycle.sh --execute`'s generated
   commands lacked a fee-payer `--keypair` (fixed in this commit).
6. The killed-writer case (§3.1 pass 2) strands nothing: the buffer reads
   back consistently and resumes exactly.

---

## 6. SMOKE-1 deltas

Everything between here and a devnet market living its whole life, with an
owner and a size where one is knowable. The first item is ember's alone.

1. **The W1 ruling — RULED by ember, live, during this run.** This lane's
   original recommendation ("the deploy is THE deploy; spend the ~31.7 once")
   was built on wrong economics: devnet SOL arrives by faucet at a few SOL a
   day, so ~32 SOL is days of accumulation and the deploy will be iterated
   many times, not performed once. Ember's ruling: **the devnet substrate
   does not need to be fully trustless if full trustlessness prevents
   iteration.** The execution design — mutable roles, iteration by `Upgrade`
   at fee-cost, and the loader's slot-write invariant replacing the
   revocation in the fast path's soundness argument (fail-closed: an upgrade
   makes every open market refuse until re-released) — is **decision 0012**
   (`docs/decisions/0012-devnet-iteration-substrate.md`), with the complete
   verified site map. What remains structurally true from this dossier's W1:
   the life cannot run on a mutable substrate *at HEAD*; 0012 is what makes
   it able to.
2. **The devnet driver (a real lane, not a flag).** An external-cluster mode
   for the successor campaign: real persisted per-role keypairs, a payer
   wallet, pacing/retry against public RPC, no validator launch,
   `--ROLE-observed-programdata` fed from real reads (the path exists and is
   proven; the driver around it does not). The journey runner needs the same
   externalization for the life + ledger stages, plus a persistent-founder
   decision it was designed to refuse.
3. **The devnet `PythReleaseV1` row — DONE, same evening** (`11f249ff`,
   minted under ember's deputization): `devnet_release_v1()` in
   `crates/dclutch-pyth-svm/src/devnet.rs`, every cluster fact carrying its
   two confirmations (2026-08-26 PROVENANCE pins + this run's live reads),
   the synthetic fixture rewired to derive from it so the shared provider
   facts keep one author, masked-equality tests bounding the derivation on
   both sides.
4. **Deploy-day transport policy** (§5): buffer writes ride the TPU path —
   measured 1.32 MB in 23 s, which makes the seven-role ladder ~2 minutes of
   writes, not the hours the `--use-rpc` pace implied. What still wants a
   dedicated RPC endpoint is everything else: the campaign's *reads* and
   publication/confirmation traffic share the public per-IP budget that one
   busy process can starve (friction 1), and the founding ladder + life are
   RPC-shaped end to end.
5. **`sbf_build_diagnostics_total = 0` again** — the dealer-accelerator's 82
   frame diagnostics (§1.4) re-close before any real deploy (checked-release
   refuses at HEAD, correctly).
6. **Re-measure `DCLTGMF1` at the deploy commit** and read the CU-BUDGET
   rows; the dry-run gate (runbook §6) rerun green at that exact commit.
7. **Runbook corrections earned by this run**: blocker C's stale text
   (GIT-SCAN item 9); §5's `--use-rpc` guidance should carry the measured
   pace and the per-IP starvation; the recycle doctrine already matches
   what we measured.
8. **κ enforcement is still kernel-only** (KAPPA board entry 2026-08-27
   13:26: `admit_founding_principal` exists and is proved, kappa defaults
   1/4, but "NOTHING ON CHAIN CALLS IT YET" — Core's Found never sees
   principal). The devnet founding the charter describes ("under kappa")
   is honest only as a *founder-side* discipline until the WAVE queue item
   (cap on the Market root) lands.
