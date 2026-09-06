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

---

# Addendum, 2026-09-05 evening (lane COHORT-16D)

**Devnet evidence. Not mainnet evidence.** Written after the acts below landed;
the document above is unedited.

## 1. The capture window was open, and the resume note was wrong about it

The hold state left by COHORT-16C said the market's capture window was "long
past" and told the next lane to expect to re-found. It was not past. Staging was
18:22:29 local and `window_lead_seconds` is 2,400, so the window opened at
**19:02:29** and closed at **19:32:29**; the market was decoded from its own
`window_spec_hex` (`DCLTWIN1`; start at offset 48, end at offset 56) at 19:03 and
the capture fired inside it. Nothing was re-founded and nothing was spent on a
second founding.

The lesson is narrow and worth keeping: a schedule estimate written into prose
is not the schedule. The window record is, and the market carries it.

## 2. `PacketTooLarge` was a missing producer, not a defect

COHORT-16C recorded the admission refusing
`admission message compilation: PacketTooLarge` and recovered six routing
lookup-table addresses by hand from their creating transactions. The refusal is
real and the recovery was correct, but the cause is one level up: the admission
routes through the founding's own **frozen** routing address lookup table, the
simulator reads that table from `sim-config.json`, and **no runbook row produced
`sim-config.json`**. Every cohort from 12 on hand-wrote one in its job
directory. That is the producer-missing pattern — a reader, a schema and a
refusal, with the producer never written — inside the admission path.

`tools/cohort/build-sim-config.py` is the producer, and `sim-config` is a
runbook row that blocks `admissions`. It **derives** the table instead of
accepting a list: `simlife_drivers.frozen_routing_table_for` reads the
founding's own `create DCLTGMF3 frozen routing address lookup table` transaction
out of `campaign-open.json`, asks `getTransaction` which table that
`CreateLookupTable` created, and then authenticates the account — owned by the
Address Lookup Table program, authority `None`, and routing this founding's own
market — before any driver sees it. The derived answer equals the hand-recovered
`9tVU5HiCrG5F6RhQ1nZrSRtAZrHU7bztKMizREXpkweH`, which turns the recovered list
from an input into a check. The buyer's delegated allowance is derived the same
way, `gross + floor(gross x bps / 10000)` = 201 atoms from the manifest's own
economics, rather than restated a third time.

One coordinate is worth stating because it costs a refusal to learn: the market
address for the sponsored push and for the Direct tickets is the founding's
`founding_market` (`3xoSXBVsAXENB1RPq4sqS8euCksT1qsnnz83eWQPEtgY`), not
`market`. Passing `market` earns *"the Source resolution state derives to
8Edj1rtp2axGNKwC3tumQmzUMUVEyrzJtHHdJmGsWSQ2, and the campaign recorded
E9FiMWJRRAeFcBfhwsjAhFNNhSrrU1DbdJY4wCgNqfyZ"*.

## 3. What landed

| act | signature | slot | CU |
| --- | --- | --- | --- |
| relay capture | `64cuxbYamuTyj2r8dZLptYzi7Rq4zVLLB7DTGGkeyzv4W9DzaKupJxiPbAnVhcJm4aPc5bCahNBWDKgCLgGJDWsC` | 493,772,406 | 119,272 |
| Direct fill (Hot) | `3xQCjpbc5bLjiVUcApjpitRm3CFqeG8wczjDdj5h4BTDeaEiU81rWjFkGqL8m3QxYmYXn7gSuWhqvZosYLzYHMNf` | 493,776,766 | **1,111,824** |
| fee settlement | `1veFd4UPxt9nADLZvQzpCsJPFFPuWaZ8XF9BtWZ3yjD7SVChBLZ8viGiQtBFDDRAYdyx8ejnD4aaPv3VdcG9kz1` | 493,777,469 | 91,853 |

The Hot figure is against a 1,400,000 CU ceiling and beside cohort-15's
**1,137,522**: 25,698 CU lower on a larger release generation.

Two participants were admitted through the routing table with no packet
refusal — Positions `AV7T6Fgm…` and `CugxGD9L…` — and the buyer's delegated
account `GHhzng5kxd1ESR1eaGjVigsrPtJN7rJnkLuzDmevLZBp` held **exactly 201 of 201
atoms**, which is the `admissions` row's own verifier.

