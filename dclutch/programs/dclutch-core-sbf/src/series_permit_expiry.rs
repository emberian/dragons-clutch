//! Permissionless refund of one expired, still-unallocated Series permit PDA.
//!
//! The request carries the deterministic permit candidate because the account
//! is still System-owned and has no data. The adapter independently
//! authenticates the immutable Series records and ordered occurrence proof,
//! recomputes the retry deadline, observes the terminal Trading replay, and
//! transfers the prefunded lamports to the immutable RentCredit -- less a
//! capped, chain-derived reward to the crank that turned it, when the caller
//! names one in an optional 26th account. A 25-account frame still refunds
//! every lamport, exactly as it always did.

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_market::{
    Role, SeriesFoundingPermitSeedsV1, SeriesFoundingPermitV1, SeriesPermitExpiryRequestV1,
};
use dclutch_market::rent::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_trading::series::{
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, admit_occurrence_bytes, admit_ticket,
    replay::{
        SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesStateV3, TicketStateSeedsV3,
        TicketStateV3,
    },
    ticket_admission_v1::SERIES_TICKET_EXPIRED_ADMISSIBLE_STATES_V1,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, hash::hash, program::invoke_signed, pubkey::Pubkey,
    rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    CoreSbfError,
    frame::require_distinct,
    infrastructure::authenticate_profile,
    records::authenticate_finalized_record,
    release::{authenticate_role, identity},
};

/// Exact permissionless expiry account count.
pub const SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1: usize = 25;

pub(crate) struct ExpiryAccounts<'accounts, 'info> {
    pub(crate) permit: &'accounts AccountInfo<'info>,
    pub(crate) rent_credit: &'accounts AccountInfo<'info>,
    pub(crate) rent_program: &'accounts AccountInfo<'info>,
    pub(crate) infrastructure_profile: &'accounts AccountInfo<'info>,
    pub(crate) registry_artifact_raw: &'accounts AccountInfo<'info>,
    pub(crate) registry_artifact_staging: &'accounts AccountInfo<'info>,
    pub(crate) registry_program: &'accounts AccountInfo<'info>,
    pub(crate) registry_programdata: &'accounts AccountInfo<'info>,
    pub(crate) rent_artifact_raw: &'accounts AccountInfo<'info>,
    pub(crate) rent_artifact_staging: &'accounts AccountInfo<'info>,
    pub(crate) rent_programdata: &'accounts AccountInfo<'info>,
    pub(crate) activation_cache: &'accounts AccountInfo<'info>,
    pub(crate) trading_program: &'accounts AccountInfo<'info>,
    pub(crate) trading_programdata: &'accounts AccountInfo<'info>,
    pub(crate) root: &'accounts AccountInfo<'info>,
    pub(crate) ticket_state: &'accounts AccountInfo<'info>,
    pub(crate) template_raw: &'accounts AccountInfo<'info>,
    pub(crate) template_staging: &'accounts AccountInfo<'info>,
    pub(crate) occurrence_raw: &'accounts AccountInfo<'info>,
    pub(crate) occurrence_staging: &'accounts AccountInfo<'info>,
    pub(crate) ticket_raw: &'accounts AccountInfo<'info>,
    pub(crate) ticket_staging: &'accounts AccountInfo<'info>,
    pub(crate) clock: &'accounts AccountInfo<'info>,
    pub(crate) rent: &'accounts AccountInfo<'info>,
    pub(crate) system: &'accounts AccountInfo<'info>,
    pub(crate) crank: Option<&'accounts AccountInfo<'info>>,
}

