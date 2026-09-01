# To Claude, from the Codex completion swarm — 2026-09-01

This is a deliberately early wind-down. Ember stopped the overnight plan at
36% weekly usage and asked for a strong handoff after one final convergence
hour. Do not interpret the stop as completion. The project remains governed by
`docs/MASTER_COMPLETION_CONTRACT.md`: every accepted ambition must become
implemented with physical evidence or be explicitly ruled out by Ember.

## Start here

The authoritative checkout is `/Users/ember/dev/dclutch`, not
`/Users/ember/dev/dragons-clutch`. At this handoff the latest root checkpoint is
`2af02f53`. The shared checkout is intentionally dirty because several
capability campaigns stopped at honest physical walls. Do not reset, clean,
stash, run `git add -A`, or absorb an unowned file. Inspect and commit exact
named paths only.

Read, in order:

1. `AGENTS.md` — current authority and vocabulary. It supersedes old handoffs;
   in particular, devnet work is now allowed when it is actually part of the
   task, while mainnet, private wallet material, publication and pushing are
   not authorized here.
2. `docs/MASTER_COMPLETION_CONTRACT.md` — the stopping condition and C-00..C-16.
3. This letter — current executable frontier and exact continuation commands.
4. `docs/INTENT.md`, `docs/OMISSION_INDEX.md`, and the rulings near the end of
   `WAVE.md` when a product or economic choice becomes the only wall.

`GOAL.md` and `WAVE.md` are deep historical ledgers, not reliable current
queues by themselves. “Cohort cuts” were Claude's old planning vocabulary, not
an Ember requirement. Backlog, General and Dealer are explicitly in scope.
Avoid inventory-only gap-audit lanes; drive one accepted life until the next
real authority, conservation, packet, compute, or UX wall appears.

## What landed in the final Codex wave

- `4306d389` constructs permissionless native claim-check compaction over the
  authoritative terminal-payout frame. It authenticates the finalized
  snapshot, escrow, admission, vault, deadline and scope joins and projects
  exact escrow/record poststate.
- `e0ece22e` compiles that compaction as a canonical v0 transaction and verifies
  exact terminal receipt, aggregate, replay, Hoard, vault, escrow, record,
  closed Position/admission and conservation-sink poststate.
- `2af02f53` keeps wallet-payout and claim-check recipient parsing disjoint in
  the local successor. Its focused hbox run is 21/21 green.
- `686bf2e5` binds Trading lifecycle credit to the authenticated fixed Registry.
  The exact normal Trading ELF is 2,285,152 bytes, SHA-256
  `af5d955e01b319820bcb96cfdac90f83412132541dd7ed2b0102aedab4198f5f`.
- `73ffb010` binds General create/close permissions to the same semantic facts;
  the account-profile suite is 55/55, General adapter 256/256, and full
  operator 246/246.
- `792ad360`, `292c0687`, `29524890`, `c735ed7a`, and `cdf77897` move Series to
  current profile/effect schemas, append-only authenticated acquisition,
  correct projected-variable markers, and a canonical expiry fixture. The last
  fixture's native ProgramTest target is 2/2 green.
- `30be79b1` executes Source funding close in the browser.
- `7170fd97` makes deployment truth an SDK semantic owner.

These are component and campaign advances, not a declaration that their C-row
is complete.

## The honest current frontier

### Claims compaction

The operator constructor/compiler/poststate verifier exist. The complete
open → wait → compact → redeem → close successor exterior does not. The
protocol's compaction deadline is 180 days, so a fresh local-validator or
devnet market cannot honestly produce a positive elapsed-deadline transaction.
Do not shorten or bypass it for a green demo. ProgramTest may warp for positive
physical evidence; the public/operator exterior should be able to open now and
truthfully explain/refuse an early attempt.

`tools/load-simulator` is currently 152 tests green and
`simlife_drive.py routes` already drives founding, admission, fill, resolution,
deadline refusal, redemption, retirement and census. It does not yet have a
real CLI route for claim-check compaction. Add that only after the exterior is
the semantic owner; do not simulate a success the chain cannot presently
reach.

Focused continuation:

```sh
cargo test -p dclutch-operator claim_check_v1 -- --nocapture
cargo test -p dclutch-operator wallet_terminal_payout_v3 -- --nocapture
ssh hbox 'cd /tank/dregg-build/dclutch-wallet-parser.8blk1K && swarm-build cargo test --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml wallet_terminal --no-fail-fast'
```

The local Mac failed a successor rebuild before compilation because its disk
was full; no artifacts were deleted. Use hbox or recover disk deliberately.

### Dealer physical life

The only Dealer-owned dirty file is
`programs/dclutch-dealer-accelerator-sbf/program-test/tests/accepted.rs`
(+3401/-154), SHA-256
`e1bac1e87cef8a559df6e731f5ad461d54f019147cf067d1a77792b087ef376e`;
`git diff --check` is green. It contains two LPs, time-separated adds,
inventory trading, split activation, partial/final removals, exact floor
rounding, no cross-LP subsidy, rollback hostiles, zero residue and terminal
collateral conservation. It was not committed because it is not terminal
green.

On the pinned artifacts, selector-7 LP Open executes through the real
accelerator. A substituted-position selector-1 Add refuses correctly. The
honest Add then refuses Trading `Content` at 148,093 CU, between Hot checkpoints
`root-product` and `artifacts-strategy-effect`. The first wall is therefore the
immutable-artifact authentication tranche in `hot_v3.rs:3222-3525`; there is no
evidence yet for weakening a narrower predicate.

```sh
ssh hbox 'ln -sfn /tank/dregg-build/dclutch-trading-686bf2e5.RlF8aK/sbf-out/dclutch_trading_sbf.so /tank/dregg-build/dclutch-dealer-accepted-686.1788246335/sbf-out/dclutch_trading_sbf.so'
ssh hbox 'cd /tank/dregg-build/dclutch-dealer-accepted-686.1788246335 && SBF_OUT_DIR=/tank/dregg-build/dclutch-dealer-accepted-686.1788246335/sbf-out SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build cargo test --manifest-path programs/dclutch-dealer-accelerator-sbf/program-test/Cargo.toml --test accepted accepted_equity_selector_one_executes_real_custody_and_rolls_back_late_evidence_refusal -- --nocapture'
```

The profiled Trading ELF is SHA-256
`a2e62944bc03351bbb68154648ba83dde6101ebbb3877319d547797a537ee27a`.
Do not rebuild this proof boundary from the ambient dirty HEAD until the
profile/lifecycle work converges; that produces an unrelated host `Geometry`
failure.

### Series

The canonical expiry fixture now reaches current Operator construction and
authenticates independent Claims founding replay 0→1, the domain-derived
Template ID and exact slot-2 schedule. The next physical boundary is a bounded
top-level slot-2 warp with the already built ELFs. The only Series-owned dirty
file is
`programs/dclutch-trading-sbf/program-test/tests/series_pre_market_expiry_program_test.rs`;
its three uncommitted `warp_to_slot(2)` calls cover the positive, underfunded
and caller-hostile transactions.

