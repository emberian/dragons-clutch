# Decision 0034: the ensemble's flagship is k = 5, q = 3, never an even quorum, with per-member bounties

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible
on request) — the ENSEMBLE note's two questions ruled by the orchestrator on
2026-09-04 under ember's standing goal, with the ensemble designed and unbuilt,
and reversible at the cost §7 states**. It was PROVISIONAL from the ruling
until 15:50 EDT, when ember read the docket and accepted it in conversation
without amending it; the confirmation line below is the whole of what was said.
The questions are
`docs/design/MECHANISM_ENSEMBLE_RESOLUTION_2026_09_04.md:499-538`; the design
and its sorry-free Lean landed at `ff4f3b142` (`GOAL.md:4764-4781`). Direction
4 of the mechanism agenda, decision 0031.

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
CONFIRMED and no longer PROVISIONAL: accepted in conversation, unamended, and
reversible on request at the cost §7 states.

## 1. The question

A market declares `k` sources under one window and a quorum `q`. Each source is
captured independently by its own provider route; the capture is a **fragment** —
a kind-1 certificate in a per-member seat under `dclutch/ensemble-fragment/v1`.
After the deadline one permissionless **fold** refuses fewer than `q`, takes the
median of the readings on the material's one scale, and commits the cell through
`resolve_primary_from_authenticated_domain` unchanged.

The members are the leading `k − 1` attempt slots of `RecoveryPolicyV2` with the
deadline pinned to the window's; the rungs are the rest. The two ensemble bytes
are four reserved bytes `SourceMaterialV3` already requires zero, decoded as
`(k − 1, q − 1)`, **so `k = q = 1` is today's material to the byte** —
cohort-15's market 3 replays as a witness (`cohort15_market3_as_an_ensemble_of_one`).

Two things the design could not decide for itself:

1. **The flagship's `k` and `q`** — the note gives a six-row table of what each
   pair costs and tolerates (`ENSEMBLE:499-527`).
2. **Whose fee the capture is** — today nothing reimburses any captor and the
   honest capture is unpaid (`work_paid: 0`, `sponsored_push_v1.rs:1261`).

## 2. The rulings

### 2a. `k = 5, q = 3` where five independent releases exist; otherwise `k = 3, q = 3` with a relayed first rung

Read off the note's own table:

| `k` | `q` | compromised sources that move the cell, all live | under the worst outage the fold accepts | outages tolerated | withholders that force the ladder |
|---|---|---|---|---|---|
| 1 | 1 | 1 | 1 | 0 | 1 |
| 3 | 2 | 2 | 1 | 1 | 2 |
| 3 | 3 | 2 | 2 | 0 | 1 |
| **5** | **3** | **3** | **2** | **2** | **3** |
| 5 | 4 | 3 | 2 | 1 | 2 |
| 5 | 5 | 3 | 3 | 0 | 1 |

`k = 5, q = 3` **tolerates two outages** — cohort-13's Pyth receiver redeploy was
one, and it took a market's whole resolution with it — while a single compromised
source never moves the cell and two can only move it under a double outage. **It
is the only row in the table that tolerates more than one outage and still leaves
a single compromised source powerless**; every other row gives up one of the two.

**The fallback is `k = 3, q = 3` with a relayed first rung**, for the case the
note names honestly: five *independent releases* for one statistic do not always
exist. `3/3` makes any single outage the **ladder's** problem rather than the
fold's, which is where decision 0027 already put it.

`k = 3, q = 2` is the cheap shape and its cost is exact rather than vague: under
one outage a single source moves the cell. It is admissible for a cheap market
and is not the flagship.

### 2b. Never an even `q`

The bound is exact and the even case is **asymmetric**: `bracketed_below_by_at_most_half`
holds at `2·m ≤ n`, the upper bracket only at `2·m < n`, and
`exactly_half_can_move_the_cell_up_and_not_down` is the `native_decide` witness —
two honest readings in cell `0`, two manipulated readings move the fold to cell
`1` from above and cannot move it below the honest range from below.

**This is a proven property, not a rounding convention**, and a market whose
quorum is even is a market with a one-directional manipulation edge that nothing
in the fold refuses. So an even `q` is refused at authoring rather than
discouraged in a comment.

### 2c. Per-member bounties at the funded-crank floor — the note's option B

