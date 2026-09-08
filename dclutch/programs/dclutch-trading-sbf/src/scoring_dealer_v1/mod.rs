//! The scoring Dealer's four top-level routes: found, quote, fill, withdraw.
//!
//! One participant, one sealed rule (`ScoringRuleV1.lean`), four permissioned
//! movements. Every route is selected by its magic alone (`DCLSFDR1`,
//! `DCLSQTR1`, `DCLSFLR1`, `DCLSWDR1`), parses a fixed PREFIX of accounts the
//! Lean states (`ScoringRuleAbiV1.foundFrame` ...), and hands the child
//! WINDOWS that follow it -- exact Custody and Claims frames, in a stated
//! order -- to the child programs, which refuse their own frames by their own
//! names. This program authenticates what is its own: the Market, the fund,
//! the rule, the Dealer's Position, the release, and the rule's arithmetic;
//! Custody and Claims authenticate theirs.
//!
//! The kernel is linked, not called across a CPI: `admit_fill` runs here at
//! the note's 39k / 46k / 93k CU for K = 2 / 3 / 5, and the accelerator's
//! Dealer arm runs the same function over the same witness for a verifier
//! that already goes through the accelerator (`dclutch-accelerator-sbf`'s
//! `scoring` arm).
//!
//! Refusals are the sub-band `0x4200` inside Trading's band (decision 0007),
//! one code per conjunct; the kernel's causes map one to one.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims::frame_spec_v1::{ClaimsFrameSpecV1, SignedDeltaFrameSpecV3};
use dclutch_claims::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_claims::protocol_position_v2::{ProtocolPositionActionV2, ProtocolPositionSeedsV2};
use dclutch_custody::token_svm::instruction::approve_checked;
use dclutch_custody::token_svm::{InstructionSpec, Mint};
use dclutch_custody::{
    CUSTODY_BUMP_RELAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyFrameSpecV1,
    CustodyReceiptV1, CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1,
    DelegatedCustodyReceiptV2, DelegatedCustodyRequestV2, OperationV1, TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::activation_auth_v1::{
    authenticate_activation_cache_identity_v1, require_cache_account,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::scoring_rule::records_v1::{
    DealerFundV1, DealerQuoteV1, FundPhaseV1, RecordErrorV1, ScoringRuleRecordV1,
};
use dclutch_trading::scoring_rule::requests_v1::{DealerReceiptV1, DealerRouteV1};
use dclutch_trading::scoring_rule::{ScoringRefusal, Vector, ZERO_VECTOR, generated};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::hot_v3::hot_cu_checkpoint_macro as hot_cu_checkpoint;

/// The accelerator's scoring arm: the same kernel over the fill witness.
pub mod accelerator;
/// `DealerFill`.
pub mod fill;
/// `DealerFound`.
pub mod found;
/// `DealerQuote`.
pub mod quote;
/// `DealerWithdraw`.
pub mod withdraw;

pub use dclutch_trading::scoring_rule::requests_v1::{
    is_dealer_fill_v1, is_dealer_found_v1, is_dealer_quote_v1, is_dealer_withdraw_v1,
};

/// The scoring Dealer's refusals: Trading band 4, sub-band `0x100`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoringDealerErrorV1 {
    /// Account count, a privilege, or an alias inside the prefix.
    Frame = 0x4200,
    /// The wire did not decode as the route's one request.
    Request = 0x4201,
    /// Not the canonical Core Market, or the request disagrees with it.
    Market = 0x4202,
    /// The activation cache or the Trading role it names.
    Release = 0x4203,
    /// The rule PDA, its body, or the fund's `rule_digest`.
    RuleSeal = 0x4204,
    /// `parametersAdmissible` false.
    Parameters = 0x4205,
    /// The recorded subsidy is not `subsidyOf`, or the deposit is short.
    Subsidy = 0x4206,
    /// The fund PDA, its body, its owner, or its Market/Dealer.
    Fund = 0x4207,
    /// The request named a fund revision the fund has left.
    FundStale = 0x4208,
    /// The Dealer's Claims Position PDA or body.
    Position = 0x4209,
    /// R0: a coordinate at or past the outcome count is nonzero.
    Width = 0x420A,
    /// R0: the Dealer cannot deliver what the fill asks.
    Deliverable = 0x420B,
    /// R0: a coordinate both receives and delivers.
    NonCanonical = 0x420C,
    /// R1: the post-fill inventory holds a complete set.
    NotNormalized = 0x420D,
    /// R2: the price is not the Dealer's marginal price at its post-fill state.
    OffSchedule = 0x420E,
    /// R3: the debit exceeds what the potential allows.
    Uncovered = 0x420F,
    /// The price vector is not a simplex at the rule's scale.
    PricesNotSimplex = 0x4210,
    /// The withdrawal exceeds `Φ`.
    WithdrawBelowFloor = 0x4211,
    /// The signer is not the fund's sponsor.
    Sponsor = 0x4212,
    /// A vault, token account, Custody replay or Custody receipt disagrees.
    Custody = 0x4213,
    /// The aggregate, a Position, or a Claims receipt disagrees.
    Claims = 0x4214,
    /// A checked intermediate left `u128`; unreachable under `Parameters`.
    Overflow = 0x4215,
    /// A write-back found bytes another instruction moved.
    Commit = 0x4216,
    /// The fund is retired, or the Market's phase refuses this route.
    Phase = 0x4217,
    /// The quote PDA or its body.
    Quote = 0x4218,
    /// The account offered as this Market's basis record is not it.
    ///
    /// `semantic_basis_id_v3` is the content address the founding committed to
    /// when it wrote the aggregate's `basis_id`; a record that reproduces it
    /// cannot disagree with the Market about the width or the payout scale,
    /// and a substituted record cannot reproduce it.
    Basis = 0x4219,
    /// The founding's `claim_unit_atoms` is not the basis record's
    /// `payout_scale`.
    ///
    /// Its own accusation because it is the divisor EVERY later route converts
    /// through: `DealerFundV1::cash_claims`, `debit_atoms`, the withdraw floor.
    /// A founding that named the wrong one would price the whole life of the
    /// Dealer in a unit the Market does not use, and every conjunct downstream
    /// would pass.
    ClaimUnit = 0x421A,
    /// The rule's `K` is not this Market's ordinary outcome count.
    ///
    /// The fill mints a complete set across coordinates `0 .. K`; a `K` short
    /// of the Market's ordinary width would mint claims on a strict subset of
    /// the outcomes, which is not a complete set and is not collateralized by
    /// the par the Hoard receives. Ordinary excludes decision 0025's failure
    /// coordinate, which the basis record's own `refunds_on_failure` names.
    OutcomeCount = 0x421B,
}

dclutch_refusal_registry::pin_refusal_band!(
    ScoringDealerErrorV1,
    dclutch_refusal_registry::TRADING_REFUSAL_BASE + generated::REFUSAL_SUB_BAND_OFFSET,
    [
        Frame,
        Request,
        Market,
        Release,
        RuleSeal,
        Parameters,
        Subsidy,
        Fund,
        FundStale,
        Position,
        Width,
        Deliverable,
        NonCanonical,
        NotNormalized,
        OffSchedule,
        Uncovered,
        PricesNotSimplex,
        WithdrawBelowFloor,
        Sponsor,
        Custody,
        Claims,
        Overflow,
        Commit,
        Phase,
        Quote,
        Basis,
        ClaimUnit,
        OutcomeCount
    ]
);

impl From<ScoringRefusal> for ScoringDealerErrorV1 {
    fn from(cause: ScoringRefusal) -> Self {
        match cause {
            ScoringRefusal::OutcomeCount
            | ScoringRefusal::Liquidity
            | ScoringRefusal::Scale
            | ScoringRefusal::Tolerance => Self::Parameters,
            ScoringRefusal::Subsidy => Self::Subsidy,
            ScoringRefusal::Width => Self::Width,
            ScoringRefusal::Deliverable => Self::Deliverable,
            ScoringRefusal::NonCanonical => Self::NonCanonical,
            ScoringRefusal::NotNormalized => Self::NotNormalized,
            ScoringRefusal::OffSchedule => Self::OffSchedule,
            ScoringRefusal::Uncovered => Self::Uncovered,
            ScoringRefusal::PricesNotSimplex => Self::PricesNotSimplex,
            ScoringRefusal::Overflow => Self::Overflow,
            ScoringRefusal::WithdrawBelowFloor => Self::WithdrawBelowFloor,
        }
    }
}

impl From<RecordErrorV1> for ScoringDealerErrorV1 {
    fn from(cause: RecordErrorV1) -> Self {
        match cause {
            RecordErrorV1::Rule(refusal) => refusal.into(),
            RecordErrorV1::Width => Self::Width,
            _ => Self::Request,
        }
    }
}

type Result<T> = core::result::Result<T, ProgramError>;

/// Parse a route's fixed prefix: the exact count, the writability the Lean
/// states, the signers it requires, and pairwise-distinct keys; then the
/// windows after it, which are the children's to police.
///
/// **What this deliberately does NOT check, and why.** `is_signer` is a
/// TRANSACTION-level property. The fee payer is message key 0 and reads
/// `is_signer == true` in every instruction that names it, whatever
/// `AccountMeta` the builder gave it. So a frame that refuses a signer -- at
/// one coordinate or across a whole window -- is not stating a constraint on
/// its own instruction; it is stating that the caller may not have built the
/// REST of its transaction a particular way, and it goes dead the moment a
/// builder places the payer in it. That is what `5ca145e8` did to the three
/// founding-abort routes, which is why `tools/seam-audit` reads a blanket
/// `is_signer` refusal as a finding on sight
/// (`TRANSACTION_LEVEL_SIGNER_CENSUS`), and this route's own founding is the
/// case that would have hit it: the sponsor signs, and Custody's rent `payer`
/// is the sponsor, so the sponsor sits inside two windows.
///
/// The same is true of `is_writable` at the TOP level, where it is a MESSAGE
/// property: one flag per account for the whole transaction, so any
/// instruction that wants an account writable makes it writable in every
/// instruction that names it. A readonly pin here is therefore also a
/// constraint on the caller's other instructions, which is not this route's to
/// impose -- and it buys nothing, because this route writes only the three
/// PDAs it derives itself, all of them declared writable.
///
/// So both privileges are required in ONE direction and free in the other: a
/// coordinate the frame declares writable must be writable, a coordinate it
/// declares a signer must be signed, and an extra bit the rest of the
/// transaction granted is not a refusal. The windows' privileges are the child
/// programs' own frame specs to enforce -- which they do, by their own names,
/// and `child_metas_v1` writes exactly those specs into the metas.
pub(crate) fn parse_prefix<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    prefix: usize,
    windows: usize,
    privileges: fn(usize) -> Option<(bool, bool)>,
) -> Result<(
    &'accounts [AccountInfo<'info>],
    &'accounts [AccountInfo<'info>],
)> {
    let total = prefix
        .checked_add(windows)
        .ok_or(ScoringDealerErrorV1::Frame)?;
    if accounts.len() != total {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    let (head, tail) = accounts.split_at(prefix);
    for (index, account) in head.iter().enumerate() {
        let (writable, signer) = privileges(index).ok_or(ScoringDealerErrorV1::Frame)?;
        if (writable && !account.is_writable) || (signer && !account.is_signer) {
            return Err(ScoringDealerErrorV1::Frame.into());
        }
        if head
            .get(index.saturating_add(1)..)
            .is_some_and(|rest| rest.iter().any(|other| other.key == account.key))
        {
            return Err(ScoringDealerErrorV1::Frame.into());
        }
    }
    Ok((head, tail))
}

pub(crate) fn get<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>> {
    accounts
        .get(index)
        .ok_or_else(|| ScoringDealerErrorV1::Frame.into())
}

/// What a route learns from the Market: its identity and phase.
#[derive(Clone, Copy)]
pub(crate) struct MarketFactsV1 {
    pub(crate) release_set: [u8; 32],
    pub(crate) registry: [u8; 32],
    pub(crate) realm: [u8; 32],
    pub(crate) generation: u64,
    /// The sole lifecycle RentCredit Core committed at founding.
    pub(crate) rent_beneficiary: [u8; 32],
    pub(crate) phase: Phase,
}

/// Hold the Market to the request, out of line: `CoreState` decodes and
/// RE-ENCODES to compare byte for byte, the widest stack object on any of
/// these routes, and the inliner is not a party to the frame constraint
/// (`direct_begin_retiring_v1::authenticate_market` explains the 4,096).
#[inline(never)]
pub(crate) fn authenticate_market_v1(
    market: &AccountInfo<'_>,
    expected_market: [u8; 32],
    admitted: &[Phase],
) -> Result<MarketFactsV1> {
    let data = market
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Market)?;
    if market.key.to_bytes() != expected_market || data.len() != STATE_BYTES {
        return Err(ScoringDealerErrorV1::Market.into());
    }
    let state = CoreState::decode(&data).map_err(|_| ScoringDealerErrorV1::Market)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        market.owner,
    )
    .0;
    if expected != *market.key
        || state
            .encode()
            .map_err(|_| ScoringDealerErrorV1::Market)?
            .as_slice()
            != data.as_ref()
        || state.identity.market_id.to_bytes() != expected_market
    {
        return Err(ScoringDealerErrorV1::Market.into());
    }
    if !admitted.contains(&state.phase) {
        return Err(ScoringDealerErrorV1::Phase.into());
    }
    Ok(MarketFactsV1 {
        release_set: state.identity.selected_release_set.to_bytes(),
        registry: state.identity.registry_program.to_bytes(),
        realm: state.identity.realm_id.to_bytes(),
        generation: state.identity.generation,
        rent_beneficiary: state.rent_beneficiary.to_bytes(),
        phase: state.phase,
    })
}

