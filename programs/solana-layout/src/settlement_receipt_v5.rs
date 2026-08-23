//! Rent-owned General SettlementReceipt successor.
//!
//! V5 preserves the complete V4 accounting/delivery/payment state machine,
//! adds one typed transition commitment, and appends the exact payer-owned rent
//! principal and hostile-prefund floor needed to delete the receipt. V4
//! remains historical and cannot be decoded through this schema. Every PDA,
//! account-data, and Replay transition domain is fresh, so no V4 address or
//! identity can be promoted by changing a version byte.

use super::{check_hash, digest, CodecError, Hash32, Result, HASH_BYTES};
pub use crate::registry::{
    GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_BYTES as SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
    GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG, GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION,
};
use crate::{
    reservation_v9::DeletableRentOwnerV1, settlement_receipt_v4::SettlementReceiptAccountV4,
};
/// Historical V4 semantic body width, including its tag/version and bump.
pub const SETTLEMENT_RECEIPT_SEMANTIC_V4_BYTES: usize = 217;
/// Exact typed transition-commitment width.
pub const SETTLEMENT_RECEIPT_TRANSITION_COMMITMENT_BYTES_V5: usize = 33;
/// Exact persisted rent owner width.
pub const SETTLEMENT_RECEIPT_RENT_OWNER_BYTES_V5: usize = 48;
/// Fresh PDA seed domain.
pub const GENERAL_SETTLEMENT_RECEIPT_SEED_V5: &[u8] = b"general-receipt:v5";
/// Fresh account-data identity domain.
pub const RECEIPT_DATA_ID_DOMAIN_V5: &[u8] = b"dragons-clutch/general-settlement-receipt/data/v5\0";
/// Fresh action-25 accounting identity domain.
pub const RECEIPT_ACCOUNTING_ID_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-settlement-receipt/accounting/v5\0";
/// Fresh action-26/36/37 delivery identity domain.
pub const RECEIPT_DELIVERY_ID_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-settlement-receipt/delivery/v5\0";
/// Fresh action-40 payment identity domain.
pub const RECEIPT_PAYMENT_ID_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-settlement-receipt/payment/v5\0";
/// Domain of the exact portfolio-pair transition preimage.
///
/// The preimage includes the V5 pre-data ID and every exact economic,
/// Position, Reservation, Replay, and post-semantic field, but deliberately
/// excludes only the resulting V5 post-data ID. Including the post-data ID
/// would be self-referential because it commits this stored hash.
pub const PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/portfolio-pair-transition/v2\0";
/// Frozen exact portfolio-pair transition preimage width.
pub const PORTFOLIO_PAIR_TRANSITION_PREIMAGE_BYTES_V2: usize = 680;
/// Exact data-ID transcript: authenticated PDA then the complete V5 account.
pub const SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V5_BYTES: usize =
    HASH_BYTES + SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5;

const TRANSITION_KIND_OFFSET: usize = SETTLEMENT_RECEIPT_SEMANTIC_V4_BYTES;
const TRANSITION_COMMITMENT_OFFSET: usize = TRANSITION_KIND_OFFSET + 1;
const RENT_OFFSET: usize = TRANSITION_COMMITMENT_OFFSET + HASH_BYTES;

/// No specialized vector transition is bound to this receipt.
pub const RECEIPT_TRANSITION_KIND_NONE_V5: u8 = 0;
/// Exact exclusive two-order portfolio-pair transition V2.
pub const RECEIPT_TRANSITION_KIND_PORTFOLIO_PAIR_V2: u8 = 1;

/// Typed specialized transition commitment embedded in a V5 receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementReceiptTransitionCommitmentV5 {
    /// Direct/split/merge General receipt with no specialized commitment.
    None,
    /// Portfolio pair before its atomic delivery transition.
    PortfolioPairPending,
    /// Exact portfolio transition hash, immutable after atomic delivery.
    PortfolioPairCommitted(Hash32),
}

