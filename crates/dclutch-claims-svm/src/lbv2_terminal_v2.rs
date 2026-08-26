//! Typed LiabilityBasisV2 terminal-redemption evidence ABI.
//!
//! This fixed-layout, allocation-free contract binds the exact Product/basis
//! authority chain, finalized rational terminal coordinate, Claims debit, and
//! optional Custody payout. It performs no hashing and evaluates no payoff:
//! the physical adapter must authenticate those facts before constructing the
//! request and must supply the digest of every exact byte transcript.

/// Exact terminal-redemption request width.
pub const LBV2_TERMINAL_REQUEST_BYTES_V2: usize = 480;
/// Exact terminal-redemption receipt width.
pub const LBV2_TERMINAL_RECEIPT_BYTES_V2: usize = 624;
/// Terminal-redemption request magic.
pub const LBV2_TERMINAL_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLBTR02";
/// Terminal-redemption receipt magic.
pub const LBV2_TERMINAL_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLBTE02";
/// Implemented terminal-redemption wire version.
pub const LBV2_TERMINAL_WIRE_VERSION_V2: u16 = 2;
/// Exact sentinel for absent Custody replay revisions when evaluated payout is zero.
pub const LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2: u64 = u64::MAX;

const VERSION_OFFSET: usize = 8;
const HEADER_RESERVED_OFFSET: usize = 10;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const PRODUCT_RECORD_OFFSET: usize = 80;
const SEMANTIC_PRODUCT_OFFSET: usize = 112;
const SEMANTIC_BASIS_OFFSET: usize = 144;
const LINKED_BASIS_RECORD_OFFSET: usize = 176;
const TERMINAL_COORDINATE_DIGEST_OFFSET: usize = 208;
const OWNER_OFFSET: usize = 240;
const PROTOCOL_POSITION_OFFSET: usize = 272;
const CLAIMS_PROGRAM_OFFSET: usize = 304;
const CUSTODY_REQUEST_DIGEST_OFFSET: usize = 336;
const CANDIDATE_DIGEST_OFFSET: usize = 368;
const TERMINAL_NUMERATOR_OFFSET: usize = 400;
const TERMINAL_DENOMINATOR_OFFSET: usize = 408;
const CLAIM_INDEX_OFFSET: usize = 412;
const PRE_MARKET_REVISION_OFFSET: usize = 416;
const POST_MARKET_REVISION_OFFSET: usize = 424;
const PRE_POSITION_REVISION_OFFSET: usize = 432;
const POST_POSITION_REVISION_OFFSET: usize = 440;
const DEBIT_QUANTITY_OFFSET: usize = 448;
const EVALUATED_PAYOUT_OFFSET: usize = 456;
const PRE_CUSTODY_REVISION_OFFSET: usize = 464;
const POST_CUSTODY_REVISION_OFFSET: usize = 472;

const RECEIPT_REQUEST_OFFSET: usize = 16;
const RECEIPT_REQUEST_DIGEST_OFFSET: usize =
    RECEIPT_REQUEST_OFFSET + LBV2_TERMINAL_REQUEST_BYTES_V2;
const RECEIPT_CUSTODY_RECEIPT_DIGEST_OFFSET: usize = RECEIPT_REQUEST_DIGEST_OFFSET + 32;
const RECEIPT_CUSTODY_REPLAY_DIGEST_OFFSET: usize = RECEIPT_CUSTODY_RECEIPT_DIGEST_OFFSET + 32;
const RECEIPT_POST_RESOURCE_DIGEST_OFFSET: usize = RECEIPT_CUSTODY_REPLAY_DIGEST_OFFSET + 32;

