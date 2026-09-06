# The cohort runbook

One table, one preflight, one generator. **A new cohort is a manifest in
`cohorts/` and nothing else** — no new directory, no forked checker, no second
copy of nineteen rows that then drift apart.

```
cohorts/15.json      the cohort: deploy commit, program ids, markets, payer,
                     the relay window, the accelerator, the money
steps.tsv            every row any cohort has ever run, each carrying `since`,
                     `until`, its `shape` and its `args`; {field} resolves
                     against the manifest, {market.field} against each market
check-steps.py       the gate. --cohort N selects; --delta shows what this
                     cohort is the first to run; --prove-frozen shows the union
                     lost nothing
preflight.sh         everything checkable before a lamport moves, for any cohort
generate-stage-scripts.py   the job directory's stage scripts, one family, from
                     the shape and args columns — no hand-written script
semantic-release-ids.py     the cohort's eight semantic release ids, derived
                     from the SHIPPED ARTIFACTS into <job>/semantic-release-ids.txt,
                     which every emitted `prepare` reads through its own
                     `semantic` helper; `validate_prepare` re-derives each one
                     from the artifact beside it and refuses a mismatch
frozen/              the exact tables cohort-14 and cohort-15 ran from, as
                     fixtures; --prove-frozen proves this file still reproduces
                     them
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

## shape and args: the 33 hand scripts become two columns

Cohort-15 ran from 33 hand-written stage scripts naming 82 absolute paths and
**134 flags the rows did not carry**, in four structural shapes. All of that is
now data:

- **`args`** is the row's invocations, ` ;; ` between them, each starting with
  its driver — `bootstrap` (the job's own successor binary, keyed endpoint),
  `bootstrap-public`, `bootstrap-offline`, `solana`, `script <repo path>`,
  `simulator`, `sh`. The 134 flags live here now, and a flag naming a fact is a
  `{field}` that resolves against the manifest — never a literal.
- **`shape`** is the four structural shapes the hand scripts open-coded, named:
  `once`, `per-role`, `attempts` (the plan-then-sign loop with a fresh output
  per attempt), `wait:capture` / `wait:settle` (the bounded wait against the
  market's own schedule, the guard that EXITS), `journal` (rerun one durable
  action per pass until the file it names exists), `commit`, and `-` for a row
  whose args are not captured yet.
- **`blocks`** was peer-chaining — a script grepping another's log for
  `SETTLE_LANDED`. Now every emitted script refuses at its first line until each
  row that blocks it has left a `GREEN` marker, and writes its own only when its
  last invocation exited zero.

Inside `args`, `@roles` / `@owned_roles` / `@participants` repeat an invocation,
`?` skips one whose output exists, `*` marks the looped act, `{market.x}` binds
each market, `{pubkey:keys/x.json}` becomes the key's address at run time, and
`{stage:key}` is another row's output directory. A row that names `{market.x}`
is emitted **once per market** of the kind its stage implies; a market fact the
manifest does not carry is refused BY NAME with nothing left on disk, so an
operator records it and regenerates. The generator emits **0 absolute paths and
0 credentials** and refuses to leave a job directory that would carry either.

## since, until, replaces

`since` is the first cohort a row applies to and `until` the last. A row that
supersedes another names it in `replaces`, and the superseded row's `until` is
the replacement's `since` minus one — the checker refuses if those two ways of
retiring a row drift apart. A `blocks` edge pointing at a retired row follows
the replacement **forward**, so cohort-14's "`activate-general` blocks
`openbatch`" still means something under cohort-15, where `openbatch` was
replaced by `openbatch-refounded`.

## The two frozen tables

`tools/cohort14/` and `tools/cohort15/` are **gone**: their `steps.tsv` files
are kept as fixtures under `frozen/cohort-14.tsv` and `frozen/cohort-15.tsv`,
and `check-steps.py --prove-frozen` is the standing proof that this directory
still reproduces exactly what those two cohorts ran — the cohort-14 view
reproduces `frozen/cohort-14.tsv` and the cohort-15 **delta** view reproduces
`frozen/cohort-15.tsv`, byte for byte in the six-column form the hand scripts
were driven from. That proof is why the `shape` and `args` columns could be
added and the two directories deleted in the same breath: adding a column, or a
`since 16` row, is proved to have changed nothing about what already ran. The
hazard stories those READMEs held are now the `### key` prose below, one author
per row.

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

