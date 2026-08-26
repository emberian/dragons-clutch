//! Fixed-topology Profile14 for registered Buy creation.
//!
//! The profile authenticates the Direct root, maker replay, registered record,
//! and the exact ordered Custody InitializeReplay/OpenVault/deposit frames.  It
//! carries no token parser: Realm and routed token identities are projected,
//! while Custody remains the sole owner of token, delegate, and allowance
//! validation.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, AccountProfileV2, FIXED_DATA_PREDICATE_BYTES,
    FIXED_DATA_PREDICATE_HEADER_BYTES, OPERATION_BYTES, RULE_BYTES, TrustedBuiltinIdentityV2,
    TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, FixedDataPredicateInputV2, IdentityCoordinateV2,
        RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_with_fixed_data_predicates_v2_atomic,
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CAPABILITY_ROOT_MARKET_OFFSET, CAPABILITY_ROOT_RELEASE_SET_OFFSET,
};
use dclutch_custody_contract::{CustodyFrameDataV1, CustodyFrameSpecV1, OperationV1};
use dclutch_market_core_codec::STATE_BYTES as CORE_STATE_BYTES;
use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3;
use dclutch_product_runtime_v2::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES, PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_realm_contract::{REALM_BYTES, RealmLayoutV1};
use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_rent_contract::RENT_CREDIT_BYTES_V1;

use crate::{
    registered_creation_artifacts_v4::{
        DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4,
        DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4,
        DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4,
        DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4, REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4,
        REGISTERED_IDENTITY_CUSTODY_AUTHORITY_V4, REGISTERED_IDENTITY_CUSTODY_VAULT_V4,
        REGISTERED_IDENTITY_LINKED_BASIS_V4, REGISTERED_IDENTITY_MAKER_BENEFICIARY_OBSERVATION_V4,
        REGISTERED_IDENTITY_MARKET_V4, REGISTERED_IDENTITY_MINT_V4, REGISTERED_IDENTITY_PAYER_V4,
        REGISTERED_IDENTITY_PRODUCT_RECORD_V4, REGISTERED_IDENTITY_REALM_V4,
        REGISTERED_IDENTITY_RECORD_BENEFICIARY_OBSERVATION_V4, REGISTERED_IDENTITY_RELEASE_SET_V4,
        REGISTERED_IDENTITY_SEMANTIC_BASIS_V4, REGISTERED_IDENTITY_SYSTEM_PROGRAM_V4,
        REGISTERED_IDENTITY_TOKEN_PROGRAM_V4, REGISTERED_IDENTITY_TRADING_PROGRAM_V4,
        REGISTERED_SCALAR_MAKER_BUMP_OBSERVATION_V4, REGISTERED_SCALAR_MAKER_LIVE_COUNT_V4,
        REGISTERED_SCALAR_MAKER_PRINCIPAL_OBSERVATION_V4, REGISTERED_SCALAR_MARKET_GENERATION_V4,
        REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_V4, REGISTERED_SCALAR_NEXT_NONCE_V4,
        REGISTERED_SCALAR_OUTCOME_COUNT_V4, REGISTERED_SCALAR_POLICY_FEE_BPS_V4,
        REGISTERED_SCALAR_PRICE_SCALE_V4, REGISTERED_SCALAR_RECORD_BUMP_OBSERVATION_V4,
        REGISTERED_SCALAR_RECORD_PRINCIPAL_OBSERVATION_V4, REGISTERED_SCALAR_ROOT_OPEN_COUNT_V4,
        REGISTERED_SCALAR_ROOT_PHASE_V4, REGISTERED_SCALAR_SLOT_V4,
    },
    registered_state_artifacts_v4::{
        DIRECT_REGISTERED_MAKER_ACCOUNT_V4, DIRECT_REGISTERED_MAKER_RENT_CREDIT_ACCOUNT_V4,
        DIRECT_REGISTERED_PAYER_ACCOUNT_V4, DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
        DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4, DIRECT_REGISTERED_RECORD_RENT_CREDIT_ACCOUNT_V4,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_BYTES_V1, DIRECT_MAKER_REPLAY_BYTES_V1,
        DIRECT_REGISTERED_RECORD_BYTES_V2, DIRECT_ROOT_STATE_BYTES_V1,
        DirectExecutionConfigLayoutV1, DirectMakerReplayLayoutV1, DirectRegisteredRecordLayoutV2,
        DirectRootStateLayoutV1,
    },
};

