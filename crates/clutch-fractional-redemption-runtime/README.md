# Exact fractional-redemption runtime contract

This crate promotes `research/fractional-redemption` into a safe, `no_std`,
allocation-free, fixed-layout runtime contract. The SBF adapter contains all
ten handlers, including Product-owned atomic family admission and terminal
consumption around the Fractional-owned account writes, deletion, and rent
splits. Their capability remains disabled until the whole family passes one
release review. Intent family `79/v1`, actions `1..=10`, and account coordinates
`0xa4/v3`, `0xa5/v1`, `0xa6/v2`, and `0xa7/v2` are centrally reserved as
`ReservedDisabled`.

The new persisted facts have one owner each:

| account | owner | exact body |
| --- | --- | ---: |
| `0xa4/v3` | immutable Market/Resolution-V5-data/Realm/claim policy and resolved common lot; Foundation-computable Market/Resolution-account PDA | 296 |
| `0xa5/v1` | ClaimLedger account binding, aggregate numerator `K`, live-credit count, and global replay sequence | 224 |
| `0xa6/v2` | one claimant's canonical numerator `<D`, generation, replay, and rent | 296 |
| `0xa7/v2` | permanent zero-credit close/reopen identity | 232 |

The never-activated `0xa4/v1`, `0xa6/v1`, and `0xa7/v1` coordinates are
explicitly withdrawn. Their identity slots were allocated as payout-vector
digests, so no current decoder accepts them and no migration or fallback
aliases them. Policy V2 is separately withdrawn before activation because its
address depended on future final Resolution bytes and could not be prefunded.
Policy V3 keeps that exact data identity in the immutable body while its PDA is
fixed by Market and Resolution account. Credit V2 remains unchanged;
`0xa5/v1` continues to own only aggregate credit, live-credit count, and its
cross-account sequence.

Resolution V5 remains the sole vector owner. The policy and every owner credit
persist its exact physical-account-bound Resolution data ID, while each transition also
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
exact a4/v3 and a5/v1 accounts, advances sequence zero to one, and emits the
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
Fractional runtime/capability release ID. Action 10 also authenticates the
current independent Token-2022 claim Program/ProgramData release before reading
mint supplies and binds that release receipt into Product's private terminal
authority. Product may not invent either release, substitute the Realm
collateral release, or turn the pure close plan into authority. Fractional now exposes only crate-private SBF
postwrite capabilities: the admission capability authenticates the exact
writable a4/a5/ClaimLedger founding bodies and PDAs, while the terminal
capability authenticates the exact Retiring ClaimLedger body, both live
pre-deletion account bodies, both observed rent balances, and an action-10
release narrowed from the loader-authenticated registry capability. Product's
consumer can accept those private values but cannot construct them or replace
their receipt. Product's atomic aggregator/root consumers and private
Foundation preallocation authority are present. Actions 1 and 10, like actions
2 through 9, remain disabled until the whole family passes release review and
is admitted by one exact linked capability profile.

## Solana activation boundary

The executable-but-capability-disabled account order is:

- `Initialize`: the 14 Product Foundation core accounts in slot order, the
  active OutcomeMint prefix, the active OutcomeCustody prefix, then Realm,
  Profile, collateral policy/program/ProgramData, claim program/ProgramData,
  MarketInstance artifact, founder Series link, FundingQuoteV4 artifact,
  SeriesRegistryV2, this Program/ProgramData, ReleaseV2/ProfileV4 artifacts,
  System Program, and Rent. No signer or second Foundation debit exists.
- Exact internal redeem: owner; Realm; collateral Profile/policy/program;
  MarketBinding; MarketRuntime; MarketInstance artifact; Hoard V2; ClaimLedger
  V3; Resolution V5; fractional policy; aggregate ledger; Position V3; GEN1
  Replay. Credited form appends credit/tombstone, authenticated Product Market
  root, its neutral sink, and Rent sysvar; fresh/reopen mode then appends an
  arbitrary writable signer payer and the System Program.
- Exact bearer redeem: claimant; Realm; collateral Profile/policy/token program;
  MarketBinding; MarketRuntime; MarketInstance artifact; writable Hoard V2 and
  ClaimLedger V3; Resolution V5; fractional policy; writable aggregate ledger;
  collateral mint; writable destination; Hoard authority; writable Hoard token;
  outcome token program; its exact immutable Upgradeable Loader ProgramData;
  writable bearer source; current Realm-collateral ProgramData; then one mint per active outcome, with only the
  selected mint writable. Credited form appends, after those mints,
  credit/tombstone, authenticated Product Market root, its neutral sink, and
  Rent sysvar; fresh/reopen mode then appends payer and System.
- Transfer/merge: source and destination claimant signers; Realm collateral
  profile/policy/program; MarketBinding/Runtime/Instance; writable Hoard V2 and
  ClaimLedger V3; Resolution; policy; ledger; both credits; then either exact
  Position/GEN1 or collateral mint/destination/Hoard authority/Hoard token;
  the current collateral ProgramData; Product root; neutral sink; Rent.
  Fresh/reopen destinations append payer and
  System. External payout authority stays private until both credits advance.
  Without a Position/GEN1 consumer, external `MergeCredit` is intentionally
  the full-source instance of `TransferCredit`; it does not invent a replay
  account merely to persist two labels for the same exact successor.
- Close credit: claimant; Realm collateral profile/policy/program;
  MarketBinding/Runtime/Instance; Hoard V2; ClaimLedger V3; Resolution; policy;
  ledger; live credit; stored rent payer; Product root; writable neutral sink;
  Rent. Only the refundable principal returns to the payer; only donation and
  excess go to the neutral sink; permanent tombstone principal stays put.
- Terminal seal: Realm; collateral Profile/policy/token program; MarketBinding;
  MarketRuntime; MarketInstance artifact; Hoard V2; writable ClaimLedger V3;
  Resolution V5; policy; writable aggregate ledger.
- Terminal close: the same Foundation core/mint/custody graph, then Realm,
  Profile, collateral policy/program/ProgramData, the independent Token-2022
  claim program/ProgramData, MarketInstance artifact, founder Series link,
  FundingQuoteV4, SeriesRegistryV2, this
  Program/ProgramData, ReleaseV2/ProfileV4, writable shared rent refund owner,
  and writable neutral sink. It consumes Product terminality before deleting
  `0xa4` and `0xa5`, refunds only both stored principals, sends every surplus
  lamport to the exact System-owned neutral sink, and advances ClaimLedger to
  Retiring. Any later refusal rolls the whole instruction back.

Disabled tuples refuse before parsing payloads or inspecting accounts. All ten
handlers perform their typed authentication, external-effect ordering where
applicable, and atomic writeback. Action 2 remains independent of bearer
claim-release availability, and action 9 remains independent because it reads
no mint and performs no claim CPI. Action 10 authenticates the current claim
release before its complete supply observation or any terminal write. Slots 11 and 12
are Product-prefunded, zero-data, System-owned writable PDAs before action 1;
Fractional will allocate, assign, and write those exact prestates without
debiting or refunding them again. Product remains the sole owner of the
Foundation debit/donation evidence and persisted typed claim-issuance binding;
the Fractional adapter will not invent a duplicate owner or provision mock
state. Action 10 consumes Product's private terminal writer before deletion;
Fractional owns release authentication, terminal postwrite verification,
account deletion, and both exact rent splits. Whole-family release review is
the remaining activation gate.
