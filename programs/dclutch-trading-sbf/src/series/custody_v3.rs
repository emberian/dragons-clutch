//! Canonical SeriesEscrow-to-Custody V1 request projection.
//!
//! Series V3 owns the semantic escrow lifecycle. This module performs the
//! narrow physical projection into the one release-selected Custody writer.
//! Account keys, token programs, rent amounts, and the parent-request digest
//! are adapter-authenticated observations; they never select the economic
//! route or its amount.

use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestV1, OperationV1,
};
use dclutch_series_v3_kernel::escrow::{
    PrepareSeriesEscrowPlanV3, SeriesEscrowEffectKindV3, SeriesEscrowEffectV3,
    TerminalSeriesEscrowPlanV3,
};

/// Stable refusal from the Series-to-Custody physical projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesCustodyProjectionErrorV3 {
    /// A required physical observation was the reserved all-zero identity.
    MissingPhysicalIdentity,
    /// A required replay or Vault rent observation was zero.
    MissingRent,
    /// The projected request did not satisfy the canonical Custody contract.
    CustodyRequest,
}

/// Adapter-authenticated physical observations shared by one escrow action.
///
/// Unused endpoint identities may be zero. Every endpoint selected by the
/// semantic effect is required nonzero before a request is returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesCustodyPhysicalV3 {
    /// Current release-selected Trading program.
    pub caller_program: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Current transaction payer for replay/Vault creation.
    pub payer: [u8; 32],
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Founder-owned collateral source used only by Prepare/Lock.
    pub founder_source: [u8; 32],
    /// Canonical Custody-owned SeriesEscrow token account.
    pub escrow_vault: [u8; 32],
    /// Canonical Market HoardPrincipal token account used only by Consume.
    pub hoard_vault: [u8; 32],
    /// Refund-owner token account used only by Expire.
    pub refund_destination: [u8; 32],
    /// Exact replay-account lamports created and later recovered.
    pub replay_rent_lamports: u64,
    /// Exact SeriesEscrow Vault lamports created and later recovered.
    pub vault_rent_lamports: u64,
}

/// Project replay initialization, SeriesEscrow Vault creation, and founder lock.
pub fn project_prepare_custody_v3(
    plan: PrepareSeriesEscrowPlanV3,
    physical: SeriesCustodyPhysicalV3,
) -> Result<[CustodyRequestV1; 3], SeriesCustodyProjectionErrorV3> {
    let [initialize, open, lock] = plan.effects();
    Ok([
        project_effect(initialize, physical)?,
        project_effect(open, physical)?,
        project_effect(lock, physical)?,
    ])
}

/// Project Consume-or-Refund followed by mandatory Vault and replay closure.
pub fn project_terminal_custody_v3(
    plan: TerminalSeriesEscrowPlanV3,
    physical: SeriesCustodyPhysicalV3,
) -> Result<[CustodyRequestV1; 3], SeriesCustodyProjectionErrorV3> {
    let [transfer, close_vault, close_replay] = plan.effects();
    Ok([
        project_effect(transfer, physical)?,
        project_effect(close_vault, physical)?,
        project_effect(close_replay, physical)?,
    ])
}

