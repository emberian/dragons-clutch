# Two clients, and which one you have

This repository ships **two** command-line clients. They read the same protocol
and they are not the same program, and until recently they declared the same
executable name — so whichever came first on your `PATH` answered, and a reader
following either runbook with the other one installed got `unknown command`.
That reads like a documentation error. It was a `PATH` fact.

The names are now distinct, and this page is the answer to "which one am I
looking at, and which one do I want."

| | `dclutch` | `dclutch-terminal` |
| --- | --- | --- |
| written in | Rust | TypeScript |
| lives at | `tools/dclutch-cli` | `packages/dclutch-cli` |
| how you get it | **downloaded** — signed release archives and a shell installer | **built from this checkout** |
| published | GitHub Releases (`v0.1.0-devnet.*`) | nowhere; `private: true`, and `@dclutch/cli` is on no registry |
| what it is for | reading state, and authoring one signed thing at a time | driving whole flows end to end |
| does it submit | never | yes, where a command says so |
| endpoint variable | `DCLUTCH_RPC_URL` | `DCLUTCH_RPC` |
| key on the command line | refused by name; only `--keypair-env VAR` | `--keypair <path>`, or `DCLUTCH_KEYPAIR` |

**If you did not build this repository, you have `dclutch`.** It is the only one
anyone can install, and it is the one the release archives contain.

## `dclutch` — the reader, and the one that ships

It fetches account bytes over ordinary JSON-RPC and hands them to the same
decoders the on-chain programs use, so you can check a market without trusting
our website. It never submits a transaction. Reading takes no credential at all;
authoring a ticket opens one key file named by an environment variable and signs
onto local disk, and that is the only thing here that touches a key.

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

**Install** with the pinned installer line in
[`tools/dclutch-cli/README.md`](../../tools/dclutch-cli/README.md), which names a
version rather than `latest` for a stated reason. **Or build it** from this
checkout — it is its own cargo workspace, so build it from its own directory:

```sh
cargo build --release --manifest-path tools/dclutch-cli/Cargo.toml
./target/release/dclutch --help
```

## `dclutch-terminal` — the flow driver, built from this checkout

Everything it states about the chain comes through
[`@dclutch/sdk`](client-developers.md), whose generated modules are byte-gated
against the protocol's own authorities — which is the point of it: it is the
proof that the SDK is a real client surface, and when you wonder how to wire a
flow up, the command that already does it is about two hundred lines.

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

`found`, `join`, `redeem` and `walk` drive the multi-step flows; each has its own
required arguments, and `--help` names every flag this client accepts. `offer`
and `intent` sign a portable ticket onto disk and submit nothing.

`buy` and `sell` are present and **disabled**: they refuse before they read your
session, your route, or your key. They stay closed until the client can journal
the exact packet before your key is opened, authenticate the chain's
`HotExecutionAckV3`, and reconcile every writable account at finality.

## Which one a runbook means

Every guide in this directory now spells the executable out. If you are reading
an older copy that says the bare `dclutch` for a flow command, it means
`dclutch-terminal`: `markets`, `portfolio`, `offer`, `intent`, `route`,
`product`, `spine`, `redeem`, `found`, `join`, `walk`, `refusal`, `buy`, `sell`.

You do not have to remember that. **Each binary knows the other's verbs and says
so**: type one of those at `dclutch` and it names the program that owns the verb
instead of saying `unknown command`, and `dclutch-terminal` does the same in
reverse. The two lists are kept in step at
`tools/dclutch-cli/src/main.rs` (`TERMINAL_CLIENT_COMMANDS_V1`) and
`packages/dclutch-cli/src/main.ts`.

Every command on this page is replayed as `--help` by the `runbooks` CI tier
([`tools/doc-commands`](../../tools/doc-commands/README.md)), so a flag that gets
renamed out from under this page turns it red rather than stranding you.
