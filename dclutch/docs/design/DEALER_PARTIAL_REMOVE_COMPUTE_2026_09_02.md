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
~~**CU recoverable if they are one projection: on the order of 93,000.**~~

> **SUPERSEDED 2026-09-03, and this row was wrong.** The reading was made: they
> are two interpreters over two artifacts, and the second reads registers the
> accelerator produces after the first has run. Nothing here is recoverable by
> merging them. The real duplicate is one level down and is worth 15,000 to
> 25,000. See "The double projection was not one question asked twice" at the
> end of this note.

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
against a 367,000 shortfall.** *(Superseded: the 93,000 was never there -- see
the end of this note. Two of these three were estimates, and the estimate that
carried this paragraph is the one that did not survive being read.)* Option (a) alone could just barely land the
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

---

## Addendum, same afternoon: the measurement was taken, and it answers

*Measured at `104f45d7e` plus the `claims-cu-profile` feature this note asked
for, on real SBF ELFs, zero frame diagnostics, campaign 30 passed / 1 failed --
unchanged, so the instrument does not move the suite. A DIFFERENT run from the
tables above, with cheaper bump draws throughout (the Custody cash leg is
111,703 against 123,703, the accelerator 395,016 against 420,514), which is why
the Remove's total requirement reads about 1,664,000 here against 1,767,000
above -- 24,174 before `start`, 1,070,088 to `before-commit`, 22,549 and 111,703
for the Custody cash leg, 44,242 between children, 174,120 for the Claims child
it cannot afford, 44,242 and 111,703 for the unreached merge, 61,352 to commit.
That hundred-thousand spread IS the draw, and it is the reason the note opened
by restricting every comparison to one run.*

`dclutch-claims-sbf` now carries fourteen checkpoints on the SignedDelta route
behind a feature that is off in every shipped build. A completing invocation of
that child costs **173,680 CU** and spends it like this:

| span | CU | share |
|---|---:|---:|
| CPI entry, deserialize -> `sd-enter` | 4,024 | 2.3% |
| `SignedDeltaPlanV3::decode` | 2,771 | 1.6% |
| **`SignedDeltaAccountsV3::parse`** | **31,054** | **17.9%** |
| `authenticate_privileges` | 2,500 | 1.4% |
| `hash(instruction_data)` | 509 | 0.3% |
| `authenticate_authority` | 3,532 | 2.0% |
| **`authenticate_releases`** | **76,245** | **43.9%** |
| `authenticate_market` | 2,206 | 1.3% |
| **`authenticate_product_and_basis`** | **41,808** | **24.1%** |
| `build_candidates` | 5,213 | 3.0% |
| **`apply_deltas` -- the economic work** | **662** | **0.4%** |
| table + resource digests, receipt | 1,192 | 0.7% |
| `commit_candidates` | 453 | 0.3% |
| `set_return_data` -> return | 1,511 | 0.9% |

Each row includes the one checkpoint that ends it -- two syscalls, on the order
of 200 CU -- so the instrument is roughly 2,800 of the 173,680 and none of the
ranking. **Every one of those spans except the entry is IDENTICAL to the digit
across two independent campaign runs with different key draws**, which the
figures above the addendum are not: these are deterministic work, not
bump-search noise.

**The child spends 662 CU applying the deltas it exists to apply. It spends
149,107 -- 85.9 per cent -- parsing a frame and authenticating releases,
products and bases that Trading authenticated earlier in the same instruction.**
Counting every authentication row rather than the three large ones it is
157,345, or 90.6 per cent; everything from `build_candidates` through
`commit_candidates` -- copy the market and positions, apply the deltas, digest,
write -- is 7,520, or 4.3 per cent.

And the Remove's own child dies exactly where that predicts. It entered with
126,944 CU, cleared the frame parse, the privileges, the authority and
`authenticate_releases` -- 76,245 of its 126,944 spent re-establishing a release
set its caller had established -- reached `sd-market` with 3,643 left, and
exhausted the meter inside `authenticate_product_and_basis`, which needs 41,808.
It never reached `build_candidates`. **The action that has been called
compute-bound for two days has never once executed a single delta.**

### What this does to the three options

**Option (a) gains its fourth source, and it is the size the note predicted.**
The guess above was "perhaps 150,000 CU if the child's cost is mostly
re-derivation". It is 149,107, and unlike the other three it does not move
between runs.

| source | CU | measured? |
|---|---:|---|
| the accelerator re-deriving what the seal already covers | 186,397 - 204,397 | measured, draw-dependent |
| **the Claims child's frame parse, releases, product and basis** | **149,107** | **measured, deterministic** |
| the second Custody leg's repeated frame parse | ~112,000 - 124,000 | estimated |
| the second account-frame projection, if the two are one question | ~93,000 | estimated |
| total | **540,000 - 570,000** | |

against a shortfall of 264,000 (this run) to 367,000 (the run above). **Option
(a) can close the gap with a real margin, and it no longer needs all four to
land.** The two MEASURED rows alone come to 335,504 - 353,504.

**Which sharpens the question rather than answering it**, because all four are
the same act: a program declining to take its caller's word. But the Claims
child's is the best-posed of them, because its 76,245-CU half is one named call
made three times. `authenticate_releases` runs `authenticate_activated_role` for
Core, for Claims and -- when the caller is Trading -- for Trading, each against
the SAME Registry activation cache account Trading itself read earlier in the
same instruction, to establish the same release set. About 25,400 CU per role,
three times, for a fact the transaction has already established.

