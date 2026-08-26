//! Exact Hot38 AccountProfile for inline ordinary Direct V3.
//!
//! The profile owns only logical projection and effect authority. Common Hot
//! independently authenticates the injected config, Product graph, portfolio,
//! and ProductBasis records. Claims and Custody remain the sole owners of their
//! account frames and state layouts.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES,
    RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
        ScalarCoordinateV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CAPABILITY_ROOT_MARKET_OFFSET, CAPABILITY_ROOT_RELEASE_SET_OFFSET,
};
use dclutch_claims_svm::{
    frame_spec_v1::{
        ClaimsFrameDataV1, SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1, SparseNativeTransferFrameSpecV1,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketLayoutV2, LiabilityBasisPositionLayoutV2,
    },
};
use dclutch_custody_contract::{
    CustodyFrameDataV1, CustodyFrameSpecV1, CustodyReplayLayoutV1, OperationV1,
    TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_market_core_codec::STATE_BYTES as CORE_STATE_BYTES;
use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3;
use dclutch_product_runtime_v2::{
    DOMAIN_CUT_BYTES, DOMAIN_HEADER_BYTES, PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES,
    PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_realm_contract::{REALM_BYTES, RealmLayoutV1};
use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_rent_contract::RENT_CREDIT_BYTES_V1;

use crate::{
    ordinary_effect_artifacts_v3::{
        DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3, DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3,
        DIRECT_INLINE_FEE_SOLE_ACCOUNT_START_V3, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
        DIRECT_INLINE_SELLER_INTERMEDIATE_ACCOUNT_START_V3,
        DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3,
    },
    ordinary_v3::{
        DIRECT_ORDINARY_COMMON_IDENTITIES_V3, DIRECT_ORDINARY_COMMON_SCALARS_V3,
        DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3, DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
        IDENTITY_BUYER_RENT_BENEFICIARY_OBSERVATION_V3, IDENTITY_BUYER_TOKEN_ACCOUNT_V3,
        IDENTITY_CUSTODY_AUTHORITY_V3, IDENTITY_FEE_RECIPIENT_V3, IDENTITY_FEE_TOKEN_ACCOUNT_V3,
        IDENTITY_LINKED_BASIS_RECORD_V3, IDENTITY_MARKET_V3, IDENTITY_MINT_V3,
        IDENTITY_PRODUCT_RECORD_DIGEST_V3, IDENTITY_REALM_V3, IDENTITY_RELEASE_SET_V3,
        IDENTITY_SELLER_RENT_BENEFICIARY_OBSERVATION_V3, IDENTITY_SELLER_TOKEN_ACCOUNT_V3,
        IDENTITY_SEMANTIC_BASIS_V3, IDENTITY_SYSTEM_PROGRAM_V3, IDENTITY_TOKEN_PROGRAM_V3,
        IDENTITY_TRADING_PROGRAM_V3, SCALAR_BUYER_BUMP_OBSERVATION_V3, SCALAR_BUYER_NEXT_NONCE_V3,
        SCALAR_BUYER_POSITION_REVISION_V3, SCALAR_BUYER_RENT_PRINCIPAL_OBSERVATION_V3,
        SCALAR_CLAIMS_MARKET_REVISION_V3, SCALAR_CUSTODY_REVISION_V3, SCALAR_MARKET_GENERATION_V3,
        SCALAR_OUTCOME_COUNT_V3, SCALAR_POLICY_FEE_BPS_V3, SCALAR_PRICE_SCALE_V3,
        SCALAR_ROOT_OPEN_COUNT_V3, SCALAR_ROOT_PHASE_V3, SCALAR_SELLER_BUMP_OBSERVATION_V3,
        SCALAR_SELLER_NEXT_NONCE_V3, SCALAR_SELLER_POSITION_REVISION_V3,
        SCALAR_SELLER_RENT_PRINCIPAL_OBSERVATION_V3, SCALAR_SLOT_V3,
    },
    state_artifacts_v3::{DIRECT_BUYER_MAKER_ACCOUNT_V3, DIRECT_SELLER_MAKER_ACCOUNT_V3},
    successor::{
        DIRECT_EXECUTION_CONFIG_BYTES_V1, DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_STATE_BYTES_V1,
        DirectExecutionConfigLayoutV1, DirectMakerReplayLayoutV1, DirectRootStateLayoutV1,
    },
};

const FIXED_ACCOUNTS: usize = DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3 as usize;
const FIXED_OPERATIONS: usize = 37;
const CLAIMS_MARKET_ACCOUNT: u16 = DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + 1;
const CLAIMS_SOURCE_POSITION_ACCOUNT: u16 = DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + 20;
const CLAIMS_DESTINATION_POSITION_ACCOUNT: u16 = DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + 21;
const SYSTEM_PROGRAM_ACCOUNT: u16 = 11;
const REALM_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 6;
const CUSTODY_REPLAY_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 8;
const COLLATERAL_MINT_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 9;
const BUYER_TOKEN_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 10;
const SELLER_TOKEN_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 11;
const CUSTODY_AUTHORITY_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 12;
const TOKEN_PROGRAM_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 13;
const FEE_TOKEN_ACCOUNT: u16 = DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3 + 11;
const ROOT_BYTES: usize = CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1;
const BASIS_PREFIX_BYTES: usize = BASIS_WIDTH_OFFSET_V3 + 4;
// Product Runtime V2 defines `outcome_count = cut_count + 2`: the fixed
// domain header already carries the two boundary outcomes, so only `N - 2`
// cuts are affine. Keeping those cuts in the item term would overstate every
// canonical domain by two rows.
const DOMAIN_AFFINE_BASE_BYTES: usize = DOMAIN_HEADER_BYTES - 2 * DOMAIN_CUT_BYTES;
const CLAIMS_ROW_BYTES: usize = 8;

/// Exact encoded fixed-topology Profile13 width for inline ordinary Direct execution.
pub const DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + FIXED_ACCOUNTS * RULE_BYTES
    + FIXED_OPERATIONS * OPERATION_BYTES;

/// Exact account observations used to finalize one release-pinned profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryAccountProfileInputV3<'a> {
    /// Exact logical data lengths in Profile13 coordinate order.
    ///
    /// Runtime-width states are checked for one consistent positive Product
    /// width, while the authenticated ProductBasis record remains variable
    /// within its fixed canonical prefix.
    pub logical_data_lengths: &'a [u32],
}

/// Stable Direct AccountProfile artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectOrdinaryAccountArtifactErrorV3 {
    /// An account/register coordinate or observed width was inconsistent.
    Geometry,
    /// A semantic-owner frame specification refused its coordinate.
    Frame,
    /// The AccountProfile encoder or hostile decoder refused.
    Profile(dclutch_account_profile_contract::v2::Error),
}

