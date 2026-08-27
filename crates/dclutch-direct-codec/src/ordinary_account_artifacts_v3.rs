//! Exact Hot38 AccountProfile for inline ordinary Direct V3.
//!
//! The profile owns only logical projection and effect authority. Common Hot
//! independently authenticates the injected config, Product graph, portfolio,
//! and ProductBasis records. Claims and Custody remain the sole owners of their
//! account frames and state layouts.

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
use dclutch_claims_svm::{
    frame_spec_v1::{
        ClaimsFrameDataV1, ClaimsFrameRoleV1, SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1,
        SparseNativeTransferFrameSpecV1,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketLayoutV2, LiabilityBasisPositionLayoutV2,
    },
};
use dclutch_custody_contract::{
    CustodyFrameDataV1, CustodyFrameRoleV1, CustodyFrameSpecV1, CustodyReplayLayoutV1, OperationV1,
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
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;

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
const FIXED_OPERATIONS: usize = 34;
const FIXED_DATA_PREDICATES: usize = 9;
const CLAIMS_MARKET_ACCOUNT: u16 = DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + 1;
const CLAIMS_SOURCE_POSITION_ACCOUNT: u16 = DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + 20;
const CLAIMS_DESTINATION_POSITION_ACCOUNT: u16 = DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + 21;
const SYSTEM_PROGRAM_ACCOUNT: u16 = 11;
const REALM_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 6;
const CUSTODY_REPLAY_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 8;
const BUYER_TOKEN_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 10;
const SELLER_TOKEN_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 11;
const CUSTODY_AUTHORITY_ACCOUNT: u16 = DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3 + 12;
const FEE_TOKEN_ACCOUNT: u16 = DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3 + 11;
const ROOT_BYTES: usize = CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1;
const BASIS_PREFIX_BYTES: usize = BASIS_WIDTH_OFFSET_V3 + 4;
// Product Runtime V2 defines `outcome_count = cut_count + 2`: the fixed
// domain header already carries the two boundary outcomes, so only `N - 2`
// cuts are affine. Keeping those cuts in the item term would overstate every
// canonical domain by two rows.
const DOMAIN_AFFINE_BASE_BYTES: usize = DOMAIN_HEADER_BYTES - 2 * DOMAIN_CUT_BYTES;
const CLAIMS_ROW_BYTES: usize = 8;

/// Exact encoded fixed-topology Profile14 width for inline ordinary Direct execution.
pub const DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3: usize = FIXED_DATA_PREDICATE_HEADER_BYTES
    + FIXED_DATA_PREDICATES * FIXED_DATA_PREDICATE_BYTES
    + FIXED_ACCOUNTS * RULE_BYTES
    + FIXED_OPERATIONS * OPERATION_BYTES;

/// Exact account observations used to finalize one release-pinned profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryAccountProfileInputV3<'a> {
    /// Exact logical data lengths in Profile14 coordinate order.
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

/// Emit one complete inline-ordinary fixed-topology Profile14 atomically.
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
    let predicates = fixed_data_predicates()?;
    let operations = operations()?;
    encode_account_profile_with_fixed_data_predicates_v2_atomic(
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
        &predicates,
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

fn fixed_data_predicates()
-> Result<[FixedDataPredicateInputV2; FIXED_DATA_PREDICATES], DirectOrdinaryAccountArtifactErrorV3>
{
    let root = |state_offset: usize| {
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(state_offset)
            .ok_or(DirectOrdinaryAccountArtifactErrorV3::Geometry)
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
        maker_magic_predicate(DIRECT_SELLER_MAKER_ACCOUNT_V3)?,
        maker_version_predicate(DIRECT_SELLER_MAKER_ACCOUNT_V3)?,
        maker_reserved_predicate(DIRECT_SELLER_MAKER_ACCOUNT_V3)?,
        maker_magic_predicate(DIRECT_BUYER_MAKER_ACCOUNT_V3)?,
        maker_version_predicate(DIRECT_BUYER_MAKER_ACCOUNT_V3)?,
        maker_reserved_predicate(DIRECT_BUYER_MAKER_ACCOUNT_V3)?,
    ])
}

fn maker_magic_predicate(
    account: u16,
) -> Result<FixedDataPredicateInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(FixedDataPredicateInputV2::RequireDataU64 {
        account,
        data_offset: offset(DirectMakerReplayLayoutV1::MAGIC)?,
        value: DirectMakerReplayLayoutV1::MAGIC_WORD,
    })
}