Each member's row carries a bounty in `FundingCompartment::Bounty`, released once
at the fold to the captor whose fragment the fold consumed, at the row's own
quote, by `release_in_place` — the ladder's mechanism, not a second one. The quote
is **rent-derived and never a source literal** (`FUNDED_CRANK_V1.md` §3), which is
decision 0024 item 5's floor applied here.

Option A — the sponsor who wants a decided market captures `k` sources at its own
expense — stays admissible for a cheap market. It is not the flagship, and the
reason is ember's own amendment to decision 0027: a pathway is only robust if the
crank that advances it is *"permissionless and cheap enough that a stranger with a
stake turns it."* An unpaid capture on five sources is five strangers asked to
work for nothing, and a market decided only by a path nobody will walk is not
decided.

**The seat is mandatory either way.** One prepaid, System-owned, zero-byte account
per member at 312 B — a member with no seat has nowhere to answer.

## 3. Ember's amendment

None directly: these are the note's two questions, answered by the orchestrator
under ember's standing goal, as decisions 0031, 0032 and 0033 were the same day.
Ember's words authorising the agenda are quoted in decision 0031 §3.

Ruling 2c is downstream of an amendment ember **did** make — decision 0027 §3,
the robust-pathway obligation and the funded, permissionless crank — and this
record treats it as governing rather than as advice.

## 4. The lanes

None is chartered by this record. ENSEMBLE closed as a design lane at
`ff4f3b142`; **cohort-17** founds the first ensemble market and is also the
measurement that lifts the two provisional CU rows (`ENSEMBLE:408-412`,
`:489-498`). Cohort-16 carries 0025/0027's founding changes and must not also
carry a material-layout change.

One thing this design already gave another lane: **the push route's fragment mode
IS the owed recovery-capture producer** — the missing producer RECOVERY named
when it closed — and RECOVERY-2 took it up the same afternoon.

## 5. The hostiles and laws that guard it

**Five hostiles, each with its refusing route, discriminant and Lean witness**
(`ENSEMBLE:346-382`):

1. **A fragment from a source not in the spec** — the route refuses a member byte
   `≥ k` (`EnsembleMember`, new, band `0x8`) *before* deriving a seat;
   `a_fragment_from_a_source_not_in_the_spec_refuses`.
2. **Two fragments from one source** — one seat per member, and the seat is
   write-once. **The guard is the terminal write's all-zero conjunct**
   (`programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:2126-2130`),
   *not* the seat's ownership: `initialize_certificate_at_kind` accepts an
   already-owned well-formed seat (`:2184-2191`), because the terminal is guarded
   by the Source phase moving off `Primary` and **a fragment moves no phase**. The
   fragment route must keep that conjunct or write-once is gone.
   `two_fragments_from_one_source_refuse`.
3. **A fold with fewer than `q`** — `EnsembleQuorum`; the crank is then the
   admissible move and the two are exclusive.
4. **A fragment outside the window** — `ProviderFreshness` on the push leg,
   `DeadlineElapsed` on the recovery leg, re-checked at the fold on each
   fragment's `observed_at`.
5. **A median over mixed scales** — `ProviderScale = 0x801C` at capture, so the
   fold never sees two scales. The witness for what it would cost is exact:
   cohort-15's cuts, three readings of $103.74 / $103.75 / $103.80 fold to cell 1
   on one scale and to cell 0 when two arrive as cent mantissas.

**The liveness law is proven and has no third arm.** `the_fold_never_stalls`: a
well-formed input under a positive quorum is either fewer than `q` and the
ladder, or at least `q` and decided. `the_ladder_is_the_fallback`: a market with
rungs advances by the crank's own transition, and a market with no rungs cannot
enter the ladder at all — which is the no-recovery market's own terminal. There is
no fragment count and no rung count at which a market sits with a closed window
and nothing to do.

**The crank/fold exclusivity is a route conjunct the build owes** — stated in the
note, not yet in Rust — and it is the one hostile in the set with no code behind
it today. The record names it as debt rather than leaving it to be found.

