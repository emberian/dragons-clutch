# Dealer accepted local-validator campaign — 2026-09-08

This records a completed owned local-validator campaign. It is neither a devnet result nor a new checked release claim. The runtime is the preserved strict 7d checkpoint; the host is a separately committed Journey source.

## Provenance

| Fact | Value |
| --- | --- |
| Runtime gate | `/tank/dregg-build/dclutch-strict-7d5f920a4-ext4sealed-20260908/candidate/CHECKED_UPGRADE_GATE.json` |
| Gate SHA-256 | `c67bc5ea4a833a4c38208df47bea05276485776303b05b2ff5695c5f7dc8b175` |
| Runtime source / tree | `7d5f920a4592899b554c2af4cd05e2c1a551053b` / `f1cd685b3b99a2fc3bb0cec5dfdb600ee6b001de63f6563765743458544a4da0` |
| Host source / tree | `c04d95a0c898b125e67ef0cbbf77a98c47aa4e36` / `b1319e4bf1109aee156ac001acb748ed435f576e` |
| Host archive SHA-256 | `6f06240a0d16b5460c2fa44640e3f2e43b35438f25e6f1927f325a141a9a210c` (`source-c04.bundle`) |
| Host binary | `/tank/dregg-build/dclutch-dealer-c04d95a0c-20260908/source-exact/target/release/dclutch-journey-campaign` |
| Host binary SHA-256 | `de4cd329c8a071c1cccdce4d71451fdaa2bb3a728b6c8b24719366b2ab2ac831` |
| Campaign directory | `/tank/dregg-build/dclutch-dealer-c04d95a0c-campaign-20260908` |

The host was exported from the exact committed tree, built on hbox through `swarm-build` with the locked dependency graph, and then launched by the existing fresh Dealer launcher against the preserved sealed gate. The focused native check was `cargo check --locked --offline -p dclutch-journey-campaign`; it completed before the release build.

The lock closure repair is host commit `c04d95a0c` (`journey: remove stale relayed bundle lock edge`). It removes the one stale lock-only edge that made the otherwise clean `ad49` host reject `--locked`; it does not take the in-progress bundle-builder API changes into the campaign.

## Runtime artifact set

| ELF | SHA-256 |
| --- | --- |
| accelerator | `8f19d86e1d47014a99654aa68a2ba03336da9879168a91cd53f5f52241029a44` |
| claims | `12abf6b1a7e276e2181697b58cccd47051550a6ab3e36d11f00c090f462e042b` |
| core | `b7afb74d26471978d22c08e4e77bccfa0939f2e7280b6197118c864b456448ac` |
| custody | `b936b48790ac89ff65d36b28562674fc95f4ba4e6e0f8a763122a6c4baed6422` |
| registry | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| rent | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| resolution | `346cf7bc9c00ab1854a38ee4826b84a40c1be163d51a362756a58519e315f550` |
| trading | `dd9a473e90ec1d02c3186f1cfe91e9711ba870ffff82956d76f25266a568b594` |

## Accepted Dealer execution

The transcript marks these stages executed: checked substrate; compile and found through Open; admit and fund taker; `dealer-found`; `dealer-quote`; `dealer-fill`; nonzero-fill poststates; and `dealer-withdraw`.

| Stage | Signature | Slot | Fee | Compute units |
| --- | --- | ---: | ---: | ---: |
| Found | `4DPottw5ENC3ezTjL6BKV2BRdpMkqjfqmX5TsakDDrPo9xTv6cDgUhWB88gE5abRb5dX8WXAVJPPQW8i8SqHPsbN` | 8,214 | 75,000 lamports | 465,738 |
| Quote | `3Xc6iDeP6otxQgoLCcizYoq2dhSX6XisJnWtbpVvTtmceKUwzUY3jHRBZ1Cau5xsvSfwJzg7KrxzH9Nn2jGiY4bc` | 8,246 | 75,000 lamports | 41,567 |
| Fill | `5juD5dkRCVfD3qKbwaWyE4W3dQ2p7BtXjoYmWzRY8arvmWBfKTTyXZqXCUGn39jd9EWDHmopbSejYzwWF2cNdYxh` | 8,408 | 75,000 lamports | 393,855 |
| Withdraw | `2xtEfxu5UfHGRXiaEVsbAK3a9rgWvLeMEc5ykfZ7gVGr81FnKVsCgEfQ7svRS3J9pX4CHzJw7J1MUdRUf6jo6xzy` | 8,443 | 75,000 lamports | 149,480 |

