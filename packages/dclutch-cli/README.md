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

# Direct settlement is bilateral: a maker signs an intent, a taker crosses it
dclutch --session session.json intent sell --route route.json \
    --outcome 1 --fill 5 --price 400000 --collateral <acct> \
    --keypair maker.json --out sell-intent.json
dclutch --session session.json buy --route route.json --take sell-intent.json \
    --outcome 1 --fill 5 --price 400000 --collateral <acct> --keypair taker.json

dclutch --session session.json portfolio
dclutch --session session.json redeem --market <market> --keypair owner.json

# the funded failure walk: a passed deadline is free money for whoever watched
dclutch --session session.json walk --book walk-book.json \
    --generation 1 --terminal-sequence 1 --keypair anyone.json

dclutch refusal 0x5000            # any custom code, named via the band registry
```

## Honesty notes

- Refusals render by NAME (band registry, decision 0007) on every error
  path; a refusal is the protocol working, and this tool says which program
  refused and why instead of printing a bare number.
- `redeem` performs the one wallet-constructible step (the Claims-role
  Custody replay) and then states the payout gap in the SDK's own words —
  the payout route for a plain Claims Position admits caller role Core or
  Trading only (ADR-0008 §7.6). It does not pretend a flag would fix that.
- The keypair is always an explicit `--keypair <path>` or `$DCLUTCH_KEYPAIR`;
  there is no default-wallet fallback, deliberately.
- `--dry-run` on `buy`/`sell`/`redeem`/`walk` builds and prints everything
  and signs nothing.

Unpublished (0.x, `private: true`) — same dispensation as the SDK.
