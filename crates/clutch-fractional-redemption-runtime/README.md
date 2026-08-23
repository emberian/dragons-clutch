# Exact fractional-redemption runtime contract

This crate promotes `research/fractional-redemption` into a safe, `no_std`,
allocation-free, fixed-layout runtime contract. The SBF adapter contains the
complete action-2 exact-internal, action-3 exact-bearer, and action-9
claims-exhausted handlers, but their capability remains disabled
until Product's canonical Hoard/ClaimLedger founding, Resolution activation,
and family-admission account producers land. Intent family `79/v1`, actions
`1..=10`, and account coordinates
`0xa4/v2`, `0xa5/v1`, `0xa6/v2`, and `0xa7/v2` are centrally reserved as
`ReservedDisabled`.

The new persisted facts have one owner each:

| account | owner | exact body |
| --- | --- | ---: |
| `0xa4/v2` | immutable Market/Resolution-V5-data/Realm/claim policy and resolved common lot | 296 |
| `0xa5/v1` | ClaimLedger account binding, aggregate numerator `K`, live-credit count, and global replay sequence | 224 |
| `0xa6/v2` | one claimant's canonical numerator `<D`, generation, replay, and rent | 296 |
| `0xa7/v2` | permanent zero-credit close/reopen identity | 232 |

The never-activated `0xa4/v1`, `0xa6/v1`, and `0xa7/v1` coordinates are
explicitly withdrawn. Their identity slots were allocated as payout-vector
digests, so no V2 decoder accepts them and no migration or fallback aliases
them. V2 uses fresh policy/credit PDA domains; unchanged `0xa5/v1` continues to
own only aggregate credit, live-credit count, and its cross-account sequence.

Resolution V5 remains the sole vector owner. The policy and every owner credit
persist its exact PDA-bound Resolution data ID, while each transition also
recomputes the body-only semantic ID and returns the V5 quotient/remainder
projection that names the exact outcome and burned quantity. Full-width ClaimLedger V3
remains the sole internal-plus-bearer supply owner. Hoard V2 remains the sole
owner of locked claim principal and Position-cash collateral classification.
Position V3 and its purpose-owned Replay V3 remain the only internal
custody/replay bodies. The Realm collateral and independent Token-2022 claim
contracts remain the only CPI authorities. Every mutation commits exact
`0xa5` pre/post semantic IDs into the matching ClaimLedger successor; their
sequences cannot advance independently.

ClaimLedger V3 begins in the explicit fractional `OpenUnlatched` state with
zero policy/ledger identities. This fractional state is distinct from the
Market liability lifecycle and survives Resolution activation unchanged.
Only `Initialize` may move it once to `Latched`: that transition stores the
exact a4/v2 and a5/v1 accounts, advances sequence zero to one, and emits the
private child receipt consumed by Product's five-family Market aggregator.
No credit liability or fractional action can exist before the latch, and no
Resolution transition may populate or relatch these identities.

Internal actions consume the canonical General `GEN1` Replay extension rather
than a Fractional-owned replay projection. Its frozen family/action/role
coordinates are `(4,2,1)`, `(4,4,1)`, `(4,6,1)`, and `(4,7,1)` for exact
redemption, credited redemption, credit-transfer payout, and credit-merge
payout respectively; every tuple uses transition version `1`.

Every redemption and credit transfer checks both its prospective prestate and
poststate against

```text
D * claim_backing_atoms
  >= weighted_remaining_native_claims + aggregate_credit_numerator.
```

Exact lots take a zero-credit fast path. Arbitrary quantities use one owner-
scoped numerator credit; mixed outcomes aggregate under the same exact
Market/Resolution/payout/generation domain. Credit transfers are custom
same-domain operations rather than a second bearer mint.

Resolution V5's direct bearer route remains exact-only. A nonzero V5 remainder
is not a permanent amount restriction: it enters the credited Fractional route,
whose single plan atomically binds the bearer burn, a5/owner-credit successor,
ClaimLedger/Hoard successor, exact whole payout, and retained numerator.

Whole internal payouts reclassify Hoard V2 locked principal into Position-cash
liability without moving token custody. Whole external payouts require the
accepted Realm-selected claim-redemption CPI receipt and bind its transition,
semantic owner, amount, and destination. A zero payout changes neither Hoard
classification and admits no external CPI receipt.

The exact bearer adapter orders its two external effects. Fractional first
prepares a private `0xa5`/ClaimLedger/Hoard successor. The independent claim
adapter must then accept the exact selected Token-2022 mint and source-account
burn before the Realm collateral request becomes visible. After collateral is
accepted, one final capability exposes all three poststates for atomic
writeback. Runtime-observed mint supplies may synchronize materialized supply
downward to recognize direct holder burns, but never upward; the full observed
vector is committed by the ClaimLedger transition and the post-burn vector
must differ at exactly the selected mint by exactly the requested quantity.

