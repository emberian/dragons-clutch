# dClutch execution-strategy contract

This SDK-free, `no_std`, `no_alloc` crate defines the fixed semantic contract
for executing one content-addressed `CapabilityProgramV3` through interpreted,
shadow-AOT, or admitted-AOT strategies without creating another state or effect
authority. The V1 comparison wire remains exported only for migration.

The V2 identity graph is deliberately acyclic:

```text
CapabilityProgramV3 -> ExecutionStrategyProgramV2
                       |-> underlying TransitionVM
                       |-> optional Certificate -> ArtifactRelease
                       `-> optional Admission -> exact Certificate
```

The descriptor binds AccountProfile, RequestProfile, EffectProgram, and the
Strategy. The Strategy binds the underlying TransitionVM and presence-tagged
Certificate/Admission identities. The Certificate binds the exact equivalence
tuple (AccountProfile, RequestProfile schema/program, Transition schema/program,
EffectProgram, ArtifactRelease, compiler, toolchain, and translation-validation
digest), but never points back to its descriptor or Strategy. The minimal
Registry admission authorizes only one exact Certificate for admitted AOT.
Finalized Certificate bytes alone remain insufficient.

Program, ProgramData, ELF, deployment slot, and upgrade policy remain solely in
the referenced `ArtifactReleaseV1`. An accelerator is stateless: it receives an
authenticated runtime-width bank and returns candidate bytes or refusal.
Trading still interprets or validates the selected result, projects the one
common EffectProgram, and is the sole root, FundingState, effect, and scratch
page writer.

The transport carries `u32` register and semantic tail counts; it has no Product
outcome cap. A 144-byte acknowledgement header leaves exactly 880 bytes under
the 1,024-byte SVM return-data bound. Larger banks use Trading-owned,
authenticated scratch pages. Every request, acknowledgement, and page binds the
whole-bank digest and length, invocation context, canonical chunk count, index,
and offset so mixed or reordered pages refuse without changing semantic width.
