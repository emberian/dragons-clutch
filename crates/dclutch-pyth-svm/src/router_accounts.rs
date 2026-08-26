//! Exact borrowed views of the pinned Wormhole router accounts used by Pyth.

/// Anchor discriminator for the router `EncodedVaa` account.
pub const ENCODED_VAA_DISCRIMINATOR_V1: [u8; 8] = [0xe2, 0x65, 0xa3, 0x04, 0x85, 0xa0, 0x54, 0xf5];
/// Anchor discriminator for the router `GuardianSet` account.
pub const GUARDIAN_SET_DISCRIMINATOR_V1: [u8; 8] = [0x78, 0x4d, 0x4a, 0x62, 0x22, 0x53, 0x60, 0x7d];
/// Router processing status for a cryptographically verified VAA.
pub const ENCODED_VAA_VERIFIED_STATUS_V1: u8 = 2;
/// Fixed header before the signed VAA body.
pub const ENCODED_VAA_HEADER_BYTES_V1: usize = 41;
/// Serialized GuardianSet header through its vector count.
pub const GUARDIAN_SET_HEADER_BYTES_V1: usize = 16;
/// One Ethereum guardian address width.
pub const GUARDIAN_ADDRESS_BYTES_V1: usize = 20;

/// Stable hostile router-account refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterAccountErrorV1 {
    /// Bytes were truncated or had an impossible exact size.
    InvalidLength,
    /// Anchor discriminator selected another account type.
    InvalidDiscriminator,
    /// Encoded VAA was not in the terminal verified phase.
    NotVerified,
    /// Signed VAA version was not the pinned V1 format.
    UnsupportedVaaVersion,
    /// Guardian count was zero, did not fit V1, or differed from the release.
    InvalidGuardianCount,
    /// Guardian set index in account and VAA differed.
    GuardianSetMismatch,
    /// Reserved allocation tail contained nonzero bytes.
    NonCanonicalTail,
}

/// Borrowed verified router `EncodedVaa` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedEncodedVaaV1<'a> {
    write_authority: [u8; 32],
    guardian_set_index: u32,
    signature_count: u8,
    signed_vaa: &'a [u8],
}

impl<'a> VerifiedEncodedVaaV1<'a> {
    /// Parse a verified account and its exact V1 signed VAA header.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, RouterAccountErrorV1> {
        if bytes.len() < 47 {
            return Err(RouterAccountErrorV1::InvalidLength);
        }
        if array::<8>(bytes, 0)? != ENCODED_VAA_DISCRIMINATOR_V1 {
            return Err(RouterAccountErrorV1::InvalidDiscriminator);
        }
        if byte(bytes, 8)? != ENCODED_VAA_VERIFIED_STATUS_V1 {
            return Err(RouterAccountErrorV1::NotVerified);
        }
        let signed_vaa = bytes
            .get(ENCODED_VAA_HEADER_BYTES_V1..)
            .ok_or(RouterAccountErrorV1::InvalidLength)?;
        if byte(signed_vaa, 0)? != 1 {
            return Err(RouterAccountErrorV1::UnsupportedVaaVersion);
        }
        let signature_count = byte(signed_vaa, 5)?;
        let signature_bytes = usize::from(signature_count)
            .checked_mul(66)
            .and_then(|width| width.checked_add(6))
            .ok_or(RouterAccountErrorV1::InvalidLength)?;
        if signature_count == 0 || signed_vaa.len() <= signature_bytes {
            return Err(RouterAccountErrorV1::InvalidLength);
        }
        Ok(Self {
            write_authority: array(bytes, 9)?,
            guardian_set_index: u32::from_be_bytes(array(signed_vaa, 1)?),
            signature_count,
            signed_vaa,
        })
    }

    /// Authority that initialized and wrote this exact EncodedVaa account.
    pub const fn write_authority(self) -> [u8; 32] {
        self.write_authority
    }
    /// Big-endian guardian set index carried by the signed VAA.
    pub const fn guardian_set_index(self) -> u32 {
        self.guardian_set_index
    }
    /// Number of signatures physically carried by this VAA.
    pub const fn signature_count(self) -> u8 {
        self.signature_count
    }
    /// Exact signed VAA bytes retained by the router account.
    pub const fn signed_vaa(self) -> &'a [u8] {
        self.signed_vaa
    }
}

/// Borrowed canonical router `GuardianSet` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardianSetV1<'a> {
    index: u32,
    guardian_count: u8,
    keys: &'a [u8],
    creation_time: u32,
    expiration_time: u32,
}

