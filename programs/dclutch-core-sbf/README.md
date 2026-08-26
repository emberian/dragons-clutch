# dclutch-core-sbf

This is the isolated Solana adapter for the sparse Market Core. The generated
`dclutch-market-core-codec` transition functions remain the lifecycle semantic
owner; this program authenticates physical accounts, current Registry-selected
programs, child CPI return data, and commit-last state writes.

The current executable slice routes:

- `Found`, using only the 72-byte Core request;
- `OpenMarket`, using the Core request followed by one exact 672-byte Custody
  request for replay initialization or Hoard-vault creation;
- fixed-role Resolution effects for `CreateFund`, `VerifyFundReady`,
  `AdmitTerminal`, and `CloseFund`;
- `ActivateCapability` and `CloseCapability`, using the compact generic
  Trading-capability route.

Other generated lifecycle actions, including the distinct
`InitializeClaims` action, remain refused until their action-specific adapters
are joined. An ABI tag is not a claim that its runtime route is present.

## Immutable infrastructure root

Before an accepted Found, the current Core deployment creates one immutable
144-byte `ProtocolInfrastructureProfileV1` PDA under
`[b"dclutch:infrastructure:v1"]`. Its one-time 16-byte initialization is
authorized by the exact upgrade-authority signer observed in the current Core
ProgramData. The initializer independently authenticates finalized Registry
and Rent `ArtifactReleaseV1` records and their current Loader deployments; both
artifacts must already be immutable and both ProgramData accounts must have no
upgrade authority. The profile stores only the exact Registry and Rent
`(program, ArtifactReleaseId)` bindings. It has no update or close route.

The initialization frame has exactly 14 accounts: payer, vacant profile PDA,
Core ProgramData, current Core upgrade authority, Registry artifact
raw/staging, Registry program/ProgramData, Rent artifact raw/staging, Rent
program/ProgramData, Rent sysvar, and System. The payer and Core authority may
be the same signer; every other alias is refused. Release deployment order is:
initialize the profile while the Core authority exists, revoke the Core
authority, finalize/activate the immutable Core artifact, then Found.

## Found frame

Found has exactly 31 pairwise-distinct accounts, in this order:

1. payer (signer, writable)
2. vacant/dust-prefunded Market PDA (writable)
3. persisted beneficiary RentCredit
4. RentCredit program
5. Realm raw record
6. Realm finalized cursor
7. Product Runtime V2 graph-root raw record
8. Product finalized cursor
9. Product-selected result-domain raw record
10. result-domain finalized cursor
11. Product-selected portfolio raw record
12. portfolio finalized cursor
13. compact SourceMaterialV2 raw record
14. SourceMaterialV2 finalized cursor
15. capability manifest raw record
16. capability manifest finalized cursor
17. execution ReleaseSet raw record
18. execution ReleaseSet finalized cursor
19. current Registry activation cache
20. this Core program
21. immutable Core ProgramData
22. Registry program
23. Rent sysvar
24. System program
25. immutable Core infrastructure profile
26. Registry ArtifactRelease raw record
27. Registry ArtifactRelease finalized cursor
28. Registry ProgramData
29. Rent ArtifactRelease raw record
30. Rent ArtifactRelease finalized cursor
31. Rent ProgramData

All accounts after the first two are read-only. Core authenticates every raw
record, its dust-tolerant empty system-owned finalized cursor, all cross-record
identities, the Market PDA, and the canonical RentCredit before transferring
only the exact missing rent, allocating, assigning, and writing the 352-byte
state. Trust is ordered: authenticate the Core-owned infrastructure profile;
directly reauthenticate its immutable Registry and Rent artifacts/current
Loader deployments; observe the selected release-set digest; reauthenticate
the immutable current Core release; only then trust the remaining finalized
Registry records and activation projection. RentCredit owner and PDA are bound
to the profiled Rent program. The state and its PDA persist the exact Registry
program used at Found; every later release reauthentication rejects a
substituted Registry before CPI.

The 208-byte SourceMaterialV2 owns only Source policy links and the selected
Product-record content digest. Core requires that digest to equal the root of
the independently authenticated Product/domain/portfolio graph; neither the
Source record nor the Found request may supply stable Product, result-domain,
portfolio, basis, release, or outcome-width facts.

## OpenMarket frame

Instruction data is `Request(72) || CustodyRequestV1(672)`. Both staged calls
use the fixed prefix:

```text
Core caller-authority PDA, Market, activation cache, Registry,
Core program + ProgramData, Custody program + ProgramData,
Realm raw + finalized cursor, Custody replay
```

`InitializeReplay` appends payer, System, and Rent (14 outer accounts).
`OpenVault` appends Mint, vacant Hoard vault, Custody authority, token program,
payer, System, and Rent (18 outer accounts). Core forwards its writable Market
read-only to Custody, authenticates the sole Registry-owned finalized Realm,
verifies the immediate Custody receipt, replay bytes, rent delta, Vault owner,
Mint, authority, and zero token balance, and only then commits
`Founding+Ready -> Open+Consumed`. Replay initialization is a prerequisite
effect and does not mutate Core state.

