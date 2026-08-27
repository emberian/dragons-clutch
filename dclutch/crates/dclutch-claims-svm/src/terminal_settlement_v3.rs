//! Family-neutral terminal Claims settlement and Custody evidence ABI.
//!
//! This is the sole wire authority between an authenticated orchestration
//! caller and Claims for terminal settlement. Product result evaluation,
//! SignedDelta mutation, and Custody payout remain independently rederived by
//! the Claims SVM adapter. No family request, descriptor action, payout input,
//! or caller-authored matrix is carried here.

use crate::CallerRole;

/// Exact terminal-settlement request width.
pub const TERMINAL_SETTLEMENT_REQUEST_BYTES_V3: usize = 640;
/// Exact terminal-settlement receipt width.
pub const TERMINAL_SETTLEMENT_RECEIPT_BYTES_V3: usize = 1008;
/// Terminal-settlement request magic.
pub const TERMINAL_SETTLEMENT_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLTSQ03";
/// Terminal-settlement receipt magic.
pub const TERMINAL_SETTLEMENT_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCLTSA03";
/// Implemented wire version.
pub const TERMINAL_SETTLEMENT_VERSION_V3: u16 = 3;
/// Exact account count of the family-neutral Claims terminal child.
pub const TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3: usize = 35;
/// Exact canonical SignedDeltaV3 prefix for one Position.
pub const TERMINAL_SETTLEMENT_SIGNED_DELTA_ACCOUNTS_V3: usize = 21;
/// Finalized exposure raw-record account index.
pub const TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3: usize = 21;
/// Vacant finalized-exposure staging account index.
pub const TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3: usize = 22;
/// Claims-derived Custody caller PDA account index.
pub const TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3: usize = 23;
/// Registry-selected Custody program account index.
pub const TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3: usize = 24;
/// Optional finalized rational terminal-coordinate raw account index.
pub const TERMINAL_SETTLEMENT_COORDINATE_ACCOUNT_V3: usize = 25;
/// Optional terminal-coordinate staging account index.
pub const TERMINAL_SETTLEMENT_COORDINATE_STAGING_ACCOUNT_V3: usize = 26;
/// Finalized Realm raw-record account index.
pub const TERMINAL_SETTLEMENT_REALM_ACCOUNT_V3: usize = 27;
/// Vacant Realm staging account index.
pub const TERMINAL_SETTLEMENT_REALM_STAGING_ACCOUNT_V3: usize = 28;
/// Canonical Custody replay account index.
pub const TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3: usize = 29;
/// Realm collateral Mint account index.
pub const TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3: usize = 30;
/// Canonical Custody Hoard account index.
pub const TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3: usize = 31;
/// External recipient token account index.
pub const TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3: usize = 32;
/// Canonical Custody transfer authority account index.
pub const TERMINAL_SETTLEMENT_CUSTODY_AUTHORITY_ACCOUNT_V3: usize = 33;
/// Realm-selected Token program account index.
pub const TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3: usize = 34;
/// Domain separating a terminal Custody candidate from all caller candidates.
pub const TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3: &[u8] =
    b"dclutch/claims-terminal-custody-candidate/v3";
/// Domain committing the exact post-CPI hoard and recipient Token bytes.
pub const TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3: &[u8] =
    b"dclutch/claims-terminal-token-poststate/v3";
/// Domain committing all Claims and optional Custody postresources.
pub const TERMINAL_SETTLEMENT_POST_RESOURCE_DOMAIN_V3: &[u8] =
    b"dclutch/claims-terminal-postresources/v3";

const ROLE_OFFSET: usize = 10;
const RELEASE_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const REALM_OFFSET: usize = 80;
const PARENT_CONTEXT_OFFSET: usize = 112;
const PRODUCT_RECORD_OFFSET: usize = 144;
const EXPOSURE_ID_OFFSET: usize = 176;
const EXPOSURE_DIGEST_OFFSET: usize = 208;
const TERMINAL_RECORD_OFFSET: usize = 240;
const OWNER_OFFSET: usize = 272;
const POSITION_OFFSET: usize = 304;
const RECIPIENT_OWNER_OFFSET: usize = 336;
const RECIPIENT_TOKEN_OFFSET: usize = 368;
const CLAIMS_PROGRAM_OFFSET: usize = 400;
const CUSTODY_PROGRAM_OFFSET: usize = 432;
const COLLATERAL_MINT_OFFSET: usize = 464;
const TOKEN_PROGRAM_OFFSET: usize = 496;
const SEMANTIC_BASIS_OFFSET: usize = 528;
const LINKED_BASIS_OFFSET: usize = 560;
const GENERATION_OFFSET: usize = 592;
const MARKET_REVISION_OFFSET: usize = 600;
const POSITION_REVISION_OFFSET: usize = 608;
const CUSTODY_REVISION_OFFSET: usize = 616;
const QUANTITY_OFFSET: usize = 624;
const CLAIM_INDEX_OFFSET: usize = 632;
const TRANSFER_INDEX_OFFSET: usize = 636;

