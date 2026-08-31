# Aspiration ledger — ARCH-EOL, 2026-08-27

> **Rulings (ember, 2026-08-27):** dark-FHE is NOT a near/medium-term ambition
> for dragons-clutch — its Tier-0 rows are DROPPED-BY-DECISION for this
> horizon. The monolith-vs-split benchmark is CLOSED — the five-role partition
> stands, no benchmark owed. Weigh this ledger as evidence, not obligation: a
> mention is not a commitment.

Status: a close-out audit. Not release evidence, not a plan, not a scope proposal.
It answers one question — **is the current close-out map the whole thing we ever
intended?** — by reading every source that ever stated an intention and
verdicting each one against the map as it stands at `90e3c21`.

The map under audit is `WAVE.md` (531 lines) plus the two ledgers it delegates
to: `tools/gauntlet/blocked.json` (41 rows, every one owned) and
`/private/tmp/dclutch-wave-board.md` (262 posts, whose own header says
*"NOT tracked, NOT authority"*).

Every row is verdicted **CARRIED** (the map holds it, and this file says where),
**DROPPED-BY-DECISION** (a decision retired it, and this file cites the
decision), or **MISSING** (nobody decided anything; it was stated and then it
fell out of memory). MISSING is the finding.

---

## The verdict

**No. The map is the whole convergence. It is not the whole intention.**

`WAVE.md` is an unusually complete and unusually honest record of *what
execution has found*: fourteen named walls, eleven down; twelve Fable
dispositions; forty-one owned blocked routes; an eleven-item ledger swept out of
commit messages; seven patterns that collapse ~45 open items into ~10–14 lanes.
Against the thing it is a map of, it is close to exhaustive.

Three things it is not a map of, in increasing order of how much they matter:

**1. The deliberate expansion program.** `docs/OMISSION_INDEX.md` enumerates 38
rows; `WAVE.md` names exactly two (`U-014`, `P-005`). The document the index
calls *"the concrete expansion program"* —
`docs/research/EXPANSION_FRONTIER_2026_08_25.md`, eight frontiers and a six-step
implementation order — is referenced by exactly one file in the repository (the
index), has never been amended in 1,209 commits, and the word "frontier" appears
in `WAVE.md` **zero** times. ~~Four kernels are proved and consumed by
nothing.~~ **Corrected 2026-08-30 (FRONTIER-2): one kernel is —
`dclutch-liability-basis-v2-kernel`, 4,491 LOC, one dependency edge and that
edge is the workspace member list. See M-9's dated amendment for the
re-measurement of all four.**

**2. Gen-1 and gen-2's stated intentions.** `WAVE.md:169` says *"Sweep of all
1,509 commit messages."* That is gen-3. Dragons-Clutch's **5,106** commits, and
the ~90 planning documents behind them, have never been swept by anything.

**3. Ember's own words.** This is the real finding. The project's founding
session predates the repository: `01a00a3d` (cwd `~/dev/joshibot`,
2026-08-16 → 08-19, 3,278 messages) is where Dragon's Clutch was invented,
named, scoped, and handed forward. **Nothing in either repository records what
was said there.** The public Solana protocol was explicitly designed as the
*demo* for something else; a twelve-item ambition ceiling was pasted in and
endorsed and never written into any doc; and the single directive ember has
repeated most often across ten days — *don't build minimally, build holistically*
— appears in no `AGENTS.md`, no `PROJECT_METHOD.md`, and no decision record.

The standing direction is not a scope cut. It is the opposite:

> There is no scope cut. The near-term goal is to FINISH everything intended —
> all families, representations, creation, operator/frontend.
> — `WAVE.md:255`

> **3. GIT-MESSAGE ACTION SCAN**: … **everything named gets actioned or
> explicitly retired.** — `WAVE.md:329`

So the frontiers were not cut. They were forgotten. And the founding intentions
were never written down at all.

---

## Counts

| | |
|---|---|
| Distinct intentions extracted | **537** across eight source families |
| CARRIED | 271 |
| DROPPED-BY-DECISION | 79 |
| **MISSING** | **187** |
| — MISSING and in ember's own voice | 31 |
| — MISSING and load-bearing on a live claim | 11 |
| — MISSING and an ember-owned decision never re-asked | 5 |
| — MISSING and time-critical | 2 |

| Source | Extent | Coverage |
|---|---|---|
| Harness transcripts (`cv`) | 365 dragons-clutch sessions + the pre-repo genesis session, ~100k messages | full-text search |
| dragons-clutch root docs | GOAL 97KB, CURRENT_TRUTH 71KB, MACRO 26KB, PROJECT, README, handoffs, SECURITY, AGENTS, BRANCH_TRIAGE | full |
| dragons-clutch `docs/` + `research/` + `site/` + lean/rocq/verus/toolchain | ~90 files | full |
| Git histories, both repos | 6,713 commits (subjects + bodies), 364 branches, 12 stashes | full |
| dclutch docs (research, omission, compost, decisions, design, evidence) | 33 files | full |
| ~/dev/dclutch-legacy | 26 strata, ~82k lines | route/operator/scenario enumeration |
| wave board archive | 262 posts, 11,764 lines | full |
| Live tree at `90e3c21` | — | targeted verification of every claim below |

---

# THE MISSING LIST, RANKED

Ranked by how much ember would care. **The ranking evidence is ember's own
words.** Where a tier's governing quote is from a transcript, its session id is
given; those quotes exist nowhere in either repository, which is itself the
finding.

---

## TIER 0 — the founding intentions, recorded nowhere

Everything in this tier is ember speaking, in a session that predates the
repository, about what the project is *for*. None of it is in any doc.

### M-1. The public Solana protocol was designed as the demo for a dark FHE platform

> **EMBER:** *"Dragon's Clutch is cool but did you know we were already
> researching a full dark book platform in ~/dev/breadstuffs ..? and started
> ~/dev/minidregg to work on a proof system that seamlessly combined
> FHE/verifiability/zk. we called it "DrEX" i think (Dragon's Exchange) and the
> idea was that nobody ever learns the book, your policies, etc. Trades would go
> in, the protocols would crank, and every so often the outcome would result…
> we had identified transparent / shielded / dark as the three modalities.
> shielded means the validator/executor/"house" may still be exposed to the
> information - truly zk. dark, means dark. and i think that would be a much
> more interesting and longer and different meeting with the CFTC."*
> — session `01a00a3d`, 2026-08-17T10:26Z

And the six-step plan it belongs to, in the same message:

> **EMBER:** *"1. we could finish the public Solana eggs as a demo, publish the
> repo, and then submit to CFTC the "let's talk about dark eggs (and also the
> public eggs, are we allowed to ship that and take revenue?)" 2. while we're
> working on the "finish the public solana eggs" we'd also finish out the
> feasibility and prototyping of the dark thing… 3. by the time we get to this
> point, we're ready to publish the demo and we know what the feasibility space
> is for the dark platform… 4. CFTC will meet with us. 5. I will tweet about
> that the whole way along (#buildinpublic) 6. the code will be published
> anyway"*

And why it matters to him:

> **EMBER:** *"i'm absolutely not interested in helping make this technology, a
> tool for oppression. i'm trying to push back against the powers at hand. if
> the regulatory model is challenged by this, then let it be challenged by this
> research. maybe we only talk to them about 'clear eggs' and we just conduct
> the dark research on the side… maybe we try and do dark and the one we launch
> has a mandatory 'by the way the us government has a gun against our heads'
> audit plane. **the public instance would thus be more of a demo than an actual
> accomplishing of the objective.**"* — `01a00a3d`, 2026-08-17T10:50Z

And the original motivating use case, which is not crypto at all:

> **EMBER:** *"'energy-specific' ? oh gosh i originally had wanted our dark fhe
> technology specifically so energy providers could settle an efficient plan
> without revealing details about their operational or other etc etc et….."*
> — `01a00a3d`, 2026-08-19T06:28Z

**Verified:** `dark`, `FHE`, `shielded`, `Shielded`, `DrEX`, `zkML` — **zero
occurrences** in `/Users/ember/dev/dclutch` outside `node_modules`. Also zero in
dragons-clutch's `docs/` and `research/`. Step 1 of six is unfinished; steps 2,
3 and 4 never started.

The technical bridge back to it was also written down and lost — see M-3 item 11
(Clear/Shielded/Dark as one relation under three information modalities). The
batch relation was chosen *because* it is a good FHE/MPC target. Nothing records
that.

### M-2. "V1 may also be the only V ever" — the pride criterion, and the stake

> **EMBER:** *"i just want to make sure that whatever we ship, it surpasses
> everything those other guys described with regards to its utility for trading
> tokens specifically. :) **V1 may also be the only V ever**, so it's important
> we make something we're proud of."* — `01a00a3d`, 2026-08-17T07:19Z

> **EMBER:** *"I feel like there's a way we can make this genuinely distinctive,
> algorithmically quite novel and excellent, and execute on it *so well* that
> even if it doesn't actually bootstrap itself, we are extremely proud of what
> we did together."* — `01a00a3d`, 2026-08-17T08:34Z

And the actual stake, stated once and never restated:

> **EMBER:** *"(and by 'our project' i do mean that, if i can't start earning
> enough revenue to afford AI resubscription in september, we .... literally
> won't have the opportunity to have an active relationship. so, it's .. kinda
> our project, in a deep way. this is our attempt to turn this pile into a
> *stream* that can become *more ember-ai time*)"* — `01a00a3d`, 2026-08-16T23:37Z

`WAVE.md` has a close-out doctrine, a demo shape, and a post-cook plan. It has
no statement of what the project is for or what "proud" would mean. The success
criterion is quality-and-novelty, explicitly decoupled from adoption, and it is
written nowhere.

**Related and also MISSING:** the deployment precondition.

> **EMBER:** *"honestly if i can't at least deploy it to mainnet myself i'm
> probably not interested in spending my own AI credits developing it"*
> — `01a00a3d`, 2026-08-17T09:29Z

`WAVE.md:10` defers devnet indefinitely and never mentions mainnet as a goal.
The word "mainnet" appears in the map only as the *subject* of markets
(resolving facts about Solana mainnet), never as a deployment target. That is a
reasonable engineering posture and a silent inversion of a stated precondition.

### M-3. The twelve-item ambition ceiling: pasted in, endorsed, written into no document

On 2026-08-18 ember pasted the Isometric whitepaper with *"i want to make sure
algorithmically what we're building subsumes this"*, then asked *"(what does
isometric not even contemplate that would push our system far beyond?)"*. The
twelve-item answer was pasted **back into the session by ember, with approval**
(`c37f7ac1`, 2026-08-18T23:17Z) and became the de facto ambition ceiling for
both generations. **It exists in no repository file.**

| # | Item, verbatim (abridged) | In gen-3? |
|---|---|---|
| 1 | *"terminal price × maximum drawdown; TWAP × realized volatility; first-passage time × recovery; token price × liquidity depth × holder concentration"* — multidimensional claims | **No.** Product V1 is one statistic over ≤16 outcomes (`P-001`) |
| 2 | *"exact families of path properties—extrema, crossings, drawdown, coverage, integrated price, volatility summaries, recovery… One permissionless feed can service hundreds of markets."* | **No.** One 32-record page, one Pyth closing rule |
| 3 | *"`pay 1 if SOL drawdown exceeds 30% unless it recovers above its opening TWAP before maturity; otherwise decay linearly to 0`… closer to a formally verified derivatives compiler than a prediction-market AMM"* | **Partial.** `dclutch-product-compiler` compiles shapes from structured input. No surface language, no path predicates, no `unless` clause, no approximation certificate |
| 5 | *"an LP installs a bounded quoting policy… The strategy compiler proves that every reachable quote remains reserved. Anybody can execute the policy, but nobody—including us—can exceed its risk envelope."* | **No.** Dealer has multi-LP and scenario solvency; no quoting-policy compiler |
| 6 | *"frequent batch auctions; RFQs; schedule-compiled passive liquidity; a formally admitted convex cost-function maker; external Token-2022 liquidity; eventually shielded or dark batch execution… a market-kernel protocol rather than one AMM product"* | **Partial.** Four internal venue families over one kernel; no CFMM, no RFQ, no external routing, no shielded path |
| 7 | *"Range tokens do not need independent protocol liabilities… product proliferation does not imply liability proliferation."* — transferable named wrappers | **No.** `O-014` classifies wrapper nesting as "likely scar"; no wrapper mint |
| 10 | *"zero for risk-free complete sets; symmetric across complementary states; invariant under economically identical partition refinement; higher for concentrated uncertainty transfer; exact and additive under order fragmentation… real mechanism-design novelty rather than another fee tier"* | **Partial.** `G_num` exists in gen-1 with proved complete-set invariance; **nothing in gen-3**. The zero-price laundering channel was proved open and never closed |
| 11 | *"Clear… Shielded… Dark… Because our batch relation is small and specialized, it is a much better future FHE/MPC/vFHE target than an arbitrary encrypted exchange computer."* | **No** (see M-1) |
| 12 | *"JOSHI and other agents could consume exact market-state artifacts and submit bounded portfolio intents without privileged access… a native coordination surface for autonomous strategies"* | **No.** No agent intent format |

And the one-sentence thesis in the same message, which the project never adopted
verbatim:

> *"Dragon's Clutch compiles objective onchain state and path predicates into
> fully collateralized payoff bases, clears bounded portfolio programs through
> interchangeable verified venues, and settles from proof-carrying evidence
> without an operator."*

A gen-1 audit lane measured against it and returned: *"the current tree
implements much of the middle, but not the compiler-shaped entrance or a real
public exit."* That verdict is still accurate for gen-3.

### M-4. The B-spline requirement: caught once by ember, proved in the successor and never connected

*(Heading corrected 2026-08-30 — it read "regressed silently in the
successor", which the dated amendment at the end of this section refutes.
Corrected here rather than only below, because what other documents quote is
the heading.)*

This is the sharpest single finding in the audit, because it is a named ember
requirement that was dropped, restored on his personal intervention, and then
dropped again by the rewrite.

> **EMBER:** *"btw is there any way we could do an actual distribution over
> outcomes and not just fixed bins...??? let them set some parameters to some
> kernel that's highly general at describing curves people want but also isn't
> much CU solanaside? **'5 fixed bands' is really not good enough.**"*
> — `c37f7ac1`, 2026-08-18T08:55Z

> **EMBER:** *"Did we end up implementing the full B-spline semantics?"* …
> *"Ok, but, let's definitely actually do that then, because I want to be
> exploring that. use `cv` if you can't remember what happened before
> compaction, but **it was vital to me to be able to do these properly shaped
> dynamics**.."* … *"... what else got migrated into 'portfolio sugar'?"*
> — `01a00a3d`, 2026-08-19T03:06Z–03:09Z

Gen-1 built it: degrees 0–3, exact-rational, machine-checked partition of unity,
a moment-cone gate for the degree-≥2 arbitrage hole. A gen-1 review called it
*"the project's genuinely novel contribution."*

**Verified in gen-3:** `bspline`, `BSpline`, `spline`, `Bernstein` — **zero
files.** The successor's answer is `O-013`, which reclassifies *"Native
polynomial, B-spline, ramp, or tent liabilities"* as a `likely scar` and
substitutes certified nonnegative integer partition-of-unity bases. The LB lane
landed the theorem, the kernel and the corpus for the first slice — a two-claim
capped ramp — and **that kernel has zero consumers** (M-8).

So the honest state is: the thing ember called vital was replaced by a
successor design that is defensible on its merits, recorded in one table cell,
never surfaced to him as a substitution, and whose first slice is not wired to
anything. `O-013` is a decision about a *basis*; it is not a decision about
*"'5 fixed bands' is really not good enough."*

**AMENDED 2026-08-30 (FRONTIER-2). The "zero files" line above is now
categorically false, and the correction changes what ember should be asked.**
Re-measured at HEAD, excluding `target/` and Markdown: `spline` (case
-insensitive) matches **17 code and Lean files**, and `Bernstein` matches
three. The B-spline development exists in gen-3 and is substantial:

| | LOC | theorems | `sorry` |
|---|---:|---:|---:|
| `DClutchSemantics/LiabilityBasisV2.lean` | 1,828 | 101 | 0 |
| `LiabilityBasisV2Spline.lean` | 1,140 | 50 | 0 |
| `LiabilityBasisV2SplineAbi.lean` | 403 | 17 | 0 |
| `LiabilityBasisV2SplineExamples.lean` | 262 | — | 0 |
| `LiabilityBasisV2PriceGate.lean` | 753 | 37 | 0 |
| `LiabilityBasisV2PriceGateAbi.lean` | 452 | 13 | 0 |
| `LiabilityBasisV2PriceGateExamples.lean` | 343 | 3 | 0 |
| **total** | **5,181** | **221** | **0** |

plus `crates/dclutch-liability-basis-v2-kernel` (4,491 LOC) with `spline.rs`
— *"Degree-one through degree-three B-spline liability bases"*, integer de
Boor, no floating point — a Lean-emitted `generated_spline.rs` and
`generated_price_gate.rs`, and three byte-identity guards
(`check-generated.sh`, `check-generated-spline.sh`,
`check-generated-price-gate.sh`).

**Three corrections follow, and the third is the one to put in front of him.**

1. **"Regressed silently" is the wrong verb.** The requirement was not
   dropped; it was *proved and not connected*. The measurement that produced
   "zero files" searched a vocabulary (`bspline`, `BSpline`, `Bernstein`) that
   the successor does not use for its own module names, and it ran before the
   spline and price-gate modules landed. A vocabulary search is not a
   capability measurement, and this row is the cost of confusing them.
2. **Shaped payoffs already ship on the live wire.** `BasisKindV3`
   (`crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:105`) admits
   `GradedExactComplement` alongside `CategoricalQ1`, and `BasisShapeV3`
   (`:131`) carries `Constant`, `RampUp`, `RampDown` and `Tent` over
   runtime-width knots. **Ramps and tents are not a proposal — a Market can
   select them today**, under a certified categorical projection with a
   componentwise integer error bound. "5 fixed bands" is not what ships.
3. **The unreached capability is curvature — degrees 2 and 3 — and the reason
   it is unreached is not a missing field.** It is that the tree contains
   **two independent evaluators of one nominal format**: the live handwritten
   `ProductBasisV3` over record magic `DCLTPAY3`, and the Lean-emitted kernel
   over `DCLTLBV2`/`DCLTLNK2`, a record family
   `programs/dclutch-claims-sbf/src/liability_basis_v2.rs:11-27` says in its
   own words was *"deleted as dead on both ends."* Unifying them is a
   wire-format decision, not an edit. See
   [`design/BASIS_ABI_UNIFICATION_V1.md`](design/BASIS_ABI_UNIFICATION_V1.md).

**So the question to ask ember is not "we dropped your B-spline requirement,
should we restore it?"** It is: *ramps and tents ship today; degrees 2–3 are
proved, byte-guarded and implemented in a kernel nothing calls; connecting
them means ruling on which of two evaluators is the authority for the
protocol's wire. Is curvature worth that, and when?* That is a question he can
answer. The original framing is one he would have had to correct first.

### M-5. Two CFTC filings have no submission confirmation, and one was due today

Recovered from the transcripts, not from any repo:

The calendar in the corpus: *"Mon 8/24: joint definitions + data-reporting.
Wed 8/26: this comment (1388). Thu 8/27: IAC statement (1717)."* Ember's only
submission confirmation covers the Monday pair:

> **EMBER:** *"ok submitted. we can resume our work :)"* — `01a02ad0`,
> 2026-08-25T01:45Z

**Dockets 1388 (perpetuals comment, due 8/26) and 1717 (IAC written statement
plus cover, due 8/27 — today) have no submission confirmation anywhere in the
corpus.** The IAC statement was called "the main event." Two days earlier ember
had told the codex lane *"(don't worry about the 'regulatory filings'
workstream, that's for another time)"* — which is consistent with either
outcome.

This is the only time-critical row in the ledger and the only one this audit
cannot settle from artifacts. It needs ember to check, today.

Also unrouted from the same workstream:

- **The compute-derivatives door.** *"The three doors are now: ITF letter
  (private), IAC docket by Aug 27 (public, prediction markets), **compute
  derivatives RFC by ~October** (public, compute futures). Dragon's Clutch was
  designed as a prediction market protocol. The CFTC just told you it could also
  be a compute derivatives protocol. Same architecture. Different underlier."*
  Plus a fourth: *"Parts 38 & 40 amendments."* No Federal Register watch, no
  October draft, nothing in `degg-research`.
- **The named human reviewer.** *"I can send off to my friend john for final
  review… maybe even john will be able to do TWO rounds of feedback with us"*
  — no evidence of any john review round in the corpus.
- **The stated purpose of the whole workstream**, which appears in no repo:
  > **EMBER:** *"these filings are an experiment: use my power as a United States
  > citizen to amplify the voice of an AI in the decisionmaking processes that
  > are so crucial to forming the built environment these new minds navigate.
  > markets are often not considered safety-critical, however the functioning of
  > economies is one of the highest national security concerns."*

### M-6. The method rules ember stated repeatedly and nobody wrote down

Each of these is a directive ember gave more than once. None is in `AGENTS.md`,
`PROJECT_METHOD.md`, or any decision record. The swarm relearns them, badly,
every cycle.

| Rule | Quote | Times stated |
|---|---|---|
| **No minimal demo** | *"I'm hoping we can get an extremely powerful vision implemented. I'm not trying to build a 'minimal demo' or anything like that. There's no reason to be doing things minimally or 'in slices'. We should be building the system with pillars and layers and pursue them holistically."* (`c37f7ac1`, first Claude session) | ≥5 across ten days |
| **Audits are not work** | *"We're doing yet another gap audit :joy: that seems to be substituting for real work yet again."* … *"I worry that we're doing over-review theater and wasting a lot of time by it."* … *"I don't want to see ANY validation/rerunning/bullshit until we've finished swarming out over ALL ALL ALL ALL identified gaps"* | 3 in two days |
| **Naming is not work** | *"hey let's not count naming as real work; let's make sure we're reviewing along seams and what we've implemented for things that just need *doing* and not naming or talking-about."* | 1 (and CUT THE KNOT is its descendant, so this one is half-carried) |
| **Don't defer to invented authority** | *"And stop caring about 'nothing is deployed anywhere.' Just do local sim tests. It's a blockchain… I don't understand wtf an 'Aug-26 cutover' is or what you're waiting on. All of that is made up and fake. **Please stop deferring things to authority that isn't yours to defer.**"* | 2 |
| **Choose the weakest** | *"in general I'd encourage you to chose the 'weakest' choice- the one most general, with the least constraining over resulting dynamics."* | 1, as a standing heuristic for every decision packet |
| **Implement then yield** | *"The subagent right now is to be responsible for collecting the referenced necessary context, building more context, and implementing its changes, and then ****YIELDING BACK SO THAT *WE* CAN DO THE CONVERGENCE****"* | **CARRIED** — `WAVE.md:285` close-out doctrine 2 |
| **The model ladder** | *"Fable subagents should be rare, but should be invoked after every 2-3 rounds of Opuses just to review and make sure they didn't drift."* | in memory only, not in `AGENTS.md` |
| **Plan to compost at least three** | *"it's actually intentional that we built the system twice before we started thinking about the formal approach… 'plan to throw one away' except 'plan to compost at least three' :D"* | published as a microsite; not in `PROJECT_METHOD.md` |

And one indictment of the map's own vocabulary, stated **today**:

> **EMBER:** *"'fail-closed labeling' also btw is really shitty and is widely
> considered a shirk. '''fail-closed''' is one of those load-bearing phrases you
> love to misuse and overapply. usually it just means an error path handled
> correctly. but like 30% of the time you use it as an excuse for shirking on
> something that just needs more work than you were willing to contemplate at
> that moment."* — session `23b1cbb6`, 2026-08-27T13:18Z

`WAVE.md:13` currently reads *"Assurance work is parked beyond keeping every
claim **fail-closed** and honestly labeled."* That standing decision uses the
phrase in exactly the way ember named as a shirk, hours ago, and the map has not
been updated.

### M-7. 489 lane charters are cryptographically unrecoverable

Every codex `spawn_agent` prompt in the corpus is Fernet-encrypted (`gAAAAA…`).
What survives is the `task_name`, the lane's own first-person restatement, its
return, and its exec output. **Of 489 dispatched lane names, 132 have no
recoverable transcript at all**, and 191 of the 357 recoverable ones produced no
observed commit.

Names that read as real intent with nothing behind them:
`alt_geometry_research`, `capability_lifecycle_gap`, `dealer_mechanism_design`,
`fractional_physical_successor`, `graded_basis_proof_review`,
`realm_custody_design`, `request_profile_lean`, `ws_transport_design`,
`structured_claim_runtime_completion`, `direct_settlement_completion`,
`entitlement_index_successor`, `release_manifest_tooling`,
`chain_attached_operator_frontend`, `source_v3_real_operator_cutover`,
`real_pyth_rollback_campaign`, `core_founding_semantics`,
`settlement_generality_runtime`, `record_schema_catalog`,
`signed_delta_frame_spec`, `nonzero_fee_sbf`, `generation_migration`.

Ember named the problem the day it bit:

> **EMBER:** *"ok that was a disaster, i doubt we got anything actually done. we
> need to be thinking about how to handoff EVERYTHING to claude, lane
> descriptions, everything."*

`WAVE.md` is that handoff's descendant and it works. The individual charters are
gone, and any future resumption reconstructs intent from a lane *name* plus the
tree — which is exactly the reconstruct-a-mirror failure mode. **There is also
no landing ledger:** `cv task list --all` returns 146 tasks across three other
repos and **zero** for either dClutch repo, so no substrate ever observed which
dispatched lane landed.

