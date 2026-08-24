//! Host-only construction of unsigned dClutch categorical-resolution instructions.
//!
//! This untrusted projection builder accepts one finalized snapshot of
//! canonical accounts, re-decodes their immutable bindings, and constructs an
//! unsigned instruction. It never performs RPC, signing, or submission.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_capability_contract::{CapabilityManifestV1, ContentId as CapabilityContentId};
use dclutch_core_contract::Phase;
use dclutch_market_contract::market::{decode_market_outcome_count, CategoricalMarketV1};
use dclutch_pyth_contract::{
    funding::{
        required_resolution_minimum_balance, validate_required_resolution_funding, FundingStateV1,
    },
    instruction::{
        ResolveCategoricalFailureV1, ResolveCategoricalPythV1, RESOLVE_FAILURE_BYTES,
        RESOLVE_HEADER_BYTES,
    },
    resolution_material::CategoricalPythResolutionMaterialV1,
};
use dclutch_pyth_svm::{PostUpdateParamsView, PythReleaseV1, PRODUCTION_RELEASES};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

/// Chain-derived unsigned Realm and Market foundation workflows.
pub mod foundation;

pub(crate) const FUND_SEED: &[u8] = b"dclutch/resolution-fund/v1";
pub(crate) const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";
const RECEIVER_CONFIG_SEED: &[u8] = b"config";

/// The exact number of accounts in a price-resolution frame.
pub const PRICE_FRAME_ACCOUNTS: usize = 15;
/// The exact number of accounts in a permissionless failure-resolution frame.
pub const FAILURE_FRAME_ACCOUNTS: usize = 6;

/// An immutable finality label supplied with an observation report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Finality {
    /// Observed at processed commitment.
    Processed,
    /// Observed at confirmed commitment.
    Confirmed,
    /// Observed at finalized commitment.
    Finalized,
}

/// Slot, wall-clock time, and finality attached to an account observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    /// Observed slot.
    pub slot: u64,
    /// Observed Unix time.
    pub unix_timestamp: i64,
    /// Commitment/finality label.
    pub finality: Finality,
}

/// Host-observed account metadata and exact bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccount {
    /// Observation provenance.
    pub observation: Observation,
    /// Account address.
    pub key: Pubkey,
    /// Program owner.
    pub owner: Pubkey,
    /// Observed lamports.
    pub lamports: u64,
    /// Observed executable bit.
    pub executable: bool,
    /// Exact account bytes.
    pub data: Vec<u8>,
}

/// Same-finalized account observations required by resolution.
///
/// Material and manifest are mandatory immutable inputs. Callers cannot supply
/// alternate policy, feed, capability, provider, or funding DTO authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionState {
    /// Provider-neutral categorical Market.
    pub market: ObservedAccount,
    /// Raw 192-byte capability funding state.
    pub fund: ObservedAccount,
    /// Immutable Pyth policy plus feed-semantics material.
    pub resolution_material: ObservedAccount,
    /// Immutable manifest selecting the funding entry.
    pub capability_manifest: ObservedAccount,
    /// Market-root authenticated rent-refund destination.
    pub sponsor: ObservedAccount,
    /// Same-snapshot rent-exempt minimum for the 192-byte funding account.
    pub fund_rent_minimum: u64,
}

/// Caller-selected transaction plumbing for a price path, never semantic authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricePlumbing {
    /// Writable signing resolver.
    pub resolver: Pubkey,
    /// Writable signing temporary update account.
    pub update: Pubkey,
    /// Provider message account.
    pub encoded_vaa: Pubkey,
    /// Exact Pyth receiver post-update body.
    pub post_update_body: Vec<u8>,
}

/// Caller-selected permissionless bounty destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailurePlumbing {
    /// Writable payout recipient.
    pub bounty_recipient: Pubkey,
}

/// Constructed unsigned instruction and the observations that selected it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionReport {
    /// Exact instruction material, never signed or sent.
    pub instruction: Instruction,
    /// Shared finalized observation.
    pub observation: Observation,
    /// Exact pre-submission funding classification.
    pub funding: FundingReport,
}

