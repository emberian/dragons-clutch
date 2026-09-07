//! The three records the scoring Dealer persists, Lean-first
//! (`ScoringRuleAbiV1.lean`): the sealed rule (`DCLSCR01`, 112 bytes), the
//! fund (`DCLSFUN1`, 224) and the quote (`DCLSQUO1`, 240). Every offset and
//! width is the emitted constant; nothing here types a number.
//!
//! All three are Trading-owned PDAs under the Trading program:
//! `[RULE_PDA_DOMAIN, market, dealer_id]`, `[FUND_PDA_DOMAIN, market,
//! dealer_id]`, `[QUOTE_PDA_DOMAIN, market, dealer_id]`. The rule is written
//! once at founding and never again (its digest is carried by the fund, so a
//! request naming another rule has no fund); the fund moves on every fill and
//! withdrawal under an optimistic `revision`; the quote is rewritten in place.

use super::generated::{
    FUND_BUMP_OFFSET, FUND_BYTES, FUND_CASH_OFFSET, FUND_CLAIM_UNIT_ATOMS_OFFSET,
    FUND_DEALER_ID_OFFSET, FUND_INVENTORY_MINIMUM_OFFSET, FUND_LIQUIDITY_COST_OFFSET, FUND_MAGIC,
    FUND_MARKET_ID_OFFSET, FUND_OUTCOME_COUNT_OFFSET, FUND_PDA_DOMAIN, FUND_PHASE_OFFSET,
    FUND_PHASE_OPEN, FUND_PHASE_RETIRED, FUND_RESERVED_OFFSET, FUND_RESERVED_TAIL_OFFSET,
    FUND_REVISION_OFFSET, FUND_RULE_DIGEST_OFFSET, FUND_SPONSOR_OFFSET, FUND_VAULT_OFFSET,
    FUND_VERSION_OFFSET, MAX_OUTCOMES, QUOTE_BUMP_OFFSET, QUOTE_BYTES, QUOTE_DEALER_ID_OFFSET,
    QUOTE_FUND_REVISION_OFFSET, QUOTE_MAGIC, QUOTE_MARKET_ID_OFFSET, QUOTE_OUTCOME_COUNT_OFFSET,
    QUOTE_PDA_DOMAIN, QUOTE_PRICES_OFFSET, QUOTE_RESERVED_OFFSET, QUOTE_RESERVED_TAIL_OFFSET,
    QUOTE_SCALE_OFFSET, QUOTE_SLOT_OFFSET, QUOTE_VERSION_OFFSET, RULE_BYTES, RULE_DEALER_ID_OFFSET,
    RULE_LIQUIDITY_OFFSET, RULE_MAGIC, RULE_MARKET_ID_OFFSET, RULE_OUTCOME_COUNT_OFFSET,
    RULE_PDA_DOMAIN, RULE_RESERVED_OFFSET, RULE_SCALE_OFFSET, RULE_SUBSIDY_OFFSET,
    RULE_TOLERANCE_OFFSET, RULE_VERSION, RULE_VERSION_OFFSET, WIRE_VERSION,
};
use super::{Potential, RuleParameters, ScoringRefusal, Vector, ZERO_VECTOR};

/// Stable refusal from record decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordErrorV1 {
    /// The bytes are not the record's one exact width.
    InvalidLength,
    /// Magic or version named another record.
    InvalidHeader,
    /// A reserved span was not zero.
    NonCanonical,
    /// A required identity was zero.
    ZeroIdentity,
    /// A phase byte named no phase.
    UnknownPhase,
    /// The rule's parameters are not admitted (`parametersAdmissible`).
    Rule(ScoringRefusal),
    /// A vector coordinate at or past the outcome count was nonzero.
    Width,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, RecordErrorV1>;

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

