# What slipped through — a documentation sweep after the 2026-08-30 wave

Roughly forty-five lanes landed on 2026-08-30 and the `.md` corpus grew faster
than it was reconciled. This is a read-mostly sweep of that corpus for three
things: claims today's landings falsified, commitments nobody executed, and
decision records that do not carry the rulings made on them.

It is grouped by **disposition**, not by topic: what was amended in place, what
is open and needs an owner, and what is stale and was deliberately left alone.
Every amendment cites its superseding evidence. Every size is a number
somebody measured or estimated, attributed to whoever said it — never a
feeling. Where an item could not be settled from the tree, it says so rather
than guessing.

Two commits carry the amendments: `a4ceb704` (decision records) and
`79522879` (falsified claims).

---

## 1. Amended in place — 18 edits across 17 files

### 1.1 The decision records now carry their own rulings

Ember ruled all seven packet questions on 2026-08-30. Every ruling lived only
in `WAVE.md`; not one had been carried into the record it rules on.

| record | was | now says | superseded by |
|---|---|---|---|
| `decisions/0014-the-fee-rate.md` | "OPEN — ember's ruling required" | **RULED, all three, none built.** D1 deferred-as-built; D2 adopted at `MAX_FEE_BPS = 500`; D3 adopted-in-principle but **blocked**, not merely sequenced | `WAVE.md` E1; `DECISION_PACKET` §1; `DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md` |
| `decisions/0015-markets-that-can-never-resolve.md` | "RULED B, THEN FOUND UNEXECUTABLE" | adds the **final disposition**: write-off accepted, markets stand unretireable, **no pre-cut retirement contingency**, option C ruled and owed | `458d47bb` |
| `decisions/0016-checked-release-identity.md` | "OPEN — recording requested" | **ADOPTED — option A + the 0012 residual** (`dclutch-release-tool` stays strict). M-25 closes | `DECISION_PACKET` §2, `27f7944b` |
| `decisions/0017-cache-read-role-authentication.md` | "OPEN — ratification requested" | **RATIFIED — A ratified, B chartered at a measured 52,592 CU, C refused**, with the per-family tripwire named as a *condition*. M-23 closes | `DECISION_PACKET` §3; `TRUST_RATCHET_V1.md`, `028f6047` |
| `decisions/DECISION_PACKET_2026_08_30.md` | read as seven open questions | **CLOSED status table**, ten rows, plus the six things still unbuilt after every ruling | `f28036bf`, `458d47bb` |

Three consequences worth reading off that table:

- **D2 is ruled and enforced nowhere in the protocol.** The only live check on
  `MAX_FEE_BPS = 500` is a shell guard at
  `tools/release/stage-devnet-sponsored-market-open.sh:84-85`. A founding that
  does not go through that one script is unbounded. §6 of 0014 priced the real
  fix at **one lane, protocol-tier**: one const, one refusal discriminant, the
  Lean bound and its two `native_decide` boundary theorems, a census
  regeneration.
- **E3 is the one packet question still genuinely open** — seal-rent
  beneficiary, leaning collector-keeps, final call deferred. Recorded on
  `OMISSION_INDEX` P-006, which is where a `CloseSeal` implementer would look.
- **E5 was accepted conditionally**, and the condition is a charter
  requirement: the debtor must always be able to settle unilaterally,
  *including when the fee recipient's token account has vanished*.

### 1.2 Claims today's landings falsified

