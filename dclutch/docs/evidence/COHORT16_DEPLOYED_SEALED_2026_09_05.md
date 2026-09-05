# Cohort-16 on devnet: the eight, the eighth for the first time, and what the run found

Status: **devnet execution evidence.** Owner: lane COHORT-16. Written
2026-09-05 at `/Users/ember/dev/dclutch` (the live tree). The job directory is
`~/jobs/dclutch-cohort16-20260905/` and it is self-contained: the driver binary,
its digest and its provenance, the checked candidate's whole evidence chain, and
every stage log and report live there. Every number below was read back off
devnet at finalized commitment or off an artifact in that directory; nothing is
relayed from a prior document.

Deploy commit `f2ae6bf75deb1f71465e1ff06a05abe80d4d22a4`. Manifest
`tools/cohort/cohorts/16.json`. Prior cohort 15.

## 0. What is new about this cohort

**The eighth link had never been deployed to any chain.** The simplification
swarm folded `dclutch-general-accelerator-sbf`, `dclutch-dealer-accelerator-sbf`
and `dclutch-series-shadow-sbf` into one `dclutch-accelerator-sbf`, and
C-05's `OpenBatch`, C-06's first Dealer market and C-07's whole Series family
all became dependent on a program nothing had put on a chain. It is on devnet
now, at `6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y`, deployment slot
493,639,473.

**Every devnet witness in the register attested to binaries the tree could not
produce.** Cohort-15 deployed from `1cae26fd6`, before the swarm base; the
convergence's own ELF table says no link is byte-identical and that cohort-16
carries every redeploy. It does.

**The runbook had no row that deploys the accelerator, and nothing said so.**
Through cohort-15 a separate one-off job deployed it and `prepare` merely
OBSERVED it. That was invisible while the accelerator was unchanged for three
cohorts. §4.

## 1. The candidate

Built on **hbox through `swarm-build`** — the named release builder artifact,
platform-tools v1.53 on Linux/x86_64 — from a clean `git archive` of the deploy
commit. A native macOS build of the same commit differs in nine of ten roles and
is refused, so the laptop drove the cohort and never built its release.

| line | value |
| --- | --- |
| `release_builder` | `true`, `release_builder_artifact_host=Linux/x86_64`, `builder_scheduler=swarm-build` |
| `source_revision` | `f2ae6bf75deb1f71465e1ff06a05abe80d4d22a4` |
| `source_digest` | `d408556a67b22ab2f9121446ed8e2c437ce47dc43af26add94de638d27f40f62` |
| `sbf_build_diagnostics_total` | **0**, and 0 on each of the eight links by name |
| `cargo_lock_immutability` | `passed` |
| `spline_product_handoff` | `passed` |
| `reproducible_release_gate_sha256` | `e3bf68529e633a40dcd6954bdfd0565ef37794df2229fa361844eb2287d01433` |

**The reproduction control.** The same commit was built twice on the named
builder into two different absolute `--work` roots — once directly and once
through the runbook's own emitted `02-redeploy-named-builder.sh`. All eight ELFs
are byte-identical across the pair and the reproducible release gate digest is
identical; `checked_upgrade_gate_sha256` differs because it carries a per-run
build id. That is the build-path control the row's verifier asks for, and it is
not a cross-host reproduction, which the row is explicit about.

### The ELF table, against the convergence's §6

`SIMPLIFICATION_CONVERGENCE_2026_09_04.md:152-175` published the converged
eight at `018ea525f`, built on the **laptop**. Cohort-16's are the **named
builder's** at `f2ae6bf75`, 200-odd commits later. Both facts move the bytes, so
none of the eight is equal to that table and the delta is stated rather than
implied.

| link | convergence §6 (laptop, `018ea525f`) | cohort-16 (hbox, `f2ae6bf75`) | bytes |
| --- | --- | --- | ---: |
| accelerator | `a8cb8c8ab35f…` | `587181d9536d19f4ed40ddebb01ebac1d3e3544d28110c3d7e1ca04b4e4c87ab` | 736,056 |
| claims | `38196b22d71d…` | `33e453e62186e3426d9ede8d090afd95fed34af2ff1ec1a685daa0e74347675d` | 1,421,856 |
| core | `236e92ae9345…` | `f637e5df9ef9a16521d58aee080b951f6950f2661abbf2ff573cf7bc98d5665e` | 1,186,440 |
| custody | `fd3b123a938e…` | `2600db72b383fc750bc8a975dfe19233b78012a4d53c21c680089bd42bcf7410` | 431,296 |
| registry | `98a1b45308d0…` | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` | 237,696 |
| rent | `2f145149ae57…` | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` | 143,152 |
| resolution | `f8bfca35ab8e…` | `7be8a398be52342546a953cccc04b7276411041eca0081990d1e43be5ed7c34b` | 846,656 |
| trading | `46757cedce83…` | `69292c3391924d628f574a17397460c805cabcf8e52d041dc6a588b7c59e88c7` | 2,179,816 |

