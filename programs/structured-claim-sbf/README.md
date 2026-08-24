# Structured Claim SBF wrapper

This is the separately deployed development executable for StructuredClaim.
Its `profile-successor-chain-attached-dev` identity admits exactly actions
1/3/5/6/7/8 after the wrapper, central base, and Token-2022 releases each pass
the exact checked manifest and hostile loader boundary. It is not a production
or deployment claim.

The create route uses an exact 38-account frame. The base side
authenticates the Product RootV3, SeriesRegistryV4, SeriesMarketLinkV3, current BundleV7,
ReleaseV2/ProfileV4, current AttachmentV6, and the read-only content-addressed
WrapperRecipeSetV1 artifact selected by that attachment. It hostile-decodes the
complete fixed-layout recipe set, recomputes its set identity, and requires the
payload recipe and fixed-depth witness to be the exact published leaf before it
atomically records Product's first Structured and Wrapper admissions, funds the
`0xb7/1` Structured root with explicit refundable principal/donation
separation, and founds the empty PositionV3/Replay pair. The wrapper snapshots
and reauthenticates the root across CPI rather than treating successful CPI as
evidence of the claimed Product/root transition. Product RootV3 and the Series
link are writable only for first root admission; later descriptors must present
both read-only and cannot advance Product state. ProductReplayV2 and the
MarketFamilyCapabilityPolicyV1 are appended read-only so first admission can
use Product's current family authority. First admission prepares the RootV3
Structured-family successor, admits Wrapper against the already-created
descriptor/mint, initializes and hostile-reopens the final Structured root,
admits Structured against that root, and commits RootV3 last. Each Product
mutation consumes a distinct move-only physical postwrite.

The full-vector and terminal frames use 32 and 33 accounts. Each
places the Realm-selected collateral ProgramData immediately after its token
program; the base authenticates the exact current ELF/slot release and commits
that private value-route receipt into every transition receipt before mutation.
The wrapper and base import one exact source/account contract from the adapter:
action 1 uses 38 accounts, actions 3/5 use 32, action 6 uses 32, action 7 uses
33, and action 8 uses 34. That contract's implemented-source mask is distinct
from its checked-release admission mask; the named development profile requires
them to be exactly equal.

Full-vector wrap/unwind, beneficiary-free surplus compaction, and exact
terminal redemption are admitted by that exact profile. Compaction uses a
32-account compaction frame, performs an exact Hoard-to-neutral Token-2022 CPI
only when donated cash is nonzero, and reconciles the unchanged wrapper mint,
exact Hoard/neutral raw-token deltas, and exact
PositionV3/ReplayV3/HoardV2/ClaimLedgerV3 successors. The five appended roles
are the distinct Hoard authority, neutral token, Structured root, Product
`0xad` link, and immutable FundingTermsV2 artifact; the three loader releases
follow them at indices 29 through 31.
Descriptor retirement uses a 34-account frame. It reopens the exact current
BundleV7 and AttachmentV6 and appends Product RootV3. RootV3 and LinkV3 are
read-only for nonfinal descriptors and writable only for the final family
terminal. The wrapper first revokes the
zero-supply mint through its private mint-authority PDA, persists the descriptor
tombstone, and calls the base with only the distinct vault-owner PDA signed.
The base seals the purpose Replay, writes the permanent Position tombstone,
deletes Replay with exact principal/donation disposition, advances the
Structured root, and physically deletes the final root with exact
principal/donation separation. That last close yields one non-Copy
Structured+Wrapper family receipt inside the base invocation and immediately
consumes it into Product RootV3 and LinkV3. The wrapper hostile-reconciles the
RootV3 family transition, both LinkV3 obligation transitions, every Structured
postimage, and every exact rent delta.
Descriptor v1 is decode-only; live state is descriptor v2.

Every route requires `ObservedPositive` wrapper/base/Token-2022 release
artifacts. Each release reauthenticates the executable Program's linked
ProgramData address, positive loader slot, and SHA-256 of the complete
ProgramData body including ELF. The manifest label alone is never an account
or deployment authority.

This crate contains no fixtures, mock Source provider, wallet, deployment, or
client signing path. Building it does not authorize deployment, and no current
build/runtime evidence has been produced for this profile.

The requestable SBF heap allocator is an explicitly named adapter/runtime trust
boundary. Its unsafe cursor access is neither Eggcrate code nor kernel
evidence and must be reviewed and measured against the exact deployed ELF.
