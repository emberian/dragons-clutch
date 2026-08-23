# Structured Claim SBF wrapper

This is the separately deployed, explicitly non-production executable that
owns the Token-2022 half of StructuredClaim actions 1, 2, and 4. It calls the
base program's signer-gated vault-founding/action-35 endpoints and reconciles
authoritative Token-2022 mint and holder bytes in the same SVM transaction.

Create uses an exact 28-account frame. The base side authenticates the Product
SeriesMarketLink, BundleV2, and AttachmentV2, verifies fixed-depth recipe-set
membership, atomically records Product's first Wrapper admission, funds the
`0xaf/1` Structured root with explicit refundable principal/donation
separation, and founds the empty PositionV3/Replay pair. The wrapper snapshots
and reauthenticates the root across CPI rather than treating successful CPI as
evidence of the claimed Product/root transition.

The artifact does not enable full-vector, compaction, redemption, or retirement
coordinates. Descriptor v1 is decode-only; live state is descriptor v2.

This crate contains no fixtures, mock Source provider, wallet, deployment, or
client signing path. Building it does not authorize deployment.

The requestable SBF heap allocator is an explicitly named adapter/runtime trust
boundary. Its unsafe cursor access is neither Eggcrate code nor kernel
evidence and must be reviewed and measured against the exact deployed ELF.
