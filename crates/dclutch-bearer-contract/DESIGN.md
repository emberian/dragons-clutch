# Optional bearer outcome-claim contract

`dclutch-bearer-contract` is safe Rust, `no_std`, `no_alloc`, fixed-layout,
SDK-free, and total. It defines one optional Market capability; it does not add
bearer Mints to the universal Market ontology. A Market can use this facility
only when its authenticated immutable capability manifest selects the exact
kind, semantic release, config, child schema, and child derivation identities
defined here. Activation consumes already segregated rent and creation
principal. It never uses Hoard collateral or future revenue.

This is a pure semantic contract, not an SBF adapter or a deployment claim.

## One owner for each fact

| Fact | Semantic owner |
|---|---|
| Realm token program and collateral Mint | immutable `RealmV1` |
| Market identity, generation, phase, direct-child count | `MarketRoot` inside `CategoricalMarketV1<N>` |
| Hoard collateral and aggregate outcome supply `S[i]` | `CategoricalMarketV1<N>` |
| One participant's native balance `P[i]` | that participant's `PositionV1<N>` |
| Materialized subset `B[i]` of aggregate supply | one `BearerCapabilityV1<N>` direct child |
| Physical materialized supply | canonical Token-2022 Mint `supply`, required to equal `B[i]` |
| Token ownership distribution | Token-2022 Accounts; it is deliberately not copied into program state |
| Optional-capability selection and funding quote | immutable capability manifest |
| Remaining/released activation principal by physical asset | typed `FundingStateV1` |
| Bearer rent-refund identity | immutable `BearerConfigV1` selected by the manifest |

`B[i]` is a representation subset, not another liability total. The Market's
`S[i]` remains the sole aggregate liability. Direct venue custody and every
other native holder remain outside this capability and are included in the
symbol `A[i]` below.

## Conservation theorem

For each ordered outcome `i`, let:

- `S[i]` be Market aggregate supply;
- `B[i]` be bearer state accounted supply;
- `T[i]` be the canonical Token-2022 Mint supply;
- `A[i]` be the sum of all canonical native Positions and native custody;
- `H` be claimant-backing Hoard collateral.

Assume the incoming composed state satisfies:

1. `S[i] = A[i] + B[i]` for every `i`;
2. `B[i] = T[i]` for every `i`;
3. the Market kernel's phase-specific solvency condition; and
4. the adapter assumptions in the trust-boundary section.

Every successful transition plan preserves (1), (2), and Market solvency after
its exact token/collateral effects execute atomically:

| Transition | `H` | `S` | native `A` | bearer `B = T` |
|---|---:|---:|---:|---:|
| Split to Position by `q` | `+q` | every cell `+q` | selected Position every cell `+q` | unchanged |
| Merge from Position by `q` | `-q` | every cell `-q` | selected Position every cell `-q` | unchanged |
| Materialize cell `i` by `q` | unchanged | unchanged | `i: -q` | `i: +q` |
| Dematerialize cell `i` by `q` | unchanged | unchanged | `i: +q` | `i: -q` |
| Transfer | unchanged | unchanged | unchanged | aggregate unchanged; Token Accounts redistribute |
| Split directly to bearer by `q` | `+q` | every cell `+q` | unchanged | every cell `+q` |
| Merge bearer complete set by `q` | `-q` | every cell `-q` | unchanged | every cell `-q` |
| Redeem native cell `i` by `q` | `-p` | `i: -q` | `i: -q` | unchanged |
| Redeem bearer cell `i` by `q` | `-p` | `i: -q` | unchanged | `i: -q` |
| Audit or retirement | unchanged | unchanged | unchanged | unchanged |

For categorical claims, `p = q` for the terminal winner and `p = 0` for a
loser. There is no rounding boundary: claim and collateral amounts are exact
`u64` raw atoms.

This is a preservation theorem, not a claim that a single instruction can
enumerate all Positions or venue custody. The incoming `S = A + B` condition is
established by the Market/Position/custody transitions which created that
state. This crate's native split, merge, and redemption composition functions
make the relevant Market/Position deltas indivisible; its bearer functions make
the Market/bearer/Mint deltas indivisible.

### No double redemption

