//! The Market's life after Open.
//!
//! Every stage here submits real transactions to the validator the founding is
//! still running on, and every stage ends at a conservation-ledger boundary.
//! A stage that cannot run says so in the transcript with the exact route it
//! would have used and the exact reason the chain would refuse it; it does not
//! simulate the effect by writing state directly, because a journey that fakes
//! its own middle proves nothing about the end.

use std::collections::BTreeMap;

use dclutch_market_core_codec::{CoreState, Phase};
use dclutch_product_payoff_v2_codec::runtime_v3::ProductBasisV3;
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2, SweepLifecycleRentCreditV2,
};
use dclutch_token_svm::{ACCOUNT_BYTES, AccountState, TOKEN_2022_PROGRAM_ID, TokenAccount};
use serde::Serialize;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::sysvar;
use solana_system_interface::instruction::create_account;

use crate::{
    Error, Result,
    ledger::ConservationLedgerV1,
    model::{AccountEvidence, SuccessorPlan, TransactionEvidence},
    plan::pubkey,
    rpc::{Rpc, account_evidence},
};

/// SPL Token `InitializeAccount3` discriminant; the owner is inline, so no
/// Rent sysvar coordinate and no second transaction.
const INITIALIZE_ACCOUNT_3: u8 = 18;

/// SPL Token `TransferChecked` discriminant. The journey never uses the
/// unchecked `Transfer`: a decimals mismatch is exactly the class of mistake a
/// conservation ledger should never have to discover after the fact.
const TRANSFER_CHECKED: u8 = 12;

/// What one stage of the journey did, or could not do.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct StageReportV1 {
    /// Stage name, stable enough to bind a census binding to.
    pub(crate) stage: String,
    /// `executed`, `probed`, or `blocked`.
    pub(crate) outcome: String,
    /// Transactions this stage submitted.
    pub(crate) transactions: usize,
    /// Compute units those transactions consumed in total.
    pub(crate) compute_units: u64,
    /// What happened, or exactly what stands in the way and who owns it.
    pub(crate) note: String,
}

/// Every address the post-Open journey needs, taken off the founding's own
/// evidence rather than re-derived, so a founding that moved an address moves
/// this journey with it instead of silently diverging.
pub(crate) struct MarketAddressesV1 {
    pub(crate) founding_market: Pubkey,
    pub(crate) found31_market: Pubkey,
    pub(crate) aggregate: Pubkey,
    pub(crate) founder_position: Pubkey,
    pub(crate) admission: Pubkey,
    pub(crate) hoard: Pubkey,
    pub(crate) custody_replay: Pubkey,
    pub(crate) mint: Pubkey,
    pub(crate) founder_wallet: Pubkey,
    pub(crate) rent_credit: Pubkey,
    pub(crate) linked_basis_record: Pubkey,
}

impl MarketAddressesV1 {
    /// Read the founding's account evidence.
    pub(crate) fn from_evidence(accounts: &BTreeMap<String, AccountEvidence>) -> Result<Self> {
        let at = |label: &str| -> Result<Pubkey> {
            let evidence = accounts.get(label).ok_or_else(|| {
                Error::new(format!(
                    "the founding's evidence names no `{label}` account; the journey cannot \
                     continue a founding whose shape it does not recognise"
                ))
            })?;
            pubkey(&evidence.address)
        };
        Ok(Self {
            founding_market: at("founding_market")?,
            found31_market: at("market")?,
            aggregate: at("claims_aggregate")?,
            founder_position: at("founder_position")?,
            admission: at("claims_admission")?,
            hoard: at("founding_hoard_vault_open")?,
            custody_replay: at("founding_normal_custody_replay")?,
            mint: at("collateral_mint")?,
            founder_wallet: at("collateral_wallet")?,
            rent_credit: at("lifecycle_rent_credit")?,
            linked_basis_record: at("linked_liability_basis_record")?,
        })
    }
}

/// One synthetic holder: a wallet the journey controls and its collateral
/// token account.
pub(crate) struct HolderV1 {
    pub(crate) label: String,
    pub(crate) owner: Keypair,
    pub(crate) token_account: Pubkey,
}

