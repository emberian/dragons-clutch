# @dclutch/cli

`dclutch` is the fail-closed dClutch terminal client. It proves that
`@dclutch/sdk` is a real client surface: every chain fact this tool states
flows through the SDK's generated, byte-gated modules; nothing is restated.

```sh
npm install && npm run build       # bundles to dist/dclutch.mjs
node bin/dclutch.mjs --help
```

## The loop

```sh
# Run a complete private-validator lifecycle from one run spec.
dclutch found --spec run-spec.json --session-out session.json

dclutch --session session.json markets ls
dclutch --session session.json markets show <market>

# which walls stand between this market and a Direct trade, by name
dclutch --session session.json spine --market <market> --keypair me.json

# Produce the chain-authenticated route in two key-free, read-only steps. Every
# local input is an absolute path pinned by the digest in its release report.
dclutch --rpc https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --bootstrap-bin /work/dclutch-local-successor-bootstrap \
  route release-set --plan /work/plan.json --expected-plan-sha256 PLAN_SHA256 \
  --core-checked /work/core.checked --expected-core-checked-sha256 CORE_SHA256 \
  --claims-checked /work/claims.checked --expected-claims-checked-sha256 CLAIMS_SHA256 \
  --trading-checked /work/trading.checked --expected-trading-checked-sha256 TRADING_SHA256 \
  --resolution-checked /work/resolution.checked --expected-resolution-checked-sha256 RESOLUTION_SHA256 \
  --custody-checked /work/custody.checked --expected-custody-checked-sha256 CUSTODY_SHA256 \
  --output /work/checked-execution-release.bin

dclutch --rpc https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --bootstrap-bin /work/dclutch-local-successor-bootstrap \
  route direct --session /work/direct-session.json \
  --checked-execution-release /work/checked-execution-release.bin \
  --expected-checked-execution-release-sha256 CHECKED_EXECUTION_SHA256 \
  --registry-checked /work/registry.checked --expected-registry-checked-sha256 REGISTRY_SHA256 \
  --rent-checked /work/rent.checked --expected-rent-checked-sha256 RENT_SHA256 \
  --output /work/route.json

# The recommended maker path derives fee, generation, collateral destination,
# start slot, and next nonce from authenticated chain observations. Price is an
# exact scaled integer; duration and fill behavior are explicit. This signs
# only the portable ticket and neither builds nor submits a transaction.
dclutch --session session.json offer sell --route /work/route.json \
  --maker <maker-address> --outcome 1 --fill 5 --price 400000 \
  --duration-slots 150 --lifecycle ioc \
  --keypair /keys/maker.json --out /work/sell-ticket.json

# `intent sell|buy` is the low-level automation surface. It additionally
# requires explicit collateral, nonce, valid-from, valid-through, and numeric
# lifecycle 0|1; it emits the same canonical ticket rather than a second DTO.

# buy and sell intentionally refuse before reading a session, route, or key.
# Keep the signed intent as an off-chain handoff; no public submitter exists yet.

dclutch --session session.json portfolio
dclutch --session session.json redeem --market <market> --keypair owner.json \
  --payer <owner-address> --recipient <collateral-token-account> \
  --payout-input payout-input.json --payout-journal payout-operation.json \
  --payout-alt-plan payout-alt-plan.json \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG

# inspect the funded failure-walk packet. Submission is disabled until this
# command has its own durable packet/signature/Submitted/poststate journal.
dclutch --session session.json walk --book walk-book.json \
    --generation 1 --terminal-sequence 1 --keypair anyone.json \
    --dry-run \
    --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG

dclutch refusal 0x5000            # any custom code, named via the band registry
```

## Compile and inspect a spline Product

`product spline` delegates the whole degree-2/3 graph to the Rust production
compiler. It reads no chain or key and atomically writes five immutable record
files plus `report.json` into a new directory. The repository carries one
canonical degree-2 input at
`docs/operator/examples/spline-product-degree2.json`; its identities are an
offline example, not published Registry state or a Market.

Set all three shell variables below to absolute canonical paths. The output
directory must not exist yet.

```sh
SUCCESSOR=/absolute/path/to/dclutch-local-successor-bootstrap
INPUT=/absolute/path/to/docs/operator/examples/spline-product-degree2.json
PRODUCT_GRAPH=/absolute/new/path/spline-product

dclutch --bootstrap-bin "$SUCCESSOR" \
  --input "$INPUT" --output-dir "$PRODUCT_GRAPH" \
  product spline

dclutch --report "$PRODUCT_GRAPH/report.json" product inspect
```

The inspection command rereads the report and its five canonical sibling
files. The SDK checks every file length and SHA-256, all generated schema IDs,
the report's basis/gate joins, and all canonical Registry raw/staging PDAs. It
then prints the exact five record coordinates the Found39 client consumes.
This remains a local handoff: it does not publish a record, read a chain, load
a wallet, sign, submit, or claim that Found will accept absent live Registry
authentication.