/// Emit one complete inline-ordinary fixed-topology Profile13 atomically.
pub fn encode_direct_inline_ordinary_account_profile_v3_atomic(
    input: DirectInlineOrdinaryAccountProfileInputV3<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectOrdinaryAccountArtifactErrorV3> {
    if scratch.len() != DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3
        || output.len() != DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3
    {
        return Err(DirectOrdinaryAccountArtifactErrorV3::Geometry);
    }
    validate_lengths(input.logical_data_lengths)?;
    let rules = rules(input.logical_data_lengths)?;
    let operations = operations()?;
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: scalar(SCALAR_SLOT_V3)?,
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: identity(IDENTITY_TRADING_PROGRAM_V3)?,
        },
        TrustedBuiltinIdentityV2::SystemProgram {
            destination: identity(IDENTITY_SYSTEM_PROGRAM_V3)?,
        },
        &[],
        &rules,
        &[],
        &operations,
        RegisterGeometryV2 {
            common_scalars: scalar(DIRECT_ORDINARY_COMMON_SCALARS_V3)?,
            item_scalar_stride: DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
            common_identities: identity(DIRECT_ORDINARY_COMMON_IDENTITIES_V3)?,
            item_identity_stride: DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3,
        },
        scratch,
        output,
    )
    .map_err(DirectOrdinaryAccountArtifactErrorV3::Profile)?;
    AccountProfileV2::decode(output).map_err(DirectOrdinaryAccountArtifactErrorV3::Profile)?;
    Ok(())
}

