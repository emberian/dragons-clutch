//! Atomic descriptor, Structured-root, and Product-obligation terminal plan.
//!
//! Product's link account and the base Position/Replay accounts remain owned by
//! their respective adapters. This module consumes only their exact projections
//! and produces one indivisible plan. A live SBF adapter must mint the Product
//! projection from private account authentication and must execute every write
//! in one instruction; caller-authored values are not runtime authority.

use clutch_product_series::ContentId;

use crate::{
    DescriptorRetirementPlanV1, DescriptorStateV1, Error, Result,
    StructuredClaimDescriptorV2, StructuredMarketRootV1, WrapperRecipeHashV1,
};

/// Domain for the exact active descriptor body committed by terminalization.
pub const STRUCTURED_DESCRIPTOR_ACTIVE_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/descriptor-active-body/v1\0";
/// Domain for the exact permanent descriptor tombstone body.
pub const STRUCTURED_DESCRIPTOR_RETIRED_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/descriptor-retired-body/v1\0";
/// Domain for one descriptor's complete mint/vault terminal receipt.
pub const STRUCTURED_DESCRIPTOR_CLOSE_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/descriptor-close/v1\0";

/// Product-owned Wrapper-obligation transition projected through its private
/// authenticated link-account writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredProductWrapperTerminalProjectionV1 {
    /// Exact Product-owned Series link account.
    pub link_account: [u8; 32],
    /// Full-width Market shared with the Structured root.
    pub market_instance_id: [u8; 32],
    /// Product/Source generation shared with the Structured root.
    pub generation: u64,
    /// Full account authentication identity before Product's write.
    pub previous_link_authentication_id: ContentId,
    /// Exact Product link semantic identity before Product's write.
    pub previous_link_semantic_id: ContentId,
    /// Product link sequence before consuming Wrapper.
    pub previous_link_transition_sequence: u64,
    /// Immutable receipt that originally admitted the Wrapper obligation.
    pub product_admission_receipt_id: ContentId,
    /// Structured aggregate receipt passed as Product's owner receipt.
    pub owner_terminal_receipt_id: ContentId,
    /// Product's typed Wrapper-obligation terminal transition receipt.
    pub obligation_terminal_receipt_id: ContentId,
    /// Full account authentication identity after Product's write.
    pub successor_link_authentication_id: ContentId,
    /// Exact Product link semantic identity after Product's write.
    pub successor_link_semantic_id: ContentId,
    /// Product link sequence after consuming Wrapper, exactly previous plus one.
    pub successor_link_transition_sequence: u64,
}

/// Exact physical deletion disposition for a terminal Structured root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRootCloseDispositionV1 {
    /// Canonical Structured root PDA deleted by this plan.
    pub root_account: [u8; 32],
    /// Persisted payer receiving only root rent principal.
    pub rent_refund_owner: [u8; 32],
    /// Persisted neutral sink receiving every excess lamport.
    pub neutral_lamport_sink: [u8; 32],
    /// Actual root balance before deletion.
    pub balance_before_lamports: u64,
    /// Exact principal refund.
    pub refund_lamports: u64,
    /// Exact donation/surplus disposition.
    pub donation_lamports: u64,
    /// Deleted accounts necessarily finish at zero lamports.
    pub balance_after_lamports: u64,
}

/// One complete terminalization plan. Non-final descriptor retirement writes
/// `root_after` and leaves the optional Product/root-close projections absent.
/// Final retirement consumes Product's Wrapper obligation and deletes the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredDescriptorTerminalPlanV1 {
    /// Exact descriptor/mint/base-vault retirement plan.
    pub descriptor_retirement: DescriptorRetirementPlanV1,
    /// Receipt inserted into the ordered Structured terminal transcript.
    pub descriptor_terminal_receipt_id: ContentId,
    /// Exact successor root, used as Product evidence before optional deletion.
    pub root_after: StructuredMarketRootV1,
    /// Private Product terminal projection, present exactly for the last descriptor.
    pub product_terminal: Option<StructuredProductWrapperTerminalProjectionV1>,
    /// Exact root deletion disposition, present exactly with Product terminality.
    pub root_close: Option<StructuredRootCloseDispositionV1>,
}

