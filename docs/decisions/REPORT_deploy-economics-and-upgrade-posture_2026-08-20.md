# Decision report: deploy economics (opt-z) + upgrade posture (F1 + F2)

Register entries: `opt-z-deploy-economics` (F1,
[`DECISION_REGISTER_2026-08-20.md`](DECISION_REGISTER_2026-08-20.md):752-773)
and `upgrade-posture` (F2, :775-796). One report because they interlock: the
upgrade posture sets the price of a wrong opt-level choice, and the opt-level
choice decides which bytes an upgrade authority would ever be asked to
replace.

Owner: **ember** (both; F2 becomes ember+counsel the moment real money is in
scope). This report decides nothing; it assembles the evidence, reports one
sanctioned new measurement, and recommends.

**Measurement disclosure (house rules honored):** exactly one new build and
one default-SVM-suite run were performed for this report — the opt-z ELF
built once into a scratch target
(`CARGO_PROFILE_RELEASE_OPT_LEVEL=z`, `SBF_TARGET_DIR` outside the tree) and
`programs/clutch-sbf/svm-tests/run_svm_tests.sh` (default profile) run once
against it, log teed to the session scratchpad
(`optz-default-suite.log`). No other suite was run; every other number below
is read from sealed artifacts or computed arithmetic.

---

## 0. The headline the register did not have

**The register's premise "Tier 0 made opt-z fully green" is stale. Measured
2026-08-20 on the current tree: the opt-z build is RED.**

