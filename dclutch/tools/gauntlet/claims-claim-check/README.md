# claims-claim-check — the native claim-check life, on real ELFs

Two routes row **C-10** owned and no campaign bound:
`claims/claim_check_compaction_v1::process_compaction` (the crank) and
`claims/claim_check_redemption_v1::process_redemption#else` (the redemption).

The campaign was never missing. `programs/dclutch-claims-sbf/tests/claim_check/mod.rs`
has driven them against real Claims, Custody, Registry, Core, Resolution and the
canonical Token-2022 v11 since it landed, and it *already emits evidence*. What
was missing is the other half: no `bindings.json` named its labels, so
`docs/reference/routes.md` read `NEVER-EXECUTED, no stated reason` for both.

## The family, in one sentence

A stranger opens an escrow on a market that went terminal, waits out the
compaction deadline, cranks a sleeping holder's position into a claim-check —
paid out of rent that was leaving anyway — and the holder is paid from it long
afterwards, having signed nothing at any point.

| act | CU | frame |
|---|---|---|
| open escrow | 40,031 | Claims + SPL Token + System |
| compaction crank | 530,670 | Claims + Custody + SPL Token + System |
| redemption | 13,610 | Claims + SPL Token |
| escrow close | 7,728 | Claims + SPL Token |

Thirteen refusals across nine codes, all inside the two `0x56xx` claim-check
families. The cheapest is `a stranger tries to redeem` at 3,120 CU
(`ClaimCheckRedemptionSbfErrorV1::Authority`) — a stranger may *create* a
claim-check for a holder and may not redeem it, which is the asymmetry the whole
family exists for, stated as a code.

## Two rows that are deliberately not claim-check routes

`the holder redeems for itself` and `redemption after a deadline-sized warp` are
**ordinary wallet payouts** through `claims/terminal_settlement_v3::process`.
They are the control the compaction is measured against — the campaign asserts a
compacted claim-check is worth exactly what the holder's own redemption would
have paid. They invoke Custody and cost 507,165 CU against redemption's 13,610.
Binding them to the redemption route because their labels say "redeem" would
credit it with transactions it never saw; a witness asserts the ratio instead.

## Why this is a separate tier

The unfiltered binary belongs to `tools/gauntlet/claims-rational-representation-v2/`.
Folding its full 273-transaction run against those bindings reports **143
problems** — dozens of unbound labels across the representation, Trading
common-Hot and failure-terms families, plus one stale binding — so
`run-claims-extended.sh` cannot pass at HEAD. That repair belongs to the row that
owns the representation campaign. This tier is scoped to the `claim_check::`
filter, the same relationship `tools/gauntlet/structured/` has to the same
binary, so the claim-check rows stand on an instrument that is green: 33
bindings, 96 transactions, 58 labels, 112 observations, zero census problems.

## Running it

    tools/gauntlet/run.sh --mode census          # once, for the inventory
    tools/gauntlet/claims-claim-check/run-claims-claim-check.sh

The runner gates on `cargo check`, refuses on any SBF stack-frame-overwrite
diagnostic, **refuses unless the Token-2022 ELF matches `canonical_elf_sha256`
in `token-2022-v11.provenance`**, and evaluates five witnesses before folding.
