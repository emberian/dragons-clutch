//! Pure exact poststate and acknowledgement projection for ordinary Direct.
//!
//! The Trading adapter authenticates programs, PDAs, account ownership, and
//! finalized records before exposing these borrowed prestates. This module is
//! the sole AccountInfo-free owner of the complete writable-account candidate,
//! child-receipt transcript, and `HotExecutionAckV3` bytes. Exterior callers use
//! the same projection and must still authenticate a finalized transaction and
//! producer before accepting the result.

use core::convert::TryFrom;

use dclutch_claims::{
    liability_basis_state_v2::{
        LiabilityBasisMarketLayoutV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionLayoutV2,
        LiabilityBasisPositionViewV2,
    },
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_BYTES_V1, SparseNativeTransferPoststateSlicesV1,
        SparseNativeTransferReceiptV1, SparseNativeTransferV1,
        sparse_native_transfer_poststate_digest_v1,
    },
};
use dclutch_custody::token_svm::{ACCOUNT_BYTES, AccountState, COption, TokenAccount};
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CustodyReplayV1, DELEGATED_CUSTODY_RECEIPT_BYTES_V2,
    DELEGATED_CUSTODY_REQUEST_BYTES_V2, DelegatedAllowanceObservationV2,
    DelegatedCustodyPoststateFactsV2, DelegatedCustodyReceiptV2, DelegatedCustodyRequestV2,
    ReceiptEvidenceV1, delegated_custody_child_execution_digest_v3,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1,
    hot_v3::{HOT_EXECUTION_ACK_BYTES_V3, HotExecutionAckV3},
};
use dclutch_sha256_adapter::{digest, digestv};

use crate::{
    inline_candidate_v2::{
        DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2, DirectInlineCandidateContextV2,
        DirectInlineCandidateV2, DirectInlineCollateralFrameV2, DirectInlineEffectDispatchV2,
        prepare_and_verify_inline_effect_partition_v2,
    },
    successor::{
        DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_STATE_BYTES_V1, InlineOrdinaryInputV2,
        MakerReplayObservationV1, MakerReplayRootV1,
    },
};

/// Domain of the complete Hot execution commitment.
pub const HOT_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-execution:v3";
/// Domain of the ordered child-receipt transcript.
pub const HOT_CHILD_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-child-execution:v3";
/// Number of writable Direct accounts committed in canonical order.
pub const DIRECT_INLINE_POSTSTATE_COUNT_V3: usize = 10;

/// Stable refusal from exact Direct finalization projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectFinalizationErrorV3 {
    /// A borrowed wire or account had another exact width.
    Width,
    /// A required address, program, or digest was zero.
    ZeroIdentity,
    /// Two semantic writable accounts aliased.
    Alias,
    /// An account owner, inner authority, mint, state, or immutable field differed.
    Binding,
    /// Checked revision, balance, length, or transcript arithmetic overflowed.
    Arithmetic,
    /// The authenticated Direct candidate or Effect partition refused.
    Candidate,
    /// Claims prestate, exact request, or receipt projection refused.
    Claims,
    /// Custody replay, exact request, or receipt projection refused.
    Custody,
    /// Token account prestate or exact transfer projection refused.
    Token,
    /// A caller requested a noncanonical commitment role or order.
    Order,
    /// Canonical Hot acknowledgement construction refused.
    Ack,
}

/// Result alias for exact Direct finalization projection.
pub type Result<T> = core::result::Result<T, DirectFinalizationErrorV3>;

/// One borrowed, owner-authenticated writable account prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineAccountPrestateV3<'a> {
    /// Exact account address.
    pub address: [u8; 32],
    /// Exact outer SVM owner program.
    pub owner: [u8; 32],
    /// Exact lamports before the instruction.
    pub lamports: u64,
    /// Complete account data before the instruction.
    pub data: &'a [u8],
}

/// All writable accounts participating in one ordinary Direct execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineAccountPrestatesV3<'a> {
    /// Mutable capability root.
    pub root: DirectInlineAccountPrestateV3<'a>,
    /// Seller maker replay root.
    pub seller_maker_replay: DirectInlineAccountPrestateV3<'a>,
    /// Buyer maker replay root.
    pub buyer_maker_replay: DirectInlineAccountPrestateV3<'a>,
    /// Claims LiabilityBasis aggregate.
    pub claims_market: DirectInlineAccountPrestateV3<'a>,
    /// Seller Claims Position.
    pub seller_position: DirectInlineAccountPrestateV3<'a>,
    /// Buyer Claims Position.
    pub buyer_position: DirectInlineAccountPrestateV3<'a>,
    /// Trading-role Custody replay.
    pub custody_replay: DirectInlineAccountPrestateV3<'a>,
    /// Buyer collateral source token account.
    pub buyer_token: DirectInlineAccountPrestateV3<'a>,
    /// Seller collateral destination token account.
    pub seller_token: DirectInlineAccountPrestateV3<'a>,
    /// Fee collateral destination token account.
    pub fee_token: DirectInlineAccountPrestateV3<'a>,
}

impl<'a> DirectInlineAccountPrestatesV3<'a> {
    fn ordered(self) -> [DirectInlineAccountPrestateV3<'a>; DIRECT_INLINE_POSTSTATE_COUNT_V3] {
        [
            self.root,
            self.seller_maker_replay,
            self.buyer_maker_replay,
            self.claims_market,
            self.seller_position,
            self.buyer_position,
            self.custody_replay,
            self.buyer_token,
            self.seller_token,
            self.fee_token,
        ]
    }
}

/// Exact Registry-selected producer programs used by the finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineFinalizationProgramsV3 {
    /// Current Trading program and owner of root/maker replay state.
    pub trading: [u8; 32],
    /// Current Claims receipt producer and owner of LiabilityBasis state.
    pub claims: [u8; 32],
    /// Current Custody receipt producer and owner of replay state.
    pub custody: [u8; 32],
    /// Realm-selected token program and owner of all three token accounts.
    pub token: [u8; 32],
}

/// Artifact coordinates committed by the generic Hot acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotExecutionArtifactFactsV3 {
    /// Selected CapabilityProgram content identity.
    pub selected_program: [u8; 32],
    /// Selected AccountProfile program identity.
    pub account_profile_program: [u8; 32],
    /// Selected RequestProfile program identity.
    pub request_profile_program: [u8; 32],
    /// Selected execution-strategy record program identity.
    pub strategy_program: [u8; 32],
    /// Strategy transition program identity.
    pub strategy_transition_program: [u8; 32],
    /// Selected Effect program identity.
    pub effect_program: [u8; 32],
    /// Descriptor derivation-policy identity.
    pub derivation_policy: [u8; 32],
    /// Descriptor configuration identity.
    pub config: [u8; 32],
    /// Market Product record identity.
    pub product_record: [u8; 32],
    /// Finalized linked-basis raw content digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Product-authenticated semantic basis identity.
    pub semantic_basis_id: [u8; 32],
    /// Product-authenticated outcome count.
    pub outcome_count: u32,
    /// Authenticated strategy execution transcript.
    pub strategy_execution_digest: [u8; 32],
}

/// Common immutable inputs to the exact Hot acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotExecutionAckInputV3 {
    /// Current release-set identity.
    pub release_set: [u8; 32],
    /// Current Core Market.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Mutable capability-root address.
    pub root: [u8; 32],
    /// SHA-256 of the complete family request.
    pub request_digest: [u8; 32],
    /// Complete root digest before execution.
    pub root_prestate_digest: [u8; 32],
    /// Exact selected artifact facts.
    pub artifacts: HotExecutionArtifactFactsV3,
}

/// Complete pure finalization input for one ordinary Direct execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineFinalizationInputV3<'a> {
    /// Authenticated signed pair and Direct root observations.
    pub direct: &'a InlineOrdinaryInputV2,
    /// PDA-free immutable Market and Realm semantic context.
    pub context: &'a DirectInlineCandidateContextV2,
    /// Stable Product identity named by the Core Market and Claims aggregate.
    pub product_id: [u8; 32],
    /// Exact token-account semantic prestates.
    pub collateral: &'a DirectInlineCollateralFrameV2,
    /// Complete authenticated Effect request bank.
    pub request_bank: &'a [u8],
    /// Exhaustive/disjoint enabled Effect partition.
    pub dispatch: DirectInlineEffectDispatchV2,
    /// Complete canonical parent family request.
    pub family_request: &'a [u8],
    /// Complete writable-account prestates.
    pub accounts: &'a DirectInlineAccountPrestatesV3<'a>,
    /// Current Registry-selected producer programs.
    pub programs: DirectInlineFinalizationProgramsV3,
    /// Generic Hot acknowledgement inputs.
    pub ack: &'a HotExecutionAckInputV3,
}

/// Canonical ordered writable-account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DirectInlinePoststateRoleV3 {
    /// Mutable capability root.
    Root = 0,
    /// Seller maker replay root.
    SellerMakerReplay = 1,
    /// Buyer maker replay root.
    BuyerMakerReplay = 2,
    /// Claims LiabilityBasis aggregate.
    ClaimsMarket = 3,
    /// Seller Claims Position.
    SellerPosition = 4,
    /// Buyer Claims Position.
    BuyerPosition = 5,
    /// Trading-role Custody replay.
    CustodyReplay = 6,
    /// Buyer collateral source token account.
    BuyerToken = 7,
    /// Seller collateral destination token account.
    SellerToken = 8,
    /// Fee collateral destination token account.
    FeeToken = 9,
}

impl DirectInlinePoststateRoleV3 {
    /// Return the exact role for one canonical ordered commitment index.
    pub fn from_index(index: usize) -> Result<Self> {
        match index {
            0 => Ok(Self::Root),
            1 => Ok(Self::SellerMakerReplay),
            2 => Ok(Self::BuyerMakerReplay),
            3 => Ok(Self::ClaimsMarket),
            4 => Ok(Self::SellerPosition),
            5 => Ok(Self::BuyerPosition),
            6 => Ok(Self::CustodyReplay),
            7 => Ok(Self::BuyerToken),
            8 => Ok(Self::SellerToken),
            9 => Ok(Self::FeeToken),
            _ => Err(DirectFinalizationErrorV3::Order),
        }
    }
}

