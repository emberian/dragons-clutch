// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical Position↔bearer claim representation transitions.
//!
//! Collateral custody does not move when an internal native Egg is
//! materialized as a Token-2022 bearer claim or burned back into a Position.
//! This module therefore consumes the independently bound claim-issuance
//! release, never a Realm collateral transfer capability. Runtime adapters
//! must authenticate the mint and holder bytes, execute the exact mint/burn,
//! reload every active mint, and only then publish the returned Position and
//! ClaimLedger successors.

use clutch_retirement::{
    GeneralPositionProjectionV3, PositionAccountV3, PositionLifecycleV3, PositionV3Fields,
    PositionV3Sha256Backend, MAX_OUTCOMES,
};

use crate::{
    digest, BoundClaimIssuanceV1, ClaimLedgerV3, Error, Id, MarketLiabilityLifecycleV1, Result,
};

/// Domain for the semantic Position/ClaimLedger representation transition.
pub const CLAIM_REPRESENTATION_TRANSITION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/claim-representation/transition/v3\0";
/// Domain for the exact accepted Token-2022 postcondition receipt.
pub const CLAIM_REPRESENTATION_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/claim-representation/receipt/v3\0";

/// Direction of one internal↔bearer claim representation change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClaimRepresentationKindV3 {
    /// Burn Position-owned native Eggs and mint equal bearer atoms.
    Materialize = 1,
    /// Burn bearer atoms and credit equal Position-owned native Eggs.
    Dematerialize = 2,
}

/// Runtime-authenticated selected mint and holder observation.
///
/// This is deliberately a forgeable adapter projection. The pure contract
/// cannot authenticate Token-2022 account ownership, parsers, PDA seeds,
/// signer privilege, or extension bytes; the SBF adapter must derive every
/// field from accounts admitted under [`BoundClaimIssuanceV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterClaimRepresentationObservationV3 {
    /// Exact selected outcome mint.
    pub mint: Id,
    /// Mint authority retained in the selected mint bytes.
    pub mint_authority: Id,
    /// Exact holder token account.
    pub holder_token_account: Id,
    /// Owner authority retained in the holder token bytes.
    pub holder_owner: Id,
    /// Selected mint supply.
    pub mint_supply_atoms: u64,
    /// Holder balance in selected claim atoms.
    pub holder_atoms: u64,
}

/// Complete mint/burn intent authorized by a prepared semantic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimIssuanceIntentV3 {
    /// Mint for one exact outcome.
    pub mint: Id,
    /// Holder token account credited or debited.
    pub holder_token_account: Id,
    /// Mint authority for materialization or holder signer for dematerialization.
    pub authority: Id,
    /// Exact raw claim atoms.
    pub amount_atoms: u64,
    /// Whether the claim program must mint (`true`) or burn (`false`).
    pub minting: bool,
    /// Whether the authority must be signed by the Dragon's Clutch program.
    pub program_signed: bool,
}

/// Prepared semantic successor and exact Token-2022 pre/post contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedClaimRepresentationV3 {
    claim_binding_id: Id,
    kind: ClaimRepresentationKindV3,
    position_account: Id,
    position_before_id: Id,
    position_after: PositionAccountV3,
    position_after_id: Id,
    claim_ledger_before_id: Id,
    claim_ledger_after: ClaimLedgerV3,
    claim_ledger_after_id: Id,
    outcome: u8,
    amount_atoms: u64,
    observed_materialized_before: [u64; MAX_OUTCOMES],
    expected_materialized_after: [u64; MAX_OUTCOMES],
    token_before: AdapterClaimRepresentationObservationV3,
    intent: ClaimIssuanceIntentV3,
    transition_id: Id,
}

impl PreparedClaimRepresentationV3 {
    /// Sole independently selected claim mint/burn this transition permits.
    pub const fn issuance_intent(self) -> ClaimIssuanceIntentV3 {
        self.intent
    }

    /// Semantic transition identity to bind into the purpose Replay.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }
}

/// Accepted exact Token-2022 delta plus both canonical semantic successors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedClaimRepresentationV3 {
    prepared: PreparedClaimRepresentationV3,
    token_after: AdapterClaimRepresentationObservationV3,
    receipt_id: Id,
}

impl AcceptedClaimRepresentationV3 {
    /// Representation direction.
    pub const fn kind(self) -> ClaimRepresentationKindV3 {
        self.prepared.kind
    }

    /// Exact Position account being changed.
    pub const fn position_account(self) -> Id {
        self.prepared.position_account
    }

    /// Position semantic ID before the transition.
    pub const fn position_before_id(self) -> Id {
        self.prepared.position_before_id
    }

    /// Complete canonical Position successor.
    pub const fn position_after(self) -> PositionAccountV3 {
        self.prepared.position_after
    }

    /// Position semantic ID after the transition.
    pub const fn position_after_id(self) -> Id {
        self.prepared.position_after_id
    }

