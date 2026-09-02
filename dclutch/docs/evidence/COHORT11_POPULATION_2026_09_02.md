# Cohort-11: strangers admit — 2026-09-02

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized.

`COHORT11_GENESIS_FOUNDED_2026_09_01.md` ends with the market founded, open, and
its population life **not** demonstrated: the load simulator stopped at
`BlockhashNotFound` on an admission whose prefund had already landed. This file
is the sequel. Two strangers now hold Positions in the cohort-11 SOL/USD market,
admitted through the simulator, and six conservation laws hold across four
census boundaries.

Three walls stood between those two states. Only the first was the one the
previous lane named.

Tree root `/Users/ember/dev/dclutch`. Source commits:

- `7fac94ec` — the `ledger-census` caller learns the argument that made L8 a law.
- `3387966f` — the blockhash is bound where it is signed, not where it was planned.
- `8fda79bf` — the frozen routing table is read by address, never searched for.
- `6c9ef569`, `e55bb599` — the compiled message is compared to the frame before
  the chain is, and the fee-payer half of that refuses before the prefund spends.

## Wall 0: the driver had not compiled since `061eaa39`

Before anything could be run, nothing could be built. `061eaa39` made `classes`
a required parameter of `Ledger::observe` — correctly, because "a law whose
input is optional is a law that reports nothing" — and did not update the
`ledger-census` caller in the successor bootstrap's `main.rs`.
`dclutch-local-successor-bootstrap` had not compiled since, which means **every
devnet driver in the tree** had not compiled since: the campaign, the admission,
the census, the trade producer. Nothing went red because no gate builds that
binary.

This is the red umbrella AGENTS.md describes, in its purest form: a shared
signature deepened, every file the deepening touched still green, one downstream
consumer in another workspace left behind.

The value it takes is the one already beside it. An external census does not
drive the transactions between its two observations, so it cannot say which
compartments they crossed, exactly as it cannot guess their fees:
`ClassClaimV1::inapplicable`, not `unchanged()`, which would be the claim that
not one atom changed class.

## Wall 1: a blockhash lives twenty-five seconds on this chain

The named defect, and the measurement that explains it.

**Devnet was producing 6.0 blocks per second** (measured 2026-09-02: block
height 479,590,576 → 479,590,701 across 20.8 seconds; 6.15 slots/s). Solana's
`MAX_PROCESSING_AGE` is 150 blocks, so a blockhash on this chain is a
**twenty-five second** artifact, not the minute the 400 ms slot assumption
suggests.

`build_report` fetched the blockhash, and then — before the driver was permitted
to sign — spent a full finalized re-observation of every semantic account, a
fresh `getFeeForMessage`, a poststate probe, and a prestate re-read. That pass
is longer than twenty-five seconds through a load-balanced endpoint. Both of the
previous lane's attempts died on it, and **the existing guard passed both times**,
because `getBlockHeight` was measured with two more round trips still to come
before the send — twelve blocks of a hundred-and-fifty-block life.

The repair is not a retry. A blockhash is not a fact about the market; it is the
submission parameter that decides which banks still accept the packet, and it is
the only field of a planned admission that expires. So it is bound last:

- `rebind_unsigned_admission_blockhash_v1` runs after `require_prestate_unchanged`
  and the genesis re-check, immediately before the first key read, and journals
  the rebound message exactly as the original was journaled.
- **Rebinding is not re-signing.** `require_rebindable_unsigned_admission_v1`
  admits only a `Planned` report with no signed packet, no packet digest and no
  expected signature — the only shape in which no signature over those bytes
  exists anywhere, and therefore the only one in which replacing them can replay
  nothing. A signed packet whose blockhash died is still archived, never
  re-signed.
- The ordering is a **type**, not a comment. `FreshlyBoundMessageV1` is
  constructed only by the rebind, and `sign_and_submit` takes its message from
  one and from nothing else.
- `resend_dispatching_packet_v1` cannot rebind — its packet is signed — so
  instead its genesis check and prestate re-read move **above** its expiry
  measurement, leaving nothing between the measurement and the send.

The previous lane's dead packet was the first thing the repaired driver met, and
it said the right thing:

```
Dispatching admission packet expired while absent; archive the journal
rather than re-signing
```

