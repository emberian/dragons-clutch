//! The one host-side producer of the family-neutral `HotBumpHintsV1` block.
//!
//! # Why this is a crate and not a helper in each builder
//!
//! `HotBumpHintsV1` is read by Trading, the Dealer accelerator and Custody, and
//! every Hot producer that fills it derives the SAME three slots from the SAME
//! three chain facts: the Core Market state's identity, the Trading capability
//! root's header, and the Market/release-set pair Custody's transfer authority
//! is keyed by. By 2026-09-03 that derivation had been written out three times
//! -- `dclutch_operator::direct_inline_v3`, `dclutch_operator::dealer_lp_hot_v4`
//! and the campaign's `dclutch-chain-bundle-builder` -- and the Rational public
//! outer builders, which are the browser's own path, had not been written at
//! all, so they emitted the all-zero block while the bundle builder beside them
//! emitted three mined bytes. Two host-side builders for one route disagreeing
//! in three of 704 bytes is what `rational_representation_v2_program_test`
//! caught.
//!
//! Three copies is also what makes that class recur: a fourth producer has no
//! function to call, so it either restates the seed walk or emits zeros. This
//! crate is the function to call.
//!
//! # A hint is a memo, never an authority
//!
//! Every byte produced here is fed to a `create_program_address` the READER
//! rebuilds for itself, and the result is compared with the account the frame
//! supplied. A wrong byte names a different address and refuses at an equality
//! that was already there. So nothing in this crate can move a conjunct, and
//! nothing in it needs to be trusted -- see `HotBumpHintsV1`'s own doc for the
//! full argument.
//!
//! # Everything here is TOTAL
//!
//! An unset hint is zero and the reader searches, which is the block's whole
//! contract. A producer that cannot decode a corpus therefore owes a ZERO, not
//! a refusal: `derive_hinted` is total in all three programs that consume these
//! bytes, and a builder that refuses instead inverts the contract and reports
//! an unbuildable bundle where the truth is a slower one. That inversion was
//! measured on 2026-09-03 and cost a lane a masked defect
//! (`series_pre_market_expiry_program_test`, where the staged Market predates
//! its own `CoreState` and the decode legitimately fails).

use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{HOT_BUMP_HINT_COUNT_V1, HOT_BUMP_HINTS_OFFSET_V1, HotBumpHintsV1},
};
use dclutch_custody::CustodyAuthoritySeedsV1;
use dclutch_market::{CoreState, MarketCoreStateSeedsV2};
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::release_set::ExecutionRoleV1;
use solana_program::pubkey::Pubkey;

/// The eight slots in canonical order, named as `HotBumpHintsV1` declares them.
///
/// A test that finds two producers disagreeing at envelope offset 127 has
/// located a byte; a test that says `HotBumpHintsV1::child_relay` has located a
/// DERIVATION. Every assertion that wants the second has so far spelled the
/// list itself -- `waist.rs` in the Direct program-test support, and the
/// TypeScript miner's own `HOT_BUMP_HINT_SLOT_NAMES_V1` in both browser
/// copies -- so the producer names them here once and the copies can be
/// withdrawn onto it.
pub const HOT_BUMP_HINT_SLOT_NAMES_V1: [&str; HOT_BUMP_HINT_COUNT_V1] = [
    "market",
    "root",
    "lifecycle[0]",
    "lifecycle[1]",
    "child_caller[0]",
    "child_caller[1]",
    "child_relay[0]",
    "child_relay[1]",
];

/// Name the hint slot a HOT ENVELOPE offset lands in, if it lands in one.
///
/// `None` for every offset outside the block, so a caller may ask this of any
/// offset in a Hot instruction and get an answer only where there is one.
#[must_use]
pub fn hot_bump_hint_slot_name_v1(envelope_offset: usize) -> Option<&'static str> {
    HOT_BUMP_HINT_SLOT_NAMES_V1
        .get(envelope_offset.checked_sub(HOT_BUMP_HINTS_OFFSET_V1)?)
        .copied()
}

/// The chain facts a Hot producer already holds, in the shape the miner reads.
///
/// Every field is something the producer must have to name the Hot frame at
/// all: the Market and root account bodies are coordinates 0 and 1 of the fixed
/// frame, the Core and Trading program keys are coordinates 23 and 25, and the
/// release set is the envelope's own field. Nothing here is fetched for the
/// sake of mining.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotBumpCorpusV1<'a> {
    /// Address of the Core Market state account in the fixed frame.
    pub market_key: Pubkey,
    /// Complete current bytes of that account, as the frame supplies them.
    pub market_data: &'a [u8],
    /// Complete current bytes of the Trading capability root account.
    pub root_data: &'a [u8],
    /// Core program selected by the Market's release set.
    pub core_program: Pubkey,
    /// Trading program selected by the Market's release set.
    pub trading_program: Pubkey,
    /// Custody program selected by the Market's release set.
    ///
    /// `None` where the producer has not authenticated a Custody deployment.
    /// The transfer-authority slot then stays zero and Custody searches, which
    /// is correct and merely slower. [`activated_custody_program_v1`] reads it
    /// out of the Market activation cache for producers that carry one.
    pub custody_program: Option<Pubkey>,
    /// Immutable execution release set the Market selected.
    pub release_set: [u8; 32],
}

