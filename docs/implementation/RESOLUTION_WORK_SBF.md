# ResolutionWork V1 live SBF cut

Status: **routed live SBF V1; the exact measured route passes its selected
liveness profile**.

This status is deliberately narrow. It admits only the measured
ResolutionWork route under the frozen V1 source, compute, rent, and reward
policy below. It is not a program release or deployment claim, does not supply
a production source release, and does not establish terminal closure or
no-stranding for the rest of Dragon's Clutch. Direct selection and the
system-wide storage/retirement profile retain their own STOPs.

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

Replay is market/work-domain state (`Work` PDA, commitment, cursor, and terminal
close), not a Position replay increment; ResolutionWork does not consume an
owner's command sequence.

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

The current sealed default artifact was built twice from exact runtime source
`2d530d218b470e5e2d1cf52480c4a9d1636c08e1`. Both ordinary builds are
byte-identical. The measured liveness profile, exact ELF, audit, and same-ELF
bank/build logs are sealed at `7931e23`. The preceding
`83e124d`/`b5700a9`/`bd20711b…` seal, and the `7e8f6b1`/`b5da74f`/`a5725a3d…`
seal before it, remain valid historical evidence only; neither is retracted and
both keep their complete artifact directories. Unlike the `a5725a3d…` →
`bd20711b…` step, this is a materially different artifact: the Direct V3
selection lifecycle merge grows the declared runtime closure from 88 to 94
files and the stripped ELF from `1,228,192` to `1,490,544` bytes, and no
stripped section except `.dynstr` and `.shstrtab` stays byte-identical. Every
CU row below was therefore remeasured against exact `af6bb79c…` rather than
relabeled from a historical seal. Each measured ResolutionWork route moved by
exactly one CU, the cost of the one added dispatcher arm, while the monolithic
comparison row is unchanged.

```text
SHA256: af6bb79cc3766bd0d889b46dc1becfebe140c7df2746971943e9edf4efc2014b
bytes:  1,490,544
audit:  research/liveness-policy-profile/artifacts/af6bb79cc3766bd0/audit/
logs:   research/liveness-policy-profile/artifacts/af6bb79cc3766bd0/logs/
```

The audit report has SHA-256
`39a8b19cae23a2a02f7ba870b18b5a4b9a07af6876c05443d6dd28e8bb89ccfb`
and the upstream 50-file checksum ledger has SHA-256
`e433c17d4be57463e78eb47554cc6e84d22aab5c1a27a53e297f83a7a21304e0`.
The complete source/toolchain/dependency and same-ELF account is
[`RUNTIME_ARTIFACT_AUDIT.md`](../../research/liveness-policy-profile/artifacts/af6bb79cc3766bd0/audit/RUNTIME_ARTIFACT_AUDIT.md).
The superseded `bd20711b…` seal keeps its own report
`626a299dd879cff5f8c775b82b488c2d6b300a386b6d5f847913b5e14797e038` and
52-file ledger
`dbf55f8e28c1674fc0f76b434049fbc8ef1e906c46db6ac0457410eaebc35f35` under
`research/liveness-policy-profile/artifacts/bd20711b01828a74/`.

The sealed same-ELF ProgramTest measurements (degree 2, real program ELF) were
collected with the selected CU limit and the maximum V1 priority-price input:

| Transition | Records/span | CU consumed | Selected limit | Accounts |
|---|---:|---:|---:|---:|
| Begin | 1 | 805,309 | 1,050,000 | 11 |
| Fold | 1 | 802,254 | 1,050,000 | 8 |
| Begin | 2 | 810,993 | 1,050,000 | 11 |
| Fold | 2 | 812,194 | 1,050,000 | 8 |
| Begin | 3 | 807,675–807,677 | 1,050,000 | 11 |
| Fold | 1, partitioned span 3 | 804,617 | 1,050,000 | 8 |
| Fold | 2, partitioned span 3 | 809,226 | 1,050,000 | 8 |
| Fold | 3 | 813,129 | 1,050,000 | 8 |
| Begin | 4 | 805,861 | 1,050,000 | 11 |
| Fold | 4 | 815,574 | 1,050,000 | 8 |
| Finalize | span 3 | 1,094,833 | 1,400,000 | 19 (`15 + 4`) |
| expired Abort | zero progress | 587,198 | 750,000 | 8 |
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

The sealed same-ELF capture passed ResolutionWork 4/4. Its other default-ELF
suites and the corrected 15/15 native-resolution fixture are inventoried in
the artifact audit; `161f530` only corrected a stale fixture source-version
literal and did not change this ELF. The isolated no-std/no-allocation model
remains the semantic oracle for arbitrary chunk compositions, mass overflow,
exact/largest-remainder behavior, gaps, and associativity/monolithic
equivalence.

## Stack and remaining scope

The exact sealed ELF's final audit found zero diagnostics naming `clutch_sbf`,
zero diagnosed symbols surviving final LTO, and all 49,521 direct `r10`
references at or below 4,096 bytes. Begin, Fold, Finalize, Abort, and the shared
occupation apply seam each have a maximum direct frame of 4,096 bytes. This
closes the first-party stack diagnostic gate for this ELF only; it is not a
deployment, inclusion, or system-liveness claim.

The default artifact has no registered production source release. Since
`cfea8e8`, its 15-account Endow refuses `SourceReleaseUnavailable` (`0x79`)
before owner-plane allocation or Token-2022 CPI, with rollback. A successful
mock-source Endow requires the distinct `non-production-mock-source` ELF and
is not evidence for this default artifact. Live Direct V2 at `e874db1` also
remains a functional compute STOP: its full top-three Select reaches exactly
1,400,000 CU and rolls back. The staged Direct V3 successor merged at `fb72b34`
is routed and resident in this sealed ELF, but it is unpromoted here: it
carries no measured CU, rent/refund/close, or terminal-admission row in this
profile, so the Direct stops above remain V2's.

No global `LivenessPolicy` or no-stranding theorem has been promoted. The
profile still stops on production source/archive work, direct lifecycle,
account retirement/rent ownership, outcome-mint closure, and terminal
disposition of Hoard donations, bearer-burn forfeiture, and fractional
fragments.

ResolutionWork does not introduce a BasisDomain cache. A future immutable cache
may be keyed by the full basis-artifact digest plus archive-domain digest, but
it may cache only validated domain facts—not cursor, masses, or a payout
vector. Source append continues to touch only the source lineage; it never fans
out writes to subscribing Markets or Work accounts.
