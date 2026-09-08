# Claims rational lifecycle SBF component mix — 2026-09-07

Evidence level: real SBF `ProgramTest` component mix on hbox. This record does
not establish an all-eight checked release, a local-validator founding, a
devnet deployment, or a live cohort.

## Scope

The regression distinguishes a Product's actual coordinate count `N` from a
receipt representation descriptor width `K`. The test begins with a Product of
258 coordinates and a receipt descriptor of width 2. It first submits a
well-formed hostile descriptor of width 3, then submits the canonical width-2
request. It uses real Claims and Token-2022 ELFs plus seeded Core, Claims, and
Rent components.

The hostile transaction must refuse as
`RationalLifecycleSbfErrorV2::Instruction` (`0x5210`) and roll back every
named protocol account. The accepted transaction must create a real
Token-2022 receipt mint whose terminally burnable profile has zero supply. The
test also compares the Core Market, Claims aggregate, and Rent credit against
their pre-activation values, so zero-supply receipt creation cannot silently
move those facts.

## Exact source and red control

The isolated old source checkout was detached and clean at
`7dd2b49b38e0de2191c1546dcc75c27cb4685426`. Its complete-source bundle had
SHA-256 `1a04f41b1645013658142bbe95588ba48e33a10eee90bc8e83134b1c54487df2`.
The filtered serial command was:

```text
SBF_OUT_DIR=<red-sbf-out> CARGO_TARGET_DIR=/tank/dregg-build/dclutch-codex-20260907/target \
CARGO_BUILD_JOBS=6 SWARM_MEM_MAX=32G swarm-build cargo test --locked \
  --manifest-path programs/dclutch-claims-sbf/program-test/rational-lifecycle/Cargo.toml \
  --test lifecycle sparse_receipt_lifecycle_keeps_product_n_separate_from_representation_k \
  -- --exact --nocapture --test-threads=1
```

It failed exactly at `RationalLifecycleSbfErrorV2::Market` (`0x5214`), before
the accepted width-2 activation. The preserved log is
`/tank/dregg-build/dclutch-claims-red.0dQYPZ/red.log`; it records one failed
and five filtered tests.

## Fixed Claims ELF and green runs

Claims was rebuilt from a separate clean detached full source checkout at
`872522e07b12d78406731285c264ba9cf5c48005`. Its source bundle had SHA-256
`da1617d42754b94236f02da59723535741a00ccb4eff75f716cd86299fe5aa2e`.
The fresh Claims-only SBF build was run through `swarm-build` with
`SWARM_MEM_MAX=32G` and `CARGO_BUILD_JOBS=6`; its strict scan found neither
SBF frame diagnostic form nor an `already loaded` scheduler no-op marker.

The fixed Claims artifact SHA-256 was:

```text
d4b15932f4eac16e88c82d24996e37db4ea3602faebcae39d15419ded2e3a059
```

It was mixed only with these unchanged controls and caller:

| Input | SHA-256 |
| --- | --- |
| Core SBF | `a08769a886cf03a061376dc5741c4141c545554830a3c9b79423477b94115c97` |
| Registry SBF | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| Rent SBF | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| canonical Token-2022 SBF | `e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697` |
| rational-lifecycle test caller SBF | `f83650a15cc9ab40f68a4e0538f7c811ff58921bd0d968f147fd684898b15391` |
| checked-gate input | `d88a1b858fafd3fff79cec2f3f70e39610c1bf3c4033afe3539761a3506172d8` |

The sparse filtered run passed one test with five filtered. Its log,
`/tank/dregg-build/dclutch-claims-red.0dQYPZ/green-sparse.log`, records the
hostile `0x5210` refusal, then real Token-2022
`InitializeMintCloseAuthority`, permissioned-burn extension initialization,
and `InitializeMint2` for the accepted zero-supply mint.

The existing equal-width control was also run serially against this same
component set:

```text
a_strangers_lamport_cannot_block_a_prepaid_receipt_activation
```

It passed one test with five filtered. Its log,
`/tank/dregg-build/dclutch-claims-red.0dQYPZ/green-equal-width-control.log`,
keeps the expected underfunding refusal
`RationalLifecycleSbfErrorV2::Rent` (`0x5215`) before the accepted
Token-2022 mint initialization.

## Limits

All run material remains under
`/tank/dregg-build/dclutch-claims-red.0dQYPZ`, including source commit files,
artifact hash manifests, build logs, red output, and separate green output.
This was deliberately a Claims component mix: it does not substitute for an
all-eight fresh checked gate or a final frame capture pair.