Materialization first removes the exact atoms from the Position representation
and adds them to `B = T`, while dematerialization performs the inverse. Native
redemption atomically debits Position and Market supply. Bearer redemption
atomically permission-burns Token-2022 supply and debits bearer state and Market
supply. Consequently one atom can inhabit only one redeemable representation
at a time. Replaying a materialization sees stale Mint supply or insufficient
Position balance; replaying a bearer redemption sees stale supply or
insufficient Token balance; replaying native redemption sees insufficient
Position balance.

The adapter may persist none of these mutations unless every returned CPI and
post-state check succeeds in the same SVM instruction. Persisting state without
its token effect is outside the theorem and intentionally fail-stops all later
audits through `B != T`.

## Token-2022 mechanism

Bearer claims are genuinely transferable Token-2022 units. They are not
non-transferable wrappers and do not require a transfer hook. Ordinary holder
transfers may execute directly through Token-2022: they redistribute Token
Accounts but leave Mint supply, Market supply, and `B` unchanged.

The required control is Token-2022 `PermissionedBurn`, not a permanent
delegate. Each outcome Mint has exactly two TLV extensions:

1. `MintCloseAuthority`, authority = the bearer capability PDA;
2. `PermissionedBurn`, authority = the bearer capability PDA.

The base Mint has decimals zero, is initialized, has mint authority equal to
the capability PDA, and has no freeze authority. No other extension is
admitted. In particular there is no transfer hook, permanent delegate,
transfer-fee state, default account state, non-transferable marker, pausable
state, confidential state, metadata pointer, or unknown TLV.

Permissioned burn requires both the capability PDA and the source owner or
delegate on the extension burn instruction. Thus a holder can transfer but
cannot reduce supply outside a dClutch plan. The holder still authorizes their
own burn; the PDA cannot confiscate tokens unilaterally. Mint authority prevents
external minting. The capability state checks observed Mint supply against
`B[i]` before every bearer action and refuses either an increase or decrease;
there is no reconciliation route that silently edits Market liabilities.

Claim Token Accounts admitted by this contract are exact 165-byte initialized
Token-2022 base Accounts with no TLV extensions and no wrapped-native reserve.
Accounts with additional behavior can receive an external transfer, which is
not a supply violation, but must transfer units back to an admitted base Account
before a protocol burn. This choice keeps CPI-guard, confidential, withheld-fee,
and other account-local behavior outside the theorem.

### Exact Token-2022 widths and provenance

The canonical Mint is 238 bytes:

- 82-byte Mint base;
- 83 bytes of zero extension padding through the 165-byte Account base width;
- one account-type byte;
- `4 + 32` bytes for `MintCloseAuthority`; and
- `4 + 32` bytes for `PermissionedBurn`.

This was derived from the locally cached official
`spl-token-2022-interface = 3.1.1` crate (Apache-2.0), crates.io archive SHA-256
`821d96d034ea31c4965d182c742153c491ae0abee531331b55771086c5030d86`.
Relevant exact source digests are:

- `src/extension/mod.rs`:
  `502b8309d3243f81d3bb7b2ff5f9e412c48d4d68f354b8994792389fc904defd`;
- `src/extension/mint_close_authority.rs`:
  `d68a5cc324c217e5e18a86dab363f9f67c19e14e828f942f534ff6e9db441a3b`;
- `src/extension/permissioned_burn/mod.rs`:
  `7ecbf5d694e90d48c4e3c1f1eeb0bf31081cf9a12fb43e5f8186177ca5b58f62`;
- `src/extension/permissioned_burn/instruction.rs`:
  `ee85792e5acef3df6a9622bbb371db8f1c83484634c9fa87ec4254d4a54429d9`.

These are ABI/interface provenance, not evidence about any deployed
Token-2022 program binary or upgrade authority.

## Capability activation and funding

The manifest is the only authority for whether bearer claims exist. A selected
entry must match these SHA-256 identities:

| Manifest field | Identity preimage |
|---|---|
| kind | `dclutch:capability-kind:bearer-outcome-claims:v1` |
| semantic release | `dclutch:bearer-contract:semantic-release:v1` |
| child schema | `dclutch:bearer-contract:child-schema:v1` |
| child derivation | `dclutch:bearer-contract:child-derivation:v1` |

The entry's config identity must equal the composing hash of the exact 80-byte
`BearerConfigV1`. Its typed funding quote must be native-only: Rent and Creation
are `NativeLamports` (or canonical zero/NotApplicable), Work, Provider, Bounty,
Liquidity, and Service are zero/NotApplicable, and no Realm-collateral binding
or token vault may exist. Bearer activation therefore has no route which can
reinterpret collateral atoms as lamports or silently accept an unrelated
funding asset.

