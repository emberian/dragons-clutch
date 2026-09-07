# Night freeze — 2026-09-01

PROVENANCE: written by a grok session at 04:39 EDT that ember stopped
minutes later. Its lanes landed nothing — no file in the table below was
modified by that run, and the "five lanes launched" line it wrote into
`GOAL.md` (since reverted) described work that never happened. Kept
anyway, and adopted deliberately, because the
digest table is the useful part and it is CORRECT: all nineteen rows were
re-verified against the tree at 04:47 EDT and every one matched. Treat this
file as a provenance snapshot only.

HEAD at freeze `cd014d54`. Letter checkpoint `2af02f53` is an ancestor.
Do not treat this file as a queue. Dirty tree is deliberate.

hbox reachable: `/tank` 1.5T free. Preserved:
`/tank/dregg-build/dclutch-dealer-accepted-686.1788246335`,
`/tank/dregg-build/dclutch-trading-686bf2e5.RlF8aK`,
`/tmp/dclutch-structured-hot.psglmE`.
Missing on hbox at freeze: `/tmp/dclutch-general-open-elfs.9L3rOz`,
`/tmp/dclutch-series-elves.GQU64N`.

Mac `/System/Volumes/Data` 166Gi free (98% used). Heavy SBF stays on hbox
via `swarm-build`. Do not delete `~/dev` to make room.

## Lane-owned dirty / untracked (copy into a worktree before editing)

| Lane | Path | SHA-256 |
| --- | --- | --- |
| S5 | `programs/dclutch-dealer-accelerator-sbf/program-test/tests/accepted.rs` | `e1bac1e87cef8a559df6e731f5ad461d54f019147cf067d1a77792b087ef376e` |
| S6 | `programs/dclutch-trading-sbf/program-test/tests/series_pre_market_expiry_program_test.rs` | `e6dd430ebbb30d8e88f990a50fff21c1c38075701b417cbb3fbb9b5b8c2b2465` |
| S4 | `programs/dclutch-trading-sbf/program-test/bundle-builder/Cargo.toml` | `074c0dfaf983aae4ac09c3a8d6f515b6fd07d9b31b74fea126a41ca9d2b2ad9f` |
| S4 | `programs/dclutch-trading-sbf/program-test/bundle-builder/src/general.rs` | `e20c25a1cd1525020a8a00bfbb4a99c4121ad910f486cac0a66a95e309cb9a92` |
| S4 | `programs/dclutch-trading-sbf/program-test/bundle-builder/src/routes.rs` | `539d5a1b7fe1bcee5bd6822b7e45ad8d9444da0d8af5267d44edd4af9f09d832` |
| S4 | `programs/dclutch-trading-sbf/program-test/bundle-builder/tests/general_dynamic_spans_v1.rs` | `f89347a513bd9c6bd4689f078b5da71512afccd1825790e09171ea3d832b1cf3` |
| S4 | `programs/dclutch-trading-sbf/program-test/general-hot/Cargo.toml` | `0af1a85bd3103e5b75df656bfb6e606ce57b66c84818ee53d601f1f0094ffbda` |
| S4 | `programs/dclutch-trading-sbf/program-test/general-hot/Cargo.lock` | `ef3cd2eee600a49009469bad0832ff740e16bbf688c2937d2c8f7b55fd690a96` |
| S4 | `programs/dclutch-trading-sbf/program-test/general-hot/src/lib.rs` | `07261c179286eab702ed3cdfb6bf8541b11341522437ae772a3a8c24201f8864` |
| S4 | `programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs` | `1a7aeedef812994d3bc4dbfb2838907c6e3c8a1600b4c38e6ad1ef887512f5c8` |
| S7 | `programs/dclutch-claims-sbf/Cargo.toml` | `3ea127ac6345ee797671c340d9920bac98c30c7f57e746963d08a86d97475730` |
| S7 | `programs/dclutch-claims-sbf/run-rational-representation-v2-program-test.sh` | `6f64e681aa3b550812b74bb03c7dc307954dd908ad40980ff4baa209062e29f8` |
| S7 | `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs` | `f1fe92e195f6e16fbd91368d2f6f2a063552fead2f7abdb9ff1712571c2ca3a3` |
| S7 | `crates/dclutch-bearer-v2-operator/src/lib.rs` | `06ddc1f105a14f92a44c96e57692821bfdb1f956585aa604114839708ff9c009` |
| S7 | `crates/dclutch-bearer-v2-operator/src/open_lifecycle_policy_v5.rs` | `796d3cb974729f08b314c825caf632948f1190842884a4300560930f131737eb` |
| S7 | `crates/dclutch-bearer-v2-operator/src/open_release_v1.rs` | `3317c96bc9fa86ab9c094d0848e13e065e9c988e820d170c535c8bab86b57230` |
| S7 | `crates/dclutch-rational-representation-v2-operator/tests/operator.rs` | `1254bd41265e6ecc242a16988eb14995fa471a8e88dcf99b7e349f5ebc8b5a85` |
| S7 | `crates/dclutch-structured-v2-operator/tests/artifacts.rs` | `323866295d133df4730fae6b393eee2e60f928a5c931dc5d9a63a035777ec7f5` |
| S3 | `crates/dclutch-direct-codec/src/registered_terminal_artifacts_v4.rs` (untracked WIP; keep unregistered) | `1280cce939337ead4304c2ab953367fd013dced9fe4bfa68a9c95ac0a744be36` |

## Ambient dirty — do not absorb

`Cargo.lock`, Core `infrastructure.rs`/`infrastructure_v2.rs`, dealer-codec
scenario files, rational-lifecycle-hot-v3, registry/lineage, successor
lockfile, `hot_v3/seal.rs`, direct close/fee/retiring tests, and the rest
of `git status --porcelain` at freeze. Identify an author before committing
or deleting any of: `.claude/`, `GOAL.md.tmp-entry`,
`apps/dclutch-web/tsconfig.tsbuildinfo`, formal generator temps.

## First-wave commands (letter)

Dealer (pinned ELF; do not rebuild from dirty HEAD):

```sh
ssh hbox 'ln -sfn /tank/dregg-build/dclutch-trading-686bf2e5.RlF8aK/sbf-out/dclutch_trading_sbf.so /tank/dregg-build/dclutch-dealer-accepted-686.1788246335/sbf-out/dclutch_trading_sbf.so'
ssh hbox 'cd /tank/dregg-build/dclutch-dealer-accepted-686.1788246335 && SBF_OUT_DIR=/tank/dregg-build/dclutch-dealer-accepted-686.1788246335/sbf-out SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build cargo test --manifest-path programs/dclutch-dealer-accelerator-sbf/program-test/Cargo.toml --test accepted accepted_equity_selector_one_executes_real_custody_and_rolls_back_late_evidence_refusal -- --nocapture'
```

Structured prior log: `/tmp/dclutch-structured-hot.psglmE/hot-run2.log`.
