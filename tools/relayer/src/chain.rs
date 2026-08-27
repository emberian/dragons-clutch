//! Foreign-cluster facts the daemon must know to *carry* bytes, and nothing it
//! would need to *interpret* them.
//!
//! §4.1 is a dependency-closure property, but it is also a source-file
//! property: the only layouts named here are the Loader V3 `ProgramData`
//! metadata prefix and the `Clock` sysvar, both of which are first-party
//! runtime structures rather than venue state.
//!
//! - The `ProgramData` prefix is read for exactly one field, `deployment_slot`,
//!   and that field is used for exactly one purpose: to key the tail-digest
//!   cache and to refuse the set when it changes.  Its *value* is never
//!   compared, thresholded, or reported as a fact — it goes into the signed
//!   message only as part of the release-pinned 45-byte inline prefix, byte for
//!   byte as read.
//! - The `Clock` sysvar's `unix_timestamp` offset is named only by
//!   `measure-skew`, a read-only measurement subcommand that signs nothing.
//!   The observation loop attests the `Clock` account like any other account
//!   and leaves the decoding to the on-devnet adapter (§4.2).

use crate::id32::ID_BYTES;

/// The upgradeable loader, whose `ProgramData` accounts carry the 45-byte
/// metadata prefix whose tail digest *is* the registry contract's `elf_digest`.
pub const LOADER_V3_PROGRAM_ID_BASE58: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

/// Bytes of [`LOADER_V3_PROGRAM_ID_BASE58`].
pub const LOADER_V3_PROGRAM_ID: [u8; ID_BYTES] = [
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61, 22,
    193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
];

/// The `Clock` sysvar address on every cluster.
pub const CLOCK_SYSVAR_ID_BASE58: &str = "SysvarC1ock11111111111111111111111111111111";

/// Bytes of [`CLOCK_SYSVAR_ID_BASE58`].
pub const CLOCK_SYSVAR_ID: [u8; ID_BYTES] = [
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
];

/// Exact width of the `Clock` sysvar account.
pub const CLOCK_ACCOUNT_BYTES: usize = 40;

/// Offset of `Clock.unix_timestamp`, an `i64` LE.
///
/// `Clock` is `slot u64 | epoch_start_timestamp i64 | epoch u64 |
/// leader_schedule_epoch u64 | unix_timestamp i64`.
pub const CLOCK_UNIX_TIMESTAMP_OFFSET: usize = 32;

/// The `UpgradeableLoaderState::ProgramData` enum discriminant, a `u32` LE at
/// offset 0.
pub const LOADER_V3_PROGRAMDATA_DISCRIMINANT: u32 = 3;

/// Offset of `deployment_slot` inside the `ProgramData` metadata prefix.
pub const LOADER_V3_DEPLOYMENT_SLOT_OFFSET: usize = 4;

/// Decode `Clock.unix_timestamp` from an exactly 40-byte `Clock` account.
pub fn clock_unix_timestamp(account_data: &[u8]) -> Option<i64> {
    if account_data.len() != CLOCK_ACCOUNT_BYTES {
        return None;
    }
    let raw: [u8; 8] = account_data
        .get(CLOCK_UNIX_TIMESTAMP_OFFSET..CLOCK_UNIX_TIMESTAMP_OFFSET + 8)?
        .try_into()
        .ok()?;
    Some(i64::from_le_bytes(raw))
}

/// Read `deployment_slot` out of a Loader V3 `ProgramData` metadata prefix.
///
/// Returns `None` when the prefix is not a `ProgramData` variant at all, which
/// is a refusal rather than a fallback: an account at a pinned `ProgramData`
/// position that is some other loader variant is exactly the substitution this
/// family exists to refuse.
pub fn programdata_deployment_slot(prefix: &[u8]) -> Option<u64> {
    let discriminant: [u8; 4] = prefix.get(..4)?.try_into().ok()?;
    if u32::from_le_bytes(discriminant) != LOADER_V3_PROGRAMDATA_DISCRIMINANT {
        return None;
    }
    let raw: [u8; 8] = prefix
        .get(LOADER_V3_DEPLOYMENT_SLOT_OFFSET..LOADER_V3_DEPLOYMENT_SLOT_OFFSET + 8)?
        .try_into()
        .ok()?;
    Some(u64::from_le_bytes(raw))
}

/// Whether a pinned position is a Loader V3 `ProgramData` position.
///
/// This is the *only* position kind whose tail digest may be cached, because it
/// is the only one with a cache key that a redeploy is guaranteed to change:
/// `deployment_slot` sits inside the 45-byte prefix the daemon re-reads every
/// cycle, so a stale cached digest is unreachable.
pub fn is_loader_v3_programdata(expected_owner: &[u8; ID_BYTES], inline_len: u16) -> bool {
    *expected_owner == LOADER_V3_PROGRAM_ID
        && usize::from(inline_len)
            == dclutch_relay_contract::LOADER_V3_PROGRAMDATA_METADATA_BYTES_V1
}

/// The Instructions sysvar, used only to select the preceding precompile.
pub const INSTRUCTIONS_SYSVAR_ID_BASE58: &str = "Sysvar1nstructions1111111111111111111111111";

