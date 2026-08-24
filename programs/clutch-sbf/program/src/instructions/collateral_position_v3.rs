//! Authenticated SBF authority for canonical General Position V3 cash state.
//!
//! This module deliberately authenticates only the full-width General side of
//! the join. A legacy Market/Hoard caller must separately prove how its
//! lowered runtime coordinate belongs to the full MarketInstanceV2; equality
//! between those two identities is never an admitted substitute.

use clutch_collateral_adapter_v2::{
    accept_market_liability_founding_v3, refine_market_collateral_v2,
    AcceptedMarketLiabilityFoundingV3, BoundCollateralProfileV2, ClaimLedgerV3, HoardV2,
    Id as CollateralId, MarketCollateralBindingV2, MarketLiabilityFoundingPlanV3,
    MarketLiabilityFoundingPostwriteV3, MarketResolutionActivationPlanV5, ResolutionStateV5,
    ResolutionV5, RuntimeAccountViewV2, CLAIM_LEDGER_V3_BYTES, HOARD_V2_BYTES,
    RESOLUTION_V5_BYTES,
};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, GeneralPositionReplayPrestateV1, Id32,
    MarketBindingV1, MarketBindingV2, MarketBindingV4, MarketRuntimeV3AccountV1,
    MARKET_BINDING_ACCOUNT_BYTES, MARKET_BINDING_ACCOUNT_BYTES_V2,
    MARKET_BINDING_ACCOUNT_BYTES_V4, MARKET_RUNTIME_ACCOUNT_BYTES,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_product_series::{ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_retirement::{
    project_general_position_v3, AdapterPositionMarketBindingV3, AdapterPositionPurposeBindingV3,
    DeletableRentOwnerV1, GeneralPositionProjectionV3, Identity32V1, PositionAccountV3,
    PositionPurposeV3, PositionV3Sha256Backend, ReplayV3HashBackend, POSITION_V3_BYTES,
};
use clutch_solana_layout::collateral_v3_accounts::{
    validate_inferred_collateral_account_metas_with_v3, CollateralActionV3,
    ObservedCollateralAccountMetaV3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::seeds;

use super::product_artifact::authenticate_product_artifact_v1;

const GENERAL_MARKET_VALUE_AUTHORITY_DOMAIN_V2: &[u8] =
    b"dragons-clutch/general-market/value-authority/v2\0";
const GENERAL_MARKET_LIABILITY_AUTHORITY_DOMAIN_V2: &[u8] =
    b"dragons-clutch/general-market/liability-authority/v2\0";
const GENERAL_MARKET_BINDING_DATA_DOMAIN_V2: &[u8] =
    b"dragons-clutch/general-market/binding-data/v2\0";
const GENERAL_MARKET_BINDING_DATA_DOMAIN_V4: &[u8] =
    b"dragons-clutch/general-market/binding-data/v4\0";
const GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-market/runtime-data/v3\0";
const GENERAL_MARKET_LIABILITY_FOUNDING_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-market/liability-founding-postwrite/v3\0";
const MARKET_RESOLUTION_ACTIVATION_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/market-resolution/activation-postwrite/v5\0";

/// Enforce the central full-width account-role, privilege, and alias contract
/// over live effective AccountInfo metadata without allocation.
pub(crate) fn validate_full_width_collateral_accounts_v3(
    accounts: &[AccountInfo<'_>],
    action: CollateralActionV3,
    selected_outcome: Option<u8>,
) -> Outcome<u8> {
    validate_inferred_collateral_account_metas_with_v3(
        action,
        selected_outcome,
        accounts.len(),
        |index| {
            accounts
                .get(index)
                .map(|account| ObservedCollateralAccountMetaV3 {
                    key: account.key.to_bytes(),
                    writable: account.is_writable,
                    signer: account.is_signer,
                })
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

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

/// Complete authenticated ordinary-Position and GEN1 Replay prestate under
/// the sole live MarketBinding V2 account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralPositionReplayAuthorityV2 {
    pub(crate) position: AuthenticatedPositionV3,
    pub(crate) projection: GeneralPositionProjectionV3,
    pub(crate) replay: GeneralPositionReplayPrestateV1,
    pub(crate) market_binding: MarketBindingV2,
    pub(crate) market_runtime: MarketRuntimeV3AccountV1,
}

/// Complete authenticated ordinary-Position and GEN1 Replay prestate under
/// the sole current Product/Revenue-authorized MarketBinding V4 account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralPositionReplayAuthorityV4 {
    pub(crate) position: AuthenticatedPositionV3,
    pub(crate) projection: GeneralPositionProjectionV3,
    pub(crate) replay: GeneralPositionReplayPrestateV1,
    pub(crate) market_binding: MarketBindingV4,
    pub(crate) market_runtime: MarketRuntimeV3AccountV1,
}

/// Current General market bodies plus General-owned full account-data IDs.
/// Downstream families consume this projection rather than defining parallel
/// binding/runtime hash transcripts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedGeneralMarketV4 {
    binding: MarketBindingV4,
    runtime: MarketRuntimeV3AccountV1,
    binding_data_id: CollateralId,
    runtime_data_id: CollateralId,
}

impl AuthenticatedGeneralMarketV4 {
    pub(crate) const fn binding(self) -> MarketBindingV4 { self.binding }
    pub(crate) const fn runtime(self) -> MarketRuntimeV3AccountV1 { self.runtime }
    pub(crate) const fn binding_data_id(self) -> CollateralId { self.binding_data_id }
    pub(crate) const fn runtime_data_id(self) -> CollateralId { self.runtime_data_id }
}

/// Canonical current General/Realm/Product collateral join shared by every
/// V4 settlement action. Product artifact authentication and collateral
/// refinement have one SBF owner; action-specific adapters add only the
/// physical accounts (for example PriceGrid or retained Feed) they consume.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedGeneralMarketCollateralV4 {
    collateral: BoundCollateralProfileV2,
    market_binding: MarketBindingV4,
    market_runtime: MarketRuntimeV3AccountV1,
    market_genesis: MarketGenesisProfileV2,
}

impl AuthenticatedGeneralMarketCollateralV4 {
    pub(crate) const fn collateral(self) -> BoundCollateralProfileV2 { self.collateral }
    pub(crate) const fn market_binding(self) -> MarketBindingV4 { self.market_binding }
    pub(crate) const fn market_runtime(self) -> MarketRuntimeV3AccountV1 { self.market_runtime }
    pub(crate) const fn market_genesis(self) -> MarketGenesisProfileV2 { self.market_genesis }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneralPositionReplayBodyV2 {
    position: AuthenticatedPositionV3,
    projection: GeneralPositionProjectionV3,
    replay: GeneralPositionReplayPrestateV1,
}

/// Authenticated full-width collateral and native-claim Market owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralMarketLiabilityAuthorityV2 {
    pub(crate) bound: BoundCollateralProfileV2,
    pub(crate) market_binding: MarketBindingV2,
    pub(crate) market_runtime: MarketRuntimeV3AccountV1,
    pub(crate) market_instance: MarketInstancePreimageV2,
    pub(crate) hoard: HoardV2,
    pub(crate) claim_ledger: ClaimLedgerV3,
    pub(crate) market_binding_data_id: CollateralId,
    pub(crate) market_runtime_data_id: CollateralId,
    pub(crate) hoard_semantic_id: CollateralId,
    pub(crate) claim_ledger_semantic_id: CollateralId,
    pub(crate) hoard_lamports: u64,
    pub(crate) claim_ledger_lamports: u64,
    pub(crate) receipt_id: CollateralId,
}

/// Same-instruction current collateral deployment proof for a value-bearing
/// Hoard CPI. Internal reclassification routes cannot construct this type
/// without presenting the exact linked ProgramData account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralMarketValueAuthorityV2 {
    pub(crate) liabilities: GeneralMarketLiabilityAuthorityV2,
    pub(crate) deployment:
        crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2,
    pub(crate) receipt_id: CollateralId,
}

/// Private SBF proof that canonical liability state and Hoard custody were
/// founded under the exact current Profile-selected token deployment and
/// exact persisted rent balances.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketLiabilityFoundingPostwriteV3 {
    accepted: AcceptedMarketLiabilityFoundingV3,
    deployment: crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2,
    hoard_lamports: u64,
    claim_ledger_lamports: u64,
    receipt_id: CollateralId,
}

