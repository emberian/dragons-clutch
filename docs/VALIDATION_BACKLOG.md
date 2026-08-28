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
8. Gate-authenticated five-role activation on Agave 4.0.2 with all seven role
   ELF paths selected by one exact checked gate. Stop the validator after the
   exact Core+Claims prefix, restart the same ledger without genesis inputs,
   require chain-derived pending order Trading, Resolution, Custody, then
   complete without replaying either admitted role. Record per-role CU and the
   size-only headroom report; the latter is not a measured-CU substitute.
9. Disposable-Loader Upgrade recovery at every durable boundary: interrupted
   Buffer writer, lost send response after landing, `SignedNotSubmitted`,
   pending `Submitted`, exact expiry/reprepare, and completed idempotent
   observation. Require one writer, no second signature/send, exact payload and
   slot advance, unchanged parked rent, and complete payer/fee arithmetic.

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

## Final devnet rehearsal

- At DEVNET-EXEC's final key-free freeze, run
  `devnet-permanent-substrate-capture-v1` into a new no-clobber path at an
  immediate finalized floor. Bind all seven fixed Program/ProgramData pairs
  plus the explicit payer in its one context; compare authority, slots, payload
  digests, Program residue, ProgramData rent, and payer balance to the execution
  envelope before any signer boundary opens.
- Rehearse the exact five-role mutation-permit order from the frozen checked
  gate, then let DEVNET-EXEC alone perform the authorized Upgrades. Preserve
  every baseline, receipt phase transition, finalized transaction, poststate,
  dump, and wallet arithmetic. Registry and Rent remain CarryForward; no
  Program is closed, recycled, or redeployed.
- After the last Upgrade, recapture the seven+payer substrate in one finalized
  context, authenticate the mixed release plan, and run campaign preflight
  through activation before opening market keys. Only then run the first public
  market/activity suite and verify the public site projects those exact live
  addresses and finalized states.

## Repository and release-authority convergence

Run these rows at the same combined freeze as the private-validator checkpoint:

1. Repair every stale Cargo graph from a detached copy, validate each result
   with `cargo metadata --locked --offline`, then atomically install only the
   exact generated `Cargo.lock` files. The last complete inventory found 45
   workspaces and 60 tracked lockfiles, with 25 stale graphs.
2. Populate the hbox Cargo cache with those exact locked graphs, then run
   `tools/release/check-all-workspaces.py` from fresh archived source under
   `swarm-build`. Require workspace pass count = workspace count and complete
   lock immutability. The last baseline was 17/45; two additional reds were
   hbox offline-cache misses (`spl-token-2022`, `lru 0.18.3`).
3. Run `tools/release/checked-release-candidate.sh` from that same commit in a
   new work root. Require all 13 shipped links freshly compiled, zero SBF frame
   diagnostics, every measured frame below 4,096 bytes, and an emitted checked
   Upgrade gate.
4. Authenticate the exact release role map. Resolution is label `resolution`,
   package `dclutch-resolution-proof-sbf`, canonical gate file
   `elf/resolution.so`. Refuse the orphaned 9,034,536-byte `dclutch_sbf.so`
   substitution. Preserve d66784f1's exact guard or its accepted successor.
5. Build `dclutch-local-successor-bootstrap` separately from the same archived
   source with `--release --locked --offline` in a fresh target. Record its
   canonical nonsymlink path, SHA-256, toolchains, command, source commit/tree,
   help output, and focused operator tests. This is operational evidence, not
   checked-gate authority.
6. Run the gate-authenticated mixed-real-ELF activation test, including ledger
   process interruption and chain-derived resume. Record observed CU per role
   without calling one observation a bound. The last canonical control observed
   Core 565,457; Claims 535,732; Trading 828,069; Resolution 351,936; Custody
   259,058 CU.
7. Regenerate every affected browser ABI owner, then the route census, GENREF,
   SBOM, and notices through their temp-file/atomic-replace generators. Require
   the second generation/check pass to be byte-identical. The last known stale
   route authority had 105 routes and 209 refusal codes.
8. Produce the exact seven-row deployment disposition: Core, Claims, Trading,
   Resolution, and Custody are `Upgrade`; Registry and Rent are `CarryForward`.
   Bind each row to the checked gate and current finalized Loader observation.
9. Only after every row above and the private checkpoint pass, hand the exact
   candidate, host tool, generated authorities, M-61 evidence, and
   five-Upgrade/two-CarryForward set to DEVNET-EXEC.

The final M-61 sweep must use the Trading ELF after Direct `InitializeReplay`
and its caller freeze. Report pass count and the 20-seed mean beside the exact
ELF SHA-256; do not report one draw or an observed minimum as a margin.

## Other preserved intermediate evidence

- `5cad4bb6` had a green checked gate and byte-identical onchain artifacts to
  c8. It is obsolete for the same lifecycle generation.
- `b589f16b` had a green checked build/frame gate but is runtime-obsolete: an
  intentional participant/Direct-buyer role overlap attempted duplicate local
  key creation.
- `00af1247` stopped during the first checked link and emitted no gate after a
  cross-generation participant rent-credit label bug was found.
- The 1,399,700-CU “Resolution” activation was an invalid manual control using
  the orphaned 9 MB `dclutch_sbf.so` Source artifact. It is hostile oversized
  substitution evidence only, not Resolution evidence.
