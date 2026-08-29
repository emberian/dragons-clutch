# Protocol capability and flow-test frontier — 2026-08-29

Evidence class: offline source-contract and checked-in evidence analysis at
commit `6d9d4b362cc14039dfce6dc6b11d7b7e17e98573`. No build, validator, RPC,
wallet, key, deployment, or submission was used for this analysis. A devnet
statement below means only what the checked release/evidence records establish;
it is not mainnet evidence.

## Result

The repository is not one uniformly callable protocol. It currently contains
four distinct strata:

1. durable private exteriors for founding, participant admission, Direct,
   Resolution, wallet payout, source abort, and retirement;
2. complete public-wallet submission for redemption only;
3. public-wallet signing and packet export, but deliberately no submission,
   for Dealer equity and release activation;
4. constructor, projection, or real-SBF component artifacts without a durable
   capability caller for Series Shadow, the Series Hot plan, General
   submission, Product Runtime V2 admission, Dealer Accepted completion, and
   the funded failure walk.

The seven devnet programs are a previous-generation role deployment. Registry
and Rent are installed, and the five execution roles Core, Claims, Trading,
Resolution, and Custody were activated. That does not activate current-source
Dealer, Series, or General links, and it does not prove a market selected any
of those families. The current Upgrade/release/fresh-Market cycle is still
pending. The prior devnet campaign published a Realm and reached real founding
prestates, but no Market reached `Open`; therefore admission, trading,
resolution, Claims payout, redemption, and retirement have no market-level
devnet activation.

There is no current-source, all-stage economic system execution. The PRIVATE
lifecycle preflight is an offline reachability and expected-execution model and
states that its actual probe still stops before validator launch. There is also
no concurrent runtime campaign. The twenty-seed multiwallet oracle includes
simultaneous candidates and shared nonces, but deterministically orders and
applies them in one offline model; it is not parallel Solana transaction
execution.

## Classification vocabulary

- **Constructor-only** means the tree can encode, derive, or compile the
  action but has no non-test durable caller that owns observation, journaling,
  signing, submission, and finalized poststate. A browser preview or packet
  download is still constructor-only at the mutation boundary.
- **Caller-backed** means a non-test executable exterior consumes the artifact.
  `partial` means the caller reaches a preparation/checkpoint step but cannot
  finish the named capability.
- **Private-runner-backed** distinguishes real-ELF ProgramTest or an
  owned-loopback/local-validator command from a public client. ProgramTest is
  named explicitly; it is not validator evidence.
- **Public-wallet-backed** is `submit` only when a browser Wallet Standard path
  both signs and submits with durable recovery. `sign/export` is a different,
  incomplete boundary.
- **Devnet-activated** distinguishes role deployment/Registry activation,
  record publication, and Market-selected capability activation. A deployed
  role is not an Open Market.
- **Economic system test** means an executable multi-stage run reconciles the
  exact integer ledger across its protocol boundaries. A pure model or an
  isolated component conservation check is named separately.
- **Concurrent system test** means two or more real transactions contend in a
  runtime. Sorting “simultaneous” model actions is not such a test.

## Capability matrix

