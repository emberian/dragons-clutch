//! The scoring Dealer's four requests, its one receipt, the accelerator's fill
//! witness, and the four account frames -- every width, offset and coordinate
//! the emitted constant (`ScoringRuleAbiV1.lean`).
//!
//! Each request is selected by its magic alone (like `DCLTDBR1`): the
//! `is_dealer_*_v1` predicates are the guard chain `process_instruction`
//! reads and the route census names.

use dclutch_sha256_adapter::digest;

use super::generated::{
    FILL_ACCOUNT_COUNT, FILL_REQUEST_BYTES, FILL_REQUEST_DEALER_ID_OFFSET,
    FILL_REQUEST_DELIVER_OFFSET, FILL_REQUEST_EXPECTED_FUND_REVISION_OFFSET, FILL_REQUEST_MAGIC,
    FILL_REQUEST_MARKET_OFFSET, FILL_REQUEST_MINT_OFFSET, FILL_REQUEST_OUTCOME_COUNT_OFFSET,
    FILL_REQUEST_PRICES_OFFSET, FILL_REQUEST_RECEIVE_OFFSET, FILL_REQUEST_RESERVED_OFFSET,
    FILL_REQUEST_TAKER_OFFSET, FILL_REQUEST_VERSION_OFFSET, FILL_SIGNER, FILL_WITNESS_BYTES,
    FILL_WITNESS_CASH_OFFSET, FILL_WITNESS_FUND_REVISION_OFFSET, FILL_WITNESS_INVENTORY_OFFSET,
    FILL_WITNESS_MAGIC, FILL_WITNESS_OUTCOME_COUNT_OFFSET, FILL_WITNESS_RESERVED_OFFSET,
    FILL_WITNESS_RULE_OFFSET, FILL_WITNESS_VERSION_OFFSET, FILL_WRITABLE, FOUND_ACCOUNT_COUNT,
    FOUND_REQUEST_BYTES, FOUND_REQUEST_CLAIM_UNIT_ATOMS_OFFSET, FOUND_REQUEST_DEALER_ID_OFFSET,
    FOUND_REQUEST_DEPOSIT_OFFSET, FOUND_REQUEST_GENERATION_OFFSET, FOUND_REQUEST_LIQUIDITY_OFFSET,
    FOUND_REQUEST_MAGIC, FOUND_REQUEST_MARKET_OFFSET, FOUND_REQUEST_OUTCOME_COUNT_OFFSET,
    FOUND_REQUEST_RELEASE_SET_OFFSET, FOUND_REQUEST_RESERVED_OFFSET, FOUND_REQUEST_SCALE_OFFSET,
    FOUND_REQUEST_TOLERANCE_OFFSET, FOUND_REQUEST_VERSION_OFFSET, FOUND_SIGNER, FOUND_WRITABLE,
    QUOTE_ACCOUNT_COUNT, QUOTE_REQUEST_BYTES, QUOTE_REQUEST_DEALER_ID_OFFSET,
    QUOTE_REQUEST_EXPECTED_FUND_REVISION_OFFSET, QUOTE_REQUEST_MAGIC, QUOTE_REQUEST_MARKET_OFFSET,
    QUOTE_REQUEST_RESERVED_OFFSET, QUOTE_REQUEST_VERSION_OFFSET, QUOTE_SIGNER, QUOTE_WRITABLE,
    RECEIPT_BYTES, RECEIPT_CASH_OFFSET, RECEIPT_DEALER_ID_OFFSET, RECEIPT_DEALER_PAYS_OFFSET,
    RECEIPT_DEALER_RECEIVES_OFFSET, RECEIPT_FUND_DIGEST_OFFSET, RECEIPT_FUND_REVISION_OFFSET,
    RECEIPT_INVENTORY_MINIMUM_OFFSET, RECEIPT_LIQUIDITY_COST_OFFSET, RECEIPT_MAGIC,
    RECEIPT_MARKET_OFFSET, RECEIPT_OUTCOME_COUNT_OFFSET, RECEIPT_REQUEST_DIGEST_OFFSET,
    RECEIPT_RESERVED_OFFSET, RECEIPT_ROUTE_OFFSET, RECEIPT_VERSION_OFFSET, ROUTE_FILL, ROUTE_FOUND,
    ROUTE_QUOTE, ROUTE_WITHDRAW, RULE_BYTES, WIRE_VERSION, WITHDRAW_ACCOUNT_COUNT,
    WITHDRAW_REQUEST_AMOUNT_OFFSET, WITHDRAW_REQUEST_BYTES, WITHDRAW_REQUEST_DEALER_ID_OFFSET,
    WITHDRAW_REQUEST_EXPECTED_FUND_REVISION_OFFSET, WITHDRAW_REQUEST_MAGIC,
    WITHDRAW_REQUEST_MARKET_OFFSET, WITHDRAW_REQUEST_RESERVED_OFFSET,
    WITHDRAW_REQUEST_VERSION_OFFSET, WITHDRAW_SIGNER, WITHDRAW_WRITABLE,
};
use super::records_v1::{
    DealerFundV1, RecordErrorV1, ScoringRuleRecordV1, read_vector, write_vector,
};
use super::{RuleParameters, Vector};

