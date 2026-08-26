//! Chain-derived Direct V3 inline execution construction.
//!
//! This host-only adapter joins the canonical action-selected Direct artifact
//! bundle, expands the authenticated AccountProfile account space, and emits
//! the adjacent native-Ed25519 plus Trading instruction pair. It never performs
//! RPC, signs maker material, signs a transaction, or submits one.

use crate::{Finality, Observation, ObservedAccount};
use dclutch_capability_program_contract::hot_v3::{
    HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
    HOT_MARKET_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_direct_codec::{
    artifacts_v3::{
        DirectArtifactBytesV3, DirectArtifactSelectionV3, authenticate_direct_artifacts_v3,
    },
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        DirectExecutionActionV3, DirectExecutionRequestV3, encode_header_v3,
    },
    intent_v2::{COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2, CompactIntentV2},
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{ed25519_program, sysvar};

use crate::versioned::{VersionedMessagePlanV0, compile_v0_message};

/// Exact Direct V3 InlineOrdinary family-request width.
pub const DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3: usize =
    DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 16;

const ED25519_DESCRIPTOR_BYTES: usize = 14;
const ED25519_SIGNATURES: usize = 2;
const ED25519_HEADER_BYTES: usize = 2 + ED25519_SIGNATURES * ED25519_DESCRIPTOR_BYTES;
const ED25519_PARTICIPANT_BYTES: usize = 32 + 64;
const CURRENT_HOT_INSTRUCTION_INDEX: u16 = 1;

/// One exact detached maker signature and its canonical signed intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDirectIntentV3 {
    /// Native Ed25519 maker public key.
    pub maker: Pubkey,
    /// Detached Ed25519 signature over `intent.signed_preimage()`.
    pub signature: [u8; 64],
    /// Exact runtime-width Direct V2 semantic intent.
    pub intent: CompactIntentV2,
}

/// One same-finalized account plus the privileges requested by the transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccountMetaV3 {
    /// Exact finalized account observation.
    pub account: ObservedAccount,
    /// Whether the transaction requests signer privilege.
    pub is_signer: bool,
    /// Whether the transaction requests writable privilege.
    pub is_writable: bool,
}

impl ObservedAccountMetaV3 {
    fn meta(&self) -> AccountMeta {
        AccountMeta {
            pubkey: self.account.key,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}

/// Checked-release evidence that the selected Trading artifact implements the
/// common V3 hot outer.
///
/// This value is not a hard-coded client constant. A chain/release checker must
/// construct it only after the selected immutable ArtifactRelease and current
/// Loader observations match a user-supplied checked manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedHotOuterReleaseV3 {
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact immutable Trading ArtifactRelease identity.
    pub artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized authority and exact physical account projection for one hot
/// Direct instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotStateV3 {
    /// Exact 30-account family-neutral prefix in canonical ABI order.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Exact disposition-selected ExecutionStrategy account suffix.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// Expanded AccountProfile physical address space, including the capability
    /// root at runtime coordinate zero. Coordinate zero is not appended twice.
    pub runtime_accounts: Vec<ObservedAccountMetaV3>,
    /// Immutable execution release-set content identity selected by Market.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Trusted Clock slot used for an exact economic preview.
    pub clock_slot: u64,
    /// Checked current hot outer, absent while the common entrypoint is not an
    /// accepted immutable release.
    pub hot_outer: Option<CheckedHotOuterReleaseV3>,
}

/// Exact economic preview derived from immutable Direct config and the request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineEconomicPreviewV3 {
    /// Claims transferred from seller to buyer.
    pub claim_transfer: u64,
    /// Exact gross collateral at the immutable price scale.
    pub gross_collateral: u64,
    /// Gross less the seller-side floor fee.
    pub seller_net_collateral_credit: u64,
    /// Gross plus the buyer-side floor fee.
    pub buyer_collateral_debit: u64,
    /// Sum of seller-withheld and buyer-added floor fees.
    pub total_fee_transfer: u64,
}

