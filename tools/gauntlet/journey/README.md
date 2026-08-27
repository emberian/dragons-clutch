# JRNY-1 — the whole-life journey tier

The census answered *does each route run at all*. This tier asks a different
question: **does a Market, founded the way a founder founds one, survive being
used** — distributed, custodied, resolved, redeemed, retired — **with every
collateral atom accounted for at every step**.

```sh
tools/gauntlet/run.sh --mode census          # once, for the inventory
tools/gauntlet/journey/run-journey.sh --holders 4
```

It takes the single global `127.0.0.1:20890` slot. Coordinate on the wave board.

## Not a fast lane

There is no `solana-program-test` lane here and there will not be one. The
journey begins with the tier-1 founding, which fails all four of `TIERS.md`'s
fast-lane conditions — genesis Loader-v3 ProgramData spans, a real
`SetAuthority(Some -> None)`, the 1,232-byte packet limit that Found31 misses by
ten, and real per-transaction compute. Answering them one at a time would just
be four separate noes.

## The producer is the tier-1 producer

`src/main.rs` compiles `tools/local-validator/bootstrap/successor/src/`'s five
modules into this binary by `#[path]`. They are not copies. The journey calls
`runtime::found_through_open` and then keeps going **in the same process, on the
same validator, as the same in-memory founder** — which is not an optimisation
but the only possibility: the founder key is ephemeral and deliberately never
persisted, so a second process reading the ledger could not sign as the founder
if it wanted to.

The one edit this required was splitting the producer's `execute` into
`found_through_open` plus the write it always did. The bootstrap binary's
behaviour is unchanged: same transactions, same order, same document.

If the founding moves, this build breaks. That is the intended tripwire — and it
has a consequence worth knowing before it surprises you. **`cargo check` in this
directory compiles the producer's files out of the shared working tree**, so it
goes red while any lane has the bootstrap dirty, whether or not anything is
wrong with the journey. The authoritative build is the one `run-journey.sh`
does, from `git archive` of an exact revision; to reproduce it by hand, archive
HEAD to a scratch directory and check there. Observed on 2026-08-27, when a lane
mid-way through adding a `--keypair-seed` option turned this tree red for
several minutes while HEAD was clean.

## Deterministic by default

The runner passes the producer's `--keypair-seed`, defaulted on. Without it the
`find_program_address` bump-search noise is 58,494 CU on `DCLTGMF1` inside a
single campaign, and it moves every rent figure this tier checks — a
conservation ledger whose numbers cannot be compared between runs is a diary.
The seed is the SHA-256 of `dclutch/gauntlet/journey/campaign-seed/v1`, a stated
derivation rather than a number somebody typed. `--keypair-seed none` takes
fresh keys instead.

It is safe here and **only** here: the producer refuses the flag outright unless
the RPC endpoint is loopback, and this tier is pinned to `127.0.0.1:20890`. Read
`seed.rs` before using it anywhere else. The transcript records
`deterministic_keypairs`, because a transcript that does not say which mode
produced its numbers is a transcript whose numbers cannot be used.

## The conservation ledger

One object, threaded through the whole journey, that re-reads the economic state
from the chain at every stage boundary and evaluates the same six laws. It is
deliberately not a set of per-step spot checks: a spot check asks "did this
transaction do what it said," and a market can pass every one of those while
leaking atoms across the seams between them.

| law | what it says | why it is not a mirror |
|---|---|---|
| L1 | tracked collateral == `Mint.supply` | the supply is the TOKEN PROGRAM's accounting, and the founding revokes the mint authority, so it is frozen. An atom in an account nobody named breaks it. |
| L2 | the Hoard moves only by what the stage DECLARED | the one law L1 cannot state: principal moving from the Hoard into a wallet the ledger already tracks leaves the total untouched, which is what an undetected leak looks like |
| L3 | Σ Positions == aggregate supply | who is owed, against what the Market's own liability record says it owes |
| L4 | Hoard ≥ worst outcome × claim unit | the unit comes from the Registry's published `ProductBasisV3.payout_scale`, not from the Hoard divided by the supply |
| L5 | observed collateral delta == DECLARED delta | a stage states what it will move before it runs; L1 alone balances for a transfer between two tracked accounts |
| L6 | closed rent arrives somewhere watched | rent is the one value that is not collateral and still must not evaporate |

