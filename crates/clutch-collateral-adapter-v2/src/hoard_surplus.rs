// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact beneficiary-free Hoard disposition after a protocol family destroys
//! its own cash and native-claim liabilities.
//!
//! This is deliberately distinct from Series segregated-vault donation and
//! claim redemption. The caller's family remains the semantic owner of the
//! transition identity and of the reason liabilities are destroyed. This
//! module only derives the unique Hoard/ClaimLedger successors and admits the
//! exact Realm-selected Hoard-to-neutral token movement.

use clutch_retirement::MAX_OUTCOMES;

use crate::{
    accept_collateral_transfer_v2, admit_collateral_account_v2, admit_collateral_mint_v2, digest,
    prepare_collateral_transfer_v2, AcceptedCollateralTransferV2, BoundCollateralProfileV2,
    CheckedTransferCpiV2, ClaimLedgerV3, CustodyTransferKindV2, Error, HoardV2, Id,
    MintObservationV2, PreparedCollateralTransferV2, Result, RuntimeAccountViewV2,
    TokenAccountObservationV2, TokenAccountRoleV2, TransferAuthorityKindV2, TransferAuthorityV2,
    TransferEndpointV2, TransferRequestV2, CLAIM_LEDGER_V3_BYTES,
    CLAIM_LEDGER_V3_SEMANTIC_DOMAIN, HOARD_V2_BYTES, HOARD_V2_SEMANTIC_DOMAIN,
};

/// Accepted beneficiary-free liability and collateral disposition domain.
pub const HOARD_SURPLUS_DISPOSITION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/hoard-surplus-disposition/v1\0";

/// Exact family-owned facts consumed by the collateral boundary.
///
/// This value is not authority. A live adapter must construct it only from a
/// private family transition capability and must compare
/// `collateral_value_receipt_id` with the same-instruction Profile-selected
/// ProgramData/ELF authority before invoking the returned CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoardSurplusDispositionRequestV1 {
    /// Exact semantic transition which destroyed the liabilities.
    pub transition_id: Id,
    /// Exact same-instruction collateral deployment/value authority receipt.
    pub collateral_value_receipt_id: Id,
    /// Realm-authenticated receive-only neutral collateral account.
    pub destination_token_account: Id,
    /// Exact Product/Series artifact owning the neutral endpoint selection.
    pub destination_semantic_owner: Id,
    /// Cash liability destroyed and therefore transferred out of the Hoard.
    pub donated_cash_atoms: u64,
    /// Internal native claims destroyed by active outcome.
    pub donated_internal: [u64; MAX_OUTCOMES],
    /// Exact hostile-decoded Hoard prestate.
    pub hoard_before: HoardV2,
    /// Exact family-proposed Hoard successor.
    pub hoard_after: HoardV2,
    /// Exact hostile-decoded ClaimLedger prestate.
    pub claim_ledger_before: ClaimLedgerV3,
    /// Exact family-proposed ClaimLedger successor.
    pub claim_ledger_after: ClaimLedgerV3,
}

/// Prepared liability successors and optional exact collateral CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedHoardSurplusDispositionV1 {
    bound: BoundCollateralProfileV2,
    request: HoardSurplusDispositionRequestV1,
    movement: Option<PreparedCollateralTransferV2>,
    mint_before: MintObservationV2,
    source_before: TokenAccountObservationV2,
    destination_before: TokenAccountObservationV2,
    hoard_before_id: Id,
    hoard_after_id: Id,
    claim_ledger_before_id: Id,
    claim_ledger_after_id: Id,
}

impl PreparedHoardSurplusDispositionV1 {
    /// Sole collateral invocation permitted by this transition. Egg-only
    /// disposition returns `None` and must perform no token CPI.
    pub const fn cpi(self) -> Option<CheckedTransferCpiV2> {
        match self.movement {
            Some(prepared) => Some(prepared.cpi()),
            None => None,
        }
    }

    /// Exact derived Hoard successor.
    pub const fn hoard_after(self) -> HoardV2 {
        self.request.hoard_after
    }

    /// Exact derived ClaimLedger successor.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.request.claim_ledger_after
    }
}

