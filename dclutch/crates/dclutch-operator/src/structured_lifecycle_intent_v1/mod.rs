//! Pure planning and instruction construction for Structured receipt lifecycle actions.
//!
//! RPC acquisition lives in [`discovery`]. This module consumes only typed,
//! caller-owned observations, derives the Claims caller authority from the
//! lifecycle request, and emits unsigned instructions. It never reads a
//! wallet, signs, submits, or depends on the local-validator bootstrap.

pub mod discovery;
pub mod discovery_context;
pub mod types;

pub use types::*;

use std::collections::BTreeSet;

use dclutch_claims::rational_lifecycle::{LifecycleActionV2, LifecycleRequestV2};
use dclutch_market::capability_program::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACTIVATION_CACHE_ACCOUNT_V3,
    HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
    HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
    HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MANIFEST_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
    HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
    HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_STRATEGY_RAW_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_vm::capability_seal::{
    CAPABILITY_SEAL_BYTES_V1, CapabilitySealKeyV1, SealedDescriptorClosureV1, SealedRecordRowV1,
    SealedRoleV1,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

/// Stable refusal from pure Structured lifecycle instruction construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructuredLifecycleConstructionErrorV1 {
    /// Claims refused the lifecycle request bytes.
    Lifecycle(dclutch_claims::rational_lifecycle::Error),
    /// The lifecycle action did not match the requested physical frame.
    Action,
    /// A coordinate action omitted its single canonical coordinate row.
    Coordinate,
    /// A semantic account differed from the lifecycle request it must serve.
    PhysicalFrame,
    /// The selected release could not derive the Trading caller authority.
    CallerAuthority(dclutch_registry::release_set::Error),
    /// Authenticated discovery context omitted or contradicted a required role.
    Deployment,
    /// Selected immutable V6 artifact bytes did not satisfy their native contract.
    SelectedBundle(crate::rational_lifecycle_hot::Error),
    /// The selected lifecycle action has no Structured selector.
    Selector,
    /// The capability-seal identity refused its selected coordinates.
    Seal(dclutch_vm::capability_seal::Error),
    /// A selected fixed-width artifact changed width.
    ArtifactWidth,
    /// The checked infrastructure manifest was malformed or internally inconsistent.
    Infrastructure(dclutch_release_tool::Error),
    /// Canonical Claims aggregate seeds refused the selected Market.
    LiabilityBasis(dclutch_claims::liability_basis_state_v2::LiabilityBasisStateErrorV2),
    /// Canonical Claims Position or admission seeds refused a coordinate.
    ProtocolPosition(dclutch_claims::protocol_position_v2::ProtocolPositionErrorV2),
    /// The finalized account corpus was malformed or incomplete.
    Discovery(discovery::StructuredLifecycleDiscoveryErrorV1),
    /// A finalized account image differed from the native plan.
    Poststate,
    /// Authenticated immutable context discovery refused.
    Context(discovery_context::StructuredLifecycleContextErrorV1),
    /// Native instruction construction refused at a named boundary.
    Native {
        /// Semantic construction boundary.
        stage: &'static str,
        /// Original native cause.
        cause: String,
    },
    /// Checked lamport or width arithmetic overflowed or underflowed.
    Arithmetic,
}

/// Exact selected artifacts and fixed account coordinates before mutable state planning.
#[derive(Clone, Debug)]
pub struct StructuredLifecycleSelectedPreparationV1 {
    /// V6 bundle reconstructed from authenticated finalized records.
    pub bundle: crate::rational_lifecycle_hot::RationalLifecycleSelectedBundleV6,
    /// Canonical Hot fixed frame in the native ABI order.
    pub fixed_accounts: Vec<Pubkey>,
    /// Selected capability seal identity and PDA source.
    pub seal_key: CapabilitySealKeyV1,
    /// Current checked Trading outer.
    pub hot_outer: crate::rational_lifecycle_hot::CheckedRationalLifecycleHotOuterV3,
}

/// Canonical common mutable accounts for one Structured receipt lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleReceiptCoordinatesV1 {
    /// Claims aggregate for the Core Market.
    pub claims_market: Pubkey,
    /// Core-selected lifecycle rent recipient.
    pub rent_credit: Pubkey,
    /// Descriptor-selected receipt Mint.
    pub receipt_mint: Pubkey,
    /// Selected action's capability seal.
    pub capability_seal: Pubkey,
}

/// Canonical physical resources for one nonzero descriptor coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleCoordinateCoordinatesV1 {
    /// Descriptor outcome coordinate.
    pub outcome: u32,
    /// Exact nonzero descriptor coefficient.
    pub coefficient: u64,
    /// Claims-owned shard Mint.
    pub shard_mint: Pubkey,
    /// Claims-owned Structured custody token account.
    pub structured_custody: Pubkey,
    /// Claims capability owner for this descriptor coordinate.
    pub owner: Pubkey,
    /// Claims liability-basis Position.
    pub position: Pubkey,
    /// Claims protocol-Position admission record.
    pub admission: Pubkey,
}

/// All mutable coordinates needed to plan one selected lifecycle action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecyclePhysicalCoordinatesV1 {
    /// Receipt-level accounts shared by every action.
    pub common: StructuredLifecycleReceiptCoordinatesV1,
    /// One row for coordinate actions, all support rows for receipt retirement.
    pub coordinates: Vec<StructuredLifecycleCoordinateCoordinatesV1>,
}

/// Project the complete Structured deployment context from one checked infrastructure manifest.
///
/// The manifest is the release pipeline's existing `infrastructure.checked` artifact. This
/// function keeps browsers and command-line clients from reconstructing its binary layout or
/// separately assigning execution roles.
pub fn structured_lifecycle_programs_from_checked_infrastructure_v1(
    bytes: &[u8],
) -> Result<StructuredLifecycleProgramsV1, StructuredLifecycleConstructionErrorV1> {
    let infrastructure = dclutch_release_tool::CheckedInfrastructureV1::decode(bytes)
        .map_err(StructuredLifecycleConstructionErrorV1::Infrastructure)?;
    let execution = infrastructure.execution();
    let release_set = execution
        .execution_release_set_id()
        .map_err(StructuredLifecycleConstructionErrorV1::Infrastructure)?
        .to_bytes();
    let artifact = |role: ExecutionRoleV1| {
        execution
            .artifacts()
            .get(role.role_index())
            .copied()
            .ok_or(StructuredLifecycleConstructionErrorV1::Deployment)
    };
    let registry = infrastructure.profile().registry().program().to_bytes();
    Ok(StructuredLifecycleProgramsV1 {
        core: artifact(ExecutionRoleV1::Core)?.program().to_bytes(),
        registry,
        claims: artifact(ExecutionRoleV1::Claims)?.program().to_bytes(),
        trading: artifact(ExecutionRoleV1::Trading)?.program().to_bytes(),
        rent_program: infrastructure.profile().rent().program().to_bytes(),
        custody: artifact(ExecutionRoleV1::Custody)?.program().to_bytes(),
        activation_cache: dclutch_registry::activation_auth_v1::activation_cache_address_v1(
            &Pubkey::new_from_array(registry),
            &release_set,
        )
        .to_bytes(),
        checked_execution_release_set: execution.encode().to_vec(),
    })
}