/// Exact non-Hoard resolution-fund movements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReport {
    /// Fund account rent refunded at closure.
    pub fund_rent_refund: u64,
    /// Immutable manifest-selected provider reimbursement.
    pub provider_fee_reimbursement: u64,
    /// Immutable manifest-selected bounty.
    pub bounty: u64,
    /// Excess refunded to the Market root's rent-refund identity.
    pub sponsor_refund_excess: u64,
}

/// Refusal from hostile state, immutable bindings, or frame construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// Market bytes were not one implemented categorical Market width.
    InvalidMarket,
    /// Funding bytes were not the raw canonical funding state.
    InvalidFund,
    /// Immutable resolution material was malformed.
    InvalidMaterial,
    /// Immutable capability manifest was malformed.
    InvalidManifest,
    /// A program-owned role had an invalid owner or executable bit.
    InvalidOwner,
    /// The root-selected sponsor was not an empty system account.
    InvalidSponsor,
    /// An observation was not finalized.
    ObservationNotFinalized,
    /// Inputs did not share exactly one observation.
    ObservationMismatch,
    /// Market address was not its identity PDA.
    MarketPdaMismatch,
    /// Funding address was not the Market-derived PDA.
    FundPdaMismatch,
    /// An immutable content binding differed.
    ContentIdentityMismatch,
    /// Manifest did not uniquely select the policy configuration.
    FundingSelectionMismatch,
    /// Funding state or balance was insufficient or incompatible.
    FundUnderfunded,
    /// Market was not open.
    MarketNotOpen,
    /// No catalog release selected the committed material release.
    ReleaseUnavailable,
    /// Observation was outside the inclusive price window.
    PriceWindowClosed,
    /// Failure was attempted before the price window elapsed.
    FailureTooEarly,
    /// Release receiver configuration was not its canonical PDA.
    ConfigPdaMismatch,
    /// Post-update body was not exact Pyth receiver material.
    InvalidPostUpdateBody,
    /// Encoding an already validated instruction failed.
    InstructionEncoding,
}

