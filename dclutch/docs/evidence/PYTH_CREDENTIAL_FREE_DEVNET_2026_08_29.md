# Credential-free Pyth devnet path — 2026-08-29

## Verdict

The $500/month screen is a Pyth API/Pro subscription screen. It is not a fee
for reading every Pyth price on Solana. After the 2026-08-26 Pyth Core cutover,
however, **Hermes itself does require an API key**. Both documented Hermes URLs
returned HTTP 401 to one bounded unauthenticated SOL/USD request on 2026-08-29.

Dragon's Clutch has a different path already implemented in source: read Pyth's
sponsored SOL/USD account directly on Solana devnet, seal an immutable
candidate under Resolution, then settle from the canonical best valid submitted
candidate. That path uses no Pyth bearer, no Hermes request and no caller-owned
Pyth update account. It still needs ordinary devnet RPC access and a devnet-SOL
transaction payer. Candidate/head rent is sponsor-funded and returned to the
beneficiary stored when each account was created; it is not protocol principal.

This is **not yet an end-to-end live-devnet route**. The protocol, real-SBF
lifecycle and durable exterior exist, but the market producer cannot yet emit
or publish the sponsored Source graph. The permanent Resolution program also
has to be upgraded to the final admitted ELF, and a frozen routing table and
public SDK surface remain to be produced. Do not describe the current devnet
flagship as sponsored-push capable until those facts exist on chain.

This audit is key-free and did not read an RPC, sign, submit, deploy, fund or
push. It audits source revision
`eda6a218b4cbfc427bcb96acf2525635d8fcf48d`, tree
`c0802e81246782cc320104fd2a3598316ea493c2`.

## The products are different

| Surface | Current credential rule | Role in Dragon's Clutch |
| --- | --- | --- |
| Pyth Pro | Subscription product with authenticated API access | Not required by the devnet demo |
| Hermes pull | API key required after the 2026-08-26 cutover | Supported only as authenticated online input or an exact offline response |
| Sponsored Solana push account | No Pyth credential is required to read the onchain account | Selected credential-free devnet transport |
| Self-run Pyth price pusher | Permissionless onchain submission, but the pusher itself consumes Hermes and needs a Solana payer | Not required and not credential-free after the Hermes cutover |

Primary official sources, read 2026-08-29:

- Pyth's [upgrade preparation guide](https://docs.pyth.network/price-feeds/core/upgrade/preparing)
  says every Hermes caller needs an API key after the cutover and shows the
  authenticated `pyth.dourolabs.app/hermes` request.
