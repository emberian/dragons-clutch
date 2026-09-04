# The cohort runbook

One table, one preflight, one generator. **A new cohort is a manifest in
`cohorts/` and nothing else** — no new directory, no forked checker, no second
copy of nineteen rows that then drift apart.

```
cohorts/15.json      the cohort: deploy commit, program ids, markets, payer,
                     the relay window, the accelerator, the money
steps.tsv            every row any cohort has ever run, each carrying `since`
                     and `until`; {field} resolves against the manifest
check-steps.py       the gate. --cohort N selects; --delta shows what this
                     cohort is the first to run; --prove-frozen shows the union
                     lost nothing
preflight.sh         everything checkable before a lamport moves, for any cohort
generate-stage-scripts.py   the job directory's stage scripts, one family
test.sh              the gate's own red proofs
```

## A cohort is a manifest

```
cp cohorts/15.json cohorts/16.json     # edit: cohort, prior_cohort, job_dir,
                                       # deploy_commit, programs, markets
python3 check-steps.py --cohort 16     # every row resolves, or it refuses
bash preflight.sh --cohort 16 --rpc-url "$(...)"
python3 generate-stage-scripts.py --cohort 16 --out ~/jobs/dclutch-cohort16-.../
```

A row never carries a literal the manifest could carry. `{general_accelerator.
program_id}`, `{prior_cohort}`, `{role_count_word}`, `{direct_fee_basis_points}`
— an unresolved field is a **refusal**, not a rendered brace, because a row that
keeps its own placeholder is a row with no author. `{role_count}` and
`{role_count_word}` derive from the manifest's `roles` list, so the sentence an
operator reads and the loop the generator emits cannot disagree.

## since, until, replaces

`since` is the first cohort a row applies to and `until` the last. A row that
supersedes another names it in `replaces`, and the superseded row's `until` is
the replacement's `since` minus one — the checker refuses if those two ways of
retiring a row drift apart. A `blocks` edge pointing at a retired row follows
the replacement **forward**, so cohort-14's "`activate-general` blocks
`openbatch`" still means something under cohort-15, where `openbatch` was
replaced by `openbatch-refounded`.

## The two frozen runbooks

`tools/cohort14/` and `tools/cohort15/` are **frozen and superseded by this
directory**. They stay until the live cohort-15 lane closes, and while they
stay, `check-steps.py --prove-frozen` is the statement that the union lost
nothing: the cohort-14 view reproduces `tools/cohort14/steps.tsv` and the
cohort-15 **delta** view reproduces `tools/cohort15/steps.tsv`, byte for byte
in their six-column form. Their READMEs hold prose this one does not repeat —
the hazard stories behind most of these rows — and that prose is why they are
frozen rather than deleted.

## What no preflight can answer

Unchanged from cohort-14's README, and worth reading before spending a lamport.
Three of these rows have no verifier that a host can run: `record-core-digest`
is a commit, `route-witness` is only true after the evidence document exists,
and `re-admit` is only true against a chain.

---

## The steps

Ordered as the union orders them. `--cohort N` selects; the id an operator sees
is the row's position in that selection, which is why nothing here is numbered.

### close-prior

Close the previous cohort's programs and reclaim their rent, ids **derived from
that cohort's own keypair files, never transcribed**. The accounts stay on
chain; only the ProgramData holding the code goes away.

### deploy

*Retired after cohort-14; see `redeploy`.* The checked release candidate at the
deploy commit, then one `solana program deploy` per role. Verify each by
dumping the on-chain image back **before the next deploy starts**: a sequence
whose steps spend money must stop at the first failure.

### redeploy

Cohort-14's `deploy`, unchanged in method and replaced in content: the deploy
commit must carry the caller-authority seed change, and a partial deploy is not
available — `AGENTS.md` permits full redeploys only. What proves the deployed
bytes carry the new derivation is `openbatch-refounded`, on chain, and nothing
before it.

### record-core-digest

Put the deployed Core's ELF sha256 into
`RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1` and **commit it before any
founding**. The constant is source, so this is a commit and not a flag. It is
its own row because its omission is invisible until money has moved: the
refusal is correct, fail-closed, and arrives at the first founding.

### ladder

`campaign --through activation`, campaign payer funded first. Re-observe from
the cluster afterwards and never trust an exit code.

### accelerator-release

`prepare` with the General accelerator group. The Registry **finalizes** the
artifact release, and that finalization is the deployment observation.

### seal

Key-free, read-only, **before any founding**. Cohort-12 founded first and
stranded its market. The five owned roles must preflight `equal:true` against a
fresh finalized observation.

### found-direct

The staged sponsored market, founded from the **sealed** plan with a founder
key whose custody is proven. Burning the founder's complete set is the only
route to retirement and to the collateral, so a founder nobody holds strands
the principal permanently.

### found-general

*Retired after cohort-14; see `refound-general`.*

### refound-general

`devnet-general-market` against a policy document with **no `external_widths`
block**, then the ordinary founding campaign. The verifier reads the widths back
off the chain, not out of the file that produced them.

### activate-direct

The verdict string, **not the exit code**, is what says this ran: cohort-13's
first `--execute` printed "planned" and exited zero. The root is read back from
the command's own report, never from a remembered address.

### activate-general

Same shape, General side; the report carries its own schema and the root it
named in advance must be occupied.

### arm-relay

Arm at **founding** time, not resolution time, and prepay the settle seat: the
terminal route allocates and assigns the certificate but never funds it, and
that rent is a caller obligation.

### admissions

