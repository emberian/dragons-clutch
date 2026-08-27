# Site content map — the Dragon's Clutch literate microsite

Status: **PLAN / PROPOSED (2026-08-20).** This is the information architecture
for a public GitHub Pages microsite. It promotes nothing: every fact the site
may state is bound to its claim plane per `CURRENT_TRUTH.md` §1, and the site
inherits that discipline as a *feature*, not a disclaimer. Produced from a
deep read of the corpus; every cite below was read, not recalled. The
companion document is
[`CONCEPT_INVENTORY_2026-08-20.md`](CONCEPT_INVENTORY_2026-08-20.md), which
maps each concept the site teaches to its authoritative source.

## 0. The job

Three audiences, one text:

- **degens** — people who trade and want to know what this lets them do that
  Polymarket, Deribit, and perp venues do not;
- **wannabe programmers** — people who can read a code block and want to see
  how the bytes actually move;
- **academics** — people who study instruments and market microstructure and
  will check whether the theorems say what the prose says.

And one thesis that serves all three: **Dragon's Clutch is a machine-native
language for beliefs about bounded quantities** — typed outcome claims from
binary bins up through degree-three smooth splines, cleared by an exact
integer batch relation whose acceptance *is* an optimality certificate *is*
a published probability density, settled under conservation identities that
hold to the atom, on a substrate that refuses rather than approximates. As
AIs trade, the precision ceiling of what a market can *say* becomes the
binding constraint; this protocol raises it, and does so in a form
(exact rationals, refusal-first, verify-not-find) that autonomous agents can
consume without trusting an operator.

The site never argues "use this." It argues "understand this." The corpus is
unusually honest about what does not exist; the site's charisma comes from
that honesty, not despite it.

## 1. The one worked example

**One market, followed across every page: "the Friday clutch" — a degree-one
smooth claim market on the time-weighted average price of SOL/USD over one
frozen week.**

- 8 knots from $100 to $240 (gap $20), degree 1, denominator `D = 64`,
  price scale `S = 10,000`. Eight Eggs — eight hat functions, each a claim
  that pays in proportion to how close the realized TWAP lands to its knot.
- **Maya** (a person) buys a tent of Eggs around $160 — one atomic portfolio
  order with coefficient vector `(0, 0, 1, 2, 1, 0, 0, 0)` — because she
  thinks SOL ends the week near $160 and wants to own that *shape*, not a
  yes/no.
- **Theo** (a person) splits complete sets and sells into the same epoch.
- **Sigma** (a solver bot — deliberately an AI, because that is the thesis)
  reads the frozen book, computes a clearing off-chain, and submits it as a
  *candidate*. The chain does not trust Sigma; it re-verifies every byte.
- The epoch freezes → Sigma's candidate is walked to VERIFIED by the
  streaming relation → selection picks the best valid submitted candidate →
  entitlements freeze → settlement moves cash and Eggs under exact
  conservation → the week ends, the TWAP resolves at $163.40 → hats evaluate,
  largest-remainder rounding distributes the last atoms → Maya redeems,
  Theo redeems, both withdraw, and every balance walks to zero.

Why this example and not another:

1. Every mechanical step mirrors evidence that actually exists in the tree:
   the per-degree blank-bank joined lifecycle (`896a1cc`, CURRENT_TRUTH §6.1),
   the Tier-2 general clearing walk — a 40-order, 3-page book with portfolio
   orders and tombstones placed, frozen, walked to VERIFIED with the on-chain
   verdict byte-equal to the host relation's, selected, entitled, and settled
   with whole-plane conservation asserted (CURRENT_TRUTH §4, clearing row;
   GOAL.md done-log, T2-6/T2-8) — and the 22-step signed walk that ends with
   every balance at zero (docs/implementation/COMMITTED_SBF_WALK.md).
