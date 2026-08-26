//! Pure Dealer projector for the canonical Trading role.
//!
//! The common Trading outer layer owns Registry/current-deployment and
//! finalized-record authentication. This module consumes that authenticated
//! context, validates the exact Dealer schemas and account-owned views, and
//! reconstructs the semantic Dealer input without persisting Claims inventory
//! or Custody balances. It performs no CPI, account creation, or commit.

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1, CapabilityProgramV1, CapabilityRootAccountV1,
    SupportedContentV1,
};
use dclutch_claims_svm::ClaimsPositionSeedsV1;
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CompartmentV1, CustodyVaultSeedsV1,
};
use dclutch_dealer_codec::{
    CandidateView, MAX_OUTCOMES, Plan, Policy, State, interpret_projected,
    root_tail::{AuthorityObservation, ROOT_TAIL_BYTES, RootTail},
    trading_request::TradingRequest,
};
use dclutch_economic_slice_kernel::{
    position_market_id, position_native, position_owner, position_revision,
};
use dclutch_token_svm::{AccountState, TokenAccount};
use solana_program::{hash::hash, pubkey::Pubkey};

use crate::dispatch::TradingFamilyContextV1;

/// Read-only admitted-AOT evaluator for scenario exact-fill.
pub mod v3_admitted;
/// Finalized RequestProfile/Transition/Effect joins for junior equity.
pub mod v3_artifacts;
/// Atomic scenario-solvent Claims/Custody portfolio-fill composition.
pub mod v3_composer;
/// Exact scenario-residual junior pool-equity kernel.
pub mod v3_equity;
/// Canonical sparse SignedDeltaV3 packet for junior-equity Claims movement.
pub mod v3_equity_claims;
/// Runtime-width chain-derived junior-equity contribution/redemption requests.
pub mod v3_equity_operator;
/// Reproducible typed Hot EffectProgram artifact for junior equity.
pub mod v3_hot_artifact;
/// Canonical Claims Position plus Trading obligation activation and retirement.
pub mod v3_lifecycle;
/// Canonical LP Open/Close Profile6 and lifecycle artifacts.
pub mod v3_lp_artifacts;
/// Scenario-solvent, custody-backed multi-LP capital under canonical Trading.
pub mod v3_multi_lp;
/// Trading-owned runtime-width terminal obligations for scenario-solvent Dealer V3.
pub mod v3_obligation;
/// Chain-derived unsigned requests for every Dealer V3 multi-LP action.
pub mod v3_operator;
/// Exact logical AccountProfile for admitted junior-equity execution.
pub mod v3_profile;
/// One global selector authority and finalized V3 capability descriptors.
pub mod v3_release;
/// EffectProgram V3 admission for exact Dealer Custody request sequences.
pub mod v3_route;
/// Runtime-width exact trade requests and scenario-solvent physical composition.
pub mod v3_trade;
/// Selector-9 Request/Transition/Effect V4 artifact joins.
pub mod v3_trade_artifacts;

/// Canonical Dealer capability-kind label.
pub const DEALER_KIND_PREIMAGE_V2: &[u8] = b"dclutch/capability/dealer-v2";
/// Canonical immutable Dealer Policy config schema.
pub const DEALER_CONFIG_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/dealer-config-v2";
/// Canonical hot request schema with exact Claims Position revision.
pub const DEALER_REQUEST_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/dealer-request-v2";
/// Canonical inventory-free mutable root-tail schema.
pub const DEALER_ROOT_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/dealer-root-tail-v2";
/// Canonical Dealer account/register projection profile.
pub const DEALER_ACCOUNT_PROFILE_PREIMAGE_V2: &[u8] = b"dclutch/account-profile/dealer-v2";
/// Canonical bounded Dealer child-effect plan schema.
pub const DEALER_EFFECT_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/effect/dealer-v2";
/// Candidate PDA domain beneath one immutable Trading child root.
pub const DEALER_CANDIDATE_PDA_DOMAIN_V2: &[u8] = b"dclutch:dealer-candidate:v2";

