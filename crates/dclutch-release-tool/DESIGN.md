# Checked release boundary

`dclutch-release-tool` is host tooling, not an onchain semantic owner. A
capability manifest continues to own its opaque semantic release identity, and
`PythReleaseV1` continues to own Pyth provider-release semantics. This tool
hashes the exact selected preimage and never invents a second capability DTO.

`CheckedReleaseV1` is a separate, offline evidence record. Its content ID does
not replace the semantic release ID. It states that one semantic preimage was
checked against one exact built SBF ELF and one exact pair of Loader V3 account
snapshots under named build metadata and assumptions.

## Canonical evidence

The binary manifest has an exact V1 header followed by six length-prefixed
metadata strings and a bounded, strictly sorted set of length-prefixed
assumptions. It commits:

- semantic preimage kind, byte length, and SHA-256 content identity;
- ELF byte length and SHA-256 digest;
- exact Program and ProgramData account-data lengths and SHA-256 digests;
- Program, ProgramData, and loader program public keys;
- Loader V3 deployment slot and optional upgrade authority;
- the fixed Loader V3 ProgramData ELF offset;
- source-tree and Cargo.lock digests;
- source revision, Rust, Solana, and cargo-build-sbf versions;
- target triple and exact build command; and
- explicit assumptions.

The text form is a deterministic machine-readable projection of the binary
record. It is not a second authority. Offline verification decodes the binary
record, rebuilds it from the supplied evidence, and requires byte-for-byte
identity.

## Loader and ELF claim

The existing SDK-free `ProgramV3View` and `ProgramDataV3View` parsers own Loader
V3 enum interpretation. This tool additionally applies the loader's fixed
45-byte ProgramData metadata allocation boundary: the checked ELF must occur
exactly there, and every byte after the ELF must be zero allocation padding.
It refuses to equate a prefix hash, a nonzero padded payload, or a merely
ELF-shaped file with the checked artifact.

The ELF validator requires ELF64, little-endian, current ELF version, shared
object type, and one of the two machine identifiers accepted by Solana's sBPF
loader: legacy `EM_BPF = 247` or registered `EM_SBF = 263`. Current
platform-tools emit `EM_SBF`; accepting only the legacy identifier would make
the release gate reject the artifact that the current runtime actually loads.
This is format evidence, not a proof of compiler correctness or runtime
behavior.

Metadata must state that both accounts were owned by the named loader, that the
Program account was executable, and that ProgramData was not executable. Those
observations cannot be recovered from account data bytes and therefore remain
explicit reproducible inputs.

## Constructed accounts and unowned preimages

Two boundaries in this crate exist because the seven-program reality is ahead of
its contracts, and both are recorded rather than smoothed over.

`loader-accounts` constructs Loader V3 `Program` and `ProgramData` bytes from an
ELF. That is unavoidable before a first deployment exists to observe, but it
makes the account evidence a *prediction*: verifying constructed accounts
against the ELF they were constructed from proves layout, not deployment. The
construction lives here so exactly one implementation of the layout exists, and
its text projection is labeled `predicted-loader-state-not-observed`. A release
that uses it owns saying so in its assumptions.

The one part of that construction that CANNOT be predicted is the authority a
revoked program retains at `[13..45]`: nothing offline knows which key a program
used to have. `--revoked-authority` therefore takes an observation and says so,
relabelling the projection `loader-state-carrying-an-observed-retained-authority`.
It exists because `Option<Pubkey>` cannot express "immutable, formerly A", and
that is the only state a real deployed-then-revoked program is in.

`SemanticPreimageKindV1::Unowned` exists because no first-party contract decodes
a role-program semantic release preimage, while `ArtifactReleaseV1` still
persists one per role. The alternatives were to mislabel those preimages
`capability`—asserting a decoder that does not exist—or to mint a role-release
DTO in host tooling, which would create a second semantic owner. Naming the
absence is the only option that neither lies nor takes ownership it should not
have. Lifting it means a contract, not a tool change.

## Honest boundary

A checked manifest is neither a deployment transaction nor mainnet evidence.
The tool performs no RPC, signing, key access, address derivation, or external
mutation. Its remaining assumptions—including how account snapshots, source
digests, and toolchain strings were obtained—are committed as text rather than
silently upgraded into facts.
