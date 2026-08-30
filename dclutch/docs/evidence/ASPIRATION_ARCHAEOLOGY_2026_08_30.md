# Aspiration archaeology — DIG, 2026-08-30

The ask (ember, verbatim): *"dig through our project history and of original
~/dev/dragons-clutch and see what we've got missing or left to do and so forth
BESIDES the things you already had in mind... an opportunity to reevaluate,
regroup, take a look."*

Method: five parallel digs (dragons-clutch git history incl. all branches and
the market-theory thread; dclutch decision/doc archaeology; the Lean formal
side; a stranger's product review of https://clutch.dregg.pro; a bounded `cv`
transcript sweep for aspirations in ember's own voice), synthesized against the
KNOWN set (GOAL.md done-log, WAVE.md queue + cycle-3 charter,
LIVENESS_CENSUS Q1–Q9). Everything in those files is excluded; this doc is only
what is NOT there.

## The reframe: the ledger already exists, and it is orphaned

The single most important prior fact: **`docs/ASPIRATION_LEDGER.md` (ARCH-EOL,
2026-08-27, 3,002 lines) already did the deep half of this dig** — 537
intentions extracted, 187 verdicted MISSING, five tiers, ten recommendations.
And **`WAVE.md` and `GOAL.md` reference it zero times** (fourteen other files
cite it; the plan is not one of them). The ledger is suffering the exact fate
it documents: named, unrouted.

So this doc has two jobs: (A) a three-day drift check — which ledger
recommendations moved since 08-27 and which still float; (B) genuinely NEW
finds the ledger does not contain. Ranked at the end by wow-per-effort for the
launch ember is actually driving (a public demo that must feel ALIVE and be
genuinely usable).

---

## A. Ledger drift check (2026-08-27 → 08-30)

The ledger closed with ten recommendations "ordered by what a miss would
cost." Verdicts today, each verified at the tree:

| # | Recommendation | Status 08-30 |
|---|---|---|
| 1 | Check the two CFTC dockets (M-5) | **HALF-OPEN.** 1717 FILED 08-27 (GOAL handoff). 1388 was "ready to file (date bump + ember's one bracketed line)" on 08-27 night and **nothing since mentions it** — no filing confirmation anywhere. It was due 08-26. The only irreversible-deadline row in the ledger, still dangling. |
| 2 | Write docs/INTENT.md — the founding intentions (M-1..M-4, M-6) | **NOT DONE.** `docs/INTENT.md` does not exist. The ledger called this "the highest-value hour in the list" because these are recoverable only from `cv`, not from artifacts. |
| 3 | Surface O-013/B-spline substitution to ember (M-4) | **MOVED.** `docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md` exists; LiabilityBasisV2Spline landed (LB lane). Residual: whether the spline is *wired* — see B.5 (Lean section). |
| 4 | Eighth pattern: THE EXPANSION FRONTIER lane (M-9..M-12) | **NOT DONE.** `docs/research/EXPANSION_FRONTIER_2026_08_25.md` still referenced by nothing in the plan; "frontier" in WAVE.md = 3 hits, all unrelated (DLR-HOT stage frontier). ~~Four proved kernels still consumed by nothing.~~ **CORRECTED 2026-08-30 (FRONTIER-2): one, not four.** Re-measured by dependency edge: `dclutch-liability-basis-v2-kernel` has exactly one referring manifest (the workspace member list) and is the true orphan; `dclutch-structured-v2-kernel` has four (incl. `programs/dclutch-claims-sbf`), `dclutch-dealer-scenario-kernel` two, and `dclutch-representation-composition-v3-kernel` **eighteen**. This row inherited the headline from `ASPIRATION_LEDGER.md` M-9, which had already refuted it in its own body on 08-27 without amending its own headline — see M-9's 2026-08-30 amendment. |
| 5 | Widen the git-message action scan to dragons-clutch --all (5,106 commits) | **NOT DONE** (this dig is a partial pass; see section B). |
| 6 | Ask the five Tier-3 ember questions (M-22..M-26) | **ONE EXECUTED, FOUR FLOATING.** M-24 (store canonical bumps) was ruled+executed by W2q. M-22 (first market unredeemable), M-23 (reentrancy decision), M-25 (checked release: artifact or account?), M-26 (the fee rate — the oldest open question, day one) remain un-asked. Note M-22 gained a sibling on 08-30: the flagship 7Mcu is permanently untradeable (TRADE wall #22) — two dead-but-open markets now, no disposition ruling for either as *product objects* (what does the site say about them forever?). |
| 7 | Move Tier 4 off the /tmp board into a tracked file | **MOSTLY MOOT** — M-27/28/29/31/34 reached WAVE's DECOMP charter; M-33 reached the small batch. M-30 (dealer-family feature build broken) found in no queue. |
| 8 | Recover dangling commit d5dda5d (364 lines, source-contract) + fix GIT-SCAN item 10 | **NOT DONE.** `git cat-file -t d5dda5d` → still a dangling commit, one `git gc` from gone. WAVE GIT-SCAN item 10 still carries the claim the ledger proved false ("stash@{0}... verified" — there is no stash). |
| 9 | ADR-0005's three omission rows + retire the monolith benchmark by decision | **UNVERIFIED-LIKELY-OPEN** (no trace in WAVE/GOAL). |
| 10 | Fix WAVE:13 "fail-closed" language | **NOT DONE** (text unchanged; the memory rule "fail-closed is not absolution" exists outside the repo only). |

**Drift verdict: 2.5 of 10 moved in three days.** The ledger's rows are not
being consumed; they are being independently rediscovered (this dig re-found
several before finding the ledger).

---

## B. NEW finds — not in the ledger, not in the plan

### B.1 The site is a protocol museum, not a venue (fresh-eyes product review)

A stranger visiting clutch.dregg.pro sees integrity and zero motion: no
question to bet on, no price, no clock, no people. Seventeen gaps were found;
the load-bearing ones, each absent from plan and ledger:

| gap | evidence | size |
|---|---|---|
| **Markets have no human-readable question.** A market is a base58 pubkey + phase word; no title/question/description field exists in `lib/marketCoreV2.ts` / `lib/marketDetail.ts`. The defining feature of the category. | fresh-eyes; zero repo hits | ~an afternoon (off-chain title registry JSON keyed by pubkey, shipped with the site) |
| **No odds/probability anywhere — stated on-page as a refusal** ("There is no volume, price, odds… here, because the chain does not store any of those", `MarketDiscoveryWorkspace.tsx`). But the chain DOES store issued-supply atoms per cell; implied probability is derivable and honest if labeled. | fresh-eyes | ~a day |
| **No wall-clock time.** Raw slots only ("Finalized floor 11020"); zero countdowns. Slot→time is arithmetic; a ticking clock is the cheapest aliveness signal there is. | fresh-eyes; zero hits for countdown/closes-in | hours |
| **No OG/share cards.** `app/layout.tsx` ships title+description only. A market link pasted into Discord/TG/X — the entire distribution channel for this demo — renders as a grey box. | fresh-eyes; zero og:image hits | ~a day, and see B.2 |
| **/pulse and /activity are not in the nav** (`Nav.tsx`: Live·Markets·Design·Portfolio·Explorer·Docs·Console) and /pulse is dark ("No simulator running"). The two aliveness surfaces are unreachable by clicking. | repo-grounded (built, unshipped) | minutes–hours (nav + SIM flip already queued) |
| **No search/filter/sort/categories** in discovery; fine at 2 markets, fatal at 200 — and the plan's load simulator will CREATE the 200. | fresh-eyes | hours (client-side) |
| **No time-series anywhere.** Every chart (`components/charts/`) is a snapshot; nothing has a time x-axis. A prediction market's signature image is a line that wiggles. One poller appending (slot, cell-supplies) to JSON = real sparklines. | fresh-eyes | ~days |
| **Live-update nothing**: every page is fetch-on-mount; Solana ws subscriptions are free (`accountSubscribe`). A number that ticks while the cursor idles beats three new pages. | fresh-eyes | ~a day |
| **/create is labeled "Design"**, never says whether an outsider may proceed, and no faucet link exists anywhere (zero hits faucet/airdrop). Psychologically closed. | fresh-eyes | hours |
| **Resolution source invisible in UI** — no market surface names its oracle/feed/rule in words ("settles from Pyth SOL/USD at slot N"), despite the Pyth work being the protocol's pride. | repo-grounded elsewhere, missing in UI | hours |
| No leaderboards/identity, no watchlist, no P&L history, no static-JSON API for integrators, no comments/social | fresh-eyes | days each; leaderboard nearly free once SIM runs |

The reviewer's verdict, worth keeping: the "empty rather than fake" honesty
discipline is excellent engineering ethics **and is currently the direct cause
of the aliveness problem** — the fix is shipping real motion so the honest
branches stop firing, never relaxing the honesty.

(Coordination: POLISH lane opened 09:41 on stats/listing presentation —
overlaps the first row's neighborhood, none of the rest.)

### B.2 2.4MB of unreferenced key art

`dclutch/apps/dclutch-web/public/art/dragons-clutch-key-art-v1.png`
(dragons-clutch working tree, untracked, created 08-29 04:33): a dragon's claw
cradling a glowing faceted polytope — genuinely good brand art, on-thesis
(payoff basis as gem), referenced by zero files, in zero commits. Wire it into
the landing + make it the OG share-card background. Also orphaned: gen-1's
whole `docs/site-plan/` (concept inventory, site content map) has no successor.
**Size: hours. Level: engineering.**

### B.3 SECURITY.md's promise — the trigger has FIRED (decayed since the ledger)

Ledger M-48 recorded gen-1 `SECURITY.md:50` — *"A private reporting address and
coordinated-disclosure process will be added before any public test
deployment"* — as latent. It is latent no longer: DEPLOY-1 put seven programs
on public devnet (08-27/28) and `73b87027` (08-29) enabled **signed browser
submission**. dclutch has no SECURITY.md at all. For a protocol whose brand is
honesty, a missing security contact on a live public deployment is the one gap
a hostile stranger screenshots. **Size: an hour for the honest version. Level:
engineering (contact address is ember's).**

### B.4 Aspirations in ember's own voice, in no file (cv sweep, deduped against the ledger)

1. **Fundraising is the point of the demo** — 08-28: *"I was hoping I'd have
   something deployed and usable by other guys so that maybe we'd get some
   weekend fundraising but nobody is gonna be able to look at, participate, or
   understand anything."* The plan tracks the demo as an engineering milestone
   with its motive stripped off. This quote is the *ranking function* for
   section B.1 — it is why aliveness gaps outrank protocol completeness.
   **Level: vision (belongs in INTENT.md).**
2. **A longform blog post/poster on the compost method** — 08-22: *"'plan to
   throw one away' except 'plan to compost at least three' :D"*, target
   `~/src/dregg-posters`, scraped from `cv`. Never started; would be the
   #buildinpublic artifact the six-step founding plan (ledger M-1) promised.
   **Size: a day. Level: vision-flavored, ember-facilitated.**
3. **AI-authorship header for the filings** — 08-20: ember explicitly wanted
   the filings to let the AI speak for itself (*"This document is written by AI
   with a human facilitator and represents the positions of the AI"*). Recorded
   nowhere; 1388 is still unfiled (A.1), so the stance could still ride it.
   **Level: vision.**
4. **A Polymarket-resolved venue** — 08-27, half-dismissed in the same breath
   (*"a different venue over polymarket-resolved stuff...hm"*). Recorded so it
   stops being re-invented; the relay design (CHAIN_STATE_SOURCES) is the
   natural home for a verdict. **Level: vision, low priority.**
5. **Factory-contract stamping out per-market programs** — 08-22, self-parked
   ("maybe for a very different type of protocol someday"). Recorded as a
   someday. **Level: vision, lowest.**

(The cv sweep's sixth find — "beat isomkts on the merits" — IS ledger M-3, the
twelve-item ceiling; not new, still unrouted. Coverage caveat: the sweep
reaches only sessions whose cwd contains "clutch", corpus starts 08-18;
dClutch thinking in degg-research/breadstuffs/dregg-posters sessions is
unswept.)

### B.5 PRODUCT_THEORY_REDIRECTION — 539 lines alive only on a branch

`git show origin/agent/product-theory-redesign:docs/reviews/PRODUCT_THEORY_REDIRECTION_2026-08-24.md`
(commit `9dba79f6`): seven concrete product upgrades — exact dual-bound
optimality certificate, prepaid lazy foundation graph, cross-expiry rolls,
products beyond one source/statistic/window, quiescent Dealer epochs +
transferable LP shares, shared Source work capitalization, fee geometry as
measured experiment profiles. Its sibling (the trading-UI brief) was rescued to
`docs/recovered/`; this one was not — the ledger cites its name (G-20) without
its content. **Recovery is one `git show > docs/recovered/...`. Size: minutes
to recover, then a reconcile pass. Level: engineering to recover, vision to
adopt.**

### B.6 There is no CI

The live tree (`~/dev/dclutch`) has **no `.github` directory at all**; the
only workflow in either repo is dragons-clutch's `pages.yml`, a site deploy.
Every gate this project is proud of (gauntlet, abi:verify, fixtures:verify,
sbom_check, emitter byte-identity, the new seam-audit) runs only when someone
runs it. The plan's SEAM-CI built the *audit*; there is no substrate that
executes it on push. Ledger G-9 named this; it is in no queue. **Size: a day
for the first honest workflow (fast gates only, hbox for heavy). Level:
engineering.**

### B.7 The market-theory thread, resolved

The branch this dig ran from (`agent/market-theory-support4`) is a vestigial
name: zero commits, zero files, zero docs mention market-theory; current work
simply lands on it. The only substantive trace is stash@{8}
(`market-theory-quantized-work-authority`, 614 insertions of gen-2
quantized-relation/price-measure work) — already inventoried as ledger M-58,
superseded by the gen-3 restart. **No action beyond M-58's existing row.**

### B.8 Gen-1's product/economics/strategy layer has no successor

`archive/gen1/docs/` holds PRODUCT_THESIS.md ("a state-space compiler"),
ECONOMICS.md, COMPETITIVE_POSITION.md, RESEARCH_AGENDA.md (R1–R4 with
falsifiers), SIMPLEX_AUCTION.md, COST_MODEL.md, BENCHMARK_PLAN.md,
DEPLOYMENT_REVENUE_BOUNDARY.md. dclutch/docs has decisions/design/evidence —
and **no product thesis, no economics, no competitive position, no research
agenda**. Two specific losses with teeth:
- **DEPLOYMENT_REVENUE_BOUNDARY.md §5's five release tracks (A–E)**: the
  project is executing something Track-C-shaped (author-affiliated devnet
  deployment) *without any record of choosing a track* — while ledger M-2
  carries ember's *"if i can't at least deploy it to mainnet myself i'm
  probably not interested"*, silently inverted by the devnet-only posture.
- **The counsel-ready regulatory packet** (`archive/gen1/docs/regulatory/`,
  never edited since 08-18) is referenced by nothing live; its own use
  discipline ("reconcile every factual statement against the then-current
  repository") is two generations stale.
**Size: the re-ask is an ember conversation; the docs are days. Level: vision.**

### B.9 Escape hatch for the live 1.4M wall, in no queue

Gen-1 scouted **Groth16 succinct clearing at ~255k CU in a 795-byte
transaction — 5.5× margin against the 1.4M ceiling** ("We are the consumer it
never had"). The fourteenth wall is that exact ceiling, live today (seeds 1 and
7 still exceed it; DECOMP owns the residual). The scouted hatch appears in no
queue (ledger M-55 named it; still unrouted). **Size: a spike to re-validate
the numbers. Level: engineering, authority-adjacent.**

### B.10 Design rationale at risk: the batch relation is an FHE target on purpose

The DrEX/dark-FHE ambition was properly DROPPED-BY-DECISION (ledger §verdict)
— but the ruling retired the *ambition* without preserving the *rationale*:
the batch relation was chosen **because** it is a good FHE/MPC/vFHE target
(ledger M-3 item 11), and nothing in either repo records that as the reason
for its shape. If the relation is ever "simplified" by someone who doesn't
know why it is shaped that way, the door closes permanently. One paragraph in
INTENT.md preserves it. (Same class: the original motivating use case —
energy providers settling plans without disclosure — is not crypto at all.)
**Size: one paragraph. Level: vision-preservation.**

### B.11 Smaller new rows

- **Manipulation-cost table withdrawn from the public site**: the thread
  transferred (`ManipulationFloorV1` live in the founding generator) but its
  public evidence surface was quarantined 08-22 (`329faab5` → `bfe4b4f0`) and
  never restored — a differentiator (manipulation-cost-aware markets) with no
  public face.
- **M-30 still unowned**: `--no-default-features --features dealer-family`
  does not compile; in no queue.
- **VALIDATION_BACKLOG.md (28KB, 08-29)** — a second live backlog neither
  GOAL nor WAVE references; same orphan class as the ledger. Someone should
  merge or cross-link it into WAVE's queue before it too is rediscovered.
- **Nothing re-queues a deferred measurement** (ledger N-20's moral): a
  measurement that loses a resource race is written down perfectly and never
  re-run. The board has no mechanism; worth one standing rule.
- **External venue routing (Manifest/AMM/RFQ/Jupiter)** — ledger M-52; noted
  here only because the plan's "exchange story" can be mistaken for it: the
  plan is markets *about* venues, M-52 is routing claims *to* venues. Distinct
  and unretired.

### B.12 The formal layer: proven, emitted — and unconsumed (Lean sweep)

Scope: `formal/dclutch-semantics` = 122 modules, **1,688 theorems**, 80
emitters, zero sorry/admit/axiom (native_decide confined to
examples/ABI-constants per TRUST.md, spot-checked). 69 Rust files carry Lean
provenance headers. **159 theorems live in 20 modules unreachable from any
emitter.** Ranked:

| find | evidence | gap |
|---|---|---|
| **LiabilityBasisV2 stack: 221 theorems, emitted, byte-checked, ZERO consumers.** Partition-exactness, no-arbitrage certificate, integer de Boor — all landed; `dclutch-liability-basis-v2-kernel` referenced only by the workspace member list. | O-013's own row: *"no consumer, and no layout by which a Market can select a spline basis at all"* | ONE Market layout field selecting a basis; everything downstream is already proved+emitted. The ember-caught B-spline requirement (ledger M-4) ends here or nowhere. |
| **The decision-0012 slot-pin admission rule is proven (43 thms, `ProtocolInfrastructure.lean`) and hand-mirrored in Rust** (`core-sbf/src/infrastructure.rs`, 901 lines, no emitter for the semantics — only the tiny Abi sibling). | `canonical_pin_admits_iff_observation_matches_release`, `moved_slot_refuses`, … | the highest-stakes hand-written mirror in the tree — this rule is what stops an upgraded program forging admission. A Lean-emitted refusal corpus is cheap vs what it protects. |
| **~54 of 69 generated files have NO re-emit byte-check, and no CI runs the 21 check scripts that exist.** Neither checked-release nor final-generated-convergence invokes `lake`. | rg over provenance headers vs check-*.sh census | the "do not edit" header is an honor system; one script walking every provenance header + re-emit + diff closes it. Pairs with B.6. |
| **GeneralV5Assurance: 24 theorems** making "best valid submitted candidate" provable (conservation, zero residual, tie-keeps-incumbent) — zero refs outside formal/. | O-017 forbids "optimal clearing" claims without a checked certificate | the discipline exists, the adapter never checks it. |
| **Series V3 refusal theorems (23) vs hand-Rust cursor machine** — the kernel has 9 dependents incl. trading-sbf; only the immutable Abi is emitted. | `prepared_current_refuses_duplicate` etc. | live on-chain state machine whose proven refusals are enforced only by hand tests. |
| **CapabilityFundingLedgerV2.lean is an import-graph island** — imported by literally nothing, while `FundingLedgerV2::activate_in_place` ships on-chain (resolution-proof core_effect.rs:292). | lakefile glob is the only reason it compiles | strongest proof/code asymmetry in the tree. |
| **Dealer scenario solvency (17 thms) unconnected** to the hand-written `dclutch-dealer-scenario-kernel` (O-010's generalization path). | `minimumSplit_is_least`, hostile refusals | connect or state why not. |
| **direct-aot-v3-contract is an orphan** — aot-sbf still depends on V2; the translation validator validates only V2. (Sharpens known U-014.) | its own header: "a relation that appears here and not there is a defect in this file" — unchecked | extend the validator to V3; that is the whole gap. |
| **Three Lean-authored crates with zero consumers**: product-payoff-codec (V1, plausibly superseded by v2-codec — but nothing says so), claims-representation-codec, economic-kernel. Emitters still wired; output rots. | source-grep zero external refs | retire-with-note or route; unlabeled limbo is the worst state. |
| **TS ABI is a second-hop transcription** — 2 of ~20 generated TS files are Lean-authored; the rest transcribe "canonical Rust". `TsEmit.lean` substrate exists. | wider form of the queued POSITION_PDA_DOMAIN item | each remaining file is one emitter away from single-author. |
| **Gen-1 theorems that never migrated**: `MomentCone.lean` (the constructive arbitrage that JUSTIFIES the price gate — the successor gate ships without its counterexample), `Solvency.lean` exactness/tightness (`is_exact_sup` + attaining weights — what stops the envelope being over-collateralized), `BSpline.lean` generic uniform evaluator (2,319 lines, the piece that makes a shape compiler cheap). | archive/gen1/lean/, all sorry-free | port or cite-by-decision; today they are compost nobody ruled on. |

**Size: mostly days-per-item; the CI+re-emit gate and the spline layout field
are the two with launch-visible payoff ("every number on this site is
theorem-checked" is a marketing sentence no competitor can say). Level:
engineering, with the spline field being an authority decision (wire break).**

### B.13 The decision-log dig: complete designs, one owner short (doc sweep)

The pattern across every top find here: the design is finished and correct;
the executable half stopped exactly one owner short. None is blocked on a
missing idea.

1. **Dealer still commits the defect class that stranded the first market's
   principal.** Decision 0008 §1: *"Every consumer DERIVES both the replay and
   the Vault from [`custody_context`]… no route may assume the Market
   address."* Verified live today: `dealer/v3_composer.rs:561`,
   `v3_accelerator_accounts.rs:849`, `v3_multi_lp.rs:1009-1011`, and
   `dealer-sbf/src/lib.rs:2049` all derive `HoardPrincipal` from the MARKET
   address; none reads `aggregate.custody_context`. The owning lane
   (tranche-A Dealer) was recorded CLOSED — for the CoreState decode fix, not
   this. Plus: Dealer v1 partitions by Dealer state address where v2/v3/v4
   partition by `child_root` — two conventions in one family. **Days;
   engineering; correctness-critical.**
2. **The recovery ladder is welded shut for every family — every devnet
   market has exactly one source attempt and no fallback.**
   `process_funded_transition` reachable only from `#[cfg(any())]`;
   `RecoveryMaterialSlotV1::new` is Pyth-only; Core refuses `CreateFund` on
   any recovery material (the correct Q2 weld). MAINNET_STATE_RELAY §13 names
   the post-v1 lane (funded FailNext over RecoveryPolicyV2, relayed leg under
   a disjoint key set); it has no owner and appears in no charter. The open
   question on top: does v1 ship one-attempt markets *forever*? **Weeks;
   engineering under a ruling.** *(edges maybe-known: GIT-SCAN item 2, census R2)*
3. **The fee program's trigger fired and the lane did not spawn.** FEE-GEO's
   row says "Trigger: cycle 3"; cycle 3 opened 08-29 with six items and
   FEE-GEO is not one. Verified: `dispersion_bps`/`FeeBaseV1`/
   `accrual_monotone` — zero hits; General's config has NO fee field (General
   charges nothing). Underneath: M-26 (the day-one rate question) and N-15
   (formalize-before-freeze precondition, recorded in no gen-3 doc). **Kernel
   + composite: weeks. The rate: ember's alone.**
4. **General is an order-book venue with no way to place an order** —
   `effect_artifacts_v3.rs:239` still lists all seven collection/candidate
   actions as `unauthored_actions!`. GEN-SEVEN sits in the 08-27 handoff
   queue with rungs laid; the cycle-3 charter does not carry it. Inside it,
   three decision-0010 residues: work-escrow lamports never move (§6.3),
   `ExpireSettlement` has no counterpart — a stalled settlement cursor is
   stuck (§6.4), nothing creates/closes the claim-escrow Position (§6.5).
   **Weeks, one coordinated unit; engineering.** *(GEN-SEVEN itself
   maybe-known; the charter-drop and the three residues are the finding)*
5. **Decision 0005's promised omission rows were never recorded** (the M-19
   shape, verified twice more): the capability-seal Lean ABI migration
   ("recorded in the omission index" — it is not; no seal emitter exists) and
   seal rent reclamation (no `CloseSeal` route anywhere; permanent
   unreclaimable rent on a growing write-once account class). OMISSION_INDEX
   has neither row — all 38 read. **Index rows: minutes. Migration: days.**
6. **The artifact bridge is frozen at day one.** All four ELF-level theorems
   date 2026-08-25 and cover artifacts retired the same day;
   `formal/qedsvm-direct-v12/` contains ZERO .lean files (traces + harness +
   evidence.json only); the eight committed Kani harnesses in
   `tools/direct-translation-validator/src/kani_proofs.rs` have never run
   once. The universal-theorems park (D-4) covers none of this half.
   **Running Kani: hours. The bridge: ember's "is this still the
   architecture?" first.**
7. **Retirement — the market's last life step — has never run anywhere, on
   any substrate.** README's own voice: "winding a market all the way down to
   retired has not run anywhere yet"; the journey gauntlet records the gap
   MOVING rather than closing (retire refuses while the Hoard holds one atom;
   emptying means redeeming; redemption is behind the Hot gate); census Q3 +
   Q6 now gate it too. The acceptance condition that distinguishes a market
   from a one-way trap. **Ruling (Q3) then days-to-weeks.**
8. **Dealer's capital design (consent-bound tranches, quiescent epochs,
   scenario solvency) is fully written and has no path to execution** —
   `grep epoch` in dealer-codec + scenario kernel: nothing; the design doc's
   tranche fence ("must not simulate tranches with Dealer counters") holds
   only because nothing simulates anything. O-011's closure condition
   untouched; Q4 rates Dealer least-live. **Design pass + weeks; a
   capital-structure ruling inside.**
9. **The plan carries a transport candidate the design already ruled out.**
   WAVE's demo-shape section still names Wormhole Queries as "candidate
   permissionless upgrade… MR lane owns pinning"; MAINNET_STATE_RELAY §3.1
   concluded "not a candidate for v1 and not a near-term upgrade path." One
   of the two is stale; a lane that picks it up re-does a closed
   investigation. **One paragraph.**
10. **Relay-slice named lifts, each with a stated trigger and no owner**:
    large-account chunking (inline window > 448 B needs persisted SHA-256
    midstate), m-of-n with m>1 (threshold expressible, no multi-signer
    campaign), the Realm-level shared observation cache (N markets pay N
    rents), and §10.1's time bomb — after DBC 0.2.0 a decoder pinned to
    `VirtualPool` silently stops seeing transfer-hook pools (fix named:
    account 5 = PoolConfig, unbuilt). The graduation market's nine-record
    set: "one getAccountInfo away and none has been read" — the demo thesis
    is complete in a harness and has never touched mainnet bytes.
11. **Cross-host reproducibility is unestablished** —
    `checked-release-candidate.sh:6` says LOCAL in its own capitals; nothing
    shows a second host reproduces the digests (PROJECT_METHOD rung 6; U-011).
    Pairs with B.3/B.6: the two assurance rungs that matter *because* the
    deployment is already public.
12. **Gen-1's ratified commit/reveal subdivision did not cross the
    generation boundary** — ADR 0006's answer to candidate withholding /
    proposer bonds; zero occurrences in gen-3; General ships Consider→Freeze
    without the subdivision and no record says why. A solver who withholds
    the best candidate faces no bond and no detection. **A design pass;
    vision-level mechanism work.** *(sharpens ledger G-7)*
13. **Three cheap honesty repairs**: (a) README advertises a public relay
    publication log at `portal.dregg.studio/relay/publication_log.jsonl`
    that does not exist — while DEPLOY_1 §6.3 has the relayer disarmed and
    §4.11 says a relayer profile "should not be released" without
    publication; the site makes a liveness-checkability claim the deployment
    cannot support. (b) `ADOPTED_2026-08-20` is cited eight times from
    dclutch at two different paths, both wrong (it lives only in
    dragons-clutch/archive/gen1) — the fee-shape decision record is
    unreachable from the repo that cites it. (c) COMPOST.md promises the
    repo graft "will have its own reviewed plan" — none exists. **Hours
    each.**

And the doc sweep's framing note, which this whole dig confirms: *"the
mechanism is not the medium; it is whether a row is in the thing A GATE
READS. blocked.json and SETTLEMENT_BLOCKERS are read by tests.
OPEN_QUESTIONS.md is read by people."* The census Q-rows, the fee lane, the
spline question, and the Dealer capital design live in documents only people
read.

---

## C. The ranked gap map (wow-per-effort for a launch that must feel alive)

**The meta-move first.** This dig's most repeated finding is the same one the
ledger made about itself: rows survive only in the thing a gate (or the plan)
reads. The ledger, VALIDATION_BACKLOG, the census Q-queue, the fee lane, and
now this doc all live outside WAVE.md. The single highest-leverage act is ONE
WAVE entry that dispositions this doc's tiers — schedule / park-with-trigger /
retire-with-reason — so this file does not become the next orphan the next
archaeology lane rediscovers.

**Tier 1 — hours each; each either visibly changes what a stranger sees or is
irreversible if missed:**
1. Market titles/questions registry (B.1) — the single highest
   wow-per-effort item this dig found.
2. Countdown clocks + implied-probability display (B.1).
3. /pulse + /activity into the nav; default activity to the flagship; SIM
   flip (B.1 — the flip is queued; the nav rows are not).
4. Key art onto the landing + OG share cards (B.2 + B.1) — distribution IS
   the demo for a memecoin-adjacent launch.
5. SECURITY.md with a real contact (B.3, B.13.11) — the "before any public
   test deployment" trigger already fired.
6. CFTC 1388: check the docket, file or explicitly withdraw (A.1) — the one
   irreversible deadline; the AI-authorship header (B.4.3) can ride it.
7. Recover PRODUCT_THEORY_REDIRECTION + d5dda5d before git gc (B.5, A.8).
8. Honesty repairs: README's nonexistent relay publication log, the
   ADOPTED_2026-08-20 broken citations, the Wormhole plan/doc contradiction
   (B.13.9, B.13.13).
9. Run the eight committed Kani harnesses once (B.13.6).

**Tier 2 — days; structural product and correctness wins:**
10. docs/INTENT.md (A.2 + B.10 + B.4.1) — one hour of writing that makes
    every future lane rank like ember ranks; still the ledger's best call,
    now three days more overdue.
11. Time-series poller + sparklines; ws live updates (B.1) — the site's
    signature image is currently missing from the site.
12. First CI workflow + the Lean re-emit byte-gate (B.6 + B.12) — 54 of 69
    generated files unguarded; every gate today runs by hand.
13. Dealer custody_context derivation fix (B.13.1) — the defect class that
    stranded the first market's principal, still live in four routes.
14. Search/filter, "Create" labeling + faucet link, static-JSON API (B.1).
15. Decision-0005 omission rows + seal emitter migration (B.13.5).

**Tier 3 — ember decisions; cheap to ask, compounding to leave un-asked:**
16. The floating Tier-3 questions M-22 (+ its new 7Mcu-untradeable sibling),
    M-23, M-25, M-26 (A.6) — and spawn FEE-GEO, whose trigger fired (B.13.3).
17. Upgrade posture / release track A–E / the mainnet precondition, re-asked
    as one conversation (B.8, B.13 items 6+9's "is this still the
    architecture?", census Q1 interaction).
18. The spline basis layout field — is the certified-basis substitution the
    B-spline requirement, and does a Market get to select it? (B.12 top row,
    M-4/O-013.) 221 theorems wait on one field.
19. Twelve-item ambition ceiling: adopt / trim / retire per item (ledger
    M-3) — the de facto product bar, still in no file.
20. Expansion-frontier + proven-unconsumed-kernel disposition lane (A.4,
    B.12) — schedule, park-with-trigger, or retire, each with a reason.
21. One-attempt markets: does v1 ship without any recovery ladder forever?
    (B.13.2.)
22. Retirement: rule Q3, then run the full wind-down ONCE anywhere
    (B.13.7) — the last unexercised acceptance condition.
23. General order collection (GEN-SEVEN) into a charter, or explicitly
    post-launch (B.13.4).

**Tier 4 — recorded so they stop being re-invented:** compost-method blog
post (B.4.2), Polymarket-resolved venue (B.4.4), factory contracts (B.4.5),
external venue routing (M-52), EVM (M-60), Groth16 hatch spike (B.9 —
promotes to Tier 2 if the 1.4M residual resists DECOMP), commit/reveal
mechanism pass (B.13.12), Dealer capital design (B.13.8 — large; wants its
own charter), relay-slice lifts incl. the DBC-0.2.0 decoder time bomb
(B.13.10 — the decoder row promotes the moment a graduation market is real).

---

*Provenance: five subagent digs 2026-08-30 morning (dragons-clutch git
archaeology; dclutch doc archaeology; Lean formal sweep; clutch.dregg.pro
stranger review; cv transcript probe), synthesized by DIG. Verification
commands inline. Read-only dig; nothing but this file changed.*
