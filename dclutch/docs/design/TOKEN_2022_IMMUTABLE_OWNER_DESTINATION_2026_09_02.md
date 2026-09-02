# The conventional payout destination: admitted twice, refused once, and where the third admission lives

Status: **decision, with two thirds of it landed.** Claims and the host-side
builder now admit the account the Associated Token Account program produces.
Custody does not, and the reason it does not is that widening it would move a
released identity that a live market's realm record pins. This document records
the contradiction, what was changed, what was deliberately not, and the exact
shape of the remaining repair.

---

## 0. What this document decides

1. The Token-2022 `ImmutableOwner` extension is **admitted** wherever this
   protocol authenticates a payout recipient by parse. It is the one extension
   that strengthens the check it appears under, and it is not optional: the ATA
   program adds it to every associated account it creates.
2. `ExactTransferProfileV1` is **not** widened. Its
   `ExtensionStoragePolicy::ExactBaseWidthsOnly` byte is inside the
   `CollateralAdapterReleaseV1` preimage, whose SHA-256 is stored on chain as a
   realm's `collateral_adapter_release_id`, and Custody selects a profile by
   matching that stored id. Redefining the release under an unchanged id is the
   one move that would make the tree and the chain disagree about what one
   identity means.
3. The repair is a **third adapter release**, added beside the two that exist,
   selected by realms a later cohort founds. §4 gives its shape.

---

## 1. The contradiction, measured on chain

`dclutch-wallet-terminal-input-operator` documents the owner's associated token
account as *"the conventional destination for a payout"* and derives it. Under
Token-2022 that account is **170 bytes**, not 165: the ATA program always writes
the `ImmutableOwner` extension, and no caller chooses otherwise. Every token
parse in this protocol was `TokenAccount::parse`, documented as *"Parse exactly
165 bytes, refusing truncation and every extension suffix"*.

Cohort-13 measured it end to end on 2026-09-02. The founder's ATA
`4BENW7YgFnAbagoRr8rAdD8p2kqhwiyvtyDzNKj8ef6W` was created, read back at 170
bytes, and refused. The redemption was paid into a **165-byte auxiliary account
created by hand** — `spl-token create-account` with an explicit keypair, not the
ATA program. That is not a thing a stranger with a browser can do, so the
protocol's own documented destination was unreachable by every wallet it was
for.

Those bytes are now a test vector: `DEVNET_FOUNDER_ATA_V1` in
`crates/dclutch-token-svm/src/state.rs` is that account, read off devnet, and
its last five bytes are the whole finding — account type `2`, then extension `7`
at length `0`.

## 2. Why this extension and no other

`ImmutableOwner` means the token program refuses
`SetAuthority(AccountOwner)` on the account. Every check a payout makes against
a destination — its mint, its owner, its initialized state — is **strengthened**
by that, because the owner just authenticated cannot afterwards be changed. The
base layout offers no such guarantee.

No other extension has that property. A transfer hook, a transfer fee, a
confidential balance, a CPI guard each change what a transfer *means* and each
would have to be reasoned about on its own terms. They stay refused, and the
width is what refuses them: 170 bytes admits exactly one empty TLV entry, and
the type check pins which one.

`TokenAccount::parse_base_or_immutable_owner` is the admission.
`TokenAccount::parse` is unchanged and still refuses the suffix, because the
admission is a decision a call site makes rather than a relaxation everything
inherits.

## 3. What now admits it, and what does not

| site | before | after |
| --- | --- | --- |
| `crates/dclutch-token-svm` | one parser, base widths only | `parse_base_or_immutable_owner` beside it, with the hostile that every other extension still refuses |
| `programs/dclutch-claims-sbf` `rational_terminal_v3::token_amount` | `TokenAccount::parse` | admits; `terminal_settlement_v3` now calls this one function instead of carrying a second copy of it |
| `crates/dclutch-operator` `wallet_terminal_payout_v3` | refused offline as `Custody` | admits, and the poststate projection copies the suffix rather than dropping it |
| the browser's two wasm derivations | inherited the refusal | inherit the admission; the input derivation now emits a complete input over the ATA |
| **`programs/dclutch-custody-sbf`** | `ExactTransferProfileV1::check_transfer_account` | **unchanged — this is the wall** |

The poststate detail is load-bearing, not incidental: the chain hashes the whole
destination account, so a 170-byte destination's poststate is 170 bytes or the
operator and the program commit to different pictures of the same transfer.

`a_conventional_170_byte_associated_token_account_is_refused_by_custody_alone`
in the Claims program-test is the measurement, on real Claims, Custody and
Token-2022 ELFs: the payout is built, submitted, reaches Custody, consumes
**345,149 CU**, and refuses `CustodySbfError::TokenState`. Nothing moves — the
Hoard, the destination balance, the Position and the Custody cursor are all
asserted unchanged. Its control is the same payout into a 165-byte destination,
which commits.

## 4. The remaining repair, and why it is not an edit

`authenticate_transfer_accounts` authenticates both transfer participants
through `ExactTransferProfileV1::check_transfer_account`. That profile is
selected at runtime: a realm record stores
`collateral_adapter_release_id = SHA-256(CollateralAdapterReleaseV1::to_bytes())`,
and the operator side recomputes that digest from the tree's constants and
refuses a mismatch. The preimage contains
`ExtensionStoragePolicy::ExactBaseWidthsOnly` and the two widths.

So widening the existing profile in place would leave every market founded under
that id — cohort-13's included — described by an identity that no longer says
what the code does, and would break every operator read against them the moment
the tree and the chain disagreed. **Devnet is disposable by ruling; a released
identity's meaning is not.**

The repair, for the lane that owns `dclutch-token-svm`'s release surface:

1. `ExtensionStoragePolicy::BaseWidthsOrImmutableOwnerAccounts = 1`, appended.
   The enum is a wire byte; it is append-only for the same reason a refusal band
   is.
2. A third `ExactTransferProfileV1` variant admitting exactly the account
   `parse_base_or_immutable_owner` admits, for **transfer participants only**.
   `check_custody_account` keeps its stronger policy on top and is unaffected
   either way: Custody creates its own vaults at 165 bytes.
3. A third `CollateralAdapterReleaseV1` in `PRODUCTION_ADAPTER_RELEASES`,
   carrying the new policy byte and the same Token-2022 interface provenance.
   The existing two entries are **not touched**, so cohort-13's realm keeps
   matching the release it was founded under and keeps its exact-base-width
   reading.
4. Realm founding selects the new release. That is a cohort boundary, which is
   where identities are supposed to change here.

Not owed and explicitly withdrawn: any change to `TokenAccount::parse`, and any
edit to the two existing adapter releases.

## 5. What this means for cohort-13 and cohort-14

Cohort-13's deployed Custody ELF has the old rule baked into shipped bytes.
**Its payout could not have used the ATA under any version of this tree**, and
the 165-byte auxiliary account was not a workaround for a defect that a later
commit repairs — it was the only destination that cohort could ever pay. The
evidence should say that rather than leaving the ATA reading as a near miss.

Cohort-14 founds its realm on the new release and pays a wallet's own associated
token account, which is the destination the input operator has documented all
along and the one a browser derives when a reader supplies nothing.

Devnet evidence. Not mainnet evidence.
