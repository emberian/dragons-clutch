//! Atomic retirement-only transfer of one Custody replay between roles.
//!
//! This wire is deliberately not a generic replay-role mutation. It accepts
//! exactly the live Trading replay that owns the Market's one remaining Hoard
//! Vault and projects exactly one Core replay for the already-checked aggregate
//! retirement route. The adapter owns PDA, account, Loader, token, and CPI
//! authentication; this module owns the fixed bytes and the exhaustive balance
//! and lineage transition.

use dclutch_release_set_contract::ExecutionRoleV1;

use crate::CustodyReplayV1;

/// Exact handoff request magic.
pub const RETIREMENT_REPLAY_HANDOFF_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLCRH01";
/// Exact handoff receipt magic.
pub const RETIREMENT_REPLAY_HANDOFF_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLCRHR1";
/// Fixed request width.
pub const RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1: usize = 208;
/// Fixed receipt width.
pub const RETIREMENT_REPLAY_HANDOFF_RECEIPT_BYTES_V1: usize = 512;
/// Fixed Core and Custody account count.
pub const RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1: usize = 23;

/// Single semantic owner for the fixed Core-to-Custody handoff frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffAccountLayoutV1;

impl RetirementReplayHandoffAccountLayoutV1 {
    /// Exact frame width.
    pub const COUNT: usize = RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1;
    /// Externally signing rent payer.
    pub const PAYER: usize = 0;
    /// Core Market.
    pub const MARKET: usize = 1;
    /// Registry activation cache.
    pub const CACHE: usize = 2;
    /// Registry program.
    pub const REGISTRY: usize = 3;
    /// Core program.
    pub const CORE_PROGRAM: usize = 4;
    /// Core ProgramData.
    pub const CORE_PROGRAMDATA: usize = 5;
    /// Trading program.
    pub const TRADING_PROGRAM: usize = 6;
    /// Trading ProgramData.
    pub const TRADING_PROGRAMDATA: usize = 7;
    /// Custody program.
    pub const CUSTODY_PROGRAM: usize = 8;
    /// Custody ProgramData.
    pub const CUSTODY_PROGRAMDATA: usize = 9;
    /// Core caller-authority PDA, signed only in the child CPI.
    pub const CALLER_AUTHORITY: usize = 10;
    /// Claims aggregate owning the retirement context.
    pub const CLAIMS_AGGREGATE: usize = 11;
    /// Finalized Realm record.
    pub const REALM: usize = 12;
    /// Vacant Realm staging cursor.
    pub const REALM_STAGING: usize = 13;
    /// Rent sysvar.
    pub const RENT: usize = 14;
    /// Market RentCredit.
    pub const RENT_CREDIT: usize = 15;
    /// Live Trading-role replay closed by the handoff.
    pub const TRADING_REPLAY: usize = 16;
    /// Vacant Core-role replay created by the handoff.
    pub const CORE_REPLAY: usize = 17;
    /// Shared Hoard token account.
    pub const HOARD: usize = 18;
    /// System program.
    pub const SYSTEM: usize = 19;
    /// Realm collateral mint.
    pub const MINT: usize = 20;
    /// Realm token program.
    pub const TOKEN_PROGRAM: usize = 21;
    /// Custody authority PDA.
    pub const CUSTODY_AUTHORITY: usize = 22;
}

/// Named coordinates consumed by the two SBF parsers.
#[allow(missing_docs)]
pub mod retirement_replay_handoff_accounts_v1 {
    use super::RetirementReplayHandoffAccountLayoutV1 as Layout;

    pub const PAYER: usize = Layout::PAYER;
    pub const MARKET: usize = Layout::MARKET;
    pub const CACHE: usize = Layout::CACHE;
    pub const REGISTRY: usize = Layout::REGISTRY;
    pub const CORE_PROGRAM: usize = Layout::CORE_PROGRAM;
    pub const CORE_PROGRAMDATA: usize = Layout::CORE_PROGRAMDATA;
    pub const TRADING_PROGRAM: usize = Layout::TRADING_PROGRAM;
    pub const TRADING_PROGRAMDATA: usize = Layout::TRADING_PROGRAMDATA;
    pub const CUSTODY_PROGRAM: usize = Layout::CUSTODY_PROGRAM;
    pub const CUSTODY_PROGRAMDATA: usize = Layout::CUSTODY_PROGRAMDATA;
    pub const CALLER_AUTHORITY: usize = Layout::CALLER_AUTHORITY;
    pub const CLAIMS_AGGREGATE: usize = Layout::CLAIMS_AGGREGATE;
    pub const REALM: usize = Layout::REALM;
    pub const REALM_STAGING: usize = Layout::REALM_STAGING;
    pub const RENT: usize = Layout::RENT;
    pub const RENT_CREDIT: usize = Layout::RENT_CREDIT;
    pub const TRADING_REPLAY: usize = Layout::TRADING_REPLAY;
    pub const CORE_REPLAY: usize = Layout::CORE_REPLAY;
    pub const HOARD: usize = Layout::HOARD;
    pub const SYSTEM: usize = Layout::SYSTEM;
    pub const MINT: usize = Layout::MINT;
    pub const TOKEN_PROGRAM: usize = Layout::TOKEN_PROGRAM;
    pub const CUSTODY_AUTHORITY: usize = Layout::CUSTODY_AUTHORITY;
}

