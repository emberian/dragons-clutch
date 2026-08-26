# dClutch exact-account claim executor

This program is the generated claim/replay child of the compiled Direct
successor. Lean owns both exact loader-v1 profiles: the four-effect inline
route and the 16-byte registered-fill route. Regenerate their offsets,
account-frame words, state tags, and instruction constants with:

```sh
cd formal/dclutch-semantics
lake exe emit-claim-sbf-profile
```

The generated output must exactly equal `src/generated_profile.rs`.

Both routes receive a controller authority and four canonical mutable accounts.
Inline execution uses a replay root and Position for each maker. Registered
execution replaces both replay roots with the canonical 232-byte registration
states; their local sequences and residual quantities become the sole replay
and fill authority. Positions remain the sole claim-balance owner. A
pair-specific projection therefore cannot fragment replay protection or
duplicate claims. Collateral does not exist in this program's state; the
custody child owns that physical boundary.

Registered instructions contain only canonical magic/version bytes and one
positive fill. The owner decodes both exact signed intents, joins maker,
Market, generation, outcome, controller, and Position facts, then executes the
168-byte Lean-owned lifecycle program independently for each registration.
Only after both executions and claim arithmetic succeed does it update the two
registration states and the two Position balances.

The raw loader adapter is the explicitly unsafe boundary. It assumes the pinned
Solana ABI-v1 buffer extent and alignment, then validates exact account count,
privileges, data lengths, owners, controller bindings, non-aliasing, state tags,
outcomes, plan tags, conservation, and arithmetic before writing state. The
registered real-ELF campaign covers two successive residual fills plus hostile
padding, terminal replay, maker/Market mismatch, partial FOK, ownership,
privilege, arithmetic, and exact four-account rollback cases.

The earlier 1,872-byte combined-projection artifact and its single-path qedsvm
proof are historical evidence only. This canonical-owner successor must receive
a fresh artifact-specific lift before any machine-code theorem is claimed for it.