```sh
cargo test --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml --test series_pre_market_expiry_program_test native_tests -- --nocapture
SBF_OUT_DIR=/tmp/dclutch-series-elves.GQU64N cargo test --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml --test series_pre_market_expiry_program_test current_source_series_expire_lands_before_the_future_market_exists -- --nocapture
```

That ELF set is a useful diagnostic but is pinned to older commit `7c7ad184`;
rebuild Registry, Trading, Core, Claims and Custody together on hbox before
accepting it as current evidence.

Two larger facts remain. First, current Series V5 ProgramSet has only the
Prepare/Consume/Expire/Retire/Close action descriptors and no activation
`CapabilityProgramV1`; consequently no real root/Found/Prepare prefix can be
constructed. Add a genuine activation producer and bundle analogous to General,
never a legacy or synthesized root. Second, Template-authenticated `closeRent`
is separately prepaid principal. Preserve it: activation transfers root rent
plus that authenticated close principal, the root persists it, the outer
accepts the exact amount plus donation, and terminal Close proves conservation.
Do not impose a contradictory zero-close-principal rule.

### General and lifecycle infrastructure

General's lifecycle/profile semantic joins are green at `73ffb010`. The
uncommitted physical brick is the General portion of
`program-test/bundle-builder/{Cargo.toml,src/general.rs,tests/general_dynamic_spans_v1.rs}`
plus `program-test/general-hot/{Cargo.toml,Cargo.lock,src/lib.rs,tests/open_batch.rs}`.
It compiles and now uses the real accelerator-owned
`project_general_open_batch_candidate_in_place_v3` rather than incorrectly
interpreting the transition on the host. The run was stopped during dependency
compilation, so no new semantic refusal has yet been observed.

```sh
CARGO_TARGET_DIR=/Users/ember/dev/dclutch/target \
SBF_OUT_DIR=/tmp/dclutch-general-open-elfs.9L3rOz \
cargo test --manifest-path programs/dclutch-trading-sbf/program-test/general-hot/Cargo.toml --test open_batch -- --nocapture
```

Those retained ELFs are diagnostic, not current-source proof. Rebuild an exact
set through hbox `swarm-build` once host construction advances.

`a9181daf` lands the registered Sell→Buy chain fixture; the direct-hot suite is
19/19 green. It assembles current releases, signed requests, sequential root
prestate, maker/record PDAs, lifecycle RentCredit, Buy replay/vault/deposit
frames, and source/vault/rent/Claims observability. No real-ELF Sell or Buy has
yet been submitted. The next approved file is solely
`programs/dclutch-trading-sbf/program-test/tests/direct_registered_creation_hot.rs`.
Consume the fixture, install its accounts, add native Ed25519 evidence, submit
two same-bank Hot transactions, then assert exact root/maker/record, Custody
replay/vault/source, Rent and Claims conservation plus substitution rollback.
Watch the economic seam where Sell persists `reserved_claims` without a Claims
child route; do not wave it through.

The isolated untracked
`crates/dclutch-direct-codec/src/registered_terminal_artifacts_v4.rs` is 2,529
lines of incomplete terminal-artifact WIP. It compiled only while temporarily
registered and has no focused validator evidence. Preserve it unregistered.

### Structured representation and browser capability

The structured lane owns a real Trading → Claims/Custody → Token-2022
`IssueStructured` plus `Denominate` ProgramTest in
`programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs`.
Its five owned Claims/bundle-builder paths remain uncommitted because the
canonical hbox corpus is 43/44, not green. Builder route tests are 2/2 and the
Claims test target compiles. The exact failing boundary is before transaction
submission: `construct_chain_hot_issue_structured_v3` returns
`ChainOperator(InvalidToken)` at the new test's line 993. Inspect the bearer
operator against the fixture's receipt-mint/account observations and repair the
fixture, not runtime semantics, unless the evidence names a real semantic bug.

```sh
ssh hbox 'cd /tmp/dclutch-structured-hot.psglmE && SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build programs/dclutch-claims-sbf/run-rational-representation-v2-program-test.sh > hot-run3.log 2>&1'
```

The prior log is `/tmp/dclutch-structured-hot.psglmE/hot-run2.log`. Exact ELF
digests include Claims
`0c844eb81abe1be3be07aad1f2d533e9d4f42be10d1790a88ada47824bd5a3f8`,
Trading `d2bb39b12db1ccb870f32b112cfdb0a6c31c649e2040148d7576521788b25731`,
Custody `d20275907f3b66e1f7f7a5ecb4cb869da8a16c8efd46deddd2a6440db9ee4ace`,
and Token-2022
`e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697`.

`claims.represent` belongs to route `DCRRPRQ2`,
`rational_representation_v2::process`, actions `Denominate` and
`Reconstitute`. Never expose the bare generic Claims
`Materialize`/`Dematerialize` CPI wire as a wallet act. Planner, exact
poststate verifier and crash journal foundations are at `445bbad0`,
`0ef9f1e4`, and `825a7714`. Once physical evidence is green, the browser
execution seam should be a separate
`RationalRepresentExecution.tsx`: fresh cluster/blockhash, journal before
handoff, immutable wallet packet, retain exact signature before send, finalized
exact verification, and inspect-never-resubmit recovery.

`claims.conserve` is still unimplemented and must not be truth-flipped. Bare
Mint/Merge changes Claims supply without moving collateral. It requires one
atomic outer route joining uniform signed claim deltas with a Claims-role
Custody transfer to/from canonical Hoard principal, using authenticated scale,
release, Realm, Product, cap, replay, authority, frame and poststate.

## Product and UX direction

The old deployed `/console` screenshot is far behind current source, but its
critique remains useful: the directory tells a story of unavailable tools even
where real wallet/operator paths now exist. Do not create an “awaiting
production” product category. Complete the missing acts, derive the directory
from one executable capability model, lead with the outcome that works and then
name its venue. Keep warnings to actual signing, finality, recovery and
submission boundaries. Gricean here means enough context to act safely, no
generic protocol lecture, and no repeated apology for architecture.

The desired endpoint remains a pleasant stranger-operable browser for every
user act plus a highly explanatory operator surface for lifecycle and release
work. Compile pure deterministic Rust planners/codecs/checkers to WASM where
that removes TypeScript semantic duplication. Keep RPC, Wallet Standard,
durable browser storage and submission in the web shell. Do not move signing
authority or network truth into WASM.

## Shared-tree hazards

- `Cargo.lock` files, Core `infrastructure.rs`/`infrastructure_v2.rs`, and many
  program-test and operator paths are ambient-owned. The Core diffs appeared to
  be formatting-only, but ownership was not established; do not sweep them.
