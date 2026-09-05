//! The rate a funding ledger was funded at, for a ledger with no field for it.
//!
//! A `FundingLedgerV2` header records the exemption-scaled rent rate its
//! founding paid, and every exactness check over the account prices against
//! that record rather than against the Rent sysvar of the moment. Ledgers
//! written before the field existed carry zero in that span -- the four bytes
//! the header used to reserve -- and a zero prices every account at nothing, so
//! `FundingLedgerV2::decode` refuses them rather than pricing them wrong.
//!
//! That is the right behaviour for a program and the wrong behaviour for a host
//! that must plan against a cohort already on chain. This module is the host's
//! recovery, and it recovers the rate from the ledger's OWN BYTES rather than
//! from anything a caller believes:
//!
//! ```text
//! rate = (lamports - remaining native principal) / (ACCOUNT_STORAGE_OVERHEAD + len)
//! ```
//!
//! which is `the_rate_is_recoverable_from_the_zero_length_minimum` and
//! `one_rate_prices_every_length` read backwards. Two things make it a
//! recovery rather than a guess:
//!
//! - **The division must be exact.** A single donated lamport puts the balance
//!   off the affine line and no rate reproduces it; that is refused by name,
//!   never rounded. It is the same hostile the recorded-rate path answers with
//!   `FundedRentNotEvidenced`, asked one layer earlier.
//! - **One founding is one rate.** `the_whole_cohort_is_one_rate`: every
//!   account a founding created was funded at the same cluster parameter, at
//!   every width. Readings from sibling accounts of the same founding are
//!   folded into one rate and any disagreement is refused.
//!
//! The recovered rate is spliced into a COPY of the ledger bytes so the rest of
//! the host reads one shape. **The copy is never written to chain**: the splice
//! is exactly what a cohort-16 founding writes there itself, and a cohort-15
//! account still holds the zeros it was created with.

use dclutch_market::capability_manifest::{
    CapabilityManifestV1, ContentId as CapabilityContentId, Error as CapabilityError,
    FundingLedgerV2,
    funding::{ACCOUNT_STORAGE_OVERHEAD_BYTES, funded_rent_minimum_v2},
};

use crate::ResolutionCoreOperatorErrorV3;

/// One reading of an account a founding created, funded, and has not touched.
///
/// `remaining_native_principal` is the native principal the account's own rows
/// still owe -- zero for an account that parks none. Everything else it holds
/// is the rent it was funded with, which is what makes the rate recoverable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundedRentReadingV2 {
    /// Exact account width at the moment of the reading.
    pub account_bytes: usize,
    /// Exact lamports the account holds.
    pub account_lamports: u64,
    /// Native principal this account's own rows still owe.
    pub remaining_native_principal: u64,
}

/// A funding ledger a host can plan against, and the rate it is priced at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredFundingLedgerV2 {
    /// The exemption-scaled rate the ledger's founding paid.
    pub funded_rent_rate: u32,
    /// The ledger bytes with that rate in the header span. Host-only.
    pub bytes: Vec<u8>,
    /// True when the rate was recovered because the header recorded none.
    pub recovered: bool,
}

/// The rate itself is what disagreed: a balance no rate reproduces, or two
/// accounts of one founding deriving two.
fn refuse(conjunct: &str) -> ResolutionCoreOperatorErrorV3 {
    eprintln!("funded-rent recovery refused: {conjunct}");
    ResolutionCoreOperatorErrorV3::FundedRentUnrecoverable
}

/// The LEDGER disagreed -- shape, manifest binding, or a decode that has
/// nothing to do with the rate. Keeps the code these paths always published.
fn refuse_funding(conjunct: &str) -> ResolutionCoreOperatorErrorV3 {
    eprintln!("funded-rent recovery refused: {conjunct}");
    ResolutionCoreOperatorErrorV3::Funding
}

/// Fold sibling readings of one founding into the single rate that prices them.
///
/// Refuses an empty reading set, a balance no rate reproduces exactly, and two
/// readings of one founding that disagree.
pub fn recover_funded_rent_rate_v2(
    readings: &[FundedRentReadingV2],
) -> Result<u32, ResolutionCoreOperatorErrorV3> {
    let mut agreed: Option<u32> = None;
    for reading in readings {
        let bytes = u64::try_from(reading.account_bytes).map_err(|_| refuse("account width"))?;
        let span = ACCOUNT_STORAGE_OVERHEAD_BYTES
            .checked_add(bytes)
            .ok_or_else(|| refuse("account span overflow"))?;
        let rent_lamports = reading
            .account_lamports
            .checked_sub(reading.remaining_native_principal)
            .ok_or_else(|| {
                refuse("account holds less than the native principal its own rows owe")
            })?;
        if span == 0 || rent_lamports % span != 0 {
            return Err(refuse(&format!(
                "no rate reproduces {} lamports less {} principal over {} bytes: \
                 {rent_lamports} is not a multiple of {span} (a donated lamport is \
                 not custody, and this division is never rounded)",
                reading.account_lamports, reading.remaining_native_principal, reading.account_bytes,
            )));
        }
        let rate = u32::try_from(rent_lamports / span).map_err(|_| refuse("rate exceeds u32"))?;
        if rate == 0 {
            return Err(refuse("a cluster charging no rent records no rate"));
        }
        // The closing loop: the recovered rate must reproduce the reading it
        // came from, through the same arithmetic every check downstream uses.
        if funded_rent_minimum_v2(rate, reading.account_bytes)
            .map_err(|_| refuse("recovered rate does not price its own reading"))?
            .checked_add(reading.remaining_native_principal)
            != Some(reading.account_lamports)
        {
            return Err(refuse("recovered rate does not reproduce its own reading"));
        }
        match agreed {
            None => agreed = Some(rate),
            Some(first) if first == rate => {}
            Some(first) => {
                return Err(refuse(&format!(
                    "one founding is one rate: a sibling reading of {} bytes derives \
                     {rate} where an earlier reading derived {first}",
                    reading.account_bytes,
                )));
            }
        }
    }
    agreed.ok_or_else(|| refuse("no reading to recover a rate from"))
}

