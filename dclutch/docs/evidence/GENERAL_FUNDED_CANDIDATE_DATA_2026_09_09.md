# General funded Candidate data authority — 2026-09-09

The v42 warm continuation reached SubmitCandidate after actual Place, Close, replay refusal and a second Open. Accelerator evaluation succeeded, then Trading returned its Transition refusal. A bounded v43 diagnostic enabled seven existing post-candidate checkpoints: candidate, postchecks, borrowed coverage, lifecycle replan, replan agreement and observation release all completed; Effect projection did not.

The projection's funding guard allowed data writes only when Funding itself created the state. General SubmitCandidate deliberately uses authenticated Lifecycle Create for identity/rent and Funding Fund for its work escrow. The guard therefore rejected that newly created Candidate's first header write.

Fund now permits account-data writes only when the revalidated prepared lifecycle table creates the exact funding state. Funding alone, Authenticate alone and creation of another state convey no such authority. Payer/System data writes and local lamport transfers touching funding remain refused. Lifecycle binding coverage, frame permissions, original funding preflight and funding postconditions remain in force.

## Controls and runtime boundary

The filtered native `funding_fund_allows_only_the_same_lifecycle_created_state_data` control executes the actual V5 Effect projection. Its prepatch positive fails with Trading's Transition error after the negative cases pass (`native-general-funding-created-red.log`). The repaired control passes (`native-general-funding-created-green.log`) and pins state/payer lamports to 25/975 after separate rent creation and top-up from 1,000. The fixture grants local transfer permission so the funding guard, rather than an unrelated permission denial, owns that refusal.

Native Trading check passes; an explicit-package production SBF build compiles the changed Trading program. This is warm diagnostic evidence under `/tank/dregg-build/dclutch-general-delegated-56904d11-20260908`, using the earlier Registry dependency closure, not a current-source cohort or devnet claim.

The v44 production attempt does not reach Submit: Place exhausts the unchanged 1,400,000 CU limit. Compared with v42, its identical Accelerator costs 55,082 CU but begins 10,914 CU later; Custody OpenVault adds 7,500 CU, delegated transfer adds 4,500 CU and Claims adds 1,498 CU. The prior 11,747 CU margin was therefore insufficient across the changed release/PDA geometry. Fund is absent from Place. Full logs are `program-test-delegated-v43-submit-phases.log` and `program-test-delegated-v44-funding-created-production.log`; exact production hashes are `production-v44-sha256.txt`.

No successful live Submit or settlement is claimed for this repair. Native semantics are green; the actual continuation remains behind the Place CU wall. The frame ratchet remains red pending integration capture.