fn rules(
    lengths: &[u32],
) -> Result<[AccountRuleWithPrestateInputV2; FIXED_ACCOUNTS], DirectOrdinaryAccountArtifactErrorV3>
{
    let readonly = AccountPrivilegesV2::new(false, false, false);
    let writable = AccountPrivilegesV2::new(false, true, false);
    let signer_writable = AccountPrivilegesV2::new(true, true, false);
    let executable = AccountPrivilegesV2::new(false, false, true);
    let none = AccountEffectPermissionsV2::new(false, false, false);
    let mut output = [exact(readonly, none, 0, 0); FIXED_ACCOUNTS];
    for (rule, data_length) in output.iter_mut().zip(lengths.iter().copied()) {
        rule.rule.data_length = data_length;
    }

    *rule_mut(&mut output, 0)? = exact(
        writable,
        AccountEffectPermissionsV2::new(false, false, true),
        width(ROOT_BYTES)?,
        0,
    );
    rule_mut(&mut output, 1)?.rule.data_length = width(DIRECT_EXECUTION_CONFIG_BYTES_V1)?;
    rule_mut(&mut output, 2)?.rule.data_length = width(PRODUCT_RECORD_BYTES_V2)?;
    let portfolio = rule_mut(&mut output, 3)?;
    portfolio.rule.data_length = width(PORTFOLIO_HEADER_BYTES)?;
    portfolio.rule.data_item_stride = width(PORTFOLIO_COEFFICIENT_BYTES)?;
    *rule_mut(&mut output, 4)? = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: readonly,
            effect_permissions: none,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: width(BASIS_PREFIX_BYTES)?,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AdapterAuthenticatedVariableData,
    };
    for account in [
        DIRECT_SELLER_MAKER_ACCOUNT_V3,
        DIRECT_BUYER_MAKER_ACCOUNT_V3,
    ] {
        *rule_mut(&mut output, usize::from(account))? = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: writable,
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: width(DIRECT_MAKER_REPLAY_BYTES_V1)?,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        };
    }
    for account in [6_usize, 9] {
        *rule_mut(&mut output, account)? = exact(
            signer_writable,
            AccountEffectPermissionsV2::new(true, false, false),
            0,
            0,
        );
    }
    for account in [7_usize, 10] {
        let rule = rule_mut(&mut output, account)?;
        rule.rule.privileges = writable;
        rule.rule.effect_permissions = AccountEffectPermissionsV2::new(false, true, false);
    }
    rule_mut(&mut output, usize::from(SYSTEM_PROGRAM_ACCOUNT))?
        .rule
        .privileges = executable;

    let claims = SparseNativeTransferFrameSpecV1;
    let mut local = 0_u16;
    while local < SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1 {
        let account = claims
            .account(local)
            .map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Frame)?;
        let rule = rule_mut(
            &mut output,
            usize::from(DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + local),
        )?;
        let privileges = claims_privileges(account.privileges());
        rule.rule.privileges = privileges;
        if matches!(
            claims
                .data(local)
                .map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Frame)?,
            ClaimsFrameDataV1::OpaqueData | ClaimsFrameDataV1::ProgramData(_)
        ) {
            *rule = opaque(privileges);
        }
        local += 1;
    }
    let claims_market = rule_mut(&mut output, usize::from(CLAIMS_MARKET_ACCOUNT))?;
    claims_market.rule.data_length = width(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2)?;
    claims_market.rule.data_item_stride = width(CLAIMS_ROW_BYTES)?;
    for account in [
        CLAIMS_SOURCE_POSITION_ACCOUNT,
        CLAIMS_DESTINATION_POSITION_ACCOUNT,
    ] {
        let position = rule_mut(&mut output, usize::from(account))?;
        position.rule.data_length = width(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2)?;
        position.rule.data_item_stride = width(CLAIMS_ROW_BYTES)?;
    }
    let domain = rule_mut(&mut output, 18)?;
    domain.rule.data_length = width(DOMAIN_AFFINE_BASE_BYTES)?;
    domain.rule.data_item_stride = width(DOMAIN_CUT_BYTES)?;

    let custody = CustodyFrameSpecV1::new(OperationV1::Transfer);
    for start in [
        DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3,
        DIRECT_INLINE_SELLER_INTERMEDIATE_ACCOUNT_START_V3,
        DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3,
        DIRECT_INLINE_FEE_SOLE_ACCOUNT_START_V3,
    ] {
        let mut local = 0_u16;
        while local < TRANSFER_ACCOUNT_COUNT_V1 {
            let account = custody
                .account(local)
                .map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Frame)?;
            let rule = rule_mut(&mut output, usize::from(start + local))?;
            let privileges = custody_privileges(account.privileges());
            rule.rule.privileges = privileges;
            if matches!(
                custody
                    .data(local)
                    .map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Frame)?,
                CustodyFrameDataV1::OpaqueData | CustodyFrameDataV1::CallerProgramData
            ) {
                *rule = opaque(privileges);
            }
            local += 1;
        }
    }
    rule_mut(&mut output, usize::from(REALM_ACCOUNT))?
        .rule
        .data_length = width(REALM_BYTES)?;
    rule_mut(&mut output, usize::from(CUSTODY_REPLAY_ACCOUNT))?
        .rule
        .data_length = width(CustodyReplayLayoutV1::BYTES)?;

    for (account, representative) in ROUTE_ALIASES {
        let privileges = rule_at(&output, usize::from(*account))?.rule.privileges;
        *rule_mut(&mut output, usize::from(*account))? = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges,
                effect_permissions: none,
                alias: AccountAliasInputV2::Fixed(*representative),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        };
    }
    Ok(output)
}

