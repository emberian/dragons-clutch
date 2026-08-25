//! Canonical Pyth feed semantics embedded directly in a categorical Market.
//!
//! A raw price observation for this profile means quote-asset units per one
//! base-asset unit. The three identifiers are opaque content/semantic IDs;
//! symbols, mint addresses, decimals, and provider-specific orientation flags
//! are deliberately not parallel sources of truth in this record.

use crate::{Error, Result, array, nonzero};

/// Exact byte width of [`PythFeedProfileV1`].
pub const FEED_PROFILE_BYTES: usize = 106;
/// Pyth feed-profile magic.
pub const FEED_PROFILE_MAGIC: [u8; 8] = *b"DCLTPF01";
/// Implemented Pyth feed-profile schema.
pub const FEED_PROFILE_SCHEMA_VERSION: u16 = 1;

const PROVIDER_FEED_ID_OFFSET: usize = 10;
const BASE_ASSET_SEMANTIC_ID_OFFSET: usize = 42;
const QUOTE_ASSET_SEMANTIC_ID_OFFSET: usize = 74;

/// Immutable semantics for one Pyth feed used by a categorical policy.
///
/// `provider_feed_id` names the provider observation stream. Its raw price is
/// interpreted as quote-asset units per one base-asset unit, where the two
/// asset meanings are named by opaque, nonzero semantic identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythFeedProfileV1 {
    provider_feed_id: [u8; 32],
    base_asset_semantic_id: [u8; 32],
    quote_asset_semantic_id: [u8; 32],
}

impl PythFeedProfileV1 {
    /// Construct one canonical profile from three nonzero, coherent IDs.
    pub fn new(
        provider_feed_id: [u8; 32],
        base_asset_semantic_id: [u8; 32],
        quote_asset_semantic_id: [u8; 32],
    ) -> Result<Self> {
        let profile = Self {
            provider_feed_id,
            base_asset_semantic_id,
            quote_asset_semantic_id,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Decode one exact canonical profile.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FEED_PROFILE_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != FEED_PROFILE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != FEED_PROFILE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        Self::new(
            array(bytes, PROVIDER_FEED_ID_OFFSET)?,
            array(bytes, BASE_ASSET_SEMANTIC_ID_OFFSET)?,
            array(bytes, QUOTE_ASSET_SEMANTIC_ID_OFFSET)?,
        )
    }

    /// Validate the profile's complete semantic domain.
    pub fn validate(&self) -> Result<()> {
        if !nonzero(&self.provider_feed_id)
            || !nonzero(&self.base_asset_semantic_id)
            || !nonzero(&self.quote_asset_semantic_id)
        {
            return Err(Error::ZeroIdentifier);
        }
        if self.base_asset_semantic_id == self.quote_asset_semantic_id {
            return Err(Error::IdenticalAssetSemanticIdentifiers);
        }
        Ok(())
    }

    /// Return the exact canonical fixed-width bytes.
    pub fn to_bytes(self) -> [u8; FEED_PROFILE_BYTES] {
        let mut output = [0u8; FEED_PROFILE_BYTES];
        output[0..8].copy_from_slice(&FEED_PROFILE_MAGIC);
        output[8..10].copy_from_slice(&FEED_PROFILE_SCHEMA_VERSION.to_le_bytes());
        output[PROVIDER_FEED_ID_OFFSET..BASE_ASSET_SEMANTIC_ID_OFFSET]
            .copy_from_slice(&self.provider_feed_id);
        output[BASE_ASSET_SEMANTIC_ID_OFFSET..QUOTE_ASSET_SEMANTIC_ID_OFFSET]
            .copy_from_slice(&self.base_asset_semantic_id);
        output[QUOTE_ASSET_SEMANTIC_ID_OFFSET..FEED_PROFILE_BYTES]
            .copy_from_slice(&self.quote_asset_semantic_id);
        output
    }

    /// Encode into an exact-width caller buffer without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != FEED_PROFILE_BYTES {
            return Err(Error::OutputLength);
        }
        self.validate()?;
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the opaque provider feed identifier.
    pub const fn provider_feed_id(&self) -> &[u8; 32] {
        &self.provider_feed_id
    }

