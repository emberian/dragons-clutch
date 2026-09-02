# `ProjectDataDigest` — the AccountProfile V2 operation that does not exist

**Status: proposal, unimplemented.** Authored by the General lane (C-05), which
owns the *need*. `crates/dclutch-account-profile-contract` is the Direct lane's
crate and they are mid-refactor beside it, so nothing here is committed into it.
This document is the patch: apply it, then verify it as §7 says, because **no
line below has been compiled or run.**

## 1. The need

`authenticated_general_domain` is the first thing all fifteen General actions
do. Its fourth conjunct is:

```rust
let product_data = data(runtime, product_coordinate)?;
let product_record_digest = hash(&product_data).to_bytes();
if product_record_digest != environment.product_record_digest {
    return Err(GeneralAcceleratorSemanticErrorV3::ProductIdentity);
}
```

`identity::PRODUCT_RECORD_DIGEST` is sourced **zero** times in
`crates/dclutch-general-adapter-contract/src/account_rules_v3.rs`, and it cannot
be sourced, because **the AccountProfile V2 operation vocabulary has twenty
operations and not one computes a digest.** Every projection reads bytes at an
offset, or a key, an owner, lamports, or a tail count.

(Counted exactly: `AccountOperationInputV2` has twenty variants and `v2.rs` has
twenty opcodes, `0..=19`, one to one. Earlier lane reports said *twenty-five*;
that number counted the `RequireData*`/`RequireZeroRange` rule predicates, which
belong to a different enum. Corrected here and in `WAVE.md`.)

Measured 2026-09-01, OpenBatch at N=2 through real Trading ELFs: the refusal
walked `0xC00A InstructionsSysvarAccount` → `ConfigMarket` → **`ProductIdentity`**
as each earlier conjunct was repaired. This is the wall at the end of that walk.

The sibling conjunct, `SEMANTIC_BASIS_ID`, was repaired in `2d9025b3` by
projecting the Portfolio record's `claim_basis_id`. It could be, because a
*field* held the value. **This one has no holder and no expressible rule.**

## 2. The policy question, answered — and it is not the one that was asked

The question posed was whether the kernel policy admits a hash in the projection
pass. On totality grounds it would: SHA-256 over a bounded account is total, has
no allocation, no floating point, and no unchecked cast, so it does not offend
`no_std`/`no_alloc`/total.

**But the crate forbids it for a different and better reason, in its own words**
(`src/lib.rs`, module doc):

> This crate receives plain account observations from an outer runtime adapter.
> It **does not inspect Solana accounts, hash content, derive PDAs, or
> authenticate Registry records.**

So the interpreter must not hash, and the right design needs no exception to
anything. **The digest is an adapter-supplied observation, projected — exactly
like `ProjectKey`, `ProjectOwner` and `ProjectLamports`, which project
adapter-supplied facts rather than reading account bytes.** A digest is a fourth
such fact.

This also means the proposal costs the policy nothing. Nothing is weakened, no
capability is added to the kernel, and the hash stays where hashing already
happens.

## 3. The observation gains one optional fact

`AccountObservationV1` already has the exact convention for this: an opt-in
constructor carrying a fact the adapter established, with the profile deciding
whether it may be consumed. `new_adapter_authenticated_variable_data` is that
convention, in the same file. Follow it.

```rust
pub struct AccountObservationV1<'a> {
    // ... unchanged ...
    /// SHA-256 over `data`, supplied by the adapter, or `None`.
    ///
    /// The interpreter never computes this. It is `None` unless the adapter
    /// chose to establish it, and `ProjectDataDigest` refuses rather than
    /// inventing one.
    data_digest: Option<&'a [u8; 32]>,
}
```

`new` and `new_adapter_authenticated_variable_data` set it to `None`, so every
existing caller keeps compiling and keeps its current behaviour. Add one
constructor beside them:

```rust
/// Construct an observation whose data digest the adapter has computed.
#[must_use]
pub const fn with_adapter_data_digest(self, digest: &'a [u8; 32]) -> Self {
    Self { data_digest: Some(digest), ..self }
}
```

**Why `Option` and not a required field.** Hashing every observed account is a
per-account SHA-256 the vast majority of profiles never ask for, and this
interpreter runs inside the CU-bound hot path. The adapter has already decoded
the profile before it builds observations, so it knows which coordinates carry a
digest operation and can hash exactly those. `Option` makes "the adapter did not
establish this" a refusal rather than a silently wrong register.

## 4. The operation

```rust
/// Project an adapter-established SHA-256 of an account's data into an identity.
///
/// The interpreter does not hash; see the crate module doc. The digest is a
/// fact the adapter supplies alongside the key, the owner and the lamports,
/// and this projects it the same way `ProjectKey` projects the key.
ProjectDataDigest {
    account: AccountCoordinateV2,
    destination: IdentityCoordinateV2,
},
```