fn operations()
-> Result<[AccountOperationInputV2; FIXED_OPERATIONS], DirectOrdinaryAccountArtifactErrorV3> {
    Ok([
        require_owner(0, IDENTITY_TRADING_PROGRAM_V3)?,
        project_identity(
            0,
            CAPABILITY_ROOT_RELEASE_SET_OFFSET,
            IDENTITY_RELEASE_SET_V3,
        )?,
        project_identity(0, CAPABILITY_ROOT_MARKET_OFFSET, IDENTITY_MARKET_V3)?,
        project_u64(
            0,
            CAPABILITY_ROOT_GENERATION_OFFSET,
            SCALAR_MARKET_GENERATION_V3,
        )?,
        project_u8(
            0,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::PHASE,
            SCALAR_ROOT_PHASE_V3,
        )?,
        project_u64(
            0,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT,
            SCALAR_ROOT_OPEN_COUNT_V3,
        )?,
        project_u64(
            1,
            DirectExecutionConfigLayoutV1::PRICE_SCALE,
            SCALAR_PRICE_SCALE_V3,
        )?,
        project_u16(
            1,
            DirectExecutionConfigLayoutV1::FEE_BASIS_POINTS,
            SCALAR_POLICY_FEE_BPS_V3,
        )?,
        project_identity(
            1,
            DirectExecutionConfigLayoutV1::FEE_RECIPIENT,
            IDENTITY_FEE_RECIPIENT_V3,
        )?,
        project_key(2, IDENTITY_PRODUCT_RECORD_DIGEST_V3)?,
        project_identity(
            3,
            PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
            IDENTITY_SEMANTIC_BASIS_V3,
        )?,
        project_key(4, IDENTITY_LINKED_BASIS_RECORD_V3)?,
        AccountOperationInputV2::ProjectTailCountU32 {
            account: fixed(4)?,
            destination: common_scalar(SCALAR_OUTCOME_COUNT_V3)?,
            data_offset: offset(BASIS_WIDTH_OFFSET_V3)?,
        },
        project_u8(
            DIRECT_SELLER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::BUMP,
            SCALAR_SELLER_BUMP_OBSERVATION_V3,
        )?,
        project_u64(
            DIRECT_SELLER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::NEXT_NONCE,
            SCALAR_SELLER_NEXT_NONCE_V3,
        )?,
        project_u64(
            DIRECT_SELLER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::RENT_PRINCIPAL,
            SCALAR_SELLER_RENT_PRINCIPAL_OBSERVATION_V3,
        )?,
        project_identity(
            DIRECT_SELLER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::RENT_OWNER,
            IDENTITY_SELLER_RENT_BENEFICIARY_OBSERVATION_V3,
        )?,
        project_u8(
            DIRECT_BUYER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::BUMP,
            SCALAR_BUYER_BUMP_OBSERVATION_V3,
        )?,
        project_u64(
            DIRECT_BUYER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::NEXT_NONCE,
            SCALAR_BUYER_NEXT_NONCE_V3,
        )?,
        project_u64(
            DIRECT_BUYER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::RENT_PRINCIPAL,
            SCALAR_BUYER_RENT_PRINCIPAL_OBSERVATION_V3,
        )?,
        project_identity(
            DIRECT_BUYER_MAKER_ACCOUNT_V3,
            DirectMakerReplayLayoutV1::RENT_OWNER,
            IDENTITY_BUYER_RENT_BENEFICIARY_OBSERVATION_V3,
        )?,
        require_key(SYSTEM_PROGRAM_ACCOUNT, IDENTITY_SYSTEM_PROGRAM_V3)?,
        require_owner(6, IDENTITY_SYSTEM_PROGRAM_V3)?,
        require_owner(9, IDENTITY_SYSTEM_PROGRAM_V3)?,
        project_u64(
            CLAIMS_MARKET_ACCOUNT,
            LiabilityBasisMarketLayoutV2::REVISION,
            SCALAR_CLAIMS_MARKET_REVISION_V3,
        )?,
        project_u64(
            CLAIMS_SOURCE_POSITION_ACCOUNT,
            LiabilityBasisPositionLayoutV2::REVISION,
            SCALAR_SELLER_POSITION_REVISION_V3,
        )?,
        project_u64(
            CLAIMS_DESTINATION_POSITION_ACCOUNT,
            LiabilityBasisPositionLayoutV2::REVISION,
            SCALAR_BUYER_POSITION_REVISION_V3,
        )?,
        project_key(REALM_ACCOUNT, IDENTITY_REALM_V3)?,
        project_identity(
            REALM_ACCOUNT,
            RealmLayoutV1::COLLATERAL_MINT,
            IDENTITY_MINT_V3,
        )?,
        project_identity(
            REALM_ACCOUNT,
            RealmLayoutV1::TOKEN_PROGRAM,
            IDENTITY_TOKEN_PROGRAM_V3,
        )?,
        project_u64(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::NEXT_REVISION_OFFSET,
            SCALAR_CUSTODY_REVISION_V3,
        )?,
        require_key(COLLATERAL_MINT_ACCOUNT, IDENTITY_MINT_V3)?,
        project_key(BUYER_TOKEN_ACCOUNT, IDENTITY_BUYER_TOKEN_ACCOUNT_V3)?,
        project_key(SELLER_TOKEN_ACCOUNT, IDENTITY_SELLER_TOKEN_ACCOUNT_V3)?,
        project_key(CUSTODY_AUTHORITY_ACCOUNT, IDENTITY_CUSTODY_AUTHORITY_V3)?,
        require_key(TOKEN_PROGRAM_ACCOUNT, IDENTITY_TOKEN_PROGRAM_V3)?,
        project_key(FEE_TOKEN_ACCOUNT, IDENTITY_FEE_TOKEN_ACCOUNT_V3)?,
    ])
}

