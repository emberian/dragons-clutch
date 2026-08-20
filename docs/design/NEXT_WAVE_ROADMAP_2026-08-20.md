# The next wave — maturation, sophistication, optimization, assurance

Status: **ROADMAP / PROPOSED.** Conditioned on the DECISION_PACKET's
recommendations as given; every item that assumes a decision names it.
Ordering honors ember's directive: implement everything implementable
**before** complecting the design or finishing formal verification.
Assurance is deliberately last and is a phase, not an afterthought.

## Phase M — maturation (close what exists; no new design surface)

1. **TerminalClosure** (assumes R4 ratified with the scope amendment). The
   convergence point of three independent reports. Close paths for
   ClearWork, CandidateFeed, receipts, FinalPot, EpochWindow, losing/
   superseded candidates; lapse/post-clear reservation release; the reclaim
   target is ~0.4–1.4 SOL per general epoch. Exit: the hostile terminal
   walk extended to the general plane, then one reseal.
2. **Terminal blocking-id retirements** the ratification unlocks (8 of 14
   decision-halves) plus sealing V3 close evidence (retires
   DIRECT.V3_CLOSE_EVIDENCE_UNSEALED; the promotion report's V1 campaign
   covers it in the same measurement pass).
3. **Promotion rung W1 + the V3 syscall-era campaign** (assumes the policy
   freeze). Evidence-only; publishes quotes the admission arithmetic
   already passes.
4. **R2 Phase 0→2 on the calendar spine** (seeded from
   `r2-caps-rebase-trial`); E2 freeze on its evidence trigger; E3 held for
   ember's go against the 12-gate table.
5. **Housekeeping with teeth**: the stale `svm_run.txt` regeneration, the
   two rustdoc private-intra-doc warnings, CI adoption (register F8 — the
   manifest gates are the CI; wiring them into an Actions matrix is now
   cheap and the Pages workflow broke the no-workflows seal).

## Phase S — sophistication (new capability, still fee-less)

1. **PartialFillLedger** (assumes the fee-base shape chosen, since carry
   interacts): partial fills on the general plane — the largest product
   gap the blocker ledgers name. Design first against the relation's
   existing partial_policy plumbing; the receipts machinery generalizes.
2. **VirtualPot**: virtual split/merge summaries entitle and settle;
   retires the freeze-time refusal.
3. **Fee plumbing to the boundary** (assumes RevenuePolicy decided): the
   policy object + record family, the treasury Position path, the carry
   fields in the next reservation version, the fee-bearing sibling policy
   const — everything up to but excluding nonzero rates, gated by the
   eight falsifiers the RevenuePolicy report requires.
4. **Wider campaigns**: 64-tick grids, exact-tie fixtures at scale, a
   second bank profile, multi-market epochs — the evidence that separates
   W1 from W2 and V1 from V2 promotion.
5. **Per-order cancellation / continuous-claims scouting** stays parked
   until the above land (design complection deferred per the directive).

## Phase O — optimization (measured, never speculative)

1. **Fold batching in keeper practice** (the [12,12,8] plan is sealed;
   client/keeper transaction composition, cluster-packet modeling for the
   real wire — the UNMODELED_BANK_TRANSPORT_ONLY caveat's discharge).
2. **opt-z rehabilitation, only if a real rent bill appears**: re-green the
   31 overflowers at z on the current tree (the Tier-0 idiom scales), then
   gate-campaign at its own identity. Explicitly parked otherwise.
3. **CU shaving on the walk's ~250k fixed floor** (codec decode/encode)
   only if promotion economics ever demand it — the Pod fallback remains
   recorded.

## Phase A — assurance (last, by direction)

1. **Adopt ADR-0005** first (it is the destination-fixing act, not a proof
   investment), then the stale-gate cleanups it unlocks.
2. **The correspondence ladder**: extend the digest-pinned review model
   (the Verus shadow's excluded-source discipline generalized) to the
   streaming relation and the walk handlers; Aeneas/Charon spike for the
   no_alloc kernel; the solanalib sBPF semantics scoping.
3. **Lean growth where it pays**: the walk's conservation identity and the
   entitlement atomicity argument as theorems over the emitted objects;
   the composite fee base's characterization formalized before any rate
   freezes.
4. **External assurance**: the STOP-8 human items (license-row review,
   security review, signed tag, second macOS host), and the hostile
   terminal walk as the standing regression floor.

## The dependency picture, one paragraph

R4 ratification opens M1–M2; the policy freeze opens M3; the RevenuePolicy
and fee-base decisions open S3 without touching rates; everything in M and
S composes into at most one reseal per merged wave under the amended
build-path protocol. Nothing in O or A blocks M or S. The faucet blocks
only public exercise — every gate above it runs on the local ladder.
