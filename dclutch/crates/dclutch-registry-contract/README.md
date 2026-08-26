# dClutch successor Registry/Core contract

This crate is the allocation-free semantic boundary between an immutable Market
selection and executable multiprogram authority.

The authority chain has one path:

```text
Market.capability_manifest_id
  -> ExecutionAuthorityManifestV1
       -> existing semantic CapabilityManifest content ID
       -> ExecutionReleaseSetV1 content ID
            -> five exact ArtifactReleaseV1 content IDs
                 -> Program / Loader V3 / ProgramData / slot / ELF / upgrade policy
```

`ArtifactReleaseV1` is the sole compact onchain artifact authority. Checked
build manifests and release-tool output are evidence used to construct and
finalize it; runtime code must not accept either as an alternative authority.

Activation authenticates all five finalized artifact identities against the
release set and current chain-derived deployment observations. It produces one
1,288-byte derived cache keyed by the release-set content ID. Identical roles
may share one complete cached artifact; partial aliases and substituted caches
refuse.

The cache has one PDA namespace. Under the selected Registry/Core program its
seed tuple is exactly
`[b"dclutch:release-activation:v1", execution_release_set_id]`, in that order.
There is no Market, payer, controller, or instruction-provided release seed.

An SBF adapter remains to be implemented. It must authenticate finalized raw
record content IDs, parse actual Loader V3 Program and ProgramData accounts,
hash the exact ELF tail, derive the activation PDA from the release-set ID, and
require Registry ownership. The contract does not accept instruction-provided
claims that those checks occurred.