The joint that might carry it is already in the frame and is not a promise from
the caller: this route's authority is a `CallerAuthoritySeedsV1` PDA, whose seed
order pins the release set, the market, the execution role and the exact
role-request digest. A valid signature on it is a cryptographic statement that a
program holding those seeds is calling. What it does NOT by itself state is that
the calling program is the Registry-ACTIVATED holder of that role for that
release set -- which is precisely the gap the comment above
`authenticate_releases` exists to close, after a caller coordinate that pinned
nothing let any executable program sit there and demand a PDA under itself. So
this is not a deletion; it is a question about which authority may state
activation.

> **May the Claims SignedDelta route take its caller's Registry role activation
> as established -- by the caller-authority PDA whose seeds already pin the
> release set, or by an `AuthenticatedRoleReceiptV1` the caller holds and passes
> -- instead of re-running `authenticate_activated_role` three times against the
> same activation cache its caller has just read?**

Claims already has the shape for the "my caller did it" case, and it is not
hypothetical. `execute_parent_authenticated` exists so an enclosing Claims route
can say exactly that, and `execute_authenticated`'s `parent_authenticated` arm
swaps the 41,808-CU `authenticate_product_and_basis` for a digest comparison.
Trading's CPI reaches none of it: it calls `process`, the unauthenticated
top-level entry, so the child pays the full 149,107. That existing path is
in-process and `pub(crate)`, so extending it across a CPI boundary is a new
request kind with its own hostiles, not a one-line reroute. It is still a route
change with a shape to follow rather than new persisted state with a new cancel
path.

**Options (b) and (c) are unchanged in structure and cheaper in urgency.** The
split still works only if the second transaction can skip the accelerator, and
an abandoned first half still costs a debited vault and a live escrow. They are
simply no longer the only way through, which is the difference between a design
this action needs and one it might want later.

### What is still not measured

*(SUPERSEDED the same night: BOTH are settled below. Custody has its own
profiling feature now and its cash leg is measured at 115,273-121,273 CU, of
which 92,515-94,015 is caller re-authentication; and the double projection
turned out not to be a duplicate at all. Read on.)*

The second Custody leg and the second account-frame projection are still
estimates and have no instrument at all: Custody has no profiling feature, and
the two Trading projections are one checkpoint each with no interior. Neither is
load-bearing for the conclusion any more -- the two measured rows come within
14,000 of even the larger shortfall on their own -- but the Custody one is the
obvious next flag, and it is the same afternoon's work: a feature, a build, a
campaign run.

---

## Second addendum, 2026-09-03: the ruling was spent, and the Claims route commits

*Measured at `babf26fed` (before) and `0aa70478e` (after), tree root
`/Users/ember/dev/dclutch`, real SBF ELFs built in this lane's own worktree with
its own target directory, zero SBF stack-frame-overwrite diagnostics on every
link built for this addendum. Campaign 30 passed / 1 failed both sides,
unchanged in count and the failure still this Remove.*

**The two runs used different Claims ELFs, so the bump draw MOVED between them,
and the comparison below is cross-run anyway. Here is why, stated as a control
rather than a claim.** `release_set_id` hashes the deployed ELF digests, so a
changed Claims executable redraws every PDA search depth in the fixture. The
campaign invokes this route ten times per run, and reading all ten:

| span | distinct values, BEFORE run | distinct values, AFTER run |
|---|---|---|
| `SignedDeltaAccountsV3::parse` | {31,054} | {21,754} |
| `authenticate_releases` | {76,245} | {30,828} |
| `authenticate_market` | {2,206} | {2,138} |
| `authenticate_product_and_basis` | {41,808} | {3,375} |
| `build_candidates` | {5,213, 6,712, 6,713, 8,213} | {5,243, 6,743, 8,243, 9,743, 11,242, 11,323} |

Four spans take **exactly one value across ten invocations** on each side, and
`build_candidates` does not: its values differ in multiples of 1,500, the cost
of one more `create_program_address` iteration, exactly as M-61 and decision
0012's per-seed decomposition predict. Solving `delta = n x 1,500 + c` on the
matching pairs (5,213/5,243, 6,713/6,743, 8,213/8,243) gives **c = +30 CU** on
all three -- `build_candidates` ceasing to be inlined into
`execute_authenticated`, which the frame manifest independently shows as a new
704-byte row. Two instruments, one cause.*

**A THIRD RUN then corrected the reading above, and the correction is worth
more than the reading was.** Constant-within-a-run is NOT the same as
search-free. Every invocation in one run executes under one `release_set_id`,
so a bump search whose depth is a function of that id is a per-run CONSTANT and
looks deterministic from inside the run. A later Claims ELF in this same lane
(the `dclutch-cu-checkpoint` extraction, which changes `.text` and therefore the
release-set id) gives a second, independent after-run:

| span | before (two runs, one ELF) | after run A | after run B |
|---|---:|---:|---:|
| `SignedDeltaAccountsV3::parse` | 31,054 | 21,754 | **21,754** |
| `authenticate_releases` | 76,245 | 30,828 | 27,828 |
| `authenticate_market` | 2,206 | 2,138 | 5,138 |
| `authenticate_product_and_basis` | 41,808 | 3,375 | 6,375 |

Only the frame parse is genuinely search-free, and it is identical to the digit
across two different executables. The other three each move by exactly 3,000 --
two iterations -- between the two after-runs, which is the draw and nothing
else. So the honest form of every claim in this addendum is a BAND, and every
band clears the noise by more than an order of magnitude:

- the frame parse saves **exactly 9,300**, draw-free on both sides;
- `authenticate_releases` saves **45,417 to 48,417**;
- `authenticate_product_and_basis` saves **35,433 to 38,433**.