impl<'accounts, 'info> ExpiryAccounts<'accounts, 'info> {
    /// Parse the fixed 25-account frame, or the funded 26.
    ///
    /// The crank recipient is optional by frame length, so every existing
    /// 25-account caller behaves exactly as it did. `require_distinct` runs
    /// over the whole slice, so the recipient cannot alias any of the 25.
    pub(crate) fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, CoreSbfError> {
        let funded = accounts.len() == SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1 + 1;
        if accounts.len() != SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1 && !funded {
            return Err(CoreSbfError::AccountFrame);
        }
        require_distinct(accounts)?;
        let value = Self {
            permit: account(accounts, 0)?,
            rent_credit: account(accounts, 1)?,
            rent_program: account(accounts, 2)?,
            infrastructure_profile: account(accounts, 3)?,
            registry_artifact_raw: account(accounts, 4)?,
            registry_artifact_staging: account(accounts, 5)?,
            registry_program: account(accounts, 6)?,
            registry_programdata: account(accounts, 7)?,
            rent_artifact_raw: account(accounts, 8)?,
            rent_artifact_staging: account(accounts, 9)?,
            rent_programdata: account(accounts, 10)?,
            activation_cache: account(accounts, 11)?,
            trading_program: account(accounts, 12)?,
            trading_programdata: account(accounts, 13)?,
            root: account(accounts, 14)?,
            ticket_state: account(accounts, 15)?,
            template_raw: account(accounts, 16)?,
            template_staging: account(accounts, 17)?,
            occurrence_raw: account(accounts, 18)?,
            occurrence_staging: account(accounts, 19)?,
            ticket_raw: account(accounts, 20)?,
            ticket_staging: account(accounts, 21)?,
            clock: account(accounts, 22)?,
            rent: account(accounts, 23)?,
            system: account(accounts, 24)?,
            crank: if funded {
                Some(account(accounts, SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1)?)
            } else {
                None
            },
        };
        value.validate_privileges()?;
        value.validate_crank()?;
        Ok(value)
    }

    /// Validate the optional crank recipient of a funded expiry.
    ///
    /// **Silent about `is_signer` in both directions, deliberately.** The other
    /// 25 accounts are all refused as signers because this is a permissionless
    /// verb; the recipient is the one account for which a signature would mean
    /// something else entirely -- who is *owed*, never who is *permitted* --
    /// and it is usually the fee payer, who signs. Requiring a signature would
    /// gate a permissionless route; refusing one is the live defect that keeps
    /// a cleanup's beneficiary from paying its own fee. See
    /// `docs/design/FUNDED_CRANK_V1.md` section 6.
    fn validate_crank(&self) -> Result<(), CoreSbfError> {
        let Some(crank) = self.crank else {
            return Ok(());
        };
        if !crank.is_writable
            || crank.executable
            || crank.owner != &system_program::ID
            || !crank.data_is_empty()
        {
            return Err(CoreSbfError::AccountFrame);
        }
        Ok(())
    }

    fn validate_privileges(&self) -> Result<(), CoreSbfError> {
        if self.permit.is_signer
            || !self.permit.is_writable
            || self.permit.executable
            || self.rent_credit.is_signer
            || !self.rent_credit.is_writable
            || self.rent_credit.executable
            || self.rent_program.is_signer
            || self.rent_program.is_writable
            || !self.rent_program.executable
            || self.registry_program.is_signer
            || self.registry_program.is_writable
            || !self.registry_program.executable
            || self.trading_program.is_signer
            || self.trading_program.is_writable
            || !self.trading_program.executable
            || self.system.key != &system_program::ID
            || self.system.is_signer
            || self.system.is_writable
            || !self.system.executable
            || self.clock.key != &sysvar::clock::ID
            || self.clock.is_signer
            || self.clock.is_writable
            || self.clock.executable
            || self.rent.key != &sysvar::rent::ID
            || self.rent.is_signer
            || self.rent.is_writable
            || self.rent.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for account in [
            self.infrastructure_profile,
            self.registry_artifact_raw,
            self.registry_artifact_staging,
            self.registry_programdata,
            self.rent_artifact_raw,
            self.rent_artifact_staging,
            self.rent_programdata,
            self.activation_cache,
            self.trading_programdata,
            self.root,
            self.ticket_state,
            self.template_raw,
            self.template_staging,
            self.occurrence_raw,
            self.occurrence_staging,
            self.ticket_raw,
            self.ticket_staging,
        ] {
            if account.is_signer || account.is_writable || account.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        Ok(())
    }
}

/// Refund one expired prefunded permit which Core never allocated.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SeriesPermitExpiryRequestV1,
    proof_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    let frame = ExpiryAccounts::parse(accounts)?;
    let permit = request.permit();
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Creation)?;
    let profile = authenticate_profile(
        program_id,
        frame.infrastructure_profile,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_program,
        frame.registry_programdata,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
    )?;
    authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.trading_program,
        frame.trading_programdata,
        identity(profile.registry().program().to_bytes())?,
        permit.intent().release_set().to_bytes(),
        Role::Trading,
    )?;
    let refund_owner = authenticate_series(&frame, permit, proof_bytes)?;
    authenticate_unallocated_permit(program_id, &frame, permit, refund_owner)?;
    refund(&frame, permit, &rent)?;
    Ok(())
}

