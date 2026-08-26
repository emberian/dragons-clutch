# dClutch capability program contract

This safe `no_std`, `no_alloc` crate owns the typed view over the Lean-owned
`CapabilityProgramV1` descriptor and immutable Trading child root. A descriptor
is a finalized, account-resident raw record. The exact authority equation is
`entry.release_id == selection.capability_release == SHA256(all canonical
descriptor bytes)`. A family config may bind that selected descriptor identity
but cannot replace it with a hard-coded family release constant. The identity
is never instruction data and never an executable artifact-release identity.

The 280-byte descriptor header binds exact kind, schema, account, derivation,
capacity, allowed-effect content identities, and the exact mutable root-tail
width. Descriptor artifact profile 2 is followed by one canonical runtime-width
TransitionVM `ProgramV2`; legacy V1 fixed-bank bodies fail closed. The existing
1,312-byte finalized-record ceiling admits at most 42 V2 instructions, making
the exact maximum canonical descriptor 1,304 bytes. Under the pinned Solana
`Rent::default()` profile its account requires 9,966,720 rent-exempt lamports.
The descriptor owns those semantics independently of execution strategy. The
current Trading artifact interprets ProgramV2; a later checked profile could use a
Registry-authenticated, translation-validated stateless accelerator over the
same descriptor, while Trading remains the canonical state/effect authority.

`SupportedContentV1` is only a compiled physical-profile gate for the current
foundation. It makes unknown schema IDs fail closed, but family-specific Rust
values supplied through that gate are still a closed adapter support list.
This crate alone therefore does not satisfy the open-family successor gate.
That gate additionally requires finalized and interpreted AccountProfile,
derivation, and effect-projection languages (or an equivalently certified AOT
profile), so their schema identities select authenticated data rather than a
compiled General/Dealer/Series list.

The existing immutable-record protocol publishes that maximum in four bounded
transactions: Begin (176 instruction bytes), Append page 0 (40 + 768), Append
page 1 (40 + 536), and Finalize (16). Activation receives the finalized record
account, checks its raw-record owner/PDA/finalized-cursor absence and complete
digest, and requires that digest to equal the selected `release_id`. It does
not inline 1,304 descriptor bytes into the activation packet.

One Trading-owned root account is the 232-byte immutable
`CapabilityRootHeaderV1` followed by the descriptor-sized mutable family-state
tail. The header embeds the exact `CapabilityExecutionSelectionV1`
reconstructed and authenticated during activation. It is a projection, not a
second semantic owner. The manifest `child_schema_id` and descriptor
`root_schema` identify the tail schema; the common header is implied by the
Trading artifact profile. Hot actions authenticate the root owner, exact PDA,
release set, Market, generation, selection, and exact total width; family code
receives only the mutable tail and does not repeat the 144-byte selector.

Run `./check-generated.sh` to compare the checked-in Rust constants with the
Lean emitter.