Fund the participants so they pay their **own** PDA rent — a fee payer is
writable unconditionally and a Position owner must sign readonly, so they can
never be the same key.

### fill

The smallest fill whose fee does not floor to zero, taken on purpose and
settled in a second transaction. The measured CU is recorded beside the
previous cohort's.

### fee-settlement

Permissionless, and nothing economic is passed in. `fee_owed` reading zero off
chain afterwards is the only thing that distinguishes a settled fee from a sent
transaction.

### census

The complete aperture is taken **before the first boundary**. An `INAPPLICABLE`
is not a pass.

### openbatch

*Retired after cohort-14; see `openbatch-refounded`.*

### openbatch-refounded

Against the re-founded, activated General root. Run the read-only session
report first: it signs nothing and exits non-zero naming **every** unsatisfiable
conjunct, because an ordering that prints only the earliest refusal is how a
second real wall becomes invisible.

### relay-capture

Fire inside the market's own on-chain window or exit. The wait is bounded by a
ceiling it **refuses** rather than sleeps past, and the guard is the last
statement before the action and exits rather than falling through.

### relay-settle

The same shape, strictly after `end + max_age`. The settle has no upper
deadline, so its loop is bounded by its attempt count alone.

### admit-terminal

A journal: one durable action per pass, rerun to advance. An expired **planned**
entry is preserved under a dated name and the next pass plans fresh; an entry
in any other phase is left exactly where it is.

### payout

Into the owner's own ATA. Done when the evidence names a finalized payout, not
when a status field says so.

### funded-rent-recorded

The rent an account was funded at is a fact fixed when it was funded, and from
cohort 16 the funding ledger's header carries it: the four bytes that were
reserved now hold the exemption-scaled rent rate -- lamports per byte-year times
the exemption threshold -- that the cluster charged when the founding created
the account. Every exactness check downstream prices `(128 + len) * rate` from
that record instead of re-deriving from the Rent sysvar of the moment. This row
exists because cohort-15 learned the cost of not having it: devnet moved the
rate from 6,333 to 5,080 at the epoch-1141 boundary mid-cohort and stranded
three walls, each refusing by exactly the rate difference times the account's
own footprint, on accounts nobody had touched. Run this immediately after the
census and before `admit-terminal`, because a zero here means the deployed image
predates the schema.

A zero is not a dead end for a cohort already on chain. The rate a ledger was
funded at is recoverable from the ledger's own bytes --
`rate = (lamports - remaining native principal) / (128 + len)`, accepted only
when the division is exact and only when every sibling account of the same
founding derives the same rate -- and
`dclutch-resolution-core-v3-operator::funded_rent_recovery_v1` is the host that
does it. The recovery is for a HOST planning against accounts that already
exist; the programs still fail closed on a zero, and a founding still records
what it paid. Cohort-15's own ledgers recover 6,333 and are corroborated at five
widths by five accounts of the same founding.

### retire

The last unrun step in the runbook, and the reason it was never written down:
until cohort 16 no market had reached it. It is four checkpointed packets, not
one transaction -- the onchain checkpoint is the route owner and each packet
keeps its own durable journal, so a crash resumes rather than inventing a second
transaction identity. Rerun until every journal reads finalized. Cohort-15's
market 1 got as far as phase 3 `Retiring` and stopped five rent-exactness guards
deep; those five now price against `funded-rent-recorded`'s figure, which is
necessary and is not sufficient.

**One wall is still open and it is not a refusal.** `ResolutionCloseFund` --
which the terminal sequence must land before this row's first packet, because it
is what takes `outstanding_capabilities` to 0 -- exceeds the default compute
budget. Driven on devnet 2026-09-04 against cohort-15's market 1 it consumed
**200,000 of 200,000 compute units and returned `ProgramFailedToComplete`,
"exceeded CUs meter at BPF instruction"**. The terminal sequence declares no
`ComputeBudget` prefix at all -- the only driver in this tree that does not,
while the wallet payout path routinely spends 235,003 CU -- and
`authenticate_terminal_message_decompilation_v1` pins the durable message at
exactly ONE instruction, so the prefix is a change to the durability schema, the
v0 placement authenticator and the completion expectations together, not a flag.
Until it lands, no market can reach this row, and that is why retirement has
never completed on any chain. The four packets themselves are drivable: they are
808, 864, 864 and 744 bytes against a 1,232-byte packet, and the DEPLOYED Core
routes all four (`Action::Retire` at
`RETIREMENT_CHECKPOINT_PREPARE_INSTRUCTION_BYTES_V1` for the prepare, and the
three suffix magics at `dclutch-core-sbf/src/lib.rs:392`). The 2,152-byte
`RETIREMENT_INSTRUCTION_BYTES_V1` aggregate route is the legacy builder's and
nothing in this row submits it.

### route-witness

Harvest the signatures out of this cohort's evidence document, ask devnet what
each transaction sent, and resolve the outer instruction's own eight bytes to
the census route that dispatches on them. It authors nothing. Run it **after**
the evidence document is written, commit the JSON, and regenerate the register:
a witness document in the tree and not in the register is the same invisible as
no document at all.

### re-admit

For a cohort whose recorded admission pins a gate digest no rebuild can
reproduce. Host-side, key-free, moves no lamports. The verifier is not that
`prepare` exits zero: the rebuilt gate digest must equal the one the deployment
set now pins, each role's `checked_candidate_elf_sha256` must equal the
ProgramData ELF digest **read off chain**, and the produce command must reach
its ticket checks from a job directory naming no path under the deleted scratch.
