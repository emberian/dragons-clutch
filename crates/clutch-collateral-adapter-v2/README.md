# Collateral adapter V2 contract

Status: **implemented pure contract and hostile-byte runtime seam; not routed in
the SBF dispatcher and not a production release.** This crate contains no
default catalog row, no DREGG branch, no program deployment, no global account
tag, and no authority to move value.

V1 remains unchanged and Token-2022-only. This successor makes the collateral
side generic without generalizing Egg issuance:

```text
Market -> Realm -> Profile -> CollateralPolicyV2 -> AdapterReleaseV2
                                                   | legacy SPL
                                                   | Token-2022 base

release manifest -> ClaimIssuanceBindingV1 -> Token-2022 Eggs
```

The two planes have separate release, deployment, and parser/CPI code
identities. `ClaimIssuanceBindingV1::require_separate_from_collateral` refuses
an accidentally reused adapter release. Sharing the Token-2022 *program* is
permitted; collapsing collateral transfer semantics into claim mint/burn
semantics is not.

## What is frozen

- `AdapterReleaseV2` is a 192-byte canonical record. Its content ID binds the
  exact parser/CPI code digest, external token-program deployment digest,
  family, parser/safe extension ceilings, custody owner guard, fixed account
  lengths, exact-transfer law, and supported operation set.
- `CollateralPolicyV2` is a 224-byte canonical record. Its content ID binds the
  selected release, program/deployment, mint, raw decimals, current mint-supply
  ceiling, per-Market cap ceiling, and Realm-narrowed extension sets.
- `ClaimIssuanceBindingV1` is a separate 160-byte canonical record for the
  fixed Token-2022 outcome mint/burn plane.
- `AdapterCatalogV2` accepts only a nonempty compile-time `'static` slice of at
  most sixteen exact releases. No fallback or arbitrary-program route exists.

The crate deliberately ships no catalog rows. Placeholder code or deployment
digests would turn a closed catalog into another mock source of authority. A
deployable profile must add exact reviewed artifact identities in the SBF
crate's compiled release module.

## Runtime/account seam

`bind_collateral_profile_v2` recomputes and joins the complete
Market/Realm/Profile/policy/release chain, the immutable Market cap, and the
runtime-observed program/deployment/parser identities. Market state carries no
second copy of mint, decimals, token program, or release facts.

`admit_collateral_mint_v2` and `admit_collateral_account_v2` parse borrowed
hostile bytes without allocation:

- legacy SPL requires exact 82-byte mint and 165-byte account layouts;
- Token-2022 collateral mints require the exact extension-free 82-byte layout;
- Token-2022 holders may use a 165-byte base account or the exact 170-byte
  `ImmutableOwner` shape when the Realm policy allows it;
- every Token-2022 custody account requires the exact 170-byte
  `ImmutableOwner` shape;
- mint/program/decimals, initialization, supply, authorities, frozen/native
  state, delegate/close authority, owner, and extensions are checked from
  current bytes on every transition.

Wrapped-native account state is refused. Raw token atoms are the only unit;
decimals authenticate `TransferChecked` and never rescale a quantity.

Legacy custody uses the explicitly weaker `PdaSoleSigner` contract: the owner
must be an adapter-authenticated canonical PDA, and the selected release must
bind a parser/CPI surface with no owner-authority-change operation. Token-2022
custody requires both a canonical PDA owner and token-enforced
`ImmutableOwner`.

## Exact transfers and cap accounting

`prepare_collateral_transfer_v2` accepts typed holder, Market Hoard, and
segregated-vault endpoints. It emits one fixed checked-transfer CPI intent and
no arbitrary passthrough. `accept_collateral_transfer_v2` reparses the mint and
both token accounts after CPI and requires:

```text
source_before - source_after           = requested_atoms
destination_after - destination_before = requested_atoms
mint_supply_after                      = mint_supply_before
```

Fees, withheld/opaque balances, transfer hooks, unknown extensions, and foreign
invocations are excluded by the exact compiled release and current account
admission before CPI; the observable postcondition is still checked after CPI.
State writers must discard every semantic write when post-admission fails.

The typed movements are Market Endow/Withdraw, segregated funding, occurrence
disbursement, payer-principal refund, and donation disposition. Each endpoint
binds an exact semantic owner and compartment. Product/Series, dealer, recovery,
and wrapper adapters must supply their own already-authenticated PDA/account
graph; this crate does not guess seeds or account-family tags.

`PositionCashV2` preserves `reserved <= cash` and permits withdrawal only from
the unreserved suffix. `CollateralBackingV2` separately owns locked liability
principal: lock/unlock is an internal accounting reclassification, never a
token CPI, and every state checks the exact immutable Market cap and visible
Hoard coverage. Hoard principal is never relabeled as a fee, reward, liveness
budget, rent source, or treasury.

Native-lamport `PresentFundingV1` and terminal liveness movements remain a
separate payer/endowment plane. This crate has no operation that can satisfy
work, keeper, rent, or terminal lamport obligations from Hoard or segregated
collateral principal.

`prepare_custody_creation_v2` freezes the selected family's account size and
initialization order:

- legacy SPL: 165 bytes, then `InitializeAccount3`;
- Token-2022: 170 bytes, `InitializeImmutableOwner`, then
  `InitializeAccount3`.

System allocation, rent, payer authorization, canonical PDA derivation,
`invoke`/`invoke_signed`, loader/ProgramData authentication, rollback, and
global dispatch remain the small live-adapter trust boundary.

## Immediate integration seam

The SBF successor should consume the crate in this order:

1. allocate an explicit V2 instruction/account namespace without changing V1;
2. compile real legacy and Token-2022 release rows only after their parser/CPI
   and external deployment digests are known;
3. bind Market/Realm/Profile/policy once per instruction;
4. use `prepare_hoard_creation_v2` or `prepare_custody_creation_v2` for account
   creation and never share that code with outcome mint creation;
5. use prepared exact transfer intents for Endow, Withdraw, Series funding,
   occurrence disbursement, and terminal refund/disposition; and
6. commit cash/backing/funding state only from the accepted post-CPI result.

No validation evidence is claimed by this implementation commit. Adversarial,
SBF, and local-validator campaigns belong after the active implementation gap
swarm is complete.