| Surface | Constructor-only? | Caller-backed | Private runner | Public wallet | Devnet activation | Economic / concurrent system evidence |
| --- | --- | --- | --- | --- | --- | --- |
| Realm | no | yes: the successor Market producer constructs `RealmV1` and publishes the Registry record consumed by founding/admission/Direct/payout readers | yes, inside the Market/founding runner | no; public clients only inspect | record publication **yes** in the previous devnet lineage; no standalone Realm program and no Open Market | no economic transition; no concurrency test |
| Founding | no | yes: the successor runner and `dclutch found` own durable founding; the CLI also chains first participant admission | yes, local-validator campaign; focused real-SBF founding components | no: Create Market is read-only and `/found` exports a legacy incomplete unsigned Found37 packet | previous generation reached Realm, Rent, Found31, and projected-Custody prestate, then `TooManyAccountLocks`; no Open Market. Current DCLTGMF2/GMF3 source is not activated | component/local founding execution exists; current-source whole economic run **no**; concurrency **no** |
| Participant admission | no | yes: `plan_user_position_admission_v1` is consumed by the durable successor admission exterior and by `dclutch found` | yes, owned-loopback command and focused ProgramTest | no; web authenticates the participant state but does not build/sign/submit its caller transaction | no Open Market, therefore no devnet admission | offline ledger predicts it; current all-stage observed economy **no**; duplicate-admission race **not tested** |
| Product Runtime V2 record admission | **yes at the non-test mutation boundary** | no durable submitter: Rust builds an unsigned instruction and Product Studio only composes its 9-account request | native ProgramTest executes the operator-built instruction; the real-ELF variant is checked in as ignored and requires external SBF output | no: compose only, no RPC/sign/submit | no | isolated admission/rollback simulation only; no economic or concurrent execution |
| Direct | no privately; **yes publicly at execution** | private yes: durable produce/execute/payout-schedule commands. Public CLI `buy`/`sell` intentionally refuse because the exact packet journal, ack, and ten finalized poststates are not yet owned | yes, real-SBF Direct campaigns and owned-loopback successor commands | no execution: browser is read-only; CLI can sign an offchain intent only | old Trading role is activated, but no Open Direct Market and current source is not installed | real-SBF component effects and exact offline multiwallet economy exist; no current all-stage observed economy; shared-nonce cases are model-only, not runtime concurrency |
| Dealer equity selectors 1–6 | **yes at durable submission** | no durable submitter; the browser constructs, signs, and exports the packet only | real-ELF Dealer refusal campaign reaches the family program but explicitly proves no accepted transition | `sign/export` only; the workbench says nothing is submitted; LP/scenario flows are hidden | no Dealer program in the seven-program devnet release and no Market selection | no accepted economic execution and no concurrent Dealer campaign |
| Dealer scenario / Accepted | **yes** | none: Trading dispatches create/page/evaluate/reserve/rollback/cleanup and the operator builds packet/journal values, but every builder is referenced only in its own module/tests; Custody reservation producer and final 41-lock Accepted executor do not exist | component only: fresh real-SBF build/frame evidence and codec/operator tests; unresolved aggregate ProgramTest dependency prevents the full route | no scenario surface; hidden in the workbench | no Dealer/accelerator program in the seven-program devnet release and no Market selection | no Accepted economic execution and no concurrent Dealer campaign |
| Series occurrence / Core consume | no for the Core action, but no production exterior | test-only caller: tier 4 invokes Core five times through `series-consume-caller`, explicitly not Trading | ProgramTest yes; it fabricates Loader-v3 state, so not validator evidence | no | no Series program or Market selection on devnet | one-shot/replay/rollback component evidence only; no economic journey or concurrency |
| Series Hot plan | **yes** | none outside `crates/dclutch-operator/src/series_hot_v3.rs` and its tests for `build_series_prepare_hot_v3`, `build_series_consume_hot_v3`, and `build_series_expire_hot_v3` | no | no | no | none |
| Series Shadow accelerator | **yes** | no real Trading caller; the nested harness exposes loader, selected-build, route-order, and rollback support only | real selected ELF/build evidence, but no caller-backed action execution | no | no | no action CU, economic journey, or concurrency evidence |
| General | no at plan/component boundary; **yes at durable submission** | partial: `build_general_hot_instruction_v3` has a real ProgramTest caller, V5 wraps it, and web consumes/exports the plan; no durable submit/finalize exterior | real-ELF ProgramTest executes all seven actions at N=1 and N=258; not local-validator evidence | no: browser exports an unsigned packet and never signs/submits | no General/accelerator program in the seven-program devnet release and no Market selection | component action execution and rollback yes; no end-to-end economy or concurrent submissions |
| Resolution | no | yes privately: successor terminal commands drive submit, provider execution, Core acceptance, reclaim, and direct close | yes: journey/ProgramTest and durable owned-loopback exterior | no mutation; SDK/browser read certificates and terminal state | previous Resolution role activated; no current Open Market/certificate, and no current-source lifecycle | real-SBF component/journey evidence exists; current PRIVATE Pyth/Resolution runtime remains owed; no concurrent candidate campaign |
| Oracle / provider transport | no | yes privately: Pyth VAA provisioning, Pyth provider, relay, and sponsored-push producers feed Resolution | yes, private journey/relayer tools and component campaigns | no wallet action | Pyth synthetic release record was published/read and old Resolution was activated; no open market observation was accepted | no all-stage observed economic run; provider-success/failure exist in model/component evidence; no competing-update race |
| Claims | no | yes through founding/admission/Direct/resolution/payout/retirement child calls; no standalone public Claims administrator | extensive real-SBF component campaigns and private successor flows | indirect through public redemption replay/payout | Claims role activated; no Open Market aggregate/terminal Claims state | component conservation/rollback and offline full ledger yes; current all-stage observed economy no; payout replay concurrency no |
| Redemption | no | yes: CLI and browser each own Claims-Custody replay followed by terminal payout, durable unsigned/submitted journals, one-send recovery, and finalized poststates | yes: wallet-terminal payout producer/exterior; current PRIVATE runtime execution still owed by its preflight record | **submit**: `RedeemFlow` signs and submits both steps with Wallet Standard | code is ready, but no terminal devnet Market exists, so not exercised | exact offline positive and zero-payout ledger plus component tests; no current whole observed run and no same-Position payout race |
| Retirement | no | yes privately: checkpoint operator plus terminal-sequence/aggregate-retirement exterior owns the four packet suffix | focused real-SBF Core/Claims/Custody/Rent campaign executes all four transactions; private validator completion is still owed | no; web status is read-only | no terminal devnet Market | isolated exact rent/refund conservation **yes**; current all-stage economy **no**; late-payout/retirement contention **not tested** |
| Recovery: staged founding abort | no | yes: `source-abort-v1` persists and advances the DCLTPCA1 -> DCLTCF1A -> DCLTCF2A suffix | private/devnet-capable successor exterior | no | previous lineage records an abort-lane Market as staged and unwound; current DCLTPCB2 exterior is source, not current devnet release evidence | suffix conservation checks exist; no concurrent open-versus-abort runtime test |
| Recovery: funded failure walk | **yes at submission** | preview only: SDK builds it and `dclutch walk` requires `--dry-run`; CLI explicitly waits for durable Submitted/finalized certificate+bounty ownership | Resolution components exist, but no caller-complete walk exterior was found | no | no funded terminal Market and no submitted walk | model/refusal tests only; no bounty economic or deadline race execution |
| Recovery: client crash/restart | no single protocol capability | yes per founding, admission, Direct-private, redemption, payout, abort, retirement, and upgrade exteriors; not yet common/conformance-owned | yes | redemption only; Dealer/release export transfers recovery responsibility outside the app | not a Market activation property | strong per-flow journal tests, but no cross-flow upgrade/interleaving system test |
| Release | no privately; **yes publicly at submission** | private successor release/activation/upgrade runners exist. Web reacquires the release, wallet-signs one role at a time, then exports bytes | yes, checked-release/private mutable runner | `sign/export` only; Release Workspace has no submit path and does not update programs | **yes for the old release**: seven programs deployed, five execution roles activated. Current 13-link checked source upgrade/activation is pending | release crash/model checks exist; no economic test; no concurrent activate/upgrade interleaving campaign |
| Public SDK as a whole | not purely; read/decode is broad, mutation completeness is narrow | caller-complete public mutation only for redemption. Failure walk/Direct remain refused; Dealer/General/release expose partial subpaths | local successor conformance is read/verification support, not a public runner | redemption `submit`; Dealer/release `sign/export`; otherwise inspect/preview | package describes devnet state but does not make a capability active | ABI/unit/model evidence only at package boundary; no SDK-wide journey or concurrency campaign |

