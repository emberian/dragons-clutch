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
- `docs/OMISSION_INDEX.md`, including every `unfinished`, `likely scar`, and
  `open research` item that the accepted project still wants;
- `WAVE.md`, `GOAL.md`, accepted decisions and design rulings;
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

**Which of YOUR row's routes have no campaign:**
`docs/evidence/UNWITNESSED_ROUTES_BY_ROW_2026_09_01.md` breaks the register's 55
`NEVER-EXECUTED` routes down per row and names every one — C-01 4, C-02 5,
C-04 4, C-06 8, C-07 2, C-08 5, C-09 14, C-10 13. An unwitnessed route is one no
campaign drives; it is a statement about coverage, not about correctness. A
route that turns out to be structurally undrivable belongs in
`tools/gauntlet/blocked.json` with a reason and an owner, not left in that list
looking like unstarted work. The entry list for C-00/C-16 is
`docs/evidence/C16_ENTRY_LIST_2026_09_01.md`.

| ID | Capability that must be finished | Terminal evidence |
| --- | --- | --- |
| C-00 | Scope closure | Every recoverable ambition is mapped here or explicitly ruled out by Ember; no orphan `unfinished`, material TODO, never-executed intended route, or contradictory current guide remains. |
| C-01 | Infrastructure, Registry, release lineage, migration and Rent | Cold bootstrap, upgrade/succession, activation, reauthentication, migration and retirement replay from exact releases; no market is silently rebound; release and profile lineage are walkable; every physical account's rent source/refund has one owner. |
| C-02 | Compiler-shaped product entrance | A human description compiles into exhaustive, disjoint, ordered, canonical product state, payoff basis, source policy, funding plan and immutable identities; the same artifacts found by the operator are explained and inspectable in the client. |
| C-03 | Curved and structured payoff bases | Constant, categorical, degree-1 and admitted degree-2/3 curves execute through the live wire, evaluator, price gate, Claims layout, terminal payout and retirement with the one named integer rounding boundary and hostile certificates. |
| C-04 | Direct | Two independent identities author portable intents, cross onchain, recover interrupted wallet operations, settle fees permissionlessly, redeem and close every Direct root. Relay absence or censorship never removes the portable-ticket path. |
| C-05 | General | Runtime dispatch turns the authored action set into a complete best-valid-submitted-candidate lifecycle: collect, consider, freeze, materialize, distribute, expire, resume and close on real ELFs. No claim of optimal clearing exists without a checked certificate. |
| C-06 | Dealer | The accepted Dealer profile completes activation, capital admission, liquidity addition/removal, inventory-bounded trading, consent-safe policy/epoch evolution, exact scenario solvency, LP withdrawal and retirement for multiple LPs. Any broader venue generalization is implemented or explicitly ruled out. |
| C-07 | Series | Recurring Series found, prepare, issue/consume/expire tickets, redistribute funding once, settle occurrences, close terminal state and replay safely across more than one occurrence. **The six routes that must be witnessed are named**: `core/series_consume`, `core/series_open`, `core/series_permit_expiry`, `core/series_permit_expiry_precommit_v1`, `claims/series_founding_transport_v1`, and Trading's `hot_v3::process_hot_execution_v3` under `try_authenticate_series_expiry_premarket_v1`. **Evidence class today, 2026-09-04: real-ELF ProgramTest, one route.** Only `core/series_consume` is witnessed, by `tier4-series-occurrence-programtest` on an in-process bank; nothing Series has ever been on a chain, so **cohort-16 is the first devnet this family can reach** and it must carry program changes — `97ce7a748` moved the Expire RequestProfile and Effect and the Consume Effect when the family's proof geometry was keyed on the Template that owns it, and a Series release compiled before that commit is not the one a cohort-16 market would run. `8b5d1c96f` moves it again — a new refusal code `0x402A` and the Expire pre-Market conjunct that carries it — so the digest move rides **cohort-16**, which has no manifest yet (`tools/cohort/cohorts/` holds 14 and 15). **The blocker list is now three, and the first is new and is a program ruling, not a fixture gap.** (a) A Series root's `selection().config()` is required to be two values that cannot be equal: the Registry record digest `hash(config_record_bytes)`, by the family-neutral Hot prelude (`hot_v3.rs::borrow_finalized_record_at`) and by the production operator (`operator/src/series_hot_v3.rs`, which requires those bytes to be the Template record); and the domain-separated `template_content_id`, by six sites — `series/accounts.rs`, `series/artifacts_v3.rs`, and Core's `series_open`, `series_consume`, `series_permit_expiry` and `series_permit_expiry_precommit_v1`. Proved natively by `series_premarket_expiry_chain_v1::native_tests::the_series_root_config_identity_has_two_authors_that_cannot_agree` and measured from both ends on real ELFs. It is why nothing Series has ever executed through the family-neutral Hot path, and either repair moves two ELFs and the convention `core/series_consume` is witnessed under. (b) The `TicketStateV3` producer `prepare_funding_artifacts_v5` declares authority for and never exercises (`series-v3-kernel/src/replay.rs:335`). (c) The absent dispatched Series Hot route — `build_series_{prepare,consume,expire}_hot_v3` still have NO caller, and the seam upstream of them is the uncommitted common authenticated Shadow callback (`series-shadow-sbf/program-test/README.md`). The pre-Market Expire campaign's "five uninstalled Series record raw/staging accounts" is REFUTED and gone: `96055c100` showed all forty-four physical accounts had been built since `1b8191f9d`. |
| C-08 | Structured/Fractional representation | Useful exact-denominator products create native claims, wrap/transfer/compact/unwrap or dematerialize them through real Token-2022 and Custody effects, redeem terminal value and retire without hidden remainder, absent-holder charge or stranded rent. |
| C-09 | Objective resolution | Accepted provider evidence and disclosed fallback paths execute with real provider verification, immutable windows, first-valid semantics, permissionless completion, reclaim and fund closure; no client or operator authors truth. |
| C-10 | Claims, Custody and terminal lifecycle | Complete-set issuance/redemption, transfers, fees, representation, terminal payout and every close conserve each asset independently from founding through `Retired`; Hoard principal never funds another class. |
| C-11 | Liveness and sustainable economics | Permissionless work can be completed and paid from explicitly classified sources without future-revenue capitalization or Hoard principal. Fee rates, beneficiaries, opener shortfall, upkeep vault and donation treatment are modeled adversarially and receive Ember's explicit economic rulings before implementation. |
| C-12 | Stranger-operable product | Static web, Wallet Standard, SDK and CLI support creation, discovery, maker/taker trade, liquidity, Series, resolution, portfolio, redemption and recovery — where *recovery* here means **recovery of an interrupted wallet operation**, not fallback-source recovery, which is the separate open row "Recovery ontology: keep or cut" below. Inputs are typed/derived with provenance; relays and indexes remain optional untrusted projections; mobile and accessible interaction is complete. |
| C-13 | Operator and cold-machine operation | A cold machine can build checked releases, bootstrap, create representative markets, drive every lifecycle, recover interruptions and inspect/export/sign/submit only the intended acts. Runbooks contain only commands actually replayed by their campaigns. |
| C-14 | Reproducible release readiness | Pinned toolchains, deterministic artifacts, SBOM/licences, source digests, migrations, compute/frame/packet ceilings and checked release manifests reproduce on supported builders. Devnet may be deployed and mutated freely for this work. Mainnet deployment remains a separately authorized external act until Ember rules its place in completion. |
| C-15 | Privacy/FHE/MPC horizon disposition | The specialized batch relation and the original privacy/energy ambition are implemented in the accepted project or explicitly ruled out by Ember; an old horizon decision is not silently treated as permanent completion scope. |
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
capability crews own C-01..C-15 and their campaign slices; they do not return an
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
| Mainnet act | Whether actual mainnet deployment is part of feature completion or follows assurance | open; no mainnet authorization |
| Privacy horizon | Whether the accepted final public project includes the original FHE/MPC/energy objective | **RULED OUT 2026-09-01 by ember** (`GOAL.md:2071`, commit `5a371810`; recorded at [`docs/decisions/0018-privacy-horizon-not-this-clutch.md`](decisions/0018-privacy-horizon-not-this-clutch.md)). Verbatim: *"privacy/FHE is a 'not yet' for sure for sure, that would be a much later version of Clutch, solana isn't ready for that kinda awesomeness onchain yet (we'd want to use minidregg, which isn't ready yet)."* A scope ruling on the accepted project, dated, with the prerequisite named — a terminal state, not the third one. C-15 closes on it. `GOAL.md:2093` forbids reporting the horizon as deferred, future work, or in-progress. The ruling's own safety condition is that `O-019` STAYS OPEN and becomes load-bearing (`docs/OMISSION_INDEX.md:59`): widening the batch relation closes the door the ruling deliberately left open |
| Sustainable economics | Exact fee/upkeep/donation/opener policies after adversarial modeling | open; Ember owns each economic choice |
| Recovery ontology: keep or cut | `RecoveryPolicyV2`, `RecoveryAttemptV2`, `source_recovery_policy_v2.rs` and recovery funding were live, green and kernel-complete with NO on-chain route: `funded::process_funded_transition` had no definition, and `SourceResolutionStateV2` had exactly three transitions of which none advanced a recovery attempt, so a market founded with a recovery policy could not be terminalized at all. Core welded `CreateFund` shut against the shape rather than let another such market exist | **KEPT AND BUILT.** Ruled by decision [`0027`](decisions/0027-recovery-is-one-funded-ordered-ladder.md) under ember's standing goal, amended by ember to require robust failure pathways (`GOAL.md:4654`): recovery is ONE funded ordered ladder. Built by the RECOVERY lane: the Lean machine gains the transition and its theorems (every entered attempt is funded; the ladder is finite; from a closed window exactly one of advance and exhaust fires; `Exhausted` is reached only after the LAST funded window's own deadline; the two ends are `Resolved` and `FailureCommitted` and `Exhausted` is neither), `SourceResolutionStateV2::crank_recovery_ladder` is the one transition, `funded::process_funded_transition` is defined and `RelayActionV1::AdvanceRecovery` is its route, and the weld is deleted. Walked end to end on real ELFs — found with a two-source policy, primary window closes unobserved, a stranger advances onto the funded alternative, that window closes, the same stranger exhausts it, and the failure walk commits the Product's own selector: 216,637 / 218,163 / 227,662 CU, three compartments spent once each (`crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs`, `a_two_source_market_walks_its_funded_ladder_and_every_rung_pays_a_stranger`). **The capture and the funding landed after, by RECOVERY-2.** The honest recovery capture has a producer: the real-Pyth outer dispatches on `provider_v3::select_rung`, which reads WHICH source may answer off the market's own `active_attempt` and requires the request's new `ProviderExecutionRequestV3.source_index` to agree, so a caller can name a rung and never choose one. A market is now ANSWERED on its funded alternative end to end on real ELFs — `a_market_is_answered_on_its_funded_second_rung`, terminal route `Recovery`, `attempt_index` 1, Core accepting: advance 215,138 CU, capture 311,232 CU. The rung is byte-cheap on the honest path: `source_index` took a byte the request already reserved, the two extra accounts ride at the frame's tail, and the primary campaigns pass unedited. Founding now funds EVERY rung — attempt `k` is paid by the manifest entry at `recovery_entry_index + k`, one index still naming the whole run — so the `attempt_count() != 1` refusal in both authors is gone and a policy may carry the four the record, the emitted layout and Lean all always carried; `RecoveryPolicyV2::validate_shape` refuses two attempts sharing one funding allocation, because one identity is one compartment. Two gates were wrong and only the capture producer could expose them: a submission against a market standing on a funded rung — a market that can still consume it — was reclaimable by a stranger, and `RESOLUTION_CAPTURABLE_SOURCE_ADMISSIBLE_STATES_V1` is now the reclaim set's exact complement. **Still owed:** `CoreSbfError::RecoveryWalkUnavailable` `0x3011` has no producer and stays allocated only because Core's discriminants are asserted contiguous, so retiring it is a renumbering release; no successor driver founds a recovery-bearing market or cranks the ladder, so cohort-16 has no `found-two-source` / `crank-ladder` rows; and the ladder's transactions emit no census evidence, for the reason `tools/gauntlet/blocked.json` now states correctly. The harness README's CU table for the deleted V1 campaign is corrected |
| Devnet succession | Whether the never-executed `ProfileV1 -> V2` ceremony must run before a release candidate | **RULED 2026-09-01: no.** Devnet is disposable — redeploy fresh, abandon cohort-8 in place. Ceremony kept as capability, demoted from blocker. Open corollary: whether C-01 wants succession EXECUTED at all before assurance |
| Registered Direct has no route | `hot_v3.rs:5372` refuses every Direct-kind Hot action except `InlineOrdinary` — RegisterSell, RegisterBuy, the fill, both cancels, expiry, every close, both splits and merges. C-04 wants all of them | open; spec at `docs/design/DIRECT_HOT_ACTION_ADMISSION_V1.md` (`35a7fa6f`). Measured `UnsupportedContent` 0x4000 at 323,523 CU, 451 CU past `preflight-children`, so the action is a complete admitted preflighted act when its KIND is rejected. Behind a probe the registered Sell EXECUTES at 374,455 CU with exact poststates — the first ever created on a chain; the probe was deliberately NOT committed. **The gate must become a DISPATCH, not a deletion:** `Ok(None)` is right for creation and the terminals (a foreign kind already takes that path, leaving Transition and Effect as sole authority, as every other family works) but WRONG for `FillRegisteredOrdinary`, which IS the registered form of the settlement the ordinary crosscheck was written for — waving it through would give the most consequential registered action less checking than its inline twin has today. Note two things are NOT blocked: `CloseMakerReplay` and Direct retirement are served today by dedicated `DCLTDMC1`/`DCLTDBR1` top-level instructions, green on real ELFs |
| `derivation_policy` — PROTOCOL-WIDE (was "Dealer") | The field is pinned per-descriptor to its own lifecycle digest AND per-root to the manifest entry; a multi-selector set cannot satisfy both | open, and now fully scoped. Convicted ON-CHAIN in TWO families by two independent routes (Dealer by CU-bracketed bisection; Direct by swapped-fixture experiment). Per-selector manifest entries and a shared lifecycle body are both UNREPRESENTABLE. Dropping the per-descriptor rule IS sufficient — its three sites span two ELFs, and with both corrected the Dealer Add clears the seam (166,768 -> 582,773 CU, hostile still refusing). **What makes it a release event:** the field is not a direct PDA seed, but two capability-root seeds are digests containing it, so every capability-root ADDRESS moves — on-chain markets cannot migrate in place, they must be re-founded. ~84 producer sites, ~43 validation, ~60 persisted/digest (including an ungated `HotExecutionAckV3.execution_digest` in the Trading ELF), ~205 tests/Lean/TS/docs. Stacks with the registered-Direct routing gap on the same family. **No lane may start it** |
| Series activation funding | The seam required an activated root to end at exactly `rent.minimum_balance(width)`, but Close required `root_rent + close_rent_remaining` | **RESOLVED 2026-09-01, no ruling needed — the question inverted.** The funding contract had modelled the principal end to end all along: `FundingCompartment::Creation` is parked at founding, required present by Core, released by `activate_in_place`, and returned as `ActivationDebitV1.creation_lamports` — and `outer.rs:1606` simply discarded that value. So a nonzero quote was both unactivatable and unreleasable. Repaired `e75b279c`: the root must now end at `rent + creation`, with rent independently pinned to the live Rent sysvar so a manifest can neither underfund below exemption nor inflate the reserve. Control: families declaring no creation principal keep byte-identical artifacts. **CLOSED `929095f5`: Series activates on a real ELF from its own Template** — a genuine `TemplateV3` config record, the triple `build_series_activation_bundle_v1` publishes bound through `capacity_profile`, the six-entry program set reached by selector 255 (a value `SeriesActionV3::decode` can never produce), and the root decoded under the Template's own occurrence count holding exactly reserve + principal, which is the balance terminal Close requires. Real-ELF evidence also REFUTED an earlier commit of this lane's own: `e75b279c` claimed the rent quote was pinned to the live Rent sysvar, but the quote is a TOP-UP (a vacant root may carry dust) — corrected in `9054c904` to pin the poststate instead, byte-identical when no principal is declared |
| Structured account widths | The account profile declares `Exact{0}` for sixteen coordinates including the Rent sysvar and the System program; nothing can satisfy it, and the same array ships in production | **open, but fully decided and PROVEN — needs only a release event.** No coordinate was ambiguous: every mismatch is read, never created. Five knowable widths already landed separately (`d4cd3b27`, 16 mismatches -> 9, no identity touched). The remainder is `opaque += {2, 3, 10, 12, 14, 15, 33}`, which cannot be separated because it changes emitted profile bytes -> artifact digest -> release identity. With the full table applied in an isolated tree the account projection PASSES and all 16 mismatches go. Ember schedules the release; the engineering is done |

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
