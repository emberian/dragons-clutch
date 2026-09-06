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

# Addendum, 2026-09-05 late evening (lane PROGRAMS-17B): §4 and §9 are REVERSED

**The founding did not seat three of four outcomes. It seated four of four, and
this document's §4 accused a correct founding because the census could not see
the Position that holds the fourth.** The wall §9 found is real, it is upstream
of nothing, and it is a different wall: a refunding market's failure coordinate
is unpayable under every certificate and its holder is a keyless PDA, so
`BeginRetiring`'s zero-supply conjunct can never be satisfied by any market
founded under decision 0025.

## 1. The escrow exists, and it has held the failure column since founding

Cohort-16.1's basis is `CategoricalQ1` at width 4 with `payout_scale` 3 =
`basis_width - 1`, so `categorical_refunds_on_failure_v3`
(`crates/dclutch-product/src/payoff/runtime_v3.rs:69`) reads it REFUNDING. A
refunding founding runs `refunding_founding_vectors_v1`
(`programs/dclutch-claims-sbf/src/founding_v5.rs:1872`) and seats the failure
coordinate in the Market's own `ClaimsCapability` escrow. The deployed candidate
`87eec1c3a6bf954a4350931af62ce8d4fcc48da2` contains `ebbccbd4e` (*claims:
founding seats the failure escrow*) — `git merge-base --is-ancestor` — so the
Claims ELF on devnet is the seating one.

Every address below is DERIVED from the aggregate's own header (owner, logical
market, width) and then read off devnet at `finalized`. The derivation's control
is the founder's Position: the same three seeds reproduce
`EXYhY3YmHeB7AsDXqZed3Jy6LhUUku4c5MoxgsyZd4fa`, the account this document's
census already watched.