/// Complete commitment to one exact writable-account poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlinePoststateCommitmentV3 {
    /// Canonical role and order tag.
    pub role: DirectInlinePoststateRoleV3,
    /// Exact account address.
    pub address: [u8; 32],
    /// Exact post-execution SVM owner.
    pub owner: [u8; 32],
    /// Exact post-execution lamports.
    pub lamports: u64,
    /// Exact post-execution data width.
    pub data_len: u32,
    /// SHA-256 of the complete post-execution data bytes.
    pub data_digest: [u8; 32],
}

/// Complete ordinary Direct finalization candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineFinalizationV3 {
    /// Independently recomputed ordinary Direct economic candidate.
    pub candidate: DirectInlineCandidateV2,
    /// Ordered exact child-receipt transcript digest.
    pub child_execution_digest: [u8; 32],
    /// Exact ordered commitments for every writable account.
    pub poststates: [DirectInlinePoststateCommitmentV3; DIRECT_INLINE_POSTSTATE_COUNT_V3],
    /// Canonical generic Hot acknowledgement.
    pub ack: HotExecutionAckV3,
    /// Exact fixed-width acknowledgement bytes.
    pub ack_bytes: [u8; HOT_EXECUTION_ACK_BYTES_V3],
}

/// Caller-owned staging for the exact ordinary Direct finalization.
///
/// The SBF adapter allocates this value on its bounded heap and the codec fills
/// each checked component in canonical order. Keeping the large result in
/// caller-owned storage prevents a complete finalization value from crossing
/// the 4KB SBF stack boundary while preserving one semantic producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineFinalizationWorkspaceV3 {
    #[cfg(not(target_os = "solana"))]
    candidate: Option<DirectInlineCandidateV2>,
    child_execution_digest: Option<[u8; 32]>,
    poststates: [Option<DirectInlinePoststateCommitmentV3>; DIRECT_INLINE_POSTSTATE_COUNT_V3],
    ack_bytes: Option<[u8; HOT_EXECUTION_ACK_BYTES_V3]>,
}

impl DirectInlineFinalizationWorkspaceV3 {
    /// Create a vacant workspace. A nonvacant workspace is always refused by
    /// the preparation entry point, so stale partial results cannot be reused.
    pub const fn vacant() -> Self {
        Self {
            #[cfg(not(target_os = "solana"))]
            candidate: None,
            child_execution_digest: None,
            poststates: [None; DIRECT_INLINE_POSTSTATE_COUNT_V3],
            ack_bytes: None,
        }
    }

    fn is_vacant(&self) -> bool {
        #[cfg(not(target_os = "solana"))]
        if self.candidate.is_some() {
            return false;
        }
        self.child_execution_digest.is_none()
            && self.poststates.iter().all(Option::is_none)
            && self.ack_bytes.is_none()
    }

    /// Return the checked economic candidate.
    #[cfg(not(target_os = "solana"))]
    pub fn candidate(&self) -> Result<&DirectInlineCandidateV2> {
        self.candidate
            .as_ref()
            .ok_or(DirectFinalizationErrorV3::Order)
    }

    /// Return the checked ordered child transcript.
    pub fn child_execution_digest(&self) -> Result<[u8; 32]> {
        self.child_execution_digest
            .ok_or(DirectFinalizationErrorV3::Order)
    }

    /// Return one checked canonical poststate commitment.
    pub fn poststate(&self, index: usize) -> Result<&DirectInlinePoststateCommitmentV3> {
        self.poststates
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(DirectFinalizationErrorV3::Order)
    }

    /// Return the checked canonical Hot acknowledgement.
    pub fn ack(&self) -> Result<HotExecutionAckV3> {
        HotExecutionAckV3::decode(self.ack_bytes()?).map_err(|_| DirectFinalizationErrorV3::Ack)
    }

    /// Return the exact checked acknowledgement bytes.
    pub fn ack_bytes(&self) -> Result<&[u8; HOT_EXECUTION_ACK_BYTES_V3]> {
        self.ack_bytes
            .as_ref()
            .ok_or(DirectFinalizationErrorV3::Order)
    }

    #[cfg(not(target_os = "solana"))]
    fn finish(&self) -> Result<DirectInlineFinalizationV3> {
        Ok(DirectInlineFinalizationV3 {
            candidate: *self.candidate()?,
            child_execution_digest: self.child_execution_digest()?,
            poststates: [
                *self.poststate(0)?,
                *self.poststate(1)?,
                *self.poststate(2)?,
                *self.poststate(3)?,
                *self.poststate(4)?,
                *self.poststate(5)?,
                *self.poststate(6)?,
                *self.poststate(7)?,
                *self.poststate(8)?,
                *self.poststate(9)?,
            ],
            ack: self.ack()?,
            ack_bytes: *self.ack_bytes()?,
        })
    }
}

/// Project the generic Hot acknowledgement from one exact child transcript and
/// complete root poststate digest.
pub fn project_hot_execution_ack_v3(
    input: HotExecutionAckInputV3,
    child_execution_digest: [u8; 32],
    root_poststate_digest: [u8; 32],
) -> Result<HotExecutionAckV3> {
    require_nonzero(&[
        input.release_set,
        input.market,
        input.root,
        input.request_digest,
        input.root_prestate_digest,
        child_execution_digest,
        root_poststate_digest,
        input.artifacts.selected_program,
        input.artifacts.account_profile_program,
        input.artifacts.request_profile_program,
        input.artifacts.strategy_program,
        input.artifacts.strategy_transition_program,
        input.artifacts.effect_program,
        input.artifacts.derivation_policy,
        input.artifacts.config,
        input.artifacts.product_record,
        input.artifacts.linked_basis_record_digest,
        input.artifacts.semantic_basis_id,
    ])?;
    if input.artifacts.outcome_count == 0 {
        return Err(DirectFinalizationErrorV3::Ack);
    }
    let execution_digest = digestv(&[
        HOT_EXECUTION_DIGEST_DOMAIN_V3,
        &input.artifacts.selected_program,
        &input.artifacts.account_profile_program,
        &input.artifacts.request_profile_program,
        &input.artifacts.strategy_program,
        &input.artifacts.strategy_transition_program,
        &input.artifacts.effect_program,
        &input.artifacts.derivation_policy,
        &input.artifacts.config,
        &input.artifacts.product_record,
        &input.artifacts.linked_basis_record_digest,
        &input.artifacts.semantic_basis_id,
        &input.artifacts.outcome_count.to_le_bytes(),
        &input.request_digest,
        &input.artifacts.strategy_execution_digest,
        &child_execution_digest,
        &root_poststate_digest,
    ]);
    HotExecutionAckV3::new(HotExecutionAckV3 {
        release_set: input.release_set,
        market: input.market,
        generation: input.generation,
        root: input.root,
        request_digest: input.request_digest,
        selected_program: input.artifacts.selected_program,
        root_prestate_digest: input.root_prestate_digest,
        root_poststate_digest,
        execution_digest,
    })
    .map_err(|_| DirectFinalizationErrorV3::Ack)
}

/// Recompute the exact ordinary Direct candidate, complete poststate
/// commitments, child transcript, and canonical Hot acknowledgement.
#[cfg(not(target_os = "solana"))]
#[inline(never)]
pub fn prepare_direct_inline_finalization_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
) -> Result<DirectInlineFinalizationV3> {
    let mut output = DirectInlineFinalizationWorkspaceV3::vacant();
    prepare_direct_inline_finalization_into_v3(input, &mut output)?;
    output.finish()
}

/// Recompute the complete finalization directly into caller-owned storage.
///
/// SBF callers use an initially-empty heap slot so the fixed 1.5KB result is
/// never materialized in the adapter's 4KB stack frame. A nonempty slot is
/// refused, preventing stale or partially reused output from becoming a
/// second semantic input.
#[inline(never)]
pub fn prepare_direct_inline_finalization_into_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    output: &mut DirectInlineFinalizationWorkspaceV3,
) -> Result<()> {
    if !output.is_vacant() {
        return Err(DirectFinalizationErrorV3::Order);
    }
    validate_input_bindings(input)?;
    let candidate = prepare_and_verify_inline_effect_partition_v2(
        *input.direct,
        *input.context,
        *input.collateral,
        input.request_bank,
        input.dispatch,
    )
    .map_err(|_| DirectFinalizationErrorV3::Candidate)?;
    #[cfg(not(target_os = "solana"))]
    {
        output.candidate = Some(candidate);
    }
    prepare_local_poststates_v3(input, &candidate, &mut output.poststates)?;
    project_claims_into_v3(
        input,
        &candidate,
        &mut output.poststates,
        &mut output.child_execution_digest,
    )?;
    project_custody_into_v3(
        input,
        &candidate,
        &mut output.poststates,
        &mut output.child_execution_digest,
        None,
    )?;
    project_tokens_into_v3(input, &candidate, &mut output.poststates)?;
    validate_optional_commitment_order_v3(&output.poststates)?;
    let child_execution_digest = output
        .child_execution_digest
        .ok_or(DirectFinalizationErrorV3::Order)?;
    let root_poststate_digest = output
        .poststates
        .first()
        .and_then(Option::as_ref)
        .ok_or(DirectFinalizationErrorV3::Order)?
        .data_digest;
    let ack =
        project_hot_execution_ack_v3(*input.ack, child_execution_digest, root_poststate_digest)?;
    output.ack_bytes = Some(ack.to_bytes());
    Ok(())
}

