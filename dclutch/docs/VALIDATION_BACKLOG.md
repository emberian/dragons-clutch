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
   frame per link, and immutable Cargo-lock closure. Reconfirm the compiled
   DCLTGMF2 59-key and DCLTPCB2 62-key censuses, including the admitted
   64-key/refused 65-key boundary.
2. One fresh owned-loopback run through Founding and collateralized participant
   admission before Direct starts. Require the explicit partition of
   1,000,000,000 founding atoms plus 100,000,000 participant atoms, removed mint
   authority, the `direct-buyer` source account owned by `participant`, and both
   participant transactions finalized. Require admission to join the exact
   `founding_lifecycle_rent_credit` owned by the DCLTGMF2/Open-market generation;
   the earlier Found37 `lifecycle_rent_credit`, an aliased coordinate, or a
   missing founding label must refuse before any signer access. For the
   owned-loopback SourceAbort lane, record the actual slot deltas across its
   sixteen finalized pre-expiry barriers and both dispatch guards; require the
   576-slot fixture policy to retain its 160-slot staging and 64-slot rollback
   margins, preserve the pre-expiry fee-only whole-transaction rollback, and
   complete DCLTPCA1 only after the real expiry slot. Public/devnet must still
   select the prior 900-slot expiry and 64-slot staging margin.
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

## Sponsored Pyth push convergence

- Exercise Capture, Settle, CommitFailure, CloseCandidate, and CloseHead through
  the final compiled Resolution artifact. The Market fixture must come through
  the V6 controller-funding split: Prepared checkpoint, CustodyStaged, Open
  consumption and checkpoint closure, then the exact active Resolution
  `FundingLedgerV2`. A caller-supplied checkpoint, a still-Pending ledger, or a
  V5 controller binding must refuse.
- Preserve the accepted hostile corpus and prove whole-transaction rollback.
  Run the real-SBF compute campaign under M-61 and report pass count plus the
  20-seed mean for every action. Re-run activation/headroom measurements for
  the final Resolution ELF; an earlier sponsored artifact is development
  evidence only.
- Exercise two independent sponsors, monotone best-valid-submitted head
  advancement, strict post-deadline settlement, both sponsor-beneficiary
  closes, and the vacant-head funded failure path on owned loopback.
- Freeze the final Resolution semantic release, account frames, caller input
  schema, GENREF/SBOM/lock rows, and checked artifact together. Re-run the old
  pull-profile rejection against the sponsored release and account body.
- Before devnet execution, provision and freeze the exact routing table and
  exercise the report's durable
  `planned -> prepared -> submitted -> finalized` restart states. Recheck the
  Receiver and push-oracle ProgramData slots, authorities, Config digest, and
  price account. Preserve the exact v0 packet/signature, table body, rent, and
  fee arithmetic separately from protocol principal.

## Resolution V7 split lifecycle convergence

- Exercise the atomic-founding recovery order from one finalized successor
  campaign: `DCLTCFQ1`, `DCLTPCB2`, `DCLTGMF2`, `core-funding-create-v1`,
  `resolution-funding-activate-v1`, `core-funding-accept-v1`, then participant
  admission and Direct. Create, activation, and Accept are three distinct
  payer-only transactions. A projected Found37 Market is not a live Core
  account, and DCLTGMF2 does not create `SourceResolutionStateV2`; neither may
  be used to skip CreateFund. Crash after Create must resume activation from
  the live Primary Source and Pending ledger. Crash after activation must
  resume the Accept suffix from the immutable `DCLRFAR1` receipt and the live
  Active ledger; exact activation and Ready/Consumed Accept replays are no-ops.