/// Build the canonical 15-account Pyth price-resolution frame.
pub fn build_price_resolution(
    program_id: Pubkey,
    state: &ResolutionState,
    plumbing: &PricePlumbing,
) -> Result<ResolutionReport, Error> {
    let observation = state_observation(state)?;
    let facts = decode_state(program_id, state)?;
    if observation.unix_timestamp < facts.price_window_start
        || observation.unix_timestamp > facts.price_window_end
    {
        return Err(Error::PriceWindowClosed);
    }
    let release = select_release(facts.release_id, observation.unix_timestamp)?;
    let post = PostUpdateParamsView::parse(&plumbing.post_update_body)
        .map_err(|_| Error::InvalidPostUpdateBody)?;
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let (expected_config, _) = Pubkey::find_program_address(&[RECEIVER_CONFIG_SEED], &receiver);
    if Pubkey::new_from_array(release.receiver_config()) != expected_config {
        return Err(Error::ConfigPdaMismatch);
    }
    let (treasury, _) =
        Pubkey::find_program_address(&[RECEIVER_TREASURY_SEED, &[post.treasury_id()]], &receiver);
    let data = encode_price(
        facts.generation,
        facts.child_count,
        &plumbing.post_update_body,
    )?;
    let accounts = vec![
        AccountMeta::new(plumbing.resolver, true),
        AccountMeta::new(plumbing.update, true),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new(state.fund.key, false),
        AccountMeta::new_readonly(state.resolution_material.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new(state.sponsor.key, false),
        AccountMeta::new_readonly(receiver, false),
        AccountMeta::new_readonly(
            Pubkey::new_from_array(release.receiver_programdata()),
            false,
        ),
        AccountMeta::new_readonly(Pubkey::new_from_array(release.receiver_config()), false),
        AccountMeta::new_readonly(plumbing.encoded_vaa, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(release.router_program()), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(release.router_programdata()), false),
        AccountMeta::new(treasury, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    Ok(ResolutionReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation,
        funding: facts.funding,
    })
}

/// Build the canonical six-account permissionless failure-resolution frame.
pub fn build_failure_resolution(
    program_id: Pubkey,
    state: &ResolutionState,
    plumbing: FailurePlumbing,
) -> Result<ResolutionReport, Error> {
    let observation = state_observation(state)?;
    let facts = decode_state(program_id, state)?;
    if observation.unix_timestamp <= facts.price_window_end {
        return Err(Error::FailureTooEarly);
    }
    let mut data = vec![0; RESOLVE_FAILURE_BYTES];
    ResolveCategoricalFailureV1::new(facts.generation, facts.child_count)
        .encode(&mut data)
        .map_err(|_| Error::InstructionEncoding)?;
    let accounts = vec![
        AccountMeta::new(plumbing.bounty_recipient, false),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new(state.fund.key, false),
        AccountMeta::new_readonly(state.resolution_material.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new(state.sponsor.key, false),
    ];
    Ok(ResolutionReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation,
        funding: facts.funding,
    })
}

#[derive(Clone, Copy)]
struct Facts {
    generation: u64,
    child_count: u64,
    policy_id: [u8; 32],
    manifest_id: [u8; 32],
    rent_refund: [u8; 32],
    release_id: [u8; 32],
    price_window_start: i64,
    price_window_end: i64,
    funding: FundingReport,
}

fn encode_price(generation: u64, child_count: u64, body: &[u8]) -> Result<Vec<u8>, Error> {
    let wire = ResolveCategoricalPythV1::new(generation, child_count, body)
        .map_err(|_| Error::InstructionEncoding)?;
    let mut data = vec![0; RESOLVE_HEADER_BYTES + body.len()];
    wire.encode(&mut data)
        .map_err(|_| Error::InstructionEncoding)?;
    Ok(data)
}

fn state_observation(state: &ResolutionState) -> Result<Observation, Error> {
    same_observation(&[
        &state.market,
        &state.fund,
        &state.resolution_material,
        &state.capability_manifest,
        &state.sponsor,
    ])
}

fn decode_state(program_id: Pubkey, state: &ResolutionState) -> Result<Facts, Error> {
    state_observation(state)?;
    if state.market.owner != program_id
        || state.fund.owner != program_id
        || state.resolution_material.owner != program_id
        || state.capability_manifest.owner != program_id
        || state.market.executable
        || state.fund.executable
        || state.resolution_material.executable
        || state.capability_manifest.executable
    {
        return Err(Error::InvalidOwner);
    }
    if state.sponsor.owner != system_program::ID
        || state.sponsor.executable
        || !state.sponsor.data.is_empty()
    {
        return Err(Error::InvalidSponsor);
    }
    let mut facts = market_facts(program_id, state.market.key, &state.market.data)?;
    let (expected_fund, _) =
        Pubkey::find_program_address(&[FUND_SEED, state.market.key.as_ref()], &program_id);
    if state.fund.key != expected_fund {
        return Err(Error::FundPdaMismatch);
    }
    let material = CategoricalPythResolutionMaterialV1::decode(&state.resolution_material.data)
        .map_err(|_| Error::InvalidMaterial)?;
    if material.to_bytes().as_slice() != state.resolution_material.data.as_slice()
        || hash(&material.policy().to_bytes()).to_bytes() != facts.policy_id
        || hash(&material.feed_profile().to_bytes()).to_bytes()
            != *material.policy().feed_profile_id()
    {
        return Err(Error::ContentIdentityMismatch);
    }
    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| Error::InvalidManifest)?;
    if manifest.as_bytes() != state.capability_manifest.data.as_slice()
        || hash(manifest.as_bytes()).to_bytes() != facts.manifest_id
    {
        return Err(Error::ContentIdentityMismatch);
    }
    let policy_id =
        CapabilityContentId::new(facts.policy_id).map_err(|_| Error::ContentIdentityMismatch)?;
    let selected = manifest
        .required_founding_entry_for_config(policy_id)
        .map_err(|_| Error::FundingSelectionMismatch)?;
    if selected.entry().release_id().to_bytes() != *material.policy().release_id() {
        return Err(Error::ContentIdentityMismatch);
    }
    let funding = FundingStateV1::decode(&state.fund.data).map_err(|_| Error::InvalidFund)?;
    if funding.to_bytes().as_slice() != state.fund.data.as_slice() {
        return Err(Error::InvalidFund);
    }
    let manifest_id =
        CapabilityContentId::new(facts.manifest_id).map_err(|_| Error::ContentIdentityMismatch)?;
    validate_required_resolution_funding(
        funding,
        manifest_id,
        manifest,
        selected,
        state.fund_rent_minimum,
        funding.remaining().total_principal(),
    )
    .map_err(|_| Error::FundUnderfunded)?;
    let minimum =
        required_resolution_minimum_balance(funding).map_err(|_| Error::FundUnderfunded)?;
    let sponsor_refund_excess = state
        .fund
        .lamports
        .checked_sub(minimum)
        .ok_or(Error::FundUnderfunded)?;
    if state.sponsor.key.to_bytes() != facts.rent_refund {
        return Err(Error::ContentIdentityMismatch);
    }
    let policy = material
        .policy()
        .to_kernel_policy()
        .map_err(|_| Error::InvalidMaterial)?;
    let (price_window_start, price_window_end) = policy
        .resolution_window()
        .map_err(|_| Error::InvalidMaterial)?;
    facts.release_id = *material.policy().release_id();
    facts.price_window_start = price_window_start;
    facts.price_window_end = price_window_end;
    facts.funding = FundingReport {
        fund_rent_refund: state.fund_rent_minimum,
        provider_fee_reimbursement: funding.remaining().provider_principal(),
        bounty: funding.remaining().bounty_principal(),
        sponsor_refund_excess,
    };
    Ok(facts)
}

fn same_observation(accounts: &[&ObservedAccount]) -> Result<Observation, Error> {
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(Error::ObservationNotFinalized)?;
    if observation.finality != Finality::Finalized {
        return Err(Error::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn market_facts(program_id: Pubkey, market_key: Pubkey, bytes: &[u8]) -> Result<Facts, Error> {
    match decode_market_outcome_count(bytes).map_err(|_| Error::InvalidMarket)? {
        2 => typed_market_facts::<2>(program_id, market_key, bytes),
        3 => typed_market_facts::<3>(program_id, market_key, bytes),
        4 => typed_market_facts::<4>(program_id, market_key, bytes),
        5 => typed_market_facts::<5>(program_id, market_key, bytes),
        6 => typed_market_facts::<6>(program_id, market_key, bytes),
        7 => typed_market_facts::<7>(program_id, market_key, bytes),
        8 => typed_market_facts::<8>(program_id, market_key, bytes),
        9 => typed_market_facts::<9>(program_id, market_key, bytes),
        10 => typed_market_facts::<10>(program_id, market_key, bytes),
        11 => typed_market_facts::<11>(program_id, market_key, bytes),
        12 => typed_market_facts::<12>(program_id, market_key, bytes),
        13 => typed_market_facts::<13>(program_id, market_key, bytes),
        14 => typed_market_facts::<14>(program_id, market_key, bytes),
        15 => typed_market_facts::<15>(program_id, market_key, bytes),
        16 => typed_market_facts::<16>(program_id, market_key, bytes),
        _ => Err(Error::InvalidMarket),
    }
}

fn typed_market_facts<const N: usize>(
    program_id: Pubkey,
    market_key: Pubkey,
    bytes: &[u8],
) -> Result<Facts, Error> {
    let market = CategoricalMarketV1::<N>::decode(bytes).map_err(|_| Error::InvalidMarket)?;
    let root = market.root();
    if root.phase() != Phase::Open {
        return Err(Error::MarketNotOpen);
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &program_id);
    if market_key != expected_market {
        return Err(Error::MarketPdaMismatch);
    }
    Ok(Facts {
        generation: root.identity().generation(),
        child_count: root.outstanding_children(),
        policy_id: root.identity().resolution_policy_id().to_bytes(),
        manifest_id: root.identity().capability_manifest_id().to_bytes(),
        rent_refund: root.rent_refund(),
        release_id: [0; 32],
        price_window_start: 0,
        price_window_end: 0,
        funding: FundingReport {
            fund_rent_refund: 0,
            provider_fee_reimbursement: 0,
            bounty: 0,
            sponsor_refund_excess: 0,
        },
    })
}

fn select_release(release_id: [u8; 32], observed_time: i64) -> Result<PythReleaseV1, Error> {
    for release in &PRODUCTION_RELEASES {
        if hash(&release.to_bytes()).to_bytes() == release_id
            && observed_time >= release.activation_time()
        {
            return Ok(*release);
        }
    }
    #[cfg(feature = "non-production-real-pyth-lab")]
    {
        let release = dclutch_pyth_svm::synthetic_local_release_v1()
            .map_err(|_| Error::ReleaseUnavailable)?;
        let release = *release.release();
        if hash(&release.to_bytes()).to_bytes() == release_id
            && observed_time >= release.activation_time()
        {
            return Ok(release);
        }
    }
    Err(Error::ReleaseUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_contract::{
        ActivationPolicy, CapabilityEntryV1, FundingQuoteV1, CAPABILITY_ENTRY_BYTES,
        MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot};
    use dclutch_kernel::resolution::categorical_pyth_v1::{
        CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
    };
    use dclutch_market_contract::market::CategoricalSettlementSummaryV1;
    use dclutch_pyth_contract::{
        feed_profile::PythFeedProfileV1,
        funding::{construct_required_resolution_funding, FUNDING_BYTES},
        instruction::ResolveCategoricalFailureV1,
        policy::CategoricalPythPolicyRecordV1,
        resolution_material::CategoricalPythResolutionMaterialV1,
    };

    fn observation() -> Observation {
        Observation {
            slot: 7,
            unix_timestamp: 12,
            finality: Finality::Finalized,
        }
    }

    fn account(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero ID")
    }

    fn fixture() -> (Pubkey, ResolutionState) {
        let program = Pubkey::new_from_array([40; 32]);
        let sponsor = Pubkey::new_from_array([41; 32]);
        let feed = PythFeedProfileV1::new([1; 32], [2; 32], [3; 32]).expect("feed");
        let policy = CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
            pyth_release_id: [9; 32],
            feed_profile_id: hash(&feed.to_bytes()).to_bytes(),
            target_time: 10,
            grace: 0,
            window: 1,
            max_crossing_lag: 1,
            max_age: 1,
            max_future_skew: 1,
            confidence_multiplier: 1,
            max_confidence_bps: 1,
            max_normalized_confidence_atoms: 1,
            normalized_decimals: 0,
            price_cell_count: 1,
            upper_edges: [0; MAX_PRICE_CELLS],
            failure_outcome_index: 1,
        })
        .expect("policy");
        let material = CategoricalPythResolutionMaterialV1::new(policy, feed).expect("material");
        let policy_id = hash(&policy.to_bytes()).to_bytes();
        let quote = FundingQuoteV1::new(100, 0, 0, 7, 11, 0, 0).expect("quote");
        let entry = CapabilityEntryV1::new(
            CapabilityContentId::new([11; 32]).expect("kind"),
            CapabilityContentId::new([9; 32]).expect("release"),
            CapabilityContentId::new(policy_id).expect("policy ID"),
            CapabilityContentId::new([12; 32]).expect("capacity"),
            CapabilityContentId::new([13; 32]).expect("schema"),
            CapabilityContentId::new([14; 32]).expect("derivation"),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("entry");
        let mut manifest_data = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest =
            CapabilityManifestV1::encode_into(&[entry], &mut manifest_data).expect("manifest");
        let manifest_id = hash(manifest.as_bytes()).to_bytes();
        let identity = MarketIdentity::new(
            id(21),
            id(22),
            id(23),
            ContentId::new(policy_id).expect("policy"),
            ContentId::new(manifest_id).expect("manifest"),
            0,
        );
        let mut root = MarketRoot::founding(identity, sponsor.to_bytes()).expect("root");
        root.transition_phase(0, Phase::Open).expect("open");
        root.register_child(0, 0).expect("child");
        let market =
            CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
                .expect("market");
        let mut market_data = vec![0; CategoricalMarketV1::<2>::encoded_len().expect("length")];
        market.encode(&mut market_data).expect("encode");
        let (market_key, _) = Pubkey::find_program_address(
            &[MARKET_SEED, &hash(&identity.to_bytes()).to_bytes()],
            &program,
        );
        let selected = manifest
            .required_founding_entry_for_config(
                CapabilityContentId::new(policy_id).expect("policy"),
            )
            .expect("selected");
        let funding = construct_required_resolution_funding(
            CapabilityContentId::new(manifest_id).expect("manifest"),
            manifest,
            selected,
            100,
            7,
        )
        .expect("funding");
        assert_eq!(FUNDING_BYTES, 192);
        let fund_data = funding.to_bytes().to_vec();
        let (fund_key, _) =
            Pubkey::find_program_address(&[FUND_SEED, market_key.as_ref()], &program);
        (
            program,
            ResolutionState {
                market: account(market_key, program, 1, market_data),
                fund: account(fund_key, program, 118, fund_data),
                resolution_material: account(
                    Pubkey::new_from_array([42; 32]),
                    program,
                    1,
                    material.to_bytes().to_vec(),
                ),
                capability_manifest: account(
                    Pubkey::new_from_array([43; 32]),
                    program,
                    1,
                    manifest_data,
                ),
                sponsor: account(sponsor, system_program::ID, 1, Vec::new()),
                fund_rent_minimum: 100,
            },
        )
    }

    #[test]
    fn failure_frame_is_current_six_role_frame_and_raw_funding_is_classified() {
        let (program, state) = fixture();
        assert_eq!(
            PRICE_FRAME_ACCOUNTS,
            dclutch_pyth_contract::frame::PRICE_RESOLUTION_FRAME_V1.len()
        );
        assert_eq!(
            FAILURE_FRAME_ACCOUNTS,
            dclutch_pyth_contract::frame::FAILURE_RESOLUTION_FRAME_V1.len()
        );
        let report = build_failure_resolution(
            program,
            &state,
            FailurePlumbing {
                bounty_recipient: Pubkey::new_from_array([44; 32]),
            },
        )
        .expect("failure frame");
        assert_eq!(report.instruction.accounts.len(), FAILURE_FRAME_ACCOUNTS);
        assert_eq!(
            report.funding,
            FundingReport {
                fund_rent_refund: 100,
                provider_fee_reimbursement: 7,
                bounty: 11,
                sponsor_refund_excess: 0
            }
        );
        assert_eq!(
            ResolveCategoricalFailureV1::decode(&report.instruction.data)
                .expect("wire")
                .child_count(),
            1
        );
        assert_eq!(
            report.instruction.accounts.get(3).map(|meta| meta.pubkey),
            Some(state.resolution_material.key)
        );
        assert_eq!(
            report.instruction.accounts.get(4).map(|meta| meta.pubkey),
            Some(state.capability_manifest.key)
        );
    }

    #[test]
    fn hostile_snapshot_and_vacancy_of_funding_principal_refuse() {
        let (program, state) = fixture();
        let mut mismatched = state.clone();
        mismatched.capability_manifest.observation.slot += 1;
        assert_eq!(
            build_failure_resolution(
                program,
                &mismatched,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::ObservationMismatch)
        );
        let mut underfunded = state;
        underfunded.fund.lamports = 117;
        assert_eq!(
            build_failure_resolution(
                program,
                &underfunded,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::FundUnderfunded)
        );
    }
}
