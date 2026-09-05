# Dragon's Clutch master completion contract

Status: active completion contract, not release evidence  
Owner: the root swarm, with Ember holding product/economic rulings  
Started: 2026-08-31

## The stopping condition

Dragon's Clutch is complete only when the current tree implements the whole
accepted project, a stranger can operate it without trusting us, every intended
onchain route has physical execution evidence, and an adversarial closure pass
finds nothing material that should still be added or removed before the
separate assurance phase begins.

A milestone is never the stopping condition. Neither a theorem, a codec, a
fixture, a constructor, a component test, a deployed program, nor one green
campaign proves its surrounding capability complete.

Every recoverable ambition has exactly two terminal states:

1. **implemented** — current code plus the evidence named below proves it; or
2. **ruled out by Ember** — a dated, explicit product ruling says the project
   no longer wants it and why.

Agents may not create a third state by calling an ambition deferred, future,
non-goal, experimental, too large, or out of scope.

## Sources this contract closes over

This file is the completion index, not a second semantic author. Its scope is
the union of:

- `docs/INTENT.md` and its recoverable founder ambitions;
- `docs/OMISSION_INDEX.md` — what is deliberately not built, and the `likely
  scar` and `open research` rows the accepted project still wants lifted;
- the decision records (`docs/decisions/`), the design notes' heads, and the
  dated ledger (`docs/ledger/`);
- the generated instruction, route, refusal, capability, and release censuses;
- current executable code, tests, operator tools, web/SDK/CLI surfaces, and
  their named residual walls;
- substantive backlog discovered while driving the completion campaigns.

If these sources disagree, the implementation does not pick a convenient
winner. Existing explicit rulings govern; otherwise Ember rules the product or
economic choice and the swarm continues independent work meanwhile.

## What counts as physical completion

An intended route or lifecycle is physically complete only when all applicable
layers agree:

1. one semantic owner defines every persisted and signed fact;
2. generated layouts/codecs reproduce that owner exactly;
3. the real program ELF admits the honest case at runtime limits;
4. adversarial cases refuse at the conjunct that owns them with rollback;
5. a local validator or devnet transaction drives the route when the route is
   chain-facing;
6. exact pre/poststate proves conservation, authority, rounding and liveness;
7. an SDK/CLI/operator/browser caller can acquire its inputs without inventing
   authority or requiring a Dragon-operated service;
8. the user-facing surface explains the act, consequence and remedy without
   claiming beyond the evidence;
9. a cold-machine command and machine-readable evidence pack reproduce it.

Simulation and fixtures remain valuable controls, but they do not substitute
for items 3–9. Devnet evidence is public-test evidence, never mainnet evidence.

## Completion matrix

Every row remains **open** until its evidence column is filled with current,
reproducible artifacts and its adversarial closure has been reviewed. Existing
work is adopted into the row; it is not discarded or presumed sufficient.

**Which routes have no campaign** is derived, never copied: `docs/reference/routes.md`
and `docs/reference/route-witnesses.md` are generated from the campaign
bindings and the devnet witnesses, and a route that is structurally undrivable
belongs in `tools/gauntlet/blocked.json` with a reason and an owner. An
unwitnessed route is a statement about coverage, not about correctness. The
hostile walks of this matrix are `docs/evidence/C16_REHEARSAL_2026_09_03.md`,
`docs/evidence/C16_REHEARSAL_2026_09_04.md` and
`docs/evidence/C16_REHEARSAL_2026_09_05.md`, each a delta on the one before it.
Rows are ordered scope closure, the capabilities, then the audit that closes
over them; an identifier is allocated when a row is written and never renumbered,
so `C-17` sits with the capabilities it belongs to rather than at the end.

