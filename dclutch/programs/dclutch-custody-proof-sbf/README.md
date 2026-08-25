# dClutch physical custody experiment

This adapter executes the two indivisible transfers in Lean's 40-byte `DCCP`
V1 plan against the real legacy SPL Token program. It accepts only the pinned
controller PDA as semantic caller and only a signed transfer delegate already
installed on the buyer's source account. The controller is expected to derive
that delegate as the maker's replay-root PDA.

The experiment pins legacy SPL Token directly. It does not yet authenticate an
immutable Realm, token-program ProgramData, or controller release. It exists to
measure and adversarially test the real physical boundary before those records
are added to the generated account profile.

Accounts are exactly controller PDA, replay-root delegate, Mint, buyer source,
seller destination, venue destination, and executable token program. The
adapter checks exact base-state widths and complete pre/post token state around
up to two `TransferChecked` CPIs.
