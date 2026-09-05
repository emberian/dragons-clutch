//! Durable two-step activation and permissionless terminal close wires.

use dclutch_sha256_adapter::digestv;

use crate::resolution::{
    Error, RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2, ResolutionCoreActionV1,
    ResolutionCoreReceiptKindV1, ResolutionRoleRequestV2,
};

/// Direct Pending-to-Active funding request magic.
pub const FUNDING_ACTIVATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLRFAQ1";
/// Exact direct activation request width.
pub const FUNDING_ACTIVATION_REQUEST_BYTES_V1: usize = 440;
/// Durable activation receipt magic.
pub const FUNDING_ACTIVATION_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLRFAR1";
/// Exact durable activation receipt width.
pub const FUNDING_ACTIVATION_RECEIPT_BYTES_V1: usize = 576;
/// Permissionless terminal-close request magic.
pub const DIRECT_FUNDING_CLOSE_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLRFCQ1";
/// Exact permissionless terminal-close request width.
pub const DIRECT_FUNDING_CLOSE_REQUEST_BYTES_V1: usize = 472;
/// Resolution-owned activation receipt PDA domain.
pub const FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dclutch/funding-activation/v1";
/// Domain for a complete observed account prestate.
pub const FUNDING_LIFECYCLE_ACCOUNT_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch/funding-lifecycle-account/v1";

const VERSION_V1: u16 = 1;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const GENERATION_OFFSET: usize = 80;
const ROLE_OFFSET: usize = 88;
const ROLE_END: usize = ROLE_OFFSET + RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2;

/// Permissionless activation request over one exact Pending subset ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingActivationRequestV1 {
    /// Activated execution-release-set identity.
    pub release_set: [u8; 32],
    /// Core-owned Market account.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Existing canonical Resolution role coordinates and three-row selection.
    pub role: ResolutionRoleRequestV2,
    /// SHA-256 of exact Core Market bytes before activation.
    pub expected_market_state_digest: [u8; 32],
    /// SHA-256 of exact primary Source-state bytes before activation.
    pub expected_source_state_digest: [u8; 32],
    /// Complete owner/key/lamports/data digest of the Pending ledger.
    pub expected_pending_ledger_digest: [u8; 32],
    /// Canonical Resolution-owned durable activation receipt PDA.
    pub receipt: [u8; 32],
}

impl FundingActivationRequestV1 {
    /// Validate one action-partitioned activation request.
    pub fn validate(self) -> Result<Self, Error> {
        self.role.validate()?;
        if self.generation == 0
            || self.role.action != ResolutionCoreActionV1::VerifyFundReady
            || self.role.receipt_kind != ResolutionCoreReceiptKindV1::None
            || self.role.receipt != [0; 32]
            || required_activation_ids(self).iter().any(is_zero)
        {
            return Err(Error::InvalidReceiptShape);
        }
        Ok(self)
    }

    /// Encode one exact activation request.
    pub fn encode(self) -> Result<[u8; FUNDING_ACTIVATION_REQUEST_BYTES_V1], Error> {
        let value = self.validate()?;
        let mut output = [0_u8; FUNDING_ACTIVATION_REQUEST_BYTES_V1];
        put(&mut output, 0, &FUNDING_ACTIVATION_REQUEST_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        put(&mut output, RELEASE_SET_OFFSET, &value.release_set)?;
        put(&mut output, MARKET_OFFSET, &value.market)?;
        put(
            &mut output,
            GENERATION_OFFSET,
            &value.generation.to_le_bytes(),
        )?;
        put(&mut output, ROLE_OFFSET, &value.role.to_bytes()?)?;
        put(&mut output, ROLE_END, &value.expected_market_state_digest)?;
        put(
            &mut output,
            ROLE_END + 32,
            &value.expected_source_state_digest,
        )?;
        put(
            &mut output,
            ROLE_END + 64,
            &value.expected_pending_ledger_digest,
        )?;
        put(&mut output, ROLE_END + 96, &value.receipt)?;
        Ok(output)
    }

    /// Decode and validate exact activation bytes.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact(
            input,
            FUNDING_ACTIVATION_REQUEST_BYTES_V1,
            &FUNDING_ACTIVATION_REQUEST_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1 || any_nonzero(input, 10, 6)? {
            return Err(Error::NonCanonicalReserved);
        }
        Self {
            release_set: read_array(input, RELEASE_SET_OFFSET)?,
            market: read_array(input, MARKET_OFFSET)?,
            generation: read_u64(input, GENERATION_OFFSET)?,
            role: ResolutionRoleRequestV2::decode(slice(
                input,
                ROLE_OFFSET,
                RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2,
            )?)?,
            expected_market_state_digest: read_array(input, ROLE_END)?,
            expected_source_state_digest: read_array(input, ROLE_END + 32)?,
            expected_pending_ledger_digest: read_array(input, ROLE_END + 64)?,
            receipt: read_array(input, ROLE_END + 96)?,
        }
        .validate()
    }

