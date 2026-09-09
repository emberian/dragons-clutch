# General signed Order persistence — 2026-09-09

PlaceOrder's Effect omitted five immutable signed fields: minimum quote credit, side, interval low/high, and claims per lot. Its native candidate projector also wrote record version one while the canonical Order layout and Transition artifact require version two. An accepted instruction consequently left an Order that its canonical decoder refused.

The AccountProfile now projects all five fields from authenticated signed terms. The native projector checks each observation against the decoded signed header with a distinct PlaceOrder clause before mutation. Four shape observations use action-disjoint Selection slots, retaining the existing General bank width; credit already had a slot. The Effect persists every field and sources expiry from the signed expiry register. The existing exact settlement-horizon equality remains required. The native projector uses the canonical Order version.

## Native controls

The new round-trip control executes the real native admission and authored Effect, comparing the entire persisted body with canonical encoding for Buy and Sell at widths 1 and 258. The seller has nonzero credit; the widest fixture uses outcome 257, so endpoints and magnitude cannot be accidentally zero or byte-width values. Five independently substituted observations refuse with their exact clause and leave the bank unchanged.

`native-general-order-body-red.log` proves the old body differs from canonical bytes. Initial fixture runs first found unseeded delegated request coordinates and were not credited as semantic red; those coordinates were seeded from the existing environment before the body comparison was reached. `native-general-order-body-green.log` then passes all 22 scoped Place controls. `native-general-order-profile-encoding.log` passes the published all-action profile encoder, including duplicate-writer checks. Trading and Accelerator package checks pass.

## Actual warm execution and boundary

Evidence lives under `/tank/dregg-build/dclutch-general-delegated-56904d11-20260908`. This warm diagnostic uses the earlier Registry closure plus coordinated patches; it is not a current-source cohort or devnet claim.

In v40 the five missing fields persisted, but version remained one. Its nominal Accelerator SBF command had compiled the workspace default Trading package and copied a stale Accelerator ELF. That run is not evidence of the new evaluator. The corrected command explicitly selects `-p dclutch-accelerator-sbf`; its log names the actual Accelerator compilation and its new ELF SHA-256 is `13ad0f4abffe8212b816b1b1a1533dd210cf0a1b0e0040baad8fd1181b5f0982`.

`program-test-delegated-v41-order-body-production.log` and the v42 continuation record accepted Place at 1,388,253 CU under the unchanged 1,400,000 limit, leaving 11,747 CU. Geometry: two outcomes, one Buy, one lot, one quote atom. Canonical Order decoding, immutable header/state/identity equality, Claims Position ownership and admissions, and Custody escrow poststates pass. Exact ELF hashes are in `production-v41-sha256.txt`.

The same bank then closes the nonempty Batch. Place does not advance root revision; Close advances it from two to three, and the host now asserts that exact ownership split. The old-close replay refuses with Trading's Root error and preserves poststate; a second Batch opens on the same Market. The first subsequent live failure in `program-test-delegated-v42-canonical-order-continuation.log` is Trading's Transition refusal after the SubmitCandidate accelerator returns successfully. Verify and economic settlement remain pending.

The frame ratchet remains red pending integration capture. This narrow headroom does not prove worst-PDA or wider-product CU feasibility. One admitted order and a closed batch are not matched nonzero settlement evidence.