| file | the claim | what superseded it |
|---|---|---|
| `tools/gauntlet/hot-cu/README.md` | "the Hot tail's compute" — no route named | **Header added**: this tier drives the *demoted* Registry Hot continuation; every figure it has ever printed is **+35,127 CU** high (the same integer on all 13 comparable seeds). Use `direct_hot_top_level_margin_gate.rs` for a public-trade figure. `CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md` (`8bf6ad40`); `DECISION_PACKET` §4 |
| `tools/gauntlet/README.md`, `tools/gauntlet/TIERS.md` | the tier "answers *does the Hot tail fit under 1,400,000 CU*" | same correction, with the founding-continuation carve-out stated so nobody sweeps it |
| `docs/evidence/TRADE_DIRECT_ACTIVATION_WALL_2026_08_29.md` §"selected keys" | maker keys would be **SELECTED** for their CU draw and labelled as chosen | **Marked REVERSED, do-not-execute.** Ember's standing test — *does it make the DEMO work, or the PRODUCT work?* — and the band it manages no longer exists: ALLKEYS made every key cost 1,336,742 CU with a tail of exactly 0. `SESSION_STATE.md` "THE ORCHESTRATOR'S OWN ERROR, RECORDED"; `308c3dff`..`e7805d62` |
| same file, "eighth entry / no capability ever activated" | quotes an `OMISSION_INDEX` line that no longer stands, and says no capability of any family has been activated anywhere | WALL22 landed the eighth `CapabilityProgramSetV2` entry and the exactly-seven relaxation (`2f21911e`/`c2cfa4db`/`9012499c`); **the first capability root in the protocol's history is live on devnet** under market18 |
| same file, the tail figure | `P ≈ 0.032%`, one trade in 3,100 | fixture artifact (real pre-lane Markets were 1-in-34.9M) → FIXBUMPS 1-in-1.10-billion (`30574297`) → **ALLKEYS: exactly 0** |
| `docs/evidence/DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md` | §4's variance table, seven surviving sites | **zero** surviving key-varying searches; the *method* survives, the table does not |
| `docs/evidence/DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md` headline | "at least **1,521,004** CU … over by at least **115,003**" | **internally inconsistent** — 1,521,004 is seed 1's lower bound (over by 121,004); 115,003 belongs to the 1,515,003 all-first-try floor. Now states the floor, which is what every downstream document quotes |
| `docs/design/FEE_SECOND_TRANSACTION_V1.md` opening | 106,527 over a 1,506,527 floor, fee leg 174,119 | the landed figures are **115,003 / 1,515,003 / 182,386** (`24b2b7f2`+`3d5dda0e`, meter-truncation artifact fixed, reproduced to the CU across two ELF sets). Its "46,592 CU of worst-seed headroom" is flagged as pre-ALLKEYS; tx1's headroom is now a flat **63,258** on every key |
| `docs/guides/devnet-pyth-market-open.md` | prose says *pass 0 for any market that must trade*; the worked example passes **50** | example corrected to `0`. As written it would have founded an untradeable market from a guide that forbids it three paragraphs earlier |
| `docs/evidence/CAPABILITY_ACTIVATION_TEMPLATE_2026_08_30.md` | "General's publication closure is not wired" | GENPUB wired it the same afternoon: three labels, eight entries (seven is *unfoundable*, not smaller), 68 publication records read back Registry-owned (`b09c4ee9`..`50f68bb5`) |
| `WAVE.md` demo-shape section | Wormhole Queries as a "candidate permissionless upgrade to verify" | `MAINNET_STATE_RELAY.md` §3 answered it **not available** — on devnet the guardian set is a single test key. Flagged by `ORPHAN_DESIGNS_TRIAGE` §3.9 at "minutes", left for whoever next edited the file |
| `AGENTS.md` refusal-code doctrine | forbade `Custom(3)`-style substring assertions; **said nothing about `is_err()`** | a bare `is_err()` is now named as not-a-refusal-assertion, with the two commits that paid for it today: `67e96e5b` (four hostile assertions "passing" for four days on a length refusal) and `d1d1ff3f` (fifteen more naming no code, three of which refused somewhere other than their author believed) |
| `docs/OMISSION_INDEX.md` P-006 | seal-rent beneficiary "requires a ruling" | records that the ruling **was put to ember and is still open**, leaning collector-keeps, and that no `CloseSeal` may land ahead of it |

---

## 2. Open — needs an owner

Cross-checked against `SESSION_STATE.md`'s lane queue and today's commits.
Sizes are the source document's own.

### 2.1 Owed by a ruling that has now been made

