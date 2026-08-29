# Direct Hot PDA syscall audit — 2026-08-28

## Scope and method

This is a static reachable-call audit of the canonical Direct ordinary Hot
fixture, rooted at Registry transparent continuation and including the Trading,
Claims sparse-transfer, and Custody delegated-transfer programs. The baseline
source was `c5ee95ab70c8be8588e8f3b52f3da920d6188eeb`. Counts include every reachable
`Pubkey::find_program_address` or `Pubkey::try_find_program_address`, including
the corresponding `sol_try_find_program_address` syscall in a child CPI. They
do not include setup, seal creation, ALT mutation, or test-only code.

This is not CU evidence. The final Trading caller still requires a fresh SBF
build, frame report, and exact 20-seed measurement before release acceptance.

## Exact phase graph

```text
Registry Hot continuation
  activation cache -> Core/Trading deployments -> admission signer
    Trading Hot
      continuation admission -> activation/deployments -> Core Market
      -> Direct root -> Product graph -> seal
      -> seller/buyer lifecycle plans -> child caller authorities
        Claims sparse transfer
          caller -> Trading/Claims/Core deployments -> Claims aggregate
          -> Product graph -> Core Market -> seller/buyer Positions
        Custody delegated transfer (one or two active routes)
          caller -> activation/Trading deployment -> Core Market -> replay
          -> Realm raw/staging -> Custody authority -> Token CPI
```

The shipped fixture has Claims plus one seller-terminal Custody route. A valid
fee-continuation execution has two Custody routes.

## Reachable search counts

| Phase | Baseline fixture | This change | Reason |
|---|---:|---:|---|
| Registry | 4 | 2 | Loader-owned Program links replace two ProgramData PDA searches |
| Trading | 19 | 17 | Loader-owned Program links replace two ProgramData PDA searches |
| Claims child | 19 | 14 | one cache search plus two authenticated-bump creates; three Loader links |
| Custody child, each | 10 | 7 | one authenticated-bump cache create, one Loader link, reused authority bump |
| **Fixture total** | **52** | **40** | one Custody route |
| **Two-Custody total** | **63** | **48** | fixed path plus two optimized Custody routes |

The fixture therefore removes 12 reachable PDA searches without changing a
wire, account frame, PDA seed, address, or program instruction. The two-Custody
shape removes 15.

## Bump and identity ownership

- The Registry activation-cache V1 state does not persist a bump. The first
  role check in each child performs the canonical search and returns an opaque
  `AuthenticatedActivationCacheBumpV1`. Only that semantic-owner crate can
  construct the witness. Later checks reproduce and compare the exact address
  with `create_program_address`. A wrong, cross-release, or cross-Registry bump
  cannot reproduce the cache key and refuses before cache bytes are admitted.
- Custody already searched the exact transfer-authority PDA while validating
  the token frame, then searched the byte-identical seeds again solely to obtain
  the Token CPI signer bump. The validation now returns an opaque local witness
  and the CPI reuses it. No request or replay DTO gains bump authority.
- Upgradeable Loader V3 Program accounts already persist the ProgramData
  address the Loader follows. Runtime authentication now requires the exact
  three-way equality `Loader-owned Program link == activated ArtifactRelease
  ProgramData == supplied Loader-owned ProgramData account`, alongside all
  existing executable, owner, slot, authority, and ELF-pin checks. Re-deriving
  the persisted Loader-owned link did not authenticate an additional fact.

## Deliberately unchanged searches

- Registry continuation admission and child caller authorities are
  request-scoped. No authenticated persisted owner carries their canonical
  bump.
- Product and Realm content records cannot self-store their own bumps because
  their complete content digest is a PDA seed. Product graph bumps belong in a
  future Core-owned Market profile, not in a child request or Claims DTO.
- Core Market, Claims aggregate/Positions, Custody replay, Direct root, and seal
  layouts do not currently own bumps. Migrating them would be an ABI/release
  change, outside this bounded Hot-only cut.
- Vacant Direct maker roots require canonical first-use searches. Existing
  maker state already persists its bump, but changing lifecycle planning to
  select the persisted branch before derivation needs a separate proof and was
  not folded into this patch.

## Focused refusal and identity evidence

- Activation-cache tests compare the searched and authenticated-bump receipts
  byte-for-byte and refuse a wrong bump.
- Existing activation/deployment hostiles retain substituted cache, wrong
  Registry/release, malformed body, writable frame, Program/ProgramData link,
  slot, authority, and ELF-pin refusals.
- Custody tests reproduce the exact authority address from the carried bump and
  prove a wrong bump cannot reproduce that key.

