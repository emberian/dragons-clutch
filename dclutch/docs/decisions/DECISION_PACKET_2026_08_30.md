# Decision packet — 2026-08-30

Seven open decisions. Four are ruled here by the orchestrator (veto window
open); three are ember's. Each entry: context in brief, the ruling or the
question, and what moves once settled. Full evidence lives in the cited
records.

## Status: CLOSED — every question below is answered

Recorded here so the packet does not read as open. Ember's rulings are in
`WAVE.md` under "Rulings — ember, 2026-08-30 (evening, on the decision
packet)" (`f28036bf`) and its E2 addendum (`458d47bb`); each is carried into
the record it rules on.

| question | outcome | where it landed |
|---|---|---|
| §1 D2 fee band | adopted, `MAX_FEE_BPS = 500` — **enforced only by a shell guard**, no protocol const | `0014` status |
| §1 D3 rate diversity | adopted in principle, **blocked**: fee-bearing is 115,003 CU over the ceiling all-first-try | `0014` status; `DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md` |
| §2 checked release identity | option A adopted; `dclutch-release-tool` stays strict | `0016` status |
| §3 cache-read authentication | A ratified, B chartered (52,592 CU measured), C refused | `0017` status |
| §4 continuation + root tails | top-level is the production route; the **Hot** continuation is harness-only; the **founding** continuation is untouched | `CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md` |
| E1 protocol revenue | **deferred, build nothing** — as-built stands by default, revisit pre-mainnet | `0014` status |
| E2 dead markets | ruled B, then found unexecutable; **write-off accepted**, markets stand, option C is the live disposition | `0015` status + §8 |
| E3 seal rent beneficiary | **still open** — leaning collector-keeps, burn as fallback, final call deferred until the consequences are laid out. No `CloseSeal` implementation. | this packet, §E3 |
| E4 unpaid-fee receivable | accepted, no deadline | `WAVE.md` E4 |
| E5 maker lockout | accepted **conditional on guaranteed unilateral self-cure**, including a vanished recipient account — a charter requirement for the fee lanes, not a suggestion | `WAVE.md` E5 |

Unbuilt after all of it: D2's protocol-side band, 0017's per-family
continuation tripwire, 0017 option B, `CloseSeal`, E5's self-cure proof, and
0015 option C's web bucket.

## Ruled now (veto window open)

### 1. Fee bound and demo diversity — 0014 D2 + D3

D2: **adopt option B, `MAX_FEE_BPS = 500`, no lower bound.** This overrides a
proved-admissible boundary (consent-is-the-whole-bound) and says why: a market
whose config takes everything is signable and indistinguishable at a glance
from an honest one; consent bounds fraud, not what a venue should host. Zero
stays expressible as a declaration (gen-1's rule).
D3: **adopt in principle — rate diversity on the demo — but sequenced behind
FEEWALL**: a fee-bearing trade has never executed and arithmetic puts it over
the CU ceiling; until FEEWALL measures the real shape (or the two-transaction
lifecycle lands), every founded market is zero-fee by necessity, declared.

### 2. Checked release identity — 0016

**Adopt option A**: a checked release describes the source by
`semantic_release_id`, the artifact by the ELF digest, and the account by a
policy the live observation must satisfy — three facts, three authors, no
self-reference. Recorded; M-25 closes. The 0012 residual is also ruled:
**`dclutch-release-tool` stays strict** — an iteration substrate is named,
never defaulted into.

### 3. Cache-read role authentication — 0017

**Ratify option A** (zero code; the shape is built and load-bearing).
**Charter option B**: the objection was that its payoff was qualitative;
SEALWIDE has since measured it — 52,592 CU, invariant across 32 keys and two
builds — so B is now ordinary costed work. **Refuse C.** Ratification ships
with the tripwire the record asks for: a per-family test exercising a child
under a real continuation.

### 4. Continuation route disposition + root tails

Per HEAPRED's evidence: **top-level is the production route; the Hot
continuation is demoted to harness-only; the heap test re-bars on the +35,127
delta; the compute fix is not chartered; full retirement waits until the ~20
program-tests are ported.** Scope carve-out (CORESTATE-3): the *founding*
continuation is load-bearing since `2dc53776` and is untouched by this ruling.
Rational/Structured **root-tail ABIs are chartered design-doc-first**, one per
family, through WALL22's template (its constructor gate refuses a bricking
bundle structurally); permanence is why they get a design doc and review, not
why they wait.

## Ember's three

### E1. Protocol revenue — 0014 D1

What's built: fees route to a per-market `fee_recipient` bound at founding.
**There is no protocol take and no treasury — the protocol earns nothing;
market founders do.** This is the strongest form of "nothing requires a
service we operate," and it dissolves the treasury question. But INTENT
records the demo's motive as turning the pile into a stream — and under D1,
your revenue exists only through markets *you found*. Adopting D1 is
recommended; it should be your chosen answer, not a discovered one.
**Question: is founder-revenue-only the business you want?**

### E2. The two dead markets — 0015 A vs B

Ruled without you: **C now** (they're filed under "open", the one untrue
thing on the site — an honest bucket lands regardless) and **refuse D** (a
sixth phase is the most expensive way to state a derivable fact). The open
choice: **A — leave them standing as witnesses** (first two markets ever
founded here; the site reads them live; the protocol's own record of a wall
hit and reported honestly) versus **B — resolve, redeem, retire** (recovers
rent + the founder's locked collateral, demonstrates the full lifecycle on a
real public market). There is no middle: the only collateral egress is the
full sequence. **Question: witnesses, or hygiene + lifecycle demo?**

### E3. Seal rent beneficiary — P-006

Every Trading release permanently strands the rent of every capability seal
written under its predecessor; the class grows with release cadence. A
`CloseSeal` route needs a beneficiary, and SEALWIDE sharpened the constraint:
seals are not per-market, so **no market's funding may take the refund**
without one market paying for every other's executions.
**Recommendation: pay the closer, capped** — rent liberation as a funded
permissionless crank, the WorkRewardV1 pattern, reward carved only from rent
the close liberates. Alternatives: refund the original submitter (fair,
but submitters are arbitrary and often gone), or burn (clean, wasteful).
**Question: who gets liberated seal rent?**

### E4/E5. The two-transaction fee's semantics — FEE_SECOND_TRANSACTION_V1

Added after the packet's first cut (design landed `54d7e628`). The fee leg
becomes a permissionless second transaction; an unpaid fee is recorded as
`fee_owed` on the buyer's maker replay and blocks that maker's next fill in
that market only.
**E4: is a settled fill with an unpaid fee acceptable, and for how long?**
Recommend: yes, forever, no deadline — an expiry can only forgive or strand.
The price of declining: never trading that market again, over ~1% of one
fill's notional.
**E5: is locking the maker out until they settle acceptable?** Recommend:
yes — scoped to one market, curable only by the debtor. The one asymmetry
(a buyer locked by a crank they didn't see) is a product-surface fix, not
protocol.

## What moves once E1–E3 are answered

E1 → the docs state the revenue model out loud; 0014 closes whole.
E2 → either a web bucket only (A) or one keeper run per market (B); 0015
closes whole. E3 → CloseSeal gets chartered with its beneficiary; unblocks
every future seal class (product-graph seal included).
