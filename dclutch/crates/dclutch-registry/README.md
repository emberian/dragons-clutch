# dClutch successor Registry/Core contract

This crate is the allocation-free semantic boundary between an immutable Market
selection and executable multiprogram authority.

The authority chain has one path:

```text
Market.capability_manifest_id
  -> ExecutionAuthorityManifestV1
       -> existing semantic CapabilityManifest content ID
       -> ExecutionReleaseSetV1 content ID
            -> five exact ArtifactReleaseV2 content IDs
                 -> Program / Loader V3 / ProgramData / slot / code commitment / upgrade policy
```

`ArtifactReleaseV2` is the sole compact onchain artifact authority. Its exact
216-byte `DCLTARF2`, schema-2 body stores the ordered Loader-payload code
commitment at offset 144. The flat ELF SHA-256 in `CheckedReleaseV2` remains a
separate offline provenance fact. Checked manifests and release-tool output are
evidence used to construct and finalize the onchain record; runtime code must
not accept them as an alternative authority.

Artifact finalization verifies the native Loader payload once, in canonical
1 MiB chunks. The 344-byte staging cursor persists only proper partial
coverage. Every call rechecks the Program/ProgramData link, Loader ownership,
deployment slot, authority, and total payload length before hashing its next
chunk. Only the last successful call compares the complete commitment, closes
the cursor, refunds its rent, and makes the raw record final. `Abort` reclaims
the raw record and cursor, returning the funded amounts according to the
cursor's sponsor/bounty policy.

Activation authenticates all five finalized artifact identities against the
release set and current native Loader envelopes. The expensive complete-byte
verification is not repeated: finalization already made the code commitment a
Registry-owned immutable fact, while activation and later reauthentication
still prove that the selected ProgramData address, slot, and authority have not
moved. Activation produces one
1,288-byte derived cache keyed by the release-set content ID. Identical roles
may share one complete cached artifact; partial aliases and substituted caches
refuse.

The cache has one PDA namespace. Under the selected Registry/Core program its
seed tuple is exactly
`[b"dclutch:release-activation:v1", execution_release_set_id]`, in that order.
There is no Market, payer, controller, or instruction-provided release seed.

The Registry SBF adapter implements publication, one-time artifact
verification, activation, and reauthentication from actual Loader V3 Program
and ProgramData accounts. It derives the activation PDA from the release-set
ID and requires Registry ownership; it does not accept instruction-provided
claims that those checks occurred.