/// Complete unsigned adjacent-evidence execution material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotReportV3 {
    /// Native Ed25519 verification followed immediately by Trading.
    pub instructions: [Instruction; 2],
    /// Complete exact HotExecutionEnvelopeV3 plus Direct request bytes.
    pub hot_instruction_data: Vec<u8>,
    /// Same finalized observation selecting every physical account.
    pub observation: Observation,
    /// Action-selected CapabilityProgramV3 content digest.
    pub selected_program: [u8; 32],
    /// Exact economic preview; onchain execution remains authoritative.
    pub preview: DirectInlineEconomicPreviewV3,
}

/// Stable refusal from stale authority, malformed signatures, artifact joins,
/// account-profile expansion, or transaction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The current selected immutable Trading release is not checked as a V3 hot outer.
    HotOuterUnavailable,
    /// A required identity, maker, or signature was zero.
    ZeroIdentity,
    /// Account observations were not finalized at one exact snapshot.
    ObservationMismatch,
    /// The family-neutral fixed frame or selected program identity differed.
    FixedFrameMismatch,
    /// Action-selected finalized artifacts did not form one Direct bundle.
    ArtifactMismatch,
    /// Runtime AccountProfile width or privileges differed.
    RuntimeProfileMismatch,
    /// Intent, slot, price, fee, or quantity facts were incompatible.
    EconomicMismatch,
    /// Checked arithmetic or instruction encoding failed.
    Arithmetic,
}

/// Encode the sole canonical Direct V3 InlineOrdinary family request.
pub fn compile_direct_inline_request_v3(
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
) -> Result<[u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3], Error> {
    if seller.maker == Pubkey::default()
        || buyer.maker == Pubkey::default()
        || seller.maker == buyer.maker
        || seller.signature.iter().all(|byte| *byte == 0)
        || buyer.signature.iter().all(|byte| *byte == 0)
        || fill == 0
        || execution_price == 0
    {
        return Err(Error::ZeroIdentity);
    }
    let mut output = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3];
    let body = encode_header_v3(DirectExecutionActionV3::InlineOrdinary, &mut output)
        .map_err(|_| Error::Arithmetic)?;
    let seller_message = seller
        .intent
        .signed_preimage()
        .map_err(|_| Error::EconomicMismatch)?;
    let buyer_message = buyer
        .intent
        .signed_preimage()
        .map_err(|_| Error::EconomicMismatch)?;
    put(body, 0, seller.maker.as_ref())?;
    put(body, 32, &seller_message)?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        buyer.maker.as_ref(),
    )?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32,
        &buyer_message,
    )?;
    put(
        body,
        2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        &fill.to_le_bytes(),
    )?;
    put(
        body,
        2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 8,
        &execution_price.to_le_bytes(),
    )?;
    DirectExecutionRequestV3::decode(&output, u32::MAX).map_err(|_| Error::EconomicMismatch)?;
    Ok(output)
}

