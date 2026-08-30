//! The Market's resolution, driven on a real validator.
//!
//! JRNY-1 stated this stage as a gap and said the gap was "a missing campaign,
//! not a closed door": `edfcb24` admitted the `(Open, Consumed)` prestate that
//! `DCLTGMF1`'s commit-last Open leaves behind, and `60a2101` walked it end to
//! end against the compiled ELFs. What was missing was a campaign that composed
//! the ladder against a chain. This module is that campaign, and the reason it
//! can exist at all is that every step of the ladder is CHAIN-DERIVED: each
//! builder in `dclutch-resolution-core-v3-operator` takes `ObservedAccount`s and
//! returns one exact `Instruction`, and `rpc.rs`'s
//! `finalized_observed_accounts` already produces that exact type — the RPC
//! reader and the operator share one `Observation` definition rather than two
//! that agree today.
//!
//! **No account here is a signer.** `create_accounts`/`verify_accounts` in the
//! operator hand back frames whose every `AccountMeta` is `is_signer: false`,
//! so the whole funding ladder is wallet-constructible: a fee payer and nothing
//! else. That is the difference between this stage and the Claims/Custody
//! stages, which need a program to sign its own PDA and are therefore behind
//! the Hot gate no matter what state the Market is in.
//!
//! This module owns the FUNDING half of the ladder. The provider half -- one
//! Pyth update through the real receiver ELF, then the Core-driven execution
//! that mints the terminal certificate -- is `provider.rs`, because it also has
//! to bootstrap two captured third-party programs and that is a different kind
//! of work from deriving a Core effect.

use std::collections::BTreeMap;

use dclutch_capability_contract::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerStatusV2, FundingLedgerV2, funding_ledger_bytes_v2,
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity as CoreIdentity, Phase, Readiness, Request,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V7,
    SOURCE_CLOSURE_RECEIPT_BYTES_V3, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3, SourceClosureReceiptV3,
};
use dclutch_resolution_core_v3_operator::{
    Observation, ObservedAccount, ResolutionCloseFundSnapshotV3, ResolutionCreateFundSnapshotV3,
    ResolutionVerifyFundReadySnapshotV3, build_resolution_close_fund_v3,
    build_resolution_create_fund_v3, build_resolution_verify_fund_ready_v3,
    validate_resolution_close_fund_report_v3, validate_resolution_create_fund_report_v3,
    validate_resolution_verify_fund_ready_report_v3,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, RECOVERY_POLICY_SCHEMA_ID_V2,
    RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1, SourceMaterialV3,
    SourceResolutionPhaseV1, SourceResolutionStateV2, WINDOW_SPEC_SCHEMA_ID_V1,
};
use solana_program::hash::hash;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    Error, Result,
    ledger::ConservationLedgerV1,
    model::{AccountEvidence, SuccessorPlan, TransactionEvidence},
    plan::pubkey,
    rpc::Rpc,
    stages::{MarketAddressesV1, StageReportV1},
};

use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;

/// Every address the resolution ladder consumes, derived once from the
/// founding's own evidence.
///
/// These are computed before the first conservation-ledger census so that the
/// ledger watches them from a boundary at which they do not yet exist. A census
/// that first meets an account already holding lamports has no predecessor to
/// difference against, and L7 says so out loud rather than counting the whole
/// balance as growth.
pub(crate) struct ResolutionAddressesV1 {
    pub(crate) market: Pubkey,
    pub(crate) generation: u64,
    pub(crate) activation_cache: Pubkey,
    pub(crate) registry_program: Pubkey,
    pub(crate) core_program: Pubkey,
    pub(crate) core_programdata: Pubkey,
    pub(crate) resolution_program: Pubkey,
    pub(crate) resolution_programdata: Pubkey,
    pub(crate) source_material: RecordPairV1,
    pub(crate) capability_manifest: RecordPairV1,
    pub(crate) recovery_policy: RecordPairV1,
    /// The five source-graph records the provider legs authenticate, plus the
    /// three Product-graph records terminal admission reads. Every one is
    /// derived from its own body's digest and cross-checked against the address
    /// the founding recorded, so a record that moved is a named mismatch here
    /// rather than an unattributable refusal three transactions later.
    pub(crate) source_spec: RecordPairV1,
    pub(crate) window_spec: RecordPairV1,
    pub(crate) statistic_spec: RecordPairV1,
    pub(crate) provider_release: RecordPairV1,
    pub(crate) adapter_config: RecordPairV1,
    pub(crate) product: RecordPairV1,
    pub(crate) result_domain: RecordPairV1,
    pub(crate) portfolio: RecordPairV1,
    /// The infrastructure plan's Pyth release record, published before any
    /// Market existed. Its raw coordinate only; the provider legs never read a
    /// staging cursor for it.
    pub(crate) pyth_release: Pubkey,
    pub(crate) source_state: Pubkey,
    /// Resolution-owned subset ledger containing recovery, exhaustion, and failure rows.
    pub(crate) funding: Pubkey,
    pub(crate) funding_entry_indices: [u16; 3],
    pub(crate) rent_beneficiary: Pubkey,
    /// The terminal certificate this Market's first terminal sequence would
    /// occupy. Watched from the start so the ledger can see it stay vacant.
    pub(crate) certificate: Pubkey,
    /// The Source closure receipt the retirement stage prepays and CloseFund
    /// writes. One sequence past the terminal certificate's.
    pub(crate) closure_receipt: Pubkey,
}

