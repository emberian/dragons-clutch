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
/// Per-order funded-reservation account seed prefix.
pub const SEED_RESERVATION: &[u8] = b"dragons-clutch:reservation:v1";
/// Candidate-record account seed prefix.
pub const SEED_CANDIDATE: &[u8] = b"dragons-clutch:candidate:v1";
/// Candidate-feed account seed prefix.
///
/// The feed is the stable byte carrier for one candidate's fills and pairing
/// slices.  Content equality alone is not account authority, so settlement
/// authenticates exactly one feed address per `(epoch, candidate)`.
pub const SEED_CANDIDATE_FEED: &[u8] = b"dragons-clutch:cand-feed:v1";

/// Clearing-checkpoint (`ClearWorkAccount`) seed prefix; 28 bytes.
pub const SEED_CLEAR_WORK: &[u8] = b"dragons-clutch:clear-work:v1";
/// General-epoch deadline-window (`EpochWindowAccount`) seed prefix; 30 bytes.
pub const SEED_EPOCH_WINDOW: &[u8] = b"dragons-clutch:epoch-window:v1";
/// Final-pot account seed prefix.
pub const SEED_POT: &[u8] = b"dragons-clutch:pot:v1";
/// Settlement-receipt account seed prefix.
pub const SEED_RECEIPT: &[u8] = b"dragons-clutch:receipt:v1";
/// General funding-ledger sibling seed prefix, keyed by the funded account's
/// own address so one machinery account has exactly one recorded funding.
pub const SEED_GENERAL_FUNDING: &[u8] = b"dc:gen-funding:v1";
/// Uploader-keyed typed artifact staging account seed prefix.
pub const SEED_ARTIFACT_STAGE: &[u8] = b"dragons-clutch:upload:v1";
/// Canonical raw collateral-policy artifact seed prefix.
pub const SEED_POLICY: &[u8] = b"dragons-clutch:policy:v1";
/// Canonical full-width batch-policy artifact seed prefix.
pub const SEED_BATCH_POLICY: &[u8] = b"dragons-clutch:batch-policy:v1";
/// DirectBatchPolicy V3 final-artifact seed prefix, disjoint from legacy policy.
pub const SEED_DIRECT_BATCH_POLICY_V3: &[u8] = b"dc:direct-policy:v3";
/// Direct candidate-window account seed prefix.
pub const SEED_DIRECT_WINDOW: &[u8] = b"dragons-clutch:direct-window:v1";
/// Full-width verified direct candidate seed prefix.
pub const SEED_DIRECT_CANDIDATE: &[u8] = b"dragons-clutch:dir-candidate:v2";
/// Direct settlement-receipt seed prefix.
pub const SEED_DIRECT_RECEIPT: &[u8] = b"dragons-clutch:direct-receipt:v2";
/// Direct zero-pot seed prefix.
pub const SEED_DIRECT_POT: &[u8] = b"dragons-clutch:direct-pot:v2";
/// Direct V3 candidate-window seed prefix, disjoint from every V1/V2 address.
pub const SEED_DIRECT_WINDOW_V3: &[u8] = b"dc:direct-window:v3";
/// Direct V3 verified-candidate seed prefix, disjoint from every V1/V2 address.
pub const SEED_DIRECT_CANDIDATE_V3: &[u8] = b"dc:direct-candidate:v3";
/// Direct V3 finite WorkBudget seed prefix.
pub const SEED_DIRECT_WORK_V3: &[u8] = b"dc:direct-work:v3";
/// Direct V3 settlement-receipt seed prefix, disjoint from V2 receipts.
pub const SEED_DIRECT_RECEIPT_V3: &[u8] = b"dc:direct-receipt:v3";
/// Direct V3 zero-pot seed prefix, disjoint from V2 pots.
pub const SEED_DIRECT_POT_V3: &[u8] = b"dc:direct-pot:v3";
/// Immutable authenticated source-spec account seed prefix.
pub const SEED_SOURCE_SPEC: &[u8] = crate::source_archive::SOURCE_SPEC_SEED_V1;
/// Per-window authenticated source-archive account seed prefix.
pub const SEED_SOURCE_ARCHIVE: &[u8] = crate::source_archive::SOURCE_ARCHIVE_SEED_V1;
/// One deterministic active ResolutionWork lock per Market.
pub const SEED_RESOLUTION_WORK: &[u8] = b"resolution-work-v1";
/// Program-owned prepaid Reserve bound to one deterministic Work PDA.
pub const SEED_RESOLUTION_RESERVE: &[u8] = b"resolution-reserve-v1";

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