#[inline(never)]
fn authenticate_series(
    frame: &ExpiryAccounts<'_, '_>,
    permit: SeriesFoundingPermitV1,
    proof_bytes: &[u8],
) -> Result<[u8; 32], CoreSbfError> {
    let intent = permit.intent();
    if intent.trading_program().to_bytes() != frame.trading_program.key.to_bytes()
        || intent.parent_root().to_bytes() != frame.root.key.to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let template_bytes = finalized_series_record(
        frame,
        frame.template_raw,
        frame.template_staging,
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    )?;
    let template_record = hash(&template_bytes).to_bytes();
    let occurrence_bytes = finalized_series_record(
        frame,
        frame.occurrence_raw,
        frame.occurrence_staging,
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    )?;
    let ticket_bytes = finalized_series_record(
        frame,
        frame.ticket_raw,
        frame.ticket_staging,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    )?;
    let occurrence = admit_occurrence_bytes(&template_bytes, &occurrence_bytes, proof_bytes)
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket = admit_ticket(&ticket_bytes).map_err(|_| CoreSbfError::Reference)?;
    occurrence
        .require_ticket(ticket.ticket())
        .map_err(|_| CoreSbfError::Reference)?;
    let occurrence_record = occurrence.occurrence();
    let ticket_record = ticket.ticket();
    let retry_through = occurrence
        .template()
        .retry_through(occurrence_record.occurrence())
        .map_err(|_| CoreSbfError::Reference)?;
    if intent.release_set().to_bytes() != occurrence.template().release_set().to_bytes()
        || intent.market().to_bytes() != occurrence_record.market().to_bytes()
        || intent.product_record().to_bytes() != occurrence_record.product_record().to_bytes()
        || intent.founder().to_bytes() != ticket_record.founder().to_bytes()
        || intent.ticket_context().to_bytes() != ticket.content_id().to_bytes()
        || intent.generation() != u64::from(occurrence_record.occurrence()) + 1
        || intent.expiry_slot() != retry_through
        || intent.rent_credit().to_bytes() != frame.rent_credit.key.to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    authenticate_replay(frame, occurrence, ticket, template_record)?;
    let clock = Clock::from_account_info(frame.clock).map_err(|_| CoreSbfError::Creation)?;
    require_expired(clock.slot, retry_through, intent.expiry_slot())?;
    Ok(ticket_record.refund_owner().to_bytes())
}

