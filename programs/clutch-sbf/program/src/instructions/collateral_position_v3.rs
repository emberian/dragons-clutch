//! Authenticated SBF authority for canonical General Position V3 cash state.
//!
//! This module deliberately authenticates only the full-width General side of
//! the join. A legacy Market/Hoard caller must separately prove how its
//! lowered runtime coordinate belongs to the full MarketInstanceV2; equality
//! between those two identities is never an admitted substitute.

use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, ClaimLedgerV3, HoardV2,
    Id as CollateralId, MarketCollateralBindingV2, CLAIM_LEDGER_V3_BYTES, HOARD_V2_BYTES,
};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, GeneralPositionReplayPrestateV1, Id32,
    MarketBindingV1, MarketRuntimeV3AccountV1, MARKET_BINDING_ACCOUNT_BYTES,
    MARKET_RUNTIME_ACCOUNT_BYTES,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_product_series::MarketInstancePreimageV2;
use clutch_retirement::{
    project_general_position_v3, AdapterPositionMarketBindingV3, AdapterPositionPurposeBindingV3,
    GeneralPositionProjectionV3, Identity32V1, PositionAccountV3, PositionPurposeV3,
    PositionV3Sha256Backend, ReplayV3HashBackend, POSITION_V3_BYTES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;

use super::product_artifact::authenticate_product_artifact_v1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeSha256;

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Complete authenticated ordinary-Position and GEN1 Replay prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralPositionReplayAuthorityV1 {
    pub(crate) position: AuthenticatedPositionV3,
    pub(crate) projection: GeneralPositionProjectionV3,
    pub(crate) replay: GeneralPositionReplayPrestateV1,
    pub(crate) market_binding: MarketBindingV1,
    pub(crate) market_runtime: MarketRuntimeV3AccountV1,
}

/// Authenticated full-width collateral and native-claim Market owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralMarketLiabilityAuthorityV1 {
    pub(crate) bound: BoundCollateralProfileV2,
    pub(crate) market_binding: MarketBindingV1,
    pub(crate) market_runtime: MarketRuntimeV3AccountV1,
    pub(crate) market_instance: MarketInstancePreimageV2,
    pub(crate) hoard: HoardV2,
    pub(crate) claim_ledger: ClaimLedgerV3,
}

fn require_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

