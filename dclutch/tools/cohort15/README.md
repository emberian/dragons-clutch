# The cohort-15 runbook — a delta over cohort-14

**Nothing here is authorization.** The devnet deploy grant in `AGENTS.md` is
standing and this document assumes it; every other act names its own condition.

**Read `tools/cohort14/README.md` first and run it.** This file is not a second
copy of it. Cohort-14's nineteen steps still describe the cohort; the five rows
here are what cohort-15 carries that cohort-14 could not, and each names where
it inserts into that ladder.

```
python3 tools/cohort14/check-steps.py tools/cohort15    the README and steps.tsv still agree
tools/cohort14/preflight.sh --tests                     unchanged; still everything checkable offline
```

---

## Why there is a cohort-15 at all

Cohort-14's General market is founded, activated, and cannot execute a single
one of its fifteen actions. `d2d342573` derived the whole `OpenBatch` frame from
the founded Market and found two walls, and `7a18a2272` recorded them. Both are
fixed in sources now and **neither can be repaired on cohort-14**:

| | why it needs a redeploy or a re-founding |
| --- | --- |
| the caller-authority seed | it is a PDA derivation inside **shipped Trading bytes**, and every admitted caller-authority address moves |
| the published external widths | they are inside a **founded** `AccountProfile`, and an `Exact` prestate is a refusal, not a preference |

**The seed.** Each of the four admitted caller authorities at top-level
coordinates 47..50 was `find_program_address` over `sha256(accelerator request
header ‖ inline register bank)`. `OpenBatch`'s AccountProfile declares
`TrustedEnvironmentV2::CurrentSlot`, so Trading seeds `scalar::CURRENT_SLOT`
from `Clock::get()` into that bank on every execution — so each address was a
function of the slot the transaction executed in, while a signed transaction's
account list is fixed when it is signed. Trading refused
`TradingSbfError::Release` `0x4001`. Seven of the fifteen General actions are
window-gated and the other eight are downstream of a batch `OpenBatch` has to
open, so the wall was at the family's entrance. The seed is
`accelerator_caller_authority_digest_v1` now — the digest of the SIGNED family
request and the invocation ordinal, and no trusted-environment scalar reaches
any address seed. See
`docs/design/GENERAL_CALLER_AUTHORITY_SLOT_BINDING_2026_09_03.md`.

**The widths.** Cohort-14's market publishes an `Exact(48)` RentCredit
coordinate. 48 is the width in `account_rules_v3.rs`'s unit-test fixture; the
only RentCredit this protocol produces is `LIFECYCLE_RENT_CREDIT_BYTES_V2` =
128, and the market's own lifecycle RentCredit is 128 bytes on chain. Two more
transcribed widths are wrong the same way and were not yet reached: the
activation cache at 160 against 1,288, and the Core Market at 320 against 368.
The devnet policy file no longer states a single account width — nine of the
eleven are protocol constants and two are functions of the run spec's own
Product graph — so the schema is `dclutch-general-devnet-policy-v2` and a v1
document refuses by schema rather than being read with its widths dropped.

## THE ORDER, and the row cohort-14 did not have

Cohort-14's own order stands, **seal before founding** included. Two things
change.

`01 record-core-digest` is new and it is the only row here that is not a
consequence of the two walls above. It is the host-skew lane's owed step:
`RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1` is empty, and the founding commits
to `sha256(CoreState)` including the Product graph's eight recorded bumps, so a
deployed Core that records them and a compiler that does not know it does is a
fail-closed refusal at the FIRST founding — after the deploy has been paid for.
It costs nothing and it must be committed before step 05 of cohort-14's ladder,
not discovered by it.

`02 refound-general` replaces cohort-14's `06 found-general`. There is no
migration: the widths are inside a founded artifact.

---

### 00 redeploy

Cohort-14's step `01 deploy`, unchanged in method and replaced in content: the
deploy commit must carry the caller-authority seed change, and **a partial
deploy is not available** — `AGENTS.md` permits full redeploys only, every
program from exact current sources with fresh identities and the old cohort
abandoned in place. The seed change is in Trading; the accelerators link
`dclutch-trading-sbf` and change with it.

The verifier is cohort-14's, because nothing about *this* change is checkable
from an ELF by eye. What proves the deployed bytes carry the new derivation is
step `03`, on chain, and nothing before it.

### 01 record-core-digest

`tools/local-validator/bootstrap/successor/src/core_bump_projection.rs` holds
two lists and refuses a Core in neither. Take the deployed Core's ELF sha256
from the deploy journal, add it to
`RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1`, and commit — the constant is
source, so this is a commit, not a flag.

It is a separate row rather than a note inside the deploy because it is the
step whose omission is invisible until money has moved: the refusal is correct,
fail-closed, and arrives at the first founding.

### 02 refound-general

Run `devnet-general-market` with a `dclutch-general-devnet-policy-v2` document
— windows and `token_account_bytes`, and no `external_widths` block at all —
then the ordinary founding campaign with the resulting `market.json`.

The verifier reads the widths back **off the chain**, not out of the file that
produced them: `devnet-general-session` recovers each published external width
by encoding the AccountProfile twice differing in exactly that width, locating
its little-endian offset, and reading the value out of the finalized record.
The RentCredit coordinate must read `Exact(128)`, and 128 must equal the width
of this market's own lifecycle RentCredit account as the chain holds it. Two
authors agreeing is the check; the policy file is no longer either of them.

### 03 openbatch

Cohort-14's step `14 openbatch`, against the re-founded market. This is the
first execution of any General action on any real chain, and it is the only
evidence that the redeployed Trading carries the new seed.

Run `devnet-general-session` first — it is read-only, reads no keypair, signs
nothing, and exits non-zero naming every unsatisfiable conjunct it finds. It
reported both walls in order on cohort-14 deliberately, because an ordering
that prints only the earliest refusal is how a second real wall becomes
invisible. It must now name none.

### 04 route-witness

Every earlier cohort executed real routes on a public chain and none of it
reached the register: `docs/reference/routes.md` printed NEVER-EXECUTED beside
`process_controller_funding_prepare_v1` while three cohorts running had driven
it, because cohort evidence is prose and the register had no channel a devnet
transaction could arrive through. It has one now, and this row is what fills
it.

```
tools/gauntlet/run.sh --mode census                 # produces the inventory, seconds, no chain
python3 tools/gauntlet/devnet-witness/corroborate.py --discover \
  --source docs/evidence/COHORT15_*.md \
  --inventory /private/tmp/dclutch-gauntlet/out/inventory.json \
  --programs <this cohort's label -> program address map> \
  --cohort 15 --out docs/evidence/witnesses/cohort-15-discovered.json
```

It authors nothing. It harvests the signatures out of the evidence document,
asks devnet what each transaction sent, and resolves the outer instruction's
own eight bytes to the census route that dispatches on them — keeping a
resolution only when that route's program is the program the instruction went
to. What it cannot resolve lands in the document's `skipped` list with the
reason, so the gap is a row rather than an absence.

Run it AFTER the evidence document is written, and commit the JSON it produces.
Then regenerate `docs/reference/route-witnesses.md`: a witness document that is
in the tree and not in the register is the same invisible as no document at
all.
