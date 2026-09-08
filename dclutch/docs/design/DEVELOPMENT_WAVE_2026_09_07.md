# Development wave — 2026-09-07

Owner: CODEX-INTEGRATION. Active work plan, not release evidence.

Ember's current request is archaeology followed by implementation, complete
local-validator protocol simulation, a fresh full devnet cohort, and an updated
GitHub Pages site/explorer with a running population of markets and participants.
The completion scope remains `docs/MASTER_COMPLETION_CONTRACT.md`; this plan
orders the work and does not close or defer any of its rows.

The intended product is a stranger-operable compiler and market kernel: express
an objective bounded-state payoff, fund it, exchange fully backed claims, resolve
from the committed observation policy, receive tokens in an ordinary wallet,
and recover or close every temporary resource. Venue families are capabilities.
The optional simulator makes this behavior visible; it must not become an
operator whose continued presence is required for holders to get paid.

## What the first inspection changed

- The publication relationship is exact-tree content sync into `dclutch/` in
  `/Users/ember/dev/dragons-clutch`, through `tools/cut.sh`. It does not merge
  development history or run `git subtree`.
- The handoff's suggestion that only assurance remains is too strong. Its own
  queue and the completion contract still name missing producers and user
  entrances. Compile success and a route implementation do not close them.
- The command named for Claims conservation ran only compaction. The suite
  runner did include conservation, but stopped at the first failed target.
  Both now use one owner for target selection and execution results.
- The conservation campaign manually changed `Phase::Open` without consuming
  readiness, so its first real execution stopped at the codec. This is a
  component-fixture defect. The seeded transition is now explicitly labeled;
  it contributes no Core Open execution evidence.
- The scoring Dealer's accepted ELF case is a null fill. Its Claims and Custody
  windows contain placeholders deliberately skipped by zero-amount legs. A
  nonzero trade must execute those children before this family can be called
  usable (`program-test/scoring-dealer/tests/fill.rs`).
- Ensemble and Series still have missing producers identified in the handoff.
  The pre-existing dirty ensemble test expansion is preserved; it contains
  fixture support but no accepted fold test at intake.

## Current execution findings

- The exact execution census is recorded in
  `docs/evidence/EXECUTION_COVERAGE_CENSUS_2026_09_08.md`: 164 routes and 456
  refusal codes, with five checked local-validator route observations and 46
  historical devnet routes (50 in their union). The 123 historical successful
  binding claims are a different quantity: 73 lack checked native evidence in
  that corpus, and 41 routes have no successful claim. The report assigns the
  missing routes to owners and names the existing census path for final-source
  replay. None of these historical totals closes current-source coverage.
- Claims' six named ProgramTest targets have executed, including backed round
  trips and exact hostile rollback. The dated authority is
  `docs/evidence/CLAIMS_CAMPAIGNS_2026_09_07.md`; this does not substitute for
  local-validator founding or full protocol route coverage.
- Two further strict eight-program checkpoints are sealed: `687431e0e` for
  Dealer/Ensemble and `68153eefc` for the General repair closure. Both passed
  fresh links, frame measurements and artifact checks; exact source and gate
  hashes live in `docs/evidence/STRICT_ALL_EIGHT_CHECKPOINTS_2026_09_08.md`.
  These are diagnostic builds. No final-source frame pair, selected Series
  accelerator execution or new devnet cohort is claimed. Publication cuts
  carry development work.
- Direct's validator journey has executed founding/Open, admission, nonzero
  fills and fees, objective Pyth resolution, and wallet payouts. The retirement
  rerun invokes the actual upkeep producer before closing maker roots. The
  preserved-runtime diagnostic now completes checkpointed retirement and
  verifies all five terminal accounts absent, with exact refunds and fees.
  Its authority is `docs/evidence/JOURNEY_RUNTIME567_DIAGNOSTIC_TAIL_2026_09_07.md`;
  this must be repeated against the final fresh runtime.
- Custody economics has an accepted real local-validator campaign: governed
  parameters, Upkeep Found, and a nonzero Deposit Credit, with exact hostile
  and duplicate refusals. Hoard principal moved zero. Its first dated authority is
  `docs/evidence/ECONOMICS_CUSTODY_LOCAL_VALIDATOR_2026_09_07.md`.
  A second 15-transaction validator campaign executes Propose, Apply and
  Withdraw, including exact hostile rollback. Apply follows a durably
  finalized, controlled local clock advance, not elapsed devnet time; see
  `docs/evidence/ECONOMICS_CUSTODY_GOVERNANCE_LOCAL_VALIDATOR_2026_09_07.md`.