pub use super::records_v1::Result;

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(RecordErrorV1::InvalidLength)?)
        .and_then(|window| window.try_into().ok())
        .ok_or(RecordErrorV1::InvalidLength)
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    array::<8>(bytes, offset).map(u64::from_le_bytes)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    array::<2>(bytes, offset).map(u16::from_le_bytes)
}

fn u8_at(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(RecordErrorV1::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let window = bytes
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(RecordErrorV1::InvalidLength)?,
        )
        .ok_or(RecordErrorV1::InvalidLength)?;
    if window.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(RecordErrorV1::NonCanonical)
    }
}

fn require_nonzero(identities: &[[u8; 32]]) -> Result<()> {
    if identities
        .iter()
        .any(|identity| identity.iter().all(|byte| *byte == 0))
    {
        Err(RecordErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_header(bytes: &[u8], magic: [u8; 8], width: usize) -> Result<()> {
    if bytes.len() != width {
        return Err(RecordErrorV1::InvalidLength);
    }
    if array::<8>(bytes, 0)? != magic || u16_at(bytes, 8)? != WIRE_VERSION {
        return Err(RecordErrorV1::InvalidHeader);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(RecordErrorV1::InvalidLength)?,
        )
        .ok_or(RecordErrorV1::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

/// Whether the instruction data selects `DealerFound`.
#[must_use]
pub fn is_dealer_found_v1(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(FOUND_REQUEST_MAGIC.as_slice())
}

/// Whether the instruction data selects `DealerQuote`.
#[must_use]
pub fn is_dealer_quote_v1(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(QUOTE_REQUEST_MAGIC.as_slice())
}

/// Whether the instruction data selects `DealerFill`.
#[must_use]
pub fn is_dealer_fill_v1(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(FILL_REQUEST_MAGIC.as_slice())
}

/// Whether the instruction data selects `DealerWithdraw`.
#[must_use]
pub fn is_dealer_withdraw_v1(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(WITHDRAW_REQUEST_MAGIC.as_slice())
}

/// `(writable, signer)` for one coordinate of a frame, from the emitted tables.
#[must_use]
pub const fn privileges(writable: &[bool], signer: &[bool], index: usize) -> Option<(bool, bool)> {
    if index < writable.len() && index < signer.len() {
        Some((writable[index], signer[index]))
    } else {
        None
    }
}

/// The found frame's `(writable, signer)` at `index`.
#[must_use]
pub const fn found_privileges_v1(index: usize) -> Option<(bool, bool)> {
    privileges(&FOUND_WRITABLE, &FOUND_SIGNER, index)
}

/// The quote frame's `(writable, signer)` at `index`.
#[must_use]
pub const fn quote_privileges_v1(index: usize) -> Option<(bool, bool)> {
    privileges(&QUOTE_WRITABLE, &QUOTE_SIGNER, index)
}

/// The fill frame's `(writable, signer)` at `index`.
#[must_use]
pub const fn fill_privileges_v1(index: usize) -> Option<(bool, bool)> {
    privileges(&FILL_WRITABLE, &FILL_SIGNER, index)
}

/// The withdraw frame's `(writable, signer)` at `index`.
#[must_use]
pub const fn withdraw_privileges_v1(index: usize) -> Option<(bool, bool)> {
    privileges(&WITHDRAW_WRITABLE, &WITHDRAW_SIGNER, index)
}

/// Frame widths, re-exported for the routes and the planners.
pub const FOUND_FRAME_ACCOUNTS: usize = FOUND_ACCOUNT_COUNT;
/// See [`FOUND_FRAME_ACCOUNTS`].
pub const QUOTE_FRAME_ACCOUNTS: usize = QUOTE_ACCOUNT_COUNT;
/// See [`FOUND_FRAME_ACCOUNTS`].
pub const FILL_FRAME_ACCOUNTS: usize = FILL_ACCOUNT_COUNT;
/// See [`FOUND_FRAME_ACCOUNTS`].
pub const WITHDRAW_FRAME_ACCOUNTS: usize = WITHDRAW_ACCOUNT_COUNT;

/// `DCLSFDR1`: found a scoring Dealer on an Open Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFoundRequestV1 {
    /// The rule the founding seals.
    pub parameters: RuleParameters,
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// The Dealer's identity under that Market; the sponsor chooses it.
    pub dealer_id: [u8; 32],
    /// The Market's selected release set, as the activation cache is keyed.
    pub release_set: [u8; 32],
    /// Atoms the sponsor deposits; at least `subsidyOf · claim_unit_atoms`.
    pub deposit: u64,
    /// Atoms per claim unit.
    pub claim_unit_atoms: u64,
    /// The Market generation the founding is bound to.
    pub generation: u64,
}

impl DealerFoundRequestV1 {
    /// Construct and validate.
    pub fn new(self) -> Result<Self> {
        self.parameters.admit().map_err(RecordErrorV1::Rule)?;
        require_nonzero(&[self.market, self.dealer_id, self.release_set])?;
        if self.claim_unit_atoms == 0 || self.deposit == 0 {
            return Err(RecordErrorV1::NonCanonical);
        }
        Ok(self)
    }

    /// Hostile-decode.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, FOUND_REQUEST_MAGIC, FOUND_REQUEST_BYTES)?;
        require_zero(input, FOUND_REQUEST_RESERVED_OFFSET, 5)?;
        Self {
            parameters: RuleParameters {
                outcome_count: u8_at(input, FOUND_REQUEST_OUTCOME_COUNT_OFFSET)?,
                liquidity: u64_at(input, FOUND_REQUEST_LIQUIDITY_OFFSET)?,
                scale: u64_at(input, FOUND_REQUEST_SCALE_OFFSET)?,
                tolerance: u64_at(input, FOUND_REQUEST_TOLERANCE_OFFSET)?,
            },
            market: array(input, FOUND_REQUEST_MARKET_OFFSET)?,
            dealer_id: array(input, FOUND_REQUEST_DEALER_ID_OFFSET)?,
            release_set: array(input, FOUND_REQUEST_RELEASE_SET_OFFSET)?,
            deposit: u64_at(input, FOUND_REQUEST_DEPOSIT_OFFSET)?,
            claim_unit_atoms: u64_at(input, FOUND_REQUEST_CLAIM_UNIT_ATOMS_OFFSET)?,
            generation: u64_at(input, FOUND_REQUEST_GENERATION_OFFSET)?,
        }
        .new()
    }

    /// Encode the one canonical request.
    pub fn to_bytes(self) -> Result<[u8; FOUND_REQUEST_BYTES]> {
        self.new()?;
        let mut output = [0_u8; FOUND_REQUEST_BYTES];
        put(&mut output, 0, &FOUND_REQUEST_MAGIC)?;
        put(
            &mut output,
            FOUND_REQUEST_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            FOUND_REQUEST_OUTCOME_COUNT_OFFSET,
            &[self.parameters.outcome_count],
        )?;
        put(&mut output, FOUND_REQUEST_MARKET_OFFSET, &self.market)?;
        put(&mut output, FOUND_REQUEST_DEALER_ID_OFFSET, &self.dealer_id)?;
        put(
            &mut output,
            FOUND_REQUEST_RELEASE_SET_OFFSET,
            &self.release_set,
        )?;
        put(
            &mut output,
            FOUND_REQUEST_LIQUIDITY_OFFSET,
            &self.parameters.liquidity.to_le_bytes(),
        )?;
        put(
            &mut output,
            FOUND_REQUEST_SCALE_OFFSET,
            &self.parameters.scale.to_le_bytes(),
        )?;
        put(
            &mut output,
            FOUND_REQUEST_TOLERANCE_OFFSET,
            &self.parameters.tolerance.to_le_bytes(),
        )?;
        put(
            &mut output,
            FOUND_REQUEST_DEPOSIT_OFFSET,
            &self.deposit.to_le_bytes(),
        )?;
        put(
            &mut output,
            FOUND_REQUEST_CLAIM_UNIT_ATOMS_OFFSET,
            &self.claim_unit_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            FOUND_REQUEST_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// The rule record this founding seals, given the subsidy it computed.
    #[must_use]
    pub const fn rule(self, subsidy: u64) -> ScoringRuleRecordV1 {
        ScoringRuleRecordV1 {
            parameters: self.parameters,
            market: self.market,
            dealer_id: self.dealer_id,
            subsidy,
        }
    }
}

/// `DCLSQTR1`: write `p̂(inv)` into the Dealer's quote account. Permissionless.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerQuoteRequestV1 {
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// The Dealer's identity.
    pub dealer_id: [u8; 32],
    /// The fund revision the caller read; the route refuses another.
    pub expected_fund_revision: u64,
}

impl DealerQuoteRequestV1 {
    /// Construct and validate.
    pub fn new(self) -> Result<Self> {
        require_nonzero(&[self.market, self.dealer_id])?;
        Ok(self)
    }

    /// Hostile-decode.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, QUOTE_REQUEST_MAGIC, QUOTE_REQUEST_BYTES)?;
        require_zero(input, QUOTE_REQUEST_RESERVED_OFFSET, 6)?;
        Self {
            market: array(input, QUOTE_REQUEST_MARKET_OFFSET)?,
            dealer_id: array(input, QUOTE_REQUEST_DEALER_ID_OFFSET)?,
            expected_fund_revision: u64_at(input, QUOTE_REQUEST_EXPECTED_FUND_REVISION_OFFSET)?,
        }
        .new()
    }

    /// Encode the one canonical request.
    pub fn to_bytes(self) -> Result<[u8; QUOTE_REQUEST_BYTES]> {
        self.new()?;
        let mut output = [0_u8; QUOTE_REQUEST_BYTES];
        put(&mut output, 0, &QUOTE_REQUEST_MAGIC)?;
        put(
            &mut output,
            QUOTE_REQUEST_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(&mut output, QUOTE_REQUEST_MARKET_OFFSET, &self.market)?;
        put(&mut output, QUOTE_REQUEST_DEALER_ID_OFFSET, &self.dealer_id)?;
        put(
            &mut output,
            QUOTE_REQUEST_EXPECTED_FUND_REVISION_OFFSET,
            &self.expected_fund_revision.to_le_bytes(),
        )?;
        Ok(output)
    }
}

/// `DCLSFLR1`: a taker's fill against the Dealer, the batch of two.
///
/// `mint` complete sets are minted at par into the fill; the Dealer receives
/// `receive` and delivers `deliver` (`ScoringRuleV1.Fill`); the taker's
/// Position moves by `mint + deliver − receive` per outcome, the Dealer's by
/// `receive − deliver`, the aggregate by `mint` everywhere. `prices` is the
/// uniform price the fill is struck at; R2 holds it to `p̂(inv′)` within `τ`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFillRequestV1 {
    /// `K`, as the caller read it off the rule.
    pub outcome_count: u8,
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// The Dealer's identity.
    pub dealer_id: [u8; 32],
    /// The taker; the frame's one signer and the Position owner it moves.
    pub taker: [u8; 32],
    /// The fund revision the taker read.
    pub expected_fund_revision: u64,
    /// Complete sets minted at par into the fill.
    pub mint: u64,
    /// The uniform price vector, at the rule's scale.
    pub prices: Vector,
    /// What the Dealer receives, per outcome.
    pub receive: Vector,
    /// What the Dealer delivers, per outcome.
    pub deliver: Vector,
}

impl DealerFillRequestV1 {
    /// Construct and validate the wire-level shape; the rule's R0–R3 are the
    /// kernel's, not this codec's.
    pub fn new(self) -> Result<Self> {
        require_nonzero(&[self.market, self.dealer_id, self.taker])?;
        let width = usize::from(self.outcome_count);
        if width < 2 || width > super::generated::MAX_OUTCOMES {
            return Err(RecordErrorV1::Rule(super::ScoringRefusal::OutcomeCount));
        }
        for vector in [&self.prices, &self.receive, &self.deliver] {
            if vector.iter().skip(width).any(|value| *value != 0) {
                return Err(RecordErrorV1::Width);
            }
        }
        Ok(self)
    }

    /// Hostile-decode.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, FILL_REQUEST_MAGIC, FILL_REQUEST_BYTES)?;
        require_zero(input, FILL_REQUEST_RESERVED_OFFSET, 5)?;
        let outcome_count = u8_at(input, FILL_REQUEST_OUTCOME_COUNT_OFFSET)?;
        Self {
            outcome_count,
            market: array(input, FILL_REQUEST_MARKET_OFFSET)?,
            dealer_id: array(input, FILL_REQUEST_DEALER_ID_OFFSET)?,
            taker: array(input, FILL_REQUEST_TAKER_OFFSET)?,
            expected_fund_revision: u64_at(input, FILL_REQUEST_EXPECTED_FUND_REVISION_OFFSET)?,
            mint: u64_at(input, FILL_REQUEST_MINT_OFFSET)?,
            prices: read_vector(input, FILL_REQUEST_PRICES_OFFSET, outcome_count)?,
            receive: read_vector(input, FILL_REQUEST_RECEIVE_OFFSET, outcome_count)?,
            deliver: read_vector(input, FILL_REQUEST_DELIVER_OFFSET, outcome_count)?,
        }
        .new()
    }

    /// Encode the one canonical request.
    pub fn to_bytes(self) -> Result<[u8; FILL_REQUEST_BYTES]> {
        self.new()?;
        let mut output = [0_u8; FILL_REQUEST_BYTES];
        put(&mut output, 0, &FILL_REQUEST_MAGIC)?;
        put(
            &mut output,
            FILL_REQUEST_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            FILL_REQUEST_OUTCOME_COUNT_OFFSET,
            &[self.outcome_count],
        )?;
        put(&mut output, FILL_REQUEST_MARKET_OFFSET, &self.market)?;
        put(&mut output, FILL_REQUEST_DEALER_ID_OFFSET, &self.dealer_id)?;
        put(&mut output, FILL_REQUEST_TAKER_OFFSET, &self.taker)?;
        put(
            &mut output,
            FILL_REQUEST_EXPECTED_FUND_REVISION_OFFSET,
            &self.expected_fund_revision.to_le_bytes(),
        )?;
        put(
            &mut output,
            FILL_REQUEST_MINT_OFFSET,
            &self.mint.to_le_bytes(),
        )?;
        write_vector(&mut output, FILL_REQUEST_PRICES_OFFSET, &self.prices)?;
        write_vector(&mut output, FILL_REQUEST_RECEIVE_OFFSET, &self.receive)?;
        write_vector(&mut output, FILL_REQUEST_DELIVER_OFFSET, &self.deliver)?;
        Ok(output)
    }

    /// The taker's signed Position delta at `outcome`: `mint + deliver − receive`.
    #[must_use]
    pub fn taker_delta(self, outcome: usize) -> i128 {
        i128::from(self.mint) + i128::from(self.deliver[outcome])
            - i128::from(self.receive[outcome])
    }

    /// The Dealer's signed Position delta at `outcome`: `receive − deliver`.
    #[must_use]
    pub fn dealer_delta(self, outcome: usize) -> i128 {
        i128::from(self.receive[outcome]) - i128::from(self.deliver[outcome])
    }
}

/// `DCLSWDR1`: the sponsor withdraws down to the floor `Φ`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerWithdrawRequestV1 {
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// The Dealer's identity.
    pub dealer_id: [u8; 32],
    /// The fund revision the sponsor read.
    pub expected_fund_revision: u64,
    /// Atoms to withdraw.
    pub amount: u64,
}

impl DealerWithdrawRequestV1 {
    /// Construct and validate.
    pub fn new(self) -> Result<Self> {
        require_nonzero(&[self.market, self.dealer_id])?;
        if self.amount == 0 {
            return Err(RecordErrorV1::NonCanonical);
        }
        Ok(self)
    }

    /// Hostile-decode.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, WITHDRAW_REQUEST_MAGIC, WITHDRAW_REQUEST_BYTES)?;
        require_zero(input, WITHDRAW_REQUEST_RESERVED_OFFSET, 6)?;
        Self {
            market: array(input, WITHDRAW_REQUEST_MARKET_OFFSET)?,
            dealer_id: array(input, WITHDRAW_REQUEST_DEALER_ID_OFFSET)?,
            expected_fund_revision: u64_at(input, WITHDRAW_REQUEST_EXPECTED_FUND_REVISION_OFFSET)?,
            amount: u64_at(input, WITHDRAW_REQUEST_AMOUNT_OFFSET)?,
        }
        .new()
    }

    /// Encode the one canonical request.
    pub fn to_bytes(self) -> Result<[u8; WITHDRAW_REQUEST_BYTES]> {
        self.new()?;
        let mut output = [0_u8; WITHDRAW_REQUEST_BYTES];
        put(&mut output, 0, &WITHDRAW_REQUEST_MAGIC)?;
        put(
            &mut output,
            WITHDRAW_REQUEST_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(&mut output, WITHDRAW_REQUEST_MARKET_OFFSET, &self.market)?;
        put(
            &mut output,
            WITHDRAW_REQUEST_DEALER_ID_OFFSET,
            &self.dealer_id,
        )?;
        put(
            &mut output,
            WITHDRAW_REQUEST_EXPECTED_FUND_REVISION_OFFSET,
            &self.expected_fund_revision.to_le_bytes(),
        )?;
        put(
            &mut output,
            WITHDRAW_REQUEST_AMOUNT_OFFSET,
            &self.amount.to_le_bytes(),
        )?;
        Ok(output)
    }
}

/// Which route a receipt came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerRouteV1 {
    /// `DealerFound`.
    Found,
    /// `DealerQuote`.
    Quote,
    /// `DealerFill`.
    Fill,
    /// `DealerWithdraw`.
    Withdraw,
}

