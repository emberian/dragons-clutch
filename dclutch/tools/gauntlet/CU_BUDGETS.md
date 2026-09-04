# Compute budgets for the golden transactions

`CU_BUDGETS.json` is the file. This is what it is for, what it can and cannot
catch, and how to change a number in it without lying.

## Why it exists

`DCLTGMF1` — the atomic founding, five stages in one rollback domain — cost
**1,184,132** CU on 2026-08-26 and **1,278,747** the next evening. That is
84.6% to 91.3% of Solana's 1,400,000 per-transaction maximum, in one evening,
from other lanes' concurrent changes to Core, Claims and Trading. Nothing in
the founding lane moved it and **nothing was watching it**.

There is no headroom to buy: the campaign already requests the maximum. At the
ceiling the failure is not slow, it is a **hard refusal with no partial
result** — which is exactly how Found31 died before `c61376d`, when it hashed
Core's 1.0 MB ELF twice on chain and exhausted 1,400,000 outright.

So: budgets, checked in, asserted by the gauntlet, with the transaction named
and the delta printed when one is exceeded.

## The shape

One file, one owner. A tier does **not** carry a copy of a number that lives
here; it carries one witness that names its campaign:

```json
{
  "id": "the-golden-transactions-are-inside-their-cu-budgets",
  "kind": "cu-budget",
  "campaign": "tier1",
  "provenance": "…"
}
```

The evaluator is a new **kind** inside the shared `tier1/check-witnesses.sh`,
not a new script — a second evaluator is a parallel authority path and the two
copies diverge on the day one of them learns something. The witness expands to
one row per budget entry, because "the campaign got more expensive" is useless
unless it says which transaction.

Five things are red, not just being over budget:

| verdict | what it means |
|---|---|
| `OVER` | the campaign consumed more than the budget. The row names the transaction and the delta. |
| `CEILING` | the budget is **above 1,400,000**. The transaction has stopped fitting and no tolerance can be written for it. |
| `MISSING` | the budget matched no transaction in the campaign. A budget that matches nothing overstates coverage, the same rule `bindings.json` lives under. |
| `AMBIGUOUS` | two transactions carry the label. A budget must name exactly one. |
| `SCHEMA` | `budget` is not `measured + tolerance`, or an enforced entry has a scope that is neither `transaction` nor `stage`. A hand-edited budget cannot drift from its stated basis. |

`RECORDED` rows are entries with `enforced: false`. They carry a required
`unenforced_reason` and are printed but never asserted.

A `stage` budget reads the chain's **own inner accounting lines**. The
evaluator walks the finalized log recovering each `consumed` line's depth from
the surrounding `invoke [n]` / `success` / `failed` lines, and a stage is the
n-th depth-2 invocation. No program address appears in the budgets file, so
stage budgets survive a run whose gauntlet-local addresses move.

## The noise, which WAS the whole problem — and is now mostly seeded away

**These numbers were not deterministic, and the reason was exact.** Every
campaign here generated fresh signing keypairs per run — `Keypair::new()` in the
successor bootstrap, `Keypair::new()` and `Pubkey::new_unique()` in the
ProgramTest fast lane. That changes how many iterations `find_program_address`
needs to find a bump, and each iteration is one `sol_create_program_address`
syscall at **1,500 CU**.

Every run-to-run delta the CU-BUDGET lane measured was an exact multiple of
1,500 (a handful of ±4 residuals aside), which is what made this a measurement
rather than a story — and what made the diagnosis actionable.

**Both campaigns now run seeded. Tier 4's band is ZERO. Tier 1's is not, and
saying otherwise would be the most expensive kind of wrong** — a file that
claims a resolution it does not have. See "Seeding, and what it does and does
not buy" below for the exact residual and its cause. The bands in the table are
kept because they are the reason the tolerances used to be what they were, and
because the second half of that section is the part nobody should forget.

Measured bands, **unseeded**, which is what this file was written against:

| what | band | how |
|---|---:|---|
| tier-1 `DCLTGMF1` | 58,494 | two campaigns 23 minutes apart, `d9f79bb` 08:27Z and `3b0c588` 08:50Z on 2026-08-27, seven ELFs byte-identical except Trading's line-number metadata |
| tier-1 `DCLTPCB1` | 51,005 | same pair |
| tier-1 `DCLTPCB1`, **within one campaign** | **79,500** | the `d9f79bb` campaign stages the ladder twice at different generations; the two differ by exactly 53 iterations on ONE binary |
| tier-1 reordered-tail refusal | 55,503 | same pair |
| tier-1 per-role activation | 1,500 – 9,000 | same pair |
| tier-1 Found31, its rollback case, the profile init, the non-terminal `DCLTPCB1` refusal | **0** | same pair |
| tier-4 founding case | 24,000 | six runs of the same ProgramTest campaign on the same ELFs |
| tier-4 late-Hoard refusal | 15,000 | same six |
| tier-4 substituted-ProgramData refusal | 6,000 | seven runs; it was 1,500 over the first **six**, and the seventh drew 4,500 higher on ELFs that had not changed a byte. Three iterations. Six runs did not bound this band. |

The tolerance rule follows from that, mechanically:

```
tolerance = roundup(observed_band, 10_000) + 10_000, floor 15_000
budget    = measured + tolerance
measured  = the HIGHEST draw observed, never a single run
```

Pinning the highest draw is what keeps ordinary noise from producing a red row.
Where the seeded band is zero the rule bottoms out at its floor: `roundup(0,
10_000) + 10_000` is 10,000, the floor lifts it to **15,000**, and `measured` is
simply the draw, because every run draws it. Where a seeded residual remains,
the rule is applied to that residual exactly as it always was.

## Seeding, and what it does and does not buy

Both campaigns take a fixed seed and derive every key from it.

