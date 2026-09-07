//! The upkeep vault: one protocol-owned lamport PDA with no authority.
//!
//! Decision 0024 item 4 charters it exactly as `docs/design/UPKEEP_VAULT_V0.md`
//! sketches it and no wider; the Lean twin `DClutchSemantics/UpkeepVaultV1.lean`
//! owns the layout, the closed source-class enum, the two operations and the
//! one transition, and `generated_upkeep_vault_v1.rs` is emitted from it.
//!
//! # The three invariants, as this module holds them
//!
//! - **I1, no involuntary inflow.** Every credit names its source from
//!   [`UpkeepSourceClassV1`], which has four members and no member a trade
//!   fee, a rent principal or a recorded receivable could be presented as.
//!   `deposit` is a signer's own lamports; the other three are a protocol
//!   route's word for lamports it moved out of an account it owns, and the
//!   adapter admits them only from the release set's caller-authority PDA.
//! - **I2, no discretionary outflow.** [`UpkeepOperationV1`] has `Found` and
//!   `Credit`. The tag a spend would have had is reserved and refused BY NAME
//!   ([`Error::NoSpendRoute`]), so the hostile that tries to spend is told
//!   which charter stopped it rather than "unknown byte". `outflow_total` is a
//!   running number nothing here moves; the census holds it at zero.
//! - **I3, full legibility.** [`UpkeepVaultV1::receipted`] is
//!   `inflow_total - outflow_total`, the lamports the record can account for
//!   above the account's own rent minimum. Lamports that reach the address
//!   without a credit -- anyone may transfer into any account -- are
//!   [`UpkeepVaultV1::unreceipted`], a class the census holds to a declared
//!   delta and never a place to hide.
//!
//! # The four classes, and which of them has a producer
//!
//! The charter names four sources; the wire admits four; two of them can
//! actually be produced today, and this list is here so the other two are a
//! STATED gap rather than a zero a reader would take for a measurement.
//!
//! - **`Deposit` — producible.** Anyone's own lamports, moved by the System
//!   transfer inside the vault's own deposit route.
//! - **`Donation` — producible, and produced.** The Direct close-maker credits
//!   the donation remainder after the closer's carve
//!   (`programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs`). On every
//!   cohort to date that slice measured ZERO, so the class the vault will
//!   actually see money under first is not this one.
//! - **`SeatRent` — no producer.** The certificate seat is prepaid by an
//!   off-chain driver's System transfer and the account it funds ends up owned
//!   by the Resolution program, which has no route that closes a seat. The
//!   2,786,520 lamports cohort-14 measured
//!   (`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:771`) are the
//!   largest real number in this charter and the vault cannot yet be told about
//!   them: the producer is a Resolution seat-close route that does not exist.
//! - **`Residue` — no producer, and one candidate already argued against.**
//!   The escrow-close residue after `opener_outlay` is serviced reaches the
//!   market's RentCredit today by a landed argument
//!   (`crates/dclutch-claims/src/claim_check_conservation_v1.rs`), and
//!   compaction dust stays escrowed against a claim that can still be formed
//!   (`fractional_claim_check_v1.rs`). Routing either here is a reversal of an
//!   argument, not a missing `match` arm, and it needs its own decision.
//!
//! # What it does not do
//!
//! It reads no account, no signer and no balance: the adapter observes the
//! vault's lamports, requires at least `amount` of them to be unreceipted, and
//! only then calls [`UpkeepVaultV1::credit`]. There is no authority field and
//! no pending change, and their absence is the charter, not a zero.

use dclutch_sha256_adapter::digest;

use crate::CallerRoleV1;
use crate::generated_upkeep_vault_v1::*;

pub use crate::generated_upkeep_vault_v1::{
    UPKEEP_DEPOSIT_CREDIT_ACCOUNT_COUNT_V1, UPKEEP_FOUND_ACCOUNT_COUNT_V1,
    UPKEEP_OPERATION_CREDIT_TAG_V1, UPKEEP_OPERATION_DEBIT_TAG_RESERVED_V1,
    UPKEEP_OPERATION_FOUND_TAG_V1, UPKEEP_PROTOCOL_CREDIT_ACCOUNT_COUNT_V1,
    UPKEEP_SOURCE_DEPOSIT_TAG_V1, UPKEEP_SOURCE_DONATION_TAG_V1, UPKEEP_SOURCE_RESIDUE_TAG_V1,
    UPKEEP_SOURCE_SEAT_RENT_TAG_V1, UPKEEP_VAULT_ABI_VERSION_V1, UPKEEP_VAULT_RECEIPT_BYTES_V1,
    UPKEEP_VAULT_RECEIPT_MAGIC_V1, UPKEEP_VAULT_RECORD_BYTES_V1, UPKEEP_VAULT_RECORD_MAGIC_V1,
    UPKEEP_VAULT_REQUEST_BYTES_V1, UPKEEP_VAULT_REQUEST_MAGIC_V1,
};

/// Discriminates this record from every other fixed-layout record at a PDA.
pub const UPKEEP_VAULT_RECORD_KIND_V1: u8 = 1;

const RECORD_RESERVED_HEADER_BYTES: usize = 4;
const RECORD_RESERVED_TAIL_BYTES: usize = 24;
const REQUEST_RESERVED_HEADER_BYTES: usize = 3;
const REQUEST_RESERVED_TAIL_BYTES: usize = 8;
const RECEIPT_RESERVED_HEADER_BYTES: usize = 4;

