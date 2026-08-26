//! Fixed, allocation-free Market retirement bundle and Core receipt.
//!
//! This physical contract binds the exact evidence consumed by the generated
//! [`crate::retire`] transition. It does not authenticate accounts, programs,
//! hashing, CPI return data, or Loader deployments; those remain adapter work.

use core::convert::TryInto;

pub use crate::generated_retirement_v1::{
    MARKET_RETIREMENT_VERSION_V1, RETIRED_CANDIDATE_DIGEST_DOMAIN_V1, RETIREMENT_BUNDLE_BYTES_V1,
    RETIREMENT_BUNDLE_MAGIC_V1, RETIREMENT_CUSTODY_RECEIPT_COUNT_V1,
    RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1, RETIREMENT_PRODUCER_CLOSURE_COUNT_V1,
    RETIREMENT_RECEIPT_BYTES_V1, RETIREMENT_RECEIPT_MAGIC_V1, RETIREMENT_ROLE_COUNT_V1,
};
use crate::generated_retirement_v1::{
    RETIREMENT_BUNDLE_CLAIMS_AGGREGATE_OFFSET, RETIREMENT_BUNDLE_CLAIMS_POST_REVISION_OFFSET,
    RETIREMENT_BUNDLE_CLAIMS_PRE_REVISION_OFFSET, RETIREMENT_BUNDLE_CLAIMS_REQUEST_DIGEST_OFFSET,
    RETIREMENT_BUNDLE_CORE_PRESTATE_DIGEST_OFFSET,
    RETIREMENT_BUNDLE_CUSTODY_CLOSE_REPLAY_REQUEST_DIGEST_OFFSET,
    RETIREMENT_BUNDLE_CUSTODY_CLOSE_VAULT_REQUEST_DIGEST_OFFSET,
    RETIREMENT_BUNDLE_CUSTODY_MIDDLE_REVISION_OFFSET,
    RETIREMENT_BUNDLE_CUSTODY_POST_REVISION_OFFSET, RETIREMENT_BUNDLE_CUSTODY_PRE_REVISION_OFFSET,
    RETIREMENT_BUNDLE_CUSTODY_RECEIPT_COUNT_OFFSET, RETIREMENT_BUNDLE_CUSTODY_REPLAY_OFFSET,
    RETIREMENT_BUNDLE_EXPECTED_CORE_LAMPORTS_OFFSET, RETIREMENT_BUNDLE_GENERATION_OFFSET,
    RETIREMENT_BUNDLE_HOARD_VAULT_OFFSET, RETIREMENT_BUNDLE_MAGIC_OFFSET,
    RETIREMENT_BUNDLE_MARKET_OFFSET, RETIREMENT_BUNDLE_RELEASE_SET_OFFSET,
    RETIREMENT_BUNDLE_RENT_CREDIT_OFFSET, RETIREMENT_BUNDLE_RESERVED_BODY_OFFSET,
    RETIREMENT_BUNDLE_RESERVED_HEADER_OFFSET, RETIREMENT_BUNDLE_ROLE_COUNT_OFFSET,
    RETIREMENT_BUNDLE_SOURCE_CLOSURE_REVISION_OFFSET,
    RETIREMENT_BUNDLE_SOURCE_RECEIPT_ACCOUNT_OFFSET,
    RETIREMENT_BUNDLE_SOURCE_RECEIPT_DIGEST_OFFSET, RETIREMENT_BUNDLE_VERSION_OFFSET,
    RETIREMENT_RECEIPT_BUNDLE_DIGEST_OFFSET, RETIREMENT_RECEIPT_CLAIMS_POST_REVISION_OFFSET,
    RETIREMENT_RECEIPT_CLAIMS_RECEIPT_DIGEST_OFFSET,
    RETIREMENT_RECEIPT_CLAIMS_REFUND_LAMPORTS_OFFSET, RETIREMENT_RECEIPT_CORE_PROGRAM_OFFSET,
    RETIREMENT_RECEIPT_CORE_REFUND_LAMPORTS_OFFSET,
    RETIREMENT_RECEIPT_CUSTODY_CLOSE_REPLAY_RECEIPT_DIGEST_OFFSET,
    RETIREMENT_RECEIPT_CUSTODY_CLOSE_VAULT_RECEIPT_DIGEST_OFFSET,
    RETIREMENT_RECEIPT_CUSTODY_POST_REVISION_OFFSET,
    RETIREMENT_RECEIPT_CUSTODY_REFUND_LAMPORTS_OFFSET, RETIREMENT_RECEIPT_GENERATION_OFFSET,
    RETIREMENT_RECEIPT_KIND_OFFSET, RETIREMENT_RECEIPT_MAGIC_OFFSET,
    RETIREMENT_RECEIPT_MARKET_OFFSET, RETIREMENT_RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
    RETIREMENT_RECEIPT_PRE_STATE_DIGEST_OFFSET, RETIREMENT_RECEIPT_RELEASE_SET_OFFSET,
    RETIREMENT_RECEIPT_RENT_CREDIT_OFFSET, RETIREMENT_RECEIPT_RESERVED_BODY_OFFSET,
    RETIREMENT_RECEIPT_RESERVED_HEADER_OFFSET, RETIREMENT_RECEIPT_RETIRED_CANDIDATE_DIGEST_OFFSET,
    RETIREMENT_RECEIPT_SOURCE_CLOSURE_REVISION_OFFSET,
    RETIREMENT_RECEIPT_SOURCE_RECEIPT_DIGEST_OFFSET, RETIREMENT_RECEIPT_VERSION_OFFSET,
};

