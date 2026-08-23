//! Exact account-role contracts for the full-width V3 collateral routes.
//!
//! This module owns only instruction account order, signer/writable policy,
//! bounded outcome-mint suffixes, and the closed alias exception. A signed
//! owner `Actor` may inherit writable privilege from transaction-level fee
//! payer union; no other read-only role may be escalated.
//! It does not authenticate account ownership, executable bits, persisted
//! bytes, PDAs, token state, or semantic joins; those remain adapter duties.

use super::{CodecError, Result, HASH_BYTES, MAX_OUTCOMES};

/// Exact full-width Endow account count, including owner-plane construction.
pub const ENDOW_ACCOUNT_COUNT_V3: usize = 18;
/// Exact full-width WithdrawCash account count.
pub const WITHDRAW_ACCOUNT_COUNT_V3: usize = 16;
/// Exact full-width Split/Merge account count.
pub const COMPLETE_SET_ACCOUNT_COUNT_V3: usize = 14;
/// Fixed Materialize/Dematerialize prefix before one mint per active outcome.
pub const CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3: usize = 15;
/// Fixed RedeemExternal prefix before one mint per active outcome.
pub const EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3: usize = 18;

/// Canonical indices shared by Endow and WithdrawCash.
pub mod collateral_cash_indices_v3 {
    /// Owner/authority signer.
    pub const ACTOR: usize = 0;
    /// Immutable Realm.
    pub const REALM: usize = 1;
    /// Immutable Profile V2.
    pub const PROFILE: usize = 2;
    /// Collateral policy artifact.
    pub const POLICY: usize = 3;
    /// Realm-selected collateral token program.
    pub const TOKEN_PROGRAM: usize = 4;
    /// General MarketBinding.
    pub const MARKET_BINDING: usize = 5;
    /// General MarketRuntime.
    pub const MARKET_RUNTIME: usize = 6;
    /// Product MarketInstance artifact.
    pub const MARKET_INSTANCE: usize = 7;
    /// Canonical Hoard V2.
    pub const HOARD: usize = 8;
    /// Canonical ClaimLedger V3.
    pub const CLAIM_LEDGER: usize = 9;
    /// Canonical Position V3.
    pub const POSITION: usize = 10;
    /// Canonical GEN1 Replay.
    pub const REPLAY: usize = 11;
    /// Realm-selected collateral mint.
    pub const COLLATERAL_MINT: usize = 12;
    /// Endow source or withdrawal destination.
    pub const DESTINATION: usize = 13;
    /// Hoard token authority.
    pub const HOARD_AUTHORITY: usize = 14;
    /// Hoard token account.
    pub const HOARD_TOKEN: usize = 15;
    /// Endow-only System Program.
    pub const SYSTEM: usize = 16;
    /// Endow-only Rent sysvar.
    pub const RENT: usize = 17;
}

/// Canonical indices for Split and Merge.
pub mod complete_set_indices_v3 {
    pub use super::collateral_cash_indices_v3::{
        ACTOR, CLAIM_LEDGER, COLLATERAL_MINT, HOARD, HOARD_TOKEN, MARKET_BINDING, MARKET_INSTANCE,
        MARKET_RUNTIME, POLICY, POSITION, PROFILE, REALM, REPLAY, TOKEN_PROGRAM,
    };
}

/// Canonical indices for Materialize and Dematerialize.
pub mod claim_representation_indices_v3 {
    /// Owner signer.
    pub const ACTOR: usize = 0;
    pub use super::collateral_cash_indices_v3::{
        CLAIM_LEDGER, HOARD, MARKET_BINDING, MARKET_INSTANCE, MARKET_RUNTIME, POLICY, POSITION,
        PROFILE, REALM, REPLAY,
    };
    /// Realm-selected collateral token program.
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 4;
    /// Independently selected outcome token program.
    pub const OUTCOME_TOKEN_PROGRAM: usize = 12;
    /// Holder claim token account.
    pub const HOLDER_TOKEN: usize = 13;
    /// Immutable ProgramData linked by the outcome token program.
    pub const OUTCOME_TOKEN_PROGRAMDATA: usize = 14;
    /// First canonical outcome-mint role.
    pub const OUTCOME_MINTS: usize = super::CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3;
}

