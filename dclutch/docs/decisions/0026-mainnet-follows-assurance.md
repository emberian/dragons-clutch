# Decision 0026: mainnet deployment follows assurance, and is not part of feature completion

Status: **PROVISIONAL — ruled by the orchestrator on 2026-09-04 under ember's
standing goal, confirmed by ember at 10:15 EDT, and reversible at the cost §7
states**. Docket item D4. Ember's amendment is at `GOAL.md:4654`. This record
closes the contract's `Mainnet act` register row
(`docs/MASTER_COMPLETION_CONTRACT.md:184`) and settles what C-14 closes on. It
authorizes nothing: mainnet remains a separately authorized external act, and
`AGENTS.md` still forbids it by name.

## 1. The question

C-14 (`docs/MASTER_COMPLETION_CONTRACT.md:99`) states the condition and then
declines to place it:

> *"Pinned toolchains, deterministic artifacts, SBOM/licences, source digests,
> migrations, compute/frame/packet ceilings and checked release manifests
> reproduce on supported builders. Devnet may be deployed and mutated freely for
> this work. **Mainnet deployment remains a separately authorized external act
> until Ember rules its place in completion.**"*

Register row `:184`: *"Whether actual mainnet deployment is part of feature
completion or follows assurance | open; no mainnet authorization."* C-16 (`:101`)
is the assurance entry, gated on six categories being empty. So the ordering
question is whether the irreversible spend happens before or after the hostile
walk.

**Devnet's whole economic model is reversibility, and every mainnet requirement
is its negation:** seven programs made immutable at about 32.9 SOL of
permanently burned rent, an `immutable-release-set` manifest over observed
accounts, a cross-host builder pair that has never been run, a clean SBOM from a
clean tree, a decided lifetime for the Core ephemeral authority, and seven
program keypairs nothing in the tree has ever minted
(`docs/design/DEVNET_DEMO_DEPLOY.md:78-127`, `:469-572`, `:1038-1097`;
`tools/release-tool/DESIGN.md:87-91`). Every tool refuses mainnet by genesis
hash today.

## 2. The ruling

**Mainnet follows assurance.** C-14 closes on devnet, plus the cross-host
builder pair actually run, plus a clean SBOM from a clean tree. The irreversible
spend moves behind the C-16 hostile walk.

This keeps decision 0012's disposability regime — the thing that let the project
burn through cohorts 9 to 14 in three days — and it makes the C-16 entry the
real gate rather than a formality performed after the money is gone.

## 3. Ember's amendment

Recorded at `GOAL.md:4654`:

> D4 — mainnet is far, after assurance

Ember's own framing to the orchestrator was that *mainnet is so far in the
future*, which is the same ruling with the emphasis on distance rather than on
ordering. It matters for one thing the orchestrator's version did not settle: no
lane may treat a mainnet requirement as a near-term blocker, and no row may
report itself blocked on mainnet. C-14's remaining work is the cross-host pair
and the SBOM, both of which are local.

## 4. The lanes implementing it, and what `supported_builders` means

**REPRO** (`GOAL.md:4658-4659`), which owns cross-host bytes. It also owes the
definition this record deliberately does *not* make:

**`supported_builders` is a definition to be produced by REPRO, not a decision.**
C-14's sentence turns on it — artifacts must *"reproduce on supported
builders"* — and the term has never been defined, so the row cannot be closed by
a ruling, only by a definition plus a measurement. The reader's own statement of
what it will mean: *the builder whose bytes are the release, and the set of
others that reproduce them*. The measurement that makes it urgent: today nine of
ten roles differ between our two machines, because a prebuilt toolchain embeds
its own build path. Ember's disposition, `GOAL.md:4656`: **converge by
swarmcycles** — the definition is reached by iterating the lane, not by ruling
on it now.

The C-16 rehearsal spoke walks the seventeen rows against current source and
produces the entry list before the spine's cohort closes, so what assurance
actually requires is a hostile's list rather than the orchestrator's estimate.

## 5. The hostiles and laws that guard it

**The genesis-hash refusal in every tool** is the mechanical guard and it
already exists: no release tool, stager or operator will act against mainnet's
genesis hash, so the ruling is enforced by construction rather than by
discipline. It should stay that way until the C-16 walk is clean, and its
removal is the act that would need naming.

**`AGENTS.md`'s authority block** already refuses mainnet anything, tags,
releases and force-pushes, and this record does not relax it.

**C-16's six categories** are the gate the spend now sits behind. That is the
substance of the ruling: not that mainnet is forbidden, but that the list which
must be empty is the same list either way, and it is cheaper to empty it while
devnet is still disposable.

**The control that would falsify the ruling** is the cross-host builder pair. If
two supported builders cannot produce identical bytes, C-14 does not close on
devnet either, and the ruling's premise — that everything C-14 needs is local —
is wrong. That measurement is REPRO's, and it is the one thing in this record
that could change it.

## 6. What was given up, named

**No mainnet market this generation, and no revenue.** Decision 0024's items 1
and 2 already ruled out a protocol take before mainnet; this record puts mainnet
after assurance. Together they mean the protocol earns nothing for the whole of
the accepted current project, deliberately.

**The seven program keypairs stay unminted**, so the identity a mainnet
deployment would carry does not exist yet and cannot be pre-committed to. Any
document that names a mainnet address today is naming nothing.

**C-14 does not get to claim "reproduces on supported builders"** until
`supported_builders` is defined and the pair is run. This record refuses to
close that half by ruling, which is the honest cost: the row stays open on a
definition rather than on a decision.

## 7. The cost of reversal

**Ruling mainnet *into* completion costs the disposability regime.** Decision
0012's substrate is mutable and iterated; ember's standing devnet grant requires
a **full redeploy with fresh identities** every time. A mainnet deployment
inside completion makes both impossible for the deployed set: programs become
immutable, identities become permanent, and the cohort-9-to-14 iteration speed
that produced every piece of chain evidence this project has stops being
available.

**It spends about 32.9 SOL irreversibly before the hostile walk has run**, on a
release whose entry list is still an estimate. The six categories C-16 requires
to be empty would be audited after the burn rather than before it, which is the
formality this ruling exists to refuse.

**It would also have to answer three unanswered questions in the same act**: the
Core ephemeral authority's lifetime, the seven unminted keypairs' custody, and
an `immutable-release-set` manifest over accounts nobody has observed.

Reversal in the other direction — deciding later that mainnet *should* be inside
completion — costs nothing structural, because assurance-first is a superset:
everything C-16 requires would have been done anyway.

## Evidence pointers

`docs/MASTER_COMPLETION_CONTRACT.md:99`, `:101`, `:184`;
`GOAL.md:4654-4659`;
`docs/design/DEVNET_DEMO_DEPLOY.md:78-127`, `:469-572`, `:1038-1097`;
`tools/release-tool/DESIGN.md:87-91`;
`docs/decisions/0012-devnet-iteration-substrate.md`;
`docs/decisions/0016-checked-release-identity.md`;
`AGENTS.md` (authority and safety block).
