# Claims campaigns executed — 2026-09-07

Owner: CODEX-INTEGRATION. Evidence level: real ELF ProgramTest, in-process bank.
No local-validator or devnet execution is claimed by these results.

## Source and execution

Live repository: `/Users/ember/dev/dclutch`.
Measured checkout: `hbox:/tank/dregg-build/dclutch-codex-20260907`.
Accepted source: `cac1a0593cba45ad1b29e565e933aac865e1f717`, detached and clean
for the build and run. Both repository root and HEAD were printed with the run.
No manifest or lock change was needed; locked workspace metadata succeeded.

The full suite runner performed the locked package check before the SBF builds,
verified the Token-2022 fixture against its provenance, and freshly built its
nine first-party program/caller links. Execution was contained by `swarm-build`
on hbox, with `SWARM_MEM_MAX=32G`, `CARGO_BUILD_JOBS=6` and serial test threads.
Neither stack-diagnostic shape nor the scheduler's silent no-op was admitted.

Run root:
`/tank/dregg-build/dclutch-codex-evidence-20260907/committed-runner/run.e0CwMY`.
The runner was invoked without target selection, which selects these six
explicit targets; it never invokes an unfiltered package suite.

| Test target | Passed | Failed | Runner exit |
|---|---:|---:|---:|
| claims_founding | 7 | 0 | 0 |
| claims_conservation | 7 | 0 | 0 |
| fractional_atomic | 13 | 0 | 0 |
| permissioned_burn_wall | 2 | 0 | 0 |
| fractional_compaction | 12 | 0 | 0 |
| escrow_pda_handover | 2 | 0 | 0 |

All 43 tests executed. None was skipped or never ran. Every target has its
separate `test-<target>.log` in the run root. Nine hermetic runner controls also
passed locally through `python3 -m unittest discover -s tools/gates/tests -p
test_fractional_runner.py`; these controls test orchestration, not the protocol.

## The red control and accepted poststates

At intake source `0a869a76633f2755ef3d359c2d8b63b5b6447f3b`, all five
transaction cases in `claims_conservation` failed at `CoreState::encode` with
`InvalidPhase`; its native census control was the sole passing case. The
fixture changed `Phase::Open` while leaving readiness `Prepaid`. Core's existing
state invariant requires `Consumed`. This was a fixture defect, not a reason
to loosen Core's codec. Baseline: `test-claims_conservation.log` in the evidence
root named below. The two previously unrun founder-bond refusals executed as
part of the seven passing founding tests.

`seed_open_market` now labels the installed prestate explicitly. Claims founding
executes the real ELF and creates the aggregate and Positions; Core Open itself
is seeded here and earns no execution credit. The refunding accepted round trip
checks backing, ordinary liabilities, failure escrow, allocation, surplus
donations, balances, and restoration. The additional categorical round trip
checks every coordinate including failure, the aggregate supply and exact owner
and vault token restoration, and proves no refunding escrow was created.

Every conservation hostile still asserts its exact refusal enum. It now also
compares complete named account pre/poststates, including the earlier token
approval in the same transaction; only the payer's exact bank-quoted fee may
change. A refusal reaching the wrong conjunct therefore cannot satisfy the
campaign by merely being an error.

## Transaction observations

Evidence root: `hbox:/tank/dregg-build/dclutch-codex-evidence-20260907`.
`committed-evidence/` contains 77 instrumented transactions across 47 labels
from founding, conservation and compaction. The other three passing targets
are not claimed to emit census observations. `claims-committed-folded.json`
was folded once from that directory; the census admitted 148 observations into
`claims-committed-ledger.json` using the committed bindings. Bindings were
written from observed transactions; a token donation contributes no protocol
route, and seeded Core Open is not credited.

| Evidence artifact | SHA-256 |
|---|---|
| claims-committed-folded.json | `a86af15e6612e88d0bceecadbff18ccae95525e4907a2ddb9bc5dd8af6ef9c3b` |
| claims-committed-ledger.json | `035aa721cc17be17293130cfe67d6cb3664e82a880bc418a50e38596daa19051` |
| committed-runner/run.e0CwMY/results.tsv | `f5f9f871a62ad627711431439f2dcb9ffd74a5ebd266d201476b326a4b4d517e` |

## Executed ELF identities

All hashes are SHA-256. These are the exact files in the run's `sbf-out/`;
`elf-sha256.txt` is retained beside the logs.

| ELF | SHA-256 |
|---|---|
| dclutch_claim_check_escrow_signer_test_sbf.so | `237c63a884aafc80b8a22f6f6b5bf3d4bc1d3c9b6156270e2fa05fadad9c38b1` |
| dclutch_claims_founding_test_caller_sbf.so | `3874f94b7d4e269fb8e158a306c67839c2ab96edabec6763917517a1698969aa` |
| dclutch_claims_sbf.so | `19a3be2fcb625bf56166f8b638adba2e7d970421d1a08e9fdd1bc63d7ddaef5d` |
| dclutch_core_sbf.so | `7013924151b5fbd1f3d5abf0c88fb6199adaf2a6ba7f2a91da89014c4cf086a9` |
| dclutch_custody_sbf.so | `178bb636165036ef061dc2268868c845fed47e3f9bdd1632d2c06161b242a5b7` |
| dclutch_fractional_atomic_test_caller_sbf.so | `d485ac634df30416a23cc25eb1dbf068bd3cbc7e395f2f4e8e3b54ca0d4d2335` |
| dclutch_fractional_compaction_test_caller_sbf.so | `76d26ce2d2e28ae4cc4d7bcc7a2237f6b75f828036f0dfad7702c8ff9c54a62c` |
| dclutch_registry_sbf.so | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| dclutch_rent_sbf.so | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| spl_token_2022.so | `e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697` |

## Reference convergence and remaining scope

`tools/gate reference --converge` ran from the clean detached worktree
`hbox:/tank/dregg-build/dclutch-codex-reference-20260907` at the accepted source
and reached its fixpoint on pass two (`reference-cac1.log`). The generated
reference discharges the conservation wiring block. It also corrected existing
General ABI drift: obsolete entries in `generalSuccessorV5` disappeared and
current `generalClearingPriceV1` and `generalOrderV2` pages appeared. No generated
page was edited by hand.

This batch closes the initial unrun Claims cases and repairs the runner's
accounting. It does not close any unexecuted family, the full local-validator
lifecycle obligation, the checked release, or the fresh devnet/site/simulator
request. Those remain in the active development-wave plan.