**tier 1 (historical evidence)** — `dclutch-local-successor-bootstrap run
--keypair-seed <64 lowercase hex>`. The former `tools/gauntlet/run.sh` campaign
passed
`SHA-256("dclutch/gauntlet/tier1/keypair-seed/v1")`, hashed from the preimage in
the script rather than written down as a constant, and the seed is part of the
campaign stamp so changing the preimage re-runs the campaign instead of being
skipped as up to date. The derivation, from
`tools/local-validator/bootstrap/successor/src/seed.rs`:

```text
index    = keys already issued for this role in this campaign, u32 little-endian
material = SHA-256( "dclutch/local-successor-bootstrap/keypair-seed/v1"
                    || 0x00 || seed[32] || 0x00 || role || 0x00 || index )
keypair  = the ed25519 keypair whose 32-byte secret seed is `material`
```

The campaign is strictly sequential — one transaction, waited to finalized, then
the next derived from it — so "the n-th key under this role" is itself a
deterministic coordinate and needs no other state. Every 32-byte string is a
valid ed25519 secret seed, so the derivation is total.

**The safety gate is not optional and it is not this file's to relax.**
`--keypair-seed` is REFUSED unless the run spec's RPC endpoint is loopback, and
it is refused *before* any key is derived. A seed is a command-line argument: it
lives in a shell history and in this repository, so every private key it derives
is reproducible by anyone who can read either. On a public cluster that hands a
stranger the campaign's funded accounts, mint authorities and upgrade
authorities. The evidence document says which mode ran — `keypair_derivation`
is `"random-per-run"` or `"seeded-deterministic"`, and `keypair_seed_sha256`
carries the seed's digest.

**tier 4** — the ProgramTest fixture in
`programs/dclutch-core-sbf/tests/found_program_test.rs` derives its payer and its
hoard / funding-source / funding-source-replay / substituted-ProgramData
addresses from `SHA-256("dclutch/gauntlet/tier4/found-program-test/keypair-seed/v1")`
under the same shape and its own domain. No gate there, because those keys only
ever exist inside a bank in one test process: no cluster, no network, nothing
funded. The `Pubkey::new_unique()` half was the worse one — it reads a
**process-global counter**, so the address a test drew depended on how four
concurrent tests interleaved, which is why six runs did not bound the
substituted-ProgramData band.

### What seeding does NOT fix

**A KEYPAIR SEED ONLY SEEDS KEYPAIRS.** This is the correction that matters and
it was found by actually running the pair rather than by reasoning about it.

Two seeded campaigns at `5465341`, one seed digest, one set of ELFs: **82 of
101 transactions are byte-identical and 19 are not.** Almost every differing
delta is an exact multiple of 1,500, so it is still `find_program_address` bump
search — but from addresses that are not keypairs and that no keypair seed can
reach. The campaign hashes SLOT- and CLOCK-derived material into them:

- expiry slots are derived as `finalized_slot + 500_000`;
- routing address lookup tables derive from `[authority, recent_slot]`;
- the record and compartment coordinates downstream of both carry that material
  into their own content digests.

All of that moves with *when the campaign happens to run*, which is a function
of machine load. Two of the differing deltas are ±2, which is not even a bump
iteration — a length that moved by a byte.

**Confirmed again 2026-09-04, by a matched pair, and this time one delta is
NEGATIVE.** Two tier-1 runs the same evening on one laptop against byte-identical
ELFs (`claims f6ab44acb904…`), differing only in the producer: `d24c191c2` with
199 transactions, `c42da8fef` with 207, the eight extra being the Registry
reauthentication and record-abort lanes. Every one of the five `DCLTGMF3` stage
deltas is a near-exact multiple of 1,500 —

| stage | 199-tx control | 207-tx run | delta |
|---|---:|---:|---:|
| 1 Custody Lock | 74,034 | 78,532 | +4,498 |
| 2 Core FoundAndPermit | 301,430 | 305,928 | +4,498 |
| 3 Custody Realize | 50,421 | 45,919 | **−4,502** |
| 4 Claims FoundingV5 | 161,089 | 177,589 | +16,500 |
| 5 Core Open | 60,765 | 63,765 | +3,000 |

— and stage 3 got **cheaper**. That is the fact worth keeping: a compute
regression cannot make a stage cheaper, so a signed multiple-of-1,500 delta
across a whole transaction is bump-search noise and nothing else, and it can be
read off a single matched pair without bisecting anything. The mechanism is the
one above: eight extra transactions move the founding to different slots, the
slots seed the coordinates, the coordinates draw different bumps.

**What it cost to learn the row was under-pinned.** `dcltgmf3-stage-4-claims-foundingv5`
went red on the 207-transaction run at 177,589 against a 175,086 budget. Its
`measured` was ONE draw (155,086) and its provenance said so; the control alone
already sat 6,003 above that draw, spending 30% of the tolerance before anything
was added. It is re-pinned to 177,589, the highest of three draws, with the
tolerance UNCHANGED at 20,000 — widening a tolerance to absorb a draw the band
already covers is how a ratchet stops ratcheting. Its four sibling stage rows
and the whole-transaction row are still single draws from `93a2793bd` and are the
same shape; they are left alone because they are green and re-pinning a green row
upward can only cost sensitivity.

**Where the seed did and did not land, per enforced row:**

| band | rows |
|---:|---|
| **0** | all five per-role activations, the pre-revocation-Core refusal, `found31-whole` and its rollback case, the infrastructure profile init, `dcltpcb1-second-prestate-whole`, `dcltpcb1-non-terminal-refusal`, `dcltgmf1-hostile-rollback`, both `DCLTPCA1` cases — **14 of 24** |
| 2 – 1,500 | `dcltgmf1` stages 1–4, `dcltpcb1-stage-3` |
| 4,500 – 9,004 | `dcltpcb1-stage-2` (4,500), `dcltpcb1-stage-1` (6,000), `dcltgmf1-whole` (9,004) |
| 24,000 | `dcltpcb1-whole`, `dcltpcb1-reordered-tail-refusal` |

