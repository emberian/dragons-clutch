# General retained projection writes — 2026-09-09

The mandatory pre-child Hot Effect projection now retains its validated account-data writes. The post-child commit applies those exact values to non-root accounts first, then the root. Request and lamport operations still execute and validate in the mandatory projection; a refusal discards the plan. The plan retains the original Effect geometry guard, bounds its allocation by the authored data-write count, excludes disabled writes, and preserves root rent and alias checks.

## Evidence level

This is native control and warm SBF program-test evidence, not a current-source cohort or devnet claim. The diagnostic checkout is `/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/source`, with its own targets. It retains the earlier Registry dependency closure and coordinated source patches. No compute-limit increase was used.

The prior terminal census found 125 expanded Effect operations but only 26 account-data writes for the two-outcome Place fixture. Re-resolving the full Effect after all children exhausted the chain limit. With retained writes, `program-test-delegated-v35-projected-writes-production.log` records an accepted Place at 1,367,173 CU under 1,400,000, leaving 32,827 CU. This fixture was one Buy, one lot, two outcomes, quote reserve 1,000,000 atoms. Its next canonical Order decode exposed a separate pre-existing persistence defect; acceptance alone did not establish economic settlement. ELF hashes are retained in `production-v35-sha256.txt` beside the log.

## Controls

`native-general-projected-bank-green.log` and `native-general-projected-bank-restored-green.log` each pass four filtered controls: real projection-to-commit writes and request narrowing refusal; root-last ordering; root rent-floor refusal; and mismatched original geometry refusal. The new control includes an inactive conditional write and checks the retained root bit and actual account bytes.

In `native-general-projected-bank-red.log`, a temporary mutation discarded every projected write. The new control failed (exit 101), observing root bits `[0]` instead of `[2]`. Accepted source was restored and byte-compared before the four-control green rerun. No mutant ELF was built.

The frame ratchet remains red pending the integration owner's capture. Headroom depends on account/PDA geometry and width; these measurements do not establish a global supported-width CU bound or matched settlement completion.