/// Stable hostile-decode or evidence refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lbv2TerminalErrorV2 {
    /// A request or receipt did not have its exact fixed width.
    InvalidLength,
    /// Magic bytes selected another packet family.
    InvalidMagic,
    /// The wire version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes were nonzero.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// The owner and ProtocolPosition account aliased.
    AccountAlias,
    /// The finalized rational terminal coordinate had a zero denominator.
    InvalidRational,
    /// An aggregate, Position, or Custody revision was not exact.
    InvalidRevision,
    /// The requested Claims debit was zero.
    InvalidQuantity,
    /// Evaluated payout and Custody evidence presence disagreed.
    InvalidCustodyShape,
    /// A receipt did not bind the exact request or digest supplied by its caller.
    ReceiptMismatch,
}

/// Result alias for the terminal-redemption ABI.
pub type Result<T> = core::result::Result<T, Lbv2TerminalErrorV2>;

/// Construction input for one exact terminal-redemption request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Lbv2TerminalRedeemRequestInputV2 {
    /// Immutable current execution release set.
    pub release_set: [u8; 32],
    /// Canonical logical Core Market identity.
    pub market: [u8; 32],
    /// Exact finalized Product-record digest.
    pub product_record_digest: [u8; 32],
    /// Exact semantic Product identity authenticated inside that record.
    pub semantic_product_id: [u8; 32],
    /// Exact semantic LiabilityBasisV2 identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact finalized linked-basis raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Exact finalized terminal-coordinate raw-record digest.
    pub terminal_coordinate_digest: [u8; 32],
    /// Sole holder whose native claim is debited.
    pub owner: [u8; 32],
    /// Canonical Claims ProtocolPosition account for `owner`.
    pub protocol_position: [u8; 32],
    /// Registry-authenticated Claims program producing the receipt.
    pub claims_program: [u8; 32],
    /// SHA-256 of the exact Custody request, zero exactly for zero payout.
    pub custody_request_digest: [u8; 32],
    /// SHA-256 of exact candidate aggregate followed by Position bytes.
    pub candidate_digest: [u8; 32],
    /// Signed finalized rational coordinate numerator.
    pub terminal_numerator: i64,
    /// Positive finalized rational coordinate denominator.
    pub terminal_denominator: u32,
    /// Product claim coordinate debited by the redemption.
    pub claim_index: u32,
    /// Exact Claims aggregate pre-revision.
    pub pre_market_revision: u64,
    /// Exact Claims aggregate post-revision.
    pub post_market_revision: u64,
    /// Exact ProtocolPosition pre-revision.
    pub pre_position_revision: u64,
    /// Exact ProtocolPosition post-revision.
    pub post_position_revision: u64,
    /// Positive native claim quantity debited from aggregate and Position.
    pub debit_quantity: u64,
    /// Exact evaluated collateral payout after checked multiplication.
    pub evaluated_payout: u64,
    /// Exact Custody replay pre-revision, or the absent sentinel.
    pub pre_custody_revision: u64,
    /// Exact Custody replay post-revision, or the absent sentinel.
    pub post_custody_revision: u64,
}

/// One exact, hostile-decodable terminal-redemption request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Lbv2TerminalRedeemRequestV2(Lbv2TerminalRedeemRequestInputV2);

impl Lbv2TerminalRedeemRequestV2 {
    /// Construct and fully validate one request.
    pub fn new(input: Lbv2TerminalRedeemRequestInputV2) -> Result<Self> {
        for identity in [
            input.release_set,
            input.market,
            input.product_record_digest,
            input.semantic_product_id,
            input.semantic_basis_id,
            input.linked_basis_record_digest,
            input.terminal_coordinate_digest,
            input.owner,
            input.protocol_position,
            input.claims_program,
            input.candidate_digest,
        ] {
            require_nonzero(identity)?;
        }
        if input.owner == input.protocol_position {
            return Err(Lbv2TerminalErrorV2::AccountAlias);
        }
        if input.terminal_denominator == 0 {
            return Err(Lbv2TerminalErrorV2::InvalidRational);
        }
        if input.debit_quantity == 0 {
            return Err(Lbv2TerminalErrorV2::InvalidQuantity);
        }
        require_next_revision(input.pre_market_revision, input.post_market_revision)?;
        require_next_revision(input.pre_position_revision, input.post_position_revision)?;
        let paid = input.evaluated_payout != 0;
        if paid {
            require_nonzero(input.custody_request_digest)?;
            require_next_revision(input.pre_custody_revision, input.post_custody_revision)?;
        } else if !is_zero(input.custody_request_digest)
            || input.pre_custody_revision != LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2
            || input.post_custody_revision != LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2
        {
            return Err(Lbv2TerminalErrorV2::InvalidCustodyShape);
        }
        Ok(Self(input))
    }