const RECEIPT_REQUEST_OFFSET: usize = 16;
const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 656;
const RECEIPT_SIGNED_PACKET_OFFSET: usize = 688;
const RECEIPT_SIGNED_TABLE_OFFSET: usize = 720;
const RECEIPT_SIGNED_POST_OFFSET: usize = 752;
const RECEIPT_CUSTODY_REQUEST_OFFSET: usize = 784;
const RECEIPT_CUSTODY_RECEIPT_OFFSET: usize = 816;
const RECEIPT_CUSTODY_REPLAY_OFFSET: usize = 848;
const RECEIPT_CUSTODY_TOKEN_POST_OFFSET: usize = 880;
const RECEIPT_POST_RESOURCE_OFFSET: usize = 912;
const RECEIPT_PAYOUT_OFFSET: usize = 944;
const RECEIPT_PRE_MARKET_OFFSET: usize = 952;
const RECEIPT_POST_MARKET_OFFSET: usize = 960;
const RECEIPT_PRE_POSITION_OFFSET: usize = 968;
const RECEIPT_POST_POSITION_OFFSET: usize = 976;
const RECEIPT_PRE_CUSTODY_OFFSET: usize = 984;
const RECEIPT_POST_CUSTODY_OFFSET: usize = 992;

/// Stable hostile-decode or evidence refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalSettlementErrorV3 {
    /// Input had another width.
    InvalidLength,
    /// Magic selected another request or receipt.
    InvalidMagic,
    /// Wire version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes or a tag were noncanonical.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// Account identities aliased where distinct resources are required.
    AccountAlias,
    /// Quantity, coordinate, or revision was invalid.
    InvalidCoordinate,
    /// Positive/zero payout and Custody evidence disagreed.
    InvalidCustodyShape,
    /// Receipt did not bind the exact request and physical commitments.
    ReceiptMismatch,
}

/// Result alias for terminal settlement ABI operations.
pub type Result<T> = core::result::Result<T, TerminalSettlementErrorV3>;

/// Construction input for one family-neutral terminal settlement request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSettlementRequestInputV3 {
    /// Registry role of the authenticated orchestration caller.
    pub caller_role: CallerRole,
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Realm selecting collateral.
    pub realm: [u8; 32],
    /// Caller-owned replay/capability context.
    pub parent_context: [u8; 32],
    /// Finalized Product graph-root digest.
    pub product_record_digest: [u8; 32],
    /// Logical finalized Product-to-Claims exposure identity.
    pub exposure_id: [u8; 32],
    /// SHA-256 of exact finalized exposure bytes.
    pub exposure_digest: [u8; 32],
    /// Exact Core terminal receipt/coordinate digest.
    pub terminal_record_digest: [u8; 32],
    /// Sole Claims Position owner debited.
    pub owner: [u8; 32],
    /// Canonical Claims Position account.
    pub position: [u8; 32],
    /// External owner receiving collateral.
    pub recipient_owner: [u8; 32],
    /// Exact external collateral token account.
    pub recipient_token_account: [u8; 32],
    /// Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// Registry-selected Custody program.
    pub custody_program: [u8; 32],
    /// Realm-selected collateral Mint.
    pub collateral_mint: [u8; 32],
    /// Realm-selected Token program.
    pub token_program: [u8; 32],
    /// Claims semantic basis persisted by LBV2.
    pub semantic_basis_id: [u8; 32],
    /// Finalized ProductBasisV3 raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Optimistic Claims aggregate revision.
    pub expected_market_revision: u64,
    /// Optimistic Claims Position revision.
    pub expected_position_revision: u64,
    /// Optimistic Custody replay revision, also observed unchanged for zero payout.
    pub expected_custody_revision: u64,
    /// Positive native Claims atoms debited.
    pub quantity: u64,
    /// Product-to-Claims translated coordinate debited.
    pub claim_index: u32,
    /// Ordered parent effect coordinate used by Custody.
    pub transfer_index: u16,
}