### close-prior-accelerator

The seven roles are not the whole of a cohort's footprint. From cohort 16 the
accelerator is a link this cohort BUILDS and DEPLOYS, so the one the previous
cohort pinned is superseded the moment the new one lands, and its ProgramData
rent -- 1.91 SOL for cohort-15's `8pgnyNvg...` -- is the cohort's own money.
Its id is read from the previous cohort's manifest, never transcribed, for the
same reason `close-prior` derives the seven from that cohort's keypair files.

Closing it is also the fail-closed half. A Program account survives its
ProgramData, so a market founded against the superseded accelerator meets an
account that is not executable rather than an accelerator that quietly answers
with pre-fold semantics.

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

### redeploy-named-builder

`redeploy`, with the reproduction clause repaired. Until cohort 16 the verifier
said *"reproduced byte-identically from a second detached worktree"*, which does
not say **same host** — and run across two hosts it was guaranteed to fail, for
a reason that has nothing to do with the deploy. Two worktrees on one machine
are the **build-path control**: they prove the absolute path is not an input,
and they prove nothing about reproduction.

Reproduction is a **second host running the named release builder artifact**:
platform-tools v1.53 on `Linux/x86_64`, whose supported builders are
`hbox-through-swarm-build`, `persvati`, and `linux-x86_64-container` — any other
machine running that artifact in a `linux/amd64` container. A native macOS build
of the same commit differs in nine of ten roles and cannot be made to agree; the
measurement and its two causes are `docs/runbooks/COLD_MACHINE_2026_09_03.md`
§10, and `tools/release/README.md` under "One builder artifact" is the standing
statement.

### deploy-accelerator

**The eighth link, and until cohort 16 nothing deployed it.** `prepare` has
always OBSERVED an accelerator and published its `ArtifactRelease`; through
cohort-15 that accelerator was deployed by a separate one-off job and the
runbook carried no row for it, which was invisible while the accelerator was
`dclutch-general-accelerator-sbf` and unchanged for three cohorts. The fold made
it `dclutch-accelerator-sbf` -- General, Dealer and series-shadow in one program
-- so the cohort that first ships the fold must also be the cohort that deploys
it, and a runbook with no row for that is a runbook with a step nobody owns.

It is its own row rather than an eighth `roles` entry because the deployment-set
journal owns exactly the seven checked roles and names no accelerator: the
successor's `prepare` says so in its own usage, and the `--general-accelerator-*`
group is legal beside the journal precisely because the journal can neither
supply that publication nor contradict it. Putting the accelerator in `roles`
would emit `--accelerator-program-id` into a command that has no such flag.

Same discipline as `deploy-roles`: dump the live image back immediately and
compare it to the candidate ELF over the ELF's whole length, before anything
observes it. What this row produces that nothing else can is the accelerator's
deployment slot, which `prepare` observes, the founding pins and the Registry's
own observation fixture transcribes.

**So `preflight.sh` is run TWICE for such a cohort, and its first pass is RED
on purpose.** The accelerator's slot, ELF digest and liveness are outputs of
this row, so the manifest carries them empty until it has run and §3 of the
preflight cannot be green before it. That RED admits the deploy and forbids the
founding, which is the honest reading and is why it is not softened; the
message names this row so the second pass is a step and not a puzzle. Record
the slot, the digest and the Registry pin text in `cohorts/<n>.json`, run the
preflight again, and found only from a fully green one.

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

### refund-scale

Author every categorical `ProductBasisV3` this cohort founds at
`payout_scale = basis_width - 1`, its ordinary-region count, and **commit the
records before any founding**. Like `record-core-digest` this is source rather
than a flag, and it is its own row for the same reason: the legacy `1` founds
without complaint and is only visible later, as the shape of a failure nobody
has had yet. At scale 1 an oracle outage pays the whole failure column to a
single holder; at `basis_width - 1` the same outage refunds every ordinary
claim. The verifier reads the founded market's authenticated basis back off the
chain at `BASIS_PAYOUT_SCALE_OFFSET_V3` rather than trusting the record that
produced it.