/// Every refusal this contract can raise.
///
/// Contract refusals, not chain codes: the Custody SBF adapter owns the
/// protocol-visible sub-band and maps these into it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Bytes were not exactly one wire's width.
    InvalidLength,
    /// Magic, version or kind selected another wire family.
    InvalidHeader,
    /// A reserved span carried a nonzero byte, or an inactive field did.
    NonCanonical,
    /// An operation byte no route answers to.
    UnknownOperation,
    /// The reserved debit tag: there is no spend instruction (I2).
    NoSpendRoute,
    /// A source-class byte outside the four the charter names (I1).
    UnknownSourceClass,
    /// A caller role that is not a release-set role, or is Custody itself.
    UnknownCallerRole,
    /// Fields did not form the selected operation's exact shape.
    InvalidOperationShape,
    /// A credit of zero is no act.
    ZeroAmount,
    /// A running total, the count, or a sum did not fit `u64`.
    ArithmeticOverflow,
    /// A persisted record whose totals do not close: not one this module wrote.
    NotLegible,
}

/// Result alias for this contract.
pub type Result<T> = core::result::Result<T, Error>;

/// The only provenances a lamport may enter the vault under (I1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UpkeepSourceClassV1 {
    /// A residue a ruling would otherwise send nowhere: escrow-close residue
    /// after the opener is serviced, compaction dust, a rate-change surplus.
    Residue = UPKEEP_SOURCE_RESIDUE_TAG_V1,
    /// An explicitly ruled donation slice: today the maker-replay close's
    /// `unclassified_donation` less the closer's carve (decision 0024 item 5).
    Donation = UPKEEP_SOURCE_DONATION_TAG_V1,
    /// The certificate seat's prepaid rent nothing reimburses -- the one class
    /// that has carried real money on every cohort.
    SeatRent = UPKEEP_SOURCE_SEAT_RENT_TAG_V1,
    /// A voluntary deposit by a signer.
    Deposit = UPKEEP_SOURCE_DEPOSIT_TAG_V1,
}

impl UpkeepSourceClassV1 {
    /// Every class, in tag order. Four, and the decoder below admits no fifth.
    pub const ALL: [Self; 4] = [Self::Residue, Self::Donation, Self::SeatRent, Self::Deposit];

    /// Decode one class byte; total, and closed over the four.
    pub const fn decode(value: u8) -> Result<Self> {
        match value {
            UPKEEP_SOURCE_RESIDUE_TAG_V1 => Ok(Self::Residue),
            UPKEEP_SOURCE_DONATION_TAG_V1 => Ok(Self::Donation),
            UPKEEP_SOURCE_SEAT_RENT_TAG_V1 => Ok(Self::SeatRent),
            UPKEEP_SOURCE_DEPOSIT_TAG_V1 => Ok(Self::Deposit),
            _ => Err(Error::UnknownSourceClass),
        }
    }

    /// The canonical one-byte tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Whether a signer, rather than a protocol route, is the source. Only a
    /// deposit is voluntary; every other class is a route's word for lamports
    /// it moved out of an account it owns.
    #[must_use]
    pub const fn is_voluntary(self) -> bool {
        matches!(self, Self::Deposit)
    }

    /// The census label the journey ledger reports this class under.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Residue => "upkeep:residue",
            Self::Donation => "upkeep:donation",
            Self::SeatRent => "upkeep:seat-rent",
            Self::Deposit => "upkeep:deposit",
        }
    }
}

/// The two operations (I2). There is no third.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UpkeepOperationV1 {
    /// Create the record once, funded to its own rent minimum and nothing more.
    Found = UPKEEP_OPERATION_FOUND_TAG_V1,
    /// Receipt lamports that have landed, under a class.
    Credit = UPKEEP_OPERATION_CREDIT_TAG_V1,
}

impl UpkeepOperationV1 {
    /// Decode one operation byte. The reserved debit tag refuses by name.
    pub const fn decode(value: u8) -> Result<Self> {
        match value {
            UPKEEP_OPERATION_FOUND_TAG_V1 => Ok(Self::Found),
            UPKEEP_OPERATION_CREDIT_TAG_V1 => Ok(Self::Credit),
            UPKEEP_OPERATION_DEBIT_TAG_RESERVED_V1 => Err(Error::NoSpendRoute),
            _ => Err(Error::UnknownOperation),
        }
    }

    /// The canonical one-byte tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// The vault's seeds: the domain alone. One vault per Custody deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpkeepVaultSeedsV1;

impl UpkeepVaultSeedsV1 {
    /// Borrow the exact ordered SVM seed slices, excluding the bump.
    #[must_use]
    pub const fn as_slices(&self) -> [&'static [u8]; 1] {
        [crate::CUSTODY_UPKEEP_VAULT_PDA_DOMAIN_V1]
    }
}

/// The record as it sits at its PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpkeepVaultV1 {
    /// PDA bump, so the record authenticates its own address.
    pub bump: u8,
    /// Every lamport ever receipted in.
    pub inflow_total: u64,
    /// Every lamport ever paid out through a priced work route. Zero until one
    /// is ruled; a nonzero value from a program that has no such route is a
    /// census violation, not a fact.
    pub outflow_total: u64,
    /// Receipted inflow under [`UpkeepSourceClassV1::Residue`].
    pub inflow_residue: u64,
    /// Receipted inflow under [`UpkeepSourceClassV1::Donation`].
    pub inflow_donation: u64,
    /// Receipted inflow under [`UpkeepSourceClassV1::SeatRent`].
    pub inflow_seat_rent: u64,
    /// Receipted inflow under [`UpkeepSourceClassV1::Deposit`].
    pub inflow_deposit: u64,
    /// One per receipted credit; the census's clock.
    pub credit_count: u64,
    /// SHA-256 of the last credit request, zero at genesis: the receipt chain.
    pub last_credit_digest: [u8; 32],
}

