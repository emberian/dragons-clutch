# Claims Fractional local-validator evidence — 2026-09-08

## Verdict

The existing `dclutch-fractional-exterior run` producer now emits finalized
native instruction evidence and completed one accepted local-validator journey:

1. `wrap` moved seven native Claims units into the Fractional reserve and
   minted 70 Token-2022 shards.
2. `token-2022-transfer-to-sleeper` moved 40 shards to an independent holder.
3. `whole-unwrap-actor-remainder` burned the actor's remaining 30 shards and
   released three native Claims units, leaving the sleeper's 40-shard liability
   backed by four native reserve claims.

All three transactions finalized successfully. The checked
[`native-evidence.json`](native-evidence.json) contains their signatures,
slots, fees, compute units, runtime logs, full finalized instruction lists and
poststates. For each routed action the producer also decoded the validator's
inner instruction, resolved its loaded-address program index, required exactly
one Claims CPI whose bytes equal the request forwarded by the caller, and
required the finalized packet to be byte-identical to the signed submitted
packet. This is local-validator evidence. It is not devnet, mainnet, or a
final-source cohort result.

## Source and artifact boundary

The evidence-aware host and the freshly rebuilt test caller came from exact
commit `a84f5a1972feb18d8ced7085f3cb21506d6f3950`. The runtime protocol ELFs came
from the retained checked candidate under
`/home/hbox/dclutch-strict-551ffc1b-ensemble-20260908-retry/candidate`, whose
summary identifies source revision
`551ffc1b99abdf01478e7e963a1ac20a99e6c0f6` and source digest
`63c3915387c7a36458d3aa7d18a42fd2e7237c315f92a6154ec42861a0d7e843`.

### 2026-09-08 gate-name addendum

The candidate directory's `gate.sha256` pins `RELEASE_GATE.json` at
`3317fd0777c60a57afd23ff69f325893770af5c1aabecf1e9aae53113c1ead81`.
Its checked build execution is separately pinned by
`CHECKED_UPGRADE_GATE.json` at
`8c6357f8e2fa257d574fde4913cd67ca54b873584360b1b6d2f630f49760f621`;
the latter adds the build run identity and manifest. Both gate documents name
the same source revision, source-tree manifest and eight ELF artifacts.

Because the host/caller source is newer than the runtime source, this run is an
explicit source-split diagnostic.

The native dispatch sources used by this fold are byte-identical between the
runtime and host revisions: `programs/dclutch-claims-sbf/src/lib.rs` hashes to
`fcdd28273595abac095de0dcdc19bf5d441fabbddb07f44a52be845021df76b2`,
`programs/dclutch-claims-sbf/src/fractional_atomic_v3.rs` to
`827fe09aa3a28cb1610d634e3a7680f951531c5e53088c5b49920fdfbd05f1ac`,
and `crates/dclutch-claims/src/fractional/request_v2.rs` to
`0c6f78d588fd576c85453972bc0ac89bc9454698ec73ccc8b4c7ae18e10117ec`
in both source archives. This check binds the source-split census selector to
the runtime's route implementation; it does not turn the run into a
final-source cohort result.

The build and run used `swarm-build`, distinct checkout-owned Cargo targets,
Solana test validator 4.0.2, and an ext4 validator ledger at
`/home/hbox/dclutch-claims-validator-a84f5a197-20260908/evidence/ledger`.
No wallet file or public RPC was read.

| Input | SHA-256 |
| --- | --- |
| Claims ELF | `b18309487b59dd39745e7937fe59e6e49eb6a983b1a0f4ea6c2f2ed056039ef2` |
| Registry ELF | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| Core ELF | `0df8ef6371848b87573f3c8931a2797feae45b7e1792afd1a208dcc8b847e3f9` |
| Custody ELF | `b936b48790ac89ff65d36b28562674fc95f4ba4e6e0f8a763122a6c4baed6422` |
| Fractional test-caller ELF | `76d26ce2d2e28ae4cc4d7bcc7a2237f6b75f828036f0dfad7702c8ff9c54a62c` |
| Token-2022 ELF | `e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697` |
| Evidence host executable | `20884545c058a2f51fa57bb7ea4f1f488c34f6d95fcaad4da7c2248015baaca5` |

## Accepted transactions and poststates

