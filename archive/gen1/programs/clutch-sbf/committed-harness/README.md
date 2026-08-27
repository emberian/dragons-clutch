# Committed local-bank harness

This crate closes one deliberately separate evidence gap in the ordinary SBF
bring-up harness.  The ordinary harness uses `simulateTransaction` with
`sigVerify: false` and gives each conceptual lifecycle step its own genesis
prestate.  This runner instead:

1. accepts only an exact loopback RPC URL;
2. replaces the fixture transaction's zero recent blockhash;
3. signs every required message slot with an explicitly supplied fresh
   test-only keypair;
4. submits with `sendTransaction`;
5. waits for a `confirmed` or `finalized` bank status;
6. reloads every declared account at `confirmed` commitment and compares its
   bytes with the offline expectation; and
7. snapshots all watched accounts around an expected refusal and requires
   exact rollback.

Run the complete gate from the repository root:

```sh
programs/clutch-sbf/scripts/run_committed.sh
```

The script builds the current ELF, generates a same-address 20-step plan,
starts a fresh `solana-test-validator`, drives the signed walk, and then starts
another fresh validator after corrupting one terminal expected byte.  The gate
passes only if the ordinary run stays green and the corrupted run fails with a
committed-byte mismatch.

This is not a wallet and is intentionally not in the SBF program workspace.
The script creates the payer, two wallet identities, and ordinary Token-2022
account identities in a private temporary directory, passes only their public
keys to fixture generation, and unlinks every generated key on exit.  Neither
the runner nor the script reads Solana CLI wallet configuration, prints secret
bytes, or admits a non-loopback RPC URL.

## Current scope boundary

`CreateMarket` now publicly creates its seven state PDAs, Hoard token account,
and outcome mints from genuinely absent targets.  A second wallet's first
backed `Endow` publicly creates its generation-zero Position and Replay.  The
walk also creates ordinary holder token accounts, transfers an Egg to a
positionless bearer, proves atomic rollback of a duplicated bearer exit, and
then burns and redeems that Egg.

The plan nevertheless has mandatory `genesis_assisted` provenance.  The
current walk preloads 11 prerequisites: Realm, Profile, Terms, two Feed heads,
collateral-policy evidence, two evidence buffers, and three observation pages.
The runner prints `NOT END TO END` and enumerates them before submitting any
transaction.

The feed advanced by the walk is not the already-matured feed used for
resolution.  There is no public artifact upload, `InitFeed`, authenticated
archive writer, or complete Epoch/candidate/receipt construction path yet.
`SettlePage` remains unimplemented.  There is also no `WithdrawCash`, so the
founder and second owner finish with 61 and 6 free cash atoms respectively
still inside pooled Hoard custody; the independent bearer alone receives its
three collateral atoms through `RedeemExternal`.

Accordingly, this evidence is **signed, committed, sequential execution from a
genesis-assisted local prestate**.  It is not a blank-bank lifecycle,
operatorless venue, deployment, devnet evidence, or mainnet evidence.  See
`docs/implementation/COMMITTED_SBF_WALK.md` for the exact source commit,
toolchain, ELF digest, signatures, byte-comparison counts, falsifiability run,
and remaining construction instructions.