/// Descriptor/vault/root postimage prepared before the Product-owned Wrapper
/// latch is consumed. A live adapter must persist and hostile-reauthenticate
/// these exact postimages before this becomes Product authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredDescriptorTerminalOwnerPlanV1 {
    /// Exact descriptor/mint/base-vault retirement plan.
    pub descriptor_retirement: DescriptorRetirementPlanV1,
    /// Receipt inserted into the ordered Structured terminal transcript.
    pub descriptor_terminal_receipt_id: ContentId,
    /// Exact successor root exposed to Product as private postwrite evidence.
    pub root_after: StructuredMarketRootV1,
}

/// Prepare the complete Structured-owned retirement postimage without
/// accepting caller-shaped Product terminal facts.
#[allow(clippy::too_many_arguments)]
pub fn prepare_structured_descriptor_terminal_owner_v1<
    H: WrapperRecipeHashV1 + ?Sized,
>(
    root_before: StructuredMarketRootV1,
    observed_root_lamports: u64,
    root_account: [u8; 32],
    wrapper_product_id: [u8; 32],
    wrapper_mint: [u8; 32],
    descriptor_account: [u8; 32],
    descriptor_before: StructuredClaimDescriptorV2,
    descriptor_retirement: DescriptorRetirementPlanV1,
    hasher: &H,
) -> Result<StructuredDescriptorTerminalOwnerPlanV1> {
    descriptor_before.validate_persisted()?;
    descriptor_retirement.descriptor.validate_persisted()?;
    if descriptor_before.state != DescriptorStateV1::Active
        || descriptor_retirement.descriptor.state != DescriptorStateV1::Retired
        || descriptor_retirement.mint_supply != 0
        || descriptor_retirement.mint_authority_before == [0; 32]
        || descriptor_retirement.mint_authority_after != [0; 32]
    {
        return Err(Error::InvalidState);
    }
    let mut expected_retired = descriptor_before;
    expected_retired.state = DescriptorStateV1::Retired;
    if descriptor_retirement.descriptor != expected_retired
        || descriptor_before.structured_root_id != root_before.binding.id(hasher)?.bytes()
        || descriptor_retirement.rent_refund_owner
            != root_before.binding.rent_refund_owner.bytes()
        || descriptor_retirement.neutral_lamport_sink
            != root_before.binding.neutral_lamport_sink.bytes()
    {
        return Err(Error::InvalidIdentity);
    }
    let identities = [
        root_account,
        wrapper_product_id,
        wrapper_mint,
        descriptor_account,
        descriptor_retirement.mint_authority_before,
        descriptor_retirement.vault_owner,
        descriptor_retirement.vault_position_account,
        descriptor_retirement.vault_replay_account,
        descriptor_retirement.vault_close_receipt,
        descriptor_retirement.vault_tombstone,
        descriptor_retirement.terminal_replay_semantic_id,
        descriptor_retirement.rent_refund_owner,
        descriptor_retirement.neutral_lamport_sink,
    ];
    require_distinct_nonzero(&identities)?;
    let synchronized_root = root_before.observe_lamport_balance(observed_root_lamports)?;
    let active_body = descriptor_before.encode()?;
    let retired_body = descriptor_retirement.descriptor.encode()?;
    let active_id = ContentId::from_bytes(
        hasher.hashv(&[STRUCTURED_DESCRIPTOR_ACTIVE_BODY_DOMAIN_V1, &active_body]),
    );
    let retired_id = ContentId::from_bytes(
        hasher.hashv(&[STRUCTURED_DESCRIPTOR_RETIRED_BODY_DOMAIN_V1, &retired_body]),
    );
    if active_id.is_zero() || retired_id.is_zero() || active_id == retired_id {
        return Err(Error::InvalidIdentity);
    }
    let descriptor_terminal_receipt_id = descriptor_terminal_receipt(
        synchronized_root.binding.id(hasher)?,
        wrapper_product_id,
        wrapper_mint,
        descriptor_account,
        active_id,
        retired_id,
        descriptor_retirement,
        hasher,
    )?;
    let root_after = synchronized_root.seal_descriptor_terminal(
        synchronized_root.product_lineage,
        active_id,
        descriptor_terminal_receipt_id,
        hasher,
    )?;
    Ok(StructuredDescriptorTerminalOwnerPlanV1 {
        descriptor_retirement,
        descriptor_terminal_receipt_id,
        root_after,
    })
}

