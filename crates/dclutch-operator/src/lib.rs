//! Host-only construction of unsigned dClutch categorical-resolution instructions.
//!
//! This crate is deliberately an untrusted projection builder.  It accepts
//! observed account bytes together with a slot, timestamp, and finality label,
//! re-decodes the persistent contracts, and derives every protocol-selected
//! account.  It neither signs, sends, nor reads accounts from RPC.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_core_contract::Phase;
use dclutch_pyth_contract::{
    funding::ResolutionFundV1,
    instruction::{ResolveCategoricalFailureV1, ResolveCategoricalPythV1},
    market::MarketStateV1,
};
use dclutch_pyth_svm::{PRODUCTION_RELEASES, PostUpdateParamsView, PythReleaseV1};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

const FUND_SEED: &[u8] = b"dclutch/resolution-fund/v1";
const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";
const RECEIVER_CONFIG_SEED: &[u8] = b"config";

/// The exact number of accounts in a price-resolution frame.
pub const PRICE_FRAME_ACCOUNTS: usize = 13;
/// The exact number of accounts in a permissionless failure-resolution frame.
pub const FAILURE_FRAME_ACCOUNTS: usize = 4;

/// An immutable finality label supplied with an observation report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Finality {
    /// The observation was obtained at processed commitment.
    Processed,
    /// The observation was obtained at confirmed commitment.
    Confirmed,
    /// The observation was obtained at finalized commitment.
    Finalized,
}

/// Slot, wall-clock time, and finality attached to a bounded account observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    /// Slot at which the account bytes were observed.
    pub slot: u64,
    /// Observed Unix timestamp used to select an eligible immutable release.
    pub unix_timestamp: i64,
    /// Commitment/finality label of this observation.
    pub finality: Finality,
}

/// Host-observed account metadata and bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccount {
    /// Account address at the recorded observation.
    pub key: Pubkey,
    /// Account owner at the recorded observation.
    pub owner: Pubkey,
    /// Lamports observed at the same slot and finality as `data`.
    pub lamports: u64,
    /// Executable bit observed at the same slot and finality as `data`.
    pub executable: bool,
    /// Exact account data at the recorded observation.
    pub data: Vec<u8>,
}

/// The two hostile account observations from which a resolution is constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionState {
    /// Mandatory provenance for both state observations and the report.
    pub observation: Observation,
    /// The hostile-decoded categorical Market account.
    pub market: ObservedAccount,
    /// The hostile-decoded prepaid resolution Fund account.
    pub fund: ObservedAccount,
    /// Rent-exempt minimum for the observed Fund length, obtained in the same report.
    pub fund_rent_minimum: u64,
}

/// Caller-selected transaction plumbing for a price path, never semantic authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricePlumbing {
    /// Writable signing resolver/payer account.
    pub resolver: Pubkey,
    /// Writable signing temporary Pyth update account.
    pub update: Pubkey,
    /// Read-only encoded VAA account selected for the post-update call.
    pub encoded_vaa: Pubkey,
    /// Exact post-update body, excluding the Pyth receiver discriminator.
    pub post_update_body: Vec<u8>,
}

/// Caller-selected permissionless bounty destination for a failure path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailurePlumbing {
    /// Writable recipient of the immutable failure bounty.
    pub bounty_recipient: Pubkey,
}

/// A constructed instruction plus the observation facts that selected it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionReport {
    /// Unsigned, exact Solana instruction material.
    pub instruction: Instruction,
    /// Required observation provenance.
    pub observation: Observation,
    /// Deterministic Fund debits and refunds implied by the observed Fund facts.
    pub funding: FundingReport,
}

/// Exact non-principal Fund movements identified before instruction submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReport {
    /// Fund-account rent returned on successful closure; never provider funding.
    pub fund_rent_refund: u64,
    /// Provider reimbursement reserved by the immutable Fund.
    pub provider_fee_reimbursement: u64,
    /// Resolver or permissionless failure-recipient bounty reserved by the Fund.
    pub bounty: u64,
    /// Excess refundable only to the immutable sponsor address.
    pub sponsor_refund_excess: u64,
}