The two Direct token PDAs were derived from the published seed domain
(`dclutch:direct-token:v1` ‖ market ‖ generation_le ‖ owner ‖ role, under
Trading), both at bump 255, and both were created in-session: seller
`5aAVNVsnANhvY2DsQ9f34s2fzfssfKG2unLQMPCYxjth` holds 199, venue fee
`ucnwPJBVxSV7omV3tGtVZKysy9fd2ZrGcNKjQDySjBC` holds 2 after settlement. The
derivation's own instrument was checked before it was believed: three real
wallet keys decode **on** the ed25519 curve and cohort-15's known Direct token
PDA decodes **off** it.

`0x8011 ProviderWindow` on capture attempts 1 through 3 is not a defect and its
own doc says so — *"no answer rather than a wrong one, and the market is still
live"*. The sponsored price account `7UVimffxr9ow…` is refreshed by a live
pusher; its `publish_time` entered the window at 19:06:53 and attempt 4 landed
at 19:08:18. Cohort-15 saw the identical sequence. **Nothing should be widened
for this code.**

## 4. THE DEFECT: the founding seats three of four outcomes

    VIOLATED L3: Positions sum to [166666667, 166666667, 166666667, 0]
                 but the aggregate owes [166666667, 166666667, 166666667, 166666667]

The founder Position holds **nothing at outcome index 3** while the Claims
aggregate owes a full complete set there. Four readings converge on this being a
founding-time fact and not a measurement artifact:

* nothing on the admission path mints or moves claims, and the two admitted
  Positions read `[0,0,0,0]`, so the gap predates them;
* the fill does not change it, because a transfer between two named Positions
  preserves the sum;
* it is not a naming gap — the census was re-run with every Position the
  founding recorded, and `claims_admission` is not a Position at all
  (`InvalidMagic`);
* cohort-15's founding census, through the same code, reported
  `HOLDS L3: 1 Positions sum to the aggregate supply vector [500000000 x4]`.

**The fill's refusal is the same defect.** The Hot transaction LANDED — err
`None`, 1,111,824 CU, finalized at slot 493,776,766 — and then
`devnet-direct-trade-v1` refused

    REFUSED: Direct terminal claim schedule is not the exact K+1 partition

which is `authenticate_direct_claim_schedule_v1`
(`tools/local-validator/bootstrap/successor/src/direct_trade.rs:3908`): it
requires `claim_balances.len() == outcome_count + 1` = 5, and
`direct_claim_balances_v1` collects only NONZERO balances, so the seller
contributes 3 and the buyer 1. The seller cannot contribute a fourth row it
never held. **The chain accepted the trade; the driver will not certify it**, and
the certification is what the terminal path downstream consumes.

Cohort-16.1 is the first cohort founded at **payout scale 3 over basis width 4**
(reserve 1,000,000,002 atoms, budget 500,000,001, complete sets 166,666,667);
cohort-15 ran scale 1 and held. That is where an owner should look first.

## 5. The census, L1 through L8 by name

Post-fill, chained to the post-admissions boundary:

    HOLDS         L1  tracked 1000000002 atoms across 5 accounts == Mint supply 1000000002
    HOLDS         L2  the Hoard moved 0 atoms since post-admissions, exactly as declared
    VIOLATED      L3  §4
    HOLDS         L4  Hoard 500000001 >= worst outcome 166666667 x unit 1
    HOLDS         L5  tracked collateral moved 0 atoms, exactly as declared
    HOLDS         L6  no watched account closed at this boundary
    INAPPLICABLE  L7  three accounts admitted that the previous census did not watch
    INAPPLICABLE  L8  external census; the compartments were not declared

L7 and L8 are inapplicable **by name and for stated reasons**, not passed.

## 6. The General market: the blocker was convicted, then removed

`found-general-family` needs `general/translation-validation.bin`. Cohort-14
minted `aa97c0c10a98248f9ada4dccc96a5a4e969073cb57ded04340efe21b6996f4f8` from a
real run, and **cohort-15 reused that exact file**. Reusing it a third time is
not available, and that is measured rather than assumed:
`create-translation` hashes 21 named inputs and three of them moved after
cohort-14 — `Cargo.lock` (34 commits since 2026-09-02),
`crates/dclutch-trading/src/lib.rs` (3) and `crates/dclutch-vm/src/lib.rs` (4) —
while every Lean source among them moved not at all.