fn require_header(bytes: &[u8], magic: [u8; 8], width: usize, version: u16) -> Result<()> {
    if bytes.len() != width {
        return Err(RecordErrorV1::InvalidLength);
    }
    if array::<8>(bytes, 0)? != magic || u16_at(bytes, 8)? != version {
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

/// Read one fixed-capacity vector, requiring every coordinate at or past
/// `outcome_count` to be zero.
pub fn read_vector(bytes: &[u8], offset: usize, outcome_count: u8) -> Result<Vector> {
    let mut vector = ZERO_VECTOR;
    for (index, slot) in vector.iter_mut().enumerate() {
        *slot = u64_at(bytes, offset + index * 8)?;
        if index >= usize::from(outcome_count) && *slot != 0 {
            return Err(RecordErrorV1::Width);
        }
    }
    Ok(vector)
}

/// Write one fixed-capacity vector.
pub fn write_vector(output: &mut [u8], offset: usize, vector: &Vector) -> Result<()> {
    for (index, value) in vector.iter().enumerate() {
        put(output, offset + index * 8, &value.to_le_bytes())?;
    }
    Ok(())
}

/// The sealed rule: the cost function IS this record (`ScoringRuleV1.ruleSchema`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoringRuleRecordV1 {
    /// The rule's parameters.
    pub parameters: RuleParameters,
    /// Logical Core Market.
    pub market: [u8; 32],
    /// The Dealer's identity under that Market (one Market may host several).
    pub dealer_id: [u8; 32],
    /// `subsidyOf(b, K)`, computed once at founding and carried so no reader
    /// re-derives it and every reader can check it.
    pub subsidy: u64,
}

impl ScoringRuleRecordV1 {
    /// Hostile-decode one exact rule record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(bytes, RULE_MAGIC, RULE_BYTES, RULE_VERSION)?;
        require_zero(bytes, RULE_RESERVED_OFFSET, 5)?;
        let parameters = RuleParameters {
            outcome_count: u8_at(bytes, RULE_OUTCOME_COUNT_OFFSET)?,
            liquidity: u64_at(bytes, RULE_LIQUIDITY_OFFSET)?,
            scale: u64_at(bytes, RULE_SCALE_OFFSET)?,
            tolerance: u64_at(bytes, RULE_TOLERANCE_OFFSET)?,
        }
        .admit()
        .map_err(RecordErrorV1::Rule)?;
        let value = Self {
            parameters,
            market: array(bytes, RULE_MARKET_ID_OFFSET)?,
            dealer_id: array(bytes, RULE_DEALER_ID_OFFSET)?,
            subsidy: u64_at(bytes, RULE_SUBSIDY_OFFSET)?,
        };
        require_nonzero(&[value.market, value.dealer_id])?;
        Ok(value)
    }

    /// Encode the one canonical rule record.
    pub fn to_bytes(self) -> Result<[u8; RULE_BYTES]> {
        self.parameters.admit().map_err(RecordErrorV1::Rule)?;
        require_nonzero(&[self.market, self.dealer_id])?;
        let mut output = [0_u8; RULE_BYTES];
        put(&mut output, 0, &RULE_MAGIC)?;
        put(
            &mut output,
            RULE_VERSION_OFFSET,
            &RULE_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            RULE_OUTCOME_COUNT_OFFSET,
            &[self.parameters.outcome_count],
        )?;
        put(&mut output, RULE_MARKET_ID_OFFSET, &self.market)?;
        put(&mut output, RULE_DEALER_ID_OFFSET, &self.dealer_id)?;
        put(
            &mut output,
            RULE_LIQUIDITY_OFFSET,
            &self.parameters.liquidity.to_le_bytes(),
        )?;
        put(
            &mut output,
            RULE_SCALE_OFFSET,
            &self.parameters.scale.to_le_bytes(),
        )?;
        put(
            &mut output,
            RULE_TOLERANCE_OFFSET,
            &self.parameters.tolerance.to_le_bytes(),
        )?;
        put(
            &mut output,
            RULE_SUBSIDY_OFFSET,
            &self.subsidy.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// The rule's PDA seeds under the Trading program, excluding the bump.
    pub fn seeds<'a>(market: &'a [u8; 32], dealer_id: &'a [u8; 32]) -> [&'a [u8]; 3] {
        [RULE_PDA_DOMAIN, market, dealer_id]
    }
}

/// The fund's lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundPhaseV1 {
    /// Quoting, filling and withdrawing to the floor.
    Open,
    /// The Market is terminal and the residue has left; nothing moves again.
    Retired,
}

impl FundPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            FUND_PHASE_OPEN => Ok(Self::Open),
            FUND_PHASE_RETIRED => Ok(Self::Retired),
            _ => Err(RecordErrorV1::UnknownPhase),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Open => FUND_PHASE_OPEN,
            Self::Retired => FUND_PHASE_RETIRED,
        }
    }
}

