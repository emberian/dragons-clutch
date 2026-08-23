# SourcePlane V3 core

This crate is the first allocation-free promotion target for recurring Dragon's
Clutch products. It is intentionally not wired into the live SBF dispatcher.
It freezes pure state and compiler semantics that an account adapter can later
wrap without importing Solana, oracle, token, CPI, or account-memory types into
the core.

The crate is `no_std`, uses no allocator or `unsafe`, and exposes exact-length
canonical codecs. SHA-256 content identities use named domains and frozen body
bytes. Legacy and future versions both refuse unless their exact version and
capability set is registered.

## State ownership

| Object | Semantic owner | Deliberately absent |
| --- | --- | --- |
| `SourceHeadV3` | SourceSpec, repair generation, page/boundary cursor, source lineage | Realm, Terms, Template, window, evaluator |
| `OpenRawPageV3` | Persistable one-boundary-at-a-time ingestion work | Product or statistic facts |
| `RawPageV3` | Immutable normalized source records and page-chain position | Window, Realm, statistic |
| `WindowSpecV3` / WindowKey | SourceSpec, reviewed SourcePlane, exact range/maturity/generation/coverage | Realm and evaluator |
| `WindowWorkV3` | Resumable rolling record root and exact page/range cursors | Opaque hash-library state |
| `WindowSealV3` | Final raw evidence content and authenticated maturity-page closure | Statistic value |
| `StatisticKeyV3` | Predictable request: WindowKey + SummaryProgram + statistic | Result bytes |
| `StatisticResultV3` | Exact WindowSeal plus successful payload or stable refusal | Caller-chosen unit/count copies |
| `ProductTemplateV3` | Relative product semantics and content references | Absolute window and liabilities |
| `InstanceDescriptorV3` | Template/Realm/policy references, absolute start, liability cap | Series, ordinal, nonce, derived ranges/amounts |
| `SeriesFundingV3` | Exact-next cursor and segregated prepaid creation/liveness/liquidity balances | Future fees and claim principal |

`SourceSpecId` remains the sole source-description identity. This core does not
persist a second adapter/grid DTO. A normalized source adapter must authenticate
the existing SourceSpec before supplying records.

## Real-ingestion route

`SourceHeadV3::open_page` creates persistable open-page work. A verified source
adapter appends one observation per authenticated boundary transaction. Source
sequences may repeat or jump; they must not regress, and a repeated sequence
must carry the exact same source body. Publication slot and time are retained.
The open prefix is sealed into an immutable 32-slot page and committed to the
exact head atomically. Later observations begin a new page, so a Window ending
inside the previous page never depends on a mutable-tail commitment.

Gap records and bounded-gap coverage deliberately refuse in this release. A
future version needs a non-forgeable adapter closure capability before an
absence can become authenticated data. `FAIL_EXTENDED_WINDOW_02` likewise
refuses: the current Template has no immutable successor-window chain or
extension count capable of executing that name.

## Recurrence and funding

`compile_instance` performs the full SourcePlane/SummaryProgram/partition/
payout/Template/Series join. It validates the final scheduled maturity up
front. Independent Series converge when their exact semantic Instance
descriptor converges; Series identity, ordinal, creator, and arbitrary nonce do
not enter Instance identity.

Series activation requires exact finite prepayment of every creation/rent,
liveness, and liquidity allocation. Creation is permitted only in
`[start - lead, start)`. Lapse after `start` and advancement over an already
existing identical Instance consume no allocation; those unused balances stay
visible for terminal refund.

The core validates the actual selected payout data for
`FAIL_UNIFORM_REFUND_01`: positive equal active weights, exact denominator
divisibility, valid failure index, and zero inactive padding. It does not claim
that the legacy runtime already enforces this invariant.

## Evidence

- `cargo test --manifest-path crates/clutch-source-plane-v3/Cargo.toml --locked`
- `cargo clippy --manifest-path crates/clutch-source-plane-v3/Cargo.toml --all-targets --locked -- -D warnings`
- `vectors/source-plane-v3.json` freezes canonical bodies, identity domains,
  hostile byte mutations, and drawdown rounding cases.

The remaining adapter obligations are account owner/PDA/tag/version/bump
authentication, source-specific release and lineage validation, canonical
Clock boundary admission, atomic account writes and rollback, and independent
SVM vectors. Until those exist, this crate is production-bound core evidence,
not a live SourcePlane deployment.