The before figures are the ones entitled to be quoted flat: 76,245 / 41,808 /
31,054 were reproduced to the digit by two runs at two different commits whose
Claims sources -- and therefore whose Claims ELF, and therefore whose
release-set id -- are identical.*

The ruling, recorded in `GOAL.md` for ember to reverse:

> **A callee invoked by a PDA-signed CPI from Trading takes the facts that
> signer's seeds pin as established.** The callee verifies the signer's
> derivation against Trading's program id and the seeds it presents -- that is
> the whole authentication of the caller -- and takes the role activation, the
> release set and the sealed records as established for exactly what the seeds
> name. The unpinned-caller history stays as a hostile.

### What it bought in Claims

| span | before | after | |
|---|---:|---:|---|
| `SignedDeltaAccountsV3::parse` | 31,054 | 21,754 | −9,300 |
| `authenticate_privileges` | 2,500 | 2,500 | |
| `authenticate_releases` | 76,245 | 30,828 | **−45,417** |
| `authenticate_market` | 2,206 | 2,138 | |
| `authenticate_product_and_basis` | 41,808 | 3,375 | **−38,433** |
| `apply_deltas` -- the economic work | 662 | 663 | |
| **a completing invocation** | **173,676** | **80,488** | **−93,188** |

The three spans that were 149,107 CU of 173,676 -- 85.9% -- are now 55,957 of
80,488. The SHARE only falls to 69.5%, and that is the honest way to read it:
what is left is dominated by the frame parse and the one cache decode the
seeds cannot establish, so the remaining ratio is near the floor this shape
has rather than slack.

**And the Remove's Claims route now EXECUTES AND COMMITS.** The child entered
with 94,423 CU, cleared the frame, the privileges, the authority, the releases,
the Market and the product/basis join, built its candidates, reached
`sd-deltas-applied`, wrote them at `sd-committed`, set its return data, and
handed **12,210 CU** back. The action that had been compute-bound for two days
and had never executed a single delta now executes and commits its deltas.

### The honest split of the 45,417, because most of it needed no ruling

`authenticate_releases` called `authenticate_activated_role` three times, and
each call ran `ActivatedExecutionReleaseSetViewV1::decode` -- the complete
five-role projection and every aliasing pair, twenty-five `decode_role` calls --
to answer a question about ONE role. The account was hostile-decoded three times
in one invocation. `dclutch-registry-activation-auth-v1`'s own doc has said
since 2026-09-02 that a multi-role frame must decode once and names the pair
that does it; Claims was not using it.

So the larger part of this saving is a redundant decode that a reading would
have found without any ruling at all, and the ruling's own contribution is the
three per-role deployment observations it drops. That is stated here rather
than folded into one number, because a measurement that lets a ruling take
credit for a redundancy is not evidence for the ruling.

The product/basis figure is the ruling's, undiluted: 41,808 to 3,375, and it is
the same repair the in-process arm already had. The plan's
`product_record_digest` and `linked_basis_record_digest` sit inside
`hash(instruction_data)`, which is the last seed of the PDA that signs this
route, so a caller has committed to them cryptographically. What remains is to
bind the frame's coordinates to those digests -- and the two conjuncts a
signature cannot carry stay unconditional: `authenticate_core_market_v3`,
because a caller may pin its own plan to whatever it likes but may not author
the Market's persisted principal cap, and `authenticate_market`.

The parse saving is a third thing again, and not the ruling either: dropping
the walk left six frame coordinates bound by name and read by nobody, and
binding a name costs a full scan of the frame spec. They stay in
`SignedDeltaFrameSpecV3` -- the frame is a wire contract shared with callers --
and `authenticate_privileges` still takes every coordinate's privileges by
index, so an unread account is still a refused writable or signer.

### What was given up

The per-role deployment observation was also the slot pin: decision 0012's
`ReleaseSuperseded`, raised when the substrate's upgrade authority ships new
bytes under an open market. The Claims SignedDelta route now inherits that
refusal from its caller, which observes all five roles before it composes the
child. It is not lost from the transaction. It is lost from this program, and
a future caller that does not observe would not be caught here.

### The hostiles, and the one thing that made them worth writing

Three, in the real-ELF fractional SignedDelta program-test, all passing:

- **a caller that is not the activated Trading** -- a second deployment of the
  identical caller ELF holds the Registry's `Trading` activation while the test
  caller invokes and signs its own correctly-seeded PDA. `0x5202`.
- **an activation cache for another release set** -- complete, Registry-owned,
  at its own canonical address, belonging to another generation. `0x5202`.
- **an unsigned caller authority** -- the activated caller invokes with
  `invoke` where `invoke_signed` belongs. `0x5201`.

`SignedDeltaSbfErrorV3::Release` is ONE discriminant over both the authority
derivation and the release bind, so asserting it proves nothing about where the
refusal happened -- exactly the trap `AGENTS.md` names. Run against a
`claims-cu-profile` build, the first two log through **`sd-authority`** and then
refuse without reaching `sd-releases`: the authority PASSED and the coordinate
bind is what said no. The third refuses after `sd-frame-parsed` without
reaching `sd-privileges`. **Owed, and not a lane's act:** `Release` should be
split so those tests name their own accusation instead of borrowing the
instrument, and the split makes `docs/reference/refusals.md` stale, which the
convergence owner regenerates.

Suites, every row run and every row reported: Dealer accelerator campaign 30/1
unchanged; Claims fractional SignedDelta 4/4; fractional-atomic 29/0;
protocol-position 7/0; `rational_representation_v2_program_test` **49/0**, which
carries the two pre-existing `Release` hostiles on the caller coordinate. That
last one first reported 48 FAILED, and the reason is worth recording: the
`spl_token_2022.so` borrowed from another lane's scratch was a different build,
and the suite's own fixture digest check refused it. It DID NOT RUN; it did not
fail. A second cached fixture had the matching digest and the suite is green.

