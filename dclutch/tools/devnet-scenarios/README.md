# Devnet economic scenarios

This tool generates three deterministic, exact-integer journeys for the
activity harness and its independent reconciler:

- `flagship.json`: a four-outcome categorical SOL/USD range market;
- `graduation.json`: a four-state relayed graduation market;
- `abandoned.json`: a graded exact-complement funded-failure partition.

Each journey uses four participant wallets plus a distinct logical deployer,
varied complete-set quantities, buyer and seller roles, partial and full Direct
fills, an exact 50-bps fee floor on each side, positive and zero terminal
payouts, complete claim burns, and a projected retirement-eligible state.
Amounts are decimal strings. There are no floats, private keys, key paths, live
addresses, signatures, RPC observations, or claims of devnet execution.

## Authority boundaries

The tool imports `DIRECT_FEE_DENOMINATOR_V1` from `dclutch-trading`; it
does not establish another fee denominator. It independently recomputes the
named quote boundary and the two side floors with checked `u128` intermediates.
The Product payout vectors are scenario inputs whose exact partition is checked;
the runtime still authenticates the real finalized Product basis and Resolution
certificate.

Canonical fixtures are `evidenceLevel: scenario-only`. Runtime-only fields are
null. Each body carries the exact public-devnet genesis hash used by the
successor's checked cluster admission; the runtime must read and authenticate
that identity again before it can bind or execute the scenario.
`callerAvailability` separates committed `public-executable` callers,
committed `preflight-only` planners, and `adapter-required` gaps. Direct and
retirement do not name uncommitted schemas; their `callerSchema` is null. Direct,
wallet payout planning, and retirement have `mutationExpected: false`, an empty
`expectedObservedDelta`, and a populated `projectedAcceptedDelta`. A harness must
not count any of them as an executed mutation. Once an executable caller is
available, the harness binds the canonical fixture into its own signed journal
rather than editing this evidence level.

Logical accounts deliberately have null addresses. The activity harness binds
addresses, market-input paths, ephemeral key paths, signatures, finalized slots,
transaction fees, certificate data digests, Position data digests, and exact
retirement rent refunds. The reconciler consumes those captured facts; it must
not infer them from these scenario projections.

`projectedAcceptedDelta.tokenDeltas` is the economic handoff row:

```text
{ walletRef|null, accountRef, mintRef, beforeState, afterState, deltaAtoms }
```

Wallet `fundingLamports` is a bounded activity request, not collateral and not a
projected ledger delta. Scenario fixtures leave all lamport deltas empty because
rent and transaction fees are chain-derived captured facts.

The abandoned scenario uses graded **resolution failure**, which the current
Claims terminal boundary consumes. Graded-success projection exists in the
schema but has no canonical journey until the public Claims caller consumes that
certificate directly; the fixtures do not disguise that gap.

## Reproduction

Run the locked, offline gate:

```sh
tools/devnet-scenarios/check.sh
```

The `generate` command accepts only a fresh absolute output directory. It writes
and fsyncs every file in a sibling temporary directory, validates the complete
bundle, then renames that directory and fsyncs its parent. It refuses an existing
target so a failed regeneration cannot overwrite accepted fixtures.

Each manifest envelope carries `bodySha256`, computed over the compact typed
`body` JSON under `canonical-compact-scenario-body-json-v1`. `SHA256SUMS` binds
the exact pretty-printed files. `check` requires the exact three filenames, the
sidecar, and byte-identical regeneration.
