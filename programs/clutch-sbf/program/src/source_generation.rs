//! The generation-agnostic seam between a sealed source page and resolution.
//!
//! Two source generations now reach a market's resolution plane. They are
//! deliberately **not** interchangeable — different account tags, different
//! commitment domains, different feed-identity domains, different admission
//! evidence — and this module is what lets one resolution join read either
//! without erasing that.
//!
//! ## What it is safe to share, and why
//!
//! [`crate::source_archive_v2`] fixed the v2 page's geometry to the V1 page's,
//! byte for byte: the same 512-byte header, the same thirty-two 64-byte record
//! slots, the same field offsets. That is a *record decoder* fact, and it is the
//! whole reason this seam is a thin enum rather than a second fold.
//!
//! ## What must never be shared, and how that is kept
//!
//! Neither arm of [`VerifiedSealedArchive`] can be constructed from bytes: each
//! comes from its own generation's verifier, and each verifier pins its own
//! account tag (`0x71`/`0x74`), its own page-commitment domain, and its own
//! source-spec type. So:
//!
//! * a V1 page presented against a v2 spec never produces
//!   [`VerifiedSealedArchive::V2`] — [`crate::source_archive_v2`]'s verifier
//!   refuses the tag before anything else is read; and
//! * a v2 page presented against a V1 spec never produces
//!   [`VerifiedSealedArchive::V1`] for the mirror reason.
//!
//! The *choice* of generation is not a caller input either. It is read from the
//! authenticated SourceSpec account's exact length, exactly as
//! [`crate::instructions::source_ingest::require_registered_source_for_market`]
//! already reads it at the collateral boundary, and the spec's own address is
//! derived from the market's frozen Terms rather than accepted.
//!
//! ## The one rule that is genuinely per-generation
//!
//! Window maturity. The two generations prove it with different evidence and
//! [`SealedArchiveBindingV1::window_has_matured`] is where that difference is
//! written down rather than averaged away; see its own documentation.

use clutch_accumulator::WindowDomain;
use clutch_solana_layout::Hash32;

use crate::source_archive::{
    ArchivedObservationV1, SealedArchiveReceiptV1, SourceArchiveError, VerifiedSealedArchiveViewV1,
    SOURCE_SPEC_ACCOUNT_V1_BYTES,
};
use crate::source_archive_v2::{
    ArchiveV2Error, SealedArchiveReceiptV2, VerifiedSealedArchiveViewV2,
    SOURCE_SPEC_ACCOUNT_V2_BYTES,
};

/// Which source generation produced a page or a spec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceGeneration {
    /// The V1 provider-neutral plane: an immutable price data account.
    V1,
    /// The v2 pull profile: an ephemeral, receiver-posted price update.
    V2,
}

/// The provenance both generations state in the same words.
///
/// Every field here is a *recomputed* fact from a verified page, never a
/// caller assertion. [`Self::generation`] is carried rather than dropped
/// because one rule below still depends on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedArchiveBindingV1 {
    /// Which generation sealed this page.
    pub generation: SourceGeneration,
    /// Canonical archive account address.
    pub archive_key: [u8; 32],
    /// Feed identity the page binds — itself domain-separated per generation.
    pub feed: Hash32,
    /// Canonical window identity.
    pub window: Hash32,
    /// Recomputed page commitment.
    pub page_commitment: Hash32,
    /// Repair generation of the window.
    pub repair_generation: u64,
    /// First bucket the page covers.
    pub start_bucket: u64,
    /// One past the last bucket the page covers.
    pub end_bucket_exclusive: u64,
    /// Feed cursor the seal fixed.
    pub sealed_feed_cursor: u64,
    /// Canonical PDA bump stored in the page.
    pub stored_bump: u8,
}

