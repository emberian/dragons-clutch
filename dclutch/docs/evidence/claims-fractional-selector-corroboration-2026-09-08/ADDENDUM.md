# Claims WholeUnwrap native-selector corroboration

Date: 2026-09-08. This is an additive correction to the immutable
[`claims-fractional-validator-2026-09-08`](../claims-fractional-validator-2026-09-08/README.md)
record. It does not alter that campaign's evidence or original ledger.

## Corrected census result

The retained local-validator transaction
`59KbAa5rSzewCS5KsFZD4RNvyhg5X7GAcJUWWnNwSUEeB8wCPjZyPX75YbVne1h1tKJkYBkdSchCxCe2tPLGxKYN`
finalized successfully at slot 197 and invoked Claims program
`Bswb3UyeD1pUTaGiE6WvqwFpJZsQSEY1xhJePCDTHdvp`. Its exact 416-byte Claims
CPI begins with `DCFREQ02`, has schema version 2, and carries action byte `2`
at byte offset 10. The header and tail reserved ranges are zero. Its checked
poststate was shard supply 40, actor shards 0, sleeper shards 40, actor native
claims 996, and reserve native claims 4.

The repaired census derives byte offset 10 from
`FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2` and value 2 from the explicit
`FractionalExposureActionV2::WholeUnwrap` discriminant. Before accepting the
route it also requires the authoritative codec's width, magic, version, and
reserved ranges. The focused source control pins those envelope assumptions to
the shipped Claims codec constants so codec drift fails the test.

[`ledger.json`](ledger.json) therefore adds one exact
`finalized-instruction` observation for
`claims/process_open#WholeUnwrap`. The route is a shared `process_open`
dispatch handler whose selector vector contains both `Wrap` and
`WholeUnwrap`. This observation proves the WholeUnwrap action named by the
binding and its action-2 bytes. It does not prove every action handled by that
match arm. The retained Wrap transaction is reproduced only at its already
corroborated parent route.

`claims/process_terminal#TerminalZeroBurn` receives no observation. Its
inventory selector is now source-derived as action 4 at byte offset 10, but no
TerminalZeroBurn transaction executed in this campaign. In particular, the
shared terminal handler's existing ProgramTest coverage and its
`TerminalRedeem` alternative do not supply accepted local-validator evidence
for TerminalZeroBurn.

## Source and evidence boundary

The evidence-aware host and caller were built at
`a84f5a1972feb18d8ced7085f3cb21506d6f3950`. The protocol ELFs were the sealed
candidate from `551ffc1b99abdf01478e7e963a1ac20a99e6c0f6`. The original record
checks that `programs/dclutch-claims-sbf/src/lib.rs`,
`programs/dclutch-claims-sbf/src/fractional_atomic_v3.rs`, and
`crates/dclutch-claims/src/fractional/request_v2.rs` are byte-identical across
those two source archives. This remains historical source-split
local-validator evidence. It is not final-source, devnet, or mainnet evidence.

The inventory and observer were built from exact census source revision
`ea716cdaa` in
`hbox:/home/hbox/dclutch-census-claims-ea716-20260908`, with checkout-owned
Cargo target `/home/hbox/dclutch-census-claims-ea716-target`, using
`swarm-build`. `inventory --check-unique` reported 164 routes, 456 refusal
codes, and zero unclassified positions. The observer admitted three rows: the
two existing parent-route observations and the newly corroborated WholeUnwrap
child. The explicit zero-route Token-2022 transfer remains checked campaign
evidence without manufacturing a first-party route.

The exact fold was:

```text
dclutch-route-census observe \
  --inventory claims-selector-inventory.json \
  --ledger claims-selector-ledger.json \
  --bindings claims-selector-bindings.json \
  --programs docs/evidence/claims-fractional-validator-2026-09-08/programs.json \
  --evidence docs/evidence/claims-fractional-validator-2026-09-08/native-evidence.json
```

Stored SHA-256 values:

| Artifact | SHA-256 |
| --- | --- |
| [`bindings.json`](bindings.json) | `228af897a872d608d992bb5bffdfff18a4cc8e1cbe10058903af4da7cf698fee` |
| [`census-observe.log`](census-observe.log) | `85877473538706a021a5192da14bd37b99050a888bd0416594de3ce035a02c63` |
| [`inventory.json`](inventory.json) | `14db19f050e3765acfcef4403b696cbd5d9e1d79b5f44ffcb3356279632902aa` |
| [`ledger.json`](ledger.json) | `b63a9b0336a12416b872ef4d0c11e96e6102cb666f2fbfa973e582e838f1499d` |
| [original `native-evidence.json`](../claims-fractional-validator-2026-09-08/native-evidence.json) | `aa2d8d7c37296d8b3fc9b9c02244edb4c3d83c1a9ceca4bd3e6e0ea75a1f1955` |
| [original `programs.json`](../claims-fractional-validator-2026-09-08/programs.json) | `ca4b63a0ad4e78d1e0387ecf960b3d5d2dde1ac4c3d44d86bcd1aabbd54d1329` |