**These row ids are the 2026-08-27 campaign's, and that campaign no longer
exists.** The bands are kept because they are dated measurements of a noise
cause that has not changed; the ids are the predecessors of today's
(`dcltgmf1-*` -> `dcltgmf3-*`, `dcltpcb1-*` -> `dcltcfq1-*` and `dcltpcb2-*`,
`found31-*` -> `found37-*`, and the two `DCLTPCA1` rows deleted with their
lane). Do not read this table as naming rows that exist.

So the seed took the whole activation half of this campaign to zero, which is
the win, and left the two founding ladders — which are exactly the transactions
that carry a clock.

**And the noise is gone from the measurement long before it is gone from the
world.** A
real founder draws real keys and still pays whatever `find_program_address`
charges for them — the same 58,494–79,500 CU band the table above records. So
every `measured` value in this file is now ONE draw, and `DCLTGMF1`'s headroom
to the 1,400,000 ceiling has to be read as needing to absorb a full band **on
top of** the number written down. The gate got sharper. The ceiling risk did
not get smaller, and a seeded green row is not a statement that a stranger's
founding fits.

The second thing given up is smaller and worth naming anyway: a seeded campaign
exercises exactly one bump-index path per PDA, every run, forever. Whatever a
different draw would have found, this campaign will never find.

## What this catches, and what it does not

**A tolerance that exceeds the band cannot also catch a regression smaller than
the band.** That sentence is why this file existed in its first shape: unseeded,
tier 1's founding band was 58,494–79,500, so a +30,000 regression to
`DCLTGMF1`'s **whole-transaction** number was not reliably caught and this file
did not pretend otherwise. The teeth at that scale lived only on the stage
budgets, on the zero-band entries, and on the tier-4 fast lane.

**Seeded, every enforced row has the resolution of its own tolerance: a
regression of `tolerance + 1` CU is a red row on EVERY run, not on most.** That
is the difference the seed bought and it is the whole point of taking it. On the
floor rows it is **+15,001 CU, anywhere, always**.

It is shown rather than asserted, by cutting the budgets and re-running the
evaluator against real evidence — see "The injected-red proof" below. Measured
on the tier-4 fast lane, four seeded runs, `2026-08-27`:

| budgets file | every row |
|---|---|
| committed | OK, `observed == measured` exactly, on all four runs |
| tolerance cut to 0 | still OK — which is the band-zero proof: the draw IS the pin |
| tolerance cut to −1,000 | **OVER by exactly 1,000**, all five rows |
| tolerance cut to −15,000 | **OVER by exactly 15,000**, all five rows |

What it still does not catch is anything smaller than its tolerance, and the
15,000 floor is deliberate rather than measured: a tolerance of zero would turn
every legitimate one-instruction change into a red row and the file would be
edited into meaninglessness within a week.

And the thing W1f actually asked for is caught unconditionally: **the moment
`DCLTGMF1` gets close enough to the ceiling that its budget can no longer be
written down.** Its budget is 1,348,747, which is 51,253 CU below 1,400,000.
When the measured value passes 1,330,000 the entry becomes a `CEILING` red row
and the campaign is refused. That refusal is the point. The number 1,278,747 is
not being blessed.

## The budgets

Ceiling: **1,400,000** — Solana's per-transaction `MAX_COMPUTE_UNIT_LIMIT`. The
chain's number, not ours.

`current` is the pinned value, which is the highest draw observed on
2026-08-27. `headroom` is what is left to the ceiling from that draw.

### tier 1 — `tools/gauntlet/run.sh --mode full`, real validator, 93a2793bd

**Re-pinned 2026-09-03 against the first tier-1 run that ever COMPLETED.** The
previous table was pinned before the founding was split and named `DCLTGMF1`,
`DCLTPCB1` and `Found31`; the campaign submits `DCLTGMF3`, `DCLTCFQ1`,
`DCLTCF1A`, `DCLTPCB2` and `Found37`, and the evaluator called all thirteen of
those rows MISSING on the first run that could reach them. The `DCLTPCA1` rows
are gone entirely: that lane is no longer submitted.

**Every number below WAS one draw, and eleven of them are no longer.** This
file's rule is to pin the highest of several, and when this table was written
there was exactly one completing run in existence. The tolerances were therefore
the ones the rows they replace carried, not re-derived: the noise CAUSE is
unchanged — a keypair seed does not seed the expiry slots and lookup-table slots
the founding ladders hash — so the recorded bands still applied and nothing
licensed tightening them. **The second and third completing runs happened on
2026-09-04** (see "Three runs, one ELF set" below); the eight new rows and the
three that were RECORDED-pending-a-second-run are pinned against them. The rest
are still single draws and are still green.

### Three runs, one ELF set — 2026-09-04, at `9ae8fd53be60`

Three tier-1 campaigns, one ELF set, one seed, the campaign stage forced with
`--from campaign` so nothing between them was rebuilt. **The machine's load
average at each start is part of the measurement**, because load is what moves
this campaign's slots and the slots are what seed its bump search:

| run | started | load average | transactions | result |
|---|---|---|---:|---|
| 1 | 03:44:58Z | 6.98 7.56 8.42 | 209 | exit 0, 24 witnesses, 0 failed |
| 2 | 04:05:06Z | 9.17 8.93 10.21 | 209 | exit 0, 24 witnesses, 0 failed |
| 3 | 04:26:51Z | 16.50 29.02 33.47 | 209 | exit 0, 24 witnesses, 0 failed |

A factor of 2.4 in one-minute load across the three, and the eight new rows drew
**byte-identically on all three**. That is not luck: none of those routes runs
`find_program_address`, so none of them has a bump search to draw, and the bump
search is the only noise source this campaign has left after the keypair seed.