So it was re-minted rather than inherited.
`tools/direct-translation-validator/check.sh` ran green against this tree —
137 Lean jobs then 10, then *"translation validation passed: 91 Lean ABI values,
7,280 single-byte ABI mutations, 3,318 hostile ABI widths, 521 Lean VM states
(147 accepted, 374 refused with rollback), 514 Direct AOT states, registered
creation corpus 14 ABIs / 2,128 mutations / 2,142 hostile widths, 19 registered
terminal transitions, 4,096 deterministic Rust roundtrips"* — and
`dclutch-release-tool create-translation` minted the canonical 688-byte
`CheckedTranslationValidationV1`
`84ddf2591716eb00bb43cec5f07543ad7629b70201a5412f5020ca05adc5f54f`.

It remains **Direct-shaped**. There is no General translation-validation corpus,
so this is evidence about a different program, and it is named here rather than
quietly inherited — the third cohort running to have to say so.

**The accelerator's source drift is gone, and that is measured.** Cohorts 14 and
15 each had to record that the deployed accelerator's source had moved after its
deploy. This cohort's own candidate ELF
(`candidate/elf/accelerator.so`, 736,056 bytes) hashes
`587181d9536d19f4ed40ddebb01ebac1d3e3544d28110c3d7e1ca04b4e4c87ab`, and the live
ProgramData tail at `DfJLGB1W12cUYGpw3doG2DmMDe6ubR2UkmrrUsqosa9g` hashes **the
same value over the same length**, with `live_elf_padding_bytes 0`. The
accelerator's semantic release id was derived from that artifact rather than
copied: `7c0a3cee0f643595da5970de505199e5cb4c4be313cecc25f096e3753e5b2fe6`.

`devnet-general-market` then compiled a **398,024-byte** manifest against
deployment slot **493,639,473**, and the founding campaign's preflight is green
with `shortfall_lamports 0`. Two chain-owned facts moved under it and the
sponsored release was re-minted against finalized slot 493,780,905:
`receiver_deployment_slot` 487,855,452 → 491,006,444 and `receiver_config_digest`
`bbbc324e…` → `f8aca67e…`. **That is the Pyth receiver redeploy again**, the same
root cause behind every stale release pin this project has chased.

The founding was deferred here on the belief that it would collide with the
settle on one payer key. **That belief rested on a misread clock and §10
supersedes it**: the settle was almost two hours out, not fifty minutes, and the
founding was executed and activated. The deferral reasoning is left standing
rather than deleted because the mistake it records — bounding a devnet act
against a time the lane never checked — is what §10's burned market identities
came from.

## 7. The settle is a clock

`window.end` 19:32:29 plus `max_age_seconds` 7,200 makes the settle legal from
**21:32:30** and due at **21:32:59**. Nothing downstream of it — admit-terminal,
the payout, CloseFund, retirement — can begin earlier, so the first retired
market on any chain was never reachable in this lane's earlier hours no matter
what else was done. The certificate seat was prepaid at 19:10 and `settle.sh` is
the bounded waiter: its guard exits rather than falling through, each attempt
gets a fresh output path, and it reuses the prepaid `input-settle.json` because
regenerating that document would move the certificate PDA off the seat that was
paid for. Its ceiling **refused** a 7,716 s wait until the wait was stated
deliberately, which is the ceiling working.

## 8. The settle, the terminal, and the winning stranger paid

| act | signature | slot | CU |
| --- | --- | --- | --- |
| settle | `641JyvMWqAGyUJhaK1fXo7FvH2YQPH3fjLQh5Shau9XpZcJMuo2wUcRePHabc4udKyNUP8qUjddxjEwzgU6ZqeJX` | 493,825,024 | 131,192 |
| admit-terminal | `5LCBtiezNduDBXyVtDnhfSKQhNHgoHYDLr9p7bdFyL4xYefQDCJtkUDaRrvkxqaLq2xASitrmf4V8Q3U4idMv9r5` | 493,826,348 | 82,282 |
| custody replay | `4LfQK3ReWZKPNA4gVomPX79iQccWhNcC6pKKkRvwCAeZwdYFDzva3C3TggNPUr4fe3WwEeCxc8GwmiN6dbhjsNYc` | 493,826,534 | 106,272 |
| **stranger payout** | `5hkFEF7ZjMcsZfvWFKNW6E89ahnCe5Sb4p7MNhQixiJfdBMjukDRXHWoRrqh1jheEhjvaPnDMfeQ53SfJDYkwiho` | 493,827,300 | 220,484 |