impl SealedArchiveBindingV1 {
    /// Project one verified V1 receipt.
    pub const fn from_v1(receipt: SealedArchiveReceiptV1) -> Self {
        Self {
            generation: SourceGeneration::V1,
            archive_key: receipt.archive_key(),
            feed: receipt.feed(),
            window: receipt.window(),
            page_commitment: receipt.page_commitment(),
            repair_generation: receipt.repair_generation(),
            start_bucket: receipt.start_bucket(),
            end_bucket_exclusive: receipt.end_bucket_exclusive(),
            sealed_feed_cursor: receipt.sealed_feed_cursor(),
            stored_bump: receipt.stored_bump(),
        }
    }

    /// Project one verified v2 receipt.
    pub const fn from_v2(receipt: SealedArchiveReceiptV2) -> Self {
        Self {
            generation: SourceGeneration::V2,
            archive_key: receipt.archive_key(),
            feed: receipt.feed(),
            window: receipt.window(),
            page_commitment: receipt.page_commitment(),
            repair_generation: receipt.repair_generation(),
            start_bucket: receipt.start_bucket(),
            end_bucket_exclusive: receipt.end_bucket_exclusive(),
            sealed_feed_cursor: receipt.sealed_feed_cursor(),
            stored_bump: receipt.stored_bump(),
        }
    }

    /// Whether this page's own admission evidence proves its whole window has
    /// closed.
    ///
    /// This is the one place the generations genuinely differ, so it is stated
    /// rather than smoothed over:
    ///
    /// * **V1** proves maturity with a *record*. `seal_archive_authenticated`
    ///   demands a reading that selects bucket `end` — one past the window —
    ///   and advances the sealed cursor to the maturity bucket. So the test is
    ///   the recorded cursor against `maturity_bucket_exclusive`.
    /// * **v2** proves maturity with the *boundary rule*. `CROSSING_V1` admits
    ///   the record for bucket `b` only once canonical Clock has passed
    ///   `(b + 1) * bucket_seconds + boundary_grace_seconds`, and
    ///   `seal_archive_v2` refuses a page short of its whole window. A complete
    ///   sealed v2 page therefore carries, in the admission of its *last*
    ///   record, the proof that the closing boundary of the window is behind
    ///   the chain — with no bucket-`end` reading to record, and so with a
    ///   sealed cursor of `end` rather than `end + 1`.
    ///
    /// Reading v2's cursor under V1's rule would refuse every honest v2 page;
    /// reading V1's under v2's would accept a V1 page whose maturity witness
    /// was never taken. Neither is a rounding error, which is why the rule
    /// travels with the generation tag instead of with the number.
    pub fn window_has_matured(&self, window: WindowDomain) -> bool {
        match self.generation {
            SourceGeneration::V1 => self.sealed_feed_cursor >= window.maturity_bucket_exclusive(),
            SourceGeneration::V2 => self.sealed_feed_cursor >= window.end_bucket_exclusive(),
        }
    }
}

/// One fully verified sealed source page, of either generation.
///
/// Neither arm has a public constructor from bytes. Both come only from their
/// generation's verifier, which is what keeps the commitment domains disjoint
/// through this seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifiedSealedArchive<'a> {
    /// A verified V1 page.
    V1(VerifiedSealedArchiveViewV1<'a>),
    /// A verified v2 page.
    V2(VerifiedSealedArchiveViewV2<'a>),
}

impl VerifiedSealedArchive<'_> {
    /// Which generation this page belongs to.
    pub const fn generation(self) -> SourceGeneration {
        match self {
            Self::V1(_) => SourceGeneration::V1,
            Self::V2(_) => SourceGeneration::V2,
        }
    }

    /// The generation-agnostic provenance of the verified page.
    pub fn binding(self) -> SealedArchiveBindingV1 {
        match self {
            Self::V1(view) => SealedArchiveBindingV1::from_v1(view.receipt()),
            Self::V2(view) => SealedArchiveBindingV1::from_v2(view.receipt()),
        }
    }

    /// Read one bounded observation from the immutable verified page.
    ///
    /// The index stays hostile in both arms; each generation checks it against
    /// its own committed record count. The returned shape is V1's because the
    /// v2 record decodes to the same six fields at the same offsets — that
    /// identity is pinned by `the_v2_page_geometry_is_the_v1_page_geometry`,
    /// not assumed here.
    pub fn archived_observation(
        self,
        index: usize,
    ) -> Result<ArchivedObservationV1, ArchiveJoinError> {
        match self {
            Self::V1(view) => view
                .archived_observation(index)
                .map_err(ArchiveJoinError::V1),
            Self::V2(view) => {
                let record = view.archived_record(index).map_err(ArchiveJoinError::V2)?;
                Ok(ArchivedObservationV1 {
                    bucket: record.bucket,
                    low: record.low,
                    high: record.high,
                    source_sequence: record.sequence,
                    publish_slot: record.publish_slot,
                    publish_time: record.publish_time,
                })
            }
        }
    }
}

