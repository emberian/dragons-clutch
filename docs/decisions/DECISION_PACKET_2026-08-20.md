# The decision packet — everything open, analyzed, ordered

Status: **SYNTHESIS.** One line per decision with the report's recommendation;
the reports carry the analysis and counterarguments; the register carries the
long tail. Recommendations are the reports', stated plainly — every one of
them is ember's to accept, amend, or refuse. Ordered by when to decide.

## Decide now (cheap, unblocking, no calendar pressure against them)

| # | Decision | Recommendation | Report |
| --- | --- | --- | --- |
| 1 | GENERAL_CLEARING_POLICY_V1 + CANDIDATE_WINDOW_SLOTS freeze | **Freeze as-pinned, one act.** Already compiled into the sealed ELF; freezing = one commit, amending = fork + reseal + discards sealed T2 evidence. R-b test demand verified satisfied. | policy-freeze |
| 2 | ADR-0005: Lean is the proof substrate of record | **Adopt** (draft ready in the report's appendix). 212 verified zero-sorry theorems vs Rocq's zero (its one apparent theorem is vacuous). Unlocks the FEE_GEOMETRY §7 rewrite and ten stale-gate cleanups. | adr-0003 |
| 3 | R2 model close (E1) | **Ratify now, before Aug 26.** Research-only; forecloses nothing. | r2-cutover |
| 4 | Realm admission allowlist | **Freeze the built allowlist**: Token-2022 base mints only, extension ceiling zero, ImmutableOwner required on the Hoard (plain SPL structurally cannot satisfy it — the freeze is principled, not accidental). DREGG dogfood mint has no executable V1 profile; say so. | realm-token2022 |
| 5 | Deploy identity + posture | **Deploy at sealed opt-3 e8ba31d5 only; refuse opt-z** (it went RED again on the current tree — 31 frame overflows; the Tier-0-era green is stale). Devnet beta authority ratified as-recorded; the reference deployment is **immutable-at-first-deployment** as design posture. | deploy-economics |
| 6 | RevenuePolicy B4c: Plane-L charges | **Permanent zero as frozen policy** — charges debit payers not keepers, the protected-pools table forbids the flow, revenue would be ~0.01 SOL/market. Kills the vault build entirely. Decide FIRST of the six: it rewrites B4d and scopes B4f. | revenue-policy |

## Decide next (each unlocks an implementation wave)

| # | Decision | Recommendation | Report |
| --- | --- | --- | --- |
| 7 | R4 terminal ratification | **Ratify incinerator sink, Arm A, failure-payout; legacy-permanent ONLY with the scope amendment** (verbatim ratification would declare the live walk plane's rent permanent — both the R4 and promotion reports converged on this trap independently). §8 variant: B conditionally or explicit deferral — **never A as written** (its falsifier already fired in-tree). Unlocks the decision-half of 8 of 14 terminal blocking ids and the TerminalClosure wave (~0.4–1.4 SOL/epoch reclaim). | r4-terminal |
| 8 | RevenuePolicy B4a/B4b/B4d/B4e/B4f | Treasury-key custody requirements now, pubkey at first fee-bearing Realm; **treasury Position** for Plane C (with the mid-epoch-close grief rider); policy-object-first sequencing; freeze 60/0/40 + AllRestingMakers; both terminal rows conditional on #7. | revenue-policy |
| 9 | Fee base (B1) | **Composite `κ·G + κ'·R`** — dispersion with a price-free quotient floor: closes the zero-price hole all consideration-proportional bases share, re-parameterizes (not destroys) the uniqueness characterization — proven in the lab (suite 33→40). Rates stay after-decisions; bytes stay `FeeBaseV1::None` until the destination lands. Jointly: ratify the market-quality descope + rewrite §7. | fee-base |
| 10 | Clearing-plane promotion | **Walk plane: rung W1** (CU/quote rows, no live flags) contingent on #1 — all 25 route maxima already pass admission arithmetic; rent (~0.79 SOL/epoch unclosed) is the W2 blocker, tied to #7. **V3: commission the sealed syscall-era measurement campaign now** (evidence-only), full admission as declared target. **B/C closures: ratify content**, with the process note recorded. | promotion |

## Calendar-bound (the week's spine)

- **Aug 24** — filings freeze (Draft 13 ready; the artifact is correctly described as value-refusing).
- **Aug 26, 16:00 UTC** — Pyth cutover lands; observation begins. **E2 identity freeze is evidence-triggered, not date-triggered** (SDK discrepancy resolved + Config stability over a named span; the freeze record must name its cluster and the 3-of-5 router trust-floor acceptance). E4: the runtime-capabilities branch rebased CLEAN across 153 commits — `r2-caps-rebase-trial` seeds Phase 0 now, merge rides E3's reseal. E5 (Pyth ToS counsel) commissioned now, concluded before E3.
- **E3 registry flip — last, and never pre-authorized.** 12-gate table in the report; gates 1–9 close on the local ladder without devnet SOL; gate 10 already green.

## What the packet deliberately leaves open

The fee **rates** (κ, κ') and the treasury **pubkey** — both structurally
after-decisions. Mainnet, real value, L0: untouched by everything above.

## The convergences worth knowing

Three independent lanes hit the same wall from different sides: the walk
plane's accounts have **no close path** (terminal report: permanent-rent by
design vs amendment; promotion report: the W2 blocker; RevenuePolicy: the
receipts/pot rows). **TerminalClosure is the single highest-leverage
engineering wave the decisions unlock** — everything else in the next wave
composes with it.
