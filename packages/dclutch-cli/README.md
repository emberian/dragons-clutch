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

# Direct settlement is bilateral. You can authenticate the route and sign an
# off-chain intent today; this neither builds nor submits a trade transaction.
# route.json must be one dclutch-direct-hot-route-manifest-v3 document carrying
# the exact 39 named fixed rows, runtime rows, sole frozen lookup table, and the
# complete checked-infrastructure bytes plus their lowercase SHA-256.
dclutch --session session.json intent sell --route route.json \
    --outcome 1 --fill 5 --price 400000 --collateral <acct> \
    --keypair maker.json --out sell-intent.json

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

## Honesty notes

- Refusals render by NAME (band registry, decision 0007) on every error
  path; a refusal is the protocol working, and this tool says which program
  refused and why instead of printing a bare number.
- `--route` is hostile input, not authority. The SDK bounds and scans its
  original UTF-8 before ordinary decoding, refuses duplicate keys at every
  object level, unknown or missing fields, aliases, noncanonical addresses,
  privileges, roles, evidence encodings and digests, then reacquires the whole
  route from finalized chain state. `intent` receives no route until the
  existing Direct authenticator recognizes the checked outer deployment
  evidence and exact frozen lookup table. No public route producer is shipped
  yet; the command does not invent or default one.
- `buy` and `sell` always refuse before context, session, route, key, signature,
  transaction construction, or RPC access. They remain closed until one public
  caller wires a durable journal for the exact packet, authenticates the
  returned `HotExecutionAckV3`, and finalizes all ten writable-account
  poststates. `spine` remains the read-only market inspection path; `intent`
  remains an off-chain signed handoff. Neither submits a transaction.
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
- Every enabled public-chain mutation (`redeem`) requires
  `--i-mean-devnet` with Solana devnet's full genesis hash. The endpoint proves
  that exact identity again before every transaction signature and submission;
  an RPC hostname and an earlier observation grant no authority.
- `walk` requires `--dry-run` and never signs or submits. `--dry-run` on
  `redeem` also builds and prints without signing. Neither flag enables `buy`
  or `sell`.

Unpublished (0.x, `private: true`) — same dispensation as the SDK.