**What DID move, on the rows that carry a clock.** Reported here rather than
re-pinned, because CU_BUDGETS.md's own rule is that re-pinning a green row
upward can only cost sensitivity:

| row | run 1 | run 2 | run 3 | band | ÷1,500 |
|---|---:|---:|---:|---:|---:|
| `dcltgmf3-whole` | 866,574 | 827,568 | 863,574 | 39,006 | 26.0 |
| `dcltcfq1-whole` | 484,735 | 489,232 | 484,735 | 4,497 | 3.0 |
| `dcltpcb2-whole` | 617,861 | 614,859 | 617,861 | 3,002 | 2.0 |
| `found37-whole` | 190,380 | 193,380 | 190,380 | 3,000 | 2.0 |
| `found37-substituted-market-rollback` | 161,293 | 164,293 | 161,293 | 3,000 | 2.0 |

Every band is a whole number of the 1,500 CU a `sol_create_program_address`
iteration costs, to within four. **`dcltgmf3-whole` is the row to read**: its pin
is 829,068 and two of the three draws are ABOVE it, so 37,506 of its 70,000
tolerance is spent by noise before any regression is added — the same shape
`dcltgmf3-stage-4-claims-foundingv5` was in before it was re-pinned. It is still
green (866,574 against 899,068) and is deliberately left alone; the three draws
are recorded here so the next lane that touches it does not have to re-measure
them. Every other enforced tier-1 row drew band 0 on all three runs.

**Read `activation-role-trading` first.** It draws 1,200,411 CU and leaves
199,589 (14.3%) to the 1,400,000 ceiling. That row is a measurement of the ELF,
not of activation: on-chain release admission hashes whole ProgramData at about
one compute unit per two bytes, so it moves whenever Trading does. All five
activations were OVER their 2026-08-27 pins on this run, Trading by 458,466.
It is now this file's smallest headroom, and the founding it replaced in that
position — `dcltgmf1-whole`, 8.7% and shrinking — came in at 40.8%.

| budget | budget CU | current | tolerance | headroom to ceiling |
|---|---:|---:|---:|---:|
| `dcltgmf3-whole` | 899,068 | 829,068 | 70,000 | 500,932 (35.8%) |
| `dcltgmf3-stage-1-custody-lock` | 114,034 | 74,034 | 40,000 | 1,285,966 (91.9%) |
| `dcltgmf3-stage-2-core-found-and-permit` | 329,927 | 299,927 | 30,000 | 1,070,073 (76.4%) |
| `dcltgmf3-stage-3-custody-realize` | 65,921 | 45,921 | 20,000 | 1,334,079 (95.3%) |
| `dcltgmf3-stage-4-claims-foundingv5` | 175,086 | 155,086 | 20,000 | 1,224,914 (87.5%) |
| `dcltgmf3-stage-5-core-open` | 76,265 | 56,265 | 20,000 | 1,323,735 (94.6%) |
| `dcltgmf3-hostile-rollback` | 50,660 | 30,660 | 20,000 | 1,349,340 (96.4%) |
| `dcltcfq1-whole` | 586,732 | 496,732 | 90,000 | 813,268 (58.1%) |
| `dcltcfq1-stage-1-resolution-pre-market-funding` | 343,180 | 313,180 | 30,000 | 1,056,820 (75.5%) |
| `dcltpcb2-whole` | 709,359 | 619,359 | 90,000 | 690,641 (49.3%) |
| `dcltpcb2-stage-1-custody-initialize` | 283,115 | 253,115 | 30,000 | 1,116,885 (79.8%) |
| `dcltpcb2-stage-2-custody-openhoard` | 76,015 | 56,015 | 20,000 | 1,323,985 (94.6%) |
| `dcltpcb2-stage-3-custody-opensourcecompartment` | 129,976 | 79,976 | 50,000 | 1,270,024 (90.7%) |
| `dcltpcb2-reordered-tail-refusal` | 108,746 | 38,746 | 70,000 | 1,291,254 (92.2%) |
| `dcltpcb2-non-terminal-refusal` | 39,706 | 24,706 | 15,000 | 1,360,294 (97.2%) |
| `dcltcf1a-pre-expiry-cleanup-refusal` | 23,453 | 8,453 | 15,000 | 1,376,547 (98.3%) |
| `found37-whole` | 211,380 | 196,380 | 15,000 | 1,188,620 (84.9%) |
| `found37-substituted-market-rollback` | 179,293 | 164,293 | 15,000 | 1,220,707 (87.2%) |
| `core-infrastructure-profile-init` | 258,321 | 243,321 | 15,000 | 1,141,679 (81.5%) |
| `activation-role-core` | 661,772 | 641,772 | 20,000 | 738,228 (52.7%) |
| `activation-role-claims` | 749,162 | 729,162 | 20,000 | 650,838 (46.5%) |
| `activation-role-trading` | 1,220,411 | 1,200,411 | 20,000 | **179,589 (12.8%)** |
| `activation-role-resolution` | 466,563 | 446,563 | 20,000 | 933,437 (66.7%) |
| `activation-role-custody` | 348,016 | 328,016 | 20,000 | 1,051,984 (75.1%) |
| `activation-refuses-pre-revocation-core` | 650,713 | 630,713 | 20,000 | 749,287 (53.5%) |
| `core-funding-create` | 333,613 | 303,613 | 30,000 | 1,066,387 (76.2%) |
| `resolution-funding-activate` | 308,311 | 278,311 | 30,000 | 1,091,689 (78.0%) |
| `core-funding-accept` | 222,371 | 192,371 | 30,000 | 1,177,629 (84.1%) |
| `reauthenticate-role-core` | 26,337 | 11,337 | 15,000 | 1,373,663 (98.1%) |
| `reauthenticate-role-claims` | 26,337 | 11,337 | 15,000 | 1,373,663 (98.1%) |
| `reauthenticate-role-trading` | 26,337 | 11,337 | 15,000 | 1,373,663 (98.1%) |
| `reauthenticate-role-resolution` | 26,336 | 11,336 | 15,000 | 1,373,664 (98.1%) |
| `reauthenticate-role-custody` | 26,336 | 11,336 | 15,000 | 1,373,664 (98.1%) |
| `abandoned-record-begin` | 33,191 | 18,191 | 15,000 | 1,366,809 (97.6%) |
| `abort-substituted-refund-refusal` | 22,258 | 7,258 | 15,000 | 1,377,742 (98.4%) |
| `abort-reclaims-abandoned-record` | 23,503 | 8,503 | 15,000 | 1,376,497 (98.3%) |