const _: () = assert!(DEALER_CANDIDATE_PDA_DOMAIN_V2.len() <= 32);

/// Stable refusal from the pure Dealer profile boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerProfileError {
    /// Descriptor, selector, config, or immutable root coordinates refused.
    Content,
    /// Candidate bytes, owner, identity, revision, or child-root PDA refused.
    Candidate,
    /// Canonical Claims Position identity, PDA, revision, width, or data refused.
    Position,
    /// Canonical Dealer Custody vault identity or token state refused.
    Vault,
    /// Mutable composite-root tail or reconstructed semantic state refused.
    Tail,
}

/// Return the exact six schema IDs implemented by this physical projector.
pub fn supported_content_v2() -> Result<SupportedContentV1, DealerProfileError> {
    Ok(SupportedContentV1 {
        config_schema: content_id(DEALER_CONFIG_SCHEMA_PREIMAGE_V2)?,
        request_schema: content_id(DEALER_REQUEST_SCHEMA_PREIMAGE_V2)?,
        root_schema: content_id(DEALER_ROOT_SCHEMA_PREIMAGE_V2)?,
        account_profile: content_id(DEALER_ACCOUNT_PROFILE_PREIMAGE_V2)?,
        derivation_policy: ContentId::new(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1)
            .map_err(|_| DealerProfileError::Content)?,
        effect_schema: content_id(DEALER_EFFECT_SCHEMA_PREIMAGE_V2)?,
    })
}

/// Return the exact Dealer capability-kind identity.
pub fn dealer_kind_v2() -> Result<ContentId, DealerProfileError> {
    content_id(DEALER_KIND_PREIMAGE_V2)
}

fn content_id(preimage: &[u8]) -> Result<ContentId, DealerProfileError> {
    ContentId::new(hash(preimage).to_bytes()).map_err(|_| DealerProfileError::Content)
}

/// Descriptor- and root-authenticated Dealer profile projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerProfileV2 {
    context: TradingFamilyContextV1,
    policy: Policy,
}

impl DealerProfileV2 {
    /// Rejoin one common-authenticated descriptor/config/context to Dealer V2.
    ///
    /// `descriptor` must be the exact value returned by the common Trading
    /// dispatch. This method defensively rechecks its persisted selector,
    /// schemas, config digest, root width, Market, and release set.
    pub fn authenticate_after_common_dispatch(
        context: TradingFamilyContextV1,
        descriptor: CapabilityProgramV1<'_>,
        config_bytes: &[u8],
    ) -> Result<Self, DealerProfileError> {
        descriptor
            .validate_persisted_selection(context.selection())
            .map_err(|_| DealerProfileError::Content)?;
        supported_content_v2()?
            .require(descriptor)
            .map_err(|_| DealerProfileError::Content)?;
        if usize::try_from(descriptor.root_state_bytes()).ok() != Some(ROOT_TAIL_BYTES)
            || descriptor.root_account_bytes().ok() != Some(context.root_account_bytes())
            || hash(config_bytes).to_bytes() != context.selection().config().to_bytes()
        {
            return Err(DealerProfileError::Content);
        }
        let policy = Policy::decode(config_bytes).map_err(|_| DealerProfileError::Content)?;
        if policy.market_id != context.market()
            || policy.release_set_id != context.release_set().to_bytes()
        {
            return Err(DealerProfileError::Content);
        }
        Ok(Self { context, policy })
    }

    /// Return the current immutable Trading family context.
    pub const fn context(self) -> TradingFamilyContextV1 {
        self.context
    }

    /// Return the exact immutable Dealer Policy decoded from selected config.
    pub const fn policy(self) -> Policy {
        self.policy
    }

