# Cohort-16.1 on devnet: a release generation superseded in place, and the first activation that carries dependency edges

Status: **devnet execution evidence.** Owner: lane COHORT-16C. Written
2026-09-05 at `/Users/ember/dev/dclutch` (the live tree). The job directory is
`~/jobs/dclutch-cohort161-20260905/` and it is self-contained: the driver
binary, its digest, the reproduced candidate with its whole evidence chain, and
every stage log and report live there. Every number below was read back off
devnet at finalized commitment or off an artifact in that directory.

Candidate commit `87eec1c3a6bf954a4350931af62ce8d4fcc48da2` (PROGRAMS-17A's).
Driver commit `05520cf7e`. Prior cohort 16, whose programs this cohort UPGRADES
IN PLACE rather than replaces.

## 0. What is new about this cohort

**It is the first cohort that is not a deployment.** Cohorts 14, 15 and 16 each
deployed fresh program identities and abandoned their predecessor in place.
Cohort-16.1 keeps every program id, upgrades two of the eight links through the
checked-upgrade seam decision 0012 built, and re-releases the generation. Six of
the eight ELFs never move.

**It is the first activation on any chain whose funding frame carries a
dependency ledger.** 36 accounts instead of 35, the selected Trading ledger and
the preserved Resolution ledger, **521,895 CU**. PROGRAMS-17A's offline
real-ELF harness predicted **521,780** for the same market shape. The gap is
**115 CU**, 0.02%.

**And it is the cohort that proved an open market cannot cross a generation.**
§1 is that finding, and it reverses a sentence in the cohort-16 evidence
document and in the runbook.

## 1. The finding: a superseded market cannot be re-admitted, ever

The ruling this lane was given was: upgrade Trading in place, re-release the
generation, and KEEP the open market
`GyD95eyERwRfwj8fSFNhWjKF2eaDg5XcREidPKex65zY`. **Keeping it is not possible on
any path**, and the whole of the argument is in accounts and layout constants
rather than in a judgement:

- `Market.release_set_id` (offset 208; read off chain as
  `85defd75b236b191de00b48e673cdc4a4bcc2408b2248c4504895815b04cc69f` at
  finalized slot 493,735,728) is written by exactly one function,
  `initialize_market` (`crates/dclutch-product/src/economic_slice/mod.rs:271`),
  which refuses any output that is not all zeros. It is write-once at founding
  and no route anywhere writes it again.
- It selects the activation cache at
  `[ACTIVATION_PDA_DOMAIN_V1, release_set_id]` under the Registry, and the
  cache pins each role's `ArtifactReleaseV1`.
- An artifact release id is `sha256` of a 216-byte record whose fields include
  the ELF digest at offset 144 **and the deployment slot at offset 176**
  (`crates/dclutch-registry/src/artifact.rs`). The execution release set id is
  `sha256` over the five `(program_id, artifact_release_id)` bindings
  (`crates/dclutch-release-tool/src/multiprogram.rs:149`).
- Every route re-reads live ProgramData and requires
  `observed_deployment_slot == release.deployment_slot()`
  (`slot_pinned_release_elf_digest_v1`), and Core's Direct activation route is
  one of them: it requests `Role::Trading`'s Loader pair
  (`programs/dclutch-core-sbf/src/capability.rs:132-152`) through
  `authenticate_roles`.
- Loader V3 writes the current slot on every `Upgrade` and refuses an `Upgrade`
  in the deployment's own slot. **The old slot is therefore unrecoverable.**
  Measured rather than asserted: recomputing the release set with the *identical
  old ELF* at any later slot gives `56eb189721260a11…`, not `85defd75…`.
- The only forward mechanism, `ReleaseLineageV1` and `lineage_walk`, **has no
  consumer.** `release_lineage_address_and_bump_v1` is called by the Registry
  program (which creates the record), by the operator's declare-successor
  builder, and by tests — and by no Core, Trading, Claims, Resolution or Custody
  route. `crates/dclutch-registry/src/lineage_walk.rs` is referenced only by its
  own tests, and its head says *"Core derives it to find the successor its
  market may hop to"* about a route that does not exist. That is the
  producer-missing pattern with the halves swapped: the reader, the refusals and
  the hop bound are all written, and nothing calls them.

**The arithmetic was checked against the chain before anything was spent.** A
216-byte record composed from the layout constants reproduces all five of
cohort-16's published artifact release ids and its `release_set_id`
`85defd75…` exactly; the same composition over the post-upgrade observations
reproduces cohort-16.1's `f533be49…` exactly, including the fact that the
release binds the **live padded** ELF digest and not the raw link.

**Nothing was lost by proceeding.** `GyD95eyE…` could not be activated at the
cohort-16 release either — measured 2026-09-05, `TradingSbfError::Content`
`0x4003` after 108,180 CU — and a market that cannot activate cannot be
retired. It was already inert. The upgrade changed which refusal it gets, from
`Content` to `ReleaseSuperseded`, and took nothing that had a future.

**What is corrected.** `tools/cohort/steps.tsv`'s `manifest-edges` verifier said
*"an already-founded edged market activates at the new Trading link without
re-founding"*, and the addendum of 2026-09-05 to
`COHORT16_DEPLOYED_SEALED_2026_09_05.md` said the release id and Market address
do not move. Both halves of that are true of the **Direct capability release**
and false of the **execution release set**, which is the thing a market pins.
A Trading link that differs by one byte moves the generation.

## 2. The candidate, reproduced

PROGRAMS-17A's candidate was one host and one run. This lane rebuilt it on hbox
under `swarm-build` from a **second repository root** (`/tank/dclutch-c16c-repo`,
an independent clone) into a **second `--work` root**
(`/tank/dclutch-c16c/candidate-a`), 445s, exit 0.

| line | value |
| --- | --- |
| `source_revision` | `87eec1c3a6bf954a4350931af62ce8d4fcc48da2` |
| `source_digest` | `408d3f5a94e6d3c1d23cb429b8ba16f42cacce595ece5fbb394e8d6d4cf0a42e` |
| `sbf_build_diagnostics_total` | **0** |
| `cargo_lock_immutability` | `passed` |
| `spline_product_handoff` | `passed` |
| `reproducible_release_gate_sha256` | `ba0205ccd3537c1f9c67b0ffcb431defe96f70a6f209fa2738157e96abe5ce40` — **equal to 17A's** |
| `checked_upgrade_gate_sha256` | `27f2845b0081bb16…` — differs, as its schema's own doc predicts |

**All eight ELFs are byte-identical across the pair**, Trading at
`e7f8e476006ce1248994ae065bffd7ea0039c8681f85fed141368790e021931b`.

The per-run envelope difference has a measured cause worth keeping: the
`SUCCESSOR_CAMPAIGN_PACK.json` digests differ, and every differing leaf is a
build log or a provenance blob whose *byte count* differs by exactly two
occurrences of the seven-character difference between the two absolute `--work`
paths. That is the same thing `dclutch-reproducible-release-gate-v1` exists to
exclude, demonstrated rather than argued.

### Against the deployed set: six, not seven

| link | cohort-16 deployed | candidate `87eec1c3a` | moves |
| --- | --- | --- | --- |
| accelerator | `587181d9536d…` | same | no |
| claims | `33e453e62186…` | same | no |
| custody | `2600db72b383…` | same | no |
| registry | `8eb3ccc0e9d0…` | same | no |
| rent | `100f211918ac…` | same | no |
| resolution | `7be8a398be52…` | same | no |
| **trading** | `69292c339192…` | `e7f8e476006c…` | **yes** — 17A's unified activation frame |
| **core** | `f637e5df9ef9…` | `29200c855bfe…` | **yes** — `17f1b6dec`, the Series rent floor |

Core is the eighth mover and it is not 17A's: it landed between the cohort-16
deploy commit and `57e4b9b27`. A checked deployment set admits no mixed
generation — `devnet-deployment-set-already-current-v1` refuses a role whose
live bytes differ — so upgrading Trading alone was never available.

## 3. The upgrade, to the lamport

| act | signature | SOL | running |
| --- | --- | ---: | ---: |
| deployer before anything | | | **29.740521650** |
| trading Buffer write (rent 11.070361400, fees 0.010780000) | | −11.081141400 | 18.659380250 |
| **trading `Upgrade`** | `4L2aZ7xseut61fozox2gN2TCR8T7fKwodydsD4aZyxmdnAcb1UJpLDzd4jw98YfSmzHMLdpesGrDp6v4jHC87hhJ` | +11.070356400 | **29.729736650** |
| core `ExtendProgram` +10,240 bytes | `5cvBkn9ZtJhL85GzxAZCnpwfHGv2UYPCBLTqf2SeysYBFi3vu2DFVdTVhYrNZiTRDz1zsPdRndukDUHeJiDrHJbV` | −0.052024200 | 29.677712450 |
| core Buffer write (rent 6.028197240, fees 0.005875000) | | −6.034072240 | 23.643640210 |
| **core `Upgrade`** | `2p7YP6P3a9RG32L82aS5NXfmfqUeENaUa6d1ReKsKYNbbuLxB6n8XH99GMABpyk1EDv3dAYzN3st68CDBZhPUdds` | +6.028192240 | **29.671832450** |
| the re-release ladder, 14 transactions | | −0.014095440 | **29.657737010** |

Net **0.082784640 SOL**, of which **0.052019200** is permanent rent on Core's
widened ProgramData (10,240 × 5,080 exactly) and **0.030765440** is fees. Both
ProgramData rents are otherwise unchanged: an upgrade that shrinks or fits pays
no rent and refunds its Buffer exactly.

| role | ProgramData | slot before | slot after | live ELF after | pad |
| --- | --- | ---: | ---: | --- | ---: |
| trading | `7RxAyfAUd3hEENzog4Faq4tqpzFfA6riM1jnYVLEgSwx` | 493,639,190 | **493,748,086** | `d7430226e9e14313faa09b0c8f21f367392726f7961df656e610918d270d9efa` | 776 |
| core | `BbyZZAwbz37VwLR6zMQMm2bJAhfqbJVFAxr9HbFRQ5AU` | 493,639,301 | **493,752,818** | `691cbc699e08d5d55b2dd404b8a0819720a3b5cecb010e64e408f1001a53da05` | 10,192 |

**A deployed link's digest is not its candidate's.** Loader V3 zero-fills
ProgramData past the new payload, so the digest the release binds is the raw ELF
followed by however many zeros the account is wider. The tooling has always done
this — `--target-live-elf-bytes` is `max(live, candidate)` and the receipt calls
its dump `live-elf-with-zero-padding` — and cohort-16.1 is the first cohort where
the two numbers differ, because it is the first that upgraded rather than
deployed.

Each live image was dumped back and compared over its whole width before the
next role moved, and the deployment set closed at **7 of 7**, `final_set_sha256`
`9783225c374ff4b1c8749f8dfb3ae05ad1f262dece8d2ee5718dde25cd52d177`,
`mutation_permitted: false`.

## 4. The re-release, and the two generations side by side

`campaign --through activation` on the sealed plan: **14 transactions, zero
errors**, and a second preflight that READS THE CLUSTER reports substrate,
publication, initialize, succession and activation all `complete`. Publication
had to mint exactly three records — `trading_artifact_release`,
`core_artifact_release` and `execution_release_set` — because the other seven of
ten were already finalized and content-addressed identically.

| | cohort-16 | cohort-16.1 |
| --- | --- | --- |
| execution release set | `85defd75b236b191de00b48e673cdc4a4bcc2408b2248c4504895815b04cc69f` | `f533be49753dd4f709c4fa58a9f4ac05c43995c168b9d16cd5ee263fef6ed839` |
| activation cache | `2xVxMvfypJyo9bacGz1FFeK4L2qgqcsHaGoR9cbun6wV` | `FCF1ggHcXoZaVx8PKS7YKnY166xL4E8N3ZaRsV29E11b` |
| both live | 1,288 bytes, `DCLTACT1`, 0.00719328 SOL | identical shape |

Both caches exist. They are different accounts, and no instruction moves a
market from one to the other. `GyD95eyE…` names the first for as long as its
account exists.

## 5. The market, re-founded and activated

| fact | value |
| --- | --- |
| Open Market | `BMK3BY415TicG5nTf43ii7YncgMyVcGGySoKWeGXKLKG` |
| Found31 Market | `3xoSXBVsAXENB1RPq4sqS8euCksT1qsnnz83eWQPEtgY` |
| release set read off chain at offset 208 | `f533be49753dd4f709c4fa58a9f4ac05c43995c168b9d16cd5ee263fef6ed839` |
| capability manifest record | `GbWjTWP1nJ4eaZGCAhisGnKAusHLKMEFCbzvLLqh54K4`, 2,128 bytes, `DCLTCAP1` |
| selected entry index | **0** |
| Trading funding ledger (selected) | `Cx81tPLZ7roVtS7sBuq4gGR53ii6CWJx2SiFng1J3nHQ` |
| Resolution funding ledger (preserved) | `E6q7VNikjQF5fS1kWGPZMEne8mH8erwyaXxAqLY8pcLt` |
| linked basis record | `GbQGoztaDGN5uAnU41be8WDzLQX9vkhEHM6UxyUSNHLS` |
| collateral mint / wallet | `8WBx5RfQU2pVjNKoK4EUnPoAH4GniDpgcnQDSd7EiXKy` / `9Bsuacno5tUY8KeBUGHwAB1P8t8Rxcd2FkmxNqRRmwyp` |
| reserve | 1,000,000,002 atoms; budget 500,000,001; scale 3; complete sets 166,666,667 |

The founding landed on the first attempt, and it exercised its own hostiles on
the way: `DCLTPCB2` rolled back a reordered `FundingLedgerV2` tail at 44,653 CU,
and `DCLTGMF3` refused a substituted Claims request with Trading `0x4003` at
31,420 CU and rolled the whole founding back. Both are deliberate probes inside
the campaign, and both are the frames the repairs of 2026-09-05 installed.

### The activation

    verdict            ACTIVATED
    signature          553E4rJHk2ZgYaNjUxhr9m6Z5SAe4jz33zq4q6anLvCm9TjDueUY9p4TR3H3NdCnApjpavTTdiD1qwB9sDbzdJ6H
    slot               493,763,946
    compute units      521,895
    instruction        36 accounts, 528 data bytes
    activation root    2j8x2wQpUr68kPRGpK1ihGLmEW15GpdKjie99iaQemp5, 256 bytes, phase Open
    entry index        0, generation 2
    fee                75,000 lamports; four routing-table transactions before it

**36 accounts is the whole finding of the cohort-16 wall, executed.** The
one-ledger frame was 35. The extra account is the Resolution-owned dependency
ledger `E6q7VNik…`, and the reason it can be there is 17A's repair: the
interpreted runtime frame is the root and the selected ledger, exactly as the
native-close route already built it, and the dependency ledger is authenticated
outside that frame.

**The offline harness predicted this to 115 CU.** 17A measured 521,780 for
`canonical_activation_admits_the_selected_entrys_two_ledger_closure` against
real Core, Trading and Registry ELFs, with the fixture parameterised to this
market's shape. The chain says 521,895. That is a cross-boundary agreement
between a program-test harness and devnet, on a route neither had ever run, of
0.02%.

## 6. Three defects this run found, and each needed a run

**(a) Nothing binds a completed Upgrade row into the deployment-set journal.**
The only code in the tree that renames a temporary file onto a deployment-set
journal is `devnet-deployment-set-already-current-v1`
(`tools/local-validator/bootstrap/successor/src/upgrade.rs:2370`), and that is
the *other* disposition. `devnet-upgrade-v1` writes its receipt and its dump and
stops; the audit then refuses *"trading dump exists but the set journal pins no
digest"*, and the whole set is stuck. Cohorts 14, 15 and 16 never saw it because
a genesis cohort's five owned roles are all AlreadyCurrent. Cohort-15's
`re-admit` runbook row already says "rewrite `upgrade/deployment-set.json`",
which is the same operator act with no producer. This run's operator act is
`bind-upgrade-row.py` and `bind-baseline.py` in the job directory; each refuses
every row but the exact one, and each writes a PIN that `prepare
--deployment-set-journal` then re-authenticates against the chain.

**(b) The checked-upgrade phase loop cannot beat a fast cluster's blockhash
window.** Every phase transition re-audits the whole seven-role deployment set,
so the gap between `getLatestBlockhash` and `sendTransaction` is about **44 s**.
Devnet was producing **6.13 blocks per second**, measured over 30 s, which makes
the 150-block window **24.5 s**. Every attempt died `BlockhashNotFound` at
simulation, three times, having spent nothing. The Upgrade has a designed escape
— `--adopt-existing-buffer` with `--adopt-finalized-cli-upgrade-signature`, which
hands the Loader transaction to the pinned CLI and adopts its finalized
signature into the same checked receipt — and that is what landed both upgrades,
with the receipt's own arithmetic and dump verification intact. **The extension
has no such escape.**

**(c) A 48-byte `ExtendProgram` is refused by the Loader itself.**
`devnet-upgrade-extend-v1` computes the exact top-up for the exact shortfall, and
Core's shortfall was 48 bytes. The deployed Loader answers *"ExtendProgram
requires a minimum of 10240 additional bytes or to extend to maximum size, but
only 48 were requested"*. The driver's exact-top-up arithmetic is unreachable for
every shortfall below 10,240 bytes, so the extension went through the pinned CLI
at the Loader's minimum and Core's ProgramData is now 10,192 bytes wider than its
ELF — which is where §3's padding comes from.
