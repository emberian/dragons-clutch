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
