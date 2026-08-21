//! Stable numeric refusal codes.
//!
//! Every refusal this program can produce maps to one explicit
//! `ProgramError::Custom(code)`.  The codes are chosen so that a refusal
//! observed in a transaction log names the exact check that fired, and so that
//! codec and kernel refusals stay distinguishable from adapter refusals.
//!
//! Range allocation:
//!
//! | range | meaning |
//! | --- | --- |
//! | `0x0001..=0x00ff` | account/runtime metadata and adapter checks |
//! | `0x0018..=0x001e` | the token plane's appends ([`ClutchError::WrongTokenProgram`] .. [`ClutchError::ShadowSupplyMismatch`]) |
//! | `0x0040..=0x004f` | market-initialization appends ([`ClutchError::AlreadyInitialized`] .. [`ClutchError::TermsBindingMismatch`]) |
//! | `0x0050..=0x005f` | the evidence gate's numeric projection (see below) |
//! | `0x0070..=0x007f` | construction and typed-artifact appends ([`ClutchError::WrongSystemProgram`] .. [`ClutchError::ArtifactRefundMismatch`]) |
//! | `0x0080..=0x008d` | resumable ResolutionWork semantic refusals |
//! | `0x0090..=0x009f` | the clearing walk's checkpoint/feed seam and the revenue admission boundary ([`ClutchError::CheckpointCodecFault`] .. [`ClutchError::RevenuePolicyRecordMissing`]) |
//! | `0x1000 + n` | [`clutch_solana_layout::CodecError`] variant `n` |
//! | `0x2000 + n` | [`clutch_kernel::Error`] variant `n` |
//! | `0x3000 + n` | [`clutch_solana_reference::Error`] variant `n` |
//!
//! The `0x0050-0x005f` block realizes the allocation `observe_resolve`'s
//! module docs proposed while this file was frozen: the eleven evidence-gate
//! classes that used to collapse onto the `0x3fff` catch-all each project to
//! their own number. The sub-reasons inside `Window(_)` and `Resolution(_)`
//! stay collapsed on-chain — they remain exactly distinguishable in the host
//! differential, which compares typed values — because minting a numeric
//! identifier per sub-reason would be a parallel truth, not a diagnostic.
//! `0x005b-0x005f` stay unallocated.
//!
//! The `0x0070-0x007f` block belongs to [`crate::instructions::genesis`], the
//! first family that creates accounts rather than writing over pre-created
//! ones.  Typed artifact transport extends that construction plane at
//! `0x0074..=0x0078`; `0x0079-0x007f` stay free.  `0x0060-0x006f` is
//! deliberately skipped: leaving a gap between the
//! evidence block and the genesis block costs nothing and keeps a later
//! evidence append from having to jump over an unrelated family.

use clutch_kernel::Error as KernelError;
use clutch_solana_layout::{resolution_work::ResolutionWorkCodecError, CodecError};
use clutch_solana_reference::Error as ReferenceError;
use solana_program_error::ProgramError;

use crate::instructions::resolution_work::ResolutionWorkError;