**The eight new rows are SEVEN's two Registry routes** (`c42da8fef`,
`c226b6d95`), which read NEVER-EXECUTED in every campaign on every substrate
until that commit and which its own message left owed here: "NO CU BUDGETS for
the eight new transactions. One draw each, and CU_BUDGETS.md says pin the highest
of several." There are now three draws each and all three agree exactly, so the
tolerance is this file's 15,000 floor on all eight.

Two things about them are worth keeping. **The five reauthentications split
11,337 / 11,336 and the split is structural**: Core, Claims and Trading draw the
higher figure and Resolution and Custody the lower, identically on every run, out
of the role discriminant's own encoding. That one CU is why these are five rows
and not one wildcard. And **none of the five is a measurement of an artifact** —
unlike the five `activation-role-*` rows above, which move whenever their ELF
does. The witness `reauthentication-does-not-rehash-the-role-elf` proves that
from the chain's own numbers at a factor of twenty-eight, so a change in one of
these rows is a change in the Registry's code and nothing else.

**`abandoned-record-begin` names an address on purpose.** `publish record:
Begin *` is a BINDING pattern covering 41 publications in this campaign, and a
budget must match exactly one transaction or the evaluator returns `AMBIGUOUS`
— so the row names the abandoned record's own derived address. That address is a
PDA over the Registry's gauntlet-local program id and a body ending in the seeded
payer's pubkey, so it is stable (verified identical on all three runs) and it
moves if the seed preimage or the program-id domain moves. When it moves the row
goes `MISSING`, which is red; it cannot quietly match a different Begin.

**The three readiness-ladder rows are now ENFORCED.** They said in their own
words that they were "enforced by nothing until a second completing tier-1 run
exists" and that "that second run is the whole of what this row is waiting for."
All three of the new draws came in BELOW the `93a2793bd` figure, so `measured`
stays at the highest of the four and nothing is re-pinned downward onto a lucky
run; the bands over four draws are 16,498, 10,498 and 17,999, each within two CU
of a whole number of 1,500, and the tolerance is what `roundup(band, 10,000) +
10,000` returns rather than a number chosen to make the row green.


`activation-role-trading` is the only row whose headroom is under a fifth, and
it is a measurement of an ELF: it moves when Trading does, and it moved +458,466
between 2026-08-27 and 2026-09-03. The founding that used to hold this position
does not any more — `dcltgmf1-whole` was 8.7% and shrinking; `dcltgmf3-whole` is
40.8%, because the controller funding and its checkpoint left the transaction.

**These pins are NOT re-pinned against the seeded pair, deliberately.** Two
seeded campaigns at `5465341` came in 24/24 green with real headroom
(`dcltgmf1-whole` drew 1,176,793 and 1,185,797 against a 1,348,747 budget —
other lanes took `DCLTGMF1` well below the 1,278,747 pin). Tightening every row
onto those draws would produce a table that `activation-role-resolution`
already violated at the time, for the measured reason in "The mode caveat"
below (that row has since been re-pinned UP at `1435e08` with the reason in
its `provenance`; the JSON is the enforced authority and this table follows it),
and this file's own rule is that a budget moves with a reason recorded in
`provenance`, not ahead of one. The bands are recorded in
`CU_BUDGETS.json`'s `tolerance_rule.measured_bands` so the next re-pin does not
have to re-measure them.

### tier 4 — `tools/gauntlet/tier4/run-campaign.sh`, ProgramTest, no validator

This is the **pre-campaign** check. It drives Core's `found` plus the one-shot
permit — the same Core code that is the 433,129-CU stage 2 of `DCLTGMF1` — with
no validator and no port, in well under a minute. A Core founding regression
surfaces here instead of after a six-minute campaign that needs a validator, a
port block and a ledger of its own.

**Seeded, band 0.** Four runs of one compiled test binary
(`c4eb6fca48bdcf62…`) against five ELFs built once and unchanged
(core `d272bc7d669f0983`, custody `fe7ce5f80f4a08c8`, registry
`0033c6b55e8277dc`, rent `3486a8197af49231`, series-consume-caller
`ce1160b7035c4295`) produced the same number every time. The fourth ran under
`--test-threads 1` and is the load-bearing one: `Pubkey::new_unique()` reads a
process-global counter, so before the seed a change of thread count would have
moved the draw.

| budget | budget CU | current | tolerance | headroom to ceiling | was |
|---|---:|---:|---:|---:|---:|
| `series-consume-founds-with-permit` | 737,142 | 722,142 | 15,000 | 677,858 (48.4%) | 744,795 / 40,000 |
| `series-consume-founds-with-permit-replay-campaign` | 737,142 | 722,142 | 15,000 | 677,858 (48.4%) | 737,295 / 30,000 |
| `series-consume-late-hoard-refusal` | 697,391 | 682,391 | 15,000 | 717,609 (51.3%) | 692,942 / 30,000 |
| `series-consume-replayed-ticket-refusal` | 381,167 | 366,167 | 15,000 | 1,033,833 (73.8%) | 370,694 / 20,000 |
| `series-consume-substituted-programdata-refusal` | 198,196 | 183,196 | 15,000 | 1,216,804 (86.9%) | 189,223 / 20,000 |

