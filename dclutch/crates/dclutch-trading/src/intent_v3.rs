//! The RFQ ticket: runtime-product-width signed intent V3.
//!
//! V3 is [`crate::intent_v2::CompactIntentV2`] byte for byte plus one field:
//! a 32-byte `counterparty` at offset 140. A maker who wants a specific taker
//! names one; a bearer ticket carries all zeros. The field is inside the
//! signed preimage, so nobody but the maker can narrow or widen who may
//! accept the quote.
//!
//! The magic, the version and the signature domain all move, so a V2 preimage
//! cannot replay as a V3 one and a V2 decoder cannot read a V3 ticket.
//!
//! `limit_price` keeps its V2 meaning — the seller's FLOOR or the buyer's CAP.
//! Under the RFQ neither party names the execution price: the ordinary
//! transition derives it as the equal split of the two limits
//! (`crate::ordinary_v3`, `SCALAR_DERIVED_PRICE_V3`).

use crate::{
    Error, array, byte, generated_intent_v3 as generated, intent_v2::CompactIntentV2, put,
    put_byte, reserved, slice,
};

pub use generated::{
    COMPACT_INTENT_BYTES_V3, COMPACT_INTENT_MAGIC_V3, COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3,
    COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V3, COMPACT_INTENT_VERSION_V3,
};

/// Named domain whose SHA-256 identity prefixes every V3 native Ed25519 message.
pub const COMPACT_INTENT_SIGNATURE_DOMAIN_PREIMAGE_V3: &[u8] =
    b"dclutch/signature/direct-compact-intent-v3";

/// A bearer ticket's counterparty: anyone may take it.
pub const COMPACT_INTENT_BEARER_COUNTERPARTY_V3: [u8; 32] = [0; 32];

/// The only independently signed RFQ ticket admitted by the Direct successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactIntentV3 {
    /// Seller `0` or buyer `1`.
    pub side: u8,
    /// Inline FOK `0`, inline IOC `1`, or registered `2`.
    pub lifecycle: u8,
    /// Product-V2 runtime outcome coordinate.
    pub outcome: u32,
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact next gap-free maker nonce.
    pub nonce: u64,
    /// First valid trusted Clock slot.
    pub valid_from: u64,
    /// Last valid trusted Clock slot, inclusive.
    pub valid_through: u64,
    /// Maximum admitted fill.
    pub maximum_fill: u64,
    /// Seller floor or buyer cap at the selected config's scale.
    pub limit_price: u64,
    /// Exact cumulative floor-fee rate accepted by the maker.
    pub fee_basis_points: u16,
    /// Seller destination or buyer source token account.
    pub collateral_account: [u8; 32],
    /// The maker this quote binds, or all zeros for a bearer ticket.
    pub counterparty: [u8; 32],
}

