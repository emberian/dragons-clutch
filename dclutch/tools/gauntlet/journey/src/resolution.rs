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

use dclutch_market::capability_manifest::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerStatusV2, FundingLedgerV2, derive_funded_rent_rate_v2, funding_ledger_bytes_v2,
};
use dclutch_market::{CoreState, Phase, Readiness};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_resolution_core_v3_operator::{
    Observation, ObservedAccount, ResolutionAdmitTerminalSnapshotV3, ResolutionCoreOperatorErrorV3,
    ResolutionCreateFundSnapshotV3, ResolutionFundingCauseV3, ResolutionVerifyFundReadySnapshotV3,
    build_resolution_admit_terminal_v3, build_resolution_create_fund_v3,
    build_resolution_verify_fund_ready_v3, select_resolution_funding_entries_v3,
    validate_resolution_admit_terminal_report_v3, validate_resolution_verify_fund_ready_report_v3,
};
use dclutch_source::resolution::{
    FUNDING_ACTIVATION_RECEIPT_BYTES_V1, FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
};
use dclutch_source::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, RECOVERY_POLICY_SCHEMA_ID_V2,
    RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1, SourceMaterialV3,
    SourceResolutionPhaseV1, SourceResolutionStateV2, WINDOW_SPEC_SCHEMA_ID_V1,
};
use solana_program::hash::hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    ledger::ConservationLedgerV1,
    model::{AccountEvidence, SuccessorPlan, TransactionEvidence},
    plan::pubkey,
    rpc::Rpc,
    stages::{MarketAddressesV1, StageReportV1},
};

use dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;

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
    /// The V7 funding activation receipt `ActivateFund` writes and
    /// `VerifyFundReady` reads. It is Resolution-owned once activation has run,
    /// and a System-owned vacancy before it, so the builder's own refusal is
    /// what says which side of activation this campaign is on.
    pub(crate) activation_receipt: Pubkey,
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

    // ONE AUTHOR FOR THE SELECTION. This tier used to carry its own copy of
    // the derivation so it could compute the subset ledger's address before
    // calling the builder -- two authors for one fact, and when they disagreed
    // the tier's copy said one thing and `build_resolution_create_fund_v3`
    // refused with a code that named nothing. The builder's own selector is
    // public now and this is its only other caller.
    let funding_entry_indices =
        select_resolution_funding_entries_v3(material, Some(policy), manifest).map_err(
            |error| {
                Error::new(format!(
                    "this Market's founding did not buy three Resolution funding compartments this \
                 campaign can name: {error:?}"
                ))
            },
        )?;
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
    let funded_rent_rate = derive_funded_rent_rate_v2(
        rpc.minimum_balance(0)?,
        ledger_bytes.len(),
        rpc.minimum_balance(ledger_bytes.len())?,
    )
    .map_err(|error| Error::new(format!("funded rent rate: {error:?}")))?;
    FundingLedgerV2::initialize(
        &mut ledger_bytes,
        manifest_id,
        manifest,
        selected_mask,
        funded_rent_rate,
    )
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

    let activation_receipt = Pubkey::find_program_address(
        &[
            FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            addresses.founding_market.as_ref(),
            &generation.to_le_bytes(),
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
        activation_receipt,
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
/// The funding-ladder stage's label, in one place.
///
/// It used to read "create and activate the Market's Resolution funding", which
/// named two acts this tier's founding always performs before the stage runs.
/// The label names the FACT the stage is about; `outcome` names who reached it
/// -- `executed` when this campaign drove `VerifyFundReady`, `not-driven` when
/// the atomic founding had already left all three rows Active -- and the note
/// says which and how it knows. One author per fact; the stage does not claim
/// another campaign's act by being named after it.
pub(crate) const FUNDING_LADDER_STAGE_V1: &str =
    "resolution: the Market's Resolution funding reaches Active";

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

    // ------------------------------------------------------------------------
    // THE ATOMIC FOUNDING IS THE AUTHOR OF CreateFund AND ActivateFund.
    //
    // Until 2026-09-06 this stage BUILT and SENT `CreateFund` itself, and it
    // could not: the founding leaves the Source resolution state already
    // created and the funding already activated, so the builder refused before
    // any transaction existed. The refusal was `Funding`, a code standing over
    // forty conjuncts, and three thirty-five-minute hbox runs said only that.
    // Split, it named itself on the first run that reached this stage
    // (`20260906T104204Z`, finalized slot 7,952):
    //
    //     chain-derived CreateFund: FundingConjunct(SourceDestinationNotVacant {
    //         expected == observed, owner = the Resolution program, data_len: 224 })
    //
    // -- the Source state EXISTS, at exactly the address this campaign derives,
    // owned by the Resolution controller. Two authors for one act, and the
    // founding is the one the chain accepted.
    //
    // So this stage OBSERVES what the founding left and drives only what the
    // founding did not. That is not a weaker stage: the poststate assertions
    // below are the same ones the old code made after its own CreateFund, and
    // the double-create hostile is stronger than it was, because the builder
    // now refuses to construct the second frame at all and does it BY NAME.
    let source_account = rpc.required_account(addresses.source_state, "Source resolution state")?;
    if source_account.owner != addresses.resolution_program || source_account.executable {
        return Err(Error::new(format!(
            "the Source resolution state at {} is owned by {} and not by this Market's Resolution \
             controller {}",
            addresses.source_state, source_account.owner, addresses.resolution_program
        )));
    }
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("SourceResolutionStateV2: {error:?}")))?;
    if source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != addresses.market.to_bytes()
        || source.generation() != addresses.generation
    {
        return Err(Error::new(format!(
            "the founding's Source resolution state does not bind this Market at this generation \
             in its primary phase: phase {:?}, generation {}",
            source.phase(),
            source.generation()
        )));
    }

    // THE DOUBLE-CREATE HOSTILE, SATISFIED WITHOUT A TRANSACTION. The Source
    // PDA is one per Market generation and the prepaid-output rule refuses
    // anything not System-owned and empty; the builder enforces that rule off
    // chain, so a second CreateFund never reaches a validator. The exact
    // discriminant is named -- a bare `is_err()` here would pass on any of the
    // builder's forty other refusals.
    match build_resolution_create_fund_v3(&create_snapshot(rpc, addresses)?) {
        Err(ResolutionCoreOperatorErrorV3::FundingConjunct(
            ResolutionFundingCauseV3::SourceDestinationNotVacant {
                expected,
                observed,
                owner,
                data_len,
                ..
            },
        )) if expected == addresses.source_state.to_bytes()
            && observed == expected
            && owner == addresses.resolution_program.to_bytes() =>
        {
            let _ = data_len;
        }
        other => {
            return Err(Error::new(format!(
                "a second CreateFund against a Market whose Source state already exists must \
                 refuse with SourceDestinationNotVacant naming that account and its Resolution \
                 owner; the builder answered {other:?}"
            )));
        }
    }

    // The founding's ActivateFund left an immutable receipt, and
    // `VerifyFundReady` is the Core acceptance that reads it. A campaign that
    // found no receipt would be about to build a frame with nothing to accept.
    let receipt = rpc.required_account(
        addresses.activation_receipt,
        "Resolution funding activation receipt",
    )?;
    if receipt.owner != addresses.resolution_program
        || receipt.executable
        || receipt.data.len() != FUNDING_ACTIVATION_RECEIPT_BYTES_V1
    {
        return Err(Error::new(format!(
            "the funding activation receipt at {} is {} bytes owned by {}, and this campaign \
             expects {FUNDING_ACTIVATION_RECEIPT_BYTES_V1} bytes owned by {}",
            addresses.activation_receipt,
            receipt.data.len(),
            receipt.owner,
            addresses.resolution_program
        )));
    }

    let before = funding_statuses(rpc, addresses)?;
    if before
        .iter()
        .all(|(_, status)| *status == FundingLedgerStatusV2::Active)
    {
        // Nothing left for this campaign to drive. Recorded as a measurement of
        // the founding rather than as a stage this tier executed, because a
        // campaign that claimed to have activated a ledger it found already
        // active would be counting another author's work as its own.
        return Ok((
            StageReportV1 {
                stage: FUNDING_LADDER_STAGE_V1.into(),
                outcome: "not-driven".into(),
                transactions: 0,
                compute_units: 0,
                note: format!(
                    "THE FOUNDING HAD ALREADY DONE ALL OF IT. The Source resolution state at {} \
                     exists in its primary phase bound to this Market at generation {}, the \
                     activation receipt at {} is present at its exact width, and all three \
                     selected funding rows {:?} are already Active. A second CreateFund refuses \
                     to be built at all, by name. This campaign sent nothing here and says so.",
                    addresses.source_state,
                    addresses.generation,
                    addresses.activation_receipt,
                    before.map(|(entry_index, _)| entry_index)
                ),
            },
            crate::ledger::LamportClaimV1::fees(0),
        ));
    }
    for (entry_index, status) in before {
        if status != FundingLedgerStatusV2::Pending {
            return Err(Error::new(format!(
                "Resolution funding row {entry_index} is {status:?} after the founding, and this \
                 campaign can continue only from a uniformly Pending or a uniformly Active \
                 ledger: {before:?}"
            )));
        }
    }

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
    let table_lamports = crate::provider::table_rent(&verify_tables);
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
            stage: FUNDING_LADDER_STAGE_V1.into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "THE FOUNDING CREATED THE SOURCE STATE AND WROTE THE ACTIVATION RECEIPT; THIS \
                 CAMPAIGN DROVE THE CORE ACCEPTANCE. `VerifyFundReady` landed on a real validator \
                 with a fee payer and no other signer -- the funding ladder's whole frame is \
                 non-signing, which is why it is reachable at all while every Claims mutation is \
                 not -- and it moved the ledger's three selected rows from Pending to Active. A \
                 second CreateFund at the same generation was proved to refuse BEFORE any frame \
                 existed, by name: the builder answered SourceDestinationNotVacant against the \
                 founding's own Source state. Manifest entries {:?} carry the recovery, \
                 exhaustion and failure compartments. The Market stayed Open + Consumed, and its \
                 rent beneficiary gained exactly the {} lamports the activation declared.",
                addresses.funding_entry_indices, verify.expected_beneficiary_credit_lamports
            ),
        },
        crate::ledger::LamportClaimV1::fees(fees).with_unwatched(
            table_lamports,
            "one address lookup table, rent-funded to route VerifyFundReady past the legacy \
             packet limit",
        ),
    ))
}

