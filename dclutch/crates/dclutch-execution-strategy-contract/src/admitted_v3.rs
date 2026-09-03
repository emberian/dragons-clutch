//! Canonical admitted-AOT invocation transcript and read-only CPI frame.
//!
//! This contract grants no accelerator write or CPI authority. Registry
//! admission, certificate, artifact, and deployment authentication remain in
//! the SVM adapter. The digest merely gives that authenticated adapter and one
//! stateless accelerator an exact common transcript to recompute.

use dclutch_capability_program_contract::hot_v3::{
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
    HOT_DESCRIPTOR_STAGING_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3, HOT_STRATEGY_STAGING_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
};
use dclutch_core_contract::ContentId;

use crate::v2::AcceleratorTransportProfileV2;
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use dclutch_sha256_adapter::digestv;

/// Domain for one authoritative admitted-AOT invocation context.
pub const ADMITTED_INVOCATION_CONTEXT_DOMAIN_V3: &[u8] = b"dclutch:admitted-invocation-context:v3";

// THE ADMITTED CPI FRAME IS NOT THIS FILE'S TO INVENT.
//
// Every coordinate below used to be a literal, and the literals described an
// eighteen-account frame that nothing has ever produced. There is exactly one
// admitted-accelerator CPI site in the tree -- Trading's `invoke_admitted_chunk`
// -- and it emits the caller authority, then the WHOLE common Hot fixed frame,
// then eight strategy-owned evidence accounts, then the runtime slice. So the
// real instructions sysvar sits at 30 and this file said 4, the real runtime
// accounts start at 48 and this file said 18, and the General accelerator --
// the only reader these constants have ever had -- refused every OpenBatch
// through real Trading ELFs with `0xC00A InstructionsSysvarAccount`, correctly,
// because at index 4 the real frame carries a vacant CapabilityManifest staging
// cursor.
//
// Every General harness built the frame from this table too, so the harnesses
// and the accelerator agreed with each other and neither agreed with the
// producer. The coordinates are DERIVED now. `HOT_*_ACCOUNT_V3` is the
// producer's own authority for what sits where inside the fixed frame, and the
// two offsets this file still owns -- the one-account authority prefix and the
// eight-account evidence suffix -- are named once each below.
//
// What this file is NOT: a second authentication path. The Dealer accelerator
// authenticates through `authenticate_accelerator_invocation_v4`, which
// re-derives certificates, admissions and deployments; the General accelerator
// deliberately does not, because that authentication stays in the SVM adapter.
// It needs coordinates, not a second verifier, and coordinates are what this is.

/// Caller-authority account in every admitted accelerator CPI.
pub const ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3: usize = 0;
/// First common Hot fixed account, immediately after the caller authority.
pub const ADMITTED_HOT_FIXED_START_V3: usize = ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3 + 1;
/// Current release-set activation cache.
pub const ADMITTED_ACTIVATION_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_ACTIVATION_CACHE_ACCOUNT_V3;
/// Immutable Registry program.
pub const ADMITTED_REGISTRY_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_REGISTRY_PROGRAM_ACCOUNT_V3;
/// Rent sysvar used to reauthenticate finalized records.
pub const ADMITTED_RENT_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_RENT_SYSVAR_ACCOUNT_V3;
/// Instructions sysvar exposing the exact top-level Trading request.
pub const ADMITTED_INSTRUCTIONS_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3;
/// Current Trading program.
pub const ADMITTED_TRADING_PROGRAM_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_TRADING_PROGRAM_ACCOUNT_V3;
/// Current Trading ProgramData.
pub const ADMITTED_TRADING_PROGRAMDATA_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_TRADING_PROGRAMDATA_ACCOUNT_V3;
/// Action-selected CapabilityProgramV3 raw record.
pub const ADMITTED_CAPABILITY_RAW_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_DESCRIPTOR_RAW_ACCOUNT_V3;
/// Vacant CapabilityProgramV3 staging cursor.
pub const ADMITTED_CAPABILITY_STAGING_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_DESCRIPTOR_STAGING_ACCOUNT_V3;
/// Descriptor-selected ExecutionStrategy raw record.
pub const ADMITTED_STRATEGY_RAW_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_STRATEGY_RAW_ACCOUNT_V3;
/// Vacant ExecutionStrategy staging cursor.
pub const ADMITTED_STRATEGY_STAGING_ACCOUNT_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_STRATEGY_STAGING_ACCOUNT_V3;

/// First strategy-owned evidence account, immediately after the Hot fixed frame.
pub const ADMITTED_STRATEGY_EVIDENCE_START_V3: usize =
    ADMITTED_HOT_FIXED_START_V3 + HOT_FIXED_ACCOUNT_COUNT_V3;
/// Exact strategy-owned evidence suffix, including accelerator Program/ProgramData.
pub const ADMITTED_STRATEGY_EVIDENCE_COUNT_V3: usize = 8;

