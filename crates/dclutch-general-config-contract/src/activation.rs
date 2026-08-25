//! Canonical General activation request carried behind a Core effect envelope.
//!
//! Core authenticates capability admission and signs this request. General is
//! the sole owner of the shared capability
//! [`FundingStateV1`](dclutch_capability_contract::FundingStateV1) transition
//! and the mutable General tail of one composite Trading root. The request
//! selects a canonical manifest entry rather than copying its 528-byte quote
//! into a second DTO.

use crate::{GENERAL_CONFIG_BYTES_V2, GENERAL_ROOT_BYTES_V2};
use dclutch_capability_contract::{CAPABILITY_ENTRY_BYTES, FUNDING_STATE_BYTES};

/// Exact Lean-owned byte width of [`GeneralActivationRequestV2`].
pub const GENERAL_ACTIVATION_REQUEST_BYTES_V2: usize =
    crate::generated::GENERAL_ACTIVATION_REQUEST_BYTES_V2;
/// Domain label for the Lean-owned V2 activation-request layout.
pub const GENERAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/general-activation-request-v2";
/// SHA-256 of [`GENERAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V2`].
pub const GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V2: [u8; 32] = [
    0xf4, 0x95, 0x67, 0x46, 0x23, 0x4d, 0x58, 0x71, 0xe4, 0xa7, 0xe1, 0x20, 0x54, 0x83, 0x5b, 0x82,
    0x26, 0x7e, 0x0d, 0x1b, 0xfd, 0x17, 0x2d, 0x4d, 0x64, 0x5a, 0x3b, 0x2c, 0x4d, 0xb7, 0xf5, 0xa8,
];

/// Explicit refusal from hostile General activation request bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationRequestError {
    /// The request had another byte width.
    InvalidLength,
    /// Magic, version, or action selected another route.
    UnsupportedSchema,
    /// Reserved bytes were nonzero.
    NonCanonicalReservedBytes,
    /// A composite root, config, manifest, funding, or RentCredit identity was zero.
    ZeroIdentity,
    /// Exact root or funding-state Rent was zero.
    ZeroRent,
    /// The request selected another canonical layout.
    LayoutMismatch,
}

/// Result alias for General activation request operations.
pub type ActivationRequestResult<T> = core::result::Result<T, ActivationRequestError>;

/// Exact General-owned request behind one Core `ActivateCapability` envelope.
///
/// Market, generation, release set, replay context, and Core authority remain
/// solely in the Core envelope. The manifest account is content-addressed by
/// `manifest_id`; `entry_index` selects the exact shared quote semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationRequestV2 {
    capability_root: [u8; 32],
    config_id: [u8; 32],
    manifest_id: [u8; 32],
    funding_state: [u8; 32],
    rent_credit: [u8; 32],
    entry_index: u16,
    current_slot: u64,
    exact_root_rent_lamports: u64,
    exact_funding_rent_lamports: u64,
}