/// Core admits the terminal state the provider execution left behind.
///
/// # The act the journey never drove, and the wall it stood at
///
/// `execute_provider_v3`'s own module comment says it plainly: the route
/// "consumes an already submitted update, derives the current Core caller PDA,
/// invokes the Registry-selected Resolution program, checks its immediate
/// receipt and terminal poststate. **A later standalone Core `AdmitTerminal`
/// consumes that durable certificate and commits the Market transition.**"
///
/// The provider execution therefore leaves a Market at `Open + Consumed` with a
/// `Resolved` Source and a `ResolutionSuccess` certificate, and that is exactly
/// what runs 14 and 15 met: `the provider execution left the Market at Open,
/// not Terminal`. The campaign asserted a poststate the executed route does not
/// write, and the missing act is one instruction that nothing here had ever
/// built. The devnet spine has driven it as its own stage since cohort 14
/// (`31-admit-terminal-*.sh`, `devnet-sponsored-push-v1 --action admit-terminal`,
/// whose stated verifier is "the Market's phase byte goes 1 to 2 and
/// `terminal_receipt` carries the certificate's own address"), and the ordering
/// it encodes -- capture, settle, admit-terminal -- is the ordering the journey
/// was missing the third of.
///
/// # Why this is a builder call and not a second frame
///
/// `build_resolution_admit_terminal_v3` is the one author of this instruction:
/// the relayed-vertical tier calls it, the successor's
/// `devnet-sponsored-push-v1 --action admit-terminal` calls it, and
/// `flagship_resolution`'s `accept` stage calls it. All three hand it the same
/// snapshot -- Market, activation cache, the four deployment programs, the
/// SourceMaterial and capability-manifest pairs, Source state, funding ledger,
/// certificate, Rent, and the three Product-graph record pairs -- and it derives
/// the terminal sequence, the receipt kind, the caller-authority PDA and the
/// selector itself. Nothing here chooses any of them.
///
/// **The selector is READ, never chosen.** The report's `selector` is the
/// Product-authenticated reading of the Source's own decision against the
/// Product record's outcome count, and the builder has already refused any
/// certificate whose own selector disagrees with it. The assertion below is the
/// remaining half: that the byte the Market ends up carrying is that same
/// reading. A campaign that named a winning outcome and then checked the chain
/// agreed would be checking its own arithmetic.
///
/// Twenty-two accounts, no signer, and it does NOT fit a legacy packet:
/// measured on hbox `20260906T150850Z`, the frame is **1,508 bytes** against
/// the 1,232-byte ceiling. It rides a finalized address lookup table like the
/// two provider frames and `CloseFund`, through the producer's own
/// `publish_routing_table`. The relayed-vertical tier sends this same builder's
/// instruction as a legacy packet, so the width is a fact about the market a
/// frame names and not about the route; why that market's is narrower is not
/// measured here and this comment does not guess.
///
/// Nothing about the permission changes: a Market that has resolved may be
/// admitted terminal by anyone, and the permission is the certificate rather
/// than a key.
pub(crate) fn admit_terminal(
    rpc: &mut Rpc,
    payer: &Keypair,
    addresses: &ResolutionAddressesV1,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(StageReportV1, crate::ledger::LamportClaimV1)> {
    let before = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    if before.phase != Phase::Open || before.readiness != Readiness::Consumed {
        return Ok((
            StageReportV1 {
                stage: ADMIT_TERMINAL_STAGE_V1.into(),
                outcome: "blocked".into(),
                transactions: 0,
                compute_units: 0,
                note: format!(
                    "AdmitTerminal admits (Open, Consumed) and (Terminal, Consumed) and the \
                     Market is ({:?}, {:?}). The provider stage above says why.",
                    before.phase, before.readiness
                ),
            },
            crate::ledger::LamportClaimV1::fees(0),
        ));
    }

    let report = build_resolution_admit_terminal_v3(&admit_terminal_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived AdmitTerminal: {error:?}")))?;
    validate_resolution_admit_terminal_report_v3(&report)
        .map_err(|error| Error::new(format!("AdmitTerminal report: {error:?}")))?;
    if report.instruction.program_id != addresses.core_program {
        return Err(Error::new(format!(
            "AdmitTerminal is addressed to {} and this Market's Core program is {}",
            report.instruction.program_id, addresses.core_program
        )));
    }
    let before_tables = transactions.len();
    let (routing, tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        "Core AdmitTerminal",
        std::slice::from_ref(&report.instruction),
        transactions,
    )?;
    let mut submitted = transactions.len().saturating_sub(before_tables);
    let mut fees = crate::provider::fees_since(transactions, before_tables);
    let table_lamports = crate::provider::table_rent(&tables);
    let evidence = rpc.send_v0_with_signers(
        "journey: Core admits the terminal state of an atomically founded Market",
        std::slice::from_ref(&report.instruction),
        payer,
        &[],
        routing,
        &tables,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    let compute_units = evidence.compute_units_consumed.unwrap_or(0);
    submitted += 1;
    transactions.push(evidence);

    // THE PHASE BYTE, THE RECEIPT AND THE SELECTOR, read back off the chain.
    let after = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("terminal Market: {error:?}")))?;
    if after.phase != Phase::Terminal {
        return Err(Error::new(format!(
            "AdmitTerminal left the Market at {:?}, not Terminal",
            after.phase
        )));
    }
    let receipt = after
        .terminal_receipt
        .ok_or_else(|| Error::new("a Terminal Market carries no terminal receipt"))?;
    if receipt.to_bytes() != addresses.certificate.to_bytes() {
        return Err(Error::new(format!(
            "the Market's terminal receipt names {} and this campaign prepaid the certificate at \
             {}",
            Pubkey::new_from_array(receipt.to_bytes()),
            addresses.certificate
        )));
    }
    if after.terminal_winner != report.selector {
        return Err(Error::new(format!(
            "the Market records terminal winner {} and the operator read the Product-authenticated \
             selector {}",
            after.terminal_winner, report.selector
        )));
    }

    Ok((
        StageReportV1 {
            stage: ADMIT_TERMINAL_STAGE_V1.into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "THE PHASE BYTE MOVED 1 -> 2. One permissionless unsigned Core instruction, \
                 twenty-two accounts over a finalized routing table, chain-derived by \
                 `build_resolution_admit_terminal_v3` -- the same builder the relayed-vertical \
                 tier, `devnet-sponsored-push-v1 --action admit-terminal` and \
                 `flagship-resolution --through accept` all call. The Market is Terminal at \
                 terminal sequence {}, its terminal receipt is the certificate the provider \
                 execution minted at {}, and its terminal winner is {} -- the selector the \
                 operator READ from the Source's own decision against a Product-authenticated \
                 outcome count of {}, not a number this campaign chose. Readiness stayed {:?}.",
                report.terminal_sequence,
                addresses.certificate,
                report.selector,
                report.outcome_count,
                after.readiness
            ),
        },
        crate::ledger::LamportClaimV1::fees(fees).with_unwatched(
            table_lamports,
            "one address lookup table, rent-funded to route the 1,508-byte AdmitTerminal frame \
             past the legacy packet limit",
        ),
    ))
}

