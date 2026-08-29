# Fractional twin V3: selected-coordinate physics and bounded retirement

Date: 2026-08-28
Status: executable Claims/Trading child rung; **not a live capability**

## Decision

Fractional keeps its own terms-owned `K` shard model. It does not lower through
Rational's receipt recipe.

`FractionalExposureTermsV2` is the sole durable owner of:

- the ordered `K` shard Mint identities;
- the exact integer denominator `D > 1`;
- the Product-`N` to Claims-`K` exposure identity;
- the Product and representation bases.

`divide_exposure_shards_v2` remains the sole quotient/remainder boundary:

```text
whole    = shard_atoms / D
consumed = whole * D
change   = shard_atoms - consumed
```

The division refuses a zero input, a sub-denominator input, an invalid
coordinate, and checked-integer overflow. `change` remains in the same source
Token account under the same Mint. There is no rounding credit, remainder
ledger, or second supply projection.

## Why the Rational child ABI is refused here

Rational's descriptor requires a receipt Mint and a `K`-wide coefficient
vector. Generic Fractional V2 terms contain neither fact. They instead name `K`
independent shard instruments and one exposure bundle. Supplying an arbitrary
coefficient vector or making every coefficient equal to `D` would create a
second Structured receipt recipe with no Fractional semantic owner. It would
also create receipt/custody resources that the Fractional model does not need.

That is the divergence audit required by decision 0011's Option-A rule. The
result is a refusal, not a hand-filled descriptor.

## Selected-coordinate topology

`plan_fractional_physical_v3` binds the exact V2 request to authenticated terms
and produces this closed topology:

| Action | Exact quantity boundary | Child route | Outer authority |
|---|---|---|---|
| Wrap | mint `D * whole`; no remainder | one Claims call atomically mutates native Claims and invokes Token-2022 | holder |
| Transfer | raw same-Mint atoms; no division | ordinary Token-2022 `TransferChecked` | source holder |
| WholeUnwrap | burn `consumed`, keep `change`, return `whole` native Claims | one Claims + Token-2022 atomic call | holder |
| TerminalRedeem | same division; positive payout remains Claims terminal-evaluator-owned | one Claims + Token-2022 + Custody atomic call | holder |
| TerminalZeroBurn | same division; payout is exactly zero | one Claims + Token-2022 atomic call | holder |
| Terminalize | no Token effect | Fractional state only | permissionless |
| V2 ZeroSupplyRetire | refused | use ordered V3 retirement | permissionless |

This module does not accept a caller-supplied payout and does not reproduce the
denominator arithmetic. Transfer intentionally bypasses the family caller: it
changes only Token-owned balances.

## Durable retirement split

The V2 retirement planner expands all `K` Mint closures into one route. That is
not a viable runtime-width topology: an ALT can reduce packet bytes but cannot
raise devnet's 64-account lock limit.

V3 adds a 296-byte exact cursor and a 288-byte exact request with three actions:

1. `Begin` binds release, Market, terms, Token program/behavior, exposure,
   producer root, RentCredit, width, cursor rent principal, and the next root
   revision.
2. `RetireCoordinate` accepts only `coordinate == next_coordinate`, rebinds all
   identities to the immutable terms, authenticates the exact terms-owned Mint
   and Claims reserve, requires both supply and reserve balance to be zero, and
   advances the cursor and revision by exactly one.
3. `Finish` has no `K`-account tail. It is available only at
   `next_coordinate == K` and produces fixed-width terminal evidence before the
   cursor/root and their historical rent principal are closed.

Replay, skip, Mint substitution, nonzero supply, nonzero reserve, missing Mint
or Claims authentication, stale revision, reserved-byte mutation, and
terms/exposure substitution all refuse in the hostile corpus.

The cursor is a progress fact only. Token-2022 remains the owner of Mint supply,
Claims remains the owner of native reserve balances, and Lifecycle Rent remains
the owner of rent refund accounting.

## Exact candidate frame census

`topology_v3.rs` freezes the maximum-distinct account geometry for the missing
caller. The fixture gives every permitted role a distinct key, compiles a
single-instruction v0 message against an activated-table shape, includes the
required 64-byte signature, and asserts exact wire bytes. These are frame
contract numbers, not real-ELF execution evidence:

| Transaction | Unique locks | Instruction data | Fully signed v0 bytes | Packet margin |
|---|---:|---:|---:|---:|
| Wrap / WholeUnwrap | 31 | 417 | 682 | 550 |
| Direct TransferChecked | 5 | 10 | 222 | 1,010 |
| TerminalRedeem / TerminalZeroBurn | 44 | 417 | 708 | 524 |
| Terminalize | 18 | 417 | 656 | 576 |
| Retirement Begin | 8 | 289 | 508 | 724 |
| Retirement Coordinate | 21 | 289 | 534 | 698 |
| Retirement Finish | 10 | 289 | 512 | 720 |

Every transaction is below 64 unique locks and 1,232 bytes. At `K = 256`, the
retirement sequence is one Begin, 256 identical bounded Coordinate
transactions, and one fixed Finish. Width changes transaction count, never a
transaction frame. This is the architectural fix; the ALT is only packet
compression.

## What is implemented

- exact physical action planning with denominator conservation;
- a production Claims handler for the exact 31-account Wrap/WholeUnwrap frame;
- a distinct production Claims handler for the exact 44-account terminal
  frame, deriving its terminal Claims request from authenticated chain state;
- native Claims mutation followed by Token-2022 mint/burn, and terminal
  Claims/Custody settlement followed by Token-2022 burn, so a late child or
  postcondition refusal aborts the enclosing SVM instruction;
- exact 256-byte open and terminal receipts with family-request, Claims,
  Custody, Token poststate, root, quantity, and payout bindings;
- Trading composition admission for all four executable actions, with distinct
  receipt kinds and two authenticated `invoke_signed` authorities: the exact
  request caller PDA and the terms/Market Fractional-root PDA;
- caller-side Claims instruction builders for the 31-account open and
  44-account terminal frames that rederive both PDAs, selected record pairs,
  canonical Claims Positions, selected Mint/Token accounts, terminal exposure
  bytes, and every privilege;
- explicit refusal of the old all-`K` retirement route;
- exact hostile-decoded retirement request and cursor formats;
- strict ordered cursor transitions and fixed-width finish evidence;
- maximum-distinct lock and signed-v0 packet fixtures;
- adversarial conservation, replay, skip, substitution, zero-state,
  authentication, revision, and encoding tests.

## What still blocks a live Fractional capability

The native Claims/Token/Custody child and its Trading composition arm now
exist. Integration still requires:

1. generated EffectProgram, AccountProfile, RequestProfile, and
   ExecutionStrategy artifacts whose exact expanded frame matches the census;
2. the outer Trading transition that advances the sole Fractional-root
   revision only after the verified child receipt, plus its rollback campaign;
3. a migration or new producer-root version that authenticates the V3 cursor
   PDA and its lifecycle rent;
4. one-coordinate Claims Position close plus Token Mint close in the retirement
   step, and fixed final Core/Lifecycle-Rent closure;
5. a real-ELF caller-backed late-Token-failure rollback campaign, frame
   diagnostic, 20-seed CU mean, and
   checked release bindings.

Until those exist, the current 14-action release remains refused and this code
must not be described as an executable or deployed Fractional market.
