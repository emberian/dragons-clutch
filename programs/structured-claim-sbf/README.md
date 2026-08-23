# Structured Claim SBF wrapper

This is the separately deployed, explicitly non-production executable that
owns the Token-2022 half of StructuredClaim actions 1, 2, and 4. It calls the
base program's signer-gated vault-founding/action-35 endpoints and reconciles
authoritative Token-2022 mint and holder bytes in the same SVM transaction.

The artifact does not enable full-vector, compaction, redemption, or retirement
coordinates. Descriptor v1 is decode-only; live state is descriptor v2.

This crate contains no fixtures, mock Source provider, wallet, deployment, or
client signing path. Building it does not authorize deployment.