impl AuthenticatedMarketLiabilityFoundingPostwriteV3 {
    /// Exact current collateral token deployment observed in this instruction.
    pub(crate) const fn deployment(
        self,
    ) -> crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2 {
        self.deployment
    }

    /// Exact admitted HoardV2 lamport balance.
    pub(crate) const fn hoard_lamports(self) -> u64 {
        self.hoard_lamports
    }

    /// Exact admitted ClaimLedgerV3 lamport balance.
    pub(crate) const fn claim_ledger_lamports(self) -> u64 {
        self.claim_ledger_lamports
    }

    /// Product-consumable exact runtime postwrite receipt.
    pub(crate) const fn receipt_id(self) -> CollateralId {
        self.receipt_id
    }
}

/// Hostile-decoded full-width Resolution bound to its exact PDA and ledgers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedResolutionV5 {
    pub(crate) account_id: CollateralId,
    pub(crate) resolution: ResolutionV5,
    pub(crate) semantic_id: CollateralId,
    pub(crate) data_id: CollateralId,
}

/// Hostile-decoded receipt for the three exact Resolution activation
/// postimages. Construction remains private to this SBF adapter; Product and
/// Failure must separately authorize the transition before any write occurs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketResolutionActivationPostwriteV5 {
    plan: MarketResolutionActivationPlanV5,
    liability_authority_receipt_id: CollateralId,
    resolution_lamports: u64,
    hoard_lamports: u64,
    claim_ledger_lamports: u64,
    receipt_id: CollateralId,
}

impl AuthenticatedMarketResolutionActivationPostwriteV5 {
    pub(crate) const fn plan(self) -> MarketResolutionActivationPlanV5 {
        self.plan
    }

    pub(crate) const fn liability_authority_receipt_id(self) -> CollateralId {
        self.liability_authority_receipt_id
    }

    pub(crate) const fn resolution_lamports(self) -> u64 {
        self.resolution_lamports
    }

    pub(crate) const fn hoard_lamports(self) -> u64 {
        self.hoard_lamports
    }

    pub(crate) const fn claim_ledger_lamports(self) -> u64 {
        self.claim_ledger_lamports
    }