**An incidental control worth keeping.** The first candidate ran at
`e311d29367f68eded5b2d0150a0d17dacb337174` and the shipped one at `f2ae6bf75`,
eleven commits later. **Seven of the eight links are byte-identical across that
pair** and only `claims` moved — 1,421,800 → 1,421,856 bytes, `e20643ec…` →
`33e453e6…` — which is exactly where the window's source moved
(`programs/dclutch-claims-sbf/src/rational_lifecycle_v2.rs`, and a lock change
that removed 89 packages). A window of eleven commits touching a program moved
one link and no other.

## 2. The ledger, to the lamport

Devnet moved its rent rate from **6,333 to 5,080** lamports per byte at the
epoch-1141 boundary, mid-cohort-15. Everything below is priced at 5,080, read
off the cluster (`solana rent 0` is 0.00065024 SOL, which is 128 x 5,080), and
that is why `steps.tsv`'s `-42.26` is cohort-15's price and not today's.

| act | SOL | running |
| --- | ---: | ---: |
| deployer before anything (finalized slot 493,627,133) | | **23.890559434** |
| `close-prior`: seven ProgramData closes | **+42.505848399** | |
| seven 5,000-lamport fees | −0.000035 | **66.396372833** |
| `close-prior-accelerator`: the superseded `8pgnyNvg…` | **+1.915282857** | |
| one fee | −0.000005 | **68.311650690** |
| eight deploys, rent | **−36.496508160** | |
| eight deploys, fees | −0.042329960 | **31.772812570** |
| `fund-payer`: the campaign payer, a distinct key | −2.000005 | **29.772807570** |

The projection is not a rounding: **every one of the eight ProgramData balances
equals `(128 + 45 + elf_bytes) × 5,080` exactly**, and every ProgramData
`Data Length` equals its candidate ELF's byte length exactly.

| role | program id | ProgramData | slot | bytes | rent SOL |
| --- | --- | --- | ---: | ---: | ---: |
| registry | `6gRRiB9BtQFN6AquyLXXjuiX1GYN2xyW8nqCTc3xJzkV` | `68Jh5pD42XWmYq5ViWoX3MKHMeENCRbgdxdGb8B7UY6k` | 493,638,685 | 237,696 | 1.20837452 |
| rent | `42xN9ULoMpULmeDbdGCtyAo82FRJved6sojUun6NSKdt` | `8KG9NGFoMRCh4dngeAGNkP7kCmtQ68KthSbk8V883x5v` | 493,638,731 | 143,152 | 0.72809100 |
| custody | `8UkoNCPD4JuWBiHWdc7WaM3j7Fj9jbf8Fe926Q1CDceo` | `AjYb8Ss7E3ruHppSCDcqxJLErwGhHikTcHQymKZu6BG1` | 493,638,796 | 431,296 | 2.19186252 |
| resolution | `jrjXw2Rph15VyJB3ztbRgoHUPJrcvMSHV6svRUYtUw3` | `PpzTFUiPbyj4MKbLoUzCxh4cAeLrZ52PBdvN8byxR1n` | 493,638,882 | 846,656 | 4.30189132 |
| claims | `8JfHfBBGaoUP1yV6VzXcvWwhQSZNV8eQmDAiYmCpNQJk` | `14EYxVmGJuSKX9iizPaLQQRj8ae3XiJJqWHdnAnCcv33` | 493,639,017 | 1,421,856 | 7.22390732 |
| trading | `ESQhDyV7obS4oNp7abjn7sSYChxtGrHru4TzvPuybJi3` | `7RxAyfAUd3hEENzog4Faq4tqpzFfA6riM1jnYVLEgSwx` | 493,639,190 | 2,179,816 | 11.07434412 |
| core | `4wv7JxoAad6JMQi2vHJyByLXasWS8RzJSTdvEEmpCjpe` | `BbyZZAwbz37VwLR6zMQMm2bJAhfqbJVFAxr9HbFRQ5AU` | 493,639,301 | 1,186,440 | 6.02799404 |
| **accelerator** | `6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y` | `DfJLGB1W12cUYGpw3doG2DmMDe6ubR2UkmrrUsqosa9g` | 493,639,473 | 736,056 | 3.74004332 |

Each live image was **dumped back and compared to its candidate ELF over the
ELF's whole length before the next role spent**, which is what `deploy-roles`
means by stopping a sequence at its first failure rather than after it.

## 3. The ladder, the seal, and what they cost

`administration` (`campaign --through activation`) ran **36 transactions**, every
one `error: null`, at 75,000 lamports each. The verifier is not the exit code: a
second preflight that READS THE CLUSTER reports **substrate, publication,
initialize, succession and activation all `complete`**, and the accelerator's
`ArtifactRelease` record is among the records the ladder finalized -- which is
the cohort's own observation of the accelerator's deployment, and the thing a
General-manifest market needs before it can be founded.

`seal` is key-free and read-only and moved **0 SOL**, before any founding, which
is the ordering cohort-12 got wrong and stranded a market on. All five owned
roles preflight `equal: true` against a fresh finalized observation --
`checked_candidate_elf_sha256` equal to `live_elf_sha256` for custody,
resolution, claims, trading and core -- and `plan-seal.json` carries
`checked_upgrade_set_final_sha256`
`8a5c8a29d780017f470053c67c8200012c9e33dcafc65b964345151cec7025fc` over the same
release set id `prepare` produced,
`85defd75b236b191de00b48e673cdc4a4bcc2408b2248c4504895815b04cc69f`.

