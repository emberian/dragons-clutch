# The compute ceiling was a hasher, not an architecture

Status: **VERIFIED FINDING, BRANCH-ONLY.** The work is on
`fable/sha-syscall`, not merged. Main's artifact still contains the software
hasher, so every measurement currently recorded against main remains true
*of main*. What changes is the **cause**, and therefore several claims that
generalized from it.

## What was found

The largest symbol in the deployable program was `sha2::sha256::compress256`
at 53,952 bytes — a fully unrolled portable SHA-256. It did not arrive from
the call sites anyone suspected. It entered through a single unconditional
dependency edge in `research/batch-policy-identity/Cargo.toml`, a crate that
both `clutch-solana-layout` and `clutch-sbf` depend on directly. Converting
only the suspected call sites moves 3,264 bytes and leaves the symbol in
place; that intermediate was measured to confirm it.

Target-gating that dependency (portable off-chain, `sol_sha256` syscall on
SBF, with the portable path retained as the differential oracle) removes it.

## Verified independently

Clean rebuild on the branch, measured by this session rather than reported:

- ELF **1,420,608 bytes** (from 1,490,544), SHA-256
  `187d5ee16f72946ae81eb928e127347c1e17aaf0a0ad7d837be5c4186dde16bd`
- `compress256` symbols: **0**
- dynamic imports include `U sol_sha256`: the syscall is linked and used

No digest value changed anywhere. Equivalence is tested at every call site
and every part count, with canonical identities additionally pinned against
independently computed Python `hashlib` bytes rather than against either
in-tree path.

## The reattribution

| route | before | after |
| --- | ---: | ---: |
| Submit replacement (tightest row) | 1,120,392 | 198,483 |
| Native resolve, degrees 1/2/3 | ~1.09–1.10M | ~182–198k |
| ResolutionWork Begin | ~805–811k | ~85–91k |
| Occupation initial, all six rows | 1.24–1.26M | 173–190k |
| InitDirectEpochV4 | 680,723 | 40,882 |

Every measured instruction improved, by roughly 3–8×. The tightest row moved
from 80% of the transaction ceiling to 14%.

**Two recorded STOPs dissolve:**

1. **Direct V2 selection.** Complete top-three selection previously consumed
   exactly 1,400,000 CU and rolled back. It now completes at **226,071 CU**
   and commits. The STOP was correctly measured; its cause was
   misattributed. It was a property of the hasher, not of the selection
   algorithm.
2. **The occupation admission gate.** All six span/degree rows previously
   failed the selected 25%-headroom policy; all six now clear it.

## What this invalidates, and what survives

Invalidated as *architectural* claims — each was true of the artifact and
false as a generalization:

- "On-chain re-execution of verification does not fit, and every axis worth
  scaling makes it worse" (`SOPHISTICATION_GAP_2026-08-19.md` §3). The
  measurement was right; the conclusion drawn from it was not.
- "Single-transaction re-execution of a clearing does not scale" as used in
  the Draft 11/12 filings' operational-readiness passage.
- The premise framing in `SUCCINCT_CLEARING_FEASIBILITY.md` §1. Succinct
  verification remains interesting for *large* books; it is no longer
  motivated by this measurement.

Survives unchanged:

- Direct V3's staged design is still correct engineering and still the thing
  that made the venue exist. Staging bounds per-transaction work regardless
  of constant factors, and it is what allows growth beyond two orders.
- V2 completing is **not** a promotion. Whether V2 may be described as live
  is a rent, staging, and lifecycle question this measurement does not
  answer, and V2 remains superseded by V3's lifecycle.
- Every scope caveat on the V3 campaign: one bank profile, five candidates,
  eleven ticks, unpromoted in the liveness profile.

## Consequences to sequence

1. Merging the branch changes the sealed runtime identity again and requires
   a full reseal: artifact audit, liveness-profile re-measurement (every row
   moves, so this is not a +1 CU cosmetic pass), 100-gate emission, manifest
   commit, post-commit check, fresh portable attestation.
2. `CURRENT_TRUTH.md`, the sophistication assessment, the succinct-clearing
   memo, the planned-versus-built scorecard, and both draft filings each
   carry the old attribution and must be corrected **from the merged
   measurement**, not from this note.
3. The liveness profile's admission policy should be re-examined rather than
   merely re-measured: thresholds chosen against 1.1M-CU routes may be the
   wrong shape for 200k-CU routes.

## Incidental finding worth keeping

The speedup exposed a latent test defect. A `source_ingest` case relied on
two slow transactions naturally acquiring different blockhashes; five times
faster they shared one, and the runtime answered `AlreadyProcessed` before
the program executed — meaning the answer under test had never been the
program's. It now demands a fresh blockhash. Measured at one failure in five
on the fast build and zero in six on the slow one, which is what identifies
it as latent rather than introduced.

## A measured negative

`opt-level = "z"` and `"s"` shrink the binary 23% and 21.5% and **fail
tests**: 205 and 139 SBF frame-overflow diagnostics respectively, producing
wrong program error codes and `ProgramFailedToComplete`. Less inlining keeps
more locals live across calls, which a 4 KiB frame cannot absorb. Keep
`opt-level` at its default. Recorded so nobody re-runs this experiment.