/// Authenticate the activation cache for the Market's release set and that
/// THIS program is the release's Trading role, from one read of the cache
/// (decision 0017, option B). Out of line for the frame's sake, as
/// `direct_begin_retiring_v1::reauthenticate_roles` explains.
///
/// Returns the release's CLAIMS program, because the same one read already
/// carries it and every route that reads a Position needs it. An account is
/// only a Claims Position if the program that owns it is the Claims role this
/// release set activated: without that, deriving the Position's address under
/// the account's own `owner` proves nothing, since any program can mint an
/// account that satisfies a derivation under itself.
#[inline(never)]
pub(crate) fn authenticate_release_v1(
    program_id: &Pubkey,
    registry: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    facts: MarketFactsV1,
) -> Result<Pubkey> {
    if registry.key.to_bytes() != facts.registry || !registry.executable {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    require_cache_account(registry.key, cache).map_err(|_| ScoringDealerErrorV1::Release)?;
    let data = cache
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data)
        .map_err(|_| ScoringDealerErrorV1::Release)?;
    authenticate_activation_cache_identity_v1(registry, cache, &facts.release_set, activated)
        .map_err(|_| ScoringDealerErrorV1::Release)?;
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| ScoringDealerErrorV1::Release)?;
    if trading.release().program().to_bytes() != program_id.to_bytes() {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    let claims = activated
        .role(ExecutionRoleV1::Claims)
        .map_err(|_| ScoringDealerErrorV1::Release)?;
    Ok(Pubkey::new_from_array(
        claims.release().program().to_bytes(),
    ))
}

