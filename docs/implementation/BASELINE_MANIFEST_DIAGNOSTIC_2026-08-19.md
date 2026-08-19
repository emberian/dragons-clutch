# Schema-v2 baseline diagnostic — 2026-08-19

Status: **DIAGNOSTIC / NOT A BASELINE**.

The clean-tree command

```sh
scripts/baseline_manifest.py emit --run-gates
```

ran all 94 declared gates from main commit `ec77d0b`. It derived content
identity SHA-256
`172ef191448e12d89e3353ab73fab8f12e91dc0aa5ab068fe022c3b8646f6861`
over 501 tracked entries. Exactly 86 gates matched their declarations and eight
contradicted them. The emitted diagnostic manifest was not committed and the
historical schema-v1 `MANIFEST.baseline.json` was restored.

## Exact contradictions

| Gate | Observed result | Required repair |
| --- | --- | --- |
| `cargo_test.solana_reference` | exit 101 | Refresh and independently inspect `programs/solana-reference/Cargo.lock`; `--locked` currently requires a change. |
| `cargo_clippy.solana_reference` | exit 101 | Same lock defect; do not classify this as a source lint result until the lock is coherent. |
| `cargo_doc.solana_reference` | exit 101 | Same lock defect. |
| `cargo_clippy.clutch_sbf` | exit 101 | `clutch-sbf-harness` helper `withdraw_cash_compares` has too many arguments under strict `-D warnings`; refactor or narrowly justify the API instead of weakening the workspace gate. |
| `benchmarks.unittest` | exit 1 | Reconcile the cost lab with `MAX_INTENT_BYTES=310`, the current PlaceOrder portfolio wire, CandidateFeed/ClearWork accounts, and versioned intent codecs. |
| `benchmarks.abi_audit` | exit 2 | Teach the ABI auditor to resolve the current ResolutionWork and artifact constants rather than treating their `encoded_len` expressions as unknown. |
| `sbf.runtime_bringup` | exit 1 | The historical walk still expects default CreateMarket/materialize/dematerialize success. It must be split into an inert default/`0x79` refusal campaign and an explicitly labelled non-production mock-source success campaign. |
| `sbf.committed_signed_walk` | exit 1 | Step 5 Endow now correctly refuses `SourceReleaseUnavailable` (`0x79`) under the default empty registry. Move success evidence to the explicit mock ELF or redesign the walk around the default refusal boundary. |

The default and explicit `non-production-mock-source` in-process SVM suites both
passed. Lean, all three refinement campaigns, the scalar batch proof and four
red mutants, B-spline/accumulator/liveness/model gates, the sealed liveness
profile, terminal-lifecycle V2, invariant campaign, vector checker, Glass, and
the declared toolchain gates also matched.

## Promotion rule

Do not change a declaration from expected success to expected failure merely to
make the summary green. Repair the stale lock/tooling/harness assumptions,
rerun the narrow eight gates, then run the complete clean-tree emission again.
Only a 94/94 result may replace the checked-in schema-v1 manifest. After the
manifest-only commit, rerun:

```sh
scripts/baseline_manifest.py check --run-gates
```

This diagnostic is local evidence only. It is not a release, deployment,
public-network run, security review, or authorization.