const ROUTE_ALIASES: &[(u16, u16)] = &[
    (14, 4),
    (16, 2),
    (20, 3),
    (35, 23),
    (36, 24),
    (37, 25),
    (38, 26),
    (39, 27),
    (49, 23),
    (50, 24),
    (51, 25),
    (52, 26),
    (53, 27),
    (54, 40),
    (55, 41),
    (56, 42),
    (57, 43),
    (58, 44),
    (59, 45),
    (60, 46),
    (61, 47),
    (63, 23),
    (64, 24),
    (65, 25),
    (66, 26),
    (67, 27),
    (68, 40),
    (69, 41),
    (70, 42),
    (71, 43),
    (72, 44),
    (74, 46),
    (75, 47),
    (77, 23),
    (78, 24),
    (79, 25),
    (80, 26),
    (81, 27),
    (82, 40),
    (83, 41),
    (84, 42),
    (85, 43),
    (86, 44),
    (87, 73),
    (88, 46),
    (89, 47),
];

fn validate_lengths(lengths: &[u32]) -> Result<(), DirectOrdinaryAccountArtifactErrorV3> {
    let loader_program_bytes = width(LOADER_V3_PROGRAM_BYTES)?;
    if lengths.len() != FIXED_ACCOUNTS {
        return Err(DirectOrdinaryAccountArtifactErrorV3::Geometry);
    }
    if length_at(lengths, 0)? != width(ROOT_BYTES)?
        || length_at(lengths, 1)? != width(DIRECT_EXECUTION_CONFIG_BYTES_V1)?
        || length_at(lengths, 2)? != width(PRODUCT_RECORD_BYTES_V2)?
        || length_at(lengths, 4)? < width(BASIS_PREFIX_BYTES)?
        || length_at(lengths, 5)? != width(DIRECT_MAKER_REPLAY_BYTES_V1)?
        || length_at(lengths, 6)? != 0
        || length_at(lengths, 7)? != width(RENT_CREDIT_BYTES_V1)?
        || length_at(lengths, 8)? != width(DIRECT_MAKER_REPLAY_BYTES_V1)?
        || length_at(lengths, 9)? != 0
        || length_at(lengths, 10)? != width(RENT_CREDIT_BYTES_V1)?
        || length_at(lengths, 11)? != 0
        || length_at(lengths, 23)? != width(CORE_STATE_BYTES)?
        || length_at(lengths, 24)? != width(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?
        || [25_usize, 26, 28, 30]
            .iter()
            .any(|index| length_at(lengths, *index) != Ok(loader_program_bytes))
        || length_at(lengths, 40)? != width(REALM_BYTES)?
        || length_at(lengths, 42)? != width(CustodyReplayLayoutV1::BYTES)?
    {
        return Err(DirectOrdinaryAccountArtifactErrorV3::Geometry);
    }
    let portfolio_count = affine_count(
        length_at(lengths, 3)?,
        PORTFOLIO_HEADER_BYTES,
        PORTFOLIO_COEFFICIENT_BYTES,
    )?;
    let claims_count = affine_count(
        length_at(lengths, usize::from(CLAIMS_MARKET_ACCOUNT))?,
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        CLAIMS_ROW_BYTES,
    )?;
    let domain_count = affine_count(
        length_at(lengths, 18)?,
        DOMAIN_AFFINE_BASE_BYTES,
        DOMAIN_CUT_BYTES,
    )?;
    let source_count = affine_count(
        length_at(lengths, usize::from(CLAIMS_SOURCE_POSITION_ACCOUNT))?,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        CLAIMS_ROW_BYTES,
    )?;
    let destination_count = affine_count(
        length_at(lengths, usize::from(CLAIMS_DESTINATION_POSITION_ACCOUNT))?,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        CLAIMS_ROW_BYTES,
    )?;
    if portfolio_count == 0
        || [claims_count, domain_count, source_count, destination_count]
            .iter()
            .any(|count| *count != portfolio_count)
    {
        return Err(DirectOrdinaryAccountArtifactErrorV3::Geometry);
    }
    for (account, representative) in ROUTE_ALIASES {
        if length_at(lengths, usize::from(*account))?
            != length_at(lengths, usize::from(*representative))?
        {
            return Err(DirectOrdinaryAccountArtifactErrorV3::Geometry);
        }
    }
    Ok(())
}

fn rule_mut(
    rules: &mut [AccountRuleWithPrestateInputV2; FIXED_ACCOUNTS],
    index: usize,
) -> Result<&mut AccountRuleWithPrestateInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    rules
        .get_mut(index)
        .ok_or(DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

fn rule_at(
    rules: &[AccountRuleWithPrestateInputV2; FIXED_ACCOUNTS],
    index: usize,
) -> Result<&AccountRuleWithPrestateInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    rules
        .get(index)
        .ok_or(DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

fn length_at(lengths: &[u32], index: usize) -> Result<u32, DirectOrdinaryAccountArtifactErrorV3> {
    lengths
        .get(index)
        .copied()
        .ok_or(DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

fn affine_count(
    bytes: u32,
    base: usize,
    stride: usize,
) -> Result<u32, DirectOrdinaryAccountArtifactErrorV3> {
    let base = width(base)?;
    let stride = width(stride)?;
    bytes
        .checked_sub(base)
        .filter(|tail| *tail % stride == 0)
        .map(|tail| tail / stride)
        .ok_or(DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

const fn exact(
    privileges: AccountPrivilegesV2,
    effect_permissions: AccountEffectPermissionsV2,
    data_length: u32,
    data_item_stride: u32,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate: AccountPrestateV2::Exact,
    }
}

const fn opaque(privileges: AccountPrivilegesV2) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    }
}

fn claims_privileges(
    value: dclutch_claims_svm::frame_spec_v1::FramePrivilegesV1,
) -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(value.signer(), value.writable(), value.executable())
}

fn custody_privileges(
    value: dclutch_custody_contract::CustodyFramePrivilegesV1,
) -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(value.signer(), value.writable(), value.executable())
}

fn require_key(
    account: u16,
    expected: usize,
) -> Result<AccountOperationInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(AccountOperationInputV2::RequireKey {
        account: fixed(account)?,
        expected: common_identity(expected)?,
    })
}

fn require_owner(
    account: u16,
    expected: usize,
) -> Result<AccountOperationInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(AccountOperationInputV2::RequireOwner {
        account: fixed(account)?,
        expected: common_identity(expected)?,
    })
}