- Series' shipped and profiled Trading targets each pass seven cases, including
  two accepted expiry variants and four exact caller refusals. They reach projected Custody cleanup,
  Core precommit, and Trading replay poststates. The committed repair separates
  future-Market bump derivation, rent credit, typed projected wire, and readonly
  replay observations. Explicit immutable-record construction now binds the
  ordinary Market compiler's actual Product, Realm, Source and manifest.
  Typed child receipt/Claims production and the selected-release assembler
  are implemented. The connected compiler now executes a native accepted
  control producing 39 records, with root-normalization and bad-Ticket checks.
  That fixture does not prove actual parent-manifest or physical account geometry.
  Prepare's 111 logical account coordinates now have typed finalized,
  canonical-record and predicted-layout sources, with physical alias checks.
  Its compiler consumes the observed geometry and checks exact publication
  bodies. The missing certificate producer now authors the existing semantic
  certificate wire before strategy/descriptor compilation; the stale claim
  that this needs a new certificate binding has been removed. A preselection
  producer now resolves the certificate ordering cycle. The selected-build
  runner reconstructs the complete generated include from the pinned source
  manifest before SBF compilation. The live input constructor now derives
  Basis, Trading-owned Ticket and principal-cap facts from their owners.
  Consume geometry now follows the native Claims V6 frame (33 accounts) and
  Core Open frame (39 accounts): 164 fixed logical coordinates plus the ordered
  FundingState insertion. The former 161-coordinate shape was stale. The host
  distinguishes predicted Prepare state, observed Prepare state, and accounts
  created during Consume; the last category starts with zero data width.
  Full constructor controls are exposing producer defects, including a Claims
  count hardcoded to one instead of the M0 Portfolio's coefficient count.
  Core and Claims child CPIs now narrow the physical account union to native
  permissions, and projected Custody has executable Lock/Realize receipt
  contracts. Native Core receipt controls bind the Found V2 funding list and
  predict Open's future replay state independently of the verifier.

  A source review on September 8 identified a larger recurrence defect:
  the selected five-action ProgramSet dispatches by action only, while Prepare
  and Consume embed occurrence-zero child requests. Normalizing the parent
  root does not normalize the Market, generation, economics or replay revision.
  The test named for two occurrences compiles the first occurrence twice with
  different roots; it does not execute the second. The repair must parameterize
  child requests from authenticated Occurrence, Ticket, root and native adapter
  facts under the same immutable selection. Commit `7ce009694` implements
  native-derived private banks through the existing VM request writes and
  moves their semantic construction into the Trading adapter. The replacement
  native test retains the same five-action publication through Prepare and
  Consume for occurrence zero, Prepare and Expire for occurrence one, both
  terminal Ticket retirements and Root close. It compares complete Root and
  Ticket encodings; this exposed and repairs Prepare's formerly missing Root
  writes. Astra owns this coupled change through runtime integration, with Sol
  owning AccountInfo authentication, bank seeding and selected evaluator
  geometry. The isolated exact-source native repetition passes, with independent
  source inventory verification, recorded in
  `docs/evidence/SERIES_NATIVE_RECURRENCE_2026_09_08.md`. Actual child CPI and
  rollback, heterogeneous occurrence geometry, selected SBF frames and two
  validator occurrences remain owed. Native poststate agreement does not close
  them.
