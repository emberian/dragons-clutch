# Offline local-validator profile

`dclutch-local-validator` is a keyless wrapper around the pinned
`solana-test-validator 4.0.2`. It is local-only: it passes `--config /dev/null`,
uses no clone flags or RPC URL, binds only `127.0.0.1`, and supplies a fixed
genesis mint public key so the validator does not need a client keypair. It
never reads a wallet, `~/.config`, browser data, or public RPC.
The validator may generate its own validator, vote, stake, and faucet keypairs
inside the new ledger; the launcher never supplies or reads any pre-existing
key material.

Run `verify-fixtures` before any launch, or use `start`, which does so itself.
The command verifies the SHA-256 of all ten committed artifacts in
`fixtures/pyth/local-upgraded-2026-08-22`, including both provider ELFs and the
raw non-ELF evidence files. The caller must provide both the dClutch SBF ELF
and its expected SHA-256; a launch refuses a mismatch.

```sh
tools/local-validator/dclutch-local-validator start \
  --ledger /absolute/new/scoped/dclutch-ledger \
  --dclutch-elf /absolute/path/to/dclutch_sbf.so \
  --dclutch-program-id YOUR_DCLUTCH_PROGRAM_ID \
  --dclutch-sha256 YOUR_EXPECTED_LOWERCASE_SHA256

tools/local-validator/dclutch-local-validator status \
  --ledger /absolute/new/scoped/dclutch-ledger
tools/local-validator/dclutch-local-validator stop \
  --ledger /absolute/new/scoped/dclutch-ledger
```

Use `--foreground` with `start` to retain the validator process in the terminal
after its local JSON-RPC `getHealth` probe succeeds. The only fixed listeners
are RPC `18890` (WebSocket `18891`), faucet `18892`, gossip `18893`, and the
dynamic local range `18900-18931`. A start requires a nonexistent absolute
ledger leaf, makes only that leaf, and writes a sentinel. `status` and `stop`
will operate only on a directory bearing that sentinel; `stop` additionally
requires the saved PID still to be a `solana-test-validator` command naming
that exact ledger. Nothing is deleted by the tool.

## Loader-evidence boundary

This is deliberately an **ELF-tail local-execution profile**, not a
release-bound Pyth provider-loader profile. The committed fixture records exact
Upgradeable Loader Program and ProgramData account identities, headers,
deployment slots, and complete-body hashes. Its `.so` files are only the ELF
tails.

On `solana-test-validator 4.0.2`, `--bpf-program` installs a BPF-loader program
with upgrades disabled, so it has no ProgramData account at all. That cannot
satisfy dClutch's Program-to-ProgramData linkage or deployment-slot checks.
`--upgradeable-program ADDRESS ELF none` is the strongest runnable choice: it
creates upgradeable program accounts at the required program IDs and disables
upgrades, but the validator constructs new Program/ProgramData headers at
genesis. The CLI accepts no complete-account-body or deployment-slot input.
Therefore it cannot reproduce the captured ProgramData addresses, historical
slots (`460336311` receiver and `460336290` router), or complete-body hashes.

The launcher uses `--upgradeable-program … none` for dClutch, the Pyth receiver,
and the Pyth router, then emits a machine-readable profile naming this boundary,
the exact IDs, captured evidence IDs/slots, local paths, hashes, ports, and
detected validator version. A release-bound profile is intentionally refused by
omission: there is no mode that calls this runnable profile release-bound.
Local execution here remains local-validator evidence, not provider-release or
mainnet evidence.

## Tests

`tests/test.sh` is an offline shell test. It validates fixture pins, validates
the argument refusal path, and checks that the profile documents the loader
boundary. It never compiles or requires a current dClutch ELF.