/// Prove the founding really left the state the journey is about to use.
///
/// This is not a duplicate of the founding's own poststate check. That one
/// asked "did my transaction do what I planned"; this one asks "is what is on
/// chain the prestate a *user* now needs," and it is asked by the code that is
/// about to depend on it.
pub(crate) fn admit_open_market(
    rpc: &mut Rpc,
    addresses: &MarketAddressesV1,
    ledger: &mut ConservationLedgerV1,
) -> Result<(u64, u8)> {
    let market = rpc.required_account(addresses.founding_market, "founded Market")?;
    let state = CoreState::decode(&market.data)
        .map_err(|error| Error::new(format!("founded Market: {error:?}")))?;
    if state.phase != Phase::Open {
        return Err(Error::new(format!(
            "the journey needs an Open Market and the founding left phase {:?}",
            state.phase
        )));
    }

    // The claim unit comes from the Registry's own linked-basis record, not
    // from the Hoard divided by the supply. Deriving it from the founding
    // poststate would make L4 assert that the founding equals itself; taking
    // it from the published basis makes L4 a real check at every boundary,
    // including the first.
    let basis = rpc.required_account(addresses.linked_basis_record, "linked liability basis")?;
    let claim_unit_atoms = ProductBasisV3::decode(&basis.data)
        .map_err(|error| Error::new(format!("linked liability basis: {error:?}")))?
        .payout_scale();
    if claim_unit_atoms == 0 {
        return Err(Error::new(
            "the published liability basis carries a zero payout scale, which cannot back a claim",
        ));
    }

    let wallet = rpc.required_account(addresses.founder_wallet, "founder collateral wallet")?;
    let decimals = {
        let parsed = TokenAccount::parse(&wallet.data)
            .map_err(|error| Error::new(format!("founder collateral wallet: {error:?}")))?;
        if parsed.state != AccountState::Initialized {
            return Err(Error::new("the founder's collateral wallet is not live"));
        }
        let mint = rpc.required_account(addresses.mint, "collateral Mint")?;
        dclutch_token_svm::Mint::parse(
            mint.data
                .get(..dclutch_token_svm::MINT_BYTES)
                .ok_or_else(|| Error::new("collateral Mint is narrower than the base layout"))?,
        )
        .map_err(|error| Error::new(format!("collateral Mint: {error:?}")))?
        .decimals
    };

    ledger.track_token_account("founder_collateral_wallet", addresses.founder_wallet);
    ledger.admit_founding(addresses.hoard, addresses.aggregate, claim_unit_atoms);
    ledger.track_position("founder_position", addresses.founder_position);
    for (label, address) in [
        ("founding_market", addresses.founding_market),
        ("found31_market", addresses.found31_market),
        ("claims_admission", addresses.admission),
        ("custody_replay", addresses.custody_replay),
        ("lifecycle_rent_credit", addresses.rent_credit),
    ] {
        ledger.watch(label, address);
    }
    Ok((claim_unit_atoms, decimals))
}

