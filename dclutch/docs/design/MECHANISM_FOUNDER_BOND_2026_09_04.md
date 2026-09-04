# The founder bond: the founder's stake against their own oracle

BOND design lane, 2026-09-04, read at `a1bf4ddf0` and written at `60ae17272`
(`git rev-parse --show-toplevel` = `/Users/ember/dev/dclutch`). **Design
only.** No program source moves; the deliverables are this note, the Lean
module `formal/dclutch-semantics/DClutchSemantics/FounderBondV1.lean` (34
theorems, 15 decided witnesses, zero `sorry`, `lake build` green at v4.30.0,
one red-proof recorded in §5), and a price. Every path:line below is HEAD.

## 0. What the tree already is, and what this note adds

Decision 0025 names the three facts that combine badly: *the founder chooses
the oracle, the window and whether there is a recovery policy; the founder
holds every failure claim; so an outage is a pre-disclosed outcome wholly owned
by the party with the most influence over whether it happens.* The escrow
removes the second fact — an outage refunds the holders and pays the founder
nothing (`EconomicKernel.lean`,
`a_founder_holding_no_ordinary_claims_is_paid_nothing`,
`the_escrow_pays_nobody_for_the_failure_coordinate`). It leaves the first fact
priced at zero. A founder who chooses a source that goes quiet still loses
nothing by it: the holders get their collateral share back, and their capital
sat locked through the window and every rung of the ladder for a flat refund.

**The bond prices the first fact.** At founding the founder posts lamports
sized to what the market's terminal costs; an honest terminal returns them in
full; an exhausted one walks them to the ordinary claims, pro rata, in the same
redemptions that pay the escrow refund. Choosing a bad oracle then costs the
founder and nobody else.

Three things the tree already has that the bond is built from, and one it
already does wrong that the bond repairs:

- **The seat prepay.** The certificate seat is funded at founding —
  `rent.minimum_balance(312)`, 2,786,520 lamports on every cohort
  (`COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md:366`) — consumed by
  the settle, and reimbursed by nothing
  (`ECONOMICS_MODELS_2026_09_04.md:472`). It is a *caller obligation checked
  only at settle*: cohort-15's market 3 was founded without it and found out six
  refusals later as `0x8002 OutputState` (`COHORT15:1700-1734`). **The bond and
  the seat are one lever seen from two sides**: both are the founder's lamports
  pledged at founding to the terminal. The seat is the certain part (the
  certificate's rent, spent either way); the bond is the contingent part
  (returned or forfeited by the certificate's kind). The bond's founding
  conjunct carries the seat's check forward to founding, where a missing
  obligation costs one refusal instead of six.
- **The first crank.** Decision 0024 item 3 keeps the crank-first order and
  discloses its cost: a single-crank market's opener eats **1,244,945**
  lamports at the cohorts' rate (`ECONOMICS_MODELS:287-291`;
  `claim_check_conservation_v1.rs:126-138`, `:176-181`).
- **The ladder.** Decision 0027's rungs are funded at founding, one Bounty
  compartment per rung, each spent exactly once when a stranger advances it —
  measured on real ELFs at `be8cac7b0`: advance 216,637 CU, exhaust 218,163,
  terminal 227,662, *"three rungs, three bounties, three compartments spent
  exactly once each."*

The bond is a founding-time commitment like the ladder and the seat, and it is
derived from the same inputs they are. Nothing in it is a typed number.

## 1. The size rule

**The bond is the founding's own projection of the terminal's cost:**

    B = S + F + Λ

    S = rent(312)                                   the certificate seat
    F = [rent(256) + rent(165)]                     the compaction opener's advance
        − [rent(128 + 8n) + rent(512) − rent(288) − cap]₊
                                                    less what the first crank repays
    Λ = Σ over rungs of the rung's Bounty quote     the ladder's funding

    rent(b) = (128 + b) × rate                      rent_exempt_reference_v1's shape
    n       = the market's outcome count (ordinary + failure)
    cap     = COMPACTION_CRANK_REWARD_LAMPORTS_V1, 200,000 today

`F` is `ClaimCheckCompactionPlanV1::new` read backwards
(`claim_check_conservation_v1.rs:126-183`): the sweep of the Position and the
admission record, less the claim check's own rent, less the cranker's capped
reward, is what the first crank can repay; the opener's advance less that is the
shortfall, and `[·]₊` is the kernel's `min`. `S` is `settle_certificate_bytes`
under the rate. `Λ` is read off the capability manifest the founding finalizes:
each rung's `funding_allocation_id` names an allocation whose Bounty quote is a
founding input (`source_recovery_policy_v2.rs:108-110`;
`CapabilityManifestV1Abi.lean:66-118`, the seven compartments), and under 0024
§3 that quote is floored by rent on the rung receipt's width — the funded
receipt is 376 bytes (`generated_source_resolution.rs:32`), so the floor is
`rent(376)`. In Lean: `bondSize`, with `seatPrepay`, `firstCrankShortfall` and
`ladderFunding` as its three terms and the widths as **parameters**, so the
module is not a second author of `CLAIM_CHECK_BYTES_V1` and its siblings
(`openerTerms.ts:35-46` pins them against the Rust by a source gate).

**The rate is the one the founding records**, not the sysvar of the moment.
Cohort-15 measured what happens otherwise: devnet moved 6,333 → 5,080 mid-life
and 491,176 lamports of one ledger's rent became an unclassified surplus
(`COHORT15:1818-1837`). The `funded-rent-recorded` row (`tools/cohort/steps.tsv`,
since 16) and `funded_rent_recovery_v1` exist so that a close prices an account
by the rate it was funded at; the bond is priced and later *observed* the same
way (§3).

### 1.1 The value on cohort-15's numbers

Cohort-15's market has four outcomes (three ordinary and the failure selector),
no recovery policy, and was founded at 6,333 lamports a byte. The other two
rates are the economics note's: 5,080 after epoch 1141, and 6,960, the kernel's
reference.

| rate | S, seat | F, first crank | Λ | **B** | in SOL | of a market lane |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 6,333 (cohort-15's founding) | 2,786,520 | 1,244,945 | 0 | **4,031,465** | **0.004031465** | 1.75% |
| 5,080 (devnet after epoch 1141) | 2,235,200 | 1,038,200 | 0 | 3,273,400 | 0.003273400 | 1.42% |
| 6,960 (kernel reference) | 3,062,400 | 1,348,400 | 0 | 4,410,800 | 0.004410800 | 1.92% |

The lane denominator is 0.230228002 SOL, cohort-14 market C's whole lamport
cost for a founded, activated, filled and armed market (`COHORT14:1741-1750`),
the same one the economics note uses. Every `S` row is the seat the cohorts
actually paid and every `F` row reproduces `ECONOMICS_MODELS:289-291` to the
lamport; all three `B` rows are decided in Lean (`cohort_fifteen_bond` and the
two `example`s beside it).

**With a ladder.** A rung's Bounty quote is a founding input; at the 0024 floor
on a 376-byte receipt it is `rent(376)` = 3,191,832 at 6,333, so a one-rung
market on cohort-15's other numbers bonds 7,223,297 and a two-rung market
10,415,129. Those two figures are the floor applied, not a rung's own quote,
and `a_rung_never_lowers_the_bond` is the only thing the Lean says about them.

### 1.2 Why terminal cost, and not a fraction of the Hoard

- **It is the one number the founding already derives.** Seat, widths, cap,
  rung quotes: every term is in the founding's hands before a lamport moves, so
  the bond can be refused at founding and can never be typed.
- **An honest founder is indifferent to it.** They lock `B` for the market's
  life and get every lamport back. A founder whose oracle goes quiet pays the
  holders exactly the cost of the terminal they made them walk. The size is the
  smallest one that makes a bad oracle cost *something real* without pricing a
  belief.
- **L8 forbids the alternative.** The Hoard is atoms of the collateral mint and
  the bond is lamports; the census exists to refuse netting one against the
  other (`ECONOMICS_MODELS:301-307`; `ledger.rs:1004-1012`). A bond
  proportional to the Hoard *in lamports* needs a price and therefore an oracle,
  which is the thing the bond exists to distrust.
- **It is not insurance, and says so.** Full collateralization means there is
  no bad debt to backstop (`UPKEEP_VAULT_V0.md` §6). The bond compensates the
  holders' locked capital and the terminal they walk; it does **not** insure
  the winnings a correct prediction would have paid, and no bond sized to a
  founding could. That exposure stays where 0025 §6 left it: disclosed.

## 2. The compartment

**None of the nine, and not a tenth.** The nine compartments are Custody token
compartments (`crates/dclutch-custody-contract/src/lib.rs:228-247`), and L8 is
stated over collateral **atoms** by class (`ledger.rs:270-280`, `:1004-1012`).
The bond never holds an atom. Putting it in an L8 class would be a category
error, and the ESCROW lane's constraint — *no compartment houses a remainder* —
is met the way the escrow met it: **the remainder is made impossible** (§3),
so there is nothing for a compartment to house. C-11's reservation of a tenth
compartment to ember is untouched.

**The bond is a lamport balance, and the census law for lamports is L7.** The
bond's account is a *watched account* declared by name and by lamport at three
boundaries: founding (`+rent + B`), each exhausted-exit redemption (`−draw`),
and the close (`−rent`, or `−rent − B` on the honest exit). This is exactly the
class cohort-15's capture exercised: L7 VIOLATED by 6,484,992 lamports, then
HOLDS once the candidate and head were declared (`COHORT15:399-420`;
`ledger.rs:78-102`, `:1076`).

**Its account is the failure escrow's own.** The bond is the lamports the
escrow account holds above its rent. Under 0025 §2 item 2's shape that is the
escrow Position cohort-16 seats the failure coordinate in, and the bond is the
*lamport side of the fact that account already owns* — what the failure
coordinate is worth, and to whom. One semantic owner per persisted fact; a
separate concept does not automatically deserve a separate account
(`AGENTS.md`, Architecture). Rent stays the founder's and returns to them on
both exits; the surplus is the bond.

If 0025 §6's open ruling goes the other way — the failure coordinate left
immobile in the founder's Position — the founder's Position cannot carry the
bond, because its lamport surplus is the founder's. Then the bond takes a
dedicated Claims PDA under a `dclutch/founder-bond/v1` domain seeded by the
aggregate, zero bytes of data, `rent(0)` = 810,624 at 6,333 returned at close.
Same exits, same theorems; one more account in the founding frame.

**Why not the funding ledger, and why CloseFund is untouched.** The funding
ledger already carries lamport principal per manifest entry and CloseFund
already classifies every lamport it discharges to the founder's beneficiary
(`resolution-core-v3-operator/src/lib.rs:3479-3495`;
`source_closure_receipt_v3.rs:51-100`;
`CapabilityFundingLedgerV2.lean:257-275`), so the honest exit would be free
there. But the exhausted exit is a *Claims* redemption, and a Resolution-owned
principal cannot be debited by Claims; a per-redemption release would need
Resolution to verify a Claims redemption it did not run. The bond therefore
lives where the walk is. **`ResolutionCloseFund` stays at its measured
252,368 CU and re-measures nothing** (`COHORT15:2335-2338`).

**Why lamports, not the payout asset.** Every term of the size rule is rent.
An atom bond would be a tenth L8 compartment with the pro-rata remainder the
escrow already found has nowhere to go, and would make the founder's stake a
function of the collateral mint's decimals.

## 3. The two exits

The certificate is written once and its kind decides the exit
(`generated_source_resolution.rs:169`, `:172`: success `1`, failure `4`). In
Lean, `exit?` is a function of the kernel's `Phase`: `none` while open,
`some .honest` on an ordinary winner, `some .exhausted` on the failure
selector, unchanged between terminal and retiring
(`retiring_keeps_the_terminal_exit`), `none` once retired.

### 3.1 Honest: returned to the founder in full

On kind `1` the escrow's own failure claims are the losing column. Their
zero-payout redemption and the account's close are one route the ESCROW lane
owes cohort-16 regardless; the bond arm is one lamport movement inside it:
**everything the account holds goes to the founder's refund wallet** — the
RentCredit's `refund_wallet`, bound at founding to `request.beneficiary()`
(`generic_founding_v1.rs:533`, `:886`) and the same beneficiary CloseFund pays.
It fires at Terminal, not at Retired: holders have no claim on the bond on this
path, so there is no reason to hold the founder's capital a slot longer.

### 3.2 Exhausted: walked to the ordinary claims, in the escrow's own walk

On kind `4` every ordinary redemption in the failure walk draws from the escrow
account

    draw = ⌊ remaining × quantity / outstanding ⌋

where `remaining` is the account's lamports above its rent **at the recorded
rate** (an observation, never a caller's number) and `outstanding` is the sum
of the ordinary coordinates of the aggregate's supply vector — which the
payout already reads coordinate by coordinate in its solvency loop
(`product_basis_terminal_v3.rs:402-425`). The redemption that retires the last
ordinary claim draws everything left (`the_last_redemption_draws_everything`).
The escrow's close on this arm **refuses while `outstanding > 0`** and then
returns rent only.

**It is the same walk as the escrow refund: one redemption, two units.**

| | the atoms side (0025) | the lamports side (this note) |
| --- | --- | --- |
| what one ordinary claim draws | one atom, exact by the founder's scale (`an_admitted_founding_makes_every_refund_exact`) | `remaining / outstanding` of what still stands, floored |
| why there is no remainder | a floored atom is refused at founding by divisibility | the last redemption draws the rest: a telescoping sum (`an_exhausting_walk_pays_the_bond_exactly`) |
| what the failure coordinate draws | nothing (`the_escrow_pays_nobody_for_the_failure_coordinate`) | nothing (`the_failure_coordinate_draws_nothing`) |
| the observation it reads | the Position's balance and the supply vector | the same, plus the account's lamports |

**Why not the escrow's own constant-per-claim shape.** The escrow made its
remainder impossible by choosing the *scale* at founding. A lamport bond has no
scale to choose: on cohort-15's numbers 4,031,465 lamports stand against
1,500,000,000 ordinary claims, so a per-claim constant is zero and the whole
bond is remainder. The share-of-what-remains rule keeps the per-claim rate
`remaining / outstanding` invariant in exact arithmetic — `(B − Bq/N)/(N − q) =
B/N` — and realizes it exactly in the aggregate over **any** partition among
holders in **any** order, with no divisibility hypothesis
(`an_exhausting_walk_pays_the_bond_exactly`, hypotheses `0 < outstanding` and
`redemptions.sum = outstanding` only).

**The one named rounding boundary is the floor**, and it is bounded at one
lamport per redemption (`draw_within_one_lamport_of_the_exact_share`,
`no_draw_exceeds_the_exact_share`). Cohort-13's own measured table, redeemed
in both orders on cohort-15's bond, is decided in Lean:

    [stranger 200, founder 1,499,999,800]  draws  [0, 4,031,465]
    [founder 1,499,999,800, stranger 200]  draws  [4,031,464, 1]

The total is exact both ways; the stranger's lamport moves with the order. A
griefer who splits into `k` Positions to farm that lamport gathers fewer than
`k` lamports for `k × 1,823,904` of Position rent — strictly negative, the
same shape as the farm cycle `ECONOMICS_MODELS:135-149` prices.

**Donations are absorbed, not stranded.** Because `remaining` is observed, a
lamport anyone sends to the escrow account mid-walk enlarges the holders' draw,
exactly as the claim-check discipline absorbs dust
(`claim_check_conservation_v1.rs:8-16`). A lamport arriving after the last
redemption and before the close leaves with the rent to the refund wallet and
is named in the close's receipt as surplus — the class `ledger_lamport_surplus`
already is (`source_closure_receipt_v3.rs:85-86`). Refusing a donation was
rejected in 0024 item 5 for the right reason and stays rejected here.

**Sleeping holders.** A compaction on the failure arm moves an ordinary claim
into a claim check; the claim check's redemption is a redemption and draws the
same way. The bond does not strand on a sleeper any more than the refund does.
This is a build item (§7), not a property the Lean states.

## 4. The crank-first order

**Does the bond repay the opener's first crank? No.** Decision 0024 item 3
stands: the single-crank opener's 1,244,945 is accepted and *disclosed*, not
reimbursed, and the terms say so on two surfaces (`openerTerms.ts`,
`OpenerFirstCrankTerms.tsx`). Repaying it from the bond would reverse that
ruling through the founder's pocket, and would un-return the bond on the honest
exit — a bond that repays a stranger's crank is not "returned to the founder in
full."

`F` is in the **size** because the size is the terminal's cost and `F` is what
the terminal costs whoever compacts a sleeper. On exhaustion it flows pro rata
to the ordinary claims; an opener who is also a holder gets a share, not a
repayment. On the honest exit the opener's shortfall is unchanged and remains
the escrow-close residue's business (`UPKEEP_VAULT_V0.md` §3;
`ECONOMICS_MODELS:323-326`, named as owed).

## 5. The properties

| | statement | Lean | status |
| --- | --- | --- | --- |
| **(a) conservation** | the bond leaves by exactly one exit, never both, never partially | `the_bond_leaves_by_exactly_one_exit`, `never_both_exits`, `an_exhausting_walk_pays_the_bond_exactly`, `the_exhausted_exit_is_the_walk`, `Walk.paid_le_remaining` | theorems |
| **(b) incentive** | the founder's loss from a source that goes quiet is `B`; a holder's exposure beyond the refund's flattening is not increased by the founder's choice and is reduced by their share of `B` | `an_exhausted_exit_pays_the_founder_nothing` and (a); the rest is §1.2's argument | (a) is the theorem; the expected-value reading is prose |
| **(c) no rent extraction** | only the founder funds it at founding; only ordinary claims receive it on failure; the failure coordinate and the founder-as-founder receive nothing | `an_admitted_founding_holds_at_least_the_bond`, `the_failure_coordinate_draws_nothing`, `an_exhausted_exit_pays_the_founder_nothing`, `an_ordinary_redemption_draws_its_share` | theorems |
| **(d) no withdrawal while live** | an open market enables no exit and no redemption draws | `no_exit_while_the_market_is_open`, `no_redemption_draws_the_bond_while_open`, `a_terminal_enables_exactly_one_exit`, `only_the_failure_selector_exhausts_the_bond` | theorems |

**Proof sketch for (a).** The settlement is a two-arm `match` on the exit, so
the aggregate identity `toFounder + toHolders = bond` and `toFounder = 0 ∨
toHolders = 0` are by cases. The walk: with the invariant *outstanding = 0 →
remaining = 0*, preserved by every feasible step because a step that retires
the last claim draws everything (`Walk.step_preserves_sound`), and the
telescoping identity *paid + remaining = what stood* (`Walk.paid_add_remaining`,
which needs only that no draw exceeds what remains,
`no_draw_exceeds_what_remains`), a sequence whose sum is the outstanding claims
ends at zero remaining and has paid the whole bond. Feasibility is not a
hypothesis: any sequence summing to the outstanding claims is feasible
(`Walk.feasible_of_sum_le`), and the aggregate refuses a Position holding more
than the supply anyway (`PositionExceedsSupply`,
`product_basis_terminal_v3.rs:410`).

**Proof sketch for (b).** By (a) the founder receives `B` under `.honest` and
`0` under `.exhausted`; having funded `B`, their loss on exhaustion is exactly
`B` and their loss on an honest terminal is the lock-up. A holder's atoms are
the escrow's business and unchanged by this note; their lamports move only by
`draw ≥ 0`. A founder whose source fails with probability `π` expects to lose
`πB`; there is no arm in which the founder gains.

**(b), stated honestly.** The refund flattens a correct prediction to its
collateral share (`basis_width − 1` atoms per complete set back, one per
claim). That is 0025's transfer and this note does not undo it: the bond
compensates lock-up and the terminal, not forgone winnings (§1.2).

**Red-proof.** `cohortFifteenWalk.paid [200, 1499999800] = 4031464` — one
lamport under the bond — fails under `decide` with *"proved that the
proposition is false"*; the witnesses are computing, not vacuous.

### 5.1 The hostiles

| hostile | what refuses it | Lean |
| --- | --- | --- |
| a founding without the bond | the founding conjunct `rent + B ≤ lamports` on the escrow account, a new Claims founding discriminant (`FounderBondUnderfunded`, next free code in the band), **red at founding** — and it carries the seat's check with it, so cohort-15 market 3's six-refusal discovery becomes one | `a_founding_one_lamport_short_refuses` |
| a withdrawal mid-life | no route moves the escrow's lamports while `Phase = Open`; the escrow has no signer; `exit?` is `none` | `no_exit_while_the_market_is_open`, `no_redemption_draws_the_bond_while_open` |
| a payout exceeding the bond | the draw is bounded by what remains for any quantity the aggregate admits | `no_draw_exceeds_what_remains` |
| a bond paid on an honest resolution | on kind `1` every redemption draws zero and the close returns everything to the refund wallet | `no_redemption_draws_the_bond_on_an_honest_terminal`, `an_honest_exit_pays_the_holders_nothing` |
| the escrow's own claims drawing on the failure arm | the failure coordinate's quantity is zero to the draw | `the_failure_coordinate_draws_nothing` |
| a close before the walk finishes | the exhausted-arm close refuses while `outstanding > 0` (an observation of the aggregate) | build conjunct; the theorem it serves is `an_exhausting_walk_pays_the_bond_exactly` |
| a stranger tops up the escrow | not refused: it enlarges the holders' draw; refusing a one-lamport transfer is the griefing vector 0024 item 5 rejected | — |
| `k` Positions to farm the rounding lamport | fewer than `k` lamports for `k × rent(Position)`; unprofitable by construction | `draw_within_one_lamport_of_the_exact_share` |

Every refusal above that reaches the chain must name its discriminant and be
proved red before green (`AGENTS.md`, Refusal codes); the two that are new are
`FounderBondUnderfunded` at founding and the exhausted-arm close's
`OrdinaryClaimsOutstanding`.

## 6. The price

**The bond.** 4,031,465 lamports = **0.004031465 SOL** on cohort-15's numbers
at its founding rate; 0.0032734 at 5,080; 0.0044108 at 6,960 (§1.1). Returned
in full on an honest terminal, so an honest founder's cost is the lock-up.

**The founding's extra cost.** Under the escrow shape the account already
exists (cohort-16, 0025); the bond adds `B` lamports to its funding and one
conjunct. The atomic founding runs at 1,184,132–1,278,747 CU, 84.6–91.3% of
the ceiling (`tools/gauntlet/CU_BUDGETS.md:9-10`), so even a conjunct is
measured, not assumed: **provisional +500 CU**, lifted by the founding
campaign's own CU-budget row on the cohort that carries it. Under the
dedicated-PDA fallback add one `create_account` CPI, **provisional +3,000–5,000
CU**, and `rent(0)` of temporary lamports returned at close.

**The honest exit.** The escrow's close, a Claims route with no measurement
yet. The nearest measured analogs are `CoreBeginRetiring` at 23,106 CU (reads
the Market, writes one byte) and the receipt prepay at 150 (a System transfer)
(`terminal_sequence.rs:430-432`); a close that authenticates the aggregate, the
Market's phase, the certificate's kind and the refund wallet, then moves
lamports, is **provisional 25,000–40,000 CU**, inside the default meter, with
the lifting plan being the measurement on the cohort that carries it. It is
also the escrow's own close, so the bond arm's marginal cost inside it is a
lamport movement: **provisional +300 CU**.

**The exhausted exit.** Per ordinary redemption, on top of the measured payout
— 235,003 CU paying, 165,591 paying zero (`COHORT15:1628-1631`), already run
above the default meter under a declared prefix (`COHORT15:2232`): one more
writable account in the frame, the `outstanding` sum inside a loop that already
reads every coordinate, one `u128` multiply-divide, two lamport writes.
**Provisional +2,000–5,000 CU per redemption**, so ~238,000–240,000 for a paying
redemption, budgeted under `CU_BUDGETS.md`'s rule (`measured + tolerance`) once
measured. Cohort-15's market — three ordinary Positions plus the escrow — walks
its exhausted exit in three redemptions and one close: roughly 0.75 M CU across
four transactions, against the 1.4 M per-transaction ceiling in none of them.

**Unchanged.** `ResolutionCloseFund` 252,368; the capture 106,810; the settle
140,902; the ladder's advance/exhaust/terminal 216,637 / 218,163 / 227,662.

## 7. What it takes to build

### 7.1 Reused as-is

The escrow account and its seeding (ESCROW, cohort-16); the certificate's kind
byte and its two constants; the payout's coordinate loop and its
`PositionExceedsSupply` / `Insolvent` refusals; the RentCredit's `refund_wallet`
and the founding's binding of it to the beneficiary; `funded_rent_recovery_v1`
for the recorded rate; L7's watched-account declarations in the journey
ledger; `outageDisclosureV1`'s reading of the escrow account
(`apps/dclutch-web/lib/marketDetail.ts:239`, twin in `packages/dclutch-sdk`).

### 7.2 New, by file

- `crates/dclutch-claims-svm/src/founding_v5.rs` — the escrow account is funded
  with `rent + bondSize(...)` computed from the founding's own inputs, and the
  founding refuses `FounderBondUnderfunded` otherwise; the seat's rent-exemption
  check moves here from the settle.
- `crates/dclutch-claims-svm/src/founder_bond_v1.rs` (new) — the size rule and
  the draw as a plan struct in the claim-check idiom: `FounderBondDrawPlanV1::new`
  refuses to exist unless `draw ≤ remaining`, and `validate_post` checks the
  observed post-balances. Arithmetic never appears inline in a route.
- `crates/dclutch-claims-svm/src/product_basis_terminal_v3.rs` and the payout
  route in `programs/dclutch-claims-sbf` — on kind `4`, the escrow account
  writable in the frame; `outstanding` summed in the existing loop; the draw
  applied. A released AccountProfile change (one more account).
- The escrow's close route (ESCROW's, cohort-16) — the two arms of §3; the
  `OrdinaryClaimsOutstanding` refusal on the exhausted arm; a receipt that
  classifies rent, bond and surplus the way `SourceClosureReceiptV3` classifies
  CloseFund's lamports.
- `crates/dclutch-claims-svm/src/claim_check_conservation_v1.rs` — a claim
  check minted on the failure arm carries its bond entitlement, so a sleeper's
  redemption draws (§3.2).
- `tools/gauntlet/journey/src/ledger.rs` — the escrow account registered as a
  watched account with declared lamport deltas at the three boundaries.
- `tools/local-validator/bootstrap/successor/src/terminal_sequence.rs` — the
  payout stage's frame; a `close-failure-escrow` stage; CU budgets declared
  under `CU_BUDGETS.md`'s rule once measured.
- `apps/dclutch-web/lib/marketDetail.ts` and its SDK twin —
  `outageDisclosureV1` gains one sentence read off the escrow account's
  lamports above `getMinimumBalanceForRentExemption(len)` at the recorded
  rate: *"if the feed goes quiet, the founder forfeits N SOL to the holders"*,
  or *"the founder posted no bond"* — derived, never typed, like the rest of it.
- `tools/cohort/steps.tsv` — a `founder-bond` row (since the cohort below):
  the escrow's lamports read back equal `rent + bondSize(...)` computed from the
  cluster's own rent, to the lamport.
- `formal/dclutch-semantics/DClutchSemantics/FounderBondV1.lean` — landed with
  this note. Its emission into Rust is not needed: the widths stay owned by the
  Rust and the theorems are over parameters.

### 7.3 The cohort

**Cohort-17.** The bond is not a record field the terminal sequence already
reads: it is a founding-frame change (the escrow's funding and conjunct), a
payout-frame change (one more account) and a close route, each an ELF. Cohort-16
carries the escrow seating and the ladder route under PROGRAMS-16D; if its
founding-frame bump is still open when this is built, the bond's account is the
same account and shares the bump — but the payout frame and the close are still
program changes, and this note does not ask cohort-16 to carry them. The first
market that shows a forfeited bond on a real chain, under a forced outage, is
the measurement that lifts every provisional figure in §6.

## 8. The question for ember

**Mandatory, or a founder's choice disclosed on the market page?**

The page can derive either: `outageDisclosureV1` already reads the escrow's
Position and supply, and the bond is the same account's lamports above rent —
*"forfeits 0.0040 SOL"* or *"posted no bond"* is one more sentence from data the
page already holds, in the refusal style when it cannot read it, exactly as
`ede3315dd` built the failure-column disclosure.

The recommendation is **mandatory at the size rule**: the bond is 1.75% of a
market's lamport lane, it is returned to every honest founder, and an optional
bond creates a class of markets a stranger has to learn to avoid. Optional
degrades honestly — an unbonded market is a founder saying they will not stake
the terminal's own cost on their own oracle, and the page would say so — but a
disclosure that has to be read is weaker than a shape that cannot be founded
otherwise, which is the argument 0025 §2 already made for the escrow.

## Evidence pointers

`formal/dclutch-semantics/DClutchSemantics/FounderBondV1.lean` (whole);
`formal/dclutch-semantics/DClutchSemantics/EconomicKernel.lean:640-1090`;
`formal/dclutch-semantics/DClutchSemantics/CapabilityFundingLedgerV2.lean:257-275`;
`formal/dclutch-semantics/DClutchSemantics/CapabilityManifestV1Abi.lean:66-118`;
`docs/decisions/0024-sustainable-economics-and-a-governable-parameter-surface.md` items 3–5;
`docs/decisions/0025-an-outage-refunds-rather-than-paying-the-founder.md` §2, §5–6;
`docs/decisions/0027-recovery-is-one-funded-ordered-ladder.md` §2, §5;
`docs/design/ECONOMICS_MODELS_2026_09_04.md:24-45, 135-149, 272-326, 465-483`;
`docs/design/UPKEEP_VAULT_V0.md` §3, §6;
`docs/design/FUNDED_CRANK_V1.md` §3;
`docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md:366,
399-420, 1621-1641, 1700-1734, 1818-1837, 2147-2152, 2232, 2335-2338`;
`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:1741-1750`;
`crates/dclutch-claims-svm/src/claim_check_conservation_v1.rs:8-16, 126-183`;
`crates/dclutch-claims-svm/src/product_basis_terminal_v3.rs:391, 402-425, 443`;
`crates/dclutch-custody-contract/src/lib.rs:228-247`;
`crates/dclutch-resolution-codec/src/source_closure_receipt_v3.rs:51-100`;
`crates/dclutch-resolution-codec/src/generated_source_resolution.rs:32, 169, 172`;
`crates/dclutch-resolution-core-v3-operator/src/lib.rs:3479-3495`;
`crates/dclutch-source-contract/src/source_recovery_policy_v2.rs:108-110`;
`programs/dclutch-core-sbf/src/generic_founding_v1.rs:533, 886`;
`tools/gauntlet/journey/src/ledger.rs:78-102, 270-280, 1004-1012, 1076`;
`tools/gauntlet/CU_BUDGETS.md:9-10`;
`tools/local-validator/bootstrap/successor/src/terminal_sequence.rs:430-432`;
`tools/cohort/steps.tsv` rows `refund-scale`, `funded-rent-recorded`;
`apps/dclutch-web/lib/openerTerms.ts:35-46`,
`apps/dclutch-web/lib/marketDetail.ts:239`;
commits `ede3315dd`, `1759b4b3c`, `be8cac7b0`.