    /// SHA-256 of the exact canonical request bytes.
    pub fn digest(self) -> Result<[u8; 32], Error> {
        Ok(dclutch_sha256_adapter::digest(&self.encode()?))
    }
}

/// Durable proof that exact V7 code activated one exact Pending ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingActivationReceiptV1 {
    /// SHA-256 of the exact activation request.
    pub request_digest: [u8; 32],
    /// Activated execution-release-set identity.
    pub release_set: [u8; 32],
    /// Resolution semantic release that authored the receipt.
    pub resolution_release: [u8; 32],
    /// Core-owned Market account.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact role coordinates and ordered three-row selection.
    pub role: ResolutionRoleRequestV2,
    /// SHA-256 of exact Core Market bytes observed at activation.
    pub market_state_digest: [u8; 32],
    /// SHA-256 of exact primary Source-state bytes observed at activation.
    pub source_state_digest: [u8; 32],
    /// Complete digest of the Pending ledger account prestate.
    pub pending_ledger_digest: [u8; 32],
    /// Complete digest of the Active ledger account poststate.
    pub active_ledger_digest: [u8; 32],
    /// Slot written into every activated row.
    pub activation_slot: u64,
    /// Exact lamports credited to the immutable Rent beneficiary.
    pub beneficiary_credit_lamports: u64,
    /// Exact Active ledger Rent reserve.
    pub ledger_rent_lamports: u64,
    /// Exact remaining native principal after activation.
    pub remaining_native_principal_lamports: u64,
    /// Exact Active ledger account balance.
    pub post_ledger_lamports: u64,
    /// Resolution program that produced this receipt.
    pub producer: [u8; 32],
}

impl FundingActivationReceiptV1 {
    /// Validate one complete receipt.
    pub fn validate(self) -> Result<Self, Error> {
        self.role.validate()?;
        let classified = self
            .ledger_rent_lamports
            .checked_add(self.remaining_native_principal_lamports)
            .ok_or(Error::InvalidClosureRefund)?;
        if self.generation == 0
            || self.activation_slot == 0
            || self.ledger_rent_lamports == 0
            || classified != self.post_ledger_lamports
            || self.role.action != ResolutionCoreActionV1::VerifyFundReady
            || self.role.receipt_kind != ResolutionCoreReceiptKindV1::None
            || receipt_ids(self).iter().any(is_zero)
        {
            return Err(Error::InvalidReceiptShape);
        }
        Ok(self)
    }