/// Exact hostile-decodable terminal settlement request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSettlementRequestV3(TerminalSettlementRequestInputV3);

impl TerminalSettlementRequestV3 {
    /// Construct and fully validate one request.
    pub fn new(input: TerminalSettlementRequestInputV3) -> Result<Self> {
        for identity in [
            input.release_set,
            input.market,
            input.realm,
            input.parent_context,
            input.product_record_digest,
            input.exposure_id,
            input.exposure_digest,
            input.terminal_record_digest,
            input.owner,
            input.position,
            input.recipient_owner,
            input.recipient_token_account,
            input.claims_program,
            input.custody_program,
            input.collateral_mint,
            input.token_program,
            input.semantic_basis_id,
            input.linked_basis_record_digest,
        ] {
            nonzero(identity)?;
        }
        if input.owner == input.position
            || input.recipient_owner == input.recipient_token_account
            || input.claims_program == input.custody_program
            || input.quantity == 0
            || input.expected_market_revision == u64::MAX
            || input.expected_position_revision == u64::MAX
            || input.expected_custody_revision == u64::MAX
        {
            return Err(TerminalSettlementErrorV3::InvalidCoordinate);
        }
        Ok(Self(input))
    }

    /// Decode one exact request and refuse every noncanonical byte.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(
            bytes,
            TERMINAL_SETTLEMENT_REQUEST_BYTES_V3,
            TERMINAL_SETTLEMENT_REQUEST_MAGIC_V3,
        )?;
        require_zero(bytes, 11, 5)?;
        require_zero(bytes, 638, 2)?;
        let caller_role = match byte(bytes, ROLE_OFFSET)? {
            0 => CallerRole::Core,
            2 => CallerRole::Trading,
            _ => return Err(TerminalSettlementErrorV3::NonCanonical),
        };
        Self::new(TerminalSettlementRequestInputV3 {
            caller_role,
            release_set: array(bytes, RELEASE_OFFSET)?,
            market: array(bytes, MARKET_OFFSET)?,
            realm: array(bytes, REALM_OFFSET)?,
            parent_context: array(bytes, PARENT_CONTEXT_OFFSET)?,
            product_record_digest: array(bytes, PRODUCT_RECORD_OFFSET)?,
            exposure_id: array(bytes, EXPOSURE_ID_OFFSET)?,
            exposure_digest: array(bytes, EXPOSURE_DIGEST_OFFSET)?,
            terminal_record_digest: array(bytes, TERMINAL_RECORD_OFFSET)?,
            owner: array(bytes, OWNER_OFFSET)?,
            position: array(bytes, POSITION_OFFSET)?,
            recipient_owner: array(bytes, RECIPIENT_OWNER_OFFSET)?,
            recipient_token_account: array(bytes, RECIPIENT_TOKEN_OFFSET)?,
            claims_program: array(bytes, CLAIMS_PROGRAM_OFFSET)?,
            custody_program: array(bytes, CUSTODY_PROGRAM_OFFSET)?,
            collateral_mint: array(bytes, COLLATERAL_MINT_OFFSET)?,
            token_program: array(bytes, TOKEN_PROGRAM_OFFSET)?,
            semantic_basis_id: array(bytes, SEMANTIC_BASIS_OFFSET)?,
            linked_basis_record_digest: array(bytes, LINKED_BASIS_OFFSET)?,
            generation: u64_at(bytes, GENERATION_OFFSET)?,
            expected_market_revision: u64_at(bytes, MARKET_REVISION_OFFSET)?,
            expected_position_revision: u64_at(bytes, POSITION_REVISION_OFFSET)?,
            expected_custody_revision: u64_at(bytes, CUSTODY_REVISION_OFFSET)?,
            quantity: u64_at(bytes, QUANTITY_OFFSET)?,
            claim_index: u32_at(bytes, CLAIM_INDEX_OFFSET)?,
            transfer_index: u16_at(bytes, TRANSFER_INDEX_OFFSET)?,
        })
    }

    /// Encode the canonical fixed request.
    pub fn to_bytes(self) -> [u8; TERMINAL_SETTLEMENT_REQUEST_BYTES_V3] {
        let mut out = [0_u8; TERMINAL_SETTLEMENT_REQUEST_BYTES_V3];
        put(&mut out, 0, &TERMINAL_SETTLEMENT_REQUEST_MAGIC_V3);
        put(&mut out, 8, &TERMINAL_SETTLEMENT_VERSION_V3.to_le_bytes());
        out[ROLE_OFFSET] = self.0.caller_role as u8;
        for (offset, value) in [
            (RELEASE_OFFSET, self.0.release_set),
            (MARKET_OFFSET, self.0.market),
            (REALM_OFFSET, self.0.realm),
            (PARENT_CONTEXT_OFFSET, self.0.parent_context),
            (PRODUCT_RECORD_OFFSET, self.0.product_record_digest),
            (EXPOSURE_ID_OFFSET, self.0.exposure_id),
            (EXPOSURE_DIGEST_OFFSET, self.0.exposure_digest),
            (TERMINAL_RECORD_OFFSET, self.0.terminal_record_digest),
            (OWNER_OFFSET, self.0.owner),
            (POSITION_OFFSET, self.0.position),
            (RECIPIENT_OWNER_OFFSET, self.0.recipient_owner),
            (RECIPIENT_TOKEN_OFFSET, self.0.recipient_token_account),
            (CLAIMS_PROGRAM_OFFSET, self.0.claims_program),
            (CUSTODY_PROGRAM_OFFSET, self.0.custody_program),
            (COLLATERAL_MINT_OFFSET, self.0.collateral_mint),
            (TOKEN_PROGRAM_OFFSET, self.0.token_program),
            (SEMANTIC_BASIS_OFFSET, self.0.semantic_basis_id),
            (LINKED_BASIS_OFFSET, self.0.linked_basis_record_digest),
        ] {
            put(&mut out, offset, &value);
        }
        for (offset, value) in [
            (GENERATION_OFFSET, self.0.generation),
            (MARKET_REVISION_OFFSET, self.0.expected_market_revision),
            (POSITION_REVISION_OFFSET, self.0.expected_position_revision),
            (CUSTODY_REVISION_OFFSET, self.0.expected_custody_revision),
            (QUANTITY_OFFSET, self.0.quantity),
        ] {
            put(&mut out, offset, &value.to_le_bytes());
        }
        put(
            &mut out,
            CLAIM_INDEX_OFFSET,
            &self.0.claim_index.to_le_bytes(),
        );
        put(
            &mut out,
            TRANSFER_INDEX_OFFSET,
            &self.0.transfer_index.to_le_bytes(),
        );
        out
    }

    /// Return all checked request fields.
    pub const fn input(self) -> TerminalSettlementRequestInputV3 {
        self.0
    }
}