Archived after confirming on chain that its signature was absent and the Position
and admission PDAs were still vacant. Note that the simulator had been reading
that failure as *backpressure* — `"blockhash not found"` is a
`BACKPRESSURE_MARKERS` entry — and therefore as something to wait out. The
repaired driver's refusal names expiry instead, so the loop halts on it.

## Wall 2: the routing table was being searched for, and could not be found

`frozen_routing_table_for` scanned `getProgramAccounts` over the entire
AddressLookupTable program and kept the frozen table whose address list held the
market. On devnet that scan returns nothing usable, so it answered `None` for a
table that demonstrably exists — and answered it as an *absence*, not a refusal.

**The predicate was wrong as well as unscannable**, which the scan's failure hid.
Cohort-11's frozen table `6Pwb16HHphgvDbr6RW4p7k82qTGDccQHizJzk3LDXZwk` routes
`founding_market` `3rBfDBpaXjKSbUU5HRaRTr6yhDQq4S1oKp2mQRsdoyb6` and sixty-three
other accounts. It does **not** contain the Core Market
`ARuPAuyJbJoLdMWGDzSqvcV9py25EkmMj8ABnfKP56s`, which is the address a reader
naturally reaches for. A search handed the wrong one of those two answers a
confident `None` even on a chain small enough to scan.

Nothing has to be searched. The founding's own create transaction is in the
campaign evidence under the label `create DCLTGMF3 frozen routing address lookup
table` (`5iyBJssn…`), and `CreateLookupTable` puts the table it creates at
account index 0 of its own instruction. Two reads by address replace the scan,
and then the account is **authenticated** against the founding it claims to
serve — owned by the ALT program, authority `None`, routing this founding's
market — so a wrong pick refuses by name rather than routes.

## Wall 3: the owner cannot pay its own fee, and only the chain said so

With the blockhash bound late, the admission stopped refusing at the endpoint and
reached the program — where it refused `TradingSbfError::Content` (`0x4003`)
after 12,233 CU with no CPI, at instruction 2. `Content` is exactly the
undifferentiated code AGENTS.md names: one accusation over thousands of sites.

One comparison found it. Of the twenty-seven outer coordinates, **exactly one**
presented a privilege the plan did not declare:

| coordinate | account | compiled | declared |
| ---: | --- | --- | --- |
| 24 | `5nTAZrNLeebevH1nrmfvNpKAsTqE6qPZMz1GDaTzAhKi` | signer, **writable** | signer, **readonly** |

Coordinate 24 is the Position owner, which `UserPositionAdmissionFrameV1`
requires to sign **readonly**. The compiled v0 message presented it writable
because the owner was also the transaction's fee payer, and a fee payer is
writable unconditionally — the fee debits it. `num_readonly_signed` was `0`
where the frame needs it to count the owner. The two requirements are jointly
unsatisfiable: **an admission whose owner pays its own fee can never land, on
any chain.**

The driver knew half of this already. `prefund_admission_rents_v1` exists
because a System transfer debiting the owner in the same transaction forces the
same promotion, and it pays rent in a separate finalized transfer to avoid it.
That repair cannot reach this one. So the prefund landed, the plan carried zero
top-ups exactly as designed, and the message was still unsatisfiable.

Nothing offline could have caught it before.
`authenticate_fresh_admission_plan_v1` reconstructs and compares the *intent*,
which carries the operator's own metas — the same metas on both sides, and the
frame satisfied in both. The promotion happens later, in the v0 compiler, and
only the compiled message knows.

Two checks now close it, in the two places their causes live:

- `require_fee_payer_never_declared_readonly_v1` — pure, over the metas the
  operator has already built, run on the **first** plan **before** the prefund
  branch. This is where cohort-11 needed it: on 2026-09-01 the prefund landed
  (`4qMCqn7f…`) and only then did the admission wall, so real lamports were
  spent on a transaction that could never have worked.
- `authenticate_compiled_privileges_v1` — the full declared-against-compiled
  comparison over every instruction and coordinate, run in `build_report` and
  **only on a final plan**. A plan that still owes rent carries its own System
  transfers, which legitimately make the owner writable; checking that
  provisional shape refused every preflight of a participant who had not paid
  rent yet, which is every new one.

### Controls, read-only, spending nothing

Against the live cohort-11 market, with `--execute` withheld:

```
owner named as its own fee payer:
  instruction 2 coordinate 24 declares 3CZxcpGp... readonly, and that account
  is this transaction's FEE PAYER, which the message marks writable
  unconditionally because the fee debits it. No plan and no prefund can
  satisfy both: name a different --fee-payer

same owner, distinct fee payer, still owing rent:
  phase: planned      (no signature; the preflight path intact)
```

And the repair, on the packet that landed: `num_required_signatures 2`,
`num_readonly_signed **1**`, static key 0 the campaign payer (writable signer),
static key 1 the Position owner (**readonly signer**).

## What executed

Simulator `tools/load-simulator/simulator.py run --execute`, cohort-11's real
market, participants funded from the campaign payer and never from the deployer.

| what | signature | slot | CU | fee |
| --- | --- | ---: | ---: | ---: |
| participant-1 admission | `4dFZ9APHud96oNbZvhRCRnehjTCZZDDDmbKDBECoF1A2rm9SM1Zf1tLEgCLMfEfT4dkVc9fRN78LTNYqAdaz1CX3` | 491,815,185 | 282,940 | 80,000 |
| participant-2 rent prefund | `2Pip9hrZj9KKm7qY6NWFNtrHTrZvMWFjuDPz6Y9B5pwvYafhV8RLHSiVSikWdUntWh8nKYroLtpEZUuQenbaoCFN` | 491,815,251 | 300 | 10,000 |
| participant-2 admission | `JKdY7qzML69C4TzUcumaEoSRj6ARnjFHnmRFg3oj3zVYRboQxx1vfmP4nFjbVD8McKbfAD68jKVgW3rgj3FVa9R` | 491,815,409 | 288,940 | 80,000 |

Both admission journals are `phase: finalized`. The poststate, re-read off chain
rather than taken from the driver:

| account | address | owner | bytes |
| --- | --- | --- | ---: |
| participant-1 Position | `5SYqhNVT8hUetS6GopkxdVNXWpKfHG1CpMwwZKXzVtAP` | Claims `HQYqqdzn…` | 160 |
| participant-1 admission | `G5JTNALRHfSTDGCmJCh1V1DTYoEyQS8skkFHQxGQRoYt` | Claims `HQYqqdzn…` | 512 |
| participant-2 Position | `4AHZpdJi8RFoh2VUKGuNqzgnqqjzXA5x2naHq8t5Mjue` | Claims `HQYqqdzn…` | 160 |
| participant-2 admission | `48KkQPsugrpH5y9sAnTAFNwpeig7mKcnaQ3Lza6XZc63` | Claims `HQYqqdzn…` | 512 |

Both Positions were System-owned and data-empty before the run. The identities
are campaign-local keys held under a mode-700 job directory beside the founder's.

## The census: six laws, four boundaries

`ledger-census` ran once per cycle for four cycles. The first attempt **halted
loudly**, and it was right to:

```
VIOLATED L1: tracked 500000000 atoms across 1 accounts != Mint supply 1000000000
VIOLATED L3: Positions sum to [0,0,0,0] but the aggregate owes [500000000 x 4]
```

Neither is a protocol violation. Both are the census being asked to check a law
without the input it needs: `census.tokens` and `census.positions` were empty and
`claim_unit_atoms` was `0`, which also made L4 vacuous (`x unit 0 = 0`). The
config now names the founder's collateral wallet, the three Positions, and the
true claim unit of `1`. Naming what exists is not weakening a law; it is the same
lesson `061eaa39` drew about L8.

With the inputs supplied, at cycle 4:

```
HOLDS L1: tracked 1000000000 atoms across 2 accounts == Mint supply 1000000000
HOLDS L2: the Hoard moved 0 atoms since load-sim-cycle-000003, exactly as declared
HOLDS L3: 3 Positions sum to the aggregate supply vector [500000000 x 4]
HOLDS L4: Hoard 500000000 >= worst outcome 500000000 x unit 1 = 500000000
HOLDS L5: tracked collateral moved 0 atoms since load-sim-cycle-000003
HOLDS L6: no watched account closed at this boundary
```

**L3 counts three Positions.** Before this run it could have counted one. That is
the sentence this lane exists for.

## Cost, and the budget it was held to

Stated before spending: nothing from the deployer, and stop at any step costing
more than 2 SOL without an executing result. The simulator additionally carried
an enforced `budget.max_lamports_spent` of 500,000,000.

