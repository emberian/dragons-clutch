# The partial equity Remove's compute wall, priced

Measured 2026-09-02, tree root `/Users/ember/dev/dclutch`, at `a0d556b9e`.
Real SBF ELFs built in this lane's own worktree with its own target directory;
zero SBF stack-frame-overwrite diagnostics on every link built for this note.

The action is `lp_lifecycle::accepted_equity_selector_one_executes_real_custody_and_rolls_back_late_evidence_refusal`
in `programs/dclutch-dealer-accelerator-sbf/program-test` -- the post-trade
partial equity Remove, the first Dealer action ever to carry a nonzero
`signed_position_count` and therefore the first to encode a Claims route with a
SignedDelta suffix. `c3e14e096` cleared its borrowed-witness wall; what is left
is compute, and this note prices it rather than ruling on it.

**Every figure here is from ONE run of the campaign**, and that restriction is
load-bearing, not politeness. `43106855a` measured the same ELF twice over this
suite and found 235 of 307 per-instruction figures differing, in multiples of
1,500 -- the `create_program_address` cost of one more bump-search iteration
over fixture keypairs the campaign draws at random. A CU comparison across two
runs measures the draw. The run used throughout is the full 31-test campaign on
`elf-a0-accprofile`, 30 passed / 1 failed, the one failure being this Remove.

Heap peak on the profiled run is **58,568 of 65,536** at the `candidate`
checkpoint, unchanged from `43106855a` and `c3e14e096`. The heap wall is not
back. This is compute alone.

## The instrument, and the half of it that had never been switched on

`hot-cu-profile` compiles 45 `hot_cu_checkpoint!` sites in
`programs/dclutch-trading-sbf/src/hot_v3.rs`; 41 distinct phases fire on this
action, over 45 log lines.

Ten of them -- `acc-enter` through `acc-context` -- are inside
`authenticate_accelerator_invocation_v4`, which runs *inside the Dealer
accelerator*, because the accelerator links Trading as a library and calls
Trading's own authenticator on its own frame. They had never fired. The lane
build script passes the feature only to the Trading manifest, so the copy of
Trading linked into `dclutch-dealer-accelerator-sbf` was built without it, and
the accelerator's 400,000-odd CU -- the single largest line item in the
transaction -- was one opaque number.

    cargo build-sbf --manifest-path programs/dclutch-dealer-accelerator-sbf/Cargo.toml \
      --sbf-out-dir <out> -- --features dclutch-trading-sbf/hot-cu-profile

That is the whole fix, and it is why this note can say what the accelerator
spends its compute on. **`dclutch-claims-sbf` has no equivalent**, and that
absence is the note's closing measurement.

One reading rule, verified rather than assumed: `sol_log_compute_units` reports
the **transaction-level** meter, so a child's checkpoints continue the parent's
sequence and the two are directly subtractable. Proof from within the run: in
two separate runs the span from the accelerator's return to the `candidate`
checkpoint is **6,339 CU in both**, which only arithmetic on one shared meter
produces.

## What the Remove actually spends

Trading is entered with 1,399,700 and consumes 1,399,692 of it.

| span | CU | note |
|---|---:|---|
| entry -> `start` | 24,174 | loader input deserialization, heap frame, ComputeBudget |
| `start` -> `root-product` | 108,144 | invocation, Market, root, Product runtime |
| -> `artifacts-strategy-effect` | 82,213 | seal, manifest, descriptor, strategy, effect |
| -> `runtime-observations` | 49,962 | |
| -> `p5-geometry-rent` | 1,852 | |
| -> `p5r-projection-banks` | 1,873 | |
| -> `p5r-account-projection` | **93,615** | the request-side account projection |
| -> `p5r-rent-quote-projection` | 961 | |
| -> `p5r-native-signatures` | 353 | |
| -> `p5r-request-projection` | 1,603 | |
| -> `p5-request-registers` | 831 | |
| -> `p5-sealed-ownership-arena` | 3,067 | |
| -> `request-lifecycle-preplan` | 1,211 | |
| -> `candidate-transcript` | 39,594 | Trading's runtime transcript digest |
| **the accelerator leg** | **538,821** | broken out below |
| -> `p7-post-candidate-checks` | 657 | |
| -> `p7-borrowed-witness` | 4,095 | the rule `c3e14e096` repaired |
| -> `p7-replan` | 1,183 | |
| -> `effect-lifecycle-replan` | 437 | |
| -> `observations-released` | 1,725 | |
| -> `p7e-permissions` | 14,800 | |
| -> `p7e-banks` | 4,546 | |
| -> `p7-effect-projection` | **96,121** | the effect-side account projection |
| -> `p7-local-effect-discipline` | 344 | |
| -> `pf-composition` | 41,301 | Claims composition decode, shared by both walks |
| -> `pf-role-programs` | 900 | |
| preflight of 3 invocations | 23,932 | 1,851+6,773 / 1,919+8,540 / 2,844+7,005 |
| -> `preflight-children` | 894 | |
| -> `children-shadow` | 548 | |
| -> `before-commit` | 491 | |
| `before-commit` -> Custody route 0 entry | 22,549 | frame build + the CPI's own charge |
| **Custody route 0** | **123,703** | the cash leg; its Token-2022 CPI is 105 |
| Custody return -> Claims route 1 entry | 42,736 | receipt bank, provenance, next composition |
| **Claims route 1** | 65,456 of 65,464 | **exhausted the meter without completing** |