/// Adapter-level refusals raised by this program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClutchError {
    /// The instruction did not carry exactly the expected account count.
    AccountCount = 0x0001,
    /// The acting account did not present an authenticated signature.
    MissingSignature = 0x0002,
    /// Two logical roles were filled by one account key.
    AccountAlias = 0x0003,
    /// A state account was not owned by this program.
    WrongProgramOwner = 0x0004,
    /// A state account that must be mutated was not declared writable.
    NotWritable = 0x0005,
    /// A read-only role was declared writable.
    UnexpectedWritable = 0x0006,
    /// A state account was executable.
    ExecutableAccount = 0x0007,
    /// A state account had the wrong exact data length.
    WrongDataLength = 0x0008,
    /// A supplied account key was not the canonical program-derived address.
    WrongPda = 0x0009,
    /// A stored bump differed from the canonical derived bump.
    WrongBump = 0x000a,
    /// Account identities, generations, phases, or immutable fields disagreed.
    MismatchedState = 0x000b,
    /// A padding, reserved, or enum field was non-canonical.
    NonCanonical = 0x000c,
    /// A request sequence was stale, skipped, or exhausted.
    Replay = 0x000d,
    /// The instruction is outside this deliberately tiny bring-up subset.
    UnsupportedInstruction = 0x000e,
    /// No authority policy exists for this action, so it fails closed.
    AuthorizationUnavailable = 0x000f,
    /// No typed maturity, sealed-window, source, terms, and payout evidence exists.
    ResolutionEvidenceUnavailable = 0x0010,
    /// The signer was authenticated but is not the position owner.
    UnauthorizedActor = 0x0011,
    /// The immutable market collateral cap would be exceeded.
    CollateralCap = 0x0012,
    /// A checked arithmetic operation overflowed or underflowed.
    Arithmetic = 0x0013,
    /// The closed single-position model's local claims did not equal aggregate supply.
    AggregateClosureMismatch = 0x0014,
    /// An account data borrow failed.
    AccountBorrowFailed = 0x0015,
    /// The market lifecycle or position close-state forbids this transition.
    NotActive = 0x0016,
    /// The instruction family exists in this program's dispatch but its
    /// transition is not written yet.
    ///
    /// This is deliberately distinct from [`ClutchError::UnsupportedInstruction`],
    /// which says "outside this program's scope", and from the two structural
    /// fail-closed refusals [`ClutchError::AuthorizationUnavailable`] and
    /// [`ClutchError::ResolutionEvidenceUnavailable`], which say "the missing
    /// thing is a policy or an evidence chain, not code".  A caller that sees
    /// this code is looking at an honest stub: no state was read, no state was
    /// written, and nothing was faked.
    NotYetImplemented = 0x0017,
    /// A mint or token account was not owned by the pinned token program.
    ///
    /// The pinned program is `clutch_solana_layout::collateral::TOKEN_2022_PROGRAM`.
    /// A non-executable account at the token-program role reports this too: an
    /// account that cannot be invoked is not the token program regardless of
    /// what its key says.
    WrongTokenProgram = 0x0018,
    /// A mint failed its admission policy on a non-extension ground.
    ///
    /// Identity, decimals, supply ceiling, zero supply, or a surviving mint or
    /// freeze authority.  Deliberately separate from
    /// [`ClutchError::TokenExtensionNotAllowed`] so a transaction log
    /// distinguishes "this is the wrong asset" from "this asset carries
    /// behaviour the Realm forbids".
    MintNotAdmitted = 0x0019,
    /// A present extension is outside the effective allowed set, a required
    /// extension is absent, or the TLV region did not parse.
    ///
    /// An extension discriminant this build does not know is *this* refusal
    /// and not a shrug: the fail-closed rule of
    /// `docs/implementation/COLLATERAL_PROFILES.md`.  The offending
    /// discriminant is not encoded in the number — one `ProgramError::Custom`
    /// carries one `u32` — and is recovered from the mint bytes by
    /// [`crate::token::first_denied_extension`], exactly as the sub-reasons of
    /// `Window(_)` stay host-differential facts.
    TokenExtensionNotAllowed = 0x001a,
    /// A token account failed the Hoard or holder policy on a non-extension
    /// ground: wrong mint, frozen, wrong owner authority, or a delegate or
    /// close authority the policy refuses.
    TokenAccountNotAdmitted = 0x001b,
    /// An observed post-CPI balance or supply delta was not the exact expected
    /// one.
    ///
    /// Not `>=`, not "at least": `docs/implementation/TOKEN2022_PLAN.md` §3.3
    /// step 6.  This is what makes solvency independent of the extension
    /// refusal being complete, and it is also what makes the off-chain no-op
    /// `solana_cpi::invoke_signed` detectable rather than silent.
    TokenDeltaMismatch = 0x001c,
    /// The Hoard token account does not cover
    /// `HoardAccount::collateral_atoms`.
    ///
    /// In the pooled-custody model, the accounting field is locked claim
    /// backing while the token account also contains Position cash and
    /// unsolicited surplus. Equality is therefore not required; this refusal
    /// means the one-sided coverage floor `token_amount >= collateral_atoms`
    /// failed.
    HoardMirrorMismatch = 0x001d,
    /// An observed outcome mint supply exceeded the last atomically persisted
    /// Token-2022 supply.
    ///
    /// **Appended beyond the plan's table**, which stops at `0x001d`.  The
    /// plan's `HoardMirrorMismatch` names the *collateral* mirror; this names
    /// the *outcome* mirror, and collapsing them would make a diagnostic
    /// unable to say which of two different single-truth cutovers broke.  The
    /// A lower supply is an ordinary direct holder burn and is synchronized as
    /// a safe liability donation.  A higher supply cannot be accepted because
    /// only this program's market PDA is admitted as mint authority and every
    /// authorized mint persists its exact post-CPI supply atomically.
    ShadowSupplyMismatch = 0x001e,
    /// A target of an initialization write was not all-zero.
    ///
    /// The account-plane re-initialization refusal: a market that already
    /// exists has nonzero bytes at its canonical address, so a second
    /// `CreateMarket` refuses here before deriving anything.  Distinct from
    /// the reference's `NonEmptyInitialization` (`0x3010`), which speaks for
    /// a nonzero *initial value* inside otherwise-decodable state.
    AlreadyInitialized = 0x0040,
    /// The Realm Profile's collateral policy is not frozen.
    ///
    /// [`reference_code`] projects the reference vocabulary's own
    /// `CollateralPolicyNotFrozen` onto this same number: one check, one
    /// code, whichever plane raised it.
    CollateralPolicyNotFrozen = 0x0041,
    /// The kernel payout set is not the immutable terms' payout set.
    PayoutSetMismatch = 0x0042,
    /// The immutable terms artifact does not bind this market, or the
    /// market's stored collateral cap is not the terms' digest-committed cap.
    TermsBindingMismatch = 0x0043,
    /// The account at the system-program role is not the system program.
    ///
    /// The system program's address is the all-zero key, which is also what an
    /// uninitialized account slot looks like, so this check reads the
    /// executable bit too: an account that cannot be invoked is not the system
    /// program regardless of what its key says.  Exactly the reading
    /// [`ClutchError::WrongTokenProgram`] already applies to the token
    /// program, for exactly the same reason.
    WrongSystemProgram = 0x0070,
    /// The account at the rent-sysvar role is not the rent sysvar.
    ///
    /// Raised for a wrong key, a wrong data length, or a writable
    /// declaration.  The genesis plane reads rent parameters off the chain
    /// rather than pinning them as constants, so this account is evidence and
    /// is checked like evidence.
    WrongRentSysvar = 0x0071,
    /// The `CreateAccount` cross-program invocation refused.
    ///
    /// Most often an unfunded payer or an address the runtime already knows,
    /// and the system program's own error is not recoverable through
    /// `ProgramError::Custom`; the transaction log carries the inner
    /// instruction's error alongside this one.  Deliberately distinct from
    /// [`ClutchError::AlreadyInitialized`], which fires *before* any CPI.
    AccountCreationFailed = 0x0072,
    /// A presented artifact buffer is not the artifact the intent names.
    ///
    /// The evidence-buffer pattern's refusal: a terms body or a price-grid
    /// body whose recomputed digest is not the one the intent declared, or
    /// whose Realm is not the Realm the account plane authenticated.  It is a
    /// *recomputation* failure and not a decode failure — a perfectly
    /// well-formed artifact for some other market earns this code.
    EvidenceBufferMismatch = 0x0073,
    /// The presented Clock sysvar had the wrong key or exact byte length.
    WrongClockSysvar = 0x0074,
    /// An upload write or seal arrived after the stage's immutable expiry.
    ArtifactExpired = 0x0075,
    /// Seal was attempted before the exact body length had been written.
    ArtifactIncomplete = 0x0076,
    /// The requested upload lifetime is shorter or longer than the frozen
    /// bounded range.
    InvalidArtifactExpiry = 0x0077,
    /// Abort's refund account is not the funder persisted at Begin.
    ArtifactRefundMismatch = 0x0078,
    /// Immutable terms selected no source parser/deployment release compiled
    /// into this exact program artifact.
    ///
    /// The default production build currently raises this for every release;
    /// a separately flagged non-production feature registers one deterministic
    /// mock solely for local-bank lifecycle evidence.
    SourceReleaseUnavailable = 0x0079,
    /// A registered source release refused provider deployment, source bytes,
    /// freshness, lineage, confidence, window, or archive provenance.
    SourceAdmissionFailed = 0x007a,
    /// The checkpoint account's body bytes are not a `ClearWorkV1` encoding.
    ///
    /// `clutch_batch::relation_v1_stream::CodecFaultV1`, collapsed: the bytes
    /// are not a checkpoint, so there is no feed to have a protocol fault in
    /// and no relation verdict.  The typed sub-fault stays exactly
    /// distinguishable in the host suites; minting one number per variant
    /// would be a parallel truth, not a diagnostic.
    CheckpointCodecFault = 0x0090,
    /// The feed protocol refused a push or a pass boundary
    /// (`clutch_batch::relation_v1_stream::FeedErrorV1`, minus the resumption
    /// mismatch, which has its own code below).
    FeedProtocolFault = 0x0091,
    /// A resumed pass is not the continuation of the pass-1 sequence: the
    /// codec's own fold seal refused (`FeedErrorV1::ResumeFoldMismatch`), or
    /// the program's anchor comparison `body.consumed_fold() !=
    /// header.consumed_fold` caught a substituted checkpoint body.
    ResumeFoldMismatch = 0x0092,
    /// Fee-bearing epoch admission refused: the Realm's pinned revenue
    /// policy carries the structural treasury-UNSET sentinel — the B4a
    /// deferral (`ADOPTED_2026-08-20.md` item 8).  No fee-bearing epoch can
    /// open until a frozen const naming a real treasury exists, and binding
    /// that key is reserved to ember.
    RevenueTreasuryUnset = 0x0093,
    /// Fee-bearing epoch admission refused: the Realm has no revenue-policy
    /// record.  The record's absence IS the zero-take state (D4), and a
    /// record is creatable only inside the Realm-creation transition, so
    /// this refusal is permanent for every existing Realm.
    RevenuePolicyRecordMissing = 0x0094,
    /// A sealing candidate cannot beat the worst retained candidate's stored
    /// score, so the full registry admits no displacement for it.
    ///
    /// Deliberately its own code rather than a `MismatchedState`: nothing is
    /// inconsistent — the registry is simply better on its components — and
    /// the staged pair survives, so the same seal may legitimately succeed
    /// later if verification lowers a retained claim below this one.
    CandidateNotCompetitive = 0x00a0,
    /// A verified candidate's stored tie digest does not equal the digest
    /// re-derived from the full-width domain and its feed's stored regions.
    ///
    /// Selection's tamper gate: a forged `score_digest` on the record, or a
    /// post-verification edit of the feed's fills or declared witness, both
    /// land here — the digest is recomputed from the presented bytes, never
    /// compared claim-to-claim.
    ScoreDigestMismatch = 0x00a1,
}