| ID | Capability that must be finished | Terminal evidence |
| --- | --- | --- |
| C-00 | Scope closure | Every recoverable ambition is mapped here or explicitly ruled out by Ember; no orphan `unfinished`, material TODO, never-executed intended route, or contradictory current guide remains. |
| C-01 | Infrastructure, Registry, release lineage, migration and Rent | Cold bootstrap, upgrade/succession, activation, reauthentication, migration and retirement replay from exact releases; no market is silently rebound; release and profile lineage are walkable; every physical account's rent source/refund has one owner. |
| C-02 | Compiler-shaped product entrance | A human description compiles into exhaustive, disjoint, ordered, canonical product state, payoff basis, source policy, funding plan and immutable identities; the same artifacts found by the operator are explained and inspectable in the client. |
| C-03 | Curved and structured payoff bases | Constant, categorical, degree-1 and admitted degree-2/3 curves execute through the live wire, evaluator, price gate, Claims layout, terminal payout and retirement with the one named integer rounding boundary and hostile certificates. |
| C-04 | Direct | Two independent identities author portable intents, cross onchain, recover interrupted wallet operations, settle fees permissionlessly, redeem and close every Direct root. Relay absence or censorship never removes the portable-ticket path. |
| C-05 | General | Runtime dispatch turns the authored action set into a complete best-valid-submitted-candidate lifecycle: collect, consider, freeze, materialize, distribute, expire, resume and close on real ELFs. No claim of optimal clearing exists without a checked certificate. |
| C-06 | Dealer | The accepted Dealer profile completes activation, capital admission, liquidity addition/removal, inventory-bounded trading, consent-safe policy/epoch evolution, exact scenario solvency, LP withdrawal and retirement for multiple LPs. Any broader venue generalization is implemented or explicitly ruled out. |
| C-07 | Series | Recurring Series found, prepare, issue/consume/expire tickets, redistribute funding once, settle occurrences, close terminal state and replay safely across more than one occurrence. **The six routes that must be witnessed are named**: `core/series_consume`, `core/series_open`, `core/series_permit_expiry`, `core/series_permit_expiry_precommit_v1`, `claims/series_founding_transport_v1`, and Trading's `hot_v3::process_hot_execution_v3` under `try_authenticate_series_expiry_premarket_v1`. **Evidence class today, 2026-09-04: real-ELF ProgramTest, one route.** Only `core/series_consume` is witnessed, by `tier4-series-occurrence-programtest` on an in-process bank; nothing Series has ever been on a chain, so **cohort-16 is the first devnet this family can reach** and it must carry program changes — `97ce7a748` moved the Expire RequestProfile and Effect and the Consume Effect when the family's proof geometry was keyed on the Template that owns it, and a Series release compiled before that commit is not the one a cohort-16 market would run. `8b5d1c96f` moves it again — a new refusal code `0x402A` and the Expire pre-Market conjunct that carries it — and `2cf96117a` moves BOTH the Trading and the Core digests, because the four Core Series routes and Trading's Series artifact join changed what they compare the root's config field against. So the digest move rides **cohort-16**, which still has no manifest (`tools/cohort/cohorts/` holds 14 and 15); no devnet act is taken for it here. **The blocker list is now three, and the config identity is no longer one of them.** (a) DISCHARGED in `2cf96117a`: a Series root's `selection().config()` has ONE author and it is the Registry record digest `hash(config_record_bytes)`, as every other family's is. It was never a choice between conventions — a Registry record's coordinate is `[RAW_RECORD_PDA_SEED_V1, schema, digest]` with `digest == hash(bytes)`, so `template_content_id(t)` names a coordinate at which no record can exist, and `selected_manifest_entry_v1` has always written the record digest for every family. The six Series sites now derive the Template's content identity from the config record's bytes they already hold; `SeriesArtifactSelectionV3` has private fields and one constructor so the root's config field is unspellable there; and the sixth site, `series/accounts.rs::authenticate_root`, was an orphan with zero callers and is deleted. Proved natively by `series_premarket_expiry_chain_v1::native_tests::the_series_root_config_identity_has_one_author`. (b) The `TicketStateV3` producer `prepare_funding_artifacts_v5` declares authority for and never exercises (`series-v3-kernel/src/replay.rs:335`) — still open, and still needing the Expire route to reach Core. (c) The absent dispatched Series Hot route — `build_series_{prepare,consume,expire}_hot_v3` still have NO caller, and the seam upstream of them is the uncommitted common authenticated Shadow callback (`series-shadow-sbf/program-test/README.md`). (d) NEW and not diagnosed: the pre-Market Expire campaign's three rows now refuse `Release` (`0x4001`) at **533,198 CU of 1,316,619** in the preflight child composition, 202,211 CU past the config-record borrow where they refused `Content` (`0x4003`) at 330,987. The code changed, so it is a different accusation. Getting there also fixed a **Trading defect**: `sealed_ownership.require` was unsatisfiable for every capability whose account profile is schema V3, because the verdict is minted over the whole record the token names while the require site presented the interior `funding.base()` — compared by pointer identity, and never reached before because no schema-V3 family had executed that far. The `0x4001` itself is LOCALIZED and not repaired: `resolve_carrier_by_representative_v3` cannot find the activated **Custody** program at any coordinate of the 81-coordinate downgraded logical frame (`dclutch-hot-why:role-carrier` case 3, key prefix `0x95959595`). `CustodyFrameSpecV1` has no callee role, so no Custody route window carries it, and the reading this points at is that the Expire AccountProfile is short a coordinate — which moves `SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5`, every route start and every alias, and the Trading digest with them. Whether the profile or the bundle builder is the defect is NOT settled. The pre-Market Expire campaign's "five uninstalled Series record raw/staging accounts" is REFUTED and gone: `96055c100` showed all forty-four physical accounts had been built since `1b8191f9d`. |
| C-08 | Structured/Fractional representation | Useful exact-denominator products create native claims, wrap/transfer/compact/unwrap or dematerialize them through real Token-2022 and Custody effects, redeem terminal value and retire without hidden remainder, absent-holder charge or stranded rent. |
| C-09 | Objective resolution | Accepted provider evidence and disclosed fallback paths execute with real provider verification, immutable windows, first-valid semantics, permissionless completion, reclaim and fund closure; no client or operator authors truth. |
| C-10 | Claims, Custody and terminal lifecycle | Complete-set issuance/redemption, transfers, fees, representation, terminal payout and every close conserve each asset independently from founding through `Retired`; Hoard principal never funds another class. |
| C-11 | Liveness and sustainable economics | Permissionless work can be completed and paid from explicitly classified sources without future-revenue capitalization or Hoard principal. Fee rates, beneficiaries, opener shortfall, upkeep vault and donation treatment are modeled adversarially and receive Ember's explicit economic rulings before implementation. |
| C-12 | Stranger-operable product | Static web, Wallet Standard, SDK and CLI support creation, discovery, maker/taker trade, liquidity, Series, resolution, portfolio, redemption and recovery — where *recovery* here means **recovery of an interrupted wallet operation**, not fallback-source recovery, which is the separate open row "Recovery ontology: keep or cut" below. Inputs are typed/derived with provenance; relays and indexes remain optional untrusted projections; mobile and accessible interaction is complete. |
| C-13 | Operator and cold-machine operation | A cold machine can build checked releases, bootstrap, create representative markets, drive every lifecycle, recover interruptions and inspect/export/sign/submit only the intended acts. Runbooks contain only commands actually replayed by their campaigns. |
| C-14 | Reproducible release readiness | Pinned toolchains, deterministic artifacts, SBOM/licences, source digests, migrations, compute/frame/packet ceilings and checked release manifests reproduce on supported builders. Devnet may be deployed and mutated freely for this work. Mainnet deployment remains a separately authorized external act until Ember rules its place in completion. |
| C-15 | Privacy/FHE/MPC horizon disposition | The specialized batch relation and the original privacy/energy ambition are implemented in the accepted project or explicitly ruled out by Ember; an old horizon decision is not silently treated as permanent completion scope. |
| C-17 | Alternative execution strategies for one semantic descriptor | Interpreted execution and translation-validated stateless AOT run the SAME authored descriptor as the compiled path, with exact equivalence to Direct under a checked certificate, a Registry-bound artifact and toolchain identity, refusal equivalence conjunct for conjunct, rollback, and a CU/packet/rent comparison against the compiled route — or Ember rules the strategies out. **This row is the rehomed `U-014`.** `docs/OMISSION_INDEX.md` retired its fifteen `U-` rows on 2026-09-04 on the ground that their subjects are this matrix's rows; fourteen were, and this one was not, so the retirement created for one subject the third state the scope section forbids. **What survives at HEAD**: `crates/dclutch-direct-aot-contract` and `tools/direct-translation-validator` (the Lean-emitted corpus, the executable validator and its Kani proofs). **What does not**: `dclutch-direct-aot-sbf`, deleted 2026-09-04, `false` in `SHIPPED_LINKS`, in no cohort, its band 10 RETIRED by the refusal registry — so no interpreted or AOT descriptor has ever executed on any chain, and the accepted ambition is stated only as the lifting path of `O-003` and `O-004`. |
| C-16 | Assurance-entry audit | Independent hostile reviewers walk this matrix against current source and artifacts. No known material gap, unexplained authority, stale claim, unowned economic flow, never-executed intended route, or user-inaccessible capability remains. Only then may the project say it is ready to begin assurance. |