/// A refusal from either generation's page reader.
///
/// The two vocabularies are carried rather than collapsed: a fold that refuses
/// should be able to say which generation's decoder refused, even where the
/// instruction layer projects both onto one numeric code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveJoinError {
    /// The V1 page reader refused.
    V1(SourceArchiveError),
    /// The v2 page reader refused.
    V2(ArchiveV2Error),
    /// The presented SourceSpec account is neither generation's exact length.
    ///
    /// The generation is read from the account, not chosen by a caller, so a
    /// length that names no generation is a refusal rather than a default.
    UnknownGeneration,
    /// The spec named a release this ELF does not carry.
    ReleaseUnavailable,
    /// The verified spec does not bind the feed the market's Terms name.
    SpecBindingMismatch,
}

/// One runtime account presented to the generation-agnostic verifier.
///
/// Deliberately raw parts rather than either generation's view type: this
/// module sits above both codecs and must not privilege one of them at its own
/// boundary. Each arm below rewraps into its own generation's view, which is
/// what re-runs that generation's metadata checks rather than trusting these
/// fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountBytesV1<'a> {
    /// Address the runtime presented.
    pub key: [u8; 32],
    /// Owning program the runtime presented.
    pub owner: [u8; 32],
    /// Executable flag the runtime presented.
    pub executable: bool,
    /// Account body.
    pub data: &'a [u8],
}

/// What one authenticated SourceSpec account fixes, generation aside.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSourceSpecBindingV1 {
    /// Which generation the account belongs to.
    pub generation: SourceGeneration,
    /// Canonical feed identity — itself domain-separated per generation.
    pub feed: Hash32,
    /// Canonical PDA bump stored by construction.
    pub stored_bump: u8,
}

/// One presented spec/archive pair, with the addresses they must occupy.
///
/// A struct rather than eight positional arguments, because two of those
/// arguments are *expected* addresses and two are *presented* accounts, and a
/// call site that transposed them would still typecheck.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentedSourcePlaneV1<'a> {
    /// This program's own id.
    pub clutch_program: [u8; 32],
    /// The SourceSpec address derived from the market's frozen Terms.
    pub expected_spec_key: [u8; 32],
    /// The canonical bump of that derivation.
    pub expected_spec_bump: u8,
    /// The presented SourceSpec account.
    pub spec: SourceAccountBytesV1<'a>,
    /// The archive address derived from the same Terms and window.
    pub expected_archive_key: [u8; 32],
    /// The presented archive page.
    pub archive: SourceAccountBytesV1<'a>,
    /// The feed identity the market's Terms froze.
    pub expected_feed: Hash32,
    /// The observation window those Terms fix.
    pub window: WindowDomain,
}