/// Derive every mutable lifecycle coordinate from authenticated immutable context.
pub fn structured_lifecycle_physical_coordinates_v1(
    intent: &StructuredLifecycleIntentV1,
    programs: &StructuredLifecycleProgramsV1,
    context: &discovery_context::StructuredLifecycleContextV1,
    prepared: &StructuredLifecycleSelectedPreparationV1,
) -> Result<StructuredLifecyclePhysicalCoordinatesV1, StructuredLifecycleConstructionErrorV1> {
    if intent.market != context.market.identity.market_id.to_bytes()
        || intent
            .selected_capability
            .is_some_and(|root| root != context.root)
        || intent
            .representation_descriptor
            .is_some_and(|descriptor| descriptor != context.selection.descriptor)
    {
        return Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame);
    }
    let selected = match intent.action {
        LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt => {
            if intent.coordinate.is_some() {
                return Err(StructuredLifecycleConstructionErrorV1::Coordinate);
            }
            if intent.action == LifecycleActionV2::RetireReceipt {
                context.selection.support.clone()
            } else {
                Vec::new()
            }
        }
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            let outcome = intent
                .coordinate
                .ok_or(StructuredLifecycleConstructionErrorV1::Coordinate)?;
            let support = context
                .selection
                .support
                .iter()
                .find(|row| row.coordinate == outcome)
                .copied()
                .ok_or(StructuredLifecycleConstructionErrorV1::Coordinate)?;
            vec![support]
        }
    };
    let claims = Pubkey::new_from_array(programs.claims);
    let claims_market_seeds =
        dclutch_claims::liability_basis_state_v2::LiabilityBasisMarketSeedsV2::new(intent.market)
            .map_err(StructuredLifecycleConstructionErrorV1::LiabilityBasis)?;
    let claims_market = Pubkey::find_program_address(&claims_market_seeds.as_slices(), &claims).0;
    let coordinates = selected
        .into_iter()
        .map(|support| {
            let outcome = support.coordinate;
            let outcome_bytes = outcome.to_le_bytes();
            let descriptor = context.selection.descriptor;
            let shard_mint = Pubkey::find_program_address(
                &[
                    dclutch_claims::rational::RATIONAL_SHARD_MINT_SEED_V2,
                    &descriptor,
                    &outcome_bytes,
                ],
                &claims,
            )
            .0;
            let structured_custody = Pubkey::find_program_address(
                &[
                    dclutch_claims::rational::RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                    &descriptor,
                    &outcome_bytes,
                ],
                &claims,
            )
            .0;
            let owner_seeds =
                dclutch_claims::protocol_position_v2::ProtocolPositionClaimsCapabilitySeedsV2::new(
                    descriptor, outcome,
                )
                .map_err(StructuredLifecycleConstructionErrorV1::ProtocolPosition)?;
            let owner = Pubkey::find_program_address(&owner_seeds.as_slices(), &claims).0;
            let position_seeds =
                dclutch_claims::protocol_position_v2::ProtocolPositionSeedsV2::new(
                    claims_market.to_bytes(),
                    owner.to_bytes(),
                )
                .map_err(StructuredLifecycleConstructionErrorV1::ProtocolPosition)?;
            let admission_seeds =
                dclutch_claims::protocol_position_v2::ProtocolPositionAdmissionSeedsV2::new(
                    claims_market.to_bytes(),
                    owner.to_bytes(),
                )
                .map_err(StructuredLifecycleConstructionErrorV1::ProtocolPosition)?;
            let position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims).0;
            let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &claims).0;
            if intent
                .expected_position
                .is_some_and(|expected| expected != position.to_bytes())
            {
                return Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame);
            }
            Ok(StructuredLifecycleCoordinateCoordinatesV1 {
                outcome,
                coefficient: support.coefficient,
                shard_mint,
                structured_custody,
                owner,
                position,
                admission,
            })
        })
        .collect::<Result<Vec<_>, StructuredLifecycleConstructionErrorV1>>()?;
    Ok(StructuredLifecyclePhysicalCoordinatesV1 {
        common: StructuredLifecycleReceiptCoordinatesV1 {
            claims_market,
            rent_credit: Pubkey::new_from_array(context.market.rent_beneficiary.to_bytes()),
            receipt_mint: Pubkey::new_from_array(context.selection.receipt_mint),
            capability_seal: Pubkey::find_program_address(
                &prepared.seal_key.seeds().as_slices(),
                &Pubkey::new_from_array(programs.trading),
            )
            .0,
        },
        coordinates,
    })
}

impl StructuredLifecyclePhysicalCoordinatesV1 {
    /// Point observations needed after immutable context discovery.
    pub fn requests(&self) -> Vec<StructuredLifecycleAccountRequestV1> {
        let mut requests = vec![
            self.common.claims_market,
            self.common.rent_credit,
            self.common.receipt_mint,
            self.common.capability_seal,
        ];
        for row in &self.coordinates {
            requests.extend([
                row.shard_mint,
                row.structured_custody,
                row.position,
                row.admission,
            ]);
        }
        requests
            .into_iter()
            .map(|address| StructuredLifecycleAccountRequestV1 {
                address: address.to_bytes(),
                data_slice: None,
            })
            .collect()
    }
}

/// Verify the exact finalized account images promised by one native lifecycle plan.
///
/// The transaction fee is applied only to the declared payer image. All other
/// owner, executable, width, data and lamport fields must match byte for byte.
pub fn verify_structured_lifecycle_poststates_v1(
    plan: &StructuredLifecyclePlanV1,
    snapshot: &StructuredLifecycleSnapshotV1,
    finalized_transaction_fee: u64,
) -> Result<(), StructuredLifecycleConstructionErrorV1> {
    let corpus = discovery::StructuredLifecycleCorpusV1::new(snapshot)
        .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?;
    if snapshot.slot < plan.finalized_slot || plan.expected_poststates.is_empty() {
        return Err(StructuredLifecycleConstructionErrorV1::Poststate);
    }
    let mut addresses = BTreeSet::new();
    let mut fee_payer_count = 0_usize;
    for expected in &plan.expected_poststates {
        if !addresses.insert(expected.address) {
            return Err(StructuredLifecycleConstructionErrorV1::Poststate);
        }
        if expected.deduct_transaction_fee {
            fee_payer_count = fee_payer_count
                .checked_add(1)
                .ok_or(StructuredLifecycleConstructionErrorV1::Poststate)?;
            if expected.address != plan.intent.payer || expected.value.is_none() {
                return Err(StructuredLifecycleConstructionErrorV1::Poststate);
            }
        }
        let actual = corpus
            .full(expected.address)
            .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?;
        match (&expected.value, actual) {
            (None, None) => {}
            (Some(expected_value), Some(actual)) => {
                let lamports = if expected.deduct_transaction_fee {
                    expected_value
                        .lamports
                        .checked_sub(finalized_transaction_fee)
                        .ok_or(StructuredLifecycleConstructionErrorV1::Poststate)?
                } else {
                    expected_value.lamports
                };
                if actual.owner != expected_value.owner
                    || actual.lamports != lamports
                    || actual.executable != expected_value.executable
                    || actual.space != expected_value.space
                    || actual.data != expected_value.data
                {
                    return Err(StructuredLifecycleConstructionErrorV1::Poststate);
                }
            }
            _ => return Err(StructuredLifecycleConstructionErrorV1::Poststate),
        }
    }
    if fee_payer_count != 1 {
        return Err(StructuredLifecycleConstructionErrorV1::Poststate);
    }
    Ok(())
}