/// Canonical immutable source-spec address and bump.
pub fn source_spec_pda(program_id: &Pubkey, feed: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SOURCE_SPEC, feed])
}

/// Canonical sealed source-archive address and bump for one exact window.
pub fn source_archive_pda(program_id: &Pubkey, feed: &[u8; 32], window: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SOURCE_ARCHIVE, feed, window])
}

/// Canonical single-active-work address for one Market.
pub fn resolution_work_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION_WORK, market])
}

/// Canonical prepaid Reserve for one Market and its deterministic Work PDA.
pub fn resolution_reserve_pda(
    program_id: &Pubkey,
    market: &[u8; 32],
    work: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION_RESERVE, market, work])
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

/// Canonical raw collateral-policy address and bump.
///
/// `profile` is the parent Profile identity recomputed from the policy digest.
/// The raw policy codec has no stored bump; callers authenticate this address
/// and recompute both identities from the exact bytes.
pub fn policy_pda(program_id: &Pubkey, profile: &[u8; 32], digest: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_POLICY, profile, digest])
}

/// Canonical full-width batch-policy artifact address.
pub fn batch_policy_pda(program_id: &Pubkey, epoch: &[u8; 32], digest: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_BATCH_POLICY, epoch, digest])
}

/// Canonical DirectBatchPolicy V3 artifact address.
pub fn direct_batch_policy_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    digest: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_BATCH_POLICY_V3, epoch, digest])
}

/// Canonical direct candidate-window address.
pub fn direct_window_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_WINDOW, epoch])
}

/// Canonical full-width verified direct-candidate address.
pub fn direct_candidate_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_CANDIDATE, epoch, candidate])
}

/// Canonical direct receipt address for the sole direct slice.
pub fn direct_receipt_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_DIRECT_RECEIPT, epoch, candidate, &0u16.to_le_bytes()],
    )
}

/// Canonical closed direct-pot address.
pub fn direct_pot_pda(program_id: &Pubkey, epoch: &[u8; 32], candidate: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_POT, epoch, candidate])
}

/// Canonical Direct V3 candidate-window address.
pub fn direct_window_v3_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_WINDOW_V3, epoch])
}

/// Canonical Direct V3 verified-candidate address.
pub fn direct_candidate_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_CANDIDATE_V3, epoch, candidate])
}

/// Canonical finite WorkBudget address for one Direct V3 Epoch.
pub fn direct_work_v3_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_WORK_V3, epoch])
}

/// Canonical Direct V3 receipt address for the sole selected slice.
pub fn direct_receipt_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_DIRECT_RECEIPT_V3,
            epoch,
            candidate,
            &0u16.to_le_bytes(),
        ],
    )
}

/// Canonical Direct V3 zero-pot address for one selected Candidate.
pub fn direct_pot_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_POT_V3, epoch, candidate])
}

/// Canonical uploader-scoped staging address and bump.
///
/// Uploader scoping keeps an abandoned partial body from poisoning the one
/// final content-derived address shared by all uploaders. The stage header
/// re-binds every seed component before admitting a write, seal, or abort.
pub fn artifact_stage_pda(
    program_id: &Pubkey,
    uploader: &[u8; 32],
    kind: &[u8; 1],
    context: &[u8; 32],
    digest: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_ARTIFACT_STAGE, uploader, kind, context, digest],
    )
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

/// Canonical general-epoch deadline-window address and bump.
///
/// The same seed tuple as [`epoch_pda`] under its own prefix: one window per
/// epoch, derivable by a caller that has not yet fetched either account.
pub fn epoch_window_pda(program_id: &Pubkey, market: &[u8; 32], epoch_index: u64) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_EPOCH_WINDOW, market, &epoch_index.to_le_bytes()],
    )
}

/// Canonical per-order reservation address and bump.
///
/// The layout-owned reservation digest already commits to market, epoch,
/// owner, Position generation, and order id. Seeding on that one fixed-width
/// identity keeps the PDA tuple short without truncating any coordinate.
pub fn reservation_pda(program_id: &Pubkey, reservation: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESERVATION, reservation])
}

/// Canonical candidate-record address and bump.
///
/// A candidate is content-addressed by the digest of its free coordinates, and
/// many candidates compete inside one epoch, so both identities are seeds.
pub fn candidate_pda(program_id: &Pubkey, epoch: &[u8; 32], candidate: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_CANDIDATE, epoch, candidate])
}

