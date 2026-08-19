# ResolutionWork V1 live SBF cut

Status: **routed release candidate; route-level real-SBF gates pass**.

This status is deliberately narrow. It admits the ResolutionWork route under
the frozen V1 source, compute, rent, and reward policy below. It is not a claim
that the complete Dragon's Clutch deployment is terminally live: production
source release, direct selection, and the system-wide storage/retirement
profile retain their own STOPs.

ResolutionWork replaces one occupation-resolution reduction with a bounded,
restartable state machine without accepting an off-chain payout authority:

```text
Begin -> active program-owned Work -> Fold* -> Finalize -> sole Resolution v4
                                      \-----> permitted Abort -> no Resolution
```

Work is public program-owned state, not confidential state. Its accumulator is
an internal exact accumulator. No instruction accepts caller record bytes,
Merkle material, proofs, points, masses, weights, or a payout vector.

## Frozen ABI and identities

The live registry allocates account tag 22 and intent tags 32 through 35. The
common intent-envelope version is 3. Exact bodies are:

```text
Begin    83B: nonce[32], finalizer:u8, expires:u64, deposit:u64, cost_digest[32]
Fold    107B: work[32], archive[32], archive_commitment[32], cursor:u64, count:u8
Finalize 74B: work[32], end_cursor:u64, archive_commitment[32]
Abort    74B: work[32], cursor:u64, archive_commitment[32]
```

The active Work account is version 1 and exactly 1,296 bytes. It stores full
Market/Terms/source/archive/basis/resolution identities, the 304-byte canonical
BasisSpec artifact, semantic versions, exact archive geometry and cursor,
sixteen checked `u128` masses, the frozen cost schedule, the payer/refund and
Reserve identities, and the active funding/donation ledger. Only ACTIVE is an
encodable Work status; terminal transitions close it.

There is one deterministic lock per Market:

```text
Work    = PDA("resolution-work-v1", market)
Reserve = PDA("resolution-reserve-v1", market, Work)
```

The payer nonce is inside `work_commitment` but cannot create a parallel Work
address. `work_commitment` also binds the exact declared payer deposit. Begin
requires an active Market, canonical unresolved occupation Resolution v4, and
an absent System-owned zero-data Work address. A stale Work can never deadlock
the Market indefinitely: the compile-time TTL is in `8..=4096` slots and every
incomplete state, including zero progress, is permissionlessly abortable
strictly after expiry. A complete valid state remains finalizable late and is
never made abortable merely by time.

## Source and semantic authority

Begin, every Fold, and Finalize authenticate the canonical program-owned
SourceSpec PDA and compare its stored bump with the canonical derivation. They
authenticate the one program-owned 2,560-byte SourceArchive PDA, source release,
window/domain/generation, sealed flag, archive stored bump, record geometry,
zero padding, and full stored commitment. Fold reads its bounded contiguous
chunk directly from cursor-indexed archive bytes. No caller record/proof tail
exists.

No post-seal archive mutator exists. Init, append, and seal are the complete
write surface; append/seal refuse an already sealed account. The router exposes
no unseal, rewrite, truncate, or repair-in-place instruction.

The current archive carries exact accepted price records but no authenticated
gap record. V1 therefore accepts exact points (`low == high`) and explicitly
refuses genuine intervals; it never silently skips or caps them. The internal
accumulator retains typed gap semantics for a later archive version.

Finalize restores and validates the exact summary, requires the end cursor,
uses the frozen finalizer, and constructs the canonical existing occupation
Resolution v4 with `resolved_slot = 0`, exactly matching monolithic v4. The
private prepared-application seam reuses the monolithic Market/kernel/supply
transition and outcome-mint truth checks. It combines one already-authenticated
Terms load with full Kernel payout-set equality, but does not weaken the full
Terms digest authentication performed by Finalize preparation. Full Resolution
bytes and Market/kernel/supply bytes are compared against monolithic execution
in the bank test.

## Account roles

| Transition | Exact account count and order |
|---|---|
| Begin | 11: payer, Market, Terms, Resolution, SourceSpec, SourceArchive, Work, Reserve, System, Rent, Clock |
| Fold | 8: worker, Market, Terms, SourceSpec, SourceArchive, Work, Reserve, Clock |
| Abort | 8: caller, frozen payer, Market, Terms, Work, Reserve, SDK incinerator, Clock |
| Finalize | `15 + outcome_count`: monolithic prefix of actor, Market, Hoard, kernel, supply, Terms, Resolution, Feed, SourceSpec, SourceArchive; then outcome mints; frozen payer, Work, Reserve, SDK incinerator, Clock |

Every role has exact signer, mutability, owner, length, PDA, and alias checks.
Terminal roles contain no System program. The neutral sink must be the exact
SDK-defined Solana incinerator address and writable; a substituted account
refuses before mutation.

## Rent, rewards, and donations

The cost schedule is recomputed from compile-time program policy plus the Rent
sysvar. A caller cannot select a schedule by presenting a digest. V1 charges
are all zero. Selected external-work policy inputs are:

```text
headroom             = 5/4
CU rounding quantum  = 50,000
runtime CU ceiling   = 1,400,000
base-fee cap         = 10,000 lamports
priority-price cap   = 1,000,000 micro-lamports/CU
keeper tip           = 100,000 lamports

Fold reward          = 1,160,000 lamports per successful call
Fold per-record      = 0
Finalize reward      = 1,510,000
Abort reward         = 860,000
```