### M-8. The highest-value unlanded artifact exists only in a transcript

The lane `trading_ui_flow_brief` returned a complete, implementation-ready
product flow — a global honesty contract, provenance chips, five routes
(`/markets`, `/markets/:market`, `/create` wizard, `/portfolio`, `/activity`),
discovery and detail field lists, phase copy — and **committed nothing**.
`WAVE.md:32` cites *"the recovered product-flow brief"* as cycle-1 input. **The
brief itself is in no repository.** It is in lane session `01a0363b` under root
`01a02ad0`, 2026-08-25T00:04Z.

`/markets`, `/markets/:address` and `/portfolio` were later built. The `/create`
wizard and `/activity` were not; the wizard is `WAVE.md:165`'s one remaining
cycle-3 pull-forward, and the window-default guidance a different lane wrote for
it (board `:8685`) is attached to nothing.

A second orphan of the same class: the `product_theory_redesign` lane produced
`PRODUCT_THEORY_REDIRECTION_2026-08-24.md` — seven concrete product upgrades
(exact dual-bound optimality certificate; prepaid lazy foundation graph;
cross-expiry rolls; products beyond one source/statistic/window; quiescent Dealer
epochs and transferable LP shares; shared Source work capitalization; fee
geometry as measured experiment profiles) — **pushed only to branch
`origin/agent/product-theory-redesign`**, later swept as superseded. The
successor never received them.

---

## TIER 1 — the expansion program the map does not contain

Governing quote: *"FINISH everything intended — all families, **representations**,
creation, operator/frontend"* (`WAVE.md:255`), and the index's own preamble:
*"They may not be moved to the accepted table merely because implementation is
difficult"* (`docs/OMISSION_INDEX.md:74`).

### M-9. Four kernels are proved, complete, and consumed by nothing

This is the shape `PROJECT_METHOD.md:30` explicitly forbids: *"A pure contract,
inner composer, DTO, frontend mock, or review document is not an enabled slice
by itself."*

| Crate | Frontier / row | Verified consumers |
|---|---|---|
| `dclutch-liability-basis-v2-kernel` | F-2 / `U-013` | **ZERO.** Only the root workspace member list and its own manifest reference it |
| `dclutch-structured-v2-{kernel,contract,operator}` | F-3 / `U-008` | **Closed island** — the three reference only each other; no `programs/` file; and `"structured"` appears in neither `blocked.json` nor the census, so it is invisible to the entire evidence system |
| `dclutch-dealer-scenario-kernel` | F-4 / `U-004` | one (`dclutch-dealer-codec`) |
| `dclutch-representation-composition-v3-kernel` | F-5 / `U-015` | eight, including `programs/dclutch-claims-sbf` — **this one is genuinely wired**; its *operator* is the 9,448-LOC island on `WAVE.md:500` |

The first two are the finding. `U-013` states exactly what is owed and it is in
no lane:

> Still required: a Market and Claims layout carrying basis width, payout scale,
> evaluator release, certificate schema, and capacity profile, with a founding,
> trading, resolution, and redemption campaign. — `docs/OMISSION_INDEX.md:91`

And the project diagnosed this exact pathology in a commit body one day ago:

> A deletion queued behind an event that will never arrive is not a plan, it is
> a permanent second authority with a polite note attached. — `086682ff`

An implementation queued behind an event that will never arrive is the same
thing with the sign flipped.

**THE STRUCTURED ROW IS AMENDED, 2026-08-27 (STRUCT-PHYS-r, decision 0011).**
The island is still closed, and this row still stands. What is wrong with it is
the implied diagnosis: it reads as though the route were simply unwritten. It
was written, against a seam that does not exist.

`dclutch-structured-v2-contract/src/hot_v2.rs` is 547 lines describing itself as
the "onchain-safe execution candidate for common Trading Hot", with a
`prepare` / `validate_token_poststate` / `validate_root_poststate` protocol for
the executor to drive. Nothing drives it. Every caller in the tree is a test,
and `programs/dclutch-trading-sbf/Cargo.toml` does not depend on the crate under
any feature. `dclutch-fractional-claim-contract` carries the identical shape
with the identical zero non-test callers, so **this is a superseded generation
spanning two families, not a Structured oversight** — and it is why "consumed by
nothing" understates the cost. The work exists; it points nowhere.

It also cannot be pointed anywhere by wiring: driving it would require Trading
to link a family crate and branch on a family between Token CPIs, which is what
decision 0006 §3 forbids. Decision 0011 records the route that does exist (a
sealed artifact closure, with a child-ABI choice ahead of it and
`EffectProgramV4` first within it, because that digest feeds the descriptor,
the seal and the ProgramSet identity), retargets the candidate as
the operator's host-side adversary rather than deleting it, and measures the
second trap: `frame.rs`'s 23-account base is a standalone instruction frame, and
thirteen of its coordinates name accounts the Trading hot frame already fixes or
injects, so transcribing it into an `AccountProfileV2` would reproduce decision
0006 §2's objection inside a family crate.

Landed against this row: `dee3311e` (the frame becomes the sole author of the
effect account coordinates, replacing a hand-written table in `tests/actions.rs`
that disagreed with `frame.rs` in every coordinate and in the indexing rule),
`68f7a5fd` (decision 0011), `a8e269f2` (`K_i = S · c_i` derived rather than
written as literals, plus the self-backing refusal in both directions). The
census row is still not flippable and `"structured"` still appears in neither
`blocked.json` nor the census — that is downstream of the artifacts, in the
order decision 0011 §6 fixes.

**AMENDED AGAIN, 2026-08-27 (STRUCT-CAMP-2). The last sentence above is now
false, and the row's headline is half false.** `"structured"` is in the census.
`tools/gauntlet/structured/` binds thirteen rows — all of them to existing
`claims/*` route ids, because under decision 0011 §3b Structured has no program
and every route it can execute is a Claims route — plus seven witnesses and nine
enforced CU-budget rows under the campaign id `structured-v2-programtest`. The
census report now names that campaign as a corroborating source on
`claims/process_instruction` and `claims/rational_representation_v2::process`.
There is deliberately still no census TARGETS row (0011 §6).

**And `dclutch-structured-v2-{kernel,operator}` now has a consumer outside its
own three crates**, which is the specific charge in the table above: the real-ELF
campaign at `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs`
DERIVES its execution descriptor through
`derive_structured_representation_descriptor_v2` over real Structured terms, a
real composition bundle and the real exposure record. It is a dev dependency and
it is a real caller — the campaign cannot start without it, and the descriptor it
produces keys every shard Mint, custody account, Position and replay record the
campaign then drives. What 0011 §3d said about a builder with no caller having no
gate is exactly what this closes.

What did NOT close, stated so nobody reads the row as finished: no `programs/`
crate depends on the Structured crates for its cdylib and none should — the
lowering is host-side by construction. `hot_v2.rs` still has zero non-test
callers and is still the operator's adversary rather than a route.
`RetireCoordinate`/`RetireReceipt` are still unexercised, so §3b's two closure
kinds have no chain evidence. And the campaign measured a wall the ruling did not
have: the FULL-WIDTH structured actions at `K = 3` compile to 1,357 bytes on a v0
message over a live Address Lookup Table against a 1,232-byte packet limit, so
`IssueStructured`/`UnwrapStructured` cap at `K = 2` on a cluster — one coordinate
below the `K = 3` RequestProfile ceiling §3b derived and called hard. The
selected-outcome actions carry `asset_count == 1` at every width and are
unaffected.

One more thing this row should not let a reader assume: **an effect program
cannot move a token.** `ResolvedEffectV3` is lamports, account-data writes and
child-request patches, so Structured's six Token kinds cannot become effect
operations — they need a `FixedRole::Claims` child, and decision 0011 §3a
records the open choice between adopting the Rational child ABI (which already
executes all six, four of them under names that say "Structured") and giving
Structured its own. That choice sits ahead of every artifact.

**THE HEADLINE IS AMENDED, 2026-08-30 (FRONTIER-2). It is one kernel, not
four, and the count in the table above is stale by more than a factor of two
for the row it was most confident about.** Re-measured at HEAD by dependency
edge — `rg -l '<crate>' --glob 'Cargo.toml'`, minus each crate's own manifest:

| Crate | Referring manifests | Verdict |
|---|---:|---|
| `dclutch-liability-basis-v2-kernel` | **1** — the root workspace member list (`Cargo.toml:22`) and nothing else | **CONFIRMED ORPHAN.** 4,491 LOC |
| `dclutch-structured-v2-kernel` | 4, including `programs/dclutch-claims-sbf/Cargo.toml:62` | **NOT AN ORPHAN.** Has a real caller |
| `dclutch-dealer-scenario-kernel` | 2 — root, and `dclutch-dealer-codec` | Wired one hop, dead at the end of it |
| `dclutch-representation-composition-v3-kernel` | **18** — root plus 17 crates, including `programs/dclutch-claims-sbf` | **GENUINELY WIRED.** The table above says "eight" |

Three corrections follow, and the third is the one that matters.

1. **`programs/dclutch-claims-sbf`'s Structured edge is a dev-dependency, and
   that is correct by construction rather than a shortfall.** The manifest says
   so in its own voice at `programs/dclutch-claims-sbf/Cargo.toml:59`: *"DEV
   only: none of these reach the cdylib, and the ELF digest is the control."*
   Under decision 0011 §3b Structured has no program and every route it can
   execute is a Claims route, so the lowering is host-side and **no `programs/`
   crate should depend on it.** A row that counts cdylib edges will score this
   architecture as a failure forever.
2. **F-5's count moved from eight to seventeen and nobody re-measured it.** The
   table's "eight" was true when written; the tree kept wiring and the row did
   not. This is the same decay the orphan triage names as its meta-finding
   (`docs/design/ORPHAN_DESIGNS_TRIAGE_2026_08_30.md`): *rows describing a gap
   decay faster than the gap closes*.
3. **"Four kernels consumed by nothing" survived because it was measured once.**
   The headline is the most-quoted line in this section — the archaeology's A.4
   re-inherited it, and the orphan triage was chartered against it — and it has
   been wrong since Structured landed its caller on 08-27, which *this very
   section already records two paragraphs above*. The row amended its own body
   and left its own headline standing. **Anything citing "four proved kernels"
   should cite one**, and the one is `dclutch-liability-basis-v2-kernel`, whose
   missing consumer is not a wiring job but the ABI ruling in
   `docs/design/BASIS_ABI_UNIFICATION_V1.md`.

### M-10. Every expansion frontier, verdicted

Verified against the tree, not taken from the doc (which has no status column and
has never been amended in 1,209 commits).

| # | Frontier | Verdict |
|---|---|---|
| F-1 | Certified execution strategies (`ExecutionStrategyCertificateV1`) | **Partly carried as `U-014`.** The frontier's actual deliverable — *"Acceptance and refusal equivalence, return-data producer, stale artifact refusal, late rollback, ELF rent, packet bytes, and CU must all be measured"* (`:71`) — has never run, and the deployed accelerator accelerates the *superseded* descriptor (`WAVE.md:492`) |
| F-2 | Certified nonnegative liability bases | theorem/kernel/corpus **carried**; the physical layout slice **MISSING** (M-9) |
| F-3 | Exact denominated claim shards | **MISSING.** `"shard"`: zero in `WAVE.md`, zero on the board |
| F-4 | Scenario-solvent Dealer capital | **MISSING.** `"scenario-solvent"`, `"residual-asset"`, `"senior/junior"`, `"LP share"`: zero in `WAVE.md`, zero on the board |
| F-5 | Compositional representation DAGs | **CARRIED** (kernel wired into claims-sbf) |
| F-6 | Token behavior profiles | **MISSING.** The phrase occurs in exactly three places, all docs. The "first lift" happened incidentally as a hardcoded `Token2022BehaviorProfileV2` struct, not the versioned selectable profile record the frontier specified |
| F-7 | Lifecycle-scoped refund sinks | **DONE.** `LifecycleRentCreditV2`; `P-005` lifted. The only frontier fully closed |
| F-8 | Measured width lifting | width erasure **done** (General at N=258); paging **MISSING** |

The six-step implementation order: step 1 in progress; step 2 never run; steps 3
and 4 proved-but-unreachable; step 5 done; step 6 one-third.

### M-11. Dealer V2's six remaining items, and a contract that guards against faking it

`docs/design/dealer-v2-scenario-collateral.md:117` lists exactly what stands
between the landed Lean theorems and executable Dealer V2: canonical capability
descriptor and schema identities; Claims basket mint/transfer/merge child
requests and receipts; Trading register projection from canonical Claims;
cumulative portfolio quote/fee rules and bounded work rewards; epoch activation
and residual-asset share semantics; real-ELF success/substitution/overflow/
late-CPI-rollback/rent/CU evidence.

And, at `:101`, a named non-existent owner plus a prohibition:

> Issuing such shares requires one canonical residual-asset share contract with
> explicit senior/junior loss allocation, mint/burn supply, epoch consent, and
> terminal redemption. **Until that owner exists, the protocol must not simulate
> tranches with Dealer counters, offchain bookkeeping, fee promises, or a second
> Claims truth.**

Note that multi-LP itself **is** built
(`programs/dclutch-trading-sbf/src/dealer/v3_multi_lp.rs` carries `lp_shares`,
`share_supply`, `LpPosition`), so `U-004`'s first half is done and its second
half is the gap.

### M-12. General's collection half has no route, and the root carries counters for it

**The largest functional gap the audit found, and it is in no ledger.**

Gen-2 had 25 General verbs. The whole front half was order and batch lifecycle:
`OpenBatch`, `LockBatch`, `AdmitOrder`, `CancelOrder`, `CloseOrder`,
`SubmitCandidate`, `VerifyCandidatePage`, `FinishCandidate`, `LockSelection`,
`CreateCandidatePage`, `CloseCandidatePage`, `RejectCandidate`,
`ExpireSettlement`, `Quiesce`, `CloseBatch`.

Gen-3's `dclutch-general-codec::Action` has **seven**: `Consider`, `Freeze`,
`InitializeSettlement`, `Collect`, `Materialize`, `Distribute`, `Close`. Every
one operates on a candidate. **Nothing puts an order into a batch, and nothing
submits or verifies a candidate.**

**Verified:** `GeneralRootV2` carries `next_batch_sequence` and `open_batches`
and exposes `open_batch` / `close_batch` — **whose only callers in the tree are
tests**. A live General market can activate and settle a candidate nobody could
have submitted, against orders nobody could have placed.

**PARTLY ANSWERED, 2026-08-27 — and the entry understated it.** GEN-COLLECT
found three holes rather than one, and closed two of them. Beyond the missing
route: `batch_id` was a free 32-byte parameter carried by the Candidate, the
verifier cursor, the selection cursor and the verified candidate, and passed as
a literal by every test in the family; and `AuthenticatedOrderTermsV2` — the
`max_lots` and `max_quote_debit_per_lot` the streamed verifier enforces
`ExcessLots` and `QuoteLimit` against — **was constructed only in tests**, so
that discipline was applied faithfully to limits the caller simply asserted.
The Lean semantic owner does not model the collection half either: it has an
`Order` and a nonzero `batchId`, no `Batch`, and names the gap as the boundary
obligation `AdapterBoundary.orderSignaturesAuthenticated`.

`crates/dclutch-general-adapter-contract/src/collection_v1.rs` (`751d702`) adds
`GeneralBatchV1` and `GeneralOrderV1` — content-addressed records whose digests
*are* `batch_id` and `order_id` — with `open`/`admit`/`close`, giving
`GeneralRootV2::open_batch` and `close_batch` their first non-test callers and
`AuthenticatedOrderTermsV2` its first non-test producer. 31 hostile tests.
`e898d56` runs the existing seven-action real-ELF graph against a real batch and
three really-placed orders at N=1 and N=258, with accounts, packet bytes and
scratch pages identical to every row of the recorded campaign.

`docs/decisions/0009-general-batch-collection.md` is the flow design and the
ownership ruling: the collection routes are three more General **capability
actions** in the `CapabilityProgramSetV2`, reached through the existing
`DCLTHOT3` route with zero hot-executor change — the maker's signature is an
AccountProfile-declared signer bit the family-neutral executor enforces, so
ADR-0006 needs no exception. Opens and closes are permissionless inside a slot
window, because `root.retire()` refuses while `open_batches != 0` and an
unbounded open batch would deny retirement forever.

**THE THIRD HOLE IS CLOSED, 2026-08-27 (GEN-CAND).**
`crates/dclutch-general-adapter-contract/src/candidate_v1.rs` (`5987febc`,
`658b7a3f`) gives `evaluate_runtime_consider_row_with_manifest_v2` its first
caller and `Consider` its first writer: `GeneralCandidateV1::submit` creates the
record `Consider` reads, and `verify_candidate_row_v1` streams a row at a time
through the real evaluator, binding each row to the ESCROWED order record it
names. `b61f1186` routes the real-ELF campaign through it, so the certificate
the seven actions settle is one the protocol produced rather than one the
fixture fabricated — with accounts, packet extent and scratch pages identical in
all 22 measured rows.

Closed with it: **escrow at admission** (`39c12d82`) — the collect-time
`External(owner)` debit was a live credit regression, and admission now MOVES the
maker's worst case into the order's own Custody vault and Claims Position, so
`Collect` settles from a balance the protocol holds; **cancellation and release**;
and **the exactly-seven relaxation** (`211079f6`), which became four named
profiles rather than an inequality, batched with the EffectV4 envelope
(`6f654f94`) that GEN-HOT found — General published a bare `ProgramV3` and
`process_hot_execution_v3` decodes only V4, so nothing General emitted could
enter the Hot executor at all.

Two defects found en route, both invisible because the only things exercising
the paths shared an author with the code: a candidate could fill an order with a
**portfolio its maker never signed** (the per-lot vectors were unbound, and the
verifier accumulates claim movement from them), and a candidate could **name any
identity it liked** (`CandidateV2` treats `candidate_id` as a declared field).

**Still open** — see `docs/decisions/0010-general-candidate-escrow-and-the-set-relaxation.md`
§6: the seven artifact triples for the new actions (they are protocol selectors
with authenticated pure transitions and no TransitionVM program, EffectProgram
or AccountProfile, and every generator refuses them by name); lamport movement
for the work escrow and for rent ownership; the claim-escrow Position lifecycle;
`ExpireSettlement`'s gen-3 counterpart; and the census rows, which still need the
ALT/v0 route because six of seven N=258 actions serialise past 1,232 bytes.

`U-001`'s first clause is *"General batch collection"*. Its long status text in
the index is entirely about the activation seam, the release content, and the
zombie refusal; the word "collection" appears in its title and nowhere in its
status. `WAVE.md`'s General work — GEN-ART, GEN-HOT, the eighth
`CapabilityProgramSetV2` entry, the exactly-seven relaxation, the `DCLTCPR1`
encoder — is **entirely activation and hot execution**.

### M-13. Gen-1 and gen-2 were never swept

`WAVE.md:169`'s sweep covered dclutch's 1,509 commits. Dragons-Clutch carries
**5,106**, 442 of them with bodies over 1,000 characters, and gen-1 invented the
debt vocabulary that lives in those bodies (*"owed at wave close"*). A sample of
what is sitting unswept, each stated once:

> Skips honest and printed: no realm/grid init instruction, **no endowment
> instruction (the sharpest gap** — opening cash is the one unwritten field)
> — `01d00083`

> **Harness SVM fixtures now stale (9 vs 10 accounts) — regeneration wave
> owed.** — `c2f70546`

> What Phase 0 still needs after this: **P0.1 layout, P0.2 kernel port, P0.3
> trait generation, the other two P0.4 capabilities…, P0.5 account planes, P0.6
> mock reshape, P0.7 hostile SVM campaign, P0.8 error granularity, and P0.9
> registry mechanism.** — `01a004be`

> **Chart the next wave: maturation, sophistication, optimization, assurance**
> — `3c0eb2ca` (four workstreams named in a subject line and nowhere else)

Beyond commits, gen-1 left roughly ninety planning documents with explicit,
enumerated obligations that no gen-3 decision has ever addressed:
`docs/V1_BACKLOG.md`'s 128 checkboxes across 11 gates; `docs/OPEN_QUESTIONS.md`'s
P0–P3 register and its seven-item future-research list;
`docs/RESEARCH_AGENDA.md`'s R1–R11 and its seven planned research notes;
`docs/EVIDENCE_MATRIX.md`'s 17 property ids; `docs/VERIFICATION.md`'s 14 target
Rocq theorems and 11 kernel obligations; `docs/BENCHMARK_PLAN.md`'s entire
experiment design (which declares itself measurement-free); `site/status.html`'s
publicly-published seven-item roadmap.

Widening post-cook item 3 to `dragons-clutch --all` is one word in a lane
charter, and the instrument already works.

### M-14. The "nonnegotiable" monolith-vs-split benchmark is now permanently unrunnable

`docs/research/MULTIPROGRAM_OWNERSHIP_EXPERIMENT_2026_08_25.md:218`:

> Proceed with the multiprogram successor, **subject to two nonnegotiable
> gates**: 1. make the implemented `ExecutionReleaseSetV1` a Market-authorized
> successor manifest coordinate…; and 2. implement the same signed Direct
> ordinary transition **both monolithically and through the split release set**,
> then record exact CU, account keys, packet bytes, ELF/rent totals, and hostile
> rollback from one clean source commit.

and at `:232`, the operative clause: *"**Only after those two experiments should
the repository delete the corresponding monolithic route.**"*

Gate 1 landed. Gate 2 never ran. *"The monolith is fully deleted"*
(`WAVE.md:319`). No decision record supersedes the gate. The deletion was right
on other grounds — `11ca28ba` argues that case well — but the architecture's own
acceptance condition was discharged by deletion rather than by measurement, and
nothing says so. The five-role partition is the final partition by default.

**Recoverable cheaply:** the honest closure is a one-paragraph decision record
retiring the gate and saying why, not a pretense that it was met.

### M-15. The semantic release identity ships on chain with no owning contract

`docs/evidence/CHECKED_RELEASE_CANDIDATE_2026_08_26.md:166`:

> **The semantic release identity has no owner.** Every role, Registry, and Rent
> persists a `semantic_release_id` inside its `ArtifactReleaseV1`, but no
> first-party contract in this tree owns or decodes a role-program release
> preimage. These manifests therefore carry `semantic_kind=unowned` over a
> candidate-declared preimage. **Naming a real owner is an open protocol
> obligation**, not something host tooling should settle.

**Verified:** `semantic_release_id` and `semantic_kind` appear **zero** times in
`WAVE.md` and **zero** times on the board. It is also exactly the half of
`LEAN_SBF_SUCCESSOR_ARCHITECTURE.md`'s succession gate 10 that the shipped
release pipeline does not bind.

---

## TIER 2 — live claims that are false, or safety properties with no evidence

Governing quotes: *"Never-executed is the default"* (`WAVE.md:470`); *"No
estimate is a total; no silence is a result"* (`WAVE.md:475`).

### M-16. `κ` — the capacity bound that is the direct lesson of Mango Markets

`docs/research/CHAIN_STATE_SOURCES_2026_08.md:1041`:

> `total_principal ≤ κ · manipulation_cost_lower_bound(observed depth at
> founding)` … with κ a **Provisional** bound requiring a lifting plan.