fn project_effect(
    effect: SeriesEscrowEffectV3,
    physical: SeriesCustodyPhysicalV3,
) -> Result<CustodyRequestV1, SeriesCustodyProjectionErrorV3> {
    require_identity(physical.caller_program)?;
    require_identity(physical.parent_request_digest)?;
    let escrow = effect.escrow();
    let market = escrow.market().to_bytes();
    let ticket = escrow.ticket_id().to_bytes();
    let refund_owner = escrow.refund_owner().to_bytes();
    let escrow_vault_context = ticket;
    let hoard_vault_context = market;

    let mut request = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: escrow.release_set().to_bytes(),
        market,
        realm: escrow.realm().to_bytes(),
        context: ticket,
        caller_program: physical.caller_program,
        semantic: ContextV1 {
            candidate: escrow.occurrence_id().to_bytes(),
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: ticket,
            parent_request_digest: physical.parent_request_digest,
            order_nonce: u64::from(escrow.occurrence()),
            generation: escrow.generation(),
            page_index: 0,
            execution_index: escrow.occurrence(),
            transfer_index: transfer_index(effect.kind()),
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: effect.expected_revision(),
        resulting_revision: effect.resulting_revision(),
        amount: effect.amount(),
        rent_lamports: 0,
    };

    match effect.kind() {
        SeriesEscrowEffectKindV3::InitializeReplay => {
            require_identity(physical.payer)?;
            require_rent(physical.replay_rent_lamports)?;
            request.payer = physical.payer;
            request.rent_refund = refund_owner;
            request.rent_lamports = physical.replay_rent_lamports;
        }
        SeriesEscrowEffectKindV3::OpenEscrowVault => {
            require_token_profile(physical)?;
            require_identity(physical.payer)?;
            require_identity(physical.escrow_vault)?;
            require_rent(physical.vault_rent_lamports)?;
            request.operation = OperationV1::OpenVault;
            request.destination_compartment = CompartmentV1::SeriesEscrow;
            request.destination = physical.escrow_vault;
            request.destination_vault_context = escrow_vault_context;
            request.mint = physical.mint;
            request.token_program = physical.token_program;
            request.payer = physical.payer;
            request.rent_refund = refund_owner;
            request.rent_lamports = physical.vault_rent_lamports;
        }
        SeriesEscrowEffectKindV3::Lock => {
            require_token_profile(physical)?;
            require_identity(physical.founder_source)?;
            require_identity(physical.escrow_vault)?;
            request.operation = OperationV1::Transfer;
            request.source_compartment = CompartmentV1::External;
            request.destination_compartment = CompartmentV1::SeriesEscrow;
            request.semantic.source_owner = escrow.founder().to_bytes();
            request.source = physical.founder_source;
            request.destination = physical.escrow_vault;
            request.destination_vault_context = escrow_vault_context;
            request.mint = physical.mint;
            request.token_program = physical.token_program;
        }
        SeriesEscrowEffectKindV3::ConsumeIntoHoard => {
            require_token_profile(physical)?;
            require_identity(physical.escrow_vault)?;
            require_identity(physical.hoard_vault)?;
            request.operation = OperationV1::Transfer;
            request.source_compartment = CompartmentV1::SeriesEscrow;
            request.destination_compartment = CompartmentV1::HoardPrincipal;
            request.source = physical.escrow_vault;
            request.destination = physical.hoard_vault;
            request.source_vault_context = escrow_vault_context;
            request.destination_vault_context = hoard_vault_context;
            request.mint = physical.mint;
            request.token_program = physical.token_program;
        }
        SeriesEscrowEffectKindV3::RefundExpired => {
            require_token_profile(physical)?;
            require_identity(physical.escrow_vault)?;
            require_identity(physical.refund_destination)?;
            request.operation = OperationV1::Transfer;
            request.source_compartment = CompartmentV1::SeriesEscrow;
            request.destination_compartment = CompartmentV1::External;
            request.semantic.destination_owner = refund_owner;
            request.source = physical.escrow_vault;
            request.destination = physical.refund_destination;
            request.source_vault_context = escrow_vault_context;
            request.mint = physical.mint;
            request.token_program = physical.token_program;
        }
        SeriesEscrowEffectKindV3::CloseEscrowVault => {
            require_token_profile(physical)?;
            require_identity(physical.escrow_vault)?;
            require_rent(physical.vault_rent_lamports)?;
            request.operation = OperationV1::CloseVault;
            request.source_compartment = CompartmentV1::SeriesEscrow;
            request.source = physical.escrow_vault;
            request.source_vault_context = escrow_vault_context;
            request.mint = physical.mint;
            request.token_program = physical.token_program;
            request.rent_refund = refund_owner;
            request.rent_lamports = physical.vault_rent_lamports;
        }
        SeriesEscrowEffectKindV3::CloseReplay => {
            require_rent(physical.replay_rent_lamports)?;
            request.operation = OperationV1::CloseReplay;
            request.rent_refund = refund_owner;
            request.rent_lamports = physical.replay_rent_lamports;
        }
    }
    request
        .validate()
        .map_err(|_| SeriesCustodyProjectionErrorV3::CustodyRequest)?;
    Ok(request)
}

const fn transfer_index(kind: SeriesEscrowEffectKindV3) -> u16 {
    match kind {
        SeriesEscrowEffectKindV3::InitializeReplay
        | SeriesEscrowEffectKindV3::ConsumeIntoHoard
        | SeriesEscrowEffectKindV3::RefundExpired => 0,
        SeriesEscrowEffectKindV3::OpenEscrowVault | SeriesEscrowEffectKindV3::CloseEscrowVault => 1,
        SeriesEscrowEffectKindV3::Lock | SeriesEscrowEffectKindV3::CloseReplay => 2,
    }
}

fn require_token_profile(
    physical: SeriesCustodyPhysicalV3,
) -> Result<(), SeriesCustodyProjectionErrorV3> {
    require_identity(physical.mint)?;
    require_identity(physical.token_program)
}

fn require_identity(value: [u8; 32]) -> Result<(), SeriesCustodyProjectionErrorV3> {
    if value == [0; 32] {
        Err(SeriesCustodyProjectionErrorV3::MissingPhysicalIdentity)
    } else {
        Ok(())
    }
}

