# Real-provider local bootstrap

This standalone host client turns the ELF-tail execution profile created by
`../dclutch-local-validator` into an initialized, executable Pyth provider
profile. It never reads Solana CLI configuration, wallet files, browser state,
or a public RPC. The caller must name an HTTP RPC URL whose host is a literal
loopback address or `localhost`; redirects and proxies are disabled. The payer,
EncodedVAA, and PriceUpdate keypairs are generated in process memory and are
never serialized.

Run it only against a fresh validator started by the sibling launcher:

```sh
cargo run --manifest-path tools/local-validator/bootstrap/Cargo.toml --locked -- \
  --rpc-url http://127.0.0.1:18890 \
  --evidence /absolute/new/path/provider-bootstrap-evidence.json
```

Add `--reclaim` to execute the real receiver's `ReclaimRent` instruction after
the posted account has been observed and verified. The evidence path must not
already exist. Without `--evidence`, the JSON record is printed to stdout.

Against the separate integrated profile, supply all four artifact-provenance
arguments. In addition to the complete provider sequence, this authenticates
the loaded dClutch ELF and both real SPL Token program accounts, then submits
two real dClutch paths. The first is the small RentCredit Create, fund, and
Withdraw probe. The second creates an authority-disabled legacy SPL Token mint;
compiles the canonical Realm, binary Product, local release-bound
SourceMaterial, and capability manifest; publishes all six immutable records;
executes Found, readiness, and Open; creates the Source state; and resolves the
Market by posting the signed fixture through the real receiver from inside the
dClutch Source instruction. Every transaction and surviving account body is
hashed in the evidence record.

```sh
cargo run --manifest-path tools/local-validator/bootstrap/Cargo.toml --locked -- \
  --rpc-url http://127.0.0.1:19890 \
  --evidence /absolute/new/path/integrated-bootstrap-evidence.json \
  --dclutch-program-id 5oEzAP4izB65uRm2yDAEf9oALGwHpWkDfyKb8zBY3euC \
  --dclutch-elf-sha256 ELF_SHA256 \
  --dclutch-source-commit FULL_GIT_COMMIT \
  --dclutch-source-archive-sha256 ARCHIVE_SHA256
```

For a provider-only campaign, the launcher still requires a third ELF in its
dClutch argument position. The integration test used the committed router ELF
again under the distinct dummy ID `11111111111111111111111111111112`, with
SHA-256 `f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb`.
That entry is only a launcher placeholder: it is not a dClutch program, cannot
execute dClutch resolution, and must not appear in release evidence.

The client verifies all ten fixture hashes before sending anything. It then
inspects the real receiver/router program and ProgramData accounts, verifies
their canonical PDA linkage and exact ELF tails, initializes the pinned
19-guardian router and receiver Config, creates and writes the captured signed
VAA through the router ABI, executes the real 13-of-19 verification, and posts
the exact accumulator message through the real receiver. The resulting
134-byte `PriceUpdateV2` must have the pinned fully-verified price semantics;
only its in-memory write authority and locally observed posted slot may differ
from the captured account body. The integrated Source transaction uses a
second in-memory PriceUpdate signer, checks the terminal Source and resolved
Market postconditions, and requires that temporary update account to have been
reclaimed atomically.

## Provenance and evidence boundary

The provider artifacts come from `pyth-network/pyth-crosschain` commit
`f50a3faf9fc5a223a22889799b2f778900f186b3`, under Apache-2.0, as documented
in `../../../fixtures/pyth/local-upgraded-2026-08-22/PROVENANCE.md`. The VAA is
the committed synthetic 13-of-19 guardian proof, not a live price.

`solana-test-validator 4.0.2 --upgradeable-program` reconstructs fresh loader
headers and uses the current validator clock. It cannot reproduce the captured
historical deployment slots, upgrade-authority option, or complete ProgramData
body hashes. This client records the observed owners, Program/ProgramData
headers, slots, authority, complete-body hashes, and exact ELF-tail hashes. It
accepts the launcher's non-signable authority representation whether the local
CLI serializes it as `None` or `Some(11111111111111111111111111111111)`, and
records the exact observed form rather than conflating their header bytes.
It does not replace those observations with captured headers and never claims
the captured provider release identity. Integrated execution instead selects
the feature-gated `local_validator_release_v1` semantic owner. That row retains
the captured provider, ABI, configuration, quorum, codec, and adapter facts,
but domain-separates the local environment and requires the two chain-observed
deployment slots to be zero.

The fixture publish time also differs from the validator wall clock. For this
bounded local campaign, the compiled immutable Window sets
`max_age_seconds = observed_clock - fixture_publish_time + 300`, with checked
`i64` subtraction and checked conversion/addition to the V1 `u32` domain. The
target instant remains the signed fixture publish time and the runtime's
freshness, future-skew, feed, exponent, confidence, release, loader, and config
checks remain enabled. This is an explicit local market policy, not captured
clock or production-market evidence. Provider-only evidence therefore retains
`dclutch_resolution_executed=false`; successful integrated evidence sets it
true only after the Market, Source state, and reclaimed update postconditions
all hold. Every record retains
`captured_release_identity_claimed=false`.

Current primary-inline Source semantics require the resolver to capitalize the
temporary update and receiver fee. Found still creates the exact canonical
resolution Fund, but this primary route does not debit it; the evidence names
that funded-liveness boundary explicitly rather than claiming Fund-financed
primary resolution.