The settle landed on attempt 1, and its certificate `CEjAknLzC8AoDzLj7bXXoF4hDxieY7gg1xfoZcZMf9Xk`
is 312 bytes of `DCSRCER2` with **kind 1, not 4** — the row's own verifier, read
off the account rather than off the report. The Market's phase byte at CoreState
offset 10 then went **1 to 2**, and `terminal_receipt` at offset 328 carries the
certificate's own address: the `admit-terminal` verifier, both halves.

**The winning stranger was paid, and this is the first time on this protocol.**
Participant-2's account went from 0 to **600 atoms** and the Hoard fell from
500,000,001 to 499,999,401 — a fall of exactly 600, which is the `payout` row's
verifier to the atom (200 claims at payout scale 3). Cohort-14 traded outcome 0,
drew cell 1 and paid zero; cohort-15's fill never landed. The founder was then
paid at claim indices 1, 0 and 2 (166,666,467 then 166,666,667 twice), and
**the Hoard reached exactly 0**.

*The honest selector, both readings.* Outcome 1 was written into the cohort-16
manifest at 12:59, hours before this lane existed and before any capture. But
this lane authored the two intent tickets at 19:16, **eight minutes after the
capture landed at 19:08**, so it could have chosen knowing the reading and a
reader is entitled to assume it might have. It did not re-derive the cell; it
took the manifest's number. Both facts belong here rather than only the
flattering one.

## 9. RETIREMENT IS UNREACHABLE, AND THE WALL HAS MOVED UPSTREAM

The market reached Terminal, paid every holder, and drained its Hoard to zero.
It still cannot retire, and the reason is §4's defect reaching its conclusion:

    BeginRetiring is blocked: Claims supply at index 3 is 166666667;
    produce and execute wallet terminal payouts first

Index 3 is the market's fourth outcome — `--cuts 10200,10600` makes three cells
plus the explicit failure outcome, and `coefficients [1,0,1,0]` pays 0 there.
The Claims aggregate minted a full complete set at that index and **no Position
was ever seated with it**. The instruction "produce and execute wallet terminal
payouts first" cannot be followed, and that is measured, not inferred:

    wallet-terminal-payout-input --claim-index 3
      founder        -> payout quantity must be within 1..=0 atoms at claim index 3
      participant-2  -> payout quantity must be within 1..=0 atoms at claim index 3
      participant-1  -> payout quantity must be within 1..=0 atoms at claim index 3