/// Accepted exact token reload and liability successors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedHoardSurplusDispositionV1 {
    request: HoardSurplusDispositionRequestV1,
    custody: Option<AcceptedCollateralTransferV2>,
    hoard_before_id: Id,
    hoard_after_id: Id,
    claim_ledger_before_id: Id,
    claim_ledger_after_id: Id,
    source_atoms_before: u64,
    source_atoms_after: u64,
    destination_atoms_before: u64,
    destination_atoms_after: u64,
    mint_supply_atoms: u64,
    receipt_id: Id,
}

impl AcceptedHoardSurplusDispositionV1 {
    /// Exact family transition bound by this collateral result.
    pub const fn transition_id(self) -> Id {
        self.request.transition_id
    }

    /// Same-instruction Profile-selected ProgramData/ELF authority receipt.
    pub const fn collateral_value_receipt_id(self) -> Id {
        self.request.collateral_value_receipt_id
    }

    /// Exact raw collateral debit, zero for Egg-only disposition.
    pub const fn donated_cash_atoms(self) -> u64 {
        self.request.donated_cash_atoms
    }

    /// Exact internal native-claim destruction vector.
    pub const fn donated_internal(self) -> [u64; MAX_OUTCOMES] {
        self.request.donated_internal
    }

    /// Exact Realm-authenticated neutral collateral token account.
    pub const fn destination_token_account(self) -> Id {
        self.request.destination_token_account
    }

    /// Product/Series artifact owning the neutral endpoint selection.
    pub const fn destination_semantic_owner(self) -> Id {
        self.request.destination_semantic_owner
    }

    /// Derived Hoard successor.
    pub const fn hoard_after(self) -> HoardV2 {
        self.request.hoard_after
    }

    /// Derived ClaimLedger successor.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.request.claim_ledger_after
    }

    /// Hoard semantic identity before the disposition.
    pub const fn hoard_before_id(self) -> Id {
        self.hoard_before_id
    }

    /// Hoard semantic identity after the disposition.
    pub const fn hoard_after_id(self) -> Id {
        self.hoard_after_id
    }

    /// ClaimLedger semantic identity before the disposition.
    pub const fn claim_ledger_before_id(self) -> Id {
        self.claim_ledger_before_id
    }

    /// ClaimLedger semantic identity after the disposition.
    pub const fn claim_ledger_after_id(self) -> Id {
        self.claim_ledger_after_id
    }

    /// Exact accepted token movement, absent only for a zero-cash disposition.
    pub const fn custody(self) -> Option<AcceptedCollateralTransferV2> {
        self.custody
    }

    /// Exact Hoard visible amount before the optional CPI.
    pub const fn source_atoms_before(self) -> u64 {
        self.source_atoms_before
    }

    /// Exact Hoard visible amount after the optional CPI.
    pub const fn source_atoms_after(self) -> u64 {
        self.source_atoms_after
    }

    /// Exact neutral destination visible amount before the optional CPI.
    pub const fn destination_atoms_before(self) -> u64 {
        self.destination_atoms_before
    }

    /// Exact neutral destination visible amount after the optional CPI.
    pub const fn destination_atoms_after(self) -> u64 {
        self.destination_atoms_after
    }

    /// Unchanged admitted collateral mint supply.
    pub const fn mint_supply_atoms(self) -> u64 {
        self.mint_supply_atoms
    }

    /// Receipt committing the authority, liabilities, release, and exact token deltas.
    pub const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

fn hoard_semantic_id(value: HoardV2) -> Result<Id> {
    let mut body = [0u8; HOARD_V2_BYTES];
    value.encode(&mut body)?;
    let id = digest(HOARD_V2_SEMANTIC_DOMAIN, &[&body]);
    id.require_live()?;
    Ok(id)
}

fn claim_ledger_semantic_id(value: ClaimLedgerV3) -> Result<Id> {
    let mut body = [0u8; CLAIM_LEDGER_V3_BYTES];
    value.encode(&mut body)?;
    let id = digest(CLAIM_LEDGER_V3_SEMANTIC_DOMAIN, &[&body]);
    id.require_live()?;
    Ok(id)
}

