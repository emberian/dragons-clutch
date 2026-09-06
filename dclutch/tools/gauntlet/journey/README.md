# JRNY — the whole-life journey tier

The census answered *does each route run at all*. This tier asks a different
question: **does a Market, founded the way a founder founds one, survive being
used** — distributed, traded, resolved, redeemed, retired — **with every
collateral atom accounted for at every step**.

```sh
tools/gauntlet/run.sh --mode census                 # once, for the inventory
tools/gauntlet/journey/run-journey.sh \
    --checked-release-gate ABS/CHECKED_UPGRADE_GATE.json --rpc-port auto
```

## 2026-09-06: the tier stands up its own substrate, and this file changed under it

Everything below this section that describes `--market`, `--spec`,
`found_through_open`, `--keypair-seed`, the seven in-runner SBF builds or
`frame-diagnostics.json` is **superseded**. Read this section for what the tier
is now; the older prose is kept for the incidents it records, not for its
instructions.

**The substrate.** The runner used to refuse to start without `--market PATH`,
and nothing could produce one: a Market compiles only through
`DirectMarketCompilerOwnedV1::load_local`, which observes a LIVE checked
deployment first, and this runner had no deployment of its own. It now brings up
the checked-mutable substrate itself from a `CHECKED_UPGRADE_GATE.json` —
`local-mutable-prepare-v1`, a fresh `solana-test-validator` over the prepared
account directory, the administration campaign through activation — links
`tools/gauntlet/relayed-vertical/src/substrate.rs` rather than forking it (the
ladder links the same file), compiles the Market against the deployment it is
standing on, founds it with `campaign --founding-only`, and **keeps the
validator** for every stage after. The gate is the build stage: it is emitted
only in strict mode and strict mode refuses a nonzero SBF
stack-frame-overwrite diagnostic count, so a gate that exists IS the
zero-diagnostic proof `TIERS.md` asks for. That is why the runner no longer
builds ELFs and no longer carries a diagnostics exemption file.

**The four stages.** Admission, fill, redemption and retirement were four
`GapV1`s. They are stages now, and every act is the **shipped command a host
runs**, called in this process with the argument vector a host would type —
`src/spine.rs` names the entry point beside each one. A tier that rebuilt any of
those frames would be measuring a second author; that is the lesson
`tools/gauntlet/ladder/` wrote down and this file follows it.

**Two of the old gap register's reasons were false**, and `gap_register()` says
so with the evidence rather than dropping them quietly: admitting a Position is
a top-level Trading route a wallet signs for itself, and terminal settlement is
wallet-signed top-level Claims. The packet arithmetic that made the canonical
Direct Hot continuation unsubmittable was answered by a fill that landed on a
loopback validator on 2026-08-31.

**Resumption loops, not calls.** Three of the drivers advance exactly one
durable action per invocation — that is their crash-safety contract — so each
stage is a bounded loop with a stated ceiling, and a stage that hits the ceiling
reports how far it got. A refused stage is a FINDING recorded with the driver's
own sentence; the run fails at the end, after the transcript exists.

**`BeginRetiring` had two authors, and one is deleted (2026-09-06).** This
campaign used to run its own hand-built `BeginRetiring` plus a Source closure
before the shipped terminal-sequence driver. It could never have shared a run
with it: it ran `ResolutionCloseFund` at position two, ahead of
`DirectCloseCapability`, which is the pair PROGRAMS-18A reversed and the one
ordering `TerminalStageV1::ORDERED` forbids — Core `CloseCapability` on the
Direct entry PRESERVES the Resolution dependency ledger byte for byte, and
`ResolutionCloseFund` is what closes it, so closing first destroys the next
stage's input and the sequence stops three short of Retired on a zero-byte
account. The convergence is not a preference between two working paths; both
drove the same corrected V7 close (`build_resolution_direct_close_fund_v1`), so
no route lost an author. `resolution.rs` keeps the reasoning where the function
was.