impl DealerRouteV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            ROUTE_FOUND => Ok(Self::Found),
            ROUTE_QUOTE => Ok(Self::Quote),
            ROUTE_FILL => Ok(Self::Fill),
            ROUTE_WITHDRAW => Ok(Self::Withdraw),
            _ => Err(RecordErrorV1::InvalidHeader),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Found => ROUTE_FOUND,
            Self::Quote => ROUTE_QUOTE,
            Self::Fill => ROUTE_FILL,
            Self::Withdraw => ROUTE_WITHDRAW,
        }
    }
}

/// `DCLSRCP1`: what every scoring Dealer route returns.
///
/// Structural decoding is insufficient: a consumer joins `request_digest` to
/// the request bytes it sent and `fund_digest` to the fund it reads back at
/// the same finalized snapshot ([`Self::authenticate_for_request`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReceiptV1 {
    /// The producing route.
    pub route: DealerRouteV1,
    /// `K`.
    pub outcome_count: u8,
    /// SHA-256 of the exact request bytes.
    pub request_digest: [u8; 32],
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// The Dealer's identity.
    pub dealer_id: [u8; 32],
    /// The fund's revision after the route.
    pub fund_revision: u64,
    /// The fund's cash after the route, atoms.
    pub cash: u64,
    /// `min inv` after the route, claim units.
    pub inventory_minimum: u64,
    /// `liquidityCost(inv)` after the route, claim units.
    pub liquidity_cost: u64,
    /// Claim units the Dealer paid (a fill), else zero.
    pub dealer_pays: u64,
    /// Claim units the Dealer received (a fill), else zero.
    pub dealer_receives: u64,
    /// SHA-256 of the fund record's bytes after the route.
    pub fund_digest: [u8; 32],
}