fn maker_version_predicate(
    account: u16,
) -> Result<FixedDataPredicateInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(FixedDataPredicateInputV2::RequireDataU16 {
        account,
        data_offset: offset(DirectMakerReplayLayoutV1::VERSION)?,
        value: DirectMakerReplayLayoutV1::ABI_VERSION,
    })
}

fn maker_reserved_predicate(
    account: u16,
) -> Result<FixedDataPredicateInputV2, DirectOrdinaryAccountArtifactErrorV3> {
    Ok(FixedDataPredicateInputV2::RequireZeroRange {
        account,
        data_offset: offset(DirectMakerReplayLayoutV1::RESERVED)?,
        length: width(DirectMakerReplayLayoutV1::RESERVED_BYTES)?,
    })
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
    // Both replay-root creation payers are one rent payer: the 1,224-byte
    // continuation packet has no room for a second signer, so coordinate 9 is
    // an authenticated route alias of coordinate 6 (see `ROUTE_ALIASES`). Both
    // coordinates are still stated here so the alias loop's privilege-equality
    // refusal has the pre-alias privileges to compare.
    for account in [6_usize, 9] {
        *rule_mut(&mut output, account)? = exact(
            signer_writable,
            AccountEffectPermissionsV2::new(true, false, false),
            0,
            0,
        );
    }
    // One lifecycle-scoped RentCredit serves the whole Market lifecycle: a
    // `LifecycleRentCreditV2` PDA is keyed by Market and generation alone, so
    // the two per-authority V1 credits this profile used to pin were never two
    // accounts on chain. Coordinate 7 is that sole credit; the adapter requires
    // it writable so a Close may credit it, and authenticates its 128 bytes,
    // rent exemption, Market/release-set/generation binding, and PDA itself.
    *rule_mut(&mut output, 7)? = exact(
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
    *rule_mut(&mut output, 10)? = opaque(executable);
    // The System Program is a chain-supplied builtin, not a protocol record: a
    // live validator backs it with a NativeLoader account whose width is the
    // validator's business (21 bytes under solana-program-test, 14 on Agave).
    // The profile authenticates its identity through the trusted-builtin
    // `require_key` below and asserts nothing about its bytes.
    *rule_mut(&mut output, usize::from(SYSTEM_PROGRAM_ACCOUNT))? = opaque(executable);

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
        let privileges = if account.role() == ClaimsFrameRoleV1::CallerAuthority {
            outer_child_authority_privileges()
        } else {
            claims_privileges(account.privileges())
        };
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
            let privileges = if account.role() == CustodyFrameRoleV1::CallerAuthority {
                outer_child_authority_privileges()
            } else {
                custody_privileges(account.privileges())
            };
            rule.rule.privileges = privileges;
            // A Realm-selected token program owns the byte width of its own
            // mint and token accounts -- a Token-2022 mint carrying extensions
            // is not 82 bytes and an ImmutableOwner account is not 165 -- and
            // the loader that deployed that program owns the program record's
            // width. None of those three widths is Direct's to assert, and
            // Custody independently authenticates all three accounts against
            // the Realm. This is 52f14fa's coordinate-11 ruling applied to the
            // collateral adapter.
            if matches!(
                custody
                    .data(local)
                    .map_err(|_| DirectOrdinaryAccountArtifactErrorV3::Frame)?,
                CustodyFrameDataV1::OpaqueData
                    | CustodyFrameDataV1::CallerProgramData
                    | CustodyFrameDataV1::TokenMint
                    | CustodyFrameDataV1::TokenAccount
                    | CustodyFrameDataV1::TokenProgram
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

    // An authenticated route alias is a privilege-free logical view: the
    // representative coordinate is the single semantic owner of the route's
    // physical privileges.  Every Direct alias already observed exactly its
    // representative's privileges, so declaring none here changes no authority.
    for (account, representative) in ROUTE_ALIASES {
        let privileges = rule_at(&output, usize::from(*representative))?
            .rule
            .privileges;
        if rule_at(&output, usize::from(*account))?.rule.privileges != privileges {
            return Err(DirectOrdinaryAccountArtifactErrorV3::Geometry);
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
        // Coordinate 9 is an authenticated route alias of coordinate 6, and an
        // operation may never target an alias coordinate: the representative is
        // the single logical authority, so this one owner anchor covers both
        // payer coordinates and the alias derives its debit permission from it.
        require_owner(6, IDENTITY_SYSTEM_PROGRAM_V3)?,
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
        // The collateral mint and the token program are authenticated immutable
        // facts of the Realm, so the profile PROJECTS them out of the Realm
        // record and never re-requires them against a coordinate.
        //
        // It cannot: `OP_REQUIRE_KEY` compares an observed key against
        // `input_identities`, and the only identities a family may place in the
        // input bank are the closed trusted-environment set (current slot,
        // current executing program, System Program) that the runtime supplies
        // directly. Record-derived facts land in the OUTPUT bank, which
        // `OP_REQUIRE_KEY` cannot read, so a
        // `require_key(mint_coordinate, IDENTITY_MINT_V3)` here compares the
        // mint against a zero register and is unsatisfiable by construction.
        // Seeding the input bank from the Realm instead would make the
        // family-neutral Hot executor a second semantic owner of RealmLayoutV1,
        // and seeding it from the caller is exactly the caller choice the
        // operation was meant to forbid.
        //
        // Neither is needed. These two projected registers are what the Effect
        // writes into `CustodyRequestLayoutV1::{MINT, TOKEN_PROGRAM}`, and
        // Custody — the semantic owner of the vault — independently
        // authenticates the Realm record and then requires
        // `request.mint == realm.collateral_mint()`,
        // `request.token_program == realm.token_program()`, the token program to
        // equal the Realm's selected collateral adapter release, and the live
        // frame accounts to equal both (plus `mint.owner == token_program`).
        // The outer restatement was strictly weaker than the child's own check,
        // which is the standing ruling for child privileges applied to child
        // identities.
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
        project_key(BUYER_TOKEN_ACCOUNT, IDENTITY_BUYER_TOKEN_ACCOUNT_V3)?,
        project_key(SELLER_TOKEN_ACCOUNT, IDENTITY_SELLER_TOKEN_ACCOUNT_V3)?,
        project_key(CUSTODY_AUTHORITY_ACCOUNT, IDENTITY_CUSTODY_AUTHORITY_V3)?,
        project_key(FEE_TOKEN_ACCOUNT, IDENTITY_FEE_TOKEN_ACCOUNT_V3)?,
    ])
}

const ROUTE_ALIASES: &[(u16, u16)] = &[
    // One rent payer funds both replay-root creations. Declaring coordinates 6
    // and 9 as distinct self-representatives asserted two signers, which the
    // 1,224-byte continuation packet cannot carry and which the runtime refuses
    // as a `CrossItemAlias` the moment both coordinates observe the same key.
    (9, 6),
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
        || length_at(lengths, 7)? != width(LIFECYCLE_RENT_CREDIT_BYTES_V2)?
        || length_at(lengths, 8)? != width(DIRECT_MAKER_REPLAY_BYTES_V1)?
        || length_at(lengths, 9)? != 0
        // Coordinates 10 and 11 are chain-supplied programs -- the Rent program
        // that owns the lifecycle credit and the System Program. A loader owns
        // the first record's width and the validator owns the second's, so
        // nothing here may pin either.
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

    use std::{vec, vec::Vec};

    use super::*;
    use dclutch_rent_contract::RENT_CREDIT_BYTES_V1;

    use dclutch_account_profile_contract::{
        AccountObservationV1, EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
        EFFECT_PERMISSION_WRITE_DATA,
        v2::{
            Error as ProfileError, FixedDataPredicateKindV2, ProjectionRegistersV2,
            derive_effect_permissions, project_atomic,
        },
    };
    use dclutch_effect_kernel::v2::AccountPermission;

    fn lengths(basis_bytes: u32) -> [u32; FIXED_ACCOUNTS] {
        let mut output = [0_u32; FIXED_ACCOUNTS];
        output[0] = width(ROOT_BYTES).expect("root");
        output[1] = width(DIRECT_EXECUTION_CONFIG_BYTES_V1).expect("config");
        output[2] = width(PRODUCT_RECORD_BYTES_V2).expect("product");
        output[3] =
            width(PORTFOLIO_HEADER_BYTES + 3 * PORTFOLIO_COEFFICIENT_BYTES).expect("portfolio");
        output[4] = basis_bytes;
        output[5] = width(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker");
        output[7] = width(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("lifecycle RentCredit");
        output[8] = width(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker");
        output[10] = width(LOADER_V3_PROGRAM_BYTES).expect("Rent program");
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
    fn profile14_round_trips_exact_hot_child_geometry_and_state_predicates() {
        let bytes = emit(&lengths(256));
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        assert!(profile.uses_fixed_data_predicates());
        assert_eq!(profile.fixed_data_predicate_count(), 9);
        let root_magic = profile.fixed_data_predicate(0).expect("root magic");
        assert_eq!(root_magic.account(), 0);
        assert_eq!(
            root_magic.data_offset(),
            u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::MAGIC)
                .expect("root magic offset")
        );
        assert_eq!(
            root_magic.kind(),
            FixedDataPredicateKindV2::RequireDataU64(DirectRootStateLayoutV1::MAGIC_WORD)
        );
        for (ordinal, account) in [(3_u16, 5_u16), (6, 8)] {
            let magic = profile.fixed_data_predicate(ordinal).expect("maker magic");
            let version = profile
                .fixed_data_predicate(ordinal + 1)
                .expect("maker version");
            let reserved = profile
                .fixed_data_predicate(ordinal + 2)
                .expect("maker reserved");
            assert_eq!(magic.account(), account);
            assert_eq!(
                magic.kind(),
                FixedDataPredicateKindV2::RequireDataU64(DirectMakerReplayLayoutV1::MAGIC_WORD)
            );
            assert_eq!(
                version.kind(),
                FixedDataPredicateKindV2::RequireDataU16(DirectMakerReplayLayoutV1::ABI_VERSION)
            );
            assert_eq!(
                reserved.kind(),
                FixedDataPredicateKindV2::RequireZeroRange(
                    u32::try_from(DirectMakerReplayLayoutV1::RESERVED_BYTES)
                        .expect("reserved width")
                )
            );
        }
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
        assert_eq!(profile.representative(3, 9), Ok(6));
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
        for coordinate in [11_u16, 12, 27, 29, 31, 34, 46, 48, 62, 76] {
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
        let credit = profile.rule(false, 7).expect("lifecycle RentCredit");
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
        let rent_program = profile.rule(false, 10).expect("Rent program");
        assert_eq!(
            rent_program.prestate(),
            AccountPrestateV2::AuthenticatedOpaqueReadonlyData
        );
        assert!(rent_program.route_privileges().executable());
        assert_eq!(rent_program.effect_permissions(), 0);
    }

    #[test]
    fn one_profile_is_polymorphic_across_categorical_and_graded_basis_bodies() {
        assert_eq!(emit(&lengths(256)), emit(&lengths(736)));
    }

    #[test]
    fn checked_release_programdata_and_authority_widths_do_not_change_profile_identity() {
        let baseline = lengths(256);
        let mut real_deployment = baseline;
        real_deployment[11] = 21;
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

    /// A Realm may select Token-2022, whose mint and token accounts carry
    /// extensions, and its Rent/token programs may sit under either loader.
    /// None of those widths is Direct's to assert.
    #[test]
    fn extended_collateral_and_program_record_widths_do_not_change_profile_identity() {
        let baseline = lengths(256);
        let mut token_2022 = baseline;
        // Token-2022 mint: 82 base + TLV padding + MetadataPointer + TransferFeeConfig.
        token_2022[43] = 82 + 83 + 6 + 234;
        // Token-2022 accounts: 165 base + TLV padding + ImmutableOwner.
        for coordinate in [44_usize, 45, 73] {
            *token_2022.get_mut(coordinate).expect("token account") = 165 + 5 + 4;
        }
        // A fixed-loader token program carries its whole ELF, not 36 bytes.
        token_2022[47] = 1_141_117;
        // The Rent program under a fixed loader, likewise.
        token_2022[10] = 987_654;
        for (account, representative) in ROUTE_ALIASES {
            let value = *token_2022
                .get(usize::from(*representative))
                .expect("representative");
            *token_2022.get_mut(usize::from(*account)).expect("alias") = value;
        }
        assert_eq!(emit(&baseline), emit(&token_2022));
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

    /// The V1 48-byte and legacy 64-byte credit geometries are exactly what the
    /// adapter refused: it authenticates 128 bytes of `LifecycleRentCreditV2`.
    #[test]
    fn superseded_rent_credit_geometries_refuse_atomically() {
        for hostile_width in [
            width(RENT_CREDIT_BYTES_V1).expect("V1 RentCredit width"),
            64,
        ] {
            let mut hostile = lengths(256);
            hostile[7] = hostile_width;
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
                Err(DirectOrdinaryAccountArtifactErrorV3::Geometry),
                "RentCredit width {hostile_width}"
            );
            assert_eq!(output, before);
        }
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

    const ADMISSION_TAIL_V3: u32 = 3;
    const TRUSTED_TRADING_PROGRAM: [u8; 32] = [0xf2; 32];
    const TRUSTED_SYSTEM_PROGRAM: [u8; 32] = [0xf1; 32];
    /// A live validator backs the System Program with a NativeLoader record:
    /// 21 bytes under `solana-program-test`, 14 on Agave. Neither is zero.
    const NATIVE_LOADER_RECORD_BYTES: usize = 21;
    /// Claims and Custody `CallerAuthority` coordinates in the Direct topology.
    const CHILD_CALLER_AUTHORITY_COORDINATES_V3: &[usize] = &[12, 34, 48, 62, 76];

    /// One complete runtime observation set for the canonical Direct topology.
    ///
    /// Coordinates are materialised per representative and then copied onto
    /// their route aliases, exactly as the Hot adapter expands one physical
    /// account vector into ninety logical coordinates.
    struct DirectObservationsV3 {
        keys: Vec<[u8; 32]>,
        owners: Vec<[u8; 32]>,
        lamports: Vec<u64>,
        data: Vec<Vec<u8>>,
        signer: Vec<bool>,
        writable: Vec<bool>,
        executable: Vec<bool>,
        variable: Vec<bool>,
    }

    impl DirectObservationsV3 {
        fn observations(&self) -> Vec<AccountObservationV1<'_>> {
            (0..FIXED_ACCOUNTS)
                .map(|coordinate| {
                    let key = self.keys.get(coordinate).expect("key");
                    let owner = self.owners.get(coordinate).expect("owner");
                    let lamports = *self.lamports.get(coordinate).expect("lamports");
                    let data = self.data.get(coordinate).expect("data").as_slice();
                    let signer = *self.signer.get(coordinate).expect("signer");
                    let writable = *self.writable.get(coordinate).expect("writable");
                    let executable = *self.executable.get(coordinate).expect("executable");
                    if *self.variable.get(coordinate).expect("variable") {
                        AccountObservationV1::new_adapter_authenticated_variable_data(
                            key, owner, lamports, data, signer, writable, executable,
                        )
                    } else {
                        AccountObservationV1::new(
                            key, owner, lamports, data, signer, writable, executable,
                        )
                    }
                })
                .collect()
        }

        fn project(&self) -> Result<(), ProfileError> {
            let bytes = emit(&lengths(256));
            let profile = AccountProfileV2::decode(&bytes).expect("decode");
            let observations = self.observations();
            let scalars = usize::from(profile.common_scalar_count())
                + usize::from(profile.item_scalar_stride()) * ADMISSION_TAIL_V3 as usize;
            let identities = usize::from(profile.common_identity_count())
                + usize::from(profile.item_identity_stride()) * ADMISSION_TAIL_V3 as usize;
            let mut input_scalars = vec![0_u64; scalars];
            *input_scalars.get_mut(SCALAR_SLOT_V3).expect("slot") = 500;
            let mut input_identities = vec![[0_u8; 32]; identities];
            *input_identities
                .get_mut(IDENTITY_TRADING_PROGRAM_V3)
                .expect("Trading") = TRUSTED_TRADING_PROGRAM;
            *input_identities
                .get_mut(IDENTITY_SYSTEM_PROGRAM_V3)
                .expect("System") = TRUSTED_SYSTEM_PROGRAM;
            let mut scratch_scalars = input_scalars.clone();
            let mut scratch_identities = input_identities.clone();
            let mut output_scalars = input_scalars.clone();
            let mut output_identities = input_identities.clone();
            project_atomic(
                profile,
                ADMISSION_TAIL_V3,
                &observations,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            )
        }
    }

    /// Materialise the canonical live topology the Hot executor actually meets.
    ///
    /// This is deliberately the *hostile* shape for the four repaired defects:
    /// one rent payer at both payer coordinates, a nonempty NativeLoader System
    /// Program record, and child `CallerAuthority` coordinates that do not sign
    /// outside their child CPI.
    fn direct_observations() -> DirectObservationsV3 {
        let bytes = emit(&lengths(256));
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        let tail = ADMISSION_TAIL_V3;
        let mut value = DirectObservationsV3 {
            keys: vec![[0_u8; 32]; FIXED_ACCOUNTS],
            owners: vec![[0_u8; 32]; FIXED_ACCOUNTS],
            lamports: vec![0_u64; FIXED_ACCOUNTS],
            data: vec![Vec::new(); FIXED_ACCOUNTS],
            signer: vec![false; FIXED_ACCOUNTS],
            writable: vec![false; FIXED_ACCOUNTS],
            executable: vec![false; FIXED_ACCOUNTS],
            variable: vec![false; FIXED_ACCOUNTS],
        };
        for coordinate in 0..FIXED_ACCOUNTS {
            let index = u16::try_from(coordinate).expect("coordinate");
            let representative = profile
                .representative(tail, coordinate)
                .expect("representative");
            if representative != coordinate {
                continue;
            }
            let rule = profile.rule(false, index).expect("rule");
            let privileges = rule.route_privileges();
            *value.keys.get_mut(coordinate).expect("key") =
                [u8::try_from(coordinate).expect("key byte") + 1; 32];
            *value.owners.get_mut(coordinate).expect("owner") = [0x0a; 32];
            *value.lamports.get_mut(coordinate).expect("lamports") = 1_000 + coordinate as u64;
            *value.signer.get_mut(coordinate).expect("signer") = privileges.signer();
            *value.writable.get_mut(coordinate).expect("writable") = privileges.writable();
            *value.executable.get_mut(coordinate).expect("executable") = privileges.executable();
            let width = match rule.prestate() {
                // A first-use replay root is vacant; its predicates are skipped
                // and its projections read zero.
                AccountPrestateV2::LifecycleBound => 0,
                AccountPrestateV2::AdapterAuthenticatedVariableData => {
                    usize::try_from(rule.data_length()).expect("basis prefix") + 64
                }
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData => NATIVE_LOADER_RECORD_BYTES,
                _ => usize::try_from(rule.data_length() + rule.data_item_stride() * tail)
                    .expect("exact width"),
            };
            *value.data.get_mut(coordinate).expect("data") = vec![0_u8; width];
            *value.variable.get_mut(coordinate).expect("variable") =
                rule.prestate() == AccountPrestateV2::AdapterAuthenticatedVariableData;
        }
        // Trusted-environment and builtin relations the profile requires.
        *value.owners.get_mut(0).expect("root owner") = TRUSTED_TRADING_PROGRAM;
        // Live facts asserted here, NOT read back out of the profile, so this
        // topology stays a witness rather than a mirror of whatever the emitter
        // happens to declare.
        //
        // One rent payer signs for both replay-root creations.
        for coordinate in [6_usize, 9] {
            *value.keys.get_mut(coordinate).expect("payer key") = [0x6a; 32];
            *value.owners.get_mut(coordinate).expect("payer owner") = TRUSTED_SYSTEM_PROGRAM;
            *value.lamports.get_mut(coordinate).expect("payer lamports") = 5_000_000;
            *value.data.get_mut(coordinate).expect("payer data") = Vec::new();
            *value.signer.get_mut(coordinate).expect("payer signer") = true;
            *value.writable.get_mut(coordinate).expect("payer writable") = true;
        }
        // The chain backs the System Program with a nonempty NativeLoader record.
        let system = usize::from(SYSTEM_PROGRAM_ACCOUNT);
        *value.keys.get_mut(system).expect("System Program key") = TRUSTED_SYSTEM_PROGRAM;
        *value.data.get_mut(system).expect("System Program data") =
            vec![0x7f; NATIVE_LOADER_RECORD_BYTES];
        // A child frame's caller authority signs only inside its own CPI, so the
        // outer observation of the same account never carries the signer bit.
        for coordinate in CHILD_CALLER_AUTHORITY_COORDINATES_V3 {
            *value.signer.get_mut(*coordinate).expect("child authority") = false;
        }
        // Fixed-data predicates on the live capability root.
        value.write_data(
            0,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::MAGIC,
            &DirectRootStateLayoutV1::MAGIC_WORD.to_le_bytes(),
        );
        value.write_data(
            0,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::VERSION,
            &DirectRootStateLayoutV1::ABI_VERSION.to_le_bytes(),
        );
        // The authenticated ProductBasis record carries the real outcome tail.
        value.write_data(4, BASIS_WIDTH_OFFSET_V3, &tail.to_le_bytes());
        for (account, representative) in ROUTE_ALIASES {
            value.copy_alias(usize::from(*account), usize::from(*representative));
        }
        value
    }

    impl DirectObservationsV3 {
        fn copy_alias(&mut self, account: usize, representative: usize) {
            let key = *self.keys.get(representative).expect("key");
            let owner = *self.owners.get(representative).expect("owner");
            let lamports = *self.lamports.get(representative).expect("lamports");
            let data = self.data.get(representative).expect("data").clone();
            let signer = *self.signer.get(representative).expect("signer");
            let writable = *self.writable.get(representative).expect("writable");
            let executable = *self.executable.get(representative).expect("executable");
            *self.keys.get_mut(account).expect("alias key") = key;
            *self.owners.get_mut(account).expect("alias owner") = owner;
            *self.lamports.get_mut(account).expect("alias lamports") = lamports;
            *self.data.get_mut(account).expect("alias data") = data;
            *self.signer.get_mut(account).expect("alias signer") = signer;
            *self.writable.get_mut(account).expect("alias writable") = writable;
            *self.executable.get_mut(account).expect("alias executable") = executable;
            *self.variable.get_mut(account).expect("alias variable") = false;
        }

        fn write_data(&mut self, coordinate: usize, offset: usize, value: &[u8]) {
            self.data
                .get_mut(coordinate)
                .expect("data")
                .get_mut(offset..offset + value.len())
                .expect("field")
                .copy_from_slice(value);
        }

        fn set_key(&mut self, coordinate: usize, value: [u8; 32]) {
            *self.keys.get_mut(coordinate).expect("key") = value;
        }

        fn set_signer(&mut self, coordinate: usize, value: bool) {
            *self.signer.get_mut(coordinate).expect("signer") = value;
        }

        fn set_data(&mut self, coordinate: usize, value: Vec<u8>) {
            *self.data.get_mut(coordinate).expect("data") = value;
        }
    }

    #[test]
    fn the_canonical_live_direct_topology_projects_at_the_real_tail() {
        assert_eq!(direct_observations().project(), Ok(()));
    }

    #[test]
    fn one_rent_payer_is_admitted_and_two_distinct_payers_are_refused() {
        let mut hostile = direct_observations();
        // Coordinate 9 is a route alias, so a second distinct payer key is now
        // an alias violation instead of the CrossItemAlias that two declared
        // self-representatives produced.
        hostile.set_key(9, [0xcc; 32]);
        assert_eq!(hostile.project(), Err(ProfileError::AliasMismatch));

        // Two distinct representatives may still never share one key.
        let mut hostile = direct_observations();
        let seller = *hostile.keys.get(5).expect("seller replay root");
        hostile.set_key(8, seller);
        assert_eq!(hostile.project(), Err(ProfileError::CrossItemAlias));
    }

    #[test]
    fn a_nonempty_system_program_record_is_admitted_at_every_validator_width() {
        for width in [0_usize, 14, 21] {
            let mut value = direct_observations();
            value.set_data(usize::from(SYSTEM_PROGRAM_ACCOUNT), vec![0x7f; width]);
            assert_eq!(value.project(), Ok(()), "System Program width {width}");
        }
    }

    #[test]
    fn a_child_caller_authority_that_signs_outside_its_cpi_is_refused() {
        let bytes = emit(&lengths(256));
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        for coordinate in CHILD_CALLER_AUTHORITY_COORDINATES_V3.iter().copied() {
            let index = u16::try_from(coordinate).expect("coordinate");
            assert_eq!(
                profile.rule(false, index).expect("authority").privileges(),
                0,
                "coordinate {coordinate} must declare no outer privilege"
            );
            let mut hostile = direct_observations();
            hostile.set_signer(coordinate, true);
            assert_eq!(
                hostile.project(),
                Err(ProfileError::PrivilegeMismatch),
                "coordinate {coordinate}"
            );
        }
    }

    #[test]
    fn the_aliased_payer_still_derives_its_representative_debit_authority() {
        let bytes = emit(&lengths(256));
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        let mut permissions = vec![
            AccountPermission::read_only();
            profile
                .logical_account_count(ADMISSION_TAIL_V3)
                .expect("logical count")
        ];
        derive_effect_permissions(profile, ADMISSION_TAIL_V3, &mut permissions)
            .expect("effect permissions");
        let debit = AccountPermission::new(true, false, false);
        assert_eq!(permissions.get(6), Some(&debit));
        assert_eq!(permissions.get(9), Some(&debit));
        // Ninety logical coordinates pack into forty-three physical accounts
        // carrying exactly one signer, which is what the 1,224-byte
        // continuation packet can actually carry.
        assert_eq!(
            profile
                .logical_account_count(ADMISSION_TAIL_V3)
                .expect("logical count"),
            FIXED_ACCOUNTS
        );
        assert_eq!(
            profile
                .physical_account_count_with_dynamic_spans(ADMISSION_TAIL_V3, &[])
                .expect("physical count"),
            43
        );
        let signers = (0..FIXED_ACCOUNTS)
            .filter(|coordinate| {
                let index = u16::try_from(*coordinate).expect("coordinate");
                profile
                    .representative(ADMISSION_TAIL_V3, *coordinate)
                    .expect("representative")
                    == *coordinate
                    && profile
                        .rule(false, index)
                        .expect("rule")
                        .route_privileges()
                        .signer()
            })
            .count();
        assert_eq!(signers, 1);
        assert_eq!(
            profile.rule(false, 9).expect("payer alias").prestate(),
            AccountPrestateV2::AuthenticatedRouteAlias
        );
        assert_eq!(profile.rule(false, 9).expect("payer alias").privileges(), 0);
        assert_eq!(
            profile
                .rule(false, 9)
                .expect("payer alias")
                .effect_permissions(),
            0
        );
    }
}
