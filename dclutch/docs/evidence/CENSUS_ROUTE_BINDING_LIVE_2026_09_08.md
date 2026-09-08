# Census route binding on preserved local-validator evidence — 2026-09-08

Evidence level: one read-only finalized transaction from the preserved Structured local validator on hbox. This records one accepted Claims CPI route and the census rejection of an ambiguous Trading outer route. It does not establish family-wide coverage, a clean producer build, or devnet/mainnet evidence.

## Source and validator

The clean inventory was generated from commit `b5628fe1fd3560a8a090e183c024ad8fb17ec7ec` in `/Users/ember/dev/dclutch`, using a detached clean worktree at that revision. The shared swarm tree had unrelated dirty successor and operator files; no census or RPC source file was changed for this capture.

The preserved Structured validator was:

```text
RPC:       http://127.0.0.1:24783
ledger:    /tank/dregg-build/dclutch-structured-fresh-a7f9-host627b9ea1-20260907/campaign/substrate/ledger
campaign:  /tank/dregg-build/dclutch-structured-fresh-a7f9-host627b9ea1-20260907/campaign/structured-founding-evidence.json
signature: 2U3PJ1SLaQ7Q5jwk7gf5oYKX4VjEjyFt2nAcPmRUXeQGQSJEDvwdUoKBw12f7wxuukJMJ8fkKa9KEtmc6i82mUKR
slot:      9789
meta.err:  null
```

General artifacts were also checked at `/tank/dregg-build/general-a7-fresh-found-20260907/general-founding-execution.json` and `/tank/dregg-build/general-a7-fresh-found-20260907/general-capability-activation-execution.json`. Their old signatures were absent from current General RPC history, so they were not used as current finalized route evidence.

## Native route distinction

The accepted transaction's top-level instruction is owned by the Trading outer program `o2PjEqp3KqgYd1kFTGuC93DG5LHWRazFgP8HbeNSyhu` and begins with `DCLTGMF3`. The inventory route `trading/generic_market_founding_v1::process_generic_market_founding_v3` has predicate, length, and magic selectors; the census rejects this outer route as ambiguous.

The admitted route is a separate inner Claims CPI owned by `Bfq84eMEv2uYAA96bcqiwjQwzC16ynjRxbsBRwqpTQKj`. Its native data begins with `DCLFDR05`, the sole selector for `claims/founding_v5::process`. The fold binds that Claims program address and selector directly from finalized instruction evidence. Generic invoke/success logs are not used to identify this route.

## Captures and hashes

The committed capture files are under `docs/evidence/census-route-binding-2026-09-08/`:

| File | SHA-256 |
| --- | --- |
| `rpc-getTransaction-json.json` | `b0824ac89dc3eb1cde2d59c23d2e73fde08c7b71c22d0a242ee57ca1494dd98b` |
| `rpc-getTransaction-base64.json` | `4398a7e11e775ccd216a19ba16fc0a4608b9b8763d5c43e43e7d12e7d93a23e3` |
| `normalized-evidence.json` | `69c4b6f0483a4db29869c725ab65731e30198f5f3240e73e39f517e3c0c8a9de` |
| `fold-bindings.json` | `0e291a1a5a43e764a2564628333d199bf367bbf212ffe9737bf66a1e04adb434` |
| `fold-programs.json` | `45f0f7df7d23a3f2af0d5f38eaedcd5ef2ec5379acd8fcbfce144813a2e2d2c9` |
| `inventory.json` | `b4575d40e7a883877643457bd1b5bc4ae5eaeb117a1b223183b185986ca8b4d2` |
| `fold-output.json` | `a25e2cee29fb5c01ff32370018ce591492f21cecb2142fa30e12360a19e17930` |

The base64 RPC response contains a 498-byte serialized transaction packet (664 base64 characters). It is retained so canonical packet authentication can be rerun once the shared successor compile blocker is cleared.

## Fold and validation

The clean inventory command was:

```text
cargo run --manifest-path tools/gauntlet/census/Cargo.toml --locked -- inventory \
  --root /tmp/dclutch-census-source --out /tmp/census-inventory-clean.json --check-unique
```

The live fold admitted one observation for `claims/founding_v5::process` at slot 9789 and returned zero fold problems. The exact filtered census command passed 16 tests:

```text
cargo test --manifest-path tools/gauntlet/census/Cargo.toml --locked \
  ledger::tests -- --test-threads=1
```

The real Rust producer decoder test against this live RPC response did not execute. The focused package build stopped first on unrelated shared successor errors: private `SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3` and `SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3` imports in `tools/local-validator/bootstrap/successor/src/series_consume_geometry.rs`. Existing realistic finalized-format controls had already passed for v0 loaded-address resolution, native CPI ceilings, failed-index exclusion, and poll-only no-send behavior. This record makes no claim that the live response passed that Rust decoder until the shared compile debt is resolved.