## Representative end-to-end campaigns

The final evidence set must contain at least these current-source campaigns.
Each starts from a fresh work directory, emits a machine-readable ledger, and
ends with all temporary protocol state either deliberately live or closed.

1. **Direct complete life** — compile → found/open → admit two participants →
   author ticket → cross → third-party fee completion → provider resolution →
   redeem → close maker/positions/children/root → `Retired`.
2. **General complete life** — compile/found → activate General → collect more
   than one valid candidate → select the best valid submitted candidate →
   materialize/distribute → terminal resolution/redemption → close.
3. **Dealer complete life** — compile/found → activate Dealer → admit multiple
   LP tranches → trade across inventory states → consent-safe evolution →
   withdraw all LP value → resolve/redeem/retire.
4. **Recurring Series life** — compile/found → execute at least two occurrences
   including consume and expiry paths → settle funding exactly once → terminal
   close and retire.
5. **Curved Structured/Fractional life** — compile an admitted nontrivial curve
   → found → issue exact fractional representation → transfer and permissionless
   compaction → unwrap/dematerialize → resolve/redeem → retire.
6. **Failure/recovery ensemble** — source silence/fallback, interrupted wallet
   signing/submission, stale nonce, substituted release/account, competing
   permissionless closer, and restart from durable journals all end safely.