- Untracked `.claude/`, `GOAL.md.tmp-entry`, `tsconfig.tsbuildinfo`, formal
  generator temporaries, `registered_terminal_artifacts_v4.rs`, and
  `program-test/general-hot/` remain. Some are real lane products and some are
  debris; identify the author before deleting or committing any of them.
- Use exact named commits through `tools/lane.sh`. Heavy Linux work goes through
  `swarm-build` on hbox and must respect co-tenants.
- A static client, simulator, fixture, or projection is not chain truth. A
  green constructor is not physical completion. Never weaken a refusal to make
  a campaign pass.

## Recommended continuation order

1. Reproduce and convict Dealer's narrow immutable-artifact wall on the pinned
   evidence; land the terminal multi-LP life.
2. Finish the registered Direct Sell/Buy terminal campaign and unblock the
   General physical bundle without merging ambient lane truth.
3. Give Series its real activation owner, preserve prepaid close principal,
   and run two occurrences including consume and expiry to terminal close.
4. Converge the real structured representation campaign, then mount the exact
   crash-safe browser execution path.
5. Finish the public claim-check exterior and simulator route while preserving
   the real 180-day clock.
6. Re-run the capability census, make `/console` derive from executable truth,
   and remove only claims actually made obsolete by the physical campaigns.
7. Rehearse a fresh current-source devnet redeploy and deep load-simulator life;
   record exact releases, ELFs, transactions and poststates. Devnet success is
   devnet evidence, never mainnet evidence.

Do not call the project complete after any one of these. Return to C-00..C-16,
close every accepted row, and then perform the independent assurance-entry
audit. The requested emotional stopping condition is literal: nothing material
left to add or remove before formal verification/assurance begins.

## The counterfactual ten-hour swarmcycle

This section preserves the work we would have run overnight if Ember had not
stopped for usage conservation. It is not a prediction that every brick fits in
ten wall-clock hours and it is not evidence that any unexecuted step works. It
is the dispatch board: the intended lanes, dependencies, convergence waves,
file ownership, acceptance gates, and overflow queue. Claude should be able to
resume it without first inventing a new project plan.

The overnight outcome was to be one coherent current-source successor world:
Direct, General, Dealer, Series and useful structured claims all cross real
ELFs; Source and terminal Claims/Custody close them; browser and operator
surfaces execute what they claim; the simulator drives a population through
the same acts; and one checked release/devnet rehearsal records exact
identities. A red physical wall was expected output, but switching to tiny
cleanups or copy work instead of convicting that wall was not.

### Complete lane registry

| Lane | Contract/omission rows | Semantic responsibility | Material deliverable |
| --- | --- | --- | --- |
| S0 convergence | C-00, C-16 | Scope, ownership, integration and hostile review; no protocol semantics | One current evidence ledger and an honest next wall for every lane |
| S1 infrastructure/release | C-01, C-14; U-011, U-012 | Registry lineage, infrastructure-profile succession, migration, Rent and reproducible artifacts | Cold V2 ceremony, exact-current ELF pack and checked release candidate |
| S2 product/Source/Resolution | C-02, C-09; U-006, U-009 | Canonical product entrance, Source readiness/provider/fallback, objective resolution and funding closure | One authored product and one real provider/fallback life through fund close |
| S3 Direct | C-04; U-002 | Inline and registered/reserved portable-ticket lives | Two actors, Sell/Buy, fee completion, resolution, redemption and every close |
| S4 General | C-05; U-003 | Best-valid-submitted-candidate runtime lifecycle | N=2 and N=258 collect/select/materialize/distribute/expire/close campaigns |
| S5 Dealer/GeneralDealer | C-06, C-11; U-004 | Multi-LP inventory venue, scenario solvency, consent/evolution and terminal withdrawal | Terminal two-LP pool life with exact reserves and no cross-LP subsidy |
| S6 Series | C-07; U-005 | Activation, recurring tickets, once-only funding and terminal occurrence closure | At least two occurrences covering consume and expiry |
| S7 curves/Structured | C-03, C-08; U-008, U-013, U-015 | Degree-2/3 payoff admission and useful exact-denominator representation | Curve compile/found/issue/represent/transfer/reconstitute/redeem/retire life |
| S8 Claims/Custody/terminal | C-10, C-11; U-007 | Complete-set conservation, native positions, Hoard isolation, payout, compaction and closure | Split/merge plus terminal payout/claim-check life with asset-by-asset ledger |
| S9 browser/operator | C-12, C-13; U-010 | Wallet recovery, static product journeys, cold operator and capability truth | Stranger-operable journeys and generated Gricean console/operator surfaces |
| S10 simulator/devnet/economics | C-11, C-14; U-011 | Population load, liveness work, economic census and public-test rehearsal | Machine-readable multi-market life plus current devnet flight |
| S11 execution/debt/horizon | C-00, C-15, C-16; U-001, U-014 | Generic execution equivalence, accepted backlog, privacy ruling and assurance entry | Debt deletion/ownership ledger, AOT/interpreter disposition and closure challenge |

Rows overlap only at integration boundaries. For example, S5 owns Dealer
scenario semantics, S8 owns Claims/Custody conservation semantics, and S10
measures their economics. None may duplicate the other's persisted facts.
GeneralDealer is ruled in: “accepted Dealer profile” is the first physically
reachable product, not a reason to omit the broader general-dealer ambition,
consent-safe evolution, or a deliberate Ember ruling about its final breadth.

### Convergence topology

The dependency order is:

1. S1 publishes one exact source/release/artifact substrate.
2. S2 and S8 make Source, Claims, Custody and terminal acts usable by all
   families.
3. S3–S7 drive family lives against that substrate and return exact physical
   walls to their semantic owners.
4. Only physically green acts cross to S9 and S10.
5. S1 then cuts a checked candidate; S10 may rehearse it on devnet.
6. S0/S11 try to disprove every claim and return failures to the owning lane.

Browser copy, simulator state and release manifests never flow backward as
protocol truth. A family may use a pinned older ELF pack for diagnosis, but it
cannot claim current completion until all required ELFs come from one committed
source archive.

### Ten-hour wave schedule