impl SettlementReceiptTransitionCommitmentV5 {
    fn validate(self, semantic: SettlementReceiptAccountV4) -> Result<()> {
        match self {
            Self::None => Ok(()),
            Self::PortfolioPairPending => {
                if semantic.leg_kind != crate::RECEIPT_LEG_DIRECT
                    || semantic.delivered_end_mask() != 0
                {
                    return Err(CodecError::InvalidEnum);
                }
                Ok(())
            }
            Self::PortfolioPairCommitted(commitment) => {
                check_hash(commitment)?;
                if semantic.leg_kind != crate::RECEIPT_LEG_DIRECT || !semantic.payment_complete() {
                    return Err(CodecError::InvalidEnum);
                }
                Ok(())
            }
        }
    }

    const fn kind(self) -> u8 {
        match self {
            Self::None => RECEIPT_TRANSITION_KIND_NONE_V5,
            Self::PortfolioPairPending | Self::PortfolioPairCommitted(_) => {
                RECEIPT_TRANSITION_KIND_PORTFOLIO_PAIR_V2
            }
        }
    }

    const fn commitment(self) -> Hash32 {
        match self {
            Self::PortfolioPairCommitted(value) => value,
            Self::None | Self::PortfolioPairPending => Hash32::ZERO,
        }
    }

    fn decode(kind: u8, commitment: Hash32, semantic: SettlementReceiptAccountV4) -> Result<Self> {
        let value = match kind {
            RECEIPT_TRANSITION_KIND_NONE_V5 if commitment == Hash32::ZERO => Self::None,
            RECEIPT_TRANSITION_KIND_NONE_V5 => return Err(CodecError::NonCanonicalPadding),
            RECEIPT_TRANSITION_KIND_PORTFOLIO_PAIR_V2 if commitment == Hash32::ZERO => {
                Self::PortfolioPairPending
            }
            RECEIPT_TRANSITION_KIND_PORTFOLIO_PAIR_V2 => Self::PortfolioPairCommitted(commitment),
            _ => return Err(CodecError::InvalidEnum),
        };
        value.validate(semantic)?;
        Ok(value)
    }
}

/// Sole future rent-owned receipt account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptAccountV5 {
    semantic: SettlementReceiptAccountV4,
    transition: SettlementReceiptTransitionCommitmentV5,
    rent: DeletableRentOwnerV1,
}

impl SettlementReceiptAccountV5 {
    /// Bind one exact V4 lifecycle state to its permanent deletable rent owner.
    pub fn new(
        semantic: SettlementReceiptAccountV4,
        transition: SettlementReceiptTransitionCommitmentV5,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        semantic.validate()?;
        transition.validate(semantic)?;
        rent.validate()?;
        Ok(Self {
            semantic,
            transition,
            rent,
        })
    }

    /// Exact accounting/delivery/payment semantic owner.
    pub const fn semantic(&self) -> SettlementReceiptAccountV4 {
        self.semantic
    }

    /// Typed specialized transition state.
    pub const fn transition(&self) -> SettlementReceiptTransitionCommitmentV5 {
        self.transition
    }

