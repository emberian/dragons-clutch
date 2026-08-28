# Projected founding V2: compact-frame evidence

Date: 2026-08-28

This note records offline build and test evidence for decision 0013. It is not
devnet execution evidence. Its source is the current decision-0013
implementation; its target is the next upgraded devnet generation. No
`DCLTCOR3` Market currently exists on devnet.

## What changed

Ordinary `ProjectFound` remains the one route that authenticates the complete
Registry graph. Its V2 frame has 37 account references. It checks the Realm,
Product, SourceMaterialV3, SourceSpec, capacity profile, optional manipulation
floor, linked basis, capability manifest, activation cache, and the current
Core, Registry, and Rent deployments.

The projected Custody state then owns the authenticated projection used by the
atomic opening route. V2 stores the projected Realm, collateral mint, Source
identity, manifest identity, release set, future Market, and canonical
`principal_cap_sets`. The cap is derived from the authenticated
SourceMaterialV3 policy and the authenticated linked-basis scale; a request
cannot supply a second copy. Zero means absent and refuses, `u64::MAX` means an
explicit or saturated unbounded policy, and every other value is a bounded
number of complete sets. V1 projected states refuse this route.

Compact `Found` has 25 fixed account references. It consumes the authenticated
projected state instead of presenting the Realm, SourceMaterial, SourceSpec,
capacity-profile, manipulation-floor, and linked-basis Registry pairs again.
It still authenticates every record needed to construct the live Market,
Claims, and capability-funding state. The two controller-specific funding
ledgers remain separate: Direct uses a Resolution-owned `0b0111` ledger and a
Trading-owned `0b1000` ledger.

The pre-Market Resolution ledger is created in the separate `DCLTPCB2`
transaction. Resolution reauthenticates the live Trading and Resolution
Program/ProgramData pairs against the activation cache before accepting the
Trading caller PDA or moving funds. Its inner frame is the seven-account live
deployment/funding prefix followed by the exact ordinary `ProjectFound37`
frame, for 44 account references. `DCLTGMF2` invokes no Resolution code and
therefore does not repeat those mutable deployment accounts.

The current source/target Core Market layout is the generated 360-byte
`DCLTCOR3`/version-3 layout. It persists `principal_cap_sets` at offset 288 and
refuses zero. This describes the source tree and the next-generation target,
not current devnet state: zero `DCLTCOR3` Markets exist there today. The older
finalized 352-byte devnet Market generation is retained only as explicit
legacy-refusal evidence; it is not decoded or promoted to the current shape.

## Compiled transaction census

The tests compile the complete bounded-v0 messages with the payer, the same
three ComputeBudget declarations used by the sender, and a canonical address
lookup table. They count static plus loaded keys from the compiled message;
they do not infer the count from account-reference widths.

For a four-entry Direct manifest, `DCLTGMF2` compiles to 59 complete keys and
430 serialized message bytes. Adding five distinct detector keys compiles to
64 keys and 440 bytes. Adding a sixth compiles to 65 keys and 442 bytes, which
the 64-lock admission rule refuses.

`DCLTPCB2` is a separate, tighter wall. Its 90 account references compile to 62
complete keys: 4 static, 7 writable loaded, and 51 readonly loaded. The message
requires two signatures, serializes to 388 bytes, and is a 517-byte fully
signed packet. Adding two distinct keys reaches 64 keys and a 521-byte packet;
adding a third reaches 65 keys and a 523-byte packet. The successor builder
runs this census over the instruction it actually constructed and refuses if
the base instruction silently gains a key.

## Offline gates

- Successor bootstrap: 72 tests passed on the integrated checkpoint.
- Trading projected-bootstrap unit slice: 10 tests passed.
- SDK focused founding and current-reader slice: 64 tests passed.
- Web projection of the same slice: 64 tests passed.
- Market Core generator/formal/codec gate: all stages passed, including the
  generated-source comparison, 11 semantic tests, and 18 lifecycle tests.

The hostile cases cover record substitution, cache/deployment mismatch,
context-slot disagreement across chunked reads, partial projected state, V1
state refusal, funding-ledger reordering, and the 64/65 compiled-message
boundary.