## Where the wall is now, exactly

The transaction still overruns, and it now overruns in a different place.

| | CU |
|---|---:|
| Trading entry through `before-commit` | 1,118,254 |
| Custody route 0 (the cash leg), with its frame build and the inter-child span | 187,023 |
| **Claims route 1 -- COMPLETES** | 87,161, of 98,455 given |
| remaining when Claims returns | **12,210** |

and what is left unreached, priced from this same run:

| unreached | CU | source of the price |
|---|---:|---|
| Claims return -> Custody route 2 entry | ~44,000 | the measured inter-child span |
| Custody route 2 (the merge) | ~116,000 | route 0 in this run cost 116,203 |
| the commit tail | 61,352 | measured end to end on the equity Add |
| **total** | **~221,400** | against 12,210 remaining |

**The Remove is short by about 209,000 CU.** It was short by 264,000 to 367,000
before this addendum. Claims has no more to give: its whole invocation is now
80,488, of which the frame parse is 21,754 and the activation-cache decode
30,828, and that decode is the one derivation the seeds cannot establish --
the seeds pin a release set, not which program holds a role in it.

## The 209,000 is mostly the accelerator's prelude, and it does not come out span by span

*(Refined by the third addendum below, which measured Custody: of the ~221,400
the transaction still owes when Claims returns, about 115,000 is the Custody
merge and 92,515 of THAT is caller re-authentication. So the accelerator is the
larger half of the problem, not the whole of it, and the merge's half is now
measured rather than estimated. The reading below stands unchanged for the
accelerator.)*

The accelerator's leg in this run: entry through `acc-enter` 124,454 (which is
Trading's CPI frame build and the runtime's own charge as much as the
accelerator's entry), then

| span | CU | what it establishes |
|---|---:|---|
| `acc-toplevel` | 22,853 | every one of the 48 frame accounts is the account the TOP-LEVEL instruction named, read back from the Instructions sysvar |
| `acc-caller-authority` | 5,580 | **the binding**: account 0 is the `CallerAuthoritySeedsV1` PDA under Trading for (release set, market, role Trading, root, `hash(request_bytes)`) |
| `acc-release-waist` | 39,579 | activation cache, Market, family context |
| `acc-product-runtime` | 39,217 | the Product/domain/portfolio/linked-basis graph |
| `acc-records` | 27,901 | manifest, program set, seal, descriptor, config |
| `acc-strategy` | 38,562 | the admitted-AOT strategy, certificate, admission and artifact-release chain |
| `acc-input-bank` | 4,897 | the input registers |
| `acc-artifacts` | 48,638 | the five sealed descriptor artifacts, geometry, representative coordinates |
| `acc-context` | 32,513 | recompute `AdmittedInvocationContextV3` and require its digest to equal `request.invocation_context()` |
| **the transition evaluation itself** | **131,790** | the work only this program can do |

**249,263 CU of prelude against a 209,000 shortfall, so the arithmetic works and
the method does not.** Reading `authenticate_accelerator_invocation_v4` end to
end, the prelude is a CHAIN, not a list: `family_context` yields the record
bumps, which locate the manifest and program set, which yield the selected
action and descriptor, which key the seal, which yields the descriptor body and
the five artifacts, which yield the geometry, which the context digest closes
over. Each stage's OUTPUT is the next stage's input, and the evaluator consumes
the last of them. Deleting a middle stage does not save its CU; it removes a
value the evaluator needs.

That is why this addendum lands no accelerator cut. The two spans whose outputs
the evaluator does not consume are `acc-toplevel` (whose 22,853 is mostly the
sysvar read that also produces the envelope, so only the comparison loops are
recoverable) and `acc-context`'s comparison. Neither is 209,000, and a partial
cut that does not land the Remove is not worth a trust change in a frame with
192 bytes of headroom: `authenticate_accelerator_invocation_v4` sits at 3,904
of 4,096.

*A correction while I am here, because it has now been repeated three times
including in this lane's own `fa00e8f28` message: that frame is NOT "the
tightest first-party frame in the tree." Read straight out of
`tools/frameguard/baseline.json` at `0aa70478e`, six rows sit deeper at 3,968
-- `trading::outer::process_close`, `custody::projected::advance_source_state`
and `realize_and_close`, `core::generic_founding_v1::authenticate_claims_and_custody`,
and two in `resolution_proof_sbf` -- and two more share 3,904 with it
(`authenticate_strategy_for_accelerator_boxed_v4` and
`authenticate_strategy_from_sealed_boxed_v3`). 192 bytes of headroom is the
true and sufficient fact; the superlative was inherited from `271ce0ed`, whose
own message named `process_close` at 3,968 as the deepest two sentences
earlier.*

**The shape the repair has to take, stated so the next lane does not rediscover
it.** Every value in the chain is something Trading COMPUTED, in the same
instruction, before it built the CPI -- and the caller-authority PDA already
pins `hash(request_bytes)`. So the request is a channel that costs nothing to
widen: anything Trading writes into it is established by a signature the
accelerator already checks for 5,580 CU. The repair is therefore not a deletion
but a MOVE -- the `AdmittedInvocationContextV3` preimage, the selected action,
the span widths, the claims and custody program ids, the outcome count, and the
representative coordinates travel in the request instead of being re-derived
from twelve accounts -- and the accelerator's prelude becomes: decode the
request, verify the caller-authority derivation, read the values, evaluate.
Priced from this run that is about 15,000 CU against 249,263, and the Remove
lands with room.