**Owed.** `bindings.json` does not yet cover the spine's labels —
deliberately: bindings are authored from what the ledger OBSERVED, never from
what a campaign ought to touch, which is the rule
`tools/gauntlet/retirement-checkpoint/` wrote down after manufacturing exactly
that false green.

## Not a fast lane

There is no `solana-program-test` lane here and there will not be one. The
journey begins with a real founding, which fails all four of `TIERS.md`'s
fast-lane conditions — genesis Loader-v3 ProgramData spans, a real
`SetAuthority(Some -> None)`, the 1,232-byte packet limit that Found31 misses by
ten, and real per-transaction compute. Answering them one at a time would just
be four separate noes.

## The producer is the successor, all of it

`src/main.rs` compiles `tools/local-validator/bootstrap/successor/src/` into
this binary by `#[path]`. They are not copies. **The set is the whole of it**
since 2026-09-06, generated from the successor's own `main.rs` module list
rather than curated: the curated subset's tripwire fired four times in six days,
twice silently and once for a whole day, because nothing in CI builds this tier.
Three names are excluded and the header says why — two are submodules of other
successor files, and `ledger` is this tier's own file, which the successor links
back the other way.

If the producer moves, this build breaks. That is the intended tripwire — and it
has a consequence worth knowing before it surprises you. **`cargo check` in this
directory compiles the producer's files out of the shared working tree**, so it
goes red while any lane has the bootstrap dirty, whether or not anything is
wrong with the journey. The authoritative build is the one `run-journey.sh`
does, from `git archive` of the gate's exact revision.

It has a second consequence, newly measured: three of the successor's own
`#[cfg(test)]` tests in `series_lifecycle_campaign.rs` read evidence fixtures at
a path relative to `CARGO_MANIFEST_DIR`, so they pass from the successor's
manifest and fail from this one. That is a fixture path with two possible roots,
and it is the successor's to fix.

## Deterministic by default

The runner passes the producer's `--keypair-seed`, defaulted on. Without it the
`find_program_address` bump-search noise is 58,494 CU on `DCLTGMF1` inside a
single campaign, and it moves every rent figure this tier checks — a
conservation ledger whose numbers cannot be compared between runs is a diary.
The seed is the SHA-256 of `dclutch/gauntlet/journey/campaign-seed/v1`, a stated
derivation rather than a number somebody typed. `--keypair-seed none` takes
fresh keys instead.

It is safe here and **only** here: the producer refuses the flag outright unless
the RPC endpoint is loopback, and this tier only ever names a `127.0.0.1`
origin, whatever base it runs on. Read
`seed.rs` before using it anywhere else. The transcript records
`deterministic_keypairs`, because a transcript that does not say which mode
produced its numbers is a transcript whose numbers cannot be used.

**What the seed does not fix, measured.** It pins the *rent* figures — every
account whose address is derived from a keypair — and that is what the
conservation ledger compares. It does **not** make compute reproducible. Two
runs at one revision under one seed (`8aa6227`, N=4 and N=16) moved `DCLTGMF1`
by 12,002 CU and `DCLTPCB1` by 5,998, every delta a multiple of ~1,500: the
`find_program_address` bump search on the addresses that are *not* keypairs —
the slot-derived routing table, and generation- and slot-derived record and
compartment PDAs — which no keypair seed can reach. So a CU figure from this
tier carries a ~1% band and a budget tolerance has to cover it; a rent figure
does not. Do not read a small `DCLTGMF1` CU delta between two runs as a
regression.

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
| L7 | `payer_delta + fees + watched_growth == 0` | the trading stages forced it. L1..L5 are about collateral ATOMS and say nothing about the lamport side of a fill; L6 only fires when a watched account CLOSES. A route that quietly debits whoever submitted it, or places rent in an account nobody named, passes all six. This is "debit == credit + fee". |

A law that cannot be evaluated at a boundary records itself `inapplicable` with
a reason and is still counted. A law that quietly stops applying is how a
conservation argument rots.

