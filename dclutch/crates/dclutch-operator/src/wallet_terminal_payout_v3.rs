//! Wallet-authorized terminal payout construction for a plain Claims Position.
//!
//! This module moves the production caller boundary out of the ProgramTest
//! campaign without creating a second payout implementation. Product payout,
//! exposure translation, and the SignedDelta packet remain owned by
//! `dclutch-claims-svm`; this host-only operator supplies the wallet request,
//! exact 36-account frame, Custody caller PDA, sole canonical lookup sequence,
//! unsigned v0 message, and independently checked postcondition.

use dclutch_claims_svm::{
    CallerRole,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketLayoutV2, LiabilityBasisMarketViewV2,
        LiabilityBasisPositionLayoutV2, LiabilityBasisPositionViewV2,
    },
    product_basis_terminal_v3::{
        ProductClaimsTerminalAdmissionV3, ProductClaimsTerminalInputV3,
        encode_product_claims_terminal_signed_delta_v3,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
    signed_delta_v3::{
        DeltaDirectionV3, SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3,
        SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3, SignedDeltaPlanV3, SignedDeltaV3, plan_bytes,
    },
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        TERMINAL_SETTLEMENT_POST_RESOURCE_DOMAIN_V3, TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3,
        TerminalSettlementReceiptV3, TerminalSettlementRequestInputV3, TerminalSettlementRequestV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_POSTSTATE_DOMAIN_V1,
    CallerRoleV1 as CustodyCallerRoleV1, CompartmentV1, ContextV1, CustodyReceiptV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
    ReceiptEvidenceV1,
};
use dclutch_product_payoff_v2_codec::runtime_v3::ProductBasisV3;
use dclutch_rational_representation_v2_kernel::product_v3::TerminalScenarioV3;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::{AccountState, TokenAccount, TokenProgram};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

use crate::{
    Finality, Observation, ObservedAccount,
    versioned::{VersionedMessagePlanV0, compile_v0_message},
};

/// Exact physical account coordinates of the Claims terminal-settlement route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalletTerminalPayoutRouteV3 {
    /// Claims aggregate.
    pub aggregate: Pubkey,
    /// Finalized linked ProductBasis raw record.
    pub linked_basis_raw: Pubkey,
    /// Vacant linked ProductBasis staging cursor.
    pub linked_basis_staging: Pubkey,
    /// Finalized Product root record.
    pub product_raw: Pubkey,
    /// Vacant Product staging cursor.
    pub product_staging: Pubkey,
    /// Finalized ResultDomain record.
    pub result_domain_raw: Pubkey,
    /// Vacant ResultDomain staging cursor.
    pub result_domain_staging: Pubkey,
    /// Finalized Portfolio record.
    pub portfolio_raw: Pubkey,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: Pubkey,
    /// Logical Core Market.
    pub market: Pubkey,
    /// Selected Registry activation cache.
    pub activation_cache: Pubkey,
    /// Selected Registry program.
    pub registry_program: Pubkey,
    /// Selected Claims program.
    pub claims_program: Pubkey,
    /// Current Claims ProgramData.
    pub claims_programdata: Pubkey,
    /// Selected Core program.
    pub core_program: Pubkey,
    /// Current Core ProgramData.
    pub core_programdata: Pubkey,
    /// Selected Resolution program.
    pub resolution_program: Pubkey,
    /// Current Resolution ProgramData.
    pub resolution_programdata: Pubkey,
    /// Canonical wallet-owned Claims Position.
    pub position: Pubkey,
    /// Finalized Product-to-Claims exposure record.
    pub exposure_raw: Pubkey,
    /// Vacant exposure staging cursor.
    pub exposure_staging: Pubkey,
    /// Selected Custody program.
    pub custody_program: Pubkey,
    /// Exact Resolution certificate named by the Core terminal receipt.
    pub terminal_certificate: Pubkey,
    /// Finalized Realm record.
    pub realm_raw: Pubkey,
    /// Vacant Realm staging cursor.
    pub realm_staging: Pubkey,
    /// Canonical Claims-role Custody replay.
    pub custody_replay: Pubkey,
    /// Realm collateral Mint.
    pub collateral_mint: Pubkey,
    /// Canonical Hoard Vault.
    pub hoard: Pubkey,
    /// Wallet collateral token account receiving payout.
    pub recipient: Pubkey,
    /// Canonical Custody transfer authority.
    pub custody_authority: Pubkey,
    /// Realm-selected token program.
    pub token_program: Pubkey,
}

/// Exact public prestate required to build one wallet terminal payout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalletTerminalPayoutInputV3<'a> {
    /// One finalized observation shared by every supplied account byte.
    pub observation: Observation,
    /// Exact physical route coordinates.
    pub route: WalletTerminalPayoutRouteV3,
    /// Nonzero wallet-owned request context.
    pub parent_context: [u8; 32],
    /// Exact terminal receipt digest carried by the Core Market.
    pub terminal_record_digest: [u8; 32],
    /// Wallet identity owning the recipient token account.
    pub recipient_owner: [u8; 32],
    /// Ordered parent-effect coordinate; zero for the plain wallet route.
    pub transfer_index: u16,
    /// Authenticated Product-to-Claims projection.
    pub admission: ProductClaimsTerminalAdmissionV3,
    /// Exact finalized ProductBasis bytes.
    pub product_basis_bytes: &'a [u8],
    /// Exact finalized Product-to-Claims exposure bytes.
    pub composition_exposure_bytes: &'a [u8],
    /// Finalized exposure record admission.
    pub composition_exposure_admission:
        dclutch_representation_composition_v3_kernel::RecordAdmissionV3,
    /// Finalized Product root digest.
    pub product_record_digest: [u8; 32],
    /// Exact Claims aggregate prestate bytes.
    pub aggregate_bytes: &'a [u8],
    /// Exact wallet Position prestate bytes.
    pub position_bytes: &'a [u8],
    /// Exact Claims-role Custody replay prestate bytes.
    pub custody_replay_bytes: &'a [u8],
    /// Exact Hoard token-account prestate bytes.
    pub hoard_token_bytes: &'a [u8],
    /// Exact recipient token-account prestate bytes.
    pub recipient_token_bytes: &'a [u8],
    /// Product terminal scenario authenticated from Core and its terminal record.
    pub terminal: TerminalScenarioV3,
    /// Wallet Position owner and transaction signer.
    pub owner: [u8; 32],
    /// Claims coordinate to burn.
    pub claim_index: u32,
    /// Positive Claims atoms to burn.
    pub quantity: u64,
    /// Immutable Market generation.
    pub expected_generation: u64,
    /// Optimistic aggregate revision.
    pub expected_market_revision: u64,
    /// Optimistic Position revision.
    pub expected_position_revision: u64,
}