/// Project one complete exact account poststate into a caller-owned slice.
///
/// The function recomputes and validates the same candidate as
/// [`prepare_direct_inline_finalization_v3`]. Exterior callers use this to
/// materialize bytes and then require their SHA-256 to equal the corresponding
/// ordered commitment.
pub fn project_direct_inline_account_poststate_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    role: DirectInlinePoststateRoleV3,
    output: &mut [u8],
) -> Result<()> {
    validate_input_bindings(input)?;
    let candidate = prepare_and_verify_inline_effect_partition_v2(
        *input.direct,
        *input.context,
        *input.collateral,
        input.request_bank,
        input.dispatch,
    )
    .map_err(|_| DirectFinalizationErrorV3::Candidate)?;
    match role {
        DirectInlinePoststateRoleV3::Root => {
            copy_exact(output, input.accounts.root.data)?;
            let tail = output
                .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .ok_or(DirectFinalizationErrorV3::Width)?;
            copy_exact(tail, &candidate.settlement.root.encode())
        }
        DirectInlinePoststateRoleV3::SellerMakerReplay => copy_exact(
            output,
            &candidate
                .settlement
                .seller_maker_root
                .encode()
                .map_err(|_| DirectFinalizationErrorV3::Candidate)?,
        ),
        DirectInlinePoststateRoleV3::BuyerMakerReplay => copy_exact(
            output,
            &candidate
                .settlement
                .buyer_maker_root
                .encode()
                .map_err(|_| DirectFinalizationErrorV3::Candidate)?,
        ),
        DirectInlinePoststateRoleV3::ClaimsMarket => project_claims_account_bytes_v3(
            input.accounts.claims_market.data,
            LiabilityBasisMarketLayoutV2::REVISION,
            candidate.claims_market_revision_after,
            None,
            output,
        ),
        DirectInlinePoststateRoleV3::SellerPosition => project_claims_account_bytes_v3(
            input.accounts.seller_position.data,
            LiabilityBasisPositionLayoutV2::REVISION,
            candidate.seller_position_revision_after,
            Some((
                LiabilityBasisPositionLayoutV2::BALANCES,
                input.direct.seller.authenticated.intent().outcome,
                false,
                input.direct.execution.fill,
            )),
            output,
        ),
        DirectInlinePoststateRoleV3::BuyerPosition => project_claims_account_bytes_v3(
            input.accounts.buyer_position.data,
            LiabilityBasisPositionLayoutV2::REVISION,
            candidate.buyer_position_revision_after,
            Some((
                LiabilityBasisPositionLayoutV2::BALANCES,
                input.direct.seller.authenticated.intent().outcome,
                true,
                input.direct.execution.fill,
            )),
            output,
        ),
        DirectInlinePoststateRoleV3::CustodyReplay => {
            let claims = project_claims_v3(input, &candidate)?;
            let mut poststates = [None; DIRECT_INLINE_POSTSTATE_COUNT_V3];
            let mut child_execution_digest = Some(claims.child_transcript);
            let mut replay_bytes = [0_u8; CUSTODY_REPLAY_BYTES_V1];
            project_custody_into_v3(
                input,
                &candidate,
                &mut poststates,
                &mut child_execution_digest,
                Some(&mut replay_bytes),
            )?;
            copy_exact(output, &replay_bytes)
        }
        DirectInlinePoststateRoleV3::BuyerToken => {
            let bytes = buyer_token_poststate_v3(input, &candidate)?;
            copy_exact(output, &bytes)
        }
        DirectInlinePoststateRoleV3::SellerToken => {
            let bytes = TokenAccount::project_amount_poststate(
                input.accounts.seller_token.data,
                candidate.seller_destination_after,
            )
            .map_err(|_| DirectFinalizationErrorV3::Token)?;
            copy_exact(output, &bytes)
        }
        DirectInlinePoststateRoleV3::FeeToken => {
            let bytes = TokenAccount::project_amount_poststate(
                input.accounts.fee_token.data,
                candidate.fee_destination_after,
            )
            .map_err(|_| DirectFinalizationErrorV3::Token)?;
            copy_exact(output, &bytes)
        }
    }
}

#[inline(never)]
fn prepare_local_poststates_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
    poststates: &mut [Option<DirectInlinePoststateCommitmentV3>; DIRECT_INLINE_POSTSTATE_COUNT_V3],
) -> Result<()> {
    put_poststate_v3(poststates, 0, root_commitment_v3(input, candidate)?)?;
    put_poststate_v3(
        poststates,
        1,
        maker_commitment_v3(
            input.accounts.seller_maker_replay,
            input.programs.trading,
            input.direct.seller.maker_replay,
            candidate.settlement.seller_maker_root,
            candidate.settlement.seller_creation,
            DirectInlinePoststateRoleV3::SellerMakerReplay,
        )?,
    )?;
    put_poststate_v3(
        poststates,
        2,
        maker_commitment_v3(
            input.accounts.buyer_maker_replay,
            input.programs.trading,
            input.direct.buyer.maker_replay,
            candidate.settlement.buyer_maker_root,
            candidate.settlement.buyer_creation,
            DirectInlinePoststateRoleV3::BuyerMakerReplay,
        )?,
    )
}

#[inline(never)]
fn project_claims_into_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
    poststates: &mut [Option<DirectInlinePoststateCommitmentV3>; DIRECT_INLINE_POSTSTATE_COUNT_V3],
    child_execution_digest: &mut Option<[u8; 32]>,
) -> Result<()> {
    if child_execution_digest.is_some() {
        return Err(DirectFinalizationErrorV3::Order);
    }
    let claims = project_claims_v3(input, candidate)?;
    put_poststate_v3(poststates, 3, claims.market)?;
    put_poststate_v3(poststates, 4, claims.seller)?;
    put_poststate_v3(poststates, 5, claims.buyer)?;
    *child_execution_digest = Some(claims.child_transcript);
    Ok(())
}

#[inline(never)]
fn project_tokens_into_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
    poststates: &mut [Option<DirectInlinePoststateCommitmentV3>; DIRECT_INLINE_POSTSTATE_COUNT_V3],
) -> Result<()> {
    let tokens = project_tokens_v3(input, candidate)?;
    put_poststate_v3(poststates, 7, tokens.buyer)?;
    put_poststate_v3(poststates, 8, tokens.seller)?;
    put_poststate_v3(poststates, 9, tokens.fee)
}

fn put_poststate_v3(
    poststates: &mut [Option<DirectInlinePoststateCommitmentV3>; DIRECT_INLINE_POSTSTATE_COUNT_V3],
    index: usize,
    commitment: DirectInlinePoststateCommitmentV3,
) -> Result<()> {
    let slot = poststates
        .get_mut(index)
        .ok_or(DirectFinalizationErrorV3::Order)?;
    if slot.is_some() || commitment.role != DirectInlinePoststateRoleV3::from_index(index)? {
        return Err(DirectFinalizationErrorV3::Order);
    }
    *slot = Some(commitment);
    Ok(())
}