The four terminal Dealer transactions charged 300,000 lamports total. The campaign also executed its named substituted-refund-wallet refusal control; it is a refusal/rollback check, not a credited refund in this accepted Dealer balance path.

## Read-back and conservation

The historical weakness was a zero-unit fill. This run refutes that weakness: the accepted Fill minted 9 claim units, or 27 atoms at 3 atoms per claim unit, and its later Withdraw transferred 1 claim unit, or 3 atoms.

| Fact | Before Fill | After Fill | After Withdraw |
| --- | ---: | ---: | ---: |
| Hoard atoms | 500,000,001 | 500,000,028 | — |
| Inventory | `[0, 0, 0, 0]` | `[0, 9, 9, 0]` | — |
| Supply | `[166,666,667, 166,666,667, 166,666,667, 166,666,667]` | `[166,666,676, 166,666,676, 166,666,676, 166,666,667]` | — |
| Taker claims | `[0, 0, 0, 0]` | `[9, 0, 0, 0]` | — |
| Taker token atoms | 1,000,000 | 999,991 | — |
| Vault atoms | 3,000,000 | 2,999,982 | — |
| Fund cash atoms | 3,000,000 | 2,999,982 | 2,999,979 |
| Fund revision | 0 | 1 | 2 |

The poststates close the relevant arithmetic:

- `9 × 3 = 27` minted atoms.
- The supply rises by 9 in each of the first three outcomes, or 27 total.
- Taker token balance falls by 9 atoms; the fill records 9 atoms on the taker-to-hoard leg and 18 on the fund-to-hoard leg, totaling 27.
- Fund cash changes `3,000,000 − 18 = 2,999,982` on Fill, then `2,999,982 − 3 = 2,999,979` on the one-unit Withdraw.

## Durable reports

| Artifact | SHA-256 |
| --- | --- |
| `transcript.json` | `c0e913c3473dcc57f49d1b11bde6c8651977ddbc7a185165e8e3e26141352251` |
| `campaign/evidence.json` | `076edf40d6aee974702c4901ca88ca98805f7d2509905d30ad7af56e6a76c1d9` |
| `campaign/dealer-found.json` | `c19a95bf430cb9c682f5d717cd3fce6e9515c1b2e7f3c1df39e0fbf6b68166c6` |
| `campaign/dealer-quote.json` | `7926eb90a6c8ac22d390f4a323520f3c812d93287896c726057f769b8567d57e` |
| `campaign/dealer-fill.json` | `0e0622a7dfb663176b0fad5600e85d1f4dd92c73b005030b2e70e64d69f17898` |
| `campaign/dealer-withdraw.json` | `1c4f932c3ca017ef9bf01d08595b3a252a790d67f564f6a1fec3fc794f633a18` |

The evidence is bounded to this local-validator campaign and its source-split artifact set. It does not claim fresh runtime convergence, a final frame pair, selected-Series runtime coverage, devnet execution, or a deployed cohort.

## Dated addendum — 2026-09-08 provenance names

The runtime provenance row above used the gate's `source_tree_sha256` value.
That is a SHA-256 digest of the gate's source-tree manifest, not a Git tree
object. The two exact runtime identifiers are:

| Fact | Value |
| --- | --- |
| Runtime Git commit | `7d5f920a4592899b554c2af4cd05e2c1a551053b` |
| Runtime Git tree object | `5b16f11e8cda3a6e9247a7540d80fbeb24148d06` |
| Gate source-tree manifest SHA-256 | `f1cd685b3b99a2fc3bb0cec5dfdb600ee6b001de63f6563765743458544a4da0` |

The sealed gate and its ELF hashes remain unchanged. This addendum corrects
only the identifier label; it makes no new runtime, devnet, or release claim.