**What must NOT move with them**, and this is the whole of the design's risk:
the accelerator exists to be a second opinion on the EVALUATION, so every input
to the transition must still be a fact about an account this program reads --
the input register bank, the runtime accounts, the root prestate. A request
field that carried an evaluation INPUT rather than an authentication RESULT
would make the accelerator a mirror of its caller, and the whole reason the
Dealer family has one would be gone.

**Author:** the Dealer accelerator's owner jointly with `admitted_composition_v3`'s,
because the request is composed there and consumed here, and the hostile that
makes it a repair rather than a relaxation belongs with them: a request field
that disagrees with the account the accelerator can still see must refuse by
name.


## The double projection was not one question asked twice, and this note's §(a) was wrong about it

Instance 2 above priced `p5r-account-projection` (93,618 here) and
`p7-effect-projection` (96,121) as possibly "one projection asked twice," put
**~93,000 CU** in the recoverable column, and said the reading was one this note
did not make. The reading is made now, and the answer is no. **Strike the
93,000.**

They are different interpreters over different artifacts, and the second's
input did not exist when the first ran:

| | `p5r-account-projection` | `p7-effect-projection` |
|---|---|---|
| interpreter | `project_dynamic_fixed_spans_atomic`, `crates/dclutch-account-profile-contract/src/v2.rs:2386` | `project_atomic_visiting`, `crates/dclutch-effect-kernel/src/v4.rs:1014` |
| artifact | `AccountProfileV2` bytes | `EffectProgramV4` bytes |
| accounts | `&[AccountObservationV1]` -- key, owner, lamports and the full data | `&[AccountInput]` -- `{lamports, data_len}` only |
| registers | the seeded PRE-transition banks | `candidate.scalars`, the POST-transition output |
| returns | the authenticated register banks | `ProjectedEffectsV3 { lamports, requests, participation }` |

The ordering makes it impossible in principle rather than merely awkward: the
accelerator leg sits between them and PRODUCES the register state the second
one reads, the effect frame's own width is a function of those post-transition
scalars, and the observation bank the first one walked is dropped before the
second runs -- `hot_v3.rs` says so in its own words at `project_hot_effects_v3`:
*"This function never sees that bank: it is released before this runs."*

And the frame-shape guess in §(a) is backwards. The effect frame is a PREFIX of
the runtime frame, not a superset: `effect_account_count <= runtime_account_count`
is enforced, every coordinate past it must be read-only, and the kernel is
handed `.get(..effect_account_count)` slices of the same banks. There are zero
accounts in one and not the other.

**The tree had already answered this and nobody carried it forward.**
`docs/evidence/DIRECT_HOT_AOT_MEASUREMENT_2026-08-31.md` says of the effect
projection: *"a **different interpreter**, over EffectProgram V4 bytes, 131
fixed effect operations. Not the TransitionVM,"* and of the register
projection: *"other interpreters over other artifacts."* `96d6e04df`, which
introduced the `p7e-*` checkpoints, calls them *"the two projections that
phases 5 and 7 turned out to be almost entirely made of"* -- two. The only text
in the tree asserting sameness was §(a) of this note, and it asserted it from a
shared LABEL: both checkpoints have the word "projection" in them.

### What IS computed twice here, and it is worth about 15,000 to 25,000

There is a real duplicate inside the pair, one level down. For every
coordinate, `expanded_rule_with_dynamic_spans` and
`representative_with_dynamic_spans` are computed:

- inside `validate_accounts_with_dynamic_spans` (`v2.rs:2662`), during p5r --
  and both results are **thrown away**;
- again in `derive_effect_permissions_with_dynamic_spans` (`v2.rs:2522`), which
  keeps only `authority_rule.permission()`. That is `p7e-permissions`: **14,800
  CU over ~74 coordinates, about 200 CU each**;
- a third time in `child_route_privileges_v3` -> `dynamic_declared_privileges_v4`
  (`programs/dclutch-trading-sbf/src/dynamic_accounts_v4.rs:129`), inside
  `pf-composition`'s 41,301.

The smallest shareable thing is therefore not a projection but a BANK: p5r
already decodes every rule and every representative, so it can emit a
caller-owned permission (and route-privilege) bank beside its register banks.
One byte per coordinate survives the observation bank's release, and it retires
`p7e-permissions` outright and part of the third walk. **Ceiling 15,000-25,000,
not 93,000.**

**What this does to §(a)'s arithmetic.** The four-source table in the first
addendum totalled 540,000-570,000 against the shortfall. Two of its rows are
now settled differently: the Claims child's 149,107 was real and has been SPENT
(93,188 of it landed; the rest was the frame parse and the cache decode, which
stay), and the double projection's ~93,000 was never there. What remains
unmeasured in that table is the second Custody leg, and Custody still has no
profiling feature -- which is now the single largest opaque number left in this
transaction, exactly where `dclutch-claims-sbf` was two days ago.

**Author:** the shared permission bank is `hot_v3`'s owner jointly with
`dclutch-account-profile-contract`'s, since the bank has to be emitted by the
projection that already computes it.


## Third addendum, same night: Custody was the last opaque number, and it is 80 per cent caller

*The note above named this as the obvious next flag -- "Custody has no
profiling feature, and it is now the single largest opaque number left in this
transaction, exactly where `dclutch-claims-sbf` was two days ago." It took one
feature, one build and one campaign run, which is the third time that sentence
has been true.*