- The [Hermes instance page](https://docs.pyth.network/price-feeds/core/api-instances-and-providers/hermes)
  still labels `hermes.pyth.network` a public test endpoint, but its page banner
  says Hermes now requires an API key. The two unauthenticated probes below
  resolve that documentation tension for the current date: both are 401.
- Pyth describes [Pyth Pro](https://docs.pyth.network/price-feeds/pro) as a
  subscriber-configured enterprise API product.
- Pyth's [Solana push-feed list](https://docs.pyth.network/price-feeds/core/push-feeds/solana)
  says the listed feeds are sponsored on mainnet and devnet, lists SOL/USD as
  `7UVimf...jLiE` / `ef0d8b...b56d`, and names the one-minute heartbeat and
  0.5% deviation policy.
- Pyth's [price-pusher source](https://github.com/pyth-network/pyth-crosschain/blob/465e8dcb5592c57b4909a6cb933d58d6d6b50a43/apps/price_pusher/README.md)
  says updates are permissionless, distinguishes on-demand and regular push,
  and shows that the pusher consumes Hermes and a chain payer. The pinned Pyth
  repository commit is `465e8dcb5592c57b4909a6cb933d58d6d6b50a43`.

The bounded, body-discarding probe was:

```sh
feed_id=ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d
curl --silent --show-error --location --max-time 15 --output /dev/null \
  --write-out '%{http_code}\n' \
  "https://hermes.pyth.network/v2/updates/price/latest?ids%5B%5D=${feed_id}&encoding=base64&parsed=false"
# 401
curl --silent --show-error --location --max-time 15 --output /dev/null \
  --write-out '%{http_code}\n' \
  "https://pyth.dourolabs.app/hermes/v2/updates/price/latest?ids%5B%5D=${feed_id}&encoding=base64&parsed=false"
# 401
```

No token file, environment credential or response body was read.

## Existing credential-free path

The route is selected by immutable Source data, not by a Realm name and not by
DREGG:

```text
SourceSpecV1(PythSponsoredPushSnapshot)
  -> ProviderReleaseV1
  -> PythSponsoredPushReleaseV1
  -> fixed devnet Receiver + Push Program + SOL/USD PDA
  -> Capture(current 134-byte PriceUpdateV2 + Clock)
  -> immutable candidate + canonical head
  -> Settle(best candidate after admission closes)
  -> Source terminal state + certificate + sponsored receipt
  -> permissionless candidate/head close to stored rent beneficiaries
```

The exact compiled devnet release binds:

- price account `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE`;
- feed ID `ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d`;
- shard `0`, bump `252`, exact 134-byte Full `PriceUpdateV2`;
- Receiver `rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ`, ProgramData
  `96QrNCjmh32H9quY9DX4NEH81nECVsbkATBDZeoVbvLV`, deployment slot
  `487855452`, exact current authority and admitted ELF digest;
- push Program `pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT`, ProgramData
  `8xAeURaAWExxyHUXJSgjsg5r96Ydr3G4cek2if7imQmz`, deployment slot
  `293898740`, exact current authority and admitted ELF digest;
- Receiver Config `DaWUKXCyXsnzcvLUyeJRWou8KTn7XtadgTsdhJ6RHS7b` and the SHA-256
  of its exact body.

These are the semantic owner's compiled release facts, captured finalized at
slots 489486600..489486972 on 2026-08-28. A deployment slot, authority,
ProgramData, Config or PDA substitution refuses. The transaction path uses the
same decision-0012 trust split as the dClutch programs: the complete ELF is
measured when the release is admitted; use reauthenticates Program ->
ProgramData, exact slot and exact authority without rehashing megabytes of ELF
inside one transaction.

The relevant current source objects are:

| Owner | File SHA-256 |
| --- | --- |
| `crates/dclutch-pyth-svm/src/sponsored_push.rs` | `c24eec5db6c005f271e7f50e09f7ef2a6a9db8072230bd9da4b595eda53938b3` |
| `crates/dclutch-resolution-codec/src/sponsored_push_v1.rs` | `92ec014cdf08c287cad6232036874d665fa8f9182bfe1d16e12d6323d488d417` |
| `programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs` | `d4e7656ae6ca1aa61a351028c94313230cfcb440a098ba8ae2895ef81700300f` |
| `tools/local-validator/bootstrap/successor/src/sponsored_push.rs` | `643ffbe6284e40b9f596db5d7295b1382c333eeab8a6d53c39abc0af33db8a81` |
| `crates/dclutch-svm-harness/tests/sponsored_push_lifecycle.rs` | `364e2292c80470fa03eb552f72237f84df74595c2a753fe0a9b8fe4e7cfa068d` |
| `tools/pyth-sponsored-push-audit/matrix.json` | `1865ca2ccc06e2b9fe4f84b299724b3c35a2d7cdb2ebd6d241f435723b3d5271` |

## Exact admission and time rules

Capture is permissionless but not unauthenticated. The Resolution instruction
authenticates the active Market and Resolution release; every finalized Registry
record and its schema/digest; the distinct sponsored Source profile; both
provider programs and ProgramData pins; Receiver Config; the fixed price PDA;
rent exemption; a non-executable, read-only, Full update body; write authority;
feed; exponent; confidence; publication ordering; `posted_slot > 0`; and
`posted_slot <= Clock.slot`.

The existing provider-neutral Source normalizer is the only owner of price
semantics. It separately requires:

```text
window.start <= publish_time <= window.end
Clock.unix_timestamp - max_age <= publish_time
publish_time <= Clock.unix_timestamp + max_future_skew
```

No sponsored adapter may widen or duplicate those rules. Capture is admitted
through `Clock.unix_timestamp <= window.end + max_age`. At the exact boundary,
capture is still possible and settlement/failure still refuses. Settlement or
vacant-head funded failure begins only when
`Clock.unix_timestamp > window.end + max_age`.

The mutable upstream account is never terminal evidence. Capture seals its
entire 134-byte body into a candidate PDA derived from Market, generation,
sponsored release, price account, publish time, posted slot and update digest.
The head orders accepted candidates by
`(publish_time, posted_slot, update_digest)`. Settlement reauthenticates the
head, candidate, body digest, PDA, capture Clock facts and Source graph, and
then commits one terminal Source state, certificate and receipt atomically.
Replay against a terminal Source refuses.

Candidate rent returns only to that candidate's stored capture sponsor. Head
rent returns only to the first head sponsor. Cleanup is available only after
the exact Source is Resolved, FailureCommitted or Retired. A substituted
beneficiary, generation, Market, Source or PDA refuses before close.

## Existing caller and recovery

The exterior is already separated from Hermes:

```sh
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml -- \
  devnet-sponsored-push-v1 \
  --rpc-url "$DEVNET_RPC_URL" \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --input /absolute/path/sponsored-market.json \
  --output /absolute/new/path/capture-report.json \
  --action capture \
  --signer SPONSOR_PUBKEY
```

That first invocation is read-only and key-free. It observes one finalized
snapshot, derives every address from chain state, authenticates the frozen ALT,
builds the exact v0 instruction, and creates a `planned` report. To submit after
review, rerun the identical argv/output path with:

```text
--execute --signer-keypair /absolute/path/devnet-sponsor.json
```

No `PYTH_API_KEY`, Hermes URL, Hermes response, Receiver update keypair or DREGG
identifier belongs in this command. Required execution inputs are only the
authenticated devnet RPC, a transaction payer with devnet SOL, the founded
Market's exact routing document and a frozen active ALT.

The report advances `planned -> prepared -> submitted -> finalized` under one
create-new filesystem lease. `prepared` durably contains the exact signed v0
packet, signature, digest and expiry before first send. A crash before recording
`submitted` may resend only those identical bytes; Solana deduplicates the
signature. `submitted` is poll-only and cannot sign or send another packet. An
expired packet is retained as evidence and requires a new report path. The
final report binds transaction signature, finalized slot, fee, compute, logs,
and exact writable pre/post account evidence.

Actions are `capture`, `settle`, `commit-failure`, `close-candidate`, and
`close-head`. `settle` derives the candidate from the canonical head.
`close-candidate` additionally takes `--candidate`. The 32-meta Settle message
is 1,277 bytes as a legacy packet, so the frozen ALT is required; the caller
refuses a mutable, inactive, substituted or changed table and any routed packet
over 1,232 bytes.

The older pull route remains useful only as a separate fallback:

```text
devnet-pyth-vaa-provision-v1
  (--hermes-url HTTPS_BASE --hermes-api-token-file ABSOLUTE_FILE
   | --hermes-response ABSOLUTE_JSON)
```

It produces a caller-owned Receiver update for
`PythTerminalOneTransaction`. It must not accept the sponsored account or
reinterpret `PythSponsoredPushSnapshot`; the two releases, account frames,
receipts and recovery journals are intentionally distinct.

## The missing producer bridge

Three static facts prevent the existing pieces from being called an operating
devnet feature:

1. `devnet-market` still hard-codes
   `SourceAccessProfile::PythTerminalOneTransaction` and consumes a caller
   `--price-update` body. It cannot emit a sponsored Source graph.
2. `MarketRunInput` has no sponsored-release body field, and
   `publish_market_records` recognizes adapter schemas only for terminal Pyth
   and relayed sources. It explicitly refuses `PythSponsoredPushSnapshot`.
   Hand-writing `sponsored-market.json` cannot repair that publication gap.
3. The Rust successor owns the sponsored codecs and instruction builder, but
   `packages/dclutch-sdk` exports no candidate/head/receipt decoder or public
   sponsored instruction builder.

The smallest coherent producer change is therefore not a standalone script.
That would create a second Market DTO truth. It is one change at the existing
market semantic owner:

1. Add an optional exact sponsored-release body to `MarketRunInput`, required
   iff `SourceSpecV1.access_profile == PythSponsoredPushSnapshot` and forbidden
   otherwise.
2. Add `devnet-sponsored-market`. In read-only mode it authenticates the
   compiled `devnet_sponsored_sol_usd_release_v1`, reads the fixed account once
   at finalized commitment, parses Full `PriceUpdateV2`, and derives the normal
   Pyth adapter config, Product, window and statistic without caller-authored
   feed/exponent/provider identities.
3. Construct `ProviderReleaseV1` from the sponsored release's family, adapter,
   release digest, codec and transport; select
   `PythSponsoredPushSnapshot`; publish the sponsored release under
   `PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1`; and retain its raw/staging pair in
   the canonical Market publication evidence.
4. After Open and Resolution funding activation, emit
   `dclutch-sponsored-push-exterior-input-v1` from the campaign's authenticated
   record closure and active funding ledger. Never make the operator copy 30+
   addresses from logs.
5. Build, freeze and finalized-observe the per-Market ALT, then run separate
   key-free preflights for Capture and the post-deadline Settle/Failure suffix.
6. Export the existing codec/PDA/action vocabulary from the public TypeScript
   SDK. SDK construction must start from the Market's Source/Provider records;
   no feed, Realm or DREGG special case may select this path.

The implementation surface is
`tools/local-validator/bootstrap/successor/src/{model,main,market}.rs`, a new
focused producer module, the campaign evidence schema, and
`packages/dclutch-sdk`. It must land with adversarial profile/body/schema
cross-substitution tests and a real-SBF lifecycle from the producer's exact
output. It should ride the same checked Resolution Upgrade as the other pending
Resolution changes.

## Evidence and deferred convergence

The existing offline safety model passed 12/12 adversarial tests at this source:

```text
tools/pyth-sponsored-push-audit/check.sh
.............
Ran 12 tests ... OK
sponsored push safety matrix: exact; hostile model accepted body
50b28fb9d3a8cc1542f3ad4bb679e713b80d9e72eede1bc5f24bc0bdde5878f8
```

Focused host checks were also green at the audited revision:

```text
cargo test --locked -p dclutch-pyth-svm sponsored
2 passed; 0 failed

cargo test --locked -p dclutch-resolution-codec sponsored
5 passed; 0 failed

cargo test --locked \
  --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml \
  sponsored
8 passed; 0 failed
```

The caller packet test compiled the routed 30-meta Capture at 348 bytes, the
32-meta Settle at 352 bytes, the 29-meta failure at 346 bytes, and each 4-meta
cleanup at 296 bytes. These are fixture packet widths under the test's frozen
ALT, not a promise about a future Market's table and not CU measurements.

Current real-SBF coverage exercises two captures, best-head advancement,
deadline boundaries, settle, head-nonvacant failure refusal, funded failure,
replay refusal and both cleanup types. Those runs are development evidence, not
M-61 acceptance for the next ELF. The convergence gate still owes a fresh
all-link build, frame diagnostics, both-sponsor exterior runs, crash/restart at
the durable packet boundaries, exact rent/fee reconciliation, and 20-seed
pass-count plus mean CU per action on the final Resolution artifact.