const VERSION_V1: u16 = 1;
const MARKET_OFFSET: usize = 16;
const CONTEXT_OFFSET: usize = 48;
const TRADING_DIGEST_OFFSET: usize = 80;
const HOARD_DIGEST_OFFSET: usize = 112;
const GENERATION_OFFSET: usize = 144;
const REVISION_OFFSET: usize = 152;
const TRADING_LAMPORTS_OFFSET: usize = 160;
const CORE_RENT_OFFSET: usize = 168;
const HOARD_LAMPORTS_OFFSET: usize = 176;
const RENT_CREDIT_OFFSET: usize = 184;
const PAYER_OFFSET: usize = 192;

const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 16;
const RECEIPT_MARKET_OFFSET: usize = 48;
const RECEIPT_CONTEXT_OFFSET: usize = 80;
const RECEIPT_TRADING_REPLAY_OFFSET: usize = 112;
const RECEIPT_CORE_REPLAY_OFFSET: usize = 144;
const RECEIPT_HOARD_OFFSET: usize = 176;
const RECEIPT_TRADING_DIGEST_OFFSET: usize = 208;
const RECEIPT_CORE_DIGEST_OFFSET: usize = 240;
const RECEIPT_HOARD_PRE_DIGEST_OFFSET: usize = 272;
const RECEIPT_HOARD_POST_DIGEST_OFFSET: usize = 304;
const RECEIPT_LINEAGE_REQUEST_OFFSET: usize = 336;
const RECEIPT_LINEAGE_POSTSTATE_OFFSET: usize = 368;
const RECEIPT_GENERATION_OFFSET: usize = 400;
const RECEIPT_REVISION_OFFSET: usize = 408;
const RECEIPT_TRADING_PRE_LAMPORTS_OFFSET: usize = 416;
const RECEIPT_TRADING_POST_LAMPORTS_OFFSET: usize = 424;
const RECEIPT_CORE_PRE_LAMPORTS_OFFSET: usize = 432;
const RECEIPT_CORE_POST_LAMPORTS_OFFSET: usize = 440;
const RECEIPT_HOARD_PRE_LAMPORTS_OFFSET: usize = 448;
const RECEIPT_HOARD_POST_LAMPORTS_OFFSET: usize = 456;
const RECEIPT_RENT_PRE_LAMPORTS_OFFSET: usize = 464;
const RECEIPT_RENT_POST_LAMPORTS_OFFSET: usize = 472;
const RECEIPT_PAYER_PRE_LAMPORTS_OFFSET: usize = 480;
const RECEIPT_PAYER_POST_LAMPORTS_OFFSET: usize = 488;
const RECEIPT_CORE_RENT_OFFSET: usize = 496;
const RECEIPT_SOURCE_REFUND_OFFSET: usize = 504;

/// Refusal from the fixed-layout handoff contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementReplayHandoffErrorV1 {
    /// Bytes had the wrong width, magic, version, or reserved region.
    InvalidBytes,
    /// A required identity, digest, generation, revision, or balance was zero.
    InvalidCoordinate,
    /// The source replay was not the exact live Trading retirement authority.
    InvalidTradingReplay,
    /// The observed accounts, digests, or balances did not match the request.
    ObservationMismatch,
    /// Checked rent arithmetic failed.
    Arithmetic,
    /// A decoded receipt did not describe the one canonical handoff delta.
    InvalidReceipt,
}

/// Exact caller-supplied prestate commitment for one retirement handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffRequestV1 {
    market: [u8; 32],
    context: [u8; 32],
    trading_replay_digest: [u8; 32],
    hoard_data_digest: [u8; 32],
    generation: u64,
    revision: u64,
    trading_replay_lamports: u64,
    core_replay_rent_lamports: u64,
    hoard_lamports: u64,
    rent_credit_lamports: u64,
    payer_lamports: u64,
}