/// Strategy-selected Certificate raw record.
pub const ADMITTED_CERTIFICATE_RAW_ACCOUNT_V3: usize = ADMITTED_STRATEGY_EVIDENCE_START_V3;
/// Vacant Certificate staging cursor.
pub const ADMITTED_CERTIFICATE_STAGING_ACCOUNT_V3: usize = ADMITTED_STRATEGY_EVIDENCE_START_V3 + 1;
/// Strategy-selected Registry Admission raw record.
pub const ADMITTED_ADMISSION_RAW_ACCOUNT_V3: usize = ADMITTED_STRATEGY_EVIDENCE_START_V3 + 2;
/// Vacant Admission staging cursor.
pub const ADMITTED_ADMISSION_STAGING_ACCOUNT_V3: usize = ADMITTED_STRATEGY_EVIDENCE_START_V3 + 3;
/// Certificate-selected ArtifactRelease raw record.
pub const ADMITTED_ARTIFACT_RAW_ACCOUNT_V3: usize = ADMITTED_STRATEGY_EVIDENCE_START_V3 + 4;
/// Vacant ArtifactRelease staging cursor.
pub const ADMITTED_ARTIFACT_STAGING_ACCOUNT_V3: usize = ADMITTED_STRATEGY_EVIDENCE_START_V3 + 5;
/// Immutable accelerator program.
///
/// The real frame has always carried this and the literal table never named it,
/// which is the tell that the table was a design rather than a description.
pub const ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3: usize = ADMITTED_STRATEGY_EVIDENCE_START_V3 + 6;
/// Immutable accelerator ProgramData.
pub const ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3: usize =
    ADMITTED_STRATEGY_EVIDENCE_START_V3 + 7;
/// First AccountProfile-ordered read-only runtime account.
pub const ADMITTED_RUNTIME_ACCOUNTS_START_V3: usize =
    ADMITTED_STRATEGY_EVIDENCE_START_V3 + ADMITTED_STRATEGY_EVIDENCE_COUNT_V3;

/// Accelerator-owned candidate output page, under `OutputPageV3` only.
///
/// APPENDED, so nothing moves. Every coordinate above -- the caller authority,
/// the whole common Hot fixed frame, the eight evidence accounts -- reads the
/// same number it read before this existed, and
/// [`ADMITTED_RUNTIME_ACCOUNTS_START_V3`] still names where the CHUNKED
/// transport's runtime slice begins. The page takes that slot and the
/// output-page transport's runtime slice begins one later, which is what
/// [`ADMITTED_OUTPUT_PAGE_RUNTIME_ACCOUNTS_START_V3`] is for.
///
/// Two constants rather than one profile-switched constant, because the switch
/// belongs to the party that already knows the transport. A single constant
/// that changed value would have moved the chunked frame for everyone, and
/// "inert until a Strategy record names the profile" would have been false.
pub const ADMITTED_OUTPUT_PAGE_ACCOUNT_V3: usize = ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3 + 1;
/// First AccountProfile-ordered runtime account under the output-page transport.
pub const ADMITTED_OUTPUT_PAGE_RUNTIME_ACCOUNTS_START_V3: usize =
    ADMITTED_OUTPUT_PAGE_ACCOUNT_V3 + 1;

/// Where an admitted accelerator CPI frame's runtime slice begins, by transport.
///
/// ONE READER FOR THE DISPLACEMENT. Trading's producer, both accelerators, the
/// operator and the host bundle builder all need this number, and the last time
/// an admitted frame coordinate had one copy per party the copies agreed with
/// each other and none of them agreed with the producer -- `0xC00A`, and the
/// note at the top of this file.
///
/// `None` for Shadow AOT rather than the chunked answer: Shadow has its own
/// six-account prefix in [`crate::shadow_v3`] and does not use this frame at
/// all, so there is no coordinate to give it.
pub const fn admitted_runtime_accounts_start_v3(
    profile: AcceleratorTransportProfileV2,
) -> Option<usize> {
    match profile {
        AcceleratorTransportProfileV2::ChunkedBankV2 => Some(ADMITTED_RUNTIME_ACCOUNTS_START_V3),
        AcceleratorTransportProfileV2::OutputPageV3 => {
            Some(ADMITTED_OUTPUT_PAGE_RUNTIME_ACCOUNTS_START_V3)
        }
        AcceleratorTransportProfileV2::ShadowTranscriptV3 => None,
    }
}