| | |
| --- | --- |
| logical Market | `3xoSXBVsAXENB1RPq4sqS8euCksT1qsnnz83eWQPEtgY` (the aggregate's `logical_market`; `BMK3BY41…` is the Found31 Market) |
| Claims aggregate | `CBzv1hhtToxpCaExaA7QqES4bMu5UjxiAiBW9bMUrCdg` |
| escrow owner | `Hq6sF5pv3i8CBkH46dsyN9fnzJi1jooS2gj6USCQmke3` — `ClaimsCapability` PDA at `(market, 3)` |
| **escrow Position** | **`7FQCfc4RrrsATEe969eNVYoLjDukmBVKMAxM1yg7AzcQ`** — Claims-owned, 160 bytes, `DCLLBP02`, 1,463,040 lamports |
| escrow admission | `4WUZ2qZKz7nkgGnnNejP8cLNHhjKCFCpHwNVDikE3T9b` — Claims-owned, 512 bytes, `DCLPPS02`, 3,251,200 lamports |

The escrow Position's own header names the aggregate as its market and
`Hq6sF5pv…` as its owner, and its vector reads

    escrow  [0, 0, 0, 166666667]   revision 1
    founder [0, 0, 0, 0]           revision 5
    aggregate supply [0, 0, 0, 166666667]  revision 6

**Revision 1 is the founding write.** The escrow has not moved since it was
seated, which is what makes the first census reconstructible: at
`post-admissions` the founder held `[q, q, q, 0]` and the escrow held
`[0, 0, 0, q]`, so

    [166666667, 166666667, 166666667, 0] + [0, 0, 0, 166666667]
      = [166666667 x4] = the aggregate

**L3 held.** That sum is `addFrom_addBelow_eq_addEvery` in bytes — the law
`EconomicKernel.lean` proves and `the_two_founded_positions_sum_to_one_complete_set`
(`founding_v5.rs:2528`) already asserted in Rust. Nothing was wrong with the
founding, and §4's four "converging readings" converge because all four were
taken through the same aperture.

## 2. What was actually wrong: L3's aperture, not the founding

`tools/gauntlet/journey/src/ledger.rs` states L3 as
`now.position_totals == now.aggregate_supply`, and `position_totals` is summed
over `self.positions` — a map an operator fills with `--position LABEL=PUBKEY`.
The escrow is a PDA nobody types. So the law reported supply nobody owns, which
is exactly what an unwatched holder looks like, and the census had no positive
control that could tell "no Position holds this" from "I was not given the
Position that does".

The repair is a derivation, not a weaker law: the ledger now derives the failure
escrow from coordinates the aggregate account itself carries
(`ledger::failure_escrow_v1`) and joins it to L3 under the reserved label
`failure-escrow`, so a census that reads the aggregate closes L3 without being
told anything further. The founding frame builder
(`tools/local-validator/bootstrap/successor/src/market.rs`) now asks the same
function instead of spelling the three derivations inline; that inline spelling
being unreachable from the census is the whole mechanism of this misreading.
`the_derived_escrow_is_the_account_cohort_16_1_founded` pins the derivation to
this market's real addresses.

Two consequences of §4 inherit the reversal:

* **The `K+1 partition` refusal is a categorical-only rule, not evidence of a
  gap.** `authenticate_direct_claim_schedule_v1`
  (`tools/local-validator/bootstrap/successor/src/direct_trade.rs:3900`) demands
  the seller contribute a nonzero row at every outcome. On a refunding market
  the seller structurally cannot hold the failure column — the escrow does — so
  the rule refused every refunding fill it was asked to certify. The chain
  accepted that trade correctly. **Repaired**: the schedule is now joined
  against the two Positions' own nonzero coordinates, which the evidence already
  carries and its digest already covers, so the count assertion is replaced by
  the invariant it stood in for — no coordinate a Position HOLDS is dropped from
  its zero-collateral burn. Strictly stronger than the count: a row at the right
  index carrying a quantity the Position does not hold used to pass.
* **`the_founder_is_issued_no_failure_claim_at_all`** (`founding_v5.rs:2554`) is
  the ruling, not the defect. §4 read the ruling as the bug.

## 3. THE WALL: a refunding market cannot retire, and no founding change fixes it

**Where the conjunct actually lives.** Core's `BeginRetiring` and Trading's
`DirectBeginRetiring` do not read Claims supply at all; the on-chain rule is the
market closure's, `programs/dclutch-claims-sbf/src/market_closure_v1.rs:656-668`,
which walks the aggregate and refuses `ClaimsMarketClosureSbfErrorV1::Liability`
-- `CLAIMS_REFUSAL_BASE + 0x503` = `Custom(0x5503)` under decision 0007 -- on any
nonzero coordinate. The operator hoists that same conjunct to the front of the
chain (`crates/dclutch-operator/src/wallet_terminal_input.rs:456`), which is why
cohort-16.1's refusal arrived as a host message and no `BeginRetiring`
transaction was ever built. The wall is real either way: a market admitted into
Retiring on this state would meet `0x5503` at closure instead.

On a refunding market the failure coordinate's supply can never reach zero, and
there are two independent reasons, either of which alone is fatal.

**It is unpayable under every certificate.** `evaluate_categorical` refuses
`FailureCoordinateNotPayable` when the selector IS the failure coordinate on a
refunding record, and `evaluate_categorical_failure` pays "one collateral atom
to every ordinary claim, nothing to the failure coordinate"
(`crates/dclutch-product/src/payoff/runtime_v3.rs:972`, `:990`). So an ordinary
resolution pays it nothing and an outage pays it nothing. There is no
certificate under which a payout drains it.

**Its holder cannot sign.** Terminal settlement under `CallerRole::Claims`
requires coordinate 0 to be a signer and to equal the Position's owner
(`programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs:635`). The program
says so in its own words at `:620`:

> A Trading record owner and a Claims capability owner are both program-derived
> addresses with no key, so neither can produce this proof

The escrow's owner is exactly a Claims capability owner. The program's
`payout == 0` arm exists and is complete (`terminal_settlement_v3.rs:506`,
`:939`); what does not exist is anyone who can authorize it for this Position.

**The one route that CAN move the escrow closes before it is needed.**
`MergeRefundingCompleteSet`
(`crates/dclutch-product/src/economic_slice/mod.rs:150`) burns the ordinary
coordinates out of a holder and the failure coordinate out of the escrow — but
it is a complete-set merge that returns collateral from the Hoard, and Terminal
drains the Hoard to zero. By the time retirement is the question, merge is not
available.

So §9's sentence *"This market can never be retired, by any sequence of acts"*
stands, and its scope is far larger than this market: **every market founded
under decision 0025 is unretirable, on any chain, whatever the founding does.**
The escrow was seated with no discharge. That is a fact decision 0025 did not
have, in the same shape as the merge foreclosure the ESCROW lane found before
it, and the choice of repair is recorded there for its owner.

## 4. What this addendum does NOT claim

The census outputs in `~/jobs/dclutch-cohort161-20260905/census/` are not
withdrawn: every number in them was read off the chain and is correct. Only the
L3 VERDICT is reversed, and only because the set it summed over was incomplete.
L1, L2, L4, L5 and L6 are unaffected. L7 will read `inapplicable` at the first
boundary after the derivation lands, naming `failure-escrow` as an admitted
account — which is the ledger's own declared behaviour for a widened aperture,
not a new gap.

**Not re-run on chain.** This lane took reads only: `getAccountInfo` and one
`getMultipleAccounts` at `finalized`. No devnet act. The redeploy that carries
the repair is cohort-17's, because the repair is a program change and a Claims
ELF that moves is a re-release plus a re-found under decision 0012.

---

# ADDENDUM, 2026-09-05, lane COHORT-16E: the General capability seal is on chain, and OpenBatch is two walls deep

Devnet execution evidence. Not mainnet evidence. Written at
`/Users/ember/dev/dclutch`; the job directory is
`~/jobs/dclutch-cohort161-20260905/` and every artifact below lives in
`seal-general/`, `openbatch/` and `logs/`. The market is the General market §10
founded and activated: `65Yq3q6tgHArrJtZPhf5RAgNzpPAGcZSoBmT4D3As9n5`, root
`4NcnH7ptihcAjYyNX2xbKypnkTCQCpLMBGscX7fduCsF`, release set `f533be49…`,
activation cache `FCF1ggHcXoZaVx8PKS7YKnY166xL4E8N3ZaRsV29E11b`.

**The accelerator `6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y` still has no
witness on any chain.** It was reached for and not reached: the OpenBatch
transaction refuses inside Trading, before any CPI, and the four `Program
6v1c2Go2… invoke` lines the route would produce do not appear in any log.

## 1. The seal: the first General capability seal on any chain

`devnet-capability-seal-v1` composed the permissionless `DCLTSEL1` outer for
the descriptor and action the `devnet-general-session` frame report states.

| | |
| --- | --- |
| seal | `HH7c3zEH1VzfE8Jzs3yfrRpCirMYzJTqpjgAupxzyLhF`, bump 250, 968 bytes, owner Trading `ESQhDyV7…`, 5,567,680 lamports |
| descriptor digest | `47a5303dd4ba2e24c4c8ce5f6a08289d21b8b2c004258580e43b9d8df064af60`, action 7 (`OpenBatch`) |
| Trading semantic release | `79fad2f04f8d9ce07d76c809fe116db8ef9374adbeb15e62f603235c3a2b96b9` |
| signature | `5tu23XBMS47p7YFVVnmKzDhGePo4zAV6wB1jVETvn8356yX27u2iHwcPrCDZy4qDXez6RhS545j2Lfn91HemifLT`, slot 493,837,946, **625,058 CU**, 41 accounts |
| routing table | `2U6tDceJaVLme1TVgqFYZQSbGn6paL59BySEmJCJzqWB`, published in four transactions and frozen |

The row's verifier holds: the seal exists at the address the builder derived
from the four seeds, which is the address the frame report stated at fixed
coordinate 38; it is Trading-owned and rent-exempt. One correction to the
runbook's wording, which says the logs *"show Trading and ComputeBudget only"*:
they show Trading, ComputeBudget **and System** — Trading CPIs System to create
the seal account, so the third program is the account creation and not a second
authority.

## 2. THE FIRST WALL, AND IT IS OURS: the General plan never declared a compute limit

The first OpenBatch attempt reached Trading and died:

    Program ESQhDyV7… consumed 202842 of 202850 compute units
    Program ESQhDyV7… failed: exceeded CUs meter at BPF instruction

`compile_general_hot_v0` pushed `request_heap_frame` and nothing else, so every
General successor transaction this tree has ever compiled carried the runtime's
DEFAULT per-instruction allocation — 202,850 units against an action the
harness measures at **654,000–677,000**. The harness could not see it:
`programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs`
pushes its own `set_compute_unit_limit(waist::COMPUTE_LIMIT)` beside the heap
frame, so the shipped builder and the only caller that exercised the route were
two authors for one frame, and every General CU figure ever published was
measured against a budget the operator never asked for. Direct has carried both
instructions since `DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1`; General carried one.

Repaired at `432d073393e14496ef6ed1973917d8392c18e1a0`:
`GENERAL_HOT_COMPUTE_UNIT_LIMIT_V3` declares the chain-profile ceiling, the
serializer's shape check becomes limit-heap-hot, and the ALT packet witness
moves +8 wire bytes on all seven N=258 actions with no account count moving —
widest 940 of 1,232. No crate compiled into an SBF link is touched, so the
deployed release set is unchanged and the candidate commit still stands.

## 3. THE PROBE DIGEST: the route's caller authorities were never the real ones

`devnet-general-session` derives the four admitted caller-authority PDAs from
`accelerator_caller_authority_digest_v1(Admitted, parent_request_digest, index)`,
and `--parent-request-digest` is OPTIONAL. Absent, it substitutes
`sha256(b"dclutch:devnet-general-session:caller-authority-probe:v1")` and says
so in the report — `parentRequestDigestIsProbe: true`. The `openbatch-refounded`
runbook row passes no such flag, so the route it produces always names four
addresses no execution can derive. The real digest is not knowable until
`general-successor-plan-v5` has produced the plan, which needs the route: the
composition is a two-pass loop and nothing in the runbook says so.

Measured, both passes on chain:

| | |
| --- | --- |
| probe digest | `5bc43870c202fcc7d1fa5a27e8afb2e21f41a022f3920faee1a0c2e8a40a581e` |
| real family request digest | `6ac8255569a9086fc414c8bd5fab5d941ec1f8157af44d7ab754bac90bff05c9` |
| span, probe | `DG7TaMXT…`, `6sttxgpp…`, `9C9t1V5Y…`, `4qZ4Nb4y…` |
| span, real | `AedW15zt…`, `DS8PzhWg…`, `EWdzZVom…`, `HNeJ2rWj…` |

**All four moved, and the refusal did not.** The second pass consumed
**239,781 units to the unit**, identical to the first, and returned the same
code. So the probe default is a real latent defect — a route that would refuse
`TradingSbfError::Release` the moment execution reached the caller-authority
derivation — and it is NOT the wall in front of OpenBatch. That is what an
identical CU says: the refusal is upstream of any use of those four accounts.

## 4. THE SECOND WALL: `TradingSbfError::Content` `0x4003`, and it could not be localized

    InstructionError [2, Custom(16387)]
    Program ESQhDyV7… consumed 239473 of 1399700 compute units
    Program ESQhDyV7… failed: custom program error: 0x4003

`Content` is *"Manifest, selected entry, descriptor, or config content refused"*
and it has **1,686 sites** in `programs/dclutch-trading-sbf/src`. The
transaction reaches Trading, spends 239,473 of the 1,399,700 units the repair
now grants it, and refuses with no accelerator invoke in the log. Ruled out by
measurement rather than by reading:

- the compute ceiling (§2), which is repaired and moved the refusal from
  203,000 to 239,473;
- the caller-authority probe span (§3): all four addresses replaced, CU
  identical to the unit;
- the capability seal's absence: it is produced, 968 bytes, Trading-owned, and
  its descriptor digest equals the plan's own `artifacts.descriptor`;
- the sealed-execution alias shape: `SEALED_EXECUTION_ALIAS_FAMILIES_V3` holds
  one row and it is Direct `InlineOrdinary`, so
  `hot_frame_uses_sealed_execution_aliases_v3(General, OpenBatch)` is false, and
  the frame carries ten distinct vacant staging cursors, so
  `frame.uses_sealed_execution_aliases()` is false too. The `!=` holds.
- the route's own conjuncts: the frame report names **no unsatisfiable conjunct
  at any of the 55 top-level coordinates** (`walls: []`), on every pass.

**The tree's own instrument cannot be pointed at this.** `hot_cu_checkpoint!`
is `#[cfg(feature = "hot-cu-profile")]`, so naming the phase means running an
instrumented Trading — and a release pins the ELF digest AND the deployment
slot, so an instrumented build refuses `Release` before it can print a
checkpoint. The instrument is usable in program-test and on no release-pinned
chain, which is a gap of its own. The tree's nearest precedent is
`docs/ledger/LETTER_TO_CLAUDE_2026_09_01.md`: a Dealer `Content` at 148,093 CU
localized *"between Hot checkpoints `root-product` and
`artifacts-strategy-effect`"* — the immutable-artifact authentication tranche.
This refusal is the same code in the same executor and the bracket is the
honest one to inherit; it is not measured here. **Could not be localized** is
the output.

## 5. CloseBatch has no producer at all

`devnet-general-session` hardcodes `const SESSION_ACTION_V1: Action =
Action::OpenBatch` and exposes no `--action`; it is the only command in the
tree that emits a `GeneralSuccessorRouteV1`. `CloseBatch` appears in no driver
source. So the sequence this lane was to run — OpenBatch, CloseBatch at the
batch's own close slot, then a second OpenBatch exercising the per-batch
selection — has one reachable step and two that no host can compose, whatever
the chain answers. That is the producer-missing pattern one action wider than
the seal was.

## 6. What landed, and the ledger

| act | result |
| --- | --- |
| GENERAL-SEAL routing table | `2U6tDceJ…`, 4 transactions, frozen; `5rk4sk3p…`, `2iKakFNu…`, `24CC3fWb…`, `rqohG27j…` |
| capability seal | `5tu23XBM…` slot 493,837,946, **625,058 CU** |
| GENERAL-HOT routing table (probe route) | `DK7DSu7U…`, 53 addresses, 5 transactions |
| GENERAL-HOT routing table (real route) | `Bmehez4x…`, 53 addresses, 5 transactions |
| OpenBatch | **NOT EXECUTED.** Two simulations, `ProgramFailedToComplete` then `0x4003`; nothing was signed or sent |

    payer 9FNUxCr2CvvTNE47KWhoyBC4LcuYJnyzeQGypkf88KAP
      1.417282684 -> 1.384214644  (0.033068040 SOL)
      of which 0.005567680 is permanent seal rent and the rest is three frozen
      routing tables and fourteen transaction fees
    deployer 28.657732010, unchanged; no top-up was needed

The batch-window state `CuW1791cAY1uqgtu1okAjEmY2iVHx8gsytQgs8WcayYQ` — the
address the request's own seeds derive — is ABSENT, as it must be while
OpenBatch has not executed. That is the row's verifier reading red, honestly.

---

# ADDENDUM, 2026-09-06, lane PROGRAMS-17D: §4's "could not be localized" is REVERSED, and OpenBatch has two walls, both the host's

Offline program-test evidence over devnet-captured accounts. Not devnet
execution evidence and not mainnet evidence. Written at
`/Users/ember/dev/dclutch`. The instrument and the recipe are
`programs/dclutch-trading-sbf/program-test/devnet-replay` and
`docs/design/DEVNET_FRAME_REPLAY_V1.md`.

**§4 above states `Content 0x4003` "could not be localized" because
`hot_cu_checkpoint!` is behind `--features hot-cu-profile` and a release pins
the ELF digest and the deployment slot. The premise is wrong.** A hot route
never hashes an ELF: `slot_pinned_release_elf_digest_v1`
(`crates/dclutch-registry/src/immutable_registry.rs:439`) compares the
ProgramData header's slot and authority and returns the release's own recorded
digest, so the pins live in 45 bytes and the ELF behind them can be the profiled
build. The exact transaction §6 recorded as NOT EXECUTED —
`openbatch/executed-plan-c2-preflight.json`'s own `transactionBase64`, its 55
accounts read back at finalized, its lookup table — replays in `ProgramTest`
against the deployed ELFs and reproduces `Custom(16387)`.

## WALL 1, and it is the one this cohort hit

    dclutch-hot-cu:p5-sealed-ownership-arena
    dclutch-hot-why:lifecycle-prepare case/ordinal/invocation/operand
    0x1a, 0x0, 0x0, 0x7          (case 26, plan 0, invocation 0, coordinate 7)

`programs/dclutch-trading-sbf/src/hot_v3/lifecycle.rs:1421`, the FIRST conjunct
of `authenticate_lifecycle_credit_v3`:

    account.key.to_bytes() != expected_key

reached from `prepare_lifecycle_v4` at plan 0, invocation 0, over the RentCredit
runtime coordinate 7. `expected_key` is `market.rent_beneficiary`.

**The route named another market's RentCredit.** Read off chain:

| | |
| --- | --- |
| the route's coordinate 53 | `CE3PC9fYBmsZSngbQYaZvnsBnXuF6gjZWWeJutp9tpnV` |
| its `DCLRNTL2` body | market `8MSesLUsQ4VMJ9WCHCBWADhnksBp8gwWmtZi7g3GiCky`, generation **1**, bump 255 |
| `8MSesLUs…` | a live `DCLTCOR3` Market account, 368 bytes — the PREVIOUS General market |
| `65Yq3q6t…`'s own `rent_beneficiary` (`STATE_RENT_BENEFICIARY_OFFSET` 296) | **`6FGsfzP7iLwcf7ZbffX65rv9jLxSSDy5ZrUZbXaD976k`** |
| that account | `DCLRNTL2`, 128 bytes, generation **2**, bump 255, Rent-owned, 13,192,766 lamports |

Both PDAs reproduce from `[b"dclutch/rent-market/v2", market, generation_le]`
under the Rent program, so the account supplied is exactly the credit of the
market and generation the route is NOT for. `lifecycle.rs:1438-1440` would have
refused the same account twice more, for its market and for its generation.

**The author is the driver, not the program.** `devnet-general-session` took
`--rent-credit` as a required address and copied it into the route without
joining it to anything, and the `openbatch-refounded` runbook row filled it from
a cohort variable. That is why §4's frame report could say `walls: []` while
carrying an account no execution can accept.

## WALL 2, which only appears once wall 1 is fixed

Replace that one lookup-table entry with `6FGsfzP7…` and the same frame walks
65,000 units further, through `request-lifecycle-preplan`,
`candidate-transcript`, `cx-context-digest`, `cx-frame-validated`,
`cx-request-built` and `cx-cpi-buffers`, and refuses `0x4001`
`TradingSbfError::Release` at **319,075 CU**, still with no accelerator invoke:

    dclutch-hot-why:caller-authority chunk/expected/seen/request-digest
    0x0, 0x52972c42bfbc535b, 0x03919e5d67f15c8f, 0x2e01df8148ce3aff

`programs/dclutch-trading-sbf/src/admitted_composition_v3.rs:749`. The seen
address is the route's own `AedW15zt…`. The third word is the preimage, and it
settles §3:

| | |
| --- | --- |
| what the chain seeds the span with | `family_request_digest_v3(request)` = `ff3ace4881df012e…` |
| what the plan published and the driver used | `sha256(request)` = `6ac8255569a90867…` |

`family_request_digest_v3`
(`crates/dclutch-market/src/execution_strategy/shadow_digest_v3.rs:384`) is
`sha256("dclutch:shadow-family-request:v3" ‖ 0x00 ‖ len_le_u32 ‖ request)`.
`crates/dclutch-operator/src/general_hot_v3.rs` published the bare SHA-256 and
`general_successor.rs` re-joined against the same bare value, so the two agreed
with each other and with nothing on chain. **§3's conclusion — "the probe
default is a real latent defect and it is NOT the wall in front of OpenBatch" —
is right about the ordering and wrong about the cause: the real digest was
never the real digest.** All four addresses moved between passes and the
refusal did not, because execution never reached them.

## What is repaired, and what COHORT-16F can do

Both walls are the host's. **The deployed Trading `e7f8e476…` at candidate
`87eec1c3a` is unchanged and correct**; nothing on the program side of either
wall moves. So COHORT-16F can drive OpenBatch against the release this cohort
deployed, with no redeploy, once it runs the repaired driver:

- the Market's `rent_beneficiary` is the RentCredit's one author, `--rent-credit`
  is an optional cross-check that refuses when it disagrees, and the runbook row
  no longer passes one;
- the operator publishes `family_request_digest_v3`, so
  `--parent-request-digest` names the address span execution derives;
- the row is two passes, because the digest does not exist until a plan does.

Wall 2 has not been shown green on any chain, and no third wall past
`cx-cpi-buffers` has been ruled out: the replay refuses there, so everything
after it is unmeasured. The batch-window state `CuW1791cAY…` is still absent.

---

# ADDENDUM, 2026-09-06, lane COHORT-16F: both host walls fall, the accelerator runs, and the third wall is a name collision

Devnet evidence, and one level below the strongest: the accelerator's execution
below is an **RPC simulation against the deployed programs at finalized state**,
not a committed transaction. Nothing landed and no fee was paid — the driver
sends through preflight and devnet's own preflight refused the send, which is
the correct behaviour and is why this reads as simulation.

    driver     /Users/ember/jobs/dclutch-cohort161-20260905/bin/dclutch-local-successor-bootstrap
               aa4e6de10ae85bb9042e82982c65c2d70a1e73f16bef39063cb07d030e43e977
               built at 4fba7a8afce86d59d94bfc881448e706d2ef8e80, detached worktree, debug
               the 432d07339 binary and its provenance are in backups/bin/
    market     65Yq3q6tgHArrJtZPhf5RAgNzpPAGcZSoBmT4D3As9n5, root 4NcnH7pt…, release set f533be49…
    no redeploy. The deployed Trading e7f8e476… is the one this ran against.

## THE TWO HOST WALLS ARE GREEN, AND EACH HAS A POSITIVE CONTROL

**Wall 1.** The repaired session refuses a disagreeing `--rent-credit` by name.
Fed the previous General market's credit `CE3PC9fY…` it answers
`route/rent-credit-not-the-markets`, naming `6FGsfzP7…` — read live from the
Market at `STATE_RENT_BENEFICIARY_OFFSET` 296 — as the one author. Dropped, the
route carries `6FGsfzP7…` and execution walks past `lifecycle.rs:1421`. The
cross-check fires **only on the `--emit-route` path**: the same flag with no
route emits a frame report and says nothing, which is right (the route is where
the address is consumed) and worth knowing before someone reads a silent run as
a pass.

**Wall 2.** The plan now publishes
`familyRequestDigest = ff3ace4881df012e3b10e48cdb6a72ce119e184003561754d3cb9869f6c95856`
— exactly PROGRAMS-17D's `ff3ace4881df012e…`, measured offline, now reproduced
by the host against the chain. The bare `6ac82555…` is gone. Passed back as
`--parent-request-digest`, the caller-authority span moves off the probe:

    probe (5bc43870…)  DG7TaMXT…  6sttxgpp…  9C9t1V5Y…  4qZ4Nb4y…
    real  (ff3ace48…)  79WAB8Qc…  EEWR5CVK…  Gdcr37oq…  GGZzXYdC…

and `parentRequestDigestIsProbe` reads false. The digest is **invariant to the
lookup table and to the parent digest passed in** (measured: cohort-16.1's own
plans published the same `6ac82555…` across two tables and two passes), so pass
one's answer is pass two's input, which is what makes the loop terminate.

## THE TWO-PASS ROW NEEDS A THIRD EMISSION, AND THE RUNBOOK DOES NOT SAY SO

`openbatch-two-pass` orders the lookup table AFTER the second session, and that
cannot work: `general-successor-plan-v5` refuses `General v0 compilation:
LookupTable` against `{market.lookup_table}` (the founding's frozen
`DfLj84Cz…`), and the four caller-authority PDAs **are in the table's canonical
set** — measured, `devnet-general-lookup-table-v1` dry-run over the probe route
lists all four of `DG7TaMXT…` and the market's own `6FGsfzP7…` among its 53
addresses. So each digest needs its own table, and each table needs the route to
be re-emitted onto it. What actually runs is five acts, not four:

    session(probe) -> TABLE 1 -> session(probe, T1) -> plan -> digest
    session(digest) -> TABLE 2 -> session(digest, T2) -> plan -> execute

    T1  5jtjo4yb55JR5Ufp72uFTM1vH6p1opHgWjUoqosYbQTV  (53 addresses, frozen)
    T2  73NjFXo4c836tWczvc11v4J8nMpjopVraQT8vCmbNEPm  (53 addresses, frozen)

## THE ACCELERATOR RAN. `6v1c2Go2…`, FOR THE FIRST TIME ON ANY CHAIN

    Program ESQhDyV7obS4oNp7abjn7sSYChxtGrHru4TzvPuybJi3 invoke [1]
    Program 6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y invoke [2]
    Program log: general: config/bank generation (config, bank)
    Program log: 0x2, 0x2, 0x0, 0x0, 0x0
    Program log: general: config/bank semantic basis (config, bank)
    Program data: 7Xv8dfXdhsxAhuziX28VxHq/3OlRUWXxj+yZ3XRN3DI= SKZu7romHw5bbf5oWKIEQxDZyIneL8O8jpfMQmnrQPc=
    Program log: general: refused, config rejects the bank's generation or basis
    Program 6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y consumed 30771 of 1070848 compute units
    Program 6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y success
    Program ESQhDyV7obS4oNp7abjn7sSYChxtGrHru4TzvPuybJi3 consumed 361230 of 1399700 compute units
    Program ESQhDyV7obS4oNp7abjn7sSYChxtGrHru4TzvPuybJi3 failed: custom program error: 0x4004

**361,538 CU** for the transaction, against 239,473 at wall 1 and 319,075 at
wall 2 — the route now reaches and crosses the CPI boundary. The harness's
654,000–677,000 CU estimate for a completed OpenBatch is still unreached and
therefore still unrefuted; 361,538 is a refusal's cost, not an action's.

## WALL 3, AND IT IS NOT A COARSE CODE — THE PROGRAM PRINTED BOTH SIDES

    code      TradingSbfError::Transition 0x4004 (Custom(16388))
    site      programs/dclutch-trading-sbf/src/admitted_composition_v3.rs:370
              `if ack.disposition() != AcceleratorDispositionV2::Accepted`
    phase     POST-CPI, first admitted invocation, on the accelerator's own receipt
              (return data DCLTAAK2, disposition byte 0x02)
    cause     GeneralAcceleratorSemanticErrorV3::ConfigMarket, raised at
              programs/dclutch-accelerator-sbf/src/general.rs:1705 inside
              `authenticated_general_domain`, from
              `config.require_market(environment.generation, environment.semantic_basis_id)`

`require_market` is two conjuncts and only one failed:

    generation      config 2, bank 2                       AGREE
    semantic basis  config ed7bfc75f5dd86cc4086ece25f6f15c47abfdce9515165f18fec99dd744ddc32
                    bank   48a66eeeba261f0e5b6dfe6858a2044310d9c889de2fc3bc8e97cc4269eb40f7   DISAGREE

No replay was needed to localize this and none was run. The accelerator's own
`sol_log_data` prints both sides — the reader PROGRAMS-17D's `hot_cu_watch_*`
work is the same instinct, already shipped inside this program — so the
instrument would have re-derived what the chain had already said.

## THE TWO VALUES ARE THIS MARKET'S OWN, AND THE COLLISION IS THE WORD "BASIS"

Read live off `65Yq3q6t…`'s portfolio record
`2WPUBZAHSPk5SUJSBoQQCNLfZTF8hVKYA53AZMnv5Wd1` (`DCLTPRF2`, 240 bytes,
Registry-owned):

    @96   PORTFOLIO_CLAIM_BASIS_ID_OFFSET       48a66eee…   the bank's value
    @128  PORTFOLIO_LIABILITY_BASIS_ID_OFFSET   ed7bfc75…   the config's value

Both are the market's own, from its own founding, one field apart. The two
authors:

- **the bank register** is projected by
  `crates/dclutch-trading/src/general/account_rules_v3.rs:1508-1512` —
  `ProjectDataIdentity` from the portfolio at `PORTFOLIO_CLAIM_BASIS_ID_OFFSET`
  into `identity::SEMANTIC_BASIS_ID`;
- **the config field** is `semantic_basis_identity_v3(linked_basis_hex)`, from
  `tools/local-validator/bootstrap/successor/src/general_market.rs:239` through
  `general_selected_release_v1.rs:1143` into `GeneralConfigInputV3.claim_basis_id`.

**Everywhere else in this tree, "semantic basis" is the LIABILITY basis.**
`crates/dclutch-product/src/payoff/registry_v3.rs:416-432` computes
`semantic_basis_id` from the linked-basis record and requires
`domain.liability_basis_id() == semantic_basis_id`;
`tools/local-validator/bootstrap/successor/src/market.rs:2428-2434` states it as
a comment and checks it; `market.rs:10367` fills
`ClaimsFoundingRequestInputV5.semantic_basis_id` from `liability_basis_id`. The
account rule is the single site that reads the CLAIM basis into a register named
`SEMANTIC_BASIS_ID`, and its own comment says why it chose the portfolio (a
config-sourced register would be compared against the account it was read from)
without saying why it chose that field: it followed the config field's NAME,
`claim_basis_id`, which is the misnomer.

**Which side moves decides whether this market survives.** The account rule is
in `dclutch-trading`, which is compiled into the Trading SBF link, so moving it
is a program change and a cohort-17 link — but it is the side that costs no
re-founding: `PORTFOLIO_LIABILITY_BASIS_ID_OFFSET` already holds `ed7bfc75…` on
this live portfolio, exactly the config's value, so `65Yq3q6t…` would satisfy
the conjunct as it stands. Moving the host derivation instead requires a fresh
~105-transaction General founding, because the config is hashed into
`environment.general_config_id` and pinned by the root. **No program change was
made here**; the owner decides, and the measurement above is what they need.

This is the two-authorities pattern the tree already has a name for, in its
purest form: every General fixture wrote `SEMANTIC_BASIS_ID` by hand (the rule's
own comment records "config `0x56` ×32, bank zero" on 2026-09-01), so the first
time the two sides were ever produced by a real founding was this transaction.

## WHAT DID NOT RUN, AND WHY

`close-batch` and `second-open-batch` are untouched: both need a Batch account
this refusal did not create. `CuW1791cAY1uqgtu1okAjEmY2iVHx8gsytQgs8WcayYQ` is
still ABSENT. The accelerator route is still **not corroborated** —
`docs/evidence/witnesses/cohort-16-1-discovered.json` names
`6v1c2Go2…` in its program roster and in no record, for the third cohort
running, and `--discover` was not re-run because no new signature exists for it
to read.

## LEDGER

    payer  1.384214644 -> 1.364363844   (0.019850800 SOL)
           two frozen GENERAL-HOT routing tables at 0.009550400 rent each
           (permanent), ten transaction fees at 75,000 lamports.
           NOTHING was spent on the OpenBatch itself: preflight refused the send.
    deployer 28.657732010, unchanged. No top-up.
