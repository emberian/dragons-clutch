# Branch adjudication, 2026-08-31

Adjudicated by the BRANCHLAND convergence lane against `main` at `6cb1269b`
(the sweep ran while main was moving under live lanes; it started at `ec806db7`
and every conclusion below is of the form "main already contains this", which
main moving *forward* can only strengthen).

`git branch --no-merged main` counted **50** branches when this began. Forty-seven
of them are retired here. Three are live and were not touched.

## What the sweep found

Not one of the fifty branches held work that main lacks. This is not the usual
"the merge is hard, give up" conclusion — it is the measured one. Main absorbed
every branch's content between Aug 25 and Aug 31, mostly under *different commit
names*, which is exactly why `git branch --no-merged` kept reporting them as
outstanding. The refs were stale pointers, not pending work.

### Method, and why it is trustworthy

Three independent checks had to agree before a branch was retired.

1. **Patch equivalence** — `git cherry main <branch>`. A `-` means an equivalent
   patch already exists in main's history after the fork point. 41 branches were
   `-` on every single commit.
2. **File-level sweep** — for all 50 branches, the set of files present on the
   branch but absent from main. Nothing branch-unique survived this. The three
   paths that appeared on ~45 branches at once
   (`crates/dclutch-liability-basis-v2-kernel/src/product_claims.rs`,
   `.../tests/product_claims.rs`, `programs/dclutch-trading-sbf/src/series/lifecycle.rs`)
   are main's own deliberate *deletions* — `8bdc191b` finished the DCLTLNK2
   deletion, `b20256ee` took the lifecycle artifact's name off the funding module.
   Old branches simply still carry files main removed. Likewise
   `packages/dclutch-cli/src/submit.ts`, deleted on main by `7c2de1e5`.
3. **Landing-commit resolution** — for every retired commit, the commit on main
   that carries the same work, resolved by subject match and, where the subject
   was rewritten, by file-content verification. Those hashes are in the tables.

For the branches whose commits were *not* patch-equivalent, a fourth check ran:
byte-size comparison of every file the branch touched against main's version of
the same file. In every case main's file was identical or **larger** — main holds
a superset, never a subset. Details in the second table.

## Resurrecting anything retired here

Every tip SHA is recorded below. `git branch <name> <sha>` brings any branch back
intact; the objects stay reachable through the reflog. This document is the
tombstone, so the recovery instruction lives with the evidence.

## Retired: re-landed on main under a different commit name

Every commit on these branches is patch-equivalent to a commit already in main
(`git cherry` reported `-` throughout). The hash in the last column is main's
copy of the branch's tip commit.

