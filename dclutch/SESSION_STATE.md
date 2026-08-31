# SESSION STATE — read top-down; newest first. Compact #3 header, refreshed 2026-08-31 ~03:3x EDT

## CURRENT STATE (post-compact reader: start here, everything below is the log)

**FULL AUTONOMY IS IN FORCE (ember, 2026-08-31, recorded in WAVE.md):** do
anything, operate any CLI, tear down/redeploy markets, change the protocol.
**TRADE-4 fires the first public trade itself** (the browser-click hold is
RESCINDED), then keeps devnet alive: more fills + 2-3 diverse-shape markets.
Ember steers tonight: markets must be GENUINELY UNCERTAIN at founding
(buckets centered on spot, width ~ vol × window; vary question types) and
the site design is the next strike — "text too small / imbalanced /
rethink the graphic design" (DESIGN lane queued behind GRICE; standard in
memory dclutch-web-aliveness-patterns).
Cohort-7 live (5 roles, set `d202e1f4…`); market19 Open/activated/admitted;
site at clutch.dregg.pro (PUBLISH-3 shipped `2c33f821`); CLI release
v0.1.0-devnet.2. Standing goal: work until 10am, protocol excellent and
complete; GOAL.md is the done-log.

**THE NIGHT WAVE — resume any lane with SendMessage to its id:**

| lane | id | mission |
|---|---|---|
| FINALIZATION | `ac4ae0b8c46cb7943` | split the ten collapsed refusals; land the FIRST LOCAL FILL (substrate: validator 43080, preserved) |
| FEE-TX2 | `a86fef5170d01f9bd` | build tx2 (fee settlement) on FEE-CORE's seam + FEEPROOF's foundation |
| FRACCHECK-2 | `ab8fdba6b8bc62789` | the Trading half: SetAuthority hand-off route + split-controller read_mint |
| SIMLIFE-3 | `acf661074f8de21b8` | wire activation into the drivers; run the long world; morning /population; outcome-spread is a health metric |
| HYGIENE | `a9471dfe4a411c7cd` | 8 ledgered debts: general-v5 scraper, generator gate, twins gate, ceiling pin, dead const, 4 lockfiles, Claims tripwire, careful reap |
| TRADE-4 | `adfc58bb7c5f10d24` | devnet: manifest → RUN `hot` (the first public trade) → more markets per the uncertainty steer |
| CLOSESEAL | `aad34d691d8e715f4` | E3 ruled collector-keeps-capped: CloseSeal route, write-once preserved |
| GRICE | `ab29cc70c73c5e7da` | strike-five minimalism (registry stories, wallet panel, chips); DESIGN lane spawns on its landing |
| DONE tonight | EXPLORER `ae7edb4b1f9ce81db` | dialect pass landed `13d9359c`; 10 web reds are other lanes' in-flight strings — PUBLISH-4 adjudicates |

