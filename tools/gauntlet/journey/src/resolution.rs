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
//! What this stage reaches, and what it does not, is decided by ONE fact about
//! the campaign's Market and is documented on `ProviderLegVerdictV1`.

use std::collections::BTreeMap;

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId as CapabilityContentId,
    FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1, FundingStatus,
};
use dclutch_market_core_codec::{CoreState, Phase, Readiness};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
};
use dclutch_resolution_core_v3_operator::{
    Observation, ObservedAccount, ResolutionCreateFundSnapshotV3,
    ResolutionVerifyFundReadySnapshotV3, build_resolution_create_fund_v3,
    build_resolution_verify_fund_ready_v3, validate_resolution_create_fund_report_v3,
    validate_resolution_verify_fund_ready_report_v3,
};
use dclutch_source_contract::{
    RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceMaterialV2, SourceResolutionPhaseV1,
    SourceResolutionStateV2,
};
use serde::Serialize;
use solana_program::hash::hash;
use solana_sdk::{
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
    pub(crate) source_state: Pubkey,
    /// Recovery, exhaustion, failure — in the order the operator consumes them.
    pub(crate) funding: [Pubkey; 3],
    pub(crate) funding_entry_indices: [u16; 3],
    pub(crate) rent_beneficiary: Pubkey,
    /// The terminal certificate this Market's first terminal sequence would
    /// occupy. Watched from the start so the ledger can see it stay vacant.
    pub(crate) certificate: Pubkey,
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

/// What the provider legs of the ladder could do against this campaign's chain.
///
/// Kept as its own type because the answer is a PROPERTY OF THE MARKET, not of
/// this tier, and it deserves to be named rather than folded into a note.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ProviderLegVerdictV1 {
    pub(crate) reachable: bool,
    /// Record identities the Market's own `SourceMaterialV2` names and that no
    /// record body can ever hash to.
    pub(crate) unrealizable_record_ids: Vec<String>,
    pub(crate) detail: String,
}

/// Derive every resolution coordinate from the founding's evidence.
///
/// The three funding destinations are re-derived here from the manifest, which
/// duplicates a private selection inside the operator. That duplication is safe
/// in exactly one direction: `build_resolution_create_fund_v3` re-derives each
/// address itself and REFUSES if the supplied account is not the one it
/// computes, so an error here becomes a refusal rather than a wrong Fund.
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

    let source_material_body = record_body(rpc, evidence, "source_material_record")?;
    let manifest_body = record_body(rpc, evidence, "capability_manifest_record")?;
    let recovery_body = record_body(rpc, evidence, "recovery_policy_record")?;

    let source_material = RecordPairV1::derive(
        registry_program,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        &source_material_body,
    );
    let capability_manifest = RecordPairV1::derive(
        registry_program,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_body,
    );
    let recovery_policy = RecordPairV1::derive(
        registry_program,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        &recovery_body,
    );
    for (label, derived, recorded) in [
        (
            "source_material_record",
            source_material.raw,
            evidence_address(evidence, "source_material_record")?,
        ),
        (
            "capability_manifest_record",
            capability_manifest.raw,
            evidence_address(evidence, "capability_manifest_record")?,
        ),
        (
            "recovery_policy_record",
            recovery_policy.raw,
            evidence_address(evidence, "recovery_policy_record")?,
        ),
    ] {
        if derived != recorded {
            return Err(Error::new(format!(
                "{label} sits at {recorded} but its own body hashes to a record at {derived}; a \
                 record whose address is not its content identity is not a finalized record"
            )));
        }
    }

    let material = SourceMaterialV2::decode(&source_material_body)
        .map_err(|error| Error::new(format!("SourceMaterialV2: {error:?}")))?;
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
    let funding_state_rent = rpc.minimum_balance(FUNDING_STATE_BYTES)?;
    let mut funding = [Pubkey::default(); 3];
    for (slot, entry_index) in funding_entry_indices.into_iter().enumerate() {
        let entry = manifest
            .entry(entry_index)
            .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
        let target = funding_state_rent
            .checked_add(entry.funding_quote().amounts().native_lamports_total())
            .ok_or_else(|| Error::new("funding target overflowed u64"))?;
        let custody = FundingCustodyObservationV1::native_only(target, funding_state_rent)
            .map_err(|error| Error::new(format!("funding custody: {error:?}")))?;
        let state = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
            .map_err(|error| Error::new(format!("pending FundingState: {error:?}")))?;
        let derivation = CapabilityFundingDerivationV1::new(
            addresses.founding_market.to_bytes(),
            generation,
            manifest_id,
            manifest,
            state,
        )
        .map_err(|error| Error::new(format!("funding derivation: {error:?}")))?;
        funding[slot] =
            Pubkey::find_program_address(&derivation.seed_components(), &resolution_program).0;
    }

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
        source_state,
        funding,
        funding_entry_indices,
        rent_beneficiary: Pubkey::new_from_array(market.rent_beneficiary.to_bytes()),
        certificate,
    })
}

