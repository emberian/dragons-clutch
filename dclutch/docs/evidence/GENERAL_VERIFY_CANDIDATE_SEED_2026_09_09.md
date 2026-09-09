# General Verify Candidate seed projection — 2026-09-09

After the lifecycle forecast repair, the actual diagnostic campaign advanced
past the disabled-plan refusal. It then refused host construction because the
observed Candidate address at coordinate 5 differed from the lifecycle-derived
address. Verify's AccountProfile had no producer for `identity::CANDIDATE`,
although Candidate, Verifier and Result all use that identity in their PDA
recipes. The RequestProfile writes a separate request identity.

Verify now projects the Candidate identity from the existing submitted
Candidate envelope, whose owner the same profile anchors to Trading. The shared
preplan adapter independently joins the request to the decoded submission and
authenticates the Batch, Candidate image and Page. The new operation precedes
the profile's derived common suffix; every declared operation remains total.

## Evidence

Diagnostic base:
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908`.
This is the existing older Registry dependency closure with scoped accepted
source patches, not a checked current-source deployment.

The v60 production diagnostic used freshly built Trading
`e17c97f213ca457c3021767a2429465b404cdc53efb330996dcaa4a528e58d75`
and Accelerator
`80b03c82e3a23f78902ba1db5cc35cf0cd6dc17de82d29fa640edb0d72f8fbab`.
Actual Place consumed 1,352,884 CU with 47,116 remaining under the unchanged
1,400,000 transaction limit; canonical Place poststates passed. Submit consumed
445,282 CU. Verify then refused construction. The v61 marker identified
coordinate 5, observed `Du7NFf6HsMD6eYCGoRjCt8e5hkDEH1VJdqrcM95tdUAj`,
derived `DNeRu6a4uGx6eiXe2P5TEA9asQTG7Z9EPpxU8vsusHTh`.

The new native control executes the published Candidate seed-producing
operations over an actual encoded submission envelope. Before the correction,
the projected identity was 32 zero bytes rather than the submission's identity;
the test failed at that exact assertion. The corrected profile passes the same
control and the existing verifier/evaluator hostile checks. Earlier test setup
compile and owner-anchor failures are not credited as the semantic red.

Four affected profile controls passed: every action's operation table is total,
and every action supplies its root, generation and authenticated domain
registers. Trading SBF, Accelerator SBF and bundle-builder package checks passed
on the combined Candidate projection and separately owned zero-debit Sell fix.
The extracted production operator projector and thin bundle-builder adapter
also passed their package check in this diagnostic archive; the next runtime
pass uses that production implementation.

Logs: `program-test-delegated-v60-verify-guard.log`,
`program-test-delegated-v61-verify-binding.log`,
`native-general-verify-candidate-seed-red.log`,
`native-general-verify-candidate-seed-green.log`,
`native-general-profile-producers-green.log`,
`check-general-verify-sell-closure.log`, and
`check-general-production-projector.log`.

Actual SBF Verify acceptance, guard-flip entrypoint rollback and matched nonzero
settlement remain pending at this record's creation. The two-outcome Buy result
does not settle the wider-width CU failures. This linked source change leaves
the frame ratchet red.