    /// Decode one exact request and refuse every noncanonical byte.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != LBV2_TERMINAL_REQUEST_BYTES_V2 {
            return Err(Lbv2TerminalErrorV2::InvalidLength);
        }
        if array_at::<8>(input, 0)? != LBV2_TERMINAL_REQUEST_MAGIC_V2 {
            return Err(Lbv2TerminalErrorV2::InvalidMagic);
        }
        if u16_at(input, VERSION_OFFSET)? != LBV2_TERMINAL_WIRE_VERSION_V2 {
            return Err(Lbv2TerminalErrorV2::UnsupportedVersion);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 6)?;
        Self::new(Lbv2TerminalRedeemRequestInputV2 {
            release_set: array_at(input, RELEASE_SET_OFFSET)?,
            market: array_at(input, MARKET_OFFSET)?,
            product_record_digest: array_at(input, PRODUCT_RECORD_OFFSET)?,
            semantic_product_id: array_at(input, SEMANTIC_PRODUCT_OFFSET)?,
            semantic_basis_id: array_at(input, SEMANTIC_BASIS_OFFSET)?,
            linked_basis_record_digest: array_at(input, LINKED_BASIS_RECORD_OFFSET)?,
            terminal_coordinate_digest: array_at(input, TERMINAL_COORDINATE_DIGEST_OFFSET)?,
            owner: array_at(input, OWNER_OFFSET)?,
            protocol_position: array_at(input, PROTOCOL_POSITION_OFFSET)?,
            claims_program: array_at(input, CLAIMS_PROGRAM_OFFSET)?,
            custody_request_digest: array_at(input, CUSTODY_REQUEST_DIGEST_OFFSET)?,
            candidate_digest: array_at(input, CANDIDATE_DIGEST_OFFSET)?,
            terminal_numerator: i64_at(input, TERMINAL_NUMERATOR_OFFSET)?,
            terminal_denominator: u32_at(input, TERMINAL_DENOMINATOR_OFFSET)?,
            claim_index: u32_at(input, CLAIM_INDEX_OFFSET)?,
            pre_market_revision: u64_at(input, PRE_MARKET_REVISION_OFFSET)?,
            post_market_revision: u64_at(input, POST_MARKET_REVISION_OFFSET)?,
            pre_position_revision: u64_at(input, PRE_POSITION_REVISION_OFFSET)?,
            post_position_revision: u64_at(input, POST_POSITION_REVISION_OFFSET)?,
            debit_quantity: u64_at(input, DEBIT_QUANTITY_OFFSET)?,
            evaluated_payout: u64_at(input, EVALUATED_PAYOUT_OFFSET)?,
            pre_custody_revision: u64_at(input, PRE_CUSTODY_REVISION_OFFSET)?,
            post_custody_revision: u64_at(input, POST_CUSTODY_REVISION_OFFSET)?,
        })
    }

    /// Encode the exact fixed request bytes.
    pub fn to_bytes(self) -> [u8; LBV2_TERMINAL_REQUEST_BYTES_V2] {
        let mut output = [0_u8; LBV2_TERMINAL_REQUEST_BYTES_V2];
        put_infallible(&mut output, 0, &LBV2_TERMINAL_REQUEST_MAGIC_V2);
        put_infallible(
            &mut output,
            VERSION_OFFSET,
            &LBV2_TERMINAL_WIRE_VERSION_V2.to_le_bytes(),
        );
        for (offset, identity) in [
            (RELEASE_SET_OFFSET, self.0.release_set),
            (MARKET_OFFSET, self.0.market),
            (PRODUCT_RECORD_OFFSET, self.0.product_record_digest),
            (SEMANTIC_PRODUCT_OFFSET, self.0.semantic_product_id),
            (SEMANTIC_BASIS_OFFSET, self.0.semantic_basis_id),
            (
                LINKED_BASIS_RECORD_OFFSET,
                self.0.linked_basis_record_digest,
            ),
            (
                TERMINAL_COORDINATE_DIGEST_OFFSET,
                self.0.terminal_coordinate_digest,
            ),
            (OWNER_OFFSET, self.0.owner),
            (PROTOCOL_POSITION_OFFSET, self.0.protocol_position),
            (CLAIMS_PROGRAM_OFFSET, self.0.claims_program),
            (CUSTODY_REQUEST_DIGEST_OFFSET, self.0.custody_request_digest),
            (CANDIDATE_DIGEST_OFFSET, self.0.candidate_digest),
        ] {
            put_infallible(&mut output, offset, &identity);
        }
        put_infallible(
            &mut output,
            TERMINAL_NUMERATOR_OFFSET,
            &self.0.terminal_numerator.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            TERMINAL_DENOMINATOR_OFFSET,
            &self.0.terminal_denominator.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            CLAIM_INDEX_OFFSET,
            &self.0.claim_index.to_le_bytes(),
        );
        for (offset, value) in [
            (PRE_MARKET_REVISION_OFFSET, self.0.pre_market_revision),
            (POST_MARKET_REVISION_OFFSET, self.0.post_market_revision),
            (PRE_POSITION_REVISION_OFFSET, self.0.pre_position_revision),
            (POST_POSITION_REVISION_OFFSET, self.0.post_position_revision),
            (DEBIT_QUANTITY_OFFSET, self.0.debit_quantity),
            (EVALUATED_PAYOUT_OFFSET, self.0.evaluated_payout),
            (PRE_CUSTODY_REVISION_OFFSET, self.0.pre_custody_revision),
            (POST_CUSTODY_REVISION_OFFSET, self.0.post_custody_revision),
        ] {
            put_infallible(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    /// Return the immutable current release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.0.release_set
    }

    /// Return the logical Core Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.0.market
    }

    /// Return the finalized Product-record digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.0.product_record_digest
    }

    /// Return the semantic Product identity.
    pub const fn semantic_product_id(self) -> [u8; 32] {
        self.0.semantic_product_id
    }

    /// Return the semantic LiabilityBasisV2 identity.
    pub const fn semantic_basis_id(self) -> [u8; 32] {
        self.0.semantic_basis_id
    }

    /// Return the finalized linked-basis record digest.
    pub const fn linked_basis_record_digest(self) -> [u8; 32] {
        self.0.linked_basis_record_digest
    }

    /// Return the finalized terminal-coordinate record digest.
    pub const fn terminal_coordinate_digest(self) -> [u8; 32] {
        self.0.terminal_coordinate_digest
    }

    /// Return the exact signed rational numerator.
    pub const fn terminal_numerator(self) -> i64 {
        self.0.terminal_numerator
    }

    /// Return the exact positive rational denominator.
    pub const fn terminal_denominator(self) -> u32 {
        self.0.terminal_denominator
    }

    /// Return the selected Product claim coordinate.
    pub const fn claim_index(self) -> u32 {
        self.0.claim_index
    }

    /// Return the holder identity whose claim is debited.
    pub const fn owner(self) -> [u8; 32] {
        self.0.owner
    }

    /// Return the canonical Claims ProtocolPosition account.
    pub const fn protocol_position(self) -> [u8; 32] {
        self.0.protocol_position
    }

    /// Return the Claims aggregate pre-revision.
    pub const fn pre_market_revision(self) -> u64 {
        self.0.pre_market_revision
    }

    /// Return the Claims aggregate post-revision.
    pub const fn post_market_revision(self) -> u64 {
        self.0.post_market_revision
    }

    /// Return the ProtocolPosition pre-revision.
    pub const fn pre_position_revision(self) -> u64 {
        self.0.pre_position_revision
    }

    /// Return the ProtocolPosition post-revision.
    pub const fn post_position_revision(self) -> u64 {
        self.0.post_position_revision
    }

    /// Return the exact native claim debit quantity.
    pub const fn debit_quantity(self) -> u64 {
        self.0.debit_quantity
    }

    /// Return the exact evaluated collateral payout.
    pub const fn evaluated_payout(self) -> u64 {
        self.0.evaluated_payout
    }

    /// Return the selected Claims program.
    pub const fn claims_program(self) -> [u8; 32] {
        self.0.claims_program
    }

    /// Return the exact Custody request digest, or zero for zero payout.
    pub const fn custody_request_digest(self) -> [u8; 32] {
        self.0.custody_request_digest
    }

    /// Return the Custody replay pre-revision or absent sentinel.
    pub const fn pre_custody_revision(self) -> u64 {
        self.0.pre_custody_revision
    }

    /// Return the Custody replay post-revision or absent sentinel.
    pub const fn post_custody_revision(self) -> u64 {
        self.0.post_custody_revision
    }

    /// Return the exact candidate aggregate+Position digest.
    pub const fn candidate_digest(self) -> [u8; 32] {
        self.0.candidate_digest
    }
}

