# Upstream provenance and license

This support tool builds one binary from the official Anza Agave repository:

- source: <https://github.com/anza-xyz/agave>
- commit: `549805f3e85f345c9df98d59759691443eef57aa`
- upstream version: `4.0.2`
- pinned Rust toolchain: `1.93.1`
- upstream license: Apache License 2.0
- upstream license at the pinned commit:
  <https://github.com/anza-xyz/agave/blob/549805f3e85f345c9df98d59759691443eef57aa/LICENSE>

No Agave source or binary is vendored into this repository. `fetch-source.sh`
creates a detached, shallow checkout under the ignored repository `.cache/`
directory. Cargo's registry/git cache and build target are also kept there.

The committed patch changes five address paths used by this test-validator build.
It makes the faucet use the CLI `--bind-address`, makes JSON-RPC and RPC
WebSocket use the node addresses already derived from that same bind address,
and binds the lazy outbound QUIC and fallback UDP client endpoints to IPv4
loopback instead of a wildcard IPv4 address. Both client paths retain upstream's
`VALIDATOR_PORT_RANGE`; only their IP addresses change. The patch does not
change transaction, ledger, consensus, SVM, or protocol semantics. It is a local
test-infrastructure modification to Apache-2.0 upstream code; the surrounding
Dragon's Clutch scripts remain under this repository's AGPL-3.0-only license.

The upstream commit, lockfile, license, zero-context patch, and all four patched
source files are SHA-256 pinned in `pins.env`. Every fetch and build refuses a
mismatch. The zero-context form keeps the committed patch free of whitespace-only
context lines while preserving exact reverse-application checks.