impl<'a> GuardianSetV1<'a> {
    /// Parse the pinned Anchor/Borsh GuardianSet layout.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, RouterAccountErrorV1> {
        if bytes.len() < GUARDIAN_SET_HEADER_BYTES_V1 + GUARDIAN_ADDRESS_BYTES_V1 + 8 {
            return Err(RouterAccountErrorV1::InvalidLength);
        }
        if array::<8>(bytes, 0)? != GUARDIAN_SET_DISCRIMINATOR_V1 {
            return Err(RouterAccountErrorV1::InvalidDiscriminator);
        }
        let index = u32::from_le_bytes(array(bytes, 8)?);
        let count = u32::from_le_bytes(array(bytes, 12)?);
        let count =
            usize::try_from(count).map_err(|_| RouterAccountErrorV1::InvalidGuardianCount)?;
        if count == 0 || count > usize::from(u8::MAX) {
            return Err(RouterAccountErrorV1::InvalidGuardianCount);
        }
        let keys_end = count
            .checked_mul(GUARDIAN_ADDRESS_BYTES_V1)
            .and_then(|width| GUARDIAN_SET_HEADER_BYTES_V1.checked_add(width))
            .ok_or(RouterAccountErrorV1::InvalidLength)?;
        let times_end = keys_end
            .checked_add(8)
            .ok_or(RouterAccountErrorV1::InvalidLength)?;
        let keys = bytes
            .get(GUARDIAN_SET_HEADER_BYTES_V1..keys_end)
            .ok_or(RouterAccountErrorV1::InvalidLength)?;
        if bytes
            .get(times_end..)
            .ok_or(RouterAccountErrorV1::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(RouterAccountErrorV1::NonCanonicalTail);
        }
        Ok(Self {
            index,
            guardian_count: u8::try_from(count)
                .map_err(|_| RouterAccountErrorV1::InvalidGuardianCount)?,
            keys,
            creation_time: u32::from_le_bytes(array(bytes, keys_end)?),
            expiration_time: u32::from_le_bytes(array(bytes, keys_end + 4)?),
        })
    }

    /// Guardian set index.
    pub const fn index(self) -> u32 {
        self.index
    }
    /// Number of exact 20-byte guardian addresses.
    pub const fn guardian_count(self) -> u8 {
        self.guardian_count
    }
    /// Guardian set creation timestamp.
    pub const fn creation_time(self) -> u32 {
        self.creation_time
    }
    /// Zero for active set, otherwise expiration timestamp.
    pub const fn expiration_time(self) -> u32 {
        self.expiration_time
    }

    /// Require the signed VAA and release to select this exact guardian set.
    pub fn authenticate(
        self,
        vaa: VerifiedEncodedVaaV1<'_>,
        release_guardian_count: u8,
        release_required_count: u8,
    ) -> Result<(), RouterAccountErrorV1> {
        if self.index != vaa.guardian_set_index() {
            return Err(RouterAccountErrorV1::GuardianSetMismatch);
        }
        if self.guardian_count() != release_guardian_count
            || vaa.signature_count() < release_required_count
            || vaa.signature_count() > release_guardian_count
        {
            return Err(RouterAccountErrorV1::InvalidGuardianCount);
        }
        Ok(())
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8, RouterAccountErrorV1> {
    bytes
        .get(offset)
        .copied()
        .ok_or(RouterAccountErrorV1::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], RouterAccountErrorV1> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(RouterAccountErrorV1::InvalidLength)?,
        )
        .ok_or(RouterAccountErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| RouterAccountErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn verified_vaa_and_guardian_set_join_exactly() {
        let mut encoded = [0_u8; 48 + 66 * 3];
        encoded[..8].copy_from_slice(&ENCODED_VAA_DISCRIMINATOR_V1);
        encoded[8] = 2;
        encoded[9..41].copy_from_slice(&[7; 32]);
        encoded[41] = 1;
        encoded[42..46].copy_from_slice(&9_u32.to_be_bytes());
        encoded[46] = 3;
        let vaa = VerifiedEncodedVaaV1::parse(&encoded).expect("verified VAA");

        let mut guardians = [0_u8; 16 + 20 * 5 + 8];
        guardians[..8].copy_from_slice(&GUARDIAN_SET_DISCRIMINATOR_V1);
        guardians[8..12].copy_from_slice(&9_u32.to_le_bytes());
        guardians[12..16].copy_from_slice(&5_u32.to_le_bytes());
        let set = GuardianSetV1::parse(&guardians).expect("guardian set");
        assert_eq!(set.authenticate(vaa, 5, 3), Ok(()));
        assert_eq!(
            set.authenticate(vaa, 4, 3),
            Err(RouterAccountErrorV1::InvalidGuardianCount)
        );
    }

    #[test]
    fn processing_and_index_substitutions_refuse() {
        let mut encoded = [0_u8; 48 + 66];
        encoded[..8].copy_from_slice(&ENCODED_VAA_DISCRIMINATOR_V1);
        encoded[8] = 1;
        encoded[41] = 1;
        encoded[46] = 1;
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&encoded),
            Err(RouterAccountErrorV1::NotVerified)
        );
        encoded[8] = 2;
        let mut guardians = [0_u8; 16 + 20 + 8];
        guardians[..8].copy_from_slice(&GUARDIAN_SET_DISCRIMINATOR_V1);
        guardians[8..12].copy_from_slice(&1_u32.to_le_bytes());
        guardians[12..16].copy_from_slice(&1_u32.to_le_bytes());
        let vaa = VerifiedEncodedVaaV1::parse(&encoded).expect("verified VAA");
        assert_eq!(
            GuardianSetV1::parse(&guardians)
                .expect("guardian set")
                .authenticate(vaa, 1, 1),
            Err(RouterAccountErrorV1::GuardianSetMismatch)
        );
    }

    #[test]
    fn captured_signed_vaa_has_the_pinned_real_quorum_shape() {
        const SIGNED: &[u8] =
            include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
        let mut account = std::vec![0_u8; ENCODED_VAA_HEADER_BYTES_V1 + SIGNED.len()];
        account
            .get_mut(..8)
            .expect("discriminator region")
            .copy_from_slice(&ENCODED_VAA_DISCRIMINATOR_V1);
        *account.get_mut(8).expect("status byte") = ENCODED_VAA_VERIFIED_STATUS_V1;
        account
            .get_mut(9..41)
            .expect("authority region")
            .copy_from_slice(&[7; 32]);
        account
            .get_mut(41..)
            .expect("signed VAA region")
            .copy_from_slice(SIGNED);
        let view = VerifiedEncodedVaaV1::parse(&account).expect("captured verified VAA");
        assert_eq!(view.guardian_set_index(), 0);
        assert_eq!(view.signature_count(), 13);
        assert_eq!(view.write_authority(), [7; 32]);
        assert_eq!(view.signed_vaa(), SIGNED);
    }
}