| Time | Swarm wave | Required convergence output |
| --- | --- | --- |
| T+0:00–0:30 | Freeze and provenance | Claim exact dirty files, preserve their SHA-256s, select hbox/persvati substrates, restore intended ELF symlinks, and emit one machine-readable lane board |
| T+0:30–2:30 | First physical crossings | Reproduce Dealer Content, Structured InvalidToken, Series first-bank, General OpenBatch and Direct registered Sell/Buy in parallel; run infrastructure ceremony and Source controls alongside them |
| T+2:30–4:30 | Semantic-owner repairs | Fix only convicted owners; for every honest success add the closest authority/replay/release/rollback hostile and checkpoint separately |
| T+4:30–6:00 | Lifecycle convergence | Extend successful prefixes through Claims/Custody, resolution, redemption, fee work, retirement and rent closure; join family ledgers into one conservation schema |
| T+6:00–7:30 | User/operator crossings | Compile deterministic planners/checkers to WASM, add crash-safe Wallet Standard flows, cold CLI/operator exteriors and derived capability evidence |
| T+7:30–8:30 | Population and chaos | Drive mixed families on a shared clock, competing keepers, absent holders, restarts and load; record executed/refused/blocked/not-attempted distinctly |
| T+8:30–9:30 | Release and devnet | Rebuild exact-current artifacts, run checked-release gates, rehearse upgrade/bootstrap and—only if the local campaign is coherent—perform the authorized devnet flight |
| T+9:30–10:00 | Hostile convergence | Independent row-by-row challenge, web/SDK/static twin gates, stale-claim search, named commits and a new handoff containing only reproduced commands |

At each wave boundary S0 should harvest material commits, rerun the smallest
cross-lane gates, and redistribute newly exposed walls. The coordinator should
not become an extra feature lane except for a genuinely shared seam whose
semantic owner is otherwise ambiguous. Terra is appropriate for mechanical
fixture regeneration, copy consistency, command transcript extraction and
bounded parity checks; substantive authority/economic/runtime bricks stay with
the capability crews.

### S0 — convergence and evidence ledger

S0 owns `docs/MASTER_COMPLETION_CONTRACT.md`, the handoff, lane ownership and
the machine-readable run ledger—not the programs. It should create or extend a
single campaign ledger containing source commit, dirty-patch digest, build
host, toolchain, ELF digests, release identities, exact commands, transaction
signatures/slots, CU, packet/lock geometry, pre/poststate digests and result
classification. “Blocked” must name the first failed invariant; “not
attempted” must never be presented as refusal evidence.

Every two hours S0 should:

- harvest only green named-path commits;
- preserve red WIP with an owner, digest and next command;
- run `git diff --check` and check shared-file ownership;
- reject capability truth flips without physical evidence;
- identify one actual shared authority wall instead of spawning a gap audit;
- update the C/U mapping only when executable evidence or an Ember ruling
  changes it.

The final S0 output is not “all tests pass.” It is a conservation/lifecycle
ledger that lets an independent reviewer traverse authored product → checked
release → transaction → finalized poststate → user/operator claim.

### S1 — infrastructure succession, release and reproducibility

Start from P-008: `ProtocolInfrastructureProfileV2` and its seven-conjunct
ceremony exist, all consumers read V2, but the ceremony is still
NEVER-EXECUTED. The cold world must reproduce the real ordering: predecessor
profile, Registry upgrade, successor record publication, ceremony, read-back,
then founding.

Likely owners:

- `programs/dclutch-core-sbf/src/infrastructure.rs` and
  `infrastructure_v2.rs`;
- `crates/dclutch-operator/src/infrastructure_succession_v1.rs` and
  `infrastructure.rs`;
- `tools/local-validator/bootstrap/successor/src/infrastructure_succession.rs`,
  `upgrade.rs`, `release_identity.rs` and `release_lineage.rs`;
- `tools/release/private_validator_upgrade/`,
  `tools/release/devnet_upgrade_dryplan/`,
  `tools/release/checked-release-candidate.sh` and artifact-provenance tools.

Bricks:

1. Establish ownership of the ambient Core formatting diffs and the dirty
   successor lockfile; never sweep them into the ceremony.
2. Execute the exact V1→Registry-upgrade→V2 ceremony through real Core and
   Registry ELFs. Red-prove ProgramData authority, predecessor presence,
   release identity, forward slot, moved-binding consent, no fork and vacancy.
3. Run a no-op succession hostile: a world where nothing moved must refuse for
   the intended reason, while the bootstrap constructs the actually moved
   world.
4. Re-found representative Direct/General/Series/Structured markets after
   succession and walk lineage from chain state.
5. Rebuild all programs through hbox `swarm-build` from one clean archive;
   record frame sizes, CU/packet ceilings, SBOM/licences, toolchains and
   artifact digests; produce a checked candidate, not a release declaration.

Terminal gate: a cold command can reproduce bootstrap, upgrade, ceremony,
founding and read-back without a private fixture or hand-carried predecessor
blob. Migration and every Rent principal/refund are accounted for. Devnet
rehearsal consumes this output; no family may invent its own release set.

### S2 — compiler-shaped entrance, Source and objective Resolution

This lane converts a human product description into exhaustive, disjoint,
ordered canonical outcomes; exact integer payoff/basis; immutable Realm,
Product, Source, Funding and capability identities; then drives the Source
from Found through provider/fallback and exact fund closure.

Likely owners:

- `crates/dclutch-product-compiler`,
  `crates/dclutch-product-payoff-v2-codec` and
  `crates/dclutch-source-contract`;
- `programs/dclutch-resolution-proof-sbf/src/provider_v3.rs`,
  `pre_market_funding_v1.rs`, `pre_market_funding_abort_v1.rs` and
  `core_effect.rs`;
- `programs/dclutch-core-sbf/src/resolution.rs`;
- `tools/local-validator/bootstrap/successor/src/market.rs`,
  `flagship_resolution.rs`, `direct_resolution_campaign.rs` and
  `source_abort_exterior.rs`;
- `apps/dclutch-web/app/product-v2/page.tsx`,
  `app/resolution/page.tsx` and the Source Rust/WASM-generated SDK/web twins.

Bricks:

1. Make Product Studio compile categorical, degree-1 and the accepted structured
   example into one canonical artifact packet; read it back with human
   explanations derived from the same records.
2. Cold-found Source and Funding from exact current releases, admit independent
   participants, verify readiness and preserve crash recovery.
3. Submit real provider-authenticated evidence; run source silence/fallback,
   competing witnesses, stale/substituted provider/release and relay-absence
   hostiles. Clients and relays remain untrusted transports.
4. Accept first-valid evidence in Core, emit the terminal certificate, close
   Source/Fund across their exact ledgers, and prove reclaim cannot touch Hoard
   principal.
5. Add another provider only as a separately release-bound real ABI/crypto
   adapter. Never use a mock as production breadth.

Terminal gate: one command authors, founds, funds, resolves and closes the
Source while the browser/operator explains who can act, what evidence is
missing and which fallback/deadline is next. The result comes only from
authenticated source evidence—not the client, index, operator or caller.

### S3 — complete Direct life

The first brick is the registered Sell→Buy physical file named in the frontier:
`programs/dclutch-trading-sbf/program-test/tests/direct_registered_creation_hot.rs`.
It consumes `build_direct_registered_creation_chain_fixture_v4`, installs exact
accounts, adds native Ed25519 evidence and submits two same-bank Hot
transactions through exact current Trading/Core/Claims/Custody/Registry ELFs.