## Swarm operating structure

The root maintains this contract and converges the shared tree. Persistent
capability crews own the capability rows -- C-01..C-15 and C-17 -- and their campaign slices; they do not return an
inventory as their deliverable. A crew that finds an in-scope seam owns it to
physical closure unless it requires an Ember ruling or conflicts with another
crew's active shared entrypoint.

- Crew checkpoints are material executable advances, not file counts.
- Terra handles bounded mechanical work inside a crew; it does not own a
  substantive capability.
- Shared program dispatchers and persisted layouts require explicit root
  coordination before edits.
- Generated artifacts are regenerated by their owner and checked on both web
  and SDK twins.
- Every new invariant or parser gets the honest control, targeted hostile and
  mutation/refutation evidence appropriate to its risk.
- Commits use exact named paths. Existing unrelated dirty files are preserved.

### Execution substrate

- Local Mac execution is ordinary infrastructure.
- `persvati` is available for builds, validators and campaigns.
- `hbox` is available for any heavy Linux build/campaign; every invocation uses
  `swarm-build` and respects co-tenant workloads.
- Devnet reads, writes, deployments and disposable markets are authorized for
  completion work. Evidence records exact program/release identities and never
  generalizes devnet success to mainnet.
- Mainnet, real-value assets, wallet/private-key dotfiles, browser sessions,
  publication and destructive external production actions require a new
  explicit authorization.

## Backlog admission

Backlog work is ruled in when it does at least one of the following:

1. closes or materially advances a C-row or representative campaign;
2. removes a systemic defect class that could invalidate more than one row;
3. restores a broken completion gate or single-author derivation;
4. turns an operator/user wall into a working path; or
5. produces evidence required by a row's terminal test.