## 4. What running the runbook found, and it is four things

Every one of these is a defect that only a RUN could produce: each is a row that
reads correctly and refuses when executed. All four are repaired in this window
and the repairs are commits, not edits to a job directory.

**(a) No row deployed the accelerator, and the accelerator had changed.**
`prepare` has always OBSERVED an accelerator and published its `ArtifactRelease`;
through cohort-15 a separate one-off job deployed it, and the runbook carried no
row for that. Invisible for three cohorts because the artifact did not move. The
fold moved it, renamed it, and made three families depend on it, so the cohort
that first ships the fold has to be the cohort that deploys it. `deploy-accelerator`
is that row. It is NOT an eighth `roles` entry, because the deployment-set
journal owns exactly the seven checked roles and names no accelerator -- the
driver's own `prepare` usage says so, and `roles` would emit an
`--accelerator-program-id` flag that does not exist.

**(b) A literal count in `checked-release-candidate.sh` refused every candidate
at HEAD, for the second time.** `SHIPPED_LINK_COUNT=12` against a shipped set of
8. The file's own comment predicted this: the POPULATION lane hit the identical
defect on 2026-09-02 when the set went 13 to 12, named the honest repair, and
deferred it. Behind the count sat a second defect it was hiding -- the shipped
set is compared POSITIONALLY, and the fold's rename of
`dclutch-general-accelerator-sbf` to `dclutch-accelerator-sbf` moved the package
to the front of the `programs/*` enumeration while its `SHIPPED_LINKS` entry
stayed where the deleted dealer accelerator had been. The count agreed and the
order did not, which is exactly what counting cannot catch.

**(c) The campaign release pack named a `Cargo.lock` the one-workspace fold
deleted.** The candidate passed every gate -- eight links, zero frame
diagnostics, both gates emitted -- and then refused at the pack for a missing
`tools/local-validator/bootstrap/successor/Cargo.lock`. Three call sites named
it; the lock that resolves that producer's dependencies is now the root lock.

**(d) Two emitted-stage defects, and both reported the wrong thing.** The
`programdata` helper called `solana program show`, which demands a default
signer to perform a read and refuses in a job directory that deliberately holds
no key -- inside a command substitution, so the address came back empty and the
NEXT command failed naming a missing argument. And `prepare`'s row omitted
`--{role}-semantic-release-id` entirely while its own command text said the ids
are derived from the ELFs; cohort-15 supplied them from a job-directory script
that had no home in the tree. Both repaired, and the derivation now lives at
`tools/cohort/semantic-release-ids.py`.

**(e) The `seal` row named a producer that does not exist.**
`devnet-deployment-set-journal-v2 --init` is not a mode any driver implements.
What ran for cohort-14 and cohort-15 was a hand-written python script inside
each job directory. `tools/cohort/deployment-set-journal.py` is that producer
with its paths taken as arguments. **This is the producer-missing pattern inside
the runbook itself**: the row was written from the intended shape rather than
from what ran, and nothing in the tree contradicted it because the thing that
ran was not in the tree.

## 5. The refunding basis, and the wall the walk still has

**`refund-scale` landed as a founding change, not a program change.** Decision
0025 is explicit that the payout arm needs a founded RECORD and not a founded
program: Core founding derives `basis_scale` from the Product
(`generic_founding_v1.rs:1104`) and Claims binds the permit's scale to the
request's, so a market founded on a refunding record funds its Hoard at
`quantity x ordinary_region_count` with no founding *code* change at all. What
was missing was the record. `compile_linked_basis_v3` hard-wired
`payout_scale: 1` -- the shape that paid cohort-13's founder the whole failure
column while the two strangers who traded got nothing -- and it now DERIVES the
scale from the width: `basis_width - 1` whenever the width can carry the
distinction, the legacy `1` below `CATEGORICAL_REFUND_MINIMUM_WIDTH_V3` where
the two numbers are equal and the record could not say which shape it is. The
byte-identity test that asserted the literal `1` was a second author for a rule
`categorical_refunds_on_failure_v3` owns; it now decodes the compiled record and
checks that `refunds_on_failure` is true of it.

**The retirement walk's wall is a founding input and it is still standing.**
`docs/HANDOFF_2026_09_05.md:114` says the capability manifest's dependency edges
were *"fixed for cohort-16"*. **That is false at HEAD, and it is false in the
direction that matters.** The founding compiler builds every manifest entry with
`dependency_count` 0 and an all-zero dependency array --
`tools/local-validator/bootstrap/successor/src/market.rs:14678-14680`, inside
the four-entry manifest a Direct market founds under (three Resolution
compartments plus the selected Direct entry, `selected_capability.rs`'s
`validate_selected_manifest_v1` requires exactly four). `git log -S
'dependency_count'` on that file returns one commit and it is not in this window.

So `capability_dependency_closure_mask_v1(manifest, selected)` still returns a
SINGLETON for the Direct entry, and stage four of retirement --
`DirectCloseCapability`, the packet that takes `outstanding_capabilities` to
zero -- still refuses at
`crates/dclutch-operator/src/terminal_retirement_v1.rs:699` with
`CapabilityFundingHeaderV2::new(physical_count 2, logical_count 1)`, because a
header counts physical ledgers whose disjoint subsets COVER the logical entries
and a singleton closure cannot cover the two Resolution compartments the close
frame preserves.