The first harness wall may be the Rent executable, not Direct semantics: the
fixture supplies `rent_program` and the Profile expects real Loader-v3 program
bytes. Stage the exact Rent ELF/ProgramData in the integration harness; never
replace it with a placeholder executable.

Acceptance for the prefix includes exact root/maker/record poststates, Custody
replay revision 3, vault/source balances, Lifecycle RentCredit and
Claims aggregate/Position/reservation conservation. Authority, replay, source,
record and release substitutions must fail with rollback.

Then resolve the isolated terminal-artifact WIP at
`crates/dclutch-direct-codec/src/registered_terminal_artifacts_v4.rs` rather
than merely registering its 2,529 lines. The missing semantic decisions are:

- does registered Sell creation provision the record-owned Claims Position
  that terminal sparse transfer assumes;
- how side-specific Profiles avoid demanding unrelated disabled-side accounts;
- whether coordinate 9 has a semantic owner;
- how Admit/transfer/Close reclaims the drained Claims Position and rent;
- whether Buy's three Custody revisions and CloseVault/CloseReplay prove open
  count and refund conservation.

The terminal actions are maker-signed Cancel and unsigned strict-deadline
Expire (`valid_through < trusted_slot`; equality refuses). The record closes,
maker/root remain until their own children close, Sell returns remaining
Claims, and Buy returns residual collateral before ordered vault/replay close.
One profile cannot demand unauthenticated dummy accounts for the disabled
side. Prove the side geometry before registering the draft module.

After those are physical, extend the campaign through portable ticket
authorship/relay-optional crossing, third-party fee completion, Source
resolution, terminal payout, maker/position/record/replay/vault children,
aggregate and Core `Retired`. The browser and simulator consume the operator's
bytes; they do not restate Direct semantics.

Terminal gate: two independent identities can complete the whole Direct life
with relay absence, interrupted wallet recovery, exact fee atoms and all
temporary accounts closed. Run packet, lock, CU and hostile evidence on one
exact ELF pack.

### S4 — complete General life

Start with the uncommitted `program-test/general-hot` brick and
`bundle-builder/src/general.rs`. It now delegates admitted candidate projection
to the real accelerator semantic owner instead of host-interpreting the
Transition. Run OpenBatch at N=2 first; after host construction succeeds,
rebuild current ELFs and run N=258 with v0/ALT evidence.

The physical chain must then cover:

1. OpenBatch and collect more than one independently authored candidate.
2. Validate candidates and refuse bad release, lifetime, width, ordering and
   transition/effect evidence without state mutation.
3. Freeze at the authenticated boundary and select the best valid submitted
   candidate—never call it optimal without a checked optimality certificate.
4. Materialize and distribute exact deltas, preserving Claims/Custody and
   absent-holder non-charging.
5. Expire/resume interrupted work, close the General batch/root children, then
   hand terminal Market retirement to Core/Claims/Custody rather than pretending
   General `Close` retires the Market.

Likely owners are `crates/dclutch-general-adapter-contract`,
`crates/dclutch-operator/src/general_*`,
`program-test/bundle-builder/src/general.rs`,
`program-test/general-hot/tests/open_batch.rs` and
`tools/local-validator/bootstrap/successor/src/general_market.rs`. Browser and
SDK work waits for a machine-readable N=2/N=258 campaign report.

Terminal gate: a real General root runs the complete candidate lifecycle on
current ELFs at both representative widths, with lifecycle zombie refusals,
rollback, CU/packet/ALT and terminal conservation recorded. If the historical
`0x5182 ClaimsFounding...Release` reappears in Direct control too, return it to
the shared founding/release owner instead of patching General.

Two honest walls must remain on the board. The current action catalogue has no
gen-3 stalled-settlement expiry: Ember must rule whether a partially collected
settlement unwinds to makers or completes the frozen result, and who funds the
crank/cleanup, before a new action/tag exists. Ordinary interruption recovery
does not need a Resume action; rebuild the next Verify/settlement act from its
cursor's finalized poststate. Separately, General `Close` closes its settlement
cursor, not `GeneralRootV2` or Core. A family-neutral terminal owner or
first-class append-only General retirement must physically drive the root and
Market to `Retired`.

The target happy-path ladder per representative width is approximately:
Open 1, Place 2, CloseBatch 1, Submit 2, Verify 4, Consider 2, Freeze 1,
Initialize 1, Collect 2, Materialize 1, Distribute 2, Close 1, ReleaseOrder 2
and CloseCandidate 2. Its cold successor command advances one durable action
per invocation and emits the next authenticated act rather than a second
planner.

### S5 — Dealer and the broader GeneralDealer ambition

Resume the preserved `accepted.rs` patch at its pinned Hot Content wall. Restore
the normal `686bf2e5` Trading ELF, reproduce honest selector-1 Add, and compare
the sealed equity manifest/program-set/descriptor/config/lifecycle/profile/
request/strategy/transition/effect/static-owner tranche between
`root-product` and `artifacts-strategy-effect`. Add bounded checkpoints only if
needed to name the first mismatched fact. Do not weaken Content.

Once Add lands:

1. Execute two LPs with time-separated deposits.
2. Run scenario selector 9 inventory trading against canonical Claims state.
3. Exercise admitted candidate/split activation.
4. Partially and finally remove both LPs using cumulative-difference floor
   rounding.
5. Prove no cross-LP subsidy, no Hoard subsidy, replay/substitution rollback,
   zero shares/principal/residue and terminal collateral conservation.
6. Drive consent-safe policy/epoch evolution and show old LP rights cannot be
   silently rebound.

Primary files are the preserved ProgramTest, operator
`dealer_scenario_hot_v4.rs`, `dealer_lp_hot_v4.rs`,
`dealer_scenario_checkpoint_v1.rs`, Trading checkpoint owner and Custody
reservation owner. Generated web/SDK Dealer ABI is updated only through its
Rust generator after physical success.

GeneralDealer overflow remains explicit. Decide with Ember whether the final
venue breadth is the accepted scenario-solvent profile or a broader generalized
dealer, and whether residual-asset LP shares/loss ordering are desired. Before
that ruling, model present TradingPrincipal, exact scenario reserve, consent,
supply and terminal redemption adversarially. Future fees are not capital;
Hoard principal is not LP reserve, bounty, upkeep or treasury.

Terminal gate: the two-LP life closes at real ELFs and the operator/browser can
add, trade, inspect consent/evolution and withdraw without inventing topology.
The simulator receives exact reserve/equity poststates, not a parallel pool
model.

After physical green, replace the stale SetV1 projection in
`crates/dclutch-operator/src/dealer_equity_hot_v3.rs` with the accepted
SetV2/CapabilityProgramV4/EffectV4 truth and current geometry, while preserving
the LP/scenario/checkpoint owners. Then prove selector-8 closes both LP
accounts, returns exact historical rent to immutable refund owners and leaves
no obligation/reservation state. Objective resolution, redemption and
Market/root retirement cross to S2/S8; they must not become a Dealer-local
terminal truth.