/// Exact unsigned payout instruction plus independently derived commitments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletTerminalPayoutReportV3 {
    /// Exact 36-account Claims instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting its public prestate.
    pub observation: Observation,
    /// Canonical request embedded in the instruction.
    pub request: TerminalSettlementRequestV3,
    /// SHA-256 of the exact request bytes.
    pub request_digest: [u8; 32],
    /// Exact SignedDelta packet independently reproduced by the operator.
    pub signed_packet: Vec<u8>,
    /// SHA-256 of the SignedDelta packet.
    pub signed_packet_digest: [u8; 32],
    /// Domain-separated digest of the SignedDelta tables.
    pub signed_table_digest: [u8; 32],
    /// Exact collateral atoms paid; zero is a real burn outcome.
    pub payout: u64,
    /// Claims-derived Custody caller; inert Claims program for zero payout.
    pub custody_caller: Pubkey,
    /// Exact positive-payout Custody request digest, zero for a zero payout.
    pub custody_request_digest: [u8; 32],
    /// Exact positive-payout Custody request, absent for a zero payout.
    pub custody_request: Option<CustodyRequestV1>,
    /// Sole wallet signer required by the instruction.
    pub owner: Pubkey,
    /// Exact physical route used to derive the instruction and postcondition.
    pub route: WalletTerminalPayoutRouteV3,
    /// Exact aggregate bytes used to build the request.
    pub pre_aggregate_bytes: Vec<u8>,
    /// Exact Position bytes used to build the request.
    pub pre_position_bytes: Vec<u8>,
    /// Exact Custody replay bytes used to build the request.
    pub pre_custody_replay_bytes: Vec<u8>,
    /// Exact Hoard token bytes used to build the request.
    pub pre_hoard_token_bytes: Vec<u8>,
    /// Exact recipient token bytes used to build the request.
    pub pre_recipient_token_bytes: Vec<u8>,
}

/// Unsigned packet-safe transaction and its exact wallet signer set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletTerminalPayoutTransactionPlanV3 {
    /// Exact v0 message compiled through one canonical finalized lookup table.
    pub message: VersionedMessagePlanV0,
    /// Fee payer followed by Position owner when they differ.
    pub required_signers: Vec<Pubkey>,
    /// Exact payout instruction report.
    pub payout: WalletTerminalPayoutReportV3,
}

/// Stable production-builder refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WalletTerminalPayoutErrorV3 {
    /// Inputs were not one nonzero finalized snapshot.
    Snapshot,
    /// Route identities, PDAs, or immutable joins disagreed.
    Route,
    /// Claims aggregate or Position bytes refused.
    Claims,
    /// Custody replay, Vault, authority, or token prestate refused.
    Custody,
    /// Product/exposure evaluation or request construction refused.
    Economic,
    /// Checked arithmetic overflowed.
    Arithmetic,
    /// The observed lookup table was not the sole canonical sequence.
    LookupTable,
    /// Versioned-message routing refused.
    Routing(crate::versioned::Error),
    /// A claimed accepted poststate disagreed with the exact plan.
    Postcondition,
}

/// Build one wallet-authorized terminal payout from exact public prestate.
pub fn build_wallet_terminal_payout_v3(
    input: WalletTerminalPayoutInputV3<'_>,
) -> Result<WalletTerminalPayoutReportV3, WalletTerminalPayoutErrorV3> {
    validate_snapshot(input.observation)?;
    let route = input.route;
    let market = LiabilityBasisMarketViewV2::decode(input.aggregate_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Claims)?;
    let position = LiabilityBasisPositionViewV2::decode(input.position_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Claims)?;
    validate_claims_route(input, market, position)?;
    let replay = validate_custody_route(input, market)?;

    let request = TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
        caller_role: CallerRole::Claims,
        release_set: market.release_set,
        market: market.logical_market,
        realm: market.realm_id,
        parent_context: input.parent_context,
        product_record_digest: input.product_record_digest,
        exposure_id: input.admission.exposure_id(),
        exposure_digest: input.admission.exposure_digest(),
        terminal_record_digest: input.terminal_record_digest,
        owner: input.owner,
        position: route.position.to_bytes(),
        recipient_owner: input.recipient_owner,
        recipient_token_account: route.recipient.to_bytes(),
        claims_program: route.claims_program.to_bytes(),
        custody_program: route.custody_program.to_bytes(),
        collateral_mint: route.collateral_mint.to_bytes(),
        token_program: route.token_program.to_bytes(),
        semantic_basis_id: market.basis_id,
        linked_basis_record_digest: input.admission.linked_basis_record_digest(),
        generation: market.generation,
        expected_market_revision: market.revision,
        expected_position_revision: position.revision,
        expected_custody_revision: replay.next_revision,
        quantity: input.quantity,
        claim_index: input.claim_index,
        transfer_index: input.transfer_index,
    })
    .map_err(|_| WalletTerminalPayoutErrorV3::Economic)?;
    let request_bytes = request.to_bytes();
    let request_digest = hash(&request_bytes).to_bytes();
    let product_width = usize::try_from(
        ProductBasisV3::decode(input.product_basis_bytes)
            .map_err(|_| WalletTerminalPayoutErrorV3::Economic)?
            .basis_width(),
    )
    .map_err(|_| WalletTerminalPayoutErrorV3::Arithmetic)?;
    let claims_width =
        usize::try_from(market.claim_count).map_err(|_| WalletTerminalPayoutErrorV3::Arithmetic)?;
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0)
        .map_err(|_| WalletTerminalPayoutErrorV3::Economic)?;
    let mut product_payouts = vec![0_u64; product_width];
    let mut translation = vec![0_u64; claims_width];
    let mut claims_payouts = vec![0_u64; claims_width];
    let mut aggregate_deltas = vec![neutral; claims_width];
    let mut signed_packet = vec![
        0_u8;
        plan_bytes(market.claim_count, 1, 1)
            .map_err(|_| WalletTerminalPayoutErrorV3::Arithmetic)?
    ];
    let hoard = TokenAccount::parse(input.hoard_token_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Custody)?;
    let payout = encode_product_claims_terminal_signed_delta_v3(
        ProductClaimsTerminalInputV3 {
            product_basis_bytes: input.product_basis_bytes,
            admission: input.admission,
            composition_exposure_bytes: input.composition_exposure_bytes,
            composition_exposure_admission: input.composition_exposure_admission,
            product_record_digest: input.product_record_digest,
            market_account: route.aggregate.to_bytes(),
            market_bytes: input.aggregate_bytes,
            position_bytes: input.position_bytes,
            owner: input.owner,
            request_id: request_digest,
            caller_role: CallerRole::Claims,
            terminal: input.terminal,
            claim_index: input.claim_index,
            quantity: input.quantity,
            expected_generation: market.generation,
            expected_market_revision: market.revision,
            expected_position_revision: position.revision,
            hoard_before: hoard.amount,
        },
        &mut product_payouts,
        &mut translation,
        &mut claims_payouts,
        &mut aggregate_deltas,
        &mut signed_packet,
    )
    .map_err(|_| WalletTerminalPayoutErrorV3::Economic)?;
    let signed_packet_digest = hash(&signed_packet).to_bytes();
    let plan = SignedDeltaPlanV3::decode(&signed_packet)
        .map_err(|_| WalletTerminalPayoutErrorV3::Economic)?;
    let (positions, aggregates, deltas) = plan.table_bytes();
    let signed_table_digest = hashv(&[
        SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3,
        positions,
        aggregates,
        deltas,
    ])
    .to_bytes();
    let (custody_caller, custody_request_digest, custody_request) = custody_caller(
        route,
        market,
        request,
        request_digest,
        signed_packet_digest,
        payout,
    )?;
    let accounts = payout_accounts(route, Pubkey::new_from_array(input.owner), custody_caller);
    if accounts.len() != TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 {
        return Err(WalletTerminalPayoutErrorV3::Route);
    }
    Ok(WalletTerminalPayoutReportV3 {
        instruction: Instruction {
            program_id: route.claims_program,
            accounts,
            data: request_bytes.to_vec(),
        },
        observation: input.observation,
        request,
        request_digest,
        signed_packet,
        signed_packet_digest,
        signed_table_digest,
        payout,
        custody_caller,
        custody_request_digest,
        custody_request,
        owner: Pubkey::new_from_array(input.owner),
        route,
        pre_aggregate_bytes: input.aggregate_bytes.to_vec(),
        pre_position_bytes: input.position_bytes.to_vec(),
        pre_custody_replay_bytes: input.custody_replay_bytes.to_vec(),
        pre_hoard_token_bytes: input.hoard_token_bytes.to_vec(),
        pre_recipient_token_bytes: input.recipient_token_bytes.to_vec(),
    })
}

