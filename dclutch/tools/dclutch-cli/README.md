# `dclutch` — read a dClutch market off the chain

A dClutch market is a Solana account. Its bytes are the truth; the website, a
screenshot, and this paragraph are all renderings of them. `dclutch` fetches
those bytes over ordinary JSON-RPC and hands them to the same decoders the
on-chain programs use, so you can check a market for yourself without trusting
our website.

## Install

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/emberian/dragons-clutch/releases/latest/download/dclutch-cli-installer.sh | sh
```

macOS (Apple Silicon and Intel) and Linux x86-64. Or build it from this
directory with `cargo build --release`; the binary lands at
`target/release/dclutch`.

## What it does

```
dclutch market show <ADDRESS>          the Market Core account, every field
dclutch market decode --base64 <DATA>  the same, from bytes you already have
dclutch capability show <ADDRESS>      the Trading root that gates execution
dclutch capability decode ...          the same, offline
dclutch ticket ...                     a named seam; see below
```

`--rpc <URL>` picks the cluster (default `https://api.devnet.solana.com`; also
`DCLUTCH_RPC_URL`). `--json` swaps the prose for a machine-readable object.

A market being `Open` is necessary and not sufficient. Execution runs through a
capability root, so the honest pre-trade check is both commands:

```
$ dclutch market show 6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4
  phase                     Open
  This market is open: claims can be bought and sold, and it has not been answered yet.

$ dclutch capability show 7kPABbyrKFmqP65FUWDKxNinb2mW7gP3EXGkeEjFWy3N
  family                    Direct
  phase                     Open
  Direct trading is open on this market: new intents are admitted.
```

## What it does not do

It never signs, never submits a transaction, never opens a key file, and never
writes to a cluster. It takes no credential of any kind. Every subcommand is a
read.

`dclutch ticket` is therefore a **named seam and not a command**. Authoring a
Direct trade ticket needs a key. The Rust author exists —
`direct-intent-ticket-author-v1` in the operator binary
`dclutch-local-successor-bootstrap`, under
`tools/local-validator/bootstrap/successor/` — but it is private to that crate,
and copying it here would make a second author of a signing preimage. There is
one ticket author per language and it stays that way; the seam takes over when
the author becomes callable. To trade today, use the web panel at
<https://clutch.dregg.pro>, which signs with your wallet.

## Single authorship

This crate lays out no byte and parses no wire format. Three crates own
everything it interprets:

| what | owner |
| --- | --- |
| the Market Core account | `dclutch_market_core_codec::CoreState`, emitted from `formal/dclutch-semantics/EmitMarketCoreRust.lean` |
| the activation header | `dclutch_capability_program_contract::CapabilityRootHeaderV1` |
| the Direct family tail | `dclutch_direct_codec::successor::DirectRootStateV1` |

`dclutch` calls `decode` on each and prints what comes back. It knows no field
offset. A decoder that refuses is reported as a refusal rather than smoothed
over, which is why a market founded before the PDA-bump widening is refused on
length with that reason named rather than rendered as a partial market.

## The endpoint is treated as a credential

Commercial RPC providers put the API key in the URL path. Nothing here ever
prints a URL: refusals name the scheme and host only, `Debug` on the parsed
arguments is written by hand to redact it, and messages a library wrote for us
are rewritten before they are shown. Four tests hold that.

## Where this crate sits

Its own Cargo workspace, like every other host tool under `tools/`. The
protocol workspace is `no_std` kernel and contract crates; this binary needs
`std`, an HTTP client and a TLS stack, and adding that tree to the root
`members` list is an integration decision this crate does not get to make for
everyone else.

It is also the crate `dist` builds for the GitHub Releases of this repository.
The release configuration is `dist-workspace.toml` at the root of
<https://github.com/emberian/dragons-clutch>, which is where the generated
workflow has to live: this working tree is vendored there as the `dclutch/`
subtree, and GitHub only reads `.github/workflows/` at a repository root.

Release notes live in `CHANGELOG.md` at that repository's root, which is the
only place `dist` reads them from — a `CHANGELOG.md` in this directory is found
and then ignored, which is worth knowing before you write one. There is
deliberately no second copy here to drift.

## License

AGPL-3.0-or-later, like the rest of dClutch. A release distributes, so the
release body carries the license and a link to this source. The repository *is*
the source: <https://github.com/emberian/dragons-clutch>.