/// Register every resolution coordinate with the conservation ledger.
pub(crate) fn watch(ledger: &mut ConservationLedgerV1, addresses: &ResolutionAddressesV1) {
    for (label, address) in [
        ("resolution_source_state", addresses.source_state),
        ("resolution_funding_recovery", addresses.funding[0]),
        ("resolution_funding_exhaustion", addresses.funding[1]),
        ("resolution_funding_failure", addresses.funding[2]),
        ("resolution_terminal_certificate", addresses.certificate),
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
) -> Result<(StageReportV1, ProviderLegVerdictV1, u64)> {
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

    // 1. Prepay. `CreateFund` consumes precommitted, prepaid destinations: the
    //    Source state and three Funds are System-owned and empty, and the
    //    operator computes the exact top-up each one needs. Prepaying in its
    //    own transaction rather than composing four transfers ahead of a
    //    twenty-account frame keeps this campaign inside the legacy packet
    //    without an address lookup table, and it makes the two facts separable
    //    in the evidence: what was funded, and what was created.
    let create = build_resolution_create_fund_v3(&create_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived CreateFund: {error:?}")))?;
    validate_resolution_create_fund_report_v3(&create)
        .map_err(|error| Error::new(format!("CreateFund report: {error:?}")))?;
    let mut prepay = Vec::with_capacity(4);
    if create.source_top_up_lamports > 0 {
        prepay.push(transfer(
            &payer.pubkey(),
            &addresses.source_state,
            create.source_top_up_lamports,
        ));
    }
    for (destination, top_up) in addresses
        .funding
        .into_iter()
        .zip(create.funding_top_up_lamports)
    {
        if top_up > 0 {
            prepay.push(transfer(&payer.pubkey(), &destination, top_up));
        }
    }
    if !prepay.is_empty() {
        let evidence = rpc.send(
            "journey: prepay the Source resolution state and the three Resolution Funds",
            &prepay,
            payer,
        )?;
        fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
        compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
        submitted += 1;
        transactions.push(evidence);
    }

    // 2. The adversarial half FIRST. A second creation at the same generation
    //    must refuse, and the cheapest honest way to prove the guard is live is
    //    to submit the creation twice and assert the SECOND one fails — which
    //    is exactly what this campaign does below, after the honest creation.
    //    Before that: an over-funded Fund. Every Fund's lamports must equal
    //    rent plus exactly the native principal its own manifest entry quotes,
    //    and over-funding is not a donation a prepaid compartment may keep.
    let over = rpc.send_expected_failure(
        "journey: over-funding a Resolution Fund by one lamport refuses the creation",
        &[
            transfer(&payer.pubkey(), &addresses.funding[0], 1),
            create.instruction.clone(),
        ],
        payer,
    )?;
    fees = fees.saturating_add(over.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(over.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(over);
    // The refusal has to roll the transfer back with it, or the Fund is now
    // permanently unfundable and the refusal was worse than useless.
    let after_refusal = rpc
        .account(addresses.funding[0])?
        .map(|account| account.lamports)
        .unwrap_or(0);

    // 3. The honest creation, rebuilt from the prepaid snapshot so its top-ups
    //    are zero and the frame is byte-identical to the one just refused.
    let create = build_resolution_create_fund_v3(&create_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived CreateFund after prepay: {error:?}")))?;
    validate_resolution_create_fund_report_v3(&create)
        .map_err(|error| Error::new(format!("CreateFund report after prepay: {error:?}")))?;
    if create.source_top_up_lamports != 0 || create.funding_top_up_lamports != [0; 3] {
        return Err(Error::new(format!(
            "the prepayment did not reach the exact funding target: source needs {} more and the \
             Funds need {:?} more. The over-funding refusal must have kept a lamport (the Fund \
             holds {after_refusal}).",
            create.source_top_up_lamports, create.funding_top_up_lamports
        )));
    }
    let evidence = rpc.send(
        "journey: an Open Market creates its own Resolution Fund",
        std::slice::from_ref(&create.instruction),
        payer,
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
    for (label, address) in funding_labels(addresses) {
        let status = FundingStateV1::decode(&rpc.required_account(address, label)?.data)
            .map_err(|error| Error::new(format!("{label}: {error:?}")))?
            .status();
        if status != FundingStatus::Pending {
            return Err(Error::new(format!(
                "{label} is {status:?} immediately after creation, not Pending"
            )));
        }
    }

    // 4. Double create. The Source PDA is one per Market generation, and the
    //    prepaid-output rule refuses anything not System-owned and empty.
    let double = rpc.send_expected_failure(
        "journey: a second CreateFund at the same generation refuses",
        &[create.instruction],
        payer,
    )?;
    fees = fees.saturating_add(double.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(double.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(double);

    // 5. Activation. Core stays Open + Consumed: an already-open Market
    //    consumed its readiness at the commit-last Open, and the activation
    //    lives in the three FundingState accounts, which terminal admission
    //    rechecks. One semantic owner for that fact, not two.
    let verify = build_resolution_verify_fund_ready_v3(&verify_snapshot(rpc, addresses)?)
        .map_err(|error| Error::new(format!("chain-derived VerifyFundReady: {error:?}")))?;
    validate_resolution_verify_fund_ready_report_v3(&verify)
        .map_err(|error| Error::new(format!("VerifyFundReady report: {error:?}")))?;
    let beneficiary_before = rpc
        .account(addresses.rent_beneficiary)?
        .map(|account| account.lamports)
        .unwrap_or(0);
    let evidence = rpc.send(
        "journey: activate the three-ledger Resolution funding of an Open Market",
        &[verify.instruction],
        payer,
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
    for (label, address) in funding_labels(addresses) {
        let status = FundingStateV1::decode(&rpc.required_account(address, label)?.data)
            .map_err(|error| Error::new(format!("{label}: {error:?}")))?
            .status();
        if status != FundingStatus::Active {
            return Err(Error::new(format!(
                "{label} is {status:?} after activation, not Active"
            )));
        }
    }

    let provider = provider_leg_verdict(rpc, addresses)?;
    Ok((
        StageReportV1 {
            stage: "resolution: create and activate the Market's Resolution funding".into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "The atomically founded Market created its own Source resolution state and three \
                 Resolution Funds and activated them, on a real validator, with a fee payer and no \
                 other signer -- the funding ladder's whole frame is non-signing, which is why it \
                 is reachable at all while every Claims mutation is not. Two refusals were proved \
                 on the way: over-funding a Fund by one lamport, and a second CreateFund at the \
                 same generation. Manifest entries {:?} carry the recovery, exhaustion and failure \
                 compartments. The Market stayed Open + Consumed, and its rent beneficiary gained \
                 exactly the {} lamports the activation declared. {}",
                addresses.funding_entry_indices,
                verify.expected_beneficiary_credit_lamports,
                provider.detail
            ),
        },
        provider,
        fees,
    ))
}

/// Decide whether the provider legs can run against THIS Market, and say why.
///
/// The answer turns on one thing, and it is not the Hot gate and not the
/// prestate: `SourceMaterialV2` names its source spec, window spec and
/// statistic spec by CONTENT IDENTITY, and a finalized record's address is
/// derived from the hash of its own body. So a Market whose material names
/// identities that are not the hash of any record body has named records that
/// cannot be published — by anyone, ever, short of a preimage attack.
fn provider_leg_verdict(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
) -> Result<ProviderLegVerdictV1> {
    let body = rpc
        .required_account(addresses.source_material.raw, "source material record")?
        .data;
    let material = SourceMaterialV2::decode(&body)
        .map_err(|error| Error::new(format!("SourceMaterialV2: {error:?}")))?;
    // A record's address is `[domain, schema, sha256(body)]`, so an identity is
    // realizable exactly when some body hashes to it. Nothing here can decide
    // that in general; what it CAN do is report the three identities the
    // Market's own material names, so the claim is checkable by whoever tries
    // to publish them rather than taken on this campaign's word.
    let unrealizable = [
        ("primary source spec", material.primary_source_spec()),
        ("window spec", material.window_spec()),
        ("statistic spec", material.statistic_spec()),
    ]
    .into_iter()
    .map(|(label, id)| format!("{label} {}", hex(&id.to_bytes())))
    .collect();
    Ok(ProviderLegVerdictV1 {
        reachable: false,
        unrealizable_record_ids: unrealizable,
        detail:
            "The provider legs -- one Pyth update submitted through the real Receiver ELF and one \
             Core-driven execution that mints the terminal certificate -- did NOT run, and the \
             reason is upstream of both the Hot gate and the prestate. Both legs authenticate the \
             Market's SourceSpecV1, WindowSpecV1 and StatisticSpecV1 as finalized Registry \
             records, and a finalized record lives at an address derived from the hash of its own \
             body. This Market's SourceMaterialV2 names all three by domain-separated DEMO \
             digests (`demo_id(\"source-spec/pyth-price-update\", ..)` and siblings in \
             market.rs::demo_market_input), which are not the hash of any record body, so no \
             record can ever be published at those identities and the ladder stops here by \
             construction. The Pyth receiver and router ELFs ARE deployed on this validator \
             (dclutch-successor-validator loads both), and the plan already publishes the \
             deployment-slot-zero `local_validator_release_v1` Pyth release record, so everything \
             else the legs need is present. The fix is journey-adjacent and named: compile real \
             SourceSpecV1/WindowSpecV1/StatisticSpecV1/ProviderReleaseV1/PythAdapterConfigV1 \
             bodies in the demo market input, name them by their own digests, and publish them \
             with the rest of the graph."
                .into(),
    })
}

fn funding_labels(addresses: &ResolutionAddressesV1) -> [(&'static str, Pubkey); 3] {
    [
        ("recovery Fund", addresses.funding[0]),
        ("exhaustion Fund", addresses.funding[1]),
        ("failure Fund", addresses.funding[2]),
    ]
}

fn select_funding_entries(
    material: &SourceMaterialV2,
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
        if entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V4 {
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
        recovery_destination: observed_or_vacant(rpc, observation, addresses.funding[0])?,
        exhaustion_destination: observed_or_vacant(rpc, observation, addresses.funding[1])?,
        failure_destination: observed_or_vacant(rpc, observation, addresses.funding[2])?,
        rent_sysvar: at(9)?,
        system_program: at(10)?,
        recovery_policy: at(11)?,
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
            addresses.funding[0],
            addresses.funding[1],
            addresses.funding[2],
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
        recovery_funding: at(10)?,
        exhaustion_funding: at(11)?,
        failure_funding: at(12)?,
        beneficiary: at(13)?,
        clock_sysvar: at(14)?,
        rent_sysvar: at(15)?,
        recovery_policy: at(16)?,
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

fn record_body(
    rpc: &mut Rpc,
    evidence: &BTreeMap<String, AccountEvidence>,
    label: &str,
) -> Result<Vec<u8>> {
    Ok(rpc
        .required_account(evidence_address(evidence, label)?, label)?
        .data)
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
