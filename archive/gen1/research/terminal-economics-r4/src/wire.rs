use crate::{
    AccountClass, Id, MintVersion, Phase, RentRecord, RentSummary, Role, Tombstone, ZERO_ID,
};

const POLICY_MAGIC: [u8; 4] = *b"TRP4";
const CREDIT_MAGIC: [u8; 4] = *b"TCR1";
const CREDIT_ROOT_MAGIC: [u8; 4] = *b"TRC1";
const RENT_MAGIC: [u8; 4] = *b"TRN1";
const TOMBSTONE_MAGIC: [u8; 4] = *b"TTB1";
const MINT_MAGIC: [u8; 4] = *b"TMI4";

pub const POLICY_WIRE_BYTES: usize = 224;
pub const CREDIT_WIRE_BYTES: usize = 136;
pub const CREDIT_ROOT_WIRE_BYTES: usize = 136;
pub const RENT_WIRE_BYTES: usize = 216;
pub const TOMBSTONE_WIRE_BYTES: usize = 248;
pub const MINT_BINDING_WIRE_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Length,
    Magic,
    Version,
    Shape,
    Padding,
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditRootWireV1 {
    pub market: Id,
    pub generation: u64,
    pub phase: Phase,
    pub credit_vault: Id,
    pub denominator: u64,
    pub credit_numerator_total: u128,
    pub forfeited_numerator: u128,
    pub nonce: u64,
    pub terminal_market_nonce: u64,
}