/// The fund at its PDA, owned by this program, for this Market and Dealer,
/// at the revision the request read, in a phase that admits the route.
pub(crate) fn authenticate_fund_v1(
    program_id: &Pubkey,
    fund: &AccountInfo<'_>,
    market: [u8; 32],
    dealer_id: [u8; 32],
    expected_revision: u64,
) -> Result<DealerFundV1> {
    if fund.owner != program_id {
        return Err(ScoringDealerErrorV1::Fund.into());
    }
    let data = fund
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Fund)?;
    let value = DealerFundV1::decode(&data).map_err(|_| ScoringDealerErrorV1::Fund)?;
    if value.market != market || value.dealer_id != dealer_id {
        return Err(ScoringDealerErrorV1::Fund.into());
    }
    let bump = [value.bump];
    let [domain, seed_market, seed_dealer] = DealerFundV1::seeds(&market, &dealer_id);
    let expected =
        Pubkey::create_program_address(&[domain, seed_market, seed_dealer, &bump], program_id)
            .map_err(|_| ScoringDealerErrorV1::Fund)?;
    if expected != *fund.key {
        return Err(ScoringDealerErrorV1::Fund.into());
    }
    if value.revision != expected_revision {
        return Err(ScoringDealerErrorV1::FundStale.into());
    }
    if value.phase != FundPhaseV1::Open {
        return Err(ScoringDealerErrorV1::Phase.into());
    }
    Ok(value)
}

/// The sealed rule: at its PDA, owned by this program, its digest the one
/// the fund carries. A request naming another rule has no fund.
pub(crate) fn authenticate_rule_v1(
    program_id: &Pubkey,
    rule: &AccountInfo<'_>,
    fund: DealerFundV1,
) -> Result<ScoringRuleRecordV1> {
    if rule.owner != program_id {
        return Err(ScoringDealerErrorV1::RuleSeal.into());
    }
    let data = rule
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::RuleSeal)?;
    let value = ScoringRuleRecordV1::decode(&data).map_err(|_| ScoringDealerErrorV1::RuleSeal)?;
    if hash(&data).to_bytes() != fund.rule_digest
        || value.market != fund.market
        || value.dealer_id != fund.dealer_id
        || value.parameters.outcome_count != fund.outcome_count
    {
        return Err(ScoringDealerErrorV1::RuleSeal.into());
    }
    let expected = Pubkey::find_program_address(
        &ScoringRuleRecordV1::seeds(&fund.market, &fund.dealer_id),
        program_id,
    )
    .0;
    if expected != *rule.key {
        return Err(ScoringDealerErrorV1::RuleSeal.into());
    }
    Ok(value)
}

/// The Dealer's inventory: its Claims Position, owned by the Claims program
/// at the canonical PDA for (aggregate, fund), read coordinate by coordinate.
/// This is the projection the design names -- `positionAdmissible` -- and it
/// is authoritative: the fund's cached `(minimum, cost)` must be RE-DERIVED
/// from it at every fill (the carrier of §6 is a cache, never an author).
pub(crate) fn read_inventory_v1(
    claims_program: &Pubkey,
    aggregate: &AccountInfo<'_>,
    position: &AccountInfo<'_>,
    fund_key: &Pubkey,
    outcome_count: u8,
) -> Result<(
    Vector,
    LiabilityBasisPositionViewV2,
    LiabilityBasisMarketViewV2,
)> {
    if aggregate.owner != claims_program || position.owner != claims_program {
        return Err(ScoringDealerErrorV1::Position.into());
    }
    let aggregate_data = aggregate
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let aggregate_view = LiabilityBasisMarketViewV2::decode(&aggregate_data)
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let data = position
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Position)?;
    let view =
        LiabilityBasisPositionViewV2::decode(&data).map_err(|_| ScoringDealerErrorV1::Position)?;
    if view.owner != fund_key.to_bytes()
        || view.market_account != aggregate.key.to_bytes()
        || view.claim_count != aggregate_view.claim_count
        // The failure coordinate (decision 0025) sits past the ordinary ones;
        // the Dealer's inventory is the ordinary K.
        || view.claim_count < u32::from(outcome_count)
    {
        return Err(ScoringDealerErrorV1::Position.into());
    }
    let seeds = ProtocolPositionSeedsV2::new(aggregate.key.to_bytes(), fund_key.to_bytes())
        .map_err(|_| ScoringDealerErrorV1::Position)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), claims_program).0;
    if expected != *position.key {
        return Err(ScoringDealerErrorV1::Position.into());
    }
    let mut inventory = ZERO_VECTOR;
    for (index, slot) in inventory
        .iter_mut()
        .enumerate()
        .take(usize::from(outcome_count))
    {
        let claim = u32::try_from(index).map_err(|_| ScoringDealerErrorV1::Overflow)?;
        *slot = view
            .balance(&data, claim)
            .map_err(|_| ScoringDealerErrorV1::Position)?;
    }
    Ok((inventory, view, aggregate_view))
}