Every `current` **fell**. That is not a saving and nothing got faster: it is the
bump-search iterations leaving the measurement. The two founding rows now
measure *exactly* the same, which is the honest answer — they are the same
transaction against the same prestate, and the 14,847 CU that used to separate
their pins was entirely noise.

### claims-custody — `tools/gauntlet/claims-custody/run-claims-custody.sh`, ProgramTest

Two campaigns, `claims-family-programtest` and `custody-family-programtest`,
budgeted because Claims and Custody are two of the three programs whose changes
moved `DCLTGMF1` from 84.6% to 91.3% of the ceiling in one evening.

**Band 0 on every row, and the payer is why that took two goes.** Seeding the
Custody fixture's four `Pubkey::new_unique()` addresses left **6 of 34** Custody
transactions still moving between runs, every delta a multiple of 1,500. What
was left was not an address: `context.payer` is ProgramTest's own genesis mint
keypair, freshly random every run with no public knob to seed it, and it goes
into `CustodyRequestV1.payer` and therefore into the replay and vault
derivations. With a seeded PROTOCOL payer signing beside it — `context.payer`
stays the FEE payer, where it enters no derivation — two runs agree on **15 of
15 Claims and 34 of 34 Custody** transactions.

| budget | budget CU | current | tolerance | headroom to ceiling |
|---|---:|---:|---:|---:|
| `claims-sparse-admit-transfer-close` | 576,881 | 561,881 | 15,000 | 838,119 (59.9%) |
| `claims-sparse-stage-1-admit` | 222,719 | 207,719 | 15,000 | 1,192,281 (85.2%) |
| `claims-sparse-stage-2-sparse-native-transfer` | 193,046 | 178,046 | 15,000 | 1,221,954 (87.3%) |
| `claims-sparse-stage-3-close` | 132,315 | 117,315 | 15,000 | 1,282,685 (91.6%) |
| `claims-position-admit` | 251,191 | 236,191 | 15,000 | 1,163,809 (83.1%) |
| `claims-sparse-substituted-admission-receipt-refusal` | 377,405 | 362,405 | 15,000 | 1,037,595 (74.1%) |
| `custody-legacy-open-vault` | 158,015 | 143,015 | 15,000 | 1,256,985 (89.8%) |
| `custody-token-2022-open-vault` | 149,082 | 134,082 | 15,000 | 1,265,918 (90.4%) |
| `custody-legacy-delegated-external-transfer` | 157,811 | 142,811 | 15,000 | 1,257,189 (89.8%) |
| `custody-token-2022-delegated-external-transfer` | 146,260 | 131,260 | 15,000 | 1,268,740 (90.6%) |
| `custody-legacy-close-vault` | 146,566 | 131,566 | 15,000 | 1,268,434 (90.6%) |
| `custody-token-2022-close-vault` | 140,533 | 125,533 | 15,000 | 1,274,467 (91.0%) |

Injected-red, run against the OTHER run's evidence so the determinism is
cross-checked rather than self-confirmed: tolerance cut to 0 leaves all twelve
OK — the draw IS the pin — and cut to −1,000 turns all twelve OVER.

**BUDGETS NAME LITERAL LABELS; BINDINGS USE WILDCARDS.** A binding may say
`custody *: open vault` because the census matches a family of transactions; a
budget must match **exactly one** transaction or the evaluator returns
`AMBIGUOUS`. So `custody legacy: open vault` and `custody token-2022: open
vault` are separate rows. When you add a transaction to these campaigns, the
label you pass the recorder is the string a budget will have to name.

Both profile campaigns record every transaction with a unique label — 15 in
Claims, 34 in Custody, no duplicates — so any of them can be budgeted. The six
chosen per campaign are the composed chain and its three stages, the single
admission, one hostile refusal, and the vault open/transfer/close triple in both
token profiles. A refusal is budgeted on purpose: a refusal that gets more
expensive is a refusal that can stop fitting.

### general-hot — `tools/gauntlet/general-hot/run-general-hot.sh --at`, ProgramTest on six real ELFs

**RECORDED, not enforced, and the reason is structural rather than a deferral.**
`check-witnesses.sh` evaluates a budget only for a campaign some witness names,
and this campaign has none: `tools/ci/run.sh` records why, in the same note that
records that it HAS had a runner since 2026-09-04 — six SBF links built from an
archive of a named commit plus a CU table is a campaign tier's shape and minutes
of work, not a program-test suite, and what it is waiting on is a census
binding. An `enforced: true` row here would never be asserted, which overstates
coverage exactly the way a `MISSING` row does. The rows are in the file so the
lane that writes that binding does not have to re-measure them.

Three draws, **one ELF set**, at `a6aed340ce94db8003b59eaf840fd3f3ba54f670` on
2026-09-04: the runner's own campaign plus two re-runs of the same test binary
against the same `$SBF_OUT_DIR`, so nothing between them was rebuilt. Six links,
**zero** SBF stack-frame-overwrite diagnostics.

| link | sha256 (12) | bytes |
|---|---|---:|
| `dclutch_registry_sbf` | `83c9b0e89b21` | 239,816 |
| `dclutch_trading_sbf` | `ca97232b0b7f` | 2,335,544 |
| `dclutch_core_sbf` | `e8209ccbf22b` | 1,189,112 |
| `dclutch_claims_sbf` | `6c55a117135b` | 1,397,808 |
| `dclutch_custody_sbf` | `176f8007b002` | 573,576 |
| `dclutch_general_accelerator_sbf` | `80c6a04d269b` | 307,752 |

