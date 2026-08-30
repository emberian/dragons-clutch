# AggregateRetirement checkpoint split — 2026-08-28

## Result

The existing AggregateRetirement route cannot be submitted as one Solana
transaction. Its direct Core instruction carries 2,152 bytes of data and the
Registry-wrapped instruction carries 2,280 bytes of data. Either payload alone
exceeds Solana's 1,232-byte transaction packet limit, before signatures,
account keys, or message headers are counted. This is a wire-format cliff, not
a compute-budget measurement.

The replacement is a four-transaction, permissionlessly resumable state
machine:

1. `prepare` proves every Claims liability is zero. Claims clears the aggregate,
   resizes it to 256 bytes, assigns it to Core under the Claims PDA signer, and
   retains every aggregate lamport in the same account. Core authenticates the
   handoff receipt and writes `ClaimsClosed` last.
2. `close-vault` reauthenticates the checkpoint and exact Custody context,
   closes the empty HoardPrincipal vault, verifies the returned receipt and
   refund, and writes `HoardVaultClosed` last.
3. `close-replay` reauthenticates the rolling receipt history, closes the exact
   normal Custody replay, verifies the returned receipt and cumulative refund,
   and writes `CustodyReplayClosed` last.
4. `finish` reauthenticates the original immutable retirement request and
   bundle, release admissions, source receipt, every rolling receipt, every
   revision, and every refund coordinate. It closes the checkpoint into the
   original RentCredit, closes Market into that same RentCredit, then asks Rent
   to close RentCredit to its original wallet last.

No intermediate phase can reopen the Market or mint authority. A failed child
call or failed receipt check rolls back that transaction without advancing the
checkpoint. Replaying or skipping a suffix is refused by the exhaustive phase
and revision checks.

## Fixed ABI

The Core-owned checkpoint is exactly 256 bytes with magic `DCLTARC1`, version
1, and this exhaustive ordered phase partition:

| Tag | Phase | Durable meaning |
| ---: | --- | --- |
| 1 | `ClaimsClosed` | Claims proved zero liabilities and handed the emptied aggregate to Core. |
| 2 | `HoardVaultClosed` | The HoardPrincipal vault was closed after phase 1. |
| 3 | `CustodyReplayClosed` | The normal Custody replay was closed after phase 2. |

Every suffix request has a 192-byte fixed prefix. Its action magics are
`DCLTARV1` (close vault), `DCLTARR1` (close replay), and `DCLTARF1` (finish).
The checkpoint binds the unchanged Core prestate digest, complete original
bundle digest, phase-dependent Claims/Custody join digest, Claims and both
Custody receipt digests, Claims refund, cumulative Custody refund, generation,
Claims revision, Custody revision, and monotonic phase revision.

Claims uses distinct 256-byte request and 320-byte receipt families,
`DCLTCRQ1` and `DCLTCRC1`. The ordinary Claims close codecs refuse these bytes;
the handoff cannot be confused with a terminal close.

Rent's `CloseAccountsV2` accepts an optional ninth account only on the direct
Core path: the already zeroed, System-owned, zero-lamport checkpoint as a
read-only balance witness. This makes the source of RentCredit's checkpoint
refund visible to the nested Rent instruction's balance check. The existing
eight-account direct ABI and nine-account continuation ABI retain their prior
meaning.

## Exact packet census

The focused real-SBF harness compiled each instruction as a v0 transaction with
a dedicated address lookup table and two ComputeBudget instructions. The
complete key count includes the payer and ComputeBudget program. These are
wire-size and key measurements, not compute margins.

| Transaction | Instruction metas | Complete unique keys | Data bytes | Serialized wire bytes |
| --- | ---: | ---: | ---: | ---: |
| prepare | 35 | 36 | 808 | 1,135 |
| close-vault | 35 | 36 | 864 | 1,191 |
| close-replay | 35 | 36 | 864 | 1,191 |
| finish | 35 | 36 | 744 | 1,071 |

All four packets are at or below 1,232 bytes and all four compiled key sets are
at or below devnet's 64-lock limit.

