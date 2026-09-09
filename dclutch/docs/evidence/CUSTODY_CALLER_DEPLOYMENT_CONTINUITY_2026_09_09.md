# Custody caller deployment continuity — 2026-09-09

## Decision and boundary

The ordinary Custody calling-release route and upkeep protocol-credit route had a real admission gap: both accepted an activated caller's stored Program/ProgramData keys without authenticating the current Loader deployment. An upgraded caller can retain its program id and therefore the exact caller-authority PDA while removing its own upstream release check. The downstream owner must authenticate this continuity.

The relevant ordinary call chain is `process_instruction` → `authenticate_series_aware_common_frame` → `authenticate_common_frame_tail` → `authenticate_calling_release`, all in `programs/dclutch-custody-sbf/src/lib.rs`. Cache identity and caller-authority derivation precede the release check, but neither authenticates the live Loader slot/authority. Claims' `rational_representation_v2::authenticate_execution_releases` authenticates its own release upstream; that self-check cannot bind a replacement caller image. The projected Custody route already used `authenticate_activated_role_v1` for this purpose.

The Registry continuation branch has an additional authenticated witness: Custody's account-frame admission requires a signer; `authenticate_registry_continuation` reproduces its Registry PDA and binds the request/cache digest; Registry `continuation_v1::process` authenticates the role batch before signing that admission PDA. The ordinary route has no such witness. The fix preserves the continuation branch.

## Change

Both vulnerable routes now call Registry's `authenticate_activated_role_in_frame_v1` over the already decoded activation view. This is the canonical read-only frame plus live Loader account ownership, Program→ProgramData link, slot pin and authority authentication. No extra cache decode or Registry CPI is introduced. Ordinary Custody retains `CustodySbfError::ReleaseSuperseded`. Upkeep appends `UpkeepVaultSbfErrorV1::ReleaseSuperseded` to its existing refusal sub-band, with other canonical activation causes logged and mapped to `Release`.

The test caller now propagates a Custody CPI refusal rather than replacing it with its own coarse `CustodyCpi` refusal. Its receipt checks retain their existing error.

## Validation

Native targeted controls invoke both actual admission functions with a fully initialized, activated Registry cache. All six cases execute for each route: accepted unchanged deployment; changed slot; changed authority; wrong Program owner; wrong ProgramData owner; wrong Program→ProgramData link. The slot case names the route's exact `ReleaseSuperseded` discriminant; the remaining hostile cases name its exact `Release` discriminant. The omitted-auth control accepts every hostile and makes both tests red. Restoring the canonical helper makes both green.

The actual SBF entrypoint control invokes `InitializeReplay` through the real caller ELF, with the same caller PDA signature and unchanged executable. Accepted and changed-authority fixture cases require actual Custody depth-two entry. The accepted case creates the Custody-owned replay; refusals leave it absent. ProgramData substitution is fixture evidence, not execution of a native Loader upgrade, devnet evidence, or mainnet evidence.

The higher-slot actual-SBF fixture hit the locked Agave `ProgramCache::assign_program` assertion, `Unexpected replacement of an entry`, during runtime program loading before Custody admission. A backtrace localized this harness boundary. That case is covered by the complete native admission matrix; the actual-SBF gate control uses a same-slot authority substitution. A future local-validator control can execute a real Loader upgrade to establish runtime `ReleaseSuperseded`; this note does not claim that evidence.

Results:

| Row | Accepted code | Omitted-auth control |
| --- | --- | --- |
| Ordinary native matrix (all 6 cases) | pass | all 5 hostiles falsely accepted; test red |
| Upkeep native matrix (all 6 cases) | pass | all 5 hostiles falsely accepted; test red |
| Real-ELF unchanged authority | accepted; replay created | accepted |
| Real-ELF changed authority | exact `CustodySbfError::Release`; replay absent | accepted; test red |

`cargo check --locked -p dclutch-custody-sbf -p dclutch-custody-test-caller-sbf` passed before either narrow SBF link. After the omitted-auth rebuild, the accepted source was restored and both native matrices plus the actual-SBF entrypoint row passed again. No whole package suite ran.

The accepted fixture used 67,568 total CU, including 54,587 Custody CU. The omitted-auth image used 62,173 total CU, including 50,692 Custody CU. These are profile observations, not an isolated helper-cost estimate: changing the Custody ELF changes the release identity and the caller PDA bump draw (the wrapper changed by 1,500 CU). The CU lane's existing `cf-caller-authority`→`cf-calling-release` checkpoints isolate this phase.

## Artifacts and provenance

The live tree was `/Users/ember/dev/dclutch`; HEAD printed before the combined green build was `a95e4edb3cb6c8ff4ac7f7b181defe7f129590c7`, and before the restored control was `2c93ac9d8476498cba23931a8ea81f2ad7314642`. The remote source is a coordinated workspace snapshot including the CU lane's replay/vault bump changes, not a deployment from a commit or a checked release manifest. Registry ELF is the existing sealed `a8e3b4e8c593a5a63eef94a51375f46ba5ad75d2` artifact; only Custody and the test caller were built here.