const RECEIPT_KIND_V1: u8 = 1;

/// Stable hostile-decode or evidence-join refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementErrorV1 {
    /// The input width was not exact.
    InvalidLength,
    /// Magic or version selected another wire family.
    InvalidHeader,
    /// Reserved bytes, counts, or tags were noncanonical.
    NonCanonical,
    /// A required account identity or digest was zero.
    ZeroIdentity,
    /// Two required distinct accounts aliased.
    AccountAlias,
    /// Generation, revision, or lamport coordinates were invalid.
    InvalidCoordinate,
    /// A receipt did not bind the exact bundle and observed digests.
    ReceiptMismatch,
}

/// Result alias for the retirement ABI.
pub type RetirementResultV1<T> = core::result::Result<T, RetirementErrorV1>;

/// Construction input for one exact retirement bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementBundleInputV1 {
    /// Canonical Core Market account.
    pub market: [u8; 32],
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Immutable RentCredit beneficiary.
    pub rent_credit: [u8; 32],
    /// Resolution-owned Source closure receipt account.
    pub source_receipt_account: [u8; 32],
    /// Canonical Claims aggregate account.
    pub claims_aggregate: [u8; 32],
    /// Canonical normal Custody replay account.
    pub custody_replay: [u8; 32],
    /// Canonical HoardPrincipal vault.
    pub hoard_vault: [u8; 32],
    /// SHA-256 of the exact Source closure receipt bytes.
    pub source_receipt_digest: [u8; 32],
    /// SHA-256 of the exact Claims closure request.
    pub claims_request_digest: [u8; 32],
    /// SHA-256 of the exact normal Custody CloseVault request.
    pub custody_close_vault_request_digest: [u8; 32],
    /// SHA-256 of the exact normal Custody CloseReplay request.
    pub custody_close_replay_request_digest: [u8; 32],
    /// SHA-256 of the exact Retiring Core state bytes.
    pub core_prestate_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Resolution closure revision.
    pub source_closure_revision: u64,
    /// Claims aggregate revision before closure.
    pub claims_pre_revision: u64,
    /// Claims aggregate closure revision.
    pub claims_post_revision: u64,
    /// Custody replay revision before CloseVault.
    pub custody_pre_revision: u64,
    /// Custody replay revision after CloseVault.
    pub custody_middle_revision: u64,
    /// Custody replay revision after CloseReplay.
    pub custody_post_revision: u64,
    /// Exact lamports observed on the Core Market before closure.
    pub expected_core_lamports: u64,
}

/// One exact hostile-decodable retirement bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementBundleV1(RetirementBundleInputV1);

