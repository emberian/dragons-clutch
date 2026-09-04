# The cohort-14 runbook

> **FROZEN until cohort-15 closes; superseded by `tools/cohort/`.** The rows
> here are now data in `tools/cohort/steps.tsv`, carrying `since` and `until`,
> and `tools/cohort/check-steps.py --prove-frozen` is the standing proof that
> this file is reproduced from them exactly. Change a row THERE, not here.
> This file stays because its prose — the hazard behind each row — is not in
> the table.

**Nothing here is authorization.** The devnet deploy grant in `AGENTS.md` is
standing and this document assumes it; every other act names its own condition.
`preflight.sh` runs offline, signs nothing, and reads no keypair.

```
tools/cohort14/preflight.sh --tests          everything checkable before a lamport moves
tools/cohort14/check-steps.py                the README and steps.tsv still agree
tools/cohort14/steps.tsv                     the same nineteen steps, as rows
```

Cohort-13's evidence is `docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md`
and every number below is priced from its ledger. Read its §6c and its
resolution addenda before running anything: three of the four walls it hit were
knowable in advance, and this file is what knowing them looks like.

---

## What cohort-14 carries that cohort-13 could not

| | why it needs a redeploy |
| --- | --- |
| `a517d27c` Trading's inline CPI input transport | OpenBatch cannot run without it, and it is in shipped bytes |
| `90a8563f` the Registry observes a deployment at finalization | the accelerator's release record is only meaningful under it |
| `e7ecfb2e` Claims admits the `ImmutableOwner` destination | otherwise the payout refuses before Custody is asked |
| `d218b963` the third collateral adapter release | the policy byte is inside a RELEASED IDENTITY a realm pins |

The fourth is the one that cannot be repaired afterwards by anything. Custody
selects `ExactTransferProfileV1` by matching the realm's stored
`collateral_adapter_release_id` against `PRODUCTION_ADAPTER_RELEASES`, and the
`ExtensionStoragePolicy` byte is inside that preimage. **Whether a market can
pay a wallet its own associated token account is fixed at founding.** Cohort-13
could not, under any version of this tree; its 165-byte auxiliary account was
never a workaround.

Measured on real Claims, Custody, Core, Registry, Resolution and Token-2022
ELFs, in `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs`:

| realm release | 170-byte ATA destination | CU |
| --- | --- | ---: |
| `228c14f9…` (cohort-13's) | refuses `0x6006 CustodySbfError::TokenState` | 363,236 |
| `430369ce…` (cohort-14's) | **commits** | 369,366 |

## THE ORDER, and the one place it differs from the brief

**Seal BEFORE founding.** The brief for this lane said "found → activate →
seal in place". Do not. Cohort-12 founded first and stranded its market: its
founded plan and its sealed plan carried different release-set identities and
the market was reachable by neither the fill nor the resolution. Cohort-13's
whole result was founding from the SEALED plan, and it is the first cohort in
this project's history whose founding and whose checked seal name the same
release set. The seal is key-free, read-only and costs **zero SOL**, so running
it first costs nothing and proves the identity the founding will pin is already
reachable.

Everything else follows the brief.

---

## The steps

Each heading matches a row of `steps.tsv`; `check-steps.py` refuses if they
drift. Costs are cohort-13's own, and the two founding rows are its single
founding cost charged twice.

### 00 close-cohort-13

Close the seven cohort-13 programs one at a time, ids derived from cohort-13's
keypair files and never transcribed.

**Verifier:** the deployer's balance rises by the table total less seven
5,000-lamport fees, and it closes exactly. Read each Program account back: a
closed program KEEPS its 36-byte account, its executable flag and the
ProgramData address it names, so asking the Program account cannot tell a live
cohort from a dead one — ask the ProgramData account it names at offset 4.

**Cost:** returns about **+42.12 SOL** (cohort-13's redeploy cost 42.123003619
and rent comes back).

### 01 deploy

`checked-release-candidate.sh --genesis-cohort` from a detached worktree at the
deploy commit, then seven `solana program deploy`.

**Verifier:** `CANDIDATE_EXIT=0` with `sbf_build_diagnostics_total=0`; a SECOND
detached worktree at the same commit reproduces all seven ELFs byte-identically;
and each live image is dumped back and compared to its ELF **before the next
deploy starts**. Three instruments, one claim, exactly as cohort-13.

**The reproduction is same-host, and that is not a formality.** Measured
2026-09-03 at three commits: two candidates on ONE host at two different
absolute `--work` roots give all ten ELFs byte-identically, and hbox (Linux)
against the laptop (macOS) gives nine of ten DIFFERENT. The cause is not this
tree: `platform-tools` ships a Rust standard library that embeds its own CI
build path (`/home/runner/...` on linux-x64, `/Users/runner/...` on
darwin-arm64) in the panic locations it puts in `.rodata`. Reproduce a deploy
candidate on the same OS as the machine that built it, or the comparison is
guaranteed to fail for a reason that has nothing to do with the deploy. See
`tools/release/README.md`, "Cross-host reproduction is scoped to one
platform-tools host OS".

**Cost:** **−42.26 SOL**, projected from cohort-13's measured 42.123003619 over
6,643,784 bytes (6,340.21 lamports/byte) against cohort-14's 6,665,200. The
affine model `890,880 + 6,960·n` predicts 46.40 and over-predicts by ~9%; use it
as a ceiling, never as a quote.

| role | cohort-13 | cohort-14 | delta |
| --- | ---: | ---: | ---: |
| registry | 234,536 | 238,000 | +3,464 |
| rent | 141,680 | 142,320 | +640 |
| custody | 572,272 | 576,552 | +4,280 |
| resolution | 819,256 | 820,248 | +992 |
| claims | 1,369,712 | 1,374,040 | +4,328 |
| trading | 2,320,152 | 2,327,616 | +7,464 |
| core | 1,186,176 | 1,186,424 | +248 |
| **total** | **6,643,784** | **6,665,200** | **+21,416** |

`solana program deploy` at CLI 4.0.2 allocates `Data Length` EXACTLY the ELF
length, so no program is growable in place; a larger successor needs `--max-len`
or a fresh identity.

### 02 ladder

`campaign --through activation`, deployer as Core upgrade authority, campaign
payer funded with 2 SOL first.

**Verifier:** a SECOND preflight that reads the CLUSTER rather than the driver's
exit code reports substrate, publication, initialize, succession and activation
all `complete`. Cohort-13's was 33 transactions, zero errors, 4,062,457 CU.

**Cost:** **−2.04 SOL** (2 SOL of payer capitalization plus 0.037 of fees and
record rent).

### 03 accelerator-release

`prepare` with the General accelerator's flag group, which publishes an EIGHTH
`ArtifactRelease` record beside the seven roles'.

```
--general-accelerator-program-id 8pgnyNvgdue7Jc8aw75BGWoghsKGevWJvFom8omUWvQY
--general-accelerator-elf <the 302,256-byte ELF, sha256 61b2d73d…>
--general-accelerator-sha256 61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae
--general-accelerator-semantic-release-id <operator-stated hex>
--general-accelerator-observed-programdata <the finalized ProgramData body>
--general-accelerator-expected-upgrade-authority 4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP
```

The authority is `ExactAuthority` and NEVER the literal `immutable`: the
deployer retains the key, and an omitted flag would mint an Immutable release
for a mutable program.

**Verifier:** the plan carries `general_accelerator` and a
`general_accelerator_artifact_release` record, and the Registry FINALIZES it —
which under `90a8563f` **is** the deployment observation:
`observe_artifact_release_deployment_v1` derives the Program and ProgramData
metas from the record's own content and compares them against the chain before
the staging cursor closes. A finalized record is the check, not a receipt of one.

**Cost:** **−0.01 SOL** of record rent.

### 04 seal

The checked release seal, key-free and read-only, **before any founding**.

**Verifier:** all five owned roles preflight `equal: true` against a FRESH
finalized observation rather than against the journal's own claim, and
`prepare --deployment-set-journal` produces a plan whose
`checked_upgrade_set_final_sha256` is the sealed set's digest with
`release_set_id` unchanged.

Two operational facts that each cost cohort-13 a cycle: the release-capture
family admits **only** `https://api.devnet.solana.com`, and the checked gate
resolves `provenance/`, `elf/`, `evidence/` and the twelve frame objects
relative to ITSELF.

**Cost:** **0.00 SOL.** Nothing is signed.

### 05 found-direct

`campaign --founding-only` from `plan-seal.json`, `--direct-fee-basis-points 50`.

**Verifier:** the Open Market's `selected_release_set` at
`STATE_SELECTED_RELEASE_SET_OFFSET = 208`, **read off chain**, equals the sealed
plan's release set. Never paste it out of `prepare`'s result JSON: that file is
the fragment's source, so the comparison would be a value against itself and the
refusal could never fire.

Re-observe from the chain rather than believing the driver's exit code. Cohort-13
exited 1 on a `getBlockTime` RPC error with the whole founding already landed.

**Cost:** **−0.34 SOL** from the campaign payer.

### 06 found-general

`devnet-general-market` to compile, then the ordinary founding campaign with
`market.json` as the market input. `input.direct_capability` is `None` and the
compiler refuses if it is not.

**Verifier:** `accelerator-observation.txt` reports `deployment_slot`
**491,959,038**. If it does not, the accelerator has been redeployed and every
certificate this market compiles pins the wrong artifact — and because the entry
is a seed of the Market PDA, that founds a DIFFERENT MARKET, not the same one
misconfigured.

**Cost:** **−0.34 SOL.**

### 07 activate-direct

`devnet-direct-capability-activation-v1 --execute`.

**Verifier:** the report's **verdict string** is `ACTIVATED` — not its exit code.
Cohort-13's first `--execute` printed `"planned"` and exited zero because
`activate.sh` built the flag into an array it never passed. Register the
activation root in advance from the manifest entry (never from the founding
checkpoint's `direct_capability_root`, which is a FOUNDING-PERMIT address at
which no account can ever exist) and require it to go from `AccountNotFound` to a
256-byte `DCLTCRT1` record across that transaction set.

**Cost:** **−0.01 SOL.**

### 08 activate-general

`devnet-general-capability-activation-v1 --execute`. The campaign report must be
a devnet one; the route checks the document and the endpoint separately against
the same value.

**Verifier:** the report carries schema
`dclutch-devnet-general-capability-activation-report-v1`, and the root named in
advance is occupied afterwards.

**Cost:** **−0.01 SOL.**

### 09 arm-relay

`devnet-sponsored-push-input-v1` **twice** — `--terminal-sequence 0` for the
capture and `1` for the settle, because the consumer refuses a mismatch and the
producer never overwrites an output path — then
`--action prepay-certificate --prepay-for settle`.

**Do this at founding time, not at resolution time.** The certificate seat's
rent is a CALLER OBLIGATION: the terminal route allocates and assigns but never
funds, and cohort-13 discovered that as `0x8002 ResolutionError::OutputState`
after 305,522 CU, thirteen seconds after its deadline, with a preflight that had
reported `planned`.

**Verifier:** the settle seat holds `rent.minimum_balance(312)` exactly, read
back from chain. On devnet that was 2,786,520 lamports; the arm computes it from
the Rent sysvar in its own snapshot rather than quoting a number.

**Cost:** **−0.01 SOL**, recoverable rent.

### 10 admissions

Fund two participants at 0.05 SOL each **from the campaign payer, never the
deployer**, then two user-position admissions and one collateral delegation.

**Verifier:** the buyer's delegated account holds exactly
`required_buyer_collateral` atoms. Execute directly rather than preflighting —
a preflight is not free on this driver.

The admission's config must name **`plan-seal.json`**, not `plan.json`: the
admission authenticates the report's own `plan_sha256`, and naming the unsealed
plan earns one coarse code over three conjuncts.

**Cost:** **−0.11 SOL.**

### 11 fill

The Direct session, nine transactions, gross 200 at 50 bps per side — the
smallest gross whose fee does not floor to zero (199 floors).

**Verifier:** the Hot transaction commits under the 1,400,000 CU ceiling, and
the measured figure is RECORDED beside cohort-13's **1,286,187** (8.1% margin).
The drift from cohort-12 was −30,942 CU across a larger ELF, so the number is
inherited as a measurement and re-measured every cohort, not remembered.

**Cost:** **−0.03 SOL.**

### 12 fee-settlement

`devnet-direct-fee-settlement-v1`, permissionless, no party to the trade signing.

**Verifier:** `fee_owed` reads **0** off chain after the transaction. That
readback is the only thing that distinguishes a settled fee from a sent one.

**Cost:** **−0.00 SOL** (75,000 lamports).

### 13 census

`ledger-census`, chained through `--prior` to the pre-fill boundary, with the
buyer's collateral source, the seller's Direct token PDA and the venue fee PDA
all named by `--token`:

```
--declared-collateral-delta 0
--declared-hoard-delta 0
--declared-class-delta unclassified=0
--declared-class-delta HoardPrincipal=0
--declared-fees-lamports <that cycle's own fees>
```

**Verifier:** L1 through L8 each reported **by name** with its actual verdict.
An INAPPLICABLE is not a pass, and a run that prints six greens and omits the two
it could not judge has reported a number it did not earn. Cohort-13 was the first
cohort with no INAPPLICABLE anywhere; cohort-14 inherits that bar.

From the first declared class, every unnamed class is a declaration of zero —
the flag is not additive commentary.

**Cost:** **0.00 SOL.** Read-only.

### 14 openbatch

OpenBatch N=2 against the activated General root, through the successor.

**Verifier:** the batch executes on chain. This needs cohort-14 specifically:
`a517d27c` moved the input bank to the inline CPI transport and cohort-13's
Trading predates it by nineteen minutes.

**Cost:** **−0.02 SOL.**

### 15 relay-capture

```
devnet-sponsored-relay-schedule-v1 --rpc-url … --window-record <DCLTWIN1 raw> \
    --wait capture
devnet-sponsored-push-v1 --action capture --input <terminalSequence 0> --execute
```

The window record's address is `accounts.window.raw` in the input document
step 09 produced. **Read the window from the account, never from a handoff** —
cohort-13's briefing said "around 15:00–16:00 EDT" and the account said
13:22:39–13:52:39.

The schedule waits until `start + margin` and never polls. Against cohort-13's
own recorded window it says the capture would have fired at **13:23:39 EDT**
with 1,740 seconds of window left; reproduce that offline with

```
devnet-sponsored-relay-schedule-v1 --replay-window 1788369759,1788371559,7200,0 \
    --replay-now 1788348159
```

**Verifier:** the capture commits AND the candidate's own
`snapshot_unix_seconds`, read off the candidate account, is inside
`[start, end]`. A capture that commits with an out-of-window observation is a
candidate settle will refuse `ProviderWindow` two hours later.

**Cost:** **−0.00 SOL.**

### 16 relay-settle

The same schedule `--wait settle`, then `--action settle` with the
`terminalSequence 1` input.

**Settle is NOT the same event.** It refuses while
`clock <= window.end + max_age_seconds`, so it is legal only strictly after the
primary deadline — two hours after the window closed, on cohort-13's numbers
19:52:39 UTC. The candidate survives the gap because settle re-normalizes
against the candidate's own snapshot, not the live account.

**Verifier:** the Source state reaches `Resolved` and not `FailureCommitted`,
and the certificate's kind byte is **1** and not 4. Kind 4 means the failure walk
ran, which means the honest observation was never captured — that is cohort-13's
outcome, and shipping it twice would make an oracle outage into founder revenue
a second time.

**Cost:** **−0.00 SOL.**

### 17 admit-terminal

`devnet-sponsored-push-v1 --action admit-terminal`.

An evidence refresh is needed first, and `resolution_funding_ledger` is
ADVANCEABLE rather than immutable — `68f0b3da` moved it, because every terminal
action debits it and a resolved market could otherwise produce no refresh at all.

**Verifier:** the Market's `phase` byte at offset 10 goes `1` → `2`,
`terminal_winner` at 12 carries the winning selector, and `terminal_receipt` at
328 carries the certificate's own address. Read all three off the account.

**Cost:** **−0.00 SOL.**

### 18 payout

`wallet-terminal-payout-input` then the payout, into **the owner's own
associated token account** — the destination the input operator has documented
all along and the one a browser derives when a reader supplies nothing. Create
it with the ATA program, not by hand.

**Verifier:** the destination is **170 bytes before AND after**, its
`ImmutableOwner` suffix byte-identical, and the Hoard falls by exactly the atoms
paid. The suffix assertion is the poststate half: the chain hashes the whole
account, so a transfer that truncated the extension storage would be a different
picture from the one the operator committed to.

**Cost:** **−0.02 SOL**, mostly the payout ALT's recoverable rent.

---

## The money, end to end

| | SOL |
| --- | ---: |
| deployer today | 32.473851850 |
| after closing cohort-13 | ≈ 74.60 |
| after the seven-program redeploy | ≈ 32.33 |
| campaign payer capitalization | −2.00 |
| every step from the ladder to the payout | ≈ −1.00 |

The deployer pays for the deploy and nothing else. Cohort-13's resolution,
payout and fee settlement moved it by **zero lamports**, and cohort-14 should be
able to say the same sentence.

**One movement cohort-13 could not attribute** is on its ledger as an open
number: the deployer moved −1.917836469 SOL during its founding window, which is
to the lamport what the General accelerator's deploy cost, from a keypair path
other lanes also use. Cohort-14 should expect the deployer to move only for its
own deploy, and should say so rather than explain it away if it does not.

## What no preflight can answer

Named because "not checked" and "fine" log identically.

1. **Whether a provider push lands inside the window.** The capture margin is
   provisional (`DEFAULT_CAPTURE_MARGIN_SECONDS_V1 = 60`) and the conjunct that
   decides it is the pinned account's own `publish_time`. The retry ladder, not
   the margin, is what makes the capture land.
2. **Whether the fill fits its CU ceiling.** 1,286,187 with 8.1% margin is
   cohort-13's measurement on a different ELF. Re-measure.
3. **The accelerator's semantic release id**, which is operator-stated because
   `checked_semantic_release_preimage_v1` refuses any role outside the seven.
   A `SourceSemanticRoleV1::GeneralAccelerator` label turns the flag into a
   check; until then it is an input nothing derives.
4. **The General market's external widths**, which are stated in
   `policy.json` and reconciled against no live account.
5. **`translation_validation` is Direct-shaped.** There is no General
   translation-validation corpus, so an honest General market names Direct's,
   and that is a claim about a different program.
6. **Retirement is still owned-loopback only.** Cohort-14 can be resolved on
   devnet. It still cannot be retired there.

Devnet evidence is not mainnet evidence, and nothing in this file is either
until it has run.