**Conservation is law-level, not yet a theorem**, and the note gives the table of
where each lamport ends: a written fragment seat closes to the Source state's
`rentBeneficiary` at retirement under L6; an unwritten one is reclaimed to the
founding's recorded payer by `ReclaimMemberSeat`, which retires the single-seat
strand decision 0024 §2 item 4 records; a consumed bounty row releases once under
L7/L8; an unconsumed one stays `Active` and returns with the ledger's remaining
principal. The census (`tools/gauntlet/journey/src/ledger.rs:1004-1012`) is the
instrument, as it is for 0025 — **no new compartment**.

## 6. What was given up, named

**`k = 5` costs about 12% more founding prepay on a cohort-15-shaped market.**
Seats and rows: 15,756,504 lamports against today's 2,786,520 — **+12,969,984,
0.0130 SOL** — plus 13,932,600 in bounties under ruling 2c, against market 3's
whole life at 0.224581914 SOL. Nearly all of it is rent that comes back. What does
**not** come back is fees: 11 transactions instead of 2, **825,000 lamports** at
cohort-15's level, spent.

**Robustness against manipulation buys weakness against withholding.** `5/3`
tolerates two outages and needs **three** colluding silent sources to force the
ladder — but `3/3` needs only one. The pair is a trade and this ruling picks one
side of it: the tree has seen an outage and has not seen a manipulation, and the
ladder is a *good* fallback, so the parameters are chosen to keep deciding
through outages rather than to minimise ladder entries.

**Ruling 2c spends founding lamports on work that may never happen.** A member
never captured releases nothing and its row stays `Active` until retirement — so
the bounty is prepaid capital idle for a market's whole life, at the funded-crank
floor rather than at a market-clearing price for the work. That floor is
rent-derived, which makes it honest and does not make it efficient.

**The two provisional CU rows are provisional.** The fold's upper bound
(`settle(1) + k × 6,000`) and its per-fragment increment are derived, not
measured; `AGENTS.md` requires a lifting plan and it is cohort-17's real-ELF
measurement at `k = 3` and `k = 5`.

## 7. The cost of reversal

**`(k, q)` is a founding-time fact in the material's own bytes**, so reversing a
market's parameters after founding is a **re-found** — the same shape as decision
0025's reversal cost, and permitted on devnet by the disposability regime and not
on mainnet.

**Reversing the flagship's parameters before it ships is free**, because the two
ensemble bytes carry any `(k, q)` and every theorem is proven for general `k`,
`q` and `n`. This is the cheapest ruling in the mechanism set to revisit, and the
note's table is the tool for revisiting it.

**Reversing 2b — admitting an even `q`** — costs a proven property. It is not a
preference: `exactly_half_can_move_the_cell_up_and_not_down` is a decided witness,
and admitting an even quorum ships a market with a one-directional manipulation
edge no conjunct refuses.

**Reversing 2c to option A** returns the honest capture to unpaid, which is the
state the tree is in today, and re-opens ember's 0027 amendment against it: a
refund reached only by a path nobody will walk is not a pathway. It saves
13,932,600 lamports of prepaid, mostly-returned capital per flagship market.

## Evidence pointers

`docs/design/MECHANISM_ENSEMBLE_RESOLUTION_2026_09_04.md:32-118`, `:119-229`,
`:230-262`, `:263-345`, `:346-382`, `:385-445`, `:446-538`;
`formal/dclutch-semantics/DClutchSemantics/EnsembleResolutionV1.lean` (whole);
`programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:2126-2130`,
`:2184-2191`, `:2408`;
`programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:134`, `:1261`;
`programs/dclutch-resolution-proof-sbf/src/funded.rs:179-239`;
`crates/dclutch-resolution-core-v3-operator/src/lib.rs:3571`;
`tools/gauntlet/journey/src/ledger.rs:44`, `:1004-1012`;
`docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md:366`,
`:892`, `:1356`, `:1392`, `:1729`, `:2150`;
`docs/design/FUNDED_CRANK_V1.md` §3;
`docs/decisions/0024-sustainable-economics-and-a-governable-parameter-surface.md`
§2 items 4 and 5;
`docs/decisions/0025-an-outage-refunds-rather-than-paying-the-founder.md`;
`docs/decisions/0027-recovery-is-one-funded-ordered-ladder.md` §3;
`docs/decisions/0030-rent-is-fixed-when-an-account-is-funded.md`;
`docs/decisions/0031-the-mechanism-agenda.md`;
`GOAL.md:4764-4781`; commits `ff4f3b142`, `12a9b13a5`.
