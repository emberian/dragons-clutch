//! The journey's conservation ledger.
//!
//! One object, threaded through every stage, that re-reads the whole economic
//! state from the chain at each stage boundary and evaluates the same fixed set
//! of laws over it. It is deliberately NOT a set of per-step spot checks: a
//! spot check answers "did this transaction do what it said," and a market can
//! pass every one of those while leaking atoms across the seams between them.
//!
//! ## The laws
//!
//! **L1 collateral closure.** The sum of every collateral-mint token account
//! the ledger tracks equals the Mint's own `supply`. This is the law that
//! catches an escape: an atom that moved to an account nobody registered makes
//! the tracked sum fall short of a supply that did not change.
//!
//! **L2 Hoard movement is declared.** Between boundaries the Hoard's balance may
//! change only by the amount the stage DECLARED it would change. This is the
//! law L1 cannot state: collateral moving from the Hoard into a wallet the
//! ledger already tracks leaves L1 perfectly balanced, and that movement is
//! exactly what an undetected leak of principal looks like. Stating it as
//! `hoard + Σ wallets == supply` instead — the obvious phrasing — would have
//! been L1's arithmetic rewritten, unable to fail on its own, and a law that
//! cannot fail independently is decoration.
//!
//! **L3 supply-vector agreement.** For every outcome `i`, the sum over all
//! tracked Positions of `balance(i)` equals the Claims aggregate's `supply(i)`.
//! The aggregate is the market's own account of what it owes; the Positions are
//! who it owes it to. A transfer that credits without debiting breaks this even
//! when both accounts individually look well-formed.
//!
//! **L4 full collateralisation.** `hoard >= max_i supply(i) * unit`. dClutch's
//! whole premise is that no outcome can be under-funded, so the worst outcome
//! is the one that has to be covered. `unit` is read from the Registry's own
//! published `ProductBasisV3.payout_scale`, not from the Hoard divided by the
//! outstanding supply — that phrasing would make L4 assert that the founding
//! equals itself.
//!
//! **L5 stage delta.** Between consecutive observations the change in tracked
//! collateral must equal the change the stage DECLARED. A stage that moves
//! atoms says how many before it runs; a stage that says zero and moves any is
//! a failure even if L1 still balances afterwards, because L1 balances for a
//! transfer between two tracked accounts.
//!
//! **L6 rent conservation.** Lamports that leave a closed protocol account
//! arrive at the declared beneficiary. Rent is the one value in the system that
//! is not collateral and still must not evaporate.
//!
//! **L7 lamport accounting.** The fee payer's lamports move by exactly the fees
//! its own transactions paid, plus whatever landed in an account this ledger
//! watches. This is the law the other six cannot state, and the trading stages
//! are what forced it: L1..L5 are about collateral ATOMS and say nothing about
//! the lamport side of a fill, and L6 only fires when a watched account CLOSES,
//! so a route that quietly debited whoever submitted it — or that placed rent
//! into an account nobody named — passes all six. Stated as
//! `payer_delta + fees + watched_growth == 0`, it is the general form of the
//! per-transaction check the rent-sweep stage already made by hand for one
//! route ("the fee payer moved by exactly the fee and nothing else"), and it is
//! exactly the "debit == credit + fee" claim a fill has to satisfy.
//!
//! A law that cannot be evaluated at a given boundary — no aggregate yet, no
//! Hoard yet — is recorded as `Inapplicable` with the reason. It is never
//! silently skipped: a law that quietly stops applying is how a conservation
//! argument rots.

use std::collections::BTreeMap;

use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_custody_contract::{CompartmentV1, CustodyVaultSeedsV1};
use dclutch_token_svm::{MINT_BYTES, Mint, TokenAccount};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

use crate::{Error, Result, rpc::Rpc};

/// What a stage claims about the lamports it moved, for L7.
///
/// `fees_lamports` is never a prediction: it is summed off the stage's own
/// transaction evidence, so L7 compares the chain against what the chain
/// charged rather than against a number this campaign chose.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct LamportClaimV1 {
    /// Fees the stage's own transactions paid, summed from their evidence.
    pub(crate) fees_lamports: u64,
    /// Lamports the stage placed in accounts this ledger does not watch.
    ///
    /// There is exactly one legitimate source of these and it is named in
    /// `unwatched_note`: an address lookup table's address is derived from the
    /// slot that created it, so it cannot be registered from before it exists
    /// the way every other account this journey creates is. The number is read
    /// off the tables themselves, not chosen — and a nonzero value with an
    /// empty note VIOLATES L7 rather than excusing it, because "declare
    /// whatever makes it balance" is the failure this term could otherwise
    /// become.
    pub(crate) unwatched_lamports: u64,
    pub(crate) unwatched_note: String,
    /// Why L7 cannot be evaluated at this boundary, when it cannot. A stage
    /// that cannot account for its lamports says so; it never stays silent.
    pub(crate) inapplicable: Option<String>,
}

impl LamportClaimV1 {
    /// A stage whose lamport movement this ledger does not account for, and why.
    pub(crate) fn inapplicable(reason: impl Into<String>) -> Self {
        Self {
            fees_lamports: 0,
            unwatched_lamports: 0,
            unwatched_note: String::new(),
            inapplicable: Some(reason.into()),
        }
    }

    /// A stage that paid exactly these fees and placed everything else in
    /// accounts the ledger watches.
    pub(crate) fn fees(fees_lamports: u64) -> Self {
        Self {
            fees_lamports,
            unwatched_lamports: 0,
            unwatched_note: String::new(),
            inapplicable: None,
        }
    }