/// Prepare one atomic descriptor retirement and, for the last live descriptor,
/// the matching Product Wrapper-obligation transition plus root deletion.
#[allow(clippy::too_many_arguments)]
pub fn prepare_structured_descriptor_terminal_v1<H: WrapperRecipeHashV1 + ?Sized>(
    root_before: StructuredMarketRootV1,
    observed_root_lamports: u64,
    root_account: [u8; 32],
    wrapper_product_id: [u8; 32],
    wrapper_mint: [u8; 32],
    descriptor_account: [u8; 32],
    descriptor_before: StructuredClaimDescriptorV2,
    descriptor_retirement: DescriptorRetirementPlanV1,
    product_terminal: Option<StructuredProductWrapperTerminalProjectionV1>,
    hasher: &H,
) -> Result<StructuredDescriptorTerminalPlanV1> {
    let synchronized_root = root_before.observe_lamport_balance(observed_root_lamports)?;
    let owner = prepare_structured_descriptor_terminal_owner_v1(
        root_before,
        observed_root_lamports,
        root_account,
        wrapper_product_id,
        wrapper_mint,
        descriptor_account,
        descriptor_before,
        descriptor_retirement,
        hasher,
    )?;
    let root_after = owner.root_after;
    let family_terminal = root_after.live_descriptor_count == 0;
    if family_terminal != product_terminal.is_some() {
        return Err(Error::AuthorityUnavailable);
    }
    let root_close = match product_terminal {
        None => None,
        Some(product) => {
            validate_product_terminal(synchronized_root, root_after, product)?;
            Some(StructuredRootCloseDispositionV1 {
                root_account,
                rent_refund_owner: root_after.binding.rent_refund_owner.bytes(),
                neutral_lamport_sink: root_after.binding.neutral_lamport_sink.bytes(),
                balance_before_lamports: observed_root_lamports,
                refund_lamports: root_after.rent_principal_lamports,
                donation_lamports: root_after.current_donation_lamports,
                balance_after_lamports: 0,
            })
        }
    };
    if let Some(close) = root_close {
        if close
            .refund_lamports
            .checked_add(close.donation_lamports)
            != Some(close.balance_before_lamports)
            || close.balance_after_lamports != 0
        {
            return Err(Error::InvariantViolation);
        }
    }
    Ok(StructuredDescriptorTerminalPlanV1 {
        descriptor_retirement: owner.descriptor_retirement,
        descriptor_terminal_receipt_id: owner.descriptor_terminal_receipt_id,
        root_after,
        product_terminal,
        root_close,
    })
}