/// The Dealer's fund: its cash, and `Ŵ` as the pair the chain reads back
/// without a logarithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFundV1 {
    /// `K`.
    pub outcome_count: u8,
    /// Lifecycle.
    pub phase: FundPhaseV1,
    /// Logical Core Market.
    pub market: [u8; 32],
    /// The Dealer's identity under that Market.
    pub dealer_id: [u8; 32],
    /// The refund source: whoever founded the fund, and the only signer a
    /// withdrawal admits.
    pub sponsor: [u8; 32],
    /// SHA-256 of the sealed rule record's bytes.
    pub rule_digest: [u8; 32],
    /// The `TradingPrincipal` Custody vault holding `cash`.
    pub vault: [u8; 32],
    /// Atoms per claim unit (`ProductBasisV3::payout_scale`).
    pub claim_unit_atoms: u64,
    /// Present cash, atoms, equal to the vault's balance at every boundary.
    pub cash: u64,
    /// `min_i inv_i` at the last settlement, claim units.
    pub inventory_minimum: u64,
    /// `liquidityCost(inv)` at the last settlement, claim units.
    pub liquidity_cost: u64,
    /// Optimistic revision; every request names the one it read.
    pub revision: u64,
    /// The fund PDA's own bump, persisted so no reader searches.
    pub bump: u8,
}

impl DealerFundV1 {
    /// Hostile-decode one exact fund record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(bytes, FUND_MAGIC, FUND_BYTES, WIRE_VERSION)?;
        require_zero(bytes, FUND_RESERVED_OFFSET, 4)?;
        require_zero(bytes, FUND_RESERVED_TAIL_OFFSET, 7)?;
        let value = Self {
            outcome_count: u8_at(bytes, FUND_OUTCOME_COUNT_OFFSET)?,
            phase: FundPhaseV1::decode(u8_at(bytes, FUND_PHASE_OFFSET)?)?,
            market: array(bytes, FUND_MARKET_ID_OFFSET)?,
            dealer_id: array(bytes, FUND_DEALER_ID_OFFSET)?,
            sponsor: array(bytes, FUND_SPONSOR_OFFSET)?,
            rule_digest: array(bytes, FUND_RULE_DIGEST_OFFSET)?,
            vault: array(bytes, FUND_VAULT_OFFSET)?,
            claim_unit_atoms: u64_at(bytes, FUND_CLAIM_UNIT_ATOMS_OFFSET)?,
            cash: u64_at(bytes, FUND_CASH_OFFSET)?,
            inventory_minimum: u64_at(bytes, FUND_INVENTORY_MINIMUM_OFFSET)?,
            liquidity_cost: u64_at(bytes, FUND_LIQUIDITY_COST_OFFSET)?,
            revision: u64_at(bytes, FUND_REVISION_OFFSET)?,
            bump: u8_at(bytes, FUND_BUMP_OFFSET)?,
        };
        require_nonzero(&[
            value.market,
            value.dealer_id,
            value.sponsor,
            value.rule_digest,
            value.vault,
        ])?;
        if value.claim_unit_atoms == 0 {
            return Err(RecordErrorV1::NonCanonical);
        }
        Ok(value)
    }

    /// Encode the one canonical fund record.
    pub fn to_bytes(self) -> Result<[u8; FUND_BYTES]> {
        require_nonzero(&[
            self.market,
            self.dealer_id,
            self.sponsor,
            self.rule_digest,
            self.vault,
        ])?;
        if self.claim_unit_atoms == 0 {
            return Err(RecordErrorV1::NonCanonical);
        }
        let mut output = [0_u8; FUND_BYTES];
        put(&mut output, 0, &FUND_MAGIC)?;
        put(
            &mut output,
            FUND_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            FUND_OUTCOME_COUNT_OFFSET,
            &[self.outcome_count],
        )?;
        put(&mut output, FUND_PHASE_OFFSET, &[self.phase.tag()])?;
        put(&mut output, FUND_MARKET_ID_OFFSET, &self.market)?;
        put(&mut output, FUND_DEALER_ID_OFFSET, &self.dealer_id)?;
        put(&mut output, FUND_SPONSOR_OFFSET, &self.sponsor)?;
        put(&mut output, FUND_RULE_DIGEST_OFFSET, &self.rule_digest)?;
        put(&mut output, FUND_VAULT_OFFSET, &self.vault)?;
        put(
            &mut output,
            FUND_CLAIM_UNIT_ATOMS_OFFSET,
            &self.claim_unit_atoms.to_le_bytes(),
        )?;
        put(&mut output, FUND_CASH_OFFSET, &self.cash.to_le_bytes())?;
        put(
            &mut output,
            FUND_INVENTORY_MINIMUM_OFFSET,
            &self.inventory_minimum.to_le_bytes(),
        )?;
        put(
            &mut output,
            FUND_LIQUIDITY_COST_OFFSET,
            &self.liquidity_cost.to_le_bytes(),
        )?;
        put(
            &mut output,
            FUND_REVISION_OFFSET,
            &self.revision.to_le_bytes(),
        )?;
        put(&mut output, FUND_BUMP_OFFSET, &[self.bump])?;
        Ok(output)
    }

    /// `Ŵ` as the fund carries it.
    #[must_use]
    pub const fn potential(self) -> Potential {
        Potential {
            minimum: self.inventory_minimum,
            cost: self.liquidity_cost,
        }
    }

    /// The fund's cash in claim units, rounded down: the only atoms-to-claims
    /// conversion the routes make.
    #[must_use]
    pub const fn cash_claims(self) -> u64 {
        self.cash / self.claim_unit_atoms
    }

    /// Claim units to atoms, checked.
    pub fn atoms(self, claims: u64) -> core::result::Result<u64, ScoringRefusal> {
        claims
            .checked_mul(self.claim_unit_atoms)
            .ok_or(ScoringRefusal::Overflow)
    }

    /// The Dealer's debit in atoms for an admitted fill's `(pays, receives)`.
    pub fn debit_atoms(
        self,
        dealer_pays: u64,
        dealer_receives: u64,
    ) -> core::result::Result<(u64, u64), ScoringRefusal> {
        Ok((self.atoms(dealer_pays)?, self.atoms(dealer_receives)?))
    }

    /// The fund's PDA seeds under the Trading program, excluding the bump.
    pub fn seeds<'a>(market: &'a [u8; 32], dealer_id: &'a [u8; 32]) -> [&'a [u8]; 3] {
        [FUND_PDA_DOMAIN, market, dealer_id]
    }
}

