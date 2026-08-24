# Direct permissionless matching contract

`dclutch-direct-contract` is safe Rust, `no_std`, `no_alloc`, fixed-layout,
and SDK-free. It owns Direct intent, replay, reservation, settlement, adapter
routing, account-frame, phase, and hostile token-authority semantics. The SBF
adapter remains responsible for decoding the canonical program-owned accounts,
calling native programs, and applying the returned effects atomically.

## One authorization and replay owner

A maker signs one exact 232-byte `DirectIntentV2`. It binds Market, generation,
maker Ed25519 key, gap-free nonce, inclusive slot interval, side, execution
lifecycle, outcome, aggregate fill capacity, price limit, fee configuration and
rate, native Position account, and collateral token account. The lifecycle is
inline fill-or-kill, inline immediate-or-cancel, or registered resting.

Every signed path uses the same cross-instruction Ed25519 form. Public keys and
signatures occur in the immediately preceding pinned native Ed25519 instruction;
descriptors point to nonoverlapping exact preimages in the following Direct
instruction. The parser checks program ID, adjacency, descriptor count/order,
all instruction indices and offsets, exact total length, no trailing bytes,
signer, and message. A caller supplies no authorization attestation.

Both lifecycles consume one 144-byte replay root for `(Market, generation,
maker)`:

1. Inline FOK/IOC consumes the exact next nonce without incrementing live count.
   It creates no intent record or collateral escrow. The first inline action may
   atomically create an absent canonical root at nonce zero using a separate
   System payer; subsequent actions use the same root. FOK requires
   `fill == max_fill`; IOC accepts a positive smaller fill and discards the
   remainder.
2. Registered resting consumes the exact next nonce, increments live count, and
   creates a 304-byte live record. A Sell moves `max_fill` native claims from
   the signed Position into record custody. A Buy moves
   `floor(max_fill * limit / PRICE_SCALE)` plus the maximum floor fee into a
   record-associated token escrow. Partial fills consume only this custody.

There is no per-order terminal tombstone. Full fill, cancellation, or expiry
closes the live record and decrements root live count. A closed nonce is below
the root high-water mark, and two actions racing the same next nonce contend for
the same writable root. Nonces cannot skip. `u64::MAX` refuses because the
high-water mark cannot advance.

The maker also has an O(1) kill switch. `CancelThroughV1` signs a strictly
increasing `minimum_live_nonce` no greater than `next_registration_nonce`.
Applying it updates only the replay root; no service enumerates or cancels
records. Every settlement rejects a record below the threshold, and anyone may
subsequently call the side-specific invalidated-record close to return remaining
claims or collateral and rent to the record's stored destinations. Each physical
close decrements live count, so root retirement still proves that no custody is
stranded.

## Phase and trusted slot authority

The SBF adapter decodes the canonical Market account and projects its phase into
the Direct phase checker. Direct does not persist a second Market phase.

| Actions | Admitted canonical phase |
|---|---|
| Register, ordinary, split, merge, all inline | Open only |
| Cancel-through, cancel, expire, close invalidated | Open, Resolved, or Retiring |
| Close replay registration, close replay root | Retiring only |
| Any Direct action | Never Founding or Retired |

Every route needing a slot uses the trusted return from `Clock::get()`. Slot is
not instruction data and Clock is not an account meta. Registration and every
fill require `valid_from_slot <= Clock.slot <= valid_through_slot`. Expiry is
permissionless only when `Clock.slot > valid_through_slot`. Cancellation does
not need a slot and remains available throughout every permitted unwind phase.

`CloseReplayRegistration` irreversibly marks the root terminal during Retiring.
`CloseReplayRoot` additionally requires that terminal mark and `live_count == 0`.
Its close effect retains the final nonce high-water for retirement evidence; a
caller-authored retirement boolean is never accepted.

## Token and native-claim custody

Off-chain Ed25519 authorization cannot itself debit an SPL token account. The
chosen preauthorization is the existing per-maker replay-root PDA as the exact
Token/Token-2022 delegate on the signed Buy source account. Before a Buy debit,
the adapter decodes the token account and requires:

- source address equals the signed collateral account;
- token owner equals the signed maker;
- mint equals the immutable Realm collateral mint;
- delegate equals the canonical replay-root PDA; and
- delegated allowance equals the one exact atomic debit.

The replay-root seeds sign the CPI. Registration consumes the exact maximum
reserve; inline execution consumes its exact gross plus floor fee. An absent
first-use root is created before the debit in the same atomic instruction. Sell
registration and registered Sell matching never use this token delegate: native
claims are reserved into the live record at registration.

A registered Buy escrow is a token account whose authority is the exact live-
record PDA. Every release and final token close uses the live-record seeds.
Full fill, cancellation, and expiry close both the Buy escrow and live record.
Sell close returns remaining claims to the signed Position; Buy close returns
unused collateral to the signed maker account.