/// Exact physical commitments produced by one accepted settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSettlementReceiptInputV3 {
    /// Digest of the complete request.
    pub request_digest: [u8; 32],
    /// Digest of the exact canonical SignedDelta packet.
    pub signed_packet_digest: [u8; 32],
    /// Digest of the ordered SignedDelta tables.
    pub signed_table_digest: [u8; 32],
    /// Digest of Claims aggregate followed by Position poststate.
    pub signed_post_resource_digest: [u8; 32],
    /// Digest of the exact Custody V1 request, zero only for zero payout.
    pub custody_request_digest: [u8; 32],
    /// Digest of the exact Custody V1 receipt, zero only for zero payout.
    pub custody_receipt_digest: [u8; 32],
    /// Digest of authenticated Custody replay poststate.
    pub custody_replay_digest: [u8; 32],
    /// Digest of hoard and recipient Token poststate.
    pub custody_token_poststate_digest: [u8; 32],
    /// Domain-separated digest of all accepted postresources.
    pub post_resource_digest: [u8; 32],
    /// Exact ProductBasis/exposure-evaluated collateral atoms.
    pub payout: u64,
    /// Aggregate pre-revision.
    pub pre_market_revision: u64,
    /// Aggregate post-revision.
    pub post_market_revision: u64,
    /// Position pre-revision.
    pub pre_position_revision: u64,
    /// Position post-revision.
    pub post_position_revision: u64,
    /// Custody replay pre-revision.
    pub pre_custody_revision: u64,
    /// Custody replay post-revision, unchanged for zero payout.
    pub post_custody_revision: u64,
}

/// Fixed receipt embedding the sole accepted terminal-settlement request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSettlementReceiptV3 {
    request: TerminalSettlementRequestV3,
    evidence: TerminalSettlementReceiptInputV3,
}