/// Plan the next pure Structured lifecycle step from one finalized corpus.
///
/// Discovery and explicit selection are returned unchanged. Once immutable
/// context exists, the first missing executable prerequisite is the exact
/// selected-artifact seal. A later call against the finalized seal continues
/// into physical lifecycle planning.
pub fn plan_structured_lifecycle_v1(
    intent: &StructuredLifecycleIntentV1,
    programs: &StructuredLifecycleProgramsV1,
    snapshot: &StructuredLifecycleSnapshotV1,
) -> Result<StructuredLifecyclePlanningV1, StructuredLifecycleConstructionErrorV1> {
    let context = match discovery_context::discover_structured_lifecycle_context_v1(
        intent, programs, snapshot,
    )
    .map_err(StructuredLifecycleConstructionErrorV1::Context)?
    {
        discovery_context::StructuredLifecycleContextProgressV1::Continue(progress) => {
            return Ok(progress);
        }
        discovery_context::StructuredLifecycleContextProgressV1::Ready(context) => context,
    };
    let prepared = prepare_selected_structured_lifecycle_v1(intent.action, programs, &context)?;
    let physical =
        structured_lifecycle_physical_coordinates_v1(intent, programs, &context, &prepared)?;
    let corpus = discovery::StructuredLifecycleCorpusV1::new(snapshot)
        .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?;
    let missing = corpus
        .missing(&physical.requests())
        .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?;
    if !missing.is_empty() {
        return Ok(StructuredLifecyclePlanningV1::Discover {
            requests: missing,
            scans: Vec::new(),
        });
    }
    let (seal, bump) = Pubkey::find_program_address(
        &prepared.seal_key.seeds().as_slices(),
        &Pubkey::new_from_array(programs.trading),
    );
    let expected_seal = selected_capability_seal_body_v1(&context, &prepared, bump)?;
    match corpus
        .full(seal.to_bytes())
        .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?
    {
        None => plan_structured_lifecycle_seal_v1(
            intent,
            programs,
            snapshot,
            &context,
            &prepared,
            seal,
            expected_seal,
        ),
        Some(account)
            if account.owner == programs.trading
                && !account.executable
                && account.data == expected_seal =>
        {
            plan_selected_structured_lifecycle_execution_v1(
                intent, programs, snapshot, &context, &prepared, &physical,
            )
        }
        Some(_) => Err(StructuredLifecycleConstructionErrorV1::Poststate),
    }
}

