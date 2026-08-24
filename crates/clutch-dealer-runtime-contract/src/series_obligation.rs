// SPDX-License-Identifier: AGPL-3.0-or-later

//! Facility-lifetime Product Series obligation owned by Dealer.
//!
//! A Product `SeriesMarketLinkV1` has one Dealer obligation slot for the whole
//! facility lifetime.  A per-Epoch Lease or CoveredDealer selection therefore
//! cannot own that slot.  This body retains the exact authenticated Product
//! admission postwrite until Dealer root retirement supplies the terminal
//! receipt and consumes the same Product obligation.

use sha2::{Digest, Sha256};

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    add, DealerStateV3, DeletableRentOwnerV1, Error, FixedCodec, Id, Result,
    DEALER_SERIES_OBLIGATION_CONTENT_DOMAIN_V1,
    DEALER_SERIES_OBLIGATION_CONTENT_DOMAIN_V2,
    DEALER_SERIES_OBLIGATION_CONTENT_DOMAIN_V3, DELETABLE_RENT_OWNER_BYTES,
};

/// Exact local semantic-body magic.
pub const DEALER_SERIES_OBLIGATION_MAGIC_V1: [u8; 8] = *b"DCDSOBV1";
/// Exact local semantic-body version.
pub const DEALER_SERIES_OBLIGATION_VERSION_V1: u16 = 1;
/// Exact canonical body bytes.
pub const DEALER_SERIES_OBLIGATION_BYTES_V1: usize =
    HEADER_BYTES + (20 * 32) + (3 * 8) + 4 + 4 + DELETABLE_RENT_OWNER_BYTES;
/// Exact local semantic magic for the Product RootV2/LinkV2 successor.
pub const DEALER_SERIES_OBLIGATION_MAGIC_V2: [u8; 8] = *b"DCDSOBV2";
/// Exact local semantic version for the Product RootV2/LinkV2 successor.
pub const DEALER_SERIES_OBLIGATION_VERSION_V2: u16 = 2;
/// Exact current semantic body bytes, excluding the global Dealer envelope.
pub const DEALER_SERIES_OBLIGATION_BYTES_V2: usize =
    HEADER_BYTES + (21 * 32) + (3 * 8) + 4 + 4 + DELETABLE_RENT_OWNER_BYTES;
/// Exact local semantic magic for the Product RootV3/LinkV3 successor.
pub const DEALER_SERIES_OBLIGATION_MAGIC_V3: [u8; 8] = *b"DCDSOBV3";
/// Exact local semantic version for the Product RootV3/LinkV3 successor.
pub const DEALER_SERIES_OBLIGATION_VERSION_V3: u16 = 3;
/// Exact current semantic body bytes, excluding the global Dealer envelope.
pub const DEALER_SERIES_OBLIGATION_BYTES_V3: usize =
    HEADER_BYTES + (24 * 32) + (3 * 8) + 4 + 4 + DELETABLE_RENT_OWNER_BYTES;

const DEALER_SERIES_ADMISSION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/series-obligation-admission-receipt/v1\0";
const DEALER_SERIES_TERMINAL_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/series-obligation-terminal-receipt/v1\0";
const DEALER_SERIES_ADMISSION_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/dealer-runtime/series-obligation-admission-receipt/v2\0";
const DEALER_SERIES_TERMINAL_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/dealer-runtime/series-obligation-terminal-receipt/v2\0";
const DEALER_SERIES_VALUE_CLOSE_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/dealer-runtime/series-obligation-value-close-receipt/v3\0";

/// Immutable Dealer-owned join to current Product RootV2/LinkV2 artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationKeyV2 {
    /// Physical rent-owned `0xaf/v2` account.
    pub binding_account_id: Id,
    /// Exact immutable Dealer policy.
    pub policy_id: Id,
    /// Exact immutable facility.
    pub facility_id: Id,
    /// Authoritative Dealer State account.
    pub dealer_state_account_id: Id,
    /// Canonical facility Position-purpose binding.
    pub facility_position_binding_id: Id,
    /// Full Product MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact current Product RootV2 account.
    pub product_market_root_account_id: Id,
    /// Immutable Product RootV2 binding identity.
    pub product_market_binding_id: Id,
    /// Exact SeriesPlanV5 identity.
    pub series_plan_v5_id: Id,
    /// Exact current Product LinkV2 account.
    pub series_market_link_account_id: Id,
    /// Exact current compiler BundleV6 identity.
    pub compiler_bundle_v6_id: Id,
    /// Exact current AttachmentV5 identity.
    pub attachment_plan_v5_id: Id,
    /// Product shared-Market generation.
    pub product_generation: u64,
    /// Exact admitted Series ordinal.
    pub series_ordinal: u32,
}