| row | draw 1 | draw 2 | draw 3 | band |
|---|---:|---:|---:|---:|
| `general-hot-open-batch` (N=2) | 654,302 | 654,302 | 654,302 | 0 |
| `general-hot-close-batch` | 640,132 | 640,132 | 640,132 | 0 |
| `general-hot-close-batch-seal` | 602,575 | 602,575 | 602,575 | 0 |
| `general-hot-second-open-batch` | 657,302 | 657,302 | 657,302 | 0 |
| `general-hot-open-batch-n13` | 659,462 | 659,462 | 659,462 | 0 |
| `general-hot-open-batch-n258` | 677,258 | 677,258 | 677,258 | 0 |
| `general-hot-standalone-capability-seal` | 616,302 | 616,302 | 616,302 | 0 |
| `general-hot-out-of-sequence-close` (`0x4002`) | 40,730 | 40,730 | 40,730 | 0 |
| `general-hot-foreign-entry` (`0x4015`) | 118,766 | 118,766 | 118,766 | 0 |

**Band 0 on every row, and it has a cause rather than luck.** This campaign's
keypairs are fixed byte arrays (`Keypair::new_from_array`) and its slots come
from `warp_to_slot` rather than a wall clock, so the bump search that is tier
1's one remaining noise source has nothing to vary. The one row whose test still
draws FRESH keypairs is the standalone seal, and it drew identically three times
as well — consistent with a route that runs no bump search, and not a bound on
one. Every tolerance is therefore the rule's floor of 15,000.

**Read the width ladder together.** N=2, 13 and 258 are 654,302 / 659,462 /
677,258 on a frame that is 55 accounts and 151 scalars and 45 identities at
every width: 89.7 CU per outcome over 256 more outcomes, and none of it is bank
width, because `OpenBatch` declares a zero per-outcome scalar stride. The
headroom at the accepted maximum Product width is 722,742 CU (51.6%).

**And read the two refusals as the pair they are.** `general-hot-foreign-entry`
is the cohort-15 wall reproduced: that deployment's `OpenBatch` refused `0x4015`
after 128,724 CU on devnet and had never been reproducible in a harness, because
the harness founded its manifest entry from the action it was about to run.
Here the entry is a parameter and everything else is byte-for-byte the campaign
that commits, which is the positive control the refusal needs.

**This table is not comparable with the one at `6ce8929ed`** (the runner's first,
recorded in `71b5ad10c`). Six of its seven rows moved — `open-batch` −2,900,
`close-batch` −5,902, `second-open-batch` +6,100, N=258 +9,099, and the seal and
foreign-entry rows by −5 each — while `out-of-sequence-close` did not move at
all. That is the whole reason the runner takes `--at`: the ELF set is different,
because six other lanes (CHUNK-REMEASURE, CLAIMS-17, DEALER-FIX, ESCROW-2,
PROGRAMS-16E, SERIES-3) landed commits under `crates/` or `programs/` between
the two revisions. A CU figure names its commit or it names nothing.

### The tiers deliberately NOT budgeted here

Not an oversight, and each can opt in with the one witness entry `TIERS.md`
documents:

- **`direct/`** — the stateless Direct AOT accelerator runs at 1,400–2,700 CU
  per transaction. Three orders of magnitude from the ceiling; a budget would
  be decoration. It already carries
  `the-campaign-stayed-under-the-protocol-compute-ceiling`.
- **`dealer/`** — its witness file was being edited by the FAM-PROF lane while
  this one ran. Deferred rather than raced.

### Recorded, not enforced

- ~~`dcltgmf1-stage-5-open-and-outer-joins`~~ — **resolved 2026-09-03.** It was
  recorded-only because the RPC truncated the finalized log before the
  commit-last Open stage's accounting line, leaving only a subtraction (328,669
  at `d9f79bb`, 322,681 at `3b0c588`) that inherits every other row's noise. On
  the first completing run the Open's own depth-2 `consumed` line is in the log:
  56,265 CU. It is `dcltgmf3-stage-5-core-open`, an ordinary enforced stage
  budget, and the subtraction is gone. The truncation was a property of a
  transaction that FAILED at stage 4, not of the RPC.
- ~~`core-funding-create`, `resolution-funding-activate`,
  `core-funding-accept`~~ — **resolved 2026-09-04.** The post-founding readiness
  ladder was reached for the first time on 2026-09-03 and one draw is not a band,
  so the three numbers were written down and enforced by nothing "until a second
  completing tier-1 run exists". Three more happened. All three rows are enforced
  in the table above, pinned to the highest of four draws with tolerances derived
  from the observed band rather than assumed.
- The nine `general-hot` rows — a full three-draw band-0 measurement with no
  evaluator to assert it, because that campaign has no `cu-budget` witness and
  is not a `SUITE_RUNNERS` row. See its own section above; they become enforced
  the day it gets a census binding.