A cold-machine smoke runs the public compiler and inspector together and saves
both machine documents plus a hash-bound handoff report in one new directory:

```sh
tools/release/spline-product-handoff-smoke.sh \
  --node /absolute/path/to/node-22.13-or-newer \
  --cli /absolute/path/to/packages/dclutch-cli/dist/dclutch.mjs \
  --successor /absolute/path/to/dclutch-local-successor-bootstrap \
  --work /absolute/new/path/spline-product-smoke
```

## Found a permanent-devnet market

You describe one market, its founding files, and its first participant in one
operation document. The CLI passes that same document through the Rust market
producer, founding campaign, and participant admission command. It does not
recalculate any protocol value.

Save this as an absolute path such as `/work/flagship-operation.json`:

```json
{
  "schema": "dclutch-devnet-market-participant-operation-v1",
  "plan": "/work/checked-devnet-plan.json",
  "market": {
    "kind": "flagship",
    "arguments": [
      "--registry-program-id", "REGISTRY_PROGRAM_ADDRESS",
      "--direct-fee-basis-points", "25",
      "--direct-fee-recipient", "FEE_RECIPIENT_ADDRESS",
      "--price-update", "/work/pyth-price-update.bin",
      "--window-start", "1800000000"
    ],
    "output": "/work/flagship-market.json"
  },
  "campaign": {
    "evidence": "/work/flagship-campaign.json",
    "keypairs": [
      { "role": "core-upgrade-authority", "path": "/keys/core-upgrade-authority.json" },
      { "role": "collateral-mint", "path": "/keys/collateral-mint.json" },
      { "role": "collateral-wallet", "path": "/keys/collateral-wallet.json" },
      { "role": "founding-beneficiary", "path": "/keys/founding-beneficiary.json" },
      { "role": "founding-founder", "path": "/keys/founding-founder.json" },
      { "role": "founding-projection-witness", "path": "/keys/founding-projection-witness.json" },
      { "role": "founding-source-funder", "path": "/keys/founding-source-funder.json" },
      { "role": "substituted-founder", "path": "/keys/substituted-founder.json" }
    ]
  },
  "participant": {
    "output": "/work/first-participant.json",
    "positionOwner": "POSITION_OWNER_ADDRESS",
    "positionOwnerKeypair": "/keys/position-owner.json",
    "feePayer": "FEE_PAYER_ADDRESS",
    "feePayerKeypair": "/keys/fee-payer.json",
    "minimumFinalizedSlot": "123456789",
    "collateral": null
  }
}
```

First prepare the exact market input. This path performs no mutation and does
not read any key file:

```sh
dclutch --rpc https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --bootstrap-bin /work/dclutch-local-successor-bootstrap \
  found --found-operation /work/flagship-operation.json \
  --found-journal /work/flagship-operation-journal.json
```

Review `/work/flagship-market.json`, then rerun the exact same command with
`--execute`. You may also add `--session-out /work/flagship-session.json`.
Execution records its authorization before the Rust campaign can read a key.
If the process stops, rerun with the same operation and journal. The command
reconciles a completed child report and resumes only a checkpoint-authenticated
founding suffix; it never starts the participant step against a different
plan or market.

The journal finishes at `participant-complete`. Its saved digests bind the
operation, successor binary, plan, RPC URL, authored market bytes, campaign
evidence, and participant evidence. The command independently requires the
full Solana devnet genesis hash. Neither this CLI entry nor either Rust child
admits mainnet.

## Join a founded market

`join` is the public participant-admission verb. It authors nothing: the Rust
successor's User Position admission remains the sole author of the admission
message, its rent and fee arithmetic, its signatures, and its durable report.
This command names that child's exact inputs and hands them over.

```sh
# Preflight: finalized read-only planning. No --execute is passed to the child.
dclutch --rpc https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --bootstrap-bin /work/dclutch-local-successor-bootstrap \
  join --plan /work/plan.json --campaign-evidence /work/campaign.json \
  --keypair /keys/participant.json --output /work/admission.json
```

Review `/work/admission.json`, then rerun the exact same command with
`--execute` to perform the admission. The child resumes that same report.

- The origin picks the child: an exact `http://127.0.0.1:PORT/` loopback origin
  runs the owned-loopback admission, which takes no cluster acknowledgment and
  refuses one; every other origin runs the devnet admission and requires
  `--i-mean-devnet` with the full genesis hash. A loopback *host* in any other
  shape (`localhost`, `https://`, no port) is refused as a spelling to fix, not
  offered an acknowledgment.