Ordinary matching transfers from one Buy escrow to the signed seller and fee
recipient; it does not touch the Market vault. Complete-set split and merge are
the only vault routes. They carry the canonical Market vault and its `Custody`
PDA. Split/merge mark Market writable because complete-set issuance/retirement
changes the Market ledger. Realm and collateral mint occur only on routes with
token semantics. Templates remain offchain bundles of native outcome claims,
not a second Direct liability basis.

## Rent and RentCredit

System creation payer, Position owner, maker, and matcher are distinct roles.
Every root/record/escrow creation frame carries the separate System payer plus a
pre-existing canonical RentCredit for that beneficiary. The root and live
record persist their original payer identity; a Buy record and escrow share one
beneficiary.

Every full fill, cancellation, expiry, invalidated-record unwind, and root close
classifies each closed program or token account independently.
`terminal_rent_transition_v2` names the authenticated rent principal and every
excess lamport as an `unclassified_donation`; their sum goes to the persisted
refund authority's canonical RentCredit. Nothing goes to the closer or an
arbitrary sink, and donated lamports are not called fees, rent, revenue, or
reserve capital.

`dclutch-rent-contract` is the sole owner of that permanent credit. Direct
hostile-decodes its exact 48-byte `RentCreditV1`, validates the immutable
`RefundAuthority` and persisted bump, and projects the actual PDA seeds
`[b"dclutch/rent-credit/v1", refund_authority, bump]`. The SBF adapter verifies
the Rent program owner and derives the supplied key from that projection.
`terminal_rent_credit_close_plan_v1` then delegates balance conservation to
`SourceCloseCreditPlanV1`: the complete observed source balance must be the
exact credit delta, the source post-balance must be zero, and the observed
credit post-balance must match. The 32-byte payer fields in Direct roots and
records are therefore immutable refund-authority identities, never arbitrary
payout addresses.

## Fixed layouts and instruction data

| Record | Bytes | Equation |
|---|---:|---|
| Signed intent | 232 | header 16 + economic facts 152 + Position 32 + collateral account 32 |
| Maker replay root | 144 | header 16 + Market 32 + generation 8 + maker 32 + next nonce 8 + live count 8 + minimum-live nonce 8 + payer 32 |
| Live intent | 304 | header 16 + intent 232 + filled 8 + reserved claims 8 + reserved collateral 8 + payer 32 |
| Cancel message | 96 | header 16 + Market 32 + generation 8 + maker 32 + nonce 8 |
| Venue fee policy | 120 | header 16 + Market 32 + generation 8 + config 32 + recipient 32 |

The common adapter header is 16 bytes. Registration is `16 + 232 = 248`;
cancellation is `16 + 96 = 112`; expiry and replay closes are 16; registered
ordinary is 34; registered split/merge are `24 + 9N`; inline ordinary is
`34 + 2*232 = 498`; inline complementary is `24 + 241N`.

## Exact action frames

The public hostile router validates magic, schema, action, participant count,
and zero reserved bytes before dispatching to an exact-width action codec.
There is no generic header-only close path: registration close and root close
have separate public codecs.

| Action | Instruction accounts | Required shape summary |
|---|---:|---|
| Register Buy | 15 | System payer, RentCredit, Market, Realm, policy, root, record, escrow, Position, source, mint, token, System, Rent, Instructions |
| Register Sell | 10 | System payer, RentCredit, Market, policy, root, record, Position, System, Rent, Instructions |
| Cancel Buy | 13 | Market, Realm, root, record, escrow, Position, refund source, RentCredit, mint, token, System, Rent, Instructions |
| Cancel Sell | 8 | Market, root, record, Position, RentCredit, System, Rent, Instructions |
| Expire Buy | 12 | Cancel Buy without Instructions |
| Expire Sell | 7 | Cancel Sell without Instructions |
| Ordinary | 19 | token base 8 + persisted Sell 5 + persisted Buy 6 |
| Split N | `10 + 6N` | writable Market/vault, Realm/mint/token/Custody base + N Buy custody groups |
| Merge N | `10 + 5N` | writable Market/vault, Realm/mint/token/Custody base + N Sell custody groups |
| Close registration | 2 | writable Market and replay root |
| Close root | 5 | Market, root, RentCredit, System, Rent |
| Cancel through | 3 | Market, replay root, Instructions |
| Close invalidated Buy | 12 | Expire Buy frame; no Clock meta exists |
| Close invalidated Sell | 7 | Expire Sell frame; no Clock meta exists |
| Inline ordinary | 17 | creation/signature/token base 11 + two root/Position/collateral groups |
| Inline split/merge N=2 | `13 + 3N = 19` | inline base adds writable vault and Custody |

Every inline route carries InstructionsSysvar. No route carries Clock. RentCredit
aliases are permitted only with other RentCredit roles; unsafe cross-role and
participant aliases refuse.

## Measured Solana v0 envelope