    pub(crate) const fn receipt_id(self) -> CollateralId {
        self.receipt_id
    }
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

fn required_deletable_rent_balance_v1(rent: DeletableRentOwnerV1) -> Outcome<u64> {
    rent.refundable_principal()
        .checked_add(rent.donation_floor())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

fn require_deletable_rent_coverage_v1(
    rent: DeletableRentOwnerV1,
    observed_lamports: u64,
    exact: bool,
) -> Outcome<()> {
    let required = required_deletable_rent_balance_v1(rent)?;
    require(
        if exact {
            observed_lamports == required
        } else {
            observed_lamports >= required
        },
        ClutchError::MismatchedState,
    )
}

fn authenticated_account_data_id_v1(
    domain: &[u8],
    account: &Pubkey,
    data: &[u8],
) -> Outcome<CollateralId> {
    let value = CollateralId::from_bytes(
        solana_sha256_hasher::hashv(&[domain, account.as_ref(), data]).to_bytes(),
    );
    value
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn market_liability_founding_postwrite_receipt_id_v3(
    accepted_receipt_id: CollateralId,
    founding_id: CollateralId,
    market_id: CollateralId,
    realm_id: CollateralId,
    policy_id: CollateralId,
    release_id: CollateralId,
    programdata_account: CollateralId,
    deployment_slot: u64,
    deployment_receipt_id: CollateralId,
    hoard_account: &Pubkey,
    hoard_id: CollateralId,
    hoard_lamports: u64,
    claim_ledger_account: &Pubkey,
    claim_ledger_id: CollateralId,
    claim_ledger_lamports: u64,
    hoard_token_account: &Pubkey,
    visible_hoard_atoms: u64,
) -> Outcome<CollateralId> {
    let receipt_id = CollateralId::from_bytes(
        solana_sha256_hasher::hashv(&[
            GENERAL_MARKET_LIABILITY_FOUNDING_POSTWRITE_DOMAIN_V3,
            &accepted_receipt_id.bytes(),
            &founding_id.bytes(),
            &market_id.bytes(),
            &realm_id.bytes(),
            &policy_id.bytes(),
            &release_id.bytes(),
            &programdata_account.bytes(),
            &deployment_slot.to_le_bytes(),
            &deployment_receipt_id.bytes(),
            hoard_account.as_ref(),
            &hoard_id.bytes(),
            &hoard_lamports.to_le_bytes(),
            claim_ledger_account.as_ref(),
            &claim_ledger_id.bytes(),
            &claim_ledger_lamports.to_le_bytes(),
            hoard_token_account.as_ref(),
            &visible_hoard_atoms.to_le_bytes(),
        ])
        .to_bytes(),
    );
    receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(receipt_id)
}

/// Authenticate exact full-width liability/custody postwrites and release the
/// private accepted founding receipt.
///
/// Product separately owns FoundationVault debit/rent evidence and the
/// replay-counted `Founding` transition. This helper proves only the state and
/// external custody facts owned by the collateral plane.
pub(crate) fn accept_general_market_liability_founding_postwrite_v3(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    deployment: crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2,
    plan: MarketLiabilityFoundingPlanV3,
    hoard_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
    hoard_token_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedMarketLiabilityFoundingPostwriteV3> {
    require_program_account(program_id, hoard_account, true, HOARD_V2_BYTES)?;
    require_program_account(
        program_id,
        claim_ledger_account,
        true,
        CLAIM_LEDGER_V3_BYTES,
    )?;
    let hoard = plan.hoard();
    let claim_ledger = plan.claim_ledger();
    let release_id = bound
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        deployment.release() == bound.release() && deployment.release_id() == release_id,
        ClutchError::AuthorizationUnavailable,
    )?;
    let hoard_lamports = hoard_account.lamports();
    let claim_ledger_lamports = claim_ledger_account.lamports();
    require_deletable_rent_coverage_v1(hoard.rent, hoard_lamports, true)?;
    require_deletable_rent_coverage_v1(
        claim_ledger.rent,
        claim_ledger_lamports,
        true,
    )?;
    let market = hoard.market_instance_id.bytes();
    expect_pda(
        hoard_account.key,
        seeds::hoard_v2_pda(program_id, &market),
        Some(hoard.stored_bump),
    )?;
    expect_pda(
        claim_ledger_account.key,
        seeds::claim_ledger_v3_pda(program_id, &market),
        Some(claim_ledger.stored_bump),
    )?;
    require(
        CollateralId::from_bytes(hoard_account.key.to_bytes()) == plan.hoard_account()
            && CollateralId::from_bytes(claim_ledger_account.key.to_bytes())
                == plan.claim_ledger_account()
            && seeds::hoard_authority_v2_pda(program_id, &market)
                .0
                .to_bytes()
                == hoard.authority.bytes()
            && seeds::hoard_token_v2_pda(program_id, &market).0.to_bytes()
                == hoard.token_account.bytes()
            && *hoard_token_account.key == Pubkey::new_from_array(hoard.token_account.bytes())
            && hoard_token_account.is_writable
            && !hoard_token_account.is_signer
            && !hoard_token_account.executable,
        ClutchError::MismatchedState,
    )?;

    let hoard_data = hoard_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let claim_ledger_data = claim_ledger_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_token_data = hoard_token_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let accepted = accept_market_liability_founding_v3(
        bound,
        plan,
        MarketLiabilityFoundingPostwriteV3 {
            hoard_account: CollateralId::from_bytes(hoard_account.key.to_bytes()),
            hoard_data: &hoard_data,
            claim_ledger_account: CollateralId::from_bytes(claim_ledger_account.key.to_bytes()),
            claim_ledger_data: &claim_ledger_data,
            hoard_token: RuntimeAccountViewV2 {
                key: CollateralId::from_bytes(hoard_token_account.key.to_bytes()),
                owner_program: CollateralId::from_bytes(hoard_token_account.owner.to_bytes()),
                data: &hoard_token_data,
                is_signer: hoard_token_account.is_signer,
                is_writable: hoard_token_account.is_writable,
                executable: hoard_token_account.executable,
            },
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let receipt_id = market_liability_founding_postwrite_receipt_id_v3(
        accepted.receipt_id(),
        plan.founding_id(),
        bound.market().market,
        bound.realm_bound().realm().realm,
        bound.policy_id(),
        release_id,
        deployment.programdata_account(),
        deployment.deployment_slot(),
        deployment.receipt_id(),
        hoard_account.key,
        plan.hoard_id(),
        hoard_lamports,
        claim_ledger_account.key,
        plan.claim_ledger_id(),
        claim_ledger_lamports,
        hoard_token_account.key,
        accepted.visible_hoard_atoms(),
    )?;
    Ok(AuthenticatedMarketLiabilityFoundingPostwriteV3 {
        accepted,
        deployment,
        hoard_lamports,
        claim_ledger_lamports,
        receipt_id,
    })
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

/// Authenticate only the live immutable General MarketBinding V2 and its
/// stable runtime. Historical 540-byte V1 accounts are not accepted here.
pub(crate) fn authenticate_general_market_v2(
    program_id: &Pubkey,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
) -> Outcome<(MarketBindingV2, MarketRuntimeV3AccountV1)> {
    require_program_account(
        program_id,
        market_binding_account,
        false,
        MARKET_BINDING_ACCOUNT_BYTES_V2,
    )?;
    require_program_account(
        program_id,
        market_runtime_account,
        false,
        MARKET_RUNTIME_ACCOUNT_BYTES,
    )?;
    let binding = MarketBindingV2::decode(&market_binding_account.data.borrow())?;
    let runtime = MarketRuntimeV3AccountV1::decode(&market_runtime_account.data.borrow())?;
    let base = binding.base();
    expect_pda(
        market_binding_account.key,
        seeds::general_v2_market_binding_pda(program_id, &base.market_instance_v2_id.bytes()),
        Some(base.stored_bump),
    )?;
    expect_pda(
        market_runtime_account.key,
        seeds::general_v2_market_runtime_pda(program_id, &market_binding_account.key.to_bytes()),
        Some(runtime.stored_bump),
    )?;
    require(
        base.market.bytes() == market_runtime_account.key.to_bytes()
            && runtime.market_binding.bytes() == market_binding_account.key.to_bytes()
            && runtime.market_instance_v2_id == base.market_instance_v2_id,
        ClutchError::MismatchedState,
    )?;
    Ok((binding, runtime))
}

/// Authenticate only the current Product/Revenue-authorized General
/// MarketBinding V4 and its stable runtime. V1/V2/V3 accounts cannot enter
/// this successor authority.
pub(crate) fn authenticate_general_market_v4(
    program_id: &Pubkey,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
) -> Outcome<(MarketBindingV4, MarketRuntimeV3AccountV1)> {
    require_program_account(
        program_id,
        market_binding_account,
        false,
        MARKET_BINDING_ACCOUNT_BYTES_V4,
    )?;
    require_program_account(
        program_id,
        market_runtime_account,
        false,
        MARKET_RUNTIME_ACCOUNT_BYTES,
    )?;
    let binding = MarketBindingV4::decode(&market_binding_account.data.borrow())?;
    let runtime = MarketRuntimeV3AccountV1::decode(&market_runtime_account.data.borrow())?;
    let base = binding.base().base();
    expect_pda(
        market_binding_account.key,
        seeds::general_v2_market_binding_pda(program_id, &base.market_instance_v2_id.bytes()),
        Some(base.stored_bump),
    )?;
    expect_pda(
        market_runtime_account.key,
        seeds::general_v2_market_runtime_pda(program_id, &market_binding_account.key.to_bytes()),
        Some(runtime.stored_bump),
    )?;
    require(
        base.market.bytes() == market_runtime_account.key.to_bytes()
            && runtime.market_binding.bytes() == market_binding_account.key.to_bytes()
            && runtime.market_instance_v2_id == base.market_instance_v2_id,
        ClutchError::MismatchedState,
    )?;
    Ok((binding, runtime))
}

/// Authenticate the sole current V4 General market, Realm-selected
/// collateral, and immutable Product MarketInstance/Genesis artifacts.
///
/// PriceGrid, EconomicDomain, Feed, and page equality remain with the
/// traversal because this helper does not receive those physical accounts.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_market_collateral_v4(
    program_id: &Pubkey,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    collateral_policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
    market_genesis_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedGeneralMarketCollateralV4> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        collateral_policy_account,
        token_program,
    )?;
    let (market_binding, market_runtime) = authenticate_general_market_v4(
        program_id,
        market_binding_account,
        market_runtime_account,
    )?;
    let base = market_binding.base().base();
    let market_instance = *authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        ContentId::from_bytes(base.market_instance_v2_id.bytes()),
    )?
    .value();
    let market_genesis = *authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        market_genesis_account,
        ContentId::from_bytes(base.market_genesis_profile_v2_id.bytes()),
    )?
    .value();
    require(
        market_instance
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes()
            == base.market_instance_v2_id.bytes()
            && market_runtime.market_instance_v2_id == base.market_instance_v2_id
            && market_instance.market_genesis_profile_id.content_id().bytes()
                == base.market_genesis_profile_v2_id.bytes()
            && market_genesis.realm_id.bytes() == realm.realm().realm.bytes()
            && market_genesis.profile_id.bytes() == realm.realm().profile.bytes()
            && market_genesis.price_measure_policy_id.content_id().bytes()
                == base.price_measure_policy_v1_id.bytes()
            && market_genesis.relation_policy_id.bytes() == base.relation_policy_id.bytes()
            && market_genesis.score_policy_id.bytes() == base.score_policy_id.bytes()
            && market_genesis.capability_profile_id.bytes() == capabilities::PROFILE_ID,
        ClutchError::MismatchedState,
    )?;

    let market_bytes = base.market_instance_v2_id.bytes();
    let collateral = refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market_bytes),
            realm: CollateralId::from_bytes(realm.realm().realm.bytes()),
            profile: CollateralId::from_bytes(realm.realm().profile.bytes()),
            collateral_cap_atoms: market_instance.collateral_cap,
            hoard_authority: CollateralId::from_bytes(
                seeds::hoard_authority_v2_pda(program_id, &market_bytes)
                    .0
                    .to_bytes(),
            ),
            hoard_token_account: CollateralId::from_bytes(
                seeds::hoard_token_v2_pda(program_id, &market_bytes)
                    .0
                    .to_bytes(),
            ),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(AuthenticatedGeneralMarketCollateralV4 {
        collateral,
        market_binding,
        market_runtime,
        market_genesis,
    })
}

/// Authenticate current V4/Runtime bodies and derive their sole canonical
/// account-key-plus-full-body identities for downstream family founders.
pub(crate) fn authenticate_general_market_v4_with_data_ids(
    program_id: &Pubkey,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedGeneralMarketV4> {
    let (binding, runtime) =
        authenticate_general_market_v4(program_id, market_binding_account, market_runtime_account)?;
    let binding_data = market_binding_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let binding_data_id = authenticated_account_data_id_v1(
        GENERAL_MARKET_BINDING_DATA_DOMAIN_V4,
        market_binding_account.key,
        &binding_data,
    )?;
    drop(binding_data);
    let runtime_data = market_runtime_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let runtime_data_id = authenticated_account_data_id_v1(
        GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3,
        market_runtime_account.key,
        &runtime_data,
    )?;
    drop(runtime_data);
    Ok(AuthenticatedGeneralMarketV4 {
        binding,
        runtime,
        binding_data_id,
        runtime_data_id,
    })
}

/// Authenticate ProfileV2 collateral, the full Product MarketInstance, and
/// the replacement Hoard/ClaimLedger accounts without consulting any lowered
/// legacy Market, Kernel, or SupplyLedger identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_market_liabilities_v2(
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
) -> Outcome<GeneralMarketLiabilityAuthorityV2> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let (market_binding, market_runtime) =
        authenticate_general_market_v2(program_id, market_binding_account, market_runtime_account)?;
    let relation_market = market_binding.base();
    let market_instance_artifact = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        relation_market.market_instance_v2_id.content_id(),
    )?;
    let market_instance = *market_instance_artifact.value();
    let market_instance_id = market_instance
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        market_instance_id.bytes() == relation_market.market_instance_v2_id.bytes()
            && market_instance.market_genesis_profile_id.bytes()
                == relation_market.market_genesis_profile_v2_id.bytes(),
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
    let market_bytes = relation_market.market_instance_v2_id.bytes();
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
                == CollateralId::from_bytes(relation_market.native_claim_basis_id.bytes())
            && claim_ledger.lifecycle == hoard.lifecycle
            && claim_ledger.outcome_count == hoard.outcome_count
            && claim_ledger.outcome_count == relation_market.outcome_count,
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
    let hoard_semantic_id = hoard
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger_semantic_id = claim_ledger
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_binding_data = market_binding_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let market_binding_data_id = authenticated_account_data_id_v1(
        GENERAL_MARKET_BINDING_DATA_DOMAIN_V2,
        market_binding_account.key,
        &market_binding_data[..],
    )?;
    drop(market_binding_data);
    let market_runtime_data = market_runtime_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let market_runtime_data_id = authenticated_account_data_id_v1(
        GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3,
        market_runtime_account.key,
        &market_runtime_data[..],
    )?;
    drop(market_runtime_data);
    let hoard_lamports = hoard_account.lamports();
    let claim_ledger_lamports = claim_ledger_account.lamports();
    require_deletable_rent_coverage_v1(hoard.rent, hoard_lamports, false)?;
    require_deletable_rent_coverage_v1(claim_ledger.rent, claim_ledger_lamports, false)?;
    let receipt_id = CollateralId::from_bytes(
        solana_sha256_hasher::hashv(&[
            GENERAL_MARKET_LIABILITY_AUTHORITY_DOMAIN_V2,
            market_binding_account.key.as_ref(),
            &market_binding_data_id.bytes(),
            market_runtime_account.key.as_ref(),
            &market_runtime_data_id.bytes(),
            market_instance_account.key.as_ref(),
            &market_instance_id.bytes(),
            hoard_account.key.as_ref(),
            &hoard_semantic_id.bytes(),
            &hoard_lamports.to_le_bytes(),
            claim_ledger_account.key.as_ref(),
            &claim_ledger_semantic_id.bytes(),
            &claim_ledger_lamports.to_le_bytes(),
            &realm.policy_id().bytes(),
            &bound
                .release()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
                .bytes(),
        ])
        .to_bytes(),
    );
    receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(GeneralMarketLiabilityAuthorityV2 {
        bound,
        market_binding,
        market_runtime,
        market_instance,
        hoard,
        claim_ledger,
        market_binding_data_id,
        market_runtime_data_id,
        hoard_semantic_id,
        claim_ledger_semantic_id,
        hoard_lamports,
        claim_ledger_lamports,
        receipt_id,
    })
}