impl UpkeepVaultV1 {
    /// The record as founded: every total zero.
    #[must_use]
    pub const fn genesis(bump: u8) -> Self {
        Self {
            bump,
            inflow_total: 0,
            outflow_total: 0,
            inflow_residue: 0,
            inflow_donation: 0,
            inflow_seat_rent: 0,
            inflow_deposit: 0,
            credit_count: 0,
            last_credit_digest: [0; 32],
        }
    }

    /// What the record accounts for above the account's own rent minimum.
    pub const fn receipted(self) -> Result<u64> {
        match self.inflow_total.checked_sub(self.outflow_total) {
            Some(value) => Ok(value),
            None => Err(Error::NotLegible),
        }
    }

    /// The per-class totals summed; equals `inflow_total` on any record this
    /// module wrote.
    pub const fn classes_sum(self) -> Result<u64> {
        let Some(sum) = self.inflow_residue.checked_add(self.inflow_donation) else {
            return Err(Error::ArithmeticOverflow);
        };
        let Some(sum) = sum.checked_add(self.inflow_seat_rent) else {
            return Err(Error::ArithmeticOverflow);
        };
        match sum.checked_add(self.inflow_deposit) {
            Some(sum) => Ok(sum),
            None => Err(Error::ArithmeticOverflow),
        }
    }

    /// Lamports at the address that no credit has receipted (I3's remainder).
    ///
    /// Never negative on a real account, because nothing debits: an observed
    /// balance below rent plus the receipted position is a balance this
    /// program could not have produced, and refuses.
    pub const fn unreceipted(self, lamports: u64, rent_minimum: u64) -> Result<u64> {
        let receipted = match self.receipted() {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let Some(floor) = rent_minimum.checked_add(receipted) else {
            return Err(Error::ArithmeticOverflow);
        };
        match lamports.checked_sub(floor) {
            Some(value) => Ok(value),
            None => Err(Error::NotLegible),
        }
    }

    /// Every class with its label and its receipted total, in tag order.
    ///
    /// The vault's whole promise is legibility (I3), and legibility is a thing
    /// a reader can ENUMERATE, not a set of four fields it has to know the
    /// names of. A census, a CLI or a page asks this and gets the four rows;
    /// adding a fifth class makes [`UpkeepSourceClassV1::ALL`] longer and every
    /// reader wider, with no reader to update by hand.
    #[must_use]
    pub fn class_report(self) -> [(UpkeepSourceClassV1, &'static str, u64); 4] {
        UpkeepSourceClassV1::ALL.map(|class| (class, class.label(), self.class_total(class)))
    }

    /// The receipted total under one class.
    #[must_use]
    pub const fn class_total(self, class: UpkeepSourceClassV1) -> u64 {
        match class {
            UpkeepSourceClassV1::Residue => self.inflow_residue,
            UpkeepSourceClassV1::Donation => self.inflow_donation,
            UpkeepSourceClassV1::SeatRent => self.inflow_seat_rent,
            UpkeepSourceClassV1::Deposit => self.inflow_deposit,
        }
    }

    /// The only transition. Lean twin: `credit`.
    ///
    /// Adds `amount` to the total and to its class, advances the count by one
    /// and records the request digest. It does not read a balance: the adapter
    /// has already required that at least `amount` unreceipted lamports sit at
    /// the address.
    pub fn credit(
        self,
        class: UpkeepSourceClassV1,
        amount: u64,
        request_digest: [u8; 32],
    ) -> Result<Self> {
        if amount == 0 {
            return Err(Error::ZeroAmount);
        }
        let inflow_total = self
            .inflow_total
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        let credit_count = self
            .credit_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let classed = self
            .class_total(class)
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut after = Self {
            inflow_total,
            credit_count,
            last_credit_digest: request_digest,
            ..self
        };
        match class {
            UpkeepSourceClassV1::Residue => after.inflow_residue = classed,
            UpkeepSourceClassV1::Donation => after.inflow_donation = classed,
            UpkeepSourceClassV1::SeatRent => after.inflow_seat_rent = classed,
            UpkeepSourceClassV1::Deposit => after.inflow_deposit = classed,
        }
        Ok(after)
    }

    /// Hostile-decode one exact record.
    ///
    /// Refuses on the READ a record whose totals do not close: every writer
    /// above keeps them closed, so the only way to see one is corruption or a
    /// foreign author, and a consumer never has to ask.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != UPKEEP_VAULT_RECORD_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(UPKEEP_VAULT_RECORD_MAGIC_V1.as_slice())
            || u16_at(input, UPKEEP_VAULT_RECORD_VERSION_OFFSET) != UPKEEP_VAULT_ABI_VERSION_V1
            || input.get(UPKEEP_VAULT_RECORD_KIND_OFFSET).copied()
                != Some(UPKEEP_VAULT_RECORD_KIND_V1)
        {
            return Err(Error::InvalidHeader);
        }
        require_zero(
            input,
            UPKEEP_VAULT_RECORD_RESERVED_HEADER_OFFSET,
            RECORD_RESERVED_HEADER_BYTES,
        )?;
        require_zero(
            input,
            UPKEEP_VAULT_RECORD_RESERVED_TAIL_OFFSET,
            RECORD_RESERVED_TAIL_BYTES,
        )?;
        let value = Self {
            bump: *input
                .get(UPKEEP_VAULT_RECORD_BUMP_OFFSET)
                .ok_or(Error::InvalidLength)?,
            inflow_total: u64_at(input, UPKEEP_VAULT_RECORD_INFLOW_TOTAL_OFFSET),
            outflow_total: u64_at(input, UPKEEP_VAULT_RECORD_OUTFLOW_TOTAL_OFFSET),
            inflow_residue: u64_at(input, UPKEEP_VAULT_RECORD_INFLOW_RESIDUE_OFFSET),
            inflow_donation: u64_at(input, UPKEEP_VAULT_RECORD_INFLOW_DONATION_OFFSET),
            inflow_seat_rent: u64_at(input, UPKEEP_VAULT_RECORD_INFLOW_SEAT_RENT_OFFSET),
            inflow_deposit: u64_at(input, UPKEEP_VAULT_RECORD_INFLOW_DEPOSIT_OFFSET),
            credit_count: u64_at(input, UPKEEP_VAULT_RECORD_CREDIT_COUNT_OFFSET),
            last_credit_digest: array_at(input, UPKEEP_VAULT_RECORD_LAST_CREDIT_DIGEST_OFFSET),
        };
        if value.receipted().is_err() || value.classes_sum()? != value.inflow_total {
            return Err(Error::NotLegible);
        }
        Ok(value)
    }

    /// Encode one canonical record.
    #[must_use]
    pub fn to_bytes(self) -> [u8; UPKEEP_VAULT_RECORD_BYTES_V1] {
        let mut output = [0_u8; UPKEEP_VAULT_RECORD_BYTES_V1];
        output[..8].copy_from_slice(&UPKEEP_VAULT_RECORD_MAGIC_V1);
        put_u16(
            &mut output,
            UPKEEP_VAULT_RECORD_VERSION_OFFSET,
            UPKEEP_VAULT_ABI_VERSION_V1,
        );
        output[UPKEEP_VAULT_RECORD_KIND_OFFSET] = UPKEEP_VAULT_RECORD_KIND_V1;
        output[UPKEEP_VAULT_RECORD_BUMP_OFFSET] = self.bump;
        for (offset, value) in [
            (UPKEEP_VAULT_RECORD_INFLOW_TOTAL_OFFSET, self.inflow_total),
            (UPKEEP_VAULT_RECORD_OUTFLOW_TOTAL_OFFSET, self.outflow_total),
            (
                UPKEEP_VAULT_RECORD_INFLOW_RESIDUE_OFFSET,
                self.inflow_residue,
            ),
            (
                UPKEEP_VAULT_RECORD_INFLOW_DONATION_OFFSET,
                self.inflow_donation,
            ),
            (
                UPKEEP_VAULT_RECORD_INFLOW_SEAT_RENT_OFFSET,
                self.inflow_seat_rent,
            ),
            (
                UPKEEP_VAULT_RECORD_INFLOW_DEPOSIT_OFFSET,
                self.inflow_deposit,
            ),
            (UPKEEP_VAULT_RECORD_CREDIT_COUNT_OFFSET, self.credit_count),
        ] {
            put_u64(&mut output, offset, value);
        }
        put_array(
            &mut output,
            UPKEEP_VAULT_RECORD_LAST_CREDIT_DIGEST_OFFSET,
            &self.last_credit_digest,
        );
        output
    }
}

/// The protocol caller of a non-voluntary credit: the coordinates the release
/// set's caller-authority PDA is derived from, exactly as every other Custody
/// route derives it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpkeepProtocolCallerV1 {
    /// Registry role of the calling program. Never Custody itself.
    pub caller_role: CallerRoleV1,
    /// Immutable execution-release-set content identity.
    pub release_set: [u8; 32],
    /// The Market the act belongs to.
    pub market: [u8; 32],
    /// The account the lamports left: the maker replay, the seat, the escrow.
    pub context: [u8; 32],
}

