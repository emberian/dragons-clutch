//! Exact persisted principal/donation accounting for Source lifecycle custody.

use clutch_source_plane_v3::ContentId;

use crate::auth::{domain_id, RuntimeKey};
use crate::{Error, Result};

const SOURCE_FUNDING_CUSTODY_MAGIC: [u8; 8] =
    [0xbd, 1, b'D', b'C', b'S', b'C', b'V', b'1'];
const SOURCE_FUNDING_CUSTODY_DOMAIN: &[u8] =
    b"dragons-clutch/source-funding-custody-ledger/v1";
const SOURCE_FUNDING_CUSTODY_TRANSITION_DOMAIN: &[u8] =
    b"dragons-clutch/source-funding-custody-transition/v1";

/// Registered current Source lifecycle-custody account discriminator.
pub const SOURCE_FUNDING_CUSTODY_ACCOUNT_TAG: u8 = SOURCE_FUNDING_CUSTODY_MAGIC[0];
/// Current Source lifecycle-custody account version.
pub const SOURCE_FUNDING_CUSTODY_ACCOUNT_VERSION: u8 = SOURCE_FUNDING_CUSTODY_MAGIC[1];
/// Exact fixed width of [`SourceFundingCustodyLedgerV1`].
pub const SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES: usize = 336;

/// Sole persisted owner of prepaid Source principal and unsolicited lamports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceFundingCustodyLedgerV1 {
    /// Exact adapter program owning this body and its lamports.
    pub adapter_program: RuntimeKey,
    /// Immutable checked Source release.
    pub release_manifest_id: ContentId,
    /// Fully authenticated real-provider route.
    pub route_id: ContentId,
    /// Exact bounded Source work schedule.
    pub source_work_schedule_id: ContentId,
    /// Product/Series-scoped Source lifecycle.
    pub lifecycle_id: ContentId,
    /// Physical content-addressed custody PDA.
    pub custody_account: RuntimeKey,
    /// Immutable FundingTerms lamport principal refund.
    pub principal_refund: RuntimeKey,
    /// Release-selected donation/surplus sink.
    pub neutral_sink: RuntimeKey,
    /// Original SourceWork allocation transferred from Funding.
    pub allocated_principal_lamports: u64,
    /// Exact unspent or recycled payer principal still held here.
    pub remaining_principal_lamports: u64,
    /// Cumulative unsolicited custody lamports observed so far.
    pub donation_lamports: u64,
    /// Monotone ledger mutation sequence, beginning at one.
    pub transition_sequence: u64,
    /// Exact private semantic postwrite authorizing the last mutation.
    pub last_transition_id: ContentId,
}

