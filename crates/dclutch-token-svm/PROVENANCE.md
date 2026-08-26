# SPL Token ABI provenance

This crate was implemented from ABI facts in two locally cached official
interface releases. They are references only: neither interface crate is a
dependency, and this crate contains no Solana SDK type.

## Legacy Token reference

- Crate: `spl-token-interface` 3.0.0
- Repository: <https://github.com/solana-program/token>
- License declared by the crate: Apache-2.0
- crates.io archive SHA-256:
  `7143c4e676984047a400e2fbd7ac29a1659f2b1b52b897cfde19316b562e4589`
- `src/lib.rs` SHA-256:
  `897b5652c8261da299a7b512fd39744916da84a9fc937650ac6c1853aa482ee4`
- `src/state.rs` SHA-256:
  `7b0b7bacf9610e0cb7b6182b70b9aa4b036a227c916d7a166e93ae666b5b8cf5`
- `src/instruction.rs` SHA-256:
  `4f26ecac09a04fd307adb52024061996fff3b660c12693bb050313eedeed2fcc`

## Token-2022 reference

- Crate: `spl-token-2022-interface` 3.1.1
- Repository: <https://github.com/solana-program/token-2022>
- Homepage declared by the crate: <https://solana-program.com>
- License declared by the crate: Apache-2.0
- crates.io archive SHA-256:
  `821d96d034ea31c4965d182c742153c491ae0abee531331b55771086c5030d86`
- `src/lib.rs` SHA-256:
  `5b47d475fa9da9a3aab2e98adf24d45de654caf773d96f3c89fbc85a96eada77`
- `src/state.rs` SHA-256:
  `4566daf8b06ff2e7b975bed3e631ef4ab9591a95ddabd4117300ca1f639a59d2`
- `src/instruction.rs` SHA-256:
  `672b3890cb58e5700a67f202491069ed44dffcbfa0e7d0b83a0cfbbb05b3a287`
- `src/extension/mod.rs` SHA-256:
  `502b8309d3243f81d3bb7b2ff5f9e412c48d4d68f354b8994792389fc904defd`
- `src/extension/mint_close_authority.rs` SHA-256:
  `d68a5cc324c217e5e18a86dab363f9f67c19e14e828f942f534ff6e9db441a3b`

The archive digests were computed directly from
`~/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/*.crate`; source-file
digests were computed from the corresponding extracted registry directories on
2026-08-24. The relevant ABI facts are:

- program IDs `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` and
  `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`;
- Mint base length 82 and field partition `36/8/1/1/36`;
- Account base length 165 and field partition `32/32/8/36/1/12/8/36`;
- four-byte little-endian `COption` tags in state;
- Token-2022 extended-Mint layout: base Mint bytes through offset 82, zero
  padding to the 165-byte base-account boundary, Mint account-type byte `1`,
  then type/length/value TLV entries;
- `MintCloseAuthority` extension tag `3`, length `32`, with the official
  `MaybeNull<Address>` all-zero absence encoding;
- instruction tags and bodies: `CloseAccount = 9`,
  `TransferChecked = 12 || amount:u64le || decimals:u8`, and
  `InitializeAccount3 = 18 || owner:[u8;32]`;
- the official single-authority account order and signer/writable flags.

## Deliberate scope and remaining trust

The parsers validate byte shape and defined tags. They intentionally ignore the
body of a state `COption::None`, matching the official interface decoder and
packer rather than inventing a stronger canonical encoding.

The Token-2022 zero-extension profile accepts only exact 82-byte Mints and
165-byte Accounts. It does not parse account-type or TLV storage. Any longer
state, including padding plus a zero/unknown/known extension, is refused.

The separately named closeable-Mint V2 profile accepts exactly 202 bytes: one
initialized base Mint, canonical zero padding, the Mint account type, and one
`MintCloseAuthority` entry. It refuses spare TLV capacity, unknown or duplicate
extensions, noncanonical padding, and trailing bytes. This narrow exception is
for lifecycle-created protocol Mints whose zero supply must permit exact rent
reclamation; it does not broaden the ordinary transfer profile.

The fixed builders cover the single-authority/PDA forms needed by dClutch.
Multisignature signer expansion is not claimed by this profile.

An SBF adapter remains responsible for all runtime facts: actual account owner
programs; program identity, executable state, and any deployment/release pin;
account keys and aliasing; signer and writable privilege union; rent; PDA signer
seeds; successful CPI; exact before/after token balances, lamports, and account
closure; transaction rollback; and correspondence between these byte views and
the program binary executing on the selected cluster. These source digests are
ABI provenance, not evidence about any deployed binary.

## Canonical adapter releases

`release.rs` owns two exact 216-byte V1 content preimages. Each commits the
profile kind, token program ID, exact Mint and Account widths, refusal of all
extension storage, the three instruction tags used here, the interface crate
release identity, archive digest, and the three reviewed source-file digests.
The interface release identities are SHA-256 of the exact ASCII strings
`crates.io:spl-token-interface@3.0.0` and
`crates.io:spl-token-2022-interface@3.1.1`.

The canonical SHA-256 content IDs are:

- Legacy exact-transfer V1:
  `956395ad71cc2030b58cfd7900233c89ae96ff049f23d7dbecc3ae8f8e0d6d3f`
- Token-2022 zero-extension exact-transfer V1:
  `228c14f9e501f86138d3f19e5ea815af628c0adf499dc6a93dd8cb185c870e29`

These identify dClutch's parser and instruction semantics. They deliberately do
not identify a deployed token-program binary or its upgrade authority.