**This lane did not guess the edge set.** The manifest entry is a seed of the
Market PDA, so a wrong dependency array does not found the same market
misconfigured -- it founds a DIFFERENT market. Naming which of the three
Resolution entries the Direct entry depends on is a decision with a hostile test
attached, and it is the next lane's, not a 3am inference with a deploy half
done. It is bounded and it is named: the two prepaid companions that `CloseFund`
refunds are the candidates, the exhaustion and recovery entries; the
source-material entry is the one the funded deadline walk consumes.

**What that costs.** Cohort-16's Direct market can be filled, settled, captured,
paid out and taken to Terminal; it cannot be Retired. A re-founding on this same
cohort costs 0.34 SOL once the edges land, and devnet markets are disposable, so
the cohort is not spent by this -- but "one market Open to Retired" is still
open, for the same reason it was open yesterday, and one artifact in the tree
said otherwise.

## 6. What a reviewer should distrust in this file

- **The reproduction is same-host, and the row says so.** Two absolute `--work`
  roots on the named builder is the BUILD-PATH control; it proves the path is
  not an input and proves nothing about a second machine. `supported_builders`
  narrowed to one named artifact on 2026-09-04 and a second host running it was
  not part of this cohort.
- **Six of the eight ELF digests here have never been compared to a second
  host's**, and the two that have (`persvati` vs hbox, 2026-09-04) were at a
  different commit.
- **The chain reads are finalized and narrow.** A ProgramData balance equalling
  a formula says the CLI allocated what the rate implies; it does not say the
  program does what the ELF says. The `cmp` after each deploy is the byte
  claim, and it covers the ELF's length and not the account's padding.
- **`prepare`'s semantic release ids are an operator statement**, re-derived and
  refused on mismatch for the five artifact-derived roles and compared against
  constants for two. **The accelerator's eighth is checked for nonzero and
  non-collision and nothing else** -- no protocol-owned derivation exists for a
  role outside the seven, which is debt this cohort inherits and does not close.
- **The runbook's own `--prove-frozen` is the only thing standing between the
  five rows added here and a claim that cohort-14 and cohort-15 ran differently
  than they did.** It is green: 19 rows and 6 rows byte-identical.
- **Two rows of this cohort are recorded green by the operator and not by a
  script**: `record-core-digest` and `refund-scale` are source commits whose
  emitted scripts exit 64 saying so, and their GREEN markers were written after
  checking the commits by hand. Each marker carries the commit beside it.

## 7. The founding refused, and the refusal is decision 0025's own rounding boundary

`found-direct` staged its `MarketRunInput`, re-minted the sponsored Pyth release
against finalized slot 493,646,104 (two chain-owned facts moved and are pinned
at their OBSERVED values, which is the design), and drove **140 founding
transactions** including Found37's hostile controls -- *"refuses substituted
lifecycle credit"*, *"refuses a substituted Market coordinate and rolls the
transaction back"* -- and then stopped:

    Error: Error("founding collateral reserve is not exactly divisible by basis scale")

**This is the guard decision 0025 designed, firing for the first time on a real
founding.** `founding_quantity_v1`
(`tools/local-validator/bootstrap/successor/src/market.rs:6137-6152`) takes the
lower half of the minted collateral as the founding budget and requires it to be
an exact multiple of the basis scale, *"accepting another floor here would create
a second rounding boundary and an unclassified remainder"* -- which is 0025 §6's
answer in as many words: **there is no remainder, because a remainder is refused
at founding rather than housed.** At the legacy scale `1` every budget divides.
At the refunding scale `3` -- this market is four outcomes wide, three ordinary
regions and the explicit failure coordinate -- the fixture's collateral atom
count does not.

**The refusal is correct and the repair is a founding INPUT, not a weakening.**
The reserve has to be chosen as a multiple of the scale the width derives; the
guard must not learn a floor. What this cohort did not have was a producer that
picks the reserve from the scale rather than from a constant that happened to
divide by one. That is the next lane's, it is small, and it has a hostile test
attached: a reserve one atom off must still refuse.

**What is on chain from the attempt.** The realm record, the Product graph, the
capability manifest and the Found31 market coordinate
`4Pm8Mfg2P1sVGwrpQ7uC1GQh2TND5DqyXU5cDsuaGEXq` were published and the campaign's
own journal in `market/campaign-open.json` names every target it derived,
including the Open Market coordinate `A9u1SFn5Xkqp2UVdADJk3SaeRwVzmNrFU7yQAs1Ut6Ra`
it did not reach. The campaign payer went 2 SOL to 1.877 across the attempt. No
market of cohort-16's reached `Open`; the General and two-source markets were
not attempted.

**So `refund-scale` is half a row.** The record side is authored and the ELF
side needs nothing, exactly as 0025 says -- and the founding cannot produce a
reserve the record admits. The row's verifier reads a founded market's
authenticated basis off chain, and there is no founded market to read.

## 8. Where the cohort stands

Deployed, sealed, and unfounded. Everything a market needs is on the chain and
current: eight programs from one commit and one named builder, a finalized
release set, an accelerator whose `ArtifactRelease` the Registry finalized, a
sealed plan, and a campaign payer with money in it. The runbook is five rows
better than it was this morning and each of those rows was written by a refusal.