impl CreditRootWireV1 {
    pub fn validate(&self) -> core::result::Result<(), DecodeError> {
        if self.market == ZERO_ID
            || self.generation == 0
            || self.credit_vault == ZERO_ID
            || self.denominator == 0
            || self.denominator > crate::MAX_ATOMS
            || !matches!(self.phase, Phase::CreditsSealed | Phase::Terminal)
            || (self.phase == Phase::CreditsSealed && self.terminal_market_nonce != 0)
            || (self.phase == Phase::Terminal && self.terminal_market_nonce == 0)
        {
            return Err(DecodeError::Shape);
        }
        if self.market == self.credit_vault {
            return Err(DecodeError::Shape);
        }
        let maximum_credit = u128::from(self.denominator)
            .checked_mul(crate::MAX_CREDITS as u128)
            .ok_or(DecodeError::Overflow)?;
        if self.credit_numerator_total >= maximum_credit {
            return Err(DecodeError::Shape);
        }
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> core::result::Result<(), DecodeError> {
        self.validate()?;
        if output.len() != CREDIT_ROOT_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut writer = Writer::new(output);
        writer.bytes(&CREDIT_ROOT_MAGIC)?;
        writer.u8(1)?;
        writer.u8(self.phase as u8)?;
        writer.zeros(2)?;
        writer.id(self.market)?;
        writer.u64(self.generation)?;
        writer.id(self.credit_vault)?;
        writer.u64(self.denominator)?;
        writer.u128(self.credit_numerator_total)?;
        writer.u128(self.forfeited_numerator)?;
        writer.u64(self.nonce)?;
        writer.u64(self.terminal_market_nonce)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> core::result::Result<Self, DecodeError> {
        if input.len() != CREDIT_ROOT_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut reader = Reader::new(input);
        if reader.array4()? != CREDIT_ROOT_MAGIC {
            return Err(DecodeError::Magic);
        }
        if reader.u8()? != 1 {
            return Err(DecodeError::Version);
        }
        let phase = match reader.u8()? {
            3 => Phase::CreditsSealed,
            4 => Phase::Terminal,
            _ => return Err(DecodeError::Shape),
        };
        reader.zeroes(2)?;
        let value = Self {
            market: reader.id()?,
            generation: reader.u64()?,
            phase,
            credit_vault: reader.id()?,
            denominator: reader.u64()?,
            credit_numerator_total: reader.u128()?,
            forfeited_numerator: reader.u128()?,
            nonce: reader.u64()?,
            terminal_market_nonce: reader.u64()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyWireV4 {
    pub market: Id,
    pub generation: u64,
    pub outcomes: u8,
    pub terminal_authority: Id,
    pub rent_refund_to: Id,
    pub neutral_sink: Id,
    pub credit_vault: Id,
    pub replay_tombstone: Id,
    pub keeper_budget_atoms: u64,
    pub minimum_credit_rent: u64,
}

impl PolicyWireV4 {
    pub fn validate(&self) -> core::result::Result<(), DecodeError> {
        if self.market == ZERO_ID
            || self.generation == 0
            || !(2..=crate::MAX_OUTCOMES as u8).contains(&self.outcomes)
            || self.terminal_authority == ZERO_ID
            || self.rent_refund_to == ZERO_ID
            || self.neutral_sink == ZERO_ID
            || self.credit_vault == ZERO_ID
            || self.replay_tombstone == ZERO_ID
            || self.keeper_budget_atoms == 0
            || self.keeper_budget_atoms > crate::MAX_ATOMS
            || self.minimum_credit_rent == 0
            || self.minimum_credit_rent > crate::MAX_ATOMS
        {
            return Err(DecodeError::Shape);
        }
        let identities = [
            self.market,
            self.terminal_authority,
            self.rent_refund_to,
            self.neutral_sink,
            self.credit_vault,
            self.replay_tombstone,
        ];
        let mut i = 0_usize;
        while i < identities.len() {
            let mut j = 0_usize;
            while j < i {
                if identities[i] == identities[j] {
                    return Err(DecodeError::Shape);
                }
                j += 1;
            }
            i += 1;
        }
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> core::result::Result<(), DecodeError> {
        self.validate()?;
        if output.len() != POLICY_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut writer = Writer::new(output);
        writer.bytes(&POLICY_MAGIC)?;
        writer.u8(4)?;
        writer.u8(self.outcomes)?;
        writer.zeros(2)?;
        writer.id(self.market)?;
        writer.u64(self.generation)?;
        writer.id(self.terminal_authority)?;
        writer.id(self.rent_refund_to)?;
        writer.id(self.neutral_sink)?;
        writer.id(self.credit_vault)?;
        writer.id(self.replay_tombstone)?;
        writer.u64(self.keeper_budget_atoms)?;
        writer.u64(self.minimum_credit_rent)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> core::result::Result<Self, DecodeError> {
        if input.len() != POLICY_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut reader = Reader::new(input);
        if reader.array4()? != POLICY_MAGIC {
            return Err(DecodeError::Magic);
        }
        if reader.u8()? != 4 {
            return Err(DecodeError::Version);
        }
        let outcomes = reader.u8()?;
        reader.zeroes(2)?;
        let value = Self {
            market: reader.id()?,
            generation: reader.u64()?,
            outcomes,
            terminal_authority: reader.id()?,
            rent_refund_to: reader.id()?,
            neutral_sink: reader.id()?,
            credit_vault: reader.id()?,
            replay_tombstone: reader.id()?,
            keeper_budget_atoms: reader.u64()?,
            minimum_credit_rent: reader.u64()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditWireV1 {
    pub market: Id,
    pub generation: u64,
    pub owner: Id,
    pub numerator: u64,
    pub denominator: u64,
    pub rent_account: Id,
    pub nonce: u64,
    pub closed: bool,
}

impl CreditWireV1 {
    pub fn validate(&self) -> core::result::Result<(), DecodeError> {
        if self.market == ZERO_ID
            || self.generation == 0
            || self.owner == ZERO_ID
            || self.denominator == 0
            || self.denominator > crate::MAX_ATOMS
            || self.numerator >= self.denominator
            || self.rent_account == ZERO_ID
            || (self.closed && self.numerator != 0)
        {
            return Err(DecodeError::Shape);
        }
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> core::result::Result<(), DecodeError> {
        self.validate()?;
        if output.len() != CREDIT_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut writer = Writer::new(output);
        writer.bytes(&CREDIT_MAGIC)?;
        writer.u8(1)?;
        writer.u8(u8::from(self.closed))?;
        writer.zeros(2)?;
        writer.id(self.market)?;
        writer.u64(self.generation)?;
        writer.id(self.owner)?;
        writer.u64(self.numerator)?;
        writer.u64(self.denominator)?;
        writer.id(self.rent_account)?;
        writer.u64(self.nonce)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> core::result::Result<Self, DecodeError> {
        if input.len() != CREDIT_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut reader = Reader::new(input);
        if reader.array4()? != CREDIT_MAGIC {
            return Err(DecodeError::Magic);
        }
        if reader.u8()? != 1 {
            return Err(DecodeError::Version);
        }
        let closed = decode_bool(reader.u8()?)?;
        reader.zeroes(2)?;
        let value = Self {
            market: reader.id()?,
            generation: reader.u64()?,
            owner: reader.id()?,
            numerator: reader.u64()?,
            denominator: reader.u64()?,
            rent_account: reader.id()?,
            nonce: reader.u64()?,
            closed,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentWireV1 {
    pub record: RentRecord,
}

impl RentWireV1 {
    pub fn encode(&self, output: &mut [u8]) -> core::result::Result<(), DecodeError> {
        self.record.validate().map_err(|_| DecodeError::Shape)?;
        if !self.record.present || output.len() != RENT_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut writer = Writer::new(output);
        writer.bytes(&RENT_MAGIC)?;
        writer.u8(1)?;
        writer.u8(self.record.class as u8)?;
        writer.u8(self.record.role.code())?;
        writer.u8(u8::from(self.record.closed))?;
        writer.id(self.record.market)?;
        writer.u64(self.record.generation)?;
        writer.id(self.record.account)?;
        writer.id(self.record.payer)?;
        writer.id(self.record.refund_to)?;
        writer.id(self.record.sink)?;
        writer.u64(self.record.principal)?;
        writer.u64(self.record.donations)?;
        writer.u64(self.record.balance)?;
        writer.u64(self.record.refund_paid)?;
        writer.u64(self.record.sink_paid)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> core::result::Result<Self, DecodeError> {
        if input.len() != RENT_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut reader = Reader::new(input);
        if reader.array4()? != RENT_MAGIC {
            return Err(DecodeError::Magic);
        }
        if reader.u8()? != 1 {
            return Err(DecodeError::Version);
        }
        let class = decode_class(reader.u8()?)?;
        let role = decode_role(reader.u8()?)?;
        let closed = decode_bool(reader.u8()?)?;
        let record = RentRecord {
            present: true,
            role,
            class,
            market: reader.id()?,
            generation: reader.u64()?,
            account: reader.id()?,
            payer: reader.id()?,
            refund_to: reader.id()?,
            sink: reader.id()?,
            principal: reader.u64()?,
            donations: reader.u64()?,
            balance: reader.u64()?,
            refund_paid: reader.u64()?,
            sink_paid: reader.u64()?,
            closed,
        };
        reader.finish()?;
        record.validate().map_err(|_| DecodeError::Shape)?;
        Ok(Self { record })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TombstoneWireV1 {
    pub tombstone: Tombstone,
}

impl TombstoneWireV1 {
    pub fn encode(&self, output: &mut [u8]) -> core::result::Result<(), DecodeError> {
        self.tombstone.validate().map_err(|_| DecodeError::Shape)?;
        if output.len() != TOMBSTONE_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut writer = Writer::new(output);
        writer.bytes(&TOMBSTONE_MAGIC)?;
        writer.u8(1)?;
        writer.u8(1)?;
        writer.u8(self.tombstone.outcomes)?;
        writer.zeros(1)?;
        writer.id(self.tombstone.market)?;
        writer.u64(self.tombstone.generation)?;
        writer.id(self.tombstone.terminal_receipt)?;
        writer.u64(self.tombstone.final_market_nonce)?;
        writer.id(self.tombstone.replay_account)?;
        writer.id(self.tombstone.credit_vault_account)?;
        writer.u32(self.tombstone.rent.closed_role_bits)?;
        writer.u32(self.tombstone.rent.permanent_role_bits)?;
        writer.u32(self.tombstone.rent.open_external_role_bits)?;
        writer.zeros(4)?;
        writer.u64(self.tombstone.rent.principal_refunded)?;
        writer.u64(self.tombstone.rent.donations_sunk)?;
        writer.u64(self.tombstone.rent.permanent_principal)?;
        writer.u64(self.tombstone.rent.permanent_donations)?;
        writer.u64(self.tombstone.rent.external_principal_live)?;
        writer.u64(self.tombstone.rent.external_donations_live)?;
        writer.u64(self.tombstone.keeper_deposit)?;
        writer.u64(self.tombstone.keeper_rewards_paid)?;
        writer.u64(self.tombstone.keeper_refund_paid)?;
        writer.u64(self.tombstone.keeper_donations_sunk)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> core::result::Result<Self, DecodeError> {
        if input.len() != TOMBSTONE_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut reader = Reader::new(input);
        if reader.array4()? != TOMBSTONE_MAGIC {
            return Err(DecodeError::Magic);
        }
        if reader.u8()? != 1 {
            return Err(DecodeError::Version);
        }
        if reader.u8()? != 1 {
            return Err(DecodeError::Shape);
        }
        let outcomes = reader.u8()?;
        reader.zeroes(1)?;
        let tombstone = Tombstone {
            present: true,
            market: reader.id()?,
            generation: reader.u64()?,
            outcomes,
            terminal_receipt: reader.id()?,
            final_market_nonce: reader.u64()?,
            replay_account: reader.id()?,
            credit_vault_account: reader.id()?,
            rent: RentSummary {
                closed_role_bits: reader.u32()?,
                permanent_role_bits: reader.u32()?,
                open_external_role_bits: reader.u32()?,
                principal_refunded: {
                    reader.zeroes(4)?;
                    reader.u64()?
                },
                donations_sunk: reader.u64()?,
                permanent_principal: reader.u64()?,
                permanent_donations: reader.u64()?,
                external_principal_live: reader.u64()?,
                external_donations_live: reader.u64()?,
            },
            keeper_deposit: reader.u64()?,
            keeper_rewards_paid: reader.u64()?,
            keeper_refund_paid: reader.u64()?,
            keeper_donations_sunk: reader.u64()?,
        };
        reader.finish()?;
        tombstone.validate().map_err(|_| DecodeError::Shape)?;
        Ok(Self { tombstone })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintBindingWireV4 {
    pub market: Id,
    pub generation: u64,
    pub mint: Id,
    pub terminal_authority: Id,
    pub outcome: u8,
    pub authoritative_supply: u64,
}

impl MintBindingWireV4 {
    pub fn encode(&self, output: &mut [u8]) -> core::result::Result<(), DecodeError> {
        if self.market == ZERO_ID
            || self.generation == 0
            || self.mint == ZERO_ID
            || self.terminal_authority == ZERO_ID
            || usize::from(self.outcome) >= crate::MAX_OUTCOMES
            || self.authoritative_supply > crate::MAX_ATOMS
            || output.len() != MINT_BINDING_WIRE_BYTES
        {
            return Err(DecodeError::Shape);
        }
        let mut writer = Writer::new(output);
        writer.bytes(&MINT_MAGIC)?;
        writer.u8(MintVersion::R4Closeable as u8)?;
        writer.u8(self.outcome)?;
        writer.zeros(2)?;
        writer.id(self.market)?;
        writer.u64(self.generation)?;
        writer.id(self.mint)?;
        writer.id(self.terminal_authority)?;
        writer.zeros(8)?;
        writer.u64(self.authoritative_supply)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> core::result::Result<Self, DecodeError> {
        if input.len() != MINT_BINDING_WIRE_BYTES {
            return Err(DecodeError::Length);
        }
        let mut reader = Reader::new(input);
        if reader.array4()? != MINT_MAGIC {
            return Err(DecodeError::Magic);
        }
        if reader.u8()? != MintVersion::R4Closeable as u8 {
            return Err(DecodeError::Version);
        }
        let outcome = reader.u8()?;
        reader.zeroes(2)?;
        let value = Self {
            market: reader.id()?,
            generation: reader.u64()?,
            mint: reader.id()?,
            terminal_authority: reader.id()?,
            outcome,
            authoritative_supply: {
                reader.zeroes(8)?;
                reader.u64()?
            },
        };
        reader.finish()?;
        if value.market == ZERO_ID
            || value.generation == 0
            || value.mint == ZERO_ID
            || value.terminal_authority == ZERO_ID
            || usize::from(value.outcome) >= crate::MAX_OUTCOMES
            || value.authoritative_supply > crate::MAX_ATOMS
        {
            return Err(DecodeError::Shape);
        }
        Ok(value)
    }
}

fn decode_bool(value: u8) -> core::result::Result<bool, DecodeError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(DecodeError::Shape),
    }
}

fn decode_class(value: u8) -> core::result::Result<AccountClass, DecodeError> {
    match value {
        1 => Ok(AccountClass::RefundableTransient),
        2 => Ok(AccountClass::PermanentInfra),
        3 => Ok(AccountClass::PermanentTombstone),
        4 => Ok(AccountClass::ExternalOwnerState),
        5 => Ok(AccountClass::UnclassifiedStop),
        _ => Err(DecodeError::Shape),
    }
}

fn decode_role(value: u8) -> core::result::Result<Role, DecodeError> {
    match value {
        1 => Ok(Role::Market),
        2 => Ok(Role::Hoard),
        3 => Ok(Role::Supply),
        4 => Ok(Role::Resolution),
        16..=19 => Ok(Role::Position(value - 16)),
        32..=35 => Ok(Role::Mint(value - 32)),
        48 => Ok(Role::CreditVault),
        49..=56 => Ok(Role::Credit(value - 49)),
        63 => Ok(Role::Replay),
        _ => Err(DecodeError::Shape),
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    offset: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        output.fill(0);
        Self { output, offset: 0 }
    }

    fn bytes(&mut self, value: &[u8]) -> core::result::Result<(), DecodeError> {
        let end = self
            .offset
            .checked_add(value.len())
            .ok_or(DecodeError::Overflow)?;
        let target = self
            .output
            .get_mut(self.offset..end)
            .ok_or(DecodeError::Length)?;
        target.copy_from_slice(value);
        self.offset = end;
        Ok(())
    }

    fn zeros(&mut self, count: usize) -> core::result::Result<(), DecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DecodeError::Overflow)?;
        let target = self
            .output
            .get_mut(self.offset..end)
            .ok_or(DecodeError::Length)?;
        target.fill(0);
        self.offset = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> core::result::Result<(), DecodeError> {
        self.bytes(&[value])
    }

    fn u64(&mut self, value: u64) -> core::result::Result<(), DecodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> core::result::Result<(), DecodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> core::result::Result<(), DecodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn id(&mut self, value: Id) -> core::result::Result<(), DecodeError> {
        self.bytes(&value)
    }

    fn finish(self) -> core::result::Result<(), DecodeError> {
        if self.offset == self.output.len() {
            Ok(())
        } else {
            Err(DecodeError::Length)
        }
    }
}

struct Reader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn bytes(&mut self, count: usize) -> core::result::Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DecodeError::Overflow)?;
        let value = self
            .input
            .get(self.offset..end)
            .ok_or(DecodeError::Length)?;
        self.offset = end;
        Ok(value)
    }

    fn zeroes(&mut self, count: usize) -> core::result::Result<(), DecodeError> {
        if self.bytes(count)?.iter().any(|byte| *byte != 0) {
            return Err(DecodeError::Padding);
        }
        Ok(())
    }

    fn u8(&mut self) -> core::result::Result<u8, DecodeError> {
        Ok(self.bytes(1)?[0])
    }

    fn u64(&mut self) -> core::result::Result<u64, DecodeError> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.bytes(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn u128(&mut self) -> core::result::Result<u128, DecodeError> {
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(self.bytes(16)?);
        Ok(u128::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> core::result::Result<u32, DecodeError> {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(self.bytes(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn id(&mut self) -> core::result::Result<Id, DecodeError> {
        let mut value = [0_u8; 32];
        value.copy_from_slice(self.bytes(32)?);
        Ok(value)
    }

    fn array4(&mut self) -> core::result::Result<[u8; 4], DecodeError> {
        let mut value = [0_u8; 4];
        value.copy_from_slice(self.bytes(4)?);
        Ok(value)
    }

    fn finish(self) -> core::result::Result<(), DecodeError> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(DecodeError::Length)
        }
    }
}