Sum: 1,399,692. Exact.

The accelerator leg, now that its interior is visible:

| span | CU |
|---|---:|
| Trading's CPI frame build + the CPI charge | 111,968 |
| accelerator entry -> `acc-enter` | 18,484 |
| -> `acc-toplevel` | 22,853 |
| -> `acc-caller-authority` | 5,580 |
| -> `acc-release-waist` | 44,079 |
| -> `acc-product-runtime` | 39,217 |
| -> `acc-records` | 27,901 |
| -> `acc-strategy` | 44,562 |
| -> `acc-input-bank` | 4,897 |
| -> `acc-artifacts` | 48,638 |
| -> `acc-context` | 32,513 |
| **`acc-context` -> return: the transition evaluation itself** | **131,790** |
| accelerator total | 420,514 |
| accelerator return -> Trading's `candidate` | 6,339 |

**Two hundred and eighty-eight thousand, seven hundred and twenty-four CU of
the accelerator's 420,514 -- 69 per cent of it -- is spent before it evaluates
anything.** The work only it can do costs 131,790.

## What the Remove still owes

Three of its legs never ran. Priced from the same run:

| unreached | CU | source of the price |
|---|---:|---|
| Claims route 1's shortfall | 136,787 - 142,819 | it was given 65,464; the Claims program's nine other invocations in this run cost 202,251 - 208,283, and the one in **this same test, in the transaction immediately before**, cost 206,783 |
| Claims return -> Custody route 2 entry | ~42,736 | the measured inter-child span in this very transaction |
| Custody route 2 (the merge) | ~123,700 | route 0 here cost 123,703; the equity Add's single Custody leg in this run cost 123,649 |
| the commit tail | 61,352 | measured end to end on the equity Add in this run: 7,705 post-child + 48,536 `commit-non-root` + 2,498 `commit-root` + 2,387 `after-commit` + 226 |

**The Remove needs about 1,767,000 CU against a 1,399,700 ceiling.** It is
short by roughly 367,000, or 26 per cent. The three "~" rows are the estimates;
everything above them is measured.

That ceiling cannot be raised to measure past. `set_compute_max_units` in
`program-test` makes the bank ignore the transaction's ComputeBudget
instructions **entirely, heap request included**, and the run then faults
writing at `0x30000ff68` on a 32 KiB grant. The wall must be priced from
below.

For scale, the neighbour that does complete: the equity **Add** -- same
prelude, same accelerator, one Custody leg, no Claims route -- consumes
1,162,732 of 1,399,700 in this run. **83 per cent of the ceiling, for the
cheaper of the two actions.** The Remove is not an outlier that overran; the
family is running at the ceiling and the Remove is the first member with three
routes.

## (a) The route's own weight: what does this action authenticate twice?

Four instances, three of them measured here.

**1. Trading's prelude and the accelerator's prelude authenticate one view.
634,409 CU, 45 per cent of the transaction.** Trading spends 345,685 from
`start` to `request-lifecycle-preplan` authenticating release waist, manifest,
descriptor, strategy, effect, Product runtime, account projection and request
registers. The accelerator then spends 288,724 doing the same over the same
accounts. This is not an accident and the accelerator's own module
documentation states it as policy:

> Common Trading authenticates the release, action descriptor, Product,
> execution artifacts, exact request, Profile13 account expansion, and input
> register bank. This program independently rejoins every Dealer semantic
> account through that public view.

The word carrying the cost is *independently*. A stateless read-only
accelerator that took its caller's word for the view would evaluate a
transition against a view nobody authenticated, and a wrong Trading would then
be able to manufacture a candidate. So the duplication buys a real property.