### found-direct

The staged sponsored market, founded from the **sealed** plan with a founder
key whose custody is proven. Burning the founder's complete set is the only
route to retirement and to the collateral, so a founder nobody holds strands
the principal permanently.

### found-general

*Retired after cohort-14; see `refound-general`.*

### refound-general

*Retired after cohort-15; see `found-general-family`.* `devnet-general-market`
against a policy document with **no `external_widths` block**, then the ordinary
founding campaign. The verifier reads the widths back off the chain, not out of
the file that produced them.

### found-general-family

`refound-general`'s command, under a claim it could not make: this is a **fresh
founding, not a re-founding**. Cohort-15's General market cannot be repaired
under cohort-15's deployed programs, and that was measured rather than reasoned
— the family policy handed to the account-profile contract as it was built at
`90de010aa` is refused by `StateLifecyclePolicyV3::decode` with
`InvalidRentQuote`, before any profile join and before any action is selected,
because the deployed rule orders rent quotes by destination alone and
`InitializeSettlement` and `PlaceOrder` quote the same four registers. The
change is in the SBF link closure and not only in the host compiler, which is
what makes it a cohort and not a re-run.

What the verifier reads is the agreement the family exists for: the fifteen
published action descriptors carry **one** `derivation_policy`, equal to the
digest of the single family lifecycle policy the release publishes, and the
manifest entry's `child_derivation_id` equals it — so every action selects the
entry the root was activated under.

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

### sim-config

