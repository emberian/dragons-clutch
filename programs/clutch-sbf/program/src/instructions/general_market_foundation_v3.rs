// SPDX-License-Identifier: AGPL-3.0-or-later

//! Private pre-root authority for the current General Market founder.
//!
//! Product authenticates the complete pre-root Foundation graph and Collateral
//! authenticates the liability, claim-mint, and claim-issuance planes.  This
//! module joins those two non-forgeable capabilities before `0xaa` exists and
//! derives the sole General founding capability for the canonical `0x79/v3`
//! MarketBinding and MarketRuntime PDAs.  It has no instruction route.
//!
//! The returned plan retains the complete Product preauthorization.  Product's
//! later MarketBinding and MarketRuntime slot composers must carry this exact
//! private value through both postwrites; re-projecting its fields from a
//! caller DTO or from an already-created root is not authority.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_product_series::{ContentId, MarketInstanceV2Id, SeriesPlanV5Id};
use solana_pubkey::Pubkey;

use super::product_market_foundation_init::
    AuthenticatedProductMarketFounderFoundationPreauthorizationV1;

const GENERAL_PRE_ROOT_FOUNDING_CAPABILITY_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/general/pre-root-founding-capability/v3\0";

/// Collateral-owned authority consumed by the pre-root General join.
///
/// The default authentication method refuses.  A pure ID tuple, even when its
/// fields happen to agree, therefore cannot authorize General founding.  The
/// Collateral adapter must implement this trait on a private capability that
/// still retains the exact deployment, liability postwrite, complete mint
/// transcript, and Profile-selected claim-issuance evidence.
pub(crate) trait AuthenticatedGeneralPreRootCollateralFoundingV3 {
    fn authenticate_general_pre_root_collateral_founding_v3(
        &self,
        _product: &AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
        _expected_market_runtime: Pubkey,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }

    fn market_instance_v2_id(&self) -> MarketInstanceV2Id;
    fn market_liability_founding_id(&self) -> ContentId;
    fn claim_mint_founding_plan_id(&self) -> ContentId;
    fn claim_issuance_binding_id(&self) -> ContentId;
    fn claim_mint_authority(&self) -> Pubkey;
    fn authentication_id(&self) -> ContentId;
}

/// Exact private General plan formed before Product creates `0xaa` or `0xad`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedGeneralMarketPreRootFoundingPlanV3 {
    product: AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
    market_binding_account: Pubkey,
    market_runtime_account: Pubkey,
    market_liability_founding_id: ContentId,
    claim_mint_founding_plan_id: ContentId,
    claim_issuance_binding_id: ContentId,
    collateral_authentication_id: ContentId,
    general_founding_capability_id: ContentId,
}

impl AuthenticatedGeneralMarketPreRootFoundingPlanV3 {
    /// Canonical General founding capability persisted by the current binding.
    pub(crate) const fn id(&self) -> ContentId {
        self.general_founding_capability_id
    }

    pub(crate) const fn product_preauthorization(
        &self,
    ) -> &AuthenticatedProductMarketFounderFoundationPreauthorizationV1 {
        &self.product
    }

    pub(crate) const fn product_preauthorization_id(&self) -> ContentId {
        self.product.id()
    }

    pub(crate) const fn market_instance_v2_id(&self) -> MarketInstanceV2Id {
        self.product.market_instance_id()
    }

    pub(crate) const fn product_generation(&self) -> u64 {
        self.product.generation()
    }

    pub(crate) const fn series_plan_v5_id(&self) -> SeriesPlanV5Id {
        self.product.series_plan_id()
    }

    pub(crate) const fn series_ordinal(&self) -> u32 {
        self.product.ordinal()
    }

    pub(crate) const fn compiler_bundle_v5_id(&self) -> ContentId {
        self.product.compiler_bundle_id()
    }

    pub(crate) const fn capability_profile_v4_id(&self) -> ContentId {
        self.product.capability_profile_id()
    }

    pub(crate) const fn attachment_plan_v4_id(&self) -> ContentId {
        self.product.attachment_plan_id()
    }