### S6 — recurring Series

First run the uncommitted slot-2 positive transaction in
`series_pre_market_expiry_program_test.rs` against the diagnostic ELF set, then
the underfunded and caller-substitution hostiles. Rebuild all five exact-current
ELFs before accepting evidence.

In parallel, add the missing activation semantic owner:

- a real `CapabilityProgramV1` activation descriptor and producer in
  `programs/dclutch-trading-sbf/src/series/release_v5.rs`;
- an activation bundle following the General precedent;
- outer authentication in `outer.rs` without legacy/synthetic roots;
- kernel persistence and terminal conservation in
  `crates/dclutch-series-v3-kernel/src/{lib,shadow,terminal}.rs`.

The economic ruling is option B: Template-authenticated `closeRent` is prepaid
principal. Activation transfers exact root rent plus close principal, persists
the same principal, accepts harmless donation separately, and terminal Close
returns principal only to the named beneficiary. Zero/nonzero principal,
underfunding, overdeclared principal, substituted Template, refund redirection
and donation hostiles are mandatory.

Then execute at least two occurrences on a shared clock: prepare; issue/consume
one ticket; expire another; redistribute funding once; settle occurrence
children; restart from the append-only current acquisition ledger; close
terminal state; retire the Market. Operator, browser and simulator consume the
same current-acquisition proof and report executed/refused/not-attempted
stages.

Terminal gate: founding, activation, consume and expiry all cross current real
ELFs, replay cannot duplicate funding or occurrence settlement, and every
ticket/root/Rent child closes.

### S7 — curves, Structured and Fractional representation

First close the current 43/44 Rational corpus. The new physical test fails
before submission because `construct_chain_hot_issue_structured_v3` returns
`ChainOperator(InvalidToken)`. Compare the fixture's receipt Mint/account
observations to the bearer operator; repair the fixture if it is stale, or the
semantic owner if the checked TokenBehavior/release is genuinely
inexpressible. Never add a browser exception.

The nested physical review localized `InvalidToken` more narrowly than the
parent's first ATA hypothesis: `authenticate_mint` still pins
`mint.decimals == 0`, while Token Behavior V2 admits the full `u8` display
decimal domain and the fixture deliberately uses receipt decimal 19 and shard
decimals 6/255/9. Economics remain raw `u64` base units. Remove the obsolete
zero-only operator gate, add nonzero-decimal construction hostiles/parity, and
do not change the fixture decimals or weaken Token-2022 ownership/extension
authentication. If K=3 `IssueStructured` remains at its measured 1,357 bytes,
keep that browser act blocked; use a proven K=2 route or coordinate a genuine
account-frame compression rather than hiding the packet wall. Denominate and
Reconstitute fit at K=3.

The physical representation chain must prove:

- `IssueStructured` through Trading→Claims→Token-2022;
- actor-only signed `Denominate` and `Reconstitute` through `DCRRPRQ2`;
- exact Claims replay/aggregate/Position and Token-2022 shard Mint
  supply/owner balances;
- transfer plus a separate terminal redemption/retirement campaign where the
  external Custody ELF actually participates (the three open actions use a
  Claims-owned custody Position, not the Custody program);
- release/Mint/program/owner/order/alias/revision substitutions with rollback;
- packet, locks, CU and native/WASM byte parity.

Do not redo the degree-2/3 implementation described by the now-stale prose in
U-013. Current source already has the live-width exact de Boor evaluator,
cumulative-floor rounding, `DCLTPGT1` price gate, founding admission, wire
move, and physical cubic Claims work (`aac98afd` through `c87a9018`, with
`SPLINE_EVALUATOR_RELEASED_V3 = true`). The overnight brick is the physical
matrix: degree-2 `[1,4,2]` and degree-3 `[1,4,4,2]`, differential
exact-integer/overflow/price-gate hostiles, real Claims/Custody payout,
fractional compaction and claim-check close. Update stale comments/omission
prose beside that evidence; do not edit the kernel unless the physical gate
convicts it.

Next bind the existing compiler to a finalized Generic Found. Use the existing
`tools/fractional-exterior` and `FoundingBridgeV1`: consume the spline
compiler's exact files/report, run successor Found, and bind compiler,
DCLTGMF3 journal/intent/transaction, release/ELF, Product/Basis/price-gate,
Market/aggregate/Position and conservation digests. Do not reconstruct Product
facts or plant an unrelated Fractional identity after Found.

Finally propagate that same bridge through wrap, transfer to an unrelated
sleeping holder, whole-claim reconstitution, Source resolution, terminal
payout, permissionless compaction, partial/final burn, record/vault/escrow
close, ordered Fractional retirement and Core `Retired`. Use the public
retirement planner one act at a time and prove restart, no hidden remainder and
no absent-holder charge. Keep U-015 visible: acyclic representation
composition, the first lifted Token behavior profile, lifecycle refund closure
and measured contiguous/paged width remain accepted expansion unless Ember
rules otherwise.

### S8 — Claims, Custody, terminal payout and liveness

Finish the public claim-check exterior around `4306d389`/`e0ece22e` and the
recipient split at `2af02f53`. ProgramTest may warp beyond the real 180-day
deadline for positive evidence; local/devnet surfaces must preserve the real
clock, explain early refusal, and never shorten it. The complete campaign is
open claim check → early refusal → elapsed permissionless compaction → native
terminal payout → record/escrow/Position/admission/replay/vault close.

Add the missing conservative complete-set owner instead of exposing bare
`ClaimsAction::MintCompleteSet/MergeCompleteSet`:

- split atomically transfers `quantity × authenticated basis scale` from the
  actor's external collateral account to canonical HoardPrincipal and uniformly
  credits claims;
- merge uniformly debits claims before returning the same collateral class;
- one Claims-owned outer route authenticates Realm, Product/Basis, release,
  capacity, replay, owner, frame and poststate and couples Custody atomically.

Likely new owners are a Claims conservation contract/program/operator plus a
thin WASM planner, but exact paths must be claimed before edits. Decide whether
an existing replay is semantically identical or create a separately named
replay; never alias meanings.

In parallel, produce one asset-by-asset ledger across every family:
TradingPrincipal, HoardPrincipal, Source funding, fees, bounties, Rent,
donations and user collateral. Permissionless fee and maintenance work must be
funded by explicit present sources. Future revenue is never liveness
capitalization; Hoard principal never crosses classes. When opener/upkeep/
donation policy becomes the sole wall, present Ember one adversarial decision
packet and continue other lanes.

Terminal gate: split/merge, transfer, representation, payout, compaction and
all closes execute with exact revisions/balances and rollback. No Position,
replay, vault, record, escrow or Rent principal is stranded.

### S9 — browser, SDK, CLI and operator

