//! Exact borrowed views of the pinned Wormhole router accounts used by Pyth.

/// Anchor discriminator for the router `EncodedVaa` account.
pub const ENCODED_VAA_DISCRIMINATOR_V1: [u8; 8] = [0xe2, 0x65, 0xa3, 0x04, 0x85, 0xa0, 0x54, 0xf5];
/// Router processing status for a cryptographically verified VAA.
pub const ENCODED_VAA_VERIFIED_STATUS_V1: u8 = 2;
/// Fixed Anchor/Borsh header before the signed VAA vector payload.
pub const ENCODED_VAA_HEADER_BYTES_V1: usize = 46;
/// Serialized legacy GuardianSet header through its vector count.
pub const GUARDIAN_SET_HEADER_BYTES_V1: usize = 8;
/// One Ethereum guardian address width.
pub const GUARDIAN_ADDRESS_BYTES_V1: usize = 20;
const GUARDIAN_SET_ALLOCATION_TAIL_BYTES_V1: usize = 8;

/// Stable hostile router-account refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterAccountErrorV1 {
    /// Bytes were truncated or had an impossible exact size.
    InvalidLength,
    /// Anchor discriminator selected another account type.
    InvalidDiscriminator,
    /// Encoded VAA was not in the terminal verified phase.
    NotVerified,
    /// Router account format version was not the pinned V1 format.
    UnsupportedAccountVersion,
    /// Signed VAA version was not the pinned V1 format.
    UnsupportedVaaVersion,
    /// Guardian count was zero, did not fit V1, or differed from the release.
    InvalidGuardianCount,
    /// Guardian set index in account and VAA differed.
    GuardianSetMismatch,
    /// Serialized content left a noncanonical account-allocation tail.
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
        if bytes.len() < ENCODED_VAA_HEADER_BYTES_V1 {
            return Err(RouterAccountErrorV1::InvalidLength);
        }
        if array::<8>(bytes, 0)? != ENCODED_VAA_DISCRIMINATOR_V1 {
            return Err(RouterAccountErrorV1::InvalidDiscriminator);
        }
        if byte(bytes, 8)? != ENCODED_VAA_VERIFIED_STATUS_V1 {
            return Err(RouterAccountErrorV1::NotVerified);
        }
        if byte(bytes, 41)? != 1 {
            return Err(RouterAccountErrorV1::UnsupportedAccountVersion);
        }
        let payload_len = usize::try_from(u32::from_le_bytes(array(bytes, 42)?))
            .map_err(|_| RouterAccountErrorV1::InvalidLength)?;
        let payload_end = ENCODED_VAA_HEADER_BYTES_V1
            .checked_add(payload_len)
            .ok_or(RouterAccountErrorV1::InvalidLength)?;
        if bytes.len() < payload_end {
            return Err(RouterAccountErrorV1::InvalidLength);
        }
        if bytes.len() != payload_end {
            return Err(RouterAccountErrorV1::NonCanonicalTail);
        }
        let signed_vaa = bytes
            .get(ENCODED_VAA_HEADER_BYTES_V1..payload_end)
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
    /// Parse the pinned legacy/Borsh GuardianSet layout.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, RouterAccountErrorV1> {
        if bytes.len()
            < GUARDIAN_SET_HEADER_BYTES_V1
                + GUARDIAN_ADDRESS_BYTES_V1
                + 8
                + GUARDIAN_SET_ALLOCATION_TAIL_BYTES_V1
        {
            return Err(RouterAccountErrorV1::InvalidLength);
        }
        let index = u32::from_le_bytes(array(bytes, 0)?);
        let count = u32::from_le_bytes(array(bytes, 4)?);
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
        let account_end = times_end
            .checked_add(GUARDIAN_SET_ALLOCATION_TAIL_BYTES_V1)
            .ok_or(RouterAccountErrorV1::InvalidLength)?;
        if bytes.len() != account_end {
            return Err(RouterAccountErrorV1::InvalidLength);
        }
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

    fn encoded_account(authority: [u8; 32], signed_vaa: &[u8]) -> std::vec::Vec<u8> {
        let mut account = std::vec![0_u8; ENCODED_VAA_HEADER_BYTES_V1 + signed_vaa.len()];
        account
            .get_mut(..8)
            .expect("discriminator region")
            .copy_from_slice(&ENCODED_VAA_DISCRIMINATOR_V1);
        *account.get_mut(8).expect("status byte") = ENCODED_VAA_VERIFIED_STATUS_V1;
        account
            .get_mut(9..41)
            .expect("authority region")
            .copy_from_slice(&authority);
        *account.get_mut(41).expect("account version") = 1;
        account
            .get_mut(42..46)
            .expect("vector length region")
            .copy_from_slice(
                &u32::try_from(signed_vaa.len())
                    .expect("bounded test VAA")
                    .to_le_bytes(),
            );
        account
            .get_mut(ENCODED_VAA_HEADER_BYTES_V1..)
            .expect("signed VAA region")
            .copy_from_slice(signed_vaa);
        account
    }

    fn signed_vaa(guardian_set_index: u32, signature_count: u8) -> std::vec::Vec<u8> {
        let mut signed = std::vec![0_u8; 7 + usize::from(signature_count) * 66];
        *signed.get_mut(0).expect("signed VAA version") = 1;
        signed
            .get_mut(1..5)
            .expect("guardian index region")
            .copy_from_slice(&guardian_set_index.to_be_bytes());
        *signed.get_mut(5).expect("signature count") = signature_count;
        signed
    }

    fn guardian_account(index: u32, count: u32) -> std::vec::Vec<u8> {
        let key_bytes = usize::try_from(count)
            .expect("bounded guardian count")
            .checked_mul(GUARDIAN_ADDRESS_BYTES_V1)
            .expect("bounded guardian bytes");
        let mut account = std::vec![
            0_u8;
            GUARDIAN_SET_HEADER_BYTES_V1
                + key_bytes
                + 8
                + GUARDIAN_SET_ALLOCATION_TAIL_BYTES_V1
        ];
        account
            .get_mut(..4)
            .expect("guardian index region")
            .copy_from_slice(&index.to_le_bytes());
        account
            .get_mut(4..8)
            .expect("guardian count region")
            .copy_from_slice(&count.to_le_bytes());
        account
    }

    fn decode_hex(hex: &str) -> std::vec::Vec<u8> {
        let value = hex.trim().as_bytes();
        assert_eq!(value.len() % 2, 0, "hex fixture has complete bytes");
        value
            .chunks_exact(2)
            .map(|pair| {
                let pair = core::str::from_utf8(pair).expect("ASCII hex fixture");
                u8::from_str_radix(pair, 16).expect("lowercase hex fixture")
            })
            .collect()
    }

    #[test]
    fn verified_vaa_and_guardian_set_join_exactly() {
        let encoded = encoded_account([7; 32], &signed_vaa(9, 3));
        let vaa = VerifiedEncodedVaaV1::parse(&encoded).expect("verified VAA");

        let guardians = guardian_account(9, 5);
        let set = GuardianSetV1::parse(&guardians).expect("guardian set");
        assert_eq!(set.authenticate(vaa, 5, 3), Ok(()));
        assert_eq!(
            set.authenticate(vaa, 4, 3),
            Err(RouterAccountErrorV1::InvalidGuardianCount)
        );
    }

    #[test]
    fn processing_and_index_substitutions_refuse() {
        let mut encoded = encoded_account([7; 32], &signed_vaa(0, 1));
        *encoded.get_mut(8).expect("status byte") = 1;
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&encoded),
            Err(RouterAccountErrorV1::NotVerified)
        );
        *encoded.get_mut(8).expect("status byte") = 2;
        let guardians = guardian_account(1, 1);
        let vaa = VerifiedEncodedVaaV1::parse(&encoded).expect("verified VAA");
        assert_eq!(
            GuardianSetV1::parse(&guardians)
                .expect("guardian set")
                .authenticate(vaa, 1, 1),
            Err(RouterAccountErrorV1::GuardianSetMismatch)
        );
    }

    #[test]
    fn account_version_and_vector_bounds_refuse() {
        let signed = signed_vaa(0, 1);
        let canonical = encoded_account([7; 32], &signed);

        let mut wrong_account_version = canonical.clone();
        *wrong_account_version.get_mut(41).expect("account version") = 2;
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&wrong_account_version),
            Err(RouterAccountErrorV1::UnsupportedAccountVersion)
        );

        let mut wrong_vaa_version = canonical.clone();
        *wrong_vaa_version
            .get_mut(ENCODED_VAA_HEADER_BYTES_V1)
            .expect("signed VAA version") = 2;
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&wrong_vaa_version),
            Err(RouterAccountErrorV1::UnsupportedVaaVersion)
        );

        let mut overclaimed = canonical.clone();
        overclaimed
            .get_mut(42..46)
            .expect("vector length region")
            .copy_from_slice(
                &u32::try_from(signed.len() + 1)
                    .expect("bounded test VAA")
                    .to_le_bytes(),
            );
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&overclaimed),
            Err(RouterAccountErrorV1::InvalidLength)
        );

        let mut underclaimed = canonical.clone();
        underclaimed
            .get_mut(42..46)
            .expect("vector length region")
            .copy_from_slice(
                &u32::try_from(signed.len() - 1)
                    .expect("bounded test VAA")
                    .to_le_bytes(),
            );
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&underclaimed),
            Err(RouterAccountErrorV1::NonCanonicalTail)
        );

        let mut truncated = canonical.clone();
        truncated.pop();
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&truncated),
            Err(RouterAccountErrorV1::InvalidLength)
        );

        let mut trailing = canonical;
        trailing.push(0);
        assert_eq!(
            VerifiedEncodedVaaV1::parse(&trailing),
            Err(RouterAccountErrorV1::NonCanonicalTail)
        );
    }

    #[test]
    fn captured_signed_vaa_has_the_pinned_real_quorum_shape() {
        const SIGNED: &[u8] =
            include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
        let account = encoded_account([7; 32], SIGNED);
        let view = VerifiedEncodedVaaV1::parse(&account).expect("captured verified VAA");
        assert_eq!(view.guardian_set_index(), 0);
        assert_eq!(view.signature_count(), 13);
        assert_eq!(view.write_authority(), [7; 32]);
        assert_eq!(view.signed_vaa(), SIGNED);
    }

    #[test]
    fn captured_real_guardian_set_has_the_legacy_layout() {
        const CAPTURED: &str = include_str!(
            "../../../fixtures/pyth/local-upgraded-2026-08-22/guardian-set-0.account.hex"
        );
        let account = decode_hex(CAPTURED);
        assert_eq!(account.len(), 404);
        let view = GuardianSetV1::parse(&account).expect("captured GuardianSet");
        assert_eq!(view.index(), 0);
        assert_eq!(view.guardian_count(), 19);
        assert_eq!(view.creation_time(), 1_787_431_680);
        assert_eq!(view.expiration_time(), 0);
    }

    #[test]
    fn guardian_count_length_timestamp_and_tail_substitutions_refuse() {
        let canonical = guardian_account(0, 3);

        let mut zero_count = canonical.clone();
        zero_count
            .get_mut(4..8)
            .expect("guardian count region")
            .copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            GuardianSetV1::parse(&zero_count),
            Err(RouterAccountErrorV1::InvalidGuardianCount)
        );

        let mut overclaimed = canonical.clone();
        overclaimed
            .get_mut(4..8)
            .expect("guardian count region")
            .copy_from_slice(&4_u32.to_le_bytes());
        assert_eq!(
            GuardianSetV1::parse(&overclaimed),
            Err(RouterAccountErrorV1::InvalidLength)
        );

        let mut truncated = canonical.clone();
        truncated.pop();
        assert_eq!(
            GuardianSetV1::parse(&truncated),
            Err(RouterAccountErrorV1::InvalidLength)
        );

        let times = GUARDIAN_SET_HEADER_BYTES_V1 + 3 * GUARDIAN_ADDRESS_BYTES_V1;
        let mut shifted_timestamp = canonical.clone();
        shifted_timestamp
            .get_mut(times..times + 4)
            .expect("creation timestamp")
            .copy_from_slice(&7_u32.to_le_bytes());
        let shifted = GuardianSetV1::parse(&shifted_timestamp).expect("valid shifted timestamp");
        assert_eq!(shifted.creation_time(), 7);

        let mut noncanonical_tail = canonical;
        *noncanonical_tail.last_mut().expect("allocation tail") = 1;
        assert_eq!(
            GuardianSetV1::parse(&noncanonical_tail),
            Err(RouterAccountErrorV1::NonCanonicalTail)
        );
    }
}