/// First logical account of the Custody InitializeReplay frame.
pub const DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4: u16 = 12;
/// First logical account of the Custody OpenVault frame.
pub const DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4: u16 = 24;
/// First logical account of the delegated reserve-deposit frame.
pub const DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4: u16 = 40;
/// Exact fixed logical account count for registered Buy creation.
pub const DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4: u16 = 54;

const FIXED_ACCOUNTS: usize = DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4 as usize;
const FIXED_OPERATIONS: usize = 33;
const FIXED_DATA_PREDICATES: usize = 9;
const SYSTEM_PROGRAM_ACCOUNT: u16 = 11;
const REALM_ACCOUNT: u16 = 18;
const REPLAY_ACCOUNT: u16 = 20;
const MINT_ACCOUNT: u16 = 33;
const VAULT_ACCOUNT: u16 = 34;
const CUSTODY_AUTHORITY_ACCOUNT: u16 = 35;
const TOKEN_PROGRAM_ACCOUNT: u16 = 36;
const SOURCE_ACCOUNT: u16 = 50;
const ROOT_BYTES: usize = CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1;
const BASIS_PREFIX_BYTES: usize = BASIS_WIDTH_OFFSET_V3 + 4;

/// Exact encoded Profile14 width for RegisterBuy.
pub const DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4: usize = FIXED_DATA_PREDICATE_HEADER_BYTES
    + FIXED_DATA_PREDICATES * FIXED_DATA_PREDICATE_BYTES
    + FIXED_ACCOUNTS * RULE_BYTES
    + FIXED_OPERATIONS * OPERATION_BYTES;

/// Exact chain-observed logical data widths used to finalize the profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisterBuyAccountProfileInputV4<'a> {
    /// Exact logical widths in the declared Profile14 coordinate order.
    pub logical_data_lengths: &'a [u32],
}

/// Stable registered AccountProfile refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredAccountArtifactErrorV4 {
    /// One fixed coordinate, register, or account width was inconsistent.
    Geometry,
    /// A semantic-owner Custody frame refused its coordinate.
    Frame,
    /// The Profile14 encoder or hostile decoder refused.
    Profile(dclutch_account_profile_contract::v2::Error),
}

/// Emit the exact registered Buy AccountProfile atomically.
pub fn encode_direct_register_buy_account_profile_v4_atomic(
    input: DirectRegisterBuyAccountProfileInputV4<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredAccountArtifactErrorV4> {
    if scratch.len() != DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4
        || output.len() != DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4
    {
        return Err(DirectRegisteredAccountArtifactErrorV4::Geometry);
    }
    validate_lengths(input.logical_data_lengths)?;
    let rules = rules(input.logical_data_lengths)?;
    let predicates = fixed_data_predicates()?;
    let operations = operations()?;
    encode_account_profile_with_fixed_data_predicates_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: scalar(REGISTERED_SCALAR_SLOT_V4)?,
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: identity(REGISTERED_IDENTITY_TRADING_PROGRAM_V4)?,
        },
        TrustedBuiltinIdentityV2::SystemProgram {
            destination: identity(REGISTERED_IDENTITY_SYSTEM_PROGRAM_V4)?,
        },
        &[],
        &predicates,
        &rules,
        &[],
        &operations,
        RegisterGeometryV2 {
            common_scalars: scalar(DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4)?,
            item_scalar_stride: DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4,
            common_identities: identity(DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4)?,
            item_identity_stride: DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4,
        },
        scratch,
        output,
    )
    .map_err(DirectRegisteredAccountArtifactErrorV4::Profile)?;
    AccountProfileV2::decode(output).map_err(DirectRegisteredAccountArtifactErrorV4::Profile)?;
    Ok(())
}

fn fixed_data_predicates() -> Result<
    [FixedDataPredicateInputV2; FIXED_DATA_PREDICATES],
    DirectRegisteredAccountArtifactErrorV4,
