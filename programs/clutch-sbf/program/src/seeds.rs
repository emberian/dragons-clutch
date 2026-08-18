//! **PROPOSED** domain-separated PDA seed schema for the bring-up program.
//!
//! Nothing in this module is frozen.  The seed strings, the seed order, and the
//! choice of which identity goes into which seed are a proposal made by this
//! bring-up lane so that obligation 1 of
//! `docs/implementation/SOLANA_REFERENCE_ADAPTER.md` can be exercised end to
//! end against a real runtime.  Changing any byte here changes every account
//! address, so a later freeze is an ABI decision, not a refactor.
//!
//! Every prefix is a single seed of at most 32 bytes and every tuple is well
//! under the 16-seed limit, so `find_program_address` can never fail for
//! length reasons.
//!
//! The 32-byte owner identity carried by
//! [`clutch_solana_layout::PositionAccount::owner`] is interpreted here as the
//! raw bytes of the owning wallet address.  That interpretation is what lets
//! the program bind an authenticated signer to a stored position, and it is
//! also a proposal rather than a frozen rule.

use solana_pubkey::Pubkey;

/// Find the canonical program address and bump for one seed tuple.
///
/// Program-address derivation is a runtime syscall under `target_os =
/// "solana"`.  Off-chain derivation is not compiled into this crate: doing it
/// in process needs the `curve25519` backend, whose proc-macro dependency has
/// no archive in this host's offline crate cache.  The differential harness
/// derives the same addresses out of process with the pinned
/// `solana find-program-derived-address` command, using the seed prefixes
/// exported above so that there is still exactly one source of truth for the
/// seed bytes.
pub fn find(program_id: &Pubkey, seeds: &[&[u8]]) -> (Pubkey, u8) {
    #[cfg(target_os = "solana")]
    {
        Pubkey::find_program_address(seeds, program_id)
    }
    #[cfg(not(target_os = "solana"))]
    {
        let _ = (program_id, seeds);
        unimplemented!(
            "off-chain PDA derivation is not compiled into clutch-sbf; \
             see docs/implementation/SBF_BRINGUP.md"
        )
    }
}

/// Realm account seed prefix.
pub const SEED_REALM: &[u8] = b"dragons-clutch:realm:v1";
/// Profile account seed prefix.
pub const SEED_PROFILE: &[u8] = b"dragons-clutch:profile:v1";
/// Market account seed prefix.
pub const SEED_MARKET: &[u8] = b"dragons-clutch:market:v1";
/// Hoard account seed prefix.
pub const SEED_HOARD: &[u8] = b"dragons-clutch:hoard:v1";
/// Position account seed prefix.
pub const SEED_POSITION: &[u8] = b"dragons-clutch:position:v1";
/// Reference-only kernel-aggregate account seed prefix.
pub const SEED_KERNEL: &[u8] = b"dragons-clutch:kernel:v1";
/// Reference-only external-shadow account seed prefix.
pub const SEED_EXTERNAL: &[u8] = b"dragons-clutch:external:v1";
/// Reference-only replay-sequence account seed prefix.
pub const SEED_REPLAY: &[u8] = b"dragons-clutch:replay:v1";
/// Seed prefix for the market-wide supply-ledger account.
pub const SEED_SUPPLY: &[u8] = b"dragons-clutch:supply:v1";

/// Canonical Realm address and bump.
pub fn realm_pda(program_id: &Pubkey, realm: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_REALM, realm])
}

/// Canonical Profile address and bump.
pub fn profile_pda(program_id: &Pubkey, realm: &[u8; 32], profile: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_PROFILE, realm, profile])
}

/// Canonical Market address and bump.
pub fn market_pda(program_id: &Pubkey, realm: &[u8; 32], market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_MARKET, realm, market])
}

/// Canonical Hoard address and bump.
pub fn hoard_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD, market])
}

/// Canonical Position address and bump.
pub fn position_pda(program_id: &Pubkey, market: &[u8; 32], owner: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_POSITION, market, owner])
}

/// Canonical reference-only kernel-aggregate address and bump.
pub fn kernel_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_KERNEL, market])
}

/// Canonical reference-only external-shadow address and bump.
pub fn external_pda(
    program_id: &Pubkey,
    market: &[u8; 32],
    owner: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_EXTERNAL, market, owner, &generation.to_le_bytes()],
    )
}

/// Canonical reference-only replay-sequence address and bump.
pub fn replay_pda(
    program_id: &Pubkey,
    market: &[u8; 32],
    owner: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_REPLAY, market, owner, &generation.to_le_bytes()],
    )
}