fn derive_liability_successors(
    request: HoardSurplusDispositionRequestV1,
) -> Result<(HoardV2, ClaimLedgerV3)> {
    request.hoard_before.validate()?;
    request.claim_ledger_before.validate()?;
    if request.hoard_before.market_instance_id != request.claim_ledger_before.market_instance_id
        || request.hoard_before.realm_id != request.claim_ledger_before.realm_id
        || request.hoard_before.lifecycle != request.claim_ledger_before.lifecycle
        || request.hoard_before.outcome_count != request.claim_ledger_before.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let width = usize::from(request.hoard_before.outcome_count);
    let mut aggregate_internal_supply = request.claim_ledger_before.aggregate_internal_supply;
    let mut any = request.donated_cash_atoms != 0;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        if index < width {
            aggregate_internal_supply[index] = aggregate_internal_supply[index]
                .checked_sub(request.donated_internal[index])
                .ok_or(Error::AggregateLiabilityInsufficient)?;
            any |= request.donated_internal[index] != 0;
        } else if request.donated_internal[index] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if !any {
        return Err(Error::InvalidParameter);
    }
    let hoard_after = HoardV2 {
        cash_liability_atoms: request
            .hoard_before
            .cash_liability_atoms
            .checked_sub(request.donated_cash_atoms)
            .ok_or(Error::AggregateLiabilityInsufficient)?,
        ..request.hoard_before
    };
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_internal_supply,
        ..request.claim_ledger_before
    };
    hoard_after.validate()?;
    claim_ledger_after.validate()?;
    Ok((hoard_after, claim_ledger_after))
}

fn require_observation_identity_unchanged(
    before: TokenAccountObservationV2,
    after: TokenAccountObservationV2,
) -> Result<()> {
    if before.address != after.address
        || before.mint != after.mint
        || before.owner_authority != after.owner_authority
        || before.extensions != after.extensions
        || before.semantic_owner != after.semantic_owner
        || before.compartment != after.compartment
    {
        return Err(Error::PostAdmissionFailed);
    }
    Ok(())
}

fn flatten_internal(values: [u64; MAX_OUTCOMES]) -> Result<[u8; 8 * MAX_OUTCOMES]> {
    let mut output = [0u8; 8 * MAX_OUTCOMES];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        let start = index.checked_mul(8).ok_or(Error::Arithmetic)?;
        let end = start.checked_add(8).ok_or(Error::Arithmetic)?;
        output[start..end].copy_from_slice(&values[index].to_le_bytes());
        index += 1;
    }
    Ok(output)
}