> {
    let root = |state_offset: usize| {
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(state_offset)
            .ok_or(DirectRegisteredAccountArtifactErrorV4::Geometry)
            .and_then(offset)
    };
    Ok([
        FixedDataPredicateInputV2::RequireDataU64 {
            account: 0,
            data_offset: root(DirectRootStateLayoutV1::MAGIC)?,
            value: DirectRootStateLayoutV1::MAGIC_WORD,
        },
        FixedDataPredicateInputV2::RequireDataU16 {
            account: 0,
            data_offset: root(DirectRootStateLayoutV1::VERSION)?,
            value: DirectRootStateLayoutV1::ABI_VERSION,
        },
        FixedDataPredicateInputV2::RequireZeroRange {
            account: 0,
            data_offset: root(DirectRootStateLayoutV1::RESERVED)?,
            length: width(DirectRootStateLayoutV1::RESERVED_BYTES)?,
        },
        state_magic(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::MAGIC,
            DirectMakerReplayLayoutV1::MAGIC_WORD,
        )?,
        state_version(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::VERSION,
            DirectMakerReplayLayoutV1::ABI_VERSION,
        )?,
        state_reserved(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::RESERVED,
            DirectMakerReplayLayoutV1::RESERVED_BYTES,
        )?,
        state_magic(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            DirectRegisteredRecordLayoutV2::MAGIC,
            DirectRegisteredRecordLayoutV2::MAGIC_WORD,
        )?,
        state_version(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            DirectRegisteredRecordLayoutV2::VERSION,
            DirectRegisteredRecordLayoutV2::ABI_VERSION,
        )?,
        state_reserved(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            DirectRegisteredRecordLayoutV2::RESERVED,
            DirectRegisteredRecordLayoutV2::RESERVED_BYTES,
        )?,
    ])
}

