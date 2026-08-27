# Aspiration ledger — ARCH-EOL, 2026-08-27

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
in `WAVE.md` **zero** times. Four kernels are proved and consumed by nothing.

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

### M-4. The B-spline requirement: caught once by ember, regressed silently in the successor

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

| ID | Question | Where it sits | Recommended answer already written? |
|---|---|---|---|
| M-22 | **The first open Market cannot be redeemed.** Its aggregate is written and `custody_context` is not mutable — re-found at a new generation, or keep it as the recorded witness. *"Owner: ember."* | ADR 0008 §6.4 + board `:11058`; **absent from `WAVE.md`** | yes, two options |
| M-23 | **The reentrancy decision.** *"Needs ember or the protocol owner."*, following *"NO CHILD ROUTE CAN EXECUTE UNDER A REGISTRY CONTINUATION"* | board `:8950`; `WAVE.md:423` records the *wall* as down, not the decision as made | partly |
| M-24 | **The record-layout decision behind the fourteenth wall.** The shipped path spends 1,336,865–1,386,359 CU and **one draw in twenty exceeds 1,400,000 outright** | `WAVE.md:117` — correctly stated, not routed | yes: store each canonical bump in its record |
| M-25 | **Does a checked release describe the artifact or the account?** Revocation is mandatory on deploy day, so every deployed role will be in the state the release cannot describe. *"Reported, not patched."* | `FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md:193` — disowned with no recipient | no |
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
| M-32 | **`GENERAL_CANDIDATE_PAGE_PDA_DOMAIN_V1` is 33 bytes** — same defect class that made an entire transition dead at runtime; only Custody has the seed-length assertion | *"unowned"* |
| M-33 | **`core-sbf/src/tests.rs:141` measures a frame 13 accounts narrower than the real one**, so its packet claim is understated | *"Core owner"* — never claimed. `WAVE.md:399` carries the packet claim but not the understatement |
| M-34 | **Four independent ProgramTest-evidence emitters** from four lanes in one hour, plus `check-witnesses.sh` duplicated | *"Somebody should own converging these before a fifth"* |
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
| M-50 | **The permissionless work economy** | *"Anyone may submit paid observation, repair, clear, finalize, or cleanup work."* — `PROJECT.md:194`; and *"Candidate submission is solver work, not a crank"* | Gen-3 has prepaid `bounty_lamports`. No solver, no keeper reward, and (M-12) no candidate submission route |
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