/// Hold the founding's claim-unit conversion and width to this Market's own
/// basis record.
///
/// The founding names two numbers no other route can question afterwards:
/// `claim_unit_atoms`, the divisor every later route converts through, and
/// `outcome_count`, the width the fill mints a complete set across. Both were
/// taken on the sponsor's word. Both are facts the Market already published,
/// and the record that publishes them is already in the frame -- Claims' own
/// `Admit` window carries it at `BasisRecord`
/// (`dclutch-claims/src/frame_spec_v1.rs:492`), so this costs one decode and
/// no account.
///
/// `semantic_basis_id_v3` is the content address the founding of the MARKET
/// committed to when it wrote the aggregate's `basis_id`, and its preimage
/// carries the kind, the width and the payout scale. A record that reproduces
/// this Market's `basis_id` cannot disagree with the Market about the scale,
/// and a substituted record cannot reproduce it -- which is the same argument
/// `market_closure_v1::refunds_on_failure_v1` makes for the same account.
pub(crate) fn authenticate_claim_unit_v1(
    aggregate: &AccountInfo<'_>,
    basis_record: &AccountInfo<'_>,
    claim_unit_atoms: u64,
    outcome_count: u8,
) -> Result<()> {
    let market = {
        let data = aggregate
            .try_borrow_data()
            .map_err(|_| ScoringDealerErrorV1::Claims)?;
        LiabilityBasisMarketViewV2::decode(&data).map_err(|_| ScoringDealerErrorV1::Claims)?
    };
    let bytes = basis_record
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Basis)?;
    let semantic = dclutch_product::payoff::runtime_v3::semantic_basis_id_v3(&bytes)
        .map_err(|_| ScoringDealerErrorV1::Basis)?;
    let basis = dclutch_product::payoff::runtime_v3::ProductBasisV3::decode(&bytes)
        .map_err(|_| ScoringDealerErrorV1::Basis)?;
    if semantic != market.basis_id || basis.basis_width() != market.claim_count {
        return Err(ScoringDealerErrorV1::Basis.into());
    }
    if claim_unit_atoms != basis.payout_scale() {
        return Err(ScoringDealerErrorV1::ClaimUnit.into());
    }
    // Decision 0025 seats the failure column past the ordinary outcomes, and
    // its holder is an escrow no Dealer can trade against; the record's own
    // `refunds_on_failure` is what says whether this Market has one.
    let ordinary = if basis.refunds_on_failure() {
        market
            .claim_count
            .checked_sub(1)
            .ok_or(ScoringDealerErrorV1::OutcomeCount)?
    } else {
        market.claim_count
    };
    if u32::from(outcome_count) != ordinary {
        return Err(ScoringDealerErrorV1::OutcomeCount.into());
    }
    Ok(())
}

/// The fund's `TradingPrincipal` vault: the address the fund records, at
/// the Custody derivation for (market, release set, fund) -- the vault
/// context IS the fund (`ScoringRuleAbiV1.vaultContextIsTheFund`).
pub(crate) fn authenticate_vault_v1(
    custody_program: &Pubkey,
    vault: &AccountInfo<'_>,
    fund: DealerFundV1,
    fund_key: &Pubkey,
    release_set: [u8; 32],
) -> Result<()> {
    let seeds = CustodyVaultSeedsV1::new(
        fund.market,
        release_set,
        fund_key.to_bytes(),
        CompartmentV1::TradingPrincipal,
    );
    let expected = Pubkey::find_program_address(&seeds.as_slices(), custody_program).0;
    if expected != *vault.key || vault.key.to_bytes() != fund.vault {
        return Err(ScoringDealerErrorV1::Custody.into());
    }
    Ok(())
}

/// One Custody `Transfer` leg, composed and invoked under this program's
/// release-pinned caller authority for `context`.
///
/// `window` is the exact 14-account Custody Transfer frame
/// (`dclutch_custody::frame_spec_v1`, indices 0..14: caller authority, Core
/// Market, activation cache, Registry, caller program, caller ProgramData,
/// Realm record, Realm staging, replay, mint, source, destination, Custody
/// authority, token program). Custody authenticates every one of them by
/// its own name; this program supplies the request, signs the authority,
/// and holds the receipt to the amount it asked for.
#[allow(clippy::too_many_arguments)]
pub(crate) struct CustodyLegV1<'accounts, 'info> {
    pub(crate) window: &'accounts [AccountInfo<'info>],
    pub(crate) custody_program: &'accounts AccountInfo<'info>,
    pub(crate) replay: CustodyReplayV1,
    pub(crate) facts: MarketFactsV1,
    pub(crate) market: [u8; 32],
    pub(crate) context: [u8; 32],
    pub(crate) source_compartment: CompartmentV1,
    pub(crate) destination_compartment: CompartmentV1,
    pub(crate) source_owner: [u8; 32],
    pub(crate) destination_owner: [u8; 32],
    pub(crate) source_vault_context: [u8; 32],
    pub(crate) destination_vault_context: [u8; 32],
    pub(crate) parent_request_digest: [u8; 32],
    pub(crate) transfer_index: u16,
    pub(crate) amount: u64,
}

/// One child window's `AccountMeta` list, built from the CHILD's own frame
/// spec.
///
/// The privileges a child sees are the ones this program writes into the
/// metas, and the child compares them against its own spec EXACTLY
/// (`dclutch-claims-sbf/src/protocol_position_v2.rs:751`,
/// `dclutch-custody-sbf/src/lib.rs:1629`). So the only correct source for
/// them is that spec, and the two guesses this program used to make were both
/// wrong in a way a green build never shows:
///
/// - `signer = index == 0` alone drops the Payer's signature, and Custody
///   declares `Payer` SIGNER_WRITABLE at `InitializeReplay:9` and
///   `OpenVault:13`, so the vault could never be opened;
/// - `signer = index == 0 || account.is_signer` propagates a TRANSACTION-level
///   bit: the fee payer is message key 0 and reads `is_signer == true` at
///   every coordinate that names it, so the moment a builder pays with an
///   account the window also carries, the child sees a signer its spec says is
///   read-only and refuses.
///
/// The window is also held to the spec here, and refuses `Frame` by name
/// rather than dying inside the CPI with a privilege-escalation error no code
/// names -- but in ONE direction only, for the reason `parse_prefix` gives: a
/// privilege the spec declares must be present, because a CPI cannot grant
/// what the caller does not hold, and a bit the rest of the transaction
/// granted is not this route's to refuse. The metas carry the spec, not the
/// observation, so the child sees exactly what its own frame declares.
///
/// [`CHILD_CALLER_AUTHORITY_ACCOUNT`] is the one coordinate exempt from the
/// signer half, and it is exempt because THIS PROGRAM is what signs it. Every
/// child frame declares coordinate zero a signer and every child frame means
/// the caller-authority PDA, which has no key and is an ordinary read-only
/// account at the top level: `is_signer` is false there for every caller that
/// could ever exist, and `invoke_signed` is what makes it true one CPI down.
/// Requiring it observed would have refused `Frame` at the first child of
/// every route -- the whole family dead, with a green build. Its identity is
/// not unchecked: each caller derives the authority from its own seeds and
/// refuses `Release` unless coordinate zero is exactly that address, before
/// this function is reached.
/// The coordinate every child frame reserves for this program's own
/// caller-authority PDA. Custody's `common(0)`, Claims'
/// `protocol_position_admit(0)` and `signed_delta(0)` all declare it, and all
/// three declare it a signer.
pub(crate) const CHILD_CALLER_AUTHORITY_ACCOUNT: usize = 0;

