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

- Claims' six named ProgramTest targets have executed, including backed round
  trips and exact hostile rollback. The dated authority is
  `docs/evidence/CLAIMS_CAMPAIGNS_2026_09_07.md`; this does not substitute for
  local-validator founding or full protocol route coverage.
- Checked eight-program builds now exist through `7f1ad2acf`; `a628bfd39`
  and `5cbc1e777` also have matching independent frame captures.
  These remain intermediate: later General and Claims changes require a
  fresh all-program build. Publication cuts carry development work; they do
  not assert that a build has become a deployed cohort.
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
  are implemented. Their actual Found/Prepare driver is being connected;
  the complete two-occurrence validator campaign has not run.
- General's accepted nonempty path is being run after repairing action-scoped
  lifecycle funding and the semantic outer frame / key-sorted Claims child
  boundary. Its next repair reads the actual external token-account owner,
  respecting the selected token profile's variable account width. Dealer's
  RentCredit repair is running in a fresh validator campaign; a nonzero fill
  is still unexecuted. Adversarial review additionally found and repaired
  Dealer/Claims admission's missing join to Core's committed rent-credit address.
- Ensemble fold/reclaim has accepted real-ELF poststates. Its direct Pyth member
  producer is being executed separately so pre-captured fragment fixtures do
  not stand in for capture evidence. The two-member compiler and first-member
  real transport CLI are running against a checked validator. Recovery's
  exhaustion branch reached terminal admission and all three refund payouts;
  Capture is restarting after the prospective-vacancy reader repair.
- Structured's Claims operations have executed against real ELFs. The V6 child bridge and per-Market publication producer are committed.
  Executing the entrance exposed a missing root-creation descriptor. The set
  now enumerates seven action descriptors plus one V1 root-activation
  descriptor. Generic root execution and receipt submission are wired. The
  validator founded the Market in 258 transactions, then exposed a partial
  JSON decoder rejecting the full selected-capability payload. That decoder
  now consumes the canonical complete DTO; actual root/receipt poststates
  remain owed.
- Browser joining now carries the linked-basis binding, requires finalized
  confirmation and authenticated poststate, and resumes its transaction journal.
  Generated reference/client mirrors and the capability graph have converged.
  Cohort 18 is still a placeholder, so no current-cohort participation claim is
  justified yet.
- The aquarium has bounded epochs, role and release checks, spending limits,
  idempotent journals, replenishment, a durable supervisor lease, explicit
  stop/resume controls and a checked static status publisher. It still needs a checked new cohort,
  actual markets and ticket-author provenance before instantiation and
  supervision on devnet.
- The initial local-validator tier at `0a869a766` still has thirteen compute
  budget regressions. Those budgets have not been widened. Complete protocol
  route coverage, final release gates, devnet redeployment and a running public
  aquarium remain open deliverables.

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
