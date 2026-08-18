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
/// Feed head account seed prefix.
pub const SEED_FEED: &[u8] = b"dragons-clutch:feed:v1";
/// Immutable terms account seed prefix.
pub const SEED_TERMS: &[u8] = b"dragons-clutch:terms:v1";
/// Frozen price-grid account seed prefix.
pub const SEED_GRID: &[u8] = b"dragons-clutch:grid:v1";
/// Resolution record account seed prefix.
pub const SEED_RESOLUTION: &[u8] = b"dragons-clutch:resolution:v1";
/// Epoch/book-domain account seed prefix.
pub const SEED_EPOCH: &[u8] = b"dragons-clutch:epoch:v1";
/// Order-page account seed prefix.
pub const SEED_PAGE: &[u8] = b"dragons-clutch:page:v1";
/// Candidate-record account seed prefix.
pub const SEED_CANDIDATE: &[u8] = b"dragons-clutch:candidate:v1";
/// Final-pot account seed prefix.
pub const SEED_POT: &[u8] = b"dragons-clutch:pot:v1";
/// Settlement-receipt account seed prefix.
pub const SEED_RECEIPT: &[u8] = b"dragons-clutch:receipt:v1";

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

/// Canonical market-wide supply-ledger address and bump.
///
/// One ledger per market, not per position: the two-term ledger is a
/// market-wide aggregate, so its address must not carry an owner.
pub fn supply_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SUPPLY, market])
}

/// Canonical feed-head address and bump.
///
/// A feed is a Realm-scoped shared cursor, so the feed identity alone names it.
pub fn feed_pda(program_id: &Pubkey, feed: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_FEED, feed])
}

/// Canonical immutable-terms address and bump.
///
/// Terms are content-addressed by their own digest and namespaced by the Realm
/// that authored them, so the address is a function of exactly those two facts
/// and never of the market that later binds them.  One terms artifact can
/// therefore be shared by many markets without being re-uploaded.
pub fn terms_pda(program_id: &Pubkey, realm: &[u8; 32], terms: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_TERMS, realm, terms])
}

/// Canonical frozen price-grid address and bump.
///
/// Content-addressed by the grid digest, namespaced by Realm, for the same
/// reason as [`terms_pda`].
pub fn grid_pda(program_id: &Pubkey, realm: &[u8; 32], grid: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GRID, realm, grid])
}

/// Canonical resolution-record address and bump.
///
/// Exactly one resolution record per market: a second address for the same
/// market would be a second place a payout could be decided.
pub fn resolution_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION, market])
}

/// Canonical epoch address and bump.
///
/// The epoch index rather than the epoch identity is the seed, because
/// `clutch_solana_layout::canonical_epoch_id` already derives the identity from
/// exactly `(market, index)`.  Seeding on the index keeps the address derivable
/// by a caller that has not yet fetched the epoch account.
pub fn epoch_pda(program_id: &Pubkey, market: &[u8; 32], epoch_index: u64) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_EPOCH, market, &epoch_index.to_le_bytes()],
    )
}

/// Canonical order-page address and bump.
///
/// The epoch identity already binds the market, so the market is not repeated.
pub fn page_pda(program_id: &Pubkey, epoch: &[u8; 32], page_index: u16) -> (Pubkey, u8) {
    find(program_id, &[SEED_PAGE, epoch, &page_index.to_le_bytes()])
}

/// Canonical candidate-record address and bump.
///
/// A candidate is content-addressed by the digest of its free coordinates, and
/// many candidates compete inside one epoch, so both identities are seeds.
pub fn candidate_pda(program_id: &Pubkey, epoch: &[u8; 32], candidate: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_CANDIDATE, epoch, candidate])
}

/// Canonical final-pot address and bump.
///
/// One pot per epoch.  The pot names the selected candidate in its bytes rather
/// than in its address, so selecting a candidate cannot move the pot.
pub fn pot_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_POT, epoch])
}

/// Canonical settlement-receipt address and bump.
///
/// One receipt per frozen slice of one candidate, so re-settling a slice
/// collides with the existing account instead of minting a second receipt.
pub fn receipt_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
    slice_index: u16,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_RECEIPT, epoch, candidate, &slice_index.to_le_bytes()],
    )
}