pub(crate) fn child_metas_v1(
    window: &[AccountInfo<'_>],
    spec: fn(usize) -> Option<(bool, bool)>,
) -> Result<Vec<AccountMeta>> {
    let mut metas = Vec::with_capacity(window.len());
    for (index, account) in window.iter().enumerate() {
        let (signer, writable) = spec(index).ok_or(ScoringDealerErrorV1::Frame)?;
        let observed_signer = index == CHILD_CALLER_AUTHORITY_ACCOUNT || account.is_signer;
        if (writable && !account.is_writable) || (signer && !observed_signer) {
            return Err(ScoringDealerErrorV1::Frame.into());
        }
        metas.push(if writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    Ok(metas)
}

/// Custody's own frame spec, as `(signer, writable)` per coordinate.
pub(crate) fn custody_privileges_v1(operation: OperationV1) -> fn(usize) -> Option<(bool, bool)> {
    match operation {
        OperationV1::InitializeReplay => custody_initialize_replay_privileges_v1,
        OperationV1::OpenVault => custody_open_vault_privileges_v1,
        OperationV1::Transfer => custody_transfer_privileges_v1,
        OperationV1::CloseVault => custody_close_vault_privileges_v1,
        OperationV1::CloseReplay => custody_close_replay_privileges_v1,
    }
}

fn custody_spec_privileges_v1(operation: OperationV1, index: usize) -> Option<(bool, bool)> {
    let coordinate = u16::try_from(index).ok()?;
    let account = CustodyFrameSpecV1::new(operation)
        .account(coordinate)
        .ok()?;
    let privileges = account.privileges();
    Some((privileges.signer(), privileges.writable()))
}

fn custody_initialize_replay_privileges_v1(index: usize) -> Option<(bool, bool)> {
    custody_spec_privileges_v1(OperationV1::InitializeReplay, index)
}

fn custody_open_vault_privileges_v1(index: usize) -> Option<(bool, bool)> {
    custody_spec_privileges_v1(OperationV1::OpenVault, index)
}

fn custody_transfer_privileges_v1(index: usize) -> Option<(bool, bool)> {
    custody_spec_privileges_v1(OperationV1::Transfer, index)
}

fn custody_close_vault_privileges_v1(index: usize) -> Option<(bool, bool)> {
    custody_spec_privileges_v1(OperationV1::CloseVault, index)
}

fn custody_close_replay_privileges_v1(index: usize) -> Option<(bool, bool)> {
    custody_spec_privileges_v1(OperationV1::CloseReplay, index)
}

/// Claims' `Admit` frame spec, as `(signer, writable)` per coordinate.
pub(crate) fn claims_admit_privileges_v1(index: usize) -> Option<(bool, bool)> {
    let coordinate = u16::try_from(index).ok()?;
    let privileges = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Admit)
        .account(coordinate)
        .ok()?
        .privileges();
    Some((privileges.signer(), privileges.writable()))
}

/// Claims' signed-delta frame spec at one Position-table width.
pub(crate) fn claims_signed_delta_privileges_v1(index: usize) -> Option<(bool, bool)> {
    let coordinate = u16::try_from(index).ok()?;
    let privileges = SignedDeltaFrameSpecV3::new(SCORING_FILL_POSITION_COUNT_V1)
        .ok()?
        .account(coordinate)
        .ok()?
        .privileges();
    Some((privileges.signer(), privileges.writable()))
}

/// The fill moves exactly two Positions: the Dealer's and the taker's.
pub(crate) const SCORING_FILL_POSITION_COUNT_V1: u32 = 2;

/// Decode the replay a Custody window carries, so a leg names the exact
/// revision Custody will require rather than guessing it.
pub(crate) fn read_replay_v1(window: &[AccountInfo<'_>]) -> Result<CustodyReplayV1> {
    let replay = get(window, 8)?;
    let data = replay
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Custody)?;
    CustodyReplayV1::decode(&data).map_err(|_| ScoringDealerErrorV1::Custody.into())
}

#[inline(never)]
pub(crate) fn invoke_custody_transfer_v1<'accounts, 'info>(
    program_id: &Pubkey,
    leg: CustodyLegV1<'accounts, 'info>,
    external_owner: Option<&'accounts AccountInfo<'info>>,
) -> Result<()> {
    if leg.source_compartment == CompartmentV1::External {
        return invoke_delegated_custody_transfer_v2(
            program_id,
            leg,
            external_owner.ok_or(ScoringDealerErrorV1::Frame)?,
        );
    }
    if external_owner.is_some() {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    invoke_standard_custody_transfer_v1(program_id, leg)
}

/// The original Custody wire remains the whole path for Custody-owned source
/// compartments. Keeping it out of the delegated frame is also what lets the
/// accelerator link measure each physical authority profile independently.
#[inline(never)]
fn invoke_standard_custody_transfer_v1<'accounts, 'info>(
    program_id: &Pubkey,
    leg: CustodyLegV1<'accounts, 'info>,
) -> Result<()> {
    if leg.amount == 0 {
        return Ok(());
    }
    if leg.window.len() != TRANSFER_ACCOUNT_COUNT_V1 as usize {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    let mint = get(leg.window, 9)?;
    let source = get(leg.window, 10)?;
    let destination = get(leg.window, 11)?;
    let token_program = get(leg.window, 13)?;
    let request = custody_request_v1(program_id, &leg, mint, source, destination, token_program)?;
    let request_bytes = request
        .to_bytes()
        .map_err(|_| ScoringDealerErrorV1::Custody)?;
    invoke_standard_custody_request_v1(program_id, leg, &request_bytes)
}

#[inline(never)]
fn invoke_delegated_custody_transfer_v2<'accounts, 'info>(
    program_id: &Pubkey,
    leg: CustodyLegV1<'accounts, 'info>,
    owner: &'accounts AccountInfo<'info>,
) -> Result<()> {
    if leg.amount == 0 {
        return Ok(());
    }
    if leg.window.len() != TRANSFER_ACCOUNT_COUNT_V1 as usize {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    let mint = get(leg.window, 9)?;
    let source = get(leg.window, 10)?;
    let destination = get(leg.window, 11)?;
    let token_program = get(leg.window, 13)?;
    let request = custody_request_v1(program_id, &leg, mint, source, destination, token_program)?;
    let delegated_request =
        prepare_terminal_delegated_request_v2(&leg, request, mint, source, token_program, owner)?;
    let request_bytes = delegated_request
        .encode()
        .map_err(|_| ScoringDealerErrorV1::Custody)?
        .to_vec();
    invoke_delegated_custody_request_v2(program_id, leg, delegated_request, &request_bytes)
}

/// Sign, invoke and authenticate the V2 successor after its temporary
/// approval has been composed in the preceding physical frame.
#[inline(never)]
fn invoke_delegated_custody_request_v2(
    program_id: &Pubkey,
    leg: CustodyLegV1<'_, '_>,
    delegated_request: DelegatedCustodyRequestV2,
    request_bytes: &[u8],
) -> Result<()> {
    let request_digest = hash(&request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        leg.facts.release_set,
        leg.market,
        ExecutionRoleV1::Trading,
        leg.context,
        request_digest,
    )
    .map_err(|_| ScoringDealerErrorV1::Release)?;
    let (authority, bump) = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if get(leg.window, 0)?.key != &authority {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    // The bump relay: Custody derives its replay and transfer authority for
    // itself; `[0, 0]` after the request means "search", exactly as the hot
    // path's `child_relay` documents.
    let mut data = Vec::with_capacity(request_bytes.len() + CUSTODY_BUMP_RELAY_BYTES_V1);
    data.extend_from_slice(&request_bytes);
    data.extend_from_slice(&[0_u8; CUSTODY_BUMP_RELAY_BYTES_V1]);
    let metas = child_metas_v1(leg.window, custody_privileges_v1(OperationV1::Transfer))?;
    let instruction = Instruction {
        program_id: *leg.custody_program.key,
        accounts: metas,
        data,
    };
    let mut infos = Vec::with_capacity(leg.window.len() + 1);
    infos.extend_from_slice(leg.window);
    infos.push(leg.custody_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(crate::child_refused_v1)?;
    verify_delegated_custody_receipt_v2(leg, delegated_request, request_digest)
}

/// Authenticate Custody's successor receipt and the replay it just committed.
#[inline(never)]
fn verify_delegated_custody_receipt_v2(
    leg: CustodyLegV1<'_, '_>,
    delegated_request: DelegatedCustodyRequestV2,
    request_digest: [u8; 32],
) -> Result<()> {
    let (producer, receipt) = get_return_data().ok_or(ScoringDealerErrorV1::Custody)?;
    if producer != *leg.custody_program.key {
        return Err(ScoringDealerErrorV1::Custody.into());
    }
    let receipt = {
        let receipt = DelegatedCustodyReceiptV2::decode(&receipt)
            .map_err(|_| ScoringDealerErrorV1::Custody)?;
        if receipt.starts_atomic_debit != delegated_request.starts_atomic_debit
            || receipt.terminal != delegated_request.terminal
            || receipt.delegate_before != delegated_request.delegate_before
            || receipt.delegate_after != delegated_request.delegate_after
            || receipt.total_debit != delegated_request.total_debit
            || receipt.allowance_before != delegated_request.allowance_before
            || receipt.allowance_after != delegated_request.allowance_after
        {
            return Err(ScoringDealerErrorV1::Custody.into());
        }
        receipt.custody
    };
    let replay_data = get(leg.window, 8)?
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Custody)?;
    let replay_digest = hash(&replay_data).to_bytes();
    let replay =
        CustodyReplayV1::decode(&replay_data).map_err(|_| ScoringDealerErrorV1::Custody)?;
    if replay.next_revision != delegated_request.custody.resulting_revision
        || replay.last_request_digest != request_digest
        || replay.last_poststate_commitment != receipt.evidence.poststate_commitment
    {
        return Err(ScoringDealerErrorV1::Custody.into());
    }
    receipt
        .verify_for(delegated_request.custody, request_digest, replay_digest)
        .map_err(|_| ScoringDealerErrorV1::Custody)?;
    Ok(())
}

/// Approve exactly this physical debit, then bind the V2 successor request to
/// the approval. The surrounding instruction remains atomic, so a later
/// Custody refusal rolls this temporary delegation back with the whole route.
#[inline(never)]
fn prepare_terminal_delegated_request_v2<'accounts, 'info>(
    leg: &CustodyLegV1<'accounts, 'info>,
    request: CustodyRequestV1,
    mint: &AccountInfo<'info>,
    source: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
) -> Result<DelegatedCustodyRequestV2> {
    if !owner.is_signer || owner.key.to_bytes() != leg.source_owner {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    let decimals = Mint::parse(
        &mint
            .try_borrow_data()
            .map_err(|_| ScoringDealerErrorV1::Custody)?,
    )
    .map_err(|_| ScoringDealerErrorV1::Custody)?
    .decimals;
    let custody_authority = get(leg.window, 12)?;
    let approve = approve_checked(
        token_program.key.to_bytes(),
        source.key.to_bytes(),
        mint.key.to_bytes(),
        custody_authority.key.to_bytes(),
        owner.key.to_bytes(),
        leg.amount,
        decimals,
    )
    .map_err(|_| ScoringDealerErrorV1::Custody)?;
    invoke(
        &token_instruction(&approve),
        &[
            source.clone(),
            mint.clone(),
            custody_authority.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )
    .map_err(crate::child_refused_v1)?;
    let successor = DelegatedCustodyRequestV2 {
        custody: request,
        starts_atomic_debit: true,
        terminal: true,
        delegate_before: custody_authority.key.to_bytes(),
        delegate_after: [0; 32],
        total_debit: leg.amount,
        allowance_before: leg.amount,
        allowance_after: 0,
    };
    if !is_terminal_delegated_debit_v2(
        successor.starts_atomic_debit,
        successor.terminal,
        successor.total_debit,
        successor.allowance_before,
        successor.allowance_after,
    ) {
        return Err(ScoringDealerErrorV1::Custody.into());
    }
    Ok(successor)
}

fn custody_request_v1(
    program_id: &Pubkey,
    leg: &CustodyLegV1<'_, '_>,
    mint: &AccountInfo<'_>,
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<CustodyRequestV1> {
    Ok(CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: leg.source_compartment,
        destination_compartment: leg.destination_compartment,
        release_set: leg.facts.release_set,
        market: leg.market,
        realm: leg.facts.realm,
        context: leg.context,
        caller_program: program_id.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: leg.source_owner,
            destination_owner: leg.destination_owner,
            order: [0; 32],
            parent_request_digest: leg.parent_request_digest,
            order_nonce: 0,
            generation: leg.facts.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: leg.transfer_index,
        },
        source: source.key.to_bytes(),
        destination: destination.key.to_bytes(),
        source_vault_context: leg.source_vault_context,
        destination_vault_context: leg.destination_vault_context,
        mint: mint.key.to_bytes(),
        token_program: token_program.key.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: leg.replay.next_revision,
        resulting_revision: leg
            .replay
            .next_revision
            .checked_add(1)
            .ok_or(ScoringDealerErrorV1::Overflow)?,
        amount: leg.amount,
        rent_lamports: 0,
    })
}

#[inline(never)]
fn invoke_standard_custody_request_v1(
    program_id: &Pubkey,
    leg: CustodyLegV1<'_, '_>,
    request_bytes: &[u8],
) -> Result<()> {
    let request_digest = hash(request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        leg.facts.release_set,
        leg.market,
        ExecutionRoleV1::Trading,
        leg.context,
        request_digest,
    )
    .map_err(|_| ScoringDealerErrorV1::Release)?;
    let (authority, bump) = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if get(leg.window, 0)?.key != &authority {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    let mut data = Vec::with_capacity(request_bytes.len() + CUSTODY_BUMP_RELAY_BYTES_V1);
    data.extend_from_slice(request_bytes);
    data.extend_from_slice(&[0_u8; CUSTODY_BUMP_RELAY_BYTES_V1]);
    let instruction = Instruction {
        program_id: *leg.custody_program.key,
        accounts: child_metas_v1(leg.window, custody_privileges_v1(OperationV1::Transfer))?,
        data,
    };
    let mut infos = Vec::with_capacity(leg.window.len() + 1);
    infos.extend_from_slice(leg.window);
    infos.push(leg.custody_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(crate::child_refused_v1)?;
    let (producer, receipt) = get_return_data().ok_or(ScoringDealerErrorV1::Custody)?;
    if producer != *leg.custody_program.key {
        return Err(ScoringDealerErrorV1::Custody.into());
    }
    let receipt = CustodyReceiptV1::decode(&receipt).map_err(|_| ScoringDealerErrorV1::Custody)?;
    if receipt.operation != OperationV1::Transfer
        || receipt.source_compartment != leg.source_compartment
        || receipt.destination_compartment != leg.destination_compartment
        || receipt.transfer_index != leg.transfer_index
        || receipt.release_set != leg.facts.release_set
    {
        return Err(ScoringDealerErrorV1::Custody.into());
    }
    Ok(())
}

fn token_instruction<const ACCOUNTS: usize, const DATA: usize>(
    specification: &InstructionSpec<ACCOUNTS, DATA>,
) -> Instruction {
    let mut accounts = Vec::with_capacity(ACCOUNTS);
    for role in specification.accounts() {
        let address = Pubkey::new_from_array(*role.address());
        accounts.push(if role.is_writable() {
            AccountMeta::new(address, role.is_signer())
        } else {
            AccountMeta::new_readonly(address, role.is_signer())
        });
    }
    Instruction {
        program_id: Pubkey::new_from_array(*specification.program_id()),
        accounts,
        data: specification.data().to_vec(),
    }
}

/// The only delegated profile Dealer may compose: one exact debit whose
/// allowance is consumed and whose delegate is revoked by Custody.
const fn is_terminal_delegated_debit_v2(
    starts_atomic_debit: bool,
    terminal: bool,
    total_debit: u64,
    allowance_before: u64,
    allowance_after: u64,
) -> bool {
    starts_atomic_debit
        && terminal
        && total_debit != 0
        && total_debit == allowance_before
        && allowance_after == 0
}

/// Commit the fund: the bytes must still be the ones this route read, and
/// the write is the whole record.
pub(crate) fn commit_fund_v1(
    fund: &AccountInfo<'_>,
    expected_digest: [u8; 32],
    next: DealerFundV1,
) -> Result<[u8; generated::FUND_BYTES]> {
    let bytes = next.to_bytes().map_err(|_| ScoringDealerErrorV1::Commit)?;
    let mut data = fund
        .try_borrow_mut_data()
        .map_err(|_| ScoringDealerErrorV1::Commit)?;
    if data.len() != bytes.len() || hash(&data).to_bytes() != expected_digest {
        return Err(ScoringDealerErrorV1::Commit.into());
    }
    data.copy_from_slice(&bytes);
    Ok(bytes)
}

/// Rewrite the quote in place from the inventory, at the fund's revision.
pub(crate) fn write_quote_v1(quote: &AccountInfo<'_>, next: DealerQuoteV1) -> Result<()> {
    let bytes = next.to_bytes().map_err(|_| ScoringDealerErrorV1::Quote)?;
    let mut data = quote
        .try_borrow_mut_data()
        .map_err(|_| ScoringDealerErrorV1::Quote)?;
    if data.len() != bytes.len() {
        return Err(ScoringDealerErrorV1::Quote.into());
    }
    data.copy_from_slice(&bytes);
    Ok(())
}

/// The one receipt every route returns.
pub(crate) fn emit_receipt_v1(
    route: DealerRouteV1,
    request_bytes: &[u8],
    fund: DealerFundV1,
    fund_bytes: &[u8],
    dealer_pays: u64,
    dealer_receives: u64,
) -> Result<()> {
    let receipt = DealerReceiptV1::from_fund(
        route,
        request_bytes,
        fund,
        fund_bytes,
        dealer_pays,
        dealer_receives,
    )
    .to_bytes()
    .map_err(|_| ScoringDealerErrorV1::Commit)?;
    set_return_data(&receipt);
    hot_cu_checkpoint!("scoring-dealer:receipt");
    Ok(())
}

#[cfg(test)]
mod tests {
    use dclutch_claims::frame_spec_v1::{
        PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
    };
    use dclutch_custody::{INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, OPEN_VAULT_ACCOUNT_COUNT_V1};

    use super::*;

    /// Dealer's external debit is one exact delegation, not standing custody
    /// authority. A residual allowance or a split debit refuses before CPI.
    #[test]
    fn delegated_dealer_debit_is_terminal_and_exact() {
        assert!(is_terminal_delegated_debit_v2(true, true, 47, 47, 0));
        assert!(!is_terminal_delegated_debit_v2(true, true, 47, 47, 1));
        assert!(!is_terminal_delegated_debit_v2(true, true, 47, 46, 0));
        assert!(!is_terminal_delegated_debit_v2(false, true, 47, 47, 0));
    }

    /// The privileges this program writes into a child's metas are the child's
    /// own, coordinate for coordinate, and not a rule this program invented.
    ///
    /// The two rules it used to invent are what this pins against. `signer =
    /// index == 0` would report `(false, _)` at Custody's `Payer`, which is
    /// `SIGNER_WRITABLE` at `InitializeReplay:9` and `OpenVault:13` -- the
    /// vault could never be opened. `signer = index == 0 || account.is_signer`
    /// would report a signer at any read-only coordinate the fee payer
    /// happens to occupy, and the child compares exactly.
    #[test]
    fn every_child_coordinate_carries_the_childs_own_privileges() {
        for (operation, count) in [
            (
                OperationV1::InitializeReplay,
                INITIALIZE_REPLAY_ACCOUNT_COUNT_V1,
            ),
            (OperationV1::OpenVault, OPEN_VAULT_ACCOUNT_COUNT_V1),
            (OperationV1::Transfer, TRANSFER_ACCOUNT_COUNT_V1),
        ] {
            let ours = custody_privileges_v1(operation);
            for index in 0..usize::from(count) {
                let coordinate = u16::try_from(index).expect("coordinate");
                let theirs = CustodyFrameSpecV1::new(operation)
                    .account(coordinate)
                    .expect("custody coordinate")
                    .privileges();
                assert_eq!(
                    ours(index),
                    Some((theirs.signer(), theirs.writable())),
                    "{operation:?} coordinate {index}"
                );
            }
            assert_eq!(ours(usize::from(count)), None);
        }

        for index in 0..usize::from(PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1) {
            let coordinate = u16::try_from(index).expect("coordinate");
            let theirs = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Admit)
                .account(coordinate)
                .expect("claims coordinate")
                .privileges();
            assert_eq!(
                claims_admit_privileges_v1(index),
                Some((theirs.signer(), theirs.writable()))
            );
        }
        assert_eq!(
            claims_admit_privileges_v1(usize::from(PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1)),
            None
        );

        let width = usize::from(SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
            + usize::try_from(SCORING_FILL_POSITION_COUNT_V1).expect("width");
        for index in 0..width {
            let coordinate = u16::try_from(index).expect("coordinate");
            let theirs = SignedDeltaFrameSpecV3::new(SCORING_FILL_POSITION_COUNT_V1)
                .expect("signed delta spec")
                .account(coordinate)
                .expect("signed delta coordinate")
                .privileges();
            assert_eq!(
                claims_signed_delta_privileges_v1(index),
                Some((theirs.signer(), theirs.writable()))
            );
        }
        assert_eq!(claims_signed_delta_privileges_v1(width), None);
    }

    /// A window whose caller-authority coordinate is not a transaction signer
    /// is ADMITTED, because this program is what signs it.
    ///
    /// This is the regression the first version of `child_metas_v1` shipped:
    /// every child frame declares coordinate zero a signer, every child frame
    /// means the caller-authority PDA, and a PDA has no key to put in the
    /// message's signer set. Requiring it observed refused `Frame` at the
    /// first child of every route -- found, fill and withdraw all dead, with a
    /// green build and no test that could tell.
    #[test]
    fn a_caller_authority_that_cannot_sign_at_the_top_level_is_admitted() {
        let keys: Vec<Pubkey> = (0..usize::from(TRANSFER_ACCOUNT_COUNT_V1))
            .map(|index| Pubkey::new_from_array([u8::try_from(index).expect("index"); 32]))
            .collect();
        let mut lamports: Vec<u64> = alloc::vec![0; keys.len()];
        let mut data: Vec<Vec<u8>> = alloc::vec![Vec::new(); keys.len()];
        let owner = Pubkey::new_from_array([0xee; 32]);
        let spec = custody_privileges_v1(OperationV1::Transfer);
        let window: Vec<AccountInfo<'_>> = keys
            .iter()
            .zip(lamports.iter_mut())
            .zip(data.iter_mut())
            .enumerate()
            .map(|(index, ((key, lamports), data))| {
                let (_, writable) = spec(index).expect("coordinate");
                AccountInfo::new(key, false, writable, lamports, data, &owner, false)
            })
            .collect();
        let metas = child_metas_v1(&window, spec).expect("the authority is this program's to sign");
        assert_eq!(metas.len(), window.len());
        assert!(metas[CHILD_CALLER_AUTHORITY_ACCOUNT].is_signer);
        for (index, meta) in metas.iter().enumerate() {
            let (signer, writable) = spec(index).expect("coordinate");
            assert_eq!(meta.is_signer, signer, "coordinate {index}");
            assert_eq!(meta.is_writable, writable, "coordinate {index}");
        }
    }

    /// A coordinate the child declares writable and the caller did not is the
    /// one privilege a CPI genuinely cannot grant, and it refuses by name.
    #[test]
    fn a_window_short_of_a_declared_writable_refuses_frame() {
        let keys: Vec<Pubkey> = (0..usize::from(TRANSFER_ACCOUNT_COUNT_V1))
            .map(|index| Pubkey::new_from_array([u8::try_from(index).expect("index"); 32]))
            .collect();
        let mut lamports: Vec<u64> = alloc::vec![0; keys.len()];
        let mut data: Vec<Vec<u8>> = alloc::vec![Vec::new(); keys.len()];
        let owner = Pubkey::new_from_array([0xee; 32]);
        let spec = custody_privileges_v1(OperationV1::Transfer);
        // Coordinate 8 is Custody's replay, and Custody declares it writable.
        let window: Vec<AccountInfo<'_>> = keys
            .iter()
            .zip(lamports.iter_mut())
            .zip(data.iter_mut())
            .enumerate()
            .map(|(index, ((key, lamports), data))| {
                let (_, writable) = spec(index).expect("coordinate");
                let observed = writable && index != 8;
                AccountInfo::new(key, false, observed, lamports, data, &owner, false)
            })
            .collect();
        assert_eq!(spec(8), Some((false, true)));
        assert_eq!(
            child_metas_v1(&window, spec).expect_err("a read-only replay cannot be written"),
            ScoringDealerErrorV1::Frame.into()
        );
    }

    /// The two coordinates the old rules got wrong, named.
    #[test]
    fn the_custody_payer_signs_and_the_claims_basis_record_does_not() {
        assert_eq!(
            custody_privileges_v1(OperationV1::InitializeReplay)(9),
            Some((true, true))
        );
        assert_eq!(
            custody_privileges_v1(OperationV1::OpenVault)(13),
            Some((true, true))
        );
        assert_eq!(claims_admit_privileges_v1(0), Some((true, false)));
        assert_eq!(
            claims_admit_privileges_v1(super::found::ADMIT_BASIS_RECORD_ACCOUNT),
            Some((false, false))
        );
    }
}
