# Dragon's Clutch

dClutch is a Solana protocol for prediction markets that are fully backed by
collateral. Pick an outcome — where the SOL price lands on Friday, say — and
buy claims on it. If you are right, each claim pays you one unit of
collateral. If you are wrong, it pays nothing.

Every claim is backed by collateral locked up before the claim exists. So
there is nothing borrowed, nothing to be liquidated, and no way to lose more
than you paid.

## It is running

The seven programs are deployed on Solana's devnet and the site is live at
**[clutch.dregg.pro](https://clutch.dregg.pro)**.

The newest open market is
[`6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4`](https://clutch.dregg.pro/market?address=6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4).
It asks where SOL/USD finishes its window, and it treats the price feed
failing to report as an outcome of its own rather than as a stall. It charges
no fee on either side.

Devnet is a public test network whose tokens are worthless by construction:
this is not an offering, there is nothing for sale, and no value is at risk
anywhere.

## Read a market yourself

A market is a Solana account, and its bytes are the truth — the website and
this page are both just renderings of them. `dclutch` is a small read-only
tool that fetches those bytes over ordinary JSON-RPC and hands them to the
same decoders the on-chain programs use, so you can check a market without
trusting us about it.

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/emberian/dragons-clutch/releases/download/v0.1.0-devnet.2/dclutch-cli-installer.sh | sh
```

macOS (Apple Silicon and Intel) and Linux x86-64. That URL names a version
rather than `latest` on purpose: every release so far is a **prerelease**, and
GitHub's `latest` endpoint skips prereleases, so the `latest` URL returns 404.
Take the current number from the [releases
page](https://github.com/emberian/dragons-clutch/releases).

Two commands do the work, and the honest pre-trade check is both of them: a
market being open is necessary and not sufficient, because execution runs
through a separate capability root.

```
$ dclutch market show 6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4
  phase                     Open
  …
  This market is open: claims can be bought and sold, and it has not been answered yet.

$ dclutch capability show 7kPABbyrKFmqP65FUWDKxNinb2mW7gP3EXGkeEjFWy3N
  family                    Direct
  phase                     Open
  …
  Direct trading is open on this market: new intents are admitted.
```

Each has an offline `decode` twin for bytes you already have. It never signs,
never submits a transaction, and never opens a key file — every subcommand is
a read. To actually trade, use the web app, which signs with your wallet.

## Building from source

The protocol lives in [`dclutch/`](dclutch/) — the programs, the formal work,
the web app, and the tools that drive them. Start at
[`dclutch/README.md`](dclutch/README.md), which has the build and the test
instructions. Everything else in this repository is prior work, kept for
reference.

## License and security

First-party source and documentation are licensed under
[AGPL-3.0-or-later](LICENSE).

Found a vulnerability? Email `security@ember.software`. Testing the deployed
programs is welcome within the ordinary courtesies of a shared public network
— [`dclutch/SECURITY.md`](dclutch/SECURITY.md) says what to include in a
report and what not to point at a public RPC endpoint.
