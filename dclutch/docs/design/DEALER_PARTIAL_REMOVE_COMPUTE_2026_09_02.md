# The partial equity Remove's compute wall, priced

**Head current at `82465e00b` (2026-09-03), tree root `/Users/ember/dev/dclutch`; program-test evidence on real SBF ELFs, not devnet and not mainnet evidence.**
The nine addenda are kept verbatim below `## History`, several of them correcting each other; this head states only the survivors.

## What it costs now

The post-trade partial equity Remove — `accepted_equity_selector_one_executes_real_custody_and_rolls_back_late_evidence_refusal`
in `programs/dclutch-dealer-accelerator-sbf/program-test`, the first Dealer action carrying a nonzero `signed_position_count` —
opened this note **short of the 1,399,700 CU ceiling by about 367,000**. It now commits, and so do both LP final Removes behind it: worst
headroom over eight filtered runs at `82465e00b` is **74,637** for the partial Remove and **76,165** / **74,647** for the two final Removes,
with `accepted` at **31 passed / 0 failed** on three consecutive full runs. That is a property of ONE artifact set and eight payer draws, never
a fixed number — the campaign's `ArtifactRelease` records hash the ELFs they name, so rebuilding any program redraws every Registry-record
search depth, and ~45,000 CU moved between two commits with no code between them (eighth addendum). One build earlier, at `98113142b`, the
worst headroom was **20,024** and run 3 of 8 died at `exceeded CUs meter`.

## The three rulings this route spent

- **0022** (`docs/decisions/0022-pda-signed-caller-facts.md`): a callee invoked by a PDA-signed CPI from Trading takes the facts the signer's
  seeds pin as established. Spent in Claims `0aa70478e`, Custody `9b5de611e`, the accelerator prelude `742d7b7be`. Its OWN contribution in all
  three was only the per-role deployment observation it drops — decision 0012's `ReleaseSuperseded`, now inherited from the caller; the larger
  half was a redundant decode a reading would have found without any ruling.
- **0023** (`docs/decisions/0023-slot-free-caller-authority-seed.md`, `3a8ac205d`): a caller authority's address is a function of
  the signed instruction alone. `accelerator_caller_authority_digest_v1` (`crates/dclutch-execution-strategy-contract/src/shadow_digest_v3.rs:107`)
  is why the accelerator's caller-authority bump is minable in principle — `derive_admitted_authorities_v1`
  (`programs/dclutch-trading-sbf/program-test/bundle-builder/src/admitted.rs:225`) computes it off chain and discards it at `find_program_address(..).0`.
- **The seal** (decision 0005; `authenticate_capability_seal_v3`, `programs/dclutch-trading-sbf/src/hot_v3/seal.rs:789`): at `742d7b7be` it became
  the accelerator's joint rather than a shortcut through one. Its key is (descriptor schema, descriptor digest, action, Trading semantic release,
  Registry), so a request naming an action this release never sealed has no seal account at all — which retired the manifest and program-set walk.

## What is still draw, and who could carry it

| term | spread over the eight runs | carrier |
|---|---:|---|
| `cx-accelerator-frame` + `acc-caller-authority` | 7,500 + 7,500 | Trading's half, the bump above — but `HotBumpHintsV1`'s slots are ROLES and none names the accelerator's own caller authority (`crates/dclutch-capability-program-contract/src/hot_v3.rs:275-280`); the accelerator's half is a process-local relay |
| `cx-accelerator-returned` | 9,000 | not located |
| `cu-transfer-validated`, twice | 6,000 + 6,000 | **none** — `validate_vault_key` (`programs/dclutch-custody-sbf/src/lib.rs:1897`) searches both vaults, `CustodyBumpRelayV1` is three bytes all spoken for, and a token account cannot carry its own bump |
| `cu-common-frame`, twice | 4,500 + 4,500 | the replay cursor, which needs the child projection the campaign builder does not do |
| `pf-invocation-preflighted` | 6,000 + 3,000 | **none** — child-caller seeds end in `hash(child_request_bytes)`, projected on chain |
| `cx-claims-frame` + `sd-authority` | 3,000 + 3,000 | Claims' half only: a three-byte relay in the shape `split_caller_authority_bump_v1` (`programs/dclutch-custody-sbf/src/lib.rs:469`) already has |

The largest **draw-free** debt is `execution_strategy_v2`'s own record walk — 82,308 CU, flat to 6 CU across eight runs, with no carrier left:
`HotBumpHintsV1` is full, `StateBumpsV1` has one reserved byte, `SelectedRecordBumpsV1` fills the capability root's four, and the seal is keyed
by descriptor. It is also **no longer priced**, per the third corollary below.

## The design law

**The expensive thing was never the check; it was a fact re-derived by someone who already had it.** Every cut here is that shape
at a different joint: a callee re-authenticating what its PDA-signing caller established (0022); one immutable Registry-owned
activation account hostile-decoded three times per invocation, with `validate_projection` running twenty-five `decode_role` calls
for five values (`crates/dclutch-registry-contract/src/activation.rs:252`, repaired `5709672aa`, −12,021 CU per decode site); a producer that
derived an address in order to NAME the account and then let the program search for it again (`dclutch-chain-bundle-builder` mined no
`HotBumpHintsV1` slot until `82465e00b`); and one program walking ten Registry addresses twice, in `execution_strategy_v2` and again in
`validate_authenticated_frame`. **The repair is always a CARRIER for the fact, never a weakening of the check** — nothing authenticated
changed, and the residual is `create_program_address` at ~1,500 CU a call.

Three measurement corollaries, each paid for by asserting its opposite first: **draw-free is not search-free** (wrong three times before a doubling
probe settled it); **a span measured on the neighbour is an estimate** (the commit tail taken on the equity Add put the shortfall a third too low);
and **a doubling probe over seeds that include an ELF digest is not a probe** — it rebuilds the program and redraws every depth, which is how 37,640
was read for a span whose total was 73,308.

## History

*Everything below is this note as written between 2026-09-02 and 2026-09-03, unchanged and in order. Where an
addendum contradicts the head above, the head is the current truth.*

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

## Fifth addendum, 2026-09-03: the Remove COMMITS, the shortfall was a third larger than this note said, and one span was 77 per cent search

*Measured at `4542a9e8a` (before) and `c81c94d91` (after), tree root
`/Users/ember/dev/dclutch`, real SBF ELFs built in this lane's own worktree with
its own target directory, **zero SBF stack-frame-overwrite diagnostics on every
one of the six links built for every figure below**. Dealer accelerator campaign
30 passed / 1 failed throughout.*

### The action commits

    census lp-hot legacy=3557 v0=1406 unique_locks=71 ... data=1048
    Program F48Umd... consumed 1397966 of 1399700 compute units
    Program F48Umd... success

Every one of the three child routes executed, and so did the whole commit tail:
`commit-lifecycle-closes` 1,524, **`commit-non-root` 80,216**, `commit-root`
2,965, `after-commit` 2,387, finishing with **1,970 CU**.

**And it does not do it on every draw.** Three re-runs on the IDENTICAL ELF set,
which redraw only the fixture payer: two put the Remove back over the ceiling at
1,399,692, and the third cleared it AND the two LP final Removes behind it
(1,388,948 and 1,382,950) that no run of this test has ever reached. The margin
is smaller than one `create_program_address` iteration. **"Commits" here means
"is inside the ceiling on a favourable draw", not "is inside it."**

### The shortfall this note has been quoting was a third too small