    /// Return the semantic identifier for the raw price's base asset.
    pub const fn base_asset_semantic_id(&self) -> &[u8; 32] {
        &self.base_asset_semantic_id
    }

    /// Return the semantic identifier for the raw price's quote asset.
    pub const fn quote_asset_semantic_id(&self) -> &[u8; 32] {
        &self.quote_asset_semantic_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Result<PythFeedProfileV1> {
        PythFeedProfileV1::new([1; 32], [2; 32], [3; 32])
    }

    #[test]
    fn exact_width_offsets_and_round_trip_are_canonical() -> Result<()> {
        assert_eq!(FEED_PROFILE_BYTES, 106);
        let profile = profile()?;
        let bytes = profile.to_bytes();
        assert_eq!(bytes.get(0..8), Some(&FEED_PROFILE_MAGIC[..]));
        assert_eq!(bytes.get(8..10), Some(&1u16.to_le_bytes()[..]));
        assert_eq!(bytes.get(10..42), Some(&[1; 32][..]));
        assert_eq!(bytes.get(42..74), Some(&[2; 32][..]));
        assert_eq!(bytes.get(74..106), Some(&[3; 32][..]));
        assert_eq!(PythFeedProfileV1::decode(&bytes), Ok(profile));
        assert_eq!(profile.provider_feed_id(), &[1; 32]);
        assert_eq!(profile.base_asset_semantic_id(), &[2; 32]);
        assert_eq!(profile.quote_asset_semantic_id(), &[3; 32]);
        Ok(())
    }

    #[test]
    fn hostile_headers_and_lengths_refuse_atomically() -> Result<()> {
        let profile = profile()?;
        let bytes = profile.to_bytes();
        for length in 0..FEED_PROFILE_BYTES {
            assert_eq!(
                PythFeedProfileV1::decode(bytes.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }
        assert_eq!(
            PythFeedProfileV1::decode(&[0; FEED_PROFILE_BYTES + 1]),
            Err(Error::InvalidLength)
        );

        let mut bad_magic = bytes;
        *bad_magic.get_mut(0).ok_or(Error::InvalidLength)? ^= 0xff;
        assert_eq!(
            PythFeedProfileV1::decode(&bad_magic),
            Err(Error::InvalidMagic)
        );

        let mut bad_schema = bytes;
        bad_schema[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            PythFeedProfileV1::decode(&bad_schema),
            Err(Error::UnsupportedSchema)
        );

        let before = [0x5a; FEED_PROFILE_BYTES - 1];
        let mut wrong = before;
        assert_eq!(profile.encode(&mut wrong), Err(Error::OutputLength));
        assert_eq!(wrong, before);
        Ok(())
    }

    #[test]
    fn zero_and_identical_semantic_identifiers_refuse() -> Result<()> {
        for (provider, base, quote) in [
            ([0; 32], [2; 32], [3; 32]),
            ([1; 32], [0; 32], [3; 32]),
            ([1; 32], [2; 32], [0; 32]),
        ] {
            assert_eq!(
                PythFeedProfileV1::new(provider, base, quote),
                Err(Error::ZeroIdentifier)
            );
        }
        assert_eq!(
            PythFeedProfileV1::new([1; 32], [2; 32], [2; 32]),
            Err(Error::IdenticalAssetSemanticIdentifiers)
        );

        for range in [10..42, 42..74, 74..106] {
            let mut zero_identifier = profile()?.to_bytes();
            zero_identifier
                .get_mut(range)
                .ok_or(Error::InvalidLength)?
                .fill(0);
            assert_eq!(
                PythFeedProfileV1::decode(&zero_identifier),
                Err(Error::ZeroIdentifier)
            );
        }

        let mut identical_assets = profile()?.to_bytes();
        identical_assets[74..106].copy_from_slice(&[2; 32]);
        assert_eq!(
            PythFeedProfileV1::decode(&identical_assets),
            Err(Error::IdenticalAssetSemanticIdentifiers)
        );
        Ok(())
    }
}