    pub(crate) const fn product_market_root_account(&self) -> Pubkey {
        self.product.lifecycle_root_account()
    }

    pub(crate) const fn series_market_link_account(&self) -> Pubkey {
        self.product.founder_link_account()
    }

    pub(crate) const fn market_binding_account(&self) -> Pubkey {
        self.market_binding_account
    }

    pub(crate) const fn market_runtime_account(&self) -> Pubkey {
        self.market_runtime_account
    }

    pub(crate) const fn market_liability_founding_id(&self) -> ContentId {
        self.market_liability_founding_id
    }

    pub(crate) const fn claim_mint_founding_plan_id(&self) -> ContentId {
        self.claim_mint_founding_plan_id
    }

    pub(crate) const fn claim_issuance_binding_id(&self) -> ContentId {
        self.claim_issuance_binding_id
    }

    pub(crate) const fn collateral_authentication_id(&self) -> ContentId {
        self.collateral_authentication_id
    }

    pub(crate) const fn general_founding_capability_id(&self) -> ContentId {
        self.general_founding_capability_id
    }

    pub(crate) fn require_same_product_preauthorization(
        &self,
        product: &AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
    ) -> Outcome<()> {
        require(&self.product == product, ClutchError::MismatchedState)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneralPreRootFoundingFactsV3 {
    program_id: Pubkey,
    product_preauthorization_id: ContentId,
    product_graph_observation_id: ContentId,
    market_instance_v2_id: MarketInstanceV2Id,
    product_generation: u64,
    series_plan_v5_id: SeriesPlanV5Id,
    series_ordinal: u32,
    compiler_bundle_v5_id: ContentId,
    capability_profile_v4_id: ContentId,
    attachment_plan_v4_id: ContentId,
    product_template_id: ContentId,
    native_claim_basis_id: ContentId,
    price_measure_policy_id: ContentId,
    market_genesis_profile_id: ContentId,
    product_market_root_account: Pubkey,
    series_market_link_account: Pubkey,
    market_binding_account: Pubkey,
    market_runtime_account: Pubkey,
    market_liability_founding_id: ContentId,
    claim_mint_founding_plan_id: ContentId,
    claim_issuance_binding_id: ContentId,
    collateral_authentication_id: ContentId,
}

impl GeneralPreRootFoundingFactsV3 {
    fn validate(self) -> Outcome<()> {
        require(self.product_generation != 0, ClutchError::MismatchedState)?;
        let semantic = [
            self.product_preauthorization_id,
            self.product_graph_observation_id,
            self.market_instance_v2_id.content_id(),
            self.series_plan_v5_id.content_id(),
            self.compiler_bundle_v5_id,
            self.capability_profile_v4_id,
            self.attachment_plan_v4_id,
            self.product_template_id,
            self.native_claim_basis_id,
            self.price_measure_policy_id,
            self.market_genesis_profile_id,
            self.market_liability_founding_id,
            self.claim_mint_founding_plan_id,
            self.claim_issuance_binding_id,
            self.collateral_authentication_id,
        ];
        for identity in semantic.iter().copied() {
            require(!identity.is_zero(), ClutchError::MismatchedState)?;
        }
        let physical = [
            self.product_market_root_account,
            self.series_market_link_account,
            self.market_binding_account,
            self.market_runtime_account,
        ];
        let mut left = 0usize;
        while left < physical.len() {
            require(physical[left] != Pubkey::default(), ClutchError::MismatchedState)?;
            let mut right = left + 1;
            while right < physical.len() {
                require(physical[left] != physical[right], ClutchError::AccountAlias)?;
                right += 1;
            }
            left += 1;
        }
        left = 0;
        while left < semantic.len() {
            let mut right = left + 1;
            while right < semantic.len() {
                require(semantic[left] != semantic[right], ClutchError::MismatchedState)?;
                right += 1;
            }
            require(
                physical
                    .iter()
                    .all(|account| account.to_bytes() != semantic[left].bytes()),
                ClutchError::AccountAlias,
            )?;
            left += 1;
        }
        Ok(())
    }

    fn general_founding_capability_id(self) -> Outcome<ContentId> {
        self.validate()?;
        let id = ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                GENERAL_PRE_ROOT_FOUNDING_CAPABILITY_DOMAIN_V3,
                self.program_id.as_ref(),
                &self.product_preauthorization_id.bytes(),
                &self.product_graph_observation_id.bytes(),
                &self.market_instance_v2_id.bytes(),
                &self.product_generation.to_le_bytes(),
                &self.series_plan_v5_id.bytes(),
                &self.series_ordinal.to_le_bytes(),
                &self.compiler_bundle_v5_id.bytes(),
                &self.capability_profile_v4_id.bytes(),
                &self.attachment_plan_v4_id.bytes(),
                &self.product_template_id.bytes(),
                &self.native_claim_basis_id.bytes(),
                &self.price_measure_policy_id.bytes(),
                &self.market_genesis_profile_id.bytes(),
                self.product_market_root_account.as_ref(),
                self.series_market_link_account.as_ref(),
                self.market_binding_account.as_ref(),
                self.market_runtime_account.as_ref(),
                &self.market_liability_founding_id.bytes(),
                &self.claim_mint_founding_plan_id.bytes(),
                &self.claim_issuance_binding_id.bytes(),
                &self.collateral_authentication_id.bytes(),
            ])
            .to_bytes(),
        );
        require(!id.is_zero(), ClutchError::MismatchedState)?;
        require(
            id != self.product_preauthorization_id
                && id != self.product_graph_observation_id
                && id != self.market_liability_founding_id
                && id != self.claim_mint_founding_plan_id
                && id != self.claim_issuance_binding_id
                && id != self.collateral_authentication_id
                && self.product_market_root_account.to_bytes() != id.bytes()
                && self.series_market_link_account.to_bytes() != id.bytes()
                && self.market_binding_account.to_bytes() != id.bytes()
                && self.market_runtime_account.to_bytes() != id.bytes(),
            ClutchError::MismatchedState,
        )?;
        Ok(id)
    }
}

