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
use dclutch_custody_contract::{
    CustodyFrameDataV1, CustodyFrameRoleV1, CustodyFrameSpecV1, OperationV1,
    TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_market_core_codec::STATE_BYTES as CORE_STATE_BYTES;
use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3;
use dclutch_product_runtime_v2::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES, PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_realm_contract::{REALM_BYTES, RealmLayoutV1};
use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;

use crate::{
    registered_creation_artifacts_v4::{
        DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4,
        DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4,
        DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4,
        DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4, REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4,
        REGISTERED_IDENTITY_CUSTODY_AUTHORITY_V4, REGISTERED_IDENTITY_CUSTODY_VAULT_V4,
        REGISTERED_IDENTITY_LIFECYCLE_RENT_CREDIT_V4,
        REGISTERED_IDENTITY_LIFECYCLE_RENT_PROGRAM_V4, REGISTERED_IDENTITY_LINKED_BASIS_V4,
        REGISTERED_IDENTITY_MAKER_BENEFICIARY_OBSERVATION_V4, REGISTERED_IDENTITY_MARKET_V4,
        REGISTERED_IDENTITY_MINT_V4, REGISTERED_IDENTITY_PAYER_V4,
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
        DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4,
        DIRECT_REGISTERED_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V4, DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
        DIRECT_REGISTERED_PAYER_ACCOUNT_V4, DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
        DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4,
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
pub const DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4: u16 = 55;
/// Executable Custody program all three Custody routes are invoked through.
///
/// A Custody FrameSpec names `CallerProgram`/`CallerProgramData` -- Trading's --
/// and never its own callee, because a CPI's callee is not one of its own
/// accounts. The family-neutral Hot executor resolves a child route's program by
/// scanning the downgraded effect accounts for the key the activated release set
/// names for that role, so the program has to BE one of the logical coordinates
/// or every route refuses before its first CPI. RegisterBuy routes to Custody
/// three times and to Claims never, so unlike the inline-ordinary topology it
/// had no frame-supplied program coordinate of any kind. Appended past every
/// route range so that carrying it renumbers no frame.
pub const DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4: u16 = 54;

const _: () = assert!(
    DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4 + 1 == DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4
);
const _: () = assert!(
    DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4
        >= DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4 + TRANSFER_ACCOUNT_COUNT_V1
);

const FIXED_ACCOUNTS: usize = DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4 as usize;
const FIXED_OPERATIONS: usize = 34;
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
    // A registered creation has ONE payer. Coordinate 9 is an authenticated
    // route alias of coordinate 6 (see `ROUTE_ALIASES`); both are still stated
    // here so the alias loop's privilege-equality refusal has the pre-alias
    // privileges to compare, exactly as the ordinary topology states its own.
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
    // One lifecycle-scoped RentCredit serves the whole Market lifecycle: a
    // `LifecycleRentCreditV2` PDA is keyed by Market and generation alone, so
    // the two per-account V1 credits this profile used to pin were never two
    // accounts on chain. Coordinate 7 is that sole credit; the adapter requires
    // it writable so a Close may credit it, and authenticates its 128 bytes,
    // rent exemption, Market/release-set/generation binding, and PDA itself.
    *rule_mut(
        &mut output,
        usize::from(DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4),
    )? = exact(
        writable,
        AccountEffectPermissionsV2::new(false, true, false),
        width(LIFECYCLE_RENT_CREDIT_BYTES_V2)?,
        0,
    );
    // The Rent program owns that credit. The adapter derives the credit from
    // `account.owner` and then requires the owner to appear in the frame as an
    // executable readonly account, so the Rent program is a coordinate here.
    // Its record is a loader's business -- 36 bytes under the upgradeable
    // loader, a whole ELF under a fixed loader -- so nothing pins its width.
    *rule_mut(
        &mut output,
        usize::from(DIRECT_REGISTERED_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V4),
    )? = opaque(executable);
    // The System Program is a chain-supplied builtin, not a protocol record: a
    // live validator backs it with a NativeLoader account whose width is the
    // validator's business (21 bytes under solana-program-test, 14 on Agave).
    // The profile authenticates its identity through the trusted-builtin
    // `require_key` below and asserts nothing about its bytes.
    *rule_mut(&mut output, usize::from(SYSTEM_PROGRAM_ACCOUNT))? = opaque(executable);
    // The Custody program the three Custody routes are invoked through. Stated
    // opaque for the same reason the inline-ordinary topology states its own:
    // the loader that deployed it owns the record width, and the Registry
    // activation cache -- not this profile -- is the sole authority on which
    // program the Custody role selects.
    *rule_mut(
        &mut output,
        usize::from(DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4),
    )? = opaque(executable);

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
            let privileges = if account.role() == CustodyFrameRoleV1::CallerAuthority {
                outer_child_authority_privileges()
            } else {
                custody_privileges(account.privileges())
            };
            let rule = rule_mut(&mut output, usize::from(start + local))?;
            rule.rule.privileges = privileges;
            // A Token-2022 mint carrying extensions is not 82 bytes, an
            // ImmutableOwner account is not 165, and a program record's width
            // belongs to whichever loader deployed it. None of the three is
            // Direct's to assert, and Custody -- the semantic owner -- already
            // authenticates all three against the authenticated Realm, so the
            // outer restatement was strictly weaker than the child's own check.
            if matches!(
                frame
                    .data(local)
                    .map_err(|_| DirectRegisteredAccountArtifactErrorV4::Frame)?,
                CustodyFrameDataV1::OpaqueData
                    | CustodyFrameDataV1::CallerProgramData
                    | CustodyFrameDataV1::TokenMint
                    | CustodyFrameDataV1::TokenAccount
                    | CustodyFrameDataV1::TokenProgram
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
        // Coordinate 9 is an authenticated route alias of coordinate 6, and an
        // operation may never target an alias coordinate: the representative is
        // the single logical authority, so this one owner anchor covers both
        // payer coordinates and the alias derives its debit permission from it.
        require_owner(
            DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
            REGISTERED_IDENTITY_SYSTEM_PROGRAM_V4,
        )?,
        project_key(
            DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
            REGISTERED_IDENTITY_PAYER_V4,
        )?,
        // The sole lifecycle credit, and the Rent program that owns it. Without
        // these two the Transition's `identity_eq` pair compared the maker's
        // SIGNED credit keys against registers nothing ever wrote -- an
        // equality against zero that the request decoder's own nonzero check
        // then made unsatisfiable. Nothing caught it because no registered
        // creation has ever executed on a chain.
        project_key(
            DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4,
            REGISTERED_IDENTITY_LIFECYCLE_RENT_CREDIT_V4,
        )?,
        project_key(
            DIRECT_REGISTERED_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V4,
            REGISTERED_IDENTITY_LIFECYCLE_RENT_PROGRAM_V4,
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

/// Logical coordinates that are views of another coordinate's physical account.
///
/// `(9, 6)` is the RULING on the registered family's second self-representative
/// signer, which `52f14fa` settled for ordinary and left open here. Coordinates
/// 6 and 9 are the maker-replay payer and the record payer, and a registered
/// creation has ONE payer: the maker signs one registration request and prepays
/// both accounts it creates. Two distinct representatives observing one key is
/// exactly `CrossItemAlias`, so the ONLY case anyone can construct refused.
///
/// The ordinary ruling's stated reason does NOT carry over, and the registered
/// packet is measured here rather than assumed. `waist.rs` measures the live
/// ordinary continuation at 1,228 bytes of the 1,232-byte v0 packet -- four
/// bytes of margin -- at 44 physical accounts and one signer, and records that
/// each further physical account costs exactly two bytes because the
/// continuation carries the nested Hot account list twice. Registered differs
/// from that baseline by three exactly known terms: fifteen fewer physical
/// accounts (29 against 44, -30 bytes), a registration request 140 bytes
/// narrower than the inline-ordinary one (316 against 456), and one signed
/// intent instead of two in the ed25519 precompile (-14 bytes). That is a
/// derived 1,044 bytes, or 188 bytes of margin -- and a second signer would
/// cost 64 for its signature, 32 for a static key a signer may never ALT-route,
/// and 2 for one more twice-carried account, reaching 1,142 and still fitting.
///
/// So the registered packet would have carried two signers. It is the
/// `CrossItemAlias` refusal, not the byte budget, that forces the alias here,
/// and the next lane to reach for a second signer should re-read that and not
/// the ordinary packet number. The derived figures are an ESTIMATE from a
/// measured baseline; no registered continuation has ever been compiled.
const ROUTE_ALIASES: &[(u16, u16)] = &[
    (
        DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4,
        DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
    ),
    // Measuring coordinate 9 found four more signers, and two of them are the
    // SAME defect: the Custody `InitializeReplay` and `OpenVault` frames each
    // carry a `Payer` coordinate, and the Effect writes
    // `REGISTERED_IDENTITY_PAYER_V4` -- projected from coordinate 6 -- into both
    // children's `payer` field. Three self-representatives held one key. The
    // other two are the frames' `CallerAuthority` coordinates, which are a
    // Trading PDA that signs only inside its CPI; those are handled by
    // `outer_child_authority_privileges`, not by an alias.
    (
        DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4 + 9,
        DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
    ),
    (
        DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4 + 13,
        DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
    ),
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
        || length_at(lengths, 7)? != width(LIFECYCLE_RENT_CREDIT_BYTES_V2)?
        || length_at(lengths, 8)? != width(DIRECT_REGISTERED_RECORD_BYTES_V2)?
        || length_at(lengths, 9)? != 0
        // Coordinates 10, 11 and 54 are chain-supplied programs -- the Rent
        // program that owns the lifecycle credit, the System Program, and the
        // release-selected Custody program the Custody routes are invoked
        // through. A loader owns the first and third record's width and the
        // validator owns the second's, so nothing here may pin any of them.
        // Coordinates 15 and 16 stay pinned: the checked-release discipline
        // requires those two to be Loader-v3 program records exactly.
        || length_at(lengths, 13)? != width(CORE_STATE_BYTES)?
        || length_at(lengths, 14)? != width(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?
        || length_at(lengths, 15)? != width(LOADER_V3_PROGRAM_BYTES)?
        || length_at(lengths, 16)? != width(LOADER_V3_PROGRAM_BYTES)?
        || length_at(lengths, 18)? != width(REALM_BYTES)?
        || length_at(lengths, 19)? != 0
        || length_at(lengths, REPLAY_ACCOUNT as usize)? != 0
        || length_at(lengths, VAULT_ACCOUNT as usize)? != 0
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

/// Outer privileges declared for a child frame's `CallerAuthority` coordinate.
///
/// A child frame's caller authority is a Trading PDA that signs only inside the
/// child CPI, where the FrameSpec is the sole owner of that privilege. The
/// outer AccountProfile observes the same physical account before any CPI, when
/// it is not a signer, so copying the child's SIGNER declaration outward states
/// something the runtime can never satisfy. The outer rule therefore declares
/// no privilege at all and leaves child privilege truth in the FrameSpec.
const fn outer_child_authority_privileges() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, false)
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
        DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5, DirectRegisteredCreationChildRentWidthsV4,
        encode_direct_registered_creation_lifecycle_v5_atomic,
    };
    use dclutch_account_profile_contract::{
        EFFECT_PERMISSION_CREDIT_LAMPORTS,
        lifecycle_v3::StateLifecyclePolicyV5,
        v2::{FixedDataPredicateKindV2, derive_effect_permissions},
    };
    use dclutch_custody_contract::{
        INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, OPEN_VAULT_ACCOUNT_COUNT_V1, TRANSFER_ACCOUNT_COUNT_V1,
    };
    use dclutch_effect_kernel::v2::AccountPermission;
    use dclutch_effect_kernel::{v2::FixedRole, v4::ProgramV4 as EffectProgramV4};

    use crate::registered_effect_artifacts_v4::{
        DIRECT_REGISTER_BUY_EFFECT_BYTES_V4, encode_direct_register_buy_effect_v4_atomic,
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
        output[7] = width(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("lifecycle RentCredit");
        output[8] = width(DIRECT_REGISTERED_RECORD_BYTES_V2).expect("record");
        output[10] = width(LOADER_V3_PROGRAM_BYTES).expect("Rent program");
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
        // Descriptive only: the Custody program rule is opaque, so no loader's
        // record width is pinned here.
        *output
            .get_mut(usize::from(DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4))
            .expect("Custody program") =
            width(LOADER_V3_PROGRAM_BYTES).expect("Custody program width");
        for (account, representative) in ROUTE_ALIASES {
            let value = *output
                .get(usize::from(*representative))
                .expect("representative");
            *output.get_mut(usize::from(*account)).expect("alias") = value;
        }
        output
    }

    fn emit_from(lengths: &[u32]) -> [u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4] {
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
        encode_direct_register_buy_account_profile_v4_atomic(
            DirectRegisterBuyAccountProfileInputV4 {
                logical_data_lengths: lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    fn emit() -> [u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4] {
        emit_from(&lengths())
    }

    /// Every child role the EffectProgram routes to must be invocable.
    ///
    /// The Hot executor resolves a child route's callee out of the effect
    /// accounts and accepts only a unique readonly executable match. RegisterBuy
    /// routes to Custody three times and to Claims never, so it has no
    /// frame-supplied program coordinate at all: without an outer Custody
    /// program coordinate its very first route refuses before any CPI. The roles
    /// come from the real emitted Effect bytes; the callee coordinate is
    /// authored here, so this is a witness rather than a mirror of the emitter.
    #[test]
    fn every_child_role_the_effect_routes_to_has_an_invocable_program_coordinate() {
        let callees: &[(FixedRole, u16)] = &[(
            FixedRole::Custody,
            DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4,
        )];
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
        let mut effect_bytes = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
        encode_direct_register_buy_effect_v4_atomic(&mut scratch, &mut effect_bytes)
            .expect("effect");
        let effect = EffectProgramV4::decode(&effect_bytes).expect("effect decode");
        let effect = effect.base();
        let bytes = emit();
        let profile = AccountProfileV2::decode(&bytes).expect("profile decode");
        assert_eq!(effect.route_count(), 3);
        let mut route = 0_u16;
        while route < effect.route_count() {
            let role = effect.route(route).expect("route").role();
            let coordinate = callees
                .iter()
                .find(|(named, _)| *named == role)
                .map(|(_, coordinate)| *coordinate)
                .expect("every routed role must name a callee coordinate");
            let rule = profile.rule(false, coordinate).expect("callee rule");
            assert!(
                rule.route_privileges().executable()
                    && !rule.route_privileges().signer()
                    && !rule.route_privileges().writable(),
                "{role:?} callee at {coordinate} is not a readonly executable"
            );
            assert_eq!(
                rule.prestate(),
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData
            );
            assert_eq!(
                profile.representative(3, usize::from(coordinate)),
                Ok(usize::from(coordinate)),
                "{role:?} callee at {coordinate} is an alias"
            );
            // The downgraded effect-account vector carries ONE entry per
            // LOGICAL coordinate, aliases included, so an alias onto the callee
            // makes the executor's scan match twice -- refused exactly as hard
            // as matching none.
            assert!(
                !ROUTE_ALIASES
                    .iter()
                    .any(|(_, representative)| *representative == coordinate),
                "{role:?} callee at {coordinate} is aliased and would match twice"
            );
            route += 1;
        }
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

        let mut lifecycle_scratch = [0_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        let mut lifecycle = [0_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        encode_direct_registered_creation_lifecycle_v5_atomic(
            crate::execution_v3::DirectExecutionActionV3::RegisterBuy,
            Some(DirectRegisteredCreationChildRentWidthsV4 { custody_vault: 165 }),
            &mut lifecycle_scratch,
            &mut lifecycle,
        )
        .expect("lifecycle");
        StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &lifecycle)
            .expect("lifecycle decode")
            .validate_account_profile(profile)
            .expect("profile/lifecycle join");
    }

    /// One credit at coordinate 7, the Rent program that owns it at 10, and a
    /// System Program whose width belongs to the validator.
    #[test]
    fn the_profile_carries_one_v2_lifecycle_credit_its_rent_program_and_an_opaque_system() {
        let bytes = emit();
        let profile = AccountProfileV2::decode(&bytes).expect("profile decode");

        let credit = profile
            .rule(false, DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4)
            .expect("lifecycle RentCredit");
        assert_eq!(credit.prestate(), AccountPrestateV2::Exact);
        assert_eq!(
            credit.data_length(),
            width(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("credit width")
        );
        assert_eq!(credit.data_length(), 128);
        assert_eq!(
            credit.effect_permissions(),
            EFFECT_PERMISSION_CREDIT_LAMPORTS
        );
        assert!(credit.route_privileges().writable());
        assert!(!credit.route_privileges().signer());

        for coordinate in [
            DIRECT_REGISTERED_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V4,
            SYSTEM_PROGRAM_ACCOUNT,
            DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4,
        ] {
            let rule = profile.rule(false, coordinate).expect("opaque program");
            assert_eq!(
                rule.prestate(),
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
                "coordinate {coordinate} still pins a chain-supplied program's width"
            );
            assert!(rule.route_privileges().executable());
            assert_eq!(rule.effect_permissions(), 0);
        }
    }

    /// The V1 48-byte and legacy 64-byte credit geometries are exactly what the
    /// adapter refused: it authenticates 128 bytes of `LifecycleRentCreditV2`.
    #[test]
    fn superseded_rent_credit_geometries_refuse_atomically() {
        for hostile_width in [48_u32, 64] {
            let mut hostile = lengths();
            *hostile
                .get_mut(usize::from(
                    DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4,
                ))
                .expect("lifecycle RentCredit") = hostile_width;
            let mut scratch = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
            let mut output = [0x5a_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
            let before = output;
            assert_eq!(
                encode_direct_register_buy_account_profile_v4_atomic(
                    DirectRegisterBuyAccountProfileInputV4 {
                        logical_data_lengths: &hostile,
                    },
                    &mut scratch,
                    &mut output,
                ),
                Err(DirectRegisteredAccountArtifactErrorV4::Geometry),
                "RentCredit width {hostile_width}"
            );
            assert_eq!(output, before);
        }
    }

    /// Everything a live chain owns the width of: the System Program's
    /// NativeLoader record (21 bytes under `solana-program-test`, 14 on Agave),
    /// a Rent program deployed under a fixed loader, a Token-2022 mint carrying
    /// extensions, token accounts with `ImmutableOwner`, and the token program
    /// itself. None of them is Direct's to assert, so none of them moves the
    /// emitted profile.
    #[test]
    fn chain_owned_record_widths_do_not_change_profile_identity() {
        let baseline = emit_from(&lengths());
        // A live chain observes: a 21-byte NativeLoader System record under
        // `solana-program-test` (14 on Agave), a fixed-loader Rent program and
        // Custody program, a Token-2022 mint carrying extensions, an
        // `ImmutableOwner` source account, and a whole token-program ELF.
        let observed: &[(u16, u32)] = &[
            (SYSTEM_PROGRAM_ACCOUNT, 21),
            (
                DIRECT_REGISTERED_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V4,
                1_141_117,
            ),
            (DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4, 987_654),
            (MINT_ACCOUNT, 278),
            (TOKEN_PROGRAM_ACCOUNT, 1_048_576),
            (SOURCE_ACCOUNT, 170),
        ];
        for system_width in [21_u32, 14] {
            let mut real_chain = lengths();
            for (coordinate, observed_width) in observed {
                let width = if *coordinate == SYSTEM_PROGRAM_ACCOUNT {
                    system_width
                } else {
                    *observed_width
                };
                *real_chain
                    .get_mut(usize::from(*coordinate))
                    .expect("observed coordinate") = width;
            }
            for (account, representative) in ROUTE_ALIASES {
                let value = *real_chain
                    .get(usize::from(*representative))
                    .expect("representative");
                *real_chain.get_mut(usize::from(*account)).expect("alias") = value;
            }
            assert_eq!(
                baseline,
                emit_from(&real_chain),
                "System Program record of {system_width} bytes moved the profile"
            );
        }
    }

    /// The registered family's second self-representative signer, ruled.
    ///
    /// `52f14fa` settled this for the ordinary topology and left the registered
    /// one explicitly unmeasured. The measurement is recorded on `ROUTE_ALIASES`
    /// and the numbers it derives from are asserted here, so the next lane
    /// reaching for a second signer argues with figures rather than a memory of
    /// the ordinary packet -- which does NOT bind here, because registered has
    /// 184 bytes of derived margin where ordinary has four measured bytes.
    ///
    /// What binds is `CrossItemAlias`: coordinates 6 and 9 were two distinct
    /// self-representatives, a registered creation has exactly one payer, and
    /// two representatives observing one key is a hard refusal. The only case
    /// anyone can construct refused.
    #[test]
    fn one_payer_signs_a_registered_creation_and_the_record_payer_aliases_it() {
        let bytes = emit();
        let profile = AccountProfileV2::decode(&bytes).expect("profile decode");

        let alias = profile
            .rule(false, DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4)
            .expect("record payer");
        assert_eq!(alias.prestate(), AccountPrestateV2::AuthenticatedRouteAlias);
        assert_eq!(alias.privileges(), 0);
        assert_eq!(
            profile.representative(3, usize::from(DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4)),
            Ok(usize::from(DIRECT_REGISTERED_PAYER_ACCOUNT_V4))
        );

        // The alias keeps its representative's debit authority: it is a logical
        // view of one physical account, not a permission-free hole.
        let mut permissions = std::vec![AccountPermission::read_only(); profile.logical_account_count(3).expect("logical")];
        derive_effect_permissions(profile, 3, &mut permissions).expect("effect permissions");
        let debit = AccountPermission::new(true, false, false);
        assert_eq!(
            permissions.get(usize::from(DIRECT_REGISTERED_PAYER_ACCOUNT_V4)),
            Some(&debit)
        );
        assert_eq!(
            permissions.get(usize::from(DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4)),
            Some(&debit)
        );

        // The two other payer coordinates the measurement found: each Custody
        // frame's own `Payer`, holding the same key the Effect writes into the
        // child request's `payer` field.
        for start in [
            DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4 + 9,
            DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4 + 13,
        ] {
            assert_eq!(
                profile.representative(3, usize::from(start)),
                Ok(usize::from(DIRECT_REGISTERED_PAYER_ACCOUNT_V4)),
                "Custody frame payer at {start} is a second physical payer"
            );
        }

        // A child frame's caller authority is a Trading PDA that signs inside
        // its CPI and is not a signer when the outer profile observes it.
        for start in [
            DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4,
            DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4,
            DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4,
        ] {
            assert_eq!(
                CustodyFrameSpecV1::new(match start {
                    DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4 =>
                        OperationV1::InitializeReplay,
                    DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4 => OperationV1::OpenVault,
                    _ => OperationV1::Transfer,
                })
                .account(0)
                .expect("caller authority")
                .role(),
                CustodyFrameRoleV1::CallerAuthority
            );
            assert_eq!(
                profile
                    .rule(false, start)
                    .expect("caller authority rule")
                    .privileges(),
                0,
                "the Custody caller authority at {start} signs outside its CPI"
            );
        }

        // The measured inputs to the packet derivation on `ROUTE_ALIASES`.
        assert_eq!(
            profile.logical_account_count(3),
            Ok(usize::from(DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4))
        );
        assert_eq!(profile.physical_account_count(3), Ok(29));
        let signers = (0..DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4)
            .filter(|coordinate| {
                profile
                    .rule(false, *coordinate)
                    .expect("rule")
                    .route_privileges()
                    .signer()
            })
            .count();
        let named: std::vec::Vec<u16> = (0..DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4)
            .filter(|coordinate| {
                profile
                    .rule(false, *coordinate)
                    .expect("rule")
                    .route_privileges()
                    .signer()
            })
            .collect();
        assert_eq!(
            signers, 1,
            "a registered creation carries one signer: {named:?}"
        );
        assert_eq!(
            crate::execution_v3::DIRECT_REGISTRATION_REQUEST_BYTES_V3,
            316
        );
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