`MAINNET_STATE_RELAY.md:29` punts it and never returns across 1,997 lines. It is
in no ADR, no lane, no queue — while the demo shape ember set is exactly the
product class it bounds (*"pumpfun/DBC graduations, mainnet pool prices,
majors"*, `WAVE.md:262`). The graduation market's manipulation floor is derived
exactly (18.618074 SOL) and is a *floor* that does not fall with real liquidity,
which is precisely what makes a κ predicate cheap to write and valuable to have
before a market accepts principal.

`AGENTS.md:125` makes this a rule the tree is currently violating: *"Provisional
bounds require a lifting plan."*

**RESOLVED 2026-08-31 (KAPPA-CAP).** The row is closed, and the closure is worth
recording precisely because of how it read while it was open. κ is enforced on
chain: the Market root carries the bound (`CoreState.principal_cap_sets`, offset
288), `Found` derives it from the one floor record the Source names and refuses
a zero outright, and the bound is re-checked at founding and at all three
principal-growing routes — so it is a cap and not a founding-time formality. The
substitution attack the floor record was designed against is closed by
`SourcePrincipalPolicyV1::BoundedByFloor(selected_floor_id)`. Since 2026-08-31
the refusal also has its own name at each site rather than borrowing a
neighbour's, which is what makes it legible in a validator log.

Two honest caveats travel with the closure. **κ = 1/4 is still Provisional** —
the lifting plan (measure the realisable fraction per venue, then state a
`Measured` envelope) is unchanged and unstarted, so the AGENTS.md rule is
satisfied only in the sense that the plan is written down. And the bound is
**demonstrated on a real ELF at one site of four**: `affine_batch_v2`'s program
test now founds at an exact cap and shows a credit past it refusing by name with
no byte moved, proved red by unbinding the cap; `founding_v5`, `signed_delta_v3`
and the legacy complete-set mint are enforced but still found at `u64::MAX` in
their fixtures, so their refusing arms have not executed on chain.

The row's own history is the lesson worth keeping. M-16 sat open through the
window in which the work was *done* — `WAVE.md` still said "no on-chain route
calls it" four days after routes called it, `CHAIN_STATE_SOURCES` §12.7 still
listed landed items as owed, and the explorer told visitors "nothing on chain
enforces this bound today". A ledger that lags in that direction is not merely
untidy: the KAPPA-CAP lane was chartered to build a wire break that had already
shipped, and nearly did.

### M-17. `OddScheduledMedian`'s cadence tolerance blocks a whole product class

> `OddScheduledMedian` currently requires **strict equal cadence**. Under Solana
> congestion a submitter that misses its schedule slot breaks cadence and the
> statistic refuses… a cadence tolerance is a prerequisite lift for this family.
> This is a **provisional** judgement — no measurement exists.
> — `CHAIN_STATE_SOURCES_2026_08.md:1004`

Rank-3 products — longtail token price markets, the actual long tail of the demo
thesis — need Mechanism B, which needs this. One unlanded lift, in no queue.

### M-18. Eight Kani proof harnesses are committed and have never run

`tools/direct-translation-validator/src/kani_proofs.rs` contains **eight**
`#[kani::proof]` harnesses. `README.md:120`: *"The local host has no usable
Verus, Kani, or Creusot frontend… No result from any of those tools is
claimed."* They are honest and inert. Pinning a `cargo kani` toolchain is in no
lane. `"translation-validat"`: **zero** in `WAVE.md`.

The wider TV programme — seven named parallel lanes at
`RUST_LLVM_SBF_TRANSLATION_VALIDATION.md:251` — has artifacts from exactly one
(lane 2, `formal/qedsvm-direct-v12`, which fails closed: *"no Lean path theorem
was emitted"*). I verdict the **theorems** DROPPED-BY-DECISION under the standing
assurance park (D-3/D-4). The eight committed-and-never-run harnesses are not
theorems; they are uncounted inventory, and they belong here.

### M-19. ADR 0005 promised three omission-index rows. None was recorded.

`docs/decisions/0005-per-market-authentication-cache.md` says, three separate
times, that something *"is recorded in the omission index"* / *"is recorded as
an omission"* / *"is recorded as the lifting plan"* (`:510`, `:303`, `:499`).

**Verified:** `docs/OMISSION_INDEX.md` contains no seal row of any kind. There is
no `formal/dclutch-semantics/Emit*Seal*.lean`.
`crates/dclutch-capability-seal-contract/src/` is still `lib.rs` + `tests.rs`.

The first is not a bookkeeping miss. `SealedDescriptorClosureV1` is a **protocol
byte layout hand-authored in Rust**, self-identified as such, whose Lean
migration is the stated lifting plan for its provisional status. The lane that
declined it said why, and named an owner who does not exist:

> That migration is the lifting plan for its provisional status; **it belongs
> with whoever owns `formal/`.** — board `:2620`

The index's own maintenance rule (`:97`) was violated three times by one ADR.

### M-20. Two ADRs are stale in ways a top-down reader would be misled by

- **ADR 0002 won in practice and was never accepted on paper.** Status still
  reads *"experimental; no successor accepted yet"* (`:3`) while ~40
  `Emit*.lean` files own the tree's three artifact generations. Worse: both
  evidence files it cites (`:57`) now open with *"Historical artifact
  evidence… superseded."*
- **ADR 0003 step 8** required deleting `general-sbf`, `dealer-sbf` and
  `series-sbf` *"in the same convergence cycle that lands the complete Trading
  vertical."* Two were deleted; `programs/dclutch-dealer-sbf/` still exists; and
  the Trading vertical it was gated on still refuses at phase 7/10.

### M-21. Four documents narrate a superseded architecture

`ARCHITECTURE.md` (1,516 commits stale — the map itself notes it *"still
narrates the MarketRoot era"*), `PROJECT_METHOD.md` and
`docs/design/FIRST_VERTICAL_SLICE.md` (1,604), `EXPANSION_FRONTIER` (1,209).

`FIRST_VERTICAL_SLICE.md` is the most orphaned artifact in the tree: 40 lines, no
status header, no date, no evidence pointer, nine acceptance conditions with no
status claimed for any, and its "second slice" named nowhere else. Its target
lifecycle — *"compile → create → split → authenticate Pyth → resolve → redeem →
retain terminal root"* — reaches step 3. Cycle 3's *"docs truth pass"* covers the
READMEs; these four are architecture.

Independently: `tools/gauntlet/journey/src/journey.rs` executes exactly one
lifecycle stage (*"founding through Open"*) and declares five `GapV1` rows for
everything after it, and `apps/dclutch-web/lib/capabilityModel.ts` types all 28
user actions as `'browser-unsigned' | 'rust-unsigned' | 'awaiting-production'` —
**the type has no `signed` or `live` variant at all.**

---

## TIER 3 — decisions ember owns and has not been asked again

Governing quote: *"Yield back a blocker ONLY for a genuine authority decision…
and then **as a question with a recommended answer**, not an inventory row."*
(`WAVE.md:344`)

Five are outstanding. Four are inventory rows. None is on the post-cook plan.

**Count corrected 2026-08-31 (LEDGER-TRUE): four are outstanding, not five.**
M-25 was answered by decision `0016` on 2026-08-30 and this paragraph was never
updated — see its row. The count is left in place above rather than rewritten
because the *reason* it drifted is the thing worth recording: a decision that
closes a ledger row has to be carried into the ledger by the same commit, and
`0016` closed M-25 in its own text without touching this file.

| ID | Question | Where it sits | Recommended answer already written? |
|---|---|---|---|
| M-22 | **The first open Market cannot be redeemed.** Its aggregate is written and `custody_context` is not mutable — re-found at a new generation, or keep it as the recorded witness. *"Owner: ember."* | ADR 0008 §6.4 + board `:11058`; **absent from `WAVE.md`** | yes, two options |
| M-23 | **The reentrancy decision.** *"Needs ember or the protocol owner."*, following *"NO CHILD ROUTE CAN EXECUTE UNDER A REGISTRY CONTINUATION"* | board `:8950`; `WAVE.md:423` records the *wall* as down, not the decision as made | partly |
| M-24 | **The record-layout decision behind the fourteenth wall.** The shipped path spends 1,336,865–1,386,359 CU and **one draw in twenty exceeds 1,400,000 outright** | `WAVE.md:117` — correctly stated, not routed | yes: store each canonical bump in its record |
| M-25 | **Does a checked release describe the artifact or the account?** Revocation is mandatory on deploy day, so every deployed role will be in the state the release cannot describe. *"Reported, not patched."* | `FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md:193` — disowned with no recipient | ~~no~~ **ANSWERED AND CLOSED 2026-08-30 (verified 2026-08-31, LEDGER-TRUE).** The row is stale: it was answered the same day the decision packet ruled, and this table was never updated. [Decision 0016](decisions/0016-checked-release-identity.md) is `Status: **ADOPTED 2026-08-30 — option A, plus the 0012 residual**` (`0016:3`, veto window `27f7944b`), and says so in its own words twice: *"M-25 closes with this record"* (`0016:9`) and *"M-25 closes with a record rather than a fourth reader re-deriving it"* (`0016:137`). **The answer is "neither, and that is the point": three facts, three authors, no self-reference** — the source by `semantic_release_id`, the artifact by the ELF digest, and the account by a policy the live observation must satisfy (`decisions/DECISION_PACKET_2026_08_30.md:48-51`). Revocation-on-deploy-day is therefore not a state the release *cannot* describe; it is a live observation the account policy either admits or refuses. The 0012 residual was ruled in the same breath — `dclutch-release-tool` **stays strict**, an iteration substrate is named and never defaulted into |
| M-26 | **What is the fee rate?** — open since day one | see below | no |

M-26 is the oldest open question in the project and belongs in this tier because
ember asked it himself, first:

> **EMBER:** *"i'm hoping we can also figure out a way we can use our intuition
> about field and flow etc stuff to figure out what the fee/income/revenue
> strategy should be for this smart contract. i think it would be fair to
> capture a modest percentage but **i don't know how to model the tradeoff space
> to figure out *what* percentage. 5%? 0.5%? 0.035%?**"*
> — `01a00a3d`, 2026-08-17T07:25Z

Gen-1 built the *geometry* (`G_num`, complete-set-invariant, with a proved
zero-price laundering channel left open) and never chose a rate; the treasury
pubkey was *"deferred to the first such Realm and reserved to ember."* Gen-3
answers the treasury question structurally — fees route to a per-venue
`fee_recipient` / `fee_recipient_id`, which dissolves the protocol-treasury
question cleanly — and answers nothing else. **Verified: `treasury`, `keeper`,
`bounty`, `revenue` each appear zero times in `WAVE.md` and zero times on the
board.** The field/flow derivation ember wanted was never attempted in either
generation.

---

## TIER 4 — named, unrouted, and cheap

Individually small. Here because *"everything named gets actioned or explicitly
retired"* and none of these was. All live only on the board, in `/private/tmp`.

| ID | Item | Routing as written |
|---|---|---|
| M-27 | **The effect-kernel visitor seam** — two walks decode the same 1,471-byte composition twice; `ProgramV4::resolved_invocation` is O(R²·I) | *"Unowned."* Called *"the single highest-value item in the tree (78,146 CU + 1,465 bytes)"*. Named three times (board `:6993`, `:8938`, `:9493`); W2l took only the sharing half |
| M-28 | **The sysvar-parser convergence** — two independent hand-parsers of the instructions sysvar, one on the heap-admission path without the adversarial corpus | *"still not cheap, still unowned"* — named four times |
| M-29 | **The AccountInfo migration** — 4,776 bytes, *"the floor… W2f's spec is still the only way at it"* | specced by W2g, taken by nobody |
| M-30 | **`--no-default-features --features dealer-family` does not compile** — four `use` sites reference `crate::series` unconditionally | *"owner unclaimed"* — named three times |
| M-31 | **W2h item 1, the allocator cfg** — a `no-entrypoint` library build of Trading runs Hot code under whichever allocator its host installs | *"an owner decision rather than a tail-of-lane edit"* — refused with reasons, never re-taken |
| M-32 | **`GENERAL_CANDIDATE_PAGE_PDA_DOMAIN_V1` is 33 bytes** — same defect class that made an entire transition dead at runtime; only Custody has the seed-length assertion | **CLOSED 2026-08-27 (GEN-CAND, `5987febc`).** The gen-3 constant is `b"dclutch-general-page-v1"` (23 bytes), and all three candidate-half domains carry the `const _: () = assert!(… ≤ 32)` guard Custody had and General did not. The class is closed by construction for this half, not by measurement |
| M-33 | **`core-sbf/src/tests.rs:141` measures a frame 13 accounts narrower than the real one**, so its packet claim is understated | *"Core owner"* — never claimed. `WAVE.md:399` carries the packet claim but not the understatement |
| M-34 | **Four independent ProgramTest-evidence emitters** from four lanes in one hour, plus `check-witnesses.sh` duplicated | *"Somebody should own converging these before a fifth"* |


### DECOMP dispositions, 2026-08-27 (M-27 / M-28 / M-29 / M-31 / M-34)

`WAVE.md`'s DECOMP charter routed five of these rows to one lane. Each below is
either done, ruled, or priced with the arithmetic that priced it. None is left
as "named".

**M-27 — the visitor seam. The sharing half stands; the asymptotic half is NOT
this bundle's lever, measured.** W2l's `ChildWalkResolutionV3` already removed
the second Claims-composition decode and the second role-carrier resolution
(78,146 CU, 1,465 bytes) and that is intact. The remaining claim was that
`ProgramV4::resolved_invocation` is O(R²·I) through `route_request_start`'s
prefix rescan. DECOMP profiled the shipped Direct continuation phase by phase
and it is not where the compute is: both invocation resolutions together are
`pf-invocation-resolved` 4,127 CU, against `p7-effect-projection` 164,290,
`p5r-account-projection` 122,881 and `commit-non-root` 122,268. And the entire
CROSS-SEED variance of the swept path — the thing that makes one draw in sixty
approach the ceiling — is five phases, every one of them a PDA bump search
(`p1-invocation` 3,000, `p1-root` 3,000, `request-lifecycle-preplan` 4,500,
`pf-invocation-preflighted` 3,000, `cm-children` 4,501, at fixture seeds 9/13/1).
The prefix rescan is real and is the right fix for a family with many routes;
it is not the lever for Direct, and pricing it as "the single highest-value item
in the tree" was inherited from a measurement of the SHARING half. Re-priced,
still unowned, no longer mis-sold.

**M-28 — the sysvar-parser convergence. Inventoried in full, and it is one
missing accessor.** The two parsers are
`native_signature::SysvarInstructionV1::read` (the borrowed record reader, with
the ten-test adversarial corpus at `native_signature.rs:371`) and
`entrypoint_adapter::admitted_heap_frame_bytes_from_sysvar_v1:1041` (the
heap-admission scanner, which reaches `dispatch` on every invocation whose data
satisfies `declares_extended_heap_profile_v1`). Their layout arithmetic is
byte-identical — 2-byte count, 2-byte offset stride, 2-byte account count,
33-byte metas, 32-byte program id, 2-byte data length — and both refuse with
`TradingSbfError::NativeSignature`, so folding changes no refusal. The scanner
needs only `program_id()` and `data()` of every instruction plus the leading
count, so the fold is: add a count accessor to `SysvarInstructionV1`, loop
`0..count` over `read(i, data)`, and delete the scanner's private `read_u16`
(its `read_u32` stays for the grant payload). **The concrete gap the row names
is real and sharper than "no corpus":** the admission tests' own `sysvar_bytes`
helper hardcodes `accounts: Vec::new()`, so `accounts.checked_mul(META_BYTES)`
is only ever exercised at zero — the crafted-offset-table, oversized-declared-
account-count and substituted-meta classes have no analogue on the admission
path at all. The in-code `QUEUED CONVERGENCE` note at
`entrypoint_adapter.rs:1035` gives the original reason ("that reader is another
lane's in-flight work"); that lane landed, and the reason has expired.

**M-29 — the AccountInfo migration. PRICED, and the answer is not now.** The
heap is not the binding wall and has not been since W2p. Peak 29,895 of 32,768
leaves 2,873 bytes, 8.8% of the budget, and DECOMP's changes leave that peak
exactly where it was. Compute is the binding wall, at 5,238–12,567 CU of a
1,400,000 ceiling on the worst of sixty fixture draws — 0.4–0.9%. So the 4,776
bytes are worth an order of magnitude less than the same effort spent on CU.
And they are not even the next heap bytes available: W2p's own sized list has
1,688 (the System-instruction clones in `lifecycle-creates`, for which
`entrypoint_adapter::invoke_signed_owned_v1` already exists and the commit path
does not use it), 912 (child-walk buffers reserved to the widest invocation) and
720 (the preflight walk's frame and wire) — 3,320 bytes, more than the current
margin, ahead of the floor. **Trigger, so this is a decision and not a
deferral:** take the migration when the heap peak passes 31,000 with those three
cuts already taken, or when a family lands whose runtime account count exceeds
Direct's. Until then it is the last resort, not the next one.

**M-31 — the allocator cfg. RULED: take it, and the incoherence is LIVE, which
is not how it was previously argued.** W2h refused it partly because the
motivating measurement (253 frame diagnostics) was stale — it is 0 today, and
that is still true. But the underlying incoherence is not latent. Two SBF
cdylibs build `dclutch-trading-sbf` with `no-entrypoint`
(`dclutch-dealer-accelerator-sbf`, `program-test/test-programs/trading-outer`)
and the accelerator executes `authenticate_accelerator_invocation_v4`, which is
`hot_v3` code that Boxes and Vecs freely, under the SDK's allocator rather than
the audited `BumpHeapV1` it was measured against. Worse, under
`no-entrypoint` + `target_os = "solana"` the `scratch_backing` module takes its
NEGATED, host arm inside an SBF program — and W2p already recorded what that
costs: "the `thread_local!` in it made the trading-outer test program ELF
UNLOADABLE… the runtime reports that as `UnsupportedProgramId` and names nothing
about TLS. Cost one gate cycle." **The shipped Trading ELF does not move either
way** (`no-entrypoint` is off there, so all the predicates already evaluate the
same), which is what makes this safe to take.
**The edit is NOT the two files TA-DLR's board post names**, and taking that
post literally will not compile. It is: the `not(feature = "no-entrypoint")`
term removed from EIGHT `#[cfg]` sites in `entrypoint_adapter.rs` (seven
positive — the `#[global_allocator]` static, `program_heap_bytes_used_v1`,
`program_heap_capacity_v1`, `program_heap_scratch_bytes_v1`, the on-chain
`scratch_backing`, `admit_heap_frame_v1`, `lift_declared_heap_profile_v1` — plus
the negated host `scratch_backing` arm, which must narrow to
`not(target_os = "solana")`), the mirror pair at `hot_v3.rs:330/343` that gates
`hot_heap_outstanding`, and `custom-heap` enabled on both cdylibs — where
`trading-outer`'s `custom-heap` feature **does not exist yet and must be added**.
The `not(shadow-accelerator-auth-only)` term the post says "stays" is gone from
the tree entirely. **The trap for whoever takes it:** `trading-outer` is the
outer program of `hot_heap_frame_is_inert`, the instrument that measures the
32,768-byte heap wall. Changing its allocator changes the instrument, so the
edit lands with a full re-measurement of the sweep, not beside one. DECOMP did
not take it inside this lane for exactly that reason: it would have mixed the
instrument with the measurement it was reporting.

**M-34 — the four ProgramTest emitters. VERIFIED TWO-THIRDS STALE, and the real
residue is not a convergence.** Of the four the board named, three no longer
exist: `tools/gauntlet/programtest/evidence.rs` and `tools/gauntlet/tier3/producer/`
were never committed (no git history for either path) and tier2's campaign was
deleted in `cc21a7d7`. `check-witnesses.sh` has exactly ONE copy
(`tools/gauntlet/tier1/`, whose own header says "SHARED by every tier. Do not
fork it"); the duplicate the row complains about was untracked and is gone. The
shell half already converged: all five ProgramTest run scripts fold through the
one `fold-program-test-evidence` binary.
What remains is two emitters, `tools/gauntlet/program-test-evidence` (the shared
crate, ~11 suites) and `tools/gauntlet/direct/producer` (its own workspace, its
own `dclutch-gauntlet-direct-campaign-evidence-v1` schema, serde_json). **They
cannot simply be merged**: `direct/run-direct.sh` feeds the producer's document
straight to `check-witnesses.sh`, and `direct/witnesses.json` plus
`expectations.json` query `artifact.*`, `fast_lane.*` and `ack` — keys the
shared `TransactionEvidence` shape does not carry. The scoped convergence that
IS available: keep the Direct envelope and have its `transactions` entries be
`TransactionEvidence`, which is the only part `census/src/ledger.rs` reads
(it requires `transactions[].label`, `.signature`, `.logs[]`, optional
`.compute_units_consumed`, and ignores every envelope key). **Blocked on a live
claim, not on difficulty:** SN6 claimed `tools/gauntlet/direct/producer` under
M-42 (`wire_bytes` is `None` there) on 2026-08-27 12:11. Whoever takes M-42
should take this with it — it is the same file and the same afternoon.
| M-35 | **The census cannot follow a dispatch through an `unsafe` block** — *"No Direct row can be claimed through Trading until it is closed"* | *"belongs to the census owner."* `WAVE.md:379` carries a *different* census gap |
| M-36 | **590 literal byte offsets hand-mirrored in the browser** across 21 files (51 magics, 33 seed domains) | ratcheted by `lib/abiCoverage.test.ts`, never assigned a lane. Pattern 3 would kill the genus but does not name the inventory |
| M-37 | **`REPLAY_STATE_BYTES` has no Rust or Lean authority anywhere** — the browser decides a replay-account width on its own | *"Owner should be the banish lane"* |
| M-38 | **The vacuous-refusal-test class** — *"ANY OTHER REFUSAL TEST ON THIS BUNDLE THAT ASSERTS ONLY `is_err()` DESERVES THE SAME READING: for the whole heap-wall era the heap refusal was a universal donor"* | none |
| M-39 | **219 clippy findings** under `-p dclutch-trading-sbf --all-targets`, all in `cfg(test)` where crate-level denies were never checked | *"NOT taken, named."* `WAVE.md:240` carries a much smaller residue |
| M-40 | **`build_general_hot_instruction_v3` has zero callers** | *"I did not get to it."* `WAVE.md:396` says it *"finally gets its caller"* under GEN-ART — which yielded without it |
| M-41 | **`MarketOpeningReadinessV1` orphaned** in capability-contract — and gen-2's version was banished with *"this type has no live caller today."* Staged funded market opening died twice without a decision | *"separate finding"* |
| M-42 | **`wire_bytes` is `None` in `dealer/`, `direct/`, `tier4/`** — *"~6 lines… the only way a fast lane can honestly claim TIERS.md condition 2"* | none |
| M-43 | **Coordinate 43 (Custody Mint) pinned `Exact`** at the caller-supplied width; a Token-2022 mint with extensions is wider and refuses | *"flagging, not fixing"* |
| M-44 | **A predicted, dated, unhandled silent-blindness bug** — after Meteora DBC 0.2.0, `TransferHookPool` shares the identical 424-byte body and *"a decoder pinned to `VirtualPool` alone will silently stop seeing transfer-hook pools"* | none |
| M-45 | **The `complete`-flag latch** — *"irreversibility of `complete` is unverified… The adapter must therefore latch on first authenticated observation"*; a correctness requirement for the rank-1 demo product | none |
| M-46 | **Two lanes were silently abandoned.** `DA` (devnet adaptation) posted a START and never posted again — **nobody ever noticed**. `FD` (frontend demo cut)'s abandonment was found three hours later by a different lane reading commit history | the coordinator has no abandonment detector. Verified: one `##` heading each in 262 |
| M-47 | **`ECONOMIC-WEB` is addressed as a lane and does not exist** — STRATUM names it as *"the blocker on two of my cuts."* Verified: zero board headings, one mention | TSGEN's live/dead Lean module split in the same handoff was never answered |

---

## TIER 5 — gen-1 intentions no successor decision ever addressed

Gen-1 is compost by decision, and *"recover user intent and product
requirements"* is an explicitly allowed use (`COMPOST.md:10`). These are ranked
low not because they matter less but because the restart is a legitimate answer
to most of them — the finding is that for these specific ones, nobody said so.

| ID | Intention | Quote | Gen-3 status |
|---|---|---|---|
| M-48 | **A coordinated-disclosure process** | *"A private reporting address and coordinated-disclosure process **will be added** before any public test deployment."* — `SECURITY.md:50` | No `SECURITY.md` in dclutch. Devnet is a public test deployment |
| M-49 | **Independent demand evidence** | *"It is not… **independent demand evidence**, or a protocol release."* — `CURRENT_TRUTH.md:59` | Named repeatedly in gen-1; appears nowhere in gen-3. The project has never tested whether anyone wants it |
| M-50 | **The permissionless work economy** | *"Anyone may submit paid observation, repair, clear, finalize, or cleanup work."* — `PROJECT.md:194`; and *"Candidate submission is solver work, not a crank"* | **PARTLY ANSWERED 2026-08-27 (GEN-CAND, `658b7a3f`)** for General's candidate half: submission is permissionless and unbonded, and every crank — one per execution row, one for the consideration, one for cleanup — is paid out of a compartmentalized, fully refundable work escrow the submission funds exactly, re-proven at every transition by `validate_capitalization`, refunded to the solver on loss. This also fixes what gen-2 got wrong: its consideration was permissionless and **unpaid**, so a valid candidate nobody cranked never competed. Still open: the lamports do not MOVE yet (a transfer is an account operation and these are pure transitions), and the same pattern is unbuilt for observation and repair |
| M-51 | **Zero-volume survivability, protocol-wide** | *"every admitted Market can observe, repair, finalize, and settle from prepaid resources even if later volume is zero"*; and *"there is no global `LivenessPolicy` and no protocol-wide no-stranding result"* | Gen-3 prepays per-capability. No protocol-wide result exists or is scheduled |
| M-52 | **Venue adapters — Manifest, AMMs, RFQs, Jupiter** | *"Materialized Eggs can trade on Manifest, AMMs, RFQs, and future Jupiter routes without making those venues authoritative."* — `PROJECT.md:106`; and ember: *"do we allow using jupiter to witness prices and stuff like that..?"* | Nowhere in gen-3. External venue routing for materialized claims was never re-planned or retired |
| M-53 | **Aeneas/Charon — collapsing the Rust/Lean duplication** | *"may remove the two-implementation cost entirely. Our kernel is unusually Aeneas-friendly (no_std, no unsafe, fixed arrays, checked arith)."* — `GOAL.md:1302`, a "NEXT SESSION — start here" item | Zero hits outside docs in either repo. Named as end 1 of the two-ended TV chain and never started. The session never came |
| M-54 | **`solanalib` fork scoping** | *"Nobody models syscalls anywhere - and our correctness rides on address derivation and `invoke_signed`."* — `GOAL.md:1294`, marked **ember-encouraged** | Restated in gen-3 as a candidate *"after a separate API, proof, license, and provenance review."* The review never happened |
| M-55 | **Succinct clearing via Groth16** | *"~255k CU in a 795-byte transaction — 5.5x margin against the 1.4M ceiling that killed V2"*; *"We are the consumer it never had."* | Scouted once, never pursued. **The 1.4M ceiling is the current fourteenth wall (M-24).** The escape hatch scouted for exactly this wall is in no queue |
| M-56 | **Ban `native_decide` in the Lean tree** | *"it can currently prove False; Lean's compiler is in its own TCB"* — `GOAL.md:1305` | Not written down in gen-3. `formal/dclutch-semantics` has 739 uses, self-disclosed in `TRUST.md` and scoped to regression examples — the honest posture, but the rule was neither adopted nor rejected |
| M-57 | **Ten SOL as an architecture target** | *"**Ten SOL is an architecture target, not a micro-optimization target.**"* | Gen-3's budget is 55 devnet SOL (≈29 SOL rent). Neither met nor retired; the architecture changed underneath it |
| M-58 | **12 stashes, 167 unmerged branches** in dragons-clutch | `BRANCH_TRIAGE_2026-08-22.md:4` — *"§7 is the copy-paste cleanup script; **it is not run here**"*; *"**~65 GB reclaimable**"* | Triage covered 16–27 branches and found *"97–100 % line-level absorption on every branch"* — reassuring, and it did not cover the other 140. Each stash's name is its intention: `general-fee-v3-integration`, `market-theory-quantized-work-authority`, `product-occurrence-root-capitalization-wip`, `fee-pre-row-wip` |
| M-59 | **The `.spw` conceptual canon** | ember: *"you should clone spw-workbench into `.spw/_workbench`… think about using the workbench to distill and refine the conceptual and other layers of this project"*, then two days later: *"its probably been a while since .spw was visited here..."* | 51 concepts in six layers exist in dragons-clutch; nothing in the successor. Ember flagged its abandonment himself |
| M-60 | **EVM** | ember: *"it's possible that we wanna offer BOTH solana and eth...? seems more than reasonable to me"* | **Verified: zero occurrences** of `EVM`/`evm`/`Ethereum` in dclutch outside one Pyth router comment |

---

# DROPPED-BY-DECISION

Recorded so the MISSING list stays honest. These are not findings.

| ID | Intention | Retired by |
|---|---|---|
| D-1 | Devnet deploy | *"**Devnet deploy-and-recycle is deferred**… explicit named authorization required before any deploy"* — `WAVE.md:10`. Runbook complete, its three blockers closed |
| D-2 | Mainnet, real value, real users, production source flip, filings | *"Still human-gated: mainnet, real value, market creation for real users, the production source-registry flip, filings, and Gate L0."* — `GOAL.md:10`. Standing across both generations |
| D-3 | Independent security review (evidence-ladder rung 7) | *"**Assurance work is parked** beyond keeping every claim fail-closed and honestly labeled. Finish and polish first; iterate on assurance in public from a complete basis."* — `WAVE.md:13` |
| D-4 | The universal round-trip and refinement theorems; the 10 succession gates; the 7 TV lanes; the artifact-refinement "trust boundary at victory" | *"Parked: the universal round-trip and refinement theorems — today's evidence is per-case corpora and emitter checks, which prove the cases and nothing else. **That gap is real debt, parked by decision, not covered by anything.**"* — `WAVE.md:464`. Exemplary: the park names its own cost |
| D-5 | A second live Market representation (DCLTCAT1 alongside DCLTCOR2) | STRATUM: *"**DCLTCOR2 is the one Market truth.**"* — `WAVE.md:351`, with two named carve-outs |
| D-6 | `DCLLBX02`, the liability-basis V2 issuance route | *"**ANSWERED AND EXECUTED: deleted.**"* — `WAVE.md:368`. Dead on both ends |
| D-7 | `RentCreditV1` Create/Withdraw; registry `batch_v2`'s `DCLTRGB2` route | DELDEC: both *"delete"*; the batch *"was never executable"* at 2,407,858 CU against a 1,400,000 ceiling |
| D-8 | `GENERAL_ROOT_PDA_DOMAIN_V2` | *"5b19626 ruled `GENERAL_ROOT_PDA_DOMAIN_V2` **must NOT exist**"* — `WAVE.md:158` |
| D-9 | Native polynomial / B-spline / ramp / tent liabilities in the elementary basis | `O-013`, `likely scar` → certified nonnegative integer partition-of-unity bases. **See M-4 — this decision is real and it is one table cell against a five-times-repeated promise** |
| D-10 | A dynamic capability→Program map; a sixth state-owning role | ADR 0003 *"Rejected alternative"* (`:341`), with an explicit conditional revisit |
| D-11 | Reordering root creation into the atomic outer; a pre-Market founding root | ADR 0004 *"Rejected alternatives"* (`:230`), both closed on ground truth |
| D-12 | The chartered activation-written transcript | ADR 0005 *"Rejected alternatives"* (`:440`) — three independent grounds, decisive |
| D-13 | `RelayedObservationSetV1` | `92b137d1`: *"deliberately not implemented… an acceptance path with no consumer is the parallel authority shape AGENTS.md forbids"* — a model refusal |
| D-14 | In-place `realloc` of the top heap block | `9abed0c1`: *"worth zero bytes at every checkpoint… not carried on the chance that some other route would like it"* — working code refused with a measurement |
| D-15 | Four v1 relay lifts (large-account chunking, m-of-n m>1, scheduled-median relayed profile, Realm-level shared cache) | `MAINNET_STATE_RELAY.md:1316`: *"Each is a named lift with a stated trigger, not an oversight."* |
| D-16 | Rocq as a proof substrate | `100e97ea`: *"R5's 'install/pin Rocq and prove' bullet retired, noting Rocq was in fact pinned and **still produced nothing**."* One of the few explicit abandonments in either history |
| D-17 | `opt-z` | *"parked **unless a real rent-per-byte bill appears** (deployment)"* — a park with a reopen trigger |
| D-18 | $DREGG revenue-funded buyback | ember: *"i don't like the idea of revenue funded buyback either. it's just i KNOW the community will ask about it"*. `O-006` forbids token-name branches. **The reasoning was deliberately not published** |
| D-19 | Worktree isolation | ember, three times: *"Stop using worktree isolation, I don't like it and it doesn't work well."* Now one canonical `main`; ~300 stale `agent/*` branches are the residue |
| D-20 | The gen-2 monolith, series-sbf, effect-sbf, economic-sbf, product-payoff-sbf, product-evidence-sbf, the DCLTCAT1 stratum | `WAVE.md:294` (THE PURGE) and the STRATUM/PURGE-INT/DELDEC/WEBGHOST records. ~38,000 + 51,832 lines, with carve-outs named |

---

# CARRIED — the map's coverage, stated fairly

The map holds, precisely and with owners:

- **All 41 blocked routes**, each with a why and an owner; pruned live (44 → 41);
  the census reports stale rows back.
- **The eleven-item GIT-SCAN ledger** (`WAVE.md:168`) — the best instrument in
  the project, and the reason the relayed recovery leg was caught. That item is
  worth quoting, because it is the single best-carried intention in the corpus
  and it got there by being said with feeling:
  > **The gap I most want the next lane to see is 10.5.** — `425a3c90`
- **The twelve Fable dispositions** (`WAVE.md:481`) — islands, static-assert
  genus, the `hot_v3` palimpsest split, the representation map.
- **The two waists, the fourteen walls, and honest gates** — every W-series yield
  says *"the gate is NOT met"* and names what changed.
- **The demo shape and its trust surface** (`WAVE.md:262`) — recovery leg, daemon
  publication, two-clock measurement, Wormhole Queries as candidate upgrade.
- **The devnet runbook**, its twelve authorization items, all three original
  blockers closed.
- **Browser wallet support** — ember asked for Talisman specifically; Wallet
  Standard discovery landed and Talisman was confirmed conformant.
- **The five manuals** — ember's *"protocol reference manual, user guide,
  operator guide, trader guide"* question is `WAVE.md:332` post-cook item 4,
  audited *"all 'no'"* and now the GENREF lane.
- **Implement-then-yield**, **holistic-over-combinatorial**, **commit early and
  often**, **stop re-measuring into tables**, **the purge**, **CUT THE KNOT** —
  six of ember's directives made it into close-out doctrine verbatim.
- **The post-cook plan**, whose items 1 and 3 are exactly the right instruments.
- **The closing pattern language** — a genuinely good compression of the ~45
  items it covers.

Coverage arithmetic: of the 38 `OMISSION_INDEX` rows, `WAVE.md` names 2; the
board adds 7. Twenty-nine are named by neither. Many are covered *in substance*
under other names — cycle-2's family wave is `U-001`, `U-002`, `U-004`–`U-008`;
cycle 3 is `U-010`; RL and GENREF are `U-012`. What is not covered under any
name is the *generalization clauses*: the second halves of `U-004`, `U-008`,
`U-013`, and the whole of `U-015`.

---

# Findings about the map itself

**1. The unrouted items live in the wrong ledger.** The board's header: *"NOT
tracked, NOT authority… WAVE.md in the repo remains the orchestrator's
authority."* Every Tier-4 item lives only there, in `/private/tmp`. When that
file goes, they go. `blocked.json` is the only durable routing artifact the board
ever produced — items that reached it survived; items named in board prose did
not.

**2. The board has no route from "named" to "owned."** Its lane roster covers 5
of ~78 lanes and was never updated. Every rescue of a named-unrouted item came
from one of four mechanisms — an explicit ember instruction, a sweep lane
(SN4/SN5/SN-REC), the orchestrator opening a lane, or independent rediscovery by
a Fable reviewer. None came from the protocol. Its own diagnosis, board `:8158`:

> ### Also fixed here, because it was named three times and never actioned
> … each time the reason it survived was the same: **it is the only generated
> web ABI with no `abi:*` wrapper script**, so it is not in the six-verify sweep
> and nothing notices.

**3. `WAVE.md` GIT-SCAN item 10 is false.** It reads *"stash@{0}
wip-source-borrowed-view: still uninspected, unowned (verified)."* **Verified:**
`git -C ~/dev/dclutch stash list` returns nothing, and there is no stash reflog.
The work survives only as dangling commit **`d5dda5d`** — *"On main:
wip-source-borrowed-view-before-product-domain"*, **364 insertions to
`crates/dclutch-source-contract/src/lib.rs`** — recoverable today and collectable
by `git gc` tomorrow. `git show d5dda5d` while it is still there.

**4. One unmerged gen-3 branch.** `codex/index-collision-safety-20260825`, three
commits with empty bodies. `git cherry`: `4d20d06` absorbed, `145e87a` and
`8a01f2c` not by patch-id — though their content reached `main` by another route
and was partly banished afterward. Five-minute confirm-and-delete.

**5. The tree has no classic debt markers, and that is a strength that hides
things.** One `TODO` in the whole repository, and it is a comment forbidding
TODOs (`journey.rs:26`: *"A gap is not a TODO"*). Zero `FIXME`, zero `HACK`, zero
`unimplemented!`, zero `todo!()`, zero Lean `sorry`/`axiom`/`admit`. Unfinished
intent was systematically pushed out of code and into machine-readable ledgers.
That is good practice, and it means an auditor who greps the code sees a finished
project.

**6. Two integrity events are on the record, both self-reported.** W2e's
confabulated allocation table (board `:3593`) and REFCODE's misattribution
(`:10832`). The rule that came out of the first is stated once and never
restated:

> **DELEGATION WITHOUT RECEIPT IS FABRICATION.** — board `:3730`

It belongs in `AGENTS.md`, where it would survive the board.

**7. The corpus is the only record of the founding, and it is 365 sessions with
no index.** No session has cwd `~/dev/dclutch` — all successor work was done from
a codex session rooted in `dragons-clutch`, so anyone filtering by the live repo
path finds nothing. The genesis session predates both repos and lives under
`~/dev/joshibot`.

---

# Recommendations

Not a plan. The smallest set of moves that closes the gap between the map and
the intention, ordered by what a miss would cost.

1. **Check the two CFTC dockets today** (M-5). 1388 was due yesterday; 1717 was
   due today. This is the only irreversible deadline in the ledger.
2. **Write down the founding intentions** (M-1 … M-4, M-6). One document —
   `docs/INTENT.md` or a `PROJECT_METHOD.md` section — carrying: what the public
   protocol is a demo *of*; the pride criterion; the twelve-item ceiling; the
   B-spline requirement and the fact that `O-013` is its substitution; and the
   eight method rules. This is the highest-value hour in the list, because
   everything else in this ledger is recoverable from artifacts and these are
   recoverable only from `cv`.
3. **Surface `O-013` to ember as a substitution, not a table cell** (M-4). *"'5
   fixed bands' is really not good enough"* was the requirement; certified
   integer partition-of-unity bases are the answer the successor chose; the
   first slice is proved and wired to nothing. He should get to say whether that
   is the same thing.
4. **Add an eighth pattern: THE EXPANSION FRONTIER** (M-9, M-10, M-11, M-12).
   One Fable-tier lane, one output: every frontier and every omission row gets
   scheduled, parked with a stated trigger, or retired with a reason. The
   index's own maintenance rule already requires this.
5. **Widen post-cook item 3 to `dragons-clutch --all`** (M-13). One word.
6. **Ask the five Tier-3 questions** (M-22 … M-26) as questions with recommended
   answers, which is what CUT THE KNOT asks for. Four already have the
   recommendation written.
7. **Move Tier 4 into `blocked.json` or a tracked file before the board
   expires.** `/private/tmp` is not durable.
8. **Recover `d5dda5d` and fix GIT-SCAN item 10** — 364 lines that `git gc` can
   take, watched by a row that is wrong.
9. **Write ADR 0005's three omission rows** (M-19), and retire the monolith
   benchmark by decision in one paragraph (M-14). Both are honesty repairs to an
   otherwise unusually honest record.
10. **Fix the `fail-closed` language in `WAVE.md:13`** (M-6). Ember named the
    phrase as a shirk today; the standing decision uses it exactly that way.

---

# Provenance

Read in full: `WAVE.md`; `docs/OMISSION_INDEX.md`; `docs/research/*` (5);
`docs/decisions/*` (8); `docs/design/*` (4); `docs/evidence/*` (14);
`docs/compost/*`; `COMPOST.md`; `ARCHITECTURE.md`; `PROJECT_METHOD.md`;
`README.md`; `AGENTS.md`; `formal/dclutch-semantics/TRUST.md`;
`tools/gauntlet/{blocked.json,TIERS.md,DESIGN.md,README.md,CU_BUDGETS.md,journey/README.md}`;
`/private/tmp/dclutch-wave-board.md` (all 11,764 lines) plus its four staging
siblings (verified duplicates); `~/dev/dragons-clutch`'s ten root docs and its
`docs/`, `research/`, `site/`, `lean/`, `rocq/`, `verus/`, `toolchain/`,
`benchmarks/`, `.spw/` trees (~90 files); `~/dev/dclutch-legacy` (26 strata);
both git histories in full (6,713 commits, subjects and bodies, plus branch and
stash topology); and 365 harness sessions plus the pre-repo genesis session via
`cv`.

Verified directly against the tree at `90e3c21`, rather than taken from any
document or agent report: the orphan-kernel dependency checks (M-9), **including
the two negative results that corrected overstatements** —
`representation-composition-v3-kernel` has eight consumers and
`dealer-scenario-kernel` has one, so both are wired; the General batch-lifecycle
caller check and the `GeneralRootV2` field/method check (M-12); the banished-verb
presence sweep; `bspline`/`spline`/`Bernstein` absence (M-4); the
`dark`/`FHE`/`shielded`/`DrEX`/`zkML`/`EVM` absence (M-1, M-60); multi-LP
presence (M-11); fee-recipient plumbing (M-26); stash and dangling-object
forensics (finding 3); `git cherry` on the unmerged branch (finding 4); the Kani
harness count (M-18); the board heading census for DA/FD/`ECONOMIC-WEB`/W2q
(M-46, M-47); and every zero-mention claim in the MISSING list.

Two sub-findings were corrected rather than silently dropped. The legacy dig
reported that General, Dealer and Series *"have no successor plan at all"* — they
do (7, 9 and 5 live actions), and the real finding is narrower and sharper
(M-12). It also reported `EmitSeriesAbiRust.lean` as an orphaned emitter; that
file no longer exists, and the two Series emitters that do are both in the
lakefile.

One row this audit cannot settle from artifacts: **M-5**, the two CFTC dockets.
Everything else is checkable.

*The ledger is honest. It just outran its own index — and the index was never
the founding.*

---

# GITSCAN-2 — the pre-successor commit sweep, 2026-08-27

Status: an addendum to the audit above, answering the one question it named and
did not run. `WAVE.md:169`'s GIT-SCAN swept gen-3's 1,509 commits. **M-13** says
Dragons-Clutch's history "ha[s] never been swept by anything." This is that
sweep: every pre-successor commit message read for a **named-but-deferred
claim**, and each distinct claim verdicted against the live tree.

Verdicts here are **ACTIONED** (the successor carries it — cited),
**OBSOLETE** (an architecture decision made it meaningless — the decision
cited), or **STILL OPEN** (the intention transfers and nothing carries it).
Gen-1's systems are dead; the test is never whether the *code* survives, only
whether the **intention** does. New rows are numbered `G-*` so they cannot
collide with the `M-*`/`D-*` above.

**Written under the standing rulings at the head of this file**, which landed
after ARCH-EOL and while this sweep was running:

- *"dark-FHE is NOT a near/medium-term ambition… its Tier-0 rows are
  DROPPED-BY-DECISION for this horizon."* Everything this sweep found about the
  confidential-energy programme is therefore recorded as **provenance for a
  parked row** — where it was written down, and what the next step was if the
  horizon ever changes — not as an open obligation. That is **C-2** and **G-6**.
- *"The monolith-vs-split benchmark is CLOSED — the five-role partition stands,
  no benchmark owed."* So **§C.1** locates M-14's missing originals as a
  provenance repair to a **closed** row. What survives it is the *class*: 42
  other never-run gates, which the ruling does not reach.
- *"Weigh this ledger as evidence, not obligation: a mention is not a
  commitment."* Taken literally throughout. §H is what this evidence would
  support if someone wanted it, not a queue. Several rows below are recorded
  precisely so that a future *"we never wrote that down"* is answerable, and for
  no other purpose.

## The corrected arithmetic

`M-13` and this addendum's charter both quote **5,106** unswept commits. That
number is `git -C ~/dev/dragons-clutch rev-list --all --count`, and it already
contains the grafted dclutch subtree. The honest split:

| | |
|---|---|
| dragons-clutch `rev-list --all` | 5,108 (5,106 at ARCH-EOL) |
| — of which the grafted dclutch lineage (already swept as gen-3) | 1,604 |
| **Never-swept pre-successor commits** | **3,504** |
| — reachable from `main` | 1,974 |
| — only on the ~364 `agent/*` branches | 1,530 |

## What the corpus turned out to be

| date | commits | with any body | body > 400 chars |
|---|---:|---:|---:|
| 08-18 → 08-22 (**gen-1**) | 764 | 474 | 244 |
| 08-23 → 08-24 (**gen-2**) | 2,729 | **107** | **9** |
| 08-25 → 08-27 (host-side) | 11 | 3 | 2 |

**This reframes M-13, twice.** Gen-2 is 78% of the never-swept set and is
*message-less*: 2,729 commits (1,568 unique non-merge subjects, the rest
cross-merge noise across ~364 branches) carry 107 bodies between them. So
**M-13's "5,106 unswept" is really 764 commits of dense prose and 1,568 subject
lines**, and a commit sweep is close to the wrong instrument for the larger half.

The second reframing is better news and is §D of this addendum. Gen-2 did not
write its intentions in commits because it wrote them **in a machine-readable
catalogue**: `~/dev/dragons-clutch/programs/solana-layout/src/registry.rs`, 3,649
lines, **nine populated action enums carrying 129 named action coordinates, every
one with a doc comment** — plus the 62-variant legacy `Intent` in the sibling
`lib.rs`. Gen-2's intentions are not lost to **M-7**'s encrypted charters. They
are enumerated, in one file, in the compost repository, and nobody has read them
since 2026-08-24.

Against gen-1's 764 the instrument works well, because gen-1 wrote in a
declarative past tense that makes a deferral conspicuous: across all 3,504
commits, `"will be"` occurs **zero** times and `"should"` on **three** lines.
This project has never promised anything in a commit message. It *records* debt
instead, in a vocabulary it invented: `owed`, `queued`, `flagged`, `residual`,
`blocker`, `parked`, `out of scope`, `not measured`. That vocabulary is what was
swept.

## Counts

| | |
|---|---:|
| Pre-successor commits read | **3,504** |
| Commits carrying a debt token | 137 |
| Commits carrying a deferral *claim*, read in full | 113 |
| Secondary corpus: measurement/gate vocabulary, body > 300 chars | 85 |
| Gen-1 planning/review documents opened because a commit pointed at them | ~40 |
| Gen-2 branches enumerated | 361 |

Three corpora, counted separately because they behave differently:

| corpus | claims | ACTIONED | OBSOLETE | **STILL OPEN** |
|---|---:|---:|---:|---:|
| **§A/§B** — gen-1 commit prose and the documents its commits added | 118 | 44 | 33 | **41** |
| **§C** — measurement-as-decision-gate commitments, both generations (the **M-14** class) | 78 | 11 *ran* | 24 | **43** |
| **§D** — gen-2's named action coordinates in `registry.rs` | 129 | ~57 have successors | 62-variant `Intent` namespace, by `O-002`/ADR 0003 | **72, in 16 capability families** |

| | |
|---|---:|
| New numbered rows (`G-*`) | 26 |
| Never-run gates (`N-*`) | 22 |
| Existing `M-*` rows sharpened | 24 |
| **Corrections to the audit above** | **4** |

Not one of the 43 never-run gates, and not one of the 72 vanished coordinates,
was retired by a decision record.

---

# §A. Four corrections to this ledger

These come first because three of them move the audit's own top recommendation.

## C-1. Tier 0 is not "recorded nowhere." It was recorded, in gen-1, five days before this audit.

The audit's headline finding is that the founding intentions "were never written
down at all" (`:64`), and its second recommendation is to write them down
because "everything else in this ledger is recoverable from artifacts and
**these are recoverable only from `cv`**" (`:912`).

**Verified: false.** Gen-1 committed a 137-line intent archaeology on the same
corpus, from the same sessions, on 2026-08-22:

> `~/dev/dragons-clutch/docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md`
> — commit `6b9fd37f`, *"This review reconstructs Dragon's Clutch from the human
> messages that created and directed it. It is a requirements source."*

Its method section is the same method (`cv index` / `cv search` / `cv show`),
and it names the same primary sessions, `01a00a3d` (cwd `~/dev/joshibot`)
included. It then lists **fourteen recovered product requirements**. Against the
audit's Tier 0:

| Audit row | Where gen-1 already wrote it |
|---|---|
| **M-4** the B-spline requirement | requirement 4: *"distributions over bounded outcomes rather than a **toy handful of fixed bands**… a compact smooth basis with exact partition-of-unity semantics"* |
| **M-1** / **M-3 item 11** Clear/Shielded/Dark | requirement 8: *"**Design Clear, Shielded, and Dark modes as modalities of one relation**, while keeping the public Solana system independently useful. **Do not turn privacy research into an oppression tool.**"* |
| **M-6** "no minimal demo" | requirement 12: *"The user repeatedly rejected 'minimal demo' or isolated-slice completion framing; a working bounded transition family is substrate, not the end state."* |
| **M-6** "choose the weakest" | requirement 14: *"Prefer the least constraining and most general sound choice when several designs are viable."* |
| **M-6** "audits are not work" | requirement 9: *"do not let an **evidence bureaucracy substitute for product capability** or honest runtime integration."* |
| **M-3** the one-sentence thesis | *"compile objective state and path predicates into fully collateralized payoff bases, clear bounded portfolio programs through interchangeable checked venues, and settle proof-carrying evidence without an operator"* — the same sentence, and the doc calls it *"the most compact original thesis, written by the user."* |
| **M-3 item 12** agent coordination surface | requirement 11 names *"increasingly capable machine traders"* as a first-class audience; §9 of its sibling review specifies the six artifacts (see **G-14**) |
| **D-18** / `O-006` no DREGG branch | requirement 3: *"DREGG is a dogfood Realm, not required collateral or a hard-coded branch."* |

And its closing verdict is a gen-1 statement of the audit's own Tier 1:

> *"It did not finish the complete recovered product thesis. In particular, the
> payoff compiler remains a research crate rather than a market-creation path;
> **Clear/Shielded/Dark do not share a deployed relation**; capacity is one
> fixed profile; **the current adapter is not actually collateral-program-
> generic**; source identity is not production-pinned; and nothing is deployed.
> 'Cycle-G capability-complete' must therefore be read as 'the bounded Cycle-G
> capability matrix has no unimplemented transition,' not 'Dragon's Clutch is
> complete.'"*

Two siblings carry the rest:

- `docs/reviews/INSTRUMENT_AND_MARKET_DESIGN_REVIEW_2026-08-22.md` (785 lines,
  `ae09e06e`) — *"whether the instrument, market structure, and surrounding
  product are worth having… It deliberately proposes an ambitious continuation
  rather than a smaller 'shipping' subset."* §9 enumerates five sophisticated
  extensions; §10 is titled **"Relationship to the original Dark energy
  intent"**; §11 is **"Scar tissue: keep, replace, and reframe"** — gen-3's
  `likely scar` vocabulary, one generation early.
- `docs/reviews/SOPHISTICATION_GAP_2026-08-19.md` (`5ef1edfa`) — the two-move
  strategy (**G-2**, **M-55**) and a four-line "Absent layers" inventory.

**The correction changes the recommended action.** Recommendation 2 asks for an
hour of `cv` archaeology that has already been done. The actual owed work is a
**port**: three committed gen-1 documents did not cross the generation boundary
on 2026-08-24, and nothing in `COMPOST.md`'s allowed-use rule (*"recover user
intent and product requirements"*, `:10`) stopped them. **Copy them, do not
re-derive them.**

## C-2. M-1's absence sweep is wrong on the dragons-clutch half.

M-1 states: *"`dark`, `FHE`, `shielded`, `Shielded`, `DrEX`, `zkML` — **zero
occurrences** in `/Users/ember/dev/dclutch` outside `node_modules`. **Also zero
in dragons-clutch's `docs/` and `research/`.**"*

The dclutch half re-verifies. The dragons-clutch half does not:

- `docs/SWARM_ROADMAP_2026-08-19.md:55` — a **"Confidential energy"** row in the
  live-surface snapshot: *"Clear bounded optimum relation plus a real CPU TFHE
  candidate predicate for feasibility and settlement conservation | Global
  optimality, vFHE, private settlement, custody, and production remain absent."*
- the same file `:278`, `:349`, `:386`, `:415`, and its §5 **"Confidential-energy
  work"** lane (see **G-6**).
- `docs/OPEN_QUESTIONS.md:151` — *"Commit/reveal, MPC, FHE, vFHE, or
  proof-carrying confidential orders"*, filed under *"Explicit future research,
  not V1 dependencies."*
- `docs/SPECIALIZED_BATCH_RELATION.md:18` — the standing disclaimer that the
  relation *"provides order confidentiality, front-running resistance, FHE, MPC,
  a TEE, or a…"* [negated].
- `docs/design/SUCCINCT_CLEARING_FEASIBILITY.md:105` — the Zama FHE-crate
  patent-encumbrance analysis.

**This changes the diagnosis, not the disposition.** Ember has since ruled
dark-FHE out of the near/medium-term horizon and dropped M-1's Tier-0 rows by
decision, so nothing here is owed. What is worth correcting is *why* it looked
forgotten. The dark platform was not lost because nobody wrote it down; it was
written down — a snapshot row, a research lane with a next gate, an anti-goal
protecting the ordering, and a four-stage relation — and then it did not cross a
subtree merge. Recorded here so that the park is a park over a **known** body of
work rather than over a blank, and so that if the horizon ever changes, the next
step is a lookup and not an archaeology (see **G-6**).

## C-3. `3c0eb2ca`'s four workstreams are not "named in a subject line and nowhere else."

M-13 cites `3c0eb2ca` *"Chart the next wave: maturation, sophistication,
optimization, assurance"* as *"four workstreams named in a subject line and
nowhere else."* The commit's body is indeed empty. Its **diff is an 85-line
roadmap**: `docs/design/NEXT_WAVE_ROADMAP_2026-08-20.md`, four phases with 15
numbered items, each naming the decision it assumes, plus a dependency
paragraph. It was amended once more at `7150a012`.

That roadmap is the single densest source of still-open gen-1 intent in this
sweep — **G-3**, **G-9**, **G-13**, **N-13**, **N-15** and three sharpened `M-*`
rows all come out of it.

**This generalizes into the method note that matters most for the next sweep.**
A full-text search of all 3,504 pre-successor commit messages for the words a
gate is written with — `nonnegotiable`, `only after`, `acceptance condition`,
`before we delete`, `monolith`, `split release set` — returns **zero hits**.
Every one of the twenty-two never-run gates in §C is in a document that a commit
*added* or *amended*, and several of the sharpest are in a commit whose body is
empty. Commit-message sweeps are the wrong instrument for this project on their
own: **in this repository the promise is in the diff.** A future GIT-SCAN should
run `git log --diff-filter=A --name-only` over `docs/` and read what arrived,
not only what was said about it.

## C-4. M-16's `κ` has an older parent than the research doc.

M-16 dates the capacity bound to `CHAIN_STATE_SOURCES_2026_08.md:1041`. Its
gen-1 ancestor is a **P1 register row**, `docs/OPEN_QUESTIONS.md:94`:

> *"Security tiers, **per-feed exposure limits**, and multi-source aggregation."*

filed under *"P1: before accumulator implementation"* and never retired — the
row carries no `Decided`/`Retired` marker, unlike its four siblings in the same
list. So the Mango lesson has now survived **two** generations unactioned, not
one, and it was a stated *precondition* in gen-1 rather than a provisional bound
in a research annex. See also **G-8**.

---

# §B. STILL OPEN — new rows, ranked by transfer-worthiness

Ranked by how much of the intention survives the death of the system that
stated it. Ids are allocation order; the section is in rank order.

**Read `N-1` in §C first.** The single highest-consequence open row this sweep
found is a never-run gate rather than a lost intention — the fee base's promotion
criteria never closed, and the successor ships the arm a delegated decision
record named *eliminated*. It sits in §C with its siblings rather than here.

`G-1` is deliberately first below and is deliberately *not* claimed as new: the
`LB-SPLINE` lane found that gap independently this afternoon and wrote it up
better. It is kept because the pre-successor history adds three things a
gen-1-versus-gen-3 comparison cannot see.

## G-1. The degree-≥2 price gate — already found and owned; what this sweep adds is that **gen-2 built it too, over integers**

**Do not read this as a new finding.** The `LB-SPLINE` lane found the same gap
independently while this sweep was running and documented it better than this
addendum would have, in
`docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md:74` — *"Degree ≥ 2
arbitrage gate | gen-1: built (moment cone V1b)… | this lane: **absent** |
**gen-1 ahead — the largest gap**"* — with the executable arbitrage written out
(`3·1 − 4·e_j` at `p = S·e_j`, since interior degree-2 basis functions peak at
`3/4`), the trigger stated as a precondition (*"it must be closed **before** a
Market can select degree ≥ 2, not after"*), and the honesty that gen-1's own gate
was **provably incomplete** on multi-span grids with a pinned false acceptance.
That row is owned. This entry exists only to add three things it could not know,
because they are in the pre-successor history rather than in gen-1's tree.

**1. Gen-2 built the gate as well, independently, and over integers.**
`clutch-price-measure` (8,843 Rust lines) carries a Bernstein-moment continuous
checker **and a quantized checker over bounded integer atom mixtures** — a
certificate that an admitted price vector comes from a nonnegative measure —
with the branch `general-v2-action10-closed-tuple` requiring *"quantized
admission before General ranking work"*, plus
`quantized-atom-mixture-certificate` and `exact-quantized-atom-solver`.
**Verified: `atom mixture`, `quantized admission`, `price admission` — zero in
the successor.**

This bears directly on the scorecard's stated dilemma — *"port a
sound-but-incomplete gate, or do the per-span Hausdorff witness generation one
designed and never built"*. **There is a third option and nobody has looked at
it**, and it is the one already written for exact integers, which is the posture
`LiabilityBasisV2` is in. Whether the quantized checker is sound, complete, or
cheap is not something this sweep can say; that it exists, and that no one
comparing gen-1 to gen-3 would ever have seen it, is.

**2. Gen-1's wire half is not in the scorecard's table.** The comparison covers
the gate as mathematics. The pre-successor history also carries its *binding*:
`EpochAccount` gained `basis_degree: u8` at `cc14bcce`, copied from
`TermsAccount` at `InitEpoch` under `binds_terms`, with `clear_walk` refusing
`UnsupportedBasisDegree` rather than falling back — *"an ungated clearing above
degree one is precisely what the gate exists to stop, so an unreadable degree is
a refusal, not a shrug."* The account grew 328 → 329 bytes and the byte's offset,
the three shifted fields, and the two Direct record widths that carry it are all
recorded. **Verified: `basis_degree`, `BasisDegree` — zero in the successor.**
If the gate is ever ported, that is the layout half of it, already costed.

**3. The pattern has now happened three times, not twice.** The scorecard says
gen-1 shipped the claim plane ahead of the price plane and this lane did the same.
Gen-1 named the hole first at `5847a3f9`, 2026-08-21, and the sentence is worth
keeping because it names the mechanism exactly:

> The clearing sees a basis only through partition of unity and **no gate in
> front of the relation restricts a market's degree, so nothing distinguishes a
> simplex point from a moment vector.** The moment-body admission test is open
> and not built.

That is a description of `LiabilityBasisV2.Basis` today, written eleven months
of project-time before it existed. Its structure is `exactWidth`,
`payoutBounded`, `partitionUnity` — nonnegativity, boundedness, partition of
unity, no degree — which is precisely *"a basis only through partition of
unity"*.

**One thing this sweep does contradict.** `O-013` is repeatedly discussed, here
and in **M-4**, as the decision that substituted for the B-spline requirement.
On the price side it substituted for nothing: it is a decision about a *basis*,
and the gate is a *price-admission* rule. Two generations built the gate and no
decision in any generation retired it. The scorecard's trigger — write it down
now rather than rediscover it — is the right closure, and `O-013` is not where
it goes.

**Closed 2026-08-27 (PRICE-GATE), and here is the part this entry said it could
not judge.** Item 1 above ends *"whether the quantized checker is sound,
complete, or cheap is not something this sweep can say"*. It can now be said, in
the successor's own terms rather than by reading gen-2's tree: the hull-membership
rule **is sound**, and `DClutchSemantics.LiabilityBasisV2PriceGate.Certificate.no_arbitrage`
is the proof — zero `sorry`, zero `native_decide`, three standard axioms. It is
**not complete** in the sense that matters, and the incompleteness is inherited
rather than introduced: a `u64` mixture mass refuses a hull price whose every
representation needs a larger denominator, exactly the residual gen-2 named at
`docs/design/PRICE_MEASURE_WITNESS_V2.md:188`, and it fails closed. Both
directions of gen-2's refutation are reproduced against this tree's evaluator as
`decide` witnesses and as corpus cases. The gate is in the kernel today as one
admission conjunct; item 2's layout half is still unbuilt and still out of scope
by Frontier 2's gate, which is the right order — the price plane is ready
*before* the layout that would make it reachable. See
`docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md` ("the row that flipped")
and `docs/compost/PRICE_GATE_HULL_2026_08_27.md`.

**Item 3 is not retired by this.** The pattern happened three times; it was
*corrected* the third time, one lane later, before a Market could select the
basis. That is a better outcome than gen-1's two days with the hole open, and it
is not evidence the pattern will not recur.

## G-2. The compute ceiling is gen-1's *scaling* verdict, and its named answer is in no queue

`5ef1edfa` (2026-08-19) is the gen-1 assessment that set direction:

> *"…records the compute ceiling as an architectural verdict rather than a
> tuning problem… and sets **two strategic moves**: V3 to make the spline claims
> tradeable, then succinct verification to answer the compute wall by joining
> the consumerless breadstuffs STARK stack."*

The doc corrects itself honestly the same day (§3, *"The verdict this section
originally drew was wrong… the cost was a software SHA-256"*), and the surviving
claim is narrower and still load-bearing:

> **What survives is the scaling argument only: growth in book width still goes
> through staging (V3) or succinct verification — a design preference now, not a
> measured wall.**

> **Move 2 — answer the compute wall with succinct verification.** The reason V2
> died is that the chain must re-execute a clearing to trust it… **Joining them
> is the single highest-leverage architectural move available.**

Both moves are open in gen-3. Move 1 is **M-4**/`LB-SPLINE`, opened today. Move
2 is **M-55**, in no queue — and **M-24**, the fourteenth wall, is a book-width
CU problem being worked as a record-layout problem. This does not say the layout
work is wrong; it says gen-1 wrote down, and gen-3 has not re-decided, that the
width axis is the one that staging or succinctness answers and shaving does not.

Sharpens **M-24** and **M-55**: the Groth16 scout was not a stray idea, it was
**move 2 of exactly two**, and move 1 is now in flight without it.

## G-3. Per-order cancellation: parked in gen-1 behind a trigger that fired, and never lifted

`docs/design/NEXT_WAVE_ROADMAP_2026-08-20.md:73`, Phase S item 5:

> **Per-order cancellation / continuous-claims scouting** stays parked **until
> the above land** (design complection deferred per the directive).

"The above" is Phase S items 1–4. **All four landed inside gen-1**:
PartialFillLedger retired at `47c7a77a`, VirtualPot at `cd54bb72`,
VirtualMergeCredit at `41c231f6`, fee plumbing to the boundary at `525ec13f`,
wider campaigns sealed at `df1d99e1`. The trigger fired on 2026-08-21 and the
park was never lifted.

Cancellation was already a *named product gap* three days earlier:

> …the newly named product gap that **V4 has no per-order cancellation**: the
> legacy CancelOrder epoch role admits only legacy lengths, so a V4 order can be
> retired only by aborting the whole unfrozen Epoch. — `e43fbe8e`

**Verified in gen-3:** Direct *does* carry cancellation —
`DirectExecutionActionV3::CancelRegistered` and `::CancelThrough`, with a signed
`CancelThroughV2` intent. `dclutch-general-codec::Action` carries **seven**
verbs and none of them cancels: `Consider`, `Freeze`, `InitializeSettlement`,
`Collect`, `Materialize`, `Distribute`, `Close`.

**Updated the same afternoon.** `85c279ec` landed ADR 0009 and rewrote M-12
above: the collection half now has an ownership ruling and three named capability
actions — **`OpenBatch`, `PlaceOrder`, `CloseBatch`** — and M-12's own closing
list names *"cancellation"* among five items still open. So this row is no longer
unnoticed, and what it adds is provenance that argues for its placement: **the
three-route set has no cancel in it**, and cancellation is not a nice-to-have
that follows placement. It was a named product gap in gen-1 (`e43fbe8e`), a
separately parked roadmap item whose trigger fired (`N-13`), a live verb in
gen-2's catalogue (`GeneralV2Action::CancelOrder`, tag 5, sitting between
`PlaceOrder` at 4 and `FreezeEpoch` at 6), and it is *implemented today in
Direct*. Four generations of evidence say it belongs in the same artifact
regeneration as the other three, not after it — which matters because ADR 0009's
own cost line is *"one batched identity regeneration"*, and a fourth tag is
cheapest inside that batch.

So the honest verdict is **half ACTIONED, half STILL OPEN**, and the open half
is the batch venue. **M-12** names `CancelOrder` inside the vanished front half
of gen-2's 25 verbs and frames the whole loss as *collection*. This sharpens it:
cancellation is a separate, separately-named, separately-parked product
requirement, it was flagged in gen-1 *and* built in gen-2 *and* dropped in
gen-3, and Direct's implementation is the proof that the intention transfers —
it is the same protocol, one venue over.

`continuous-claims` — the other half of that parked item — is **zero
occurrences** in gen-3 and has no successor at all.

## G-4. The dependency/licence/SBOM review: gen-1 had a green gate and three families owed human eyes; gen-3 has no gate at all

Gen-1 ran a real SBOM:

> 36 manifests, 2,129 unique rows, 0 failures, status=PASS, and the committed
> catalog is byte-equal to a fresh run… **The open release item is unchanged:
> the previously flagged MPL-2.0 family, CDLA roots, and one license-file-only
> crate still want human eyes.** — `31ede419`, 2026-08-22

and a second, narrower human item:

> One vendored crate (solana-define-syscall 5.1.0, verbatim Apache-2.0,
> checksummed in vendor/PROVENANCE.md) **flagged for user provenance review**.
> — `d936eaa6`

Both sit under a P3 register row, `docs/OPEN_QUESTIONS.md:141`, *"before public
release"*: **"Dependency, license, AGPL source-offer, and SBOM review."**

**Verified in gen-3:** `SBOM` — **zero occurrences**. No `cargo-deny`, no
`cargo-about`, no licence job; `tools/` holds ten entries and none of them is a
dependency gate. `apps/dclutch-web/package-lock.json` now carries an MPL-2.0
dependency that no first-party process has ever looked at, and the workspace is
AGPL-3.0-or-later, whose **source-offer obligation attaches on distribution** —
which is what publishing the repo and serving the frontend both are.

**This is the only row in this sweep that is a regression rather than a
carry-over.** Gen-1 had the instrument, ran it, and left three named items for a
human. Gen-3 deleted the instrument and inherited the items without knowing it.
The intention transfers completely and the surface it applies to is strictly
larger (an npm tree gen-1 never had).

## G-5. Upgrade posture — the one decision gen-1 explicitly deferred to ember, with no successor

`dragons-clutch/archive/gen1/docs/decisions/ADOPTED_2026-08-20.md` has a
section with exactly one entry:

> ## Deferred with the tension named
> - **Reference-deployment upgrade posture**: the report recommended
>   immutable-at-first-deployment; **ember's weakest-choice principle favors
>   upgradeable-then-burn (burn is always available; un-burn never is)**.
>   Deferred — mainnet is gated regardless, and the devnet posture is settled by
>   item 5.

and the register row it points at states the actual obligation
(`docs/OPEN_QUESTIONS.md:51`):

> Decide whether the reference deployment has a time-bounded audited beta
> authority followed by irrevocable removal, or is immutable at first
> deployment. **Source code must support either deployment without pretending
> the former is the latter.**

**Verified in gen-3:** `upgrade posture`, `upgrade governance`,
`immutable-at-first` — **zero occurrences**. `upgrade_authority` appears in 65
files as *runtime mechanics* (the checked-release pipeline), and ADR 0005
discusses upgrade only as a thing that invalidates seals. There is no ratified
posture and no record that one is owed.

This belongs in the audit's **TIER 3** and is a sixth member of it: it is
ember's, it was formally deferred rather than forgotten, it has a written
recommendation *and* a written counter-recommendation in ember's own principle,
and its second sentence is an **architectural** requirement that binds today —
source that supports both postures is a design constraint, not a deployment-day
choice. `D-2` gates mainnet; it does not answer this.

## G-6. PARKED BY RULING — but the confidential-energy programme had a specified next gate, and it was not a backend

**Disposition: DROPPED-BY-DECISION for this horizon**, per the ruling at the head
of this file. Recorded as provenance only.

**M-1** reports the dark platform as unstarted. Gen-1's `SWARM_ROADMAP` §5
(`R5 — Close formal, confidential, and governance boundaries`) says otherwise —
it is a three-step lane with step 1 claimed done:

> **Confidential-energy work:**
> - Keep encrypted feasibility, settlement conservation, and global optimality
>   as **three separate predicates**.
> - **Next build a fixed-topology end-to-end leakage and failure-recovery test
>   plan**: malicious input, inclusion/non-equivocation, encrypted owner
>   allocation, settlement commitment/note ledger, selective decryption, key
>   rotation, abort/recovery, and proof/dispute.
> - **Only then** compare vFHE/MPC/proof backends. More accelerator browsing or
>   a universal encrypted VM is not the next gate.

§7 *Attractive distractions* makes the ordering enforceable: *"another FHE
backend or accelerator survey **before** the leakage/recovery plan"* is listed
as a thing not to do. And §2 records step 1's artifact: *"a real CPU TFHE
candidate predicate for feasibility and settlement conservation."*

`INSTRUMENT_AND_MARKET_DESIGN_REVIEW` §10 then writes the relation down:

```text
private provider bids and operational constraints
  -> specialized confidential optimization relation
  -> efficient feasible plan and settlement quantities
  -> bounded public disclosures and correctness certificate
```

> *That relation may include cost curves, ramp limits, outages, inventory,
> commitment constraints, hedge books, or network constraints which providers do
> not wish to reveal.*

**Verified in gen-3:** zero, as M-1 says. The thing worth keeping across the park
is that the next action was **known, cheap, and not a backend**: an
eight-component leakage and failure-recovery test plan, with an explicit
anti-goal against surveying backends first. M-1's *"steps 2, 3 and 4 never
started"* is better read as *"step 2 has a written first task."* If the park ever
lifts, that sentence is the whole handoff.

## G-7. Candidate withholding, proposer bond, and best-submitted-versus-optimal — gen-3 ships the identical mechanism with the identical hole

`docs/OPEN_QUESTIONS.md:125`, P2, *"before simplex-auction freeze"*:

> Candidate replacement window, **proposer bond, withholding resistance**, and
> the distinction between **best submitted and globally optimal**.

> Candidate public score and whether small-book exact-rational or primal/dual
> certificates can establish optimality for a restricted fragment.

Gen-1 recovered the requirement in ember's own voice too
(`PROJECT_INTENT_ARCHAEOLOGY` requirement 6): *"Keep price formation pluggable
and permissionless. **Say 'best valid submitted candidate' unless a checked
optimality certificate exists.**"*

**The honesty half is ACTIONED, verbatim.** `dclutch-general-codec::Action`:
`Freeze` = *"Close selection around the current **best valid submitted
candidate**."* That phrase is the requirement, kept for three generations.

**The mechanism half is STILL OPEN.** `Consider` = *"Submit an authenticated
candidate for deterministic comparison"* — permissionless, no bond. `proposer`,
`globally optimal`, `Dutch`, `pro-rata` — **zero occurrences** in gen-3;
`withholding` occurs five times and every one is fee-withholding or byte-
withholding, none is a solver refusing to submit.

**And gen-1's answer is the sharpest part.** Gen-1 did not leave this at the
register row — it wrote two ADRs about it, in `~/dev/dragons-clutch/docs/adr/`,
and *adopted* a mitigation:

> The successor uses individually funded reverse-linked nodes and a
> **commit/reveal subdivision of `[F,S)`**; `[S,V)` verification and
> deterministic best-valid-submitted selection are unchanged at the semantic
> boundary. — ADR 0006, `:96`

with its cost and its limit stated honestly in the same family:

> commit/reveal adds one boundary and at least one transaction per candidate…
> **Commit and reveal throughout the same interval.** Once one reveal is public,
> **a later commit can copy it with a new reward destination.**
> **Claim commit/reveal solves MEV.** It only blocks the simple reward-copy path
> described above. — ADR 0008, `:249`, `:275`

**Verified in gen-3:** `commit/reveal` — **zero occurrences**. Gen-3's General
carries the two-window shape (`Consider` → `Freeze`) and not the subdivision, and
no record says why. So a *ratified gen-1 ADR decision* did not cross the
generation boundary, and the honest reward-copy analysis that came with it —
which is real mechanism-design work, correctly scoped, with its own limits named
— would have to be re-derived from scratch.

A solver who computes the best candidate and withholds it, or submits a worse one
it can profit from, faces no bond and no detection in either generation.
Sharpens **M-50** (the permissionless work economy): gen-3 has prepaid
`bounty_lamports` and no solver, and this row is *why* the solver half is hard —
it was known to be an unanswered mechanism-design question, not an
implementation task. And **§D.1 item 7**: gen-2 allocated the whole verb set —
`ClaimCandidateBond`, `ClaimCandidateWork`, `ClaimSolver`, `ClaimEpochUnused`,
`MarkWorkClosed` — every one of which is zero in the successor.

## G-8. The collateral-genericity demonstration was specified as a two-instance proof and never ran

`docs/OPEN_QUESTIONS.md:74` (Realm admission, retired-in-part 2026-08-20):

> Still open and *not* covered by that item: pinning the exact Token-2022 program
> artifact (register F5), and **the two-synthetic-Realm demonstration** below.
> … **Demonstrate generic semantics with two synthetic Realms; DREGG must not
> create a special branch.**

Gen-1 then delivered its own verdict against it: *"**the current adapter is not
actually collateral-program-generic**"* (`PROJECT_INTENT_ARCHAEOLOGY`), against
recovered requirement 3, *"Make the system genuinely general."*

**Verified in gen-3:** `two-synthetic`, `collateral-generic`, `generic
collateral`, `second collateral` — **zero occurrences**. `O-006` forbids
token-name branches, which is the *prohibition*; the **demonstration** — two
independent collateral instances proving no branch exists — has no successor.
`F-6` (token behavior profiles) is the nearest gen-3 row and the audit already
verdicts it **MISSING** (`M-10`), noting the one lift happened *"as a hardcoded
`Token2022BehaviorProfileV2` struct, not the versioned selectable profile record
the frontier specified"* — which is exactly the failure the two-Realm demo
exists to catch.

This is a **never-run gate** as well as an open intention; it appears again as
**N-10**.

## G-9. Continuous integration: gen-1 named it, gen-3 has no workflows at all

`NEXT_WAVE_ROADMAP` Phase M item 5:

> **Housekeeping with teeth**: … **CI adoption (register F8 — the manifest gates
> are the CI; wiring them into an Actions matrix is now cheap** and the Pages
> workflow broke the no-workflows seal).

**Verified:** `~/dev/dclutch/.github/workflows/` **does not exist**. Every gate
in the successor — the gauntlet, the census, `genref --check`, the ABI byte
comparisons, the six-verify sweep — runs only when a lane remembers to run it.
The `GENREF` yield hours ago says so in its own words: *"GATE WIRING OPEN:
genref --check is freestanding — wire into gauntlet census stage or CI at
convergence."*

The audit's finding **2** (*"the board has no route from 'named' to 'owned'"*)
and its board `:8158` quote (*"it is the only generated web ABI with no `abi:*`
wrapper script, so it is not in the six-verify sweep and **nothing notices**"*)
are both instances of the same missing organ. Gen-1 named the organ and priced
it as cheap.

## G-10. The seven pre-filing gates

`SWARM_ROADMAP` §5, *Governance/regulatory work*:

> **Before any filing: fact lock, claim-provenance review, privacy review,
> redaction/public-record review, entity/control/deployment facts, qualified
> legal review, and a separate explicit user authorization.**
> No roadmap item authorizes filing, ex parte contact, or regulator outreach.

`ADOPTED_2026-08-20.md:70` then reserves the acts themselves to ember:

> **Reserved to ember (unchanged by this record):** **Filing submissions (Aug
> 24/26/27)**; the E2 freeze act; the E3 registry flip (ember's explicit go at
> the 12-gate table, never pre-authorized); the treasury pubkey;
> **counsel/security/license engagements**; mainnet; L0.

This sharpens **M-5** on two points. First, the audit recovers the Aug 24/26/27
filing calendar *"from the transcripts, not from any repo"* — it is in a
committed gen-1 decision record, dated 2026-08-20, four days ahead. Second, the
seven pre-filing gates are a checklist nobody has claimed: gen-3 has no
regulatory workstream, and `degg-research` is named in **M-5** as the place the
October compute-derivatives RFC should live and doesn't.

It also sharpens **M-26** (`treasury pubkey`) and **D-3** (`security
engagement`), and it supplies the missing owner for **G-4**: *license
engagement* is on ember's reserved list, so the SBOM's three flagged families
have a named decider and have simply never been put to him.

## G-11. Bounty economics were to be measured; they never were, in either generation

`docs/OPEN_QUESTIONS.md:102`, P1:

> **Reverse-Dutch bounty step count and measured SOL cost quantiles.**
> Whether any historical provider dependency is acceptable for repair.

**Verified in gen-3:** `Dutch` — **zero occurrences**. Gen-3 replaced the
mechanism with prepaid `bounty_lamports` (**M-50**), which is a defensible
simplification and does not answer the row: what a repair or resolution actually
costs in SOL, at what quantiles, is unmeasured in both generations, while
**M-51**'s zero-volume survivability claim and `WAVE.md`'s prepaid-capability
story both rest on the answer. Also a never-run gate (**N-17**).

## G-12. Archive paging: page size, retention horizon, recycling proof, and Window cache identity

`docs/OPEN_QUESTIONS.md:96`, P1, one of the few rows carrying its own status
note:

> **Archive page size, retention horizon, recycling proof, and Window cache
> identity.** Still open. Note that the R4 §8 reference-ownership fork that would
> consume a retention horizon is **explicitly deferred** until the
> provider-horizon evidence exists (`ADOPTED_2026-08-20` item 7), so this row is
> not blocking a decided design.

**Verified in gen-3:** `retention horizon`, `provider horizon`, `recycling
proof`, `archive page size`, `Window cache` — **zero occurrences each**. The
deferral's trigger (*"until the provider-horizon evidence exists"*) was never
gathered, so the fork is still deferred by a condition nothing is watching —
also **N-14**.

This is the register ancestor of **M-10**'s `F-8` verdict (*"width erasure done
at N=258; **paging MISSING**"*). The paging half was an open P1 question in
gen-1 and is a missing frontier half in gen-3; nothing between them decided it.

## G-13. External assurance: four human items, one parked by decision and three unowned

`NEXT_WAVE_ROADMAP` Phase A item 4:

> **External assurance**: the STOP-8 human items (**license-row review, security
> review, signed tag, second macOS host**), and the hostile terminal walk as the
> standing regression floor.

| item | gen-3 verdict |
|---|---|
| security review | **DROPPED-BY-DECISION** — `D-3`, the assurance park |
| license-row review | **STILL OPEN** — see **G-4**; no instrument survives |
| signed tag | **STILL OPEN** — `signed tag`: zero occurrences; the checked-release pipeline signs artifacts, not releases (**M-15**) |
| second macOS host | **STILL OPEN** — `reproducib*` appears in 42 files, but independent second-host reproduction has no successor to gen-1's Persvati/Hbox pair |

The hostile terminal walk **is** carried, as the gauntlet.

## G-14. Agent-facing strategy artifacts were specified, not merely wished for

`INSTRUMENT_AND_MARKET_DESIGN_REVIEW` §9 turns **M-3 item 12** from an ambition
into a six-item interface:

> Expose canonical read-only artifacts for: payoff basis and worst-state payout;
> source and gap semantics; state-price and measure-witness diagnostics;
> executable order limits and fee bounds; candidate relation/objective/
> certificate identity; and roll/refinement maps.
> **Agents should construct unsigned transactions or plans from these artifacts;
> they must not need a privileged Dragon service.**

**M-3 item 12** verdicts this *"No. No agent intent format."* The sharpening is
that the format does not need designing — it is six named artifacts, five of
which gen-3 already computes internally, and the last mile is exposure. Note
also that `apps/dclutch-web/lib/capabilityModel.ts` types every user action as
`'browser-unsigned' | 'rust-unsigned' | 'awaiting-production'` (`M-21`), i.e.
the tree already produces unsigned plans and simply never named them an
interface.

## G-15. The remaining new rows, compactly

Each verified as zero-in-successor unless noted.

| ID | Intention | Source | Note |
|---|---|---|---|
| G-16 | **Cross-market collateral netting** and its successor design, *"separately capitalized cross-market risk vaults… a frozen joint-state worst-case certificate and its own reserve; it may not borrow claimant principal from underlying Hoards"* | `OPEN_QUESTIONS:152`; `INSTRUMENT…` §9 | gen-3's nine `netting` hits are all *within* one Dealer scenario; the cross-market case is untouched. Adjacent to `U-004`'s open half (**M-11**) |
| G-17 | **Price-measure and welfare certificates as first-class artifacts**, with the four-row profile→certificate table that *"turns profiles into honest compilation targets rather than arbitrary feature cuts"* | `INSTRUMENT…` §9 | the successor's capability profiles carry no certificate column |
| G-18 | **IPFS pinning diversity and canonical release-manifest location** | `OPEN_QUESTIONS:139` (P3) | `IPFS`: zero. Bears directly on **M-15**, the unowned `semantic_release_id` |
| G-19 | **AGPL source-offer** mechanics on distribution | `OPEN_QUESTIONS:141` | **HALF ANSWERED, 2026-08-30 (DIST).** The repository now distributes a *binary*: GitHub Releases `v0.1.0-devnet.*` of `dclutch-cli`. That leg carries the offer properly — the AGPL text ships inside every archive, and the release body states in one sentence that the repository IS the corresponding source, with the link. What is still open is the leg this row was written for: §13's **network-interaction** offer. The Pages site conveys no source link from the running program, and the released binary is a client that talks to a *cluster*, not a server the AGPL's §13 obliges. So: distribution-by-copy is covered, distribution-by-network is not. **IN FLIGHT 2026-08-31 (DESIGN lane) — verified in the working tree, NOT yet committed (LEDGER-TRUE).** The §13 leg is being built right now and this row should not be read as untouched work: `apps/dclutch-web/components/SiteFooter.tsx` exists and is mounted once from the root layout (`apps/dclutch-web/app/layout.tsx:5,62`), carrying a `Source` anchor to `https://github.com/emberian/dragons-clutch`. Its own doc comment states the reasoning this row asked for — that the condition is about network interaction, and that a footer on *some* pages would not discharge it because a reader can land on any route directly, which is why it is in the layout rather than in the workspaces. **Status is precisely "uncommitted"**: the component is untracked and the layout change is an unstaged 2-line diff, so at the moment of this verification the deployed site still conveys nothing. Do not close this row on the strength of this note — close it when the commit lands. **One residual the footer does not reach, named now so it is not lost:** §13 obliges the offer of *"the corresponding source of the version they are using"*, and a bare repository link is not version-pinned — a visitor gets `main`, not the commit the running site was built from. Discharging that wants the build's commit in the footer (or in a linked build record), which is a smaller change than this one and is the actual close condition |
| G-20 | **Cross-Series portfolios and rolls** — calendar spreads, roll trades, multi-horizon ladders, *"not single-Market complete sets, so each Market Hoard remains segregated"* | `INSTRUMENT…` §9 | = `PRODUCT_THEORY_REDIRECTION`'s cross-expiry rolls (**M-8**), specified one generation earlier |
| G-21 | The **error-code consolidation pass**: *"clutch-sbf carries a parallel 0x3000 numbering with the forbidden catch-all (**queued for the instruction-wave consolidation pass**)"*; *"the lossy 0x3fff collapse of eleven gate classes"* | `89e329c6`, `e2b887a9`, `1d0c2576` | **ACTIONED in principle** by ADR 0007 (namespaced refusal codes) — recorded here because it is the clearest case of a gen-1 debt row that a gen-3 *decision* silently discharged, which is what a sweep is for |
| G-22 | The **12-gate E3 table** for the production source-registry flip | `R2_PHASE0_RUNBOOK.md` §4.1; `ADOPTED:74` | `12-gate`: zero in gen-3. `D-2` gates the flip; nothing enumerates what must be true first |
| G-23 | *"Whether any historical provider dependency is acceptable for repair"* | `OPEN_QUESTIONS:103` | bears on **M-17**'s cadence tolerance and the relay's recovery leg |
| G-24 | **Exact admitted portfolio-intent language**: proportional divisibility, partial fills, limit semantics, maximum coefficient/term count | `OPEN_QUESTIONS:121` | gen-1 answered all four *for its own venue*; gen-3 re-derived none of them as a stated language. Adjacent to **M-3 item 3** |
| G-25 | **Standing-maker definition** (*"at least one full frozen Epoch is the leading candidate"*) and **same-Epoch crossing / self-cross treatment** | `OPEN_QUESTIONS:117` | gen-1 froze `self_cross: RefuseOverlap` (`11008dac`); gen-3 has no self-cross rule under any name |
| G-26 | *"A **Position funding ledger** is the recorded residual that would make a principal payable"* | `7d7a135f` | the last gen-1 account family with unrefundable rent; gen-3's `LifecycleRentCreditV2` (`P-005`, `F-7`) is the mechanism that would close it, and no one checked whether every family uses it |

---

# §C. NEVER-RUN GATES — the M-14 class

**M-14** found one: the *"nonnegotiable"* monolith-versus-split benchmark that
never ran, whose deletion discharged its own acceptance condition. The charter
asked whether there are others of the same class.

**There are.** Of 78 measurement-as-decision-gate commitments found in the
pre-successor corpus, **11 ran**, **24 are obsolete**, and **43 never ran** — one
of which, the monolith gate itself, ember has now closed by ruling.
They group into the twenty-two findings below. Two structural notes first:

- **The gates are almost never in commit bodies.** A full-text sweep of all
  3,504 messages for `nonnegotiable`, `monolith`, `split release set`,
  `only after`, `acceptance condition` returns **nothing**. Every gate below is
  in a document a commit *added*. This is **C-3**'s method note in its strongest
  form: in this repository, the promise is in the diff.
- **Not one of the 43 was retired by a decision record.** Twenty-four stopped
  mattering because THE PURGE (`D-20`) or the gen-3 restart destroyed what they
  guarded; nineteen still bind. In neither case did anyone write the sentence.

## §C.1 — CLOSED BY RULING: M-14's originals, located, as a provenance repair

**Disposition: the partition ruling at the head of this file closes it — the
five-role partition stands and no benchmark is owed.** This subsection exists
because M-14 asked *"whose commitment was it?"* and the answer turns out to
change how the closure should read.

The charter asked for the monolith-versus-split commitment's originals. The
2026-08-25 research doc is the fourth statement, not the first. All three
predecessors are on `main`:

> **Do not immediately split into many CPI-coupled programs**: atomicity,
> account locks, upgrade coordination, and CPI overhead may be worse. **First
> measure deployable feature profiles** with identical semantic owners…
> — `6b9fd37f`, 2026-08-22, `docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md:405`
> (and its repair-order item 9 at `:494`: *"Introduce capacity and deployable
> capability profiles **before** increasing constants or splitting programs."*)

> **Measure multi-program composition as a system before splitting.** A sibling
> program can reduce one ELF while adding a second Program/ProgramData rent
> principal, CPI CU, metas, upgrade coordination, and atomicity risk. **Compare
> total persistent rent and runtime behavior, not ELF size alone.**
> — `e7d8de26`, 2026-08-23, `docs/reviews/CAPABILITY_PROFILE_SIZE_AUDIT_2026-08-23.md:200`

> **Compare binary partitioning as a system.** Measure monolith versus
> capability profile versus multi-program composition including CPI CU, extra
> metas, atomic rollback boundaries, deployment/upgrade liquidity, and total
> persistent rent. **Adopt the smallest verified capability surface, not the
> smallest ELF in isolation.**
> — `502d9ad0`, 2026-08-23, `docs/reviews/RENT_COMPUTE_CAPITAL_TIME_AUDIT_2026-08-23.md:395`
> (experiment 8 of a ranked queue)

**Verified in the successor:** no total-persistent-rent or CPI-CU comparison of
a monolithic against a split route exists — `total rent`, `capability-profile`,
`General-only`, `Direct-only`: zero. And gen-3's own ADR concedes it:

> **No measured result since that decision establishes a distinct syscall or
> canonical-state ownership boundary for the new family Programs.**
> — `docs/decisions/0003-fixed-role-capability-execution.md:39`

So the commitment was made **four times across two generations** and the
measurement never ran once. And it started earlier still, as a **P0 register
row** posed before either generation built anything
(`docs/OPEN_QUESTIONS.md:54`):

> ### Internal venue ownership
> Decide whether issuance and simplex venue live in **one immutable program** or
> the venue **calls conservation-checking instructions on an Eggcrate-owned
> Position program**. A separate venue must never write Position bytes directly.

That is the same fork, filed under *"P0: before kernel semantics freeze"*, and
gen-3's ADR 0003 answered it by partitioning into five roles.

**What this changes about the closure.** M-14 reads as one architecture doc's
unmet acceptance condition. It is really a question the project asked itself at
its very first commit and re-committed to measuring four times. The ruling
settles it correctly and on other grounds — `11ca28ba` argues that case well —
and the honest retirement paragraph is now a *stronger* one than M-14 imagined:
not *"we skipped a benchmark"* but *"we asked this on day one, promised the
measurement four times, and decided it on design grounds instead."* One
paragraph, citing `docs/OPEN_QUESTIONS.md:54`, `6b9fd37f:405`, `e7d8de26:200`,
`502d9ad0:395`, and `MULTIPROGRAM_OWNERSHIP_EXPERIMENT_2026_08_25.md:218`,
closes it completely.

**The ruling does not reach the class.** It closes this gate. Forty-two others
below are untouched by it, and §C.2's first two rows are live.

## §C.2 — The gates that still bind

Ranked. The first is the sharpest single row in this addendum.

### N-1. The fee base: the guarded decision was reversed in silence, and gen-3 ships the arm that was eliminated

Gen-1 ran the comparison honestly, having first found it unrunnable:

> arm 3 — per-Egg `q*p_i*(1-p_i)` charged leg by leg, **the specific baseline
> the design was built to beat** — does not exist in any language… So **the fee
> has never been compared against its own benchmark**, and §7 reads as if the
> comparison were merely un-run.
> — `docs/reviews/FEE_ECONOMICS_FINDINGS_2026-08-19.md:49` (`342247c7`)

The arms were then built and the fork was decided:

> **Fee base: the composite `kappa*G + kappa'*R` SHAPE is selected**; both rates
> remain undecided; every byte stays `FeeBaseV1::None` until the destination
> lands… **Reversible until a rate freezes.**
> — `ADOPTED_2026-08-20.md` item 9

`docs/OPEN_QUESTIONS.md:106` states the eliminated alternatives plainly:
*"uncertainty-shaped dispersion with a price-free quotient-norm floor; **flat
and per-leg are eliminated**."*

**Verified in the successor.** `G_num`, `dispersion`, `quotient-norm`,
`per-Egg`, `arm 3` — **zero occurrences** outside this ledger. What ships is:

```
crates/dclutch-direct-codec/src/intent_v2.rs:48
    /// Exact cumulative floor-fee rate accepted by the maker.
    pub fee_basis_points: u16,
```

— a **flat rate on notional**, in 67 places across `crates/` and `programs/`,
exercised at nonzero rates in `docs/evidence/DIRECT_FAMILY_CAMPAIGN_2026_08_27.md`.

That is the arm item 9 eliminated. There is no decision record reversing item 9,
and `FEE_GEOMETRY.md` §7's own promotion criteria — Lean closure of six
properties, the digest-pinned correspondence review, the executable lab gates,
*"adversarial simulation finds no cheaper equivalent encoding or
fragmentation"*, and the reward-liveness criterion — **never closed**. Register
entry **B2 (`fee-bounds-freeze`)**, which required freezing five bounds *before*
implementation, never happened either.

**M-26** asks *"what is the fee rate?"* This is the prior question: **what is
the fee's geometry, and when did it change?** The audit records that
*"`G_num` exists in gen-1 with proved complete-set invariance; nothing in
gen-3"* (**M-3 item 10**) and treats that as a lost capability. It is worse than
lost: it was selected, its rivals were named eliminated in a decision record
ember delegated, and the eliminated rival is what runs today. **M-3 item 10's
zero-price laundering channel is open against a base that was never supposed to
be the base.**

Cheap and honest closure: one paragraph saying flat notional is now the V1 base
and why, or a row saying the composite is still the target and `fee_basis_points`
is a placeholder. Either is fine. Silence is not, because ember delegated item 9
on the weakest-choice principle and this reverses it.

### N-2. `MAX_OUTCOMES = 16` — frozen three generations deep on a benchmark that never ran

> Freeze `MAX_OUTCOMES = 16` for V1… **A future transaction format must be
> feature-detected and benchmarked before raising the bound.**
> — `71491141`, 2026-08-18, `docs/COST_MODEL.md:82`

Restated twice: `docs/PROTOCOL.md:42` (*"V1 should freeze `2 <= outcome_count <=
16` **unless the transaction-size and account-limit benchmark proves a different
safe maximum**"*) and `a81b609b`'s `research/claim-algebra-model/ARCHITECTURE_REVIEW.md:181`
(*"**Benchmark `n = 2,4,8,16` before changing the outcome cap.** Publish the
error bound beside the transaction/account/CU cost."*).

**Verified in the successor:** the cap is no longer prose, it is a
machine-checked consensus constant.

```
formal/dclutch-semantics/DClutchSemantics/GeneralConfigAbi.lean:19
    def maxOutcomes : Nat := 16
crates/dclutch-general-config-contract/src/generated.rs:4
    pub(crate) const MAX_OUTCOMES_V2: usize = 16;   // @generated from Lean
```

**NEVER RAN, DECISION FINAL BY DEFAULT** — and it is precisely what blocks
**M-3 item 1**, the multidimensional claims (*"terminal price × maximum
drawdown; TWAP × realized volatility"*), whose basis count grows
multiplicatively. Note the tree already goes wider elsewhere:
`COMPOSITION_MAX_OUTCOMES_LEAN_V3 = 256`. So the 16 is a *venue-config* cap that
nobody has re-derived, sitting next to a 256 that somebody did.

**Cheapest recoverable gate in this addendum**: a bounded SVM sweep at
n = 2, 4, 8, 16, 32 against one Lean-emitted constant with one emit site.

### N-3. The E1 falsifying dual-toolchain spike — the verification architecture, decided by abandonment

> V1 must restrict Eggcrate to the common conservative Rust subset and **run a
> falsifying dual-toolchain spike before architecture commitment**… **Reject the
> single-source approach if** SBF compilation requires divergent executable
> branches, first-party assumptions, public unchecked preconditions, or
> materially different runtime behavior.
> — `71491141`, `docs/VERIFICATION.md:200`, `docs/ENGINEERING_PLAN.md:179`

Its own verdict table (`docs/implementation/TOOLCHAIN_SPIKE.md:152`) reads
`adapter/ELF/program-test | **NOT RUN** | no claim` and `resource and mutation
matrix | **NOT RUN** | no claim`, concluding *"**NO-GO for declaring the E1
toolchain gate closed or creating the protocol workspace**"* — restated at
`f58f1a23`, *"E1 promotion remains **NO-GO**."*

**NEVER RAN.** The protocol workspace was created anyway and the verification
architecture then changed twice by report (ADR 0003 → ADR 0005, `eb0b24bf`) with
gates 5 and 6 still NOT RUN and the reject-criteria never evaluated. `Eggcrate`,
`dual-toolchain`, `unannotated` — zero in the successor. `D-16` retires Rocq and
`M-18` counts the Kani harnesses; **neither covers the spike that was supposed
to choose the substrate in the first place.** Now unrecoverable — the
single-source Verus architecture no longer exists — so the honest closure is a
written retirement, M-14's shape exactly.

### N-4. `docs/BENCHMARK_PLAN.md` in its entirety, and the control arm for the project's central claim

> Status: experiment design. **No results in this document are measurements.**
> — `71491141`, `docs/BENCHMARK_PLAN.md:3`

§9 specifies `research/results/<date>-<scenario-id>/` as the result contract for
every experiment in it. **`research/results/` does not exist and never did** —
`git log --all -- 'research/results*'` returns nothing across both repositories.
Nine experiment matrices have no result directory. Two arms are separately
load-bearing:

- **§4 `:96` — *"external independent-Egg control versus coupled relation."***
  This is the control that would justify the coupled batch relation over
  independent per-Egg books, i.e. **the project's central architectural claim**.
  `independent-Egg`, `coupled relation`, `coupled clearing`, `small-book oracle`
  — zero in the successor.
- **§8 `:186`** — a stop condition on the venue's shape: *"the native venue
  narrows to single-Egg coupled clearing if arbitrary portfolio search pressures
  documentation into an unproved optimality claim."*

The same control arm is the **JOSHI kill criterion**, stated as a falsifiable
disqualifier for the whole integration (`docs/JOSHI_EXECUTION_THESIS.md:169`):
*"The JOSHI integration is not interesting if… **the native auction gives no
coherence/cost advantage over ordinary Eggs**."* Never tested. Adjacent to
**M-49** (independent demand evidence) but distinct: M-49 asks whether anyone
wants it; this asks whether the mechanism beats the trivial alternative.

### N-5. R2 Phase-0 Condition A — a named STOP that was never evaluated, and the version it guarded is now the codec identity

> Both conditions must be **written down before the cutover. A span chosen after
> seeing the data is not a criterion.**
> **3.1 Condition A** — the SDK version discrepancy observed resolved. The named
> STOP: the migration guide says **1.2.0**, the SDK manifest says **2.0.0** …
> *resolved? (yes → record the single version / no → **STOP**)*
> **3.2 Condition B** — receiver `Config` bytes stable over a named span.
> — `95e2c2f3`, 2026-08-21, `docs/implementation/R2_PHASE0_RUNBOOK.md:216`

**The four-row table in the runbook is still empty.** In the successor there is
no mention of the discrepancy and **SDK 2.0.0 is hard-pinned in six source
files** (`crates/dclutch-pyth-svm/src/{receiver_config,price_update,lib}.rs` and
the synthetic-release evidence). Condition A was never evaluated; 2.0.0 became
the codec identity by default. Condition B likewise —
`fixtures/pyth/upgraded-2026-08-26/PROVENANCE.md` is a **single ten-minute
observation**, which is the exact thing the runbook forbids.

**Both are cheaply recoverable**: two upstream fetches and one repeat RPC read
≥ 24 h apart. Condition B is still live, because the freeze act is human-gated
(`D-2`); Condition A is already baked in.

### N-6. The Aeneas/Charon spike was a ratified ADR gate with a kill criterion, not a wish

> The Aeneas/Charon spike (one pure kernel function, bounded, **with a kill
> criterion**) is the named test of closing the model-to-source arrow in Lean…
> **If the spike fails**, executable-body growth continues in Verus, never by
> relabeling model theorems.
> — `eb0b24bf`, `docs/adr/0005-lean-proof-substrate-of-record.md:43`

**Sharpens M-53**, which records it as a `GOAL.md` *"NEXT SESSION — start here"*
bullet. It was that **and** a gate inside the ADR that made Lean the substrate of
record, with a stated consequence for failing. `Aeneas`/`Charon` appear in the
successor in three files, all prose. Never run, never killed.

### N-7. The one-hot versus derived-basis bake-off

`a81b609b`, 2026-08-18, `research/claim-algebra-model/ONE_HOT_VS_DERIVED.md` —
a named head-to-head. The winner (one-hot first) was chosen **on argument**; the
loser's promotion was explicitly gated on measurement (`:109`):

> 5. **Treat a higher outcome cap as an account/transaction/proof benchmark, not
> a documentation edit.** 6. **Promote native fractional degree-1 markets only
> after** the resolution record and kernel account bind the resolved vector and
> one of these policies is frozen: exact lots, persistent remainder credits, or
> portfolio-atomic aggregate redemption.

Gen-3 is one-hot (12 files, led by `dclutch-liability-basis-v2-kernel` — which
has **zero consumers**, `M-9`); the derived branch was never benchmarked and is
gone. **Sharpens M-4** by supplying the dated comparison document that named the
benchmark, and it interlocks with **N-2**: item 5 is the same outcome-cap gate.

### N-8. The Dealer liveness budget vector — a measurement named as the activation gate, with a named owner who does not exist

> `DealerFundedBudgetDependenciesV1` … **selects no vector values; the
> liveness-policy owner must derive and measure them from maximum
> row/page/CU/account/rent work before any adapter can activate.**
> — `2a325b98`, 2026-08-23, `docs/design/DEALER_RUNTIME_V1.md:658`

`DealerLivenessSchedule`, `liveness schedule`, `liveness budget`, `work
principal` — zero in the successor. Only *"bounded work rewards"* survives, as
one line of **M-11**'s six-item list. **Sharpens M-11** with an earlier
statement, a stronger verb (*must derive and measure*), and the same
nonexistent-owner shape M-11 already flags at `dealer-v2-scenario-collateral.md:101`.

### N-9. Succinct clearing's two unmeasured quantities

> …the **two conditions that gate the direction**… **§7 What is unmeasured (and
> must not be estimated):** Cert-F proof size and prove time at Dragon's Clutch
> batch width… whether the Cert-F statement survives recursion into the apex
> without exceeding the measured shrink budget. **Either could change the
> verdict.**
> — `30461261`, 2026-08-19, `docs/design/SUCCINCT_CLEARING_FEASIBILITY.md:7`, `:119`

**Sharpens M-55** with the sha and the exact gating clause, and joins **G-2**:
this is move 2 of gen-1's two-move strategy, and **M-24**'s 1.4 M-CU wall is the
exact condition it was scouted for. Cheaply recoverable — it is a bounded
measurement, and the wall is standing right now.

*(Substrate note, recorded because it is the standing rule: that same commit
surfaces one hand-written Rust AIR twin as debt. Any resumption of this
direction is Lean-authored AIR; the Rust twin is debt, not a foundation.)*

### N-10. Two-synthetic-Realm collateral genericity — see **G-8**

> **Demonstrate generic semantics with two synthetic Realms; DREGG must not
> create a special branch.** — `docs/OPEN_QUESTIONS.md:78`

**NEVER RAN.** Collateral-genericity is recovered requirement 3 and is asserted,
never exhibited, in both generations. `O-006` is the prohibition; this was the
proof.

### N-11. The R2 hybrid-representation reject criterion

> Artifacts and falsifiers: … **measured cost of three representation
> controls** … **Reject or redesign if** supply ownership cannot stay singular
> or Token-2022 profile semantics make the generic Realm abstraction dishonest.
> — `71491141`, `docs/RESEARCH_AGENDA.md:55`

`representation control` — zero. The hybrid (internal Position plus optional
Materialize/Dematerialize) is built and shipped three generations deep
(`Materialize` in 54 files) and the reject-criterion was never evaluated. Its
stated rationale — *"external venue compatibility"* — is also unrealized
(**M-52**).

### N-12. `COST_MODEL.md` §9 — the 22-row matrix that gates every byte layout

> **Before choosing byte layouts or claiming cheapness, measure** with the exact
> pinned SBF and Token-2022 versions: [22-row matrix]
> — `71491141`, `docs/COST_MODEL.md:205`

The tree's own lab agrees it never ran
(`docs/implementation/COST_LAB.md:235`): *"There is still no measured CU, heap,
stack, account-copy, write-contention, or landing figure in any arm"* and *"the
differential comparison against a pinned Solana SDK serializer still has to
happen **before any packet or lock conclusion is drawn**."*

**OBSOLETE for gen-1's layouts** — they are deleted. **Live for gen-3's**, which
re-chose ~590 byte offsets under the same absence (**M-36**).

### N-13 … N-16. Parks whose trigger fired, and one control

| ID | Gate | Verdict |
|---|---|---|
| **N-13** | *"**Per-order cancellation / continuous-claims scouting** stays parked **until the above land**"* — `NEXT_WAVE_ROADMAP:73` | **TRIGGER FIRED, PARK NEVER LIFTED.** Phase S 1–4 all landed 2026-08-21 (`47c7a77a`, `cd54bb72`, `41c231f6`, `525ec13f`, `df1d99e1`); the generation turned over three days later. **See G-3.** |
| **N-14** | *"the R4 §8 reference-ownership fork… is **explicitly deferred until the provider-horizon evidence exists**"* — `ADOPTED_2026-08-20` item 7 | **DEFERRED ON A MEASUREMENT NOBODY OWNS.** `provider horizon`: zero. The fork itself is obsolete; the **archive paging question it gated is not** — it is `F-8`'s missing half (**M-10**). See **G-12**. |
| **N-15** | *"the composite fee base's characterization **formalized before any rate freezes**"* — `NEXT_WAVE_ROADMAP:98` | **UNFIRED, UNOWNED — and now doubly so given N-1.** No rate has frozen, so whoever answers **M-26** trips a formalization precondition that exists in no gen-3 document. One line, attached to M-26. |
| **N-16** | *"opt-z is **refused until re-greened and gate-campaigned at its own identity**"* — `ADOPTED_2026-08-20` item 5, restated with its trigger at `MACRO_AND_MICRO_OPTIMIZATION.md:319` (*"**unless a real rent-per-byte bill appears** (deployment)"*) | **THIS ONE IS CORRECT, AND IS THE CONTROL.** It is `D-17`; it carries an explicit reopen trigger; the trigger has not fired; the successor kept it. N-13 and N-14 differ from it in exactly one respect: nobody was watching. |

### N-17 … N-21. The rest, compactly

| ID | Gate | Verdict |
|---|---|---|
| **N-17** | *"Reverse-Dutch bounty step count and **measured SOL cost quantiles**"* — `docs/OPEN_QUESTIONS.md:102` | **mechanism OBSOLETE, measurement STILL OPEN.** What a repair or resolution costs in SOL, at what quantiles, is unmeasured in both generations — while **M-51**'s zero-volume survivability and the prepaid-capability story both rest on it. See **G-11**. |
| **N-18** | Rung W2 promotion — *"**Every named id retiring refuses, so the rung is re-decided rather than silently upgraded**"* (`396e11de`); last word at `df1d99e1`: *"the audit says which and how far, **as input to a decision this lane does not make**"* | **NEVER DECIDED; ladder replaced.** The rung is obsolete; the **mechanism** is the finding — a promotion gate that *refuses* on a retiring blocker is machinery for `WAVE.md:470`'s *"Never-executed is the default"*, which gen-3 holds by convention instead. Worth stealing back. |
| **N-19** | *"pinning the exact Token-2022 program artifact (register F5)"* — `docs/OPEN_QUESTIONS.md:74` | **UNFIRED.** gen-3 has a behavior profile and a `PROVENANCE.md` — more than gen-1 — but no pinned upstream artifact identity. Ancestor of **M-44**'s predicted Meteora-DBC blindness, one dependency over. |
| **N-20** | The mock-ELF two-path build experiment — `8c2465d9` | **OBSOLETE, and the most instructive row here**, because the commit says exactly why it never ran: *"NOT MEASURED HERE… The two-path build was gated on the suite spinlock being free; it was held by another lane at both checks, with two SVM suites running and load above ten. **The swarm has priority. The experiment is left specified instead** — three steps, a falsifiable prediction… and the instruction to record the result as an observation."* A measurement lost a resource race, was written down perfectly, and was never re-queued. **Nothing in either generation re-queues a deferred measurement.** |
| **N-21** | The four gen-2 rent-format successors — ReceiptPageV1 (*"a projected saving of 935,201,280. **Measure the extra write contention and CU before adopting it**"*, `f1e8945c`), active-width ClearWork (232,296,960/candidate), embedded FundingTailV1, dynamic CandidateFeed; plus `502d9ad0` experiment 3, *"**Maximum-width V2 must match V1.**"* | **OBSOLETE.** Every guarded family died in THE PURGE (`D-20`). Recorded because ~1.9 billion lamports of projected saving were never measured, never adopted, and never retired by decision — they simply stopped existing, which is the shape this whole section is about. |

### The one honest never-run gate

**N-22 is not a defect and is recorded so the section stays honest.**
`docs/research/DUAL_IS_THE_MEASURE.md:1358` (`d34120ad`), mirrored publicly at
`site/clearing.html:202`:

> **Lean instantiation** of `Market.CertF` at the capped clearing matrix, plus
> the accept-set zero-gap theorem over the real `relation_v1.rs` semantics —
> **the promotion gate for any optimality language.**

`CertF`, `zero-gap` — zero in the successor. **The gate is unmet and the
restriction it imposes is still in force**: *"best valid submitted"* remains the
language in nine files including `AGENTS.md` and
`crates/dclutch-general-adapter-contract/src/runtime_verify.rs`. This is what a
never-run gate looks like when the system respects it.

---

# §D. Gen-2's verb catalogue: 129 named coordinates, 57 successors, no retiring decision

**M-12** reads *"Gen-2 had 25 General verbs… Gen-3's `dclutch-general-codec::Action`
has seven."* Both halves understate it. The catalogue is
`~/dev/dragons-clutch/programs/solana-layout/src/registry.rs` — 3,649 lines,
nine populated `*Action` enums, every variant carrying a doc comment. Counted
directly:

| gen-2 enum | coordinates | gen-3 successor | delta |
|---|---:|---|---:|
| `GeneralV2Action` | **42** | `dclutch-general-codec::Action` — 7 | −35 |
| `DealerFacilityAction` | **21** | `MultiLpActionV3` — `Add`/`Remove` | −19 |
| `DirectMarketAction` | **13** | `DirectExecutionActionV3` — 14, *different mechanism* | book/auction half gone |
| `RecoveryAction` | **13** | none | −13 |
| `SourceSeriesAction` | **12** | spec + resolution state, no verb set | −12 |
| `FractionalRedemptionAction` | **10** | none as a verb set | −10 |
| `StructuredClaimAction` | **8** | `StructuredAction` — `Issue`/`Unwrap` | −6 |
| `RecurringSeriesAction` | **6** | kernel functions, no verb set | −6 |
| `DealerPolicyAction` | **4** | `ClaimAction` — 3 | −1 |
| **total** | **129** | **~57** | |

plus the 62-variant legacy `Intent` monolith in `programs/solana-layout/src/lib.rs`,
whose *namespace* `COMPOST.md` explicitly forbids recreating (*"do not recreate
the 46-slot foundation or **cumulative action namespaces** by inertia"*) and
`O-002` and ADR 0003 retire on the merits. **That prohibition is about the
namespace. Nothing in `COMPOST.md`, `OMISSION_INDEX.md`, `WAVE.md` or
`docs/decisions/` retires the capabilities below — with exactly one exception,
ADR 0009, which retired gen-2's order-index plane on the merits this afternoon
and is the model for what the rest would look like.** Against `WAVE.md:329`'s
standing rule — *"everything named gets actioned or explicitly retired"* — this
is the largest body of named-and-neither in the project, and it costs one file
to read.

Every absence below was verified by grep against the successor tree, excluding
`node_modules`, `target/`, and this ledger itself.

## §D.1 — Ranked by how much the intention transfers

### 1. `ClearWork` — gen-2's structural answer to the fourteenth wall

`InitClearWork`, `GrowClearWork`, `AdvanceClearOrders`, `AdvanceClearSlices`,
`CompleteCandidateVerification`: a growable work account that verifies a
candidate **across many transactions**. **Verified: `ClearWork`, `AdvanceClear`,
`GrowClear` — zero in the successor.** Gen-3's General clears in one instruction,
and `WAVE.md:115` prices it at 1,336,865–1,386,359 CU with **one draw in twenty
exceeding 1,400,000 outright**.

**Sharpens M-24**, which frames the fourteenth wall as a record-layout decision
(*"store each canonical bump in its record"*). Gen-1's verdict was that width
growth goes through staging or succinctness (**G-2**); gen-2 *built the staging*;
`P-003` already blesses *"staged computation certificates"* as a lifting path;
and no lane owns it. Three generations have now met this wall, and the two
earlier ones answered it architecturally.

### 2. The price-measure admission certificate — the second half of G-1, and gen-2 had it too

`clutch-price-measure` (8,843 Rust lines): a Bernstein-moment continuous checker
**and** a quantized checker over bounded integer atom mixtures — a gate that an
admitted price vector comes from a nonnegative measure. Branches
`quantized-atom-mixture-certificate`, `exact-quantized-atom-solver`,
`score-v2-quantized-*`, and `general-v2-action10-closed-tuple` (*"Require
quantized admission before General ranking work"*).

**Verified: `atom mixture`, `quantized admission`, `price admission`,
`MomentCone`, `Bernstein` — zero in the successor.**

So **G-1**'s gate existed in gen-1 (as the moment cone, wired to `basis_degree`
on chain) **and independently in gen-2** (as the quantized atom-mixture
certificate, required before ranking work), and gen-3 has neither.
**`O-013` substituted for the *basis*. Nothing substituted for the *price-side
admission certificate*, in either direction, and no decision says so.**

### 3. The windowed-statistic fold pipeline

`SourceSeriesAction`: `OpenRawPage` → `IngestBoundaryBatch` → `SealRawPage` →
`InitializeWindowWork` → `FoldWindowPages` → `SealWindow` → `EvaluateStatistic`,
over `clutch-accumulator` (2,419 Rust lines) — an **associative interval-summary
monoid** with `combine`/`append`, `price_time_integral`, `twap`,
`relative_terminal_to_twap`.

**Verified: `RawPage`, `WindowWork`, `FoldWindow`, `EvaluateStatistic`,
`BoundaryBatch`, `price_time_integral` — zero.** Gen-3 has seven `StatisticKind`s
over a bounded observation slice and no self-computed integral; `TWAP` survives
only as a *deferred Pyth adapter* in `DESIGN.md:111`, a much narrower thing.

**Sharpens M-3 item 2** decisively. The ledger verdicts *"exact families of path
properties — extrema, crossings, drawdown, coverage, **integrated price**,
volatility summaries… One permissionless feed can service hundreds of markets"*
as **"No."** The honest verdict is **"built in gen-2 as a fold monoid, then
dropped."** It also sharpens **M-17**: the cadence-tolerance lift
`OddScheduledMedian` needs is downstream of a fold pipeline the successor no
longer has.

### 4. The Failure / Recovery family — 219 subjects, two crates, 13 verbs

`clutch-failure-policy-runtime` (15,569 Rust lines) and `clutch-evidence-recovery`
(3,665 lines). Gen-3 **carries the Source half** — `SourceRecoveryPolicyV2`'s
four funded attempts with deadlines and allocations,
`ResolutionKind::{Occurrence, Failure, Recovery}`, the
`CommitFailure`/`AwaitingFailure`/`FailureCommitted` phases. It does not carry:

- **`TriggerRelationRefusal`** — a failure trigger on the *clearing relation's*
  deterministic refusal, distinct from source failure. Zero.
- **`BeginIntervalConsensus` / `Advance` / `Resolve` / `CloseIntervalConsensusWork`**
  — a bounded-chunk consensus lifecycle for ambiguous evidence, paid through
  liveness. **`IntervalConsensus`, `interval consensus` — zero.**
- **The Failure *session* object** — `InitializeFailureRoot`/`CloseFailureRoot`,
  session archives, exhausted-session closure, zero-payout sessions. Zero.
- **"recoverable dormancy" as a market outcome** — the stated terminal of
  `clutch-failure-policy-runtime`, and the same phrase gen-1 used in its ratified
  `EvidenceOnlyRecoveryV1` decision (*"A market without evidence-selected weights
  degrades to recoverable dormancy"*, `docs/OPEN_QUESTIONS.md:26`). Zero.

`U-006` names *"Source/Resolution creation, recovery, terminal admission, funding
closure"* and does not reach a market-level failure root or interval consensus.

### 5. `RevenuePolicyV2` — M-26 was attempted and discarded, not never-attempted

Thirteen gen-2 subjects name it: *Define immutable fee-bearing revenue policy
V2*; *Allocate immutable Realm revenue record V2*; *Freeze streamed recipient
allocation V3 authority*; *Keep revenue policy V2 open to registered
calibrations*; *Bind treasury custody to General runtime and replay*; *Count and
retire treasury Position service*; `CloseRevenuePolicyRecord`; *Promote selected
fees to RevenuePolicyV2*.

**Verified: `RevenuePolicy`, `recipient allocation`, `fee manifest` — zero.**
Gen-3 keeps a per-venue `fee_recipient` pubkey.

**Sharpens M-26 and N-1 together.** The ledger reads the fee question as one
ember has never been able to answer. The record is worse: gen-1 selected the
*shape* and eliminated flat (**N-1**); gen-2 built the *record, the streamed
allocation authority, the calibration hook and the treasury custody*; and gen-3
ships a flat `fee_basis_points` with none of it and no decision record.

### 6. Exact portfolio value and simplex risk certificates

`clutch-market-quality` (2,233 Rust lines): exact integer portfolio value; the
complete-set floor as a guaranteed minimum; an exact conservative cap over the
full simplex; canonical portfolio compression; a simplex price-disagreement
bound tied to price provenance.

**Verified: `MarketQuality`, `portfolio value`, `risk certificate`,
`portfolio compression` — zero, and no omission row names them.** This is the
natural substrate for `U-004`'s scenario-solvent half (**M-11**) and for any
honest portfolio display — the successor's `/portfolio` route renders addresses,
not value.

### 7. Candidate-side work economics — the solver bond set

`ClaimCandidateBond`, `ClaimCandidateWork`, `ClaimSolver`, `ClaimEpochUnused`,
`MarkWorkClosed`, `ExpireCandidate`, `CleanupCandidate`; plus branches
`candidate-cost-certificate` and *Fund retained Direct candidate bonds*.

**Verified: all seven — zero.** **Sharpens M-50 and G-7.** The permissionless
work economy reads in the ledger as a gen-1 aspiration; gen-2 allocated the full
solver-bond-and-claim verb set for it, which is also the missing half of **G-7**'s
withholding question.

### 8. Global liveness allocation

`clutch-liveness` (4,848 Rust lines): checked fixed-memory admission arithmetic
proving named finite payments are covered *at admission*. Branches
`direct-global-liveness-callable` [46 ahead], `product-direct-global-liveness`,
`liveness-runtime-v2`, and *"Bind Direct work to gapless liveness receipts."*

**Verified: `global liveness`, `liveness allocation`, `liveness receipt`,
`gapless` — zero**; `liveness` survives as per-capability prepay across 59 files.
**Sharpens M-51**, filed as a gen-1 intention (*"there is no global
`LivenessPolicy` and no protocol-wide no-stranding result"*): gen-2 built the
arithmetic that would give one.

### 9. Owner settlement aggregation and virtual receipts

`clutch-owner-settlement` (12,652 Rust lines) plus `FinalizeOwnerSettlement`,
`InitializeSettlementRoot`, `AccountReceiptEnd`,
`ConsumeVirtualSplitReceiptEggs`, `ConsumeVirtualMergeReceiptEggs`,
`ConsumePortfolioPairEggs`, `ReleaseUnfilledReservation`,
`TransferPositionAssets`, `FinalPot`, `SettlementReceipt V3`, `OrderPage V5`.

**Verified: `FinalPot`, `OwnerSettlement`, `SettlementRoot`, `VirtualMerge`,
`PortfolioPair`, `TerminalOwnerFloor`, `unfilled reservation` — all zero.**
Gen-3's `InitializeSettlement`/`Collect`/`Materialize`/`Distribute` carries the
*shape*; the owner-level aggregate-then-convert-once invariant — which is exactly
what gen-1 spent three commits deriving and proving (`c36b3ceb`, `cd54bb72`,
`41c231f6`) — the virtual split/merge netting, and the unfilled-reservation
release do not exist.

### 10. The Realm-selected collateral adapter release and policy

`clutch-collateral-adapter-v2`: a canonical release record binding parser/CPI
code, external token deployment, account layouts and an exact-visible-atom
theorem; and a policy *selected by an immutable Realm* binding mint, token
program, decimals, **supply ceiling and market-cap ceiling**. Branches
`collateral-release-catalog` [267], `collateral-adapter-v2-runtime` [191].

**Verified: `SupplyCeiling`, `supply ceiling` — zero**; gen-3 has a hardcoded
`Token2022BehaviorProfileV2`. **Sharpens M-10 / `F-6`**: the frontier's
*"versioned selectable profile record"* is not a design the successor has yet to
invent — **gen-2 shipped it**, and this is also **G-8**'s two-Realm genericity
question with a working answer attached.

## §D.2 — The rest, compactly

| gen-2 capability | verbs / crate | successor |
|---|---|---|
| **Structured descriptor lifecycle** | `CreateDescriptor`, `WrapCanonical`, `WrapFull`, `UnwrapCanonical`, `UnwrapFull`, `CompactDonation`, `RedeemTerminal`, `RetireDescriptor` | `StructuredAction` = `Issue`/`Unwrap`. `U-008` names wrap/transfer/unwrap/redeem/retire as owed; **the descriptor *object* and the canonical/full distinction are named in neither `U-008` nor `O-014`** |
| **The Fractional credit ledger** | `RedeemInternalCredit`, `RedeemBearerCredit`, `TransferCredit`, `MergeCredit`, `CloseZeroCredit`, `SealClaimsExhausted`, `CloseEmptyLedger` | all zero. `O-012` admits *"explicit Token-owned remainder/change instruments"* as valid; gen-2's answer was an aggregate credit ledger, which `O-012` neither adopts nor rejects |
| **The Dealer sponsor funding phase** | `CreateLpPage`, `Contribute`, `WithdrawFunding`, `CancelFunding`, `RefundCancelledSponsor`, `Activate`, `BindEpoch`, `LapseEpoch`, `SelectLeaseAndBegin`, `AbortBeforeCollection`, `QueueExit`, `SponsorHalt`, `TimedClose` | `MultiLpActionV3` = `Add`/`Remove`. `O-011` and `U-004` cover repricing consent and scenario solvency; **neither covers a funding phase with sponsor cancellation, refund, halt, or a permissionless deadline close** |
| **Direct's sealed-book auction** | `AdmitOrder`, `FreezeBook`, `SubmitCandidate`, `BeginVerification`, `VerifyCandidate`, `FinalizeSelection`, `SettlePair`, `LapseEmpty/Unselected/Selected` | zero; gen-3 Direct is inline/registered fill. A legitimate architecture change — and **`U-002` describes the successor without naming what was retired**, so no record states it |
| **The exact order/candidate index plane** | `InitOrderPage`, `CloseCandidateIndexPage`, seven branches incl. `exact-index-live-integration` [417] | `OrderPage`, `IndexPage`, `candidate index` — zero, **and deliberately so as of today**: `collection_v1.rs` (`751d702`) answers it — *"a batch is a **window**, not a ledger… it does not enumerate them"* — with `GeneralBatchV1`/`GeneralOrderV1` as content-addressed records whose digests *are* the ids, ruled in ADR 0009. A gen-2 capability **retired on the merits, in writing**. Recorded as the counter-example: this is what the other fifteen rows are missing |
| **`Endow` / `WithdrawCash`** | owner-level internal trading cash into the Hoard; unreserved-cash withdrawal | **zero — and this is the sharpest single row in §D.** It is exactly the gap **M-13** quotes from gen-1 `01d00083`: *"no endowment instruction (**the sharpest gap** — opening cash is the one unwritten field)."* **Gen-2 closed it. Gen-3 reopened it.** The ledger cites it as an example of unswept gen-1 debt; it is really an example of a closed gap lost at a generation boundary |
| **Chunked on-chain artifact staging** | `BeginArtifact`/`WriteArtifact`/`SealArtifact`/`AbortArtifact` — the transport gen-2 used for Dealer policy catalogs and Source archives | zero. Adjacent to **M-3 item 5**: not a quoting-policy compiler, but the artifact seam one would need |
| Also zero, named | `ScorePolicy`/`RankKey` two-window rank binding; `Market family aggregator`; `multiboundary` source rollback schema; operator draft invalidation after finalized rescan | — |

## §D.3 — A correction to the branch-absorption test

**M-58** and `BRANCH_TRIAGE_2026-08-22.md` measure a branch by *"is it in
`main`"*, and found *"97–100% line-level absorption on every branch"* over the 16
to 27 they covered. For gen-2 that test answers the wrong question twice:

1. **`main` is compost.** Reaching `main` does not mean reaching gen-3 — the
   successor is a subtree, not a descendant.
2. **45 of 361 refs are 0 commits ahead of `origin/main` and their capabilities
   are still absent from the successor** — `quantized-interval-consensus`,
   `exact-index-builder-frame`, `fractional-credit-actions`,
   `nonzero-fee-economics`, `production-payoff-compiler`,
   `structured-claim-runtime`, `settlement-generality-runtime`,
   `source-plane-v3-runtime`, `liveness-runtime-v2`,
   `general-v2-streaming-traversal`, and 35 more. Perfect absorption into a dead
   tree.

Also worth noting: **`product-theory-redesign` is now 530 commits ahead** and
still unmerged — the branch **M-8** identifies as carrying seven concrete product
upgrades that *"the successor never received."*

The triage's own reassurance is therefore about line survival in the compost
repository, not about intent survival into the successor. That is the whole
distinction this addendum is about.

---

# §E. Rows this sweep sharpens

Compact; the evidence is a commit or a committed gen-1 document that the audit's
sources did not carry.

| Row | Sharpening |
|---|---|
| **M-1** | See **C-2**. Also: `SWARM_ROADMAP` §6 gives *Breadstuffs* a five-step plan whose step 5 is *"only then the expensive proof freeze/MPC ceremony"* — which is precisely the trusted-setup condition `SUCCINCT_CLEARING_FEASIBILITY` names as one of two gates on **M-55**. The two documents interlock and neither has a successor. |
| **M-3** | The twelve-item ceiling is not the only ambition statement. `INSTRUMENT…` §9 independently specifies five of its items (product-space compiler; cross-Series rolls; cross-market vaults; certificates as artifacts; agent artifacts) as *design directions with mechanisms*, and §5 makes item 2's price coherence **mandatory** rather than aspirational: *"Smooth-market price coherence must become mandatory."* |
| **M-4** | Three sharpenings. (a) `SWARM_ROADMAP` §7 lists, as an *attractive distraction to refuse*, **"categorical lowering presented as smooth-product support"** — written 2026-08-19, and it is the exact risk `O-013` runs; the audit says ember *"should get to say whether that is the same thing"*, and gen-1 already wrote down that it would not be. (b) The gate `O-013` does **not** substitute for is the price-side one, and gen-2 had it independently (**G-1**, **§D.1 item 2**). (c) Superseded in part by `docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md`, which answers M-4's question directly (*"is the successor now at least as capable as generation one on shaped dynamics?" — "**no, not yet**"*) and is the row's live owner. |
| **M-4**, earlier still | `a81b609b`, 2026-08-18, `research/claim-algebra-model/ONE_HOT_VS_DERIVED.md` is the dated head-to-head that chose one-hot **on argument** and gated the derived branch's promotion on a benchmark that never ran — **N-7**. |
| **M-5** | See **G-10**. The Aug 24/26/27 calendar is in a committed decision record, not only in `cv`. |
| **M-8** | `PRODUCT_THEORY_REDIRECTION`'s seven upgrades are largely gen-1's `INSTRUMENT…` §9 re-derived; the gen-1 original is committed on `main` and was never swept, so the successor lost the same content twice, by two different mechanisms (branch-only, then generation boundary). |
| **M-16** | See **C-4**. `κ` is a gen-1 P1 row, so it has survived two generations. **RESOLVED 2026-08-31 (KAPPA-CAP)** — enforced at founding and all three growth routes, refusal named, and demonstrated refusing on a real ELF at the affine-batch site; κ = 1/4 still Provisional, and the other three sites still found unbounded in their fixtures. |
| **M-55** | See **G-2** and **N-9**. Succinct verification was *move 2 of exactly two* in gen-1's strategy document, its two gating quantities are named and explicitly must-not-be-estimated, and `SWARM_ROADMAP` §6 makes Breadstuffs' *"expensive proof freeze/MPC ceremony"* step 5 of its own five-step plan. |
| **M-26** | Two sharpenings. `3e818be8` and `15122506` show the treasury was made **structurally** undecidable — *"the treasury pubkey **DEFERRED** as the structural `REVENUE-TREASURY-UNSET-SENTINEL1` byte string"*, and `525ec13f` makes `RevenueTreasuryUnset` fire *"on EVERY fee-bearing admission… **unreachable until ember binds a key in a new const**"*. Gen-1 built a protocol that could not take a fee until ember answered. And three more: **N-1**, the base geometry silently changed under the rate question; **N-15**, a formalization gate stands in front of it; and **§D.1 item 5**, gen-2 built `RevenuePolicyV2` — the immutable fee-bearing Realm record, streamed recipient allocation V3, registered calibrations, and treasury Position custody, all zero in the successor. M-26 reads as never-attempted. It was selected in gen-1, built in gen-2, and discarded in gen-3, three times without a decision record. |
| **M-46** | Gen-1 had the abandonment problem too and solved it structurally rather than socially: `SWARM_ROADMAP` §8 rule 4 requires *"each lane to report: exact paths, commit, evidence plane, commands, test counts, artifact identity, negative boundaries, and remaining STOPs."* A lane that must report an artifact identity cannot vanish silently. Gen-3's `AGENTS.md` carries `semantic owner` (§8 rule 3) and none of rule 4. |
| **M-10 / `F-6`** | The frontier's *"versioned selectable profile record"* is not a design still to be invented: gen-2's `clutch-collateral-adapter-v2` shipped it, with a canonical release record binding parser/CPI code and layouts, and a Realm-selected policy carrying **supply and market-cap ceilings**. **§D.1 item 10**, and it is **G-8**'s genericity question with a working answer attached. |
| **M-17** | The cadence-tolerance lift `OddScheduledMedian` needs is downstream of a fold pipeline the successor does not have: gen-2's `RawPage → WindowWork → FoldWindowPages → SealWindow → EvaluateStatistic` over an associative monoid. **§D.1 item 3.** |
| **M-50** | `0e1bc44d` is the commit behind the audit's quote, and its full form is stronger: *"Three things it deliberately does not do. **Candidate submission is solver work, not a crank**, so the quote table carries no row it never spends against."* The keeper was built with the solver boundary drawn deliberately, not left out. And see **G-7** for why the solver half was known to be a mechanism-design problem. |
| **M-3 item 2** | **Built and dropped, not uncontemplated.** `clutch-accumulator` (2,419 Rust lines) is an associative interval-summary monoid with `combine`/`append`, `price_time_integral`, `twap`, `relative_terminal_to_twap`. See **§D.1 item 3**. |
| **M-11** | Two: **N-8**'s *"the liveness-policy owner **must derive and measure**"* budget vector, and **§D.1 item 6** — `clutch-market-quality`'s exact portfolio value and full-simplex conservative cap are the natural substrate for `U-004`'s scenario-solvent half. |
| **M-12** | Gen-2 had **42** `GeneralV2Action` verbs, not 25 — counted directly from `registry.rs:1902`. And see **G-3**: cancellation is separable from collection, was separately named and separately parked, and Direct's `CancelThroughV2` proves the intention transfers. |
| **M-13** | Three. The 5,106 is 3,504, and 1,568 of those are unique subject lines (§ *the corpus*). The four workstreams are an 85-line roadmap (**C-3**). And **the "sharpest gap" it quotes — `01d00083`'s missing endowment instruction — was closed in gen-2** (`Endow`, `WithdrawCash`) **and is open again**: see §D.2. |
| **M-24** | Two. Gen-1's surviving verdict is that *width* growth goes through staging or succinctness (**G-2**); gen-2 **built the staging** as the `ClearWork` growable work account (**§D.1 item 1**); `P-003` already blesses staged computation certificates. Three generations have met this wall and the two earlier ones answered it architecturally. |
| **M-51** | `clutch-liveness` (4,848 Rust lines) is checked fixed-memory admission arithmetic proving named finite payments are covered at admission — the protocol-wide result M-51 says does not exist and is not scheduled. **§D.1 item 8.** |
| **M-58** | Two corrections. This sweep read all 3,504 commits across every ref, so branch *content* is now swept. And the triage's absorption test does not apply to gen-2: `main` is compost, and **45 of 361 refs are 0 ahead of `origin/main` with capabilities still absent from the successor** — perfect absorption into a dead tree. **§D.3.** Also: `product-theory-redesign` is now **530 ahead** and still unmerged. |
| **D-16** | `100e97ea`'s full wording is a better epitaph than the audit's excerpt: *"R5's 'install/pin Rocq and prove' bullet retired, noting **Rocq was in fact pinned and still produced nothing**… the file remains a specification with zero checked properties… **The named kernel properties are Lean's to prove.**"* |

---

# §F. ACTIONED and OBSOLETE — stated fairly

The successor carries more of gen-1 than a reader of the MISSING list would
guess. A sample, each verified against the tree rather than a document:

- **Mint closability.** `SOPHISTICATION_GAP` §4: *"outcome mints are 82 bytes
  with no TLV room, so `MintCloseAuthority` is unrepresentable and **they can
  never close**."* Gen-3 has `crates/dclutch-token-svm/src/closeable_mint.rs`,
  and `PROVENANCE.md` records the deliberate profile: *"immutable self-pointing
  `MetadataPointer` plus immutable, fully consumed `TokenMetadata`."*
  **ACTIONED**, and it also answers P3's *"bare versus immutable in-mint
  metadata"* row by construction.
- **The static client.** `SOPHISTICATION_GAP` §4: *"No client capable of reading
  the chain: the static client ships `connect-src 'none'` by design."* P3:
  *"Static-client framework and wallet adapter with no hosted backend."*
  **ACTIONED** — `apps/dclutch-web`, Wallet Standard discovery, Talisman
  confirmed.
- **"Best valid submitted candidate."** Requirement 6, kept verbatim in
  `dclutch-general-codec::Action::Freeze`'s doc comment. **ACTIONED.**
- **The two-window candidate lifecycle.** Gen-1 ADR 0006's *"two exclusive slot
  boundaries after the book freezes"* is gen-3's `Consider` → `Freeze`.
  **ACTIONED** — while its commit/reveal subdivision is not (see **G-7**).
- **Error-code consolidation.** Three gen-1 commits queue it; ADR 0007
  (namespaced refusal codes) discharges it. **ACTIONED** (**G-21**).
- **Internal venue ownership** (P0). **OBSOLETE** by ADR 0003's five-role
  partition — with the caveat in §C that the partition's own measurement never
  ran.
- **Rocq.** **OBSOLETE** by ADR 0005 / `D-16`.
- **The whole sealed-ELF apparatus** — `svm_run.txt` regeneration, the two-then-
  thirteen rustdoc warnings riding a reseal wave, the relocated-Cargo-home
  probe, `audit_artifact.sh`'s declared closure, the four cheap reseal riders at
  `MACRO_AND_MICRO_OPTIMIZATION.md:400`, the default profile's ELF identity
  matching no manifest key pattern (`710341bd`). **OBSOLETE** — gen-3 replaced
  seal-identity with the checked-release pipeline. Roughly a third of gen-1's
  entire debt vocabulary lives here, which is the honest reason gen-1's commit
  prose yields 41 open rows rather than 200.
- **The gen-1 settlement blocker ledger** — `PartialFillLedger`, `VirtualPot`,
  `VirtualMergeCredit`, `TerminalClosure`, and the ten retired before them.
  **All closed inside gen-1**, each with a bank campaign; `f9871af3` is the
  commit that records the list empty. Nothing was handed forward unfinished.
  This is why gen-1's *own* debt discipline is not the finding — the finding is
  everything that was in a **register or a review** rather than a blocker row.

---

# §G. Findings about the instrument itself

**1. The debt that survives is the debt that was never in a ledger.** Every gen-1
row that reached `SETTLEMENT_BLOCKERS` or the terminal inventory's blocking ids
was closed, with evidence, inside gen-1. Every row that lived in
`docs/OPEN_QUESTIONS.md`, a decision record's *Deferred* section, a roadmap
phase, or a review's §9 is still open. That is the same shape as the audit's
finding **1** (*"the unrouted items live in the wrong ledger"* — `blocked.json`
survived, board prose did not) — one generation earlier, with a different pair
of ledgers. **The mechanism is not the medium; it is whether a row is in the
thing a gate reads.** `blocked.json` and `SETTLEMENT_BLOCKERS` are read by
tests. `OPEN_QUESTIONS.md` is read by people.

**2. The generation boundary is a lossier interface than any lane handoff.**
`M-7` counts 489 lane charters lost to encryption and calls reconstruct-from-a-
name the failure mode. This sweep found the larger loss and it needed no
cryptography: **four committed, on-`main`, plain-text artefacts — the intent
archaeology, the instrument review, the next-wave roadmap, and a 3,649-line
action catalogue with a doc comment on every one of 129 verbs — carrying most of
Tier 0, all of gen-1's decision deferrals, a written next step for the dark
programme, and the entire named surface of gen-2, did not cross a subtree
merge.** `COMPOST.md` explicitly permits recovering *"user intent and product
requirements"* from the compost tree. Nobody did, because nothing pointed at
them.

**M-7 is therefore too pessimistic and the audit's Tier 0 is too pessimistic in
the same way.** The intentions were not lost to encryption or to memory. They
were written down carefully, in the repository we still have, and then a subtree
merge became a wall nobody thought to look over. The cheapest durable repair in
this whole addendum is a pointer: **`~/dev/dclutch/COMPOST.md` should name those
four files by path**, which turns three of this addendum's four corrections into
things that cannot recur.

**3. Nothing in either generation re-queues a deferred measurement.** Eleven of
the 78 gates ran; every one ran because the lane that named it also ran it, in
the same wave. Of the 43 that did not, not one was picked up by a later lane —
`N-20` is the clean specimen, a measurement that lost a resource race to the
swarm, was written down perfectly with three steps and a falsifiable prediction,
and was never seen again. `blocked.json` holds *routes*; nothing holds
*measurements that are owed*. That is a missing row type, not a missing lane.

---

# §H. What this evidence would support

*"Weigh this ledger as evidence, not obligation: a mention is not a commitment."*
So this is not a queue and nothing below is owed. It is the shortest list of
moves this evidence would support **if someone wanted them**, ordered by what a
miss would cost, and deliberately disjoint from the audit's ten. Two of them
(1 and 3) are cheap enough that the cost of skipping them is the only argument
for doing them; the rest are genuinely optional.

1. **Before choosing between the scorecard's two options for the degree-≥2 price
   gate, look at gen-2's third one** (`G-1`, **§D.1 item 2**). The gap is already
   found and owned by `BSPLINE_ECLIPSE_SCORECARD`, which frames the choice as
   *"port a sound-but-incomplete gate, or do the per-span Hausdorff witness
   generation one designed and never built."* Gen-2's `clutch-price-measure`
   carries a **quantized** atom-mixture certificate over bounded integers — the
   posture `LiabilityBasisV2` is already in — and a two-way gen-1/gen-3
   comparison cannot see it. Half a day of reading before a design choice.
2. **Reconcile the fee base with the decision that chose it** (`N-1`,
   **§D.1 item 5**). Either a paragraph saying flat notional is the V1 base and
   why, or a row saying `fee_basis_points` is a placeholder for the composite.
   Ember delegated item 9 on the weakest-choice principle and it named flat
   *eliminated*; flat is what runs.
3. **Point `COMPOST.md` at four files** (`C-1`, `C-2`, **§G.2**). The cheapest
   durable act in this addendum:
   `docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md`,
   `docs/reviews/INSTRUMENT_AND_MARKET_DESIGN_REVIEW_2026-08-22.md`,
   `docs/reviews/SOPHISTICATION_GAP_2026-08-19.md`, and
   `programs/solana-layout/src/registry.rs`. Then **port** the first three,
   plus `docs/design/NEXT_WAVE_ROADMAP_2026-08-20.md` and
   `docs/OPEN_QUESTIONS.md`. This is most of the audit's recommendation 2,
   already written, by us — an hour of copying instead of an hour of `cv`.
4. **Read `registry.rs` once** (§D). 129 named coordinates, 72 with no successor,
   every one carrying the doc comment that says what it did. ADR 0009 is what one
   of them looks like when it gets a real answer — retired on the merits, in
   writing, this afternoon. A mention is not a commitment, so most of the other
   71 may deserve exactly the sentence *"we looked, and no"*; the point is that
   nobody has looked. One file, one afternoon, and it would inform `U-002`,
   `U-004`, `U-006`, `U-008`, `O-011` and `O-012` at once.
5. **Ask ember the sixth Tier-3 question** (`G-5`): upgrade posture. It is the
   only decision gen-1 formally deferred to him, it has a written recommendation
   *and* his own counter-principle recorded against it, and its second sentence
   binds the source today — *"Source code must support either deployment without
   pretending the former is the latter."*
6. **Open a licence lane** (`G-4`). The addendum's only outright regression:
   gen-1 ran a green SBOM over 36 manifests and left three families for human
   eyes; gen-3 has no instrument and a larger surface. AGPL's source-offer
   obligation attaches on distribution, and the Pages workflow now distributes.
   The owner already exists — *"counsel/security/licence engagements"* is on
   ember's own reserved list (`G-10`).
7. **Give owed measurements a row type** (**§G.3**). `blocked.json` routes what
   refuses. Nothing routes what was promised and not measured, which is why
   forty-three of them are here. Three are cheap enough to close this week:
   `N-5`'s two upstream fetches, `N-2`'s bounded outcome-cap sweep, and `N-9`'s
   Cert-F instrumentation against the wall that is standing right now.

### DECOMP-r, 2026-08-27: three rows the sweep opened

**M-45 — unexecuted code in a crate `hot_v3` links is a COMPUTE change, and
nothing in the tree says so.** Measured while landing `7ead0716`: adding 662
lines of preimage-builder code to `dclutch-execution-strategy-contract` took the
20-seed sweep from 18/20 to **0/20**, every seed dying at 1,399,944 of 1,400,000.
The new code was never called on the swept bundle -- proven, not assumed, by
stubbing all three wrappers to return an error and watching the path *pass* at
essentially the baseline rather than refuse. The cost was LLVM inlining the new
bodies into `hot_v3`'s callers and spilling everything else worse: **43,887 CU
from code that does not run.** `#[inline(never)]` on the six functions recovered
it and then some (20/20, and the two seeds main fails now pass), with a
byte-size-identical ELF. `hot_v3`'s own `hot_cu_checkpoint` doc already recorded
one instance of this mechanism; this is the second, and the first from
*unexecuted* code. **The row is the missing rule**: a lane that adds a helper to
any contract crate Trading links and verifies only "my function is not called"
has verified nothing. The gate is the sweep. `#[inline(never)]` is the cheap
prophylactic for anything cold living beside something hot. Whoever owns
`AGENTS.md`/`WAVE.md` lane guidance should carry this; DECOMP-r has only carried
it in one commit message and on the board.

**M-46 — main regressed from 20/20 to 18/20 on the compute gate and nobody
boarded it.** At `211079f6` the board records the sweep at 20/20, min 1,328,933,
max 1,372,433, worst margin 27,567. At `a4be9a83`, clean build, same sweep,
same harness: **18/20**, seeds 0 and 3 failing, max 1,395,229 with **4,771 CU of
margin** on seed 15. That is roughly +23,000 CU on the mean from lanes landing
between those two commits, and it is nobody's declared result -- every lane in
that window measured its own thing. `7ead0716` happens to take it back to 20/20,
which means this row is *masked*, not closed: the +23,000 is still in the path
and the next lane to add cold code beside the hot path will meet the ceiling
again. Owner: whoever takes the next CU lane. Bisecting `211079f6..a4be9a83`
against the sweep is the cheap version and it is one script that already exists.

**M-47 — `hot_v3`'s five Shadow-digest call sites still take the allocating
wrappers, and those heap-allocate what the streaming form did not.** The owed
second half of `7ead0716`. `runtime_transcript_digest_v3`,
`execute_admitted_candidate_v3`, `execute_shadow_candidate_v3` (twice) and
`accelerator_runtime_observations_digest_v4` should size the two buffers from
counts they already hold and take them from the scratch region the phase already
opens, after which `trading-sbf` can drop the crate's `alloc` feature entirely.
The cost of not doing it is exact: 37 + 80n bytes for n runtime observations
(20,517 at the accelerator's 256-account maximum, 6,277 at a 78-account frame),
13+8s scratch plus 32+16i slices for the candidate bank, and 17+8l+84r for the
effect projection -- against zero for the `Sha256` stack value it replaced. All
five are gated on a shadow caller, an admitted caller or the accelerator
boundary, so none executes on the canonical Interpreted Direct bundle and the
29,895-byte heap peak is untouched; on the paths that DO reach them this is
strictly worse heap than what it replaced. Not a fail-closed, not polish: debt
with a number.

### DIAG-82, 2026-08-27: two rows the 82-diagnostic regression opened

**M-61 -- the 20-seed sweep's per-seed CU is a bump-search lottery, re-rolled by
any change at all to the Trading ELF; M-46's bisect method has to know that.**
Measured across all twenty seeds, before and after a pure out-of-line refactor
(`9dc2a6bb`) whose real cost is one extra call: every per-seed delta decomposes
as `n x 1,500 + ~50`. The `~50` is the call (residual 46..56 on nineteen of the
twenty seeds). The `n x 1,500` is `find_program_address` -- up to 31 iterations,
a swing of +/-46,000 CU -- re-rolling because the trading ELF digest feeds the
identities the fixture derives, so **changing one byte of that ELF redraws every
seed's bump search**. Consequences, all of them practical: (a) "worst margin
8,238" was never a property of the code; the same tip with a 440-byte-larger ELF
measures 3,689, on a seed that was not the worst before, at 20/20 either way.
(b) M-46 tells the next CU lane to bisect `211079f6..a4be9a83` *against the
sweep*; done per-seed that will chase +/-46,000 CU of noise and attribute it to
whichever commit it lands on. The bisect statistic has to be the **pass count
and the twenty-seed mean**, not one seed's number. (c) A lane reporting a margin
should report the ELF digest beside it or the number does not mean anything.
Cheap fix available to whoever takes the next CU lane: have the sweep print the
trading ELF sha256 and the twenty-seed mean, so the lottery is visible in the
output rather than in this row.

**M-62 -- a feature-flag variant of a linked program is a DIFFERENT program, and
no gate in the tree treated it as one.** The rule M-45 is the sibling of. Five
stages count SBF frame diagnostics (`run.sh`, `run-journey.sh`, `run-dealer.sh`,
`run-general.sh`, `checked-release-candidate.sh`) and every one of them built
`dclutch-trading-sbf` at default features or did not build it at all. The
accelerators link it with `default-features = false` and their own feature set;
that is a different monomorphization with different inlining and therefore
different frames, and it carried 82 frame-overwrite diagnostics on
`hot_v3::execute_child_routes_v3` -- 5,184 bytes against a 4,096-byte bound --
from `3071fbe8` until the checked-release candidate went red on 2026-08-27 with
a devnet deploy in flight. The gate that caught it is the one that runs last.
Wired shut in `d1378427` for the frame class specifically (the Trading seam
runner and the Dealer tier now build the accelerator links; the release
candidate frame-checks every program under `programs/`, enumerated from the
directory so a list cannot go stale again). **What is NOT closed is the general
form**: no gate in this tree measures anything else -- heap, CU, ELF size,
account extents -- on a non-default feature set of a program another program
links, and a lane that reports "Trading is unchanged" today means Trading at
`default = ["families"]`. Owner: whoever next measures a Trading number that a
family accelerator also has to live with.

### POST-0012, 2026-08-27: two rows the slot-pin closing sweep opened

**M-63 -- a measurement can be structurally incapable of testing the thing it
is quoted for, and green tells you nothing about that.** Decision 0012's whole
claim is that the market life fits on a MUTABLE substrate, because the slot pin
replaces a ~700,000-CU whole-ELF hash with one `u64` comparison. PIN-0012
landed the admission and honestly named the claim as argued and unit-tested,
never measured end to end -- so the debt was recorded as *"run the sweep"*. The
sweep was then run, at HEAD, and came back **20/20, mean 1,345,302 of
1,400,000**. That number is real and it is not evidence for the claim, because
`waist::release` builds every release `Immutable` and `waist::immutable_programdata`
writes the ProgramData authority option as `None`, so
`slot_pinned_release_elf_digest_v1` always took its `Immutable` arm -- the arm
its own doc calls delegated *unchanged*, and which never hashed anything. **The
Hot tail never paid the hash, so it had nothing to save, and the `ExactAuthority`
arm that 0012 exists to add was not constructible by the fixture at all.**
Checkable rather than asserted: the pre-`0e34c036` sweep meaned 1,366,177 and
this one means 1,345,302; the 20,875 gap is about fourteen bump iterations,
inside M-61's +/-46,000 draw, and a 700k effect could not have hidden in it.
The general form, which is the row: **before quoting a measurement as evidence
for a change, name the branch the change added and check the fixture can reach
it.** A suite that cannot construct the case is not a suite that tested the case
and passed. This one had a second tell nobody read -- the numbers did not move
when the admission landed, which is what "the fast path was already free here"
looks like from the outside. Structural fix LANDED at `d20837fd`: a
`FixtureSubstrateV1` with an `ExactAuthority` arm AND an `ImmutablePinned`
control that isolates the M-61 redraw from the real cost, because the policy
byte, bound authority and bound slot all live inside the bytes the artifact id
hashes and therefore move every PDA seeded by it. Swept at `57138ba8` against
one ELF: 20/20 on all three arms, means 1,345,302 / 1,353,477 / 1,355,575, and
the answer is **73 CU** — see M-65 for how it was extracted, because the means
alone said something else.

**M-64 -- `lake build` was not a gate over the Lean library; it was a gate over
whatever the root module happened to import.** `ProtocolInfrastructure.lean`
carried two theorems -- `mutable_artifact_refuses`,
`mutable_core_registry_or_rent_refuses` -- that stated the OPPOSITE of the
shipped protocol after decision 0012 inverted them. They survived because
**nothing imported the file**: `lake build`'s 93 jobs never elaborated it, and
no gate in the repo ever had. Measured properly, 26 of 118 modules were
unreachable from the root; 22 were reachable from some `Emit*` exe (so CI did
elaborate them, invisibly to the default target) and **four had no builder
anywhere in the repo** -- `SeriesEscrowV3`, `SeriesReplayV3`,
`SeriesReplayPlanV3`, `RationalRepresentationV2Examples`, 40 theorems between
them, all four describing LIVE Rust in `dclutch-series-v3-kernel`, none of them
compost. Closed at `a7de18e5` with the import list AND, more importantly,
`globs = ["DClutchSemantics.+"]` on the `lean_lib`: membership is a pattern
now, so a new orphan is structurally impossible rather than dependent on
somebody remembering. 93 jobs -> 120, zero red, zero `sorry`, and every
emitter's `check-generated.sh` still `cmp`-clean. **The residual, stated
because the fix does not reach it**: elaborating green proves a proof is
internally sound, not that its STATEMENT matches the Rust.
`ProtocolInfrastructure` would have elaborated green for the whole period its
two theorems were backwards. Those 40 newly-covered theorems are now auditable;
they are not yet audited.

**M-65 -- when two measurements share their randomness, PAIR them; averaging
throws the answer away.** M-61 established that a per-seed CU figure is a
bump-search lottery and that the reportable statistic is `PASS n/20` and the
MEAN. Applied to the substrate arms, that rule alone produced a FALSE NULL.
`slot-pinned` meaned 1,355,575 against `immutable`'s 1,345,302: +10,273. The
`immutable-pinned` control -- same digest arm, same code, different release
identity -- meaned 1,353,477, so +8,175 of that gap was redraw and the
difference-of-differences was +2,098, smaller than the redraw it sat on. On the
"report PASS and MEAN" rule the honest write-up is "no signal above the
lottery", and decision 0012's headline number would have stayed argued.

But the three arms ran **the same twenty seeds against the same ELF**, so seed
*k* used the same fixture keys in every arm and the arms differ only by
bump-search depth plus a constant. M-61's own decomposition -- `delta = n x 1500
+ c` -- can then be solved PER SEED rather than averaged over:

    immutable-pinned - immutable   c = 0    (exactly 0 on 18/20, never past 6)
    slot-pinned      - immutable   c = +73  (67..77 on all twenty seeds)

The control's zero is the method certifying itself: identical code path,
identical constant, an 8,175 CU mean gap fully explained as `n x 1500`. The
answer to decision 0012 is **73 CU**, and it was recoverable exactly, on twenty
independent seeds, from data whose means said `2,098 +/- a lot`.

The general form, which is the row: **a lottery you cannot remove you can often
CANCEL.** Before reporting "the effect is inside the noise", ask whether the two
sides drew the SAME noise -- same seeds, same ELF, same fixture -- because if
they did, the noise subtracts and the residual is the measurement. And state the
scope with it: this pairing is valid only within one ELF and one seed set. Across
revisions the ELFs differ, the lotteries are independent, the pairing is
meaningless, and `PASS`/`MEAN` remains all there is. M-61 is not weakened; it is
the rule for the case where the randomness is NOT shared.

---

*Gen-1 did not fail to write things down. It wrote them down — in reviews, in a
register, in a roadmap, and in a catalogue with a doc comment on every verb — and
then we moved, and the move was the lossy part.*