/// The stage label, in one place: two reports and a witness read it.
pub(crate) const ADMIT_TERMINAL_STAGE_V1: &str =
    "resolution: Core admits the terminal state and the Market's phase byte moves";

/// Same-finalized snapshot for the terminal admission.
///
/// The five staging cursors are vacant by construction: every record this
/// Market selects was published in a transaction and never staged again, and a
/// cursor that is NOT vacant is exactly what `authenticate_finalized_record`
/// refuses inside the builder.
fn admit_terminal_snapshot(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
) -> Result<ResolutionAdmitTerminalSnapshotV3> {
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
            sysvar::rent::ID,
            addresses.product.raw,
            addresses.result_domain.raw,
            addresses.portfolio.raw,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ResolutionAdmitTerminalSnapshotV3 {
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
        rent_sysvar: at(12)?,
        product_raw: at(13)?,
        product_staging: vacant(observation, addresses.product.staging),
        result_domain_raw: at(14)?,
        result_domain_staging: vacant(observation, addresses.result_domain.staging),
        portfolio_raw: at(15)?,
        portfolio_staging: vacant(observation, addresses.portfolio.staging),
    })
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
        // Observed, never assumed vacant: `build_resolution_verify_fund_ready_v3`
        // REQUIRES a Resolution-owned receipt of the exact width, so handing it a
        // constructed vacancy would turn "this campaign has not activated yet"
        // into a bare `Funding` that names nothing.
        activation_receipt: observed_or_vacant(rpc, observation, addresses.activation_receipt)?,
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