impl RetirementReplayHandoffRequestV1 {
    /// Construct one canonical request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        market: [u8; 32],
        context: [u8; 32],
        trading_replay_digest: [u8; 32],
        hoard_data_digest: [u8; 32],
        generation: u64,
        revision: u64,
        trading_replay_lamports: u64,
        core_replay_rent_lamports: u64,
        hoard_lamports: u64,
        rent_credit_lamports: u64,
        payer_lamports: u64,
    ) -> Result<Self, RetirementReplayHandoffErrorV1> {
        let value = Self {
            market,
            context,
            trading_replay_digest,
            hoard_data_digest,
            generation,
            revision,
            trading_replay_lamports,
            core_replay_rent_lamports,
            hoard_lamports,
            rent_credit_lamports,
            payer_lamports,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), RetirementReplayHandoffErrorV1> {
        if [
            self.market,
            self.context,
            self.trading_replay_digest,
            self.hoard_data_digest,
        ]
        .iter()
        .any(|value| value.iter().all(|byte| *byte == 0))
            || self.generation == 0
            || self.revision == 0
            || self.trading_replay_lamports == 0
            || self.core_replay_rent_lamports == 0
            || self.hoard_lamports == 0
            || self.rent_credit_lamports == 0
            || self.payer_lamports < self.core_replay_rent_lamports
        {
            return Err(RetirementReplayHandoffErrorV1::InvalidCoordinate);
        }
        self.rent_credit_lamports
            .checked_add(self.trading_replay_lamports)
            .ok_or(RetirementReplayHandoffErrorV1::Arithmetic)?;
        Ok(())
    }

    /// Decode hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementReplayHandoffErrorV1> {
        require_header(
            input,
            &RETIREMENT_REPLAY_HANDOFF_REQUEST_MAGIC_V1,
            Self::BYTES,
        )?;
        if input
            .get(200..Self::BYTES)
            .ok_or(RetirementReplayHandoffErrorV1::InvalidBytes)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(RetirementReplayHandoffErrorV1::InvalidBytes);
        }
        let value = Self::new(
            array(input, MARKET_OFFSET)?,
            array(input, CONTEXT_OFFSET)?,
            array(input, TRADING_DIGEST_OFFSET)?,
            array(input, HOARD_DIGEST_OFFSET)?,
            u64_at(input, GENERATION_OFFSET)?,
            u64_at(input, REVISION_OFFSET)?,
            u64_at(input, TRADING_LAMPORTS_OFFSET)?,
            u64_at(input, CORE_RENT_OFFSET)?,
            u64_at(input, HOARD_LAMPORTS_OFFSET)?,
            u64_at(input, RENT_CREDIT_OFFSET)?,
            u64_at(input, PAYER_OFFSET)?,
        )?;
        Ok(value)
    }

    /// Encode canonical bytes.
    pub fn to_bytes(self) -> [u8; RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1] {
        let mut output = [0_u8; RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1];
        output[..8].copy_from_slice(&RETIREMENT_REPLAY_HANDOFF_REQUEST_MAGIC_V1);
        output[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
        put(&mut output, MARKET_OFFSET, &self.market);
        put(&mut output, CONTEXT_OFFSET, &self.context);
        put(
            &mut output,
            TRADING_DIGEST_OFFSET,
            &self.trading_replay_digest,
        );
        put(&mut output, HOARD_DIGEST_OFFSET, &self.hoard_data_digest);
        put_u64(&mut output, GENERATION_OFFSET, self.generation);
        put_u64(&mut output, REVISION_OFFSET, self.revision);
        put_u64(
            &mut output,
            TRADING_LAMPORTS_OFFSET,
            self.trading_replay_lamports,
        );
        put_u64(
            &mut output,
            CORE_RENT_OFFSET,
            self.core_replay_rent_lamports,
        );
        put_u64(&mut output, HOARD_LAMPORTS_OFFSET, self.hoard_lamports);
        put_u64(&mut output, RENT_CREDIT_OFFSET, self.rent_credit_lamports);
        put_u64(&mut output, PAYER_OFFSET, self.payer_lamports);
        output
    }

    /// Exact wire width.
    pub const BYTES: usize = RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1;
    /// Core Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Claims-owned aggregate Custody context.
    pub const fn context(self) -> [u8; 32] {
        self.context
    }
    /// Expected digest of the live Trading replay bytes.
    pub const fn trading_replay_digest(self) -> [u8; 32] {
        self.trading_replay_digest
    }
    /// Expected digest of immutable Hoard token-account bytes.
    pub const fn hoard_data_digest(self) -> [u8; 32] {
        self.hoard_data_digest
    }
    /// Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Exact replay revision transferred without advancement.
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Source replay lamports refunded to RentCredit.
    pub const fn trading_replay_lamports(self) -> u64 {
        self.trading_replay_lamports
    }
    /// Current exact rent prepaid for the Core replay.
    pub const fn core_replay_rent_lamports(self) -> u64 {
        self.core_replay_rent_lamports
    }
    /// Expected Hoard lamports.
    pub const fn hoard_lamports(self) -> u64 {
        self.hoard_lamports
    }
    /// Expected RentCredit lamports before the refund.
    pub const fn rent_credit_lamports(self) -> u64 {
        self.rent_credit_lamports
    }
    /// Expected payer lamports before the prepayment.
    pub const fn payer_lamports(self) -> u64 {
        self.payer_lamports
    }
}

