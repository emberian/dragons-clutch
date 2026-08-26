# Canonical Custody SBF adapter

This is the one physical collateral child shared by Core, Claims, Trading, and
Resolution. It accepts only the Lean-owned `CustodyRequestV1`, independently
reauthenticates the calling Registry role, authenticates the exact immutable
Realm and its selected legacy Token or zero-extension Token-2022 profile, and
commits replay bytes only after every real CPI and postcondition succeeds.

The caller signs with the sole release-set-owned PDA projection
`["dclutch:role-authority:v1", releaseSet, market, roleByte, context,
SHA256(custodyRequest)]` under its selected program. This is acyclic: the
upstream semantic request digest is already inside the Custody request; its
digest then selects the authority used by any outer effect envelope. Custody
signs token movement with
`["dclutch:custody-authority:v1", market, releaseSet]`. Program-owned token
vaults are PDAs under
`["dclutch:custody-vault:v1", market, releaseSet, vaultContext,
compartmentByte]`; this prevents one Vault from being relabeled as Hoard,
fees, liveness capital, Series escrow, or recovery reserve.

## Account frames

Every route begins with this nine-account prefix:

| # | Account | Privilege |
| -: | --- | --- |
| 0 | release-pinned caller authority PDA | signer, readonly |
| 1 | persisted Core Market | readonly |
| 2 | Registry activation cache | readonly |
| 3 | exact persisted Registry program | executable, readonly |
| 4 | selected caller-role program | executable, readonly |
| 5 | caller ProgramData | readonly |
| 6 | Registry-owned finalized Realm raw record | readonly |
| 7 | vacant Realm staging cursor | readonly |
| 8 | Custody replay PDA | writable |

The 352-byte Market state is the join authority for its Market, Realm,
generation, release set, and Registry coordinates. Custody independently
derives and hashes the sole Registry raw/staging Realm pair and never accepts a
Core-owned Realm copy.

Suffixes are exact:

- `InitializeReplay` (12 accounts): payer signer+writable, System program,
  Rent sysvar.
- `OpenVault` (16): Mint readonly, vacant destination Vault writable, Custody
  authority readonly, Realm token program executable, payer signer+writable,
  System program, Rent sysvar.
- `Transfer` (14): Mint readonly, source writable, destination writable,
  Custody authority readonly, Realm token program executable.
- `CloseVault` (14): Mint readonly, source Vault writable, Custody authority
  readonly, Realm token program executable, rent-refund account writable. The
  refund may also be a transaction signer; signer status cannot change the
  persisted beneficiary or close semantics.
- `CloseReplay` (10): the exact persisted replay-rent refund beneficiary
  writable. The current Registry-authenticated caller role may close only its
  exact replay context, at the exact next revision, after every Vault opened
  under that context has been closed.

Each External side is independently bound to its exact semantic token-account
owner. External sources must also have delegated the exact amount to the sole
release/Market-pinned Custody authority; the caller cannot select an authority.
Every non-external side must be the canonical compartment/context Vault, owned
by the Custody authority, with no delegate, native reserve, or close authority.
Return data is the exact 384-byte `CustodyReceiptV1`; its producer must be the
Registry-selected Custody program. The caller verifies the request digest,
parent-plan digest, replay revision/digest, exact token deltas, and poststate
commitment before committing its own semantic state.

`./run-program-test.sh` builds and executes the real Custody, Registry, and
test-caller ELFs against ProgramTest's bundled canonical legacy Token and
Token-2022 programs. The campaign measures every lifecycle route and proves
byte-for-byte rollback after a caller deliberately refuses after successful
Custody and token CPI. It also executes a transfer between distinct External
owners and refuses stale replay, wrong delegate, and either side's owner
substitution without changing replay or token state. It also refuses early,
stale, and foreign-role replay closure, reclaims replay rent only at zero live
Vaults, and refuses a Vault operation after the replay has been reclaimed.

Replay account rent and token-vault rent are explicit `rent_lamports` and never
collateral `amount`. Custody has no Hoard balance, fee balance, liveness balance,
or liability-supply DTO; those facts remain owned by their semantic programs.
Each replay owns its checked `open_vault_count`; `OpenVault` increments it and
`CloseVault` decrements it. Custody proves zero live Vaults before
`CloseReplay`, while the authenticated caller's canonical state machine remains
the semantic owner of terminal/quiescent authorization and of refusing a fresh
`InitializeReplay` after termination. A static client is not an authority for
either condition.
