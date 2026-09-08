# General loopback Found/root addendum — 2026-09-08

This dated addendum supersedes the final conclusion in
`GENERAL_LOOPBACK_SOURCE_SPLIT_2026_09_07.md`. The earlier source-split
continuation stopped at `SponsorUnderfunded`; projecting every publication and
terminal debit was not required to exercise the local fixture. The incomplete
whole-cost projection was removed in `a8f99a9b4`.

The fresh local-validator run used the established relayed-vertical fixture
setup: one finalized `requestAirdrop` grant of 500,000,000,000 lamports to the
campaign payer only. It is recorded separately from campaign transactions at
`/tank/dregg-build/general-a7-fresh-found-20260907/general-founding-payer-grant.json`:

- payer: `AvdDne13Q3U3ahoYeQrbqJa2sKsjjrjAeU2rHzmNuAD3`
- signature: `YzPphiGV5xGyh5WY6xy7EmcU4XKTjxCUuUBwdjhchQ6ihpPXPtN21jJhRE3JwKavcLNQnonaf36LQep8xsJjtbA`
- balance: 890,880 to 500,000,890,880 lamports

No protocol-created founding role received a fixture grant. The collateral,
founding supplier, rent-credit, Custody, and ledger accounts were created or
funded only by the ordered protocol transactions recorded in the campaign
evidence.

On the fresh ledger at `/tank/dregg-build/general-a7-fresh-found-20260907`
(loopback RPC `http://127.0.0.1:22274`, genesis
`6kAr6wYNbkPUYoF3YjrARNJP2eGQJoTopewDdtTwpwL9`), the canonical General
compiler input `general-market-c94df436.json` had SHA-256
`49e98d25e6b8eff09d323c6de343aeb71f763e8ffc8d0db92dec8c1e38d8169b`.
The execution submitted 559 transactions and opened Found/root Market
`5gtcAGXAqqdRU7oEK4B9oTT7rbve7uvAxpdteoCR5a7P`. Its full transaction and
poststate artifacts are, respectively,
`general-founding-execution.json` and `general-founding-poststate.json` in
that directory. The latter is a read-only preflight that returned exit status
zero and marked substrate, publication, initialize, succession, activation,
and founding all `complete`.

This remains a source-split local-validator diagnostic, not release proof:
the runtime artifacts are from checked gate
`6d02cebd1980ce2d84d6dc7b33533e6f40a2044a4159b5bf95334bcb6b467abe`
(runtime source `a7f9f226d13e9fba49d746f64f5a2f34f43d0dd4`) and the host
binary came from `f153cbbd38264565ac8e8ac86071e10f02a670bf`.