fn rules(
    lengths: &[u32],
) -> Result<[AccountRuleWithPrestateInputV2; FIXED_ACCOUNTS], DirectRegisteredAccountArtifactErrorV4>
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
    for (account, bytes) in [
        (
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DIRECT_MAKER_REPLAY_BYTES_V1,
        ),
        (
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            DIRECT_REGISTERED_RECORD_BYTES_V2,
        ),
    ] {
        *rule_mut(&mut output, usize::from(account))? = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: writable,
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: width(bytes)?,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        };
    }
    for account in [
        DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
        DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4,
    ] {
        *rule_mut(&mut output, usize::from(account))? = exact(
            signer_writable,
            AccountEffectPermissionsV2::new(true, false, false),
            0,
            0,
        );
    }
    for account in [
        DIRECT_REGISTERED_MAKER_RENT_CREDIT_ACCOUNT_V4,
        DIRECT_REGISTERED_RECORD_RENT_CREDIT_ACCOUNT_V4,
    ] {
        let rule = rule_mut(&mut output, usize::from(account))?;
        rule.rule.privileges = writable;
        rule.rule.effect_permissions = AccountEffectPermissionsV2::new(false, true, false);
    }
    rule_mut(&mut output, usize::from(SYSTEM_PROGRAM_ACCOUNT))?
        .rule
        .privileges = executable;

    for (operation, start) in [
        (
            OperationV1::InitializeReplay,
            DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4,
        ),
        (
            OperationV1::OpenVault,
            DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4,
        ),
        (
            OperationV1::Transfer,
            DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4,
        ),
    ] {
        let frame = CustodyFrameSpecV1::new(operation);
        let mut local = 0_u16;
        while local < frame.account_count() {
            let account = frame
                .account(local)
                .map_err(|_| DirectRegisteredAccountArtifactErrorV4::Frame)?;
            let privileges = custody_privileges(account.privileges());
            let rule = rule_mut(&mut output, usize::from(start + local))?;
            rule.rule.privileges = privileges;
            if matches!(
                frame
                    .data(local)
                    .map_err(|_| DirectRegisteredAccountArtifactErrorV4::Frame)?,
                CustodyFrameDataV1::OpaqueData | CustodyFrameDataV1::CallerProgramData
            ) {
                *rule = opaque(privileges);
            }
            local = local
                .checked_add(1)
                .ok_or(DirectRegisteredAccountArtifactErrorV4::Geometry)?;
        }
    }
    // An authenticated route alias is a privilege-free logical view: the
    // representative coordinate is the single semantic owner of the route's
    // physical privileges.  The equality check keeps that fact observed rather
    // than silently discarded when the alias is rewritten.
    for (account, representative) in ROUTE_ALIASES {
        let privileges = rule_at(&output, usize::from(*representative))?
            .rule
            .privileges;
        if rule_at(&output, usize::from(*account))?.rule.privileges != privileges {
            return Err(DirectRegisteredAccountArtifactErrorV4::Geometry);
        }
        *rule_mut(&mut output, usize::from(*account))? = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: readonly,
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
-> Result<[AccountOperationInputV2; FIXED_OPERATIONS], DirectRegisteredAccountArtifactErrorV4> {
    Ok([
        require_owner(0, REGISTERED_IDENTITY_TRADING_PROGRAM_V4)?,
        project_identity(
            0,
            CAPABILITY_ROOT_RELEASE_SET_OFFSET,
            REGISTERED_IDENTITY_RELEASE_SET_V4,
        )?,
        project_identity(
            0,
            CAPABILITY_ROOT_MARKET_OFFSET,
            REGISTERED_IDENTITY_MARKET_V4,
        )?,
        project_u64(
            0,
            CAPABILITY_ROOT_GENERATION_OFFSET,
            REGISTERED_SCALAR_MARKET_GENERATION_V4,
        )?,
        project_u8(
            0,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::PHASE,
            REGISTERED_SCALAR_ROOT_PHASE_V4,
        )?,
        project_u64(
            0,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT,
            REGISTERED_SCALAR_ROOT_OPEN_COUNT_V4,
        )?,
        project_u64(
            1,
            DirectExecutionConfigLayoutV1::PRICE_SCALE,
            REGISTERED_SCALAR_PRICE_SCALE_V4,
        )?,
        project_u16(
            1,
            DirectExecutionConfigLayoutV1::FEE_BASIS_POINTS,
            REGISTERED_SCALAR_POLICY_FEE_BPS_V4,
        )?,
        project_key(2, REGISTERED_IDENTITY_PRODUCT_RECORD_V4)?,
        project_identity(
            3,
            PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
            REGISTERED_IDENTITY_SEMANTIC_BASIS_V4,
        )?,
        project_key(4, REGISTERED_IDENTITY_LINKED_BASIS_V4)?,
        AccountOperationInputV2::ProjectTailCountU32 {
            account: fixed(4)?,
            destination: common_scalar(REGISTERED_SCALAR_OUTCOME_COUNT_V4)?,
            data_offset: offset(BASIS_WIDTH_OFFSET_V3)?,
        },
        project_u8(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::BUMP,
            REGISTERED_SCALAR_MAKER_BUMP_OBSERVATION_V4,
        )?,
        project_u64(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::NEXT_NONCE,
            REGISTERED_SCALAR_NEXT_NONCE_V4,
        )?,
        project_u64(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::LIVE_COUNT,
            REGISTERED_SCALAR_MAKER_LIVE_COUNT_V4,
        )?,
        project_u64(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::MINIMUM_LIVE_NONCE,
            REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_V4,
        )?,
        project_u64(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::RENT_PRINCIPAL,
            REGISTERED_SCALAR_MAKER_PRINCIPAL_OBSERVATION_V4,
        )?,
        project_identity(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            DirectMakerReplayLayoutV1::RENT_OWNER,
            REGISTERED_IDENTITY_MAKER_BENEFICIARY_OBSERVATION_V4,
        )?,
        project_u8(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            DirectRegisteredRecordLayoutV2::BUMP,
            REGISTERED_SCALAR_RECORD_BUMP_OBSERVATION_V4,
        )?,
        project_u64(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            DirectRegisteredRecordLayoutV2::RENT_PRINCIPAL,
            REGISTERED_SCALAR_RECORD_PRINCIPAL_OBSERVATION_V4,
        )?,
        project_identity(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            DirectRegisteredRecordLayoutV2::RENT_OWNER,
            REGISTERED_IDENTITY_RECORD_BENEFICIARY_OBSERVATION_V4,
        )?,
        require_key(
            SYSTEM_PROGRAM_ACCOUNT,
            REGISTERED_IDENTITY_SYSTEM_PROGRAM_V4,
        )?,
        require_owner(
            DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
            REGISTERED_IDENTITY_SYSTEM_PROGRAM_V4,
        )?,
        require_owner(
            DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4,
            REGISTERED_IDENTITY_SYSTEM_PROGRAM_V4,
        )?,
        project_key(
            DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
            REGISTERED_IDENTITY_PAYER_V4,
        )?,
        project_key(REALM_ACCOUNT, REGISTERED_IDENTITY_REALM_V4)?,
        project_identity(
            REALM_ACCOUNT,
            RealmLayoutV1::COLLATERAL_MINT,
            REGISTERED_IDENTITY_MINT_V4,
        )?,
        project_identity(
            REALM_ACCOUNT,
            RealmLayoutV1::TOKEN_PROGRAM,
            REGISTERED_IDENTITY_TOKEN_PROGRAM_V4,
        )?,
        require_key(MINT_ACCOUNT, REGISTERED_IDENTITY_MINT_V4)?,
        project_key(VAULT_ACCOUNT, REGISTERED_IDENTITY_CUSTODY_VAULT_V4)?,
        project_key(
            CUSTODY_AUTHORITY_ACCOUNT,
            REGISTERED_IDENTITY_CUSTODY_AUTHORITY_V4,
        )?,
        require_key(TOKEN_PROGRAM_ACCOUNT, REGISTERED_IDENTITY_TOKEN_PROGRAM_V4)?,
        project_key(SOURCE_ACCOUNT, REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4)?,
    ])
}

const ROUTE_ALIASES: &[(u16, u16)] = &[
    (22, 11),
    (25, 13),
    (26, 14),
    (27, 15),
    (28, 16),
    (29, 17),
    (30, 18),
    (31, 19),
    (32, 20),
    (38, 11),
    (39, 23),
    (41, 13),
    (42, 14),
    (43, 15),
    (44, 16),
    (45, 17),
    (46, 18),
    (47, 19),
    (48, 20),
    (49, 33),
    (51, 34),
    (52, 35),
    (53, 36),
];

fn validate_lengths(lengths: &[u32]) -> Result<(), DirectRegisteredAccountArtifactErrorV4> {
    if lengths.len() != FIXED_ACCOUNTS
        || length_at(lengths, 0)? != width(ROOT_BYTES)?
        || length_at(lengths, 1)? != width(DIRECT_EXECUTION_CONFIG_BYTES_V1)?
        || length_at(lengths, 2)? != width(PRODUCT_RECORD_BYTES_V2)?
        || length_at(lengths, 4)? < width(BASIS_PREFIX_BYTES)?
        || length_at(lengths, 5)? != width(DIRECT_MAKER_REPLAY_BYTES_V1)?
        || length_at(lengths, 6)? != 0
        || length_at(lengths, 7)? != width(RENT_CREDIT_BYTES_V1)?
        || length_at(lengths, 8)? != width(DIRECT_REGISTERED_RECORD_BYTES_V2)?
        || length_at(lengths, 9)? != 0
        || length_at(lengths, 10)? != width(RENT_CREDIT_BYTES_V1)?
        || length_at(lengths, 11)? != 0
        || length_at(lengths, 13)? != width(CORE_STATE_BYTES)?
        || length_at(lengths, 14)? != width(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?
        || length_at(lengths, 15)? != width(LOADER_V3_PROGRAM_BYTES)?
        || length_at(lengths, 16)? != width(LOADER_V3_PROGRAM_BYTES)?
        || length_at(lengths, 18)? != width(REALM_BYTES)?
        || length_at(lengths, 19)? != 0
        || length_at(lengths, REPLAY_ACCOUNT as usize)? != 0
        || length_at(lengths, VAULT_ACCOUNT as usize)? != 0
        || length_at(lengths, SOURCE_ACCOUNT as usize)? == 0
    {
        return Err(DirectRegisteredAccountArtifactErrorV4::Geometry);
    }
    let portfolio_count = affine_count(
        length_at(lengths, 3)?,
        PORTFOLIO_HEADER_BYTES,
        PORTFOLIO_COEFFICIENT_BYTES,
    )?;
    if portfolio_count == 0 {
        return Err(DirectRegisteredAccountArtifactErrorV4::Geometry);
    }
    for (account, representative) in ROUTE_ALIASES {
        if length_at(lengths, usize::from(*account))?
            != length_at(lengths, usize::from(*representative))?
        {
            return Err(DirectRegisteredAccountArtifactErrorV4::Geometry);
        }
    }
    Ok(())
}

fn state_magic(
    account: u16,
    data_offset: usize,
    value: u64,
) -> Result<FixedDataPredicateInputV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(FixedDataPredicateInputV2::RequireDataU64 {
        account,
        data_offset: offset(data_offset)?,
        value,
    })
}

fn state_version(
    account: u16,
    data_offset: usize,
    value: u16,
) -> Result<FixedDataPredicateInputV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(FixedDataPredicateInputV2::RequireDataU16 {
        account,
        data_offset: offset(data_offset)?,
        value,
    })
}

