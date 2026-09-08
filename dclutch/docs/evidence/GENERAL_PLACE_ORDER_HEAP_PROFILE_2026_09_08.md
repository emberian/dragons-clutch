# General PlaceOrder heap-profile diagnostic — 2026-09-08

## Scope and provenance

This is a ProgramTest diagnostic of one attempted General `PlaceOrder`
transaction request, not a release verdict. Both measured PlaceOrder rows
refused before any PlaceOrder poststate. The host source snapshot was
`310080a1e29feef7612ecd6750692689c8916375`, which includes
`d70575d620c253c831a093882fd85a4a6858506e`'s funded-profile seal reuse.
That commit removed the repeated funded lifecycle/profile join that had cost
425,458 Trading CU before the accelerator on the prior failing route; it did
not make an accepted PlaceOrder claim.

The profiled Trading ELF was built from that source at:

```text
/tank/dregg-build/dclutch-681-general-open-batch-20260908/
  trading-310080a1e-profiled/dclutch_trading_sbf.so
sha256 9cf8038f6d2d5d3e10c00fc0de7de91f71b9b9085d1e5f1ad5683011813a2a7b
```

Its build log is `trading-310080a1e-profiled-build.log` in the same evidence
root and records zero compiler frame diagnostics. The test used that fresh
Trading ELF with the previously sealed `681` companion ELFs, so this is an
explicitly mixed diagnostic, not an all-eight closure.

## Identical route, two General frame requests

Both rows used the exact filtered serial host target
`one_founded_market_opens_and_then_closes_its_batch_in_one_bank --exact
--test-threads=1`, the same source, profiled Trading ELF, companion controls,
and PlaceOrder packet (709 wire bytes). Transaction #1 is the accepted
OpenBatch and transaction #2 writes its PlaceOrder seal. Transaction #3 is the
PlaceOrder request being measured.

| Requested General frame | Transaction #3 signature | Result | Trading CU | Boundary reached | Evidence |
| --- | --- | --- | ---: | --- | --- |
| 65,536 | `34oqjtw3WWmTzQTGkDGTSGwA8mf8WWuUkKXJ5HAXAvpXNQHzoistf8wReYmfM5btSwCVFnwDDSbJCVsyn27jkbVD` | `TradingSbfError::HeapExhausted` (`0x4027`, `InstructionError(2, Custom(16423))`) | 1,110,605 | before accelerator; profile frame word `0x10000` | `open-batch-310080a1e-profiled-65536.log`, `open-batch-310080a1e-profiled-65536-evidence/` |
| 131,072 | `4tfxSMqGBmrG6KYWq4ZJpsZ24qTkThzuVYyfyXqDEsNVYDUD8gQtjzGh436zRnrKDKdVSoeSfPH45veGhE8VVjZg` | `GeneralAcceleratorSbfErrorV3::HeapFrameNotRequested` (`0xC006`, `InstructionError(2, Custom(49158))`) | 1,202,688 | `admitted-cpi-buffers`, then accelerator invocation | `open-batch-310080a1e-profiled-131072.log`, `open-batch-310080a1e-profiled-131072-evidence/` |

The 128KiB row recorded General heap profile words `0x10006 / 0xb48e /
0x4b78` against frame `0x20000` at `admitted-cpi-buffers`. It proves the
Trading-side allocator can proceed past the old 64KiB failure for this exact
request. It does not prove the accelerator accepts that profile: its prior
strict matcher required `RequestHeapFrame(65_536)` and named the mismatch as
`0xC006` before child execution.

The successful prior transactions were identical across the two controlled
runs: OpenBatch #1
`52MMHBvpZZULR2xTEBNeTYH9nbR2WLALCXDTY8MhErFQw6wcoT6njmiGR7DjuvtsRAphd2aKhDW8BdcUn1roVDNo`
(808,209 CU, 650 wire bytes), and PlaceOrder seal #2
`5WowXrqUP1CQen1FCpMGReVLgFv4NYMsg1FjGtNVGHjRJ5JhTssMYiDwRVhRx9vw5xsKQcGvawPnCAa8YgWpbPFV`
(946,524 CU, 485 wire bytes).

## Decision carried forward

64KiB is an old measured General profile, not the SVM maximum. The next
coherent General build changes only the General profile to the supported,
1,024-byte-aligned 131,072 bytes and requires that exact request in both the
operator and General accelerator. Direct continues to own and request 65,536
bytes. A fresh matching Trading-plus-accelerator measurement must still show
the post-accelerator outcome and poststate before this diagnostic can become
accepted execution evidence.