fn project_key(
    account: u16,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(AccountOperationInputV2::ProjectKey {
        account: fixed(account)?,
        destination: common_identity(destination)?,
    })
}

fn project_u8(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(AccountOperationInputV2::ProjectDataU8 {
        account: fixed(account)?,
        destination: common_scalar(destination)?,
        data_offset: offset(data_offset)?,
    })
}

fn project_u16(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(AccountOperationInputV2::ProjectDataU16 {
        account: fixed(account)?,
        destination: common_scalar(destination)?,
        data_offset: offset(data_offset)?,
    })
}

fn project_u64(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(AccountOperationInputV2::ProjectDataU64 {
        account: fixed(account)?,
        destination: common_scalar(destination)?,
        data_offset: offset(data_offset)?,
    })
}

fn project_identity(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: fixed(account)?,
        destination: common_identity(destination)?,
        data_offset: offset(data_offset)?,
    })
}

fn fixed(value: u16) -> Result<AccountCoordinateV2, DirectOrdinaryAccountArtifactErrorV3> {
    if usize::from(value) >= FIXED_ACCOUNTS {
        return Err(DirectOrdinaryAccountArtifactErrorV3::Geometry);
    }
    Ok(AccountCoordinateV2::fixed(value))
}

fn common_scalar(value: usize) -> Result<ScalarCoordinateV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(ScalarCoordinateV2::common(scalar(value)?))
}

fn common_identity(
    value: usize,
) -> Result<IdentityCoordinateV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(IdentityCoordinateV2::common(identity(value)?))
}