`dclutch-custody-sbf` now carries `custody-cu-profile`, off in every shipped
build, over both routes the Dealer family reaches: the main
`CustodyRequestV1` route the Remove's legs take, and the Dealer scenario
reservation route the top-level reservation transactions take. The macro and
the `#[inline(never)]` that guards it moved into a new crate,
`dclutch-cu-checkpoint`, because `claims_cu_checkpoint!`'s own doc said to:
*"If a third program needs one, that is the moment to extract the pair, not
before."* Custody is the third program. What did NOT move is each program's
feature name and domain prefix -- a build line names the feature, a log reader
greps the prefix -- and Trading's `hot_checkpoint`, which also reports its bump
allocator's outstanding heap and is a different instrument.

**The Remove's Custody cash leg, in two independent campaign runs.** Two,
because the lesson two sections up applies here too: one run cannot separate the
code from the bump draw.

| span | run A | run B |
|---|---:|---:|
| dispatch, `CustodyRequestV1::decode`, frame count, request digest | 3,240 | 3,240 |
| **`authenticate_series_aware_common_frame`** | **61,739** | **63,239** |
| **`authenticate_realm`** | **30,776** | **30,776** |
| transfer frame coordinates | 277 | 277 |
| token program, mint, custody authority, vault keys | 5,257 | 9,757 |
| prestate balances | 2,104 | 2,104 |
| **`invoke_exact_transfer` -- the economic work** | **2,380** | **2,380** |
| poststate balances and the conservation check | 1,301 | 1,301 |
| receipt, replay advance, return data | 4,940 | 4,940 |
| whole invocation | 115,273 | 121,273 |

Six of the nine spans are identical to the digit across the two, `realm` and the
token CPI among them. The two that move do so by 1,500 and 4,500 -- one and
three `create_program_address` iterations, the draw and nothing else.

**The Token-2022 CPI inside that 2,380 consumed 105 CU.** Custody spends
**92,515 to 94,015 of 115,273 to 121,273 -- between 77 and 81 per cent --
re-authenticating the market, the release set, the replay cursor and the realm
that Trading authenticated earlier in the same instruction, and 105 CU moving
the tokens it exists to move.** The Claims finding, one program over, in the
same proportion.

Which makes instance 3 of §(a) measured rather than estimated. It guessed "two
Custody legs pay about 247,000 CU of frame authentication to move 210 CU of
tokens" and priced the recovery at 112,000-124,000. The true figure per leg is
92,515 of caller re-authentication, so **185,030 over the Remove's two legs**,
and the ruling applies to Custody exactly as it applies to Claims: this route is
reached by a PDA-signed CPI under `CallerAuthoritySeedsV1`, whose last seed is
`hash(request_bytes)`.

**Caveats, both of them.** This is one invocation per run -- the campaign's other
Custody legs go through the reservation route -- so unlike the Claims table
there is no within-run repetition behind it -- the two runs above are the
control instead. And Custody is
not Claims: its realm and replay joins bind a cursor that the caller advances,
so what the seeds establish and what they do not needs the same line-by-line
reading `authenticate_releases` got, not a copy of its conclusion.

### The instrument costs the shipped Custody executable nothing, and that is proven twice over

Default builds, no feature: **`.text` is byte-identical** -- 555,824 bytes, same
sha, and `.rodata` identical too. The whole ELF differs in exactly **ONE byte**,
in `.data.rel.ro`, and it is a `core::panic::Location` line number that goes
from 40 to 78. This commit adds exactly **38 lines** to
`programs/dclutch-custody-sbf/src/lib.rs` above it.

Getting there took a wrong turn worth recording. The first version made
`dclutch-cu-checkpoint` an unconditional dependency, and the default ELF then
differed in **1,153 of 555,824 `.text` bytes** at identical size and identical
instruction count -- 378 differing instructions of 68,319, changed opcodes and
immediates rather than added work. Rebuilding HEAD's own Custody SOURCES against
only the new manifest reproduced the same 1,153 bytes, which is what identified
it: the whole difference was the dependency EDGE, and none of it was a line
anyone wrote. The dependency is now `optional = true` and the feature is
`["dep:dclutch-cu-checkpoint"]`, so a shipped build has no such edge -- and the
`.text` sha says so. `dclutch-claims-sbf`'s dependency was made optional in the
same breath, for the same reason, and there the control is even cleaner: the
default Claims ELF built after the extraction is **byte-identical, all
1,373,224 of them**, to the one built before it. Moving a macro into a crate
and taking its dependency edge back out again is a no-op on the shipped
executable, and that is a measurement rather than an expectation.

### And the Remove now reaches the merge

In the run before this one -- same code, luckier draw -- the transaction got
past the Claims child, past the inter-child span, and **invoked Custody a
second time**, dying 7,908 CU into the merge leg. That is the first time this
action has reached its third route. The wall has moved twice tonight: from
inside the Claims child's product/basis join, to after the Claims child's
commit, to inside the Custody merge.

The remaining arithmetic is unchanged in size and better sourced: the merge
needs about 115,000 and the commit tail 61,352, against the ~12,000 the
transaction has when Claims returns. **Still short by roughly 165,000 to
210,000 depending on the draw** -- and of that, 92,515 is now a MEASURED
re-authentication inside the merge itself, which was the estimated row.

## Fourth addendum, 2026-09-03: the ruling's three applications, and the Remove reaches the commit tail

*Measured at `036002288` (before), `9b5de611e` (Custody) and `742d7b7be`
(accelerator), tree root `/Users/ember/dev/dclutch`, real SBF ELFs built in this
lane's own worktree with its own target directory, zero SBF stack-frame-overwrite
diagnostics on every link built for this addendum. Dealer accelerator campaign 30
passed / 1 failed throughout, unchanged in count, the failure still this Remove.
Every comparison below is between two ELFs and therefore two bump draws; the
draw-free rows are named where they exist.*