    /// Encode one exact durable receipt.
    pub fn encode(self) -> Result<[u8; FUNDING_ACTIVATION_RECEIPT_BYTES_V1], Error> {
        let value = self.validate()?;
        let mut output = [0_u8; FUNDING_ACTIVATION_RECEIPT_BYTES_V1];
        put(&mut output, 0, &FUNDING_ACTIVATION_RECEIPT_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        put(&mut output, 16, &value.request_digest)?;
        put(&mut output, 48, &value.release_set)?;
        put(&mut output, 80, &value.resolution_release)?;
        put(&mut output, 112, &value.market)?;
        put(&mut output, 144, &value.generation.to_le_bytes())?;
        put(&mut output, 152, &value.role.to_bytes()?)?;
        put(&mut output, 376, &value.market_state_digest)?;
        put(&mut output, 408, &value.source_state_digest)?;
        put(&mut output, 440, &value.pending_ledger_digest)?;
        put(&mut output, 472, &value.active_ledger_digest)?;
        put(&mut output, 504, &value.activation_slot.to_le_bytes())?;
        put(
            &mut output,
            512,
            &value.beneficiary_credit_lamports.to_le_bytes(),
        )?;
        put(&mut output, 520, &value.ledger_rent_lamports.to_le_bytes())?;
        put(
            &mut output,
            528,
            &value.remaining_native_principal_lamports.to_le_bytes(),
        )?;
        put(&mut output, 536, &value.post_ledger_lamports.to_le_bytes())?;
        put(&mut output, 544, &value.producer)?;
        Ok(output)
    }

    /// Decode and validate exact receipt bytes.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact(
            input,
            FUNDING_ACTIVATION_RECEIPT_BYTES_V1,
            &FUNDING_ACTIVATION_RECEIPT_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1 || any_nonzero(input, 10, 6)? {
            return Err(Error::NonCanonicalReserved);
        }
        Self {
            request_digest: read_array(input, 16)?,
            release_set: read_array(input, 48)?,
            resolution_release: read_array(input, 80)?,
            market: read_array(input, 112)?,
            generation: read_u64(input, 144)?,
            role: ResolutionRoleRequestV2::decode(slice(
                input,
                152,
                RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2,
            )?)?,
            market_state_digest: read_array(input, 376)?,
            source_state_digest: read_array(input, 408)?,
            pending_ledger_digest: read_array(input, 440)?,
            active_ledger_digest: read_array(input, 472)?,
            activation_slot: read_u64(input, 504)?,
            beneficiary_credit_lamports: read_u64(input, 512)?,
            ledger_rent_lamports: read_u64(input, 520)?,
            remaining_native_principal_lamports: read_u64(input, 528)?,
            post_ledger_lamports: read_u64(input, 536)?,
            producer: read_array(input, 544)?,
        }
        .validate()
    }
}

/// Permissionless direct close request over one Retiring Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFundingCloseRequestV1 {
    /// Activated execution-release-set identity.
    pub release_set: [u8; 32],
    /// Core-owned Retiring Market.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Existing canonical close coordinates and ordered three-row selection.
    pub role: ResolutionRoleRequestV2,
    /// SHA-256 of exact Core Market bytes.
    pub market_state_digest: [u8; 32],
    /// SHA-256 of exact terminal Source-state bytes.
    pub source_state_digest: [u8; 32],
    /// Complete digest of the Active ledger account.
    pub funding_ledger_digest: [u8; 32],
    /// SHA-256 of exact admitted certificate bytes.
    pub certificate_digest: [u8; 32],
    /// Complete digest of the vacant, prefunded closure destination.
    pub closure_prestate_digest: [u8; 32],
}

impl DirectFundingCloseRequestV1 {
    /// Validate one close-partitioned request.
    pub fn validate(self) -> Result<Self, Error> {
        self.role.validate()?;
        if self.generation == 0
            || self.role.action != ResolutionCoreActionV1::CloseFund
            || self.role.receipt_kind != ResolutionCoreReceiptKindV1::Closure
            || direct_close_ids(self).iter().any(is_zero)
        {
            return Err(Error::InvalidReceiptShape);
        }
        Ok(self)
    }