impl TerminalSettlementReceiptV3 {
    /// Construct one receipt after every Claims, Custody, and Token postcondition.
    pub fn new(
        request: TerminalSettlementRequestV3,
        evidence: TerminalSettlementReceiptInputV3,
    ) -> Result<Self> {
        for digest in [
            evidence.request_digest,
            evidence.signed_packet_digest,
            evidence.signed_table_digest,
            evidence.signed_post_resource_digest,
            evidence.custody_replay_digest,
            evidence.custody_token_poststate_digest,
            evidence.post_resource_digest,
        ] {
            nonzero(digest)?;
        }
        let expected = request.input();
        if evidence.pre_market_revision != expected.expected_market_revision
            || evidence.pre_position_revision != expected.expected_position_revision
            || evidence.pre_custody_revision != expected.expected_custody_revision
            || evidence.post_market_revision
                != evidence
                    .pre_market_revision
                    .checked_add(1)
                    .ok_or(TerminalSettlementErrorV3::InvalidCoordinate)?
            || evidence.post_position_revision
                != evidence
                    .pre_position_revision
                    .checked_add(1)
                    .ok_or(TerminalSettlementErrorV3::InvalidCoordinate)?
        {
            return Err(TerminalSettlementErrorV3::ReceiptMismatch);
        }
        if evidence.payout == 0 {
            if !is_zero(evidence.custody_request_digest)
                || !is_zero(evidence.custody_receipt_digest)
                || evidence.post_custody_revision != evidence.pre_custody_revision
            {
                return Err(TerminalSettlementErrorV3::InvalidCustodyShape);
            }
        } else if is_zero(evidence.custody_request_digest)
            || is_zero(evidence.custody_receipt_digest)
            || evidence.post_custody_revision
                != evidence
                    .pre_custody_revision
                    .checked_add(1)
                    .ok_or(TerminalSettlementErrorV3::InvalidCoordinate)?
        {
            return Err(TerminalSettlementErrorV3::InvalidCustodyShape);
        }
        Ok(Self { request, evidence })
    }