    /// A stage that also rent-funded routing tables, whose addresses depend on
    /// the slot that created them and so cannot be watched in advance.
    pub(crate) fn with_unwatched(mut self, lamports: u64, note: impl Into<String>) -> Self {
        self.unwatched_lamports = lamports;
        self.unwatched_note = note.into();
        self
    }
}

/// One law's outcome at one stage boundary.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct VerdictV1 {
    /// `L1`..`L7`.
    pub(crate) law: String,
    /// `holds`, `violated`, or `inapplicable`.
    pub(crate) status: String,
    /// The arithmetic, or the reason the law does not apply here.
    pub(crate) detail: String,
}

impl VerdictV1 {
    fn holds(law: &str, detail: String) -> Self {
        Self {
            law: law.into(),
            status: "holds".into(),
            detail,
        }
    }

    fn violated(law: &str, detail: String) -> Self {
        Self {
            law: law.into(),
            status: "violated".into(),
            detail,
        }
    }

    fn inapplicable(law: &str, detail: &str) -> Self {
        Self {
            law: law.into(),
            status: "inapplicable".into(),
            detail: detail.into(),
        }
    }

    fn failed(&self) -> bool {
        self.status == "violated"
    }
}

/// Every physical Custody compartment, with the label L8 reports it under.
///
/// All nine, deliberately. A per-class law that omitted `HoardPrincipal`
/// because L2 already watches it would leave the same hole in mirror image:
/// atoms could cross from the Hoard into a class L8 does not name, and the two
/// laws would each see a movement the other was supposed to cover.
const COMPARTMENTS: [(CompartmentV1, &str); 9] = [
    (CompartmentV1::None, "None"),
    (CompartmentV1::External, "External"),
    (CompartmentV1::Settlement, "Settlement"),
    (CompartmentV1::HoardPrincipal, "HoardPrincipal"),
    (CompartmentV1::TradingPrincipal, "TradingPrincipal"),
    (CompartmentV1::FeeVault, "FeeVault"),
    (CompartmentV1::LivenessVault, "LivenessVault"),
    (CompartmentV1::SeriesEscrow, "SeriesEscrow"),
    (CompartmentV1::RecoveryReserve, "RecoveryReserve"),
];

/// The class of a tracked collateral account that is not a Custody vault under
/// any namespace this ledger can derive — an ordinary wallet, or a vault under
/// a context the journey has not admitted.
///
/// It is a CLASS, not a bucket to hide in: L8 holds it to a declared delta like
/// every other, so atoms moving into or out of it must still be stated.
const UNCLASSIFIED: &str = "unclassified";

/// The exact state of one account the ledger tracks.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AccountStateV1 {
    pub(crate) address: String,
    pub(crate) exists: bool,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) data_len: usize,
}

/// One complete census of the economic state, plus every law evaluated on it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ObservationV1 {
    /// The stage boundary this census was taken at.
    pub(crate) stage: String,
    /// The finalized slot the census was read at.
    pub(crate) slot: u64,
    /// Collateral atoms the stage said it would move in or out of the tracked
    /// set, signed.
    pub(crate) declared_collateral_delta: i128,
    /// Collateral atoms the stage said it would move in or out of the Hoard,
    /// signed. Zero is the strong claim, not the absent one.
    pub(crate) declared_hoard_delta: i128,
    /// What the stage claims about the lamports it moved.
    pub(crate) lamports: LamportClaimV1,
    /// The fee payer's lamports at this census.
    pub(crate) payer_lamports: u64,
    pub(crate) mint_supply: u64,
    /// label -> raw atoms, for every tracked collateral token account.
    pub(crate) token_atoms: BTreeMap<String, u64>,
    /// compartment class -> raw atoms, summed by ADDRESS so an account tracked
    /// under several labels is counted once. This is L8's census.
    pub(crate) class_atoms: BTreeMap<String, u64>,
    /// What the stage claims it moved per class, signed. `None` means the stage
    /// has not stated a per-class claim at all, which makes L8 inapplicable
    /// rather than green: a stage that has not accounted for its classes says
    /// so, exactly as `LamportClaimV1::inapplicable` does for L7.
    pub(crate) declared_class_deltas: Option<BTreeMap<String, i128>>,
    pub(crate) tracked_collateral: u64,
    pub(crate) hoard_atoms: u64,
    pub(crate) outcome_count: u32,
    /// The Claims aggregate's own liability supply, one entry per outcome.
    pub(crate) aggregate_supply: Vec<u64>,
    /// label -> that Position's claim balances, one entry per outcome.
    pub(crate) position_balances: BTreeMap<String, Vec<u64>>,
    /// The sum over every tracked Position, one entry per outcome.
    pub(crate) position_totals: Vec<u64>,
    /// Every tracked account's exact state, protocol accounts included.
    pub(crate) accounts: BTreeMap<String, AccountStateV1>,
    pub(crate) verdicts: Vec<VerdictV1>,
}