/// Return the exact first-use, duplicate-free lookup sequence proven by the campaign.
///
/// The fee payer and Position owner remain static signers. Every other program
/// or account key is listed at its first appearance in the instruction frame,
/// including the invoked Claims program. An extra, missing, sorted, or otherwise
/// reordered table is a different route and is refused by the compiler below.
pub fn canonical_wallet_terminal_payout_lookup_addresses_v3(
    report: &WalletTerminalPayoutReportV3,
    payer: Pubkey,
) -> Result<Vec<Pubkey>, WalletTerminalPayoutErrorV3> {
    if payer == Pubkey::default() || report.owner == Pubkey::default() {
        return Err(WalletTerminalPayoutErrorV3::Snapshot);
    }
    let mut addresses = Vec::new();
    for address in core::iter::once(report.instruction.program_id)
        .chain(report.instruction.accounts.iter().map(|meta| meta.pubkey))
    {
        if address != payer && address != report.owner && !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    if addresses.is_empty() || addresses.len() > 256 {
        return Err(WalletTerminalPayoutErrorV3::LookupTable);
    }
    Ok(addresses)
}

/// The compute-unit ceiling one terminal payout declares.
///
/// Solana's per-transaction maximum. See `compile_wallet_terminal_payout_v0`
/// for why this is a ceiling and not a tuned number.
pub const WALLET_TERMINAL_PAYOUT_COMPUTE_UNITS_V3: u32 = 1_400_000;

/// Compile one payout through exactly one finalized canonical lookup table.
pub fn compile_wallet_terminal_payout_v0(
    report: WalletTerminalPayoutReportV3,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<WalletTerminalPayoutTransactionPlanV3, WalletTerminalPayoutErrorV3> {
    validate_snapshot(report.observation)?;
    if lookup_table.observation != report.observation
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
    {
        return Err(WalletTerminalPayoutErrorV3::LookupTable);
    }
    let expected = canonical_wallet_terminal_payout_lookup_addresses_v3(&report, payer)?;
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| WalletTerminalPayoutErrorV3::LookupTable)?;
    if table.addresses.as_ref() != expected.as_slice() {
        return Err(WalletTerminalPayoutErrorV3::LookupTable);
    }
    // A terminal payout does not fit the runtime's 200,000-CU default. Measured
    // on a driven substrate: the Claims program consumed the whole default and
    // was still running -- `exceeded CUs meter at BPF instruction` -- so every
    // payout ever attempted through this compiler would have died there. It had
    // never been attempted, because the Claims-role replay the route decodes had
    // no creation caller until one existed.
    //
    // The declaration is the transaction ceiling rather than a tuned figure.
    // This route's cost is a function of the Market's claim count and its
    // terminal composition graph, which are per-market facts this compiler does
    // not see, and there is no priority fee on this transaction for a tight
    // limit to save. A wrong-but-tight limit would be a liveness failure on some
    // future market's shape; the ceiling cannot be. The cost that actually
    // matters is recorded per payout in its receipt, read back from chain.
    //
    // The ComputeBudget program id compiles as a static key, so the canonical
    // lookup-table census is unchanged and an already-frozen table stays valid.
    let instructions = [
        ComputeBudgetInstruction::set_compute_unit_limit(WALLET_TERMINAL_PAYOUT_COMPUTE_UNITS_V3),
        report.instruction.clone(),
    ];
    let message = compile_v0_message(
        payer,
        &instructions,
        recent_blockhash,
        report.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(WalletTerminalPayoutErrorV3::Routing)?;
    let required_signers = if payer == report.owner {
        vec![payer]
    } else {
        vec![payer, report.owner]
    };
    if usize::from(message.required_signatures) != required_signers.len() {
        return Err(WalletTerminalPayoutErrorV3::Route);
    }
    Ok(WalletTerminalPayoutTransactionPlanV3 {
        message,
        required_signers,
        payout: report,
    })
}

/// Exact account bytes observed after an accepted payout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalletTerminalPayoutPoststateV3<'a> {
    /// Claims return data containing the terminal receipt.
    pub receipt_bytes: &'a [u8],
    /// Claims aggregate poststate.
    pub aggregate_bytes: &'a [u8],
    /// Wallet Position poststate.
    pub position_bytes: &'a [u8],
    /// Claims-role Custody replay poststate.
    pub custody_replay_bytes: &'a [u8],
    /// Hoard token-account poststate.
    pub hoard_token_bytes: &'a [u8],
    /// Recipient token-account poststate.
    pub recipient_token_bytes: &'a [u8],
}

/// Exact owned poststate projected before a wallet payout is signed.
///
/// This is the sole host-side owner of the Claims, Custody replay, token, and
/// nested Custody/Claims receipt arithmetic. Exterior executors persist these
/// bytes as durable intent; the verifier below then compares accepted chain
/// bytes against the same projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletTerminalPayoutExpectedPoststateV3 {
    /// Exact Claims return-data receipt bytes.
    pub receipt_bytes: Vec<u8>,
    /// Exact Claims aggregate bytes after the burn.
    pub aggregate_bytes: Vec<u8>,
    /// Exact wallet Position bytes after the burn.
    pub position_bytes: Vec<u8>,
    /// Exact Claims-role Custody replay bytes after the payout.
    pub custody_replay_bytes: Vec<u8>,
    /// Exact Hoard token-account bytes after the payout.
    pub hoard_token_bytes: Vec<u8>,
    /// Exact recipient token-account bytes after the payout.
    pub recipient_token_bytes: Vec<u8>,
}