/// Canonical candidate-feed address.
pub fn candidate_feed_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_CANDIDATE_FEED, epoch, candidate])
}

/// Canonical clearing-checkpoint address and bump.
///
/// Exactly one resumable checkpoint per `(epoch, candidate)` — the same tuple
/// that addresses the candidate record and its feed, because the checkpoint
/// verifies exactly one candidate against one frozen epoch.  The market is
/// bound through the epoch identity rather than repeated, matching
/// [`candidate_pda`].  A PDA rather than a keypair address is the design
/// decision of `docs/implementation/SOLANA_LAYOUT.md`'s staged-creation
/// analysis: a keypair-addressed checkpoint is substitutable, a PDA at this
/// tuple is not.
pub fn clear_work_pda(program_id: &Pubkey, epoch: &[u8; 32], candidate: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_CLEAR_WORK, epoch, candidate])
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

/// Canonical general funding-ledger address and bump.
///
/// Keyed by the funded account's own address: exactly one recorded funding
/// per machinery account, and a close route can re-derive its ledger from the
/// account it is closing with no other input.
pub fn general_funding_pda(program_id: &Pubkey, target: &Pubkey) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_FUNDING, &target.to_bytes()])
}

/* ------------------------------------------------------------------------ */
/* Token plane — PROPOSED appends                                            */
/* ------------------------------------------------------------------------ */

/* `docs/implementation/TOKEN2022_PLAN.md` §3.2 records that this module "is
 * missing every token address" and proposes exactly three.  They are appended
 * here, unfrozen like every other seed above, with one correction the plan
 * could not make because it never derived an address: **the proposed
 * `"dragons-clutch:hoard-authority:v1"` is 33 bytes and cannot be a seed.**  A
 * single seed is capped at 32 bytes, so `find_program_address` refuses it
 * outright — on-chain that is a panic, not a refusal code.  The prefix is
 * therefore `"dragons-clutch:hoard-auth:v1"` (28 bytes).  The other two are the
 * plan's, unaltered, at 30 and 29 bytes.
 *
 * The Hoard *authority* and the Hoard *token account* stay two addresses, for
 * the reason the plan gives: collapsing them makes the signing seeds and the
 * account seeds the same bytes. */

/// Outcome-mint seed prefix; 30 bytes.
pub const SEED_OUTCOME_MINT: &[u8] = b"dragons-clutch:outcome-mint:v1";
/// Hoard signing-authority seed prefix; 28 bytes.
///
/// Shortened from the plan's `hoard-authority`, which does not fit a seed.
pub const SEED_HOARD_AUTHORITY: &[u8] = b"dragons-clutch:hoard-auth:v1";
/// Hoard token-account seed prefix; 29 bytes.
pub const SEED_HOARD_TOKEN: &[u8] = b"dragons-clutch:hoard-token:v1";

/// Canonical outcome-mint address and bump.
///
/// One mint per `(market, outcome index)`.  The index rather than the outcome
/// identity is the seed, for the same reason [`epoch_pda`] seeds on the index:
/// the address stays derivable by a caller that has not fetched the market
/// account, and `MarketAccount::outcomes` already binds index to identity.
pub fn outcome_mint_pda(program_id: &Pubkey, market: &[u8; 32], outcome_index: u8) -> (Pubkey, u8) {
    find(program_id, &[SEED_OUTCOME_MINT, market, &[outcome_index]])
}

/// Canonical Hoard signing-authority address and bump.
///
/// This is the address the program signs *as* to move collateral out of the
/// Hoard token account.  It holds no data and is never a state account.
pub fn hoard_authority_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD_AUTHORITY, market])
}

