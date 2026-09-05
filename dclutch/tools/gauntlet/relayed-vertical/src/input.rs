//! The twin's relayed market input: a shim over the shared compiler.
//!
//! The graph compiler moved to the successor tree
//! (`tools/local-validator/bootstrap/successor/src/relayed.rs`) so the
//! external driver's `graduation-market` subcommand and this campaign answer
//! from ONE author. This shim supplies the TWIN's venue facts — the
//! synthetic-of-real set `twin.rs` documents field by field — and keeps this
//! campaign's call sites unchanged.

#[path = "../../../local-validator/bootstrap/successor/src/relayed.rs"]
mod relayed;

use dclutch_source::relay::decode::RelayedObservableV1;
use dclutch_source::relay::release::AccountSetEntryV1;
use solana_sdk::pubkey::Pubkey;

use relayed::RelayedVenueFactsV1;
pub(crate) use relayed::{
    DISCLOSED_FAILURE_CONFLATION, RelayedMarketFactsV1, WALK_BOUNTY_LAMPORTS, WindowChoiceV1,
    window_choice,
};

use crate::Result;
use crate::direct_market::DirectMarketCompilerInputV1;
use crate::twin;

/// The twin's venue facts, exactly as `twin.rs` states them.
fn twin_venue_facts() -> RelayedVenueFactsV1 {
    RelayedVenueFactsV1 {
        program: twin::DBC_PROGRAM,
        programdata: twin::DBC_PROGRAMDATA,
        pool: twin::pool_address(),
        elf_digest: twin::synthetic_elf_digest(),
        deployment_slot: twin::SYNTHETIC_DEPLOYMENT_SLOT,
        upgrade_authority: twin::upgrade_authority(),
    }
}

/// The twin exercises row 0; a row-1 twin would supply its own mint facts.
pub(crate) const TWIN_OBSERVABLE: RelayedObservableV1 =
    RelayedObservableV1::DbcMigrationProgressV1;

pub(crate) fn account_set_entries() -> [AccountSetEntryV1; 4] {
    relayed::account_set_entries(TWIN_OBSERVABLE, &twin_venue_facts())
}

pub(crate) fn relayed_market_input(
    registry: Pubkey,
    relayer_pubkey: [u8; 32],
    window_choice: &WindowChoiceV1,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<RelayedMarketFactsV1> {
    relayed::relayed_market_input(
        registry,
        relayer_pubkey,
        window_choice,
        TWIN_OBSERVABLE,
        &twin_venue_facts(),
        direct,
    )
}