| | lamports | SOL |
| --- | ---: | ---: |
| deployer `4zrxtw5c…` before | 38,738,044,775 | 38.738044775 |
| **deployer after** | **38,738,044,775** | **38.738044775** |
| campaign payer before | 1,562,479,345 | 1.562479345 |
| campaign payer after | 1,562,309,345 | 1.562309345 |
| campaign payer spent (three fees, exactly) | −170,000 | −0.000170 |
| participant-2 own rent (its two PDAs) | −5,877,024 | −0.005877024 |

**The deployer did not move.** Total chain cost of demonstrating population life:
0.000170 SOL in fees plus 0.005877024 SOL of rent that now sits in the
participant's own accounts.

## What is still not demonstrated: the Direct trade

Population life here is *admission* life. No trade executed, and preparing one
found a wall bigger than the missing artifacts.

### Cohort-11's market cannot be filled, and no artifact repairs it

**The market was founded at 30 basis points.** Read off chain, not off the
staging file: the finalized `direct_execution_config_record`
`62kFf7i2vRkGEGCwdvK5AKfk18Jt56UjihoaVEqUjHNQ` (64 bytes, owner
`ADB72ar6…` = Registry) decodes `DCLTDEC1`, `price_scale 1000000`,
**`fee_basis_points 30`**, `fee_recipient EP4CMPQKidMRzA4bsbpVRx6eBgpQRpiMvU7p6uJzsYpx`.

`programs/dclutch-trading-sbf/src/direct_token_setup_v1.rs:477` refuses
`TradingSbfError::Content` unless the Market's finalized Direct config reads
**exactly** `DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1`, which is `50`
(`crates/dclutch-direct-codec/src/token_setup_v1.rs:25`). `direct_token_setup_v1`
is the sole creator of the seller's and the venue's Direct token accounts, and
it precedes every fill. So those accounts can never be created for this market,
and **this market can never take a fill** — not when the checked execution
release ships, not ever. A Direct config is a finalized Registry record; it is
immutable by construction.

This is a founding parameter, not a defect in the code that refuses it. The
stager says so in capitals already:

> `--direct-fee-basis-points` has no default and must be stated. The rate is
> sealed into the Market at founding and cannot be changed afterwards.
> **PASS 50. Not 0.** … Devnet market19 `6WZXJ7jB` was founded at 0 on
> 2026-08-30 and is permanently unfillable for that reason alone.

Three markets were already known to be unfillable this way. Cohort-11's is a
**fourth**, founded on 2026-09-01 — the day after that warning was written —
at a rate that is neither 0 nor 50. The genesis evidence file records the band,
the cuts, the coefficients and the denominator, and does not record the fee
rate; nothing read it back, and the founding succeeded, because every stage
before the fill is indifferent to it.

### And the fill has a size ceiling, which decides the ticket terms

From the same stager passage, and it constrains any trade this substrate can
demonstrate: `fee = mul_div_floor(gross, policy_fee_bps, 10_000)`, so at 50 bps
every trade whose **gross collateral is 1..=199 atoms has fee 0**, takes the
one-CPI branch, and is measured at 1,329,618..1,349,118 CU against the
1,400,000 ceiling. Any larger trade floors to a nonzero fee, takes the two-CPI
branch at 1,515,003 CU, and is **over the ceiling until the second-transaction
fee leg ships**.

The producer's own loopback defaults (`FILL_ATOMS_V1 = 100_000_000`,
`EXECUTION_PRICE_V1 = 500_000`, `direct_trade_producer.rs:103`) give
gross = 50,000,000 atoms and a fee of 250,000 — the blocked branch. A devnet
demonstration must therefore be small.

### The prepared terms

`gross = fill × price ÷ price_scale` and must divide exactly
(`exact_quote_v1`, `direct_trade_producer.rs:3396`):

| field | value | why |
| --- | ---: | --- |
| `--maximum-fill` | `100` | atoms of claim on one outcome |
| `--limit-price` | `1000000` | equals `price_scale`, so gross = fill |
| gross | `100` | ≤ 199, so the fee floors to 0 |
| fee | `0` | the one-CPI branch, inside the CU ceiling |
| `required_buyer_collateral` | `100` | `gross + fee` |
| `--fee-basis-points` | the market's own config | `terms.fee_basis_points != config.fee_basis_points()` refuses (`:2296`) |
| `--outcome` | `0` | must be `< claim_count` (4) and within the seller's balance |
| `--generation` | `1` | the Open Market identity generation |
| `--lifecycle` | `fok` | both sides; the pair gate demands lifecycle 0 |
| `--side` | `sell` / `buy` | seller side 0, buyer side 1, distinct makers |