#[allow(clippy::too_many_arguments)]
fn descriptor_terminal_receipt<H: WrapperRecipeHashV1 + ?Sized>(
    structured_root_id: ContentId,
    wrapper_product_id: [u8; 32],
    wrapper_mint: [u8; 32],
    descriptor_account: [u8; 32],
    active_descriptor_id: ContentId,
    retired_descriptor_id: ContentId,
    retirement: DescriptorRetirementPlanV1,
    hasher: &H,
) -> Result<ContentId> {
    let receipt = ContentId::from_bytes(hasher.hashv(&[
        STRUCTURED_DESCRIPTOR_CLOSE_RECEIPT_DOMAIN_V1,
        &structured_root_id.bytes(),
        &wrapper_product_id,
        &wrapper_mint,
        &descriptor_account,
        &active_descriptor_id.bytes(),
        &retired_descriptor_id.bytes(),
        &retirement.mint_authority_before,
        &retirement.vault_owner,
        &retirement.vault_position_account,
        &retirement.vault_replay_account,
        &retirement.vault_close_receipt,
        &retirement.vault_tombstone,
        &retirement.terminal_replay_semantic_id,
        &retirement.vault_tombstone_principal_lamports.to_le_bytes(),
        &retirement.vault_refund_lamports.to_le_bytes(),
        &retirement.vault_donation_lamports.to_le_bytes(),
        &retirement.rent_refund_owner,
        &retirement.neutral_lamport_sink,
    ]));
    if receipt.is_zero()
        || receipt == active_descriptor_id
        || receipt == retired_descriptor_id
        || receipt == structured_root_id
    {
        return Err(Error::InvalidIdentity);
    }
    Ok(receipt)
}

fn validate_product_terminal(
    root_before: StructuredMarketRootV1,
    root_after: StructuredMarketRootV1,
    product: StructuredProductWrapperTerminalProjectionV1,
) -> Result<()> {
    let previous = root_before.product_lineage;
    let identities = [
        product.previous_link_authentication_id,
        product.previous_link_semantic_id,
        product.product_admission_receipt_id,
        product.owner_terminal_receipt_id,
        product.obligation_terminal_receipt_id,
        product.successor_link_authentication_id,
        product.successor_link_semantic_id,
    ];
    let mut left = 0_usize;
    while left < identities.len() {
        if identities[left].is_zero() {
            return Err(Error::InvalidIdentity);
        }
        let mut right = left + 1;
        while right < identities.len() {
            if identities[left] == identities[right] {
                return Err(Error::InvalidIdentity);
            }
            right += 1;
        }
        left += 1;
    }
    if product.link_account != root_before.binding.link_account
        || product.market_instance_id != root_before.binding.market_instance_id.bytes()
        || product.generation != root_before.binding.generation
        || product.previous_link_authentication_id != previous.link_authentication_id
        || product.previous_link_semantic_id != previous.link_semantic_id
        || product.previous_link_transition_sequence
            != previous.product_link_transition_sequence
        || product.product_admission_receipt_id != previous.product_admission_receipt_id
        || product.owner_terminal_receipt_id != root_after.aggregate_terminal_receipt_id
        || product.successor_link_transition_sequence
            != product
                .previous_link_transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(())
}