Activation is admitted only while the Market is Founding or Open. The adapter
constructs `FundingCustodyObservationV1::native_only` from the actual
program-owned funding-state lamports and the authenticated current Rent minimum.
`FundingStateV1::activate` is evaluated on a candidate state. Its typed
`rent_lamports` and `creation_lamports` debit must equal the adapter's exact
physical requirement for the capability state plus all `N` 238-byte Mints.
The typed funding mutation, payer reimbursement, account/Mint creation, Market
`register_child`, and bearer-state write must commit atomically. There is no
legacy scalar-present-principal path or compatibility decoder.

`BearerConfigV1.rent_refund` receives recovered state/Mint rent through the
repository's permanent RentCredit mechanism. Rent principal is sponsor
principal. It is not a fee, bounty, reserve, treasury source, Hoard atom, or
liveness capitalization.

## Canonical persisted layouts

### Bearer config: 80 bytes

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | `DCLTBRC1` magic |
| 8 | 2 | schema `1` |
| 10 | 6 | zero reserved |
| 16 | 32 | official Token-2022 program |
| 48 | 32 | immutable rent-refund identity |

### Bearer capability: `64 + 8N` bytes

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | `DCLTBRS1` magic |
| 8 | 2 | schema `1` |
| 10 | 1 | exact outcome count |
| 11 | 1 | provisional profile `1` |
| 12 | 4 | zero reserved |
| 16 | 32 | Market key |
| 48 | 8 | generation, little endian |
| 56 | 2 | manifest entry index |
| 58 | 6 | zero reserved |
| 64 | `8N` | `B[0..N]`, exact `u64` little endian |

Widths are 80 bytes for `N=2` and 192 bytes for `N=16`. The active Market
remains `320 + 8N`, Position remains `88 + 8N`, terminal Market remains 312,
each claim Mint is 238, and an admitted holder claim Account is 165.

## PDA preimages

Each seed component is at most the chain-derived 32-byte limit. The adapter
must use these exact ordered components and must not hash or rewrite a domain:

- capability root: `[b"dclutch/bearer-cap/v1", market, generation_le]`;
- outcome Mint:
  `[b"dclutch/bearer-mint/v1", market, generation_le, outcome_u8]`.

The executing dClutch program ID is the implicit PDA namespace. The capability
root PDA is the mint, permissioned-burn, and close authority. `N <= 16`, so one
outcome byte is canonical.

## Lifecycle and terminal child accounting

Activation registers exactly one direct Market child. The `N` Token-owned
Mints are descendants named and controlled by that child; they do not each
increment Market child count. Activation creates all Mints at once, so no
unfunded lazy per-outcome object can appear later.

During Retiring, no materialization or split is admitted. Dematerialization,
transfer, and redemption remain available. Once every `B[i]` and observed Mint
supply is zero, retirement atomically closes all Mints, routes every lamport to
the config's refund identity, closes the capability root, and calls
`Market::retire_child(generation, expected_prior_child_count)`. There is no
partially closed status and no separate child counter. A replay meets either a
closed account or the Market's child-count mismatch. The Market therefore
cannot become Retired and compact while bearer supply or a live bearer root
remains.

## Instruction wire

Every instruction begins with the exact 16-byte header
`magic[8] || schema:u16le || action:u8 || outcome_count:u8 || zero[4]`.
Trailing bytes, nonzero reserved bytes, unknown actions, unsupported widths,
and out-of-range outcomes refuse.

| Action | Data bytes | Payload after header |
|---|---:|---|
| Activate | 32 | generation and prior child count; entry, Clock, Rent, custody, and physical requirements come from authenticated accounts/runtime |
| Audit | 24 | generation |
| Split/Merge native | 32 | generation, quantity |
| Materialize/Dematerialize/Transfer | 40 | generation, quantity, outcome, zero[7] |
| Split/Merge bearer | 32 | generation, quantity |
| Redeem native/bearer | 40 | generation, quantity, outcome, zero[7] |
| Retire | 32 | generation, prior child count |

## Exact account frames

`frame::validate_account_frame` rejects an incorrect count, reordered role,
zero key, extra signer/writable privilege, missing privilege, unsafe alias, or
wrong executable flag. The only allowed duplicate key is the claim
Token-2022/collateral-token program pair when the Realm itself uses Token-2022.