/// Authenticate the immutable General MarketBinding and its stable runtime.
pub(crate) fn authenticate_general_market_v1(
    program_id: &Pubkey,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
) -> Outcome<(MarketBindingV1, MarketRuntimeV3AccountV1)> {
    require_program_account(
        program_id,
        market_binding_account,
        false,
        MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_program_account(
        program_id,
        market_runtime_account,
        false,
        MARKET_RUNTIME_ACCOUNT_BYTES,
    )?;
    let binding = MarketBindingV1::decode(&market_binding_account.data.borrow())?;
    let runtime = MarketRuntimeV3AccountV1::decode(&market_runtime_account.data.borrow())?;
    expect_pda(
        market_binding_account.key,
        seeds::general_v2_market_binding_pda(program_id, &binding.market_instance_v2_id.bytes()),
        Some(binding.stored_bump),
    )?;
    expect_pda(
        market_runtime_account.key,
        seeds::general_v2_market_runtime_pda(program_id, &market_binding_account.key.to_bytes()),
        Some(runtime.stored_bump),
    )?;
    require(
        binding.market.bytes() == market_runtime_account.key.to_bytes()
            && runtime.market_binding.bytes() == market_binding_account.key.to_bytes()
            && runtime.market_instance_v2_id == binding.market_instance_v2_id,
        ClutchError::MismatchedState,
    )?;
    Ok((binding, runtime))
}

/// Authenticate ProfileV2 collateral, the full Product MarketInstance, and
/// the replacement Hoard/ClaimLedger accounts without consulting any lowered
/// legacy Market, Kernel, or SupplyLedger identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_market_liabilities_v1(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
    hoard_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
    hoard_writable: bool,
    claim_ledger_writable: bool,
) -> Outcome<GeneralMarketLiabilityAuthorityV1> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let (market_binding, market_runtime) =
        authenticate_general_market_v1(program_id, market_binding_account, market_runtime_account)?;
    let market_instance_artifact = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        market_binding.market_instance_v2_id.content_id(),
    )?;
    let market_instance = *market_instance_artifact.value();
    let market_instance_id = market_instance
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        market_instance_id.bytes() == market_binding.market_instance_v2_id.bytes()
            && market_instance.market_genesis_profile_id.bytes()
                == market_binding.market_genesis_profile_v2_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require_program_account(program_id, hoard_account, hoard_writable, HOARD_V2_BYTES)?;
    require_program_account(
        program_id,
        claim_ledger_account,
        claim_ledger_writable,
        CLAIM_LEDGER_V3_BYTES,
    )?;
    let hoard = HoardV2::decode(&hoard_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger = ClaimLedgerV3::decode(&claim_ledger_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_bytes = market_binding.market_instance_v2_id.bytes();
    expect_pda(
        hoard_account.key,
        seeds::hoard_v2_pda(program_id, &market_bytes),
        Some(hoard.stored_bump),
    )?;
    expect_pda(
        claim_ledger_account.key,
        seeds::claim_ledger_v3_pda(program_id, &market_bytes),
        Some(claim_ledger.stored_bump),
    )?;
    let authority = seeds::hoard_authority_v2_pda(program_id, &market_bytes).0;
    let token_account = seeds::hoard_token_v2_pda(program_id, &market_bytes).0;
    require(
        hoard.market_instance_id == CollateralId::from_bytes(market_bytes)
            && hoard.realm_id == realm.realm().realm
            && hoard.profile_id == realm.realm().profile
            && hoard.collateral_policy_id == realm.policy_id()
            && hoard.collateral_release_id
                == realm
                    .release()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && hoard.authority == CollateralId::from_bytes(authority.to_bytes())
            && hoard.token_account == CollateralId::from_bytes(token_account.to_bytes())
            && hoard.collateral_cap_atoms == market_instance.collateral_cap
            && claim_ledger.market_instance_id == hoard.market_instance_id
            && claim_ledger.realm_id == hoard.realm_id
            && claim_ledger.native_claim_basis_id
                == CollateralId::from_bytes(market_binding.native_claim_basis_id.bytes())
            && claim_ledger.lifecycle == hoard.lifecycle
            && claim_ledger.outcome_count == hoard.outcome_count
            && claim_ledger.outcome_count == market_binding.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let bound = refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market_bytes),
            realm: hoard.realm_id,
            profile: hoard.profile_id,
            collateral_cap_atoms: hoard.collateral_cap_atoms,
            hoard_authority: hoard.authority,
            hoard_token_account: hoard.token_account,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(GeneralMarketLiabilityAuthorityV1 {
        bound,
        market_binding,
        market_runtime,
        market_instance,
        hoard,
        claim_ledger,
    })
}

/// Authenticate one existing canonical ordinary Position and its exact GEN1
/// Replay. This function cannot be used to authenticate Dealer, Series, or
/// structured-claim Positions because their purpose binding is different.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_position_replay_v1(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
    expected_sequence: u64,
) -> Outcome<GeneralPositionReplayAuthorityV1> {
    let (market_binding, market_runtime) =
        authenticate_general_market_v1(program_id, market_binding_account, market_runtime_account)?;
    require_program_account(program_id, position_account, true, POSITION_V3_BYTES)?;
    require_program_account(
        program_id,
        replay_account,
        true,
        clutch_general_v2_contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES,
    )?;
    require(
        bound.market().market
            == CollateralId::from_bytes(market_binding.market_instance_v2_id.bytes())
            && bound.market().realm
                == CollateralId::from_bytes(bound.realm_bound().realm().realm.bytes())
            && bound.market().profile
                == CollateralId::from_bytes(bound.realm_bound().realm().profile.bytes()),
        ClutchError::MismatchedState,
    )?;

    let position = PositionAccountV3::decode(&position_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fields = position.fields();
    let purpose_binding = Identity32V1::new(market_runtime_account.key.to_bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let owner = Identity32V1::new(expected_owner)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_identity = Identity32V1::new(market_binding.market_instance_v2_id.bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_position = seeds::position_v3_pda(
        program_id,
        &market_identity.bytes(),
        &expected_owner,
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    expect_pda(
        position_account.key,
        expected_position,
        Some(position.stored_bump()),
    )?;
    let expected_replay = seeds::purpose_replay_v3_pda(
        program_id,
        &position_account.key.to_bytes(),
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    expect_pda(replay_account.key, expected_replay, None)?;
    require(
        fields.replay_account.bytes() == replay_account.key.to_bytes()
            && fields.owner == owner
            && fields.controller == owner
            && fields.purpose_binding_id == purpose_binding,
        ClutchError::MismatchedState,
    )?;

    let projection = project_general_position_v3(
        position,
        AdapterPositionMarketBindingV3 {
            market_instance_id: market_identity,
            outcome_count: market_binding.outcome_count,
            realm_id: Identity32V1::new(bound.market().realm.bytes())
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            collateral_policy_id: Identity32V1::new(bound.policy_id().bytes())
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            collateral_release_id: Identity32V1::new(
                bound
                    .release()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .bytes(),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        },
        AdapterPositionPurposeBindingV3 {
            owner,
            controller: owner,
            purpose_binding_id: purpose_binding,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_id = position
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let authenticated = AuthenticatedPositionV3 {
        account: position_account.key.to_bytes(),
        general_market_runtime: market_runtime_account.key.to_bytes(),
        semantic: position,
        semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    let replay = project_general_position_replay_prestate_v1(
        Id32::new(replay_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        expected_replay.1,
        expected_sequence,
        &replay_account.data.borrow(),
        authenticated,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    Ok(GeneralPositionReplayAuthorityV1 {
        position: authenticated,
        projection,
        replay,
        market_binding,
        market_runtime,
    })
}