/// Adapter-authenticated accounts and balances supplied to the semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffObservationV1 {
    /// Current Core program.
    pub core_program: [u8; 32],
    /// Current Trading program.
    pub trading_program: [u8; 32],
    /// Trading replay account.
    pub trading_replay: [u8; 32],
    /// Vacant Core replay account.
    pub core_replay: [u8; 32],
    /// Shared Hoard Vault.
    pub hoard_vault: [u8; 32],
    /// Immutable RentCredit.
    pub rent_credit: [u8; 32],
    /// Hostile-decoded Trading replay.
    pub replay: CustodyReplayV1,
    /// Digest of exact Trading replay bytes.
    pub trading_replay_digest: [u8; 32],
    /// Digest of exact Hoard bytes.
    pub hoard_data_digest: [u8; 32],
    /// Trading replay lamports.
    pub trading_replay_lamports: u64,
    /// Core replay lamports before creation, canonically zero.
    pub core_replay_lamports: u64,
    /// Hoard lamports.
    pub hoard_lamports: u64,
    /// RentCredit lamports.
    pub rent_credit_lamports: u64,
    /// Payer lamports.
    pub payer_lamports: u64,
}

/// Planned Core replay and exhaustive physical balance commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffPlanV1 {
    core_replay: CustodyReplayV1,
    receipt: RetirementReplayHandoffReceiptV1,
}

impl RetirementReplayHandoffPlanV1 {
    /// Construct the sole cross-role retirement handoff.
    pub fn new(
        request: RetirementReplayHandoffRequestV1,
        request_digest: [u8; 32],
        observed: RetirementReplayHandoffObservationV1,
        core_replay_post_digest: [u8; 32],
    ) -> Result<Self, RetirementReplayHandoffErrorV1> {
        request.validate()?;
        let replay = observed.replay;
        if request_digest.iter().all(|byte| *byte == 0)
            || core_replay_post_digest.iter().all(|byte| *byte == 0)
            || observed.trading_replay == observed.core_replay
            || replay.caller_role != ExecutionRoleV1::Trading
            || replay.market != request.market
            || replay.context != request.context
            || replay.caller_program != observed.trading_program
            || replay.rent_refund != observed.rent_credit
            || replay.open_vault_count != 1
            || replay.next_revision != request.revision
            || replay.generation != request.generation
            || observed.trading_replay_digest != request.trading_replay_digest
            || observed.hoard_data_digest != request.hoard_data_digest
            || observed.trading_replay_lamports != request.trading_replay_lamports
            || observed.core_replay_lamports != 0
            || observed.hoard_lamports != request.hoard_lamports
            || observed.rent_credit_lamports != request.rent_credit_lamports
            || observed.payer_lamports != request.payer_lamports
        {
            return Err(RetirementReplayHandoffErrorV1::ObservationMismatch);
        }
        let core_replay = CustodyReplayV1 {
            caller_role: ExecutionRoleV1::Core,
            caller_program: observed.core_program,
            ..replay
        };
        let canonical_digest = digest_replay_bytes(core_replay)?;
        if canonical_digest != core_replay_post_digest {
            return Err(RetirementReplayHandoffErrorV1::ObservationMismatch);
        }
        let rent_after = observed
            .rent_credit_lamports
            .checked_add(observed.trading_replay_lamports)
            .ok_or(RetirementReplayHandoffErrorV1::Arithmetic)?;
        let payer_after = observed
            .payer_lamports
            .checked_sub(request.core_replay_rent_lamports)
            .ok_or(RetirementReplayHandoffErrorV1::Arithmetic)?;
        let receipt = RetirementReplayHandoffReceiptV1 {
            request_digest,
            market: request.market,
            context: request.context,
            trading_replay: observed.trading_replay,
            core_replay: observed.core_replay,
            hoard_vault: observed.hoard_vault,
            trading_replay_pre_digest: observed.trading_replay_digest,
            core_replay_post_digest,
            hoard_pre_data_digest: observed.hoard_data_digest,
            hoard_post_data_digest: observed.hoard_data_digest,
            lineage_request_digest: replay.last_request_digest,
            lineage_poststate_digest: replay.last_poststate_commitment,
            generation: request.generation,
            revision: request.revision,
            trading_replay_pre_lamports: observed.trading_replay_lamports,
            trading_replay_post_lamports: 0,
            core_replay_pre_lamports: 0,
            core_replay_post_lamports: request.core_replay_rent_lamports,
            hoard_pre_lamports: observed.hoard_lamports,
            hoard_post_lamports: observed.hoard_lamports,
            rent_credit_pre_lamports: observed.rent_credit_lamports,
            rent_credit_post_lamports: rent_after,
            payer_pre_lamports: observed.payer_lamports,
            payer_post_lamports: payer_after,
            core_replay_rent_lamports: request.core_replay_rent_lamports,
            source_refund_lamports: observed.trading_replay_lamports,
        };
        receipt.validate()?;
        Ok(Self {
            core_replay,
            receipt,
        })
    }

