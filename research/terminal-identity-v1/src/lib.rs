// SPDX-License-Identifier: AGPL-3.0-or-later
//! TerminalIdentityV1: the uniform rent/donation header of the R4 terminal
//! lifecycle design, as a fixed-layout codec plus a lifecycle value model
//! that delegates every economic judgment to the `clutch-liveness`
//! [`DonationLedger`] kernel.
//!
//! Status: **PROPOSED / MODEL-ONLY** — interim step 3 of
//! `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md` §9. No SBF program,
//! account family, or gate uses this header yet; nothing here promotes any
//! surface. §1 of that design is controlling for the semantics below.
//!
//! The neutral sink is a parameter everywhere in this crate. The design's
//! decision 10 pins the runtime value to one frozen program-wide sink: the
//! incinerator (`RESOLUTION_WORK_NEUTRAL_SINK_V1` generalizes). The crate
//! deliberately does not restate that constant; it refuses only the
//! relations the header can check (`payer != sink`, both live identities).

#![no_std]
#![forbid(unsafe_code)]

pub use clutch_liveness::{DonationLedger, Error as LivenessError, Id};

/// Exact persisted byte width of one TerminalIdentityV1 header.
pub const HEADER_BYTES: usize = 56;

/// A fail-closed refusal from the header codec or a lifecycle transition.
///
/// Creation-path identity and funding refusals surface as the delegated
/// kernel error ([`Error::Liveness`]); byte-level and header-shape refusals
/// surface as this crate's own variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A refusal delegated unchanged from the `clutch-liveness` kernel.
    Liveness(LivenessError),
    /// The header byte string is not exactly [`HEADER_BYTES`] long. Both
    /// truncation and trailing bytes are this refusal: the layout is fixed
    /// and nothing may ride behind it.
    HeaderLength,
    /// The payer key is the reserved all-zero padding identity.
    ZeroPayer,
    /// The neutral sink parameter is the reserved all-zero padding identity.
    ZeroNeutralSink,
    /// The payer is the frozen neutral sink; a sink can never be a principal
    /// recipient (design §1 decision 8, the survival of the V2 model's
    /// `refund_to != surplus_sink` check).
    PayerIsNeutralSink,
    /// The recorded payer principal is zero; every headed account records an
    /// exact positive rent debit.
    ZeroPayerPrincipal,
    /// The generation is zero. Generations count close/reopen eras from 1;
    /// zero is reserved as never-created, mirroring [`Id::ZERO`] padding.
    ZeroGeneration,
    /// A live-state transition was attempted on an already closed generation.
    /// Close happens once, ever, per generation.
    AlreadyClosed,
    /// Reopen requires the previous generation to have closed first.
    ReopenBeforeClose,
    /// While live, the caller's accounted compartments must cover the payer
    /// principal; under-accounting it would reclassify principal into the
    /// monotone donation compartment, where it burns instead of refunding.
    AccountedBelowPrincipal,
    /// Close accounts exactly the payer principal and nothing else. Economic
    /// close strictly precedes rent close (design §3.16): every other
    /// compartment must already be disposed when the header closes.
    TerminalAccountedMismatch,
    /// The reopen generation counter would wrap.
    GenerationOverflow,
}

impl From<LivenessError> for Error {
    fn from(error: LivenessError) -> Self {
        Self::Liveness(error)
    }
}

/// The uniform persisted rent/donation header (design §1), byte-layout
/// identical everywhere it appears:
///
/// ```text
/// offset  size  field            encoding
///      0    32  payer            [u8; 32] exact funding wallet
///     32     8  payer_principal  u64 LE, exact lamports debited
///     40     8  donation_floor   u64 LE, monotone DonationLedger lower bound
///     48     8  generation       u64 LE, close/reopen and replay era
/// ```
///
/// Total: 56 bytes, no magic, no padding, nothing behind it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalIdentityV1 {
    /// Exact funding wallet and sole principal recipient.
    pub payer: Id,
    /// Exact lamports debited from the payer after prefund normalization.
    pub payer_principal: u64,
    /// Monotone [`DonationLedger`] lower bound persisted at last observation.
    pub donation_floor: u64,
    /// Close/reopen and replay era, counted from 1.
    pub generation: u64,
}

