# Decision 0030: an account's rent is fixed when it is funded, and the fact persisted is the RATE

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible
on request) — RULED by the orchestrator on 2026-09-04 under ember's standing
goal, put to ember on the docket as D8 the same morning, not objected to at the
10:15 EDT reading, landed the same day, and reversible at the cost §7 states**.
It was RULED from the morning of 2026-09-04 until 15:50 EDT, when ember read
the docket and accepted it in conversation without amending it; the
confirmation line below is the whole of what was said. The ruling is
`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4471-4475`, carrying the standing formula *"RULING (under the standing
goal; ember may reverse)"*. Landed at `c0a1586b1`, `4137ec0d3`, `8a0d3f893`,
`315c1df2e` (lane PROGRAMS-16) with the cohort-15 host recovery at `afab02c25`
and `ec373d90d` (lane COHORT-15F). It is the only one of the eight docket
rulings that arrived from a live wall rather than from a queue.

**Confirmed, 2026-09-04 15:50 EDT.** Ember, after reading the docket and the
mechanism cohort page:

> you aren't waiting on me for rulings are you? i was reading the docket and
> contemplating it, but overall find your takes reasonable

The orchestrator's reply: nothing was waiting on ember — the rulings were
provisional and already in force, and the lanes had been working under them
since they were made; *"overall find your takes reasonable"* is taken as
confirmation rather than as an invitation to re-argue them; and the one thing
still genuinely ember's is the flagship conditional market's feature gate, its
slot and its metric (decision 0029's tenth item). So the status above is
CONFIRMED and no longer RULED: accepted in conversation, unamended, and
reversible on request at the cost §7 states.

## 1. The defect

Devnet's rent-exempt rate **fell from 6,333 to 5,080 lamports per byte at epoch
1141** (slot 492,912,000), between cohort-15 market 1's terminal admission and
market 3's settle.

Every exact-rent check on the terminal path compares an account's lamports
against **the live sysvar's** minimum plus principal, with equality. So every
account the cohort funded refuses by exactly the rent difference: **491,176
lamports on a 264-byte ledger** — the surplus is the difference to the lamport
(`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4461-4466`). The operator reads the live sysvar, and
`resolution.rs:1270` makes the same call, so no host-side change lands the
payout. Widening the comparison to `≥` would admit a donation as custody, which
the conservation laws forbid.

The consequence was concrete: the stranger's payout on cohort-15 market 3 sat
**one conjunct away** with the ATA created, the certificate kind 1, selector 1
and the buyer holding outcome 1; market 1's retirement was five exact rent
guards deep, and a test defends that exactness (the lane loosened two guards,
went red, and reverted).

**The brief that reached ember was wrong on one point and the instrument
corrected it:** the refusal was described as a founding-time constant against a
live sysvar. It is the opposite — the operator already reads the live sysvar,
and the accounts carry the old rate.

## 2. The ruling, verbatim

> **RULING (under the standing goal; ember may reverse): an account's rent is
> fixed when it is funded, and every exactness check compares against the rent
> it was funded at, never the sysvar of the moment** — persisted in the
> account's own record (Lean-first), the live sysvar read only for accounts
> created now.
> — `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4471-4475`

Ember read the docket at 10:15 EDT the same morning and did not object; D8 was
the one item put as *"needs you · ruled provisionally"*, and the amendments
recorded at `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4652-4656` touch D1, D2, D4, D5, D6, D7 and
`supported_builders`, and not this. Silence is never a ruling
(`docs/MASTER_COMPLETION_CONTRACT.md:180`), so this record stays the
orchestrator's until ember says otherwise — the difference between an unopposed
ruling and a ratified one is exactly what this status line is for.

## 3. What changed, and the one thing the ruling did not anticipate

**The fact to persist is the RATE, not the minimum.** `minimum_balance(len) =
(128 + len) × rate`, so one `u32` reconstructs every width. It fits **the four
bytes the ledger header had already reserved**; Lean owns the layout as
`LedgerHeaderFieldV2.fundedRentRate` at coordinate `(12, 4)`
(`formal/dclutch-semantics/DClutchSemantics/CapabilityManifestV1Abi.lean:326`,
`:333`, `:351`, `:654`), with five theorems, and **cohort-15's 491,176 is now a
corollary** rather than an incident.

That is the ruling's own refinement of itself: it said *persisted in the
account's own record*, and persisting the minimum would have needed a field per
width. All fifteen production `validate_native_custody` sites check a
`FundingLedgerV2`, so **one header field serves every one of them**.

The discipline that follows: **pre-existing-account sites price from the record;
creating sites read the sysvar and record what they paid.** The terminal session
v2 records the rate and the sequence never reads the sysvar again. Refusals:
`CoreSbfError::FundedRent = 0x301D`
(`programs/dclutch-core-sbf/src/lib.rs:244`) and
`TradingSbfError::FundedRent = 0x4029`
(`programs/dclutch-trading-sbf/src/lib.rs:537`).

**A cohort already on chain records no rate, and its own bytes still name one.**
`afab02c25` adds `funded_rent_recovery_v1`: `rate = (lamports − principal) / (128
+ len)`, **exact division or `FundedRentUnrecoverable`** — five accounts at five
different widths all derive 6,333, which is the cross-check that the recovery is
reading a fact rather than fitting one. The session records the recovered rate
(`ec373d90d`), so cohort-15 gets the ruling without a redeploy.

**A correction to the evidence, landed with it:** addendum E said Core runs the
conjunct on `AdmitTerminal`. It does not — **CreateFund only**. The wall
cohort-15 actually hit is the operator's planner, a host
(`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4512-4514`).

## 4. What it saved

The stranger's payout and the retirement path. Cohort-15's first stranger was
paid on an honest selector the same morning (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4541`), and the first
`ResolutionCloseFund` on any chain was certified at 08:25
(`890b58886`; `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4645-4649`). Neither was reachable while every account
the cohort funded refused by the rent difference.

It also converted a whole class of latent breakage into a censused one. The
RELEASE-PREFLIGHT lane inverted the deploy preflight under this ruling —
cohort-15's fourteen program accounts hold exactly `(128+len) × 6,333` while
devnet quotes 5,080 and `Rent::default()` says 6,960, *"one cohort, three rates,
one of them funded it"* — and **genuine pre-existing floors fell from 122 to 9**,
with 13 floors converted (three of them permit-expiry refunds the floor had been
stranding permanently) and 9 kept as *creating* sites with the runtime's
precondition cited. A third spelling the census could not see — a field holding
today's minimum — was found by the same pass.

## 5. The hostiles and laws that guard it

**The epoch program-test on real ELFs**, which is the whole ruling in one run:
accounts **funded at 6,960**, the **sysvar dropped to 5,080 mid-test**, and the
terminal admission **commits** (`4137ec0d3`). The rate moving inside the test is
the positive control — a test where the rate never moves cannot tell "the ruling
works" from "the instrument is disconnected".

**Exact division or refuse**, in the recovery: a ledger whose lamports are not
`principal + (128 + len) × rate` for an integral rate yields
`FundedRentUnrecoverable` rather than a plausible number. The five-widths
agreement at 6,333 is the corroboration.

**The five guards keep their exactness.** The test that defends it proved itself
by refusing the loosening: the lane widened two guards, went red, and reverted.
The ruling does not relax any comparison; it changes what each one compares
against.

**Lean owns the layout**, so the field cannot drift between the header, the Rust
and the emitted twin, and the five theorems make the reserved-bytes claim
checkable rather than asserted.

**Still owed** and named rather than left to be found: the ~112 `is_exempt`
floor sites over pre-existing accounts that a rate **rise** would break — the
symmetric direction, censused by class and fixed at the author under this ruling
by the RENT-FLOORS lane, with a rate-rise program-test as its control; and one
third-spelling site in the immutable registry.

## 6. What was given up, named

**A `u32` of header space** that was reserved and is now spent, in every funding
ledger.

**Two facts where there was one.** An account now carries the rate it was funded
at *and* lives on a cluster with a current rate, and every site has to know
which question it is asking. The discipline — *pre-existing prices from the
record, creating reads the sysvar* — is a rule a reader must hold, and the
refusal codes exist because it can be got wrong.

**Cohorts founded before this record recorded nothing**, so they depend on
recovery-by-arithmetic rather than on a written fact. That works because the
division is exact or refuses, but it is a weaker provenance than the record, and
it is available only while `principal` and `len` are known.

## 7. The cost of reversal

The two alternatives the docket named, with what each costs:

- **The at-least form (`≥`).** Admits a real donation as custody, which the
  conservation laws forbid — the surplus would have to be *classified* rather
  than tolerated, which needs its own conservation law and a home for the
  classified lamports. Decision 0024's upkeep vault is the only candidate home
  and it has no code.
- **Re-fund every account at the new rate on each epoch change.** An operator
  sweep nothing owns, on every account of every live cohort, triggered by a
  cluster event no program observes.

Reverting the landed shape also costs the header field, the five Lean theorems,
the two refusal codes, the epoch program-test, and cohort-15's recovered rate —
and it returns the tree to the state where a cluster-side rate change strands
every funded account of every cohort at the next terminal step, which is where
this ruling found it.

## Evidence pointers

`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4460-4476`, `:4505-4519`, `:4541`, `:4600-4612`, `:4645-4649`;
commits `c0a1586b1`, `4137ec0d3`, `8a0d3f893`, `315c1df2e`, `afab02c25`,
`ec373d90d`, `890b58886`, `1fd3e3c3f`, `ace5d24e9`;
`formal/dclutch-semantics/DClutchSemantics/CapabilityManifestV1Abi.lean:326`,
`:333`, `:351`, `:654`;
`programs/dclutch-core-sbf/src/lib.rs:244`;
`programs/dclutch-trading-sbf/src/lib.rs:537`;
`crates/dclutch-resolution-core-v3-operator/tests/funded_rent_recovery_v1.rs`;
`docs/MASTER_COMPLETION_CONTRACT.md:180`.
