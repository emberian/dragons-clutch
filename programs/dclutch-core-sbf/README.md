# dclutch-core-sbf

This is the isolated Solana adapter for the sparse Market Core. The generated
`dclutch-market-core-codec` transition functions remain the lifecycle semantic
owner; this program authenticates physical accounts, current Registry-selected
programs, child CPI return data, and commit-last state writes.

The current executable slice routes:

- `Found`, using only the 72-byte Core request;
- `ActivateCapability` and `CloseCapability`, using the compact generic
  Trading-capability route.

Other generated lifecycle actions, including the distinct
`InitializeClaims` action, remain refused until their action-specific adapters
are joined. An ABI tag is not a claim that its runtime route is present.

## Found frame

Found has exactly 24 pairwise-distinct accounts, in this order:

1. payer (signer, writable)
2. vacant/dust-prefunded Market PDA (writable)
3. persisted beneficiary RentCredit
4. RentCredit program
5. Realm raw record
6. Realm finalized cursor
7. Product Instance raw record
8. Product Instance finalized cursor
9. Product Terms raw record
10. Product Terms finalized cursor
11. finite result-domain raw record
12. finite result-domain finalized cursor
13. occurrence SourceMaterial raw record
14. SourceMaterial finalized cursor
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

All accounts after the first two are read-only. Core authenticates every raw
record, its dust-tolerant empty system-owned finalized cursor, all cross-record
identities, the current Registry/Loader-backed Core artifact, the Market PDA,
and the canonical RentCredit before transferring only the exact missing rent,
allocating, assigning, and writing the 320-byte state.

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
authenticated manifest raw (read-only), family tail
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
outer-account aliases, and the exact v0 envelope. The optimized SBF build is
checked separately for verifier stack diagnostics. Real multi-program SVM
tests for stale/underfunded funding, substituted releases, child-ack mismatch,
and late child failure are required before this adapter is release evidence.
