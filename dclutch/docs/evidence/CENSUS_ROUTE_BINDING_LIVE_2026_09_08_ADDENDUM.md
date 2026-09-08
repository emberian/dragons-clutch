# Addendum — live finalized producer decoder now executed — 2026-09-08

This addendum updates the test-status limitation in
`CENSUS_ROUTE_BINDING_LIVE_2026_09_08.md`; it does not rewrite that dated
record. The earlier conclusion was accurate when recorded: the shared
successor build was then blocked. After the bootstrap compile repair landed,
the producer test ran against the committed real-validator response.

## Focused producer proof

The test is
`rpc::tests::committed_live_v0_fixture_passes_the_finalized_decoder_and_resolves_claims_cpi`
in `tools/local-validator/bootstrap/successor/src/rpc.rs`. It serves the
committed `getSignatureStatuses` and base64 `getTransaction` JSON through a
local read-only HTTP fixture, then calls `Rpc::finalized_signed_packet`.
Consequently the proof includes the producer's finalized-status check,
canonical base64 decoding, bincode packet decoding, signature verification,
v0 message handling, loaded-address resolution, and native top-level/CPI
instruction extraction.

The exact filtered command was:

```text
cargo test -p dclutch-local-successor-bootstrap --locked \
  --bin dclutch-local-successor-bootstrap \
  rpc::tests::committed_live_v0_fixture_passes_the_finalized_decoder_and_resolves_claims_cpi \
  -- --exact --test-threads=1
```

Result: `1 passed; 0 failed; 942 filtered out`.

The test found the Claims CPI program
`Bfq84eMEv2uYAA96bcqiwjQwzC16ynjRxbsBRwqpTQKj` with native `DCLFDR05`, and
the separate Trading outer program
`o2PjEqp3KqgYd1kFTGuC93DG5LHWRazFgP8HbeNSyhu` with native `DCLTGMF3`.
The census fold remains the authority for rejecting the Trading route's
ambiguous predicate/length/magic selector vector.

## Pins

The fixture is unchanged. Its base64 RPC response SHA-256 is
`4398a7e11e775ccd216a19ba16fc0a4608b9b8763d5c43e43e7d12e7d93a23e3`.
The producer source was tested from the workspace at commit
`b5628fe1fd3560a8a090e183c024ad8fb17ec7ec` plus the focused test in this
addendum's commit. This is one preserved local-validator transaction and one
producer path; it is not a full family or route coverage claim.
