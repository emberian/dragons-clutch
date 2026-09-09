# Claims protocol Position V2 real-ELF controls — 2026-09-09

## Result

Three separately filtered `solana-program-test` controls passed against the
sealed Claims/Core/Registry/Rent ELFs from exact production source
`fd7e5fec68cb948a326b41dc69be76f27156da10`. They execute protocol Position
admission and close, the fractional retirement walk, exact refusals, rollback,
account closure and lamport conservation.

This is installed-state ProgramTest evidence. The fixture installs an Open
Core/Product/LBV2 graph and changes Core to Terminal with `set_account` between
transactions. It does not fund a Market, trade, or execute Resolution, and is
not a full economic lifecycle or local-validator result.

## Exact runtime and host boundary

The production cohort was the independently checked all-eight run at:

```text
/tank/dregg-build/dclutch-strict-fd7e5fec6-v2-no-general-owner-20260909-run2
```

Its release driver exited zero, SBF freshness admitted all eight links, stack
diagnostics were zero, and two frame captures admitted the same baseline. The
checked release manifest SHA-256 is
`5d6609c418145cfe88f50f8f7d6ffd033bc776c045452923143f2bbddc3ff1c9`.
Claims measured at most 3,904 of 4,096 frame bytes across 306 frames.

The controls load these exact artifacts:

| role | SHA-256 |
| --- | --- |
| Claims production ELF, 1,460,240 bytes | `0a4b3527076bd0f32963dcbb54d06ad2361f1831bcb7d576532975dca454afb6` |
| Core production ELF | `d242da365eac4e7f6cea239b2649d2dd9ab6d358bf38be55974ccb09f30a492d` |
| Registry production ELF | `48ae0e74a7146dedf4af46c6b97a66a93425ad3d87447f6eae8d0e016fc1608f` |
| Rent production ELF | `6d00dfde5c764c7039e58df784ec91a88aa216e513a9f5fb79815a10cc358ac7` |
| exact-source test caller | `49ebc6776bf7590de5956a34e21e24a15e6be2f781d7697ee049547f71b7d552` |
| canonical Token-2022 v11 ELF | `e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697` |

The auxiliary caller was built from exact `fd7e5fec6` source through
`swarm-build`; its build log has zero frame diagnostics and no scheduler-no-op
marker. The host checkout was the same exact source plus only the fixture patch
committed as `01784cb00ab0f6b1c556409b53b8d6eef32ee1ea`. Its source and Cargo target
were private to this campaign.

Each row ran alone with `--test-threads=1`:

```text
SBF_OUT_DIR=<exact-artifact-map> CARGO_TARGET_DIR=<checkout-owned-target> \
  swarm-build cargo test --locked --offline \
  -p dclutch-claims-protocol-position-program-test --test lifecycle \
  <exact-test-name> -- --nocapture --test-threads=1
```

## First blocker and repair

The first accepted run reached the Claims ELF and refused at exact `0x5146`
(`ProtocolPositionSbfErrorV2::Rent`) after 103,126 Claims CU. Retirement never
ran. The fixture had compiled Core with a provisional wallet as its immutable
rent beneficiary, then derived and installed a different lifecycle RentCredit.

The fixture now uses the established two-pass construction: project the stable
Core Market address, derive the lifecycle RentCredit, recompile the final graph
with that RentCredit as Core's beneficiary, and assert that the Market address
did not change. No program code or ELF changed.

## Accepted Position and retirement walk

`a_fractional_market_retires_end_to_end_from_begin_through_finish` passed:

- admit created the zero root reserve Position and its admission;
- the host changed the installed Open Core state to Terminal;
- `Begin` created the cursor, `RetireCoordinate` closed the Position,
  admission and real Token-2022 mint, and `Finish` closed the cursor;
- every intermediate and whole-walk lamport conservation assertion passed,
  including settlement of the stranger's 4,242-lamport cursor donation.

Measured retirement CU was 36,111 for Begin, 132,033 for the coordinate, and
27,290 for Finish.

## Exact refusals and rollback

`a_strangers_lamport_cannot_block_admission_and_underfunding_still_refuses`
passed. Over-declared funding refused at exact `0x5145`
(`ProtocolPositionSbfErrorV2::Position`) after 112,070 Claims CU and left the
Position unallocated. Honest admission then consumed 153,817 Claims CU. A close
with a substituted persisted balance baseline refused at exact `0x5148`
(`ProtocolPositionSbfErrorV2::Admission`) after 50,688 Claims CU. The honest
close consumed 72,215 Claims CU, closed both accounts, and credited their full
live balances, including the stranger's lamport, to RentCredit.

`an_incomplete_walk_cannot_be_finished_and_the_cursor_survives_the_attempt`
passed. Begin consumed 36,111 Claims CU. Finish before coordinate zero refused
at exact `0x5008` (`ClaimsSbfError::Representation`) after 23,906 Claims CU,
and the complete cursor `Account` remained byte-for-byte and lamport-for-lamport
equal to its pre-transaction value.

## Logs

The authoritative logs are:

```text
/tank/dregg-build/dclutch-claims-position-fd7e5fec6-v2-20260909/logs/accepted-retirement.log
/tank/dregg-build/dclutch-claims-position-fd7e5fec6-v2-20260909/logs/accepted-retirement-fixed.log
/tank/dregg-build/dclutch-claims-position-fd7e5fec6-v2-20260909/logs/underfund-refusal-rollback.log
/tank/dregg-build/dclutch-claims-position-fd7e5fec6-v2-20260909/logs/incomplete-walk-refusal-rollback.log
```

The first is the deliberate retained red observation. The latter three each
report one passed, zero failed and six filtered out. All four report zero
`Unit run-u*.scope was already loaded` scheduler-no-op markers.
