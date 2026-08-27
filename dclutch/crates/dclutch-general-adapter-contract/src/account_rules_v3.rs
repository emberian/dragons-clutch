//! Exact Profile13 account rules for the General successor.
//!
//! Child-frame order and privileges come directly from the Claims and Custody
//! semantic owners. Repeated appearances of the same semantic role are encoded
//! as authenticated route aliases, so no General-local physical account table
//! can drift from the child adapters. The sole dynamic span is the Trading-owned
//! authenticated scratch-page bank; its count comes from the protected scalar
//! derived from canonical register-bank geometry.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
    encode::{
        AccountAliasInputV2, AccountEffectPermissionsV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2,
    },
};
use dclutch_claims_svm::frame_spec_v1::{
    ClaimsFrameDataV1, ClaimsFrameRoleV1, ClaimsFrameSpecV1, FramePrivilegesV1,
};
use dclutch_custody_contract::{
    CustodyFrameDataV1, CustodyFramePrivilegesV1, CustodyFrameRoleV1, CustodyFrameSpecV1,
};
use dclutch_general_codec::{Action, SELECTION_POLICY_BYTES};
use dclutch_general_config_contract::v3::GENERAL_CONFIG_BYTES_V3;
use dclutch_product_runtime_v2::{PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;

use crate::{
    effect_artifacts_v3::{
        GeneralChildFrameV3, general_effect_account_count_v3, general_effect_route_count_v3,
        general_effect_route_frame_v3,
    },
    hot_candidate_v3::scalar,
    local_state_v3::GENERAL_LOCAL_STATE_HEADER_BYTES_V3,
    runtime_manifest::SETTLEMENT_MANIFEST_HEADER_BYTES_V2,
    runtime_selection::RUNTIME_SELECTION_CURSOR_BYTES_V2,
    runtime_verify::RUNTIME_VERIFIER_HEADER_BYTES_V2,
    runtime_width::{SETTLEMENT_CURSOR_HEADER_BYTES_V2, VERIFIED_CANDIDATE_HEADER_BYTES_V2},
    state_artifacts_v3::{
        GENERAL_CLOSE_PAYER_ACCOUNT_V3, GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
        GeneralReadonlyEvidenceKindV3, general_readonly_evidence_count_v3,
        general_readonly_evidence_v3,
    },
};

/// Profile13 discriminator required by every successor General account artifact.
pub const GENERAL_ACCOUNT_PROFILE_ARTIFACT_V3: u16 = DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE;
/// One scratch-page rule is repeated by the protected physical page count.
pub const GENERAL_SCRATCH_PAGE_RULE_STRIDE_V3: u16 = 1;

/// Release-selected external account widths not owned by General.
///
/// Every value is either an exact fixed width or a checked nonzero prefix for
/// an adapter-authenticated variable record. The selected release builder gets
/// these values from the named semantic owner; no runtime caller controls them.
///
/// The Realm-selected collateral Mint, token Account and Token Program widths
/// are deliberately absent: those three belong to the token program the Realm
/// selected and to the loader that deployed it, never to General. Their
/// coordinates are emitted opaque, so there is no value here for a caller to
/// supply and no width for this profile to assert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralExternalAccountWidthsV3 {
    /// Checked nonzero linked-basis prefix.
    pub linked_basis_prefix: u32,
    /// Finalized result-domain record width.
    pub result_domain: u32,
    /// Runtime Rent sysvar width.
    pub rent_sysvar: u32,
    /// Canonical Core Market width.
    pub core_market: u32,
    /// Current Registry activation-cache width.
    pub activation_cache: u32,
    /// Loader-v3 Program account width.
    pub upgradeable_program: u32,
    /// Checked nonzero Trading ProgramData prefix.
    pub trading_programdata_prefix: u32,
    /// Checked nonzero Claims ProgramData prefix.
    pub claims_programdata_prefix: u32,
    /// Checked nonzero Core ProgramData prefix.
    pub core_programdata_prefix: u32,
    /// Immutable Realm record width.
    pub realm_record: u32,
    /// Canonical RentCredit width.
    pub rent_credit: u32,
}

