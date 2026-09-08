//! Hot SDK boundary for the shared authenticated Series runtime reader.

use super::frame::{AuthenticatedChildProgramsV3, HotFrameV3};
use crate::series::runtime_registers_v1::{
    SeriesRuntimeBankContextV1, SeriesRuntimeChildProgramsV1, seed_series_derived_scalars_v1,
};
use dclutch_product::svm_reader::AuthenticatedProductRuntimeV3;
use dclutch_trading::series::{SERIES_SUCCESSOR_KIND_PREIMAGE_V3, request::SeriesActionV3};
use dclutch_vm::account_profile::AccountObservationV1;
use solana_program::{
    clock::Clock, hash::hash, program_error::ProgramError, pubkey::Pubkey, rent::Rent,
    sysvar::Sysvar,
};

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn seed_authenticated_series_derived_scalars_v1(
    selected_kind: [u8; 32],
    action: u32,
    family_request: &[u8],
    program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    child_programs: Option<AuthenticatedChildProgramsV3>,
    observations: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
) -> Result<bool, ProgramError> {
    let now_slot = if selected_kind == hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes()
        && action <= SeriesActionV3::Expire as u32
    {
        Clock::get()?.slot
    } else {
        0
    };
    seed_authenticated_series_derived_scalars_at_slot_v1(
        selected_kind,
        action,
        family_request,
        program_id,
        frame,
        product,
        rent,
        child_programs,
        observations,
        scalars,
        now_slot,
    )
}

// Tests call the same SDK-boundary body with a deterministic authenticated slot.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn seed_authenticated_series_derived_scalars_at_slot_v1(
    selected_kind: [u8; 32],
    action: u32,
    family_request: &[u8],
    program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    child_programs: Option<AuthenticatedChildProgramsV3>,
    observations: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
    now_slot: u64,
) -> Result<bool, ProgramError> {
    if selected_kind != hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes() {
        return Ok(false);
    }
    let config = frame.config_raw.try_borrow_data()?;
    let context = SeriesRuntimeBankContextV1 {
        trading_program: *program_id,
        core_program: *frame.core_program.key,
        registry_program: *frame.registry.key,
        parent_root: *frame.root.key,
        config_body: &config,
        child_programs: child_programs.map(|programs| SeriesRuntimeChildProgramsV1 {
            custody: programs.custody,
            claims: programs.claims,
        }),
    };
    seed_series_derived_scalars_v1(
        &context,
        selected_kind,
        action,
        family_request,
        product,
        rent,
        observations,
        scalars,
        now_slot,
    )
}

#[cfg(test)]
#[path = "series_registers_tests.rs"]
mod tests;