/// Canonical indices for exact-whole external redemption.
pub mod external_redemption_indices_v3 {
    /// Bearer claimant signer.
    pub const CLAIMANT: usize = 0;
    /// Immutable Realm.
    pub const REALM: usize = 1;
    /// Immutable Profile V2.
    pub const PROFILE: usize = 2;
    /// Collateral policy artifact.
    pub const POLICY: usize = 3;
    /// Realm-selected collateral token program.
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 4;
    /// General MarketBinding.
    pub const MARKET_BINDING: usize = 5;
    /// General MarketRuntime.
    pub const MARKET_RUNTIME: usize = 6;
    /// Product MarketInstance artifact.
    pub const MARKET_INSTANCE: usize = 7;
    /// Canonical Hoard V2.
    pub const HOARD: usize = 8;
    /// Canonical ClaimLedger V3.
    pub const CLAIM_LEDGER: usize = 9;
    /// Finalized Resolution V5.
    pub const RESOLUTION: usize = 10;
    /// Realm-selected collateral mint.
    pub const COLLATERAL_MINT: usize = 11;
    /// Collateral payout destination.
    pub const DESTINATION: usize = 12;
    /// Hoard token authority.
    pub const HOARD_AUTHORITY: usize = 13;
    /// Hoard token account.
    pub const HOARD_TOKEN: usize = 14;
    /// Independently selected outcome token program.
    pub const OUTCOME_TOKEN_PROGRAM: usize = 15;
    /// Bearer claim token source.
    pub const SOURCE: usize = 16;
    /// Immutable ProgramData linked by the outcome token program.
    pub const OUTCOME_TOKEN_PROGRAMDATA: usize = 17;
    /// First canonical outcome-mint role.
    pub const OUTCOME_MINTS: usize = super::EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3;
}

/// Enabled full-width collateral action family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollateralActionV3 {
    /// Deposit owner collateral and, when absent, found the owner plane.
    Endow,
    /// Withdraw unreserved owner cash.
    WithdrawCash,
    /// Reclassify cash into a complete native claim set.
    Split,
    /// Reclassify a complete native claim set into cash.
    Merge,
    /// Mint one bearer outcome token against an internal native claim.
    Materialize,
    /// Burn one bearer outcome token into an internal native claim.
    Dematerialize,
    /// Burn a bearer claim and pay its exact whole-atom Resolution V5 value.
    RedeemExternal,
}

impl CollateralActionV3 {
    const fn has_outcome_mint_suffix(self) -> bool {
        matches!(
            self,
            Self::Materialize | Self::Dematerialize | Self::RedeemExternal
        )
    }

    const fn permits_token_program_alias(self) -> bool {
        self.has_outcome_mint_suffix()
    }
}

/// Closed semantic vocabulary for every V3 collateral instruction account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollateralAccountRoleV3 {
    /// Owner, transfer authority, and Endow rent payer.
    Actor,
    /// Read-only signer that owns the externally redeemed bearer claim.
    Claimant,
    /// Immutable Realm selecting the collateral Profile.
    Realm,
    /// Immutable Profile V2 selected by the Realm.
    Profile,
    /// Immutable collateral policy selected by the Profile.
    CollateralPolicy,
    /// Realm-selected collateral token program.
    CollateralTokenProgram,
    /// Immutable General MarketBinding.
    MarketBinding,
    /// Stable General MarketRuntime and claim mint authority.
    MarketRuntime,
    /// Full content-addressed Product MarketInstanceV2 artifact.
    MarketInstanceArtifact,
    /// Canonical Hoard V2 liability mirror.
    Hoard,
    /// Canonical ClaimLedger V3 aggregate.
    ClaimLedger,
    /// Canonical ordinary Position V3.
    Position,
    /// Canonical GEN1 replay account for the ordinary Position.
    Replay,
    /// Realm-selected collateral mint.
    CollateralMint,
    /// Owner collateral token account debited by Endow.
    CollateralSource,
    /// Holder collateral token account credited by withdrawal or redemption.
    CollateralDestination,
    /// Program-derived Hoard token authority.
    HoardAuthority,
    /// Realm-selected collateral custody token account.
    HoardToken,
    /// Canonical System Program used only by Endow owner-plane founding.
    SystemProgram,
    /// Canonical Rent sysvar used only by Endow owner-plane founding.
    RentSysvar,
    /// Separately selected Token-2022 outcome-claim program.
    OutcomeTokenProgram,
    /// Immutable Upgradeable Loader deployment behind the outcome program.
    OutcomeTokenProgramData,
    /// Holder claim token account minted to or burned from.
    HolderClaimToken,
    /// Finalized canonical Resolution V5 account.
    ResolutionV5,
    /// Bearer claim token account burned by external redemption.
    ExternalClaimSource,
    /// Canonical outcome mint at the enclosed zero-based outcome ordinal.
    OutcomeMint(u8),
}

