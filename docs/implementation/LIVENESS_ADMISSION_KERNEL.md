# Prepaid liveness admission kernel

Status: **PURE KERNEL / HOST-TESTED / NOT RUNTIME-INTEGRATED**.

The implementation is
[`crates/clutch-liveness`](../../crates/clutch-liveness). It promotes the exact
integer identities from commit `13490e7` into safe, fixed-memory, `no_std`,
`no_alloc` Rust. It does **not** select an economic policy, measure an SBF path,
move lamports, authenticate an account, guarantee inclusion, or change a release
STOP. The Solana adapter and persisted account layouts remain future work.

## Boundary

The crate accepts only native-lamport funding that is present at the transition.
There is deliberately no argument for:

- Hoard or claim collateral;
- venue fees, treasury atoms, or future order volume;
- a service token, token price, swap, or token-to-SOL conversion;
- an insurance promise, emission, buyback, or future subscriber;
- an RPC, signer, account balance, clock, or oracle.

Consequently none of those quantities can repair a one-lamport shortfall. An
adapter must fund the dedicated accounts before calling the corresponding pure
transition. `Id` values are only fixed byte identities here; the adapter must
authenticate their signer/PDA meaning.

## Frozen compartments

`LivenessPolicy` has five independent, strictly positive maxima:

| compartment | purpose | terminal destinations |
| --- | --- | --- |
| market work | observation/repair/cleanup work frozen by the Realm | keeper plus payer refund, or keeper plus neutral failure sink |
| market storage | rent principal for mandatory market state | locked storage plus payer residue; valid closure returns the same principal to the payer |
| resolution | final observation/finalization work | keeper plus payer refund, or keeper plus neutral failure sink |
| per-order clear | unavoidable clearing/lapse work added by one order | keeper plus payer refund, release, or neutral failure |
| per-order settle | unavoidable settlement work added by one order | keeper plus payer refund, release, or neutral failure |

The policy rejects a zero maximum on any mandatory compartment. This is a
spam/admission rule, not evidence that any particular positive number is enough.
Every maximum remains an **unmeasured policy input** until a final instruction
shape has reproducible SBF measurements and a separately reviewed safety
margin. No fee rate or keeper rate is chosen by this crate.

`MarketEndowment::admit` checks market work, storage, and resolution deposits
component by component. Excess is immediately and explicitly owned by the
admission payer as `admission_residue_lamports`; excess in one compartment never
excuses underfunding another. `OrderEndowment::admit` reserves both per-order
maxima even if the signed intent's venue fee is zero:

```text
order funding >= max_clear_work + max_settle_work
```

There is no fee input to this equation. Optional zero-fee work may be paid by
its transaction submitter; work the protocol promises to finish is prepaid by
admission.

## Transition identities

A one-shot `WorkReservation` begins with maximum `M`. Exactly one of these
terminal relations may occur:

```text
success:         M = keeper_paid + refundable_residue
release:         M = refundable_residue
neutral failure: M = keeper_paid + neutral_residue
```

`keeper_paid <= M`. A second terminal transition and an over-maximum payment
refuse. A release function does not decide that cancellation is legally or
economically permitted; the adapter must prove the relevant order/market has no
remaining liability before selecting it. The kernel only makes the accounting
result total and replay-resistant.

Storage is a separate relation:

```text
admission: M_storage = locked_principal + immediate_refund
closure:   returned_principal = locked_principal
```

The returned principal joins the payer's refundable residue. It never enters a
keeper or treasury field. External account donations are outside this equality
and need a separately named adapter policy rather than being mislabeled as rent
principal.

For both a market and an order, `accounted_lamports()` checks the aggregate
identity:

```text
funded = pending reservations
       + locked storage
       + keeper payouts
       + payer-owned refunds
       + neutral-failure residues
```

All additions use checked `u64` arithmetic. Admission rejects a combination
whose aggregate cannot be represented.

## Shared source and archive reserves

Source delivery and authenticated archive/replay are independent shared-feed
reserves. `SharedFeedPair` joins them as one pure value transition, so a caller
cannot receive source credit after failing the archive deposit. A real adapter
must preserve the same atomicity across lamport transfers and account writes.