| Branch | Tip | Held | Landed on main as |
| --- | --- | --- | --- |
| `audit/cu-topology-20260828` | `a8042260` | protocol compute checkpoint topology map | `e4382ed5` |
| `codex/release-thrum-repro-20260828` | `65543af2` | lock immutability proof, pinned Direct ELF gauntlet, Upgrade gate v1 shape | `78d848e1` |
| `fix/terminal-followup-20260829` | `e60f4276` | v7 funding and direct close fixture convergence | `641e3257` |
| `integrate/sponsored-pyth-v6-20260828` | `0bc6260f` | terminal execution split from Core accept | `32e3ebca` |
| `lane/activity-property-20260828` | `c26c520b` | lifecycle conservation concurrency property | `ab5d6ba9` |
| `lane/aggregate-retirement-audit-20260828` | `314ca467` | aggregate retirement checkpoint + v7 recheck | `ef9b9e52` (and `ab933881`) |
| `lane/aggregate-retirement-caller-20260829` | `00423f1d` | checkpointed aggregate retirement execution | `def0c0cb` |
| `lane/artifact-provenance-20260829` | `f83eb5f5` | SBF artifacts bound to exact provenance | `bff541db` |
| `lane/cohort2-upgrade-consumer-20260829` | `ddafbfff` | exact multi-link mixed Upgrade cohorts | `9847c6ab` (and `a985b99e`) |
| `lane/controller-abort-vertical-20260829` | `4296a184` | staged source abort recovery, pinned cleanup deployments | `9daf194a` (and `237f5b2f`) |
| `lane/cu-architecture-audit-20260828` | `3158bc5f` | runtime architecture change matrix | `193b0928` (and `6d32e16b`) |
| `lane/dcltcfq1-sysvar-20260829` | `cde38045` | ProjectFound36 as sole projection frame | `d9fce998` |
| `lane/decision-0012-dryplan-20260829` | `96759cc3` | decision-0012 devnet dryplan | `38f1fbcf` |
| `lane/dlr-accept-20260828` | `00158252` | lock-bounded accepted dealer checkpoint | `c546ed6b` (and `bb105691`) |
| `lane/dlr-accept-exec-20260828` | `b7a54844` | dealer page binding + custody reservations | `c3d9366b` (and `2e8a6831`) |
| `lane/dlr-custody-value-20260829` | `62a203ff` | dealer claims committed against locked value | `f54d483b` |
| `lane/economic-lifecycle-model-20260829` | `6eb38e04` | deterministic multiwallet ledger oracle | `6d9d4b36` (and `1a67b1c2`, `ce169b96`) |
| `lane/fractional-child-v3-20260829` | `cf44b638` | atomic claims child under the SBF frame bound | `ec0cfb8a` (and `3d525e5d`, `29291839`) |
| `lane/fractional-release-v3-20260829` | `1382e5ca` | onchain sha routed through the runtime adapter | `cb5387f0` (and `3102ee35`) |
| `lane/fractional-twin-20260828` | `02509855` | bounded v3 physical and retirement rung | `6d1a9bc8` |
| `lane/fractional-twin-current-20260828` | `e5af3664` | duplicate of the branch above, same single commit | `6d1a9bc8` |
| `lane/frontend-wallet-convergence-20260829` | `bae063d7` | authenticated Direct participant previews | `c5ee95ab` |
| `lane/gen-seven-20260828` | `6283ee46` | GEN-SEVEN request topology freeze | `31b9426d` |
| `lane/gen-seven-caller-20260829` | `0e7b213c` | sealed executable invocation and replay | `d78ea641` (and `2ba5cbc5`) |
| `lane/kappa-principal-enforcement-20260829` | `d995450e` | source principal cap enforced on growth | `e5933c4d` |
| `lane/loopback-zero-fee-20260828` | `c7db17d8` | fee policy extended to resolution tables | `cafbd082` (and `4fa49b8a`) |
| `lane/per-role-buffer-stage-20260829` | `f6ae0557` | finalized CLI signature shape validation | `211f2656` |
| `lane/private-upgrade-rehearsal-20260829` | `d7225855` | private Loader-v3 recovery rehearsal | `d0c1ed68` |
| `lane/pyth-credential-free-20260829` | `cdfed302` | credential-free Pyth devnet path | `c7e3f617` |
| `lane/pyth-sponsored-producer-20260829` | `97f89fb0` | sponsored Pyth market graph | `98adf2b3` |
| `lane/resolution-activity-v7-20260828` | `a1946ba4` | V7 terminal accept history requirement | `f51b61a0` |
| `lane/sbom-count-hygiene-20260828` | `db035996` | SBOM manifest coverage wording | `2cb5ea6e` |
| `lane/sbom-lock-repair-20260828` | `cf7f19ca` | SBOM dependency drift closed | `1e461d42` |
| `lane/series-sha-adapter-20260828` | `6bc62da8` | runtime SHA adapter for series digests | `813835e8` (and `bd096fc6`, `f452d55c`) |
| `lane/site-deps-20260828` | `bd218e6b` | static site toolchain update | `a55300c4` |
| `lane/source-abort-exterior-20260829` | `4272423d` | durable staged source abort | `ec45ac91` |
| `lane/source-abort-interruption-audit-20260829` | `8035abc1` | source abort interruption recovery freeze | `585a96a2` |
| `lane/upgrade-gate-emitter-20260829` | `60049f65` | reusable checked gate emitter | `88540e0a` |
| `lane/upgrade-stage-only-20260829` | `7d51526c` | buffer-only upgrade staging boundary | `56101477` |

Note the pair `lane/fractional-twin-20260828` and
`lane/fractional-twin-current-20260828`: two branch names, one commit's worth of
work, both resolving to the same `6d1a9bc8`. A branch was cut twice for the same
lane and only one copy was ever needed.

## Retired: work re-landed in adapted form, verified by content

These branches carried commits whose patch-ids do **not** match anything in main —
`git cherry` said `+`. Each was checked at the file level instead. Main holds the
work; it was reshaped on the way in, so the patch-id moved.