/// One credit: what landed, under which class, receipted by whom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpkeepCreditV1 {
    /// The provenance.
    pub source_class: UpkeepSourceClassV1,
    /// `None` for a deposit (a signer is the source); the caller-authority
    /// coordinates for the three protocol classes.
    pub caller: Option<UpkeepProtocolCallerV1>,
    /// SHA-256 of the calling route's own receipt for the act that moved the
    /// lamports, so the act is receipted twice; zero for a deposit.
    pub receipt_digest: [u8; 32],
    /// Lamports to receipt. Never zero.
    pub amount: u64,
}

/// One exact request, for either route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpkeepRequestV1 {
    /// Which route.
    pub operation: UpkeepOperationV1,
    /// The credit, exactly when `operation` is [`UpkeepOperationV1::Credit`].
    pub credit: Option<UpkeepCreditV1>,
}

impl UpkeepRequestV1 {
    /// The founding request: a header and zeros.
    pub const FOUND: Self = Self {
        operation: UpkeepOperationV1::Found,
        credit: None,
    };

    /// Validate the request's shape against its operation.
    pub fn validate(self) -> Result<Self> {
        match (self.operation, self.credit) {
            (UpkeepOperationV1::Found, None) => Ok(self),
            (UpkeepOperationV1::Credit, Some(credit)) => {
                if credit.amount == 0 {
                    return Err(Error::ZeroAmount);
                }
                match (credit.source_class.is_voluntary(), credit.caller) {
                    (true, None) => {
                        if !is_zero(&credit.receipt_digest) {
                            return Err(Error::InvalidOperationShape);
                        }
                    }
                    (false, Some(caller)) => {
                        if is_zero(&caller.release_set)
                            || is_zero(&caller.market)
                            || is_zero(&caller.context)
                            || is_zero(&credit.receipt_digest)
                        {
                            return Err(Error::InvalidOperationShape);
                        }
                        if matches!(caller.caller_role, CallerRoleV1::Custody) {
                            return Err(Error::UnknownCallerRole);
                        }
                    }
                    _ => return Err(Error::InvalidOperationShape),
                }
                Ok(self)
            }
            _ => Err(Error::InvalidOperationShape),
        }
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != UPKEEP_VAULT_REQUEST_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(UPKEEP_VAULT_REQUEST_MAGIC_V1.as_slice())
            || u16_at(input, UPKEEP_VAULT_REQUEST_VERSION_OFFSET) != UPKEEP_VAULT_ABI_VERSION_V1
        {
            return Err(Error::InvalidHeader);
        }
        require_zero(
            input,
            UPKEEP_VAULT_REQUEST_RESERVED_HEADER_OFFSET,
            REQUEST_RESERVED_HEADER_BYTES,
        )?;
        require_zero(
            input,
            UPKEEP_VAULT_REQUEST_RESERVED_TAIL_OFFSET,
            REQUEST_RESERVED_TAIL_BYTES,
        )?;
        let operation = UpkeepOperationV1::decode(
            *input
                .get(UPKEEP_VAULT_REQUEST_OPERATION_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )?;
        let class_byte = *input
            .get(UPKEEP_VAULT_REQUEST_SOURCE_CLASS_OFFSET)
            .ok_or(Error::InvalidLength)?;
        let role_byte = *input
            .get(UPKEEP_VAULT_REQUEST_CALLER_ROLE_OFFSET)
            .ok_or(Error::InvalidLength)?;
        let release_set = array_at(input, UPKEEP_VAULT_REQUEST_RELEASE_SET_OFFSET);
        let market = array_at(input, UPKEEP_VAULT_REQUEST_MARKET_OFFSET);
        let context = array_at(input, UPKEEP_VAULT_REQUEST_CONTEXT_OFFSET);
        let receipt_digest = array_at(input, UPKEEP_VAULT_REQUEST_RECEIPT_DIGEST_OFFSET);
        let amount = u64_at(input, UPKEEP_VAULT_REQUEST_AMOUNT_OFFSET);
        let credit = match operation {
            UpkeepOperationV1::Found => {
                // A founding carries nothing below the header, and a founding
                // that carried something would be a request with two shapes.
                if class_byte != 0
                    || role_byte != 0
                    || !is_zero(&release_set)
                    || !is_zero(&market)
                    || !is_zero(&context)
                    || !is_zero(&receipt_digest)
                    || amount != 0
                {
                    return Err(Error::NonCanonical);
                }
                None
            }
            UpkeepOperationV1::Credit => {
                let source_class = UpkeepSourceClassV1::decode(class_byte)?;
                let caller = if source_class.is_voluntary() {
                    if role_byte != 0
                        || !is_zero(&release_set)
                        || !is_zero(&market)
                        || !is_zero(&context)
                    {
                        return Err(Error::NonCanonical);
                    }
                    None
                } else {
                    Some(UpkeepProtocolCallerV1 {
                        caller_role: caller_role(role_byte)?,
                        release_set,
                        market,
                        context,
                    })
                };
                Some(UpkeepCreditV1 {
                    source_class,
                    caller,
                    receipt_digest,
                    amount,
                })
            }
        };
        Self { operation, credit }.validate()
    }

    /// Encode one canonical request.
    pub fn to_bytes(self) -> Result<[u8; UPKEEP_VAULT_REQUEST_BYTES_V1]> {
        self.validate()?;
        let mut output = [0_u8; UPKEEP_VAULT_REQUEST_BYTES_V1];
        output[..8].copy_from_slice(&UPKEEP_VAULT_REQUEST_MAGIC_V1);
        put_u16(
            &mut output,
            UPKEEP_VAULT_REQUEST_VERSION_OFFSET,
            UPKEEP_VAULT_ABI_VERSION_V1,
        );
        output[UPKEEP_VAULT_REQUEST_OPERATION_OFFSET] = self.operation.tag();
        if let Some(credit) = self.credit {
            output[UPKEEP_VAULT_REQUEST_SOURCE_CLASS_OFFSET] = credit.source_class.tag();
            if let Some(caller) = credit.caller {
                output[UPKEEP_VAULT_REQUEST_CALLER_ROLE_OFFSET] = caller.caller_role as u8;
                put_array(
                    &mut output,
                    UPKEEP_VAULT_REQUEST_RELEASE_SET_OFFSET,
                    &caller.release_set,
                );
                put_array(
                    &mut output,
                    UPKEEP_VAULT_REQUEST_MARKET_OFFSET,
                    &caller.market,
                );
                put_array(
                    &mut output,
                    UPKEEP_VAULT_REQUEST_CONTEXT_OFFSET,
                    &caller.context,
                );
            }
            put_array(
                &mut output,
                UPKEEP_VAULT_REQUEST_RECEIPT_DIGEST_OFFSET,
                &credit.receipt_digest,
            );
            put_u64(
                &mut output,
                UPKEEP_VAULT_REQUEST_AMOUNT_OFFSET,
                credit.amount,
            );
        }
        Ok(output)
    }

    /// SHA-256 of the canonical request: what the record's `last_credit_digest`
    /// and the receipt's `request_digest` both carry.
    pub fn digest(self) -> Result<[u8; 32]> {
        Ok(digest(&self.to_bytes()?))
    }
}

/// What a credit returns: the act, and the record's position after it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpkeepCreditReceiptV1 {
    /// The operation receipted.
    pub operation: UpkeepOperationV1,
    /// The class credited; `Residue`'s tag for a founding, whose class is none.
    pub source_class: UpkeepSourceClassV1,
    /// SHA-256 of the request.
    pub request_digest: [u8; 32],
    /// The vault's address.
    pub vault: [u8; 32],
    /// Lamports receipted by this act; zero for a founding.
    pub amount: u64,
    /// The record's inflow total after the act.
    pub inflow_total_after: u64,
    /// The record's outflow total after the act. Zero, until a route is ruled.
    pub outflow_total_after: u64,
    /// The record's credit count after the act.
    pub credit_count_after: u64,
}