impl RetirementBundleV1 {
    /// Construct and validate a retirement bundle.
    pub fn new(input: RetirementBundleInputV1) -> RetirementResultV1<Self> {
        require_nonzero(&[
            input.market,
            input.release_set,
            input.rent_credit,
            input.source_receipt_account,
            input.claims_aggregate,
            input.custody_replay,
            input.hoard_vault,
            input.source_receipt_digest,
            input.claims_request_digest,
            input.custody_close_vault_request_digest,
            input.custody_close_replay_request_digest,
            input.core_prestate_digest,
        ])?;
        require_distinct(&[
            input.market,
            input.rent_credit,
            input.source_receipt_account,
            input.claims_aggregate,
            input.custody_replay,
            input.hoard_vault,
        ])?;
        if input.generation == 0
            || input.source_closure_revision == 0
            || input.claims_pre_revision.checked_add(1) != Some(input.claims_post_revision)
            || input.custody_pre_revision.checked_add(1) != Some(input.custody_middle_revision)
            || input.custody_middle_revision.checked_add(1) != Some(input.custody_post_revision)
            || input.expected_core_lamports == 0
        {
            return Err(RetirementErrorV1::InvalidCoordinate);
        }
        Ok(Self(input))
    }

    /// Hostile-decode one exact retirement bundle.
    pub fn decode(input: &[u8]) -> RetirementResultV1<Self> {
        require_header(
            input,
            &RETIREMENT_BUNDLE_MAGIC_V1,
            RETIREMENT_BUNDLE_BYTES_V1,
            RETIREMENT_BUNDLE_VERSION_OFFSET,
        )?;
        if byte(input, RETIREMENT_BUNDLE_ROLE_COUNT_OFFSET)? != RETIREMENT_ROLE_COUNT_V1
            || byte(input, RETIREMENT_BUNDLE_CUSTODY_RECEIPT_COUNT_OFFSET)?
                != RETIREMENT_CUSTODY_RECEIPT_COUNT_V1
        {
            return Err(RetirementErrorV1::NonCanonical);
        }
        require_zero(input, RETIREMENT_BUNDLE_RESERVED_HEADER_OFFSET, 4)?;
        require_zero(input, RETIREMENT_BUNDLE_RESERVED_BODY_OFFSET, 16)?;
        Self::new(RetirementBundleInputV1 {
            market: array(input, RETIREMENT_BUNDLE_MARKET_OFFSET)?,
            release_set: array(input, RETIREMENT_BUNDLE_RELEASE_SET_OFFSET)?,
            rent_credit: array(input, RETIREMENT_BUNDLE_RENT_CREDIT_OFFSET)?,
            source_receipt_account: array(input, RETIREMENT_BUNDLE_SOURCE_RECEIPT_ACCOUNT_OFFSET)?,
            claims_aggregate: array(input, RETIREMENT_BUNDLE_CLAIMS_AGGREGATE_OFFSET)?,
            custody_replay: array(input, RETIREMENT_BUNDLE_CUSTODY_REPLAY_OFFSET)?,
            hoard_vault: array(input, RETIREMENT_BUNDLE_HOARD_VAULT_OFFSET)?,
            source_receipt_digest: array(input, RETIREMENT_BUNDLE_SOURCE_RECEIPT_DIGEST_OFFSET)?,
            claims_request_digest: array(input, RETIREMENT_BUNDLE_CLAIMS_REQUEST_DIGEST_OFFSET)?,
            custody_close_vault_request_digest: array(
                input,
                RETIREMENT_BUNDLE_CUSTODY_CLOSE_VAULT_REQUEST_DIGEST_OFFSET,
            )?,
            custody_close_replay_request_digest: array(
                input,
                RETIREMENT_BUNDLE_CUSTODY_CLOSE_REPLAY_REQUEST_DIGEST_OFFSET,
            )?,
            core_prestate_digest: array(input, RETIREMENT_BUNDLE_CORE_PRESTATE_DIGEST_OFFSET)?,
            generation: u64_at(input, RETIREMENT_BUNDLE_GENERATION_OFFSET)?,
            source_closure_revision: u64_at(
                input,
                RETIREMENT_BUNDLE_SOURCE_CLOSURE_REVISION_OFFSET,
            )?,
            claims_pre_revision: u64_at(input, RETIREMENT_BUNDLE_CLAIMS_PRE_REVISION_OFFSET)?,
            claims_post_revision: u64_at(input, RETIREMENT_BUNDLE_CLAIMS_POST_REVISION_OFFSET)?,
            custody_pre_revision: u64_at(input, RETIREMENT_BUNDLE_CUSTODY_PRE_REVISION_OFFSET)?,
            custody_middle_revision: u64_at(
                input,
                RETIREMENT_BUNDLE_CUSTODY_MIDDLE_REVISION_OFFSET,
            )?,
            custody_post_revision: u64_at(input, RETIREMENT_BUNDLE_CUSTODY_POST_REVISION_OFFSET)?,
            expected_core_lamports: u64_at(input, RETIREMENT_BUNDLE_EXPECTED_CORE_LAMPORTS_OFFSET)?,
        })
    }