/// One finalized Registry record's two coordinates.
#[derive(Clone, Copy)]
pub(crate) struct RecordPairV1 {
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

impl RecordPairV1 {
    /// Derive both coordinates from the schema and the body digest.
    ///
    /// A record's identity IS the hash of its body, so this is a derivation and
    /// not a lookup: passing the founding's recorded raw address back in as a
    /// cross-check is how a moved record surfaces as a mismatch rather than as
    /// a later refusal nobody can attribute.
    fn derive(registry: Pubkey, schema: [u8; 32], body: &[u8]) -> Self {
        let digest = hash(body).to_bytes();
        Self {
            raw: Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0,
            staging: Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0,
        }
    }
}

/// Derive every resolution coordinate from the founding's evidence.
///
/// The subset-ledger destination is re-derived here from the manifest, which
/// duplicates a private selection inside the operator. That duplication is safe
/// in exactly one direction: `build_resolution_create_fund_v3` re-derives the
/// address itself and REFUSES if the supplied account is not the one it
/// computes, so an error here becomes a refusal rather than a wrong ledger.
pub(crate) fn derive(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    addresses: &MarketAddressesV1,
    evidence: &BTreeMap<String, AccountEvidence>,
) -> Result<ResolutionAddressesV1> {
    let registry_program = pubkey(&plan.registry.program_id)?;
    let resolution_program = pubkey(&plan.resolution.program_id)?;

    let market_account = rpc.required_account(addresses.founding_market, "founded Market")?;
    let market = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("founded Market: {error:?}")))?;
    let generation = market.identity.generation;

    // Every finalized record is located the same way: read the body the
    // founding recorded, derive both coordinates from its digest, and REFUSE if
    // the derived raw address is not the one the founding named. That check is
    // the whole reason to derive rather than to look up -- a record whose
    // address is not its own content identity is not a finalized record, and
    // this is where that stops being true quietly.
    let located = |rpc: &mut Rpc,
                   label: &str,
                   schema: [u8; 32]|
     -> Result<(RecordPairV1, Vec<u8>)> {
        let recorded = evidence_address(evidence, label)?;
        let body = rpc.required_account(recorded, label)?.data;
        let pair = RecordPairV1::derive(registry_program, schema, &body);
        if pair.raw != recorded {
            return Err(Error::new(format!(
                "{label} sits at {recorded} but its own body hashes to a record at {}; a record \
                 whose address is not its content identity is not a finalized record",
                pair.raw
            )));
        }
        Ok((pair, body))
    };
    let (source_material, source_material_body) = located(
        rpc,
        "source_material_record",
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let (capability_manifest, manifest_body) = located(
        rpc,
        "capability_manifest_record",
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let (recovery_policy, recovery_body) =
        located(rpc, "recovery_policy_record", RECOVERY_POLICY_SCHEMA_ID_V2)?;
    let (source_spec, _) = located(rpc, "source_spec_record", SOURCE_SPEC_SCHEMA_ID_V1)?;
    let (window_spec, _) = located(rpc, "window_spec_record", WINDOW_SPEC_SCHEMA_ID_V1)?;
    let (statistic_spec, _) = located(rpc, "statistic_spec_record", STATISTIC_SPEC_SCHEMA_ID_V1)?;
    let (provider_release, _) = located(
        rpc,
        "provider_release_record",
        PROVIDER_RELEASE_SCHEMA_ID_V1,
    )?;
    let (adapter_config, _) = located(
        rpc,
        "pyth_adapter_config_record",
        PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
    )?;
    let (product, _) = located(rpc, "product_record", PRODUCT_RECORD_SCHEMA_ID_V2)?;
    let (result_domain, _) = located(rpc, "result_domain_record", RESULT_DOMAIN_SCHEMA_ID_V2)?;
    let (portfolio, _) = located(rpc, "portfolio_record", PORTFOLIO_SCHEMA_ID_V2)?;
    let pyth_release = crate::runtime::record(plan, "pyth_release")?.0;

    let material = SourceMaterialV3::decode(&source_material_body)
        .map_err(|error| Error::new(format!("SourceMaterialV3: {error:?}")))?;
    let policy = RecoveryPolicyV2::decode(&recovery_body)
        .map_err(|error| Error::new(format!("RecoveryPolicyV2: {error:?}")))?;
    let manifest = CapabilityManifestV1::decode(&manifest_body)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;

    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            addresses.founding_market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;

    let funding_entry_indices = select_funding_entries(&material, &policy, manifest)?;
    let manifest_id = CapabilityContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|error| Error::new(format!("Market capability manifest identity: {error:?}")))?;
    let selected_mask = funding_entry_indices
        .into_iter()
        .fold(0_u16, |mask, entry_index| mask | (1_u16 << entry_index));
    let mut ledger_bytes = vec![
        0_u8;
        funding_ledger_bytes_v2(3).map_err(|error| Error::new(format!(
            "FundingLedgerV2 width: {error:?}"
        )))?
    ];
    FundingLedgerV2::initialize(&mut ledger_bytes, manifest_id, manifest, selected_mask)
        .map_err(|error| Error::new(format!("pending FundingLedgerV2: {error:?}")))?;
    let ledger = FundingLedgerV2::decode(&ledger_bytes)
        .map_err(|error| Error::new(format!("FundingLedgerV2: {error:?}")))?;
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        resolution_program.to_bytes(),
        addresses.founding_market.to_bytes(),
        generation,
        manifest_id,
        ledger,
    )
    .map_err(|error| Error::new(format!("funding-ledger derivation: {error:?}")))?;
    let funding =
        Pubkey::find_program_address(&derivation.seed_components(), &resolution_program).0;

    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &[1],
            &1_u64.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;

    let closure_receipt = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &2_u64.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;

    Ok(ResolutionAddressesV1 {
        market: addresses.founding_market,
        generation,
        activation_cache: pubkey(&plan.activation)?,
        registry_program,
        core_program: pubkey(&plan.core.program_id)?,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        resolution_program,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        source_material,
        capability_manifest,
        recovery_policy,
        source_spec,
        window_spec,
        statistic_spec,
        provider_release,
        adapter_config,
        product,
        result_domain,
        portfolio,
        pyth_release,
        source_state,
        funding,
        funding_entry_indices,
        rent_beneficiary: Pubkey::new_from_array(market.rent_beneficiary.to_bytes()),
        certificate,
        closure_receipt,
    })
}

