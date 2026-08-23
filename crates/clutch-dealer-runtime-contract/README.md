# Clutch dealer runtime contract

Status: **STANDALONE SEMANTIC CONTRACT / ALL ACTIONS PLANNED AND DISABLED / NO
LIVE AUTHORITY** (2026-08-23)

This crate is the allocation-free, `no_std`, safe-Rust contract for liquidity
runtime slice A. It translates the selected covered signed-dealer research
model into strict versioned semantic bodies without adding a Solana route.

It owns:

- exact V1 bodies and hostile codecs for `DealerPolicy`, canonical facility
  genesis, the distinct external `DealerFacilityPosition`, its authority
  binding, the permanent root tombstone, `DealerState`, paged LP
  ownership, one-generation `Lease`, three-stage `SettlementPot`, `FeeBudget`,
  and `LivenessBudget`;
- `SHA256(domain || canonical_body)` content identities under fresh, trailing-
  NUL domains;
- Solana-compatible PDA seed recipes without importing or emulating the Solana
  SDK;
- exact payer/refundable-principal/donation-floor rent records;
- one policy-owned immutable neutral sink joined by every rent record and
  separately funded budget;
- the exhaustive DealerState child-count graph; and
- fail-closed enumeration of every planned action.

`DealerState` does not own assets. The adapter-authenticated **Facility Position
is the sole long-lived owner of dealer cash and Eggs while no lease exists**.
From successful Begin until atomic Finalize closes it, `SettlementPot` is the
sole transient selected-leg custody owner; the leased Position and pot are one
authenticated aggregate refinement, never mirror balances. Pot custody is
derived from exact immutable aggregates and monotone totals rather than stored
again. The pot contains no per-order allocation.

The fixed body map and PDA recipes are frozen in [SCHEMA.md](SCHEMA.md).

## Counted graph

`DealerStateV1` is the only count owner. Its exhaustive disjoint children are:

| Counted class | Cardinality | Meaning |
| --- | ---: | --- |
| Facility Position | `0..=1` | long-lived idle asset owner |
| Facility Replay | `0..=1` | Position replay companion |
| LP page | `0..=policy.maximum_lp_pages` | fixed ownership page |
| Live LP position | `0..=16*lp_pages` | active entry nested in pages |
| Unclaimed LP position | `0..=live_lp_positions` | terminal claim still open |
| Epoch binding | `0..=1` | sole active auction epoch |
| Lease | `0..=1` | exact selected generation lock |
| Settlement pot | `0..=1` | sole transient selected-leg custody |
| Fee budget | `0..=1` | separate fee liabilities |
| Liveness budget | `0..=1` | separately prepaid work/rent liabilities |
| Resolution/claim work | `0..=1` | bounded terminal work |

The immutable policy is a retained catalog reference rather than a counted
child. A child cannot retire before its named liabilities are terminal, and
the root cannot retire until every count is zero. The pure fold checks parent
identity, exact counts, rejects future-generation children, and requires the
active Epoch/Lease/Pot generation to equal the root generation. An adapter
must additionally authenticate unique child PDAs, nested page entries, and
account ownership.

Policy-catalog rent remains locked under the retained catalog's adapter-owned
account wrapper. This crate freezes rent ownership inside every mutable root or
deletable child body, but does not invent a global policy account tag or catalog
rent codec.

## Exact selected boundaries

- LP pages contain 16 strictly owner-sorted entries and chain by consecutive
  page ordinal. The 4,096-page semantic cap lifts the research model's
  `MAX_LPS=8` limit without allocation.
- A Lease binds one and only one `g -> g+1` transition, the exact Epoch, final
  `SettlementCandidateId`, quote, checked dealer-leg verdict, explicit curve-
  price-certificate ID, deadlines, Facility Position pre-state, pot, and both
  budgets.
- Settlement is input-first: Collect must become exact before Deliver; all
  outputs must be exact before Finalize atomically sweeps the residue, applies
  the one receipt, advances generation, and closes the Lease/Pot. There is no
  serializable post-sweep Pot phase. Strict contiguous
  cursors are the only replay truth; there are no redundant bitmaps. The pot
  checks `U_in + D_out = U_out + D_in`, one-directional dealer cash, and the
  exact `F_sell/F_buy` Egg custody equations at every phase. Same-outcome Egg
  inputs and outputs must already be netted. The Policy/State/Lease transition
  join independently recomputes `q'` and
  `ceil(C(q')) - ceil(C(q))`; a verdict digest alone is not cash authority.
- Fee and liveness principal are separately prepaid and exactly partitioned as
  available, reserved liability, spent, refunded, or sinked. There is no field
  for expected future fee revenue.
- Sponsor capital is a separate present amount whose refundable/donated/
  refunded disposition is explicit. The policy recomputes the selected loss
  bound and lower-corner bid-financing minimum.

## Not enabled

No global account tag, instruction tag, capability-profile membership, program
dependency, account meta list, or transfer path is allocated by this pure
crate. The separate SBF adapter now allocates a non-production, catalog-only
staged transport documented in
[`DEALER_POLICY_SBF_VERTICAL_SLICE.md`](../../docs/implementation/DEALER_POLICY_SBF_VERTICAL_SLICE.md).
That route persists an unadmitted immutable Policy and does not activate a
facility action.
The facility-genesis, Facility Position, authority-binding, and root-tombstone
codecs close the previous pure-core identity/retirement-shape holes, but no
global account allocation or SBF handler persists them yet. Existing
`DealerStateV1` is joined through an explicit initialization validator, not
silently reinterpreted.
`require_action_enabled` refuses every current action with `ActionDisabled`.
The price-certificate fields are binding slots, not a choice between the still
unresolved exact-divisibility and canonical-quantization profiles. Likewise,
`FeeBudgetV1` is only a segregated prepaid-liability ledger; it does not enable
nonzero fee direction, custody, recipient, rebate, or distribution economics.

The next adapter lane must separately land and review:

1. central account and intent allocation plus a new disabled-by-default
   capability identity;
2. exact SBF account codecs and metas for Policy, State, Facility Position,
   pages, Lease, Pot, budgets, Epoch, final candidate/verdict, price
   certificate, Clock, token accounts, system/rent, and payer/authority roles;
3. authentication of Realm/Profile, full MarketInstanceV2, claim basis,
   collateral mint/token program, Hoard custody semantics, RelationV2,
   price-measure, curve, fee, liveness, retirement, and quote authority;
4. atomic Begin transfer from Facility Position to sole transient Pot custody,
   row collection/delivery, final residue sweep back to Position, exact reloads,
   and hostile-prefund/rent routing;
5. rollback tests for every failed collect/deliver/finalize or count mutation;
   and
6. SVM and local-validator scenarios before any capability can be promoted.

## Verification

Run independently:

```sh
cargo test --manifest-path crates/clutch-dealer-runtime-contract/Cargo.toml
cargo test --release --manifest-path crates/clutch-dealer-runtime-contract/Cargo.toml
cargo clippy --manifest-path crates/clutch-dealer-runtime-contract/Cargo.toml \
  --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc \
  --manifest-path crates/clutch-dealer-runtime-contract/Cargo.toml --no-deps
```

The implementation depends only on `sha2 = 0.10.9` with default features
disabled. It imports no research crate, Solana SDK, Token-2022 type, oracle SDK,
FFI, allocator, float, or unsafe code.
