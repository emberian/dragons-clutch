# Canonical dClutch Trading SBF

This is the one `ExecutionRoleV1::Trading` Program selected by a Market's
five-role `ExecutionReleaseSetV1`. General, Dealer, Direct, Series, and later
data-defined venue families do not receive separate Registry Program bindings.

The common dispatch boundary authenticates:

1. a current Registry `AuthenticatedRoleReceiptV1` for exactly Trading, this
   Program, and the Market-selected release set;
2. the immutable Trading root header, descriptor-sized mutable tail, and exact
   whole-account PDA under this Program;
3. the selected manifest bytes, entry index, and all selector coordinates;
4. the finalized, account-resident `CapabilityProgramV1` bytes whose complete
   digest equals `entry.release_id == selection.capability_release`;
5. exact config bytes whose digest equals `selection.config`; and
6. every content schema against the current artifact's supported projection
   and effect boundary before running the data-defined TransitionVM program.

`dispatch.rs` contains no General/Dealer/Direct/Series tag or static family
jump. Unsupported content identities fail closed. Family modules may supply
physical register projection and separately admitted effect application after
this common boundary; they do not become executable-authority selectors.
`SupportedContentV1` is nevertheless a compiled physical-profile gate in this
foundation. Instantiating it once per named family would remain a closed
adapter list, so this base is not by itself the open-family successor gate.
Final convergence requires finalized/interpreted AccountProfile,
derivation/effect-projection languages, or an equivalently certified AOT
profile, that make those schema IDs authenticated data rather than Rust cases.

Activation and closure carry
`CapabilityExecutionSelectionV1(144) || FundingListHeaderV1(16) || family
request`. The FundingState accounts are the list: no 36-byte caller-supplied
descriptors are inlined. Hot calls authenticate the immutable 232-byte root
header, the descriptor-sized mutable tail, and omit the selector.

Core's exact child-CPI account prefix is:

```text
0                         Core release-set caller-authority signer
1                         writable composite Trading root
2 .. 2+funding_count      ordered writable FundingState accounts
2+funding_count           selected manifest raw-record account
3+funding_count           authenticated Core Market account, read-only
4+funding_count ..        descriptor-account-profile-owned suffix
```

`TradingActivationRequestV1` and `TradingActivationAccountsV1` decode these
two common projections. The suffix is selected by the authenticated
descriptor's `account_profile` and must contain the finalized descriptor and
config record material, Registry/current-deployment admission accounts, and
the exact family resources required by that profile. It is not selected by a
General/Dealer/Direct/Series tag. The physical profile projector must validate
its exact suffix length, order, privilege, owner, PDA, and content IDs before
constructing `TradingFamilyContextV1` or TransitionVM registers.
The generic prefix rejects aliases involving Core authority, composite root,
any FundingState, manifest, or the Core-forwarded Market. Trading decodes that
Market's immutable Registry, release set, Market identity, and generation and
rejoins them to the Core caller-authority seeds; a family suffix cannot supply
a competing Market. Aliasing wholly inside the suffix is neither
universally accepted nor rejected here; the authenticated AccountProfile owns
that decision.

The maximum 1,304-byte schema-V1/profile-2 `CapabilityProgramV1` is a finalized
raw-record account, not instruction data. Its pinned default-rent balance is
9,966,720 lamports and the existing 768-byte record-page profile publishes it
as two Append pages. Its transition body is hostile-decoded as runtime-width
TransitionVM `ProgramV2`; V1 fixed-bank bodies are not an alternate path.

This base intentionally does not apply effects or dispatch to a family module.
Those integrations land only with exact schema support, child authority,
postcondition checks, commit-last persistence, and hostile rollback coverage.
The fixed five state-owning roles and current interpreter are a safe release profile,
not a permanent execution-strategy ceiling. A future checked stateless
accelerator may consume the same descriptor only through a new measured,
Registry-authenticated profile; it does not acquire Trading state or effect
authority.