The Claims address was
`Bswb3UyeD1pUTaGiE6WvqwFpJZsQSEY1xhJePCDTHdvp`. Both Claims calls carried
the native selector `4443465245513032` (`DCFREQ02`) in the exact CPI bytes
stored in [`native-evidence.json`](native-evidence.json).

| Action | Signature | Slot | CU | Wire bytes | Poststate: supply / actor shards / sleeper shards / actor native / reserve native |
| --- | --- | ---: | ---: | ---: | --- |
| Wrap | `3RtSzWNB3oRfyGL6urbePHYRsmmBBzJh7QevDiAWHARindRWtMyjsCPXruSQT1tbkAvbxTZbxyNRULbgx8YstEsj` | 132 | 139,527 | 820 | `70 / 70 / 0 / 993 / 7` |
| Token-2022 transfer | `fRTDZkv57TMg4FsB2eVCNDwiKw122MQfeCg1w5Q66PWD9DzNwiTF6SAqHPccN4Hkh3RiGUA9NFSvAoLgqvP8uMY` | 164 | 2,457 | 358 | `70 / 30 / 40 / 993 / 7` |
| WholeUnwrap | `59KbAa5rSzewCS5KsFZD4RNvyhg5X7GAcJUWWnNwSUEeB8wCPjZyPX75YbVne1h1tKJkYBkdSchCxCe2tPLGxKYN` | 197 | 140,471 | 820 | `40 / 0 / 40 / 996 / 4` |

The canonical lifecycle journal digest is
`5227dfd9acac0be811896cf19bc549477a0e1734e87734d934fc3fc553a77e4c`.
The finalized native evidence digest is
`aa2d8d7c37296d8b3fc9b9c02244edb4c3d83c1a9ceca4bd3e6e0ea75a1f1955`.
The host's offline verifier accepted both documents after the validator exited.

## Census result and remaining boundary

The exact-source inventory at host commit `a84f5a197` contained 164 routes,
456 protocol-visible refusal codes, and no unclassified dispatch positions.
[`census-observe.log`](census-observe.log) records two admitted
finalized-instruction observations. [`ledger.json`](ledger.json) credits both
accepted Claims transactions to `claims/fractional_atomic_v3::process`; its
digest is
`34d071ee22fc90593b0c1762da8552f669a379e34fe29a78cbe0a52beb8bdfb3`.
The middle Token-2022 transaction has an explicit zero-route binding, so its
poststate is retained without manufacturing first-party coverage.

The WholeUnwrap branch did execute and its exact action bytes and poststate are
retained here, but the inventory describes
`claims/process_open#WholeUnwrap` with two `variant` selectors, Wrap and
WholeUnwrap, without an action-byte offset. The observer therefore cannot
corroborate that child route from native bytes and this evidence does not mark
it witnessed. The same structural limit applies to
`claims/process_terminal#TerminalZeroBurn`; this journey never entered a
terminal market in any event. `claims/market_closure_v1::process` requires a
Retiring market and also was not submitted here. Those three routes remain
explicitly uncredited rather than receiving an accepted status inferred from
the parent call.

## Stored artifacts

| Artifact | SHA-256 |
| --- | --- |
| [`bindings.json`](bindings.json) | `6babfe05c24a1440afd36029a6f1468aeeb22d17bfae4c0464776b75f44f7b94` |
| [`canonical.json`](canonical.json) | `5227dfd9acac0be811896cf19bc549477a0e1734e87734d934fc3fc553a77e4c` |
| [`census-observe.log`](census-observe.log) | `89f4ecb10a57192e0be0177188a34dffa11fced027a07184066b61f6b80da870` |
| [`ledger.json`](ledger.json) | `34d071ee22fc90593b0c1762da8552f669a379e34fe29a78cbe0a52beb8bdfb3` |
| [`manifest.json`](manifest.json) | `e87b3febd4ebbd3d44e948e08e572026a2fbc07018e5c80f2afbef07888c9607` |
| [`native-evidence.json`](native-evidence.json) | `aa2d8d7c37296d8b3fc9b9c02244edb4c3d83c1a9ceca4bd3e6e0ea75a1f1955` |
| [`programs.json`](programs.json) | `ca4b63a0ad4e78d1e0387ecf960b3d5d2dde1ac4c3d44d86bcd1aabbd54d1329` |
| [`run.log`](run.log) | `e4f7cb6370f966637433a985c4bba68e65cc6c7f900b67d3bc78e73db77e39f2` |