| Action | Accounts | Shape summary |
|---|---:|---|
| Activate | `10 + N` | writable Market/state/typed funding/payer, manifest/config/refund, Token-2022/System/Rent, `N` writable Mints |
| Audit | `3 + N` | Market, state, Token-2022, `N` read-only Mints |
| Split/Merge native | 9 | Market, Position, Realm, custody, vault, collateral account, holder, token program, collateral Mint |
| Materialize/Dematerialize | 7 | Market, writable state/Position/Mint/claim Account, holder, Token-2022 |
| Transfer | 7 | Market, state, Mint, two claim Accounts, holder, Token-2022 |
| Split/Merge bearer | `10 + 2N` | Market/state plus collateral base and ordered `(Mint, claim Account)` pairs |
| Redeem native | 9 | native value frame |
| Redeem bearer | 12 | Market/state plus collateral payout and bearer burn accounts |
| Retire | `8 + N` | writable Market/state/refund and Mints; manifest/config, Token-2022/System/Rent |

Program owner, account-data codec, PDA key, Realm content hash, manifest content
hash, collateral vault/custody relationship, and native sysvar identity checks
remain additional adapter checks; role/privilege validation does not pretend to
replace them.

## Provisional `N = 2..=16` bound

The bound is the repository's current measured categorical profile, not a
mathematical limitation. The contract executes and tests every width from two
through sixteen. Lifting requires a new reviewed bearer state/profile and wire
discriminator, a correspondingly wider Market/kernel profile, and fresh SVM
packet/account-lock measurements. Existing profile-1 bytes and identities do
not change.

At `N=16`, the largest persisted bearer root is 192 bytes. The largest account
frames are complete-set split/merge at 42 accounts (`10 + 2N`) and activation
at 26 accounts (`10 + N`), before invoked programs are counted by a transaction
message. These are semantic frame counts, not serialized packet measurements;
the future adapter must measure actual v0 transactions before claiming an SBF
route fits.

## Exact remaining adapter boundary

The future SBF adapter must, in one atomic instruction:

1. authenticate Market, Realm, Position, custody, manifest, config, funding,
   RentCredit, sysvars, and every Token-2022 account owner and key;
2. recompute content identities and exact PDA addresses from the documented
   preimages;
3. pin a reviewed Token-2022 interface/program release which actually supports
   `PermissionedBurn`, and disclose its deployment/upgrade-authority boundary;
4. fully parse Mint base, account-type, and every TLV; reject wrong length,
   duplicate, unknown, omitted, or extra extensions and wrong authorities;
5. parse holder Accounts at exact 165-byte base width and validate authority,
   balance, initialization, Mint, and absence of native reserve;
6. obtain `Clock` and `Rent` from runtime-authoritative access, construct the
   typed native-only `FundingCustodyObservationV1`, compute exact bearer
   physical rent/creation requirements, and compose the funding semantic owner;
7. use checked canonical CPI builders for initialize, mint, permissioned burn,
   transfer, close, and Realm-selected collateral movement;
8. use holder signatures for debits and the capability PDA only for its narrow
   mint/burn/close roles;
9. re-read and verify every Mint supply, holder balance, collateral balance,
   lamport balance, closed account, and program-owned state against the returned
   exact plan; and
10. commit all state/token/collateral/rent effects together or return an error
    so the runtime rolls all of them back.

Until that adapter exists and its Token-2022 deployment is pinned, this crate
proves only the pure transition theorem under the named boundary. It is not
mainnet, devnet, or local-validator execution evidence and is not a formal
verification claim.

## Adversarial evidence

The focused unit suite covers:

- exact config/state codecs, reserved bytes, output atomicity, and widths;
- every provisional width `N=2..=16`;
- unchanged Mint supply after external transfers and refusal of observed
  external burn/mint drift;
- double materialization, dematerialization, and bearer/native double
  redemption attempts;
- direct bearer complete-set split/merge;
- stale generation and Market child-count replay;
- wrong Mint key/program/authority/extension count/account state;
- `u64` supply arithmetic overflow with no partial mutation;
- hostile instruction lengths, trailing/reserved bytes, and outcome bounds;
- exact frame privileges and alias refusal; and
- all-Mint retirement followed by replay refusal.
