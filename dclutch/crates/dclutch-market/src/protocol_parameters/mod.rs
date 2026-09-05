//! The governed protocol-parameters record.
//!
//! One record holding every economic knob the protocol has, so that changing
//! one is a governed act with a delay and a receipt rather than a source
//! constant riding whichever ELF happens to carry it.
//!
//! Ruled 2026-09-04 (C-11 D1, as amended). The Lean twin
//! `DClutchSemantics/ProtocolParametersV1.lean` owns the layout, the bands and
//! the change procedure; `src/generated.rs` is emitted from it and this module
//! is the executable form of the same statements.
//!
//! # What governance can and cannot do
//!
//! The bands are the constitution and the record is the statute. A band is a
//! source constant emitted from Lean: moving it needs a new ELF and a release.
//! A parameter is a record field: moving it needs the authority, a proposal and
//! the delay.
//!
//! - **The fee ceiling only narrows.** [`PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1`]
//!   is decision 0014 D2's 500 -- the number that is today the enforced band
//!   and becomes here the BOUND ON the enforced band. Governance may set the
//!   effective cap anywhere at or below it and can never widen past what the
//!   deployed release already allows, so a holder who read the ELF knows the
//!   worst case without reading the record.
//! - **A take and a payee move together.** `protocol_take_basis_points` is zero
//!   exactly when `protocol_beneficiary` is the zero key, in both directions.
//!   Ruling D1's *"no protocol fee take before mainnet; no protocol
//!   beneficiary"* is therefore one fact and not two that could drift apart,
//!   and a take with no payee is unrepresentable rather than merely absent.
//! - **The notice period has a floor.** Governance sets `change_delay_slots` at
//!   or above [`PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1`] and can never set it
//!   below, so it cannot make itself instantaneous by governing its own delay
//!   first.
//! - **The freeze is one-way.** A zero `governance_authority` is legal and
//!   means NOBODY MAY PROPOSE. Nothing in this module writes an authority, so a
//!   deployment that wants immutable economics sets it to zero and the record
//!   is finished forever. A reversible freeze is not a freeze.
//!
//! # What it does not do
//!
//! This is the record and its procedure. It does not read a signer, a clock or
//! an account: `signer_is_authority`, `digest_matches` and `current_slot` are
//! the adapter's answers, arriving as arguments, exactly as
//! `dclutch-custody` takes the caller's compartment claim rather than
//! deriving it. Which key an authority is, is an authentication question a
//! record cannot be wrong about.
//!
//! **And no program creates or writes one yet.** Said plainly here because the
//! alternative is a reader finding out: this crate is a decoder, an encoder, a
//! band predicate and a procedure, with hostiles that exercise all four, and
//! every one of them runs against records built in a test. The consumers that
//! would read it -- the Direct fee band, the closer carve ceiling, the crank
//! reward cap -- currently project [`ProtocolParametersV1::genesis`]'s values as
//! constants, which makes the record their single AUTHOR without making it
//! their runtime SOURCE. What is owed is a PDA at
//! [`PROTOCOL_PARAMETERS_PDA_DOMAIN_V1`], a dispatcher owning the refusal
//! sub-band, and each consumer taking the value out of its frame. Until that
//! lands, nothing here has ever been exercised by a caller that was not a test.

// `rustfmt::skip` because the file is EMITTED and `check-generated.sh` diffs it
// byte for byte against a fresh emission. rustfmt follows `mod` declarations, so
// formatting this crate's root rewraps two long constants and the parity gate
// then reports a divergence that is the formatter's, not the emitter's --
// measured 2026-09-04, one line apart, on the first run of the gate.
#[rustfmt::skip]
#[allow(missing_docs)]
mod generated;

use dclutch_sha256_adapter::digest;

pub use generated::{
    PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1, PROTOCOL_BASIS_POINT_DENOMINATOR_V1,
    PROTOCOL_GENESIS_CLOSER_CARVE_BASIS_POINTS_V1, PROTOCOL_GENESIS_CLOSER_REWARD_CAP_LAMPORTS_V1,
    PROTOCOL_GENESIS_CRANK_REWARD_CAP_LAMPORTS_V1, PROTOCOL_GENESIS_MAX_FEE_BASIS_POINTS_V1,
    PROTOCOL_GENESIS_TAKE_BASIS_POINTS_V1, PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1,
    PROTOCOL_PARAMETERS_ABI_VERSION_V1, PROTOCOL_PARAMETERS_PDA_DOMAIN_V1,
    PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1, PROTOCOL_PARAMETERS_RECEIPT_MAGIC_V1,
    PROTOCOL_PARAMETERS_RECORD_BYTES_V1, PROTOCOL_PARAMETERS_RECORD_MAGIC_V1,
    PROTOCOL_SLOTS_PER_NOMINAL_DAY_V1,
};

