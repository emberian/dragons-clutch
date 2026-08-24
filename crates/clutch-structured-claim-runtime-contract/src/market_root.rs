//! Series-link-scoped Structured market root.
//!
//! Product authenticates the exact active RootV3-bound Series link, BundleV7,
//! and AttachmentV6. Structured then owns this mutable descriptor-count root,
//! recipe membership, rent principal, donation residue, and terminal receipt.
//! A caller-authored value is never authority; the live adapter must construct
//! the authorization below only from Product's private authenticated receipt.

use clutch_product_series::{
    CompiledProductSeriesBundleV7Id, ContentId, MarketInstanceV2Id, SeriesAttachmentPlanV6Id,
    SeriesPlanV5Id,
};
use clutch_structured_claim::DeploymentBinding;

use crate::{
    put, Error, Result, StructuredMarketProjectionStateV1, StructuredMarketProjectionV1,
    WrapperRecipeHashV1,
};

/// Central Structured root account discriminator.
pub const STRUCTURED_MARKET_ROOT_ACCOUNT_TAG: u8 = 0xb7;
/// First Structured root account version.
pub const STRUCTURED_MARKET_ROOT_ACCOUNT_VERSION: u8 = 1;
/// Exact mutable Structured root account width.
pub const STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES: usize = 656;
/// Stable root-binding identity domain.
pub const STRUCTURED_MARKET_ROOT_BINDING_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/market-root-binding/v1\0";
/// Exact wrapper/base/Token-2022 deployment-owner release domain.
pub const STRUCTURED_OWNER_RELEASE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/owner-release/v1\0";
/// Current release-set identity domain. Unlike V1, this commits the three
/// content-addressed, locus-aware loader release artifacts.
pub const STRUCTURED_OWNER_RELEASE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/structured-claim/owner-release/v2\0";
/// Exact stable root-binding preimage width.
pub const STRUCTURED_MARKET_ROOT_BINDING_BYTES_V1: usize = 400;
/// Admission transcript domain.
pub const STRUCTURED_DESCRIPTOR_ADMISSION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/descriptor-admission/v1\0";
/// Terminal transcript domain.
pub const STRUCTURED_DESCRIPTOR_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/descriptor-terminal/v1\0";
/// Aggregate terminal-root receipt domain.
pub const STRUCTURED_MARKET_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/market-terminal/v1\0";
/// Exact terminal-root preimage width after excluding its recursive receipt.
pub const STRUCTURED_MARKET_TERMINAL_PREIMAGE_BYTES_V1: usize =
    STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES - 32;

/// Derive the sole deployment-owner release identity shared by both SBF
/// adapters. This commits the exact wrapper, base, and Token-2022 loader
/// deployments without placing Solana account parsing in the pure contract.
pub fn structured_owner_release_id_v1<H: WrapperRecipeHashV1>(
    binding: DeploymentBinding,
    hasher: &H,
) -> Result<ContentId> {
    binding.validate().map_err(|_| Error::InvalidIdentity)?;
    let wrapper_slot = binding.wrapper_deployment_slot.to_le_bytes();
    let base_slot = binding.base_deployment_slot.to_le_bytes();
    let token_slot = binding.token_2022_deployment_slot.to_le_bytes();
    let id = ContentId::from_bytes(hasher.hashv(&[
        STRUCTURED_OWNER_RELEASE_DOMAIN_V1,
        &binding.wrapper_program,
        &binding.wrapper_program_data,
        &wrapper_slot,
        &binding.base_program,
        &binding.base_program_data,
        &base_slot,
        &binding.token_2022_program,
        &binding.token_2022_program_data,
        &token_slot,
    ]));
    if id.is_zero() {
        return Err(Error::InvalidIdentity);
    }
    Ok(id)
}

/// Derive the current runtime-owner identity from three independently
/// authenticated loader releases plus the exact deployment addresses/slots
/// incorporated by the wrapper product identity.
pub fn structured_owner_release_id_v2<H: WrapperRecipeHashV1>(
    binding: DeploymentBinding,
    wrapper_release_id: ContentId,
    base_release_id: ContentId,
    token_release_id: ContentId,
    hasher: &H,
) -> Result<ContentId> {
    binding.validate().map_err(|_| Error::InvalidIdentity)?;
    let releases = [wrapper_release_id, base_release_id, token_release_id];
    let mut left = 0usize;
    while left < releases.len() {
        if releases[left].is_zero() {
            return Err(Error::InvalidIdentity);
        }
        let mut right = left + 1;
        while right < releases.len() {
            if releases[left] == releases[right] {
                return Err(Error::InvalidIdentity);
            }
            right += 1;
        }
        left += 1;
    }
    let wrapper_slot = binding.wrapper_deployment_slot.to_le_bytes();
    let base_slot = binding.base_deployment_slot.to_le_bytes();
    let token_slot = binding.token_2022_deployment_slot.to_le_bytes();
    let id = ContentId::from_bytes(hasher.hashv(&[
        STRUCTURED_OWNER_RELEASE_DOMAIN_V2,
        &binding.wrapper_program,
        &binding.wrapper_program_data,
        &wrapper_slot,
        &wrapper_release_id.bytes(),
        &binding.base_program,
        &binding.base_program_data,
        &base_slot,
        &base_release_id.bytes(),
        &binding.token_2022_program,
        &binding.token_2022_program_data,
        &token_slot,
        &token_release_id.bytes(),
    ]));
    if id.is_zero() {
        return Err(Error::InvalidIdentity);
    }
    Ok(id)
}