## Focused evidence

`checkpointed_retirement_is_packet_bounded_resumable_and_conserving` passed
1/1 against the rebuilt real Core, Claims, Custody, and Rent SBF programs on
hbox. The fixture begins at the exact valid terminal prestate and exercises the
four actual transactions. It proves:

- prepare cannot strand or relabel Claims lamports: the account retains its
  exact lamports, becomes Core-owned, is exactly 256 bytes, and RentCredit is
  unchanged;
- close-vault before prepare and close-replay before close-vault both refuse
  without changing the observed lifecycle snapshot;
- a substituted Claims owner, a checkpoint reassigned away from Core, and a
  substituted terminal refund wallet each refuse without changing the hostile
  snapshot;
- replay of prepare, close-vault, and close-replay each refuses without changing
  bytes or lamports;
- the vault and replay close only in canonical order while Market and
  RentCredit remain live;
- finish removes checkpoint, Market, and RentCredit and transfers the exact
  sum of their original lamports to the immutable refund wallet; and
- the authenticated source receipt remains unchanged.

Checkpoint codec tests separately cover exact round trips, inactive-field
canonicality, action separation, overflow, replay, skip, and phase substitution.
Claims codec tests prove the handoff families round-trip and remain distinct
from terminal Claims closure.

The focused real-SBF run emitted diagnostic per-transaction compute
observations, but no M-61 compute margin is claimed. A margin requires the
20-seed pass count and arithmetic mean for each exact final ELF.

The final focused campaign was rebuilt from a clean `git archive` of program
source `adef965048b80d7d671cc538ba841db5d10942ad`, whose parent is mainline
`6d1a9bc857fc7678f7f5470bddd34c9e7e4d4009`. It includes the converged
Resolution V7 Accept/direct-close fixture as well as checkpointed retirement.
Its log is
`/tank/dregg-build/dclutch-aggregate-retirement-adef9650-evidence/log-lifecycle.txt`
(SHA-256
`3e79b3f3b3f6abc63b63703ec9d4cc9f00ed2560e428f61b666386991940ed08`).
The exact tested changed ELFs were:

| Program | Bytes | SHA-256 |
| --- | ---: | --- |
| Core | 1,045,856 | `2b2cf2e8cac9881cd4c4387651ba550c812ab020b62087a1f5be4da3881e1d43` |
| Claims | 1,023,632 | `d24120ce9b87b1956e7bc113d43c3240605dca746039f3a9358e45c606f70f8b` |
| Rent | 138,072 | `c28a3827c512148f89843b67e2ac8f4e1c59aac9c61987c72527951f459db16b` |

Fresh emitted-stack measurement builds produced zero stack-frame-overwrite
diagnostics for every changed link. Core measured 235 frames with a 3,968-byte
deepest frame, Claims measured 181 frames with a 3,904-byte deepest frame, and
Rent measured 24 frames with a 1,344-byte deepest frame. The reports are
`frame-core.txt`, `frame-claims.txt`, and `frame-rent.txt`
under the same evidence root, with SHA-256 values
`5349bd2924ce288e710f10fbb312fc6a756bb9a64cb99df27b3e3d96391ed692`,
`7defea258bd718770eaa2f5c659389cf389ab6ba222cd28c1353324662f34f58`,
and `96ce71f62115a935e52a28a0613ceacd43cfde3cd59d2e555dbdd2fc9f5d805e`.
These frame-measurement objects are evidence artifacts and are not substituted
for the tested optimized ELFs above.

## Integration surface and deferred convergence

The operator exposes `build_checkpoint_market_retirement_v1`, returning the
four fixed direct Core instructions in predecessor order. A durable external
caller must give each mutation its own crash journal and resume from the
onchain phase; it must never infer completion from process memory.

This slice does not replace the existing monolithic builder or its codec. That
route remains useful as a refusal/control fixture but is not executable on the
network. Full pre-genesis lifecycle, chaos, M-61, all-program release,
private-validator, and caller-journal convergence obligations are tracked in
`docs/VALIDATION_BACKLOG.md`.
