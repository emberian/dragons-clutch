//! Immutable capability policy for the five shared-Market Product families.
//!
//! The policy is a content-addressed artifact. It owns the enabled-family
//! mask exactly once and binds that choice to one immutable Realm/ProfileV2
//! and one reviewed RegistryCapabilityProfileV4. Market-scoped family-root
//! addresses are derived by the account adapter from this authenticated body;
//! they are deliberately not caller fields in this codec.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ContentId, Error, FixedCodec, MarketFamilyV1, RegistryCapabilityProfileV4Id,
    Result, SeriesLinkObligationConfigurationV2, SeriesLinkObligationConfigurationV3,
    SeriesLinkObligationStatusV2, SeriesLinkObligationStatusV3,
    MARKET_FAMILY_COUNT_V1,
};

const MARKET_FAMILY_CAPABILITY_POLICY_MAGIC_V1: [u8; 8] = *b"DCMFCPV1";
const MARKET_FAMILY_CAPABILITY_POLICY_SCHEMA_V1: u16 = 1;
const ALL_MARKET_FAMILY_BITS_V1: u8 = (1_u8 << MARKET_FAMILY_COUNT_V1) - 1;

/// Semantic-ID domain for [`MarketFamilyCapabilityPolicyV1`].
pub const MARKET_FAMILY_CAPABILITY_POLICY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-family-capability-policy/v1";
/// Exact canonical artifact-body width.
pub const MARKET_FAMILY_CAPABILITY_POLICY_BYTES_V1: usize = 128;

/// Immutable Realm/Profile and registry authority for one exact family mask.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFamilyCapabilityPolicyV1 {
    /// Immutable Realm selected by the Market genesis profile.
    pub realm_id: ContentId,
    /// Frozen collateral ProfileV2 selected by that Realm.
    pub collateral_profile_id: ContentId,
    /// Reviewed current central-registry capability profile.
    pub registry_capability_profile_id: RegistryCapabilityProfileV4Id,
    /// Exact enabled-family bits in canonical `MarketFamilyV1` order.
    pub enabled_family_mask: u8,
}

impl MarketFamilyCapabilityPolicyV1 {
    /// Validate the complete immutable body without claiming account authority.
    pub fn validate(&self) -> Result<()> {
        self.realm_id.validate()?;
        self.collateral_profile_id.validate()?;
        self.registry_capability_profile_id.validate()?;
        if self.realm_id == self.collateral_profile_id
            || self.realm_id == self.registry_capability_profile_id.content_id()
            || self.collateral_profile_id == self.registry_capability_profile_id.content_id()
            || self.enabled_family_mask & !ALL_MARKET_FAMILY_BITS_V1 != 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Domain-separated identity of the exact hostile-encoded policy body.
    pub fn id(&self) -> Result<MarketFamilyCapabilityPolicyV1Id> {
        let mut body = [0u8; MARKET_FAMILY_CAPABILITY_POLICY_BYTES_V1];
        self.encode_into(&mut body)?;
        let id = MarketFamilyCapabilityPolicyV1Id(content_id(
            MARKET_FAMILY_CAPABILITY_POLICY_DOMAIN_V1,
            &body,
        ));
        id.validate()?;
        Ok(id)
    }

    /// Whether the immutable policy enables one exhaustive Market family.
    pub const fn is_enabled(&self, family: MarketFamilyV1) -> bool {
        self.enabled_family_mask & family.mask() != 0
    }

    /// Derive the exact initial per-Series obligation configuration.
    ///
    /// Dealer owns both the Dealer child and its liquidity-facility child;
    /// Structured owns both the Structured child and its wrapper child. The
    /// attachment remains an exact immutable identity, but cannot override the
    /// five-family capability policy. This fixed mapping is part of the V1
    /// policy domain rather than a caller-selectable mask.
    pub fn obligation_configuration(
        &self,
        attachment_plan_id: ContentId,
    ) -> Result<SeriesLinkObligationConfigurationV2> {
        self.validate()?;
        attachment_plan_id.validate()?;
        let enabled = SeriesLinkObligationStatusV2::EnabledNeverFounded;
        let disabled = SeriesLinkObligationStatusV2::CapabilityDisabled;
        let dealer = if self.is_enabled(MarketFamilyV1::Dealer) {
            enabled
        } else {
            disabled
        };
        let structured = if self.is_enabled(MarketFamilyV1::Structured) {
            enabled
        } else {
            disabled
        };
        let value = SeriesLinkObligationConfigurationV2 {
            capability_profile_id: self.registry_capability_profile_id.content_id(),
            attachment_plan_id,
            initial_statuses: [dealer, structured, dealer, structured],
        };
        value.validate()?;
        Ok(value)
    }

    /// Derive the exact RootV3/LinkV3 obligation configuration.
    ///
    /// This is the current counterpart of [`Self::obligation_configuration`];
    /// it preserves the same immutable Dealer/Liquidity and
    /// Structured/Wrapper ownership mapping without reinterpreting the V2
    /// configuration bytes.
    pub fn obligation_configuration_v3(
        &self,
        attachment_plan_id: ContentId,
    ) -> Result<SeriesLinkObligationConfigurationV3> {
        self.validate()?;
        attachment_plan_id.validate()?;
        let enabled = SeriesLinkObligationStatusV3::EnabledNeverFounded;
        let disabled = SeriesLinkObligationStatusV3::CapabilityDisabled;
        let dealer = if self.is_enabled(MarketFamilyV1::Dealer) {
            enabled
        } else {
            disabled
        };
        let structured = if self.is_enabled(MarketFamilyV1::Structured) {
            enabled
        } else {
            disabled
        };
        let value = SeriesLinkObligationConfigurationV3 {
            capability_profile_id: self.registry_capability_profile_id.content_id(),
            attachment_plan_id,
            initial_statuses: [dealer, structured, dealer, structured],
        };
        value.validate()?;
        Ok(value)
    }
}

impl FixedCodec for MarketFamilyCapabilityPolicyV1 {
    const ENCODED_LEN: usize = MARKET_FAMILY_CAPABILITY_POLICY_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_FAMILY_CAPABILITY_POLICY_MAGIC_V1);
        writer.u16(MARKET_FAMILY_CAPABILITY_POLICY_SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.realm_id);
        writer.id(self.collateral_profile_id);
        writer.id(self.registry_capability_profile_id.content_id());
        writer.u8(self.enabled_family_mask);
        writer.reserved(15);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_FAMILY_CAPABILITY_POLICY_MAGIC_V1)?;
        if reader.u16() != MARKET_FAMILY_CAPABILITY_POLICY_SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            realm_id: reader.id(),
            collateral_profile_id: reader.id(),
            registry_capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes(
                reader.id().bytes(),
            ),
            enabled_family_mask: reader.u8(),
        };
        reader.reserved(15)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Typed identity of one exact immutable family capability policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MarketFamilyCapabilityPolicyV1Id(ContentId);