fn require_distinct_nonzero(identities: &[[u8; 32]]) -> Result<()> {
    let mut left = 0_usize;
    while left < identities.len() {
        if identities[left] == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        let mut right = left + 1;
        while right < identities.len() {
            if identities[left] == identities[right] {
                return Err(Error::InvalidIdentity);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clutch_product_series::{
        CompiledProductSeriesBundleV5Id, MarketInstanceV2Id, SeriesAttachmentPlanV4Id,
        SeriesPlanV5Id,
    };

    use super::*;
    use crate::{
        structured_descriptor_admission_receipt_v1, StructuredMarketRootBindingV1,
        StructuredProductLineageV1, DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION,
    };

    #[derive(Debug)]
    struct DeterministicHash;

    impl WrapperRecipeHashV1 for DeterministicHash {
        fn hashv(&self, slices: &[&[u8]]) -> [u8; 32] {
            let mut output = [0_u8; 32];
            let mut cursor = 0_usize;
            for slice in slices {
                for byte in *slice {
                    let index = cursor & 31;
                    output[index] = output[index]
                        .wrapping_mul(131)
                        .wrapping_add(*byte)
                        .wrapping_add((cursor as u8).rotate_left((index & 7) as u32));
                    cursor += 1;
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
            attachment_plan_id: SeriesAttachmentPlanV4Id::from_bytes([6; 32]),
            compiler_output_id: CompiledProductSeriesBundleV5Id::from_bytes([7; 32]),
            compiler_release_id: id(8),
            registry_release_id: id(9),
            capability_profile_id: id(10),
            wrapper_recipe_set_id: id(11),
            owner_release_id: id(12),
            rent_refund_owner: id(13),
            neutral_lamport_sink: id(14),
        }
    }

    fn descriptor(structured_root_id: [u8; 32]) -> StructuredClaimDescriptorV2 {
        let mut primitive = [0_u64; crate::MAX_OUTCOMES];
        primitive[0] = 1;
        primitive[1] = 2;
        StructuredClaimDescriptorV2 {
            tag: DESCRIPTOR_ACCOUNT_TAG,
            version: DESCRIPTOR_ACCOUNT_VERSION,
            flags: 0,
            base_program: [21; 32],
            base_program_data: [22; 32],
            base_deployment_slot: 1,
            wrapper_program_data: [23; 32],
            wrapper_deployment_slot: 2,
            token_2022_program: [24; 32],
            token_2022_program_data: [25; 32],
            token_2022_deployment_slot: 3,
            market: [26; 32],
            terms_digest: [27; 32],
            structured_root_id,
            wrapper_recipe_id: [28; 32],
            primitive,
            state: DescriptorStateV1::Active,
            descriptor_bump: 1,
            mint_bump: 2,
            mint_authority_bump: 3,
            vault_owner_bump: 4,
        }
    }

    fn retirement(before: StructuredClaimDescriptorV2) -> DescriptorRetirementPlanV1 {
        let mut retired = before;
        retired.state = DescriptorStateV1::Retired;
        DescriptorRetirementPlanV1 {
            descriptor: retired,
            mint_supply: 0,
            mint_authority_before: [31; 32],
            mint_authority_after: [0; 32],
            vault_close_receipt: [32; 32],
            vault_tombstone: [33; 32],
            vault_position_account: [34; 32],
            vault_replay_account: [35; 32],
            vault_owner: [36; 32],
            terminal_replay_semantic_id: [37; 32],
            vault_tombstone_principal_lamports: 11,
            vault_refund_lamports: 17,
            vault_donation_lamports: 19,
            rent_refund_owner: [13; 32],
            neutral_lamport_sink: [14; 32],
        }
    }

    fn root_and_descriptor(
        hash: &DeterministicHash,
    ) -> (StructuredMarketRootV1, StructuredClaimDescriptorV2, ContentId) {
        let binding = binding();
        let descriptor = descriptor(binding.id(hash).unwrap().bytes());
        let active_body = descriptor.encode().unwrap();
        let active_id = ContentId::from_bytes(hash.hashv(&[
            STRUCTURED_DESCRIPTOR_ACTIVE_BODY_DOMAIN_V1,
            &active_body,
        ]));
        let recipe_id = ContentId::from_bytes(descriptor.wrapper_recipe_id);
        let admission = structured_descriptor_admission_receipt_v1(
            ContentId::ZERO,
            active_id,
            recipe_id,
            1,
            hash,
        )
        .unwrap();
        let lineage = StructuredProductLineageV1 {
            link_authentication_id: id(41),
            link_semantic_id: id(42),
            product_admission_receipt_id: admission,
            product_link_transition_sequence: 7,
        };
        let root = StructuredMarketRootV1::initialize(
            binding,
            lineage,
            active_id,
            recipe_id,
            100,
            5,
            9,
            hash,
        )
        .unwrap();
        (root, descriptor, active_id)
    }

    fn product_projection(
        root: StructuredMarketRootV1,
        owner_terminal_receipt_id: ContentId,
    ) -> StructuredProductWrapperTerminalProjectionV1 {
        StructuredProductWrapperTerminalProjectionV1 {
            link_account: root.binding.link_account,
            market_instance_id: root.binding.market_instance_id.bytes(),
            generation: root.binding.generation,
            previous_link_authentication_id: root.product_lineage.link_authentication_id,
            previous_link_semantic_id: root.product_lineage.link_semantic_id,
            previous_link_transition_sequence: root
                .product_lineage
                .product_link_transition_sequence,
            product_admission_receipt_id: root.product_lineage.product_admission_receipt_id,
            owner_terminal_receipt_id,
            obligation_terminal_receipt_id: id(44),
            successor_link_authentication_id: id(45),
            successor_link_semantic_id: id(46),
            successor_link_transition_sequence: root
                .product_lineage
                .product_link_transition_sequence
                + 1,
        }
    }

    #[test]
    fn last_descriptor_requires_exact_product_terminal_and_root_disposition() {
        let hash = DeterministicHash;
        let (root, descriptor, active_id) = root_and_descriptor(&hash);
        let retirement = retirement(descriptor);
        let synchronized = root.observe_lamport_balance(107).unwrap();
        let active_body = descriptor.encode().unwrap();
        let retired_body = retirement.descriptor.encode().unwrap();
        let active_body_id = ContentId::from_bytes(hash.hashv(&[
            STRUCTURED_DESCRIPTOR_ACTIVE_BODY_DOMAIN_V1,
            &active_body,
        ]));
        assert_eq!(active_id, active_body_id);
        let retired_id = ContentId::from_bytes(hash.hashv(&[
            STRUCTURED_DESCRIPTOR_RETIRED_BODY_DOMAIN_V1,
            &retired_body,
        ]));
        let descriptor_receipt = descriptor_terminal_receipt(
            synchronized.binding.id(&hash).unwrap(),
            [51; 32],
            [52; 32],
            [53; 32],
            active_body_id,
            retired_id,
            retirement,
            &hash,
        )
        .unwrap();
        let candidate = synchronized
            .seal_descriptor_terminal(
                synchronized.product_lineage,
                active_body_id,
                descriptor_receipt,
                &hash,
            )
            .unwrap();
        assert_eq!(
            prepare_structured_descriptor_terminal_v1(
                root,
                107,
                [54; 32],
                [51; 32],
                [52; 32],
                [53; 32],
                descriptor,
                retirement,
                None,
                &hash,
            ),
            Err(Error::AuthorityUnavailable)
        );
        let projection = product_projection(root, candidate.aggregate_terminal_receipt_id);
        let plan = prepare_structured_descriptor_terminal_v1(
            root,
            107,
            [54; 32],
            [51; 32],
            [52; 32],
            [53; 32],
            descriptor,
            retirement,
            Some(projection),
            &hash,
        )
        .unwrap();
        assert_eq!(plan.root_after, candidate);
        assert_eq!(plan.root_close.unwrap().refund_lamports, 100);
        assert_eq!(plan.root_close.unwrap().donation_lamports, 7);

        let mut substituted = projection;
        substituted.owner_terminal_receipt_id = id(47);
        assert_eq!(
            prepare_structured_descriptor_terminal_v1(
                root,
                107,
                [54; 32],
                [51; 32],
                [52; 32],
                [53; 32],
                descriptor,
                retirement,
                Some(substituted),
                &hash,
            ),
            Err(Error::AuthorityUnavailable)
        );
    }

    #[test]
    fn retirement_refuses_lamport_decrease_and_payer_sink_substitution() {
        let hash = DeterministicHash;
        let (root, descriptor, _) = root_and_descriptor(&hash);
        let retirement = retirement(descriptor);
        assert_eq!(
            prepare_structured_descriptor_terminal_v1(
                root,
                104,
                [54; 32],
                [51; 32],
                [52; 32],
                [53; 32],
                descriptor,
                retirement,
                None,
                &hash,
            ),
            Err(Error::InvariantViolation)
        );
        let mut substituted = retirement;
        substituted.neutral_lamport_sink = [55; 32];
        assert_eq!(
            prepare_structured_descriptor_terminal_v1(
                root,
                107,
                [54; 32],
                [51; 32],
                [52; 32],
                [53; 32],
                descriptor,
                substituted,
                None,
                &hash,
            ),
            Err(Error::InvalidIdentity)
        );
    }
}
