# General Verify lifecycle forecast — 2026-09-09

Verify previously evaluated its conditional Result/Create plan before the
candidate evaluator had populated the terminal-row register. Both native and
host preplan rejected the disabled declaration. Skipping that declaration alone
would also be wrong: a terminal evaluator result would introduce a Create that
the preplan had never funded or authenticated.

The candidate semantic owner now exposes the same authenticated row selector
used by verification. It forecasts only whether the selected row is the last
row of the last page. It binds the closed Batch to the Candidate, validates the
Candidate identity and submission, and checks the Page revision and coordinates.
It does not authorize an Order, cursor advance, economic result or payout.

A shared General adapter validates the actual owner and complete Batch and
Candidate account envelopes, binds the request's Candidate, then writes only
VERIFY_TERMINAL. Native Hot calls it before lifecycle preparation. The later
candidate evaluator still requires its complete result to equal that forecast.

Lifecycle preparation records every declared guard decision. Both passes skip
disabled declarations, while replan must reproduce each decision and the exact
active plan table. A skipped Create consumes no bump hint. Immutable artifact
facts remain separate from these mutable register decisions; account balances
and observations remain freshly read after the accelerator.

## Validation

The combined source was checked in the existing diagnostic source archive at
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/source`, using its
own `target-native` and `swarm-build`. This archive retains the older Registry
dependency closure; it is not an exact-current-source cohort or devnet claim.
The live source root was `/Users/ember/dev/dclutch`, with HEAD
`27044dd43ce3a8df5a56a177b5bac7cbc5a52218` when the mutation results were read.

- Combined package check: Trading SBF, Accelerator SBF and bundle builder
  passed. The diagnostic host contained the matching preplan seed and disabled
  guard changes before the production projection extraction.
- `terminal_forecast_`: 2 passed. The positive control distinguishes both row
  and page terminal coordinates. The hostile control checks a foreign Batch,
  noncanonical Candidate, substituted Page revision and wrong coordinates.
- Mutation: deleting only the canonical Batch/Candidate join made the foreign
  Batch assertion fail: actual `Ok(true)`, expected
  `GeneralCandidateErrorV1::Collection(GeneralCollectionErrorV1::Substitution)`.
  The test process exited 101. The accepted file was restored byte for byte;
  both tests then passed.
- `verify_candidate_projects_the_exact_terminal_cursor_result_and_reward`:
  1 passed, including eight shared preplan refusals for owner, envelope width,
  request identity, page/row coordinates, action and scalar-bank width. Every
  refusal preserves the complete caller scalar bank.
- `lifecycle_static_tests`: 6 passed, covering immutable cache bindings,
  commit-last refusal, terminal/nonterminal guard sets, guard flips with equal
  active counts, exact walk width and active-plan count. The active hint order
  is Candidate/Verifier for nonterminal rows and Candidate/Result/Verifier for
  terminal rows. The lifecycle owner separately proved omission of decision
  comparison red and restored these tests green on the current V2 closure.

Logs are in that diagnostic base:
`check-general-verify-guard-closure.log`,
`native-general-terminal-forecast.log`,
`native-general-terminal-forecast-mutant-red.log`,
`native-general-terminal-forecast-restored-green.log`,
`native-general-verify-preplan-controls.log`, and
`native-general-verify-guard-controls.log`.

## Boundaries

The native semantic closure is committed before the coordinated production host
projection extraction that consumes its shared helper. Actual SBF Verify,
entrypoint guard-flip rollback, matched nonzero settlement, supported-width CU
coverage and production user-entrypoint reconciliation remain pending at this
record's creation. Fresh production Trading and Accelerator diagnostic links
are building. This change leaves the frame ratchet red; no frame baseline or
new cohort acceptance is claimed here.
