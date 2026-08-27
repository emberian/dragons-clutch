# tier 4 — the Series occurrence waist, as a ProgramTest fast lane

Tier 1 is the infrastructure floor and runs on a real validator. This tier does
not. It drives real ELFs under `solana-program-test`, and TIERS.md is explicit
that a fast lane is **additional** evidence, never a substitute, and that the
tier must state which of the four fast-lane conditions it satisfies.

(Tier 2 is the Claims and Custody fast lane; tier 3 is the Direct AOT campaign.
This tier was authored as "tier 2" for about fifteen minutes on 2026-08-27 and
collided with tier 2's real owner; the campaign name in `bindings.json` is
`tier4-series-occurrence-programtest` and any ledger row still naming
`tier2-series-occurrence-programtest` predates the move.)

## Which conditions this tier satisfies

| TIERS.md condition | this tier |
|---|---|
| does not depend on genesis Loader-v3 ProgramData layout, a real `SetAuthority(Some -> None)`, or ProgramData deployment slots | **NOT SATISFIED.** The campaign constructs each ProgramData account itself — a 45-byte Loader V3 header with variant 3, deployment slot 0, and no upgrade authority — and Registry's release authentication reads that construction. A layout the campaign wrote cannot corroborate a layout the loader would have written. |
| does not depend on packet serialisation limits | satisfied — it makes no claim about frame width, and could not: ProgramTest submits no packet. Found31 missing the 1,232-byte legacy maximum by ten bytes is exactly the defect this kind of campaign cannot see. |
| sets the compute limit to 1,400,000 and the heap to 32,768 and treats neither as adjustable | satisfied. `set_compute_max_units(1_400_000)` in `found_program_test.rs`, never raised; the 32,768 heap is the SVM default and the campaign does not request a frame. The `every-transaction-fits-the-compute-maximum` witness is the check. |
| account shapes are the real Agave ones | satisfied for what it touches — the all-zero System Program with NativeLoader metadata, real Registry activation-cache bytes, real finalized-record PDAs. It touches no Token-2022 mint. |

**One of four is unsatisfied, and it is not a small one.** Read every row this
tier produces as: *these programs, built from this tree, agreed with each other
about a deployment the campaign described to them.* It is real execution of
real ELFs and it is not validator evidence. The census records the campaign
name on every observation so the report always shows where a row came from.

## What it drives

One route — `core/series_consume::process` — five times:

| transaction | outcome | what it proves |
|---|---|---|
| Series occurrence Consume founds its Market with a Core permit | executed | the occurrence consumes its ticket and commits Found at 258 outcomes; the Market lands in Founding/Prepaid and Core writes the `SeriesFoundingPermit` |
| ...the same, `(replay campaign)` | executed | the setup half of the replay case, bound separately so a campaign's setup is never mistaken for its result |
| Series Consume refuses a replayed ticket | refused, `core/CoreSbfError::Market` | the one-shot property: a ticket is the authority to found this Market at this index exactly once, and a second Consume would mint a second liability from one prepayment. The replay carries a **fresh blockhash**, so the bank cannot dismiss it as already processed — duplicate-signature rejection is the runtime declining to look, not Core declining to act twice. The campaign also asserts the Market and permit are untouched by the refused replay |
| Series Consume refuses a substituted Claims ProgramData | refused, `registry/RegistryError::Deployment` | release identity: Registry's batch refuses the substitution and Core propagates its code unchanged |
| Series Consume refuses a late Hoard postcondition | refused, `core/CoreSbfError::ChildAck` | atomicity: the refusal lands after Found committed and after the child CPIs returned, so the transaction boundary must undo the Market, the permit, and every replay account |

The transaction's top-level program is
`programs/dclutch-core-sbf/test-programs/series-consume-caller`, a test-only
caller standing in for Trading's Series route. **This tier therefore binds no
`trading/*` route and `programs.json` deliberately omits `trading`**, so no row
can claim Trading executed anything. Its address in the campaign is the one the
census would call `trading`, which is precisely why leaving it out of the map
matters: a mapped label would let a future binding credit the caller's
invocation to the real adapter.

## What it does NOT reach

`core/series_open::process` and `core/series_permit_expiry::process` stay
NEVER-EXECUTED. Both need the joined founding composition and an open Series
Market, which is the founding lane's, not this one's. `blocked.json` names them
with that reason.

`programs/dclutch-series-sbf` is a different matter: its four routes are
NEVER-EXECUTED and **no campaign anyone writes can drive them**, because its
Core seam does not exist on the Core side. See
`docs/evidence/SERIES_ADAPTER_CORE_SEAM_2026_08_27.md`; that one needs an owner
decision, not a tier.

## Running it

```sh
tools/gauntlet/tier4/run-campaign.sh
```

It builds the five ELFs, runs the campaign with
`DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` set, folds the per-transaction records into
one document, checks the witnesses, and folds the result into the census
ledger. Everything lands under `--work` (default `/private/tmp/dclutch-tier4`),
never in the repo.

It needs no validator and no port, so unlike `run.sh --mode full` it is not a
single global slot and may run while another lane holds one.

## The producer

`tools/gauntlet/program-test-evidence` is the general emitter, not a Series
thing: any real-ELF ProgramTest campaign in the tree can call `record` per
transaction and become visible to the census. Before it existed, the only
producer was the local-validator bootstrap, and every ProgramTest campaign in
the tree drove routes the census could not see.

**It is also not the only such emitter in flight.** As of 2026-08-27 there are
several, written independently within the same hour by different lanes. The
census's `observe` contract is one shape and should have one producer; whoever
converges them should, and this one has no claim to being the survivor.