/// One exact account requirement at one instruction index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralAccountMetaV3 {
    /// Semantic role at this index.
    pub role: CollateralAccountRoleV3,
    /// Required writable privilege. `false` remains forbidden except for the
    /// explicitly admitted signed-Actor transaction-payer escalation.
    pub writable: bool,
    /// Exact effective signer privilege.
    pub signer: bool,
}

/// Runtime-observed key and effective privileges for one instruction account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedCollateralAccountMetaV3 {
    /// Runtime account key. The all-zero sentinel is never a live account.
    pub key: [u8; HASH_BYTES],
    /// Effective writable privilege observed by the instruction.
    pub writable: bool,
    /// Effective signer privilege observed by the instruction.
    pub signer: bool,
}

/// Validated, allocation-free account contract for one action and market width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralAccountContractV3 {
    action: CollateralActionV3,
    outcome_count: u8,
    selected_outcome: Option<u8>,
}

impl CollateralAccountContractV3 {
    /// Action whose physical account shape this contract describes.
    pub const fn action(self) -> CollateralActionV3 {
        self.action
    }

    /// Canonical active outcome prefix length.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Selected mint ordinal for claim representation/redemption routes.
    pub const fn selected_outcome(self) -> Option<u8> {
        self.selected_outcome
    }

    /// Exact total account count.
    pub fn len(self) -> usize {
        let prefix = match self.action {
            CollateralActionV3::Endow => ENDOW_ACCOUNT_COUNT_V3,
            CollateralActionV3::WithdrawCash => WITHDRAW_ACCOUNT_COUNT_V3,
            CollateralActionV3::Split | CollateralActionV3::Merge => COMPLETE_SET_ACCOUNT_COUNT_V3,
            CollateralActionV3::Materialize | CollateralActionV3::Dematerialize => {
                CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3
            }
            CollateralActionV3::RedeemExternal => EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3,
        };
        if self.action.has_outcome_mint_suffix() {
            prefix + usize::from(self.outcome_count)
        } else {
            prefix
        }
    }

    /// Whether the contract contains no accounts.
    pub const fn is_empty(self) -> bool {
        false
    }

    /// Required role and privilege policy at one instruction index.
    pub fn meta(self, index: usize) -> Option<CollateralAccountMetaV3> {
        let fixed = match self.action {
            CollateralActionV3::Endow => ENDOW_METAS_V3.get(index).copied(),
            CollateralActionV3::WithdrawCash => WITHDRAW_METAS_V3.get(index).copied(),
            CollateralActionV3::Split | CollateralActionV3::Merge => {
                COMPLETE_SET_METAS_V3.get(index).copied()
            }
            CollateralActionV3::Materialize | CollateralActionV3::Dematerialize => {
                CLAIM_REPRESENTATION_METAS_V3.get(index).copied()
            }
            CollateralActionV3::RedeemExternal => EXTERNAL_REDEMPTION_METAS_V3.get(index).copied(),
        };
        if fixed.is_some() {
            return fixed;
        }
        if !self.action.has_outcome_mint_suffix() {
            return None;
        }
        let prefix = match self.action {
            CollateralActionV3::Materialize | CollateralActionV3::Dematerialize => {
                CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3
            }
            CollateralActionV3::RedeemExternal => EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3,
            _ => return None,
        };
        let outcome = index.checked_sub(prefix)?;
        if outcome >= usize::from(self.outcome_count) {
            return None;
        }
        let outcome = u8::try_from(outcome).ok()?;
        Some(meta(
            CollateralAccountRoleV3::OutcomeMint(outcome),
            self.selected_outcome == Some(outcome),
            false,
        ))
    }

    /// Whether this exact action permits these two logical roles to share a key.
    ///
    /// The sole exception is the two read-only token-program roles on claim
    /// representation and external-redemption routes. No state, mint, holder,
    /// authority, signer, or other program role may alias.
    pub const fn allows_alias(
        self,
        left: CollateralAccountRoleV3,
        right: CollateralAccountRoleV3,
    ) -> bool {
        self.action.permits_token_program_alias()
            && matches!(
                (left, right),
                (
                    CollateralAccountRoleV3::CollateralTokenProgram,
                    CollateralAccountRoleV3::OutcomeTokenProgram
                ) | (
                    CollateralAccountRoleV3::OutcomeTokenProgram,
                    CollateralAccountRoleV3::CollateralTokenProgram
                )
            )
    }

    /// Whether a read-only role may inherit transaction-level writable
    /// privilege without changing its protocol authority.
    pub const fn allows_writable_signer_escalation(self, role: CollateralAccountRoleV3) -> bool {
        matches!(role, CollateralAccountRoleV3::Actor)
    }
}

const fn meta(
    role: CollateralAccountRoleV3,
    writable: bool,
    signer: bool,
) -> CollateralAccountMetaV3 {
    CollateralAccountMetaV3 {
        role,
        writable,
        signer,
    }
}