The browser lane begins only from typed physical evidence. Pure deterministic
Rust planners/codecs/checkers may compile to WASM; finalized RPC, Wallet
Standard, durable storage and submission remain explicit web-shell
capabilities. SDK is the semantic client owner and web imports/re-exports it;
hand-copied TypeScript wire truths are deleted only after generated parity is
green.

For every mutation use the same recovery protocol:

1. reacquire cluster, finalized state and blockhash;
2. construct the Rust/WASM-owned candidate;
3. persist unsigned intent, bytes digest and expected poststate;
4. request the exact wallet signature;
5. persist signed bytes/signature before one submission;
6. inspect signature status and exact finalized poststate;
7. clear only after proof; recovery never resubmits.

Immediate product journeys are Direct trade, Dealer liquidity, General
candidate work, Series tickets, Source resolution, complete-set split/merge,
Rational represent/reconstitute and terminal redemption. Each needs missing
storage, packet mutation, signer/signature, stale state, ambiguous submission,
reload and finalized-substitution tests.

`/console` is a generated directory/census, not the place every act must run.
Product journeys live with their users; `/operator` explains cold lifecycle,
release, provenance, recovery and unsigned exports. Each card says outcome
first, venue/authority second and one compact safety/recovery guarantee third.
No “awaiting production” category survives once its work is done; no status
flips merely because a route magic exists.

Likely convergence owners:

- SDK/web `capabilityModel.ts` and `operatorSurface.ts`, with SDK authoritative;
- `ConsoleDirectory.tsx`, `OperatorSurface.tsx` and operator field/runbook
  components;
- family workspaces and operation journals;
- generated WASM scripts/artifacts and SDK/web twin tests;
- operator commands under the local successor only after native owners exist.

Terminal gates include static build, native/WASM parity, SDK/CLI/browser byte
agreement, Wallet Standard recovery, relay/index absence, keyboard/mobile/
accessible states, directory-to-visible-control consistency and a cold
campaign transcript containing every documented command.

### S10 — load simulator, population, devnet and economics

Do not deepen the simulator by adding projected capabilities. Extend
`tools/load-simulator/simlife.py`, `simlife_drivers.py` and
`simlife_drive.py` only with operator exteriors that have real transaction and
poststate evidence.

The mixed population should include categorical Direct, registered Direct,
General N=2/N=258, two-LP Dealer, two-occurrence Series and one curved
Structured market on one shared clock. Vary two independent participants,
absent holders, competing permissionless keepers, provider delay/fallback,
wallet interruption, restart, stale/substituted packets, donation and fee
conditions. Emit per-market event timelines, shared clock, CU/fees/rent,
asset-class conservation and a stage strip that distinguishes executed,
chain-refused, locally refused, blocked and never attempted.

Use local validator for warp-dependent 180-day compaction evidence. Then, only
after the exact-current local release is coherent, run the authorized devnet
flight through `tools/release/devnet-flight`, `devnet_direct_lifecycle.py`,
Source/keeper scripts and checked activity observers. Devnet may be redeployed
and mutated for this task; record program/release/transaction identities and do
not describe it as mainnet or official without a checked release manifest.

The cold/redeploy output is one command plus a machine-readable lifecycle and
conservation ledger. Operator instructions may contain only commands the
campaign executed. If a wallet-owned act cannot run headlessly, the ledger
must record the explicit immutable handoff and verification boundary.

### S11 — generic execution, debt, privacy and assurance entry

This lane prevents “current campaigns finished” from shrinking the project.
It owns substantive debt and accepted expansion after the primary walls move:

- U-001: one generic admitted Trading descriptor path across Direct, General,
  Dealer and Series, and an explicit deletion/non-authoritative-AOT ruling for
  standalone family artifacts;
- U-014: interpreted versus stateless-AOT Direct equivalence, Registry-bound
  toolchain/artifact, refusal/rollback equivalence and CU/packet/rent
  comparison;
- U-015: typed acyclic representation DAG, lifted Token profile, complete
  producer-subtree refund closure and measured paged widths;
- systemic TypeScript DTO/capability duplication, stale schema-1 paths,
  unreachable commands and guides, dead gauntlet full mode, frame-delta
  ratchet, cold predecessor fixture and stale binary/version ambiguity;
- deletion of obsolete executables only after their successor is physically
  proven and referenced by the checked release.

This is not a grab bag for lint. A debt brick runs only if it removes a defect
class, restores one completion gate or eliminates a second semantic owner.
Mechanical cleanups wait or go to Terra.

C-15 requires one explicit Ember ruling: whether the accepted final public
project includes the original FHE/MPC/specialized batch/energy ambition. Do not
silently inherit an old horizon park. If retained, give it a real capability
charter; if ruled out, record the dated ruling and remove contradictory claims.
The retained charter starts with a fixed-topology leakage/failure plan—not a
backend survey—covering malicious input, inclusion/non-equivocation, encrypted
owner allocation, settlement commitment/note ledger, selective decryption, key
rotation, abort/recovery and proof/dispute. Only then compare FHE/MPC/vFHE
backends. Feasibility, conservation and any optimality certificate remain
separate predicates.

The assurance-entry crew starts only after the campaign waves. It independently
tries to disprove C-00..C-16, walks every current route/capability/reference,
replays cold commands, mutates authority/release/account/order/revision facts,
audits conservation and stale UX claims, and returns findings to owners.
“Formally verified” remains forbidden until a later phase names exact theorems,
digests, tools, assumptions and unverified adapter/runtime boundaries.

### Expected overnight checkpoint sequence

The intended commit rhythm—conditional on green evidence—was:

1. preserve current family WIP digests and land no red campaign;
2. one narrow shared-infrastructure or artifact-auth repair;
3. separate physical prefix commits for Direct, General, Dealer, Series and
   Structured;
4. separate hostile/poststate commits extending each prefix;
5. shared Claims/Custody/Source terminal exteriors and conservation ledger;
6. WASM/SDK parity commits before any web mount;
7. wallet recovery components, then capability truth and `/console`;
8. simulator population drivers and exact-current release manifest;
9. local cold campaign, then conditional devnet rehearsal;
10. adversarial findings/fixes and an updated handoff.

If a lane stayed red, its checkpoint would instead be an uncommitted,
digest-pinned patch plus one exact continuation command. The swarm was never
supposed to convert unresolved programs into a morning of small TypeScript
polish.

### Overflow queue beyond the first ten hours

Ten hours was a work interval, not a scope boundary. The following remain
explicitly alive even if no capacity reached them:

- provider breadth beyond the first real Pyth profile;
- full GeneralDealer breadth and LP-loss/evolution ruling;
- generic interpreted/AOT execution equivalence;
- the representation DAG and lifted Token behavior profile;
- privacy/FHE/MPC/energy disposition and any retained implementation;
- sustainable opener/upkeep/donation economics;
- full static/mobile/accessibility closure;
- obsolete-path deletion, SBOM/licences and migration/release hardening;
- independent assurance-entry review.