The SDK package is named `@dclutch/sdk` but currently declares
`"private": true`. Its root comment is the correct governing rule: a packet
constructor is not public until one caller owns the durable journal,
acknowledgement, and finalized poststate. The wildcard subpath export makes
many `lib/*.ts` modules importable even when they are not re-exported at the
root, while `directTransaction`, `directCodec`, and registered Direct packet
paths are explicitly nulled. Package reachability must therefore be tested as
a real API boundary rather than inferred from `index.ts` alone.

## Executable producer/caller trace

| Durable fact or transition | Producer | Executable consumer/caller | Missing edge |
| --- | --- | --- | --- |
| Realm record | `tools/local-validator/bootstrap/successor/src/market.rs` constructs `RealmV1` and publishes it through Registry | successor founding; admission, Direct token setup, and wallet-terminal readers | public Realm creator absent by design |
| Open Market and founder admission | generic founding operators plus successor Market campaign | `local-private-validator-market-v1`, founding campaign, and `dclutch found` | no public wallet founding; no current devnet success |
| Participant Position/admission | `crates/dclutch-operator/src/user_position_admission_v1.rs` | successor `user_position_admission.rs`; `dclutch found` first-participant step | public wallet caller absent |
| Direct packet/poststate | Direct operator/transaction compiler and successor producer | private durable Direct executor | public CLI/web durable packet/ack/poststate owner absent |
| Dealer Accepted state | Trading checkpoint codecs/operators | no non-test durable caller; SBF dispatch and module-local tests exist | **artifact without caller**, release-authenticated Custody reservations, and final Accepted commit absent |
| Series occurrence | Core Series consume | test-only `series-consume-caller` | projected Trading production caller absent |
| Series Hot prepare/consume/expire | `series_hot_v3` operator builders | none outside same module/tests | **artifact without caller** |
| Series Shadow evaluation | selected Series Shadow ELF and nested loader harness | none reaches evaluator through real Trading | **artifact without caller** |
| General successor plan | General operator V3/V5 | ProgramTest caller; web hostile decoder/export | durable sign/submit/finalize exterior absent |
| Product Runtime V2 admission | Rust operator and Product Studio request composer | native ProgramTest submits it; no non-test durable caller | **artifact without production caller** |
| Resolution terminal certificate | Resolution operators and successor Pyth/relay/flagship commands | private runner; public SDK reads certificate | public resolution submitter absent; current full private run owed |
| Terminal payout | Claims replay + wallet-terminal builders | CLI `redeem`, browser `RedeemFlow`, private payout exterior | devnet terminal Market absent |
| Aggregate retirement | checkpoint retirement operator | focused real-SBF campaign and private aggregate-retirement exterior | public retirement exterior absent; full private run owed |
| Staged source abort | source-abort planner | durable successor `source-abort-v1` | public caller absent |
| Funded failure walk | SDK `failureWalk` builder | CLI dry-run only | **artifact without submission caller** |
| Release activation | release registry planner | private release runner; browser sign/export | public submit/finalize owner absent |