fn le_word(bytes: &[u8]) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(bytes);
    u64::from_le_bytes(word)
}

impl TerminalIdentityV1 {
    /// Refuse every header shape the layout can express but the design
    /// forbids: zero payer, zero principal, zero generation, and a payer
    /// equal to the frozen neutral sink supplied by the caller.
    pub fn validate(&self, neutral_sink: Id) -> Result<(), Error> {
        if neutral_sink.is_zero() {
            return Err(Error::ZeroNeutralSink);
        }
        if self.payer.is_zero() {
            return Err(Error::ZeroPayer);
        }
        if self.payer == neutral_sink {
            return Err(Error::PayerIsNeutralSink);
        }
        if self.payer_principal == 0 {
            return Err(Error::ZeroPayerPrincipal);
        }
        if self.generation == 0 {
            return Err(Error::ZeroGeneration);
        }
        Ok(())
    }

    /// Encode the validated header into its exact 56-byte layout.
    pub fn encode(&self, neutral_sink: Id) -> Result<[u8; HEADER_BYTES], Error> {
        self.validate(neutral_sink)?;
        let mut bytes = [0u8; HEADER_BYTES];
        bytes[..32].copy_from_slice(&self.payer.bytes());
        bytes[32..40].copy_from_slice(&self.payer_principal.to_le_bytes());
        bytes[40..48].copy_from_slice(&self.donation_floor.to_le_bytes());
        bytes[48..56].copy_from_slice(&self.generation.to_le_bytes());
        Ok(bytes)
    }

    /// Decode exactly [`HEADER_BYTES`] bytes and validate the result.
    /// Truncated input and trailing bytes both refuse.
    pub fn decode(input: &[u8], neutral_sink: Id) -> Result<Self, Error> {
        let fixed: &[u8; HEADER_BYTES] = match input.try_into() {
            Ok(fixed) => fixed,
            Err(_) => return Err(Error::HeaderLength),
        };
        let mut payer = [0u8; 32];
        payer.copy_from_slice(&fixed[..32]);
        let value = Self {
            payer: Id::from_bytes(payer),
            payer_principal: le_word(&fixed[32..40]),
            donation_floor: le_word(&fixed[40..48]),
            generation: le_word(&fixed[48..56]),
        };
        value.validate(neutral_sink)?;
        Ok(value)
    }
}

/// The exact once-only terminal disposition of one generation: the stored
/// payer receives exactly the stored principal, and the entire remaining
/// surplus goes to the neutral sink. `payer_principal + neutral_surplus`
/// equals the closing account balance exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseSplit {
    /// The stored payer, sole principal recipient.
    pub payer: Id,
    /// Exactly the stored principal, never more, never less.
    pub payer_principal: u64,
    /// The entire remaining balance, routed through the kernel's
    /// `terminal_split` to the neutral sink.
    pub neutral_surplus: u64,
    /// The frozen neutral sink recorded at admission.
    pub sink: Id,
}

/// One generation of a §1-headed account: the persisted header plus the
/// kernel [`DonationLedger`] it mirrors, and whether the generation has
/// reached its once-only close.
///
/// The value is `Copy` like every kernel state; a refused transition leaves
/// the caller's copy exactly as it was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalAccountV1 {
    header: TerminalIdentityV1,
    ledger: DonationLedger,
    closed: bool,
}

impl TerminalAccountV1 {
    /// Admit generation 1 with the artifact-stage transfer/allocate/assign
    /// pattern: `payer_principal = rent_shortfall`, and any pre-existing
    /// balance seeds the donation floor via the kernel's `admit_prefunded`
    /// (a prefund is never credited to the payer; `payer == neutral_sink`
    /// refuses inside the kernel).
    pub fn create(
        payer: Id,
        neutral_sink: Id,
        balance_before: u64,
        rent_shortfall: u64,
        balance_after: u64,
    ) -> Result<Self, Error> {
        Self::admit(
            payer,
            neutral_sink,
            balance_before,
            rent_shortfall,
            balance_after,
            1,
        )
    }

