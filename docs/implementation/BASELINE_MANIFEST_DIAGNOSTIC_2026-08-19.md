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

## Repair progress

Seven contradictions are repaired and focused-green after that historical run:

- `b0e87dc` refactors the harness comparison input and restores strict
  workspace Clippy;
- `9c371fe` refreshes the Solana-reference lock graph and fixes its rustdoc
  link; 54 tests, strict Clippy, and strict rustdoc pass offline/locked;
- `7b056bc` reconciles the cost/ABI model with the current 310-byte intent,
  account families, and versioned codecs; 43 unit tests, `abi-audit`, and the
  263-scenario check pass; and
- `38c8957` runs the signed walk against the explicitly different
  `non-production-mock-source` ELF. All 22 transactions confirm, 18 watched
  accounts reload, the terminal falsifier goes red, and the walk ends with both
  owners' cash and pooled custody drained. It now reports one semantic refusal
  and one exact two-instruction compute-ceiling STOP with rollback. The default
  ELF still refuses Endow with `0x79`.

All eight contradictions now have landed repairs (`83e124d` split the
bringup evidence; `b5700a9` resealed the liveness profile to the current
runtime identity). The complete clean-tree emission is IN PROGRESS, started
2026-08-19 ~08:01 by the Fable session at HEAD after this commit. Do not
commit to this repository until the emission completes and its manifest
lands; a mid-run commit makes the run non-hermetic (this raced once already
this morning).

## Default-ELF identity fork (discovered ~07:30, verified by fresh builds)

`9c371fe` repaired the reference lock graph and, in the same commit, fixed a
rustdoc link by editing three doc-comment lines in
`programs/solana-reference/src/resolution.rs` — a file inside the 88-file
declared runtime source closure (`clutch-sbf` depends on the crate by path).
Consequence, proven not inferred: a fresh clean build at `ec77d0b`
reproduces the sealed default ELF `a5725a3d8e149b2b…` exactly, while a fresh
clean build at HEAD produces
`bd20711b01828a745ce89de3aacb4b908cbcde32307b61be2c7d612bb8516b60`; an
independent interrupted gate run reproduced the same new digest. The full
source delta between the two identities is exactly that one doc comment.

Because the doc-comment fix is required for `cargo_doc.solana_reference` to
pass, the sealed `a5725a3d…` identity is unreproducible from any tree that
can pass 94/94. Under this ledger's own promotion rule the baseline must
therefore adopt the new clean-HEAD identity and re-seal what is bound to the
old bytes: the `python.liveness_policy_profile_current_seal` gate and its
artifact ledger, the final-LTO/stack audit artifacts under
`research/liveness-policy-profile/artifacts/a5725a3d8e149b2b`, the
`CURRENT_TRUTH.md` §2 identity paragraph, and a fresh portable attestation
(already queued by the handoff as the post-`b5da74f` decision — now
required rather than optional, since the delta is no longer host-only).
The `a5725a3d…`/`b5da74f` seal and the Persvati attestation remain valid
historical evidence for source `7e8f6b1`; nothing about them is retracted.
Do not hand-edit any digest: rebuild, re-measure, and let the gates record
the new identity from fresh outputs.

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
run the remaining bringup gate, then run the complete clean-tree emission again.
Only a 94/94 result may replace the checked-in schema-v1 manifest. After the
manifest-only commit, rerun:

```sh
scripts/baseline_manifest.py check --run-gates
```

This diagnostic is local evidence only. It is not a release, deployment,
public-network run, security review, or authorization.