## Flow-test frontier before GMF3 all-link work

These gates are host/static/model work or focused existing-link work. None
requires the expensive all-link build. The order prioritizes false capability
claims and cross-stage seams before compute measurement.

### P0 — machine-readable capability reachability

Add one source-owned manifest with a row per mutating action, not merely per
program. Each row should name:

- constructor symbol and ABI/version;
- program dispatch route;
- non-test caller command/component;
- journal and terminal evidence schema;
- private/public-wallet exposure;
- release role/link and Market selector;
- current execution evidence class.

A static test should refuse an exported mutating constructor with no caller or
an executable command hidden from accepted help. Seed the hostile cases with
the Series three-builder orphan, Product Runtime V2 admission, Series Shadow,
General's missing submission edge, Dealer's missing final commit, and failure
walk submission. The existing PRIVATE preflight already proves dispatch/help
reachability for its fourteen commands; generalize that pattern instead of
creating a parallel list.

### P0 — release deployment is not capability activation

Add a pure release/capability state classifier with exhaustive states:

```text
Absent -> Installed -> RegistryActivated -> MarketSelected -> Executed
```

Require source digest and activation evidence at every transition. A test must
refuse to label Dealer, Series, or General “devnet” merely because the SDK has
an ABI or the current checked release enumerates 13 links. It must likewise
report the old seven-role release honestly without projecting it onto current
source. This gate is cheap and prevents the most damaging public-truth error.

### P0 — cross-flow durable journal conformance

Extract a test contract, not a second implementation, for the common states:

```text
Observed -> Planned/fsynced -> Signed+Submitted/fsynced -> Finalized+poststate
```

Run the existing founding, admission, Direct, payout, abort, retirement, and
upgrade journal fixtures through it. Required hostile cases: ambiguous send
never resubmits; expired unsigned packet may be archived/rebuilt; submitted
packet may not be discarded; release/Market/generation/account scope cannot
change; poststate must be read at or after the confirmed slot. This is a
high-value pre-GMF3 architecture test because every later journey depends on
the same crash semantics.

### P0 — account-lock and packet census from constructors

Compile host-side v0 messages with two ComputeBudget instructions and count
resolved unique transaction locks, not instruction metas. Cover one maximum
fixture for:

- current founding plan;
- participant admission;
- Direct;
- Dealer create/page/evaluate/reserve/rollback/cleanup and projected commit;
- Series prepare/consume/expire;
- all seven General actions;
- Claims replay and payout;
- each aggregate-retirement suffix;
- staged abort and funded failure walk;
- release activation.

Require `packet <= 1232` and `locks <= 64`. Existing General, retirement,
Direct, founding, and Dealer measurements can become fixtures. This catches
transport impossibility before an all-link build without pretending to measure
SBF frame or compute.

### P1 — producer-to-consumer poststate graph

For each journey, feed the prior stage's decoded poststate into the next
production constructor in memory. Do not permit hand-planted intermediate
accounts. At minimum exercise:

```text
Realm -> founding -> admission -> Direct -> Resolution -> payout -> retirement
```

and the sibling fronts:

```text
Dealer prepare -> reservations -> commit/rollback
Series open -> prepare -> consume/expire -> close
General batch -> orders -> best valid submitted candidate -> settlement -> close
```

The test should fail today at the honest missing edge, not fabricate a green
journey. This pulls the Series caller, Dealer reservation producer, and General
durable exterior gaps into executable architecture.

### P1 — exact economy against observed component snapshots

