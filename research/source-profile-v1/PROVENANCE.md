# Upstream provenance and fixture derivation

Review performed 2026-08-18.  Only current primary official documentation and
official project source were used for protocol facts.  No public RPC, wallet,
transaction, deployment, or funded account was used.

## Pyth

Repository revision:
[`pyth-network/pyth-crosschain@ec456fca86adf2ab451ef3622833097c2a36ab00`](https://github.com/pyth-network/pyth-crosschain/commit/ec456fca86adf2ab451ef3622833097c2a36ab00)
(commit time 2026-08-18T13:35:09Z).

| Path | Fact used | SHA-256 of reviewed raw file |
| --- | --- | --- |
| [`pythnet/pythnet_sdk/src/messages.rs`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/pythnet/pythnet_sdk/src/messages.rs) | `PriceFeedMessage` fields and the unique `prev_publish_time < T <= publish_time` rule, including migration/equal-time caveats | `859f315c9474e694dc5ddb04d1900c5bae8b115a347c4e59746123245cbabc2a` |
| [`target_chains/solana/pyth_solana_receiver_sdk/src/price_update.rs`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/target_chains/solana/pyth_solana_receiver_sdk/src/price_update.rs) | `VerificationLevel`, `PriceUpdateV2`, field order, and 134-byte allocated size | `12d0ce8bc3907ae2949043397eaf3d5bd25deed98450c6969d957be402c807ae` |
| [`target_chains/solana/programs/pyth-solana-receiver/src/lib.rs`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/target_chains/solana/programs/pyth-solana-receiver/src/lib.rs) | posting, full verification, caller write authority, reclaim, and mutable receiver governance/config | `554462c221f075e8d7b85f685ea64f3c6ee4d8224a8c475af33c8234d9bc6f8f` |
| [`apps/hermes/server/src/api/rest/v2/timestamp_price_updates.rs`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/apps/hermes/server/src/api/rest/v2/timestamp_price_updates.rs) | timestamp endpoint returns first update with publish time at least requested time | `a79967d1a868ed4ab8fa6f3dfee25faef7f189d248af279379bd0dbb600dfc53` |
| [`target_chains/solana/sdk/js/pyth_solana_receiver/src/address.ts`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/target_chains/solana/sdk/js/pyth_solana_receiver/src/address.ts) | default and pro-compatible receiver/Wormhole/push program IDs | `40b52e3fd1c64e81da3401df7ef8134020177c3ade9e6be16b4ba6bc023255ac` |
| [`apps/developer-hub/content/docs/price-feeds/core/upgrade/how-it-works.mdx`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/apps/developer-hub/content/docs/price-feeds/core/upgrade/how-it-works.mdx) | announced cutover and 3-of-5 router architecture | `f638f02ecb41efac50884232bace64c0cd0ac36cae73f5c9b2321327874fcf8f` |
| [`apps/developer-hub/content/docs/price-feeds/core/use-historical-price-data.mdx`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/apps/developer-hub/content/docs/price-feeds/core/use-historical-price-data.mdx) | Benchmarks flow, interval semantics, and API-key notice | `5c0a2fc697848c3c52923a10a759bc97896d5d4d63aaff2fb91e00061d294158` |
| [`target_chains/solana/pyth_solana_receiver_sdk/Cargo.toml`](https://github.com/pyth-network/pyth-crosschain/blob/ec456fca86adf2ab451ef3622833097c2a36ab00/target_chains/solana/pyth_solana_receiver_sdk/Cargo.toml) | reviewed SDK version 2.0.0 and `pro-compatible` feature | `31cb23af12d28c26ecb5c5481e3a2997e267f583839e1fc888d335805c7031b6` |

The human-facing official pages checked were [historical Pyth
Benchmarks](https://docs.pyth.network/price-feeds/core/use-historical-price-data),
[Solana contract
addresses](https://docs.pyth.network/price-feeds/core/contract-addresses/solana),
and [the Pyth Core upgrade
architecture](https://docs.pyth.network/price-feeds/core/upgrade/how-it-works).

## Switchboard

Repository revision:
[`switchboard-xyz/switchboard-sdk@50297eb915cdd15e3c7c8df2173fa0d093d45227`](https://github.com/switchboard-xyz/switchboard-sdk/commit/50297eb915cdd15e3c7c8df2173fa0d093d45227)
(commit time 2026-08-13T16:49:45Z).

| Path | Fact used | SHA-256 of reviewed raw file |
| --- | --- | --- |
| [`oracle_quote/quote_verifier.rs`](https://github.com/switchboard-xyz/switchboard-sdk/blob/50297eb915cdd15e3c7c8df2173fa0d093d45227/solana/rust/switchboard-on-demand/src/on_demand/oracle_quote/quote_verifier.rs) | recent-slot/SlotHashes verification and max-age check | `4e17641c4e11d833ae369fef178869fdb80aa75e4a8e264a7a4a7c2fadf3a567` |
| [`oracle_quote/feed_info.rs`](https://github.com/switchboard-xyz/switchboard-sdk/blob/50297eb915cdd15e3c7c8df2173fa0d093d45227/solana/rust/switchboard-on-demand/src/on_demand/oracle_quote/feed_info.rs) | signed slot hash and feed payload fields | `360f7df964e1c44697f94521709bda1a7f793c6d147e4deebfe3fe30b48d4f1e` |

The official [Solana/SVM deployment
page](https://docs.switchboard.xyz/docs-by-chain/solana-svm) and [managed feed
deployment flow](https://docs.switchboard.xyz/custom-feeds/build-and-deploy-feed/deploy-feed)
were also checked.

## Orca

Repository revision:
[`orca-so/whirlpools@630c0e01b74ad88eab69f8ed4cc2d3dc9a3d0bd5`](https://github.com/orca-so/whirlpools/commit/630c0e01b74ad88eab69f8ed4cc2d3dc9a3d0bd5)
(commit time 2026-08-18T18:22:24Z).

| Path | Fact used | SHA-256 of reviewed raw file |
| --- | --- | --- |
| [`programs/whirlpool/src/state/oracle.rs`](https://github.com/orca-so/whirlpools/blob/630c0e01b74ad88eab69f8ed4cc2d3dc9a3d0bd5/programs/whirlpool/src/state/oracle.rs) | named Oracle account is adaptive-fee state | `0e60f057062c4d4653073859a2b9bfb294e06868af1303e06cb907742f752ca2` |
| [`programs/whirlpool/src/state/whirlpool.rs`](https://github.com/orca-so/whirlpools/blob/630c0e01b74ad88eab69f8ed4cc2d3dc9a3d0bd5/programs/whirlpool/src/state/whirlpool.rs) | current pool `sqrt_price`/tick state and lack of an observation history | `dc6f34e643416068c21ec0f8bfb58434883d91a5bc54810f4a3a641ce8f046dc` |

## Solana

The official [Clock/sysvar
reference](https://solana.com/docs/tools/litesvm/typescript/api-reference/time-and-sysvars)
was used only for the Clock field distinction.  The official [RPC commitment
documentation](https://solana.com/docs/rpc) was used for the client-side
meaning of `finalized`.  No claim equates an in-program Clock read with RPC
commitment.

## Fixture derivation

[`fixtures/price-update-v2-full.hex`](fixtures/price-update-v2-full.hex) is not a
mainnet capture and contains no copied Pyth payload.  It is an original exact
schema-derived vector.  Its file SHA-256 is
`9fbb9575b6c9032e95390dea93e1018ee40358e68adacfc7f4e48706bf7430bc`:

| Bytes | Value |
| --- | --- |
| 0..8 | SHA-256(`account:PriceUpdateV2`)[0..8] = `22f123639d7ef4cd` |
| 8..40 | synthetic write authority `0x11` repeated |
| 40 | Borsh enum variant 1 = `VerificationLevel::Full` |
| 41..73 | synthetic feed ID `0x22` repeated |
| 73..81 | price 123,456,789 little-endian |
| 81..89 | confidence 12,345 little-endian |
| 89..93 | exponent -8 little-endian |
| 93..101 | publish time 1,700,000,011 little-endian |
| 101..109 | previous publish time 1,699,999,999 little-endian |
| 109..117 | EMA price 123,450,000 little-endian |
| 117..125 | EMA confidence 20,000 little-endian |
| 125..133 | posted slot 250,000,000 little-endian |
| 133 | zero tail left by full-variant serialization in 134-byte allocated space |

The fixture's purpose is layout/parser reproducibility, not source-value
authenticity.  Production fixtures must be post-cutover receiver-generated
accounts with separately recorded deployment/config provenance.
