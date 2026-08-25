# dClutch exact-account claim executor

This program is the generated claim/replay child of the compiled Direct
successor experiment. Lean owns its loader-v1 offsets, account-frame words,
state tags, instruction length, and four ordered Effect tags. Regenerate them
with:

```sh
cd formal/dclutch-semantics
lake exe emit-claim-sbf-profile
```

The generated output must exactly equal `src/generated_profile.rs`.

The child receives a controller authority and four canonical mutable accounts:
seller replay root, buyer replay root, seller maker/outcome Position, and buyer
maker/outcome Position. Each replay nonce and claim balance therefore has one
semantic owner. A pair-specific projection cannot fragment replay protection or
duplicate claims. Collateral does not exist in this program's state; the custody
child owns that physical boundary.

The raw loader adapter is the explicitly unsafe boundary. It assumes the pinned
Solana ABI-v1 buffer extent and alignment, then validates exact account count,
privileges, data lengths, owners, controller bindings, non-aliasing, state tags,
outcomes, plan tags, conservation, and arithmetic before writing four fields.

The earlier 1,872-byte combined-projection artifact and its single-path qedsvm
proof are historical evidence only. This canonical-owner successor must receive
a fresh artifact-specific lift before any machine-code theorem is claimed for it.