impl DealerReceiptV1 {
    /// The receipt a route produces from the fund it just wrote.
    #[must_use]
    pub fn from_fund(
        route: DealerRouteV1,
        request_bytes: &[u8],
        fund: DealerFundV1,
        fund_bytes: &[u8],
        dealer_pays: u64,
        dealer_receives: u64,
    ) -> Self {
        Self {
            route,
            outcome_count: fund.outcome_count,
            request_digest: digest(request_bytes),
            market: fund.market,
            dealer_id: fund.dealer_id,
            fund_revision: fund.revision,
            cash: fund.cash,
            inventory_minimum: fund.inventory_minimum,
            liquidity_cost: fund.liquidity_cost,
            dealer_pays,
            dealer_receives,
            fund_digest: digest(fund_bytes),
        }
    }

    /// Hostile-decode.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, RECEIPT_MAGIC, RECEIPT_BYTES)?;
        require_zero(input, RECEIPT_RESERVED_OFFSET, 4)?;
        let value = Self {
            route: DealerRouteV1::decode(u8_at(input, RECEIPT_ROUTE_OFFSET)?)?,
            outcome_count: u8_at(input, RECEIPT_OUTCOME_COUNT_OFFSET)?,
            request_digest: array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            market: array(input, RECEIPT_MARKET_OFFSET)?,
            dealer_id: array(input, RECEIPT_DEALER_ID_OFFSET)?,
            fund_revision: u64_at(input, RECEIPT_FUND_REVISION_OFFSET)?,
            cash: u64_at(input, RECEIPT_CASH_OFFSET)?,
            inventory_minimum: u64_at(input, RECEIPT_INVENTORY_MINIMUM_OFFSET)?,
            liquidity_cost: u64_at(input, RECEIPT_LIQUIDITY_COST_OFFSET)?,
            dealer_pays: u64_at(input, RECEIPT_DEALER_PAYS_OFFSET)?,
            dealer_receives: u64_at(input, RECEIPT_DEALER_RECEIVES_OFFSET)?,
            fund_digest: array(input, RECEIPT_FUND_DIGEST_OFFSET)?,
        };
        require_nonzero(&[
            value.request_digest,
            value.market,
            value.dealer_id,
            value.fund_digest,
        ])?;
        Ok(value)
    }

    /// Encode the one canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; RECEIPT_BYTES]> {
        require_nonzero(&[
            self.request_digest,
            self.market,
            self.dealer_id,
            self.fund_digest,
        ])?;
        let mut output = [0_u8; RECEIPT_BYTES];
        put(&mut output, 0, &RECEIPT_MAGIC)?;
        put(
            &mut output,
            RECEIPT_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(&mut output, RECEIPT_ROUTE_OFFSET, &[self.route.tag()])?;
        put(
            &mut output,
            RECEIPT_OUTCOME_COUNT_OFFSET,
            &[self.outcome_count],
        )?;
        put(
            &mut output,
            RECEIPT_REQUEST_DIGEST_OFFSET,
            &self.request_digest,
        )?;
        put(&mut output, RECEIPT_MARKET_OFFSET, &self.market)?;
        put(&mut output, RECEIPT_DEALER_ID_OFFSET, &self.dealer_id)?;
        put(
            &mut output,
            RECEIPT_FUND_REVISION_OFFSET,
            &self.fund_revision.to_le_bytes(),
        )?;
        put(&mut output, RECEIPT_CASH_OFFSET, &self.cash.to_le_bytes())?;
        put(
            &mut output,
            RECEIPT_INVENTORY_MINIMUM_OFFSET,
            &self.inventory_minimum.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_LIQUIDITY_COST_OFFSET,
            &self.liquidity_cost.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_DEALER_PAYS_OFFSET,
            &self.dealer_pays.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_DEALER_RECEIVES_OFFSET,
            &self.dealer_receives.to_le_bytes(),
        )?;
        put(&mut output, RECEIPT_FUND_DIGEST_OFFSET, &self.fund_digest)?;
        Ok(output)
    }

    /// Join this receipt to the request bytes sent and the fund read back.
    pub fn authenticate_for_request(
        self,
        request_bytes: &[u8],
        observed_fund_bytes: &[u8],
    ) -> Result<Self> {
        let fund = DealerFundV1::decode(observed_fund_bytes)?;
        let expected = Self::from_fund(
            self.route,
            request_bytes,
            fund,
            observed_fund_bytes,
            self.dealer_pays,
            self.dealer_receives,
        );
        if self != expected {
            return Err(RecordErrorV1::NonCanonical);
        }
        Ok(self)
    }
}