/// Give N synthetic holders a live collateral token account and a share of the
/// founder's remaining collateral.
///
/// This is the COLLATERAL leg of post-Open life, and only that. It is the
/// prestate a holder needs before they can acquire outcome tokens; acquiring
/// them is a Claims mutation, which the journey records as its named gap. The
/// distinction matters and the transcript keeps it: nothing here mints, moves,
/// or implies a single unit of any outcome.
#[allow(clippy::too_many_arguments)]
pub(crate) fn distribute_collateral(
    rpc: &mut Rpc,
    addresses: &MarketAddressesV1,
    payer: &Keypair,
    decimals: u8,
    holder_count: u32,
    ledger: &mut ConservationLedgerV1,
    transactions: &mut Vec<TransactionEvidence>,
    accounts: &mut BTreeMap<String, AccountEvidence>,
) -> Result<(Vec<HolderV1>, StageReportV1)> {
    if holder_count == 0 {
        return Err(Error::new(
            "a journey with no holders is the founding again; --holders must be positive",
        ));
    }
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let wallet_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
    let founder_before = {
        let account = rpc.required_account(addresses.founder_wallet, "founder wallet")?;
        TokenAccount::parse(&account.data)
            .map_err(|error| Error::new(format!("founder wallet: {error:?}")))?
            .amount
    };
    // The founder keeps a share too: a distribution that empties the founder is
    // a different scenario, and a journey that only ever tests the boundary
    // case is not testing the ordinary one.
    let share = founder_before / u64::from(holder_count).saturating_add(1);
    if share == 0 {
        return Err(Error::new(format!(
            "{founder_before} collateral atoms cannot be split across {holder_count} holders and \
             the founder; raise the founding collateral or lower --holders"
        )));
    }

    let mut holders = Vec::with_capacity(holder_count as usize);
    let mut compute_units = 0_u64;
    let mut submitted = 0_usize;
    for index in 0..holder_count {
        let owner = Keypair::new();
        let token = Keypair::new();
        let label = format!("holder_{index}");
        let mut initialize = Vec::with_capacity(33);
        initialize.push(INITIALIZE_ACCOUNT_3);
        initialize.extend_from_slice(owner.pubkey().as_ref());
        let mut transfer = Vec::with_capacity(10);
        transfer.push(TRANSFER_CHECKED);
        transfer.extend_from_slice(&share.to_le_bytes());
        transfer.push(decimals);
        let instructions = [
            create_account(
                &payer.pubkey(),
                &token.pubkey(),
                wallet_rent,
                ACCOUNT_BYTES as u64,
                &token_program,
            ),
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(token.pubkey(), false),
                    AccountMeta::new_readonly(addresses.mint, false),
                ],
                data: initialize,
            },
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(addresses.founder_wallet, false),
                    AccountMeta::new_readonly(addresses.mint, false),
                    AccountMeta::new(token.pubkey(), false),
                    AccountMeta::new_readonly(payer.pubkey(), true),
                ],
                data: transfer,
            },
        ];
        let evidence = rpc.send_with_signers(
            &format!("journey: open and fund synthetic holder {index}"),
            &instructions,
            payer,
            &[&token],
        )?;
        compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
        submitted += 1;
        transactions.push(evidence);

        let account = rpc.required_account(token.pubkey(), &label)?;
        let parsed = TokenAccount::parse(&account.data)
            .map_err(|error| Error::new(format!("{label} token account: {error:?}")))?;
        if parsed.mint != addresses.mint.to_bytes()
            || parsed.owner != owner.pubkey().to_bytes()
            || parsed.amount != share
            || parsed.state != AccountState::Initialized
        {
            return Err(Error::new(format!(
                "{label} did not reach the exact funded holder poststate"
            )));
        }
        accounts.insert(format!("journey_{label}"), account_evidence(token.pubkey(), &account));
        ledger.track_token_account(&label, token.pubkey());
        holders.push(HolderV1 {
            label,
            owner,
            token_account: token.pubkey(),
        });
    }

    Ok((
        holders,
        StageReportV1 {
            stage: "post-open life: collateral distribution".into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "{holder_count} synthetic holders opened a Token-2022 collateral account and \
                 received {share} atoms each from the founder. Collateral only: acquiring outcome \
                 tokens is a Claims mutation and is this journey's named gap."
            ),
        },
    ))
}

/// Move collateral holder-to-holder, with nobody privileged in the transfer.
///
/// The founder is not a party. That is the point: the ledger's laws have to
/// hold over movement the founding never authorised and cannot see.
pub(crate) fn holder_to_holder(
    rpc: &mut Rpc,
    addresses: &MarketAddressesV1,
    payer: &Keypair,
    decimals: u8,
    holders: &[HolderV1],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<StageReportV1> {
    if holders.len() < 2 {
        return Ok(StageReportV1 {
            stage: "post-open life: holder-to-holder collateral".into(),
            outcome: "blocked".into(),
            transactions: 0,
            compute_units: 0,
            note: "a holder-to-holder transfer needs at least two holders; run with --holders 2 or \
                   more."
                .into(),
        });
    }
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let mut compute_units = 0_u64;
    let mut submitted = 0_usize;
    // One hop around the ring: every holder both sends and receives, so no
    // holder's balance is left untouched by the stage the ledger then checks.
    for index in 0..holders.len() {
        let source = &holders[index];
        let destination = &holders[(index + 1) % holders.len()];
        let account = rpc.required_account(source.token_account, &source.label)?;
        let held = TokenAccount::parse(&account.data)
            .map_err(|error| Error::new(format!("{} token account: {error:?}", source.label)))?
            .amount;
        let amount = held / 4;
        if amount == 0 {
            continue;
        }
        let mut data = Vec::with_capacity(10);
        data.push(TRANSFER_CHECKED);
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(decimals);
        let evidence = rpc.send_with_signers(
            &format!(
                "journey: holder-to-holder collateral transfer {} -> {}",
                source.label, destination.label
            ),
            &[Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(source.token_account, false),
                    AccountMeta::new_readonly(addresses.mint, false),
                    AccountMeta::new(destination.token_account, false),
                    AccountMeta::new_readonly(source.owner.pubkey(), true),
                ],
                data,
            }],
            payer,
            &[&source.owner],
        )?;
        compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
        submitted += 1;
        transactions.push(evidence);
    }
    Ok(StageReportV1 {
        stage: "post-open life: holder-to-holder collateral".into(),
        outcome: "executed".into(),
        transactions: submitted,
        compute_units,
        note: format!(
            "{submitted} transfers around a ring of {} holders, each signed by the holding \
             wallet and by nobody privileged in the Market.",
            holders.len()
        ),
    })
}

