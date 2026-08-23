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
  projected future fees are mechanically inadmissible as liveness capital.

The crate deliberately contains no Solana account tags, PDA recipes, codec,
instruction, CPI, token movement, rate choice, treasury key, capability
profile, or deployment claim. Those are adapter and release work. Old Realms,
policies, reservations, candidates, and settlement routes remain zero-fee and
must refuse this successor shape rather than reinterpret it.

The runtime join still requires a versioned persistent owner/carry host and an
adapter proof that the presented payer envelopes and standing-maker weights
are the exhaustive projections of the selected candidate. A static client or
index is never that authority.