    /// Encode the exact canonical bundle bytes.
    pub fn to_bytes(self) -> [u8; RETIREMENT_BUNDLE_BYTES_V1] {
        let mut output = [0; RETIREMENT_BUNDLE_BYTES_V1];
        put(
            &mut output,
            RETIREMENT_BUNDLE_MAGIC_OFFSET,
            &RETIREMENT_BUNDLE_MAGIC_V1,
        );
        put_u16(
            &mut output,
            RETIREMENT_BUNDLE_VERSION_OFFSET,
            MARKET_RETIREMENT_VERSION_V1,
        );
        output[RETIREMENT_BUNDLE_ROLE_COUNT_OFFSET] = RETIREMENT_ROLE_COUNT_V1;
        output[RETIREMENT_BUNDLE_CUSTODY_RECEIPT_COUNT_OFFSET] =
            RETIREMENT_CUSTODY_RECEIPT_COUNT_V1;
        for (offset, value) in [
            (RETIREMENT_BUNDLE_MARKET_OFFSET, self.0.market),
            (RETIREMENT_BUNDLE_RELEASE_SET_OFFSET, self.0.release_set),
            (RETIREMENT_BUNDLE_RENT_CREDIT_OFFSET, self.0.rent_credit),
            (
                RETIREMENT_BUNDLE_SOURCE_RECEIPT_ACCOUNT_OFFSET,
                self.0.source_receipt_account,
            ),
            (
                RETIREMENT_BUNDLE_CLAIMS_AGGREGATE_OFFSET,
                self.0.claims_aggregate,
            ),
            (
                RETIREMENT_BUNDLE_CUSTODY_REPLAY_OFFSET,
                self.0.custody_replay,
            ),
            (RETIREMENT_BUNDLE_HOARD_VAULT_OFFSET, self.0.hoard_vault),
            (
                RETIREMENT_BUNDLE_SOURCE_RECEIPT_DIGEST_OFFSET,
                self.0.source_receipt_digest,
            ),
            (
                RETIREMENT_BUNDLE_CLAIMS_REQUEST_DIGEST_OFFSET,
                self.0.claims_request_digest,
            ),
            (
                RETIREMENT_BUNDLE_CUSTODY_CLOSE_VAULT_REQUEST_DIGEST_OFFSET,
                self.0.custody_close_vault_request_digest,
            ),
            (
                RETIREMENT_BUNDLE_CUSTODY_CLOSE_REPLAY_REQUEST_DIGEST_OFFSET,
                self.0.custody_close_replay_request_digest,
            ),
            (
                RETIREMENT_BUNDLE_CORE_PRESTATE_DIGEST_OFFSET,
                self.0.core_prestate_digest,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (RETIREMENT_BUNDLE_GENERATION_OFFSET, self.0.generation),
            (
                RETIREMENT_BUNDLE_SOURCE_CLOSURE_REVISION_OFFSET,
                self.0.source_closure_revision,
            ),
            (
                RETIREMENT_BUNDLE_CLAIMS_PRE_REVISION_OFFSET,
                self.0.claims_pre_revision,
            ),
            (
                RETIREMENT_BUNDLE_CLAIMS_POST_REVISION_OFFSET,
                self.0.claims_post_revision,
            ),
            (
                RETIREMENT_BUNDLE_CUSTODY_PRE_REVISION_OFFSET,
                self.0.custody_pre_revision,
            ),
            (
                RETIREMENT_BUNDLE_CUSTODY_MIDDLE_REVISION_OFFSET,
                self.0.custody_middle_revision,
            ),
            (
                RETIREMENT_BUNDLE_CUSTODY_POST_REVISION_OFFSET,
                self.0.custody_post_revision,
            ),
            (
                RETIREMENT_BUNDLE_EXPECTED_CORE_LAMPORTS_OFFSET,
                self.0.expected_core_lamports,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        output
    }

    /// Borrow all validated bundle coordinates.
    pub const fn input(self) -> RetirementBundleInputV1 {
        self.0
    }

    /// Borrow all validated bundle coordinates without duplicating the fixed
    /// release and evidence projection on a constrained adapter stack.
    pub const fn input_ref(&self) -> &RetirementBundleInputV1 {
        &self.0
    }
}

/// Construction input for the immediate Core retirement receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReceiptInputV1 {
    /// Current Core program producing the receipt.
    pub core_program: [u8; 32],
    /// Canonical Market being closed.
    pub market: [u8; 32],
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Immutable RentCredit beneficiary.
    pub rent_credit: [u8; 32],
    /// SHA-256 of exact [`RetirementBundleV1`] bytes.
    pub bundle_digest: [u8; 32],
    /// SHA-256 of exact Resolution closure receipt bytes.
    pub source_receipt_digest: [u8; 32],
    /// SHA-256 of exact Claims closure receipt bytes.
    pub claims_receipt_digest: [u8; 32],
    /// SHA-256 of exact Custody CloseVault receipt bytes.
    pub custody_close_vault_receipt_digest: [u8; 32],
    /// SHA-256 of exact Custody CloseReplay receipt bytes.
    pub custody_close_replay_receipt_digest: [u8; 32],
    /// SHA-256 of exact Retiring Core state bytes.
    pub pre_state_digest: [u8; 32],
    /// Domain-separated digest of the generated Retired candidate bytes.
    pub retired_candidate_digest: [u8; 32],
    /// Domain-separated complete producer-subtree closure digest.
    ///
    /// Its exact preimage orders the fixed role/count header, RentCredit,
    /// Source/Claims/CloseVault/CloseReplay receipt digests, and the three
    /// refund-lamport observations followed by final RentCredit lamports. The
    /// bound RentCredit remains sole owner of its immutable refund-wallet fact.
    pub post_resource_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Resolution closure revision.
    pub source_closure_revision: u64,
    /// Claims aggregate closure revision.
    pub claims_post_revision: u64,
    /// Custody replay closure revision.
    pub custody_post_revision: u64,
    /// Exact Core Market lamports refunded.
    pub core_refund_lamports: u64,
    /// Exact Claims aggregate lamports refunded.
    pub claims_refund_lamports: u64,
    /// Exact combined Custody vault and replay lamports refunded.
    pub custody_refund_lamports: u64,
}

/// Immediate producer-bound Core retirement acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReceiptV1(RetirementReceiptInputV1);

impl RetirementReceiptV1 {
    /// Construct and validate an immediate Core receipt.
    pub fn new(input: RetirementReceiptInputV1) -> RetirementResultV1<Self> {
        require_nonzero(&[
            input.core_program,
            input.market,
            input.release_set,
            input.rent_credit,
            input.bundle_digest,
            input.source_receipt_digest,
            input.claims_receipt_digest,
            input.custody_close_vault_receipt_digest,
            input.custody_close_replay_receipt_digest,
            input.pre_state_digest,
            input.retired_candidate_digest,
            input.post_resource_digest,
        ])?;
        if input.generation == 0
            || input.source_closure_revision == 0
            || input.claims_post_revision == 0
            || input.custody_post_revision == 0
            || input.core_refund_lamports == 0
            || input.claims_refund_lamports == 0
            || input.custody_refund_lamports == 0
        {
            return Err(RetirementErrorV1::InvalidCoordinate);
        }
        Ok(Self(input))
    }

    /// Hostile-decode one exact Core retirement receipt.
    pub fn decode(input: &[u8]) -> RetirementResultV1<Self> {
        require_header(
            input,
            &RETIREMENT_RECEIPT_MAGIC_V1,
            RETIREMENT_RECEIPT_BYTES_V1,
            RETIREMENT_RECEIPT_VERSION_OFFSET,
        )?;
        if byte(input, RETIREMENT_RECEIPT_KIND_OFFSET)? != RECEIPT_KIND_V1 {
            return Err(RetirementErrorV1::NonCanonical);
        }
        require_zero(input, RETIREMENT_RECEIPT_RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, RETIREMENT_RECEIPT_RESERVED_BODY_OFFSET, 56)?;
        Self::new(RetirementReceiptInputV1 {
            core_program: array(input, RETIREMENT_RECEIPT_CORE_PROGRAM_OFFSET)?,
            market: array(input, RETIREMENT_RECEIPT_MARKET_OFFSET)?,
            release_set: array(input, RETIREMENT_RECEIPT_RELEASE_SET_OFFSET)?,
            rent_credit: array(input, RETIREMENT_RECEIPT_RENT_CREDIT_OFFSET)?,
            bundle_digest: array(input, RETIREMENT_RECEIPT_BUNDLE_DIGEST_OFFSET)?,
            source_receipt_digest: array(input, RETIREMENT_RECEIPT_SOURCE_RECEIPT_DIGEST_OFFSET)?,
            claims_receipt_digest: array(input, RETIREMENT_RECEIPT_CLAIMS_RECEIPT_DIGEST_OFFSET)?,
            custody_close_vault_receipt_digest: array(
                input,
                RETIREMENT_RECEIPT_CUSTODY_CLOSE_VAULT_RECEIPT_DIGEST_OFFSET,
            )?,
            custody_close_replay_receipt_digest: array(
                input,
                RETIREMENT_RECEIPT_CUSTODY_CLOSE_REPLAY_RECEIPT_DIGEST_OFFSET,
            )?,
            pre_state_digest: array(input, RETIREMENT_RECEIPT_PRE_STATE_DIGEST_OFFSET)?,
            retired_candidate_digest: array(
                input,
                RETIREMENT_RECEIPT_RETIRED_CANDIDATE_DIGEST_OFFSET,
            )?,
            post_resource_digest: array(input, RETIREMENT_RECEIPT_POST_RESOURCE_DIGEST_OFFSET)?,
            generation: u64_at(input, RETIREMENT_RECEIPT_GENERATION_OFFSET)?,
            source_closure_revision: u64_at(
                input,
                RETIREMENT_RECEIPT_SOURCE_CLOSURE_REVISION_OFFSET,
            )?,
            claims_post_revision: u64_at(input, RETIREMENT_RECEIPT_CLAIMS_POST_REVISION_OFFSET)?,
            custody_post_revision: u64_at(input, RETIREMENT_RECEIPT_CUSTODY_POST_REVISION_OFFSET)?,
            core_refund_lamports: u64_at(input, RETIREMENT_RECEIPT_CORE_REFUND_LAMPORTS_OFFSET)?,
            claims_refund_lamports: u64_at(
                input,
                RETIREMENT_RECEIPT_CLAIMS_REFUND_LAMPORTS_OFFSET,
            )?,
            custody_refund_lamports: u64_at(
                input,
                RETIREMENT_RECEIPT_CUSTODY_REFUND_LAMPORTS_OFFSET,
            )?,
        })
    }

    /// Encode the exact Core receipt bytes.
    pub fn to_bytes(self) -> [u8; RETIREMENT_RECEIPT_BYTES_V1] {
        let mut output = [0; RETIREMENT_RECEIPT_BYTES_V1];
        put(
            &mut output,
            RETIREMENT_RECEIPT_MAGIC_OFFSET,
            &RETIREMENT_RECEIPT_MAGIC_V1,
        );
        put_u16(
            &mut output,
            RETIREMENT_RECEIPT_VERSION_OFFSET,
            MARKET_RETIREMENT_VERSION_V1,
        );
        output[RETIREMENT_RECEIPT_KIND_OFFSET] = RECEIPT_KIND_V1;
        for (offset, value) in [
            (RETIREMENT_RECEIPT_CORE_PROGRAM_OFFSET, self.0.core_program),
            (RETIREMENT_RECEIPT_MARKET_OFFSET, self.0.market),
            (RETIREMENT_RECEIPT_RELEASE_SET_OFFSET, self.0.release_set),
            (RETIREMENT_RECEIPT_RENT_CREDIT_OFFSET, self.0.rent_credit),
            (
                RETIREMENT_RECEIPT_BUNDLE_DIGEST_OFFSET,
                self.0.bundle_digest,
            ),
            (
                RETIREMENT_RECEIPT_SOURCE_RECEIPT_DIGEST_OFFSET,
                self.0.source_receipt_digest,
            ),
            (
                RETIREMENT_RECEIPT_CLAIMS_RECEIPT_DIGEST_OFFSET,
                self.0.claims_receipt_digest,
            ),
            (
                RETIREMENT_RECEIPT_CUSTODY_CLOSE_VAULT_RECEIPT_DIGEST_OFFSET,
                self.0.custody_close_vault_receipt_digest,
            ),
            (
                RETIREMENT_RECEIPT_CUSTODY_CLOSE_REPLAY_RECEIPT_DIGEST_OFFSET,
                self.0.custody_close_replay_receipt_digest,
            ),
            (
                RETIREMENT_RECEIPT_PRE_STATE_DIGEST_OFFSET,
                self.0.pre_state_digest,
            ),
            (
                RETIREMENT_RECEIPT_RETIRED_CANDIDATE_DIGEST_OFFSET,
                self.0.retired_candidate_digest,
            ),
            (
                RETIREMENT_RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
                self.0.post_resource_digest,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (RETIREMENT_RECEIPT_GENERATION_OFFSET, self.0.generation),
            (
                RETIREMENT_RECEIPT_SOURCE_CLOSURE_REVISION_OFFSET,
                self.0.source_closure_revision,
            ),
            (
                RETIREMENT_RECEIPT_CLAIMS_POST_REVISION_OFFSET,
                self.0.claims_post_revision,
            ),
            (
                RETIREMENT_RECEIPT_CUSTODY_POST_REVISION_OFFSET,
                self.0.custody_post_revision,
            ),
            (
                RETIREMENT_RECEIPT_CORE_REFUND_LAMPORTS_OFFSET,
                self.0.core_refund_lamports,
            ),
            (
                RETIREMENT_RECEIPT_CLAIMS_REFUND_LAMPORTS_OFFSET,
                self.0.claims_refund_lamports,
            ),
            (
                RETIREMENT_RECEIPT_CUSTODY_REFUND_LAMPORTS_OFFSET,
                self.0.custody_refund_lamports,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        output
    }

    /// Verify the receipt against one exact bundle and observed receipt digests.
    pub fn verify_for(
        self,
        bundle: RetirementBundleV1,
        bundle_digest: [u8; 32],
        claims_receipt_digest: [u8; 32],
        custody_close_vault_receipt_digest: [u8; 32],
        custody_close_replay_receipt_digest: [u8; 32],
    ) -> RetirementResultV1<()> {
        let bundle = bundle.input();
        if self.0.market != bundle.market
            || self.0.release_set != bundle.release_set
            || self.0.rent_credit != bundle.rent_credit
            || self.0.bundle_digest != bundle_digest
            || self.0.source_receipt_digest != bundle.source_receipt_digest
            || self.0.claims_receipt_digest != claims_receipt_digest
            || self.0.custody_close_vault_receipt_digest != custody_close_vault_receipt_digest
            || self.0.custody_close_replay_receipt_digest != custody_close_replay_receipt_digest
            || self.0.pre_state_digest != bundle.core_prestate_digest
            || self.0.generation != bundle.generation
            || self.0.source_closure_revision != bundle.source_closure_revision
            || self.0.claims_post_revision != bundle.claims_post_revision
            || self.0.custody_post_revision != bundle.custody_post_revision
            || self.0.core_refund_lamports != bundle.expected_core_lamports
        {
            return Err(RetirementErrorV1::ReceiptMismatch);
        }
        Ok(())
    }

    /// Borrow all validated receipt coordinates.
    pub const fn input(self) -> RetirementReceiptInputV1 {
        self.0
    }
}

fn require_nonzero(values: &[[u8; 32]]) -> RetirementResultV1<()> {
    if values
        .iter()
        .any(|value| value.iter().all(|byte| *byte == 0))
    {
        Err(RetirementErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_distinct(values: &[[u8; 32]]) -> RetirementResultV1<()> {
    for (index, value) in values.iter().enumerate() {
        if values
            .get(index + 1..)
            .is_some_and(|tail| tail.contains(value))
        {
            return Err(RetirementErrorV1::AccountAlias);
        }
    }
    Ok(())
}

fn require_header(
    input: &[u8],
    magic: &[u8; 8],
    width: usize,
    version_offset: usize,
) -> RetirementResultV1<()> {
    if input.len() != width || input.get(..8) != Some(magic.as_slice()) {
        return Err(RetirementErrorV1::InvalidLength);
    }
    if u16_at(input, version_offset)? != MARKET_RETIREMENT_VERSION_V1 {
        return Err(RetirementErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> RetirementResultV1<()> {
    if input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(RetirementErrorV1::InvalidLength)?,
        )
        .ok_or(RetirementErrorV1::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(RetirementErrorV1::NonCanonical)
    } else {
        Ok(())
    }
}

fn byte(input: &[u8], offset: usize) -> RetirementResultV1<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(RetirementErrorV1::InvalidLength)
}

fn array(input: &[u8], offset: usize) -> RetirementResultV1<[u8; 32]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(32)
                    .ok_or(RetirementErrorV1::InvalidLength)?,
        )
        .ok_or(RetirementErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| RetirementErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> RetirementResultV1<u16> {
    Ok(u16::from_le_bytes(
        input
            .get(
                offset
                    ..offset
                        .checked_add(2)
                        .ok_or(RetirementErrorV1::InvalidLength)?,
            )
            .ok_or(RetirementErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| RetirementErrorV1::InvalidLength)?,
    ))
}

fn u64_at(input: &[u8], offset: usize) -> RetirementResultV1<u64> {
    Ok(u64::from_le_bytes(
        input
            .get(
                offset
                    ..offset
                        .checked_add(8)
                        .ok_or(RetirementErrorV1::InvalidLength)?,
            )
            .ok_or(RetirementErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| RetirementErrorV1::InvalidLength)?,
    ))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset + value.len()) {
        target.copy_from_slice(value);
    }
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> RetirementBundleV1 {
        RetirementBundleV1::new(RetirementBundleInputV1 {
            market: [1; 32],
            release_set: [2; 32],
            rent_credit: [3; 32],
            source_receipt_account: [4; 32],
            claims_aggregate: [5; 32],
            custody_replay: [6; 32],
            hoard_vault: [7; 32],
            source_receipt_digest: [8; 32],
            claims_request_digest: [9; 32],
            custody_close_vault_request_digest: [10; 32],
            custody_close_replay_request_digest: [11; 32],
            core_prestate_digest: [12; 32],
            generation: 3,
            source_closure_revision: 7,
            claims_pre_revision: 8,
            claims_post_revision: 9,
            custody_pre_revision: 10,
            custody_middle_revision: 11,
            custody_post_revision: 12,
            expected_core_lamports: 100,
        })
        .expect("canonical bundle")
    }

    #[test]
    fn bundle_round_trip_and_ordered_revisions() {
        let value = bundle();
        assert_eq!(RetirementBundleV1::decode(&value.to_bytes()), Ok(value));
        let mut hostile = value.to_bytes();
        hostile
            .get_mut(
                RETIREMENT_BUNDLE_CUSTODY_MIDDLE_REVISION_OFFSET
                    ..RETIREMENT_BUNDLE_CUSTODY_MIDDLE_REVISION_OFFSET + 8,
            )
            .expect("fixed revision field")
            .copy_from_slice(&12_u64.to_le_bytes());
        assert_eq!(
            RetirementBundleV1::decode(&hostile),
            Err(RetirementErrorV1::InvalidCoordinate)
        );
    }

    #[test]
    fn swapped_receipts_and_aliases_refuse() {
        let value = bundle();
        let bundle_input = value.input();
        let receipt = RetirementReceiptV1::new(RetirementReceiptInputV1 {
            core_program: [13; 32],
            market: bundle_input.market,
            release_set: bundle_input.release_set,
            rent_credit: bundle_input.rent_credit,
            bundle_digest: [14; 32],
            source_receipt_digest: bundle_input.source_receipt_digest,
            claims_receipt_digest: [15; 32],
            custody_close_vault_receipt_digest: [16; 32],
            custody_close_replay_receipt_digest: [17; 32],
            pre_state_digest: bundle_input.core_prestate_digest,
            retired_candidate_digest: [18; 32],
            post_resource_digest: [19; 32],
            generation: bundle_input.generation,
            source_closure_revision: bundle_input.source_closure_revision,
            claims_post_revision: bundle_input.claims_post_revision,
            custody_post_revision: bundle_input.custody_post_revision,
            core_refund_lamports: bundle_input.expected_core_lamports,
            claims_refund_lamports: 30,
            custody_refund_lamports: 40,
        })
        .expect("receipt");
        assert_eq!(
            receipt.verify_for(value, [14; 32], [15; 32], [16; 32], [17; 32]),
            Ok(())
        );
        assert_eq!(
            receipt.verify_for(value, [14; 32], [15; 32], [17; 32], [16; 32]),
            Err(RetirementErrorV1::ReceiptMismatch)
        );
        let mut alias = bundle_input;
        alias.hoard_vault = alias.custody_replay;
        assert_eq!(
            RetirementBundleV1::new(alias),
            Err(RetirementErrorV1::AccountAlias)
        );
    }
}