/// Locate the four header bytes that carry the funded rate, without naming an
/// offset this module would then be a second author of.
///
/// The span is the one that is currently all zero and, filled with a probe
/// rate, yields a header that decodes to THAT rate with its manifest identity
/// and selected mask unchanged. Exactly one span in a legacy header does that:
/// a span overlapping the magic, schema or mask stops the header decoding, and
/// one overlapping the manifest identity stops it authenticating.
fn locate_funded_rent_span_v2(
    ledger_bytes: &[u8],
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    probe_rate: u32,
) -> Result<usize, ResolutionCoreOperatorErrorV3> {
    let mut found: Option<usize> = None;
    let mut start = 0_usize;
    while start + 4 <= ledger_bytes.len() {
        let span = ledger_bytes
            .get(start..start + 4)
            .ok_or_else(|| refuse("header span"))?;
        if span.iter().all(|byte| *byte == 0) {
            let mut probe = ledger_bytes.to_vec();
            if let Some(slot) = probe.get_mut(start..start + 4) {
                slot.copy_from_slice(&probe_rate.to_le_bytes());
            }
            let admits = FundingLedgerV2::decode(&probe).is_ok_and(|ledger| {
                ledger.funded_rent_rate() == probe_rate
                    && ledger.manifest_content_id() == manifest_id
                    && ledger.authenticate(manifest_id, manifest).is_ok()
            });
            if admits {
                if found.is_some() {
                    return Err(refuse("more than one header span carries the funded rate"));
                }
                found = Some(start);
            }
        }
        start += 1;
    }
    found.ok_or_else(|| refuse("no header span carries the funded rate"))
}

/// Price one on-chain funding ledger, recovering its rate if it records none.
///
/// A ledger whose header already carries a rate is returned unchanged and
/// `recovered` is false -- the record is the authority and this module never
/// second-guesses it. A ledger written before the field existed is recovered
/// from its own bytes, cross-checked against `siblings` (other accounts of the
/// SAME founding, at any width), and returned with the rate spliced in.
pub fn ledger_with_funded_rent_rate_v2(
    ledger_bytes: &[u8],
    ledger_lamports: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    siblings: &[FundedRentReadingV2],
) -> Result<RecoveredFundingLedgerV2, ResolutionCoreOperatorErrorV3> {
    match FundingLedgerV2::decode(ledger_bytes) {
        Ok(ledger) => {
            return Ok(RecoveredFundingLedgerV2 {
                funded_rent_rate: ledger.funded_rent_rate(),
                bytes: ledger_bytes.to_vec(),
                recovered: false,
            });
        }
        Err(CapabilityError::FundedRentRateMissing) => {}
        Err(error) => return Err(refuse_funding(&format!("ledger decode: {error:?}"))),
    }
    // A probe rate no cluster charges, so a span that decodes to some OTHER
    // number cannot be mistaken for the one being located.
    const PROBE_RATE: u32 = 0x5350_4E31;
    let offset = locate_funded_rent_span_v2(ledger_bytes, manifest_id, manifest, PROBE_RATE)?;
    let mut probe = ledger_bytes.to_vec();
    probe
        .get_mut(offset..offset + 4)
        .ok_or_else(|| refuse("located span"))?
        .copy_from_slice(&PROBE_RATE.to_le_bytes());
    // Nothing the rate prices is read here: the principal the rows still owe is
    // a sum over the slots, and the probe only makes the header decodable.
    let remaining_native_principal = FundingLedgerV2::decode(&probe)
        .map_err(|_| refuse_funding("probe decode"))?
        .authenticate(manifest_id, manifest)
        .map_err(|_| refuse_funding("probe manifest binding"))?
        .remaining_native_lamports_total()
        .map_err(|_| refuse_funding("remaining native principal did not sum"))?;
    let mut readings = vec![FundedRentReadingV2 {
        account_bytes: ledger_bytes.len(),
        account_lamports: ledger_lamports,
        remaining_native_principal,
    }];
    readings.extend_from_slice(siblings);
    let funded_rent_rate = recover_funded_rent_rate_v2(&readings)?;
    let mut bytes = ledger_bytes.to_vec();
    bytes
        .get_mut(offset..offset + 4)
        .ok_or_else(|| refuse("located span"))?
        .copy_from_slice(&funded_rent_rate.to_le_bytes());
    // The conjunct that refused this ledger is the one that must now pass, and
    // it must pass EXACTLY -- recovery that admits a donation is not recovery.
    FundingLedgerV2::decode(&bytes)
        .map_err(|_| refuse_funding("recovered ledger decode"))?
        .authenticate(manifest_id, manifest)
        .map_err(|_| refuse_funding("recovered ledger manifest binding"))?
        .validate_recorded_native_custody(ledger_lamports, ledger_bytes.len(), false)
        .map_err(|error| refuse(&format!("recovered custody arithmetic: {error:?}")))?;
    eprintln!(
        "funded-rent recovery: ledger of {} bytes holding {ledger_lamports} lamports \
         over {remaining_native_principal} native principal was funded at \
         {funded_rent_rate} lamports per byte ({} siblings agreed)",
        ledger_bytes.len(),
        siblings.len(),
    );
    Ok(RecoveredFundingLedgerV2 {
        funded_rent_rate,
        bytes,
        recovered: true,
    })
}