// The evidence suffix is the one span this file states rather than derives, so
// it is pinned to the accounts that occupy it: the last named coordinate must be
// the last slot before the runtime slice. A ninth evidence account added to the
// producer without a name here stops compiling instead of silently shifting
// every runtime coordinate by one.
const _: () = {
    assert!(
        ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3 + 1 == ADMITTED_RUNTIME_ACCOUNTS_START_V3,
        "the admitted evidence suffix count and its named coordinates disagree"
    );
    // The page is APPENDED: it occupies the slot the chunked runtime slice
    // starts at, and displaces only the output-page transport's own slice. A
    // ninth evidence account would break this at the same time it breaks the
    // assertion above, rather than silently pushing the page into the runtime.
    assert!(
        ADMITTED_OUTPUT_PAGE_ACCOUNT_V3 == ADMITTED_RUNTIME_ACCOUNTS_START_V3,
        "the appended output page does not sit immediately after the evidence suffix"
    );
    assert!(
        ADMITTED_OUTPUT_PAGE_RUNTIME_ACCOUNTS_START_V3 == ADMITTED_RUNTIME_ACCOUNTS_START_V3 + 1,
        "the output-page runtime slice is not displaced by exactly the page"
    );
};

/// Stable refusal from admitted transcript construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmittedTranscriptErrorV3 {
    /// SHA-256 produced the reserved zero content identity.
    ZeroDigest,
    /// A witness body is not the exact width its own header declares.
    InvalidLength,
    /// The leading eight bytes are not a prelude witness at all.
    InvalidMagic,
    /// The persisted schema version is not one this contract reads.
    UnsupportedSchema,
    /// A reserved field carried something other than zero.
    NonCanonicalReserved,
}

/// Exact authenticated facts committed by one admitted invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedInvocationContextV3 {
    /// Current immutable release set.
    pub release_set: ContentId,
    /// Logical Core Market.
    pub market: ContentId,
    /// Mutable Trading root key.
    pub root: ContentId,
    /// Immutable Registry program.
    pub registry_program: ContentId,
    /// Current Trading program.
    pub trading_program: ContentId,
    /// Immutable admitted accelerator program.
    pub accelerator_program: ContentId,
    /// Action-selected CapabilityProgramV3.
    pub capability_program: ContentId,
    /// Descriptor-selected AccountProfile.
    pub account_profile: ContentId,
    /// Descriptor-selected RequestProfile.
    pub request_profile: ContentId,
    /// Strategy-selected Transition program.
    pub transition: ContentId,
    /// Descriptor-selected EffectProgram.
    pub effect: ContentId,
    /// Descriptor-selected lifecycle policy.
    pub lifecycle: ContentId,
    /// Descriptor-selected ExecutionStrategy.
    pub strategy: ContentId,
    /// Strategy-selected translation certificate.
    pub certificate: ContentId,
    /// Registry Admission authorizing the exact Certificate.
    pub admission: ContentId,
    /// Immutable ArtifactRelease selected by Certificate.
    pub artifact_release: ArtifactReleaseIdV1,
    /// Manifest-selected immutable config content identity.
    pub config: ContentId,
    /// Core-selected Product graph-root content identity.
    pub product: ContentId,
    /// Product-selected portfolio content identity.
    pub portfolio: ContentId,
    /// Exact authenticated Product-linked basis raw identity.
    pub linked_basis: ContentId,
    /// `family_request_digest_v3` of the exact complete family request.
    ///
    /// Not a bare SHA-256 of the request bytes: it is the domain-separated,
    /// length-prefixed form defined in [`crate::shadow_digest_v3`]. This
    /// distinction is load-bearing, and a doc comment that described the bare
    /// form once made a bare recomputation look correct.
    pub family_request_digest: ContentId,
    /// Digest of the exact AccountProfile-ordered read-only observations.
    pub runtime_observations_digest: ContentId,
    /// SHA-256 of the exact root prestate.
    pub root_prestate_digest: ContentId,
    /// Action selector selected by CapabilityProgramSetV1.
    pub selected_action: u32,
    /// Product-authoritative semantic width.
    pub tail_count: u32,
    /// Exact logical runtime-account count.
    pub account_count: u32,
    /// Exact scalar-bank count.
    pub scalar_count: u32,
    /// Exact identity-bank count.
    pub identity_count: u32,
}

