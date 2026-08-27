# Direct Selection Authority V1

Status: **EXECUTABLE STOP; NO LIVE SELECTION OR RECEIPT-FREEZE TRANSITION**

This note answers a narrow question: can the deterministic two-order output of
`SubmitDirectPage` be relabeled `SELECTED`, can the Epoch become `CLEARED`, and
can its one direct receipt be created without adding any other authenticated
state?

No. The order and reservation set is complete in this exact shape, but the
relation and selection authority is not. The program preserves `SUBMITTED` and
`FROZEN` rather than treating structural determinism as an economic verdict.

## Executed counterexamples

`narrow_direct_book_is_not_policy_invariant_selection_authority` drives the
host relation over the exact submitted shape: two distinct owners, opposite
sides of one outcome, equal quantity four, equal price 5,000 on a 10,000 scale,
zero minimum fill, and no virtual flow.

Two frozen-policy choices change consensus facts:

1. Under `FeeBaseV1::None`, the candidate is valid and the buy debit is exactly
   two collateral atoms. Under `FlatNotional { bps: 100 }`, the same two-atom
   limit reservation is insufficient after the named owner-level rounding
   boundary; the relation returns `FeePayerUnfunded`. An opaque policy digest
   therefore cannot authorize even the zero-fee narrow candidate.
2. `PairingWitnessPolicyV1::RecomputedConstructor` and `ExplicitSlices` admit
   different proof shapes. The explicit variant refuses a missing witness, and
   the policy code plus explicit slice changes the canonical relation digest
   even when prices and fills are identical.

The first counterexample changes validity. The second changes proof admission
and identity. Neither is metadata that an adapter may choose after seeing only
`Epoch.policy: Hash32`.

The real-SBF `coupled_authority` test then constructs the canonical Candidate
and CandidateFeed through the live `SubmitDirectPage` instruction. It also
presents well-formed canonical Position, reservation, and receipt fixtures to
the existing `SettlePage` consumer. The program returns `NotActive` because the
Epoch remains `FROZEN` and the Candidate remains `SUBMITTED`; all ten observed
accounts remain byte-identical. This is rollback evidence, not evidence that a
receipt can currently be created.

In the recorded local run, successful prefunded `SubmitDirectPage` consumed
1,249,371 transaction compute units and the refused `SettlePage` authority
attempt consumed 593,728, each under a 1,400,000-unit budget. The real-SBF ELF
was 809,824 bytes with SHA-256
`c8ff4ac7286004cb5d897cc92b05f7a9e386107d295cb1441adcd227e0b35138`.
That digest names the joined dirty working tree used by the local test, not a
release or deployment artifact.

## Executable dependency order

`SETTLEMENT_BLOCKERS` now reports the first unavailable fact rather than the
circular label “candidate selection”:

1. `FrozenPolicyPreimage`;
2. `FullWidthRelationDomain`;
3. `CandidateWindowClosure`;
4. `EntitlementFreeze`;
5. `GeneralReservationSetClosure`;
6. `PartialFillLedger`;
7. `VirtualPot`; and
8. `TerminalClosure`.

The exact narrow constructor discharges item 5 locally: its authenticated page
contains exactly two live orders, no tombstones, and each order binds one exact
untouched ACTIVE reservation. That does not discharge the earlier three items,
and it does not create item 4.

## Minimum immutable BatchPolicy artifact

The next honest policy owner is a versioned, fixed-layout, immutable artifact.
Its canonical body must name every `FrozenPolicyV1` field explicitly:

```text
tag, schema_version, relation_version,
allocation, self_cross, aon, rounding,
residual_settlement, transfer_phase, portfolio_lots,
pairing_witness, dust, score, fee_base_kind, fee_bps,
reserved_zero_bytes
```

Requirements:

- there is no default constructor and no omitted selector;
- enum discriminants and the conditional `fee_bps` field are decoded
  canonically, with non-applicable bytes required to be zero;
- the semantic digest is domain-separated SHA-256 over the exact canonical
  body, and must equal `Epoch.policy`;
- creation uses the existing staged-artifact lifecycle or an equivalently typed
  codec, never a generic evidence blob;
- the final content PDA is immutable, owner-checked, exact-length, and created
  with the prefund-safe signed `Allocate` plus `Assign` path; and
- decoding produces the relation's complete `FrozenPolicyV1` without adapter
  convention.

Adding only this artifact is useful but not sufficient to select.

## Minimum full-width relation domain

Account truth uses 32-byte identities for market, book, Epoch, policy, and
order set. Relation V1 currently takes a distinct five-`u64` domain. Truncation,
folding, and independently assigned lookup numbers are not injective and are
therefore forbidden.

The next relation revision must consume the complete 32-byte identities in its
candidate digest, or consume a cryptographic commitment to the complete typed
domain whose preimage is checked in the same transition. This is a relation
revision with new golden vectors, not an adapter cast. `Epoch.relation_version`
must select it explicitly; relation V1 bytes must retain their old meaning.

## Minimum candidate-window closure

A content-derived Candidate PDA proves uniqueness of that content, not closure
of the submitted set. “Best valid submitted candidate” requires one semantic
owner for at least:

```text
epoch, order_set, policy, relation_version,
opened_slot, closes_slot,
submission_count, ordered_submission_commitment,
state = OPEN | CLOSED | SELECTED,
selected_candidate, stored_bump, reserved_zero_bytes
```

Every candidate creation must atomically register exactly one canonical entry
while the window is `OPEN`. Closing requires the authenticated Clock at or
after `closes_slot`, freezes the ordered commitment and count once, and rejects
all later registration. Verification must bind every registered Candidate and
feed to the frozen policy, full-width relation domain, complete book, and exact
reservation set. Selection compares only valid registered candidates under the
relation score and records exactly one winner. A zero-valid-candidate window
must take an explicit lapse path rather than manufacture a winner.

For the two-order specialization, a future codec may prove that exactly one
canonical candidate was registered. It may not infer that fact from the
existence of one PDA or from the absence of another account in one transaction.

## Atomic selection and entitlement freeze

Only after all three authority prerequisites exist may one transition:

1. complete and verify the Candidate score and relation digest;
2. close the candidate window and mark exactly one Candidate `SELECTED`;
3. create the complete canonical receipt set, prefund-safely;
4. prove every selected fill is covered exactly once by those receipts;
5. preserve every unfilled reservation refund entitlement;
6. change the Epoch from `FROZEN` to `CLEARED`; and
7. write all affected accounts atomically, so any late create/encode failure
   rolls back the selection, receipts, and phase change together.

For the currently implemented narrow path, the complete set would contain one
direct receipt at slice index zero. General partials, portfolios, fees, virtual
pots, lapse, and terminal closure remain separate STOPs and are not smuggled
through this specialization.
