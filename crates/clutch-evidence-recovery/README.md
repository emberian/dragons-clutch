# Evidence-only recovery runtime projection

Status: **PURE KERNEL / HOST-TESTED / NOT AN SBF ADAPTER OR LIVE ABI**.

`clutch-evidence-recovery` owns mutable phase, accepted-progress, reserve,
rent, donation, and transition-plan semantics for `EvidenceOnlyRecoveryV1`. It
does not own a second recovery policy.

The sole reusable schedule owner is the path-pinned `clutch-product-series`:
its `EvidenceOnlyRecoveryPolicyV1` and `compile_ordinal` derive up to
eight ordered absolute `(repair_generation, open_bucket, close_bucket)`
attempts. This crate imports the exact `CompiledScheduleV1` type and binds both
the typed recovery-policy ID and `MarketInstanceId`. An adapter must
authenticate the Template, Market, policy, compiler output, and exact lowering
before admission. The runtime projection is not another persisted policy
truth.

Work pricing belongs to the separate FundingQuote attachment, not the Market
identity. `clutch-product-series::SeriesFundingQuoteV1` is its sole semantic
owner: its content ID commits the recovery-policy ID, active attempt count,
every progress cap/rate row, recovery rent, and aggregate component debit. The
recovery kernel consumes that exact type directly, derives work principal from
its checked rows, and refuses admission unless the presented quote hashes to
the typed ID authenticated through the Series attachment. Thus two valid rate
allocations with the same aggregate cannot share an admitted identity. The
authoritative quote also refuses a nonzero recovery collateral component; a
future collateral custody/disposition design requires a successor funding
owner/API rather than an untracked field.

The crate is `no_std`, fixed-memory, allocation-free, safe Rust, and uses only
checked integer arithmetic. Its local lockfile isolates the path dependency
and that crate's pinned dependency graph from every SBF workspace.

## Runtime boundary

The adapter still owns all authority:

- `RecoveryClock.current_bucket` must be the exact mapping of an authenticated
  canonical Clock into the authenticated SourceSpec grid. The kernel checks
  slot, Unix-time, and bucket monotonicity plus inclusive-open/exclusive-close
  boundaries; it does not parse a sysvar or derive a grid bucket.
- `EvidenceDecision` is only a nonzero opaque identity. The kernel does not
  authenticate evidence, select a source, or compute a payout.
- State generation freshness and permanent nonreuse require adapter PDA,
  tombstone, and counted-retirement checks. A nonzero generation is only a
  transition-plan binding inside this crate.
- `state_id` names the reserve/logical recovery instance that is debited by a
  plan. It must be distinct from the work funder, rent payer, neutral sink, and
  each progress-reward recipient; otherwise a terminal zero-balance plan could
  self-credit its own source account. Recipient-to-recipient aliasing remains
  valid and must be coalesced exactly by the adapter.
- The rent ledger covers only the expendable reserve account. The logical
  `RecoveryState` must remain durably available after that reserve reaches a
  zero balance (for example in an independently funded market root or
  tombstone), because late evidence still needs the dormant disposition and
  generation. This crate neither selects nor funds that persistent carrier.
- `neutral_sink` is an abstract role. A production adapter must bind it to the
  canonical SDK incinerator and reject every interested-role alias.
- Transfer entries are disjoint accounting compartments, but their recipients
  may alias. The adapter must aggregate by authenticated address, precompute
  checked recipient balances, execute all transfers atomically, verify every
  exact delta and the reserve post-balance, then commit the planned state.
- The adapter may advance a progress cursor only for accepted replay-protected
  onchain work under the authenticated FundingQuote. An offchain assertion,
  failed transaction, zero progress, or repeated cursor earns nothing.
- Each Work/evidence path must bind the recovery generation, current attempt
  index, and that attempt's exact compiled source repair generation to the
  authenticated Terms/SourceSpec successor chain. A stale Work or a source
  from another attempt is not accepted progress; there is no generation or
  numeric fallback.
- The adapter must authenticate the typed quote ID as both the canonical
  `SeriesFundingQuoteV1::id()` and its `SeriesAttachmentPlanV1` reference. It
  must also authenticate the bound Market and full recovery-policy artifact;
  the kernel independently checks the expected policy ID and attempt count,
  preventing replay of one component across different recovery semantics.

This crate defines no failure payout, Hoard funding, fee, future revenue,
treasury input, Solana account tag, codec, PDA, CPI, Token-2022 type, source
provider, or liveness promise.

## Phase and anti-grief rules

The mutable phase machine is:

```text
Active -> DegradedRecoverable -> RecoveryDormant -> Resolved
```

Evidence may resolve before dormancy as well. New exposure checks both phase
and the current authenticated bucket, so crank delay or a delayed first repair
window cannot keep exposure open after primary evidence maturity.
The phase/schedule planning APIs take no privileged authority identity, so an
adapter can expose them permissionlessly after authenticating the presented
accounts, Clock, Work, and evidence inputs.

There is at most one recorded Work, but recording/replacing it is atomic with
strictly positive newly accepted progress and its exact payment. A free
zero-progress Work cannot occupy the slot. A different Work with later
accepted progress may replace a stalled Work, so the previous identity is not
a liveness lock. Final attempt close ownership is derived from current bucket
time even if no dormancy crank landed first.

After the final exclusive close, unused work principal and donations go only
to the neutral sink, while rent returns to its exact payer. Before that close,
evidence success returns unused work principal to its exact funder. Later
caller-funded evidence can resolve a dormant market without recreating a work
budget. Any hostile lamports sent to the closed reserve are classified only as
new donations and neutralized in that same late-resolution plan, so one
lamport cannot grief recovery.

## Conservation

```text
work_initial
  = work_remaining
  + accepted_progress_paid
  + success_refunded
  + dormancy_neutralized

rent_initial = rent_remaining + rent_refunded

donations_received = donations_remaining + donations_neutralized
```

While open:

```text
reserve_balance = work_remaining + rent_remaining + donations_remaining
```

Admission checks the work-funder and rent-payer debits individually. Prior or
later donations cannot cover either debit and never become a refund.

## Evidence commands

```sh
cargo test --manifest-path crates/clutch-evidence-recovery/Cargo.toml --offline --locked
cargo test --release --manifest-path crates/clutch-evidence-recovery/Cargo.toml --offline --locked
cargo clippy --manifest-path crates/clutch-evidence-recovery/Cargo.toml \
  --offline --locked --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc \
  --manifest-path crates/clutch-evidence-recovery/Cargo.toml \
  --offline --locked --no-deps
cargo fmt --manifest-path crates/clutch-evidence-recovery/Cargo.toml -- --check
```

Host compatibility follows the repository Rust 1.89.0 pin. Code and
documentation are original greenfield AGPL-3.0-or-later work.
