# Collateral adapter profiles V2

Status: **PROPOSED successor design plus host-model and local-bank evidence.**
This document changes no consensus route, admits no real Realm, authenticates no
production token-program deployment, and does not make the current DREGG profile
executable.

## 1. Outcome

Collateral generality should be implemented as a closed, release-identified
adapter catalog selected by an immutable Realm. It must not be implemented as
"invoke whichever token program id the Realm supplied."

The crucial split is:

```text
collateral plane                         claim plane
Realm-selected adapter release           Clutch-selected claim release
legacy SPL / Token-2022 / future          Token-2022 Eggs today
deposit + withdrawal only                 mint + burn + bearer transfer
exact visible-atom contract               existing claim-supply contract
```

Generalizing the left side must not change the right side. In particular, a
legacy SPL collateral Realm still issues Token-2022 Eggs. Eggcrate sees only
checked atom deltas and never learns which external program moved them.

## 2. Audit of what exists

The immutable policy model is already mostly correct:

- `CollateralPolicy` commits program, mint, decimals, supply ceiling, strict
  authority flags, and extension masks in canonical bytes;
- `ProfileAccount` commits the recomputed policy digest, and Realm/Market carry
  the parent Profile identity;
- legacy SPL and Token-2022 program ids are distinct canonical currency values;
- unknown extension bits and unknown token-program ids fail closed; and
- DREGG is only an offline instance of the generic policy type.

The executable adapter is not collateral-generic:

1. `token::require_drivable_collateral` accepts only Token-2022.
2. Every collateral CPI constructor writes `TOKEN_2022_PROGRAM_ID` rather than
   the policy-selected program.
3. market creation requires the policy to allow `ImmutableOwner`, creates an
   extended Token-2022 Hoard, and invokes `InitializeImmutableOwner`.
4. instruction-family account checks independently require the Token-2022
   executable and Token-2022 account owners.
5. outcome issuance and collateral movement share one `token` module, obscuring
   that only the latter is intended to vary by Realm.
6. the current policy identity does not bind a parser/CPI adapter release. A
   program id authenticates an address, not the behavior or code Clutch audited.

This is why the legacy DREGG policy can decode but cannot found a market. The
codec is broader than the adapter, and the runtime's additional
`ImmutableOwner` requirement makes the mismatch intentional rather than a
missed conditional.

## 3. Common conformance interface

A successor policy binds an `AdapterReleaseId` in addition to the existing
currency identity. A compiled release record has these semantic fields:

| Field | Required meaning |
| --- | --- |
| release id | content/release identity of the exact parser and CPI implementation |
| family | legacy SPL, Token-2022 at a pinned extension universe, or an explicitly implemented successor |
| program/deployment identity | exact external program and checked release boundary |
| parser ceiling | every understood mint/account state and extension discriminant |
| safe ceiling | subset with a reviewed exact-atom theorem; Realms may narrow, never widen |
| Hoard owner guard | token-enforced immutable owner or the separately named PDA-sole-signer theorem |
| transfer law | exact visible source debit and destination credit, unchanged supply, no withheld state |
| foreign invocation law | no transfer hook or other unenumerated program execution |
| supported instructions | create/init, checked transfer in, checked transfer out; never arbitrary passthrough |

The adapter interface has five operations, whether they are implemented as
traits, modules, or generated static dispatch:

1. `admit_policy(policy, release)` resolves an exact closed-catalog release and
   refuses arbitrary program ids.
2. `observe_and_admit_mint` parses hostile bytes and verifies runtime owner,
   address, initialization, decimals, positive bounded supply, absent mint and
   freeze authorities, extension ceiling, and semantic behavior.
3. `observe_and_admit_account(role)` verifies program, mint, owner authority,
   initialized/unfrozen state, visible amount, extensions, and—on the Hoard—no
   delegate or close authority plus the release's owner guard.
4. `create_hoard` uses the family-specific fixed layout and initialization
   sequence. It never creates claim mints.