| ELF | SHA-256 |
| --- | --- |
| Accepted Custody | `d362938a1c9a16324ee6613397d77ee8bdd9dbc2b2c39a4ce4ae9af6e11bb4ff` |
| Omitted-auth Custody | `cc8afdd1cda50f012751bdfccdeae5230dc4f05f404892bb5f9a3a7e95cd05b6` |
| Same test caller in both runs | `c6a50c77d5514713c0120b81cbbd85d8f62ccc6bf69a1417b68f58538a8f99ec` |
| Same Registry in both runs | `48ae0e74a7146dedf4af46c6b97a66a93425ad3d87447f6eae8d0e016fc1608f` |

Accepted snapshot source SHA-256: `src/lib.rs` = `9ebcbbf646aef064dc06140f7cfbb30a974b7b28218dc856f2ec2ec0ef3523b4`; `src/upkeep_vault_v1.rs` = `ef4ca64ef4c017a0ba0c02e2167fae530674ab493592e118fc47f8588f178a09`; `src/delegated.rs` = `7e9201599faf657afc63b7f078463353d797d87ec3261b29f1ea307648bb224f`; test caller = `4ebdf17f377bb3121c611ca9cf2b616d20eb5dd195a8f0037d20216dc5113b89`. `Cargo.lock` SHA-256 = `4a9fe5c9ac40966146b07d1f16903a8bfe03275c1879c0b0721980b301c53ebf`.

Logs and both image sets are retained in `continuity/`: `custody-continuity-sbf-red.log`, `custody-continuity-sbf-green.log`, `custody-continuity-restored.log`, and `custody-continuity-runtime-backtrace.log`; the native complete red matrix is in the SBF red log. Reproduction scripts are `continuity-build.sh` and `continuity-red-build.sh` beside that directory. All heavy work uses `swarm-build` on hbox under `/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909`, with that source checkout's own workspace-root `target`.

## Addendum 2026-09-09 — same-defect caller-admission sweep

The bounded retained-program sweep found one additional omission in Claims `signed_delta_v3::authenticate_releases`, reached directly by the public SignedDelta dispatcher and by the authenticated in-process parent route. Its old comment explicitly delegated current deployment checks to the caller. Claims now authenticates the live caller role through the same canonical in-frame helper, preserving the existing single cache decode and cached Core/Claims state-owner projections. `SignedDeltaSbfErrorV3::ReleaseSuperseded` preserves the actionable slot refusal; other canonical causes are logged and mapped to `Release`.

The targeted native matrix validates the actual Core and Trading caller PDAs before testing accepted metadata, caller slot, caller authority, caller ProgramData owner, and an unrelated Claims counterpart slot. The old predicate accepted every hostile; the repaired predicate refuses the three caller mutations and preserves both controls. The complete public `crate::process_instruction` frame reaches a deliberately empty Claims-state decode only for admitted releases. Deleting the dispatcher's release-gate call makes stale callers reach that downstream `ClaimsState` sentinel and the test red; restoration passes. This proves native entrypoint gate execution, not a successful economic state commit or SBF execution. `cargo check --locked -p dclutch-claims-sbf` and locked workspace metadata passed. No additional SBF cohort was built.

Logs are retained in the same hbox `continuity/` directory: `signed-delta-continuity-red.log`, `signed-delta-continuity-green.log`, and `signed-delta-continuity-dispatch-control.log`; the latter records the public-gate deletion and restored green run. The last control printed live HEAD `c11eb7bc1b60b6722953e66a22afeae90c158394` and `/Users/ember/dev/dclutch`. No other same-pattern caller-admission omission was found in the bounded sweep; cached state-owner projections and checked Registry continuation witnesses were retained.

## Addendum 2026-09-09 — isolated continuity-check cost

The CU lane's `cf-caller-authority` → `cf-calling-release` interval measures **1,947 CU** with continuity restored versus **1,132 CU** with the check omitted: **815 CU per invocation**, or **2,445 CU** across the three Custody calls in the measured General PlaceOrder. Each of the three corrected calls has the same interval. This is Registry V1 diagnostic SBF evidence, separate from the Registry V2 correctness controls above; the complete diagnostic lifecycle remained blocked by General Order poststate decoding. The measurement owner and artifact hashes are in [Custody allocation PDA evidence](CUSTODY_ALLOCATION_PDA_CU_2026_09_09.md).

Compare hbox logs under `/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/`: `program-test-delegated-v36-custody-baseline-profile.log`, `program-test-delegated-v38-custody-continuity-baseline.log`, and `program-test-delegated-v39-custody-bumps-profile.log`. The v38/v39 comparison retains the restored continuity check in both images; whole-transaction CU differences additionally contain PDA key-draw variation and are not the isolated check cost.