/// Build one complete chain-derived Direct inline batch without signing or submitting.
#[allow(clippy::too_many_arguments)]
pub fn build_direct_inline_hot_v3(
    state: &DirectInlineHotStateV3,
    artifact_selection: DirectArtifactSelectionV3,
    artifact_bytes: DirectArtifactBytesV3<'_>,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
) -> Result<DirectInlineHotReportV3, Error> {
    let checked = state.hot_outer.ok_or(Error::HotOuterUnavailable)?;
    if checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || state.release_set == [0; 32]
        || state.outcome_count == 0
    {
        return Err(Error::ZeroIdentity);
    }
    let observation = validate_frame(state, checked)?;
    let request = compile_direct_inline_request_v3(seller, buyer, fill, execution_price)?;
    let bundle = authenticate_direct_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &request,
        state.outcome_count,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    if bundle.action != DirectExecutionActionV3::InlineOrdinary
        || !bundle.request_profile.requires_native_signature()
    {
        return Err(Error::ArtifactMismatch);
    }
    validate_runtime_profile(state, bundle)?;
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?
        .account
        .key;
    let root = &state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?
        .account;
    let preview = preview_economics(
        market,
        state,
        bundle.config,
        seller,
        buyer,
        fill,
        execution_price,
    )?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).map_err(|_| Error::Arithmetic)?,
        state.release_set,
        market.to_bytes(),
        state.generation,
        hash(&root.data).to_bytes(),
    )
    .map_err(|_| Error::FixedFrameMismatch)?;
    let mut hot_instruction_data = Vec::with_capacity(HOT_FAMILY_REQUEST_OFFSET_V3 + request.len());
    hot_instruction_data.extend_from_slice(&envelope.to_bytes());
    hot_instruction_data.extend_from_slice(&request);

    let mut accounts = Vec::new();
    accounts.extend(state.fixed_accounts.iter().map(ObservedAccountMetaV3::meta));
    accounts.extend(
        state
            .strategy_accounts
            .iter()
            .map(ObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .runtime_accounts
            .iter()
            .skip(1)
            .map(ObservedAccountMetaV3::meta),
    );
    let trading = Instruction {
        program_id: checked.trading_program,
        accounts,
        data: hot_instruction_data.clone(),
    };
    let native = native_ed25519_instruction([seller, buyer])?;
    Ok(DirectInlineHotReportV3 {
        instructions: [native, trading],
        hot_instruction_data,
        observation,
        selected_program: hash(artifact_bytes.descriptor).to_bytes(),
        preview,
    })
}

/// Compile the exact adjacent pair into an unsigned packet-safe v0 message.
pub fn compile_direct_inline_hot_v0(
    report: &DirectInlineHotReportV3,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
) -> Result<VersionedMessagePlanV0, crate::versioned::Error> {
    compile_v0_message(
        payer,
        &report.instructions,
        recent_blockhash,
        report.observation,
        lookup_tables,
    )
}

fn validate_frame(
    state: &DirectInlineHotStateV3,
    checked: CheckedHotOuterReleaseV3,
) -> Result<Observation, Error> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3 {
        return Err(Error::FixedFrameMismatch);
    }
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let root = state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let runtime_root = state
        .runtime_accounts
        .first()
        .ok_or(Error::FixedFrameMismatch)?;
    let trading = state
        .fixed_accounts
        .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let rent = state
        .fixed_accounts
        .get(HOT_RENT_SYSVAR_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let instructions = state
        .fixed_accounts
        .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    if root.account.key != runtime_root.account.key
        || trading.account.key != checked.trading_program
        || !trading.account.executable
        || rent.account.key != sysvar::rent::ID
        || instructions.account.key != sysvar::instructions::ID
    {
        return Err(Error::FixedFrameMismatch);
    }
    let observation = market.account.observation;
    for value in state
        .fixed_accounts
        .iter()
        .chain(&state.strategy_accounts)
        .chain(&state.runtime_accounts)
    {
        if value.account.observation.finality != Finality::Finalized
            || value.account.observation != observation
        {
            return Err(Error::ObservationMismatch);
        }
    }
    Ok(observation)
}

