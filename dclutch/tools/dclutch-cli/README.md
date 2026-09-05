# `dclutch` — inspect dClutch state and build unsigned handoffs

A dClutch market is a Solana account. Its bytes are the truth; the website, a
screenshot, and this paragraph are all renderings of them. `dclutch` fetches
those bytes over ordinary JSON-RPC and hands them to the same decoders the
on-chain programs use, so you can check a market for yourself without trusting
our website.

## Install

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/emberian/dragons-clutch/releases/download/v0.1.0-devnet.2/dclutch-cli-installer.sh | sh
```

The URL names a version rather than `latest` on purpose, and it is not
pedantry: every release so far is a **prerelease**, and GitHub's
`/releases/latest/` endpoint skips prereleases entirely — that URL returns 404
today, which is a bad first thing to hand a stranger. Take the current version
from the [releases
page](https://github.com/emberian/dragons-clutch/releases). When a release
stops being a prerelease, `latest` starts working and this line can lose its
version.

macOS (Apple Silicon and Intel) and Linux x86-64. Or build it from this
directory with `cargo build --release`; the binary lands at
`target/release/dclutch`.

## What it does

```
dclutch market show <ADDRESS>          the Market Core account, every field
dclutch market decode --base64 <DATA>  the same, from bytes you already have
dclutch capability show <ADDRESS>      the Trading root that gates execution
dclutch capability decode ...          the same, offline
dclutch ticket author --keypair-env VAR ...  sign a Direct intent into a ticket
dclutch ticket verify <PATH>           check a ticket's signature, no key needed
dclutch general plan --route /absolute/route.json --output /absolute/plan.json
                                        derive one unsigned General wallet handoff
dclutch fractional-retirement-next --route /absolute/route.json --output /absolute/plan.json
                                        derive the state-selected next Fractional retirement act
```

`--rpc <URL>` picks the cluster (default `https://api.devnet.solana.com`; also
`DCLUTCH_RPC_URL`). `--json` swaps the prose for a machine-readable object.

## Producing a General wallet handoff

A General action is not safely described by a form full of caller-supplied
facts. Start with one route document emitted for the current release, then let
the shared Rust operator reacquire and authenticate its state:

```sh
dclutch general plan \
  --route /absolute/path/general-route.json \
  --output /absolute/path/general-plan.json
```

The command reads every routed account together at finalized commitment,
derives the action request and lifecycle from the authenticated artifacts, and
writes one new mode-0600 JSON file containing an unsigned Solana v0
transaction. The output path must not exist. No key is read, and nothing is
signed, simulated, or submitted. The transaction carries a recent blockhash,
so generate it immediately before wallet review rather than archiving it as a
durable instruction to execute later.

## Producing the next Fractional retirement handoff

Ordered Fractional retirement is deliberately not a form where an operator
chooses `begin`, a coordinate, or `finish`. Point the CLI at a current-release
route containing only public account addresses:

```sh
dclutch fractional-retirement-next \
  --route /absolute/path/fractional-retirement-route.json \
  --output /absolute/path/fractional-retirement-plan.json
```

The command performs two finalized reads. The first authenticates the release,
Market, terminal root, terms, and retirement cursor so the Rust owner can
derive the next act and, when needed, the exact Position, admission, and shard
Mint addresses. The second reacquires that complete graph in one observation
and compiles the production instruction into a packet-safe unsigned v0
transaction. The route has no fields for an action, coordinate, Mint, Position,
endpoint credential, or key. A missing canonical cursor or staging PDA is
preserved as exact zero-lamport System-owned vacancy; any other missing account
refuses.

The private output pins the request digest, release set, artifact identities,
current Program/ProgramData addresses, root revision anchor, cursor revision,
representation width, account frame, wire width, optional lookup table, and a
plain-language consequence and remedy. It expires with its recent blockhash.
No key is read and nothing is signed, simulated, or submitted.

A V1 route has this exact shape. Every value except the format and decimal slot
is a canonical base58 public address; `lookupTable` may be omitted when the
inline message fits.