- Preserve CreateFund's exact 16-account frame, or 18 accounts with recovery
  policy, the direct activation's exact 18/20-account frame, and the no-CPI
  Core Accept's exact 18/20-account frame. Record separate signatures,
  finalized slots, fees, compute units, pre/post account digests, and durable
  journal phases for `core-funding-create-v1`,
  `resolution-funding-activate-v1`, and `core-funding-accept-v1`. Refuse a V6
  release ID, writable or substituted beneficiary, mismatched receipt PDA or
  request digest, Pending ledger at Accept, and any receipt/live-ledger digest
  disagreement.
- Exercise provider terminalization as two additional durable mutations:
  `resolution-provider-execute-v1` owns Source, provider-lifecycle, and
  `ResolutionCertificateV2`; `core-terminal-accept-v1` independently
  authenticates that certificate and commits Core Terminal last without CPI.
  Persist both transaction rows, including independent fees and compute units.
  Crash after execute must resume Accept-only, must not authorize payout, and
  exact Terminal Accept replay must be a no-op. The superseded Resolution child
  route and Core `ExecuteProvider` wrapper must refuse.
- Exercise permissionless `DCLRFCQ1` close directly against the 19-account
  Resolution frame, or 21 accounts with recovery policy. Bind the immutable
  beneficiary and preserve `SourceClosureReceiptV3` as the sole terminal fact.
  Cover late Core-release substitution with whole-transaction rollback, exact
  replay refusal after physical close, and restart from every submitted
  boundary without a second signature or send.
- At the final feature freeze, rebuild formatted Core and Resolution SBF, run
  the complete all-13 frame diagnostic, then run the activation, funding
  Accept, provider execute, terminal Accept, and direct-close controls under
  M-61. Report pass count and the exact 20-seed arithmetic mean for every named
  mutation; no single focused draw is an M-61 result.

## Activity V3 devnet convergence

- With one accepted V3 manifest, checked release, Market, harness digest, CLI
  digest, and at-most-six-hour `maxCycles=1` authorization, externally fund
  only the disposable campaign payer. Authenticate the payer-only initial
  funding closure, then run the exact founding -> post-init funding ->
  participant -> Direct -> resolution -> payout -> retirement lifecycle.
- Retain finalized post-init journals, the post-init closure, the combined
  funding-lifecycle fact, reconciliation output, and supervisor status.
  Exercise one interrupted `Dispatching`/`Submitted` restart in poll-only
  recovery and prove `Prepared` never dispatches without the signer-owning
  command.
- Require every `getFeeForMessage` quote to equal the finalized fee. Enforce
  separate post-init transfer/fee caps and campaign gross-debit/activity-fee
  caps, one global signature set, exact per-wallet finalized histories, and
  exact final balances without counting internal wallet redistribution as a
  second external bankroll debit.
- Public evidence may contain addresses and path digests, but never private
  paths, packet bytes, or permanent deployer/funder material. No funding row
  may be projected as Direct economics.

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
   workspaces and 60 tracked lockfiles. The 25 stale graphs were repaired
   through `a0e4f695`; the final source-bound inventory must still prove the
   whole set immutable.
2. Populate the hbox Cargo cache with those exact locked graphs, then run
   `tools/release/check-all-workspaces.py` from fresh archived source under
   `swarm-build`. Require workspace pass count = workspace count and complete
   lock immutability. The last full exact-source census was 41/45 at
   `400fcfca`: the two offline-cache misses were populated from the local
   pinned Cargo cache without a network fetch, and their focused checks passed;
   Journey and Relayed-vertical then passed focused locked/offline checks at
   `d2724c2f` and `b18ac2da`. Those focused results are not a 45/45
   source-bound census. The exact archived `2b0e6c29` checkpoint later passed
   45/45 with all 60 locks immutable; its summary is
   `/tank/dregg-build/dclutch-all-workspaces-2b0e6c29-run1/SUMMARY.txt`
   (SHA-256
   `5f790218ff76ae514dab2efe62c072697c4a9e5cb19adb59bb29e93e25f60b2f`).
   That source failed the real founding runtime below, so rerun the enumerator
   if the next accepted CU repair changes any workspace or lock graph.
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
   route authority had 107 routes and 209 refusal codes.
