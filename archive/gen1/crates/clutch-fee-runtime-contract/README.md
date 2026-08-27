# Clutch fee runtime contract

Status: **PURE ACCOUNT-NEUTRAL SUCCESSOR CONTRACT / NO LIVE ROUTE / NO
PRODUCTION RATE OR TREASURY** (2026-08-23).

This allocation-free, `no_std`, safe-Rust crate owns the economic joins a
fee-bearing adapter must implement without weakening today's zero-fee
refusals:

- `SelectedCompositeFeeV1` binds one canonical fee-record identity and
  selected candidate to exact nonzero composite rates, batch-policy digest,
  revenue-policy digest, real treasury owner and ordinary treasury Position,
  price scale, outcome width, and the relation-derived `u128` denominator;
- `OwnerFeeCarryV1` is the sole composite carry owner: it binds one owner to
  that fee record, validates restored `u128` state, calls the batch relation's
  owner-wide composite kernel, and makes the floor/terminal-ceil transition;
- `allocate_payer_debit` deterministically partitions the owner-level debit
  across that same owner's strictly identity-ordered signed intent envelopes,
  never above an individual or aggregate bound;
- `allocate_recipients` rebinds the selected revenue-policy preimage, refuses
  a nonzero executor share until an executor identity exists, and assigns the
  maker pool by Hamilton largest remainder over candidate-verified standing
  weights, with Position identity as the total tie-break;
- `TreasuryLedgerV1` validates restored state, conserves credited, available,
  and withdrawn atoms, refuses close while any fee-bearing epoch is
  outstanding, and credits only the treasury residual of an exactly validated
  fee settlement classified as a collected trading fee; and
- settlement, redemption, and liveness integration structs require exact
  conservation, zero redemption rake, unchanged Hoard collateral, and present
  independently prepaid lamports. Hoard principal, redemption principal, and
  projected future fees are mechanically inadmissible as liveness capital; and
- terminal contracts make the existing owner carry PDA a one-way immutable
  finalization receipt, delete its payer-allocation snapshot atomically, and
  produce candidate-wide settled/abort plus closure-manifest receipts before
  retiring selected, recipient, treasury, and temporary owner fee accounts.

The owner-settlement bridge consumes the authenticated terminal owner carry,
recomputes the terminal payer allocation from signed reservation envelopes,
and proves cumulative envelope debits equal the closed carry total. It emits
exactly one `clutch_owner_settlement::SelectedOwnerFeeV1` per lexicographically
ordered participating owner. Seller-only owners are explicit zero rows; every
positive row must fit that owner's aggregate buy cash reservation. The complete
book must equal the selected candidate's `selected_fee_atoms` before recipient
allocation or treasury credit.

[`SCHEMA.md`](SCHEMA.md) freezes the account-neutral inner codec widths,
discriminators, typed action joins, and mutation owners. Apart from the
centrally reserved owner-carry `0x83` version transition described below,
outer SBF account tags, PDA seeds, actions, and capability profile remain
unallocated.

The crate deliberately contains no Solana PDA recipes,
instruction, CPI, token movement, rate choice, treasury key, capability
profile, or deployment claim. Those are adapter and release work. Old Realms,
policies, reservations, candidates, and settlement routes remain zero-fee and
must refuse this successor shape rather than reinterpret it.

The central adapter reservation for the owner carry is outer tag `0x83`:
version `1` contains the 128-byte `DCFEECRY` body; version `2` contains the
496-byte `DCFEEFIN` body plus the unchanged bump/flags suffix, for an exact
500-byte outer account. The PDA remains `(fee record, owner)`. Version `1` is
never reinterpreted as version `2`; the adapter must perform an authenticated
in-place realloc and transition. The associated payer-allocation account is
closed in that same action and has no independent receipt lifetime.

The realloc delta is exactly `max(v2 rent minimum - observed carry balance,
0)`. Its top-up payer must be the existing carry rent-principal refund owner.
Existing hostile prefunding remains donation, never new refundable principal;
the deleted payer snapshot refunds its own principal owner and sends donation
only to the neutral sink. The owner receipt persists the authenticated digest
of that complete rent-ledger transition, not a second rent-receipt account.

Candidate-wide `DCFEEEND` and `DCFEECLS` bodies are still account-neutral: the
central adapter has not allocated outer tags or PDA recipes for them. The
closure manifest commits the authenticated digest of the canonical ordered
closure set. Its refund/donation totals cover accounts closed by that terminal
action; each earlier payer-snapshot close remains transitively committed by
its owner's `DCFEEFIN` rent-disposition data digest.

The runtime join still requires adapter proof that payer envelopes,
standing-maker weights, exact row bytes, account-data digests, and closure-set
digests are authoritative projections of the selected candidate. A static
client or index is never that authority.