5. `transfer_exact` constructs only the family's checked-transfer operation,
   then reloads and authenticates both accounts and the mint before accepting
   the exact postcondition.

The postcondition for requested raw atom quantity `q` is:

```text
source_before - source_after            = q
destination_after - destination_before  = q
mint_supply_after                       = mint_supply_before
withheld_before = withheld_after        = 0
foreign_program_invocations             = 0
```

Every subtraction is checked. Decimals authenticate the mint and the
checked-transfer instruction; they never rescale `q`. UI amounts do not cross
the adapter interface. Solana does not give the caller a post-CPI counter of
inner invocations: `foreign_program_invocations = 0` is therefore a
release/parser theorem exercised by the conformance harness, while the live
adapter enforces it by refusing every hook-capable state before CPI. The exact
balance/supply checks are live post-CPI checks.

Admission runs before every collateral CPI and again on the post-state. A
creation-time snapshot is insufficient because owner/delegate/close/frozen
state can drift, a closeable mint could be reinitialized, and any future
admitted extension may have mutable configuration. A future extension
discriminant is a refusal until a new release teaches the parser its exact
location, payload, mutability, authority, CPI, and atom law.

## 4. Token-2022 base profile

The existing conservative profile remains sound and should become one catalog
entry rather than the universal collateral implementation:

- mint extensions: none;
- account extensions: only `ImmutableOwner`;
- `ImmutableOwner`: required on the Hoard, optional on ordinary holder
  accounts if the selected policy admits it;
- mint/freeze authorities: absent;
- Hoard delegate/close authority: absent;
- transfer fees, hooks, confidential balances, nontransferability,
  default-frozen state, permanent delegates, pause controls, permissioned burn,
  and mutable unit scaling: refused; and
- unknown or newly added extension discriminants: refused.

The program must continue to parse actual Token-2022 bytes itself or through a
small separately named adapter dependency. None of this belongs in Eggcrate.

## 5. Legacy SPL profile, where it is sound

Legacy SPL provides fixed 82-byte mint and 165-byte token-account layouts,
transparent integer balances, a checked transfer with authenticated decimals,
and no extension surface. It can satisfy the exact transfer equation.

It does **not** provide Token-2022 `ImmutableOwner`. Legacy instruction 22 is a
compatibility no-op. A local in-process bank test against the exact
`spl_p_token-1.0.0.so` BPF artifact installed at the legacy program id by
`solana-program-binaries` 4.2.1 now demonstrates both facts:

- 600,000 raw atoms debited the holder and credited a PDA-owned Hoard exactly,
  with zero supply change;
- a wallet could not transfer from the PDA-owned Hoard (`OwnerMismatch`, custom
  error 4); and
- an account initialized after legacy `InitializeImmutableOwner` could still
  change its owner using the current owner's signature.

Therefore the admissible legacy theorem is narrower and must be named exactly:

> The Hoard's current owner is the canonical Clutch PDA; no external signer can
> sign for it; the selected Clutch adapter release exposes no
> `SetAuthority(AccountOwner)` route; and the checked deployment/release
> boundary cannot silently change.

This is a valid custody construction under that complete boundary, but it is
weaker than token-enforced immutable ownership. A build that is freely
upgradeable without a release/Realm migration story cannot use this theorem to
call legacy custody immutable.

The legacy parser must require exact base lengths, initialized state, exact
mint/owner/program/decimals, positive bounded supply, absent mint/freeze
authorities, no Hoard delegate or close authority, and no trailing/extension
claims. It must rerun those checks at every transition and apply the same exact
post-delta gate.

## 6. Fee, hook, confidential, frozen, and unit cases

The interface rejects dangerous behavior by semantic class as well as by
program-specific extension number:

| Behavior | Why a generic exact-atom adapter refuses |
| --- | --- |
| fee on transfer | gross debit differs from spendable Hoard credit; withheld state has another authority |
| transfer hook | foreign code and extra accounts enter the collateral transition; reentrancy/account-list semantics change |
| confidential amount/supply | the exact post-delta and Hoard mirror cannot be evaluated from authenticated integers |
| nontransferable/default frozen/pausable | entry or exit can fail under semantics outside the immutable market state |
| permanent delegate/permissioned burn | a third party can seize or alter collateral availability |
| interest/scaled UI amount | UI conversion is mutable or time-dependent; protocol accounting remains in raw atoms only |