fn selected_capability_seal_body_v1(
    context: &discovery_context::StructuredLifecycleContextV1,
    prepared: &StructuredLifecycleSelectedPreparationV1,
    bump: u8,
) -> Result<Vec<u8>, StructuredLifecycleConstructionErrorV1> {
    let records = [
        &context.action_records[0],
        &context.action_records[3],
        &context.action_records[1],
        &context.action_records[2],
        &context.action_records[5],
        &context.action_records[6],
    ];
    let roles = SealedRoleV1::canonical_order();
    let mut rows = Vec::with_capacity(records.len());
    for index in 0..records.len() {
        let record = records[index];
        rows.push(
            SealedRecordRowV1::new(
                roles[index],
                u32::try_from(record.raw.data.len())
                    .map_err(|_| StructuredLifecycleConstructionErrorV1::ArtifactWidth)?,
                record.key.schema,
                record.key.content,
                record.key.raw,
                record.key.staging,
            )
            .map_err(StructuredLifecycleConstructionErrorV1::Seal)?,
        );
    }
    let rows = rows
        .try_into()
        .map_err(|_| StructuredLifecycleConstructionErrorV1::ArtifactWidth)?;
    let mut bytes = vec![0; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(prepared.seal_key, rows, bump, &mut bytes)
        .map_err(StructuredLifecycleConstructionErrorV1::Seal)?;
    Ok(bytes)
}

fn plan_structured_lifecycle_seal_v1(
    intent: &StructuredLifecycleIntentV1,
    programs: &StructuredLifecycleProgramsV1,
    snapshot: &StructuredLifecycleSnapshotV1,
    context: &discovery_context::StructuredLifecycleContextV1,
    prepared: &StructuredLifecycleSelectedPreparationV1,
    seal: Pubkey,
    expected_seal: Vec<u8>,
) -> Result<StructuredLifecyclePlanningV1, StructuredLifecycleConstructionErrorV1> {
    let corpus = discovery::StructuredLifecycleCorpusV1::new(snapshot)
        .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?;
    let rent = crate::observation::decode_rent(
        &corpus
            .observed(sysvar::rent::ID.to_bytes(), context.clock.unix_timestamp)
            .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?,
    )
    .map_err(|error| StructuredLifecycleConstructionErrorV1::Native {
        stage: "Rent sysvar",
        cause: format!("{error:?}"),
    })?;
    let payer = corpus
        .present(intent.payer)
        .map_err(StructuredLifecycleConstructionErrorV1::Discovery)?;
    if payer.owner != system_program::ID.to_bytes()
        || payer.executable
        || payer.space != 0
        || !payer.data.is_empty()
    {
        return Err(StructuredLifecycleConstructionErrorV1::Poststate);
    }
    let seal_rent = rent.minimum_balance(CAPABILITY_SEAL_BYTES_V1);
    let payer_after = payer
        .lamports
        .checked_sub(seal_rent)
        .ok_or(StructuredLifecycleConstructionErrorV1::Arithmetic)?;
    let trading = context
        .roles
        .iter()
        .find(|role| role.role == ExecutionRoleV1::Trading)
        .ok_or(StructuredLifecycleConstructionErrorV1::Deployment)?;
    let selector =
        dclutch_claims::rational_lifecycle::hot_v6::structured_lifecycle_action_selector_v1(
            dclutch_claims::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
            intent.action,
        )
        .ok_or(StructuredLifecycleConstructionErrorV1::Selector)?;
    let instruction = crate::capability_seal_v1::capability_seal_instruction_v1(
        crate::capability_seal_v1::CapabilitySealInstructionInputV1 {
            trading_program: Pubkey::new_from_array(programs.trading),
            registry_program: Pubkey::new_from_array(programs.registry),
            trading_semantic_release: trading.activated.release().semantic_release_id().to_bytes(),
            descriptor_digest: context.action_records[0].key.content,
            action: selector,
            fixed_frame: &prepared.fixed_accounts,
            payer: Pubkey::new_from_array(intent.payer),
        },
    )
    .map_err(|error| StructuredLifecycleConstructionErrorV1::Native {
        stage: "Capability seal instruction",
        cause: format!("{error:?}"),
    })?
    .instruction;
    let expected_poststates = vec![
        StructuredLifecycleExpectedAccountV1 {
            address: intent.payer,
            value: Some(StructuredLifecycleAccountValueV1 {
                lamports: payer_after,
                ..payer.clone()
            }),
            deduct_transaction_fee: true,
        },
        StructuredLifecycleExpectedAccountV1 {
            address: seal.to_bytes(),
            value: Some(StructuredLifecycleAccountValueV1 {
                owner: programs.trading,
                lamports: seal_rent,
                executable: false,
                space: u64::try_from(CAPABILITY_SEAL_BYTES_V1)
                    .map_err(|_| StructuredLifecycleConstructionErrorV1::Arithmetic)?,
                data: expected_seal,
            }),
            deduct_transaction_fee: false,
        },
    ];
    let preview = StructuredLifecyclePreviewV1 {
        receipt_mint: context.selection.receipt_mint,
        coordinate: intent.coordinate,
        position: physical_position_v1(intent, context, programs)?,
        preparation_lamports: seal_rent,
        returned_rent_lamports: 0,
        rent_recipient: None,
        receipt_supply_before: 0,
        receipt_supply_after: 0,
    };
    let mut plan = StructuredLifecyclePlanV1 {
        intent: intent.clone(),
        step_id: [0; 32],
        step_kind: StructuredLifecycleStepKindV1::SealArtifact,
        selected_capability: context.root,
        finalized_slot: snapshot.slot,
        instructions: vec![instruction],
        required_wallet_signers: vec![intent.payer],
        preview,
        expected_poststates,
    };
    plan.step_id = structured_lifecycle_step_id_v1(&plan)?;
    Ok(StructuredLifecyclePlanningV1::Ready { plan })
}

fn physical_position_v1(
    intent: &StructuredLifecycleIntentV1,
    context: &discovery_context::StructuredLifecycleContextV1,
    programs: &StructuredLifecycleProgramsV1,
) -> Result<Option<[u8; 32]>, StructuredLifecycleConstructionErrorV1> {
    if intent.coordinate.is_none() {
        return Ok(None);
    }
    let prepared = prepare_selected_structured_lifecycle_v1(intent.action, programs, context)?;
    Ok(
        structured_lifecycle_physical_coordinates_v1(intent, programs, context, &prepared)?
            .coordinates
            .first()
            .map(|coordinate| coordinate.position.to_bytes()),
    )
}

fn structured_lifecycle_step_id_v1(
    plan: &StructuredLifecyclePlanV1,
) -> Result<[u8; 32], StructuredLifecycleConstructionErrorV1> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"dclutch/structured-lifecycle-step/v1");
    bytes.extend_from_slice(&plan.intent.market);
    bytes.extend_from_slice(&plan.intent.payer);
    bytes.push(plan.intent.action.tag());
    bytes.extend_from_slice(&plan.intent.coordinate.unwrap_or(u32::MAX).to_le_bytes());
    bytes.extend_from_slice(&plan.selected_capability);
    bytes.push(match plan.step_kind {
        StructuredLifecycleStepKindV1::SealArtifact => 0,
        StructuredLifecycleStepKindV1::FundRent => 1,
        StructuredLifecycleStepKindV1::ExecuteLifecycle => 2,
    });
    for instruction in &plan.instructions {
        bytes.extend_from_slice(&instruction.program_id.to_bytes());
        bytes.extend_from_slice(
            &u32::try_from(instruction.accounts.len())
                .map_err(|_| StructuredLifecycleConstructionErrorV1::Arithmetic)?
                .to_le_bytes(),
        );
        for meta in &instruction.accounts {
            bytes.extend_from_slice(&meta.pubkey.to_bytes());
            bytes.push(u8::from(meta.is_signer));
            bytes.push(u8::from(meta.is_writable));
        }
        bytes.extend_from_slice(&hash(&instruction.data).to_bytes());
    }
    for expected in &plan.expected_poststates {
        bytes.extend_from_slice(&expected.address);
        bytes.push(u8::from(expected.deduct_transaction_fee));
        match &expected.value {
            None => bytes.push(0),
            Some(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.owner);
                bytes.extend_from_slice(&value.lamports.to_le_bytes());
                bytes.push(u8::from(value.executable));
                bytes.extend_from_slice(&value.space.to_le_bytes());
                bytes.extend_from_slice(&hash(&value.data).to_bytes());
            }
        }
    }
    Ok(hash(&bytes).to_bytes())
}

fn plan_selected_structured_lifecycle_execution_v1(
    _intent: &StructuredLifecycleIntentV1,
    _programs: &StructuredLifecycleProgramsV1,
    _snapshot: &StructuredLifecycleSnapshotV1,
    _context: &discovery_context::StructuredLifecycleContextV1,
    _prepared: &StructuredLifecycleSelectedPreparationV1,
    _physical: &StructuredLifecyclePhysicalCoordinatesV1,
) -> Result<StructuredLifecyclePlanningV1, StructuredLifecycleConstructionErrorV1> {
    Err(StructuredLifecycleConstructionErrorV1::Native {
        stage: "Lifecycle execution planning",
        cause: "selected artifact seal is ready; physical transition extraction continues".into(),
    })
}

/// Bind a Claims lifecycle child to the exact V6 family bytes Hot will carry.
///
/// The family form clears `parent_context`; the specialized Claims child carries
/// the digest of that family. A nonzero provisional context lets the Claims
/// decoder validate the otherwise complete child before the native V6 projector
/// derives its canonical family form.
pub fn bind_selected_lifecycle_parent_v6(
    mut header: dclutch_claims::rational_lifecycle::LifecycleHeaderV2,
    coordinate_bytes: &[u8],
) -> Result<
    dclutch_claims::rational_lifecycle::LifecycleHeaderV2,
    StructuredLifecycleConstructionErrorV1,
> {
    header.parent_context = [1; 32];
    let provisional = LifecycleRequestV2::new(header, coordinate_bytes)
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    let mut family_bytes = vec![
        0;
        dclutch_claims::rational_lifecycle::LIFECYCLE_HEADER_BYTES_V2
            .checked_add(coordinate_bytes.len())
            .ok_or(StructuredLifecycleConstructionErrorV1::ArtifactWidth)?
    ];
    let family =
        dclutch_claims::rational_lifecycle::hot_v6::RationalLifecycleHotRequestV6::from_child_into(
            provisional,
            &mut family_bytes,
        )
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    header.parent_context = hash(family.as_bytes()).to_bytes();
    Ok(header)
}

