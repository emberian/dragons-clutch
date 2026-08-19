# Blank-bank construction evidence, V2

**Status:** implemented and exercised through the real SBF runtime on 2026-08-19.

This note records the construction boundary proved by
`programs/clutch-sbf/svm-tests/tests/blank_bank_lifecycle.rs`. It is narrower
than “the complete protocol is permissionless”: it proves that the immutable
policy plane and the initial market plane no longer need hidden
program-owned genesis accounts.

## Promoted path

Starting a local bank with only the Clutch program installed, an ordinary
wallet can:

1. create a real Token-2022 collateral mint, mint a nonzero fixed supply, and
   revoke its mint authority;
2. upload and seal the exact CollateralPolicy, PriceGrid, and Terms codecs by
   typed artifact instructions;
3. create the canonical Realm and Profile from the sealed policy PDA; and
4. submit `CreateMarket` using the sealed policy and Terms PDAs.

No Clutch-owned account is injected into either successful test. The
constructor creates the seven program-state PDAs, both outcome mints, and the
ImmutableOwner Hoard token account. The degree-zero case creates the 165-byte
V2 `ResolutionAccount`; basis degree one creates the 319-byte V3
`NativeResolutionAccount`. Degrees two and three select the same V3 ABI by the
same immutable Terms field and are covered by the host relation tests.

The measured `CreateMarket` costs from the focused SBF run are:

| Terms basis | Resolution ABI | Resolution bytes | CU |
| --- | --- | ---: | ---: |
| degree 0 | categorical V2 | 165 | 916,052 |
| degree 1 | native V3 | 319 | 909,302 |

The fixture uses deterministic mint and holder keys, so these are reproducible
for the recorded ELF. They are evidence measurements, not promised budget
ceilings.

## Predictable-PDA prefunding

A predictable PDA is constructible when it is writable, non-executable,
System-owned, and has zero data. Its lamport balance is not initialization
evidence. Construction:

1. transfers only a rent shortfall from the payer;
2. PDA-signs System `Allocate` for the exact frozen width;
3. PDA-signs System `Assign` to Clutch or Token-2022; and
4. checks the exact owner, width, and unchanged funded balance after CPI.

The SBF success cases prefund Resolution, an outcome mint, and the Hoard token
above their eventual rent minima through ordinary transactions and prove the
excess remains a donation. They also exercise recovery from a one-lamport
System-owned Market target. The one-lamport prestate is injected because the
runtime correctly refuses an ordinary transaction that would leave a newly
credited zero-data account below the zero-data rent minimum; it is a primitive
edge-case test, not a claim that public transfer can create that sub-rent
prestate.

Bytes, executable state, or foreign ownership remain hard initialization
evidence. A focused negative case places bytes in the last outcome-mint target,
observes `AlreadyInitialized`, and proves transaction rollback left all seven
state targets and the earlier outcome mint absent.

## Canonical evidence boundary

`InitRealm`, `InitProfile`, and `CreateMarket` now require the program-owned
`policy(Profile, digest)` PDA produced by typed sealing. The former arbitrary
caller-buffer `InitPriceGrid` and `InitTerms` paths refuse as unsupported;
typed `SealArtifact` is their sole constructor.

The following boundary remains explicit: the Terms codec binds a content-valid
PriceGrid ID, but `SealArtifact(Terms)` and `CreateMarket` do not currently
receive the Grid account and therefore do not prove that the referenced
canonical Grid PDA exists. The successful lifecycle constructs it, but that
existence is not yet a consensus precondition of market creation. Closing that
gap needs an account-plane/version decision, not an off-chain convention.

## Remaining STOPs

- public construction and canonical linkage of the Feed/Epoch plane;
- the Terms-to-Grid existence check described above;
- native complete-set Split and external bearer redemption reconstruction;
- any older instruction whose local preflight still requires exactly zero
  lamports, even though the shared constructor itself is prefund-safe.

## Reproduction

From `programs/clutch-sbf/svm-tests`, after building the current SBF artifact:

```text
cargo test --locked --test blank_bank_lifecycle -- --nocapture --test-threads=1
```

The expected result is two passing tests and the two CU lines above. The host
program suite at the implementing commit passes 131/131 tests with:

```text
cargo test --offline -p clutch-sbf --lib
```