/// What the ledger watches, and the laws it evaluates over it.
pub(crate) struct ConservationLedgerV1 {
    mint: Pubkey,
    hoard: Option<Pubkey>,
    aggregate: Option<Pubkey>,
    /// Collateral atoms one claim of one outcome is worth.
    claim_unit_atoms: u64,
    /// The key that pays for every journey-owned transaction. L7 is stated
    /// about this account because it is the only one every stage debits.
    payer: Pubkey,
    token_accounts: BTreeMap<String, Pubkey>,
    positions: BTreeMap<String, Pubkey>,
    watched: BTreeMap<String, Pubkey>,
    /// Vault address -> compartment class, DERIVED from
    /// [`CustodyVaultSeedsV1`], never declared by a caller. The compartment tag
    /// is one of the PDA's own seeds, so an account's class is a fact about its
    /// address: a mislabelled account cannot satisfy L8, and a law that read a
    /// self-declared class would be a guard whose two sides move together.
    vault_classes: BTreeMap<Pubkey, String>,
    /// The per-class claim the next `observe` will evaluate L8 against.
    pending_class_claim: Option<BTreeMap<String, i128>>,
    observations: Vec<ObservationV1>,
}

impl ConservationLedgerV1 {
    /// Start a ledger over one collateral Mint.
    ///
    /// The Hoard, the aggregate and the claim unit are not known until the
    /// founding commits, so they are admitted later. Until then the laws that
    /// need them record themselves inapplicable rather than vanishing.
    pub(crate) fn new(mint: Pubkey, payer: Pubkey) -> Self {
        Self {
            mint,
            hoard: None,
            aggregate: None,
            claim_unit_atoms: 0,
            payer,
            token_accounts: BTreeMap::new(),
            positions: BTreeMap::new(),
            watched: BTreeMap::new(),
            vault_classes: BTreeMap::new(),
            pending_class_claim: None,
            observations: Vec::new(),
        }
    }

    /// Register a collateral token account. Every one must be named: an
    /// unnamed account is exactly what L1 exists to catch.
    pub(crate) fn track_token_account(&mut self, label: &str, address: Pubkey) {
        self.token_accounts.insert(label.into(), address);
        self.watched.insert(label.into(), address);
    }

    /// Register a Claims Position whose balances participate in L3.
    pub(crate) fn track_position(&mut self, label: &str, address: Pubkey) {
        self.positions.insert(label.into(), address);
        self.watched.insert(label.into(), address);
    }

    /// Watch an account for its exact final state without giving it a role in
    /// the collateral or supply laws — rent credits, replays, permits.
    pub(crate) fn watch(&mut self, label: &str, address: Pubkey) {
        self.watched.insert(label.into(), address);
    }

    /// Derive every Custody vault address under one namespace and record the
    /// compartment each one IS.
    ///
    /// One call per `(market, release_set, context)` the journey can name from
    /// authenticated chain state. Nine `find_program_address` derivations per
    /// call, done once here rather than per census. A vault under a context the
    /// journey has not admitted stays [`UNCLASSIFIED`], which is a class L8
    /// holds to a declared delta like any other — never a hiding place.
    pub(crate) fn admit_custody_namespace(
        &mut self,
        custody_program: Pubkey,
        market: [u8; 32],
        release_set: [u8; 32],
        context: [u8; 32],
    ) {
        for (compartment, label) in COMPARTMENTS {
            let vault = Pubkey::find_program_address(
                &CustodyVaultSeedsV1::new(market, release_set, context, compartment).as_slices(),
                &custody_program,
            )
            .0;
            self.vault_classes.insert(vault, label.to_string());
        }
    }

    /// State what the next census's stage moved per compartment class, signed.
    ///
    /// Unused so far: no stage states a per-class claim yet, so L8 reports
    /// `inapplicable` and names the stage. Wiring the stages is the next unit,
    /// and until it lands L8 never reports a green it did not earn.
    #[allow(dead_code)]
    ///
    /// A class absent from `deltas` is a claim of zero, which is the strong
    /// statement. Not calling this at all leaves L8 inapplicable for that
    /// boundary and names the stage.
    pub(crate) fn declare_class_deltas(&mut self, deltas: BTreeMap<String, i128>) {
        self.pending_class_claim = Some(deltas);
    }

    /// The compartment an address IS, by derivation.
    fn class_of(&self, address: &Pubkey) -> String {
        self.vault_classes
            .get(address)
            .cloned()
            .unwrap_or_else(|| UNCLASSIFIED.to_string())
    }

    /// Admit the founding's Hoard, aggregate, and per-claim collateral unit.
    pub(crate) fn admit_founding(
        &mut self,
        hoard: Pubkey,
        aggregate: Pubkey,
        claim_unit_atoms: u64,
    ) {
        self.hoard = Some(hoard);
        self.aggregate = Some(aggregate);
        self.claim_unit_atoms = claim_unit_atoms;
        self.token_accounts.insert("hoard".into(), hoard);
        self.watched.insert("hoard".into(), hoard);
        self.watched.insert("claims_aggregate".into(), aggregate);
    }