8. Produce the exact seven-row deployment disposition: Core, Claims, Trading,
   Resolution, and Custody are `Upgrade`; Registry and Rent are `CarryForward`.
   Bind each row to the checked gate and current finalized Loader observation.
9. Only after every row above and the private checkpoint pass, hand the exact
   candidate, host tool, generated authorities, M-61 evidence, and
   five-Upgrade/two-CarryForward set to DEVNET-EXEC.

The final M-61 sweep must use the Trading ELF after Direct `InitializeReplay`
and its caller freeze. Report pass count and the 20-seed mean beside the exact
ELF SHA-256; do not report one draw or an observed minimum as a margin.

## Current integrated build evidence

- Source `2b0e6c29b9adea55b979585e20cfc024ea07816c` includes the Resolution
  duplicate-work repair and Trading's authenticated duplicate-preplan removal.
  Its exact archived-source gate is
  `/tank/dregg-build/dclutch-checked-2b0e6c29-run1/work/CHECKED_UPGRADE_GATE.json`
  (SHA-256
  `ed013070f4f0194d84ec49a954cc64cc0184d5b0974aea4f65a93456145c2997`).
  All 13 links were fresh, diagnostics were zero, and all 60 Cargo locks were
  unchanged with before/after set SHA-256
  `6e22305800897e66bb05e20d1c9e69b0690a7ac8aa4c32d0172cbf97576dc4b2`.
  Trading measured 732 frames below 4,096 bytes, deepest 4,032 bytes. The exact
  Trading ELF is 1,690,680 bytes with SHA-256
  `675c9c45bde6089ef4b57daf770ece7d2bd33870a0043e42e5d0e2119c229d2a`.
- The fresh immutable-substrate M-61 sweep for those exact Trading bytes passed
  20/20 seeds with arithmetic mean 1,359,277 CU. Its summary is
  `/tank/dregg-build/dclutch-m61-2b0e6c29-run1/summary-immutable.json`
  (SHA-256
  `af21319eb4a06af7371ae9b9b0eccc0ac013979da436145407c234ab1746dc70`).
  This is compute evidence, not a private-validator lifecycle acceptance.
- The exact one-seed participant-through runtime rejected this candidate during
  honest founding: three Custody calls consumed 355,209, 98,172, and 113,313
  CU, Core consumed 258,805 CU, Resolution exhausted at 623,608 of 623,664 CU,
  and outer Trading exhausted at 1,399,494 of 1,399,550 CU. The transaction
  consumed the 1,400,000-CU maximum and admitted no participant. Its summary is
  `/tank/dregg-build/dclutch-private-participant-2b0e-run1/SUMMARY.json`
  (SHA-256
  `229cc903f1e99f83b0c2e272c72b3755cdc707540e9d89e631c682965c3e14ab`).
  The Trading preplan removal still increased the allowance before the
  Resolution child by an observed 58,709 CU, but post-child Trading work was
  never reached. Preserve the gate, workspace census, and M-61 result as build
  evidence only; a larger onchain CU repair and a fresh runtime candidate are
  required.