For reserve cap `B` and `k > 0` arrival-order subscribers, shares are canonical:

```text
q, r = divmod(B, k)
share_i(B,k) = q + 1 when i < r, otherwise q
sum_i share_i(B,k) = B
max_i share_i - min_i share_i <= 1
```

The first subscriber deposits all `B`; there is no assumed second subscriber.
Subscriber `k+1` deposits its new last share:

```text
deposit_(k+1) = floor(B/(k+1))
              = sum_i (share_i(B,k) - share_i(B,k+1))
```

That deposit reimburses incumbents and the active reserve stays exactly `B`.
Deposits must be exact because a surplus has no owner in this relation.
Duplicate identities, fixed-array exhaustion, and a join whose canonical share
would be zero all refuse. The zero-share rule intentionally hardens the original
offline model: it prevents unlimited free subscriber/account growth once
`k > B` and also refuses a zero-cap feed.

For successful terminal keeper spend `A <= B`:

```text
cost_i   = share_i(A,k)
refund_i = share_i(B,k) - cost_i
sum_i cost_i = A
sum_i refund_i = B - A
cost_i + refund_i = subscriber_i's current net capital
```

On terminal failure, subscriber refunds are zero, the keeper may receive at
most `A`, and `B-A` is assigned to the immutable neutral sink. Per-subscriber
economic cost is the full capital share while the physical reserve splits as:

```text
B = keeper_paid + neutral_residue
```

The sink must be nonzero and cannot be the market owner/subscriber in this
kernel. The adapter must additionally prove it is not a resolver, claimant,
maker, executor, treasury, or other interested recipient. Naming a sink is not
evidence that its policy is socially or legally acceptable.

## Persistent fee carry is not liveness capital

`IntentFeeCarry` gives one `(owner, intent_id)` pair a frozen positive
denominator and persistent remainder. Each fragment applies:

```text
accumulated = old_remainder + exact_fragment_numerator
floor_charge, new_remainder = divmod(accumulated, denominator)
```

Closing the intent once charges one terminal atom exactly when the remainder is
nonzero. Wrong-owner, wrong-intent, overflow, reset-after-close, and reopen
attempts refuse. The result is invariant to bounded fragmentation.

The fee carry has no method that funds a `WorkReservation`, `StorageReservation`,
or shared feed. It exists here only to freeze ownership at the liveness/fee
boundary: abandoning a fractional carry cannot erase the terminal ceiling, and
collected fee atoms never become evidence that mandatory SOL work was prepaid.
The actual fee basis, coefficient, denominator bounds, and allocation rates are
still proposed/unmeasured policy.

## Executed evidence

The crate's host tests cover:

- independent one-atom underfunding on every market axis;
- success, release, neutral failure, replay, and over-payment refusals;
- storage lock/close conservation and return mismatch;
- zero-fee order underfunding and clear/settle conservation;
- overflow and zero-mandatory-maximum refusal;
- exhaustive shared-feed caps `1..=32`, up to 16 subscribers, and every keeper
  spend `0..=B` while a positive next share exists;
- join reimbursement, success cost/refund, and terminal-failure identities;
- duplicate, zero-share, zero-cap, neutral-sink, and capacity spam refusal;
- atomic source/archive contribution checks;
- fee-carry ownership, terminal closure, and bounded fragmentation for
  denominators `1..=31` and exact totals `0..=127`.

Run locally:

```sh
cargo test --manifest-path crates/clutch-liveness/Cargo.toml
cargo clippy --manifest-path crates/clutch-liveness/Cargo.toml --all-targets -- -D warnings
```

Passing these tests establishes the pure arithmetic identities only. Promotion
still requires a canonical account encoding, hostile-byte parsing, authenticated
funding accounts and destinations, atomic System-program transfers, replay
protection, measured final SBF paths, account-rent evidence, formal arithmetic
refinement, and real-SBF adversarial tests. A finite prepaid maximum covers a
finite promised payment; it does not guarantee inclusion under unbounded
congestion or censorship.