/// Fixed terminal-redemption receipt embedding the exact accepted request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Lbv2TerminalRedeemReceiptV2 {
    request: Lbv2TerminalRedeemRequestV2,
    request_digest: [u8; 32],
    custody_receipt_digest: [u8; 32],
    custody_replay_digest: [u8; 32],
    post_resource_digest: [u8; 32],
}

impl Lbv2TerminalRedeemReceiptV2 {
    /// Construct one receipt after all exact child postconditions succeed.
    pub fn new(
        request: Lbv2TerminalRedeemRequestV2,
        request_digest: [u8; 32],
        custody_receipt_digest: [u8; 32],
        custody_replay_digest: [u8; 32],
        post_resource_digest: [u8; 32],
    ) -> Result<Self> {
        require_nonzero(request_digest)?;
        require_nonzero(post_resource_digest)?;
        if request.evaluated_payout() == 0 {
            if !is_zero(custody_receipt_digest) || !is_zero(custody_replay_digest) {
                return Err(Lbv2TerminalErrorV2::InvalidCustodyShape);
            }
        } else {
            require_nonzero(custody_receipt_digest)?;
            require_nonzero(custody_replay_digest)?;
        }
        Ok(Self {
            request,
            request_digest,
            custody_receipt_digest,
            custody_replay_digest,
            post_resource_digest,
        })
    }

