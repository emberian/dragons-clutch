# Structured Claim SBF wrapper

This is the separately deployed, explicitly non-production executable seam
for StructuredClaim. Its capability mask is currently zero: no action is
admitted while the Product, deployment-release, and collateral value-route
authorities are still being joined.

The disabled create seam uses an exact 34-account frame. The base side
authenticates the Product SeriesRegistryV2, SeriesMarketLink, current BundleV5,
ReleaseV2/ProfileV4, and current AttachmentV4, verifies
fixed-depth recipe-set membership, atomically records Product's first Wrapper
admission, funds the
`0xaf/1` Structured root with explicit refundable principal/donation
separation, and founds the empty PositionV3/Replay pair. The wrapper snapshots
and reauthenticates the root across CPI rather than treating successful CPI as
evidence of the claimed Product/root transition. The Series link is writable
only for first root admission; later descriptors must present it read-only and
cannot advance Product state.

The disabled full-vector and terminal frames use 32 and 33 accounts. Each
places the Realm-selected collateral ProgramData immediately after its token
program; the base authenticates the exact current ELF/slot release and commits
that private value-route receipt into every transition receipt before mutation.
The withdrawn canonical action 2/4 execution route is not dispatched, and the
wrapper explicitly refuses those historical wire variants.

Full-vector wrap/unwind, beneficiary-free surplus compaction, and exact
terminal redemption are implemented behind the zero mask. Compaction uses a
27-account vault-only frame, performs no Token-2022 CPI, and reconciles the
unchanged mint plus exact PositionV3/ReplayV3/HoardV2/ClaimLedgerV3 successors.
Descriptor retirement remains incomplete. Descriptor v1 is decode-only; live
state is descriptor v2.

This crate contains no fixtures, mock Source provider, wallet, deployment, or
client signing path. Building it does not authorize deployment.

The requestable SBF heap allocator is an explicitly named adapter/runtime trust
boundary. Its unsafe cursor access is neither Eggcrate code nor kernel
evidence and must be reviewed and measured against the exact deployed ELF.