### The prelude's interior, which the last addendum could not see

`30d02f5c0` could say the accelerator's prelude was 249,263 CU and that it is a
CHAIN. It could not say what each link cost, and a chain nobody has priced link
by link cannot be cut surgically. Eight `hot_cu_checkpoint!` sites, behind
`hot-cu-profile` and absent from every shipped build, say it now:

| link | CU | what it establishes |
|---|---:|---|
| `acc-toplevel` | 22,853 | the 48 fixed accounts and the evidence suffix ARE the top-level instruction's |
| `acc-caller-authority` | 4,080 | the binding: account 0 is Trading's `CallerAuthoritySeedsV1` PDA over the request |
| `acc-activation` | 30,743 | ONE activation-cache decode: which program holds Trading in this release set |
| `acc-market` | 3,777 | the Core Market's persisted identity |
| `acc-release-waist` | 2,801 | the root's family context and the Rent sysvar |
| `acc-product-runtime` | 39,217 | the Product/domain/portfolio/linked-basis graph -- **draw-free** |
| `acc-manifest` | 11,034 | the manifest record and its selected entry |
| `acc-programset` | 7,189 | the program set, the selected action and the descriptor identity |
| `acc-seal` | 3,878 | decision 0005's seal -- **draw-free** |
| `acc-descriptor` | 3,072 | the descriptor body, read through the seal |
| `acc-records` | 4,211 | the config record and the common projection bindings |
| `acc-strategy` | 37,061 | the admitted-AOT strategy, certificate, admission and artifact-release chain |
| `acc-input-bank` | 4,897 | the input register bank, out of the runtime accounts |
| `acc-artifact-records` | 37,914 | the five sealed artifact records, borrowed and decoded |
| `acc-artifacts` | 11,092 | geometry: span widths, logical count, `require_geometry`, representatives |
| `acc-observations` | 30,344 | the observation digest over the runtime accounts |
| `acc-context` | 2,547 | assemble the context and compare its digest to the request's |

**The two spans that are identical to the digit across every ELF this lane built
are `acc-product-runtime` (39,217) and `acc-seal` (3,878).** Everything else
moves with the draw, in multiples of 1,500.

### (i) Custody: the cache was decoded three times

The third addendum measured Custody's caller re-authentication at 92,515 to
94,015 per leg and said the reading it needed was its own. It is, and most of
what it found needed no ruling at all: `authenticate_market`,
`authenticate_calling_release` and `authenticate_realm` each borrowed the SAME
immutable Registry-owned activation account and each ran
`ActivatedExecutionReleaseSetViewV1::decode` -- the complete five-role
projection, twenty-five `decode_role` calls -- to answer one question about one
role.

| span | before | after A | after B |
|---|---:|---:|---:|
| `cu-common-frame` | 58,750 | 35,980 | 40,470 |
| `cu-realm` | 30,781 | 9,053 | 9,051 |
| whole invocation | 121,289 | 78,301 | 73,789 |

Three ELFs, three draws: `cu-realm` is 9,053 and 9,051 across the two
after-runs and `cu-common-frame` differs by 4,490 -- three
`create_program_address` iterations -- so the saving is a BAND of 40,000 to
44,500 per leg and the noise it clears is 4,500.

**The ruling's own contribution is one thing**, stated separately because a
measurement that lets a ruling take credit for a redundancy is not evidence for
the ruling: `authenticate_calling_release` no longer observes the caller role's
live deployment, and takes the role's Program and ProgramData identities out of
the decoded view instead. What that gives up is decision 0012's
`ReleaseSuperseded` on this route, inherited from the caller, exactly as Claims
gave it up.

**What stays, and why each conjunct does.** The activation identity, because the
seeds name a role and not a key. The Market, because a caller may pin its own
request to whatever it likes but may not author the Market's persisted identity,
and the realm reads both record bumps out of it. And **the replay cursor,
entire**: its address is a PDA under CUSTODY'S OWN program id, so the caller
pins the seeds but does not choose which cursor this program advances; its owner
and exact width are a fact about the account, partitioned by operation; and the
revision the route advances is read from that account and never from the
request. Custody's `caller_authority.is_signer` conjunct lives in
`require_account_count`'s frame-spec privilege scan rather than beside the
derivation -- which is why grepping the file for it finds nothing and the
signature the ruling rests on is nonetheless required.

### (ii) The accelerator: the chain moved into the request

Everything in the chain is something Trading computed before it built the CPI.
`admitted_composition_v3` now writes the complete `AdmittedInvocationContextV3`
preimage and the two AccountProfile-derived geometry banks into an
`AdmittedPreludeWitnessV1` appended to the request.

| | before | after |
|---|---:|---:|
| the whole prelude | 256,650 | 165,153 |
| the whole accelerator invocation | 399,484 | 329,984 |

with `acc-manifest`, `acc-programset`, `acc-strategy` and `acc-artifact-records`
gone entirely, `acc-records` 4,211 to 516, `acc-artifacts` 11,092 to 9,001, and
a new `acc-witness` at 1,947. Draw-corrected the saving is about 99,000;
measured across the two ELFs it is 91,497, and both of the two rows that moved
against the change (`acc-caller-authority` +4,571, `acc-market` +3,000) are
whole numbers of `create_program_address` iterations.