fn scalar(value: usize) -> Result<u16, DirectOrdinaryAccountArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

fn identity(value: usize) -> Result<u16, DirectOrdinaryAccountArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

fn offset(value: usize) -> Result<u32, DirectOrdinaryAccountArtifactErrorV3> {
    u32::try_from(value).map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

fn width(value: usize) -> Result<u32, DirectOrdinaryAccountArtifactErrorV3> {
    u32::try_from(value).map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Geometry)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;
    use dclutch_account_profile_contract::{
        EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
        EFFECT_PERMISSION_WRITE_DATA,
    };

    fn lengths(basis_bytes: u32) -> [u32; FIXED_ACCOUNTS] {
        let mut output = [0_u32; FIXED_ACCOUNTS];
        output[0] = width(ROOT_BYTES).expect("root");
        output[1] = width(DIRECT_EXECUTION_CONFIG_BYTES_V1).expect("config");
        output[2] = width(PRODUCT_RECORD_BYTES_V2).expect("product");
        output[3] =
            width(PORTFOLIO_HEADER_BYTES + 3 * PORTFOLIO_COEFFICIENT_BYTES).expect("portfolio");
        output[4] = basis_bytes;
        output[5] = width(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker");
        output[7] = width(RENT_CREDIT_BYTES_V1).expect("seller RentCredit");
        output[8] = width(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker");
        output[10] = width(RENT_CREDIT_BYTES_V1).expect("buyer RentCredit");
        output[13] =
            width(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 3 * CLAIMS_ROW_BYTES).expect("market");
        output[14] = basis_bytes;
        output[16] = output[2];
        output[18] = width(DOMAIN_AFFINE_BASE_BYTES + 3 * DOMAIN_CUT_BYTES).expect("domain");
        output[20] = output[3];
        output[22] = 17;
        output[23] = width(CORE_STATE_BYTES).expect("Core");
        output[24] = width(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation");
        output[25] = width(LOADER_V3_PROGRAM_BYTES).expect("Registry program");
        output[26] = width(LOADER_V3_PROGRAM_BYTES).expect("Trading program");
        output[27] = 1_024;
        output[28] = width(LOADER_V3_PROGRAM_BYTES).expect("Claims program");
        output[29] = 1_024;
        output[30] = width(LOADER_V3_PROGRAM_BYTES).expect("Core program");
        output[31] = 1_024;
        output[32] = width(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 3 * CLAIMS_ROW_BYTES)
            .expect("position");
        output[33] = output[32];
        output[34] = 0;
        output[35] = output[23];
        output[36] = output[24];
        output[37] = output[25];
        output[38] = output[26];
        output[39] = output[27];
        output[40] = width(REALM_BYTES).expect("Realm");
        output[41] = 0;
        output[42] = width(CustodyReplayLayoutV1::BYTES).expect("replay");
        output[43] = 82;
        output[44] = 165;
        output[45] = 165;
        output[46] = 0;
        output[47] = 36;
        output[73] = 165;
        for (account, representative) in ROUTE_ALIASES {
            let value = *output
                .get(usize::from(*representative))
                .expect("representative");
            *output.get_mut(usize::from(*account)).expect("alias") = value;
        }
        for caller in [48_usize, 62, 76] {
            *output.get_mut(caller).expect("caller") = 0;
        }
        output
    }

    fn emit(lengths: &[u32]) -> [u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3] {
        let mut scratch = [0_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
        let mut output = [0x55_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
        encode_direct_inline_ordinary_account_profile_v3_atomic(
            DirectInlineOrdinaryAccountProfileInputV3 {
                logical_data_lengths: lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    #[test]
    fn profile13_round_trips_exact_hot_and_child_geometry() {
        let bytes = emit(&lengths(256));
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        assert_eq!(profile.fixed_account_count(), 90);
        assert_eq!(profile.item_account_stride(), 0);
        assert_eq!(profile.dynamic_fixed_span_count(), 0);
        assert_eq!(profile.common_scalar_count(), 65);
        assert_eq!(profile.item_scalar_stride(), 2);
        assert_eq!(profile.common_identity_count(), 32);
        assert_eq!(profile.item_identity_stride(), 0);
        assert_eq!(profile.trusted_current_slot_scalar(), Some(1));
        assert_eq!(
            profile.trusted_current_executing_program_identity(),
            Some(14)
        );
        assert_eq!(profile.trusted_system_program_identity(), Some(23));
        assert_eq!(profile.representative(3, 14), Ok(4));
        assert_eq!(profile.representative(3, 87), Ok(73));
        assert_eq!(
            profile.rule(false, 4).expect("basis").prestate(),
            AccountPrestateV2::AdapterAuthenticatedVariableData
        );
        assert_eq!(
            profile.rule(false, 14).expect("basis alias").prestate(),
            AccountPrestateV2::AuthenticatedRouteAlias
        );
        let root = profile.rule(false, 0).expect("root");
        assert_eq!(root.effect_permissions(), EFFECT_PERMISSION_WRITE_DATA);
        let maker = profile.rule(false, 5).expect("maker");
        assert_eq!(
            maker.effect_permissions(),
            EFFECT_PERMISSION_CREDIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA
        );
        assert_eq!(maker.prestate(), AccountPrestateV2::LifecycleBound);
        for coordinate in [12_u16, 27, 29, 31, 34, 46, 48, 62, 76] {
            assert_eq!(
                profile
                    .rule(false, coordinate)
                    .expect("opaque route")
                    .prestate(),
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData
            );
        }
        assert_eq!(
            profile
                .rule(false, 39)
                .expect("ProgramData alias")
                .prestate(),
            AccountPrestateV2::AuthenticatedRouteAlias
        );
        assert_eq!(
            profile.rule(false, 6).expect("payer").effect_permissions(),
            EFFECT_PERMISSION_DEBIT_LAMPORTS
        );
    }

    #[test]
    fn one_profile_is_polymorphic_across_categorical_and_graded_basis_bodies() {
        assert_eq!(emit(&lengths(256)), emit(&lengths(736)));
    }

    #[test]
    fn checked_release_programdata_and_authority_widths_do_not_change_profile_identity() {
        let baseline = lengths(256);
        let mut real_deployment = baseline;
        real_deployment[12] = 91;
        real_deployment[27] = 1_141_117;
        for coordinate in [39_usize, 53, 67, 81] {
            let value = *real_deployment.get(27).expect("ProgramData");
            *real_deployment
                .get_mut(coordinate)
                .expect("ProgramData alias") = value;
        }
        real_deployment[34] = 17;
        real_deployment[46] = 29;
        for coordinate in [60_usize, 74, 88] {
            let value = *real_deployment.get(46).expect("Custody authority");
            *real_deployment
                .get_mut(coordinate)
                .expect("Custody authority alias") = value;
        }
        real_deployment[48] = 31;
        real_deployment[62] = 37;
        real_deployment[76] = 41;
        assert_eq!(emit(&baseline), emit(&real_deployment));
    }

    #[test]
    fn route_alias_and_product_width_substitution_refuse_atomically() {
        let mut hostile = lengths(256);
        hostile[87] = hostile[73] + 1;
        let mut scratch = [0_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
        let mut output = [0x5a_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
        assert_eq!(
            encode_direct_inline_ordinary_account_profile_v3_atomic(
                DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &hostile,
                },
                &mut scratch,
                &mut output,
            ),
            Err(DirectOrdinaryAccountArtifactErrorV3::Geometry)
        );
        assert_eq!(
            output,
            [0x5a; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3]
        );

        let mut hostile = lengths(256);
        *hostile.get_mut(33).expect("Claims destination") +=
            u32::try_from(CLAIMS_ROW_BYTES).expect("Claims row width");
        assert_eq!(
            encode_direct_inline_ordinary_account_profile_v3_atomic(
                DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &hostile,
                },
                &mut scratch,
                &mut output,
            ),
            Err(DirectOrdinaryAccountArtifactErrorV3::Geometry)
        );
        assert_eq!(
            output,
            [0x5a; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3]
        );
    }

    #[test]
    fn legacy_64_byte_rent_credit_geometry_refuses_atomically() {
        let mut hostile = lengths(256);
        hostile[7] = 64;
        hostile[10] = 64;
        let mut scratch = [0_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
        let mut output = [0x5a_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
        let before = output;
        assert_eq!(
            encode_direct_inline_ordinary_account_profile_v3_atomic(
                DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &hostile,
                },
                &mut scratch,
                &mut output,
            ),
            Err(DirectOrdinaryAccountArtifactErrorV3::Geometry)
        );
        assert_eq!(output, before);
    }

    #[test]
    fn wrong_output_width_preserves_output() {
        let lengths = lengths(256);
        let mut scratch = vec![0_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
        let mut output = vec![0x33_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3 - 1];
        assert_eq!(
            encode_direct_inline_ordinary_account_profile_v3_atomic(
                DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &lengths,
                },
                &mut scratch,
                &mut output,
            ),
            Err(DirectOrdinaryAccountArtifactErrorV3::Geometry)
        );
        assert!(output.iter().all(|byte| *byte == 0x33));
    }
}
