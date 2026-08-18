# SBF bring-up: one instruction, executed by a real SVM

Status: **bring-up evidence for a single instruction (`Split`)**. Not a complete
program, not audited, not a deployment authorization, and not mainnet, devnet,
or testnet evidence. `Resolve` and `RedeemInternal` refuse here exactly as they
refuse in the offline reference adapter, and every other instruction is refused
as out of scope.

This document records what the `programs/clutch-sbf` lane built, what actually
ran, what failed, and what is deferred. Numbers below are from the run recorded
in [Results](#results); re-running `programs/clutch-sbf/scripts/run_bringup.sh`
reproduces them.

## What this lane converts

Before this lane, `programs/solana-reference` was an *offline reference
transition adapter*: a pure function from caller-asserted account metadata and
account bytes to post-state bytes, with no entrypoint, no runtime, no address
derivation, and no write-back. The open question was whether that shape survives
contact with an actual SVM.

This lane answers that for one instruction. It produces:

1. a real deployable SBF ELF with a `entrypoint` symbol, built reproducibly by
   the pinned `cargo-build-sbf`;
2. a program that validates hostile `AccountInfo` metadata, derives and checks
   every program address, decodes through `clutch-solana-layout`, transitions
   through `clutch-kernel`, and writes back into runtime account data;
3. a **differential result**: for one fixture, the six state accounts after a
   real SVM execution are byte-identical to the offline reference adapter's
   post-state; and
4. three adversarial refusals that refuse in the SVM with the same meaning the
   offline adapter gives them.

It does **not** establish correctness of the protocol, of the kernel, of the
layout, or of any economic claim. It establishes that this one instruction
survives the account-facing boundary on the pinned toolchain.

## Layering, and where the logic is not

`programs/clutch-sbf/program` contains no semantic or economic logic:

- balances, supplies, collateral, and invariants come from `clutch-kernel`;
- byte ownership comes from `clutch-solana-layout` and from the reference-only
  account codecs in `clutch-solana-reference`;
- what the program adds is only what those crates cannot have: account
  authentication, program-address derivation, and write-back.

`clutch-kernel`, `clutch-solana-layout`, and `clutch-solana-reference` are
unmodified path dependencies. The program depends on `clutch-solana-reference`
**only** for the reference-only `KernelAccount`, `ExternalAccount`,
`ReplayAccount`, and `Request` codecs, so those byte layouts keep exactly one
semantic owner. It never calls `clutch_solana_reference::apply`; the transition
composition is written independently, which is what makes the differential a
comparison of two adapters over one kernel rather than a comparison of a
function with itself.

That the offline `apply` is genuinely absent from the artifact is checked, not
assumed. A linker map of the built ELF lists exactly these symbols from the
reference crate, and nothing else:

```text
clutch_solana_reference::KernelAccount::encode
clutch_solana_reference::KernelAccount::decode
clutch_solana_reference::KernelAccount::validate_shape
clutch_solana_reference::ExternalAccount::decode
clutch_solana_reference::ReplayAccount::encode
clutch_solana_reference::ReplayAccount::decode
clutch_solana_reference::Request::decode
```

`clutch-accumulator` entered the graph late, as a new dependency of
`clutch-solana-reference`. Not one of its symbols reaches the ELF either.

Reproduce with
`RUSTFLAGS="-C link-arg=-Map=<file>" cargo-build-sbf --manifest-path programs/clutch-sbf/program/Cargo.toml`.

## Proposed PDA seed schema

`programs/clutch-sbf/program/src/seeds.rs` is a **proposal**, not a frozen ABI.
Changing any byte changes every account address, so freezing it is an ABI
decision for a later gate.

| role | seeds |
| --- | --- |
| Realm | `"dragons-clutch:realm:v1"`, `realm_hash` |
| Profile | `"dragons-clutch:profile:v1"`, `realm_hash`, `profile_hash` |
| Market | `"dragons-clutch:market:v1"`, `realm_hash`, `market_id` |
| Hoard | `"dragons-clutch:hoard:v1"`, `market_id` |
| Position | `"dragons-clutch:position:v1"`, `market_id`, `owner` |
| kernel aggregate | `"dragons-clutch:kernel:v1"`, `market_id` |
| external shadow | `"dragons-clutch:external:v1"`, `market_id`, `owner`, `generation` (u64 LE) |
| replay sequence | `"dragons-clutch:replay:v1"`, `market_id`, `owner`, `generation` (u64 LE) |

Two further proposals ride along and are equally unfrozen:

- the 32-byte `PositionAccount::owner` identity is interpreted as the raw bytes
  of the owning wallet address, which is what lets an authenticated signer be
  bound to a stored position; and
- `generation` is part of the external-shadow and replay seeds, so a
  close/reopen produces different addresses rather than reusing a sequence.

Every address is recomputed with `find_program_address` and compared against the
supplied account key, and every stored bump is compared against the canonical
bump. Caller-supplied expected keys are never accepted. Two accounts carry no
bump field in their frozen layout — Profile and the reference-only kernel
aggregate — so those two are checked by address only; that gap is listed under
[Deferred checks](#deferred-checks).

## Instruction and account set

One instruction, `Split`, carried in the reference adapter's `Request` envelope
(`0xd1`, version, u64 sequence, layout action, `u16` length, frozen `Intent`
bytes). The account list is fixed at exactly nine, in this order, and a
different count refuses:

| # | role | signer | writable |
| --- | --- | --- | --- |
| 0 | actor (must be the position owner) | yes | — |
| 1 | Realm | no | no |
| 2 | Profile | no | no |
| 3 | Market | no | yes |
| 4 | Hoard | no | yes |
| 5 | Position | no | yes |
| 6 | kernel aggregate | no | yes |
| 7 | external shadow | no | yes |
| 8 | replay sequence | no | yes |

The external-balance shadow is **not** omitted. It could have been — `Split`
does not change it — but the closed single-position aggregate-closure check
(`internal + external == total_supply`) is only meaningful with it present, and
keeping it makes the differential an exact six-account byte comparison.

### Checks the program performs

Metadata, before any borrow: exact account count; actor signature; pairwise key
distinctness across all nine roles (including actor-versus-state aliasing);
program ownership of all eight state accounts; non-executable bit; declared
writability per role, including refusing a *writable* Realm or Profile; exact
data length per role.

Derivation: canonical address and canonical bump for Realm, Market, Hoard,
Position, external shadow, and replay; canonical address for Profile and the
kernel aggregate; and `MarketAccount::hoard_bump` against the derived Hoard bump.

Decoding: every account through its frozen codec, which re-checks length,
discriminator, version, enums, identities, and canonical padding.

Linkage, mirroring `validate_links` in the offline adapter and adding the
Realm/Profile edges that the offline adapter only checks at market
initialization: Realm/Profile/Market identity agreement, profile version
agreement, `realm.max_outcomes == MAX_OUTCOMES`, `outcome_count <=
max_outcomes`, Market/Hoard/Position/kernel/external/replay identity agreement,
owner and generation agreement across Position, external shadow, and replay,
lifecycle-versus-phase agreement, `lifecycle <= 1`, payout outcome count against
market outcome count, and outcome count within the kernel bound.

State: zero padding beyond the active outcome count in every balance vector;
aggregate closure before the transition and again after it; exact replay
sequence and a checked increment.

Transition: signer identity equal to `position.owner`; intent market and owner
bound to the stored accounts; `lifecycle == 0` and `close_state == 0`; checked
collateral cap; checked position-cash debit; then `clutch_kernel`'s
`MarketState::split`, which runs its own invariant check over the prospective
state before its first write.

Write-back: Hoard, Position, kernel aggregate, and replay are re-encoded through
their codecs. Market and the external shadow are left untouched because `Split`
changes neither; the differential still compares all six against the reference
adapter's re-encoded post-state, so a codec that failed to round-trip would fail
the comparison rather than be hidden by a rewrite.

Every refusal maps to a stable `ProgramError::Custom(code)`; the table is in
`programs/clutch-sbf/program/src/error.rs`.

### Deferred checks

Nothing below is implemented. Each is a real gap, not a formality.

Relative to what `programs/solana-reference` already does:

1. `Merge`, `Materialize`, and `Dematerialize` are refused
   (`UnsupportedInstruction`) even though the offline adapter implements them.
2. `validate_market_init` has no on-chain counterpart: there is no
   initialization instruction, so every account in the fixture is preloaded at
   genesis instead of being created and validated by the program.
3. `CreateMarket` refuses with `AuthorizationUnavailable` and
   `Resolve`/`RedeemInternal` refuse with `ResolutionEvidenceUnavailable`, which
   matches the offline adapter — these are intended refusals, not gaps, but they
   mean the program cannot bring a market into existence at all.

Relative to obligations 1-4 of `SOLANA_REFERENCE_ADAPTER.md`:

4. Rent-exemption and account lifecycle state are not checked (obligation 2).
   The fixture funds every account well above the rent-exempt minimum, which
   hides the question rather than answering it.
5. Profile and kernel-aggregate accounts have no stored bump to compare, so
   their derivation check is address-only (obligation 1).
6. Account creation, closing, close/reopen generation reuse, and the
   destination of closed-account lamports are untested (obligations 3 and 12).
7. Transaction-level replay is untested. The program consumes the local replay
   sequence, but no committed transaction is ever sent, so Solana transaction
   replay, durable nonces, instruction duplication within one transaction, and
   batch retries are all outside this evidence (obligations 3 and 9).
8. Transaction-atomic ordering is asserted, not demonstrated: the program
   validates and computes before its first write, and relies on SVM rollback to
   discard partial writes on a later failure. No test forces a failure after the
   first write (obligation 4).
9. No token program, CPI, mint, or escrow behaviour exists (obligations 5-7).
10. Multi-position aggregate closure remains refused by representation, exactly
    as in the offline adapter (obligation 11).
11. The resource envelope is one compute measurement for one fixture. Heap,
    account count, transaction size, and worst-case outcome counts are unmeasured
    (obligation 10).
12. There are no host-side unit tests of the program's processor. Off-chain
    address derivation is not compiled into the crate (see
    [Toolchain and offline constraints](#toolchain-and-offline-constraints)), so
    `process` cannot run on the host at all; the SVM differential is the only
    test of it.

## How the transition is executed

The harness never signs. Every identity it uses — program id, fee payer, actor,
a stranger, and an imposter address — is a System-program PDA of a fixed literal
seed, so the fixture is reproducible and this lane holds no key material of any
kind.

`scripts/run_bringup.sh` starts a `solana-test-validator` bound to loopback with
the ELF at the chosen program id and all nine accounts loaded at genesis, then
`scripts/simulate.py` sends each transaction to `simulateTransaction` with
`sigVerify: false`, `replaceRecentBlockhash: true`, and an `accounts` request for
the six state accounts, and compares the returned data against the reference
post-state.

What that does establish: the ELF is loaded and executed by an Agave bank; the
runtime serializes real account data into the VM and writes the program's
mutations back; the `is_signer`, `is_writable`, `owner`, and `executable` bits
the program reads are the runtime's, taken from the transaction message header
and the loaded accounts; program-address derivation runs as the real
`sol_try_find_program_address` syscall; and compute is metered.

What it does **not** establish: no Ed25519 signature is verified, so "the actor
signed" is a message-header fact rather than a cryptographic one; nothing is
committed to a ledger, so no fee is paid, no state persists, and no replay or
durable-nonce behaviour is exercised; and a simulated bank is not a cluster.

## Toolchain and offline constraints

Verified on this host, `aarch64-apple-darwin`:

```text
solana-cli 4.0.2 (src:549805f3; feat:6ff76655, client:Agave)
cargo-build-sbf 4.0.0
platform-tools v1.53
rustc 1.89.0
```

### The panamax mirror does not exist on this host

The lane brief named a full panamax mirror at `~/crates.io/full`, to be used
through a registry source replacement in `programs/clutch-sbf/.cargo/config.toml`.
It is not there:

```text
ls: /Users/ember/crates.io/full/: No such file or directory
```

`lcrio --source panamax` likewise resolves nothing. The only offline crate
source available is the ordinary Cargo download cache under `~/.cargo/registry`,
so `.cargo/config.toml` sets `[net] offline = true` instead of a source
replacement: a resolution that would need a fetch fails loudly rather than
silently reaching for crates.io. **Reproducing this build on a machine without
that cache requires network access or a real mirror**; `Cargo.lock` pins exactly
what to fetch.

### Dependency selection was forced by what the cache holds

`cargo-build-sbf` runs `cargo metadata`, which resolves for *every* platform and
requires a downloaded `.crate` archive for every package in the graph — even
packages that this platform never builds. The cache holds 2058 archives but 4147
unpacked sources, so several otherwise-reasonable dependency sets are
unbuildable offline. Two blockers, verbatim:

```text
error: failed to download `curve25519-dalek-derive v0.1.1`

Caused by:
  attempting to make an HTTP request, but --offline was specified
```

```text
error: failed to download `solana-define-syscall v5.2.0`

Caused by:
  attempting to make an HTTP request, but --offline was specified
```

The first is why the host-side `curve25519` backend of `solana-pubkey` is not
enabled anywhere in this workspace: it is needed only off-chain, it is never
built on `aarch64`, and enabling it makes `cargo metadata` fail. The consequence
is that `seeds::find` is a syscall under `target_os = "solana"` and
`unimplemented!()` off-chain, and the harness derives addresses out of process
with the pinned `solana find-program-derived-address` command instead. Seed
prefixes still come from `clutch_sbf::seeds`, so the seed bytes keep one source
of truth.

The second was resolved by pinning `solana-define-syscall` to `=5.1.0`, the
version whose source this host actually has. Its `.crate` archive is missing, so
the workspace patches that one dependency to
`programs/clutch-sbf/vendor/solana-define-syscall-5.1.0`, a verbatim copy of the
published crate with provenance and the crates.io checksum recorded in
`vendor/PROVENANCE.md`. This is build plumbing for an offline host, not a fork:
deleting the directory and the `[patch.crates-io]` entry restores an ordinary
registry dependency.

### Pins

| crate | version | source |
| --- | --- | --- |
| `clutch-accumulator` | 0.1.0 | local path (transitive, via `clutch-solana-reference`) |
| `clutch-kernel` | 0.1.0 | local path |
| `clutch-solana-layout` | 0.1.0 | local path |
| `clutch-solana-reference` | 0.1.0 | local path |
| `clutch-sbf` | 0.1.0 | local path |
| `clutch-sbf-harness` | 0.1.0 | local path |
| `five8` | 1.0.0 | registry |
| `five8_const` | 1.0.0 | registry |
| `five8_core` | 1.0.0 | registry |
| `solana-account-info` | 3.1.1 | registry |
| `solana-address` | 2.6.1 | registry |
| `solana-define-syscall` | 4.0.1 | registry |
| `solana-define-syscall` | 5.1.0 | vendored path (see above) |
| `solana-program-entrypoint` | 3.1.1 | registry |
| `solana-program-error` | 3.0.1 | registry |
| `solana-program-memory` | 3.1.0 | registry |
| `solana-pubkey` | 4.2.0 | registry |
| `solana-sanitize` | 3.0.1 | registry |

## Harness ladder: what was tried, in order

**(a) `solana-program-test` in-process bank — unavailable.** The crate is absent
from this host's offline cache in every version, along with `litesvm` and
`mollusk`. `cargo-test-sbf` is installed but has nothing to run.

**(a′) `agave-ledger-tool program run` — blocked by an upstream defect.** This
subcommand executes an SBF ELF against a mocked runtime from a JSON account
description and would have been the cleanest in-process harness. In the pinned
release it fails before reading the input, for every invocation:

```text
$ agave-ledger-tool program run -l <ledger> --input <file>.json --output json <program>.so
[... INFO agave_ledger_tool] agave-ledger-tool 4.0.2 (src:549805f3; feat:6ff76655, client:Agave)
error: The argument 'accounts_index_limit' wasn't found
```

The subcommand reads an argument it never registers, so no ledger, flag, or
input fixes it; `--accounts-index-limit` is rejected as an unexpected argument.
Building an in-process harness from `solana-program-runtime`'s
`mock_process_instruction` was also considered and rejected: the 4.2.1 sources
are unpacked on this host but not one of the ~40 crates in that graph has a
`.crate` archive.

**(b) `solana-test-validator` on loopback — reached, and this is the recorded
result.** See [How the transition is executed](#how-the-transition-is-executed).

**(c) ELF plus a manual plan — not needed.**

## Results

Command: `programs/clutch-sbf/scripts/run_bringup.sh`, recorded against commit
`ad1e330` on `aarch64-apple-darwin`.

The ELF is a function of the `clutch-kernel`, `clutch-solana-layout`, and
`clutch-solana-reference` sources, all three of which changed while this lane
ran. A later commit to any of them changes the hash; that is correct behaviour,
not a reproducibility failure. The reproducibility claim is that two builds of
one source tree agree.

### Reproducible ELF

Built twice into fresh target directories:

```text
pass 1  sha256=42d553132b0a22ebffd374c85d12a444e4ca8c3e99aa211322c5b8a947467cdd  bytes=102568
pass 2  sha256=42d553132b0a22ebffd374c85d12a444e4ca8c3e99aa211322c5b8a947467cdd  bytes=102568
sbf_reproducibility=PASS
```

Dynamic symbols: exports `entrypoint` and `custom_panic`; imports the syscalls
`sol_try_find_program_address`, `sol_log_`, `sol_panic_`, `sol_memset_`,
`sol_memcpy_`, `sol_memcmp_`, and `abort`. There is no CPI syscall and no token
program reference, because there is no CPI and no token code.

### Differential against the offline reference adapter

One `Split` of quantity 5 on a two-outcome market, from a fixture that mirrors
the offline adapter's own `Split` test: position generation 2, replay sequence 0,
position cash 100, reserved cash 7, collateral cap 1000, Hoard collateral 0, all
balances zero.

The offline adapter computes the expected post-state; the SVM executes the ELF;
the six returned accounts are compared byte for byte.

```text
accept: executed, unitsConsumed=72869
differential market    MATCH (unchanged by Split)
differential hoard     MATCH (changed by Split)
differential position  MATCH (changed by Split)
differential kernel    MATCH (changed by Split)
differential external  MATCH (unchanged by Split)
differential replay    MATCH (changed by Split)
```

`market` and `external` are unchanged by `Split` and match the reference's
re-encoded post-state; `hoard`, `position`, `kernel`, and `replay` change and
match exactly.

Diffing the pre-state and post-state bytes shows that the SVM execution changed
exactly the fields the offline adapter's own byte-evidence section names, at the
same offsets:

| account | offset | before | after |
| --- | --- | --- | --- |
| Hoard | collateral at `98..106` | 0 | 5 |
| Position | outcome 0 at `74..82` | 0 | 5 |
| Position | outcome 1 at `82..90` | 0 | 5 |
| Position | cash at `202..210` | 100 | 95 |
| kernel aggregate | supply 0 at `38..46` | 0 | 5 |
| kernel aggregate | supply 1 at `46..54` | 0 | 5 |
| replay | sequence at `74..82` | 0 | 1 |

No other byte of any of the six accounts changed.

### Refusals

Each adversarial case is run in the SVM and cross-checked against the offline
adapter's refusal for the same situation.

```text
refusal refuse-unsigned  Custom(0x0002) (offline reference: MissingSignature)
refusal refuse-stranger  Custom(0x0011) (offline reference: UnauthorizedActor)
refusal refuse-imposter  Custom(0x0009) (offline reference: WrongAccountKey)
```

- `refuse-unsigned` presents the position owner as a read-only, non-signing
  account.
- `refuse-stranger` presents a different authenticated signer.
- `refuse-imposter` presents byte-identical replay-account state at an address
  that is not the canonical replay PDA. Every decode and every linkage check
  passes on it, so only address derivation can refuse it. The offline adapter
  refuses the same situation as `WrongAccountKey` because it is handed a trusted
  binding; the program derives the address instead, which is the stronger check
  obligation 1 asks for.

### The differential is falsifiable

A comparison that cannot go red is not evidence. Mutating the reference
expectation for one field — Hoard collateral `5` to `6` at byte 98 of
`plan/expected/hoard.hex`, leaving the program and the SVM run untouched — turns
the check red on exactly that account and leaves the other five green:

```text
FAIL
  differential hoard: on-chain bytes != reference bytes
```

Reproduce by running the gate once, editing that byte in the work directory, and
re-running `scripts/simulate.py --plan <work>/plan` against the still-running
validator.

### Resource envelope

`unitsConsumed=72 869` for the accepting `Split`, against a 200 000
compute-unit default. Address derivation dominates: eight
`sol_try_find_program_address` calls. This is one measurement of one fixture with
two outcomes; it is not an envelope.

### Stack finding in the offline reference adapter

The SBF backend rejects two functions in `clutch-solana-reference` outright. They
are dead-code-eliminated from this ELF and are never called by a program, but the
finding stands on its own as obligation-10 evidence about the *offline adapter's
shape*: its by-value, whole-state calling convention does not fit an SBF frame.

```text
Error: Function _ZN23clutch_solana_reference5apply...E overflows the maximum
allowed frame space by accessing an offset 1984 bytes greater than the maximum
of 4096. Please, minimize large stack variables. Estimated function frame size:
6080 bytes. Exceeding the maximum stack offset may cause undefined behavior
during execution.

Error: Function _ZN23clutch_solana_reference20validate_market_init...E overflows
the maximum allowed frame space by accessing an offset 1600 bytes greater than
the maximum of 4096. Please, minimize large stack variables. Estimated function
frame size: 5696 bytes. Exceeding the maximum stack offset may cause undefined
behavior during execution.
```

`clutch-sbf`'s own `process` is not flagged. Staying inside the 4 KiB frame is
not automatic and is why the processor splits its large decoded values —
`MarketAccount`, `KernelAccount`, and `clutch_kernel::MarketState` are together
well over 4 KiB — into `#[inline(never)]` helpers that each get their own frame
and return only small facts. Any future instruction over this state has the same
problem and needs the same care.

## Reproducing

```sh
programs/clutch-sbf/scripts/run_bringup.sh          # full gate, ~1 minute
cargo test   --manifest-path programs/clutch-sbf/Cargo.toml
cargo clippy --manifest-path programs/clutch-sbf/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path programs/clutch-sbf/Cargo.toml --no-deps
```

The gate needs `solana-test-validator`, `python3`, and `curl`. It binds
`127.0.0.1:18899` and `127.0.0.1:19900` by default (`CLUTCH_RPC_PORT`,
`CLUTCH_FAUCET_PORT` override) and contacts nothing else.

## Correct description

"A bring-up SBF program for one instruction, whose account validation, address
derivation, and post-state bytes agree with the offline reference adapter on one
fixture under a local simulated bank." It is not a complete program, not
verified, not audited, and not authorization to deploy anywhere.
