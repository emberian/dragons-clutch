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
# found a market via the run-spec producer (the founding client of record),
# leaving a session file with the rpc url, program ids, and market addresses
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

# the funded failure walk: a passed deadline is free money for whoever watched
dclutch --session session.json walk --book walk-book.json \
    --generation 1 --terminal-sequence 1 --keypair anyone.json \
    --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG

dclutch refusal 0x5000            # any custom code, named via the band registry
```

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
  the command. Replay and lookup-table setup finish in separate runs. Replay
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
- Every enabled public-chain mutation (`redeem` and `walk`) requires
  `--i-mean-devnet` with Solana devnet's full genesis hash. The endpoint proves
  that exact identity again before every transaction signature and submission;
  an RPC hostname and an earlier observation grant no authority.
- `--dry-run` on `redeem` and `walk` builds and prints without signing. It does
  not enable `buy` or `sell`.

Unpublished (0.x, `private: true`) — same dispensation as the SDK.