**L7's fee term is never a prediction**: it is summed off the stage's own
transaction evidence, so the law compares the chain against what the chain
charged. Two consequences shape the code. The founding boundary records L7
`inapplicable` — the founding's lamport placements are the tier-1 producer's,
and restating them here would be re-deriving another campaign's arithmetic and
calling the agreement evidence. And every account the journey creates is
registered with the ledger BEFORE the first census (`stages::plan_holders`,
`resolution::watch`, `provider::watch`), so the census preceding each creation
records a checked vacancy rather than meeting a balance with no predecessor. A
boundary that still admits a new label reports L7 inapplicable and NAMES the
labels rather than counting a whole balance as growth.

## What executes

| stage | what it does |
|---|---|
| founding through Open | the tier-1 campaign, called not copied |
| collateral distribution | N synthetic holders open a Token-2022 account and receive a share. N is the load knob. |
| holder-to-holder | a ring of transfers in which the founder is not a party |
| resolution funding | `CreateFund` and `VerifyFundReady`, chain-derived, with an over-funded Fund and a double-create refusing first |
| resolution transport | the real Wormhole router and Pyth receiver, a verified VAA, one posted update, and the two dClutch provider legs that resolve the Source and mint the terminal certificate |
| terminal admission | the standalone Core `AdmitTerminal` that moves the Market's phase byte 1 → 2 and writes the terminal receipt — the devnet spine's own third act, and the one this tier drove nothing for until 2026-09-06 |
| rent recovery | `rent/process_sweep_v2#Sweep`, **executed for the first time by any tier**, with the adversarial half first |

## The resolution half, and why it is reachable at all

JRNY-1 stated resolution as a gap and said the gap was "a missing campaign, not
a closed door". This is that campaign, and one fact makes it possible:
`create_accounts` and `verify_accounts` in `dclutch-resolution-core-v3-operator`
hand back frames in which **every `AccountMeta` is `is_signer: false`**. The
whole funding ladder is wallet-constructible — a fee payer and nothing else —
which is exactly what every Claims and Custody route is not. `rpc.rs`'s
`finalized_observed_accounts` already returns the operator's own
`ObservedAccount`, because `dclutch-versioned-message-operator` re-exports that
type rather than declaring a second one that agrees today.

**Two frames do not fit a legacy packet.** Measured, not predicted: the first
execution of `CreateFund` was refused by the RPC at 2,016 bytes against the
1,232 limit. `CreateFund`, `VerifyFundReady` and both provider frames ride
finalized address lookup tables as v0 transactions, through the producer's own
`publish_routing_table` rather than a second copy of the routing shape here.

**What the first run of this stage found in the Market itself.** The demo
Market's then-current `SourceMaterialV2` named its source spec, window spec and
statistic spec — and its failure policy — by domain-separated demo digests. A finalized
record lives at an address derived from the hash of its own body, so those were
records nobody could ever publish, and the Market could fund its resolution and
then stop forever one step short of a certificate. Nothing refused; it simply
had no next move. The current campaign uses the clean-break 240-byte
`SourceMaterialV3`; `market.rs::demo_market_input` compiles its graph, every
identity is its body's digest, `validate_market_input` checks exactly that, and
the selected records are published with the rest.

## §12.3 is two clocks, and only one is a market parameter

```
observation_unix_seconds in [window.start, window.end]        -- what it is ABOUT
publication_unix_seconds in [now - max_age, now + max_skew]   -- how FRESH it is
```

The window is a real 300-second terminal period ending at the captured
publication. That width is TWIN's finding: a window forced to one instant is
answered only when a publication happens to land on that exact second, and
Pyth's SOL/USD cadence is nearer five minutes, so a degenerate window is a market
nobody can resolve. It is legal, and it is a choice rather than the type's
demand.

`max_age_seconds` is **not** a market parameter here. The fixture's publication
instant is frozen at its capture date and a validator's clock is wall-clock, so
the quantity it bounds is THE AGE OF THE FIXTURE and it grows by 86,400 a day.
It is stated as the fixture's declared shelf life, and `provider.rs` measures
that age against the chain's own clock before submitting anything and refuses
with an instruction to **recapture the fixture** — explicitly not to widen the
number, which is the failure the bound exists to prevent. A Market resolving
against a live feed states seconds there.

