# Decision 0032: the joint clearing's three owed rulings — the residual strands, the tie-break minimises the vector, the collecting batch is sealed

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible
on request) — the three rulings the JOINT-CLEARING note owed ember, made by the
orchestrator on 2026-09-04 under ember's standing goal, none of them changing a
theorem in `JointClearingV1.lean`, and reversible at the cost §7 states**. It
was PROVISIONAL from the ruling until 15:50 EDT, when ember read the docket and
accepted it in conversation without amending it; the confirmation line below is
the whole of what was said. The note states the three questions at
`docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md:467-475`; the design and
its 44 sorry-free theorems landed at `554a29119`. This record rules them; it
does not rule the batch spine's commitment (decision 0031 §6), which stays
ember's.

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

The joint-clearing rule is a **certificate**: eight `O(N·K)` integer KKT
conjuncts the chain verifies, with the solver off chain. The Lean proves that a
passing certificate is an optimal clearing. Three things the certificate cannot
decide for itself were written down as owed, because each is a choice about the
product rather than about the mathematics, and the note was explicit that *"the
certificate and every theorem are identical under both"* for the first and that
the second *"changes no theorem"*:

1. **Where a residual goes.** Complementary slackness allows `net_i < M` only
   where `p_i = 0`. On such an outcome the batch mints `M` claims and hands out
   `net_i`; the difference is a residual whose value at the batch's own prices is
   zero, coordinate by coordinate (`residual_worth_nothing`,
   `JointClearingV1.lean:380-386`). Refusing residuals outright — the shipping
   `claimsBalance` — caps the mint at the thin outcome's demand and rations the
   deep side strictly inside its limits: `Examples.thinRationed` is worth 160
   against 320. **The LP is right and the residual is the price of it**
   (`JOINT_CLEARING:112-150`).
2. **Which optimal price vector.** The LP's optimal face can be a segment
   (`jointMint` vs `jointMintSkewed`): every price inside both full-filled
   orders' limits certifies, so the rule must choose one or the clearing is a
   function of the solver rather than of the book (`:170-190`).
3. **Sealed or visible.** Orders are on-chain records placed during the window
   and cancellable while collecting, so *"being first buys nothing; being last
   lets a trader condition on the visible book. The batch is therefore not
   sealed-bid"* (`:277-288`).

## 2. The rulings

### 2a. A residual on a zero-priced outcome is STRANDED, and the beneficiary row is REFUSED

The note priced three dispositions and recommended the beneficiary row: the
residual distributed to the configured `quote_surplus_beneficiary`
(`crates/dclutch-general-config-contract/src/lib.rs:101, 365, 407`) as one more
`Distribute` row, exactly as the rounding remainder is routed at `Close`.

**That is refused, and decision 0024 is why.** 0024 item 2 rules **no protocol
beneficiary**: *"there is no treasury, no protocol sweep instruction, and gen-1's
`REVENUE-TREASURY-UNSET-SENTINEL1` is not carried into this generation."* The
`quote_surplus_beneficiary` the note points at is a per-market configured key,
which is not the protocol's — but making it the standing owner of *minted claims*
is a different thing from routing it a rounding remainder in the quote asset. It
gives a founder-chosen key a position in the market's own outcome space, on every
batch that prices an outcome at zero, for free. The note itself concedes the
distribution is worth nothing and that *"any participant outbids it for free"*;
what it does not price is that the beneficiary is the only party who never has to.

**So the residual strands**: the claims are burned without releasing collateral,
leaving `supply_i < M` with `hoard = M = max_i supply_i`. **L4 holds with zero
excess** — the Hoard is not short — and no claim is created that nobody paid for.

This is the disposition the note *refused on cost*, and the ruling pays that cost
knowingly. §6 names it.

**What is NOT ruled: refusing residuals.** That is the shipping rule, it loses
the LP, and keeping it would make the whole design pointless. The batch mints.

### 2b. The tie-break MINIMISES THE PRICE VECTOR

Applied by the selection policy among **certified candidates only**, in this
order:

1. minimise rounding surplus — the existing `minimizeQuoteSurplus`;
2. then the **lexicographically minimal price vector**;
3. then candidate id.

Lexicographic minimum is content-derived, cheap to compare on the existing
selection cursor, and it makes the clearing **a function of the book rather than
of the solver** — which is the property that makes solver neutrality
(`JOINT_CLEARING:290-292`) true rather than hoped for. Rationing among orders
exactly marginal at the price stays pro-rata by lots with the remainder assigned
in increasing order-id order, which the candidate's rows are already required to
be in (`NonCanonicalOrder`, `runtime_verify.rs:191-192`).

This ruling changes no theorem and adds no conjunct the verifier does not already
have; it is a selection-policy criterion.

