# Failure payout V1 research model

Status: **MODEL-ONLY / HOST-TESTED**. This dependency-free, allocation-free
Rust model encodes the R4 `EvidenceOnlyRecoveryV1` decision. It changes no
kernel, SBF program, account ABI, token mint, Realm, market, or release claim.

The model has no numeric data-failure payout. A missing source freezes new
exposure, spends only a finite separately prepaid SOL repair compartment,
sends failure residue to the canonical SDK incinerator, then leaves the market
dormant but recoverable by later valid evidence. Claims keep their original
meaning, complete-set merge stays exact, and abandonment is never inferred
from time. New native bearer token atoms represent a conservative universal
raw-claim lot divisible by the resolution denominator.

Current authenticated Token-2022 mint supply remains external bearer truth.
Ordinary holder burns reduce actual supply without paying anyone and leave
locked backing conservative. Imported numerator credits remain explicit
liabilities and prevent terminality; this V1 creates none. After all claims,
cash, credits, booked repair work, and terminal dependencies are zero,
remaining whole collateral is destroyed under a creation-time burn disposition
rather than paid to an interested sink.

Run:

```sh
cargo test --manifest-path research/failure-payout-v1/Cargo.toml
cargo test --release --manifest-path research/failure-payout-v1/Cargo.toml
cargo clippy --manifest-path research/failure-payout-v1/Cargo.toml \
  --all-targets --all-features -- -D warnings
```

The decision, equations, rejected alternatives, falsifiers, and minimum
onchain authorities are in
[`docs/implementation/FAILURE_PAYOUT_DECISION_V1.md`](../../docs/implementation/FAILURE_PAYOUT_DECISION_V1.md).