/// Join Product's exact pre-root graph to Collateral's exact founding proof.
///
/// This constructor reads no root or link account.  Their canonical future
/// addresses are already bound by Product, while General independently derives
/// the sole `0x79/v3` and Runtime PDAs from the authenticated Market instance.
pub(crate) fn prepare_general_market_pre_root_founding_v3<C>(
    program_id: &Pubkey,
    product: AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
    collateral: &C,
) -> Outcome<AuthenticatedGeneralMarketPreRootFoundingPlanV3>
where
    C: AuthenticatedGeneralPreRootCollateralFoundingV3 + ?Sized,
{
    let market = product.market_instance_id().bytes();
    let market_binding_account = seeds::general_v2_market_binding_pda(program_id, &market).0;
    let market_runtime_account =
        seeds::general_v2_market_runtime_pda(program_id, &market_binding_account.to_bytes()).0;
    collateral.authenticate_general_pre_root_collateral_founding_v3(
        &product,
        market_runtime_account,
    )?;
    require(
        collateral.market_instance_v2_id() == product.market_instance_id()
            && collateral.claim_mint_authority() == market_runtime_account,
        ClutchError::MismatchedState,
    )?;
    let facts = GeneralPreRootFoundingFactsV3 {
        program_id: *program_id,
        product_preauthorization_id: product.id(),
        product_graph_observation_id: product.graph_observation_id(),
        market_instance_v2_id: product.market_instance_id(),
        product_generation: product.generation(),
        series_plan_v5_id: product.series_plan_id(),
        series_ordinal: product.ordinal(),
        compiler_bundle_v5_id: product.compiler_bundle_id(),
        capability_profile_v4_id: product.capability_profile_id(),
        attachment_plan_v4_id: product.attachment_plan_id(),
        product_template_id: product.product_template_id(),
        native_claim_basis_id: product.native_claim_basis_id(),
        price_measure_policy_id: product.price_measure_policy_id(),
        market_genesis_profile_id: product.market_genesis_profile_id(),
        product_market_root_account: product.lifecycle_root_account(),
        series_market_link_account: product.founder_link_account(),
        market_binding_account,
        market_runtime_account,
        market_liability_founding_id: collateral.market_liability_founding_id(),
        claim_mint_founding_plan_id: collateral.claim_mint_founding_plan_id(),
        claim_issuance_binding_id: collateral.claim_issuance_binding_id(),
        collateral_authentication_id: collateral.authentication_id(),
    };
    let general_founding_capability_id = facts.general_founding_capability_id()?;
    Ok(AuthenticatedGeneralMarketPreRootFoundingPlanV3 {
        product,
        market_binding_account,
        market_runtime_account,
        market_liability_founding_id: facts.market_liability_founding_id,
        claim_mint_founding_plan_id: facts.claim_mint_founding_plan_id,
        claim_issuance_binding_id: facts.claim_issuance_binding_id,
        collateral_authentication_id: facts.collateral_authentication_id,
        general_founding_capability_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn facts() -> GeneralPreRootFoundingFactsV3 {
        GeneralPreRootFoundingFactsV3 {
            program_id: Pubkey::new_from_array([1; 32]),
            product_preauthorization_id: id(2),
            product_graph_observation_id: id(3),
            market_instance_v2_id: MarketInstanceV2Id::from_bytes([4; 32]),
            product_generation: 5,
            series_plan_v5_id: SeriesPlanV5Id::from_bytes([6; 32]),
            series_ordinal: 7,
            compiler_bundle_v5_id: id(8),
            capability_profile_v4_id: id(9),
            attachment_plan_v4_id: id(10),
            product_template_id: id(11),
            native_claim_basis_id: id(12),
            price_measure_policy_id: id(13),
            market_genesis_profile_id: id(14),
            product_market_root_account: Pubkey::new_from_array([15; 32]),
            series_market_link_account: Pubkey::new_from_array([16; 32]),
            market_binding_account: Pubkey::new_from_array([17; 32]),
            market_runtime_account: Pubkey::new_from_array([18; 32]),
            market_liability_founding_id: id(19),
            claim_mint_founding_plan_id: id(20),
            claim_issuance_binding_id: id(21),
            collateral_authentication_id: id(22),
        }
    }

    #[test]
    fn capability_commits_product_collateral_and_both_general_pdas() {
        let exact = facts();
        let expected = exact.general_founding_capability_id().unwrap();
        for changed in [
            {
                let mut value = exact;
                value.product_preauthorization_id = id(23);
                value
            },
            {
                let mut value = exact;
                value.claim_mint_founding_plan_id = id(24);
                value
            },
            {
                let mut value = exact;
                value.claim_issuance_binding_id = id(25);
                value
            },
            {
                let mut value = exact;
                value.market_binding_account = Pubkey::new_from_array([26; 32]);
                value
            },
            {
                let mut value = exact;
                value.market_runtime_account = Pubkey::new_from_array([27; 32]);
                value
            },
        ] {
            assert_ne!(changed.general_founding_capability_id().unwrap(), expected);
        }
    }

    #[test]
    fn aliases_zero_generation_and_founder_id_substitution_refuse() {
        let mut account_alias = facts();
        account_alias.market_runtime_account = account_alias.market_binding_account;
        assert!(account_alias.general_founding_capability_id().is_err());

        let mut identity_alias = facts();
        identity_alias.claim_issuance_binding_id = identity_alias.claim_mint_founding_plan_id;
        assert!(identity_alias.general_founding_capability_id().is_err());

        let mut zero_generation = facts();
        zero_generation.product_generation = 0;
        assert!(zero_generation.general_founding_capability_id().is_err());

        let mut physical_identity_alias = facts();
        physical_identity_alias.market_liability_founding_id =
            ContentId::from_bytes(physical_identity_alias.market_runtime_account.to_bytes());
        assert!(physical_identity_alias.general_founding_capability_id().is_err());
    }
}
