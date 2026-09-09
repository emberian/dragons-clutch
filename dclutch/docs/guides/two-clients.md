# Command-line clients

This checkout contains two command-line clients:

| | `dclutch` | `dclutch-terminal` |
| --- | --- | --- |
| Language | Rust | TypeScript |
| Source | `tools/dclutch-cli` | `packages/dclutch-cli` |
| Main uses | Inspect accounts, sign offers and build unsigned operation plans | Market workflows using the SDK and native operator tools |
| RPC environment variable | `DCLUTCH_RPC_URL` | `DCLUTCH_RPC` |
| Signing key option | `--keypair-env VAR` | `--keypair <path>` or `DCLUTCH_KEYPAIR` |

Run the selected executable with `--help` to see its commands. Some commands
inspect or prepare an operation; others submit it. Each command’s help names
its required inputs.

## Build and use `dclutch`

The Rust client reads market and capability accounts, signs portable Direct
tickets and builds unsigned General and Fractional retirement plans.

```sh
dclutch market show <ADDRESS>        # a Market Core account, every field
dclutch market decode --file <PATH>  # the same rendering, no network
dclutch capability show <ADDRESS>    # can this market actually execute a trade
dclutch ticket author --keypair-env VAR ...
dclutch ticket verify <PATH>
dclutch ticket post --board URL <PATH>
dclutch general plan --route ABSOLUTE.json --output ABSENT-ABSOLUTE.json
dclutch fractional-retirement-next --route ABSOLUTE.json --output ABSENT-ABSOLUTE.json
```

See the [client README](../../tools/dclutch-cli/README.md) for packaged
installation. To build from this checkout, run these commands at the repository
root:

```sh
cargo build --release -p dclutch-cli
./target/release/dclutch --help
```

## `dclutch-terminal` — the flow driver, built from this checkout

The terminal client uses [the SDK](client-developers.md) and the native
operator binary for market workflows.

```sh
npm install --prefix packages/dclutch-cli
npm run build --prefix packages/dclutch-cli
node packages/dclutch-cli/bin/dclutch-terminal.mjs --help
```

The launcher is `bin/dclutch-terminal.mjs`; `npm link` puts it on your `PATH`
under that name. Its commands:

```sh
dclutch-terminal markets ls
dclutch-terminal markets show <address>
dclutch-terminal portfolio
dclutch-terminal spine --market <address>
dclutch-terminal offer sell
dclutch-terminal intent buy
dclutch-terminal route direct
dclutch-terminal product inspect
dclutch-terminal refusal <code>
```

`found`, `join` and `redeem` provide market workflows. `walk --dry-run`
previews a failure-walk transaction. `offer` and `intent` write signed portable
tickets for another participant to use.

The terminal’s `buy` and `sell` submission commands are currently unavailable.
Use a compatible market’s browser page to take a Direct offer. The
[terminal README](../../packages/dclutch-cli/README.md) lists operation inputs
and examples.