    /// Exact payer/refundable-principal/donation owner.
    pub const fn rent(&self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Atomically latch the one exact portfolio transition and delivery state.
    ///
    /// The poststate is derived rather than supplied: both direct ends become
    /// delivered, the slice becomes exhausted, consideration quantity is
    /// settled, and the already-accounted latch is preserved. A pending or
    /// completed accounting state cannot be used to rewrite the commitment.
    pub fn commit_portfolio_pair_delivery(self, commitment: Hash32) -> Result<Self> {
        check_hash(commitment)?;
        if self.transition != SettlementReceiptTransitionCommitmentV5::PortfolioPairPending
            || self.semantic.leg_kind != crate::RECEIPT_LEG_DIRECT
            || self.semantic.accounted_end_mask != self.semantic.expected_end_mask()
            || self.semantic.delivered_end_mask() != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        let mut semantic = self.semantic;
        semantic.settled_quantity = semantic.quantity;
        semantic.consumed_flags = crate::RECEIPT_FLAG_BUY_CONSUMED
            | crate::RECEIPT_FLAG_SELL_CONSUMED
            | crate::RECEIPT_FLAG_SLICE_EXHAUSTED;
        Self::new(
            semantic,
            SettlementReceiptTransitionCommitmentV5::PortfolioPairCommitted(commitment),
            self.rent,
        )
    }

    /// Validate both semantic and lamport compartments.
    pub fn validate(&self) -> Result<()> {
        self.semantic.validate()?;
        self.transition.validate(self.semantic)?;
        self.rent.validate()
    }

    /// Encode exactly 298 canonical bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        if output.len() < SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 {
            return Err(CodecError::OutputTooSmall);
        }
        if output.len() > SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 {
            return Err(CodecError::TrailingBytes);
        }
        let semantic = self.semantic.encode_exact()?;
        output[..SETTLEMENT_RECEIPT_SEMANTIC_V4_BYTES].copy_from_slice(&semantic);
        output[1] = GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION;
        output[TRANSITION_KIND_OFFSET] = self.transition.kind();
        output[TRANSITION_COMMITMENT_OFFSET..RENT_OFFSET]
            .copy_from_slice(&self.transition.commitment().0);
        write_rent(&mut output[RENT_OFFSET..], self.rent);
        Ok(())
    }

    /// Return the exact canonical account bytes.
    pub fn encode_exact(&self) -> Result<[u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5]> {
        let mut output = [0u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5];
        self.encode(&mut output)?;
        Ok(output)
    }

    /// Decode exactly 298 hostile bytes and refuse every other version.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 {
            return Err(CodecError::Truncated);
        }
        if input.len() > SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 {
            return Err(CodecError::TrailingBytes);
        }
        if input[0] != GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let mut historical = [0u8; SETTLEMENT_RECEIPT_SEMANTIC_V4_BYTES];
        historical.copy_from_slice(&input[..SETTLEMENT_RECEIPT_SEMANTIC_V4_BYTES]);
        historical[1] = crate::registry::GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION;
        let semantic = SettlementReceiptAccountV4::decode(&historical)?;
        let mut commitment = [0u8; HASH_BYTES];
        commitment.copy_from_slice(&input[TRANSITION_COMMITMENT_OFFSET..RENT_OFFSET]);
        let transition = SettlementReceiptTransitionCommitmentV5::decode(
            input[TRANSITION_KIND_OFFSET],
            Hash32::from_bytes(commitment),
            semantic,
        )?;
        let rent = read_rent(&input[RENT_OFFSET..])?;
        Self::new(semantic, transition, rent)
    }

    /// Derive all three stable, mutually disjoint V5 action identities.
    pub fn transition_ids(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<SettlementReceiptTransitionIdsV5> {
        self.validate()?;
        check_hash(authenticated_receipt_pda)?;
        let accounting = digest(
            RECEIPT_ACCOUNTING_ID_DOMAIN_V5,
            &[&authenticated_receipt_pda.0],
        );
        let delivery = digest(
            RECEIPT_DELIVERY_ID_DOMAIN_V5,
            &[&authenticated_receipt_pda.0],
        );
        let payment = digest(
            RECEIPT_PAYMENT_ID_DOMAIN_V5,
            &[&authenticated_receipt_pda.0],
        );
        check_hash(accounting)?;
        check_hash(delivery)?;
        check_hash(payment)?;
        if accounting == delivery || accounting == payment || delivery == payment {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(SettlementReceiptTransitionIdsV5 {
            receipt_accounting_id: accounting,
            delivery_transition_id: delivery,
            payment_transition_id: payment,
        })
    }

    /// Encode the exact data-ID transcript.
    pub fn data_id_transcript(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<[u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V5_BYTES]> {
        check_hash(authenticated_receipt_pda)?;
        let body = self.encode_exact()?;
        let mut transcript = [0u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V5_BYTES];
        transcript[..HASH_BYTES].copy_from_slice(&authenticated_receipt_pda.0);
        transcript[HASH_BYTES..].copy_from_slice(&body);
        Ok(transcript)
    }

    /// Exact mutable V5 receipt-prestate identity.
    pub fn data_id(&self, authenticated_receipt_pda: Hash32) -> Result<Hash32> {
        let transcript = self.data_id_transcript(authenticated_receipt_pda)?;
        let data_id = digest(RECEIPT_DATA_ID_DOMAIN_V5, &[&transcript]);
        check_hash(data_id)?;
        Ok(data_id)
    }

    /// Project exact current bytes and deterministic action identities.
    pub fn evidence(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<SettlementReceiptEvidenceV5> {
        Ok(SettlementReceiptEvidenceV5 {
            receipt: authenticated_receipt_pda,
            exact_body: self.encode_exact()?,
            receipt_data_id: self.data_id(authenticated_receipt_pda)?,
            transition_ids: self.transition_ids(authenticated_receipt_pda)?,
        })
    }
}

/// Stable V5 action identities derived only from an authenticated receipt PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptTransitionIdsV5 {
    receipt_accounting_id: Hash32,
    delivery_transition_id: Hash32,
    payment_transition_id: Hash32,
}

impl SettlementReceiptTransitionIdsV5 {
    /// Accounting-only action-25 identity.
    pub const fn receipt_accounting_id(&self) -> Hash32 {
        self.receipt_accounting_id
    }
    /// Atomic Egg-delivery action identity.
    pub const fn delivery_transition_id(&self) -> Hash32 {
        self.delivery_transition_id
    }
    /// Merge-payment action-40 identity.
    pub const fn payment_transition_id(&self) -> Hash32 {
        self.payment_transition_id
    }
}

/// Exact decoded V5 receipt evidence for a higher checked composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptEvidenceV5 {
    receipt: Hash32,
    exact_body: [u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5],
    receipt_data_id: Hash32,
    transition_ids: SettlementReceiptTransitionIdsV5,
}

impl SettlementReceiptEvidenceV5 {
    /// Authenticated V5 receipt PDA.
    pub const fn receipt(&self) -> Hash32 {
        self.receipt
    }
    /// Exact current V5 account bytes.
    pub const fn exact_body(&self) -> &[u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5] {
        &self.exact_body
    }
    /// Exact current V5 account-data identity.
    pub const fn receipt_data_id(&self) -> Hash32 {
        self.receipt_data_id
    }
    /// Accounting-only action identity.
    pub const fn receipt_accounting_id(&self) -> Hash32 {
        self.transition_ids.receipt_accounting_id()
    }
    /// Delivery action identity.
    pub const fn delivery_transition_id(&self) -> Hash32 {
        self.transition_ids.delivery_transition_id()
    }
    /// Merge-payment action identity.
    pub const fn payment_transition_id(&self) -> Hash32 {
        self.transition_ids.payment_transition_id()
    }
}

/// Decode hostile V5 bytes and project deterministic action evidence.
pub fn project_settlement_receipt_evidence_v5(
    authenticated_receipt_pda: Hash32,
    exact_body: &[u8],
) -> Result<(SettlementReceiptAccountV5, SettlementReceiptEvidenceV5)> {
    check_hash(authenticated_receipt_pda)?;
    let receipt = SettlementReceiptAccountV5::decode(exact_body)?;
    let evidence = receipt.evidence(authenticated_receipt_pda)?;
    Ok((receipt, evidence))
}

/// Derive the exact nonzero portfolio-pair transition commitment.
///
/// The caller supplies the canonical 680-byte preimage owned by the portfolio
/// transition contract. That preimage must omit only the circular V5 post-data
/// ID; every post-semantic field remains present.
pub fn portfolio_pair_transition_commitment_v2(
    exact_preimage: &[u8; PORTFOLIO_PAIR_TRANSITION_PREIMAGE_BYTES_V2],
) -> Result<Hash32> {
    let commitment = digest(
        PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2,
        &[exact_preimage],
    );
    check_hash(commitment)?;
    Ok(commitment)
}

fn write_rent(output: &mut [u8], rent: DeletableRentOwnerV1) {
    output[..HASH_BYTES].copy_from_slice(&rent.payer.bytes());
    output[HASH_BYTES..HASH_BYTES + 8].copy_from_slice(&rent.refundable_principal.to_le_bytes());
    output[HASH_BYTES + 8..HASH_BYTES + 16].copy_from_slice(&rent.donation_floor.to_le_bytes());
}

fn read_rent(input: &[u8]) -> Result<DeletableRentOwnerV1> {
    if input.len() != SETTLEMENT_RECEIPT_RENT_OWNER_BYTES_V5 {
        return Err(CodecError::Truncated);
    }
    let mut payer = [0u8; HASH_BYTES];
    payer.copy_from_slice(&input[..HASH_BYTES]);
    let mut principal = [0u8; 8];
    principal.copy_from_slice(&input[HASH_BYTES..HASH_BYTES + 8]);
    let mut donation = [0u8; 8];
    donation.copy_from_slice(&input[HASH_BYTES + 8..]);
    let rent = DeletableRentOwnerV1 {
        payer: Hash32::from_bytes(payer),
        refundable_principal: u64::from_le_bytes(principal),
        donation_floor: u64::from_le_bytes(donation),
    };
    rent.validate()?;
    Ok(rent)
}

const _: () = assert!(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 == 298);
const _: () = assert!(
    SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5
        == SETTLEMENT_RECEIPT_SEMANTIC_V4_BYTES
            + SETTLEMENT_RECEIPT_TRANSITION_COMMITMENT_BYTES_V5
            + SETTLEMENT_RECEIPT_RENT_OWNER_BYTES_V5
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        settlement_receipt_v4::RECEIPT_ACCOUNTED_SELL_END, RECEIPT_FLAG_SELL_CONSUMED,
        RECEIPT_LEG_MERGE,
    };

    fn id(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; HASH_BYTES])
    }

    fn value() -> SettlementReceiptAccountV5 {
        SettlementReceiptAccountV5::new(
            SettlementReceiptAccountV4 {
                epoch: id(1),
                market: id(2),
                candidate: id(3),
                buy_order_id: Hash32::ZERO,
                sell_order_id: id(4),
                consideration_price_units: 0,
                quantity: 7,
                settled_quantity: 7,
                price: 0,
                sequence: 10,
                slice_index: 9,
                outcome: 3,
                leg_kind: RECEIPT_LEG_MERGE,
                consumed_flags: RECEIPT_FLAG_SELL_CONSUMED,
                stored_bump: 254,
                accounted_end_mask: RECEIPT_ACCOUNTED_SELL_END,
            },
            SettlementReceiptTransitionCommitmentV5::None,
            DeletableRentOwnerV1 {
                payer: id(9),
                refundable_principal: 2_000,
                donation_floor: 17,
            },
        )
        .unwrap()
    }

    #[test]
    fn exact_round_trip_refuses_v4_and_preserves_rent() {
        let value = value();
        let bytes = value.encode_exact().unwrap();
        assert_eq!(&bytes[..2], &[0x0f, 5]);
        assert_eq!(SettlementReceiptAccountV5::decode(&bytes), Ok(value));
        assert_eq!(value.rent().payer, id(9));
        assert_eq!(
            SettlementReceiptAccountV4::decode(&bytes),
            Err(CodecError::TrailingBytes)
        );
        let mut wrong = bytes;
        wrong[1] = 4;
        assert_eq!(
            SettlementReceiptAccountV5::decode(&wrong),
            Err(CodecError::WrongVersion)
        );
    }

    #[test]
    fn exact_lengths_and_three_fresh_transition_domains_refuse_aliases() {
        let value = value();
        let bytes = value.encode_exact().unwrap();
        let mut oversized = [0u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 + 1];
        oversized[..SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5].copy_from_slice(&bytes);
        for len in 0..=SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 + 1 {
            let result = SettlementReceiptAccountV5::decode(&oversized[..len]);
            if len == SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 {
                assert_eq!(result, Ok(value));
            } else if len < SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5 {
                assert_eq!(result, Err(CodecError::Truncated));
            } else {
                assert_eq!(result, Err(CodecError::TrailingBytes));
            }
        }
        let evidence = value.evidence(id(90)).unwrap();
        assert_ne!(
            evidence.receipt_accounting_id(),
            evidence.delivery_transition_id()
        );
        assert_ne!(
            evidence.receipt_accounting_id(),
            evidence.payment_transition_id()
        );
        assert_ne!(
            evidence.delivery_transition_id(),
            evidence.payment_transition_id()
        );
        let v4 = value.semantic().evidence(id(90)).unwrap();
        assert_ne!(evidence.receipt_data_id(), v4.receipt_data_id());
        assert_ne!(evidence.receipt_accounting_id(), v4.receipt_accounting_id());
    }

    #[test]
    fn hostile_transition_kind_commitment_pairs_are_refused() {
        let value = value();
        let bytes = value.encode_exact().unwrap();

        let mut unknown = bytes;
        unknown[TRANSITION_KIND_OFFSET] = 2;
        assert_eq!(
            SettlementReceiptAccountV5::decode(&unknown),
            Err(CodecError::InvalidEnum)
        );

        let mut none_with_hash = bytes;
        none_with_hash[TRANSITION_COMMITMENT_OFFSET] = 1;
        assert_eq!(
            SettlementReceiptAccountV5::decode(&none_with_hash),
            Err(CodecError::NonCanonicalPadding)
        );

        let mut pending_terminal = bytes;
        pending_terminal[TRANSITION_KIND_OFFSET] = RECEIPT_TRANSITION_KIND_PORTFOLIO_PAIR_V2;
        assert_eq!(
            SettlementReceiptAccountV5::decode(&pending_terminal),
            Err(CodecError::InvalidEnum)
        );

        let mut committed = pending_terminal;
        committed[TRANSITION_COMMITMENT_OFFSET] = 1;
        assert_eq!(
            SettlementReceiptAccountV5::decode(&committed),
            Err(CodecError::InvalidEnum)
        );
    }

    #[test]
    fn portfolio_commitment_is_set_once_with_exact_terminal_delivery() {
        let mut semantic = value().semantic();
        semantic.buy_order_id = id(6);
        semantic.sell_order_id = id(7);
        semantic.leg_kind = crate::RECEIPT_LEG_DIRECT;
        semantic.settled_quantity = 0;
        semantic.consumed_flags = 0;
        semantic.accounted_end_mask =
            crate::settlement_receipt_v4::RECEIPT_ACCOUNTED_BUY_END | RECEIPT_ACCOUNTED_SELL_END;
        let pending = SettlementReceiptAccountV5::new(
            semantic,
            SettlementReceiptTransitionCommitmentV5::PortfolioPairPending,
            value().rent(),
        )
        .unwrap();
        let committed = pending.commit_portfolio_pair_delivery(id(70)).unwrap();
        assert_eq!(
            committed.transition(),
            SettlementReceiptTransitionCommitmentV5::PortfolioPairCommitted(id(70))
        );
        assert!(committed.semantic().payment_complete());
        assert_eq!(
            committed.commit_portfolio_pair_delivery(id(71)),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(
            SettlementReceiptAccountV5::decode(&committed.encode_exact().unwrap()),
            Ok(committed)
        );
    }
}
