# Releases

The release log for binaries published from this repository. Each section below
is the body of one GitHub Release; `dist` reads this file to write it, which is
why it lives here at the repository root rather than beside the crate.

Everything released so far is the `dclutch` CLI, built from
`dclutch/tools/dclutch-cli/`.

## 0.1.0-devnet.1

**The first release of anything in this project.** It is a prerelease, and it
is one small read-only tool — not the protocol, not a wallet, not a trading
client.

### What `dclutch` does

A dClutch market is a Solana account, and its bytes are the truth. This binary
fetches those bytes over ordinary JSON-RPC and hands them to the same decoders
the on-chain programs use, so you can check a market yourself instead of
trusting our website.

```
dclutch market show <ADDRESS>          the Market Core account, every field it carries
dclutch market decode --base64 <DATA>  the same rendering, offline, from bytes you have
dclutch capability show <ADDRESS>      the Trading root that decides whether a market can trade
dclutch capability decode ...          the same rendering, offline
```

A market being `Open` is necessary and not sufficient — execution runs through
a capability root — so the honest pre-trade check is both commands. Each one
ends in a plain sentence ("This market is open: claims can be bought and sold,
and it has not been answered yet.") because the field dump above it is not much
use to someone who has not read the protocol. `--json` swaps the prose for a
machine-readable object; `--rpc <URL>` (or `DCLUTCH_RPC_URL`) picks the cluster.

### Which cluster

**Solana devnet, and nothing else.** dClutch is deployed on devnet and nowhere
else; devnet SOL is not money and devnet state is wiped by the cluster's
operators without notice. That is the default endpoint, and it is a safe
default for exactly that reason: the worst outcome of running the wrong command
is reading the wrong test chain.

There is no mainnet deployment. If you find something claiming to be one, it is
not ours.

### What it is not

It never signs, never submits a transaction, never opens a key file, and never
writes to a cluster. It takes no credential of any kind. Every subcommand is a
read.

`dclutch ticket` is a **named seam, not a command**, and refuses with the reason
in full. Authoring a Direct trade ticket needs a key, and the signed message is
owned by one emitted codec per language; a second implementation of a signing
preimage is a signature that verifies nowhere, discovered at the refused trade.
The Rust author exists — `direct-intent-ticket-author-v1` in the operator
binary `dclutch-local-successor-bootstrap` — but it is private to that crate
today, so the seam names it and points at the web trade panel instead of
growing a copy.

### If you hand it a commercial RPC endpoint

Providers put the API key in the URL path. This binary never prints a URL:
refusals name the scheme and host only, and messages written by the HTTP
library are rewritten before you see them.

### License, and where the source is

AGPL-3.0-or-later. A release distributes, so, plainly: the full license text is
`LICENSE` in this repository and ships inside every archive below, and this
repository **is** the corresponding source for this binary and for every
program it reads — <https://github.com/emberian/dragons-clutch>. The crate is
`dclutch/tools/dclutch-cli/`.

### Platforms

macOS on Apple Silicon and Intel, and Linux x86-64. Windows is absent
deliberately rather than forgotten: the TLS stack this tree already pins
(`ring`, through `reqwest` 0.12.28) needs an assembler on `windows-msvc` that
was not verified here, and an unverified target does not belong in a release.

### Verified before publication

The same code was built locally in release mode and run against live devnet,
reading market `6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4` and its
capability root `7kPABbyrKFmqP65FUWDKxNinb2mW7gP3EXGkeEjFWy3N`. It reproduced
the market's recorded PDA bumps (252 / 254 / 253) independently of the tool
that founded it. 33 unit tests cover the refusals, the renderings and the
endpoint redaction.