impl GeneralActivationRequestV2 {
    /// Validate and construct one exact activation request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        capability_root: [u8; 32],
        config_id: [u8; 32],
        manifest_id: [u8; 32],
        funding_state: [u8; 32],
        rent_credit: [u8; 32],
        entry_index: u16,
        current_slot: u64,
        exact_root_rent_lamports: u64,
        exact_funding_rent_lamports: u64,
    ) -> ActivationRequestResult<Self> {
        if [
            capability_root,
            config_id,
            manifest_id,
            funding_state,
            rent_credit,
        ]
            .iter()
            .any(is_zero)
        {
            return Err(ActivationRequestError::ZeroIdentity);
        }
        if exact_root_rent_lamports == 0 || exact_funding_rent_lamports == 0 {
            return Err(ActivationRequestError::ZeroRent);
        }
        Ok(Self {
            capability_root,
            config_id,
            manifest_id,
            funding_state,
            rent_credit,
            entry_index,
            current_slot,
            exact_root_rent_lamports,
            exact_funding_rent_lamports,
        })
    }

    /// Hostile-decode one exact Lean-owned activation request.
    pub fn decode(bytes: &[u8]) -> ActivationRequestResult<Self> {
        if bytes.len() != GENERAL_ACTIVATION_REQUEST_BYTES_V2 {
            return Err(ActivationRequestError::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != crate::generated::GENERAL_ACTIVATION_MAGIC_V2
            || read_u16(bytes, crate::generated::ACTIVATION_VERSION_OFFSET)?
                != crate::generated::ABI_VERSION_V2
            || read_u8(bytes, crate::generated::ACTIVATION_ACTION_OFFSET)?
                != crate::generated::GENERAL_ACTIVATION_ACTION_V2
        {
            return Err(ActivationRequestError::UnsupportedSchema);
        }
        require_zero(
            bytes,
            crate::generated::ACTIVATION_RESERVED_HEADER_OFFSET,
            5,
        )?;
        require_zero(bytes, crate::generated::ACTIVATION_RESERVED_ENTRY_OFFSET, 6)?;
        require_zero(bytes, crate::generated::ACTIVATION_RESERVED_TAIL_OFFSET, 32)?;
        if read_usize(
            bytes,
            crate::generated::ACTIVATION_ROOT_STATE_BYTES_OFFSET,
        )?
            != GENERAL_ROOT_BYTES_V2
            || read_usize(bytes, crate::generated::ACTIVATION_CONFIG_BYTES_OFFSET)?
                != GENERAL_CONFIG_BYTES_V2
            || read_usize(bytes, crate::generated::ACTIVATION_FUNDING_BYTES_OFFSET)?
                != FUNDING_STATE_BYTES
            || read_usize(
                bytes,
                crate::generated::ACTIVATION_SELECTED_ENTRY_BYTES_OFFSET,
            )? != CAPABILITY_ENTRY_BYTES
        {
            return Err(ActivationRequestError::LayoutMismatch);
        }
        Self::new(
            read_array(bytes, crate::generated::ACTIVATION_CAPABILITY_ROOT_OFFSET)?,
            read_array(bytes, crate::generated::ACTIVATION_CONFIG_ID_OFFSET)?,
            read_array(bytes, crate::generated::ACTIVATION_MANIFEST_ID_OFFSET)?,
            read_array(bytes, crate::generated::ACTIVATION_FUNDING_STATE_OFFSET)?,
            read_array(bytes, crate::generated::ACTIVATION_RENT_CREDIT_OFFSET)?,
            read_u16(bytes, crate::generated::ACTIVATION_ENTRY_INDEX_OFFSET)?,
            read_u64(bytes, crate::generated::ACTIVATION_CURRENT_SLOT_OFFSET)?,
            read_u64(
                bytes,
                crate::generated::ACTIVATION_EXACT_ROOT_RENT_LAMPORTS_OFFSET,
            )?,
            read_u64(
                bytes,
                crate::generated::ACTIVATION_EXACT_FUNDING_RENT_LAMPORTS_OFFSET,
            )?,
        )
    }

    /// Encode the exact canonical request preimage.
    #[must_use]
    pub fn to_bytes(self) -> [u8; GENERAL_ACTIVATION_REQUEST_BYTES_V2] {
        let mut output = [0_u8; GENERAL_ACTIVATION_REQUEST_BYTES_V2];
        put(
            &mut output,
            0,
            &crate::generated::GENERAL_ACTIVATION_MAGIC_V2,
        );
        put(
            &mut output,
            crate::generated::ACTIVATION_VERSION_OFFSET,
            &crate::generated::ABI_VERSION_V2.to_le_bytes(),
        );
        put(
            &mut output,
            crate::generated::ACTIVATION_ACTION_OFFSET,
            &[crate::generated::GENERAL_ACTIVATION_ACTION_V2],
        );
        for (offset, value) in [
            (
                crate::generated::ACTIVATION_CAPABILITY_ROOT_OFFSET,
                self.capability_root,
            ),
            (
                crate::generated::ACTIVATION_CONFIG_ID_OFFSET,
                self.config_id,
            ),
            (
                crate::generated::ACTIVATION_MANIFEST_ID_OFFSET,
                self.manifest_id,
            ),
            (
                crate::generated::ACTIVATION_FUNDING_STATE_OFFSET,
                self.funding_state,
            ),
            (
                crate::generated::ACTIVATION_RENT_CREDIT_OFFSET,
                self.rent_credit,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        put(
            &mut output,
            crate::generated::ACTIVATION_ENTRY_INDEX_OFFSET,
            &self.entry_index.to_le_bytes(),
        );
        for (offset, value) in [
            (
                crate::generated::ACTIVATION_CURRENT_SLOT_OFFSET,
                self.current_slot,
            ),
            (
                crate::generated::ACTIVATION_EXACT_ROOT_RENT_LAMPORTS_OFFSET,
                self.exact_root_rent_lamports,
            ),
            (
                crate::generated::ACTIVATION_EXACT_FUNDING_RENT_LAMPORTS_OFFSET,
                self.exact_funding_rent_lamports,
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        for (offset, value) in [
            (
                crate::generated::ACTIVATION_ROOT_STATE_BYTES_OFFSET,
                width_u32(GENERAL_ROOT_BYTES_V2),
            ),
            (
                crate::generated::ACTIVATION_CONFIG_BYTES_OFFSET,
                width_u32(GENERAL_CONFIG_BYTES_V2),
            ),
            (
                crate::generated::ACTIVATION_FUNDING_BYTES_OFFSET,
                width_u32(FUNDING_STATE_BYTES),
            ),
            (
                crate::generated::ACTIVATION_SELECTED_ENTRY_BYTES_OFFSET,
                width_u32(CAPABILITY_ENTRY_BYTES),
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    /// Exact composite Trading capability-root PDA selected by the request.
    #[must_use]
    pub const fn capability_root(self) -> [u8; 32] {
        self.capability_root
    }
    /// SHA-256 identity of the exact immutable General config bytes.
    #[must_use]
    pub const fn config_id(self) -> [u8; 32] {
        self.config_id
    }
    /// SHA-256 identity of the exact canonical capability manifest bytes.
    #[must_use]
    pub const fn manifest_id(self) -> [u8; 32] {
        self.manifest_id
    }
    /// Canonical General-owned capability funding-state PDA.
    #[must_use]
    pub const fn funding_state(self) -> [u8; 32] {
        self.funding_state
    }
    /// Core-authenticated RentCredit destination for displaced Rent and dust.
    #[must_use]
    pub const fn rent_credit(self) -> [u8; 32] {
        self.rent_credit
    }
    /// Exact selected entry in the authenticated manifest.
    #[must_use]
    pub const fn entry_index(self) -> u16 {
        self.entry_index
    }
    /// Core-observed slot at which General performs the funding transition.
    #[must_use]
    pub const fn current_slot(self) -> u64 {
        self.current_slot
    }
    /// Exact Rent retained by the entire common-header plus General-tail root.
    #[must_use]
    pub const fn exact_root_rent_lamports(self) -> u64 {
        self.exact_root_rent_lamports
    }
    /// Exact Rent retained by the General funding-state account.
    #[must_use]
    pub const fn exact_funding_rent_lamports(self) -> u64 {
        self.exact_funding_rent_lamports
    }
}

fn width_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or_default()
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn read_u8(input: &[u8], offset: usize) -> ActivationRequestResult<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(ActivationRequestError::InvalidLength)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> ActivationRequestResult<[u8; N]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(ActivationRequestError::InvalidLength)?,
        )
        .ok_or(ActivationRequestError::InvalidLength)?
        .try_into()
        .map_err(|_| ActivationRequestError::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> ActivationRequestResult<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> ActivationRequestResult<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> ActivationRequestResult<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_usize(input: &[u8], offset: usize) -> ActivationRequestResult<usize> {
    usize::try_from(read_u32(input, offset)?).map_err(|_| ActivationRequestError::LayoutMismatch)
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> ActivationRequestResult<()> {
    if input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(ActivationRequestError::InvalidLength)?,
        )
        .ok_or(ActivationRequestError::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(ActivationRequestError::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    let Some(end) = offset.checked_add(value.len()) else {
        return;
    };
    let Some(target) = output.get_mut(offset..end) else {
        return;
    };
    target.copy_from_slice(value);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use sha2::{Digest, Sha256};
    use std::vec::Vec;

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }

    fn request() -> GeneralActivationRequestV2 {
        GeneralActivationRequestV2::new(
            id(0x55),
            id(0x66),
            id(0x77),
            id(0x88),
            id(0x99),
            3,
            44,
            100,
            200,
        )
        .unwrap_or_else(|error| panic!("activation request fixture: {error:?}"))
    }

    #[test]
    fn lean_fixture_round_trips_exactly() {
        let request = request();
        assert_eq!(
            request.to_bytes(),
            crate::generated::GENERAL_ACTIVATION_EXAMPLE_V2
        );
        assert_eq!(
            GeneralActivationRequestV2::decode(&request.to_bytes()),
            Ok(request)
        );
        assert_eq!(
            Sha256::digest(GENERAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V2).as_slice(),
            GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V2
        );
    }

    #[test]
    fn hostile_width_headers_reserved_identities_rent_and_layout_refuse() {
        let canonical = request().to_bytes();
        for width in 0..GENERAL_ACTIVATION_REQUEST_BYTES_V2 {
            assert_eq!(
                GeneralActivationRequestV2::decode(&canonical[..width]),
                Err(ActivationRequestError::InvalidLength)
            );
        }
        let mut extended = Vec::from(canonical);
        extended.push(0);
        assert_eq!(
            GeneralActivationRequestV2::decode(&extended),
            Err(ActivationRequestError::InvalidLength)
        );
        for (offset, expected) in [
            (0, ActivationRequestError::UnsupportedSchema),
            (
                crate::generated::ACTIVATION_VERSION_OFFSET,
                ActivationRequestError::UnsupportedSchema,
            ),
            (
                crate::generated::ACTIVATION_ACTION_OFFSET,
                ActivationRequestError::UnsupportedSchema,
            ),
            (
                crate::generated::ACTIVATION_RESERVED_HEADER_OFFSET,
                ActivationRequestError::NonCanonicalReservedBytes,
            ),
            (
                crate::generated::ACTIVATION_RESERVED_ENTRY_OFFSET,
                ActivationRequestError::NonCanonicalReservedBytes,
            ),
            (
                crate::generated::ACTIVATION_RESERVED_TAIL_OFFSET,
                ActivationRequestError::NonCanonicalReservedBytes,
            ),
        ] {
            let mut hostile = canonical;
            hostile[offset] ^= 1;
            assert_eq!(GeneralActivationRequestV2::decode(&hostile), Err(expected));
        }
        for offset in [
            crate::generated::ACTIVATION_CAPABILITY_ROOT_OFFSET,
            crate::generated::ACTIVATION_CONFIG_ID_OFFSET,
            crate::generated::ACTIVATION_MANIFEST_ID_OFFSET,
            crate::generated::ACTIVATION_FUNDING_STATE_OFFSET,
            crate::generated::ACTIVATION_RENT_CREDIT_OFFSET,
        ] {
            let mut hostile = canonical;
            hostile[offset..offset + 32].fill(0);
            assert_eq!(
                GeneralActivationRequestV2::decode(&hostile),
                Err(ActivationRequestError::ZeroIdentity)
            );
        }
        for offset in [
            crate::generated::ACTIVATION_EXACT_ROOT_RENT_LAMPORTS_OFFSET,
            crate::generated::ACTIVATION_EXACT_FUNDING_RENT_LAMPORTS_OFFSET,
        ] {
            let mut hostile = canonical;
            hostile[offset..offset + 8].fill(0);
            assert_eq!(
                GeneralActivationRequestV2::decode(&hostile),
                Err(ActivationRequestError::ZeroRent)
            );
        }
        for offset in [
            crate::generated::ACTIVATION_ROOT_STATE_BYTES_OFFSET,
            crate::generated::ACTIVATION_CONFIG_BYTES_OFFSET,
            crate::generated::ACTIVATION_FUNDING_BYTES_OFFSET,
            crate::generated::ACTIVATION_SELECTED_ENTRY_BYTES_OFFSET,
        ] {
            let mut hostile = canonical;
            hostile[offset] ^= 1;
            assert_eq!(
                GeneralActivationRequestV2::decode(&hostile),
                Err(ActivationRequestError::LayoutMismatch)
            );
        }
    }
}
