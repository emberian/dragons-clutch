# Source V3 Pyth parser SBF

This is the first-party, read-only parser release used by SourceSeries 77/v2
action 4. It is a separate deployed program from both Dragon's Clutch and the
Pyth receiver.

Its only successful instruction accepts exactly three read-only, non-signing
accounts in this order:

1. a 256-byte immutable parser config owned by this parser;
2. the exact config-selected `PriceUpdateV2` account, owned by the configured
   Pyth receiver; and
3. the canonical Clock sysvar.

Clutch constructs the fixed 24-byte request from the authenticated
`OpenRawPage` cursor and release clock policy. The parser requires the exact
`prev_publish_time < boundary <= publish_time` crossing, full Pyth
verification, canonical padding, configured feed ID, nonfuture and bounded-age
publish time, nonfuture and bounded-lag posted slot, and checked integer
normalization. Decimal downscaling has one named rounding boundary: the low
endpoint rounds down and the high endpoint rounds up.

On success it sets exactly the canonical 120-byte `ParserOutputV1` as Solana
return data. It performs no CPI and mutates no account. Posting or refreshing
the receiver-owned update is a separate transaction step outside action 4.

The reviewed `PriceUpdateV2` layout and upstream provenance are pinned in
[`../../research/source-profile-v1/PROVENANCE.md`](../../research/source-profile-v1/PROVENANCE.md).
An ELF is not a release merely because it builds: operator and local-session
manifests require its explicit program, ProgramData, deployment slot, and ELF
SHA-256 identity before Source can select it.