2. Degree one is where the magic is *provable*: hats are butterflies, the
   price vector is literally a probability measure, and the site can say so
   with a theorem behind each clause (docs/research/DUAL_IS_THE_MEASURE.md
   §7.2–7.3). Degree two and three exist and are proved as basis
   constructions, but their measure story genuinely breaks (§7.4) — the site
   shows the ladder and tells the truth about the top rungs.
3. It gives all three audiences a handle in the same paragraph: Maya's tent
   is a trade, a coefficient vector, and a piecewise-linear payoff functional
   at once.

**Labeling rule:** the Friday clutch is an *illustration*. No such market
exists anywhere. Its arithmetic is real (checkable with the repo's own
constants); its existence is fiction; every page that advances the story
carries the evidence badge of the mechanism it illustrates, pointing at the
bank campaign or theorem the step mirrors.

## 2. The page tree (7 pages)

### P1. The Clutch (home) — *what this is*

Beats:
1. A market is a question with a frozen answer procedure. No reporter, no
   committee, no discretion — a transaction either carries uniquely
   qualifying evidence or it is refused (PROJECT.md §6).
2. Deposit collateral into the Hoard, receive one of every Egg — a complete
   set, a Clutch. The Clutch is money: it merges back to collateral at par,
   before or after resolution.
3. The one promise, stated as the corpus states it: *for every reachable
   state, the market-local Hoard covers the maximum payout allowed by the
   market's immutable terms* (PROJECT.md §1). No debt, no margin call, no
   liquidation, no socialized loss — not as policy, as geometry.
4. Meet the Friday clutch: the question, the knots, the eight Eggs. Maya
   wants a shape, not a side.
5. Where you are: a local prototype with an unusual honesty apparatus —
   follow the badge on any claim to see exactly what kind of evidence backs
   it. (First appearance of the evidence-badge device; links to P7.)

### P2. The Shape of a Claim — *why the algorithms are powerful, part 1*

Beats:
1. Degree zero is every prediction market you already know: exhaustive,
   disjoint bins, one winner. Fine for elections; a lie for prices — your
   P&L cliff-edges at an arbitrary bin boundary.
2. Climb the ladder: degrees one through three are open-clamped B-spline
   Eggs — overlapping, nonnegative, summing exactly to one everywhere
   (partition of unity, machine-checked: CURRENT_TRUTH §3). Payout degrades
   *continuously* in the resolved value; manipulation buys the manipulator
   value proportional to the nudge, not a full unit per straddled holder
   (RISK_SUMMED_POSITIONS §4.1).
3. A portfolio is a coefficient vector over the basis. Maya's tent is one
   asset, one atomic fill, no leg risk — on an options venue it is three
   legs and a margin computation.
4. The hat is a butterfly: the discrete Breeden–Litzenberger second
   difference *is the claim price itself*; no inversion, no fitted smile
   (DUAL_IS_THE_MEASURE §7.3). Buying the density is native.
5. Truth about the top rungs: degrees two and three are proved as exact
   basis constructions and currently refuse at terms admission; at degree
   ≥ 2 the simplex gate stops being the no-arbitrage body — with the
   explicit arbitrage in the corpus (§7.4). The ladder is honest about
   where it ends.

### P3. The Clearing — *why the algorithms are powerful, part 2*

Beats:
1. Nobody on-chain searches for the clearing. Anyone off-chain may. The
   chain's job is *verify-not-find*: a candidate names the price vector,
   the imbalance, and the fills; everything else is derived and checked for
   exact equality; "best valid submitted candidate," never "optimal" —
   until a certificate says otherwise (DUAL_IS_THE_MEASURE §1).
2. Sigma the solver submits. The streaming relation walks the frozen book —
   pages, tombstones skipped, every order's funded reservation re-verified —
   across many transactions, checkpointed in a 48 KB account that refuses
   tampering at three layers (TIER2 plan T2-1/T2-6).