    /// Encode one exact direct-close request.
    pub fn encode(self) -> Result<[u8; DIRECT_FUNDING_CLOSE_REQUEST_BYTES_V1], Error> {
        let value = self.validate()?;
        let mut output = [0_u8; DIRECT_FUNDING_CLOSE_REQUEST_BYTES_V1];
        put(&mut output, 0, &DIRECT_FUNDING_CLOSE_REQUEST_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        put(&mut output, RELEASE_SET_OFFSET, &value.release_set)?;
        put(&mut output, MARKET_OFFSET, &value.market)?;
        put(
            &mut output,
            GENERATION_OFFSET,
            &value.generation.to_le_bytes(),
        )?;
        put(&mut output, ROLE_OFFSET, &value.role.to_bytes()?)?;
        for (offset, field) in [
            (ROLE_END, value.market_state_digest),
            (ROLE_END + 32, value.source_state_digest),
            (ROLE_END + 64, value.funding_ledger_digest),
            (ROLE_END + 96, value.certificate_digest),
            (ROLE_END + 128, value.closure_prestate_digest),
        ] {
            put(&mut output, offset, &field)?;
        }
        Ok(output)
    }

    /// Decode and validate exact direct-close bytes.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact(
            input,
            DIRECT_FUNDING_CLOSE_REQUEST_BYTES_V1,
            &DIRECT_FUNDING_CLOSE_REQUEST_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1 || any_nonzero(input, 10, 6)? {
            return Err(Error::NonCanonicalReserved);
        }
        Self {
            release_set: read_array(input, RELEASE_SET_OFFSET)?,
            market: read_array(input, MARKET_OFFSET)?,
            generation: read_u64(input, GENERATION_OFFSET)?,
            role: ResolutionRoleRequestV2::decode(slice(
                input,
                ROLE_OFFSET,
                RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2,
            )?)?,
            market_state_digest: read_array(input, ROLE_END)?,
            source_state_digest: read_array(input, ROLE_END + 32)?,
            funding_ledger_digest: read_array(input, ROLE_END + 64)?,
            certificate_digest: read_array(input, ROLE_END + 96)?,
            closure_prestate_digest: read_array(input, ROLE_END + 128)?,
        }
        .validate()
    }

    /// SHA-256 of the exact canonical request bytes.
    pub fn digest(self) -> Result<[u8; 32], Error> {
        Ok(dclutch_sha256_adapter::digest(&self.encode()?))
    }
}

/// Digest owner, key, lamports, and exact data as one account pre/poststate.
pub fn funding_lifecycle_account_digest_v1(
    owner: [u8; 32],
    key: [u8; 32],
    lamports: u64,
    data: &[u8],
) -> [u8; 32] {
    let len = u64::try_from(data.len()).unwrap_or(u64::MAX).to_le_bytes();
    digestv(&[
        FUNDING_LIFECYCLE_ACCOUNT_DIGEST_DOMAIN_V1,
        &owner,
        &key,
        &lamports.to_le_bytes(),
        &len,
        data,
    ])
}

fn required_activation_ids(value: FundingActivationRequestV1) -> [[u8; 32]; 11] {
    [
        value.release_set,
        value.market,
        value.role.source_material,
        value.role.source_state,
        value.role.capability_manifest,
        value.role.funding_ledger,
        value.role.beneficiary,
        value.expected_market_state_digest,
        value.expected_source_state_digest,
        value.expected_pending_ledger_digest,
        value.receipt,
    ]
}

fn receipt_ids(value: FundingActivationReceiptV1) -> [[u8; 32]; 14] {
    [
        value.request_digest,
        value.release_set,
        value.resolution_release,
        value.market,
        value.role.source_material,
        value.role.source_state,
        value.role.capability_manifest,
        value.role.funding_ledger,
        value.role.beneficiary,
        value.market_state_digest,
        value.source_state_digest,
        value.pending_ledger_digest,
        value.active_ledger_digest,
        value.producer,
    ]
}

