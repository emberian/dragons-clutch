# Claims admission activation-cache reuse — 2026-09-09

## Decision and scope

Claims ProtocolPosition Admit now decodes and authenticates the immutable
Registry activation cache once for its three role checks. Each Trading,
Claims and Core role still passes the canonical read-only frame and current
deployment authenticator. The cache owner, address, release-set identity,
width and complete hostile role projection remain checked. Close is unchanged.
The admission route also has feature-gated `claims-cu-profile` phase markers;
production builds compile those markers away.

## Real-transaction diagnostic evidence

These are SBF ProgramTest measurements in the General lane's warm Registry V1
continuation, not devnet or a current Registry V2 cohort. Source was applied as
an exact Claims-only delta to the existing diagnostic checkout; the current
Registry V2 tree was not copied into that cohort. The General lane owns the
complete transaction evidence and remaining route walls.

Evidence root on hbox:
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/`.

| Measurement | Baseline v25 | Cache reuse v26 |
| --- | ---: | ---: |
| Admission release authentication | 34,918 CU | 15,078 CU |
| Product/Core authentication | 34,684 CU | 34,685 CU |
| Complete instrumented Claims Admit CPI | 116,084 CU | 102,246 CU |

The release phase saves 19,840 CU. The entire CPI saves 13,838 CU in these two
runs; authority and vacancy/allocation PDA searches vary with generated
identities, so that total is not a controlled estimate of this change alone.
Both transactions still reach a later compute wall before delegated Custody;
this result does not claim General PlaceOrder completes.

Logs:
- `program-test-delegated-v25-claims-profile.log`
- `program-test-delegated-v26-claims-cache.log`
- `check-claims-admit-profile.log`, `build-claims-admit-profile.log`
- `check-claims-admit-cache.log`, `build-claims-admit-cache.log`
- `build-claims-admit-cache-production.log`

ELF SHA-256:
- Profile baseline (`claims-admit-profile-elf/dclutch_claims_sbf.so`):
  `39798eeba5cad2264bb1e98e28a874cdad16e8402c24fb24e10abe7b3ce6d811`
- Profile cache reuse (`claims-admit-cache-elf/dclutch_claims_sbf.so`):
  `7e2d765d58d621aba2c50e30827b3f8008ecea3f761c809fa1afe77d3cc4c11d`
- Production cache reuse (`claims-admit-cache-production-elf/dclutch_claims_sbf.so`):
  `c460ed0d3c3fa5831e6058c22d1478fdc3dbc9d235dd4bbb7637df5b7900df32`

Diagnostic Claims source snapshots, SHA-256:
- `claims-admit-profile.rs`:
  `8cb1b07c1d613fdd0430d1987f3674ebaa9e338dbfca41f6d380506c71c444ea`
- `claims-admit-cache.rs`:
  `be17c7f88f091aabc9fd0b2fe07d9ae4c19d30dc2ad15c23f3281bd13710b114`

## Controls

The exact named native control is
`protocol_position_v2::tests::admit_release_cache_once_preserves_identity_and_each_role`.
It admits one authentic cache and refuses ten mutations with
`ProtocolPositionSbfErrorV2::Release`: wrong cache address, owner, writable
privilege, or release; stale ProgramData deployment slot separately for each
of Trading/Claims/Core; writable Program separately for each of those roles.

The warm Registry V1 control passed one test, ten hostile mutations, with only
the test fixture's ArtifactRelease/DeploymentObservation type names adapted
from V2 to V1. `native-claims-admit-cache-v1-exact.log` records the result.
The earlier unadapted fixture did not compile against that old Registry;
`native-claims-admit-cache-exact.log` is that failed build, not a test result.

The deliberate red control authenticated only Trading, skipping Claims and Core.
The stale Claims ProgramData case returned `Ok(())` instead of the exact
`ProtocolPositionSbfErrorV2::Release`, failing the named test at mutation 5
(exit 101). `native-claims-admit-cache-red.log` records that failure. The
accepted source was restored and byte-compared before the green rerun, which
passed one test (`native-claims-admit-cache-restored.log`).

Current V2 package check passed from archive
`7eb4d69ada25fc8386110e666a511b8c6b27fb51` plus this Claims-only source change,
in `/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909/source`, using its own
workspace-root target. `check-v2.log` records `cargo check --locked -p
dclutch-claims-sbf --lib`; current V2 exact native execution is recorded in
`native-v2.log` alongside it: one test passed, zero failed, 98 filtered out.
The first isolated native build took 6m 46s including an OpenSSL header-install
I/O stall that cleared without intervention.

No frame baseline was captured by this lane. Its implementation commit leaves
the frame ratchet red for convergence.
