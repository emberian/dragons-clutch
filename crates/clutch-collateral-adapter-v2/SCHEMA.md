# Collateral adapter schema

All integers are little-endian. Every body requires exact length, exact magic,
exact version, and zero reserved bytes. Content identities are SHA-256 over the
listed domain followed by the complete canonical body.

## `AdapterReleaseV2` — 192 bytes

Domain: `dragons-clutch/collateral-adapter-release/v2\0`

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | `DCCAR2\0\0` |
| 8 | 2 | version `2` |
| 10 | 2 | family: `1=LegacySpl`, `2=Token2022Base` |
| 12 | 1 | owner guard: `1=ImmutableOwner`, `2=PdaSoleSigner` |
| 13 | 1 | checked-transfer discriminator `12` |
| 14 | 1 | close-account discriminator `9` |
| 15 | 2 | exact operation mask `0x001f` |
| 17 | 2 | exact release flags `0x000f` |
| 19 | 2 | intrinsic dangerous behaviors, exactly zero |
| 21 | 32 | token program |
| 53 | 32 | external deployment/release-manifest digest |
| 85 | 32 | parser/CPI code digest |
| 117 | 8 | known mint-extension mask |
| 125 | 8 | safe mint-extension mask |
| 133 | 8 | known account-extension mask |
| 141 | 8 | safe account-extension mask |
| 149 | 8 | required custody-extension mask |
| 157 | 2 | exact mint account bytes |
| 159 | 2 | exact base holder account bytes |
| 161 | 2 | exact custody account bytes |
| 163 | 29 | zero reserved tail |

Legacy is fixed to the legacy SPL Token program, `82/165/165`, zero extension
masks, and `PdaSoleSigner`. Token-2022 is fixed to the Token-2022 program,
`82/165/170`, the 29-discriminant known masks, only `ImmutableOwner` safe on
accounts, and `ImmutableOwner` required for custody. Unknown family values do
not decode. A future family requires a successor implementation and schema.

## `CollateralPolicyV2` — 224 bytes

Domain: `dragons-clutch/collateral-policy/v2\0`

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | `DCCPOL2\0` |
| 8 | 2 | version `2` |
| 10 | 2 | exact strict flags `0x003f` |
| 12 | 32 | adapter release ID |
| 44 | 32 | token program |
| 76 | 32 | token-program deployment digest |
| 108 | 32 | collateral mint |
| 140 | 1 | decimals |
| 141 | 7 | zero reserved |
| 148 | 8 | maximum admitted current mint supply, atoms |
| 156 | 8 | maximum per-Market cap, atoms |
| 164 | 8 | allowed mint extensions |
| 172 | 8 | required mint extensions |
| 180 | 8 | allowed account extensions |
| 188 | 8 | required account extensions |
| 196 | 28 | zero reserved tail |

Both ceilings are nonzero and the Market ceiling cannot exceed the mint-supply
ceiling. The actual Market cap remains Terms-owned and must be nonzero and no
larger than the policy ceiling. Extension masks may narrow the resolved release
but never widen it.

## `ClaimIssuanceBindingV1` — 160 bytes

Domain: `dragons-clutch/claim-issuance-binding/v1\0`

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | `DCCLAIM1` |
| 8 | 2 | version `1` |
| 10 | 2 | exact claim flags `0x000f` |
| 12 | 32 | claim adapter release |
| 44 | 32 | Token-2022 program |
| 76 | 32 | external deployment/release-manifest digest |
| 108 | 32 | claim parser/CPI code digest |
| 140 | 1 | decimals, exactly zero |
| 141 | 8 | mint extensions, exactly zero |
| 149 | 8 | account extensions imposed by issuance, exactly zero |
| 157 | 3 | zero reserved tail |

This body is not a collateral policy child. Its adapter release must differ
from the collateral adapter release even when both planes use the same external
Token-2022 deployment.

## CPI intents

Checked transfer data is exactly:

```text
12:u8 | raw_atoms:u64_le | authenticated_decimals:u8
```

Account metas are ordered `source writable`, `mint readonly`, `destination
writable`, `source owner authority signer`. Program-derived authority changes
only the outer adapter's choice of `invoke_signed`; it does not change the
external instruction or account order.

Empty custody close data is exactly the release-frozen discriminator `9`.
Account metas are ordered `empty custody vault writable`, `exact lamport vault
writable`, `custody owner authority signer`. Both admitted families use this
shape, but the Realm-selected release chooses the external program and binds
the close operation; Series code does not hard-code the discriminator.

Legacy custody initialization emits only `InitializeAccount3` (`18 ||
owner[32]`). Token-2022 custody emits `InitializeImmutableOwner` (`22`) before
the identical `InitializeAccount3` body. Claim mint initialization is not part
of this crate's collateral creation plan.

## Series terminal join

`SeriesCollateralFundingJoinV2` binds Realm/Profile, SeriesPlan, FundingTerms,
the exact funding-state account, quote, funding authority, and exact token and
lamport refund/sink accounts. The five custody bindings use compartments
`1..=5`. `SeriesCollateralTerminalJoinV2` adds a one-shot terminal receipt; its
activation generation is the canonical constant `1`, never caller input.

A close admits a zero-atom segregated vault under that graph, moves all of only
that token account's lamports to its exact zero-data System-owned component
vault, and checks the exact delta plus the closed account's zero-lamport,
zero-data, System-owned terminal state. The subsequent split is:

```text
refund_lamports  = stored_token_vault_rent_principal
neutral_lamports = closed_token_vault_lamports - refund_lamports
```

It refuses when stored principal is not covered. Postcheck requires the
component lamport vault to return to its exact pre-close balance, so existing
component prefunding is neither swept nor reclassified. These quantities are
lamports, never collateral atoms, Hoard principal, fees, work, or liveness
capital.