```json
{
  "format": "dclutch/fractional-retirement-next-route/v1",
  "minimumFinalizedSlot": "123",
  "payer": "...",
  "root": "...",
  "lookupTable": "...",
  "coreMarket": "...",
  "claimsMarket": "...",
  "activationCache": "...",
  "registryProgram": "...",
  "coreProgram": "...",
  "coreProgramdata": "...",
  "claimsProgram": "...",
  "claimsProgramdata": "...",
  "tradingProgram": "...",
  "tradingProgramdata": "...",
  "rentProgram": "...",
  "rentProgramdata": "...",
  "rentCredit": "...",
  "cursor": "...",
  "termsRaw": "...",
  "termsStaging": "...",
  "tokenBehaviorRaw": "...",
  "tokenBehaviorStaging": "...",
  "rentSysvar": "...",
  "systemProgram": "...",
  "tokenProgram": "..."
}
```

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

## Authoring a ticket

A Direct inline fill settles two independently signed intents. A ticket is one
of them: the maker, their detached Ed25519 signature, and every field that
signature covers.

```
$ export DCLUTCH_MAKER_KEY=/absolute/path/to/keypair.json
$ dclutch ticket author \
    --keypair-env DCLUTCH_MAKER_KEY \
    --maker <PUBKEY> --market <PUBKEY> --collateral-account <PUBKEY> \
    --side sell --lifecycle ioc --outcome 3 --generation 7 --nonce 9 \
    --valid-from 11 --valid-through 4294967295 \
    --maximum-fill 100000000 --limit-price 500000 --fee-basis-points 50 \
    --out /absolute/path/seller-ticket.json
```

The bytes it writes are **byte-identical to the ones the browser trade panel
signs**, signature included — there is one ticket author per language, this is
the Rust one, and a cross-language vector pins the two together. It prints a
receipt carrying the ticket's SHA-256, which is exactly what the producer wants
told to it next.

**The key path is never an argument.** `--keypair-env` names an *environment
variable* holding the path; any flag that would carry a key, or a path to one,
is refused at parse — because a path on the command line is a path in the
process table and in the shell history. Nothing about the key reaches the
receipt or any refusal message either. Nothing is read off a cluster to fill a
field in: this command guesses no nonce, generation or slot window, because a
guessed field is a signature over something you did not mean.

`dclutch ticket verify <PATH>` reads a ticket back, checks the signature, and
prints every field it binds. It takes no key and no network.

## What it does not do

It never submits a transaction and never writes to a cluster. Reading takes no
credential of any kind; `ticket author` opens one key file, named by an
environment variable, and signs onto local disk. `general plan` and
`fractional-retirement-next` each write one private local file, but read no key
and leave every signature zero.

**Authoring is not submitting.** Settling a ticket needs the other side's ticket
and a transaction, and this binary sends none. A pair is settled by
`devnet-direct-trade-produce-v1` in the operator binary
`dclutch-local-successor-bootstrap`, which re-checks every signed field against
finalized chain state and refuses on any mismatch. A browser panel named by a
checked release manifest can instead ask a wallet to sign without ever seeing
a key file.

## Single authorship

This crate does not independently lay out protocol bytes. The semantic owners
of what it interprets and produces are:

| what | owner |
| --- | --- |
| the Market Core account | `dclutch_market::CoreState`, emitted from `formal/dclutch-semantics/EmitMarketCoreRust.lean` |
| the activation header | `dclutch_market::capability_program::CapabilityRootHeaderV1` |
| the Direct family tail | `dclutch_trading::successor::DirectRootStateV1` |
| General route, request, lifecycle, v0 compilation, and plan JSON | `dclutch_general_successor_operator` |
| state-selected Fractional retirement discovery, request, instruction, and v0 compilation | `dclutch_fractional_claim_operator` |

`dclutch` calls those owners and reports what comes back. A decoder or producer
that refuses is reported as a refusal rather than smoothed over, which is why a
market founded before the PDA-bump widening is refused on length with that
reason named rather than rendered as a partial market.

## The endpoint is treated as a credential

Commercial RPC providers put the API key in the URL path. Nothing here ever
prints a URL: refusals name the scheme and host only, `Debug` on the parsed
arguments is written by hand to redact it, and messages a library wrote for us
are rewritten before they are shown. The General planner uses the same
redaction rule and has hostile tests for duplicate response keys, wrong request
IDs, extra response fields, noncanonical base64, and mismatched account widths.

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