/// Register every resolution coordinate with the conservation ledger.
pub(crate) fn watch(ledger: &mut ConservationLedgerV1, addresses: &ResolutionAddressesV1) {
    for (label, address) in [
        ("resolution_source_state", addresses.source_state),
        ("resolution_funding_subset_ledger", addresses.funding),
        ("resolution_terminal_certificate", addresses.certificate),
        ("resolution_closure_receipt", addresses.closure_receipt),
        ("resolution_rent_beneficiary", addresses.rent_beneficiary),
    ] {
        ledger.watch(label, address);
    }
}

/// Create and activate this Market's Resolution funding, then say exactly how
/// far the provider legs can go.
pub(crate) fn resolve(
    rpc: &mut Rpc,
    payer: &Keypair,
    addresses: &ResolutionAddressesV1,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(StageReportV1, crate::ledger::LamportClaimV1)> {
    let state = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    if state.phase != Phase::Open || state.readiness != Readiness::Consumed {
        return Err(Error::new(format!(
            "the resolution ladder expects the atomic founding's poststate (Open + Consumed) and \
             the chain holds ({:?}, {:?})",
            state.phase, state.readiness
        )));
    }
    if state.terminal_receipt.is_some() {
        return Err(Error::new(
            "this Market already carries a terminal receipt; the campaign founded it in this \
             process and nothing else can have resolved it",
        ));
    }

    let mut fees = 0_u64;
    let mut submitted = 0_usize;
    let mut compute_units = 0_u64;

    // 1. Prepay only the vacant Source state. Founding already initialized and
    //    funded the canonical Resolution-owned subset ledger, whose three rows
    //    must still be Pending. CreateFund consumes that existing ledger; it
    //    neither creates it nor accepts a second funding transfer.
    let create = build_resolution_create_fund_v3(&create_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived CreateFund: {error:?}")))?;
    validate_resolution_create_fund_report_v3(&create)
        .map_err(|error| Error::new(format!("CreateFund report: {error:?}")))?;
    if create.source_top_up_lamports > 0 {
        let evidence = rpc.send(
            "journey: prepay the Source resolution state",
            &[transfer(
                &payer.pubkey(),
                &addresses.source_state,
                create.source_top_up_lamports,
            )],
            payer,
        )?;
        fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
        compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
        submitted += 1;
        transactions.push(evidence);
    }

    // Rebuild from the prepaid snapshot and insist the Source destination now
    // has exactly the required balance before publishing the routed frame.
    let create = build_resolution_create_fund_v3(&create_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived CreateFund after prepay: {error:?}")))?;
    validate_resolution_create_fund_report_v3(&create)
        .map_err(|error| Error::new(format!("CreateFund report after prepay: {error:?}")))?;
    if create.source_top_up_lamports != 0 {
        return Err(Error::new(format!(
            "the Source prepayment did not reach its exact target: {} lamports remain",
            create.source_top_up_lamports
        )));
    }

    // 2. The frame does not fit a legacy packet, and that is a measurement
    //    rather than an inconvenience: `CreateFund` carries eighteen accounts
    //    and a Core effect envelope. It rides
    //    a finalized address lookup table as a v0 transaction, exactly as
    //    Found31 and DCLTGMF1 do, through the producer's own table publisher --
    //    one author for the routing shape rather than a second copy of it here.
    let before_tables = transactions.len();
    let (routing, tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        "Resolution CreateFund",
        std::slice::from_ref(&create.instruction),
        transactions,
    )?;
    submitted += transactions.len().saturating_sub(before_tables);
    fees = fees.saturating_add(crate::provider::fees_since(transactions, before_tables));
    let mut table_lamports = crate::provider::table_rent(&tables);

    // 3. The honest creation consumes the already-existing Pending ledger.
    let evidence = rpc.send_v0_with_signers(
        "journey: an Open Market creates its Source state from its pending Resolution ledger",
        std::slice::from_ref(&create.instruction),
        payer,
        &[],
        routing,
        &tables,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(evidence);

    let source = SourceResolutionStateV2::decode(
        &rpc.required_account(addresses.source_state, "Source resolution state")?
            .data,
    )
    .map_err(|error| Error::new(format!("SourceResolutionStateV2: {error:?}")))?;
    if source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != addresses.market.to_bytes()
        || source.generation() != addresses.generation
    {
        return Err(Error::new(
            "the created Source resolution state does not bind this Market at this generation",
        ));
    }
    for (entry_index, status) in funding_statuses(rpc, addresses)? {
        if status != FundingLedgerStatusV2::Pending {
            return Err(Error::new(format!(
                "Resolution funding row {entry_index} is {status:?} immediately after creation, \
                 not Pending"
            )));
        }
    }

    // 4. Double create. The Source PDA is one per Market generation, and the
    //    prepaid-output rule refuses anything not System-owned and empty.
    let double = rpc.send_v0_expected_failure_with_signers(
        "journey: a second CreateFund at the same generation refuses",
        &[create.instruction],
        payer,
        &[],
        routing,
        &tables,
    )?;
    fees = fees.saturating_add(double.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(double.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(double);

    // 5. Activation. Core stays Open + Consumed: an already-open Market
    //    consumed its readiness at the commit-last Open, and the activation
    //    lives in the Resolution-owned subset ledger, which terminal admission
    //    rechecks. One semantic owner for that fact, not two.
    let verify = build_resolution_verify_fund_ready_v3(&verify_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived VerifyFundReady: {error:?}")))?;
    validate_resolution_verify_fund_ready_report_v3(&verify)
        .map_err(|error| Error::new(format!("VerifyFundReady report: {error:?}")))?;
    let beneficiary_before = rpc
        .account(addresses.rent_beneficiary)?
        .map(|account| account.lamports)
        .unwrap_or(0);
    // Same shape, same reason: nineteen accounts and an effect envelope do not
    // fit a legacy packet. A second table rather than a reused one, because
    // the two frames route different accounts and a table extended to cover
    // both would be a routing fact this campaign invented.
    let before_verify_tables = transactions.len();
    let (verify_routing, verify_tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        "Resolution VerifyFundReady",
        std::slice::from_ref(&verify.instruction),
        transactions,
    )?;
    submitted += transactions.len().saturating_sub(before_verify_tables);
    fees = fees.saturating_add(crate::provider::fees_since(
        transactions,
        before_verify_tables,
    ));
    table_lamports = table_lamports.saturating_add(crate::provider::table_rent(&verify_tables));
    let evidence = rpc.send_v0_with_signers(
        "journey: activate the Resolution subset ledger of an Open Market",
        std::slice::from_ref(&verify.instruction),
        payer,
        &[],
        verify_routing,
        &verify_tables,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(evidence);

    let activated = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    if activated.phase != Phase::Open || activated.readiness != Readiness::Consumed {
        return Err(Error::new(format!(
            "activation rewrote an already-open Market to ({:?}, {:?})",
            activated.phase, activated.readiness
        )));
    }
    let beneficiary_after = rpc
        .required_account(addresses.rent_beneficiary, "Market rent beneficiary")?
        .lamports;
    if beneficiary_after
        != beneficiary_before.saturating_add(verify.expected_beneficiary_credit_lamports)
    {
        return Err(Error::new(format!(
            "the Market's rent beneficiary moved from {beneficiary_before} to {beneficiary_after} \
             and the activation declared exactly {}",
            verify.expected_beneficiary_credit_lamports
        )));
    }
    for (entry_index, status) in funding_statuses(rpc, addresses)? {
        if status != FundingLedgerStatusV2::Active {
            return Err(Error::new(format!(
                "Resolution funding row {entry_index} is {status:?} after activation, not Active"
            )));
        }
    }

    Ok((
        StageReportV1 {
            stage: "resolution: create and activate the Market's Resolution funding".into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "The atomically founded Market consumed its existing Pending Resolution subset \
                 ledger to create the Source resolution state, then activated the ledger's three \
                 selected rows, on a real validator, with a fee payer and no other signer -- the \
                 funding ladder's whole frame is non-signing, which is why it is reachable at all \
                 while every Claims mutation is not. A second CreateFund at the same generation \
                 was proved to refuse. Manifest entries {:?} carry the recovery, exhaustion and \
                 failure compartments. The Market stayed Open + Consumed, and its rent beneficiary \
                 gained exactly the {} lamports the activation declared.",
                addresses.funding_entry_indices, verify.expected_beneficiary_credit_lamports
            ),
        },
        crate::ledger::LamportClaimV1::fees(fees).with_unwatched(
            table_lamports,
            "two address lookup tables, rent-funded to route CreateFund and VerifyFundReady past \
             the legacy packet limit",
        ),
    ))
}

fn funding_statuses(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
) -> Result<[(u16, FundingLedgerStatusV2); 3]> {
    let market = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    let manifest_account = rpc.required_account(
        addresses.capability_manifest.raw,
        "capability manifest record",
    )?;
    let manifest = CapabilityManifestV1::decode(&manifest_account.data)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    let manifest_id = CapabilityContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|error| Error::new(format!("Market capability manifest identity: {error:?}")))?;
    let funding_account = rpc.required_account(addresses.funding, "Resolution subset ledger")?;
    let funding = FundingLedgerV2::decode(&funding_account.data)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .map_err(|error| Error::new(format!("Resolution subset ledger: {error:?}")))?;
    let mut statuses = [(0_u16, FundingLedgerStatusV2::Pending); 3];
    for (status, entry_index) in statuses.iter_mut().zip(addresses.funding_entry_indices) {
        *status = (
            entry_index,
            funding
                .slot(entry_index)
                .map_err(|error| {
                    Error::new(format!("Resolution funding row {entry_index}: {error:?}"))
                })?
                .status(),
        );
    }
    Ok(statuses)
}

fn select_funding_entries(
    material: &SourceMaterialV3,
    policy: &RecoveryPolicyV2,
    manifest: CapabilityManifestV1<'_>,
) -> Result<[u16; 3]> {
    let recovery_policy = material
        .recovery_policy()
        .ok_or_else(|| Error::new("this Market's source material declares no recovery policy"))?;
    let expected = [
        policy
            .attempt(0)
            .map_err(|error| Error::new(format!("recovery attempt 0: {error:?}")))?
            .funding_allocation_id()
            .to_bytes(),
        recovery_policy.to_bytes(),
        hash(&material.to_bytes()).to_bytes(),
    ];
    let mut selected = [None; 3];
    for entry_index in 0..manifest.entry_count() {
        let entry = manifest
            .entry(entry_index)
            .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
        if entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7 {
            continue;
        }
        for (slot, config) in expected.iter().enumerate() {
            if entry.config_id().to_bytes() == *config
                && selected
                    .get_mut(slot)
                    .ok_or_else(|| Error::new("funding slot overflow"))?
                    .replace(entry_index)
                    .is_some()
            {
                return Err(Error::new(format!(
                    "two manifest entries carry the same Resolution funding configuration at slot \
                     {slot}"
                )));
            }
        }
    }
    let [recovery, exhaustion, failure] = selected;
    let entries = [
        recovery.ok_or_else(|| Error::new("no manifest entry funds the recovery attempt"))?,
        exhaustion.ok_or_else(|| Error::new("no manifest entry funds the recovery policy"))?,
        failure.ok_or_else(|| Error::new("no manifest entry funds the failure walk"))?,
    ];
    if entries[0] == entries[1] || entries[0] == entries[2] || entries[1] == entries[2] {
        return Err(Error::new(
            "one manifest entry was selected for two Resolution compartments",
        ));
    }
    Ok(entries)
}

fn create_snapshot(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
) -> Result<ResolutionCreateFundSnapshotV3> {
    let (observation, present) = rpc.finalized_observed_accounts(
        &[
            addresses.market,
            addresses.activation_cache,
            addresses.registry_program,
            addresses.core_program,
            addresses.core_programdata,
            addresses.resolution_program,
            addresses.resolution_programdata,
            addresses.source_material.raw,
            addresses.capability_manifest.raw,
            addresses.funding,
            sysvar::rent::ID,
            system_program::ID,
            addresses.recovery_policy.raw,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ResolutionCreateFundSnapshotV3 {
        market: at(0)?,
        activation_cache: at(1)?,
        registry_program: at(2)?,
        core_program: at(3)?,
        core_programdata: at(4)?,
        resolution_program: at(5)?,
        resolution_programdata: at(6)?,
        source_material: at(7)?,
        source_material_staging: vacant(observation, addresses.source_material.staging),
        capability_manifest: at(8)?,
        capability_manifest_staging: vacant(observation, addresses.capability_manifest.staging),
        source_destination: observed_or_vacant(rpc, observation, addresses.source_state)?,
        funding_ledger: at(9)?,
        rent_sysvar: at(10)?,
        system_program: at(11)?,
        recovery_policy: at(12)?,
        recovery_policy_staging: vacant(observation, addresses.recovery_policy.staging),
    })
}

fn verify_snapshot(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
) -> Result<ResolutionVerifyFundReadySnapshotV3> {
    let (observation, present) = rpc.finalized_observed_accounts(
        &[
            addresses.market,
            addresses.activation_cache,
            addresses.registry_program,
            addresses.core_program,
            addresses.core_programdata,
            addresses.resolution_program,
            addresses.resolution_programdata,
            addresses.source_material.raw,
            addresses.capability_manifest.raw,
            addresses.source_state,
            addresses.funding,
            addresses.rent_beneficiary,
            sysvar::clock::ID,
            sysvar::rent::ID,
            addresses.recovery_policy.raw,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ResolutionVerifyFundReadySnapshotV3 {
        market: at(0)?,
        activation_cache: at(1)?,
        registry_program: at(2)?,
        core_program: at(3)?,
        core_programdata: at(4)?,
        resolution_program: at(5)?,
        resolution_programdata: at(6)?,
        source_material: at(7)?,
        source_material_staging: vacant(observation, addresses.source_material.staging),
        capability_manifest: at(8)?,
        capability_manifest_staging: vacant(observation, addresses.capability_manifest.staging),
        source_state: at(9)?,
        funding_ledger: at(10)?,
        beneficiary: at(11)?,
        clock_sysvar: at(12)?,
        rent_sysvar: at(13)?,
        recovery_policy: at(14)?,
        recovery_policy_staging: vacant(observation, addresses.recovery_policy.staging),
    })
}

/// A staging cursor that was never opened, or a destination not yet created.
///
/// The finalized record routine finalizes by CLOSING the staging cursor, so a
/// finalized record's cursor is genuinely a System-owned vacancy rather than an
/// account this campaign failed to read.
fn vacant(observation: Observation, key: Pubkey) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: system_program::ID,
        lamports: 0,
        executable: false,
        data: Vec::new(),
    }
}

fn observed_or_vacant(
    rpc: &mut Rpc,
    observation: Observation,
    key: Pubkey,
) -> Result<ObservedAccount> {
    Ok(match rpc.account(key)? {
        None => vacant(observation, key),
        Some(account) => ObservedAccount {
            observation,
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data,
        },
    })
}

fn evidence_address(evidence: &BTreeMap<String, AccountEvidence>, label: &str) -> Result<Pubkey> {
    let recorded = evidence.get(label).ok_or_else(|| {
        Error::new(format!(
            "the founding's evidence names no `{label}`; the journey cannot resolve a Market whose \
             record shape it does not recognise"
        ))
    })?;
    pubkey(&recorded.address)
}

/// Begin retiring the resolved Market and close its Source subtree.
///
/// `BeginRetiring` admits only `Phase::Terminal`, so this stage exists at all
/// only because the provider legs ran. Both routes here are permissionless and
/// non-signing, which is the same reason the funding ladder was reachable.
///
/// It stops short of the retirement itself, and where it stops is the finding:
/// see the note this returns.
pub(crate) fn retire(
    rpc: &mut Rpc,
    payer: &Keypair,
    addresses: &ResolutionAddressesV1,
    hoard: Pubkey,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(StageReportV1, crate::ledger::LamportClaimV1)> {
    let mut fees = 0_u64;
    let mut compute_units = 0_u64;
    let mut submitted = 0_usize;

    let state = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    if state.phase != Phase::Terminal {
        return Ok((
            StageReportV1 {
                stage: "retirement: begin retiring and close the Source subtree".into(),
                outcome: "blocked".into(),
                transactions: 0,
                compute_units: 0,
                note: format!(
                    "BeginRetiring admits only Phase::Terminal and the Market is {:?}. The \
                     resolution stages above say why.",
                    state.phase
                ),
            },
            crate::ledger::LamportClaimV1::fees(0),
        ));
    }

    // BeginRetiring: five accounts, no signer, no lookup table. A Market that
    // has resolved may be retired by anyone -- the permission is the terminal
    // receipt, not a key.
    let request = Request::administrative(
        Action::BeginRetiring,
        addresses.generation,
        CoreIdentity::new(addresses.market.to_bytes())
            .map_err(|error| Error::new(format!("Market identity: {error:?}")))?,
    );
    let evidence = rpc.send(
        "journey: a resolved Market begins retiring",
        &[Instruction {
            program_id: addresses.core_program,
            accounts: vec![
                AccountMeta::new(addresses.market, false),
                AccountMeta::new_readonly(addresses.activation_cache, false),
                AccountMeta::new_readonly(addresses.registry_program, false),
                AccountMeta::new_readonly(addresses.core_program, false),
                AccountMeta::new_readonly(addresses.core_programdata, false),
            ],
            data: request
                .encode()
                .map_err(|error| Error::new(format!("BeginRetiring request: {error:?}")))?
                .to_vec(),
        }],
        payer,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(evidence);

    let retiring = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    if retiring.phase != Phase::Retiring {
        return Err(Error::new(format!(
            "BeginRetiring left the Market at {:?}, not Retiring",
            retiring.phase
        )));
    }

    // CloseFund closes the Source subtree and writes the closure receipt the
    // retirement itself consumes. Prepaid in its own transaction, same rule as
    // every other precommitted output.
    let closure_rent = rpc.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3)?;
    let evidence = rpc.send(
        "journey: prepay the Source closure receipt",
        &[transfer(
            &payer.pubkey(),
            &addresses.closure_receipt,
            closure_rent,
        )],
        payer,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(evidence);

    let close = build_resolution_close_fund_v3(&close_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived CloseFund: {error:?}")))?;
    validate_resolution_close_fund_report_v3(&close)
        .map_err(|error| Error::new(format!("CloseFund report: {error:?}")))?;
    if close.closure_receipt != addresses.closure_receipt {
        return Err(Error::new(format!(
            "the operator derives the closure receipt at {} and this campaign prepaid {}",
            close.closure_receipt, addresses.closure_receipt
        )));
    }
    let before_tables = transactions.len();
    let (routing, tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        "Resolution CloseFund",
        std::slice::from_ref(&close.instruction),
        transactions,
    )?;
    submitted += transactions.len().saturating_sub(before_tables);
    fees = fees.saturating_add(crate::provider::fees_since(transactions, before_tables));
    let table_lamports = crate::provider::table_rent(&tables);
    let beneficiary_before = rpc
        .account(addresses.rent_beneficiary)?
        .map(|account| account.lamports)
        .unwrap_or(0);
    let evidence = rpc.send_v0_with_signers(
        "journey: Resolution closes the Source subtree of a retiring Market",
        std::slice::from_ref(&close.instruction),
        payer,
        &[],
        routing,
        &tables,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(evidence);

    let receipt = SourceClosureReceiptV3::decode(
        &rpc.required_account(addresses.closure_receipt, "Source closure receipt")?
            .data,
    )
    .map_err(|error| Error::new(format!("SourceClosureReceiptV3: {error:?}")))?;
    if receipt.market != addresses.market.to_bytes() {
        return Err(Error::new(
            "the Source closure receipt does not bind the Market that was closed",
        ));
    }
    for (label, observed, expected) in [
        (
            "Source-state refund",
            receipt.source_refund_lamports,
            close.source_refund_lamports,
        ),
        (
            "remaining native ledger principal",
            receipt.ledger_remaining_native_principal,
            close.ledger_remaining_native_principal,
        ),
        (
            "ledger rent reserve",
            receipt.ledger_rent_lamports,
            close.ledger_rent_lamports,
        ),
        (
            "ledger lamport surplus",
            receipt.ledger_lamport_surplus,
            close.ledger_lamport_surplus,
        ),
        (
            "total beneficiary refund",
            receipt.refund_lamports,
            close.expected_refund_lamports,
        ),
    ] {
        if observed != expected {
            return Err(Error::new(format!(
                "the V3 Source closure receipt commits {observed} lamports for {label}, while the \
                 chain-derived CloseFund report declares {expected}"
            )));
        }
    }
    let beneficiary_after = rpc
        .required_account(addresses.rent_beneficiary, "Market rent beneficiary")?
        .lamports;
    if beneficiary_after != beneficiary_before.saturating_add(close.expected_refund_lamports) {
        return Err(Error::new(format!(
            "the closure refunded the beneficiary {} lamports and the operator declared {}",
            beneficiary_after.saturating_sub(beneficiary_before),
            close.expected_refund_lamports
        )));
    }
    if rpc.account(addresses.funding)?.is_some() {
        return Err(Error::new(
            "the Resolution subset ledger still exists after the Source subtree was closed",
        ));
    }

    // Where it stops, and why. The retirement itself is one atomic Registry
    // continuation that closes the Claims aggregate, the Custody replay and the
    // Hoard vault together, and `build_market_retirement_v1` REFUSES to compile
    // it while the Hoard holds a single atom -- partial Custody settlement
    // cannot retire, which is the correct rule and not a wall to route around.
    // This Market's Hoard holds the whole founding principal, because emptying
    // it means redeeming, and redemption is a Claims mutation behind the Hot
    // gate. So the last step of the Market's life is behind the same door as
    // the middle of it, and this stage says so with the Hoard's actual balance
    // rather than by reading the operator.
    let hoard_atoms = rpc
        .account(hoard)?
        .and_then(|account| {
            dclutch_token_svm::TokenAccount::parse(&account.data)
                .ok()
                .map(|parsed| parsed.amount)
        })
        .unwrap_or(0);
    Ok((
        StageReportV1 {
            stage: "retirement: begin retiring and close the Source subtree".into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "The resolved Market entered Retiring and its whole Source subtree is closed: the \
                 Resolution subset ledger is gone, the Source closure receipt binds this Market, \
                 and the Market's rent beneficiary gained exactly {} lamports: {} from the Source \
                 state, {} remaining native principal, {} of ledger rent, and {} of ledger \
                 surplus. Both routes are permissionless and non-signing -- a Market that has \
                 resolved may be retired by anyone, and the permission is the terminal receipt \
                 rather than a key. THE RETIREMENT ITSELF DOES NOT RUN, and the reason is measured \
                 rather than read: `build_market_retirement_v1` refuses to compile the atomic \
                 continuation while the Hoard holds a single atom (partial Custody settlement \
                 cannot retire), and this Hoard holds {hoard_atoms}. Emptying it means redeeming, \
                 redemption is a Claims mutation, and every Claims mutation is behind the Hot \
                 gate. So the LAST step of the Market's life is behind the same door as the \
                 middle of it -- which is worth saying plainly, because the retirement gap looked \
                 like it was behind the terminal receipt right up until the receipt existed.",
                close.expected_refund_lamports,
                close.source_refund_lamports,
                close.ledger_remaining_native_principal,
                close.ledger_rent_lamports,
                close.ledger_lamport_surplus,
            ),
        },
        crate::ledger::LamportClaimV1::fees(fees).with_unwatched(
            table_lamports,
            "one address lookup table, rent-funded to route CloseFund past the legacy packet limit",
        ),
    ))
}

fn close_snapshot(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
) -> Result<ResolutionCloseFundSnapshotV3> {
    let (observation, present) = rpc.finalized_observed_accounts(
        &[
            addresses.market,
            addresses.activation_cache,
            addresses.registry_program,
            addresses.core_program,
            addresses.core_programdata,
            addresses.resolution_program,
            addresses.resolution_programdata,
            addresses.source_material.raw,
            addresses.capability_manifest.raw,
            addresses.source_state,
            addresses.funding,
            addresses.certificate,
            addresses.closure_receipt,
            addresses.rent_beneficiary,
            sysvar::clock::ID,
            sysvar::rent::ID,
            system_program::ID,
            addresses.recovery_policy.raw,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ResolutionCloseFundSnapshotV3 {
        market: at(0)?,
        activation_cache: at(1)?,
        registry_program: at(2)?,
        core_program: at(3)?,
        core_programdata: at(4)?,
        resolution_program: at(5)?,
        resolution_programdata: at(6)?,
        source_material: at(7)?,
        source_material_staging: vacant(observation, addresses.source_material.staging),
        capability_manifest: at(8)?,
        capability_manifest_staging: vacant(observation, addresses.capability_manifest.staging),
        source_state: at(9)?,
        funding_ledger: at(10)?,
        certificate: at(11)?,
        closure_destination: at(12)?,
        beneficiary: at(13)?,
        clock_sysvar: at(14)?,
        rent_sysvar: at(15)?,
        system_program: at(16)?,
        recovery_policy: at(17)?,
        recovery_policy_staging: vacant(observation, addresses.recovery_policy.staging),
    })
}