- General's accepted nonempty path is being run after repairing action-scoped
  lifecycle funding and the semantic outer frame / key-sorted Claims child
  boundary. Its next repair authenticates the actual Core, Realm, Claims
  Position, canonical RentCredit and token-source observations before projecting
  owners into the General bank. The local Market compiler now accepts checked
  accelerator deployment evidence. Fresh loopback infrastructure publication,
  succession and all five activation stages execute; subsequent collateral
  creation executes, but Market record publication exposes an incomplete
  sponsor projection. The incomplete new publication-cost mirror is removed;
  the local test resumes with the existing validator launcher's explicit fixture
  payer funding. That fresh validator run now completes founding in 559
  transactions and opens the General Market. The sole fixture grant went to
  the campaign payer; protocol-created roles received their funding through
  protocol transactions. It is test setup, not a protocol cost estimate or a
  devnet funding policy. The dated authority is
  `docs/evidence/GENERAL_LOOPBACK_FOUNDING_ADDENDUM_2026_09_08.md`.
  The preserved local validator also accepts OpenBatch after its producer uses
  the semantic Product identity for occurrence and Batch derivation; see
  `docs/evidence/GENERAL_OPENBATCH_PRODUCT_ID_LOOPBACK_2026_09_08.md`.
  Separately, serial General PlaceOrder ProgramTests reproduced an allocation
  failure on both the old and fresh `68153eefc` Trading ELF. Deduplicating CPI
  backing accounts exposed a compute failure. Commit `d70575d62` reuses the
  authenticated funded seal's immutable profile join, with same-wrapper and
  same-policy controls, instead of validating that join a second time.
  Subsequent runs reach the next allocation boundary within the compute limit.
  The canonical General heap profile, allocator and request now agree at
  128 KiB. Serial execution then reaches the actual 1.4 million CU ceiling:
  the compiler still selects chunked output, and each chunk recomputes the
  whole candidate. Caching admitted CPI sort keys saves 22,234 CU in the
  controlled diagnostic but does not make PlaceOrder finish. This is distinct
  from the inherited historical performance comparisons.

  One Sol owner now owns the complete General compiler, host and executable
  campaign integration with the existing OutputPage transport. The committed
  compiler selects one candidate invocation. Source review also found that the
  fixture's hashed output-page address had no real creation signer. The committed
  producer now constructs signed, caller-funded System creation and supplies the
  observed accelerator-owned page; native controls check ownership, width and
  rent. Runtime execution of that producer remains owed. Integration review
  also found the host session still counting callers with the old chunk
  classifier. That consumer now derives its geometry from the selected transport;
  its exact-source hbox control passes. Actual mixed-runtime execution confirms
  one accelerator invocation and accepted OpenBatch, but PlaceOrder still
  exhausts compute during effect projection. Measured repeated lifecycle scans
  are the next repair target; nonempty acceptance is still owed.
  The account is reusable scratch, and the accelerator currently has no close
  route, so rent recovery remains an explicit resource-lifecycle obligation.
  Accepted nonempty ProgramTest and local-validator poststates remain owed.
  Dealer's
  first RentCredit repair exposed a second false assumption: its sponsor need
  not be the immutable Market credit's refund beneficiary. Finalized account
  bytes located that exact mismatch; the repair retains the canonical credit
  and Core anchor. Correcting the external Custody authority's V2 request
  digest now reaches accepted Dealer Found and Quote. Fill next exposed a
  signed-delta position table whose host and runtime disagreed about semantic
  owner order versus Position PDA order. Both now use the Claims semantic
  owner order. The next runtime repair (`7d5f920a4`) binds Claims' signed-delta
  request identity to the actual Fill digest. The sealed runtime now completes
  a local-validator campaign using committed host `c04d95a0c`: Found, Quote,
  a nonzero Fill, and Withdraw. Read-back verifies 27 additional Hoard atoms,
  supplied by 9 taker atoms and 18 Dealer atoms, and the corresponding claim
  balances; the subsequent withdrawal transfers 3 atoms. The dated authority is
  `docs/evidence/DEALER_ACCEPTED_LOCAL_VALIDATOR_2026_09_08.md`. This closes the
  null-fill defect for that accepted route; final-source replay, remaining Dealer
  routes, and full retirement are still owed.
