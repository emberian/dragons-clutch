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
upgrades. Because ProgramData is a canonical PDA of the program ID, it derives
the same ProgramData addresses as the captured release. But the validator
constructs new Program/ProgramData headers at genesis, and the CLI accepts no
complete-account-body or deployment-slot input. Therefore it cannot reproduce
the captured historical slots (`460336311` receiver and `460336290` router),
captured upgrade-authority option, or complete-body hashes.

The launcher uses `--upgradeable-program … none` for dClutch, the Pyth receiver,
and the Pyth router, then emits a machine-readable profile naming this boundary,
the exact IDs, captured evidence IDs/slots, local paths, hashes, ports, and
detected validator version. A release-bound profile is intentionally refused by
omission: there is no mode that calls this runnable profile release-bound.
Local execution here remains local-validator evidence, not provider-release or
mainnet evidence.

The launcher loads the real router and receiver ELFs, but it deliberately does
not initialize Wormhole, router, receiver, receiver-config, or price-update
state and does not execute the captured signed VAA. Accordingly, every emitted
profile records `provider_state_initialized: false`; this is not yet an
end-to-end provider campaign.

## Tests

`tests/test.sh` is an offline shell test. It validates fixture pins, validates
the argument refusal path, and checks that the profile documents the loader
boundary. It never compiles or requires a current dClutch ELF.

## Integrated profile

`dclutch-integrated-validator` creates a separate fresh profile on RPC `19890`
(WebSocket `19891`), faucet `19892`, gossip `19893`, and dynamic range
`19900-19931`. It loads the canonical dClutch program ID plus the same pinned
real Pyth router and receiver ELFs. `solana-test-validator 4.0.2` supplies the
real upgradeable SPL Token and Token-2022 genesis programs; the bootstrap client
authenticates their canonical Program-to-ProgramData linkage and captures their
ELF hashes before calling dClutch.

The launcher requires an exact source commit, reproducible source-archive hash,
and verifier-clean ELF hash. A build that merely emits an ELF while reporting
SBF verifier errors is not admissible. The launcher preflights ELF magic and
SHA-256 and requires a machine attestation bound to the source commit, archive
hash, exact ELF path/hash, canonical `[71; 32]` program ID, tool versions, build
command, and build-log hash. Its verifier status must be `clean` with zero
diagnostics. Validator genesis acceptance plus the RPC health probe is a second
gate: a loader/verifier diagnostic makes the validator exit and the launcher
refuses to report the profile ready.

```sh
tools/local-validator/dclutch-integrated-validator start \
  --ledger /absolute/new/scoped/integrated-ledger \
  --dclutch-elf /absolute/path/to/verifier-clean/dclutch_sbf.so \
  --dclutch-sha256 ELF_SHA256 \
  --source-commit FULL_GIT_COMMIT \
  --source-archive-sha256 ARCHIVE_SHA256 \
  --sbf-attestation /absolute/path/to/elf-attestation.json

tools/local-validator/dclutch-integrated-validator status \
  --ledger /absolute/new/scoped/integrated-ledger
```

This profile runs in the background and writes no keys outside its scoped
ledger. Stop is sentinel- and command-line-guarded; it never deletes the ledger.
The client-side payer and action keys remain only in process memory.

Run the standalone [`bootstrap`](bootstrap/README.md) client after the
integrated profile is healthy. With the same attested source and ELF pins, it
initializes the real router/receiver, writes and verifies the signed VAA, and
executes the complete local dClutch Realm/record/Found/Open/Source-resolution
composition. This proves only the named local validator release and exact ELF
execution. Regenerated Loader headers, deployment slots, and current wall clock
remain outside captured-devnet release identity, and the evidence keeps that
boundary machine-readable.

## Immutable multi-program successor profile

`dclutch-successor-validator` uses a fresh ledger on RPC `20890` — or on the
base `--rpc-port` / `$DCLUTCH_RPC_PORT` names, from which the whole block is
derived (`faucet BASE+2`, `gossip BASE+3`, `dynamic BASE+10..BASE+41`; BASE
20890 reproduces the historical `20890-20931` byte for byte). `status` and
`stop` read the base back out of the ledger's own profile, so a ledger started
on a nonstandard base is still addressable by path alone. Given
`--supervisor-pid PID` the validator is bound to that process's lifetime and is
killed when it dies **even if that process is SIGKILLed and never runs its own
cleanup**, which is how a finished campaign once left a validator with PPID 1
holding the pinned port. It requires
separately attested, actual Registry, Core, Claims, Trading, Resolution,
Custody, and RentCredit ELFs under seven pairwise-distinct program IDs. It also
loads the committed real Pyth router and receiver ELFs. Unlike
`--upgradeable-program`, the prepared ProgramData accounts preserve the
canonical fixed 45-byte Loader V3 metadata span: variant `3`, slot `0`,
authority `None`, zero authority padding, then the exact ELF.

The standalone [`successor`](bootstrap/successor/README.md) package prepares a
hash-pinned infrastructure plan containing all seven distinct ArtifactRelease
bodies, the five-role execution release set, the captured local-Pyth release,
and the exact expected 144-byte Core-owned Registry/Rent infrastructure
profile. Genesis contains six immutable Loader programs plus an explicitly
pre-init authority-bearing Core ProgramData account; Core is not recognized as
the immutable release until its authority is revoked to `None`. Finalized
Registry record bodies are the only other genesis fixtures.

Core `d6d5f2d` now provides verifier-clean infrastructure-init and Found31. The
remaining gate is one same-process supervisor retaining an ephemeral Core
upgrade authority only in memory across init, Loader revocation, immutable
release activation, and Found. The bootstrap `run` command currently validates
the complete plan and provider evidence, then fails before opening an RPC
connection or signing. It does not retain the obsolete direct Resolution V1
path or genesis-prepare Market/Source/Funding state. No checked production
release or captured Pyth deployment identity is claimed.