    /// Decode one exact receipt and its embedded canonical request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != LBV2_TERMINAL_RECEIPT_BYTES_V2 {
            return Err(Lbv2TerminalErrorV2::InvalidLength);
        }
        if array_at::<8>(input, 0)? != LBV2_TERMINAL_RECEIPT_MAGIC_V2 {
            return Err(Lbv2TerminalErrorV2::InvalidMagic);
        }
        if u16_at(input, VERSION_OFFSET)? != LBV2_TERMINAL_WIRE_VERSION_V2 {
            return Err(Lbv2TerminalErrorV2::UnsupportedVersion);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 6)?;
        Self::new(
            Lbv2TerminalRedeemRequestV2::decode(subslice(
                input,
                RECEIPT_REQUEST_OFFSET,
                LBV2_TERMINAL_REQUEST_BYTES_V2,
            )?)?,
            array_at(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            array_at(input, RECEIPT_CUSTODY_RECEIPT_DIGEST_OFFSET)?,
            array_at(input, RECEIPT_CUSTODY_REPLAY_DIGEST_OFFSET)?,
            array_at(input, RECEIPT_POST_RESOURCE_DIGEST_OFFSET)?,
        )
    }

    /// Encode the exact fixed receipt bytes.
    pub fn to_bytes(self) -> [u8; LBV2_TERMINAL_RECEIPT_BYTES_V2] {
        let mut output = [0_u8; LBV2_TERMINAL_RECEIPT_BYTES_V2];
        put_infallible(&mut output, 0, &LBV2_TERMINAL_RECEIPT_MAGIC_V2);
        put_infallible(
            &mut output,
            VERSION_OFFSET,
            &LBV2_TERMINAL_WIRE_VERSION_V2.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_REQUEST_OFFSET,
            &self.request.to_bytes(),
        );
        for (offset, digest) in [
            (RECEIPT_REQUEST_DIGEST_OFFSET, self.request_digest),
            (
                RECEIPT_CUSTODY_RECEIPT_DIGEST_OFFSET,
                self.custody_receipt_digest,
            ),
            (
                RECEIPT_CUSTODY_REPLAY_DIGEST_OFFSET,
                self.custody_replay_digest,
            ),
            (
                RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
                self.post_resource_digest,
            ),
        ] {
            put_infallible(&mut output, offset, &digest);
        }
        output
    }

    /// Require this receipt to bind one exact accepted request and byte digest.
    pub fn verify_for(
        self,
        request: Lbv2TerminalRedeemRequestV2,
        request_digest: [u8; 32],
    ) -> Result<()> {
        if self.request != request || self.request_digest != request_digest {
            return Err(Lbv2TerminalErrorV2::ReceiptMismatch);
        }
        Ok(())
    }

    /// Return the exact embedded request.
    pub const fn request(self) -> Lbv2TerminalRedeemRequestV2 {
        self.request
    }

    /// Return the SHA-256 digest of exact request bytes.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }

    /// Return the exact Custody receipt digest, or zero for zero payout.
    pub const fn custody_receipt_digest(self) -> [u8; 32] {
        self.custody_receipt_digest
    }

    /// Return the exact post-Custody replay digest, or zero for zero payout.
    pub const fn custody_replay_digest(self) -> [u8; 32] {
        self.custody_replay_digest
    }

    /// Return the digest of exact post aggregate, Position, Custody, and replay resources.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }
}