/// Canonical Hoard token-account address and bump.
///
/// The Token-2022 account [`hoard_authority_pda`] owns.  Distinct from
/// [`hoard_pda`], which is this program's own collateral-accounting state.
pub fn hoard_token_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD_TOKEN, market])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seed longer than 32 bytes is not a refusal code, it is a panic inside
    /// `find_program_address`.  Every prefix this module exports is checked
    /// here rather than by inspection, which is how the plan's proposed
    /// `hoard-authority` prefix was caught at 33 bytes.
    #[test]
    fn every_seed_prefix_fits_one_seed() {
        const PREFIXES: [&[u8]; 42] = [
            SEED_EPOCH_WINDOW,
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_CLEAR_WORK,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        for prefix in PREFIXES {
            assert!(
                prefix.len() <= 32,
                "seed prefix {:?} is {} bytes and cannot be a seed",
                core::str::from_utf8(prefix).unwrap_or("<non-utf8>"),
                prefix.len()
            );
        }
        assert_eq!(SEED_HOARD_AUTHORITY.len(), 28);
        assert_eq!(SEED_OUTCOME_MINT.len(), 30);
        assert_eq!(SEED_HOARD_TOKEN.len(), 29);
        assert_eq!(SEED_BATCH_POLICY.len(), 30);
        assert_eq!(SEED_DIRECT_BATCH_POLICY_V3.len(), 19);
        assert_eq!(SEED_DIRECT_WINDOW.len(), 31);
        assert_eq!(SEED_DIRECT_CANDIDATE.len(), 31);
        assert_eq!(SEED_DIRECT_RECEIPT.len(), 32);
        assert_eq!(SEED_DIRECT_POT.len(), 28);
        assert_eq!(SEED_DIRECT_WINDOW_V3.len(), 19);
        assert_eq!(SEED_DIRECT_CANDIDATE_V3.len(), 22);
        assert_eq!(SEED_DIRECT_WORK_V3.len(), 17);
        assert_eq!(SEED_DIRECT_RECEIPT_V3.len(), 20);
        assert_eq!(SEED_DIRECT_POT_V3.len(), 16);
        assert_eq!(SEED_CLEAR_WORK.len(), 28);
        assert_eq!(SEED_EPOCH_WINDOW.len(), 30);
        // The plan's own proposal, kept here as the falsifier: it does not fit.
        assert_eq!(b"dragons-clutch:hoard-authority:v1".len(), 33);
    }

    /// The three token prefixes are distinct from each other and from every
    /// prefix that was already here: a shared prefix is a shared address space.
    #[test]
    fn the_token_prefixes_collide_with_nothing() {
        const EXISTING: [&[u8]; 20] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_POT,
            SEED_RECEIPT,
        ];
        const APPENDED: [&[u8]; 3] = [SEED_OUTCOME_MINT, SEED_HOARD_AUTHORITY, SEED_HOARD_TOKEN];
        for added in APPENDED {
            for old in EXISTING {
                assert_ne!(added, old);
            }
        }
        assert_ne!(APPENDED[0], APPENDED[1]);
        assert_ne!(APPENDED[0], APPENDED[2]);
        assert_ne!(APPENDED[1], APPENDED[2]);
        /* `SEED_HOARD` is a strict prefix of `SEED_HOARD_TOKEN` as a *string*,
         * which is harmless: seeds are length-delimited arguments, not a
         * concatenation, so `["dragons-clutch:hoard:v1", m]` and
         * `["dragons-clutch:hoard-token:v1", m]` are different tuples. */
        assert!(SEED_HOARD_TOKEN.starts_with(b"dragons-clutch:hoard"));
    }

    /// Every Direct V3 namespace is disjoint from every historical direct and
    /// generic namespace. Account versions therefore cannot be squatted by a
    /// valid older PDA carrying the same semantic identity.
    #[test]
    fn direct_v3_prefixes_are_pairwise_disjoint_from_the_full_registry() {
        const HISTORICAL: [&[u8]; 35] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        const DIRECT_V3: [&[u8]; 5] = [
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
        ];
        for (index, added) in DIRECT_V3.iter().enumerate() {
            for old in HISTORICAL {
                assert_ne!(*added, old);
            }
            for later in DIRECT_V3.iter().skip(index + 1) {
                assert_ne!(*added, *later);
            }
        }
    }

    /// The clearing-checkpoint prefix shares an address space with nothing.
    ///
    /// `SEED_CANDIDATE_FEED` and `SEED_CANDIDATE` take the *same* seed tuple
    /// `(epoch, candidate)` that `SEED_CLEAR_WORK` takes, so prefix
    /// distinctness is exactly what keeps the three accounts three addresses.
    #[test]
    fn the_clear_work_prefix_collides_with_nothing() {
        const REGISTRY: [&[u8]; 40] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        for old in REGISTRY {
            assert_ne!(SEED_CLEAR_WORK, old);
        }
    }

    /// The window prefix shares an address space with nothing.
    ///
    /// `SEED_EPOCH` takes the *same* seed tuple `(market, epoch_index)` the
    /// window takes, so prefix distinctness is exactly what keeps the epoch
    /// and its deadline window two addresses.
    #[test]
    fn the_epoch_window_prefix_collides_with_nothing() {
        const REGISTRY: [&[u8]; 41] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_CLEAR_WORK,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        for old in REGISTRY {
            assert_ne!(SEED_EPOCH_WINDOW, old);
        }
    }
}