    /// Decode and hostile-validate one exact receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(
            bytes,
            TERMINAL_SETTLEMENT_RECEIPT_BYTES_V3,
            TERMINAL_SETTLEMENT_RECEIPT_MAGIC_V3,
        )?;
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 1000, 8)?;
        let request = TerminalSettlementRequestV3::decode(slice(
            bytes,
            RECEIPT_REQUEST_OFFSET,
            TERMINAL_SETTLEMENT_REQUEST_BYTES_V3,
        )?)?;
        Self::new(
            request,
            TerminalSettlementReceiptInputV3 {
                request_digest: array(bytes, RECEIPT_REQUEST_DIGEST_OFFSET)?,
                signed_packet_digest: array(bytes, RECEIPT_SIGNED_PACKET_OFFSET)?,
                signed_table_digest: array(bytes, RECEIPT_SIGNED_TABLE_OFFSET)?,
                signed_post_resource_digest: array(bytes, RECEIPT_SIGNED_POST_OFFSET)?,
                custody_request_digest: array(bytes, RECEIPT_CUSTODY_REQUEST_OFFSET)?,
                custody_receipt_digest: array(bytes, RECEIPT_CUSTODY_RECEIPT_OFFSET)?,
                custody_replay_digest: array(bytes, RECEIPT_CUSTODY_REPLAY_OFFSET)?,
                custody_token_poststate_digest: array(bytes, RECEIPT_CUSTODY_TOKEN_POST_OFFSET)?,
                post_resource_digest: array(bytes, RECEIPT_POST_RESOURCE_OFFSET)?,
                payout: u64_at(bytes, RECEIPT_PAYOUT_OFFSET)?,
                pre_market_revision: u64_at(bytes, RECEIPT_PRE_MARKET_OFFSET)?,
                post_market_revision: u64_at(bytes, RECEIPT_POST_MARKET_OFFSET)?,
                pre_position_revision: u64_at(bytes, RECEIPT_PRE_POSITION_OFFSET)?,
                post_position_revision: u64_at(bytes, RECEIPT_POST_POSITION_OFFSET)?,
                pre_custody_revision: u64_at(bytes, RECEIPT_PRE_CUSTODY_OFFSET)?,
                post_custody_revision: u64_at(bytes, RECEIPT_POST_CUSTODY_OFFSET)?,
            },
        )
    }

    /// Encode the exact accepted request and every postcondition commitment.
    pub fn to_bytes(self) -> [u8; TERMINAL_SETTLEMENT_RECEIPT_BYTES_V3] {
        let mut out = [0_u8; TERMINAL_SETTLEMENT_RECEIPT_BYTES_V3];
        put(&mut out, 0, &TERMINAL_SETTLEMENT_RECEIPT_MAGIC_V3);
        put(&mut out, 8, &TERMINAL_SETTLEMENT_VERSION_V3.to_le_bytes());
        put(&mut out, RECEIPT_REQUEST_OFFSET, &self.request.to_bytes());
        for (offset, value) in [
            (RECEIPT_REQUEST_DIGEST_OFFSET, self.evidence.request_digest),
            (
                RECEIPT_SIGNED_PACKET_OFFSET,
                self.evidence.signed_packet_digest,
            ),
            (
                RECEIPT_SIGNED_TABLE_OFFSET,
                self.evidence.signed_table_digest,
            ),
            (
                RECEIPT_SIGNED_POST_OFFSET,
                self.evidence.signed_post_resource_digest,
            ),
            (
                RECEIPT_CUSTODY_REQUEST_OFFSET,
                self.evidence.custody_request_digest,
            ),
            (
                RECEIPT_CUSTODY_RECEIPT_OFFSET,
                self.evidence.custody_receipt_digest,
            ),
            (
                RECEIPT_CUSTODY_REPLAY_OFFSET,
                self.evidence.custody_replay_digest,
            ),
            (
                RECEIPT_CUSTODY_TOKEN_POST_OFFSET,
                self.evidence.custody_token_poststate_digest,
            ),
            (
                RECEIPT_POST_RESOURCE_OFFSET,
                self.evidence.post_resource_digest,
            ),
        ] {
            put(&mut out, offset, &value);
        }
        for (offset, value) in [
            (RECEIPT_PAYOUT_OFFSET, self.evidence.payout),
            (RECEIPT_PRE_MARKET_OFFSET, self.evidence.pre_market_revision),
            (
                RECEIPT_POST_MARKET_OFFSET,
                self.evidence.post_market_revision,
            ),
            (
                RECEIPT_PRE_POSITION_OFFSET,
                self.evidence.pre_position_revision,
            ),
            (
                RECEIPT_POST_POSITION_OFFSET,
                self.evidence.post_position_revision,
            ),
            (
                RECEIPT_PRE_CUSTODY_OFFSET,
                self.evidence.pre_custody_revision,
            ),
            (
                RECEIPT_POST_CUSTODY_OFFSET,
                self.evidence.post_custody_revision,
            ),
        ] {
            put(&mut out, offset, &value.to_le_bytes());
        }
        out
    }

    /// Require agreement with the exact request and independently recomputed evidence.
    pub fn verify_for(
        self,
        request: TerminalSettlementRequestV3,
        evidence: TerminalSettlementReceiptInputV3,
    ) -> Result<()> {
        if self != Self::new(request, evidence)? {
            return Err(TerminalSettlementErrorV3::ReceiptMismatch);
        }
        Ok(())
    }

    /// Return the embedded accepted request.
    pub const fn request(self) -> TerminalSettlementRequestV3 {
        self.request
    }
    /// Return all physical commitments.
    pub const fn evidence(self) -> TerminalSettlementReceiptInputV3 {
        self.evidence
    }
}