    /// Decode the exact inventory-free tail from this authenticated root.
    pub fn root_tail(
        self,
        descriptor: CapabilityProgramV1<'_>,
        root_account_data: &[u8],
    ) -> Result<RootTail, DealerProfileError> {
        let root = CapabilityRootAccountV1::decode(root_account_data, descriptor)
            .map_err(|_| DealerProfileError::Tail)?;
        let header = root.header();
        if header.release_set() != self.context.release_set()
            || header.market() != self.context.market()
            || header.generation() != self.context.generation()
            || header.selection() != self.context.selection()
        {
            return Err(DealerProfileError::Tail);
        }
        RootTail::decode(root.state()).map_err(|_| DealerProfileError::Tail)
    }

    /// Authenticate one Candidate account beneath this exact child root.
    pub fn candidate<'a>(
        self,
        observed_key: &Pubkey,
        observed_owner: &Pubkey,
        bytes: &'a [u8],
    ) -> Result<CandidateView<'a>, DealerProfileError> {
        let candidate = CandidateView::decode(bytes).map_err(|_| DealerProfileError::Candidate)?;
        if candidate.outcome_count != self.policy.outcome_count
            || candidate.work_funding < self.policy.minimum_work_funding
            || observed_owner.to_bytes() != self.context.program_id()
        {
            return Err(DealerProfileError::Candidate);
        }
        let child_root = self.context.child_root_key();
        let expected = Pubkey::find_program_address(
            &[
                DEALER_CANDIDATE_PDA_DOMAIN_V2,
                &child_root,
                &candidate.candidate_id,
            ],
            &Pubkey::new_from_array(self.context.program_id()),
        )
        .0;
        if expected != *observed_key {
            return Err(DealerProfileError::Candidate);
        }
        Ok(candidate)
    }

    /// Reconstruct one transaction-local semantic state from sole owners.
    pub fn materialize_state(
        self,
        tail: RootTail,
        active: CandidateView<'_>,
        pending: Option<CandidateView<'_>>,
        position: DealerPositionProjectionV2,
        vaults: DealerVaultProjectionV2,
        terminal_winner: Option<u8>,
    ) -> Result<State, DealerProfileError> {
        tail.materialize(
            self.policy,
            active,
            pending,
            AuthorityObservation {
                release_set_id: self.context.release_set().to_bytes(),
                inventory: position.native(),
                quote_custody: vaults.trading_principal,
                fee_custody: vaults.fee_vault,
                liveness_custody: vaults.liveness_vault,
                terminal_winner,
            },
        )
        .map_err(|_| DealerProfileError::Tail)
    }
}

/// Ephemeral canonical Dealer Claims Position projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPositionProjectionV2 {
    revision: u64,
    count: u8,
    native: [u64; MAX_OUTCOMES],
}

impl DealerPositionProjectionV2 {
    /// Authenticate one exact Claims-owned Position and project native claims.
    pub fn authenticate(
        profile: DealerProfileV2,
        current_claims_program: &Pubkey,
        observed_key: &Pubkey,
        observed_owner: &Pubkey,
        position_bytes: &[u8],
        expected_revision: u64,
    ) -> Result<Self, DealerProfileError> {
        if observed_owner != current_claims_program {
            return Err(DealerProfileError::Position);
        }
        let market = profile.context.market();
        // The internal inventory Position is owned by the immutable Trading
        // child root. `policy.dealer_id` is the external capital owner and is
        // never reused as protocol custody. This makes add/remove and fills
        // real Claims transfers between distinct canonical Positions.
        let holder = profile.context.child_root_key();
        let seeds =
            ClaimsPositionSeedsV1::new(market, holder).map_err(|_| DealerProfileError::Position)?;
        let expected = Pubkey::find_program_address(&seeds.as_slices(), current_claims_program).0;
        let count_u32 = u32::from(profile.policy.outcome_count);
        if expected != *observed_key
            || position_market_id(position_bytes, count_u32).ok() != Some(market)
            || position_owner(position_bytes, count_u32).ok() != Some(holder)
            || position_revision(position_bytes, count_u32).ok() != Some(expected_revision)
        {
            return Err(DealerProfileError::Position);
        }
        let mut native = [0_u64; MAX_OUTCOMES];
        let count = usize::from(profile.policy.outcome_count);
        for (outcome, slot) in native.iter_mut().take(count).enumerate() {
            let outcome = u32::try_from(outcome).map_err(|_| DealerProfileError::Position)?;
            *slot = position_native(position_bytes, count_u32, outcome)
                .map_err(|_| DealerProfileError::Position)?;
        }
        Ok(Self {
            revision: expected_revision,
            count: profile.policy.outcome_count,
            native,
        })
    }

