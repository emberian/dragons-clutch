# dClutch execution release set contract

This allocation-free contract owns the one canonical semantic map from five
execution roles—Core, Claims, Trading, Resolution, and Custody—to exact program
identities and checked artifact-release content identities.

The profile is deliberately not “five distinct programs.” Roles may share one
identical `(program, artifact release)` pair. The codec refuses either half of
that pair being aliased independently, because one live program identity cannot
simultaneously denote two artifact releases and one artifact release cannot
authorize two program identities.

The 336-byte value is a content-addressed preimage. This crate does not hash it,
inspect Loader accounts, admit a release, read a Market, or execute a CPI. A
composing adapter must:

1. authenticate a finalized record under
   `EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1`;
2. hash the exact record bytes and match the one release-set identity selected
   by the immutable Market capability profile;
3. resolve every referenced artifact release through checked Program,
   ProgramData, deployment-slot, ELF, and upgrade-policy evidence; and
4. persist or authenticate one activation binding before lending any
   controller PDA authority.

The live capability manifest and controller are intentionally not wired yet.
That convergence requires a successor manifest profile; reinterpreting the
existing `release_id` field in place would create two meanings for already
founded Markets.