**THE WITNESS RIDES OUTSIDE `hash(request_bytes)`, AND THAT IS FORCED.** This
note's design said "the caller-authority PDA already pins `hash(request_bytes)`,
so the request is a channel that costs nothing to widen." That is true of the
BYTES and false of the ADDRESS: a caller-authority PDA is an account that must
be in the frame before the transaction executes, so its address is derived
off-chain by a producer that reproduces the request exactly, and the witness is
composed on-chain out of values only the executing program has. The campaign
proved it in one run, refusing `0x4001` at `invoke_admitted_accelerator_v3`
because every caller-authority account in the fixture was at the old address.
`dclutch-custody-sbf`'s `split_caller_authority_bump_v1` met the same fixed
point in 2026-08 and resolved it the same way.

So the binding is the request's own `invocation_context` field, which IS inside
the signed prefix: the reader requires
`admitted_invocation_context_digest_v3(witness.context())` to equal it, and the
whole 756-byte preimage is committed by a value the caller signed. The
representative bank is committed one level in, by the context's
`runtime_observations_digest`, which the accelerator recomputes over bytes it
reads itself. **The span bank is committed by neither and the route refuses a
nonempty one** -- every family it serves asserts `span_widths().is_empty()` in
its own words, so the refusal costs no honest traffic; a dynamic-span profile on
this path is owed a binding before it is admitted.

**Twenty-three of the context's twenty-eight fields are rejoined against a
source on the accelerator's side of the boundary**, and the five that are not
are named rather than buried: `strategy` and `certificate` (also request header
fields, and compared against them), `admission`, `artifact_release` and
`lifecycle`. What is given up with them is the accelerator's own proof that the
admitted strategy names THIS program. It is not lost from the transaction --
Trading authenticates the strategy chain before composing the CPI -- it is lost
from this program.

**And the seal became the joint rather than a shortcut through one.** Its key is
(descriptor schema, descriptor digest, action, Trading semantic release,
Registry); the descriptor digest is the request's own `capability_program`, the
action is the witness's, the release is the activation's. A request naming an
action this Trading release never sealed for this descriptor has NO SEAL ACCOUNT
AT ALL -- which is what retires the manifest and program-set walk, a walk to the
same answer through twelve accounts.

**What did NOT move, because the accelerator is a second opinion on the
EVALUATION.** `acc-product-runtime` stays at 39,217: it supplies the payout
scale, the outcome count and the semantic basis, and those decide the
arithmetic. The input register bank is still read out of the runtime accounts.
The root prestate is still hashed from the root. And the observation digest
stays -- 30,285 of the new prelude -- because it is the ONLY thing binding the
runtime slice: `acc-toplevel` binds the forty-eight fixed accounts and the
evidence suffix to the top-level instruction, and the runtime slice is neither.

**The frame nearly ate the change, and the note predicted it.** "A partial cut
is not worth a trust change in a frame with 192 bytes of headroom." The first
version inlined the geometry banks, the eight-argument observation digest and
the rejoin: `authenticate_accelerator_invocation_v4` went from 3,904 to 5,248
and the linker emitted thirty-four overwrite diagnostics. The second, with the
rejoin in its own callee, was still 64 bytes over, because the join struct took
`HotFrameV3` -- thirty-nine account references -- and `HotExecutionEnvelopeV3`
by value. **The two callees this move deleted were load-bearing as FRAMES and
not only as code.**

### (iii) Claims, already spent at `0aa70478e`, and what the three have in common

All three programs found the same two things in the same proportion: a
redundant decode of one immutable Registry-owned account that a reading would
have found with no ruling at all, and a per-role deployment observation that the
ruling drops. In every case the activation identity itself STAYED, because the
seeds name a role and not a key. The ruling is not "trust the caller"; it is
"verify the signer's derivation, and stop re-deriving what the signer's seeds
already pin" -- and in all three programs the largest single line was something
else entirely.

### Where the wall is now

| | CU |
|---|---:|
| Trading entry through `before-commit` | 1,040,583 |
| Custody route 0 (the cash leg), with its frame build | 108,030 |
| **Claims route 1 -- COMPLETES** | 87,162 |
| inter-child span and Custody route 2's frame build | 41,492 |
| **Custody route 2, the MERGE -- COMPLETES** | 71,746 |
| `commit-lifecycle-closes` | 7,274 |
| remaining | **4,510** |

**Every one of the Remove's three child routes now executes and commits.** The
merge moved its tokens, wrote its poststate, advanced its replay cursor and
returned its receipt. What is left is the commit tail: about 53,600 CU --
48,536 `commit-non-root`, 2,498 `commit-root`, 2,387 `after-commit`, 226 --
measured end to end on the equity Add. **The Remove is short by about 49,000
CU**, down from 264,000 to 367,000 when this note opened.

Tonight the wall moved four times: from inside the Claims child's product/basis
join, to after the Claims child's commit, to inside the Custody merge, to inside
the commit tail.

And the neighbour that completes has room again. The two Add-shaped transactions
in the last run consume 1,025,642 and 1,032,528 of 1,399,700 -- **367,000 to
374,000 CU of headroom**, against 241,000 to 266,000 before tonight, on a family
that was running at 83 per cent of the ceiling when this note was written.

### What the next 49,000 costs, priced

| candidate | CU | measured? |
|---|---:|---|
| the shared permission bank: `p7e-permissions` retired outright, plus part of `pf-composition`'s third walk | 15,000 - 25,000 | the 14,800 row is measured; the ceiling is not |
| `acc-product-runtime`'s four record walks, bound by the digests the Market and the witness already pin instead of searched for | up to ~39,000, unknown split | not measured; the span is draw-free, so it is decode and not search |
| the three inter-child frame builds, 25,247 + 41,492 + 41,492 | ~108,000 | measured, and it is Trading's CPI account-passing rather than anyone's authentication |
| the commit tail itself | 53,600 | measured on the Add |

The first is the one this note already owns an author for. The second is the
largest single remaining number inside the accelerator and has never been read.