/// Project the exact poststate and nested receipts of one checked payout.
pub fn project_wallet_terminal_payout_postcondition_v3(
    report: &WalletTerminalPayoutReportV3,
) -> Result<WalletTerminalPayoutExpectedPoststateV3, WalletTerminalPayoutErrorV3> {
    let request = report.request.input();
    let aggregate_bytes = debited_claim_bytes(
        &report.pre_aggregate_bytes,
        LiabilityBasisMarketLayoutV2::REVISION,
        LiabilityBasisMarketLayoutV2::SUPPLIES,
        LiabilityBasisMarketLayoutV2::SUPPLY_STRIDE,
        request.claim_index,
        request.quantity,
    )?;
    let position_bytes = debited_claim_bytes(
        &report.pre_position_bytes,
        LiabilityBasisPositionLayoutV2::REVISION,
        LiabilityBasisPositionLayoutV2::BALANCES,
        LiabilityBasisPositionLayoutV2::BALANCE_STRIDE,
        request.claim_index,
        request.quantity,
    )?;
    LiabilityBasisMarketViewV2::decode(&aggregate_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
    LiabilityBasisPositionViewV2::decode(&position_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;

    let before_replay = CustodyReplayV1::decode(&report.pre_custody_replay_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
    let post_custody_revision = before_replay
        .next_revision
        .checked_add(u64::from(report.payout != 0))
        .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?;
    let before_hoard = TokenAccount::parse(&report.pre_hoard_token_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
    let before_recipient = TokenAccount::parse(&report.pre_recipient_token_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
    let after_hoard_amount = before_hoard
        .amount
        .checked_sub(report.payout)
        .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?;
    let after_recipient_amount = before_recipient
        .amount
        .checked_add(report.payout)
        .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?;
    let hoard_token_bytes = token_amount_bytes(&report.pre_hoard_token_bytes, after_hoard_amount)?;
    let recipient_token_bytes =
        token_amount_bytes(&report.pre_recipient_token_bytes, after_recipient_amount)?;
    let custody_replay_bytes = if report.payout == 0 {
        if report.custody_request.is_some() || report.custody_request_digest != [0; 32] {
            return Err(WalletTerminalPayoutErrorV3::Postcondition);
        }
        report.pre_custody_replay_bytes.clone()
    } else {
        let poststate_commitment = hashv(&[
            CUSTODY_POSTSTATE_DOMAIN_V1,
            &report.custody_request_digest,
            &report.route.hoard.to_bytes(),
            &report.route.recipient.to_bytes(),
            &before_hoard.amount.to_le_bytes(),
            &after_hoard_amount.to_le_bytes(),
            &before_recipient.amount.to_le_bytes(),
            &after_recipient_amount.to_le_bytes(),
            &0_u64.to_le_bytes(),
        ])
        .to_bytes();
        CustodyReplayV1 {
            next_revision: post_custody_revision,
            last_request_digest: report.custody_request_digest,
            last_poststate_commitment: poststate_commitment,
            ..before_replay
        }
        .to_bytes()
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?
        .to_vec()
    };

    let custody_receipt_digest = if report.payout == 0 {
        [0; 32]
    } else {
        let request = report
            .custody_request
            .ok_or(WalletTerminalPayoutErrorV3::Postcondition)?;
        let replay_state_digest = hash(&custody_replay_bytes).to_bytes();
        let replay = CustodyReplayV1::decode(&custody_replay_bytes)
            .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
        let receipt = CustodyReceiptV1::new(
            request,
            report.custody_request_digest,
            ReceiptEvidenceV1 {
                source_before: before_hoard.amount,
                source_after: after_hoard_amount,
                destination_before: before_recipient.amount,
                destination_after: after_recipient_amount,
                poststate_commitment: replay.last_poststate_commitment,
                replay_state_digest,
            },
        )
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
        hash(
            &receipt
                .to_bytes()
                .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?,
        )
        .to_bytes()
    };
    let signed_post_resource_digest = hashv(&[
        SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3,
        &aggregate_bytes,
        &position_bytes,
    ])
    .to_bytes();
    let custody_replay_digest = hashv(&[&custody_replay_bytes]).to_bytes();
    let custody_token_poststate_digest = hashv(&[
        TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3,
        &hoard_token_bytes,
        &recipient_token_bytes,
    ])
    .to_bytes();
    let post_resource_digest = hashv(&[
        TERMINAL_SETTLEMENT_POST_RESOURCE_DOMAIN_V3,
        &report.request_digest,
        &signed_post_resource_digest,
        &custody_replay_digest,
        &custody_token_poststate_digest,
        &custody_receipt_digest,
    ])
    .to_bytes();
    let receipt_bytes = TerminalSettlementReceiptV3::new(
        report.request,
        dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementReceiptInputV3 {
            request_digest: report.request_digest,
            signed_packet_digest: report.signed_packet_digest,
            signed_table_digest: report.signed_table_digest,
            signed_post_resource_digest,
            custody_request_digest: report.custody_request_digest,
            custody_receipt_digest,
            custody_replay_digest,
            custody_token_poststate_digest,
            post_resource_digest,
            payout: report.payout,
            pre_market_revision: request.expected_market_revision,
            post_market_revision: request
                .expected_market_revision
                .checked_add(1)
                .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?,
            pre_position_revision: request.expected_position_revision,
            post_position_revision: request
                .expected_position_revision
                .checked_add(1)
                .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?,
            pre_custody_revision: request.expected_custody_revision,
            post_custody_revision,
        },
    )
    .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?
    .to_bytes()
    .to_vec();
    Ok(WalletTerminalPayoutExpectedPoststateV3 {
        receipt_bytes,
        aggregate_bytes,
        position_bytes,
        custody_replay_bytes,
        hoard_token_bytes,
        recipient_token_bytes,
    })
}

/// Verify the exact economic and committed poststate of one accepted payout.
pub fn verify_wallet_terminal_payout_postcondition_v3(
    report: &WalletTerminalPayoutReportV3,
    post: WalletTerminalPayoutPoststateV3<'_>,
) -> Result<(), WalletTerminalPayoutErrorV3> {
    let expected = project_wallet_terminal_payout_postcondition_v3(report)?;
    if post.receipt_bytes != expected.receipt_bytes
        || post.aggregate_bytes != expected.aggregate_bytes
        || post.position_bytes != expected.position_bytes
        || post.custody_replay_bytes != expected.custody_replay_bytes
        || post.hoard_token_bytes != expected.hoard_token_bytes
        || post.recipient_token_bytes != expected.recipient_token_bytes
    {
        return Err(WalletTerminalPayoutErrorV3::Postcondition);
    }
    Ok(())
}

fn validate_snapshot(observation: Observation) -> Result<(), WalletTerminalPayoutErrorV3> {
    if observation.finality != Finality::Finalized || observation.slot == 0 {
        return Err(WalletTerminalPayoutErrorV3::Snapshot);
    }
    Ok(())
}

fn validate_claims_route(
    input: WalletTerminalPayoutInputV3<'_>,
    market: LiabilityBasisMarketViewV2,
    position: LiabilityBasisPositionViewV2,
) -> Result<(), WalletTerminalPayoutErrorV3> {
    let route = input.route;
    let aggregate = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            market.logical_market.as_slice(),
        ],
        &route.claims_program,
    )
    .0;
    let position_seeds = ProtocolPositionSeedsV2::new(route.aggregate.to_bytes(), input.owner)
        .map_err(|_| WalletTerminalPayoutErrorV3::Claims)?;
    let expected_position =
        Pubkey::find_program_address(&position_seeds.as_slices(), &route.claims_program).0;
    let required_route_addresses = [
        route.linked_basis_raw,
        route.linked_basis_staging,
        route.product_raw,
        route.product_staging,
        route.result_domain_raw,
        route.result_domain_staging,
        route.portfolio_raw,
        route.portfolio_staging,
        route.market,
        route.activation_cache,
        route.registry_program,
        route.claims_program,
        route.claims_programdata,
        route.core_program,
        route.core_programdata,
        route.resolution_program,
        route.resolution_programdata,
        route.position,
        route.exposure_raw,
        route.exposure_staging,
        route.realm_raw,
        route.realm_staging,
    ];
    if required_route_addresses.contains(&Pubkey::default())
        || route.aggregate != aggregate
        || route.position != expected_position
        || route.market.to_bytes() != market.logical_market
        || route.registry_program.to_bytes() != market.registry_program
        || market.product_instance_id != input.admission.product_id()
        || market.basis_id != input.admission.semantic_basis_id()
        || market.release_set != input.admission.release_set_id()
        || market.logical_market != input.admission.market_id()
        || market.claim_count != input.admission.basis_width()
        || market.generation != input.expected_generation
        || market.revision != input.expected_market_revision
        || position.claim_count != market.claim_count
        || position.market_account != route.aggregate.to_bytes()
        || position.owner != input.owner
        || position.basis_id != market.basis_id
        || position.revision != input.expected_position_revision
        || input.quantity == 0
        || position
            .balance(input.position_bytes, input.claim_index)
            .map_err(|_| WalletTerminalPayoutErrorV3::Claims)?
            < input.quantity
        || market
            .supply(input.aggregate_bytes, input.claim_index)
            .map_err(|_| WalletTerminalPayoutErrorV3::Claims)?
            < input.quantity
        || route.claims_program == Pubkey::default()
        || route.claims_programdata == Pubkey::default()
        || route.core_program == Pubkey::default()
        || route.core_programdata == Pubkey::default()
        || route.resolution_program == Pubkey::default()
        || route.resolution_programdata == Pubkey::default()
        || route.activation_cache == Pubkey::default()
        || input.product_record_digest == [0; 32]
        || input.terminal_record_digest == [0; 32]
        || route.terminal_certificate.to_bytes() != input.terminal_record_digest
    {
        return Err(WalletTerminalPayoutErrorV3::Route);
    }
    Ok(())
}

fn validate_custody_route(
    input: WalletTerminalPayoutInputV3<'_>,
    market: LiabilityBasisMarketViewV2,
) -> Result<CustodyReplayV1, WalletTerminalPayoutErrorV3> {
    let route = input.route;
    let replay = CustodyReplayV1::decode(input.custody_replay_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Custody)?;
    let expected_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.logical_market,
            market.release_set,
            CustodyCallerRoleV1::Claims,
            market.custody_context,
        )
        .as_slices(),
        &route.custody_program,
    )
    .0;
    let expected_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            market.logical_market.as_slice(),
            market.release_set.as_slice(),
        ],
        &route.custody_program,
    )
    .0;
    let vault = CustodyVaultSeedsV1::new(
        market.logical_market,
        market.release_set,
        market.custody_context,
        CompartmentV1::HoardPrincipal,
    );
    let expected_hoard = Pubkey::find_program_address(&vault.as_slices(), &route.custody_program).0;
    let hoard = TokenAccount::parse(input.hoard_token_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Custody)?;
    let recipient = TokenAccount::parse(input.recipient_token_bytes)
        .map_err(|_| WalletTerminalPayoutErrorV3::Custody)?;
    if route.custody_replay != expected_replay
        || route.custody_authority != expected_authority
        || route.hoard != expected_hoard
        || replay.caller_role != CustodyCallerRoleV1::Claims
        || replay.release_set != market.release_set
        || replay.market != market.logical_market
        || replay.realm != market.realm_id
        || replay.context != market.custody_context
        || replay.caller_program != route.claims_program.to_bytes()
        || replay.next_revision == u64::MAX
        || replay.generation != market.generation
        || hoard.mint != route.collateral_mint.to_bytes()
        || hoard.owner != route.custody_authority.to_bytes()
        || hoard.state != AccountState::Initialized
        || recipient.mint != route.collateral_mint.to_bytes()
        || recipient.owner != input.recipient_owner
        || recipient.state != AccountState::Initialized
        || TokenProgram::parse(route.token_program.to_bytes()).is_err()
        || route.custody_program == Pubkey::default()
        || route.token_program == Pubkey::default()
        || route.recipient == Pubkey::default()
        || route.terminal_certificate == Pubkey::default()
    {
        return Err(WalletTerminalPayoutErrorV3::Custody);
    }
    Ok(replay)
}