impl DealerSeriesObligationKeyV2 {
    /// Validate the complete current immutable Product coordinate set.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.binding_account_id,
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_binding_id,
            self.market_instance_v2_id,
            self.product_market_root_account_id,
            self.product_market_binding_id,
            self.series_plan_v5_id,
            self.series_market_link_account_id,
            self.compiler_bundle_v6_id,
            self.attachment_plan_v5_id,
        ];
        for identity in identities {
            identity.validate_live()?;
        }
        let physical = [
            self.binding_account_id,
            self.dealer_state_account_id,
            self.product_market_root_account_id,
            self.series_market_link_account_id,
        ];
        let mut left = 0usize;
        while left < physical.len() {
            let mut right = left + 1;
            while right < physical.len() {
                if physical[left] == physical[right] {
                    return Err(Error::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        if self.product_generation == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Derive the exact Dealer receipt consumed by current LinkV2 admission.
    pub fn admission_owner_receipt_id(
        &self,
        link_pre_semantic_id: Id,
        link_transition_sequence: u64,
    ) -> Result<Id> {
        self.validate()?;
        link_pre_semantic_id.validate_live()?;
        if link_transition_sequence == 0 {
            return Err(Error::InvalidParameter);
        }
        let mut hasher = Sha256::new();
        hasher.update(DEALER_SERIES_ADMISSION_RECEIPT_DOMAIN_V2);
        hash_key_v2(self, &mut hasher);
        hasher.update(link_pre_semantic_id.bytes());
        hasher.update(link_transition_sequence.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

/// Current facility-lifetime Product obligation. Product alone advances its
/// LinkV2 status; Dealer persists the exact accepted pre/post receipt chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationBindingV2 {
    /// Immutable current coordinate set.
    pub key: DealerSeriesObligationKeyV2,
    /// Dealer-owned admission receipt supplied to Product.
    pub admission_owner_receipt_id: Id,
    /// Exact Product LinkV2 admission projection receipt.
    pub admission_projection_id: Id,
    /// Exact LinkV2 semantic identity before admission.
    pub admission_link_pre_semantic_id: Id,
    /// Exact LinkV2 semantic identity after admission.
    pub admission_link_post_semantic_id: Id,
    /// Dealer-owned terminal receipt; zero while live.
    pub terminal_owner_receipt_id: Id,
    /// Exact Product LinkV2 terminal projection; zero while live.
    pub terminal_projection_id: Id,
    /// LinkV2 semantic identity before terminal consumption; zero while live.
    pub terminal_link_pre_semantic_id: Id,
    /// LinkV2 semantic identity after terminal consumption; zero while live.
    pub terminal_link_post_semantic_id: Id,
    /// Exact terminal Dealer State receipt; zero while live.
    pub terminal_state_receipt_id: Id,
    /// LinkV2 transition sequence after admission.
    pub admission_link_transition_sequence: u64,
    /// LinkV2 transition sequence after terminal consumption; zero while live.
    pub terminal_link_transition_sequence: u64,
    /// Exhaustive facility-obligation phase.
    pub phase: DealerSeriesObligationPhaseV1,
    /// Exact refundable rent owner and hostile-prefund sink.
    pub rent: DeletableRentOwnerV1,
}

impl DealerSeriesObligationBindingV2 {
    /// Construct the sole live current Product obligation.
    #[allow(clippy::too_many_arguments)]
    pub fn new_live(
        key: DealerSeriesObligationKeyV2,
        admission_owner_receipt_id: Id,
        admission_projection_id: Id,
        admission_link_pre_semantic_id: Id,
        admission_link_post_semantic_id: Id,
        admission_link_transition_sequence: u64,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        if admission_owner_receipt_id
            != key.admission_owner_receipt_id(
                admission_link_pre_semantic_id,
                admission_link_transition_sequence,
            )?
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            key,
            admission_owner_receipt_id,
            admission_projection_id,
            admission_link_pre_semantic_id,
            admission_link_post_semantic_id,
            terminal_owner_receipt_id: Id::ZERO,
            terminal_projection_id: Id::ZERO,
            terminal_link_pre_semantic_id: Id::ZERO,
            terminal_link_post_semantic_id: Id::ZERO,
            terminal_state_receipt_id: Id::ZERO,
            admission_link_transition_sequence,
            terminal_link_transition_sequence: 0,
            phase: DealerSeriesObligationPhaseV1::Live,
            rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate live-versus-terminal receipt exhaustiveness.
    pub fn validate(&self) -> Result<()> {
        self.key.validate()?;
        self.rent.validate()?;
        for identity in [
            self.admission_owner_receipt_id,
            self.admission_projection_id,
            self.admission_link_pre_semantic_id,
            self.admission_link_post_semantic_id,
        ] {
            identity.validate_live()?;
        }
        if self.rent.payer == self.rent.neutral_sink
            || self.admission_link_transition_sequence == 0
        {
            return Err(Error::MismatchedBinding);
        }
        let terminal = [
            self.terminal_owner_receipt_id,
            self.terminal_projection_id,
            self.terminal_link_pre_semantic_id,
            self.terminal_link_post_semantic_id,
            self.terminal_state_receipt_id,
        ];
        match self.phase {
            DealerSeriesObligationPhaseV1::Live => {
                if terminal.iter().any(|identity| !identity.is_zero())
                    || self.terminal_link_transition_sequence != 0
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerSeriesObligationPhaseV1::Terminal => {
                for identity in terminal {
                    identity.validate_live()?;
                }
                if self.terminal_link_transition_sequence
                    <= self.admission_link_transition_sequence
                {
                    return Err(Error::InvalidPhase);
                }
            }
        }
        Ok(())
    }

    /// Exact current binding identity retained by Dealer StateV3.
    pub fn binding_id(&self) -> Result<Id> {
        self.content_id(DEALER_SERIES_OBLIGATION_CONTENT_DOMAIN_V2)
    }

    /// Commit Product's exact LinkV2 terminal postwrite once. Product owns
    /// the Link mutation; Dealer owns this immutable receipt chain and cannot
    /// infer terminality from a caller-supplied status or counter.
    pub fn terminalized(
        self,
        terminal_owner_receipt_id: Id,
        terminal_projection_id: Id,
        terminal_link_pre_semantic_id: Id,
        terminal_link_post_semantic_id: Id,
        terminal_state_receipt_id: Id,
        terminal_link_transition_sequence: u64,
    ) -> Result<Self> {
        let expected_owner_receipt = self.terminal_owner_receipt_id(
            terminal_state_receipt_id,
            terminal_link_pre_semantic_id,
            terminal_link_transition_sequence,
        )?;
        if terminal_owner_receipt_id != expected_owner_receipt
            || self.phase != DealerSeriesObligationPhaseV1::Live
            || terminal_link_transition_sequence <= self.admission_link_transition_sequence
        {
            return Err(Error::InvalidPhase);
        }
        let value = Self {
            terminal_owner_receipt_id,
            terminal_projection_id,
            terminal_link_pre_semantic_id,
            terminal_link_post_semantic_id,
            terminal_state_receipt_id,
            terminal_link_transition_sequence,
            phase: DealerSeriesObligationPhaseV1::Terminal,
            ..self
        };
        value.validate()?;
        Ok(value)
    }

    /// Derive the once-only Dealer receipt supplied to current LinkV2 terminalization.
    pub fn terminal_owner_receipt_id(
        &self,
        terminal_state_receipt_id: Id,
        terminal_link_pre_semantic_id: Id,
        terminal_link_transition_sequence: u64,
    ) -> Result<Id> {
        self.validate()?;
        terminal_state_receipt_id.validate_live()?;
        terminal_link_pre_semantic_id.validate_live()?;
        if self.phase != DealerSeriesObligationPhaseV1::Live
            || terminal_link_transition_sequence <= self.admission_link_transition_sequence
        {
            return Err(Error::InvalidPhase);
        }
        let mut hasher = Sha256::new();
        hasher.update(DEALER_SERIES_TERMINAL_RECEIPT_DOMAIN_V2);
        hasher.update(self.binding_id()?.bytes());
        hasher.update(terminal_state_receipt_id.bytes());
        hasher.update(terminal_link_pre_semantic_id.bytes());
        hasher.update(terminal_link_transition_sequence.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

impl FixedCodec for DealerSeriesObligationBindingV2 {
    const ENCODED_LEN: usize = DEALER_SERIES_OBLIGATION_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_SERIES_OBLIGATION_MAGIC_V2,
            DEALER_SERIES_OBLIGATION_VERSION_V2,
        );
        for identity in [
            self.key.binding_account_id,
            self.key.policy_id,
            self.key.facility_id,
            self.key.dealer_state_account_id,
            self.key.facility_position_binding_id,
            self.key.market_instance_v2_id,
            self.key.product_market_root_account_id,
            self.key.product_market_binding_id,
            self.key.series_plan_v5_id,
            self.key.series_market_link_account_id,
            self.key.compiler_bundle_v6_id,
            self.key.attachment_plan_v5_id,
            self.admission_owner_receipt_id,
            self.admission_projection_id,
            self.admission_link_pre_semantic_id,
            self.admission_link_post_semantic_id,
            self.terminal_owner_receipt_id,
            self.terminal_projection_id,
            self.terminal_link_pre_semantic_id,
            self.terminal_link_post_semantic_id,
            self.terminal_state_receipt_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.key.product_generation);
        writer.u64(self.admission_link_transition_sequence);
        writer.u64(self.terminal_link_transition_sequence);
        writer.u32(self.key.series_ordinal);
        writer.u8(self.phase.byte());
        writer.reserved(3);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_SERIES_OBLIGATION_MAGIC_V2,
            DEALER_SERIES_OBLIGATION_VERSION_V2,
        )?;
        let mut identities = [Id::ZERO; 21];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index] = reader.id();
            index += 1;
        }
        let value = Self {
            key: DealerSeriesObligationKeyV2 {
                binding_account_id: identities[0],
                policy_id: identities[1],
                facility_id: identities[2],
                dealer_state_account_id: identities[3],
                facility_position_binding_id: identities[4],
                market_instance_v2_id: identities[5],
                product_market_root_account_id: identities[6],
                product_market_binding_id: identities[7],
                series_plan_v5_id: identities[8],
                series_market_link_account_id: identities[9],
                compiler_bundle_v6_id: identities[10],
                attachment_plan_v5_id: identities[11],
                product_generation: reader.u64(),
                series_ordinal: 0,
            },
            admission_owner_receipt_id: identities[12],
            admission_projection_id: identities[13],
            admission_link_pre_semantic_id: identities[14],
            admission_link_post_semantic_id: identities[15],
            terminal_owner_receipt_id: identities[16],
            terminal_projection_id: identities[17],
            terminal_link_pre_semantic_id: identities[18],
            terminal_link_post_semantic_id: identities[19],
            terminal_state_receipt_id: identities[20],
            admission_link_transition_sequence: reader.u64(),
            terminal_link_transition_sequence: reader.u64(),
            phase: DealerSeriesObligationPhaseV1::Live,
            rent: DeletableRentOwnerV1 {
                payer: Id::ZERO,
                neutral_sink: Id::ZERO,
                refundable_principal: 0,
                donation_floor: 0,
            },
        };
        let mut value = value;
        value.key.series_ordinal = reader.u32();
        value.phase = DealerSeriesObligationPhaseV1::decode(reader.u8())?;
        reader.reserved(3)?;
        value.rent = DeletableRentOwnerV1::decode_body(&mut reader);
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Immutable Dealer-owned join to Product RootV3/LinkV3.
///
/// Unlike V2, this key names the current BundleV7 and AttachmentV6 and binds
/// the capability-derived obligation configuration. Product's generic
/// admission owner receipt is not re-derived here: the SBF two-phase composer
/// authenticates it against the hostile RootV3/LinkV3 plan before this body is
/// physically written, and later readers compare it with LinkV3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationKeyV3 {
    /// Physical rent-owned `0xaf/v3` account.
    pub binding_account_id: Id,
    /// Exact immutable Dealer policy.
    pub policy_id: Id,
    /// Exact immutable facility.
    pub facility_id: Id,
    /// Authoritative Dealer State account.
    pub dealer_state_account_id: Id,
    /// Canonical facility Position-purpose binding.
    pub facility_position_binding_id: Id,
    /// Full Product MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact Product RootV3 account.
    pub product_market_root_account_id: Id,
    /// Immutable Product RootV3 binding identity.
    pub product_market_binding_id: Id,
    /// Exact SeriesPlanV5 identity.
    pub series_plan_v5_id: Id,
    /// Exact Product LinkV3 account.
    pub series_market_link_account_id: Id,
    /// Exact compiler BundleV7 identity.
    pub compiler_bundle_v7_id: Id,
    /// Exact AttachmentV6 identity.
    pub attachment_plan_v6_id: Id,
    /// Capability-derived LinkV3 obligation configuration.
    pub obligation_configuration_v3_id: Id,
    /// Product shared-Market generation.
    pub product_generation: u64,
    /// Exact admitted Series ordinal.
    pub series_ordinal: u32,
}

impl DealerSeriesObligationKeyV3 {
    /// Validate the complete current immutable Product coordinate set.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.binding_account_id,
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_binding_id,
            self.market_instance_v2_id,
            self.product_market_root_account_id,
            self.product_market_binding_id,
            self.series_plan_v5_id,
            self.series_market_link_account_id,
            self.compiler_bundle_v7_id,
            self.attachment_plan_v6_id,
            self.obligation_configuration_v3_id,
        ];
        for identity in identities {
            identity.validate_live()?;
        }
        let physical = [
            self.binding_account_id,
            self.dealer_state_account_id,
            self.product_market_root_account_id,
            self.series_market_link_account_id,
        ];
        let mut left = 0usize;
        while left < physical.len() {
            let mut right = left + 1;
            while right < physical.len() {
                if physical[left] == physical[right] {
                    return Err(Error::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        if self.product_generation == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Current facility-lifetime Product obligation admitted through the generic
/// RootV3 family and LinkV3 obligation two-phase writers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationBindingV3 {
    /// Immutable current coordinate set.
    pub key: DealerSeriesObligationKeyV3,
    /// Dealer's exact prewrite consumed by both Product preparations.
    pub owner_prewrite_id: Id,
    /// Product RootV3 family admission receipt accepted after Dealer postwrite.
    pub family_admission_receipt_id: Id,
    /// Product LinkV3 owner admission receipt.
    pub admission_owner_receipt_id: Id,
    /// Exact Product LinkV3 admission projection.
    pub admission_projection_id: Id,
    /// Exact LinkV3 semantic identity before admission.
    pub admission_link_pre_semantic_id: Id,
    /// Exact LinkV3 semantic identity after admission.
    pub admission_link_post_semantic_id: Id,
    /// Dealer-owned terminal receipt; zero while live.
    pub terminal_owner_receipt_id: Id,
    /// Exact Product LinkV3 terminal projection; zero while live.
    pub terminal_projection_id: Id,
    /// LinkV3 semantic identity before terminal consumption; zero while live.
    pub terminal_link_pre_semantic_id: Id,
    /// LinkV3 semantic identity after terminal consumption; zero while live.
    pub terminal_link_post_semantic_id: Id,
    /// Exact terminal Dealer State receipt; zero while live.
    pub terminal_state_receipt_id: Id,
    /// LinkV3 transition sequence after admission.
    pub admission_link_transition_sequence: u64,
    /// LinkV3 transition sequence after terminal consumption; zero while live.
    pub terminal_link_transition_sequence: u64,
    /// Exhaustive facility-obligation phase.
    pub phase: DealerSeriesObligationPhaseV1,
    /// Exact refundable rent owner and hostile-prefund sink.
    pub rent: DeletableRentOwnerV1,
}

impl DealerSeriesObligationBindingV3 {
    /// Construct the sole live RootV3/LinkV3 obligation postimage.
    #[allow(clippy::too_many_arguments)]
    pub fn new_live(
        key: DealerSeriesObligationKeyV3,
        owner_prewrite_id: Id,
        family_admission_receipt_id: Id,
        admission_owner_receipt_id: Id,
        admission_projection_id: Id,
        admission_link_pre_semantic_id: Id,
        admission_link_post_semantic_id: Id,
        admission_link_transition_sequence: u64,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        let value = Self {
            key,
            owner_prewrite_id,
            family_admission_receipt_id,
            admission_owner_receipt_id,
            admission_projection_id,
            admission_link_pre_semantic_id,
            admission_link_post_semantic_id,
            terminal_owner_receipt_id: Id::ZERO,
            terminal_projection_id: Id::ZERO,
            terminal_link_pre_semantic_id: Id::ZERO,
            terminal_link_post_semantic_id: Id::ZERO,
            terminal_state_receipt_id: Id::ZERO,
            admission_link_transition_sequence,
            terminal_link_transition_sequence: 0,
            phase: DealerSeriesObligationPhaseV1::Live,
            rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate live-versus-terminal receipt exhaustiveness.
    pub fn validate(&self) -> Result<()> {
        self.key.validate()?;
        self.rent.validate()?;
        for identity in [
            self.owner_prewrite_id,
            self.family_admission_receipt_id,
            self.admission_owner_receipt_id,
            self.admission_projection_id,
            self.admission_link_pre_semantic_id,
            self.admission_link_post_semantic_id,
        ] {
            identity.validate_live()?;
        }
        if self.rent.payer == self.rent.neutral_sink
            || self.admission_link_pre_semantic_id == self.admission_link_post_semantic_id
            || self.admission_link_transition_sequence == 0
        {
            return Err(Error::MismatchedBinding);
        }
        let terminal = [
            self.terminal_owner_receipt_id,
            self.terminal_projection_id,
            self.terminal_link_pre_semantic_id,
            self.terminal_link_post_semantic_id,
            self.terminal_state_receipt_id,
        ];
        match self.phase {
            DealerSeriesObligationPhaseV1::Live => {
                if terminal.iter().any(|identity| !identity.is_zero())
                    || self.terminal_link_transition_sequence != 0
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerSeriesObligationPhaseV1::Terminal => {
                for identity in terminal {
                    identity.validate_live()?;
                }
                if self.terminal_link_pre_semantic_id == self.terminal_link_post_semantic_id
                    || self.terminal_link_transition_sequence
                        <= self.admission_link_transition_sequence
                {
                    return Err(Error::InvalidPhase);
                }
            }
        }
        Ok(())
    }

    /// Exact current binding identity retained by Dealer StateV3.
    pub fn binding_id(&self) -> Result<Id> {
        self.content_id(DEALER_SERIES_OBLIGATION_CONTENT_DOMAIN_V3)
    }
}

impl FixedCodec for DealerSeriesObligationBindingV3 {
    const ENCODED_LEN: usize = DEALER_SERIES_OBLIGATION_BYTES_V3;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_SERIES_OBLIGATION_MAGIC_V3,
            DEALER_SERIES_OBLIGATION_VERSION_V3,
        );
        for identity in [
            self.key.binding_account_id,
            self.key.policy_id,
            self.key.facility_id,
            self.key.dealer_state_account_id,
            self.key.facility_position_binding_id,
            self.key.market_instance_v2_id,
            self.key.product_market_root_account_id,
            self.key.product_market_binding_id,
            self.key.series_plan_v5_id,
            self.key.series_market_link_account_id,
            self.key.compiler_bundle_v7_id,
            self.key.attachment_plan_v6_id,
            self.key.obligation_configuration_v3_id,
            self.owner_prewrite_id,
            self.family_admission_receipt_id,
            self.admission_owner_receipt_id,
            self.admission_projection_id,
            self.admission_link_pre_semantic_id,
            self.admission_link_post_semantic_id,
            self.terminal_owner_receipt_id,
            self.terminal_projection_id,
            self.terminal_link_pre_semantic_id,
            self.terminal_link_post_semantic_id,
            self.terminal_state_receipt_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.key.product_generation);
        writer.u64(self.admission_link_transition_sequence);
        writer.u64(self.terminal_link_transition_sequence);
        writer.u32(self.key.series_ordinal);
        writer.u8(self.phase.byte());
        writer.reserved(3);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_SERIES_OBLIGATION_MAGIC_V3,
            DEALER_SERIES_OBLIGATION_VERSION_V3,
        )?;
        let mut identities = [Id::ZERO; 24];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index] = reader.id();
            index += 1;
        }
        let mut value = Self {
            key: DealerSeriesObligationKeyV3 {
                binding_account_id: identities[0],
                policy_id: identities[1],
                facility_id: identities[2],
                dealer_state_account_id: identities[3],
                facility_position_binding_id: identities[4],
                market_instance_v2_id: identities[5],
                product_market_root_account_id: identities[6],
                product_market_binding_id: identities[7],
                series_plan_v5_id: identities[8],
                series_market_link_account_id: identities[9],
                compiler_bundle_v7_id: identities[10],
                attachment_plan_v6_id: identities[11],
                obligation_configuration_v3_id: identities[12],
                product_generation: reader.u64(),
                series_ordinal: 0,
            },
            owner_prewrite_id: identities[13],
            family_admission_receipt_id: identities[14],
            admission_owner_receipt_id: identities[15],
            admission_projection_id: identities[16],
            admission_link_pre_semantic_id: identities[17],
            admission_link_post_semantic_id: identities[18],
            terminal_owner_receipt_id: identities[19],
            terminal_projection_id: identities[20],
            terminal_link_pre_semantic_id: identities[21],
            terminal_link_post_semantic_id: identities[22],
            terminal_state_receipt_id: identities[23],
            admission_link_transition_sequence: reader.u64(),
            terminal_link_transition_sequence: reader.u64(),
            phase: DealerSeriesObligationPhaseV1::Live,
            rent: DeletableRentOwnerV1 {
                payer: Id::ZERO,
                neutral_sink: Id::ZERO,
                refundable_principal: 0,
                donation_floor: 0,
            },
        };
        value.key.series_ordinal = reader.u32();
        value.phase = DealerSeriesObligationPhaseV1::decode(reader.u8())?;
        reader.reserved(3)?;
        value.rent = DeletableRentOwnerV1::decode_body(&mut reader);
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Immutable facility-to-Product coordinates authenticated at the SBF edge.
///
/// This key does not authenticate accounts by itself.  The program constructs
/// it only after decoding the current Product root, link, Series, attachment,
/// General Market binding, Dealer State, and Position-purpose binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationKeyV1 {
    /// Physical rent-owned binding account.
    pub binding_account_id: Id,
    /// Exact immutable Dealer policy.
    pub policy_id: Id,
    /// Exact immutable Dealer facility.
    pub facility_id: Id,
    /// Authoritative Dealer State account.
    pub dealer_state_account_id: Id,
    /// Canonical Position V3 purpose binding.
    pub facility_position_binding_id: Id,
    /// Full Product MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact Product MarketLifecycleRoot account.
    pub product_market_root_account_id: Id,
    /// Semantic Product Market lifecycle binding.
    pub product_market_binding_id: Id,
    /// Exact SeriesPlanV5 identity.
    pub series_plan_v5_id: Id,
    /// Exact Product SeriesMarketLink account.
    pub series_market_link_account_id: Id,
    /// Exact authenticated SeriesAttachmentPlanV4 identity.
    pub attachment_plan_v4_id: Id,
    /// Product Market lifecycle generation selected by founding authority.
    pub product_generation: u64,
    /// Exact ordinal whose start bucket rederives the MarketInstanceV2.
    pub series_ordinal: u32,
}

/// Exact atomic close plan after Product terminal consumption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationClosePlanV1 {
    /// Authoritative State postimage with the child count decremented.
    pub state_after: DealerStateV3,
    /// Deleted physical binding account.
    pub binding_account_id: Id,
    /// Retained terminal content identity.
    pub terminal_binding_id: Id,
    /// Sole refundable-principal recipient.
    pub rent_payer: Id,
    /// Exact refundable principal.
    pub rent_payer_credit_lamports: u64,
    /// Immutable hostile-prefund/surplus sink.
    pub neutral_sink: Id,
    /// Donation floor plus any later surplus.
    pub neutral_sink_credit_lamports: u64,
}

/// Exact close plan for a live current obligation after Dealer value custody
/// has already reached its terminal postimage in the same instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationValueClosePlanV3 {
    /// Authoritative State postimage with the Dealer obligation child removed.
    pub state_after: DealerStateV3,
    /// Deleted physical binding account.
    pub binding_account_id: Id,
    /// Semantic identity of the live binding that was physically removed.
    pub live_binding_id: Id,
    /// Exact Dealer value-terminal receipt authorizing this local close.
    pub dealer_value_terminal_receipt_id: Id,
    /// Canonical receipt for the value-authorized obligation deletion.
    pub close_receipt_id: Id,
    /// Sole refundable-principal recipient.
    pub rent_payer: Id,
    /// Exact refundable principal.
    pub rent_payer_credit_lamports: u64,
    /// Immutable hostile-prefund/surplus sink.
    pub neutral_sink: Id,
    /// Donation floor plus any later surplus.
    pub neutral_sink_credit_lamports: u64,
}

/// Close the counted terminal binding without losing Product admission or
/// terminal evidence.  The adapter must apply the State write, both coalesced
/// credits, and binding-account deletion in one rollback domain.
pub fn prepare_dealer_series_obligation_close_v1(
    state: DealerStateV3,
    binding: &DealerSeriesObligationBindingV1,
    binding_lamports_before: u64,
) -> Result<DealerSeriesObligationClosePlanV1> {
    binding.validate()?;
    let floor = add(
        binding.rent.refundable_principal,
        binding.rent.donation_floor,
    )?;
    if binding.phase != DealerSeriesObligationPhaseV1::Terminal
        || binding_lamports_before < floor
    {
        return Err(Error::InvalidPhase);
    }
    let state_after = state.close_terminal_binding(binding)?;
    Ok(DealerSeriesObligationClosePlanV1 {
        state_after,
        binding_account_id: binding.key.binding_account_id,
        terminal_binding_id: binding.binding_id()?,
        rent_payer: binding.rent.payer,
        rent_payer_credit_lamports: binding.rent.refundable_principal,
        neutral_sink: binding.rent.neutral_sink,
        neutral_sink_credit_lamports: binding_lamports_before
            .checked_sub(binding.rent.refundable_principal)
            .ok_or(Error::ArithmeticOverflow)?,
    })
}

/// Close the counted current Product RootV2/LinkV2 terminal binding.
///
/// Product owns the LinkV2 terminal mutation. This pure plan may be applied
/// only after the adapter hostile-reopens that exact postwrite and must be
/// committed atomically with the StateV3 child decrement, refundable-principal
/// credit, donation/surplus disposition, and physical `0xaf/v2` deletion.
pub fn prepare_dealer_series_obligation_close_v2(
    state: DealerStateV3,
    binding: &DealerSeriesObligationBindingV2,
    binding_lamports_before: u64,
) -> Result<DealerSeriesObligationClosePlanV1> {
    binding.validate()?;
    let floor = add(
        binding.rent.refundable_principal,
        binding.rent.donation_floor,
    )?;
    if binding.phase != DealerSeriesObligationPhaseV1::Terminal
        || binding_lamports_before < floor
    {
        return Err(Error::InvalidPhase);
    }
    let state_after = state.close_current_terminal_binding(binding)?;
    Ok(DealerSeriesObligationClosePlanV1 {
        state_after,
        binding_account_id: binding.key.binding_account_id,
        terminal_binding_id: binding.binding_id()?,
        rent_payer: binding.rent.payer,
        rent_payer_credit_lamports: binding.rent.refundable_principal,
        neutral_sink: binding.rent.neutral_sink,
        neutral_sink_credit_lamports: binding_lamports_before
            .checked_sub(binding.rent.refundable_principal)
            .ok_or(Error::ArithmeticOverflow)?,
    })
}

/// Close Dealer's live current obligation after physical value terminalization.
///
/// This plan deliberately accepts no Product receipt, Root, Link, or caller
/// authority. The exact value receipt is derived by Dealer's SBF adapter from
/// either Fractional custody execution or unused-funding deletion, and Product
/// must consume the resulting move-only family receipt in the same outer.
pub fn prepare_dealer_series_obligation_value_close_v3(
    state: DealerStateV3,
    binding: &DealerSeriesObligationBindingV2,
    dealer_value_terminal_receipt_id: Id,
    binding_lamports_before: u64,
) -> Result<DealerSeriesObligationValueClosePlanV3> {
    binding.validate()?;
    dealer_value_terminal_receipt_id.validate_live()?;
    let floor = add(
        binding.rent.refundable_principal,
        binding.rent.donation_floor,
    )?;
    if binding.phase != DealerSeriesObligationPhaseV1::Live
        || binding_lamports_before < floor
    {
        return Err(Error::InvalidPhase);
    }
    let live_binding_id = binding.binding_id()?;
    let state_after = state.close_current_live_binding_after_value(
        binding,
        dealer_value_terminal_receipt_id,
    )?;
    let state_after_id = state_after.state_id()?;
    let mut hasher = Sha256::new();
    hasher.update(DEALER_SERIES_VALUE_CLOSE_RECEIPT_DOMAIN_V3);
    hasher.update(binding.key.binding_account_id.bytes());
    hasher.update(live_binding_id.bytes());
    hasher.update(dealer_value_terminal_receipt_id.bytes());
    hasher.update(state_after_id.bytes());
    hasher.update(binding_lamports_before.to_le_bytes());
    let close_receipt_id = Id::from_bytes(hasher.finalize().into());
    close_receipt_id.validate_live()?;
    Ok(DealerSeriesObligationValueClosePlanV3 {
        state_after,
        binding_account_id: binding.key.binding_account_id,
        live_binding_id,
        dealer_value_terminal_receipt_id,
        close_receipt_id,
        rent_payer: binding.rent.payer,
        rent_payer_credit_lamports: binding.rent.refundable_principal,
        neutral_sink: binding.rent.neutral_sink,
        neutral_sink_credit_lamports: binding_lamports_before
            .checked_sub(binding.rent.refundable_principal)
            .ok_or(Error::ArithmeticOverflow)?,
    })
}

impl DealerSeriesObligationKeyV1 {
    /// Validate live, pairwise-distinct physical/semantic owners and a
    /// nonzero Product generation.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.binding_account_id,
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_binding_id,
            self.market_instance_v2_id,
            self.product_market_root_account_id,
            self.product_market_binding_id,
            self.series_plan_v5_id,
            self.series_market_link_account_id,
            self.attachment_plan_v4_id,
        ];
        for identity in identities {
            identity.validate_live()?;
        }
        let physical = [
            self.binding_account_id,
            self.dealer_state_account_id,
            self.product_market_root_account_id,
            self.series_market_link_account_id,
        ];
        let mut left = 0usize;
        while left < physical.len() {
            let mut right = left + 1;
            while right < physical.len() {
                if physical[left] == physical[right] {
                    return Err(Error::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        if self.product_generation == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Derive the family-owned receipt supplied to Product's first Dealer
    /// obligation admission.  The adapter must still authenticate Product's
    /// projection and resulting link postimage.
    pub fn admission_owner_receipt_id(
        &self,
        link_pre_semantic_id: Id,
        link_transition_sequence: u64,
    ) -> Result<Id> {
        self.validate()?;
        link_pre_semantic_id.validate_live()?;
        if link_transition_sequence == 0 {
            return Err(Error::InvalidParameter);
        }
        let mut hasher = Sha256::new();
        hasher.update(DEALER_SERIES_ADMISSION_RECEIPT_DOMAIN_V1);
        hash_key(self, &mut hasher);
        hasher.update(link_pre_semantic_id.bytes());
        hasher.update(link_transition_sequence.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

/// Exhaustive facility-level obligation phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerSeriesObligationPhaseV1 {
    /// Product Dealer obligation is live.
    Live,
    /// Facility terminal receipt consumed the Product obligation.
    Terminal,
}

impl DealerSeriesObligationPhaseV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Live => 1,
            Self::Terminal => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Live),
            2 => Ok(Self::Terminal),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Persistent facility-lifetime proof of the one Product Dealer obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeriesObligationBindingV1 {
    /// Immutable authenticated coordinate set.
    pub key: DealerSeriesObligationKeyV1,
    /// Dealer-owned admission receipt passed into Product.
    pub admission_owner_receipt_id: Id,
    /// Product-owned admission projection identity.
    pub admission_projection_id: Id,
    /// Exact link state before Dealer admission.
    pub admission_link_pre_semantic_id: Id,
    /// Exact link state after Dealer admission.
    pub admission_link_post_semantic_id: Id,
    /// Dealer-owned facility terminal receipt, zero while live.
    pub terminal_owner_receipt_id: Id,
    /// Product-owned terminal projection identity, zero while live.
    pub terminal_projection_id: Id,
    /// Exact link state before terminal consumption, zero while live.
    pub terminal_link_pre_semantic_id: Id,
    /// Exact link state after terminal consumption, zero while live.
    pub terminal_link_post_semantic_id: Id,
    /// Exact terminal Dealer State receipt, zero while live.
    pub terminal_state_receipt_id: Id,
    /// Product link sequence after first Dealer admission.
    pub admission_link_transition_sequence: u64,
    /// Product link sequence after terminal consumption, zero while live.
    pub terminal_link_transition_sequence: u64,
    /// Exhaustive lifecycle phase.
    pub phase: DealerSeriesObligationPhaseV1,
    /// Exact refundable rent owner and hostile-prefund sink.
    pub rent: DeletableRentOwnerV1,
}

impl DealerSeriesObligationBindingV1 {
    /// Construct a live binding from adapter-authenticated Product pre/post
    /// state and the exact Product admission projection identity.
    #[allow(clippy::too_many_arguments)]
    pub fn new_live(
        key: DealerSeriesObligationKeyV1,
        admission_owner_receipt_id: Id,
        admission_projection_id: Id,
        admission_link_pre_semantic_id: Id,
        admission_link_post_semantic_id: Id,
        admission_link_transition_sequence: u64,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        let value = Self {
            key,
            admission_owner_receipt_id,
            admission_projection_id,
            admission_link_pre_semantic_id,
            admission_link_post_semantic_id,
            terminal_owner_receipt_id: Id::ZERO,
            terminal_projection_id: Id::ZERO,
            terminal_link_pre_semantic_id: Id::ZERO,
            terminal_link_post_semantic_id: Id::ZERO,
            terminal_state_receipt_id: Id::ZERO,
            admission_link_transition_sequence,
            terminal_link_transition_sequence: 0,
            phase: DealerSeriesObligationPhaseV1::Live,
            rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Derive the owner receipt for Product terminal consumption from the
    /// exact terminal State receipt and the current Product link state.
    pub fn terminal_owner_receipt_id(
        &self,
        terminal_state_receipt_id: Id,
        terminal_link_pre_semantic_id: Id,
        terminal_link_transition_sequence: u64,
    ) -> Result<Id> {
        self.validate()?;
        terminal_state_receipt_id.validate_live()?;
        terminal_link_pre_semantic_id.validate_live()?;
        if self.phase != DealerSeriesObligationPhaseV1::Live
            || terminal_link_transition_sequence <= self.admission_link_transition_sequence
        {
            return Err(Error::InvalidPhase);
        }
        self.derive_terminal_owner_receipt_id(
            terminal_state_receipt_id,
            terminal_link_pre_semantic_id,
            terminal_link_transition_sequence,
        )
    }

    fn derive_terminal_owner_receipt_id(
        &self,
        terminal_state_receipt_id: Id,
        terminal_link_pre_semantic_id: Id,
        terminal_link_transition_sequence: u64,
    ) -> Result<Id> {
        let mut hasher = Sha256::new();
        hasher.update(DEALER_SERIES_TERMINAL_RECEIPT_DOMAIN_V1);
        hasher.update(self.admission_state_id()?.bytes());
        hasher.update(terminal_state_receipt_id.bytes());
        hasher.update(terminal_link_pre_semantic_id.bytes());
        hasher.update(terminal_link_transition_sequence.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }

    fn admission_state_id(&self) -> Result<Id> {
        self.key.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(b"dragons-clutch/dealer-runtime/series-obligation-live-state/v1\0");
        hash_key(&self.key, &mut hasher);
        for identity in [
            self.admission_owner_receipt_id,
            self.admission_projection_id,
            self.admission_link_pre_semantic_id,
            self.admission_link_post_semantic_id,
        ] {
            identity.validate_live()?;
            hasher.update(identity.bytes());
        }
        hasher.update(self.admission_link_transition_sequence.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }

    /// Consume the exact Product obligation after facility-level Dealer
    /// terminality.  Lease finalization and abort cannot call this transition.
    #[allow(clippy::too_many_arguments)]
    pub fn terminalize(
        self,
        terminal_owner_receipt_id: Id,
        terminal_projection_id: Id,
        terminal_link_pre_semantic_id: Id,
        terminal_link_post_semantic_id: Id,
        terminal_state_receipt_id: Id,
        terminal_link_transition_sequence: u64,
    ) -> Result<Self> {
        let expected = self.terminal_owner_receipt_id(
            terminal_state_receipt_id,
            terminal_link_pre_semantic_id,
            terminal_link_transition_sequence,
        )?;
        if terminal_owner_receipt_id != expected {
            return Err(Error::MismatchedBinding);
        }
        let next = Self {
            terminal_owner_receipt_id,
            terminal_projection_id,
            terminal_link_pre_semantic_id,
            terminal_link_post_semantic_id,
            terminal_state_receipt_id,
            terminal_link_transition_sequence,
            phase: DealerSeriesObligationPhaseV1::Terminal,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Validate the immutable join and exhaustive phase-dependent fields.
    pub fn validate(&self) -> Result<()> {
        self.key.validate()?;
        self.rent.validate()?;
        for identity in [
            self.admission_owner_receipt_id,
            self.admission_projection_id,
            self.admission_link_pre_semantic_id,
            self.admission_link_post_semantic_id,
        ] {
            identity.validate_live()?;
        }
        if self.admission_link_pre_semantic_id == self.admission_link_post_semantic_id
            || self.admission_link_transition_sequence == 0
            || self.admission_owner_receipt_id
                != self.key.admission_owner_receipt_id(
                    self.admission_link_pre_semantic_id,
                    self.admission_link_transition_sequence,
                )?
            || self.rent.neutral_sink == self.key.policy_id
        {
            return Err(Error::MismatchedBinding);
        }
        let terminal = [
            self.terminal_owner_receipt_id,
            self.terminal_projection_id,
            self.terminal_link_pre_semantic_id,
            self.terminal_link_post_semantic_id,
            self.terminal_state_receipt_id,
        ];
        match self.phase {
            DealerSeriesObligationPhaseV1::Live => {
                if terminal.iter().any(|identity| !identity.is_zero())
                    || self.terminal_link_transition_sequence != 0
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerSeriesObligationPhaseV1::Terminal => {
                for identity in terminal {
                    identity.validate_live()?;
                }
                if self.terminal_link_pre_semantic_id == self.terminal_link_post_semantic_id
                    || self.terminal_link_transition_sequence
                        <= self.admission_link_transition_sequence
                    || self.terminal_owner_receipt_id
                        != self.derive_terminal_owner_receipt_id(
                            self.terminal_state_receipt_id,
                            self.terminal_link_pre_semantic_id,
                            self.terminal_link_transition_sequence,
                        )?
                {
                    return Err(Error::MismatchedBinding);
                }
            }
        }
        Ok(())
    }

    /// Exact content identity of the current binding state.
    pub fn binding_id(&self) -> Result<Id> {
        self.content_id(DEALER_SERIES_OBLIGATION_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for DealerSeriesObligationBindingV1 {
    const ENCODED_LEN: usize = DEALER_SERIES_OBLIGATION_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_SERIES_OBLIGATION_MAGIC_V1,
            DEALER_SERIES_OBLIGATION_VERSION_V1,
        );
        for identity in [
            self.key.binding_account_id,
            self.key.policy_id,
            self.key.facility_id,
            self.key.dealer_state_account_id,
            self.key.facility_position_binding_id,
            self.key.market_instance_v2_id,
            self.key.product_market_root_account_id,
            self.key.product_market_binding_id,
            self.key.series_plan_v5_id,
            self.key.series_market_link_account_id,
            self.key.attachment_plan_v4_id,
            self.admission_owner_receipt_id,
            self.admission_projection_id,
            self.admission_link_pre_semantic_id,
            self.admission_link_post_semantic_id,
            self.terminal_owner_receipt_id,
            self.terminal_projection_id,
            self.terminal_link_pre_semantic_id,
            self.terminal_link_post_semantic_id,
            self.terminal_state_receipt_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.key.product_generation);
        writer.u64(self.admission_link_transition_sequence);
        writer.u64(self.terminal_link_transition_sequence);
        writer.u32(self.key.series_ordinal);
        writer.u8(self.phase.byte());
        writer.reserved(3);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_SERIES_OBLIGATION_MAGIC_V1,
            DEALER_SERIES_OBLIGATION_VERSION_V1,
        )?;
        let value = Self {
            key: DealerSeriesObligationKeyV1 {
                binding_account_id: reader.id(),
                policy_id: reader.id(),
                facility_id: reader.id(),
                dealer_state_account_id: reader.id(),
                facility_position_binding_id: reader.id(),
                market_instance_v2_id: reader.id(),
                product_market_root_account_id: reader.id(),
                product_market_binding_id: reader.id(),
                series_plan_v5_id: reader.id(),
                series_market_link_account_id: reader.id(),
                attachment_plan_v4_id: reader.id(),
                product_generation: 0,
                series_ordinal: 0,
            },
            admission_owner_receipt_id: reader.id(),
            admission_projection_id: reader.id(),
            admission_link_pre_semantic_id: reader.id(),
            admission_link_post_semantic_id: reader.id(),
            terminal_owner_receipt_id: reader.id(),
            terminal_projection_id: reader.id(),
            terminal_link_pre_semantic_id: reader.id(),
            terminal_link_post_semantic_id: reader.id(),
            terminal_state_receipt_id: reader.id(),
            admission_link_transition_sequence: 0,
            terminal_link_transition_sequence: 0,
            phase: DealerSeriesObligationPhaseV1::Live,
            rent: DeletableRentOwnerV1 {
                payer: Id::ZERO,
                neutral_sink: Id::ZERO,
                refundable_principal: 0,
                donation_floor: 0,
            },
        };
        let mut value = value;
        value.key.product_generation = reader.u64();
        value.admission_link_transition_sequence = reader.u64();
        value.terminal_link_transition_sequence = reader.u64();
        value.key.series_ordinal = reader.u32();
        value.phase = DealerSeriesObligationPhaseV1::decode(reader.u8())?;
        reader.reserved(3)?;
        value.rent = DeletableRentOwnerV1::decode_body(&mut reader);
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

fn hash_key(key: &DealerSeriesObligationKeyV1, hasher: &mut Sha256) {
    for identity in [
        key.binding_account_id,
        key.policy_id,
        key.facility_id,
        key.dealer_state_account_id,
        key.facility_position_binding_id,
        key.market_instance_v2_id,
        key.product_market_root_account_id,
        key.product_market_binding_id,
        key.series_plan_v5_id,
        key.series_market_link_account_id,
        key.attachment_plan_v4_id,
    ] {
        hasher.update(identity.bytes());
    }
    hasher.update(key.product_generation.to_le_bytes());
    hasher.update(key.series_ordinal.to_le_bytes());
}

fn hash_key_v2(key: &DealerSeriesObligationKeyV2, hasher: &mut Sha256) {
    for identity in [
        key.binding_account_id,
        key.policy_id,
        key.facility_id,
        key.dealer_state_account_id,
        key.facility_position_binding_id,
        key.market_instance_v2_id,
        key.product_market_root_account_id,
        key.product_market_binding_id,
        key.series_plan_v5_id,
        key.series_market_link_account_id,
        key.compiler_bundle_v6_id,
        key.attachment_plan_v5_id,
    ] {
        hasher.update(identity.bytes());
    }
    hasher.update(key.product_generation.to_le_bytes());
    hasher.update(key.series_ordinal.to_le_bytes());
}

const _: () = assert!(DEALER_SERIES_OBLIGATION_BYTES_V1 == 764);
const _: () = assert!(DEALER_SERIES_OBLIGATION_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_SERIES_OBLIGATION_BYTES_V2 == 796);
const _: () = assert!(DEALER_SERIES_OBLIGATION_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_SERIES_OBLIGATION_BYTES_V3 == 892);
const _: () = assert!(DEALER_SERIES_OBLIGATION_BYTES_V3 <= crate::MAX_SEMANTIC_BODY_BYTES);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn key() -> DealerSeriesObligationKeyV1 {
        DealerSeriesObligationKeyV1 {
            binding_account_id: id(1),
            policy_id: id(2),
            facility_id: id(3),
            dealer_state_account_id: id(4),
            facility_position_binding_id: id(5),
            market_instance_v2_id: id(6),
            product_market_root_account_id: id(7),
            product_market_binding_id: id(8),
            series_plan_v5_id: id(9),
            series_market_link_account_id: id(10),
            attachment_plan_v4_id: id(11),
            product_generation: 12,
            series_ordinal: 13,
        }
    }

    fn live() -> DealerSeriesObligationBindingV1 {
        let key = key();
        let pre = id(14);
        DealerSeriesObligationBindingV1::new_live(
            key,
            key.admission_owner_receipt_id(pre, 15).unwrap(),
            id(16),
            pre,
            id(17),
            15,
            DeletableRentOwnerV1 {
                payer: id(18),
                neutral_sink: id(19),
                refundable_principal: 20,
                donation_floor: 21,
            },
        )
        .unwrap()
    }

    fn current_live() -> DealerSeriesObligationBindingV2 {
        let key = DealerSeriesObligationKeyV2 {
                binding_account_id: id(31),
                policy_id: id(32),
                facility_id: id(33),
                dealer_state_account_id: id(34),
                facility_position_binding_id: id(35),
                market_instance_v2_id: id(36),
                product_market_root_account_id: id(37),
                product_market_binding_id: id(38),
                series_plan_v5_id: id(39),
                series_market_link_account_id: id(40),
                compiler_bundle_v6_id: id(41),
                attachment_plan_v5_id: id(42),
                product_generation: 43,
                series_ordinal: 44,
            };
        DealerSeriesObligationBindingV2::new_live(
            key,
            key.admission_owner_receipt_id(id(47), 49).unwrap(),
            id(46),
            id(47),
            id(48),
            49,
            DeletableRentOwnerV1 {
                payer: id(50),
                neutral_sink: id(51),
                refundable_principal: 52,
                donation_floor: 53,
            },
        )
        .unwrap()
    }

    fn product_v3_live() -> DealerSeriesObligationBindingV3 {
        DealerSeriesObligationBindingV3::new_live(
            DealerSeriesObligationKeyV3 {
                binding_account_id: id(61),
                policy_id: id(62),
                facility_id: id(63),
                dealer_state_account_id: id(64),
                facility_position_binding_id: id(65),
                market_instance_v2_id: id(66),
                product_market_root_account_id: id(67),
                product_market_binding_id: id(68),
                series_plan_v5_id: id(69),
                series_market_link_account_id: id(70),
                compiler_bundle_v7_id: id(71),
                attachment_plan_v6_id: id(72),
                obligation_configuration_v3_id: id(73),
                product_generation: 74,
                series_ordinal: 75,
            },
            id(76),
            id(77),
            id(78),
            id(79),
            id(80),
            id(81),
            82,
            DeletableRentOwnerV1 {
                payer: id(83),
                neutral_sink: id(84),
                refundable_principal: 85,
                donation_floor: 86,
            },
        )
        .unwrap()
    }

    #[test]
    fn live_and_terminal_round_trip_preserve_exact_product_postwrites() {
        let live = live();
        let mut bytes = [0u8; DEALER_SERIES_OBLIGATION_BYTES_V1];
        live.encode_into(&mut bytes).unwrap();
        assert_eq!(DealerSeriesObligationBindingV1::decode(&bytes), Ok(live));

        let state_receipt = id(22);
        let terminal_pre = id(23);
        let owner_receipt = live
            .terminal_owner_receipt_id(state_receipt, terminal_pre, 24)
            .unwrap();
        let terminal = live
            .terminalize(owner_receipt, id(25), terminal_pre, id(26), state_receipt, 24)
            .unwrap();
        terminal.encode_into(&mut bytes).unwrap();
        assert_eq!(
            DealerSeriesObligationBindingV1::decode(&bytes),
            Ok(terminal)
        );
    }

    #[test]
    fn hostile_alias_receipt_swap_padding_and_replay_refuse() {
        let mut alias = key();
        alias.series_market_link_account_id = alias.product_market_root_account_id;
        assert_eq!(alias.validate(), Err(Error::MismatchedBinding));

        let live = live();
        assert_eq!(
            DealerSeriesObligationBindingV1::new_live(
                live.key,
                id(27),
                live.admission_projection_id,
                live.admission_link_pre_semantic_id,
                live.admission_link_post_semantic_id,
                live.admission_link_transition_sequence,
                live.rent,
            ),
            Err(Error::MismatchedBinding)
        );
        assert_eq!(
            live.terminalize(id(28), id(29), id(30), id(31), id(32), 33),
            Err(Error::MismatchedBinding)
        );

        let mut bytes = [0u8; DEALER_SERIES_OBLIGATION_BYTES_V1];
        live.encode_into(&mut bytes).unwrap();
        let phase_offset = HEADER_BYTES + (20 * 32) + (3 * 8) + 4;
        bytes[phase_offset + 1] = 1;
        assert_eq!(
            DealerSeriesObligationBindingV1::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );
        assert_eq!(
            DealerSeriesObligationBindingV1::decode(&bytes[..bytes.len() - 1]),
            Err(Error::Truncated)
        );
    }

    #[test]
    fn current_product_obligation_round_trip_binds_bundle_and_attachment() {
        let live = current_live();
        let mut bytes = [0u8; DEALER_SERIES_OBLIGATION_BYTES_V2];
        live.encode_into(&mut bytes).unwrap();
        assert_eq!(DealerSeriesObligationBindingV2::decode(&bytes), Ok(live));

        let mut substituted = live;
        substituted.key.compiler_bundle_v6_id = id(54);
        assert_ne!(substituted.binding_id().unwrap(), live.binding_id().unwrap());
        substituted = live;
        substituted.key.attachment_plan_v5_id = id(55);
        assert_ne!(substituted.binding_id().unwrap(), live.binding_id().unwrap());
        assert_eq!(
            DealerSeriesObligationBindingV2::new_live(
                live.key,
                id(56),
                live.admission_projection_id,
                live.admission_link_pre_semantic_id,
                live.admission_link_post_semantic_id,
                live.admission_link_transition_sequence,
                live.rent,
            ),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn product_v3_obligation_round_trip_binds_both_generic_admissions() {
        let live = product_v3_live();
        let mut bytes = [0u8; DEALER_SERIES_OBLIGATION_BYTES_V3];
        live.encode_into(&mut bytes).unwrap();
        assert_eq!(DealerSeriesObligationBindingV3::decode(&bytes), Ok(live));

        let mut substituted = live;
        substituted.owner_prewrite_id = id(87);
        assert_ne!(substituted.binding_id().unwrap(), live.binding_id().unwrap());
        substituted = live;
        substituted.family_admission_receipt_id = id(88);
        assert_ne!(substituted.binding_id().unwrap(), live.binding_id().unwrap());
        substituted = live;
        substituted.key.obligation_configuration_v3_id = id(89);
        assert_ne!(substituted.binding_id().unwrap(), live.binding_id().unwrap());
    }

    #[test]
    fn product_v3_obligation_parser_refuses_alias_padding_and_truncation() {
        let live = product_v3_live();
        let mut aliased = live;
        aliased.key.series_market_link_account_id = aliased.key.product_market_root_account_id;
        assert_eq!(aliased.validate(), Err(Error::MismatchedBinding));

        let mut bytes = [0u8; DEALER_SERIES_OBLIGATION_BYTES_V3];
        live.encode_into(&mut bytes).unwrap();
        let phase_offset = HEADER_BYTES + (24 * 32) + (3 * 8) + 4;
        bytes[phase_offset + 1] = 1;
        assert_eq!(
            DealerSeriesObligationBindingV3::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );
        assert_eq!(
            DealerSeriesObligationBindingV3::decode(&bytes[..bytes.len() - 1]),
            Err(Error::Truncated)
        );
    }

    #[test]
    fn current_product_obligation_refuses_terminal_or_padding_substitution() {
        let live = current_live();
        let mut terminal = live;
        terminal.phase = DealerSeriesObligationPhaseV1::Terminal;
        assert_eq!(terminal.validate(), Err(Error::ZeroIdentity));

        let mut bytes = [0u8; DEALER_SERIES_OBLIGATION_BYTES_V2];
        live.encode_into(&mut bytes).unwrap();
        let phase_offset = HEADER_BYTES + (21 * 32) + (3 * 8) + 4;
        bytes[phase_offset + 1] = 1;
        assert_eq!(
            DealerSeriesObligationBindingV2::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );
    }

    #[test]
    fn current_product_terminal_successor_is_once_only_and_sequence_ordered() {
        let live = current_live();
        let owner_receipt = live
            .terminal_owner_receipt_id(id(60), id(58), 50)
            .unwrap();
        assert_eq!(
            live.terminalized(owner_receipt, id(57), id(58), id(59), id(60), 49),
            Err(Error::InvalidPhase)
        );
        let terminal = live
            .terminalized(owner_receipt, id(57), id(58), id(59), id(60), 50)
            .unwrap();
        assert_eq!(terminal.phase, DealerSeriesObligationPhaseV1::Terminal);
        assert_eq!(
            terminal.terminalized(id(61), id(62), id(63), id(64), id(65), 51),
            Err(Error::InvalidPhase)
        );
    }
}