const fn require_rent(value: u64) -> Result<(), SeriesCustodyProjectionErrorV3> {
    if value == 0 {
        Err(SeriesCustodyProjectionErrorV3::MissingRent)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;
    use dclutch_series_v3_kernel::escrow::{
        consume_series_escrow_v3, expire_series_escrow_v3, prepare_series_escrow_v3,
    };
    use dclutch_series_v3_kernel::{
        AccountKeyV3, AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_BYTES_V3,
        SERIES_TEMPLATE_BYTES_V3, SERIES_TICKET_BYTES_V3, admit_occurrence, admit_ticket,
        generated, occurrence_content_id, pre_founding_series_escrow, template_content_id,
    };
    use solana_program::hash::hashv;

    use super::*;

    const HASH_SEPARATOR: [u8; 1] = [0];

    fn key(byte: u8) -> AccountKeyV3 {
        AccountKeyV3::new([byte; 32]).expect("key")
    }

    fn put<const N: usize>(target: &mut [u8], offset: usize, value: &[u8; N]) {
        target[offset..offset + N].copy_from_slice(value);
    }

    fn node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        hashv(&[
            &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
            &HASH_SEPARATOR,
            left,
            right,
        ])
        .to_bytes()
    }

    fn escrow() -> dclutch_series_v3_kernel::PrefoundingSeriesEscrowV3 {
        let mut template: [u8; SERIES_TEMPLATE_BYTES_V3] = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3] = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket: [u8; SERIES_TICKET_BYTES_V3] = generated::SERIES_EXAMPLE_TICKET_V3;
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence");
        let siblings = [[90; 32], [91; 32]];
        let root = node(&node(&siblings[0], &occurrence_id.to_bytes()), &siblings[1]);
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &root,
        );
        let template_id = template_content_id(&template).expect("template");
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        let admitted = admit_occurrence(&template, &occurrence, &siblings).expect("admitted");
        let product = AuthenticatedProductProjectionV2::new(
            admitted.occurrence().product_record(),
            ContentId::new([61; 32]).expect("product"),
            ContentId::new([62; 32]).expect("domain"),
        );
        pre_founding_series_escrow(
            admitted,
            admit_ticket(&ticket).expect("ticket"),
            product,
            key(59),
        )
        .expect("escrow")
    }

    const fn physical() -> SeriesCustodyPhysicalV3 {
        SeriesCustodyPhysicalV3 {
            caller_program: [1; 32],
            parent_request_digest: [2; 32],
            payer: [3; 32],
            mint: [4; 32],
            token_program: [5; 32],
            founder_source: [6; 32],
            escrow_vault: [7; 32],
            hoard_vault: [8; 32],
            refund_destination: [9; 32],
            replay_rent_lamports: 10,
            vault_rent_lamports: 11,
        }
    }

    #[test]
    fn prepare_maps_one_to_one_into_canonical_custody_requests() {
        let projection = escrow();
        let requests = project_prepare_custody_v3(prepare_series_escrow_v3(projection), physical())
            .expect("prepare requests");
        assert_eq!(
            requests.map(|request| request.operation),
            [
                OperationV1::InitializeReplay,
                OperationV1::OpenVault,
                OperationV1::Transfer,
            ]
        );
        assert_eq!(requests.map(|request| request.expected_revision), [0, 1, 2]);
        assert_eq!(
            requests[1].destination_compartment,
            CompartmentV1::SeriesEscrow
        );
        assert_eq!(
            requests[2].semantic.source_owner,
            projection.founder().to_bytes()
        );
        assert_eq!(requests[2].amount, projection.hoard_principal());
    }

    #[test]
    fn consume_and_refund_share_cleanup_but_not_destination_authority() {
        let projection = escrow();
        let consume = project_terminal_custody_v3(consume_series_escrow_v3(projection), physical())
            .expect("consume requests");
        let refund = project_terminal_custody_v3(expire_series_escrow_v3(projection), physical())
            .expect("refund requests");
        assert_eq!(
            consume.map(|request| request.operation),
            [
                OperationV1::Transfer,
                OperationV1::CloseVault,
                OperationV1::CloseReplay,
            ]
        );
        assert_eq!(consume.map(|request| request.expected_revision), [3, 4, 5]);
        assert_eq!(
            consume[0].destination_compartment,
            CompartmentV1::HoardPrincipal
        );
        assert_eq!(
            consume[0].destination_vault_context,
            projection.market().to_bytes()
        );
        assert_eq!(refund[0].destination_compartment, CompartmentV1::External);
        assert_eq!(
            refund[0].semantic.destination_owner,
            projection.refund_owner().to_bytes()
        );
        assert_eq!(consume[1], refund[1]);
        assert_eq!(consume[2], refund[2]);
    }

    #[test]
    fn missing_selected_physical_observation_refuses_without_partial_output() {
        let projection = escrow();
        let mut missing = physical();
        missing.escrow_vault = [0; 32];
        assert_eq!(
            project_prepare_custody_v3(prepare_series_escrow_v3(projection), missing),
            Err(SeriesCustodyProjectionErrorV3::MissingPhysicalIdentity)
        );
        let mut no_rent = physical();
        no_rent.vault_rent_lamports = 0;
        assert_eq!(
            project_terminal_custody_v3(consume_series_escrow_v3(projection), no_rent),
            Err(SeriesCustodyProjectionErrorV3::MissingRent)
        );
    }
}