impl CompactIntentV3 {
    /// Hostile-decode one exact canonical RFQ ticket, excluding signature evidence.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidLength`] off the exact width, [`Error::InvalidMagic`] on
    /// any magic but `DCLTDIW3` — a V2 ticket included — [`Error::UnsupportedVersion`]
    /// off version three, and [`Error::NonzeroReserved`] on either reserved span.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != COMPACT_INTENT_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if slice(input, generated::COMPACT_INTENT_MAGIC_OFFSET_V3, 8)? != COMPACT_INTENT_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(input, generated::COMPACT_INTENT_VERSION_OFFSET_V3)?)
            != COMPACT_INTENT_VERSION_V3
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(input, generated::COMPACT_INTENT_RESERVED_A_OFFSET_V3, 4)?;
        reserved(input, generated::COMPACT_INTENT_RESERVED_B_OFFSET_V3, 6)?;
        Ok(Self {
            side: byte(input, generated::COMPACT_INTENT_SIDE_OFFSET_V3)?,
            lifecycle: byte(input, generated::COMPACT_INTENT_LIFECYCLE_OFFSET_V3)?,
            outcome: u32::from_le_bytes(array(input, generated::COMPACT_INTENT_OUTCOME_OFFSET_V3)?),
            market: array(input, generated::COMPACT_INTENT_MARKET_OFFSET_V3)?,
            generation: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_GENERATION_OFFSET_V3,
            )?),
            nonce: u64::from_le_bytes(array(input, generated::COMPACT_INTENT_NONCE_OFFSET_V3)?),
            valid_from: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_VALID_FROM_OFFSET_V3,
            )?),
            valid_through: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_VALID_THROUGH_OFFSET_V3,
            )?),
            maximum_fill: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V3,
            )?),
            limit_price: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V3,
            )?),
            fee_basis_points: u16::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V3,
            )?),
            collateral_account: array(
                input,
                generated::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V3,
            )?,
            counterparty: array(input, generated::COMPACT_INTENT_COUNTERPARTY_OFFSET_V3)?,
        })
    }

    /// Encode the exact canonical persisted ticket bytes.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidLength`] if any field does not fit its span, which the
    /// fixed layout makes unreachable.
    pub fn encode(self) -> Result<[u8; COMPACT_INTENT_BYTES_V3], Error> {
        let mut output = [0_u8; COMPACT_INTENT_BYTES_V3];
        put(
            &mut output,
            generated::COMPACT_INTENT_MAGIC_OFFSET_V3,
            &COMPACT_INTENT_MAGIC_V3,
        )?;
        put(
            &mut output,
            generated::COMPACT_INTENT_VERSION_OFFSET_V3,
            &COMPACT_INTENT_VERSION_V3.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated::COMPACT_INTENT_SIDE_OFFSET_V3,
            self.side,
        )?;
        put_byte(
            &mut output,
            generated::COMPACT_INTENT_LIFECYCLE_OFFSET_V3,
            self.lifecycle,
        )?;
        for (offset, value) in [
            (
                generated::COMPACT_INTENT_OUTCOME_OFFSET_V3,
                self.outcome.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_GENERATION_OFFSET_V3,
                self.generation.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_NONCE_OFFSET_V3,
                self.nonce.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_VALID_FROM_OFFSET_V3,
                self.valid_from.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_VALID_THROUGH_OFFSET_V3,
                self.valid_through.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V3,
                self.maximum_fill.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V3,
                self.limit_price.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V3,
                self.fee_basis_points.to_le_bytes().as_slice(),
            ),
        ] {
            put(&mut output, offset, value)?;
        }
        put(
            &mut output,
            generated::COMPACT_INTENT_MARKET_OFFSET_V3,
            &self.market,
        )?;
        put(
            &mut output,
            generated::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V3,
            &self.collateral_account,
        )?;
        put(
            &mut output,
            generated::COMPACT_INTENT_COUNTERPARTY_OFFSET_V3,
            &self.counterparty,
        )?;
        Ok(output)
    }

    /// Construct the exact native-Ed25519 message.
    ///
    /// # Errors
    ///
    /// Whatever [`Self::encode`] refuses.
    pub fn signed_preimage(self) -> Result<[u8; COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V3], Error> {
        let intent = self.encode()?;
        let mut output = [0_u8; COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V3];
        put(&mut output, 0, &COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3)?;
        put(
            &mut output,
            COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3.len(),
            &intent,
        )?;
        Ok(output)
    }

    /// Hostile-decode the exact message authenticated by native Ed25519.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidLength`] off the exact preimage width,
    /// [`Error::InvalidMagic`] on any signature domain but V3's, then whatever
    /// [`Self::decode`] refuses about the ticket that follows it.
    pub fn decode_signed_preimage(input: &[u8]) -> Result<Self, Error> {
        if input.len() != COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if slice(input, 0, COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3.len())?
            != COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3
        {
            return Err(Error::InvalidMagic);
        }
        Self::decode(slice(
            input,
            COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3.len(),
            COMPACT_INTENT_BYTES_V3,
        )?)
    }

    /// A ticket anyone may take: every counterparty byte is zero.
    #[must_use]
    pub fn bearer(self) -> bool {
        self.counterparty == COMPACT_INTENT_BEARER_COUNTERPARTY_V3
    }

    /// The counterparty conjunct: a named ticket binds exactly one other
    /// maker, a bearer ticket binds nobody.
    #[must_use]
    pub fn admits(self, other_maker: &[u8; 32]) -> bool {
        self.bearer() || self.counterparty == *other_maker
    }

    /// The V2 terms a V3 ticket carries: every field but the counterparty.
    ///
    /// The economics read exactly these — the counterparty is an admission
    /// conjunct, never a price or a quantity — so the ordinary transition
    /// keeps one author for the terms it executes.
    #[must_use]
    pub fn terms(self) -> CompactIntentV2 {
        CompactIntentV2 {
            side: self.side,
            lifecycle: self.lifecycle,
            outcome: self.outcome,
            market: self.market,
            generation: self.generation,
            nonce: self.nonce,
            valid_from: self.valid_from,
            valid_through: self.valid_through,
            maximum_fill: self.maximum_fill,
            limit_price: self.limit_price,
            fee_basis_points: self.fee_basis_points,
            collateral_account: self.collateral_account,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generated_intent_v2, generated_intent_v3 as generated};

    fn named() -> CompactIntentV3 {
        CompactIntentV3 {
            side: 0,
            lifecycle: 0,
            outcome: 3,
            market: [0x21; 32],
            generation: 9,
            nonce: 12,
            valid_from: 100,
            valid_through: 200,
            maximum_fill: 5_000,
            limit_price: 400_000,
            fee_basis_points: 25,
            collateral_account: [0x45; 32],
            counterparty: [0x77; 32],
        }
    }

    fn bearer() -> CompactIntentV3 {
        CompactIntentV3 {
            counterparty: COMPACT_INTENT_BEARER_COUNTERPARTY_V3,
            ..named()
        }
    }

    #[test]
    fn the_lean_owned_named_and_bearer_tickets_round_trip() {
        let intent = named();
        let bytes = intent.encode().expect("named ticket");
        assert_eq!(bytes, generated::COMPACT_INTENT_NAMED_EXAMPLE_V3);
        assert_eq!(CompactIntentV3::decode(&bytes), Ok(intent));
        let signed = intent.signed_preimage().expect("named preimage");
        assert_eq!(signed, generated::COMPACT_INTENT_NAMED_SIGNED_EXAMPLE_V3);
        assert_eq!(CompactIntentV3::decode_signed_preimage(&signed), Ok(intent));

        let open = bearer();
        let open_bytes = open.encode().expect("bearer ticket");
        assert_eq!(open_bytes, generated::COMPACT_INTENT_BEARER_EXAMPLE_V3);
        assert_eq!(CompactIntentV3::decode(&open_bytes), Ok(open));
        assert_ne!(bytes, open_bytes);
    }

    #[test]
    fn the_counterparty_is_the_only_field_the_terms_projection_forgets() {
        assert_eq!(named().terms(), bearer().terms());
        assert_eq!(
            named().terms().encode().expect("named terms"),
            bearer().terms().encode().expect("bearer terms")
        );
        assert_eq!(named().terms().limit_price, named().limit_price);
        assert_eq!(
            COMPACT_INTENT_BYTES_V3,
            generated_intent_v2::COMPACT_INTENT_BYTES_V2 + 32
        );
    }

    #[test]
    fn a_named_ticket_admits_its_taker_alone_and_a_bearer_ticket_admits_anyone() {
        assert!(!named().bearer());
        assert!(named().admits(&[0x77; 32]));
        assert!(!named().admits(&[0x78; 32]));
        assert!(bearer().bearer());
        assert!(bearer().admits(&[0x77; 32]));
        assert!(bearer().admits(&[0x78; 32]));
    }

    #[test]
    fn a_v2_ticket_a_v2_domain_and_a_dirty_reserved_span_all_refuse() {
        let v2 = CompactIntentV2 {
            side: 0,
            lifecycle: 0,
            outcome: 3,
            market: [0x21; 32],
            generation: 9,
            nonce: 12,
            valid_from: 100,
            valid_through: 200,
            maximum_fill: 5_000,
            limit_price: 400_000,
            fee_basis_points: 25,
            collateral_account: [0x45; 32],
        }
        .encode()
        .expect("v2 ticket");
        assert_eq!(CompactIntentV3::decode(&v2), Err(Error::InvalidLength));

        let mut widened = [0_u8; COMPACT_INTENT_BYTES_V3];
        widened[..v2.len()].copy_from_slice(&v2);
        assert_eq!(CompactIntentV3::decode(&widened), Err(Error::InvalidMagic));

        let mut wrong_version = named().encode().expect("named ticket");
        wrong_version[generated::COMPACT_INTENT_VERSION_OFFSET_V3] = 2;
        assert_eq!(
            CompactIntentV3::decode(&wrong_version),
            Err(Error::UnsupportedVersion)
        );

        let mut dirty = named().encode().expect("named ticket");
        dirty[generated::COMPACT_INTENT_RESERVED_A_OFFSET_V3] = 1;
        assert_eq!(CompactIntentV3::decode(&dirty), Err(Error::NonzeroReserved));
        let mut dirty_b = named().encode().expect("named ticket");
        dirty_b[generated::COMPACT_INTENT_RESERVED_B_OFFSET_V3] = 1;
        assert_eq!(
            CompactIntentV3::decode(&dirty_b),
            Err(Error::NonzeroReserved)
        );

        let mut v2_domain = named().signed_preimage().expect("named preimage");
        v2_domain[..32]
            .copy_from_slice(&generated_intent_v2::COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2);
        assert_eq!(
            CompactIntentV3::decode_signed_preimage(&v2_domain),
            Err(Error::InvalidMagic)
        );
    }
}