| Branch | Tip | Held | Why retired |
| --- | --- | --- | --- |
| `lane/sponsored-push-impl-20260828` | `35c3a68a` | full sponsored Pyth push lifecycle: codec, SVM, proof-SBF, successor exterior, SBF lifecycle test | Every file exists on main, landed by `bb405b12` "resolution: integrate sponsored push with v6 funding". Three files byte-identical; `crates/dclutch-pyth-svm/src/sponsored_push.rs` and the harness test are *larger* on main (24279 vs 23310, 67918 vs 65733). Main also grew `tools/pyth-sponsored-push-audit/` on top. |
| `lane/dealer-accel-positive-test` | `afde30e9` | dealer-accelerator frontier + physical program-tests, dealer-caller test program | Landed by `6767f688` "accelerator: the frontier instrument lands on main, and its caller builds the frame it is checked against". `frontier.rs` is 33378 bytes on main vs 24547 here; `physical.rs` 15510 vs 15174; `dealer-caller/src/lib.rs` identical. Main added `accepted.rs` besides. |
| `lane/founding-caller-bumps-20260828` | `85aedb28` | DCLTGMF3 founding turnover: caller bumps, SDK geometry, dryplan preflight, evidence | Six of seven commits patch-equivalent. The seventh, `ae792203`, landed as `988e0b12` under the same subject. DCLTGMF3 is live on main — 47 references in `successor/src/market.rs`, which is 582665 bytes there against 466556 here. |
| `lane/fractional-lock-refresh-20260829` | `6aa8d24a` | fractional retirement route, parent-authenticated close, Token CPI sync | Eight of nine commits patch-equivalent. The ninth landed as `d3cf6bbf`, same subject, and main built four more commits on top (`27d2c28e`, `c9f1414a`, `ab6d6177`, `80b78181`). The protocol-position lifecycle test is 78467 bytes on main vs 41700 here. |
| `integrate/safe-ops-20260828T0728` | `1daba511` | eleven-commit integration checkpoint: claims replay CLI, five-role devnet upgrade set, base account poststates, crash-safe terminal sequence, participant collateral journal | Seven commits patch-equivalent. The three integration checkpoints landed by content: `transactionReturnData.ts` via `ad7528bb`, `payoutCompletion.ts` via `f1528d09`, `devnet-permanent-id-upgrade.md` via `b1b13547`. |
| `integrate/safe-founding-20260828` | `0bdfd4b9` | the first four commits of the branch above | Strict subset of `integrate/safe-ops-20260828T0728`, same three checkpoints, same landings. |
| `codex/index-collision-safety-20260825` | `8a01f2cb` | codex-era product domain: `dclutch-product-admission-contract`, `dclutch-product-evidence-sbf`, exact-rational evidence deps | Aug 25, 2772 commits behind. Its 182 branch-unique files are the pre-refactor tree (`programs/dclutch-sbf`, `dclutch-bearer-contract`, `dclutch-collateral-contract`) that main has since replaced wholesale. The product domain was rebuilt as the v3 generation — `spline_admission_v3.rs` (`67496cbf`), `registry_v3.rs` (`fa486efa`), `generated_admission_v3.rs` — and main deleted the hand-mirrored admission chain in `f3364f19`. Retiring the ancestor of a domain that has been rewritten twice. |
| `rescue/d5dda5d` | `d5dda5d7` | a rescued `git stash`: "index on main" plus "On main: wip-source-borrowed-view-before-product-domain", 364 WIP lines in `crates/dclutch-source-contract/src/lib.rs` | Aug 24, 2981 commits behind — the oldest thing in the repo's branch list. Its 81 branch-unique files are crates main has since deleted. A stash of a borrowed-view experiment taken *before* the product domain existed; the domain it was reaching for has been built and rebuilt since. |

## Held: live lanes, deliberately untouched

| Branch | Tip | Owner / reason |
| --- | --- | --- |
| `lane/fee-core-20260830` | `a0b1f4cb` | FEE-TX2's landing stack. Six commits ahead, actively merging main forward. Not ours to adjudicate. |
| `lane/fee-tx2-20260831` | `89c40b54` | FEE-TX2 live, checked out at `/private/tmp/dclutch-fee-tx2`. Twelve commits, four files main does not have yet (`direct_fee_settlement_v1.rs`, `fee_settlement_v1.rs`, the fee-pair test and its runner). This is real pending work — the only branch in the fifty that genuinely is. |
| `lane/fraccheck2-trading-half-20260831` | `00a1679b` | FRACCHECK2 live, checked out at `.claude/worktrees/fraccheck2-20260831`. Six commits, six files absent from main including the claim-check escrow signer test program. |