impl SourceFundingCustodyLedgerV1 {
    /// Initialize exact capitalized principal under Product's private preauth.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        adapter_program: RuntimeKey,
        release_manifest_id: ContentId,
        route_id: ContentId,
        source_work_schedule_id: ContentId,
        lifecycle_id: ContentId,
        custody_account: RuntimeKey,
        principal_refund: RuntimeKey,
        neutral_sink: RuntimeKey,
        allocated_principal_lamports: u64,
        capitalization_authority_id: ContentId,
    ) -> Result<Self> {
        let value = Self {
            adapter_program,
            release_manifest_id,
            route_id,
            source_work_schedule_id,
            lifecycle_id,
            custody_account,
            principal_refund,
            neutral_sink,
            allocated_principal_lamports,
            remaining_principal_lamports: allocated_principal_lamports,
            donation_lamports: 0,
            transition_sequence: 1,
            last_transition_id: capitalization_authority_id,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate the canonical principal/donation partition.
    pub fn validate(&self) -> Result<()> {
        self.adapter_program.validate()?;
        self.custody_account.validate()?;
        self.principal_refund.validate()?;
        self.neutral_sink.validate()?;
        for id in [
            self.release_manifest_id,
            self.route_id,
            self.source_work_schedule_id,
            self.lifecycle_id,
            self.last_transition_id,
        ] {
            if id.is_zero() {
                return Err(Error::ZeroIdentity);
            }
        }
        if self.allocated_principal_lamports == 0
            || self.remaining_principal_lamports > self.allocated_principal_lamports
            || self.transition_sequence == 0
            || self.adapter_program == self.custody_account
            || self.adapter_program == self.principal_refund
            || self.adapter_program == self.neutral_sink
            || self.custody_account == self.principal_refund
            || self.custody_account == self.neutral_sink
            || self.principal_refund == self.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Apply one exact physical post-balance after a principal debit/refund.
    /// Any excess not explained by principal is permanently classified as a
    /// donation and can never later be refunded as payer principal.
    pub fn transition(
        self,
        principal_debit_lamports: u64,
        principal_credit_lamports: u64,
        physical_balance_after: u64,
        semantic_postwrite_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        if semantic_postwrite_id.is_zero()
            || (principal_debit_lamports == 0 && principal_credit_lamports == 0)
        {
            return Err(Error::MismatchedBinding);
        }
        let remaining_after_debit = self
            .remaining_principal_lamports
            .checked_sub(principal_debit_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        let remaining_principal_lamports = remaining_after_debit
            .checked_add(principal_credit_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if remaining_principal_lamports > self.allocated_principal_lamports {
            return Err(Error::MismatchedBinding);
        }
        let explained_balance = remaining_principal_lamports
            .checked_add(self.donation_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        let newly_observed_donation = physical_balance_after
            .checked_sub(explained_balance)
            .ok_or(Error::MismatchedBinding)?;
        let donation_lamports = self
            .donation_lamports
            .checked_add(newly_observed_donation)
            .ok_or(Error::ArithmeticOverflow)?;
        let transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let last_transition_id = domain_id(
            SOURCE_FUNDING_CUSTODY_TRANSITION_DOMAIN,
            &transition_preimage(
                self.id()?,
                semantic_postwrite_id,
                principal_debit_lamports,
                principal_credit_lamports,
                physical_balance_after,
                remaining_principal_lamports,
                donation_lamports,
                transition_sequence,
            ),
        );
        let value = Self {
            remaining_principal_lamports,
            donation_lamports,
            transition_sequence,
            last_transition_id,
            ..self
        };
        value.validate()?;
        Ok(value)
    }

    /// Observe a final balance without inventing a zero-value transition.
    pub fn observe_terminal_balance(
        self,
        physical_balance: u64,
        semantic_postwrite_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        if semantic_postwrite_id.is_zero() {
            return Err(Error::ZeroIdentity);
        }
        let explained = self
            .remaining_principal_lamports
            .checked_add(self.donation_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        let newly_observed_donation = physical_balance
            .checked_sub(explained)
            .ok_or(Error::MismatchedBinding)?;
        if newly_observed_donation == 0 {
            return Ok(self);
        }
        let donation_lamports = self
            .donation_lamports
            .checked_add(newly_observed_donation)
            .ok_or(Error::ArithmeticOverflow)?;
        let transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let last_transition_id = domain_id(
            SOURCE_FUNDING_CUSTODY_TRANSITION_DOMAIN,
            &transition_preimage(
                self.id()?,
                semantic_postwrite_id,
                0,
                0,
                physical_balance,
                self.remaining_principal_lamports,
                donation_lamports,
                transition_sequence,
            ),
        );
        let value = Self {
            donation_lamports,
            transition_sequence,
            last_transition_id,
            ..self
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode exact hostile-account bytes.
    pub fn encode(&self) -> Result<[u8; SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES]> {
        self.validate()?;
        let mut out = [0_u8; SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES];
        out[..8].copy_from_slice(&SOURCE_FUNDING_CUSTODY_MAGIC);
        out[16..48].copy_from_slice(&self.adapter_program.bytes());
        out[48..80].copy_from_slice(&self.release_manifest_id.bytes());
        out[80..112].copy_from_slice(&self.route_id.bytes());
        out[112..144].copy_from_slice(&self.source_work_schedule_id.bytes());
        out[144..176].copy_from_slice(&self.lifecycle_id.bytes());
        out[176..208].copy_from_slice(&self.custody_account.bytes());
        out[208..240].copy_from_slice(&self.principal_refund.bytes());
        out[240..272].copy_from_slice(&self.neutral_sink.bytes());
        out[272..280].copy_from_slice(&self.allocated_principal_lamports.to_le_bytes());
        out[280..288].copy_from_slice(&self.remaining_principal_lamports.to_le_bytes());
        out[288..296].copy_from_slice(&self.donation_lamports.to_le_bytes());
        out[296..304].copy_from_slice(&self.transition_sequence.to_le_bytes());
        out[304..336].copy_from_slice(&self.last_transition_id.bytes());
        Ok(out)
    }

    /// Decode exact canonical bytes and refuse reserved-byte aliases.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
            || input[..8] != SOURCE_FUNDING_CUSTODY_MAGIC
            || input[8..16].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        let value = Self {
            adapter_program: key_at(input, 16),
            release_manifest_id: id_at(input, 48),
            route_id: id_at(input, 80),
            source_work_schedule_id: id_at(input, 112),
            lifecycle_id: id_at(input, 144),
            custody_account: key_at(input, 176),
            principal_refund: key_at(input, 208),
            neutral_sink: key_at(input, 240),
            allocated_principal_lamports: le_u64(input, 272),
            remaining_principal_lamports: le_u64(input, 280),
            donation_lamports: le_u64(input, 288),
            transition_sequence: le_u64(input, 296),
            last_transition_id: id_at(input, 304),
        };
        value.validate()?;
        Ok(value)
    }

    /// Content identity of the exact current ledger state.
    pub fn id(&self) -> Result<ContentId> {
        Ok(domain_id(SOURCE_FUNDING_CUSTODY_DOMAIN, &self.encode()?))
    }
}

fn transition_preimage(
    before_id: ContentId,
    semantic_postwrite_id: ContentId,
    debit: u64,
    credit: u64,
    physical_after: u64,
    remaining_after: u64,
    donation_after: u64,
    sequence: u64,
) -> [u8; 112] {
    let mut out = [0_u8; 112];
    out[..32].copy_from_slice(&before_id.bytes());
    out[32..64].copy_from_slice(&semantic_postwrite_id.bytes());
    out[64..72].copy_from_slice(&debit.to_le_bytes());
    out[72..80].copy_from_slice(&credit.to_le_bytes());
    out[80..88].copy_from_slice(&physical_after.to_le_bytes());
    out[88..96].copy_from_slice(&remaining_after.to_le_bytes());
    out[96..104].copy_from_slice(&donation_after.to_le_bytes());
    out[104..112].copy_from_slice(&sequence.to_le_bytes());
    out
}

fn key_at(input: &[u8], at: usize) -> RuntimeKey {
    let mut value = [0_u8; 32];
    value.copy_from_slice(&input[at..at + 32]);
    RuntimeKey::from_bytes(value)
}

fn id_at(input: &[u8], at: usize) -> ContentId {
    ContentId::from_bytes(key_at(input, at).bytes())
}

fn le_u64(input: &[u8], at: usize) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&input[at..at + 8]);
    u64::from_le_bytes(value)
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    fn key(seed: u8) -> RuntimeKey {
        RuntimeKey::from_bytes([seed; 32])
    }

    fn id(seed: u8) -> ContentId {
        ContentId::from_bytes([seed; 32])
    }

    fn ledger() -> SourceFundingCustodyLedgerV1 {
        SourceFundingCustodyLedgerV1::new(
            key(1), id(2), id(3), id(4), id(5), key(6), key(7), key(8), 100, id(9),
        )
        .unwrap()
    }

    #[test]
    fn donation_after_spend_never_becomes_refundable_principal() {
        let spent = ledger().transition(40, 0, 60, id(10)).unwrap();
        let donated = spent.observe_terminal_balance(85, id(11)).unwrap();
        assert_eq!(donated.remaining_principal_lamports, 60);
        assert_eq!(donated.donation_lamports, 25);
    }

    #[test]
    fn exact_refund_restores_only_recorded_principal() {
        let spent = ledger().transition(40, 0, 60, id(10)).unwrap();
        let refunded = spent.transition(0, 15, 75, id(11)).unwrap();
        assert_eq!(refunded.remaining_principal_lamports, 75);
        assert_eq!(refunded.donation_lamports, 0);
    }

    #[test]
    fn forged_credit_and_undercollateralized_postbalance_refuse() {
        assert!(ledger().transition(0, 1, 101, id(10)).is_err());
        assert!(ledger().transition(1, 0, 98, id(10)).is_err());
    }

    #[test]
    fn codec_refuses_reserved_bytes() {
        let mut bytes = ledger().encode().unwrap();
        bytes[8] = 1;
        assert_eq!(
            SourceFundingCustodyLedgerV1::decode(&bytes),
            Err(Error::InvalidCodec)
        );
    }

    #[test]
    fn codec_accepts_registered_bd_and_refuses_withdrawn_af_alias() {
        let bytes = ledger().encode().unwrap();
        assert_eq!(bytes[0], 0xbd);
        assert_eq!(SourceFundingCustodyLedgerV1::decode(&bytes), Ok(ledger()));

        let mut withdrawn = bytes;
        withdrawn[0] = 0xaf;
        assert_eq!(
            SourceFundingCustodyLedgerV1::decode(&withdrawn),
            Err(Error::InvalidCodec)
        );
    }
}
