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

## Split state machine

`dclutch-dealer-codec::scenario_checkpoint_v1` now defines the fixed 752-byte
Trading-owned checkpoint required by a lock-bounded route. Its sequence is:

1. Create one request-scoped checkpoint from exact release, Market, root,
   obligation, request, Claims, and obligation prestate commitments.
2. Append six authenticated page receipts in canonical order. A skipped,
   repeated, mixed, or reordered page refuses.
3. Seal one selected admitted evaluation. The seal binds the typed evaluation
   receipt, the best valid submitted candidate bank, the candidate obligation,
   the Claims delta, and the ordered Custody effects.
4. In a separate atomic commit, reauthenticate the request and every mutable
   prestate, execute and verify Claims/Custody children, write the obligation
   last, then close the checkpoint. There is no persisted `Committed` phase.
5. If preparation expires, permissionless cleanup may close the checkpoint but
   can refund rent only to its immutable beneficiary.

The codec tests exercise six-page resume, exact round-trip decoding, replay and
page-mixing refusal, all nine final-commit substitutions, commit-last phase
discipline, expiry, and fixed-beneficiary cleanup.

## Lock gate

The operator now counts resolved execution locks from the payer, every account
meta, and every invoked program before signing or serialization. Its boundary
test admits exactly 64 distinct locks and refuses 65. The unsplit Dealer
instruction is also passed through this gate and refuses at 122 transaction
locks.

Every eventual prepare, evaluate, commit, and cleanup transaction must pass
this same census independently. Adding an ALT does not change the result.

## Capability still missing

This checkpoint is the durable semantic boundary, not the finished route. The
following work remains before `DLR-ACCEPT` can claim an executed row:

- a Trading caller that creates and mutates the checkpoint PDA;
- page producers that authenticate the release/artifact waist, Claims view,
  each Dealer/Custody span, and input-bank pages before emitting receipts;
- a selected-accelerator evaluation receipt joined to those exact pages;
- a final Claims/Custody/obligation executor whose real account list is at most
  64 locks and whose immediate receipts and poststates are verified;
- crash/resume and expiry cleanup ProgramTests against real SBF links;
- exact packet, frame-diagnostic, and 20-seed compute evidence for every new
  shipped link; and
- the AddLiquidity/RemoveLiquidity pool campaign.

Until those callers exist, the checkpoint contract is a staged protocol
primitive and `44048ea7` remains topology evidence only.