A range whose upper bound is below its lower bound is the shape of an empty
holding. And it is not a holder this lane failed to name: every other founding
identity — beneficiary, projection witness, source funder, substituted founder,
campaign payer — has **no Claims Position account at all** ("wallet payout
snapshot is missing Claims Position ..."), so the three that exist are all there
are, and all three read zero at index 3.

So the chain closes: the founding seats three of four outcomes; L3 says so at
the first census; `authenticate_direct_claim_schedule_v1` refuses to certify a
landed fill because the seller cannot supply a fourth nonzero row; every payout
drains its own index; index 3 has no holder to drain; `BeginRetiring` refuses
while its supply is nonzero; and the only mechanism that could lower it refuses
to be built. **This market can never be retired, by any sequence of acts.**

**The wall this project has been tracking has moved.** `tools/cohort/README.md`
records the retirement wall as `DirectCloseCapability`, blocked because a
market's manifest declares no dependency edges — *"a cohort that intends to
retire must found a manifest whose Direct entry declares its Resolution
dependencies. Until then no market reaches this row, and that is why retirement
has never completed on any chain."* Cohort-16.1 **did** found such a manifest
(COHORT-16C's activation carried the dependency ledger, 521,895 CU over 36
accounts), and this run got past nothing else to reach it: the sequence never
got as far as `DirectCloseCapability`, because `BeginRetiring` is stage one and
it stopped there. The named wall was not refuted and was not reached. A new,
earlier one was found in front of it.

`outstanding_capabilities` at CoreState offset 280 still reads **1**.

Two coordinates the next lane should not have to rediscover: the terminal
sequence refuses the founding's DCLTGMF3 routing table (*"supplied terminal ALT
holds 11338560 against the 9387840 its funded rate of 5080 lamports per byte
prices 1720 bytes"*) and must be run with **no** `--lookup-table`, building its
own; and it refuses the founding evidence alone (*"the Direct first-use accounts
are created together by the first trade, so campaign evidence must carry all of
them or none. It carries direct_trading_funding_ledger and omits
direct_capability_root"*) until `devnet-refresh-evidence-v1` supplies the root —
whose flags are `--plan --expected-plan-sha256 --market-input
--expected-market-input-sha256 --campaign-report --expected-campaign-report-sha256
--output`, and which takes no `--market` and no `--evidence`.

## 10. The General market is founded and activated, and a founding cannot be interrupted

| | |
| --- | --- |
| Open Market | `65Yq3q6tgHArrJtZPhf5RAgNzpPAGcZSoBmT4D3As9n5` |
| atomic Found frame | `5dzKEFZDnxEPSQzVvKfKfQ…` (Lock, Found, Realize, Claims, Open, DCLTGMF3) |
| activation | `2y2NaDKpFQTwYypQhvsgyQ6kokXZaABQ98yfERmcDVQvZQCs4ad9Xzv9Fx9ZoGUh5YJae67dwQLsYhrU`, slot 493,809,950, **520,541 CU** |
| activation root | `4NcnH7ptihcAjYyNX2xbKypnkTCQCpLMBGscX7fduCsF`, 360 bytes, lifecycle **Active** |

The row's verifier is that the root named in advance is the root occupied, and
it holds exactly: `expectedRootTailSha256` and `rootTailSha256` are both
`a2717010554944b20b873403bd35438d4171071e810443645fcb6672f7be7082`.

**A founding campaign has no suffix resume, and this lane proved it the
expensive way.** The first attempt was bounded by a 55-minute `timeout` chosen
against a misread clock. The SIGTERM landed at 446 transactions, after every
ladder stage was complete — a rerun reports substrate, publication, initialize,
succession and activation all *"already complete, skipped"* — but inside the
founding, and the rerun then refuses:

    this founding has STARTED on this chain (the Open Market does not exist at
    H9TsXtAAcyJWQ3Wznza4mSqY36FFAmwEv4jEXP4qPEq1 but this founding has started:
    collateral mint 8bdPW7bb…, collateral wallet J31bkZKf…, realm record
    79bikSkH…), but no compatible durable DCLTPCB2 checkpoint authenticates a
    safe suffix resume

Those identities are burned and `H9TsXtAA…` will never exist. The recovery is a
fresh collateral mint/wallet pair and a fresh evidence path, which is what the
`-16d` founding is. **The publication stages resume and the founding frame does
not**, so an operator who must bound a founding must bound it before the frame,
never during. A killed campaign also leaves `campaign-open.json.lock`, which is
deliberate — *"locks are never removed automatically"* — and the operator act is
to confirm the recorded `pid` is gone and then remove it.

## 11. What is owed

1. **The founding's failure-outcome seating** (§4, §9). Every other item here is
   downstream of it. Owner: whoever owns the Direct founding's complete-set
   seating at `payout scale 3 / basis width 4`.
2. **The deployment-set journal's missing Upgrade-row producer** — COHORT-16C's
   defect (a), still unrepaired; `bind-upgrade-row.py` in the job directory is an
   operator act standing in for a producer.
3. The checked-upgrade phase loop's blockhash race, and the Loader's
   10,240-byte extension minimum — COHORT-16C's (b) and (c), unrepaired.
4. **A General translation-validation corpus.** Three cohorts running have had
   to say that the manifest they publish is Direct-shaped.
5. OpenBatch N=2, which is still the only route that would put a transaction
   through the accelerator and give it its first witness on any chain.