fn custody_caller(
    route: WalletTerminalPayoutRouteV3,
    market: LiabilityBasisMarketViewV2,
    request: TerminalSettlementRequestV3,
    request_digest: [u8; 32],
    packet_digest: [u8; 32],
    payout: u64,
) -> Result<(Pubkey, [u8; 32], Option<CustodyRequestV1>), WalletTerminalPayoutErrorV3> {
    if payout == 0 {
        return Ok((route.claims_program, [0; 32], None));
    }
    let input = request.input();
    let candidate_digest = hashv(&[
        TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        &request_digest,
        &packet_digest,
        &payout.to_le_bytes(),
        &input.exposure_digest,
        &input.terminal_record_digest,
    ])
    .to_bytes();
    let resulting_revision = input
        .expected_custody_revision
        .checked_add(1)
        .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?;
    let custody = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CustodyCallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: input.release_set,
        market: input.market,
        realm: input.realm,
        context: market.custody_context,
        caller_program: route.claims_program.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: [0; 32],
            destination_owner: input.recipient_owner,
            order: [0; 32],
            parent_request_digest: request_digest,
            order_nonce: input.expected_position_revision,
            generation: input.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: input.transfer_index,
        },
        source: route.hoard.to_bytes(),
        destination: route.recipient.to_bytes(),
        source_vault_context: market.custody_context,
        destination_vault_context: [0; 32],
        mint: route.collateral_mint.to_bytes(),
        token_program: route.token_program.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: input.expected_custody_revision,
        resulting_revision,
        amount: payout,
        rent_lamports: 0,
    };
    let custody_bytes = custody
        .to_bytes()
        .map_err(|_| WalletTerminalPayoutErrorV3::Custody)?;
    let custody_request_digest = hash(&custody_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(input.release_set).map_err(|_| WalletTerminalPayoutErrorV3::Route)?,
        input.market,
        ExecutionRoleV1::Claims,
        market.custody_context,
        custody_request_digest,
    )
    .map_err(|_| WalletTerminalPayoutErrorV3::Route)?;
    Ok((
        Pubkey::find_program_address(&seeds.as_slices(), &route.claims_program).0,
        custody_request_digest,
        Some(custody),
    ))
}

fn payout_accounts(
    route: WalletTerminalPayoutRouteV3,
    owner: Pubkey,
    custody_caller: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(owner, true),
        AccountMeta::new(route.aggregate, false),
        AccountMeta::new_readonly(route.linked_basis_raw, false),
        AccountMeta::new_readonly(route.linked_basis_staging, false),
        AccountMeta::new_readonly(route.product_raw, false),
        AccountMeta::new_readonly(route.product_staging, false),
        AccountMeta::new_readonly(route.result_domain_raw, false),
        AccountMeta::new_readonly(route.result_domain_staging, false),
        AccountMeta::new_readonly(route.portfolio_raw, false),
        AccountMeta::new_readonly(route.portfolio_staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(route.market, false),
        AccountMeta::new_readonly(route.activation_cache, false),
        AccountMeta::new_readonly(route.registry_program, false),
        AccountMeta::new_readonly(route.claims_program, false),
        AccountMeta::new_readonly(route.claims_programdata, false),
        AccountMeta::new_readonly(route.claims_program, false),
        AccountMeta::new_readonly(route.claims_programdata, false),
        AccountMeta::new_readonly(route.core_program, false),
        AccountMeta::new_readonly(route.core_programdata, false),
        AccountMeta::new(route.position, false),
        AccountMeta::new_readonly(route.exposure_raw, false),
        AccountMeta::new_readonly(route.exposure_staging, false),
        AccountMeta::new_readonly(custody_caller, false),
        AccountMeta::new_readonly(route.custody_program, false),
        AccountMeta::new_readonly(route.terminal_certificate, false),
        AccountMeta::new_readonly(route.resolution_program, false),
        AccountMeta::new_readonly(route.resolution_programdata, false),
        AccountMeta::new_readonly(route.realm_raw, false),
        AccountMeta::new_readonly(route.realm_staging, false),
        AccountMeta::new(route.custody_replay, false),
        AccountMeta::new_readonly(route.collateral_mint, false),
        AccountMeta::new(route.hoard, false),
        AccountMeta::new(route.recipient, false),
        AccountMeta::new_readonly(route.custody_authority, false),
        AccountMeta::new_readonly(route.token_program, false),
    ]
}