fn state_reserved(
    account: u16,
    data_offset: usize,
    length: usize,
) -> Result<FixedDataPredicateInputV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(FixedDataPredicateInputV2::RequireZeroRange {
        account,
        data_offset: offset(data_offset)?,
        length: width(length)?,
    })
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

fn custody_privileges(
    value: dclutch_custody_contract::CustodyFramePrivilegesV1,
) -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(value.signer(), value.writable(), value.executable())
}

fn require_key(
    account: u16,
    expected: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(AccountOperationInputV2::RequireKey {
        account: fixed(account)?,
        expected: common_identity(expected)?,
    })
}

fn require_owner(
    account: u16,
    expected: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(AccountOperationInputV2::RequireOwner {
        account: fixed(account)?,
        expected: common_identity(expected)?,
    })
}

fn project_key(
    account: u16,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectKey {
        account: fixed(account)?,
        destination: common_identity(destination)?,
    })
}

fn project_u8(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredAccountArtifactErrorV4> {
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
) -> Result<AccountOperationInputV2, DirectRegisteredAccountArtifactErrorV4> {
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
) -> Result<AccountOperationInputV2, DirectRegisteredAccountArtifactErrorV4> {
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
) -> Result<AccountOperationInputV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: fixed(account)?,
        destination: common_identity(destination)?,
        data_offset: offset(data_offset)?,
    })
}