What it does not obviously buy is the **method**. Of the accelerator's 288,724,
the five spans that re-derive artifacts Trading has already authenticated *and
sealed* -- `acc-release-waist` 44,079, `acc-product-runtime` 39,217,
`acc-records` 27,901, `acc-strategy` 44,562, `acc-artifacts` 48,638 -- come to
**204,397 CU**. `authenticate_capability_seal_v3` has already reduced exactly
that set of facts to one canonical seal. An accelerator that bound to the seal
and checked it, rather than re-walking what the seal covers, would still be
taking nobody's word for anything: the seal is a first-party derivation from
the same accounts, not the caller's assertion. **CU recoverable: up to 204,397,
and the invariant that must survive is that the accelerator's acceptance still
depends on nothing Trading merely tells it.**

**2. The account frame is projected twice. 189,736 CU.**
`p5r-account-projection` costs 93,615 and `p7-effect-projection` costs 96,121.
The first projects the frame for the request, the second for the effect. Both
scale with routes, which is why the Remove pays 96,121 where the Add pays
59,002. Whether these are two questions or one asked twice is a reading of
`hot_v3.rs` this note does not make, but the pair is the second-largest line
item in the transaction after the accelerator and it deserves the reading.
**CU recoverable if they are one projection: on the order of 93,000.**

**3. Two Custody legs, two frame parses, 210 CU of actual token movement.**
Custody route 0 costs 123,703 and its Token-2022 CPI inside costs **105**.
Route 2, the merge, is another transfer and would cost about the same again.
So roughly 247,000 CU moves 210 CU worth of tokens, and the difference is
Custody authenticating the same frame, the same replay account and the same
escrow twice, once per leg, with the second leg unable to use anything the
first established. **CU recoverable if a second leg can ride the first's
authenticated frame: on the order of 120,000.** The invariant is the one
`1f41f40a` paid for: Custody debits the vault at the one moment in the sequence
when it may be debited -- after the checkpoint pages have authenticated it
undebited, before the commit -- so any sharing between legs must not move that
moment.

**4. The Claims child's re-derivation of Trading's transcript. About 205,000
CU, and UNMEASURED.** This is the largest single unknown in the note.
`dclutch-claims-sbf` has no profiling feature, so the child's ~205,000 is one
opaque number the way the accelerator's 420,514 was until this afternoon. What
is known: `child_receipt_provenance_v4` derives an expected provenance in
Trading *before* the CPI, and the child necessarily re-derives its own view of
the parent's request, transcript and frame on the other side of the boundary --
the same class as instances 1 and 3.

**Adding the three measured recoveries: 204,397 + 93,000 + 120,000 = 417,397,
against a 367,000 shortfall.** Option (a) alone could just barely land the
Remove inside the ceiling -- at about 1,350,000 of 1,399,700, a 3.5 per cent
margin, on an action whose sibling already runs at 83 per cent. That is not a
margin anyone should ship a settlement route on, and it assumes all three
recoveries land in full.

**Author:** the accelerator seal-binding is the Dealer accelerator's owner
jointly with whoever owns `authenticate_capability_seal_v3`. The double
projection is `hot_v3`'s owner. The Custody leg sharing is Custody's.

## (b) A two-transaction Remove, on the shape the trade leg already has

`1f41f40a` moved the selector-9 trade delivery onto Custody's own reservation:
Custody debits the vault and creates the escrow itself, writes the batch's
replay prestate over the account rather than over a reconstruction of it, and
**the delivery activates in a second transaction**. That shape is not
hypothetical here -- it runs in this very test. In the same run, immediately
before the Remove, two **top-level Custody transactions** cost **259,974** and
**269,686** CU.

Those two numbers are the reason this option is the strongest one, and the
reason it is not obvious. A top-level Custody transaction pays **no Trading
prelude and no accelerator at all** -- 260,000 CU against the 1,145,248 that
any transaction entering through Trading's Hot boundary pays before it does
anything.

Which exposes the arithmetic that governs every split:

**The prelude does not divide. It multiplies.** Entry through
`before-commit` costs 1,145,248 CU and it is paid per transaction. Two
transactions that both enter through Trading pay it twice: 2,290,496 CU of
prelude for a 1,767,000 CU job. A split only helps if the second transaction
enters somewhere cheaper, or skips the accelerator, or both.

Priced:

- **T1** = prelude 1,145,248 + 22,549 + Custody route 0 123,703 + commit
  61,352 = **1,352,852**. Fits, with 46,848 CU of headroom -- 3.3 per cent.
  Tight enough that a single bump draw could break it.
- **T2, if it must re-enter through Trading and re-run the accelerator** =
  1,145,248 + 205,000 + 42,736 + 123,700 + 61,352 = **1,578,036**. Does not
  fit. The split fails.