    /// ClaimLedger semantic ID before direct-burn synchronization and transition.
    pub const fn claim_ledger_before_id(self) -> Id {
        self.prepared.claim_ledger_before_id
    }

    /// Complete canonical ClaimLedger successor.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.prepared.claim_ledger_after
    }

    /// ClaimLedger semantic ID after the transition.
    pub const fn claim_ledger_after_id(self) -> Id {
        self.prepared.claim_ledger_after_id
    }

    /// Exact transition identity committed by GEN1 Replay.
    pub const fn transition_id(self) -> Id {
        self.prepared.transition_id
    }

    /// Exact accepted Token-2022 evidence identity committed by GEN1 Replay.
    pub const fn receipt_id(self) -> Id {
        self.receipt_id
    }

    /// Exact selected mint/holder observation after CPI.
    pub const fn token_after(self) -> AdapterClaimRepresentationObservationV3 {
        self.token_after
    }
}

/// Prepare one Position↔bearer transition after authenticating every active
/// outcome-mint supply. Lower observed supplies safely recognize direct holder
/// burns; any increase above the canonical ClaimLedger refuses.
#[allow(clippy::too_many_arguments)]
pub fn prepare_claim_representation_v3<B: PositionV3Sha256Backend>(
    claim: BoundClaimIssuanceV1,
    position_account: Id,
    position: GeneralPositionProjectionV3,
    claim_ledger: ClaimLedgerV3,
    kind: ClaimRepresentationKindV3,
    outcome: u8,
    amount_atoms: u64,
    observed_materialized_supply: [u64; MAX_OUTCOMES],
    token_before: AdapterClaimRepresentationObservationV3,
    authority: Id,
    backend: &B,
) -> Result<PreparedClaimRepresentationV3> {
    position_account.require_live()?;
    authority.require_live()?;
    claim_ledger.validate()?;
    claim.binding().validate()?;
    let position_before = position.position();
    position_before
        .validate()
        .map_err(|_| Error::MismatchedBinding)?;
    let fields = position_before.fields();
    let owner = Id::from_bytes(fields.owner.bytes());
    if amount_atoms == 0
        || outcome >= fields.outcome_count
        || fields.outcome_count != claim_ledger.outcome_count
        || Id::from_bytes(fields.market_instance_id.bytes()) != claim_ledger.market_instance_id
        || Id::from_bytes(fields.realm_id.bytes()) != claim_ledger.realm_id
        || position_before.lifecycle() != PositionLifecycleV3::Open
        || claim_ledger.lifecycle == MarketLiabilityLifecycleV1::Retiring
        || token_before.holder_owner != owner
        || token_before.mint.is_zero()
        || token_before.mint_authority.is_zero()
        || token_before.holder_token_account.is_zero()
        || token_before.mint == token_before.holder_token_account
        || token_before.holder_token_account == position_account
        || token_before.mint_supply_atoms != observed_materialized_supply[usize::from(outcome)]
    {
        return Err(Error::MismatchedBinding);
    }
    match kind {
        ClaimRepresentationKindV3::Materialize if token_before.mint_authority != authority => {
            return Err(Error::WrongAccountRole);
        }
        ClaimRepresentationKindV3::Dematerialize if authority != owner => {
            return Err(Error::WrongAccountRole);
        }
        _ => {}
    }

    let mut synchronized_materialized = observed_materialized_supply;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        if index < usize::from(claim_ledger.outcome_count) {
            if fields.native_eggs[index] > claim_ledger.aggregate_internal_supply[index]
                || synchronized_materialized[index]
                    > claim_ledger.aggregate_materialized_supply[index]
            {
                return Err(Error::AggregateLiabilityInsufficient);
            }
        } else if synchronized_materialized[index] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }

    let selected = usize::from(outcome);
    let mut next_fields: PositionV3Fields = fields;
    let mut aggregate_internal_supply = claim_ledger.aggregate_internal_supply;
    match kind {
        ClaimRepresentationKindV3::Materialize => {
            next_fields.native_eggs[selected] = next_fields.native_eggs[selected]
                .checked_sub(amount_atoms)
                .ok_or(Error::AggregateLiabilityInsufficient)?;
            aggregate_internal_supply[selected] = aggregate_internal_supply[selected]
                .checked_sub(amount_atoms)
                .ok_or(Error::AggregateLiabilityInsufficient)?;
            synchronized_materialized[selected] = synchronized_materialized[selected]
                .checked_add(amount_atoms)
                .ok_or(Error::Arithmetic)?;
        }
        ClaimRepresentationKindV3::Dematerialize => {
            if token_before.holder_atoms < amount_atoms {
                return Err(Error::AggregateLiabilityInsufficient);
            }
            next_fields.native_eggs[selected] = next_fields.native_eggs[selected]
                .checked_add(amount_atoms)
                .ok_or(Error::Arithmetic)?;
            aggregate_internal_supply[selected] = aggregate_internal_supply[selected]
                .checked_add(amount_atoms)
                .ok_or(Error::Arithmetic)?;
            synchronized_materialized[selected] = synchronized_materialized[selected]
                .checked_sub(amount_atoms)
                .ok_or(Error::AggregateLiabilityInsufficient)?;
        }
    }
    let position_after =
        PositionAccountV3::new(next_fields).map_err(|_| Error::MismatchedBinding)?;
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_internal_supply,
        aggregate_materialized_supply: synchronized_materialized,
        ..claim_ledger
    };
    claim_ledger_after.validate()?;
    let position_before_id = Id::from_bytes(
        position_before
            .semantic_id(backend)
            .map_err(|_| Error::MismatchedBinding)?
            .bytes(),
    );
    let position_after_id = Id::from_bytes(
        position_after
            .semantic_id(backend)
            .map_err(|_| Error::MismatchedBinding)?
            .bytes(),
    );
    let claim_ledger_before_id = claim_ledger.semantic_id(backend)?;
    let claim_ledger_after_id = claim_ledger_after.semantic_id(backend)?;
    let kind_byte = [kind as u8];
    let transition_id = digest(
        CLAIM_REPRESENTATION_TRANSITION_DOMAIN_V3,
        &[
            &kind_byte,
            &claim.binding_id().bytes(),
            &claim_ledger.market_instance_id.bytes(),
            &position_account.bytes(),
            &position_before_id.bytes(),
            &position_after_id.bytes(),
            &claim_ledger_before_id.bytes(),
            &claim_ledger_after_id.bytes(),
            &token_before.mint.bytes(),
            &token_before.holder_token_account.bytes(),
            &authority.bytes(),
            &[outcome],
            &amount_atoms.to_le_bytes(),
            &token_before.mint_supply_atoms.to_le_bytes(),
            &token_before.holder_atoms.to_le_bytes(),
        ],
    );
    transition_id.require_live()?;
    let intent = ClaimIssuanceIntentV3 {
        mint: token_before.mint,
        holder_token_account: token_before.holder_token_account,
        authority,
        amount_atoms,
        minting: kind == ClaimRepresentationKindV3::Materialize,
        program_signed: kind == ClaimRepresentationKindV3::Materialize,
    };
    Ok(PreparedClaimRepresentationV3 {
        claim_binding_id: claim.binding_id(),
        kind,
        position_account,
        position_before_id,
        position_after,
        position_after_id,
        claim_ledger_before_id,
        claim_ledger_after,
        claim_ledger_after_id,
        outcome,
        amount_atoms,
        observed_materialized_before: observed_materialized_supply,
        expected_materialized_after: synchronized_materialized,
        token_before,
        intent,
        transition_id,
    })
}