fn require_next_revision(before: u64, after: u64) -> Result<()> {
    if before == u64::MAX || before.checked_add(1) != Some(after) {
        Err(Lbv2TerminalErrorV2::InvalidRevision)
    } else {
        Ok(())
    }
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if is_zero(value) {
        Err(Lbv2TerminalErrorV2::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn is_zero(value: [u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if subslice(input, offset, width)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Lbv2TerminalErrorV2::NonCanonical)
    }
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

fn i64_at(input: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(input, offset, N)?
        .try_into()
        .map_err(|_| Lbv2TerminalErrorV2::InvalidLength)
}

fn subslice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(Lbv2TerminalErrorV2::InvalidLength)?,
        )
        .ok_or(Lbv2TerminalErrorV2::InvalidLength)
}

fn put_infallible(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn paid_input() -> Lbv2TerminalRedeemRequestInputV2 {
        Lbv2TerminalRedeemRequestInputV2 {
            release_set: id(1),
            market: id(2),
            product_record_digest: id(3),
            semantic_product_id: id(4),
            semantic_basis_id: id(5),
            linked_basis_record_digest: id(6),
            terminal_coordinate_digest: id(7),
            owner: id(8),
            protocol_position: id(9),
            claims_program: id(10),
            custody_request_digest: id(11),
            candidate_digest: id(12),
            terminal_numerator: -17,
            terminal_denominator: 13,
            claim_index: 3,
            pre_market_revision: 20,
            post_market_revision: 21,
            pre_position_revision: 30,
            post_position_revision: 31,
            debit_quantity: 7,
            evaluated_payout: 35,
            pre_custody_revision: 40,
            post_custody_revision: 41,
        }
    }

    fn paid_request() -> Lbv2TerminalRedeemRequestV2 {
        Lbv2TerminalRedeemRequestV2::new(paid_input()).expect("request")
    }

    #[test]
    fn paid_request_and_receipt_roundtrip_bind_every_exact_evidence_digest() {
        let request = paid_request();
        let request_bytes = request.to_bytes();
        assert_eq!(
            Lbv2TerminalRedeemRequestV2::decode(&request_bytes),
            Ok(request)
        );
        assert_eq!(request.release_set(), id(1));
        assert_eq!(request.market(), id(2));
        assert_eq!(request.product_record_digest(), id(3));
        assert_eq!(request.semantic_product_id(), id(4));
        assert_eq!(request.semantic_basis_id(), id(5));
        assert_eq!(request.linked_basis_record_digest(), id(6));
        assert_eq!(request.terminal_coordinate_digest(), id(7));
        assert_eq!(
            (request.terminal_numerator(), request.terminal_denominator()),
            (-17, 13)
        );
        assert_eq!(request.claim_index(), 3);
        assert_eq!(
            (request.owner(), request.protocol_position()),
            (id(8), id(9))
        );
        assert_eq!(
            (
                request.pre_market_revision(),
                request.post_market_revision()
            ),
            (20, 21)
        );
        assert_eq!(
            (
                request.pre_position_revision(),
                request.post_position_revision()
            ),
            (30, 31)
        );
        assert_eq!(request.debit_quantity(), 7);
        assert_eq!(request.evaluated_payout(), 35);
        assert_eq!(request.claims_program(), id(10));
        assert_eq!(request.custody_request_digest(), id(11));
        assert_eq!(
            (
                request.pre_custody_revision(),
                request.post_custody_revision()
            ),
            (40, 41)
        );
        assert_eq!(request.candidate_digest(), id(12));

        let receipt = Lbv2TerminalRedeemReceiptV2::new(request, id(13), id(14), id(15), id(16))
            .expect("receipt");
        let receipt_bytes = receipt.to_bytes();
        assert_eq!(
            Lbv2TerminalRedeemReceiptV2::decode(&receipt_bytes),
            Ok(receipt)
        );
        receipt.verify_for(request, id(13)).expect("exact join");
        assert_eq!(receipt.request(), request);
        assert_eq!(receipt.request_digest(), id(13));
        assert_eq!(receipt.custody_receipt_digest(), id(14));
        assert_eq!(receipt.custody_replay_digest(), id(15));
        assert_eq!(receipt.post_resource_digest(), id(16));
    }

    #[test]
    fn zero_payout_is_exactly_custody_free() {
        let request = Lbv2TerminalRedeemRequestV2::new(Lbv2TerminalRedeemRequestInputV2 {
            evaluated_payout: 0,
            custody_request_digest: [0; 32],
            pre_custody_revision: LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
            post_custody_revision: LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
            ..paid_input()
        })
        .expect("zero payout");
        let receipt = Lbv2TerminalRedeemReceiptV2::new(request, id(13), [0; 32], [0; 32], id(16))
            .expect("custody-free receipt");
        assert_eq!(
            Lbv2TerminalRedeemReceiptV2::decode(&receipt.to_bytes()),
            Ok(receipt)
        );

        assert_eq!(
            Lbv2TerminalRedeemRequestV2::new(Lbv2TerminalRedeemRequestInputV2 {
                evaluated_payout: 0,
                pre_custody_revision: LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
                post_custody_revision: LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
                ..paid_input()
            }),
            Err(Lbv2TerminalErrorV2::InvalidCustodyShape)
        );
        assert_eq!(
            Lbv2TerminalRedeemReceiptV2::new(request, id(13), id(14), [0; 32], id(16)),
            Err(Lbv2TerminalErrorV2::InvalidCustodyShape)
        );
    }

    #[test]
    fn hostile_identity_rational_quantity_revision_and_custody_shapes_refuse() {
        for hostile in [
            Lbv2TerminalRedeemRequestInputV2 {
                product_record_digest: [0; 32],
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                protocol_position: id(8),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                terminal_denominator: 0,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                debit_quantity: 0,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                post_market_revision: 22,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                pre_position_revision: u64::MAX,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                custody_request_digest: [0; 32],
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                post_custody_revision: 42,
                ..paid_input()
            },
        ] {
            assert!(Lbv2TerminalRedeemRequestV2::new(hostile).is_err());
        }
    }

    #[test]
    fn hostile_wire_and_same_shape_request_substitution_refuse() {
        let request = paid_request();
        let bytes = request.to_bytes();
        assert_eq!(
            Lbv2TerminalRedeemRequestV2::decode(bytes.get(..bytes.len() - 1).expect("truncated")),
            Err(Lbv2TerminalErrorV2::InvalidLength)
        );
        let mut hostile_magic = bytes;
        *hostile_magic.get_mut(0).expect("magic") ^= 1;
        assert_eq!(
            Lbv2TerminalRedeemRequestV2::decode(&hostile_magic),
            Err(Lbv2TerminalErrorV2::InvalidMagic)
        );
        let mut hostile_reserved = bytes;
        *hostile_reserved.get_mut(10).expect("reserved") = 1;
        assert_eq!(
            Lbv2TerminalRedeemRequestV2::decode(&hostile_reserved),
            Err(Lbv2TerminalErrorV2::NonCanonical)
        );

        let receipt = Lbv2TerminalRedeemReceiptV2::new(request, id(13), id(14), id(15), id(16))
            .expect("receipt");
        for substituted in [
            Lbv2TerminalRedeemRequestInputV2 {
                release_set: id(28),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                market: id(29),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                product_record_digest: id(30),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                semantic_product_id: id(31),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                semantic_basis_id: id(32),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                linked_basis_record_digest: id(33),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                terminal_coordinate_digest: id(34),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                terminal_numerator: -16,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                terminal_denominator: 14,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                claim_index: 4,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                owner: id(35),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                protocol_position: id(36),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                debit_quantity: 8,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                pre_market_revision: 21,
                post_market_revision: 22,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                pre_position_revision: 31,
                post_position_revision: 32,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                evaluated_payout: 40,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                claims_program: id(37),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                custody_request_digest: id(38),
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                pre_custody_revision: 41,
                post_custody_revision: 42,
                ..paid_input()
            },
            Lbv2TerminalRedeemRequestInputV2 {
                candidate_digest: id(39),
                ..paid_input()
            },
        ] {
            let substituted =
                Lbv2TerminalRedeemRequestV2::new(substituted).expect("well formed substitution");
            assert_eq!(
                receipt.verify_for(substituted, id(13)),
                Err(Lbv2TerminalErrorV2::ReceiptMismatch)
            );
        }
        assert_eq!(
            receipt.verify_for(request, id(40)),
            Err(Lbv2TerminalErrorV2::ReceiptMismatch)
        );
    }

    #[test]
    fn hostile_receipt_wire_and_post_digests_refuse() {
        let request = paid_request();
        let receipt = Lbv2TerminalRedeemReceiptV2::new(request, id(13), id(14), id(15), id(16))
            .expect("receipt");
        let bytes = receipt.to_bytes();
        assert_eq!(
            Lbv2TerminalRedeemReceiptV2::decode(bytes.get(..bytes.len() - 1).expect("truncated")),
            Err(Lbv2TerminalErrorV2::InvalidLength)
        );
        let mut hostile_reserved = bytes;
        *hostile_reserved.get_mut(10).expect("reserved") = 1;
        assert_eq!(
            Lbv2TerminalRedeemReceiptV2::decode(&hostile_reserved),
            Err(Lbv2TerminalErrorV2::NonCanonical)
        );
        for hostile in [
            Lbv2TerminalRedeemReceiptV2::new(request, [0; 32], id(14), id(15), id(16)),
            Lbv2TerminalRedeemReceiptV2::new(request, id(13), [0; 32], id(15), id(16)),
            Lbv2TerminalRedeemReceiptV2::new(request, id(13), id(14), [0; 32], id(16)),
            Lbv2TerminalRedeemReceiptV2::new(request, id(13), id(14), id(15), [0; 32]),
        ] {
            assert!(hostile.is_err());
        }
    }
}