`4bfe5394b` measured the commit tail END TO END ON THE EQUITY ADD -- 48,536
`commit-non-root` + 2,498 + 2,387 + 226 = 53,647 -- and concluded the Remove was
"short by about 49,000". The Remove's own `commit-non-root` is **80,216**,
because it commits more accounts, so its tail is **86,332** and the true
shortfall at `742d7b7be` was about **82,000**. Every "the next 49,000" figure in
the fourth addendum is a third low. The lesson is the one this note keeps
re-learning at a different joint: a span measured on the neighbour is an
estimate, and it must be labelled one.

### `acc-product-runtime` was 77 per cent PDA SEARCH, and this note reasoned the opposite

The fourth addendum's table says of this row: *"not measured; the span is
draw-free, so it is decode and not search."* **Draw-free and search-free are not
the same property, and the inference is wrong.** `authenticate_record` runs TWO
`find_program_address` calls per record -- raw body and staging cursor -- and the
Product graph walk covers four records. Their seeds are a PDA domain, a
canonical schema id and a **content digest**: fixture data, none of which moves
with the release-set id. So the eight searches run at a FIXED depth that is
identical across every ELF, which is exactly what "draw-free" observes.

Priced by doubling them, on real ELFs:

| | `acc-product-runtime` |
|---|---:|
| the span, on nine invocations of one run | 39,217 |
| the same span with the eight searches doubled | **69,389** |
| **so the searches are** | **30,172 (77%)** |
| and the four hashes, four decodes and the identity join are | 9,045 |

### What that bought, and what it costs to keep

The Dealer accelerator runs this walk a SECOND time, independently, over the
same four Registry records its caller authenticated a few thousand instructions
earlier. `admitted_composition_v3` now relays the eight bumps its own prelude
derived, in the prelude witness's header, and the accelerator reproduces each
address with `create_program_address`:

| | before | after |
|---|---:|---:|
| `acc-product-runtime` | 39,217 | **21,750** |

**−17,467, draw-free on both sides.** The residual 12,000 is eight
`create_program_address` calls at 1,500 each, which is that design's floor: a
hint does not make a derivation free, it makes it O(1). That is why the probe's
30,172 and the landed 17,467 are different numbers, and quoting the first as the
saving would have been wrong by 12,000.

**Nothing moves in what is authenticated.** Each bump is fed to a derivation
over seeds the accelerator builds for itself, and the address is compared
against the account the frame supplied by the equality that was always there.
Canonicality is enforced where the record is MADE -- the Registry writes
finalized records only at the canonical bump -- so a non-canonical hint names an
address at which no Registry-owned record exists.

**Reading a hint must not be able to refuse, and `frontier` said so.** The first
version decoded the witness before the Product walk and propagated its error;
`frontier`'s marker moved from `AcceleratorArtifact` back to
`AcceleratorRuntimeView` -- a hint reader reporting a conjunct it does not own.
`accelerator_record_bump_hints_v4` is total: an unreadable witness yields the
absent bank and the walk searches exactly as it used to, while the witness's own
decode still refuses by name for every field it owns.

### The Dealer family had never mined a single bump hint

`HotBumpHintsV1` has been read by Trading, the accelerator and Custody since it
was added, and **the only producer that ever filled it is `direct_inline_v3`**.
Every Dealer packet this tree has emitted carried the all-zero block and every
reader on the route searched. This is the producer-missing shape: reader,
schema and fallback all built, producer never written for this family, and
nothing goes red because the fallback is correct and merely slower.

`dealer_lp_hot_v4` now mines `market`, `root` and Custody's transfer authority.
Being HOST-side, the ELFs stay byte-identical and the draw does not move, which
makes the reading exact: **`cu-transfer-validated` falls by 4,500 on BOTH Custody
legs**, and every other movement in that run is a whole number of iterations in
both directions, because the request digest changed and the caller-authority
depths redrew. **The hint is 9,000; the rest was the draw.**

