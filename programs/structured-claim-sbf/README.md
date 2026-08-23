# Structured Claim SBF wrapper

This is the separately deployed, explicitly non-production executable seam
for StructuredClaim. Its capability mask is currently zero: no action is
admitted while the Product, deployment-release, and collateral value-route
authorities are still being joined.

The disabled create seam uses an exact 33-account frame. The base side
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

Full-vector wrap/unwind and exact terminal redemption are implemented behind
the zero mask. Compaction and retirement remain incomplete. Descriptor v1 is
decode-only; live state is descriptor v2.

This crate contains no fixtures, mock Source provider, wallet, deployment, or
client signing path. Building it does not authorize deployment.

The requestable SBF heap allocator is an explicitly named adapter/runtime trust
boundary. Its unsafe cursor access is neither Eggcrate code nor kernel
evidence and must be reviewed and measured against the exact deployed ELF.