    /// Take a census at a stage boundary and evaluate every law over it.
    ///
    /// `declared_collateral_delta` is what the stage said it would move; L5
    /// checks the chain against that claim rather than against zero.
    pub(crate) fn observe(
        &mut self,
        rpc: &mut Rpc,
        stage: &str,
        declared_collateral_delta: i128,
        declared_hoard_delta: i128,
        lamports: LamportClaimV1,
    ) -> Result<()> {
        let payer_lamports = rpc
            .account(self.payer)?
            .map(|account| account.lamports)
            .unwrap_or(0);
        let slot = rpc.finalized_slot()?;
        let mint_account = rpc.required_account(self.mint, "collateral Mint")?;
        let mint_bytes = mint_account
            .data
            .get(..MINT_BYTES)
            .ok_or_else(|| Error::new("collateral Mint is narrower than the base layout"))?;
        let mint = Mint::parse(mint_bytes)
            .map_err(|error| Error::new(format!("collateral Mint: {error:?}")))?;

        let mut token_atoms = BTreeMap::new();
        let mut tracked_collateral: u64 = 0;
        for (label, address) in &self.token_accounts {
            let atoms = match rpc.account(*address)? {
                None => 0,
                Some(account) => {
                    let state = TokenAccount::parse(&account.data).map_err(|error| {
                        Error::new(format!("{label} is not a token account: {error:?}"))
                    })?;
                    if state.mint != self.mint.to_bytes() {
                        return Err(Error::new(format!(
                            "{label} holds a different mint than the ledger tracks"
                        )));
                    }
                    state.amount
                }
            };
            tracked_collateral = tracked_collateral.checked_add(atoms).ok_or_else(|| {
                Error::new("tracked collateral overflowed u64, which no real supply can")
            })?;
            token_atoms.insert(label.clone(), atoms);
        }
        // L8's census. Summed by ADDRESS, for the same reason L7 is: the ledger
        // legitimately watches one account under several labels, and summing per
        // label would count a class twice.
        let mut atoms_by_address: BTreeMap<Pubkey, u64> = BTreeMap::new();
        for (label, address) in &self.token_accounts {
            if let Some(atoms) = token_atoms.get(label) {
                atoms_by_address.insert(*address, *atoms);
            }
        }
        let mut class_atoms: BTreeMap<String, u64> = BTreeMap::new();
        for (address, atoms) in &atoms_by_address {
            let class = self.class_of(address);
            let total = class_atoms.entry(class).or_insert(0);
            *total = total.checked_add(*atoms).ok_or_else(|| {
                Error::new("a compartment class overflowed u64, which no real supply can")
            })?;
        }

        let hoard_atoms = self
            .hoard
            .and_then(|_| token_atoms.get("hoard").copied())
            .unwrap_or(0);

        let (outcome_count, aggregate_supply) = match self.aggregate {
            None => (0, Vec::new()),
            Some(address) => {
                let account = rpc.required_account(address, "Claims aggregate")?;
                let view = LiabilityBasisMarketViewV2::decode(&account.data)
                    .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
                let count = view.claim_count;
                let mut supply = Vec::with_capacity(count as usize);
                for index in 0..count {
                    supply.push(view.supply(&account.data, index).map_err(|error| {
                        Error::new(format!("aggregate supply {index}: {error:?}"))
                    })?);
                }
                (count, supply)
            }
        };

        let mut position_balances = BTreeMap::new();
        let mut position_totals = vec![0_u64; outcome_count as usize];
        for (label, address) in &self.positions {
            let Some(account) = rpc.account(*address)? else {
                position_balances.insert(label.clone(), Vec::new());
                continue;
            };
            let view = LiabilityBasisPositionViewV2::decode(&account.data)
                .map_err(|error| Error::new(format!("{label} Position: {error:?}")))?;
            // A Position narrower or wider than the aggregate would make L3 sum
            // over a partial vector and still balance. Refuse instead: the law
            // is only meaningful over one common outcome width.
            if view.claim_count != outcome_count {
                return Err(Error::new(format!(
                    "{label} carries {} outcomes and the Claims aggregate owes over {outcome_count}; \
                     a conservation law cannot be evaluated across two widths",
                    view.claim_count
                )));
            }
            let mut balances = Vec::with_capacity(view.claim_count as usize);
            for index in 0..view.claim_count {
                let balance = view
                    .balance(&account.data, index)
                    .map_err(|error| Error::new(format!("{label} balance {index}: {error:?}")))?;
                balances.push(balance);
                if let Some(total) = position_totals.get_mut(index as usize) {
                    *total = total.checked_add(balance).ok_or_else(|| {
                        Error::new("Position totals overflowed u64, which no real supply can")
                    })?;
                }
            }
            position_balances.insert(label.clone(), balances);
        }

        let mut accounts = BTreeMap::new();
        for (label, address) in &self.watched {
            let state = match rpc.account(*address)? {
                None => AccountStateV1 {
                    address: address.to_string(),
                    exists: false,
                    owner: String::new(),
                    lamports: 0,
                    data_len: 0,
                },
                Some(account) => AccountStateV1 {
                    address: address.to_string(),
                    exists: true,
                    owner: account.owner.to_string(),
                    lamports: account.lamports,
                    data_len: account.data.len(),
                },
            };
            accounts.insert(label.clone(), state);
        }

        let mut observation = ObservationV1 {
            stage: stage.into(),
            slot,
            declared_collateral_delta,
            declared_hoard_delta,
            lamports,
            payer_lamports,
            mint_supply: mint.supply,
            token_atoms,
            class_atoms,
            declared_class_deltas: self.pending_class_claim.take(),
            tracked_collateral,
            hoard_atoms,
            outcome_count,
            aggregate_supply,
            position_balances,
            position_totals,
            accounts,
            verdicts: Vec::new(),
        };
        observation.verdicts = self.evaluate(&observation);
        self.observations.push(observation);
        Ok(())
    }