/// Recover the rent the founding accumulated, and prove the floor holds.
///
/// The lifecycle credit collects the rent of every account the founding closed
/// into it. `Sweep` is the only route that moves that surplus back out while
/// the Market is alive, it takes three accounts, and it needs no signature at
/// all — so the adversarial half is the one that matters: a sweep of one
/// lamport more than the surplus must refuse, or the credit could be drained
/// below its own rent minimum and the Market's rent beneficiary would cease to
/// exist mid-life.
pub(crate) fn recover_rent(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    addresses: &MarketAddressesV1,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<StageReportV1> {
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let credit = rpc.required_account(addresses.rent_credit, "lifecycle RentCreditV2")?;
    let state = LifecycleRentCreditV2::decode(&credit.data)
        .map_err(|error| Error::new(format!("lifecycle RentCreditV2: {error:?}")))?;
    // The refund wallet is immutable, pinned when the credit was created. The
    // journey takes it from the credit's own bytes rather than assuming it is
    // the founder, because "the payer is the beneficiary" is a property of this
    // campaign's spec and not of the route.
    let wallet = Pubkey::new_from_array(state.refund_wallet().to_bytes());
    let minimum = rpc.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2)?;
    let surplus = credit.lamports.saturating_sub(minimum);
    if surplus == 0 {
        return Ok(StageReportV1 {
            stage: "rent recovery".into(),
            outcome: "blocked".into(),
            transactions: 0,
            compute_units: 0,
            note: format!(
                "the lifecycle credit holds {} lamports against a {minimum}-lamport rent minimum, \
                 so there is no surplus to sweep. rent/process_sweep_v2#Sweep stays unexecuted.",
                credit.lamports
            ),
        });
    }

    let sweep = |amount: u64| -> Result<Instruction> {
        Ok(Instruction {
            program_id: rent_program,
            accounts: vec![
                AccountMeta::new(addresses.rent_credit, false),
                AccountMeta::new(wallet, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
            ],
            data: SweepLifecycleRentCreditV2::new(amount)
                .map_err(|error| Error::new(format!("sweep request: {error:?}")))?
                .to_bytes()
                .to_vec(),
        })
    };

    let mut compute_units = 0_u64;
    let over = sweep(surplus.saturating_add(1))?;
    let refused = rpc.send_expected_failure(
        "journey: sweeping past the rent floor refuses",
        &[over],
        payer,
    )?;
    compute_units = compute_units.saturating_add(refused.compute_units_consumed.unwrap_or(0));
    transactions.push(refused);
    let after_refusal = rpc.required_account(addresses.rent_credit, "lifecycle RentCreditV2")?;
    if after_refusal.lamports != credit.lamports {
        return Err(Error::new(
            "the refused over-sweep still moved lamports out of the lifecycle credit",
        ));
    }

    let wallet_before = rpc.account(wallet)?.map(|value| value.lamports).unwrap_or(0);
    let executed = rpc.send(
        "journey: sweep lifecycle rent surplus to the refund wallet",
        &[sweep(surplus)?],
        payer,
    )?;
    compute_units = compute_units.saturating_add(executed.compute_units_consumed.unwrap_or(0));
    let fee = executed
        .fee_lamports
        .ok_or_else(|| Error::new("the sweep transaction omitted its exact fee"))?;
    transactions.push(executed);

    let after = rpc.required_account(addresses.rent_credit, "lifecycle RentCreditV2")?;
    let wallet_after = rpc.required_account(wallet, "lifecycle refund wallet")?;
    if after.lamports != minimum {
        return Err(Error::new(format!(
            "the sweep left {} lamports in the credit, not the {minimum}-lamport rent minimum",
            after.lamports
        )));
    }
    // The refund wallet is also the fee payer here, so the exact expectation is
    // surplus in, one fee out. Stating it that precisely is what makes this a
    // check rather than a direction-of-travel observation.
    if wallet_after.lamports.checked_add(fee) != wallet_before.checked_add(surplus) {
        return Err(Error::new(format!(
            "the refund wallet moved from {wallet_before} to {} lamports; a {surplus}-lamport \
             sweep less a {fee}-lamport fee does not account for that",
            wallet_after.lamports
        )));
    }

    Ok(StageReportV1 {
        stage: "rent recovery".into(),
        outcome: "executed".into(),
        transactions: 2,
        compute_units,
        note: format!(
            "swept {surplus} lamports of accumulated lifecycle rent to the immutable refund \
             wallet, leaving exactly the {minimum}-lamport rent minimum, after proving a sweep of \
             one lamport more refuses and moves nothing."
        ),
    })
}
