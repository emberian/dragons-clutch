# Dealer terminal composition — native controls, September 8, 2026

Owner: DEALER-COMPLETION. Evidence level: **native diagnostic controls**, not
SBF, local-validator or devnet execution. This does not close C-06.

The implementation adds a permissionless Dealer terminal parent over the existing
Claims terminal evaluator. Claims admits an explicit TradingPrincipal recipient
only through its authenticated Trading caller path. The parent binds the fund,
owner, context, programs and exact capital vault; joins the child request and
receipt; checks the vault increase against collateral payout; and commits fund
cash and the resulting Claims inventory last. Native bond proceeds go to the
fund account, separately from collateral. LP accounting for those lamports and
full capital exit remain the next fund-schema convergence.

## Substrate and source

The isolated hbox source is
`/tank/dclutch-c16c-repo/dealer-completion-20260908/candidate`, with its own
workspace-root `target`. Its base is the archive of
`f3f20f42afe12121665fee1e6fef5cacb75184a7`, overlaid with the lane's changed
Rust files and dispatch hunks. This is not an exact-current committed checkout.
All ten changed Rust module/producer files were compared by SHA-256 against the
live lane source after the guard mutations were restored; all matched.
[The machine-readable record](DEALER_TERMINAL_NATIVE_COMPOSITION_2026_09_08.json)
carries the source hashes, commands, statuses and final guard-log hashes.

Heavy checks ran through `swarm-build`, sequentially in this checkout. Logs and
runner scripts remain beside its `candidate` directory. No Mac Cargo build was
used. Local locked offline metadata and the focused Lean/SDK emission verifier
passed. The preserved generated Rust and TypeScript widths and offsets come
from `ScoringRuleAbiV1.lean` and its emitters.

## Observed controls

| Focused group | Passed |
| --- | ---: |
| Claims terminal codec | 7 |
| Dealer request/receipt codecs and frames | 6 |
| Claims recipient adapter | 2 |
| Dealer terminal owner/context adapter | 2 |
| Operator Trading positive/zero payout and native-bond separation | 2 |
| Existing wallet positive/zero payout control | 1 |
| Existing founder-bond tail control | 1 |
| Final successor Dealer origin/flag module | 16 |

The final Claims adapter control also substitutes recipient, Custody authority
and Custody program, and refuses the enclosing-Claims authority mode. Each
refusal leaves its host AccountInfo bytes and lamports unchanged. This is host
nonmutation evidence; transaction-wide validator rollback is still owed.

Three deliberate mutations were separately compiled and executed: remove the
canonical recipient-account equality, remove the Dealer owner equality, and
redirect native bond proceeds to the token authority. Each named test failed
with a test assertion (exit 101, no compiler error). Each original source was
then restored byte-for-byte and its focused test passed. The final refusal
census passes `--check-unique`, recognizes the Dealer redemption magic and all
four named `TerminalRecipientErrorV3` variants. Its explicitly labeled archive
inventory is `terminal-inventory-explicit.json`; it is not a current-source
protocol coverage claim.

The existing explorer's three scoring tests passed in the web owner's isolated
Node checkout after copying only `accountRecords.ts` and the generated scoring
module. Both copied hashes matched their live source. The first local attempt
could not run because the cleaned Mac tree had no `vitest`; no test result is
attributed to that invocation.

The first operator Trading test selected a zero-payout coordinate while asking
for a positive Custody request and failed. It was corrected to select the
winning coordinate and assert a nonzero payout before checking the transfer.
A separate zero-payout case checks unchanged replay and token bytes. No economic
refusal was weakened. Review also corrected the parent receipt: its fill-only
claim-unit flow fields remain zero on redemption; collateral atoms are
represented by the child receipt and resulting fund cash.

## Still required

SBF link, frame and CU checks, accepted terminal redemption and hostile rollback
on a real validator, full LP/equity and policy evolution, General reservation,
capital exit and physical retirement have not executed for this slice. The
commit deliberately leaves the frame ratchet red. A fresh exact-current
committed eight-program run remains required before protocol completion.