    /// Return the exact optimistic Claims Position revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Borrow the runtime-width native Claims quantities.
    pub fn native(&self) -> &[u64] {
        self.native.get(..usize::from(self.count)).unwrap_or(&[])
    }
}

/// One already parsed SVM token-account observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VaultAccountObservationV2 {
    /// SVM account key.
    pub key: Pubkey,
    /// SVM owner, which must be the exact Realm-selected token program.
    pub owner_program: Pubkey,
    /// Canonically parsed exact-width token state.
    pub token: TokenAccount,
}

/// Exact balances of all three canonical Dealer state-bearing vaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerVaultProjectionV2 {
    /// Dealer TradingPrincipal amount.
    pub trading_principal: u64,
    /// Realized FeeVault amount.
    pub fee_vault: u64,
    /// Present LivenessVault amount.
    pub liveness_vault: u64,
}

impl DealerVaultProjectionV2 {
    /// Authenticate all state-bearing Dealer vaults on every route.
    pub fn authenticate(
        profile: DealerProfileV2,
        current_custody_program: &Pubkey,
        token_program: &Pubkey,
        collateral_mint: &Pubkey,
        observations: [VaultAccountObservationV2; 3],
    ) -> Result<Self, DealerProfileError> {
        if observations[0].key == observations[1].key
            || observations[0].key == observations[2].key
            || observations[1].key == observations[2].key
        {
            return Err(DealerProfileError::Vault);
        }
        let market = profile.context.market();
        let release = profile.context.release_set().to_bytes();
        let context = profile.context.child_root_key();
        let expected_authority = Pubkey::find_program_address(
            &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market, &release],
            current_custody_program,
        )
        .0;
        for (observation, compartment) in observations.iter().zip([
            CompartmentV1::TradingPrincipal,
            CompartmentV1::FeeVault,
            CompartmentV1::LivenessVault,
        ]) {
            let seeds = CustodyVaultSeedsV1::new(market, release, context, compartment);
            let expected =
                Pubkey::find_program_address(&seeds.as_slices(), current_custody_program).0;
            if observation.key != expected
                || observation.owner_program != *token_program
                || observation.token.mint != collateral_mint.to_bytes()
                || observation.token.owner != expected_authority.to_bytes()
                || observation.token.state != AccountState::Initialized
                || !observation.token.delegate.is_none()
                || observation.token.delegated_amount != 0
                || !observation.token.native_reserve.is_none()
                || !observation.token.close_authority.is_none()
            {
                return Err(DealerProfileError::Vault);
            }
        }
        Ok(Self {
            trading_principal: observations[0].token.amount,
            fee_vault: observations[1].token.amount,
            liveness_vault: observations[2].token.amount,
        })
    }
}

/// Decode and bind the exact hot request to the projected Claims revision.
pub fn authenticate_request_v2(
    bytes: &[u8],
    position: DealerPositionProjectionV2,
) -> Result<TradingRequest, DealerProfileError> {
    let request = TradingRequest::decode(bytes).map_err(|_| DealerProfileError::Content)?;
    if request.expected_position_revision != position.revision() {
        return Err(DealerProfileError::Position);
    }
    Ok(request)
}