| item | source | current truth | size |
|---|---|---|---|
| ~~**0015 option C — the honest discovery bucket**~~ | `0015` §5 C, §7, §8.8 | **CLOSED while this sweep was being written** (BUCKET, `e3600765`): a fifth listing group, a card state, the pulse counting the class by name, the detail page, and the portfolio admitting the count is arithmetic only — all off one predicate, `marketActivationOutlookV1`, with every card still printing `Open`. **What it did not reach** is §3's row: the generated reference and the public docs landing still say *"there is no open market"* | was *one web lane*; landed as 14 files |
| **0014 D1's paragraph** — say out loud who receives fees | `0014` §6 | **Not written.** `README.md` and `docs/guides/trader.md` contain no statement of fee destination; trader.md's only "fee" prose is about the transaction fee payer | *one paragraph*, two files |
| **0014 D2's protocol band** | `0014` §6 | **Ruled, unbuilt.** See §1.1 | *one lane, protocol-tier* |
| **0017's continuation tripwire** | `0017` §7; `DECISION_PACKET` §3 | Ratification shipped *with* this condition and the test does not exist: a per-family test exercising a child under a real continuation. The pattern is already in `crates/dclutch-registry-activation-auth-v1/src/tests.rs:246-264` | no size stated; the enforcement is **subtractive** — nothing refuses a re-added Registry CPI except the runtime, at the cost of the whole transaction |
| **0017 option B** | `0017` §6; `TRUST_RATCHET_V1.md` | Chartered today. Three top-level CPI sites (`outer.rs::reauthenticate_role`, `direct_begin_retiring_v1.rs:685`, `hot_v3.rs`'s non-continuation arm) | *one lane, mechanical*; **52,592 CU**, invariant across 32 keys and two builds |
| **E5's self-cure proof** | `WAVE.md` E5; `FEE_SECOND_TRANSACTION_V1.md` | A charter requirement, not a suggestion: the debtor must always settle unilaterally, including when the recipient's token account has vanished (create-idempotent or equivalent) | rides FEE2TX's *5 lanes / 3–4 days* |
| **The two omission rows 0015 asked for** | `0015` §7, §8.8 | **Not added.** `OMISSION_INDEX` has no row for either. The doc's own maintenance rule requires it. Not taken here because assigning an ID, a classification and a closure condition is a normative act, not a mechanical one. The sentences are written and ready to paste: *(a) a market whose resolution authority is unreachable would strand every lamport it holds, and nothing in the protocol prevents founding one*; *(b) a market whose founder identity is unheld strands its collateral and can never retire, and the protocol cannot tell the difference from the outside* — and (b) is **not decidable from chain state**, which is why it has to be enforced at founding | two rows |

### 2.2 Named today, carried nowhere

Each of these was named by a lane in its yield and appears in no queue outside
`SESSION_STATE.md`, which is rewritten at each compact.

| item | named by | size |
|---|---|---|
| Cross-cohort readers not fixed for the 360→368 `CoreState` widening — relayer `keeper.rs`, journey gauntlet | CORESTATE, `e93fe5e9` | no size stated; **cohort isolation is FALSE for reading** |
| Five more hand-pinned digests *of emitted constants* in `ordinary_bundle_v4.rs` / `ordinary_artifacts_v3.rs` — structurally invisible to emitted-file-vs-emitter gates | CORESTATE-4 | 5 sites |
| Near-wall SBF frames: `outer::process_close` at 3,968 and four functions at 3,904 against the 4,096 wall | CORESTATE-2 | 5 functions; `tools/sbf-frame-sizes.py` is the detector-with-distance |
| The suites tier does not refuse on SBF frame diagnostics | UNRUN | inherited, named |
| `capability_close_alias` and `retirement_replay_handoff` are run by **nothing** | CI-3 | 2 targets |
| Four `tools/release/test-*.sh` run by nothing | TRADE-3 | 4 scripts |
| The four TS ABI generators are hand-maintained byte-identical duplicates across `apps/` and `packages/`, **guarded by nothing while their outputs are guarded** | BASIS-ENUM | 16/17 basenames identical, 1 legitimately divergent — "a naive gate arrives red" |
| `routeCensus.ts` — 4 unrendered instruction magics (registry-lineage + claim-check) sitting as exemption entries | BASIS-ENUM | 4 magics |
| The child-caller hint slots need the projection port before a browser can fill 8/8 | HINTS-TS | **10,643 measured non-test Rust lines across 4 crates**; per-request draws, so no key is permanently stuck |
| `budget.max_lamports_spent` — the simulator has no spend-based kill | SIMVIZ | 1 config field |
| Open-family fixture lifecycle policy parks its only plan at `action: u32::MAX` — a dead plan that reads as a design | MEMBRANE / STRUCT-SEL | 5 open actions, tags 1–5 |
| `LiabilityBasisMarketSeedsV2` constructor is the migration target for raw-spelled sites tree-wide | FRACLIFE | ~24 sites |
| S-3 staleness tripwire before any record-reclamation route lands | SEALWIDE | no size stated |
| 2 explorer deep-links dropped from `RedeemFlow.tsx` in the cut | PUBLISH | 2 links |
| 9 live commits after `bb6d4edb` awaiting the next cut | PUBLISH | 9 commits |
| genref reports 9 stale reference files — 2 fixed, **7 unowned** | CORESTATE-4 | 7 files; candidate for the cut's authorized quiet-tree regeneration |

### 2.3 Structural: the reference index does not know today happened

`docs/reference/decisions.md` is generated and **stops at decision 0013**. All
four of today's decision records — the fee rate, the dead markets, checked
release identity, cache-read authentication — are absent from the index that
ships to the public site. `docs/reference/README.md`'s totals (13 programs,
141 routes, 212 refusal codes) are likewise pre-`BASIS-ENUM`.