/// Commit one complete authenticated admitted invocation context.
pub fn admitted_invocation_context_digest_v3(
    context: AdmittedInvocationContextV3,
) -> Result<ContentId, AdmittedTranscriptErrorV3> {
    // Every field is fixed-width, so the preimage is a fixed slice list. The
    // identities are borrowed where they sit; only the release id and the five
    // counts are materialized, and they are materialized contiguously so they
    // cost one slice rather than six.
    let artifact_release = context.artifact_release.to_bytes();
    let mut counts = [0_u8; 5 * 4];
    for (index, value) in [
        context.selected_action,
        context.tail_count,
        context.account_count,
        context.scalar_count,
        context.identity_count,
    ]
    .into_iter()
    .enumerate()
    {
        counts
            .get_mut(index * 4..index * 4 + 4)
            .ok_or(AdmittedTranscriptErrorV3::ZeroDigest)?
            .copy_from_slice(&value.to_le_bytes());
    }
    let preimage: [&[u8]; 25] = [
        ADMITTED_INVOCATION_CONTEXT_DOMAIN_V3,
        context.release_set.as_bytes(),
        context.market.as_bytes(),
        context.root.as_bytes(),
        context.registry_program.as_bytes(),
        context.trading_program.as_bytes(),
        context.accelerator_program.as_bytes(),
        context.capability_program.as_bytes(),
        context.account_profile.as_bytes(),
        context.request_profile.as_bytes(),
        context.transition.as_bytes(),
        context.effect.as_bytes(),
        context.lifecycle.as_bytes(),
        context.strategy.as_bytes(),
        context.certificate.as_bytes(),
        context.admission.as_bytes(),
        &artifact_release,
        context.config.as_bytes(),
        context.product.as_bytes(),
        context.portfolio.as_bytes(),
        context.linked_basis.as_bytes(),
        context.family_request_digest.as_bytes(),
        context.runtime_observations_digest.as_bytes(),
        context.root_prestate_digest.as_bytes(),
        &counts,
    ];
    ContentId::new(digestv(&preimage)).map_err(|_| AdmittedTranscriptErrorV3::ZeroDigest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero fixture identity")
    }

    fn context() -> AdmittedInvocationContextV3 {
        AdmittedInvocationContextV3 {
            release_set: id(1),
            market: id(2),
            root: id(3),
            registry_program: id(4),
            trading_program: id(5),
            accelerator_program: id(6),
            capability_program: id(7),
            account_profile: id(8),
            request_profile: id(9),
            transition: id(10),
            effect: id(11),
            lifecycle: id(12),
            strategy: id(13),
            certificate: id(14),
            admission: id(15),
            artifact_release: ArtifactReleaseIdV1::new([16; 32]).expect("artifact"),
            config: id(17),
            product: id(18),
            portfolio: id(19),
            linked_basis: id(20),
            family_request_digest: id(21),
            runtime_observations_digest: id(22),
            root_prestate_digest: id(23),
            selected_action: 24,
            tail_count: 258,
            account_count: 30,
            scalar_count: 64,
            identity_count: 42,
        }
    }

    #[test]
    fn invocation_digest_binds_admission_runtime_and_linked_basis() {
        let canonical = context();
        let digest = admitted_invocation_context_digest_v3(canonical).expect("digest");
        for hostile in [
            AdmittedInvocationContextV3 {
                admission: id(31),
                ..canonical
            },
            AdmittedInvocationContextV3 {
                runtime_observations_digest: id(32),
                ..canonical
            },
            AdmittedInvocationContextV3 {
                linked_basis: id(33),
                ..canonical
            },
            AdmittedInvocationContextV3 {
                tail_count: 257,
                ..canonical
            },
        ] {
            assert_ne!(
                admitted_invocation_context_digest_v3(hostile).expect("hostile digest"),
                digest
            );
        }
    }

    /// One witness body, encoded and read back, with its every hostile named.
    ///
    /// THE CONTEXT PREIMAGE IS THE POINT OF THE ROUND TRIP. The witness's whole
    /// authority is that `admitted_invocation_context_digest_v3` of what comes
    /// back out equals the `invocation_context` field of a request the caller
    /// signed, so a decoder that reordered one identity or read one count at
    /// the wrong offset would produce a body that could never be admitted --
    /// and would look like a caller error rather than a codec defect. This
    /// asserts the digest, not merely the fields.
    #[test]
    fn the_prelude_witness_round_trips_and_names_every_hostile() {
        const WIDTH: usize = admitted_prelude_witness_bytes_v1(3, 1);
        let canonical = AdmittedInvocationContextV3 {
            account_count: 3,
            ..context()
        };
        let representatives = [0_usize, 0, 2];
        let mut bytes = [0_u8; WIDTH];
        AdmittedPreludeWitnessV1::encode_into(canonical, &[7], &representatives, &mut bytes)
            .expect("encode witness");

        let witness = AdmittedPreludeWitnessV1::decode(&bytes).expect("decode witness");
        assert_eq!(witness.context(), canonical);
        assert_eq!(
            admitted_invocation_context_digest_v3(witness.context()).expect("witness digest"),
            admitted_invocation_context_digest_v3(canonical).expect("canonical digest"),
        );
        assert_eq!(witness.span_count(), 1);
        assert_eq!(witness.span_width(0).expect("span width"), 7);
        for (index, expected) in representatives.iter().enumerate() {
            assert_eq!(
                witness.representative(index).expect("representative"),
                *expected
            );
        }

        // Each hostile names its own accusation, and a body that is merely
        // shorter or longer than its header declares is `InvalidLength` rather
        // than a successful decode of a prefix.
        let mut wrong_magic = bytes;
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            AdmittedPreludeWitnessV1::decode(&wrong_magic),
            Err(AdmittedTranscriptErrorV3::InvalidMagic)
        );
        let mut wrong_schema = bytes;
        wrong_schema[8] = 9;
        assert_eq!(
            AdmittedPreludeWitnessV1::decode(&wrong_schema),
            Err(AdmittedTranscriptErrorV3::UnsupportedSchema)
        );
        let mut dirty_reserved = bytes;
        dirty_reserved[10] = 1;
        assert_eq!(
            AdmittedPreludeWitnessV1::decode(&dirty_reserved),
            Err(AdmittedTranscriptErrorV3::NonCanonicalReserved)
        );
        assert_eq!(
            AdmittedPreludeWitnessV1::decode(&bytes[..WIDTH - 1]),
            Err(AdmittedTranscriptErrorV3::InvalidLength)
        );
        let mut long = [0_u8; WIDTH + 1];
        long.get_mut(..WIDTH)
            .expect("wide enough")
            .copy_from_slice(&bytes);
        assert_eq!(
            AdmittedPreludeWitnessV1::decode(&long),
            Err(AdmittedTranscriptErrorV3::InvalidLength)
        );

        // The header's account count and the context's are ONE number, and a
        // body that states them differently is refused before either bank is
        // sliced. Without this a witness could declare a two-coordinate frame
        // in its header and a three-coordinate one in the body its digest
        // covers, and a reader would slice the shorter.
        let mut disagreeing = bytes;
        disagreeing[12] = 2;
        assert_eq!(
            AdmittedPreludeWitnessV1::decode(&disagreeing),
            Err(AdmittedTranscriptErrorV3::InvalidLength)
        );

        // A representative names a coordinate of THIS frame. The encoder
        // refuses one that does not, and a body hand-edited past the encoder is
        // refused on the read rather than handed to a caller that would index
        // with it.
        let mut out_of_range = bytes;
        out_of_range[WIDTH - 2] = 3;
        assert_eq!(
            AdmittedPreludeWitnessV1::decode(&out_of_range)
                .expect("decode")
                .representative(2),
            Err(AdmittedTranscriptErrorV3::InvalidLength)
        );
        let mut narrow = [0_u8; admitted_prelude_witness_bytes_v1(3, 0)];
        assert_eq!(
            AdmittedPreludeWitnessV1::encode_into(canonical, &[], &[0, 0, 3], &mut narrow),
            Err(AdmittedTranscriptErrorV3::InvalidLength)
        );
        // And the encoder refuses a context whose account count is not the
        // width of the representative bank it is being asked to carry.
        let mut mismatched = [0_u8; admitted_prelude_witness_bytes_v1(2, 0)];
        assert_eq!(
            AdmittedPreludeWitnessV1::encode_into(canonical, &[], &[0, 0], &mut mismatched),
            Err(AdmittedTranscriptErrorV3::InvalidLength)
        );
    }

    /// The prefix is one authority, the whole Hot fixed frame, then a
    /// contiguous evidence suffix, then the runtime slice.
    ///
    /// THIS TEST ASSERTED FIVE LITERALS -- `0`, `1`, `17`, `18`, `18` -- and
    /// when `68f7c849` derived these coordinates from the producer's own
    /// `HOT_*` table, four of them became snapshots of a frame nothing emits:
    /// it asserted `1` where the constant now reads 23, and `18` where it reads
    /// 48. **A pin written as the number it is pinning cannot notice the thing
    /// it exists to notice** -- it just becomes the last place the old value
    /// survives, and it went red on the commit that made the constants right.
    ///
    /// Derived from the same constants now, so it asserts the SHAPE rather than
    /// a reading of it, and a future displacement moves the test with the table.
    #[test]
    fn cpi_prefix_is_contiguous_and_runtime_readonly_tail_follows() {
        assert_eq!(ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3, 0);
        assert_eq!(
            ADMITTED_HOT_FIXED_START_V3,
            ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3 + 1
        );

        // Every prefix coordinate is its Hot coordinate, displaced by exactly
        // the caller authority. This is the conjunct the literals could not
        // state, and it is the one the `0xC00A` wall was made of.
        for (admitted, hot) in [
            (
                ADMITTED_ACTIVATION_ACCOUNT_V3,
                HOT_ACTIVATION_CACHE_ACCOUNT_V3,
            ),
            (
                ADMITTED_REGISTRY_ACCOUNT_V3,
                HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
            ),
            (ADMITTED_RENT_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3),
            (
                ADMITTED_INSTRUCTIONS_ACCOUNT_V3,
                HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
            ),
            (
                ADMITTED_TRADING_PROGRAM_ACCOUNT_V3,
                HOT_TRADING_PROGRAM_ACCOUNT_V3,
            ),
            (
                ADMITTED_TRADING_PROGRAMDATA_ACCOUNT_V3,
                HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
            ),
            (
                ADMITTED_CAPABILITY_RAW_ACCOUNT_V3,
                HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
            ),
            (
                ADMITTED_CAPABILITY_STAGING_ACCOUNT_V3,
                HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
            ),
            (
                ADMITTED_STRATEGY_RAW_ACCOUNT_V3,
                HOT_STRATEGY_RAW_ACCOUNT_V3,
            ),
            (
                ADMITTED_STRATEGY_STAGING_ACCOUNT_V3,
                HOT_STRATEGY_STAGING_ACCOUNT_V3,
            ),
        ] {
            assert_eq!(admitted, ADMITTED_HOT_FIXED_START_V3 + hot);
        }

        // The evidence suffix begins where the Hot fixed frame ends, is
        // contiguous and in order, and its last account is immediately before
        // the runtime slice.
        assert_eq!(
            ADMITTED_STRATEGY_EVIDENCE_START_V3,
            ADMITTED_HOT_FIXED_START_V3 + HOT_FIXED_ACCOUNT_COUNT_V3
        );
        let evidence = [
            ADMITTED_CERTIFICATE_RAW_ACCOUNT_V3,
            ADMITTED_CERTIFICATE_STAGING_ACCOUNT_V3,
            ADMITTED_ADMISSION_RAW_ACCOUNT_V3,
            ADMITTED_ADMISSION_STAGING_ACCOUNT_V3,
            ADMITTED_ARTIFACT_RAW_ACCOUNT_V3,
            ADMITTED_ARTIFACT_STAGING_ACCOUNT_V3,
            ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3,
            ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3,
        ];
        assert_eq!(evidence.len(), ADMITTED_STRATEGY_EVIDENCE_COUNT_V3);
        for (index, coordinate) in evidence.iter().enumerate() {
            assert_eq!(*coordinate, ADMITTED_STRATEGY_EVIDENCE_START_V3 + index);
        }
        assert_eq!(
            ADMITTED_RUNTIME_ACCOUNTS_START_V3,
            ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3 + 1
        );

        // The appended output page moves no coordinate the chunked transport
        // reads: it takes the first slot after the evidence suffix, which is
        // where the chunked runtime slice starts, and pushes only the
        // output-page transport's own runtime slice.
        assert_eq!(
            ADMITTED_OUTPUT_PAGE_ACCOUNT_V3,
            ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3 + 1
        );
        assert_eq!(
            ADMITTED_OUTPUT_PAGE_RUNTIME_ACCOUNTS_START_V3,
            ADMITTED_OUTPUT_PAGE_ACCOUNT_V3 + 1
        );
    }
}

