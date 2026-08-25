# dClutch account-profile contract

This standalone safe `no_std`, `no_alloc` contract interprets an exact
content-selected account profile. The profile owns only physical account
framing and register projection: suffix count, exact privileges and data
lengths, canonical alias partitions, key/owner joins to identities already
authenticated by the caller, and projections into runtime-width TransitionVM
V2 register banks.

Each rule also carries exact debit/credit/data-write bits. The interpreter
derives `dclutch_effect_kernel::v2::AccountPermission` values from those
authenticated bytes; the adapter never supplies permission flags. Any effect
permission requires a writable account, and debit or data-write permission
also requires the canonical alias representative's owner to equal an immutable
authenticated input identity.

The interpreter does not hash content, authenticate finalized records, derive
PDAs, inspect Solana `AccountInfo`, or grant program authority. A composing
adapter must authenticate the profile record and pass both its observed content
identity and the descriptor-selected identity. They must be equal and nonzero.
Every distinct account representative must then be anchored by a key or owner
relation to the immutable input identity bank. Profile bytes contain no
expected account identity literals.

Projection is commit-last. The caller supplies immutable input, mutable
scratch, and mutable candidate-output banks with the exact profile widths.
Account and relation refusals leave output unchanged; scratch is copied and
projected only after every validation succeeds. The output can then be supplied
directly as TransitionVM V2 input.

The profile wire has a 32-byte header, one 16-byte rule per account, and one
16-byte operation per relation or projection. Its maximum 1,312-byte size is
the current finalized-record transport bound, not a semantic family limit. It
contains no General/Dealer/Series tag or compiled family enum.

`ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1` is SHA-256 of the separate
`dclutch/schema/account-profile-v1` schema preimage and selects Registry raw /
staging derivation. It is never substituted for the descriptor's
`account_profile`, which is SHA-256 of the complete exact profile bytes.

Run `./check-generated.sh` to rebuild the Lean-owned constants and agreement /
refusal corpus into a temporary file, format it, and compare it to the checked
in Rust.