Not regenerated here: whole-tree generators are barred at this lane count
(`tools/genref/generate.sh` swept eighteen lanes' refusal codes into one
reference once already), and the one authorized quiet-tree regeneration is
TRADE-2's, at the cut. **This should be on the cut checklist** alongside the
refusals regeneration, not discovered afterwards.

### 2.4 The gate list nothing points at

`docs/VALIDATION_BACKLOG.md` is 453 lines of release prerequisites — nine
numbered convergence gates, a seven-row deployment disposition, twenty named
seeds — and **`WAVE.md`, `GOAL.md`, `README.md`, `AGENTS.md` and
`docs/INTENT.md` reference it zero times.** `ASPIRATION_ARCHAEOLOGY` flagged
this on 2026-08-30 (*"someone should merge or cross-link it"*) and nothing
did. It also still points the release CU gate at the M-61 sweep — that is,
at the tier §1.2 just demoted.

---

## 3. Stale, listed and deliberately not fixed

These need more than a sentence, or belong to an owner who must make a call.

| file | line | the stale claim | why not amended |
|---|---|---|---|
| `docs/reference/budgets.md` | 88 | "Remaining ceiling headroom from the pin is 121,253 CU, 8.7%, AND SHRINKING" — a one-draw headroom, and the generated file drops the source's own sample caveat | **generated**; the fix is in `tools/genref` |
| `docs/reference/routes.md` | 145 | `registry/hot_continuation_v2::process` reads as a live route; `blocked.json`'s reason for it is the "20/20 under 1,400,000" claim the evidence calls false by a factor of nineteen | **generated** from `blocked.json`; fixing the source row is a route-census change |
| `tools/genref/generate.mjs` | 404-406 | "there is no open market or value at risk today" | three devnet markets are `Phase::Open` (`0015` §8.1). True in spirit — nothing is tradeable — false in letter. **This is 0015 option C's defect one layer up.** BUCKET (`e3600765`) fixed the app and did not touch these; four more instances live at `render-site.mjs:453,690` and `docs/reference/README.md:35`, one of them a link-check assertion string that will fail if the prose changes without it |
| `tools/genref/render-site.mjs` | 470 | "There is no open market, no value at risk, and nothing to buy today" — the **live public site** | same; the site simultaneously lists the open markets |
| `README.md` (dclutch) | 14-15 | "no current open Market" | same class |
| `README.md` (dclutch) | 35-36 | "winding a market all the way down to retired has not run anywhere yet" | contradicted by FRACLIFE (`FRACTIONAL_RETIREMENT_LIFECYCLE_2026_08_30.md`, four real transactions, Begin → walk → Finish). The honest rewrite has to distinguish *harness* from *validator* and *fractional* from *the flagship*, which is a judgment about launch-surface copy, not a mechanical fix |
| `README.md` (dclutch) | 149 | advertises a relay publication log at `portal.dregg.studio/relay/publication_log.jsonl` | `ORPHAN_DESIGNS_TRIAGE` §3.13a sized this at **hours** and called the claim unsupportable. RELAY-3 has since executed the public-submission proof end to end, so the premise may have changed — **needs one check of whether the log is actually served**, then delete the claim or keep it |
| `ARCHITECTURE.md` | 19-21 | "within-cell graded ramps and tents do not [compile exactly]" | degree-0 and degree-1 shaped payoffs ship today under a certified categorical projection (`OMISSION_INDEX` U-013). Already covered by the file's supersession banner; named here for the queued REPRESENTATION MAP lane |
| `SESSION_STATE.md` | 10-11, 21-22 | "The public Direct Hot route does not fit under the 1,400,000 CU ceiling for arbitrary keys" / "worst seed 1,393,616 … 6,384 CU of margin" | the single most-read stale sentence in the corpus, and **false after ALLKEYS**. Left alone: `SESSION_STATE.md` is the orchestrator's own pre-compact handoff and is rewritten wholesale each compact; a sweep lane editing it races the next write |
| `SESSION_STATE.md` | 136-167 | "PENDING EMBER DECISIONS" lists all four ADRs as pending | same reason; §1.1 above is the durable record |
| `docs/design/BASIS_ABI_UNIFICATION_V1.md` | 483-484, 701-703, 739 | budgets a hull check against "8,006 CU of headroom" and "1,225 B of 1,232" — both **continuation** figures | the design's conclusions may survive the correction (top-level is cheaper and the wire is 1,167 B), but re-deciding them is the basis lane's call, not a sweep's |
| `docs/design/TRUST_RATCHET_V1.md` | 432-434 | uses the continuation's 1,225 B packet as the frame budget for a seal on the public route | same |
| `docs/evidence/PROTOCOL_CU_TOPOLOGY_2026_08_28.md` | 619-621 | "≥30,000 CU of 20-seed mean headroom" against a `hot-cu` mean | acceptance basis is a continuation mean; re-basing it is a CU-owner decision |
| `docs/decisions/0005-per-market-authentication-cache.md` | 335, 432-433 | "about 12,000 CU of headroom"; the canonical continuation packet at 1,225 B | a ratified decision record; superseding numbers belong in an amendment its owner writes |
| `docs/VALIDATION_BACKLOG.md` | 307-309, 325-329 | the release M-61 sweep as the compute gate | see §2.4 — the whole file needs an owner before its rows are individually corrected |
| `tools/devnet-scenarios/`, `tools/activity-properties/`, `tools/economic-lifecycle-ledger/` READMEs | various | fixtures and property suites built on a 50-bps fee | these are *fixture* descriptions, not founding advice, and the fee-bearing shape is legitimately exercised in a harness. Flagged so nobody reads them as a founding precedent |
| `docs/design/FEE_GEOMETRY.md` | 469-470, 481-488 | demo markets at 25 / 0 / 100 bps "at zero new code" | correct arithmetic, impossible today: any nonzero rate founds a market that cannot trade until the second-transaction fee leg ships. The doc's own §4 sequencing is the fix |

