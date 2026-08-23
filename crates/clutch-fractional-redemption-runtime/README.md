# Exact fractional-redemption runtime contract

This crate promotes `research/fractional-redemption` into a safe, `no_std`,
allocation-free, fixed-layout runtime contract. It does **not** enable a Solana
route. Intent family `79/v1`, actions `1..=10`, and account coordinates
`0xa4..=0xa7` are centrally reserved as `ReservedDisabled`.

The new persisted facts have one owner each:

| account | owner | exact body |
| --- | --- | ---: |
| `0xa4/v1` | immutable Market/Resolution/Realm/claim policy and resolved common lot | 296 |
| `0xa5/v1` | aggregate numerator `K`, live-credit count, and global replay sequence | 224 |
| `0xa6/v1` | one claimant's canonical numerator `<D`, generation, replay, and rent | 296 |
| `0xa7/v1` | permanent zero-credit close/reopen identity | 232 |

Resolution/Terms remain the sole vector owner. SupplyLedger remains the sole
internal-plus-bearer supply owner. Market backing remains the sole claim-
collateral owner. Position V3 and its purpose-owned Replay V3 remain the only
internal custody/replay bodies. The Realm collateral and independent
Token-2022 claim contracts remain the only CPI authorities.

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

The only terminal policy is `RetainUntilExactAggregation`. If all native claims
are gone but aggregate credit is `D*A+r`, voluntary aggregation can pay `A`
whole atoms. When `r != 0`, the remaining credits and claim backing stay live.
The close route requires claims, aggregate credit, live credit accounts, and
claim backing all to be zero. It therefore cannot sweep a final Hoard atom,
reinterpret donation surplus as revenue, invent a reserve, or silently forfeit
a claimant numerator.

## Disabled Solana account contract

The frozen future account order is:

- `Initialize`: payer; MarketInstance; Realm; collateral Profile/policy;
  Resolution; claim-issuance binding; policy PDA; ledger PDA; System Program;
  Rent sysvar; neutral sink; capability/release manifest.
- Internal redeem: owner; policy; ledger; Resolution; SupplyLedger; Market
  backing; Position V3; Replay V3; capability manifest. Credited form appends
  credit/tombstone, payer, System Program, and neutral sink.
- Bearer redeem: claimant; policy; ledger; Resolution; SupplyLedger; Market
  backing; outcome mint; bearer source; Realm Hoard; collateral destination;
  claim token program; collateral token program; capability manifest; exact
  claim/collateral release record. Credited form appends credit/tombstone,
  payer, System Program, and neutral sink.
- Transfer/merge: source and destination claimant signers; policy; ledger;
  source and destination credits; Market backing; exact Position/Replay or
  external collateral payout target; collateral release/program; funding and
  tombstone metas; capability manifest.
- Close credit: claimant; policy; ledger; live credit; stored rent payer;
  neutral sink; System Program; capability manifest; Resolution.
- Terminal seal/close: policy; ledger; Resolution; SupplyLedger; Market
  backing; capability manifest, followed by stored rent destinations on close.

`refuse_disabled_fractional_redemption_v1` returns `CapabilityDisabled` before
parsing the payload or inspecting these accounts. Activation must atomically
replace that refusal with owner/PDA/signature checks, canonical Resolution and
SupplyLedger decoding, exact Token-2022 burn and Realm-collateral CPI
postchecks, Position/Replay V3 writeback, rent admission, and a checked release
manifest tuple.