The three things the next lane needs, in the order they block each other:

1. **A founding collateral reserve that is a multiple of the derived basis
   scale.** Nothing else can be attempted until a market exists.
2. **The capability manifest's dependency edges** (§5). Without them a market
   founded on this cohort can reach Terminal and cannot be Retired.
3. **`tools/gate witness --discover` against this cohort**, which is what
   re-anchors the register's 42 devnet witnesses to binaries the tree produces.
   The cohort exists now; the discovery pass does not.

---

## Addendum, 2026-09-05, lane COHORT-16B: the first market is founded, and blocker 2 is not a founding input

**Devnet execution evidence.** Written after §8's blockers 1 and 2 were
attempted. §8 stands as written; two of its three items now read differently and
this addendum says how. Nothing above is edited.

### The first cohort-16 market is Open

    Open Market        GyD95eyERwRfwj8fSFNhWjKF2eaDg5XcREidPKex65zY
    Found31 Market     82na8PkLLvcMq9R5kFquForANHRmv7t8G14Lp4TwUmag
    collateral mint    CAjMoVjvZmgToqVFK63iDDhvWBdDvHGi9cuyCPqwJA6j
    realm record       3asCuLzysFGxv1qnn79tpPhUYgrwTDwPYHrEN1wyeGYW
    manifest record    iTNLkxiStkthYLruRCYCxcJJ7XMy4Gw6FSAxs1RJyCa
    basis record       GbQGoztaDGN5uAnU41be8WDzLQX9vkhEHM6UxyUSNHLS
    Trading ledger     GJwPzPdz5ppCD8sz3ymaZvcabsmeBSKNy5f7GFX2mqeh   mask 0x0001
    Resolution ledger  DtvxF2xgFvnNuCgn7uDuKcErWfVHvJatvusm9ZDRShrd   mask 0x000e
    driver             d76b13158 (the 582e46531 build cannot found this cohort)

Read back off devnet at finalized commitment:

- The Open Market's `selected_release_set` at offset 208 is
  `85defd75b236b191de00b48e673cdc4a4bcc2408b2248c4504895815b04cc69f`, equal to
  the sealed plan's `release_set_id`. That is `found-direct`'s own verifier,
  read off the chain and never pasted from `prepare`.
- The authenticated `ProductBasisV3` carries kind `1`
  (`BASIS_CATEGORICAL_KIND_V3`), `basis_width 4` and `payout_scale 3` at
  `BASIS_PAYOUT_SCALE_OFFSET_V3`, so `categorical_refunds_on_failure_v3` is true
  of it. **`refund-scale` is green on a founded market**: an oracle outage on
  this market refunds every ordinary claim.
- `initial_collateral_atoms` is **1,000,000,002**, not the 1,000,000,000 every
  prior cohort typed.

### Blocker 1 is closed, and the reserve is a stated input

The founding compiler derives the reserve against the scale the width derives
and prints one line before anything is authorized:

    founding-reserve-terms-v1: reserve 1000000002 atoms; founding budget
    500000001 atoms (the lower half); basis width 4; derived payout scale 3;
    complete sets 166666667 = budget / scale, exact; reserve provenance derived:
    the smallest reserve at or above the intended 1000000000 atoms whose budget
    this scale divides exactly

The line is rendered *through* `founding_quantity_v1`, the guard itself, so it
cannot be printed for a reserve the founding would refuse. The staging script
refuses to stage a founding that states no such line and quotes it verbatim into
`market-open-staging.json`. Landed at `fe86bac5b`; the guard is untouched.

### The dependency edges are on a chain, and they are the derived set

The manifest record at `iTNLkxiSt…`, owned by the cohort-16 Registry, decodes to:

| entry | `dependency_count` | dependencies | tail zero |
| ---: | ---: | --- | --- |
| **0 (selected Direct)** | **3** | **[1, 2, 3]** | yes |
| 1 (Resolution companion) | 0 | — | yes |
| 2 (Resolution companion) | 0 | — | yes |
| 3 (Resolution companion) | 0 | — | yes |

**The selected entry is at index 0, not 3.** `campaign-open.json` records
`direct_selected_manifest_entry_index: 0` and the two funding ledgers read back
`0x0001` (Trading) and `0x000e` (Resolution). The manifest position is
kind-digest order over the four capability kinds, and this is where the real
market puts it. `terminal_retirement_v1.rs:120-123` names the opposite pair —
`0b0111` preserved, `0b1000` closed — and `:816-822` requires
`entry_index == 3`. That is a four-entry fixture, not this market.

### Blocker 2 is NOT a founding input, and this is the finding

With the edges in place the selected entry's closure is `0b1111`, which is what
stage four's funding header needs. It is also what **every** route that consults
the closure needs, and activation is one of them. Core's
`validate_funding_header` requires `selected_mask == closure` and
`validate_funding_ledger_masks_v2` requires the frame's ledger masks to
partition it, so an activation frame must carry both ledgers. The drivers were
repaired to do exactly that — 36 accounts instead of 35, the slice ordered by
each ledger's lowest selected index — and the preflight planned cleanly.