---

## 4. `~/dev/dragons-clutch` — host-repo drift (listed only, not edited)

The successor subtree was cut to `bb6d4edb` today. The wrapper's own prose was
not, and it describes a project three days behind.

| file | line | claim | reality |
|---|---|---|---|
| `README.md` | 14-15 | **"Nothing in this repository is live: no deployment you can use, no live market, nothing value-bearing. Everything runs on local test chains."** | **Seven programs have been permanently deployed on Solana devnet since 2026-08-27/28** (`dclutch/docs/evidence/DEPLOY_1.md` §2, "PERMANENT ADDRESSES"), three devnet markets are `Phase::Open`, market18 carries a live capability root with both participants admitted and funded, and `clutch.dregg.pro` serves from that chain. "Nothing value-bearing" remains true — devnet tokens have no value. "Everything runs on local test chains" has been false for three days |
| `SECURITY.md` | 3 | "Dragon's Clutch is **pre-implementation** and has no deployed funds" | seven deployed programs and ~31.77 SOL of parked devnet rent. The successor's own `SECURITY.md` is already honest about this ("Hacking the shit out of the devnet deployment", a real reporting address) — the wrapper's was never updated to match |
| `SECURITY.md` | 50-51 | "A private reporting address and coordinated-disclosure process **will be added before any public test deployment**" | the public test deployment happened on 2026-08-27/28. The trigger fired; the wrapper still promises. `dclutch/SECURITY.md` has the address (`security@ember.software`) — the wrapper needs a pointer, not a new policy |
| `AGENTS.md` | 44-53 | the entire "Kernel policy" section governs **"the Eggcrate crate"** — `no_std`, no `assume`/`admit`/`external_body`/`cfg(verus_only)` | gen-1 vocabulary. There is no Eggcrate and no Verus in the successor; its kernel policy lives in `dclutch/AGENTS.md` and names Lean. An agent reading the wrapper is given rules for a tree that no longer exists |
| `.github/workflows/pages.yml` | 8-9 | "the site describes an **unreleased local implementation** rather than an official deployment — the landing page and every rendered page carry that labeling" | the site describes a live devnet deployment and says so. The labeling the comment promises is now a different labeling. (Not a `.md`; included because the drift was noted by HEAPRED and this is the file that carries it) |
| `README.md` | 28 | "its static microsite (`archive/gen1/site/` — no longer published; the live site builds from `dclutch/`)" | **correct** — verified against `pages.yml` and PUBLISH's cut. Recorded so nobody re-fixes it |