/// Reproduce the exact 36-account frame owned by a payout report.
///
/// Successor operators that wrap the terminal route must extend this frame,
/// never restate its account order. The returned value is derived only from
/// the report's already-authenticated route, owner, and Custody caller; callers
/// should still compare it with `report.instruction.accounts` before trusting
/// a report received across an API boundary.
#[must_use]
pub fn wallet_terminal_payout_account_frame_v3(
    report: &WalletTerminalPayoutReportV3,
) -> Vec<AccountMeta> {
    payout_accounts(report.route, report.owner, report.custody_caller)
}

fn debited_claim_bytes(
    prestate: &[u8],
    revision_offset: usize,
    vector_offset: usize,
    stride: usize,
    claim_index: u32,
    quantity: u64,
) -> Result<Vec<u8>, WalletTerminalPayoutErrorV3> {
    let mut poststate = prestate.to_vec();
    let revision = read_u64(&poststate, revision_offset)?;
    write_u64(
        &mut poststate,
        revision_offset,
        revision
            .checked_add(1)
            .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?,
    )?;
    let index =
        usize::try_from(claim_index).map_err(|_| WalletTerminalPayoutErrorV3::Arithmetic)?;
    let offset = index
        .checked_mul(stride)
        .and_then(|tail| vector_offset.checked_add(tail))
        .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?;
    let amount = read_u64(&poststate, offset)?;
    write_u64(
        &mut poststate,
        offset,
        amount
            .checked_sub(quantity)
            .ok_or(WalletTerminalPayoutErrorV3::Postcondition)?,
    )?;
    Ok(poststate)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, WalletTerminalPayoutErrorV3> {
    let end = offset
        .checked_add(8)
        .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?;
    let value: [u8; 8] = bytes
        .get(offset..end)
        .ok_or(WalletTerminalPayoutErrorV3::Postcondition)?
        .try_into()
        .map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
    Ok(u64::from_le_bytes(value))
}

fn write_u64(
    bytes: &mut [u8],
    offset: usize,
    value: u64,
) -> Result<(), WalletTerminalPayoutErrorV3> {
    let end = offset
        .checked_add(8)
        .ok_or(WalletTerminalPayoutErrorV3::Arithmetic)?;
    bytes
        .get_mut(offset..end)
        .ok_or(WalletTerminalPayoutErrorV3::Postcondition)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn token_amount_bytes(
    prestate: &[u8],
    amount: u64,
) -> Result<Vec<u8>, WalletTerminalPayoutErrorV3> {
    let mut poststate = prestate.to_vec();
    write_u64(&mut poststate, 64, amount)?;
    TokenAccount::parse(&poststate).map_err(|_| WalletTerminalPayoutErrorV3::Postcondition)?;
    Ok(poststate)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::borrow::Cow;

    use super::*;
    use dclutch_claims_svm::{
        liability_basis_state_v2::{
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
            encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
            liability_basis_vector_width_v2,
        },
        terminal_settlement_v3::TERMINAL_SETTLEMENT_REQUEST_BYTES_V3,
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BasisInputV3, BasisKindV3, basis_record_bytes_v3, compile_basis_v3,
    };
    use dclutch_representation_composition_v3_kernel::{
        CompositionExposureInputV3, CompositionExposureRowInputV3, CompositionExposureTermV3,
        RecordAdmissionV3, composition_exposure_bytes_v3, encode_composition_exposure_v3_atomic,
    };
    use solana_address_lookup_table_interface::state::LookupTableMeta;

    const MARKET: [u8; 32] = [10; 32];
    const RELEASE_SET: [u8; 32] = [11; 32];
    const REGISTRY: [u8; 32] = [12; 32];
    const PRODUCT: [u8; 32] = [13; 32];
    const SEMANTIC_BASIS: [u8; 32] = [14; 32];
    const REALM: [u8; 32] = [15; 32];
    const CUSTODY_CONTEXT: [u8; 32] = [16; 32];
    const OWNER: [u8; 32] = [17; 32];
    const LINKED_BASIS: [u8; 32] = [18; 32];
    const PRODUCT_RECORD: [u8; 32] = [19; 32];
    const EXPOSURE: [u8; 32] = [20; 32];
    const RESULT_DOMAIN: [u8; 32] = [21; 32];
    const COORDINATE_DOMAIN: [u8; 32] = [22; 32];
    const RESULT_UNIT: [u8; 32] = [23; 32];
    const EVALUATOR: [u8; 32] = [24; 32];

    struct Fixture {
        observation: Observation,
        route: WalletTerminalPayoutRouteV3,
        admission: ProductClaimsTerminalAdmissionV3,
        exposure_admission: RecordAdmissionV3,
        basis: Vec<u8>,
        exposure: Vec<u8>,
        aggregate: Vec<u8>,
        position: Vec<u8>,
        replay: Vec<u8>,
        hoard: Vec<u8>,
        recipient: Vec<u8>,
    }

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn observation() -> Observation {
        Observation {
            slot: 99,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn token(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
        let mut bytes = vec![0_u8; 165];
        bytes[0..32].copy_from_slice(&mint.to_bytes());
        bytes[32..64].copy_from_slice(&owner.to_bytes());
        bytes[64..72].copy_from_slice(&amount.to_le_bytes());
        bytes[108] = 1;
        TokenAccount::parse(&bytes).expect("canonical token account");
        bytes
    }

    fn fixture() -> Fixture {
        let claims_program = key(40);
        let custody_program = key(41);
        let collateral_mint = key(42);
        let aggregate = Pubkey::find_program_address(
            &[LIABILITY_BASIS_MARKET_SEED_V2, MARKET.as_slice()],
            &claims_program,
        )
        .0;
        let position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), OWNER)
                .expect("Position seeds")
                .as_slices(),
            &claims_program,
        )
        .0;
        let custody_replay = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::new(
                MARKET,
                RELEASE_SET,
                CustodyCallerRoleV1::Claims,
                CUSTODY_CONTEXT,
            )
            .as_slices(),
            &custody_program,
        )
        .0;
        let custody_authority = Pubkey::find_program_address(
            &[
                CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
                MARKET.as_slice(),
                RELEASE_SET.as_slice(),
            ],
            &custody_program,
        )
        .0;
        let hoard = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                MARKET,
                RELEASE_SET,
                CUSTODY_CONTEXT,
                CompartmentV1::HoardPrincipal,
            )
            .as_slices(),
            &custody_program,
        )
        .0;
        let route = WalletTerminalPayoutRouteV3 {
            aggregate,
            linked_basis_raw: key(50),
            linked_basis_staging: key(51),
            product_raw: key(52),
            product_staging: key(53),
            result_domain_raw: key(54),
            result_domain_staging: key(55),
            portfolio_raw: key(56),
            portfolio_staging: key(57),
            market: Pubkey::new_from_array(MARKET),
            activation_cache: key(58),
            registry_program: Pubkey::new_from_array(REGISTRY),
            claims_program,
            claims_programdata: key(59),
            core_program: key(60),
            core_programdata: key(61),
            resolution_program: key(67),
            resolution_programdata: key(68),
            position,
            exposure_raw: key(62),
            exposure_staging: key(63),
            custody_program,
            terminal_certificate: key(29),
            realm_raw: key(64),
            realm_staging: key(65),
            custody_replay,
            collateral_mint,
            hoard,
            recipient: key(66),
            custody_authority,
            token_program: Pubkey::new_from_array(dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID),
        };

        let basis_bytes =
            basis_record_bytes_v3(BasisKindV3::CategoricalQ1, 3, 0, 0).expect("basis bytes");
        let mut basis = vec![0_u8; basis_bytes];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: PRODUCT,
                result_domain_id: RESULT_DOMAIN,
                coordinate_domain_id: COORDINATE_DOMAIN,
                result_unit_id: RESULT_UNIT,
                evaluator_release_id: EVALUATOR,
                basis_width: 3,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
                // Exempt by proof: degree 0 and 1 need no price gate,
                // and a digest offered alongside one is refused.
                price_gate_certificate_digest: [0_u8; 32],
            },
            &mut basis,
        )
        .expect("categorical Product basis");

        let terms = [
            [CompositionExposureTermV3 {
                product_coordinate: 0,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 2,
                numerator: 1,
            }],
        ];
        let rows = [
            CompositionExposureRowInputV3 {
                node_id: [70; 32],
                denominator: 1,
                terms: &terms[0],
            },
            CompositionExposureRowInputV3 {
                node_id: [71; 32],
                denominator: 1,
                terms: &terms[1],
            },
        ];
        let exposure_bytes = composition_exposure_bytes_v3(2, 2).expect("exposure bytes");
        let mut exposure_scratch = vec![0_u8; exposure_bytes];
        let mut exposure = vec![0_u8; exposure_bytes];
        encode_composition_exposure_v3_atomic(
            CompositionExposureInputV3 {
                market: MARKET,
                result_domain: RESULT_DOMAIN,
                release_set: RELEASE_SET,
                product_basis: LINKED_BASIS,
                representation_basis: SEMANTIC_BASIS,
                graph_id: [72; 32],
                product_width: 3,
                rows: &rows,
            },
            &mut exposure_scratch,
            &mut exposure,
        )
        .expect("Product-to-Claims exposure");
        let exposure_digest = hash(&exposure).to_bytes();
        let exposure_admission = RecordAdmissionV3 {
            selected_id: EXPOSURE,
            finalized_id: EXPOSURE,
            recomputed_digest: exposure_digest,
            finalized_digest: exposure_digest,
            record_authenticated: true,
        };
        let admission = ProductClaimsTerminalAdmissionV3::new(
            EXPOSURE,
            exposure_digest,
            PRODUCT,
            RESULT_DOMAIN,
            COORDINATE_DOMAIN,
            RESULT_UNIT,
            SEMANTIC_BASIS,
            LINKED_BASIS,
            MARKET,
            RELEASE_SET,
            EVALUATOR,
            2,
            1,
        )
        .expect("terminal admission");

        let mut aggregate_bytes =
            vec![
                0_u8;
                liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, 2)
                    .expect("aggregate width")
            ];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 7,
                logical_market: MARKET,
                release_set: RELEASE_SET,
                registry_program: REGISTRY,
                product_instance_id: PRODUCT,
                basis_id: SEMANTIC_BASIS,
                realm_id: REALM,
                custody_context: CUSTODY_CONTEXT,
                generation: 3,
            },
            &[5, 7],
            &mut aggregate_bytes,
        )
        .expect("aggregate");
        let mut position_bytes =
            vec![
                0_u8;
                liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, 2)
                    .expect("Position width")
            ];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 11,
                market_account: aggregate.to_bytes(),
                owner: OWNER,
                basis_id: SEMANTIC_BASIS,
            },
            &[2, 4],
            &mut position_bytes,
        )
        .expect("Position");
        let replay = CustodyReplayV1 {
            caller_role: CustodyCallerRoleV1::Claims,
            release_set: RELEASE_SET,
            market: MARKET,
            realm: REALM,
            context: CUSTODY_CONTEXT,
            caller_program: claims_program.to_bytes(),
            rent_refund: [25; 32],
            open_vault_count: 1,
            next_revision: 5,
            generation: 3,
            last_request_digest: [26; 32],
            last_poststate_commitment: [27; 32],
        }
        .to_bytes()
        .expect("Claims replay")
        .to_vec();
        Fixture {
            observation: observation(),
            route,
            admission,
            exposure_admission,
            basis,
            exposure,
            aggregate: aggregate_bytes,
            position: position_bytes,
            replay,
            hoard: token(collateral_mint, custody_authority, 100),
            recipient: token(collateral_mint, Pubkey::new_from_array(OWNER), 9),
        }
    }

    fn input(fixture: &Fixture, claim_index: u32) -> WalletTerminalPayoutInputV3<'_> {
        WalletTerminalPayoutInputV3 {
            observation: fixture.observation,
            route: fixture.route,
            parent_context: [28; 32],
            terminal_record_digest: [29; 32],
            recipient_owner: OWNER,
            transfer_index: 0,
            admission: fixture.admission,
            product_basis_bytes: &fixture.basis,
            composition_exposure_bytes: &fixture.exposure,
            composition_exposure_admission: fixture.exposure_admission,
            product_record_digest: PRODUCT_RECORD,
            aggregate_bytes: &fixture.aggregate,
            position_bytes: &fixture.position,
            custody_replay_bytes: &fixture.replay,
            hoard_token_bytes: &fixture.hoard,
            recipient_token_bytes: &fixture.recipient,
            terminal: TerminalScenarioV3::Categorical(2),
            owner: OWNER,
            claim_index,
            quantity: 2,
            expected_generation: 3,
            expected_market_revision: 7,
            expected_position_revision: 11,
        }
    }

    pub(crate) fn test_report(claim_index: u32) -> WalletTerminalPayoutReportV3 {
        let fixture = fixture();
        build_wallet_terminal_payout_v3(input(&fixture, claim_index)).expect("test payout report")
    }

    pub(crate) fn test_report_for_recipient(
        claim_index: u32,
        recipient: Pubkey,
        recipient_owner: Pubkey,
    ) -> WalletTerminalPayoutReportV3 {
        let mut fixture = fixture();
        fixture.route.recipient = recipient;
        fixture.recipient = token(fixture.route.collateral_mint, recipient_owner, 9);
        let mut input = input(&fixture, claim_index);
        input.recipient_owner = recipient_owner.to_bytes();
        build_wallet_terminal_payout_v3(input).expect("recipient-bound test payout report")
    }

    fn lookup(report: &WalletTerminalPayoutReportV3, payer: Pubkey) -> ObservedAccount {
        let addresses = canonical_wallet_terminal_payout_lookup_addresses_v3(report, payer)
            .expect("canonical lookup addresses");
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(key(240)),
                last_extended_slot: 98,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: report.observation,
            key: key(241),
            owner: lookup_table_program::id(),
            lamports: 1_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("lookup bytes"),
        }
    }

    fn accepted_poststate(
        report: &WalletTerminalPayoutReportV3,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let expected = project_wallet_terminal_payout_postcondition_v3(report)
            .expect("accepted wallet payout poststate");
        (
            expected.receipt_bytes,
            expected.aggregate_bytes,
            expected.position_bytes,
            expected.custody_replay_bytes,
            expected.hoard_token_bytes,
            expected.recipient_token_bytes,
        )
    }

    #[test]
    fn builder_emits_exact_wallet_frame_for_positive_and_zero_payouts() {
        let fixture = fixture();
        let positive =
            build_wallet_terminal_payout_v3(input(&fixture, 1)).expect("positive payout");
        assert_eq!(positive.payout, 2);
        assert_eq!(
            positive.instruction.accounts.len(),
            TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3
        );
        assert_eq!(
            positive.instruction.data.len(),
            TERMINAL_SETTLEMENT_REQUEST_BYTES_V3
        );
        assert_eq!(
            positive.instruction.accounts[0],
            AccountMeta::new_readonly(key(17), true)
        );
        assert_eq!(
            positive.instruction.accounts[20].pubkey,
            fixture.route.position
        );
        assert_eq!(
            positive.instruction.accounts[23].pubkey,
            positive.custody_caller
        );
        assert_ne!(positive.custody_caller, fixture.route.claims_program);
        assert_ne!(positive.custody_request_digest, [0; 32]);

        let zero = build_wallet_terminal_payout_v3(input(&fixture, 0)).expect("zero payout");
        assert_eq!(zero.payout, 0);
        assert_eq!(zero.custody_caller, fixture.route.claims_program);
        assert_eq!(zero.custody_request_digest, [0; 32]);

        let mut substituted = input(&fixture, 1);
        substituted.route.position = key(200);
        assert_eq!(
            build_wallet_terminal_payout_v3(substituted),
            Err(WalletTerminalPayoutErrorV3::Route)
        );
    }

    #[test]
    fn canonical_alt_is_packet_safe_and_refuses_reorder_extra_or_stale() {
        let fixture = fixture();
        let report = build_wallet_terminal_payout_v3(input(&fixture, 1)).expect("payout");
        let payer = key(242);
        let canonical = canonical_wallet_terminal_payout_lookup_addresses_v3(&report, payer)
            .expect("canonical addresses");
        assert_eq!(canonical.len(), 33);
        assert_eq!(canonical.first(), Some(&fixture.route.claims_program));
        assert!(!canonical.contains(&payer));
        assert!(!canonical.contains(&report.owner));
        let table = lookup(&report, payer);
        let plan = compile_wallet_terminal_payout_v0(
            report.clone(),
            payer,
            Hash::new_from_array([31; 32]),
            &table,
        )
        .expect("packet-safe payout");
        assert_eq!(plan.required_signers, vec![payer, key(17)]);
        assert_eq!(plan.message.loaded_addresses, 32);
        // Four static keys: the payer, the owner, the Claims program, and the
        // ComputeBudget program whose limit declaration this route cannot land
        // without. A program id compiles static, so the ComputeBudget prefix
        // costs a static key and NOT a lookup entry -- the canonical table is
        // still exactly the payout instruction's own coordinates, and a table
        // frozen before the prefix existed stays valid.
        assert_eq!(plan.message.message.static_account_keys().len(), 4);
        assert_eq!(plan.message.loaded_addresses + 4, 36);
        assert!(plan.message.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
        // The prefix is the whole difference: same single payout instruction,
        // now behind one compute-unit declaration.
        assert_eq!(plan.message.message.instructions().len(), 2);

        let mut reordered = table.clone();
        let decoded = AddressLookupTable::deserialize(&reordered.data).expect("table");
        let mut addresses = decoded.addresses.into_owned();
        addresses.swap(0, 1);
        reordered.data = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(addresses),
        }
        .serialize_for_tests()
        .expect("reordered lookup");
        assert_eq!(
            compile_wallet_terminal_payout_v0(
                report.clone(),
                payer,
                Hash::new_from_array([31; 32]),
                &reordered,
            ),
            Err(WalletTerminalPayoutErrorV3::LookupTable)
        );

        let mut extra = table.clone();
        let decoded = AddressLookupTable::deserialize(&extra.data).expect("table");
        let mut addresses = decoded.addresses.into_owned();
        addresses.push(key(243));
        extra.data = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(addresses),
        }
        .serialize_for_tests()
        .expect("extra lookup");
        assert_eq!(
            compile_wallet_terminal_payout_v0(
                report.clone(),
                payer,
                Hash::new_from_array([31; 32]),
                &extra,
            ),
            Err(WalletTerminalPayoutErrorV3::LookupTable)
        );
        let mut stale = table;
        stale.observation.slot += 1;
        assert_eq!(
            compile_wallet_terminal_payout_v0(
                report,
                payer,
                Hash::new_from_array([31; 32]),
                &stale,
            ),
            Err(WalletTerminalPayoutErrorV3::LookupTable)
        );
    }

    #[test]
    fn exact_positive_postcondition_accepts_and_substitutions_refuse() {
        let fixture = fixture();
        let report = build_wallet_terminal_payout_v3(input(&fixture, 1)).expect("payout");
        let (receipt, aggregate, position, replay, hoard, recipient) = accepted_poststate(&report);
        let post = WalletTerminalPayoutPoststateV3 {
            receipt_bytes: &receipt,
            aggregate_bytes: &aggregate,
            position_bytes: &position,
            custody_replay_bytes: &replay,
            hoard_token_bytes: &hoard,
            recipient_token_bytes: &recipient,
        };
        assert_eq!(
            verify_wallet_terminal_payout_postcondition_v3(&report, post),
            Ok(())
        );

        let mut changed_recipient = recipient.clone();
        changed_recipient[64] ^= 1;
        let changed = WalletTerminalPayoutPoststateV3 {
            recipient_token_bytes: &changed_recipient,
            ..post
        };
        assert_eq!(
            verify_wallet_terminal_payout_postcondition_v3(&report, changed),
            Err(WalletTerminalPayoutErrorV3::Postcondition)
        );
        let mut changed_aggregate = aggregate.clone();
        *changed_aggregate.last_mut().expect("aggregate tail") ^= 1;
        let changed = WalletTerminalPayoutPoststateV3 {
            aggregate_bytes: &changed_aggregate,
            ..post
        };
        assert_eq!(
            verify_wallet_terminal_payout_postcondition_v3(&report, changed),
            Err(WalletTerminalPayoutErrorV3::Postcondition)
        );
    }

    #[test]
    fn zero_payout_burn_requires_byte_identical_custody_and_tokens() {
        let fixture = fixture();
        let report = build_wallet_terminal_payout_v3(input(&fixture, 0)).expect("zero payout");
        let (receipt, aggregate, position, replay, hoard, recipient) = accepted_poststate(&report);
        let post = WalletTerminalPayoutPoststateV3 {
            receipt_bytes: &receipt,
            aggregate_bytes: &aggregate,
            position_bytes: &position,
            custody_replay_bytes: &replay,
            hoard_token_bytes: &hoard,
            recipient_token_bytes: &recipient,
        };
        assert_eq!(
            verify_wallet_terminal_payout_postcondition_v3(&report, post),
            Ok(())
        );
        let mut changed_replay = replay.clone();
        changed_replay[264] ^= 1;
        assert_eq!(
            verify_wallet_terminal_payout_postcondition_v3(
                &report,
                WalletTerminalPayoutPoststateV3 {
                    custody_replay_bytes: &changed_replay,
                    ..post
                },
            ),
            Err(WalletTerminalPayoutErrorV3::Postcondition)
        );
    }
}