    fn evaluate(&self, now: &ObservationV1) -> Vec<VerdictV1> {
        let mut verdicts = Vec::new();

        // L1 collateral closure.
        verdicts.push(if now.tracked_collateral == now.mint_supply {
            VerdictV1::holds(
                "L1",
                format!(
                    "tracked {} atoms across {} accounts == Mint supply {}",
                    now.tracked_collateral,
                    now.token_atoms.len(),
                    now.mint_supply
                ),
            )
        } else {
            VerdictV1::violated(
                "L1",
                format!(
                    "tracked {} atoms across {} accounts != Mint supply {}; {} atoms are in accounts this ledger does not name",
                    now.tracked_collateral,
                    now.token_atoms.len(),
                    now.mint_supply,
                    now.mint_supply.abs_diff(now.tracked_collateral)
                ),
            )
        });

        // L2 Hoard movement is declared.
        verdicts.push(match (self.hoard, self.observations.last()) {
            (None, _) => {
                VerdictV1::inapplicable("L2", "no Hoard exists before the founding commits")
            }
            (Some(_), None) => {
                VerdictV1::inapplicable("L2", "the first census has no predecessor to move from")
            }
            (Some(_), Some(previous)) => {
                let observed = i128::from(now.hoard_atoms) - i128::from(previous.hoard_atoms);
                if observed == now.declared_hoard_delta {
                    VerdictV1::holds(
                        "L2",
                        format!(
                            "the Hoard moved {observed} atoms since `{}`, exactly as declared; it holds {}",
                            previous.stage, now.hoard_atoms
                        ),
                    )
                } else {
                    VerdictV1::violated(
                        "L2",
                        format!(
                            "the Hoard moved {observed} atoms since `{}` and the stage declared {}. \
                             L1 cannot see this: principal moving from the Hoard into a wallet this \
                             ledger already tracks leaves the total untouched.",
                            previous.stage, now.declared_hoard_delta
                        ),
                    )
                }
            }
        });

        // L3 supply-vector agreement.
        verdicts.push(if self.aggregate.is_none() {
            VerdictV1::inapplicable(
                "L3",
                "no Claims aggregate exists before the founding commits",
            )
        } else if now.position_totals == now.aggregate_supply {
            VerdictV1::holds(
                "L3",
                format!(
                    "{} Positions sum to the aggregate supply vector {:?}",
                    now.position_balances.len(),
                    now.aggregate_supply
                ),
            )
        } else {
            VerdictV1::violated(
                "L3",
                format!(
                    "Positions sum to {:?} but the aggregate owes {:?}",
                    now.position_totals, now.aggregate_supply
                ),
            )
        });

        // L4 full collateralisation.
        verdicts.push(match (self.aggregate, now.aggregate_supply.iter().max()) {
            (None, _) | (_, None) => VerdictV1::inapplicable(
                "L4",
                "no outstanding liability exists before the founding commits",
            ),
            (Some(_), Some(worst)) => match worst.checked_mul(self.claim_unit_atoms) {
                None => VerdictV1::violated(
                    "L4",
                    format!(
                        "worst outcome {worst} claims at {} atoms each overflows u64",
                        self.claim_unit_atoms
                    ),
                ),
                Some(required) if now.hoard_atoms >= required => VerdictV1::holds(
                    "L4",
                    format!(
                        "Hoard {} >= worst outcome {worst} x unit {} = {required}",
                        now.hoard_atoms, self.claim_unit_atoms
                    ),
                ),
                Some(required) => VerdictV1::violated(
                    "L4",
                    format!(
                        "Hoard {} < worst outcome {worst} x unit {} = {required}; the Market is under-collateralised",
                        now.hoard_atoms, self.claim_unit_atoms
                    ),
                ),
            },
        });

        // L5 stage delta.
        verdicts.push(match self.observations.last() {
            None => VerdictV1::inapplicable("L5", "the first census has no predecessor"),
            Some(previous) => {
                let observed =
                    i128::from(now.tracked_collateral) - i128::from(previous.tracked_collateral);
                if observed == now.declared_collateral_delta {
                    VerdictV1::holds(
                        "L5",
                        format!(
                            "tracked collateral moved {observed} atoms since `{}`, exactly as declared",
                            previous.stage
                        ),
                    )
                } else {
                    VerdictV1::violated(
                        "L5",
                        format!(
                            "tracked collateral moved {observed} atoms since `{}`; the stage declared {}",
                            previous.stage, now.declared_collateral_delta
                        ),
                    )
                }
            }
        });

        // L6 rent conservation.
        verdicts.push(match self.observations.last() {
            None => VerdictV1::inapplicable("L6", "the first census has no predecessor"),
            Some(previous) => {
                let vanished: Vec<String> = previous
                    .accounts
                    .iter()
                    .filter_map(|(label, before)| {
                        let after = now.accounts.get(label)?;
                        (before.exists && !after.exists && before.lamports > 0)
                            .then(|| format!("{label} ({} lamports)", before.lamports))
                    })
                    .collect();
                if vanished.is_empty() {
                    VerdictV1::holds("L6", "no watched account closed at this boundary".into())
                } else {
                    // A closure is not a violation; an unaccounted one is. The
                    // stage that closes an account states where the rent went,
                    // and the journey watches that destination, so the check is
                    // that SOMETHING watched grew by at least the closed rent.
                    let closed: u64 = previous
                        .accounts
                        .iter()
                        .filter_map(|(label, before)| {
                            let after = now.accounts.get(label)?;
                            (before.exists && !after.exists).then_some(before.lamports)
                        })
                        .sum();
                    let gained: u64 = now
                        .accounts
                        .iter()
                        .filter_map(|(label, after)| {
                            let before = previous.accounts.get(label)?;
                            after.lamports.checked_sub(before.lamports)
                        })
                        .sum();
                    if gained >= closed {
                        VerdictV1::holds(
                            "L6",
                            format!(
                                "closed {} lamports across {}; watched accounts gained {gained}",
                                closed,
                                vanished.join(", ")
                            ),
                        )
                    } else {
                        VerdictV1::violated(
                            "L6",
                            format!(
                                "closed {closed} lamports across {}; watched accounts gained only {gained}, so the rent went somewhere this journey does not watch",
                                vanished.join(", ")
                            ),
                        )
                    }
                }
            }
        });

        // L7 lamport accounting.
        // L8 per-class conservation.
        verdicts.push(self.evaluate_classes(now));

        verdicts.push(self.evaluate_lamports(now));

        verdicts
    }