impl From<ClutchError> for ProgramError {
    fn from(value: ClutchError) -> Self {
        ProgramError::Custom(value as u32)
    }
}

/// Stable ordinal for a frozen-layout codec refusal.
#[allow(unreachable_patterns)]
pub const fn codec_code(error: CodecError) -> u32 {
    let ordinal = match error {
        CodecError::Truncated => 0,
        CodecError::TrailingBytes => 1,
        CodecError::WrongTag => 2,
        CodecError::WrongVersion => 3,
        CodecError::InvalidCount => 4,
        CodecError::InvalidEnum => 5,
        CodecError::ZeroValue => 6,
        CodecError::ZeroIdentity => 7,
        CodecError::NonCanonicalIdentity => 8,
        CodecError::NonCanonicalPadding => 9,
        CodecError::ArithmeticOverflow => 10,
        CodecError::OutputTooSmall => 11,
        CodecError::InvalidPriceGrid => 12,
        CodecError::InvalidTick => 13,
        CodecError::MismatchedBinding => 14,
        CodecError::AggregateClosureMismatch => 15,
        CodecError::InvalidConsideration => 16,
        /* A refusal added to the frozen layout after this table was written.
         * It is reported, not swallowed, but it has no code of its own yet and
         * must be given one before this program is anything but a bring-up
         * probe. */
        _ => 0xfff,
    };
    0x1000 + ordinal
}

