# General V2 Position asset transfer

Status: canonical pure payload and central `ReservedDisabled` action allocation;
no SBF capability or success handler.

General V2 family `74/1`, local action 35 `TransferPositionAssets`, is the base
supply-neutral and Hoard-neutral movement primitive required by structured
claim custody. Allocation is not activation. It must remain disabled until the
PositionV2/Replay adapter and typed custody authorization below are live in the
same source profile.

## Canonical 298-byte payload

The codec owner is `clutch-structured-claim-runtime-contract`, adjacent to
`prepare_atomic_position_asset_transfer_v1`; no `repr(C)` width is used as wire
authority.

```text
market[32]
|| source_owner[32]
|| destination_owner[32]
|| source_generation u64_le
|| destination_generation u64_le
|| source_replay_sequence u64_le
|| destination_replay_sequence u64_le
|| cash_atoms u64_le
|| internal[16] u64_le
|| phase_policy u8
|| authority_kind u8
|| authority_id[32]
```

`phase_policy` is exactly `0=ActiveOnly`, `1=ActiveOrResolved`.
`authority_kind` is exactly `0=OwnerSigner`, `1=TypedCustodyCapability`.
The owner route requires `authority_id == source_owner`. Both authority IDs and
all three Market/owner identities are nonzero, the two owners are distinct,
and at least one cash/native-Egg quantity is nonzero. The authenticated Market
later owns the active outcome width and therefore the required zero padding in
the remainder of the sixteen-entry vector.

## Frozen ordered account contract

0. authority, signer and read-only
1. canonical Market, read-only and program-owned
2. source PositionV2, writable and program-owned
3. source current-generation Replay successor, writable and program-owned
4. destination PositionV2, writable and program-owned
5. destination current-generation Replay successor, writable and program-owned

For `OwnerSigner`, account 0 is the exact semantic source owner. For
`TypedCustodyCapability`, account 0 is the exact `authority_id`, must be a CPI
signer, and must decode/authenticate as the wrapper-product/descriptor PDA
selected by the future custody owner. A caller-provided signer bit or opaque
PDA is not that capability. This second route remains unimplemented and is the
reason action 35 stays disabled.

The eventual handler must authenticate exact PositionV2 and 132-byte Replay
successor codecs, owners, PDAs, stored bumps, Market, generation, sequence,
distinct source/destination accounts, Market phase, and every alias before
calling `prepare_atomic_position_asset_transfer_v1`. It must write both
Position and Replay poststates atomically and independently observe that global
claim supply, token supply, and Hoard collateral did not move.

This primitive performs no wrapper mint/burn, pricing, fee assessment,
treasury credit, settlement, or descriptor lifecycle transition. Those remain
owned by their typed callers and cannot be inferred from action 35.
