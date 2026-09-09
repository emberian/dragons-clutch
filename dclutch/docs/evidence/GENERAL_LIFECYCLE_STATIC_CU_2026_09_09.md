# General lifecycle immutable facts — 2026-09-09

The native lifecycle adapter now retains its authenticated immutable policy,
profile, action, tail width and Rent snapshot across preplan and replan. It
reuses selected declarations, their fixed widths and rent quotes. Registers,
guards, account observations, RentCredit authentication, seeds, identity
bindings and strict prepared-plan agreement are still evaluated. This matters
because an Accelerator CPI separates the passes and can change account state
or credit writable accounts. Four redundant register-bank copies were removed:
the canonical atomic kernel overwrites scratch and publishes complete output
banks only after success.

The source patch was measured against the unchanged source with the same four
CU checkpoints in General's existing Registry V1 diagnostic cohort. The
integration HEAD when recording these results was
`5c19d2dc44b95813317bccd802babef49f47b85c` in `/Users/ember/dev/dclutch`.
This is local ProgramTest actual-SBF evidence, not a fresh Registry V2 cohort
completion or devnet evidence.

| Place interval | v58 baseline | v59 candidate | Saved CU |
| --- | ---: | ---: | ---: |
| Sealed ownership arena to lifecycle preplan | 65,912 | 63,810 | 2,102 |
| Borrowed witness to lifecycle replan | 62,451 | 33,676 | 28,775 |
| Sum | 128,363 | 97,486 | 30,877 |

Heap used after preplan was 18,857 → 19,225 bytes and after replan
52,624 → 52,992 bytes: 368 additional bytes. Scratch remained 19,448 bytes.
Heap markers followed the phase-end CU markers. Whole profiled Place consumed
1,385,182 → 1,357,149 CU; that whole-transaction difference also includes
changed PDA geometry and is not the exact isolated saving. Candidate Place
had 42,851 CU headroom. Submit consumed 461,492 CU; the later Verify route
retained its existing disabled-plan refusal. This optimization does not claim
to repair that separate semantic boundary.

The paired logs are under
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/`:

- `program-test-delegated-v58-lifecycle-baseline.log`
- `program-test-delegated-v59-lifecycle-static.log`

The candidate package check and explicit Trading SBF link passed. Focused
native controls passed 3/3: exact immutable-input binding, target-width/Rent
quote binding, and atomic kernel refusal without publishing poisoned output
banks. Removing the immutable binding predicate made the first control fail
(red 0/1); restoration passed 3/3. Existing seed/bump/binding/replan agreement
controls passed 6/6. Their retained logs are under
`/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909/continuity/`:
`lifecycle-static-binding-red.log`, `lifecycle-static-restored-green.log`, and
`lifecycle-existing-replan-green.log`.

Frame ratchet rows remain owed to the next inclusive native capture.