/// Prepare the unique liability successors and exact optional Hoard CPI.
///
/// A nonzero cash donation emits one release-selected checked transfer. An
/// Egg-only donation still authenticates all three token accounts and requires
/// them to reload unchanged, but emits no invocation.
#[allow(clippy::too_many_arguments)]
pub fn prepare_hoard_surplus_disposition_v1(
    bound: BoundCollateralProfileV2,
    request: HoardSurplusDispositionRequestV1,
    authority: TransferAuthorityV2,
    mint: RuntimeAccountViewV2<'_>,
    hoard_source: RuntimeAccountViewV2<'_>,
    neutral_destination: RuntimeAccountViewV2<'_>,
) -> Result<PreparedHoardSurplusDispositionV1> {
    for id in [
        request.transition_id,
        request.collateral_value_receipt_id,
        request.destination_token_account,
        request.destination_semantic_owner,
    ] {
        id.require_live()?;
    }
    let market = bound.market();
    let realm = bound.realm_bound().realm();
    let release_id = bound.release().id()?;
    if request.hoard_before.market_instance_id != market.market
        || request.hoard_before.realm_id != realm.realm
        || request.hoard_before.profile_id != realm.profile
        || request.hoard_before.collateral_policy_id != bound.policy_id()
        || request.hoard_before.collateral_release_id != release_id
        || request.hoard_before.authority != market.hoard_authority
        || request.hoard_before.token_account != market.hoard_token_account
        || request.claim_ledger_before.market_instance_id != market.market
        || request.claim_ledger_before.realm_id != realm.realm
        || request.destination_token_account == market.hoard_token_account
        || request.destination_token_account == bound.policy().mint
        || authority.address != market.hoard_authority
        || authority.kind != TransferAuthorityKindV2::ProgramDerived
    {
        return Err(Error::MismatchedBinding);
    }
    authority.validate()?;
    let (expected_hoard_after, expected_claim_ledger_after) =
        derive_liability_successors(request)?;
    if request.hoard_after != expected_hoard_after
        || request.claim_ledger_after != expected_claim_ledger_after
    {
        return Err(Error::MismatchedBinding);
    }
    if mint.is_writable
        || !hoard_source.is_writable
        || !neutral_destination.is_writable
        || hoard_source.key == neutral_destination.key
        || hoard_source.key == mint.key
        || neutral_destination.key == mint.key
        || hoard_source.key != market.hoard_token_account
        || neutral_destination.key != request.destination_token_account
    {
        return Err(Error::WrongAccountRole);
    }
    let mint_before = admit_collateral_mint_v2(bound, mint)?;
    let source_before =
        admit_collateral_account_v2(bound, hoard_source, TokenAccountRoleV2::Hoard)?;
    let destination_before = admit_collateral_account_v2(
        bound,
        neutral_destination,
        TokenAccountRoleV2::ReceiveOnly {
            account: request.destination_token_account,
        },
    )?;
    let required_before = request.hoard_before.required_custody_atoms()?;
    let required_after = request.hoard_after.required_custody_atoms()?;
    let visible_after = source_before
        .amount_atoms
        .checked_sub(request.donated_cash_atoms)
        .ok_or(Error::HoardCoverageMismatch)?;
    if source_before.amount_atoms < required_before || visible_after < required_after {
        return Err(Error::HoardCoverageMismatch);
    }
    let movement = if request.donated_cash_atoms == 0 {
        None
    } else {
        Some(prepare_collateral_transfer_v2(
            bound,
            TransferRequestV2 {
                kind: CustodyTransferKindV2::HoardSurplusDisposition,
                source: TransferEndpointV2 {
                    token_role: TokenAccountRoleV2::Hoard,
                    semantic_owner: market.market,
                    compartment: 1,
                },
                destination: TransferEndpointV2 {
                    token_role: TokenAccountRoleV2::ReceiveOnly {
                        account: request.destination_token_account,
                    },
                    semantic_owner: request.destination_semantic_owner,
                    compartment: 0,
                },
                authority,
                amount_atoms: request.donated_cash_atoms,
                position_cash: None,
                locked_collateral_atoms: required_after,
            },
            mint,
            hoard_source,
            neutral_destination,
        )?)
    };
    Ok(PreparedHoardSurplusDispositionV1 {
        bound,
        request,
        movement,
        mint_before,
        source_before,
        destination_before,
        hoard_before_id: hoard_semantic_id(request.hoard_before)?,
        hoard_after_id: hoard_semantic_id(request.hoard_after)?,
        claim_ledger_before_id: claim_ledger_semantic_id(request.claim_ledger_before)?,
        claim_ledger_after_id: claim_ledger_semantic_id(request.claim_ledger_after)?,
    })
}