The execution refused, in the deployed Trading program:

    Program ESQhDyV7obS4oNp7abjn7sSYChxtGrHru4TzvPuybJi3 invoke [2]
    consumed 108180 of 1168949 compute units
    failed: custom program error: 0x4003

`0x4003` is `TradingSbfError::Content` — *"Manifest, selected entry, descriptor,
or config content refused"* (`programs/dclutch-trading-sbf/src/lib.rs:143`), a
coarse code over many conjuncts. The conjunct is named by the activation
bundle's own declaration:

- `crates/dclutch-trading/src/activation_bundle_v1.rs:355` —
  `/// The root being created and the sole selected funding ledger.`
  `const ACTIVATION_ACCOUNT_COUNT: u16 = 2;`
- the profile and effect index their accounts as `ACTIVATION_ROOT_ACCOUNT_V2 = 0`
  and `ACTIVATION_FIRST_FUNDING_ACCOUNT_V2 = 1`
  (`crates/dclutch-market/src/capability_program/mod.rs:147,149`)
- `programs/dclutch-trading-sbf/src/outer.rs:2076-2096` —
  `require_activation_local_effects` computes
  `first_nonfunding = 1 + funding_count` and refuses `Content` for any effect
  write below it.

**A wider funding slice moves the frame under a bundle whose account indices are
published records.** Those records — the activation descriptor, effect and
account profile — are named by the manifest entry, and the manifest digest is a
Market-PDA seed, so they cannot be re-indexed for a market that already exists.

So the two requirements are in conflict at cohort-16's deployed artifacts:
**retirement needs the edges; activation at this release forbids them.** That is
not a founding input and not a driver repair. It is a Direct release change —
`ACTIVATION_ACCOUNT_COUNT` and the bundle's indices must express the dependency
ledger — which moves the Direct release id, the manifest entry and the Market
address, and which changes a crate compiled into the Trading SBF link. This lane
holds no program authority and stopped here.

`GyD95ey…` is therefore a market that is Open, refunding, and carries correct
dependency edges, and that cannot be activated at this release. It is the
positive control for the conflict, not a market to trade.

### One more mirror, caught by a real founding

The founding refused twice before it landed, and the second refusal was a defect
older than this lane:

    DCLTGMF3 resolved account count changed: expected 58, observed 60
    (market does not carry a recovery policy)

Seating the failure escrow at founding (decision 0025 item 2) moved the composed
founding frame from 58 keys to 60 — the escrow's Position and its admission. It
moved `GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3`, the census fixture derived
from it, the census test that pins `60/14/43`, and the live check every real
founding runs. It did not move
`FoundingSubmissionOperationV1::exact_unique_accounts`, which is consulted only
when a real founding compiles its real message — so every offline suite was
green and the frame refused three transactions from Open. A test in the same
table asserted `58` and `60` as literals and was a third author agreeing with
the stale pin. The pin now reads the frame's width; the literals are gone; a new
test welds them. Landed at `248654d9f`. This is the second time that one table
has been a stale mirror, and its own doc comment records the first.

### The cost, and what the payer holds

Three founding attempts. The first refused before any market state existed
(`the founding has STARTED on this chain … no compatible durable DCLTPCB2
checkpoint`) because the refused zero-edge attempt had already consumed the
collateral mint; the collateral mint and wallet keypairs were rotated for each
attempt and the old ones kept. The campaign payer went 1.877 → 1.684 → ~1.3 SOL.
The deployer is untouched at 29.74.

### What §8's list reads as now

1. **Closed.** The reserve is derived, stated and on a chain.
2. **Reshaped.** The edges are derived, on a chain, and correct; the wall moved
   to the Direct activation bundle's two-account frame, and it is a release
   change with program authority attached.
3. **Untouched.** `tools/gate witness --discover` against this cohort has not
   been run.

---

## Addendum, 2026-09-05, lane PROGRAMS-17A: the conflict was not real, and the release identity does not move

**Offline program-test evidence on real ELFs.** Written after the first addendum's
finding was taken to the Trading program. Nothing above is edited; this reverses
one verdict in it and says how it was measured.

### What the first addendum concluded, and what is wrong with it

It concluded: *"It is a Direct release change — `ACTIVATION_ACCOUNT_COUNT` and
the bundle's indices must express the dependency ledger — which moves the Direct
release id, the manifest entry and the Market address."*

**The first half is right and the second half is false.** It is a Trading
release change. It is not a change to the Direct activation bundle, and it moves
no artifact byte, no release id, no manifest entry and no Market address.

**A three-account activation profile cannot be encoded at all.** Measured
2026-09-05 against `encode_account_profile_v1_atomic` with the exact three rules
the first addendum's reading calls for:

    RESULT: Err(UnanchoredAccount)

`AccountProfileV1::validate_structure` requires every self-representative rule to
be named by at least one REQUIREMENT operation
(`crates/dclutch-vm/src/account_profile/mod.rs:744-765`), and the requirement
operations an activation artifact can write compare against the seam-seeded
identity bank — Trading, Core, Registry, and the root
(`activation_registers_v2`). No coordinate there names a foreign controller, so
a rule for a Resolution-owned dependency ledger is unanchored at any width. The
bundle could not have been widened; extending the seam's identity bank to make
it possible would have been a new ABI, and would have moved every founded
market's address for nothing.