/// Magic of one caller-composed admitted prelude witness.
pub const ADMITTED_PRELUDE_WITNESS_MAGIC_V1: [u8; 8] = *b"dcAPWv1\0";

/// Schema version of the only prelude-witness body this contract admits.
pub const ADMITTED_PRELUDE_WITNESS_SCHEMA_V1: u16 = 1;

/// Fixed header bytes before one witness's context preimage.
pub const ADMITTED_PRELUDE_WITNESS_HEADER_BYTES_V1: usize = 20;

/// Exact bytes of one `AdmittedInvocationContextV3` preimage.
pub const ADMITTED_INVOCATION_CONTEXT_BYTES_V3: usize = 23 * 32 + 5 * 4;

/// Exact bytes of one witness carrying `accounts` coordinates and `spans` widths.
pub const fn admitted_prelude_witness_bytes_v1(accounts: usize, spans: usize) -> usize {
    ADMITTED_PRELUDE_WITNESS_HEADER_BYTES_V1
        + ADMITTED_INVOCATION_CONTEXT_BYTES_V3
        + spans * 4
        + accounts * 2
}

/// Borrowed exact caller-composed prelude witness.
///
/// # What this is, and what it is not
///
/// It is the OUTPUT of the prelude chain the caller already ran: the complete
/// [`AdmittedInvocationContextV3`] preimage whose digest the request header
/// already carries, plus the two AccountProfile-derived geometry banks a callee
/// would otherwise decode five sealed artifacts to reproduce. Every byte of it
/// is inside `hash(request_bytes)`, which is the last seed of the
/// `CallerAuthoritySeedsV1` PDA the caller must sign, so it is a signed
/// statement by the program at the caller coordinate and not a hint.
///
/// It is NOT a licence to skip the callee's own reading. A callee that consumes
/// this is expected to re-derive every field it holds an independent source for
/// -- the coordinates its frame names, the digests its own accounts hash to,
/// the identities a persisted first-party verdict already fixes -- and to
/// refuse on the first disagreement. What the witness buys is the SEARCH and
/// the DECODE, not the conjunct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedPreludeWitnessV1<'a> {
    context: AdmittedInvocationContextV3,
    span_widths: &'a [u8],
    representatives: &'a [u8],
}

