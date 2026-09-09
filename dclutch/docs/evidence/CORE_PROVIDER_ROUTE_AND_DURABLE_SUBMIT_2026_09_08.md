# Direct provider continuation: Registry lineage and terminal route

2026-09-08. Owner: RESOLUTION-COMPLETION. Local-validator and native-test
evidence; the corrected Core has not yet executed in a checked current cohort.

## Authenticated identities

A Resolution success certificate's `route` names the SourceSpec's
`ProviderReleaseV1` content digest. That Source record, in turn, pins the Pyth
deployment release. `ProviderExecutionRequestV3.provider_release` and the
immediate receipt's corresponding field name the Pyth deployment. The two
identities have separate owners and are ordinarily unequal. This is a dated
semantic clarification of the earlier Pyth-route observation in
`docs/design/EVIDENCE_REFRESH_V1.md` section 7.14.1; that historical observation
is not the current certificate contract.

Core now authenticates the Source prestate and immutable material/spec/provider
record chain before invoking Resolution. Primary selects the material's primary
SourceSpec. Recovery selects the native policy attempt at the Source's active
attempt, requires the request to agree, and authenticates the policy's named
SourceSpec and provider. This happens before resolution clears the active
attempt. The certificate must name this authenticated Source provider; the
provider record must separately name the request's Pyth deployment. Existing
immediate receipt, lifecycle, Source terminal decision and certificate payload
checks remain required.

## Actual held Direct execution

The held validator used the checked eight-program `bf06c8d752eafa572a433b49cb0db38fe413fbbd`
cohort. Its checked gate was
`/tank/dregg-build/dclutch-strict-bf06c8d75-shared-20260908/CHECKED_UPGRADE_GATE.json`,
SHA-256 `e9e575f48ecf13ee80760f284ebc6adf534fd4d3481689fb4b6f6f7786978d5f`.
Market `84NbhnwQoi3tKLHqoDVDJCqHKHtVQmqZwP7SScVZSdFG` had an accepted Direct
trade. The continuation kept that ledger and Market.

| Boundary | Observed result |
| --- | --- |
| Captured signed Pyth VAA verification | Finalized slot 27167, 335,276 CU; signature `2fgPCzjcDf4nZijSpq3aHnwW2xrm1jrYKbqB3dKThpPoXQj1WYSTvZ2UqR4iosfLQUiidTfwh2N9dveTLCjng7pa` |
| Submit with bundled lifecycle funding | Canonical packet planner measured 1,233 bytes against the unchanged 1,232-byte limit |
| Separate lifecycle prepayment | Native 528-byte lifecycle rent: 4,565,760 lamports; signature `4yb8ax1vUjDudGjYNxAbQBWNF9yxTnCPwXW7vV2B7pYPj8VXa4ReueTviP5LivekrTfRg5SXSm1fXCih2Ww8yrwq` |
| Submit using predecessor Registry plan row | Resolution `ResolutionError::FinalizedRecord`; no mutation |
| Submit using authenticated successor Registry selection | Finalized slot 38761, 149,783 CU, 80,000-lamport fee; signature `2jN7x9JPdFZUBe4B27TGdRYWBdv3oybABwh7j2rXEtaJ4sNnBfTxRLLoo4wgPMRGaiXj4K37jUQx9Uckx86pFgY4` |
| Execute | Resolution CPI succeeded, consumed 193,305 CU and returned DCLTPRC3; Core refused `CoreSbfError::ChildAck`. Whole Execute rolled back; no terminal certificate or Accept |

The immediate receipt's 18 identity/header comparisons, request digest and update
digest matched. The remaining certificate comparison equated Source provider
and Pyth deployment identities. The corrected Core is a new executable; this
held cohort is not described as repaired, and no unilateral upgrade was made.

The complete read-only regression capture is
`/tank/dregg-build/dclutch-direct-terminal-6c62d72210-bf06-20260908-run1/core-regression-prestate`.
It contains all 47 Execute accounts from one finalized observation at slot
48178, exact input/checkpoint/signed packet and simulation response, accepted
Submit transaction, account histories, signature statuses, checked gate and host
provenance. Its `SHA256SUMS` digest is
`4ddf2684a64a9fa134f2ff01a29e542c511f5285e1a361a193dc4a0b551e8672`.
The accepted host binary digest was
`970993c9481130aab63b65012e00477573dbe275b310a4a060fe732ce9456b88`.

## Repairs and controls

- `da4614b39` reuses Direct's existing authenticated successor Registry selector
  in the flagship producer. A predecessor plan row is lineage, not authority
  for the current deployment. Existing selector positive/substitution tests
  passed; the same-ledger Submit above is the accepted execution control.
- `6402a5313` makes the checked evidence emitter iterate its existing canonical
  six-label advanceable set. A two-rung campaign had changed the Resolution
  funding ledger, but the emitter still emitted only its old Claims subset.
  Restoring that old loop made the new test fail with three emitted rows versus
  six; restoring the repair passed all 20 filtered refresh tests.
- `59ad1199d` refreshes ladder evidence before producing native input and prepays
  native lifecycle rent separately before Submit. Certificate rent remains a
  separate prepayment before Execute. Transaction fees and compute evidence
  remain included in the campaign's conservation accounting.
- `6f37266a4` corrects Core's authenticated route relationship. Two filtered
  native tests cover primary and second-rung selection with unequal Source/Pyth
  IDs, full certificate validation, substituted source/deployment identities,
  wrong Registry ownership, populated staging, and the wrong active rung.
  Restoring the original deployment comparison made the positive full-certificate
  test fail exactly with `CoreSbfError::ChildAck`; restoring the repair passed
  both tests. The restored source file SHA-256 is
  `1e8135524bd0d8600b70d9c6f8e90015f79f921a13e2079c0a7cc4ef5198bf29`.

These checks used the isolated workspace-root target under
`/tank/dclutch-c17/resolution-activation-diagnostic-0f7-20260908/src`, based on
`0f7d65466da0a3bfd7cb886d0e3e831da2474b14` with explicitly overlaid owned files.
They are qualified overlay checks, not an exact current all-eight release gate.
Filtered native test logs and source inventory are in that checkout's parent.
The combined Core/bootstrap/ladder check exited zero in 1m04s. The Core frame
ratchet remains red until capture against the corrected compiled cohort.

## Remaining execution boundary

A current checked cohort must execute the provider through Accept, reclaim and
complete, followed by holder payout and retirement. A new coherent cohort is
required because the captured Market and activated release records bind the old
Core executable. Rebinding deployment identities in a replay fixture would be
component evidence, not continuation of this immutable held cohort.

For this local campaign only, explicitly authorized tiny `/tmp` journals avoided
measured `/tank` fsync stalls that expired unsigned planned transactions. Each
accepted CLI step was copied to the permanent `/tank` evidence root and hash
verified before the next action. This was a copy/read verification boundary,
not an explicit fsync guarantee, and is not a production durability pattern.