/// Refusal from hostile state, catalog selection, or frame construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The Market bytes did not decode as an implemented outcome width.
    InvalidMarket,
    /// The Fund bytes did not decode as the exact fund contract.
    InvalidFund,
    /// An observed state account was not owned by the adapter program.
    InvalidOwner,
    /// Account observations not labeled finalized cannot construct an eligible instruction.
    ObservationNotFinalized,
    /// The Market key was not its canonical PDA.
    MarketPdaMismatch,
    /// The Fund key was not its canonical PDA.
    FundPdaMismatch,
    /// Fund facts did not bind the observed Market, generation, and sponsor.
    FundMismatch,
    /// Observed Fund lamports could not cover rent, provider reimbursement, and bounty.
    FundUnderfunded,
    /// The Market was not in the open phase required by either path.
    MarketNotOpen,
    /// Canonical policy or feed-profile content identities did not bind.
    ContentIdentityMismatch,
    /// No catalog entry selected the immutable policy release at observation time.
    ReleaseUnavailable,
    /// The observed clock was outside the inclusive price-resolution window.
    PriceWindowClosed,
    /// Permissionless failure resolution was attempted before the price window elapsed.
    FailureTooEarly,
    /// The catalog's receiver configuration was not its canonical PDA.
    ConfigPdaMismatch,
    /// The post-update body was not one exact Pyth receiver body.
    InvalidPostUpdateBody,
    /// Encoding an already validated dClutch instruction unexpectedly failed.
    InstructionEncoding,
}

/// Build the fixed 13-role price frame from hostile-decoded state and the shared catalog.
pub fn build_price_resolution(
    program_id: Pubkey,
    state: &ResolutionState,
    plumbing: &PricePlumbing,
) -> Result<ResolutionReport, Error> {
    let facts = decode_state(program_id, state)?;
    if state.observation.unix_timestamp < facts.price_window_start
        || state.observation.unix_timestamp > facts.price_window_end
    {
        return Err(Error::PriceWindowClosed);
    }
    let release = select_release(facts.release_id, state.observation.unix_timestamp)?;
    let post = PostUpdateParamsView::parse(&plumbing.post_update_body)
        .map_err(|_| Error::InvalidPostUpdateBody)?;
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let (expected_config, _) = Pubkey::find_program_address(&[RECEIVER_CONFIG_SEED], &receiver);
    if Pubkey::new_from_array(release.receiver_config()) != expected_config {
        return Err(Error::ConfigPdaMismatch);
    }
    let treasury_id = [post.treasury_id()];
    let (treasury, _) =
        Pubkey::find_program_address(&[RECEIVER_TREASURY_SEED, &treasury_id], &receiver);
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
        AccountMeta::new(facts.sponsor, false),
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
        AccountMeta::new_readonly(Pubkey::new_from_array([0; 32]), false),
    ];
    Ok(ResolutionReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation: state.observation,
        funding: facts.funding,
    })
}

/// Build the fixed four-role permissionless failure frame from hostile-decoded state.
pub fn build_failure_resolution(
    program_id: Pubkey,
    state: &ResolutionState,
    plumbing: FailurePlumbing,
) -> Result<ResolutionReport, Error> {
    let facts = decode_state(program_id, state)?;
    if state.observation.unix_timestamp <= facts.price_window_end {
        return Err(Error::FailureTooEarly);
    }
    let mut data = vec![0; dclutch_pyth_contract::instruction::RESOLVE_FAILURE_BYTES];
    ResolveCategoricalFailureV1::new(facts.generation, facts.child_count)
        .encode(&mut data)
        .map_err(|_| Error::InstructionEncoding)?;
    let accounts = vec![
        AccountMeta::new(plumbing.bounty_recipient, false),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new(state.fund.key, false),
        AccountMeta::new(facts.sponsor, false),
    ];
    Ok(ResolutionReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation: state.observation,
        funding: facts.funding,
    })
}

#[derive(Clone, Copy)]
struct Facts {
    generation: u64,
    child_count: u64,
    sponsor: Pubkey,
    release_id: [u8; 32],
    price_window_start: i64,
    price_window_end: i64,
    funding: FundingReport,
}

fn encode_price(generation: u64, child_count: u64, body: &[u8]) -> Result<Vec<u8>, Error> {
    let wire = ResolveCategoricalPythV1::new(generation, child_count, body)
        .map_err(|_| Error::InstructionEncoding)?;
    let mut data = vec![0; dclutch_pyth_contract::instruction::RESOLVE_HEADER_BYTES + body.len()];
    wire.encode(&mut data)
        .map_err(|_| Error::InstructionEncoding)?;
    Ok(data)
}

