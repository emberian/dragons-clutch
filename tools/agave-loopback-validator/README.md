# Pinned loopback-only Agave test validator

Agave 4.0.2's `--bind-address 127.0.0.1` does not cover every socket. The
validator-node sockets honor it, but the standalone faucet, JSON-RPC, and RPC
WebSocket servers are constructed with wildcard IPv4 addresses. Lazily created
outbound QUIC and fallback UDP client sockets also bind wildcard IPv4. This tool
builds the exact installed upstream revision with those five address paths
repaired, then provides a fail-closed runtime listener probe.

This is local test infrastructure. It is not an official Agave release, a
mainnet artifact, or protocol evidence by itself.

## One online preparation, then offline builds

The official source fetch is the only source checkout step:

```sh
tools/agave-loopback-validator/fetch-source.sh
```

The first build may populate the repository-local Cargo cache:

```sh
tools/agave-loopback-validator/build.sh --allow-network
```

Every subsequent build defaults to Cargo offline mode and refuses source,
patch, lockfile, license, or toolchain drift:

```sh
tools/agave-loopback-validator/build.sh
```

On macOS the RocksDB bindgen helper also needs a runtime `libclang.dylib`.
`build.sh` selects Homebrew LLVM first and the active Xcode toolchain second,
then records the exact library path and SHA-256. Set
`CLUTCH_AGAVE_LIBCLANG_PATH` to an explicit library directory to override that
selection.

Artifacts are intentionally ignored by Git:

- binary: `.cache/agave-loopback-validator/bin/solana-test-validator`
- build cache: `.cache/agave-loopback-validator/target/`
- Cargo cache: `.cache/agave-loopback-validator/cargo-home/`
- build record: `.cache/agave-loopback-validator/build-provenance.txt`

The build record includes the exact binary SHA-256 and wall time. The binary
must be invoked with `--bind-address 127.0.0.1` and an explicit, disjoint port
plan. Remember that Agave reserves `RPC_PORT + 1` for WebSocket; the faucet must
not use that port.

## Fail-closed runtime proof

After launching the patched binary, probe the exact PID and port plan:

```sh
tools/agave-loopback-validator/probe-listeners.sh \
  VALIDATOR_PID RPC_PORT FAUCET_PORT \
  .cache/agave-loopback-validator/bin/solana-test-validator
```

The probe requires healthy RPC plus reachable WebSocket and faucet listeners on
`127.0.0.1`; requires the PID to be the expected binary; rejects every
non-loopback TCP listener or UDP bind owned by that PID—including lazily opened
QUIC or UDP client sockets—and actively refuses reachability through each configured
non-loopback IPv4 address on the Mac.
Absence of required socket evidence is a failure, not a skipped check.

See `PROVENANCE.md` for the upstream/license boundary and `pins.env` for every
pinned digest.