- **T2, if the reservation carries the authenticated candidate so the
  accelerator can be skipped** = prelude minus the accelerator leg (1,145,248 -
  538,821 = 606,427) + 205,000 + 42,736 + 123,700 + 61,352 = **1,039,215**.
  Fits, with 360,485 CU of headroom -- 26 per cent.

**What carries across:** the reservation must publish, as authenticated state,
(i) the exact candidate bank the accelerator produced or a digest binding it,
(ii) the replay cursor's next revision, and (iii) the escrow poststate. Facts
(ii) and (iii) are precisely the two `1f41f40a` found being *reconstructed*
instead of read, and the fix there was to make Custody write what it actually
accepted. Fact (i) is new and is the whole question.

**What T2 re-authenticates:** release waist and descriptor (it must, or a
stale release could deliver against a fresh reservation), the reservation's own
body from the chain, and the effect's remaining routes. It must NOT need to
re-derive the candidate; if it does, the accelerator comes back and the option
collapses into the failing row above.

**What an abandoned first half costs:** a debited vault and a live escrow with
no delivery. This is the real price of (b) and it is not a CU price. It needs
an expiry or a permissionless cancel, which is new persisted state, a new close
route, and a new refusal band -- and under this tree's rules the cancel is a
vertical slice of its own, not a follow-up.

**Author:** Custody's owner for the reservation body and its cancel; Dealer's
for what the reservation must bind; Trading's for the T2 entry that skips the
accelerator.

## (c) The Claims child hoisted to its own transaction

Priced the same way, (c) converges on (b), and that convergence is itself a
finding.

The Claims route's receipt is consumed by Custody route 2 -- `route(2)`'s
`receipt_dependency()` names `producer_role` Claims and `producer_route` 1 --
so hoisting Claims alone puts a receipt across a transaction boundary that
today lives in `ChildReceiptBankV3`, an in-memory bank inside one invocation.
Persisting it is new state with the same shape as (b)'s reservation, and then
routes 1 and 2 want to be in the same second transaction anyway, which is
exactly (b)'s split.

Splitting after route 1 instead does not work: T1 = prelude 1,145,248 + 22,549
+ 123,703 + 42,736 + 205,000 + commit 61,352 = **1,600,588**. Over.

So (c) is either (b) under another name, or it is a **three**-transaction
Remove, and a three-transaction settlement route pays the prelude three times
and needs two kinds of durable intermediate state. Its only advantage over (b)
is that the intermediate state is a receipt rather than a debited vault, so an
abandoned middle transaction costs nothing but rent. That is a real advantage
and it is why (c) should not be dismissed: **(b) is cheaper in transactions and
(c) is cheaper in abandonment.**

**Author:** whoever owns `ChildReceiptBankV3` would own the persisted receipt.

## The question, stated for the coordinator

This note does not rule, and the reason is the same one `0f0d7f57b` gave for
not ruling on the borrowed witness: the choice here is a policy about what one
program may take on another's authority, and loosening that to make an
integration test pass is the thing this tree's instructions forbid by name.

> **May a Custody reservation carry, as authenticated first-party state, the
> candidate the Dealer accelerator produced -- so that a second transaction can
> bind to it in O(1) instead of re-deriving it?**

If **yes**, the two-transaction Remove fits at 1,352,852 and 1,039,215 against
1,399,700, no route has to get cheaper, and the work is a Custody reservation
body plus its cancel.

If **no**, every split pays the 538,821-CU accelerator leg twice and none of
them fit, so the Remove must find roughly 500,000 CU inside its own route --
and the only places that large are the three re-authentications priced in (a),
each of which exists so that no program takes another's word for anything.
Which is to say the same question, asked at three smaller joints instead of one
large one.

Ember may reverse either way; the numbers are the same numbers.

## The one measurement that would settle it

**Give `dclutch-claims-sbf` the profiling feature `dclutch-trading-sbf` already
has, and read the SignedDelta child's ~205,000 CU.**

It is 15 per cent of the chain ceiling and it is the last opaque number in the
transaction. If most of it is the child re-deriving Trading's transcript, its
frame and its request -- the same class as the accelerator's 204,397 and
Custody's 120,000 -- then option (a) has a fourth source of perhaps 150,000 CU,
the total recoverable passes 550,000, and the Remove fits in one transaction
with a real margin. If it is irreducible economic work on the fractional claim,
option (a) tops out around 417,000 against a 367,000 shortfall, the margin is
3.5 per cent, and the split is forced.

The cost of that measurement is one feature flag, one ELF, and one campaign
run. It is exactly the cost that made the accelerator's interior legible this
afternoon, and the accelerator's interior turned out to be the largest single
finding in this note.