**`--market` is the `founding_market`, not the Core Market.** The producer takes
it from `campaign_address_v1(campaign, "founding_market")`
(`direct_trade_producer.rs:2061`) — `3rBfDBpaXjKSbUU5HRaRTr6yhDQq4S1oKp2mQRsdoyb6`,
not `ARuPAuyJ…`. This is the same trap the routing table set, and it is worth
saying twice: on this cohort, "the market" is two different addresses depending
on which authority is asking.

The makers are fixed by the chain, not chosen: the seller is
`founder_position`'s owner `BmDp2LRfAUxPw6qhQr9ceGMoitMtkQf3H547iTS631rv` (key
held), and the buyer is the admitted participant's owner
`BffsiBzZYExGVFhMEQgZZjAgujEm5x55wxVUeDtHAHkC` (key held). The ticket author
never takes a key path as an argument — `--keypair-env` names an environment
variable holding it.

### The one preparable thing this lane did not do, and why

The buyer's ticket must name a `--collateral-account`, and the producer requires
that account to hold at least `required_buyer_collateral` atoms **and to carry a
delegated allowance exactly equal to it** — an equality, not a floor, because
"the allowance authorizes one trade and is spent to zero, so more is as refused
as less" (`refusing_buyer_collateral_clauses_v1`, `:745`). Participant-2 was
admitted with `collateral: null`, so it has neither. The admission driver's
`--collateral-source-owner/-account/--collateral-quantity-atoms` leg creates and
delegates it, and re-running the *same finalized journal* with those flags adds
it (`resume_admission_and_collateral` runs the collateral leg on a `Finalized`
report).

That leg was not run, deliberately: it moves real collateral out of the founder's
wallet into a delegated account bound to **this market's** trade, and this market
cannot trade. Doing it would spend atoms to produce an artifact that can never
be used.

### The exact command, and what it is waiting on

```
dclutch-local-successor-bootstrap devnet-direct-trade-produce-v1   --rpc-url <devnet https> --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG   --plan          <job>/plan.json                  --expected-plan-sha256 <hex64>   --market-input  <job>/market/market.json         --expected-market-input-sha256 <hex64>   --campaign-report <job>/market/campaign-open.json --expected-campaign-report-sha256 <hex64>   --buyer-participant <job>/sim/admissions/participant-2.json       --expected-buyer-participant-sha256 <hex64>   --checked-execution-release <release.json>       --expected-checked-execution-release-sha256 <hex64>   --seller-ticket <seller.json> --expected-seller-ticket-sha256 <hex64>   --buyer-ticket  <buyer.json>  --expected-buyer-ticket-sha256 <hex64>   --payer 3CZxcpGpWthrdcx55AciJUEPyjZzqHK7UuF7p77yPLyG   --payer-keypair <job>/keys/campaign-payer.json   --output-dir <ABSOLUTE EXISTING EMPTY DIR>
```

`--buyer-participant` is **satisfied**: `participant-2.json` is a finalized
admission report of exactly the shape it wants, and this lane produced it.

What the command is waiting on, by commit:

1. **A market founded at 50 bps.** Nothing in the tree fixes cohort-11's 30; the
   unblock is a founding, with
   `stage-devnet-sponsored-market-open.sh --direct-fee-basis-points 50`, cuts
   centred on spot. Not a release, not a rebuild.
2. **`--checked-execution-release`**, from
   `direct_hot_route_manifest::run_checked_execution_release`, which the checked
   release candidate gates — and the candidate refuses at
   **`bfc8383f`** (the trading stack-frame regression bisected below).
3. **The buyer's delegated collateral leg**, one re-run of the admission driver
   against a fillable market's participant.

### What must then be observed

Against the terms above, with the founder selling 100 atoms of outcome 0:

| observation | before | after |
| --- | --- | --- |
| seller Position claim vector | `[500000000, 500000000, 500000000, 500000000]` | `[499999900, …]` |
| buyer Position claim vector | `[0, 0, 0, 0]` | `[100, 0, 0, 0]` |
| aggregate supply vector (L3) | `[500000000 × 4]` | **unchanged** — a trade moves claims between Positions, it does not mint them |
| buyer delegated allowance | `100` | `0` — spent to zero by the one trade it authorized |
| venue fee token account | `0` | `0` at this size — the fee floors to zero, which is why this size is fillable |

