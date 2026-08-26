# dclutch-economic-sbf

This is the bounded physical successor adapter for the shared economic
microkernel. It is a new program boundary; it is not a handler in the legacy
`dclutch-sbf` monolith and it does not depend on a Direct controller.

The program owns one fixed 1,136-byte two-holder projection. The projection is
the sole persisted owner of its claim partition, complete-set supply, Hoard
liability, lifecycle, and optimistic revision. Market identity and execution
release membership remain references to their separate canonical owners.

Release-set profile-1 roles are used without local aliases: Core is role `0`,
Claims is `1`, Trading is `2`, Resolution is `3`, and Custody is `4`. This
bounded deployment requires Claims and Custody to name this same exact
program/release pair. Founding is admitted by Core; open operations by Trading;
redemption and retirement by Resolution.

The SDK-free adapter contract stages the `dclutch-economic-kernel` transition.
This Solana layer authenticates account memory, hashes the exact release-set
bytes, parses exact legacy SPL Token state, and performs the one derived
`TransferChecked` CPI. Projection bytes are written only afterward. Solana
transaction rollback is therefore the boundary for a CPI that succeeds before
a later refusal.

Materialization in this slice changes the canonical claim representation owned
by the projection. It does not claim to mint a transferable Token-2022 wrapper;
that would be a separate wrapper program and release role, not hidden here.
