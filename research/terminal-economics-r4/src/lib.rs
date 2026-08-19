#![no_std]
#![forbid(unsafe_code)]

//! MODEL-only R4 terminal economics.
//!
//! This dependency-free, allocation-free state machine models a creation-time
//! V4 profile. It is not a Solana program, token adapter, migration, or release
//! claim. Legacy mints without a creation-time close authority remain permanent
//! infrastructure and are never relabeled as closeable.

mod wire;

pub use wire::{
    CreditRootWireV1, CreditWireV1, DecodeError, MintBindingWireV4, PolicyWireV4, RentWireV1,
    TombstoneWireV1, CREDIT_ROOT_WIRE_BYTES, CREDIT_WIRE_BYTES, MINT_BINDING_WIRE_BYTES,
    POLICY_WIRE_BYTES, RENT_WIRE_BYTES, TOMBSTONE_WIRE_BYTES,
};

pub const MAX_OUTCOMES: usize = 4;
pub const MAX_POSITIONS: usize = 4;
pub const MAX_BEARERS: usize = 8;
pub const MAX_CREDITS: usize = 8;
pub const MAX_RENTS: usize = 22;
pub const MAX_ATOMS: u64 = 1_000_000_000_000;
pub type Id = [u8; 32];

const ZERO_ID: Id = [0; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Shape,
    Identity,
    Authority,
    Phase,
    Replay,
    Arithmetic,
    Insufficient,
    Occupied,
    Missing,
    NonCanonical,
    OutstandingClaims,
    OutstandingCredit,
    Rent,
    Sink,
    LegacyStop,
    MigrationStop,
    Invariant,
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Phase {
    Active = 1,
    Resolved = 2,
    CreditsSealed = 3,
    Terminal = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AccountClass {
    RefundableTransient = 1,
    PermanentInfra = 2,
    PermanentTombstone = 3,
    ExternalOwnerState = 4,
    UnclassifiedStop = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MintVersion {
    LegacyNoClose = 1,
    R4Closeable = 4,
}

/// Legacy no-close mints remain permanent infrastructure. There is no mutation
/// here that pretends an extension can be retrofitted after mint creation.
pub fn classify_mint(version: MintVersion, close_authority: Option<Id>) -> Result<AccountClass> {
    match version {
        MintVersion::LegacyNoClose => Ok(AccountClass::PermanentInfra),
        MintVersion::R4Closeable => match close_authority {
            Some(authority) if authority != ZERO_ID => Ok(AccountClass::RefundableTransient),
            _ => Err(Error::Authority),
        },
    }
}

/// An attempted in-place legacy upgrade is a named refusal, not a migration.
pub const fn migrate_legacy_mint_in_place() -> Result<()> {
    Err(Error::MigrationStop)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Market,
    Hoard,
    Supply,
    Resolution,
    Position(u8),
    Mint(u8),
    CreditVault,
    Credit(u8),
    Replay,
}

impl Role {
    pub const fn index(self) -> usize {
        match self {
            Self::Market => 0,
            Self::Hoard => 1,
            Self::Supply => 2,
            Self::Resolution => 3,
            Self::Position(slot) => 4 + slot as usize,
            Self::Mint(outcome) => 8 + outcome as usize,
            Self::CreditVault => 12,
            Self::Credit(slot) => 13 + slot as usize,
            Self::Replay => 21,
        }
    }

    pub const fn code(self) -> u8 {
        match self {
            Self::Market => 1,
            Self::Hoard => 2,
            Self::Supply => 3,
            Self::Resolution => 4,
            Self::Position(slot) => 16_u8.wrapping_add(slot),
            Self::Mint(outcome) => 32_u8.wrapping_add(outcome),
            Self::CreditVault => 48,
            Self::Credit(slot) => 49_u8.wrapping_add(slot),
            Self::Replay => 63,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentRecord {
    pub present: bool,
    pub role: Role,
    pub class: AccountClass,
    pub market: Id,
    pub generation: u64,
    pub account: Id,
    pub payer: Id,
    pub refund_to: Id,
    pub sink: Id,
    pub principal: u64,
    pub donations: u64,
    pub balance: u64,
    pub refund_paid: u64,
    pub sink_paid: u64,
    pub closed: bool,
}

impl RentRecord {
    pub const EMPTY: Self = Self {
        present: false,
        role: Role::Market,
        class: AccountClass::UnclassifiedStop,
        market: ZERO_ID,
        generation: 0,
        account: ZERO_ID,
        payer: ZERO_ID,
        refund_to: ZERO_ID,
        sink: ZERO_ID,
        principal: 0,
        donations: 0,
        balance: 0,
        refund_paid: 0,
        sink_paid: 0,
        closed: false,
    };

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role: Role,
        class: AccountClass,
        market: Id,
        generation: u64,
        payer: Id,
        refund_to: Id,
        sink: Id,
        principal: u64,
        prefund_donation: u64,
    ) -> Result<Self> {
        let account = role_account(market, role);
        if class == AccountClass::UnclassifiedStop
            || market == ZERO_ID
            || generation == 0
            || payer == ZERO_ID
            || refund_to == ZERO_ID
            || sink == ZERO_ID
            || refund_to == sink
            || account == ZERO_ID
            || account == refund_to
            || account == sink
            || principal == 0
            || principal > MAX_ATOMS
            || prefund_donation > MAX_ATOMS
        {
            return Err(Error::Rent);
        }
        let balance = principal
            .checked_add(prefund_donation)
            .ok_or(Error::Arithmetic)?;
        if balance > MAX_ATOMS {
            return Err(Error::Rent);
        }
        let record = Self {
            present: true,
            role,
            class,
            market,
            generation,
            account,
            payer,
            refund_to,
            sink,
            principal,
            donations: prefund_donation,
            balance,
            refund_paid: 0,
            sink_paid: 0,
            closed: false,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.present {
            return if *self == Self::EMPTY {
                Ok(())
            } else {
                Err(Error::NonCanonical)
            };
        }
        match self.role {
            Role::Position(slot) if usize::from(slot) >= MAX_POSITIONS => {
                return Err(Error::Rent);
            }
            Role::Mint(outcome) if usize::from(outcome) >= MAX_OUTCOMES => {
                return Err(Error::Rent);
            }
            Role::Credit(slot) if usize::from(slot) >= MAX_CREDITS => {
                return Err(Error::Rent);
            }
            _ => {}
        }
        if self.class == AccountClass::UnclassifiedStop
            || self.market == ZERO_ID
            || self.generation == 0
            || self.account != role_account(self.market, self.role)
            || self.account == self.refund_to
            || self.account == self.sink
            || self.payer == ZERO_ID
            || self.refund_to == ZERO_ID
            || self.sink == ZERO_ID
            || self.refund_to == self.sink
            || self.principal == 0
            || self.principal > MAX_ATOMS
            || self.donations > MAX_ATOMS
        {
            return Err(Error::Rent);
        }
        let total = self
            .principal
            .checked_add(self.donations)
            .ok_or(Error::Arithmetic)?;
        if total > MAX_ATOMS {
            return Err(Error::Rent);
        }
        let observed = self
            .balance
            .checked_add(self.refund_paid)
            .and_then(|value| value.checked_add(self.sink_paid))
            .ok_or(Error::Arithmetic)?;
        if observed != total {
            return Err(Error::Invariant);
        }
        match self.class {
            AccountClass::PermanentInfra | AccountClass::PermanentTombstone => {
                if self.closed || self.refund_paid != 0 || self.sink_paid != 0 {
                    return Err(Error::Rent);
                }
            }
            AccountClass::RefundableTransient | AccountClass::ExternalOwnerState => {
                if self.closed {
                    if self.balance != 0
                        || self.refund_paid != self.principal
                        || self.sink_paid != self.donations
                    {
                        return Err(Error::Rent);
                    }
                } else if self.refund_paid != 0 || self.sink_paid != 0 {
                    return Err(Error::Rent);
                }
            }
            AccountClass::UnclassifiedStop => return Err(Error::Rent),
        }
        Ok(())
    }

    pub fn donate(&mut self, amount: u64) -> Result<()> {
        self.validate()?;
        if self.closed || amount == 0 {
            return Err(Error::Rent);
        }
        let mut next = *self;
        next.donations = next
            .donations
            .checked_add(amount)
            .ok_or(Error::Arithmetic)?;
        next.balance = next.balance.checked_add(amount).ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn close(&mut self, refund_to: Id, sink: Id) -> Result<RentCloseEffect> {
        self.validate()?;
        if self.closed {
            return Err(Error::Replay);
        }
        if !matches!(
            self.class,
            AccountClass::RefundableTransient | AccountClass::ExternalOwnerState
        ) {
            return Err(Error::LegacyStop);
        }
        if refund_to != self.refund_to || sink != self.sink {
            return Err(Error::Identity);
        }
        let mut next = *self;
        next.balance = 0;
        next.refund_paid = next.principal;
        next.sink_paid = next.donations;
        next.closed = true;
        next.validate()?;
        let effect = RentCloseEffect {
            refund_atoms: next.refund_paid,
            donation_sink_atoms: next.sink_paid,
        };
        *self = next;
        Ok(effect)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentCloseEffect {
    pub refund_atoms: u64,
    pub donation_sink_atoms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentSummary {
    pub closed_role_bits: u32,
    pub permanent_role_bits: u32,
    pub open_external_role_bits: u32,
    pub principal_refunded: u64,
    pub donations_sunk: u64,
    pub permanent_principal: u64,
    pub permanent_donations: u64,
    pub external_principal_live: u64,
    pub external_donations_live: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentBook {
    pub records: [RentRecord; MAX_RENTS],
}

impl RentBook {
    pub const fn empty() -> Self {
        Self {
            records: [RentRecord::EMPTY; MAX_RENTS],
        }
    }

    pub fn insert(&mut self, record: RentRecord) -> Result<()> {
        record.validate()?;
        let index = record.role.index();
        if index >= MAX_RENTS || self.records[index] != RentRecord::EMPTY {
            return Err(Error::Occupied);
        }
        self.records[index] = record;
        self.validate()
    }

    pub fn get(&self, role: Role) -> Result<RentRecord> {
        let index = role.index();
        if index >= MAX_RENTS || !self.records[index].present {
            return Err(Error::Missing);
        }
        Ok(self.records[index])
    }

    pub fn get_mut(&mut self, role: Role) -> Result<&mut RentRecord> {
        let index = role.index();
        if index >= MAX_RENTS || !self.records[index].present {
            return Err(Error::Missing);
        }
        Ok(&mut self.records[index])
    }

    pub fn validate(&self) -> Result<()> {
        let mut index = 0_usize;
        while index < MAX_RENTS {
            let record = self.records[index];
            record.validate()?;
            if record.present && record.role.index() != index {
                return Err(Error::NonCanonical);
            }
            index += 1;
        }
        Ok(())
    }

    pub fn summary(&self) -> Result<RentSummary> {
        self.validate()?;
        let mut summary = RentSummary {
            closed_role_bits: 0,
            permanent_role_bits: 0,
            open_external_role_bits: 0,
            principal_refunded: 0,
            donations_sunk: 0,
            permanent_principal: 0,
            permanent_donations: 0,
            external_principal_live: 0,
            external_donations_live: 0,
        };
        let mut index = 0_usize;
        while index < MAX_RENTS {
            let record = self.records[index];
            if record.present {
                let bit = 1_u32.checked_shl(index as u32).ok_or(Error::Arithmetic)?;
                match record.class {
                    AccountClass::RefundableTransient => {
                        if record.closed {
                            summary.closed_role_bits |= bit;
                            summary.principal_refunded = summary
                                .principal_refunded
                                .checked_add(record.refund_paid)
                                .ok_or(Error::Arithmetic)?;
                            summary.donations_sunk = summary
                                .donations_sunk
                                .checked_add(record.sink_paid)
                                .ok_or(Error::Arithmetic)?;
                        }
                    }
                    AccountClass::PermanentInfra | AccountClass::PermanentTombstone => {
                        summary.permanent_role_bits |= bit;
                        summary.permanent_principal = summary
                            .permanent_principal
                            .checked_add(record.principal)
                            .ok_or(Error::Arithmetic)?;
                        summary.permanent_donations = summary
                            .permanent_donations
                            .checked_add(record.donations)
                            .ok_or(Error::Arithmetic)?;
                    }
                    AccountClass::ExternalOwnerState => {
                        if record.closed {
                            summary.closed_role_bits |= bit;
                            summary.principal_refunded = summary
                                .principal_refunded
                                .checked_add(record.refund_paid)
                                .ok_or(Error::Arithmetic)?;
                            summary.donations_sunk = summary
                                .donations_sunk
                                .checked_add(record.sink_paid)
                                .ok_or(Error::Arithmetic)?;
                        } else {
                            summary.open_external_role_bits |= bit;
                            summary.external_principal_live = summary
                                .external_principal_live
                                .checked_add(record.principal)
                                .ok_or(Error::Arithmetic)?;
                            summary.external_donations_live = summary
                                .external_donations_live
                                .checked_add(record.donations)
                                .ok_or(Error::Arithmetic)?;
                        }
                    }
                    AccountClass::UnclassifiedStop => return Err(Error::Rent),
                }
            }
            index += 1;
        }
        Ok(summary)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeeperBudget {
    pub payer: Id,
    pub refund_to: Id,
    pub sink: Id,
    pub deposit: u64,
    pub donations: u64,
    pub balance: u64,
    pub rewards_paid: u64,
    pub refund_paid: u64,
    pub sink_paid: u64,
    pub closed: bool,
}

impl KeeperBudget {
    pub fn new(payer: Id, refund_to: Id, sink: Id, deposit: u64) -> Result<Self> {
        if payer == ZERO_ID
            || refund_to == ZERO_ID
            || sink == ZERO_ID
            || refund_to == sink
            || deposit == 0
            || deposit > MAX_ATOMS
        {
            return Err(Error::Shape);
        }
        Ok(Self {
            payer,
            refund_to,
            sink,
            deposit,
            donations: 0,
            balance: deposit,
            rewards_paid: 0,
            refund_paid: 0,
            sink_paid: 0,
            closed: false,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.payer == ZERO_ID
            || self.refund_to == ZERO_ID
            || self.sink == ZERO_ID
            || self.refund_to == self.sink
            || self.deposit == 0
            || self.rewards_paid > self.deposit
        {
            return Err(Error::Invariant);
        }
        let ingress = self
            .deposit
            .checked_add(self.donations)
            .ok_or(Error::Arithmetic)?;
        let disposition = self
            .balance
            .checked_add(self.rewards_paid)
            .and_then(|value| value.checked_add(self.refund_paid))
            .and_then(|value| value.checked_add(self.sink_paid))
            .ok_or(Error::Arithmetic)?;
        if ingress != disposition || ingress > MAX_ATOMS {
            return Err(Error::Invariant);
        }
        if self.closed {
            if self.balance != 0
                || self.refund_paid != self.deposit - self.rewards_paid
                || self.sink_paid != self.donations
            {
                return Err(Error::Invariant);
            }
        } else if self.refund_paid != 0 || self.sink_paid != 0 {
            return Err(Error::Invariant);
        }
        Ok(())
    }

    pub fn pay_reward(&mut self, amount: u64) -> Result<()> {
        self.validate()?;
        if self.closed || amount == 0 || amount > self.deposit - self.rewards_paid {
            return Err(Error::Insufficient);
        }
        let mut next = *self;
        next.balance = next
            .balance
            .checked_sub(amount)
            .ok_or(Error::Insufficient)?;
        next.rewards_paid = next
            .rewards_paid
            .checked_add(amount)
            .ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn donate(&mut self, amount: u64) -> Result<()> {
        self.validate()?;
        if self.closed || amount == 0 {
            return Err(Error::Shape);
        }
        let mut next = *self;
        next.donations = next
            .donations
            .checked_add(amount)
            .ok_or(Error::Arithmetic)?;
        next.balance = next.balance.checked_add(amount).ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.validate()?;
        if self.closed {
            return Err(Error::Replay);
        }
        let mut next = *self;
        next.refund_paid = next.deposit - next.rewards_paid;
        next.sink_paid = next.donations;
        next.balance = 0;
        next.closed = true;
        next.validate()?;
        *self = next;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub present: bool,
    pub closed: bool,
    pub holder: Id,
    pub claims: [u64; MAX_OUTCOMES],
}

impl Position {
    pub const EMPTY: Self = Self {
        present: false,
        closed: false,
        holder: ZERO_ID,
        claims: [0; MAX_OUTCOMES],
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerAccount {
    pub present: bool,
    pub token_account: Id,
    pub owner: Id,
    pub outcome: u8,
    pub amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedBearer {
    pub present: bool,
    pub token_account: Id,
    pub amount: u64,
}

impl ObservedBearer {
    pub const EMPTY: Self = Self {
        present: false,
        token_account: ZERO_ID,
        amount: 0,
    };
}

impl BearerAccount {
    pub const EMPTY: Self = Self {
        present: false,
        token_account: ZERO_ID,
        owner: ZERO_ID,
        outcome: 0,
        amount: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditAccount {
    pub present: bool,
    pub closed: bool,
    pub owner: Id,
    pub numerator: u64,
}

impl CreditAccount {
    pub const EMPTY: Self = Self {
        present: false,
        closed: false,
        owner: ZERO_ID,
        numerator: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintState {
    pub present: bool,
    pub version: MintVersion,
    pub mint: Id,
    pub close_authority: Option<Id>,
    pub authoritative_supply: u64,
    pub closed: bool,
}

impl MintState {
    pub const EMPTY: Self = Self {
        present: false,
        version: MintVersion::LegacyNoClose,
        mint: ZERO_ID,
        close_authority: None,
        authoritative_supply: 0,
        closed: false,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupplyTruth {
    pub internal: u64,
    pub external: u64,
    pub redeemed: u64,
    pub direct_burned: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoardLedger {
    pub balance: u64,
    pub issuance_in: u64,
    pub donations_in: u64,
    pub redemption_out: u64,
    pub credit_vault_out: u64,
    pub surplus_sink_out: u64,
}

impl HoardLedger {
    fn validate(&self) -> Result<()> {
        let ingress = self
            .issuance_in
            .checked_add(self.donations_in)
            .ok_or(Error::Arithmetic)?;
        let disposition = self
            .balance
            .checked_add(self.redemption_out)
            .and_then(|value| value.checked_add(self.credit_vault_out))
            .and_then(|value| value.checked_add(self.surplus_sink_out))
            .ok_or(Error::Arithmetic)?;
        if ingress != disposition || ingress > MAX_ATOMS {
            return Err(Error::Invariant);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditVault {
    pub sealed: bool,
    pub nonce: u64,
    pub balance: u64,
    pub ingress: u64,
    pub payout_out: u64,
    pub forfeiture_sink_out: u64,
    pub donation_balance: u64,
    pub donations_in: u64,
    pub donation_sink_out: u64,
    pub credit_numerator_total: u128,
    pub forfeited_numerator: u128,
    pub terminal_rent_snapshot: Option<RentSummary>,
    pub terminal_market_nonce: u64,
}

impl CreditVault {
    fn validate(&self, denominator: u64) -> Result<()> {
        if !self.sealed {
            if *self
                != (Self {
                    sealed: false,
                    nonce: 0,
                    balance: 0,
                    ingress: 0,
                    payout_out: 0,
                    forfeiture_sink_out: 0,
                    donation_balance: self.donation_balance,
                    donations_in: self.donations_in,
                    donation_sink_out: self.donation_sink_out,
                    credit_numerator_total: 0,
                    forfeited_numerator: 0,
                    terminal_rent_snapshot: None,
                    terminal_market_nonce: 0,
                })
            {
                return Err(Error::NonCanonical);
            }
            if self.donations_in
                != self
                    .donation_balance
                    .checked_add(self.donation_sink_out)
                    .ok_or(Error::Arithmetic)?
                || self.donations_in > MAX_ATOMS
            {
                return Err(Error::Invariant);
            }
            return Ok(());
        }
        if denominator == 0 {
            return Err(Error::Invariant);
        }
        let disposition = self
            .balance
            .checked_add(self.payout_out)
            .and_then(|value| value.checked_add(self.forfeiture_sink_out))
            .ok_or(Error::Arithmetic)?;
        if disposition != self.ingress
            || self.ingress > MAX_ATOMS
            || u128::from(self.balance)
                .checked_mul(u128::from(denominator))
                .ok_or(Error::Arithmetic)?
                < self.credit_numerator_total
        {
            return Err(Error::Invariant);
        }
        if self.donations_in
            != self
                .donation_balance
                .checked_add(self.donation_sink_out)
                .ok_or(Error::Arithmetic)?
            || self
                .ingress
                .checked_add(self.donations_in)
                .ok_or(Error::Arithmetic)?
                > MAX_ATOMS
        {
            return Err(Error::Invariant);
        }
        let slack = u128::from(self.balance)
            .checked_mul(u128::from(denominator))
            .and_then(|value| value.checked_sub(self.credit_numerator_total))
            .ok_or(Error::Invariant)?;
        if slack >= u128::from(denominator) {
            return Err(Error::Invariant);
        }
        Ok(())
    }

    pub fn rounding_slack_numerator(&self, denominator: u64) -> Result<u128> {
        self.validate(denominator)?;
        u128::from(self.balance)
            .checked_mul(u128::from(denominator))
            .and_then(|value| value.checked_sub(self.credit_numerator_total))
            .ok_or(Error::Invariant)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resolution {
    pub denominator: u64,
    pub weights: [u64; MAX_OUTCOMES],
    pub receipt: Id,
}

impl Resolution {
    fn validate(&self, outcomes: u8) -> Result<()> {
        if self.denominator == 0 || self.denominator > MAX_ATOMS || self.receipt == ZERO_ID {
            return Err(Error::Shape);
        }
        let mut sum = 0_u64;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            if index < usize::from(outcomes) {
                if self.weights[index] > self.denominator {
                    return Err(Error::Shape);
                }
                sum = sum
                    .checked_add(self.weights[index])
                    .ok_or(Error::Arithmetic)?;
            } else if self.weights[index] != 0 {
                return Err(Error::NonCanonical);
            }
            index += 1;
        }
        if sum != self.denominator {
            return Err(Error::Shape);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreationConfig {
    pub market: Id,
    pub generation: u64,
    pub terminal_authority: Id,
    pub rent_payer: Id,
    pub rent_refund_to: Id,
    pub neutral_sink: Id,
    pub outcomes: u8,
    pub refundable_rent_principal: u64,
    pub credit_vault_rent_principal: u64,
    pub replay_rent_principal: u64,
    pub keeper_budget: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditRentFunding {
    pub payer: Id,
    pub refund_to: Id,
    pub principal: u64,
    pub prefund_donation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tombstone {
    pub present: bool,
    pub market: Id,
    pub generation: u64,
    pub outcomes: u8,
    pub terminal_receipt: Id,
    pub final_market_nonce: u64,
    pub replay_account: Id,
    pub credit_vault_account: Id,
    pub rent: RentSummary,
    pub keeper_deposit: u64,
    pub keeper_rewards_paid: u64,
    pub keeper_refund_paid: u64,
    pub keeper_donations_sunk: u64,
}

impl Tombstone {
    pub const EMPTY: Self = Self {
        present: false,
        market: ZERO_ID,
        generation: 0,
        outcomes: 0,
        terminal_receipt: ZERO_ID,
        final_market_nonce: 0,
        replay_account: ZERO_ID,
        credit_vault_account: ZERO_ID,
        rent: RentSummary {
            closed_role_bits: 0,
            permanent_role_bits: 0,
            open_external_role_bits: 0,
            principal_refunded: 0,
            donations_sunk: 0,
            permanent_principal: 0,
            permanent_donations: 0,
            external_principal_live: 0,
            external_donations_live: 0,
        },
        keeper_deposit: 0,
        keeper_rewards_paid: 0,
        keeper_refund_paid: 0,
        keeper_donations_sunk: 0,
    };

    pub fn validate(&self) -> Result<()> {
        if !self.present
            || self.market == ZERO_ID
            || self.generation == 0
            || !(2..=MAX_OUTCOMES as u8).contains(&self.outcomes)
            || self.terminal_receipt == ZERO_ID
            || self.replay_account == ZERO_ID
            || self.credit_vault_account == ZERO_ID
            || self.replay_account == self.credit_vault_account
        {
            return Err(Error::Identity);
        }
        let valid_bits = (1_u32 << MAX_RENTS) - 1;
        let occupied = self.rent.closed_role_bits
            | self.rent.permanent_role_bits
            | self.rent.open_external_role_bits;
        let required_mint_bits = ((1_u32 << self.outcomes) - 1) << 8;
        let all_mint_bits = ((1_u32 << MAX_OUTCOMES) - 1) << 8;
        let inactive_mint_bits = all_mint_bits & !required_mint_bits;
        let position_bits = ((1_u32 << MAX_POSITIONS) - 1) << 4;
        let credit_bits = ((1_u32 << MAX_CREDITS) - 1) << 13;
        let required_permanent_bits =
            (1_u32 << Role::CreditVault.index()) | (1_u32 << Role::Replay.index());
        let allowed_closed_bits = 0b1111 | position_bits | required_mint_bits;
        if occupied & !valid_bits != 0
            || self.rent.closed_role_bits & self.rent.permanent_role_bits != 0
            || self.rent.closed_role_bits & self.rent.open_external_role_bits != 0
            || self.rent.permanent_role_bits & self.rent.open_external_role_bits != 0
            || self.rent.closed_role_bits & 0b1111 != 0b1111
            || self.rent.closed_role_bits & required_mint_bits != required_mint_bits
            || occupied & inactive_mint_bits != 0
            || self.rent.closed_role_bits & !allowed_closed_bits != 0
            || self.rent.permanent_role_bits != required_permanent_bits
            || self.rent.open_external_role_bits & !credit_bits != 0
            || self.keeper_deposit
                != self
                    .keeper_rewards_paid
                    .checked_add(self.keeper_refund_paid)
                    .ok_or(Error::Arithmetic)?
            || self
                .keeper_deposit
                .checked_add(self.keeper_donations_sunk)
                .ok_or(Error::Arithmetic)?
                > MAX_ATOMS
        {
            return Err(Error::Invariant);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Registry {
    pub active_market: Id,
    pub active_generation: u64,
    pub tombstone: Tombstone,
}

impl Registry {
    pub const fn empty() -> Self {
        Self {
            active_market: ZERO_ID,
            active_generation: 0,
            tombstone: Tombstone::EMPTY,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedemptionEffect {
    pub paid_atoms: u64,
    pub credit_numerator: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Model {
    pub market: Id,
    pub generation: u64,
    pub market_nonce: u64,
    pub terminal_authority: Id,
    pub rent_refund_to: Id,
    pub neutral_sink: Id,
    pub outcomes: u8,
    pub phase: Phase,
    pub resolution: Option<Resolution>,
    pub supply: [SupplyTruth; MAX_OUTCOMES],
    pub mints: [MintState; MAX_OUTCOMES],
    pub positions: [Position; MAX_POSITIONS],
    pub bearers: [BearerAccount; MAX_BEARERS],
    pub credits: [CreditAccount; MAX_CREDITS],
    pub credit_numerator_total: u128,
    pub hoard: HoardLedger,
    pub credit_vault: CreditVault,
    pub rents: RentBook,
    pub keeper: KeeperBudget,
}

impl Model {
    pub fn new(registry: &mut Registry, config: CreationConfig) -> Result<Self> {
        if registry.tombstone.present
            || registry.active_market != ZERO_ID
            || registry.active_generation != 0
        {
            return Err(Error::Replay);
        }
        if config.market == ZERO_ID
            || config.generation == 0
            || config.terminal_authority == ZERO_ID
            || config.rent_payer == ZERO_ID
            || config.rent_refund_to == ZERO_ID
            || config.neutral_sink == ZERO_ID
            || config.market == config.terminal_authority
            || config.market == config.rent_refund_to
            || config.market == config.neutral_sink
            || config.terminal_authority == config.rent_refund_to
            || config.terminal_authority == config.neutral_sink
            || config.rent_refund_to == config.neutral_sink
            || !(2..=MAX_OUTCOMES as u8).contains(&config.outcomes)
        {
            return Err(Error::Shape);
        }
        let mut role_index = 0_usize;
        let roles = [
            Role::Market,
            Role::Hoard,
            Role::Supply,
            Role::Resolution,
            Role::Mint(0),
            Role::Mint(1),
            Role::Mint(2),
            Role::Mint(3),
            Role::CreditVault,
            Role::Replay,
        ];
        while role_index < roles.len() {
            let role = roles[role_index];
            let active_role = !matches!(role, Role::Mint(outcome) if outcome >= config.outcomes);
            if active_role {
                let account = role_account(config.market, role);
                if account == config.terminal_authority
                    || account == config.rent_refund_to
                    || account == config.neutral_sink
                {
                    return Err(Error::Identity);
                }
            }
            role_index += 1;
        }
        let mut rents = RentBook::empty();
        for role in [Role::Market, Role::Hoard, Role::Supply, Role::Resolution] {
            rents.insert(RentRecord::new(
                role,
                AccountClass::RefundableTransient,
                config.market,
                config.generation,
                config.rent_payer,
                config.rent_refund_to,
                config.neutral_sink,
                config.refundable_rent_principal,
                0,
            )?)?;
        }
        rents.insert(RentRecord::new(
            Role::CreditVault,
            AccountClass::PermanentInfra,
            config.market,
            config.generation,
            config.rent_payer,
            config.rent_refund_to,
            config.neutral_sink,
            config.credit_vault_rent_principal,
            0,
        )?)?;
        rents.insert(RentRecord::new(
            Role::Replay,
            AccountClass::PermanentTombstone,
            config.market,
            config.generation,
            config.rent_payer,
            config.rent_refund_to,
            config.neutral_sink,
            config.replay_rent_principal,
            0,
        )?)?;
        let mut mints = [MintState::EMPTY; MAX_OUTCOMES];
        let mut outcome = 0_usize;
        while outcome < usize::from(config.outcomes) {
            let role = Role::Mint(outcome as u8);
            rents.insert(RentRecord::new(
                role,
                AccountClass::RefundableTransient,
                config.market,
                config.generation,
                config.rent_payer,
                config.rent_refund_to,
                config.neutral_sink,
                config.refundable_rent_principal,
                0,
            )?)?;
            mints[outcome] = MintState {
                present: true,
                version: MintVersion::R4Closeable,
                mint: role_account(config.market, role),
                close_authority: Some(config.terminal_authority),
                authoritative_supply: 0,
                closed: false,
            };
            outcome += 1;
        }
        let model = Self {
            market: config.market,
            generation: config.generation,
            market_nonce: 0,
            terminal_authority: config.terminal_authority,
            rent_refund_to: config.rent_refund_to,
            neutral_sink: config.neutral_sink,
            outcomes: config.outcomes,
            phase: Phase::Active,
            resolution: None,
            supply: [SupplyTruth {
                internal: 0,
                external: 0,
                redeemed: 0,
                direct_burned: 0,
            }; MAX_OUTCOMES],
            mints,
            positions: [Position::EMPTY; MAX_POSITIONS],
            bearers: [BearerAccount::EMPTY; MAX_BEARERS],
            credits: [CreditAccount::EMPTY; MAX_CREDITS],
            credit_numerator_total: 0,
            hoard: HoardLedger {
                balance: 0,
                issuance_in: 0,
                donations_in: 0,
                redemption_out: 0,
                credit_vault_out: 0,
                surplus_sink_out: 0,
            },
            credit_vault: CreditVault {
                sealed: false,
                nonce: 0,
                balance: 0,
                ingress: 0,
                payout_out: 0,
                forfeiture_sink_out: 0,
                donation_balance: 0,
                donations_in: 0,
                donation_sink_out: 0,
                credit_numerator_total: 0,
                forfeited_numerator: 0,
                terminal_rent_snapshot: None,
                terminal_market_nonce: 0,
            },
            rents,
            keeper: KeeperBudget::new(
                config.rent_payer,
                config.rent_refund_to,
                config.neutral_sink,
                config.keeper_budget,
            )?,
        };
        model.validate()?;
        registry.active_market = config.market;
        registry.active_generation = config.generation;
        Ok(model)
    }

    pub fn validate(&self) -> Result<()> {
        if self.market == ZERO_ID
            || self.generation == 0
            || self.terminal_authority == ZERO_ID
            || self.rent_refund_to == ZERO_ID
            || self.neutral_sink == ZERO_ID
            || self.terminal_authority == self.rent_refund_to
            || self.terminal_authority == self.neutral_sink
            || self.rent_refund_to == self.neutral_sink
            || !(2..=MAX_OUTCOMES as u8).contains(&self.outcomes)
        {
            return Err(Error::Identity);
        }
        self.rents.validate()?;
        let mut rent_index = 0_usize;
        while rent_index < MAX_RENTS {
            let rent = self.rents.records[rent_index];
            if rent.present
                && (rent.market != self.market
                    || rent.generation != self.generation
                    || rent.sink != self.neutral_sink)
            {
                return Err(Error::Rent);
            }
            if rent.present
                && matches!(
                    rent.role,
                    Role::Market
                        | Role::Hoard
                        | Role::Supply
                        | Role::Resolution
                        | Role::Mint(_)
                        | Role::CreditVault
                        | Role::Replay
                )
                && rent.refund_to != self.rent_refund_to
            {
                return Err(Error::Rent);
            }
            rent_index += 1;
        }
        self.keeper.validate()?;
        self.hoard.validate()?;
        if self.credit_vault.ingress != self.hoard.credit_vault_out {
            return Err(Error::Invariant);
        }
        let count = usize::from(self.outcomes);
        let mut internal = [0_u64; MAX_OUTCOMES];
        let mut slot = 0_usize;
        while slot < MAX_POSITIONS {
            let position = self.positions[slot];
            if position.present {
                if position.closed || position.holder == ZERO_ID {
                    return Err(Error::NonCanonical);
                }
                let rent = self.rents.get(Role::Position(slot as u8))?;
                if rent.closed
                    || rent.class != AccountClass::RefundableTransient
                    || rent.refund_to == self.neutral_sink
                {
                    return Err(Error::Rent);
                }
                let mut outcome = 0_usize;
                while outcome < count {
                    internal[outcome] = internal[outcome]
                        .checked_add(position.claims[outcome])
                        .ok_or(Error::Arithmetic)?;
                    outcome += 1;
                }
                while outcome < MAX_OUTCOMES {
                    if position.claims[outcome] != 0 {
                        return Err(Error::NonCanonical);
                    }
                    outcome += 1;
                }
            } else if position == Position::EMPTY {
                if self.rents.records[Role::Position(slot as u8).index()].present {
                    return Err(Error::Rent);
                }
            } else if !position.closed
                || position.holder == ZERO_ID
                || position.claims != [0; MAX_OUTCOMES]
                || !self.rents.get(Role::Position(slot as u8))?.closed
                || self.rents.get(Role::Position(slot as u8))?.class
                    != AccountClass::RefundableTransient
            {
                return Err(Error::NonCanonical);
            }
            slot += 1;
        }
        let mut external = [0_u64; MAX_OUTCOMES];
        slot = 0;
        while slot < MAX_BEARERS {
            let bearer = self.bearers[slot];
            if bearer.present {
                if bearer.token_account == ZERO_ID
                    || bearer.owner == ZERO_ID
                    || usize::from(bearer.outcome) >= count
                {
                    return Err(Error::Identity);
                }
                let mut prior = 0_usize;
                while prior < slot {
                    if self.bearers[prior].present
                        && self.bearers[prior].token_account == bearer.token_account
                    {
                        return Err(Error::Identity);
                    }
                    prior += 1;
                }
                let index = usize::from(bearer.outcome);
                external[index] = external[index]
                    .checked_add(bearer.amount)
                    .ok_or(Error::Arithmetic)?;
            } else if bearer != BearerAccount::EMPTY {
                return Err(Error::NonCanonical);
            }
            slot += 1;
        }
        let mut observed_credit = 0_u128;
        slot = 0;
        while slot < MAX_CREDITS {
            let credit = self.credits[slot];
            if credit.present {
                if credit.closed || credit.owner == ZERO_ID {
                    return Err(Error::NonCanonical);
                }
                let denominator = self.resolution.ok_or(Error::Phase)?.denominator;
                if credit.numerator >= denominator {
                    return Err(Error::Invariant);
                }
                let rent = self.rents.get(Role::Credit(slot as u8))?;
                if rent.closed || rent.class != AccountClass::ExternalOwnerState {
                    return Err(Error::Rent);
                }
                let mut prior = 0_usize;
                while prior < slot {
                    if self.credits[prior].present && self.credits[prior].owner == credit.owner {
                        return Err(Error::Identity);
                    }
                    prior += 1;
                }
                observed_credit = observed_credit
                    .checked_add(u128::from(credit.numerator))
                    .ok_or(Error::Arithmetic)?;
            } else if credit == CreditAccount::EMPTY {
                if self.rents.records[Role::Credit(slot as u8).index()].present {
                    return Err(Error::Rent);
                }
            } else if !credit.closed
                || credit.owner == ZERO_ID
                || credit.numerator != 0
                || !self.rents.get(Role::Credit(slot as u8))?.closed
            {
                return Err(Error::NonCanonical);
            }
            slot += 1;
        }
        let active_credit_total = if self.credit_vault.sealed {
            self.credit_vault.credit_numerator_total
        } else {
            self.credit_numerator_total
        };
        if observed_credit != active_credit_total {
            return Err(Error::Invariant);
        }
        let mut outcome = 0_usize;
        while outcome < MAX_OUTCOMES {
            if outcome < count {
                if self.supply[outcome].internal != internal[outcome]
                    || self.supply[outcome].external != external[outcome]
                    || !self.mints[outcome].present
                    || self.mints[outcome].mint
                        != role_account(self.market, Role::Mint(outcome as u8))
                    || self.mints[outcome].version != MintVersion::R4Closeable
                    || self.mints[outcome].close_authority != Some(self.terminal_authority)
                    || self.mints[outcome].authoritative_supply != external[outcome]
                {
                    return Err(Error::Invariant);
                }
                if self.mints[outcome].closed && external[outcome] != 0 {
                    return Err(Error::OutstandingClaims);
                }
                let raw_disposition = self.supply[outcome]
                    .internal
                    .checked_add(self.supply[outcome].external)
                    .and_then(|value| value.checked_add(self.supply[outcome].redeemed))
                    .and_then(|value| value.checked_add(self.supply[outcome].direct_burned))
                    .ok_or(Error::Arithmetic)?;
                if raw_disposition != self.hoard.issuance_in {
                    return Err(Error::Invariant);
                }
                let mint_rent = self.rents.get(Role::Mint(outcome as u8))?;
                if mint_rent.class != AccountClass::RefundableTransient
                    || mint_rent.closed != self.mints[outcome].closed
                {
                    return Err(Error::Rent);
                }
            } else if self.supply[outcome]
                != (SupplyTruth {
                    internal: 0,
                    external: 0,
                    redeemed: 0,
                    direct_burned: 0,
                })
                || self.mints[outcome] != MintState::EMPTY
            {
                return Err(Error::NonCanonical);
            }
            outcome += 1;
        }
        for role in [Role::Market, Role::Hoard, Role::Supply, Role::Resolution] {
            let rent = self.rents.get(role)?;
            if rent.class != AccountClass::RefundableTransient
                || rent.closed != (self.phase == Phase::Terminal)
            {
                return Err(Error::Rent);
            }
        }
        let vault_rent = self.rents.get(Role::CreditVault)?;
        let replay_rent = self.rents.get(Role::Replay)?;
        if vault_rent.class != AccountClass::PermanentInfra
            || vault_rent.closed
            || replay_rent.class != AccountClass::PermanentTombstone
            || replay_rent.closed
        {
            return Err(Error::Rent);
        }
        if self.keeper.closed != (self.phase == Phase::Terminal) {
            return Err(Error::Invariant);
        }
        if self.credit_vault.sealed {
            if self.credit_numerator_total != 0 {
                return Err(Error::Invariant);
            }
        } else if self.credit_vault.credit_numerator_total != 0 {
            return Err(Error::Invariant);
        }
        let denominator = self
            .resolution
            .map_or(1, |resolution| resolution.denominator);
        self.credit_vault.validate(denominator)?;
        match (self.phase, self.resolution) {
            (Phase::Active, None) => {
                if self.credit_numerator_total != 0 || self.credit_vault.sealed {
                    return Err(Error::Phase);
                }
            }
            (Phase::Resolved | Phase::CreditsSealed | Phase::Terminal, Some(resolution)) => {
                resolution.validate(self.outcomes)?;
                self.validate_resolved_conservation(resolution)?;
                if self.phase == Phase::Resolved && self.credit_vault.sealed {
                    return Err(Error::Phase);
                }
                if matches!(self.phase, Phase::CreditsSealed | Phase::Terminal)
                    && !self.credit_vault.sealed
                {
                    return Err(Error::Phase);
                }
                if self.phase == Phase::Terminal && self.hoard.balance != 0 {
                    return Err(Error::Invariant);
                }
                if self.phase == Phase::Terminal {
                    if self.credit_vault.terminal_rent_snapshot.is_none()
                        || self.credit_vault.terminal_market_nonce != self.market_nonce
                    {
                        return Err(Error::Invariant);
                    }
                } else if self.credit_vault.terminal_rent_snapshot.is_some()
                    || self.credit_vault.terminal_market_nonce != 0
                {
                    return Err(Error::Invariant);
                }
            }
            _ => return Err(Error::Phase),
        }
        Ok(())
    }

    fn validate_resolved_conservation(&self, resolution: Resolution) -> Result<()> {
        let denominator = u128::from(resolution.denominator);
        let mut claims = 0_u128;
        let mut burned = 0_u128;
        let mut outcome = 0_usize;
        while outcome < usize::from(self.outcomes) {
            let remaining = self.supply[outcome]
                .internal
                .checked_add(self.supply[outcome].external)
                .ok_or(Error::Arithmetic)?;
            claims = claims
                .checked_add(
                    u128::from(remaining)
                        .checked_mul(u128::from(resolution.weights[outcome]))
                        .ok_or(Error::Arithmetic)?,
                )
                .ok_or(Error::Arithmetic)?;
            burned = burned
                .checked_add(
                    u128::from(self.supply[outcome].direct_burned)
                        .checked_mul(u128::from(resolution.weights[outcome]))
                        .ok_or(Error::Arithmetic)?,
                )
                .ok_or(Error::Arithmetic)?;
            outcome += 1;
        }
        let credit = if self.credit_vault.sealed {
            self.credit_vault.credit_numerator_total
        } else {
            self.credit_numerator_total
        };
        let issued = u128::from(self.hoard.issuance_in)
            .checked_mul(denominator)
            .ok_or(Error::Arithmetic)?;
        let paid = u128::from(
            self.hoard
                .redemption_out
                .checked_add(self.credit_vault.payout_out)
                .ok_or(Error::Arithmetic)?,
        )
        .checked_mul(denominator)
        .ok_or(Error::Arithmetic)?;
        let rights = claims
            .checked_add(credit)
            .and_then(|value| value.checked_add(paid))
            .and_then(|value| value.checked_add(burned))
            .and_then(|value| value.checked_add(self.credit_vault.forfeited_numerator))
            .ok_or(Error::Arithmetic)?;
        if issued != rights {
            return Err(Error::Invariant);
        }
        if self.credit_vault.sealed {
            if claims != 0
                || u128::from(self.credit_vault.balance)
                    .checked_mul(denominator)
                    .ok_or(Error::Arithmetic)?
                    < credit
            {
                return Err(Error::OutstandingClaims);
            }
        } else {
            let available = u128::from(self.hoard.balance)
                .checked_mul(denominator)
                .ok_or(Error::Arithmetic)?;
            let required = claims.checked_add(credit).ok_or(Error::Arithmetic)?;
            if available < required {
                return Err(Error::Insufficient);
            }
        }
        Ok(())
    }

    fn transact<F>(&mut self, expected_nonce: u64, action: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        self.validate()?;
        if self.phase == Phase::Terminal || expected_nonce != self.market_nonce {
            return Err(Error::Replay);
        }
        let mut next = *self;
        action(&mut next)?;
        next.market_nonce = next.market_nonce.checked_add(1).ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn issue_position(
        &mut self,
        expected_nonce: u64,
        slot: usize,
        holder: Id,
        quantity: u64,
        rent_payer: Id,
        rent_refund_to: Id,
        rent_principal: u64,
        prefund_donation: u64,
    ) -> Result<()> {
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Active
                || slot >= MAX_POSITIONS
                || next.positions[slot] != Position::EMPTY
                || holder == ZERO_ID
                || quantity == 0
            {
                return Err(Error::Shape);
            }
            next.rents.insert(RentRecord::new(
                Role::Position(slot as u8),
                AccountClass::RefundableTransient,
                next.market,
                next.generation,
                rent_payer,
                rent_refund_to,
                next.neutral_sink,
                rent_principal,
                prefund_donation,
            )?)?;
            let mut claims = [0_u64; MAX_OUTCOMES];
            let mut outcome = 0_usize;
            while outcome < usize::from(next.outcomes) {
                claims[outcome] = quantity;
                next.supply[outcome].internal = next.supply[outcome]
                    .internal
                    .checked_add(quantity)
                    .ok_or(Error::Arithmetic)?;
                outcome += 1;
            }
            next.positions[slot] = Position {
                present: true,
                closed: false,
                holder,
                claims,
            };
            next.hoard.balance = next
                .hoard
                .balance
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.hoard.issuance_in = next
                .hoard
                .issuance_in
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            Ok(())
        })
    }

    pub fn donate_collateral(&mut self, expected_nonce: u64, quantity: u64) -> Result<()> {
        self.transact(expected_nonce, |next| {
            if quantity == 0 {
                return Err(Error::Shape);
            }
            next.hoard.balance = next
                .hoard
                .balance
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.hoard.donations_in = next
                .hoard
                .donations_in
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            Ok(())
        })
    }

    /// Reconciles unsolicited collateral observed in the permanent CreditVault
    /// before market-graph close. It creates no credit or payout right.
    pub fn reconcile_credit_vault_donation(
        &mut self,
        expected_nonce: u64,
        quantity: u64,
    ) -> Result<()> {
        self.transact(expected_nonce, |next| {
            if quantity == 0 {
                return Err(Error::Shape);
            }
            next.credit_vault.donation_balance = next
                .credit_vault
                .donation_balance
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.credit_vault.donations_in = next
                .credit_vault
                .donations_in
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn materialize(
        &mut self,
        expected_nonce: u64,
        position_slot: usize,
        bearer_slot: usize,
        holder: Id,
        token_account: Id,
        outcome: u8,
        quantity: u64,
    ) -> Result<()> {
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Active
                || position_slot >= MAX_POSITIONS
                || bearer_slot >= MAX_BEARERS
                || holder == ZERO_ID
                || token_account == ZERO_ID
                || quantity == 0
                || usize::from(outcome) >= usize::from(next.outcomes)
            {
                return Err(Error::Shape);
            }
            let position = &mut next.positions[position_slot];
            if !position.present
                || position.holder != holder
                || position.claims[usize::from(outcome)] < quantity
            {
                return Err(Error::Authority);
            }
            let bearer = next.bearers[bearer_slot];
            if bearer.present
                && (bearer.token_account != token_account
                    || bearer.owner != holder
                    || bearer.outcome != outcome)
            {
                return Err(Error::Identity);
            }
            position.claims[usize::from(outcome)] -= quantity;
            next.supply[usize::from(outcome)].internal -= quantity;
            next.supply[usize::from(outcome)].external = next.supply[usize::from(outcome)]
                .external
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.mints[usize::from(outcome)].authoritative_supply = next.mints
                [usize::from(outcome)]
            .authoritative_supply
            .checked_add(quantity)
            .ok_or(Error::Arithmetic)?;
            next.bearers[bearer_slot] = BearerAccount {
                present: true,
                token_account,
                owner: holder,
                outcome,
                amount: bearer
                    .amount
                    .checked_add(quantity)
                    .ok_or(Error::Arithmetic)?,
            };
            Ok(())
        })
    }

    /// Reconciles a complete per-outcome observation after unauthorized burns.
    ///
    /// The caller supplies authenticated token-account and mint post-state.
    /// The model requires the sum of all registered bearer deltas to equal the
    /// authoritative mint delta. Token-account ownership is deliberately not
    /// treated as authorization for already-observed burns.
    pub fn reconcile_observed_direct_burns(
        &mut self,
        expected_nonce: u64,
        outcome: u8,
        observed_mint_supply: u64,
        observations: [ObservedBearer; MAX_BEARERS],
    ) -> Result<()> {
        self.transact(expected_nonce, |next| {
            if usize::from(outcome) >= usize::from(next.outcomes) {
                return Err(Error::Shape);
            }
            let outcome_index = usize::from(outcome);
            let prior_mint_supply = next.mints[outcome_index].authoritative_supply;
            if observed_mint_supply >= prior_mint_supply
                || next.supply[outcome_index].external != prior_mint_supply
            {
                return Err(Error::Identity);
            }
            let mut bearer_delta_total = 0_u64;
            let mut slot = 0_usize;
            while slot < MAX_BEARERS {
                let bearer = next.bearers[slot];
                let observed = observations[slot];
                if bearer.present && bearer.outcome == outcome {
                    if !observed.present
                        || observed.token_account != bearer.token_account
                        || observed.amount > bearer.amount
                    {
                        return Err(Error::Identity);
                    }
                    bearer_delta_total = bearer_delta_total
                        .checked_add(bearer.amount - observed.amount)
                        .ok_or(Error::Arithmetic)?;
                    next.bearers[slot].amount = observed.amount;
                } else if observed != ObservedBearer::EMPTY {
                    return Err(Error::NonCanonical);
                }
                slot += 1;
            }
            let mint_delta = prior_mint_supply - observed_mint_supply;
            if bearer_delta_total != mint_delta {
                return Err(Error::Invariant);
            }
            next.supply[outcome_index].external = observed_mint_supply;
            next.supply[outcome_index].direct_burned = next.supply[outcome_index]
                .direct_burned
                .checked_add(bearer_delta_total)
                .ok_or(Error::Arithmetic)?;
            next.mints[outcome_index].authoritative_supply = observed_mint_supply;
            Ok(())
        })
    }

    pub fn resolve(&mut self, expected_nonce: u64, resolution: Resolution) -> Result<()> {
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Active {
                return Err(Error::Phase);
            }
            resolution.validate(next.outcomes)?;
            next.resolution = Some(resolution);
            next.phase = Phase::Resolved;
            Ok(())
        })
    }

    fn credit_effect(
        &mut self,
        owner: Id,
        credit_slot: usize,
        claim_numerator: u128,
        funding: CreditRentFunding,
    ) -> Result<RedemptionEffect> {
        if credit_slot >= MAX_CREDITS || owner == ZERO_ID {
            return Err(Error::Shape);
        }
        let denominator = self.resolution.ok_or(Error::Phase)?.denominator;
        let existing = self.credits[credit_slot];
        if existing.present && existing.owner != owner {
            return Err(Error::Identity);
        }
        let mut prior_slot = 0_usize;
        while prior_slot < MAX_CREDITS {
            if prior_slot != credit_slot
                && self.credits[prior_slot].present
                && self.credits[prior_slot].owner == owner
            {
                return Err(Error::Identity);
            }
            prior_slot += 1;
        }
        let prior = if existing.present {
            existing.numerator
        } else {
            0
        };
        let accumulated = u128::from(prior)
            .checked_add(claim_numerator)
            .ok_or(Error::Arithmetic)?;
        let paid =
            u64::try_from(accumulated / u128::from(denominator)).map_err(|_| Error::Arithmetic)?;
        let residue =
            u64::try_from(accumulated % u128::from(denominator)).map_err(|_| Error::Arithmetic)?;
        if paid > self.hoard.balance {
            return Err(Error::Insufficient);
        }
        if residue != 0 && !existing.present {
            self.rents.insert(RentRecord::new(
                Role::Credit(credit_slot as u8),
                AccountClass::ExternalOwnerState,
                self.market,
                self.generation,
                funding.payer,
                funding.refund_to,
                self.neutral_sink,
                funding.principal,
                funding.prefund_donation,
            )?)?;
            self.credits[credit_slot] = CreditAccount {
                present: true,
                closed: false,
                owner,
                numerator: residue,
            };
        } else if existing.present {
            self.credits[credit_slot].numerator = residue;
        }
        self.credit_numerator_total = self
            .credit_numerator_total
            .checked_sub(u128::from(prior))
            .and_then(|value| value.checked_add(u128::from(residue)))
            .ok_or(Error::Arithmetic)?;
        self.hoard.balance -= paid;
        self.hoard.redemption_out = self
            .hoard
            .redemption_out
            .checked_add(paid)
            .ok_or(Error::Arithmetic)?;
        Ok(RedemptionEffect {
            paid_atoms: paid,
            credit_numerator: residue,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn redeem_internal(
        &mut self,
        expected_nonce: u64,
        position_slot: usize,
        holder: Id,
        outcome: u8,
        quantity: u64,
        credit_slot: usize,
        funding: CreditRentFunding,
    ) -> Result<RedemptionEffect> {
        let mut effect = RedemptionEffect {
            paid_atoms: 0,
            credit_numerator: 0,
        };
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Resolved
                || position_slot >= MAX_POSITIONS
                || usize::from(outcome) >= usize::from(next.outcomes)
                || quantity == 0
            {
                return Err(Error::Shape);
            }
            let position = next.positions[position_slot];
            if !position.present
                || position.holder != holder
                || position.claims[usize::from(outcome)] < quantity
            {
                return Err(Error::Authority);
            }
            let weight = next.resolution.ok_or(Error::Phase)?.weights[usize::from(outcome)];
            effect = next.credit_effect(
                holder,
                credit_slot,
                u128::from(quantity)
                    .checked_mul(u128::from(weight))
                    .ok_or(Error::Arithmetic)?,
                funding,
            )?;
            next.positions[position_slot].claims[usize::from(outcome)] -= quantity;
            next.supply[usize::from(outcome)].internal -= quantity;
            next.supply[usize::from(outcome)].redeemed = next.supply[usize::from(outcome)]
                .redeemed
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            Ok(())
        })?;
        Ok(effect)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn redeem_external(
        &mut self,
        expected_nonce: u64,
        bearer_slot: usize,
        owner: Id,
        token_account: Id,
        quantity: u64,
        observed_bearer_amount: u64,
        observed_mint_supply: u64,
        credit_slot: usize,
        funding: CreditRentFunding,
    ) -> Result<RedemptionEffect> {
        let mut effect = RedemptionEffect {
            paid_atoms: 0,
            credit_numerator: 0,
        };
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Resolved || bearer_slot >= MAX_BEARERS || quantity == 0 {
                return Err(Error::Shape);
            }
            let bearer = next.bearers[bearer_slot];
            if !bearer.present
                || bearer.owner != owner
                || bearer.token_account != token_account
                || bearer.amount < quantity
                || bearer.amount - quantity != observed_bearer_amount
            {
                return Err(Error::Authority);
            }
            let outcome = usize::from(bearer.outcome);
            if next.mints[outcome].authoritative_supply < quantity
                || next.mints[outcome].authoritative_supply - quantity != observed_mint_supply
            {
                return Err(Error::Invariant);
            }
            let weight = next.resolution.ok_or(Error::Phase)?.weights[outcome];
            effect = next.credit_effect(
                owner,
                credit_slot,
                u128::from(quantity)
                    .checked_mul(u128::from(weight))
                    .ok_or(Error::Arithmetic)?,
                funding,
            )?;
            next.bearers[bearer_slot].amount = observed_bearer_amount;
            next.supply[outcome].external = observed_mint_supply;
            next.supply[outcome].redeemed = next.supply[outcome]
                .redeemed
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.mints[outcome].authoritative_supply = observed_mint_supply;
            Ok(())
        })?;
        Ok(effect)
    }

    pub fn close_position(
        &mut self,
        expected_nonce: u64,
        slot: usize,
        refund_to: Id,
    ) -> Result<RentCloseEffect> {
        let mut effect = RentCloseEffect {
            refund_atoms: 0,
            donation_sink_atoms: 0,
        };
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Resolved || slot >= MAX_POSITIONS {
                return Err(Error::Phase);
            }
            let position = next.positions[slot];
            if !position.present || position.claims.iter().any(|claim| *claim != 0) {
                return Err(Error::OutstandingClaims);
            }
            effect = next
                .rents
                .get_mut(Role::Position(slot as u8))?
                .close(refund_to, next.neutral_sink)?;
            next.positions[slot] = Position {
                present: false,
                closed: true,
                holder: position.holder,
                claims: [0; MAX_OUTCOMES],
            };
            Ok(())
        })?;
        Ok(effect)
    }

    pub fn close_mint(
        &mut self,
        expected_nonce: u64,
        outcome: u8,
        authority: Id,
    ) -> Result<RentCloseEffect> {
        let mut effect = RentCloseEffect {
            refund_atoms: 0,
            donation_sink_atoms: 0,
        };
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Resolved || usize::from(outcome) >= usize::from(next.outcomes) {
                return Err(Error::Phase);
            }
            let mint = next.mints[usize::from(outcome)];
            if mint.version != MintVersion::R4Closeable {
                return Err(Error::LegacyStop);
            }
            if authority == ZERO_ID
                || mint.close_authority != Some(authority)
                || authority != next.terminal_authority
            {
                return Err(Error::Authority);
            }
            if mint.authoritative_supply != 0
                || next.supply[usize::from(outcome)].external != 0
                || next.supply[usize::from(outcome)].internal != 0
            {
                return Err(Error::OutstandingClaims);
            }
            effect = next
                .rents
                .get_mut(Role::Mint(outcome))?
                .close(next.rent_refund_to, next.neutral_sink)?;
            next.mints[usize::from(outcome)].closed = true;
            Ok(())
        })?;
        Ok(effect)
    }

    pub fn seal_credit_vault(&mut self, expected_nonce: u64) -> Result<u64> {
        let mut transferred = 0_u64;
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::Resolved || next.any_claims() {
                return Err(Error::OutstandingClaims);
            }
            if next.mints[..usize::from(next.outcomes)]
                .iter()
                .any(|mint| !mint.closed)
                || next.positions.iter().any(|position| position.present)
            {
                return Err(Error::Phase);
            }
            let denominator = next.resolution.ok_or(Error::Phase)?.denominator;
            let total = next.credit_numerator_total;
            let atoms = if total == 0 {
                0
            } else {
                u64::try_from(
                    total
                        .checked_add(u128::from(denominator) - 1)
                        .ok_or(Error::Arithmetic)?
                        / u128::from(denominator),
                )
                .map_err(|_| Error::Arithmetic)?
            };
            if atoms > next.hoard.balance {
                return Err(Error::Insufficient);
            }
            next.hoard.balance -= atoms;
            next.hoard.credit_vault_out = next
                .hoard
                .credit_vault_out
                .checked_add(atoms)
                .ok_or(Error::Arithmetic)?;
            next.credit_vault = CreditVault {
                sealed: true,
                nonce: 0,
                balance: atoms,
                ingress: atoms,
                payout_out: 0,
                forfeiture_sink_out: 0,
                donation_balance: next.credit_vault.donation_balance,
                donations_in: next.credit_vault.donations_in,
                donation_sink_out: next.credit_vault.donation_sink_out,
                credit_numerator_total: total,
                forfeited_numerator: 0,
                terminal_rent_snapshot: None,
                terminal_market_nonce: 0,
            };
            next.credit_numerator_total = 0;
            next.phase = Phase::CreditsSealed;
            transferred = atoms;
            Ok(())
        })?;
        Ok(transferred)
    }

    pub fn dispose_hoard_surplus(&mut self, expected_nonce: u64) -> Result<u64> {
        let mut disposed = 0_u64;
        self.transact(expected_nonce, |next| {
            if next.phase != Phase::CreditsSealed {
                return Err(Error::Phase);
            }
            disposed = next.hoard.balance;
            next.hoard.balance = 0;
            next.hoard.surplus_sink_out = next
                .hoard
                .surplus_sink_out
                .checked_add(disposed)
                .ok_or(Error::Arithmetic)?;
            Ok(())
        })?;
        Ok(disposed)
    }

    pub fn close_market_graph(
        &mut self,
        registry: &mut Registry,
        expected_nonce: u64,
        authority: Id,
    ) -> Result<Tombstone> {
        self.validate()?;
        if registry.tombstone.present
            || registry.active_market != self.market
            || registry.active_generation != self.generation
            || expected_nonce != self.market_nonce
        {
            return Err(Error::Replay);
        }
        if self.phase != Phase::CreditsSealed
            || self.hoard.balance != 0
            || authority == ZERO_ID
            || authority != self.terminal_authority
        {
            return Err(Error::Authority);
        }
        let mut next = *self;
        next.keeper.close()?;
        for role in [Role::Supply, Role::Resolution, Role::Hoard, Role::Market] {
            next.rents
                .get_mut(role)?
                .close(next.rent_refund_to, next.neutral_sink)?;
        }
        next.market_nonce = next.market_nonce.checked_add(1).ok_or(Error::Arithmetic)?;
        next.phase = Phase::Terminal;
        let terminal_rent_snapshot = next.rents.summary()?;
        next.credit_vault.terminal_rent_snapshot = Some(terminal_rent_snapshot);
        next.credit_vault.terminal_market_nonce = next.market_nonce;
        next.validate()?;
        let tombstone = Tombstone {
            present: true,
            market: next.market,
            generation: next.generation,
            outcomes: next.outcomes,
            terminal_receipt: next.resolution.ok_or(Error::Phase)?.receipt,
            final_market_nonce: next.market_nonce,
            replay_account: next.rents.get(Role::Replay)?.account,
            credit_vault_account: next.rents.get(Role::CreditVault)?.account,
            rent: terminal_rent_snapshot,
            keeper_deposit: next.keeper.deposit,
            keeper_rewards_paid: next.keeper.rewards_paid,
            keeper_refund_paid: next.keeper.refund_paid,
            keeper_donations_sunk: next.keeper.sink_paid,
        };
        tombstone.validate()?;
        *self = next;
        registry.active_market = ZERO_ID;
        registry.active_generation = 0;
        registry.tombstone = tombstone;
        Ok(tombstone)
    }

    pub fn transfer_credit(
        &mut self,
        expected_vault_nonce: u64,
        source_slot: usize,
        source_owner: Id,
        destination_slot: usize,
        destination_owner: Id,
        numerator: u64,
    ) -> Result<u64> {
        self.validate()?;
        if self.phase != Phase::Terminal
            || expected_vault_nonce != self.credit_vault.nonce
            || source_slot >= MAX_CREDITS
            || destination_slot >= MAX_CREDITS
            || source_slot == destination_slot
            || numerator == 0
        {
            return Err(Error::Replay);
        }
        let source = self.credits[source_slot];
        let destination = self.credits[destination_slot];
        if !source.present
            || source.owner != source_owner
            || source.numerator < numerator
            || !destination.present
            || destination.owner != destination_owner
        {
            return Err(Error::Authority);
        }
        let denominator = self.resolution.ok_or(Error::Phase)?.denominator;
        let accumulated = u128::from(destination.numerator) + u128::from(numerator);
        let paid =
            u64::try_from(accumulated / u128::from(denominator)).map_err(|_| Error::Arithmetic)?;
        let residue =
            u64::try_from(accumulated % u128::from(denominator)).map_err(|_| Error::Arithmetic)?;
        if paid > self.credit_vault.balance {
            return Err(Error::Insufficient);
        }
        let mut next = *self;
        next.credits[source_slot].numerator -= numerator;
        next.credits[destination_slot].numerator = residue;
        next.credit_vault.balance -= paid;
        next.credit_vault.payout_out = next
            .credit_vault
            .payout_out
            .checked_add(paid)
            .ok_or(Error::Arithmetic)?;
        next.credit_vault.credit_numerator_total = next
            .credit_vault
            .credit_numerator_total
            .checked_sub(
                u128::from(paid)
                    .checked_mul(u128::from(denominator))
                    .ok_or(Error::Arithmetic)?,
            )
            .ok_or(Error::Invariant)?;
        next.credit_vault.nonce = next
            .credit_vault
            .nonce
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(paid)
    }

    /// Creates an owner-accepted zero credit account after market-graph close,
    /// so an owner without a prior redemption residue can receive a transfer.
    pub fn open_terminal_credit(
        &mut self,
        expected_vault_nonce: u64,
        slot: usize,
        owner: Id,
        funding: CreditRentFunding,
    ) -> Result<()> {
        self.validate()?;
        if self.phase != Phase::Terminal
            || expected_vault_nonce != self.credit_vault.nonce
            || slot >= MAX_CREDITS
            || owner == ZERO_ID
            || self.credits[slot] != CreditAccount::EMPTY
        {
            return Err(Error::Replay);
        }
        let mut other = 0_usize;
        while other < MAX_CREDITS {
            if self.credits[other].present && self.credits[other].owner == owner {
                return Err(Error::Identity);
            }
            other += 1;
        }
        let mut next = *self;
        next.rents.insert(RentRecord::new(
            Role::Credit(slot as u8),
            AccountClass::ExternalOwnerState,
            next.market,
            next.generation,
            funding.payer,
            funding.refund_to,
            next.neutral_sink,
            funding.principal,
            funding.prefund_donation,
        )?)?;
        next.credits[slot] = CreditAccount {
            present: true,
            closed: false,
            owner,
            numerator: 0,
        };
        next.credit_vault.nonce = next
            .credit_vault
            .nonce
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Reconciles unsolicited collateral observed after graph close.
    pub fn reconcile_terminal_credit_vault_donation(
        &mut self,
        expected_vault_nonce: u64,
        quantity: u64,
    ) -> Result<()> {
        self.validate()?;
        if self.phase != Phase::Terminal
            || expected_vault_nonce != self.credit_vault.nonce
            || quantity == 0
        {
            return Err(Error::Replay);
        }
        let mut next = *self;
        next.credit_vault.donation_balance = next
            .credit_vault
            .donation_balance
            .checked_add(quantity)
            .ok_or(Error::Arithmetic)?;
        next.credit_vault.donations_in = next
            .credit_vault
            .donations_in
            .checked_add(quantity)
            .ok_or(Error::Arithmetic)?;
        next.credit_vault.nonce = next
            .credit_vault
            .nonce
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Sends only the segregated unsolicited-donation compartment to sink.
    pub fn dispose_terminal_credit_vault_donations(
        &mut self,
        expected_vault_nonce: u64,
    ) -> Result<u64> {
        self.validate()?;
        if self.phase != Phase::Terminal || expected_vault_nonce != self.credit_vault.nonce {
            return Err(Error::Replay);
        }
        let mut next = *self;
        let disposed = next.credit_vault.donation_balance;
        next.credit_vault.donation_balance = 0;
        next.credit_vault.donation_sink_out = next
            .credit_vault
            .donation_sink_out
            .checked_add(disposed)
            .ok_or(Error::Arithmetic)?;
        next.credit_vault.nonce = next
            .credit_vault
            .nonce
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(disposed)
    }

    pub fn forfeit_credit(
        &mut self,
        expected_vault_nonce: u64,
        slot: usize,
        owner: Id,
        numerator: u64,
    ) -> Result<u64> {
        self.validate()?;
        if self.phase != Phase::Terminal
            || expected_vault_nonce != self.credit_vault.nonce
            || slot >= MAX_CREDITS
            || numerator == 0
        {
            return Err(Error::Replay);
        }
        let credit = self.credits[slot];
        if !credit.present || credit.owner != owner || credit.numerator < numerator {
            return Err(Error::Authority);
        }
        let denominator = self.resolution.ok_or(Error::Phase)?.denominator;
        let mut next = *self;
        next.credits[slot].numerator -= numerator;
        next.credit_vault.credit_numerator_total = next
            .credit_vault
            .credit_numerator_total
            .checked_sub(u128::from(numerator))
            .ok_or(Error::Invariant)?;
        next.credit_vault.forfeited_numerator = next
            .credit_vault
            .forfeited_numerator
            .checked_add(u128::from(numerator))
            .ok_or(Error::Arithmetic)?;
        let required = if next.credit_vault.credit_numerator_total == 0 {
            0
        } else {
            u64::try_from(
                next.credit_vault
                    .credit_numerator_total
                    .div_ceil(u128::from(denominator)),
            )
            .map_err(|_| Error::Arithmetic)?
        };
        let released = next
            .credit_vault
            .balance
            .checked_sub(required)
            .ok_or(Error::Invariant)?;
        next.credit_vault.balance = required;
        next.credit_vault.forfeiture_sink_out = next
            .credit_vault
            .forfeiture_sink_out
            .checked_add(released)
            .ok_or(Error::Arithmetic)?;
        next.credit_vault.nonce = next
            .credit_vault
            .nonce
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(released)
    }

    pub fn close_credit(
        &mut self,
        expected_vault_nonce: u64,
        slot: usize,
        owner: Id,
        refund_to: Id,
    ) -> Result<RentCloseEffect> {
        self.validate()?;
        if self.phase != Phase::Terminal
            || expected_vault_nonce != self.credit_vault.nonce
            || slot >= MAX_CREDITS
        {
            return Err(Error::Replay);
        }
        let credit = self.credits[slot];
        if !credit.present || credit.owner != owner || credit.numerator != 0 {
            return Err(Error::OutstandingCredit);
        }
        let mut next = *self;
        let effect = next
            .rents
            .get_mut(Role::Credit(slot as u8))?
            .close(refund_to, next.neutral_sink)?;
        next.credits[slot] = CreditAccount {
            present: false,
            closed: true,
            owner,
            numerator: 0,
        };
        next.credit_vault.nonce = next
            .credit_vault
            .nonce
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        next.validate()?;
        *self = next;
        Ok(effect)
    }

    pub fn registry_matches(&self, registry: &Registry) -> Result<()> {
        self.validate()?;
        let tombstone = registry.tombstone;
        tombstone.validate()?;
        let current_rent = self.rents.summary()?;
        let terminal_rent = self
            .credit_vault
            .terminal_rent_snapshot
            .ok_or(Error::Invariant)?;
        let terminal_roles_still_accounted = tombstone.rent.closed_role_bits
            & current_rent.closed_role_bits
            == tombstone.rent.closed_role_bits;
        let terminal_external_roles_still_accounted = tombstone.rent.open_external_role_bits
            & (current_rent.open_external_role_bits | current_rent.closed_role_bits)
            == tombstone.rent.open_external_role_bits;
        if self.phase != Phase::Terminal
            || registry.active_market != ZERO_ID
            || registry.active_generation != 0
            || !tombstone.present
            || tombstone.market != self.market
            || tombstone.generation != self.generation
            || tombstone.outcomes != self.outcomes
            || tombstone.terminal_receipt != self.resolution.ok_or(Error::Phase)?.receipt
            || tombstone.final_market_nonce != self.market_nonce
            || tombstone.replay_account != self.rents.get(Role::Replay)?.account
            || tombstone.credit_vault_account != self.rents.get(Role::CreditVault)?.account
            || tombstone.rent != terminal_rent
            || !terminal_roles_still_accounted
            || !terminal_external_roles_still_accounted
            || tombstone.rent.permanent_role_bits != current_rent.permanent_role_bits
            || tombstone.rent.permanent_principal != current_rent.permanent_principal
            || tombstone.rent.permanent_donations > current_rent.permanent_donations
            || tombstone.rent.principal_refunded > current_rent.principal_refunded
            || tombstone.rent.donations_sunk > current_rent.donations_sunk
            || tombstone.keeper_deposit != self.keeper.deposit
            || tombstone.keeper_rewards_paid != self.keeper.rewards_paid
            || tombstone.keeper_refund_paid != self.keeper.refund_paid
            || tombstone.keeper_donations_sunk != self.keeper.sink_paid
        {
            return Err(Error::Replay);
        }
        Ok(())
    }

    pub fn terminal_slack_equation(&self) -> Result<(u128, u128)> {
        self.validate()?;
        if self.phase != Phase::Terminal {
            return Err(Error::Phase);
        }
        let resolution = self.resolution.ok_or(Error::Phase)?;
        let mut burn = 0_u128;
        let mut outcome = 0_usize;
        while outcome < usize::from(self.outcomes) {
            burn = burn
                .checked_add(
                    u128::from(self.supply[outcome].direct_burned)
                        .checked_mul(u128::from(resolution.weights[outcome]))
                        .ok_or(Error::Arithmetic)?,
                )
                .ok_or(Error::Arithmetic)?;
            outcome += 1;
        }
        let total_donations = self
            .hoard
            .donations_in
            .checked_add(self.credit_vault.donations_in)
            .ok_or(Error::Arithmetic)?;
        let left = u128::from(total_donations)
            .checked_mul(u128::from(resolution.denominator))
            .and_then(|value| value.checked_add(burn))
            .and_then(|value| value.checked_add(self.credit_vault.forfeited_numerator))
            .ok_or(Error::Arithmetic)?;
        let sink_atoms = self
            .hoard
            .surplus_sink_out
            .checked_add(self.credit_vault.forfeiture_sink_out)
            .and_then(|value| value.checked_add(self.credit_vault.donation_sink_out))
            .and_then(|value| value.checked_add(self.credit_vault.donation_balance))
            .ok_or(Error::Arithmetic)?;
        let right = u128::from(sink_atoms)
            .checked_mul(u128::from(resolution.denominator))
            .and_then(|value| {
                value.checked_add(
                    self.credit_vault
                        .rounding_slack_numerator(resolution.denominator)
                        .ok()?,
                )
            })
            .ok_or(Error::Arithmetic)?;
        Ok((left, right))
    }

    fn any_claims(&self) -> bool {
        self.supply[..usize::from(self.outcomes)]
            .iter()
            .any(|supply| supply.internal != 0 || supply.external != 0)
    }
}

fn role_account(mut market: Id, role: Role) -> Id {
    market[31] ^= role.code();
    if market == ZERO_ID {
        market[0] = role.code();
    }
    market
}