    /// Exact Core-role replay bytes to persist.
    pub const fn core_replay(self) -> CustodyReplayV1 {
        self.core_replay
    }
    /// Exact immediate receipt.
    pub const fn receipt(self) -> RetirementReplayHandoffReceiptV1 {
        self.receipt
    }
}

/// Exact immediate receipt for the cross-role handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffReceiptV1 {
    /// SHA-256 of the request.
    pub request_digest: [u8; 32],
    /// Core Market.
    pub market: [u8; 32],
    /// Claims-owned Custody context.
    pub context: [u8; 32],
    /// Closed Trading replay.
    pub trading_replay: [u8; 32],
    /// Created Core replay.
    pub core_replay: [u8; 32],
    /// Unchanged Hoard Vault.
    pub hoard_vault: [u8; 32],
    /// Pre-handoff Trading replay digest.
    pub trading_replay_pre_digest: [u8; 32],
    /// Post-handoff Core replay digest.
    pub core_replay_post_digest: [u8; 32],
    /// Hoard data digest before.
    pub hoard_pre_data_digest: [u8; 32],
    /// Hoard data digest after.
    pub hoard_post_data_digest: [u8; 32],
    /// Preserved replay lineage request digest.
    pub lineage_request_digest: [u8; 32],
    /// Preserved replay lineage poststate digest.
    pub lineage_poststate_digest: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// Preserved next revision.
    pub revision: u64,
    /// Trading replay lamports before.
    pub trading_replay_pre_lamports: u64,
    /// Trading replay lamports after.
    pub trading_replay_post_lamports: u64,
    /// Core replay lamports before.
    pub core_replay_pre_lamports: u64,
    /// Core replay lamports after.
    pub core_replay_post_lamports: u64,
    /// Hoard lamports before.
    pub hoard_pre_lamports: u64,
    /// Hoard lamports after.
    pub hoard_post_lamports: u64,
    /// RentCredit lamports before.
    pub rent_credit_pre_lamports: u64,
    /// RentCredit lamports after.
    pub rent_credit_post_lamports: u64,
    /// Payer lamports before.
    pub payer_pre_lamports: u64,
    /// Payer lamports after.
    pub payer_post_lamports: u64,
    /// Exact Core replay rent charged.
    pub core_replay_rent_lamports: u64,
    /// Exact Trading replay rent refunded.
    pub source_refund_lamports: u64,
}

impl RetirementReplayHandoffReceiptV1 {
    fn validate(self) -> Result<(), RetirementReplayHandoffErrorV1> {
        if [
            self.request_digest,
            self.market,
            self.context,
            self.trading_replay,
            self.core_replay,
            self.hoard_vault,
            self.trading_replay_pre_digest,
            self.core_replay_post_digest,
            self.hoard_pre_data_digest,
            self.hoard_post_data_digest,
            self.lineage_request_digest,
            self.lineage_poststate_digest,
        ]
        .iter()
        .any(|value| value.iter().all(|byte| *byte == 0))
            || self.trading_replay == self.core_replay
            || self.generation == 0
            || self.revision == 0
            || self.trading_replay_pre_lamports == 0
            || self.trading_replay_post_lamports != 0
            || self.core_replay_pre_lamports != 0
            || self.core_replay_post_lamports != self.core_replay_rent_lamports
            || self.core_replay_rent_lamports == 0
            || self.source_refund_lamports != self.trading_replay_pre_lamports
            || self.hoard_pre_data_digest != self.hoard_post_data_digest
            || self.hoard_pre_lamports != self.hoard_post_lamports
            || self
                .rent_credit_pre_lamports
                .checked_add(self.source_refund_lamports)
                != Some(self.rent_credit_post_lamports)
            || self
                .payer_post_lamports
                .checked_add(self.core_replay_rent_lamports)
                != Some(self.payer_pre_lamports)
        {
            return Err(RetirementReplayHandoffErrorV1::InvalidReceipt);
        }
        Ok(())
    }