- Ensemble fold/reclaim has accepted real-ELF poststates. Its direct Pyth member
  producer is being executed separately so pre-captured fragment fixtures do
  not stand in for capture evidence. The two-member compiler and first-member
  real transport CLI reached fixed-three assumptions in host and pre-market
  funding wires. Those consumers now derive the full selection, with accepted
  two-member and exact missing/extra/foreign member-run controls. Actual funding
  then exposed a remaining three-row ledger allocation and Core's fixed-four
  funding partition. Both repairs are in the sealed build. Actual activation
  then exposed a receipt rent quote for three rows instead of the authenticated
  ledger width. The receipt and relay consumers now carry that exact width;
  focused mixed-ELF ProgramTests cover multi-row fold/reclaim and the direct
  three-row control. Foundable Ensemble quorum is canonically odd. The Journey
  campaign now uses four members, three fresh Pyth captures, quorum-three fold
  and reclaim of the unused fourth member. It validates this shape before
  infrastructure publication. A local-validator run now accepts all three
  captures, then refuses the fold because their certificates name the Pyth
  deployment release where the Ensemble policy requires the Source's provider
  release. The Resolution producer repair in `d448a2772` carries the already
  authenticated Source provider identity into the certificate while retaining
  the Pyth deployment identity in its request and receipt. A focused control
  explicitly distinguishes both identities. The corrected host and sealed
  `551ffc1b` runtime accept all three captures on a fresh local validator, then
  fold refuses with Resolution's Transition error. Native replay of the exact
  retained inputs identifies `DeadlineNotReached`; changing only Clock to after
  the deadline accepts the Source transition. The host now waits on finalized
  chain Clock. The preserved validator resumed after a verified backup; the
  corrected host passes that guard and reaches a later `OutputState` refusal
  during Fold simulation. The success certificate and uncaptured member seat
  lacked rent. After explicit funding, the unchanged sealed runtime accepts
  Fold and Reclaim: the Source is Resolved, the receipt consumes exactly three
  members, and the fourth seat's entire rent reaches its persisted beneficiary.
  The dated authority is
  `docs/evidence/ENSEMBLE_ACCEPTED_LOCAL_VALIDATOR_2026_09_08.md`. This is
  accepted retained-runtime execution. The Sol owner is separately checking
  founding capitalization and payment timing: a late campaign-payer top-up
  cannot prove that a newly founded market needs no rescue funding to finish.
  Recovery exhaustion reached terminal
  admission and all three refund payouts, recorded in
  `docs/evidence/RECOVERY_EXHAUST_LOCAL_VALIDATOR_2026_09_07.md`. Capture's prior
  Pyth submit succeeded but execution correctly refused an elapsed deadline.
  A measured 900-second rung is committed. A later expired Core-funding packet
  prompted phase instrumentation. The fresh 16-tick diagnostic now completes
  all six founding transactions and reaches Recovery resolution, selector 0,
  with Core terminal poststate. The separate 64-tick control stopped before
  planning because its source window no longer matched projected slot-time.
  Packet freshness was not relaxed; the earlier long stall's cause remains
  unproven. See `docs/evidence/FOUNDING_PACKET_FRESHNESS_RUNTIME_2026_09_07.md`.
- Structured's Claims operations have executed against real ELFs. The V6 child bridge and per-Market publication producer are committed.
  Executing the entrance exposed a missing root-creation descriptor. The set
  now enumerates seven action descriptors plus one V1 root-activation
  descriptor. Generic root execution and receipt submission are wired. The
  validator founded the Market in 258 transactions, then exposed a partial
  JSON decoder rejecting the full selected-capability payload. That decoder
  now consumes the canonical complete DTO. Executing publication then exposed
  unsorted hashed graph nodes and semantic IDs confused with content digests.
  The actual producer now passes complete sparse K=2, N=4 admission across
  eight Market identities; restoring each defect reproduces its exact failure.
  The Portfolio payoff remains unchanged when receipt units are scaled.
  The preserved validator now accepts selector-255 root activation. Receipt
  construction next exposed a wrong descriptor selection and an N/K join.
  Correcting those reached the demo producer's invented release ID, incompatible
  with the real Core-selected execution release. The producer now binds the
  checked release; its immutable old Market must be replaced. The next Claims
  join incorrectly equated component count N with outcome count K. A controlled
  real-ELF red/green run repairs that join while preserving exact hostile
  rollback; see `docs/evidence/CLAIMS_RATIONAL_LIFECYCLE_COMPONENT_MIX_2026_09_07.md`.
  That is component evidence. The fresh validator accepts root activation;
  its receipt constructor next refused a missing child-authority signer flag.
  The producer now preserves that child fact while outer transaction compaction
  retains its own signer rules. The native control is red before this repair
  and green after it. Further seal instrumentation located producer/frame
  defects; the selected lifecycle profile now gives its existing root the
  canonical header plus state width (`d64cd316b`). Those repairs still require
  a fresh actual receipt, representation and retirement poststate.
