//! Shared physical-account expansion for dynamic-span AccountProfile execution.
//!
//! The finalized Profile13 artifact remains the sole owner of logical width,
//! physical representatives, and route-local privileges. This adapter only
//! materializes those checked relations as Solana `AccountInfo` views; it does
//! not infer aliases, privileges, or a runtime span from the account slice.

use std::vec::Vec;

use dclutch_account_profile_contract::v2::AccountProfileV2;
use solana_program::{account_info::AccountInfo, program_error::ProgramError};

use crate::TradingSbfError;

/// Expand one exact physical representative slice into the Profile13 logical
/// vector while preserving each representative's outer union privileges.
pub(crate) fn expand_dynamic_physical_accounts_v4<'accounts, 'info>(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    physical_accounts: &[&'accounts AccountInfo<'info>],
) -> Result<Vec<&'accounts AccountInfo<'info>>, ProgramError> {
    let logical_count = profile
        .logical_account_count_with_dynamic_spans(tail_count, span_counts)
        .map_err(|_| TradingSbfError::Content)?;
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(tail_count, span_counts)
        .map_err(|_| TradingSbfError::Content)?;
    if physical_accounts.len() != physical_count {
        return Err(TradingSbfError::Content.into());
    }
    let mut logical = Vec::new();
    logical
        .try_reserve_exact(logical_count)
        .map_err(|_| TradingSbfError::Content)?;
    let mut coordinate = 0_usize;
    while coordinate < logical_count {
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(tail_count, span_counts, coordinate)
            .map_err(|_| TradingSbfError::Content)?;
        logical.push(
            *physical_accounts
                .get(ordinal)
                .ok_or(TradingSbfError::Content)?,
        );
        coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(logical)
}

/// Clone the complete logical vector into route-local child views.
///
/// Signer and writable bits are downgraded to the exact logical-coordinate
/// declaration. Executability must already agree with the physical
/// representative and cannot be upgraded or suppressed by this adapter.
pub(crate) fn downgrade_dynamic_child_accounts_v4<'info>(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    logical_accounts: &[&AccountInfo<'info>],
) -> Result<Vec<AccountInfo<'info>>, ProgramError> {
    let logical_count = profile
        .logical_account_count_with_dynamic_spans(tail_count, span_counts)
        .map_err(|_| TradingSbfError::Content)?;
    if logical_accounts.len() != logical_count {
        return Err(TradingSbfError::Content.into());
    }
    let mut downgraded = Vec::new();
    downgraded
        .try_reserve_exact(logical_count)
        .map_err(|_| TradingSbfError::Content)?;
    for (coordinate, physical) in logical_accounts.iter().enumerate() {
        let privileges = profile
            .route_privileges_with_dynamic_spans(tail_count, span_counts, coordinate)
            .map_err(|_| TradingSbfError::Content)?;
        if physical.executable != privileges.executable() {
            return Err(TradingSbfError::Content.into());
        }
        let mut logical = (*physical).clone();
        logical.is_signer = privileges.signer();
        logical.is_writable = privileges.writable();
        downgraded.push(logical);
    }
    Ok(downgraded)
}

#[cfg(all(test, any(feature = "families", feature = "series-family")))]
mod tests {
    extern crate alloc;

    use alloc::{boxed::Box, vec, vec::Vec};

    use dclutch_account_profile_contract::v2::AccountProfileV2;
    use solana_program::pubkey::Pubkey;

    use super::*;
    use crate::series::account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SeriesConsumeAccountProfileInputV4,
        encode_series_consume_account_profile_v4_atomic,
    };

    fn profile_bytes() -> Vec<u8> {
        let lengths = [0_u32; 157];
        let mut scratch = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        encode_series_consume_account_profile_v4_atomic(
            SeriesConsumeAccountProfileInputV4 {
                fixed_data_lengths: &lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("Series Profile13");
        output
    }

    fn account(writable: bool, executable: bool) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(Vec::new().into_boxed_slice());
        AccountInfo::new(key, false, writable, lamports, data, owner, executable)
    }

    fn physical_accounts(
        profile: AccountProfileV2<'_>,
        funding_count: u32,
    ) -> Vec<AccountInfo<'static>> {
        let span_counts = [funding_count];
        let physical_count = profile
            .physical_account_count_with_dynamic_spans(0, &span_counts)
            .expect("physical count");
        let mut physical = Vec::with_capacity(physical_count);
        let mut ordinal = 0_usize;
        while ordinal < physical_count {
            let representative = profile
                .physical_representative_coordinate_with_dynamic_spans(0, &span_counts, ordinal)
                .expect("representative");
            let executable = profile
                .route_privileges_with_dynamic_spans(0, &span_counts, representative)
                .expect("representative privileges")
                .executable();
            let writable = representative == 53;
            physical.push(account(writable, executable));
            ordinal += 1;
        }
        physical
    }

    #[test]
    fn physical_representatives_expand_once_and_children_are_downgraded() {
        let bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let span_counts = [1_u32];
        let physical = physical_accounts(profile, span_counts[0]);
        let physical_refs = physical.iter().collect::<Vec<_>>();
        assert_eq!(physical.len(), 65);
        let logical = expand_dynamic_physical_accounts_v4(profile, 0, &span_counts, &physical_refs)
            .expect("logical expansion");
        assert_eq!(logical.len(), 158);
        assert_eq!(logical[18].key, logical[20].key);
        assert_eq!(logical[53].key, logical[137].key);
        assert!(logical[53].is_writable);

        let child = downgrade_dynamic_child_accounts_v4(profile, 0, &span_counts, &logical)
            .expect("child views");
        assert!(!child[18].is_writable);
        assert!(child[20].is_writable);
        assert!(!child[53].is_writable);
        assert!(!child[137].is_writable);
        assert_eq!(child[18].key, child[20].key);
    }

    #[test]
    fn physical_width_and_executable_substitution_refuse() {
        let bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let span_counts = [1_u32];
        let mut physical = physical_accounts(profile, span_counts[0]);
        physical.pop();
        let physical_refs = physical.iter().collect::<Vec<_>>();
        assert!(
            expand_dynamic_physical_accounts_v4(profile, 0, &span_counts, &physical_refs).is_err()
        );

        let mut physical = physical_accounts(profile, span_counts[0]);
        let executable_coordinate = 8_usize;
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(0, &span_counts, executable_coordinate)
            .expect("executable ordinal");
        physical[ordinal].executable = false;
        let physical_refs = physical.iter().collect::<Vec<_>>();
        let logical = expand_dynamic_physical_accounts_v4(profile, 0, &span_counts, &physical_refs)
            .expect("logical expansion");
        assert!(downgrade_dynamic_child_accounts_v4(profile, 0, &span_counts, &logical).is_err());
    }
}