- `--position-owner` and `--fee-payer` are derived from key files, never typed.
  The CLI reads each named keypair for its PUBLIC key alone and passes the file
  path to the child, which is the process that signs. `--fee-payer-keypair` is
  optional and defaults to the `--keypair` position owner.
- `--minimum-finalized-slot` is read from the endpoint inside a fresh cluster
  admission, so the floor provably comes from the chain the invocation named.
  Pass the flag to state it yourself and the command performs no RPC at all.
- To fund the admitted position after admission, pass all three of
  `--collateral-source-owner-keypair`, `--collateral-source-account`, and
  `--collateral-quantity-atoms`, or none. The source owner's address is derived
  from its keypair. A partial tuple is refused before the child starts.

## Honesty notes

- Refusals render by NAME (band registry, decision 0007) on every error
  path; a refusal is the protocol working, and this tool says which program
  refused and why instead of printing a bare number.
- `route release-set` and `route direct` expose the successor's existing
  key-free, read-only producers; the TypeScript CLI does not reconstruct their
  release, account, lookup-table, or route facts. Both require the full devnet
  genesis acknowledgment, and the Rust producer reauthenticates the endpoint.
  `route release-set` reads the live Registry activation cache and revalidates
  every caller-pinned checked-release file. `route direct` uses the same
  finalized `DirectTradePlanningV1` as the executor and refuses until the
  session's durable journal proves its lookup table frozen. Each child emits a
  machine report for the exact new output; neither command can read a key or
  submit a transaction.
- `--route` is hostile input, not authority. The SDK bounds and scans its
  original UTF-8 before ordinary decoding, refuses duplicate keys at every
  object level, unknown or missing fields, aliases, noncanonical addresses,
  privileges, roles, evidence encodings and digests, then reacquires the whole
  route from finalized chain state. The authoring commands receive no route until the
  existing Direct authenticator recognizes the checked outer deployment
  evidence and exact frozen lookup table. `dclutch route direct` is the public
  producer; a copied, stale, or substituted output still has no authority.
- `offer sell` is the participant-facing authoring path. It authenticates the
  route, seller Claims Position, canonical Direct collateral destination, and
  maker replay root before opening the explicitly named signer file. It derives
  the fee, generation, start slot and next nonce from those observations,
  refuses overselling, and immediately decodes the signed output through the
  same canonical ticket reader used by a taker. It signs no transaction and
  does not require or contact a relay. `intent sell|buy` remains available for
  low-level callers only when every lifecycle, nonce, validity, and collateral
  field is explicit; it has no guessed nonce or validity defaults.
- `buy` and `sell` always refuse before context, session, route, key, signature,
  transaction construction, or RPC access. They remain closed until one public
  caller wires a durable journal for the exact packet, authenticates the
  returned `HotExecutionAckV3`, and finalizes all ten writable-account
  poststates. `spine` remains the read-only market inspection path; `offer` and
  `intent` remain off-chain signed handoffs. Neither submits a transaction.
- `redeem` checks the market, your position, the receiving token account, the
  winning claim, and the full available quantity at finalized commitment. You
  can pass a completed campaign with `--spec <plan> --payout-evidence
  <evidence>`, and the command asks the read-only Rust planner to derive the
  payout input. An already-derived `--payout-input` remains available for
  automation.
- Keep the same `--payout-alt-plan` and `--payout-journal` paths when you rerun
  the command. The CLI accepts an already-finalized payout lookup table. If
  that table still needs creation or extension, it saves the checked plan and
  stops before loading your key; those mutations reopen after each packet has
  its own durable Submitted journal and finalized readback. Replay
  creation uses `<payout-journal>.claims-replay.json`: its exact unsigned packet
  is saved before your key is read, and its exact signed ID is saved before one
  raw send. The command checks the exact finalized Claims transaction, Custody
  receipt, rent movement, and replay account, archives that journal, and asks
  you to rerun before continuing. The payout uses the same ordering in the main
  journal, including its verifier state. If a submitted run is interrupted,
  rerunning only checks that saved transaction at finalized commitment; it
  never signs or sends it again. You may archive an unsigned plan with
  `--discard-unsigned-payout`, but a submitted journal stays until the exact
  receipt and account changes pass.
- The keypair is always an explicit `--keypair <path>` or `$DCLUTCH_KEYPAIR`;
  there is no default-wallet fallback, deliberately.
- Every enabled public-chain mutation (`redeem`, `join --execute`) requires
  `--i-mean-devnet` with Solana devnet's full genesis hash. The endpoint proves
  that exact identity again before every transaction signature and submission;
  an RPC hostname and an earlier observation grant no authority.
- `walk` requires `--dry-run` and never signs or submits. `--dry-run` on
  `redeem` also builds and prints without signing. Neither flag enables `buy`
  or `sell`.

Unpublished (0.x, `private: true`) — same dispensation as the SDK.