fn rule_mut(
    rules: &mut [AccountRuleWithPrestateInputV2; FIXED_ACCOUNTS],
    index: usize,
) -> Result<&mut AccountRuleWithPrestateInputV2, DirectRegisteredAccountArtifactErrorV4> {
    rules
        .get_mut(index)
        .ok_or(DirectRegisteredAccountArtifactErrorV4::Geometry)
}

fn rule_at(
    rules: &[AccountRuleWithPrestateInputV2; FIXED_ACCOUNTS],
    index: usize,
) -> Result<&AccountRuleWithPrestateInputV2, DirectRegisteredAccountArtifactErrorV4> {
    rules
        .get(index)
        .ok_or(DirectRegisteredAccountArtifactErrorV4::Geometry)
}

fn length_at(lengths: &[u32], index: usize) -> Result<u32, DirectRegisteredAccountArtifactErrorV4> {
    lengths
        .get(index)
        .copied()
        .ok_or(DirectRegisteredAccountArtifactErrorV4::Geometry)
}

fn affine_count(
    bytes: u32,
    base: usize,
    stride: usize,
) -> Result<u32, DirectRegisteredAccountArtifactErrorV4> {
    let base = width(base)?;
    let stride = width(stride)?;
    bytes
        .checked_sub(base)
        .filter(|tail| *tail % stride == 0)
        .map(|tail| tail / stride)
        .ok_or(DirectRegisteredAccountArtifactErrorV4::Geometry)
}

fn fixed(value: u16) -> Result<AccountCoordinateV2, DirectRegisteredAccountArtifactErrorV4> {
    if usize::from(value) >= FIXED_ACCOUNTS {
        return Err(DirectRegisteredAccountArtifactErrorV4::Geometry);
    }
    Ok(AccountCoordinateV2::fixed(value))
}

fn common_scalar(
    value: usize,
) -> Result<ScalarCoordinateV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(ScalarCoordinateV2::common(scalar(value)?))
}

fn common_identity(
    value: usize,
) -> Result<IdentityCoordinateV2, DirectRegisteredAccountArtifactErrorV4> {
    Ok(IdentityCoordinateV2::common(identity(value)?))
}

fn scalar(value: usize) -> Result<u16, DirectRegisteredAccountArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredAccountArtifactErrorV4::Geometry)
}

fn identity(value: usize) -> Result<u16, DirectRegisteredAccountArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredAccountArtifactErrorV4::Geometry)
}

fn offset(value: usize) -> Result<u32, DirectRegisteredAccountArtifactErrorV4> {
    u32::try_from(value).map_err(|_| DirectRegisteredAccountArtifactErrorV4::Geometry)
}

