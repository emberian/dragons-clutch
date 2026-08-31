# The fee-bearing Direct trade, executed — 2026-08-30

**The measurement:** a Direct Hot trade whose fee does not floor to zero costs
**1,515,003 CU with every search landing on its first candidate**, and is over
the 1,400,000 protocol ceiling **by 115,003 CU before a single participant key
is drawn** (§5, "The figure the decision turns on"). The cheapest draw actually
observed was higher — seed 1 at a lower bound of 1,521,004, over by 121,004
(§5's table) — so the all-first-try figure is the honest floor and the one
every downstream document quotes. It is not a near miss and it is not a
lottery. Thirty-two of thirty-two seeds refuse, every one of them at the
compute meter, with the second Custody CPI reached on all thirty-two.

The zero-fee route, measured in the same run on the same five ELFs with the same
keys and the same substrate, executes on 32 of 32 and lands between 1,329,618
and 1,349,118.

Every figure here is from the landed tree. The measurement was taken twice, on
two ELF sets four commits of `main` apart; §5 reports what survived that, and
the fee leg it implies reproduced to the compute unit.

The instrument is
`programs/dclutch-trading-sbf/program-test/tests/direct_hot_fee_bearing_margin_gate.rs`.
Both of its tests pass; passing means the fee-bearing route fails to fit and
says so with numbers.

## 0. Why this had never been measured

`DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md` finding 3, and `30574297`'s
closing note, both name it: the fixture trades `FILL = 10` at `EXECUTION_PRICE =
50` against a `PRICE_SCALE` of 100, so `gross = 5`, and the market's 50 basis
points of that is `5 * 50 / 10_000` — which floors to **zero**. A zero combined
fee sets the `seller_terminal` enable register and clears both fee registers, so
of the four Custody routes the Direct Effect declares, exactly one is projected
live. The route makes one Custody CPI. The fee leg — the second Custody route,
its own caller authority, its own replay revision step, its own delegated
transfer — had never executed anywhere in this tree, and every compute figure
this project has ever quoted for the top-level Direct route describes the
fee-free trade.

The census could only estimate the other shape: "a fee-bearing trade is around
1.49-1.52M CU and DOES NOT FIT. That is arithmetic on a measured single route,
not a measurement of the two-route shape, and it wants its own lane."

That estimate was right and slightly conservative. The measured lower bound is
1.52–1.53M.

## 1. The wall the scenario had to get past first, which is not compute

The obvious way to buy a nonzero fee is a bigger price. The protocol forbids it.

`crates/dclutch-direct-aot-v3-contract/src/lib.rs:163` requires
`execution_price <= price_scale`, and :167 computes `gross` with **`mul_div_exact`**
rather than a floor. The price is a *fraction of scale* — which is what a
prediction-market claim's price is — so `gross` can never exceed the fill, and
`fill * price` must divide the scale exactly. At 50 basis points a nonzero fee
needs `gross >= 200`, so it needs **`fill >= 200`**, and no price anywhere in the
admissible range gets there on a fill of 10.

The first fee-bearing scenario this lane wrote raised the price to 2,000 and was
refused by the host transition VM at instruction 31 — `OP_SCALAR_LE` on registers
23 and 25, `SCALAR_EXECUTION_PRICE_V3 <= SCALAR_PRICE_SCALE_V3`. That refusal is
worth recording on its own, because it says the zero-fee measurement was not an
unlucky constant:

> **The historical fixture's fee floored because its trade was two orders of
> magnitude too small for its own market's fee rate to bite.** Not because 50 bps
> is small, and not because the fixture picked an odd price.

The fixture's Claims supply was the other half of the same wall: the seller held
100 claims of the traded outcome, of a market supply of 100, so `fill <= 100` and
`gross <= 100` — a fee of zero for any price the protocol admits.

`DirectTradeScenarioV1::FEE_BEARING` therefore keeps the price at 50 of 100 —
the same half the zero-fee scenario trades at — and moves the size: supply 1,000,
fill 400, `gross = 400 * 50 / 100 = 200`, fee `200 * 50 / 10_000 = 1` per side,
seller net 199, combined fee 2, buyer debit 201. That is the smallest admissible
fee-bearing trade at this market's rate, and smallest is right: the enable
registers are booleans, so a larger trade buys nothing but a bigger number.

## 2. The controlled pair

The trade scenario is a HOST-side fixture input. It changes no ELF, so the two
arms are the same programs, the same keys, the same seeds and the same substrate,
differing in what the transaction does and in nothing else. That is the same
property `30574297` used to A/B the CoreState carry, and it is why both arms run
inside one test rather than one arm being compared against a figure from another
tree — `release_set_id` hashes the five role ELF digests, so a CU number does not
travel between builds.

Two independent checks say the fee-bearing fixture is the real shape and not a
plausible-looking one:

* **The host transition engine reproduces it.** `via_builder` runs the real
  artifact bytecode through `dclutch-transition-vm` and derives the caller
  authorities itself; `builder_reproduces_the_hand_built_fee_bearing_fixture`
  asserts the builder's accounts, routes, instruction and signed messages are the
  hand fixture's, byte for byte. A hand fixture that had guessed at the fee leg
  would not survive that.
* **The chain reports the shape.** The Custody invocation count comes out of the
  program log, not out of the fixture's arithmetic: **1 on every zero-fee seed
  and 2 on every fee-bearing seed**, asserted per seed.

## 3. The sweep, 32 seeds per arm

| | zero-fee | fee-bearing |
|---|---|---|
| executed | 32 / 32 | **0 / 32** |
| best | 1,329,618 | — |
| worst | 1,349,118 | — |
| mean | 1,337,539 | — |
| band | 19,500 | — |
| worst margin under 1,400,000 | 50,882 | — |
| key-independent floor | 1,319,115 | — |
| Custody CPIs | 1 | 2 |
| refusal | none | 32, **every one at the compute meter** |

Every fee-bearing refusal carries `exceeded CUs meter at BPF instruction` in the
program log; the transaction-level error is `ProgramFailedToComplete` on most
seeds and `ComputationalBudgetExceeded` on a few, which is the same event
reported at two depths. On every one of the thirty-two, the log shows Trading
consuming 1,399,692 of 1,399,700 — the whole budget — having already invoked
Custody twice.

**It is a compute wall and not a heap wall, not an account wall and not an
arithmetic wall.** That distinction is the reason the refusal code is asserted
rather than assumed.

## 4. The number, and how it was obtained past a ceiling the route cannot meet

A route that dies at the meter reports the meter. Two ways of getting past that
do not work, recorded so the next lane does not spend the afternoon:

* **Ask for a bigger budget.** 1,400,000 is the runtime's maximum. A
  `SetComputeUnitLimit` above it is clamped; there is nothing to request.
* **Lift the meter under the harness** with `ProgramTest::set_compute_max_units`.
  It replaces the whole `ComputeBudget`, which resets the heap to the protocol
  default of 32 KiB. Submitting *with* the heap-frame instruction then lets
  `admit_heap_frame_v1` read the 64 KiB grant out of the instructions sysvar and
  lift the allocator's ceiling past what the runtime mapped — measured, every
  seed dies at `Access violation writing 8 bytes at address 0x30000fa58`, which
  is heap offset 64,088. Submitting *without* it refuses as
  `TradingSbfError::HeapFrame` (0x4008) at 47,835 CU, because this route declares
  an extended heap profile and refuses rather than allocating until it dies.
  There is no third door: the route needs 64 KiB and a lifted meter cannot grant
  it.

So the cost is assembled from the program log, which reports the compute meter at
every invocation boundary whether or not the transaction completes.
`Program <id> consumed X of Y` gives both an invocation's cost and the meter
remaining when it started, so consumption at any child boundary is exactly
`budget - Y`. Per seed:

```
  A   everything before the first Custody CPI      = budget - Y(first Custody)
  C   the Custody CPIs and the Trading work around
      them, to the last Custody CPI's return       measured
  P   everything after the last Custody CPI        measured on the ZERO-FEE arm
```

The zero-fee arm runs to completion, so `A + C + P` reconstructs it exactly —
and the test asserts the reconstruction, because a decomposition that does not
add up is not a measurement. The remainder outside Trading (two ComputeBudget
instructions and the Ed25519 precompile) is **308 CU on every seed**, a constant,
which is itself the check that nothing outside Trading is key-dependent.

`P` is **132,027 CU on all eight draws, to the compute unit** — no bump search
happens after the last child returns, so the tail is a constant and not a
distribution.

The fee-bearing arm reaches its second Custody CPI's return and dies after it, so
`A` and `C` are measured there too. The reported figure is `A + C + P(zero-fee) +
308`, and it is a **lower bound**: the fee route's commit phase writes one more
child's poststate than the zero-fee route's, so its real `P` is at least
`P(zero-fee)`.

| seed | before | Custody span | reached | second leg | lower bound | over the ceiling by ≥ |
|---|---|---|---|---|---|---|
| 0 | 1,103,098 | 290,070 | 1,393,168 | returned | 1,525,503 | 125,503 |
| 1 | 1,098,599 | 290,070 | 1,388,669 | returned | 1,521,004 | 121,004 |
| 2 | 1,107,599 | 290,070 | 1,397,669 | returned | 1,530,004 | 130,004 |
| 3 | 1,104,598 | 290,070 | 1,394,668 | returned | 1,527,003 | 127,003 |
| 4 | 1,106,098 | 287,070 | 1,393,168 | returned | 1,525,503 | 125,503 |
| 5 | 1,107,598 | 284,070 | 1,391,668 | returned | 1,524,003 | 124,003 |
| 6 | 1,113,599 | 286,093 | 1,399,692 | **died at the meter** | 1,532,027 | 132,027 |
| 7 | 1,107,599 | 292,093 | 1,399,692 | **died at the meter** | 1,532,027 | 132,027 |

The zero-fee arm on the same eight draws: before 1,070,884–1,081,384, Custody
span 126,399–130,899, tail 132,027, totals 1,332,618–1,344,618.

**The two truncated rows do not enter the floor below.** A seed whose Custody leg
died at the meter has a `reached` that *is* the meter — capped at the budget,
while its modelled attempts are subtracted in full — so it produces an
artificially low residual and silently captures the minimum. The first draft of
this measurement did exactly that, and §5 is the story of catching it.

## 5. The two numbers a decision needs, and the test that they are real

**The key-independent lower bound is 1,501,503 CU**, taken over the six of eight
draws whose second Custody leg returned. It is the same residual statistic the
margin gate's floor is — `total - 1,500 * modelled attempts` — so it is a
property of the code and not of a key draw.

That is a claim about cross-build comparability, and this measurement got to
test it rather than assert it. It was taken twice: once on ELFs from
`391a65ff`, and again after rebasing onto four commits of `main` that changed
role source and therefore redrew every bump depth on the route.

| | before the rebase | after | moved |
|---|---|---|---|
| zero-fee floor (8 draws) | 1,318,908 | 1,319,117 | **+209** |
| fee-bearing floor (completed seeds) | 1,501,294 | 1,501,503 | **+209** |
| **implied fee leg** | **182,386** | **182,386** | **0** |

Both shapes paid `main` the same 209 CU, and the fee leg reproduced **to the
compute unit** across two different ELF sets. That is what a key-independent
statistic is supposed to do, and it is the strongest evidence here that the
number is about the code.

**It also caught a defect in this instrument.** Computed over *all* eight draws
— including the two whose Custody leg died at the meter — the same floor read
1,493,027 and 1,497,527, appearing to move **4,500 CU** on a rebase that cost the
route 209. The 4,500 was entirely an artifact of which truncated seed happened to
be the minimum. The floor now excludes them, and the test says so where it does
it.

### The figure the decision turns on

Implied `C0` = 1,500,003. The fee-bearing shape searches at **eight distinct
addresses over ten search instances** (the two Custody-side sites are searched
once per Custody invocation at one drawn depth each; the two caller authorities
are different addresses because their seeds carry each route's own child-request
digest). So a fee-bearing trade whose every search landed on its first candidate
would cost:

> **1,515,003 CU — over the 1,400,000 ceiling by 115,003, before any key is
> drawn.**

No key draw makes it fit, no gate constant makes it fit, and the tail probability
the zero-fee route is accepted on has no meaning here: the fee-bearing route's
*floor* is over the roof.

**The fee leg's key-independent cost is at least 182,386 CU.** It is a bound
rather than a measurement of the whole leg, because the fee arm's commit phase
never ran.

## 6. The recommendation, with the arithmetic that decides it

The ruling asked for a choice between (a) CU work on the fee leg and (b) the
two-transaction lifecycle. **The arithmetic decides it before the engineering
judgement gets a turn.**

To make the fee-bearing route fit *on the luckiest possible key*, it must lose
**115,003 CU**. To give it the same safety the zero-fee route is accepted on —
71,883 CU of headroom above an all-first-try route, which is `P(over) = 1.6e-9`,
about one public trade in 614 million — it must lose **186,886 CU**.

**The entire fee leg is 182,386 CU.** So (a) is not "optimise the fee leg". (a)
is "make the second Custody route cost nothing, and then find four and a half
thousand more somewhere else".

### (a) CU work on the fee leg — sized, and it does not reach

The one real lever is collapsing the two transfers into a **single Custody CPI
carrying both legs**, so the second invocation's fixed cost — realm
authentication, replay PDA derivation, transfer authority derivation, request
decode, frame shape checks, the `invoke` itself — is paid once instead of twice.
Measured, that fixed cost is the difference between the arms' Custody spans:
**157,671 CU at the cheapest draw** (284,070 − 126,399), of which the extra token
transfer itself is 112 CU.

Suppose it lands a 150,000 CU saving — near the top of what the measurement
allows. The route would then sit at an all-first-try 1,360,503 with 39,497 CU of
headroom over seven searched addresses: `P(a stranger's key exceeds 1,400,000) =
1.6e-4`, **about one public trade in 6,200**. That is five orders of magnitude
worse than the route the project accepted, and it spends the entire structural
saving with nothing left for the next feature. Only a saving equal to the WHOLE
fee leg — 182,386 CU, every compute unit of it — returns the route to the safety
the zero-fee shape already has.

Size of the work, since a recommendation without one is an opinion: the request
shape in `dclutch-custody-contract`, the handler and a new refusal class in
`dclutch-custody-sbf`, the four declared routes in the Direct Effect artifact,
the enable-register arithmetic in the transition (which is Lean-generated —
`formal/dclutch-semantics/EmitDirectOrdinaryV3Rust.lean` and
`crates/dclutch-direct-codec/src/generated_ordinary_v3.rs`), the AoT mirror in
`dclutch-direct-aot-v3-contract`, the fixture, and the ABI mirrors. Roughly eight
files across six crates plus a Lean regeneration and a semantics review:
**three to four lanes, two to three days of swarm time**, for an outcome that is
still a refusal every eighty thousand public trades.

### (b) The two-transaction lifecycle — the only option that reaches

ember has pre-approved multi-transaction (WAVE.md Rulings), and the variance
census priced the lever at **496,410 CU, 36.1% of the transaction's mass**. It is
the only structure that clears the gap with room left over rather than consuming
it.

The smallest version that solves *this* problem specifically — and it is smaller
than the full lifecycle split — is to move the **fee leg** into a second
transaction. One observation in its favour, from this lane's own fixture work:
the two routes are already sequenced through the Custody replay account rather
than through transaction atomicity. `custody_registers` derives
`after_seller = CUSTODY_REVISION + 1` and `after_fee = CUSTODY_REVISION + 2`, so
the fee route already expects to find the replay at the revision the seller route
left it at. The ordering the two-transaction form needs is a property the accounts
already carry.

Sized: a fee-settlement route in Trading and the Effect, the transition
projecting across two requests, and the fixture and gate to cover it — **two to
three lanes**. What it is *not* is only engineering: a trade that lands with its
fee unsettled until a second transaction is a protocol-semantics change, and
whether that is acceptable is ember's decision and not a lane's.

### What holds until one of them ships

**The founding parameter stands, and it is now measured rather than inferred.**
`227387da` recorded it as a judgement from arithmetic: market19 must be founded
with zero fees, because a fee-bearing trade would not fit and founding cannot be
corrected afterwards. This document replaces the arithmetic with an executed
measurement, and the measurement is worse than the estimate. Rate diversity on
the demo (ADR 0014 D3) stays blocked behind (a) or (b).

## 7. What this does NOT establish

* **The fee-bearing route's true total.** Every fee-bearing figure here is a
  LOWER bound. The commit phase never ran, so the tail added to it is the
  zero-fee tail — which writes one child's poststate where the fee route writes
  two. The true cost is higher by whatever that difference is, and nothing here
  measures it.
* **Whether the fee leg's Custody request would validate in a later
  transaction.** §6(b)'s observation is about the replay revision arithmetic only.
  The caller authority's seeds carry the parent request digest, and this lane did
  not check whether Custody would accept the fee route's request outside the
  transaction that produced that parent. That is the first thing (b) has to
  establish and it is an hour's work, not a design.
* **The `FeeSole` route.** `CUSTODY_ROUTES_V3` slot 3 — a fee with no seller
  leg — is still unexecuted. It is one Custody CPI, so it is the cheap shape, but
  nothing here ran it.
* **Any rate other than 50 bps.** The scenario moves the trade SIZE, not the
  market's fee rate. A market founded at a different rate has the same route
  shape and therefore the same cost, but that is an argument, not a measurement.
* **Anything about the continuation route.** Everything here is the top-level
  route a public caller sends.
* **A third build, or any build but these two.** The measurement was taken on
  two ELF sets (§5), both with zero SBF stack-frame-overwrite diagnostics on all
  eight links. Two agreeing builds is what supports the cross-build claim in §5
  and nothing wider.

## 8. Reproduction

```
# five role ELFs plus the three test programs, from this tree
for m in programs/dclutch-{trading,registry,core,claims,custody}-sbf/Cargo.toml \
         programs/dclutch-trading-sbf/program-test/test-programs/{trading-outer,core-caller,registry}/Cargo.toml
do cargo build-sbf --manifest-path "$m" --sbf-out-dir "$ELF"; done

SBF_OUT_DIR="$ELF" cargo test \
  --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
  --test direct_hot_fee_bearing_margin_gate -- --nocapture
```

About seventy seconds for both tests. The tagged lines are `SHAPE` (Custody
invocation count per seed), `SEEDCU`, `SEEDREFUSED` with its log tail, `ARM`,
`FLOOR`, `HEADROOM`, `PARTS` (the decomposition), `TAILWORK`, `BOUND` and `FEELEG`.
