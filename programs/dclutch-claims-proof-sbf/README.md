# dClutch exact-account claim executor

This program is the generated claim/replay child of the multiprogram Direct
successor experiment. Lean owns its loader-v1 offsets, account-frame words,
state tag, instruction length, and four ordered Effect tags. Regenerate them
with:

```sh
cd formal/dclutch-semantics
lake env lean --run EmitClaimSbfProfile.lean
```

The generated output must exactly equal `src/generated_profile.rs`.

The 80-byte projection contains only its controller authority, selected outcome,
two next nonces, and two claim balances. Collateral does not exist in this
program's state; the custody child owns that physical boundary. The raw loader
adapter remains unsafe and assumes the pinned Solana ABI-v1 buffer extent and
alignment until joined to loader semantics.
