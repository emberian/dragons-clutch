# Glass static client

The implemented client lives in [`../static-client`](../static-client). It is a
reproducible, dependency-free, secret-free, inspect-only projection and an
unsigned intent-byte compiler. It owns no protocol truth, index, matcher,
source, wallet, signer, or submission authority.

Its bundled capability ledger is a historical offline snapshot and has not yet
been regenerated after the 2026-08-22 architecture review. The implementation
therefore remains offline and unsigned. An immutable content digest and checked
release manifest—not a mutable Pages URL—would identify a future release.