- Browser joining now carries the linked-basis binding, requires finalized
  confirmation and authenticated poststate, and resumes its transaction journal.
  Generated reference/client mirrors and the capability graph have converged.
  A bounded live aquarium panel reads finalized Market details and recent
  signatures, discards late results, and preserves stale facts honestly. Its
  snapshot status ages independently of network refreshes.
  Cohort 18 is still a placeholder, so no current-cohort participation claim is
  justified yet.
- The aquarium has bounded epochs, role and release checks, spending limits,
  idempotent journals, replenishment, a durable supervisor lease, explicit
  stop/resume controls and a checked static status publisher. It still needs a checked new cohort,
  actual markets and ticket-author provenance before instantiation and
  supervision on devnet.
- The fresh local-validator floor at `88aec17e8` completed 209 transactions;
  eight compute-budget rows remain red, compared with thirteen in the older
  run. Its native checks, runtime hashes, witnesses and limited route coverage
  are recorded in `docs/evidence/INFRASTRUCTURE_FLOOR_88AEC17E8_2026_09_07.md`.
  These rows are inherited performance baselines in
  `tools/gauntlet/CU_BUDGETS.json`, not transaction compute grants. Their red
  comparisons are distinct from a transaction exhausting its requested compute
  or heap. Complete protocol
  route coverage, final release gates, devnet redeployment and a running public
  aquarium remain open deliverables.

## Existing public site

The publication repository's `origin/main:.github/workflows/pages.yml` builds
`dclutch/apps/dclutch-web` at the domain root and renders the existing
`docs/guides/README.md` index under `/docs`. GitHub's Pages metadata names
`clutch.dregg.pro` and workflow deployment. The workflow is manual; a publication
cut alone does not deploy Pages. The last successful deployment at inspection
was run `33442687321`, source `9fe6ec20872a94380a8c291d87afe976d5efcf29`, on
2026-08-31. The publication checkout's parked branch is not its published main;
inspect `origin/main` explicitly.

The renovation uses the current homepage, market discovery, console and explorer
components. It moves market/design entrances and observed activity into view,
adds search over the console's existing capability catalogue, and decodes the
Dealer's generated layouts and previously unmapped instructions. The current
Devnet SDK addresses were cohort 17 while their human provenance still named
cohort 16; the evidence now follows the actual configured program set and
identifies historical observations separately from fresh reads. Reader, trader
and operator explanations remain in the existing guide hierarchy.

The renovation and explorer follow-up are published through the existing static
export, renderer, cut and Pages workflow. The first publication is recorded in
`docs/evidence/PAGES_RENOVATION_2026_09_07.md`; the subsequent navigation and
layout fixes are recorded in `docs/evidence/PAGES_EXPLORER_FOLLOWUP_2026_09_07.md`.
Both name exact source, publication SHA, workflow result and validation limits.
The subsequent checkpoint documentation refresh also passed the same existing
workflow, recorded in `docs/evidence/PAGES_CHECKPOINT_REFRESH_2026_09_08.md`.
Runtime execution and the new devnet cohort remain outstanding.

## Execution order and completion evidence

| Work | Required result |
|---|---|
| Claims execution and runner repair | Refunding and categorical split/merge reach exact token/claim poststates; hostile acts assert exact refusals and rollback; founding bond cases execute; observed transactions enter the census. |
| Current-source local validator baseline | Run tier 1 through its real launcher and operators; record its last reached stage and every failure. Seeded ProgramTest state cannot fill a missing validator stage. |
| Remaining family entrances and exits | Nonzero Dealer trade; General lifecycle; Series producers and two occurrences; ensemble material/fragments/fold; Structured representation and retirement; economics and recovery flows. Use the contract's campaign list and the route census to expose anything left unexecuted. |
| Release convergence | Required gates, generated references/client mirrors, frame captures and checked release evidence from the final commit. |
| Fresh devnet cohort | Every current cohort program gets a fresh identity, all ELF hashes and deployment signatures are recorded, and representative markets reach their stated poststates. Follow current authorization: abandon the old cohort in place. |
| Public participation and observation | Site/explorer identities agree with the checked cohort; ordinary wallets can find active markets and act; a supervised, bounded simulator produces observed activity, reports failures honestly, and has a documented stop/resume procedure. Publish through the cut. |

Measurements belong in dated evidence, with source revision and artifact hashes.
A declared binding, an in-process bank result, a local-validator transaction and
a devnet transaction remain separate evidence levels throughout this wave.