fn validate_runtime_profile(
    state: &DirectInlineHotStateV3,
    bundle: dclutch_direct_codec::artifacts_v3::DirectArtifactBundleV3<'_>,
) -> Result<(), Error> {
    let profile = bundle.account_profile;
    let fixed = usize::from(profile.fixed_account_count());
    let stride = usize::from(profile.item_account_stride());
    let tail = usize::try_from(state.outcome_count).map_err(|_| Error::Arithmetic)?;
    let expected = stride
        .checked_mul(tail)
        .and_then(|value| fixed.checked_add(value))
        .ok_or(Error::Arithmetic)?;
    if state.runtime_accounts.len() != expected {
        return Err(Error::RuntimeProfileMismatch);
    }
    for (coordinate, account) in state.runtime_accounts.iter().enumerate() {
        let (item, index) = if coordinate < fixed {
            (false, coordinate)
        } else {
            if stride == 0 {
                return Err(Error::RuntimeProfileMismatch);
            }
            (true, (coordinate - fixed) % stride)
        };
        let rule = profile
            .rule(item, u16::try_from(index).map_err(|_| Error::Arithmetic)?)
            .map_err(|_| Error::RuntimeProfileMismatch)?;
        let privileges = rule.privileges();
        if account.is_signer != (privileges & 1 != 0)
            || account.is_writable != (privileges & 2 != 0)
            || account.account.executable != (privileges & 4 != 0)
        {
            return Err(Error::RuntimeProfileMismatch);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn preview_economics(
    market: Pubkey,
    state: &DirectInlineHotStateV3,
    config: dclutch_direct_codec::successor::DirectExecutionConfigV1,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
) -> Result<DirectInlineEconomicPreviewV3, Error> {
    for (participant, side) in [(seller, 0_u8), (buyer, 1_u8)] {
        let intent = participant.intent;
        if intent.side != side
            || intent.lifecycle > 1
            || intent.market != market.to_bytes()
            || intent.generation != state.generation
            || intent.outcome >= state.outcome_count
            || intent.maximum_fill < fill
            || intent.fee_basis_points != config.fee_basis_points()
            || state.clock_slot < intent.valid_from
            || state.clock_slot > intent.valid_through
        {
            return Err(Error::EconomicMismatch);
        }
        if intent.lifecycle == 0 && intent.maximum_fill != fill {
            return Err(Error::EconomicMismatch);
        }
    }
    if seller.intent.outcome != buyer.intent.outcome
        || execution_price < seller.intent.limit_price
        || execution_price > buyer.intent.limit_price
        || execution_price > config.price_scale()
    {
        return Err(Error::EconomicMismatch);
    }
    let scaled = u128::from(fill)
        .checked_mul(u128::from(execution_price))
        .ok_or(Error::Arithmetic)?;
    let scale = u128::from(config.price_scale());
    if scaled % scale != 0 {
        return Err(Error::EconomicMismatch);
    }
    let gross = u64::try_from(scaled / scale).map_err(|_| Error::Arithmetic)?;
    let fee = u64::try_from(
        u128::from(gross)
            .checked_mul(u128::from(config.fee_basis_points()))
            .ok_or(Error::Arithmetic)?
            / 10_000,
    )
    .map_err(|_| Error::Arithmetic)?;
    Ok(DirectInlineEconomicPreviewV3 {
        claim_transfer: fill,
        gross_collateral: gross,
        seller_net_collateral_credit: gross.checked_sub(fee).ok_or(Error::Arithmetic)?,
        buyer_collateral_debit: gross.checked_add(fee).ok_or(Error::Arithmetic)?,
        total_fee_transfer: fee.checked_mul(2).ok_or(Error::Arithmetic)?,
    })
}

fn native_ed25519_instruction(
    participants: [SignedDirectIntentV3; ED25519_SIGNATURES],
) -> Result<Instruction, Error> {
    let payload_bytes = ED25519_SIGNATURES
        .checked_mul(ED25519_PARTICIPANT_BYTES)
        .and_then(|value| ED25519_HEADER_BYTES.checked_add(value))
        .ok_or(Error::Arithmetic)?;
    let mut data = vec![0_u8; payload_bytes];
    *data.first_mut().ok_or(Error::Arithmetic)? =
        u8::try_from(ED25519_SIGNATURES).map_err(|_| Error::Arithmetic)?;
    for (index, participant) in participants.iter().enumerate() {
        let descriptor = 2 + index * ED25519_DESCRIPTOR_BYTES;
        let public_key = ED25519_HEADER_BYTES + index * ED25519_PARTICIPANT_BYTES;
        let signature = public_key + 32;
        let family_offset = if index == 0 { 64 } else { 268 };
        let message = HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family_offset)
            .ok_or(Error::Arithmetic)?;
        for (offset, value) in [
            (descriptor, signature),
            (descriptor + 2, usize::from(u16::MAX)),
            (descriptor + 4, public_key),
            (descriptor + 6, usize::from(u16::MAX)),
            (descriptor + 8, message),
            (descriptor + 10, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2),
            (descriptor + 12, usize::from(CURRENT_HOT_INSTRUCTION_INDEX)),
        ] {
            put(
                &mut data,
                offset,
                &u16::try_from(value)
                    .map_err(|_| Error::Arithmetic)?
                    .to_le_bytes(),
            )?;
        }
        put(&mut data, public_key, participant.maker.as_ref())?;
        put(&mut data, signature, &participant.signature)?;
    }
    Ok(Instruction {
        program_id: ed25519_program::ID,
        accounts: Vec::new(),
        data,
    })
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Arithmetic)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(side: u8, maker_byte: u8) -> SignedDirectIntentV3 {
        SignedDirectIntentV3 {
            maker: Pubkey::new_from_array([maker_byte; 32]),
            signature: [maker_byte; 64],
            intent: CompactIntentV2 {
                side,
                lifecycle: 1,
                outcome: 70_000,
                market: [7; 32],
                generation: 9,
                nonce: 3,
                valid_from: 100,
                valid_through: 200,
                maximum_fill: 1_000,
                limit_price: if side == 0 { 400_000 } else { 600_000 },
                fee_basis_points: 25,
                collateral_account: [maker_byte + 10; 32],
            },
        }
    }

    #[test]
    fn inline_request_has_exact_signed_offsets_and_u32_outcome() {
        let seller = intent(0, 1);
        let buyer = intent(1, 2);
        let request = compile_direct_inline_request_v3(seller, buyer, 1_000, 500_000)
            .expect("inline request");
        assert_eq!(request.len(), 456);
        let seller_message = seller.intent.signed_preimage().expect("seller message");
        let buyer_message = buyer.intent.signed_preimage().expect("buyer message");
        assert_eq!(request.get(64..236), Some(seller_message.as_slice()));
        assert_eq!(request.get(268..440), Some(buyer_message.as_slice()));
        assert!(matches!(
            DirectExecutionRequestV3::decode(&request, 70_001),
            Ok(DirectExecutionRequestV3::InlineOrdinary(_))
        ));
        assert_eq!(
            request.get(440..448),
            Some(1_000_u64.to_le_bytes().as_slice())
        );
        assert_eq!(
            request.get(448..456),
            Some(500_000_u64.to_le_bytes().as_slice())
        );
    }

    #[test]
    fn adjacent_ed25519_reads_messages_from_hot_instruction_one() {
        let seller = intent(0, 1);
        let buyer = intent(1, 2);
        let instruction = native_ed25519_instruction([seller, buyer]).expect("native evidence");
        assert_eq!(instruction.program_id, ed25519_program::ID);
        assert_eq!(instruction.data.first().copied(), Some(2));
        for (descriptor, expected_message) in [(2_usize, 192_u16), (16, 396)] {
            assert_eq!(
                read_test_u16(&instruction.data, descriptor + 8),
                expected_message
            );
            assert_eq!(read_test_u16(&instruction.data, descriptor + 10), 172);
            assert_eq!(read_test_u16(&instruction.data, descriptor + 12), 1);
        }
    }

    fn read_test_u16(bytes: &[u8], offset: usize) -> u16 {
        let end = offset.checked_add(2).expect("test offset");
        let encoded = bytes.get(offset..end).expect("test u16 bytes");
        u16::from_le_bytes(<[u8; 2]>::try_from(encoded).expect("test u16 width"))
    }

    #[test]
    fn zero_signature_and_maker_alias_refuse_before_artifact_use() {
        let seller = intent(0, 1);
        let mut buyer = intent(1, 2);
        buyer.signature = [0; 64];
        assert_eq!(
            compile_direct_inline_request_v3(seller, buyer, 1, 1),
            Err(Error::ZeroIdentity)
        );
        let mut buyer = intent(1, 2);
        buyer.maker = seller.maker;
        assert_eq!(
            compile_direct_inline_request_v3(seller, buyer, 1, 1),
            Err(Error::ZeroIdentity)
        );
    }
}
