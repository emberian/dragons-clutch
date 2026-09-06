# COHORT-17 — THE FIRST REFUNDING MARKET THAT COULD BE FILLED

**Devnet evidence. Not mainnet evidence.** Written by lane COHORT-17, 2026-09-06.
Dated and not edited; a reversed verdict gets an in-place, dated addendum.

Cohort-17 exists to carry four links that cohort-16.1 could not: the closure
burn (`7d45d6ba3`), the retirement tail (`42c3bb931`), the General account
profile's liability basis (`1dd18be91`) and the profile checkpoints
(`fc39a5bc7`). The question it answers is the one cohort-16.1 could not reach:
**can a market founded REFUNDING be traded, settled and retired at all.**

What this document reports, in one line each:

- the candidate reproduces from two roots on the named builder, and cohort-17 is
  **four** links and not the three the ledger says;
- the founding SEATS the failure column in the market's own escrow, and the
  census reads L3 HOLDING where cohort-16.1 read it VIOLATED;
- the fill is CERTIFIED, which cohort-16.1's could not be, and the escrow is why;
- the General market's basis wall is CLEARED and a later one is named;
- the runbook had ten defects that only running it could find, and each is
  repaired in the runbook rather than worked around in a lane.
## 1. THE CANDIDATE, AND THE FOURTH LINK NOBODY COUNTED

    deploy commit      932edc83fc5a108fa362be216c84a3b0f78f29b4
    driver commit      5a0d022e2 (record-core-digest), detached worktree, debug,
                       CARGO_TARGET_DIR = the live tree's target/
                       sha256 0a497c4eaf39dfc0056b60a558386819956895e33c212778a249786f7160de96
    builder            hbox through swarm-build, platform-tools v1.53 on
                       Linux/x86_64, node v26.4.0, SWARM_MEM_MAX=32G
    reproduction       two repository roots (/tank/dclutch-c17/repoA, repoB) and
                       two --work roots on that host
    release gate       a98ed988f9d266084b3835dc5f838a03c8513810669395373960d58ce7df6be3
                       EQUAL from both roots; all eight ELFs byte-identical (cmp)
    diagnostics        sbf_build_diagnostics_total=0 in both summaries;
                       sbf_build_freshness=passed, links=8; release_builder=true

| role | cohort-17 | cohort-16.1 | moved |
| --- | --- | --- | --- |
| trading | `65ff376e876e3398d4e438171a955314202c9d3c7a194a9035231f203ac0c596` | `e7f8e476…` | YES |
| claims | `26809c62bf1ffa675ec73a746f4c9956e8d734612c59b04f9b827b2aa1a2ecdc` | `33e453e6…` | YES |
| core | `2f28309fee8c8d2dde6c141ae65c16ad1a24a2bcdb5284b441984d9def01e0f0` | `29200c85…` | YES |
| accelerator | `d2ff2b87ac79e5329b83f10e10a2f2d9b03ac16d445d070188f03946598da91a` | `587181d9…` | **YES** |
| custody | `2600db72b383fc750bc8a975dfe19233b78012a4d53c21c680089bd42bcf7410` | same | no |
| registry | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` | same | no |
| rent | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` | same | no |
| resolution | `7be8a398be52342546a953cccc04b7276411041eca0081990d1e43be5ed7c34b` | same | no |

**Cohort-17 is FOUR links, not three.** `docs/ledger/2026-09-05.md` and the
frames ratchet both say three -- trading, claims, core -- and the accelerator
moved too, between cohort-16.1's candidate `87eec1c3a` and this one. PROGRAMS-17F
measured `d2ff2b87…` at both `1dd18be91` and `bded4c038`, so the move is at or
before `1dd18be91` and is not the basis-rule commit; which commit moved it was
not bisected, because a bisect costs a build and the count is the fact that
matters. It changes nothing about the deploy -- a genesis cohort deploys eight
fresh identities regardless -- and it changes what a reader is told the cohort is.

The two runs' `CHECKED_UPGRADE_GATE` and `SUCCESSOR_CAMPAIGN_PACK` digests are
NOT equal (`6569364c…`/`45a7c982…` against `d585477c…`/`642637ca…`); the
reproducible release gate and every artifact are. Only the latter is this row's
verifier, and the difference is not investigated here.
## 2. THE FOUNDING WAS LAUNCHED TWICE BEFORE IT RAN, AND BOTH ABORTS WERE RIGHT