The only terminal policy is `RetainUntilExactAggregation`. If all native claims
are gone but aggregate credit is `D*A+r`, voluntary aggregation can pay `A`
whole atoms. When `r != 0`, the remaining credits and claim backing stay live.
The close route requires claims, aggregate credit, live credit accounts, and
claim backing all to be zero. It closes the policy and aggregate ledger only
under the matching private Product five-family terminal authorization,
refunds their stored rent payers independently, and sends only excess lamports
to the neutral sink. It therefore cannot sweep a final Hoard atom, reinterpret
donation surplus as revenue, strand policy rent, permit reinitialization,
invent a reserve, or silently forfeit a claimant numerator.

The Fractional-owned terminal receipt commits the full a4/a5 and ClaimLedger
terminal tuple, both exact rent splits, and a separately adapter-authenticated
Fractional runtime/capability release ID. Product consumes that receipt; it may
not invent the release, substitute the Realm collateral release, or turn the
pure close plan into authority. Fractional now exposes only crate-private SBF
postwrite capabilities: the admission capability authenticates the exact
writable a4/a5/ClaimLedger founding bodies and PDAs, while the terminal
capability authenticates the exact Retiring ClaimLedger body, both live
pre-deletion account bodies, both observed rent balances, and an action-10
release narrowed from the loader-authenticated registry capability. Product's
consumer can accept those private values but cannot construct them or replace
their receipt. The shared actions 1 and 10 remain disabled until Product lands
the atomic aggregator/root consumers and its private Foundation preallocation
authority.

## Solana activation boundary

The frozen future account order is:

- `Initialize`: payer; MarketInstance; Realm; collateral Profile/policy;
  Resolution; claim-issuance binding; policy PDA; ledger PDA; ClaimLedger V3;
  System Program; Rent sysvar; neutral sink; capability/release manifest.
- Exact internal redeem: owner; Realm; collateral Profile/policy/program;
  MarketBinding; MarketRuntime; MarketInstance artifact; Hoard V2; ClaimLedger
  V3; Resolution V5; fractional policy; aggregate ledger; Position V3; GEN1
  Replay. Credited form appends
  credit/tombstone, payer, System Program, and neutral sink.
- Exact bearer redeem: claimant; Realm; collateral Profile/policy/token program;
  MarketBinding; MarketRuntime; MarketInstance artifact; writable Hoard V2 and
  ClaimLedger V3; Resolution V5; fractional policy; writable aggregate ledger;
  collateral mint; writable destination; Hoard authority; writable Hoard token;
  outcome token program; writable bearer source; then one mint per active
  outcome, with only the selected mint writable. Credited form remains staged
  and adds credit/tombstone, payer, System Program, and neutral sink.
- Transfer/merge: source and destination claimant signers; policy; ledger;
  source and destination credits; ClaimLedger V3; Hoard V2; exact Position/Replay or
  external collateral payout target; collateral release/program; funding and
  tombstone metas; capability manifest.
- Close credit: claimant; policy; ledger; ClaimLedger V3; live credit; stored
  rent payer; neutral sink; System Program; capability manifest; Resolution.
- Terminal seal: Realm; collateral Profile/policy/token program; MarketBinding;
  MarketRuntime; MarketInstance artifact; Hoard V2; writable ClaimLedger V3;
  Resolution V5; policy; writable aggregate ledger.
- Terminal close: policy; ledger; Resolution; ClaimLedger V3; Hoard V2;
  writable Product five-family aggregator authorization; capability manifest; policy
  rent payer; ledger rent payer; neutral sink. It deletes `0xa4` and `0xa5`
  atomically and advances ClaimLedger to Retiring.

Disabled tuples refuse before parsing payloads or inspecting accounts. Actions
2, 3, and 9 already perform their complete typed authentication, external-
effect ordering where applicable, and atomic writeback. Action 2 remains
independent of bearer claim-release availability. Enabling actions 1 and 2
together still depends on Product exposing its stable per-slot Foundation
preallocation authority and atomic family-admission consumer. Slots 11 and 12
are Product-prefunded, zero-data, System-owned writable PDAs before action 1;
Fractional will allocate, assign, and write those exact prestates without
debiting or refunding them again. Product remains the sole owner of the
Foundation debit/donation evidence and persisted typed claim-issuance binding;
the Fractional adapter will not invent a duplicate owner or provision mock
state. Action 10 analogously waits only for Product's private terminal
consumer; Fractional already owns the release authentication, terminal
postwrite verification, account deletion, and the two exact rent splits.