/// Stable ordinal for a pure-kernel refusal.
#[allow(unreachable_patterns)]
pub const fn kernel_code(error: KernelError) -> u32 {
    let ordinal = match error {
        KernelError::InvalidOutcomeCount => 0,
        KernelError::InvalidPayoutCount => 1,
        KernelError::InvalidPayoutIndex => 2,
        KernelError::InvalidDenominator => 3,
        KernelError::InvalidPayoutWeights => 4,
        KernelError::ZeroQuantity => 5,
        KernelError::ArithmeticOverflow => 6,
        KernelError::ArithmeticUnderflow => 7,
        KernelError::InsufficientBalance => 8,
        KernelError::InsufficientCollateral => 9,
        KernelError::NotActive => 10,
        KernelError::AlreadyResolved => 11,
        KernelError::NotResolved => 12,
        KernelError::InvariantViolation => 13,
        KernelError::RemainderRequired => 14,
        KernelError::WrongResolutionMode => 15,
        // See the note in `codec_code`.
        _ => 0xfff,
    };
    0x2000 + ordinal
}

/// Stable ordinal for a reference-only refusal.
///
/// The evidence-gate classes project onto the `0x0050-0x005f` block that
/// `observe_resolve` reserved (see the module docs above): the class is the
/// number, and the sub-reason a `Window(_)` or `Resolution(_)` carries stays
/// a host-differential fact rather than a second on-chain identifier.
/// `CollateralPolicyNotFrozen` projects onto the adapter append `0x0041`, so
/// a transaction log names that check by one number whichever vocabulary
/// raised it.
#[allow(unreachable_patterns)]
pub const fn reference_code(error: ReferenceError) -> u32 {
    match error {
        ReferenceError::Layout(inner) => codec_code(inner),
        ReferenceError::Kernel(inner) => kernel_code(inner),
        ReferenceError::WrongLength => 0x3000,
        ReferenceError::WrongTag => 0x3001,
        ReferenceError::WrongVersion => 0x3002,
        ReferenceError::NonCanonical => 0x3003,
        ReferenceError::Arithmetic => 0x3004,
        ReferenceError::WrongProgramOwner => 0x3005,
        ReferenceError::AccountAlias => 0x3006,
        ReferenceError::WrongAccountKey => 0x3007,
        ReferenceError::NotWritable => 0x3008,
        ReferenceError::MissingSignature => 0x3009,
        ReferenceError::UnauthorizedActor => 0x300a,
        ReferenceError::AuthorizationUnavailable => 0x300b,
        ReferenceError::ResolutionEvidenceUnavailable => 0x300c,
        ReferenceError::WrongBump => 0x300d,
        ReferenceError::MismatchedState => 0x300e,
        ReferenceError::AggregateClosureMismatch => 0x300f,
        ReferenceError::NonEmptyInitialization => 0x3010,
        ReferenceError::Replay => 0x3011,
        ReferenceError::UnsupportedIntent => 0x3012,
        ReferenceError::CollateralCap => 0x3013,
        ReferenceError::CollateralPolicyNotFrozen => ClutchError::CollateralPolicyNotFrozen as u32,
        ReferenceError::Window(_) => 0x0050,
        ReferenceError::Resolution(_) => 0x0051,
        ReferenceError::TermsBindingMismatch => 0x0052,
        ReferenceError::PayoutSetMismatch => 0x0053,
        ReferenceError::ResolutionBindingMismatch => 0x0054,
        ReferenceError::ResolutionAlreadyRecorded => 0x0055,
        ReferenceError::ResolutionNotRecorded => 0x0056,
        ReferenceError::PayoutIndexMismatch => 0x0057,
        ReferenceError::ImmutableAccountWritable => 0x0058,
        ReferenceError::UnexpectedEvidence => 0x0059,
        ReferenceError::WindowIdentityUnavailable => 0x005a,
        // See the note in `codec_code`.
        _ => 0x3fff,
    }
}