impl UpkeepCreditReceiptV1 {
    /// Encode one canonical receipt.
    #[must_use]
    pub fn to_bytes(self) -> [u8; UPKEEP_VAULT_RECEIPT_BYTES_V1] {
        let mut output = [0_u8; UPKEEP_VAULT_RECEIPT_BYTES_V1];
        output[..8].copy_from_slice(&UPKEEP_VAULT_RECEIPT_MAGIC_V1);
        put_u16(
            &mut output,
            UPKEEP_VAULT_RECEIPT_VERSION_OFFSET,
            UPKEEP_VAULT_ABI_VERSION_V1,
        );
        output[UPKEEP_VAULT_RECEIPT_OPERATION_OFFSET] = self.operation.tag();
        output[UPKEEP_VAULT_RECEIPT_SOURCE_CLASS_OFFSET] = self.source_class.tag();
        put_array(
            &mut output,
            UPKEEP_VAULT_RECEIPT_REQUEST_DIGEST_OFFSET,
            &self.request_digest,
        );
        put_array(&mut output, UPKEEP_VAULT_RECEIPT_VAULT_OFFSET, &self.vault);
        for (offset, value) in [
            (UPKEEP_VAULT_RECEIPT_AMOUNT_OFFSET, self.amount),
            (
                UPKEEP_VAULT_RECEIPT_INFLOW_TOTAL_AFTER_OFFSET,
                self.inflow_total_after,
            ),
            (
                UPKEEP_VAULT_RECEIPT_OUTFLOW_TOTAL_AFTER_OFFSET,
                self.outflow_total_after,
            ),
            (
                UPKEEP_VAULT_RECEIPT_CREDIT_COUNT_AFTER_OFFSET,
                self.credit_count_after,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        output
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != UPKEEP_VAULT_RECEIPT_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(UPKEEP_VAULT_RECEIPT_MAGIC_V1.as_slice())
            || u16_at(input, UPKEEP_VAULT_RECEIPT_VERSION_OFFSET) != UPKEEP_VAULT_ABI_VERSION_V1
        {
            return Err(Error::InvalidHeader);
        }
        require_zero(
            input,
            UPKEEP_VAULT_RECEIPT_RESERVED_HEADER_OFFSET,
            RECEIPT_RESERVED_HEADER_BYTES,
        )?;
        let value = Self {
            operation: UpkeepOperationV1::decode(
                *input
                    .get(UPKEEP_VAULT_RECEIPT_OPERATION_OFFSET)
                    .ok_or(Error::InvalidLength)?,
            )?,
            source_class: UpkeepSourceClassV1::decode(
                *input
                    .get(UPKEEP_VAULT_RECEIPT_SOURCE_CLASS_OFFSET)
                    .ok_or(Error::InvalidLength)?,
            )?,
            request_digest: array_at(input, UPKEEP_VAULT_RECEIPT_REQUEST_DIGEST_OFFSET),
            vault: array_at(input, UPKEEP_VAULT_RECEIPT_VAULT_OFFSET),
            amount: u64_at(input, UPKEEP_VAULT_RECEIPT_AMOUNT_OFFSET),
            inflow_total_after: u64_at(input, UPKEEP_VAULT_RECEIPT_INFLOW_TOTAL_AFTER_OFFSET),
            outflow_total_after: u64_at(input, UPKEEP_VAULT_RECEIPT_OUTFLOW_TOTAL_AFTER_OFFSET),
            credit_count_after: u64_at(input, UPKEEP_VAULT_RECEIPT_CREDIT_COUNT_AFTER_OFFSET),
        };
        if value.inflow_total_after < value.amount
            || value.outflow_total_after > value.inflow_total_after
            || (value.operation == UpkeepOperationV1::Credit) != (value.amount != 0)
        {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }
}

/// The same four bytes `CustodyRequestV1` admits as a caller role, and the
/// same refusal of Custody as a caller of itself.
const fn caller_role(value: u8) -> Result<CallerRoleV1> {
    match value {
        0 => Ok(CallerRoleV1::Core),
        1 => Ok(CallerRoleV1::Claims),
        2 => Ok(CallerRoleV1::Trading),
        3 => Ok(CallerRoleV1::Resolution),
        _ => Err(Error::UnknownCallerRole),
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let span = input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?;
    if span.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

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

// THE HELPERS BELOW INDEX BY A CONSTANT OFFSET INTO AN EXACT-WIDTH WIRE. Every
// caller is a decode that has already refused any other width, or an encode
// into an array of exactly that width, and every offset is a `const` of this
// module's own emitted layout.
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
mod tests {
    use super::*;

    const RENT_MINIMUM: u64 = 1_781_760;
    const DIGEST: [u8; 32] = [0x5a; 32];

    fn protocol_credit(class: UpkeepSourceClassV1, amount: u64) -> UpkeepRequestV1 {
        UpkeepRequestV1 {
            operation: UpkeepOperationV1::Credit,
            credit: Some(UpkeepCreditV1 {
                source_class: class,
                caller: Some(UpkeepProtocolCallerV1 {
                    caller_role: CallerRoleV1::Trading,
                    release_set: [0x11; 32],
                    market: [0x12; 32],
                    context: [0x13; 32],
                }),
                receipt_digest: [0x14; 32],
                amount,
            }),
        }
    }

    fn deposit(amount: u64) -> UpkeepRequestV1 {
        UpkeepRequestV1 {
            operation: UpkeepOperationV1::Credit,
            credit: Some(UpkeepCreditV1 {
                source_class: UpkeepSourceClassV1::Deposit,
                caller: None,
                receipt_digest: [0; 32],
                amount,
            }),
        }
    }

    #[test]
    fn genesis_round_trips_and_is_legible_at_its_rent() {
        let vault = UpkeepVaultV1::genesis(254);
        let bytes = vault.to_bytes();
        assert_eq!(UpkeepVaultV1::decode(&bytes), Ok(vault));
        assert_eq!(vault.receipted(), Ok(0));
        assert_eq!(vault.unreceipted(RENT_MINIMUM, RENT_MINIMUM), Ok(0));
        assert_eq!(vault.unreceipted(RENT_MINIMUM + 7, RENT_MINIMUM), Ok(7));
        // Below its own rent is a balance this program could not have produced.
        assert_eq!(
            vault.unreceipted(RENT_MINIMUM - 1, RENT_MINIMUM),
            Err(Error::NotLegible)
        );
    }

    /// I2, by name: the reserved debit tag is refused as `NoSpendRoute`, not
    /// as an unknown byte, and every other unknown byte still says unknown.
    #[test]
    fn there_is_no_spend_instruction_and_it_says_so() {
        assert_eq!(
            UpkeepOperationV1::decode(UPKEEP_OPERATION_DEBIT_TAG_RESERVED_V1),
            Err(Error::NoSpendRoute)
        );
        assert_eq!(UpkeepOperationV1::decode(3), Err(Error::UnknownOperation));
        let mut hostile = deposit(5).to_bytes().expect("deposit encodes");
        hostile[UPKEEP_VAULT_REQUEST_OPERATION_OFFSET] = UPKEEP_OPERATION_DEBIT_TAG_RESERVED_V1;
        assert_eq!(UpkeepRequestV1::decode(&hostile), Err(Error::NoSpendRoute));
    }

    /// I1: four classes, no fifth, and a protocol class cannot arrive without
    /// a caller while a deposit cannot arrive with one.
    #[test]
    fn the_source_classes_are_closed_and_shaped() {
        for class in UpkeepSourceClassV1::ALL {
            assert_eq!(UpkeepSourceClassV1::decode(class.tag()), Ok(class));
        }
        assert_eq!(
            UpkeepSourceClassV1::decode(4),
            Err(Error::UnknownSourceClass)
        );
        let mut hostile = deposit(5).to_bytes().expect("deposit encodes");
        hostile[UPKEEP_VAULT_REQUEST_SOURCE_CLASS_OFFSET] = 4;
        assert_eq!(
            UpkeepRequestV1::decode(&hostile),
            Err(Error::UnknownSourceClass)
        );

        // A residue "from a wallet": the class says protocol, the frame says
        // signer. Unrepresentable on the wire, refused by shape.
        let mut laundered = deposit(5);
        laundered.credit = laundered.credit.map(|credit| UpkeepCreditV1 {
            source_class: UpkeepSourceClassV1::Residue,
            ..credit
        });
        assert_eq!(laundered.to_bytes(), Err(Error::InvalidOperationShape));
        // And Custody presenting itself as the caller of its own vault.
        let mut reflexive = protocol_credit(UpkeepSourceClassV1::Residue, 5);
        reflexive.credit = reflexive.credit.map(|credit| UpkeepCreditV1 {
            caller: credit.caller.map(|caller| UpkeepProtocolCallerV1 {
                caller_role: CallerRoleV1::Custody,
                ..caller
            }),
            ..credit
        });
        assert_eq!(reflexive.to_bytes(), Err(Error::UnknownCallerRole));
    }

    /// I3 as something a reader can ENUMERATE: the report names every class,
    /// labels it, and its four totals close on the inflow total exactly.
    ///
    /// This is also where the charter's honest measurement lives. The class
    /// that has carried real money on every cohort is the certificate seat's
    /// prepay -- 2,786,520 lamports, cohort-14 -- and the class the one wired
    /// producer credits is `Donation`, which has measured zero every time. A
    /// report that could only say "inflow_total" would let those two facts look
    /// like one number.
    #[test]
    fn the_class_report_names_every_class_and_closes_on_the_total() {
        let vault = UpkeepVaultV1::genesis(254);
        assert_eq!(
            vault.class_report(),
            [
                (UpkeepSourceClassV1::Residue, "upkeep:residue", 0),
                (UpkeepSourceClassV1::Donation, "upkeep:donation", 0),
                (UpkeepSourceClassV1::SeatRent, "upkeep:seat-rent", 0),
                (UpkeepSourceClassV1::Deposit, "upkeep:deposit", 0),
            ]
        );
        let seated = vault
            .credit(UpkeepSourceClassV1::SeatRent, 2_786_520, DIGEST)
            .expect("seat rent credits");
        let report = seated.class_report();
        assert_eq!(
            report.map(|(class, _, total)| (class, total)),
            [
                (UpkeepSourceClassV1::Residue, 0),
                (UpkeepSourceClassV1::Donation, 0),
                (UpkeepSourceClassV1::SeatRent, 2_786_520),
                (UpkeepSourceClassV1::Deposit, 0),
            ]
        );
        assert_eq!(
            report.iter().map(|(_, _, total)| total).sum::<u64>(),
            seated.inflow_total
        );
        assert_eq!(seated.receipted(), Ok(seated.inflow_total));
    }

    #[test]
    fn requests_round_trip_and_refuse_reserved_bytes() {
        for request in [
            UpkeepRequestV1::FOUND,
            deposit(9),
            protocol_credit(UpkeepSourceClassV1::Donation, 1_463_040),
            protocol_credit(UpkeepSourceClassV1::SeatRent, 2_786_520),
        ] {
            let bytes = request.to_bytes().expect("encodes");
            assert_eq!(UpkeepRequestV1::decode(&bytes), Ok(request));
            let mut hostile = bytes;
            hostile[UPKEEP_VAULT_REQUEST_RESERVED_TAIL_OFFSET] = 1;
            assert_eq!(UpkeepRequestV1::decode(&hostile), Err(Error::NonCanonical));
        }
        let mut founding_with_a_body = UpkeepRequestV1::FOUND.to_bytes().expect("encodes");
        put_u64(
            &mut founding_with_a_body,
            UPKEEP_VAULT_REQUEST_AMOUNT_OFFSET,
            1,
        );
        assert_eq!(
            UpkeepRequestV1::decode(&founding_with_a_body),
            Err(Error::NonCanonical)
        );
        assert_eq!(deposit(0).to_bytes(), Err(Error::ZeroAmount));
    }

    /// I3 across the only transition: totals, class, count and digest move
    /// exactly as the amount says, and the outflow total never moves.
    #[test]
    fn a_credit_moves_exactly_the_amount_and_nothing_else() {
        let vault = UpkeepVaultV1::genesis(254);
        let after = vault
            .credit(UpkeepSourceClassV1::Donation, 1_463_040, DIGEST)
            .expect("credits");
        assert_eq!(after.inflow_total, 1_463_040);
        assert_eq!(after.inflow_donation, 1_463_040);
        assert_eq!(
            after.inflow_residue + after.inflow_seat_rent + after.inflow_deposit,
            0
        );
        assert_eq!(after.outflow_total, 0);
        assert_eq!(after.credit_count, 1);
        assert_eq!(after.last_credit_digest, DIGEST);
        assert_eq!(after.receipted(), Ok(1_463_040));
        assert_eq!(
            after.unreceipted(RENT_MINIMUM + 1_463_040, RENT_MINIMUM),
            Ok(0)
        );
        let twice = after
            .credit(UpkeepSourceClassV1::SeatRent, 2_786_520, [0x6b; 32])
            .expect("credits again");
        assert_eq!(twice.inflow_total, 1_463_040 + 2_786_520);
        assert_eq!(twice.credit_count, 2);
        assert_eq!(UpkeepVaultV1::decode(&twice.to_bytes()), Ok(twice));
        assert_eq!(
            vault.credit(UpkeepSourceClassV1::Deposit, 0, DIGEST),
            Err(Error::ZeroAmount)
        );
        assert_eq!(
            twice.credit(UpkeepSourceClassV1::Deposit, u64::MAX, DIGEST),
            Err(Error::ArithmeticOverflow)
        );
    }

    /// A record whose totals do not close is not one this module wrote.
    #[test]
    fn a_record_whose_totals_do_not_close_refuses_on_decode() {
        let vault = UpkeepVaultV1::genesis(254)
            .credit(UpkeepSourceClassV1::Residue, 5, DIGEST)
            .expect("credits");
        let mut foreign = vault.to_bytes();
        put_u64(&mut foreign, UPKEEP_VAULT_RECORD_INFLOW_TOTAL_OFFSET, 6);
        assert_eq!(UpkeepVaultV1::decode(&foreign), Err(Error::NotLegible));
        let mut spent = vault.to_bytes();
        put_u64(&mut spent, UPKEEP_VAULT_RECORD_OUTFLOW_TOTAL_OFFSET, 6);
        assert_eq!(UpkeepVaultV1::decode(&spent), Err(Error::NotLegible));
        let mut wrong_kind = vault.to_bytes();
        wrong_kind[UPKEEP_VAULT_RECORD_KIND_OFFSET] = UPKEEP_VAULT_RECORD_KIND_V1 + 1;
        assert_eq!(
            UpkeepVaultV1::decode(&wrong_kind),
            Err(Error::InvalidHeader)
        );
    }

    #[test]
    fn receipts_round_trip_and_refuse_a_credit_larger_than_the_total() {
        let receipt = UpkeepCreditReceiptV1 {
            operation: UpkeepOperationV1::Credit,
            source_class: UpkeepSourceClassV1::Donation,
            request_digest: DIGEST,
            vault: [0x77; 32],
            amount: 10,
            inflow_total_after: 25,
            outflow_total_after: 0,
            credit_count_after: 3,
        };
        assert_eq!(
            UpkeepCreditReceiptV1::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
        let overstated = UpkeepCreditReceiptV1 {
            amount: 26,
            ..receipt
        };
        assert_eq!(
            UpkeepCreditReceiptV1::decode(&overstated.to_bytes()),
            Err(Error::NonCanonical)
        );
    }

    #[test]
    fn the_seeds_are_the_domain_alone() {
        assert_eq!(
            UpkeepVaultSeedsV1.as_slices(),
            [b"dclutch:custody-upkeep:v1".as_slice()]
        );
    }
}