- `hot-canonical-bundle-phase-subtotals` — there is no green number to pin. The
  canonical Hot bundle does not pass at HEAD (tail over the 32,768-byte heap at
  phase 7; W2i's gate), and its phase subtotals need `--features hot-cu-profile`
  to turn the ten `hot_cu_checkpoint!` sites from no-ops into log lines. The
  entry carries ADR 0005's own measured table instead of a re-measurement, per
  the close-out doctrine. It becomes enforced the day W2i's heap gate is green.

### The mode caveat, measured 2026-08-27

`--record-publication transaction` is a DIFFERENT CAMPAIGN from `genesis`, and
some of these rows move between them. Two runs whose Registry, Core, Claims,
Custody and Rent ELFs were byte-identical drew per-role activation figures
differing by exact multiples of 1,500 — bump search again, from the nine extra
record-publication transactions landing the campaign on different slots.

The rows in this file are pinned against `genesis`. A `transaction`-mode run is
worth doing and its witnesses are worth reading; a single OVER row on one is not
by itself a regression, and the thing to do with one is reproduce it in
`genesis` mode before believing it.

**The exception, and it is the gate working:** on 2026-08-27 a transaction-mode
run put `activation-role-resolution` OVER by 16,672 CU, and it was real. The
Resolution artifact had grown **18,944 bytes** at `87e4590` and activation
authenticates the artifact, so the cost moved with it at roughly 1.13 CU per
byte — far outside anything the mode difference explains. Owner: the lane that
grew it. The row DID red-row and was then re-pinned at `1435e08` (measured
330,385, orchestrator-accepted, reason in `provenance`): the gate fired, the
growth was named, and the number moved with a recorded reason — that is what
the row is for.

## Re-pinning a number

A budget goes UP only with a reason, and the reason goes in `provenance`. The
honest sequence:

1. Run the campaign. **Seeded, "it is noise" is no longer an available answer.**
   Both campaigns are deterministic now: an `OVER` row on the same seed, the same
   revision and the same ELFs is a real change in what the chain executed. If you
   believe it is noise, the thing to prove is that a build input moved — a
   different ELF, a different seed preimage, a different validator version — and
   that proof goes in `provenance`.
2. Say what changed and where. "Core grew" is not a reason; "Core
   re-authenticates the Registry once more per Found stage" is.
3. Update `measured`, `tolerance` and `budget` together. The evaluator refuses
   `budget != measured + tolerance`, so they cannot drift apart silently.
4. If `measured + tolerance` now exceeds 1,400,000 you are not re-pinning a
   budget, you are recording that a transaction has stopped fitting. Do not
   shrink the tolerance to make it fit; that is the alarm going off.

## The injected-red proof

The gate has to be shown to be capable of failing, so it was cut and re-run
against real evidence. `DCLUTCH_CU_BUDGETS_OVERRIDE` points the evaluator at a
different budgets file and makes it print a three-line banner saying the run is
a demonstration and not a gate.

Against the `d9f79bb` campaign evidence, 24 witnesses:

| budgets | witness result | red rows |
|---|---|---|
| canonical | 24 checked, 0 failed, exit 0 | 0 of 23 |
| every enforced budget cut 30,000 | 24 checked, 1 failed, exit 1 | **15 of 23** |
| every enforced budget cut 100,000 | 24 checked, 1 failed, exit 1 | **23 of 23** |

A cut of `N` simulates a `+N` regression. Which rows survive a 30,000 cut is the
honest map of this gate's resolution: `dcltgmf1-whole` (tolerance 70,000),
`dcltpcb1-whole` (90,000) and the reordered-tail refusal (70,000) stay green,
while three of the four `DCLTGMF1` stage rows and every zero-band row go red.

A red row reads:

```
  OVER      found31-whole                                        237041     222041      +15000
            OVER BUDGET by 15000 CU: create canonical Found31 Market
```

## The injected-red proof, tier 1's eight new rows — 2026-09-04

The same cut, against the first of the three runs' real evidence, on the eight
rows added that day. It is the band-0 shape, and on tier 1 rather than tier 4 for
the first time:

| budgets file | every one of the eight |
|---|---|
| committed | OK, `observed == measured` exactly, on all three runs |
| tolerance cut to 0 | still OK — the draw IS the pin |
| tolerance cut to −1,000 | **OVER by exactly 1,000**, all eight |
| tolerance cut to −15,000 | **OVER by exactly 15,000**, all eight |

The middle row is the one that matters and it has never been available on tier 1
before: with the tolerance at zero the budget IS the measured value, so a green
run proves the campaign drew exactly the pinned number rather than "within
tolerance of it". Tier 1's founding ladders cannot say that and still cannot; the
Registry routes can, because they run no bump search.

## The injected-red proof, tier 4 — reproduced 2026-08-27

Re-run against four seeded runs of the tier-4 campaign, and it is the sharpest
version of this proof the file has:

| budgets file | result |
|---|---|
| committed | 5/5 OK, `observed == measured` EXACTLY, on all four runs |
| tolerance cut to 0 | still 5/5 OK — which IS the band-zero proof: the draw is the pin |
| tolerance cut to −1,000 | **5/5 OVER by exactly 1,000** |
| tolerance cut to −15,000 | **5/5 OVER by exactly 15,000** |

The middle row is the one worth understanding. With the tolerance at zero the
budget IS the measured value, so a green run proves the campaign drew exactly
the pinned number — not "within tolerance of it". That is what band 0 means and
it is the thing the seed bought.

## The owner-decision this lane surfaced — TAKEN, 2026-08-27

The CU-BUDGET lane ended on this, and did not take it because it was read-only
toward both files:

> **Seed the campaign fixtures and every tolerance here collapses.** If
> `dclutch-local-successor-bootstrap run` took a `--keypair-seed`, the tier-1
> band would go to zero, every tolerance could drop to the 15,000 floor, and a
> +30,000 regression to `DCLTGMF1` would be red on every run instead of on most.
> The same is true of ProgramTest's genesis payer for the tier-4 fast lane.

ember took it. `--keypair-seed` exists on the bootstrap behind a loopback-only
refusal that fires BEFORE any key is derived, and the tier-4 and claims-custody
ProgramTest fixtures derive their keys from documented preimages.

**What it bought, measured rather than predicted:** tier 4 went to band 0 on
every row and its tolerances are the 15,000 floor. Tier 1 went to band 0 on 14
of its 24 enforced rows — the whole activation half — and did NOT collapse on
the two founding ladders.

**The prediction in that quote was wrong about tier 1, and the reason is worth
keeping.** A keypair seed only seeds keypairs. The founding ladders hash
slot- and clock-derived material into their addresses, so the way to shrink
those rows further is to make the campaign's expiry and lookup-table inputs a
function of the seed rather than of the wall clock — a change to what the
campaign SUBMITS, not to how it draws keys. Not obviously worth doing: a
founding is exactly where a real founder pays that noise too.

And the half that is not good news stands unchanged: the world is as noisy as it
ever was, and a seeded green row says nothing about a stranger's founding
fitting under the ceiling.