3. The punchline the corpus proved this month: under the
   certificate-demanding policy, an accepted candidate carries a
   zero-duality-gap proof that no feasible clearing of the same book beats
   it — and the witness price vector is the optimal dual
   (DUAL_IS_THE_MEASURE Thm 5.1; scoped, with its falsifiers named).
4. And the dual is a *measure*: at degree ≤ 1, every publishable price
   vector is exactly a probability distribution's moment vector. Clearing
   the batch and publishing the market's density are one act (Thm 7.1).
5. The Friday clutch clears: the walk, the verdict, selection among
   candidates by re-derived tie digests, Maya's tent filled. Evidence
   badge: this whole lifecycle ran in a local bank, verdict byte-equal to
   the host relation, and is UNPROMOTED (CURRENT_TRUTH §4).

### P4. The Ledger — *conservation, resolution, and walking to zero*

Beats:
1. One identity rules custody: `H = L + P + S` — actual Hoard atoms equal
   retained claim backing plus every Position's cash plus unsolicited
   surplus (CURRENT_TRUTH §5). Every transition's exact deltas, in one
   table.
2. Settlement under the identity: the Friday clutch's fills move cash and
   Eggs with whole-plane conservation asserted to the atom — cash sums
   exact, per-outcome position totals exact, final Positions byte-equal to
   the verified summary's implied allocation (GOAL.md done-log, T2-8).
3. Resolution is a measurement: the one transition that is not an isometry —
   the risk space collapses to a point, and the requirement can only fall
   (RISK_SUMMED_POSITIONS §1.5). The TWAP lands at $163.40; the hats
   evaluate; largest-remainder rounding with lowest-index ties distributes
   the final atoms, deterministically.
4. Redemption is exact-or-refuse: fractional lots return
   `RemainderRequired` before any mutation; nothing is silently rounded
   against you (CURRENT_TRUTH §3).
5. Walk to zero: both owners redeem and withdraw; 18 watched accounts
   reload; the pooled Hoard ends at zero — and the corpus's own gate went
   red when one terminal expectation was corrupted, which is what makes the
   green mean something (COMMITTED_SBF_WALK.md; CURRENT_TRUTH §2).

### P5. The Price of Risk — *fees and collateral as geometry*

Beats:
1. What should a venue charge for? Not principal, not redemption, not
   carrying a riskless Clutch — adding complete sets to any position
   changes nothing that matters, so a principled fee must vanish on the
   diagonal (FEE_GEOMETRY §3; RISK_SUMMED_POSITIONS §3.1).
2. The candidate: state-contingent dispersion `G(a,p)` — the exact
   generalization of `q·p·(1−p)` to any payoff shape, *uniquely* forced by
   two axioms (Props 11–12), computable in at most 120 integer pair terms.
3. The twist the corpus refused to hide: `G` is provably *not* the
   model-free risk norm — it is bounded by `R(a)/4` and vanishes at extreme
   prices while at-risk capital stays fixed (Prop 10), and at zero prices
   its kernel grows a laundering hole (Prop 9). Two candidates survive; the
   choice is economics, not mathematics; and the fee is **forced to zero in
   every configuration currently true in the tree** (ECONOMICS §6).
4. Collateral is the other half of the geometry: requirement = sup-norm,
   exact at degrees ≤ 1; settlement of one leg can never raise the margin
   on the rest (Prop 8) — "margin call" has no referent here; and no
   model-free calendar or cross-asset netting exists, *provably*, so the
   sum this venue charges is the honest number, not a lazy one (Prop 6).
5. What that costs, plainly: tail writers lock the full worst case; no
   leverage; capital to term. The trade-off table, both columns
   (RISK_SUMMED_POSITIONS §2.4, §4.2).

### P6. For the Machines — *the AI thesis*

Beats:
1. Markets are how agents with different information reach one number. The
   constraint on that number's *resolution* is the instrument language: a
   binary market lets a sophisticated agent say one bit about a
   distribution it models in full.
