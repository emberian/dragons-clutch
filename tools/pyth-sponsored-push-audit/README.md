# Sponsored Pyth push safety audit

## Verdict

**Current HEAD is not a safe drop-in. The design is conditionally safe.** The
legacy sponsored SOL/USD account can remove the Hermes bearer and ephemeral
post/reclaim lifecycle from the devnet flagship path, but only through a new,
immutable sponsored-push release/profile, permissionless candidate-seal path and
distinct Resolution consume instruction. Passing `7UVimff…` to the current
provider instruction must refuse.

The reason is structural, not cosmetic:

- the accepted `PythReleaseV1` pins the upgraded `rec2HH…` Receiver and Router,
  while the frozen sponsored account is owned by legacy `rec5EK…`;
- `PythReleaseV1` does not bind the push Program or its ProgramData;
- `PythTerminalOneTransaction` currently means Receiver post/reclaim transport;
- the current request requires a dClutch-owned `ProviderUpdateLifecycleV3`,
  post-body digest, ephemeral authority and precommitted update digest. The
  sponsored PDA has none of those, and a legitimate sponsor can mutate it
  between client preflight and transaction execution.

Those are four independent refusals. Relaxing any one would create an alternate
truth. The safe route authenticates one atomic upstream snapshot and seals it in
an immutable dClutch-owned candidate before the mutable account changes again.
It reuses existing Source/Product normalization and later performs the existing
single-write terminal transition from the canonical candidate head.

## Exact target

| Fact | Frozen value |
| --- | --- |
| Cluster | Solana devnet, authenticated by genesis hash |
| Account | `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` |
| Receiver owner | `rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ` |
| Push Program | `pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT` |
| PDA seeds | `[0u16 little-endian, SOL/USD feed id]`, bump `252` |
| Feed id | `ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d` |
| Body | SDK 2.0.0 `PriceUpdateV2`, 134 bytes, discriminator `22f123639d7ef4cd`, `Full` tag `1` |

`matrix.json` is the complete machine-checked trust-boundary matrix. Its 35 rows
cover cluster and release selection, feed/PDA/account/owner/provider program
identity, verification level, writer, publish slot/time, window,
freshness/skew, confidence, exponent and rounding, immutable snapshot identity,
funding and cleanup, candidate ordering and closure, replay, substitution,
closing, upgrades, sponsor liveness, single-answer finality, fast-path and
failover refusals, rent and privileges.

## Exact arithmetic and time

There is no decimal conversion in this route. `PythAdapterConfigV1` admits the
raw signed price only when the exponent exactly equals the Market's committed
exponent, then widens `i64` to `i128`. Product cuts use that same integer unit.
Confidence is the inclusive checked inequality
`confidence * 10_000 <= abs(price) * max_confidence_bps`. No float and no second
rounding boundary exist.

The two time predicates remain separate:

1. `WindowSpec[start,end]` says whether the price is about the period sold.
2. `[Clock-now-max_age, Clock-now+max_future_skew]` says whether devnet will
   still act on it.

The sponsored account is latest-value mutable storage, not an archive. A public
watcher may submit any currently admissible sample without Hermes. One immutable
candidate PDA is derived from the Market, generation, release, price account,
`publish_time`, `posted_slot` and full-body digest. The candidate stores the
exact 134 bytes and its admission Clock facts once. It cannot update or close
while Source is Primary. External sponsors fund candidate/head rent and fees;
Hoard principal and future fees are never available.

One head per `(market, generation, release)` advances only to the greatest
submitted candidate under the exact order
`(publish_time, posted_slot, body_digest)`. This is the **best valid submitted
candidate**, not provider-latest and not an optimal price. Multiple candidates
are necessary: a one-record cap would let the first submitter choose the answer.
Market and generation in every identity prevent cross-market reuse. Terminal
cleanup refunds each record to its stored sponsor only after Source leaves
Primary.

Candidate-set closure always has a provider-independent rule. Let
`primary_deadline = window.end + maximum_age`. Candidate admission is inclusive
through the deadline; after Clock is strictly greater than it, no in-window
price can pass the existing freshness predicate. The terminal instruction then
consumes the head without re-aging the sealed observation, or takes the funded
failure route only after authenticating that the head is vacant. This prevents
failure from racing a submitted answer.

An authenticated upstream publication strictly after `window.end` may close
the set earlier only if the exact deployed push ProgramData/ELF is reproducibly
bound to source that enforces strict publish-time increase and an exact-ELF
equal/older-time hostile passes. The generic Receiver account update path does
not itself compare the previous stored time, so Receiver provenance, SDK layout,
`Full` verification and `prev_publish_time` are not enough. Without that exact
push proof, early closure refuses; the Clock deadline remains safe. A direct
same-transaction consume is at most a separately founded optional fast path,
not the durable flagship route.

The bounded 2026-08-28 observation supplied to this audit saw 100 finalized
account signatures over 1,163 seconds, no transaction errors, with transaction
interval median 9 seconds, mean 11.75, p90 34 and max 50. This is account
activity only, not decoded historical `publish_time` cadence and not a future
liveness bound. The advertised one-minute heartbeat / 0.5% deviation policy is
also not an onchain liveness guarantee.

## Provider provenance boundary

Official OP-PIP-5 names the legacy Receiver deployment and two relevant Receiver
source commits:

- <https://github.com/pyth-network/governance/blob/main/docs/pips/operational/op-pip-5.md>
- <https://github.com/pyth-network/pyth-crosschain/commit/f79f205895de61ddec69ae3ed6d4bd1ca1c6542f>
- <https://github.com/pyth-network/pyth-crosschain/commit/1e5df8537adbecf300fa51a8b9330db754950a05>
- <https://github.com/pyth-network/pyth-crosschain/commit/a607335>
- <https://raw.githubusercontent.com/pyth-network/pyth-crosschain/4c6bac1/target_chains/solana/programs/pyth-push-oracle/src/lib.rs>

They are provenance leads, not a deployed-ELF semantic proof. The generic rec5
Receiver source assigns an authenticated message into the account; the strict
increase property relevant to this fixed self/PDA-writer account comes from the
separately pinned push Program. Official push source at `a607335` introduced the
idempotent condition `next_timestamp > current_timestamp`; the inspected
`4c6bac1` tree retains it and skips Receiver CPI for equal or older input. That
source check is necessary but not sufficient. The new release still must bind
both Programs, both ProgramData accounts, deployment slots, authorities and
accepted ELF digests, and the exact deployed ELF must pass the rewind hostile.
Slot drift is `ReleaseSuperseded`, never an implicit downgrade or fallback to
the upgraded sponsored account.

## Run

```sh
tools/pyth-sponsored-push-audit/check.sh
```

The offline check validates the exact matrix and every cited repository path,
reproduces the pinned PDA, parses the fixed SDK body, and runs hostile owner,
address, writer, feed, Full/Partial, length, tail, slot, time, window, freshness,
confidence, exponent, closing, digest-race, monotone-head, deadline, rewind and
head-vacancy cases. Its synthetic price body is test input only; it is not
devnet evidence and the tool is not a protocol semantic owner.
