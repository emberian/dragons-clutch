# @dclutch/cli

`dclutch` — the dClutch trader loop from a terminal, and the proof that
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

# Direct settlement is bilateral: a maker signs an intent, a taker crosses it
# route.json must be one dclutch-direct-hot-route-manifest-v3 document carrying
# the exact 39 named fixed rows, runtime rows, sole frozen lookup table, and the
# complete checked-infrastructure bytes plus their lowercase SHA-256.
dclutch --session session.json intent sell --route route.json \
    --outcome 1 --fill 5 --price 400000 --collateral <acct> \
    --keypair maker.json --out sell-intent.json
dclutch --session session.json buy --route route.json --take sell-intent.json \
    --outcome 1 --fill 5 --price 400000 --collateral <acct> --keypair taker.json \
    --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG

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
  route from finalized chain state. `intent`, `buy`, and `sell` receive no route
  until the existing Direct authenticator recognizes the checked outer
  deployment evidence and exact frozen lookup table. No public route producer
  is shipped yet; these commands do not invent or default one.
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
- Every public-chain mutation (`buy`, `sell`, `redeem`, and `walk`) requires
  `--i-mean-devnet` with Solana devnet's full genesis hash. The endpoint proves
  that exact identity again before every intent signature, transaction
  signature, and submission; an RPC hostname and an earlier observation grant
  no authority.
- `--dry-run` on `buy`/`sell`/`redeem`/`walk` builds and prints everything
  and signs nothing.

Unpublished (0.x, `private: true`) — same dispensation as the SDK.
