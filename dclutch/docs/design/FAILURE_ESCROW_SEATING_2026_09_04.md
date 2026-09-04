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

## 8. What shape A actually cost, and where it landed

Added by lane CLAIMS-17 on 2026-09-04, after the orchestrator ruled shape A.
Section 6 above priced shape A; three of its four estimates were wrong in the
same direction, and the fourth was wrong in the other.

**The wire did not move at all.** Not the request magic, version, width or any
field; not the receipt. Section 6 priced "a wider request than the fixed 832
bytes to carry the escrow Position and admission addresses, their rent
principals, observed lamports and revisions" — and five of those six are
DERIVED. Both addresses come from the Market and its runtime width. Both rent
principals are the founder's own, because the escrow's Position and admission
have the founder's widths. Both revisions are the vacant-zero-to-live-one every
founding writes. Only the observed lamports would have needed a field, and they
are deliberately not pinned: nothing upstream joins them, and pinning them would
hand anyone who can read a derivation a founding-time denial by sending one
lamport to the address first. `ClaimsFoundingRequestV5` is unchanged.

**The receipt did not move either, and the transcript domain stayed at v5 on
purpose.** The post-resource transcript now hashes five Claims accounts instead
of three. A categorical founding allocates neither escrow account, so both
contribute zero bytes and the digest is exactly what three accounts produced
before the escrow existed. That is what makes a categorical founding
byte-identical across the change — aggregate, Position, admission, request,
receipt and transcript alike — and a fresh domain would have destroyed it for no
reader's benefit.

**The shape is fixed by the RECORD, not by a wire bit.** `refunds_on_failure`
is carried out of the Market-wide basis admission, which already decoded the
authenticated `ProductBasisV3` and threw the answer away. No caller states the
shape; no second author spells the rule.

**What did move is the account frame, in five places rather than one, and that
is the part section 6 under-priced.** Claims founding goes 31 → 33 with both
escrow accounts APPENDED, so no existing index moves. Core's generic Open
window goes 21 → 23 and its Series Open window 37 → 39, both read-only, because
both re-verify a receipt whose transcript now covers five accounts. Trading
mirrors that transcript on both founding routes. The host driver derives,
pre-funds and supplies the pair. Five frames, four of them outside Claims.

**Two costs section 6 did not name.**

- **Two account locks.** The composed `DCLTGMF3` message compiles to 60 locks
  where it compiled to 58, against an unchanged devnet limit of 64. A founding
  that could carry six physical funding entries now carries four. The SDK and
  browser tests state both numbers rather than leaving a cohort to discover it.
- **A narrowing.** Every founding names its Market's escrow even when the record
  is categorical, so the economic slice kernel's width-two structural floor
  becomes founding's: a width-one market can no longer be founded. Nothing in
  the tree founds one, and a one-outcome market is not a partition, but it is a
  narrowing and not a no-op.

**Where it landed.** `ebbccbd4e` (the five program frames, the layout, the
hostiles) and `266c1d687` (the host driver). `FailureEscrowIdentityV1::derive`
in `programs/dclutch-claims-sbf/src/lib.rs` is now the sole author of a Market's
escrow identity, called by the founding that SEATS it and by the complete-set
gate that requires it to STAY seated; a founding whose escrow account is not the
derived one refuses `0x5010 FailureEscrow`, the same code and the same
accusation as the routed split's, one stage earlier.
`refunding_founding_vectors_v1` in `founding_v5.rs` is the layout alone,
extracted so a test can read it without an account frame.

**What this section does NOT claim.** There is no ELF evidence. The tree has no
program-test that executes a Claims founding at all — the only founding ELF test
is Core's Found stage (`programs/dclutch-core-sbf/tests/found_program_test.rs`),
which stops before Claims — so founding v5's Claims half was never covered
either, and v6 inherits that hole rather than opening it. The evidence level for
this change is: unit tests over the layout and the derivation, a green build of
every affected crate and SBF link, and green SDK and browser frame suites.
Cohort-17 is the first evidence that it founds, and `escrow-seated` in
`tools/cohort/steps.tsv` is the row that says what to read off the chain.

**The frame ratchet is red** for the Claims, Core and Trading links as of
`ebbccbd4e`, which says so in its own message. It was left red rather than
recaptured because three other lanes were landing program commits in the same
hour and a recapture riding one of them names the wrong commit — the exact
failure `tools/frameguard/run.sh` documents from 2026-09-02.

## 9. What section 7 still owes, restated with what is now known

Item 1 is done. The rest, sharpened:

2. **A Claims-owned split/merge route that moves collateral.** ~~Owed.~~
   **The dispatcher landed at `4f847be64`**, and one sentence of this item was
   wrong when it was written: it said the contract "does not yet know the
   refunding shape" and needed changing. It did not.
   `MintRefundingCompleteSet` and `MergeRefundingCompleteSet` take the SAME
   uniform vector the categorical actions take and seat the coordinates
   themselves, so `write_uniform_quantities` was already right for both shapes
   and `dclutch-claims-conservation-contract` is untouched. The route picks its
   action from the RECORD, through `categorical_refunds_on_failure_v3`, exactly
   as founding does.

   What the route still owes is the half that matters: **there is no ELF test.**
   The Claims program-tests carry no Custody-plus-Token-2022 fixture over a
   founded market's Hoard — `affine-batch` has the record set and no Custody,
   `fractional-atomic` has Token-2022 and Custody and no founded Hoard — so the
   route today has a compiling link, a census row (165 routes), unit hostiles
   over its pure decisions, and no execution against a real Custody or a real
   mint. Building that fixture is the next commit.

   Two things the SBF build caught that no unit test would have, recorded
   because the next arm added to this route will hit them: `move_collateral`'s
   first draft built at a **6,528-byte frame against a 4,096 maximum** with four
   "overwrites values in the frame" diagnostics, which is undefined behaviour at
   execution; boxing brought it to 4,544, still over, because both Custody wire
   arms' locals were live in one function; splitting them into their own
   `#[inline(never)]` frames brought it under.
3. **The refunding failure walk on real ELFs.** Still owed and now blocked on
   item 2 as well as on a founded refunding market; and, as above, on a Claims
   founding program-test that does not exist.
4. **Threading `refundsOnFailure` to the market page.** Still owed.
5. **The `signed_delta_v3` waist.** ~~Owed.~~ **Closed at `fd2cb0905`.** A
   refunding Market's failure coordinate may now only be CREDITED to the
   Market's own escrow, and the asymmetry is the rule rather than a gap in it:
   the hazard is worthless claims in somebody's hands, and only a credit puts
   them there. Refusing debits too would have frozen the failure column of every
   cohort-16 market — refunding by record, unseated because the seating rides
   cohort-17 — and a market that cannot retire leaks rent forever. The gate
   costs the route nothing on the common path: a plan touching no coordinate at
   the runtime width's last index reads no account and derives no address.