    fn admit(
        payer: Id,
        neutral_sink: Id,
        balance_before: u64,
        rent_shortfall: u64,
        balance_after: u64,
        generation: u64,
    ) -> Result<Self, Error> {
        if rent_shortfall == 0 {
            return Err(Error::ZeroPayerPrincipal);
        }
        let ledger = DonationLedger::admit_prefunded(
            payer,
            neutral_sink,
            balance_before,
            rent_shortfall,
            balance_after,
        )?;
        let header = TerminalIdentityV1 {
            payer,
            payer_principal: rent_shortfall,
            donation_floor: ledger.donation_lamports(),
            generation,
        };
        header.validate(neutral_sink)?;
        Ok(Self {
            header,
            ledger,
            closed: false,
        })
    }

    /// The persisted header as of the last transition.
    pub const fn header(self) -> TerminalIdentityV1 {
        self.header
    }

    /// The mirrored kernel donation ledger.
    pub const fn ledger(self) -> DonationLedger {
        self.ledger
    }

    /// Whether this generation has reached its once-only close.
    pub const fn is_closed(self) -> bool {
        self.closed
    }

    /// Re-run the kernel's `observe` after a mutating transition: surplus
    /// accretes monotonically into the donation compartment and is never
    /// reclassified. `accounted_lamports` must cover the payer principal;
    /// a balance below `accounted + donation_floor` refuses in the kernel
    /// rather than clamping.
    pub fn observe_transition(
        mut self,
        actual_balance: u64,
        accounted_lamports: u64,
    ) -> Result<Self, Error> {
        if self.closed {
            return Err(Error::AlreadyClosed);
        }
        if accounted_lamports < self.header.payer_principal {
            return Err(Error::AccountedBelowPrincipal);
        }
        self.ledger = self.ledger.observe(actual_balance, accounted_lamports)?;
        self.header.donation_floor = self.ledger.donation_lamports();
        Ok(self)
    }

    /// Close once, ever, for this generation: pay exactly `payer_principal`
    /// to the stored payer and route the entire remaining surplus through
    /// the kernel's `terminal_split` to the neutral sink.
    ///
    /// `accounted_lamports` must equal the stored principal exactly —
    /// economic close strictly precedes rent close, so no other compartment
    /// may remain. A deficit refuses in the kernel; a second close refuses
    /// here.
    pub fn close(
        mut self,
        actual_balance: u64,
        accounted_lamports: u64,
    ) -> Result<(Self, CloseSplit), Error> {
        if self.closed {
            return Err(Error::AlreadyClosed);
        }
        if accounted_lamports != self.header.payer_principal {
            return Err(Error::TerminalAccountedMismatch);
        }
        let observed = self.ledger.observe(actual_balance, accounted_lamports)?;
        let disposition = observed.terminal_split(actual_balance, accounted_lamports)?;
        self.ledger = observed;
        self.header.donation_floor = observed.donation_lamports();
        self.closed = true;
        let split = CloseSplit {
            payer: self.header.payer,
            payer_principal: self.header.payer_principal,
            neutral_surplus: disposition.neutral_lamports,
            sink: disposition.neutral_sink,
        };
        Ok((self, split))
    }