const ENDOW_METAS_V3: &[CollateralAccountMetaV3; ENDOW_ACCOUNT_COUNT_V3] = &[
    meta(CollateralAccountRoleV3::Actor, true, true),
    meta(CollateralAccountRoleV3::Realm, false, false),
    meta(CollateralAccountRoleV3::Profile, false, false),
    meta(CollateralAccountRoleV3::CollateralPolicy, false, false),
    meta(
        CollateralAccountRoleV3::CollateralTokenProgram,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::MarketBinding, false, false),
    meta(CollateralAccountRoleV3::MarketRuntime, false, false),
    meta(
        CollateralAccountRoleV3::MarketInstanceArtifact,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::Hoard, true, false),
    meta(CollateralAccountRoleV3::ClaimLedger, false, false),
    meta(CollateralAccountRoleV3::Position, true, false),
    meta(CollateralAccountRoleV3::Replay, true, false),
    meta(CollateralAccountRoleV3::CollateralMint, false, false),
    meta(CollateralAccountRoleV3::CollateralSource, true, false),
    meta(CollateralAccountRoleV3::HoardAuthority, false, false),
    meta(CollateralAccountRoleV3::HoardToken, true, false),
    meta(CollateralAccountRoleV3::SystemProgram, false, false),
    meta(CollateralAccountRoleV3::RentSysvar, false, false),
];

const WITHDRAW_METAS_V3: &[CollateralAccountMetaV3; WITHDRAW_ACCOUNT_COUNT_V3] = &[
    meta(CollateralAccountRoleV3::Actor, false, true),
    meta(CollateralAccountRoleV3::Realm, false, false),
    meta(CollateralAccountRoleV3::Profile, false, false),
    meta(CollateralAccountRoleV3::CollateralPolicy, false, false),
    meta(
        CollateralAccountRoleV3::CollateralTokenProgram,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::MarketBinding, false, false),
    meta(CollateralAccountRoleV3::MarketRuntime, false, false),
    meta(
        CollateralAccountRoleV3::MarketInstanceArtifact,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::Hoard, true, false),
    meta(CollateralAccountRoleV3::ClaimLedger, false, false),
    meta(CollateralAccountRoleV3::Position, true, false),
    meta(CollateralAccountRoleV3::Replay, true, false),
    meta(CollateralAccountRoleV3::CollateralMint, false, false),
    meta(CollateralAccountRoleV3::CollateralDestination, true, false),
    meta(CollateralAccountRoleV3::HoardAuthority, false, false),
    meta(CollateralAccountRoleV3::HoardToken, true, false),
];

const COMPLETE_SET_METAS_V3: &[CollateralAccountMetaV3; COMPLETE_SET_ACCOUNT_COUNT_V3] = &[
    meta(CollateralAccountRoleV3::Actor, false, true),
    meta(CollateralAccountRoleV3::Realm, false, false),
    meta(CollateralAccountRoleV3::Profile, false, false),
    meta(CollateralAccountRoleV3::CollateralPolicy, false, false),
    meta(
        CollateralAccountRoleV3::CollateralTokenProgram,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::MarketBinding, false, false),
    meta(CollateralAccountRoleV3::MarketRuntime, false, false),
    meta(
        CollateralAccountRoleV3::MarketInstanceArtifact,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::Hoard, true, false),
    meta(CollateralAccountRoleV3::ClaimLedger, true, false),
    meta(CollateralAccountRoleV3::Position, true, false),
    meta(CollateralAccountRoleV3::Replay, true, false),
    meta(CollateralAccountRoleV3::CollateralMint, false, false),
    meta(CollateralAccountRoleV3::HoardToken, false, false),
];

const CLAIM_REPRESENTATION_METAS_V3: &[CollateralAccountMetaV3;
     CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3] = &[
    meta(CollateralAccountRoleV3::Actor, false, true),
    meta(CollateralAccountRoleV3::Realm, false, false),
    meta(CollateralAccountRoleV3::Profile, false, false),
    meta(CollateralAccountRoleV3::CollateralPolicy, false, false),
    meta(
        CollateralAccountRoleV3::CollateralTokenProgram,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::MarketBinding, false, false),
    meta(CollateralAccountRoleV3::MarketRuntime, false, false),
    meta(
        CollateralAccountRoleV3::MarketInstanceArtifact,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::Hoard, false, false),
    meta(CollateralAccountRoleV3::ClaimLedger, true, false),
    meta(CollateralAccountRoleV3::Position, true, false),
    meta(CollateralAccountRoleV3::Replay, true, false),
    meta(CollateralAccountRoleV3::OutcomeTokenProgram, false, false),
    meta(CollateralAccountRoleV3::HolderClaimToken, true, false),
    meta(
        CollateralAccountRoleV3::OutcomeTokenProgramData,
        false,
        false,
    ),
];