    /// L8: every compartment class moved by exactly the amount its stage
    /// declared.
    ///
    /// This is L2's argument made for all nine classes instead of one. L1 and
    /// L5 are stated over a single total, so they balance for any transfer
    /// between two tracked accounts; L2 closes that for the Hoard alone. A
    /// transfer between any other pair of compartments passes every one of
    /// L1..L7, which is the cross-subsidy C-10 exists to forbid.
    ///
    /// The class is DERIVED from the vault's own PDA seeds, so this law cannot
    /// be satisfied by relabelling an account.
    fn evaluate_classes(&self, now: &ObservationV1) -> VerdictV1 {
        let Some(declared) = &now.declared_class_deltas else {
            return VerdictV1::inapplicable(
                "L8",
                &format!(
                    "stage `{}` has not stated a per-class claim; L8 resumes when it does",
                    now.stage
                ),
            );
        };
        let Some(previous) = self.observations.last() else {
            return VerdictV1::inapplicable("L8", "the first census has no predecessor");
        };
        let mut classes: Vec<&str> = previous
            .class_atoms
            .keys()
            .chain(now.class_atoms.keys())
            .map(String::as_str)
            .collect();
        classes.sort_unstable();
        classes.dedup();
        let mut breaches = Vec::new();
        let mut held = Vec::new();
        for class in classes {
            let before = i128::from(previous.class_atoms.get(class).copied().unwrap_or(0));
            let after = i128::from(now.class_atoms.get(class).copied().unwrap_or(0));
            let observed = after - before;
            let expected = declared.get(class).copied().unwrap_or(0);
            if observed == expected {
                held.push(format!("{class} {observed:+}"));
            } else {
                breaches.push(format!(
                    "{class} moved {observed:+} atoms and the stage declared {expected:+}"
                ));
            }
        }
        if breaches.is_empty() {
            VerdictV1::holds(
                "L8",
                format!(
                    "every compartment moved exactly as declared since `{}`: {}",
                    previous.stage,
                    held.join(", ")
                ),
            )
        } else {
            VerdictV1::violated(
                "L8",
                format!(
                    "{}. L1 and L5 cannot see this: a transfer between two tracked \
                     compartments leaves the single total untouched, and L2 covers only the Hoard.",
                    breaches.join("; ")
                ),
            )
        }
    }

    /// L7: the payer's lamports moved by exactly its fees plus what landed in
    /// accounts this ledger watches.
    ///
    /// The growth term is summed only over labels present at BOTH boundaries.
    /// A label the stage introduced has no predecessor balance to subtract, and
    /// silently treating its whole balance as growth would let a stage admit an
    /// account into the ledger and its own leak in the same breath. So a
    /// boundary that introduced a watched label reports `inapplicable` and
    /// NAMES the labels, rather than reporting a green it did not earn.
    fn evaluate_lamports(&self, now: &ObservationV1) -> VerdictV1 {
        if let Some(reason) = &now.lamports.inapplicable {
            return VerdictV1::inapplicable("L7", reason);
        }
        let Some(previous) = self.observations.last() else {
            return VerdictV1::inapplicable("L7", "the first census has no predecessor");
        };
        let introduced: Vec<&str> = now
            .accounts
            .keys()
            .filter(|label| !previous.accounts.contains_key(*label))
            .map(String::as_str)
            .collect();
        if !introduced.is_empty() {
            return VerdictV1::inapplicable(
                "L7",
                &format!(
                    "this boundary admitted {} account(s) the previous census did not watch ({}), \
                     so their balances have no predecessor to difference against; L7 resumes at \
                     the next boundary",
                    introduced.len(),
                    introduced.join(", ")
                ),
            );
        }
        // Sum by ADDRESS, not by label. The ledger legitimately watches one
        // account under several names -- the discovery loop names accounts by
        // the founding's own evidence keys, and several of those keys point at
        // one account: the Market's rent beneficiary IS the founding's
        // lifecycle credit, the Found31 Market is the `market` record, the
        // normal and projected Custody replays are the same realized account,
        // and the fee payer is also a credit's refund wallet. Summing per label
        // counted every change to those FOUR addresses twice, and the first run
        // that closed accounts and refunded rent turned that into three L7
        // violations whose residuals were exactly the doubled amounts.
        //
        // The payer is then excluded outright rather than subtracted back out:
        // it appears under two labels here, so subtracting "the payer's label"
        // removed one copy and left the other.
        let payer = self.payer.to_string();
        let mut before_by_address: BTreeMap<&str, u64> = BTreeMap::new();
        let mut after_by_address: BTreeMap<&str, u64> = BTreeMap::new();
        for (label, after) in &now.accounts {
            if after.address == payer {
                continue;
            }
            let Some(before) = previous.accounts.get(label) else {
                continue;
            };
            before_by_address.insert(before.address.as_str(), before.lamports);
            after_by_address.insert(after.address.as_str(), after.lamports);
        }
        let mut watched_growth: i128 = 0;
        for (address, after) in &after_by_address {
            let before = before_by_address.get(address).copied().unwrap_or(0);
            watched_growth += i128::from(*after) - i128::from(before);
        }
        let payer_delta = i128::from(now.payer_lamports) - i128::from(previous.payer_lamports);

        if now.lamports.unwatched_lamports > 0 && now.lamports.unwatched_note.trim().is_empty() {
            return VerdictV1::violated(
                "L7",
                format!(
                    "the stage declared {} lamports placed outside the watched set and did not say \
                     where; an undescribed declaration is a hole with a number in it",
                    now.lamports.unwatched_lamports
                ),
            );
        }
        let residual = payer_delta
            + i128::from(now.lamports.fees_lamports)
            + watched_growth
            + i128::from(now.lamports.unwatched_lamports);
        if residual == 0 {
            VerdictV1::holds(
                "L7",
                format!(
                    "the payer moved {payer_delta} lamports since `{}`, its transactions paid {} in \
                     fees, watched accounts gained {}, and {} went to {}; debit == credit + fee",
                    previous.stage,
                    now.lamports.fees_lamports,
                    watched_growth,
                    now.lamports.unwatched_lamports,
                    if now.lamports.unwatched_note.is_empty() {
                        "nothing unwatched"
                    } else {
                        now.lamports.unwatched_note.as_str()
                    }
                ),
            )
        } else {
            VerdictV1::violated(
                "L7",
                format!(
                    "the payer moved {payer_delta} lamports since `{}` and its transactions paid {} \
                     in fees, but watched accounts gained {}; {residual} lamports are unaccounted \
                     for. L1..L6 cannot see this: they are stated about collateral atoms and about \
                     accounts that CLOSE, and nothing closed here.",
                    previous.stage, now.lamports.fees_lamports, watched_growth
                ),
            )
        }
    }

