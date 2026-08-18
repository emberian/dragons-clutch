# Terms and evidence closure

Status: **PROPOSED join map**. The accumulator, distributional-claims, and
resolution documents own their detailed state machines; this file identifies
what continuous claims and liquidity policies must bind before activation.

## Immutable market terms

The market terms digest must cover:

- collateral profile, mint/program identities, decimals, and extension policy;
- outcome domain, finite partition or basis, knots/cells, denominator, edge
  behavior, and maximum outcomes;
- source adapters, feed/account identities, source generations, confidence and
  freshness rules;
- statistic, time domain, sampling cadence, coverage/gap requirements,
  ambiguity/failure policy, maturity, repair grace, and finality convention;
- payout compiler/version, rounding, approximation norm/bound, and exact payout
  vector derivation;
- batch relation, price scale, allocation, self-cross, fee, and settlement
  policies;
- admitted liquidity-policy families and all reserve/withdrawal limits; and
- upgrade posture, activation slot, and immutable code/artifact digests.

No live-market governance action may replace these fields. A changed field
creates a new market/version.

## Evidence chain

Resolution requires a closed chain:

```text
authenticated source bytes
  -> admitted observation receipt
  -> ordered accumulator transition
  -> sealed WindowResult
  -> statistic/ambiguity decision under frozen terms
  -> exact basis weight vector
  -> kernel resolution receipt
```

Every arrow binds its predecessor's exact digest, occurrence identity, time/cut,
source generation, and authority. A source brand, caller-supplied number, or
syntactically valid digest is not evidence closure.

For path predicates, the accumulator must retain enough authenticated summary
state for the frozen predicate family. A constant-size generic accumulator
cannot answer arbitrary post-hoc path questions. Shared feeds amortize one
observation across markets; they do not remove the information lower bound.

## Gap and ambiguity semantics

Terms choose one complete behavior for missing, corrected, stale, conflicting,
or unavailable evidence. The system first permits permissionless repair within
the funded grace period. After the deadline it follows the frozen compatible-
outcome or refund/haircut rule. An equal-outcome fallback is not presumed safe:
tail holders can profit from inducing ambiguity.

Unused liveness endowment must not reward a party capable of causing failure.
No new market may activate unless every remaining mandatory observation,
finalization, and cleanup job is booked at its maximum SOL and token bounty.

## Liquidity-policy binding

A quote or LP tranche additionally binds:

- the exact market terms and claim artifact;
- the tranche's reserve and inventory generation;
- valid batch interval and expiry;
- all-in limit/quantity schedule;
- fee/rounding/carry policy; and
- withdrawal/settlement priority.

Resolution evidence never authorizes a quote, and a quote never authorizes
resolution. These are separate state machines joined only through immutable
identities.

## Static-client trust boundary

GitHub Pages or IPFS may serve the client, but the client is untrusted. It must
derive canonical addresses, display program/version/upgrade authority and exact
terms, construct transactions, and verify returned account bytes locally.
Consensus does not depend on the origin, availability, or honesty of any hosted
UI, RPC endpoint, or indexer.

## Release evidence

Before activation, publish exact terms bytes, program and toolchain identities,
reproducible-build record, account-layout digest, source-adapter vectors,
accumulator/window goldens, claim compiler goldens, adversarial failures, formal
theorem statements and assumptions, and a deployment manifest. “Verified” must
name the theorem/tool/source revision and every unverified adapter boundary.