**What N costs**, measured at N=4 and N=16: exactly `2N` transactions and
`3,658 N` CU — 1,788 to open and fund a holder, 1,870 for one ring transfer,
both identical per holder at either N. Rent recovery is 2 transactions and 7,268
CU at any N, and the founding half does not move with N at all. There is no
superlinear term to find yet, because these stages are SPL Token operations and
nothing here invokes a dClutch program — which is also why this tier reports no
heap figure for N: there is no dClutch heap in the part that scales. The load
knob becomes interesting the day the Hot gate opens and the distribution stage
starts routing through Claims.

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

The resolution stages are the first of this tier's own transactions with a
compute profile worth reading (`CreateFund`, `VerifyFundReady`, and the two
provider legs, all under real ELFs). This tier still writes no budget row for
them, on the same rule: measure first, across enough runs to know the band, and
then pin. The transcript records the per-stage totals so that measurement exists
before anybody writes a number down.

## What the whole life costs, measured

**The table below is the pre-substrate campaign's** (`20260827T172103Z-606c042e8416-h4`,
N=4, 161 transactions, 17,293,041 CU) and is kept for the routes it priced, not
as this tier's current shape: that run founded through the ladder, and its
"resolution funding" and "begin retiring" rows are acts this tier's atomic
founding and shipped terminal-sequence driver own now. The current campaign's
per-stage numbers are in each run's transcript; hbox `20260906T150850Z`, N=4,
measured founding through Open 198 tx / 6,270,009 CU, the Direct capability
activation 5 / 547,294, the admission 3 / 345,247, the Direct Hot fill 9 /
2,102,027, the fee settlement 1 / 94,766, the Pyth transport 21 / 820,188 and
rent recovery 2 / 7,517.

| stage | tx | CU |
|---|---|---|
| founding through Open | 116 | 8,608,838 |
| distribution + ring | 8 | 14,632 |
| resolution funding | 10 | 4,514,578 |
| resolution: Pyth transport to Terminal | 19 | 2,719,033 |
| retirement: begin retiring + close the Source subtree | 6 | 1,273,097 |
| rent recovery | 2 | 7,134 |

The routes nobody had executed on a chain before:

| route | CU |
|---|---|
| an Open Market creates its own Resolution Fund | 1,193,660 |
| VerifyFundReady activates three ledgers | 1,181,368 |
| the real router verifies the 13-of-19 signed VAA | 335,276 |
| Resolution submits one update through the real receiver ELF | 1,054,022 |
| Core admits the terminal state | 1,281,705 |
| a resolved Market begins retiring | 62,425 |
| Resolution closes the Source subtree | 1,210,372 |

Reproducibility across the two runs that reached these: CreateFund 1,202,639 →
1,193,660, VerifyFundReady 1,181,347 → 1,181,368 (21 apart), the submit leg
1,054,017 → 1,054,022 (5 apart) — inside the ~1% bump-search band this tier
documents above. Four of them sit at 84–92% of the 1,400,000 ceiling on routes
that have no CU budget row.

### The ledger's own bug, and how it was verified without a re-run

That run reported three L7 violations, and all three were L7's fault. It summed
watched accounts per LABEL, and four addresses carry more than one of the
founding's evidence keys: the Market's rent beneficiary IS the founding's
lifecycle credit, the fee payer is also a credit's refund wallet, `found31_market`
IS `market`, and the normal and projected Custody replays are one account because
the projection is realized in place. Every change to those four counted twice.

It stayed invisible until a stage CLOSED accounts and REFUNDED rent through them
— the first time this campaign ever did either — and then the residuals were the
doubled amounts exactly.

The fix sums by address. It was verified by recomputing both the old and the new
arithmetic from the transcript's own recorded per-account addresses and lamports:
**the new residual is zero at every boundary.** That is the argument for a
transcript that records what it SAW and not only what it concluded — it can
answer a question asked after the chain is gone.