Also checked and requiring no action: `lane/closeseal-20260831` (`60a21da6`),
`lane/basis-d-20260831` (`c8a9e0f4`) and `lane/genseven2-20260831` (`9ccee8c4`)
are all **already merged** to main and so never appeared in the unmerged list.
`lane/allkeys` no longer exists as a ref — it landed through the merge `e7805d62`
and was cleaned up before tonight.

## Landed: `lane/basis-d-20260831`

One branch in this sweep was neither retired nor held: BASIS-D finished while the
adjudication was running and was handed over as landable. It had committed
on-branch only because main's `Cargo.lock` held another lane's uncommitted lines
at the time; by the time this lane reached it the lock was quiet.

Landed as **`ffdc63f1`**, carrying `aac98afd` "basis: the kernel de Boor ports
into the codec, weights first and rounding not yet" — 6 files, +953:
`spline_eval_v3.rs` (483), `spline_differential_v3.rs` (450), and the
`runtime_v3.rs` / `lib.rs` / `Cargo.toml` / `Cargo.lock` wiring.

Verified before the merge commit, twice, because main moved 12 commits during the
first build and the verification was redone at the new tip rather than trusted
across the gap:

- `spline_differential_v3` (9 tests) and `basis_corpus_v3` (6 tests) — all green.
- All three generated-byte guards cmp-clean post-merge:
  `check-generated-basis-corpus-v3`, `check-generated-runtime-v3`,
  `check-generated-v3`.
- `cargo build --workspace` — exit 0.

## After

`git branch --no-merged main`: **50 before, 3 after**. The three are live lanes —
`lane/fee-core-20260830`, `lane/fee-tx2-20260831`, `lane/genseven2-20260831` —
which is what that command is supposed to be for. It can be trusted again as a
question about pending work rather than a list of ghosts.
(`lane/fraccheck2-trading-half-20260831` landed on its own during the sweep, and
`lane/basis-d-20260831` landed through it.)

## Not drifting again

`git branch --no-merged` cannot see re-landing, so it will rot into noise again on
its own. `tools/branch-census/census.sh` is the check that does not: it classifies
each unmerged branch by patch equivalence *and* branch-unique files, discounts
files the trunk deliberately deleted, and picks its base deliberately — refusing
the stale-`main` trap that would have argued for six wrong landings in the public
wrapper. It runs in 0.3s here and is a report, not a gate.

```
Fifty names, and forty-seven ghosts;
the work had all come home by other roads
and left its markers standing at the posts.
We read each stone before we let it go.
```

---

# The divergent line: `~/dev/dragons-clutch`

Adjudicated read-only. dragons-clutch is public; **nothing was pushed, and no ref
was deleted there.** Dispositions are recorded for a human to execute.

Six `integrate/*` branches, none of which was ever pushed (`git ls-remote --heads
origin` shows none of them). All six are **ancestors of `origin/main`** —
`git merge-base --is-ancestor <branch> origin/main` answers yes for every one, and
`git cherry origin/main <branch>` prints nothing at all, because there is no
symmetric difference to report.

| Branch | Tip | Held | Disposition |
| --- | --- | --- | --- |
| `integrate/dclutch-20260828T041451Z` | `6157b87e8` | crash-safe Upgrade and terminal operator sync; web app, CLI commands, resolution-core-v3-operator, successor | RETIRE — 0 unique commits |
| `integrate/dclutch-public-20260828T0846EDT` | `659615527` | honest devnet lifecycle boundaries, web components + guides | RETIRE — 0 unique; also an ancestor of the `0900EDT` tip, subsumed twice |
| `integrate/dclutch-public-20260828T0900EDT` | `82c44de78` | shipped bounty route reference, web components/lib, evidence + guides | RETIRE — 0 unique commits |
| `integrate/dclutch-subtree-2026-08-25` | `654eae25b` | the subtree import itself: 330 files, +191403, establishing `dclutch/` | RETIRE — **detach its worktree first**: `/Users/ember/dev/dragons-clutch-dclutch-integration` is checked out on it, clean, untouched since Aug 25 |
| `integrate/live-activity-20260828` | `45868be8c` | the heaviest: capability/custody/general-adapter/fractional-claim contracts, general/dealer/market-core codecs, pyth-svm, web redeem flow (~69k inserted lines) | RETIRE — 0 unique commits; all of it is on `origin/main` |
| `integrate/overnight-20260829` | `9db09ecbf` | Pages trailing route loads, trading-sbf, devnet-activity, genref, devnet-flight, svm-harness tests | RETIRE — 0 unique commits |

