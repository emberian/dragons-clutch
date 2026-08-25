# dClutch exact-account Effect proof target

This program is a measurement and artifact-proof target, not a deployable
market or custody program. Lean owns its loader-v1 offsets, account-frame words,
state tag, instruction length, and ordered Effect tags. Regenerate those values
with:

```sh
cd formal/dclutch-semantics
lake env lean --run EmitSbfProfile.lean
```

The generated output must exactly equal `src/generated_profile.rs`.

`src/lib.rs` is the deliberately small unsafe boundary around the loader-owned
input pointer. Its memory extent, alignment, ABI version, and duplicate-account
serialization are assumptions until joined to the pinned loader semantics. The
program authenticates an ordinary stored signer, not a controller PDA; it owns
an internal projection, not Realm-selected SPL custody. Those omissions are why
this artifact cannot succeed the current Direct route.