## Fixed-role Resolution frame

Instruction data is:

```text
Request(72) || CoreEffectEnvelopeV1(280)
|| CapabilityFundingHeaderV1(16) || ResolutionRoleRequestV1(288)
```

Core reauthenticates the persisted Registry and current Core and Resolution
artifacts, derives the one Core caller-authority PDA, invokes the exact child
frame, and requires a producer-bound 240-byte `CoreEffectAckV1` plus the
action-specific poststate digest before any Core write. `CreateFund` is a
prerequisite effect with no Core transition; `VerifyFundReady` commits
`Prepaid -> Ready`; `AdmitTerminal` independently authenticates the finalized
Product Runtime V2 graph and commits `Open+Consumed -> Terminal`;
`CloseFund` authenticates the closure/funding projection but does not by itself
complete Core retirement.

All four actions share accounts 0 through 15: caller authority, Market,
activation cache, Registry, Core program/ProgramData, Resolution
program/ProgramData, SourceMaterialV2 raw/staging, capability manifest
raw/staging, Source state, and the ordered recovery/exhaustion/failure
FundingStates. Their exact tails are:

- Create (20 total): Rent, System, RecoveryPolicyV2 raw/staging.
- Verify (21 total): beneficiary, Clock, Rent, RecoveryPolicyV2 raw/staging.
- Admit (24 total): terminal certificate, Rent, then Product, result-domain,
  and portfolio raw/staging pairs.
- Close (24 total): terminal certificate, closure receipt, beneficiary, Clock,
  Rent, System, then RecoveryPolicyV2 raw/staging.

Create, Verify, and Close independently authenticate the exact Product-free
496-byte RecoveryPolicyV2 selected by SourceMaterialV2 and require its sole
admitted attempt plus the Source/recovery/failure identities to select the
same three manifest funding entries. Admit instead reauthenticates the complete
Product graph and derives the native `u32` outcome count; no caller count or
legacy Product bridge participates.

## Generic capability frame

Instruction data is:

```text
Request(72) || CoreEffectEnvelope(280)
|| CapabilityExecutionSelection(144)
|| CapabilityFundingHeader(16)
|| family_request(B)
```

The fixed width is therefore `512 + B` bytes. The 16-byte header carries only
the nonzero funding-account count. Profile 1 admits 1 through 16 accounts; a
wider profile requires a new manifest/physical ABI rather than truncation.

The outer account order is:

```text
0  Market (writable)
1  Realm raw
2  Realm finalized cursor
3  capability manifest raw
4  capability manifest finalized cursor
5..5+n  child-owned FundingState accounts (writable, strict entry-index order)
then child activation root/context (writable)
then Registry activation cache
then Core program, Core ProgramData
then selected Trading program, Trading ProgramData
then Registry program, Rent sysvar
then Core caller-authority PDA
then family-owned tail
```

The outer count is `14 + n + t`, where `t` is the family tail. Core invokes the
selected Trading program with:

```text
Core authority (signer), root (writable), n FundingStates (writable),
authenticated manifest raw (read-only), authenticated Market (read-only),
family tail
```

The child is the only writer and custody authority for every FundingState and
for its activation root. Core validates the manifest entries, derived child
PDAs, prestate, exact poststate, root lifecycle, immediate return-data producer,
and exact `CoreEffectAckV1`; only then does it update
`outstanding_capabilities`. Core never treats a program-owned FundingState as a
payer and never mirrors child balances.

## Measured transaction envelope

The pinned SDK serializer test covers the current widest standard General
profile: a 256-byte family request, 16 FundingStates, and a three-account family
tail. That is 768 instruction-data bytes and 33 instruction account metas.
With one address lookup table the complete one-signature v0 transaction is
1,040 bytes, below Solana's 1,232-byte packet limit. The identical transaction
without a lookup table is 2,029 bytes and is refused by the packet boundary.
This is a measured profile, not a universal promise for arbitrary family tails.

## Evidence boundary

Unit tests cover hostile instruction truncation, noncanonical funding headers,
outer-account aliases, Registry substitution, and the exact v0 envelope. The
optimized SBF build is checked separately for verifier stack diagnostics.
`run-open-market-program-test.sh` executes real Registry, Rent, Core, and
Custody ELFs. It proves exact-authority one-time profile initialization,
mutable-infrastructure refusal, a 258-outcome Found31, Registry/Rent
substitution and mutable-Core rollback without a Market write, then replay and
Vault creation with the commit-last Core transition.
Additional real multi-program campaigns remain required for every Resolution
action and for late Resolution-child rollback before that adapter is release
evidence.
