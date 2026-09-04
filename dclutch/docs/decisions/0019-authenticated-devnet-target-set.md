# Decision 0019: the devnet upgrade target set is an authenticated input, not a constant in our source

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible on request).** Previously: PROVISIONAL — ruled by the orchestrator on 2026-09-02 under ember's**
standing goal, landed the same morning, and reversible by ember at the cost §7
states**. The ruling is `GOAL.md:2996-2999`, carrying the standing formula
*"RULING (under the standing goal; ember may reverse)"*. It amends decision
0012 by one paragraph (`docs/decisions/0012-devnet-iteration-substrate.md:124-157`)
rather than replacing it. Landed at `8e1f98507` (2026-09-02 06:09), evidence
`615c243f8`.

## 1. The question

`PERMANENT_DEVNET_UPGRADE_TARGETS_V1`
(`tools/local-validator/bootstrap/successor/src/upgrade.rs`, a DEPLOY-1-era
constant) hard-coded seven program/ProgramData id pairs — cohort-7/8's — and the
capture family accepted no caller-supplied Program set. So the entire
checked-upgrade lineage, and therefore every plan's `checked_upgrade_set`, was
scoped to exactly one substrate.

Condition (a) of ember's standing devnet grant requires **fresh identities on
every redeploy**. The two are mutually exclusive by construction, and the
consequence had been paid for weeks without being named:

> from cohort-9 onward no cohort's journal could match the table, so no cohort
> could be sealed, so no checked execution release could be built, so **no
> devnet Direct fill has ever executed**
> — `docs/decisions/0012-devnet-iteration-substrate.md:132-134`

The pinned substrate itself (`Hies39GB…`) had been closed by that same redeploy
discipline, so the constant could not even seal itself.

## 2. The ruling, verbatim

> **RULING (under the standing goal; ember may reverse): the target set becomes
> an authenticated INPUT** from the plan the ladder already authenticated, with
> the journal's per-row chain re-read as the safety and an explicit refusal when
> plan and chain disagree; decision 0012 amended by one paragraph; the Lean
> admission model verified not to name the constant first.
> — `GOAL.md:2996-2999`

## 3. What it changed in the trust model

The anchor moves from **a constant in our source** to **a set authenticated
per-row against the cluster**. The seven ids now come from the deployment-set
journal that names them, authenticated as a set by
`DevnetUpgradeTargetsV1::authenticate`
(`tools/local-validator/bootstrap/successor/src/upgrade.rs:157`, `:161`;
entered from `journal_targets` at `:257`): exact width, canonical role order,
`programdata(program)` — the Loader-derived ProgramData coordinate, which is the
check the retired table was itself validated against and which now applies to
every set rather than to one — and fourteen distinct non-native accounts
(`0012:139-144`).

The principle is stated in the amendment itself (`0012:144-147`):

> **The safety was never the constant.** It is that every row is re-read against
> the cluster, under the journal's own `retained_upgrade_authority`, before any
> of it is believed; a constant adds nothing a fresh observation does not.

The Lean admission model was verified to name no constant first — a check that
the formal side did not silently encode the retired assumption. That was a
verification, not a formal change.

## 4. What it saved, measured

It unblocked the thing that had never happened. **Cohort-12 sealed at zero SOL**
the same evening (`GOAL.md:3106-3107`); cohort-13 sealed and founded
(`docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md`); cohort-14 deployed,
sealed and **filled** (`GOAL.md:3885-3892`,
`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md`). The saving is not
a CU figure; it is the existence of C-04's chain evidence at all.

## 5. The hostiles that guard it

Where the plan's declared set and the chain's observed set disagree, the refusal
**names the role and BOTH ids**. The refusal it replaces named one id and a
constant, *"and cost a lane a night"* (`0012:155-157`) — the tree's own
`map_err`-that-discards-its-cause lesson applied to an identity comparison.

The capture CLI's test *"asserted the opposite property and is rewritten; that
property is what made the lineage unreachable"* (commit body `8e1f98507`) — a
test that had been green for weeks while pinning the defect.

## 6. What was given up, named rather than left to be found

`0012:147-157` records both losses in the amendment itself:

- `is_permanent_devnet_program_set` and the `validate_prepare` refusal that used
  it are **RETIRED**, because a prepare with no `--deployment-set-journal` has
  no authenticated set to compare against. The general rule that refusal was a
  special case of survives in `campaign::require_checked_mutable_binding`.
- `devnet-permanent-substrate-capture-v1` now takes seven
  `--expected-<role>-program` flags: a **declared**-set capture rather than a
  fixed-set one — still key-free, still read-only, and now deriving every
  ProgramData coordinate instead of accepting it.

The retired constant survives only as a test fixture with a comment saying so
(`upgrade.rs:9941`, `:9948`).

## 7. The cost of reversal

Restoring the constant re-creates the exact deadlock: no fresh cohort can seal,
so no Direct fill can execute on devnet, so C-04's chain evidence stops at
whatever cohort the constant names. It would also have to un-retire two removed
refusals and re-invert the capture test. The devnet grant's condition (a) would
have to be relaxed in the same act, because the two cannot both hold.

## Evidence pointers

`GOAL.md:2992-2999`, `:3106-3107`;
`docs/decisions/0012-devnet-iteration-substrate.md:124-157`;
`tools/local-validator/bootstrap/successor/src/upgrade.rs:146`, `:157-161`,
`:257`, `:9941-9948`; commits `8e1f98507`, `615c243f8`;
`docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md`,
`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md`.

**Confirmed, 2026-09-04 15:50 EDT.** Ember, having read the docket that listed this ruling under "M1–M6: a word if any should be reversed; silence is not a ruling": "you aren't waiting on me for rulings are you? i was reading the docket and contemplating it, but overall find your takes reasonable." Taken as confirmation; reversible on request.
