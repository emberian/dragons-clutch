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
| E3 seal rent beneficiary | ~~**still open** — leaning collector-keeps, burn as fallback, final call deferred until the consequences are laid out. No `CloseSeal` implementation.~~ **RULED AND BUILT 2026-08-31**: seal rent goes to the **closer**, capped by a profile gate; burn rejected because it preserves the stranding. `CloseSeal` is on chain — the implementation is `f253c4e0`, its five real-ELF cases are `e8668b52`. | `WAVE.md` "Rulings — 2026-08-31"; `f253c4e0`; `OMISSION_INDEX.md` P-006 |
| E4 unpaid-fee receivable | accepted, no deadline | `WAVE.md` E4 |
| E5 maker lockout | accepted **conditional on guaranteed unilateral self-cure**, including a vanished recipient account — a charter requirement for the fee lanes, not a suggestion | `WAVE.md` E5 |

Unbuilt after all of it: D2's protocol-side band, 0017's per-family
continuation tripwire, 0017 option B, `CloseSeal`, E5's self-cure proof, and
0015 option C's web bucket.

**AMENDED 2026-08-31 (LEDGER-TRUE) — that sentence is stale in five of its six
items, and two of them were already false when it was written.** Verified
against the tree and `git`, item by item; the line is kept above because what it
listed on the evening of 2026-08-30 is the record of what the packet cost.

| item | status at HEAD | evidence |
|---|---|---|
| `CloseSeal` | **LANDED on `main`** | `f253c4e0` (*"the stranded seal gets a closer, and the closer keeps the rent"*) is the implementation; `e8668b52` adds the five real-ELF cases; `8c216642` fixed the dead seal **write** outer found on the way. Route dispatched at `programs/dclutch-trading-sbf/src/lib.rs:539-541` into `hot_v3::process_capability_seal_close_v1` (`hot_v3/seal.rs:287`). All three ancestors of `main` |
| 0017 option B | **LANDED, and worth more than chartered** | `1da601e7`, ancestor of `main`. Measured **−66,921 CU**, not the 52,592 the charter costed; the figure is a live assertion, not prose — `TOP_LEVEL_KEY_INDEPENDENT_CU_V1 = 1_254_251` at `programs/dclutch-trading-sbf/program-test/tests/direct_hot_top_level_margin_gate.rs:260`, asserted `:716`. `0017:3` now reads `RATIFIED … B BUILT AND MEASURED` |
| 0015 option C's web bucket | **LANDED — and it was already built when this line was written** | `e3600765` (*"markets: an honest bucket for the two that can never trade"*), 2026-08-30 **19:59**, ancestor of `main`. `marketActivationOutlookV1` is live at `packages/dclutch-sdk/lib/marketDiscovery.ts:1076`, consumed by `MarketDiscoveryWorkspace.tsx:107` and `portfolio.ts:157`. The packet listed it as unbuilt the same evening it shipped |
| 0017's per-family continuation tripwire | **LANDED — and it was also already built** | `09c1c8fc` (*"the ninth wall gets a tripwire, and it has been seen to fire"*, 2026-08-30 23:22) and `46083e7a`, both ancestors of `main`. Tests at `programs/dclutch-trading-sbf/program-test/tests/registry_hot_continuation.rs:1479-1894`; `hot_v3.rs:10604` names it in so many words. Both halves have **executed** reds, not asserted ones. A lane TRIPWIRE was later spawned against this item (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:33-34`) and has produced no branch and no commit — **there is nothing left for it to build here**; what remains is the coverage gap `0017:274-284` discloses (the dynamic half covers one route of Claims; Dealer and Rent have no continuation fixture) |
| D2's protocol-side band | **BRANCH-ONLY — still unbuilt on `main`** | `DIRECT_MAX_FEE_BASIS_POINTS_V1` does not exist in any code on `main`. It is declared and genuinely enforced on `lane/fee-tx2-20260831` — `crates/dclutch-direct-codec/src/successor.rs:65` (`= 500`), refusing at `:397` in `DirectExecutionConfigV1::new` with `SuccessorError::FeeBandExceeded`. That branch is **not** an ancestor of `main` (11 commits ahead). Main's only fee guard is still the denominator, `successor.rs:342` — **a 9,999 bps fee is admitted on `main` today** |
| E5's self-cure proof | **BRANCH-ONLY, and "five ways" is a stitched count** | On `lane/fee-tx2-20260831` only. `9e23ad4d` — whose own subject says *"as **four** legs"* — proves four in `crates/dclutch-direct-codec/src/successor.rs:3070` (`the_debtor_can_always_settle_unilaterally`: no third party can withhold `:3076`, cure re-admits the nonce `:3083`, cannot settle short `:3103`, settling twice is not a second cure `:3113`). The fifth, the **vanished recipient**, is a different kind of thing — a destination bound by owner (`direct_fee_settlement_v1.rs:549`) with one real-ELF case (`direct_hot_fee_pair.rs:663`). Four legs + one case, on a branch |

**The pattern worth naming, because it is the one that wastes lanes:** three of
these were *already true* when the packet declared them unbuilt — the bucket by
four hours, the tripwire by an hour. A closing sentence written from memory at
the end of a long session is exactly where staleness enters, and it enters
pre-dated. The two that really are outstanding are both on the same fee branch,
and neither is a decision — they are a merge.

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
~~**Question: who gets liberated seal rent?**~~

**ANSWERED AND BUILT 2026-08-31.** The recommendation above was taken: **the
closer**, permissionlessly, reward carved only from rent the close liberates,
no Market's funding receiving it (SEALWIDE's constraint). Burn was rejected
because it preserves the stranding this row exists to end. The ruling is in
`WAVE.md`, *"Rulings — 2026-08-31, ember's full-autonomy directive"*, made by
the orchestrator under that directive; the full closure argument is
`OMISSION_INDEX.md` P-006.

**One precision the shorthand loses, verified in the code.** "Capped" here is a
**profile gate, not an arithmetic** — and the distinction is load-bearing
because the phrase "closer-keeps-capped" reads as though a `min()` runs.
`hot_v3/seal.rs:354` takes `let liberated = seal.lamports();` and moves the
**whole balance**. What bounds it is that artifact profile 1 defines no lamport
role beyond rent exemption, and `SealedDescriptorClosureV1::decode` refuses
every other profile (`crates/dclutch-capability-seal-contract/src/lib.rs:481`),
so the only balance this route can ever reach is liberated rent. A future seal
class carrying a bounty or escrow is a different profile byte, which refuses
here until it gets its own close naming its own beneficiary. Paying
`min(balance, exemption)` was considered and is not available anyway: a
zero-data account left below `Rent::minimum_balance(0)` is rent-paying and the
runtime rejects the transaction, and refusing an over-funded seal would hand a
griefer a one-lamport permanent re-strand (`seal.rs:244-262`).

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

**E3's half of that happened, and went further than "chartered" — 2026-08-31
(LEDGER-TRUE).** CloseSeal was ruled, built, and landed the same night
(`f253c4e0`, five real-ELF cases `e8668b52`), so the unblocking this line
anticipates is real and can be spent: the product-graph seal R-3 proposes
inherits both the beneficiary rule (**the closer**, no Market's funding) and a
constraint this line did not foresee — the **profile gate**. Because
`SealedDescriptorClosureV1::decode` admits only artifact profile 1, a future
seal class carrying a bounty or an escrow refuses at this close until it
defines its own, so the ruling cannot silently pay out lamports a later class
means for someone else. See `docs/design/TRUST_RATCHET_V1.md` §9.2, whose
requested ordering — P-006 before R-3 — was honoured.