The physical numbers were recomputed after the frame correction. The isolated
measurement uses exact `solana-sdk = 3.0.0`, `solana-packet = 3.0.0`, transitive
`solana-transaction = 3.1.0`, and `bincode = 1.3.3`. It constructs actual
`v0::Message::try_compile` messages and serializes `VersionedTransaction`s with
one fee-payer signature, one ALT containing non-signer metas, invoked programs
static, exact Direct data, and the exact native Ed25519 instruction where used.
There is no compute-budget padding. The pinned ceilings are 1,232 packet bytes
and 128 loaded-account locks.

| Action | Ix accounts | Direct data | Ed25519 data | Serialized bytes | Locks |
|---|---:|---:|---:|---:|---:|
| Register Buy | 15 | 248 | 112 | 630 | 17 |
| Register Sell | 10 | 248 | 112 | 620 | 12 |
| Cancel Buy | 13 | 112 | 112 | 521 | 16 |
| Cancel Sell | 8 | 112 | 112 | 511 | 11 |
| Expire Buy | 12 | 16 | — | 276 | 14 |
| Expire Sell | 7 | 16 | — | 266 | 9 |
| Ordinary | 19 | 34 | — | 308 | 21 |
| Close registration | 2 | 16 | — | 256 | 4 |
| Close root | 5 | 16 | — | 262 | 7 |
| Cancel through | 3 | 112 | 112 | 501 | 6 |
| Close invalidated Buy | 12 | 16 | — | 276 | 14 |
| Close invalidated Sell | 7 | 16 | — | 266 | 9 |
| Inline ordinary | 17 | 498 | 222 | 995 | 19 |
| Inline complement N=2 | 19 | 506 | 222 | 1,007 | 21 |

Registered complete-set geometry remains within both physical limits:

| N | Split accounts | Split bytes | Split locks | Merge accounts | Merge bytes | Merge locks |
|---:|---:|---:|---:|---:|---:|---:|
| 2 | 22 | 322 | 24 | 20 | 318 | 22 |
| 3 | 28 | 343 | 30 | 25 | 337 | 27 |
| 4 | 34 | 364 | 36 | 30 | 356 | 32 |
| 5 | 40 | 385 | 42 | 35 | 375 | 37 |
| 6 | 46 | 406 | 48 | 40 | 394 | 42 |
| 7 | 52 | 427 | 54 | 45 | 413 | 47 |
| 8 | 58 | 448 | 60 | 50 | 432 | 52 |
| 9 | 64 | 469 | 66 | 55 | 451 | 57 |
| 10 | 70 | 490 | 72 | 60 | 470 | 62 |
| 11 | 76 | 511 | 78 | 65 | 489 | 67 |
| 12 | 82 | 533 | 84 | 70 | 509 | 72 |
| 13 | 88 | 554 | 90 | 75 | 528 | 77 |
| 14 | 94 | 575 | 96 | 80 | 547 | 82 |
| 15 | 100 | 596 | 102 | 85 | 566 | 87 |
| 16 | 106 | 617 | 108 | 90 | 585 | 92 |

Inline complementary N=2 fits at 1,007 bytes. N=3 is 1,364 bytes and refuses.
The exact N=2..16 reference sequence is `1007, 1364, 1721, 2078, 2435, 2792,
3149, 3506, 3863, 4220, 4577, 4934, 5291, 5648, 6005`. Independently, even a
perfectly shared message leaves N=16 native Ed25519 scaffolding at
`2 + 16*(14+32+64) = 1,762` bytes before message or transaction framing.
Registered custody is therefore required for complementary N>=3. Ordinary and
N=2 retain intentional rent-free inline execution.

The adapter must still inspect the live serialized transaction and refuse above
1,232 bytes or 128 locks. Adding compute-budget instructions or changing ALT
placement changes bytes; these values define the pinned reference profile, not
permission to skip live physical admission.

## Remaining SBF seam

The kernel now describes sufficient action routing, account roles, phase/slot
policy, source delegation, escrow authority, custody, RentCredit beneficiary,
and close effects, but it does not execute them. The SBF adapter must still:

- decode canonical Market/Realm/Position/policy/root/record state and PDA bumps;
- use trusted `Clock::get()`, decode the 48-byte `RentCreditV1`, verify its
  `dclutch-rent-contract` owner, and derive its key from the projected domain,
  refund authority, and persisted bump;
- inspect InstructionsSysvar and require successful adjacent native Ed25519;
- decode Token/Token-2022 owner, mint, delegate, allowance, escrow, vault, and
  Custody authority fields;
- create/close accounts, invoke token transfers with exact PDA seeds, classify
  account and token-account rent independently, then apply the exact
  `SourceCloseCreditPlanV1` source-zero/credit-delta postconditions; and
- apply Market/Position/root/record/token effects with rollback on any refusal
  or failed CPI.

Until that adapter exists and is exercised, Direct is not deployed or an
end-to-end trading implementation.