/// Mine the three family-neutral slots every Hot route pays for.
///
/// # Which slots are filled, and which are deliberately left searching
///
/// * `market` -- the Core Market state PDA, whose seeds are the immutable
///   identity the account itself carries.
/// * `root` -- the Trading capability root PDA, whose seeds are the header the
///   root account itself carries.
/// * `child_relay[1]` -- Custody's transfer authority, whose seeds are the
///   Market address and the release set. That pair is the same for every
///   Custody leg of every family, which is what makes ONE slot correct for a
///   route that drives two of them.
///
/// The other five are not family-neutral and cannot be mined from this corpus.
/// `child_relay[0]` is Custody's replay cursor, whose seeds end in the
/// projected child request's replay context; `child_caller` seeds end in a
/// digest over a request projected ON chain, so nothing off chain holds their
/// preimage at all; `lifecycle` is the family's created accounts in
/// materialization order. A producer that DOES project its children -- Direct's
/// `direct_inline_hot_bump_hints_v1` is the only one today -- fills those on
/// top of this result, in its own file, because their seeds are that family's.
#[must_use]
pub fn mine_hot_bump_hints_v1(corpus: &HotBumpCorpusV1<'_>) -> HotBumpHintsV1 {
    HotBumpHintsV1 {
        market: market_state_bump_v1(corpus).unwrap_or_default(),
        root: capability_root_bump_v1(corpus).unwrap_or_default(),
        child_relay: [
            0,
            custody_transfer_authority_bump_v1(corpus).unwrap_or_default(),
        ],
        ..HotBumpHintsV1::ABSENT
    }
}

/// The Core Market state PDA bump, re-derived from the identity it carries.
///
/// `None` where the supplied bytes are not a `CoreState` -- a Market staged
/// before it exists, most often -- and the reader then searches.
#[must_use]
pub fn market_state_bump_v1(corpus: &HotBumpCorpusV1<'_>) -> Option<u8> {
    let market = CoreState::decode(corpus.market_data).ok()?;
    Some(
        Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
            &corpus.core_program,
        )
        .1,
    )
}

/// The Trading capability root PDA bump, re-derived from its own header.
///
/// `None` where the supplied bytes are shorter than the header or do not decode
/// as one, and the reader then searches.
#[must_use]
pub fn capability_root_bump_v1(corpus: &HotBumpCorpusV1<'_>) -> Option<u8> {
    let header =
        CapabilityRootHeaderV1::decode(corpus.root_data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)?)
            .ok()?;
    Some(Pubkey::find_program_address(&header.seeds().as_slices(), &corpus.trading_program).1)
}

/// Custody's transfer-authority PDA bump for this Market and release set.
///
/// `None` where the producer named no Custody deployment; the seeds themselves
/// are read out of no account body, so this is the one slot whose only failure
/// mode is a missing program identity.
#[must_use]
pub fn custody_transfer_authority_bump_v1(corpus: &HotBumpCorpusV1<'_>) -> Option<u8> {
    let custody_program = corpus.custody_program?;
    Some(
        Pubkey::find_program_address(
            &CustodyAuthoritySeedsV1::new(corpus.market_key.to_bytes(), corpus.release_set)
                .as_slices(),
            &custody_program,
        )
        .1,
    )
}

/// Read the release set's Custody deployment out of a Market activation cache.
///
/// Custody is not in the Hot fixed frame; the Market's activation cache is
/// (coordinate 22), and it names the deployment every role in the release set
/// resolved to. Total, like everything else here: an undecodable cache yields
/// `None`, `custody_program` stays unset, and Custody searches.
#[must_use]
pub fn activated_custody_program_v1(activation_cache_data: &[u8]) -> Option<Pubkey> {
    Some(Pubkey::new_from_array(
        ActivatedExecutionReleaseSetViewV1::decode(activation_cache_data)
            .ok()?
            .role(ExecutionRoleV1::Custody)
            .ok()?
            .release()
            .program()
            .to_bytes(),
    ))
}

#[cfg(test)]
mod tests;
