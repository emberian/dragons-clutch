# Withdrawn General V2 Position asset transfer

Status: **WITHDRAWN DESIGN HISTORY; NOT A CURRENT CODEC OR EXECUTION
AUTHORITY** (2026-08-24).

General V2 family `74/1`, local action 35 `TransferPositionAssets`, remains a
centrally allocated `ReservedDisabled` coordinate. Allocation is not a payload
contract, account contract, capability, or execution route.

An earlier Structured-claim design proposed a public 298-byte payload and a
six-account Position V2 transfer at this coordinate. That design has been
removed. The payload codec, preparation function, caller-selectable custody
authority, duplicate CPI encoder, and action-2/4 wrapper executor no longer
exist. No current caller may construct action 35 as Structured custody
authority, and no checked release admits it.

The current Structured successor owns its lifecycle directly. Actions 3, 5,
6, 7, and 8 compose hostile-authenticated Hoard V2, ClaimLedger V3, Position
V3, purpose-owned Replay V3, Product, collateral, and Token-2022 state in one
family-owned atomic route. Canonical actions 2 and 4 are withdrawn and refused;
they are not aliases for action 35.

Current implementation boundaries are documented in:

- [`programs/structured-claim-adapter/README.md`](../../programs/structured-claim-adapter/README.md)
- [`crates/clutch-structured-claim-runtime-contract/README.md`](../../crates/clutch-structured-claim-runtime-contract/README.md)
- [`programs/structured-claim-sbf/README.md`](../../programs/structured-claim-sbf/README.md)
- [`CENTRAL_INTENT_REGISTRY.md`](CENTRAL_INTENT_REGISTRY.md)

This tombstone is retained so historical links fail closed rather than making
the deleted payload or authority appear current.