impl GeneralExternalAccountWidthsV3 {
    fn validate(self) -> Result<()> {
        if self.linked_basis_prefix == 0
            || self.result_domain == 0
            || self.rent_sysvar == 0
            || self.core_market == 0
            || self.activation_cache == 0
            || self.upgradeable_program == 0
            || self.trading_programdata_prefix == 0
            || self.claims_programdata_prefix == 0
            || self.core_programdata_prefix == 0
            || self.realm_record == 0
            || self.rent_credit == 0
        {
            Err(GeneralAccountRuleErrorV3::ExternalWidth)
        } else {
            Ok(())
        }
    }
}

/// Stable refusal from General Profile13 rule generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAccountRuleErrorV3 {
    /// An action-selected coordinate or child frame was outside its exact range.
    Geometry,
    /// A release-selected external width was zero or otherwise unusable.
    ExternalWidth,
}

/// Result alias for General Profile13 rule generation.
pub type Result<T> = core::result::Result<T, GeneralAccountRuleErrorV3>;

/// Exact base logical account count before authenticated scratch pages.
pub fn general_account_profile_fixed_count_v3(action: Action) -> Result<u16> {
    general_effect_account_count_v3(action).map_err(|_| GeneralAccountRuleErrorV3::Geometry)
}

/// Descriptor-owned dynamic scratch-page span.
pub fn general_scratch_page_span_v3(action: Action) -> Result<DynamicFixedSpanInputV2> {
    Ok(DynamicFixedSpanInputV2 {
        insertion_coordinate: general_account_profile_fixed_count_v3(action)?,
        count_scalar: u16::try_from(scalar::INPUT_SCRATCH_PAGE_COUNT)
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
        rule_start: 0,
        rule_stride: GENERAL_SCRATCH_PAGE_RULE_STRIDE_V3,
        minimum: 1,
        maximum: u32::MAX,
        step: 1,
    })
}

/// Exact opaque readonly rule repeated for every Trading-owned scratch page.
#[must_use]
pub const fn general_scratch_page_rule_v3() -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, false, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    }
}

