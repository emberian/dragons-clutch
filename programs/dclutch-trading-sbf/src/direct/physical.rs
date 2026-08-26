//! Commit-last Direct successor state candidates and shared physical facts.
//!
//! This module is not a transition or effect authority. The composing Trading
//! outer authenticates the descriptor, account profile, fixed-role programs,
//! accounts, and receipts, then commits these Direct candidates last.

use dclutch_direct_codec::successor::{
    DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_REGISTERED_RECORD_BYTES_V2, DirectExecutionConfigV1,
    RegisteredOrdinarySettlementV2, RegisteredRecordAfterFillV2, RegisteredRecordCloseV2,
};

/// Stable refusal from Direct physical projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPhysicalError {
    /// A required program, content, account, or digest identity was zero.
    ZeroIdentity,
    /// Market, generation, maker, config, or endpoint bindings differed.
    Binding,
    /// Runtime Product width or caller-owned buffers had another exact width.
    Width,
    /// Checked balance, revision, or vector arithmetic failed.
    Arithmetic,
    /// The sole Direct successor settlement refused.
    Settlement,
    /// Canonical Claims request construction or receipt verification refused.
    Claims,
    /// Canonical Custody request construction or receipt verification refused.
    Custody,
    /// A child receipt or observed poststate differed from the exact plan.
    Postcondition,
    /// Exact Direct state-candidate encoding or output geometry refused.
    State,
    /// The common effect profile cannot yet create or close required Trading state.
    LifecycleUnavailable,
}

/// Result alias for Direct physical planning.
pub type Result<T> = core::result::Result<T, DirectPhysicalError>;

/// One authenticated external collateral token account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExternalCollateralV2 {
    /// Exact token-account key.
    pub account: [u8; 32],
    /// Exact persisted external token authority.
    pub owner: [u8; 32],
    /// Authenticated token amount before this outer action.
    pub balance: u64,
}

/// One authenticated external collateral source delegated to Custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExternalDebitV2 {
    /// Exact token-account key.
    pub account: [u8; 32],
    /// Exact persisted external token authority.
    pub owner: [u8; 32],
    /// Canonical Custody transfer-authority PDA.
    pub delegate: [u8; 32],
    /// Exact remaining delegated allowance before this fill.
    pub delegated_amount: u64,
    /// Authenticated token amount before this fill.
    pub balance: u64,
}

/// Terminal disposition of one registered-record account after child success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRecordCommitV2 {
    /// Persist the encoded live record from the corresponding output buffer.
    WriteLive,
    /// Close the record and route exact rent plus donation as described.
    Close(RegisteredRecordCloseV2),
}

/// Direct state candidate encoded only after the semantic settlement accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryStateCandidateV2 {
    /// Seller record disposition.
    pub seller_record: DirectRecordCommitV2,
    /// Buyer record disposition.
    pub buyer_record: DirectRecordCommitV2,
}

/// Caller-owned scratch and output buffers for commit-last Direct state.
///
/// Scratch may change on refusal. Every output is byte-for-byte unchanged on
/// refusal. For a closed record its output remains unchanged on success too;
/// the Trading outer closes that account only after all child receipts accept.
pub struct DirectOrdinaryStateBuffersV2<'a> {
    /// Seller maker-root encoded output.
    pub seller_maker_output: &'a mut [u8],
    /// Buyer maker-root encoded output.
    pub buyer_maker_output: &'a mut [u8],
    /// Seller live-record encoding scratch.
    pub seller_record_scratch: &'a mut [u8],
    /// Buyer live-record encoding scratch.
    pub buyer_record_scratch: &'a mut [u8],
    /// Seller live-record encoded output.
    pub seller_record_output: &'a mut [u8],
    /// Buyer live-record encoded output.
    pub buyer_record_output: &'a mut [u8],
}

/// Encode the complete ordinary Direct state candidate without committing it.
///
/// The common Trading outer invokes Claims and every positive Custody request,
/// verifies their immediate producer/poststate receipts, then copies these
/// maker/record outputs or performs the exact record closes as its final local
/// effects. This function never writes authoritative accounts itself.
pub fn encode_registered_ordinary_state_candidate_v2(
    settlement: RegisteredOrdinarySettlementV2,
    config: DirectExecutionConfigV1,
    outcome_count: u32,
    buffers: DirectOrdinaryStateBuffersV2<'_>,
) -> Result<DirectOrdinaryStateCandidateV2> {
    if buffers.seller_maker_output.len() != DIRECT_MAKER_REPLAY_BYTES_V1
        || buffers.buyer_maker_output.len() != DIRECT_MAKER_REPLAY_BYTES_V1
        || buffers.seller_record_scratch.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
        || buffers.buyer_record_scratch.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
        || buffers.seller_record_output.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
        || buffers.buyer_record_output.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
    {
        return Err(DirectPhysicalError::Width);
    }
    let seller_maker = settlement
        .seller
        .maker_root
        .encode()
        .map_err(|_| DirectPhysicalError::State)?;
    let buyer_maker = settlement
        .buyer
        .maker_root
        .encode()
        .map_err(|_| DirectPhysicalError::State)?;
    let seller_record = encode_record_candidate(
        settlement.seller.record,
        config,
        outcome_count,
        buffers.seller_record_scratch,
    )?;
    let buyer_record = encode_record_candidate(
        settlement.buyer.record,
        config,
        outcome_count,
        buffers.buyer_record_scratch,
    )?;

    buffers.seller_maker_output.copy_from_slice(&seller_maker);
    buffers.buyer_maker_output.copy_from_slice(&buyer_maker);
    if seller_record == DirectRecordCommitV2::WriteLive {
        buffers
            .seller_record_output
            .copy_from_slice(buffers.seller_record_scratch);
    }
    if buyer_record == DirectRecordCommitV2::WriteLive {
        buffers
            .buyer_record_output
            .copy_from_slice(buffers.buyer_record_scratch);
    }
    Ok(DirectOrdinaryStateCandidateV2 {
        seller_record,
        buyer_record,
    })
}

fn encode_record_candidate(
    candidate: RegisteredRecordAfterFillV2,
    config: DirectExecutionConfigV1,
    outcome_count: u32,
    scratch: &mut [u8],
) -> Result<DirectRecordCommitV2> {
    match candidate {
        RegisteredRecordAfterFillV2::Live(record) => {
            let encoded = record
                .encode_selected(config, outcome_count)
                .map_err(|_| DirectPhysicalError::State)?;
            scratch.copy_from_slice(&encoded);
            Ok(DirectRecordCommitV2::WriteLive)
        }
        RegisteredRecordAfterFillV2::Closed(close) => Ok(DirectRecordCommitV2::Close(close)),
    }
}
