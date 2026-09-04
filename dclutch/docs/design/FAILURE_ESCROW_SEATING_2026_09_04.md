# The failure escrow: what landed, and the four facts the ruling did not have

Status: **implementation note for decision 0025 item 2, lane ESCROW-2,
2026-09-04.** Decision 0025 is provisional and ember may reverse it; this note
does not rule on anything. It records what the refunding complete-set law is,
where each half of it lives, and four facts found while building it that change
what the remaining work is and, in one case, what the escrow is FOR.

## 1. What the law is

A refunding market's complete set is redefined over the ORDINARY coordinates.
The failure coordinate is seated in a Position the market derives, is burned by
the program alongside a holder's ordinary claims, and is never part of a
holder's merge.

Three statements of one law, and each names its author:

- **Lean.** `formal/dclutch-semantics/DClutchSemantics/EconomicKernel.lean`,
  the "refunding complete-set law" section. `refundingSplitPost` and
  `refundingMergePost`, the aggregate-equivalence theorems, both inverse
  directions, the seated-escrow induction
  (`a_vacant_market_is_seated` plus `the_refunding_actions_keep_the_escrow_seated`),
  L1/L3 conservation, and `a_holder_without_the_failure_coordinate_cannot_merge`
  -- the foreclosure the whole law exists because of, proved against the
  kernel's own `commandAccepts` rather than asserted. Twenty-one declarations,
  zero `sorry`, no `Classical.choice`.
- **The economic slice kernel.**
  `crates/dclutch-economic-slice-kernel/src/lib.rs:218`,
  `refunding_failure_index`, is the SOLE AUTHOR of which coordinate a refunding
  complete set seats where. `BasketAction::MintRefundingCompleteSet` and
  `MergeRefundingCompleteSet` move the aggregate exactly as their categorical
  namesakes do; only the Position each coordinate lands in differs.
- **The Claims adapter.** `ClaimsAction::MintRefundingCompleteSet = 8` and
  `MergeRefundingCompleteSet = 9`
  (`crates/dclutch-claims-svm/src/lib.rs:221`, `:228`), authenticated by
  `authenticate_failure_escrow` (`programs/dclutch-claims-sbf/src/lib.rs:1503`)
  against two codes: `FailureEscrow` (`:336`), the packet named a Position that
  is not this market's escrow, and `FailureEscrowUnseated` (`:354`), the right
  account and the wrong market shape.

The escrow's identity is the existing `ClaimsCapability` owner PDA at
`(market, failure selector)` -- no new owner kind for one use, and the same
admission the rational-representation custody owner already passes.

**The slot rule, stated once.** The escrow takes the slot the CATEGORICAL action
of the same name leaves empty: source for a refunding mint, destination for a
refunding merge. That is not a flourish. The kernel derives a merge's collateral
payout from the SOURCE owner, so putting the escrow there would pay the escrow
instead of the holder who burned the ordinary claims.

## 2. Finding: L3 forecloses "no Position at all"

Decision 0025 section 6 offered three places for the failure coordinate --
"an escrow Position, a Position nobody owns, or no Position at all". The third
is not available, and the reason is a law already running.

**L3 is an EQUALITY**, not an inequality:
`tools/gauntlet/journey/src/ledger.rs:828` reads
`now.position_totals == now.aggregate_supply`. Aggregate supply that no
Position holds makes the tracked sum fall short of a supply that did not change,
which is L3 VIOLATED at every boundary of that market's life. The economic
slice kernel's own `valid` would permit it -- it asks only that the two
projected holders not EXCEED the supply -- so nothing in the program would
object and the census would read red on a protocol that did what it was told.

So the escrow Position is mandatory. This is a fact about the instrument, not a
preference between shapes, and it removes one of the three options the ruling
listed.

## 3. Finding: the merge the ruling forecloses does not move collateral

Decision 0025 section 6 calls the escrow's foreclosure of `MergeCompleteSet` a
wall: "it is reachable and used". It is reachable. What it is not is a user act
that moves collateral.

`crates/dclutch-claims-conservation-contract/src/lib.rs:50` says so in its own
words -- *"Split and merge remain UNIMPLEMENTED as user acts"* -- and states two
defects in the route the ruling cites: a mint credits the aggregate's Hoard
SCALAR and transfers no atoms, and a merge reports its payout in complete SETS
where a Custody transfer moves atoms. Nothing on chain dispatches that crate's
magic; no operator builds it; no client can send it.

The generic `ClaimsPlanV1` route itself is marked
`ECONOMIC_SLICE_MIGRATION_ONLY` (`programs/dclutch-claims-sbf/src/lib.rs:705`),
its only remaining consumer is the General adapter's child-packet builder, and
it has **no ELF test anywhere in the tree** -- `ClaimsPlanV1` appears in five
files and none of them is a program-test.

The foreclosure is real and the Lean proves it. The thing foreclosed is smaller
than the ruling thought, and the honest consequence is that "the escrow breaks
merge" should not by itself decide between the escrow and the immobility shape.

## 4. Finding: who an outage pays is the payout scale, and the escrow is not that

This is the one that changes what the escrow is FOR, and it was found by
shipping the wrong thing first and catching it before anything founded on it.

The payout arm at `f9d40b615` put the economics in the payoff VECTOR. A
refunding basis pays one atom to every ordinary claim and **nothing to the
failure coordinate, whoever holds it**. So:

| record scale | failure column seated in | an outage pays |
| --- | --- | --- |
| legacy `1` | the founder | the founder, all of it |
| legacy `1` | the escrow | the escrow -- collateral stranded |
| refunding `w-1` | the founder | ordinary holders, refunded |
| refunding `w-1` | the escrow | ordinary holders, refunded |