    /// Admit the next generation at the same address after a completed
    /// close. The reopening payer may differ from the closed generation's
    /// payer; the frozen neutral sink carries over unchanged, and the
    /// generation advances by exactly one.
    pub fn reopen(
        self,
        payer: Id,
        balance_before: u64,
        rent_shortfall: u64,
        balance_after: u64,
    ) -> Result<Self, Error> {
        if !self.closed {
            return Err(Error::ReopenBeforeClose);
        }
        let generation = self
            .header
            .generation
            .checked_add(1)
            .ok_or(Error::GenerationOverflow)?;
        Self::admit(
            payer,
            self.ledger.neutral_sink(),
            balance_before,
            rent_shortfall,
            balance_after,
            generation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn sink() -> Id {
        id(250)
    }

    fn header(principal: u64, floor: u64, generation: u64) -> TerminalIdentityV1 {
        TerminalIdentityV1 {
            payer: id(1),
            payer_principal: principal,
            donation_floor: floor,
            generation,
        }
    }

    #[test]
    fn header_round_trips_exactly() {
        for value in [
            header(1, 0, 1),
            header(17, 3, 2),
            header(u64::MAX, u64::MAX, u64::MAX),
        ] {
            let bytes = value.encode(sink()).unwrap();
            assert_eq!(TerminalIdentityV1::decode(&bytes, sink()), Ok(value));
        }
    }

    #[test]
    fn header_layout_is_pinned_little_endian() {
        let value = TerminalIdentityV1 {
            payer: id(0x11),
            payer_principal: 0x0102_0304_0506_0708,
            donation_floor: 0x1112_1314_1516_1718,
            generation: 0x2122_2324_2526_2728,
        };
        let bytes = value.encode(sink()).unwrap();
        let mut expected = [0u8; HEADER_BYTES];
        expected[..32].copy_from_slice(&[0x11; 32]);
        expected[32..40].copy_from_slice(&[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
        expected[40..48].copy_from_slice(&[0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11]);
        expected[48..56].copy_from_slice(&[0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn truncated_and_trailing_bytes_refuse() {
        let bytes = header(17, 3, 1).encode(sink()).unwrap();
        for length in [0usize, 1, 32, 55] {
            assert_eq!(
                TerminalIdentityV1::decode(&bytes[..length], sink()),
                Err(Error::HeaderLength)
            );
        }
        let mut trailing = [0u8; HEADER_BYTES + 8];
        trailing[..HEADER_BYTES].copy_from_slice(&bytes);
        for length in [HEADER_BYTES + 1, HEADER_BYTES + 8] {
            assert_eq!(
                TerminalIdentityV1::decode(&trailing[..length], sink()),
                Err(Error::HeaderLength)
            );
        }
    }

    #[test]
    fn zero_payer_key_refuses() {
        let mut bytes = header(17, 3, 1).encode(sink()).unwrap();
        bytes[..32].copy_from_slice(&[0; 32]);
        assert_eq!(
            TerminalIdentityV1::decode(&bytes, sink()),
            Err(Error::ZeroPayer)
        );
        let hostile = TerminalIdentityV1 {
            payer: Id::ZERO,
            ..header(17, 3, 1)
        };
        assert_eq!(hostile.encode(sink()), Err(Error::ZeroPayer));
    }

    #[test]
    fn zero_principal_refuses() {
        let bytes = header(17, 3, 1).encode(sink()).unwrap();
        let mut hostile = bytes;
        hostile[32..40].copy_from_slice(&[0; 8]);
        assert_eq!(
            TerminalIdentityV1::decode(&hostile, sink()),
            Err(Error::ZeroPayerPrincipal)
        );
        assert_eq!(
            header(0, 3, 1).encode(sink()),
            Err(Error::ZeroPayerPrincipal)
        );
    }

    #[test]
    fn payer_equal_to_neutral_sink_refuses_and_sink_must_be_live() {
        let bytes = header(17, 3, 1).encode(sink()).unwrap();
        assert_eq!(
            TerminalIdentityV1::decode(&bytes, id(1)),
            Err(Error::PayerIsNeutralSink)
        );
        assert_eq!(
            header(17, 3, 1).encode(id(1)),
            Err(Error::PayerIsNeutralSink)
        );
        assert_eq!(
            TerminalIdentityV1::decode(&bytes, Id::ZERO),
            Err(Error::ZeroNeutralSink)
        );
    }

    #[test]
    fn zero_generation_refuses() {
        let bytes = header(17, 3, 1).encode(sink()).unwrap();
        let mut hostile = bytes;
        hostile[48..56].copy_from_slice(&[0; 8]);
        assert_eq!(
            TerminalIdentityV1::decode(&hostile, sink()),
            Err(Error::ZeroGeneration)
        );
        assert_eq!(header(17, 3, 0).encode(sink()), Err(Error::ZeroGeneration));
    }

    #[test]
    fn create_seeds_prefund_as_donation_floor() {
        let account = TerminalAccountV1::create(id(1), sink(), 5, 17, 22).unwrap();
        assert_eq!(account.header(), header(17, 5, 1));
        assert_eq!(account.ledger().donation_lamports(), 5);
        assert_eq!(account.ledger().neutral_sink(), sink());
        assert!(!account.is_closed());
    }

    #[test]
    fn create_refusals_delegate_to_the_kernel() {
        assert_eq!(
            TerminalAccountV1::create(Id::ZERO, sink(), 0, 17, 17),
            Err(Error::Liveness(LivenessError::ZeroIdentity))
        );
        assert_eq!(
            TerminalAccountV1::create(sink(), sink(), 0, 17, 17),
            Err(Error::Liveness(LivenessError::SameOwnerAndNeutralSink))
        );
        assert_eq!(
            TerminalAccountV1::create(id(1), sink(), 5, 17, 21),
            Err(Error::Liveness(LivenessError::FundingDeltaMismatch))
        );
        assert_eq!(
            TerminalAccountV1::create(id(1), sink(), u64::MAX, 1, 0),
            Err(Error::Liveness(LivenessError::ArithmeticOverflow))
        );
        assert_eq!(
            TerminalAccountV1::create(id(1), sink(), 0, 0, 0),
            Err(Error::ZeroPayerPrincipal)
        );
    }

    #[test]
    fn observe_transition_updates_floor_and_requires_principal_accounting() {
        let account = TerminalAccountV1::create(id(1), sink(), 5, 17, 22).unwrap();
        let grown = account.observe_transition(30, 17).unwrap();
        assert_eq!(grown.header().donation_floor, 13);
        assert_eq!(grown.header().payer_principal, 17);
        assert_eq!(
            account.observe_transition(30, 16),
            Err(Error::AccountedBelowPrincipal)
        );
        let (closed, _) = grown.close(30, 17).unwrap();
        assert_eq!(closed.observe_transition(30, 17), Err(Error::AlreadyClosed));
    }

    #[test]
    fn close_pays_exact_principal_and_burns_surplus_once() {
        let account = TerminalAccountV1::create(id(1), sink(), 5, 17, 22).unwrap();
        assert_eq!(account.close(22, 16), Err(Error::TerminalAccountedMismatch));
        assert_eq!(account.close(22, 18), Err(Error::TerminalAccountedMismatch));
        let (closed, split) = account.close(22, 17).unwrap();
        assert_eq!(
            split,
            CloseSplit {
                payer: id(1),
                payer_principal: 17,
                neutral_surplus: 5,
                sink: sink(),
            }
        );
        assert!(closed.is_closed());
        assert_eq!(closed.close(22, 17), Err(Error::AlreadyClosed));
    }

    #[test]
    fn reopen_advances_generation_and_requires_closed() {
        let account = TerminalAccountV1::create(id(1), sink(), 0, 17, 17).unwrap();
        assert_eq!(
            account.reopen(id(2), 0, 9, 9),
            Err(Error::ReopenBeforeClose)
        );
        let (closed, _) = account.close(17, 17).unwrap();
        let reopened = closed.reopen(id(2), 3, 9, 12).unwrap();
        assert_eq!(
            reopened.header(),
            TerminalIdentityV1 {
                payer: id(2),
                payer_principal: 9,
                donation_floor: 3,
                generation: 2,
            }
        );
        assert!(!reopened.is_closed());
        assert_eq!(
            closed.reopen(sink(), 0, 9, 9),
            Err(Error::Liveness(LivenessError::SameOwnerAndNeutralSink))
        );
        let last_era = TerminalAccountV1::admit(id(1), sink(), 0, 17, 17, u64::MAX).unwrap();
        let (last_era, _) = last_era.close(17, 17).unwrap();
        assert_eq!(
            last_era.reopen(id(2), 0, 9, 9),
            Err(Error::GenerationOverflow)
        );
    }
}