2. Here the language is typed: a belief is a coefficient vector; its degree
   is its precision; an agent that upgrades its model from "above/below" to
   a density upgrades its *order*, on the same substrate, with the same
   conservation laws.
3. The read side is native too: the cleared price vector *is* the implied
   density at grid resolution (`p_i/(S·g)`), the implied forward is one
   portfolio quote — machine-readable market state with no inversion step,
   no indexer of record (DUAL_IS_THE_MEASURE §7.3, §9.1).
4. The work is permissionless and verifiable: solvers like Sigma compete by
   submitting candidates the chain re-verifies exactly; observations,
   repairs, and cleanup are paid public instructions; nothing requires
   trusting the operator because there isn't one to trust (PROJECT.md §8).
5. Refusal-first is what makes it agent-safe: exact integers, no silent
   rounding, hostile-byte parsing, and a system that lapses rather than
   publishes a false number — the properties you want *before* you point an
   autonomous trader at a venue. The claim-plane vocabulary itself (P7) is
   machine-checkable honesty an agent can consume.

### P7. The Evidence — *how this project says true things*

Beats:
1. The claim vocabulary, verbatim: PROVED-MODEL, CHECKED-RUST-SUBSET,
   CHECKED-FINITE, HOST-TESTED, SBF-EXECUTED, PROFILE-ADMITTED, MODEL-ONLY,
   PROPOSED, STOP — deliberately nontransitive; a green model never
   impersonates runtime evidence (CURRENT_TRUTH §1). This page is where
   every badge on the site resolves.
2. What the strongest evidence looks like: 184 zero-sorry Lean theorems;
   eight Lean-computed fixtures byte-equal to the digest-pinned production
   evaluator with five real-source mutants going red; a 34,766-case
   independent differential; sealed byte-identical builds; an independent
   second-host attestation at 44/44 gates, 0 STOP.
3. What honesty looks like when it hurts: the compute ceiling that
   "killed" on-chain re-verification was one dependency's software SHA-256 —
   a 53,952-byte symbol; with the syscall, the same measured route fell
   from exactly 1,400,000 CU (rollback) to 226,071 (commit). The corpus
   kept the wrong verdict in the record, labeled, next to its correction
   (COMPUTE_CEILING_REATTRIBUTION). Sites don't usually show you their
   wrong answers; this one does, because the correction *process* is the
   product.
4. The two-ELF discipline: success against a mock source requires a
   *different, explicitly non-production* program; the default artifact
   refuses value with error `0x79` because its source registry is empty.
   The system is built to make its own incompleteness machine-visible.
5. The scope note (§6 below), in full, once.

**Cross-page thread:** the Friday clutch advances one lifecycle stage per
page (P1 born → P2 shaped → P3 cleared → P4 settled and resolved → P5 what
it cost → P6 who traded it → P7 what all of that was evidence of).

## 3. The audience-lane device

**Mechanism: braided registers — one main text, three tagged lane callouts.**
The main prose is written for everyone (register: plain, vivid, exact). Any
section may close with up to three short callouts, visually distinct
(rendered as compact labeled asides), each re-saying the section's central
fact in one lane's native register:

- `⟨degen⟩` — what you can do with it, in trade language;
- `⟨builder⟩` — how the bytes do it, with a file cite;
- `⟨scholar⟩` — the precise statement and where it is proved.