fn header(bytes: &[u8], width: usize, magic: [u8; 8]) -> Result<()> {
    if bytes.len() != width {
        return Err(TerminalSettlementErrorV3::InvalidLength);
    }
    if array::<8>(bytes, 0)? != magic {
        return Err(TerminalSettlementErrorV3::InvalidMagic);
    }
    if u16_at(bytes, 8)? != TERMINAL_SETTLEMENT_VERSION_V3 {
        return Err(TerminalSettlementErrorV3::UnsupportedVersion);
    }
    Ok(())
}
fn nonzero(value: [u8; 32]) -> Result<()> {
    if is_zero(value) {
        Err(TerminalSettlementErrorV3::ZeroIdentity)
    } else {
        Ok(())
    }
}
fn is_zero(value: [u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}
fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(TerminalSettlementErrorV3::InvalidLength)
}
fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(bytes, offset, N)?
        .try_into()
        .map_err(|_| TerminalSettlementErrorV3::InvalidLength)
}
fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(TerminalSettlementErrorV3::InvalidLength)?,
        )
        .ok_or(TerminalSettlementErrorV3::InvalidLength)
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}
fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}
fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().any(|byte| *byte != 0) {
        Err(TerminalSettlementErrorV3::NonCanonical)
    } else {
        Ok(())
    }
}
fn put(out: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(destination) = out.get_mut(offset..offset.saturating_add(value.len())) {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }
    fn request() -> TerminalSettlementRequestV3 {
        TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
            caller_role: CallerRole::Trading,
            release_set: id(1),
            market: id(2),
            realm: id(3),
            parent_context: id(4),
            product_record_digest: id(5),
            exposure_id: id(6),
            exposure_digest: id(7),
            terminal_record_digest: id(8),
            owner: id(9),
            position: id(10),
            recipient_owner: id(11),
            recipient_token_account: id(12),
            claims_program: id(13),
            custody_program: id(14),
            collateral_mint: id(15),
            token_program: id(16),
            semantic_basis_id: id(17),
            linked_basis_record_digest: id(18),
            generation: 19,
            expected_market_revision: 20,
            expected_position_revision: 21,
            expected_custody_revision: 22,
            quantity: 23,
            claim_index: 4,
            transfer_index: 5,
        })
        .expect("request")
    }
    fn evidence(payout: u64) -> TerminalSettlementReceiptInputV3 {
        TerminalSettlementReceiptInputV3 {
            request_digest: id(31),
            signed_packet_digest: id(32),
            signed_table_digest: id(33),
            signed_post_resource_digest: id(34),
            custody_request_digest: if payout == 0 { [0; 32] } else { id(35) },
            custody_receipt_digest: if payout == 0 { [0; 32] } else { id(36) },
            custody_replay_digest: id(37),
            custody_token_poststate_digest: id(38),
            post_resource_digest: id(39),
            payout,
            pre_market_revision: 20,
            post_market_revision: 21,
            pre_position_revision: 21,
            post_position_revision: 22,
            pre_custody_revision: 22,
            post_custody_revision: if payout == 0 { 22 } else { 23 },
        }
    }

    #[test]
    fn exact_request_and_positive_or_zero_receipts_round_trip() {
        let request = request();
        assert_eq!(
            TerminalSettlementRequestV3::decode(&request.to_bytes()),
            Ok(request)
        );
        for payout in [0, 55] {
            let receipt =
                TerminalSettlementReceiptV3::new(request, evidence(payout)).expect("receipt");
            assert_eq!(
                TerminalSettlementReceiptV3::decode(&receipt.to_bytes()),
                Ok(receipt)
            );
        }
    }

    #[test]
    fn substitutions_and_custody_shapes_refuse() {
        let request = request();
        let mut bytes = request.to_bytes();
        bytes[EXPOSURE_DIGEST_OFFSET] ^= 1;
        assert_ne!(TerminalSettlementRequestV3::decode(&bytes), Ok(request));
        let mut zero = evidence(0);
        zero.custody_receipt_digest = id(40);
        assert_eq!(
            TerminalSettlementReceiptV3::new(request, zero),
            Err(TerminalSettlementErrorV3::InvalidCustodyShape)
        );
        let mut paid = evidence(1);
        paid.post_custody_revision = paid.pre_custody_revision;
        assert_eq!(
            TerminalSettlementReceiptV3::new(request, paid),
            Err(TerminalSettlementErrorV3::InvalidCustodyShape)
        );
    }

    #[test]
    fn truncation_reserved_and_poststate_substitution_refuse() {
        let request = request();
        let receipt = TerminalSettlementReceiptV3::new(request, evidence(9)).expect("receipt");
        let bytes = receipt.to_bytes();
        assert_eq!(
            TerminalSettlementReceiptV3::decode(&bytes[..bytes.len() - 1]),
            Err(TerminalSettlementErrorV3::InvalidLength)
        );
        let mut reserved = bytes;
        reserved[1007] = 1;
        assert_eq!(
            TerminalSettlementReceiptV3::decode(&reserved),
            Err(TerminalSettlementErrorV3::NonCanonical)
        );
        let mut changed = evidence(9);
        changed.signed_post_resource_digest = id(99);
        assert_eq!(
            receipt.verify_for(request, changed),
            Err(TerminalSettlementErrorV3::ReceiptMismatch)
        );
    }
}