Cosmetic churn, audit-only inventories, naming-only work, and isolated cleanup
with no completion consequence wait. This is prioritization, not deletion: an
item that the final C-00 sweep shows the accepted project wants must still be
implemented or ruled out.

## Decision register

Economic, product, legal and genuinely new wire choices are recorded here as
short questions when they become the only wall on a seam. The swarm asks Ember
one question, records the answer in its owning decision/ruling, and continues.
Silence is never a ruling.

| Decision | Why completion needs it | State |
| --- | --- | --- |
| Mainnet act | Whether mainnet deployment is part of feature completion or follows assurance | **RULED**: mainnet follows assurance — [0026](decisions/0026-mainnet-follows-assurance.md). No mainnet authorization exists |
| Privacy horizon | Whether the accepted project includes the original FHE/MPC/energy objective | **RULED OUT 2026-09-01 by ember** — [0018](decisions/0018-privacy-horizon-not-this-clutch.md). C-15 closes on it; `O-019` stays open and load-bearing |
| Sustainable economics | Exact fee, upkeep, donation and opener policies after adversarial modelling | **RULED, amended by ember** — [0024](decisions/0024-sustainable-economics-and-a-governable-parameter-surface.md); the models are `docs/design/ECONOMICS_MODELS_2026_09_04.md`; the upkeep vault is chartered and unbuilt |
| Recovery ontology | Whether the recovery policy is kept or cut | **KEPT AND BUILT** — [0027](decisions/0027-recovery-is-one-funded-ordered-ladder.md): one funded ordered ladder, walked end to end on real ELFs; the loopback tier is a lane |
| An outage's payout | Whether a source failure pays the founder or refunds | **RULED** — [0025](decisions/0025-an-outage-refunds-rather-than-paying-the-founder.md) |
| Devnet succession | Whether the `ProfileV1 -> V2` ceremony must run before a release candidate | **RULED 2026-09-01: no.** Devnet is disposable ([0012](decisions/0012-devnet-iteration-substrate.md), [0019](decisions/0019-authenticated-devnet-target-set.md)); the ceremony is a capability, not a blocker. Open corollary: whether C-01 wants succession EXECUTED before assurance |
| Registered Direct | C-04 wants the whole registered lifecycle | **open, partly built**: `RegisterSell`/`RegisterBuy` dispatch; the fills, terminals, splits and merges still refuse `UnsupportedContent` in `programs/dclutch-trading-sbf/src/hot_v3.rs` (the comment above the refusal names them). Spec: `docs/design/DIRECT_HOT_ACTION_ADMISSION_V1.md`. The gate must become a dispatch, and `FillRegisteredOrdinary` keeps the ordinary crosscheck. The batch-spine design ([0031](decisions/0031-the-mechanism-agenda.md)) proposes deleting the registered branch instead |
| `derivation_policy`, protocol-wide | Pinned per descriptor to its own lifecycle digest AND per root to the manifest entry; a multi-selector set cannot satisfy both | **open, fully scoped, a release event**: every capability-root address moves, so markets must be re-founded. No lane may start it without a cohort that carries it |
| Structured account widths | The account profile declared `Exact{0}` for coordinates nothing can satisfy | **decided and proven, needs a release event**: the remaining opaque widths change the profile bytes and so the release identity |
| The flagship conditional market | Its feature gate, slot and metric | **OPEN, ember's** — [0029](decisions/0029-the-product-list-nine-rulings.md) item ten |

## Closure procedure

The root may mark the master goal complete only after:

1. every C-row links to current authoritative evidence or an explicit Ember
   ruling;
2. every representative campaign replays from cold source on a named substrate;
3. the full generated references, web/SDK, program, frame/compute/packet,
   conservation and hostile gates pass at the committed source digest;
4. an independent adversarial crew attempts to disprove every row and all
   findings are fixed or explicitly adjudicated; and
5. a final tree/search/reference pass finds no material unfinished capability,
   stale public claim, unexplained omission or unowned fact.

If any condition is missing, the project is not complete and the goal remains
active.