Rules that keep it honest: the main text never *requires* a callout; a
callout never introduces a fact the main text lacks (it re-registers, it
doesn't smuggle); every `⟨builder⟩` and `⟨scholar⟩` callout carries a repo
cite; lanes never condescend — the degen lane is sharp, not dumbed down.

**Proof paragraph, three-voiced** (topic: the complete set):

> Main text: A Clutch — one of every Egg — is worth exactly one collateral
> unit no matter how the market resolves, because the basis weights sum to
> one at every admissible value. That makes complete sets money: mint them
> by depositing, melt them by merging, park cash in them without taking a
> view.
>
> ⟨degen⟩ The complete set is your risk-free exit and your inventory. Hold
> "everything except my view" and you're flat with yield-free cash — no
> funding rate, no liquidation price, redeemable at par before or after
> resolution.
>
> ⟨builder⟩ `split` adds `+q` to every outcome balance and debits `q` cash;
> `merge` is its exact inverse; both are pure reclassifications inside the
> pooled Hoard — no token moves (`crates/clutch-kernel`; accounting table,
> CURRENT_TRUTH.md §5).
>
> ⟨scholar⟩ This is unitality of the payoff operator: `Φ(1) = 1` under H2,
> so `span(1)` is the risk-free direction and required collateral moves in
> lockstep along it — Proposition 4 characterizes these as *exactly* the
> counterparty-free value-preserving moves
> (docs/research/RISK_SUMMED_POSITIONS.md §1.1, §1.4).

## 4. The honest-claims sidebar pattern

**Evidence badges, not disclaimers.** Every load-bearing factual claim on
the site carries an inline badge naming its claim plane, styled as a small
capsule after the sentence, linking to an anchor on P7 and to the citing
file in the repo:

> The on-chain verdict is byte-equal to the host relation's.
> `[SBF-EXECUTED · bank · UNPROMOTED — CURRENT_TRUTH.md §4]`

Design rules:

1. **The badge vocabulary is the repo's, verbatim** — the site invents no
   softer synonyms. If a fact is MODEL-ONLY, the badge says MODEL-ONLY.
2. **Badges are nontransitive and never upgraded by adjacency.** A proved
   theorem next to an executed instruction does not make the instruction
   proved; the site's layout may never imply otherwise (no shared badge for
   a paragraph mixing planes — split the paragraph).
3. **One badge per claim, at the claim.** No global disclaimer page doing
   the work badges should do locally — and no scattered hedging doing the
   work the one scope note (§6) does globally. Armor nowhere, precision
   everywhere.
4. **STOPs are content, not shame.** Where the site touches something that
   does not exist (live source, fees, deployment), the badge is `[STOP]` or
   `[PROPOSED]` and the surrounding prose says plainly what gate is open —
   in the same confident register as everything else.

## 5. Show-stoppers worth citing (with verification paths for the builder)

Numbers and theorems the site should spend, each with the file a site
builder verifies against:

1. **184 zero-sorry Lean theorems**, axioms only `propext`,
   `Classical.choice`, `Quot.sound`; the B-spline file alone: 159
   declarations, 116 theorems (docs/reviews/PLANNED_VS_BUILT_2026-08-19.md
   "Quietly superseded" item 1; CURRENT_TRUTH.md §3; lean/DragonsClutch/).
2. **The clearing walk verdict, byte-equal:** a 40-order, 3-page, 4-outcome
   book with 2 portfolio buys and 3 tombstones placed through the general
   arm, frozen, and walked to VERIFIED across ~20 transactions with the
   on-chain verdict and persisted score byte-equal to the host relation's
   (GOAL.md done-log T2-6, merge `87fd342`; CURRENT_TRUTH.md §4 clearing
   row).
3. **Whole-plane conservation at settlement:** cash sums exact to the atom,
   per-outcome totals exact, final Positions byte-equal to the verified
   summary's implied allocation (GOAL.md done-log T2-8; CURRENT_TRUTH.md §4).
4. **`H = L + P + S`** — the custody identity and its per-transition delta
   table (CURRENT_TRUTH.md §5).
5. **The zero-gap theorem:** accepted ⇒ surplus-optimal for the LP
   relaxation with the witness price as optimal dual, under the named
   policy tuple; and **the measure theorem**: at degree ≤ 1 every
   publishable price vector is a probability measure's basis-moment vector,
   explicitly (docs/research/DUAL_IS_THE_MEASURE.md Thm 5.1, Thm 7.1 —
   paper proofs, falsifiers named in its §11; badge accordingly).