The Tier-0 record (GOAL.md:249-252) was true when written: on the 08-19 tree
the opt-z suite went fully green at a 1,092,928-byte ELF (−23.4%), after the
ten then-overflowers were brought under the 4,096-byte frame line
(FRAME_BUDGET_PLAN_2026-08-19.md §4 Tier 0). Since then the T2-3/T2-5/T2-7/
T2-8 waves landed — exactly the growth the plan's watch item warned about
("~12 resident handlers sit at exactly 4,096 … the recorded 'one more
dispatcher arm' watch item", §3). The sanctioned re-measurement:

- **opt-z ELF, current tree: 1,474,584 bytes**, sha256 `58332d47d3a2011139…`
  (built in this worktree; per
  [`BUILD_PATH_IDENTITY_2026-08-20.md`](../reviews/BUILD_PATH_IDENTITY_2026-08-20.md)
  the digest is path-tied — the size is the portable number, the canonical
  digest would need a canonical-path build). Versus the sealed opt-3 default
  identity `e8ba31d5…` at 1,914,432 B: **−22.98%**, almost exactly the
  historical −23%.
- **Build diagnostics: 31 frame-overflow errors** ("overflows the maximum
  allowed frame space"), where the Tier-0-era build had zero. By crate:
  12 `clutch_batch_policy_identity`, 10 `clutch_batch` (including the
  known-impossible host monoliths `verify_inner` 37,056 B /
  `canonical_candidate` 44,288 B, which are diagnostics-only by design),
  5 `clutch_solana_reference` (host crate) — **and 4 in
  `clutch_solana_layout`, the crate the program decodes every account
  through**: `Intent::decode` (4,928 B frame, +832 over),
  `validate_artifact` (5,120, +1,024), `OrderPageAccount::decode` (9,088,
  +4,992), `OrderPageAccount::decode_on_grid` (8,448, +4,352). The program
  crate itself has zero overflowers — Tier 0's boxed-decode idiom held where
  it was applied; the regression re-entered through the layout crate the
  Tier-2 waves deepened.
- **Suite result: FAILED.** `artifact_transport` 6/6 PASS, then
  `blank_bank_lifecycle` **0/2** —
  `categorical_and_native_markets_construct_from_only_sealed_artifacts`
  dies with `InstructionError(1, ProgramFailedToComplete)` (a crash on the
  market-construction path, which runs exactly through `validate_artifact`
  and the layout decoders), and
  `a_late_token_target_refusal_rolls_every_earlier_creation_back` expected
  the `Custom(64)` refusal and got `ProgramFailedToComplete` instead — the
  same corrupted-stack signature class as the pre-Tier-0 failures
  (GOAL.md:446-449: "padding-canonicality refusals are the signature of a
  corrupted stack buffer"). Cargo's fail-fast stopped the run there; the
  same tests are green in the sealed opt-3 gates (blank-bank rows sealed
  PASS in evidence.json at ~195k-215k CU, nowhere near the compute
  ceiling — this is a fault, not exhaustion).
- **CU spot rows: honestly not obtainable.** The run died before any
  CU-printing suite executed, and CU measured on a build that faults on
  reachable paths would be meaningless anyway. The only CU-tax figures
  in-tree remain the Tier-0-era band: **+60–220% on some rows**
  (FRAME_BUDGET_PLAN §4), measured on the 08-19 tree, never on this one.

Consequence for F1: **option 2 (deploy opt-z) is not currently live.** It is
not "cheaper at a CU tax"; it is red. Re-greening requires a Tier-0-style
pass over the reachable subset of the 31 overflowers (the four layout-crate
functions first), then a full gate campaign at whatever identity results —
none of which exists today.

---

## 1. The two decisions, stated

- **F1 `opt-z-deploy-economics`:** when the devnet deployer is funded, deploy
  the devnet ELF(s) at the sealed default `opt-level=3` identity
  (`e8ba31d5…`, 1,914,432 B), or at an opt-z identity (~23% smaller, ~3.06
  SOL less rent per program, CU tax historically +60–220% on some rows, and
  as of today red), or opt-3 now with opt-z as a later comparison
  deployment. Included sub-question: whether devnet should carry the sealed
  identity for evidence continuity even at higher rent — and (this report
  adds) whether both program profiles belong on devnet at all.
- **F2 `upgrade-posture`:** does the reference deployment carry a
  time-bounded audited beta upgrade authority followed by irrevocable
  removal, or is it immutable at first deployment (P0 row,
  OPEN_QUESTIONS.md:28-32)? Source must support either without pretending
  one is the other. Sub-decision surfaced by the deploy script: the devnet
  deployment's posture is already de-facto written down and needs
  ratification.

---

## 2. Deploy-cost table at current sizes

Loader-v3 (`solana program deploy`, the deploy script's path) creates two
rent-exempt accounts per program: the 36-byte Program account and the
ProgramData account at **45 metadata bytes + max_len** (the 45-byte fixed
metadata region is pinned by the project's own decoder work — GOAL.md:639-651,
the `fable/r2-runtime-capabilities` record, including the finding that a
revoked authority leaves stale authority bytes in [13..45)). The script
passes `--max-len` = exact ELF size. Rent rate from the sealed profile
(evidence.json `rent`): 6,960 lamports per byte plus the 128-byte account
overhead, i.e. `(128 + data_len) × 6960` per account.

Sizes: default opt-3 `e8ba31d5…` 1,914,432 B and mock opt-3 `55ec393e…`
1,942,904 B are the sealed identities (MANIFEST.baseline.json:3154-3156);
default opt-z 1,474,584 B is this report's measurement; mock opt-z was **not
built** (one sanctioned build) and is estimated at the measured −22.98%.

| deployment | bytes | deploy rent (ProgramData + Program) |
|---|---:|---:|
| default, opt-3 (sealed `e8ba31d5`) | 1,914,432 | **13.3268 SOL** (13,326,792,240 lamports) |
| default, opt-z (measured, RED) | 1,474,584 | 10.2655 SOL |
| default saving | −439,848 | **3.0613 SOL** |
| mock, opt-3 (sealed `55ec393e`) | 1,942,904 | **13.5250 SOL** |
| mock, opt-z (ESTIMATE, unbuilt) | ~1,496,514 | ~10.4181 SOL |
| mock saving (estimate) | | ~3.1069 SOL |
| **both profiles, opt-3** | | **26.8517 SOL** |
| both profiles, opt-z | | ~20.6835 SOL |
| **both-profile saving** | | **~6.1682 SOL** |

(The register/frame-plan "~13.3 SOL" figure quoted against the walk-era
1,785,904-byte ELF actually computes to 12.43 SOL; at the *current* size the
exact figure happens to be 13.33 SOL. The table above is exact arithmetic at
current sizes.)

**The CU tax, quantified as far as honesty allows.** No current-tree opt-z CU
row exists (section 0). Taking the era band (+60–220%) as the only available
prior and running it against the sealed opt-3 rows and the profile's own
admission rule (25% headroom under the 1.4M ceiling ⇒ raw-CU admission
boundary 1,120,000 — REPORT_clearing-plane-promotion §1.1):

| sealed opt-3 row (CU) | ×1.6 (+60%) | ×3.2 (+220%) |
|---|---:|---:|
| FreezeEpoch 3pg/40 orders — 717,829 | **1,148,526 → STOP_HEADROOM** | 2,297,053 → exceeds the 1.4M ceiling outright |
| FreezeEpoch 2pg/17 — 478,009 | 764,814 PASS | **1,529,629 → exceeds ceiling** |
| walk pass 1, 40-order — 400,428 | 640,685 PASS | **1,281,370 → STOP_HEADROOM** |
| Direct V2 freeze — 357,879 | 572,606 PASS | **1,145,213 → STOP_HEADROOM** |
| CancelOrder — 287,671 | 460,274 PASS | 920,547 PASS |
| EntitleSlice portfolio pair — 246,173 | 393,877 PASS | 787,754 PASS |

Any row above 700,000 raw CU flips to STOP at the bottom of the band; any
above 350,000 flips at the top. The walk plane's worst sealed row is
717,829. **The promotion report's 25/25-PASS admission table is an
opt-3-identity fact**; at opt-z it is not known to survive, and at the era's
measured band it does not. The opt-level choice is therefore entangled with
D1/D2, not a free rent knob. (Also sealed and worth re-stating: Tier 0's
frame fixes cost almost nothing at opt-3 — cycle-B re-measurement drift was
+4 CU on folds, worst +274, GOAL.md:207-210. The cheap direction is safe;
the cheap *binary* is not.)

**What the rent buys, in context.** On devnet SOL is valueless but rationed:
the faucet is rate-limited, ember's 08-20 directive records "no devnet SOL
coming", and a patient collector polls in the background (GOAL.md:32-35,
:193, :713-714). The deploy-rent number is therefore a *time-to-actionable*
number, not money: ~13.33 SOL of collector patience for the default program,
~26.85 SOL for both profiles, ~3.1/~6.2 SOL less at opt-z. On any real-money
deployment the same bytes would be real SOL — which is why the frame plan
calls opt-z "a per-deployment economics choice, not a default" — but that
deployment is Track D, blocked behind counsel and audits (F6), and would
re-run this decision at its own identity anyway.

---

## 3. Upgrade posture against the project's own evidence culture

The options, in Solana-convention terms: (a) **immutable-from-start**
(deploy `--final` / authority `None` at first deploy); (b)
**upgradeable-then-burn** (retain authority for a bounded audited beta, then
irrevocably set `None`); (c) **multisig authority** (a Squads-style multisig
holds the authority, possibly forever); (d) **per-cluster split** — devnet
upgradeable, reference/mainnet posture decided separately.

What the tree already says:

1. **An upgrade authority is a writable trust root, and the project already
   treats it as one — for other people's programs.** The R2 source-trust
   model authenticates the Pyth receiver's ProgramData, pins its deployment
   slot, refuses an executable ProgramData, and treats any
   governance/upgrade change as a new feed generation by construction
   (research/source-profile-v1/src/auth_v2.rs:28-30, :162-171;
   R2_PULL_PROMOTION_PLAN.md §3). Symmetric honesty: whatever Dragon's
   Clutch demands of its source providers' deployment identity, its own
   deployment offers to its users. A retained authority is a disclosed
   fact-of-control in every filing and Terms sentence
   (DEPLOYMENT_REVENUE_BOUNDARY.md §6: release manifests identify deployer
   and upgrade authority; no UI calls an author-affiliated venue neutral or
   ownerless "merely because the programs are immutable").
2. **The design corpus already leans immutable.** REVENUE_POLICY_V1.md:171-175:
   recipient rotation is "representable only as a program upgrade, i.e. not
   representable in this immutable deployment" — the revenue design's
   no-admin-instruction guarantee is *derived from* immutability. Every
   frozen policy const (`DIRECT_POLICY_V1`, the proposed
   `GENERAL_CLEARING_POLICY_V1`), every "no reap ABI ever" clause in R4, and
   the B4a treasury-rotation-means-new-Realm rule are honest exactly insofar
   as no key can rewrite the program under them.
3. **"Burn" must be provable, and the project already knows how hard that
   is.** The parked decoder work (E4, `fable/r2-runtime-capabilities`
   f9045a0) found that a revoked authority serializes to 13 bytes while the
   metadata region stays 45, so bytes [13..45) still hold the *previous*
   authority — a naive check reports a live authority on an immutable
   program. Upgradeable-then-burn therefore ends in a state whose proof
   requires exactly the decoder that is currently unmerged; and every user
   before the burn trusted the key, which the filings would have to say.
4. **Immutability forfeits fixes — and the tree's compensation machinery is
   real but has a measured hole.** The seal/attestation apparatus
   (100/100-gate manifest, Persvati attestation, byte-verified deploys)
   makes *what was deployed* provable, and the successor path (deploy a new
   program + new Realm; the R2 generation model) makes *replacement*
   representable without mutation. What it does not give is in-place defect
   response: an immutable program with a fault keeps the fault, users must
   migrate ids, and the sunk rent is unrecoverable (closing a ProgramData
   account requires the authority — an immutable program's ~13.3 SOL is
   permanently gone, where an upgradeable devnet program can be
   `solana program close`d and its rent recycled into the next generation).
   Today's opt-z measurement is a live demonstration of the defect class
   that would be frozen forever by a premature immutable deploy.
5. **Multisig** changes who holds the root, not whether there is one: the
   disclosure burden is identical in kind, the removal proof is the same,
   and it adds a governance/operational surface (signer set, thresholds,
   key rotation) that the filings would also have to describe. It is the
   right shape for a real-money operator with obligations; it is overhead
   without benefit for a Track-C research deployment and a weaker honesty
   story than immutability for the reference claim.

**The devnet fact on the ground:** the deploy script
(`~/jobs/dragons-clutch-devnet-20260819/deploy-and-verify.sh`) already
writes into its deployment record: "Upgrade authority: retained by the
deployer key for the paces window as a recorded Track-C beta authority
decision; irrevocability is a release-time decision, not made here." That is
option (d)'s devnet half, already drafted by a lane and awaiting
ratification — the same "ratify what's built" shape as A7.

---

## 4. How the two decisions interact

- **An immutable deploy raises the price of the wrong opt-level choice to
  its maximum.** Immutable + opt-z freezes a ~3 SOL saving against a defect
  class measured *today* as an actual fault, with no fix channel and the
  program id burned. Immutable + opt-3 wastes at most ~3.06 SOL of rent per
  program — bounded, known, and paid once. The asymmetry is stark: the rent
  cost of conservatism is capped; the semantic cost of a frozen wrong
  binary is not. Under an upgradeable posture the same wrong choice costs
  one upgrade (plus, for a *larger* replacement ELF, a
  `solana program extend` at the delta rent — note the script's exact
  `--max-len` sizing forecloses in-place growth without it).
- **Identity forking runs through both.** Every closure-byte change forks
  the ELF identity (the reseal protocol's whole premise), and opt-level is
  the largest single identity fork available. The seal, the manifest, the
  Persvati attestations, every sealed CU row, and the promotion report's
  admission arithmetic all bind to `e8ba31d5…`. A devnet deployment at any
  other identity is not "the sealed program, cheaper" — it is a second
  program with no attested evidence, and the deploy script's byte-verify
  step would be verifying bytes nothing else vouches for.
- **The R2 calendar makes devnet upgradeability structurally useful.** E2/E3
  (post-Aug-26 identity freeze, then the registry flip) force a reseal and
  a new identity by construction. An upgradeable devnet program id can
  carry the identity sequence in place — each upgrade a recorded
  deployment fact, id continuity for the paces record and the filings'
  Track-C paragraph — where an immutable devnet would strand ~13.3 SOL and
  burn a program id per generation, against a faucet that is already dry.
- **The mock-profile question is a pure F1×F2 artifact.** The mock ELF
  exists to run the funded lifecycle the default ELF refuses — but the mock
  provider trio cannot exist on a public cluster (GOAL.md:677-686): devnet
  paces on the mock add little the default ELF's native-market plane does
  not already exercise publicly. Deploying it doubles the funding
  requirement (26.85 vs 13.33 SOL) for a program whose distinguishing path
  cannot run there. Under an upgradeable posture this is a cheap deferral:
  deploy default first, add mock only if a concrete devnet scenario needs
  it.

---

## 5. What deploys where, under the current authorization state

- **Authorized now (Track C):** devnet/testnet with fresh throwaway keys and
  bounded public-RPC use (ember 2026-08-19, CURRENT_TRUTH.md:136-148); no
  real value, exact build identity, measured operation, no legal claim.
  Mainnet, real value, the registry flip, and official-claim language remain
  human-gated (F6/E3).
- **Blocked in fact:** the faucet. "No devnet SOL coming" (GOAL.md:32-35);
  the collector polls; the deploy job (fresh deployer `4zrxtw5c…`, program
  ids `3SLhMAFm…` default / `EbWhsDm4…` mock) is "ready to fire the moment
  the deployer is funded" (GOAL.md:686, :711-714). No testnet job exists;
  the same authorization and the same analysis apply if one is created.
- **Stale pointer to fix before it fires:** `deploy-and-verify.sh` deploys
  `DEFAULT_ELF=…/artifacts/bd20711b01828a74/clutch_sbf.so` — a superseded
  identity four seals old — and a job-local mock ELF of the same era. If
  the collector succeeds tonight, the script as written deploys bytes the
  current manifest does not bind. Repoint to
  `research/liveness-policy-profile/artifacts/e8ba31d582be3939/clutch_sbf.so`
  (and stage the current `55ec393e…` mock, if the mock deploys at all)
  before funding arrives.
- **The current evidence ceiling is local, and it is high:** the sealed
  loopback 22-transaction signed walk, the devnet-paces dry-run (both
  profiles, blank validator, fresh program ids — devnet's exact shape, plus
  a required-red control), and the in-flight general-plane signed validator
  walk (GOAL next-3 item 1) — the maximum devnet-free evidence class.
  Devnet adds public-cluster inclusion reality and a citable deployment
  record (the Draft-11 `[DEVNET RECORD]` fill-ins), and nothing else: the
  paces are 28 accepted public transactions plus the exact refusal
  boundaries (0x79/0x7a/0x4); the funded mock lifecycle stays local until
  the real Pyth-pull build (R2). If the faucet stays dry through Aug 24,
  the filings' Track-C paragraphs must ship honestly deployment-less.

---

## 6. Recommendations, with counterarguments

### F1: deploy devnet at the sealed opt-3 identity `e8ba31d5…` — default profile first, mock deferred. Refuse opt-z for any deployment until it is re-greened and gate-campaigned at its own identity; keep "opt-z devnet-2 comparison" as an explicitly optional later act.

Rationale, in order of force:

1. **Opt-z is red today** (section 0) — there is nothing green to deploy at
   that opt-level. This alone decides the near-term question.
2. **Evidence continuity is the point of the deployment.** Track C promises
   "exact build identity"; only `e8ba31d5…` is manifested, attested, and
   CU-measured. A devnet carrying the sealed identity turns every local
   claim into a publicly re-verifiable one (the script's dump-and-hash step
   literally byte-verifies it). A devnet at any other identity would be a
   second unattested program — evidence *fragmentation*, not savings.
3. **The saving is faucet-time, not money** (~3.06 SOL/program on a
   valueless rationed token), while the cost of the opt-z route is a
   re-green lane + a full gate campaign + a permanently forked evidence
   plane. The economics invert on a real-money deployment — and that
   deployment (Track D) re-runs this choice at its own reseal anyway.
4. **Mock deferred:** its distinguishing path cannot run on a public
   cluster; deferring halves the funding threshold to ~13.33 SOL + fees and
   makes the deploy actionable sooner. Deploy it later from the same job if
   a concrete scenario wants it (upgradeable posture makes this cheap).

Counterarguments, answered:

- *"The savings compound: ~6.2 SOL both-profiles, every generation."* On
  devnet the true cost driver is collector time, and the mock deferral
  (−13.5 SOL) saves twice what opt-z would (−6.2) with zero engineering and
  zero evidence fork. The compounding argument belongs to a real-money
  deployment that is blocked on other grounds entirely.
- *"Re-green it now — Tier 0 took one lane."* Perhaps, but Tier 0's own
  history is the caution: its green lasted less than two days of merges
  because nothing *enforces* the frame margin at opt-z (no gate builds at
  z). A re-green without a standing opt-z gate is a snapshot, not a
  property — and the register's fan-out already has higher-ranked uses for
  a lane (the walk plane, R2). If ember wants the option alive, the honest
  unit is "re-green + add an opt-z frame-diagnostic gate to the manifest",
  costed in section 7 — worth doing only when a deployment that pays real
  rent is actually in view.
- *"Deploy opt-z on devnet precisely to measure the CU tax publicly."* The
  tax is measurable locally for free once re-greened (the same bank
  measures both identities); devnet adds nothing to a CU number and would
  spend the scarcer resource (faucet SOL) on the less-attested artifact.
- *"The +60–220% band is stale — maybe the syscall era shrank the tax."*
  Plausible (the SHA syscall removed the biggest CU consumer), genuinely
  unknown, and unmeasurable until opt-z executes without faulting. The
  argument cuts toward re-measuring after a re-green, not toward deploying.

### F2: split the posture, and decide both halves now. Devnet (Track C): ratify the recorded beta authority — deployer key retained for the paces window, disclosed in the deployment record, with `solana program close` recycling permitted between generations. Reference deployment (Track B kit and any real-money instance): **immutable at first deployment**, stated now as the design posture; revisit with counsel at Gate L0 only if audit sequencing genuinely demands a bounded beta — and if so, upgradeable-then-burn with the burn proven by the E4 decoder, never a standing authority, never multisig-forever.

Rationale:

1. **Devnet upgradeability is honest and useful there**: no immutability
   claim is made or needed at Track C; the R2 calendar guarantees identity
   churn (section 4); rent recycling matters when the faucet is the
   constraint; and the posture is already written into the deployment
   record text awaiting ratification — ratifying makes the de-facto
   decision a decided one, which is exactly the hygiene the register exists
   for.
2. **Reference immutability is what the corpus already assumes** (revenue
   rotation-as-new-Realm, frozen policy consts, R4's no-reap-ABI-ever,
   "programs are immutable" as the premise of every neutrality caveat).
   Declaring it now lets B4a, C1, and the Terms language build on a decided
   fact instead of a lean; the manifest fields (deployer / upgrade
   authority) get their reference values ("authority: none, from slot 0").
3. **The trust-root argument is the project's own.** The filings would have
   to disclose a retained authority as control; the evidence culture's
   whole apparatus (seals, attestation, byte-verified deploys, the R2
   generation model) exists to make *replacement* provable so that
   *mutation* is never needed. Fix channel under immutability: deploy the
   successor, new Realm, users migrate by choice — the same generation
   semantics the project imposes on Pyth.
4. **The forfeited-fixes cost is real but already sequenced away from
   danger**: real money is blocked until audits and counsel exist (F6
   Track D), so the window in which an upgrade authority would be the only
   defect response — post-real-money, pre-audit — is a window the project
   has already forbidden itself. Devnet, where defects are actually
   expected (today's opt-z finding, the paces campaign), keeps its
   authority under this recommendation.

Counterarguments, answered:

- *"Immutable-from-start on devnet too — practice the real posture."* It
  strands rent per reseal generation on a dry faucet, burns program-id
  continuity for the paces record, and practices nothing the byte-verify
  step doesn't already prove. The deployment record's honesty (authority
  disclosed, no immutability claim) is the practiced discipline that
  matters.
- *"Keep a bounded beta authority on the reference deployment — first
  deployments have defects."* The reference deployment's users would then
  hold instruments whose terms a key could rewrite during exactly the
  period defects are likeliest — the worst moment to hold the root. The
  project's alternative is to keep the *reference* claim off any instance
  until the evidence ladder (audits, second-profile promotion, paces) has
  been climbed on mutable devnet instances — i.e., spend the beta period
  where the beta authority already legitimately lives.
- *"Multisig splits the risk."* It multiplies the disclosure and proves
  nothing stronger; see section 3(5). Right answer for an operator with
  obligations (Track D with counsel), wrong default for a reference
  artifact whose value is that no one holds it.
- *"Deciding mainnet posture now is premature — F6 owns mainnet."* The
  decision recommended is the *design posture* (what the manifest schema,
  Terms drafts, and revenue design assume) — precisely what F2's P0 row
  asks ("source must support either without pretending"). The
  deploy-time act stays behind F6/L0 regardless; ember+counsel can still
  overturn it with the full facts, and the counterargument they must
  answer is item 3 above.

---

## 7. Execution costs

| act | prerequisite | engineering | cost when it runs |
|---|---|---|---|
| **F1 as recommended** (opt-3 default to devnet) | collector reaches ~13.33 SOL + fees | repoint two ELF paths in `deploy-and-verify.sh`; defer the mock `deploy_one` call | zero reseal (deploys sealed bytes); one deployment record; byte-verify included in the script |
| mock to devnet, later | ~13.53 SOL more; a scenario that needs it | none (script already handles it) | same |
| **opt-z revival** (only if a rent-paying deployment appears) | ember's call to spend a lane | Tier-0b pass over the reachable overflowers (start: 4 layout-crate fns incl. `Intent::decode` +832, `validate_artifact` +1,024); add a standing opt-z frame-diagnostic gate so the green cannot rot silently; then ONE full default+mock gate campaign at the new identity; CU re-measurement to price the tax truthfully | a near-reseal-scale campaign for a second identity; permanent double-identity bookkeeping if both ever coexist |
| **F2 devnet ratification** | none | none (text already in the script's record) | one register/CURRENT_TRUTH line marking the recorded posture decided |
| **F2 reference-immutable posture** | none for the declaration | retire OPEN_QUESTIONS P0 row; fix the manifest field semantics (upgrade authority: none) in DEPLOYMENT_REVENUE_BOUNDARY §6's manifest list; align Terms/filing draft language; note the E4 decoder as the burn-verification dependency if the bounded-beta escape is ever taken | the deploy-time `--final` is free; the irreversibility is the point and the price |

Ordering: the two zero-cost F2 acts and the script repoint are same-day; the
deploy fires on the faucet's schedule; opt-z revival is deliberately parked
behind a real rent bill.

---

*Compiled 2026-08-20. Sealed inputs: manifest at `788581c` (identities
MANIFEST.baseline.json:3154-3156), liveness evidence.json at root
`e8ba31d582be3939…` (rent params, opt-3 CU rows), FRAME_BUDGET_PLAN
_2026-08-19.md, GOAL.md lane records, deploy script read from
`~/jobs/dragons-clutch-devnet-20260819/`. New measurement: one opt-z build
(scratch target, this worktree — digest path-tied) + one default SVM suite
run, log in the session scratchpad (`optz-default-suite.log`); the staged
opt-z fixture was removed after the run. The measurement is reproducible
with one command:
`CARGO_PROFILE_RELEASE_OPT_LEVEL=z SBF_TARGET_DIR=<scratch> programs/clutch-sbf/svm-tests/run_svm_tests.sh`.*
