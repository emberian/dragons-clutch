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

**tier 1** — `dclutch-local-successor-bootstrap run --keypair-seed <64 lowercase
hex>`. `tools/gauntlet/run.sh` passes
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

**Where the seed did and did not land, per enforced row:**

| band | rows |
|---:|---|
| **0** | all five per-role activations, the pre-revocation-Core refusal, `found31-whole` and its rollback case, the infrastructure profile init, `dcltpcb1-second-prestate-whole`, `dcltpcb1-non-terminal-refusal`, `dcltgmf1-hostile-rollback`, both `DCLTPCA1` cases — **14 of 24** |
| 2 – 1,500 | `dcltgmf1` stages 1–4, `dcltpcb1-stage-3` |
| 4,500 – 9,004 | `dcltpcb1-stage-2` (4,500), `dcltpcb1-stage-1` (6,000), `dcltgmf1-whole` (9,004) |
| 24,000 | `dcltpcb1-whole`, `dcltpcb1-reordered-tail-refusal` |

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

### tier 1 — `tools/gauntlet/run.sh --mode full`, real validator

| budget | budget CU | current | tolerance | headroom to ceiling |
|---|---:|---:|---:|---:|
| `dcltgmf1-whole` | 1,348,747 | 1,278,747 | 70,000 | **121,253 (8.7%)** |
| `dcltgmf1-stage-1-custody-lock` | 184,840 | 144,840 | 40,000 | 1,255,160 (89.7%) |
| `dcltgmf1-stage-2-core-found-and-permit` | 463,129 | 433,129 | 30,000 | 966,871 (69.1%) |
| `dcltgmf1-stage-3-custody-realize` | 123,858 | 103,858 | 20,000 | 1,296,142 (92.6%) |
| `dcltgmf1-stage-4-claims-foundingv5` | 287,951 | 267,951 | 20,000 | 1,132,049 (80.9%) |
| `dcltgmf1-stage-5-open-and-outer-joins` | — | — | — | RECORDED, not enforced |
| `dcltgmf1-hostile-rollback` | 52,686 | 32,686 | 20,000 | 1,367,314 (97.7%) |
| `dcltpcb1-whole` | 935,307 | 845,307 | 90,000 | 554,693 (39.6%) |
| `dcltpcb1-stage-1-custody-initialize` | 384,337 | 354,337 | 30,000 | 1,045,663 (74.7%) |
| `dcltpcb1-stage-2-custody-openhoard` | 135,594 | 115,594 | 20,000 | 1,284,406 (91.7%) |
| `dcltpcb1-stage-3-custody-opensourcecompartment` | 209,108 | 159,108 | 50,000 | 1,240,892 (88.6%) |
| `dcltpcb1-second-prestate-whole` | 882,807 | 792,807 | 90,000 | 607,193 (43.4%) |
| `dcltpcb1-reordered-tail-refusal` | 851,102 | 781,102 | 70,000 | 618,898 (44.2%) |
| `dcltpcb1-non-terminal-refusal` | 37,176 | 22,176 | 15,000 | 1,377,824 (98.4%) |
| `found31-whole` | 252,041 | 237,041 | 15,000 | 1,162,959 (83.1%) |
| `found31-substituted-market-rollback` | 158,399 | 143,399 | 15,000 | 1,256,601 (89.8%) |
| `core-infrastructure-profile-init` | 244,835 | 229,835 | 15,000 | 1,170,165 (83.6%) |
| `activation-role-core` | 566,984 | 546,984 | 20,000 | 853,016 (60.9%) |
| `activation-role-claims` | 593,441 | 573,441 | 20,000 | 826,559 (59.0%) |
| `activation-role-trading` | 741,945 | 721,945 | 20,000 | 678,055 (48.4%) |
| `activation-role-resolution` | 313,713 | 293,713 | 20,000 | 1,106,287 (79.0%) |
| `activation-role-custody` | 255,103 | 235,103 | 20,000 | 1,164,897 (83.2%) |
| `activation-refuses-pre-revocation-core` | 555,927 | 535,927 | 20,000 | 864,073 (61.7%) |
| `dcltpca1-unwind` | 189,496 | 159,496 | 30,000 | 1,240,504 (88.6%) |
| `dcltpca1-pre-expiry-refusal` | 162,166 | 142,166 | 20,000 | 1,257,834 (89.8%) |

`DCLTGMF1` is the only row whose headroom is in single-digit percent, and it is
**shrinking**: 15.4% at `cd05331`, 8.7% at `d9f79bb`, in one evening.

**These pins are NOT re-pinned against the seeded pair, deliberately.** Two
seeded campaigns at `5465341` came in 24/24 green with real headroom
(`dcltgmf1-whole` drew 1,176,793 and 1,185,797 against a 1,348,747 budget —
other lanes took `DCLTGMF1` well below the 1,278,747 pin). Tightening every row
onto those draws would produce a table that `activation-role-resolution`
ALREADY violates at HEAD, for the measured reason in "The mode caveat" below,
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

- `dcltgmf1-stage-5-open-and-outer-joins` — the RPC truncates the finalized log
  before the commit-last Open stage's own accounting line, so the only figure
  available is arithmetic (whole − 300 for the two ComputeBudget instructions −
  the four measured stages): 328,669 at `d9f79bb`, 322,681 at `3b0c588`. A
  subtraction inherits every other row's noise and would red-row on all of it at
  once. Budgetable the day a producer surfaces the untruncated log.
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
grew it. **This row will red-row the next `genesis`-mode campaign too**, and it
should: that is what the row is for.

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
