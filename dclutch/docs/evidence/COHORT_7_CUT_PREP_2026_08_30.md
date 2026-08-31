# Cohort-7 cut prep: the floor, measured on the cohort's own ELFs

2026-08-30, TRADE-3. Prep phase only — **no devnet write was made**, and the go
for one still comes from the orchestrator. This is the measurement the locked
cut protocol demands before market19 is founded, taken on the ELFs the cut would
actually deploy rather than on numbers carried over from `main`.

## The candidate

**`a93256c10644718b66e087ff0c69327e99160d7e`**, which contains `a7e2f668`
(the FOUND-5182 fix). Built from a **clean `git archive` of that commit**, not
from the working tree — the shared tree routinely carries a dozen half-written
files belonging to other lanes, and under Ledger M-61 a one-byte ELF difference
redraws every fixture seed by up to ±46,000 CU, which is more than twice this
route's whole margin. On a compute gate, "whatever was on disk" is not a noisy
answer to the right question; it is a different question.

Instrument: `tools/ci/run.sh programs --commit <rev>`. Eight manifests built,
**zero SBF stack-frame-overwrite diagnostics**, then the 32-seed
`direct_hot_top_level_margin_gate` run against those ELFs.

**Freshness control, because this exact defect has bitten before.** CORESTATE-2
found that `git archive`'s commit-time stamps plus a reused work root make cargo
silently attribute the *previous* commit's artifact, so any CU figure from a
reused work dir has to be re-derived. Both roots here were removed before the
run and re-created by the archive step, and every per-link build log carries at
least one `Compiling` line for its own top package (trading 123, claims 111,
core 3, custody 1, registry 1 — the counts differ only because cargo shares the
dependency tree inside one archive root). **No link was silently reused.**

**Not `tools/gauntlet/hot-cu`.** That tier drives the Registry continuation, and
HEAPRED measured it at +35,127 CU against top-level on all thirteen comparable
seeds. Every "Hot CU" figure that tier ever printed is that much high.

## The verdict: floor and tail

A worst swept seed is a **sample of a lottery, not a bound**, and this document
does not report one as if it were. The route's cost decomposes exactly:
`CU(seed) = C0 + 1,500 × T(seed)`, where `C0` belongs to the code and `T` — the
number of `find_program_address` candidates rejected across the seven surviving
key-varying sites — belongs to the keys.

| quantity | measured |
|---|---|
| **key-independent floor** | **1,319,583 CU** |
| gate constant `TOP_LEVEL_KEY_INDEPENDENT_CU_V1` | 1,320,326 — **pass, by 743 CU** |
| implied `C0` | 1,318,083 |
| all-first-try cost | 1,328,583 |
| residual spread over the sweep | 6,001 |

**The tail.** Headroom above `C0` is `1,400,000 − 1,318,083 = 81,917 CU`, which
buys 54 search attempts at 1,500 CU each, so a stranger's keys refuse only at
**55 or more** attempts summed over the seven sites. With each site geometric:

| assumption | P(a stranger's key exceeds the ceiling) |
|---|---|
| `p = 1/2` | **1 in 614,150,615** |
| measured `p̂ = 0.446` | **1 in 8,593,254** |

Context, and explicitly **not** the verdict: the 32 swept seeds ran
1,328,584–1,346,584 CU, mean 1,337,634, band 18,000; the analytic worst over the
deepest draw each site made in this sweep is 1,376,583 against 1,400,000. Both
are facts about these 32 keys and this ELF set.

The wrong-bump control passed — the four-arm test that separates a live carry
from a decorative one — so the bump carry is engaged rather than staged and
ignored, which is the defect the variance census found last time.

## The drift worth naming

FIXBUMPS measured floor **1,318,826** and set the gate constant at **1,320,326**
— exactly 1,500 CU of headroom. The floor is now **1,319,583**.

**757 of that 1,500 has been spent**, so a little over half the gate's headroom
is gone, and the half that moved is the one that belongs to the *code* rather
than to the keys. The tail moved with it: 1 in 1.10 billion at FIXBUMPS, 1 in
614 million here.

This is green and the tail is still enormous. It is recorded because the gate
exists precisely to make this visible at its author rather than on devnet a
month later.

**It is very likely bought rather than lost, and saying so matters.** Thirty-four
commits touch `programs/` or `crates/` between `30574297` and the candidate, and
the leading suspect is ALLKEYS — which deliberately traded constant compute for
the elimination of key-dependent refusals, under ember's *"ALL KEYS MUST
TRANSACT"* ruling, and which measured its own hint arm at **+295 CU on the
luckiest seed**. The sweep's 6,001 residual spread is exactly ALLKEYS' reported
hinted band, which is consistent with the hint path being live here. Paying
constant CU to remove a refusal tail is precisely what that ruling asked for, so
this is probably the price of a decision, not a regression.

**What would settle it, with its size:** a floor measurement at each of ~6
bisect points over those 34 commits — the floor is key-independent, so one run
per point suffices — at roughly 15–20 minutes per cold build, so about two
lane-hours. **It was not run**, and until it is, the attribution above is a
hypothesis with a motive, not a measurement.

## Scope, said plainly

The floor gate builds the **five** programs on the Direct hot route plus three
test-only callers. **The deploy set is seven roles**: Resolution and
Rent-Credit are not in this measurement. The complete cut artifact is
`tools/release/checked-release-candidate.sh` at the same pinned commit; it was
run, it is green, and the section below records it.

### ELF digests, floor-gate set

```
e277be085218483fe5f424a967c9981791bd59b0b4cbc26db0d4cf1e8f60e8cb  dclutch_claims_sbf.so
a18b083d0c286c982fcb2e6bab0b23109f9de130e7b578e1b74161d73712d165  dclutch_core_sbf.so
01a9fb5c8e793337b274a1122b9e3d929e5386379e8b3af5ef3721012abaa237  dclutch_custody_sbf.so
b90fd427a5cc838c18a6ebd0ba7d2d588f9d9e27b45e62feac759032a1d6f315  dclutch_registry_sbf.so
d8f93f2d2bb74f639210511f0dc2f912bb3795a372c9723dc148d83b26710b34  dclutch_trading_sbf.so
b1cf9a2992f3224c2f14a7b7200280934326b75199f466760afa0be35e688576  dclutch_trading_core_caller_test_program.so
8ce19f9450e823f5807a9489c8a2b9d6dbce55845af526e4ff8e2428e0c51417  dclutch_trading_outer_test_program.so
06073851ea2fc1209ee1fb61fd022f0965f1ca4765140ec67aef517a5d9dde63  dclutch_trading_registry_test_program.so
```

## The full cut artifact, and the control that ties it to the floor

`tools/release/checked-release-candidate.sh --commit <rev>` at the same pinned
commit, fresh work root: **green**.

- 13 links built, `sbf_build_freshness=passed`;
- **`sbf_build_diagnostics_total=0`, and `sbf_build_diagnostics_accepted=false`**
  — zero diagnostics, not diagnostics waved through;
- `cargo_lock_immutability=passed`, 64 locks, set digest `ab2a103a…`;
- all ten role and accelerator artifacts checked, plus the five-role execution
  release set and the immutable Core/Registry/Rent infrastructure;
- **checked Upgrade gate `877ba6a33a48a75839c34c306f42f561c9e8fe45f470f2f028489ec665b8eb52`**;
- source revision `a93256c1…`, source digest `aada96d4…`.

**This is the direct retest of the `ReleaseSetSelectionMismatch` that CAMPAIGN
hit on a fresh checked release at main HEAD.** It does not reproduce at this
commit, which is what FOUND-5182's fix riding `main` was supposed to mean and is
now measured rather than assumed.

**The control that makes the floor transferable.** The floor gate and the
checked release are two independent builds, from two separate archive roots,
minutes apart. All five protocol ELFs came out **byte-identical**:

| role | sha256 |
|---|---|
| core | `a18b083d0c286c982fcb2e6bab0b23109f9de130e7b578e1b74161d73712d165` |
| claims | `e277be085218483fe5f424a967c9981791bd59b0b4cbc26db0d4cf1e8f60e8cb` |
| trading | `d8f93f2d2bb74f639210511f0dc2f912bb3795a372c9723dc148d83b26710b34` |
| custody | `01a9fb5c8e793337b274a1122b9e3d929e5386379e8b3af5ef3721012abaa237` |
| registry | `b90fd427a5cc838c18a6ebd0ba7d2d588f9d9e27b45e62feac759032a1d6f315` |

Under Ledger M-61 a one-byte ELF difference redraws every fixture seed by up to
±46,000 CU, so without this the floor would be a number about *some* build. With
it, **the ELFs the gate measured are the ELFs the cut deploys.**

## What this does not settle

- **The founding parameter is still the irreversible one.** market19 must be
  founded **zero-fee**: a fee-bearing trade is 1,515,003 CU at all-first-try,
  over the ceiling by 115,003 before any key is drawn, and the rate is sealed at
  founding. `tools/release/stage-devnet-sponsored-market-open.sh` now refuses to
  default it.
- **The founder identity is the other one.** Found only against a key we hold;
  decision 0015 §8 is what happens otherwise, and the same file now refuses a
  bare public founder.
- **This measurement is a ProgramTest, not devnet.** It proves the route fits;
  it does not prove the devnet founding sequence executes.
- The cut's disclosure items — what the upgrade does to the three markets
  already on devnet, and the funding-readiness suffix — are in
  `tools/release/README.md` under "The cohort cut window", verified rather than
  inherited.