/// Stable numeric projection for resumable ResolutionWork semantics.
///
/// Nested codec/archive/accumulator reasons remain typed host evidence; the
/// on-chain class still says which trust boundary refused the transition.
pub const fn resolution_work_code(error: ResolutionWorkError) -> u32 {
    match error {
        ResolutionWorkError::Codec(_) => 0x0080,
        ResolutionWorkError::OutputCodec(_) => 0x0081,
        ResolutionWorkError::Terms(_) => 0x0082,
        ResolutionWorkError::Accumulator(_) => 0x0083,
        ResolutionWorkError::Archive(_) => 0x0084,
        ResolutionWorkError::BindingMismatch => 0x0085,
        ResolutionWorkError::WrongCursor => 0x0086,
        ResolutionWorkError::InvalidChunk => 0x0087,
        ResolutionWorkError::Expired => 0x0088,
        ResolutionWorkError::InvalidSlot => 0x0089,
        ResolutionWorkError::NotAtEnd => 0x008a,
        ResolutionWorkError::Underfunded => 0x008b,
        ResolutionWorkError::AbortForbidden => 0x008c,
        ResolutionWorkError::ArithmeticOverflow => 0x008d,
    }
}

/// One refusal type for the whole processor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// Adapter-level refusal.
    Adapter(ClutchError),
    /// Frozen-layout codec refusal.
    Codec(CodecError),
    /// Pure-kernel refusal.
    Kernel(KernelError),
    /// Reference-only codec refusal.
    Reference(ReferenceError),
    /// Resumable occupation-resolution refusal.
    ResolutionWork(ResolutionWorkError),
}