/// Reconstruct the selected V6 bundle and Hot fixed frame from authenticated context.
///
/// This accepts no account coordinates beyond the selected deployment. Every Registry
/// pair is taken from `context`, whose discovery path already authenticated its content
/// address and finalized staging vacancy.
pub fn prepare_selected_structured_lifecycle_v1(
    action: LifecycleActionV2,
    programs: &StructuredLifecycleProgramsV1,
    context: &discovery_context::StructuredLifecycleContextV1,
) -> Result<StructuredLifecycleSelectedPreparationV1, StructuredLifecycleConstructionErrorV1> {
    let release_set = context.market.identity.selected_release_set.to_bytes();
    let config: [u8; dclutch_custody::token_svm::TOKEN_BEHAVIOR_SELECTION_BYTES_V2] = context
        .config
        .raw
        .data
        .as_slice()
        .try_into()
        .map_err(|_| StructuredLifecycleConstructionErrorV1::ArtifactWidth)?;
    let descriptor: [u8; dclutch_market::capability_program::v4::CAPABILITY_PROGRAM_V4_BYTES] =
        context.action_records[0]
            .raw
            .data
            .as_slice()
            .try_into()
            .map_err(|_| StructuredLifecycleConstructionErrorV1::ArtifactWidth)?;
    let strategy: [u8;
        dclutch_market::execution_strategy::v2::EXECUTION_STRATEGY_PROGRAM_BYTES_V2] = context
        .action_records[4]
        .raw
        .data
        .as_slice()
        .try_into()
        .map_err(|_| StructuredLifecycleConstructionErrorV1::ArtifactWidth)?;
    let bundle = crate::rational_lifecycle_hot::RationalLifecycleSelectedBundleV6 {
        action,
        release_set,
        token_program: dclutch_custody::token_svm::TokenBehaviorSelectionV2::decode(&config)
            .map_err(|_| StructuredLifecycleConstructionErrorV1::ArtifactWidth)?
            .token_program(),
        token_behavior_selection: config,
        account_profile: context.action_records[1].raw.data.clone(),
        request_profile: context.action_records[2].raw.data.clone(),
        lifecycle_policy: context.action_records[3].raw.data.clone(),
        strategy,
        transition: context.action_records[5].raw.data.clone(),
        effect: context.action_records[6].raw.data.clone(),
        descriptor,
    };
    crate::rational_lifecycle_hot::validate_rational_lifecycle_selected_bundle_v6(&bundle)
        .map_err(StructuredLifecycleConstructionErrorV1::SelectedBundle)?;

    let role = |wanted| {
        context
            .roles
            .iter()
            .find(|role| role.role == wanted)
            .copied()
            .ok_or(StructuredLifecycleConstructionErrorV1::Deployment)
    };
    let core = role(ExecutionRoleV1::Core)?;
    let trading = role(ExecutionRoleV1::Trading)?;
    if core.program != programs.core || trading.program != programs.trading {
        return Err(StructuredLifecycleConstructionErrorV1::Deployment);
    }
    let selector =
        dclutch_claims::rational_lifecycle::hot_v6::structured_lifecycle_action_selector_v1(
            dclutch_claims::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
            action,
        )
        .ok_or(StructuredLifecycleConstructionErrorV1::Selector)?;
    let trading_semantic_release = trading.activated.release().semantic_release_id().to_bytes();
    let seal_key = CapabilitySealKeyV1::new(
        dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
        context.action_records[0].key.content,
        selector,
        trading_semantic_release,
        programs.registry,
    )
    .map_err(StructuredLifecycleConstructionErrorV1::Seal)?;
    let seal = Pubkey::find_program_address(
        &seal_key.seeds().as_slices(),
        &Pubkey::new_from_array(programs.trading),
    )
    .0;
    let mut fixed_accounts = vec![Pubkey::default(); HOT_FIXED_ACCOUNT_COUNT_V3];
    let mut place = |index: usize, key: [u8; 32]| {
        if key == [0; 32] {
            return Err(StructuredLifecycleConstructionErrorV1::Deployment);
        }
        *fixed_accounts
            .get_mut(index)
            .ok_or(StructuredLifecycleConstructionErrorV1::Deployment)? =
            Pubkey::new_from_array(key);
        Ok(())
    };
    place(
        HOT_MARKET_ACCOUNT_V3,
        context.market.identity.market_id.to_bytes(),
    )?;
    place(HOT_ROOT_ACCOUNT_V3, context.root)?;
    for (index, record) in [
        (HOT_MANIFEST_RAW_ACCOUNT_V3, &context.manifest),
        (HOT_PROGRAM_SET_RAW_ACCOUNT_V3, &context.program_set),
        (HOT_DESCRIPTOR_RAW_ACCOUNT_V3, &context.action_records[0]),
        (HOT_CONFIG_RAW_ACCOUNT_V3, &context.config),
        (
            HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
            &context.action_records[1],
        ),
        (
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            &context.action_records[2],
        ),
        (HOT_TRANSITION_RAW_ACCOUNT_V3, &context.action_records[5]),
        (HOT_EFFECT_RAW_ACCOUNT_V3, &context.action_records[6]),
        (HOT_LIFECYCLE_RAW_ACCOUNT_V3, &context.action_records[3]),
        (HOT_STRATEGY_RAW_ACCOUNT_V3, &context.action_records[4]),
        (HOT_PRODUCT_RAW_ACCOUNT_V3, &context.product),
        (HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, &context.result_domain),
        (HOT_PORTFOLIO_RAW_ACCOUNT_V3, &context.portfolio),
        (HOT_LINKED_BASIS_RAW_ACCOUNT_V3, &context.basis),
    ] {
        place(index, record.key.raw)?;
        place(index + 1, record.key.staging)?;
    }
    place(HOT_ACTIVATION_CACHE_ACCOUNT_V3, programs.activation_cache)?;
    place(HOT_CORE_PROGRAM_ACCOUNT_V3, core.program)?;
    place(HOT_CORE_PROGRAMDATA_ACCOUNT_V3, core.programdata)?;
    place(HOT_TRADING_PROGRAM_ACCOUNT_V3, trading.program)?;
    place(HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, trading.programdata)?;
    place(HOT_REGISTRY_PROGRAM_ACCOUNT_V3, programs.registry)?;
    place(HOT_RENT_SYSVAR_ACCOUNT_V3, sysvar::rent::ID.to_bytes())?;
    place(
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        sysvar::instructions::ID.to_bytes(),
    )?;
    place(HOT_CAPABILITY_SEAL_ACCOUNT_V3, seal.to_bytes())?;
    if fixed_accounts.iter().any(|key| *key == Pubkey::default()) {
        return Err(StructuredLifecycleConstructionErrorV1::Deployment);
    }
    let hot_outer = crate::rational_lifecycle_hot::CheckedRationalLifecycleHotOuterV3 {
        trading_program: Pubkey::new_from_array(trading.program),
        artifact_release: hash(&trading.activated.release().to_bytes()).to_bytes(),
        checked_manifest_digest: context
            .checked_release
            .checked_execution_release_set_id()
            .map_err(|_| StructuredLifecycleConstructionErrorV1::Deployment)?
            .to_bytes(),
    };
    Ok(StructuredLifecycleSelectedPreparationV1 {
        bundle,
        fixed_accounts,
        seal_key,
        hot_outer,
    })
}