    /// Decode hostile receipt bytes.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementReplayHandoffErrorV1> {
        require_header(
            input,
            &RETIREMENT_REPLAY_HANDOFF_RECEIPT_MAGIC_V1,
            Self::BYTES,
        )?;
        let value = Self {
            request_digest: array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            market: array(input, RECEIPT_MARKET_OFFSET)?,
            context: array(input, RECEIPT_CONTEXT_OFFSET)?,
            trading_replay: array(input, RECEIPT_TRADING_REPLAY_OFFSET)?,
            core_replay: array(input, RECEIPT_CORE_REPLAY_OFFSET)?,
            hoard_vault: array(input, RECEIPT_HOARD_OFFSET)?,
            trading_replay_pre_digest: array(input, RECEIPT_TRADING_DIGEST_OFFSET)?,
            core_replay_post_digest: array(input, RECEIPT_CORE_DIGEST_OFFSET)?,
            hoard_pre_data_digest: array(input, RECEIPT_HOARD_PRE_DIGEST_OFFSET)?,
            hoard_post_data_digest: array(input, RECEIPT_HOARD_POST_DIGEST_OFFSET)?,
            lineage_request_digest: array(input, RECEIPT_LINEAGE_REQUEST_OFFSET)?,
            lineage_poststate_digest: array(input, RECEIPT_LINEAGE_POSTSTATE_OFFSET)?,
            generation: u64_at(input, RECEIPT_GENERATION_OFFSET)?,
            revision: u64_at(input, RECEIPT_REVISION_OFFSET)?,
            trading_replay_pre_lamports: u64_at(input, RECEIPT_TRADING_PRE_LAMPORTS_OFFSET)?,
            trading_replay_post_lamports: u64_at(input, RECEIPT_TRADING_POST_LAMPORTS_OFFSET)?,
            core_replay_pre_lamports: u64_at(input, RECEIPT_CORE_PRE_LAMPORTS_OFFSET)?,
            core_replay_post_lamports: u64_at(input, RECEIPT_CORE_POST_LAMPORTS_OFFSET)?,
            hoard_pre_lamports: u64_at(input, RECEIPT_HOARD_PRE_LAMPORTS_OFFSET)?,
            hoard_post_lamports: u64_at(input, RECEIPT_HOARD_POST_LAMPORTS_OFFSET)?,
            rent_credit_pre_lamports: u64_at(input, RECEIPT_RENT_PRE_LAMPORTS_OFFSET)?,
            rent_credit_post_lamports: u64_at(input, RECEIPT_RENT_POST_LAMPORTS_OFFSET)?,
            payer_pre_lamports: u64_at(input, RECEIPT_PAYER_PRE_LAMPORTS_OFFSET)?,
            payer_post_lamports: u64_at(input, RECEIPT_PAYER_POST_LAMPORTS_OFFSET)?,
            core_replay_rent_lamports: u64_at(input, RECEIPT_CORE_RENT_OFFSET)?,
            source_refund_lamports: u64_at(input, RECEIPT_SOURCE_REFUND_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode canonical receipt bytes.
    pub fn to_bytes(self) -> [u8; RETIREMENT_REPLAY_HANDOFF_RECEIPT_BYTES_V1] {
        let mut output = [0_u8; RETIREMENT_REPLAY_HANDOFF_RECEIPT_BYTES_V1];
        output[..8].copy_from_slice(&RETIREMENT_REPLAY_HANDOFF_RECEIPT_MAGIC_V1);
        output[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
        for (offset, value) in [
            (RECEIPT_REQUEST_DIGEST_OFFSET, self.request_digest),
            (RECEIPT_MARKET_OFFSET, self.market),
            (RECEIPT_CONTEXT_OFFSET, self.context),
            (RECEIPT_TRADING_REPLAY_OFFSET, self.trading_replay),
            (RECEIPT_CORE_REPLAY_OFFSET, self.core_replay),
            (RECEIPT_HOARD_OFFSET, self.hoard_vault),
            (
                RECEIPT_TRADING_DIGEST_OFFSET,
                self.trading_replay_pre_digest,
            ),
            (RECEIPT_CORE_DIGEST_OFFSET, self.core_replay_post_digest),
            (RECEIPT_HOARD_PRE_DIGEST_OFFSET, self.hoard_pre_data_digest),
            (
                RECEIPT_HOARD_POST_DIGEST_OFFSET,
                self.hoard_post_data_digest,
            ),
            (RECEIPT_LINEAGE_REQUEST_OFFSET, self.lineage_request_digest),
            (
                RECEIPT_LINEAGE_POSTSTATE_OFFSET,
                self.lineage_poststate_digest,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (RECEIPT_GENERATION_OFFSET, self.generation),
            (RECEIPT_REVISION_OFFSET, self.revision),
            (
                RECEIPT_TRADING_PRE_LAMPORTS_OFFSET,
                self.trading_replay_pre_lamports,
            ),
            (
                RECEIPT_TRADING_POST_LAMPORTS_OFFSET,
                self.trading_replay_post_lamports,
            ),
            (
                RECEIPT_CORE_PRE_LAMPORTS_OFFSET,
                self.core_replay_pre_lamports,
            ),
            (
                RECEIPT_CORE_POST_LAMPORTS_OFFSET,
                self.core_replay_post_lamports,
            ),
            (RECEIPT_HOARD_PRE_LAMPORTS_OFFSET, self.hoard_pre_lamports),
            (RECEIPT_HOARD_POST_LAMPORTS_OFFSET, self.hoard_post_lamports),
            (
                RECEIPT_RENT_PRE_LAMPORTS_OFFSET,
                self.rent_credit_pre_lamports,
            ),
            (
                RECEIPT_RENT_POST_LAMPORTS_OFFSET,
                self.rent_credit_post_lamports,
            ),
            (RECEIPT_PAYER_PRE_LAMPORTS_OFFSET, self.payer_pre_lamports),
            (RECEIPT_PAYER_POST_LAMPORTS_OFFSET, self.payer_post_lamports),
            (RECEIPT_CORE_RENT_OFFSET, self.core_replay_rent_lamports),
            (RECEIPT_SOURCE_REFUND_OFFSET, self.source_refund_lamports),
        ] {
            put_u64(&mut output, offset, value);
        }
        output
    }

    /// Exact receipt width.
    pub const BYTES: usize = RETIREMENT_REPLAY_HANDOFF_RECEIPT_BYTES_V1;
}

fn digest_replay_bytes(
    replay: CustodyReplayV1,
) -> Result<[u8; 32], RetirementReplayHandoffErrorV1> {
    let bytes = replay
        .to_bytes()
        .map_err(|_| RetirementReplayHandoffErrorV1::InvalidTradingReplay)?;
    Ok(dclutch_sha256_adapter::digest(&bytes))
}

fn require_header(
    input: &[u8],
    magic: &[u8; 8],
    width: usize,
) -> Result<(), RetirementReplayHandoffErrorV1> {
    if input.len() != width
        || input.get(..8) != Some(magic.as_slice())
        || input.get(8..10) != Some(VERSION_V1.to_le_bytes().as_slice())
        || input
            .get(10..16)
            .ok_or(RetirementReplayHandoffErrorV1::InvalidBytes)?
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(RetirementReplayHandoffErrorV1::InvalidBytes);
    }
    Ok(())
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32], RetirementReplayHandoffErrorV1> {
    input
        .get(offset..offset.saturating_add(32))
        .ok_or(RetirementReplayHandoffErrorV1::InvalidBytes)?
        .try_into()
        .map_err(|_| RetirementReplayHandoffErrorV1::InvalidBytes)
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64, RetirementReplayHandoffErrorV1> {
    let bytes: [u8; 8] = input
        .get(offset..offset.saturating_add(8))
        .ok_or(RetirementReplayHandoffErrorV1::InvalidBytes)?
        .try_into()
        .map_err(|_| RetirementReplayHandoffErrorV1::InvalidBytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn put(output: &mut [u8], offset: usize, value: &[u8; 32]) {
    if let Some(slot) = output.get_mut(offset..offset.saturating_add(32)) {
        slot.copy_from_slice(value);
    }
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    if let Some(slot) = output.get_mut(offset..offset.saturating_add(8)) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
}

#[cfg(test)]
mod account_layout_tests {
    use super::*;

    #[test]
    fn handoff_roles_are_contiguous_and_exact() {
        assert_eq!(
            [
                RetirementReplayHandoffAccountLayoutV1::PAYER,
                RetirementReplayHandoffAccountLayoutV1::MARKET,
                RetirementReplayHandoffAccountLayoutV1::CACHE,
                RetirementReplayHandoffAccountLayoutV1::REGISTRY,
                RetirementReplayHandoffAccountLayoutV1::CORE_PROGRAM,
                RetirementReplayHandoffAccountLayoutV1::CORE_PROGRAMDATA,
                RetirementReplayHandoffAccountLayoutV1::TRADING_PROGRAM,
                RetirementReplayHandoffAccountLayoutV1::TRADING_PROGRAMDATA,
                RetirementReplayHandoffAccountLayoutV1::CUSTODY_PROGRAM,
                RetirementReplayHandoffAccountLayoutV1::CUSTODY_PROGRAMDATA,
                RetirementReplayHandoffAccountLayoutV1::CALLER_AUTHORITY,
                RetirementReplayHandoffAccountLayoutV1::CLAIMS_AGGREGATE,
                RetirementReplayHandoffAccountLayoutV1::REALM,
                RetirementReplayHandoffAccountLayoutV1::REALM_STAGING,
                RetirementReplayHandoffAccountLayoutV1::RENT,
                RetirementReplayHandoffAccountLayoutV1::RENT_CREDIT,
                RetirementReplayHandoffAccountLayoutV1::TRADING_REPLAY,
                RetirementReplayHandoffAccountLayoutV1::CORE_REPLAY,
                RetirementReplayHandoffAccountLayoutV1::HOARD,
                RetirementReplayHandoffAccountLayoutV1::SYSTEM,
                RetirementReplayHandoffAccountLayoutV1::MINT,
                RetirementReplayHandoffAccountLayoutV1::TOKEN_PROGRAM,
                RetirementReplayHandoffAccountLayoutV1::CUSTODY_AUTHORITY,
            ],
            core::array::from_fn::<_, 23, _>(|index| index)
        );
        assert_eq!(RetirementReplayHandoffAccountLayoutV1::COUNT, 23);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request() -> RetirementReplayHandoffRequestV1 {
        RetirementReplayHandoffRequestV1::new(
            id(1),
            id(2),
            id(3),
            id(4),
            7,
            9,
            100,
            120,
            130,
            140,
            1_000,
        )
        .expect("request")
    }

    fn replay() -> CustodyReplayV1 {
        CustodyReplayV1 {
            caller_role: ExecutionRoleV1::Trading,
            release_set: id(5),
            market: id(1),
            realm: id(6),
            context: id(2),
            caller_program: id(7),
            rent_refund: id(8),
            open_vault_count: 1,
            next_revision: 9,
            generation: 7,
            last_request_digest: id(9),
            last_poststate_commitment: id(10),
        }
    }

    fn observation() -> RetirementReplayHandoffObservationV1 {
        RetirementReplayHandoffObservationV1 {
            core_program: id(11),
            trading_program: id(7),
            trading_replay: id(12),
            core_replay: id(13),
            hoard_vault: id(14),
            rent_credit: id(8),
            replay: replay(),
            trading_replay_digest: id(3),
            hoard_data_digest: id(4),
            trading_replay_lamports: 100,
            core_replay_lamports: 0,
            hoard_lamports: 130,
            rent_credit_lamports: 140,
            payer_lamports: 1_000,
        }
    }

    #[test]
    fn fixed_request_round_trip_refuses_reserved_and_truncation() {
        let bytes = request().to_bytes();
        assert_eq!(
            RetirementReplayHandoffRequestV1::decode(&bytes),
            Ok(request())
        );
        assert!(RetirementReplayHandoffRequestV1::decode(&bytes[..207]).is_err());
        let mut reserved = bytes;
        reserved[10] = 1;
        assert!(RetirementReplayHandoffRequestV1::decode(&reserved).is_err());
        let mut trailing_reserved = bytes;
        trailing_reserved[207] = 1;
        assert!(RetirementReplayHandoffRequestV1::decode(&trailing_reserved).is_err());
    }

    #[test]
    fn exact_handoff_preserves_lineage_and_commits_every_balance() {
        let expected = CustodyReplayV1 {
            caller_role: ExecutionRoleV1::Core,
            caller_program: id(11),
            ..replay()
        };
        let expected_digest = digest_replay_bytes(expected).expect("digest");
        let plan =
            RetirementReplayHandoffPlanV1::new(request(), id(15), observation(), expected_digest)
                .expect("plan");
        assert_eq!(plan.core_replay(), expected);
        let receipt = plan.receipt();
        assert_eq!(receipt.rent_credit_post_lamports, 240);
        assert_eq!(receipt.payer_post_lamports, 880);
        assert_eq!(
            receipt.hoard_pre_data_digest,
            receipt.hoard_post_data_digest
        );
        assert_eq!(
            RetirementReplayHandoffReceiptV1::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
    }

    #[test]
    fn role_count_revision_digest_and_partial_destination_refuse() {
        let expected = CustodyReplayV1 {
            caller_role: ExecutionRoleV1::Core,
            caller_program: id(11),
            ..replay()
        };
        let digest = digest_replay_bytes(expected).expect("digest");
        for hostile in [
            RetirementReplayHandoffObservationV1 {
                replay: CustodyReplayV1 {
                    open_vault_count: 0,
                    ..replay()
                },
                ..observation()
            },
            RetirementReplayHandoffObservationV1 {
                replay: CustodyReplayV1 {
                    next_revision: 10,
                    ..replay()
                },
                ..observation()
            },
            RetirementReplayHandoffObservationV1 {
                trading_replay_digest: id(99),
                ..observation()
            },
            RetirementReplayHandoffObservationV1 {
                core_replay_lamports: 1,
                ..observation()
            },
        ] {
            assert!(
                RetirementReplayHandoffPlanV1::new(request(), id(15), hostile, digest).is_err()
            );
        }
    }
}