### What actually refused, and the repair

`outer.rs::RuntimeFrameV2::new` composed the interpreted runtime frame out of
the root PLUS EVERY PHYSICAL LEDGER. The profile the release publishes declares
two accounts, the frame presented three, and `project_accounts` refused
`Content`. The native-close route on the same page had already answered this,
deliberately and with its reason written down: `new_close` projected through
*"only root, selected Trading ledger, and RentCredit … a foreign dependency can
never acquire a descriptor-owned runtime permission"*, while keeping every
physical ledger for authentication and poststate.

The two constructors are now one. The activation frame is the root and the
selected ledger; dependency ledgers are authenticated exactly as before —
manifest binding, PDA derivation, funded-rent custody, per-row Active status,
and the canonical mask partition — outside the interpreted frame, and their
lamports are checked against their own balances rather than against a projection
that can no longer name them.

### On real ELFs, built on hbox under `swarm-build`

`programs/dclutch-core-sbf/tests/capability_close_alias_program_test.rs`, which
is the only offline harness that reaches `process_activation` through real Core,
Trading and Registry. Its activation fixture is now parameterised by market
shape, and the two-ledger shape is **devnet market `GyD95eyE…`'s**: four
entries, the selected Direct entry at index 0 with edges `[1, 2, 3]`, funded by
`0x0001` (Trading, selected, written) and `0x000e` (Resolution, preserved).

| campaign | result | CU, top level |
| --- | --- | ---: |
| `canonical_activation_creates_the_direct_root_through_real_core_and_trading` (zero-edge, the control) | root created | **298,598** |
| `canonical_activation_admits_the_selected_entrys_two_ledger_closure` | root created, dependency ledger byte-identical and lamports unmoved | **521,780** |
| `canonical_high_selector_closes_through_real_core_and_trading` (`DirectCloseCapability`, `outstanding_capabilities` → 0) | unchanged | — |
| `begin_direct_retiring_m61_twenty_seed_real_sbf_campaign` | 20/20, mean 94,559 | — |

The 223,182-CU delta between the two shapes is the dependency ledger's price:
one more `find_program_address`, one more `authenticate`, and three
manifest-row status reads instead of none. It is paid once, at activation.

**The artifacts did not move.** `dclutch-trading`'s sealed activation tests —
`exact_activation_bundle_inherits_release_and_binds_root_width`,
`the_real_effect_kernel_composes_the_exact_initial_root_tail`,
`the_family_neutral_template_reproduces_this_sealed_bundle_byte_for_byte` — are
green unchanged, and the two-account template's width is now READ from
`activation_account_count_v2(ACTIVATION_RUNTIME_FUNDING_ACCOUNTS_V2)` rather
than written down twice.

### What this means for `GyD95eyE…`

It is Open, refunding, carries correct dependency edges, and **its release id,
manifest entry and Market address are exactly what they were.** A Trading link
built from this commit activates it as it stands. Whether COHORT-16C reaches it
by upgrading cohort-16's Trading in place — which supersedes the release
generation under decision 0012 and needs a re-release before any open market
executes — or by a fresh full-cohort redeploy that re-founds everything, is
COHORT-16C's decision and not this lane's. The fact this lane owns is that
re-founding is no longer *required* by the activation frame.

### The ledger-set refusal now has a name, and it has never fired

`TradingSbfError::ActivationLedgerCount` (`0x402B`) is Trading's own statement
that the physical ledgers presented are the canonical disjoint partition of the
selected entry's dependency closure. **Core owns that partition and refuses
first**: both hostiles — a dependency ledger withheld, and a third ledger
overlapping the dependency's rows — refuse `CoreSbfError::Funding` (`0x3008`)
before the CPI, which
`an_activation_whose_ledgers_are_not_the_closure_refuses_by_name` measures and
names. The Trading code is the restatement a program that does not trust its
caller must make anyway, and it forbids nothing that has ever executed, on the
same terms as `ShadowTrustedEnvironment`.

### The retirement close frame stopped being a four-entry fixture

`crates/dclutch-operator/src/terminal_retirement_v1.rs` required
`entry_count == 4 && entry_index == 3 && required_union == 0b1111`, and named its
two funding coordinates `resolution_funding` and `trading_funding` — a
controller per position. On the real market the selected entry is index **0**
and the order is the mirror image. The gate is gone: only the `F=2` frame width
is fixed, the entry index and the union are the manifest's, and the frame's
writable meta follows `selected_funding_position`, which
`authenticate_close_funding` discovers by reading `selected_mask` off the
ledgers. `the_written_funding_meta_follows_the_selected_position_not_a_controller`
runs both orders through the one projection.

### What is still owed

- **The complete retirement walk has not run in any harness.** Its stages have
  real-ELF coverage one at a time — found, activate, Core and Direct
  begin-retiring, `ResolutionCloseFund`, `DirectCloseCapability`, the replay
  handoff, the four checkpoint instructions to Retired — in five different test
  binaries with five different fixtures, and nothing joins them. Per-stage CU
  therefore exists only where each harness prints it, and this lane adds
  activation's two numbers and nothing else.
- **`terminal_sequence.rs`'s six-stage walker is still fixture-only**, and its
  own note that *"the three stages after `ResolutionCloseFund` have never been
  reached"* stands.