A law that cannot be evaluated at a boundary records itself `inapplicable` with
a reason and is still counted. A law that quietly stops applying is how a
conservation argument rots.

## What executes

| stage | what it does |
|---|---|
| founding through Open | the tier-1 campaign, called not copied |
| collateral distribution | N synthetic holders open a Token-2022 account and receive a share. N is the load knob. |
| holder-to-holder | a ring of transfers in which the founder is not a party |
| rent recovery | `rent/process_sweep_v2#Sweep`, **executed for the first time by any tier**, with the adversarial half first |

The sweep is the one worth reading. It takes three accounts, one of them a
sysvar, and **needs no signature at all** — so the only thing between the
lifecycle credit and being drained below its own rent minimum is the checked
balance plan. The tier submits a sweep of one lamport past the surplus, asserts
it refuses `Balance` and moved nothing, and only then sweeps the surplus,
asserting the credit is left holding exactly the rent minimum, that the refund
wallet named in the credit's own bytes gains exactly the surplus, and that the
fee payer — a different key — moved by exactly the fee and nothing else.

**Which credit** is discovered, not named. The founding leaves several: one per
projected-Custody prestate lane plus Found31's, and only the lane that actually
closed accounts into its credit carries a surplus. The first execution of this
stage named `lifecycle_rent_credit`, read Found31's credit sitting at exactly
its rent floor, and reported `blocked` — while the abort lane's credit held
13,488,480 recoverable lamports two keys away. Two census bindings then matched
no transaction and the census refused the run, which is the gate working. The
stage now re-reads every rent-program-owned, credit-width account the founding
recorded and sweeps the one above the floor, the same way the collateral
partition is discovered rather than hand-listed.

## The build gate, and what it caught on its first run

`cargo build-sbf` exits **zero** when the SBF backend reports that a call
overwrites its own stack frame and "may cause undefined behavior during
execution". `run.sh` counts them and warns. This tier **refuses**: the journey's
whole claim is about state surviving a long chain of transactions, and
undefined behaviour anywhere in that chain voids the claim silently.

The first time it ran it refused, on **65 diagnostics — every one of them in
`dclutch_resolution_proof_sbf::relay_transport_v1::process_relay_transport_v1`**,
with the other six role artifacts at zero (measured at `0ca81cc`). That artifact
is bound into the five-role release set and activated by tier 1, which had been
producing evidence on it under a warning nobody has to read.

**It reports zero at `37d873f`.** Nothing in the Resolution program's own
history obviously fixes it — the codegen moved under it — so it can come back,
and if it does this tier refuses again. `frame-diagnostics.json` is therefore
*empty* rather than holding a lapsed exemption "just in case": an entry is kept
only while it is true, exactly as `blocked.json` requires of a blocking reason.
The measurement is recorded here so deleting the entry does not delete the
history.

The narrow exception is `frame-diagnostics.json`, shaped like `blocked.json`:
each entry names the exact mangled symbol, the measured count, why this campaign
does not reach the function, and who owns the fix.
`check-frame-diagnostics.py` refuses a diagnostic that matches no entry, refuses
one attributed to the wrong role, and refuses a **count that grew** — a growing
count is a new defect wearing an old exemption. A count that shrank is reported
loudly as stale and does *not* fail, so whoever lands the fix is not met with a
red run. All three refusals are exercised.

The exemption is that the journey does not execute the function. It is **not**
that the function is fine; the shipped Resolution ELF still has it. The known
fix is the frame split W2h used on `hot_v3::process_hot_execution_v3`.

## No CU budget of its own, on purpose

`TIERS.md` asks a tier to opt into a CU budget *if its transactions are worth
budgeting*. This tier's own transactions are a handful of SPL Token operations
and a three-account sweep; a budget on those would be a number nobody would ever
act on. The transactions in this campaign that ARE worth budgeting — `DCLTGMF1`
and its five stages, `DCLTPCB1`, `Found31` — are tier 1's, they carry tier 1's
entries in `CU_BUDGETS.json`, and this run evaluates them because tier 1's
witness set runs against this campaign's evidence. The coverage is inherited,
not absent. When a post-Open stage grows into something with a real compute
profile — the first Hot execution, terminal settlement — that is when this tier
should add its own entry, and it should measure before it writes one.

## What does not, and exactly why