`child_relay[0]` (Custody's own replay) and `lifecycle` stay zero: this builder
is handed the family request and does not project the children, which is where
`direct_inline_v3` leaves its two child caller slots for the same reason.

### The inter-child block, decomposed

`4bfe5394b` priced it at 25,247 + 41,492 + 41,492 and called it "Trading's CPI
account-passing rather than anyone's authentication". Six new checkpoints --
`cw-dependencies`, `cw-child-returned`, `cw-banked`, `cx-custody-frame`,
`cx-claims-frame` and `cx-accelerator-frame` -- say what it actually is:

| | CU |
|---|---:|
| Trading builds the accelerator's frame and request | 44,862 |
| the runtime's CPI charge + the accelerator's entry | 77,743 |
| Custody leg 0: dependencies 4,364, frame 7,314 | 11,678 |
| the runtime's CPI charge + Custody's entry | 14,263 |
| Custody returns: 3,541 + banked 2,932 + next deps 2,176 | 8,649 |
| Trading builds the Claims frame | 16,134 |
| the runtime's CPI charge + Claims' entry | 26,986 |
| Claims returns: 11,417 + banked 3,572 + next deps 6,148 | 21,137 |
| Custody leg 2: frame 7,474 | 7,474 |
| the runtime's CPI charge + Custody's entry | 14,263 |
| Custody returns: 3,486 + banked 2,952 | 6,438 |

**About 76,000 is Trading building four child frames, about 133,000 is the
RUNTIME's own charge for passing seventy-four accounts four times, and about
34,000 is the receipt bank and the provenance derivation between children.**
Only the first of those three is anyone's to cut, and the row that called the
whole block "Trading's CPI account-passing" was attributing the runtime's price
to us.

### A SECOND WALL IN THIS TEST, and it has been misreported as the first

`accepted.rs:2407` -- *"every page must commit"* -- is a scenario-checkpoint page
whose own comment already names itself as DEBT: pages 0 and 1 cost **1,192,550
and 1,305,050** of a 1,399,850 ceiling. On an unlucky draw page 0 goes over and
the test dies there, **having never reached the Remove at all**.

That is what the predecessor lane's own `campaign-acc4` run did on 2026-09-03,
and it was reported as "30 passed / 1 failed, the failure still this Remove"
because the COUNT was unchanged. **Two different walls in one test produce the
same 30/1**, and which one fires is decided by the deployed ELF set, because
`release_set_id` hashes it -- including the test caller, whose only difference
between two lanes was the absolute path of the worktree it was built in.

The page's cost is dominated by hashing every observation account's complete
data, on top of two unhinted `find_program_address` calls whose depth is drawn:
`require_checkpoint_pda` and the membership manifest PDA. **Hinting those two is
what would make this test's outcome reproducible at all**, and it is owed.

### Where the wall is now

| | CU |
|---|---:|
| entry through `before-commit` | 987,877 |
| **Custody route 0**, its dependencies, frame build, CPI charge and receipt | 93,943 |
| **Claims route 1**, the same four | 135,003 |
| **Custody route 2, the merge**, the same four | 93,815 |
| **the commit tail** -- 1,524 + 80,216 + 2,965 + 2,387 | 87,092 |
| **remaining** | **1,970** |

and the neighbours: the two Add-shaped transactions in that run consume
1,012,702 and 1,013,314 of 1,399,700 -- **386,000 CU of headroom**, against
367,000-374,000 at `742d7b7be`.

### What remains, priced

| candidate | CU | measured? |
|---|---:|---|
| **Trading's OWN Product graph walk**, the twin of the one just hinted, inside `root-product` | **~18,000** | the search is measured at 30,172 and a hinted walk keeps 12,000 |
| the shared permission bank: `p7e-permissions` retired outright plus part of `pf-composition`'s third walk | 15,000 - 25,000 | the 14,800 row is measured; the ceiling is not |
| the three child caller-authority searches in `pf-invocation-preflighted` | 26,820 in this run | measured, but `HotBumpHintsV1` has two `child_caller` slots and this route has three children |
| Trading's four child frame builds | ~76,000 | measured, and now split from the runtime's charge |

**The first row has an owner and no channel.** Trading's prelude cannot be
hinted from the packet: `HotBumpHintsV1` is family-neutral, all eight slots are
allocated, and its own doc records why the envelope cannot grow. The right
carrier is the one this tree already built for exactly this shape -- the Market's
`StateBumpsV1`, which carries `market`, `realm_raw_record` and
`realm_staging_record`, three bumps of a Registry record pair, for precisely
this reason. It is one record family short of covering the Product graph.
Widening it is a Lean-emitted persisted layout and a migration, so it is Core's
and not a lane's.

**Author:** the Market's `StateBumpsV1` widening is Core's, jointly with
`dclutch-product-runtime-v2-svm-reader`'s, which now has the hinted entry point
waiting for it.

### The witness's two caller-composed banks: one has a consumer, one does not

Asked what remains for "the alias row", I could not find a row under that name
in this note, so here is what the alias machinery actually does after this
commit, verified rather than guessed:

- **the representative bank HAS a consumer and it is load-bearing.** It is the
  per-logical-coordinate route-alias table, and
  `accelerator_runtime_observations_digest_v4` walks it to take the digest that
  binds the runtime slice. It is committed one level in, by the context's
  `runtime_observations_digest`, which this program recomputes over bytes it
  read itself. The row stays.
- **the span-width bank HAS NO CONSUMER, and the route refuses it nonempty.**
  `admitted_composition_v3` writes it; `authenticate_accelerator_witness_v4`
  requires `witness.span_count() == 0` and refuses otherwise, so the only value
  the producer can legally emit is the empty one -- a producer whose output no
  reader may read. Every family this accelerator serves asserts
  `span_widths().is_empty()` in its own words, so the refusal costs no honest
  traffic today. **Its consumer would be a dynamic-span profile on the admitted
  accelerator path**, and what that consumer needs first is a BINDING: the span
  bank is committed by neither the request digest nor the context's observation
  digest, which is why the route refuses it rather than believing it. Until
  that binding exists the producer side is dead weight and should be read as
  such -- it is one `u32` per span in a bank that is always zero-length.

The new record-bump bank is deliberately the third shape and neither of those
two: it is a hint whose whole check is the derivation that consumes it, so it
needs no binding at all, and reading it cannot refuse.

---

## Sixth addendum, 2026-09-03: the second wall was the page PARTITION, the Market carries the Product graph, and the span bank has a consumer after all

*Measured at `cee27ff16` (page hints and the balanced split) and `b312ce3c4`
(the Market's Product-graph bumps), tree root `/Users/ember/dev/dclutch`, real
SBF ELFs built in this lane's own worktree with its own target directory, zero
SBF stack-frame-overwrite diagnostics on every one of the twenty-four links
built for the figures below. Dealer accelerator campaign 30 passed / 1 failed
throughout, the failure the partial Remove. Frame rows landed with both
commits; the ratchet was red at `c81c94d91` and is green at every commit since.*

### The second wall is not the one the fifth addendum named

That addendum said of the scenario-checkpoint page: *"The page's cost is
dominated by hashing every observation account's complete data, on top of two
unhinted `find_program_address` calls whose depth is drawn... Hinting those two
is what would make this test's outcome reproducible at all."* The first clause
is right. The second is wrong by two orders of magnitude, and the measurement
that separates them is three runs on ONE ELF set.

**The two hints are real and they are worth 1,500 CU each.** The page
instruction grows a two-byte mined tail — the checkpoint PDA's bump under
Trading and the membership manifest's under its producer — and 409 wire bytes
on the widest page become 411.

| six-page total, one ELF set, three runs | | | | spread |
|---|---:|---:|---:|---:|
| unhinted | 2,901,035 | 2,910,035 | 2,910,035 | **9,000** |
| hinted | 2,883,290 | 2,883,296 | 2,883,290 | **6** |

The 9,000 is exactly six `create_program_address` iterations over the twelve
sites a scenario pays, and the residual 6 is not a search at all.

**And the page's outcome still depended on the draw, by 688,860 CU.** The same
three hinted runs, per page:

| page | run 1 | run 2 | run 3 | |
|---|---:|---:|---:|---|
| 0 | 1,192,491 | 1,192,569 | 1,192,161 | |
| 1 | **1,305,100** | **616,240** | **1,304,470** | spread 688,860 |
| 2 | 26,868 | 714,918 | 26,764 | spread 688,154 |
| 3 | 22,801 | 22,920 | 22,683 | |
| 4 | 313,206 | 313,172 | 314,193 | |
| 5 | 22,824 | 23,477 | 23,019 | |
| total | 2,883,290 | 2,883,296 | 2,883,290 | **spread 6** |

The same total work, distributed differently. The canonical membership split
was EQUAL-COUNT over a key sort, and the observations differ in width by four
orders of magnitude — a loader ProgramData body against a 368-byte Market
header — so which page a megabyte-wide account lands on is decided by a keypair
the campaign draws fresh. At 1,305,100 of a 1,399,850 ceiling, one more account
on the wrong side of a boundary is a failed page and a campaign that dies
without ever reaching the action under test. **That is the failure the
predecessor lane observed and attributed to the bump draw.**

### So the split balances hashed BYTES, and the widest page stopped moving

An account costs a page its data plus the 81 bytes the receipt digest hashes
beside it. The producer now minimizes the widest page — binary search over
capacities with a greedy feasibility walk — keeping the partition contiguous in
key order, which is the route's only ordering conjunct, and every page nonempty
and inside the manifest's 48-account ceiling. **Nothing on chain changes:**
`page_account_counts` has always been per-page data in the producer-owned
manifest, and an equal-count manifest is still one this route accepts.

Three more runs on the SAME ELF set — the change is host-side, so the
executables and every draw are unmoved and the comparison is exact:

| page | run 1 | run 2 | run 3 |
|---|---:|---:|---:|
| 0 | 20,075 | 19,615 | 21,137 |
| 1 | **1,182,094** | **1,182,094** | **1,182,094** |
| 2 | 619,344 | 619,158 | 617,478 |
| 3 | 724,734 | 725,223 | 724,646 |
| 4 | 323,627 | 323,440 | 323,667 |
| 5 | 14,860 | 15,198 | 15,626 |
| total | 2,884,734 | 2,884,728 | 2,884,648 |

The widest page fell from 1,305,100 to 1,182,094 and is identical to the digit
across three runs. Headroom on it is 217,756 of 1,399,850 — 15.6 per cent,
against 6.8. The residual on the other five is 227 to 1,866 CU, and it is the
boundary shifting by a small account rather than a search: the six-page total
holds to within 86. **1,182,094 is the FLOOR of this shape**, the widest single
observation; no partition puts less than one account on a page, and getting
under it means the page route not hashing complete account data, which is a
different design.

### The Market carries the Product graph, in four bytes it already had

`c81c94d91` retired the ACCELERATOR's Product-graph walk with a witness relay
and said Trading's own copy had no carrier, because `HotBumpHintsV1` is full
and the Market's `StateBumpsV1` was "one record family short". It is four
record families short — eight bumps, two per record — and five reserved bytes
do not hold eight.

**They hold eight nibbles.** A recorded nibble `v` is the bump `256 - v`,
carrying 255 down to 241; zero is unrecorded and its reader searches; a bump
below 241 is recorded as unrecorded, which costs a search on about one
derivation in 32,768 and can never cost a refusal. `STATE_BYTES` stays 368, so
**there is no migration to make**: an account written before the field existed
holds zeros, decodes, works, and keeps the search. The alternative — appending
eight bytes — is what `tools/release/README.md` already prices: Core's only
`resize` is `resize(0)`, so every market already written would be refused by
length forever.

| Trading `start` -> `root-product`, one byte-identical ELF set | |
|---|---|
| Market records nothing | 104,040 / 105,540 / 105,540 / 104,040 / 104,040 / 107,040 |
| Market records the graph | 89,139 / 86,139 / 87,639 |

**A saving of 14,901 to 20,901, centre about 17,900**, which is the fifth
addendum's predicted ~18,000. The 12,000 it keeps is eight
`create_program_address` calls at 1,500, this design's floor.

**The producer half is where the work actually was.** Three separate producers
had to learn to record, and one of them turned the whole change into a no-op
until it did: with the reader landed and the campaign's fixture still writing
`StateBumpsV1::UNRECORDED`, `root-product` was 104,040 before and 104,040
after, to the digit, and a probe on the campaign's own Market read eight zeros.
`AuthenticatedProductRuntimeV2` also had to start CARRYING the six bumps its
walk derives rather than writing them into a caller's out-parameter the
unhinted entry points threw away — without that, Core's founding would have
persisted six zeros beside two bumps and nothing would have gone red. And the
local-validator founding driver predicts `sha256(CoreState)` two stages before
Core writes one, so it predicts the new tail too; its own doc records that a
tail left zero moves the permit digest and refuses three legs later naming
nothing.

### Where the Remove's wall is, and what this does not claim

On this ELF set the partial equity Remove does **not** commit, on any of three
draws. What the hint moved is the wall: without the Market's bumps it dies
before `commit-lifecycle-closes`; with them it reaches the commit tail with
26,684 / 34,174 / 44,684 CU remaining against the 85,568 that tail needs, so it
is short by **40,884 to 58,884**.

`c81c94d91` saw it commit with 1,970 CU left on a favourable draw and said so.
The distance between "commits with 1,970" and "short by 51,000" IS the draw,
and that is the honest reading of a margin smaller than one
`create_program_address` iteration: **it was never headroom.** Every remaining
candidate in the fifth addendum's table is still owed, and the Remove needs
about 50,000 of them before "commits" stops meaning "on a favourable draw".

### The span-width bank HAS a consumer, and the route that consumes it is dead

The fifth addendum said the span bank "HAS NO CONSUMER, and the route refuses it
nonempty... the producer side is dead weight and should be read as such." That
reading was taken over the equity and LP families, which do assert
`span_widths().is_empty()`. **There is a third family, and it needs nine.**

    programs/dclutch-trading-sbf/src/dealer/v3_accelerator_accounts.rs:138
        let spans: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4] = invocation
            .span_widths()
            .try_into()
            .map_err(|_| DealerScenarioAcceleratorErrorV4::Invocation)?;

`DEALER_SCENARIO_PROFILE_SPANS_V4` is 9. The chain is four links and every one
of them is unconditional:

1. `dclutch-dealer-accelerator-sbf/src/lib.rs:409` dispatches the selector-9
   Dealer scenario family to `evaluate_authenticated_dealer_scenario_v4`;
2. that evaluator's first substantive act is the nine-width `try_into` above;
3. `invocation.span_widths()` is the `Vec<u32>` built in
   `authenticate_accelerator_invocation_v4`, whose sole constructor is
   `hot_v3.rs:1277`;
4. it is filled by `authenticate_accelerator_witness_v4`, which refuses
   `witness.span_count() != 0` and then loops `0..span_count()` — so it is
   **always empty**, and the `try_into` into `[u32; 9]` **always fails**.

**The selector-9 Dealer scenario family is unconditionally refused by the
admitted accelerator, with `DealerScenarioAcceleratorErrorV4::Invocation`, on
every input.** Nothing is red because nothing exercises that route through the
accelerator: `accepted.rs` runs the equity and LP families over the split
checkpoint routes, and `physical.rs` refuses on the frame long before the
witness. This is the producer-missing shape one turn around — reader, schema
and refusal all built, and a blanket refusal on the carrier that the third
family's reader depends on.

**So the bank is not removed, and the binding it needs is not a digest.** The
obvious repair — commit the span bank in `context.runtime_observations_digest`,
the way the representative bank is committed — is exactly what the fourth
addendum forbids by name: *"A request field that carried an evaluation INPUT
rather than an authentication RESULT would make the accelerator a mirror of its
caller."* Span widths shape the account frame the transition evaluates over;
they are an evaluation input. A caller-signed commitment to them is the caller's
word, not a fact about an account the accelerator reads.

The binding therefore has to be a DERIVATION on the accelerator's side:
`authenticate_dynamic_span_widths_v3` already computes widths from the
AccountProfile's dynamic span rules and the runtime scalars, which is the same
shape as `acc-product-runtime` staying at 39,217 because it decides the
arithmetic. If the accelerator derives them, the witness's span bank is
redundant and should go with the refusal; if it cannot (the profile moved into
the witness at `742d7b7be`), then the widths are the first value on this route
that needs an authenticated carrier of its own.

**Author:** the Dealer accelerator's owner jointly with `hot_v3`'s. It is a
route-liveness question, not a compute one, and it is the first thing the next
lane should read.

### What this lane owes

- **The three rejoin hostiles have no home yet, and that is a finding rather
  than a deferral.** `accepted.rs` runs the real Trading ELF, which composes the
  witness on chain, so there is no seam to inject a tampered one; the only
  injection seam is the test caller's verbatim request account, which
  `physical.rs` uses and which sits behind a prelude no complete chain fixture
  has ever been staged for. `dealer_chain`'s only constructor,
  `project_dealer_scenario_unsplit_chain_topology_v4`, still has no caller in
  the tree. The hostiles are blocked on that fixture, which is a unit of its
  own and not a hostile-sized one.
- The scenario route's span widths, above.
- `capability_funding_header_v2`'s generator-freshness test needs a local
  `.lake/build` and fails here for want of one; the market-core emitter this
  work does change was run and its output matches the checked-in file byte for
  byte.

---

## Seventh addendum, 2026-09-03: one record walk instead of two, the permission byte the projection already had, and the draw is now bigger than the shortfall

*Measured at `6380cdf3c` (before), `9ade7439a` (the two cuts) and `07184fa82`
(selector 9), tree root `/Users/ember/dev/dclutch`, real SBF ELFs built in this
lane's own worktree with its own target directory, zero SBF
stack-frame-overwrite diagnostics on every one of the thirty links built for the
figures below. Dealer accelerator campaign 30 passed / 1 failed throughout.
Frame rows landed with each commit.*

### The sixth addendum's "four inter-child frame builds" was two-thirds something else

That note priced `Trading's four child frame builds ~76,000` and named them as
the next candidate. Decomposed at `6380cdf3c` by five new sub-checkpoints, the
largest of the four is not a frame build at all:

| span | CU | what it is |
|---|---:|---|
| `cx-witness-encoded` | 2,450 | the context preimage and the prelude witness |
| `cx-context-digest` | 1,147 | the 756-byte context digest |
| **`cx-frame-validated`** | **41,766** | `validate_authenticated_frame` |
| `cx-request-built` | 4,286 | the input bank, its digest, the first request |
| `cx-cpi-buffers` | 8,140 | the 48+N metas and infos, built once |
| `cx-accelerator-frame` | 3,796 | request encode, digest, caller-authority search |
| the three child frame builds | 7,314 + 16,134 + 7,474 | the actual frame builds |

**Sixty-five per cent of the accelerator's leg-entry span is a PDA walk**, and it
is the second walk over a set of ten addresses this program had already derived.

### Both walks priced, by doubling each

`admitted_composition_v3::validate_authenticated_frame` re-derives the
descriptor, strategy, certificate, admission and artifact-release record pairs
that `execution_strategy_v2` derived while it authenticated them a few thousand
instructions earlier. Doubling each walk's `find_program_address` calls on real
ELFs:

| walk | span it lives in | searches cost |
|---|---|---:|
| `execution_strategy_v2`'s, over four records | `artifacts-strategy-effect` (89,704) | **37,640** |
| `validate_authenticated_frame`'s, over five pairs | `cx-frame-validated` (41,766) | **29,235** |

The seeds are a PDA domain, a canonical schema id and a content digest — fixture
data, none of which moves with the release-set id — which is why both spans read
draw-free while being almost entirely search. **Draw-free is not search-free**,
for the third time in this note.

`AuthenticatedExecutionStrategyV2` now carries the ten bumps its own walk
derived and the second walk reproduces each address with
`create_program_address`: **41,766 to 19,866, draw-free on both sides.** The
residual 13,500 is nine `create_program_address` calls at 1,500, this design's
floor, exactly as `acc-product-runtime` kept 12,000 for eight.

### The permission byte the projection already had, and the half that mattered

`p7e-permissions` — 14,800 CU over about seventy-four coordinates — decoded
every coordinate's rule, and for a route alias its representative's, to keep one
byte per coordinate. The account projection had decoded the same rules a phase
earlier and thrown both away. `validate_accounts` and its dynamic twin now emit
the bank as they go.

**The first version was worth 2,612 and the second is worth 11,451**, and the
difference is one decode. Emitting the permission with a separate
`expanded_rule(representative)` cost the projection 11,093 to save 13,705;
decoding the representative's rule ONCE and handing it to both the privilege
check and the permission byte costs 2,254.

| span | before | after | |
|---|---:|---:|---|
| `p5r-account-projection` | 93,833 | 96,087 | +2,254 |
| `p7e-permissions` | 14,800 | 1,095 | **−13,705** |

Both identical to the digit across three runs on each side. The note's estimate
for this row was "15,000 to 25,000, the 14,800 is measured and the ceiling is
not"; the ceiling was not there, because the third walk in `pf-composition` is
the *dynamic* twin and this route's profile does not use dynamic fixed spans at
all — a doubling probe on `expanded_rule_with_dynamic_spans` moved
`p5r-account-projection`, `p7e-permissions`, `p7-effect-projection` and
`pf-composition` by **zero CU each**.

### Where the Remove is now

Three runs on one ELF set, against the 85,568 CU the commit tail needs after
`commit-lifecycle-closes`:

| | run 1 | run 2 | run 3 |
|---|---:|---:|---:|
| remaining, at `6380cdf3c` | 16,184 | 26,672 | 38,674 |
| remaining, at `9ade7439a` | 61,681 | 57,225 | 75,183 |

and on the middle draw **the partial equity Remove COMMITS** — 1,389,323 of
1,399,700, 10,377 CU of headroom — **and so does the first LP final Remove
behind it** at 1,392,291, with the failure moving to `accepted.rs:9509`, the
SECOND LP final Remove, which no run of this test has reached. The other two
draws are short by 10,385 and 23,887.

One draw of three, not three of three. The draw-free part of the improvement is
**33,351**: 21,900 from the record walk and 11,451 from the permission bank.

### The draw is now bigger than the shortfall, and it is measured

Across three runs of one ELF set the spread is about **96,000 CU**, against a
worst-case shortfall of 23,887. Every term is a whole number of
`create_program_address` iterations:

| span | spread |
|---|---:|
| **`cx-accelerator-returned`** (the Dealer EQUITY evaluator) | **27,000** |
| `cu-transfer-validated`, twice | 9,000 + 9,000 |
| `cx-claims-frame` | 9,000 |
| `sd-authority` (Claims' own caller-authority search) | 9,000 |
| `sd-candidates` | 7,500 |
| `pf-invocation-preflighted`, three times | 3,000 + 6,000 |
| `cu-common-frame`, twice | 4,500 + 4,500 |
| `root-product`, `acc-release-waist`, `cx-accelerator-frame`, `acc-caller-authority` | 3,000 each |

**The largest single term is inside the accelerator's own evaluator**, and it is
not what the fifth addendum would have guessed: `v4_equity_accelerator_accounts.rs`
runs nine `find_program_address` sites plus one per Claims position, over seeds
drawn from fixture keypairs — the Claims aggregate, each protocol position, the
Core Market state, the LP position, the Custody authority, the Custody replay
and the realm record pair. Doubling all of them on one run priced them at
**15,341**, which is depth one on that draw: they are cheap when lucky and
27,000 when not. **Hinting them does not lower the best case and removes the
worst**, which is exactly what "commits on three consecutive runs" needs.

**The carrier is already in the wire.** `DealerEquityRequestV3::decode` requires
four bytes at 476..480 to be zero — eight nibbles under `b312ce3c4`'s encoding,
enough for the eight fixed sites — and the producer derived every one of those
addresses in order to name the accounts at all. This is the shape `cee27ff16`
gave the page route and `c81c94d91` gave the Product graph, host-side on the
producer and a total reader on the program.

### What the Remove still owes, priced

| candidate | CU | measured? |
|---|---:|---|
| the equity evaluator's own searches, hinted from the request's reserved bytes | 0 best draw, **up to 27,000** worst | measured as a spread; 15,341 on one draw by doubling |
| **`execution_strategy_v2`'s own record walk**, the twin of the one this addendum cut | **25,640** (37,640 search, 12,000 floor) | measured by doubling; draw-free |
| Claims' `sd-authority` and Trading's `cx-claims-frame`: the child caller authority, searched on both sides | 18,000 worst draw | measured as a spread |
| Custody's `validate_vault_key`, two searches per leg | 18,000 worst draw | measured as a spread |

**The second row is the largest draw-free number left and it has no carrier**,
which is why this commit does not take it. Its bumps cannot ride a
content-addressed artifact — a Registry record's address is Registry-relative,
which is the reason `SelectedRecordBumpsV1` lives in the Market's root at all.
The Market's `StateBumpsV1` has one reserved byte left after `b312ce3c4` took
four. `HotBumpHintsV1` is full and its own doc records why the envelope cannot
grow. And the capability seal, which does carry raw and staging ADDRESSES per
row, is keyed by descriptor and never witnessed the strategy chain's cursors. A
mined tail on the Trading hot instruction, in the shape `cee27ff16` gave the
page route, is the carrier that fits, and it is a unit of its own.

**The three child caller-authority searches cannot be mined at all**, and this
is worth recording so the fifth addendum's row is not attempted: their seeds end
in `hash(child_request_bytes)`, the child requests are projected on chain out of
the candidate the accelerator produces, and no off-chain producer can know them.
That is why `direct_inline_v3` leaves `HotBumpHintsV1`'s two `child_caller`
slots zero as well. The reachable half is Claims' own second search for the same
authority Trading just derived — a three-byte relay in the shape Custody's
`split_caller_authority_bump_v1` already has.

### Selector 9's route was dead, and it is a derivation that revives it

The sixth addendum's finding is confirmed and repaired. `span_widths()` is empty
on every admitted invocation because `authenticate_accelerator_witness_v4`
refuses a nonzero span count, so the scenario evaluator's
`try_into::<[u32; 9]>` failed on every input and the family was refused
unconditionally.

The repair is a derivation on the accelerator's side, because the fourth
addendum forbids the binding: span widths shape the frame the transition
evaluates over, so a caller-signed commitment to them is the caller's word.
`dealer_scenario_span_widths_v4` reproduces what
`authenticate_dynamic_span_widths_v3` computes — the six optional-Custody route
widths `f5d4912e` put in the request header at 384..389, the Claims position
count, the trailing evidence count and the fixed six-page scratch width — and
the two conjuncts that were already there make it safe: the geometry admits only
the widths the profile's own rules admit, and `frame.logical_account_count ==
runtime.len()` pins the total against the runtime slice the accelerator hashes
for itself. The scenario evaluator now asserts `span_widths().is_empty()` in its
own words like the other two families, so a caller-supplied nonempty bank is
refused twice.

**What is not proven**: the selector-9 trade leg executing through the ADMITTED
accelerator on real ELFs. There is no seam — `accepted.rs` never submits a
selector-9 trade, `physical.rs` refuses on the frame before the witness is read,
and `project_dealer_scenario_unsplit_chain_topology_v4` still has no caller.
That is the same fixture the three rejoin hostiles are blocked on. The test that
does exist checks the derivation against the AUTHORITY rather than beside it: it
builds the scalar bank the RequestProfile would have written, asks the encoded
AccountProfile for the nine widths, and requires the two to agree — proved red
by swapping two slots before it was trusted green.

**And the witness's span bank now has no reader at all.** All three families
require it empty and this route derives its own widths, so the producer in
`admitted_composition_v3` writes a section every consumer refuses to be nonzero.
Its deletion from `AdmittedPreludeWitnessV1` is a wire change with a round-trip
test and should carry its own measurement.

### What this lane owes

- The three rejoin hostiles, still blocked on the chain fixture the sixth
  addendum named. Nothing changed that.
- The witness span bank's deletion, above.
- The four priced candidates in the table above, of which the equity evaluator's
  hint is the one that turns "commits on a favourable draw" into "commits".
- Two pre-existing reds were fixed in passing rather than left:
  `dealer_scenario_checkpoint_v1`'s selector test still spelled a nine-byte page
  instruction after `cee27ff16` grew it to eleven, and
  `dclutch-resolution-proof-sbf`'s synthetic provider fixture was left behind by
  `b312ce3c4`'s widening of `AuthenticatedProductRuntimeV2`, which had made
  `cargo check --workspace --tests` red.

## Eighth addendum, 2026-09-03: the Remove commits, both final Removes commit, the campaign is 31 of 31 — and about 45,000 CU of that is the ELF digest, not the code

*Measured at `2fbd6adf3` (the control, taken in its OWN worktree so the host
tree and the executables match), `3c42f0ece` (the hints) and `40427e0f1` (the
witness cut), tree root `/Users/ember/dev/dclutch`, real SBF ELFs built in this
lane's own worktree with its own target directory, zero SBF
stack-frame-overwrite diagnostics on any of the twenty-four links built for the
figures below. Eight runs per ELF set rather than three, because three samples
could not separate a spread of 34,496 from a shift of 18,781.*

### The campaign passes

`accepted` is **31 passed / 0 failed on three consecutive full runs**, and the
action that has been this note's subject since its first paragraph — the
partial equity Remove — commits, together with both LP final Removes behind it,
on **eight consecutive filtered runs of one ELF set**:

| action | headroom over eight draws | worst |
|---|---|---:|
| partial equity Remove | 29,026 / 44,038 / 26,038 / 39,526 / 18,540 / 38,026 / 32,038 / 24,538 | **18,540** |
| first LP final Remove | 36,558 / 35,070 / 21,570 / 44,058 / 5,072 / 30,558 / 38,070 / 32,070 | **5,072** |
| second LP final Remove | 33,570 / 18,558 / 35,070 / 36,564 / 36,558 / 41,058 / 30,570 / 33,572 | **18,558** |

The control, eight runs at `2fbd6adf3`, reached `commit-lifecycle-closes` with
19,692 / 48,180 / 1,672 / — / 21,194 / — / 30,182 / 24,182 against the 85,568
its tail needs, and **two of the eight never reached that checkpoint at all.**

### Three walks over one set of addresses, in one invocation

The evaluator authenticates eleven PDAs. It then hands its result to
`prepare_multi_lp_v3`, **which searches for four of them again**. That third
walk was invisible until the first two were hinted, and it was most of the
remaining draw:

| ELF set | what is hinted | `cx-accelerator-returned` | spread |
|---|---|---|---:|
| control | nothing | 127,669 … 162,165 | 34,496 |
| intermediate | the evaluator's eleven | 111,378 … 144,382 | **33,004** |
| `3c42f0ece` | and the planner's four | 108,888 … 114,888 | **6,000** |

The carriers are three, each the one that fits. Three of the eleven already
persist their own bump (`liability_basis_market_bump_v2`,
`liability_basis_position_bump_v2`, `DealerLpPositionV3::pda_bump`) and needed
only a fixture that records what a founding would have. Eight ride as nibbles in
the four bytes `DealerEquityRequestV3` already reserved at 476..480, which is
`b312ce3c4`'s encoding one family over. The planner's three are a process-local
relay in `MultiLpContextV3`, not a wire field at all.

Claims reads the same three persisted bytes: `signed_delta_v3` searched for the
aggregate and both Positions on every child invocation, over accounts whose
bodies already carried the answer for `sparse_native_transfer_v1`.

| span | control | `3c42f0ece` |
|---|---|---|
| `sd-candidates` | 5,323 … 15,824, spread 10,501 | 5,542 … 5,543, spread **1** |
| `sd-market` | 2,138 flat | 2,241 flat |
| whole measured span sum | 1,327,347 … 1,373,855 | 1,311,710 … 1,343,212 |

### And now the part this note has to say against itself

**The campaign's `ArtifactRelease` records hash the ELFs they name.** A Registry
record lives at `[RAW_RECORD_PDA_SEED_V1, schema, hash(content)]`, and this
fixture's release content carries `hash(elf)` for every program it deploys. So
**rebuilding any program moves several content digests and every
Registry-record search depth with them.** Between `3c42f0ece`'s ELF set and
`40427e0f1`'s, draw-free spans moved by whole `create_program_address`
iterations in both directions with no code between them that could explain it:

| span | before | after | |
|---|---:|---:|---|
| `root-product` | 96,899 | 81,899 | −10 iterations |
| `acc-activation` | 38,235 | 29,235 | −6 |
| `acc-market` | 9,918 | 3,918 | −4 |
| `artifacts-strategy-effect` | 73,308 | 82,308 | **+6** |
| Custody's common frame, per leg, twice | 48,037 | 36,861 | −7 |

That is about **45,000 CU of the distance between "short by 22,000 on the best
of eight draws" and "31 of 31", and none of it is engineering.** Two
consequences:

- **A doubling probe on a walk whose seeds include an ELF digest is not a
  probe.** It rebuilds the program, redraws the addresses, and reports the sum
  of one extra search and an unrelated depth change. This note's own **37,640
  for `execution_strategy_v2`'s record walk was obtained that way**; re-run at
  `3c42f0ece` the same probe reported 78,204 for a span that was 73,308 in
  total, which is impossible and is the proof. Read that row as an order of
  magnitude, and price the walk by HINTING it, not by doubling it.
- **Draw-free is not fixed.** These depths are a property of one deployed
  artifact set. "The Remove commits" is a claim about this ELF set and eight
  payer draws; the next build redraws every one of them.

### Where the draw is now, and what it is made of

Total spread across four runs of `3c42f0ece`'s set: **31,502**, every term a
whole number of iterations.

| term | spread | can it be mined? |
|---|---:|---|
| `cx-accelerator-frame` + `acc-caller-authority` | 6,000 + 6,000 | **no** — the seeds end in a digest over a request projected on chain |
| `cx-claims-frame` + `sd-authority` | 6,000 + 6,000 | the Trading half no; **Claims' own second search for the authority Trading just derived is a three-byte relay** |
| `cx-accelerator-returned` residual | 4,497 | not yet located |
| `cu-common-frame`, twice | 3,000 + 3,000 | inside the activation-cache authentication |
| `cu-transfer-validated`, twice | 1,500 + 1,500 | |
| `pf-invocation-preflighted`, `root-product`, `acc-release-waist` | 3,000 + 1,500 + 1,500 | |

### Custody's common frame, decomposed for the first time

48,037 CU twice per Remove, never split. Four profile-only checkpoints:

| span | CU | what it is |
|---|---:|---|
| **`cf-accounts`** | **23,694** | `authenticate_activation_cache_identity_v1` |
| `cf-market` | 5,031 | premarket selection and the live Market |
| `cf-caller-authority` | 2,080 | the relayed caller-authority derivation |
| `cf-calling-release` | 1,158 | the calling program's release |
| `cu-common-frame` | 4,898 | the replay identity |

**Sixty per cent of it is the release activation cache**, authenticated once per
leg over a cache the caller authenticated a few thousand instructions earlier.
That is the duplicate `0aa70478e` repaired in Claims, one program over, and it
is 47,000 CU per Remove.

### The witness's span bank is deleted

`07184fa82` left it with no reader: all three admitted families require it empty
and the one that used to read it derives its nine widths on the accelerator's
own side. `AdmittedPreludeWitnessV1` no longer carries widths;
`admitted_prelude_witness_bytes_v1` takes one argument; the header word at 16
stays a canonical zero, because moving `ADMITTED_PRELUDE_RECORD_BUMP_OFFSET_V1`
would move every offset after it for a field nothing reads, and the DECODER
refuses a nonzero. The route's `span_count() != 0` refusal and three evaluators'
`span_widths().is_empty()` assertions go with it. The body is byte-identical on
every live route, so this is a wire shape change worth under a hundred CU.

### What this lane owes

- **`execution_strategy_v2`'s own record walk**, still the largest draw-free
  candidate and still without a carrier: `HotBumpHintsV1` is full and its own
  doc records why the envelope cannot grow, `StateBumpsV1` has one byte left,
  `SelectedRecordBumpsV1` fills the capability root's four reserved bytes, and
  the seal is keyed by descriptor. It is also **no longer priced**, per the
  probe finding above.
- **Custody's activation-cache authentication**, 23,694 CU twice per Remove,
  which is the largest single decomposed term in this note that nobody has
  tried to cut.
- **Claims' `sd-authority`**, a three-byte relay in the shape
  `split_caller_authority_bump_v1` already has.
- The three rejoin hostiles and the selector-9 chain fixture, unchanged: still
  blocked on `project_dealer_scenario_unsplit_chain_topology_v4` having no
  caller.
- An assertion that had never run was wrong, and reaching it is what found it:
  `accepted.rs` required the Dealer Claims Position to be byte-identical to the
  planted body after the round trip, when its revision advances once per
  SignedDeltaV3 commit. It now re-encodes the planted Position at the terminal
  revision and compares every byte.

## Ninth addendum, 2026-09-03: the 23,694 was the DECODER, not the identity check; the campaign mined no bump hints at all; and the worst draw is now 74,637

*Measured at `98113142b` (the control, in this lane's own worktree), `5709672aa`
(the decoder repair) and `82465e00b` (the mined hints), tree root
`/Users/ember/dev/dclutch`, real SBF ELFs built in that worktree with its own
target directory, zero SBF stack-frame-overwrite diagnostics on every link
built in this lane, including both frameguard captures' twelve. Eight filtered
runs per column. The second comparison is
the first one in this note taken **within one ELF set**, because its change is
host-side and rebuilds nothing.*

### The eighth addendum named the wrong term, and two checkpoints say so

It reported `cf-accounts` — 23,694 CU twice per partial equity Remove — as
`authenticate_activation_cache_identity_v1`, and called it "the largest single
decomposed term in this note that nobody has tried to cut". Splitting it:

| span | CU | what it holds |
|---|---:|---|
| `cf-cache-decode` | **21,984** | `require_cache_account`, the borrow, and `ActivatedExecutionReleaseSetViewV1::decode` |
| `cf-cache-identity` | 3,366 | the identity conjunction itself |
| `cf-accounts` | 258 | three account lookups and one key equality |

The identity check is 3,366 and was never the cost. **Ninety-three per cent of
that span is one call to `decode`.**

### And the ruling had already been spent, so the enumeration comes out empty

`9b5de611e` gave this route the Claims shape from `0aa70478e` exactly: ONE
borrow and ONE decode per invocation, every role read out of that view. There
was no second re-authentication of the cache left to drop. Stated in full, since
a unit sent to apply a ruling owes the enumeration even when the answer is none:

* the seeds pin the release set, the Market, the caller's execution role, the
  replay context and `hash(request_bytes)`, with the SIGNER bit required at
  coordinate 0 by `require_account_count`'s privilege scan and the address
  reproduced in `authenticate_common_frame_tail`;
* **which program** holds `caller_role` in that release set is not among them —
  the seeds name a role, not a key;
* **the cache's own coordinate** is not among them — the seeds name a release
  set, the ACCOUNT comes from the transaction, and 3,366 CU is the whole of what
  binds the two;
* **the cache's completeness** is not among them. A Registry-owned cache
  legitimately holds a strict subset of its five roles between activation
  transactions (`ActivationCacheProgressV1`), and it is the full five-role
  decode that makes a partial one inert for every reader. A signature over a
  release-set id cannot carry that.

So nothing came out under the ruling, and what was left was not a ruling
question at all.

### Twenty-five `decode_role` calls for five values

`ActivatedExecutionReleaseSetViewV1::validate_projection` ran
`release_set_projection` — five decodes — then a ten-pair aliasing scan that
decoded BOTH SIDES INSIDE THE LOOP: twenty more, each re-running
`ArtifactReleaseIdV1::decode` and `ArtifactReleaseV1::decode` over bytes an
earlier iteration had already accepted. This is the sole hostile decoder for the
activation cache and it is reached about six times per top-level Direct
transaction.

It now decodes each role once, keeps the five 64-byte projection bindings, and
tests the pairs over those. The pair test is `binding equal ⟹ the whole
ActivatedRoleV1 equal`, and a binding is decided by the two identities the loop
already holds — so a pair whose bindings differ could never raise
`AliasedRoleActivationMismatch`, and decoding it to find that out was dead work.
All five roles are still decoded before the first pair is examined, and a
re-decode of bytes that already decoded cannot fail, so no refusal moves.

`cf-cache-decode` contains no `create_program_address` at all — owner, width,
borrow and byte validation only — so unlike almost every figure in this note it
is comparable ACROSS ELF sets:

| span | control | `5709672aa` | |
|---|---:|---:|---|
| `cf-cache-decode` | 21,984 | **8,464** | one activation-cache decode |
| `sd-releases` (Claims) | 26,334 | 14,313 | its decode, identity and three roles |
| `acc-activation` (Trading) | 29,235 | 17,214 | its decode and identity |

12,021 in Claims and 12,021 in Trading, to the digit, from the same call; twice
per Remove in Custody. About 51,000 CU across the transaction.

**The borrowed view's aliasing scan had no test.**
`aliased_roles_must_share_one_complete_activation` has always pinned the OWNED
decoder; nothing named the view, which is the decoder every adapter runs.
`the_borrowed_view_refuses_an_aliased_role_that_disagrees_in_one_byte` now does,
with both arms — an aliased pair agreeing in every field is a LEGAL cache and
must decode; one flipped deployment-slot byte must refuse — and it was proved
red before it was trusted green.

### The campaign was the last producer in the tree that mined no bump hints

`HotBumpHintsV1` has been read by Trading, the Dealer accelerator and Custody
since it was added, and `dclutch-operator`'s producers fill it — `direct_inline_v3`
four slots, `dealer_lp_hot_v4` three. **`dclutch-chain-bundle-builder`, which is
what every figure in this note was measured over, filled none.** Every packet the
campaign has ever emitted carried the all-zero block and every reader walked down
from 255. That is the pre-hint route, not a neutral default — the same defect
`3c42f0ece` repaired one account over when the campaign left
`liability_basis_market_bump_v2` zero.

Three slots now come out of the fixed corpus the builder already binds:
`market`, `root`, and `child_relay[1]` (Custody's transfer authority, whose
seeds are the Market and the release set — one slot correct for both legs).
`child_relay[0]` needs the child projection this builder does not do;
`child_caller` cannot be mined off chain at all; `lifecycle` is not projected
here. A zero slot is correct and merely slower.

### Where the Remove is now

| action | worst headroom, `98113142b` | `5709672aa` | `82465e00b` |
|---|---:|---:|---:|
| partial equity Remove | 20,024 | 64,175 | **74,637** |
| first LP final Remove | 3,562, and **one overrun** | 43,209 | **76,165** |
| second LP final Remove | 14,072 | 61,217 | **74,647** |

`accepted` is 31 passed / 0 failed on three consecutive full runs at each of the
two later columns.

**The control is not eight of eight, and that is this note's caveat about itself
coming true.** On the ELF set built at `98113142b` in this lane's worktree, run
3 of 8 died at `exceeded CUs meter` on the first LP final Remove, with the
partial Remove ahead of it finishing on **8 CU** of headroom. The eighth
addendum said "the Remove commits" was a claim about one artifact set and eight
payer draws; the next build redrew every depth, and this is what that looks like.

### What remains draw, priced

Per-span spread over the eight runs at `82465e00b`, which is where the worst case
now comes from:

| term | spread | can it be mined? |
|---|---:|---|
| `cx-accelerator-frame` + `acc-caller-authority` | 7,500 + 7,500 | Trading's half yes, from the bump `derive_admitted_authorities_v1` already computes and discards — but `HotBumpHintsV1` has no slot for the accelerator's own caller authority, and its slots are ROLES, not free space. The accelerator's half is a process-local relay Trading could carry, and the witness's reserved word at offset 16 is now unowned. |
| `cx-accelerator-returned` | 9,000 | not located |
| `cu-transfer-validated`, twice | 6,000 + 6,000 | `validate_vault_key` searches for the source and destination vaults. **No carrier**: `CustodyBumpRelayV1` is three bytes and all three are spoken for, and a token account cannot carry its own bump. |
| `cu-common-frame`, twice | 4,500 + 4,500 | the replay cursor — needs the child projection the campaign builder does not do |
| `pf-invocation-preflighted` | 6,000 + 3,000 | the child caller authorities, unminable for the same reason |
| `cx-claims-frame` + `sd-authority` | 3,000 + 3,000 | Claims' half is the three-byte relay `split_caller_authority_bump_v1` already has one program over |

Filling the hint block **redrew** `cx-accelerator-frame` and
`acc-caller-authority` — the block is inside the envelope, which is inside the
digest those seeds end in — so their spread went from 1,500 to 7,500 on this
draw and will move again on the next. That is the same unminable lottery at a new
seed, which is why the table above is worst-of-eight rather than a difference.

### The chain fixture seam is not "no caller"; it is that no submittable admitted selector-9 instruction exists

Three addenda have recorded the three rejoin hostiles as blocked on
`project_dealer_scenario_unsplit_chain_topology_v4` having no caller, without
naming what a caller would buy. Read against the code, the block is structural
and one level further down:

1. `split_scenario_from_admitted_trade` already computes every input that
   constructor takes — the three observation slices, the semantic state and the
   family request — and hands them to the OPERATOR's
   `project_dealer_scenario_unsplit_topology_v4` instead. A caller is one line
   away. What the program-test wrapper adds is the installable-account list and
   the rollback classification, i.e. the ability to seed a fresh ProgramTest.
2. But its own `unique_account_lock_count` is asserted, in that same function,
   to **exceed `SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1`**. The unsplit admitted
   instruction is 121 locks against a 64-lock ceiling, so whatever is installed,
   the topology is evidence and never a transaction.
3. And the split route the scenario family takes never enters the accelerator
   ELF. `split_scenario_from_admitted_trade` builds the admitted bundle IN
   PROCESS, installs it, and submits a Custody delivery through
   `submit_activation`; nothing on that path CPIs
   `dclutch-dealer-accelerator-sbf`. Checked in the profile rather than assumed:
   `acc-enter` appears in eight of this test's twenty-two ≥1M-budget
   transactions and all eight are top-level Trading Hot transactions, while the
   delivery transactions carry an address-lookup table, a different top-level
   program, and no accelerator span at all.
4. The hostiles sit behind `acc-witness` and `acc-seal`, which come AFTER
   `acc-product-runtime` in `authenticate_accelerator_invocation_v4` — and
   `frontier.rs`, the only stage-attributing instrument, stops in
   `authenticate_product_runtime_v3` for want of the four finalized Registry
   records.

So the unit is not "call the constructor". It is either (a) the durable
preparation/commit split for the scenario family, so an admitted selector-9 leg
can be a transaction at all, or (b) the Product record graph staged into
`frontier.rs`, so the in-process probe reaches the witness. (b) is much the
smaller, and `accepted.rs` already builds those four record bodies.

### What this lane owes

- **`execution_strategy_v2`'s record walk**, 82,308 CU and now flat to 6 across
  eight runs — still the largest single draw-free span on the route, still
  without a carrier, and still not priced by doubling.
- **The accelerator's caller authority**, 15,000 of draw across the two halves,
  with the carrier for each half named in the table above.
- **Custody's two vault derivations**, 12,000 of draw, with no carrier at all —
  which makes it a design question (where does a token vault's canonical bump
  live?) rather than a lane-sized fix.
- **Claims' `sd-authority`** three-byte relay, unchanged; its blocker is that
  `packet_digest` is taken over the WHOLE instruction data, so a suffix byte has
  no fixed point until that digest is narrowed to the prefix the way Custody's
  `the_caller_authority_digest_covers_the_request_prefix_only` pins.
- **The three rejoin hostiles and the selector-9 leg**, with the blocker restated
  above as a structural fact rather than a missing caller.
