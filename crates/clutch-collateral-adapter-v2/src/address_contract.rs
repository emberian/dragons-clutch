// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical physical seed prefixes for Realm-selected collateral accounts.
//!
//! The pure crate owns only the byte strings. Solana adapters remain
//! responsible for PDA derivation and hostile account authentication.

/// Immutable Realm account PDA prefix.
pub const REALM_PDA_SEED_V1: &[u8] = b"dragons-clutch:realm:v1";
/// Immutable Profile V2 account PDA prefix.
pub const PROFILE_PDA_SEED_V1: &[u8] = b"dragons-clutch:profile:v1";
/// Immutable CollateralPolicy V2 account PDA prefix.
pub const COLLATERAL_POLICY_PDA_SEED_V1: &[u8] = b"dragons-clutch:policy:v1";
/// Full-width Market Hoard V2 account PDA prefix.
pub const HOARD_V2_PDA_SEED_V1: &[u8] = b"dc:hoard:v2";
/// Full-width Market ClaimLedger V3 account PDA prefix.
pub const CLAIM_LEDGER_V3_PDA_SEED_V1: &[u8] = b"dc:claim-ledger:v3";
/// Sole Hoard V2 token-custody authority PDA prefix.
pub const HOARD_AUTHORITY_V2_PDA_SEED_V1: &[u8] = b"dc:hoard-auth:v2";
/// Exact Hoard V2 external token account PDA prefix.
pub const HOARD_TOKEN_V2_PDA_SEED_V1: &[u8] = b"dc:hoard-token:v2";

const _: () = assert!(REALM_PDA_SEED_V1.len() <= 32);
const _: () = assert!(PROFILE_PDA_SEED_V1.len() <= 32);
const _: () = assert!(COLLATERAL_POLICY_PDA_SEED_V1.len() <= 32);
const _: () = assert!(HOARD_V2_PDA_SEED_V1.len() <= 32);
const _: () = assert!(CLAIM_LEDGER_V3_PDA_SEED_V1.len() <= 32);
const _: () = assert!(HOARD_AUTHORITY_V2_PDA_SEED_V1.len() <= 32);
const _: () = assert!(HOARD_TOKEN_V2_PDA_SEED_V1.len() <= 32);