use generated::{
    PROTOCOL_PARAMETERS_RECEIPT_ACTIVATION_SLOT_OFFSET,
    PROTOCOL_PARAMETERS_RECEIPT_DELAY_SLOTS_OFFSET, PROTOCOL_PARAMETERS_RECEIPT_GENERATION_OFFSET,
    PROTOCOL_PARAMETERS_RECEIPT_NEW_DIGEST_OFFSET,
    PROTOCOL_PARAMETERS_RECEIPT_PREVIOUS_DIGEST_OFFSET,
    PROTOCOL_PARAMETERS_RECEIPT_PROPOSED_AT_SLOT_OFFSET,
    PROTOCOL_PARAMETERS_RECEIPT_RESERVED_HEADER_OFFSET, PROTOCOL_PARAMETERS_RECEIPT_VERSION_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_ACTIVATION_SLOT_OFFSET, PROTOCOL_PARAMETERS_RECORD_BUMP_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_CHANGE_DELAY_SLOTS_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_CLOSER_CARVE_BASIS_POINTS_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_CLOSER_REWARD_CAP_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_CRANK_REWARD_CAP_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_GENERATION_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_GOVERNANCE_AUTHORITY_OFFSET, PROTOCOL_PARAMETERS_RECORD_KIND_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_PENDING_DIGEST_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_PENDING_EARLIEST_APPLY_SLOT_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_PROTOCOL_BENEFICIARY_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_RESERVED_HEADER_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_RESERVED_TAIL_OFFSET,
    PROTOCOL_PARAMETERS_RECORD_TAKE_BASIS_POINTS_OFFSET, PROTOCOL_PARAMETERS_RECORD_VERSION_OFFSET,
};

/// Discriminates this record from every other fixed-layout record at a PDA.
pub const PROTOCOL_PARAMETERS_RECORD_KIND_V1: u8 = 1;

const RESERVED_HEADER_BYTES: usize = 4;
const RESERVED_TAIL_BYTES: usize = 26;
const RECEIPT_RESERVED_HEADER_BYTES: usize = 6;

/// Every refusal this contract can raise.
///
/// Contract refusals, not chain codes: no `#[repr(u32)]`, no band. The
/// dispatcher that reaches this record owns the protocol-visible taxonomy and
/// maps these into its own registered sub-band, exactly as `dclutch-claims-sbf`
/// maps `dclutch_custody::Error` today.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Bytes were not exactly [`PROTOCOL_PARAMETERS_RECORD_BYTES_V1`] wide.
    InvalidLength,
    /// Magic, version or kind selected another record family.
    InvalidHeader,
    /// A reserved span carried a nonzero byte.
    NonCanonical,
    /// The authority is the zero key: this record is frozen forever.
    GovernanceFrozen,
    /// The signer is not this record's governance authority.
    UnauthorizedGovernance,
    /// A proposal already stands; withdraw it or wait it out.
    ProposalOutstanding,
    /// The proposed value is outside a constitutional band.
    ParameterOutOfBand,
    /// Nothing has been proposed.
    NoPendingProposal,
    /// The change delay has not elapsed.
    ProposalNotMatured,
    /// The bytes offered are not the bytes the proposal pinned.
    ProposalDigestMismatch,
    /// A slot or generation sum did not fit `u64`.
    ArithmeticOverflow,
}

/// Result alias for this contract.
pub type Result<T> = core::result::Result<T, Error>;