Derive the simulator's config from the founding's **own** records. Nothing here
is transcribed: every address comes out of `campaign-open.json`'s accounts map,
and the routing address lookup table comes from the founding's own
`create DCLTGMF3 frozen routing address lookup table` transaction — one
`getTransaction`, then the account is authenticated (frozen, and routing this
founding's market) before any driver sees it. Cohort-16.1 stopped at
`admission message compilation: PacketTooLarge` because no row produced this
file and the table is the only thing that makes the admission packet fit.

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

*Retired after cohort-15; see `openbatch-two-pass`.* Against the re-founded,
activated General root. Run the read-only session report first: it signs
nothing and exits non-zero naming **every** unsatisfiable conjunct, because an
ordering that prints only the earliest refusal is how a second real wall
becomes invisible.

### openbatch-two-pass

The same act, in the two passes the caller-authority span needs, without
`--rent-credit`, and with `--action` said out loud.

**Two sessions, one row.** Trading seeds every admitted caller-authority PDA
with `accelerator_caller_authority_digest_v1(Admitted, family_request_digest,
index)`, and that digest does not exist until a plan has been produced from a
route. So the first pass emits a probe route, `general-successor-plan-v5` turns
it into a plan, and the second pass re-emits the route at the plan's own
`familyRequestDigest`. A route whose span came from the session's probe default
names four addresses no execution can derive; cohort-16.1 ran both passes by
hand and recorded the loop as undocumented.

**No `--rent-credit`.** The Market's `rent_beneficiary` is that account's one
author. Cohort-16.1 passed the PREVIOUS General market's credit, the session
copied it into the route without joining it to anything, the frame reported no
wall, and OpenBatch refused `TradingSbfError::Content` on chain at 239,473 CU
inside `authenticate_lifecycle_credit_v3`. The flag survives as an optional
cross-check that refuses when it disagrees with the Market.

### openbatch-frozen-tables

*Replaces `openbatch-two-pass` from cohort-17.* The same two passes, plus the
two frozen lookup tables they need, in the order cohort-16.1 measured.

**A table per digest, and the route re-emitted onto it.** `openbatch-two-pass`
ordered the lookup table AFTER its second session, and that cannot work: the
four admitted caller-authority PDAs are IN the canonical address set the table
must hold, so a route emitted against the founding's frozen table names a set
that table does not contain and `general-successor-plan-v5` refuses it by name,
`General v0 compilation: LookupTable`. Cohort-16.1 measured both halves --
`devnet-general-lookup-table-v1`'s dry run over the probe route lists all four
PDAs and the Market's own rent beneficiary among its 53 addresses -- and ran the
five acts by hand. So each pass is: session against the founding table, freeze a
table over that route, re-emit the session onto the frozen table, plan. The
digest is invariant to the table and to the digest passed in, which is what
makes the loop terminate at two passes rather than diverge.

Both lookup-table acts are `?`: the driver refuses to overwrite its own
evidence, so a resumed stage reuses the table it already paid rent for.

**It needs a fresh General founding, and it needs no Trading redeploy.**
Cohort-16.1's `OpenBatch` reached the accelerator -- the first on any chain --
and refused `TradingSbfError::Transition` `0x4004` on the accelerator's ack,
cause `GeneralAcceleratorSemanticErrorV3::ConfigMarket`: the General
AccountProfile projected the Portfolio's `claim_basis_id` @96 into
`SEMANTIC_BASIS_ID` while the config binds
`semantic_basis_identity_v3(linked_basis)`, the liability basis @128. The rule
now projects @128.

WHERE THAT RULE LIVES WAS MEASURED, because the obvious answer was wrong. Two
commits differing only in that one constant build a **byte-identical**
`dclutch_trading_sbf.so`, `65ff376e876e3398d4e438171a955314202c9d3c7a194a9035231f203ac0c596`,
and a byte-identical accelerator, `d2ff2b87ac79e5329b83f10e10a2f2d9b03ac16d445d070188f03946598da91a`,
while their campaigns disagree completely. `programs/dclutch-trading-sbf` names
`account_rules_v3` nowhere: Trading INTERPRETS a profile artifact presented as an
account. The operator encodes that artifact
(`general_selected_release_v1.rs:1166`) and binds `digest(account_profile)` into
the per-action `CapabilityProgramV4` descriptor (1216-1218), which the founding
pins as its manifest entry. So the fix moves the published artifact, the
descriptor and the entry, and moves no link: a market founded at the old entry
cannot select the new profile, and no market needs its Portfolio touched --
the live record already carries the config's value at @128.

### close-batch

*Cohort-17.* CloseBatch at the batch's own `collection_close_slot`, through the
same two-pass session. **Owed before it can run**, and the refusal says so by
name: `devnet-general-session --action close-batch` exits
`session/action-not-composable`, because CloseBatch's subject is the open
Batch's own body and the session derives its subject from the capability root
alone. The bundle builder already composes CloseBatch, so the missing author is
the devnet session and nothing deeper.

### second-open-batch

*Cohort-17.* The first transaction that exercises the per-batch selection: two
Batch accounts at two addresses with two occurrence ids, which no single
OpenBatch can show. It needs `close-batch` first, because the root admits one
open batch at a time.

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

**`ResolutionCloseFund` HAS NOW EXECUTED, and the wall moved rather than
closing.** It is what takes `outstanding_capabilities` to 0, so the terminal
sequence must land it before this row's first packet. It used to exceed the
default compute meter -- 200,000 of 200,000, `ProgramFailedToComplete` -- because
the terminal sequence declared no `ComputeBudget` prefix at all. It declares one
now: the durable message carries exactly one first-party instruction optionally
preceded by exactly one `SetComputeUnitLimit` for the recorded budget, and the
budget is 267,518, derived from a measured 252,518 under `CU_BUDGETS.md`'s
tolerance rule. Driven on devnet 2026-09-04 against cohort-15's market 1 the
route consumed **252,368 of 267,368 compute units and succeeded** --
`3rDH7V5XoHDPwZEzfoCi6f4mWYaWR3ZrDnjuUuKi1hAjMCqrEttPJNzT4aQRwF2ePYJqnzw6ftADhJYrnMs3cXin`,
slot 493,003,631.

**`ResolutionCloseFund` IS ALSO CERTIFIED NOW**, and the wall moved again. Its
receipt's three disagreeing `u64`s were two questions and both are answered.
`ledger_rent_lamports` and `ledger_lamport_surplus` are one partition whose sum
is invariant: the deployed program prices the closing ledger from the Rent
sysvar of the moment and the host was pricing from the rate the account was
FUNDED at, so the second question now gets its own author, keyed by the
Resolution role's checked candidate ELF digest in
`closure_receipt_projection.rs`. `closed_at` is the Clock at EXECUTION against
the Clock at PLANNING and cannot be predicted at all, so the poststate model
admits one interval-bound field -- lower bound the plan's own observation clock,
upper bound the sequence's `TERMINAL_FINALITY_WAIT`, every other byte still
exact. Market 1's journal reads phase `finalized`; the gap was nine seconds.

**The open wall is now stage FOUR, `DirectCloseCapability`, which is what takes
`outstanding_capabilities` to 0, and it is a founding input rather than a
producer.** `CapabilityFundingHeaderV2::new(physical_count 2, logical_count 1,
mask 0b1)` refuses at
`crates/dclutch-operator/src/terminal_retirement_v1.rs:699`, because a header
counts physical ledgers whose disjoint subsets COVER the logical entries and
market 1's manifest declares NO dependency edges: all four entries have
`dependency_count 0`, so the Direct entry's closure is a singleton and can never
cover the Resolution compartments its close frame preserves. Behind it sits a
second fact -- stage three closed the Resolution funding ledger that stage four
decodes and preserves -- which is a question about which stage owns those
lamports, not a typo. **A cohort that intends to retire must found a manifest
whose Direct entry declares its Resolution dependencies.** Until then no market
reaches this row, and that is why retirement has never completed on any chain.
The four packets themselves are drivable: they are
808, 864, 864 and 744 bytes against a 1,232-byte packet, and the DEPLOYED Core
routes all four (`Action::Retire` at
`RETIREMENT_CHECKPOINT_PREPARE_INSTRUCTION_BYTES_V1` for the prepare, and the
three suffix magics at `dclutch-core-sbf/src/lib.rs:392`). The 2,152-byte
`RETIREMENT_INSTRUCTION_BYTES_V1` aggregate route is the legacy builder's and
nothing in this row submits it.

**A SECOND wall sits behind that one, and it is a program change rather than a
founding input: a market founded REFUNDING cannot reach this row at all.**
Decision 0025 seats the failure coordinate in an escrow Position whose owner is
a program-derived address with no key. No certificate pays that column
(`runtime_v3.rs:972`, `:990`), so terminal settlement drains every ordinary
claim and the Hoard and leaves it standing; the closure's own conjunct then
refuses `ClaimsMarketClosureSbfErrorV1::Liability` on it
(`programs/dclutch-claims-sbf/src/market_closure_v1.rs:656-668`) and the
operator hoists that conjunct to the `BeginRetiring` preflight, so no
transaction is ever built. Cohort-16.1 is in exactly this state on devnet: the
escrow Position `7FQCfc4RrrsATEe969eNVYoLjDukmBVKMAxM1yg7AzcQ` holds
`[0, 0, 0, 166666667]` at revision 1 and has never moved.

The repair is decision 0025's shape A: **closure BURNS that column** -- the
aggregate's supply and the escrow's Position debited by the same amount at that
one coordinate -- and closes the escrow's Position and admission alongside the
aggregate, rent to the market's RentCredit like every other closed account. It
is not a relaxed supply check: `protocol_position_v2.rs:608` refuses to close a
Position with any nonzero balance, so a closure that merely tolerated the column
would strand the escrow's two accounts and leak their rent. The burn must
precede the escrow's close and the close must precede checkpoint 1's handoff,
because the handoff reassigns the aggregate to Core and no Position of that
market can be closed afterwards.

**IT SHIPPED at `7d45d6ba3` (lane PROGRAMS-17E, 2026-09-06), and this row is
reachable by a refunding market once the cohort carries both new links.** The
closure's frame gains three TRAILING accounts -- the escrow's Position, its
protocol-Position admission, and the Market's linked `ProductBasisV3` record --
so 11 becomes 14 (12 becomes 15 with the Registry continuation); Core's
checkpointed retirement frame goes 35 to 38 and carries them on ALL FOUR packets,
because `aggregate_retirement_journal.rs` requires one frame per retirement. A
CATEGORICAL retirement is byte-for-byte the thirty-five-account one that shipped
and its four packet extents do not move: no request byte changed.

What this row must supply, and where each address comes from:

- the escrow Position and its admission:
  `dclutch_claims::protocol_position_v2::failure_escrow_v1`, derived from the
  Claims aggregate alone (its owner is the program, its header carries the
  logical Market and the runtime width);
- the linked basis record: the address the FOUNDING producer used. It is not
  derivable from the aggregate, and it is not re-derived by a second hand
  either. `market.rs`'s `publish_market_records` publishes it under
  `GRADED_BASIS_RECORD_SCHEMA_ID_V3` and writes it into the founding evidence as
  `linked_liability_basis_record`; the retirement path reads it there through
  `routed_record`, which recomputes the address from the digest the producer
  stored and refuses a report whose row does not reproduce it. **Threaded at
  `42c3bb931` (lane HOST-RETIRE, 2026-09-06)**, along with the journal's shape:
  `aggregate_retirement_journal.rs` spelled 35 accounts, 36 protocol-and-payer
  keys and 37 resolved keys as constants, so this row would have refused a
  refunding retirement at "retirement operator changed account or data width"
  before it reached a chain. It now names two shapes and derives every width
  from the operator's two account counts.

Nothing about this row is validator-run yet. Both walls above are host-side and
closed; what stands between them and a Retired refunding market on any chain is
a cohort founded on links that carry the burn.

The cost, restated: two ELFs move, so cohort-17 is a full re-release of every
program plus a re-found under decision 0012, with frame-baseline rows for both
links (carried at `7d45d6ba3`, admitted at `b1fe0193d`). **Cohort-16.1 cannot be
repaired in place** -- its Claims and Core are the old ELFs and decision 0012
forbids upgrading a founded market's release set -- so its escrow Position
`7FQCfc4Rrrs…` stays where it is and its market stays unretired. The evidence
that the route works is `a_refunding_market_retires_once_the_closure_burns_its_
failure_column` in `crates/dclutch-svm-harness/tests/market_retirement_v1_
lifecycle.rs`, which is ProgramTest against real ELFs and not validator evidence
(`tools/gauntlet/TIERS.md`).

The preflight says the wall by name rather than instructing a payout nobody can
produce (`crates/dclutch-operator/src/wallet_terminal_input.rs`, lane
PROGRAMS-17C, 2026-09-05); its fifth arm's sentence is now a statement about
which release set a market was founded on rather than about a route that does not
exist.

### found-two-source

The first cohort market that buys a second answerer. Decision 0027's funded
ordered ladder has been walked end to end on real ELFs since `beca9243e` and
answered on its second rung since the same commit, and until cohort 16 no
producer outside a program test could found one: `recovery_policy_hex` was
written empty by every caller there was.

What the flag buys is one prepaid Resolution compartment per rung on top of the
two every market already has, and one genuinely different source. For a
Pyth-backed feed "different" has exactly one axis — the adapter's tolerance for
the provider's own stated confidence interval — so a rung is the same feed at
the same exponent admitted under a TIGHTER bound. A market whose first choice
went silent has a reason to demand a better-conditioned reading from its second,
and the bound is capped at 10,000 bp, so tighter is the only direction there is.

The rung's lifetime is stated as seconds AFTER the leg before it, because the
primary leg's deadline is the live window's close plus its submission-latency
budget and is computed inside the producer. State it generously: the whole point
of the rung is to be answerable after the primary was not.

### crank-ladder

The permissionless crank, run once per leg. `AdvanceRecovery` is a 32-byte
instruction naming only the generation and the terminal sequence; which rung,
which source and when it expires are read by the program off the market's own
state, so the driver is a frame builder and a bounded wait rather than a
decision.

**It is admissible STRICTLY after the leg's deadline.** The last second an
honest observation may land and the first second a crank may run are different
seconds, and the driver refuses the earlier one by name before a lamport moves.
`--wait --max-wait-seconds` sleeps to it through one bounded wait against the
chain's own clock and refuses a target further away than the stated ceiling; it
never warps, because a crank cannot be brought forward, only waited for.

The crank pays whoever runs it, out of the compartment the rung it enters names.
That is the property worth checking on devnet rather than in a bank: a route
nobody is paid to run is a route nobody runs, and cohort-13's silent window is
what a market with no such route costs its holders.

### capture-rung

The other outcome, and the one that makes a ladder worth buying. Its sibling
`crank-ladder` proves the ladder can be WALKED; walking it to the end reaches
the same pre-disclosed failure a market with no policy gets for free. This row
is the market being ANSWERED by the alternative it paid for.

The capture is the ordinary flagship submit/execute pair with two additions: the
input names the `RecoveryPolicyV2` record pair, and the three finalized-record
positions carry the rung's own source instead of the primary's. The market's
`active_attempt` is the authority for which source may speak — a request naming
a rung the market has not reached refuses `SourceLadder` rather than being
joined against the wrong feed — so nothing about this row is a second opinion
about which leg is live.

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

## The cohort-16 rows: prepare moves before the ladder

Cohort-16 is a full redeploy that founds a refunding market, buys a second
source, and retires the first. Five rows are new and two of the old founding
rows are replaced, and the change is one of ORDER: on a genesis cohort the
checked candidate is installed directly, so `prepare` can run before the ladder
rather than as a step of it. `--prove-frozen` proves cohort-14 and cohort-15 are
untouched by any of it.

### deploy-roles

`redeploy`'s content as an emitted stage: one `solana program deploy` per role
from the candidate's `elf/`, each image dumped back and compared **before the
next deploy starts** — a sequence that spends money stops at the first failure.
Replaces cohort-15's ladder-embedded deploy.

### prepare

`accelerator-release`'s successor, moved ahead of the ladder. Observe every
deployed role's ProgramData and the accelerator's, then `prepare` with the role
groups and the accelerator group, deriving the seven semantic release ids from
the ELFs rather than typing them. The Registry finalizes the accelerator record
during the ladder, which is the deployment observation — so `prepare` names it
and `administration` proves it.

### fund-payer

Capitalize the campaign payer — a DISTINCT keypair from the deployer, because
the founding's fee payer is writable while the consenting authority is readonly
— before the ladder runs on it.

### administration

`ladder`'s successor: `campaign --through activation` from `plan.json`, the
payer funded first. A second preflight that READS THE CLUSTER is the verifier,
never the driver's exit code.

### checked-execution-release

The five-role checked execution release, minted from the sealed plan **before
any fill**: the produce command and the fee settlement both require it, and a
cohort that founds without it is reachable by neither.

### seal-general

`devnet-capability-seal-v1` from the General session's frame report, over a v0
routing table with the extended heap the `DCLTSEL1` profile admits. The builder
derives the seal address from the four seeds and refuses a frame naming a
different one; the address it lands at must equal the coordinate-38 address the
session stated in advance — two authors, one address, neither told by the other.

### refund-scale-seated

*Cohort-17.* `refund-scale`'s successor: from this cohort the founding SEATS the
failure column rather than issuing it, so the founder pre-funds the escrow
Position and its admission at the derived addresses. The page must print that the
failure column is seated in the market's escrow, not held by the founder.

### manifest-edges

Author the selected trade capability's manifest **dependency edges**, and commit
them before any founding. The selected entry names every other manifest index,
ascending — three edges in a four-entry manifest.

The edge set is derived, not chosen. Retirement's stage four is the production
`F=2` Direct close, and its frame carries two physical funding ledgers: the
Resolution-owned `0b0111` dependency ledger, preserved, beside the Trading-owned
`0b1000` selected one, closed. `validate_funding_ledger_masks_v2` requires those
two masks to be a disjoint partition of the funding header's required union, and
the union is the selected entry's dependency closure — so the union can only be
every bit, and a closure reaches every bit only when the selected entry names the
other three.

It is source rather than a flag for the same reason `refund-scale` is, and it is
stricter than any other founding input: the capability-manifest digest is a
Market-PDA seed. A wrong array does not misconfigure a market, it founds a
different one, and nothing later can repair a market founded without the edges.
Cohort-15 and cohort-16 both founded `dependency_count 0` at every entry; their
markets can be filled, settled, captured, paid out and taken to Terminal, and
none of them can ever be Retired.

**This row was blocked on a release change, and the release change has landed.**
Cohort-16 measured the block on 2026-09-05: the edges make the selected entry's
closure every bit, every route that consults the closure needs a two-ledger
frame, and a market founded with edges refused activation in the deployed
Trading program at `TradingSbfError::Content` (`0x4003`), 108,180 CU. The
reading at the time was that the Direct activation bundle had to declare three
accounts and that the release id, the manifest entry and the Market address
would move with it. **That reading is reversed**
(`docs/evidence/COHORT16_DEPLOYED_SEALED_2026_09_05.md`, second addendum): a
three-account activation profile cannot be encoded at all — `AccountProfileV1`
refuses `UnanchoredAccount` for a rule no seam-seeded identity can anchor, and
none names a foreign controller. The defect was in the Trading outer, which
composed its interpreted frame out of every physical ledger; it now composes the
root and the selected ledger, exactly as the native-close route already did, and
authenticates dependency ledgers outside that frame.

**So this row needs a Trading link carrying that fix, and nothing else.** No
artifact byte moves: the Direct release id, the manifest entry and the Market
address are unchanged, and a market already founded with the edges — devnet
`GyD95eyE…` — activates at the new link without re-founding. A cohort whose
Trading link predates the fix still founds markets that are Open and
unactivatable if it takes this row, and markets that can never be Retired if it
declines. Cohort-16 has one of each, and only the Trading link stands between
its edged market and activation.

**Which cohort carries it is a real choice with a cost either way, and the
runbook does not make it.** A full redeploy with fresh identities — cohort
**17** — is what the standing devnet authorization admits without further
argument, and it re-founds everything, so `GyD95eyE…`'s survival buys nothing.
A Trading-only upgrade in place — **16.1** — keeps that market's address and is
the only way to reach the market that already carries the edges, but upgrading
a program moves its Loader slot, which supersedes the release generation under
decision 0012.

**And a re-release does NOT re-pin a market that already exists.** This was
ruled the other way and measured on 2026-09-05
(`docs/evidence/COHORT161_UPGRADED_SEALED_2026_09_05.md` §1): a market pins the
EXECUTION RELEASE SET — `Market.release_set_id`, offset 208, written only by
`initialize_market` into an all-zero account and by nothing else, ever — and
that id is `sha256` over five `(program_id, artifact_release_id)` bindings whose
records carry each role's ELF digest **and its deployment slot**. Any `Upgrade`
moves it, and so does redeploying identical bytes, because Loader V3 writes the
current slot and refuses an `Upgrade` in the deployment's own slot. The
activation cache is derived from the set id, and `lineage_walk` — the one
forward mechanism — has no consumer in any capability program. **So upgrading a
link strands every market founded before it**, and the re-release is what lets
the NEXT market found and execute, not a repair of the last one. Cohort-16.1
took that path with its eyes open: `GyD95eyE…` could not be activated at
cohort-16's release either, so it had no future to lose.

The row is satisfied by either path; the operator states which and why in the
cohort's own evidence.

**The in-place path has no rows of its own yet, and three things block writing
them**, all measured in cohort-16.1 and all in that document's §6: nothing binds
a completed Upgrade row's receipt and dump into the deployment-set journal (the
only journal writer is the AlreadyCurrent row); the checked-upgrade phase loop
re-audits all seven roles per phase, so its blockhash-to-send gap is ~44 s
against a devnet that was running a 24.5 s window, and only the Upgrade has an
escape (`--adopt-existing-buffer` with `--adopt-finalized-cli-upgrade-signature`);
and the Loader refuses any `ExtendProgram` below 10,240 bytes, which makes the
driver's exact-top-up arithmetic unreachable for a small shortfall. Until those
are repaired the sequence is operator-driven, and cohort-16.1's job directory
carries it end to end.

Measured in the same run: the selected entry sits at manifest index **0** on a
real market, not the four-entry fixture's 3, so the funding slice's order is
derived from the entry index and never typed.

Adding this row renumbers the emitted scripts after `seal`. That is expected: a
cohort that gains a row renumbers, the stage names and their `GREEN` markers do
not move, and a job directory carrying the old numbering must be regenerated
before it is used — which a cohort founding under this row must do anyway,
because the driver that authors the edges is newer than any driver copied in
before it.

### escrow-seated

*Cohort-17.* For every refunding market this cohort founded, derive the failure
escrow — the ClaimsCapability PDA at `(market, claim_count − 1)`, then the
`ProtocolPositionV2` under that owner and the aggregate — and prove the two
Positions sum to the aggregate supply at every coordinate with the founder
issued no failure claim (decision 0025 item 2), read off chain rather than off
the founding's own receipt.
