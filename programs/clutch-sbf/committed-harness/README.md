# Committed local-bank harness

This crate closes one deliberately separate evidence gap in the ordinary SBF
bring-up harness.  The ordinary harness uses `simulateTransaction` with
`sigVerify: false` and gives each conceptual lifecycle step its own genesis
prestate.  This runner instead:

1. accepts only an exact loopback RPC URL;
2. replaces the fixture transaction's zero recent blockhash;
3. signs each required message key with a matching fresh test-only keypair,
   including additional holder identities when a step needs them;
4. submits it with `sendTransaction`;
5. waits for a `confirmed` or `finalized` bank status; and
6. reloads every declared account at `confirmed` commitment and compares its
   bytes with the offline expectation.

Expected refusals are submitted with preflight disabled.  The runner requires
the bank to record the declared `InstructionError::Custom` and requires every
watched account to have the same bytes before and after the failed transaction.

This is not a wallet and is intentionally not in the SBF program workspace.
The surrounding script creates random payer and actor keypairs in a temporary
directory and removes that directory on every exit.  Additional test holder
keys may be supplied explicitly.  No keypair is accepted from the Solana CLI
configuration, and the runner never prints secret bytes.

## Current scope boundary

The plan has a mandatory `genesis_assisted` field.  While it is true, the
runner prints `NOT END TO END` and enumerates the program-owned accounts the
local validator injected at genesis.  In particular, the current
`CreateMarket` implementation expects eight zeroed, program-owned state PDAs
to exist already.  An ordinary wallet cannot create an account owned by the
Clutch program at a PDA for which it cannot sign.

Closing that gap requires one atomic program instruction (most naturally a
repaired `CreateMarket`) to System-program CPI-create, fund to rent exemption,
allocate, and assign these PDAs before encoding them:

- `MarketAccount`;
- `HoardAccount`;
- the founding owner's `PositionAccount`;
- `KernelAccount`;
- the founding owner's `ExternalAccount`;
- the founding owner's `ReplayAccount`;
- `SupplyLedgerAccount`; and
- `ResolutionAccount`.

The instruction must derive every address before moving lamports, require each
target to be genuinely absent rather than accepting arbitrary zeroed
program-owned storage, use `invoke_signed` with the frozen seed schema, and
rely on transaction atomicity for rollback.  A separate permissionless actor
plane initializer is also needed for every later owner/generation; otherwise
only the founding owner can ever receive a position, external shadow, and
replay lane.

Until those instructions exist and this plan sets `genesis_assisted` to false,
the evidence is accurately named: **signed, committed, sequential execution
from a genesis-assisted local prestate**.  It is not permissionless lifecycle,
deployment, devnet evidence, or mainnet evidence.