Neither abort touched a chain. The founding frame has no suffix resume, so the
only safe place to stop one is BEFORE the frame starts -- and both stops were in
`cargo build`, with the price-update file the only thing on disk.

**Abort 1: the driver was building under the wrong toolchain.** The stage script
`13-found-direct-1.sh` shells into `tools/release/stage-devnet-sponsored-market-open.sh`,
which builds the successor driver with

    cargo build --locked --manifest-path "$BOOT/Cargo.toml"

`--manifest-path` does not move cargo. **rustup resolves `rust-toolchain.toml`
from the current working directory**, and this lane's shell had its cwd reset to
a sibling checkout that pins `nightly`. So `$REPO`'s driver compiled under
`nightly-aarch64-apple-darwin` while `/Users/ember/dev/dclutch` pins `1.97.1`,
from a cold target directory, and NOTHING SAID SO: the log is a wall of
`Compiling` lines either way. The only tell was the toolchain path in `ps`.

Repaired at both build sites: `( cd "$REPO" && cargo build ... )`. The toolchain
is a property of the repository being built and of nothing else. The generator
carries the same guard one level up -- every emitted stage now `cd "$HERE"`
before it does anything -- as defence against the class, not as a second author.

**The cost was not only the forty minutes.** `target/` is shared by every lane
in this tree, and a nightly excursion through it invalidates the 1.97.1
fingerprints, so the next lane to build paid a cold rebuild too.

**Abort 2: the window lead is anchored at LAUNCH, not at the founding.**
`--window-start $(( $(date -u +%s) + 2400 ))` is evaluated when the stage script
starts. A build queued behind another lane's cold rebuild therefore spends the
market's whole 2,400-second lead before the founding frame opens, and the market
would be founded with its capture window already open -- with `activate`,
`checked-execution-release`, `arm-relay`, `sim-config`, `admissions`, `fill` and
`fee-settlement` still to run inside a 1,800-second window. The relaunch was
held until the driver was already built.
## 3. THE REFUNDING DIRECT MARKET, AND THE SEATING THAT MADE IT TRADEABLE

    Open Market (founding_market)  9e8fTH75s82pjcEK8pY8PaLPoZW6W1am1qQ6J4JHjagQ
    Found31 Market (market)        7yaJUoUk8qjzPUMVeehs3owsmdfHy6Ud2nyJNNMruEYt
    generation                     2
    collateral mint                3FdkfVoG1C3khoDpXpDsfC7czTPrR9NbVegHTNuJ7Bfd
    Hoard vault                    DzPa65EueTKcG1CBjowDH6rE8jWzDpivfzVXsDEX9e2o
    Claims aggregate               EMgGTUGsmdvvvve7pLhDrWxaYGqjkLUEqzXA51R1ndPv
    founder Position               8txUquS8iyLWBUpzCmYsH2fPkST7L22wwCT4JjArs5Xo
    FAILURE ESCROW Position        8HLLen86V9Vy7NXQYHxP1hGEm4FT99av6WWsagm2FbNA
    claims admission               GiPm7k4F6uf4JB9rA6yeAiYGbzGmkj5KiEDJ4iZ8mGhg
    linked liability basis         Cv9t9a7tJV5Fm3d1rderPnyAKFXmyRf2CGepCvkSM7Rz
    capability manifest record     DUfiXKPgVboiRudPH1wwYPSZ9XkACtNAQ9zpKkzWSFXn
    window spec record             AxQF2E338bv7wr1UFjySQzVskWEAXFRB5KgjV1jmvs6N
    frozen DCLTGMF3 routing table  DMcjvbaftXzqdoHY8GHJg4Qvf6h7TjeKaeYm5vogMqa9
    selected manifest entry index  0
    window                         09:04:55Z to 09:34:55Z (2,400 s lead, 1,800 s wide)

The compiler's own founding terms, quoted verbatim from its stderr:

    founding-reserve-terms-v1: reserve 1000000002 atoms; founding budget
    500000001 atoms (the lower half); basis width 4; derived payout scale 3;
    complete sets 166666667 = budget / scale, exact; reserve provenance
    derived: the smallest reserve at or above the intended 1000000000 atoms
    whose budget this scale divides exactly

**Payout scale 3 against basis width 4 is `categorical_refunds_on_failure_v3`:
this is a REFUNDING market, the shape no market had ever traded past.**

