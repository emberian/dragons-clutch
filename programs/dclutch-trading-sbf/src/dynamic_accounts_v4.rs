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
    // One forward sweep, not one prefix recount per coordinate.
    //
    // `physical_account_ordinal_with_dynamic_spans` recounts self-
    // representatives from zero for every coordinate, so resolving the whole
    // vector one call at a time re-decodes O(n^2) rules. An authenticated route
    // alias always names a strictly earlier self-representative, whose physical
    // account this sweep has already resolved, so the alias simply repeats it.
    // The alias relation is rechecked rather than assumed: a coordinate naming
    // a later representative, or one that is not itself a self-representative,
    // refuses.
    let packs = profile.supports_route_alias_packing();
    let mut logical = Vec::new();
    logical
        .try_reserve_exact(logical_count)
        .map_err(|_| TradingSbfError::Content)?;
    let mut next = 0_usize;
    let mut coordinate = 0_usize;
    while coordinate < logical_count {
        let representative = profile
            .representative_with_dynamic_spans(tail_count, span_counts, coordinate)
            .map_err(|_| TradingSbfError::Content)?;
        let resolved = if !packs {
            *physical_accounts
                .get(coordinate)
                .ok_or(TradingSbfError::Content)?
        } else if representative == coordinate {
            let resolved = *physical_accounts
                .get(next)
                .ok_or(TradingSbfError::Content)?;
            next = next.checked_add(1).ok_or(TradingSbfError::Content)?;
            resolved
        } else {
            if representative >= coordinate
                || profile
                    .representative_with_dynamic_spans(tail_count, span_counts, representative)
                    .map_err(|_| TradingSbfError::Content)?
                    != representative
            {
                return Err(TradingSbfError::Content.into());
            }
            *logical
                .get(representative)
                .ok_or(TradingSbfError::Content)?
        };
        logical.push(resolved);
        coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(logical)
}

/// Clone the complete logical vector into route-local child views.
///
/// Every privilege in a child view comes from the representative coordinate --
/// the semantic owner of the physical account's authenticated declaration --
/// and never from an authenticated route alias, which the AccountProfile
/// validator requires to be emitted privilege-free and therefore states
/// nothing at all. A declaration is never turned into a meta the transaction
/// did not grant, and executability is exact in both directions.
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
        let representative = profile
            .representative_with_dynamic_spans(tail_count, span_counts, coordinate)
            .map_err(|_| TradingSbfError::Content)?;
        let declared = profile
            .route_privileges_with_dynamic_spans(tail_count, span_counts, representative)
            .map_err(|_| TradingSbfError::Content)?;
        if (declared.writable() && !physical.is_writable)
            || declared.executable() != physical.executable
        {
            return Err(TradingSbfError::Content.into());
        }
        let mut logical = (*physical).clone();
        logical.is_signer = declared.signer();
        logical.is_writable = declared.writable();
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
            let declared = profile
                .route_privileges_with_dynamic_spans(0, &span_counts, representative)
                .expect("representative privileges");
            // A frame the chain would actually present: every coordinate the
            // profile declares writable is included writable, exactly as
            // `validate_accounts` requires. Coordinate 53 is additionally
            // writable without declaring it, which is the legitimate
            // effect-permission case a route view must NOT upgrade.
            let writable = declared.writable() || representative == 53;
            physical.push(account(writable, declared.executable()));
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
        // 18 is the Shared Market representative and owns every privilege fact
        // about the physical account. 20 is an authenticated route alias of it:
        // an alias is emitted privilege-free, so it states nothing, and a child
        // CPI meta built from the alias would silently hand the child program a
        // readonly view of an account the representative declares writable.
        assert!(child[18].is_writable);
        assert!(child[20].is_writable);
        assert_eq!(child[18].key, child[20].key);
        // 53 and its alias 137 are readonly at the representative, so both
        // child views stay readonly: inheriting from the representative is not
        // a blanket upgrade.
        assert!(!child[53].is_writable);
        assert!(!child[137].is_writable);
    }

    /// A route view must never claim a privilege the transaction did not grant,
    /// even when the authenticated declaration states one.
    #[test]
    fn a_declared_privilege_the_transaction_withheld_refuses() {
        let bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let span_counts = [1_u32];
        let mut physical = physical_accounts(profile, span_counts[0]);
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(0, &span_counts, 18)
            .expect("shared market ordinal");
        assert!(physical[ordinal].is_writable, "representative is writable");
        physical[ordinal].is_writable = false;
        let physical_refs = physical.iter().collect::<Vec<_>>();
        let logical = expand_dynamic_physical_accounts_v4(profile, 0, &span_counts, &physical_refs)
            .expect("logical expansion");
        assert!(downgrade_dynamic_child_accounts_v4(profile, 0, &span_counts, &logical).is_err());
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