/// Accept only the exact selected mint and holder deltas after the claim CPI,
/// while requiring every nonselected active mint to remain unchanged.
pub fn accept_claim_representation_v3(
    prepared: PreparedClaimRepresentationV3,
    observed_materialized_after: [u64; MAX_OUTCOMES],
    token_after: AdapterClaimRepresentationObservationV3,
) -> Result<AcceptedClaimRepresentationV3> {
    if observed_materialized_after != prepared.expected_materialized_after
        || token_after.mint != prepared.token_before.mint
        || token_after.mint_authority != prepared.token_before.mint_authority
        || token_after.holder_token_account != prepared.token_before.holder_token_account
        || token_after.holder_owner != prepared.token_before.holder_owner
        || token_after.mint_supply_atoms
            != observed_materialized_after[usize::from(prepared.outcome)]
    {
        return Err(Error::PostAdmissionFailed);
    }
    let expected_holder = match prepared.kind {
        ClaimRepresentationKindV3::Materialize => prepared
            .token_before
            .holder_atoms
            .checked_add(prepared.amount_atoms)
            .ok_or(Error::Arithmetic)?,
        ClaimRepresentationKindV3::Dematerialize => prepared
            .token_before
            .holder_atoms
            .checked_sub(prepared.amount_atoms)
            .ok_or(Error::PostAdmissionFailed)?,
    };
    if token_after.holder_atoms != expected_holder {
        return Err(Error::PostAdmissionFailed);
    }
    let receipt_id = digest(
        CLAIM_REPRESENTATION_RECEIPT_DOMAIN_V3,
        &[
            &prepared.transition_id.bytes(),
            &prepared.claim_binding_id.bytes(),
            &prepared.position_before_id.bytes(),
            &prepared.position_after_id.bytes(),
            &prepared.claim_ledger_before_id.bytes(),
            &prepared.claim_ledger_after_id.bytes(),
            &prepared.observed_materialized_before[usize::from(prepared.outcome)].to_le_bytes(),
            &token_after.mint_supply_atoms.to_le_bytes(),
            &prepared.token_before.holder_atoms.to_le_bytes(),
            &token_after.holder_atoms.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(AcceptedClaimRepresentationV3 {
        prepared,
        token_after,
        receipt_id,
    })
}