impl MarketFamilyCapabilityPolicyV1Id {
    /// Construct from exact bytes without claiming artifact authentication.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ContentId::from_bytes(bytes))
    }

    /// Return the exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Return the generic content identity.
    pub const fn content_id(self) -> ContentId {
        self.0
    }

    /// Refuse the reserved all-zero identity.
    pub fn validate(self) -> Result<()> {
        self.0.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> MarketFamilyCapabilityPolicyV1 {
        MarketFamilyCapabilityPolicyV1 {
            realm_id: ContentId::from_bytes([1; 32]),
            collateral_profile_id: ContentId::from_bytes([2; 32]),
            registry_capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes([3; 32]),
            enabled_family_mask: 0b1_0111,
        }
    }

    #[test]
    fn exact_round_trip_and_identity() {
        let value = policy();
        let mut body = [0u8; MARKET_FAMILY_CAPABILITY_POLICY_BYTES_V1];
        value.encode_into(&mut body).unwrap();
        assert_eq!(MarketFamilyCapabilityPolicyV1::decode(&body), Ok(value));
        assert!(!value.id().unwrap().content_id().is_zero());
    }

    #[test]
    fn hostile_reserved_bits_and_identity_aliases_refuse() {
        let mut reserved = policy();
        reserved.enabled_family_mask = 0b10_0000;
        assert_eq!(reserved.validate(), Err(Error::InvalidParameter));

        let mut aliased = policy();
        aliased.collateral_profile_id = aliased.realm_id;
        assert_eq!(aliased.validate(), Err(Error::InvalidParameter));
    }

    #[test]
    fn obligation_mapping_is_exhaustive_and_family_owned() {
        let mut value = policy();
        value.enabled_family_mask = MarketFamilyV1::Dealer.mask();
        let configuration = value
            .obligation_configuration(ContentId::from_bytes([9; 32]))
            .unwrap();
        assert_eq!(
            configuration.initial_statuses,
            [
                SeriesLinkObligationStatusV2::EnabledNeverFounded,
                SeriesLinkObligationStatusV2::CapabilityDisabled,
                SeriesLinkObligationStatusV2::EnabledNeverFounded,
                SeriesLinkObligationStatusV2::CapabilityDisabled,
            ]
        );

        value.enabled_family_mask = MarketFamilyV1::Structured.mask();
        let configuration = value
            .obligation_configuration(ContentId::from_bytes([9; 32]))
            .unwrap();
        assert_eq!(
            configuration.initial_statuses,
            [
                SeriesLinkObligationStatusV2::CapabilityDisabled,
                SeriesLinkObligationStatusV2::EnabledNeverFounded,
                SeriesLinkObligationStatusV2::CapabilityDisabled,
                SeriesLinkObligationStatusV2::EnabledNeverFounded,
            ]
        );
    }
}