- Resolution descendant `68f6f7d6` collapsed the remaining duplicate
  pre-Market authentication. In its fixed-payer focused harness, whole-caller
  work fell from 861,269 to 813,778 CU (47,491 CU observed reduction) and
  Resolution-exclusive work fell from 512,659 to 474,174 CU (38,485 CU
  observed reduction). The evidence root is
  `/tank/dregg-build/dclutch-resolution-cu2.WDJYpG`; host tests passed 4/4,
  the compiled-SBF positive and Found37-alias rollback cases passed, and all
  133 measured frames were below 4,096 bytes with a 1,920-byte pre-Market
  maximum. This focused harness does not contain Trading's post-child work.
  Direct and PRIVATE therefore selected a durable Prepared funding checkpoint
  as required architecture; do not emit another gate for the old one-transaction
  DCLTPCB2 shape. Run the next all-13/frame/M-61 gate only after that vertical
  slice freezes.

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
- `cb4e93b5` removed duplicated pre-Market Resolution work after the
  `400fcfca` private run exhausted its 561,958-CU nested budget at 561,902 CU.
  Its focused hbox overlay used the `400fcfca` source plus the exact accepted
  `pre_market_funding_v1.rs`; that overlay is not itself a checked candidate.
  A separate exact-source all-13 build subsequently emitted the strict forensic
  gate
  `/tank/dregg-build/dclutch-checked-cb4e93b5-run1/work/CHECKED_UPGRADE_GATE.json`
  (SHA-256
  `013cf739b08c51f62ac98b9ed7a2555abb3dc55d555b6448dad7f056858b1b80`),
  with zero diagnostics and immutable 60-lock closure. It is not the final
  runtime candidate: full Hot remained under live test and the paired Trading
  CU repair was still open. The optimized SBF build log is
  `/tank/dregg-build/dclutch-resolution-cu.LKt6s0/logs/resolution-build.log`
  (SHA-256 `75329d4c0e88de0aa047a9e1712c6969b955b5b1dea2229a91eb4805d59952fe`),
  and the real-SVM success log is
  `/tank/dregg-build/dclutch-resolution-cu.LKt6s0/logs/pre-market-svm.log`
  (SHA-256 `165d126f5f19bbfbd7d52c54131615abd92fde00066344c30bff3b804bf2f23a`).
  One paired randomized-caller draw reduced Resolution-exclusive work from
  516,146 to 511,159 CU after subtracting nested Core, an observed 4,987-CU
  reduction, not an M-61 margin. The emitted-stack object is
  `/tank/dregg-build/dclutch-resolution-cu.LKt6s0/frame-target/sbpf-solana-solana/release/deps/dclutch_resolution_proof_sbf.o`
  (SHA-256 `b7c44d70507446bd9079c29255afe346fb6455d3eef2eefd204b85069edd65dc`):
  all 133 frames were below 4,096 bytes and the deepest pre-Market frame was
  1,920 bytes. The exact-source one-seed private run then failed the honest
  founding transaction before participant admission with the same 56-CU tail:
  Resolution consumed 564,899 of 564,955 CU, outer Trading consumed 1,399,494
  of 1,399,550 CU, and simulation reported 1,400,000 units consumed. Its
  summary is
  `/tank/dregg-build/dclutch-private-participant-cb4e-run1/SUMMARY.json`
  (SHA-256
  `cfdd565d27d4e3909ddba0d2a6a15bc8511afc3149b95ef32faf0ca19144c6d9`).
  Preserve the strict gate and focused 4,987-CU draw as forensic evidence only;
  the combined Trading duplicate-preplan repair, fresh gate, and production
  runtime comparison remain required.

## Public delivery checkpoint

- After the final deployment facts are published, run the manual GitHub Pages
  workflow from the exact wrapper commit and record its run ID, head SHA, and
  successful deployment job. Recheck the public hostname independently; an
  HTTP 200 alone is availability evidence, not an interactive browser result.
- In a cold browser, open `/operate`, choose the checked live-devnet preset, and
  prove it fills only the endpoint and six published roles. Market and Realm
  must remain empty until chain discovery supplies real addresses. Require the
  fresh finalized Loader/header and activation-cache checks to pass while the
  page still states that route-specific release admission is unproven.
- Exercise the public Market, trade, activity, portfolio, and redemption pages
  against the real devnet addresses and finalized activity dossier. Verify
  wallet connect, explicit signing prompts, Submitted-before-send recovery,
  transaction links, refresh/reload recovery, and plain second-person refusal
  copy. No static projection may be presented as onchain completion.
- Redeploy the Sites mirror from the same accepted web source and repeat the
  public navigation/smoke checks there. Record the project/deployment identity
  separately from GitHub Pages; one host passing does not prove the other.