impl Refusal {
    /// The stable numeric code reported to the runtime.
    pub const fn code(self) -> u32 {
        match self {
            Self::Adapter(error) => error as u32,
            Self::Codec(error) => codec_code(error),
            Self::Kernel(error) => kernel_code(error),
            Self::Reference(error) => reference_code(error),
            Self::ResolutionWork(error) => resolution_work_code(error),
        }
    }
}

impl From<ClutchError> for Refusal {
    fn from(value: ClutchError) -> Self {
        Self::Adapter(value)
    }
}

impl From<CodecError> for Refusal {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl From<KernelError> for Refusal {
    fn from(value: KernelError) -> Self {
        Self::Kernel(value)
    }
}

impl From<ReferenceError> for Refusal {
    fn from(value: ReferenceError) -> Self {
        Self::Reference(value)
    }
}

impl From<ResolutionWorkError> for Refusal {
    fn from(value: ResolutionWorkError) -> Self {
        Self::ResolutionWork(value)
    }
}

impl From<ResolutionWorkCodecError> for Refusal {
    fn from(value: ResolutionWorkCodecError) -> Self {
        Self::ResolutionWork(ResolutionWorkError::Codec(value))
    }
}

impl From<Refusal> for ProgramError {
    fn from(value: Refusal) -> Self {
        ProgramError::Custom(value.code())
    }
}