// ---------------------------------------------------------------------------
// THE RETIREMENT HAD TWO AUTHORS, AND THIS ONE IS DELETED (2026-09-06).
//
// `resolution::retire` stood here: a hand-built Core `BeginRetiring`, a
// prepaid Source closure receipt, and `build_resolution_direct_close_fund_v1`.
// It was this tier's own, its bindings witnessed it, and it ran BEFORE the
// shipped `local-private-validator-terminal-sequence-v1`.
//
// It could never have shared a run with that driver. PROGRAMS-18A gave the six
// terminal mutations one author
// (`dclutch_market_retirement_v1_operator::terminal_stage_order_v1`) and one
// admissible order, and the invariant it encodes is that the stage which
// PRESERVES a dependency runs before the stage that owns and closes it: Core
// `CloseCapability` on the Direct entry re-states the Resolution dependency
// funding ledger byte for byte, and `ResolutionCloseFund` is what closes that
// ledger. This stage ran `ResolutionCloseFund` at position two, ahead of
// `DirectCloseCapability` -- the exact pair the ruling reversed, in the exact
// direction it forbids -- so a journey that ran it would have destroyed
// `DirectCloseCapability`'s input and stopped three stages short of Retired
// with a refusal on a zero-byte account.
//
// So the convergence is not a preference between two working paths. The
// shipped driver is kept because it walks `TerminalStageV1::ORDERED` and this
// one contradicted it; both drive the same corrected V7 route
// (`build_resolution_direct_close_fund_v1`), so no route loses an author. Its
// one output nothing else derived -- the Source closure receipt address -- has
// always been derivable, and `ResolutionAddressesV1::closure_receipt` is where
// this campaign derives it; `spine::retire` takes that as `--source-receipt`
// rather than fishing for a label in the founding's evidence.