/// Generate one exact fixed rule in the action-selected General Profile13.
pub fn general_account_profile_rule_v3(
    action: Action,
    coordinate: u16,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    widths.validate()?;
    if coordinate >= general_account_profile_fixed_count_v3(action)? {
        return Err(GeneralAccountRuleErrorV3::Geometry);
    }
    if coordinate < 5 {
        return common_rule(action, coordinate, widths);
    }
    if coordinate == GENERAL_PRIMARY_STATE_ACCOUNT_V3
        || (action == Action::Close && coordinate == GENERAL_TERMINAL_STATE_ACCOUNT_V3)
    {
        return local_state_rule(action, coordinate);
    }
    let payer = if action == Action::Close {
        GENERAL_CLOSE_PAYER_ACCOUNT_V3
    } else {
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3
    };
    let rent_credit = if action == Action::Close {
        GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3
    } else {
        GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3
    };
    if coordinate == payer {
        return Ok(exact_rule(true, true, false, 0, 0, no_effects()));
    }
    if coordinate == rent_credit {
        return Ok(exact_rule(
            false,
            true,
            false,
            widths.rent_credit,
            0,
            no_effects(),
        ));
    }
    let mut evidence = 0_u16;
    while evidence < general_readonly_evidence_count_v3(action) {
        let selected = general_readonly_evidence_v3(action, evidence)
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
        if selected.coordinate == coordinate {
            return evidence_rule(selected.kind);
        }
        evidence = evidence
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    child_rule(action, coordinate, widths)
}

fn common_rule(
    action: Action,
    coordinate: u16,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    match coordinate {
        0 => Ok(exact_rule(
            false,
            true,
            false,
            u32::try_from(
                dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
                    .checked_add(dclutch_general_config_contract::GENERAL_ROOT_BYTES_V2)
                    .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
            )
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        1 => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(GENERAL_CONFIG_BYTES_V3)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        2 => Ok(rule(
            physical_role_privileges(action, ChildRoleV3::ProductRecord)?,
            u32::try_from(PRODUCT_RECORD_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        )),
        3 => Ok(rule(
            physical_role_privileges(action, ChildRoleV3::PortfolioRecord)?,
            u32::try_from(PORTFOLIO_HEADER_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            u32::try_from(PORTFOLIO_COEFFICIENT_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            AccountPrestateV2::Exact,
        )),
        4 => Ok(variable_rule_with(
            physical_role_privileges(action, ChildRoleV3::BasisRecord)?,
            widths.linked_basis_prefix,
        )),
        _ => Err(GeneralAccountRuleErrorV3::Geometry),
    }
}

fn local_state_rule(action: Action, coordinate: u16) -> Result<AccountRuleWithPrestateInputV2> {
    let semantic_header = if matches!(action, Action::Consider | Action::Freeze) {
        RUNTIME_SELECTION_CURSOR_BYTES_V2
    } else {
        SETTLEMENT_CURSOR_HEADER_BYTES_V2
    };
    // `LifecycleBound` admits either vacant data or the declared live width,
    // and that is the truth for every coordinate a lifecycle plan may create.
    // Close creates only the terminal record. Its settlement cursor is the
    // account being closed: the Close plan and the operator both require it
    // live at exactly this width, so declaring it possibly-vacant was a
    // weaker refusal than the transition it guards.
    let closed_settlement_state =
        action == Action::Close && coordinate == GENERAL_PRIMARY_STATE_ACCOUNT_V3;
    Ok(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: u32::try_from(
                GENERAL_LOCAL_STATE_HEADER_BYTES_V3
                    .checked_add(semantic_header)
                    .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
            )
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            data_item_stride: if matches!(action, Action::Consider | Action::Freeze) {
                0
            } else {
                8
            },
        },
        prestate: if closed_settlement_state {
            AccountPrestateV2::Exact
        } else {
            AccountPrestateV2::LifecycleBound
        },
    })
}

fn evidence_rule(kind: GeneralReadonlyEvidenceKindV3) -> Result<AccountRuleWithPrestateInputV2> {
    match kind {
        GeneralReadonlyEvidenceKindV3::SelectionPolicy => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(SELECTION_POLICY_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate
        | GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(VERIFIED_CANDIDATE_HEADER_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            16,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::FrozenSelection => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(RUNTIME_SELECTION_CURSOR_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::RuntimeVerifier => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(RUNTIME_VERIFIER_HEADER_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            40,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::SettlementManifest => Ok(variable_rule(
            u32::try_from(SETTLEMENT_MANIFEST_HEADER_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
        )),
    }
}

fn child_rule(
    action: Action,
    coordinate: u16,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    let (frame, relative) = child_coordinate(action, coordinate)?;
    let role = child_role(frame, relative)?;
    let privileges = physical_role_privileges(action, role)?;
    if let Some(representative) = prior_role_coordinate(action, coordinate, role)? {
        return Ok(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, false),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::Fixed(representative),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        });
    }
    match frame {
        GeneralChildFrameV3::ClaimsProtocolPosition(action) => claims_data_rule(
            ClaimsFrameSpecV1::protocol_position(action)
                .data(relative)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            privileges,
            widths,
        ),
        GeneralChildFrameV3::ClaimsAffine { position_count } => claims_data_rule(
            ClaimsFrameSpecV1::affine(position_count)
                .and_then(|spec| spec.data(relative))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            privileges,
            widths,
        ),
        GeneralChildFrameV3::Custody(operation) => custody_data_rule(
            CustodyFrameSpecV1::new(operation)
                .data(relative)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            privileges,
            widths,
        ),
    }
}

fn child_coordinate(action: Action, coordinate: u16) -> Result<(GeneralChildFrameV3, u16)> {
    let mut route = 0_u16;
    while route < general_effect_route_count_v3(action) {
        let selected = general_effect_route_frame_v3(action, route)
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
        let count = selected
            .frame
            .account_count()
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
        let end = selected
            .account_start
            .checked_add(count)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
        if coordinate >= selected.account_start && coordinate < end {
            return Ok((selected.frame, coordinate - selected.account_start));
        }
        route = route
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    Err(GeneralAccountRuleErrorV3::Geometry)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildRoleV3 {
    CallerAuthority,
    CoreMarket,
    ActivationCache,
    RegistryProgram,
    TradingProgram,
    TradingProgramData,
    RentSysvar,
    SystemProgram,
    ProductRecord,
    PortfolioRecord,
    BasisRecord,
    Claims(ClaimsFrameRoleV1),
    Custody(CustodyFrameRoleV1),
}

fn child_role(frame: GeneralChildFrameV3, relative: u16) -> Result<ChildRoleV3> {
    match frame {
        GeneralChildFrameV3::ClaimsProtocolPosition(action) => {
            ClaimsFrameSpecV1::protocol_position(action)
                .account(relative)
                .map(|account| normalize_claims_role(account.role()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::ClaimsAffine { position_count } => {
            ClaimsFrameSpecV1::affine(position_count)
                .and_then(|spec| spec.account(relative))
                .map(|account| normalize_claims_role(account.role()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::Custody(operation) => CustodyFrameSpecV1::new(operation)
            .account(relative)
            .map(|account| normalize_custody_role(account.role()))
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry),
    }
}

fn normalize_claims_role(role: ClaimsFrameRoleV1) -> ChildRoleV3 {
    match role {
        ClaimsFrameRoleV1::CallerAuthority => ChildRoleV3::CallerAuthority,
        ClaimsFrameRoleV1::CoreMarket => ChildRoleV3::CoreMarket,
        ClaimsFrameRoleV1::ActivationCache => ChildRoleV3::ActivationCache,
        ClaimsFrameRoleV1::RegistryProgram => ChildRoleV3::RegistryProgram,
        ClaimsFrameRoleV1::TradingProgram | ClaimsFrameRoleV1::CallerProgram => {
            ChildRoleV3::TradingProgram
        }
        ClaimsFrameRoleV1::TradingProgramData | ClaimsFrameRoleV1::CallerProgramData => {
            ChildRoleV3::TradingProgramData
        }
        ClaimsFrameRoleV1::RentSysvar => ChildRoleV3::RentSysvar,
        ClaimsFrameRoleV1::SystemProgram => ChildRoleV3::SystemProgram,
        ClaimsFrameRoleV1::ProductRecord => ChildRoleV3::ProductRecord,
        ClaimsFrameRoleV1::PortfolioRecord => ChildRoleV3::PortfolioRecord,
        ClaimsFrameRoleV1::BasisRecord => ChildRoleV3::BasisRecord,
        other => ChildRoleV3::Claims(other),
    }
}

fn normalize_custody_role(role: CustodyFrameRoleV1) -> ChildRoleV3 {
    match role {
        CustodyFrameRoleV1::CallerAuthority => ChildRoleV3::CallerAuthority,
        CustodyFrameRoleV1::CoreMarket => ChildRoleV3::CoreMarket,
        CustodyFrameRoleV1::ActivationCache => ChildRoleV3::ActivationCache,
        CustodyFrameRoleV1::RegistryProgram => ChildRoleV3::RegistryProgram,
        CustodyFrameRoleV1::CallerProgram => ChildRoleV3::TradingProgram,
        CustodyFrameRoleV1::CallerProgramData => ChildRoleV3::TradingProgramData,
        CustodyFrameRoleV1::RentSysvar => ChildRoleV3::RentSysvar,
        CustodyFrameRoleV1::SystemProgram => ChildRoleV3::SystemProgram,
        other => ChildRoleV3::Custody(other),
    }
}

fn prior_role_coordinate(
    action: Action,
    coordinate: u16,
    role: ChildRoleV3,
) -> Result<Option<u16>> {
    let common = match role {
        ChildRoleV3::ProductRecord => Some(2),
        ChildRoleV3::PortfolioRecord => Some(3),
        ChildRoleV3::BasisRecord => Some(4),
        _ => None,
    };
    if common.is_some() {
        return Ok(common);
    }
    let mut prior = crate::state_artifacts_v3::general_child_account_start_v3(action);
    while prior < coordinate {
        let (frame, relative) = child_coordinate(action, prior)?;
        if child_role(frame, relative)? == role {
            return Ok(Some(prior));
        }
        prior = prior
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    Ok(None)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ChildPrivilegeFactsV3 {
    signer: bool,
    writable: bool,
    executable: bool,
}

impl ChildPrivilegeFactsV3 {
    fn include(&mut self, other: Self) {
        self.signer |= other.signer;
        self.writable |= other.writable;
        self.executable |= other.executable;
    }

    fn account_privileges(self) -> AccountPrivilegesV2 {
        AccountPrivilegesV2::new(self.signer, self.writable, self.executable)
    }
}

fn child_privilege_facts(
    frame: GeneralChildFrameV3,
    relative: u16,
) -> Result<ChildPrivilegeFactsV3> {
    match frame {
        GeneralChildFrameV3::ClaimsProtocolPosition(action) => {
            ClaimsFrameSpecV1::protocol_position(action)
                .account(relative)
                .map(|account| claims_privilege_facts(account.privileges()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::ClaimsAffine { position_count } => {
            ClaimsFrameSpecV1::affine(position_count)
                .and_then(|spec| spec.account(relative))
                .map(|account| claims_privilege_facts(account.privileges()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::Custody(operation) => CustodyFrameSpecV1::new(operation)
            .account(relative)
            .map(|account| custody_privilege_facts(account.privileges()))
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry),
    }
}

fn claims_privilege_facts(value: FramePrivilegesV1) -> ChildPrivilegeFactsV3 {
    ChildPrivilegeFactsV3 {
        signer: value.signer(),
        writable: value.writable(),
        executable: value.executable(),
    }
}

fn custody_privilege_facts(value: CustodyFramePrivilegesV1) -> ChildPrivilegeFactsV3 {
    ChildPrivilegeFactsV3 {
        signer: value.signer(),
        writable: value.writable(),
        executable: value.executable(),
    }
}

fn physical_role_privileges(action: Action, role: ChildRoleV3) -> Result<AccountPrivilegesV2> {
    let mut union = ChildPrivilegeFactsV3::default();
    let mut coordinate = crate::state_artifacts_v3::general_child_account_start_v3(action);
    let count = general_account_profile_fixed_count_v3(action)?;
    while coordinate < count {
        let (frame, relative) = child_coordinate(action, coordinate)?;
        if child_role(frame, relative)? == role {
            union.include(child_privilege_facts(frame, relative)?);
        }
        coordinate = coordinate
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    Ok(union.account_privileges())
}

fn claims_data_rule(
    data: ClaimsFrameDataV1,
    privileges: AccountPrivilegesV2,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    match data {
        ClaimsFrameDataV1::Exact(bytes) => Ok(rule(privileges, bytes, 0, AccountPrestateV2::Exact)),
        ClaimsFrameDataV1::OpaqueData | ClaimsFrameDataV1::PositionOwnerIdentity => {
            Ok(opaque_rule(privileges))
        }
        ClaimsFrameDataV1::ProductTail { base, item_stride } => Ok(rule(
            privileges,
            base,
            item_stride,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::LinkedBasisRecord => {
            Ok(variable_rule_with(privileges, widths.linked_basis_prefix))
        }
        ClaimsFrameDataV1::ProductRecord => exact_external(privileges, PRODUCT_RECORD_BYTES_V2),
        ClaimsFrameDataV1::ResultDomainRecord => Ok(rule(
            privileges,
            widths.result_domain,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::PortfolioRecord => Ok(rule(
            privileges,
            u32::try_from(PORTFOLIO_HEADER_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            u32::try_from(PORTFOLIO_COEFFICIENT_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::RentSysvar => Ok(rule(
            privileges,
            widths.rent_sysvar,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::CoreMarket => Ok(rule(
            privileges,
            widths.core_market,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::ActivationCache => Ok(rule(
            privileges,
            widths.activation_cache,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::UpgradeableProgram => Ok(rule(
            privileges,
            widths.upgradeable_program,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::ProgramData(role) => {
            let prefix = match role {
                dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Trading
                | dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Caller => {
                    widths.trading_programdata_prefix
                }
                dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Claims => {
                    widths.claims_programdata_prefix
                }
                dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Core => {
                    widths.core_programdata_prefix
                }
            };
            Ok(variable_rule_with(privileges, prefix))
        }
        ClaimsFrameDataV1::RentCredit => Ok(rule(
            privileges,
            widths.rent_credit,
            0,
            AccountPrestateV2::Exact,
        )),
    }
}

fn custody_data_rule(
    data: CustodyFrameDataV1,
    privileges: AccountPrivilegesV2,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    match data {
        CustodyFrameDataV1::Exact(bytes) => {
            Ok(rule(privileges, bytes, 0, AccountPrestateV2::Exact))
        }
        CustodyFrameDataV1::OpaqueData => Ok(opaque_rule(privileges)),
        CustodyFrameDataV1::CoreMarket => Ok(rule(
            privileges,
            widths.core_market,
            0,
            AccountPrestateV2::Exact,
        )),
        CustodyFrameDataV1::ActivationCache => Ok(rule(
            privileges,
            widths.activation_cache,
            0,
            AccountPrestateV2::Exact,
        )),
        CustodyFrameDataV1::UpgradeableProgram => Ok(rule(
            privileges,
            widths.upgradeable_program,
            0,
            AccountPrestateV2::Exact,
        )),
        // A Realm-selected token program owns the byte width of its own mint
        // and token accounts -- a Token-2022 mint carrying extensions is not
        // 82 bytes and an ImmutableOwner account is not 165 -- and the loader
        // that deployed that program owns the program record's width. None of
        // those three widths is General's to assert, and Custody independently
        // authenticates all three accounts against the authenticated Realm, so
        // the outer restatement was strictly weaker than the child's own
        // check. This is ee1dc7d's collateral-width ruling applied to the
        // General child frames.
        CustodyFrameDataV1::TokenProgram
        | CustodyFrameDataV1::TokenMint
        | CustodyFrameDataV1::TokenAccount => Ok(opaque_rule(privileges)),
        CustodyFrameDataV1::CallerProgramData => Ok(variable_rule_with(
            privileges,
            widths.trading_programdata_prefix,
        )),
        CustodyFrameDataV1::RealmRecord => Ok(rule(
            privileges,
            widths.realm_record,
            0,
            AccountPrestateV2::Exact,
        )),
        CustodyFrameDataV1::RentSysvar => Ok(rule(
            privileges,
            widths.rent_sysvar,
            0,
            AccountPrestateV2::Exact,
        )),
    }
}

fn exact_external(
    privileges: AccountPrivilegesV2,
    bytes: usize,
) -> Result<AccountRuleWithPrestateInputV2> {
    Ok(rule(
        privileges,
        u32::try_from(bytes).map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
        0,
        AccountPrestateV2::Exact,
    ))
}

const fn no_effects() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, false)
}

const fn exact_rule(
    signer: bool,
    writable: bool,
    executable: bool,
    data_length: u32,
    data_item_stride: u32,
    effect_permissions: AccountEffectPermissionsV2,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(signer, writable, executable),
            effect_permissions,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate: AccountPrestateV2::Exact,
    }
}

const fn rule(
    privileges: AccountPrivilegesV2,
    data_length: u32,
    data_item_stride: u32,
    prestate: AccountPrestateV2,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: no_effects(),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate,
    }
}

const fn variable_rule(prefix: u32) -> AccountRuleWithPrestateInputV2 {
    variable_rule_with(AccountPrivilegesV2::new(false, false, false), prefix)
}

const fn variable_rule_with(
    privileges: AccountPrivilegesV2,
    prefix: u32,
) -> AccountRuleWithPrestateInputV2 {
    rule(
        privileges,
        prefix,
        0,
        AccountPrestateV2::AdapterAuthenticatedVariableData,
    )
}

const fn opaque_rule(privileges: AccountPrivilegesV2) -> AccountRuleWithPrestateInputV2 {
    rule(
        privileges,
        0,
        0,
        AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTHS: GeneralExternalAccountWidthsV3 = GeneralExternalAccountWidthsV3 {
        linked_basis_prefix: 64,
        result_domain: 192,
        rent_sysvar: 17,
        core_market: 320,
        activation_cache: 160,
        upgradeable_program: 36,
        trading_programdata_prefix: 45,
        claims_programdata_prefix: 45,
        core_programdata_prefix: 45,
        realm_record: 112,
        rent_credit: 48,
    };

    const ACTIONS: [Action; 7] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
    ];

    #[test]
    fn every_action_rule_is_total_and_scratch_span_is_physical_only() {
        for action in ACTIONS {
            let count = general_account_profile_fixed_count_v3(action).expect("fixed count");
            let mut coordinate = 0_u16;
            while coordinate < count {
                general_account_profile_rule_v3(action, coordinate, WIDTHS).expect("exact rule");
                coordinate += 1;
            }
            assert_eq!(
                general_account_profile_rule_v3(action, count, WIDTHS),
                Err(GeneralAccountRuleErrorV3::Geometry)
            );
            let span = general_scratch_page_span_v3(action).expect("span");
            assert_eq!(span.insertion_coordinate, count);
            assert_eq!(
                span.count_scalar,
                u16::try_from(scalar::INPUT_SCRATCH_PAGE_COUNT).expect("bounded scalar")
            );
            assert_eq!(span.minimum, 1);
            assert_eq!(span.maximum, u32::MAX);
        }
    }

    #[test]
    fn child_frames_reuse_semantic_roles_through_authenticated_aliases() {
        for action in [Action::InitializeSettlement, Action::Close] {
            let child = crate::state_artifacts_v3::general_child_account_start_v3(action);
            let count = general_account_profile_fixed_count_v3(action).expect("count");
            let mut aliases = 0_usize;
            let mut repeated_signer = false;
            for coordinate in child..count {
                let rule = general_account_profile_rule_v3(action, coordinate, WIDTHS)
                    .expect("exact child rule");
                let (frame, relative) = child_coordinate(action, coordinate).expect("child");
                let role = child_role(frame, relative).expect("role");
                if let AccountAliasInputV2::Fixed(representative) = rule.rule.alias {
                    aliases += 1;
                    assert_eq!(
                        rule.rule.privileges,
                        AccountPrivilegesV2::new(false, false, false),
                        "aliases carry no second privilege truth"
                    );
                    if child_privilege_facts(frame, relative)
                        .expect("FrameSpec privilege")
                        .signer
                    {
                        repeated_signer = true;
                        assert_eq!(
                            general_account_profile_rule_v3(action, representative, WIDTHS)
                                .expect("prior physical representative")
                                .rule
                                .privileges,
                            physical_role_privileges(action, role).expect("physical union"),
                        );
                    }
                }
            }
            assert!(aliases > 0);
            assert!(
                repeated_signer,
                "fixture must exercise repeated signed child roles"
            );
        }
    }

    #[test]
    fn zero_external_width_refuses_before_any_rule_is_emitted() {
        let hostile = GeneralExternalAccountWidthsV3 {
            realm_record: 0,
            ..WIDTHS
        };
        assert_eq!(
            general_account_profile_rule_v3(Action::Freeze, 0, hostile),
            Err(GeneralAccountRuleErrorV3::ExternalWidth)
        );
    }

    /// Every Realm-selected collateral coordinate emits no width claim.
    ///
    /// A Token-2022 mint carrying extensions is not 82 bytes, an
    /// ImmutableOwner token account is not 165, and the width of a program
    /// record belongs to the loader that deployed it. This test names those
    /// coordinates through the Custody FrameSpec that owns them, so a rule
    /// that went back to asserting any of the three fails here rather than on
    /// a validator.
    #[test]
    fn realm_selected_collateral_coordinates_assert_no_width() {
        let mut observed = 0_usize;
        for action in ACTIONS {
            let count = general_account_profile_fixed_count_v3(action).expect("fixed count");
            let child_start = crate::state_artifacts_v3::general_child_account_start_v3(action);
            for coordinate in child_start..count {
                let (frame, relative) = match child_coordinate(action, coordinate) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                let GeneralChildFrameV3::Custody(operation) = frame else {
                    continue;
                };
                let data = CustodyFrameSpecV1::new(operation)
                    .data(relative)
                    .expect("custody frame data");
                if !matches!(
                    data,
                    CustodyFrameDataV1::TokenMint
                        | CustodyFrameDataV1::TokenAccount
                        | CustodyFrameDataV1::TokenProgram
                ) {
                    continue;
                }
                let rule = general_account_profile_rule_v3(action, coordinate, WIDTHS)
                    .expect("collateral rule");
                // A repeated semantic role is an authenticated route alias and
                // carries no second truth of any kind; every other appearance
                // is the physical coordinate and must be opaque.
                if rule.prestate != AccountPrestateV2::AuthenticatedRouteAlias {
                    observed += 1;
                    assert_eq!(
                        rule.prestate,
                        AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
                        "{action:?} coordinate {coordinate} restates a width General does not own"
                    );
                }
                assert_eq!(rule.rule.data_length, 0);
                assert_eq!(rule.rule.data_item_stride, 0);
            }
        }
        assert!(
            observed >= 3,
            "fixture must reach every collateral data kind"
        );
    }

    /// The account Close destroys must be live, never possibly vacant.
    ///
    /// `LifecycleBound` admits vacant data. The Close plan closes the
    /// settlement cursor and the operator requires it live at exactly the
    /// declared width, so a `LifecycleBound` settlement coordinate is a
    /// weaker prestate than the transition it guards -- and, since a
    /// LifecycleBound coordinate must carry an AuthenticateOrCreate plan, it
    /// also refuses the policy/profile join outright.
    #[test]
    fn close_binds_only_the_terminal_record_it_creates() {
        let settlement = general_account_profile_rule_v3(
            Action::Close,
            GENERAL_PRIMARY_STATE_ACCOUNT_V3,
            WIDTHS,
        )
        .expect("Close settlement rule");
        assert_eq!(settlement.prestate, AccountPrestateV2::Exact);
        let terminal = general_account_profile_rule_v3(
            Action::Close,
            GENERAL_TERMINAL_STATE_ACCOUNT_V3,
            WIDTHS,
        )
        .expect("Close terminal rule");
        assert_eq!(terminal.prestate, AccountPrestateV2::LifecycleBound);
        assert_eq!(settlement.rule.data_length, terminal.rule.data_length);
        assert_eq!(
            settlement.rule.data_item_stride,
            terminal.rule.data_item_stride
        );
        for action in ACTIONS {
            if action == Action::Close {
                continue;
            }
            assert_eq!(
                general_account_profile_rule_v3(action, GENERAL_PRIMARY_STATE_ACCOUNT_V3, WIDTHS)
                    .expect("primary state rule")
                    .prestate,
                AccountPrestateV2::LifecycleBound,
                "{action:?} creates its own primary state"
            );
        }
    }
}