## Three findings the chain made that a fixture could not

All three cost a full campaign each to discover, and all three are the same
shape: a fact that looks settled in ProgramTest and is not settled on a chain.

### A fixture that binds two roles to one key hides a role confusion

`ProviderExecuteDeploymentV3.trading_program` means the TRADING role.
`resolution_core_v3_lifecycle.rs` passes CUSTODY there and passes, because that
fixture's release set binds Custody's key to the Trading role — the two are the
same bytes, so the confusion has nowhere to show. A real five-role activation
binds five different keys, and `provider_instruction_v3` authenticates accounts
13/14 against `activation.role(ExecutionRoleV1::Trading).release().program()`.
The defect is invisible in ProgramTest *by construction* and fatal on the first
validator: `ResolutionRelease` (0x8005) after 681,773 CU. **Anywhere a fixture
reuses one key for two roles, this class of hole is available.**

### Two live readers of one field, with incompatible rules

`ProviderReleaseV1.adapter_release_id`:

| reader | demands | so the field means |
|---|---|---|
| `PythProviderAdapterObligationV1::from_material_view` | `PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1` | which provider EXTENSION this is |
| `authenticate_provider_release` (V3 route) | `pyth_release.adapter_id()` | which adapter release the deployment carries |

The two constants differ, so **no `ProviderReleaseV1` satisfies both.** The live
V3 route joins through the V2 obligation, which does not check the extension, and
then through `authenticate_provider_release` — so the second reading is the one a
chain enforces. Both refusals this campaign collected were correct *given their
own reading*, which is exactly why neither named the problem. Carried as an owner
decision: delete V1's reading, or split the field into the two facts it is being
asked to be.

### A refusal taxonomy that erases a distinction its own contract calls
load-bearing

`normalize_authenticated_update`'s doc comment says it plainly:

> A fresh publication about the wrong period and a stale publication about the
> right one must both refuse, and **an operator reading the log should be able to
> tell which happened.**

They cannot. `InvalidObservationSchedule`, `InvalidPublicationTime` and
`InvalidPythObservation` all reach the wire through
`.map_err(|_| ProviderJoinErrorV3::Provider)`, and `map_provider_join_error`
sends that single variant to the single code `ResolutionError::ProviderObservation`
(0x800A). Three questions — *is this about the right period, is it fresh enough,
is it a well-formed Pyth observation* — arrive as one number. This tier paid a
diagnosis cycle to that collapse, which is the evidence that the doc comment was
right about it mattering.

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
`open_series_market` (`crates/dclutch-market/src/generated.rs:922`)
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

That gap said: **a missing campaign, not a closed door** — nothing yet composed
`CreateFund` → `VerifyFundReady` → posted provider evidence → Core-driven
execution against a live validator. **JRNY-2 built it**, and it is the
`resolution funding` and `resolution transport` stages above. Building it turned
up the thing behind the gap: the campaign's Market named three source-graph
records that could not exist. See "The resolution half" above.

Worth keeping the whole arc visible, because it is the tier working as intended:
JRNY-1 found by READING that an atomically founded Market could never be
resolved; SRC-FOUND fixed the prestate and proved it against the compiled ELFs;
JRNY-2 drove it on a chain and found that the door was open and the Market had
no key. Each step needed the one before it, and none of them was a re-run.

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
run-journey.sh    build -> campaign -> ledger -> witnesses -> census
bindings.json     THIS campaign's transactions; tier 1's are merged in at run time
witnesses.json    evaluated by the shared tier1/check-witnesses.sh
src/ledger.rs     the seven laws
src/stages.rs     the collateral and rent stages
src/resolution.rs the funding half of the resolution ladder
src/provider.rs   the Pyth transport and the two provider legs
src/journey.rs    orchestration, the transcript document, and the gap register
```

**A stage that refuses does not discard the run.** The transcript and the
complete conservation ledger are written first, the refusal is recorded on the
stage and in `unexpected_refusals`, and the campaign fails afterwards. A wall is
worth more beside a complete ledger than as an error that throws the rest away.