None of the above was edited: the host repository is out of this lane's scope
by charter, and its README and SECURITY are launch-surface copy.

---

## 5. What this sweep did NOT cover

Stated so the next reader knows the boundary rather than inferring coverage
from silence.

- **`~/dev/dragons-clutch/archive/`** — gen-1, already swept by the Aspiration
  Ledger. Not re-dug.
- **`docs/ASPIRATION_LEDGER.md` and `docs/board-archive-2026-08-27.md`** —
  excluded as archives. The ledger is itself a sweep artifact and the board is
  explicitly not authority. Their M-numbers were used as citations, not audited.
- **`docs/compost/`** and `COMPOST.md` — the compost rules are their own
  process and were not evaluated.
- **`GOAL.md`** — a running done-log with stacked "old thrust" sections. It is
  historical by construction, so staleness there is not a defect. Not swept.
- **`node_modules`** — 2,000+ vendored READMEs, excluded.
- **The 205-row raw commitment inventory.** A full sweep of *every* forward
  promise in `docs/decisions`, `docs/design`, `docs/evidence`,
  `OMISSION_INDEX` and `VALIDATION_BACKLOG` produced roughly 205 rows across 63
  files. §2 carries the ones whose truth **changed today** or that nothing
  outside a pre-compact file carries. The remainder — every `U-` row in the
  omission index, the whole of `VALIDATION_BACKLOG`, `LIVENESS_CENSUS`'s
  Q1–Q10 costed queue, `ORPHAN_DESIGNS_TRIAGE`'s thirteen sized rows,
  `ASPIRATION_ARCHAEOLOGY`'s three tiers — is **genuinely open, already
  written down in a document that owns it, and was not re-litigated here.**
  Re-verifying 205 rows against the tree is a lane of its own; this sweep did
  not do it and does not claim to have.
- **Whether the code matches the docs.** This is a documentation sweep. Where
  it says a commitment is unbuilt, that is a grep for the named symbol (for
  example `MAX_FEE_BPS`, which exists in no `.rs`, `.lean` or `.ts` in the
  tree), not a behavioural test.
- **Three cross-file ordering constraints** surfaced by the commitment sweep
  and left un-audited, because each is a design decision rather than a doc
  defect: Dealer entry must not be built before the exit-affordability
  invariant (`DEALER_EXIT_AFFORDABILITY_V1.md:275`, `LIVENESS_CENSUS:401,425`);
  the Q1 ruling must precede the refcount which must precede
  `CloseActivation` (`LIVENESS_CENSUS:408`,
  `RELEASE_LINEAGE_MIGRATION_V1.md:1305-1312`); and Reaffirm lands before or
  with lineage commit 8 (`RELEASE_LINEAGE_MIGRATION_V1.md:1126-1131`). They
  are recorded here so a lane taking any one of them finds the other two.

---

## 6. The three that would cost the most if they stayed lost

1. **Every "Hot CU" number in the tree belonged to a route that was demoted
   this morning.** `tools/gauntlet/hot-cu` drives the Registry Hot
   continuation, which runs a constant +35,127 CU above the production
   top-level route, and its README, the gauntlet README, `TIERS.md` and
   `VALIDATION_BACKLOG`'s release gate all presented its output as the Hot
   route's compute. `CONTINUATION_ROUTE_FIX_OR_RETIRE` sized the repair at
   *half a lane-hour* and named no owner, which is exactly how a correction
   goes missing.

2. **An evidence document still carried the plan ember reversed.**
   `TRADE_DIRECT_ACTIVATION_WALL_2026_08_29.md` described selecting maker keys
   for their CU draw and publishing them as "selected for CU" — rigging the
   demo and labelling the rig. The reversal was recorded in
   `SESSION_STATE.md`, which is rewritten every compact; the plan was recorded
   in an evidence file, which is not. The durable copy said the wrong thing.

3. **A ruled fee band that nothing enforces.** `MAX_FEE_BPS = 500` is in four
   documents and zero programs. Its only live check is a bash comparison in a
   single staging script — a second author for a protocol bound, in a tree
   whose whole defect taxonomy today was *two authors for one fact*.