/// Accept exact hostile postreloads after the optional collateral CPI.
pub fn accept_hoard_surplus_disposition_v1(
    prepared: PreparedHoardSurplusDispositionV1,
    mint_after: RuntimeAccountViewV2<'_>,
    hoard_source_after: RuntimeAccountViewV2<'_>,
    neutral_destination_after: RuntimeAccountViewV2<'_>,
) -> Result<AcceptedHoardSurplusDispositionV1> {
    let request = prepared.request;
    let mint = admit_collateral_mint_v2(prepared.bound, mint_after)
        .map_err(|_| Error::PostAdmissionFailed)?;
    let source = admit_collateral_account_v2(
        prepared.bound,
        hoard_source_after,
        TokenAccountRoleV2::Hoard,
    )
    .map_err(|_| Error::PostAdmissionFailed)?;
    let destination = admit_collateral_account_v2(
        prepared.bound,
        neutral_destination_after,
        TokenAccountRoleV2::ReceiveOnly {
            account: request.destination_token_account,
        },
    )
    .map_err(|_| Error::PostAdmissionFailed)?;
    require_observation_identity_unchanged(prepared.source_before, source)?;
    require_observation_identity_unchanged(prepared.destination_before, destination)?;
    if mint != prepared.mint_before
        || prepared
            .source_before
            .amount_atoms
            .checked_sub(source.amount_atoms)
            != Some(request.donated_cash_atoms)
        || destination
            .amount_atoms
            .checked_sub(prepared.destination_before.amount_atoms)
            != Some(request.donated_cash_atoms)
        || source.amount_atoms < request.hoard_after.required_custody_atoms()?
    {
        return Err(Error::TransferDeltaMismatch);
    }
    let custody = match prepared.movement {
        Some(movement) => {
            let accepted = accept_collateral_transfer_v2(
                movement,
                mint_after,
                hoard_source_after,
                neutral_destination_after,
            )?;
            if accepted.kind != CustodyTransferKindV2::HoardSurplusDisposition
                || accepted.amount_atoms != request.donated_cash_atoms
                || accepted.source_semantic_owner != prepared.bound.market().market
                || accepted.destination_semantic_owner != request.destination_semantic_owner
                || accepted.hoard_atoms_after != Some(source.amount_atoms)
            {
                return Err(Error::TransferDeltaMismatch);
            }
            Some(accepted)
        }
        None => {
            if request.donated_cash_atoms != 0 {
                return Err(Error::TransferDeltaMismatch);
            }
            None
        }
    };
    let internal = flatten_internal(request.donated_internal)?;
    let release = prepared.bound.release();
    let release_id = release.id()?;
    let receipt_id = digest(
        HOARD_SURPLUS_DISPOSITION_RECEIPT_DOMAIN_V1,
        &[
            &request.transition_id.bytes(),
            &request.collateral_value_receipt_id.bytes(),
            &prepared.bound.market().market.bytes(),
            &prepared.bound.realm_bound().realm().realm.bytes(),
            &prepared.bound.policy_id().bytes(),
            &release_id.bytes(),
            &release.token_program.bytes(),
            &release.token_program_deployment.bytes(),
            &prepared.hoard_before_id.bytes(),
            &prepared.hoard_after_id.bytes(),
            &prepared.claim_ledger_before_id.bytes(),
            &prepared.claim_ledger_after_id.bytes(),
            &prepared.bound.market().hoard_token_account.bytes(),
            &request.destination_token_account.bytes(),
            &request.destination_semantic_owner.bytes(),
            &request.donated_cash_atoms.to_le_bytes(),
            &internal,
            &prepared.source_before.amount_atoms.to_le_bytes(),
            &source.amount_atoms.to_le_bytes(),
            &prepared.destination_before.amount_atoms.to_le_bytes(),
            &destination.amount_atoms.to_le_bytes(),
            &mint.supply_atoms.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(AcceptedHoardSurplusDispositionV1 {
        request,
        custody,
        hoard_before_id: prepared.hoard_before_id,
        hoard_after_id: prepared.hoard_after_id,
        claim_ledger_before_id: prepared.claim_ledger_before_id,
        claim_ledger_after_id: prepared.claim_ledger_after_id,
        source_atoms_before: prepared.source_before.amount_atoms,
        source_atoms_after: source.amount_atoms,
        destination_atoms_before: prepared.destination_before.amount_atoms,
        destination_atoms_after: destination.amount_atoms,
        mint_supply_atoms: mint.supply_atoms,
        receipt_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_retirement::{DeletableRentOwnerV1, Identity32V1};

    const fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn rent() -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1::from_persisted(Identity32V1::new([31; 32]).unwrap(), 1, 0)
            .unwrap()
    }

    fn liabilities() -> (HoardV2, ClaimLedgerV3) {
        (
            HoardV2 {
                market_instance_id: id(1),
                realm_id: id(2),
                profile_id: id(3),
                collateral_policy_id: id(4),
                collateral_release_id: id(5),
                authority: id(6),
                token_account: id(7),
                collateral_cap_atoms: 100,
                cash_liability_atoms: 20,
                locked_claim_principal_atoms: 30,
                lifecycle: crate::MarketLiabilityLifecycleV1::Open,
                outcome_count: 2,
                stored_bump: 1,
                rent: rent(),
            },
            ClaimLedgerV3 {
                market_instance_id: id(1),
                realm_id: id(2),
                native_claim_basis_id: id(8),
                fractional_policy_id: Id::ZERO,
                fractional_ledger_account: Id::ZERO,
                resolution_account: Id::ZERO,
                aggregate_internal_supply: [10, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                aggregate_materialized_supply: [3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                next_fractional_sequence: 0,
                last_fractional_transition_id: Id::ZERO,
                fractional_binding: crate::FractionalBindingStateV1::OpenUnlatched,
                lifecycle: crate::MarketLiabilityLifecycleV1::Open,
                outcome_count: 2,
                stored_bump: 2,
                rent: rent(),
            },
        )
    }

    fn request(cash: u64, internal: [u64; MAX_OUTCOMES]) -> HoardSurplusDispositionRequestV1 {
        let (hoard_before, claim_ledger_before) = liabilities();
        let mut hoard_after = hoard_before;
        hoard_after.cash_liability_atoms = hoard_after
            .cash_liability_atoms
            .checked_sub(cash)
            .unwrap();
        let mut claim_ledger_after = claim_ledger_before;
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            claim_ledger_after.aggregate_internal_supply[index] = claim_ledger_after
                .aggregate_internal_supply[index]
                .checked_sub(internal[index])
                .unwrap();
            index += 1;
        }
        HoardSurplusDispositionRequestV1 {
            transition_id: id(20),
            collateral_value_receipt_id: id(21),
            destination_token_account: id(22),
            destination_semantic_owner: id(23),
            donated_cash_atoms: cash,
            donated_internal: internal,
            hoard_before,
            hoard_after,
            claim_ledger_before,
            claim_ledger_after,
        }
    }

    #[test]
    fn cash_disposition_preserves_locked_principal_and_bearer_supply() {
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[0] = 2;
        let request = request(5, internal);
        let (hoard_after, claim_after) = derive_liability_successors(request).unwrap();
        assert_eq!(hoard_after.cash_liability_atoms, 15);
        assert_eq!(hoard_after.locked_claim_principal_atoms, 30);
        assert_eq!(claim_after.aggregate_internal_supply[0], 8);
        assert_eq!(claim_after.aggregate_materialized_supply, request.claim_ledger_before.aggregate_materialized_supply);
    }

    #[test]
    fn egg_only_disposition_changes_no_hoard_liability() {
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[1] = 1;
        let request = request(0, internal);
        let (hoard_after, claim_after) = derive_liability_successors(request).unwrap();
        assert_eq!(hoard_after, request.hoard_before);
        assert_eq!(claim_after.aggregate_internal_supply[1], 10);
    }

    #[test]
    fn inactive_tail_and_wrong_poststate_refuse() {
        let mut tail = [0u64; MAX_OUTCOMES];
        tail[0] = 1;
        let mut tail_request = request(0, tail);
        tail_request.donated_internal[0] = 0;
        tail_request.donated_internal[2] = 1;
        assert_eq!(derive_liability_successors(tail_request), Err(Error::NonCanonicalPadding));

        let mut internal = [0u64; MAX_OUTCOMES];
        internal[0] = 1;
        let mut wrong = request(1, internal);
        wrong.hoard_after.locked_claim_principal_atoms = wrong
            .hoard_after
            .locked_claim_principal_atoms
            .checked_sub(1)
            .unwrap();
        let (expected_hoard, expected_claim) = derive_liability_successors(wrong).unwrap();
        assert_ne!(wrong.hoard_after, expected_hoard);
        assert_eq!(wrong.claim_ledger_after, expected_claim);
    }

    #[test]
    fn zero_and_insufficient_donations_refuse() {
        assert_eq!(
            derive_liability_successors(request(0, [0u64; MAX_OUTCOMES])),
            Err(Error::InvalidParameter)
        );
        let mut excessive = [0u64; MAX_OUTCOMES];
        excessive[0] = 1;
        let mut excessive_request = request(0, excessive);
        excessive_request.donated_internal[0] = 11;
        assert_eq!(
            derive_liability_successors(excessive_request),
            Err(Error::AggregateLiabilityInsufficient)
        );
    }
}