/// Bytes of [`INSTRUCTIONS_SYSVAR_ID_BASE58`].
pub const INSTRUCTIONS_SYSVAR_ID: [u8; ID_BYTES] = [
    6, 167, 213, 23, 24, 123, 209, 102, 53, 218, 212, 4, 85, 253, 194, 192, 193, 36, 198, 143, 33,
    86, 117, 165, 219, 186, 203, 95, 8, 0, 0, 0,
];

/// The Rent sysvar.
pub const RENT_SYSVAR_ID_BASE58: &str = "SysvarRent111111111111111111111111111111111";

/// Bytes of [`RENT_SYSVAR_ID_BASE58`].
pub const RENT_SYSVAR_ID: [u8; ID_BYTES] = [
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
];

/// The ComputeBudget program.
pub const COMPUTE_BUDGET_PROGRAM_ID_BASE58: &str = "ComputeBudget111111111111111111111111111111";

/// Bytes of [`COMPUTE_BUDGET_PROGRAM_ID_BASE58`].
pub const COMPUTE_BUDGET_PROGRAM_ID: [u8; ID_BYTES] = [
    3, 6, 70, 111, 229, 33, 23, 50, 255, 236, 173, 186, 114, 195, 155, 231, 188, 140, 229, 187,
    197, 247, 18, 107, 44, 67, 155, 58, 64, 0, 0, 0,
];

/// `ComputeBudgetInstruction::SetComputeUnitLimit` discriminant.
pub const COMPUTE_BUDGET_SET_UNIT_LIMIT_TAG: u8 = 2;

/// `ComputeBudgetInstruction::SetComputeUnitPrice` discriminant.
pub const COMPUTE_BUDGET_SET_UNIT_PRICE_TAG: u8 = 3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id32::base58;

    #[test]
    fn the_pinned_program_ids_are_their_base58_spellings() {
        assert_eq!(base58(&LOADER_V3_PROGRAM_ID), LOADER_V3_PROGRAM_ID_BASE58);
        assert_eq!(base58(&CLOCK_SYSVAR_ID), CLOCK_SYSVAR_ID_BASE58);
    }

    #[test]
    fn the_metadata_prefix_width_is_the_wire_crates_pinned_width() {
        assert_eq!(
            dclutch_relay_contract::LOADER_V3_PROGRAMDATA_METADATA_BYTES_V1,
            45
        );
        assert_eq!(
            dclutch_relay_contract::MAINNET_CLOCK_SYSVAR_BYTES_V1,
            CLOCK_ACCOUNT_BYTES
        );
    }

    #[test]
    fn the_pinned_sysvar_and_program_ids_are_their_base58_spellings() {
        assert_eq!(
            base58(&INSTRUCTIONS_SYSVAR_ID),
            INSTRUCTIONS_SYSVAR_ID_BASE58
        );
        assert_eq!(base58(&RENT_SYSVAR_ID), RENT_SYSVAR_ID_BASE58);
        assert_eq!(
            base58(&COMPUTE_BUDGET_PROGRAM_ID),
            COMPUTE_BUDGET_PROGRAM_ID_BASE58
        );
        // The wire crate pins the Ed25519 program independently; the two
        // spellings must be the same 32 bytes or the precompile this daemon
        // builds would be addressed to nothing.
        assert_eq!(
            base58(&dclutch_relay_contract::signature::ED25519_PROGRAM_ID_3_0),
            "Ed25519SigVerify111111111111111111111111111"
        );
    }

    #[test]
    fn a_programdata_prefix_yields_its_deployment_slot() {
        let mut prefix = [0u8; 45];
        prefix[..4].copy_from_slice(&3u32.to_le_bytes());
        prefix[4..12].copy_from_slice(&360_000_001u64.to_le_bytes());
        prefix[12] = 1;
        assert_eq!(programdata_deployment_slot(&prefix), Some(360_000_001));
    }

    #[test]
    fn another_loader_variant_at_a_programdata_position_refuses() {
        let mut prefix = [0u8; 45];
        prefix[..4].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(programdata_deployment_slot(&prefix), None);
        assert_eq!(programdata_deployment_slot(&[]), None);
        assert_eq!(programdata_deployment_slot(&[3, 0, 0, 0]), None);
    }

    #[test]
    fn the_clock_timestamp_is_read_from_a_full_width_account_only() {
        let mut clock = [0u8; CLOCK_ACCOUNT_BYTES];
        clock[CLOCK_UNIX_TIMESTAMP_OFFSET..].copy_from_slice(&1_772_000_000i64.to_le_bytes());
        assert_eq!(clock_unix_timestamp(&clock), Some(1_772_000_000));
        assert_eq!(clock_unix_timestamp(&clock[..39]), None);
    }

    #[test]
    fn only_a_loader_v3_position_at_45_bytes_is_cacheable() {
        assert!(is_loader_v3_programdata(&LOADER_V3_PROGRAM_ID, 45));
        assert!(!is_loader_v3_programdata(&LOADER_V3_PROGRAM_ID, 44));
        assert!(!is_loader_v3_programdata(&[9u8; ID_BYTES], 45));
    }
}
