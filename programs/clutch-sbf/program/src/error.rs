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
//! | `0x1000 + n` | [`clutch_solana_layout::CodecError`] variant `n` |
//! | `0x2000 + n` | [`clutch_kernel::Error`] variant `n` |
//! | `0x3000 + n` | [`clutch_solana_reference::Error`] variant `n` |

use clutch_kernel::Error as KernelError;
use clutch_solana_layout::CodecError;
use clutch_solana_reference::Error as ReferenceError;
use solana_program_error::ProgramError;

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
        // See the note in `codec_code`.
        _ => 0xfff,
    };
    0x2000 + ordinal
}

/// Stable ordinal for a reference-only codec refusal.
///
/// Only the reference-only account and request codecs are reachable from this
/// program; the reference transition function itself is never called here.
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
        // See the note in `codec_code`.
        _ => 0x3fff,
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
}

impl Refusal {
    /// The stable numeric code reported to the runtime.
    pub const fn code(self) -> u32 {
        match self {
            Self::Adapter(error) => error as u32,
            Self::Codec(error) => codec_code(error),
            Self::Kernel(error) => kernel_code(error),
            Self::Reference(error) => reference_code(error),
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

impl From<Refusal> for ProgramError {
    fn from(value: Refusal) -> Self {
        ProgramError::Custom(value.code())
    }
}
