# Direct permissionless matching contract

`dclutch-direct-contract` is safe Rust, `no_std`, `no_alloc`, fixed-layout,
and SDK-free. It owns semantic intent, replay, reservation, settlement, and the
hostile-decodable SBF boundary. It does not claim to perform token CPI, PDA
derivation, account-owner authentication, native Ed25519 cryptography, Rent
sysvar reads, or transaction introspection.

## One authorization and replay model

A maker signs the exact 232-byte `DirectIntentV2` preimage. It binds Market,
generation, maker Ed25519 key, gap-free nonce, inclusive slot interval, side,
execution lifecycle, outcome, aggregate fill capacity, price limit, fee config
and rate, native Position account, and collateral token account. The lifecycle
is one of immediate fill-or-kill, immediate-or-cancel, or registered resting.

Every signed path uses one canonical cross-instruction Ed25519 layout. Public
keys and signatures occur once in the immediately preceding pinned native
Ed25519 instruction; exact descriptor offsets point to nonoverlapping intent
bytes in the following Direct instruction. The kernel authenticates program ID,
adjacency, descriptor count/order/offsets/instruction indices, no trailing data,
signers, and messages. This removes duplicate message bytes without accepting a
caller authorization DTO.

There are two intentional state lifecycles under the same signed preimage and
maker replay root:

1. Inline FOK/IOC consumes the exact next nonce without incrementing live count.
   The transaction atomically debits seller Position or buyer token account,
   credits the counterparty/Market vault, pays fees, and discards any IOC
   remainder. It creates no record, escrow, cancellation right, or per-order
   rent. FOK requires `fill == max_fill`; IOC accepts a positive smaller fill.
2. Registered resting consumes the exact next nonce and increments live count.
   In the same atomic registration, the SBF adapter must:

   - authenticate Market, fee policy, native Position, Realm token program/mint,
     canonical replay-root/record/escrow PDAs, and record absence;
   - for a Sell, debit `max_fill` claims from the signed Position into semantic
     record custody; or
   - for a Buy, transfer `floor(max_fill * limit / PRICE_SCALE)` plus the maximum
     floor fee into the record-associated token escrow.

The resulting 304-byte program-owned live record is the resting settlement
authorization. An inline match instead carries the signed intents and the
sealed native evidence in the same transaction. A matcher supplies no owner
attestation and no signature bytes to either semantic checker. Inline and
registered legs cannot mix inside one settlement action, and signed lifecycle
prevents a relayer from turning an intended IOC into a resting order. Partial
fills update only registered records. Completion, exact signed cancellation, or
permissionless close strictly after `valid_through_slot` decrements live count
and closes the live record atomically.

Both lifecycles consume the same `next_registration_nonce`; a nonce won by an
inline transaction cannot later register, and a registered nonce cannot execute
inline. Replay is not one permanent tombstone per order. The 136-byte maker root
is the single monotonic high-water mark. A replayed closed nonce is below
`next_registration_nonce`; two transactions racing the same next nonce contend
for the same writable root and only the first state transition remains valid.
Nonces cannot skip. `u64::MAX` cannot register because the high-water mark could
not advance.

During authenticated Market retirement, the `CloseReplayRegistration` adapter
action must authenticate the actual program-owned Market and generation while
irreversibly closing Market registration and each root's registration status.
The kernel accepts no caller-authored retirement boolean. New registration then
refuses. A root closes only after registration is closed and its live count is
zero; the Market is then unable to recreate a valid registration path. This
Market account/phase join remains part of the SBF seam until that entrypoint is
implemented.

## Custody, fills, and rent

Registered ordinary settlement consumes one reserved Sell and one escrowed Buy. It releases
the exact quote from buyer escrow to the signed seller token account, releases
the floor fee to the policy recipient, and credits the signed buyer Position.
The seller Position was debited at registration and is not debited again.

A registered complementary split consumes N escrowed Buys in canonical outcome order.
Prices sum exactly to `PRICE_SCALE`; exact gross debits sum to `fill`; the Market
vault receives that fill; and each signed Position receives its indexed claim.
A registered complementary merge consumes N claim-reserved Sells in canonical outcome
order. It debits the Market vault by `fill`, releases each exact gross quote less
its floor fee to the signed seller account, and never touches a maker Position a
second time. Inline ordinary performs the corresponding Position/token movements
directly. Inline complementary settlement is admitted only for N=2, the sole
measured complementary width that fits with all native signatures and intents.

Full fill, cancellation, and expiry return unused Buy collateral to the signed
maker token account or remaining Sell claims to the signed Position. The live
record stores its rent payer. A Buy uses that same payer for its escrow account;
closing both returns each authenticated rent principal to that payer. A Sell has
no escrow account and refunds only live-record rent. Lamports above an
authenticated rent principal are donations and go to a neutral sink, never to
the closer. The replay root has its own persisted rent payer and refunds only at
safe Market retirement. Thus the permanent rent cost is one root per active
maker/Market generation, not one tombstone per historical intent.

Quotes use exact scaled integers. `fill * price / PRICE_SCALE` must divide
exactly or refuse. The only rounding boundary is
`floor(gross * fee_basis_points / 10_000)`. `u128` intermediates preserve the
full `u64` input domain and conversion back to `u64` is checked.

## Fixed layouts

