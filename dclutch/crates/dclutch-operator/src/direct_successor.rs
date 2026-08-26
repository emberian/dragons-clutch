//! Exact unsigned signing material for the Direct V2 successor.
//!
//! This module does not choose Market, generation, replay nonce, fee policy, or
//! transaction accounts. A chain-derived Trading builder owns those facts and
//! passes its already assembled [`CompactIntentV2`] here. The result is only the
//! exact domain-separated message a maker signs; it is not a signature,
//! authorization attestation, instruction, or submission request.

use dclutch_direct_codec::intent_v2::{
    CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
    CancelThroughV2, CompactIntentV2,
};
use solana_program::pubkey::Pubkey;

/// Exact material presented to a maker's signing interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIntentSigningPlanV2 {
    /// Maker public key expected to authenticate the message.
    pub maker: Pubkey,
    /// Sole canonical Direct V2 intent assembled from chain-derived authority.
    pub intent: CompactIntentV2,
    /// Exact domain-separated native-Ed25519 message.
    pub message: [u8; COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2],
}

/// Exact material presented for one maker-wide O(1) invalidation signature.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCancelThroughSigningPlanV2 {
    /// Maker public key expected to authenticate the message.
    pub maker: Pubkey,
    /// Sole canonical Direct V2 invalidation message.
    pub message: CancelThroughV2,
    /// Exact separately domain-separated native-Ed25519 message.
    pub signed_preimage: [u8; CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2],
}

/// Refusal while projecting one exact signing message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A zero maker cannot own replay authority.
    ZeroMaker,
    /// The canonical V2 intent could not be encoded.
    InvalidIntent,
}

/// Project one exact V2 signing message without signing or submitting it.
///
/// The chain-derived caller must construct `intent` from authenticated Market,
/// config, and maker-root state. This narrow function intentionally accepts no
/// detached signature, replay attestation, program address, or account frame.
/// The physical Trading builder will place these exact bytes into its
/// adjacency-checked Ed25519/Trading transaction pair.
pub fn compile_direct_intent_signing_plan_v2(
    maker: Pubkey,
    intent: CompactIntentV2,
) -> Result<DirectIntentSigningPlanV2, Error> {
    if maker == Pubkey::default() {
        return Err(Error::ZeroMaker);
    }
    let message = intent.signed_preimage().map_err(|_| Error::InvalidIntent)?;
    Ok(DirectIntentSigningPlanV2 {
        maker,
        intent,
        message,
    })
}

/// Project one exact V2 maker-wide invalidation message without signing it.
pub fn compile_direct_cancel_through_signing_plan_v2(
    maker: Pubkey,
    message: CancelThroughV2,
) -> Result<DirectCancelThroughSigningPlanV2, Error> {
    if maker == Pubkey::default() {
        return Err(Error::ZeroMaker);
    }
    let signed_preimage = message
        .signed_preimage()
        .map_err(|_| Error::InvalidIntent)?;
    Ok(DirectCancelThroughSigningPlanV2 {
        maker,
        message,
        signed_preimage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_direct_codec::intent_v2::{
        COMPACT_INTENT_BYTES_V2, COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2, CompactIntentV2,
    };

    fn intent() -> CompactIntentV2 {
        CompactIntentV2 {
            side: 1,
            lifecycle: 2,
            outcome: 70_000,
            market: [7; 32],
            generation: 9,
            nonce: 12,
            valid_from: 100,
            valid_through: 200,
            maximum_fill: 5_000,
            limit_price: 600_000,
            fee_basis_points: 25,
            collateral_account: [8; 32],
        }
    }

    #[test]
    fn signing_plan_is_exact_v2_domain_then_intent() {
        let maker = Pubkey::new_from_array([9; 32]);
        let intent = intent();
        let plan = compile_direct_intent_signing_plan_v2(maker, intent).expect("signing plan");
        assert_eq!(plan.maker, maker);
        assert_eq!(plan.intent, intent);
        assert_eq!(
            plan.message.get(..32),
            Some(COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2.as_slice())
        );
        assert_eq!(
            plan.message.get(32..),
            Some(
                intent
                    .encode()
                    .expect("intent")
                    .get(..COMPACT_INTENT_BYTES_V2)
                    .expect("exact intent")
            )
        );
        assert_eq!(
            CompactIntentV2::decode_signed_preimage(&plan.message),
            Ok(intent)
        );
    }

    #[test]
    fn zero_maker_refuses_without_message_authority() {
        assert_eq!(
            compile_direct_intent_signing_plan_v2(Pubkey::default(), intent()),
            Err(Error::ZeroMaker)
        );
    }

    #[test]
    fn cancel_through_signing_plan_uses_its_own_v2_domain() {
        let message = CancelThroughV2 {
            market: [7; 32],
            generation: 9,
            minimum_live_nonce: 13,
        };
        let plan =
            compile_direct_cancel_through_signing_plan_v2(Pubkey::new_from_array([9; 32]), message)
                .expect("kill-switch signing plan");
        assert_eq!(plan.message, message);
        assert_eq!(
            CancelThroughV2::decode_signed_preimage(&plan.signed_preimage),
            Ok(message)
        );
        assert_ne!(
            plan.signed_preimage.get(..32),
            Some(COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2.as_slice())
        );
    }
}
