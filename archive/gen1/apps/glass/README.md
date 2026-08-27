# Glass static client

The implemented client lives in [`../static-client`](../static-client). It is a
reproducible, dependency-free, secret-free, inspect-only projection and an
unsigned intent-byte compiler. It owns no protocol truth, index, matcher,
source, wallet, signer, or submission authority.

It contains no bundled capability ledger, fixture account set, default RPC, or
restart descriptor. Attachment requires operatord's canonical
`/v1/session` projection, which binds the explicitly configured RPC endpoint
digests, genesis hash, Program/ProgramData/ELF coordinates, checked release and
profile identities, finalized canonical account decodes, and onchain-derived
restart cursors. Missing or changing session state is displayed as unavailable.

The client remains unsigned. An immutable content digest and checked release
manifest—not a mutable Pages URL—would identify a future published frontend.