/// Exact transaction-local Dealer plan and Dealer-owned post-tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTransitionProjectionV2 {
    post_tail: RootTail,
    plan: Plan,
    outcome_count: u8,
    post_inventory: [u64; MAX_OUTCOMES],
}

impl DealerTransitionProjectionV2 {
    /// Return the only bytes eligible for commit to the mutable root tail.
    pub const fn post_tail(self) -> RootTail {
        self.post_tail
    }

    /// Return the exact Claims/Custody intent to refine into child requests.
    pub const fn plan(self) -> Plan {
        self.plan
    }

    /// Borrow the exact expected canonical Claims post-inventory.
    pub fn post_inventory(&self) -> &[u64] {
        self.post_inventory
            .get(..usize::from(self.outcome_count))
            .unwrap_or(&[])
    }
}

/// Transaction-local inputs for one Dealer transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerInterpretationInputV2<'a> {
    /// Inventory-free Dealer-owned root tail.
    pub tail: RootTail,
    /// Exact currently active immutable quote candidate.
    pub active: CandidateView<'a>,
    /// Exact scheduled replacement candidate, when present.
    pub pending: Option<CandidateView<'a>>,
    /// Candidate proposed by this request, when the action requires one.
    pub proposed: Option<CandidateView<'a>>,
    /// Ephemeral canonical Claims Position projection.
    pub position: DealerPositionProjectionV2,
    /// Ephemeral canonical Custody vault projection.
    pub vaults: DealerVaultProjectionV2,
    /// Canonical Core terminal winner, when terminal.
    pub terminal_winner: Option<u8>,
    /// Exact authenticated Dealer hot request.
    pub request: TradingRequest,
}

/// Authenticate the complete initial economic state before creating a Dealer root.
///
/// Activation requires present principal, work funding, and every native-claim
/// coordinate to already satisfy the immutable active Candidate. It therefore
/// cannot create an underfunded root and hope that a later transaction repairs
/// it. Claims and Custody remain the sole balance owners.
pub fn initialize_projected_v2(
    profile: DealerProfileV2,
    active: CandidateView<'_>,
    position: DealerPositionProjectionV2,
    vaults: DealerVaultProjectionV2,
) -> Result<RootTail, DealerProfileError> {
    if position.revision() != 0
        || vaults.fee_vault != 0
        || vaults.liveness_vault != active.work_funding
        || vaults.trading_principal < active.quote_reserve_floor
    {
        return Err(DealerProfileError::Tail);
    }
    let tail = RootTail::initialize(active);
    profile.materialize_state(tail, active, None, position, vaults, None)?;
    Ok(tail)
}

/// Interpret one request after every sole-owner projection is authenticated.
///
/// The full semantic post-state exists only long enough to derive child plans,
/// postconditions, and the inventory-free Dealer tail. The outer Trading layer
/// must apply child effects, verify their receipts and exact post-inventory,
/// then commit `post_tail` last.
pub fn interpret_projected_v2(
    profile: DealerProfileV2,
    input: DealerInterpretationInputV2<'_>,
) -> Result<DealerTransitionProjectionV2, DealerProfileError> {
    if input.request.expected_position_revision != input.position.revision() {
        return Err(DealerProfileError::Position);
    }
    let state = profile.materialize_state(
        input.tail,
        input.active,
        input.pending,
        input.position,
        input.vaults,
        input.terminal_winner,
    )?;
    let transition = interpret_projected(
        profile.policy,
        input.active,
        input.pending,
        input.proposed,
        state,
        input
            .request
            .semantic_request()
            .map_err(|_| DealerProfileError::Content)?,
    )
    .map_err(|_| DealerProfileError::Tail)?;
    Ok(DealerTransitionProjectionV2 {
        post_tail: RootTail::from_validated_post(transition.post),
        plan: transition.plan,
        outcome_count: profile.policy.outcome_count,
        post_inventory: transition.post.inventory,
    })
}

#[cfg(test)]
mod tests;