/// Authenticate the full-width liability owners and the exact current
/// Profile-selected collateral ProgramData before any value-bearing CPI.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_market_value_authority_v2(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
    hoard_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
    hoard_writable: bool,
    claim_ledger_writable: bool,
) -> Outcome<GeneralMarketValueAuthorityV2> {
    let liabilities = authenticate_general_market_liabilities_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
        market_binding_account,
        market_runtime_account,
        market_instance_account,
        hoard_account,
        claim_ledger_account,
        hoard_writable,
        claim_ledger_writable,
    )?;
    let deployment = crate::collateral_release::authenticate_collateral_release_deployment_v2(
        liabilities.bound.release(),
        token_program,
        token_programdata,
    )?;
    require(
        deployment.release_id()
            == liabilities
                .bound
                .release()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?,
        ClutchError::AuthorizationUnavailable,
    )?;
    let receipt_id = CollateralId::from_bytes(
        solana_sha256_hasher::hashv(&[
            GENERAL_MARKET_VALUE_AUTHORITY_DOMAIN_V2,
            &liabilities.receipt_id.bytes(),
            &liabilities.market_binding.base().market_instance_v2_id.bytes(),
            &liabilities.bound.policy_id().bytes(),
            &deployment.release_id().bytes(),
            &deployment.programdata_account().bytes(),
            &deployment.deployment_slot().to_le_bytes(),
            &deployment.receipt_id().bytes(),
            &liabilities.hoard_semantic_id.bytes(),
            &liabilities.claim_ledger_semantic_id.bytes(),
        ])
        .to_bytes(),
    );
    receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(GeneralMarketValueAuthorityV2 {
        liabilities,
        deployment,
        receipt_id,
    })
}