The transcript carries a gap register; `src/journey.rs::gap_register` is its
source. These are read off the code rather than off a refused transaction,
because the frames are not **constructible** by a wallet at all, so there is no
honest transaction to submit and record. Two findings are worth stating here.

### The Hot gate is wider than "Direct fills"

Every Claims mutation frame puts a `CallerAuthority` at index 0 that must be
both a **signer** and the `CallerAuthoritySeedsV1` PDA under the calling
program, and then re-authenticates that program against the Registry activation
cache as the **Trading** role. Only a program can sign its own PDA. So on a
validator carrying the immutable five-role release set the sole admissible
caller is the deployed Trading program — and Trading's outer dispatch routes
everything that is not `DCLTGMF1`, `DCLTPCB1`, `DCLTPCA1`, or the capability
seal into `hot_v3::process_hot_execution_v3`.

Custody's nine-account common prefix has the same shape at indices 0 and 4.

**The whole of post-Open Claims and Custody life is behind the W2i Hot gate**,
not just Direct fills: no holder can be admitted a Position, no outcome token
can move, no vault can be opened. That reframes W2i from "trading does not work
yet" to "the Market's entire post-Open life is behind one door."

### The campaign leaves two admissible resolution prestates — and the gap that used to be here is closed

This section used to read *"an atomically founded Market can never be
resolved."* It is worth keeping the correction visible, because the finding was
real when it was written and the fix landed between the tier being built and the
tier first running.

The claim was: every route that can put a terminal receipt on a Market consumes
a `SourceResolutionStateV2`; the **only** route that creates one is
`core/resolution::process#CreateFund`; its phase gate admitted
`Founding+Prepaid` and nothing else; and `DCLTGMF1`'s commit-last
`open_series_market` (`crates/dclutch-market-core-codec/src/generated.rs:922`)
goes `Founding+Prepaid -> Open+Consumed` in **one** transition, never passing
through `Ready`. So the atomic founding closed the resolution door behind
itself.

`edfcb24` admitted the second prestate and `60a2101` walked it end to end
against the compiled Registry, Core, Custody and Resolution ELFs and a real
posted Pyth update. The gate is now `resolution_fund_prestate_admissible`
(`programs/dclutch-core-sbf/src/resolution.rs:386`):

```rust
state.terminal_receipt.is_none()
    && matches!(
        (state.phase, state.readiness),
        (Phase::Founding, Readiness::Prepaid) | (Phase::Open, Readiness::Consumed)
    )
```

So both Markets this one campaign leaves on one ledger are admissible starting
points, and two witnesses pin them: the founded Market at `Open+Consumed+false`
and the canonical Found31 Market at `Founding+Prepaid`. A Source/provider tier
needs no new campaign to reach either.

What remains is **a missing campaign, not a closed door**: nothing yet composes
`CreateFund` → `VerifyFundReady` → posted provider evidence → Core-driven
execution → `AdmitTerminal` against a live validator. The journey states that as
its gap and does not build it.

### The campaign locks the entire collateral supply, and strands half of it

Not a protocol defect — a **campaign shape**, and worth writing down because it
is invisible until something tries to spend afterwards. The founding runs its
projected-Custody prestate ladder twice, once for the founding lane and once for
the source-abort lane, and each lane locks `initial_collateral_atoms / 2`. Two
lanes therefore consume the supply exactly: half ends in the Hoard, and half is
refunded by the abort into a token account owned by an ephemeral beneficiary key
the campaign never persists. **The founder's own wallet ends at exactly zero,
and nobody can spend the refunded half — this journey included.**

In a real deployment the abort beneficiary is a user's own key, so nothing is
lost there. But post-Open collateral movement needs a founding that does not
lock the whole supply, so the distribution stage opens its holders and reports
`blocked` with nothing to send. The conservation ledger is what makes this
visible rather than confusing: L1 stays green because the ledger DISCOVERS its
collateral partition by re-reading every address the founding named and keeping
the ones that are live token accounts for this Mint. A hand-listed partition
would have shown a 500,000,000-atom hole and sent someone hunting a bug in the
protocol.

## Files

```
run-journey.sh   build -> campaign -> ledger -> witnesses -> census
bindings.json    THIS campaign's transactions; tier 1's are merged in at run time
witnesses.json   six, evaluated by the shared tier1/check-witnesses.sh
src/ledger.rs    the six laws
src/stages.rs    the post-Open stages
src/journey.rs   orchestration, the transcript document, and the gap register
```