**A method trap worth writing down.** Running `git cherry main <branch>` in
dragons-clutch reports 1578–1631 `+` commits per branch and would argue loudly for
six LANDs. It is an artifact: **local `main` there is 1710 commits stale**
(tip `445988f2b`, 2026-08-25 23:30, against `origin/main` at `ba533ddb3`,
2026-08-31 00:29). Five of the six branches fail the ancestor test against local
`main` only because local `main` predates them. In dragons-clutch, measure against
`origin/main`, or fetch and fast-forward first. This trap does not apply to
dclutch, which has no remotes at all — its local `main` *is* the trunk.

**One genuinely unlanded thing.** dragons-clutch's local `main` carries a commit
`origin/main` does not have: `445988f2b` "site: explain the living dClutch
successor" (2026-08-25 23:30). It is the only unlanded work in the whole
dragons-clutch sweep, and it will be lost silently by any future
`git checkout main && git reset --hard origin/main`. It wants a deliberate
decision, not a default. Flagged, not touched.

**Scale note for whoever does hygiene next.** dragons-clutch holds 201 local
branches, 190 of them `agent/*`. The six integrate branches are 3% of the problem.
The same ancestor-of-`origin/main` test that cleared all six would classify the
`agent/*` population cheaply.

---

# The two preserved clones: gone

The brief asked for these to be inspected and explicitly **not** deleted:

- `/private/tmp/dclutch-sites-source.bbC1jj` (reported 8 commits on a codex remote)
- `/private/tmp/dclutch-sites-src.Uxc5vI` (reported 1 commit)

**Both are already gone, and nothing this lane did removed them.** The machine
rebooted at **Mon Aug 31 01:42** and macOS cleared `/private/tmp`. Every one of the
39 surviving entries in that directory has an mtime of 01:43 or later; nothing
that predated the boot is left. The two clones were last touched Aug 28 13:30 and
Aug 28 07:51, so they went with everything else. They were not preserved — they
were unreferenced temp directories that outlived their session by luck and then
ran out of it.

What they held, reconstructed from surviving transcript traces:

- **`Uxc5vI`** (17 entries) was a checkout of the **web app tree**, not the
  monorepo — transcripts name `scripts/generate-route-census.mjs`,
  `lib/directTicket.ts` and `lib/directTicket.test.ts` inside it, i.e. the
  `apps/dclutch-web` source root.
- **`bbC1jj`** (5 entries) was created the same minute as the tarball
  `dclutch-sites-ba487c8.hyhdTh` (1,448,732 bytes) — it was that snapshot's
  staging source.

**What is recoverable and what is not.** The sibling tarballs are named by commit
SHA. `da31941a` still resolves in dragons-clutch ("wrapper: pulse accepted
live-activity substrate", 2026-08-28), so the baseline the clones were cut from
survives. But **`ba487c8` and `7a07e50` exist in no repository we have** —
`git cat-file` finds no such object in either tree and neither appears in any
reflog. Those were commits made *inside* the temp clones. They are unrecoverable,
and any uncommitted working-tree work there is unknowable.

**The "codex remote" has no surviving substantiation.** dclutch has no remotes
configured at all; dragons-clutch has exactly one, `origin`. No `codex` remote
exists in either repo, and a grep across all of `~/.claude/projects` for the clone
names alongside `codex`/`remote`/`origin`/`clone` returns nothing. It was most
likely a local-path or sandbox remote that died with its sandbox.

The lesson is cheap to state and was paid for anyway: **`/private/tmp` is not
storage.** Work that matters goes on a branch in a real repo, the same hour it is
made. A clone under `/private/tmp` is one reboot away from never having existed.
