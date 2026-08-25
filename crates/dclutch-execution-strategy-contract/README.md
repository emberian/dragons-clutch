# dClutch execution-strategy contract

This SDK-free, `no_std`, `no_alloc` crate defines the fixed semantic contract
for executing one content-addressed `CapabilityProgramV1` through more than one
implementation strategy without creating another state or effect authority.

The descriptor remains the sole transition/effect meaning. A strategy
certificate binds a checked artifact to that exact descriptor, account
projection, and effect schema. Program, ProgramData, ELF, deployment slot, and
upgrade policy remain solely in the referenced `ArtifactReleaseV1`. The
accelerator receives the same canonical runtime-width input register bank as
the interpreter and returns a candidate output bank or refusal. Trading
compares both results, runs the one common effect projector, and is the only
component allowed to apply the effect or write the capability root.

The first profile is deliberately comparison-only. A finalized certificate and
an authenticated deployment prove identity, not semantic correctness: either
can be published for an incorrect accelerator. AOT-only execution therefore
remains unavailable until Registry owns an immutable
`descriptor -> certificate -> ArtifactRelease` admission and reauthentication
route, or an onchain verifier checks the named equivalence proof. This refusal
is part of the public contract, not an operator convention.

The wire is runtime-width. It carries register counts, not an outcome-count enum
or an `N = 2..16` branch. The first SVM return-data transport can carry at most
864 register bytes because the pinned local Solana SDK exposes a 1,024-byte
return-data ceiling and the acknowledgement header is 160 bytes. That is a
chain-derived physical transport profile, not a semantic Product-width limit. A later
authenticated scratch-page transport can lift it without changing descriptor
meaning or strategy certificates.