const EXTERNAL_REDEMPTION_METAS_V3: &[CollateralAccountMetaV3;
     EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3] = &[
    meta(CollateralAccountRoleV3::Claimant, false, true),
    meta(CollateralAccountRoleV3::Realm, false, false),
    meta(CollateralAccountRoleV3::Profile, false, false),
    meta(CollateralAccountRoleV3::CollateralPolicy, false, false),
    meta(
        CollateralAccountRoleV3::CollateralTokenProgram,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::MarketBinding, false, false),
    meta(CollateralAccountRoleV3::MarketRuntime, false, false),
    meta(
        CollateralAccountRoleV3::MarketInstanceArtifact,
        false,
        false,
    ),
    meta(CollateralAccountRoleV3::Hoard, true, false),
    meta(CollateralAccountRoleV3::ClaimLedger, true, false),
    meta(CollateralAccountRoleV3::ResolutionV5, false, false),
    meta(CollateralAccountRoleV3::CollateralMint, false, false),
    meta(CollateralAccountRoleV3::CollateralDestination, true, false),
    meta(CollateralAccountRoleV3::HoardAuthority, false, false),
    meta(CollateralAccountRoleV3::HoardToken, true, false),
    meta(CollateralAccountRoleV3::OutcomeTokenProgram, false, false),
    meta(CollateralAccountRoleV3::ExternalClaimSource, true, false),
    meta(
        CollateralAccountRoleV3::OutcomeTokenProgramData,
        false,
        false,
    ),
];

/// Construct the exact account contract for one enabled action.
///
/// Every market width is in `1..=MAX_OUTCOMES`. Fixed routes require no
/// selected outcome; claim representation and external redemption require one
/// selected ordinal strictly inside the active prefix.
pub fn account_contract_v3(
    action: CollateralActionV3,
    outcome_count: u8,
    selected_outcome: Option<u8>,
) -> Result<CollateralAccountContractV3> {
    if outcome_count == 0 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(CodecError::InvalidCount);
    }
    if action.has_outcome_mint_suffix() {
        match selected_outcome {
            Some(outcome) if outcome < outcome_count => {}
            _ => return Err(CodecError::InvalidCount),
        }
    } else if selected_outcome.is_some() {
        return Err(CodecError::InvalidCount);
    }
    Ok(CollateralAccountContractV3 {
        action,
        outcome_count,
        selected_outcome,
    })
}

/// Validate exact count, live keys, effective privileges, and aliases.
///
/// Privileges are compared after unioning the two permitted same-key token
/// program roles, matching Solana's effective privilege behavior. Every other
/// duplicate key is refused.
pub fn validate_collateral_account_metas_v3(
    action: CollateralActionV3,
    outcome_count: u8,
    selected_outcome: Option<u8>,
    observed: &[ObservedCollateralAccountMetaV3],
) -> Result<()> {
    validate_collateral_account_metas_with_v3(
        action,
        outcome_count,
        selected_outcome,
        observed.len(),
        |index| observed.get(index).copied(),
    )
}

/// Allocation-free validator over a caller-owned account observation source.
pub fn validate_collateral_account_metas_with_v3<F>(
    action: CollateralActionV3,
    outcome_count: u8,
    selected_outcome: Option<u8>,
    observed_len: usize,
    mut observed_at: F,
) -> Result<()>
where
    F: FnMut(usize) -> Option<ObservedCollateralAccountMetaV3>,
{
    let contract = account_contract_v3(action, outcome_count, selected_outcome)?;
    if observed_len < contract.len() {
        return Err(CodecError::Truncated);
    }
    if observed_len > contract.len() {
        return Err(CodecError::TrailingBytes);
    }
    for index in 0..observed_len {
        let account = observed_at(index).ok_or(CodecError::Truncated)?;
        if account.key.iter().all(|byte| *byte == 0) {
            return Err(CodecError::ZeroIdentity);
        }
        let requirement = contract.meta(index).ok_or(CodecError::InvalidCount)?;
        let mut effective_writable = requirement.writable;
        let mut effective_signer = requirement.signer;
        for other_index in 0..observed_len {
            let other = observed_at(other_index).ok_or(CodecError::Truncated)?;
            if index == other_index || account.key != other.key {
                continue;
            }
            let other_requirement = contract.meta(other_index).ok_or(CodecError::InvalidCount)?;
            if !contract.allows_alias(requirement.role, other_requirement.role) {
                return Err(CodecError::MismatchedBinding);
            }
            effective_writable |= other_requirement.writable;
            effective_signer |= other_requirement.signer;
        }
        let permitted_actor_escalation = !effective_writable
            && account.writable
            && effective_signer
            && contract.allows_writable_signer_escalation(requirement.role);
        if (account.writable != effective_writable && !permitted_actor_escalation)
            || account.signer != effective_signer
        {
            return Err(CodecError::MismatchedBinding);
        }
    }
    Ok(())
}