/// Stable immutable Product/Series authority for one Structured root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredMarketRootBindingV1 {
    /// Exact Product-owned Series link account.
    pub link_account: [u8; 32],
    /// Exact recurring Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact finite Series ordinal.
    pub ordinal: u32,
    /// Exact economic market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Product/Source generation.
    pub generation: u64,
    /// Exact successor attachment.
    pub attachment_plan_id: SeriesAttachmentPlanV6Id,
    /// Exact successor compiler bundle.
    pub compiler_output_id: CompiledProductSeriesBundleV7Id,
    /// Exact compiler semantic owner.
    pub compiler_release_id: ContentId,
    /// Exact loader-authenticated central Registry ReleaseV2.
    pub registry_release_id: ContentId,
    /// Exact central capability profile.
    pub capability_profile_id: ContentId,
    /// Exact Structured-owned recipe-set commitment.
    pub wrapper_recipe_set_id: ContentId,
    /// Exact Structured runtime owner release.
    pub owner_release_id: ContentId,
    /// Sole refundable root-rent owner.
    pub rent_refund_owner: ContentId,
    /// Sole neutral lamport donation sink.
    pub neutral_lamport_sink: ContentId,
}

impl StructuredMarketRootBindingV1 {
    /// Encode the stable PDA/hash binding.
    pub fn encode_preimage(self) -> Result<[u8; STRUCTURED_MARKET_ROOT_BINDING_BYTES_V1]> {
        self.validate()?;
        let mut output = [0_u8; STRUCTURED_MARKET_ROOT_BINDING_BYTES_V1];
        let mut cursor = 0_usize;
        for id in [
            self.link_account,
            self.series_plan_id.bytes(),
            self.market_instance_id.bytes(),
            self.attachment_plan_id.bytes(),
            self.compiler_output_id.bytes(),
            self.compiler_release_id.bytes(),
            self.registry_release_id.bytes(),
            self.capability_profile_id.bytes(),
            self.wrapper_recipe_set_id.bytes(),
            self.owner_release_id.bytes(),
            self.rent_refund_owner.bytes(),
            self.neutral_lamport_sink.bytes(),
        ] {
            put(&mut output, &mut cursor, &id)?;
        }
        put(&mut output, &mut cursor, &self.ordinal.to_le_bytes())?;
        put(&mut output, &mut cursor, &self.generation.to_le_bytes())?;
        put(&mut output, &mut cursor, &[0; 4])?;
        if cursor != output.len() {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    /// Stable root binding identity through the supplied SHA-256 boundary.
    pub fn id<H: WrapperRecipeHashV1 + ?Sized>(self, hasher: &H) -> Result<ContentId> {
        let preimage = self.encode_preimage()?;
        let id = ContentId::from_bytes(
            hasher.hashv(&[STRUCTURED_MARKET_ROOT_BINDING_DOMAIN_V1, &preimage]),
        );
        if id.is_zero() {
            return Err(Error::InvalidIdentity);
        }
        Ok(id)
    }

    fn validate(self) -> Result<()> {
        if self.generation == 0 {
            return Err(Error::InvalidIdentity);
        }
        let ids = [
            self.link_account,
            self.series_plan_id.bytes(),
            self.market_instance_id.bytes(),
            self.attachment_plan_id.bytes(),
            self.compiler_output_id.bytes(),
            self.compiler_release_id.bytes(),
            self.registry_release_id.bytes(),
            self.capability_profile_id.bytes(),
            self.wrapper_recipe_set_id.bytes(),
            self.owner_release_id.bytes(),
            self.rent_refund_owner.bytes(),
            self.neutral_lamport_sink.bytes(),
        ];
        let mut left = 0_usize;
        while left < ids.len() {
            if ids[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < ids.len() {
                if ids[left] == ids[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }
}

/// Immutable Product link/Wrapper admission authority plus one monotone audit
/// sequence. Sibling obligations, Failure sessions, and lamport donations may
/// legitimately advance the shared link while these immutable identities remain
/// unchanged; the sequence is never a whole-link liveness lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredProductLineageV1 {
    /// Immutable complete Product link binding identity.
    pub link_binding_id: ContentId,
    /// Immutable configuration governing Product's Wrapper obligation.
    pub wrapper_obligation_configuration_id: ContentId,
    /// Persisted Product projection which moved Wrapper from never-founded to
    /// live. This is deliberately distinct from Structured's first descriptor
    /// admission transcript.
    pub product_admission_receipt_id: ContentId,
    /// Most recently observed Product link sequence, retained for audit only.
    /// Live authentication permits unrelated monotone sibling transitions.
    pub last_observed_link_transition_sequence: u64,
}

impl StructuredProductLineageV1 {
    fn validate(self) -> Result<()> {
        if self.link_binding_id.is_zero()
            || self.wrapper_obligation_configuration_id.is_zero()
            || self.product_admission_receipt_id.is_zero()
            || self.last_observed_link_transition_sequence == 0
            || self.link_binding_id == self.wrapper_obligation_configuration_id
            || self.link_binding_id == self.product_admission_receipt_id
            || self.wrapper_obligation_configuration_id == self.product_admission_receipt_id
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
}

/// Mutable Structured descriptor-family root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredMarketRootV1 {
    /// Immutable Product/Series/root identity.
    pub binding: StructuredMarketRootBindingV1,
    /// Immutable Product link/Wrapper admission lineage and observed sequence.
    pub product_lineage: StructuredProductLineageV1,
    /// Monotone Structured transition sequence.
    pub transition_sequence: u64,
    /// Total admitted descriptor identities.
    pub admitted_descriptor_count: u32,
    /// Admitted descriptors that remain live.
    pub live_descriptor_count: u32,
    /// Admitted descriptors sealed terminal.
    pub terminal_descriptor_count: u32,
    /// Rolling ordered admission transcript.
    pub admission_transcript_id: ContentId,
    /// Rolling ordered terminal transcript, zero until the first terminal.
    pub terminal_transcript_id: ContentId,
    /// Aggregate terminal receipt, zero until every admitted descriptor is terminal.
    pub aggregate_terminal_receipt_id: ContentId,
    /// Exact refundable rent principal.
    pub rent_principal_lamports: u64,
    /// Hostile prefund/donation floor, never principal.
    pub donation_floor_lamports: u64,
    /// Current donation residue.
    pub current_donation_lamports: u64,
    /// Canonical root PDA bump.
    pub root_bump: u8,
}

impl StructuredMarketRootV1 {
    /// Advance the observed Product sequence without consuming a Structured
    /// transition sequence. A live adapter must hostile-authenticate the current
    /// link account, immutable binding/configuration, and exact Wrapper admission
    /// receipt before constructing `current_product_lineage`.
    pub fn observe_current_product_lineage(
        self,
        current_product_lineage: StructuredProductLineageV1,
    ) -> Result<Self> {
        self.validate()?;
        current_product_lineage.validate()?;
        validate_product_lineage_successor(self.product_lineage, current_product_lineage)?;
        let next = Self {
            product_lineage: current_product_lineage,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Reconcile unsolicited lamports immediately before a descriptor/root
    /// mutation. The persisted principal is immutable; every excess lamport is
    /// donation residue and can only increase while the account remains live.
    /// This observation does not consume a semantic transition sequence.
    pub fn observe_lamport_balance(self, observed_lamports: u64) -> Result<Self> {
        self.validate()?;
        let observed_donation_lamports = observed_lamports
            .checked_sub(self.rent_principal_lamports)
            .ok_or(Error::ArithmeticUnderflow)?;
        if observed_donation_lamports < self.current_donation_lamports
            || observed_donation_lamports < self.donation_floor_lamports
        {
            return Err(Error::InvariantViolation);
        }
        let next = Self {
            current_donation_lamports: observed_donation_lamports,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Initialize the root with its first authenticated descriptor.
    #[allow(clippy::too_many_arguments)]
    pub fn initialize<H: WrapperRecipeHashV1>(
        binding: StructuredMarketRootBindingV1,
        product_lineage: StructuredProductLineageV1,
        descriptor_id: ContentId,
        recipe_id: ContentId,
        rent_principal_lamports: u64,
        donation_floor_lamports: u64,
        root_bump: u8,
        hasher: &H,
    ) -> Result<Self> {
        binding.validate()?;
        product_lineage.validate()?;
        validate_distinct_descriptor_recipe(descriptor_id, recipe_id)?;
        if rent_principal_lamports == 0 {
            return Err(Error::InvalidAccount);
        }
        let current_donation_lamports = donation_floor_lamports;
        rent_principal_lamports
            .checked_add(current_donation_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        let admission_transcript_id = structured_descriptor_admission_receipt_v1(
            ContentId::ZERO,
            descriptor_id,
            recipe_id,
            1,
            hasher,
        )?;
        if product_lineage.product_admission_receipt_id == admission_transcript_id {
            return Err(Error::InvalidIdentity);
        }
        let value = Self {
            binding,
            product_lineage,
            transition_sequence: 1,
            admitted_descriptor_count: 1,
            live_descriptor_count: 1,
            terminal_descriptor_count: 0,
            admission_transcript_id,
            terminal_transcript_id: ContentId::ZERO,
            aggregate_terminal_receipt_id: ContentId::ZERO,
            rent_principal_lamports,
            donation_floor_lamports,
            current_donation_lamports,
            root_bump,
        };
        value.validate()?;
        Ok(value)
    }

    /// Admit another descriptor after reauthenticating the same live Product link.
    pub fn admit_descriptor<H: WrapperRecipeHashV1>(
        self,
        current_product_lineage: StructuredProductLineageV1,
        descriptor_id: ContentId,
        recipe_id: ContentId,
        hasher: &H,
    ) -> Result<Self> {
        self.validate()?;
        current_product_lineage.validate()?;
        validate_distinct_descriptor_recipe(descriptor_id, recipe_id)?;
        validate_product_lineage_successor(self.product_lineage, current_product_lineage)?;
        let transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let next = Self {
            product_lineage: current_product_lineage,
            transition_sequence,
            admitted_descriptor_count: self
                .admitted_descriptor_count
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            live_descriptor_count: self
                .live_descriptor_count
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            admission_transcript_id: structured_descriptor_admission_receipt_v1(
                self.admission_transcript_id,
                descriptor_id,
                recipe_id,
                transition_sequence,
                hasher,
            )?,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Seal one exact descriptor terminal and optionally close the whole family.
    pub fn seal_descriptor_terminal<H: WrapperRecipeHashV1 + ?Sized>(
        self,
        current_product_lineage: StructuredProductLineageV1,
        descriptor_id: ContentId,
        descriptor_terminal_receipt_id: ContentId,
        hasher: &H,
    ) -> Result<Self> {
        self.validate()?;
        current_product_lineage.validate()?;
        validate_product_lineage_successor(self.product_lineage, current_product_lineage)?;
        if descriptor_id.is_zero()
            || descriptor_terminal_receipt_id.is_zero()
            || descriptor_id == descriptor_terminal_receipt_id
            || self.live_descriptor_count == 0
        {
            return Err(Error::InvalidIdentity);
        }
        let transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let live_descriptor_count = self
            .live_descriptor_count
            .checked_sub(1)
            .ok_or(Error::ArithmeticUnderflow)?;
        let terminal_descriptor_count = self
            .terminal_descriptor_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let is_family_terminal = live_descriptor_count == 0;
        let terminal_transcript_id = terminal_receipt(
            self.terminal_transcript_id,
            descriptor_id,
            descriptor_terminal_receipt_id,
            transition_sequence,
            hasher,
        )?;
        let mut next = Self {
            product_lineage: current_product_lineage,
            transition_sequence,
            live_descriptor_count,
            terminal_descriptor_count,
            terminal_transcript_id,
            aggregate_terminal_receipt_id: ContentId::ZERO,
            ..self
        };
        if is_family_terminal {
            next.aggregate_terminal_receipt_id = derive_market_terminal_receipt_v1(next, hasher)?;
        }
        next.validate()?;
        Ok(next)
    }

    /// Encode the exact authoritative account body.
    pub fn encode(&self) -> Result<[u8; STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES];
        let mut cursor = 0_usize;
        put(
            &mut output,
            &mut cursor,
            &[
                STRUCTURED_MARKET_ROOT_ACCOUNT_TAG,
                STRUCTURED_MARKET_ROOT_ACCOUNT_VERSION,
                0,
                0,
            ],
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.binding.encode_preimage()?,
        )?;
        for id in [
            self.product_lineage.link_binding_id,
            self.product_lineage.wrapper_obligation_configuration_id,
            self.product_lineage.product_admission_receipt_id,
        ] {
            put(&mut output, &mut cursor, &id.bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self
                .product_lineage
                .last_observed_link_transition_sequence
                .to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.transition_sequence.to_le_bytes(),
        )?;
        for count in [
            self.admitted_descriptor_count,
            self.live_descriptor_count,
            self.terminal_descriptor_count,
        ] {
            put(&mut output, &mut cursor, &count.to_le_bytes())?;
        }
        for id in [
            self.admission_transcript_id,
            self.terminal_transcript_id,
            self.aggregate_terminal_receipt_id,
        ] {
            put(&mut output, &mut cursor, &id.bytes())?;
        }
        for amount in [
            self.rent_principal_lamports,
            self.donation_floor_lamports,
            self.current_donation_lamports,
        ] {
            put(&mut output, &mut cursor, &amount.to_le_bytes())?;
        }
        put(&mut output, &mut cursor, &[self.root_bump])?;
        put(&mut output, &mut cursor, &[0; 7])?;
        if cursor != output.len() {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    /// Decode one exact root body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES
            || input[0] != STRUCTURED_MARKET_ROOT_ACCOUNT_TAG
            || input[1] != STRUCTURED_MARKET_ROOT_ACCOUNT_VERSION
            || input[2..4] != [0; 2]
        {
            return Err(Error::InvalidHeader);
        }
        let mut cursor = 4_usize;
        let binding = decode_binding(take_exact::<STRUCTURED_MARKET_ROOT_BINDING_BYTES_V1>(
            input,
            &mut cursor,
        )?)?;
        let product_lineage = StructuredProductLineageV1 {
            link_binding_id: ContentId::from_bytes(read_id(input, &mut cursor)?),
            wrapper_obligation_configuration_id: ContentId::from_bytes(read_id(input, &mut cursor)?),
            product_admission_receipt_id: ContentId::from_bytes(read_id(input, &mut cursor)?),
            last_observed_link_transition_sequence: read_u64(input, &mut cursor)?,
        };
        let transition_sequence = read_u64(input, &mut cursor)?;
        let admitted_descriptor_count = read_u32(input, &mut cursor)?;
        let live_descriptor_count = read_u32(input, &mut cursor)?;
        let terminal_descriptor_count = read_u32(input, &mut cursor)?;
        let admission_transcript_id = ContentId::from_bytes(read_id(input, &mut cursor)?);
        let terminal_transcript_id = ContentId::from_bytes(read_id(input, &mut cursor)?);
        let aggregate_terminal_receipt_id =
            ContentId::from_bytes(read_id(input, &mut cursor)?);
        let rent_principal_lamports = read_u64(input, &mut cursor)?;
        let donation_floor_lamports = read_u64(input, &mut cursor)?;
        let current_donation_lamports = read_u64(input, &mut cursor)?;
        let root_bump = read_byte(input, &mut cursor)?;
        if take_slice(input, &mut cursor, 7)? != [0; 7] || cursor != input.len() {
            return Err(Error::NonCanonicalPadding);
        }
        let value = Self {
            binding,
            product_lineage,
            transition_sequence,
            admitted_descriptor_count,
            live_descriptor_count,
            terminal_descriptor_count,
            admission_transcript_id,
            terminal_transcript_id,
            aggregate_terminal_receipt_id,
            rent_principal_lamports,
            donation_floor_lamports,
            current_donation_lamports,
            root_bump,
        };
        value.validate()?;
        Ok(value)
    }

    /// Typed exhaustive projection consumed only after adapter authentication.
    pub fn projection<H: WrapperRecipeHashV1>(
        self,
        hasher: &H,
    ) -> Result<StructuredMarketProjectionV1> {
        self.validate()?;
        let state = if self.live_descriptor_count == 0 {
            StructuredMarketProjectionStateV1::Terminal
        } else {
            StructuredMarketProjectionStateV1::Live
        };
        let projection = StructuredMarketProjectionV1 {
            market_instance_id: self.binding.market_instance_id,
            series_plan_id: self.binding.series_plan_id,
            series_market_link_account: self.binding.link_account,
            ordinal: self.binding.ordinal,
            generation: self.binding.generation,
            attachment_plan_id: self.binding.attachment_plan_id,
            wrapper_recipe_set_id: self.binding.wrapper_recipe_set_id,
            compiler_output_id: self.binding.compiler_output_id,
            compiler_release_id: self.binding.compiler_release_id,
            registry_release_id: self.binding.registry_release_id,
            capability_profile_id: self.binding.capability_profile_id,
            owner_release_id: self.binding.owner_release_id,
            structured_root_id: self.binding.id(hasher)?,
            product_admission_receipt_id: self
                .product_lineage
                .product_admission_receipt_id,
            state,
            admitted_descriptor_count: self.admitted_descriptor_count,
            live_descriptor_count: self.live_descriptor_count,
            terminal_descriptor_count: self.terminal_descriptor_count,
            terminal_receipt_id: self.aggregate_terminal_receipt_id.bytes(),
        };
        if state == StructuredMarketProjectionStateV1::Terminal {
            let expected = derive_market_terminal_receipt_v1(
                Self {
                    aggregate_terminal_receipt_id: ContentId::ZERO,
                    ..self
                },
                hasher,
            )?;
            if expected != self.aggregate_terminal_receipt_id {
                return Err(Error::InvalidIdentity);
            }
        }
        projection.validate_counts()?;
        Ok(projection)
    }

    fn validate(self) -> Result<()> {
        self.binding.validate()?;
        self.product_lineage.validate()?;
        let canonical_transition_sequence = u64::from(self.admitted_descriptor_count)
            .checked_add(u64::from(self.terminal_descriptor_count))
            .ok_or(Error::ArithmeticOverflow)?;
        if self.transition_sequence == 0
            || self.transition_sequence != canonical_transition_sequence
            || self.admitted_descriptor_count == 0
            || self.live_descriptor_count
                .checked_add(self.terminal_descriptor_count)
                != Some(self.admitted_descriptor_count)
            || self.admission_transcript_id.is_zero()
            || self.rent_principal_lamports == 0
            || self.current_donation_lamports < self.donation_floor_lamports
        {
            return Err(Error::InvariantViolation);
        }
        self.rent_principal_lamports
            .checked_add(self.current_donation_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if (self.terminal_descriptor_count == 0) != self.terminal_transcript_id.is_zero()
            || (self.live_descriptor_count == 0) != !self.aggregate_terminal_receipt_id.is_zero()
            || (!self.aggregate_terminal_receipt_id.is_zero()
                && (self.aggregate_terminal_receipt_id == self.admission_transcript_id
                    || self.aggregate_terminal_receipt_id == self.terminal_transcript_id
                    || self.aggregate_terminal_receipt_id
                        == self.product_lineage.link_binding_id
                    || self.aggregate_terminal_receipt_id
                        == self.product_lineage.wrapper_obligation_configuration_id
                    || self.aggregate_terminal_receipt_id
                        == self.product_lineage.product_admission_receipt_id))
        {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }
}

fn derive_market_terminal_receipt_v1<H: WrapperRecipeHashV1 + ?Sized>(
    terminal_without_receipt: StructuredMarketRootV1,
    hasher: &H,
) -> Result<ContentId> {
    if terminal_without_receipt.admitted_descriptor_count == 0
        || terminal_without_receipt.live_descriptor_count != 0
        || terminal_without_receipt.terminal_descriptor_count
            != terminal_without_receipt.admitted_descriptor_count
        || terminal_without_receipt.terminal_transcript_id.is_zero()
        || !terminal_without_receipt.aggregate_terminal_receipt_id.is_zero()
    {
        return Err(Error::InvariantViolation);
    }
    terminal_without_receipt.binding.validate()?;
    terminal_without_receipt.product_lineage.validate()?;
    let canonical_transition_sequence = u64::from(
        terminal_without_receipt.admitted_descriptor_count,
    )
    .checked_add(u64::from(
        terminal_without_receipt.terminal_descriptor_count,
    ))
    .ok_or(Error::ArithmeticOverflow)?;
    if terminal_without_receipt.transition_sequence != canonical_transition_sequence
        || terminal_without_receipt.admission_transcript_id.is_zero()
        || terminal_without_receipt.rent_principal_lamports == 0
        || terminal_without_receipt.current_donation_lamports
            < terminal_without_receipt.donation_floor_lamports
    {
        return Err(Error::InvariantViolation);
    }
    terminal_without_receipt
        .rent_principal_lamports
        .checked_add(terminal_without_receipt.current_donation_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut preimage = [0_u8; STRUCTURED_MARKET_TERMINAL_PREIMAGE_BYTES_V1];
    let mut cursor = 0_usize;
    put(
        &mut preimage,
        &mut cursor,
        &[
            STRUCTURED_MARKET_ROOT_ACCOUNT_TAG,
            STRUCTURED_MARKET_ROOT_ACCOUNT_VERSION,
            0,
            0,
        ],
    )?;
    put(
        &mut preimage,
        &mut cursor,
        &terminal_without_receipt.binding.encode_preimage()?,
    )?;
    for id in [
        terminal_without_receipt
            .product_lineage
            .link_binding_id,
        terminal_without_receipt.product_lineage.wrapper_obligation_configuration_id,
        terminal_without_receipt
            .product_lineage
            .product_admission_receipt_id,
    ] {
        put(&mut preimage, &mut cursor, &id.bytes())?;
    }
    for sequence in [
        terminal_without_receipt
            .product_lineage
            .last_observed_link_transition_sequence,
        terminal_without_receipt.transition_sequence,
    ] {
        put(&mut preimage, &mut cursor, &sequence.to_le_bytes())?;
    }
    for count in [
        terminal_without_receipt.admitted_descriptor_count,
        terminal_without_receipt.live_descriptor_count,
        terminal_without_receipt.terminal_descriptor_count,
    ] {
        put(&mut preimage, &mut cursor, &count.to_le_bytes())?;
    }
    for id in [
        terminal_without_receipt.admission_transcript_id,
        terminal_without_receipt.terminal_transcript_id,
    ] {
        put(&mut preimage, &mut cursor, &id.bytes())?;
    }
    for amount in [
        terminal_without_receipt.rent_principal_lamports,
        terminal_without_receipt.donation_floor_lamports,
        terminal_without_receipt.current_donation_lamports,
    ] {
        put(&mut preimage, &mut cursor, &amount.to_le_bytes())?;
    }
    put(
        &mut preimage,
        &mut cursor,
        &[terminal_without_receipt.root_bump],
    )?;
    put(&mut preimage, &mut cursor, &[0; 7])?;
    if cursor != preimage.len() {
        return Err(Error::InvalidLength);
    }
    let receipt = ContentId::from_bytes(
        hasher.hashv(&[STRUCTURED_MARKET_TERMINAL_DOMAIN_V1, &preimage]),
    );
    if receipt.is_zero() {
        return Err(Error::InvalidIdentity);
    }
    Ok(receipt)
}

/// Derive one exact root admission receipt before an atomic Product mutation.
pub fn structured_descriptor_admission_receipt_v1<H: WrapperRecipeHashV1>(
    previous: ContentId,
    descriptor_id: ContentId,
    recipe_id: ContentId,
    sequence: u64,
    hasher: &H,
) -> Result<ContentId> {
    validate_distinct_descriptor_recipe(descriptor_id, recipe_id)?;
    if sequence == 0 {
        return Err(Error::InvalidState);
    }
    let id = ContentId::from_bytes(hasher.hashv(&[
        STRUCTURED_DESCRIPTOR_ADMISSION_DOMAIN_V1,
        &previous.bytes(),
        &descriptor_id.bytes(),
        &recipe_id.bytes(),
        &sequence.to_le_bytes(),
    ]));
    if id.is_zero() {
        return Err(Error::InvalidIdentity);
    }
    Ok(id)
}

fn terminal_receipt<H: WrapperRecipeHashV1 + ?Sized>(
    previous: ContentId,
    descriptor_id: ContentId,
    receipt_id: ContentId,
    sequence: u64,
    hasher: &H,
) -> Result<ContentId> {
    if descriptor_id.is_zero()
        || receipt_id.is_zero()
        || descriptor_id == receipt_id
        || sequence == 0
    {
        return Err(Error::InvalidIdentity);
    }
    let id = ContentId::from_bytes(hasher.hashv(&[
        STRUCTURED_DESCRIPTOR_TERMINAL_DOMAIN_V1,
        &previous.bytes(),
        &descriptor_id.bytes(),
        &receipt_id.bytes(),
        &sequence.to_le_bytes(),
    ]));
    if id.is_zero() {
        return Err(Error::InvalidIdentity);
    }
    Ok(id)
}

fn validate_distinct_descriptor_recipe(
    descriptor_id: ContentId,
    recipe_id: ContentId,
) -> Result<()> {
    if descriptor_id.is_zero() || recipe_id.is_zero() || descriptor_id == recipe_id {
        return Err(Error::InvalidIdentity);
    }
    Ok(())
}

fn validate_product_lineage_successor(
    previous: StructuredProductLineageV1,
    current: StructuredProductLineageV1,
) -> Result<()> {
    if current.link_binding_id != previous.link_binding_id
        || current.wrapper_obligation_configuration_id
            != previous.wrapper_obligation_configuration_id
        || current.product_admission_receipt_id != previous.product_admission_receipt_id
        || current.last_observed_link_transition_sequence
            < previous.last_observed_link_transition_sequence
    {
        return Err(Error::InvalidIdentity);
    }
    Ok(())
}

fn decode_binding(input: [u8; STRUCTURED_MARKET_ROOT_BINDING_BYTES_V1]) -> Result<StructuredMarketRootBindingV1> {
    let mut cursor = 0_usize;
    let binding = StructuredMarketRootBindingV1 {
        link_account: read_id(&input, &mut cursor)?,
        series_plan_id: SeriesPlanV5Id::from_bytes(read_id(&input, &mut cursor)?),
        market_instance_id: MarketInstanceV2Id::from_bytes(read_id(&input, &mut cursor)?),
        attachment_plan_id: SeriesAttachmentPlanV6Id::from_bytes(read_id(&input, &mut cursor)?),
        compiler_output_id: CompiledProductSeriesBundleV7Id::from_bytes(read_id(&input, &mut cursor)?),
        compiler_release_id: ContentId::from_bytes(read_id(&input, &mut cursor)?),
        registry_release_id: ContentId::from_bytes(read_id(&input, &mut cursor)?),
        capability_profile_id: ContentId::from_bytes(read_id(&input, &mut cursor)?),
        wrapper_recipe_set_id: ContentId::from_bytes(read_id(&input, &mut cursor)?),
        owner_release_id: ContentId::from_bytes(read_id(&input, &mut cursor)?),
        rent_refund_owner: ContentId::from_bytes(read_id(&input, &mut cursor)?),
        neutral_lamport_sink: ContentId::from_bytes(read_id(&input, &mut cursor)?),
        ordinal: read_u32(&input, &mut cursor)?,
        generation: read_u64(&input, &mut cursor)?,
    };
    if take_slice(&input, &mut cursor, 4)? != [0; 4] || cursor != input.len() {
        return Err(Error::NonCanonicalPadding);
    }
    binding.validate()?;
    Ok(binding)
}

fn take_exact<const N: usize>(input: &[u8], cursor: &mut usize) -> Result<[u8; N]> {
    let mut value = [0_u8; N];
    value.copy_from_slice(take_slice(input, cursor, N)?);
    Ok(value)
}

fn take_slice<'a>(input: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = cursor.checked_add(len).ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidLength)?;
    *cursor = end;
    Ok(value)
}

fn read_id(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    take_exact::<32>(input, cursor)
}

fn read_byte(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(take_slice(input, cursor, 1)?[0])
}

fn read_u32(input: &[u8], cursor: &mut usize) -> Result<u32> {
    Ok(u32::from_le_bytes(take_exact::<4>(input, cursor)?))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    Ok(u64::from_le_bytes(take_exact::<8>(input, cursor)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DeterministicHash;

    impl WrapperRecipeHashV1 for DeterministicHash {
        fn hashv(&self, slices: &[&[u8]]) -> [u8; 32] {
            let mut output = [0_u8; 32];
            let mut at = 0_usize;
            for slice in slices {
                for byte in *slice {
                    output[at & 31] = output[at & 31].wrapping_add(*byte).wrapping_add(1);
                    at += 1;
                }
            }
            output[0] |= 1;
            output
        }
    }

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn binding() -> StructuredMarketRootBindingV1 {
        StructuredMarketRootBindingV1 {
            link_account: [1; 32],
            series_plan_id: SeriesPlanV5Id::from_bytes([2; 32]),
            ordinal: 3,
            market_instance_id: MarketInstanceV2Id::from_bytes([4; 32]),
            generation: 5,
            attachment_plan_id: SeriesAttachmentPlanV6Id::from_bytes([6; 32]),
            compiler_output_id: CompiledProductSeriesBundleV7Id::from_bytes([7; 32]),
            compiler_release_id: id(8),
            registry_release_id: id(9),
            capability_profile_id: id(10),
            wrapper_recipe_set_id: id(11),
            owner_release_id: id(12),
            rent_refund_owner: id(13),
            neutral_lamport_sink: id(14),
        }
    }

    fn deployment() -> DeploymentBinding {
        DeploymentBinding {
            wrapper_program: [21; 32],
            wrapper_program_data: [22; 32],
            wrapper_deployment_slot: 23,
            base_program: [24; 32],
            base_program_data: [25; 32],
            base_deployment_slot: 26,
            token_2022_program: [27; 32],
            token_2022_program_data: [28; 32],
            token_2022_deployment_slot: 29,
        }
    }

    #[test]
    fn current_owner_release_commits_each_disjoint_release_artifact() {
        let hash = DeterministicHash;
        let expected = structured_owner_release_id_v2(
            deployment(),
            id(30),
            id(31),
            id(32),
            &hash,
        )
        .unwrap();
        assert_ne!(
            structured_owner_release_id_v2(deployment(), id(33), id(31), id(32), &hash)
                .unwrap(),
            expected,
        );
        assert_eq!(
            structured_owner_release_id_v2(deployment(), id(30), id(30), id(32), &hash),
            Err(Error::InvalidIdentity),
        );
    }

    fn lineage(_hash: &DeterministicHash) -> StructuredProductLineageV1 {
        StructuredProductLineageV1 {
            link_binding_id: id(14),
            wrapper_obligation_configuration_id: id(15),
            product_admission_receipt_id: id(16),
            last_observed_link_transition_sequence: 3,
        }
    }

    #[test]
    fn product_projection_and_descriptor_transcript_are_distinct_authorities() {
        let hash = DeterministicHash;
        let root = StructuredMarketRootV1::initialize(
            binding(),
            lineage(&hash),
            id(17),
            id(18),
            1_000,
            40,
            7,
            &hash,
        )
        .unwrap();
        assert_eq!(root.product_lineage.product_admission_receipt_id, id(16));
        assert_eq!(
            root.admission_transcript_id,
            structured_descriptor_admission_receipt_v1(
                ContentId::ZERO,
                id(17),
                id(18),
                1,
                &hash,
            )
            .unwrap(),
        );
        assert_ne!(
            root.product_lineage.product_admission_receipt_id,
            root.admission_transcript_id,
        );
        let mut aliased = lineage(&hash);
        aliased.product_admission_receipt_id = root.admission_transcript_id;
        assert_eq!(
            StructuredMarketRootV1::initialize(
                binding(),
                aliased,
                id(17),
                id(18),
                1_000,
                40,
                7,
                &hash,
            ),
            Err(Error::InvalidIdentity),
        );
    }

    #[test]
    fn root_round_trip_partitions_counts_and_rent() {
        let hash = DeterministicHash;
        let root = StructuredMarketRootV1::initialize(
            binding(),
            lineage(&hash),
            id(17),
            id(18),
            1_000,
            40,
            7,
            &hash,
        )
        .unwrap();
        let bytes = root.encode().unwrap();
        assert_eq!(StructuredMarketRootV1::decode(&bytes), Ok(root));
        assert_eq!(root.projection(&hash).unwrap().live_descriptor_count, 1);
        let mut impossible_sequence = root;
        impossible_sequence.transition_sequence = 99;
        assert_eq!(
            impossible_sequence.encode(),
            Err(Error::InvariantViolation)
        );
        let mut hostile = bytes;
        hostile[2] = 1;
        assert_eq!(
            StructuredMarketRootV1::decode(&hostile),
            Err(Error::InvalidHeader)
        );
    }

    #[test]
    fn sibling_link_churn_is_observable_but_binding_and_wrapper_splices_are_refused() {
        let hash = DeterministicHash;
        let root = StructuredMarketRootV1::initialize(
            binding(),
            lineage(&hash),
            id(17),
            id(18),
            1_000,
            40,
            7,
            &hash,
        )
        .unwrap();
        let mut sibling_churn = root.product_lineage;
        sibling_churn.last_observed_link_transition_sequence += 9;
        let observed = root
            .observe_current_product_lineage(sibling_churn)
            .expect("unrelated monotone Product transition");
        assert_eq!(
            observed
                .product_lineage
                .last_observed_link_transition_sequence,
            sibling_churn.last_observed_link_transition_sequence
        );

        let mut binding_splice = sibling_churn;
        binding_splice.link_binding_id = id(55);
        assert!(root
            .observe_current_product_lineage(binding_splice)
            .is_err());
        let mut wrapper_splice = sibling_churn;
        wrapper_splice.wrapper_obligation_configuration_id = id(56);
        assert!(root
            .observe_current_product_lineage(wrapper_splice)
            .is_err());
        let mut admission_splice = sibling_churn;
        admission_splice.product_admission_receipt_id = id(57);
        assert!(root
            .observe_current_product_lineage(admission_splice)
            .is_err());
    }

    #[test]
    fn colliding_dealer_coordinate_never_decodes_as_structured_root() {
        let hash = DeterministicHash;
        let root = StructuredMarketRootV1::initialize(
            binding(),
            lineage(&hash),
            id(17),
            id(18),
            1_000,
            0,
            7,
            &hash,
        )
        .unwrap();
        let mut bytes = root.encode().unwrap();
        assert_eq!(bytes[0], 0xb7);
        bytes[0] = 0xaf;
        assert_eq!(
            StructuredMarketRootV1::decode(&bytes),
            Err(Error::InvalidHeader)
        );
    }

    #[test]
    fn terminal_receipt_commits_the_exact_final_root_lineage() {
        let hash = DeterministicHash;
        let root = StructuredMarketRootV1::initialize(
            binding(),
            lineage(&hash),
            id(17),
            id(18),
            1_000,
            0,
            7,
            &hash,
        )
        .unwrap();
        let terminal = root
            .seal_descriptor_terminal(lineage(&hash), id(17), id(19), &hash)
            .unwrap();
        assert_eq!(terminal.live_descriptor_count, 0);
        assert_eq!(
            terminal.projection(&hash).unwrap().state,
            StructuredMarketProjectionStateV1::Terminal
        );
        let mut forged = terminal;
        forged.aggregate_terminal_receipt_id = id(20);
        assert_eq!(forged.projection(&hash), Err(Error::InvalidIdentity));
        forged = terminal;
        forged.product_lineage.wrapper_obligation_configuration_id = id(21);
        assert_eq!(forged.projection(&hash), Err(Error::InvalidIdentity));
    }

    #[test]
    fn root_refuses_aliased_product_roles_and_unfunded_donation_state() {
        let hash = DeterministicHash;
        let mut aliased = binding();
        aliased.neutral_lamport_sink = aliased.rent_refund_owner;
        assert_eq!(aliased.id(&hash), Err(Error::InvalidIdentity));

        let root = StructuredMarketRootV1::initialize(
            binding(),
            lineage(&hash),
            id(17),
            id(18),
            1_000,
            40,
            7,
            &hash,
        )
        .unwrap();
        let mut unfunded = root;
        unfunded.current_donation_lamports = 39;
        assert_eq!(unfunded.encode(), Err(Error::InvariantViolation));
    }

    #[test]
    fn later_admission_is_ordered_and_requires_live_product_lineage() {
        let hash = DeterministicHash;
        let root = StructuredMarketRootV1::initialize(
            binding(),
            lineage(&hash),
            id(17),
            id(18),
            1_000,
            0,
            7,
            &hash,
        )
        .unwrap();
        let successor = root
            .admit_descriptor(lineage(&hash), id(19), id(20), &hash)
            .unwrap();
        assert_eq!(successor.transition_sequence, 2);
        assert_eq!(successor.admitted_descriptor_count, 2);
        assert_eq!(successor.live_descriptor_count, 2);
        assert_ne!(successor.admission_transcript_id, root.admission_transcript_id);

        let mut hostile_lineage = lineage(&hash);
        hostile_lineage.product_admission_receipt_id = ContentId::ZERO;
        assert_eq!(
            root.admit_descriptor(hostile_lineage, id(19), id(20), &hash),
            Err(Error::InvalidIdentity)
        );
    }
}