fn direct_close_ids(value: DirectFundingCloseRequestV1) -> [[u8; 32]; 13] {
    [
        value.release_set,
        value.market,
        value.role.source_material,
        value.role.source_state,
        value.role.capability_manifest,
        value.role.funding_ledger,
        value.role.receipt,
        value.role.beneficiary,
        value.market_state_digest,
        value.source_state_digest,
        value.funding_ledger_digest,
        value.certificate_digest,
        value.closure_prestate_digest,
    ]
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn exact(input: &[u8], width: usize, magic: &[u8; 8]) -> Result<(), Error> {
    if input.len() != width {
        return Err(Error::InvalidLength);
    }
    if input.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    Ok(())
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], Error> {
    input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn any_nonzero(input: &[u8], offset: usize, width: usize) -> Result<bool, Error> {
    Ok(slice(input, offset, width)?.iter().any(|byte| *byte != 0))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

const _: () = assert!(FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1.len() <= 32);

#[cfg(test)]
mod tests {
    use super::*;

    fn role(action: ResolutionCoreActionV1) -> ResolutionRoleRequestV2 {
        let close = action == ResolutionCoreActionV1::CloseFund;
        ResolutionRoleRequestV2 {
            action,
            receipt_kind: if close {
                ResolutionCoreReceiptKindV1::Closure
            } else {
                ResolutionCoreReceiptKindV1::None
            },
            source_state: [1; 32],
            source_material: [2; 32],
            capability_manifest: [3; 32],
            funding_ledger: [4; 32],
            receipt: if close { [5; 32] } else { [0; 32] },
            beneficiary: [6; 32],
            recovery_entry_index: 1,
            exhaustion_entry_index: 4,
            failure_entry_index: 7,
            receipt_sequence: if close { 9 } else { 0 },
        }
    }

    #[test]
    fn activation_request_and_receipt_are_exact_and_nonaliasing() {
        let request = FundingActivationRequestV1 {
            release_set: [7; 32],
            market: [8; 32],
            generation: 11,
            role: role(ResolutionCoreActionV1::VerifyFundReady),
            expected_market_state_digest: [9; 32],
            expected_source_state_digest: [10; 32],
            expected_pending_ledger_digest: [11; 32],
            receipt: [12; 32],
        };
        let bytes = request.encode().expect("activation request");
        assert_eq!(bytes.len(), FUNDING_ACTIVATION_REQUEST_BYTES_V1);
        assert_eq!(FundingActivationRequestV1::decode(&bytes), Ok(request));
        let receipt = FundingActivationReceiptV1 {
            request_digest: request.digest().expect("request digest"),
            release_set: request.release_set,
            resolution_release: [13; 32],
            market: request.market,
            generation: request.generation,
            role: request.role,
            market_state_digest: request.expected_market_state_digest,
            source_state_digest: request.expected_source_state_digest,
            pending_ledger_digest: request.expected_pending_ledger_digest,
            active_ledger_digest: [14; 32],
            activation_slot: 15,
            beneficiary_credit_lamports: 16,
            ledger_rent_lamports: 17,
            remaining_native_principal_lamports: 18,
            post_ledger_lamports: 35,
            producer: [19; 32],
        };
        let receipt_bytes = receipt.encode().expect("activation receipt");
        assert_eq!(receipt_bytes.len(), FUNDING_ACTIVATION_RECEIPT_BYTES_V1);
        assert_eq!(
            FundingActivationReceiptV1::decode(&receipt_bytes),
            Ok(receipt)
        );
        assert_ne!(
            request.digest().expect("request digest"),
            receipt.active_ledger_digest
        );
    }

    #[test]
    fn direct_close_roundtrips_and_reserved_bytes_refuse() {
        let request = DirectFundingCloseRequestV1 {
            release_set: [7; 32],
            market: [8; 32],
            generation: 11,
            role: role(ResolutionCoreActionV1::CloseFund),
            market_state_digest: [9; 32],
            source_state_digest: [10; 32],
            funding_ledger_digest: [11; 32],
            certificate_digest: [12; 32],
            closure_prestate_digest: [13; 32],
        };
        let bytes = request.encode().expect("direct close");
        assert_eq!(bytes.len(), DIRECT_FUNDING_CLOSE_REQUEST_BYTES_V1);
        assert_eq!(DirectFundingCloseRequestV1::decode(&bytes), Ok(request));
        let mut hostile = bytes;
        hostile[10] = 1;
        assert_eq!(
            DirectFundingCloseRequestV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
    }

    #[test]
    fn account_digest_binds_owner_key_balance_length_and_data() {
        let exact = funding_lifecycle_account_digest_v1([1; 32], [2; 32], 3, &[4, 5]);
        assert_ne!(
            exact,
            funding_lifecycle_account_digest_v1([9; 32], [2; 32], 3, &[4, 5])
        );
        assert_ne!(
            exact,
            funding_lifecycle_account_digest_v1([1; 32], [2; 32], 4, &[4, 5])
        );
        assert_ne!(
            exact,
            funding_lifecycle_account_digest_v1([1; 32], [2; 32], 3, &[4, 5, 0])
        );
    }
}