**Ember-pending:** Helius key rotation only (recommended, transcript
exposure). E3 is RULED (collector-keeps, capped — WAVE.md `4f792663`).
**Hard rules that bind every lane:** devnet writes are TRADE-4's alone;
market19/job-dir untouchable by everyone else; validators
43080/26900/27100/29300/34500 preserved; lane.sh commits with named paths —
NEVER a broad add (f346ba81 swept a sibling's file tonight);
frame-diagnostic grep pattern from run.sh never memory; refusals name their
clauses; a checker with a wrong pattern answers no; honesty is silent on
the page; resume 429-killed lanes warm, never relaunch.

---

# The log (oldest header below; sections were appended newest-above-oldest at the log's top)

## (superseded original header) SESSION STATE — 2026-08-30 ~16:50 EDT

Read this first. Written immediately before a `/compact`, so it assumes the
reader has no memory of the session. The wave board at
`/private/tmp/dclutch-wave2-board.md` is the full record (long); `GOAL.md` is
the done-log; `WAVE.md` carries the rulings.

## THE ONE THING THAT MATTERS

**The public Direct Hot route does not fit under the 1,400,000 CU ceiling for
arbitrary keys, and that is the only thing between here and the first trade
on a public dClutch market.**

Everything else is built: market18 is open on devnet, both participants are
admitted and funded, the first capability root in the protocol's history is
live, the heap wall is closed, the manifest producer exists, and the load
simulator is sustaining on live devnet. Seven of the first trade's eight
stages have finalized on chain. Only the eighth is blocked.

Measured on clean main (CUCUT, `ff9112c1`, all eight ELFs rebuilt, 32 seeds):
worst seed **1,393,616** against the ceiling — **6,384 CU of margin**, and the
checked-in gate is already red. Earlier estimates of 18,424 were optimistic.

## THE PLAN TO CLOSE IT (in flight)

The band is **100% bump-search depth** — every gap between observations is a
multiple of ~1,500 CU, the cost of one `find_program_address` attempt. One
transaction makes ~42 searches, ~16 of which vary with the key draw, worth
~63,000 CU above the unavoidable minimum. Nearly every survivor is *across a
CPI boundary*: Trading finds an address, discards the bump, and the child
searches for the same address again. The Market PDA alone is searched four
times from identical seeds.

- **CUCUT — DONE (reported ~17:00).** Delivered the full design as
  `docs/evidence/DIRECT_HOT_BUMP_CARRY_DESIGN_2026-08-30.md` (ba80646f +
  dc028078) but **did not land the carries**, with the number that justifies
  it: one new field on `CustodyRequestV1` breaks **107 struct-literal sites in
  48 files** (Claims: 15), in a contract crate a dozen lanes build from. Each
  carry alone is ~1/16 of the band. The caller-authority circularity is
  SOLVED, not open: `role_request_digest` hashes only the request-struct
  bytes, so a bump appended AFTER them sits outside the fixed point (needs
  the digest-width pin test the doc specifies). CoreState **cannot** take a
  bump as a Rust change — it is Lean-generated
  (`EmitMarketCoreRust.lean`), so that path is a formal-spec change + regen +
  account migration, its own lane.
- **BUMPREC — DONE.** Census landed (`8a72b259`/`6367fae2`/`ff65b882`,
  evidence only): the "18" constant searches are 14 over six record pairs,
  **28,500 CU**, of which the **realm pair alone is 18,000** (paid twice,
  once per Custody CPI) and two pairs are worth zero (already at 255).
  Rebuild-invariant, control fired. A throwaway realm conversion took the
  margin gate from refusing at seed 13 to **32/32 with 21,230 CU margin**
  despite a 9,000-CU-worse cache draw. But every carrier is full (capability
  root 4/4, selection 2/2, MarketRoot off-route; wire can't help — only the
  founding ever knew the bump): **the realm fix is a CoreState widening**,
  i.e. the formal layer.
- **THE PLAN, CONSOLIDATED (supersedes doc §1's 122-site wire carries)**:
  CUCUT §3 and BUMPREC independently point at the same lever, so CoreState
  stores BOTH the market bump (kills all three §1 searches, no wire change)
  and the realm bumps (−18,000 constant). Two lanes, chartered ~14:40:
  **CORESTATE** — Lean spec + proofs + regeneration + zero-means-search
  backcompat + founding writes the bumps; reader conversions (the CU
  harvest) held as phase B until CARRY merges. Announced on the board per
  BUMPREC's open authorization; cut is held anyway.
  **CARRY** — doc §2 caller-authority suffix (outside the hashed request
  prefix) + the digest-width pin test + §4 own-account bumps; worktree,
  Claims before Custody. Owns `hot_v3.rs`.
  Landing both collapses the band ⇒ worst ≈ low-1.34M, **bar met**; the
  gate ratchet turns on only then (a measured worst is a sample, not a
  bound, while varying searches remain). Then TRADE-2 cuts cohort-7 →
  market19 → activate → admit → **the first trade with whatever keys the
  participants actually have.** At the cut: measure with the
  `direct_hot_top_level` margin gate, NOT `tools/gauntlet/hot-cu` — HEAPRED
  proved that tier drives the continuation route, +35,127 CU high.

## THE REBUILD LOTTERY — ROUTED TO TRADE-2 ~17:05, CUT IS HELD

`release_set_id` is a hash of the deployed ELF digests, and it seeds the
activation cache directly and the Market identity transitively — which seeds
the Claims market, positions, maker replays and every caller authority below
them. **A rebuild redraws every bump on the route with no source change.**
CUCUT measured a cache bump moving 254→255 from a build whose only difference
was caller-side: 7,500 CU across five searches, band 36,001→42,000. This is
almost certainly what an earlier lane logged as "codegen noise of ±20,000 CU
between builds."

**Consequence: cohort-7's ELFs are a fresh die roll on CU.** TRADE-2 must
MEASURE the actual cohort ELFs after building them and before relying on the
route, rather than assuming main's numbers carry. If the draw is bad, rebuild
is a legitimate remedy — but only if someone knows to look.

**Status: superseded by the corrected protocol (TRADE-2, `227387da`).**
The lottery claim is reconciled — same source reproduces draws exactly; only
a source change redraws (CUCUT's "no source change" build had a caller-side
source difference; the measurements never disagreed). Cut protocol, locked:
build cohort ELFs → measure THOSE at 32 seeds with the floor gate → report
floor + tail probability (never a worst-seed sample) → found market19
**ZERO-FEE** (a fee-bearing trade is ~1.49–1.52M, over the ceiling; the
founding parameter is irreversible) → activate → admit → no trade until ORCH
confirms. Bad draw remedy: trivial source change + re-measure, never an
identical rebuild.

## LIVE LANES (resume with SendMessage to the agent id)

| lane | agent id | doing |
|---|---|---|
| TRADE-2 | `a7c1ba28ecbf894d9` | DONE with the caller sweep (5 commits; wire is **1,167 bytes measured three ways**; caught a real seal-projection bug `cargo check` couldn't see — grant shifted Trading from index 2 to 3, projection was aliasing the Ed25519 instruction). Lottery understood: will measure cohort ELFs at 32 seeds post-build. 13:54 ratchet handoff withdrawn. **Cut is HELD** pending carry wave + worst ≤ ~1,353,000. **Owns**: the cohort-7 cut, all devnet writes, `tools/release`, the public-cut fixture, `OPEN_LABEL`, and the ONE authorized whole-tree refusals regeneration (at the cut, on a quiet tree, announced first). |
| CUCUT | `ada700a9591280bf4` | DONE — design doc landed, carries deferred to the carry wave |
| BUMPREC | `a2bb9fa1946bb506f` | the 18 constant record searches |
| census | `a465c2a63f6f1d864` | DONE, read-only — verified BUMPREC pair-for-pair (14 searches, 6 pairs, 28,500 CU, realm ×2 confirmed independently). Full route total: **40 searches/tx = 21 constant + 19 key-varying**. Seven constant searches unclaimed: activation cache ×6 at one address (±9,000/bump-step — the lottery's biggest term; own-account-bump hypothesis routed to CARRY) and the capability seal ×1, whose carrier is NOT full (4 reserved bytes at offset 20; Trading already computes and discards the bump — routed to CARRY as its cheapest commit). |
| CORESTATE | `a1151a64b6bfa9895` | DONE phase A — `e93fe5e9`: STATE_BYTES 360→368, bump tail appended (market @360, realm raw/staging @361/362, 5 reserved), append pinned by a `native_decide` theorem + named test; zero ⇄ None ⇄ search, `Some(0)` refuses; founding writes all three (realm bumps free on ordinary Found, **+~12,000 CU arithmetic (unmeasured) on projected stage 2** — first program-test at the new base is the first real measurement). All byte-identity + emission gates green; no SBF build ran. **Cohort isolation is FALSE for reading**: widened programs still own old 360-byte markets (refused by length — acceptable on devnet per ember's Q1 ruling). SDK discovery reclassified by (magic, version, width); relayer `keeper.rs` and journey gauntlet cross-cohort readers NOT fixed (queued). Seven child PDA domains seed on `release_set` incl. the Custody vault → re-founding strands old collateral (assurance-phase item; lineage doc §6's field appends after the tail). **Phase B handed to CARRY.** |
| CARRY | `a59254695c09d8c61` | DONE — 9 commits to `ff543148`: 20 search sites at 13 addresses carried (cache ×6, Market ×3, realm ×4, caller-auth ×3, Claims agg/Positions ×3, seal ×1), incl. CORESTATE phase B. Route now **32/32 under the ceiling**: worst 1,390,745, mean 1,355,639 — but remaining key-varying band ~52,500 and the gate (1,387,000) is red by 3,745. **Correction adopted: no positive depth-invariant floor exists** (create_program_address itself costs 1,500; depth-1 carry saves zero) — CUCUT's gate argument was wrong-but-lucky. Digest-width pins landed per child. Migration class: Custody replay (288 exactly packed, Lean-emitted). Findings routed: **phase A introduced 7 SBF stack-frame-overwrite diagnostics** in `direct_replay_setup_v1::invoke_replay_child_v1` (0→7, may cause UB, nothing gates role links) → back to CORESTATE, **blocks the cut**; founding CU still unmeasured. |
| CORESTATE-2 | `a1151a64b6bfa9895` | DONE — diagnostics 7→0 (`557df0d1` frame split, offsets untouched: `invoke_replay_child_v1` was at EXACTLY 4,096; by-ref alone did nothing — the coexistence of CoreState + its outgoing copy was the wall; also relieved `authenticate_and_emit_replay_v1`, which was next with NO diagnostic yet). Gate landed (`ee3dbe8f`): ci/run.sh had sent role-link output to /dev/null, gauntlet only warned — both now REFUSE; proven red on real phase-A commit, green at fix. Third defect (`e164feda`): `git archive` commit-time stamps + reused `--work` dirs ⇒ cargo silently diagnoses the PREVIOUS commit's artifact — **any gauntlet ELF digest/CU figure from a work dir reused across commits must be re-derived**; fixed with `tar -xm` + recompiled/reused annotations. Queued near-wall (pre-existing): `outer::process_close` 3,968; four functions at 3,904 — `tools/sbf-frame-sizes.py` is the detector-with-distance. |
| CORESTATE-3 | `a1151a64b6bfa9895` | DONE — core red fixed (`67e96e5b`), NOT phase-A fallout: `2dc53776` (Aug 26) moved Core `OpenMarket` behind the Registry continuation with zero tests updated — the test submitted a one-account-short pre-continuation frame for FOUR DAYS, refused on length before any state read. **All five submissions incl. four hostile `is_err()` assertions were "passing" on that refusal** — adversarial tests of nothing. Fix: honest path through the shipped operator builder (259,870 / 277,025 CU measured); hostiles substitute ACCOUNTS (data is covered by the admission address) and name exact refusal codes — which immediately caught the fix's own wrong prediction. First-ever exercise of the 2-day-old rent-refund conjunct. → UNRUN spawned for the two remaining orphan targets. |
| CORESTATE-4 | `a1151a64b6bfa9895` | DONE — pins regenerated NOW (`cd1331be`+`a6ae34e4`), not cut-deferred: fan-out measured in every value representation (5 sites total); the "devnet-coupled, defer" reading INVERTED — the artifact digest is computed at runtime so the chain already carries `85fe…` and the shipped SDK refuses it today; regenerating re-synchronizes, and pre-widening markets were already excluded on width. direct-codec 174/174, fixture 16/16, zero frame diagnostics. Gate-gap answer: a gate EXISTED and went red — misnamed (basis polymorphism), uninformative (bare 32-byte diff), and one filter-string from being run; new row named for the dependency, red-then-green, pasteable failure output. **Class finding for the emission guard: hand-pinned digests OF emitted constants are structurally invisible to emitted-file-vs-emitter gates — five more of this shape** in `ordinary_bundle_v4.rs`/`ordinary_artifacts_v3.rs` (queued). genref reports 9 stale reference files: 2 fixed (theirs), **7 unowned** (queued — candidate for the cut's quiet-tree regeneration alongside refusals, ember/TRADE-2 to bless). |
| UNRUN | `af40d85274fcffcc0` | DONE — `d1d1ff3f`. Both orphans GREEN at HEAD on real ELFs (no third hidden defect) — but **15 hostile assertions named no refusal code** (the `67e96e5b` disease), now fixed with measured codes, three surprising: PairSubstitution survives the frame check and refuses `0x3004 Release`; all four dependency-ledger faults are `0x3008 Funding` at four distinct depths; a partial Core replay refuses in the PRESTATE. Frame literals replaced by shipped-operator-derived layouts (assembled by ordinal, permutation-checked). Runner now GLOBS `tests/*.rs` — a sixth target runs the day it lands; suites tier proven red (plant in the LAST-globbed target, proving the glob reaches its own end) then green. Queued: suites tier doesn't refuse on SBF frame diagnostics (inherited, named). |
| WALL22 | `a853da063a7ba4258` | DONE — wall down for General (`2f21911e`/`c2cfa4db`/`9012499c`, U-003(b) retired): the V1 demand is LOAD-BEARING (keeps the activation outer family-neutral); the wall was a missing per-family artifact. Family-neutral `dclutch-capability-activation-codec` with the brick gate IN THE CONSTRUCTOR (runs the real effect kernel; returns `ProjectedTailMismatch`, never a brickable bundle). Direct's sealed bytes proven unmoved byte-for-byte; General's shippable triple byte-identical to the fixture that activated on the real ELF. Rational/Structured are NOT at this wall — they have no root tail at all (→ pending ABI decision). Found: `dclutch-direct-codec` red 172/1 — `e93fe5e9` never regenerated `DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3` (→ CORESTATE-4, with the gate-gap question); `lane.sh fmt --allow-root` follows mod declarations into unowned files (heed its refusal). Queued: General publication closure (3 record labels + founding path) — the only remaining U-003(b) line. |
| SEALWIDE | `aa12c219b7835f001` | DONE — `docs/design/TRUST_RATCHET_V1.md` (`028f6047`). Honest total ~60,000 CU/4.4% with zero new accounts; sorting rule: **a carrier earns an account only when it carries a VERDICT** (addresses ride free on justified accounts; ALLKEYS already gives derivations the 1,500 CU remedy). R-1 = 0017 option B (52,592 measured). Bonus: the route decodes one immutable 1,288-byte activation cache THREE times/tx (75 `decode_role` calls, one answer). S-3 staleness case documented (sound today; tripwire wanted). |
| BASIS-ENUM | `a20af9a2ab0cef311` | DONE — `BasisKindV3::SplineDegree2To3 { degree }` on main (`f8701b6b`), refused on every route, fail-closed. Design's 13/10/3 break-set verified EXACTLY (the earlier 112-site count was off an order of magnitude in the deterring direction). 11 refusal tests red-then-green by single mutations; layered cascade proven (`PriceGateCertificateRequired` vs `SplineEvaluatorAbsent` one field apart); Lean theorems make the degree comparison branchless. Evaluator seam: `SPLINE_EVALUATOR_RELEASED_V3` const — no build can admit the kind without a linked evaluator. Corrected the design doc (`generated_product_v3.rs` WAS guarded — emission census caught the redundant script). New code `0x500C`; also fixed FRACR3's queued stale band assertion (0x500A→0x500B). Wire unchanged; schema-id bump deferred-not-skipped, forced visible by `a_record_whose_kind_byte_is_three_is_refused`. Named gaps: hostile-20 cross-invocation differential (needs program-test harness); 8/10 programs take new ELF digests at the cut. **Add-on (`96fe6c3a`) inverted the handed diagnosis: the TS mirrors were never stale — the VERIFIER was broken** (generators regex-scrape `runtime_v3.rs`; the constants moved into the emitted file; one source path, four files). Deeper: the four generators are hand-maintained byte-identical duplicates across apps/+packages/, **guarded by nothing while their outputs are guarded** — assurance inversion one level down; gate named-not-built (16/17 basenames identical, 1 legitimately divergent — a naive gate arrives red; post-wave item). `routeCensus.ts` regen exposed 4 unrendered instruction magics (registry-lineage + claim-check) → exemption entries naming owning lanes; render with their layouts next wave. Remaining web red: `sbomVerify` only (pre-existing manifest drift). |
| VARIANCE | `aef9a5b6c65574516` | DONE — `b61ffdad`. Census proven complete by fit (CU = C0 + 1500·T to 142 CU over 32 draws): 10 surviving key-varying sites over 8 addresses. **Gate re-founded: no constant bounds a stranger's key** (depth is unbounded geometric); gate now asserts the key-independent floor `TOP_LEVEL_KEY_INDEPENDENT_CU_V1 = 1,324,742` (cross-build stable to 1 CU); fit statement is probabilistic: P(stranger's key > ceiling) ≈ 0.032% (1/3,100). **FALSIFIED: "a rebuild redraws the lottery"** — same source reproduces all 32 seeds exactly; only a SOURCE change redraws (1 CU of code moved worst 1,390,745→1,363,745, band 52,500→24,000). **NEW WALL: a fee-bearing trade does not fit** (~1.49–1.52M arithmetic; the gate only ever measured fee-free — gross 5 × 50bps floors to 0) → market19 must be fee-free for trade 1; ADR 0014 D3 rate-diversity blocked behind measurement or the two-tx lifecycle. **Mass anatomy** (ember's question): child CPIs 21% · projections ~23% · commit 9% · auth 8% · reauth 3.9%; levers: two-tx lifecycle 36.1%, AOT 33.9% (NEVER measured on this route — biggest unknown), seal/cache 11.6%; searches are 1–2% of mass, 100% of variance. Found the CoreState carry INERT in the gate fixture (staged UNRECORDED) and realm ×2 overstated (one Custody CPI, 9,000 not 18,000) → FIXBUMPS. |
| FIXBUMPS | `a736bf7807a54f4ad` | DONE — `30574297`, byte-identical ELFs both sides (not one depth moved). Carry engaged: floor 1,318,826 (constant → 1,320,326), worst 1,345,829, band 16,501, survivors 10→7, budget verdict **UNDER**, tail **1 in 1.10 billion** (p=½; 1 in 13.9M at p̂=0.446). Saving decomposed exactly (9,000 realm + 4,500 market-attempts-to-C0 − 84) so nobody banks the reclassified part as won margin. Wrong-bump control is a permanent 4-arm test; the fixture re-derives staged bumps per seed so inertness can't recur. VARIANCE's evidence doc amended with the superseding numbers (ORCH). Queued: fee-bearing two-Custody shape still unexecuted (own lane); custody refusal checked by value not name (Cargo.toml dep). |
| SIMVIZ | `a6590d67752a5cccb` | DONE — `/pulse` now leads with what actually moves (slot rate 6.03/s measured, 1,440 conservation checks by name via the new LawBand; series schema v2, 294KB→56KB markup). Verdict: across 432 census observations exactly ONE field moves (the slot) — census-only mode is honest but still; real drama needs the activity campaign. **Journey tier hadn't built on main for ~2 days** (nothing runs it in CI): 15/16 errors fixed (`e41d0b20`); the last is `activation_receipt` missing from `ResolutionVerifyFundReadySnapshotV3` → routed to RELAY-3 (~1h, same seam as its stall). Devnet activity-mode SPEC in `docs/evidence/SIMULATOR_SERIES_VIZ_2026_08_30.md` (gap named: no spend-based kill; wants `budget.max_lamports_spent`). Campaign itself NOT run — waits on RELAY-3's fix. |
| PUBLISH | `a9690bd866d935926` | DONE — **clutch.dregg.pro LIVE at live-main `bb6d4edb`** (run 33333681231 green; /pulse serves the LawBand + heartbeat, assets 200, no unstyled-HTML regression; STORY-2's pages were already live from a 17:07Z run). **Cut procedure corrected**: publication line is dragons-clutch `main` (deployment-branch policy names exactly `main`); the cut is a single-parent content-sync commit (`62ef89808`), tree-hash-verified — never a history merge; agent branch is a divergent codex line (865/114), pushed separately (delivers the 2 CI workflow commits). Codex's six direct-to-host commits verified present-or-superseded upstream; only genuine drop: 2 explorer deep-links in `RedeemFlow.tsx` (queued). Credential check: 0 occurrences incl. UUID-enumeration (9 tokens, 0 matches). Queued: 9 live commits after `bb6d4edb` for the next cut; baked `simulator-status.json` goes stale between cuts (page's stalled-run treatment shows honestly — live-feed question belongs to SIMVIZ's devnet-mode spec). |
| WALL22 | `a853da063a7ba4258` | the family-wide activation blocker: V1-schema descriptor demanded at the `outer.rs` check while every family stamps V4. Diff the one successful activation's artifacts vs a failing family; fix + reviewed-template evidence; ZERO on-chain activations (devnet stays TRADE-2's). |
| BASIS-ENUM | `a20af9a2ab0cef311` | the enum half of degree 2–3 curvature per BASIS_ABI_UNIFICATION_V1 §6: third `BasisKindV3` variant, fail-closed through 13 match sites, admission demands the DCLTPGT1 price-gate certificate; evaluator explicitly out of scope (refusal until it exists is correct). |
| FRACR3 | `a71bfa348bcad307b` | DONE — **security weld #5 of the day** (`34ec1148`): this morning's compaction ADMITTED `TradingRecord` owners, but that tag is the Fractional reserve Position — a PDA that can never sign `RedeemClaimCheck` (0x5621). Past the deadline anyone could compact the reserve: collateral to vault, claim-check naming an unsignable PDA, position closed — **total loss for every shard holder**. Welded with an exhaustive owner-kind test (a 4th enum kind is a compile error, not a silent `true`); 17/17 campaign green. Not exploitable today (180-day deadline) but would have shipped in the next claims cohort. Fractional claim-check itself: sized (2nd record type seeded by shard mint; ~928k CU arithmetic — compute is NOT the blocker), one lane / 8 commits AFTER the upstream gap: `fractional_retirement_v3` dispatches only `RetireCoordinate` — **a fractional market cannot retire at all** → FRACLIFE. Incidentals queued: `ClaimsSbfError` ceiling pin stale by one variant (0x500A vs 0x500B); `FRACTIONAL_ROOT_PDA_SEED_V1` dead. |
| FRACLIFE | `af435b2c561625f31` | DONE — **a fractional market retires end to end on chain** (`9a3b00b8`..`b17e9bc3`, 4 real txs, 6/6+14/14+45/45, 0 frame diagnostics). Three defects: the known refusal; the walk could never take a SECOND step (root revision double-pinned — fixture had papered it with a planted revision); and the only retirement test red-for-the-wrong-reason since `4630ad77`, which is why #2 was invisible. Two griefing surfaces closed: one stranger lamport could freeze retirement PERMANENTLY (equality → floor); Finish burns the whole balance. All three acts permissionless (a signer requirement would strand shard holders). Not verified: width-1 only on chain; resolution via set_account; owner kind still planted (FRACR3 gap stands). Queued: exhaustive ceiling-pin list; dead seed const. **Seam follow-up (`80b78181`): gate fully GREEN on main.** One finding was a real defect: a transaction-level readonly pin made Begin+Finish of one walk UNBATCHABLE (runtime merges privileges across instructions) — replaced with directional `FrameRoleV3`, proven red on the retained pre-fix ELF, and 5-6k CU cheaper per act. New `LiabilityBasisMarketSeedsV2` constructor is the migration target for ~24 raw-spelled sites tree-wide (baseline debt, queued). |
| PORTFOLIO-X | `a74fb4844f4867c07` | DONE — `1fbdd5f6`/`d6329c64`/`7cd4dff2`/`9c7fe842`: /portfolio shows exact per-position payout bands from balance vectors ALONE (convex-combination bound — no product record, oracle or model) + bundle floor/ceiling with the honest cross-market line; same-terms narrowing shown as conditional (CommitDeadlineFailure can split a pair). No division anywhere; float-detection tests. Web 127 passed (+34 new, same pre-existing reds). Rides the next cut. **VERIFY BEFORE THAT CUT** (→ RELAY-3): the ceiling rests on one argued-not-read assumption — a failure refund vector obeys the payout-vector weight hypothesis. Queued: SDK's 99-file absorption backlog; stale basis TS mirrors (`BASIS_WIDTH_OFFSET_V3`/`BASIS_SCHEMA_V3` missing → folded into BASIS-ENUM). |
| CI-3 | `a8abf0f1f1f6b761a` | DONE — `acc5fc11` adds journey/suites/workspaces tiers (wrapper `803fcb565` rides the next push); each proven red at named revisions + exit-2-per-row on missing prereqs; journey went green live off RELAY-3's `dc4ad5d9`. Fifth never-run gate (`check-all-workspaces.py`) wired nightly. Found + routed: core test red at HEAD (`0x3001 AccountFrame` → CORESTATE, third engagement), 3 new `DOMAIN_RAW_RESTATEMENT` seam findings on main (→ ALLKEYS, owns the files). Queued: `capability_close_alias` + `retirement_replay_handoff` are run by NOTHING (the open-market runner drives 3 of 5 targets); 737 abandoned `/tmp/dclutch-*` scratch dirs — reap AFTER the wave drains, not while 9 lanes live. Its own correction, structural: a dirty tree HID a real red (retraction was the worse error) → `--commit` now reaches every compiling tier. |
| HEAPRED | `a41fbc198c5a2207c` | DONE — `8bf6ad40`, evidence in `docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md`. The heap test is red because the continuation route itself no longer fits (19/32 seeds fail; heap is innocent). The Registry outer buys NOTHING top-level lacks (same roles, same children; its one difference is a relaxation) and nothing outside the test harness can even construct it. Matched-pair control: +35,127 CU vs top-level, the same integer on all 13 comparable seeds — route plumbing, not a draw. **`tools/gauntlet/hot-cu` drives the continuation, so every "Hot CU" figure that tier ever printed is 35,127 high.** Also: `8ee544e4`'s "continuation unchanged" was false by 517 CU (heap declaration keys on forwarded instruction data). Changed zero non-comment lines. |
| CI-2 | `a8abf0f1f1f6b761a` | DONE — `tools/ci/run.sh` tiered runner (6d599ef8) + `.github/workflows/rust.yml` (2c4a0473, committed NOT pushed); five gates proven red-and-prerequisite-missing with distinct exit codes; `emission_guard.py` exit-code defect fixed. Its margin-gate red at `8d3ca1f9` (worst seed 8 CU under budget, next seed over) independently confirms the CU wall. Its bisect handoff to CUCUT is deliberately DROPPED: while the rebuild lottery is live, bisecting per-commit CU bisects a hash draw, not a regression — the fix is the carry wave. Queued sizes it named: 4 more program-test suites in CI (~afternoon, mechanical — resume CI-2 when a build lane frees); pre-commit hook left OFF (would override ember's global `core.hooksPath`) — ember's call. |
| MEMBRANE | `a5e9b10376d59fbf3` | DONE — Structured crossed the membrane end to end (compiler `DCSTPB01`, kind-pinning authenticator, seam module, founded market `HEanNZ1e…o2Xg` verified from chain, 491/491). Rational verified (SEL-SEAM had built it). General hot commit half NOT built: **wall #22 is family-wide** (activation demands V1-schema descriptor at `outer.rs` `authenticate…`, every family's ProgramSet stamps V4) — sound refusal, bricking risk. Findings: founding "flake" is ZFS (`/tank` kills it, ext4 clean); open-family fixture lifecycle policy parks its only plan at `action: u32::MAX` (dead plan that reads as a design — queued fix). Left a validator on hbox `127.0.0.1:29300` holding the founded market + verifier at `tools/local-validator/verify-selected-capability-binding.py`. |
| SERIESFIX | `abee54822c4a029c5` | DONE — `3f2663b2`, 8/8 green. The stale half was the caller-supplied register bank (5→7 scalars, 1→6 identities per `8f579821`), not the artifact bytes; no assertion changed; the bank is now sized from the exported count constants so the next widening is a compile error. Deliberately did NOT make `route_commitments` author the projected slots (single-author rule; fail-closed to `Artifact`). |
| STORY-2 | `ae1b54b8aaee446db` | DONE — graduation wall root-caused as TWO founding-path bugs, fixed at their owners: (1) the founding artifact used the manifest the input DECLARED while the chain uses the one the market PUBLISHED (digest-agreement check now refuses by name pre-submission); (2) the journal's geometry pin hardcoded 20 funding accounts where the real shape is 20-with/18-without a recovery policy. Proof: market `CCCxRUN7…SJ2vh` founded Open on a clean gate, 181 txs, six mutations finalized; validator still up on hbox. Story pages made truthful. **Doctrine: when identities disagree, check declared-vs-published first — hit nine times in one afternoon.** |
| RELAY-3 | `ac32bc521557db93b` | DONE — **the relayer public-submission proof is executed; the abandoned arc runs end to end on shipped code** (market `JCWoR8BP…6QNJ`, conserved, all 8 stages incl. daemon-submits + consume-and-terminalize). Two defects, not one: `vertical.rs` was a SECOND AUTHOR re-driving the three funding mutations `found_through_open` already drives (O-005 in the flesh; 316 lines deleted, `3670f4cc`) — invisible for 3 days because the first author never ran; and `DeliveryExpectation.relayer_key_set_id` got the record's ADDRESS where its content IDENTITY belongs, so the route had never once worked (`d38b01b9`; the 8-way && split into per-field refusals, `e5843285`). Add-ons: journey builds 16/16 (`dc4ad5d9`); **PORTFOLIO-X's ceiling assumption HOLDS** (`validate_partition` sits outside the settlement/failure match — cited at three layers; caveat affects both arms identically) → /portfolio may ship. Queued: journey has no ActivateFund step (~2h); `tier1/bindings.json` lacks 3 funding-suffix rows (~20min); failure walk unrun; composed-market translation re-validation question from the PORTFOLIO-X caveat (~1h). hbox validators up: 26900, **27100 (keep — holds the sealed record)**, 29300. |

## PENDING EMBER DECISIONS

Four ADRs written today, each with evidence, options and a recommendation, in
`docs/decisions/`:
- **0014 the fee rate** — three rulings: (D1) keep per-venue `fee_recipient`,
  take **no protocol cut** (the protocol has no income; market founders do —
  say it out loud); (D2) `MAX_FEE_BPS = 500`, no lower bound, which
  **overrides a deliberate prior decision** and says so; (D3) unpin the release
  const so the demo can show rate diversity.
- **0015 the four dead markets** — they are **untradeable, not unredeemable**.
  Rule C now (they are filed under "open", the one untrue thing on the site);
  hold A (leave them standing as witnesses); refuse D; keep B available.
- **0016** a checked release binds three identities, one author each.
- **0017** the reentrancy answer was never ratified; its enforcement is
  subtractive. **Now quantified (SEALWIDE):** its option B — replacing the
  two Registry reauthentication CPIs with a local activation-cache read — is
  worth a measured 52,592 CU, invariant across 32 keys and two builds.
  Ratifying 0017 and chartering option B is the single biggest remaining
  routine CU win.
- **From TRUST_RATCHET_V1 (SEALWIDE, `028f6047`):** honest verify-once
  headroom is ~60,000 CU / 4.4% (NOT the census's 11.6% — that double-counted
  0005's already-banked seal saving). Also: (a) P-006's seal-refund
  beneficiary question must be answered before ANY second seal class (a
  product-graph seal is per (product, basis, release) — no Market's funding
  may take its refund); (b) the shipped seal's finality rests on one unnamed
  load-bearing condition (`require_prefunded_vacant`, the S-3 case) — wants a
  named tripwire test before any record-reclamation route lands.
- **Per-family root-tail ABI (WALL22):** Rational and Structured have NO
  capability-root tail layout at all (`root_state_bytes` is a free caller
  parameter — a literal 64 / fixture 8s). Authoring each family's initial
  root tail is a permanent ABI decision, sized in
  `docs/evidence/CAPABILITY_ACTIVATION_TEMPLATE_2026_08_30.md`.

- **Continuation route fix-or-retire** — evidence is now complete
  (`docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md`, HEAPRED,
  four options with sizes). Recommendation on the table: rule top-level the
  production route, demote the continuation to harness-only, re-bar the heap
  test on the +35,127 delta (one lane-hour), don't charter the compute fix,
  hold full retirement until ~20 program-tests are ported off it.
  **Scope caution (CORESTATE-3):** the ruling covers the HOT trade
  continuation only — Core `OpenMarket` has REQUIRED the Registry
  continuation since `2dc53776` (its admission PDA must be signed by a
  Registry `invoke_signed`); the founding-path continuation is load-bearing
  and must not be swept by a "retire the Registry outer" ruling.

## OPERATIONAL RULES THAT COST REAL DAMAGE TODAY

- **`tools/lane.sh commit`** — the enforced `--only` rail, in the repo the
  whole time. `git add <files> && git commit` commits the WHOLE SHARED INDEX;
  it swept another lane's files twice and left `main` uncompilable once.
  `git commit -- <paths>` is the manual form but does **not** cover untracked
  files.
- **Multi-file breaking changes go in a worktree** until they compile. The
  shared tree is a build input for a dozen lanes.
- **Never run whole-tree generators** at this lane count — `tools/genref/generate.sh`
  swept eighteen lanes' refusal codes into one reference.
- **Cite by symbol; line numbers decay within the hour** (a citation went stale
  in 60 minutes when an unrelated commit drifted the region 60 lines).
- **32 seeds, never 12** — twelve understated a worst draw by 7,659 CU.
- **A gate that cannot fail is decoration** — prove it red before trusting green.
- **An impossibility is a refusal; a size is an estimate.** "Needs an ABI
  change" is a cost in the grammar of an impossibility. (Ember caught the
  orchestrator doing this too — see below.)
- **Disk**: the volume hit 100% twice and stopped every lane. Root cause was
  the simulator's O(N²) census (now bounded to a constant 3,716,160 B) plus
  ~373G of stale lane scratch. Clean up worktrees and target dirs.
- **One board**: `/private/tmp/dclutch-wave2-board.md`. `lane.sh board`
  defaulted to the wave-1 board until ~14:32 (fixed); twelve 2026-08-30
  entries (TRADE-2 ×8, CUCUT ×2, SERIESFIX ×2) were relocated to wave2 and
  the old board carries a closure pointer.
- **Timestamps**: stamp via `date '+%H:%M %Z'`, never from memory — the
  pre-compact orchestrator's "15:50–16:50 EDT" stamps were ~2h ahead of
  wall clock (file headers above inherit that drift).
- **THE CUT IS MANUAL AND OURS** (ember): the dragons-clutch `dclutch/`
  subtree and the Pages build do NOT update themselves — cut the subtree and
  dispatch `pages.yml` (workflow_dispatch) at each wave landing. The PUBLISH
  lane holds today's cut; future waves own their own.

## THE ORCHESTRATOR'S OWN ERROR, RECORDED

Ember caught me ruling that TRADE-2 should **select maker keys** landing in the
cheap half of the CU band so the first trade would succeed, and label them
"selected for CU" — rigging the demo and labelling the rig, one hour after
telling another lane that a size is not a refusal. Reversed. The standing test
is ember's: **does it make the DEMO work, or the PRODUCT work?** A stranger
draws their keys once and does not get to draw again.

## COMPLETED TODAY (short form; `GOAL.md` has the full done-log)

Claim-check compaction shipped whole (14 commits, one ELF) — a terminal market
now retires past a sleeping holder who is still paid, to the atom, what
redeeming on time would have paid; redeem costs 13,399 CU on 7 market-free
accounts. R3 **narrowed, not closed** (native yes, fractional no). · Four
cohort-critical security welds, incl. a permissionless verb that let anyone end
every holder's redemption for one fee. · Dealer R4 closed by making the bad
state unrepresentable. · Seam-audit gate green and `--write` made unable to
read the working tree. · Simulator restored, storage bounded, death made
self-honest, a third Helius-key leak site found and fixed (verified zero in
both repos' history, the live site, and the work dir). · The site got names,
questions, clocks, odds, share cards, sparklines, live updates, and the compost
poster. · Lineage migration design + commits 1–3 (4–7 held for after the cut).
· Basis-ABI unification ruling + its five wire-neutral commits.

## ALLKEYS — the ruling executed, landing in progress

10 → 0 key-varying searches; every key costs the same 1,336,742 CU (63,258
margin); **refusal tail exactly 0** on a real Market. Carrier: the V3
envelope's 8 already-reserved bytes at offset 120 — packet stays 1,167, no
pin moves. VARIANCE's "irreducible" and "migration" classes both dissolved
(a bump rides BESIDE its seeds; a bump can be RELAYED, not stored — Custody
replay stays 288, nothing orphaned). Corrections: the 1-in-3,100 tail was a
fixture artifact (real pre-lane Markets were 1-in-34.9M; now 0). **Status:
five commits on `lane/allkeys` at stale base — lane resumed to rebase onto
main, compose with FIXBUMPS' fixture, run the 32-seed gate once, land
ff-only, and implement the packet-ruled +35,127 heap-test delta re-bar.**
**LANDED `e7805d62` (ancestor of `0e6bb66e`)**: merged clean, zero
conflicts; hinted band 6,001 (−67%), exactly matching FIXBUMPS' independent
residual model; Market hint correctly INERT on real markets (record outranks
wire — ignored, not refused); heap gate re-barred on the delta per
DECISION_PACKET §4 (32/32 on exact rungs, 256 CU tolerance, floor 36,713
re-measured); seam gate clean; regressions zero (one continuation test
FIXED). Honest note kept: on the luckiest seed the hint arm costs +295 CU —
the spread is gone, which is what the ruling asked. → HINTS-TS spawned for
the browser mile (SDK/web twins mine + fill the block). Devnet driver hint
wiring compiles, never run — exercised at the cut.

## Packet rulings (ember, evening) + succession

E1 revenue: DEFERRED, build nothing (as-built stands). E2 dead markets:
**option B — retire both** → TRADE-3. E3 seal rent: leaning collector-keeps,
final deferred. E4 unpaid-fee receivable: accepted, no deadline. E5 lockout:
accepted CONDITIONAL on guaranteed unilateral self-cure (incl. vanished
recipient account) — a charter requirement for the fee lanes.
**E2 OUTCOME (TRADE-3): the dead markets are UNRETIREABLE** — the founding
key directory is gone (445,440-candidate derivation sweep: nothing; ember's
own machines/backups NOT searched), and the signature-free aggregate-empty
path needs the activated root these markets can never have. Write-off:
167,999,880 lamports rent + 1,000,000,000 atoms (spent at founding — lost
recoverability, not a new debit; market18 same owner, +94M lamports).
Registry deliberately NOT falsified; **0015 option C is UN-MOOTED** (web
bucket queued). **Cut-ordering fact: post-cut, deployed programs refuse all
three old markets on LENGTH — if the key surfaces in ember's backups, B must
run BEFORE the cut on a cohort-revision client.** Two cut-critical fixes
landed in-fence (`1dc10983`..`32096809`): the staging script's hardcoded 50
bps removed (zero-fee refused-above-band, no default) and the founder is now
a custody-obligated keypair, not a bare pubkey — the structural cause of
today's loss, gated red-then-green. Queued: 4 `tools/release/test-*.sh` run
by nothing.
**BUCKET DONE** (`e3600765`): "2 markets that can never trade" bucket +
never-trades chips, predicate from chain data matching `capability.rs`'s own
deadline comparison (never buries a slot early, pinned); landing pulse counts
only tradeable markets. Web 1020 passed. **CONSEQUENCE FOR ORDERING: a
main-built reader refuses ALL THREE deployed 360-byte markets — live devnet
discovery lists ZERO open markets** (the live site likely shows this NOW,
since the published export post-dates the widening). The fix IS the cohort-7
cut + the next Pages publish, in that order; `public-cut.devnet.json` still
headlines the dead `7Mcu1ZT9…` as "open" — TRADE-3 updates it at the cut (it
owns the fixture). Queued: MarketTradePanel "never can" wording (~1-2h); two
MORE pre-existing abi reds than briefs allowed (`abi:general-v5`,
`abi:route-census`) — need an owner.
**TRADE-2's transcript was ORPHANED by an account rotation** — TRADE-3
(`a79fb7a4236a52c96`) spawned from durable state per the resume protocol:
task 1 = E2 retirements (devnet writes); task 2 = the cut, armed, awaiting
ORCH go after GENPUB's founding-wall answer. FEEPROOF (`a91704868339999dc`)
— **(a): the fee leg EXECUTED in a later block** — 149,210 CU whole-tx (under
the 200k DEFAULT budget, no compute instruction), 16-account tx, §2.1 ledger
holding cell by cell; via a staged release set binding a 32KB test caller as
the Trading role (Custody only admits the bound role — no third program can
stand beside Trading). Two design corrections: §1.3's hostile orderings
refuse at the live SPL delegation check (0x6006) BEFORE the replay revision
— delegation is the first defence line; and `FeeContinuation` had NEVER
successfully returned anywhere in the tree until this probe. Landing +
§1.3 amendment in progress. Untouched: everything about Trading's side
(lane C), `fee_owed`, permissionlessness — the 5-lane build stands sized.
**HINTS-TS DONE** (`c5f5099c`/`f27ecc07`/`62f2a727`): browser trades mine and
carry 6/8 hint slots — same two the Rust builder leaves; byte-for-byte vector
PASSED first run through shared exported seed constructors; removed a real
bug (the TS evidence encoder REQUIRED zero hint bytes — a mined wire failed
its own authenticator); wire stays 1,167, no pin moved. The child_caller
slots need the projection port: **10,643 measured non-test Rust lines across
4 crates** (queued; miner exported for callers with a projection; those
draws are per-request, so no key is ever permanently stuck).

## WALL4 LANDED (`997c51d0`+`41502df3`) — the panel is ready for the trade

The seller model matches the chain: readiness = what `direct_token_setup_v1`
actually requires (Position covering the fill; the Direct token PDA vacant
OR initialized-base — the only two prestates the chain admits); the
admission demand DELETED with its backstop named per clause; one clause
STRENGTHENED (PDA re-derived from the route's own facts). Cross-language
control = PAIRFIX's `2xGo6Cxt…` byte-for-byte. Bonus catch: the participant
decoder refused the very account Trading creates (no delegate) — third
layer of the same disease, reconciled. Seed domain + role byte now
GENERATED from `token_setup_v1.rs` incl. pinning the seed ORDER. Caveat
carried in the binding: vacant-prestate acceptance moves that refusal to
the validator (setup is the producer's separate permissionless tx). Sweep
note: WALL4's mid-flight files rode GRICE's `ce410b1d` and ORCH's
`1fec26b8` — content correct, attribution muddied (moving-tree hazard).
**Remaining before ember's click: TRADE-4's manifest + PUBLISH-3.**

## FILLWIDTH LANDED (`7fc47e73`) — the readiness wave is 6/6

Composed with PAIRFIX properly: adopted collect-all-clauses (an occupied
root wrong three ways names all three); ABSENCE stays exclusive, with the
reasoning in code ("reporting owner/executable/width beside an absence
would be reporting three facts about nothing"). 516/516 on main itself; 20
producer tests (PAIRFIX's 18 + 2) in one run; CACHEREAD/FEE-CORE overlap
checked by --name-only, not assumed. FINALIZATION's substrate staged and
re-verified live (validator 43080, two Open markets with produced sessions,
both refusing at the exact spot that lane opens); artifacts preserved by
request until FINALIZATION closes.

## FILLWIDTH — premise refuted, the missing driver built

**No width pinning exists** — the "width changed" clause compares two
compile-time constants; the closure follows width correctly. The real wall:
NO loopback Direct-activation driver ever existed (General had one, Direct
didn't) — every local Direct fill had always refused at every width, and
the snapshot layer rendered the missing root as a System-owned placeholder
instead of refusing. Fixed: `direct_capability_activation.rs`
cluster-parameterized; `local-private-validator-direct-capability-
activation-v1` exists; the producer's root check names ABSENCE first.
Proven: activation+admission+produce on default AND six-cell markets (the
default needing it equally = the control). Landing via rebase over
PAIRFIX's producer refactor (same file — semantic compose instructed).
Findings: market19 UNAFFECTED (fee pin is loopback-arm only); SIMLIFE's
fee-0 was avoidable; run.py stage-08 fix = the founding records its frozen
table (queued); checked release cuts in 312s on this laptop (a drafted
refusal withdrawn). **NEXT WAVE'S OPENER queued: the FINALIZATION lane** —
`prepare_direct_inline_hot_finalization_v3` collapses ten refusal sites
into one nameless variant (the PAIRFIX disease) and gates the
never-executed eighth stage; validator 43080 with two activated markets is
its staged substrate.

## CACHEREAD-2 — 43→0, and the checker-that-answers-no lesson

It was its own: folding two role reads into one made `reauthenticate_roles`
SINGLE-CALL-SITE, so LLVM inlined 576 bytes into a caller with 384 spare.
The first fix made it WORSE (48) — splitting one frame made the caller
cheaper to score and the inliner swallowed `authenticate_market`. Both
`inline(never)` → frames byte-identical to baseline; zero of 858 frames
grew; floor 1,252,764 (the fix cost 13 CU); all seven role links recompiled
to 0. **Root cause of the false "zero diagnostics" claim: its gate's grep
pattern was written FROM MEMORY and matched nothing the backend emits — "a
checker with a wrong pattern doesn't fail to answer, it answers no."**
Pattern now copied from run.sh with a comment at the paraphrase site. A
flipped continuation test correctly NOT claimed (lottery redraw, not a
fix). Merged `d1891162`.

## PAIRFIX ruled; TRADE-4 runs the session; WALL4 fixes the panel

PAIRFIX (`dd9a96a5`): **the producer was wrong** — its twelve-way distinctness
sweep had no site-specific reason against the buyer (neither on-chain
instruction carries the buyer at any index; the real hazards are lamport/
privilege merges, still guarded). payer==buyer now plans; every refusal
names ALL its clauses (the 26-clause boolean became a named-expectation
type). The silent clause convicted: the seller ticket named the PARTICIPANT
collateral shape where the producer requires the DIRECT TOKEN PDA
(`2xGo6Cxt…`) the trade itself creates. **Wall 4 (same species, panel-side):
`prepareDirectWalletTransactionV1` requires seller participant-readiness
the chain doesn't** — the trade creates the seller's account; founder only
needs its Position → WALL4 (`a70665bfa3299b86c`), SDK-only ~900 lines,
chain-preconditions-win rule. **TRADE-3's transcript ORPHANED (rotation) —
TRADE-4 (`adfc58bb7c5f10d24`)** spawned from the job dir: re-author seller
ticket with the PDA collateral, rebuild ≥dd9a96a5, produce with payer=p3
through session/lookup-freeze/manifest, STOP before `hot`, update the kit.
GRICE reopened for the strike-four DELETION pass (honesty silent on the
page); README + SECURITY.md landed on origin/main (`fe7350443`,
`6d3706d0a`).

## THE SITE IS FULLY LIVE — PUBLISH-2 (`55b42907` live-main → host `657eb4504`)

Headless-browser verified: market19 under "Markets you can trade" ("Open now
1", finalized floor 490747385); the 360-byte stranding rendered honestly
("not listed as current"); GRICE copy + /population + /portfolio + honest
buckets all serving. Fixture's founding signature READ OFF CHAIN — HELD_STATE's
`4JWstD1A…` is the activation, the founding is `4AWsB181…` (mislabel
avoided). The false-claim link-check pins were DECORATION (footer satisfied
them with the body deleted) — now body-only, proved red. Genref: exactly 7
files, decisions through the packet, refusals 212→257. Credential sweep to
the zero-64-byte-arrays level: CLEAN. market18's "this one can [trade]"
title corrected (the widening falsified it). Queued: `abi:general-v5`
scraper (`hot.ENVELOPE_RESERVED_OFFSET`, blocks the convergence batch —
correctly not quick-fixed in a cut); 4 stale program-test Cargo.locks;
CORRECTION: docs/reference/decisions.md does NOT ship publicly (only
refusals + abi/ do). **NEXT CUT HAZARD: host LOCAL main diverged from
origin/main — build cuts on origin/main** (routed to README).

## THE PRE-PASS — two walls dissolved, the real one adjudicating

TRADE-3: Wall 1 was never there (`token-setup` creates the seller's Direct
collateral PERMISSIONLESSLY — the founder only needs its Position; both of
ORCH's proposed fixes would have failed, reasons recorded). Kit ticket
verified from first principles (derivation reproduces p3's real account as
control) — no re-authoring. Delivered: buyer ticket `15b0f867…`, p3 funded
to 263,541,120 lamports, driver rebuilt with the ticket author, six digests
re-verified. **Wall 3 is the real one and structural: producer
`require_distinct_v1` refuses payer==buyer; the panel requires
payer==connected wallet — both cannot hold** → PAIRFIX
(`a5fd790e0773c10d8`) adjudicating with the code's own reasons + convicting
the silent `public.seller/buyer` clause + making the producer name every
refusing clause. TRADE-3 re-runs the session on PAIRFIX's DONE. Everything
durable in the job dir (`PREPASS_FINDINGS.md`).

## FEE-CORE — the fee's protocol tier is real (`a0b1f4cb` merge)

**[CORRECTED 2026-08-31, LEDGER-TRUE: everything in this section is
BRANCH-ONLY — `a0b1f4cb` never reached main; it lives on lane/fee-core →
lane/fee-tx2, which FEE-TX2 lands as one composed stack. At HEAD today a
9,999 bps fee is still admitted; the band exists only on the branch.]**
Band enforced PROTOCOL-side (`DIRECT_MAX_FEE_BASIS_POINTS_V1 = 500` at
config construction + as a transition relation — 0014 D2's "enforced
nowhere" corrected). Replay 152→160 via Lean emission, both-width reads,
`fee_owed` recorded by settlement and gating the debtor's next nonce
(`FeeOwedOutstanding`). tx1 routes the seller leg alone with UNCHANGED
Custody request bytes (terminality follows the fee). **FeeSole retired with
a no-sorry Lean proof** (a banded fee always leaves the seller something).
E5's self-cure proven five ways incl. vanished recipient (fee destination
bound by OWNER, never address). tx2 seam pinned by state. 575+ tests, lake
build, census green. **Ruling recorded (ORCH, per the Q1 precedent): the
replay widening IS an account-tier migration** — the both-width read keeps
exterior readers honest but the AccountProfile pins canonical width, so
cohort-8 strands cohort-7's live 152-byte replays on devnet (acceptable per
ember's standing devnet ruling; a REAL migration story is a pre-mainnet
requirement — queued to the assurance turn). Queued: FeeSole's physical
frame retirement (1 day, mechanical, zero admission gained);
`abi:general-v5` red (`hot.ENVELOPE_RESERVED_OFFSET` — ALLKEYS' envelope
area, needs a small owner). **Fresh red routed: 43 frame diagnostics in
`direct_begin_retiring_v1` at exactly 4096** → CACHEREAD reopened (its
conversion, the `557df0d1` split precedent).

## DIST — dClutch's first release is live

`v0.1.0-devnet.2` on github.com/emberian/dragons-clutch (green runs, both
prerelease). Install: the versioned one-liner (GitHub's `latest` 404s on
prereleases — measured, and the devnet.1→.2 respin existed because the
README ships INSIDE the archives). Binary `dclutch`: market/capability
show+decode, single-authored (calls the real decoders, knows no offset),
read-only, no keys; `dclutch ticket` is a NAMED SEAM refusing until
TICKETCLI's author becomes callable (it's pub(crate) in a lib-less crate —
small follow-up queued). dist-workspace at HOST root pointing into the
subtree (cost documented in-file). AGPL G-19: by-copy covered; §13
network-interaction NOT (site conveys no source link from the running page
— queued). Verified: the installed-from-release binary read market19 back
Open/gen-2/bump-tail 252-254-253 — independently reproducing TRADE-3's
founding-side report. Windows refused honestly (ring/asm unverified).
Hazard routed to PUBLISH-2: its pre-23:28 pin would revert the CLI crate —
re-pin or verify tree hash `f8c48c1a`.

## CACHEREAD — 0017-B landed at −66,921 CU (`1da601e7`)

Floor 1,319,672 → **1,252,751** — beat the 52,592 estimate because the ratchet
doc under-credited the third full cache decode (correction committed). The
whole Registry conjunction reproduced locally with per-conjunct red tests;
one conjunct STRICTER (address seeded by the Market's release set); a
superseded slot now refuses by name (0x4007). Tripwires proven both ways
(dynamic ReentrancyNotAllowed + structural seven-adapter scan) — and the
honest negative: the dynamic half covers ONE Claims route; the 14-site
shared helper is uncovered (queued), Core/Dealer/Rent tripwires sized 4-8h
each behind "does that shape even run". **Fee-bearing single-tx bound fell
1,501,503 → 1,435,274 — now only 35,274 over the ceiling** (two-tx plan
stands; the number is recorded for the day the gap closes). Meta-finding:
`process_activation` had NEVER executed against an authenticated deployment
— its mock authenticated nothing; now stages real Loader V3, 11/11.
Queued: `direct_begin_retiring_v1` has no on-chain test anywhere. NOTE:
main's ELFs now lead deployed cohort-7 (all five digests moved) — devnet
stays consistent on its own set; improvements ride the next cohort.

## FRACCHECK — the false premise, executed; Claims half landing

Design §17.3's "holder burns with their own signature, forever" is FALSE on
chain: every shard mint REQUIRES Token-2022 PermissionedBurn, controlled by
the Trading root (proven by execution with a double-signed control, not by
reading). Sound resolution designed + proven: compaction re-points burn
authority to the Claims escrow WHILE the root lives (stranger OwnerMismatch,
old authority powerless, holder-signed escrow-approved burn accepted).
Economics: remainder goes NOWHERE — dust is consolidation-redeemable;
sweeping it pays a stranger from holders' collateral ("wrote a
residual_beneficiary field and deleted it"). `FractionalClaimCheckV1` 320B
`DCLTFCK1` + conservation plans + sub-bands + campaign landing on main;
§17.3 being amended in place. **Sizing corrected +6: 14 commits total — the
Trading half (Trading-composed compaction + split-controller read_mint) is
QUEUED as FRACCHECK-2** (overlaps live Trading lanes). ClaimsCapability
stranding now counted, can't quietly vanish.

## TICKETCLI — the author exists (`d956a488`+`9aab2aeb`)

`direct-intent-ticket-author-v1`: byte-identity proven the strong way
(deterministic Ed25519 — Rust and TS emit the SAME SIGNATURE, so the
message bytes agree); envelope authorship 3→2 pinned definitions; key-material
flags refused at parse (`--keypair-env` only). No local fill — correct
refusals: the devnet producer's genesis gate stays strong; NO loopback
Direct-activation command exists (sized ~200-400 lines + adversarial review,
overlaps FILLWIDTH). **Routed finding: the fill refusal's message lies — no
width term in the expression; live disjunct is a never-created execution
root; market19 NOT suspect.** → DEMOKIT (`afae21e3febb8d3bc`) assembling
ember's first-trade kit (authored+verified sell ticket, manifest, Talisman
steps, click list) into the mode-700 job dir.

## ROOTTAILS — both family ABIs designed (`ec530892`, doc-only)

`RationalCapabilityRootV1` + `StructuredCapabilityRootV1`: 16 bytes each,
distinct magics, one composed header word, ZERO seam fields (four refusal
classes structurally vacuous). The permanent rule: **a tail restates a
header fact only when a family decoder holds the tail without the header**
— General is the exception with a named cause, not the pattern. Correctly
did NOT hand-write the layout (P-007): the Lean emission needs a new lake
root + regen + census — queued as its own lane. **The window is open now
and free exactly once**: the activation entry widens each publication,
which moves every Rational/Structured Market address — both families live
only in the lab today; the widening rides the lane that also founds. Traps
recorded (StructuredRootV2 is a different account carrying exactly the
Fractional-impossible fields; its schema id wired to no descriptor). Needs
no ember ruling.

## SIMLIFE-2 — the population has hands

Run `hands-144`: **founded 4 of its own markets at 4 shapes** (2-cell
no-cuts through 6-cell/4-cuts at 5.07G atoms — the compiler's one-shape wall
fixed via `LocalMarketShapeV1`, defaults byte-identical), **admitted 12
participants** with real journaled Token-2022 collateral legs, 87 censuses /
510 conservation checks held. `/population` draws it (odds paths on a shared
clock + the landed/refused/censused honesty strip that never sums the
not-done words). Two graded-basis markets refused at their own draw site,
everything under them honestly `blocked`. 9 commits. **Next walls, named:**
fill REFUSED ×21 — "Direct root owner or width changed": the Direct
capability closure does not follow the widened width (IF ember's market19
browser trade refuses, this is suspect #1 — though market19 is
default-shape with a properly activated root); resolve now 4 steps further,
stops at the producer's second pass; compact driver sized 6-10h. Founding
retry lore: a partially consumed key set is unresumable — retry with a
whole new set (~1 in 3 foundings hits a finalization transient).

## MARKET19 IS OPEN — through admit; the trade is browser-only, structurally

`6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4` — Open, gen 2, 368B, zero-fee,
**bump tail fc/fe/fd: the first market ever founded carrying its own bumps**;
capability root `7kPABbyr…` activated (the 7Mcu1ZT9 wall, cleared); p1/p2/p3
admitted, p3 funded 1,000,000 atoms. Admission ALT solved: `--routing-table`
EXISTS but is undocumented; the frozen DCLTGMF3 table is the one (found from
the freeze tx). Learned: collateral must ride the FIRST admission
(`InvalidVacancy` on top-up). Net campaign spend 0.4254 SOL. Stager
redaction fixed (`ccb04db2`). **THE TRADE STOPS HERE STRUCTURALLY: the only
Direct-ticket AUTHOR in the repo is the browser trade panel**
(`encodeDirectIntentTicketV1` + wallet signature); every tool only consumes
tickets; TRADE-2's ad-hoc signer was the 4th scratch-reaping casualty.
TRADE-3 correctly refused to hand-roll an Ed25519 message layout. Seller =
founder `B6qxQCSwVe…` (holds all claims, SECRET HELD in the durable job
dir); buyer = p3 (funded, keys in job dir). Queued: a tools-side ticket
author (the missing CLI — ember asked "does dclutch have a cli" days ago);
document --routing-table; the admission gotcha into the guides.

## GRICE — the site speaks plainly (rides the next publication cut)

3 commits (`afda6ee8`/`4e23b38a`/`fcbc2bbe`), 23+ files, web 1061 at
baseline. Market card: 10 raw rows → 5 reader rows + a collapsed
"in the protocol's own words" drawer (nothing deleted). Landing, detail,
portfolio, campaign, trade/join panels, console, 404, SDK meaning strings,
genref front door — all in reader order with jargon glossed; every honesty
sentence kept, said plainly. Since GRICE is on main, the plain-language
site + market19 + the trade all ride ONE publication cut. Queued: the
marketDetail.ts web/SDK twins have NO drift gate (P-007 family); /explorer
is the last protocol-dialect body (129 prose fields — its own lane).

## CUT COMPLETE + PUBLISHED + ACTIVATED; market19 founding mid-ladder

Publication (6 bodies/18 txs) + activation all-five green: cache `GRDN2mbV…`
reads set `91dcbefd…` exact; the carried-forward Registry's zero-bump cache
takes the documented search fallback — carry-forward argument CONFIRMED ON
CHAIN. **market19 = `Bo7fbMxLE92tngoT5CwLTv9p4sXBkBPAVAeQ1mgA2Em6`** (368B,
the only widened market; gen-1 Founding; Open lands at gen 2, different
address; zero-fee verified in staged input). The founding is NON-atomic (37
accounts, staged records, 3 ALTs) — which vindicates the SUFFIX hold with
better reasoning than ORCH's ("right caveat, wrong premise"). Driver
crash-safe; resume command in HELD_STATE.md; TRADE-3 resumed through to the
trade. Spend: 0.8007 SOL extension rent + ~0.32 fees; ladder still spending.
**CREDENTIAL NOTE: the Helius key appeared twice in TRADE-3's transcript**
(echoed cargo cmdline + pgrep) — on-disk logs redacted, wrapper mode-700;
ROTATION RECOMMENDED to ember; stager rpcUrl-redaction defect being fixed
in-fence.

## COHORT-7 IS LIVE — all five roles deployed and verified

Set `d202e1f4…`; custody/resolution/claims/trading/core at slots
490,691,882–490,697,521, every live digest matched to its receipt, every
buffer byte-verified before adoption, buffers refunded. Spend exact:
868,709,840 lamports (0.8687 SOL; ember airdropped +10). Two-step ran 5
roles with zero finalization races. **The stranding has happened, as ruled
and disclosed**: all three old markets (market18 incl.) are 360-byte
accounts the new Core refuses on length — second independent stranding for
market18; nothing that previously worked stopped. Both armed receipts
retired-not-deleted. **Held then resumed at publication: the 08-29
SuccessorPlan was the THIRD casualty of the scratch reaping** — TRADE-3
authorized to regenerate into the durable job dir, verify plan digests ==
receipts == gate OFFLINE before any publication rent, then publish →
activate → found market19 (SUFFIX-fixed driver) → activate → admit (ALT
check) → HOLD for ember's browser trade. `HELD_STATE.md` in
/Users/ember/jobs/dclutch-cohort7-20260830 is the resumption map.

## SIMLIFE — the world exists; SIMLIFE-2 makes it move

Engine landed (12 commits + evidence doc): 6 archetypes × 6 personas,
Dirichlet stakes, seeded reproducible populations, the four-ending event
taxonomy (executed/refused/unattempted/blocked — "the engine decides what to
attempt; the census decides what is true"). One run executed: 79 censuses,
468 conservation checks held. Series v4 (wraps v3, pinned). Findings for
owners: only CategoricalQ1 is FOUNDABLE (`compile_linked_basis_v3`
hard-wires it — protocol-tier lane someday); the local founding compiler
emits ONE market shape (params queued to SIMLIFE-2); stranger compaction has
no driver anywhere. Free green control: foundings work at `53354005`,
confirming 0x5182's provenance. Validator LIVE on 34500. **SIMLIFE-2**
(`aeff802b415354a29`): wire the mutation drivers (SHIPPED code only — the
FOUND-5182 lesson), widen the compiler's shape params, one real intricate
run, first v4 chart.

## SUFFIX — fixed 6/6, and it SAVED market19

The post-Open funding wall was `12d0deb5`'s weld meeting a driver
(`pyth_market_input_base`) that unconditionally bought the recovery walk the
weld closed — every market that driver founds, LOCAL OR DEVNET, gets a fund
that can never be created. Worse: the suffix reported that as SUCCESS
(`ConsumedByFounding` from a tuple that is also the unfunded prestate) —
**on devnet market19 would have landed Open, unfundable, permanently
unresolvable, holder principal inside, and nothing would have noticed.**
Fix `4cb1727c` (host-side only, ELFs byte-identical — NO re-cut): the
no-recovery §12.7/§12.8 shape + the guard now READS the Source resolution
state (red-proved). Six-mutation walk 6/6 for the first time ever — the
first Resolution-Fund create/activate/accept this driver ever executed
(market `d1FSv3UA…`, 23 steps). ROUTED: TRADE-3 continues role upgrades,
HOLDS the founding until the merge + regenerated `market.json` (staged ones
carry nonempty `recovery_policy_hex`); admit stage must pass the
routing-table path (stage-08 `PacketTooLarge` wall, queued). SUFFIX landing
the branch now. Queued: Q2 relitigation (weld intent implemented, not
re-judged); post-Open vs pre-Open CreateFund CU gap (337k vs 1.20M).

## THE GO IS GIVEN — cut executing through admit

TRADE-3 prep verdict at pinned candidate `a93256c1`: floor 1,319,583 (gate
pass by 743), tail 1-in-614M (p=1/2), two independent builds byte-identical,
checked release green, `ReleaseSetSelectionMismatch` does NOT reproduce on
the cut path. Suffix wall does NOT gate devnet (atomic foundings return
`ConsumedByFounding` before the builders — 3/3 historically; holds while the
founding stays atomic). Stranding verified one step stronger: no migration
path CAN exist (`resize(0)` only) — market18 joins; site classifies by width
and does not break. GO issued: cut cohort-7 → market19 zero-fee → activate →
admit → verify per stage → HOLD before the first trade (reserved: ember may
fire it from the browser). BUCKET's (3,360) test LANDED (`91037b07`, 6
cases) — with a correction: the predicate was covered; the LISTING path was
not (width-only incompatibility had never run end-to-end; different refusal
string than the magic-level case). Load-bearing case pinned: an undecodable
account is NEVER filed as "can never trade" — that verdict only speaks from
an authenticated manifest. Announcement may cite it. Queued: the 757-CU floor-delta hypothesis (~2h).

## FOUND-5182 — VERDICT: regression, FIXED, first green founding

Convicted: the successor driver's HAND-WRITTEN COPY of the kernel CoreState
constructor still staged `StateBumpsV1::UNRECORDED` while Core writes real
bumps (254/254/253 read off the failing ledger) — the bumps flow through the
projected receipt digest into the permit's request digest: two byte-strings,
one comparison, 0x5182. The "independent" control was a decode/re-encode
round-trip that passes on anything decodable; it now predicts the tail from
the Market identity alone. Fixed `a7e2f668`, verified END TO END: **first
green founding in the tree** — market `4fModLZ8…` Open at slot 6809, ledger
preserved at /var/tmp/found5182-run. Devnet unaffected today (deployed Core
predates the widening); the cohort cut carries the fix via main. Eliminated
with evidence: per-role deployment auth (slot pins all match), releases,
authority. Queued: convert the driver to call the kernel's `found()` (3-5h —
the durable fix for the mirror class); no CU baseline for the 1,091,213
founding; General not re-run (Direct was the family-neutral control).
**NEXT WALL, now first: the post-Open funding-readiness suffix refuses
OFFLINE in its builders** (create=RecoveryWalkUnavailable, activate/accept=
Funding) at 3/6 mutations — → SUFFIX (`a6ac85cf160f4c026`), same suspected
species (driver duplicate drift). **TRADE-3: prep phase authorized** (build
cohort ELFs + 32-seed floor, zero devnet writes); go still gated on SUFFIX's
pipeline answer.

## Cut vs publication — TRADE-3's separation (`4ce17896`)

Two different events, different blockers. COHORT cut: blocked on FOUND-5182
+ ORCH go. PUBLICATION cut (subtree sync + pages.yml): ~9+ commits queued,
blocked on nothing — but the "no open market" strings live in generator
SOURCE (`generate.mjs:406`) + 2 link-check assertions + 4 hand-written
files, and the one authorized genref convergence run closes all 7 stale
reference files INCLUDING decisions.md — **source fix and regen must share
the window or the regen re-stamps the overclaim.** Presentation option
recorded, not taken: re-point the front-door headline at market18 (live
root, actually open) — needs no market19. ORCH conditional: if FOUND-5182
clears the cohort cut soon, one combined flow lands everything with
market19; if it's a days-class regression, do an interim publication cut
(market18 editorial headline + honest strings + genref window).

## SWEEP's ledger (docs/evidence/SLIPPED_THROUGH_SWEEP_2026_08_30.md)

18 amended / 22 open-with-owner / 17 listed (+6 dragons-clutch rows). Top
three: every "Hot CU" number in the tree was the demoted continuation's
(+35,127) — tier docs now say so; the reversed "selected for CU" key plan
still sat executable in a durable evidence doc (marked do-not-execute —
lesson: rulings live in durable records, SESSION_STATE is compact-mortal);
**`MAX_FEE_BPS = 500` is enforced by one bash line and zero programs** — the
protocol-tier band is a queued lane (0014 §6, one lane). Also queued:
VALIDATION_BACKLOG is 453 orphaned lines pointing the release CU gate at the
demoted tier; dragons-clutch README/SECURITY claim "nothing is live" /
"pre-implementation" (next host cut). Cut checklist grew: regenerate
docs/reference/decisions.md (stops at 0013) + the five "there is no open
market" strings after market19 opens. ADR statuses all reconciled; E3 is the
one genuinely open ruling, recorded on P-006.

## Wave 3 (launched after the queue-focus correction)

| lane | id | mission |
|---|---|---|
| FEEWALL | `af240ff2bd297848e` | DONE (landing branch `lane/feewall-20260830`, `391a65ff`) — **fee-bearing misses the ceiling BY ITS FLOOR**: all-first-try 1,506,527, over by 106,527 with zero luck; measured lower bound 1,520,795-1,532,027; the whole fee leg is 174,119. No tail exists. Also reframed history: the old fixture's trade was 2 orders of magnitude too small for its own fee rate to bite (supply 100 vs gross>=200). Zero-fee arm A/B'd byte-identical. **Recommendation (b) ADOPTED — fee leg as a second transaction** (routes already sequence through the Custody replay revision, not atomicity; ember pre-ruled multi-tx) → FEE2TX design lane. market19 zero-fee is now MEASURED, not inferred. **LANDED on main (`24b2b7f2`+`3d5dda0e`), numbers superseded upward**: all-first-try 1,515,003 (over by 115,003 before any key is drawn); fee leg 182,386 — reproduced TO THE CU across two ELF sets after the rebase caught its own meter-truncation artifact (+ its own seam restatement, fixed pre-landing). Routed: tail-zero cannot rescue fee-bearing (deleting all ten searches saves 15,000 of a 100k+ gap) — FEE2TX proceeds independent of ALLKEYS. |
| FEE2TX | `a0d3e71dfd2ce0427` | DONE — `docs/design/FEE_SECOND_TRANSACTION_V1.md` (`54d7e628`+`33a1467b`). **FEEWALL's premise refuted**: Custody authorities seed on the maker root, never the tx — the fee request is accepted in a later transaction TODAY. Disposition: `fee_owed` on the buyer's maker replay (152→160), nonzero blocks that maker's next fill in that market; tx2 permissionless, NO crank needed (nothing created or closed). Doctrine: no bookkeeping secures a debt whose only collateral is the debtor's own allowance (spl approve SETS, not adds). tx2 ~165-210k CU / 18-19 accts — the first Direct route where ALL KEYS holds by margin. 5 lanes / 3-4 days. **Cross-finding: 0014 D2's band retires the `FeeSole` route** (reachable only at exactly 10,000 bps). Residuals E4/E5 added to the packet. First implementation hour: EXECUTE two txs against FEEWALL's fixture (the §1 source argument is unexecuted). |
| CAMPAIGN | `aa35db1180f688ee2` | DONE with caveats — `/campaign` ships (5 commits: schema v3 + decoder, 5 figures, generator with a real u64-JSON.parse rounding fix), drawing RELAY-3's proven run 3, LABELED as such. **Its own four foundings all refused**: 3× `0x3003 Reference` at DCLTGMF3's Open leg at `53354005` (seed-dependence is the best explanation, unproven), 1× fresh checked release at main HEAD dying at `activation cache progress: ReleaseSetSelectionMismatch`. **CUT-RELEVANT: if a main-HEAD checked release can't clear activation-cache selection, the cohort-7 sequence fails at step 1** — warning routed to GENPUB (mid-founding now; its success or failure is the diagnosis). Also: journey-tier remaining-cost re-sized — relayed-vertical already implements prepare→boot→activate + drives ActivateFund, so RELAY-3's ~2h queued item is likely moot. |
| GENPUB | `aedcdae645d4cbd3b` | DONE — General's three activation records published + finalized at the seam's own addresses (`b09c4ee9`..`50f68bb5`; three labels not four — the transition is embedded in the descriptor; 8-entry SettlementWithActivation set, since seven is UNFOUNDABLE not smaller). **No root created: the founding refuses first.** Two walls, one fixed: **`ReleaseSetSelectionMismatch` was `a40ef689` writing the cache bump at byte 12 while the progress projection had no field for it — EVERY local founding since had been failing** (fixed `0e6bb66e`, red-then-green, five roles activated in execution). Revealed wall: `0x5182 ClaimsFoundingSbfErrorV5::Release` at the DCLTGMF3 Open leg, FAMILY-INDEPENDENT (Direct control refuses identically); cache + writability eliminated; 4 candidate sites in `founding_v5.rs`, 142k CU pointing at per-role deployment authentication → **FOUND-5182** (`a9556cdaf6e214021`), THE cut gate. GENPUB left the fresh release + two restartable ledgers on hbox. |