### 2c. A collecting batch is SEALED, and the clearing is PUBLISHED

**The shape the mechanism commits to** is: while a batch collects, the book is
not readable; when it clears, the price vector, the fills and the residual are a
published chain fact. The information advantage the note names — *"being last
lets a trader condition on the visible book"* — is a **defect to be closed, not a
feature of the venue**; and the clearing is never sealed, because *the price
series is the forecast* and a forecast nobody can read is not a product
(`BATCH_SPINE`, the agenda's direction 1).

**What this ruling does NOT settle, and the record says so plainly: the
transport.** The only transport the note names for sealed orders is the FHE/MPC
target, and **decision 0018 rules that horizon out of THIS Clutch** — ember's
own ruling, dated, with the prerequisite named, and terminal rather than
deferred. `GOAL.md:2093` forbids reporting the privacy horizon as deferred,
future work or in-progress, and this record does not do so: **cohort-17 ships the
visible book.**

So the operative content of this ruling *in this Clutch* is `O-019` made concrete
for the clearing rule, and it is a real obligation rather than a decoration:

- **Sealed orders are a transport change, not a rule change** — the note's own
  words, and *"every theorem here survives it"* (`:284-286`). This ruling makes
  that a requirement on the design rather than an observation about it.
- **The batch relation must stay narrow enough that it remains true.** That is
  `O-019` (`docs/OMISSION_INDEX.md:59`), which decision 0018 made load-bearing
  precisely so a later Clutch is still reachable: *"narrowing further is free;
  widening spends an option nothing else in the tree records."* Any conjunct
  added to the clearing rule that reads a *published* order in a way a committed
  one could not satisfy widens the relation and is refused by this ruling.
- **A transport that seals the collecting book without the FHE horizon is an
  owed design, not a build item**, and nothing in this record charters one.

## 3. Ember's standing authority, and the boundary on ruling 2c

There is no amendment: these are the three rulings the note owed ember and the
orchestrator made them under the standing goal, as it made decisions 0031, 0033
and 0034 the same day. Ember's words authorising the agenda are quoted in 0031
§3.

Ruling 2c is bounded by ember's own decision 0018 and stays inside it. If any
future reader takes 2c as licence to reopen the FHE/MPC horizon, that reading is
wrong and 0018 §2 is the answer: the condition for revisiting is *"a much later
version of Clutch"* on a substrate that can carry it.

## 4. The lanes

None is chartered by this record. JOINT-CLEARING closed at `554a29119`
(`GOAL.md:4715-4721`), and **cohort-17** is where the order record, the verifier
conjuncts and the selection policy's two new criteria land together with their
Lean emission (`JOINT_CLEARING:456-466`).

**Ruling 2a needs an owner outside the General family**, and it is the one thing
in this record that is not a General change: a partial burn is a new kernel
command, so the owner is whoever carries the next `EconomicKernel` change — the
same seam ESCROW-2 is working in for decision 0025's refunding merge.

## 5. The hostiles and laws that guard them

**The four new refusal codes the certificate needs**, all `RuntimeVerifyErrorV2`
variants in the adapter contract, reaching the log through the accelerator's
`log_line()` and the chain through Trading's family refusal, in the bands those
already own (`JOINT_CLEARING:198-212`):

| hostile | conjunct | code |
| --- | --- | --- |
| an order that would mint an unbacked set (`Examples.unbackedMint`) | slackness | `PricedResidual` |
| a fill below a seller's floor (`Examples.belowFloor`) | at-or-better | `CreditLimit`, plus the record field that carries the floor |
| an order rationed strictly inside its limit (`Examples.thinRationed`) | marginal | `RationedInsideLimit` |
| a certificate that omits an order (`Examples.closedBatch.clear? soloBuy`) | complete | `OrderOmitted`, at the terminal row |

Each must name its exact discriminant and be **proved red before green**
(`AGENTS.md`, Refusal codes); a bare `is_err()` on any of them is a test of
nothing.

**The new standing invariant is complementary slackness itself** — *residual only
at zero price* (`residual_nonneg`, `residual_worth_nothing`) — asserted per batch
as the batch-local form of L4.

**Ruling 2a moves two census statements, and this is the part a reader must not
skip.** The note's §2(c) reads *"L3 holds because every distributed claim is a
Position row and every residual is the beneficiary's"* — under the strand there
is no beneficiary and no Position row, so **L3's statement changes**: every
distributed claim is a Position row, and the residual is *burned*, with the
Hoard's collateral for it neither released nor re-attributed. And **L4's
uniformity changes**: a strand is the first **pre-terminal non-uniform supply
vector** in the tree, so every family's complete-set law
(`EconomicKernel.lean:67-70`) gains an exception.

That exception lands in the same module ESCROW-2 has just been working in, which
is a benefit rather than a coincidence: `uniformSupply` was moved beside the
complete-set law at `e37116b03` precisely because *"uniformity is a property of
the actions that move supply, not of the payout arithmetic that reads it."* A
partial burn is one more such action, and it must be stated there or the two
statements will disagree.

**Ruling 2c's guard is `O-019`** and the OMISSION_INDEX row that keeps it open:
widening the batch relation closes a door that a dated ruling from ember
deliberately left open, and closing that row requires a new dated ruling from
ember that no refactor may substitute for.

## 6. What was given up, named

**2a buys a new kernel command.** The note refused stranding on cost and the cost
is real: a partial burn, the first pre-terminal non-uniform supply vector, and an
exception in every family's complete-set law. Nothing about that is cheap, and it
is paid to avoid handing a founder-configured key a standing position in the
market's outcome space.

**2a also strands collateral, and the record must say the uncomfortable half.**
The Hoard keeps `M · unit` behind claims that no longer exist at outcome `i`. It
is not lost — the complete-set law reads `hoard = max_i supply_i` and the excess
is zero, so the market's own arithmetic is unchanged — but the atoms behind a
burned residual are only released when the market terminates, and until then they
are collateral nobody can move. The beneficiary row would have handed them to
someone on the spot. That is what was traded away.

**2b makes one solver's answer canonical and forecloses the maker-price rule.**
A Direct fill today clears at the *maker's* limit; in the batch the same two
orders certify at every price between the two limits and this tie-break picks the
lexicographic minimum instead. The note is explicit that *"the maker-price rule
is a different tie-break, not a different mechanism"* — so choosing the vector
minimum quietly moves the surplus toward the taker on exactly the two-order book
where Direct has a rule of its own. Direct stays the no-solver path, so nothing
breaks today; it is a divergence that must be disclosed the day the RFQ rides
General's verifier.

**2c costs cohort-17 nothing and constrains every cohort after it.** The visible
book ships. What is given up is the freedom to add a conjunct that only a visible
order can satisfy — a small-sounding restriction that is the whole content of
`O-019`, and the reason ember's 0018 was safe to make.

## 7. The cost of reversal

**2a → the beneficiary row.** Cheap in code — it is the `Distribute` row the
note designed, and the kernel keeps a partial burn it no longer needs — and it
reopens decision 0024 item 2, because the tree would then have a party that
receives value for holding a configured key. Reversing after cohort-17 also means
re-founding: which disposition a market's batches use is verifier behaviour
pinned by the Strategy record.

**2a → refuse residuals.** The expensive reversal: it takes the LP back out, caps
every mint at the thin outcome's demand, and returns `Examples.thinRationed`'s
160-against-320 to being the shipping outcome.

**2b → another content-derived rule** (maker-price, maximum vector, minimum
distance to the previous batch's clearing) costs one selection-policy criterion
and no theorem, at any time. This is the least expensive ruling in the record to
reverse and the one most likely to be revisited once a real book exists.

**2c → visible by design.** Reversing means declaring the information advantage a
property of the venue rather than a defect. It costs nothing to build and it
spends the option `O-019` exists to hold: the batch relation would be free to
widen, and `docs/INTENT.md:118-120` states the consequence — *a door closes
permanently*. Per `docs/OMISSION_INDEX.md:59`, that reversal is not the
orchestrator's to make; it needs a new dated ruling from ember that explicitly
permits the door to shut.

## Evidence pointers

`docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md:112-150`, `:152-197`,
`:198-212`, `:226-292`, `:339-389`, `:434-475`;
`formal/dclutch-semantics/DClutchSemantics/JointClearingV1.lean:380-386`,
`:634-657`;
`formal/dclutch-semantics/DClutchSemantics/EconomicKernel.lean:67-70`;
`formal/dclutch-semantics/DClutchSemantics/GeneralClearing.lean:220-230`;
`crates/dclutch-general-adapter-contract/src/runtime_verify.rs:191-192`,
`:1234-1245`; `crates/dclutch-general-config-contract/src/lib.rs:101, 365, 407`,
`v3.rs:95`; `crates/dclutch-general-adapter-contract/src/collection_v1.rs:114-132`;
`tools/gauntlet/journey/src/ledger.rs:11-14`, `:1004-1012`;
`docs/decisions/0018-privacy-horizon-not-this-clutch.md` §2;
`docs/decisions/0024-sustainable-economics-and-a-governable-parameter-surface.md`
item 2; `docs/decisions/0010-general-candidate-escrow-and-the-set-relaxation.md` §1;
`docs/decisions/0031-the-mechanism-agenda.md`;
`docs/OMISSION_INDEX.md:59`; `docs/INTENT.md:114-120`;
`GOAL.md:2071-2094`, `:4715-4721`;
commits `554a29119`, `e37116b03`.