impl<'a> AdmittedPreludeWitnessV1<'a> {
    /// Hostile-decode one exact witness body.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, AdmittedTranscriptErrorV3> {
        if slice(bytes, 0, 8)? != ADMITTED_PRELUDE_WITNESS_MAGIC_V1 {
            return Err(AdmittedTranscriptErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != ADMITTED_PRELUDE_WITNESS_SCHEMA_V1 {
            return Err(AdmittedTranscriptErrorV3::UnsupportedSchema);
        }
        if read_u16(bytes, 10)? != 0 {
            return Err(AdmittedTranscriptErrorV3::NonCanonicalReserved);
        }
        let accounts = usize_from(read_u32(bytes, 12)?)?;
        let spans = usize_from(read_u32(bytes, 16)?)?;
        if bytes.len() != admitted_prelude_witness_bytes_v1(accounts, spans) {
            return Err(AdmittedTranscriptErrorV3::InvalidLength);
        }
        let context = decode_invocation_context_v3(slice(
            bytes,
            ADMITTED_PRELUDE_WITNESS_HEADER_BYTES_V1,
            ADMITTED_INVOCATION_CONTEXT_BYTES_V3,
        )?)?;
        // The context's own account count is the width the two banks are read
        // at, so a header that disagrees with the body it introduces is refused
        // before either bank is sliced: there is no second opinion about how
        // wide a bank is.
        if usize_from(context.account_count)? != accounts {
            return Err(AdmittedTranscriptErrorV3::InvalidLength);
        }
        let spans_offset =
            ADMITTED_PRELUDE_WITNESS_HEADER_BYTES_V1 + ADMITTED_INVOCATION_CONTEXT_BYTES_V3;
        Ok(Self {
            context,
            span_widths: slice(bytes, spans_offset, spans * 4)?,
            representatives: slice(bytes, spans_offset + spans * 4, accounts * 2)?,
        })
    }

    /// Exact complete invocation-context preimage the caller committed.
    pub const fn context(self) -> AdmittedInvocationContextV3 {
        self.context
    }

    /// Number of dynamic fixed-span widths this witness carries.
    pub const fn span_count(self) -> usize {
        self.span_widths.len() / 4
    }

    /// One dynamic fixed-span width, in descriptor order.
    pub fn span_width(self, index: usize) -> Result<u32, AdmittedTranscriptErrorV3> {
        read_u32(self.span_widths, index * 4)
    }

    /// One logical coordinate's representative coordinate.
    pub fn representative(self, index: usize) -> Result<usize, AdmittedTranscriptErrorV3> {
        let value = usize::from(read_u16(self.representatives, index * 2)?);
        // A representative names a coordinate of THIS frame, and the frame's
        // width is a context field, so an out-of-range one is refused here
        // rather than handed to a caller that would index with it.
        if value >= self.representatives.len() / 2 {
            return Err(AdmittedTranscriptErrorV3::InvalidLength);
        }
        Ok(value)
    }

    /// Encode one exact witness body for `context` and its two geometry banks.
    pub fn encode_into(
        context: AdmittedInvocationContextV3,
        span_widths: &[u32],
        representatives: &[usize],
        output: &mut [u8],
    ) -> Result<(), AdmittedTranscriptErrorV3> {
        let accounts = representatives.len();
        if usize_from(context.account_count)? != accounts
            || output.len() != admitted_prelude_witness_bytes_v1(accounts, span_widths.len())
        {
            return Err(AdmittedTranscriptErrorV3::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &ADMITTED_PRELUDE_WITNESS_MAGIC_V1)?;
        put(output, 8, &ADMITTED_PRELUDE_WITNESS_SCHEMA_V1.to_le_bytes())?;
        put(
            output,
            12,
            &u32::try_from(accounts)
                .map_err(|_| AdmittedTranscriptErrorV3::InvalidLength)?
                .to_le_bytes(),
        )?;
        put(
            output,
            16,
            &u32::try_from(span_widths.len())
                .map_err(|_| AdmittedTranscriptErrorV3::InvalidLength)?
                .to_le_bytes(),
        )?;
        encode_invocation_context_v3(
            context,
            output
                .get_mut(
                    ADMITTED_PRELUDE_WITNESS_HEADER_BYTES_V1
                        ..ADMITTED_PRELUDE_WITNESS_HEADER_BYTES_V1
                            + ADMITTED_INVOCATION_CONTEXT_BYTES_V3,
                )
                .ok_or(AdmittedTranscriptErrorV3::InvalidLength)?,
        )?;
        let spans_offset =
            ADMITTED_PRELUDE_WITNESS_HEADER_BYTES_V1 + ADMITTED_INVOCATION_CONTEXT_BYTES_V3;
        for (index, width) in span_widths.iter().enumerate() {
            put(output, spans_offset + index * 4, &width.to_le_bytes())?;
        }
        let representatives_offset = spans_offset + span_widths.len() * 4;
        for (index, coordinate) in representatives.iter().enumerate() {
            let value =
                u16::try_from(*coordinate).map_err(|_| AdmittedTranscriptErrorV3::InvalidLength)?;
            if usize::from(value) >= accounts {
                return Err(AdmittedTranscriptErrorV3::InvalidLength);
            }
            put(
                output,
                representatives_offset + index * 2,
                &value.to_le_bytes(),
            )?;
        }
        Ok(())
    }
}

fn usize_from(value: u32) -> Result<usize, AdmittedTranscriptErrorV3> {
    usize::try_from(value).map_err(|_| AdmittedTranscriptErrorV3::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], AdmittedTranscriptErrorV3> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(len)
                    .ok_or(AdmittedTranscriptErrorV3::InvalidLength)?,
        )
        .ok_or(AdmittedTranscriptErrorV3::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, AdmittedTranscriptErrorV3> {
    let raw: [u8; 2] = slice(bytes, offset, 2)?
        .try_into()
        .map_err(|_| AdmittedTranscriptErrorV3::InvalidLength)?;
    Ok(u16::from_le_bytes(raw))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, AdmittedTranscriptErrorV3> {
    let raw: [u8; 4] = slice(bytes, offset, 4)?
        .try_into()
        .map_err(|_| AdmittedTranscriptErrorV3::InvalidLength)?;
    Ok(u32::from_le_bytes(raw))
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<[u8; 32], AdmittedTranscriptErrorV3> {
    slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| AdmittedTranscriptErrorV3::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), AdmittedTranscriptErrorV3> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(AdmittedTranscriptErrorV3::InvalidLength)?,
        )
        .ok_or(AdmittedTranscriptErrorV3::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

/// Write one context preimage in the exact order its digest commits.
///
/// The order is [`admitted_invocation_context_digest_v3`]'s preimage minus its
/// domain, so a reader that decodes this and re-digests reproduces the request
/// header's `invocation_context` or refuses. One order, one file.
pub fn encode_invocation_context_v3(
    context: AdmittedInvocationContextV3,
    output: &mut [u8],
) -> Result<(), AdmittedTranscriptErrorV3> {
    if output.len() != ADMITTED_INVOCATION_CONTEXT_BYTES_V3 {
        return Err(AdmittedTranscriptErrorV3::InvalidLength);
    }
    let artifact_release = context.artifact_release.to_bytes();
    for (index, identity) in [
        context.release_set.as_bytes(),
        context.market.as_bytes(),
        context.root.as_bytes(),
        context.registry_program.as_bytes(),
        context.trading_program.as_bytes(),
        context.accelerator_program.as_bytes(),
        context.capability_program.as_bytes(),
        context.account_profile.as_bytes(),
        context.request_profile.as_bytes(),
        context.transition.as_bytes(),
        context.effect.as_bytes(),
        context.lifecycle.as_bytes(),
        context.strategy.as_bytes(),
        context.certificate.as_bytes(),
        context.admission.as_bytes(),
        &artifact_release,
        context.config.as_bytes(),
        context.product.as_bytes(),
        context.portfolio.as_bytes(),
        context.linked_basis.as_bytes(),
        context.family_request_digest.as_bytes(),
        context.runtime_observations_digest.as_bytes(),
        context.root_prestate_digest.as_bytes(),
    ]
    .into_iter()
    .enumerate()
    {
        put(output, index * 32, identity)?;
    }
    for (index, value) in [
        context.selected_action,
        context.tail_count,
        context.account_count,
        context.scalar_count,
        context.identity_count,
    ]
    .into_iter()
    .enumerate()
    {
        put(output, 23 * 32 + index * 4, &value.to_le_bytes())?;
    }
    Ok(())
}

/// Hostile-decode one context preimage written by [`encode_invocation_context_v3`].
pub fn decode_invocation_context_v3(
    bytes: &[u8],
) -> Result<AdmittedInvocationContextV3, AdmittedTranscriptErrorV3> {
    if bytes.len() != ADMITTED_INVOCATION_CONTEXT_BYTES_V3 {
        return Err(AdmittedTranscriptErrorV3::InvalidLength);
    }
    let identity = |index: usize| -> Result<ContentId, AdmittedTranscriptErrorV3> {
        ContentId::new(read_identity(bytes, index * 32)?)
            .map_err(|_| AdmittedTranscriptErrorV3::ZeroDigest)
    };
    let count = |index: usize| read_u32(bytes, 23 * 32 + index * 4);
    Ok(AdmittedInvocationContextV3 {
        release_set: identity(0)?,
        market: identity(1)?,
        root: identity(2)?,
        registry_program: identity(3)?,
        trading_program: identity(4)?,
        accelerator_program: identity(5)?,
        capability_program: identity(6)?,
        account_profile: identity(7)?,
        request_profile: identity(8)?,
        transition: identity(9)?,
        effect: identity(10)?,
        lifecycle: identity(11)?,
        strategy: identity(12)?,
        certificate: identity(13)?,
        admission: identity(14)?,
        artifact_release: ArtifactReleaseIdV1::new(read_identity(bytes, 15 * 32)?)
            .map_err(|_| AdmittedTranscriptErrorV3::ZeroDigest)?,
        config: identity(16)?,
        product: identity(17)?,
        portfolio: identity(18)?,
        linked_basis: identity(19)?,
        family_request_digest: identity(20)?,
        runtime_observations_digest: identity(21)?,
        root_prestate_digest: identity(22)?,
        selected_action: count(0)?,
        tail_count: count(1)?,
        account_count: count(2)?,
        scalar_count: count(3)?,
        identity_count: count(4)?,
    })
}