Begin itself is payer-originated construction work and receives no Reserve
reward. Its selected external caller quote is 1,160,000 lamports and is not
part of the Work deposit.

Under `Rent::default()`, Work rent is 9,911,040 lamports and zero-data Reserve
rent is 890,880, for exact principal `R = 10,801,920`. For `n` archive records:

```text
minimum_deposit(n) = R + n * 1,160,000 + max(1,510,000, 860,000)
                   = 12,311,920 + n * 1,160,000
minimum_deposit(32) = 49,431,920
```

The singleton term is intentional: callers may choose any bounded partition,
so every accepted worst-case path is funded even if all 32 records use separate
Fold calls.

Predictable PDAs may be hostile-prefunded. Begin snapshots the exact combined
prebalance, then nevertheless transfers the full payer rent principal and full
work budget. The persisted ledger replaces a redundant stored-deposit field
with monotone `donation_lamports`. At every successful active transition:

```text
payer_deposit = rent_locked + prepaid_remaining + charges_paid + rewards_paid
actual_held   = rent_locked + prepaid_remaining + charges_paid + donations
```

Any later surplus is added to donations before the next successful write. It
can never increase payer credit or worker budget. Terminal accounting returns
exactly rent plus unused prepaid budget to the frozen payer, pays the exact
caller reward only from prepaid budget, transfers donations to the canonical
incinerator, and zeros both Work and Reserve. This deliberately destroys
unsolicited surplus; it has no beneficiary and no later Work can spend it.
V1 charges are zero, so no nonzero charge-disposition claim is made.

## Real-SBF evidence

Final candidate artifact (dirty integration closure before its exact source
commit):

```text
ELF:    /tmp/resolution-work-sbf.cYXtPl/out-16/clutch_sbf.so
SHA256: a5725a3d8e149b2b52605e1785f7ad29fdc6b2db1ed32ca83a31b41822d6b6a1
bytes:  1,228,192
log:    /tmp/resolution-work-sbf.cYXtPl/build-16.log
bank:   /tmp/resolution-work-sbf.cYXtPl/bank-16.log
```

Build-16 ProgramTest measurements (degree 2, real program ELF) were collected
with the selected CU limit and the maximum V1 priority-price input:

| Transition | Records/span | CU consumed | Selected limit | Accounts |
|---|---:|---:|---:|---:|
| Begin | 1 | 805,308 | 1,050,000 | 11 |
| Fold | 1 | 802,253 | 1,050,000 | 8 |
| Begin | 2 | 810,992 | 1,050,000 | 11 |
| Fold | 2 | 812,193 | 1,050,000 | 8 |
| Begin | 3 | 807,674–807,676 | 1,050,000 | 11 |
| Fold | 1, partitioned span 3 | 804,616 | 1,050,000 | 8 |
| Fold | 2, partitioned span 3 | 809,225 | 1,050,000 | 8 |
| Fold | 3 | 813,128 | 1,050,000 | 8 |
| Begin | 4 | 805,860 | 1,050,000 | 11 |
| Fold | 4 | 815,573 | 1,050,000 | 8 |
| Finalize | span 3 | 1,094,832 | 1,400,000 | 19 (`15 + 4`) |
| expired Abort | zero progress | 587,197 | 750,000 | 8 |
| monolithic comparison | span 3 | 1,253,326 | 1,400,000 | 14 |

The largest measured Finalize now clears the exact 1,120,000-CU 25%-headroom
threshold. All four bank tests passed. Nonmonotonicity remains explicit; these
rows admit only their exact
measured shapes and do not support extrapolation. The final campaign also
executes the selected CU limit and maximum priority price, not merely a 1.4m
diagnostic ceiling.

The bank campaign proves:

- Begin underfunding refuses despite hostile prefunds; replay is byte exact;
- pre-Begin and later donations cannot become payer credit or rewards;
- wrong cursor, replay, same-domain archive substitution, malformed stored
  SourceSpec bump, and substituted sink refuse atomically;
- Fold sizes 1 through 4 execute from the sealed account bytes;
- early Finalize, post-completion expired Abort, and substituted-sink late
  Finalize preserve every watched byte and lamport;
- late Finalize succeeds, writes byte-identical monolithic Resolution/Market/
  kernel/supply state, pays exact rewards/refund, burns exact donations, and
  removes both Work and Reserve;
- expired zero-progress Abort is permissionless, pays only the frozen reward,
  returns exact payer principal, removes both accounts, and permits a nonce-
  separated reopen.

Host gates are 157/157 program tests and 10/10 hostile layout tests. The
isolated no-std/no-allocation model remains the semantic oracle for arbitrary
chunk compositions, mass overflow, exact/largest-remainder behavior, gaps, and
associativity/monolithic equivalence.

## Stack and remaining scope

Earlier final-LTO audit after the stack decomposition found zero first-party
diagnostic survivors and direct `r10` references no greater than 4,096 for
Begin/Fold/Finalize/Abort and the shared occupation apply seam. A final audit
must be rerun over the final policy ELF; pre-LTO dependency diagnostics are not
deployment claims.

ResolutionWork does not introduce a BasisDomain cache. A future immutable cache
may be keyed by the full basis-artifact digest plus archive-domain digest, but
it may cache only validated domain facts—not cursor, masses, or a payout
vector. Source append continues to touch only the source lineage; it never fans
out writes to subscribing Markets or Work accounts.