/// The last quote written for a Dealer: `p̂(inv)` at the fund revision it
/// was derived at. Fresh iff `fund_revision` equals the fund's revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerQuoteV1 {
    /// `K`.
    pub outcome_count: u8,
    /// Logical Core Market.
    pub market: [u8; 32],
    /// The Dealer's identity.
    pub dealer_id: [u8; 32],
    /// The fund revision the inventory was read at.
    pub fund_revision: u64,
    /// The slot the quote was written.
    pub slot: u64,
    /// The rule's scale (the prices' denominator).
    pub scale: u64,
    /// `p̂(inv)`.
    pub prices: Vector,
    /// The quote PDA's own bump.
    pub bump: u8,
}

impl DealerQuoteV1 {
    /// Hostile-decode one exact quote record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(bytes, QUOTE_MAGIC, QUOTE_BYTES, WIRE_VERSION)?;
        require_zero(bytes, QUOTE_RESERVED_OFFSET, 5)?;
        require_zero(bytes, QUOTE_RESERVED_TAIL_OFFSET, 7)?;
        let outcome_count = u8_at(bytes, QUOTE_OUTCOME_COUNT_OFFSET)?;
        let value = Self {
            outcome_count,
            market: array(bytes, QUOTE_MARKET_ID_OFFSET)?,
            dealer_id: array(bytes, QUOTE_DEALER_ID_OFFSET)?,
            fund_revision: u64_at(bytes, QUOTE_FUND_REVISION_OFFSET)?,
            slot: u64_at(bytes, QUOTE_SLOT_OFFSET)?,
            scale: u64_at(bytes, QUOTE_SCALE_OFFSET)?,
            prices: read_vector(bytes, QUOTE_PRICES_OFFSET, outcome_count)?,
            bump: u8_at(bytes, QUOTE_BUMP_OFFSET)?,
        };
        require_nonzero(&[value.market, value.dealer_id])?;
        Ok(value)
    }

    /// Encode the one canonical quote record.
    pub fn to_bytes(self) -> Result<[u8; QUOTE_BYTES]> {
        require_nonzero(&[self.market, self.dealer_id])?;
        if usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(RecordErrorV1::Width);
        }
        let mut output = [0_u8; QUOTE_BYTES];
        put(&mut output, 0, &QUOTE_MAGIC)?;
        put(
            &mut output,
            QUOTE_VERSION_OFFSET,
            &WIRE_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            QUOTE_OUTCOME_COUNT_OFFSET,
            &[self.outcome_count],
        )?;
        put(&mut output, QUOTE_MARKET_ID_OFFSET, &self.market)?;
        put(&mut output, QUOTE_DEALER_ID_OFFSET, &self.dealer_id)?;
        put(
            &mut output,
            QUOTE_FUND_REVISION_OFFSET,
            &self.fund_revision.to_le_bytes(),
        )?;
        put(&mut output, QUOTE_SLOT_OFFSET, &self.slot.to_le_bytes())?;
        put(&mut output, QUOTE_SCALE_OFFSET, &self.scale.to_le_bytes())?;
        write_vector(&mut output, QUOTE_PRICES_OFFSET, &self.prices)?;
        put(&mut output, QUOTE_BUMP_OFFSET, &[self.bump])?;
        Ok(output)
    }

    /// Whether this quote was derived at the fund's present revision.
    #[must_use]
    pub const fn is_fresh(self, fund: DealerFundV1) -> bool {
        self.fund_revision == fund.revision
    }

    /// The quote's PDA seeds under the Trading program, excluding the bump.
    pub fn seeds<'a>(market: &'a [u8; 32], dealer_id: &'a [u8; 32]) -> [&'a [u8]; 3] {
        [QUOTE_PDA_DOMAIN, market, dealer_id]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn rule() -> ScoringRuleRecordV1 {
        ScoringRuleRecordV1 {
            parameters: RuleParameters {
                outcome_count: 3,
                liquidity: 1 << 20,
                scale: 1 << 62,
                tolerance: 1,
            },
            market: id(1),
            dealer_id: id(2),
            subsidy: 1_661_954,
        }
    }

    #[test]
    fn rule_round_trips_and_refuses_the_hostiles() {
        let bytes = rule().to_bytes().expect("encode");
        assert_eq!(bytes.len(), RULE_BYTES);
        assert_eq!(ScoringRuleRecordV1::decode(&bytes), Ok(rule()));
        let mut short = bytes.to_vec();
        short.pop();
        assert_eq!(
            ScoringRuleRecordV1::decode(&short),
            Err(RecordErrorV1::InvalidLength)
        );
        let mut reserved = bytes;
        reserved[RULE_RESERVED_OFFSET] = 1;
        assert_eq!(
            ScoringRuleRecordV1::decode(&reserved),
            Err(RecordErrorV1::NonCanonical)
        );
        let mut wide = bytes;
        wide[RULE_OUTCOME_COUNT_OFFSET] = 17;
        assert_eq!(
            ScoringRuleRecordV1::decode(&wide),
            Err(RecordErrorV1::Rule(ScoringRefusal::OutcomeCount))
        );
    }

    #[test]
    fn fund_round_trips_and_reads_its_potential() {
        let fund = DealerFundV1 {
            outcome_count: 3,
            phase: FundPhaseV1::Open,
            market: id(1),
            dealer_id: id(2),
            sponsor: id(3),
            rule_digest: id(4),
            vault: id(5),
            claim_unit_atoms: 3,
            cash: 9_000,
            inventory_minimum: 0,
            liquidity_cost: 1_661_954,
            revision: 7,
            bump: 254,
        };
        let bytes = fund.to_bytes().expect("encode");
        assert_eq!(bytes.len(), FUND_BYTES);
        assert_eq!(DealerFundV1::decode(&bytes), Ok(fund));
        assert_eq!(fund.potential().value(), -1_661_954);
        assert_eq!(fund.cash_claims(), 3_000);
        assert_eq!(fund.debit_atoms(5, 0), Ok((15, 0)));
        let mut phase = bytes;
        phase[FUND_PHASE_OFFSET] = 9;
        assert_eq!(
            DealerFundV1::decode(&phase),
            Err(RecordErrorV1::UnknownPhase)
        );
    }

    #[test]
    fn quote_round_trips_and_knows_its_freshness() {
        let mut prices = ZERO_VECTOR;
        prices[0] = 1 << 61;
        prices[1] = 1 << 61;
        let quote = DealerQuoteV1 {
            outcome_count: 2,
            market: id(1),
            dealer_id: id(2),
            fund_revision: 7,
            slot: 100,
            scale: 1 << 62,
            prices,
            bump: 253,
        };
        let bytes = quote.to_bytes().expect("encode");
        assert_eq!(bytes.len(), QUOTE_BYTES);
        assert_eq!(DealerQuoteV1::decode(&bytes), Ok(quote));
        let mut wide = bytes;
        wide[QUOTE_PRICES_OFFSET + 2 * 8] = 1;
        assert_eq!(DealerQuoteV1::decode(&wide), Err(RecordErrorV1::Width));
        let fund = DealerFundV1 {
            outcome_count: 2,
            phase: FundPhaseV1::Open,
            market: id(1),
            dealer_id: id(2),
            sponsor: id(3),
            rule_digest: id(4),
            vault: id(5),
            claim_unit_atoms: 1,
            cash: 1,
            inventory_minimum: 0,
            liquidity_cost: 1,
            revision: 7,
            bump: 1,
        };
        assert!(quote.is_fresh(fund));
        assert!(!quote.is_fresh(DealerFundV1 {
            revision: 8,
            ..fund
        }));
    }
}