fn decode_state(program_id: Pubkey, state: &ResolutionState) -> Result<Facts, Error> {
    if state.observation.finality != Finality::Finalized {
        return Err(Error::ObservationNotFinalized);
    }
    if state.market.owner != program_id || state.fund.owner != program_id {
        return Err(Error::InvalidOwner);
    }
    let facts = market_facts(program_id, state.market.key, &state.market.data)?;
    let (expected_fund, _) =
        Pubkey::find_program_address(&[FUND_SEED, state.market.key.as_ref()], &program_id);
    if state.fund.key != expected_fund {
        return Err(Error::FundPdaMismatch);
    }
    let fund = ResolutionFundV1::decode(&state.fund.data).map_err(|_| Error::InvalidFund)?;
    if fund.market() != state.market.key.as_ref() || fund.generation() != facts.generation {
        return Err(Error::FundMismatch);
    }
    let classified = fund
        .classify_balance(state.fund.lamports, state.fund_rent_minimum)
        .map_err(|_| Error::FundUnderfunded)?;
    Ok(Facts {
        sponsor: Pubkey::new_from_array(*fund.sponsor_refund()),
        funding: FundingReport {
            fund_rent_refund: state.fund_rent_minimum,
            provider_fee_reimbursement: fund.provider_fee_reimbursement(),
            bounty: fund.success_bounty(),
            sponsor_refund_excess: classified.sponsor_refund_excess(),
        },
        ..facts
    })
}