### One link moves, and it is Trading

Built on hbox under `swarm-build`, from a `git archive` of `57e4b9b27` — the
commit before this lane — and from the checked release candidate at
`87eec1c3a`:

| link | `57e4b9b27` | candidate `87eec1c3a` | cohort-16 deployed |
| --- | --- | --- | --- |
| core | `29200c855bfe9376cf813ea38aadd126212667945e86ed10f80637aafeb4d192` | **same** | `f637e5df9ef9…` |
| trading | `69292c3391924d628f574a17397460c805cabcf8e52d041dc6a588b7c59e88c7` | `e7f8e476006ce1248994ae065bffd7ea0039c8681f85fed141368790e021931b` | `69292c339192…` |

Three things are stated by that table and none of them is inferred. **This
lane's change reaches Trading alone**: the two constants it added to
`dclutch-market` are compiled into Core as well, and Core comes out byte for
byte identical, so they emit nothing. **The pre-lane Trading link is what is
deployed on devnet right now** — `69292c339192…` is cohort-16's own row. And
**core's move away from cohort-16's `f637e5df9ef9…` predates this lane**; it
happened between `f2ae6bf75` and `57e4b9b27` and belongs to COHORT-16B.

Accelerator, claims, custody, registry, rent and resolution are byte-identical
to cohort-16's deployed set. A cohort that wants this repair needs the Trading
link; whether it also wants COHORT-16B's Core is that cohort's question.

### The candidate, whole

`tools/release/checked-release-candidate.sh` at `87eec1c3a`, genesis, on hbox
under `swarm-build`, 433s, exit 0.

| line | value |
| --- | --- |
| `release_builder` | `true`, `release_builder_artifact_host=Linux/x86_64` |
| `source_revision` | `87eec1c3a6bf954a4350931af62ce8d4fcc48da2` |
| `source_digest` | `408d3f5a94e6d3c1d23cb429b8ba16f42cacce595ece5fbb394e8d6d4cf0a42e` |
| `sbf_build_diagnostics_total` | **0** |
| `cargo_lock_immutability` | `passed` |
| `spline_product_handoff` | `passed` |
| `reproducible_release_gate_sha256` | `ba0205ccd3537c1f9c67b0ffcb431defe96f70a6f209fa2738157e96abe5ce40` |
| `checked_upgrade_gate_sha256` | `32568317b901c1d9d7451c7988c6150ac51a18a24d37384194582f94297c4ffc` |
| successor campaign release pack | `3d1d1f46d62fcf9a4d0f1eee0746ae4959a253c6f1385ce02c822a84b5ab556e` |

**It is one candidate on one host and it is not a cross-host reproduction.**
The earlier candidate at `c38b26054` REFUSED at this same product handoff, which
is how the sequencer's stale close-coordinate fields were found; the run above
is the first at this revision, so the build-path control cohort-16 ran — two
absolute `--work` roots on the same builder — has not been run here.

---

## Addendum, 2026-09-05, lane COHORT-16C: `GyD95eyE…` is stranded, and the release identity that matters is the one this document did not name

**Devnet execution evidence.** Nothing above is edited; this reverses one
sentence of the PROGRAMS-17A addendum and says what replaced it. The whole
cohort-16.1 record is `COHORT161_UPGRADED_SEALED_2026_09_05.md`.

The 17A addendum concluded, of `GyD95eyE…`: *"its release id, manifest entry and
Market address are exactly what they were. A Trading link built from this commit
activates it as it stands."* **The first sentence is true of the Direct
capability release and false of the execution release set, and the second
sentence is false.**

A market pins the **execution release set id** — `Market.release_set_id`, offset
208, read off chain as `85defd75b236…`. That id is `sha256` over five
`(program_id, artifact_release_id)` bindings, and an artifact release id hashes
its role's ELF digest **and its deployment slot**. So a Trading link that differs
by one byte moves the set, and so does redeploying the identical bytes, because
Loader V3 writes the current slot on every `Upgrade` and refuses one in the
deployment's own slot. The field is written only by `initialize_market` into an
all-zero account and by nothing else, ever; the activation cache is derived from
it; and the one forward mechanism, `ReleaseLineageV1` with `lineage_walk`, has no
consumer in any capability program.

Cohort-16.1 upgraded Trading (and Core, which had moved at `17f1b6dec` — the
deployed set differs from the 87eec1c3a candidate in **two** links, not one) and
re-released. Both activation caches are live and distinct:
`2xVxMvfypJyo9bacGz1FFeK4L2qgqcsHaGoR9cbun6wV` for `85defd75…` and
`FCF1ggHcXoZaVx8PKS7YKnY166xL4E8N3ZaRsV29E11b` for `f533be49…`.
`GyD95eyE…` names the first and cannot be moved to the second.

**It lost nothing.** It could not be activated at cohort-16's release either
(this document's own §"Blocker 2", `0x4003` at 108,180 CU), and a market that
cannot activate cannot be retired. The market that carries the edges forward is
`BMK3BY415TicG5nTf43ii7YncgMyVcGGySoKWeGXKLKG`, founded on the new generation
and **activated at 521,895 CU with 36 accounts** — the first funding frame on any
chain to carry a dependency ledger.