fn validate_input_bindings(input: &DirectInlineFinalizationInputV3<'_>) -> Result<()> {
    require_nonzero(&[
        input.programs.trading,
        input.programs.claims,
        input.programs.custody,
        input.programs.token,
        input.product_id,
    ])?;
    if input.programs.trading != input.context.trading_program
        || input.programs.token != input.context.token_program
        || input.ack.release_set != input.context.release_set
        || input.ack.market != input.context.market
        || input.ack.generation != input.context.generation
        || input.ack.root != input.accounts.root.address
        || input.ack.request_digest != input.context.parent_request_digest
        || input.ack.root_prestate_digest != digest(input.accounts.root.data)
        || input.ack.artifacts.product_record != input.context.product_record_digest
        || input.ack.artifacts.linked_basis_record_digest
            != input.context.linked_basis_record_digest
        || input.ack.artifacts.semantic_basis_id != input.context.semantic_basis_id
        || input.ack.artifacts.outcome_count != input.context.outcome_count
        || digest(input.family_request) != input.context.parent_request_digest
    {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let accounts = input.accounts.ordered();
    for (index, account) in accounts.iter().enumerate() {
        if account.address == [0; 32] {
            return Err(DirectFinalizationErrorV3::ZeroIdentity);
        }
        if let Some((prior_index, prior)) = accounts
            .get(..index)
            .ok_or(DirectFinalizationErrorV3::Arithmetic)?
            .iter()
            .enumerate()
            .find(|(_, prior)| prior.address == account.address)
        {
            // The Direct collateral contract permits exactly one semantic
            // alias: seller and fee destinations may be the same token account
            // when their complete authenticated prestates are identical.
            if !((prior_index, index) == (8, 9) && *prior == *account) {
                return Err(DirectFinalizationErrorV3::Alias);
            }
        }
    }
    Ok(())
}

fn root_commitment_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
) -> Result<DirectInlinePoststateCommitmentV3> {
    let account = input.accounts.root;
    let expected_width = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    if account.owner != input.programs.trading || account.data.len() != expected_width {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let tail = account
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(DirectFinalizationErrorV3::Width)?;
    if crate::successor::DirectRootStateV1::decode(tail)
        .map_err(|_| DirectFinalizationErrorV3::Binding)?
        != input.direct.root
    {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let next = candidate.settlement.root.encode();
    commitment_v3(
        DirectInlinePoststateRoleV3::Root,
        account.address,
        account.owner,
        account.lamports,
        expected_width,
        digestv(&[
            account
                .data
                .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
                .ok_or(DirectFinalizationErrorV3::Width)?,
            &next,
        ]),
    )
}

fn maker_commitment_v3(
    account: DirectInlineAccountPrestateV3<'_>,
    trading_program: [u8; 32],
    observation: MakerReplayObservationV1,
    expected: MakerReplayRootV1,
    creation: Option<crate::successor::MakerReplayCreationPlanV1>,
    role: DirectInlinePoststateRoleV3,
) -> Result<DirectInlinePoststateCommitmentV3> {
    let (owner, lamports) = match (observation, creation) {
        (MakerReplayObservationV1::Existing(value), None) => {
            if account.owner != trading_program
                || account.data.len() != DIRECT_MAKER_REPLAY_BYTES_V1
                || MakerReplayRootV1::decode(account.data)
                    .map_err(|_| DirectFinalizationErrorV3::Binding)?
                    != value
            {
                return Err(DirectFinalizationErrorV3::Binding);
            }
            (account.owner, account.lamports)
        }
        (MakerReplayObservationV1::Vacant(_), Some(plan)) => {
            if account.owner != [0; 32]
                || !account.data.is_empty()
                || account.lamports != plan.observed_lamports
            {
                return Err(DirectFinalizationErrorV3::Binding);
            }
            (trading_program, plan.post_lamports)
        }
        _ => return Err(DirectFinalizationErrorV3::Binding),
    };
    let bytes = expected
        .encode()
        .map_err(|_| DirectFinalizationErrorV3::Candidate)?;
    commitment_v3(
        role,
        account.address,
        owner,
        lamports,
        bytes.len(),
        digest(&bytes),
    )
}

struct ClaimsProjectionV3 {
    market: DirectInlinePoststateCommitmentV3,
    seller: DirectInlinePoststateCommitmentV3,
    buyer: DirectInlinePoststateCommitmentV3,
    child_transcript: [u8; 32],
}

fn project_claims_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
) -> Result<ClaimsProjectionV3> {
    let market_account = input.accounts.claims_market;
    let seller_account = input.accounts.seller_position;
    let buyer_account = input.accounts.buyer_position;
    if market_account.owner != input.programs.claims
        || seller_account.owner != input.programs.claims
        || buyer_account.owner != input.programs.claims
    {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let market = LiabilityBasisMarketViewV2::decode(market_account.data)
        .map_err(|_| DirectFinalizationErrorV3::Claims)?;
    let seller = LiabilityBasisPositionViewV2::decode(seller_account.data)
        .map_err(|_| DirectFinalizationErrorV3::Claims)?;
    let buyer = LiabilityBasisPositionViewV2::decode(buyer_account.data)
        .map_err(|_| DirectFinalizationErrorV3::Claims)?;
    let seller_maker = input.direct.seller.authenticated.maker();
    let buyer_maker = input.direct.buyer.authenticated.maker();
    if market.claim_count != input.context.outcome_count
        || market.revision != input.context.claims_market_revision
        || market.logical_market != input.context.market
        || market.release_set != input.context.release_set
        || market.product_instance_id != input.product_id
        || market.basis_id != input.context.semantic_basis_id
        || market.generation != input.context.generation
        || seller.claim_count != input.context.outcome_count
        || buyer.claim_count != input.context.outcome_count
        || seller.revision != input.context.seller_position_revision
        || buyer.revision != input.context.buyer_position_revision
        || seller.market_account != market_account.address
        || buyer.market_account != market_account.address
        || seller.owner != seller_maker
        || buyer.owner != buyer_maker
        || seller.basis_id != input.context.semantic_basis_id
        || buyer.basis_id != input.context.semantic_basis_id
    {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let outcome = input.direct.seller.authenticated.intent().outcome;
    let seller_balance = seller
        .balance(seller_account.data, outcome)
        .map_err(|_| DirectFinalizationErrorV3::Claims)?;
    let buyer_balance = buyer
        .balance(buyer_account.data, outcome)
        .map_err(|_| DirectFinalizationErrorV3::Claims)?;
    seller_balance
        .checked_sub(input.direct.execution.fill)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    buyer_balance
        .checked_add(input.direct.execution.fill)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let market_digest = projected_claims_digest_v3(
        market_account.data,
        LiabilityBasisMarketLayoutV2::REVISION,
        candidate.claims_market_revision_after,
        None,
    )?;
    let seller_digest = projected_claims_digest_v3(
        seller_account.data,
        LiabilityBasisPositionLayoutV2::REVISION,
        candidate.seller_position_revision_after,
        Some((
            LiabilityBasisPositionLayoutV2::BALANCES,
            outcome,
            false,
            input.direct.execution.fill,
        )),
    )?;
    let buyer_digest = projected_claims_digest_v3(
        buyer_account.data,
        LiabilityBasisPositionLayoutV2::REVISION,
        candidate.buyer_position_revision_after,
        Some((
            LiabilityBasisPositionLayoutV2::BALANCES,
            outcome,
            true,
            input.direct.execution.fill,
        )),
    )?;
    let post_resource_digest = projected_claims_resource_digest_v3(
        market_account.data,
        seller_account.data,
        buyer_account.data,
        outcome,
        input.direct.execution.fill,
        candidate,
    )?;
    let request_bytes = input
        .request_bank
        .get(..SPARSE_NATIVE_TRANSFER_BYTES_V1)
        .ok_or(DirectFinalizationErrorV3::Width)?;
    let request = SparseNativeTransferV1::decode(request_bytes)
        .map_err(|_| DirectFinalizationErrorV3::Claims)?;
    let receipt = SparseNativeTransferReceiptV1::new(
        request,
        digest(request_bytes),
        input.programs.claims,
        post_resource_digest,
        candidate.claims_market_revision_after,
        candidate.seller_position_revision_after,
        candidate.buyer_position_revision_after,
    )
    .map_err(|_| DirectFinalizationErrorV3::Claims)?;
    let receipt_bytes = receipt.to_bytes();
    let receipt_digest = digest(&receipt_bytes);
    let child_digest = digestv(&[HOT_CHILD_EXECUTION_DIGEST_DOMAIN_V3, &receipt_bytes]);
    let initial = digestv(&[
        HOT_CHILD_EXECUTION_DIGEST_DOMAIN_V3,
        &input.context.parent_request_digest,
    ]);
    let child_transcript = transcript_step_v3(
        initial,
        1,
        0,
        0,
        input.programs.claims,
        receipt_digest,
        child_digest,
    );
    Ok(ClaimsProjectionV3 {
        market: commitment_v3(
            DirectInlinePoststateRoleV3::ClaimsMarket,
            market_account.address,
            market_account.owner,
            market_account.lamports,
            market_account.data.len(),
            market_digest,
        )?,
        seller: commitment_v3(
            DirectInlinePoststateRoleV3::SellerPosition,
            seller_account.address,
            seller_account.owner,
            seller_account.lamports,
            seller_account.data.len(),
            seller_digest,
        )?,
        buyer: commitment_v3(
            DirectInlinePoststateRoleV3::BuyerPosition,
            buyer_account.address,
            buyer_account.owner,
            buyer_account.lamports,
            buyer_account.data.len(),
            buyer_digest,
        )?,
        child_transcript,
    })
}

struct CustodyProjectionStateV3 {
    replay: CustodyReplayV1,
    source: u64,
    delegated: u64,
    seller: u64,
    fee: u64,
    child_transcript: [u8; 32],
}

#[inline(never)]
fn project_custody_into_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
    poststates: &mut [Option<DirectInlinePoststateCommitmentV3>; DIRECT_INLINE_POSTSTATE_COUNT_V3],
    child_execution_digest: &mut Option<[u8; 32]>,
    replay_output: Option<&mut [u8; CUSTODY_REPLAY_BYTES_V1]>,
) -> Result<()> {
    let account = input.accounts.custody_replay;
    if account.owner != input.programs.custody || account.data.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let replay =
        CustodyReplayV1::decode(account.data).map_err(|_| DirectFinalizationErrorV3::Custody)?;
    if replay.next_revision != input.context.custody_revision {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let child_transcript = child_execution_digest
        .take()
        .ok_or(DirectFinalizationErrorV3::Order)?;
    let seller = input.collateral.seller_destination.balance;
    let fee = if input.collateral.seller_destination.account
        == input.collateral.fee_destination.account
    {
        seller
    } else {
        input.collateral.fee_destination.balance
    };
    let count = usize::from(input.dispatch.custody_count);
    if count > DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2 {
        return Err(DirectFinalizationErrorV3::Width);
    }
    let mut state = CustodyProjectionStateV3 {
        replay,
        source: input.collateral.buyer_source.balance,
        delegated: input.collateral.buyer_source.delegated_amount,
        seller,
        fee,
        child_transcript,
    };
    let mut index = 0_usize;
    while index < count {
        let slot = usize::from(
            *input
                .dispatch
                .custody_slots
                .get(index)
                .ok_or(DirectFinalizationErrorV3::Order)?,
        );
        project_custody_step_v3(input, &mut state, slot)?;
        index = index
            .checked_add(1)
            .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    }
    if input.dispatch.custody_count != candidate.custody_count
        || state.replay.next_revision != candidate.custody_revision_after
        || state.source != candidate.buyer_source_after
        || state.delegated != candidate.buyer_delegated_after
        || state.seller != candidate.seller_destination_after
        || state.fee != candidate.fee_destination_after
    {
        return Err(DirectFinalizationErrorV3::Candidate);
    }
    let replay_bytes = state
        .replay
        .to_bytes()
        .map_err(|_| DirectFinalizationErrorV3::Custody)?;
    put_poststate_v3(
        poststates,
        6,
        commitment_v3(
            DirectInlinePoststateRoleV3::CustodyReplay,
            account.address,
            account.owner,
            account.lamports,
            replay_bytes.len(),
            digest(&replay_bytes),
        )?,
    )?;
    if let Some(output) = replay_output {
        output.copy_from_slice(&replay_bytes);
    }
    *child_execution_digest = Some(state.child_transcript);
    Ok(())
}

#[inline(never)]
fn project_custody_step_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    state: &mut CustodyProjectionStateV3,
    slot: usize,
) -> Result<()> {
    let request_offset = slot
        .checked_mul(DELEGATED_CUSTODY_REQUEST_BYTES_V2)
        .and_then(|offset| SPARSE_NATIVE_TRANSFER_BYTES_V1.checked_add(offset))
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let request_end = request_offset
        .checked_add(DELEGATED_CUSTODY_REQUEST_BYTES_V2)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let request_bytes = input
        .request_bank
        .get(request_offset..request_end)
        .ok_or(DirectFinalizationErrorV3::Width)?;
    let request = DelegatedCustodyRequestV2::decode(request_bytes)
        .map_err(|_| DirectFinalizationErrorV3::Custody)?;
    let request_digest = digest(request_bytes);
    let source_after = state
        .source
        .checked_sub(request.custody.amount)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let destination_before =
        if request.custody.destination == input.collateral.seller_destination.account {
            state.seller
        } else if request.custody.destination == input.collateral.fee_destination.account {
            state.fee
        } else {
            return Err(DirectFinalizationErrorV3::Binding);
        };
    let destination_after = destination_before
        .checked_add(request.custody.amount)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    if request.custody.source != input.collateral.buyer_source.account
        || request.delegate_before != input.collateral.buyer_source.delegate
        || request.allowance_before != state.delegated
    {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let poststate_preimage = DelegatedCustodyPoststateFactsV2 {
        request_digest,
        source: request.custody.source,
        destination: request.custody.destination,
        source_before: state.source,
        source_after,
        destination_before,
        destination_after,
        delegate_before: request.delegate_before,
        allowance_before: request.allowance_before,
        delegate_after: request.delegate_after,
        allowance_after: request.allowance_after,
    }
    .to_bytes()
    .map_err(|_| DirectFinalizationErrorV3::Custody)?;
    let poststate_commitment = digest(&poststate_preimage);
    state.replay = state
        .replay
        .advance(request.custody, request_digest, poststate_commitment)
        .map_err(|_| DirectFinalizationErrorV3::Custody)?;
    let replay_bytes = state
        .replay
        .to_bytes()
        .map_err(|_| DirectFinalizationErrorV3::Custody)?;
    let receipt = DelegatedCustodyReceiptV2::new(
        request,
        request_digest,
        ReceiptEvidenceV1 {
            source_before: state.source,
            source_after,
            destination_before,
            destination_after,
            poststate_commitment,
            replay_state_digest: digest(&replay_bytes),
        },
        DelegatedAllowanceObservationV2 {
            delegate_before: request.delegate_before,
            allowance_before: request.allowance_before,
            delegate_after: request.delegate_after,
            allowance_after: request.allowance_after,
        },
    )
    .map_err(|_| DirectFinalizationErrorV3::Custody)?;
    let receipt_bytes = receipt
        .encode()
        .map_err(|_| DirectFinalizationErrorV3::Custody)?;
    if receipt_bytes.len() != DELEGATED_CUSTODY_RECEIPT_BYTES_V2 {
        return Err(DirectFinalizationErrorV3::Width);
    }
    let route = u16::try_from(
        slot.checked_add(1)
            .ok_or(DirectFinalizationErrorV3::Arithmetic)?,
    )
    .map_err(|_| DirectFinalizationErrorV3::Arithmetic)?;
    let child_digest =
        delegated_custody_child_execution_digest_v3(route, 0, request_digest, &receipt_bytes);
    state.child_transcript = transcript_step_v3(
        state.child_transcript,
        4,
        route,
        0,
        input.programs.custody,
        digest(&receipt_bytes),
        child_digest,
    );
    state.source = source_after;
    state.delegated = request.allowance_after;
    if request.custody.destination == input.collateral.seller_destination.account {
        state.seller = destination_after;
    }
    if request.custody.destination == input.collateral.fee_destination.account {
        state.fee = destination_after;
    }
    Ok(())
}

struct TokenProjectionV3 {
    buyer: DirectInlinePoststateCommitmentV3,
    seller: DirectInlinePoststateCommitmentV3,
    fee: DirectInlinePoststateCommitmentV3,
}

fn project_tokens_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
) -> Result<TokenProjectionV3> {
    let buyer_account = input.accounts.buyer_token;
    let seller_account = input.accounts.seller_token;
    let fee_account = input.accounts.fee_token;
    for account in [buyer_account, seller_account, fee_account] {
        if account.owner != input.programs.token || account.data.len() != ACCOUNT_BYTES {
            return Err(DirectFinalizationErrorV3::Binding);
        }
    }
    let buyer =
        TokenAccount::parse(buyer_account.data).map_err(|_| DirectFinalizationErrorV3::Token)?;
    let seller =
        TokenAccount::parse(seller_account.data).map_err(|_| DirectFinalizationErrorV3::Token)?;
    let fee =
        TokenAccount::parse(fee_account.data).map_err(|_| DirectFinalizationErrorV3::Token)?;
    if buyer.state != AccountState::Initialized
        || seller.state != AccountState::Initialized
        || fee.state != AccountState::Initialized
        || !buyer.native_reserve.is_none()
        || !seller.native_reserve.is_none()
        || !fee.native_reserve.is_none()
        || buyer.mint != input.context.mint
        || seller.mint != input.context.mint
        || fee.mint != input.context.mint
        || buyer_account.address != input.collateral.buyer_source.account
        || seller_account.address != input.collateral.seller_destination.account
        || fee_account.address != input.collateral.fee_destination.account
        || buyer.owner != input.collateral.buyer_source.owner
        || seller.owner != input.collateral.seller_destination.owner
        || fee.owner != input.collateral.fee_destination.owner
        || buyer.amount != input.collateral.buyer_source.balance
        || seller.amount != input.collateral.seller_destination.balance
        || fee.amount != input.collateral.fee_destination.balance
        || buyer.delegate != COption::Some(input.collateral.buyer_source.delegate)
        || buyer.delegated_amount != input.collateral.buyer_source.delegated_amount
    {
        return Err(DirectFinalizationErrorV3::Binding);
    }
    let buyer_bytes = buyer_token_poststate_v3(input, candidate)?;
    let seller_bytes = TokenAccount::project_amount_poststate(
        seller_account.data,
        candidate.seller_destination_after,
    )
    .map_err(|_| DirectFinalizationErrorV3::Token)?;
    let fee_bytes =
        TokenAccount::project_amount_poststate(fee_account.data, candidate.fee_destination_after)
            .map_err(|_| DirectFinalizationErrorV3::Token)?;
    Ok(TokenProjectionV3 {
        buyer: commitment_v3(
            DirectInlinePoststateRoleV3::BuyerToken,
            buyer_account.address,
            buyer_account.owner,
            buyer_account.lamports,
            buyer_bytes.len(),
            digest(&buyer_bytes),
        )?,
        seller: commitment_v3(
            DirectInlinePoststateRoleV3::SellerToken,
            seller_account.address,
            seller_account.owner,
            seller_account.lamports,
            seller_bytes.len(),
            digest(&seller_bytes),
        )?,
        fee: commitment_v3(
            DirectInlinePoststateRoleV3::FeeToken,
            fee_account.address,
            fee_account.owner,
            fee_account.lamports,
            fee_bytes.len(),
            digest(&fee_bytes),
        )?,
    })
}

/// The buyer's collateral account after the fill, delegation included.
///
/// **A delegation is revoked by exhaustion, not by a transfer happening.** This
/// read `custody_count == 0` for "the delegation survives" and `COption::None`
/// for every other case, which was true while the only fill shape was one that
/// spent the whole allowance: `SellerTerminal` moves the gross and closes the
/// delegation in the same leg. It stopped being true the moment the fee leg
/// left the transaction (`docs/design/FEE_SECOND_TRANSACTION_V1.md`). A
/// fee-bearing fill now runs `SellerIntermediate`, which is NON-terminal by
/// construction -- `delegate_after == delegate_before`, `allowance_after ==
/// combined_fee` -- precisely so the second transaction has an allowance to
/// spend. So the projection said the account had no delegate while the chain
/// correctly left one standing, and the fill refused at
/// `TradingSbfError::Commit` on the buyer token's poststate digest.
///
/// The condition is now the one `DelegatedCustodyRequestV2::validate` itself
/// enforces: `terminal == (allowance_after == 0)`, and `terminal` is the only
/// thing that zeroes the delegate. A fill that dispatches no transfer at all
/// leaves the observed delegation exactly as it found it.
fn buyer_token_poststate_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    candidate: &DirectInlineCandidateV2,
) -> Result<[u8; ACCOUNT_BYTES]> {
    let delegate_after = if candidate.custody_count == 0 || candidate.buyer_delegated_after != 0 {
        COption::Some(input.collateral.buyer_source.delegate)
    } else {
        COption::None
    };
    TokenAccount::project_delegated_source_poststate(
        input.accounts.buyer_token.data,
        candidate.buyer_source_after,
        delegate_after,
        candidate.buyer_delegated_after,
    )
    .map_err(|_| DirectFinalizationErrorV3::Token)
}

fn projected_claims_digest_v3(
    data: &[u8],
    revision_offset: usize,
    revision_after: u64,
    selected: Option<(usize, u32, bool, u64)>,
) -> Result<[u8; 32]> {
    let revision_end = revision_offset
        .checked_add(8)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let prefix = data
        .get(..revision_offset)
        .ok_or(DirectFinalizationErrorV3::Width)?;
    let revision = revision_after.to_le_bytes();
    match selected {
        None => Ok(digestv(&[
            prefix,
            &revision,
            data.get(revision_end..)
                .ok_or(DirectFinalizationErrorV3::Width)?,
        ])),
        Some((base, outcome, add, quantity)) => {
            let selected_offset = indexed_u64_offset_v3(base, outcome)?;
            let selected_end = selected_offset
                .checked_add(8)
                .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
            let before = read_u64_v3(data, selected_offset)?;
            let after = if add {
                before.checked_add(quantity)
            } else {
                before.checked_sub(quantity)
            }
            .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
            Ok(digestv(&[
                prefix,
                &revision,
                data.get(revision_end..selected_offset)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
                &after.to_le_bytes(),
                data.get(selected_end..)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
            ]))
        }
    }
}

fn projected_claims_resource_digest_v3(
    market: &[u8],
    seller: &[u8],
    buyer: &[u8],
    outcome: u32,
    quantity: u64,
    candidate: &DirectInlineCandidateV2,
) -> Result<[u8; 32]> {
    let seller_value = read_u64_v3(
        seller,
        indexed_u64_offset_v3(LiabilityBasisPositionLayoutV2::BALANCES, outcome)?,
    )?
    .checked_sub(quantity)
    .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let buyer_value = read_u64_v3(
        buyer,
        indexed_u64_offset_v3(LiabilityBasisPositionLayoutV2::BALANCES, outcome)?,
    )?
    .checked_add(quantity)
    .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let market_revision_end = LiabilityBasisMarketLayoutV2::REVISION
        .checked_add(8)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let seller_revision_end = LiabilityBasisPositionLayoutV2::REVISION
        .checked_add(8)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let seller_value_offset =
        indexed_u64_offset_v3(LiabilityBasisPositionLayoutV2::BALANCES, outcome)?;
    let seller_value_end = seller_value_offset
        .checked_add(8)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let buyer_value_offset = seller_value_offset;
    let buyer_value_end = seller_value_end;
    let empty = &[][..];
    Ok(sparse_native_transfer_poststate_digest_v1(
        SparseNativeTransferPoststateSlicesV1 {
            market: [
                market
                    .get(..LiabilityBasisMarketLayoutV2::REVISION)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
                &candidate.claims_market_revision_after.to_le_bytes(),
                market
                    .get(market_revision_end..)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
                empty,
                empty,
            ],
            source: [
                seller
                    .get(..LiabilityBasisPositionLayoutV2::REVISION)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
                &candidate.seller_position_revision_after.to_le_bytes(),
                seller
                    .get(seller_revision_end..seller_value_offset)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
                &seller_value.to_le_bytes(),
                seller
                    .get(seller_value_end..)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
            ],
            destination: [
                buyer
                    .get(..LiabilityBasisPositionLayoutV2::REVISION)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
                &candidate.buyer_position_revision_after.to_le_bytes(),
                buyer
                    .get(seller_revision_end..buyer_value_offset)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
                &buyer_value.to_le_bytes(),
                buyer
                    .get(buyer_value_end..)
                    .ok_or(DirectFinalizationErrorV3::Width)?,
            ],
        },
    ))
}

fn project_claims_account_bytes_v3(
    input: &[u8],
    revision_offset: usize,
    revision_after: u64,
    selected: Option<(usize, u32, bool, u64)>,
    output: &mut [u8],
) -> Result<()> {
    copy_exact(output, input)?;
    write_u64_v3(output, revision_offset, revision_after)?;
    if let Some((base, outcome, add, quantity)) = selected {
        let offset = indexed_u64_offset_v3(base, outcome)?;
        let before = read_u64_v3(input, offset)?;
        let after = if add {
            before.checked_add(quantity)
        } else {
            before.checked_sub(quantity)
        }
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
        write_u64_v3(output, offset, after)?;
    }
    Ok(())
}

fn transcript_step_v3(
    prior: [u8; 32],
    role: u8,
    route: u16,
    invocation: u32,
    producer: [u8; 32],
    receipt_digest: [u8; 32],
    child_digest: [u8; 32],
) -> [u8; 32] {
    digestv(&[
        HOT_CHILD_EXECUTION_DIGEST_DOMAIN_V3,
        &prior,
        &[role],
        &route.to_le_bytes(),
        &invocation.to_le_bytes(),
        &producer,
        &receipt_digest,
        &child_digest,
    ])
}

#[cfg(test)]
fn validate_commitment_order_v3(
    commitments: &[DirectInlinePoststateCommitmentV3; DIRECT_INLINE_POSTSTATE_COUNT_V3],
) -> Result<()> {
    for (index, commitment) in commitments.iter().enumerate() {
        if commitment.role != DirectInlinePoststateRoleV3::from_index(index)? {
            return Err(DirectFinalizationErrorV3::Order);
        }
    }
    Ok(())
}

fn validate_optional_commitment_order_v3(
    commitments: &[Option<DirectInlinePoststateCommitmentV3>; DIRECT_INLINE_POSTSTATE_COUNT_V3],
) -> Result<()> {
    for (index, commitment) in commitments.iter().enumerate() {
        if commitment
            .as_ref()
            .ok_or(DirectFinalizationErrorV3::Order)?
            .role
            != DirectInlinePoststateRoleV3::from_index(index)?
        {
            return Err(DirectFinalizationErrorV3::Order);
        }
    }
    Ok(())
}

fn commitment_v3(
    role: DirectInlinePoststateRoleV3,
    address: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data_len: usize,
    data_digest: [u8; 32],
) -> Result<DirectInlinePoststateCommitmentV3> {
    require_nonzero(&[address, owner, data_digest])?;
    Ok(DirectInlinePoststateCommitmentV3 {
        role,
        address,
        owner,
        lamports,
        data_len: u32::try_from(data_len).map_err(|_| DirectFinalizationErrorV3::Arithmetic)?,
        data_digest,
    })
}

fn require_nonzero(values: &[[u8; 32]]) -> Result<()> {
    if values.contains(&[0; 32]) {
        Err(DirectFinalizationErrorV3::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn indexed_u64_offset_v3(base: usize, index: u32) -> Result<usize> {
    usize::try_from(index)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| base.checked_add(value))
        .ok_or(DirectFinalizationErrorV3::Arithmetic)
}

fn read_u64_v3(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    let array: [u8; 8] = bytes
        .get(offset..end)
        .ok_or(DirectFinalizationErrorV3::Width)?
        .try_into()
        .map_err(|_| DirectFinalizationErrorV3::Width)?;
    Ok(u64::from_le_bytes(array))
}

fn write_u64_v3(bytes: &mut [u8], offset: usize, value: u64) -> Result<()> {
    let end = offset
        .checked_add(8)
        .ok_or(DirectFinalizationErrorV3::Arithmetic)?;
    bytes
        .get_mut(offset..end)
        .ok_or(DirectFinalizationErrorV3::Width)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn copy_exact(output: &mut [u8], input: &[u8]) -> Result<()> {
    if output.len() != input.len() {
        return Err(DirectFinalizationErrorV3::Width);
    }
    output.copy_from_slice(input);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use dclutch_claims::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
        encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
    };
    use dclutch_custody::CallerRoleV1;
    use dclutch_custody::token_svm::state::TokenAccountLayoutV1;

    use crate::{
        inline_candidate_v2::{
            DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3, encode_inline_claims_request_v2,
            prepare_inline_ordinary_candidate_v2, project_inline_custody_effect_v2,
        },
        intent_v2::CompactIntentV2,
        successor::{
            AuthenticatedCompactIntentV2, DirectExecutionConfigV1, DirectRootStateV1,
            InlineExecutionV2, InlineParticipantV2, MakerReplayFirstUseV1, MakerReplayVacancyV1,
        },
    };

    const OUTCOMES: usize = 3;
    const MARKET_BYTES: usize = LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 8 * OUTCOMES;
    const POSITION_BYTES: usize = LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8 * OUTCOMES;
    const ROOT_BYTES: usize = CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    struct Fixture {
        direct: InlineOrdinaryInputV2,
        context: DirectInlineCandidateContextV2,
        collateral: DirectInlineCollateralFrameV2,
        dispatch: DirectInlineEffectDispatchV2,
        request_bank: [u8; DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3],
        family_request: [u8; 5],
        root: [u8; ROOT_BYTES],
        claims_market: [u8; MARKET_BYTES],
        seller_position: [u8; POSITION_BYTES],
        buyer_position: [u8; POSITION_BYTES],
        custody_replay: [u8; CUSTODY_REPLAY_BYTES_V1],
        buyer_token: [u8; ACCOUNT_BYTES],
        seller_token: [u8; ACCOUNT_BYTES],
        fee_token: [u8; ACCOUNT_BYTES],
        ack: HotExecutionAckInputV3,
    }

    impl Fixture {
        fn new() -> Self {
            let family_request = [90, 91, 92, 93, 94];
            let trading = id(40);
            let claims = id(41);
            let custody = id(42);
            let token = id(43);
            let market = id(1);
            let release_set = id(2);
            let product_id = id(3);
            let product_record = id(4);
            let basis = id(5);
            let linked_basis = id(6);
            let realm = id(7);
            let mint = id(8);
            let seller = id(10);
            let buyer = id(11);
            let seller_token_address = id(12);
            let buyer_token_address = id(13);
            let fee_token_address = id(14);
            let fee_recipient = id(15);
            let buyer_maker_address = id(17);
            let custody_authority = id(18);
            let claims_market_address = id(19);
            let root_address = id(23);
            let request_digest = digest(&family_request);
            let config =
                DirectExecutionConfigV1::new(100, 500, fee_recipient).expect("valid config");
            let participant = |maker, side, collateral_account, bump| InlineParticipantV2 {
                authenticated: AuthenticatedCompactIntentV2::from_adjacent_ed25519(
                    maker,
                    CompactIntentV2 {
                        side,
                        outcome: 1,
                        lifecycle: 0,
                        market,
                        generation: 9,
                        nonce: 0,
                        valid_from: 1,
                        valid_through: 20,
                        maximum_fill: 100,
                        limit_price: if side == 0 { 40 } else { 60 },
                        fee_basis_points: 500,
                        collateral_account,
                    },
                )
                .expect("authenticated fixture intent"),
                maker_replay: MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(bump, 3)),
                first_use: Some(MakerReplayFirstUseV1 {
                    rent_owner: id(bump),
                    rent_principal: 100,
                }),
            };
            let direct = InlineOrdinaryInputV2 {
                root: DirectRootStateV1::new(),
                seller: participant(seller, 0, seller_token_address, 24),
                buyer: participant(buyer, 1, buyer_token_address, 25),
                execution: InlineExecutionV2 {
                    config,
                    outcome_count: 3,
                    slot: 5,
                    fill: 100,
                    execution_price: 50,
                },
            };
            let context = DirectInlineCandidateContextV2 {
                release_set,
                market,
                generation: 9,
                outcome_count: 3,
                product_record_digest: product_record,
                semantic_basis_id: basis,
                linked_basis_record_digest: linked_basis,
                trading_program: trading,
                realm,
                mint,
                token_program: token,
                buyer_maker_root: buyer_maker_address,
                custody_authority,
                parent_request_digest: request_digest,
                claims_market_revision: 5,
                seller_position_revision: 6,
                buyer_position_revision: 7,
                custody_revision: 8,
            };
            let collateral = DirectInlineCollateralFrameV2 {
                buyer_source: crate::inline_candidate_v2::DirectExternalDebitV2 {
                    account: buyer_token_address,
                    owner: buyer,
                    delegate: custody_authority,
                    delegated_amount: 52,
                    balance: 1_000,
                },
                seller_destination: crate::inline_candidate_v2::DirectExternalCollateralV2 {
                    account: seller_token_address,
                    owner: seller,
                    balance: 20,
                },
                fee_destination: crate::inline_candidate_v2::DirectExternalCollateralV2 {
                    account: fee_token_address,
                    owner: fee_recipient,
                    balance: 30,
                },
            };
            let candidate = prepare_inline_ordinary_candidate_v2(direct, context, collateral)
                .expect("candidate");
            // One Custody transfer in tx1: the non-terminal seller leg. The
            // fee leg is a second transaction and slot 3 is retired.
            assert_eq!(candidate.custody_count, 1);
            let dispatch = DirectInlineEffectDispatchV2 {
                custody_slots: [1],
                custody_count: 1,
                child_dispatch_writable: [false, true, false, false],
            };
            let mut request_bank = [0; DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3];
            let claims_request =
                encode_inline_claims_request_v2(direct, context).expect("claims request");
            request_bank[..SPARSE_NATIVE_TRANSFER_BYTES_V1].copy_from_slice(&claims_request);
            for (index, slot) in (0_u8..).zip(dispatch.custody_slots.iter()) {
                let effect = project_inline_custody_effect_v2(
                    direct,
                    context,
                    collateral,
                    candidate.settlement,
                    index,
                )
                .expect("custody request");
                let bytes = effect.request.encode().expect("custody bytes");
                let start = SPARSE_NATIVE_TRANSFER_BYTES_V1
                    + usize::from(*slot) * DELEGATED_CUSTODY_REQUEST_BYTES_V2;
                request_bank
                    .get_mut(start..start + DELEGATED_CUSTODY_REQUEST_BYTES_V2)
                    .expect("fixture custody request slice")
                    .copy_from_slice(&bytes);
            }
            let mut root = [33; ROOT_BYTES];
            root[CAPABILITY_ROOT_HEADER_BYTES_V1..].copy_from_slice(&direct.root.encode());
            let mut claims_market = [0; MARKET_BYTES];
            encode_liability_basis_market_into_v2(
                LiabilityBasisMarketInputV2 {
                    revision: context.claims_market_revision,
                    logical_market: market,
                    release_set,
                    registry_program: id(26),
                    product_instance_id: product_id,
                    basis_id: basis,
                    realm_id: realm,
                    custody_context: id(27),
                    generation: context.generation,
                },
                &[0, 500, 0],
                &mut claims_market,
            )
            .expect("claims market");
            let mut seller_position = [0; POSITION_BYTES];
            encode_liability_basis_position_into_v2(
                LiabilityBasisPositionInputV2 {
                    revision: context.seller_position_revision,
                    market_account: claims_market_address,
                    owner: seller,
                    basis_id: basis,
                },
                &[0, 200, 0],
                &mut seller_position,
            )
            .expect("seller position");
            let mut buyer_position = [0; POSITION_BYTES];
            encode_liability_basis_position_into_v2(
                LiabilityBasisPositionInputV2 {
                    revision: context.buyer_position_revision,
                    market_account: claims_market_address,
                    owner: buyer,
                    basis_id: basis,
                },
                &[0, 10, 0],
                &mut buyer_position,
            )
            .expect("buyer position");
            let custody_replay = CustodyReplayV1 {
                caller_role: CallerRoleV1::Trading,
                release_set,
                market,
                realm,
                context: buyer_maker_address,
                caller_program: trading,
                rent_refund: id(28),
                open_vault_count: 0,
                next_revision: context.custody_revision,
                generation: context.generation,
                last_request_digest: id(29),
                last_poststate_commitment: id(30),
            }
            .to_bytes()
            .expect("custody replay");
            let buyer_token = TokenAccount::project_delegated_source_poststate(
                &TokenAccount::initialized_base_bytes(mint, buyer).expect("buyer base"),
                collateral.buyer_source.balance,
                COption::Some(custody_authority),
                collateral.buyer_source.delegated_amount,
            )
            .expect("buyer delegated token");
            let seller_token = TokenAccount::project_amount_poststate(
                &TokenAccount::initialized_base_bytes(mint, seller).expect("seller base"),
                collateral.seller_destination.balance,
            )
            .expect("seller token");
            let fee_token = TokenAccount::project_amount_poststate(
                &TokenAccount::initialized_base_bytes(mint, fee_recipient).expect("fee base"),
                collateral.fee_destination.balance,
            )
            .expect("fee token");
            let mut artifacts = ack_input().artifacts;
            artifacts.product_record = context.product_record_digest;
            artifacts.linked_basis_record_digest = context.linked_basis_record_digest;
            artifacts.semantic_basis_id = context.semantic_basis_id;
            artifacts.outcome_count = context.outcome_count;
            let ack = HotExecutionAckInputV3 {
                release_set: context.release_set,
                market: context.market,
                generation: context.generation,
                root: root_address,
                request_digest: context.parent_request_digest,
                root_prestate_digest: digest(&root),
                artifacts,
            };
            let fixture = Self {
                direct,
                context,
                collateral,
                dispatch,
                request_bank,
                family_request,
                root,
                claims_market,
                seller_position,
                buyer_position,
                custody_replay,
                buyer_token,
                seller_token,
                fee_token,
                ack,
            };
            // Keep every named address/program coordinate next to the input
            // method rather than silently relying on the repeated test IDs.
            assert_eq!(
                fixture.address(DirectInlinePoststateRoleV3::Root),
                root_address
            );
            assert_eq!(
                fixture.programs(),
                DirectInlineFinalizationProgramsV3 {
                    trading,
                    claims,
                    custody,
                    token,
                }
            );
            fixture
        }

        fn address(&self, role: DirectInlinePoststateRoleV3) -> [u8; 32] {
            match role {
                DirectInlinePoststateRoleV3::Root => id(23),
                DirectInlinePoststateRoleV3::SellerMakerReplay => id(16),
                DirectInlinePoststateRoleV3::BuyerMakerReplay => id(17),
                DirectInlinePoststateRoleV3::ClaimsMarket => id(19),
                DirectInlinePoststateRoleV3::SellerPosition => id(20),
                DirectInlinePoststateRoleV3::BuyerPosition => id(21),
                DirectInlinePoststateRoleV3::CustodyReplay => id(22),
                DirectInlinePoststateRoleV3::BuyerToken => id(13),
                DirectInlinePoststateRoleV3::SellerToken => id(12),
                DirectInlinePoststateRoleV3::FeeToken => id(14),
            }
        }

        fn programs(&self) -> DirectInlineFinalizationProgramsV3 {
            DirectInlineFinalizationProgramsV3 {
                trading: id(40),
                claims: id(41),
                custody: id(42),
                token: id(43),
            }
        }

        fn accounts(&self) -> DirectInlineAccountPrestatesV3<'_> {
            let account = |role, owner, lamports, data| DirectInlineAccountPrestateV3 {
                address: self.address(role),
                owner,
                lamports,
                data,
            };
            DirectInlineAccountPrestatesV3 {
                root: account(DirectInlinePoststateRoleV3::Root, id(40), 2_000, &self.root),
                seller_maker_replay: account(
                    DirectInlinePoststateRoleV3::SellerMakerReplay,
                    [0; 32],
                    3,
                    &[],
                ),
                buyer_maker_replay: account(
                    DirectInlinePoststateRoleV3::BuyerMakerReplay,
                    [0; 32],
                    3,
                    &[],
                ),
                claims_market: account(
                    DirectInlinePoststateRoleV3::ClaimsMarket,
                    id(41),
                    3_000,
                    &self.claims_market,
                ),
                seller_position: account(
                    DirectInlinePoststateRoleV3::SellerPosition,
                    id(41),
                    1_000,
                    &self.seller_position,
                ),
                buyer_position: account(
                    DirectInlinePoststateRoleV3::BuyerPosition,
                    id(41),
                    1_000,
                    &self.buyer_position,
                ),
                custody_replay: account(
                    DirectInlinePoststateRoleV3::CustodyReplay,
                    id(42),
                    2_000,
                    &self.custody_replay,
                ),
                buyer_token: account(
                    DirectInlinePoststateRoleV3::BuyerToken,
                    id(43),
                    2_039_280,
                    &self.buyer_token,
                ),
                seller_token: account(
                    DirectInlinePoststateRoleV3::SellerToken,
                    id(43),
                    2_039_280,
                    &self.seller_token,
                ),
                fee_token: account(
                    DirectInlinePoststateRoleV3::FeeToken,
                    id(43),
                    2_039_280,
                    &self.fee_token,
                ),
            }
        }

        fn input<'a>(
            &'a self,
            accounts: &'a DirectInlineAccountPrestatesV3<'a>,
        ) -> DirectInlineFinalizationInputV3<'a> {
            DirectInlineFinalizationInputV3 {
                direct: &self.direct,
                context: &self.context,
                product_id: id(3),
                collateral: &self.collateral,
                request_bank: &self.request_bank,
                dispatch: self.dispatch,
                family_request: &self.family_request,
                accounts,
                programs: self.programs(),
                ack: &self.ack,
            }
        }
    }

    fn ack_input() -> HotExecutionAckInputV3 {
        HotExecutionAckInputV3 {
            release_set: [1; 32],
            market: [2; 32],
            generation: 3,
            root: [4; 32],
            request_digest: [5; 32],
            root_prestate_digest: [6; 32],
            artifacts: HotExecutionArtifactFactsV3 {
                selected_program: [7; 32],
                account_profile_program: [8; 32],
                request_profile_program: [9; 32],
                strategy_program: [10; 32],
                strategy_transition_program: [11; 32],
                effect_program: [12; 32],
                derivation_policy: [13; 32],
                config: [14; 32],
                product_record: [15; 32],
                linked_basis_record_digest: [16; 32],
                semantic_basis_id: [17; 32],
                outcome_count: 2,
                strategy_execution_digest: [18; 32],
            },
        }
    }

    #[test]
    fn exact_ack_is_canonical_and_hostile_checked() {
        let input = ack_input();
        let ack = project_hot_execution_ack_v3(input, [19; 32], [20; 32]).expect("ack");
        assert_eq!(HotExecutionAckV3::decode(&ack.to_bytes()), Ok(ack));
        let mut zero = input;
        zero.artifacts.effect_program = [0; 32];
        assert_eq!(
            project_hot_execution_ack_v3(zero, [19; 32], [20; 32]),
            Err(DirectFinalizationErrorV3::ZeroIdentity)
        );
        let mut width = input;
        width.artifacts.outcome_count = 0;
        assert_eq!(
            project_hot_execution_ack_v3(width, [19; 32], [20; 32]),
            Err(DirectFinalizationErrorV3::Ack)
        );
        let mut admitted = input;
        admitted.artifacts.strategy_execution_digest = [0; 32];
        let admitted_ack = project_hot_execution_ack_v3(admitted, [19; 32], [20; 32])
            .expect("an admitted strategy may have the canonical zero transcript");
        assert_ne!(admitted_ack, ack);
    }

    #[test]
    fn commitment_roles_are_exact_and_ordered() {
        for index in 0..DIRECT_INLINE_POSTSTATE_COUNT_V3 {
            assert_eq!(
                DirectInlinePoststateRoleV3::from_index(index).map(|role| role as usize),
                Ok(index)
            );
        }
        assert_eq!(
            DirectInlinePoststateRoleV3::from_index(DIRECT_INLINE_POSTSTATE_COUNT_V3),
            Err(DirectFinalizationErrorV3::Order)
        );
    }

    #[test]
    fn complete_finalization_materializes_every_exact_poststate() {
        let fixture = Fixture::new();
        let accounts = fixture.accounts();
        let input = fixture.input(&accounts);
        let finalization =
            prepare_direct_inline_finalization_v3(&input).expect("complete finalization");
        assert_eq!(finalization.ack.to_bytes(), finalization.ack_bytes);
        assert_eq!(
            HotExecutionAckV3::decode(&finalization.ack_bytes),
            Ok(finalization.ack)
        );
        for (index, commitment) in finalization.poststates.iter().enumerate() {
            assert_eq!(
                commitment.role,
                DirectInlinePoststateRoleV3::from_index(index).expect("canonical role")
            );
            let mut output = std::vec![0; commitment.data_len as usize];
            project_direct_inline_account_poststate_v3(&input, commitment.role, &mut output)
                .expect("poststate bytes");
            assert_eq!(digest(&output), commitment.data_digest);
        }
    }

    #[test]
    fn alias_width_product_and_producer_substitution_refuse() {
        let fixture = Fixture::new();
        let mut alias_accounts = fixture.accounts();
        alias_accounts.buyer_position.address = alias_accounts.claims_market.address;
        let alias = fixture.input(&alias_accounts);
        assert_eq!(
            prepare_direct_inline_finalization_v3(&alias),
            Err(DirectFinalizationErrorV3::Alias)
        );

        let accounts = fixture.accounts();
        let input = fixture.input(&accounts);
        let mut short = [0; ROOT_BYTES - 1];
        assert_eq!(
            project_direct_inline_account_poststate_v3(
                &input,
                DirectInlinePoststateRoleV3::Root,
                &mut short,
            ),
            Err(DirectFinalizationErrorV3::Width)
        );

        let mut product = fixture.input(&accounts);
        product.product_id = id(99);
        assert_eq!(
            prepare_direct_inline_finalization_v3(&product),
            Err(DirectFinalizationErrorV3::Binding)
        );

        let mut producer = fixture.input(&accounts);
        producer.programs.claims = id(99);
        assert_eq!(
            prepare_direct_inline_finalization_v3(&producer),
            Err(DirectFinalizationErrorV3::Binding)
        );

        let mut request = fixture.input(&accounts);
        request.family_request = &[1, 2, 3];
        assert_eq!(
            prepare_direct_inline_finalization_v3(&request),
            Err(DirectFinalizationErrorV3::Binding)
        );
    }

    #[test]
    fn claims_overflow_and_commitment_reordering_refuse() {
        let mut fixture = Fixture::new();
        let balance = LiabilityBasisPositionLayoutV2::BALANCES + 8;
        fixture
            .buyer_position
            .get_mut(balance..balance + 8)
            .expect("fixture buyer balance")
            .copy_from_slice(&u64::MAX.to_le_bytes());
        let accounts = fixture.accounts();
        assert_eq!(
            prepare_direct_inline_finalization_v3(&fixture.input(&accounts)),
            Err(DirectFinalizationErrorV3::Arithmetic)
        );

        let fixture = Fixture::new();
        let accounts = fixture.accounts();
        let mut poststates = prepare_direct_inline_finalization_v3(&fixture.input(&accounts))
            .expect("finalization")
            .poststates;
        poststates.swap(3, 4);
        assert_eq!(
            validate_commitment_order_v3(&poststates),
            Err(DirectFinalizationErrorV3::Order)
        );
    }

    #[test]
    fn token_owner_state_amount_delegate_and_close_authority_are_exact() {
        let fixture = Fixture::new();
        let mut outer_owner_accounts = fixture.accounts();
        outer_owner_accounts.buyer_token.owner = id(99);
        let outer_owner = fixture.input(&outer_owner_accounts);
        assert_eq!(
            prepare_direct_inline_finalization_v3(&outer_owner),
            Err(DirectFinalizationErrorV3::Binding)
        );

        let mut state = Fixture::new();
        state.buyer_token[TokenAccountLayoutV1::STATE] = AccountState::Frozen as u8;
        let state_accounts = state.accounts();
        assert_eq!(
            prepare_direct_inline_finalization_v3(&state.input(&state_accounts)),
            Err(DirectFinalizationErrorV3::Binding)
        );

        let mut amount = Fixture::new();
        amount.buyer_token[TokenAccountLayoutV1::AMOUNT..TokenAccountLayoutV1::AMOUNT + 8]
            .copy_from_slice(&999_u64.to_le_bytes());
        let amount_accounts = amount.accounts();
        assert_eq!(
            prepare_direct_inline_finalization_v3(&amount.input(&amount_accounts)),
            Err(DirectFinalizationErrorV3::Binding)
        );

        let mut delegate = Fixture::new();
        delegate.buyer_token[TokenAccountLayoutV1::DELEGATE..TokenAccountLayoutV1::DELEGATE + 4]
            .copy_from_slice(&[0; 4]);
        let delegate_accounts = delegate.accounts();
        assert_eq!(
            prepare_direct_inline_finalization_v3(&delegate.input(&delegate_accounts)),
            Err(DirectFinalizationErrorV3::Binding)
        );

        let mut malformed_close = Fixture::new();
        malformed_close.buyer_token
            [TokenAccountLayoutV1::CLOSE_AUTHORITY..TokenAccountLayoutV1::CLOSE_AUTHORITY + 4]
            .copy_from_slice(&[2, 0, 0, 0]);
        let malformed_close_accounts = malformed_close.accounts();
        assert_eq!(
            prepare_direct_inline_finalization_v3(
                &malformed_close.input(&malformed_close_accounts),
            ),
            Err(DirectFinalizationErrorV3::Token)
        );

        let mut close = Fixture::new();
        close.buyer_token
            [TokenAccountLayoutV1::CLOSE_AUTHORITY..TokenAccountLayoutV1::CLOSE_AUTHORITY + 4]
            .copy_from_slice(&[1, 0, 0, 0]);
        close.buyer_token
            [TokenAccountLayoutV1::CLOSE_AUTHORITY + 4..TokenAccountLayoutV1::CLOSE_AUTHORITY + 36]
            .copy_from_slice(&id(77));
        let close_accounts = close.accounts();
        let input = close.input(&close_accounts);
        let finalization =
            prepare_direct_inline_finalization_v3(&input).expect("valid close authority persists");
        let commitment = finalization
            .poststates
            .get(DirectInlinePoststateRoleV3::BuyerToken as usize)
            .expect("buyer token commitment");
        let mut output = [0; ACCOUNT_BYTES];
        project_direct_inline_account_poststate_v3(
            &input,
            DirectInlinePoststateRoleV3::BuyerToken,
            &mut output,
        )
        .expect("buyer poststate");
        assert_eq!(
            &output
                [TokenAccountLayoutV1::CLOSE_AUTHORITY..TokenAccountLayoutV1::CLOSE_AUTHORITY + 36],
            &close.buyer_token
                [TokenAccountLayoutV1::CLOSE_AUTHORITY..TokenAccountLayoutV1::CLOSE_AUTHORITY + 36]
        );
        output[TokenAccountLayoutV1::CLOSE_AUTHORITY + 4] ^= 1;
        assert_ne!(digest(&output), commitment.data_digest);
    }
}