6. **Hats are butterflies:** `p_i/S = [C(t_{i−1}) − 2C(t_i) + C(t_{i+1})]/g`
   — Breeden–Litzenberger pre-inverted; implied forward = one portfolio
   quote (DUAL_IS_THE_MEASURE §7.3).
7. **The fee characterization pair:** `G` is the *unique* 1-homogeneous
   layer-additive extension of `q·p·(1−p)` (Props 11–12) and is provably
   *not* the model-free risk norm — `G ≤ R/4`, envelope exact (Prop 10),
   kernel hole at zero prices (Prop 9)
   (docs/research/RISK_SUMMED_POSITIONS.md §3; docs/FEE_GEOMETRY.md §3).
8. **No post-settlement margin call, structurally** (Prop 8) and **no
   model-free calendar/cross-asset netting exists** (Prop 6) — the honest
   comparison table against SPAN/VaR venues
   (docs/research/RISK_SUMMED_POSITIONS.md §2.2–2.4).
9. **The model/executable bridge:** 8 Lean-computed fixtures byte-equal to
   the digest-pinned production evaluator; 5 real-source mutants compile,
   execute, and go red; 34,766-case independent Python differential
   (CURRENT_TRUTH.md §3, `be8eba3`).
10. **The compute reattribution:** one 53,952-byte software-SHA symbol;
    every measured route 3–8× cheaper via `sol_sha256`; Direct V2 selection
    from exactly 1,400,000 CU (rollback) to 226,071 CU (commit)
    (docs/reviews/COMPUTE_CEILING_REATTRIBUTION_2026-08-19.md).
11. **The walk to zero:** 22 signed sequential transactions including two
    *expected refusals*, 18 accounts reloaded, terminal balances zero, and
    a deliberately corrupted expectation failing the gate on committed
    bytes (docs/implementation/COMMITTED_SBF_WALK.md; CURRENT_TRUTH.md §2).
12. **The evidence machine:** 100/100 manifest gates executed; independent
    second-host attestation 44/44 portable gates PASS, 0 STOP; sealed ELF
    identities with cross-OS divergence *exhaustively classified* down to
    toolchain path strings (CURRENT_TRUTH.md §2; GOAL.md done-log Cycle D).
13. **Honesty as a number:** the mock source provider's account body is the
    literal string `MOCK-PROVIDER-V1`, recorded in the corpus's own
    assessment "the joins are the fiction"
    (docs/reviews/SOPHISTICATION_GAP_2026-08-19.md §1) — cite it *on the
    site* as proof the badges mean something.
14. **Small numbers that teach:** `MAX_OUTCOMES = 16`
    (crates/clutch-kernel/src/lib.rs:31) — hence at most 120 fee pair
    terms; `FEE-001`: a 1-atom fee on 1 atom of consideration is 10,000
    basis points on the smallest fill — the corpus's own lab refusing to
    let its fee look flattering (docs/FEE_GEOMETRY.md §4;
    research/economics/fixtures.py).

## 6. What NOT to say — the scope note (once, on P7, linked from every badge)

> **Scope.** Dragon's Clutch is a research prototype. Nothing is deployed;
> no market, token, or deployment carries real value, and none is offered.
> The clearing plane and smooth-claim lifecycle you read about here executed
> in local test banks and remain UNPROMOTED in the project's own liveness
> profile; fees are forced to zero everywhere; the default build refuses
> deposits (`0x79`) because no production data source exists yet; no
> external security audit, signed release, or legal determination exists.
> The mathematics pages distinguish machine-checked theorems from paper
> proofs from measured executions — follow any badge. Nothing on this site
> is an offer, a solicitation, financial advice, or a statement that any
> regulator has approved anything.

That is the *entire* hedge. No other page hedges. The badges carry local
truth; this note carries global scope; prose everywhere else is confident.