Encoding, in `v2/encode.rs`, is the `identity(..)` shape already used by
`ProjectKey` and `ProjectOwner` — no data offset, no stride, so
**`OPERATION_BYTES` stays 16 and no existing opcode is renumbered**:

```rust
Self::ProjectDataDigest { account, destination } =>
    identity(OP_PROJECT_DATA_DIGEST, account, destination, 0, 0),
```

**Opcode 20**, the next free value in `src/v2.rs`, whose table runs `0..=19`
today and is a hand-written private const list imported by the encoder through
`use super::` — one authority, no renumbering needed.

Execution, beside the other identity projections:

```rust
OP_PROJECT_DATA_DIGEST => {
    let digest = observation
        .data_digest()
        .ok_or(Error::DataDigestUnavailable)?;
    write_identity(destination, digest)?;
}
```

and one new error variant, `Error::DataDigestUnavailable` — "a digest projection
ran against an observation whose adapter supplied none." **Total, one output
register, no allocation, bounded input.** Same discipline as its neighbours.

## 5. The Lean side — and the finding underneath it

The brief assumed the operation needs semantics in `formal/dclutch-semantics`
and its emitter. **Checked, and for V2 it does not, which is itself the
finding.**

- The Lean-emitted `src/generated.rs` (from `EmitAccountProfileAbiRust.lean`)
  carries the **V1** opcode table — a different ABI with different numbering
  (`OP_PROJECT_KEY` is `3` there and `2` in V2).
- The **V2** opcode table is hand-written in `src/v2.rs` and emitted from
  nowhere.
- The Lean-emitted V2 file, `src/v2/generated_profile14.rs` (from
  `EmitAccountProfileV2Profile14Rust.lean`), carries header and
  fixed-data-predicate constants, not operations.
- `DClutchSemantics/AccountProfileV2Profile13.lean` says so in its own words:
  *"it does not print or replace the executable AccountProfile interpreter."*

> **The operation vocabulary that every profile in this tree actually executes
> has no Lean model, so a twenty-first operation would receive no formal
> scrutiny at all — and neither did the twenty before it.**

That is not this proposal's to fix and it should not be smuggled in as a
condition of it. It is a separate unit, and the shape is clear: emit the V2
opcode table from Lean the way V1's already is, so the interpreter's vocabulary
and its model cannot drift. Filed here so the next author does not rediscover
that the emitter they went looking for was never written.

## 6. First consumer

`crates/dclutch-general-adapter-contract/src/account_rules_v3.rs`, added the way
the semantic basis was in `2d9025b3` — a **derived** index in front of the
two-operation derived tail (creation-payer owner anchor, then root identity),
never a literal, for the reason that file now records:

```rust
basis_digest if basis_digest == general_product_digest_operation_index_v3(action) => {
    Ok(AccountOperationInputV2::ProjectDataDigest {
        account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_PRODUCT_COORDINATE_V3)?),
        destination: common_identity(identity::PRODUCT_RECORD_DIGEST)?,
    })
}
```

Every action's operation count gains one again, `GENERAL_MAX_ACCOUNT_PROFILE_OPERATIONS_V3`
goes 35 → 36, and Trading's adapter must supply the digest for the Product
coordinate. Expect `Geometry` on `Consider` if the index is placed wrong; it is
the cheapest action and the first to run out of arms.

## 7. How to verify it, and what must be red first

- **The hostile, red before green**: substitute a different account at the
  Product coordinate and require the exact discriminant —
  `GeneralAcceleratorSemanticErrorV3::ProductIdentity` at the accelerator, and
  at the interpreter a digest mismatch, never a bare `is_err()`. Prove it red
  with the substitution and green without.
- **A second hostile with no bare code**: run a profile carrying
  `ProjectDataDigest` against an observation built with plain `new`, and require
  `Error::DataDigestUnavailable`. This is the conjunct that stops a missing
  adapter digest from reading as a zero register.
- **The end-to-end**: OpenBatch at N=2 through real Trading ELFs must stop
  refusing `ProductIdentity`. Name-filtered, `--test-threads=1`, ELFs hashed,
  no `CARGO_TARGET_DIR` override across the nested program-test workspaces.
- **The control**: the accelerator program-test's count must not move except by
  what this changes — it stands at 22 passed / 3 failed, the three being the
  width-258 heap rows that wait on the `BumpHeapV1` extraction.

## 8. What this deliberately does not do

- It does not let the interpreter hash.
- It does not change `OPERATION_BYTES` or renumber an existing opcode.
- It does not make the digest mandatory on every observation.
- It does not add the V2 Lean model, which §5 files separately.