/// Every non-derived account in the fixed receipt Claims frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredReceiptPhysicalAccountsV1 {
    /// Current Trading program.
    pub trading: Pubkey,
    /// Current Trading ProgramData.
    pub trading_programdata: Pubkey,
    /// Current Claims program.
    pub claims: Pubkey,
    /// Current Claims ProgramData.
    pub claims_programdata: Pubkey,
    /// Immutable Registry program.
    pub registry: Pubkey,
    /// Activated release-set cache.
    pub activation_cache: Pubkey,
    /// Finalized representation descriptor record.
    pub descriptor_raw: Pubkey,
    /// Vacant representation descriptor staging cursor.
    pub descriptor_staging: Pubkey,
    /// Claims-derived representation authority.
    pub representation_authority: Pubkey,
    /// Closeable receipt Mint.
    pub receipt_mint: Pubkey,
    /// Lifecycle-scoped RentCredit.
    pub rent_credit: Pubkey,
    /// dClutch Rent program.
    pub rent_program: Pubkey,
    /// Claims liability-basis Market.
    pub claims_market: Pubkey,
    /// Core Market.
    pub core_market: Pubkey,
    /// Current Core program.
    pub core: Pubkey,
    /// Current Core ProgramData.
    pub core_programdata: Pubkey,
}

/// The additional accounts in one coordinate lifecycle Claims frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCoordinatePhysicalAccountsV1 {
    /// Fixed receipt/common accounts.
    pub common: StructuredReceiptPhysicalAccountsV1,
    /// Claims child-authority PDA derived for this coordinate request.
    pub child_authority: Pubkey,
    /// Claims liability-basis Position.
    pub position: Pubkey,
    /// Protocol Position admission record.
    pub admission: Pubkey,
    /// Closeable shard Mint.
    pub shard_mint: Pubkey,
    /// Claims-derived Structured custody token account.
    pub structured_custody: Pubkey,
    /// Claims custody-owner PDA.
    pub owner: Pubkey,
    /// Finalized ProductBasis record.
    pub basis_record: Pubkey,
    /// Vacant ProductBasis staging cursor.
    pub basis_staging: Pubkey,
    /// Finalized Product record.
    pub product_record: Pubkey,
    /// Vacant Product staging cursor.
    pub product_staging: Pubkey,
    /// Finalized ResultDomain record.
    pub result_record: Pubkey,
    /// Vacant ResultDomain staging cursor.
    pub result_staging: Pubkey,
    /// Finalized Portfolio record.
    pub portfolio_record: Pubkey,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: Pubkey,
}

/// Construct the exact Claims child for receipt activation.
pub fn activate_receipt_claims_instruction_v1(
    accounts: StructuredReceiptPhysicalAccountsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    receipt_claims_instruction_v1(
        accounts,
        lifecycle_bytes,
        LifecycleActionV2::ActivateReceipt,
    )
}

/// Construct the exact Claims child for complete-support receipt retirement.
pub fn retire_receipt_claims_instruction_v1(
    accounts: StructuredReceiptPhysicalAccountsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    receipt_claims_instruction_v1(accounts, lifecycle_bytes, LifecycleActionV2::RetireReceipt)
}

fn receipt_claims_instruction_v1(
    accounts: StructuredReceiptPhysicalAccountsV1,
    lifecycle_bytes: &[u8],
    expected_action: LifecycleActionV2,
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    let request = LifecycleRequestV2::decode(lifecycle_bytes)
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    let header = request.header();
    if header.action != expected_action
        || !matches!(
            header.action,
            LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt
        )
        || (header.action == LifecycleActionV2::ActivateReceipt && header.coordinate_count != 0)
        || (header.action == LifecycleActionV2::RetireReceipt && header.coordinate_count == 0)
    {
        return Err(StructuredLifecycleConstructionErrorV1::Action);
    }
    if header.representation_authority != accounts.representation_authority.to_bytes()
        || header.receipt_mint != accounts.receipt_mint.to_bytes()
        || header.rent_credit != accounts.rent_credit.to_bytes()
        || header.rent_program != accounts.rent_program.to_bytes()
        || header.market != accounts.core_market.to_bytes()
    {
        return Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame);
    }
    let outer = caller_authority(accounts.trading, lifecycle_bytes, request)?;
    let mut metas = vec![
        AccountMeta::new_readonly(outer, true),
        AccountMeta::new_readonly(accounts.trading, false),
        AccountMeta::new_readonly(accounts.trading_programdata, false),
        AccountMeta::new_readonly(accounts.claims, false),
        AccountMeta::new_readonly(accounts.claims_programdata, false),
        AccountMeta::new_readonly(accounts.registry, false),
        AccountMeta::new_readonly(accounts.activation_cache, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(accounts.descriptor_raw, false),
        AccountMeta::new_readonly(accounts.descriptor_staging, false),
        AccountMeta::new_readonly(accounts.representation_authority, false),
        AccountMeta::new(accounts.receipt_mint, false),
        AccountMeta::new_readonly(header.token_program.into(), false),
        if header.action == LifecycleActionV2::RetireReceipt {
            AccountMeta::new(accounts.rent_credit, false)
        } else {
            AccountMeta::new_readonly(accounts.rent_credit, false)
        },
        AccountMeta::new_readonly(accounts.rent_program, false),
        AccountMeta::new_readonly(accounts.claims_market, false),
        AccountMeta::new_readonly(accounts.core_market, false),
        AccountMeta::new_readonly(accounts.core, false),
        AccountMeta::new_readonly(accounts.core_programdata, false),
    ];
    if header.action == LifecycleActionV2::RetireReceipt {
        for row in request.coordinates() {
            let row = row.map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
            metas.extend(
                [
                    row.shard_mint,
                    row.structured_custody_account,
                    row.claims_custody_owner,
                    row.claims_custody_position,
                    row.position_admission,
                ]
                .into_iter()
                .map(|key| AccountMeta::new_readonly(Pubkey::new_from_array(key), false)),
            );
        }
    }
    Ok(Instruction {
        program_id: accounts.claims,
        accounts: metas,
        data: lifecycle_bytes.to_vec(),
    })
}

