# Dealer Accepted split topology — 2026-08-28

## Result

The old selector-9 frame cannot execute on Solana devnet. Its canonical test
scenario has 122 instruction metas, 121 distinct instruction locks, and 122
transaction locks when the payer is distinct. Its nine protected spans are:

```text
[14, 14, 0, 0, 2, 14, 0, 0, 6]
```

The six final entries are not caller-selected geometry. They are derived from
the same Dealer semantic plan and candidate register bank that determine the
Claims and Custody effects.

An address lookup table can reduce serialized message bytes. It cannot reduce
the number of accounts Solana locks. The 121-lock shape is therefore topology
evidence only; it is not an executable bundle and it is not an Accepted
capability.

Commit `44048ea7` established that unsplit census. The follow-up changes rename
its public projection as an unsplit topology and refuse to describe v0/ALT as a
solution to the runtime lock wall.

The reachable semantic fixture also proves that merely delaying the mutation
does not solve the wall: the Claims, obligation, and six repeated Custody
frames alone still require 75 instruction metas and 77 transaction locks. The
Custody frames must be replaced with durable reservations before the final
atomic activation.

## Split state machine

`dclutch-dealer-codec::scenario_checkpoint_v1` now defines the fixed 944-byte
Trading-owned checkpoint required by a lock-bounded route. Its sequence is:

1. A selected producer derives the complete physical account membership,
   deduplicates and sorts it once, and publishes a typed manifest PDA. The
   reachable fixture has 121 accounts and one unique balanced partition:
   `[21, 20, 20, 20, 20, 20]`.
2. Create one request-scoped checkpoint from exact release, Market, root,
   obligation, request, manifest, and obligation prestate commitments.
3. Append six readonly page receipts in canonical order. Trading checks each
   page against the immutable manifest and enforces strict key ordering both
   within and across page boundaries. Omission, substitution, duplication,
   page mixing, skips, and replay refuse.
4. Seal one selected admitted evaluation. The seal binds the typed evaluation
   receipt, the best valid submitted candidate bank, the candidate obligation,
   the Claims delta, and the ordered Custody effects.
5. Ingest one typed producer-owned reservation receipt for every ordered
   Custody effect. Each receipt binds the checkpoint prestate, request, complete
   effect bank, ordinal, reservation account, and exact reservation transition.
6. In a separate atomic commit, reauthenticate the request and every mutable
   prestate, activate the reservations with Claims, write the obligation last,
   then close the checkpoint. There is no persisted `Committed` phase.
7. If preparation expires, rollback receipts must release reservations in
   reverse order. Only then may permissionless cleanup refund checkpoint rent
   to its immutable beneficiary.

The codec tests exercise six-page resume, hostile manifest and receipt decoding,
replay and page-mixing refusal, final-commit substitutions, ordered reservation,
reverse rollback, commit-last discipline, expiry, and fixed-beneficiary cleanup.

## Executable Trading slice

Trading SBF now dispatches create, page, evaluate, reserve, rollback, and
cleanup routes. The operator builds every unsigned v0 packet and journals
finalized poststates for crash-safe resume. Exact packet evidence is:

| Route | Fully signed bytes | Transaction locks | ALT-loaded addresses |
| --- | ---: | ---: | ---: |
| Create | 607 | 13 | 0 |
| Maximum 48-account page | 409 | 53 | 48 |
| Evaluate | 443 | 10 | 0 |
| Reserve | 344 | 7 | 0 |
| Rollback | 344 | 7 | 0 |
| Cleanup | 278 | 5 | 0 |

The 64/65 boundary test uses resolved transaction locks, not instruction-meta
count. The reachable reserved final projection is 39 instruction metas and 41
transaction locks for three Custody effects. This is topology evidence for the
remaining executor; the executor is not implemented yet.

A fresh real-SBF Trading build produced no overwrite diagnostics. The exact
Dealer checkpoint frames are: checkpoint write 3,840 bytes (256 spare),
reservation/rollback 3,712 (384 spare), evaluation 3,712 (384 spare), page 3,136
(960 spare), and create 2,944 (1,152 spare). The complete 797-frame link still
has an unrelated deepest frame at 4,032 bytes (64 spare).

## Lock gate

The operator now counts resolved execution locks from the payer, every account
meta, and every invoked program before signing or serialization. Its boundary
test admits exactly 64 distinct locks and refuses 65. The unsplit Dealer
instruction is also passed through this gate and refuses at 122 transaction
locks.

Every eventual prepare, evaluate, commit, and cleanup transaction must pass
this same census independently. Adding an ALT does not change the result.

## Capability still missing

This checkpoint is the durable semantic boundary, not finished Dealer
acceptance. The current Trading routes authenticate producer ownership and
PDA/body identity, but they do not yet authenticate either producer through the
release-selected ProgramData/activation artifacts. No Custody SBF route yet
owns reservation state, locks token value, rolls that value back, or batch
activates it. A receipt without that producer is not a reservation.

The smallest remaining executable split is:

1. Add release-authenticated Custody `Reserve`, `Rollback`, and `ActivateBatch`
   routes with one semantic owner for the reservation state and exact token,
   vault, amount, revision, expiry, and replay conservation.
2. Add release authentication to manifest creation, evaluation, and Custody
   receipt ingestion.
3. Implement the 41-lock Trading final route: verify every reservation live,
   execute and verify Claims, batch-activate Custody, write the candidate
   obligation last, and close/refund the checkpoint atomically.
4. Exercise crash, replay, expiry, rollback, and commit under real SBF, then run
   frame diagnostics and 20-seed compute measurement on every shipped link.

The existing aggregate Trading ProgramTest cannot currently resolve its own
dependency graph: its root pins `solana-account = 4.3.2`, while its direct-hot
support crate pins `solana-account = 4.6.0` (and the two also pin different
`solana-program-test` versions). Cargo refuses before compilation. This lane did
not force an incompatible ProgramTest. The strongest feasible execution check
was the fresh real-SBF build plus exact ELF frame scan above.

Until the Custody producer and final executor exist, this is an executable
checkpoint and receipt-ingestion primitive, never an Accepted capability.