/// Authenticate the exact newly finalized Resolution and both liability
/// successor postimages after the private Product/Failure writer has applied
/// one [`MarketResolutionActivationPlanV5`].
///
/// This function is deliberately not an authorization to write. It accepts
/// only the private prewrite liability authority and the pure activation plan,
/// then hostile-decodes all three writable accounts and binds their exact
/// semantic identities into a private receipt.
pub(crate) fn authenticate_market_resolution_activation_postwrite_v5(
    program_id: &Pubkey,
    liabilities: GeneralMarketLiabilityAuthorityV2,
    plan: MarketResolutionActivationPlanV5,
    resolution_account: &AccountInfo<'_>,
    hoard_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedMarketResolutionActivationPostwriteV5> {
    require_program_account(program_id, resolution_account, true, RESOLUTION_V5_BYTES)?;
    require_program_account(program_id, hoard_account, true, HOARD_V2_BYTES)?;
    require_program_account(
        program_id,
        claim_ledger_account,
        true,
        CLAIM_LEDGER_V3_BYTES,
    )?;
    let relation_market = liabilities.market_binding.base();
    let market_bytes = relation_market.market_instance_v2_id.bytes();
    let expected_resolution_account = CollateralId::from_bytes(resolution_account.key.to_bytes());
    require(
        plan.resolution_account() == expected_resolution_account
            && plan.hoard_before_id() == liabilities.hoard_semantic_id
            && plan.claim_ledger_before_id() == liabilities.claim_ledger_semantic_id,
        ClutchError::MismatchedState,
    )?;
    let resolution = ResolutionV5::decode(&resolution_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        resolution_account.key,
        seeds::resolution_v5_pda(program_id, &market_bytes),
        Some(resolution.stored_bump),
    )?;
    expect_pda(
        hoard_account.key,
        seeds::hoard_v2_pda(program_id, &market_bytes),
        Some(plan.hoard_after().stored_bump),
    )?;
    expect_pda(
        claim_ledger_account.key,
        seeds::claim_ledger_v3_pda(program_id, &market_bytes),
        Some(plan.claim_ledger_after().stored_bump),
    )?;

    let hoard = HoardV2::decode(&hoard_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger = ClaimLedgerV3::decode(&claim_ledger_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let resolution_semantic_id = resolution
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let resolution_data_id = resolution
        .data_id(expected_resolution_account)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let hoard_semantic_id = hoard
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger_semantic_id = claim_ledger
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let resolution_lamports = resolution_account.lamports();
    let hoard_lamports = hoard_account.lamports();
    let claim_ledger_lamports = claim_ledger_account.lamports();
    require_deletable_rent_coverage_v1(resolution.rent, resolution_lamports, true)?;
    require_deletable_rent_coverage_v1(hoard.rent, hoard_lamports, false)?;
    require_deletable_rent_coverage_v1(claim_ledger.rent, claim_ledger_lamports, false)?;
    require(
        resolution.state == ResolutionStateV5::Finalized
            && resolution.facts.market_instance_id == CollateralId::from_bytes(market_bytes)
            && resolution.facts.native_claim_basis_id
                == CollateralId::from_bytes(relation_market.native_claim_basis_id.bytes())
            && resolution.facts.outcome_count == relation_market.outcome_count
            && resolution_semantic_id == plan.resolution_id()
            && resolution_data_id == plan.resolution_data_id()
            && hoard == plan.hoard_after()
            && hoard_semantic_id == plan.hoard_after_id()
            && claim_ledger == plan.claim_ledger_after()
            && claim_ledger_semantic_id == plan.claim_ledger_after_id()
            && hoard_lamports == liabilities.hoard_lamports
            && claim_ledger_lamports == liabilities.claim_ledger_lamports,
        ClutchError::MismatchedState,
    )?;
    let receipt_id = CollateralId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_RESOLUTION_ACTIVATION_POSTWRITE_DOMAIN_V5,
            &liabilities.receipt_id.bytes(),
            &plan.receipt_id().bytes(),
            resolution_account.key.as_ref(),
            &resolution_semantic_id.bytes(),
            &resolution_data_id.bytes(),
            &resolution_lamports.to_le_bytes(),
            hoard_account.key.as_ref(),
            &hoard_semantic_id.bytes(),
            &hoard_lamports.to_le_bytes(),
            claim_ledger_account.key.as_ref(),
            &claim_ledger_semantic_id.bytes(),
            &claim_ledger_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(AuthenticatedMarketResolutionActivationPostwriteV5 {
        plan,
        liability_authority_receipt_id: liabilities.receipt_id,
        resolution_lamports,
        hoard_lamports,
        claim_ledger_lamports,
        receipt_id,
    })
}

/// Authenticate the sole V5 payout owner. No legacy Resolution, Market, or
/// Terms body participates in this join.
pub(crate) fn authenticate_resolution_v5(
    program_id: &Pubkey,
    resolution_account: &AccountInfo<'_>,
    liabilities: GeneralMarketLiabilityAuthorityV2,
) -> Outcome<AuthenticatedResolutionV5> {
    require_program_account(program_id, resolution_account, false, RESOLUTION_V5_BYTES)?;
    let resolution = ResolutionV5::decode(&resolution_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let relation_market = liabilities.market_binding.base();
    let market_bytes = relation_market.market_instance_v2_id.bytes();
    expect_pda(
        resolution_account.key,
        seeds::resolution_v5_pda(program_id, &market_bytes),
        Some(resolution.stored_bump),
    )?;
    let account_id = CollateralId::from_bytes(resolution_account.key.to_bytes());
    require_deletable_rent_coverage_v1(resolution.rent, resolution_account.lamports(), false)?;
    require(
        resolution.state == ResolutionStateV5::Finalized
            && resolution.facts.market_instance_id == CollateralId::from_bytes(market_bytes)
            && resolution.facts.native_claim_basis_id
                == CollateralId::from_bytes(
                    relation_market.native_claim_basis_id.bytes(),
                )
            && resolution.facts.outcome_count == relation_market.outcome_count
            && liabilities.hoard.lifecycle
                == clutch_collateral_adapter_v2::MarketLiabilityLifecycleV1::Resolved
            && liabilities.claim_ledger.lifecycle
                == clutch_collateral_adapter_v2::MarketLiabilityLifecycleV1::Resolved
            && liabilities.claim_ledger.resolution_account == account_id,
        ClutchError::MismatchedState,
    )?;
    let semantic_id = resolution
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let data_id = resolution
        .data_id(account_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedResolutionV5 {
        account_id,
        resolution,
        semantic_id,
        data_id,
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

/// Authenticate the common ordinary Position/Replay body only after a caller
/// has authenticated one exact MarketBinding schema and projected its
/// RelationV2 body.
#[allow(clippy::too_many_arguments)]
fn authenticate_general_position_replay_body_v2(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    relation_market: MarketBindingV1,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
    expected_sequence: u64,
    position_writable: bool,
) -> Outcome<GeneralPositionReplayBodyV2> {
    require_program_account(
        program_id,
        position_account,
        position_writable,
        POSITION_V3_BYTES,
    )?;
    require_program_account(
        program_id,
        replay_account,
        true,
        clutch_general_v2_contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES,
    )?;
    require(
        bound.market().market
            == CollateralId::from_bytes(relation_market.market_instance_v2_id.bytes())
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
    let market_identity = Identity32V1::new(relation_market.market_instance_v2_id.bytes())
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
            outcome_count: relation_market.outcome_count,
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
        writable: position_writable,
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
    Ok(GeneralPositionReplayBodyV2 {
        position: authenticated,
        projection,
        replay,
    })
}

/// Authenticate one writable ordinary Position and writable GEN1 Replay.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_position_replay_v2(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
    expected_sequence: u64,
) -> Outcome<GeneralPositionReplayAuthorityV2> {
    let (market_binding, market_runtime) =
        authenticate_general_market_v2(program_id, market_binding_account, market_runtime_account)?;
    let body = authenticate_general_position_replay_body_v2(
        program_id,
        bound,
        market_binding.relation_projection(),
        market_runtime_account,
        position_account,
        replay_account,
        expected_owner,
        expected_sequence,
        true,
    )?;
    Ok(GeneralPositionReplayAuthorityV2 {
        position: body.position,
        projection: body.projection,
        replay: body.replay,
        market_binding,
        market_runtime,
    })
}

/// Authenticate one read-only ordinary Position and writable GEN1 Replay.
///
/// Action 40 advances payment accounting and Replay without mutating the
/// Position. Requiring a writable Position here would add an unnecessary lock
/// and would contradict the pure transition's unchanged-Position obligation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_position_replay_readonly_v2(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
    expected_sequence: u64,
) -> Outcome<GeneralPositionReplayAuthorityV2> {
    let (market_binding, market_runtime) =
        authenticate_general_market_v2(program_id, market_binding_account, market_runtime_account)?;
    let body = authenticate_general_position_replay_body_v2(
        program_id,
        bound,
        market_binding.relation_projection(),
        market_runtime_account,
        position_account,
        replay_account,
        expected_owner,
        expected_sequence,
        false,
    )?;
    Ok(GeneralPositionReplayAuthorityV2 {
        position: body.position,
        projection: body.projection,
        replay: body.replay,
        market_binding,
        market_runtime,
    })
}

/// Authenticate one writable ordinary Position and Replay under the sole
/// current MarketBinding V4 authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_position_replay_v4(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
    expected_sequence: u64,
) -> Outcome<GeneralPositionReplayAuthorityV4> {
    authenticate_general_position_replay_with_access_v4(
        program_id,
        bound,
        market_binding_account,
        market_runtime_account,
        position_account,
        replay_account,
        expected_owner,
        expected_sequence,
        true,
    )
}

/// Authenticate one read-only ordinary Position and writable Replay under
/// the sole current MarketBinding V4 authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_position_replay_readonly_v4(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
    expected_sequence: u64,
) -> Outcome<GeneralPositionReplayAuthorityV4> {
    authenticate_general_position_replay_with_access_v4(
        program_id,
        bound,
        market_binding_account,
        market_runtime_account,
        position_account,
        replay_account,
        expected_owner,
        expected_sequence,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_general_position_replay_with_access_v4(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
    expected_sequence: u64,
    position_writable: bool,
) -> Outcome<GeneralPositionReplayAuthorityV4> {
    let (market_binding, market_runtime) =
        authenticate_general_market_v4(program_id, market_binding_account, market_runtime_account)?;
    let body = authenticate_general_position_replay_body_v2(
        program_id,
        bound,
        market_binding.relation_projection(),
        market_runtime_account,
        position_account,
        replay_account,
        expected_owner,
        expected_sequence,
        position_writable,
    )?;
    Ok(GeneralPositionReplayAuthorityV4 {
        position: body.position,
        projection: body.projection,
        replay: body.replay,
        market_binding,
        market_runtime,
    })
}

#[cfg(test)]
mod rent_coverage_tests {
    use super::*;

    #[test]
    fn deletable_rent_coverage_distinguishes_founding_from_live_surplus() {
        let rent = DeletableRentOwnerV1::from_persisted(
            Identity32V1::new([7; 32]).unwrap(),
            10,
            3,
        )
        .unwrap();
        assert!(require_deletable_rent_coverage_v1(rent, 12, false).is_err());
        assert!(require_deletable_rent_coverage_v1(rent, 13, true).is_ok());
        assert!(require_deletable_rent_coverage_v1(rent, 14, false).is_ok());
        assert!(require_deletable_rent_coverage_v1(rent, 14, true).is_err());
    }

    #[test]
    fn full_binding_and_runtime_data_ids_change_on_one_hostile_byte() {
        let account = Pubkey::new_from_array([9; 32]);
        let binding = [11u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        let mut changed_binding = binding;
        changed_binding[MARKET_BINDING_ACCOUNT_BYTES_V2 - 1] ^= 1;
        let binding_id = authenticated_account_data_id_v1(
            GENERAL_MARKET_BINDING_DATA_DOMAIN_V2,
            &account,
            &binding,
        )
        .unwrap();
        let changed_binding_id = authenticated_account_data_id_v1(
            GENERAL_MARKET_BINDING_DATA_DOMAIN_V2,
            &account,
            &changed_binding,
        )
        .unwrap();
        assert_ne!(binding_id, changed_binding_id);

        let runtime = [13u8; MARKET_RUNTIME_ACCOUNT_BYTES];
        let mut changed_runtime = runtime;
        changed_runtime[MARKET_RUNTIME_ACCOUNT_BYTES - 1] ^= 1;
        let runtime_id = authenticated_account_data_id_v1(
            GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3,
            &account,
            &runtime,
        )
        .unwrap();
        let changed_runtime_id = authenticated_account_data_id_v1(
            GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3,
            &account,
            &changed_runtime,
        )
        .unwrap();
        assert_ne!(runtime_id, changed_runtime_id);
        assert_ne!(binding_id, runtime_id);
    }

    #[test]
    fn founding_receipt_commits_deployment_and_both_exact_rent_balances() {
        let hoard_account = Pubkey::new_from_array([10; 32]);
        let claim_ledger_account = Pubkey::new_from_array([11; 32]);
        let hoard_token_account = Pubkey::new_from_array([12; 32]);
        let receipt = |programdata, deployment_slot, hoard_lamports, claim_ledger_lamports| {
            market_liability_founding_postwrite_receipt_id_v3(
                CollateralId::from_bytes([1; 32]),
                CollateralId::from_bytes([2; 32]),
                CollateralId::from_bytes([3; 32]),
                CollateralId::from_bytes([4; 32]),
                CollateralId::from_bytes([5; 32]),
                CollateralId::from_bytes([6; 32]),
                programdata,
                deployment_slot,
                CollateralId::from_bytes([8; 32]),
                &hoard_account,
                CollateralId::from_bytes([9; 32]),
                hoard_lamports,
                &claim_ledger_account,
                CollateralId::from_bytes([13; 32]),
                claim_ledger_lamports,
                &hoard_token_account,
                0,
            )
            .unwrap()
        };
        let exact = receipt(CollateralId::from_bytes([7; 32]), 19, 23, 29);
        assert_ne!(
            exact,
            receipt(CollateralId::from_bytes([14; 32]), 19, 23, 29)
        );
        assert_ne!(
            exact,
            receipt(CollateralId::from_bytes([7; 32]), 20, 23, 29)
        );
        assert_ne!(
            exact,
            receipt(CollateralId::from_bytes([7; 32]), 19, 24, 29)
        );
        assert_ne!(
            exact,
            receipt(CollateralId::from_bytes([7; 32]), 19, 23, 30)
        );
    }
}