/// Validate a live account list while deriving its bounded active-outcome
/// width from the canonical variable suffix.
///
/// Fixed-width actions return `1`; their physical contract is independent of
/// market width. Variable-width actions return the exact nonzero suffix width
/// admitted by the same central role contract. This lets runtimes admit count,
/// privileges, and aliases before indexing any hostile account slice.
pub fn validate_inferred_collateral_account_metas_with_v3<F>(
    action: CollateralActionV3,
    selected_outcome: Option<u8>,
    observed_len: usize,
    observed_at: F,
) -> Result<u8>
where
    F: FnMut(usize) -> Option<ObservedCollateralAccountMetaV3>,
{
    let outcome_count = if action.has_outcome_mint_suffix() {
        let prefix = match action {
            CollateralActionV3::Materialize | CollateralActionV3::Dematerialize => {
                CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3
            }
            CollateralActionV3::RedeemExternal => EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3,
            _ => return Err(CodecError::InvalidTag),
        };
        let suffix_len = observed_len
            .checked_sub(prefix)
            .ok_or(CodecError::Truncated)?;
        u8::try_from(suffix_len).map_err(|_| CodecError::InvalidCount)?
    } else {
        1
    };
    validate_collateral_account_metas_with_v3(
        action,
        outcome_count,
        selected_outcome,
        observed_len,
        observed_at,
    )?;
    Ok(outcome_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    fn observed(
        action: CollateralActionV3,
        outcome_count: u8,
        selected_outcome: Option<u8>,
    ) -> Vec<ObservedCollateralAccountMetaV3> {
        let contract = account_contract_v3(action, outcome_count, selected_outcome).unwrap();
        let mut accounts = Vec::new();
        for index in 0..contract.len() {
            let requirement = contract.meta(index).unwrap();
            accounts.push(ObservedCollateralAccountMetaV3 {
                key: [u8::try_from(index + 1).unwrap(); HASH_BYTES],
                writable: requirement.writable,
                signer: requirement.signer,
            });
        }
        accounts
    }

    fn role_index(contract: CollateralAccountContractV3, role: CollateralAccountRoleV3) -> usize {
        (0..contract.len())
            .find(|index| contract.meta(*index).unwrap().role == role)
            .unwrap()
    }

    #[test]
    fn all_seven_actions_pin_their_exact_counts() {
        assert_eq!(
            account_contract_v3(CollateralActionV3::Endow, 16, None)
                .unwrap()
                .len(),
            ENDOW_ACCOUNT_COUNT_V3
        );
        assert_eq!(
            account_contract_v3(CollateralActionV3::WithdrawCash, 16, None)
                .unwrap()
                .len(),
            WITHDRAW_ACCOUNT_COUNT_V3
        );
        for action in [CollateralActionV3::Split, CollateralActionV3::Merge] {
            assert_eq!(
                account_contract_v3(action, 16, None).unwrap().len(),
                COMPLETE_SET_ACCOUNT_COUNT_V3
            );
        }
        for action in [
            CollateralActionV3::Materialize,
            CollateralActionV3::Dematerialize,
        ] {
            assert_eq!(
                account_contract_v3(action, 16, Some(15)).unwrap().len(),
                CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3 + 16
            );
        }
        assert_eq!(
            account_contract_v3(CollateralActionV3::RedeemExternal, 16, Some(15))
                .unwrap()
                .len(),
            EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3 + 16
        );
    }

    #[test]
    fn inferred_validator_owns_fixed_and_variable_counts() {
        let fixed = observed(CollateralActionV3::Endow, 2, None);
        assert_eq!(
            validate_inferred_collateral_account_metas_with_v3(
                CollateralActionV3::Endow,
                None,
                fixed.len(),
                |index| fixed.get(index).copied(),
            ),
            Ok(1)
        );

        let variable = observed(CollateralActionV3::Materialize, 2, Some(1));
        assert_eq!(
            validate_inferred_collateral_account_metas_with_v3(
                CollateralActionV3::Materialize,
                Some(1),
                variable.len(),
                |index| variable.get(index).copied(),
            ),
            Ok(2)
        );
        assert_eq!(
            validate_inferred_collateral_account_metas_with_v3(
                CollateralActionV3::Materialize,
                Some(1),
                CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3,
                |_| None,
            ),
            Err(CodecError::InvalidCount)
        );
    }

    #[test]
    fn exported_indices_are_the_contracts_semantic_order() {
        let cash = account_contract_v3(CollateralActionV3::Endow, 2, None).unwrap();
        assert_eq!(
            cash.meta(collateral_cash_indices_v3::MARKET_INSTANCE)
                .unwrap()
                .role,
            CollateralAccountRoleV3::MarketInstanceArtifact
        );
        assert_eq!(
            cash.meta(collateral_cash_indices_v3::DESTINATION)
                .unwrap()
                .role,
            CollateralAccountRoleV3::CollateralSource
        );
        assert_eq!(
            cash.meta(collateral_cash_indices_v3::RENT).unwrap().role,
            CollateralAccountRoleV3::RentSysvar
        );

        let complete = account_contract_v3(CollateralActionV3::Merge, 2, None).unwrap();
        assert_eq!(
            complete
                .meta(complete_set_indices_v3::CLAIM_LEDGER)
                .unwrap()
                .role,
            CollateralAccountRoleV3::ClaimLedger
        );
        assert_eq!(
            complete
                .meta(complete_set_indices_v3::HOARD_TOKEN)
                .unwrap()
                .role,
            CollateralAccountRoleV3::HoardToken
        );

        let claim = account_contract_v3(CollateralActionV3::Materialize, 2, Some(1)).unwrap();
        assert_eq!(
            claim
                .meta(claim_representation_indices_v3::OUTCOME_TOKEN_PROGRAM)
                .unwrap()
                .role,
            CollateralAccountRoleV3::OutcomeTokenProgram
        );
        assert_eq!(
            claim
                .meta(claim_representation_indices_v3::OUTCOME_TOKEN_PROGRAMDATA)
                .unwrap()
                .role,
            CollateralAccountRoleV3::OutcomeTokenProgramData
        );
        assert_eq!(
            claim
                .meta(claim_representation_indices_v3::OUTCOME_MINTS + 1)
                .unwrap()
                .role,
            CollateralAccountRoleV3::OutcomeMint(1)
        );

        let external = account_contract_v3(CollateralActionV3::RedeemExternal, 2, Some(1)).unwrap();
        assert_eq!(
            external
                .meta(external_redemption_indices_v3::RESOLUTION)
                .unwrap()
                .role,
            CollateralAccountRoleV3::ResolutionV5
        );
        assert_eq!(
            external
                .meta(external_redemption_indices_v3::SOURCE)
                .unwrap()
                .role,
            CollateralAccountRoleV3::ExternalClaimSource
        );
        assert_eq!(
            external
                .meta(external_redemption_indices_v3::OUTCOME_TOKEN_PROGRAMDATA)
                .unwrap()
                .role,
            CollateralAccountRoleV3::OutcomeTokenProgramData
        );
    }

    #[test]
    fn widths_and_selection_are_canonical() {
        assert_eq!(
            account_contract_v3(CollateralActionV3::Endow, 0, None),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            account_contract_v3(
                CollateralActionV3::Endow,
                u8::try_from(MAX_OUTCOMES + 1).unwrap(),
                None,
            ),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            account_contract_v3(CollateralActionV3::Split, 2, Some(0)),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            account_contract_v3(CollateralActionV3::Materialize, 2, None),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            account_contract_v3(CollateralActionV3::Dematerialize, 2, Some(2)),
            Err(CodecError::InvalidCount)
        );
    }

    #[test]
    fn selected_mint_is_the_only_writable_mint_at_minimum_and_maximum_width() {
        for (count, selected) in [(1_u8, 0_u8), (u8::try_from(MAX_OUTCOMES).unwrap(), 7_u8)] {
            for action in [
                CollateralActionV3::Materialize,
                CollateralActionV3::Dematerialize,
                CollateralActionV3::RedeemExternal,
            ] {
                let contract = account_contract_v3(action, count, Some(selected)).unwrap();
                for outcome in 0..count {
                    let requirement = contract
                        .meta(role_index(
                            contract,
                            CollateralAccountRoleV3::OutcomeMint(outcome),
                        ))
                        .unwrap();
                    assert_eq!(requirement.writable, outcome == selected);
                    assert!(!requirement.signer);
                }
            }
        }
    }

    #[test]
    fn exact_privilege_contracts_admit_their_own_observations() {
        let cases = [
            (CollateralActionV3::Endow, None),
            (CollateralActionV3::WithdrawCash, None),
            (CollateralActionV3::Split, None),
            (CollateralActionV3::Merge, None),
            (CollateralActionV3::Materialize, Some(1)),
            (CollateralActionV3::Dematerialize, Some(1)),
            (CollateralActionV3::RedeemExternal, Some(1)),
        ];
        for (action, selected) in cases {
            let accounts = observed(action, 2, selected);
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, selected, &accounts),
                Ok(())
            );

            let programdata =
                role_index(contract, CollateralAccountRoleV3::OutcomeTokenProgramData);
            let mut deployment_alias = observed(action, 2, selected);
            deployment_alias[programdata].key = deployment_alias[outcome_program].key;
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, selected, &deployment_alias),
                Err(CodecError::MismatchedBinding)
            );
        }
    }

    #[test]
    fn only_signed_actor_may_inherit_writable_fee_payer_privilege() {
        for action in [
            CollateralActionV3::WithdrawCash,
            CollateralActionV3::Split,
            CollateralActionV3::Merge,
            CollateralActionV3::Materialize,
            CollateralActionV3::Dematerialize,
        ] {
            let selected = action.has_outcome_mint_suffix().then_some(0);
            let mut accounts = observed(action, 2, selected);
            accounts[0].writable = true;
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, selected, &accounts),
                Ok(())
            );
        }

        let mut external = observed(CollateralActionV3::RedeemExternal, 2, Some(0));
        external[0].writable = true;
        assert_eq!(
            validate_collateral_account_metas_v3(
                CollateralActionV3::RedeemExternal,
                2,
                Some(0),
                &external,
            ),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn exact_count_zero_keys_and_privilege_changes_are_refused() {
        let action = CollateralActionV3::Materialize;
        let selected = Some(1);
        let accounts = observed(action, 2, selected);
        assert_eq!(
            validate_collateral_account_metas_v3(
                action,
                2,
                selected,
                &accounts[..accounts.len() - 1]
            ),
            Err(CodecError::Truncated)
        );
        let mut trailing = accounts.clone();
        trailing.push(ObservedCollateralAccountMetaV3 {
            key: [0xFE; HASH_BYTES],
            writable: false,
            signer: false,
        });
        assert_eq!(
            validate_collateral_account_metas_v3(action, 2, selected, &trailing),
            Err(CodecError::TrailingBytes)
        );

        let mut zero = accounts.clone();
        zero[5].key = [0; HASH_BYTES];
        assert_eq!(
            validate_collateral_account_metas_v3(action, 2, selected, &zero),
            Err(CodecError::ZeroIdentity)
        );

        for mutate in [
            |meta: &mut ObservedCollateralAccountMetaV3| meta.writable = !meta.writable,
            |meta: &mut ObservedCollateralAccountMetaV3| meta.signer = !meta.signer,
        ] {
            let mut changed = accounts.clone();
            mutate(&mut changed[5]);
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, selected, &changed),
                Err(CodecError::MismatchedBinding)
            );
        }
    }

    #[test]
    fn only_the_two_token_program_roles_may_alias_on_three_claim_routes() {
        for action in [
            CollateralActionV3::Materialize,
            CollateralActionV3::Dematerialize,
            CollateralActionV3::RedeemExternal,
        ] {
            let selected = Some(0);
            let contract = account_contract_v3(action, 2, selected).unwrap();
            let collateral_program =
                role_index(contract, CollateralAccountRoleV3::CollateralTokenProgram);
            let outcome_program =
                role_index(contract, CollateralAccountRoleV3::OutcomeTokenProgram);
            let mut accounts = observed(action, 2, selected);
            accounts[outcome_program].key = accounts[collateral_program].key;
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, selected, &accounts),
                Ok(())
            );

            let mut forbidden = observed(action, 2, selected);
            forbidden[1].key = forbidden[0].key;
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, selected, &forbidden),
                Err(CodecError::MismatchedBinding)
            );

            let mut mint_alias = observed(action, 2, selected);
            let first_mint = contract.len() - usize::from(contract.outcome_count());
            mint_alias[first_mint + 1].key = mint_alias[first_mint].key;
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, selected, &mint_alias),
                Err(CodecError::MismatchedBinding)
            );
        }
    }

    #[test]
    fn non_claim_routes_have_no_alias_exception() {
        for action in [
            CollateralActionV3::Endow,
            CollateralActionV3::WithdrawCash,
            CollateralActionV3::Split,
            CollateralActionV3::Merge,
        ] {
            let mut accounts = observed(action, 2, None);
            accounts[2].key = accounts[1].key;
            assert_eq!(
                validate_collateral_account_metas_v3(action, 2, None, &accounts),
                Err(CodecError::MismatchedBinding)
            );
        }
    }
}