/// Construct the exact Claims child for one coordinate activation or retirement.
pub fn coordinate_claims_instruction_v1(
    accounts: StructuredCoordinatePhysicalAccountsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    let request = LifecycleRequestV2::decode(lifecycle_bytes)
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    let header = request.header();
    if !matches!(
        header.action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    ) || header.coordinate_count != 1
    {
        return Err(StructuredLifecycleConstructionErrorV1::Action);
    }
    let mut coordinates = request.coordinates();
    let row = coordinates
        .next()
        .ok_or(StructuredLifecycleConstructionErrorV1::Coordinate)?
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    if coordinates.next().is_some() {
        return Err(StructuredLifecycleConstructionErrorV1::Coordinate);
    }
    if header.representation_authority != accounts.common.representation_authority.to_bytes()
        || header.receipt_mint != accounts.common.receipt_mint.to_bytes()
        || header.rent_credit != accounts.common.rent_credit.to_bytes()
        || header.rent_program != accounts.common.rent_program.to_bytes()
        || header.market != accounts.common.core_market.to_bytes()
        || row.claims_custody_position != accounts.position.to_bytes()
        || row.position_admission != accounts.admission.to_bytes()
        || row.shard_mint != accounts.shard_mint.to_bytes()
        || row.structured_custody_account != accounts.structured_custody.to_bytes()
        || row.claims_custody_owner != accounts.owner.to_bytes()
    {
        return Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame);
    }
    let outer = caller_authority(accounts.common.trading, lifecycle_bytes, request)?;
    let mut metas = vec![
        AccountMeta::new_readonly(outer, true),
        AccountMeta::new_readonly(accounts.common.trading, false),
        AccountMeta::new_readonly(accounts.common.trading_programdata, false),
        AccountMeta::new_readonly(accounts.common.claims, false),
        AccountMeta::new_readonly(accounts.common.claims_programdata, false),
        AccountMeta::new_readonly(accounts.common.registry, false),
        AccountMeta::new_readonly(accounts.common.activation_cache, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(accounts.common.descriptor_raw, false),
        AccountMeta::new_readonly(accounts.common.descriptor_staging, false),
        AccountMeta::new_readonly(accounts.common.representation_authority, false),
        AccountMeta::new_readonly(accounts.common.receipt_mint, false),
        AccountMeta::new_readonly(header.token_program.into(), false),
        if header.action == LifecycleActionV2::RetireCoordinate {
            AccountMeta::new(accounts.common.rent_credit, false)
        } else {
            AccountMeta::new_readonly(accounts.common.rent_credit, false)
        },
        AccountMeta::new_readonly(accounts.common.rent_program, false),
        AccountMeta::new_readonly(accounts.common.claims_market, false),
        AccountMeta::new_readonly(accounts.common.core_market, false),
        AccountMeta::new_readonly(accounts.common.core, false),
        AccountMeta::new_readonly(accounts.common.core_programdata, false),
        AccountMeta::new_readonly(accounts.child_authority, true),
        AccountMeta::new(accounts.position, false),
        AccountMeta::new(accounts.admission, false),
        AccountMeta::new(accounts.shard_mint, false),
        AccountMeta::new(accounts.structured_custody, false),
        AccountMeta::new_readonly(accounts.owner, false),
        AccountMeta::new_readonly(accounts.basis_record, false),
        AccountMeta::new_readonly(accounts.basis_staging, false),
        AccountMeta::new_readonly(accounts.product_record, false),
        AccountMeta::new_readonly(accounts.product_staging, false),
        AccountMeta::new_readonly(accounts.result_record, false),
        AccountMeta::new_readonly(accounts.result_staging, false),
        AccountMeta::new_readonly(accounts.portfolio_record, false),
        AccountMeta::new_readonly(accounts.portfolio_staging, false),
    ];
    // Spell these privileges after construction so the two semantic CPI
    // authorities remain visible to reviewers and cannot drift independently.
    metas[0].is_signer = true;
    metas[20].is_signer = true;
    Ok(Instruction {
        program_id: accounts.common.claims,
        accounts: metas,
        data: lifecycle_bytes.to_vec(),
    })
}