fn width(value: usize) -> Result<u32, DirectRegisteredAccountArtifactErrorV4> {
    u32::try_from(value).map_err(|_| DirectRegisteredAccountArtifactErrorV4::Geometry)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::registered_state_artifacts_v4::{
        DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5,
        encode_direct_registered_creation_lifecycle_v5_atomic,
    };
    use dclutch_account_profile_contract::{
        lifecycle_v3::StateLifecyclePolicyV5, v2::FixedDataPredicateKindV2,
    };
    use dclutch_custody_contract::{
        INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, OPEN_VAULT_ACCOUNT_COUNT_V1, TRANSFER_ACCOUNT_COUNT_V1,
    };

    fn lengths() -> [u32; FIXED_ACCOUNTS] {
        let mut output = [0_u32; FIXED_ACCOUNTS];
        output[0] = width(ROOT_BYTES).expect("root");
        output[1] = width(DIRECT_EXECUTION_CONFIG_BYTES_V1).expect("config");
        output[2] = width(PRODUCT_RECORD_BYTES_V2).expect("Product");
        output[3] =
            width(PORTFOLIO_HEADER_BYTES + 3 * PORTFOLIO_COEFFICIENT_BYTES).expect("portfolio");
        output[4] = 256;
        output[5] = width(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker");
        output[7] = width(RENT_CREDIT_BYTES_V1).expect("maker RentCredit");
        output[8] = width(DIRECT_REGISTERED_RECORD_BYTES_V2).expect("record");
        output[10] = width(RENT_CREDIT_BYTES_V1).expect("record RentCredit");
        output[13] = width(CORE_STATE_BYTES).expect("Core Market");
        output[14] = width(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation");
        output[15] = width(LOADER_V3_PROGRAM_BYTES).expect("Registry");
        output[16] = width(LOADER_V3_PROGRAM_BYTES).expect("Trading");
        output[17] = 1_024;
        output[18] = width(REALM_BYTES).expect("Realm");
        output[23] = 17;
        output[33] = 82;
        output[35] = 0;
        output[36] = 36;
        output[50] = 165;
        for (account, representative) in ROUTE_ALIASES {
            let value = *output
                .get(usize::from(*representative))
                .expect("representative");
            *output.get_mut(usize::from(*account)).expect("alias") = value;
        }
        output
    }

    fn emit() -> [u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4] {
        let lengths = lengths();
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
        encode_direct_register_buy_account_profile_v4_atomic(
            DirectRegisterBuyAccountProfileInputV4 {
                logical_data_lengths: &lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    #[test]
    fn profile14_round_trips_buy_routes_and_joins_lifecycle_v5() {
        assert_eq!(INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, 12);
        assert_eq!(OPEN_VAULT_ACCOUNT_COUNT_V1, 16);
        assert_eq!(TRANSFER_ACCOUNT_COUNT_V1, 14);
        let bytes = emit();
        let profile = AccountProfileV2::decode(&bytes).expect("profile decode");
        assert!(profile.uses_fixed_data_predicates());
        assert_eq!(profile.fixed_data_predicate_count(), 9);
        assert_eq!(
            profile.fixed_account_count(),
            DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4
        );
        assert!(profile.supports_route_alias_packing());
        assert_eq!(
            profile
                .fixed_data_predicate(8)
                .expect("record reserved")
                .kind(),
            FixedDataPredicateKindV2::RequireZeroRange(5)
        );

        let mut lifecycle_scratch = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        let mut lifecycle = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        encode_direct_registered_creation_lifecycle_v5_atomic(
            crate::execution_v3::DirectExecutionActionV3::RegisterBuy,
            &mut lifecycle_scratch,
            &mut lifecycle,
        )
        .expect("lifecycle");
        StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &lifecycle)
            .expect("lifecycle decode")
            .validate_account_profile(profile)
            .expect("profile/lifecycle join");
    }

    #[test]
    fn malformed_width_or_output_refuses_atomically() {
        let mut lengths = lengths();
        *lengths.get_mut(8).expect("record") = 267;
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = [0x55_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
        let before = output;
        assert_eq!(
            encode_direct_register_buy_account_profile_v4_atomic(
                DirectRegisterBuyAccountProfileInputV4 {
                    logical_data_lengths: &lengths,
                },
                &mut scratch,
                &mut output,
            ),
            Err(DirectRegisteredAccountArtifactErrorV4::Geometry)
        );
        assert_eq!(output, before);
    }
}