Keep `tools/economic-lifecycle-ledger/ledger.py` as the one economic arithmetic
owner. Have focused real-SBF or private-runner stages emit its existing observed
snapshot schema after admission, each Direct group, resolution, payout groups,
and retirement. Check all twenty named seeds at the component boundary before
the full validator campaign. Include zero-payout losing burns, the 50-bps
gross-199/gross-200 floor, exact fee destination, Hoard principal conservation,
and the five retirement refund classes.

This is still not a whole-system result until one runner observes all stages,
but it converts the current detached oracle into a reusable differential gate.

### P1 — transaction contention model, then focused runtime races

First derive a writable-account conflict matrix from the exact compiled
messages. Then add small focused real-runtime campaigns for:

- two Direct candidates sharing a maker nonce: one commit, one stale refusal;
- two admissions for one owner/Market/generation: one creates, one observes or
  refuses without double funding;
- two payouts for one Position coordinate: one burn, one replay refusal;
- payouts for distinct Positions: both may complete without a shared mutable
  client-side journal;
- late payout racing retirement: retirement cannot pass until every liability
  is zero;
- two Resolution candidates for one terminal sequence: only the best valid
  submitted candidate accepted by the named selection rule may advance;
- source abort racing Open: the state partition admits exactly one branch;
- release activation/upgrade racing a planned user packet: stale release
  binding refuses before semantic mutation.

The conflict matrix is architecture evidence. Only the follow-on runtime runs
may be called concurrent system tests.

### P2 — Rust/report/client differential gates

For General, Dealer, Series, founding, failure walk, payout, retirement, and
release, serialize production Rust operator reports and feed those exact bytes
to the SDK/browser hostile decoders. Require canonical re-encoding or an exact
named refusal. Series currently lacks the public caller/decoder completeness
of the better-covered families; add it only with the production caller, not as
another decorative ABI.

Add a package-boundary test that enumerates `package.json` exports and the root
exports. It should prove that intentionally private Direct packet modules
remain unreachable, that wildcard subpaths do not silently convert a preview
into a supported mutation API, and that every advertised public mutation has
the same-package durable caller contract.

### P2 — upgrade/resume interleaving

Use the current upgrade dry-plan and existing operation journals to model a
release change between `Planned`, `Submitted`, and `Finalized`. A submitted
signature is polled against its original release; it is never rebuilt merely
because activation moved. An unsigned plan may be rebuilt only after explicit
archive and fresh reauthentication. Apply the same test vector to founding,
admission, Direct, redemption, retirement, source abort, and release itself.

## Evidence anchors

- `docs/evidence/DEVNET_ITERATION_2.md` — exact previous seven-program
  deployment and pending current release/Market lifecycle.
- `docs/evidence/DEPLOY_1.md` — previous devnet Realm/founding prestates and
  explicit no-Open-Market result.
- `docs/evidence/PRIVATE_LIFECYCLE_OFFLINE_EXECUTION_MODEL_2026_08_28.md` —
  current private call graph and runtime evidence still owed.
- `tools/economic-lifecycle-ledger/README.md` — exact offline economic and
  twenty-seed simultaneous-candidate model boundary.
- `docs/evidence/DEALER_ACCEPT_SPLIT_TOPOLOGY_2026_08_28.md` — executable
  checkpoint primitives and missing Custody/final Accepted executor.
- `docs/evidence/GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md` and
  `docs/evidence/GENERAL_ALT_PACKET_WITNESS_2026_08_27.md` — real-ELF action
  execution versus production packet-construction boundary.
- `tools/gauntlet/tier4/README.md`,
  `docs/evidence/SERIES_ADAPTER_CORE_SEAM_2026_08_27.md`, and
  `docs/evidence/SERIES_SHADOW_SHA_ADAPTER_2026_08_28.md` — test-only Core
  occurrence caller and Series artifact-without-caller boundaries.
- `docs/evidence/AGGREGATE_RETIREMENT_CHECKPOINT_SPLIT_2026_08_28.md` — focused
  four-transaction real-SBF retirement conservation.
- `packages/dclutch-sdk/index.ts` and `packages/dclutch-sdk/package.json` —
  read-first public contract and actual package export boundary.
- `packages/dclutch-cli/src/commands/trade.ts`, `redeem.ts`, and `walk.ts` —
  refused Direct submission, caller-complete redemption, and dry-run-only
  failure walk.
- `apps/dclutch-web/components/DealerLiquidityWorkspace.tsx`,
  `GeneralWorkspace.tsx`, `ReleaseWorkspace.tsx`, `RedeemFlow.tsx`, and
  `CreateMarketWizard.tsx` — the exact browser sign/export/submit/read-only
  boundaries.