### THE SEATING, READ OFF CHAIN

`escrow-seated`, which is `ledger-census` at the founding boundary:

    L1 holds  tracked 1000000002 atoms across 3 accounts == Mint supply 1000000002
    L3 holds  4 Positions sum to the aggregate supply vector [166666667 x4]
    L4 holds  Hoard 500000001 >= worst outcome 166666667 x unit 1

    failure-escrow  [0, 0, 0, 166666667]
    founder         [166666667, 166666667, 166666667, 0]
    participant-1   [0, 0, 0, 0]
    participant-2   [0, 0, 0, 0]

The escrow holds the WHOLE failure column and the founder was issued NO failure
claim. That is decision 0025 item 2, on a chain, for the first time. Cohort-16.1
founded the same shape and read

    VIOLATED L3: Positions sum to [166666667, 166666667, 166666667, 0]
                 but the aggregate owes [166666667 x4]

and could not fill, could not retire, and could not be repaired.

### THE SPINE, WITH ITS SIGNATURES AND ITS COMPUTE

| act | signature | slot | CU |
| --- | --- | --- | --- |
| activate-direct | `4A8x8iW19YD7gQN4pxGc3ejfPi3Pn8cqaYFDghehnjFu4CyeUXW3vQJtxUrNe3TTLPkJhcZm9af8nwKvSTrtiz8v` (last of four) | 493,985,609+ | **508,377** on 36 accounts |
| admission participant-1 | `3PZ8wbf8hJooJBc7hQ83ewFcXQCfHo6FjkT4fT3BeTRdyHrb6y6w9u1v9tdzaKyS163xTBKcdGuL6XNasCAurGoy` | | |
| admission participant-2 | `3Ci6SPg2bBZSajY2EchCotUiLeQSxdXCACv4Q3u9powBFXBa4cypVn5ZPsfpjoXnbhg9aR4sJ4QmiV7xV2EBgMK5` | | |
| relay capture | `38nrFuZGazvSDjHxHWUvmvs6soiRsSBdUEgquYLmchmSHvRAzg5rVhgsmK5CWW7kp3rCStfYFMZKcRqwM3qTsxe1` | 493,992,422 | candidate `8zrHgpvyuDrssDWdwCp589CP2Ua8YrdxjmBQRzVVUAV8` |
| fill (Hot) | `4cevxQwCHnTdfT5o6upCVH71ioE5Th99ew4hqgMhdDmweFsEBXnhFFbzwik9NwQwi9GgEHGgAwRqi7ajt2WCc5Zm` | 493,995,625 | **1,116,618** (ceiling 1,400,000; cohort-16.1's 1,111,824) |
| fee settlement | `2ike4FN3MkxxZ28xjhS1MbYu5yAK4Pc6xusZEQcUMctxrCCj4uycLHGJnwGXhyDhjRfRc2HHigv9ih3kyT2j1W1n` | 493,995,908 | **94,853** |

**THE FILL IS CERTIFIED, AND THAT IS THE POINT.** Cohort-16.1's Hot transaction
LANDED and its driver then refused

    REFUSED: Direct terminal claim schedule is not the exact K+1 partition

because `direct_claim_balances_v1` collects only NONZERO balances and the seller
could not contribute a fourth row it never held: seller 3 + buyer 1 = 4, against
the required `outcome_count + 1` = 5. **The escrow is the fifth row.** Cohort-17's
`direct-trade-finalized.json` reads `status: finalized`, and the post-fill census
reads

    L1 holds  tracked 1000000002 atoms across 5 accounts == Mint supply 1000000002
    L3 holds  4 Positions sum to the aggregate supply vector

with the claims moved (`founder [166666667, 166666467, 166666667, 0]`,
`participant-2 [0, 200, 0, 0]`) and the collateral split 199 to the seller's
destination `DqRro8k3HcoJmbCMitfX7yLigMJpaGe9u27URd695rxN` and 2 to the venue fee
account `8NayUmLgEAa19LcCMGShcfT1BBMB7PQH75Ne1Bj2GjwT`.

**The relay capture needed eight tries and SEVEN OF THEM WERE THE LANE'S OWN
FAULT.** `devnet-sponsored-push-v1` is plan-then-sign ON ONE OUTPUT PATH: the
first `--execute` writes a key-free planned report and refuses, and the sign
must follow it back to back because the candidate PDA is seeded by the live
`publish_time`. The EMITTED STAGE SCRIPT DOES EXACTLY THAT, and says so in a
comment it carries. This lane did not run the emitted stage -- the `blocks`
chain puts `census` in front of `relay-capture`, and the census could not be
green until the fill, which is not what the chain requires -- so it hand-rolled
a loop that used a fresh path per attempt and planned six times without ever
signing. The seventh, signing a plan made five minutes earlier, drew a real
refusal from the deployed Resolution program:

    custom program error 0x8015 = ResolutionSbfError::SponsoredPush
    "Sponsored-push candidate, head, release, or deadline authentication failed"
    89,993 of 1,399,700 units, no CPI

The eighth was a plan and a sign back to back and landed on the first send. **The
runbook was right and the hand-run was wrong**, and the only real finding here is
that `0x8015` is what a stale plan looks like -- a code coarse over four causes
by its own doc comment.

**A `blocks` edge that is not a chain constraint costs a window.** `census`
blocks `relay-capture` in the runbook, but the capture is bound by the market's
own window and nothing else; cohort-16.1 fired its capture BEFORE its admissions
and its fill, in that order, and was right to. Left as a finding rather than
edited under a running clock.

## 4. THE RETIREMENT: BEGINRETIRING EXECUTED, AND ONE HOST GUARD STANDS BETWEEN IT AND RETIRED

Cohort-16.1 could not reach `BeginRetiring` at all. Cohort-17 executed it, and
then the closure of the funds behind it, and stopped one stage short of Retired.

| stage | signature | phase |
| --- | --- | --- |
| terminal ALT create | `5H2oPkpfWCqLgUAQGXEDYfN53o7MoqQfADWEYDhC7tb4` | finalized |
| ALT extend x3 | `4r6TfLEeyVzjr8GzTL6MRJVA5ZTVeiyX94KHBWuwqa2i`, `4NMF84C74kAwytvpazpSFLDx8J3coY2jNVxujd2SEGcf`, `5sHMgB1kPHLhT7SB1w4QKRpUqfXHHWKY1fZA5QPVuKcy` | finalized |
| ALT freeze | `v28yQFjP2dcdvNtntucjjPH2F2giahfCfEvS7U1SZ3Dk` | finalized |
| **core BeginRetiring** | `1jKfxPjvY6J2VvSsz7erkX1hkceviQTfptAwtefj1JuU` | **finalized** |
| **direct BeginRetiring** | `54JL4eQLcLPUb5en5tmAZGS1i9FNM2d4eac91oYi5Ajh` | **finalized** |
| resolution receipt prepay | `2H5C7BhvK1GTDoecSubxuvZNn9Gv44WMfK2xypRyCaB6` | finalized |
| **ResolutionCloseFund** | `3L6ULNQN4S41wnYN9KuYH7pwoCFyexbgpJyACQfNCPtE` | **finalized** |

Payouts that emptied the market first:

| payee | claim index | atoms | CU | signature |
| --- | --- | --- | --- | --- |
| **the stranger** (participant-2) | 1 | **600** | 215,396 | `5GLKEVCi5pgiebHSZJo4EXCa92ChzFPgVQjx3ajXGgdJwYKPm2tMha7AC7VJUupb943hpcUiRXbCBfiYxvvsUiJn` |
| founder | 0 | 0 | 149,302 | `3sSyg43R28Lo43MSodpCQLS25CNUiEYCT4ojUZQBZmxBCC4ygvZmFQACcPJZjNzXF5obR4C1Za7Pw2mvr26pfdPu` |
| founder | 1 | 499,999,401 | 215,396 | `3arxHzuieW4cHddyrykdYgT41wnC7NuHSpfqf8y51x9BZmfvVHawEzPDCCZahLAta4HJ8nHNy7fKLTTHuFwss6vZ` |
| founder | 2 | 0 | 149,302 | `5uCLdfjrWNAFYBsyYAwXXeC7XLcVsJT7zdEbwhib7bXWxxPH9AexHkoctcaARqcBzKtLDYpiQQbsBsLQ8NrEuamR` |

The census at the retiring boundary, read off chain:

    L1 holds        tracked 1000000002 atoms across 5 accounts == Mint supply
    L3 holds        4 Positions sum to the aggregate supply vector [0, 0, 0, 166666667]
    L4 inapplicable the Market is retiring: settlement DISCHARGED the liability
                    this law is stated about
    Hoard           0
    aggregate       [0, 0, 0, 166666667]
    failure escrow  [0, 0, 0, 166666667], still open

**The three ordinary columns are gone and the Hoard is empty. What remains is
exactly the column decision 0025 says the CLOSURE burns**, and the closure has
not run because the stage before it has not.

### THE WALL, LOCATED TO ONE CONJUNCT ON ONE LINE

    Error: Direct close caller preflight: Successor(MakerRootCountInvariant)

`require_closable` (`crates/dclutch-trading/src/successor.rs:706`) is one
discriminant over two conjuncts, and the failing one is measurable: the Direct
capability root `3dgyRvrebvaXKL8Z1hqgYMWRewXXsgxBjbseERjep3xK` reads, at its own
bytes,

    DCLTDRT1 ... open_maker_root_count = 2

The fill opened two maker replay roots and both are live: `59rvxdSf4vzfin1pdh
VidvDHEZSAAgURhaPo8GNkNZRu` (seller) and `VnQRqKSg3uHnQj8VdoqEX3vVv6PiwiRvMGJ
UGvM15kw` (buyer), 160 bytes each, owned by Trading.

**The producer for closing them EXISTS** -- `devnet-direct-close-maker-v1`,
"closes one Direct maker replay inside Retiring: wall 22's missing decrement" --
**and the runbook has no row that calls it.** That is the producer-missing
pattern, in the runbook rather than in the tree.

**And when it is called by hand it refuses, and the refusal is a host defect:**

    REFUSED: embedded Direct Hot journal lookup closure was not exact, ordered,
             and distinct

That message is one sentence over three conjuncts
(`tools/local-validator/bootstrap/successor/src/direct_trade.rs:5757-5765`), and
the failing one is the FIRST:

    lookup_keys.iter().any(|key| *key == Pubkey::default())

**`Pubkey::default()` is thirty-two zero bytes, which is the System program.**
The Hot frame's frozen lookup table `H2QMjhby88h4mGAovFX8rwS3BJcgUXTMEnEfMAVLexox`
holds 57 addresses and its **index 36 is `11111111111111111111111111111111`** --
read off the chain, not off the journal -- because Trading CPIs System to create
accounts and the table must name it. The journal reproduces the table exactly
(all 57 distinct, index 36 identical to the on-chain word), so the list is
neither inexact nor indistinct: the guard is reading a legitimate System-program
entry as an unset slot.

The list's authentication is the third conjunct, `pubkey_list_sha256(&lookup_keys)
!= journal.lookup_addresses_sha256`, and a genuinely unset slot cannot survive
it. **NO CHANGE WAS MADE.** This is a ruling for the owner: whether the
default-key conjunct is redundant against the hash it sits beside.

**What that costs:** `open_maker_root_count` stays 2, `require_closable`
refuses, `DirectCloseCapability` cannot run, and the four retirement packets and
the closure's burn are unreachable. The market
`9e8fTH75s82pjcEK8pY8PaLPoZW6W1am1qQ6J4JHjagQ` sits in Retiring with an empty
Hoard, three discharged columns, and its failure column seated exactly where
decision 0025 puts it.

**No market has been retired on any chain, and this cohort says so.** What moved
is that the wall is no longer the founding's seating, no longer the manifest's
dependency edges, no longer `BeginRetiring`, and no longer `ResolutionCloseFund`
-- all four of those are behind it now.
## 5. THE GENERAL MARKET: WALL 3 IS CLEARED AND WALL 4 IS NAMED

    Open Market (founding_market)  E5jHZvCPLJ2K22dayGuJM9MzDZhKS5FGPqz45cJ3HabY
    Found31 Market (market)        CJVARrAjDvv45GnExJfRKn2LdGCyNXFVqRr1GdaNg7Ne
    generation                     2
    collateral mint                7AGbrfnU5SrFfi1qoR1JgV2BRad7hQbotqT9yX2R7keX
    Claims aggregate               ARCw6w3UoWvkE1KBjDaxxLm4x3huH4Um56aSsTZSQHPC
    founder Position               7Ey7qTAjv7LZBMNEbgEuBvsQH7NCRo5gX7iUDvcCe3ei
    result domain record           GzaQibVTxLVcdaHD2nRb4uhQHgMvn7iARwHFo4Kcyk5V
    portfolio record               A87J263uYDcMhY1bjEDHRLTjbdAnjFNKGcG294x4T9w2
    linked liability basis record  DRETjpnLtT4eYrRz8GS8DghkQJVW2MEiCsPaFo2yz7C5
    rent credit                    2oaNBEctfWKwgPFc9fHueBN3YY7gLqohbqn62rNCUNC2
    founding frozen table          4jZvzXdtbS12sCwA9bHdYD57LQDTiJKgzmTt5YUdbqJq (DERIVED)
    founding                       547 transactions, completed true
    activation                     484,523 CU (cohort-16.1's 520,541)
    capability seal                3AryF51KGHt6BLTxBzA58A1tCZM1w4PAmf8C2gY5dPAn
                                   `2dUc4EkP2L6JjTVE2GdfjPEagTwZJkW2XSuZXj7QjfEgx2wvXMiWDGNDLUu73SRKSnqv6G2XdEqcmvvwRJ5QAu1R`
                                   slot 494,024,322, 622,058 CU, action 7,
                                   frozen table 3Q1dJ8JCpBajEUSq4M4iPouD5nAMgEXk7Fm1XnBsECpG

The General inputs are this cohort's own: `compiler-release.txt` and
`toolchain.txt` authored for accelerator `WMGVfhVu…` / `d2ff2b87…`, a fresh
134-byte `price-update.bin`, and a translation-validation manifest RE-MINTED
from a real `check.sh` run against this tree,
`976e535fa1bb34e0b9eed997f6f655e904e783ebeb4723e6f71195281742e267`, whose whole
evidence directory is archived at `candidate/translation-evidence/` so the next
cohort does not lose what cohort-15 lost.

### THE OPENBATCH REFUSAL, AND WHY IT IS PROGRESS

`openbatch-frozen-tables` ran both passes and both frozen tables. The route
compiled, the plan produced, the execution simulated at **361,934 units** and
devnet's preflight refused the send, so nothing landed and no fee was paid.

    outer   272BExJXWc7FSFh5PLps6DBWVWqoggmskoPbfbp21VNd (Trading)
            custom program error 0x4004 = TradingSbfError::Transition, Custom(16388)
            consumed 361,626 of 1,399,700
    inner   WMGVfhVuWuMRCkBDnR7nrqj6uY1XbnxQtShxdM4h7km (the accelerator)
            consumed 31,797 of 1,071,478 -- INVOKED, and it answered
            log: general-candidate: refused, a record did not decode
            log: general: refused, candidate projection
    receipt DCLTAAK2, 144 bytes, generation 2
            4e39c194c5f2cf420805a21cacf648b7458c831f18101ad2d6224394961ca32f
            aa89ce45d8707c686380aafbbe119014a589e62d6eabe95d4a036c59e171d99b

**COHORT-16.1's WALL 3 IS GONE.** Its refusal was
`GeneralAcceleratorSemanticErrorV3::ConfigMarket` --- the account profile
projected the Portfolio's CLAIM basis at offset 96 while the config binds the
LIABILITY basis at 128 --- and PROGRAMS-17F's repair moves the published
artifact, the descriptor and the manifest entry and moves no link. Cohort-17
founded on the corrected profile and the accelerator no longer disputes the
market: it now reaches the CANDIDATE PROJECTION, one phase later, and consumes
31,797 units getting there against cohort-16F's 30,771.

**Wall 4 is `GeneralHotCandidateErrorV3::InvalidPlan`**, whose text is "a record
did not decode" and which is COARSE by construction: it is the target of a
`map_err(|_| …)` at eight separate sites in
`crates/dclutch-trading/src/general/hot_candidate_v3.rs` --- 755, 816, 820, 879,
881, 929, 996 and 998 --- covering `GeneralRootV2::decode`, `GeneralBatchV1::
decode` and the projection in between. It is exactly the shape AGENTS.md names:
"`map_err(|_| Coarse)` converts a located defect into a search". **It is NOT
localized here, and no program was changed to chase it.** The step stops at the
refusal, as instructed.

`close-batch` and `second-open-batch` need a Batch this refusal did not create,
and were not attempted.

## 6. TEN RUNBOOK DEFECTS, EACH FOUND BY RUNNING THE ROW

A row that has never run is a row with no author. Every one of these was
invisible to `check-steps.py`, to `preflight.sh` and to `test.sh`, and every one
was repaired in the runbook rather than worked around in this lane.

1. **`MARKET_KIND` is keyed by row.** `openbatch-two-pass` and its two cohort-17
   successors ask for `market.result_domain_record`, which only a General market
   carries, and the default kinds sent them at market 1 -- so the generator
   refused the whole General family and deleted every script it had written.
   *That is why cohort-16.1 ran those five OpenBatch acts by hand.*
2. **A `{stage:…}` brace in VERIFIER PROSE.** Prose is not substituted, so
   `seal-general`'s tail reached the emitted script as the literal
   `/session.json` and the self-contained value test read it as an absolute path.
3. **The preamble created `$OUT` and nothing else.** `prepare --output-file
   $HERE/observed/<role>.programdata.bin` died on the first of seven roles with
   `No such file or directory (os error 2)`, naming neither path nor flag.
4. **A write directory must survive its own backup.** `devnet-direct-trade-
   produce-v1 --output-dir` refuses an absent directory; the preamble created it
   and the backup step removed it one line later. `prepare --account-dir` is the
   opposite and refuses one that EXISTS, so the exception is named rather than
   guessed.
5. **The `?` guard checked an input.** It took the first write-shaped flag, so
   `arm-relay` -- which passes `--evidence <the founding report>` before its
   `--output` -- announced "present, not overwriting" about a file that had been
   there since the founding, and produced nothing at all.
6. **`{execute:…}` never expanded its payload.** Single-quoted,
   `--fee-payer-keypair $HERE/keys/campaign-payer.json` reached the driver with
   the four characters `$HERE` in it, and the refusal was "fee settlement payer
   keypair path must be absolute" -- about a path that was never a path.
7. **The fill row named a file the producer does not write.** It passed
   `direct-trade-private.json`; the producer writes `direct-trade-session.json`
   and states it in its own `privateSession` field, which the row now reads.
8. **`seal-general` passed `--routing-table` unconditionally.**
   `devnet-capability-seal-v1` publishes its own table unless given one, so an
   empty manifest field made `--execute` the flag's value: "invalid pubkey
   --execute". The table is an OUTPUT of that row.
9. **`escrow-seated` had no producer, and then no token list.** The row is the
   census that already derives the escrow through
   `dclutch_operator::failure_escrow_v1`; giving it only the Positions left L1
   reading "tracked 500000001 atoms across 1 accounts".
10. **The census's `--token`/`--position` take `LABEL=PUBKEY`.** Cohort-16's
    manifest carried empty lists, so no cohort had ever put a value there.

And one defect NOT in the runbook, which cost this lane forty minutes and every
other lane a cold rebuild: **`cargo --manifest-path` does not move cargo.** See
section 2.

Two further findings are left as findings, not edits:

- **A `blocks` edge that is not a chain constraint costs a window.** `census`
  blocks `relay-capture`, but the capture is bound by the market's own window and
  by nothing else. Cohort-16.1 fired its capture before its admissions and its
  fill, and was right to.
- **`arm-relay`'s third act is plan-then-sign and its shape is `once`.**
  `devnet-sponsored-push-v1 --action prepay-certificate` writes a key-free
  planned report on its first `--execute` and signs on the second, and a `once`
  row runs it exactly once -- while the emitted `attempts` and `wait:` shapes
  already run the pair back to back and say so in a comment they carry.

## 7. THE LEDGER, TO THE LAMPORT

    deployer before anything                      28.657732010
    close cohort-16.1's seven roles              +32.808449040   61.466181050
    close cohort-16.1's accelerator               +3.740043320   65.206219370
    deploy the eight                             -36.601529360   28.604690010
    fund-payer, 2 SOL plus one fee                -2.000005000   26.604685010
    the ladder (36 tx) and the seal                -0.032285920   26.572399090
    deployer, unchanged since the seal                           26.572399090

    payer before the cohort                        1.364363844
    the one recorded top-up, deployer -> payer    +2.000000000    3.364363844
    the whole Direct spine, the General founding
      and seal, both OpenBatch tables, the four
      payouts and the terminal sequence           -1.359424138    2.004939706

Closing cohort-16.1 returned **36.548487360 SOL** and the eight deploys cost
**36.601529360** — the cohort very nearly paid for itself, and the 0.053042 SOL
difference is the four links that grew.

Nothing was spent on the General `OpenBatch` itself: devnet's preflight refused
the send, so no fee was paid for a transaction that never landed.