Both Position vectors above are read off chain today, not asserted. The
conservation laws must then say: **L1 unchanged** (no atoms enter or leave the
tracked set, they move between named token accounts), **L3 still holds** with the
buyer Position now nonzero, **L4 still holds** (the Hoard is untouched by a claim
transfer between Positions), and **L2/L5 report the collateral that moved
between the buyer's and seller's Direct token accounts against a declared delta
rather than against zero**. Any of those failing is the finding, not the
formatting.

## The genesis release candidate: it finally ran, and it refuses

Five attempts. The first three ended for reasons that were not the candidate's;
the last two produced verdicts:

1. At `8ae2c9c9` (the previous lane): thirteen SBF links, freshness gate, all
   thirteen frame reports and provenance descriptors — then died on the stale
   successor `Cargo.lock` that `b2ac8a79` fixes, because the candidate archives
   the source of the commit it is handed and `8ae2c9c9` predates the fix.
2. At `659d6f26`: exited 1 during the registry link. **Its work directory had
   been deleted underneath it** by an unrelated `/private/tmp` sweep — the volume
   is at 96% — so the reason cannot be read and most likely *was* the deletion.
3. At `ae1c2bd4`, in a work directory a sweep cannot reach: all thirteen SBF
   links built, `SBF build freshness PASS links=13`, `build-diagnostics.txt` all
   zero *including trading*, the release tool built — and then the process group
   was torn down. (`nohup ... &` does not survive this harness and macOS has no
   `setsid`; the fourth attempt was launched with `start_new_session=True` and
   ran to completion.)
4. At `803ee31c`: **it finished, and it refuses.** Two causes, both real, and
   neither this lane's.

### The verdict, at last: two refusals

```
BUILD DIAGNOSTIC: trading emitted 3 SBF stack-frame overwrite reports
Error: A function call in method
  _ZN19dclutch_trading_sbf6hot_v328execute_authenticated_hot_v3...E
  overwrites values in the frame.
SBF build freshness PASS links=12
refusing: checked Upgrade admission requires the exact 13-link shipped set;
          enumerated 12
```

**`dclutch-dealer-sbf` was deleted at `e6b7bf1a`** — "the prototype the C-06
tier was witnessing" — taking `SHIPPED_LINKS` from thirteen to twelve.
`aa7f8892` swept the two Rust readers that went red. This shell gate was the
**third** reader, and nothing turns it red: it hardcoded `!= "13"` and refused
every candidate at HEAD after building all twelve links cleanly. Fixed in
`0f0ec379`, which also writes down the debt it does not pay — the count still
lives in two places, and the honest repair is for the successor binary to print
its shipped set so this script can compare content rather than count.

**`hot_v3::execute_authenticated_hot_v3` grew past its SBF stack frame at
`bfc8383f`** — "hot: the tail-count agreement was unsatisfiable for every
fixed-topology profile". Bisected to that single commit with two SBF builds of
`dclutch-trading-sbf` from detached worktrees:

| commit | frame-overwrite reports |
| --- | ---: |
| `bfc8383f^` (`723eed12`) | **0** |
| `bfc8383f` | **3** |

All three name the one symbol
`_ZN19dclutch_trading_sbf6hot_v328execute_authenticated_hot_v3…E`. A frame
overwrite is undefined behaviour at execution, not a warning, so the candidate
is right to withhold the artifact — and this is the hottest route the protocol
has.

**The control is closed as a refusal, not as a pass.** That is worth more than
another queueing: for two days it could not say anything, and now it names two
things nothing else was watching. Neither blocks cohort-11, which runs bytes
built at `8ae2c9c9`, where trading was clean and dealer-sbf still shipped.

With the link gate repaired at `0f0ec379`, the fifth run got past it and
refused on the substance instead, which is the whole point of repairing it:

```
BUILD DIAGNOSTIC: trading emitted 3 SBF stack-frame overwrite reports
SBF build freshness PASS links=12
refusing: 3 SBF build diagnostics; fix them at their owner, or re-run with
          --allow-build-diagnostics to record them explicitly
```

The link-count refusal had been *masking* this one. **The trading stack-frame
regression at `bfc8383f` is the open residue**, left to its owner with an exact
commit rather than a window.

Devnet evidence. Not mainnet evidence.