fn market_facts(program_id: Pubkey, market_key: Pubkey, bytes: &[u8]) -> Result<Facts, Error> {
    let outcomes = bytes.get(10).copied().ok_or(Error::InvalidMarket)?;
    match outcomes {
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
    let market = MarketStateV1::<N>::decode(bytes).map_err(|_| Error::InvalidMarket)?;
    let root = market.root();
    if root.phase() != Phase::Open {
        return Err(Error::MarketNotOpen);
    }
    if hash(&market.policy().to_bytes()).to_bytes()
        != root.identity().resolution_policy_id().to_bytes()
        || hash(&market.feed_profile().to_bytes()).to_bytes() != *market.policy().feed_profile_id()
    {
        return Err(Error::ContentIdentityMismatch);
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &program_id);
    if market_key != expected_market {
        return Err(Error::MarketPdaMismatch);
    }
    let policy = market
        .policy()
        .to_kernel_policy()
        .map_err(|_| Error::InvalidMarket)?;
    let (price_window_start, price_window_end) = policy
        .resolution_window()
        .map_err(|_| Error::InvalidMarket)?;
    Ok(Facts {
        generation: root.identity().generation(),
        child_count: root.outstanding_children(),
        sponsor: Pubkey::default(),
        release_id: *market.policy().release_id(),
        price_window_start,
        price_window_end,
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
    use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot};
    use dclutch_kernel::resolution::categorical_pyth_v1::{
        CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
    };
    use dclutch_pyth_contract::{
        feed_profile::PythFeedProfileV1, policy::CategoricalPythPolicyRecordV1,
        receipt::ResolutionReceiptV1,
    };

    fn observed_state(release_id: [u8; 32]) -> ResolutionState {
        let profile = PythFeedProfileV1::new([1; 32], [2; 32], [3; 32]).expect("profile");
        let edges = [0; MAX_PRICE_CELLS];
        let policy = CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
            pyth_release_id: release_id,
            feed_profile_id: hash(&profile.to_bytes()).to_bytes(),
            target_time: 10,
            grace: 0,
            window: 10,
            max_crossing_lag: 10,
            max_age: 10,
            max_future_skew: 10,
            confidence_multiplier: 1,
            max_confidence_bps: 100,
            max_normalized_confidence_atoms: 100,
            normalized_decimals: 0,
            price_cell_count: 1,
            upper_edges: edges,
            failure_outcome_index: 1,
        })
        .expect("policy");
        let identity = MarketIdentity::new(
            ContentId::new([4; 32]).expect("realm"),
            ContentId::new([5; 32]).expect("terms"),
            ContentId::new([6; 32]).expect("basis"),
            ContentId::new(hash(&policy.to_bytes()).to_bytes()).expect("policy identity"),
            ContentId::new([7; 32]).expect("capability manifest"),
            7,
        );
        let mut root = MarketRoot::founding(identity);
        root.transition_phase(7, Phase::Open).expect("open");
        root.register_child(7, 0).expect("child");
        let market = MarketStateV1::<2>::new(
            root,
            policy,
            profile,
            0,
            [0, 0],
            ResolutionReceiptV1::empty(2).expect("receipt"),
        )
        .expect("market");
        let program = Pubkey::new_from_array([9; 32]);
        let (market_key, _) = Pubkey::find_program_address(
            &[MARKET_SEED, &hash(&identity.to_bytes()).to_bytes()],
            &program,
        );
        let sponsor = Pubkey::new_from_array([8; 32]);
        let (fund_key, _) =
            Pubkey::find_program_address(&[FUND_SEED, market_key.as_ref()], &program);
        ResolutionState {
            observation: Observation {
                slot: 123,
                unix_timestamp: 21,
                finality: Finality::Finalized,
            },
            market: ObservedAccount {
                key: market_key,
                owner: program,
                lamports: 0,
                executable: false,
                data: {
                    let mut bytes =
                        vec![0; MarketStateV1::<2>::encoded_len().expect("market width")];
                    market.encode(&mut bytes).expect("market encoding");
                    bytes
                },
            },
            fund: ObservedAccount {
                key: fund_key,
                owner: program,
                lamports: 1,
                executable: false,
                data: ResolutionFundV1::new(market_key.to_bytes(), 7, sponsor.to_bytes(), 0, 1)
                    .expect("fund")
                    .to_bytes()
                    .to_vec(),
            },
            fund_rent_minimum: 0,
        }
    }

    #[test]
    fn production_catalog_is_an_explicit_refusal() {
        assert_eq!(select_release([7; 32], 0), Err(Error::ReleaseUnavailable));
    }

    #[test]
    fn failure_wire_is_exact_and_has_no_provider_roles() {
        let instruction = ResolveCategoricalFailureV1::new(9, 11);
        let mut data = vec![0; 32];
        instruction.encode(&mut data).expect("fixed failure wire");
        assert_eq!(data.len(), 32);
        assert_eq!(
            data.get(10),
            Some(&dclutch_pyth_contract::instruction::RESOLVE_FAILURE_TAG)
        );
    }

    #[test]
    fn failure_frame_derives_replay_facts_sponsor_and_exact_privileges() {
        let program = Pubkey::new_from_array([9; 32]);
        let state = observed_state([7; 32]);
        let report = build_failure_resolution(
            program,
            &state,
            FailurePlumbing {
                bounty_recipient: Pubkey::new_from_array([42; 32]),
            },
        )
        .expect("valid hostile-decoded state");
        assert_eq!(report.observation, state.observation);
        assert_eq!(
            report.funding,
            FundingReport {
                fund_rent_refund: 0,
                provider_fee_reimbursement: 0,
                bounty: 1,
                sponsor_refund_excess: 0
            }
        );
        assert_eq!(report.instruction.accounts.len(), FAILURE_FRAME_ACCOUNTS);
        assert_eq!(
            report.instruction.accounts.first(),
            Some(&AccountMeta::new(Pubkey::new_from_array([42; 32]), false))
        );
        assert_eq!(
            report.instruction.accounts.get(1),
            Some(&AccountMeta::new(state.market.key, false))
        );
        assert_eq!(
            report.instruction.accounts.get(2),
            Some(&AccountMeta::new(state.fund.key, false))
        );
        assert_eq!(
            report.instruction.accounts.get(3),
            Some(&AccountMeta::new(Pubkey::new_from_array([8; 32]), false))
        );
        assert_eq!(
            ResolveCategoricalFailureV1::decode(&report.instruction.data)
                .expect("wire")
                .generation(),
            7
        );
    }

    #[test]
    fn stale_market_or_fund_keys_refuse() {
        let program = Pubkey::new_from_array([9; 32]);
        let mut state = observed_state([7; 32]);
        state.market.key = Pubkey::new_from_array([4; 32]);
        assert_eq!(
            build_failure_resolution(
                program,
                &state,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::MarketPdaMismatch)
        );
        let mut state = observed_state([7; 32]);
        state.fund.key = Pubkey::new_from_array([4; 32]);
        assert_eq!(
            build_failure_resolution(
                program,
                &state,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::FundPdaMismatch)
        );
    }

    #[test]
    fn stale_fund_state_and_hostile_body_refuse() {
        let program = Pubkey::new_from_array([9; 32]);
        let mut state = observed_state([7; 32]);
        let byte = state.fund.data.get_mut(48).expect("fund generation byte");
        *byte ^= 1;
        assert_eq!(
            build_failure_resolution(
                program,
                &state,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::FundMismatch)
        );
        let mut price_state = observed_state([7; 32]);
        price_state.observation.unix_timestamp = 20;
        assert_eq!(
            build_price_resolution(
                program,
                &price_state,
                &PricePlumbing {
                    resolver: Pubkey::new_unique(),
                    update: Pubkey::new_unique(),
                    encoded_vaa: Pubkey::new_unique(),
                    post_update_body: vec![0; 10],
                }
            ),
            Err(Error::ReleaseUnavailable)
        );
    }

    #[test]
    fn nonfinalized_or_underfunded_observations_refuse() {
        let program = Pubkey::new_from_array([9; 32]);
        let mut state = observed_state([7; 32]);
        state.observation.finality = Finality::Confirmed;
        assert_eq!(
            build_failure_resolution(
                program,
                &state,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::ObservationNotFinalized)
        );
        let mut state = observed_state([7; 32]);
        state.fund.lamports = 0;
        assert_eq!(
            build_failure_resolution(
                program,
                &state,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::FundUnderfunded)
        );
    }

    #[test]
    fn failure_before_the_strict_deadline_refuses() {
        let program = Pubkey::new_from_array([9; 32]);
        let mut state = observed_state([7; 32]);
        state.observation.unix_timestamp = 20;
        assert_eq!(
            build_failure_resolution(
                program,
                &state,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique(),
                },
            ),
            Err(Error::FailureTooEarly)
        );
    }

    #[test]
    fn post_update_body_refuses_trailing_provider_bytes() {
        assert!(PostUpdateParamsView::parse(&[0; 10]).is_err());
    }

    #[cfg(feature = "non-production-real-pyth-lab")]
    #[test]
    fn lab_price_frame_uses_only_the_shared_release_row() {
        let program = Pubkey::new_from_array([9; 32]);
        let release = dclutch_pyth_svm::synthetic_local_release_v1().expect("pinned local release");
        let release_id = hash(&release.release().to_bytes()).to_bytes();
        let mut state = observed_state(release_id);
        state.observation.unix_timestamp = 20;
        let sponsor = Pubkey::new_from_array([8; 32]);
        state.fund.data =
            ResolutionFundV1::new(state.market.key.to_bytes(), 7, sponsor.to_bytes(), 1, 1)
                .expect("fund")
                .to_bytes()
                .to_vec();
        state.fund.lamports = 2;
        let full_post = include_bytes!(
            "../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data"
        );
        let body = full_post.get(8..).expect("captured discriminator").to_vec();
        let plumbing = PricePlumbing {
            resolver: Pubkey::new_from_array([41; 32]),
            update: Pubkey::new_from_array([42; 32]),
            encoded_vaa: Pubkey::new_from_array([43; 32]),
            post_update_body: body,
        };
        let report = build_price_resolution(program, &state, &plumbing).expect("lab frame");
        assert_eq!(report.instruction.accounts.len(), PRICE_FRAME_ACCOUNTS);
        assert_eq!(
            report.instruction.accounts.first(),
            Some(&AccountMeta::new(plumbing.resolver, true))
        );
        assert_eq!(
            report.instruction.accounts.get(1),
            Some(&AccountMeta::new(plumbing.update, true))
        );
        assert_eq!(report.funding.provider_fee_reimbursement, 1);
        let decoded = dclutch_pyth_contract::instruction::ResolveCategoricalPythV1::decode(
            &report.instruction.data,
        )
        .expect("price wire");
        assert_eq!(decoded.generation(), 7);
        assert_eq!(decoded.child_count(), 1);
        assert_eq!(decoded.body(), plumbing.post_update_body);
    }
}