/// Authenticate one SourceSpec account and the sealed page it governs, at
/// whichever generation the spec account actually is.
///
/// The generation comes from the spec account's exact length and from nothing
/// else — not from instruction data, not from a caller-supplied version byte,
/// not from the archive. Then the matching generation's verifier runs, and it
/// is that verifier which pins the archive's account tag and page-commitment
/// domain. So a v1 page can never satisfy a v2 spec and a v2 page can never
/// satisfy a v1 spec: the cross pairings are refused by the page verifier
/// before any binding is compared, not by a comparison that could be relaxed.
///
/// `expected_feed` is the market's own frozen Terms feed. It is checked here so
/// that a spec of the *right* generation but the wrong feed is refused in the
/// same breath, rather than surviving to a later window comparison.
pub fn verify_recorded_sealed_source<'a>(
    presented: PresentedSourcePlaneV1<'a>,
) -> Result<(VerifiedSourceSpecBindingV1, VerifiedSealedArchive<'a>), ArchiveJoinError> {
    let PresentedSourcePlaneV1 {
        clutch_program,
        expected_spec_key,
        expected_spec_bump,
        spec,
        expected_archive_key,
        archive,
        expected_feed,
        window,
    } = presented;
    match spec.data.len() {
        SOURCE_SPEC_ACCOUNT_V1_BYTES => {
            let verified = crate::source_archive::verify_source_spec_account(
                clutch_program,
                expected_spec_key,
                crate::source_archive::SourceSpecAccountViewV1::new(
                    spec.key,
                    spec.owner,
                    spec.executable,
                    spec.data,
                ),
            )
            .map_err(ArchiveJoinError::V1)?;
            if verified.feed() != expected_feed || verified.stored_bump() != expected_spec_bump {
                return Err(ArchiveJoinError::SpecBindingMismatch);
            }
            let view = crate::source_archive::verify_recorded_sealed_archive_view(
                clutch_program,
                expected_archive_key,
                crate::source_archive::ArchiveAccountViewV1::new(
                    archive.key,
                    archive.owner,
                    archive.executable,
                    archive.data,
                ),
                verified,
                window,
            )
            .map_err(ArchiveJoinError::V1)?;
            Ok((
                VerifiedSourceSpecBindingV1 {
                    generation: SourceGeneration::V1,
                    feed: verified.feed(),
                    stored_bump: verified.stored_bump(),
                },
                VerifiedSealedArchive::V1(view),
            ))
        }
        SOURCE_SPEC_ACCOUNT_V2_BYTES => {
            let verified = crate::source_archive_v2::verify_source_spec_v2_account(
                clutch_program,
                expected_spec_key,
                crate::source_archive_v2::AccountViewV2::new(
                    spec.key,
                    spec.owner,
                    spec.executable,
                    spec.data,
                ),
            )
            .map_err(ArchiveJoinError::V2)?;
            if verified.feed() != expected_feed || verified.stored_bump() != expected_spec_bump {
                return Err(ArchiveJoinError::SpecBindingMismatch);
            }
            /* The same closed registry the collateral boundary and the ingest
             * routes ask.  A market whose release this ELF has retired cannot
             * resolve from its pages either, which is the property that makes
             * a registry flip reversible. */
            let release = crate::source_identity::select_release(verified.spec())
                .ok_or(ArchiveJoinError::ReleaseUnavailable)?;
            let view = crate::source_archive_v2::verify_recorded_sealed_archive_v2_view(
                clutch_program,
                expected_archive_key,
                crate::source_archive_v2::AccountViewV2::new(
                    archive.key,
                    archive.owner,
                    archive.executable,
                    archive.data,
                ),
                verified,
                release,
                window,
            )
            .map_err(ArchiveJoinError::V2)?;
            Ok((
                VerifiedSourceSpecBindingV1 {
                    generation: SourceGeneration::V2,
                    feed: verified.feed(),
                    stored_bump: verified.stored_bump(),
                },
                VerifiedSealedArchive::V2(view),
            ))
        }
        _ => Err(ArchiveJoinError::UnknownGeneration),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Filler wide enough to slice any tested account length out of.
    const SCRATCH: [u8; 2_560] = [0_u8; 2_560];
    use clutch_accumulator::{CoveragePolicy, FeedIdentity, Grid};

    fn window(start: u64, end: u64) -> WindowDomain {
        WindowDomain::new(
            FeedIdentity::new([0x21; 32], [0x22; 32], 3, 1).expect("feed identity"),
            Grid::new(7, 1, 60).expect("grid"),
            start,
            end,
            end + 1,
            0,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .expect("window")
    }

    fn binding(generation: SourceGeneration, sealed_feed_cursor: u64) -> SealedArchiveBindingV1 {
        SealedArchiveBindingV1 {
            generation,
            archive_key: [0x31; 32],
            feed: Hash32::from_bytes([0x32; 32]),
            window: Hash32::from_bytes([0x33; 32]),
            page_commitment: Hash32::from_bytes([0x34; 32]),
            repair_generation: 0,
            start_bucket: 100,
            end_bucket_exclusive: 104,
            sealed_feed_cursor,
            stored_bump: 254,
        }
    }

    #[test]
    fn each_generation_answers_maturity_with_its_own_evidence() {
        let domain = window(100, 104);
        assert_eq!(domain.maturity_bucket_exclusive(), 105);

        /* V1 seals to the maturity bucket because a bucket-104 reading was
         * taken; a V1 page that stopped at 104 has no maturity witness. */
        assert!(binding(SourceGeneration::V1, 105).window_has_matured(domain));
        assert!(!binding(SourceGeneration::V1, 104).window_has_matured(domain));

        /* v2 seals to the window end because the last record's own admission
         * already required the closing boundary plus grace to have passed. */
        assert!(binding(SourceGeneration::V2, 104).window_has_matured(domain));
        assert!(!binding(SourceGeneration::V2, 103).window_has_matured(domain));
    }

    #[test]
    fn a_spec_account_of_neither_length_names_no_generation() {
        /* The generation is read from the account, so a length that is neither
         * generation's whole account is a refusal rather than a default.  The
         * length is consulted before any other byte, which is why dummy
         * contents are enough to reach it. */
        let bytes = |len: usize| SourceAccountBytesV1 {
            key: [0x41; 32],
            owner: [0x42; 32],
            executable: false,
            data: &SCRATCH[..len],
        };
        for len in [0_usize, 1, 291, 293, 403, 405, 2_560] {
            let presented = PresentedSourcePlaneV1 {
                clutch_program: [0x43; 32],
                expected_spec_key: [0x41; 32],
                expected_spec_bump: 254,
                spec: bytes(len),
                expected_archive_key: [0x44; 32],
                archive: bytes(SCRATCH.len()),
                expected_feed: Hash32::from_bytes([0x45; 32]),
                window: window(100, 104),
            };
            assert_eq!(
                verify_recorded_sealed_source(presented),
                Err(ArchiveJoinError::UnknownGeneration),
                "{len} bytes is neither generation"
            );
        }
        /* And the two admitted lengths are exactly the two account widths, so
         * the arms above are reachable rather than dead. */
        assert_eq!(SOURCE_SPEC_ACCOUNT_V1_BYTES, 292);
        assert_eq!(SOURCE_SPEC_ACCOUNT_V2_BYTES, 404);
    }

    #[test]
    fn the_generation_tag_is_not_cosmetic() {
        let domain = window(100, 104);
        /* The exact cursor an honest v2 seal writes is refused under V1's
         * rule, and the exact cursor a short V1 seal would leave is accepted
         * under v2's.  So the tag decides the answer, and mislabelling a page
         * is not a harmless relabelling. */
        let honest_v2_cursor = 104;
        assert!(binding(SourceGeneration::V2, honest_v2_cursor).window_has_matured(domain));
        assert!(!binding(SourceGeneration::V1, honest_v2_cursor).window_has_matured(domain));
    }
}