| Record | Bytes | Equation |
|---|---:|---|
| Signed intent | 232 | header 16 (including lifecycle) + economic facts 152 + Position 32 + collateral account 32 |
| Maker replay root | 136 | header 16 + Market 32 + generation 8 + maker 32 + next nonce 8 + live count 8 + rent payer 32 |
| Live intent | 304 | header 16 + intent 232 + filled 8 + reserved claims 8 + reserved collateral 8 + rent payer 32 |
| Cancel message | 96 | header 16 + Market 32 + generation 8 + maker 32 + nonce 8 |
| Venue fee policy | 120 | header 16 + Market 32 + generation 8 + config 32 + recipient 32 |

The adapter instruction header is 16 bytes. Registration data is `16 + 232 =
248`; cancellation is `16 + 96 = 112`; registered ordinary is `16 + fill 8 +
price 8 + two mode bytes = 34`; registered split/merge are `24 + 9N`. Inline
ordinary is `34 + 2*232 = 498`; inline complementary is `24 + 9N + 232N =
24 + 241N`.

## Measured Solana v0 envelope

These are measured physical profiles, not provisional limits. A local isolated
measurement program used the repository's pinned `solana-sdk = 3.0.0`,
`solana-packet = 3.0.0`, transitive `solana-transaction = 3.1.0`, and
`bincode = 1.3.3`. It constructed actual `v0::Message::try_compile` messages and
serialized `VersionedTransaction`s with one fee-payer signature, one ALT, the
exact instruction account count/data width, and no compute-budget padding.
`solana-packet 3.0.0::PACKET_DATA_SIZE` is 1,232 bytes and
`solana-transaction 3.1.0::MAX_TX_ACCOUNT_LOCKS` is 128.

- Buy registration: 13 instruction accounts, native Ed25519 data
  `2 + 14 + 32 + 64 = 112`, Direct data 248, 626 serialized bytes.
- Sell registration: 12 instruction accounts, the same 112/248 data, 624 bytes.
- Inline ordinary: 12 instruction accounts, Ed25519 data
  `2 + 2*(14+32+64) = 222`, Direct data 498, 985 bytes.
- Inline complementary N=2: 12 instruction accounts, Ed25519 data 222, Direct
  data `24 + 241*2 = 506`, 993 bytes.
- Ordinary: 15 instruction accounts, 34 data bytes, 268 bytes, 16 total locks
  including the Direct program.
- Split: `6 + 5N` instruction accounts (root, record, escrow, Position, maker
  collateral per Buy), `24 + 9N` data bytes.
- Merge: `6 + 4N` instruction accounts (root, record, Position, maker collateral
  per Sell), `24 + 9N` data bytes.

| N | Split accounts | Split bytes | Merge accounts | Merge bytes |
|---:|---:|---:|---:|---:|
| 2 | 16 | 278 | 14 | 274 |
| 3 | 21 | 297 | 18 | 291 |
| 4 | 26 | 316 | 22 | 308 |
| 5 | 31 | 335 | 26 | 325 |
| 6 | 36 | 354 | 30 | 342 |
| 7 | 41 | 373 | 34 | 359 |
| 8 | 46 | 392 | 38 | 376 |
| 9 | 51 | 411 | 42 | 393 |
| 10 | 56 | 430 | 46 | 410 |
| 11 | 61 | 449 | 50 | 427 |
| 12 | 66 | 469 | 54 | 445 |
| 13 | 71 | 488 | 58 | 462 |
| 14 | 76 | 507 | 62 | 479 |
| 15 | 81 | 526 | 66 | 496 |
| 16 | 86 | 545 | 70 | 513 |

At N=16 the larger registered split has 87 total locks including the program and 545
serialized bytes, below both pinned ceilings. This supports the current
Position N=2..16 profile through persisted authorization.

Inline complementary N=3 is already 1,350 serialized bytes and therefore
refuses; its size grows by 357 bytes for each additional maker under this exact
layout. The measured N=2..16 inline sequence is `993, 1350, 1707, 2064, 2421,
2778, 3135, 3492, 3849, 4206, 4563, 4920, 5277, 5634, 5991`; only the first
entry fits. Even granting all makers one perfectly shared message, N=16 native
Ed25519 scaffolding alone is
`2 + 16 * (14 descriptor + 32 public key + 64 signature) = 1,762` bytes. That
already exceeds the entire 1,232-byte packet before the shared message, v0
framing, fee-payer signature, accounts, or settlement instruction. Sixteen
individual intent messages embedded in Direct data make the measured N=16
transaction 5,991 bytes with lifecycle mode bytes. Registered preregistration
for N>=3 is therefore a product decision forced by the packet envelope, while
ordinary and N=2 retain a rent-free immediate path.

Adding compute-budget instructions or changing lookup/static placement changes
serialized size; the adapter must measure the actual transaction and refuse
above 1,232 bytes or 128 locks. The table is the reproducible reference shape,
not permission to skip live packet admission.

## Remaining SBF seam

The kernel returns exact effects but does not yet execute them. The SBF adapter
must implement the encoded actions and atomically authenticate program owners,
PDA seeds and bumps, Realm token program/mint, Position and fee-policy bytes,
Market/generation/phase, Clock/Rent/instructions sysvars, native Ed25519 success,
Token-2022 account ownership/mint/authority, escrow balance, token transfers,
account create/close/realloc, root write conflicts, and rollback on any failed
CPI. Until that seam exists and is exercised, the contract is not a deployed or
end-to-end trading implementation.
