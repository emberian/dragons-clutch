# Exact fractional-redemption runtime contract

This crate promotes `research/fractional-redemption` into a safe, `no_std`,
allocation-free, fixed-layout runtime contract. It does **not** enable a Solana
route. Intent family `79/v1`, actions `1..=10`, and account coordinates
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

The only terminal policy is `RetainUntilExactAggregation`. If all native claims
are gone but aggregate credit is `D*A+r`, voluntary aggregation can pay `A`
whole atoms. When `r != 0`, the remaining credits and claim backing stay live.
The close route requires claims, aggregate credit, live credit accounts, and
claim backing all to be zero. It closes the policy and aggregate ledger only
under the matching private ProductOccurrenceRoot terminal authorization,
refunds their stored rent payers independently, and sends only excess lamports
to the neutral sink. It therefore cannot sweep a final Hoard atom, reinterpret
donation surplus as revenue, strand policy rent, permit reinitialization,
invent a reserve, or silently forfeit a claimant numerator.

## Disabled Solana account contract

The frozen future account order is:

- `Initialize`: payer; MarketInstance; Realm; collateral Profile/policy;
  Resolution; claim-issuance binding; policy PDA; ledger PDA; ClaimLedger V3;
  System Program; Rent sysvar; neutral sink; capability/release manifest.
- Internal redeem: owner; policy; ledger; Resolution; ClaimLedger V3; Hoard V2;
  Position V3; Replay V3; capability manifest. Credited form appends
  credit/tombstone, payer, System Program, and neutral sink.
- Bearer redeem: claimant; policy; ledger; Resolution; ClaimLedger V3; Hoard V2;
  outcome mint; bearer source; Realm Hoard; collateral destination;
  claim token program; collateral token program; capability manifest; exact
  claim/collateral release record. Credited form appends credit/tombstone,
  payer, System Program, and neutral sink.
- Transfer/merge: source and destination claimant signers; policy; ledger;
  source and destination credits; ClaimLedger V3; Hoard V2; exact Position/Replay or
  external collateral payout target; collateral release/program; funding and
  tombstone metas; capability manifest.
- Close credit: claimant; policy; ledger; ClaimLedger V3; live credit; stored
  rent payer; neutral sink; System Program; capability manifest; Resolution.
- Terminal seal: policy; ledger; Resolution; ClaimLedger V3; Hoard V2;
  capability manifest.
- Terminal close: policy; ledger; Resolution; ClaimLedger V3; Hoard V2;
  writable ProductOccurrenceRoot authorization; capability manifest; policy
  rent payer; ledger rent payer; neutral sink. It deletes `0xa4` and `0xa5`
  atomically and advances ClaimLedger to Retiring.

`refuse_disabled_fractional_redemption_v1` returns `CapabilityDisabled` before
parsing the payload or inspecting these accounts. Activation must atomically
replace that refusal with owner/PDA/signature checks, canonical Resolution and
ClaimLedger V3/Hoard V2 decoding, exact Token-2022 burn and Realm-collateral CPI
postchecks, Position/Replay V3 writeback, rent admission, and a checked release
manifest tuple.

The SBF `fractional-redemption-adapter-seam` feature additionally compiles the
private Product 0xaa terminal receipt composition. It still cannot mint its
release receipt because action 10 is absent from every capability profile; it
adds no dispatch route and executes no close. The frozen receipt binds the
Product occurrence's exact registry release/profile, Series, Market,
generation, Resolution V5 account/data/body identities, NativeClaimBasis,
physical a4/a5 accounts, both terminal state IDs, ClaimLedger retirement latch,
and independent payer/neutral rent dispositions.