/// The five governed values plus the bookkeeping that carries them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolParametersV1 {
    /// The one key that may propose. Zero means frozen forever.
    pub governance_authority: [u8; 32],
    /// Where a protocol take would go. Zero means there is no protocol
    /// beneficiary, which is what ruling D1 says today.
    pub protocol_beneficiary: [u8; 32],
    /// Bumps by exactly one on every applied change; the census's clock.
    pub generation: u64,
    /// The slot at which THIS value became effective.
    pub activation_slot: u64,
    /// Proposal-to-apply wait, at or above the constitutional floor.
    pub change_delay_slots: u64,
    /// Lamport ceiling on a permissionless closer's carve.
    pub closer_reward_cap_lamports: u64,
    /// Lamport ceiling on one compaction crank's reward.
    pub crank_reward_cap_lamports: u64,
    /// The effective fee band a market's rate is checked against.
    pub max_fee_basis_points: u16,
    /// The protocol's own cut of a fill. Zero, by ruling, until mainnet.
    pub protocol_take_basis_points: u16,
    /// Share of a close's donation slice the closer may carve, before the cap.
    pub closer_carve_basis_points: u16,
}

impl ProtocolParametersV1 {
    /// Today's deployed economics, exactly, as the record would hold them.
    ///
    /// The authority is a caller-supplied placeholder because a genesis with a
    /// baked-in authority would put one key in the ELF; the beneficiary is the
    /// zero key because ruling D1 says there is no protocol beneficiary, and
    /// the pair rule then forces the take to zero.
    #[must_use]
    pub const fn genesis(governance_authority: [u8; 32]) -> Self {
        Self {
            governance_authority,
            protocol_beneficiary: [0; 32],
            generation: 0,
            activation_slot: 0,
            change_delay_slots: PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1,
            closer_reward_cap_lamports: PROTOCOL_GENESIS_CLOSER_REWARD_CAP_LAMPORTS_V1,
            crank_reward_cap_lamports: PROTOCOL_GENESIS_CRANK_REWARD_CAP_LAMPORTS_V1,
            max_fee_basis_points: PROTOCOL_GENESIS_MAX_FEE_BASIS_POINTS_V1,
            protocol_take_basis_points: PROTOCOL_GENESIS_TAKE_BASIS_POINTS_V1,
            closer_carve_basis_points: PROTOCOL_GENESIS_CLOSER_CARVE_BASIS_POINTS_V1,
        }
    }

    /// Every band, as one predicate. Lean twin: `inBand`.
    ///
    /// A `bool` and not a `Result` on purpose: the caller that needs a reason
    /// gets [`Error::ParameterOutOfBand`] from the act it attempted, and a
    /// second, finer taxonomy here would be a second author for the same rule.
    #[must_use]
    pub const fn in_band(self) -> bool {
        self.max_fee_basis_points <= PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1
            && self.protocol_take_basis_points <= self.max_fee_basis_points
            && (self.protocol_take_basis_points == 0) == is_zero(&self.protocol_beneficiary)
            && self.closer_carve_basis_points <= PROTOCOL_BASIS_POINT_DENOMINATOR_V1
            && self.change_delay_slots >= PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1
    }

    /// The carve one close pays its permissionless closer.
    ///
    /// A share of the DONATION SLICE and never of the principal, then capped in
    /// lamports. Both bounds are needed and they bound different things: the
    /// share keeps the recorded beneficiary's fraction whatever the donation
    /// is, and the cap keeps a single enormous donation from paying an enormous
    /// reward for one ordinary crank. Rounds DOWN, toward the beneficiary,
    /// which is the same direction the Direct fee rounds.
    ///
    /// It cannot refuse. A crank that could refuse for lack of funds is an
    /// unturned crank, which is the sleeping-holder deadlock coming back
    /// through the funding door (`docs/design/FUNDED_CRANK_V1.md` section 2).
    #[must_use]
    pub const fn closer_carve(self, donation_lamports: u64) -> u64 {
        let scaled = (donation_lamports as u128) * (self.closer_carve_basis_points as u128)
            / (PROTOCOL_BASIS_POINT_DENOMINATOR_V1 as u128);
        // The share is at most the whole donation, so the cast is exact.
        #[allow(clippy::cast_possible_truncation)]
        let share = scaled as u64;
        if share < self.closer_reward_cap_lamports {
            share
        } else {
            self.closer_reward_cap_lamports
        }
    }
}

/// A commitment to a proposed value, or none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingChangeV1 {
    /// SHA-256 of the proposed record body. Zero means no proposal stands.
    pub digest: [u8; 32],
    /// The first slot at which a standing proposal may be applied.
    pub earliest_apply_slot: u64,
}