**The seating does not change who is paid.** A market founded on a refunding
record refunds its ordinary holders while its failure column still sits with the
founder, which is exactly what cohort-16 will found. What the escrow prevents is
narrower and is the thing decision 0025 named in its own last paragraph: a
refunding market's failure claims are worth nothing, and until they are seated
they are worth nothing IN SOMEBODY'S HANDS -- worse than worthless, because they
are sellable to a stranger who reads a claim balance and not a payout scale.

Two consequences already taken:

- The market page's outage disclosure now reads `refundsOnFailure` off the
  market's authenticated payout scale for WHO IS PAID, and the derived escrow
  for WHERE THE COLUMN SITS, and states them as two sentences. It says outright
  when it has not read the scale rather than guessing. A disclosure keyed on the
  seating alone would have told a cohort-16 buyer that the founder takes the
  whole pool on a market whose payout vector pays the founder nothing.
- The `refund-scale` runbook row's verifier no longer requires that "the market
  page names no single failure-column holder". Founding v5 cannot deliver that,
  and the row would have failed cohort-16 for doing exactly what it was asked.

## 5. Finding: the browser was a second author of "scale must be 1"

`validateProductBasisV3` in `apps/dclutch-web/lib/directHotChain.ts` refused
`payoutScale !== 1n` outright. Every browser in the tree would have rejected
every refunding market as a noncanonical categorical basis, with a message about
`Q=1` naming nothing a reader could act on -- the same defect
`native_categorical_v1.rs` shed when the payout arm landed, standing one layer
further out and outliving it by one morning.

Fixed: `categoricalRefundsOnFailureV1` is the sole author on that side of the
wire, and the cross-language join to the program's escrow derivation is one
base58 literal asserted in both `programs/dclutch-claims-sbf/src/lib.rs` and
`marketDetail.test.ts`.

The general lesson is the one the tree already knows and pays for repeatedly:
**when a rule gets a single author in Rust, sweep the browser in the same
cycle.** `npm run abi:coverage` lists what the browser still states in its own
words, and a rule with no generated module joining it is a rule the browser can
disagree with silently.

## 6. What founding must do, and what it costs

Founding is the only thing that may establish the refunding shape -- the
issuance shape is fixed at founding (0025 section 6), and
`FailureEscrowUnseated` enforces it: a routed split may maintain a refunding
market and may never convert a categorical one.

Today `build_liability_candidates`
(`programs/dclutch-claims-sbf/src/founding_v5.rs:1719`) writes
`vec![quantity; claim_count]` into BOTH the aggregate's supply vector and the
founder's one Position. The seating needs the founder's vector to become
`[q; ordinary] ++ [0]` and a second Position to carry `[0; ordinary] ++ [q]`.

**Shape A -- founding v6.** Two more accounts (the escrow Position and its
admission), taking `CLAIMS_FOUNDING_ACCOUNT_COUNT_V5 = 31` to 33; a wider
request than the fixed 832 bytes to carry the escrow Position and admission
addresses, their rent principals, observed lamports and revisions; the escrow's
allocation and rent paid from the same credit; the post-resource digest domain
extended to hash five accounts instead of three; and a new receipt width. The
`ClaimsCapability` admission also needs a real `capability_descriptor` and
`capability_outcome`, which founding currently hard-zeroes
(`founding_v5.rs`, `build_admission_candidate`). Compute is the risk: founding's
margin is already the binding constraint by its own error documentation.

**Shape B -- seat it in the founding transaction, not in the founding
instruction.** Found as today, then a categorical merge and a refunding mint in
the same transaction leave the state the ruling wants with no wire change at
all, because `MintRefundingCompleteSet` already exists. Its cost is that the
shape stops being a founding-time immutable: a founder who omits the second
instruction has a market that looks refunding in its record and is not seated,
which is precisely the state the disclosure now reports. It also cannot pass
`FailureEscrowUnseated`, which requires the seating to already hold -- so shape B
needs that gate relaxed for the founding transaction, and relaxing it is what
would let a categorical market become a hybrid.

**Shape A is the one the ruling asks for and shape B is not a shortcut to it.**
The choice is ember's or the orchestrator's, as 0025 section 6 says of the
sibling choice, and this lane does not take it.

## 7. What is owed

1. **The founding that seats the escrow.** Until it lands,
   `FailureEscrowUnseated` refuses every refunding action on every market --
   which is the correct answer and not a working feature. Named in 0025 section
   5 already; this note adds the account-frame and wire cost above.
2. **A Claims-owned split/merge route that moves collateral.** Section 3. The
   conservation contract is written and unreachable; the route it was written
   for does not exist. Without it, "merge" is a General child effect and nothing
   a holder can do.
3. **The refunding failure walk on real ELFs**: found refunding, fill, exhaust
   the window, take the failure selector, and check that the stranger is
   refunded and the escrow draws zero. It needs (1). The generic route has no
   ELF test to extend, so this is a new program-test, not an added case.
4. **Threading `refundsOnFailure` to the market page.** The disclosure asks for
   it and nothing supplies it, so every page today prints the caveat. The
   activity read has no Product basis in hand; `inspectDirectTradeSpineV1` would
   have to carry it, or the workspace read it.
5. **The signed-delta waist.** `signed_delta_v3` expresses an arbitrary
   conservative batch, so it can already write a refunding split with no escrow
   check at all. Decision 0025 section 6 named this route for the immobility
   shape; under the escrow shape it is the hole the complete-set gate does not
   cover.