    /// Every census taken, in order.
    /// Reload observations a previous invocation of an EXTERNAL census took,
    /// so the delta laws (L2, L5, L7's fee history) evaluate across process
    /// boundaries. The chain stays the authority for every ABSOLUTE law; what
    /// this restores is only what a prior census DECLARED and read.
    ///
    /// Used by the successor driver's `ledger-census`; the journey itself
    /// never restores — its boundaries all live in one process.
    #[allow(dead_code)]
    pub(crate) fn restore_observations(&mut self, observations: Vec<ObservationV1>) {
        self.observations = observations;
    }

    pub(crate) fn observations(&self) -> &[ObservationV1] {
        &self.observations
    }

    /// Every violated law across the whole journey, named by stage.
    pub(crate) fn violations(&self) -> Vec<String> {
        self.observations
            .iter()
            .flat_map(|observation| {
                observation
                    .verdicts
                    .iter()
                    .filter(|verdict| verdict.failed())
                    .map(move |verdict| {
                        format!("{}: {} {}", observation.stage, verdict.law, verdict.detail)
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUSTODY: u8 = 0xc5;
    const MARKET: [u8; 32] = [0xc6; 32];
    const RELEASE: [u8; 32] = [0xc7; 32];
    const CONTEXT: [u8; 32] = [0xc8; 32];

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    /// The real vault address for one compartment, derived exactly as Custody
    /// derives it. The tests therefore exercise the classifier, not a label.
    fn vault(compartment: CompartmentV1) -> Pubkey {
        Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(MARKET, RELEASE, CONTEXT, compartment).as_slices(),
            &key(CUSTODY),
        )
        .0
    }

    fn account(address: Pubkey, lamports: u64) -> AccountStateV1 {
        AccountStateV1 {
            address: address.to_string(),
            exists: true,
            owner: key(0xb0).to_string(),
            lamports,
            data_len: 165,
        }
    }

    fn new_ledger() -> ConservationLedgerV1 {
        let mut ledger = ConservationLedgerV1::new(key(0xc1), key(0xd1));
        ledger.admit_custody_namespace(key(CUSTODY), MARKET, RELEASE, CONTEXT);
        ledger.admit_founding(vault(CompartmentV1::HoardPrincipal), key(0xa4), 10);
        ledger.track_token_account(
            "trading_principal_vault",
            vault(CompartmentV1::TradingPrincipal),
        );
        ledger.track_token_account("fee_vault", vault(CompartmentV1::FeeVault));
        ledger
    }

    /// A census in which the market holds `hoard` principal, `trading` in a
    /// TradingPrincipal vault and `fee` in a FeeVault, against one outcome of
    /// `supply` claims worth ten atoms each. `class_atoms` is built through the
    /// ledger's own derivation, so a wrong classifier fails these tests.
    fn census(
        ledger: &ConservationLedgerV1,
        stage: &str,
        hoard: u64,
        trading: u64,
        fee: u64,
        supply: u64,
        declared_class_deltas: Option<BTreeMap<String, i128>>,
    ) -> ObservationV1 {
        let tracked = hoard + trading + fee;
        let balances = [
            (vault(CompartmentV1::HoardPrincipal), "hoard", hoard),
            (
                vault(CompartmentV1::TradingPrincipal),
                "trading_principal_vault",
                trading,
            ),
            (vault(CompartmentV1::FeeVault), "fee_vault", fee),
        ];
        let mut token_atoms = BTreeMap::new();
        let mut class_atoms: BTreeMap<String, u64> = BTreeMap::new();
        let mut accounts = BTreeMap::new();
        for (address, label, atoms) in balances {
            token_atoms.insert(label.to_string(), atoms);
            *class_atoms.entry(ledger.class_of(&address)).or_insert(0) += atoms;
            accounts.insert(label.to_string(), account(address, 2_039_280));
        }
        ObservationV1 {
            stage: stage.into(),
            slot: 1,
            declared_collateral_delta: 0,
            declared_hoard_delta: 0,
            lamports: LamportClaimV1::inapplicable("this census is synthetic"),
            payer_lamports: 1_000_000,
            mint_supply: tracked,
            token_atoms,
            class_atoms,
            declared_class_deltas,
            tracked_collateral: tracked,
            hoard_atoms: hoard,
            outcome_count: 1,
            aggregate_supply: vec![supply],
            position_balances: BTreeMap::from([("holder".into(), vec![supply])]),
            position_totals: vec![supply],
            accounts,
            verdicts: Vec::new(),
        }
    }

    fn moved(entries: &[(&str, i128)]) -> Option<BTreeMap<String, i128>> {
        Some(
            entries
                .iter()
                .map(|(class, delta)| ((*class).to_string(), *delta))
                .collect(),
        )
    }

    fn verdict<'a>(verdicts: &'a [VerdictV1], law: &str) -> &'a VerdictV1 {
        verdicts
            .iter()
            .find(|value| value.law == law)
            .unwrap_or_else(|| panic!("{law} must be evaluated"))
    }

    /// The classifier is a derivation, not a label: every one of the nine
    /// compartments resolves to its own class, and an address that is not a
    /// vault under an admitted namespace is `unclassified` rather than being
    /// folded into one.
    #[test]
    fn every_compartment_is_recovered_from_its_own_pda_seeds() {
        let ledger = new_ledger();
        for (compartment, label) in COMPARTMENTS {
            assert_eq!(ledger.class_of(&vault(compartment)), label);
        }
        assert_eq!(ledger.class_of(&key(0xee)), UNCLASSIFIED);
        // A vault under a context the ledger has not admitted is not silently
        // classified as one it has.
        let foreign = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(MARKET, RELEASE, [0xfe; 32], CompartmentV1::FeeVault)
                .as_slices(),
            &key(CUSTODY),
        )
        .0;
        assert_eq!(ledger.class_of(&foreign), UNCLASSIFIED);
    }

    /// THE DEFECT, unchanged: L1..L7 are blind to a cross-class transfer.
    ///
    /// This is the control for L8, and it is asserted law by law rather than in
    /// aggregate, so a future change that alters one of the seven is caught here
    /// rather than quietly rewriting what L8 is compared against.
    #[test]
    fn the_original_seven_laws_hold_while_atoms_cross_from_trading_principal_to_the_fee_vault() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            None,
        )]);
        let after = census(&new_ledger(), "after", 1_000, 0, 500, 100, None);
        let verdicts = ledger.evaluate(&after);

        for law in ["L1", "L2", "L3", "L4", "L5", "L6"] {
            assert_eq!(verdict(&verdicts, law).status, "holds", "{law}");
        }
        assert_eq!(verdict(&verdicts, "L7").status, "inapplicable");
        // And L8 does not invent a green it did not earn: an undeclared stage is
        // inapplicable and says which stage.
        assert_eq!(verdict(&verdicts, "L8").status, "inapplicable");
        assert!(verdict(&verdicts, "L8").detail.contains("after"));
    }

    /// L8 RED on exactly the transfer the seven laws could not see.
    #[test]
    fn l8_catches_the_cross_class_transfer_the_other_laws_cannot_see() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            None,
        )]);
        // The stage claims it moved nothing at all.
        let after = census(&new_ledger(), "after", 1_000, 0, 500, 100, moved(&[]));
        let verdicts = ledger.evaluate(&after);

        let l8 = verdict(&verdicts, "L8");
        std::println!("L8 {} -- {}", l8.status, l8.detail);
        assert_eq!(l8.status, "violated");
        assert!(l8.detail.contains("TradingPrincipal moved -500"));
        assert!(l8.detail.contains("FeeVault moved +500"));
        // The other seven are unchanged by L8's arrival.
        for law in ["L1", "L2", "L3", "L4", "L5", "L6"] {
            assert_eq!(verdict(&verdicts, law).status, "holds", "{law}");
        }
    }

    /// L8 GREEN when the stage states the movement it actually made.
    #[test]
    fn l8_admits_the_same_transfer_once_the_stage_declares_it() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            None,
        )]);
        let after = census(
            &new_ledger(),
            "after",
            1_000,
            0,
            500,
            100,
            moved(&[("TradingPrincipal", -500), ("FeeVault", 500)]),
        );
        let verdicts = ledger.evaluate(&after);
        let l8 = verdict(&verdicts, "L8");
        std::println!("L8 {} -- {}", l8.status, l8.detail);
        assert_eq!(l8.status, "holds");
    }

    /// L8 covers the Hoard too. A law that omitted it because L2 exists would
    /// leave the same hole in mirror image.
    #[test]
    fn l8_covers_the_hoard_and_does_not_defer_to_l2() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            None,
        )]);
        let after = census(&new_ledger(), "after", 500, 500, 500, 50, moved(&[]));
        let verdicts = ledger.evaluate(&after);
        assert_eq!(verdict(&verdicts, "L2").status, "violated");
        let l8 = verdict(&verdicts, "L8");
        assert_eq!(l8.status, "violated");
        assert!(l8.detail.contains("HoardPrincipal moved -500"));
    }
}