impl PendingChangeV1 {
    /// No proposal stands.
    pub const NONE: Self = Self {
        digest: [0; 32],
        earliest_apply_slot: 0,
    };

    /// Whether a proposal stands.
    #[must_use]
    pub const fn is_standing(self) -> bool {
        !is_zero(&self.digest)
    }
}

/// The record as it sits at its PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolParametersRecordV1 {
    /// PDA bump, so the record authenticates its own address.
    pub bump: u8,
    /// The governed values.
    pub parameters: ProtocolParametersV1,
    /// The standing proposal, if any.
    pub pending: PendingChangeV1,
}

/// What a census reads: one applied change, with its notice period checkable
/// from the receipt alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolParametersChangeReceiptV1 {
    /// SHA-256 of the record body before the change.
    pub previous_digest: [u8; 32],
    /// SHA-256 of the record body after it.
    pub new_digest: [u8; 32],
    /// The generation this change produced.
    pub generation: u64,
    /// The slot the proposal was made, back-derived from the maturity and the
    /// delay, so a reader never has to trust a separately recorded number.
    pub proposed_at_slot: u64,
    /// The slot the change became effective.
    pub activation_slot: u64,
    /// The delay the change actually served.
    pub delay_slots: u64,
}

impl ProtocolParametersRecordV1 {
    /// Stage a change. The authority's act.
    ///
    /// `signer_is_authority` is the adapter's answer after comparing a signer
    /// against `parameters.governance_authority`; this contract never reads an
    /// account.
    pub fn propose(
        self,
        signer_is_authority: bool,
        proposed: ProtocolParametersV1,
        current_slot: u64,
    ) -> Result<Self> {
        if is_zero(&self.parameters.governance_authority) {
            return Err(Error::GovernanceFrozen);
        }
        if !signer_is_authority {
            return Err(Error::UnauthorizedGovernance);
        }
        if self.pending.is_standing() {
            return Err(Error::ProposalOutstanding);
        }
        if !proposed.in_band() {
            return Err(Error::ParameterOutOfBand);
        }
        let earliest_apply_slot = current_slot
            .checked_add(self.parameters.change_delay_slots)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Self {
            pending: PendingChangeV1 {
                digest: proposed.body_digest(),
                earliest_apply_slot,
            },
            ..self
        })
    }

    /// Take a standing proposal back. The authority's act, and nobody else's.
    pub fn withdraw(self, signer_is_authority: bool) -> Result<Self> {
        if is_zero(&self.parameters.governance_authority) {
            return Err(Error::GovernanceFrozen);
        }
        if !signer_is_authority {
            return Err(Error::UnauthorizedGovernance);
        }
        if !self.pending.is_standing() {
            return Err(Error::NoPendingProposal);
        }
        Ok(Self {
            pending: PendingChangeV1::NONE,
            ..self
        })
    }

    /// Install a matured proposal. Anybody's act.
    ///
    /// Permissionless because a governed change is still a crank: an authority
    /// that had to show up twice could propose a change and then decline to
    /// finish it, leaving the record in a state only it can leave.
    ///
    /// The band is re-checked HERE and not only at proposal, because a release
    /// landing between the two acts may have narrowed the constitution under a
    /// proposal that was legal when it was made. A proposal is a commitment to
    /// a value, never a grant of permission to install it.
    pub fn apply_change(
        self,
        proposed: ProtocolParametersV1,
        current_slot: u64,
    ) -> Result<(Self, ProtocolParametersChangeReceiptV1)> {
        if !self.pending.is_standing() {
            return Err(Error::NoPendingProposal);
        }
        if current_slot < self.pending.earliest_apply_slot {
            return Err(Error::ProposalNotMatured);
        }
        if proposed.body_digest() != self.pending.digest {
            return Err(Error::ProposalDigestMismatch);
        }
        if !proposed.in_band() {
            return Err(Error::ParameterOutOfBand);
        }
        let generation = self
            .parameters
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        // The proposal's own delay is the one it was staged under, which is the
        // OLD record's -- a change that shortens the delay serves the old one.
        let delay_slots = self.parameters.change_delay_slots;
        let proposed_at_slot = self
            .pending
            .earliest_apply_slot
            .checked_sub(delay_slots)
            .ok_or(Error::ArithmeticOverflow)?;
        let after = Self {
            bump: self.bump,
            parameters: ProtocolParametersV1 {
                generation,
                activation_slot: current_slot,
                ..proposed
            },
            pending: PendingChangeV1::NONE,
        };
        Ok((
            after,
            ProtocolParametersChangeReceiptV1 {
                previous_digest: self.parameters.body_digest(),
                new_digest: after.parameters.body_digest(),
                generation,
                proposed_at_slot,
                activation_slot: current_slot,
                delay_slots,
            },
        ))
    }

    /// Hostile-decode one exact record.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != PROTOCOL_PARAMETERS_RECORD_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(PROTOCOL_PARAMETERS_RECORD_MAGIC_V1.as_slice())
            || u16_at(input, PROTOCOL_PARAMETERS_RECORD_VERSION_OFFSET)
                != PROTOCOL_PARAMETERS_ABI_VERSION_V1
            || input.get(PROTOCOL_PARAMETERS_RECORD_KIND_OFFSET).copied()
                != Some(PROTOCOL_PARAMETERS_RECORD_KIND_V1)
        {
            return Err(Error::InvalidHeader);
        }
        for span in [
            input
                .get(
                    PROTOCOL_PARAMETERS_RECORD_RESERVED_HEADER_OFFSET
                        ..PROTOCOL_PARAMETERS_RECORD_RESERVED_HEADER_OFFSET + RESERVED_HEADER_BYTES,
                )
                .ok_or(Error::InvalidLength)?,
            input
                .get(
                    PROTOCOL_PARAMETERS_RECORD_RESERVED_TAIL_OFFSET
                        ..PROTOCOL_PARAMETERS_RECORD_RESERVED_TAIL_OFFSET + RESERVED_TAIL_BYTES,
                )
                .ok_or(Error::InvalidLength)?,
        ] {
            if span.iter().any(|byte| *byte != 0) {
                return Err(Error::NonCanonical);
            }
        }
        let value = Self {
            bump: *input
                .get(PROTOCOL_PARAMETERS_RECORD_BUMP_OFFSET)
                .ok_or(Error::InvalidLength)?,
            parameters: ProtocolParametersV1 {
                governance_authority: array_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_GOVERNANCE_AUTHORITY_OFFSET,
                ),
                protocol_beneficiary: array_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_PROTOCOL_BENEFICIARY_OFFSET,
                ),
                generation: u64_at(input, PROTOCOL_PARAMETERS_RECORD_GENERATION_OFFSET),
                activation_slot: u64_at(input, PROTOCOL_PARAMETERS_RECORD_ACTIVATION_SLOT_OFFSET),
                change_delay_slots: u64_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_CHANGE_DELAY_SLOTS_OFFSET,
                ),
                closer_reward_cap_lamports: u64_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_CLOSER_REWARD_CAP_OFFSET,
                ),
                crank_reward_cap_lamports: u64_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_CRANK_REWARD_CAP_OFFSET,
                ),
                max_fee_basis_points: u16_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET,
                ),
                protocol_take_basis_points: u16_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_TAKE_BASIS_POINTS_OFFSET,
                ),
                closer_carve_basis_points: u16_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_CLOSER_CARVE_BASIS_POINTS_OFFSET,
                ),
            },
            pending: PendingChangeV1 {
                digest: array_at(input, PROTOCOL_PARAMETERS_RECORD_PENDING_DIGEST_OFFSET),
                earliest_apply_slot: u64_at(
                    input,
                    PROTOCOL_PARAMETERS_RECORD_PENDING_EARLIEST_APPLY_SLOT_OFFSET,
                ),
            },
        };
        // A persisted record that is out of band is not a record this protocol
        // wrote: every writer above checks the bands, so the only way to see
        // one is corruption or a foreign author. Refusing on the READ means a
        // consumer never has to ask.
        if !value.parameters.in_band() {
            return Err(Error::ParameterOutOfBand);
        }
        Ok(value)
    }

    /// Encode one canonical record.
    #[must_use]
    pub fn to_bytes(self) -> [u8; PROTOCOL_PARAMETERS_RECORD_BYTES_V1] {
        let mut output = [0_u8; PROTOCOL_PARAMETERS_RECORD_BYTES_V1];
        output[..8].copy_from_slice(&PROTOCOL_PARAMETERS_RECORD_MAGIC_V1);
        put_u16(
            &mut output,
            PROTOCOL_PARAMETERS_RECORD_VERSION_OFFSET,
            PROTOCOL_PARAMETERS_ABI_VERSION_V1,
        );
        output[PROTOCOL_PARAMETERS_RECORD_KIND_OFFSET] = PROTOCOL_PARAMETERS_RECORD_KIND_V1;
        output[PROTOCOL_PARAMETERS_RECORD_BUMP_OFFSET] = self.bump;
        put_array(
            &mut output,
            PROTOCOL_PARAMETERS_RECORD_GOVERNANCE_AUTHORITY_OFFSET,
            &self.parameters.governance_authority,
        );
        put_array(
            &mut output,
            PROTOCOL_PARAMETERS_RECORD_PROTOCOL_BENEFICIARY_OFFSET,
            &self.parameters.protocol_beneficiary,
        );
        put_array(
            &mut output,
            PROTOCOL_PARAMETERS_RECORD_PENDING_DIGEST_OFFSET,
            &self.pending.digest,
        );
        for (offset, value) in [
            (
                PROTOCOL_PARAMETERS_RECORD_GENERATION_OFFSET,
                self.parameters.generation,
            ),
            (
                PROTOCOL_PARAMETERS_RECORD_ACTIVATION_SLOT_OFFSET,
                self.parameters.activation_slot,
            ),
            (
                PROTOCOL_PARAMETERS_RECORD_CHANGE_DELAY_SLOTS_OFFSET,
                self.parameters.change_delay_slots,
            ),
            (
                PROTOCOL_PARAMETERS_RECORD_PENDING_EARLIEST_APPLY_SLOT_OFFSET,
                self.pending.earliest_apply_slot,
            ),
            (
                PROTOCOL_PARAMETERS_RECORD_CLOSER_REWARD_CAP_OFFSET,
                self.parameters.closer_reward_cap_lamports,
            ),
            (
                PROTOCOL_PARAMETERS_RECORD_CRANK_REWARD_CAP_OFFSET,
                self.parameters.crank_reward_cap_lamports,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        for (offset, value) in [
            (
                PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET,
                self.parameters.max_fee_basis_points,
            ),
            (
                PROTOCOL_PARAMETERS_RECORD_TAKE_BASIS_POINTS_OFFSET,
                self.parameters.protocol_take_basis_points,
            ),
            (
                PROTOCOL_PARAMETERS_RECORD_CLOSER_CARVE_BASIS_POINTS_OFFSET,
                self.parameters.closer_carve_basis_points,
            ),
        ] {
            put_u16(&mut output, offset, value);
        }
        output
    }
}

impl ProtocolParametersV1 {
    /// SHA-256 over the ten governed values, in wire order.
    ///
    /// The BODY, not the record: `generation`, `activation_slot` and the
    /// pending commitment are bookkeeping the apply writes, so hashing them
    /// would make a proposal's digest depend on when it was made and no
    /// proposal could ever be applied.
    #[must_use]
    pub fn body_digest(self) -> [u8; 32] {
        let mut preimage = [0_u8; 88];
        preimage[..32].copy_from_slice(&self.governance_authority);
        preimage[32..64].copy_from_slice(&self.protocol_beneficiary);
        preimage[64..72].copy_from_slice(&self.change_delay_slots.to_le_bytes());
        preimage[72..80].copy_from_slice(&self.closer_reward_cap_lamports.to_le_bytes());
        preimage[80..88].copy_from_slice(&self.crank_reward_cap_lamports.to_le_bytes());
        let mut tail = [0_u8; 6];
        tail[..2].copy_from_slice(&self.max_fee_basis_points.to_le_bytes());
        tail[2..4].copy_from_slice(&self.protocol_take_basis_points.to_le_bytes());
        tail[4..6].copy_from_slice(&self.closer_carve_basis_points.to_le_bytes());
        let mut joined = [0_u8; 94];
        joined[..88].copy_from_slice(&preimage);
        joined[88..].copy_from_slice(&tail);
        digest(&joined)
    }
}

impl ProtocolParametersChangeReceiptV1 {
    /// Encode one canonical receipt.
    #[must_use]
    pub fn to_bytes(self) -> [u8; PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1] {
        let mut output = [0_u8; PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1];
        output[..8].copy_from_slice(&PROTOCOL_PARAMETERS_RECEIPT_MAGIC_V1);
        put_u16(
            &mut output,
            PROTOCOL_PARAMETERS_RECEIPT_VERSION_OFFSET,
            PROTOCOL_PARAMETERS_ABI_VERSION_V1,
        );
        put_array(
            &mut output,
            PROTOCOL_PARAMETERS_RECEIPT_PREVIOUS_DIGEST_OFFSET,
            &self.previous_digest,
        );
        put_array(
            &mut output,
            PROTOCOL_PARAMETERS_RECEIPT_NEW_DIGEST_OFFSET,
            &self.new_digest,
        );
        for (offset, value) in [
            (
                PROTOCOL_PARAMETERS_RECEIPT_GENERATION_OFFSET,
                self.generation,
            ),
            (
                PROTOCOL_PARAMETERS_RECEIPT_PROPOSED_AT_SLOT_OFFSET,
                self.proposed_at_slot,
            ),
            (
                PROTOCOL_PARAMETERS_RECEIPT_ACTIVATION_SLOT_OFFSET,
                self.activation_slot,
            ),
            (
                PROTOCOL_PARAMETERS_RECEIPT_DELAY_SLOTS_OFFSET,
                self.delay_slots,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        output
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(PROTOCOL_PARAMETERS_RECEIPT_MAGIC_V1.as_slice())
            || u16_at(input, PROTOCOL_PARAMETERS_RECEIPT_VERSION_OFFSET)
                != PROTOCOL_PARAMETERS_ABI_VERSION_V1
        {
            return Err(Error::InvalidHeader);
        }
        if input
            .get(
                PROTOCOL_PARAMETERS_RECEIPT_RESERVED_HEADER_OFFSET
                    ..PROTOCOL_PARAMETERS_RECEIPT_RESERVED_HEADER_OFFSET
                        + RECEIPT_RESERVED_HEADER_BYTES,
            )
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonical);
        }
        let value = Self {
            previous_digest: array_at(input, PROTOCOL_PARAMETERS_RECEIPT_PREVIOUS_DIGEST_OFFSET),
            new_digest: array_at(input, PROTOCOL_PARAMETERS_RECEIPT_NEW_DIGEST_OFFSET),
            generation: u64_at(input, PROTOCOL_PARAMETERS_RECEIPT_GENERATION_OFFSET),
            proposed_at_slot: u64_at(input, PROTOCOL_PARAMETERS_RECEIPT_PROPOSED_AT_SLOT_OFFSET),
            activation_slot: u64_at(input, PROTOCOL_PARAMETERS_RECEIPT_ACTIVATION_SLOT_OFFSET),
            delay_slots: u64_at(input, PROTOCOL_PARAMETERS_RECEIPT_DELAY_SLOTS_OFFSET),
        };
        // The notice period, checkable from the receipt alone. A receipt whose
        // own three slot numbers do not close is not evidence of a governed
        // change; it is evidence of something else.
        if value.previous_digest == value.new_digest
            || value.generation == 0
            || value.delay_slots < PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1
            || value
                .proposed_at_slot
                .checked_add(value.delay_slots)
                .is_none_or(|matured| matured > value.activation_slot)
        {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }
}

// A `const fn` over an exact array with a bounded index: the same width fact as
// the helpers below, stated once.
#[allow(clippy::indexing_slicing)]
const fn is_zero(value: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < 32 {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

// THE SIX HELPERS BELOW INDEX BY A CONSTANT OFFSET INTO AN EXACT-WIDTH RECORD.
// Every caller is a decode that has already refused any other width, or an
// encode into an array of exactly that width, and every offset is a `const`
// of this module's own layout. A fallible read here would be a second width
// check with a refusal nothing can reach; the allow is narrower than that.
#[allow(clippy::indexing_slicing)]
fn u16_at(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

#[allow(clippy::indexing_slicing)]
fn u64_at(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

#[allow(clippy::indexing_slicing)]
fn array_at(input: &[u8], offset: usize) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&input[offset..offset + 32]);
    bytes
}

#[allow(clippy::indexing_slicing)]
fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[allow(clippy::indexing_slicing)]
fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[allow(clippy::indexing_slicing)]
fn put_array(output: &mut [u8], offset: usize, value: &[u8; 32]) {
    output[offset..offset + 32].copy_from_slice(value);
}

#[cfg(test)]
mod tests;
