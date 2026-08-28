# Validation backlog

This file names expensive or cross-component validation that must run at a
convergence checkpoint. Focused tests still run with each semantic change. A
checked build, private-validator fixture, or devnet transaction from an earlier
source commit never substitutes for the exact convergence commit.

## Preserved failing baseline

- Source `c8b8e41c1ced8aa6c8d4934b43838694b68b469a`, checked gate SHA-256
  `8e1b058dc3c9dae26ab107600dc2566c9c64d656c255d009a554430d5d9f7d49`,
  failed its one-seed local-only probe at the honest DCLTPCB2 transaction.
  Trading returned `0x4003` after the non-terminal and reordered-ledger hostile
  probes passed. The exact summary is
  `/tank/dregg-build/dclutch-private-participant-probe-c8b8e41c-a/SUMMARY.json`
  on hbox. Keep this as the regression baseline; do not rerun or relabel it.

## Next private-validator convergence gate

Run one fresh, source-pinned checkpoint after Direct, sponsored submission,
provider genesis, payout, retirement, and aggregate lifecycle receipt surfaces
are frozen in one commit.

The checkpoint must include:

1. A fresh all-13 checked release build on hbox under `swarm-build`, with a
   separate frame-diagnostic build for every shipped link. Record the gate
   digest, exact source commit, all ELF digests, zero build diagnostics, deepest
   frame per link, and immutable Cargo-lock closure.
2. One fresh owned-loopback run through Founding and collateralized participant
   admission before Direct starts. Require the explicit partition of
   1,000,000,000 founding atoms plus 100,000,000 participant atoms, removed mint
   authority, the `direct-buyer` source account owned by `participant`, and both
   participant transactions finalized.
3. Direct setup and execution through the accepted caller: InitializeReplay,
   lookup-table creation/extensions/freeze/activation, capability seal, and Hot.
   Preserve every mutating signature, finalized slot, fee, compute measurement,
   and exact poststate owner delta.
4. Real local Pyth prerequisites, Resolution, payout, and retirement. Pyth
   Receiver and Router must come from explicit Loader-v3 Program/ProgramData
   genesis accounts with deployment slot zero and authority tag `None`.
   `solana-test-validator --upgradeable-program ... none` is forbidden because
   Solana 4.0.2 encodes it as `Some(system_program)`.
5. One finalized capture containing all nine Programs and nine ProgramData
   accounts at one finalized slot. Re-decode every Loader link, deployment slot,
   full ProgramData digest, ELF-tail digest, executable/owner state, and upgrade
   authority from captured bytes.
6. One canonical aggregate receipt and a separate exact chaos session. Activity
   stages are `founding, participant, alt, seal, direct, resolution, payout,
   retirement`; chaos stages are `founding, participant, alt, seal, hot,
   resolution, payout, retire`. Do not merge or rename the two vocabularies.
7. Crash/restart checks at every Submitted boundary. A restart may poll the one
   exact expected signature; it must never sign or send a second packet.

## Provider genesis and closure

- Generate Receiver and Router Program plus ProgramData JSON accounts into the
  authenticated local genesis account directory. Require Loader-v3 Program
  links, slot-zero ProgramData, null authority tag, rent-exempt lamports, fixed
  program IDs, exact fixture ELF digests, and pairwise-distinct coordinates.
- Add hostiles for tag-one default/system authority, wrong ProgramData link,
  wrong slot, wrong ELF tail, wrong full ProgramData bytes, extra Loader account,
  and substituted provider closure receipt.
- Produce `dclutch-owned-loopback-pyth-provider-closure-v1` at a finalized
  observation and bind it by path and SHA-256 from the aggregate receipt. The
  four-field Pyth update facts remain the separate owner of PostUpdate inputs.

## Terminal simulator surface

- Keep `local-private-validator-wallet-terminal-payout-v1` callable and require
  its finalized evidence signature, slot, compute units, return data, and exact
  payout poststate.
- Add one stable callable retirement completion receipt. It must bind all
  terminal mutation journals, signatures, fees, compute units, finalized slots,
  and before/after balances. A scenario-only projection or ephemeral stdout is
  not completion evidence.
- Hand the exact payout and retirement argv, schemas, signature pointers, and
  finalized-delta fields to the activity reconciler and simulator.

## Broad release measurements

- After the one-seed lifecycle is fully green, run exactly 20 named seeds from
  the same checked source and fresh local ledgers.
- Report pass count and exact arithmetic mean for every named compute metric as
  numerator, denominator, floor, and remainder. Never publish one draw as an
  M-61 result.
- Run the eight-stage lifecycle chaos matrix against the same accepted command
  and schemas. Preserve all case receipts and terminal session SHA-256 values.
- Run the independent owned-loopback activity reconciler over the finalized
  capture, manifest, semantic-owner journals, provider closure, and aggregate
  receipt. Record the aggregate receipt SHA-256 and dossier SHA-256 in the final
  summary.

All entries above are private-validator or build evidence. They are not devnet
execution evidence and must not be described as a public deployment result.