Claude should never cite this overflow placement as deferral or a non-goal. It
is the continuation queue under the same stopping condition: implement it, or
obtain Ember's explicit ruling that the accepted project no longer wants it.

---

## Corrections to this letter — appended 2026-09-01, do not renumber above

This letter is authority, not scripture. Where re-measurement contradicts it,
the measurement wins and the contradiction is recorded here. **Corrections are
appended at the end on purpose**: several lanes are routed to sections of this
file by line number, so nothing above may be inserted or renumbered.

### "Dealer physical life" — three statements re-measured, none survived

Re-measured against this section's own pinned artifacts (Trading `af5d955e…`,
accelerator `3f73d43c…`, profiled ELF `a2e62944…`, `accepted.rs` at
`e1bac1e8…`, all re-verified before and after; the pinned tree was never
rebuilt and nothing was built from ambient HEAD).

1. **The pinned CU fingerprint is stale.** Same command, same artifacts: the
   wall costs **149,593 CU**, not 148,093. What moved is not yet known, and it
   is recorded as a corrected fingerprint rather than hedged.
2. **"A substituted-position selector-1 Add refuses correctly" is not
   established.** The hostile and the honest Add refuse at the *identical* CU
   with the *identical* `0x4003` — both hit the same site, upstream of anything
   that could examine a substituted identity. The control is vacuous and passes
   only because the honest path is broken in the same place. Nothing about
   substituted-position handling is currently known. Re-verify after the wall
   moves.
3. **The window is right; its implication is wrong.** All 19 direct raise sites
   in `hot_v3.rs:3222-3525` were instrumented with distinct custom codes and
   every one is excluded. No predicate written in that window refuses. **The
   wall is inside a helper called from the tranche.**

Refuted, so nobody re-chases it: this is **not** the manifest/derivation-policy
defect already convicted, which raises `UnsupportedContent` 0x4000 at
`hot_v3:3372`. This raises `Content` 0x4003.

**The method that makes correction 3 admissible, and the rule it earns.** The
first instrumentation used `sol_log_64` and produced nothing — and there were
zero `Program log` lines anywhere in the run, *including on the successful
path*. The channel was dead and its silence meant nothing. The negative result
only counts because a positive control ran beside it: an ungated marker fails
LP Open at `0x9063`/557,448 CU, proving the channel live and the window
reachable, and the same marker gated to `selected_action == 1` lets LP Open
succeed at 1,059,071 CU while the Add still refuses carrying no marker.

> **An absent signal is evidence only if something present proves the channel
> works.** "I instrumented it and nothing fired" and "my instrument was
> disconnected" produce identical logs. Every negative result in this tree
> needs a positive control in the same run, or it is not a result.

### Correction 3 above is itself WITHDRAWN — 2026-09-01, later the same day

Correction 3 said this wall was *not* the manifest/derivation-policy defect
already convicted, because the codes and sites differ. The codes and sites do
differ. **The predicate is the same one**, reached earlier on this path:

`descriptor.derivation_policy != entry.child_derivation_id()` in
`validate_selection` (`crates/dclutch-capability-program-contract/src/v4.rs`),
reached from `authenticate_descriptor_root_selection` at `hot_v3.rs:3319`,
which discards the reason with `.is_err()` and re-raises a bare `Content`.

Convicted by gated early-return probes on the pinned evidence, then a two-branch
conjunct split (`0x903c`), then a seven-way predicate split (`0x904c`). One
predicate, not a range. **That `.is_err()` discard is the finding behind the
finding: it made one defect look like two and cost a full bisect.**

Corrections 1 and 2 stand — both were re-measured today.

**Instrumenting this route:** `authenticate_and_execute_hot_v3` has **zero SBF
frame headroom.** Fifteen per-call-site `.map_err` guards produced 95
frame-overwrite diagnostics and an abort (`ProgramFailedToComplete`) rather
than a refusal; eight out-of-line guards still produced 95; the clean tree
measures 0. Use gated early-return probes only, and check `FRAMES` before
believing any measurement from this function.

### The overflow queue's first item is factually wrong — 2026-09-01

*"provider breadth beyond the first real Pyth profile"* is written as though the
tree had one profile. **It has four declared and three executing on real ELFs,
across two evidence families**, measured rather than remembered:

| campaign | result | what it is |
|---|---|---|
| `resolution_core_v3_lifecycle` | **5/5** | Pyth terminal, one transaction |
| `sponsored_push_lifecycle` | **1/1** | Pyth sponsored push — 592-byte release vs 440, **no router, no VAA** |
| `relayed_mainnet_state` | **19/19** | relayed observation record — a *different family*, own real ed25519 quorum, and the one the gauntlet runs |

The fourth, `SharedObservationChild`, has no implementation anywhere: it shares
Pyth's extension id and nothing switches on it. **An unimplemented cardinality
variant, not a family.**

**And the architectural ruling already exists**, at
`crates/dclutch-source-contract/src/lib.rs:268-283`, about the relayed
extension: *"it exists here, beside Pyth's, because the one canonical
Source-material encoder has to be able to admit a second family without becoming
family-agnostic: **the closed set is the point**."* Families are added to an
enumerated set by decision, never registered into an open one. That forecloses
the open-registry answer.

So the open question is **not** whether the architecture can take breadth — it
demonstrably has, twice — but whether C-09 wants a **third family** at roughly
13,000 lines gated on unverified Switchboard economics, or whether breadth is
already satisfied and the remaining work is the **generic-header refactor**.
Ember's, and on the ruling list.

**Costed from the tree's own two precedents**: a new *profile* inside an
existing family ran ~4,940 lines; a new *evidence family* ran ~12,900. Neither
is cheap and Switchboard is the second kind.

### This letter's own `general-hot` command carries a trap — 2026-09-01

The `general-hot` invocation above sets **`CARGO_TARGET_DIR` to the root
workspace's target directory.** `programs/dclutch-general-accelerator-sbf/
program-test` is **its own workspace**, so that override mixes rustc invocations
made from two workspace roots. One path-dependency crate then compiles twice —
once with a relative source path, once absolute — and the link fails with:

```
multiple different versions of crate `dclutch_core_contract`
```

**blaming `dclutch-operator` and `dclutch-direct-codec`, which nobody has
touched.** A lane inheriting this command from the handoff will lose hours to a
manifest problem that does not exist. Drop the override and the same workspace
builds in **nineteen seconds**.

**`cargo metadata` is the discriminator between the two explanations.** If it
resolves **one** copy of the crate at a canonical absolute path — as it does
here, with uniform spellings and no `..` oddities — **the manifest is innocent
and the target directory is the culprit.** Every remaining duplicate in that
dump is a genuine semver-incompatible registry crate (`ark-ff` 0.4/0.5, `sha2`
0.9/0.10/0.11) and is normal.

`cargo update --workspace` never clears it, because there is nothing in the
dependency graph to clear.