An advanced adapter may eventually support a fee token or hook only with a
different liability equation, escrow ownership model, reentrancy model, and
proof obligation. It is not a bitset widening of the exact-one-to-one profile.

## 7. Realm immutability and semantic ownership

Persisted ownership remains singular:

```text
Market --references--> Realm --owns--> parent Profile
                                  Profile --owns--> collateral-policy digest
                                  policy --owns--> collateral adapter release
                                                    + token-program deployment
                                                    + mint/program/decimals

deployment/release manifest --owns--> independent Egg issuance adapter identity
```

Market must not copy mint, decimals, or adapter fields. Each instruction
authenticates the Market/Realm/Profile chain, recomputes the canonical policy
digest, resolves the exact compiled release, and consumes only the checked
profile. Changing collateral mint, family, decimals, parser ceiling, or adapter
release creates a new policy, parent Profile, and Realm. Existing liabilities
never migrate by mutating a Realm.

## 8. Future collateral programs

Future support is a code-and-evidence path, not a permissionless token-program
slot:

1. implement a hostile-byte parser and bounded CPI constructor outside
   Eggcrate;
2. assign a new family/schema and content-derived release identity;
3. state the exact atom, custody, authority, hook, auxiliary-balance, and
   upgrade assumptions;
4. run the common conformance corpus plus family-specific adversarial tests
   against the real external program artifact;
5. add the release to the compiled closed catalog and release manifest; and
6. create new Realm/Profile identities selecting it.

An unrecognized release id, program id, extension, account shape, auxiliary
balance, or foreign invocation fails closed. This permits future programs
without pretending arbitrary programs share SPL semantics.

## 9. Implementation sequence

The safe integration order is:

1. freeze a successor policy encoding that binds `AdapterReleaseId` while
   retaining the current canonical mint/program/decimals and strict flags;
2. split the current `token` module into a fixed claim adapter and a collateral
   interface with `legacy_spl` and `token_2022_base` implementations;
3. make market construction obtain Hoard space/initialization from the selected
   collateral adapter while outcome mints remain Token-2022;
4. replace each instruction's Token-2022 literal with the already-authenticated
   collateral adapter handle—never with an unchecked request program;
5. reload mint/source/destination after every CPI and run common exact-delta
   conformance before committing kernel/accounting state;
6. run the same blank-bank lifecycle under two synthetic Realms, one per
   family, including prefund, rollback, alias, wrong-program, wrong-mint,
   extension drift, and owner-guard negatives; and
7. only then create a separately reviewed real collateral release/profile.

No V1 gate should be weakened in place. Existing V1 Realms remain Token-2022
only; legacy support arrives under a new release/profile identity and an
explicitly different Hoard owner guarantee.

## 10. Executable evidence in this lane

`research/collateral-adapter-conformance` supplies a safe-Rust host model with
eleven tests covering:

- distinct Token-2022 and legacy profiles;
- the conditional legacy PDA custody rule;
- mandatory Token-2022 `ImmutableOwner` on the Hoard;
- fee, hook, confidential, nontransferable, default-frozen, permanent-delegate,
  pausable, and scaled-unit semantic refusals;
- Token-2022 extension drift;
- frozen, opaque, delegated, and closeable Hoard refusals;
- exact debit/credit/supply/withheld/foreign-invocation postconditions;
- decimal mismatch with one-raw-atom preservation;
- explicit future-release admission and unknown-release refusal; and
- the immutable Market/Realm/Profile/policy/release chain; and
- legacy collateral paired with an independently identified Token-2022 Egg
  issuance plane.

`toolchain/probes/token2022` adds the comparative real-bank legacy test described
in section 5. Neither result is an SBF collateral route, public-cluster result,
production deployment identity, or production release qualification.