/// `DCLSFLW1`: what the accelerator's Dealer arm evaluates a fill against --
/// the sealed rule, the inventory the Position holds, and the fund scalars
/// the request named. Trading composes it from authenticated accounts; the
/// arm re-runs the same kernel over it and writes a [`DealerReceiptV1`]-shaped
/// candidate the route compares byte for byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFillWitnessV1 {
    /// The sealed rule.
    pub rule: ScoringRuleRecordV1,
    /// The Dealer's inventory at admission.
    pub inventory: Vector,
    /// The fund revision the fill is checked against.
    pub fund_revision: u64,
    /// The fund's cash, atoms.
    pub cash: u64,
}

impl DealerFillWitnessV1 {
    /// Hostile-decode.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, FILL_WITNESS_MAGIC, FILL_WITNESS_BYTES)?;
        require_zero(input, FILL_WITNESS_RESERVED_OFFSET, 5)?;
        let outcome_count = u8_at(input, FILL_WITNESS_OUTCOME_COUNT_OFFSET)?;
        let rule = ScoringRuleRecordV1::decode(
            input
                .get(FILL_WITNESS_RULE_OFFSET..FILL_WITNESS_RULE_OFFSET + RULE_BYTES)
                .ok_or(RecordErrorV1::InvalidLength)?,
        )?;
        if rule.parameters.outcome_count != outcome_count {
            return Err(RecordErrorV1::NonCanonical);
        }
        Ok(Self {
            rule,
            inventory: read_vector(input, FILL_WITNESS_INVENTORY_OFFSET, outcome_count)?,
            fund_revision: u64_at(input, FILL_WITNESS_FUND_REVISION_OFFSET)?,
            cash: u64_at(input, FILL_WITNESS_CASH_OFFSET)?,
        })
    }

    /// Encode the one canonical witness.
    pub fn to_bytes(self) -> Result<[u8; FILL_WITNESS_BYTES]> {
        let mut output = [0_u8; FILL_WITNESS_BYTES];
        put(&mut output, 0, &FILL_WITNESS_MAGIC)?;
        put(
            &mut output,
            FILL_WITNESS_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            FILL_WITNESS_OUTCOME_COUNT_OFFSET,
            &[self.rule.parameters.outcome_count],
        )?;
        put(
            &mut output,
            FILL_WITNESS_RULE_OFFSET,
            &self.rule.to_bytes()?,
        )?;
        write_vector(&mut output, FILL_WITNESS_INVENTORY_OFFSET, &self.inventory)?;
        put(
            &mut output,
            FILL_WITNESS_FUND_REVISION_OFFSET,
            &self.fund_revision.to_le_bytes(),
        )?;
        put(
            &mut output,
            FILL_WITNESS_CASH_OFFSET,
            &self.cash.to_le_bytes(),
        )?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::super::ZERO_VECTOR;
    use super::super::records_v1::FundPhaseV1;
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn parameters() -> RuleParameters {
        RuleParameters {
            outcome_count: 2,
            liquidity: 1 << 30,
            scale: 1 << 62,
            tolerance: 1,
        }
    }

    #[test]
    fn the_four_predicates_select_by_magic_alone() {
        let found = DealerFoundRequestV1 {
            parameters: parameters(),
            market: id(1),
            dealer_id: id(2),
            release_set: id(3),
            deposit: 10,
            claim_unit_atoms: 1,
            generation: 0,
        }
        .to_bytes()
        .expect("found");
        let quote = DealerQuoteRequestV1 {
            market: id(1),
            dealer_id: id(2),
            expected_fund_revision: 0,
        }
        .to_bytes()
        .expect("quote");
        let fill = DealerFillRequestV1 {
            outcome_count: 2,
            market: id(1),
            dealer_id: id(2),
            taker: id(4),
            expected_fund_revision: 0,
            mint: 0,
            prices: ZERO_VECTOR,
            receive: ZERO_VECTOR,
            deliver: ZERO_VECTOR,
        }
        .to_bytes()
        .expect("fill");
        let withdraw = DealerWithdrawRequestV1 {
            market: id(1),
            dealer_id: id(2),
            expected_fund_revision: 0,
            amount: 1,
        }
        .to_bytes()
        .expect("withdraw");
        assert!(is_dealer_found_v1(&found) && !is_dealer_quote_v1(&found));
        assert!(is_dealer_quote_v1(&quote) && !is_dealer_fill_v1(&quote));
        assert!(is_dealer_fill_v1(&fill) && !is_dealer_withdraw_v1(&fill));
        assert!(is_dealer_withdraw_v1(&withdraw) && !is_dealer_found_v1(&withdraw));
        assert_eq!(found.len(), FOUND_REQUEST_BYTES);
        assert_eq!(quote.len(), QUOTE_REQUEST_BYTES);
        assert_eq!(fill.len(), FILL_REQUEST_BYTES);
        assert_eq!(withdraw.len(), WITHDRAW_REQUEST_BYTES);
        assert_eq!(
            DealerFoundRequestV1::decode(&found)
                .expect("decode")
                .deposit,
            10
        );
        assert_eq!(
            DealerFillRequestV1::decode(&fill).expect("decode").taker,
            id(4)
        );
        assert_eq!(
            DealerWithdrawRequestV1::decode(&withdraw)
                .expect("decode")
                .amount,
            1
        );
    }

    #[test]
    fn the_frames_have_one_signer_at_zero() {
        for (count, privileges) in [
            (
                FOUND_FRAME_ACCOUNTS,
                found_privileges_v1 as fn(usize) -> Option<(bool, bool)>,
            ),
            (QUOTE_FRAME_ACCOUNTS, quote_privileges_v1),
            (FILL_FRAME_ACCOUNTS, fill_privileges_v1),
            (WITHDRAW_FRAME_ACCOUNTS, withdraw_privileges_v1),
        ] {
            assert_eq!(privileges(0).map(|(_, signer)| signer), Some(true));
            for index in 1..count {
                assert_eq!(privileges(index).map(|(_, signer)| signer), Some(false));
            }
            assert_eq!(privileges(count), None);
        }
    }

    #[test]
    fn a_fill_with_a_nonzero_inactive_coordinate_is_refused() {
        let mut receive = ZERO_VECTOR;
        receive[3] = 1;
        assert_eq!(
            DealerFillRequestV1 {
                outcome_count: 2,
                market: id(1),
                dealer_id: id(2),
                taker: id(4),
                expected_fund_revision: 0,
                mint: 0,
                prices: ZERO_VECTOR,
                receive,
                deliver: ZERO_VECTOR,
            }
            .new(),
            Err(RecordErrorV1::Width)
        );
    }

    #[test]
    fn the_receipt_joins_the_request_and_the_fund() {
        let fund = DealerFundV1 {
            outcome_count: 2,
            phase: FundPhaseV1::Open,
            market: id(1),
            dealer_id: id(2),
            sponsor: id(3),
            rule_digest: id(4),
            vault: id(5),
            claim_unit_atoms: 1,
            cash: 100,
            inventory_minimum: 0,
            liquidity_cost: 7,
            revision: 3,
            bump: 250,
        };
        let fund_bytes = fund.to_bytes().expect("fund");
        let request = DealerQuoteRequestV1 {
            market: id(1),
            dealer_id: id(2),
            expected_fund_revision: 3,
        }
        .to_bytes()
        .expect("request");
        let receipt =
            DealerReceiptV1::from_fund(DealerRouteV1::Quote, &request, fund, &fund_bytes, 0, 0);
        let bytes = receipt.to_bytes().expect("receipt");
        assert_eq!(bytes.len(), RECEIPT_BYTES);
        let decoded = DealerReceiptV1::decode(&bytes).expect("decode");
        assert_eq!(
            decoded.authenticate_for_request(&request, &fund_bytes),
            Ok(receipt)
        );
        let other = DealerFundV1 {
            revision: 4,
            ..fund
        }
        .to_bytes()
        .expect("other");
        assert_eq!(
            decoded.authenticate_for_request(&request, &other),
            Err(RecordErrorV1::NonCanonical)
        );
    }

    #[test]
    fn the_witness_round_trips() {
        let witness = DealerFillWitnessV1 {
            rule: ScoringRuleRecordV1 {
                parameters: parameters(),
                market: id(1),
                dealer_id: id(2),
                subsidy: 1_073_741_825,
            },
            inventory: ZERO_VECTOR,
            fund_revision: 1,
            cash: 2,
        };
        let bytes = witness.to_bytes().expect("witness");
        assert_eq!(bytes.len(), FILL_WITNESS_BYTES);
        assert_eq!(DealerFillWitnessV1::decode(&bytes), Ok(witness));
    }
}