fn caller_authority(
    trading: Pubkey,
    lifecycle_bytes: &[u8],
    request: LifecycleRequestV2<'_>,
) -> Result<Pubkey, StructuredLifecycleConstructionErrorV1> {
    let header = request.header();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        header.parent_context,
        hash(lifecycle_bytes).to_bytes(),
    )
    .map_err(StructuredLifecycleConstructionErrorV1::CallerAuthority)?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &trading).0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::rational_lifecycle::{
        LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2, LifecycleCoordinateV2,
        LifecycleHeaderV2,
    };
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn common() -> StructuredReceiptPhysicalAccountsV1 {
        StructuredReceiptPhysicalAccountsV1 {
            trading: key(20),
            trading_programdata: key(21),
            claims: key(22),
            claims_programdata: key(23),
            registry: key(24),
            activation_cache: key(25),
            descriptor_raw: key(26),
            descriptor_staging: key(27),
            representation_authority: key(6),
            receipt_mint: key(7),
            rent_credit: key(8),
            rent_program: key(9),
            claims_market: key(28),
            core_market: key(2),
            core: key(29),
            core_programdata: key(30),
        }
    }

    fn header(action: LifecycleActionV2, coordinate_count: u32) -> LifecycleHeaderV2 {
        LifecycleHeaderV2 {
            action,
            release_set: key(1).to_bytes(),
            market: key(2).to_bytes(),
            graph_id: key(3).to_bytes(),
            descriptor_id: key(4).to_bytes(),
            parent_context: key(5).to_bytes(),
            representation_authority: key(6).to_bytes(),
            receipt_mint: key(7).to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            rent_credit: key(8).to_bytes(),
            rent_program: key(9).to_bytes(),
            generation: 10,
            expected_claims_market_revision: 11,
            observed_receipt_lamports: 12,
            receipt_rent_principal: 12,
            expected_receipt_supply: 0,
            outcome_count: 3,
            coordinate_count,
            rent_credit_before: 13,
            rent_credit_after: 13,
        }
    }

    fn lifecycle_bytes(
        action: LifecycleActionV2,
        coordinate: Option<LifecycleCoordinateV2>,
    ) -> Vec<u8> {
        let mut rows = Vec::new();
        if let Some(coordinate) = coordinate {
            let mut encoded = [0; LIFECYCLE_COORDINATE_BYTES_V2];
            coordinate.encode_into(&mut encoded).expect("coordinate");
            rows.extend_from_slice(&encoded);
        }
        let request =
            LifecycleRequestV2::new(header(action, u32::from(coordinate.is_some())), &rows)
                .expect("request");
        let mut bytes = vec![0; LIFECYCLE_HEADER_BYTES_V2 + rows.len()];
        request.encode_into(&mut bytes).expect("encode request");
        bytes
    }

    fn coordinate() -> LifecycleCoordinateV2 {
        LifecycleCoordinateV2 {
            outcome: 1,
            coefficient: 2,
            shard_mint: key(40).to_bytes(),
            structured_custody_account: key(41).to_bytes(),
            claims_custody_owner: key(42).to_bytes(),
            claims_custody_position: key(43).to_bytes(),
            position_admission: key(44).to_bytes(),
            observed_shard_lamports: 0,
            observed_structured_lamports: 0,
            observed_position_lamports: 0,
            observed_admission_lamports: 0,
            shard_rent_principal: 14,
            structured_rent_principal: 15,
            position_rent_principal: 16,
            admission_rent_principal: 17,
            expected_shard_supply: 0,
            expected_structured_amount: 0,
            expected_position_revision: 0,
        }
    }

    #[test]
    fn receipt_builder_owns_exact_frame_and_action() {
        let accounts = common();
        let bytes = lifecycle_bytes(LifecycleActionV2::ActivateReceipt, None);
        let instruction =
            activate_receipt_claims_instruction_v1(accounts, &bytes).expect("instruction");
        assert_eq!(instruction.program_id, accounts.claims);
        assert_eq!(instruction.accounts.len(), 20);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[12].is_writable);
        assert!(!instruction.accounts[14].is_writable);
        assert_eq!(instruction.data, bytes);

        let wrong = lifecycle_bytes(LifecycleActionV2::RetireReceipt, None);
        assert_eq!(
            activate_receipt_claims_instruction_v1(accounts, &wrong),
            Err(StructuredLifecycleConstructionErrorV1::Action)
        );
    }

    #[test]
    fn coordinate_builder_refuses_a_substituted_physical_coordinate() {
        let row = coordinate();
        let bytes = lifecycle_bytes(LifecycleActionV2::ActivateCoordinate, Some(row));
        let mut accounts = StructuredCoordinatePhysicalAccountsV1 {
            common: common(),
            child_authority: key(45),
            position: Pubkey::new_from_array(row.claims_custody_position),
            admission: Pubkey::new_from_array(row.position_admission),
            shard_mint: Pubkey::new_from_array(row.shard_mint),
            structured_custody: Pubkey::new_from_array(row.structured_custody_account),
            owner: Pubkey::new_from_array(row.claims_custody_owner),
            basis_record: key(46),
            basis_staging: key(47),
            product_record: key(48),
            product_staging: key(49),
            result_record: key(50),
            result_staging: key(51),
            portfolio_record: key(52),
            portfolio_staging: key(53),
        };
        let instruction = coordinate_claims_instruction_v1(accounts, &bytes).expect("instruction");
        assert_eq!(instruction.accounts.len(), 34);
        assert_eq!(
            instruction
                .accounts
                .iter()
                .filter(|meta| meta.is_signer)
                .count(),
            2
        );
        assert_eq!(instruction.accounts[20].pubkey, accounts.child_authority);

        accounts.position = key(54);
        assert_eq!(
            coordinate_claims_instruction_v1(accounts, &bytes),
            Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame)
        );
    }

    #[test]
    fn selected_parent_is_the_digest_of_the_native_family() {
        let mut coordinate_bytes = [0; LIFECYCLE_COORDINATE_BYTES_V2];
        coordinate()
            .encode_into(&mut coordinate_bytes)
            .expect("coordinate");
        let bound = bind_selected_lifecycle_parent_v6(
            header(LifecycleActionV2::ActivateCoordinate, 1),
            &coordinate_bytes,
        )
        .expect("bound header");
        assert_ne!(bound.parent_context, [0; 32]);
        assert_ne!(bound.parent_context, key(5).to_bytes());

        let request = LifecycleRequestV2::new(bound, &coordinate_bytes).expect("bound request");
        let mut family_bytes = vec![0; LIFECYCLE_HEADER_BYTES_V2 + coordinate_bytes.len()];
        let family =
            dclutch_claims::rational_lifecycle::hot_v6::RationalLifecycleHotRequestV6::from_child_into(
                request,
                &mut family_bytes,
            )
            .expect("family");
        assert_eq!(bound.parent_context, hash(family.as_bytes()).to_bytes());

        coordinate_bytes[0] ^= 1;
        let changed = bind_selected_lifecycle_parent_v6(
            header(LifecycleActionV2::ActivateCoordinate, 1),
            &coordinate_bytes,
        )
        .expect("changed bound header");
        assert_ne!(bound.parent_context, changed.parent_context);
    }

    #[test]
    fn poststate_verifier_applies_the_fee_only_to_the_exact_payer() {
        let payer = key(70).to_bytes();
        let value = StructuredLifecycleAccountValueV1 {
            owner: system_program::ID.to_bytes(),
            lamports: 100,
            executable: false,
            space: 0,
            data: Vec::new(),
        };
        let plan = StructuredLifecyclePlanV1 {
            intent: StructuredLifecycleIntentV1 {
                market: key(71).to_bytes(),
                payer,
                action: LifecycleActionV2::ActivateReceipt,
                coordinate: None,
                expected_position: None,
                selected_capability: Some(key(72).to_bytes()),
                representation_descriptor: Some(key(73).to_bytes()),
            },
            step_id: key(74).to_bytes(),
            step_kind: StructuredLifecycleStepKindV1::SealArtifact,
            selected_capability: key(72).to_bytes(),
            finalized_slot: 10,
            instructions: Vec::new(),
            required_wallet_signers: vec![payer],
            preview: StructuredLifecyclePreviewV1 {
                receipt_mint: key(75).to_bytes(),
                coordinate: None,
                position: None,
                preparation_lamports: 0,
                returned_rent_lamports: 0,
                rent_recipient: None,
                receipt_supply_before: 0,
                receipt_supply_after: 0,
            },
            expected_poststates: vec![StructuredLifecycleExpectedAccountV1 {
                address: payer,
                value: Some(value.clone()),
                deduct_transaction_fee: true,
            }],
        };
        let snapshot = StructuredLifecycleSnapshotV1 {
            slot: 11,
            accounts: vec![StructuredLifecycleAccountV1 {
                request: StructuredLifecycleAccountRequestV1 {
                    address: payer,
                    data_slice: None,
                },
                value: Some(StructuredLifecycleAccountValueV1 {
                    lamports: 93,
                    ..value
                }),
            }],
            scans: Vec::new(),
        };
        assert_eq!(
            verify_structured_lifecycle_poststates_v1(&plan, &snapshot, 7),
            Ok(())
        );
        assert_eq!(
            verify_structured_lifecycle_poststates_v1(&plan, &snapshot, 6),
            Err(StructuredLifecycleConstructionErrorV1::Poststate)
        );
        let mut wrong_payer = plan;
        wrong_payer.intent.payer = key(76).to_bytes();
        assert_eq!(
            verify_structured_lifecycle_poststates_v1(&wrong_payer, &snapshot, 7),
            Err(StructuredLifecycleConstructionErrorV1::Poststate)
        );
    }
}