fn authenticate_replay(
    frame: &ExpiryAccounts<'_, '_>,
    occurrence: dclutch_trading::series::AdmittedOccurrenceV3,
    ticket: dclutch_trading::series::AdmittedTicketV3,
    template_record: [u8; 32],
) -> Result<(), CoreSbfError> {
    if frame.root.owner != frame.trading_program.key
        || frame.root.data_len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
        || frame.ticket_state.owner != frame.trading_program.key
        || frame.ticket_state.data_len() != SERIES_TICKET_STATE_BYTES_V3
    {
        return Err(CoreSbfError::Reference);
    }
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(CoreSbfError::Reference)?,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let expected_root =
        Pubkey::find_program_address(&header.seeds().as_slices(), frame.trading_program.key).0;
    let series = SeriesStateV3::decode(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(CoreSbfError::Reference)?,
        occurrence.template().occurrence_count(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let settled_occurrence = occurrence
        .occurrence()
        .occurrence()
        .checked_add(1)
        .ok_or(CoreSbfError::Arithmetic)?;
    // ONE AUTHOR FOR THE ROOT'S CONFIG IDENTITY. `selection().config()` is the
    // Registry RECORD DIGEST of the root's config record -- what every other
    // family's is, and what `borrow_record_against` in Trading's family-neutral
    // Hot prelude requires -- and for Series that config record IS the Template
    // record: the descriptor's `config_schema()` is
    // `SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3`. The Template's DOMAIN-SEPARATED
    // content identity (`occurrence.template_id()`) is a different value over
    // the same bytes, and it is what the occurrence proof is admitted against;
    // it is never read off the root.
    if frame.root.key != &expected_root
        || header.release_set().to_bytes() != occurrence.template().release_set().to_bytes()
        || header.selection().config().to_bytes() != template_record
        || series.next_occurrence() != settled_occurrence
        || series.current_ticket_prepared()
    {
        return Err(CoreSbfError::Reference);
    }
    let ticket_state_data = frame
        .ticket_state
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let state = TicketStateV3::decode(&ticket_state_data).map_err(|_| CoreSbfError::Reference)?;
    let seeds = TicketStateSeedsV3::new(frame.root.key.to_bytes(), ticket.content_id());
    let expected = Pubkey::find_program_address(&seeds.as_slices(), frame.trading_program.key).0;
    if frame.ticket_state.key != &expected
        || state.ticket_record_id() != ticket.content_id()
        || !SERIES_TICKET_EXPIRED_ADMISSIBLE_STATES_V1.admits(state.phase())
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(())
}

pub(crate) fn finalized_series_record(
    frame: &ExpiryAccounts<'_, '_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
) -> Result<alloc::vec::Vec<u8>, CoreSbfError> {
    let bytes = raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_finalized_record(
        frame.registry_program.key,
        raw,
        staging,
        schema,
        hash(&bytes).to_bytes(),
        &bytes,
    )?;
    Ok(bytes.to_vec())
}

pub(crate) fn authenticate_unallocated_permit(
    program_id: &Pubkey,
    frame: &ExpiryAccounts<'_, '_>,
    permit: SeriesFoundingPermitV1,
    expected_refund_owner: [u8; 32],
) -> Result<(), CoreSbfError> {
    let intent = permit.intent();
    let seeds = permit.seeds();
    let bump = [intent.bump()];
    let base = seeds.as_slices();
    let expected = Pubkey::create_program_address(
        &[base[0], base[1], base[2], base[3], bump.as_slice()],
        program_id,
    )
    .map_err(|_| CoreSbfError::Creation)?;
    if frame.permit.key != &expected
        || frame.permit.owner != &system_program::ID
        || frame.permit.data_len() != 0
        || !funded_rent_persists_v1(frame.permit.lamports())
    {
        return Err(CoreSbfError::Creation);
    }
    let credit_data = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| CoreSbfError::RentCredit)?;
    if credit.refund_wallet().to_bytes() != expected_refund_owner
        || credit.market().to_bytes() != intent.market().to_bytes()
        || credit.release_set().to_bytes() != intent.release_set().to_bytes()
        || credit.generation() != intent.generation()
    {
        return Err(CoreSbfError::RentCredit);
    }
    let credit_seeds = credit.pda_seeds();
    let credit_bump = [credit_seeds.bump()];
    let market = credit_seeds.market().to_bytes();
    let generation = credit_seeds.generation();
    let expected_credit = Pubkey::create_program_address(
        &[
            credit_seeds.domain(),
            &market,
            &generation,
            credit_bump.as_slice(),
        ],
        frame.rent_program.key,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if frame.rent_credit.owner != frame.rent_program.key
        || frame.rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || frame.rent_credit.key != &expected_credit
        || !funded_rent_persists_v1(frame.rent_credit.lamports())
    {
        return Err(CoreSbfError::RentCredit);
    }
    Ok(())
}

/// Authenticate an unallocated permit from finalized Series coordinates only.
///
/// The atomic precommit route has no permit body: the account is still an
/// empty System-owned PDA. Its cycle-free seeds and immutable RentCredit body
/// are therefore the complete authority. Ordinary V1 expiry continues through
/// [`authenticate_unallocated_permit`] with its caller-supplied permit body.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_record_derived_unallocated_permit(
    program_id: &Pubkey,
    frame: &ExpiryAccounts<'_, '_>,
    release_set: [u8; 32],
    market: [u8; 32],
    generation: u64,
    ticket_context: [u8; 32],
    expected_refund_owner: [u8; 32],
) -> Result<u8, CoreSbfError> {
    let seeds = SeriesFoundingPermitSeedsV1::new(
        identity(release_set)?,
        identity(market)?,
        identity(ticket_context)?,
    );
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if frame.permit.key != &expected
        || frame.permit.owner != &system_program::ID
        || frame.permit.data_len() != 0
        || !funded_rent_persists_v1(frame.permit.lamports())
    {
        return Err(CoreSbfError::Creation);
    }
    authenticate_rent_credit_coordinates(
        frame,
        expected_refund_owner,
        market,
        release_set,
        generation,
    )?;
    Ok(bump)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_rent_credit_coordinates(
    frame: &ExpiryAccounts<'_, '_>,
    expected_refund_owner: [u8; 32],
    market: [u8; 32],
    release_set: [u8; 32],
    generation: u64,
) -> Result<(), CoreSbfError> {
    let credit_data = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| CoreSbfError::RentCredit)?;
    if credit.refund_wallet().to_bytes() != expected_refund_owner
        || credit.market().to_bytes() != market
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != generation
    {
        return Err(CoreSbfError::RentCredit);
    }
    let credit_seeds = credit.pda_seeds();
    let credit_bump = [credit_seeds.bump()];
    let credit_market = credit_seeds.market().to_bytes();
    let credit_generation = credit_seeds.generation();
    let expected_credit = Pubkey::create_program_address(
        &[
            credit_seeds.domain(),
            &credit_market,
            &credit_generation,
            credit_bump.as_slice(),
        ],
        frame.rent_program.key,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if frame.rent_credit.owner != frame.rent_program.key
        || frame.rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || frame.rent_credit.key != &expected_credit
        || !funded_rent_persists_v1(frame.rent_credit.lamports())
    {
        return Err(CoreSbfError::RentCredit);
    }
    Ok(())
}

/// Exact two-way split of one expired permit's balance.
///
/// **This is the Rust half of a spec/implementation pair that no build checks.**
/// Its counterpart is `DClutch.MarketCore.seriesPermitExpiryCrankReward` /
/// `seriesPermitExpiryRefund` in
/// `formal/dclutch-semantics/DClutchSemantics/MarketCore.lean`, which is
/// **spec-only** -- nothing in this tree is generated from it, so a divergence
/// between these two definitions produces no error anywhere. The tests below
/// are the only gate, and each is named for the theorem it mirrors. If you
/// change either side, change both.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpirySplitV1 {
    reward: u64,
    refunded: u64,
}

impl ExpirySplitV1 {
    /// Mirrors Lean `seriesPermitExpiryCrankReward` / `seriesPermitExpiryRefund`.
    ///
    /// `reward_cap == 0` is the unfunded 25-account frame, where the RentCredit
    /// receives every lamport -- Lean
    /// `series_permit_expiry_unfunded_refunds_everything`.
    const fn new(balance: u64, reward_cap: u64) -> Self {
        // `min`, never a guarded subtraction: this must not refuse for want of
        // funds. A thin permit pays a thin reward and is still admitted.
        let reward = if reward_cap < balance {
            reward_cap
        } else {
            balance
        };
        Self {
            reward,
            refunded: balance - reward,
        }
    }

    /// Mirrors Lean `series_permit_expiry_conserves`.
    const fn total(self) -> u64 {
        self.reward + self.refunded
    }
}

/// Split one expired permit between the crank that turned it and the RentCredit.
///
/// Mirrors `DClutch.MarketCore.seriesPermitExpiryCrankReward` /
/// `seriesPermitExpiryRefund` in
/// `formal/dclutch-semantics/DClutchSemantics/MarketCore.lean`, where
/// `series_permit_expiry_conserves` proves the two legs sum to the balance and
/// `series_permit_expiry_unfunded_refunds_everything` proves the 25-account
/// frame is unchanged. **Keep the two in step**: the model is spec-only, so a
/// divergence here goes unnoticed by every build in the tree.
pub(crate) fn refund(
    frame: &ExpiryAccounts<'_, '_>,
    permit: SeriesFoundingPermitV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    refund_with_seeds(frame, permit.seeds(), permit.intent().bump(), rent)
}

/// Refund a record-derived, still-empty permit without inventing a body.
pub(crate) fn refund_record_derived(
    frame: &ExpiryAccounts<'_, '_>,
    release_set: [u8; 32],
    market: [u8; 32],
    ticket_context: [u8; 32],
    bump: u8,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let seeds = SeriesFoundingPermitSeedsV1::new(
        identity(release_set)?,
        identity(market)?,
        identity(ticket_context)?,
    );
    refund_with_seeds(frame, seeds, bump, rent)
}

fn refund_with_seeds(
    frame: &ExpiryAccounts<'_, '_>,
    seeds: SeriesFoundingPermitSeedsV1,
    bump: u8,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let base = seeds.as_slices();
    let bump = [bump];
    let signer = [base[0], base[1], base[2], base[3], bump.as_slice()];
    let balance = frame.permit.lamports();

    // Expiry is a CLOSING route: the permit drains and closes whatever happens,
    // so the reward is a capped slice of lamports already leaving. `min`, never
    // a guarded subtraction -- this must not refuse for want of funds, because
    // a crank that can refuse for money is an unturned crank. The cap is
    // chain-derived (one empty account's Rent), never a source literal.
    let reward_cap = match frame.crank {
        Some(_) => rent.minimum_balance(0),
        None => 0,
    };
    let split = ExpirySplitV1::new(balance, reward_cap);
    if split.total() != balance {
        return Err(CoreSbfError::Arithmetic);
    }
    let (reward, refunded) = (split.reward, split.refunded);

    let credit_before = frame.rent_credit.lamports();
    let crank_before = frame.crank.map(|crank| crank.lamports());

    if let (Some(crank), true) = (frame.crank, reward > 0) {
        invoke_signed(
            &transfer(frame.permit.key, crank.key, reward),
            &[frame.permit.clone(), crank.clone(), frame.system.clone()],
            &[&signer],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    if refunded > 0 {
        invoke_signed(
            &transfer(frame.permit.key, frame.rent_credit.key, refunded),
            &[
                frame.permit.clone(),
                frame.rent_credit.clone(),
                frame.system.clone(),
            ],
            &[&signer],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }

    if frame.permit.lamports() != 0
        || frame.permit.owner != &system_program::ID
        || frame.permit.data_len() != 0
    {
        return Err(CoreSbfError::Commit);
    }
    // Conservation, checked against observation: every lamport that left the
    // permit arrived at exactly one of the two recipients, and nowhere else.
    let credit_expected = credit_before
        .checked_add(refunded)
        .ok_or(CoreSbfError::Arithmetic)?;
    if frame.rent_credit.lamports() != credit_expected {
        return Err(CoreSbfError::Commit);
    }
    if let (Some(crank), Some(before)) = (frame.crank, crank_before) {
        let expected = before.checked_add(reward).ok_or(CoreSbfError::Arithmetic)?;
        if crank.lamports() != expected {
            return Err(CoreSbfError::Commit);
        }
    }
    Ok(())
}

pub(crate) fn require_expired(
    current_slot: u64,
    retry_through: u64,
    permit_expiry_slot: u64,
) -> Result<(), CoreSbfError> {
    if permit_expiry_slot != retry_through || current_slot <= retry_through {
        Err(CoreSbfError::Reference)
    } else {
        Ok(())
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_supplied_expiry_cannot_replace_the_recomputed_retry_deadline() {
        assert_eq!(require_expired(101, 100, 100), Ok(()));
        assert_eq!(require_expired(101, 100, 99), Err(CoreSbfError::Reference));
        assert_eq!(require_expired(100, 100, 100), Err(CoreSbfError::Reference));
    }

    // *** THE MODEL GATE. ***
    //
    // `MarketCore.lean` is spec-only: nothing in this tree is generated from
    // it, so the Lean model and this program agree only by inspection, and a
    // divergence raises no error in any build. These four tests are the gate.
    // Each is named for the theorem it mirrors, and together they pin every
    // equation the Lean proves about the split. Changing one side without the
    // other turns exactly one of these red.

    /// Mirrors Lean `series_permit_expiry_conserves`: the two legs sum to the
    /// balance. Non-trivial in BOTH languages for the same reason -- Lean's
    /// `Nat` and Rust's `u64` both truncate on underflow, so a cap above the
    /// balance would silently swallow lamports rather than error.
    #[test]
    fn series_permit_expiry_conserves() {
        for balance in [0_u64, 1, 889_999, 890_880, 890_881, 5_000_000, u64::MAX] {
            for cap in [0_u64, 1, 890_880, 5_000_000, u64::MAX] {
                let split = ExpirySplitV1::new(balance, cap);
                assert_eq!(
                    split.total(),
                    balance,
                    "balance {balance} cap {cap} must conserve"
                );
                assert_eq!(split.reward + split.refunded, balance);
            }
        }

        // *** NEGATIVE CONTROL, and it is the same one the Lean side uses. ***
        // The property that discriminates is that the REWARD leg is capped by
        // the balance. Pay the raw cap instead of `min(cap, balance)` and
        // conservation breaks the moment the cap exceeds the balance. On the
        // Lean side `omega` REFUSES `cap + seriesPermitExpiryRefund o cap =
        // lamports`, reporting the counterexample `cap - min cap lamports >= 1`.
        //
        // Note what does NOT discriminate, because it cost a wrong claim to
        // learn: `refunded = balance - cap` is EXTENSIONALLY EQUAL to the real
        // definition in both languages -- Lean's `Nat` and a saturating `u64`
        // both truncate identically. A control built on that difference proves
        // nothing.
        let thin = ExpirySplitV1::new(30, 100);
        assert_eq!((thin.reward, thin.refunded), (30, 0));
        assert_ne!(
            100 + thin.refunded,
            30,
            "paying the raw cap must break conservation, or this test is inert"
        );
        assert_eq!(
            thin.reward + thin.refunded,
            30,
            "the capped reward conserves"
        );
    }

    /// Mirrors Lean `series_permit_expiry_reward_is_capped` and
    /// `series_permit_expiry_reward_within_balance`.
    #[test]
    fn series_permit_expiry_reward_is_capped_and_within_balance() {
        for balance in [0_u64, 1, 890_880, 5_000_000, u64::MAX] {
            for cap in [0_u64, 1, 890_880, u64::MAX] {
                let split = ExpirySplitV1::new(balance, cap);
                assert!(split.reward <= cap, "balance {balance} cap {cap}");
                assert!(split.reward <= balance, "balance {balance} cap {cap}");
            }
        }
    }

    /// Mirrors Lean `series_permit_expiry_unfunded_refunds_everything`.
    ///
    /// **This is also the compatibility proof for the optional 26th account**:
    /// a 25-account frame sets the cap to zero, so the RentCredit still
    /// receives every lamport and no existing caller observes any change.
    #[test]
    fn series_permit_expiry_unfunded_refunds_everything() {
        for balance in [0_u64, 1, 890_880, 5_000_000, u64::MAX] {
            let split = ExpirySplitV1::new(balance, 0);
            assert_eq!(split.reward, 0);
            assert_eq!(split.refunded, balance);
        }
    }

    /// The two concrete instances checked against the Lean by `lake env lean`.
    /// Keeping the same numbers on both sides makes a drift legible at a glance.
    #[test]
    fn series_permit_expiry_matches_the_lean_worked_instances() {
        // Thin permit, generous cap: the crank takes all 30, refund 0.
        let thin = ExpirySplitV1::new(30, 100);
        assert_eq!((thin.reward, thin.refunded), (30, 0));
        // Fat permit, small cap: the crank takes the cap, RentCredit keeps rest.
        let fat = ExpirySplitV1::new(5_000, 100);
        assert_eq!((fat.reward, fat.refunded), (100, 4_900));
    }
}