## 7. Visual and diagram opportunities (6)

1. **The clearing walk as a comic strip** (P3): one panel per phase —
   place, freeze, the walk (the checkpoint account as a recurring character
   carrying its consumed-fold latch), VERIFIED, select, entitle, settle.
   Sigma the solver appears outside the panels; only its candidate crosses
   the border, which *is* the verify-not-find lesson drawn.
2. **The conservation ledger as an animation** (P4): four bars — Hoard `H`,
   backing `L`, position cash `P`, surplus `S` — morphing through Endow →
   Split → trade → Resolve → Redeem → Withdraw with `H = L + P + S`
   pinned true in every frame; the final frame is all-zeros.
3. **The degree ladder** (P2): the same question rendered at degree 0
   (bins, cliff edges) and degree 1 (hats, continuous), with degrees 2–3
   sketched and labeled "proved, refused at admission" — the honesty is in
   the diagram itself.
4. **The hat is a butterfly** (P2/P3): overlay one hat basis function on
   the three-strike butterfly payoff; annotate the second-difference
   identity; then show the full cleared price vector re-plotted as a
   density histogram `p_i/(S·g)` — the market's belief, drawn from data
   already on-chain.
5. **The simplex and the diagonal** (P5): the price vector as a point on a
   2-simplex; complete-set motion as travel along the diagonal (free, by
   theorem); the fee's kernel as that diagonal — and, at a boundary vertex,
   the kernel visibly fattening (Prop 9's hole, drawn).
6. **The evidence map** (P7): the claim planes as literal territories —
   Lean model, host Rust, SBF bank, sealed artifact — with the refinement
   boundaries drawn as *gaps with named bridges where bridges exist*
   (the 8-fixture CHECKED-FINITE bridge; the Verus subset bridge) and open
   water where they do not. Most protocol sites draw one continent; this
   map's honesty is its beauty.

## 8. Sample section (the site's actual voice)

*From P2, "The Shape of a Claim" — section: "Buy the density."*

---

### Buy the density

A prediction market asks you a yes/no question. A perp asks you a
direction. Both throw away almost everything you actually believe. If you
think SOL ends the week *near* $160 — probably between $140 and $180,
almost certainly not above $220 — a binary market makes you round that
belief to one bit, and a perp makes you convert it into a leverage number
and a liquidation price you didn't want.

Dragon's Clutch asks a better question: *what shape is your belief?*

The Friday clutch has eight Eggs — eight hat-shaped claims, one per knot
from $100 to $240. Each hat pays its full weight if the week's
time-weighted average price lands exactly on its knot, and fades linearly
to zero at the neighbors. The hats overlap, none is ever negative, and at
every admissible price they sum to exactly one — which is why one of each,
a complete set, is worth exactly one collateral unit no matter what
happens. That "sums to exactly one" is not a design intention; it is a
machine-checked theorem about the emitted basis, one of 184 the project
maintains at zero `sorry`.
`[PROVED-MODEL — CURRENT_TRUTH.md §3; lean/DragonsClutch/BSpline.lean]`

Maya's belief is a tent: nothing below $120, rising to a peak at $160,
gone by $200. She writes it as a vector — `(0, 0, 1, 2, 1, 0, 0, 0)` —
and that vector *is* her order. One asset, one atomic fill, one price. No
legging into three strikes, no leg risk, no margin rule to trust: the
collateral her tent requires is its worst-case payout, read directly off
the vector, and settling one leg of anything can never spring a margin
call on the rest, because there are no legs and there are no margin calls.

Here is the part that should make the academics sit up. On an options
venue, the market's implied probability density is something you
*reconstruct*: fit a smile, differentiate twice, pray. Here the
reconstruction is pre-inverted. A hat claim's payoff is algebraically
identical to a butterfly spread, so the cleared price of the $160 Egg
already *is* the second difference of call prices — the market's
probability mass near $160, quoted directly, tradable directly. When the
batch clears at prices `p`, dividing by the knot gap hands you the
density histogram. Nobody computes the market's belief after the fact;
the clearing *publishes* it, exactly, as a side effect of being verified.
`[paper proof — docs/research/DUAL_IS_THE_MEASURE.md §7.3]`

⟨degen⟩ You can finally own "it pins $160" as one position with defined
risk and no liquidation price. The whole curve is a quote board: cheap
hats are where the market thinks it won't land. Disagree? That's the
trade.

⟨builder⟩ The tent order travels as a coefficient array over the frozen
basis; the evaluator is safe, `no_std`, allocation-free, float-free
exact-rational Rust, and eight Lean-computed fixtures match its production
bytes while five deliberately broken variants fail
(crates/clutch-bspline; CURRENT_TRUTH.md §3).

⟨scholar⟩ Prices of a complete basis live on the scaled simplex; at degree
≤ 1 every simplex point is the moment vector of an explicit representing
measure `Q* = Σ (p_i/S)·δ_{t_i}`, so "implied density at grid resolution"
is exact language, not metaphor (DUAL_IS_THE_MEASURE §7.2, Thm 7.1).

One honest edge: eight knots is a coarse curve, the grid is frozen at
creation, and payoffs are bounded at the edges — no unbounded upside
exists here, by construction. What you get for that is a market that can
tell you, to the atom, what it believes.

---

*(~590 words. The register above — declarative, concrete, one badge per
load-bearing claim, lanes braided at the end, the honest edge stated in
the same confident voice — is the site's voice everywhere.)*

## 9. Production notes

- Static site, GitHub Pages, no RPC, no wallet, no analytics — the site
  inherits Static Glass's zero-operator posture (docs/STATIC_CLIENT.md) and
  should say so on P7 in one line.
- Diagrams as inline SVG with both themes; the ledger animation degrades to
  a static frame sequence.
- Every badge is a link; every cite in §5 resolves to a file in this
  repository at a pinned commit, so the site builder can verify each number
  before publishing and re-pin on reseal.
- The Friday clutch's arithmetic should be generated by a small checked
  script against the repo's constants (D, S, largest-remainder rule) so the
  illustration can never drift from the semantics it illustrates.

## 10. Build errata (2026-08-20, v1 site build)

Recorded during the build of `site/`; every site number was re-verified
against the tree, and these are the places this plan needed correction or
sharpening.

1. **P7 beat 1 lists nine claim labels; CURRENT_TRUTH.md §1 defines ten.**
   IN-FLIGHT is omitted above. The built site lists all ten, verbatim.
2. **§5 item 12's 44/44 attestation is GOAL.md's Cycle D** (over exact
   `788581c`); CURRENT_TRUTH.md §2's own newest attestation paragraph still
   records the earlier 41/41 over `98fb070` and has not absorbed Cycle D.
   The site cites 44/44 to the GOAL.md done-log, Cycle D.
3. **The §8 sample's phrase "a machine-checked theorem about the emitted
   basis" over-reaches by one plane.** The 184 theorems are about the Lean
   model; the tie to the production evaluator is the finite 8-fixture/
   5-mutant CHECKED-FINITE bridge, not a refinement theorem. The built site
   says "one of 184 theorems the project maintains at zero `sorry`" and
   lets the badge plus P7 carry the boundary.
4. **"48 KB account" (P3 beat 2) is exactly 48,750 bytes** (TIER2 plan
   T2-3); the site states the exact figure.
5. **The §9 production-note script now exists:**
   [`friday_clutch_check.py`](friday_clutch_check.py) — all checks pass,
   and the site's illustration numbers (weights `(0,0,0,53,11,0,0,0)/64`,
   1,600 lots at 1,728 atoms, payouts 2,925/275, ledger frames at 5,200,
   density 0.0165/$ at $160, forward $170, `G = 0.4086 ≤ R/4`) are its
   output.
